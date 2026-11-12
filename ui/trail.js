// 🌈 무지개 구름 자국 그리기. 앱(src-tauri/src/trail.rs)이 `trail-puffs`로 새 구름들을 보냄
// (논리 px, 이 창 왼쪽 위가 0,0 — x·y는 구름 가운데, size는 폭, color는 RAINBOW 순서, age는 이미 지난 초).
// 구름은 놓인 자리에서 살짝 떠오르며 부풀고 LIFE초 동안 흐려짐. CPU를 아끼는 규칙: 구름 MAX_PUFFS개까지,
// 색마다 미리 그린 그림을 찍기, 지난 프레임에 그린 자리만 지우기, 구름이 없으면 requestAnimationFrame도 걸지 않음.
// 브라우저에서 trail.html?preview=1 (&pixel=1, &light=1)로 앱 없이 볼 수 있음.

const LIFE = 1.0;
const MAX_PUFFS = 60;
// 사는 동안 떠오르는 거리, 뒤로 흘러가는 거리 (논리 px)
const RISE = 16;
const DRIFT = 14;
// ui/main.js의 무지개 몸 색과 같은 일곱 색
const RAINBOW = ["#ff5b5b", "#ff9838", "#f7cc2f", "#45cc78", "#48c0f5", "#4d7bff", "#a46bff"];
// 동글 구름: 지름 1인 구름 안의 동그라미들 (가운데 x, y, 반지름)
const BLOBS = [
  [-0.24, 0.08, 0.22],
  [0.0, -0.06, 0.29],
  [0.25, 0.06, 0.22],
  [0.02, 0.14, 0.22],
];
const ROUND_BOX = 64;
// 픽셀 구름: 글자 표 (h 밝은 윗면, b 몸, s 아랫 그늘). 테두리는 그릴 때 둘레에 칸을 더함
const PX_CLOUD = [
  "...hhh......",
  "..hbbbh.hh..",
  ".hbbbbbhbbh.",
  "hbbbbbbbbbbh",
  "bbbbbbbbbbbb",
  ".ssssssssss.",
];

const canvas = document.getElementById("trail");
const ctx = canvas.getContext("2d");
const roundSprites = new Map();
const pixelSprites = new Map();
let puffs = [];
let frameRequest = 0;
// 지난 프레임에 그린 자리 (그 자리만 지움)
let dirty = null;

// 색 hex를 to(0 검정·255 흰색) 쪽으로 k만큼 섞음
function mix(hex, to, k) {
  const n = parseInt(hex.slice(1), 16);
  const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((v) => Math.round(v + (to - v) * k));
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

// w×h 캔버스에 draw로 그린 스프라이트를 만듦
function makeSprite(w, h, draw) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  draw(c.getContext("2d"));
  return c;
}

// 둥근 구름 덩어리 모양을 grow만큼 부풀려 칠함
function blobs(g, grow, s) {
  g.beginPath();
  for (const [x, y, r] of BLOBS) {
    g.moveTo(s / 2 + x * s + (r * s + grow), s / 2 + y * s);
    g.arc(s / 2 + x * s, s / 2 + y * s, r * s + grow, 0, Math.PI * 2);
  }
  g.fill();
}

// 동글: 짙은 테두리 → 몸 → 위쪽 하이라이트 (흰 창 위에서도, 까만 바탕화면에서도 보이게)
function roundSprite(color) {
  let s = roundSprites.get(color);
  if (s) return s;
  const hex = RAINBOW[color];
  s = makeSprite(ROUND_BOX, ROUND_BOX, (g) => {
    const box = ROUND_BOX * 0.86;
    g.translate((ROUND_BOX - box) / 2, (ROUND_BOX - box) / 2);
    g.fillStyle = mix(hex, 0, 0.32);
    blobs(g, 2, box);
    g.fillStyle = hex;
    blobs(g, 0, box);
    g.fillStyle = "rgba(255,255,255,0.55)";
    g.beginPath();
    g.ellipse(box * 0.4, box * 0.36, box * 0.13, box * 0.07, -0.35, 0, Math.PI * 2);
    g.fill();
  });
  roundSprites.set(color, s);
  return s;
}

// 픽셀: 한 칸 = 1px로 그려 두고 정수 배로 늘려 찍음 (테두리 한 칸 포함)
function pixelSprite(color) {
  let s = pixelSprites.get(color);
  if (s) return s;
  const hex = RAINBOW[color];
  const tones = { h: mix(hex, 255, 0.45), b: hex, s: mix(hex, 0, 0.18) };
  const rows = PX_CLOUD.length;
  const cols = PX_CLOUD[0].length;
  const filled = (x, y) => y >= 0 && y < rows && x >= 0 && x < cols && PX_CLOUD[y][x] !== ".";
  s = makeSprite(cols + 2, rows + 2, (g) => {
    g.fillStyle = mix(hex, 0, 0.45);
    for (let y = -1; y <= rows; y++) {
      for (let x = -1; x <= cols; x++) {
        if (filled(x, y)) continue;
        if (filled(x - 1, y) || filled(x + 1, y) || filled(x, y - 1) || filled(x, y + 1)) g.fillRect(x + 1, y + 1, 1, 1);
      }
    }
    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        const ch = PX_CLOUD[y][x];
        if (ch === ".") continue;
        g.fillStyle = tones[ch];
        g.fillRect(x + 1, y + 1, 1, 1);
      }
    }
  });
  pixelSprites.set(color, s);
  return s;
}

// 새 구름들을 받아 그리기 시작함
function addPuffs(list) {
  const now = performance.now();
  for (const p of list || []) {
    if (!(p.age < LIFE) || !(p.size > 0) || !RAINBOW[p.color]) continue;
    puffs.push({ x: p.x, y: p.y, size: p.size, color: p.color, pixel: !!p.pixel, dir: p.dir || 0, born: now - p.age * 1000 });
  }
  if (puffs.length > MAX_PUFFS) puffs = puffs.slice(puffs.length - MAX_PUFFS);
  if (puffs.length && !frameRequest) frameRequest = requestAnimationFrame(frame);
}

// 매 프레임 구름을 그림. 다 사라지면 멈춤
function frame(now) {
  frameRequest = 0;
  if (dirty) ctx.clearRect(dirty[0], dirty[1], dirty[2] - dirty[0], dirty[3] - dirty[1]);
  dirty = null;
  puffs = puffs.filter((p) => now - p.born < LIFE * 1000);
  for (const p of puffs) {
    const t = Math.max(0, (now - p.born) / (LIFE * 1000));
    const grow = 0.6 + 0.5 * (1 - (1 - t) * (1 - t));
    let alpha = t < 0.1 ? t / 0.1 : Math.pow(1 - (t - 0.1) / 0.9, 1.5);
    const cx = p.x + p.dir * DRIFT * t;
    const cy = p.y - RISE * t;
    let box;
    let sprite;
    let w;
    let h;
    if (p.pixel) {
      // 칸 크기는 정수 px, 투명도도 네 계단 — 픽셀답게
      sprite = pixelSprite(p.color);
      const dot = Math.max(2, Math.round((p.size * grow) / sprite.width));
      w = sprite.width * dot;
      h = sprite.height * dot;
      alpha = Math.ceil(alpha * 4) / 4;
      box = [Math.round((cx - w / 2) / dot) * dot, Math.round((cy - h / 2) / dot) * dot];
    } else {
      sprite = roundSprite(p.color);
      w = h = (p.size * grow) / 0.86;
      box = [cx - w / 2, cy - h / 2];
    }
    if (alpha <= 0) continue;
    ctx.imageSmoothingEnabled = !p.pixel;
    ctx.globalAlpha = Math.min(1, alpha * 0.92);
    ctx.drawImage(sprite, box[0], box[1], w, h);
    const [l, tp, r, b] = [Math.floor(box[0]) - 1, Math.floor(box[1]) - 1, Math.ceil(box[0] + w) + 1, Math.ceil(box[1] + h) + 1];
    dirty = dirty ? [Math.min(dirty[0], l), Math.min(dirty[1], tp), Math.max(dirty[2], r), Math.max(dirty[3], b)] : [l, tp, r, b];
  }
  ctx.globalAlpha = 1;
  if (puffs.length) frameRequest = requestAnimationFrame(frame);
}

// 캔버스를 창 크기에 맞춤
function fitCanvas() {
  canvas.width = window.innerWidth;
  canvas.height = window.innerHeight;
  dirty = null;
}

window.addEventListener("resize", fitCanvas);
window.addEventListener("contextmenu", (e) => e.preventDefault());
fitCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 구름만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("trail-puffs", (e) => addPuffs(e.payload));
} else if (new URLSearchParams(location.search).get("preview")) {
  devPreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 무지개 동물이 화면을 가로질러 달림
function devPreview() {
  const q = new URLSearchParams(location.search);
  const pixel = !!q.get("pixel");
  document.documentElement.style.background = q.get("light") ? "#eef1f6" : "#1d2030";
  const ground = Math.round(window.innerHeight * 0.7);
  // 이미 한참 달려온 꼬리 (스크린숏에도 보이게)
  addPuffs(Array.from({ length: 12 }, (_, i) => ({ x: 60 + i * 26, y: ground, size: 38, color: i % RAINBOW.length, pixel, dir: -1, age: (11 - i) * 0.08 })));
  let x = 60 + 12 * 26 + 36;
  let dir = 1;
  let color = 12 % RAINBOW.length;
  const step = () => {
    x += dir * 26;
    if (x > window.innerWidth - 40 || x < 40) dir = -dir;
    addPuffs([{ x: x - dir * 36, y: ground, size: 38, color, pixel, dir: -dir, age: 0 }]);
    color = (color + 1) % RAINBOW.length;
    setTimeout(step, 80);
  };
  step();
}
