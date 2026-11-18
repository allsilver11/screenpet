"""말풍선 우선순위 — 백엔드가 정한 글 말풍선("<이름>: busy~" 등)이 떠 있는 동안 짧은 상황 대사(ouch! 등)가 덮지 않음."""
import json
from pathlib import Path

from conftest import needs_node, run_node

UI = Path(__file__).resolve().parent.parent / "ui"

HARNESS = r"""
const fs = require("fs");
const vm = require("vm");
const listeners = {};
let now = 0;
const el = () => ({ hidden: true, textContent: "", title: "", children: [], replaceChildren(...c) { this.children = c; }, addEventListener() {}, querySelectorAll: () => [] });
const timers = [];
const ctx = {
  Date: { now: () => now },
  setTimeout: (f, ms) => { timers.push({ f, at: now + ms }); return timers.length; },
  clearTimeout: () => {},
  document: { createElement: () => ({ textContent: "" }) },
  console,
};
ctx.window = { __TAURI__: { core: { invoke: async () => {} }, webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (ev, h) => { listeners[ev] = h; } }) } }, devicePixelRatio: 1 };
vm.createContext(ctx);
vm.runInContext(fs.readFileSync(process.argv[2], "utf8") + "\nthis.setupSpeech = setupSpeech;", ctx);
const bubbleEl = el();
ctx.setupSpeech({ bubbleEl, pickerEl: el(), favBtn: el(), getManifest: () => ({ speech: { oof: ["ouch!"] } }) });
const shown = () => (bubbleEl.hidden ? null : bubbleEl.children[0]?.textContent);
const out = {};
listeners["pet-say-text"]({ payload: "Mina: busy~" });
now = 300;
listeners["pet-say"]({ payload: "oof" });
out.during = shown();
now = 2600;
listeners["pet-say"]({ payload: "oof" });
out.after = shown();
console.log(JSON.stringify(out));
"""


@needs_node
def test_a_message_bubble_is_not_covered_by_a_short_reaction(tmp_path):
    script = tmp_path / "h.js"
    script.write_text(HARNESS, encoding="utf-8")
    out = json.loads(run_node(script, UI / "common.js").stdout)
    assert out["during"] == "Mina: busy~", "떨어지며 ouch!가 busy 말풍선을 덮으면 안 됨"
    assert out["after"] == "ouch!", "글 말풍선이 끝난 뒤엔 다시 대사가 나옴"
