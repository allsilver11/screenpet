// 물건 창: 그림 한 장 + 포스트잇. 물리는 앱(코어 object.rs)이 하고 여기는 받은 각도만 CSS로 돌림.
// 그림은 제 무게중심(코어 tumble.rs)을 축으로 돌고, 창은 그 축을 가운데에 둔 채 움직임 — 그래서 어떤 각도에서도 잘리지 않음.
// 포스트잇 글자는 받는 사람이 누른 순간에만 `object-note`로 옴 — 그 전엔 이 창에 없고, `object-note-end`면 지움.
// 📌 작업표시줄에서 꺼낸 앱 아이콘 장난감 (app_icon)은 그림 파일 대신 앱이 초기화 스크립트에 넣어 준 아이콘 칸(base64 RGBA)을 canvas에 그림 —
// 그림은 이 창 안에만 있음 (주소로 만들지도, 어디로 보내지도 않음).
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 포스트잇 가운데 자리 (128 칸 좌표, tools/draw_objects.py POSTIT_AT과 같음)
const POSTIT_AT = {
  can: [76, 80],
  paper: [86, 104],
  stone: [76, 86],
  glasses: [40, 100],
  computer: [92, 72],
  milk: [74, 92],
  shirt: [64, 100],
  umbrella: [46, 96],
};

const info = window.OBJECT || { kind: "can", sticker: false };
const thing = document.getElementById("thing");
const art = document.getElementById("art");
const sticker = document.getElementById("sticker");
const note = document.getElementById("note");

const spot = POSTIT_AT[info.kind] || POSTIT_AT.can;
const SCALE = 96 / 128;
// 그림 크기 배율 (앱이 정함: 쓰레기는 절반, 똥은 싼 동물 크기를 따름). 코어 Object::scale과 같은 값
let DRAW_SCALE = info.scale || 1;
const IS_POOP = ["poop", "pink", "rainbow", "pellets"].includes(info.kind);
// 거대한 돌·똥: 동글 그림은 1024 px로 크게, 픽셀 그림은 칸 수가 적음 (24칸 중 20칸을 잘라 칸 안을 1.2배 채움 — tools/draw_pixel_world.py GIANT_CROP)
const PIXEL_DOTS = { giant_stone: 20, giant_poop: 20, giant_pink: 20, giant_rainbow: 20 };
const DOTS = PIXEL_DOTS[info.kind] || 24;
const IS_GIANT = info.kind in PIXEL_DOTS;
// 📌 장난감: 아이콘을 그릴 칸 (128 칸 좌표: 왼쪽, 위, 한 변 — 코어 tumble.rs AppIcon 모양과 같음), 픽셀 스타일에서 줄여 그릴 칸 수
const IS_TOY = info.kind === "app_icon";
const TOY_BOX = [30, 60, 68];
const TOY_DOTS = 14;
let toy = null;
if (IS_TOY) {
  toy = document.createElement("canvas");
  toy.id = "toy";
  art.style.display = "none";
  thing.insertBefore(toy, art);
}
// 픽셀 똥은 칸이 정수라 동글보다 조금 크게 잡아야 단계 차이가 남
const PIXEL_POOP_DOT = 1.2;
// 축을 아래쪽 정사각형(한 변 side)의 가운데에 놓음 (창이 쪽지로 넓어져도 그대로, objects.rs logical_size)
let side = info.side || 112;
const [rawPivotX, pivotY0] = info.pivot || [48, 72];
let pivotX = rawPivotX;
let pivotY = pivotY0;

// 픽셀 스타일 (objects_pixel/): 칸이 물리 px의 정수배. 돌 때는 90도 계단으로만 그리고 (칸이 무너지지 않게), 구르는 거리·물리는 그대로임
let pixelArt = false;
let dotPx = 3;
let solid = []; // 불투명 칸 가운데 (칸 좌표) — 계단 각도에서도 바닥에 닿게
let lastPose = { angle: 0, dx: 0 };
const POSTIT_DOTS = 6;
const POSTIT_MIN_SCALE = 0.7;

// OBJECT-POSE-START (tools/test_draw_pixel_world.py가 node로 이 부분을 그대로 돌림)
// angle(도)일 때 축에서 가장 낮은 칸까지 (칸) — 코어 tumble.rs drop_at과 같은 계산, 칸 그림 위에서
function dropOf(cells, pivot, angle) {
  const rad = (angle * Math.PI) / 180;
  const [sin, cos] = [Math.sin(rad), Math.cos(rad)];
  let low = -Infinity;
  for (const [x, y] of cells) low = Math.max(low, (x - pivot[0]) * sin + (y - pivot[1]) * cos);
  return low === -Infinity ? 0 : low;
}
// 픽셀 포즈: 가까운 90도 + 진짜 각도에서처럼 바닥에 닿도록 내린 칸 수 (물리가 그 각도의 바닥을 기준으로 축을 놓으니까).
// physics = 물리의 칸(4 논리 px, 동글 96 px 기준) / 그림 칸 — 그림 칸(3 CSS px쯤)이 더 작으니 그만큼 더 내려야 바닥에 닿음
function pixelPose(cells, pivot, angle, physics = 1) {
  const quarter = Math.round(angle / 90) * 90;
  return { quarter, drop: dropOf(cells, pivot, angle) * physics - dropOf(cells, pivot, quarter) };
}
// 칸 한 변·그림 한 변·축 (CSS px): 칸이 물리 px의 정수배. 축(96 논리 px 기준 좌표)도 그림과 같은 비율로, 물리 px에 맞춰서.
// dots = 그림의 칸 수 (보통 24, 거대한 것은 20): 칸 수가 달라도 그림 한 변은 같음 (72 × scale)
function pixelMetrics(dpr, [rawX, rawY], scale = 1, dots = 24) {
  const dot = Math.max(1, Math.round(((3 * scale * 24) / dots) * dpr)) / dpr;
  const k = (dot * dots) / 96;
  const snap = (v) => Math.round(v * k * dpr) / dpr;
  return { dot, side: dots * dot, pivotX: snap(rawX), pivotY: snap(rawY) };
}
// OBJECT-POSE-END

function layout() {
  const pixelScale = IS_GIANT ? DRAW_SCALE : DRAW_SCALE < 1 ? DRAW_SCALE * (IS_POOP ? PIXEL_POOP_DOT : 1) : 1;
  const m = pixelArt ? pixelMetrics(window.devicePixelRatio || 1, [rawPivotX, pivotY0], pixelScale, DOTS) : { dot: 4 * DRAW_SCALE, side: 96 * DRAW_SCALE, pivotX: rawPivotX * DRAW_SCALE, pivotY: pivotY0 * DRAW_SCALE };
  dotPx = m.dot;
  pivotX = m.pivotX;
  pivotY = m.pivotY;
  thing.style.width = pixelArt || DRAW_SCALE !== 1 ? `${m.side}px` : "";
  thing.style.height = pixelArt || DRAW_SCALE !== 1 ? `${m.side}px` : "";
  // 절반 크기여도 눈에 띄도록 쪽지는 그림만큼 줄이지 않음
  const postit = pixelArt ? POSTIT_DOTS * dotPx : 21 * Math.max(DRAW_SCALE, POSTIT_MIN_SCALE);
  // 포스트잇 자리: 96 논리 px 좌표 → 칸 (96 ÷ 24칸 = 칸마다 4)
  const px = pixelArt ? Math.round((spot[0] * SCALE) / 4) * dotPx : spot[0] * SCALE * DRAW_SCALE;
  const py = pixelArt ? Math.round((spot[1] * SCALE) / 4) * dotPx : spot[1] * SCALE * DRAW_SCALE;
  sticker.style.width = `${postit}px`;
  sticker.style.height = `${postit}px`;
  sticker.style.left = `${px - postit / 2}px`;
  sticker.style.top = `${py - postit / 2}px`;
  if (toy) {
    const k = m.side / 128;
    const cell = pixelArt ? m.side / 24 : 0;
    // 픽셀 스타일은 칸에 맞춤
    const snap = (v) => (pixelArt ? Math.round(v / cell) * cell : v);
    toy.style.left = `${snap(TOY_BOX[0] * k)}px`;
    toy.style.top = `${snap(TOY_BOX[1] * k)}px`;
    toy.style.width = `${snap(TOY_BOX[2] * k)}px`;
    toy.style.height = `${snap(TOY_BOX[2] * k)}px`;
  }
  thing.style.transformOrigin = `${pivotX}px ${pivotY}px`;
  thing.style.left = `calc(50% - ${pivotX}px)`;
  thing.style.bottom = `${side / 2 - (m.side - pivotY)}px`;
  note.style.bottom = `${side + 4}px`;
}

// 받은 기울기를 그림에 입힘 (픽셀은 칸에 맞춰 돌림)
function applyPose(p) {
  lastPose = p;
  if (!pixelArt) {
    thing.style.left = `calc(50% - ${pivotX}px + ${p.dx}px)`;
    thing.style.transform = `rotate(${p.angle}deg)`;
    return;
  }
  const dpr = window.devicePixelRatio || 1;
  // 물리의 칸 = 96 × 배율 ÷ 칸 수 (보통 4 × 배율)
  const { quarter, drop } = pixelPose(solid, [pivotX / dotPx, pivotY / dotPx], p.angle, (96 * DRAW_SCALE) / DOTS / dotPx);
  // 창 가운데(홀수 폭이면 반 px)·분수 이동이 칸을 어긋나게 하지 않도록 물리 px에 맞춤
  const left = Math.round((window.innerWidth / 2 - pivotX + p.dx) * dpr) / dpr;
  thing.style.left = `${left}px`;
  thing.style.transform = `translateY(${Math.round(drop * dotPx * dpr) / dpr}px) rotate(${quarter}deg)`;
}

// 불투명 칸을 읽어 둠 (못 읽으면 계단 보정 없이 그림)
function readSolid() {
  try {
    const c = document.createElement("canvas");
    c.width = art.naturalWidth;
    c.height = art.naturalHeight;
    const g = c.getContext("2d", { willReadFrequently: true });
    g.drawImage(art, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    solid = [];
    for (let y = 0; y < c.height; y++) {
      for (let x = 0; x < c.width; x++) {
        if (d[(y * c.width + x) * 4 + 3] > 128) solid.push([x + 0.5, y + 0.5]);
      }
    }
  } catch {
    solid = [];
  }
  applyPose(lastPose);
}

// 📌 장난감 아이콘 칸을 canvas에 (픽셀 스타일은 TOY_DOTS 칸으로 줄여 각지게)
function drawToy(pixel) {
  const icon = info.icon;
  if (!toy || !icon || !icon.size) return;
  const n = icon.size;
  const raw = atob(icon.rgba);
  const cells = new Uint8ClampedArray(n * n * 4);
  for (let i = 0; i < cells.length && i < raw.length; i++) cells[i] = raw.charCodeAt(i);
  const full = document.createElement("canvas");
  full.width = full.height = n;
  full.getContext("2d").putImageData(new ImageData(cells, n, n), 0, 0);
  const side = pixel ? TOY_DOTS : n;
  toy.width = toy.height = side;
  const g = toy.getContext("2d");
  g.imageSmoothingEnabled = true;
  g.imageSmoothingQuality = "high";
  g.clearRect(0, 0, side, side);
  g.drawImage(full, 0, 0, side, side);
  if (pixel) {
    // 반투명 가장자리를 칸 단위로 딱 자름
    const img = g.getImageData(0, 0, side, side);
    for (let i = 3; i < img.data.length; i += 4) img.data[i] = img.data[i] >= 110 ? 255 : 0;
    g.putImageData(img, 0, 0);
  }
  // 픽셀 계단 보정용 불투명 칸 (24칸 그림 좌표): 네모의 네 귀퉁이면 됨
  const [x0, y0, w] = TOY_BOX.map((v) => (v * 24) / 128);
  solid = [[x0, y0], [x0 + w, y0], [x0, y0 + w], [x0 + w, y0 + w]];
}

// 그림 스타일(픽셀·동글)을 바꿈
function applyArt(pixel) {
  pixelArt = pixel;
  document.body.classList.toggle("pixel", pixel);
  if (IS_TOY) {
    drawToy(pixel);
    layout();
    applyPose(lastPose);
    return;
  }
  art.src = `${pixel ? "objects_pixel" : "objects"}/${info.kind}.png`;
  sticker.src = `${pixel ? "objects_pixel" : "objects"}/postit.png`;
  layout();
  applyPose(lastPose);
}
art.addEventListener("load", () => {
  if (pixelArt && !IS_TOY) readSolid();
});
window.addEventListener("resize", () => {
  if (pixelArt) {
    layout();
    applyPose(lastPose);
  }
});
applyArt(window.ART_STYLE === "pixel");
sticker.hidden = !info.sticker;

thing.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("object_down");
});
thing.addEventListener("contextmenu", () => invoke("object_menu"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());

here.listen("object-pose", ({ payload: p }) => applyPose(p));
// 크기 조절 창에서 물건 크기를 바꿨음
here.listen("object-scale", ({ payload: p }) => {
  DRAW_SCALE = p.scale;
  side = p.side;
  layout();
  applyPose(lastPose);
});
here.listen("art-style", (e) => applyArt(e.payload === "pixel"));
here.listen("object-sticker", ({ payload: on }) => {
  sticker.hidden = !on;
});
here.listen("object-note", ({ payload: text }) => {
  note.textContent = text;
  note.hidden = false;
});
here.listen("object-note-end", () => {
  note.textContent = "";
  note.hidden = true;
});
