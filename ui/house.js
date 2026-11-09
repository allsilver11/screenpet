// 하우스 창: 집 그림 + 문간에 앉은 동물 + 말풍선·음식 고르기 (common.js를 먼저 읽는다).
// 동물이 집 안에 있는 동안(house-state.housed) 문간 애니메이션은 여기서 정한다:
// 대개 sit_front, 가끔(20–40초마다) 10–20초 sleep, 집을 누르면 2초 wave.
// 먹기·성장 축하·갸웃·하트는 백엔드(pet-render)가 정한 대로 보인다. 집 안에서 자라면 집이 작게 폴짝 뛴다 (말풍선 "I grew up!!"은 백엔드 pet-say,
// 집 크기는 백엔드가 곧이어 house-state로 바꾼다 — 새 그림을 미리 받아 두었다가 한 번에 바꿔 깜빡이지 않는다).
// 집은 동물의 단계에 맞는 그림(house/<kind>_<stage>.png — pet.json house: 보통 house, 호박·유령 haunted,
// 루돌프·산타 xmas — 굴뚝에서 연기가 피어오른다)을
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

// 지금 그림 이름 (house_<stage> | haunted_<stage> | xmas_<stage>)과 그 문 사각형 (그림 좌표)
let art = null;
let door = { x: 30, y: 60, w: 52, h: 62 };
// 픽셀 집 (house_pixel/, 좌표는 칸): 집 그림 한 변의 칸 수 — 동글이면 0
let artDots = 0;
let house = { pet: null, stage: "teenager", scale: 1, style: "round", hungry: false, housed: false };
let manifest = null;
let display = 64;
let backendAnim = "sit_front";
let localAnim = "sit_front";
let localUntil = 0;
let nextSleep = performance.now() + rand(SLEEP_EVERY);
let animStart = performance.now();
let loadSeq = 0;
let loadedKey = null;
// 지금 시트 { url, img, style } — 문 크기가 바뀌면 다시 크기를 맞춘다
let sheet = null;

function rand([lo, hi]) {
  return lo + Math.random() * (hi - lo);
}

function houseScale() {
  return HOUSE_SCALE[house.stage] ?? 1;
}

// 집 그림 좌표 하나의 CSS px: 동글은 단계 배율, 픽셀은 그 단계 칸 (물리 px의 정수배)
function artUnit() {
  return artDots ? snapDot(PIXEL_DOT) : houseScale();
}

// 집 종류를 고르는 기준 (src-tauri/src/house.rs house_kind, core manifest house_kind와 같다)
const HOUSE_KINDS = new Set(["house", "haunted", "xmas"]);

// pet.json house가 있으면 그 집, 없으면 보통 집
function houseKind(m) {
  return HOUSE_KINDS.has(m.house) ? m.house : "house";
}

function artName(m) {
  return `${houseKind(m)}_${house.stage}`;
}

// 집 그림·앞 레이어·문간을 지금 단계 크기로 놓는다 (집은 창 오른쪽 아래에 붙는다)
function placeHouse() {
  const k = artUnit();
  const side = (artDots || ART_SIZE) * k;
  // 픽셀 집이면 칸 크기를 CSS에도 (폴짝·연기가 칸 단위로 움직인다)
  document.body.classList[artDots ? "add" : "remove"]("pixel-house");
  if (artDots) document.body.style.setProperty("--dot", k + "px");
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
  placeSmoke(k);
  sizePet();
}

// 굴뚝 연기 (크리스마스 벽돌집 — <art>.json "smoke" {x, y}): 짙은 회색 한 모금(house/xmas_smoke.png) 셋이
// 굴뚝 입구에서 번갈아 피어올라 커지며 흐려진다 (CSS .smoke-puff 반복 애니메이션, 크기·높이는 집 배율 --k)
const SMOKE_PUFFS = 3;
const SMOKE_SECS = 3.6;
let smoke = null;
let smokeEl = null;

function placeSmoke(k) {
  // 그림 스타일이 바뀌면 연기도 새로 만든다
  if (smokeEl && smokeEl.dataset.px !== String(!!artDots)) {
    smokeEl.remove();
    smokeEl = null;
  }
  if (!smoke) {
    smokeEl?.remove();
    smokeEl = null;
    return;
  }
  if (!smokeEl) {
    smokeEl = document.createElement("div");
    smokeEl.id = "smoke";
    smokeEl.dataset.px = String(!!artDots);
    for (let i = 0; i < SMOKE_PUFFS; i++) {
      // 픽셀: 네 장면 띠(house_pixel/xmas_smoke.png)를 계단으로 넘기며 칸 단위로 오른다 (style.css .smoke-px)
      const p = document.createElement(artDots ? "div" : "img");
      p.className = artDots ? "smoke-px" : "smoke-puff";
      if (!artDots) {
        p.src = "house/xmas_smoke.png";
        p.alt = "";
      }
      p.style.animationDuration = `${SMOKE_SECS}s`;
      p.style.animationDelay = `${(-i * SMOKE_SECS) / SMOKE_PUFFS}s`;
      smokeEl.appendChild(p);
    }
    houseEl.insertBefore(smokeEl, houseEl.firstChild);
  }
  smokeEl.style.left = smoke.x * k + "px";
  smokeEl.style.top = smoke.y * k + "px";
  smokeEl.style.setProperty("--k", k);
}

// 첫 그림이 정해지면 보인다 (house.html은 art-pending으로 시작한다). 끝내 못 읽어도 ART_WAIT_MS 뒤엔 기본 집이라도 보인다
const ART_WAIT_MS = 3000;
function showHouse() {
  document.body.classList.remove("art-pending");
}
setTimeout(showHouse, ART_WAIT_MS);

// 단계(또는 동물 종류)가 바뀌었으면 그 단계 그림과 문을 읽는다
// seq: loadPet의 순번 — 읽는 사이 다른 동물/단계로 바뀌었으면 버린다
async function loadArt(name, seq, pixel) {
  let dir = pixel ? "house_pixel" : "house";
  if (`${dir}/${name}` === art) return;
  let d = null;
  let puff = null;
  let dots = 0;
  const read = async (from) => {
    const meta = await (await fetch(`${from}/${name}.json`)).json();
    // 그림을 먼저 받아 풀어 둔다 — 없는 그림을 바로 끼우면 집이 잠깐 비어 보인다
    await Promise.all([decodeSheet(`${from}/${name}.png`), decodeSheet(`${from}/${name}_front.png`)]);
    d = meta.door;
    puff = meta.smoke ?? null;
    dots = meta.dots ?? 0;
  };
  try {
    await read(dir);
  } catch {
    // 픽셀 집을 못 읽으면 동글 집, 그림이 없으면 예전 집 (teenager)
    if (pixel) {
      dir = "house";
      try {
        await read(dir);
      } catch {}
    }
  }
  if (seq !== loadSeq) return;
  if (!d) {
    name = "house";
    dir = "house";
    dots = 0;
    d = { x: 30, y: 60, w: 52, h: 62 };
  }
  art = `${dir}/${name}`;
  door = d;
  smoke = puff;
  artDots = dots;
  houseEl.style.backgroundImage = `url(${dir}/${name}.png)`;
  frontEl.src = `${dir}/${name}_front.png`;
  placeHouse();
  showHouse();
}

// 문간 스프라이트 크기: 화면 DOOR_PET × 몸 배율 (그림을 읽기 전에 시트가 먼저 와도 다시 맞춘다)
function sizePet() {
  if (!sheet || !manifest) return;
  const pixel = sheet.style === "pixel";
  // 픽셀 집이면 동물은 밖에서와 똑같은 칸·크기로 앉는다 (칸은 전 종 같다 — 코어 style.rs, 문은 시트 한 칸이 그대로 들어가는 넓이다)
  if (pixel && artDots) {
    const k = artUnit();
    const dot = k;
    display = manifest.frame_size * dot;
    const dpr = window.devicePixelRatio || 1;
    petEl.style.width = display + "px";
    petEl.style.bottom = "0px";
    petEl.style.left = `${Math.round(((door.w * k - display) / 2) * dpr) / dpr}px`;
    petEl.style.transform = "none";
    petEl.style.backgroundImage = `url(${sheet.url})`;
    petEl.style.backgroundSize = `${sheet.img.width * dot}px ${sheet.img.height * dot}px`;
    return;
  }
  // 픽셀 동물 + 동글 집 (집 그림을 못 읽음): 칸이 물리 px의 정수배가 되게 — 문 밖으로 넘치면 잘리니 내림
  display = pixel ? snapPixelSide(DOOR_PET * house.scale, manifest.frame_size, "floor") : Math.round(DOOR_PET * house.scale);
  petEl.style.width = display + "px";
  const sunk = -(SILL_HIDE * houseScale() + display * FEET_PAD_RATIO);
  // 픽셀: 가운데 맞추기(translateX -50%)·아래 자리가 물리 px에서 어긋나지 않게
  const dpr = window.devicePixelRatio || 1;
  petEl.style.bottom = (pixel ? Math.round(sunk * dpr) / dpr : sunk) + "px";
  petEl.style.left = pixel ? `${Math.round(((door.w * houseScale() - display) / 2) * dpr) / dpr}px` : "";
  petEl.style.transform = pixel ? "none" : "";
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${sheet.url})`;
  petEl.style.backgroundSize = `${sheet.img.width * k}px ${sheet.img.height * k}px`;
}

async function loadPet() {
  const key = `${house.pet}/${house.stage}/${house.scale}/${house.style}`;
  if (!house.pet || key === loadedKey) return;
  const seq = ++loadSeq;
  const { manifest: m, url, img, style } = await loadPetSheet(house.pet, house.stage, house.style);
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  await loadArt(artName(m), seq, house.style === "pixel");
  if (seq !== loadSeq) return;
  loadedKey = key;
  manifest = m;
  sheet = { url, img, style };
  setPixelBody(style === "pixel");
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

const writeDoorPet = styleWriter(petEl);

function draw(now) {
  const hide = !house.housed || !manifest;
  if (petEl.hidden !== hide) petEl.hidden = hide;
  if (!hide) {
    const anim = currentAnim(now);
    const a = manifest.animations;
    const spec = a[anim] ?? a.sit_front ?? a.idle;
    const f = frameIndex(spec, now - animStart);
    writeDoorPet("backgroundPosition", `${-f * display}px ${-spec.row * display}px`);
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
  if (anim === "level_up" && backendAnim !== "level_up" && e.payload.housed) houseHop();
  if (anim === "eat") speech.hidePicker();
  backendAnim = anim;
});

// 집 안에서 성장: 집이 작게 폴짝 (CSS 한 번)
function houseHop() {
  houseEl.classList.remove("house-hop");
  void houseEl.offsetWidth; // 애니메이션 재시작
  houseEl.classList.add("house-hop");
}
houseEl.addEventListener("animationend", (e) => {
  if (e.target === houseEl) houseEl.classList.remove("house-hop");
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

// 픽셀 집: 다른 배율의 모니터로 옮겨 가면 칸을 다시 잰다
window.addEventListener("resize", () => {
  if (artDots) placeHouse();
});

blockWebviewDefaults();

// 그리기 고리는 먼저 돌린다 — 첫 그림을 못 읽어도 다음 house-state로 다시 그릴 수 있게
requestAnimationFrame(draw);

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
})();
