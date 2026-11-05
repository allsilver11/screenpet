// 💩 똥피하기 창: 백엔드(src-tauri/src/dodge.rs)가 보낸 판(`dodge_state` 커맨드, 매 틱 `dodge-state` 이벤트)을 그리고,
// 이 창이 받은 키를 `dodge_keys`로 보낸다 (게임 동안 키는 이 창에만 — 다른 앱에 가지 않는다). 친구 이름은 textContent로만.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const PIXEL = window.ART_STYLE === "pixel";
const POOP_SRC = `${PIXEL ? "objects_pixel" : "objects"}/poop.png`;
// 그림 한 변 = 반지름 × 이만큼 (그림 둘레의 빈 곳까지)
const POOP_DRAW = 2.4;
// 키 → 비트 (screenpet_core::brain::control_moves::ControlInput::bits)
const KEY_BITS = { ArrowLeft: 1, ArrowRight: 2, ArrowUp: 4, ArrowDown: 8, " ": 16, Shift: 32 };

if (PIXEL) document.body.classList.add("pixel");

const $ = (id) => document.getElementById(id);
const shown = new Map(); // 똥 id → img
let phase = "";
let held = 0;

function el(tag, className, text) {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

function drawPoops(poops) {
  const keep = new Set();
  for (const [id, x, y, r] of poops) {
    keep.add(id);
    let img = shown.get(id);
    if (!img) {
      img = el("img");
      img.src = POOP_SRC;
      img.alt = "";
      shown.set(id, img);
      $("field").append(img);
    }
    const side = r * POOP_DRAW;
    img.style.width = img.style.height = `${side}px`;
    img.style.left = `${x - side / 2}px`;
    img.style.top = `${y - side / 2}px`;
  }
  for (const [id, img] of shown) {
    if (!keep.has(id)) {
      img.remove();
      shown.delete(id);
    }
  }
}

function drawRanks(ranking) {
  const list = $("ranks");
  list.textContent = "";
  ranking.forEach((r, i) => {
    const li = el("li", r.me ? "me" : "");
    li.append(el("span", "", `${i + 1}`), el("span", "n", r.name), el("span", "s", `${r.score}`));
    list.append(li);
  });
}

function show(view) {
  phase = view.phase || "ready";
  const playing = phase === "play";
  // 끝난 판은 마지막 자리 그대로 둔다 (어디서 맞았는지 보이게), 새 판이면 비운다
  drawPoops(view.poops || []);
  $("hud").hidden = !playing;
  $("hud").textContent = `💩 ${view.dodged || 0}`;
  $("card").hidden = playing;
  if (playing) return;
  const over = phase === "over";
  $("score").hidden = !over;
  $("score").textContent = `${view.dodged || 0}개 피함`;
  $("record").hidden = !(over && view.new_best);
  $("keys").hidden = over;
  $("start").textContent = over ? "다시 (Space)" : "시작 (Space)";
  drawRanks(view.ranking || []);
}

function sendKeys(bits) {
  if (bits === held) return;
  held = bits;
  invoke("dodge_keys", { bits }).catch(() => {});
}

function start() {
  if (phase !== "play") invoke("dodge_start").catch(() => {});
}

function close() {
  invoke("dodge_close").catch(() => {});
}

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    close();
    return;
  }
  const bit = KEY_BITS[e.key];
  if (bit === undefined) return;
  e.preventDefault();
  if (e.key === " " && !e.repeat && phase !== "play") {
    start();
    return;
  }
  sendKeys(held | bit);
});
window.addEventListener("keyup", (e) => {
  const bit = KEY_BITS[e.key];
  if (bit === undefined) return;
  e.preventDefault();
  sendKeys(held & ~bit);
});
// 다른 창으로 가면 키를 다 놓은 것으로
window.addEventListener("blur", () => sendKeys(0));
$("start").addEventListener("click", start);
$("close").addEventListener("click", close);

(async () => {
  await here.listen("dodge-state", (e) => show(e.payload));
  show(await invoke("dodge_state"));
})();
