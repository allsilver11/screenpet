// 📮 건의함 두 창 (개발자 건의함 mailbox.js · 공개 건의함 board.js)이 같이 쓰는 쪽 넘기기와 날짜.
// 한 쪽에 5개, "‹ 이전  n / m  다음 ›". 글자는 textContent로만 넣는다.
const PER_PAGE = 5;

// 쪽 수 (비어 있어도 1쪽)
function pageCount(total) {
  return Math.max(1, Math.ceil(total / PER_PAGE));
}

// 있는 쪽으로 (지워서 마지막 쪽이 비면 한 쪽 앞으로)
function clampPage(page, total) {
  return Math.min(Math.max(1, Math.floor(page) || 1), pageCount(total));
}

// 그 쪽에 보일 것
function pageSlice(list, page) {
  const p = clampPage(page, list.length);
  return list.slice((p - 1) * PER_PAGE, p * PER_PAGE);
}

// YYYY-MM-DD (내 컴퓨터 시간대)
function ymd(at) {
  const d = new Date(at * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

// root 안에 이전·쪽·다음을 만든다. 누르면 onGo(가고 싶은 쪽)
function makePager(root, onGo) {
  const prev = document.createElement("button");
  prev.textContent = "‹ 이전";
  const label = document.createElement("span");
  label.className = "pages";
  const next = document.createElement("button");
  next.textContent = "다음 ›";
  root.replaceChildren(prev, label, next);
  let cur = 1;
  let pages = 1;
  prev.addEventListener("click", () => cur > 1 && onGo(cur - 1));
  next.addEventListener("click", () => cur < pages && onGo(cur + 1));
  return {
    // shown: 목록이 비었으면 숨긴다. busy: 받아 오는 중엔 누르지 못한다
    set(page, count, shown = true, busy = false) {
      cur = page;
      pages = count;
      label.textContent = `${page} / ${count}`;
      prev.disabled = busy || page <= 1;
      next.disabled = busy || page >= count;
      root.style.display = shown ? "flex" : "none";
    },
  };
}
