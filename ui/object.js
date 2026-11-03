// 물건 창: 그림 한 장 + 포스트잇. 물리는 앱(코어 object.rs)이 하고 여기는 받은 모습만 CSS로 옮긴다.
// 포스트잇 글자는 받는 사람이 누른 순간에만 `object-note`로 온다 — 그 전엔 이 창에 없고, `object-note-end`면 지운다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 포스트잇 가운데 자리 (128 칸 좌표, tools/draw_objects.py POSTIT_AT과 같다)
const POSTIT_AT = {
  can: [76, 80],
  paper: [78, 84],
  glasses: [40, 100],
  computer: [92, 72],
  banana: [64, 100],
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

thing.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("object_down");
});
thing.addEventListener("contextmenu", () => invoke("object_menu"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());

here.listen("object-pose", ({ payload: p }) => {
  thing.style.left = `calc(50% - 48px + ${p.dx}px)`;
  thing.style.transform = `translateY(${-p.lift}px) rotate(${p.angle}deg) scaleY(${p.flip})`;
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
