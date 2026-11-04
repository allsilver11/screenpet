// 바닥에 떨어진 선물 하나 (gift.html). 그림 이름·스타일은 앱이 창을 만들 때 넣어 준다 (gifts.rs).
const giftEl = document.getElementById("gift");
// 스타일에 맞는 그림 (art-style: 떠 있는 동안 바뀌면 앱이 창 크기도 맞춘다)
function applyArt(pixel) {
  document.body.classList.toggle("pixel", pixel);
  giftEl.src = `${pixel ? "gifts_pixel" : "gifts"}/${window.GIFT_IMAGE || "cat_yarn"}.png`;
}
applyArt(window.ART_STYLE === "pixel");
const { invoke } = window.__TAURI__.core;
window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("art-style", (e) => applyArt(e.payload === "pixel"));
let sent = false;
window.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || sent) return;
  sent = true;
  invoke("gift_collect");
});
window.addEventListener("contextmenu", (e) => e.preventDefault());
