// 계절 날씨 그리기. 앱(src-tauri/src/weather.rs)이 `weather-scene`으로 내려앉을 곳(tops)과 더미(piles)를 보낸다 (논리 px, 이 창 왼쪽 위가 0,0).
// CPU를 아끼는 규칙: 24 fps 이하, 알갱이 80개 이하, 미리 그린 그림을 정수 자리에 찍기, 알갱이 자리만 지우기, 더미는 바뀔 때만.
// 멈춰 있으면(paused) 타이머도 돌지 않는다. 브라우저에서 weather.html?preview=snow (light_snow·rain·petals)로 앱 없이 볼 수 있다.

const FPS = 24;
const FRAME_MS = 1000 / FPS;
const MAX_PARTICLES = 80;
const MIN_PARTICLES = 30;
// 한 프레임에 다시 재 보는 알갱이 수 (옆으로 흘러도 내려앉을 곳이 맞도록)
const RELAND_PER_FRAME = 6;

// count: 폭 1920 논리 px에서의 개수, fall·wind·sway: px/s, rest: 내려앉아 머무는 초, box: 그림 칸
const KINDS = {
  light_snow: { count: 30, fall: [24, 46], wind: [-5, 5], sway: 12, rest: 0.6, box: 9 },
  snow: { count: 55, fall: [28, 62], wind: [-7, 7], sway: 14, rest: 0.6, box: 9 },
  rain: { count: 70, fall: [520, 700], wind: [-46, -34], sway: 0, rest: 0.12, box: 18 },
  petals: { count: 35, fall: [30, 56], wind: [12, 30], sway: 20, rest: 1.0, box: 13 },
};

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

function buildSprites() {
  splash = null;
  if (kind === "rain") {
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
  } else if (kind === "petals") {
    sprites = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5].map((k) => makeSprite(spec.box, petal(k * 0.8)));
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

function frame(now) {
  if (!running) return;
  const dt = Math.min((now - lastT) / 1000, 0.1);
  lastT = now;
  const box = spec.box;
  const half = box / 2;
  const sway = spec.sway;
  for (let i = 0; i < n; i++) fallCtx.clearRect(oldX[i], oldY[i], box, box);
  for (let k = 0; k < RELAND_PER_FRAME && n > 0; k++) {
    cursor = (cursor + 1) % n;
    if (rest[cursor] === 0) land[cursor] = landAt(px[cursor], py[cursor]);
  }
  const spin = kind === "petals";
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
    const sprite = splash && rest[i] > 0 ? splash : sprites[spin ? (look[i] + Math.floor(phase[i] * 1.2)) % sprites.length : look[i]];
    // 그림 아래쪽이 내려앉는 높이에 닿는다
    const x = (px[i] - half) | 0;
    const y = (py[i] - box + 2) | 0;
    fallCtx.drawImage(sprite, x, y);
    oldX[i] = x;
    oldY[i] = y;
  }
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

function drawPetalPile(p) {
  const g = pilesCtx;
  const look = pileSprites();
  for (let i = 0; i < p.h.length; i++) {
    const count = Math.round((p.h[i] / 10) * 1.1);
    for (let k = 0; k < count; k++) {
      const x = p.x + (i + hash(i, k)) * p.cell;
      const y = p.y - 3 - hash(k, i + 7) * (p.h[i] / 10);
      g.drawImage(look[Math.floor(hash(i + 3, k) * look.length)], (x - 6) | 0, (y - 6) | 0);
    }
  }
}

let petalLook = null;
function pileSprites() {
  if (!petalLook) petalLook = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5].map((k) => makeSprite(13, petal(k * 0.8)));
  return petalLook;
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
    if (pileKind === "petals") drawPetalPile(p);
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
  if (kind) {
    const k = kind;
    kind = null;
    startKind(k);
  }
}

window.addEventListener("resize", fitCanvas);
window.addEventListener("contextmenu", (e) => e.preventDefault());
fitCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 장면만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받는다)
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("weather-scene", (e) => applyScene(e.payload));
} else {
  devPreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 창 두 개 + 자란 더미
function devPreview() {
  const wanted = new URLSearchParams(location.search).get("preview") || "snow";
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
    for (let i = 0; i < Math.ceil((x1 - x0) / cell); i++) h.push(wanted === "rain" ? 0 : wanted === "petals" ? Math.round(mound(x0, x1, i) * 0.5) : mound(x0, x1, i));
    return { x: x0, y, cell, h, clips };
  };
  const piles =
    wanted === "rain"
      ? []
      : [pile(front[0], front[2], front[1], [[front[0], front[2]]]), pile(back[0], back[2], back[1], [[back[0], front[0]]]), pile(0, width, height, [[0, width]])];
  applyScene({ paused: false, kind: wanted, pile_kind: wanted === "rain" ? null : wanted, tops, piles });
}
