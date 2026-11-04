// 동글 펫 그림 확장자: 배포 파일을 만들 때 tools/pack_webp.py가 "webp"로 바꾼다 (저장소에는 PNG)
const PET_ART_EXT = "png";

// 창들이 같이 쓰는 것: 시트 읽기, 프레임 계산, 말풍선, 음식 고르기 (펫·하우스 창), 글 쓰는 창의 키.
// 일반 스크립트끼리는 최상위 const가 겹치면 안 되므로 window.__TAURI__는 함수 안에서만 꺼낸다.

async function decodeSheet(url) {
  const img = new Image();
  img.src = url;
  await img.decode();
  return img;
}

// 픽셀 시트의 단계별 칸 수 (코어 style::pixel_grid, tools/draw_pixel.py STAGE_GRID와 같다 — pet.json에 없을 때)
const PIXEL_DOTS = { baby: 24, kinder: 28, child: 32, teenager: 32, adult: 36 };

// 칸 크기: frame_sizes[단계] → frame_size(표 또는 숫자) → PIXEL_DOTS (코어 manifest::parse_pixel_manifest와 같은 순서)
function pixelFrameSize(px, stageName) {
  const field = px.frame_size;
  return px.frame_sizes?.[stageName] ?? field?.[stageName] ?? (typeof field === "number" ? field : PIXEL_DOTS[stageName] ?? 32);
}

// 픽셀 몸 한 변 (CSS px): 칸 한 변이 물리 px의 정수배가 되도록 맞춘다 (코어 style::pixel_body_scale로 이미 맞춰 온 값을 지금 창의 배율로 한 번 더).
// fit = "floor"면 내림 (문간처럼 넘치면 잘리는 자리)
function snapPixelSide(side, dots, fit = "round") {
  const dpr = window.devicePixelRatio || 1;
  const dot = fit === "floor" ? Math.floor((side / dots) * dpr) : Math.round((side / dots) * dpr);
  return (dots * Math.max(1, dot)) / dpr;
}

// 단계 칸 크기 (논리 px, 전 종 같다 — 동글 stage_size를 곱하지 않는다. 코어 style::pixel_grid, tools/draw_pixel_world.py STAGE_GRID와 같다)
const PIXEL_BASE_DOT = { baby: 3, kinder: 3, child: 3, teenager: 3, adult: 3 };

// 논리 칸 크기 → 물리 px의 정수배가 되는 CSS px (코어 style::dot_physical과 같은 계산)
function snapDot(logical) {
  const dpr = window.devicePixelRatio || 1;
  return Math.max(1, Math.round(logical * dpr)) / dpr;
}

// 지금 그림이 픽셀이면 <body class="pixel"> (style.css: image-rendering pixelated, 흐려지는 효과 대신 계단)
function setPixelBody(on) {
  document.body.classList[on ? "add" : "remove"]("pixel");
}

// 픽셀 그림 (pets_pixel/<id>/pet.json + sheet_<stage>.png)을 동글 pet.json 위에 얹는다: 시트 칸·동작 표·머리·목 표만 그림 것, 대사·이름·집은 그대로
async function loadPixelSheet(id, stageName, round) {
  const px = await (await fetch(`pets_pixel/${id}/pet.json`)).json();
  const url = `pets_pixel/${id}/sheet_${stageName}.png`;
  const img = await decodeSheet(url);
  const manifest = { ...round, frame_size: pixelFrameSize(px, stageName), animations: px.animations, head: px.head, neck: px.neck, neck_stretch: px.neck_stretch };
  return { manifest, url, img, style: "pixel", dir: `pets_pixel/${id}/` };
}

// 동물 id의 pet.json과 단계별 시트 sheet_<stage>.png (없으면 sheet.png) → { manifest, url, img, style, dir }.
// style "pixel"인데 그림을 못 읽으면 동글로. 돌려주는 manifest.frame_size는 이 단계 시트의 칸 크기다
async function loadPetSheet(id, stageName, style = "round") {
  const manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  if (style === "pixel") {
    try {
      return await loadPixelSheet(id, stageName, manifest);
    } catch {
      // 아래 동글로
    }
  }
  let url = `pets/${id}/sheet_${stageName}.${PET_ART_EXT}`;
  let img;
  // 크게 보이는 단계는 시트 칸이 2배다 (frame_sizes, tools/sprite_spec.py frame_px). 표시 크기는 칸과 상관없다 — head·neck·neck_stretch 표도 이 칸의 px
  let frameSize = manifest.frame_sizes?.[stageName] ?? manifest.frame_size;
  try {
    img = await decodeSheet(url);
  } catch {
    url = `pets/${id}/sheet.${PET_ART_EXT}`; // sheet.png는 sheet_teenager.png와 같은 그림
    img = await decodeSheet(url);
    frameSize = manifest.frame_sizes?.teenager ?? manifest.frame_size;
  }
  return { manifest: { ...manifest, frame_size: frameSize }, url, img, style: "round", dir: `pets/${id}/` };
}

function frameIndex(spec, elapsedMs) {
  // 이벤트가 이번 프레임 시각 뒤에 와서 animStart가 더 늦을 수 있다 — 음수 칸은 없다
  const n = Math.max(0, Math.floor((elapsedMs / 1000) * spec.fps));
  return spec.loop === false ? Math.min(n, spec.frames - 1) : n % spec.frames;
}

// 큰 점프(백엔드 anim "bigjump") 그림이 없는 동물은 hop 줄로 대신한다: hop을 그냥 틀면 땅 위에서 미리 떠 버리니까,
// 웅크림(hop 0)을 코어 hop::BIG_JUMP_CROUCH_SECS 동안 버티다가 뜨는 순간 hop 1, 그다음 hop 2로 내려설 때까지
const LEAP_CROUCH_MS = 500;
function hopLeapFrame(spec, elapsedMs) {
  if (elapsedMs < LEAP_CROUCH_MS) return 0;
  const n = Math.floor(((elapsedMs - LEAP_CROUCH_MS) / 1000) * spec.fps);
  return Math.min(1 + n, spec.frames - 1);
}

// 값이 그대로면 쓰지 않는 스타일 쓰기 (rAF는 화면 주사율마다 돈다 — 칸이 그대로인 프레임은 아무것도 건드리지 않는다).
// 이 요소의 그 속성은 이 함수로만 쓴다
function styleWriter(el) {
  const shown = {};
  return (prop, value) => {
    if (shown[prop] === value) return;
    shown[prop] = value;
    el.style[prop] = value;
  };
}

// 말풍선(`pet-say`)과 음식 고르기(`food-picker`). 백엔드는 지금 동물이 있는 창에만(emit_to) 보낸다.
// getManifest() = 지금 동물의 pet.json (대사 목록). 돌려주는 hidePicker는 먹기 시작하면 부른다.
// isQuiet() = 참이면 다른 말풍선·음식 고르기를 띄우지 않는다 (손님 편지를 말풍선에 보이는 동안 — 무엇도 덮지 않게, 그냥 버린다).
// hideBubble은 편지를 띄우기 전에 부른다 (남은 숨김 타이머도 끈다)
// 곁에 있기·하루 일과·집중 친구의 말 (코어 brain_presence.rs) — 펫 창과 하우스 창이 같이 쓴다. pet.json에 있으면 그쪽이 먼저
const RHYTHM_SPEECH = {
  welcome_back: ["you're back!!"],
  good_morning: ["good morning!"],
  lunch_time: ["lunch time?"],
  go_to_bed: ["go to bed~"],
  yawn: ["*yaaawn*", "yawn~"],
  break_time: ["break time!"],
  focus: ["focus!"],
};

function setupSpeech({ bubbleEl, pickerEl, favBtn, getManifest, isQuiet = () => false }) {
  const { invoke } = window.__TAURI__.core;
  // 전역 event.listen은 다른 창 앞으로 보낸(emit_to) 이벤트까지 받는다 — 이 창 앞으로 온 것만 듣는다
  const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
  const listen = (event, handler) => here.listen(event, handler);
  let bubbleTimer = null;
  let pickerTimer = null;

  function hidePicker() {
    pickerEl.hidden = true;
    clearTimeout(pickerTimer);
  }

  // 백엔드가 고른 상황(chatter/tickle/grumpy/hiss/hungry/full/level_up/ignore)에 맞는 대사를
  // pet.json speech에서 랜덤으로 띄운다. 그 동물에 없는 상황이면 말풍선 없음.
  listen("pet-say", (e) => {
    const lines = getManifest()?.speech?.[e.payload] ?? RHYTHM_SPEECH[e.payload] ?? [];
    if (!lines.length || !pickerEl.hidden || isQuiet()) return; // 음식 고르기 버튼·편지 쪽지가 우선
    bubbleEl.textContent = lines[Math.floor(Math.random() * lines.length)];
    bubbleEl.hidden = false;
    clearTimeout(bubbleTimer);
    bubbleTimer = setTimeout(() => {
      bubbleEl.hidden = true;
    }, 1500);
  });

  // 백엔드가 정한 글자 그대로 (친구가 데려갈 때 "off to <name>'s~ brb!", 부탁 거절 "<name>: busy~")
  listen("pet-say-text", (e) => {
    if (typeof e.payload !== "string" || !e.payload || !pickerEl.hidden || isQuiet()) return;
    bubbleEl.textContent = e.payload;
    bubbleEl.hidden = false;
    clearTimeout(bubbleTimer);
    bubbleTimer = setTimeout(() => {
      bubbleEl.hidden = true;
    }, 2500);
  });

  // 배고플 때 클릭하면 머리 위에 🍚 (좋아하는 것) 🍱 버튼 — 간식·물은 트레이·우클릭 메뉴에서 언제든
  listen("food-picker", (e) => {
    if (isQuiet()) return; // 편지 말풍선이 우선
    favBtn.textContent = e.payload.emoji;
    favBtn.title = e.payload.name;
    bubbleEl.hidden = true;
    pickerEl.hidden = false;
    clearTimeout(pickerTimer);
    pickerTimer = setTimeout(hidePicker, 6000);
  });

  // 하우스로 가기 시작하면 펫 창의 음식 고르기는 닫는다
  listen("hide-picker", hidePicker);

  for (const btn of pickerEl.querySelectorAll("button")) {
    // 버튼을 누르는 게 펫 잡기(하우스 클릭)로 이어지지 않게 한다
    btn.addEventListener("mousedown", (e) => e.stopPropagation());
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("feed", { food: btn.dataset.food });
      hidePicker();
    });
  }

  function hideBubble() {
    bubbleEl.hidden = true;
    clearTimeout(bubbleTimer);
  }

  return { hidePicker, hideBubble };
}

// 글을 쓰는 창과 도감: 우클릭 메뉴·새로 고침만 막고 타자는 그대로. Enter는 글 넣기 (버튼 위면 그 버튼만 눌리게, 한글 조합 중이면 글자 확정), Esc는 닫기
function setupFormKeys({ submit, close } = {}) {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      if (!e.isComposing && e.target.tagName !== "BUTTON") submit?.();
    } else if (e.key === "Escape") close?.();
    else if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) e.preventDefault();
  });
}

// 키보드 UI가 없는 창: 웹뷰 기본 동작(우클릭 메뉴, F5, Ctrl+R, Alt+F4 등)을 막는다.
function blockWebviewDefaults() {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("keydown", (e) => e.preventDefault());
}

// 글자를 클립보드에 (펫 머리 위 친구 코드 띠, 코드 창). 클릭 처리기 안에서 부른다. 성공하면 true
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // 예전 방식: 숨긴 입력칸을 골라 복사
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {}
    ta.remove();
    return ok;
  }
}
