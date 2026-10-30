// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그린다. (common.js를 먼저 읽는다)
// 내 동물 창("pet")과 손님 창("guest-<id>")이 같이 쓴다 — 자기 동물은 창 라벨로 `slot_info`에 묻는다.
const { invoke } = window.__TAURI__.core;
// 전역 event.listen은 다른 창 앞으로 보낸(emit_to) 이벤트까지 받는다 — 이 창 앞으로 온 것만 듣는다
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
const listen = (event, handler) => here.listen(event, handler);

const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");
const bubbleEl = document.getElementById("bubble");
const pickerEl = document.getElementById("picker");
const favBtn = document.getElementById("fav");
const nametagEl = document.getElementById("nametag");
const stageEl = document.getElementById("stage");
const envelopeEl = document.getElementById("envelope");
const noteEl = document.getElementById("note");

let manifest = null;
let display = 128;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();
// 지금 동물과 몸 크기 배율 (단계 배율 × 동물별 배율) — 백엔드 `pet-stage`
let stage = { pet: null, stage: "teenager", scale: 1 };
let loadSeq = 0;

// 단계별 시트 sheet_<stage>.png를 먼저 쓰고, 없으면 sheet.png
async function loadPet() {
  const seq = ++loadSeq;
  const { manifest: m, url, img } = await loadPetSheet(stage.pet, stage.stage);
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  const side = 128 * stage.scale;
  petEl.style.width = side + "px";
  // 창 가운데 (창 너비는 128 × max(1, 배율), 편지 쪽지를 펼친 동안은 더 넓다 — 백엔드가 몸을 창 가운데에 둔다)
  petEl.style.left = `calc(50% - ${side / 2}px)`;
  stageEl.style.setProperty("--pet-h", side + "px");
  display = side;
  manifest = m;
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${url})`;
  petEl.style.backgroundSize = `${img.width * k}px ${img.height * k}px`;
  animStart = performance.now();
}

// 꿀벌 쏘기: 스프라이트를 1 → 1.2 → 1로 키웠다 줄여 화면 쪽으로 다가오는 것처럼 보인다.
// 부풀기는 돌진 그림(sting의 마지막 프레임, pet.json frames/fps)이 보이는 동안, 백엔드 돌진 국면
// (sting::ALERT_SECS 0.5초 뒤 DASH_SECS 0.3초, 끝나는 순간 콕!)과 겹치는 때 한다. 1.2배면 128px 창 안에 남는다.
const STING_ALERT_MS = 500;
const STING_DASH_MS = 300;
const STING_DASH_SCALE = 0.2;

function stingPulse(anim, spec, elapsedMs) {
  if (anim !== "sting" || !spec?.fps) return 1;
  // 돌진 그림이 처음 보이는 때 — 백엔드 돌진보다 늦으면 그때부터, 콕!(돌진 끝)까지
  const lungeMs = ((spec.frames - 1) / spec.fps) * 1000;
  const start = Math.max(lungeMs, STING_ALERT_MS);
  // 그림이 백엔드보다 한참 늦게 바뀌는 시트여도 짧게나마 부푼다
  const end = Math.max(STING_ALERT_MS + STING_DASH_MS, start + STING_DASH_MS / 2);
  const dur = end - start;
  const t = (elapsedMs - start) / dur;
  if (t <= 0 || t >= 1) return 1;
  return 1 + STING_DASH_SCALE * Math.sin(Math.PI * t);
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
    const pulse = stingPulse(render.anim, spec, elapsed);
    if (pulse !== 1) parts.push(`scale(${pulse})`);
    petEl.style.transform = parts.length ? parts.join(" ") : "none";
  }
  requestAnimationFrame(draw);
}

// 할로윈 동물의 짧은 효과 (CSS 애니메이션 한 번, style.css .boo/.flare — 끝나면 클래스를 뗀다):
// 유령은 간질이면 "부~" 하고 40%까지 흐려졌다 돌아오고, 호박은 누르면 눈·입 빛이 확 번진다
const PET_FX = { ghost: { on: "tickle", cls: "boo" }, pumpkin: { on: "click", cls: "flare" } };
function petFx(trigger) {
  const fx = PET_FX[stage.pet];
  if (!fx || fx.on !== trigger) return;
  petEl.classList.remove(fx.cls);
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add(fx.cls);
}
petEl.addEventListener("animationend", () => petEl.classList.remove("boo", "flare"));

// 놀러 보내져 손 흔든 뒤 "뿅" (백엔드 render.poof, 0.4초): 몸이 작아지며 흐려지고 반짝이 몇 개가 퍼진다.
// 다 사라지면 백엔드가 창을 숨긴다 — 돌아와 다시 움직이면 클래스를 뗀다
const POOF_SPARKS = 6;
function poof() {
  petEl.classList.remove("poof");
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add("poof");
  const cy = display * 0.45;
  for (let i = 0; i < POOF_SPARKS; i++) {
    const a = (i / POOF_SPARKS) * Math.PI * 2 + Math.random() * 0.5;
    const r = display * (0.28 + Math.random() * 0.12);
    const s = document.createElement("span");
    s.className = "spark";
    s.textContent = i % 2 ? "✦" : "✧";
    s.style.bottom = cy + "px";
    s.style.setProperty("--dx", `${Math.cos(a) * r}px`);
    s.style.setProperty("--dy", `${-Math.sin(a) * r}px`);
    stageEl.appendChild(s);
    s.addEventListener("animationend", () => s.remove());
  }
}

// 첫 pet-render는 이 웹뷰가 듣기 전에 지나갔을 수 있다 — 첫 로드 땐 slot_info의 render를 쓴다 (이벤트가 먼저 왔으면 그쪽이 새것)
let gotRender = false;
const listeningRender = listen("pet-render", (e) => {
  gotRender = true;
  if (e.payload.anim !== render.anim) animStart = performance.now();
  if (e.payload.heart && !render.heart) {
    heartEl.classList.remove("show");
    void heartEl.offsetWidth; // 애니메이션 재시작
    heartEl.classList.add("show");
  }
  // 트레이에서 먹였거나 다른 길로 먹기 시작하면 고르기 버튼은 필요 없다
  if (e.payload.anim === "eat") speech.hidePicker();
  if (e.payload.anim === "tickle" && render.anim !== "tickle") petFx("tickle");
  if (e.payload.poof && !render.poof) poof();
  // 사라진 뒤(Away = idle 그림)엔 창이 숨을 때까지 그대로 두고, 다시 움직이기 시작하면(돌아옴) 뗀다
  else if (!e.payload.poof && e.payload.anim !== "idle") petEl.classList.remove("poof");
  render = e.payload;
});

// 손님이면 머리 위 띠에 "🏠 <주인>" — 처음 왔을 때 10초, 그 뒤엔 누를 때마다 5초 (말풍선이 뜨는 동안은 가려진다 — style.css)
const NAMETAG_ARRIVE_MS = 10000;
const NAMETAG_CLICK_MS = 5000;
let isGuest = false;
let nametagTimer = null;
function showNametag(ms) {
  if (!isGuest) return;
  nametagEl.hidden = false;
  clearTimeout(nametagTimer);
  nametagTimer = setTimeout(() => (nametagEl.hidden = true), ms);
}
// 손님이 들고 온 편지 💌 (주인이 실어 보낸 것 — 백엔드가 다듬어 slot_info/pet-stage의 note로 준다).
// 안 읽음: 머리 위에서 통통 튄다. 봉투나 몸을 누르면 쪽지 "💌 <주인>: <편지>"를 15초 (이 누름엔 도망·애교 없음).
// 읽은 뒤: 봉투는 작고 흐리게 머리 옆에 남는다 — 봉투를 누르면 다시 15초, 몸을 누르면 평소 손님 놀이.
// 쪽지가 펼쳐진 동안은 수다 말풍선이 덮지 않는다. 글자는 textContent로만 (HTML 아님)
const NOTE_MS = 15000;
let note = null;
let ownerName = "";
let noteRead = false;
let noteOpen = false;
let noteTimer = null;
function openNote() {
  if (!note) return;
  noteRead = true;
  envelopeEl.classList.add("read");
  speech.hideBubble();
  noteEl.textContent = `💌 ${ownerName}: ${note}`;
  clearTimeout(noteTimer);
  noteTimer = setTimeout(closeNote, NOTE_MS);
  if (!noteOpen) {
    noteOpen = true;
    // 창을 위로 키운 뒤(백엔드) 쪽지를 보인다
    invoke("note_space", { open: true }).finally(() => {
      if (noteOpen) noteEl.hidden = false;
    });
  }
}
function closeNote() {
  clearTimeout(noteTimer);
  if (!noteOpen) return;
  noteOpen = false;
  noteEl.hidden = true;
  invoke("note_space", { open: false });
}
// 봉투 누르기는 펫 잡기·손님 놀이로 이어지지 않는다
envelopeEl.addEventListener("mousedown", (e) => {
  e.stopPropagation();
  e.preventDefault();
  if (e.button === 0) openNote();
});

function applyRole(info) {
  const guest = info.role === "guest";
  if (guest) nametagEl.textContent = `🏠 ${info.owner ?? ""}`;
  if (guest) ownerName = info.owner ?? "";
  // 편지는 처음 받은 것 그대로 (손님이 떠날 때까지)
  if (guest && note === null && typeof info.note === "string" && info.note) {
    note = info.note;
    envelopeEl.hidden = false;
  }
  if (guest && !isGuest) {
    isGuest = true;
    showNametag(NAMETAG_ARRIVE_MS);
  } else if (!guest) {
    isGuest = false;
    clearTimeout(nametagTimer);
    nametagEl.hidden = true;
    note = null;
    envelopeEl.hidden = true;
    closeNote();
  }
}

// 동물 교체·성장·리셋: 창 크기는 백엔드가 바꾸고, 여기선 시트와 스프라이트 크기를 맞춘다
const listeningStage = listen("pet-stage", (e) => {
  stage = e.payload;
  applyRole(e.payload);
  loadPet();
});

// pet.json에 없는 상황의 기본 대사 (직접 주기: 배고플 때 손에 든 음식을 봤다)
const EXTRA_SPEECH = {
  eager: ["yum!?"],
  // 다른 동물과 마주침 (pet.json에 없으면 이 말)
  greet: ["hi!", "♪", "hello!"],
  play: ["play!", "tag!"],
  flee: ["catch me!", "hehe!"],
  ignore: ["..."],
  // 놀러 보내져 손 흔들고 떠남 (내 동물 / 이 화면 주인이 다음 친구 화면으로 보낸 손님)
  bye: ["bye~!"],
  // 편지를 들고 떠남
  letter: ["💌!"],
};
const speech = setupSpeech({
  bubbleEl,
  pickerEl,
  favBtn,
  getManifest: () => manifest && { ...manifest, speech: { ...EXTRA_SPEECH, ...manifest.speech } },
  isQuiet: () => noteOpen,
});

// 트레이 "내 친구 코드 보기": 머리 위 띠에 "🔑 ABCD-1234", 누르면 복사 (잠시 뒤 사라진다)
const codetagEl = document.getElementById("codetag");
let codeTimer = null;
function hideCodeLater(ms) {
  clearTimeout(codeTimer);
  codeTimer = setTimeout(() => (codetagEl.hidden = true), ms);
}
listen("show-code", (e) => {
  codetagEl.dataset.code = e.payload;
  codetagEl.textContent = `🔑 ${e.payload}`;
  codetagEl.hidden = false;
  hideCodeLater(20000);
});
// 띠를 누르는 게 펫 잡기로 이어지지 않게 한다
codetagEl.addEventListener("mousedown", (e) => e.stopPropagation());
codetagEl.addEventListener("click", async (e) => {
  e.stopPropagation();
  const ok = await copyText(codetagEl.dataset.code ?? "");
  codetagEl.textContent = ok ? "복사했어요!" : `🔑 ${codetagEl.dataset.code}`;
  hideCodeLater(ok ? 1500 : 20000);
});

petEl.addEventListener("mousedown", (e) => {
  // 안 읽은 편지가 있으면 첫 누름은 쪽지만 연다 (잡기·도망 없음)
  if (e.button === 0 && note && !noteRead) {
    openNote();
    return;
  }
  if (e.button === 0) showNametag(NAMETAG_CLICK_MS);
  if (e.button === 0) invoke("pointer_down");
  if (e.button === 0) petFx("click");
});

// 펫을 우클릭하면 트레이와 같은 메뉴 (손님은 "<주인>에게 돌려보내기"만)
petEl.addEventListener("contextmenu", () => invoke("show_menu"));

blockWebviewDefaults();

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다 — 그 사이 바뀐 것은 이벤트로 온다
  await Promise.all([listeningRender, listeningStage]);
  stage = await invoke("slot_info");
  applyRole(stage);
  if (!gotRender && stage.render) {
    render = stage.render;
    animStart = performance.now();
  }
  await loadPet();
  requestAnimationFrame(draw);
})();
