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
// 📌 장난감: 아이콘을 그릴 칸 (128 칸 좌표: 왼쪽, 위, 한 변 — 코어 tumble.rs AppIcon 모양과 같음)
const IS_TOY = info.kind === "app_icon";
const TOY_BOX = [30, 60, 68];
let toy = null;
if (IS_TOY) {
  toy = document.createElement("canvas");
  toy.id = "toy";
  art.style.display = "none";
  thing.insertBefore(toy, art);
}
// 축을 아래쪽 정사각형(한 변 side)의 가운데에 놓음 (창이 쪽지로 넓어져도 그대로, objects.rs logical_size)
let side = info.side || 112;
const [rawPivotX, pivotY0] = info.pivot || [48, 72];
let pivotX = rawPivotX;
let pivotY = pivotY0;
let lastPose = { angle: 0, dx: 0 };
const POSTIT_MIN_SCALE = 0.7;

// 그림·축·포스트잇·장난감 칸을 지금 크기 배율로 놓음
function layout() {
  const m = { side: 96 * DRAW_SCALE, pivotX: rawPivotX * DRAW_SCALE, pivotY: pivotY0 * DRAW_SCALE };
  pivotX = m.pivotX;
  pivotY = m.pivotY;
  thing.style.width = DRAW_SCALE !== 1 ? `${m.side}px` : "";
  thing.style.height = DRAW_SCALE !== 1 ? `${m.side}px` : "";
  // 절반 크기여도 눈에 띄도록 쪽지는 그림만큼 줄이지 않음
  const postit = 21 * Math.max(DRAW_SCALE, POSTIT_MIN_SCALE);
  const px = spot[0] * SCALE * DRAW_SCALE;
  const py = spot[1] * SCALE * DRAW_SCALE;
  sticker.style.width = `${postit}px`;
  sticker.style.height = `${postit}px`;
  sticker.style.left = `${px - postit / 2}px`;
  sticker.style.top = `${py - postit / 2}px`;
  if (toy) {
    const k = m.side / 128;
    toy.style.left = `${TOY_BOX[0] * k}px`;
    toy.style.top = `${TOY_BOX[1] * k}px`;
    toy.style.width = `${TOY_BOX[2] * k}px`;
    toy.style.height = `${TOY_BOX[2] * k}px`;
  }
  thing.style.transformOrigin = `${pivotX}px ${pivotY}px`;
  thing.style.left = `calc(50% - ${pivotX}px)`;
  thing.style.bottom = `${side / 2 - (m.side - pivotY)}px`;
  note.style.bottom = `${side + 4}px`;
}

// 받은 기울기를 그림에 입힘
function applyPose(p) {
  lastPose = p;
  thing.style.left = `calc(50% - ${pivotX}px + ${p.dx}px)`;
  thing.style.transform = `rotate(${p.angle}deg)`;
}

// 📌 장난감 아이콘 칸을 canvas에
function drawToy() {
  const icon = info.icon;
  if (!toy || !icon || !icon.size) return;
  const n = icon.size;
  const raw = atob(icon.rgba);
  const cells = new Uint8ClampedArray(n * n * 4);
  for (let i = 0; i < cells.length && i < raw.length; i++) cells[i] = raw.charCodeAt(i);
  toy.width = toy.height = n;
  toy.getContext("2d").putImageData(new ImageData(cells, n, n), 0, 0);
}

if (IS_TOY) drawToy();
else {
  art.src = `objects/${info.kind}.png`;
  sticker.src = "objects/postit.png";
}
layout();
applyPose(lastPose);
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
