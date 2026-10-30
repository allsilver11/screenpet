// 이름 바꾸기 창: 친구들에게 보일 내 이름 / 친구에게 붙인 별명.
// 백엔드가 `name-state`({target, current, fallback, max})로 무엇을 고치는지 알려 주고, 저장은 `name_submit`으로 한다.
// 비워서 저장하면 기본 이름(컴퓨터 사용자 이름 / 친구가 정한 이름)으로 돌아간다. 다듬기·길이 제한은 백엔드가 다시 한다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const titleEl = document.getElementById("title");
const inputEl = document.getElementById("input");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");

let view = { target: "me", current: "", fallback: "", max: 20 };

function show(v) {
  view = v;
  const max = v.max || 20;
  inputEl.maxLength = max;
  titleEl.textContent = v.target === "me" ? "친구들에게 보일 내 이름" : `"${v.fallback}" 친구를 뭐라고 부를까요?`;
  inputEl.placeholder = v.fallback;
  inputEl.value = v.current;
  noteEl.textContent = v.target === "me" ? `비우면 "${v.fallback}" (최대 ${max}자)` : `비우면 친구가 정한 이름 (최대 ${max}자)`;
  inputEl.focus();
  inputEl.select();
}

function submit() {
  invoke("name_submit", { name: inputEl.value });
}

okBtn.addEventListener("click", submit);
cancelBtn.addEventListener("click", () => invoke("name_close"));

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Enter") submit();
  else if (e.key === "Escape") invoke("name_close");
  else if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) e.preventDefault();
});

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("name-state", (e) => show(e.payload));
  show(await invoke("name_state"));
})();
