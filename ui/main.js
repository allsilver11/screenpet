// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그린다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");
const bubbleEl = document.getElementById("bubble");
const pickerEl = document.getElementById("picker");
const favBtn = document.getElementById("fav");

let manifest = null;
let display = 128;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();
// 지금 동물과 몸 크기 배율 (단계 배율 × 동물별 배율) — 백엔드 `pet-stage`
let stage = { pet: null, stage: "teenager", scale: 1 };
let loadSeq = 0;

async function decodeSheet(url) {
  const img = new Image();
  img.src = url;
  await img.decode();
  return img;
}

// 단계별 시트 sheet_<stage>.png를 먼저 쓰고, 없으면 sheet.png
async function loadPet() {
  const seq = ++loadSeq;
  const id = stage.pet;
  const m = await (await fetch(`pets/${id}/pet.json`)).json();
  let url = `pets/${id}/sheet_${stage.stage}.png`;
  let img;
  try {
    img = await decodeSheet(url);
  } catch {
    url = `pets/${id}/sheet.png`;
    img = await decodeSheet(url);
  }
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  const side = 128 * stage.scale;
  petEl.style.width = side + "px";
  // 창 너비 = 128 × max(1, 배율) (백엔드 window_logical_size) — 리사이즈 도착 전에도 가운데
  petEl.style.left = (128 * Math.max(1, stage.scale) - side) / 2 + "px";
  display = side;
  manifest = m;
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${url})`;
  petEl.style.backgroundSize = `${img.width * k}px ${img.height * k}px`;
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
    // 이동 속도는 √배율, 몸 반지름은 배율에 비례 → 구르는 각속도는 1/√배율
    if (spec.spin) parts.push(`rotate(${((elapsed / 1000) * spec.spin / Math.sqrt(stage.scale)) % 360}deg)`);
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
  // 트레이에서 먹였거나 다른 길로 먹기 시작하면 고르기 버튼은 필요 없다
  if (e.payload.anim === "eat") hidePicker();
  render = e.payload;
});

// 동물 교체·성장·리셋: 창 크기는 백엔드가 바꾸고, 여기선 시트와 스프라이트 크기를 맞춘다
listen("pet-stage", (e) => {
  stage = e.payload;
  loadPet();
});

let bubbleTimer = null;

// 백엔드가 고른 상황(chatter/tickle/grumpy/hiss/hungry/full/level_up/ignore)에 맞는 대사를
// pet.json speech에서 랜덤으로 띄운다. 그 동물에 없는 상황이면 말풍선 없음.
listen("pet-say", (e) => {
  const lines = manifest?.speech?.[e.payload] ?? [];
  if (!lines.length || !pickerEl.hidden) return; // 음식 고르기 버튼이 우선
  bubbleEl.textContent = lines[Math.floor(Math.random() * lines.length)];
  bubbleEl.hidden = false;
  clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => {
    bubbleEl.hidden = true;
  }, 1500);
});

let pickerTimer = null;

function hidePicker() {
  pickerEl.hidden = true;
  clearTimeout(pickerTimer);
}

// 배고플 때 클릭하면 머리 위에 🍪 🍚 (좋아하는 것) 버튼
listen("food-picker", (e) => {
  favBtn.textContent = e.payload.emoji;
  favBtn.title = e.payload.name;
  bubbleEl.hidden = true;
  pickerEl.hidden = false;
  clearTimeout(pickerTimer);
  pickerTimer = setTimeout(hidePicker, 6000);
});

for (const btn of pickerEl.querySelectorAll("button")) {
  // 버튼을 누르는 게 펫 잡기로 이어지지 않게 한다
  btn.addEventListener("mousedown", (e) => e.stopPropagation());
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    invoke("feed", { food: btn.dataset.food });
    hidePicker();
  });
}

petEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("pointer_down");
});

// 펫을 우클릭하면 트레이와 같은 메뉴
petEl.addEventListener("contextmenu", () => invoke("show_menu"));

// 펫 창엔 키보드 UI가 없으므로 웹뷰 기본 동작(우클릭 메뉴, F5, Ctrl+R, Alt+F4 등)을 막는다.
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => e.preventDefault());

(async () => {
  stage = await invoke("current_stage");
  await loadPet();
  requestAnimationFrame(draw);
})();
