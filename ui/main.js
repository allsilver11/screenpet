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
  // 창 너비 = 128 × max(1, 배율) (백엔드 window_logical_size) — 리사이즈 도착 전에도 가운데
  petEl.style.left = (128 * Math.max(1, stage.scale) - side) / 2 + "px";
  display = side;
  manifest = m;
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${url})`;
  petEl.style.backgroundSize = `${img.width * k}px ${img.height * k}px`;
  animStart = performance.now();
}

// 꿀벌 쏘기: "!" (백엔드 sting::ALERT_SECS = 0.5초) 뒤 0.3초 돌진 동안 스프라이트를 1 → 1.35 → 1로 키웠다 줄여
// 화면 쪽으로 다가오는 것처럼 보인다
const STING_DASH_START_MS = 500;
const STING_DASH_MS = 300;
const STING_DASH_SCALE = 0.35;

function stingPulse(anim, elapsedMs) {
  const t = (elapsedMs - STING_DASH_START_MS) / STING_DASH_MS;
  if (anim !== "sting" || t <= 0 || t >= 1) return 1;
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
    const pulse = stingPulse(render.anim, elapsed);
    if (pulse !== 1) parts.push(`scale(${pulse})`);
    petEl.style.transform = parts.length ? parts.join(" ") : "none";
  }
  requestAnimationFrame(draw);
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
  render = e.payload;
});

// 손님이면 머리 위 띠에 "🏠 <주인>" (말풍선이 뜨는 동안은 가려진다 — style.css)
function applyRole(info) {
  const guest = info.role === "guest";
  nametagEl.hidden = !guest;
  if (guest) nametagEl.textContent = `🏠 ${info.owner ?? ""}`;
}

// 동물 교체·성장·리셋: 창 크기는 백엔드가 바꾸고, 여기선 시트와 스프라이트 크기를 맞춘다
const listeningStage = listen("pet-stage", (e) => {
  stage = e.payload;
  applyRole(e.payload);
  loadPet();
});

const speech = setupSpeech({ bubbleEl, pickerEl, favBtn, getManifest: () => manifest });

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
  if (e.button === 0) invoke("pointer_down");
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
