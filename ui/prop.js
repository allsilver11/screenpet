// 먹이 소품·손에 든 음식 (prop.html). 그림 이름·스타일은 앱이 창을 만들 때 넣어 준다 (prop.rs).
const name = window.PROP_IMAGE || "bowl_food";
if (window.PROP_MODE === "hand") document.body.classList.add("hand");
const el = document.getElementById("prop");
el.className = name;
const pixel = window.ART_STYLE === "pixel";
if (pixel) document.body.classList.add("pixel");
el.src = `${pixel ? "effects_pixel" : "effects"}/${name}.png`;
// 픽셀 눈덩이는 커지는 대신 네 그림으로 자란다 (brain_snow.rs ROLL_SECS 3초)
if (pixel && name === "snowball") {
  el.src = "effects_pixel/snowball_0.png";
  ["snowball_1", "snowball_2", "snowball"].forEach((n, i) => setTimeout(() => (el.src = `effects_pixel/${n}.png`), (i + 1) * 750));
}
window.addEventListener("contextmenu", (e) => e.preventDefault());
