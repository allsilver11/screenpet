// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그린다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");

let manifest = null;
let display = 128;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();

async function loadPet(id) {
  manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  display = petEl.clientWidth || 128;
  const scale = display / manifest.frame_size;
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
    // 교체된 동물에 없는 종 전용 애니메이션이어도 throw로 rAF가 끊기지 않게 idle로 대체
    const spec = manifest.animations[render.anim] ?? manifest.animations.idle;
    const elapsed = now - animStart;
    const f = frameIndex(spec, elapsed);
    petEl.style.backgroundPosition = `${-f * display}px ${-spec.row * display}px`;
    // 좌우 반전 공간 안에서 돌리므로 왼쪽으로 갈 땐 자동으로 반대로 구른다
    const parts = [];
    if (render.flip) parts.push("scaleX(-1)");
    if (spec.spin) parts.push(`rotate(${((elapsed / 1000) * spec.spin) % 360}deg)`);
    petEl.style.transform = parts.length ? parts.join(" ") : "none";
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
});

listen("pet-change", (e) => loadPet(e.payload));

const bubbleEl = document.getElementById("bubble");
let bubbleTimer = null;

// 백엔드가 고른 상황(chatter/tickle/grumpy)에 맞는 대사를 pet.json speech에서 랜덤으로 띄운다
listen("pet-say", (e) => {
  const lines = manifest?.speech?.[e.payload] ?? [];
  if (!lines.length) return;
  bubbleEl.textContent = lines[Math.floor(Math.random() * lines.length)];
  bubbleEl.hidden = false;
  clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => {
    bubbleEl.hidden = true;
  }, 1500);
});

petEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("pointer_down");
});

// 펫 창엔 키보드/메뉴 UI가 없으므로 웹뷰 기본 동작(우클릭 메뉴, F5, Ctrl+R, Alt+F4 등)을 막는다.
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => e.preventDefault());

(async () => {
  await loadPet(await invoke("current_pet"));
  requestAnimationFrame(draw);
})();
