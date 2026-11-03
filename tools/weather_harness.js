// ui/weather.js를 가짜 캔버스·타이머 위에서 돌려 호출 수를 세는 도구 (브라우저 없이, tools/test_weather_ui.py가 쓴다).
// node tools/weather_harness.js <kind> → 결과 JSON. 실제 그림·GPU 비용은 재지 못한다 — 프레임당 호출 수와 JS 시간만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const W = 1920;
const H = 1040;
const LINE_Y = 500;

function run(kind) {
  const counts = {};
  let draws = [];
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        return (...args) => {
          counts[name] = (counts[name] || 0) + 1;
          if (name === "drawImage") draws.push({ x: args[1], y: args[2], box: args[0].width });
        };
      },
      set: () => true,
    },
  );
  const canvas = () => ({ width: 0, height: 0, getContext: () => ctx });
  const timers = new Map();
  let nextId = 1;
  const delays = [];
  let rafs = [];
  let now = 0;
  let listener = null;
  const sandbox = {
    document: { getElementById: canvas, createElement: canvas, documentElement: { style: {} }, body: {} },
    window: { innerWidth: W, innerHeight: H, addEventListener() {}, __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (_, f) => (listener = f) }) } } },
    performance: { now: () => now },
    setTimeout(f, ms) {
      delays.push(ms);
      timers.set(nextId, f);
      return nextId++;
    },
    clearTimeout: (id) => timers.delete(id),
    requestAnimationFrame: (f) => rafs.push(f),
    location: { search: "" },
    console,
    Math,
    Float32Array,
    Uint8Array,
    Array,
    JSON,
    URLSearchParams,
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, "..", "ui", "weather.js"), "utf8"), sandbox);

  const step = () => {
    now += 42;
    const due = [...timers.values()];
    timers.clear();
    due.forEach((f) => f());
    const r = rafs;
    rafs = [];
    r.forEach((f) => f(now));
  };
  const pile = { x: 100, y: LINE_Y, cell: 20, h: [10, 30, 50, 30, 10], clips: [[100, 200]] };
  const scene = {
    paused: false,
    kind,
    pile_kind: kind === "rain" ? null : kind,
    tops: [[0, W, LINE_Y], [0, W, H]],
    piles: kind === "rain" ? [] : [pile],
  };
  listener({ payload: scene });
  const pileFills = (counts.fill || 0) + (counts.drawImage || 0);

  let deepest = -Infinity;
  let maxDraws = 0;
  let maxClears = 0;
  const started = process.hrtime.bigint();
  const FRAMES = 1500; // 약 1분
  for (let f = 0; f < FRAMES; f++) {
    draws = [];
    const clearsBefore = counts.clearRect || 0;
    step();
    maxDraws = Math.max(maxDraws, draws.length);
    maxClears = Math.max(maxClears, (counts.clearRect || 0) - clearsBefore);
    for (const d of draws) deepest = Math.max(deepest, d.y + d.box - 2);
  }
  const msPerFrame = Number(process.hrtime.bigint() - started) / 1e6 / FRAMES;
  const particles = draws.map((d) => [d.x, d.y]);

  // 같은 장면을 다시 받아도 더미는 다시 그리지 않는다
  const fillsBefore = counts.fill || 0;
  listener({ payload: scene });
  const repaintedPiles = (counts.fill || 0) - fillsBefore;

  // 멈추면 타이머도 그림도 없다
  listener({ payload: { ...scene, paused: true } });
  const pendingAfterPause = timers.size;
  rafs.splice(0).forEach((f) => f(now));
  const timersAfterRaf = timers.size;
  draws = [];
  for (let i = 0; i < 20; i++) step();
  const drawsWhilePaused = draws.length;

  // 다시 풀리면 이어서 그린다
  listener({ payload: scene });
  draws = [];
  for (let i = 0; i < 10; i++) step();
  const drawsAfterResume = draws.length;

  return {
    kind,
    maxDraws,
    maxClears,
    deepest,
    lineY: LINE_Y,
    pileFills,
    repaintedPiles,
    pendingAfterPause,
    timersAfterRaf,
    drawsWhilePaused,
    drawsAfterResume,
    minDelay: Math.min(...delays),
    msPerFrame,
    particles,
  };
}

console.log(JSON.stringify(run(process.argv[2] || "snow")));
