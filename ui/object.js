// 물건 창: 그림 한 장 + 포스트잇. 물리는 앱(코어 object.rs)이 하고 여기는 받은 각도만 CSS로 돌린다.
// 그림은 제 무게중심(코어 tumble.rs)을 축으로 돌고, 창은 그 축을 가운데에 둔 채 움직인다 — 그래서 어떤 각도에서도 잘리지 않는다.
// 포스트잇 글자는 받는 사람이 누른 순간에만 `object-note`로 온다 — 그 전엔 이 창에 없고, `object-note-end`면 지운다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 포스트잇 가운데 자리 (128 칸 좌표, tools/draw_objects.py POSTIT_AT과 같다)
const POSTIT_AT = {
  can: [76, 80],
  paper: [78, 84],
  glasses: [40, 100],
  computer: [92, 72],
  milk: [74, 92],
  sock: [58, 76],
  umbrella: [44, 66],
};

const info = window.OBJECT || { kind: "can", sticker: false };
const thing = document.getElementById("thing");
const art = document.getElementById("art");
const sticker = document.getElementById("sticker");
const note = document.getElementById("note");

art.src = `objects/${info.kind}.png`;
const spot = POSTIT_AT[info.kind] || POSTIT_AT.can;
const SCALE = 96 / 128;
sticker.style.left = `${spot[0] * SCALE - 10.5}px`;
sticker.style.top = `${spot[1] * SCALE - 10.5}px`;
sticker.hidden = !info.sticker;

// 축을 아래쪽 정사각형(한 변 side)의 가운데에 놓는다 (창이 쪽지로 넓어져도 그대로, objects.rs logical_size)
const side = info.side || 112;
const [pivotX, pivotY] = info.pivot || [48, 72];
thing.style.transformOrigin = `${pivotX}px ${pivotY}px`;
thing.style.left = `calc(50% - ${pivotX}px)`;
thing.style.bottom = `${side / 2 - (96 - pivotY)}px`;
note.style.bottom = `${side + 4}px`;

thing.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("object_down");
});
thing.addEventListener("contextmenu", () => invoke("object_menu"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());

here.listen("object-pose", ({ payload: p }) => {
  thing.style.left = `calc(50% - ${pivotX}px + ${p.dx}px)`;
  thing.style.transform = `rotate(${p.angle}deg)`;
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
