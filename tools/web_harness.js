// ui/web.js(🕷️ 거미인간 줄·거미집)를 가짜 캔버스·rAF·타이머 위에서 돌려 보는 도구 (브라우저 없이, tools/test_web_ui.py가 쓴다).
// node tools/web_harness.js → 결과 JSON. 실제 그림은 보지 못한다 — 프레임 수, 거미집 수, 선을 몇 번 긋는지만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "web.js"), "utf8");

/** 가짜 창 하나에 web.js를 올림 — 이벤트 이름별 듣는 함수를 모아 둠 */
function load() {
  const counts = {};
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        return (...args) => {
          counts[name] = (counts[name] || 0) + 1;
        };
      },
      set() {
        return true;
      },
    },
  );
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  let rafs = [];
  let timers = [];
  let now = 0;
  const listeners = {};
  const sandbox = {
    document: { getElementById: () => canvas, documentElement: { style: {} } },
    window: {
      innerWidth: 1920,
      innerHeight: 1040,
      addEventListener() {},
      __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (name, f) => (listeners[name] = f) }) } },
    },
    performance: { now: () => now },
    requestAnimationFrame: (f) => rafs.push(f),
    setTimeout: (f, ms) => timers.push({ f, at: now + ms }),
    clearTimeout() {},
    location: { search: "" },
    URLSearchParams,
    Math,
    Number,
    Array,
    Object,
    console,
  };
  vm.runInNewContext(SRC, sandbox);
  // 처음 맞춘 캔버스 프레임은 비우고 시작
  rafs.splice(0).forEach((f) => f(now));
  return {
    listeners: () => Object.keys(listeners).sort(),
    send: (name, payload) => listeners[name]({ payload }),
    /** 프레임 하나 (그은 선 수를 돌려줌) */
    frame() {
      now += 16;
      const before = counts.stroke || 0;
      const r = rafs;
      rafs = [];
      r.forEach((f) => f(now));
      return (counts.stroke || 0) - before;
    },
    /** 걸린 프레임이 없어질 때까지 (최대 n) — 돈 프레임 수 */
    settle(n = 5000) {
      let k = 0;
      while (rafs.length && k < n) {
        this.frame();
        k++;
      }
      return k;
    },
    /** 시간을 건너뜀: 그 사이 타이머를 부름 */
    skip(ms) {
      now += ms;
      const due = timers.filter((t) => t.at <= now);
      timers = timers.filter((t) => t.at > now);
      due.forEach((t) => t.f());
    },
    /** 프레임 하나 (그은 조각 수 — moveTo 수) */
    frameMoves() {
      const before = counts.moveTo || 0;
      this.frame();
      return (counts.moveTo || 0) - before;
    },
    rafs: () => rafs.length,
    timers: () => timers.length,
    alive: () => vm.runInContext("webs.length", sandbox),
    /** 거미집마다 [살 수, 고리 수, 모서리에서 가장 먼 점 / 반지름, 살 각도들] */
    shapes: () =>
      vm.runInContext(
        "JSON.stringify(webs.map((w) => { const p = fanPaths(w, w.size); const far = Math.max(...p.flat().map(([x, y]) => Math.hypot(x - w.x, y - w.y))); const s = webShape(w); return [s.angs.length, s.rings.length, far / w.size, s.angs]; }))",
        sandbox,
      ),
    /** 거미집마다 [짜는 시간, 줄 횟수] */
    weaves: () => vm.runInContext("JSON.stringify(webs.map((w) => [w.build, w.shots]))", sandbox),
    /** 거미집 id의 지금 자리와 숨김 */
    at: (id) => vm.runInContext(`(() => { const w = webs.find((w) => w.id === ${id}); return w && [w.x, w.y, !!w.hidden]; })()`, sandbox),
  };
}

const web = (id, extra = {}) => ({ id, x: 0, y: 0, corner: "top_left", size: 80, pixel: false, age: 0, life: 60, ...extra });
const out = {};

let h = load();
out.listeners = h.listeners();
out.idle = h.rafs() + h.timers();

// 많이 와도 12개까지, 모르는 모서리·이상한 값은 버림
h.send("trail-webs", Array.from({ length: 20 }, (_, i) => web(i + 1)));
out.capped = h.alive();
h = load();
h.send("trail-webs", [web(1, { corner: "middle" }), web(2, { size: -3 }), web(3, { x: NaN }), web(4, { age: 999 }), web(5)]);
out.filtered = h.alive();

// 피어나는 동안만 프레임, 그대로 있는 동안은 타이머 하나, 남는 시간이 지나면 흐려져 사라짐
h = load();
h.send("trail-webs", [web(1)]);
out.grow_frames = h.settle();
out.hold_rafs = h.rafs();
out.hold_timers = h.timers();
h.skip(61000);
out.fade_frames = h.settle();
out.alive_end = h.alive();

// 줄: 있는 동안 매 프레임 그리고, 빈 목록이 오면 멈춤
h = load();
h.send("trail-lines", [{ x0: 10, y0: 900, x1: 300, y1: 0 }]);
let strokes = 0;
for (let i = 0; i < 10; i++) strokes += h.frame();
out.line_strokes = strokes;
out.line_rafs = h.rafs();
h.send("trail-lines", []);
h.settle();
out.line_rafs_after = h.rafs();
h.send("trail-lines", [{ x0: 1, y0: 2, x1: "x", y1: 4 }]);
h.frame();
out.bad_line_rafs = h.rafs();

// 창이 움직인 거미집 하나만 지움 · 🧹 다 치우기는 전부
h = load();
h.send("trail-webs", [web(1), web(2, { corner: "bottom_right", x: 1920, y: 1040 })]);
h.settle();
h.send("trail-web-drop", [1]);
h.settle();
out.after_drop = h.alive();
h.send("trail-web-clear", null);
h.settle();
out.after_clear = h.alive();

// 창을 따라 옮겨 감 · 가려지면 숨고 (그리지 않음) 다시 보임 · 이상한 값은 무시
h = load();
h.send("trail-webs", [web(1, { x: 300, y: 300 }), web(2)]);
h.settle();
h.send("trail-web-move", [{ id: 1, x: 500, y: 120, hidden: false }]);
out.moved = h.at(1);
out.move_rafs = h.rafs();
h.settle();
h.send("trail-web-move", [{ id: 1, x: 500, y: 120, hidden: true }, { id: 2, x: "x", y: 0, hidden: false }]);
out.hidden = h.at(1);
out.untouched = h.at(2);
out.hidden_strokes = h.frame();
h.send("trail-web-move", [{ id: 1, x: 500, y: 120, hidden: false }]);
out.shown_strokes = h.frame();

// 짜는 거미집: 쏠 때마다 조각이 늘고 (살 → 고리), 다 짜면 9조각 (그늘 + 실 = 두 번씩)
h = load();
h.send("trail-webs", [web(1, { shots: 5, build: 1.28 })]);
const woven = [];
for (let i = 0; i < 120; i++) woven.push(h.frameMoves());
out.woven = woven;
h = load();
h.send("trail-webs", [web(1, { shots: 99, build: 1.28 }), web(2, { shots: 5, build: -1 })]);
out.bad_weave = JSON.parse(h.weaves());

// 모양: 거미집마다 다르고, 같은 거미집은 늘 같고, 반지름 밖으로 나가지 않음
h = load();
h.send("trail-webs", [1, 2, 3, 4, 5, 6, 7, 8].map((id) => web(id, { size: 100 })));
out.shapes = JSON.parse(h.shapes());
out.shapes_again = JSON.parse(h.shapes());

console.log(JSON.stringify(out));
