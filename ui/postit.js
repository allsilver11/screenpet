// 포스트잇 쓰기 창 (물건 우클릭 "포스트잇 붙이기…"). 다듬기(보이지 않는 글자 빼기·80자 자르기)는 앱이 다시 함.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const inputEl = document.getElementById("input");
const countEl = document.getElementById("count");

const field = noteField(inputEl, (n, max) => {
  countEl.textContent = `${n}/${max}자 · Enter로 붙이기`;
});

// 쓴 글을 백엔드에 넘김
const submit = () => invoke("postit_submit", { text: inputEl.value });

document.getElementById("ok").addEventListener("click", submit);
document.getElementById("cancel").addEventListener("click", () => invoke("postit_close"));

setupFormKeys({ submit, close: () => invoke("postit_close") });

(async () => {
  await here.listen("postit-open", (e) => field.open(e.payload));
  field.open(await invoke("postit_max"));
})();
