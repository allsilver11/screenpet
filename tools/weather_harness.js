// ui/weather.js를 가짜 캔버스·타이머 위에서 돌려 호출 수를 세는 도구 (브라우저 없이, tools/test_weather_ui.py가 쓴다).
// node tools/weather_harness.js <kind> → 결과 JSON. 실제 그림·GPU 비용은 재지 못한다 — 프레임당 호출 수와 JS 시간만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const W = 1920;
const H = 1040;
const LINE_Y = 500;

function run(kind, style, size) {
  const counts = {};
  let draws = [];
  let lineWidth = 1;
  // 지금 그리는 그림 캔버스 (fillStyle을 그 그림의 색으로 적어 둔다)
  let drawing = null;
  // 🌀 깔때기를 그리는 동안의 그림들 (drawFunnel을 감싸 모음): 찍은 자리·크기·투명도, 그 밖의 그리기 호출 수
  let inFunnel = false;
  let funnelDraws = [];
  let funnelOther = 0;
  // 지울 칸 안에 드는지 셀 수 있는 그리기만 (나머지는 funnelOther로 셈 — 0이어야 칸 약속을 증명함)
  const FUNNEL_SAFE = new Set(["drawImage", "fillRect", "save", "restore", "clearRect"]);
  // 캔버스마다 제 ctx (그림 캔버스는 쓴 호출 이름을 ops에 모음), 호출 수는 함께 셈
  const makeCtx = (c) => {
    let alpha = 1;
    const stack = [];
    return new Proxy(
      {},
      {
        get(_, name) {
          if (name === "globalAlpha") return alpha;
          return (...args) => {
            counts[name] = (counts[name] || 0) + 1;
            if (c.ops) c.ops.add(name);
            if (name === "save") stack.push([alpha, lineWidth]);
            if (name === "restore" && stack.length) [alpha, lineWidth] = stack.pop();
            if (name === "createRadialGradient" || name === "createLinearGradient") return { addColorStop() {} };
            if (inFunnel && c.screen) {
              if (name === "drawImage") {
                const w = args.length >= 5 ? args[3] : args[0].width;
                const h = args.length >= 5 ? args[4] : args[0].height;
                funnelDraws.push({ x: args[1], y: args[2], w, h, alpha, sprite: args[0], scaled: args.length >= 5 });
              } else if (name === "fillRect") funnelDraws.push({ x: args[0], y: args[1], w: args[2], h: args[3], alpha, sprite: null, scaled: false });
              else if (!FUNNEL_SAFE.has(name)) funnelOther++;
              return;
            }
            if (name === "drawImage") draws.push({ x: args[1], y: args[2], box: args[0].width });
          };
        },
        set(_, name, value) {
          if (c.ops) c.ops.add(`set:${name}`);
          if (name === "fillStyle" && drawing) drawing.fill = value;
          if (name === "lineWidth") lineWidth = value;
          if (name === "globalAlpha") alpha = value;
          return true;
        },
      },
    );
  };
  const canvas = (screen) => {
    const c = { width: 0, height: 0, screen: !!screen, ...(screen ? { style: {} } : {}) };
    const ctx = makeCtx(c);
    c.getContext = () => ctx;
    return c;
  };
  const screenCanvas = () => canvas(true);
  const sprite = () => {
    drawing = canvas(false);
    drawing.ops = new Set();
    return drawing;
  };
  const timers = new Map();
  let nextId = 1;
  const delays = [];
  let rafs = [];
  const rafIds = new Map();
  let nextRaf = 1;
  let resized = null;
  let now = 0;
  let listener = null;
  const sandbox = {
    document: { getElementById: screenCanvas, createElement: sprite, documentElement: { style: {} }, body: {} },
    window: { ART_STYLE: style, innerWidth: W, innerHeight: H, addEventListener: (ev, f) => ev === "resize" && (resized = f), __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (_, f) => (listener = f) }) } } },
    performance: { now: () => now },
    setTimeout(f, ms) {
      delays.push(ms);
      timers.set(nextId, f);
      return nextId++;
    },
    clearTimeout: (id) => timers.delete(id),
    requestAnimationFrame: (f) => {
      const id = nextRaf++;
      const once = (t) => rafIds.delete(id) && f(t);
      rafIds.set(id, once);
      rafs.push(once);
      return id;
    },
    cancelAnimationFrame: (id) => rafIds.delete(id),
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
  // 깔때기 그리기를 감싸 그 동안의 그림만 따로 모음 (알갱이 수·깊이 셈에 섞이지 않게)
  const drawFunnel = sandbox.drawFunnel;
  sandbox.drawFunnel = (t) => {
    inFunnel = true;
    try {
      drawFunnel(t);
    } finally {
      inFunnel = false;
    }
  };

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
  const stormy = kind === "typhoon";
  const wet = kind === "rain" || kind === "shower" || stormy;
  const scene = {
    paused: false,
    kind,
    pile_kind: wet ? null : kind,
    tops: [[0, W, LINE_Y], [0, W, H]],
    piles: wet ? [] : [pile],
    // 🌀 태풍 장면엔 바람과 회오리가 실려 옴
    // 넷째 값 = 깔때기 크기 (🌀 허리케인급은 2 — 없으면 예전 장면처럼 보통 크기)
    ...(stormy ? { wind: -1.3, tornado: size ? [600, H, 0, size] : [600, H, 0] } : {}),
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
  // ❄️ 떨어지는 알갱이 그림 크기와 수 (함박눈은 크고 많음)
  const spriteBox = Math.max(...vm.runInContext("sprites", sandbox).map((s) => s.width));
  const particleCount = vm.runInContext("n", sandbox);
  // 쓸려 날아오는 알갱이 그림 크기 (함박눈은 함박눈 그대로)
  const flurryBox = Math.max(...vm.runInContext(`flurrySprites(${JSON.stringify(kind)})`, sandbox).map((s) => s.width));

  let deepest = -Infinity;
  let maxDraws = 0;
  let maxClears = 0;
  const started = process.hrtime.bigint();
  const FRAMES = 1500; // 약 1분
  // 🌀 매 프레임 깔때기가 제 지울 칸을 넘는지, 축에서 가장 먼 끝은 어디인지 (회오리 x = 600, vx 0)
  const axis = 600;
  const outsideBox = (ds, box) => (!box ? 0 : ds.filter((d) => d.x < box[0] || d.y < box[1] || d.x + d.w > box[0] + box[2] || d.y + d.h > box[1] + box[3]).length);
  const reachOf = (ds) => Math.max(0, ...ds.map((d) => Math.max(axis - d.x, d.x + d.w - axis)));
  let everOutside = 0;
  let maxReach = 0;
  for (let f = 0; f < FRAMES; f++) {
    draws = [];
    funnelDraws = [];
    funnelOther = 0;
    const clearsBefore = counts.clearRect || 0;
    step();
    everOutside += funnelOther + outsideBox(funnelDraws, vm.runInContext("funnelBox", sandbox));
    maxReach = Math.max(maxReach, reachOf(funnelDraws));
    maxDraws = Math.max(maxDraws, draws.length);
    maxClears = Math.max(maxClears, (counts.clearRect || 0) - clearsBefore);
    for (const d of draws) deepest = Math.max(deepest, d.y + d.box - 2);
  }
  const msPerFrame = Number(process.hrtime.bigint() - started) / 1e6 / FRAMES;
  const particles = draws.map((d) => [d.x, d.y]);

  // 🌀 깔때기: 한 프레임의 구름 뭉치들, 그 다음 프레임 (돌며 움직였는지), 지울 칸 밖으로 나간 그림
  funnelDraws = [];
  funnelOther = 0;
  step();
  const puffs = funnelDraws;
  const otherOps = funnelOther;
  const box = vm.runInContext("funnelBox", sandbox);
  funnelDraws = [];
  step();
  const nextPuffs = funnelDraws;
  const funnelOutside = everOutside + otherOps + outsideBox(puffs, box);
  const funnelReach = Math.max(maxReach, reachOf(puffs));
  // 위 1/3과 아래 1/3의 폭 (위가 넓은 깔때기)
  const ys = puffs.map((d) => d.y + d.h / 2);
  const lo = Math.min(...ys), hi = Math.max(...ys);
  const spread = (ds) => (ds.length ? Math.max(...ds.map((d) => d.x + d.w)) - Math.min(...ds.map((d) => d.x)) : 0);
  const funnelTopWidth = spread(puffs.filter((d) => d.y + d.h / 2 < lo + (hi - lo) / 3));
  const funnelHeight = puffs.length ? H - Math.min(...puffs.map((d) => d.y)) : 0;
  const funnelBottomWidth = spread(puffs.filter((d) => d.y + d.h / 2 > hi - (hi - lo) / 3));
  // 돌기: 다음 프레임에 자리가 바뀐 뭉치 몫 (회오리는 제자리 — vx 0)
  const key = (d) => `${Math.round(d.x)},${Math.round(d.y)}`;
  const before = new Set(puffs.map(key));
  const funnelMoved = nextPuffs.filter((d) => !before.has(key(d))).length / Math.max(1, nextPuffs.length);
  const alphas = puffs.map((d) => d.alpha);
  const funnelSprites = [...new Set(puffs.map((d) => d.sprite).filter(Boolean))];
  // 뭉치 그림을 그린 호출 이름들 (픽셀은 칸 채우기만)
  const funnelSpriteOps = [...new Set(funnelSprites.flatMap((c) => [...(c.ops || [])]))].sort();
  const funnelOnGrid = puffs.every((d) => !d.scaled && d.x % 3 === 0 && d.y % 3 === 0);
  const funnelRings = puffs.length;
  const debrisColours = [...new Set(vm.runInContext("debris", sandbox).map((s) => s.fill))];
  const flow = () => {
    const before = vm.runInContext("[Array.from(px), Array.from(rest)]", sandbox);
    step();
    const after = vm.runInContext("[Array.from(px), Array.from(rest)]", sandbox);
    const moves = before[0].map((x, i) => after[0][i] - x).filter((d, i) => before[1][i] === 0 && after[1][i] === 0 && Math.abs(d) < W / 2);
    return moves.reduce((a, b) => a + b, 0) / Math.max(1, moves.length);
  };
  const flowNow = flow();
  listener({ payload: { ...scene, wind: 1.3 } });
  for (let i = 0; i < 40; i++) step();
  const flowFlipped = flow();
  listener({ payload: { ...scene, tornado: undefined } });
  funnelDraws = [];
  step();
  const funnelAfterGone = funnelDraws.length;
  listener({ payload: scene });

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

  // 창 크기가 바뀌면 캔버스가 지워진다 — 같은 장면이 다시 오지 않아도 더미를 다시 그린다 (멈춰 있으면 안 그린다)
  const clipsBefore = counts.clip || 0;
  sandbox.window.innerWidth = W - 100;
  resized();
  const pilesAfterResize = (counts.clip || 0) - clipsBefore;
  sandbox.window.innerWidth = W;
  resized();
  listener({ payload: { ...scene, paused: true } });
  const clipsPaused = counts.clip || 0;
  resized();
  const pilesAfterPausedResize = (counts.clip || 0) - clipsPaused;

  // 타이머는 지났는데 rAF가 아직 안 온 사이(가려진 창은 rAF가 쉰다)에 멈췄다 풀려도 그리기 고리는 하나다
  listener({ payload: scene });
  const dueNow = [...timers.values()];
  timers.clear();
  dueNow.forEach((f) => f());
  listener({ payload: { ...scene, paused: true } });
  listener({ payload: scene });
  for (let i = 0; i < 5; i++) step();
  const loopsAfterPauseRace = timers.size;

  // 쌓였던 더미가 비 오는 날 녹는 동안에도 제 그림 그대로
  const meltBefore = counts.drawImage || 0;
  listener({ payload: { ...scene, kind: "rain", piles: wet ? [] : [{ ...pile, h: [5, 15, 25, 15, 5] }] } });
  const meltingPileImages = (counts.drawImage || 0) - meltBefore;

  return {
    kind,
    funnelRings,
    funnelOutside,
    funnelReach,
    funnelTopWidth,
    funnelBottomWidth,
    funnelHeight,
    funnelMoved,
    funnelAlphas: puffs.length ? [Math.min(...alphas), Math.max(...alphas)] : [],
    funnelSpriteCount: funnelSprites.length,
    funnelSpriteOps,
    funnelOnGrid,
    funnelAfterGone,
    debrisColours,
    flowNow,
    flowFlipped,
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
    spriteBox,
    particleCount,
    flurryBox,
    repaintedPiles,
    meltingPileImages,
    pendingAfterPause,
    timersAfterRaf,
    drawsWhilePaused,
    drawsAfterResume,
    pilesAfterResize,
    pilesAfterPausedResize,
    loopsAfterPauseRace,
    minDelay: Math.min(...delays),
    msPerFrame,
    particles,
  };
}

console.log(JSON.stringify(run(process.argv[2] || "snow", process.argv[3] || "", Number(process.argv[4]) || 0)));
