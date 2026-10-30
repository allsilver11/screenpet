// 편지 쓰기 창 ("친구랑 놀기 → <친구> → 편지 들려 보내기…"): 한 줄 편지를 쓰고 "보내기"(또는 Enter)면
// "놀러 보내기"처럼 내 동물이 그 친구에게 가고, 방문에 편지가 실린다. 받는 친구 이름은 `letter-state`({name, max})로 온다.
// 다듬기(보이지 않는 글자 빼기·80자 자르기)는 백엔드가 다시 한다. 비워서 보내면 편지 없이 그냥 놀러 간다.
// 손님 우클릭 "편지 써서 돌려보내기…"로 열리면 답장 창 (reply: true) — 보내면 손님이 답장을 들고 집으로 (비우면 그냥 돌려보내기)
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const titleEl = document.getElementById("title");
const inputEl = document.getElementById("input");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");

let max = 80;

function count() {
  const n = [...inputEl.value].length;
  noteEl.textContent = `${n}/${max}자 · Enter로 보내기`;
}

function show(v) {
  max = v.max || 80;
  inputEl.maxLength = max * 2; // maxlength는 UTF-16 단위 — 이모지도 들어가게 넉넉히, 글자 수는 아래에서 자른다
  // 답장: 와 있는 손님이 들고 주인에게 돌아간다
  titleEl.textContent = v.reply ? `💌 ${v.name}에게 답장` : `💌 ${v.name}에게 펫과 함께 보낼 편지`;
  inputEl.value = "";
  count();
  inputEl.focus();
}

inputEl.addEventListener("input", () => {
  const chars = [...inputEl.value];
  if (chars.length > max) inputEl.value = chars.slice(0, max).join("");
  count();
});

function submit() {
  invoke("letter_submit", { text: inputEl.value });
}

okBtn.addEventListener("click", submit);
cancelBtn.addEventListener("click", () => invoke("letter_close"));

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  // 한글 조합 중 Enter는 글자 확정 — 보내지 않는다
  if (e.key === "Enter" && !e.isComposing) submit();
  else if (e.key === "Escape") invoke("letter_close");
  else if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) e.preventDefault();
});

(async () => {
  await here.listen("letter-state", (e) => show(e.payload));
  show(await invoke("letter_state"));
})();
