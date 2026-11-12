// 📮 개발자 건의함 (개발자 비밀키가 있는 컴퓨터만 — spec 2026-11-10-feedback-mailbox §5, 예전 "📬 편지함").
// 목록은 `mailbox-state`([{id, at, from, read, done, rejected, ok, text, name, species, stage, style, version, os}], 최신순)로 온다.
// 보낸 사람이 쓴 글은 모두 textContent로만 넣는다 (HTML로 해석하지 않는다). 열면 읽음, ✅ 반영함 / 🙅 반려 (하나만), 지우기, 이 사람 막기.
// 공개 건의함(board.js)과 같은 쪽 넘기기 (pager.js, 5개씩): 읽거나 지우거나 막아도 보던 쪽에 남고, 그 쪽이 비면 한 쪽 앞으로.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const listEl = document.getElementById("list");
const emptyEl = document.getElementById("empty");
let letters = [];
let page = 1;
const pager = makePager(document.getElementById("pager"), (p) => {
  page = p;
  render();
});

const PET_EMOJI = { cat: "🐱", persian: "🐱", rabbit: "🐰", frog: "🐸", duck: "🦆", bird: "🐦", penguin: "🐧", squirrel: "🐿️", redpanda: "🐾", heumul: "🤸", giraffe: "🦒", pumpkin: "🎃", ghost: "👻", santa: "🎅", rudolph: "🦌", snowman: "⛄" };

function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function when(at) {
  const d = new Date(at * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}.${p(d.getMonth() + 1)}.${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

// 새 목록이 와도 보던 쪽 그대로 (비었으면 clampPage가 한 쪽 앞으로)
function update(next) {
  letters = next;
  render();
}

function render() {
  page = clampPage(page, letters.length);
  listEl.replaceChildren();
  emptyEl.style.display = letters.length ? "none" : "block";
  pager.set(page, pageCount(letters.length), letters.length > 0);
  for (const l of pageSlice(letters, page)) {
    const box = el("div", ["letter", l.read ? "" : "unread", l.done ? "done" : "", l.rejected ? "rejected" : ""].filter(Boolean).join(" "));
    const meta = el("div", "meta");
    const who = `${PET_EMOJI[l.species] || "💌"} ${l.name || "누군가"}`;
    meta.append(el("span", "who", who), el("span", "", `${when(l.at)} · v${l.version || "?"} · ${l.os || "?"}`));
    const text = el("div", "text", l.text);
    const row = el("div", "row");
    const del = el("button", "", "지우기");
    del.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("mailbox_delete", { id: l.id });
    });
    const block = el("button", "", "이 사람 막기");
    // 두 번 눌러야 막는다 (맥 웹뷰는 confirm 창을 띄우지 않을 수 있다): 처음엔 "정말 막을까요?"로 바뀌고 3초 안에 한 번 더
    let armed = null;
    block.addEventListener("click", (e) => {
      e.stopPropagation();
      if (armed) {
        clearTimeout(armed);
        invoke("mailbox_block", { from: l.from });
        return;
      }
      block.textContent = "정말 막을까요? (받은 편지도 지워요)";
      armed = setTimeout(() => {
        armed = null;
        block.textContent = "이 사람 막기";
      }, 3000);
    });
    // ✅ 반영함 / 🙅 반려: 하나만 (하나를 켜면 다른 하나가 꺼지고, 켠 것을 끄면 아무것도 아니다).
    // 릴레이에 저장돼 두 컴퓨터가 같고, 📮 공개 건의함에도 ✅ 반영됨 / 🙅 반려됨으로 보인다
    const status = el("span", "status");
    const checkbox = (text, on) => {
      const label = el("label");
      const box = el("input");
      box.type = "checkbox";
      box.checked = !!on;
      box.addEventListener("click", (e) => e.stopPropagation());
      label.append(box, el("span", "", text));
      label.addEventListener("click", (e) => e.stopPropagation());
      status.append(label);
      return box;
    };
    const check = checkbox("✅ 반영함", l.done);
    const reject = checkbox("🙅 반려", l.rejected && !l.done);
    check.addEventListener("change", () => {
      if (check.checked) reject.checked = false;
      invoke("mailbox_done", { id: l.id, done: check.checked });
    });
    reject.addEventListener("change", () => {
      if (reject.checked) check.checked = false;
      invoke("mailbox_reject", { id: l.id, rejected: reject.checked });
    });
    row.append(status, del, block);
    box.append(meta, text, row);
    // 열어 보면 읽음 (안 읽은 편지만)
    if (!l.read) box.addEventListener("click", () => invoke("mailbox_read", { id: l.id }), { once: true });
    listEl.append(box);
  }
}

document.getElementById("close").addEventListener("click", () => invoke("mailbox_close"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("mailbox_close");
});

(async () => {
  await here.listen("mailbox-state", (e) => update(e.payload));
  update(await invoke("mailbox_state"));
})();
