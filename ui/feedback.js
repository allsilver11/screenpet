// 💌 건의하기 창 (트레이 "💌 개발자에게 건의하기…"): 편지를 쓰고 "보내기" → 백엔드가 개발자 열쇠로 봉해 릴레이 편지함에 맡김
// (spec 2026-11-10-feedback-mailbox §4). 하루 5통·글자 수 같은 제한은 릴레이가 셈 — 여기서는 결과를 한 줄로 보여 줄 뿐.
// 결과는 `feedback-result`({ok, reason}) 이벤트로 옴. 보냈으면 2초 뒤 창이 닫히고, 못 보냈으면 쓴 글이 그대로 남음.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const MAX_CHARS = 1000;
const inputEl = document.getElementById("input");
const countEl = document.getElementById("count");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");
let sending = false;

// 릴레이(또는 앱)가 알려 준 이유 → 한 줄
function reasonText(reason) {
  switch (reason) {
    case "daily":
      return "건의는 하루에 5통까지 보낼 수 있어요 — 내일 또 보내 주세요";
    case "new":
      return "앱을 쓴 지 하루가 지나야 보낼 수 있어요";
    case "ip":
    case "global":
      return "오늘은 편지가 많이 왔어요 — 내일 보내 주세요";
    case "full":
      return "건의함이 꽉 찼어요 — 나중에 보내 주세요";
    case "size":
      return "너무 길어요";
    default:
      return "지금은 보낼 수 없어요";
  }
}

// 글자 수를 보임
function count() {
  countEl.textContent = `${[...inputEl.value].length}/${MAX_CHARS}자`;
}

// 편지를 보냄 (보내는 중이면 무시)
function submit() {
  const text = inputEl.value.trim();
  if (!text || sending) return;
  sending = true;
  okBtn.disabled = true;
  noteEl.textContent = "펫이 편지를 들고 가는 중…";
  invoke("feedback_submit", { text: inputEl.value });
}

// 보낸 결과를 한 줄로 보이고 버튼을 다시 켬
function result({ ok, reason }) {
  sending = false;
  okBtn.disabled = false;
  if (ok) {
    noteEl.textContent = "보냈어요 💌 고마워요!";
    inputEl.value = "";
    count();
    setTimeout(() => invoke("feedback_close"), 2000);
  } else {
    noteEl.textContent = reasonText(reason);
  }
}

inputEl.addEventListener("input", () => {
  noteEl.textContent = "";
  count();
});
okBtn.addEventListener("click", submit);
cancelBtn.addEventListener("click", () => invoke("feedback_close"));
// 여러 줄 글이라 Enter는 줄바꿈 — Ctrl/⌘+Enter로 보내기, Esc로 닫기
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    submit();
  } else if (e.key === "Escape") {
    invoke("feedback_close");
  }
});

(async () => {
  await here.listen("feedback-result", (e) => result(e.payload));
  await here.listen("feedback-open", () => {
    noteEl.textContent = "";
    inputEl.focus();
  });
  count();
  inputEl.focus();
})();
