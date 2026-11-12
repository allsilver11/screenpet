// 📮 건의함 보기 (누구나, 읽기 전용): 개발자에게 온 건의의 공개 목록. 이름은 첫 글자만 (김**), 내용은 첫 줄만,
// 날짜와 ✅ 반영됨 / 🙅 반려됨. 읽음·지우기·막기·반영 같은 버튼은 없다. 가린 이름과 첫 줄은 개발자 앱이 만들어
// 릴레이에 올린 것 — 여기서는 textContent로만 넣는다 (HTML로 해석하지 않는다).
// 쪽은 릴레이가 나눠 준다: board_list({page}) → `board-page` {ok, reason, items, page, pages, total}.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const listEl = document.getElementById("list");
const emptyEl = document.getElementById("empty");
let page = 1;
let pages = 1;
const pager = makePager(document.getElementById("pager"), (p) => ask(p));

// 못 받은 이유 → 한 줄
function reasonText(reason) {
  switch (reason) {
    case "offline":
      return "인터넷 친구 서버에 연결되어 있지 않아요";
    case "old":
      return "서버가 아직 준비되지 않았어요";
    default:
      return "지금은 건의함을 볼 수 없어요 — 잠시 뒤에 다시 열어 주세요";
  }
}

function statusBadge(status) {
  if (status === "done") return el("span", "badge done", "✅ 반영됨");
  if (status === "rejected") return el("span", "badge rejected", "🙅 반려됨");
  return null;
}

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function say(text) {
  listEl.replaceChildren();
  emptyEl.textContent = text;
  emptyEl.style.display = "block";
}

function ask(p) {
  page = p;
  emptyEl.textContent = "불러오는 중…";
  emptyEl.style.display = listEl.childElementCount ? "none" : "block";
  pager.set(page, pages, listEl.childElementCount > 0, true);
  invoke("board_list", { page: p });
}

function render(v) {
  if (!v.ok) {
    pager.set(1, 1, false);
    say(reasonText(v.reason));
    return;
  }
  page = v.page;
  pages = v.pages;
  if (!v.items.length) {
    pager.set(1, 1, false);
    say("아직 올라온 건의가 없어요");
    return;
  }
  emptyEl.style.display = "none";
  listEl.replaceChildren();
  for (const it of v.items) {
    const box = el("div", "letter");
    const meta = el("div", "meta");
    const right = el("span", "");
    const badge = statusBadge(it.status);
    if (badge) right.append(badge, " ");
    right.append(ymd(it.at));
    meta.append(el("span", "who", `💌 ${it.name || "누군가"}`), right);
    box.append(meta, el("div", "text", it.line));
    listEl.append(box);
  }
  pager.set(page, pages, true);
}

document.getElementById("close").addEventListener("click", () => invoke("board_close"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("board_close");
  else if (e.key === "ArrowLeft" && page > 1) ask(page - 1);
  else if (e.key === "ArrowRight" && page < pages) ask(page + 1);
});

(async () => {
  await here.listen("board-page", (e) => render(e.payload));
  // 열 때마다 첫 쪽부터 새로
  await here.listen("board-open", () => ask(1));
  ask(1);
})();
