// 🖍️ 분필 낙서 그리기. 흐물이가 바닥에 그린 하트·웃는 얼굴·별·이름을 🌈 무지개 구름과 같은 창 (trail.html)의
// 아래 캔버스에 그림. 앱(src-tauri/src/trail.rs)이 `trail-doodle`로 새 낙서들을 보냄 (논리 px, 이 창 왼쪽 위가 0,0 —
// x는 가운데, y는 바닥 줄, size는 높이, kind는 heart·smile·star·name, name은 펫 이름, facing은 바라보는 쪽, age는 이미 지난 초).
// draw초 (앱이 흐물이 그리기 동작 길이 DOODLE_SECS를 보냄, 없으면 DRAW) 동안 분필로 그려지고, HOLD초 그대로 있다가, FADE초 동안 흐려짐. CPU를 아끼는 규칙: 낙서 MAX_DOODLES개까지,
// 분필 가루는 낙서마다 따로 그림에 한 번만 찍기, 그리거나 흐려지는 동안만 requestAnimationFrame, 가만히 있는 동안은 타이머 하나.
// 이름은 캔버스 fillText로만 넣음 (HTML로 넣지 않음), MAX_NAME글자까지.
// 브라우저에서 trail.html?doodle=1 (&pixel=1, &light=1)로 앱 없이 볼 수 있음.

const DRAW = 2.0;
// 받아들이는 그리기 시간 (초) — 밖이면 DRAW
const DRAW_RANGE = [0.3, 5];
const HOLD = 20;
const FADE = 3;
const MAX_DOODLES = 6;
const MAX_NAME = 12;
const KINDS = ["heart", "smile", "star", "name"];
// 파스텔 분필 (이름은 흰 분필)
const CHALK = { heart: "#ffc4d6", smile: "#fff0a0", star: "#bfe4ff", name: "#f7f5ee" };
// 밝은 창 위에서도 보이게 깔리는 그늘
const SHADE = "rgba(30,32,48,0.5)";
const FONT = '"Segoe Print", "Chalkboard SE", "Comic Sans MS", "Nanum Pen Script", "Malgun Gothic", "Apple SD Gothic Neo", cursive, sans-serif';

const doodleCanvas = document.getElementById("doodle");
const dctx = doodleCanvas.getContext("2d");
let doodles = [];
let doodleFrame = 0;
let doodleTimer = 0;
let doodleDirty = null;

// 낙서마다 같은 가루가 나오게 하는 작은 난수
function seeded(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// 모양: 단위 상자 (x -0.5..0.5, y -1..0 — 0이 바닥) 안의 선들
function shapeLines(kind) {
  const lines = [];
  if (kind === "heart") {
    const pts = [];
    for (let i = 0; i <= 64; i++) {
      const t = (i / 64) * Math.PI * 2;
      const x = 16 * Math.pow(Math.sin(t), 3);
      const y = 13 * Math.cos(t) - 5 * Math.cos(2 * t) - 2 * Math.cos(3 * t) - Math.cos(4 * t);
      pts.push([x / 36, -0.5 - y / 36]);
    }
    lines.push(pts);
  } else if (kind === "smile") {
    const ring = [];
    for (let i = 0; i <= 56; i++) {
      const t = -Math.PI / 2 + (i / 56) * Math.PI * 2.04;
      ring.push([0.44 * Math.cos(t), -0.5 + 0.44 * Math.sin(t)]);
    }
    lines.push(ring);
    lines.push([[-0.16, -0.66], [-0.15, -0.58]]);
    lines.push([[0.15, -0.66], [0.16, -0.58]]);
    const mouth = [];
    for (let i = 0; i <= 16; i++) {
      const t = Math.PI * (0.18 + (0.64 * i) / 16);
      mouth.push([0.24 * Math.cos(t), -0.48 + 0.2 * Math.sin(t)]);
    }
    lines.push(mouth);
  } else if (kind === "star") {
    const pts = [];
    for (let i = 0; i <= 10; i++) {
      const t = -Math.PI / 2 + (i * Math.PI) / 5;
      const r = i % 2 ? 0.2 : 0.48;
      pts.push([r * Math.cos(t), -0.48 + r * Math.sin(t)]);
    }
    lines.push(pts);
  }
  return lines;
}

// 선들을 일정한 간격의 점으로 (분필이 지나가는 자리). 길이 = 점 수
function walk(lines, w, h, flip, step) {
  const out = [];
  for (const line of lines) {
    for (let i = 1; i < line.length; i++) {
      const [ax, ay] = line[i - 1];
      const [bx, by] = line[i];
      const x0 = ax * flip * w;
      const y0 = ay * h;
      const x1 = bx * flip * w;
      const y1 = by * h;
      const n = Math.max(1, Math.ceil(Math.hypot(x1 - x0, y1 - y0) / step));
      for (let k = 0; k < n; k++) out.push([x0 + ((x1 - x0) * k) / n, y0 + ((y1 - y0) * k) / n]);
    }
  }
  return out;
}

// w×h 캔버스를 만듦 (최소 1px)
function newCanvas(w, h) {
  const c = document.createElement("canvas");
  c.width = Math.max(1, Math.ceil(w));
  c.height = Math.max(1, Math.ceil(h));
  return c;
}

// 이름을 분필 글자로 (동글: 글자 위를 가루로 깎음, 픽셀: 작게 써서 칸으로 끊음)
function nameSprite(d) {
  const text = Array.from(String(d.name || "")).slice(0, MAX_NAME).join("");
  const px = d.pixel ? 11 : Math.max(12, d.size * 0.5);
  const probe = newCanvas(1, 1).getContext("2d");
  probe.font = `${px}px ${FONT}`;
  const width = (probe.measureText(text) || {}).width || px * Array.from(text).length;
  const c = newCanvas(width + px, px * 1.6);
  const g = c.getContext("2d");
  g.font = `bold ${px}px ${FONT}`;
  g.textBaseline = "middle";
  if (d.pixel) {
    // 반쯤 넘게 칠해진 칸만 남기고, 둘레 한 칸에 그늘
    g.fillStyle = "#fff";
    g.fillText(text, px / 2, c.height / 2);
    const img = g.getImageData(0, 0, c.width, c.height);
    if (img && img.data) {
      g.clearRect(0, 0, c.width, c.height);
      const on = (x, y) => y >= 0 && y < c.height && img.data[(y * c.width + x) * 4 + 3] > 110;
      for (const [tone, dy] of [[SHADE, 1], [CHALK.name, 0]]) {
        g.fillStyle = tone;
        for (let y = 0; y < c.height; y++) {
          for (let x = 0; x < c.width; x++) {
            const lit = dy === 0 ? on(x, y) : !on(x, y) && (on(x, y - 1) || on(x - 1, y) || on(x + 1, y) || on(x, y + 1));
            if (lit) g.fillRect(x, y, 1, 1);
          }
        }
      }
    }
  } else {
    // 밝은 창 위에서도 읽히게 둘레에 그늘
    g.strokeStyle = SHADE;
    g.lineWidth = Math.max(2, px / 9);
    g.lineJoin = "round";
    g.strokeText(text, px / 2 + 0.5, c.height / 2 + 1);
    g.fillStyle = CHALK.name;
    g.fillText(text, px / 2, c.height / 2);
    // 분필 결: 글자 곳곳을 작게 지움
    const rnd = seeded(d.seed);
    g.globalCompositeOperation = "destination-out";
    const holes = Math.round((c.width * c.height) / 18);
    for (let i = 0; i < holes; i++) {
      g.globalAlpha = 0.35 + rnd() * 0.6;
      g.fillRect(rnd() * c.width, rnd() * c.height, 1 + rnd() * 1.5, 1);
    }
    g.globalAlpha = 1;
    g.globalCompositeOperation = "source-over";
  }
  return c;
}

// 낙서 하나를 그릴 준비를 함 (이름이면 글자 그림을 만듦)
function prepare(d) {
  if (d.kind === "name") {
    const text = nameSprite(d);
    const dot = d.pixel ? Math.max(2, Math.round(d.size / 20)) : 1;
    d.text = text;
    d.w = text.width * dot;
    d.h = text.height * dot;
    d.dot = dot;
    return;
  }
  // 동글: 실제 크기로, 픽셀: 한 칸 = 1px로 그려 두고 정수 배로 늘림
  const dot = d.pixel ? Math.max(2, Math.round(d.size / 18)) : 1;
  const h = d.size / dot;
  const w = h;
  const pad = d.pixel ? 2 : 4;
  d.dot = dot;
  d.pad = pad;
  d.chalk = newCanvas(w + pad * 2, h + pad * 2);
  d.shade = newCanvas(w + pad * 2, h + pad * 2);
  d.points = walk(shapeLines(d.kind), w, h, d.facing < 0 ? -1 : 1, d.pixel ? 1 : 0.8);
  d.drawn = 0;
  d.w = d.chalk.width * dot;
  d.h = d.chalk.height * dot;
  d.rnd = seeded(d.seed);
}

// 분필 가루를 points[d.drawn..upto]에 찍음 (한 번 찍은 자리는 다시 찍지 않음)
function stamp(d, upto) {
  if (!d.points) return;
  const c = d.chalk.getContext("2d");
  const s = d.shade.getContext("2d");
  const ox = d.chalk.width / 2;
  const oy = d.chalk.height - d.pad;
  c.fillStyle = CHALK[d.kind];
  s.fillStyle = SHADE;
  const thick = d.pixel ? 0 : Math.max(1.6, d.size / 26);
  for (let i = d.drawn; i < upto; i++) {
    const [x, y] = d.points[i];
    if (d.pixel) {
      // 칸 하나, 가끔 빠진 칸 (분필이 덜 묻은 자리)
      if (d.rnd() < 0.12) continue;
      const gx = Math.round(x + ox);
      const gy = Math.round(y + oy);
      c.fillRect(gx, gy, 1, 1);
      s.fillRect(gx, gy + 1, 1, 1);
      continue;
    }
    for (let k = 0; k < 3; k++) {
      const jx = (d.rnd() - 0.5) * thick * 1.4;
      const jy = (d.rnd() - 0.5) * thick * 1.4;
      const r = 0.6 + d.rnd() * thick * 0.6;
      c.globalAlpha = 0.45 + d.rnd() * 0.55;
      c.fillRect(x + ox + jx - r / 2, y + oy + jy - r / 2, r, r);
      s.fillRect(x + ox + jx - r / 2 + 0.8, y + oy + jy - r / 2 + 1.4, r, r);
    }
  }
  c.globalAlpha = 1;
  d.drawn = Math.max(d.drawn, upto);
}

// 새 낙서들을 받아 그리기 시작함
function addDoodles(list) {
  const now = performance.now();
  for (const p of list || []) {
    const draw = p.draw >= DRAW_RANGE[0] && p.draw <= DRAW_RANGE[1] ? p.draw : DRAW;
    if (!KINDS.includes(p.kind) || !(p.age >= 0 && p.age < draw + HOLD + FADE) || !(p.size > 0 && p.size < 2000)) continue;
    if (!Number.isFinite(p.x) || !Number.isFinite(p.y)) continue;
    const d = { x: p.x, y: p.y, kind: p.kind, name: p.name, facing: p.facing < 0 ? -1 : 1, size: p.size, pixel: !!p.pixel, draw, born: now - p.age * 1000, seed: (Math.random() * 4294967296) >>> 0 };
    prepare(d);
    doodles.push(d);
  }
  if (doodles.length > MAX_DOODLES) doodles = doodles.slice(doodles.length - MAX_DOODLES);
  wake();
}

// 그리기 타이머를 지우고 다음 프레임을 바로 그리게 함
function wake() {
  if (doodleTimer) clearTimeout(doodleTimer);
  doodleTimer = 0;
  if (doodles.length && !doodleFrame) doodleFrame = requestAnimationFrame(drawDoodles);
}

// 낙서들을 그림. 다 사라지면 멈춤
function drawDoodles(now) {
  doodleFrame = 0;
  if (doodleDirty) dctx.clearRect(doodleDirty[0], doodleDirty[1], doodleDirty[2] - doodleDirty[0], doodleDirty[3] - doodleDirty[1]);
  doodleDirty = null;
  doodles = doodles.filter((d) => now - d.born < (d.draw + HOLD + FADE) * 1000);
  let moving = false;
  let next = Infinity;
  for (const d of doodles) {
    const t = (now - d.born) / 1000;
    const drawn = Math.min(1, t / d.draw);
    let alpha = t < d.draw + HOLD ? 1 : Math.max(0, 1 - (t - d.draw - HOLD) / FADE);
    if (drawn < 1 || t >= d.draw + HOLD) moving = true;
    else next = Math.min(next, d.draw + HOLD - t);
    if (d.pixel) alpha = Math.ceil(alpha * 4) / 4;
    if (alpha <= 0) continue;
    // 바닥 줄 위에, x가 가운데
    const left = Math.round(d.x - d.w / 2);
    const top = Math.round(d.y - d.h + (d.kind === "name" ? d.h * 0.25 : d.pad * d.dot));
    dctx.imageSmoothingEnabled = !d.pixel;
    dctx.globalAlpha = alpha;
    if (d.text) {
      // 글자는 왼쪽부터 써 나감
      const sw = Math.max(1, Math.round(d.text.width * drawn));
      dctx.drawImage(d.text, 0, 0, sw, d.text.height, left, top, sw * d.dot, d.h);
    } else {
      stamp(d, Math.round(d.points.length * drawn));
      dctx.drawImage(d.shade, left, top, d.w, d.h);
      dctx.drawImage(d.chalk, left, top, d.w, d.h);
    }
    const box = [left - 2, top - 2, left + d.w + 4, top + d.h + 4];
    doodleDirty = doodleDirty ? [Math.min(doodleDirty[0], box[0]), Math.min(doodleDirty[1], box[1]), Math.max(doodleDirty[2], box[2]), Math.max(doodleDirty[3], box[3])] : box;
  }
  dctx.globalAlpha = 1;
  if (!doodles.length && !doodleDirty) return;
  if (moving || !doodles.length) {
    doodleFrame = requestAnimationFrame(drawDoodles);
  } else {
    // 다 그려져 가만히 있음: 다음 흐려지기까지 잠
    doodleTimer = setTimeout(() => {
      doodleTimer = 0;
      wake();
    }, Math.ceil(next * 1000) + 5);
  }
}

// 낙서 캔버스를 창 크기에 맞춤
function fitDoodleCanvas() {
  doodleCanvas.width = window.innerWidth;
  doodleCanvas.height = window.innerHeight;
  doodleDirty = null;
  wake();
}

window.addEventListener("resize", fitDoodleCanvas);
fitDoodleCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 낙서만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("trail-doodle", (e) => addDoodles(e.payload));
} else if (new URLSearchParams(location.search).get("doodle")) {
  doodlePreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 네 가지 낙서, 다 그려진 것과 그리는 중인 것
function doodlePreview() {
  const q = new URLSearchParams(location.search);
  const pixel = !!q.get("pixel");
  document.documentElement.style.background = q.get("light") ? "#eef1f6" : "#1d2030";
  const floor = Math.round(window.innerHeight * 0.75);
  const name = q.get("name") || "흐물이";
  const ages = [5, 5, 5, 5, 0.9];
  const kinds = ["heart", "smile", "star", "name", "star"];
  addDoodles(kinds.map((kind, i) => ({ x: 90 + i * 150, y: floor, kind, name, facing: i % 2 ? -1 : 1, size: 64, pixel, age: ages[i] })));
}
