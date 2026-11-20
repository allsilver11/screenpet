// 먹이 소품·손에 든 음식 (prop.html). 그림 이름은 앱이 창을 만들 때 넣어 줌 (prop.rs).
const name = window.PROP_IMAGE || "bowl_food";
if (window.PROP_MODE === "hand") document.body.classList.add("hand");
const el = document.getElementById("prop");
el.className = name;
el.src = `effects/${name}.png`;
window.addEventListener("contextmenu", (e) => e.preventDefault());
