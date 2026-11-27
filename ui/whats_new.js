// ✨ 업데이트 소식 창 — 업데이트 뒤 처음 켤 때 한 번 (src-tauri/src/whats_new.rs). 노트(docs/releases/v*.md)를 글자로만 그림:
// "## 제목" → 큰 제목, 한 줄 전체가 "**…**" → 묶음 제목, "- …" → 목록 (안의 **굵게**는 굵게), 그 밖 → 문단. HTML로 넣지 않고 글자로만.
// 받기: whats_new_state() (첫 로드) · `whats-new` 이벤트 [{version, text}] (최신 먼저).
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
const notesEl = document.getElementById("notes");

// 태그와 글자로 요소를 만듦
function el(tag, text) {
  const e = document.createElement(tag);
  if (text !== undefined) e.textContent = text;
  return e;
}

// "**굵게**"가 섞인 한 줄을 글자·굵은 조각으로 붙임
function inline(parent, line) {
  line.split("**").forEach((piece, i) => {
    if (piece) parent.append(i % 2 ? el("b", piece) : el("span", piece));
  });
}

// 노트 하나를 그림 (제목이 없으면 버전을 제목으로)
function renderNote(note) {
  const box = el("div");
  box.className = "note";
  let list = null;
  const lines = String(note.text).split("\n");
  if (!lines.some((l) => l.trim().startsWith("## "))) box.append(el("h2", note.version));
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) {
      list = null;
      continue;
    }
    if (line.startsWith("## ")) {
      box.append(el("h2", line.slice(3)));
      list = null;
    } else if (/^\*\*[^*]+\*\*$/.test(line)) {
      box.append(el("h3", line.slice(2, -2)));
      list = null;
    } else if (line.startsWith("- ")) {
      if (!list) {
        list = el("ul");
        box.append(list);
      }
      const li = el("li");
      inline(li, line.slice(2));
      list.append(li);
    } else {
      const p = el("p");
      inline(p, line);
      box.append(p);
      list = null;
    }
  }
  return box;
}

// 노트들을 모두 그림 (최신 먼저)
function render(notes) {
  notesEl.replaceChildren();
  for (const n of notes || []) notesEl.append(renderNote(n));
}

document.getElementById("ok").addEventListener("click", () => invoke("whats_new_close"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" || e.key === "Enter") invoke("whats_new_close");
});
(async () => {
  await here.listen("whats-new", (e) => render(e.payload));
  render(await invoke("whats_new_state"));
})();
