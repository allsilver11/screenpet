// ui/doodle.js를 가짜 캔버스·rAF·타이머 위에서 돌려 보는 도구 (브라우저 없이, tools/test_doodle_ui.py가 쓴다).
// node tools/doodle_harness.js → 결과 JSON. 실제 그림은 보지 못한다 — 호출 수, 낙서 수, 글자를 어떻게 넣는지만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "doodle.js"), "utf8");

function load() {
  const counts = { main: {}, sprite: {} };
  const texts = [];
  const ctxFor = (kind) =>
    new Proxy(
      {},
      {
        get(_, name) {
          if (name === "measureText") return (t) => ({ width: Array.from(t).length * 10 });
          if (name === "getImageData") return (x, y, w, h) => ({ data: new Uint8ClampedArray(Math.max(0, w * h * 4)), width: w, height: h });
          return (...args) => {
            counts[kind][name] = (counts[kind][name] || 0) + 1;
            if (name === "fillText") texts.push(args[0]);
          };
        },
        set() {
          return true;
        },
      },
    );
  const canvas = (kind) => ({ width: 0, height: 0, getContext: () => ctxFor(kind) });
  let rafs = [];
  let timers = [];
  let now = 0;
  let listener = null;
  let html = 0;
  const sandbox = {
    document: {
      getElementById: () => canvas("main"),
      createElement: () => canvas("sprite"),
      documentElement: { style: {} },
      body: { set innerHTML(_) { html++; } },
    },
    window: {
      innerWidth: 1920,
      innerHeight: 1040,
      addEventListener() {},
      __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (name, f) => name === "trail-doodle" && (listener = f) }) } },
    },
    performance: { now: () => now },
    requestAnimationFrame: (f) => rafs.push(f),
    setTimeout: (f, ms) => timers.push({ f, at: now + ms }),
    clearTimeout() {},
    location: { search: "" },
    URLSearchParams,
    Math,
    Map,
    Array,
    Uint8ClampedArray,
    console,
  };
  vm.runInNewContext(SRC, sandbox);
  const h = {
    counts,
    texts,
    html: () => html,
    send: (list) => listener({ payload: list }),
    frame() {
      now += 16;
      const before = { ...counts.main };
      const r = rafs;
      rafs = [];
      r.forEach((f) => f(now));
      const delta = (k) => (counts.main[k] || 0) - (before[k] || 0);
      return { draws: delta("drawImage"), clears: delta("clearRect") };
    },
    // 시간을 건너뛴다: 그 사이 타이머를 부르고 (rAF는 걸릴 뿐)
    skip(ms) {
      now += ms;
      const due = timers.filter((t) => t.at <= now);
      timers = timers.filter((t) => t.at > now);
      due.forEach((t) => t.f());
    },
    rafs: () => rafs.length,
    timers: () => timers.length,
    alive: () => vm.runInContext("doodles.length", sandbox),
  };
  return h;
}

const doodle = (kind, extra = {}) => ({ x: 400, y: 800, kind, name: "흐물이", facing: 1, size: 60, pixel: false, age: 0, draw: 2, ...extra });
const out = {};

// 낙서가 오기 전엔 아무것도 돌지 않는다
let h = load();
out.idle = h.rafs() + h.timers();

// 많이 와도 6개까지, 모르는 종류·수명이 지난 것은 버린다
h.send(["heart", "smile", "star", "name", "heart", "smile", "star", "name"].map((k) => doodle(k)));
out.capped = h.alive();
h = load();
h.send([doodle("rocket"), doodle("heart", { age: 999 }), doodle("star", { size: -1 }), doodle("smile", { age: 1 })]);
out.filtered = h.alive();

// 그리는 동안·흐려지는 동안만 프레임을 걸고, 가만히 있는 20초 동안은 타이머 하나만 걸어 둔다
h = load();
h.send([doodle("heart")]);
let frames = 0;
while (h.rafs() && frames < 1000) {
  h.frame();
  frames++;
}
out.draw_frames = frames;
for (const [key, draw] of [["draw_frames_1s", 1], ["draw_frames_junk", 999]]) {
  const g = load();
  g.send([doodle("star", { draw })]);
  let n = 0;
  while (g.rafs() && n < 1000) {
    g.frame();
    n++;
  }
  out[key] = n;
}
out.hold_timers = h.timers();
out.hold_rafs = h.rafs();
h.skip(21000);
let last = null;
let fade = 0;
while (h.rafs() && fade < 2000) {
  last = h.frame();
  fade++;
}
out.fade_frames = fade;
out.alive_end = h.alive();
out.cleared_last = !!last && last.draws === 0 && last.clears > 0;
out.end_rafs = h.rafs();

// 이름: 캔버스 fillText로만 (HTML로 넣지 않는다), 12글자까지
for (const pixel of [false, true]) {
  h = load();
  h.send([doodle("name", { name: "<img src=x onerror=alert(1)>가나다라마바사아자차카타파하", pixel })]);
  h.frame();
  out[`name_${pixel ? "pixel" : "round"}`] = h.texts.slice();
  out[`html_${pixel ? "pixel" : "round"}`] = h.html();
}

// 두 스타일: 동글은 작은 분필 가루, 픽셀은 칸 (둘 다 fillRect), 픽셀은 부드럽게 늘리지 않는다
for (const pixel of [false, true]) {
  h = load();
  h.send(["heart", "smile", "star"].map((k) => doodle(k, { pixel, age: 2 })));
  h.frame();
  out[`${pixel ? "pixel" : "round"}_specks`] = h.counts.sprite.fillRect || 0;
  out[`${pixel ? "pixel" : "round"}_draws`] = h.counts.main.drawImage || 0;
}

process.stdout.write(JSON.stringify(out));
