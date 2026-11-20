// 👻 투명도 창: 막대를 움직이는 동안 모든 떠 있는 창에 바로 보여 주고, 놓거나 ↺를 누르면 저장함.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const sliderEl = document.getElementById("opacity");
const valueEl = document.getElementById("opacity-value");
const resetEl = document.getElementById("reset");

// 막대 값을 퍼센트 글자로 보임 (100%면 ↺를 끔)
function paint() {
  valueEl.textContent = `${sliderEl.value}%`;
  resetEl.disabled = Number(sliderEl.value) === 100;
}

// 지금 값을 백엔드에 보냄 (save면 파일에도 남김)
function send(save) {
  paint();
  invoke("opacity_set", { pct: Number(sliderEl.value), save });
}

// 백엔드가 준 지금 값(%)을 막대에 넣음
function show(pct) {
  sliderEl.value = pct;
  paint();
}

sliderEl.addEventListener("input", () => send(false));
sliderEl.addEventListener("change", () => send(true));
resetEl.addEventListener("click", () => {
  sliderEl.value = 100;
  send(true);
});
document.getElementById("close").addEventListener("click", () => invoke("opacity_close"));

setupFormKeys({ close: () => invoke("opacity_close") });

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 물음
  await here.listen("opacity-state", (e) => show(e.payload));
  show(await invoke("opacity_state"));
})();
