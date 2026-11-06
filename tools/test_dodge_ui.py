"""💩 똥피하기 창: 커맨드·권한·메뉴 연결, 점수 부탁 형식, dodge.js를 가짜 DOM에서 돌려 본다 (브라우저 없이)."""

import json
from pathlib import Path

from conftest import needs_node, run_node

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"

HARNESS = r"""
const fs = require("fs");
class Node {
  constructor(tag) { this.tag = tag; this.children = []; this.className = ""; this.style = {}; this.text = ""; this.hidden = false; this.listeners = {}; this.classList = { add() {} }; }
  set textContent(v) { this.text = String(v); this.children = []; }
  get textContent() { return this.text + this.children.map((c) => (typeof c === "string" ? c : c.textContent)).join(""); }
  append(...xs) { for (const x of xs) { if (typeof x !== "string") x.parent = this; this.children.push(x); } }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter((c) => c !== this); }
  addEventListener(name, h) { this.listeners[name] = h; }
  set innerHTML(v) { throw new Error("innerHTML must not be used"); }
}
const ids = ["field", "hud", "card", "score", "record", "keys", "ranks", "start", "close"];
const byId = Object.fromEntries(ids.map((id) => [id, new Node("div")]));
global.document = { createElement: (t) => new Node(t), getElementById: (id) => byId[id], body: new Node("body") };
const calls = [];
let handler = null;
const winListeners = {};
const VIEWS = JSON.parse(process.argv[3]);
const KEYS = JSON.parse(process.argv[4]);
global.window = {
  addEventListener(name, h) { winListeners[name] = h; },
  __TAURI__: {
    core: { invoke: async (cmd, args) => { calls.push([cmd, args || null]); if (cmd === "dodge_state") return VIEWS[0]; } },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (name, h) => { if (name === "dodge-state") handler = h; } }) },
  },
};
const settle = () => new Promise((r) => setTimeout(r, 5));
const kids = (n) => n.children.filter((c) => typeof c !== "string");
const snap = () => ({
  poops: kids(byId.field).map((p) => ({ src: p.src, left: p.style.left, top: p.style.top, width: p.style.width })),
  hud: byId.hud.hidden ? null : byId.hud.textContent,
  card: byId.card.hidden ? null : {
    score: byId.score.hidden ? null : byId.score.textContent,
    record: !byId.record.hidden,
    keys: !byId.keys.hidden,
    start: byId.start.textContent,
    ranks: kids(byId.ranks).map((li) => ({ me: li.className === "me", cells: li.children.map((c) => c.textContent) })),
  },
});
require("vm").runInThisContext(fs.readFileSync(process.argv[2], "utf8"));
(async () => {
  await settle();
  const out = { snaps: [snap()] };
  for (const v of VIEWS.slice(1)) { handler({ payload: v }); out.snaps.push(snap()); }
  for (const [type, key, repeat] of KEYS) {
    winListeners[type]({ key, repeat: !!repeat, preventDefault() {} });
  }
  if (KEYS.length) winListeners.blur();
  await settle();
  out.calls = calls.filter(([c]) => c !== "dodge_state");
  console.log(JSON.stringify(out));
  process.exit(0);
})();
"""


def render(tmp_path, views, keys=()):
    script = tmp_path / "harness.js"
    script.write_text(HARNESS, encoding="utf-8")
    return json.loads(run_node(script, UI / "dodge.js", json.dumps(views), json.dumps(list(keys)), timeout=30).stdout)


def view(phase, poops=(), dodged=0, best=0, new_best=False, ranking=None):
    return {"phase": phase, "poops": [list(p) for p in poops], "dodged": dodged, "best": best, "new_best": new_best,
            "ranking": ranking if ranking is not None else [{"name": "나", "score": best, "me": True}]}


def test_commands_menu_and_capability_are_wired():
    main = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
    build = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
    for cmd in ("dodge_state", "dodge_keys", "dodge_start", "dodge_close"):
        assert f"dodge::{cmd}" in main and f'"{cmd}"' in build, cmd
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    perms = {p for c in caps if "dodge" in c["windows"] for p in c["permissions"]}
    assert perms == {"allow-dodge-state", "allow-dodge-keys", "allow-dodge-start", "allow-dodge-close", "core:event:allow-listen", "core:event:allow-unlisten"}
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert '"💩 똥피하기"' in tray and "DODGE_ID => send(app, Input::ShowDodge)" in tray


def test_scores_ride_the_ask_channel_older_versions_ignore():
    # 예전 판은 모르는 부탁을 무시한다 — 새 메시지 type을 만들지 않는다
    net = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    assert "dodge::parse_score_ask(&ask)" in net and "dodge::score_ask(" in net
    proto = (ROOT / "crates" / "screenpet-net" / "src" / "protocol.rs").read_text(encoding="utf-8")
    assert "dodge" not in proto.lower()
    core = (ROOT / "crates" / "screenpet-core" / "src" / "dodge.rs").read_text(encoding="utf-8")
    assert 'pub const ASK_PREFIX: &str = "dodge:";' in core


def test_page_is_plain():
    src = (UI / "dodge.html").read_text(encoding="utf-8") + (UI / "dodge.js").read_text(encoding="utf-8")
    assert "innerHTML" not in src and "setInterval" not in src


@needs_node
def test_ready_card_shows_ranking_and_keys(tmp_path):
    ranking = [{"name": "<b>민수</b>", "score": 30, "me": False}, {"name": "나", "score": 12, "me": True}]
    r = render(tmp_path, [view("ready", best=12, ranking=ranking)])["snaps"][0]
    assert r["hud"] is None and r["poops"] == []
    card = r["card"]
    assert card["score"] is None and card["keys"] and not card["record"] and card["start"] == "시작 (Space)"
    # 친구 이름은 글자 그대로, 나는 강조
    assert card["ranks"] == [{"me": False, "cells": ["1", "<b>민수</b>", "30"]}, {"me": True, "cells": ["2", "나", "12"]}]


@needs_node
def test_play_draws_poops_and_count_then_over_card(tmp_path):
    play1 = view("play", poops=[(0, 100, 50, 17), (1, 300, 200, 17)], dodged=3)
    play2 = view("play", poops=[(1, 300, 260, 17), (2, 500, -10, 17)], dodged=4)
    over = view("over", poops=[(1, 300, 300, 17)], dodged=4, best=4, new_best=True)
    snaps = render(tmp_path, [view("ready"), play1, play2, over])["snaps"]
    p1, p2, o = snaps[1], snaps[2], snaps[3]
    assert p1["card"] is None and p1["hud"] == "💩 3"
    assert [x["left"] for x in p1["poops"]] == ["79.6px", "279.6px"] and p1["poops"][0]["width"] == "40.8px"
    assert all(x["src"] == "objects/poop.png" for x in p1["poops"])
    # 바닥에 닿은 똥은 지우고, 남은 똥은 같은 그림을 옮기고, 새 똥은 더한다
    assert len(p2["poops"]) == 2 and p2["poops"][0]["top"] == f"{260 - 20.4}px" and p2["hud"] == "💩 4"
    # 끝: 맞은 자리의 똥은 그대로, 결과 카드
    assert len(o["poops"]) == 1 and o["hud"] is None
    assert o["card"]["score"] == "4개 피함" and o["card"]["record"] and not o["card"]["keys"] and o["card"]["start"] == "다시 (Space)"


@needs_node
def test_keys_go_to_the_game_space_starts_escape_closes(tmp_path):
    keys = [
        ["keydown", " "],            # 시작 카드에서 Space = 시작 (점프 비트는 보내지 않는다)
        ["keydown", "ArrowLeft"],
        ["keydown", "ArrowLeft", True],  # 누르고 있는 반복은 다시 보내지 않는다
        ["keydown", "Shift"],
        ["keyup", "ArrowLeft"],
        ["keydown", "a"],            # 모르는 키
        ["keydown", "Escape"],
    ]
    r = render(tmp_path, [view("ready")], keys)
    assert r["calls"] == [
        ["dodge_start", None],
        ["dodge_keys", {"bits": 1}],
        ["dodge_keys", {"bits": 33}],
        ["dodge_keys", {"bits": 32}],
        ["dodge_close", None],
        ["dodge_keys", {"bits": 0}],  # 창을 떠나면 다 놓은 것으로
    ]


@needs_node
def test_pixel_style_uses_pixel_poop(tmp_path):
    harness = tmp_path / "harness.js"
    harness.write_text(HARNESS.replace("global.window = {", "global.window = { ART_STYLE: \"pixel\",", 1), encoding="utf-8")
    out = json.loads(run_node(harness, UI / "dodge.js", json.dumps([view("play", poops=[(0, 10, 10, 17)])]), "[]", timeout=30).stdout)
    assert out["snaps"][0]["poops"][0]["src"] == "objects_pixel/poop.png"


def test_review_fixes_for_the_app_side():
    src = (ROOT / "src-tauri" / "src" / "dodge.rs").read_text(encoding="utf-8")
    # 하우스 안이면 하우스 창에 말한다
    assert "speech_target(&own.pet)" in src and 'emit_to(OWN_LABEL, "pet-say-text"' not in src
    # 게임 중 하우스에 들어가거나 친구에게 가면 판을 닫는다
    tick = src[src.index("pub fn tick") :]
    assert "can_play(own)" in tick[: tick.index("fn publish")]
    # 새 기록은 몰아서 한 번만 알린다 (친구의 부탁 예산을 함께 쓴다)
    assert "SHARE_AFTER" in src and "network.share_dodge_best(" in tick
    assert src.count("share_dodge_best(") == 1
    # 다시 시작할 때 지난 판의 눌린 키가 남지 않는다
    start = src[src.index("DodgeInput::Start =>") :]
    assert "b.keys = ControlInput::default();" in start[: start.index("DodgeInput::Close")]
