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
  // 지금 그리는 그림 캔버스 (fillStyle을 그 그림의 색으로 적어 둔다)
  let drawing = null;
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        return (...args) => {
          counts[name] = (counts[name] || 0) + 1;
          if (name === "drawImage") draws.push({ x: args[1], y: args[2], box: args[0].width });
        };
      },
      set(_, name, value) {
        if (name === "fillStyle" && drawing) drawing.fill = value;
        return true;
      },
    },
  );
  const canvas = () => ({ width: 0, height: 0, getContext: () => ctx });
  const sprite = () => (drawing = canvas());
  const timers = new Map();
  let nextId = 1;
  const delays = [];
  let rafs = [];
  let now = 0;
  let listener = null;
  const sandbox = {
    document: { getElementById: canvas, createElement: sprite, documentElement: { style: {} }, body: {} },
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
  const wet = kind === "rain" || kind === "shower";
  const scene = {
    paused: false,
    kind,
    pile_kind: wet ? null : kind,
    tops: [[0, W, LINE_Y], [0, W, H]],
    piles: wet ? [] : [pile],
  };
  listener({ payload: scene });
  const pileFills = (counts.fill || 0) + (counts.drawImage || 0);
  const pileImages = counts.drawImage || 0;

  // 한 알갱이가 떨어지는 동안의 그림들 (흔들림 위상을 돌려 본다)
  const tumble = vm.runInContext("Array.from({ length: 40 }, (_, k) => { phase[0] = k * 0.5; rest[0] = 0; return spriteFor(0); })", sandbox);
  const tumbleSprites = new Set(tumble).size;
  const tumbleColours = new Set(tumble.map((s) => s.fill)).size;
  const colours = [...new Set(vm.runInContext("sprites", sandbox).map((s) => s.fill))];
  const hasSplash = vm.runInContext("splash !== null", sandbox);

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
  const paintsBefore = (counts.fill || 0) + (counts.drawImage || 0);
  listener({ payload: scene });
  const repaintedPiles = (counts.fill || 0) + (counts.drawImage || 0) - paintsBefore;

  // 🧹 흩날림: 몰려와도 상한을 넘지 않고, 곧 모두 사라지며, 자리는 남기지 않는다
  const addFlurry = (count) => vm.runInContext(`addFlurry({ kind: ${JSON.stringify(kind)}, x: 100, y: 100, vx: 5000, vy: -200, count: ${count} })`, sandbox);
  addFlurry(500);
  addFlurry(500);
  const flurryPeak = vm.runInContext("flurry.length", sandbox);
  for (let f = 0; f < 120; f++) step();
  const flurryLeft = vm.runInContext("flurry.length", sandbox);

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

  // 쌓였던 더미가 비 오는 날 녹는 동안에도 제 그림 그대로
  const meltBefore = counts.drawImage || 0;
  listener({ payload: { ...scene, kind: "rain", piles: wet ? [] : [{ ...pile, h: [5, 15, 25, 15, 5] }] } });
  const meltingPileImages = (counts.drawImage || 0) - meltBefore;

  return {
    kind,
    flurryPeak,
    flurryLeft,
    maxDraws,
    maxClears,
    deepest,
    lineY: LINE_Y,
    pileFills,
    pileImages,
    tumbleSprites,
    tumbleColours,
    colours,
    hasSplash,
    repaintedPiles,
    meltingPileImages,
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
