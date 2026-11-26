// 🦇 박쥐인간의 🔦 박쥐 신호 그리기. 🌈 무지개 구름·🖍️ 낙서·🕷️ 거미집과 같은 모니터 덮개 창 (trail.html)의 맨 아래 캔버스에 그림.
// 앱(src-tauri/src/trail.rs)이 보내는 것 (논리 px, 이 창 왼쪽 위가 0,0):
//   trail-signal  새 박쥐 신호들 [{x, y, size, secs, age}] — x·y는 바닥 조명의 렌즈, size는 몸 크기,
//                 secs초 동안 비추고 SIG_FADE초 동안 흐려짐 (age는 이미 지난 초)
// 그림: 렌즈에서 화면 위쪽 (높이의 SIG_SKY 자리)으로 넓어지는 반투명 노란 빛줄기 + 그 끝의 둥근 빛 속 까만 박쥐 무늬 — 빛은 살짝 좌우로 흔들림.
// CPU를 아끼는 규칙: 신호가 있는 동안만 requestAnimationFrame, 없으면 아무것도 안 함. 한 번에 SIG_MAX개까지.
// 브라우저에서 trail.html?signal=1로 앱 없이 볼 수 있음.

const SIG_FADE = 0.8;
const SIG_RISE = 0.4;
const SIG_MAX = 4;
// 받아들이는 비추는 시간 (초)
const SIG_SECS_RANGE = [0.5, 30];
// 빛 동그라미가 뜨는 높이 (창 높이 몫) · 빛 동그라미 반지름 (몸 크기 배) · 흔들림 (몸 크기 배, 주기 초)
const SIG_SKY = 0.13;
const SIG_SPOT = [1.25, 0.6];
const SIG_SWAY = [0.35, 3.2];
// 박쥐 무늬 (tools/draw_bat.py bat_shape와 같은 점 — 오른쪽 반, 왼쪽은 뒤집음)
const SIG_BAT_TOP = [[0.07, -0.22], [0.15, -0.5], [0.23, -0.2], [0.52, -0.36], [1.0, -0.16]];
const SIG_BAT_BOT = [[0.84, 0.12], [0.68, 0.02], [0.56, 0.3], [0.4, 0.1], [0.26, 0.36], [0.12, 0.18], [0.0, 0.46]];

const sigCanvas = document.getElementById("signal");
const sctx = sigCanvas.getContext("2d");
let signals = [];
let sigFrame = 0;

// 새 박쥐 신호들을 받음 (이상한 값은 버림)
function addSignals(list) {
  const now = performance.now();
  for (const p of Array.isArray(list) ? list : []) {
    if (!p || !Number.isFinite(p.x) || !Number.isFinite(p.y) || !(p.size > 0 && p.size < 2000)) continue;
    if (!(p.secs >= SIG_SECS_RANGE[0] && p.secs <= SIG_SECS_RANGE[1])) continue;
    if (!(p.age >= 0 && p.age < p.secs + SIG_FADE)) continue;
    signals.push({ x: p.x, y: p.y, size: p.size, secs: p.secs, born: now - p.age * 1000 });
  }
  if (signals.length > SIG_MAX) signals = signals.slice(signals.length - SIG_MAX);
  wakeSignals();
}

// 박쥐 무늬 다각형 점들 (가운데 cx·cy, 반폭 w)
function sigBatPath(cx, cy, w) {
  const right = SIG_BAT_TOP.concat(SIG_BAT_BOT);
  const left = right.slice(0, -1).reverse().map(([x, y]) => [-x, y]);
  return right.concat(left).map(([x, y]) => [cx + x * w, cy + y * w]);
}

// 신호 하나의 밝기 (0…1): 켜지며 밝아지고, 끝에 흐려짐
function sigAlpha(s, t) {
  if (t < SIG_RISE) return t / SIG_RISE;
  if (t < s.secs) return 1;
  return Math.max(0, 1 - (t - s.secs) / SIG_FADE);
}

// 신호 하나를 그림 (t = 켠 뒤 초)
function drawSignal(s, t) {
  const a = sigAlpha(s, t);
  if (a <= 0) return;
  const H = sigCanvas.height;
  const sway = Math.sin((t / SIG_SWAY[1]) * Math.PI * 2) * SIG_SWAY[0] * s.size;
  const sx = s.x + sway;
  const sy = Math.min(H * SIG_SKY, s.y - s.size * 2);
  const rx = s.size * SIG_SPOT[0];
  const ry = s.size * SIG_SPOT[1];
  const lens = s.size * 0.07;
  // 빛줄기 (렌즈 → 빛 동그라미 양옆)
  const g = sctx.createLinearGradient(s.x, s.y, sx, sy);
  g.addColorStop(0, `rgba(255,240,170,${0.42 * a})`);
  g.addColorStop(1, `rgba(255,236,150,${0.14 * a})`);
  sctx.fillStyle = g;
  sctx.beginPath();
  sctx.moveTo(s.x - lens, s.y);
  sctx.lineTo(sx - rx * 0.92, sy);
  sctx.lineTo(sx + rx * 0.92, sy);
  sctx.lineTo(s.x + lens, s.y);
  sctx.closePath();
  sctx.fill();
  // 빛 동그라미 + 까만 박쥐
  sctx.fillStyle = `rgba(255,232,140,${0.62 * a})`;
  sctx.beginPath();
  sctx.ellipse(sx, sy, rx, ry, 0, 0, Math.PI * 2);
  sctx.fill();
  sctx.fillStyle = `rgba(24,24,34,${0.88 * a})`;
  sctx.beginPath();
  sigBatPath(sx, sy + ry * 0.05, rx * 0.72).forEach(([x, y], i) => (i ? sctx.lineTo(x, y) : sctx.moveTo(x, y)));
  sctx.closePath();
  sctx.fill();
}

// 한 프레임: 끝난 신호는 버리고, 남았으면 다음 프레임
function drawSignals() {
  sigFrame = 0;
  const now = performance.now();
  signals = signals.filter((s) => (now - s.born) / 1000 < s.secs + SIG_FADE);
  sctx.clearRect(0, 0, sigCanvas.width, sigCanvas.height);
  sctx.imageSmoothingEnabled = false;
  for (const s of signals) drawSignal(s, (now - s.born) / 1000);
  if (signals.length) sigFrame = requestAnimationFrame(drawSignals);
}

// 그리기를 깨움 (이미 돌고 있으면 그대로)
function wakeSignals() {
  if (!sigFrame) sigFrame = requestAnimationFrame(drawSignals);
}

// 캔버스를 창 크기에 맞춤
function fitSignalCanvas() {
  sigCanvas.width = window.innerWidth;
  sigCanvas.height = window.innerHeight;
  if (signals.length) wakeSignals();
}

window.addEventListener("resize", fitSignalCanvas);
fitSignalCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 것만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  (window.spOn || ((t, e, h) => t.listen(e, h)))(window.__TAURI__.webviewWindow.getCurrentWebviewWindow(), "trail-signal", (e) => addSignals(e.payload));
} else if (new URLSearchParams(location.search).get("signal")) {
  // 앱 없이 브라우저에서 보기 (개발용): 화면 아래에서 비추는 신호 둘
  document.documentElement.style.background = "#1d2030";
  addSignals([
    { x: window.innerWidth * 0.3, y: window.innerHeight - 40, size: 110, secs: 30, age: 0 },
    { x: window.innerWidth * 0.7, y: window.innerHeight - 40, size: 80, secs: 30, age: 0 },
  ]);
}
