// 🚗 탈것 창: 시트 한 장 (ui/vehicles/<종류>.png — 가로 FRAMES칸, tools/draw_vehicles.py)에서 칸을 골라 그림.
// 앱이 `vehicle-pose` {dir, moving, rotor, tilt, dust, bump}를 보냄: 달리면 바퀴 칸을, 시동이 걸렸으면 프로펠러 칸을 넘기고,
// 왼쪽을 보면 뒤집고, 날며 앞으로 숙임 (바퀴 바닥 가운데 축 — 탄 펫도 render.lean으로 같이 기욺). `vehicle-fx` {kind: "screech"|"crash"}면 먼지·💥.
// 누르면 vehicle_down (잡아 끌기 — 탄 펫은 튕겨 내림), 우클릭이면 vehicle_menu.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 시트 칸 수 (tools/draw_vehicles.py FRAMES와 같음)
const FRAMES = 3;
// 바퀴 칸을 넘기는 빠르기 (초당 칸) — 프로펠러는 더 빠름
const WHEEL_FPS = 12;
const ROTOR_FPS = 18;

const info = window.VEHICLE || { kind: "car" };
const art = document.getElementById("art");
art.style.backgroundImage = `url(vehicles/${info.kind}.png)`;

let pose = { dir: 1, moving: false, rotor: false, tilt: 0, dust: false, bump: false };
let frame = 0;
let last = performance.now();
let acc = 0;

// 칸을 그림에 입힘
function showFrame() {
  art.style.backgroundPosition = `${(frame * 100) / (FRAMES - 1)}% 0`;
}

// 보는 쪽·기울기를 입힘
function applyPose() {
  const flip = pose.dir < 0 ? -1 : 1;
  art.style.transform = `scaleX(${flip}) rotate(${pose.tilt || 0}deg)`;
}

// 달리거나 시동이 걸린 동안만 칸을 넘김 (멈춘 탈것의 웹뷰는 쉼)
function tick(now) {
  const fps = pose.rotor ? ROTOR_FPS : pose.moving ? WHEEL_FPS : 0;
  const dt = (now - last) / 1000;
  last = now;
  if (fps > 0) {
    acc += dt * fps;
    if (acc >= 1) {
      frame = (frame + Math.floor(acc)) % FRAMES;
      acc %= 1;
      showFrame();
    }
    requestAnimationFrame(tick);
  } else {
    running = false;
  }
}
let running = false;
// 칸 넘기기를 (필요하면) 다시 시작함
function wake() {
  if (running || !(pose.rotor || pose.moving)) return;
  running = true;
  last = performance.now();
  requestAnimationFrame(tick);
}

// 끼익: 앞바퀴 쪽에 먼지 몇 덩이
function screech() {
  const front = pose.dir < 0 ? 0.18 : 0.82;
  for (let i = 0; i < 5; i++) {
    const d = document.createElement("div");
    d.className = "dust";
    d.style.left = `calc(${front * 100}% + ${(Math.random() - 0.5) * 16}px)`;
    d.style.setProperty("--dx", `${-pose.dir * (10 + Math.random() * 24)}px`);
    d.style.animationDelay = `${i * 60}ms`;
    document.body.appendChild(d);
    setTimeout(() => d.remove(), 900);
  }
}

// 쿵: 앞쪽에 💥
function crash() {
  const c = document.createElement("div");
  c.className = "crash";
  c.textContent = "💥";
  c.style.left = pose.dir < 0 ? "2px" : "calc(100% - 36px)";
  document.body.appendChild(c);
  setTimeout(() => c.remove(), 800);
  bump();
}

// 출렁
function bump() {
  art.classList.remove("bump");
  void art.offsetWidth;
  art.classList.add("bump");
}

showFrame();
applyPose();

art.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("vehicle_down");
});
art.addEventListener("contextmenu", () => invoke("vehicle_menu"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("dragstart", (e) => e.preventDefault());

here.listen("vehicle-pose", ({ payload: p }) => {
  if (p.bump && !pose.bump) bump();
  pose = p;
  applyPose();
  wake();
});
here.listen("vehicle-fx", ({ payload: p }) => {
  if (p.kind === "screech") screech();
  else if (p.kind === "crash") crash();
});
