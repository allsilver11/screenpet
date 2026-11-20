// 백엔드가 보내는 {anim, flip, heart}를 받아 스프라이트 프레임만 그림. (common.js를 먼저 읽음)
// 내 동물 창("pet")과 손님 창("guest-<id>")이 같이 씀 — 자기 동물은 창 라벨로 `slot_info`에 물음.
const { invoke } = window.__TAURI__.core;
// 전역 event.listen은 다른 창 앞으로 보낸(emit_to) 이벤트까지 받음 — 이 창 앞으로 온 것만 들음
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
// 이 창 앞으로 온 이벤트만 들음
const listen = (event, handler) => here.listen(event, handler);

const petEl = document.getElementById("pet");
const heartEl = document.getElementById("heart");
const dizzyEl = document.getElementById("dizzy");
const cocoonEl = document.getElementById("cocoon");
const bubbleEl = document.getElementById("bubble");
const pickerEl = document.getElementById("picker");
const favBtn = document.getElementById("fav");
const nametagEl = document.getElementById("nametag");
const stageEl = document.getElementById("stage");
const envelopeEl = document.getElementById("envelope");
const ctltagEl = document.getElementById("ctltag");

let manifest = null;
let display = 128;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();
// 지금 동물과 몸 크기 배율 (단계 배율 × 동물별 배율) — 백엔드 `pet-stage`
let stage = { pet: null, stage: "teenager", scale: 1 };
let loadSeq = 0;

// 단계별 시트 sheet_<stage>.png
async function loadPet() {
  const seq = ++loadSeq;
  const { manifest: m, url, img, dir } = await loadPetSheet(stage.pet, stage.stage);
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  sheetImg = img;
  headCache.clear();
  headgearAt = null;
  const side = 128 * stage.scale;
  petEl.style.width = side + "px";
  // 창 가운데 (창 너비는 128 × max(1, 배율), 편지 쪽지를 펼친 동안은 더 넓음 — 백엔드가 몸을 창 가운데에 둠)
  petEl.style.left = `calc(50% - ${side / 2}px)`;
  stageEl.style.setProperty("--pet-h", side + "px");
  display = side;
  manifest = m;
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${url})`;
  petEl.style.backgroundSize = `${img.width * k}px ${img.height * k}px`;
  setupTint(m, dir, `${img.width * k}px ${img.height * k}px`);
  // 동물이 바뀌면 모자 ↔ 목도리 (pet-presence는 그대로라 다시 오지 않음)
  if (lastPresence) {
    setCap(!!lastPresence.nightcap);
    applyRing();
  }
  animStart = performance.now();
}

// 🎨 몸 색 (백엔드 pet-stage `tint` — 코어 tint.rs, 메뉴 "🎨 흐물이 색"): 사용자 — 어두운 바탕화면에서 짙은 잉크 선이 안 보임.
// 잉크 선으로 그린 동물(pet.json tint)만: 고른 색을 칠한 #tint 층을 시트의 잉크 자리(ink_<stage>, tools/draw_heumul.py)로 가려 몸 위에 얹음.
// 층은 #pet의 자식이라 반전·구르기·매달림·덜덜을 그대로 받고, 마스크 칸은 draw()가 몸 칸과 같이 옮김. 모르는 색이면 그림 그대로
const tintEl = document.getElementById("tint");
const writeTint = styleWriter(tintEl);
// 어두운·밝은 바탕 모두에서 보이는 색 (흰색은 크림색 머리와 구별되게 살짝 푸른 회색빛)
const TINT_COLORS = { white: "#e6ebf4", red: "#ff5b5b", orange: "#ff9838", yellow: "#f7cc2f", green: "#45cc78", sky: "#48c0f5", blue: "#4d7bff", purple: "#a46bff", pink: "#ff76bb" };
// 🌈 머리부터 발끝까지 (빨주노초하파보) — 천천히 흘러내림
const RAINBOW = ["#ff5b5b", "#ff9838", "#f7cc2f", "#45cc78", "#48c0f5", "#4d7bff", "#a46bff"];
let tintOn = null;

// 🎨 몸 색 덧칠을 준비함 (색을 고를 수 있는 동물이고 아는 색일 때만)
function setupTint(m, dir, size) {
  const id = stage.tint;
  tintOn = m.tint && (id === "rainbow" || Object.hasOwn(TINT_COLORS, id)) ? id : null;
  tintEl.hidden = !tintOn;
  if (!tintOn) return;
  const mask = `url(${dir}ink_${stage.stage}.${PET_ART_EXT})`;
  for (const [prop, value] of [["maskImage", mask], ["maskSize", size], ["maskRepeat", "no-repeat"]]) {
    writeTint(prop, value);
    writeTint("webkit" + prop[0].toUpperCase() + prop.slice(1), value);
  }
  if (tintOn !== "rainbow") writeTint("background", TINT_COLORS[tintOn]);
}

// 무지개 한 바퀴 = 몸 높이쯤 (칸의 0.75)
function rainbowFill(now) {
  const band = Math.max(1, Math.round((display * 0.75) / RAINBOW.length));
  const period = band * RAINBOW.length;
  const shift = (Math.floor(now / 200) * 2) % period;
  const at = (i) => `${i * band + shift - period}px`;
  const stops = [...RAINBOW.map((c, i) => `${c} ${at(i)}`), `${RAINBOW[0]} ${at(RAINBOW.length)}`];
  return `repeating-linear-gradient(180deg, ${stops.join(", ")})`;
}

// 몸 색 덧칠 마스크를 지금 프레임 자리로 옮김
function drawTint(pos, now) {
  if (!tintOn) return;
  writeTint("maskPosition", pos);
  writeTint("webkitMaskPosition", pos);
  if (tintOn === "rainbow") writeTint("background", rainbowFill(now));
}

// 공룡 쿵쾅 (v0.3.6): 쿵! 하는 그림(stomp 1·3장)마다 몸이 덜덜, 쿵쾅에 흔들린 친구(백엔드 render.wobble, 0.4초)도 덜덜 —
// 스프라이트만 살짝 옮김 (창은 그대로)
function jitter(f, now) {
  const stomping = render.anim === "stomp" && f % 2 === 1;
  if (!stomping && !render.wobble) return null;
  const amp = (stomping ? 1.5 : 2.5) * Math.sqrt(stage.scale);
  return `translate(${(Math.sin(now / 17) * amp).toFixed(2)}px, ${(Math.cos(now / 23) * amp * 0.5).toFixed(2)}px)`;
}

// 수면 모자·집중 고리 (백엔드 pet-presence {nightcap, ring}): 둘 다 #pet의 자식이라 몸의 변환(반전·구르기·매달림·덜덜·쏘기·뿅)과
// 창 크기 변화를 따로 계산하지 않아도 한 덩어리로 움직임. 여기서는 칸 안의 자리만 정함 — pet.json head[단계][동작][칸] = [x, y, w, r]:
// 그림을 그린 코드(tools/draw_pets.py mark_head)가 적어 둔 모자 입구 가운데 · 입구 너비 (칸 px) · 머리 기울기 (도). 칸이 바뀔 때만 옮김
const nightcapEl = document.getElementById("nightcap");
const focusringEl = document.getElementById("focusring");
const focusringFill = document.getElementById("focusring-fill");
// 밤 목도리: pet.json에 neck 표가 있는 동물(산타 — 모자를 이미 썼음)은 수면 모자 대신 두름. 자리·몸과 한 덩어리인 것은 모자와 같음
const scarfEl = document.getElementById("scarf");
let headgearAt = null;

// HEADGEAR-LAYOUT-START (tools/test_nightcap.py가 node로 이 부분을 그대로 돌림)
// 모자 SVG(ui/index.html)의 입구 가운데와 너비 (viewBox 좌표)
const CAP_BRIM_X = 16.5;
const CAP_BRIM_Y = 20.5;
const CAP_BRIM_W = 27;
const CAP_TILT = -8; // 끝이 머리 뒤로 처지게 (칸은 오른쪽을 보고, 그림의 방울은 오른쪽이라 뒤집어 쓴다)
const RING_R = 10; // 고리 SVG 반지름 (px)
const HEAD_FALLBACK = [64, 24, 44, 0]; // head 표가 없는 동물: 칸 위쪽 가운데

// k = 표시 px / 칸 px. 고리는 머리 위 (모자를 쓰면 모자 위)에 뜸. 반전된 몸 안에서도 진행 방향은 그대로 (scaleX로 되돌림)
function headgearLayout(entry, k, capOn, flip) {
  const [x, y, w, r] = entry ?? HEAD_FALLBACK;
  const s = (w * k) / CAP_BRIM_W;
  const cap = `translate(${(x * k).toFixed(2)}px, ${(y * k).toFixed(2)}px) rotate(${(r + CAP_TILT).toFixed(1)}deg) scale(${(-s).toFixed(3)}, ${s.toFixed(3)}) translate(${-CAP_BRIM_X}px, ${-CAP_BRIM_Y}px)`;
  const rad = (r * Math.PI) / 180;
  const lift = (capOn ? 0.6 : 0.3) * w * k + RING_R + 2;
  const rx = x * k + Math.sin(rad) * lift;
  const ry = y * k - Math.cos(rad) * lift;
  const ring = `translate(${(rx - RING_R).toFixed(2)}px, ${(ry - RING_R).toFixed(2)}px)${flip ? " scaleX(-1)" : ""}`;
  return { cap, ring };
}
// 앞모습 동작 (코어 Anim::is_front) — 머리가 가운데임. 옆모습은 시트에서 오른쪽을 봄
const FRONT_ANIMS = new Set(["sit_front", "wave", "tilt", "belly", "stretch", "heart", "tickle", "hungry", "eat", "level_up", "huff"]);
// 목도리 SVG(ui/index.html #scarf)의 기준점과 너비 (viewBox 좌표)
const SCARF_ANCHOR_X = 26;
const SCARF_ANCHOR_Y = 8;
const SCARF_W = 52;
// 목도리 그림 자리(translate·rotate·scale)를 계산함
function scarfLayout(entry, k) {
  const [x, y, w, r] = entry;
  const s = (w * k) / SCARF_W;
  return `translate(${(x * k).toFixed(2)}px, ${(y * k).toFixed(2)}px) rotate(${r.toFixed(1)}deg) scale(${s.toFixed(3)}) translate(${-SCARF_ANCHOR_X}px, ${-SCARF_ANCHOR_Y}px)`;
}
// 지금 칸에 보일 모양 (썰매는 초록 목도리의 펄럭임과 칸마다 같은 모양으로 덮음)
function scarfView(anim, f) {
  if (anim === "roll") return `sled${f % 2}`;
  if (anim === "climb" || anim === "slide") return "back";
  return FRONT_ANIMS.has(anim) ? "front" : "side";
}
// 매달린 몸이 흔들리는 축 = 커서 끝이 닿는 머리 꼭대기 (코어 PetManifest::hang_grip과 같은 계산: dragged 칸들의 평균, 모자 입구보다 조금 위)
const HEAD_TOP_ABOVE_BRIM = 0.15;
// 🎣 hang_cursor 손 자리 (칸 px [x, y]) → transform-origin
function gripOrigin(cell) {
  const size = manifest.frame_sizes?.[stage.stage] ?? manifest.frame_size;
  return `${((cell[0] / size) * 100).toFixed(2)}% ${((cell[1] / size) * 100).toFixed(2)}%`;
}
// 🕺 춤 반짝이 간격
const SPARKLE_MS = 220;
let lastSparkle = 0;
// 수면 모자가 매달리는 기준점 (머리 칸들의 평균에서)
function hangOrigin(cells, frameSize) {
  if (!cells?.length) return "50% 6%";
  const mean = (i) => cells.reduce((a, c) => a + c[i], 0) / cells.length;
  const y = Math.max(0, mean(1) - HEAD_TOP_ABOVE_BRIM * mean(2));
  return `${((mean(0) / frameSize) * 100).toFixed(2)}% ${((y / frameSize) * 100).toFixed(2)}%`;
}
// HEADGEAR-LAYOUT-END

// 아래는 눈사람 눈구름(placeCloud)이 쓰는 머리 찾기 — 시트의 지금 칸을 재서 맨 위를 찾음 (칸마다 한 번 재서 기억)
const headCache = new Map();
let sheetImg = null;

// 불투명한 덩어리 중 가장 큰 것 (zzz·반짝이처럼 떨어진 조각은 머리가 아님) → 칸 안 칸번호 목록
function largestBlob(data, size) {
  const seen = new Uint8Array(size * size);
  const stack = [];
  let best = [];
  for (let start = 0; start < seen.length; start++) {
    if (seen[start] || data[start * 4 + 3] <= 128) continue;
    const blob = [];
    seen[start] = 1;
    stack.push(start);
    while (stack.length) {
      const c = stack.pop();
      blob.push(c);
      const x = c % size;
      const near = [x > 0 ? c - 1 : -1, x < size - 1 ? c + 1 : -1, c - size, c + size];
      for (const n of near) {
        if (n >= 0 && n < seen.length && !seen[n] && data[n * 4 + 3] > 128) {
          seen[n] = 1;
          stack.push(n);
        }
      }
    }
    if (blob.length > best.length) best = blob;
  }
  return best;
}

// 칸 (row, f)의 머리 꼭대기: 칸 왼쪽 위 기준 {x, y} (시트 px). 몸통 덩어리에서, 옆모습이면 앞쪽(오른쪽) 45%만 봄
function headTop(row, f, front) {
  const key = `${row}:${f}`;
  if (headCache.has(key)) return headCache.get(key);
  const size = manifest.frame_size;
  let top = { x: size / 2, y: size * 0.15 };
  try {
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = size;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(sheetImg, f * size, row * size, size, size, 0, 0, size, size);
    const blob = largestBlob(ctx.getImageData(0, 0, size, size).data, size);
    if (blob.length) {
      const xs = blob.map((c) => c % size);
      const [lo, hi] = [Math.min(...xs), Math.max(...xs)];
      const from = front ? lo : hi - (hi - lo) * 0.45;
      const part = blob.filter((c) => c % size >= from);
      const first = Math.min(...part.map((c) => Math.floor(c / size)));
      // 맨 위 줄부터 조금 아래까지의 가운데 (귀 끝 한 점에 쏠리지 않게)
      const band = part.filter((c) => Math.floor(c / size) < first + size * 0.08).map((c) => c % size);
      top = { x: band.reduce((a, b) => a + b, 0) / band.length, y: first };
    }
  } catch {
    // 시트를 읽지 못하면 칸 위쪽 가운데
  }
  headCache.set(key, top);
  return top;
}

// 모자·고리는 #pet의 자식 (몸과 한 덩어리)
function placeHeadgear(anim, f) {
  if (nightcapEl.parentNode !== petEl) petEl.append(nightcapEl, focusringEl, scarfEl);
  const wearing = !!lastPresence.nightcap;
  const scarf = manifest.neck?.[stage.stage]?.[anim]?.[f];
  const capOn = wearing && !manifest.neck;
  const key = `${stage.stage}:${anim}:${f}:${render.flip}:${display}:${capOn}:${wearing}`;
  if (key === headgearAt) return;
  headgearAt = key;
  // 표가 없는 칸: 칸 위쪽 가운데 (HEAD_FALLBACK은 128칸 기준이라 이 단계 시트의 칸 크기로 옮김)
  const entry = manifest.head?.[stage.stage]?.[anim]?.[f] ?? HEAD_FALLBACK.map((v, i) => (i < 3 ? (v * manifest.frame_size) / 128 : v));
  const { cap, ring } = headgearLayout(entry, display / manifest.frame_size, capOn, render.flip);
  nightcapEl.style.transform = cap;
  focusringEl.style.transform = ring;
  if (wearing && scarf) {
    scarfEl.style.transform = scarfLayout(scarf, display / manifest.frame_size);
    scarfEl.dataset.view = scarfView(anim, f);
  }
}

// 눈사람의 작은 눈구름 (백엔드 render.cloud): 떠 있는 동안만 프레임이 바뀔 때 머리 위로 옮김 — 눈송이는 CSS.
// 구름은 창 안 (말풍선 자리까지)에 머묾. 창 맨 위를 넘지 않게 자름
const snowcloudEl = document.getElementById("snowcloud");
const CLOUD_GAP = 14;
let cloudAt = null;
// 눈구름을 지금 프레임의 머리 위에 놓음 (바뀌었을 때만)
function placeCloud(row, f) {
  const key = `${row}:${f}:${render.flip}:${display}`;
  if (key === cloudAt) return;
  const fresh = cloudAt === null;
  cloudAt = key;
  const k = display / manifest.frame_size;
  // 뒷모습은 머리가 가운데 — 앞모습처럼 덩어리 전체에서 찾음
  const head = headTop(row, f, FRONT_ANIMS.has(render.anim) || render.anim === "climb" || render.anim === "slide");
  const size = Math.sqrt(stage.scale);
  const x = (render.flip ? manifest.frame_size - head.x : head.x) * k + (stageEl.clientWidth - display) / 2;
  const y = Math.min(display - head.y * k + CLOUD_GAP * size, stageEl.clientHeight - 37 * size);
  // 처음 뜰 땐 미끄러져 오지 않게 바로 그 자리에 (이후 프레임만 둥실 따라옴)
  if (fresh) snowcloudEl.style.transition = "none";
  snowcloudEl.style.transform = `translate(${(x - 28).toFixed(1)}px, ${(-y).toFixed(1)}px) scale(${size.toFixed(2)})`;
  if (fresh) {
    void snowcloudEl.offsetWidth;
    snowcloudEl.style.transition = "";
  }
}
// 눈구름을 보이거나 숨김
function syncCloud(on) {
  if (snowcloudEl.hidden === !on) return;
  snowcloudEl.hidden = !on;
  cloudAt = null;
}

// 뿅 하고 튀는 반짝이 하나 (동글 그림). 애니메이션이 끝나면 지움
function spark(i, bottom, dx, dy, marginLeft) {
  const s = document.createElement("span");
  s.className = "spark";
  s.textContent = i % 2 ? "✦" : "✧";
  s.style.bottom = bottom + "px";
  if (marginLeft !== undefined) s.style.marginLeft = marginLeft + "px";
  s.style.setProperty("--dx", `${dx}px`);
  s.style.setProperty("--dy", `${dy}px`);
  stageEl.appendChild(s);
  s.addEventListener("animationend", () => s.remove());
}

// 🙌 하이파이브 짝! (v0.6.3, 백엔드 render.clap이 켜지는 순간): 몸 앞쪽 위(맞댄 손)에서 반짝이가 작게 터짐 —
// 마주 본 짝도 제 앞에 터뜨려 둘의 반짝이가 손 사이에서 만남. 오른쪽을 보면 앞 = 오른쪽 (flip이면 왼쪽)
const CLAP_SPARKS = 5;
// 박수 반짝이를 터뜨림 (창이 가려져 있으면 안 함)
function clapBurst(flip) {
  if (document.visibilityState === "hidden") return;
  const front = display * 0.3 * (flip ? -1 : 1);
  const bottom = display * 0.62;
  for (let i = 0; i < CLAP_SPARKS; i++) {
    const a = (i / CLAP_SPARKS) * Math.PI * 2 + Math.random() * 0.4;
    const r = display * (0.12 + Math.random() * 0.08);
    spark(i, bottom, Math.cos(a) * r, -Math.sin(a) * r, front);
  }
}

// 🐛 고치 (백엔드 render.cocoon = 감긴 실 겹 수, 코어 brain_snare.rs): 새 그림 없이 몸 위에 하얀 실 띠를 겹 수만큼 비스듬히 두르고
// (style.css #cocoon — 고치째 버둥버둥), 다 풀리면 (0으로 돌아가면) 하얀 반짝이가 뿅 퍼짐
const COCOON_POP_BITS = 6;
// 지금 보이는 띠 겹 수 (바뀔 때만 DOM을 만짐)
let cocoonShown = 0;
// 실 띠를 겹 수만큼 보임 (0이면 숨김)
function syncCocoon(bands) {
  if (bands === cocoonShown) return;
  cocoonShown = bands;
  cocoonEl.hidden = !bands;
  Array.from(cocoonEl.children).forEach((band, i) => {
    band.hidden = i >= bands;
  });
}
// 고치가 풀리며 뿅
function cocoonPop() {
  if (document.visibilityState === "hidden") return;
  const cy = display * 0.4;
  for (let i = 0; i < COCOON_POP_BITS; i++) {
    const a = (i / COCOON_POP_BITS) * Math.PI * 2 + Math.random() * 0.5;
    const r = display * (0.3 + Math.random() * 0.1);
    spark(i, cy, Math.cos(a) * r, -Math.sin(a) * r);
  }
}

// CSS 효과가 끝나지 않아도(창이 숨겨짐 등) 모자는 반드시 사라짐
const CAP_POOF_MS = 450;
const CAP_POOF_SPARKS = 5;
let capPoofTimer = null;
// 밤에 쓰는 것: 대개 수면 모자, neck 표가 있는 동물은 목도리
const nightWear = () => (manifest?.neck ? scarfEl : nightcapEl);
// 수면 모자(또는 목도리)를 씌우거나 벗김
function setCap(on) {
  clearTimeout(capPoofTimer);
  const wear = nightWear();
  for (const el of [nightcapEl, scarfEl]) {
    el.classList.remove("cap-poof");
    el.toggleAttribute("hidden", !on || wear !== el);
  }
}
// 집중 고리: 집중 중일 때만 보임
function applyRing() {
  const ring = typeof lastPresence?.ring === "number";
  focusringEl.toggleAttribute("hidden", !ring);
  if (!ring) return;
  focusringFill.setAttribute("stroke-dashoffset", String(100 - lastPresence.ring));
}
// 모자가 벗겨질 때 뿅 하는 효과
function capPoof() {
  const wear = nightWear();
  const box = wear.getBoundingClientRect();
  const home = stageEl.getBoundingClientRect();
  const cx = box.left + box.width / 2 - home.left - home.width / 2;
  const up = home.bottom - (box.top + box.height / 2);
  for (let i = 0; i < CAP_POOF_SPARKS; i++) {
    const a = (i / CAP_POOF_SPARKS) * Math.PI * 2 + Math.random() * 0.5;
    const r = 14 + Math.random() * 10;
    spark(i, up, Math.cos(a) * r, -Math.sin(a) * r, cx);
  }
  wear.classList.add("cap-poof");
  capPoofTimer = setTimeout(() => setCap(false), CAP_POOF_MS);
}

let lastPresence = null;
listen("pet-presence", (e) => {
  const p = e.payload ?? {};
  if (lastPresence?.nightcap === p.nightcap && lastPresence?.ring === p.ring) return;
  // 막 벗는 순간에만 뿅 (앱을 8시 30분 뒤에 켰거나 숨어 있으면 효과 없이 안 보임) — 시계가 끄므로 인사가 미뤄져도 모자만 남지 않음
  const capOff = lastPresence?.nightcap && !p.nightcap;
  lastPresence = p;
  if (capOff && !nightWear().hidden && document.visibilityState !== "hidden") capPoof();
  else setCap(!!p.nightcap);
  applyRing();
  headgearAt = null;
});

const writePet = styleWriter(petEl);
// ⌨️ render.tap → type 칸: 0 두 손 쉼 · -1 화면 왼쪽 손 톡 · +1 오른쪽 손 톡 (tools/draw_heumul.py hm_type)
const TYPE_FRAME = { 0: 0, "-1": 1, 1: 2 };

// 매 프레임 지금 애니메이션의 칸을 그림
function draw(now) {
  if (manifest) {
    // 교체된 동물에 없는 종 전용 애니메이션이어도 throw로 rAF가 끊기지 않게 idle로 대체
    // 큰 점프 그림이 없는 동물은 hop 줄로 (common.js hopLeapFrame)
    const hopLeap = render.anim === "bigjump" && !manifest.animations.bigjump;
    const spec = hopLeap ? manifest.animations.hop : (manifest.animations[render.anim] ?? manifest.animations.idle);
    const elapsed = Math.max(0, now - animStart);
    // ⌨️ 흐물이 타자 응원: type 칸은 시간이 아니라 코어가 고른 손으로 (render.tap — 내가 키를 누를 때마다 바뀜)
    const f = hopLeap ? hopLeapFrame(spec, elapsed) : render.anim === "type" ? Math.min(TYPE_FRAME[render.tap] ?? 0, spec.frames - 1) : frameIndex(spec, elapsed);
    const framePos = `${-f * display}px ${-spec.row * display}px`;
    writePet("backgroundPosition", framePos);
    drawTint(framePos, now);
    if (lastPresence && (lastPresence.nightcap || typeof lastPresence.ring === "number")) {
      const shown = hopLeap ? "hop" : manifest.animations[render.anim] ? render.anim : "idle";
      placeHeadgear(shown, f);
    }
    if (sheetImg && render.cloud) placeCloud(spec.row, f);
    // 👜 구슬 가방 (그 칸의 손 자리에)
    drawBag(render.bag || 0, manifest.animations[render.anim] ? render.anim : "idle", f);
    // 좌우 반전 공간 안에서 돌리므로 왼쪽으로 갈 땐 자동으로 반대로 구름
    const parts = [];
    const shake = jitter(f, now);
    if (shake) parts.push(shake);
    // 커서에 매달림 (백엔드 render.hang): 창은 머리 꼭대기가 커서 끝에 닿는 자리에 있음 (코어 hang_spot) — 커서 끝을 축으로 살랑살랑
    // 🎣 흐물이가 두 손으로 매달려 흔들흔들 (render.swing 도): 두 손(pet.json grip hang_cursor) = 커서 끝을 축으로 돌림
    const swingGrip = render.anim === "hang_cursor" ? manifest.grip?.[stage.stage]?.hang_cursor?.[f] : null;
    writePet("transformOrigin", swingGrip ? gripOrigin(swingGrip) : render.hang ? hangOrigin(manifest.head?.[stage.stage]?.dragged, manifest.frame_size) : "");
    if (render.hang) parts.push(`rotate(${(Math.sin(now / 430) * 6).toFixed(2)}deg)`);
    if (swingGrip && render.swing) parts.push(`rotate(${render.swing.toFixed(2)}deg)`);
    // 🕺 가장 신나는 춤 (render.sparkle): 반짝이가 퐁퐁
    if (render.sparkle && now - lastSparkle > SPARKLE_MS) {
      lastSparkle = now;
      const a = Math.random() * Math.PI * 2;
      const r = display * (0.3 + Math.random() * 0.15);
      spark(Math.floor(Math.random() * 2), display * 0.5, Math.cos(a) * r, -Math.sin(a) * r);
    }
    if (render.flip) parts.push("scaleX(-1)");
    // 이동 속도는 √배율, 몸 반지름은 배율에 비례 → 구르는 각속도는 1/√배율
    if (spec.spin) parts.push(`rotate(${((elapsed / 1000) * spec.spin / Math.sqrt(stage.scale)) % 360}deg)`);
    writePet("transform", parts.length ? parts.join(" ") : "none");
  }
  requestAnimationFrame(draw);
}

// 할로윈·크리스마스 동물의 짧은 효과 (CSS 애니메이션 한 번, style.css .flare-*/.nose-* — 끝나면 클래스를 뗌):
// 호박은 누르면(잡을 때도) 빛이 2초쯤 번짐 —
// 누를 때마다 지난번과 다른 무늬 (cls가 배열이면 그중 하나를 고르되 직전 것은 빼고)
const PET_FX = {
  pumpkin: { on: "click", cls: ["flare-pulse", "flare-flicker", "flare-bloom", "flare-sparkle", "flare-beat"] },
  // 루돌프: 누르면 빨간 코가 환하게 — 따뜻한 빨간 빛이 번지거나, 반짝이와 함께 깜빡깜빡
  rudolph: { on: "click", cls: ["nose-glow", "nose-twinkle"] },
  // 펭귄: 누르면(잡을 때도), 그리고 장난칠 때 (백엔드 render.flash — 커서에 닿음·친구와 놀기·가끔, brain_flash.rs) "안경 번쩍" —
  // 렌즈만 하얗게 번쩍 + ✦ (index.html #lens-flash 필터가 그림이 표시해 둔 렌즈 자리만 고름)
  penguin: { on: ["click", "play"], cls: "glasses-flash" },
};
const FX_CLASSES = Object.values(PET_FX).flatMap((fx) => [].concat(fx.cls));
let lastFx = null;
// 동물별 특수 효과(PET_FX)를 trigger에 맞춰 켬
function petFx(trigger) {
  const fx = PET_FX[stage.pet];
  if (!fx || ![].concat(fx.on).includes(trigger)) return;
  const pool = [].concat(fx.cls);
  const pick = pool.length > 1 ? pool.filter((c) => c !== lastFx) : pool;
  const cls = pick[Math.floor(Math.random() * pick.length)];
  lastFx = cls;
  petEl.classList.remove(...FX_CLASSES);
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add(cls);
}
// 반짝이(::before/::after)의 animationend도 올라옴 — 몸 자체 애니메이션이 끝날 때만 뗌
petEl.addEventListener("animationend", (e) => {
  if (e.target === petEl && !e.pseudoElement) petEl.classList.remove(...FX_CLASSES);
});

// 놀러 보내져 손 흔든 뒤 "뿅" (백엔드 render.poof, 0.4초): 몸이 작아지며 흐려지고 반짝이 몇 개가 퍼짐.
// 다 사라지면 백엔드가 창을 숨김 — poof가 아닌 render가 오면(취소·돌아옴) 바로 클래스를 뗌
const POOF_SPARKS = 6;
// 뿅 사라지는 애니메이션을 다시 시작함
function poof() {
  petEl.classList.remove("poof");
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add("poof");
  const cy = display * 0.45;
  for (let i = 0; i < POOF_SPARKS; i++) {
    const a = (i / POOF_SPARKS) * Math.PI * 2 + Math.random() * 0.5;
    const r = display * (0.28 + Math.random() * 0.12);
    spark(i, cy, Math.cos(a) * r, -Math.sin(a) * r);
  }
}

// 성장 축하 (백엔드 anim "level_up"이 시작될 때 한 번): 하트와 별이 몸 가운데에서 펑 터져 퍼지며 사라짐.
// CSS 애니메이션 한 번 (.burst-bit — 프레임마다 JS 없음), 끝나면 지움. 조각은 #stage의 자식이라 몸의 변환·모자와 상관없음.
// 성장 순간 백엔드가 창을 새 크기로 바꾸므로 지금 창을 재지 않고 새 몸 크기로 계산함 (창 = 128 × max(1, 배율) × 36 + 몸)
// BURST-LAYOUT-START (tools/test_levelup_ui.py가 node로 이 부분을 그대로 돌림)
const BURST_COUNT = 12;
// 성장 축하 폭죽 조각들의 자리를 계산함
function burstLayout(side) {
  const halfW = Math.max(128, side) / 2;
  const cy = side / 2;
  const up = 36 + side - cy;
  const reach = Math.max(side * 0.62, 50);
  const bits = [];
  for (let i = 0; i < BURST_COUNT; i++) {
    const a = (i / BURST_COUNT) * Math.PI * 2 + Math.random() * 0.35;
    const r = reach * (0.72 + Math.random() * 0.38);
    const size = 14 + Math.random() * 5;
    // 커지는 순간(CSS 1.2배)까지 창 안에 남도록 끝자리를 창 안으로 당김. 시작점과 끝점이 창 안이면 곧게 가는 길도 창 안임
    const pad = size * 0.75;
    const dx = Math.max(-(halfW - pad), Math.min(halfW - pad, Math.cos(a) * r));
    const dy = Math.max(-(cy - pad), Math.min(up - pad, Math.sin(a) * r));
    bits.push({ heart: i % 2 === 0, size, dx, dy, delay: Math.random() * 80 });
  }
  return { cy, bits };
}
// 축하 중에 눌러 잡혔다 놓여도 (level_up → dragged → level_up) 한 번의 성장엔 한 번만 (코어 LEVEL_UP_SECS 2초보다 길게)
const BURST_ONCE_MS = 2500;
// BURST-LAYOUT-END
const BURST_MS = 1000;
let lastBurst = -Infinity;
// 성장 축하 폭죽을 터뜨림 (잠깐 사이엔 한 번만)
function levelUpBurst() {
  const now = performance.now();
  if (now - lastBurst < BURST_ONCE_MS || document.visibilityState === "hidden") return;
  lastBurst = now;
  const { cy, bits } = burstLayout(128 * stage.scale);
  for (const b of bits) {
    const s = document.createElement("span");
    s.className = "burst-bit";
    s.textContent = b.heart ? "♥" : "★";
    s.style.color = b.heart ? "#ff5a8a" : "#ffd84a";
    s.style.fontSize = b.size.toFixed(1) + "px";
    s.style.bottom = cy + "px";
    s.style.animationDelay = b.delay.toFixed(0) + "ms";
    s.style.setProperty("--dx", `${b.dx.toFixed(1)}px`);
    s.style.setProperty("--dy", `${(-b.dy).toFixed(1)}px`);
    stageEl.appendChild(s);
    s.addEventListener("animationend", () => s.remove());
    // 그사이 창이 숨겨져 애니메이션이 멈춰도 남지 않게
    setTimeout(() => s.remove(), BURST_MS + 500);
  }
}

// 첫 pet-render는 이 웹뷰가 듣기 전에 지나갔을 수 있음 — 첫 로드 땐 slot_info의 render를 씀 (이벤트가 먼저 왔으면 그쪽이 새것)
let gotRender = false;
// 루돌프 코·호박 빛이 나는 동안 앱이 창을 아래로 늘림: 펫과 머리 위 띠를 그만큼 올려 그림
listen("pet-pad", (e) => {
  stageEl.style.bottom = `${Number(e.payload) || 0}px`;
});
const listeningRender = listen("pet-render", (e) => {
  gotRender = true;
  if (e.payload.anim !== render.anim) animStart = performance.now();
  if (e.payload.heart && !render.heart) {
    heartEl.classList.remove("show");
    void heartEl.offsetWidth; // 애니메이션 재시작
    heartEl.classList.add("show");
  }
  // 트레이에서 먹였거나 다른 길로 먹기 시작하면 고르기 버튼은 필요 없음
  if (e.payload.anim === "eat") speech.hidePicker();
  if (e.payload.flash && !render.flash) petFx("play");
  // 🙌 하이파이브 짝! (코어 brain_highfive.rs render.clap)
  if (e.payload.clap && !render.clap) clapBurst(e.payload.flip);
  // 🐛 고치가 풀림 (코어 brain_snare.rs render.cocoon → 0)
  if (render.cocoon && !e.payload.cocoon) cocoonPop();
  // 👜 가방이 사라짐 (코어 brain_study.rs render.bag → 0)
  if (render.bag > 0 && !(e.payload.bag > 0)) bagPoof(render.flip);
  // 콰당! 몸이 발밑을 축으로 납작 찌그러졌다 튕기고 (style.css #pet.thud), 머리 위에 💫이 돎
  if (e.payload.thud) petEl.classList.add("thud");
  else petEl.classList.remove("thud");
  dizzyEl.hidden = !e.payload.thud;
  // 펭귄 안경 반사: 켜져 있는 동안 렌즈가 하얗게 (style.css #pet.glasses-glare)
  if (e.payload.glare) petEl.classList.add("glasses-glare");
  else petEl.classList.remove("glasses-glare");
  // 하우스 안이면 하우스 창이 대신 축하함 (ui/house.js 집 폴짝)
  if (e.payload.anim === "level_up" && render.anim !== "level_up" && !e.payload.housed) levelUpBurst();
  // 뿅은 손 흔들기 끝에서만 시작함 (걸어 나가 사라진 Away도 poof지만 그건 그림 없이)
  if (e.payload.poof && !render.poof && e.payload.anim !== "idle") poof();
  // poof가 아닌 모습이 오면(취소돼 남음·돌아옴) 바로 다시 보임. 사라진 뒤(Away)엔 백엔드가 poof를 켜 둠
  else if (!e.payload.poof) petEl.classList.remove("poof");
  render = e.payload;
  // 조작 모드 중이면 머리 위에 🎮
  ctltagEl.hidden = !render.ctl;
  syncCloud(!!render.cloud);
  syncCocoon(render.cocoon || 0);
  syncSeat(render.seat || 0);
});

// 👜 헤르미온느의 끝없는 구슬 가방 (백엔드 render.bag = 크기 0.15…1, 0 = 없음 — 코어 brain_study.rs): 그림 시트가 아니라 SVG로 그려
// 넣은 수만큼 부드럽게 커짐. 끈은 지금 칸의 손 자리 (pet.json grip.bag_walk·bag_drag·bag_stuff — tools/draw_hermione.py)로 이어짐:
// 작으면 손에 대롱대롱, 바닥에 닿을 만큼 크거나 끌기 (bag_drag)면 바닥에 놓여 몸 뒤로 끌림. 손 자리가 없는 칸 ("Phew!")이면 몸 뒤 바닥에.
// 0으로 돌아가면 뿅 반짝이. 좌표는 칸 128 기준 (#bag viewBox) — #pet과 같이 좌우로 뒤집힘
const BAG_FLOOR = 125;
// 가방 높이 = BAG_H[0] + BAG_H[1] × 크기 (칸 128 기준), 폭은 높이의 BAG_W배
const BAG_H = [6, 52];
const BAG_W = 0.9;
const bagEl = document.getElementById("bag");
const bagStrap = document.getElementById("bag-strap");
const bagBody = document.getElementById("bag-body");
const bagBeads = document.getElementById("bag-beads");
const bagNeck = document.getElementById("bag-neck");
const bagShine = document.getElementById("bag-shine");
// 마지막으로 그린 (크기, 동작, 칸) — 바뀔 때만 DOM을 만짐
let bagDrawn = "";
// 지금 칸의 손 자리 (칸 128 기준) — 없으면 null
function bagHand(anim, f) {
  const cell = manifest?.grip?.[stage.stage]?.[anim]?.[f];
  if (!cell) return null;
  const size = manifest.frame_sizes?.[stage.stage] ?? manifest.frame_size;
  return [(cell[0] / size) * 128, (cell[1] / size) * 128];
}
// 가방을 그림 (크기 b, 동작·칸으로 손 자리)
function drawBag(b, anim, f) {
  const key = b > 0 ? `${b.toFixed(3)}|${anim}|${f}|${stage.stage}` : "";
  if (key === bagDrawn) return;
  bagDrawn = key;
  bagEl.hidden = !(b > 0);
  if (!(b > 0)) return;
  const h = BAG_H[0] + BAG_H[1] * Math.min(1, b);
  const w = h * BAG_W;
  const hand = bagHand(anim, f);
  let cx;
  let top;
  if (!hand) {
    // 손을 놓음: 몸 뒤 바닥에
    cx = Math.max(w / 2, 34 - w * 0.2);
    top = BAG_FLOOR - h;
  } else if (anim === "bag_drag") {
    // 끌기: 바닥에 놓여 손 뒤쪽으로
    cx = Math.max(w / 2 - 6, hand[0] - w * 0.45);
    top = BAG_FLOOR - h;
  } else {
    // 손에 대롱대롱 (바닥에 닿으면 바닥에 놓임)
    cx = hand[0];
    top = Math.min(hand[1] + 4, BAG_FLOOR - h);
  }
  const neckY = top + h * 0.12;
  const bot = top + h;
  bagBody.setAttribute(
    "d",
    `M${cx - w * 0.2},${neckY} C${cx - w * 0.66},${top + h * 0.3} ${cx - w * 0.62},${bot} ${cx},${bot} ` +
      `C${cx + w * 0.62},${bot} ${cx + w * 0.66},${top + h * 0.3} ${cx + w * 0.2},${neckY} Z`,
  );
  bagNeck.setAttribute("cx", cx);
  bagNeck.setAttribute("cy", neckY);
  bagNeck.setAttribute("rx", w * 0.27);
  bagNeck.setAttribute("ry", Math.max(1.2, h * 0.07));
  bagShine.setAttribute("d", `M${cx - w * 0.34},${top + h * 0.42} Q${cx - w * 0.38},${top + h * 0.7} ${cx - w * 0.2},${top + h * 0.86}`);
  // 구슬: 몸통 안에 줄줄이 (클수록 많음 — 동그라미 요소는 모자랄 때만 더 만듦)
  const r = Math.max(0.7, h * 0.035);
  const step = Math.max(r * 3.2, 3);
  let n = 0;
  for (let row = 0; row < 4; row++) {
    const y = top + h * (0.34 + row * 0.16);
    const half = w * (0.36 + row * 0.04);
    for (let x = cx - half + ((row % 2) * step) / 2; x <= cx + half; x += step) {
      let bead = bagBeads.children[n];
      if (!bead) bead = bagBeads.appendChild(document.createElementNS("http://www.w3.org/2000/svg", "circle"));
      bead.setAttribute("cx", x.toFixed(1));
      bead.setAttribute("cy", y.toFixed(1));
      bead.setAttribute("r", r.toFixed(2));
      n++;
    }
  }
  while (bagBeads.children.length > n) bagBeads.lastChild.remove();
  if (hand) {
    bagStrap.setAttribute("x1", cx + w * 0.12);
    bagStrap.setAttribute("y1", neckY);
    bagStrap.setAttribute("x2", hand[0]);
    bagStrap.setAttribute("y2", hand[1]);
  }
  bagStrap.style.display = hand ? "" : "none";
}
// 가방이 사라질 때 뿅 반짝이 (몸 뒤쪽 아래)
function bagPoof(flip) {
  if (document.visibilityState === "hidden") return;
  const behind = display * 0.25 * (flip ? 1 : -1);
  for (let i = 0; i < 6; i++) {
    const a = (i / 6) * Math.PI * 2 + Math.random() * 0.5;
    const r = display * (0.12 + Math.random() * 0.08);
    spark(i, display * 0.15, Math.cos(a) * r, -Math.sin(a) * r, behind);
  }
}

// 🚗 탈것 좌석 (백엔드 render.seat = 그림 아래 몫, 코어 brain_vehicle.rs): 문·짐칸 벽에 가려지는 다리를 잘라 냄 —
// 탈것 창이 펫 창 아래에 있어도 다리가 차 문 뒤로 들어간 것처럼 보임
let seatShown = 0;
function syncSeat(share) {
  if (share === seatShown) return;
  seatShown = share;
  petEl.style.clipPath = share > 0 ? `inset(0 0 ${(share * 100).toFixed(1)}% 0)` : "";
}

// 손님이면 머리 위 띠에 "🏠 <주인>" — 처음 왔을 때 10초, 그 뒤엔 누를 때마다 5초 (말풍선이 뜨는 동안은 가려짐 — style.css)
const NAMETAG_ARRIVE_MS = 10000;
const NAMETAG_CLICK_MS = 5000;
let isGuest = false;
let nametagTimer = null;
// 손님 이름표를 ms 동안 보임
function showNametag(ms) {
  if (!isGuest) return;
  nametagEl.hidden = false;
  clearTimeout(nametagTimer);
  nametagTimer = setTimeout(() => (nametagEl.hidden = true), ms);
}
// 손님이 들고 온 편지 💌 (주인이 실어 보낸 것 — 백엔드가 다듬어 slot_info/pet-stage의 note로 줌).
// 도착하면 봉투가 머리 옆에서 통통 튐. 봉투나 몸을 누르면 평소 말풍선에 "💌 <주인>: <편지>"를 10초
// (이 누름엔 도망·애교 없음) — 한 번 읽으면 봉투는 사라지고, 그 뒤 몸 누르기는 평소 손님 놀이.
// 편지 말풍선이 떠 있는 동안 다른 말풍선(수다·간질·인사·손님 놀이 등)과 음식 고르기는 버림.
// 말풍선은 여러 줄로 커지므로 그동안 창을 위로 키움(백엔드 note_space). 글자는 textContent로만 (HTML 아님)
// 내 동물도 친구가 답장을 써서 돌려보내면 같은 봉투를 들고 돌아옴 (`pet-mail` {from, note} — "💌 <친구>: <답장>")
const NOTE_MS = 10000;
let note = null;
let ownerName = "";
let noteRead = false;
let noteOpen = false;
let noteTimer = null;
// 손님 편지를 펼침 (한 번만)
function openNote() {
  if (!note || noteRead) return;
  noteRead = true;
  envelopeEl.hidden = true;
  noteOpen = true;
  speech.hideBubble();
  speech.hidePicker();
  const text = `💌 ${ownerName}: ${note}`;
  noteTimer = setTimeout(closeNote, NOTE_MS);
  // 창을 위로 키운 뒤(백엔드) 말풍선을 보임
  invoke("note_space", { open: true }).finally(() => {
    if (!noteOpen) return;
    const span = document.createElement("span");
    span.textContent = text;
    bubbleEl.replaceChildren(span);
    bubbleEl.classList.add("note");
    bubbleEl.hidden = false;
  });
}
// 펼친 편지를 접음
function closeNote() {
  clearTimeout(noteTimer);
  if (!noteOpen) return;
  noteOpen = false;
  bubbleEl.hidden = true;
  bubbleEl.classList.remove("note");
  invoke("note_space", { open: false });
}
// 봉투 누르기는 펫 잡기·손님 놀이로 이어지지 않음
envelopeEl.addEventListener("mousedown", (e) => {
  e.stopPropagation();
  e.preventDefault();
  if (e.button === 0) openNote();
});

// 내 동물·손님 역할에 맞게 이름표·편지를 맞춤
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
    clearTimeout(nametagTimer);
    nametagEl.hidden = true;
    // 손님이던 창만 손님 편지를 지움 (내 동물의 성장·교체(pet-stage)는 받은 답장을 지우지 않음)
    if (isGuest) {
      note = null;
      noteRead = false;
      envelopeEl.hidden = true;
      closeNote();
    }
    isGuest = false;
  }
}

// 친구 화면에서 답장을 들고 돌아온 내 동물: 머리 옆에 💌 — 봉투나 몸을 누르면 "💌 <친구>: <답장>"을 10초 (한 번 읽으면 봉투는 사라짐)
listen("pet-mail", (e) => {
  const mail = e.payload ?? {};
  if (isGuest || typeof mail.note !== "string" || !mail.note) return;
  closeNote();
  note = mail.note;
  ownerName = typeof mail.from === "string" ? mail.from : "";
  noteRead = false;
  envelopeEl.hidden = false;
});

// 동물 교체·성장·리셋: 창 크기는 백엔드가 바꾸고, 여기선 시트와 스프라이트 크기를 맞춤
const listeningStage = listen("pet-stage", (e) => {
  stage = e.payload;
  applyRole(e.payload);
  loadPet();
});

// pet.json에 없는 상황의 기본 대사 (직접 주기: 배고플 때 손에 든 음식을 봤음)
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
  // 손님이 이 화면 주인의 답장을 들고 집으로
  reply: ["bye~! 💌"],
  // 공룡이 쿵쾅해서 흔들림
  whoa: ["whoa!", "eek!"],
  // 위/아래 친구 화면으로 높이 뛰어 나감·아래에서 튀어 올라옴 / 바닥 밑으로 쏙
  boing: ["boing!"],
  whee: ["whee!"],
  // 눈사람을 다 만들었음
  snowman: ["snowman!", "⛄!"],
  // 커서 끝을 붙잡음 / 서 있던 창이 사라짐 / 내려서서 째려봄 (cursor_play는 아래 CURSOR_LINES)
  gotcha: ["gotcha!", "got you!"],
  eek: ["eek!"],
  hmph: ["hmph!"],
  // 공룡 동작 (pet.json에 없으면 이 말)
  roar: ["RAWR!"],
  fire: ["fwoosh~"],
  stomp: ["stomp!"],
  // 앞에 똥이 있음 / 똥을 밟고 꽈당·친구에게 머리를 밟힘
  ew: ["ew!", "yuck!"],
  oof: ["oof!", "ouch!"],
  // 던져져 날다가 친구에게 부딪혀 되튐 / 날아온 것에 살짝 맞음
  ting: ["ting!", "boink!"],
  ow: ["ow!", "ouch!"],
  // 🤹 흐물이가 물건을 번쩍 / 거대한 것에 끙차 / 휙 던짐 / 친구 화면에 배달 도착 / 너무 무거워 털썩 (코어 brain_juggle.rs)
  hup: ["hup!", "heave-ho!"],
  strain: ["hnngh!", "heave~!"],
  yeet: ["yeet!", "catch!"],
  tada: ["ta-da!", "special delivery!"],
  phew: ["phew...", "too heavy..."],
  // 📌 흐물이가 작업표시줄에서 장난감이 쑥 빠졌음 / 도로 밀어 넣음 (코어 brain_taskbar.rs)
  pop: ["pop!", "got one!"],
  stow: ["back you go!", "in you go!"],
  // 🔄 업데이트 확인이 새 버전을 찾았음 — 10초쯤 신나게 뜀 (코어 brain_cheer.rs)
  new_version: ["new version!!"],
  // 흐물이 더 놀기 (v0.6.3): 🎣 커서에 매달려 흔들흔들 / 세게 흔들면 / 🙆 들려서 버둥 / 🫣 들켰음 / 못 찾아서 나옴 / 🕺 춤 / ⚾ 캐치볼 20번 (⌨️ 타자 응원 대사는 코어가 글자 그대로 — brain_typing.rs TYPING_LINES)
  swing: ["Wheee~", "Wheeee!"],
  more: ["More!", "Again!"],
  put_me_down: ["Put me down!", "Hey!"],
  found: ["Found me!"],
  peekaboo: ["Peekaboo~"],
  dance: ["♪", "♪♪"],
  rally_end: ["Done!"],
  // 다른 동물들도 더 놀기 (v0.6.3): 🐱 고양이 툭툭 / 밀어 떨어뜨림 · 🦖 큰 쿵 · 🐿️ 볼에 쏙 · 🙌 하이파이브
  tap: ["Tap… tap…"],
  oops: ["Oops~"],
  big_stomp: ["STOMP!"],
  nom: ["Nom nom~"],
  high_five: ["High five!"],
  // ⛄ 눈덩이에 맞았음 (v0.6.5, 코어 brain_hit.rs)
  brr: ["brr!", "hey!"],
  // ⛄ 발밑 눈을 뭉쳐 던짐 / 눈밭에서 데굴 (코어 brain_snowplay.rs)
  snowball: ["snowball!", "catch!"],
  flop: ["fluffy!", "brrr~"],
  // ⚾🖱️ 먼저 커서에게 놀자고 던지거나 밂 (코어 brain_cursor_rally.rs — 받은 횟수 "3!"·"boop! 2"는 글자 그대로)
  invite: ["catch!", "play?"],
  // 🕷️ 거미인간 줄 쏘기 / 모서리 거미집 / 줄 그네 / 영웅 착지 (코어 brain_web.rs — pet.json speech가 있으면 그 말)
  thwip: ["thwip!"],
  web: ["web time!"],
  web_swing: ["wheee!", "woohoo!"],
  hero_land: ["nailed it!", "hero landing!", "super landing!", "stuck it!", "like a pro", "boom!", "still got it", "ground secured", "ta-da!", "whoosh… done"],
  // 🕷️🕷️🕷️ 멀티버스 모임: 다른 거미인간을 보고 깜짝 / 흩어지며 (코어 social_multiverse.rs)
  multiverse: ["multiverse?!", "wait… you too?", "no way!"],
  see_ya: ["see ya!"],
  // 🙃 거미인간이 거꾸로 매달린 채 가까운 펫·커서 쪽을 보며 인사 (코어 brain_web.rs Speech::HangHey)
  hang_hey: ["hey there", "sup?"],
  // 🦇 박쥐인간: 오토바이에 올라 / 윌리 / 점프·드리프트 / 🔦 박쥐 신호 / 💪 거대한 것도 번쩍 (코어 brain_bike.rs)
  vroom: ["vroom!", "to the bike!"],
  // 🚗 탈것: 올라 부릉 / 끼익 급정거 / 🛻 짐칸에 타라고 부름 (코어 vehicle.rs·brain_vehicle.rs)
  drive: ["vroom!", "vroom vroom!"],
  honk: ["beep beep!", "beep!"],
  hop_in: ["hop in!"],
  wheelie: ["yeah!", "wheelie!"],
  bike_trick: ["skrrt!", "hup!"],
  bat_signal: ["I'm the night", "signal's up!"],
  hero_lift: ["easy!", "light as air"],
  // 🦸 슈퍼인간: 날아오르기 / 친구 화면으로 날아감 / 🔄 공중제비 / 사뿐 내려앉기 (코어 brain_soar.rs)
  take_off: ["up, up and away!", "whoosh!"],
  rescue: ["to the rescue!", "on my way!"],
  air_flip: ["wheee!", "flip!"],
  soft_land: ["touchdown!", "soft landing"],
  // 🧙 마법사 (코어 brain_broom.rs·brain_spell.rs·brain_study.rs): 🧹 빗자루로 날아오름 / 내려앉음 / 친구 화면으로 · ✨ 주문 셋과 실패 ·
  // 🟡 스니치를 봄 / 잡음 · 📖 책 읽다 중얼 · 🙋 손들기 · 👜 가방 청소 시작 / 다 치움
  broom_up: ["up we go!", "whoosh!"],
  broom_land: ["smooth landing", "and down!"],
  broom_away: ["off I go!", "see you!"],
  expelliarmus: ["Expelliarmus!"],
  leviosa: ["Wingardium Leviosa!"],
  fizzle: ["Leviosaaa?", "oops..."],
  reparo: ["Reparo!"],
  snitch: ["the Snitch!"],
  got_it: ["Got it!"],
  study: ["fascinating...", "hmm..."],
  i_know: ["I know!"],
  tidy_up: ["tidy time!"],
  all_clean: ["Phew! All clean!"],
  // 펫끼리 놀기 더 (코어 social_romp.rs): 🥊 투닥투닥 / 😈 몰래 찌르고 키득 · 찔려서 화들짝 / 🫸 되밀기 / 🏁 달리기 시합 / 💬 수다 (먼저 · 받는 쪽)
  scuffle: ["hey!", "mine!", "grr", "grr!"],
  hehe: ["hehe", "hehe~"],
  hey: ["hey!"],
  hey_back: ["you hey!", "no, YOU!"],
  ready: ["ready…"],
  go: ["go!"],
  i_win: ["I win!"],
  no_fair: ["no fair!"],
  chat_a: ["hi!", "nice day", "😺", "lol", "guess what?", "you ok?"],
  chat_b: ["really?", "same!", "lol", "hehe", "😺", "yeah!"],
  // 🕷️ 거미인간에게 걸림 (코어 brain_snare.rs): 🐛 고치에 감겨 버둥 / 🕸️ 거미집에 걸려 버둥 / 눌러서 풀어 줌 / 혼자 뿅 빠져나옴
  mmph: ["mmph!", "mmf!"],
  help: ["help!", "stuck!"],
  thanks: ["thanks!", "phew!"],
  broke_free: ["free!", "phew!"],
  // 🌱 연령대별 특기 (코어 brain_stage.rs·social_romp.rs): 👶 엉덩방아 / 혼자 오래 / 🧒 올려다보기 / 🧑‍🎤 공중제비 / 시무룩 / 🧓 쓰다듬기 / 👶 졸졸
  plop: ["oops!", "uh-oh"],
  waah: ["waah", "waah~"],
  ooh: ["ooh?", "ooh!"],
  flip: ["yeah!", "nailed it!"],
  sulk: ["ugh", "whatever"],
  good_kid: ["good kid", "there there"],
  tag_along: ["wait for me!", "me too!"],
};
// 🌱 연령대 말투 (모든 동물 공통): 지금 연령대의 줄이 종 말풍선에 더해짐. pet.json `stage_speech: { "baby": { "chatter": [...] } }`가 있으면 그 키는 그 줄로 바꿈
const STAGE_SPEECH = {
  baby: { chatter: ["ba!", "mama?", "goo~"] },
  kinder: { chatter: ["why?", "look!", "ooh!"] },
  child: { chatter: ["hehe", "race you!", "let's play!"] },
  teenager: { chatter: ["ugh", "whatever", "cool."] },
  adult: { chatter: ["hm hm", "nice day", "kids these days"] },
};
// 지금 연령대의 말풍선 표: 종 말 + 공통 연령대 말 (같은 키는 이어 붙임), pet.json stage_speech는 키마다 덮어씀
function stageSpeech(base, m, stageName) {
  const out = { ...base };
  for (const [key, lines] of Object.entries(STAGE_SPEECH[stageName] ?? {})) out[key] = [...(out[key] ?? []), ...lines];
  return { ...out, ...(m.stage_speech?.[stageName] ?? {}) };
}
// 🐾 커서 장난: 커서를 보고 장난기가 돌 때 — 동물마다 다르게 (코어 brain_cursor.rs)
const CURSOR_LINES = {
  cat: ["mrrow...", "*wiggle wiggle*"],
  rabbit: ["hop hop!"],
  squirrel: ["chk chk?", "ooh, shiny?"],
  pumpkin: ["bounce!"],
  rudolph: ["jingle!"],
  santa: ["ho ho ho!"],
  snowman: ["brrr~!", "snow!"],
  dino_yellow: ["RAWR!"],
  dino_green: ["rawr!"],
  penguin: ["wheee~!", "slide slide!"],
  heumul: ["zoom zoom!", "hup hup!"],
  spider: ["thwip thwip!", "web time!"],
  bat: ["to the bike!", "I'm the night"],
  super: ["up, up and away!", "to the rescue!"],
  harry: ["Quidditch time!", "mischief managed"],
  hermione: ["Levi-O-sa!", "honestly..."],
};
const speech = setupSpeech({
  bubbleEl,
  pickerEl,
  favBtn,
  getManifest: () => manifest && { ...manifest, speech: stageSpeech({ cursor_play: CURSOR_LINES[stage.pet] ?? ["!"], ...EXTRA_SPEECH, ...manifest.speech }, manifest, stage.stage) },
  isQuiet: () => noteOpen,
});

// 메뉴 "친구 코드 보기": 머리 위 띠에 "🔑 ABCD-1234", 누르면 복사 (잠시 뒤 사라짐)
const codetagEl = document.getElementById("codetag");
let codeTimer = null;
// 친구 코드 띠를 ms 뒤 숨김
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
// 띠를 누르는 게 펫 잡기로 이어지지 않게 함
codetagEl.addEventListener("mousedown", (e) => e.stopPropagation());
codetagEl.addEventListener("click", async (e) => {
  e.stopPropagation();
  const ok = await copyText(codetagEl.dataset.code ?? "");
  codetagEl.textContent = ok ? "복사했어요!" : `🔑 ${codetagEl.dataset.code}`;
  hideCodeLater(ok ? 1500 : 20000);
});

petEl.addEventListener("mousedown", (e) => {
  // 안 읽은 편지가 있으면 첫 누름은 편지만 엶 (잡기·도망 없음)
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

// 그리기 고리는 먼저 돌림 — 첫 그림을 못 읽어도 다음 pet-stage로 다시 그릴 수 있게 (그림이 없는 동안 draw는 아무것도 안 함)
requestAnimationFrame(draw);

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 물음 — 그 사이 바뀐 것은 이벤트로 옴
  await Promise.all([listeningRender, listeningStage]);
  stage = await invoke("slot_info");
  applyRole(stage);
  if (!gotRender && stage.render) {
    render = stage.render;
    syncCloud(!!render.cloud);
    syncCocoon(render.cocoon || 0);
    animStart = performance.now();
  }
  await loadPet();
})();
