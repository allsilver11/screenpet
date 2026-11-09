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
const ctltagEl = document.getElementById("ctltag");

let manifest = null;
let display = 128;
let render = { anim: "fall", flip: false, heart: false };
let animStart = performance.now();
// 지금 동물과 몸 크기 배율 (단계 배율 × 동물별 배율) — 백엔드 `pet-stage`
let stage = { pet: null, stage: "teenager", scale: 1, style: "round" };
let loadSeq = 0;
// 픽셀 그림을 그리는 중 — dot = 칸 한 변 (CSS px, 물리 px의 정수배). 칸이 흐려지는 변환(회전·확대·분수 이동)은 쓰지 않는다
let pixelMode = false;
let dot = 1;
// dot을 잰 화면 배율 — 웹뷰 배율이 pet-stage보다 늦게 바뀌면 resize에서 다시 잰다
let measuredDpr = 1;

// 단계별 시트 sheet_<stage>.png
async function loadPet() {
  const seq = ++loadSeq;
  const { manifest: m, url, img, style, dir } = await loadPetSheet(stage.pet, stage.stage, stage.style);
  if (seq !== loadSeq) return; // 그 사이 다른 동물/단계로 바뀜
  sheetImg = img;
  headCache.clear();
  headgearAt = null;
  pixelMode = style === "pixel";
  setPixelBody(pixelMode);
  measuredDpr = window.devicePixelRatio || 1;
  const side = pixelMode ? snapPixelSide(128 * stage.scale, m.frame_size) : 128 * stage.scale;
  dot = side / m.frame_size;
  if (pixelMode) document.body.style.setProperty("--dot", dot + "px");
  petEl.style.width = side + "px";
  // 창 가운데 (창 너비는 128 × max(1, 배율), 편지 쪽지를 펼친 동안은 더 넓다 — 백엔드가 몸을 창 가운데에 둔다)
  petEl.style.left = centerLeft(side);
  stageEl.style.setProperty("--pet-h", side + "px");
  display = side;
  manifest = m;
  const k = display / manifest.frame_size;
  petEl.style.backgroundImage = `url(${url})`;
  petEl.style.backgroundSize = `${img.width * k}px ${img.height * k}px`;
  setupNeck(m, side, k, dir);
  // 동물이 바뀌면 모자 ↔ 목도리 (pet-presence는 그대로라 다시 오지 않는다)
  if (lastPresence) {
    setCap(!!lastPresence.nightcap);
    applyRing();
  }
  animStart = performance.now();
}

// 몸의 left: 동글은 창 가운데 그대로, 픽셀은 물리 px에 맞춰서 (반 칸 어긋나면 칸이 고르지 않다 — 창 너비가 바뀌면 다시 잰다)
function centerLeft(side) {
  if (!pixelMode) return `calc(50% - ${side / 2}px)`;
  const dpr = window.devicePixelRatio || 1;
  return `${Math.round(((window.innerWidth - side) / 2) * dpr) / dpr}px`;
}
window.addEventListener("resize", () => {
  if (!pixelMode) return;
  if ((window.devicePixelRatio || 1) !== measuredDpr) {
    loadPet();
    return;
  }
  petEl.style.left = centerLeft(display);
  neckEl.style.left = petEl.style.left;
});

// 기린 목 늘이기 (백엔드 anim "neckstretch" — 코어 behavior::NECK_STRETCH_SECS 3.4초 동안 제자리):
// 목이 평소의 두 배로 쭉 늘었다가 돌아온다. 시트 한 칸으로는 담을 수 없어 층으로 그린다 —
// 픽셀 그림은 늘 PNG다 (WebP로 바꾸는 건 동글 그림뿐, tools/pack_webp.py)
function neckExt(dir) {
  return dir.startsWith("pets_pixel") ? "png" : PET_ART_EXT;
}

// pets/giraffe/neck_<stage>.png (가로 4칸: 몸 · 머리 "어?" · 머리 깜빡 · 머리 ✧ 흐뭇, 모두 시트와 같은 좌표)와
// neck_tile_<stage>.png (곧은 목 한 토막, 무늬 주기 = 높이). pet.json neck_stretch[stage] = {cut, tile, ext}:
// cut 줄 아래는 몸 층, 위는 머리 층. 머리 층을 ext만큼 올리고 그 사이를 목 토막으로 채운다 (토막은 머리에 붙어 함께 올라간다).
// 그동안 창은 백엔드(slots STRETCH_SPACE)가 위로 키우고, 몸(#pet)은 투명하게 두어 잡기·누르기는 그대로 받는다
const NECK_UP_MS = 700;
const NECK_HOLD_MS = 2000;
const NECK_DOWN_MS = 700;
const neckEl = document.createElement("div");
neckEl.id = "neck";
neckEl.hidden = true;
neckEl.innerHTML = '<div class="nk-body"></div><div class="nk-tile"></div><div class="nk-head"></div>';
petEl.after(neckEl);
const [nkBody, nkTile, nkHead] = neckEl.children;
let neck = null;

function setupNeck(m, side, k, dir) {
  const meta = m.neck_stretch?.[stage.stage];
  neck = null;
  neckEl.hidden = true;
  petEl.style.opacity = "";
  if (!meta) return;
  neck = { meta, k, side };
  neckEl.style.width = side + "px";
  neckEl.style.height = side + "px";
  neckEl.style.left = petEl.style.left;
  for (const el of [nkBody, nkHead]) {
    el.style.backgroundImage = `url(${dir}neck_${stage.stage}.${neckExt(dir)})`;
    el.style.backgroundSize = `${4 * side}px ${side}px`;
  }
  nkTile.style.backgroundImage = `url(${dir}neck_tile_${stage.stage}.${neckExt(dir)})`;
  nkTile.style.backgroundSize = `${side}px ${meta.tile * k}px`;
}

const easeOut = (t) => 1 - (1 - t) ** 3;
const easeInOut = (t) => (t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2);

// 늘이는 정도 0..1과 머리 그림 (0 어? · 1 깜빡 · 2 ✧) — 다 늘인 동안 어? → 깜빡 → ✧ 흐뭇
function neckPhase(ms) {
  if (ms < NECK_UP_MS) return { p: easeOut(ms / NECK_UP_MS), head: 0 };
  const h = ms - NECK_UP_MS;
  if (h < NECK_HOLD_MS) return { p: 1, head: h < 600 ? 0 : h < 850 ? 1 : 2 };
  const d = Math.min(1, (h - NECK_HOLD_MS) / NECK_DOWN_MS);
  return { p: 1 - easeInOut(d), head: d < 0.5 ? 2 : 0 };
}

// 그리면 true (그동안 #pet은 투명)
function drawNeck(elapsed) {
  if (!neck || render.anim !== "neckstretch") {
    if (!neckEl.hidden) {
      neckEl.hidden = true;
      petEl.style.opacity = "";
    }
    return false;
  }
  const { meta, k, side } = neck;
  const { p, head } = neckPhase(elapsed);
  // 픽셀: 한 칸씩 늘어난다
  const ext = pixelMode ? Math.round(meta.ext * p) * k : meta.ext * k * p;
  nkTile.style.top = `${meta.cut * k - ext}px`;
  nkTile.style.height = `${ext}px`;
  nkHead.style.backgroundPosition = `${-(head + 1) * side}px 0`;
  nkHead.style.transform = `translateY(${-ext}px)`;
  neckEl.style.transform = render.flip ? "scaleX(-1)" : "none";
  if (neckEl.hidden) {
    neckEl.hidden = false;
    petEl.style.opacity = "0";
  }
  return true;
}

// 공룡 쿵쾅 (v0.3.6): 쿵! 하는 그림(stomp 1·3장)마다 몸이 덜덜, 쿵쾅에 흔들린 친구(백엔드 render.wobble, 0.4초)도 덜덜 —
// 스프라이트만 살짝 옮긴다 (창은 그대로)
function jitter(f, now) {
  const stomping = render.anim === "stomp" && f % 2 === 1;
  if (!stomping && !render.wobble) return null;
  // 픽셀: 한 칸씩 좌우로 (분수 이동은 칸을 흐린다)
  if (pixelMode) return `translateX(${Math.round(Math.sin(now / 17)) * dot}px)`;
  const amp = (stomping ? 1.5 : 2.5) * Math.sqrt(stage.scale);
  return `translate(${(Math.sin(now / 17) * amp).toFixed(2)}px, ${(Math.cos(now / 23) * amp * 0.5).toFixed(2)}px)`;
}

// 수면 모자·집중 고리 (백엔드 pet-presence {nightcap, ring}): 둘 다 #pet의 자식이라 몸의 변환(반전·구르기·매달림·덜덜·쏘기·뿅)과
// 창 크기 변화를 따로 계산하지 않아도 한 덩어리로 움직인다. 여기서는 칸 안의 자리만 정한다 — pet.json head[단계][동작][칸] = [x, y, w, r]:
// 그림을 그린 코드(tools/draw_pets.py mark_head)가 적어 둔 모자 입구 가운데 · 입구 너비 (칸 px) · 머리 기울기 (도). 칸이 바뀔 때만 옮긴다
const nightcapEl = document.getElementById("nightcap");
const focusringEl = document.getElementById("focusring");
const focusringFill = document.getElementById("focusring-fill");
// 밤 목도리: pet.json에 neck 표가 있는 동물(산타 — 모자를 이미 썼다)은 수면 모자 대신 두른다. 자리·몸과 한 덩어리인 것은 모자와 같다
const scarfEl = document.getElementById("scarf");
// 픽셀 스타일: 같은 자리에 칸 그림 (effects_pixel/nightcap.png·scarf.png 띠, 고리는 캔버스)
const capPxEl = document.getElementById("nightcap-px");
const scarfPxEl = document.getElementById("scarf-px");
const ringPxEl = document.getElementById("focusring-px");
let headgearAt = null;

// HEADGEAR-LAYOUT-START (tools/test_nightcap.py가 node로 이 부분을 그대로 돌린다)
// 모자 SVG(ui/index.html)의 입구 가운데와 너비 (viewBox 좌표)
const CAP_BRIM_X = 16.5;
const CAP_BRIM_Y = 20.5;
const CAP_BRIM_W = 27;
const CAP_TILT = -8; // 끝이 머리 뒤로 처지게 (칸은 오른쪽을 보고, 그림의 방울은 오른쪽이라 뒤집어 쓴다)
const RING_R = 10; // 고리 SVG 반지름 (px)
const HEAD_FALLBACK = [64, 24, 44, 0]; // head 표가 없는 동물: 칸 위쪽 가운데

// k = 표시 px / 칸 px. 고리는 머리 위 (모자를 쓰면 모자 위)에 뜬다. 반전된 몸 안에서도 진행 방향은 그대로 (scaleX로 되돌림)
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
// 앞모습 동작 (코어 Anim::is_front) — 머리가 가운데다. 옆모습은 시트에서 오른쪽을 본다
const FRONT_ANIMS = new Set(["sit_front", "wave", "tilt", "belly", "stretch", "heart", "tickle", "hungry", "eat", "level_up", "huff", "surprise"]);
// 목도리 SVG(ui/index.html #scarf)의 기준점과 너비 (viewBox 좌표)
const SCARF_ANCHOR_X = 26;
const SCARF_ANCHOR_Y = 8;
const SCARF_W = 52;
function scarfLayout(entry, k) {
  const [x, y, w, r] = entry;
  const s = (w * k) / SCARF_W;
  return `translate(${(x * k).toFixed(2)}px, ${(y * k).toFixed(2)}px) rotate(${r.toFixed(1)}deg) scale(${s.toFixed(3)}) translate(${-SCARF_ANCHOR_X}px, ${-SCARF_ANCHOR_Y}px)`;
}
// 지금 칸에 보일 모양 (썰매는 초록 목도리의 펄럭임과 칸마다 같은 모양으로 덮는다)
function scarfView(anim, f) {
  if (anim === "roll") return `sled${f % 2}`;
  if (anim === "climb" || anim === "slide") return "back";
  return FRONT_ANIMS.has(anim) ? "front" : "side";
}
// 매달린 몸이 흔들리는 축 = 커서 끝이 닿는 머리 꼭대기 (코어 PetManifest::hang_grip과 같은 계산: dragged 칸들의 평균, 모자 입구보다 조금 위)
const HEAD_TOP_ABOVE_BRIM = 0.15;
function hangOrigin(cells, frameSize) {
  if (!cells?.length) return "50% 6%";
  const mean = (i) => cells.reduce((a, c) => a + c[i], 0) / cells.length;
  const y = Math.max(0, mean(1) - HEAD_TOP_ABOVE_BRIM * mean(2));
  return `${((mean(0) / frameSize) * 100).toFixed(2)}% ${((y / frameSize) * 100).toFixed(2)}%`;
}
// HEADGEAR-LAYOUT-END

// PIXEL-HEADGEAR-START (tools/test_draw_pixel_world.py가 node로 이 부분을 그대로 돌린다)
// 띠 한 칸의 크기·기준점 (칸, tools/draw_pixel_world.py CAP_*·SCARF_*와 같다)
const CAP_CELL = [40, 30];
const CAP_ANCHOR = [20, 24];
const CAP_WIDTHS = [4, 26];
const SCARF_CELL = [48, 24];
const SCARF_ANCHOR = [24, 6];
const SCARF_WIDTHS = [6, 30];
const RING_DOTS = 11;
// entry = 머리 표 [x, y, w, r] (칸). 자리는 칸 단위 정수, 기울기는 90도 단위로만 (칸이 무너지지 않게), 그림은 너비에 맞는 칸을 고른다
function pixelWearLayout(entry, capOn, flip, d) {
  const [x, y, w, r] = entry;
  const quarter = ((Math.round(r / 90) * 90) % 360) + 0;
  const px = Math.round(x);
  const py = Math.round(y);
  const pick = ([lo, hi]) => Math.min(hi, Math.max(lo, Math.round(w))) - lo;
  const rad = (r * Math.PI) / 180;
  const lift = (capOn ? 0.6 : 0.3) * w + RING_DOTS / 2 + 1;
  const half = (RING_DOTS - 1) / 2;
  return {
    capCell: pick(CAP_WIDTHS),
    cap: `translate(${(px - CAP_ANCHOR[0]) * d}px, ${(py - CAP_ANCHOR[1]) * d}px) rotate(${quarter}deg)`,
    scarfCell: pick(SCARF_WIDTHS),
    scarf: `translate(${(px - SCARF_ANCHOR[0]) * d}px, ${(py - SCARF_ANCHOR[1]) * d}px) rotate(${quarter}deg)`,
    ring: `translate(${(Math.round(x + Math.sin(rad) * lift) - half) * d}px, ${(Math.round(y - Math.cos(rad) * lift) - half) * d}px)${flip ? " scaleX(-1)" : ""}`,
  };
}
// 고리 칸들: 위에서 시계 방향 순서
function ringCells() {
  const c = (RING_DOTS - 1) / 2;
  const cells = [];
  for (let y = 0; y < RING_DOTS; y++) {
    for (let x = 0; x < RING_DOTS; x++) {
      const d = Math.hypot(x - c, y - c);
      if (d > 4 && d < 5.4) cells.push({ x, y, a: (Math.atan2(x - c, c - y) + Math.PI * 2) % (Math.PI * 2) });
    }
  }
  return cells.sort((p, q) => p.a - q.a);
}
// PIXEL-HEADGEAR-END

// 아래는 눈사람 눈구름(placeCloud)이 쓰는 머리 찾기 — 시트의 지금 칸을 재서 맨 위를 찾는다 (칸마다 한 번 재서 기억)
const headCache = new Map();
let sheetImg = null;

// 불투명한 덩어리 중 가장 큰 것 (zzz·반짝이처럼 떨어진 조각은 머리가 아니다) → 칸 안 칸번호 목록
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

// 칸 (row, f)의 머리 꼭대기: 칸 왼쪽 위 기준 {x, y} (시트 px). 몸통 덩어리에서, 옆모습이면 앞쪽(오른쪽) 45%만 본다
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

// host: 지금 머리가 있는 층 — 평소 #pet, 기린 목 늘이기 동안은 머리 층 (#pet이 투명해지고 머리가 위로 올라간다)
function placeHeadgear(anim, f, host) {
  if (nightcapEl.parentNode !== host) host.append(nightcapEl, focusringEl, scarfEl, capPxEl, scarfPxEl, ringPxEl);
  const wearing = !!lastPresence.nightcap;
  const scarf = manifest.neck?.[stage.stage]?.[anim]?.[f];
  const capOn = wearing && !manifest.neck;
  const key = `${stage.stage}:${anim}:${f}:${render.flip}:${display}:${capOn}:${wearing}`;
  if (key === headgearAt) return;
  headgearAt = key;
  // 표가 없는 칸: 칸 위쪽 가운데 (HEAD_FALLBACK은 128칸 기준이라 이 단계 시트의 칸 크기로 옮긴다)
  const entry = manifest.head?.[stage.stage]?.[anim]?.[f] ?? HEAD_FALLBACK.map((v, i) => (i < 3 ? (v * manifest.frame_size) / 128 : v));
  if (pixelMode) {
    placeHeadgearPixel(entry, capOn, scarf, wearing, anim, f);
    return;
  }
  const { cap, ring } = headgearLayout(entry, display / manifest.frame_size, capOn, render.flip);
  nightcapEl.style.transform = cap;
  focusringEl.style.transform = ring;
  if (wearing && scarf) {
    scarfEl.style.transform = scarfLayout(scarf, display / manifest.frame_size);
    scarfEl.dataset.view = scarfView(anim, f);
  }
}

// 픽셀 모자·목도리·고리: 칸 크기(dot)로 띠에서 한 칸 골라 칸 단위 자리에 놓는다
function placeHeadgearPixel(entry, capOn, scarf, wearing, anim, f) {
  const l = pixelWearLayout(entry, capOn, render.flip, dot);
  capPxEl.style.width = CAP_CELL[0] * dot + "px";
  capPxEl.style.height = CAP_CELL[1] * dot + "px";
  capPxEl.style.backgroundSize = `${(CAP_WIDTHS[1] - CAP_WIDTHS[0] + 1) * CAP_CELL[0] * dot}px ${CAP_CELL[1] * dot}px`;
  capPxEl.style.backgroundPosition = `${-l.capCell * CAP_CELL[0] * dot}px 0`;
  capPxEl.style.transformOrigin = `${CAP_ANCHOR[0] * dot}px ${CAP_ANCHOR[1] * dot}px`;
  capPxEl.style.transform = l.cap;
  ringPxEl.style.width = ringPxEl.style.height = RING_DOTS * dot + "px";
  ringPxEl.style.transform = l.ring;
  if (wearing && scarf) {
    const s = pixelWearLayout(scarf, false, false, dot);
    const view = scarfView(anim, f);
    const row = view === "front" ? 0 : view === "back" ? 2 : 1;
    scarfPxEl.style.width = SCARF_CELL[0] * dot + "px";
    scarfPxEl.style.height = SCARF_CELL[1] * dot + "px";
    scarfPxEl.style.backgroundSize = `${(SCARF_WIDTHS[1] - SCARF_WIDTHS[0] + 1) * SCARF_CELL[0] * dot}px ${3 * SCARF_CELL[1] * dot}px`;
    scarfPxEl.style.backgroundPosition = `${-s.scarfCell * SCARF_CELL[0] * dot}px ${-row * SCARF_CELL[1] * dot}px`;
    scarfPxEl.style.transformOrigin = `${SCARF_ANCHOR[0] * dot}px ${SCARF_ANCHOR[1] * dot}px`;
    scarfPxEl.style.transform = s.scarf;
  }
}

// 픽셀 고리 그리기: 바탕 + 회색 칸 + 진행한 만큼 주황 칸 (p: 0..100)
let ringCellList = null;
function drawRingPixel(p) {
  ringCellList ??= ringCells();
  const g = ringPxEl.getContext("2d");
  g.clearRect(0, 0, RING_DOTS, RING_DOTS);
  g.fillStyle = "#fffaf2";
  g.fillRect(3, 3, RING_DOTS - 6, RING_DOTS - 6);
  const done = Math.round((p / 100) * ringCellList.length);
  ringCellList.forEach((c, i) => {
    g.fillStyle = i < done ? "#ef8a5b" : "#d8c6ae";
    g.fillRect(c.x, c.y, 1, 1);
  });
}

// 눈사람의 작은 눈구름 (백엔드 render.cloud): 떠 있는 동안만 프레임이 바뀔 때 머리 위로 옮긴다 — 눈송이는 CSS.
// 구름은 창 안 (말풍선 자리까지)에 머문다. 창 맨 위를 넘지 않게 자른다
const snowcloudEl = document.getElementById("snowcloud");
const CLOUD_GAP = 14;
let cloudAt = null;
function placeCloud(row, f) {
  const key = `${row}:${f}:${render.flip}:${display}`;
  if (key === cloudAt) return;
  const fresh = cloudAt === null;
  cloudAt = key;
  const k = display / manifest.frame_size;
  // 뒷모습은 머리가 가운데 — 앞모습처럼 덩어리 전체에서 찾는다
  const head = headTop(row, f, FRONT_ANIMS.has(render.anim) || render.anim === "climb" || render.anim === "slide");
  if (pixelMode) {
    const hx = render.flip ? manifest.frame_size - head.x : head.x;
    const px = Math.round(((stageEl.clientWidth - display) / 2 + hx * k) / dot - 7) * dot;
    const py = Math.round(Math.min(display - head.y * k + 2 * dot, stageEl.clientHeight - 9 * dot) / dot) * dot;
    snowcloudEl.style.transform = `translate(${px}px, ${-py}px)`;
    return;
  }
  const size = Math.sqrt(stage.scale);
  const x = (render.flip ? manifest.frame_size - head.x : head.x) * k + (stageEl.clientWidth - display) / 2;
  const y = Math.min(display - head.y * k + CLOUD_GAP * size, stageEl.clientHeight - 37 * size);
  // 처음 뜰 땐 미끄러져 오지 않게 바로 그 자리에 (이후 프레임만 둥실 따라온다)
  if (fresh) snowcloudEl.style.transition = "none";
  snowcloudEl.style.transform = `translate(${(x - 28).toFixed(1)}px, ${(-y).toFixed(1)}px) scale(${size.toFixed(2)})`;
  if (fresh) {
    void snowcloudEl.offsetWidth;
    snowcloudEl.style.transition = "";
  }
}
function syncCloud(on) {
  if (snowcloudEl.hidden === !on) return;
  snowcloudEl.hidden = !on;
  cloudAt = null;
}

// 뿅 하고 튀는 반짝이 하나 (동글 그림). 애니메이션이 끝나면 지운다
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

// CSS 효과가 끝나지 않아도(창이 숨겨짐 등) 모자는 반드시 사라진다
const CAP_POOF_MS = 450;
const CAP_POOF_SPARKS = 5;
let capPoofTimer = null;
// 밤에 쓰는 것: 대개 수면 모자, neck 표가 있는 동물은 목도리
const nightWear = () => (manifest?.neck ? (pixelMode ? scarfPxEl : scarfEl) : pixelMode ? capPxEl : nightcapEl);
function setCap(on) {
  clearTimeout(capPoofTimer);
  const wear = nightWear();
  for (const el of [nightcapEl, scarfEl, capPxEl, scarfPxEl]) {
    el.classList.remove("cap-poof");
    el.toggleAttribute("hidden", !on || wear !== el);
  }
}
// 집중 고리: 지금 스타일의 것만 보인다
function applyRing() {
  const ring = typeof lastPresence?.ring === "number";
  focusringEl.toggleAttribute("hidden", !ring || pixelMode);
  ringPxEl.toggleAttribute("hidden", !ring || !pixelMode);
  if (!ring) return;
  if (pixelMode) drawRingPixel(lastPresence.ring);
  else focusringFill.setAttribute("stroke-dashoffset", String(100 - lastPresence.ring));
}
// 픽셀 효과 조각 (effects_pixel/<name>.png): 칸 단위로 계단처럼 움직이고 뚝 사라진다 — 글자·흐림·확대 없음.
// cx = 몸 가운데에서 가로, bottom = 아래에서, (dx, dy) = 움직일 거리 (5계단이 칸에 맞게 맞춘다, dy는 위가 음수)
const BIT_STEPS = 5;
function pixelBit(name, dots, cx, bottom, dx, dy, delayMs = 0, ms = 900) {
  const s = document.createElement("span");
  const side = dots * dot;
  const snap = (v) => Math.round(v / (BIT_STEPS * dot)) * BIT_STEPS * dot;
  s.className = "px-bit";
  s.style.width = s.style.height = side + "px";
  s.style.backgroundImage = `url(effects_pixel/${name}.png)`;
  s.style.left = `calc(50% + ${Math.round((cx - side / 2) / dot) * dot}px)`;
  s.style.bottom = `${Math.round((bottom - side / 2) / dot) * dot}px`;
  s.style.setProperty("--dx", `${snap(dx)}px`);
  s.style.setProperty("--dy", `${snap(dy)}px`);
  s.style.setProperty("--ms", `${ms}ms`);
  s.style.animationDelay = `${delayMs.toFixed(0)}ms`;
  stageEl.appendChild(s);
  setTimeout(() => s.remove(), ms + delayMs + 100);
}
function capPoof() {
  const wear = nightWear();
  const box = wear.getBoundingClientRect();
  const home = stageEl.getBoundingClientRect();
  const cx = box.left + box.width / 2 - home.left - home.width / 2;
  const up = home.bottom - (box.top + box.height / 2);
  for (let i = 0; i < CAP_POOF_SPARKS; i++) {
    const a = (i / CAP_POOF_SPARKS) * Math.PI * 2 + Math.random() * 0.5;
    const r = 14 + Math.random() * 10;
    if (pixelMode) {
      pixelBit("fx_spark2", 3, cx, up, Math.cos(a) * r, -Math.sin(a) * r, 0, 500);
      continue;
    }
    spark(i, up, Math.cos(a) * r, -Math.sin(a) * r, cx);
  }
  wear.classList.add("cap-poof");
  capPoofTimer = setTimeout(() => setCap(false), CAP_POOF_MS);
}

let lastPresence = null;
listen("pet-presence", (e) => {
  const p = e.payload ?? {};
  if (lastPresence?.nightcap === p.nightcap && lastPresence?.ring === p.ring) return;
  // 막 벗는 순간에만 뿅 (앱을 8시 30분 뒤에 켰거나 숨어 있으면 효과 없이 안 보인다) — 시계가 끄므로 인사가 미뤄져도 모자만 남지 않는다
  const capOff = lastPresence?.nightcap && !p.nightcap;
  lastPresence = p;
  if (capOff && !nightWear().hidden && document.visibilityState !== "hidden") capPoof();
  else setCap(!!p.nightcap);
  applyRing();
  headgearAt = null;
});

const writePet = styleWriter(petEl);

function draw(now) {
  if (manifest) {
    // 교체된 동물에 없는 종 전용 애니메이션이어도 throw로 rAF가 끊기지 않게 idle로 대체
    // 큰 점프 그림이 없는 동물은 hop 줄로 (common.js hopLeapFrame)
    const hopLeap = render.anim === "bigjump" && !manifest.animations.bigjump;
    const spec = hopLeap ? manifest.animations.hop : (manifest.animations[render.anim] ?? manifest.animations.idle);
    const elapsed = Math.max(0, now - animStart);
    const neckOn = drawNeck(elapsed);
    const f = hopLeap ? hopLeapFrame(spec, elapsed) : frameIndex(spec, elapsed);
    writePet("backgroundPosition", `${-f * display}px ${-spec.row * display}px`);
    if (lastPresence && (lastPresence.nightcap || typeof lastPresence.ring === "number")) {
      const shown = hopLeap ? "hop" : manifest.animations[render.anim] ? render.anim : "idle";
      placeHeadgear(shown, f, neckOn ? nkHead : petEl);
    }
    if (sheetImg && render.cloud) placeCloud(spec.row, f);
    // 좌우 반전 공간 안에서 돌리므로 왼쪽으로 갈 땐 자동으로 반대로 구른다
    const parts = [];
    const shake = jitter(f, now);
    if (shake) parts.push(shake);
    // 커서에 매달림 (백엔드 render.hang): 창은 머리 꼭대기가 커서 끝에 닿는 자리에 있다 (코어 hang_spot) — 커서 끝을 축으로 살랑살랑
    writePet("transformOrigin", render.hang ? hangOrigin(manifest.head?.[stage.stage]?.dragged, manifest.frame_size) : "");
    // 픽셀은 돌리지 않고 한 칸씩 좌우로 (6도 회전은 칸을 무너뜨린다)
    if (render.hang) parts.push(pixelMode ? `translateX(${Math.round(Math.sin(now / 430) * 1.4) * dot}px)` : `rotate(${(Math.sin(now / 430) * 6).toFixed(2)}deg)`);
    if (render.flip) parts.push("scaleX(-1)");
    // 이동 속도는 √배율, 몸 반지름은 배율에 비례 → 구르는 각속도는 1/√배율
    if (spec.spin && !pixelMode) parts.push(`rotate(${((elapsed / 1000) * spec.spin / Math.sqrt(stage.scale)) % 360}deg)`);
    writePet("transform", parts.length ? parts.join(" ") : "none");
  }
  requestAnimationFrame(draw);
}

// 할로윈·크리스마스 동물의 짧은 효과 (CSS 애니메이션 한 번, style.css .boo/.flare-*/.nose-* — 끝나면 클래스를 뗀다):
// 유령은 간질이면 "부~" 하고 40%까지 흐려졌다 돌아오고, 호박은 누르면(잡을 때도) 빛이 2초쯤 번진다 —
// 누를 때마다 지난번과 다른 무늬 (cls가 배열이면 그중 하나를 고르되 직전 것은 빼고)
const PET_FX = {
  ghost: { on: "tickle", cls: "boo" },
  pumpkin: { on: "click", cls: ["flare-pulse", "flare-flicker", "flare-bloom", "flare-sparkle", "flare-beat"] },
  // 루돌프: 누르면 빨간 코가 환하게 — 따뜻한 빨간 빛이 번지거나, 반짝이와 함께 깜빡깜빡
  rudolph: { on: "click", cls: ["nose-glow", "nose-twinkle"] },
};
const FX_CLASSES = Object.values(PET_FX).flatMap((fx) => [].concat(fx.cls));
let lastFx = null;
function petFx(trigger) {
  const fx = PET_FX[stage.pet];
  if (!fx || fx.on !== trigger) return;
  const pool = [].concat(fx.cls);
  const pick = pool.length > 1 ? pool.filter((c) => c !== lastFx) : pool;
  const cls = pick[Math.floor(Math.random() * pick.length)];
  lastFx = cls;
  petEl.classList.remove(...FX_CLASSES);
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add(cls);
}
// 반짝이(::before/::after)의 animationend도 올라온다 — 몸 자체 애니메이션이 끝날 때만 뗀다
petEl.addEventListener("animationend", (e) => {
  if (e.target === petEl && !e.pseudoElement) petEl.classList.remove(...FX_CLASSES);
});

// 놀러 보내져 손 흔든 뒤 "뿅" (백엔드 render.poof, 0.4초): 몸이 작아지며 흐려지고 반짝이 몇 개가 퍼진다.
// 다 사라지면 백엔드가 창을 숨긴다 — poof가 아닌 render가 오면(취소·돌아옴) 바로 클래스를 뗀다
const POOF_SPARKS = 6;
function poof() {
  petEl.classList.remove("poof");
  void petEl.offsetWidth; // 애니메이션 재시작
  petEl.classList.add("poof");
  const cy = display * 0.45;
  for (let i = 0; i < POOF_SPARKS; i++) {
    const a = (i / POOF_SPARKS) * Math.PI * 2 + Math.random() * 0.5;
    const r = display * (0.28 + Math.random() * 0.12);
    if (pixelMode) {
      pixelBit(i % 2 ? "fx_spark" : "fx_spark2", i % 2 ? 5 : 3, 0, cy, Math.cos(a) * r, -Math.sin(a) * r, 0, 500);
      continue;
    }
    spark(i, cy, Math.cos(a) * r, -Math.sin(a) * r);
  }
}

// 성장 축하 (백엔드 anim "level_up"이 시작될 때 한 번): 하트와 별이 몸 가운데에서 펑 터져 퍼지며 사라진다.
// CSS 애니메이션 한 번 (.burst-bit — 프레임마다 JS 없음), 끝나면 지운다. 조각은 #stage의 자식이라 몸의 변환·모자와 상관없다.
// 성장 순간 백엔드가 창을 새 크기로 바꾸므로 지금 창을 재지 않고 새 몸 크기로 계산한다 (창 = 128 × max(1, 배율) × 36 + 몸)
// BURST-LAYOUT-START (tools/test_levelup_ui.py가 node로 이 부분을 그대로 돌린다)
const BURST_COUNT = 12;
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
    // 커지는 순간(CSS 1.2배)까지 창 안에 남도록 끝자리를 창 안으로 당긴다. 시작점과 끝점이 창 안이면 곧게 가는 길도 창 안이다
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
function levelUpBurst() {
  const now = performance.now();
  if (now - lastBurst < BURST_ONCE_MS || document.visibilityState === "hidden") return;
  lastBurst = now;
  const { cy, bits } = burstLayout(128 * stage.scale);
  for (const b of bits) {
    if (pixelMode) {
      pixelBit(b.heart ? "fx_heart" : "fx_star", 5, 0, cy, b.dx, -b.dy, b.delay);
      continue;
    }
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

// 첫 pet-render는 이 웹뷰가 듣기 전에 지나갔을 수 있다 — 첫 로드 땐 slot_info의 render를 쓴다 (이벤트가 먼저 왔으면 그쪽이 새것)
let gotRender = false;
// 루돌프 코·호박 빛이 나는 동안 앱이 창을 아래로 늘린다: 펫과 머리 위 띠를 그만큼 올려 그린다
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
  // 트레이에서 먹였거나 다른 길로 먹기 시작하면 고르기 버튼은 필요 없다
  if (e.payload.anim === "eat") speech.hidePicker();
  if (e.payload.anim === "tickle" && render.anim !== "tickle") petFx("tickle");
  // 하우스 안이면 하우스 창이 대신 축하한다 (ui/house.js 집 폴짝)
  if (e.payload.anim === "level_up" && render.anim !== "level_up" && !e.payload.housed) levelUpBurst();
  // 뿅은 손 흔들기 끝에서만 시작한다 (걸어 나가 사라진 Away도 poof지만 그건 그림 없이)
  if (e.payload.poof && !render.poof && e.payload.anim !== "idle") poof();
  // poof가 아닌 모습이 오면(취소돼 남음·돌아옴) 바로 다시 보인다. 사라진 뒤(Away)엔 백엔드가 poof를 켜 둔다
  else if (!e.payload.poof) petEl.classList.remove("poof");
  render = e.payload;
  // 조작 모드 중이면 머리 위에 🎮
  ctltagEl.hidden = !render.ctl;
  syncCloud(!!render.cloud);
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
// 도착하면 봉투가 머리 옆에서 통통 튄다. 봉투나 몸을 누르면 평소 말풍선에 "💌 <주인>: <편지>"를 10초
// (이 누름엔 도망·애교 없음) — 한 번 읽으면 봉투는 사라지고, 그 뒤 몸 누르기는 평소 손님 놀이.
// 편지 말풍선이 떠 있는 동안 다른 말풍선(수다·간질·인사·손님 놀이 등)과 음식 고르기는 버린다.
// 말풍선은 여러 줄로 커지므로 그동안 창을 위로 키운다(백엔드 note_space). 글자는 textContent로만 (HTML 아님)
// 내 동물도 친구가 답장을 써서 돌려보내면 같은 봉투를 들고 돌아온다 (`pet-mail` {from, note} — "💌 <친구>: <답장>")
const NOTE_MS = 10000;
let note = null;
let ownerName = "";
let noteRead = false;
let noteOpen = false;
let noteTimer = null;
function openNote() {
  if (!note || noteRead) return;
  noteRead = true;
  envelopeEl.hidden = true;
  noteOpen = true;
  speech.hideBubble();
  speech.hidePicker();
  const text = `💌 ${ownerName}: ${note}`;
  noteTimer = setTimeout(closeNote, NOTE_MS);
  // 창을 위로 키운 뒤(백엔드) 말풍선을 보인다
  invoke("note_space", { open: true }).finally(() => {
    if (!noteOpen) return;
    const span = document.createElement("span");
    span.textContent = text;
    bubbleEl.replaceChildren(span);
    bubbleEl.classList.add("note");
    bubbleEl.hidden = false;
  });
}
function closeNote() {
  clearTimeout(noteTimer);
  if (!noteOpen) return;
  noteOpen = false;
  bubbleEl.hidden = true;
  bubbleEl.classList.remove("note");
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
    clearTimeout(nametagTimer);
    nametagEl.hidden = true;
    // 손님이던 창만 손님 편지를 지운다 (내 동물의 성장·교체(pet-stage)는 받은 답장을 지우지 않는다)
    if (isGuest) {
      note = null;
      noteRead = false;
      envelopeEl.hidden = true;
      closeNote();
    }
    isGuest = false;
  }
}

// 친구 화면에서 답장을 들고 돌아온 내 동물: 머리 옆에 💌 — 봉투나 몸을 누르면 "💌 <친구>: <답장>"을 10초 (한 번 읽으면 봉투는 사라진다)
listen("pet-mail", (e) => {
  const mail = e.payload ?? {};
  if (isGuest || typeof mail.note !== "string" || !mail.note) return;
  closeNote();
  note = mail.note;
  ownerName = typeof mail.from === "string" ? mail.from : "";
  noteRead = false;
  envelopeEl.hidden = false;
});

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
  // 손님이 이 화면 주인의 답장을 들고 집으로
  reply: ["bye~! 💌"],
  // 공룡이 쿵쾅해서 흔들림
  whoa: ["whoa!", "eek!"],
  // 위/아래 친구 화면으로 높이 뛰어 나감·아래에서 튀어 올라옴 / 바닥 밑으로 쏙
  boing: ["boing!"],
  whee: ["whee!"],
  // 눈사람을 다 만들었다
  snowman: ["snowman!", "⛄!"],
  // 커서 끝을 붙잡음 / 서 있던 창이 사라짐 / 내려서서 째려봄 (cursor_play는 아래 CURSOR_LINES)
  gotcha: ["gotcha!", "got you!"],
  eek: ["eek!"],
  hmph: ["hmph!"],
  // 공룡 동작 (pet.json에 없으면 이 말)
  roar: ["RAWR!"],
  fire: ["fwoosh~"],
  stomp: ["stomp!"],
  // 앞에 똥이 있다 / 똥을 밟고 꽈당·친구에게 머리를 밟힘
  ew: ["ew!", "yuck!"],
  oof: ["oof!", "ouch!"],
  // 던져져 날다가 친구에게 부딪혀 되튐 / 날아온 것에 살짝 맞음
  ting: ["ting!", "boink!"],
  ow: ["ow!", "ouch!"],
  // 빠른 새가 친구 화면으로 쌩 날아 나가거나 들어옴
  whoosh: ["whoosh!", "whoosh~"],
  // 깜짝 놀라 벌떡 (레서판다, pet.json에 없으면 이 말)
  surprise: ["!!"],
};
// 🐾 커서 장난: 커서를 보고 장난기가 돌 때 — 동물마다 다르게 (코어 brain_cursor.rs)
const CURSOR_LINES = {
  cat: ["mrrow...", "*wiggle wiggle*"],
  persian: ["...mrr?", "*streeetch*"],
  rabbit: ["hop hop!"],
  squirrel: ["chk chk?", "ooh, shiny?"],
  frog: ["ribbit!"],
  pumpkin: ["bounce!"],
  ghost: ["boo~"],
  duck: ["quack!", "quack quack!"],
  giraffe: ["ooh, up there?"],
  rudolph: ["jingle!"],
  santa: ["ho ho ho!"],
  snowman: ["brrr~!", "snow!"],
  dino_yellow: ["RAWR!"],
  dino_green: ["rawr!"],
  dino_purple: ["rawr~"],
  bird: ["chirp chirp!", "tweet?"],
  penguin: ["wheee~!", "slide slide!"],
  redpanda: ["pounce~!", "kyu kyu!"],
};
const speech = setupSpeech({
  bubbleEl,
  pickerEl,
  favBtn,
  getManifest: () => manifest && { ...manifest, speech: { cursor_play: CURSOR_LINES[stage.pet] ?? ["!"], ...EXTRA_SPEECH, ...manifest.speech } },
  isQuiet: () => noteOpen,
});

// 메뉴 "친구 코드 보기": 머리 위 띠에 "🔑 ABCD-1234", 누르면 복사 (잠시 뒤 사라진다)
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
  // 안 읽은 편지가 있으면 첫 누름은 편지만 연다 (잡기·도망 없음)
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

// 그리기 고리는 먼저 돌린다 — 첫 그림을 못 읽어도 다음 pet-stage로 다시 그릴 수 있게 (그림이 없는 동안 draw는 아무것도 안 한다)
requestAnimationFrame(draw);

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다 — 그 사이 바뀐 것은 이벤트로 온다
  await Promise.all([listeningRender, listeningStage]);
  stage = await invoke("slot_info");
  applyRole(stage);
  if (!gotRender && stage.render) {
    render = stage.render;
    syncCloud(!!render.cloud);
    animStart = performance.now();
  }
  await loadPet();
})();
