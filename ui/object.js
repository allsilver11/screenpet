// 물건 창: 그림 한 장 + 포스트잇. 물리는 앱(코어 object.rs)이 하고 여기는 받은 각도만 CSS로 돌린다.
// 그림은 제 무게중심(코어 tumble.rs)을 축으로 돌고, 창은 그 축을 가운데에 둔 채 움직인다 — 그래서 어떤 각도에서도 잘리지 않는다.
// 포스트잇 글자는 받는 사람이 누른 순간에만 `object-note`로 온다 — 그 전엔 이 창에 없고, `object-note-end`면 지운다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 포스트잇 가운데 자리 (128 칸 좌표, tools/draw_objects.py POSTIT_AT과 같다)
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
// 그림 크기 배율 (앱이 정한다: 쓰레기는 절반, 똥은 싼 동물 크기를 따른다). 코어 Object::scale과 같은 값
let DRAW_SCALE = info.scale || 1;
const IS_POOP = ["poop", "pink", "rainbow", "pellets"].includes(info.kind);
// 픽셀 똥은 칸이 정수라 동글보다 조금 크게 잡아야 단계 차이가 난다
const PIXEL_POOP_DOT = 1.2;
// 축을 아래쪽 정사각형(한 변 side)의 가운데에 놓는다 (창이 쪽지로 넓어져도 그대로, objects.rs logical_size)
let side = info.side || 112;
const [rawPivotX, pivotY0] = info.pivot || [48, 72];
let pivotX = rawPivotX;
let pivotY = pivotY0;

// 픽셀 스타일 (objects_pixel/): 칸이 물리 px의 정수배. 돌 때는 90도 계단으로만 그리고 (칸이 무너지지 않게), 구르는 거리·물리는 그대로다
let pixelArt = false;
let dotPx = 3;
let solid = []; // 불투명 칸 가운데 (칸 좌표) — 계단 각도에서도 바닥에 닿게
let lastPose = { angle: 0, dx: 0 };
const POSTIT_DOTS = 6;
const POSTIT_MIN_SCALE = 0.7;

// OBJECT-POSE-START (tools/test_draw_pixel_world.py가 node로 이 부분을 그대로 돌린다)
// angle(도)일 때 축에서 가장 낮은 칸까지 (칸) — 코어 tumble.rs drop_at과 같은 계산, 칸 그림 위에서
function dropOf(cells, pivot, angle) {
  const rad = (angle * Math.PI) / 180;
  const [sin, cos] = [Math.sin(rad), Math.cos(rad)];
  let low = -Infinity;
  for (const [x, y] of cells) low = Math.max(low, (x - pivot[0]) * sin + (y - pivot[1]) * cos);
  return low === -Infinity ? 0 : low;
}
// 픽셀 포즈: 가까운 90도 + 진짜 각도에서처럼 바닥에 닿도록 내린 칸 수 (물리가 그 각도의 바닥을 기준으로 축을 놓으니까).
// physics = 물리의 칸(4 논리 px, 동글 96 px 기준) / 그림 칸 — 그림 칸(3 CSS px쯤)이 더 작으니 그만큼 더 내려야 바닥에 닿는다
function pixelPose(cells, pivot, angle, physics = 1) {
  const quarter = Math.round(angle / 90) * 90;
  return { quarter, drop: dropOf(cells, pivot, angle) * physics - dropOf(cells, pivot, quarter) };
}
// 칸 한 변·그림 한 변·축 (CSS px): 칸이 물리 px의 정수배. 축(96 논리 px 기준 좌표)도 그림과 같은 비율로, 물리 px에 맞춰서
function pixelMetrics(dpr, [rawX, rawY], scale = 1) {
  const dot = Math.max(1, Math.round(3 * scale * dpr)) / dpr;
  const k = dot / 4;
  const snap = (v) => Math.round(v * k * dpr) / dpr;
  return { dot, side: 24 * dot, pivotX: snap(rawX), pivotY: snap(rawY) };
}
// OBJECT-POSE-END

function layout() {
  const m = pixelArt ? pixelMetrics(window.devicePixelRatio || 1, [rawPivotX, pivotY0], DRAW_SCALE < 1 ? DRAW_SCALE * (IS_POOP ? PIXEL_POOP_DOT : 1) : 1) : { dot: 4 * DRAW_SCALE, side: 96 * DRAW_SCALE, pivotX: rawPivotX * DRAW_SCALE, pivotY: pivotY0 * DRAW_SCALE };
  dotPx = m.dot;
  pivotX = m.pivotX;
  pivotY = m.pivotY;
  thing.style.width = pixelArt || DRAW_SCALE !== 1 ? `${m.side}px` : "";
  thing.style.height = pixelArt || DRAW_SCALE !== 1 ? `${m.side}px` : "";
  // 절반 크기여도 눈에 띄도록 쪽지는 그림만큼 줄이지 않는다
  const postit = pixelArt ? POSTIT_DOTS * dotPx : 21 * Math.max(DRAW_SCALE, POSTIT_MIN_SCALE);
  // 포스트잇 자리: 96 논리 px 좌표 → 칸 (96 ÷ 24칸 = 칸마다 4)
  const px = pixelArt ? Math.round((spot[0] * SCALE) / 4) * dotPx : spot[0] * SCALE * DRAW_SCALE;
  const py = pixelArt ? Math.round((spot[1] * SCALE) / 4) * dotPx : spot[1] * SCALE * DRAW_SCALE;
  sticker.style.width = `${postit}px`;
  sticker.style.height = `${postit}px`;
  sticker.style.left = `${px - postit / 2}px`;
  sticker.style.top = `${py - postit / 2}px`;
  thing.style.transformOrigin = `${pivotX}px ${pivotY}px`;
  thing.style.left = `calc(50% - ${pivotX}px)`;
  thing.style.bottom = `${side / 2 - (m.side - pivotY)}px`;
  note.style.bottom = `${side + 4}px`;
}

function applyPose(p) {
  lastPose = p;
  if (!pixelArt) {
    thing.style.left = `calc(50% - ${pivotX}px + ${p.dx}px)`;
    thing.style.transform = `rotate(${p.angle}deg)`;
    return;
  }
  const dpr = window.devicePixelRatio || 1;
  const { quarter, drop } = pixelPose(solid, [pivotX / dotPx, pivotY / dotPx], p.angle, (4 * DRAW_SCALE) / dotPx);
  // 창 가운데(홀수 폭이면 반 px)·분수 이동이 칸을 어긋나게 하지 않도록 물리 px에 맞춘다
  const left = Math.round((window.innerWidth / 2 - pivotX + p.dx) * dpr) / dpr;
  thing.style.left = `${left}px`;
  thing.style.transform = `translateY(${Math.round(drop * dotPx * dpr) / dpr}px) rotate(${quarter}deg)`;
}

// 불투명 칸을 읽어 둔다 (못 읽으면 계단 보정 없이 그린다)
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

function applyArt(pixel) {
  pixelArt = pixel;
  document.body.classList.toggle("pixel", pixel);
  art.src = `${pixel ? "objects_pixel" : "objects"}/${info.kind}.png`;
  sticker.src = `${pixel ? "objects_pixel" : "objects"}/postit.png`;
  layout();
  applyPose(lastPose);
}
art.addEventListener("load", () => {
  if (pixelArt) readSolid();
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
// 크기 조절 창에서 물건 크기를 바꿨다
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
