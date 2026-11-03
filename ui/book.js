// 🎁 선물 도감 창: 백엔드가 만든 내용(`book_state` 커맨드, 주울 때마다 `book-state` 이벤트 — screenpet_core::gift::BookView)을 그린다.
// 친구 이름이 들어 있으니 글자는 textContent로만 넣는다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

function el(tag, className, text) {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

// 그림 폴더: 픽셀 스타일이면 칸 그림 — 칸(12칸 그림)이 물리 px의 정수배가 되게 크기도 맞춘다
let pixelArt = window.ART_STYLE === "pixel";
let lastView = null;
function artDir() {
  return pixelArt ? "gifts_pixel" : "gifts";
}
function applyArt() {
  document.body.classList[pixelArt ? "add" : "remove"]("pixel");
  const dpr = window.devicePixelRatio || 1;
  document.body.style.setProperty("--gift-side", `${(12 * Math.max(1, Math.round(4 * dpr))) / dpr}px`);
}

function cell(item) {
  const have = item.count > 0 || item.shiny > 0;
  const c = el("div", `cell${have ? "" : " missing"}${item.shiny > 0 ? " shiny" : ""}`);
  const img = el("img");
  img.alt = "";
  img.draggable = false;
  img.src = `${artDir()}/${item.image}.png`;
  c.append(img, el("div", "name", item.name));
  const count = el("div", "count");
  if (item.count > 0) count.append(`×${item.count}`);
  if (item.shiny > 0) {
    if (item.count > 0) count.append(" ");
    count.append(el("b", "", `✨×${item.shiny}`));
  }
  c.append(count);
  if (item.from) {
    const from = el("div", "from", `💌 ${item.from}`);
    from.title = `${item.from}님이 준 선물`;
    c.append(from);
  }
  return c;
}

function show(view) {
  lastView = view;
  document.getElementById("sum").textContent = `모은 선물 ${view.collected} / ${view.total} · ✨ 반짝이 ${view.shiny}`;
  document.getElementById("bar").style.width = `${view.total ? (100 * view.collected) / view.total : 0}%`;
  const root = document.getElementById("sections");
  const scroll = window.scrollY;
  root.textContent = "";
  for (const s of view.sections) {
    const box = el("section", s.collected === s.items.length ? "full" : "");
    const head = el("h2", "", `${s.emoji} ${s.title}`);
    head.append(el("small", "", `${s.collected} / ${s.items.length}`));
    const grid = el("div", "grid");
    grid.append(...s.items.map(cell));
    box.append(head, grid);
    root.append(box);
  }
  window.scrollTo(0, scroll);
}

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) e.preventDefault();
});

(async () => {
  if (pixelArt) applyArt();
  await here.listen("art-style", (e) => {
    pixelArt = e.payload === "pixel";
    applyArt();
    if (lastView) show(lastView);
  });
  await here.listen("book-state", (e) => show(e.payload));
  show(await invoke("book_state"));
})();
