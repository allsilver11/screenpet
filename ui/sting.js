// 침 자국 (sting.html). 그림을 못 읽으면 SVG로 그린 침 자국을 보인다 — 듣기 시작한 뒤에 그림을 고른다 (실패를 놓치지 않게).
const mark = document.getElementById("mark");
mark.addEventListener("error", () => {
  mark.hidden = true;
  document.getElementById("fallback").removeAttribute("hidden");
});
// 픽셀 스타일: 칸 그림 (창이 칸 정수배 크기다), 흐려지는 대신 계단으로
if (window.ART_STYLE === "pixel") {
  mark.src = "effects_pixel/sting.png";
  mark.style.width = mark.style.height = "100%";
  mark.style.imageRendering = "pixelated";
  document.getElementById("decal").style.animationTimingFunction = "steps(3, end)";
} else {
  mark.src = "effects/sting.png";
}
window.addEventListener("contextmenu", (e) => e.preventDefault());
