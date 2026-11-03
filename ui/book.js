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

function cell(item) {
  const have = item.count > 0 || item.shiny > 0;
  const c = el("div", `cell${have ? "" : " missing"}${item.shiny > 0 ? " shiny" : ""}`);
  const img = el("img");
  img.alt = "";
  img.draggable = false;
  img.src = `gifts/${item.image}.png`;
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
  await here.listen("book-state", (e) => show(e.payload));
  show(await invoke("book_state"));
})();
