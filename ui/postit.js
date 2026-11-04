// 포스트잇 쓰기 창 (물건 우클릭 "포스트잇 붙이기…"). 다듬기(보이지 않는 글자 빼기·80자 자르기)는 앱이 다시 한다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const inputEl = document.getElementById("input");
const countEl = document.getElementById("count");

let max = 80;

function count() {
  countEl.textContent = `${[...inputEl.value].length}/${max}자 · Enter로 붙이기`;
}

function open(n) {
  max = n || 80;
  inputEl.maxLength = max * 2; // maxlength는 UTF-16 단위 — 이모지도 들어가게 넉넉히, 글자 수는 아래에서 자른다
  inputEl.value = "";
  count();
  inputEl.focus();
}

inputEl.addEventListener("input", () => {
  const chars = [...inputEl.value];
  if (chars.length > max) inputEl.value = chars.slice(0, max).join("");
  count();
});

const submit = () => invoke("postit_submit", { text: inputEl.value });

document.getElementById("ok").addEventListener("click", submit);
document.getElementById("cancel").addEventListener("click", () => invoke("postit_close"));

setupFormKeys({ submit, close: () => invoke("postit_close") });

(async () => {
  await here.listen("postit-open", (e) => open(e.payload));
  open(await invoke("postit_max"));
})();
