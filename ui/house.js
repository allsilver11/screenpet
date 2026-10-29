// 하우스 창: 집 그림 + 문간에 앉은 동물 + 말풍선·음식 고르기 (common.js를 먼저 읽는다).
// 동물이 집 안에 있는 동안(house-state.housed) 문간 애니메이션은 여기서 정한다:
// 대개 sit_front, 가끔(20–40초마다) 10–20초 sleep, 집을 누르면 2초 wave.
// 먹기·성장 축하·갸웃·하트는 백엔드(pet-render)가 정한 대로 보인다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const houseEl = document.getElementById("house");
const doorEl = document.getElementById("door");
const petEl = document.getElementById("door-pet");
const bubbleEl = document.getElementById("bubble");
const pickerEl = document.getElementById("picker");
const favBtn = document.getElementById("fav");

// 백엔드가 정하는 동작 (하우스 안에서 먹을 때) — 그 밖엔 문간 동작을 여기서 고른다
const BACKEND_ANIMS = new Set(["eat", "level_up", "tilt", "heart"]);
const SLEEP_EVERY = [20000, 40000];
const SLEEP_FOR = [10000, 20000];
const WAVE_MS = 2000;
// 문간 스프라이트 한 변 = 문 너비 / 이 비율 (프레임 안에서 동물 몸이 차지하는 폭) × 몸 배율
const BODY_WIDTH_RATIO = 0.7;
// 프레임 아래쪽 빈 공간 비율 — 발이 문턱 바로 뒤에 오도록 그만큼 내린다
const FEET_PAD_RATIO = 0.1;
// 문턱 뒤로 발을 숨기는 깊이 (px, tools/draw_house.py pet_in_door와 같은 6)
const SILL_HIDE = 6;

let door = { x: 30, y: 60, w: 52, h: 62 };
let house = { pet: null, stage: "teenager", scale: 1, hungry: false, housed: false };
let manifest = null;
let display = 64;
let backendAnim = "sit_front";
let localAnim = "sit_front";
let localUntil = 0;
let nextSleep = performance.now() + rand(SLEEP_EVERY);
let animStart = performance.now();
let loadSeq = 0;
let loadedKey = null;
// 지금 시트 { url, img } — 문 크기가 바뀌면 다시 크기를 맞춘다
let sheet = null;

function rand([lo, hi]) {
  return lo + Math.random() * (hi - lo);
}

function placeDoor() {
  doorEl.style.left = door.x + "px";
  doorEl.style.top = door.y + "px";
  doorEl.style.width = door.w + "px";
  doorEl.style.height = door.h + "px";
  sizePet();
}

// 문간 스프라이트 크기: 문 너비와 몸 배율에서 (house.json을 읽기 전에 시트가 먼저 와도 다시 맞춘다)
function sizePet() {
  if (!sheet || !manifest) return;
  display = Math.round(((door.w - 2) / BODY_WIDTH_RATIO) * house.scale);
  petEl.style.width = display + "px";
  petEl.style.bottom = -(SILL_HIDE + display * FEET_PAD_RATIO) + "px";
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${sheet.url})`;
  petEl.style.backgroundSize = `${sheet.img.width * k}px ${sheet.img.height * k}px`;
}

async function loadPet() {
  const key = `${house.pet}/${house.stage}/${house.scale}`;
  if (!house.pet || key === loadedKey) return;
  const seq = ++loadSeq;
  const { manifest: m, url, img } = await loadPetSheet(house.pet, house.stage);
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  loadedKey = key;
  manifest = m;
  sheet = { url, img };
  sizePet();
  animStart = performance.now();
}

function setLocal(anim, ms, now) {
  if (anim !== localAnim) animStart = now;
  localAnim = anim;
  localUntil = ms ? now + ms : 0;
}

// 지금 문간에 보일 동작
function currentAnim(now) {
  if (BACKEND_ANIMS.has(backendAnim)) return backendAnim;
  if (localUntil && now >= localUntil) {
    setLocal("sit_front", 0, now);
    nextSleep = now + rand(SLEEP_EVERY);
  }
  if (localAnim === "sit_front" && now >= nextSleep) setLocal("sleep", rand(SLEEP_FOR), now);
  return localAnim;
}

function draw(now) {
  petEl.hidden = !house.housed || !manifest;
  if (!petEl.hidden) {
    const anim = currentAnim(now);
    const a = manifest.animations;
    const spec = a[anim] ?? a.sit_front ?? a.idle;
    const f = frameIndex(spec, now - animStart);
    petEl.style.backgroundPosition = `${-f * display}px ${-spec.row * display}px`;
  }
  requestAnimationFrame(draw);
}

listen("house-state", (e) => {
  const wasHoused = house.housed;
  house = e.payload;
  if (house.housed && !wasHoused) {
    // 막 들어왔다 — 앉은 모습부터
    setLocal("sit_front", 0, performance.now());
    nextSleep = performance.now() + rand(SLEEP_EVERY);
  }
  loadPet();
});

listen("pet-render", (e) => {
  const anim = e.payload.anim;
  if (anim !== backendAnim && BACKEND_ANIMS.has(anim)) animStart = performance.now();
  if (anim === "eat") speech.hidePicker();
  backendAnim = anim;
});

// 집을 눌렀는데 배고프지 않으면 손 흔들기
listen("house-wave", () => setLocal("wave", WAVE_MS, performance.now()));

const speech = setupSpeech({ bubbleEl, pickerEl, favBtn, getManifest: () => manifest });

houseEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) invoke("house_click");
});

// 집을 우클릭해도 트레이와 같은 메뉴
houseEl.addEventListener("contextmenu", () => invoke("show_menu"));

blockWebviewDefaults();

(async () => {
  try {
    const j = await (await fetch("house/house.json")).json();
    if (j.door) door = j.door;
  } catch {
    // 기본 문 위치 그대로
  }
  placeDoor();
  house = await invoke("house_state");
  await loadPet();
  requestAnimationFrame(draw);
})();
