// 🛷 썰매 세리머니: 앱이 창을 화면 한 바퀴 옮기고 (src-tauri/src/sleigh.rs), 여기선 썰매 위 산타·앞에서 끄는 루돌프를
// 동물 시트에서 그린다. 앱이 보내는 건 방향·기울기뿐 (`sleigh-pose`).
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
// 앱이 창을 만들 때 넣는다 (없으면 기본 짝 — 창 시험용)
const setup = window.SLEIGH || {
  puller: { id: "rudolph", stage: "teenager", pixel: false, side: 128 },
  rider: { id: "santa", stage: "teenager", pixel: false, side: 128 },
  unit: 128,
  pixel: false,
  below: 80,
};
const ride = document.getElementById("ride");
const els = { puller: document.getElementById("puller"), rider: document.getElementById("rider") };
const sleighEl = document.getElementById("sleigh");
const reins = document.getElementById("reins");
const ho = document.getElementById("ho");

const u = setup.unit;
const W = u * 2.5;
const H = u * 1.25;
setPixelBody(setup.pixel);
sleighEl.src = setup.pixel ? "sleigh/sleigh_pixel.png" : "sleigh/sleigh.png";
// 썰매 그림: 동글은 정사각 칸(아래쪽에 썰매), 픽셀은 32×16칸
const sleighW = u * 1.35;
const sleighH = setup.pixel ? sleighW / 2 : sleighW;
// 썰매 아래 가운데(도는 축)를 창 아래에서 below만큼 띄운다: 기울면 한쪽 끝이 축보다 내려가도 안 잘린다
Object.assign(ride.style, { width: `${W}px`, height: `${H}px`, marginLeft: `${-W / 2}px`, bottom: `${setup.below || 0}px` });
Object.assign(sleighEl.style, { left: "0px", width: `${sleighW}px`, height: `${sleighH}px` });
// 산타는 썰매 안 (다리는 썰매 앞판에 가린다), 루돌프는 앞에서 끈다
const riderSide = setup.rider.side;
const pullerSide = setup.puller.side;
Object.assign(els.rider.style, { left: `${sleighW * 0.12}px`, bottom: `${sleighW * 0.28}px`, width: `${riderSide}px`, height: `${riderSide}px` });
Object.assign(els.puller.style, { left: `${sleighW * 0.95}px`, bottom: `${u * 0.08}px`, width: `${pullerSide}px`, height: `${pullerSide}px` });
const reinsLen = u * 0.42;
Object.assign(reins.style, { left: `${sleighW * 0.78}px`, bottom: `${u * 0.55}px`, width: `${reinsLen}px`, transform: "rotate(10deg)" });
Object.assign(ho.style, { left: `${sleighW * 0.05}px`, bottom: `${sleighW * 0.28 + riderSide * 0.95}px` });
[["spark1", 0.0, 0.25, 0], ["spark2", -0.15, 0.45, 300], ["spark3", -0.05, 0.1, 600]].forEach(([id, x, y, delay]) => {
  const el = document.getElementById(id);
  Object.assign(el.style, { left: `${u * x}px`, bottom: `${u * y}px`, animationDelay: `${delay}ms` });
});
setTimeout(() => (ho.style.display = "none"), 3200);

// 끄는 쪽은 달리고, 타는 쪽은 손을 흔든다 (그 줄이 없으면 걷기·쉬기)
const ANIMS = { puller: ["run", "walk"], rider: ["wave", "idle"] };
const sheets = {};
async function load(key) {
  const p = setup[key];
  const sheet = await loadPetSheet(p.id, p.stage, p.pixel ? "pixel" : "round");
  const name = ANIMS[key].find((a) => sheet.manifest.animations[a]);
  const spec = sheet.manifest.animations[name];
  const cols = Math.round(sheet.img.naturalWidth / sheet.manifest.frame_size);
  const rows = Math.round(sheet.img.naturalHeight / sheet.manifest.frame_size);
  const el = els[key];
  el.style.backgroundImage = `url("${sheet.url}")`;
  el.style.backgroundSize = `${cols * p.side}px ${rows * p.side}px`;
  sheets[key] = { spec, side: p.side, el };
}

const started = performance.now();
function frame(now) {
  for (const s of Object.values(sheets)) {
    const i = frameIndex(s.spec, now - started);
    s.el.style.backgroundPosition = `${-i * s.side}px ${-s.spec.row * s.side}px`;
  }
  requestAnimationFrame(frame);
}

let pose = { facing: 1, tilt: 0 };
function applyPose() {
  // 오른쪽이 기본 (시트는 오른쪽을 본다). 왼쪽으로 갈 땐 통째로 뒤집고, 기울기는 앞이 위로
  ride.style.transform = `scaleX(${pose.facing < 0 ? -1 : 1}) rotate(${-pose.tilt}deg)`;
  ho.style.transform = pose.facing < 0 ? "scaleX(-1)" : "none";
}
here.listen("sleigh-pose", ({ payload }) => {
  pose = payload;
  applyPose();
});
applyPose();
Promise.all([load("puller"), load("rider")]).then(() => requestAnimationFrame(frame));
