"""🕐 최근 한 시간 창: 커맨드·권한·메뉴 연결, 기록이 밖으로 나가지 않는지, timeline.js를 가짜 DOM에서 돌려 본다 (브라우저 없이)."""

import json
from pathlib import Path

from conftest import needs_node, run_node

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"

HARNESS = r"""
const fs = require("fs");
class Node {
  constructor(tag) { this.tag = tag; this.children = []; this.className = ""; this.style = {}; this.text = ""; this.title = ""; }
  set textContent(v) { this.text = String(v); this.children = []; }
  get textContent() { return this.text + this.children.map((c) => (typeof c === "string" ? c : c.textContent)).join(""); }
  append(...xs) { this.children.push(...xs); }
  set innerHTML(v) { throw new Error("innerHTML must not be used"); }
}
const byId = Object.fromEntries(["sum", "strip", "bites", "foods", "food-box", "log"].map((id) => [id, new Node("div")]));
// 내용 높이 = 순서대로 줄 수에 비례 (창 맞추기 확인용)
global.document = { createElement: (t) => new Node(t), getElementById: (id) => byId[id],
  body: { getBoundingClientRect: () => ({ height: 100 + 20 * kids(byId.log).length }) } };
global.setupFormKeys = () => {}; // common.js
// 타이머는 손으로 돌린다 (poll이 끝없이 다시 건다). 시계도 가짜
const timers = [];
let clockMs = 1_000_000;
const realSetTimeout = setTimeout;
global.setTimeout = (fn, ms) => { timers.push({ fn, at: clockMs + ms }); };
Date.now = () => clockMs;
let invokes = 0;
const fits = [];
let handler = null;
const VIEWS = JSON.parse(process.argv[3]);
let served = VIEWS[0];
global.window = {
  addEventListener() {},
  __TAURI__: {
    core: { invoke: async (cmd, args) => {
      if (cmd === "timeline_fit") { fits.push(args.height); return; }
      if (cmd !== "timeline_state") throw new Error(cmd);
      invokes++; return served; } },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (name, h) => { if (name === "timeline-state") handler = h; } }) },
  },
};
const kids = (n) => n.children.filter((c) => typeof c !== "string");
const settle = () => new Promise((r) => realSetTimeout(r, 5));
// 가짜 시계를 ms만큼 돌리며 그 사이 타이머를 부른다
async function advance(ms) {
  const end = clockMs + ms;
  for (;;) {
    timers.sort((a, b) => a.at - b.at);
    if (!timers.length || timers[0].at > end) break;
    const t = timers.shift();
    clockMs = t.at;
    t.fn();
    await settle();
  }
  clockMs = end;
}
const snap = () => ({
  sum: byId.sum.textContent,
  strip: kids(byId.strip).map((b) => ({ left: b.style.left, width: b.style.width, bg: b.style.background, title: b.title })),
  bites: kids(byId.bites).map((b) => ({ text: b.textContent, left: b.style.left })),
  foods: kids(byId.foods).map((f) => f.textContent),
  foodHidden: !!byId["food-box"].hidden,
  log: kids(byId.log).map((e) => ({ cls: e.className, cells: e.children.map((c) => c.textContent), dot: e.children[1].style.background || "" })),
  invokes,
  fits: [...fits],
});
require("vm").runInThisContext(fs.readFileSync(process.argv[2], "utf8"));
(async () => {
  await settle();
  const out = { first: snap() };
  if (VIEWS.length > 1) {
    // 1초 뒤 이벤트가 오면 바로 그린다 (직접 묻지 않는다)
    await advance(1000);
    handler({ payload: VIEWS[1] });
    await advance(1000);
    out.event = snap();
  }
  console.log(JSON.stringify(out));
  process.exit(0);
})();
"""

def render(tmp_path, *views):
    script = tmp_path / "harness.js"
    script.write_text(HARNESS, encoding="utf-8")
    return json.loads(run_node(script, UI / "timeline.js", json.dumps(list(views)), timeout=30).stdout)


def test_command_menu_and_capability_are_wired():
    main = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
    assert "timeline::timeline_state" in main
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    timeline_caps = [c for c in caps if "timeline" in c["windows"]]
    assert {p for c in timeline_caps for p in c["permissions"]} == {"allow-timeline-state", "allow-timeline-fit", "core:event:allow-listen", "core:event:allow-unlisten"}
    assert "timeline::timeline_fit" in main
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert '"🕐 최근 한 시간"' in tray and "TIMELINE_ID => send(app, Input::ShowTimeline)" in tray


def test_timeline_never_leaves_this_computer():
    # 기록은 메모리에만: 설정에 저장하지 않고 네트워크로 보내지 않는다
    for name in ("network.rs", "game_loop.rs"):
        rs = (ROOT / "src-tauri" / "src" / name).read_text(encoding="utf-8")
        assert "TimelineView" not in rs and "timeline.view" not in rs, name
    for path in (ROOT / "crates" / "screenpet-core" / "src" / "config.rs", *(ROOT / "crates" / "screenpet-net" / "src").glob("*.rs")):
        assert "timeline" not in path.read_text(encoding="utf-8").lower(), path.name


def test_every_meal_is_recorded():
    # 그릇·직접 주기 둘 다 먹으면 타임라인에 적는다
    rs = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    assert rs.count("care.feed(") == rs.count("timeline.ate(") == 2


def test_page_is_plain():
    src = (UI / "timeline.html").read_text(encoding="utf-8") + (UI / "timeline.js").read_text(encoding="utf-8")
    assert "innerHTML" not in src and "infinite" not in src and "setInterval" not in src
    # 자리별 합계 목록은 두지 않는다 (막대 + 순서대로로 충분)
    assert "어디에 얼마나" not in src
    # 군더더기 안내 글은 없다
    for chatter in ("메모리에만 있어요", "앱을 켠 지", "기록이 없어요"):
        assert chatter not in src, chatter


def full_view():
    return {
        "window_secs": 3600, "now": 1,
        "rows": [
            {"emoji": "🐾", "label": "<b>민수</b>네 놀러 감", "secs": 1500},
            {"emoji": "🎈", "label": "집에서 놀기", "secs": 720},
            {"emoji": "🏠", "label": "하우스", "secs": 40},
        ],
        "spans": [
            {"row": 2, "ago_start": 2260, "ago_end": 2220},
            {"row": 0, "ago_start": 2220, "ago_end": 720},
            {"row": 1, "ago_start": 720, "ago_end": 0},
        ],
        "foods": [{"emoji": "🍚", "name": "밥", "count": 1}, {"emoji": "🥛", "name": "우유", "count": 2}],
        "meals": [
            {"emoji": "🥛", "name": "우유", "ago": 3000},
            {"emoji": "🍚", "name": "밥", "ago": 600},
            {"emoji": "🥛", "name": "우유", "ago": 60},
        ],
    }


@needs_node
def test_renders_strip_foods_and_log(tmp_path):
    r = render(tmp_path, full_view())["first"]
    assert r["sum"] == "지금: 🎈 집에서 놀기 (12분째)"
    # 창 높이를 내용에 맞춘다
    assert r["fits"] == [100 + 20 * 6]
    # 막대: 왼쪽 = 한 시간 전. 같은 자리는 같은 색
    last = r["strip"][-1]
    assert last["left"] == "80%" and last["width"] == "20%" and "12분" in last["title"]
    assert r["strip"][1]["bg"] != last["bg"]
    # 먹은 때는 막대 아래 이모지
    assert [b["text"] for b in r["bites"]] == ["🥛", "🍚", "🥛"]
    assert r["foods"] == ["🍚 밥 1번", "🥛 우유 2번"] and not r["foodHidden"]
    # 순서대로: 최근 것부터 (자리는 시작한 때로), 자리와 먹은 때가 섞인다
    names = [e["cells"][2] for e in r["log"]]
    assert names == ["🥛 우유", "🍚 밥", "🎈 집에서 놀기", "🐾 <b>민수</b>네 놀러 감", "🏠 하우스", "🥛 우유"]
    assert r["log"][2]["cells"][0].endswith("– 지금") and len(r["log"][2]["cells"]) == 3
    # 자리 줄의 점 = 막대의 그 자리 색 (먹은 때는 점 없음), 친구 이름은 글자 그대로
    assert r["log"][2]["dot"] == last["bg"] and r["log"][3]["dot"] == r["strip"][1]["bg"]
    assert "meal" in r["log"][0]["cls"] and not r["log"][0]["dot"]


@needs_node
def test_empty_view_says_nothing_extra(tmp_path):
    view = {"window_secs": 3600, "now": None, "rows": [], "spans": [], "foods": [], "meals": []}
    r = render(tmp_path, view)["first"]
    assert r["sum"] == "" and r["strip"] == [] and r["log"] == [] and r["foodHidden"]


def ground_view(secs):
    return {"window_secs": 3600, "now": 0, "foods": [], "meals": [],
            "rows": [{"emoji": "🎈", "label": "집에서 놀기", "secs": secs}],
            "spans": [{"row": 0, "ago_start": secs, "ago_end": 0}]}


@needs_node
def test_each_event_redraws_at_once(tmp_path):
    r = render(tmp_path, ground_view(100), ground_view(120))
    assert r["first"]["sum"] == "지금: 🎈 집에서 놀기 (1분째)"
    # 1초 뒤 이벤트: 초까지 보여서 바뀐 게 보인다 (커맨드는 첫 로드 때만)
    # 이벤트가 오면 바로 다시 그린다 (커맨드는 첫 로드 때만), 초는 보이지 않는다
    assert r["event"]["invokes"] == 1 and r["event"]["sum"] == "지금: 🎈 집에서 놀기 (2분째)"
    # 내용 높이가 그대로면 창 크기를 다시 맞추지 않는다
    assert r["event"]["fits"] == [120]


@needs_node
def test_under_a_minute_says_just_now(tmp_path):
    r = render(tmp_path, ground_view(42))["first"]
    assert r["sum"] == "지금: 🎈 집에서 놀기 (방금)" and "1분 미만" in r["strip"][0]["title"]


def test_fitting_the_window_does_not_take_focus():
    """Windows에서 tao의 set_size·set_position은 보이는 창을 다시 활성화한다: 다른 앱에서 치던 글이 끊긴다."""
    src = (Path(__file__).resolve().parent.parent / "src-tauri" / "src" / "timeline.rs").read_text(encoding="utf-8")
    fit = src[src.index("pub fn timeline_fit") :]
    assert "resize_quietly" in fit and "move_quietly" in fit
    assert ".set_position(" not in fit
