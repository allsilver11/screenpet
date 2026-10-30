// 하우스 창: 집 그림 + 문간에 앉은 동물 + 말풍선·음식 고르기 (common.js를 먼저 읽는다).
// 동물이 집 안에 있는 동안(house-state.housed) 문간 애니메이션은 여기서 정한다:
// 대개 sit_front, 가끔(20–40초마다) 10–20초 sleep, 집을 누르면 2초 wave.
// 먹기·성장 축하·갸웃·하트는 백엔드(pet-render)가 정한 대로 보인다.
// 집은 동물의 단계에 맞는 그림(house/<kind>_<stage>.png — pet.json house: 보통 house, 꿀벌 hive, 호박·유령 haunted)을
// 128 × HOUSE_SCALE[stage] 크기로 창 오른쪽 아래에 그린다 (창 크기는 src-tauri/src/house.rs가 같은 배율로 맞춘다).
const { invoke } = window.__TAURI__.core;
// 전역 event.listen은 손님 창 앞으로 보낸 pet-render까지 받는다 — 이 창 앞으로 온 것만 듣는다
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
const listen = (event, handler) => here.listen(event, handler);

const houseEl = document.getElementById("house");
const frontEl = document.getElementById("house-front");
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
// 단계별 집 크기 배율 (src-tauri/src/house.rs house_scale, tools/draw_house.py HOUSE_SCALE와 같다)
const HOUSE_SCALE = { baby: 0.55, kinder: 0.68, child: 0.82, teenager: 1.0, adult: 1.12 };
// 집 그림 한 변 (그림 좌표)
const ART_SIZE = 128;
// 문간 스프라이트 한 변 (화면 px) = 이 값 × 몸 배율 — teenager 집 문(52) / 0.7 (프레임 안에서 몸이 차지하는 폭).
// 단계마다 문 사각형이 이 크기의 펫을 같은 비율로 채우도록 그려져 있다 (tools/draw_house.py DOOR_PET)
const DOOR_PET = (52 - 2) / 0.7;
// 프레임 아래쪽 빈 공간 비율 — 발이 문턱 바로 뒤에 오도록 그만큼 내린다
const FEET_PAD_RATIO = 0.1;
// 문턱 뒤로 발을 숨기는 깊이 (그림 px, tools/draw_house.py SILL_HIDE와 같은 6)
const SILL_HIDE = 6;

// 지금 그림 이름 (house_<stage> | hive_<stage> | haunted_<stage>)과 그 문 사각형 (그림 좌표)
let art = null;
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

function houseScale() {
  return HOUSE_SCALE[house.stage] ?? 1;
}

// 집 종류를 고르는 기준 (src-tauri/src/house.rs house_kind, core manifest house_kind와 같다)
const HOUSE_KINDS = new Set(["house", "hive", "haunted"]);
const FALLBACK_KIND = { bee: "hive", pumpkin: "haunted", ghost: "haunted" };

// pet.json house가 있으면 그 집, 없으면 sting이 있는 동물(꿀벌)은 벌통 (예전 pet.json), 나머지는 보통 집.
// pet.json이 아직 없으면 id로
function houseKind(m) {
  if (!m) return FALLBACK_KIND[house.pet] ?? "house";
  if (HOUSE_KINDS.has(m.house)) return m.house;
  return m.animations?.sting ? "hive" : "house";
}

function artName(m) {
  return `${houseKind(m)}_${house.stage}`;
}

// 집 그림·앞 레이어·문간을 지금 단계 크기로 놓는다 (집은 창 오른쪽 아래에 붙는다)
function placeHouse() {
  const k = houseScale();
  const side = ART_SIZE * k;
  houseEl.style.left = "auto";
  houseEl.style.right = "0";
  houseEl.style.width = side + "px";
  houseEl.style.height = side + "px";
  houseEl.style.backgroundSize = `${side}px ${side}px`;
  frontEl.style.width = side + "px";
  frontEl.style.height = side + "px";
  doorEl.style.left = door.x * k + "px";
  doorEl.style.top = door.y * k + "px";
  doorEl.style.width = door.w * k + "px";
  doorEl.style.height = door.h * k + "px";
  sizePet();
}

// 단계(또는 동물 종류)가 바뀌었으면 그 단계 그림과 문을 읽는다
// seq: loadPet의 순번 — 읽는 사이 다른 동물/단계로 바뀌었으면 버린다
async function loadArt(name, seq) {
  if (name === art) return;
  let d = null;
  try {
    d = (await (await fetch(`house/${name}.json`)).json()).door;
  } catch {
    // 그림이 없으면 예전 집 (teenager)
  }
  if (seq !== loadSeq) return;
  if (!d) {
    name = "house";
    d = { x: 30, y: 60, w: 52, h: 62 };
  }
  art = name;
  door = d;
  houseEl.style.backgroundImage = `url(house/${name}.png)`;
  frontEl.src = `house/${name}_front.png`;
  placeHouse();
}

// 문간 스프라이트 크기: 화면 DOOR_PET × 몸 배율 (그림을 읽기 전에 시트가 먼저 와도 다시 맞춘다)
function sizePet() {
  if (!sheet || !manifest) return;
  display = Math.round(DOOR_PET * house.scale);
  petEl.style.width = display + "px";
  petEl.style.bottom = -(SILL_HIDE * houseScale() + display * FEET_PAD_RATIO) + "px";
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
  await loadArt(artName(m), seq);
  if (seq !== loadSeq) return;
  loadedKey = key;
  manifest = m;
  sheet = { url, img };
  placeHouse();
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

const listeningState = listen("house-state", (e) => {
  const wasHoused = house.housed;
  house = e.payload;
  if (house.housed && !wasHoused) {
    // 막 들어왔다 — 앉은 모습부터
    setLocal("sit_front", 0, performance.now());
    nextSleep = performance.now() + rand(SLEEP_EVERY);
  }
  loadPet();
});

// 첫 로드 땐 내 동물의 지금 모습을 slot_info("pet")로 묻는다 (이벤트가 먼저 왔으면 그쪽이 새것)
let gotRender = false;
const listeningRender = listen("pet-render", (e) => {
  gotRender = true;
  const anim = e.payload.anim;
  if (anim !== backendAnim && BACKEND_ANIMS.has(anim)) animStart = performance.now();
  if (anim === "eat") speech.hidePicker();
  backendAnim = anim;
});

// 집을 눌렀는데 배고프지 않으면 손 흔들기
listen("house-wave", () => setLocal("wave", WAVE_MS, performance.now()));

const speech = setupSpeech({ bubbleEl, pickerEl, favBtn, getManifest: () => manifest });

// 집을 누른 채 DRAG_PX 넘게 움직이면 집을 끈다 (창은 src-tauri/src/house.rs가 커서를 따라 옮기고, 놓으면 바닥에 앉힌다).
// 움직이지 않고 놓으면 예전처럼 누르기 (배고프면 음식 고르기, 아니면 손 흔들기)
const DRAG_PX = 5;
let press = null;

houseEl.addEventListener("mousedown", (e) => {
  if (e.button === 0) press = { x: e.screenX, y: e.screenY, dragging: false };
});

window.addEventListener("mousemove", (e) => {
  if (!press) return;
  if (!(e.buttons & 1)) {
    // 창 밖에서 놓았다 — 끌기는 백엔드가 버튼 상태로 끝낸다
    endPress();
    return;
  }
  if (!press.dragging && Math.hypot(e.screenX - press.x, e.screenY - press.y) > DRAG_PX) {
    press.dragging = true;
    document.body.classList.add("dragging");
    speech.hidePicker();
    invoke("house_drag_start");
  }
});

window.addEventListener("mouseup", (e) => {
  if (e.button !== 0 || !press) return;
  const clicked = !press.dragging;
  endPress();
  if (clicked) invoke("house_click");
});

function endPress() {
  press = null;
  document.body.classList.remove("dragging");
}

// 집을 우클릭해도 트레이와 같은 메뉴
houseEl.addEventListener("contextmenu", () => invoke("show_menu"));

blockWebviewDefaults();

(async () => {
  placeHouse();
  // 듣기 시작한 뒤에 지금 상태를 묻는다 — 그 사이 바뀐 것은 이벤트로 온다
  await Promise.all([listeningState, listeningRender]);
  house = await invoke("house_state");
  const own = await invoke("slot_info", { label: "pet" }).catch(() => null);
  if (!gotRender && own?.render && BACKEND_ANIMS.has(own.render.anim)) {
    backendAnim = own.render.anim;
    animStart = performance.now();
  }
  await loadPet();
  requestAnimationFrame(draw);
})();
