// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그린다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const DISPLAY = 128;
const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");

let manifest = null;
let scale = 2;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();

async function loadPet(id) {
  manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  scale = DISPLAY / manifest.frame_size;
  const img = new Image();
  img.src = `pets/${id}/sheet.png`;
  await img.decode();
  petEl.style.backgroundImage = `url(pets/${id}/sheet.png)`;
  petEl.style.backgroundSize = `${img.width * scale}px ${img.height * scale}px`;
  animStart = performance.now();
}

function frameIndex(spec, elapsedMs) {
  const n = Math.floor((elapsedMs / 1000) * spec.fps);
  return spec.loop === false ? Math.min(n, spec.frames - 1) : n % spec.frames;
}

function draw(now) {
  if (manifest) {
    const spec = manifest.animations[render.anim];
    const f = frameIndex(spec, now - animStart);
    petEl.style.backgroundPosition = `${-f * DISPLAY}px ${-spec.row * DISPLAY}px`;
  }
  requestAnimationFrame(draw);
}

listen("pet-render", (e) => {
  if (e.payload.anim !== render.anim) animStart = performance.now();
  if (e.payload.heart && !render.heart) {
    heartEl.classList.remove("show");
    void heartEl.offsetWidth; // 애니메이션 재시작
    heartEl.classList.add("show");
  }
  render = e.payload;
  petEl.classList.toggle("flip", render.flip);
});

listen("pet-change", (e) => loadPet(e.payload));

petEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("pointer_down");
});

(async () => {
  await loadPet(await invoke("current_pet"));
  requestAnimationFrame(draw);
})();
