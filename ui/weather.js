// 계절 날씨 그리기. 앱(src-tauri/src/weather.rs)이 `weather-scene`으로 내려앉을 곳(tops)과 더미(piles)를 보낸다 (논리 px, 이 창 왼쪽 위가 0,0).
// CPU를 아끼는 규칙: 24 fps 이하, 알갱이 80개 이하, 미리 그린 그림을 정수 자리에 찍기, 알갱이 자리만 지우기, 더미는 바뀔 때만.
// 멈춰 있으면(paused) 타이머도 돌지 않는다. 브라우저에서 weather.html?preview=snow (light_snow·rain·shower·petals·leaves)로 앱 없이 볼 수 있다.

const FPS = 24;
const FRAME_MS = 1000 / FPS;
const MAX_PARTICLES = 80;
const MIN_PARTICLES = 30;
// 한 프레임에 다시 재 보는 알갱이 수 (옆으로 흘러도 내려앉을 곳이 맞도록)
const RELAND_PER_FRAME = 6;

// count: 폭 1920 논리 px에서의 개수, fall·wind·sway: px/s, rest: 내려앉아 머무는 초, box: 그림 칸, frames: 떨어지며 도는 그림 수
const KINDS = {
  light_snow: { count: 30, fall: [24, 46], wind: [-5, 5], sway: 12, rest: 0.6, box: 9, frames: 1 },
  snow: { count: 55, fall: [28, 62], wind: [-7, 7], sway: 14, rest: 0.6, box: 9, frames: 1 },
  rain: { count: 70, fall: [520, 700], wind: [-46, -34], sway: 0, rest: 0.12, box: 18, frames: 1 },
  shower: { count: 70, fall: [620, 820], wind: [-70, -50], sway: 0, rest: 0.12, box: 18, frames: 1 },
  petals: { count: 35, fall: [30, 56], wind: [12, 30], sway: 20, rest: 1.0, box: 13, frames: 8 },
  leaves: { count: 30, fall: [36, 68], wind: [-14, 14], sway: 34, rest: 1.4, box: 15, frames: 8 },
};
// 낙엽 색: 속, 테두리
const LEAF_TONES = [
  ["#d6452b", "rgba(120,36,18,0.85)"],
  ["#ee8a24", "rgba(140,76,14,0.85)"],
  ["#f2c230", "rgba(150,110,16,0.85)"],
];

// 픽셀 스타일 (window.ART_STYLE, 앱의 art-style 이벤트): 칸(PX_DOT px)으로만 그린다 — 안티에일리어싱 없음, 자리도 칸에 맞춘다, 더미는 칸 계단.
// 그림은 글자 표 (글자 → 색), 도는 낙엽·꽃잎은 90도 네 장을 번갈아 (frames 수는 같게 맞춘다)
const PX_DOT = 3;
const PX_COLORS = {
  w: "#ffffff", c: "#b4cdeb", b: "#6491d2", B: "#4a70b0",
  p: "#fbc4d4", P: "#d87896",
  r: "#d6452b", R: "#78241a", o: "#ee8a24", O: "#8c4c0e", y: "#f2c230", Y: "#966e10",
};
const PX_FLAKES = [["w"], ["cw", "wc"], [".c.", "cwc", ".c."]];
const PX_DROP = ["..b.", "..b.", ".b..", ".b.."];
const PX_SPLASH = ["b..b", ".bb."];
const PX_PETAL = ["pp.", "Ppp", ".pP"];
const PX_LEAF = [".LL.", "LLLF", "FLLL", ".FF."];
const PX_BOX = { light_snow: 9, snow: 9, rain: 12, shower: 12, petals: 9, leaves: 12 };

function pxRotate(rows, turns) {
  let out = rows;
  for (let t = 0; t < turns; t++) {
    out = Array.from({ length: out[0].length }, (_, x) => out.map((r) => r[x]).reverse().join(""));
  }
  return out;
}

// 표 → 칸 그림 (box 안 가운데, 칸에 맞춰)
function pxSprite(rows, box, colors = PX_COLORS) {
  return makeSprite(box, (g) => {
    const x0 = Math.floor((box / PX_DOT - rows[0].length) / 2) * PX_DOT;
    const y0 = Math.floor((box / PX_DOT - rows.length) / 2) * PX_DOT;
    rows.forEach((row, j) => {
      [...row].forEach((ch, i) => {
        if (ch === ".") return;
        g.fillStyle = colors[ch];
        g.fillRect(x0 + i * PX_DOT, y0 + j * PX_DOT, PX_DOT, PX_DOT);
      });
    });
  });
}

let pixelArt = window.ART_STYLE === "pixel";

const pilesCanvas = document.getElementById("piles");
const fallCanvas = document.getElementById("fall");
const pilesCtx = pilesCanvas.getContext("2d");
const fallCtx = fallCanvas.getContext("2d");

let width = 0;
let height = 0;
let kind = null;
let spec = null;
let sprites = [];
let splash = null;
let tops = [];
let piles = [];
let lastPiles = "";
let pileKind = null;
let running = false;
let timer = 0;
let lastT = 0;
let cursor = 0;
let n = 0;
// 알갱이: 자리, 지난 자리, 속도, 흔들림 위상, 내려앉을 높이, 머문 시간(>0이면 내려앉음), 그림
let px, py, oldX, oldY, vy, vx, phase, land, rest, look;

function makeSprite(size, draw) {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  draw(c.getContext("2d"), size);
  return c;
}

function flake(radius) {
  return (g, s) => {
    g.beginPath();
    g.arc(s / 2, s / 2, radius, 0, Math.PI * 2);
    g.fillStyle = "#fff";
    g.fill();
    // 흰 창 위에서도 보이게 옅은 테두리
    g.lineWidth = 0.8;
    g.strokeStyle = "rgba(120,150,190,0.55)";
    g.stroke();
  };
}

function petal(angle) {
  return (g, s) => {
    g.translate(s / 2, s / 2);
    g.rotate(angle);
    g.beginPath();
    g.moveTo(-4.6, 0);
    g.bezierCurveTo(-2.4, -3.4, 2.6, -3.2, 4.8, -0.6);
    g.lineTo(3.6, 0.3);
    g.lineTo(4.6, 1.0);
    g.bezierCurveTo(2.4, 3.2, -2.4, 3.2, -4.6, 0);
    g.fillStyle = "#fbc4d4";
    g.fill();
    g.lineWidth = 0.7;
    g.strokeStyle = "rgba(216,120,150,0.8)";
    g.stroke();
  };
}

// 도는 낙엽: 납작해졌다 펴지며 뒤집히는 것처럼 보인다 (a/8 바퀴)
function leaf([fill, line], a) {
  const angle = (a * Math.PI) / 4;
  return (g, s) => {
    g.translate(s / 2, s / 2);
    g.rotate(angle);
    g.scale(1, 0.45 + 0.55 * Math.abs(Math.cos(angle)));
    g.beginPath();
    g.moveTo(-6, 0);
    g.bezierCurveTo(-3, -4.6, 2, -4.4, 6, 0);
    g.bezierCurveTo(2, 4.4, -3, 4.6, -6, 0);
    g.fillStyle = fill;
    g.fill();
    g.lineWidth = 0.8;
    g.strokeStyle = line;
    g.stroke();
    g.beginPath();
    g.moveTo(-6.6, 0);
    g.lineTo(4, 0);
    g.lineWidth = 0.6;
    g.stroke();
  };
}

// 떨어질 때도 더미에서도 같은 그림 (한 번만 만든다). 낙엽은 색마다 frames장: [색0 각도0..7, 색1 각도0..7, ...]
const tumblers = {
  petals: () => [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5].map((k) => makeSprite(KINDS.petals.box, petal(k * 0.8))),
  leaves: () => LEAF_TONES.flatMap((tone) => Array.from({ length: KINDS.leaves.frames }, (_, a) => makeSprite(KINDS.leaves.box, leaf(tone, a)))),
};
// 픽셀: 같은 장 수, 90도씩만 돈다
const pxTumblers = {
  petals: () => Array.from({ length: KINDS.petals.frames }, (_, a) => pxSprite(pxRotate(PX_PETAL, a % 4), PX_BOX.petals)),
  leaves: () =>
    [["r", "R"], ["o", "O"], ["y", "Y"]].flatMap(([fill, dark]) =>
      Array.from({ length: KINDS.leaves.frames }, (_, a) => pxSprite(pxRotate(PX_LEAF, a % 4), PX_BOX.leaves, { L: PX_COLORS[fill], F: PX_COLORS[dark] })),
    ),
};
const tumblerCache = {};
function tumblerSprites(k) {
  const key = `${pixelArt}:${k}`;
  if (!tumblerCache[key]) tumblerCache[key] = (pixelArt ? pxTumblers : tumblers)[k]();
  return tumblerCache[key];
}

function buildSprites() {
  splash = null;
  if (pixelArt) {
    if (kind === "rain" || kind === "shower") {
      sprites = [pxSprite(PX_DROP, spec.box)];
      splash = pxSprite(PX_SPLASH, spec.box);
    } else if (tumblers[kind]) {
      sprites = tumblerSprites(kind);
    } else {
      sprites = PX_FLAKES.map((rows) => pxSprite(rows, spec.box));
    }
    return;
  }
  if (kind === "rain" || kind === "shower") {
    sprites = [
      makeSprite(spec.box, (g, s) => {
        g.beginPath();
        g.moveTo(s / 2 + 1.2, 1);
        g.lineTo(s / 2 - 1.2, s - 1);
        g.lineWidth = 1.2;
        g.lineCap = "round";
        g.strokeStyle = "rgba(100,145,210,0.65)";
        g.stroke();
      }),
    ];
    splash = makeSprite(spec.box, (g, s) => {
      g.lineWidth = 1;
      g.lineCap = "round";
      g.strokeStyle = "rgba(100,145,210,0.7)";
      for (const [dx, dy] of [
        [-4.5, -2.4],
        [4.5, -2.4],
        [-1.6, -4],
        [1.6, -4],
      ]) {
        g.beginPath();
        g.moveTo(s / 2 + dx * 0.4, s - 2 + dy * 0.2);
        g.lineTo(s / 2 + dx, s - 2 + dy);
        g.stroke();
      }
    });
  } else if (tumblers[kind]) {
    sprites = tumblerSprites(kind);
  } else {
    sprites = [1.6, 2.2, 2.9].map((r) => makeSprite(spec.box, flake(r)));
  }
}

function between([a, b]) {
  return a + Math.random() * (b - a);
}

function pileHeightAt(x, y) {
  for (const p of piles) {
    if (p.y !== y) continue;
    const h = p.h[Math.floor((x - p.x) / p.cell)];
    if (h) return h / 10;
  }
  return 0;
}

// x에서 y 아래 가장 먼저 만나는 디딤판 (더미 꼭대기). 없으면 화면 밑
function landAt(rawX, y) {
  // 화면 옆으로 살짝 나간 알갱이도 끝자리의 디딤판을 본다 (다시 들어올 때 뚫고 지나가지 않게)
  const x = Math.min(Math.max(rawX, 0), width);
  let best = Infinity;
  for (const t of tops) {
    if (x >= t[0] && x <= t[1] && t[2] >= y && t[2] < best) best = t[2];
  }
  return best === Infinity ? height + spec.box : best - pileHeightAt(x, best);
}

function respawn(i, first) {
  // 바람에 밀려 오는 만큼 옆 여유를 두고 위에서 떨어진다
  px[i] = Math.random() * width;
  py[i] = first ? -Math.random() * height : -spec.box - Math.random() * height * 0.2;
  vy[i] = between(spec.fall);
  vx[i] = between(spec.wind);
  phase[i] = Math.random() * Math.PI * 2;
  look[i] = Math.floor(Math.random() * sprites.length);
  rest[i] = 0;
  land[i] = landAt(px[i], py[i]);
}

function startKind(next) {
  kind = next;
  spec = KINDS[kind] || KINDS.snow;
  if (pixelArt) spec = { ...spec, box: PX_BOX[kind] ?? PX_BOX.snow };
  buildSprites();
  n = Math.round((spec.count * Math.max(width, 1)) / 1920);
  n = Math.min(MAX_PARTICLES, Math.max(MIN_PARTICLES, n));
  [px, py, oldX, oldY, vy, vx, phase, land, rest] = Array.from({ length: 9 }, () => new Float32Array(n));
  look = new Uint8Array(n);
  fallCtx.clearRect(0, 0, width, height);
  for (let i = 0; i < n; i++) {
    respawn(i, true);
    oldX[i] = px[i];
    oldY[i] = py[i];
  }
}

// 이 알갱이의 지금 그림: 내려앉은 빗방울은 튀는 그림, 꽃잎·낙엽은 제 색 그대로 각도만 돈다
function spriteFor(i) {
  if (splash && rest[i] > 0) return splash;
  const frames = spec.frames;
  if (frames === 1) return sprites[look[i]];
  const angle = look[i] % frames;
  return sprites[look[i] - angle + (angle + Math.floor(phase[i] * 1.2)) % frames];
}

// ---- 🧹 흩날리는 알갱이 (빗자루로 쓴 것, 친구가 쓸어 보낸 것) ----
// 받은 속도로 날아가다 공기에 느려지며 천천히 떨어져 사라진다. 없으면 일도 없다
const MAX_FLURRY = 120;
const FLURRY_DRAG = 1.8;
const FLURRY_GRAVITY = 90;
const FLURRY_MAX_FALL = 90;
const FLURRY_MAX_SPEED = 900;
const FLURRY_LIFE = [1.8, 3.6];
let flurry = [];

function flurryBox(k) {
  return pixelArt ? PX_BOX[k] ?? PX_BOX.snow : (KINDS[k] || KINDS.snow).box;
}

function flurrySprites(k) {
  if (tumblers[k]) return tumblerSprites(k);
  const key = `${pixelArt}:flake`;
  if (!tumblerCache[key]) {
    const box = flurryBox("snow");
    tumblerCache[key] = pixelArt ? PX_FLAKES.map((rows) => pxSprite(rows, box)) : [1.6, 2.2, 2.9].map((r) => makeSprite(box, flake(r)));
  }
  return tumblerCache[key];
}

function addFlurry(f) {
  if (!running || !KINDS[f.kind]) return;
  const speed = Math.hypot(f.vx, f.vy);
  const limit = speed > FLURRY_MAX_SPEED ? FLURRY_MAX_SPEED / speed : 1;
  const count = Math.min(f.count, MAX_FLURRY);
  const look = flurrySprites(f.kind);
  const box = flurryBox(f.kind);
  const tumbling = !!tumblers[f.kind];
  // 넘치면 가장 오래된 것부터 (지울 자리는 이번 프레임에 함께 지운다)
  const over = flurry.length + count - MAX_FLURRY;
  if (over > 0) {
    for (const p of flurry.splice(0, over)) fallCtx.clearRect(p.oldX, p.oldY, p.box, p.box);
  }
  for (let i = 0; i < count; i++) {
    const spread = 0.55 + Math.random() * 0.9;
    flurry.push({
      x: f.x + (Math.random() - 0.5) * 50,
      y: f.y + (Math.random() - 0.5) * 50,
      vx: f.vx * limit * spread + (Math.random() - 0.5) * 120,
      vy: f.vy * limit * spread - Math.random() * 110,
      age: 0,
      life: between(FLURRY_LIFE),
      look,
      first: Math.floor(Math.random() * look.length),
      tumbling,
      box,
      oldX: -box * 2,
      oldY: -box * 2,
    });
  }
}

function clearFlurry() {
  for (const p of flurry) fallCtx.clearRect(p.oldX, p.oldY, p.box, p.box);
}

function stepFlurry(dt) {
  const drag = Math.max(0, 1 - FLURRY_DRAG * dt);
  let kept = 0;
  for (const p of flurry) {
    p.age += dt;
    p.vx *= drag;
    p.vy = Math.min(p.vy * drag + FLURRY_GRAVITY * dt, FLURRY_MAX_FALL + Math.max(0, p.vy * drag));
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    if (p.age >= p.life || p.x < -p.box || p.x > width + p.box || p.y > height + p.box) continue;
    flurry[kept++] = p;
    // 도는 그림은 같은 색 8장 안에서 돈다
    const sprite = p.tumbling ? p.look[p.first - (p.first % 8) + ((p.first % 8) + Math.floor(p.age * 9)) % 8] : p.look[p.first];
    const x = pixelArt ? Math.floor((p.x - p.box / 2) / PX_DOT) * PX_DOT : (p.x - p.box / 2) | 0;
    const y = pixelArt ? Math.floor((p.y - p.box / 2) / PX_DOT) * PX_DOT : (p.y - p.box / 2) | 0;
    fallCtx.drawImage(sprite, x, y);
    p.oldX = x;
    p.oldY = y;
  }
  flurry.length = kept;
}

function frame(now) {
  if (!running) return;
  const dt = Math.min((now - lastT) / 1000, 0.1);
  lastT = now;
  clearFlurry();
  const box = spec.box;
  const half = box / 2;
  const sway = spec.sway;
  for (let i = 0; i < n; i++) fallCtx.clearRect(oldX[i], oldY[i], box, box);
  for (let k = 0; k < RELAND_PER_FRAME && n > 0; k++) {
    cursor = (cursor + 1) % n;
    if (rest[cursor] === 0) land[cursor] = landAt(px[cursor], py[cursor]);
  }
  for (let i = 0; i < n; i++) {
    if (rest[i] > 0) {
      rest[i] -= dt;
      if (rest[i] <= 0) respawn(i, false);
    } else {
      phase[i] += dt * 1.6;
      px[i] += (vx[i] + Math.sin(phase[i]) * sway) * dt;
      py[i] += vy[i] * dt;
      if (px[i] < -box) px[i] = width + box / 2;
      else if (px[i] > width + box) px[i] = -box / 2;
      if (py[i] >= land[i]) {
        py[i] = land[i];
        rest[i] = spec.rest;
      }
    }
    if (py[i] < -box) {
      oldX[i] = -box * 2;
      oldY[i] = -box * 2;
      continue;
    }
    const sprite = spriteFor(i);
    // 그림 아래쪽이 내려앉는 높이에 닿는다
    // 픽셀: 자리도 칸에 맞춘다
    const x = pixelArt ? Math.floor((px[i] - half) / PX_DOT) * PX_DOT : (px[i] - half) | 0;
    const y = pixelArt ? Math.floor((py[i] - box + 2) / PX_DOT) * PX_DOT : (py[i] - box + 2) | 0;
    fallCtx.drawImage(sprite, x, y);
    oldX[i] = x;
    oldY[i] = y;
  }
  stepFlurry(dt);
  schedule();
}

// rAF는 창이 가려지면 쉬고, 타이머가 24 fps 아래로 묶어 둔다 (rAF만 쓰면 화면 주사율마다 깨어난다)
function schedule() {
  timer = setTimeout(() => requestAnimationFrame(frame), FRAME_MS);
}

function start() {
  if (running || !spec) return;
  running = true;
  lastT = performance.now();
  schedule();
}

function stop() {
  running = false;
  clearTimeout(timer);
  fallCtx.clearRect(0, 0, width, height);
  pilesCtx.clearRect(0, 0, width, height);
  lastPiles = "";
  flurry = [];
}

function hash(a, b) {
  const v = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
  return v - Math.floor(v);
}

function drawSnowPile(p) {
  const y = p.y;
  const g = pilesCtx;
  g.beginPath();
  g.moveTo(p.x, y);
  let prevX = p.x;
  let prevY = y;
  for (let i = 0; i < p.h.length; i++) {
    const cx = p.x + (i + 0.5) * p.cell;
    const cy = y - p.h[i] / 10;
    g.quadraticCurveTo(prevX, prevY, (prevX + cx) / 2, (prevY + cy) / 2);
    prevX = cx;
    prevY = cy;
  }
  g.quadraticCurveTo(prevX, prevY, p.x + p.h.length * p.cell, y);
  g.closePath();
  g.fillStyle = "#fdfeff";
  g.fill();
  g.lineWidth = 1;
  g.strokeStyle = "rgba(120,150,190,0.8)";
  g.stroke();
}

// 픽셀 눈 더미: 칸 너비 기둥을 칸 높이로 끊어 쌓는다 (곡선 없음). 맨 위 칸은 옅은 파랑 — 흰 창 위에서도 보이게
function drawSnowPilePixel(p) {
  const g = pilesCtx;
  const n = p.h.length;
  const heightAt = (x) => {
    const u = (x - p.x) / p.cell - 0.5;
    const i = Math.max(0, Math.min(n - 1, Math.floor(u)));
    const j = Math.min(n - 1, i + 1);
    const t = Math.max(0, Math.min(1, u - i));
    return (p.h[i] + (p.h[j] - p.h[i]) * t) / 10;
  };
  const left = Math.floor(p.x / PX_DOT) * PX_DOT;
  for (let x = left; x < p.x + n * p.cell; x += PX_DOT) {
    const h = Math.round(heightAt(x + PX_DOT / 2) / PX_DOT) * PX_DOT;
    if (h <= 0) continue;
    g.fillStyle = PX_COLORS.w;
    g.fillRect(x, p.y - h, PX_DOT, h);
    g.fillStyle = PX_COLORS.c;
    g.fillRect(x, p.y - h, PX_DOT, PX_DOT);
  }
}

function drawSpritePile(p) {
  const g = pilesCtx;
  const look = tumblerSprites(pileKind);
  const half = (pixelArt ? PX_BOX[pileKind] : KINDS[pileKind].box) >> 1;
  for (let i = 0; i < p.h.length; i++) {
    const count = Math.round((p.h[i] / 10) * 1.1);
    for (let k = 0; k < count; k++) {
      const x = p.x + (i + hash(i, k)) * p.cell;
      const y = p.y - 3 - hash(k, i + 7) * (p.h[i] / 10);
      const dx = pixelArt ? Math.floor((x - half) / PX_DOT) * PX_DOT : (x - half) | 0;
      const dy = pixelArt ? Math.floor((y - half) / PX_DOT) * PX_DOT : (y - half) | 0;
      g.drawImage(look[Math.floor(hash(i + 3, k) * look.length)], dx, dy);
    }
  }
}

// 장면이 바뀔 때만 (많아야 1초에 몇 번)
function drawPiles() {
  const g = pilesCtx;
  g.clearRect(0, 0, width, height);
  for (const p of piles) {
    g.save();
    g.beginPath();
    for (const c of p.clips) g.rect(c[0], p.y - 32, c[1] - c[0], 33);
    g.clip();
    if (tumblers[pileKind]) drawSpritePile(p);
    else if (pixelArt) drawSnowPilePixel(p);
    else drawSnowPile(p);
    g.restore();
  }
}

function applyScene(s) {
  if (s.paused) {
    stop();
    return;
  }
  tops = s.tops;
  piles = s.piles;
  pileKind = s.pile_kind;
  if (s.kind !== kind) startKind(s.kind);
  else {
    for (let i = 0; i < n; i++) {
      if (rest[i] === 0) land[i] = landAt(px[i], py[i]);
    }
  }
  const pilesNow = JSON.stringify([pileKind, piles]);
  if (pilesNow !== lastPiles) {
    lastPiles = pilesNow;
    drawPiles();
  }
  start();
}

function fitCanvas() {
  width = window.innerWidth;
  height = window.innerHeight;
  pilesCanvas.width = fallCanvas.width = width;
  pilesCanvas.height = fallCanvas.height = height;
  lastPiles = "";
  // 동글은 예전 그대로 (캔버스 스타일을 건드리지 않는다)
  if (pixelArt || pilesCanvas.style?.imageRendering) pilesCanvas.style.imageRendering = fallCanvas.style.imageRendering = pixelArt ? "pixelated" : "";
  if (kind) {
    const k = kind;
    kind = null;
    startKind(k);
  }
}

// 스타일이 바뀌면 알갱이·더미를 새 그림으로
function setPixelArt(on) {
  if (on === pixelArt) return;
  pixelArt = on;
  fitCanvas();
  if (running) drawPiles();
}

window.addEventListener("resize", fitCanvas);
window.addEventListener("contextmenu", (e) => e.preventDefault());
fitCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 장면만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받는다)
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("art-style", (e) => setPixelArt(e.payload === "pixel"));
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("weather-flurry", (e) => addFlurry(e.payload));
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("weather-scene", (e) => applyScene(e.payload));
} else {
  devPreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 창 두 개 + 자란 더미
function devPreview() {
  const wanted = new URLSearchParams(location.search).get("preview") || "snow";
  if (new URLSearchParams(location.search).get("pixel")) setPixelArt(true);
  const wet = wanted === "rain" || wanted === "shower";
  document.documentElement.style.background = "#3a4258";
  const windows = [
    [Math.round(width * 0.08), Math.round(height * 0.3), Math.round(width * 0.5), Math.round(height * 0.86)],
    [Math.round(width * 0.42), Math.round(height * 0.16), Math.round(width * 0.9), Math.round(height * 0.64)],
  ];
  for (const [l, t, r, b] of windows) {
    const el = document.createElement("div");
    el.style.cssText = `position:absolute;left:${l}px;top:${t}px;width:${r - l}px;height:${b - t}px;background:#f4f5f8;border-radius:6px;box-shadow:0 4px 18px #0005`;
    document.body.insertBefore(el, pilesCanvas);
  }
  const back = windows[0];
  const front = windows[1];
  const tops = [
    [front[0], front[2], front[1]],
    [back[0], front[0], back[1]],
    [0, width, height],
  ];
  const cell = 20;
  const mound = (x0, x1, i) => Math.round(30 + 34 * (0.5 + 0.5 * Math.sin((i * cell) / 90 + x0)) + 12 * Math.sin(i * 1.7));
  const pile = (x0, x1, y, clips) => {
    const h = [];
    for (let i = 0; i < Math.ceil((x1 - x0) / cell); i++) h.push(tumblers[wanted] ? Math.round(mound(x0, x1, i) * 0.5) : mound(x0, x1, i));
    return { x: x0, y, cell, h, clips };
  };
  const piles = wet ? [] : [pile(front[0], front[2], front[1], [[front[0], front[2]]]), pile(back[0], back[2], back[1], [[back[0], front[0]]]), pile(0, width, height, [[0, width]])];
  applyScene({ paused: false, kind: wanted, pile_kind: wet ? null : wanted, tops, piles });
}
