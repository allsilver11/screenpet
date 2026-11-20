// 계절 날씨 그리기. 앱(src-tauri/src/weather.rs)이 `weather-scene`으로 내려앉을 곳(tops)과 더미(piles)를 보냄 (논리 px, 이 창 왼쪽 위가 0,0).
// CPU를 아끼는 규칙: 24 fps 이하, 알갱이 80개 이하, 미리 그린 그림을 정수 자리에 찍기, 알갱이 자리만 지우기, 더미는 바뀔 때만.
// 멈춰 있으면(paused) 타이머도 돌지 않음. 브라우저에서 weather.html?preview=snow (light_snow·heavy_snow·rain·shower·petals·leaves·green_leaves·typhoon)로 앱 없이 볼 수 있음.
// 🌀 태풍: 장면의 wind(부호 = 방향, 돌풍에 커짐)로 비·잎이 옆으로 눕고, tornado([x, 바닥 y, vx])가 있으면 깔때기를 그림 (사이는 vx로 이어 그림).
// 깔때기 = 미리 한 번 그려 둔 반투명 먹구름 뭉치들이 나선으로 돌며 오름 (프레임마다 흐림 필터 없음), 찍은 칸만 다음 프레임에 지움.

const FPS = 24;
const FRAME_MS = 1000 / FPS;
const MAX_PARTICLES = 80;
const MIN_PARTICLES = 30;
// 한 프레임에 다시 재 보는 알갱이 수 (옆으로 흘러도 내려앉을 곳이 맞도록)
const RELAND_PER_FRAME = 6;

// count: 폭 1920 논리 px에서의 개수, fall·wind·sway: px/s, rest: 내려앉아 머무는 초, box: 그림 칸, frames: 떨어지며 도는 그림 수,
// flutter: 흔들림·돎 빠르기 (없으면 1.6)
const KINDS = {
  light_snow: { count: 30, fall: [24, 46], wind: [-5, 5], sway: 12, rest: 0.6, box: 9, frames: 1 },
  snow: { count: 55, fall: [28, 62], wind: [-7, 7], sway: 14, rest: 0.6, box: 9, frames: 1 },
  // 🌨️ 함박눈: 크고 몽실한 눈송이가 펑펑, 천천히 살랑이며 내려옴
  heavy_snow: { count: 70, fall: [18, 40], wind: [-9, 9], sway: 22, flutter: 1.2, rest: 0.9, box: 13, frames: 1 },
  rain: { count: 70, fall: [520, 700], wind: [-46, -34], sway: 0, rest: 0.12, box: 18, frames: 1 },
  shower: { count: 70, fall: [620, 820], wind: [-70, -50], sway: 0, rest: 0.12, box: 18, frames: 1 },
  petals: { count: 35, fall: [30, 56], wind: [12, 30], sway: 20, rest: 1.0, box: 13, frames: 8 },
  leaves: { count: 30, fall: [36, 68], wind: [-14, 14], sway: 34, rest: 1.4, box: 15, frames: 8 },
  // 🌿 여름 푸른 잎: 낙엽보다 가볍게 팔랑이며 빙글빙글 (더 크게·더 빨리 흔들림)
  green_leaves: { count: 32, fall: [26, 52], wind: [-22, 22], sway: 48, flutter: 2.8, rest: 1.2, box: 15, frames: 8 },
  // 🌀 태풍: 굵고 비스듬한 빗줄기 (wind는 크기 — 방향·세기는 장면의 wind를 곱함), 네 개에 하나는 옆으로 날아가는 푸른 잎·부스러기
  typhoon: { count: 70, fall: [760, 980], wind: [240, 320], sway: 0, rest: 0.1, box: 18, frames: 1 },
};
// 🌀 태풍 부스러기 몫 (DEBRIS_EVERY개에 하나), 떨어지는 빠르기, 옆으로 더 빠른 배
const DEBRIS_EVERY = 4;
const DEBRIS_FALL = [140, 260];
const DEBRIS_WIND = 1.8;
// 🌀 회오리 깔때기: 높이, 바닥·꼭대기 반지름 (core typhoon.rs FUNNEL_R와 같음 — 뭉치 끝까지 축에서 FUNNEL_R[1] + 30 안)
const FUNNEL_HEIGHT = 300;
const FUNNEL_R = [26, 90];
// 먹구름 뭉치: 층 수, 층마다 줄기 수, 도는 빠르기 (라디안/초), 한 층 위로 갈 때 비틀림, 오르는 빠르기 (층/초), 꼭대기 지붕·바닥 먼지 수
const FUNNEL_LEVELS = 14;
const FUNNEL_STRANDS = 3;
const FUNNEL_SPIN = 2.2;
const FUNNEL_TWIST = 0.62;
const FUNNEL_RISE = 0.45;
const FUNNEL_CAP = 7;
const FUNNEL_DUST = 5;
// 미리 그리는 뭉치 그림 크기 (동글은 늘려 찍음), 이보다 옅은 뭉치는 안 그림
const PUFF_BOX = 96;
const PUFF_MIN_ALPHA = 0.2;
// 뭉치 색 [밝은 속, 그늘, 가장자리]: 먹구름 (뒤 어둡게 → 앞 밝게), 마지막은 바닥 흙먼지
const PUFF_TONES = [
  ["#5d626d", "#3e424c", "#272a31"],
  ["#7b808b", "#555a64", "#383c45"],
  ["#9a9fa9", "#6f747e", "#4b4f58"],
  ["#b3a68e", "#8a7e69", "#5f5646"],
];
const FLUTTER = 1.6;
// 낙엽 색: 속, 테두리
const LEAF_TONES = [
  ["#d6452b", "rgba(120,36,18,0.85)"],
  ["#ee8a24", "rgba(140,76,14,0.85)"],
  ["#f2c230", "rgba(150,110,16,0.85)"],
];

// 푸른 잎 색: 속, 테두리 (밝은 연두·잎새 초록)
const GREEN_TONES = [
  ["#8ed64c", "rgba(46,110,30,0.85)"],
  ["#b4e866", "rgba(70,124,26,0.85)"],
  ["#5cc25e", "rgba(28,96,40,0.85)"],
];

// 픽셀 스타일 (window.ART_STYLE, 앱의 art-style 이벤트): 칸(PX_DOT px)으로만 그림 — 안티에일리어싱 없음, 자리도 칸에 맞춤, 더미는 칸 계단.
// 그림은 글자 표 (글자 → 색), 도는 낙엽·꽃잎은 90도 네 장을 번갈아 (frames 수는 같게 맞춤)
const PX_DOT = 3;
const PX_COLORS = {
  w: "#ffffff", c: "#b4cdeb", b: "#6491d2", B: "#4a70b0",
  p: "#fbc4d4", P: "#d87896",
  r: "#d6452b", R: "#78241a", o: "#ee8a24", O: "#8c4c0e", y: "#f2c230", Y: "#966e10",
  g: "#8ed64c", G: "#2e6e1e", h: "#b4e866", H: "#467c1a", k: "#5cc25e", K: "#1c6028",
};
const PX_FLAKES = [["w"], ["cw", "wc"], [".c.", "cwc", ".c."]];
// 함박눈 픽셀 눈송이: 4~5칸짜리 뭉치
const PX_HEAVY_FLAKES = [
  [".cwc.", "cwwwc", "wwwww", "cwwwc", ".cwc."],
  ["..c..", ".cwc.", "cwwwc", ".cwc.", "..c.."],
  [".cc.", "cwwc", "cwwc", ".cc."],
];
const PX_DROP = ["..b.", "..b.", ".b..", ".b.."];
const PX_SPLASH = ["b..b", ".bb."];
const PX_PETAL = ["pp.", "Ppp", ".pP"];
const PX_LEAF = [".LL.", "LLLF", "FLLL", ".FF."];
const PX_BOX = { light_snow: 9, snow: 9, heavy_snow: 15, rain: 12, shower: 12, petals: 9, leaves: 12, green_leaves: 12, typhoon: 12 };

// 픽셀 그림 줄들을 시계 방향으로 turns번 90도 돌림
function pxRotate(rows, turns) {
  let out = rows;
  for (let t = 0; t < turns; t++) {
    out = Array.from({ length: out[0].length }, (_, x) => out.map((r) => r[x]).reverse().join(""));
  }
  return out;
}

// 표 → 칸 그림 (box 안 가운데, 칸에 맞춰)
function pxSprite(rows, box, colors = PX_COLORS) {
  return makeSprite(box, (g) => {
    const x0 = Math.floor((box / PX_DOT - rows[0].length) / 2) * PX_DOT;
    const y0 = Math.floor((box / PX_DOT - rows.length) / 2) * PX_DOT;
    rows.forEach((row, j) => {
      [...row].forEach((ch, i) => {
        if (ch === ".") return;
        g.fillStyle = colors[ch];
        g.fillRect(x0 + i * PX_DOT, y0 + j * PX_DOT, PX_DOT, PX_DOT);
      });
    });
  });
}

let pixelArt = window.ART_STYLE === "pixel";

const pilesCanvas = document.getElementById("piles");
const fallCanvas = document.getElementById("fall");
const pilesCtx = pilesCanvas.getContext("2d");
const fallCtx = fallCanvas.getContext("2d");

let width = 0;
let height = 0;
let kind = null;
let spec = null;
let sprites = [];
let splash = null;
let tops = [];
let piles = [];
let lastPiles = "";
let pileKind = null;
let running = false;
let timer = 0;
let frameRequest = 0;
let lastT = 0;
let cursor = 0;
let n = 0;
// 🌀 바람 (장면의 wind 쪽으로 부드럽게), 부스러기 그림, 회오리 {x, y, vx, at}과 지난번 지운 자리
let wind = 1;
let windTarget = 1;
let debris = [];
let tornado = null;
let funnelBox = null;
// 알갱이: 자리, 지난 자리, 속도, 흔들림 위상, 내려앉을 높이, 머문 시간(>0이면 내려앉음), 그림
let px, py, oldX, oldY, vy, vx, phase, land, rest, look;

// size×size 캔버스에 draw로 그린 스프라이트를 만듦
function makeSprite(size, draw) {
  const c = document.createElement("canvas");
  c.width = c.height = size;
  draw(c.getContext("2d"), size);
  return c;
}

// 반지름 radius의 눈송이 그리기 함수를 돌려줌
function flake(radius) {
  return (g, s) => {
    g.beginPath();
    g.arc(s / 2, s / 2, radius, 0, Math.PI * 2);
    g.fillStyle = "#fff";
    g.fill();
    // 흰 창 위에서도 보이게 옅은 테두리
    g.lineWidth = 0.8;
    g.strokeStyle = "rgba(120,150,190,0.55)";
    g.stroke();
  };
}

// 반지름 radius의 몽실한 함박눈송이 그리기 함수를 돌려줌: 가운데 동그라미에 작은 혹 넷 (테두리를 먼저 긋고 흰 속을 덮어 바깥 테두리만 남김)
function fluffyFlake(radius) {
  const lobes = [0, 1, 2, 3].map((k) => [Math.cos(k * 1.7 + 0.4) * radius * 0.45, Math.sin(k * 1.7 + 0.4) * radius * 0.45, radius * 0.6]);
  const blobs = [[0, 0, radius * 0.8], ...lobes];
  return (g, s) => {
    g.lineWidth = 1.4;
    g.strokeStyle = "rgba(120,150,190,0.4)";
    for (const [dx, dy, r] of blobs) {
      g.beginPath();
      g.arc(s / 2 + dx, s / 2 + dy, r, 0, Math.PI * 2);
      g.stroke();
    }
    g.fillStyle = "#fff";
    for (const [dx, dy, r] of blobs) {
      g.beginPath();
      g.arc(s / 2 + dx, s / 2 + dy, r, 0, Math.PI * 2);
      g.fill();
    }
  };
}

// 눈송이 그림들 (k: 눈 종류, box: 칸). 함박눈은 크고 몽실하게, 나머지 눈은 작은 동그라미
function flakeSprites(k, box) {
  if (k === "heavy_snow") return pixelArt ? PX_HEAVY_FLAKES.map((rows) => pxSprite(rows, box)) : [3.4, 4.2, 5.0].map((r) => makeSprite(box, fluffyFlake(r)));
  return pixelArt ? PX_FLAKES.map((rows) => pxSprite(rows, box)) : [1.6, 2.2, 2.9].map((r) => makeSprite(box, flake(r)));
}

// angle만큼 돌린 꽃잎 그리기 함수를 돌려줌
function petal(angle) {
  return (g, s) => {
    g.translate(s / 2, s / 2);
    g.rotate(angle);
    g.beginPath();
    g.moveTo(-4.6, 0);
    g.bezierCurveTo(-2.4, -3.4, 2.6, -3.2, 4.8, -0.6);
    g.lineTo(3.6, 0.3);
    g.lineTo(4.6, 1.0);
    g.bezierCurveTo(2.4, 3.2, -2.4, 3.2, -4.6, 0);
    g.fillStyle = "#fbc4d4";
    g.fill();
    g.lineWidth = 0.7;
    g.strokeStyle = "rgba(216,120,150,0.8)";
    g.stroke();
  };
}

// 도는 낙엽: 납작해졌다 펴지며 뒤집히는 것처럼 보임 (a/8 바퀴)
function leaf([fill, line], a) {
  const angle = (a * Math.PI) / 4;
  return (g, s) => {
    g.translate(s / 2, s / 2);
    g.rotate(angle);
    g.scale(1, 0.45 + 0.55 * Math.abs(Math.cos(angle)));
    g.beginPath();
    g.moveTo(-6, 0);
    g.bezierCurveTo(-3, -4.6, 2, -4.4, 6, 0);
    g.bezierCurveTo(2, 4.4, -3, 4.6, -6, 0);
    g.fillStyle = fill;
    g.fill();
    g.lineWidth = 0.8;
    g.strokeStyle = line;
    g.stroke();
    g.beginPath();
    g.moveTo(-6.6, 0);
    g.lineTo(4, 0);
    g.lineWidth = 0.6;
    g.stroke();
  };
}

// 떨어질 때도 더미에서도 같은 그림 (한 번만 만듦). 낙엽은 색마다 frames장: [색0 각도0..7, 색1 각도0..7, ...]
const tumblers = {
  petals: () => [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5].map((k) => makeSprite(KINDS.petals.box, petal(k * 0.8))),
  leaves: () => LEAF_TONES.flatMap((tone) => Array.from({ length: KINDS.leaves.frames }, (_, a) => makeSprite(KINDS.leaves.box, leaf(tone, a)))),
  green_leaves: () => GREEN_TONES.flatMap((tone) => Array.from({ length: KINDS.green_leaves.frames }, (_, a) => makeSprite(KINDS.green_leaves.box, leaf(tone, a)))),
};
// 픽셀: 같은 장 수, 90도씩만 돎
const pxTumblers = {
  petals: () => Array.from({ length: KINDS.petals.frames }, (_, a) => pxSprite(pxRotate(PX_PETAL, a % 4), PX_BOX.petals)),
  leaves: () =>
    [["r", "R"], ["o", "O"], ["y", "Y"]].flatMap(([fill, dark]) =>
      Array.from({ length: KINDS.leaves.frames }, (_, a) => pxSprite(pxRotate(PX_LEAF, a % 4), PX_BOX.leaves, { L: PX_COLORS[fill], F: PX_COLORS[dark] })),
    ),
  green_leaves: () =>
    [["g", "G"], ["h", "H"], ["k", "K"]].flatMap(([fill, dark]) =>
      Array.from({ length: KINDS.green_leaves.frames }, (_, a) => pxSprite(pxRotate(PX_LEAF, a % 4), PX_BOX.green_leaves, { L: PX_COLORS[fill], F: PX_COLORS[dark] })),
    ),
};
const tumblerCache = {};
// 굴러가는 알갱이(꽃잎·잎) k의 스프라이트들 (스타일별로 한 번만 만듦)
function tumblerSprites(k) {
  const key = `${pixelArt}:${k}`;
  if (!tumblerCache[key]) tumblerCache[key] = (pixelArt ? pxTumblers : tumblers)[k]();
  return tumblerCache[key];
}

// 지금 종류·스타일의 떨어지는 알갱이 스프라이트를 만듦
function buildSprites() {
  splash = null;
  debris = kind === "typhoon" ? tumblerSprites("green_leaves") : [];
  if (pixelArt) {
    if (isWet(kind)) {
      sprites = [pxSprite(PX_DROP, spec.box)];
      splash = pxSprite(PX_SPLASH, spec.box);
    } else if (tumblers[kind]) {
      sprites = tumblerSprites(kind);
    } else {
      sprites = flakeSprites(kind, spec.box);
    }
    return;
  }
  if (isWet(kind)) {
    // 태풍 빗줄기는 더 굵고 더 누움
    const slant = kind === "typhoon" ? 4.5 : 1.2;
    sprites = [
      makeSprite(spec.box, (g, s) => {
        g.beginPath();
        g.moveTo(s / 2 + slant, 1);
        g.lineTo(s / 2 - slant, s - 1);
        g.lineWidth = kind === "typhoon" ? 1.6 : 1.2;
        g.lineCap = "round";
        g.strokeStyle = "rgba(100,145,210,0.65)";
        g.stroke();
      }),
    ];
    splash = makeSprite(spec.box, (g, s) => {
      g.lineWidth = 1;
      g.lineCap = "round";
      g.strokeStyle = "rgba(100,145,210,0.7)";
      for (const [dx, dy] of [
        [-4.5, -2.4],
        [4.5, -2.4],
        [-1.6, -4],
        [1.6, -4],
      ]) {
        g.beginPath();
        g.moveTo(s / 2 + dx * 0.4, s - 2 + dy * 0.2);
        g.lineTo(s / 2 + dx, s - 2 + dy);
        g.stroke();
      }
    });
  } else if (tumblers[kind]) {
    sprites = tumblerSprites(kind);
  } else {
    sprites = flakeSprites(kind, spec.box);
  }
}

// 비 그림(빗방울·튀는 물)을 쓰는 날씨
function isWet(k) {
  return k === "rain" || k === "shower" || k === "typhoon";
}

// 🌀 이 알갱이가 태풍 부스러기 (푸른 잎)
function isDebris(i) {
  return debris.length > 0 && i % DEBRIS_EVERY === DEBRIS_EVERY - 1;
}

// [a, b) 사이 난수
function between([a, b]) {
  return a + Math.random() * (b - a);
}

// (x, y) 바닥의 더미 높이 (없으면 0)
function pileHeightAt(x, y) {
  for (const p of piles) {
    if (p.y !== y) continue;
    const h = p.h[Math.floor((x - p.x) / p.cell)];
    if (h) return h / 10;
  }
  return 0;
}

// x에서 y 아래 가장 먼저 만나는 디딤판 (더미 꼭대기). 없으면 화면 밑
function landAt(rawX, y) {
  // 화면 옆으로 살짝 나간 알갱이도 끝자리의 디딤판을 봄 (다시 들어올 때 뚫고 지나가지 않게)
  const x = Math.min(Math.max(rawX, 0), width);
  let best = Infinity;
  for (const t of tops) {
    if (x >= t[0] && x <= t[1] && t[2] >= y && t[2] < best) best = t[2];
  }
  return best === Infinity ? height + spec.box : best - pileHeightAt(x, best);
}

// 알갱이 i를 위에서 다시 떨어뜨림 (first면 화면 곳곳에서 시작)
function respawn(i, first) {
  // 바람에 밀려 오는 만큼 옆 여유를 두고 위에서 떨어짐
  px[i] = Math.random() * width;
  py[i] = first ? -Math.random() * height : -spec.box - Math.random() * height * 0.2;
  vy[i] = between(spec.fall);
  vx[i] = between(spec.wind);
  phase[i] = Math.random() * Math.PI * 2;
  look[i] = Math.floor(Math.random() * sprites.length);
  if (isDebris(i)) {
    // 바람 반대쪽 끝에서 들어와 옆으로 휙 지나감
    vy[i] = between(DEBRIS_FALL);
    vx[i] *= DEBRIS_WIND;
    look[i] = Math.floor(Math.random() * debris.length);
  }
  rest[i] = 0;
  land[i] = landAt(px[i], py[i]);
}

// 날씨 종류를 바꾸고 알갱이를 새로 만듦
function startKind(next) {
  kind = next;
  spec = KINDS[kind] || KINDS.snow;
  if (pixelArt) spec = { ...spec, box: PX_BOX[kind] ?? PX_BOX.snow };
  buildSprites();
  n = Math.round((spec.count * Math.max(width, 1)) / 1920);
  n = Math.min(MAX_PARTICLES, Math.max(MIN_PARTICLES, n));
  [px, py, oldX, oldY, vy, vx, phase, land, rest] = Array.from({ length: 9 }, () => new Float32Array(n));
  look = new Uint8Array(n);
  fallCtx.clearRect(0, 0, width, height);
  for (let i = 0; i < n; i++) {
    respawn(i, true);
    oldX[i] = px[i];
    oldY[i] = py[i];
  }
}

// 이 알갱이의 지금 그림: 내려앉은 빗방울은 튀는 그림, 꽃잎·낙엽은 제 색 그대로 각도만 돎
function spriteFor(i) {
  if (isDebris(i)) {
    // 같은 색 8장 안에서 빙글빙글
    const a = look[i] % 8;
    return debris[look[i] - a + (a + Math.floor(phase[i] * 3)) % 8];
  }
  if (splash && rest[i] > 0) return splash;
  const frames = spec.frames;
  if (frames === 1) return sprites[look[i]];
  const angle = look[i] % frames;
  return sprites[look[i] - angle + (angle + Math.floor(phase[i] * 1.2)) % frames];
}

// ---- 🧹 흩날리는 알갱이 (빗자루로 쓴 것, 친구가 쓸어 보낸 것) ----
// 받은 속도로 날아가다 공기에 느려지며 천천히 떨어져 사라짐. 없으면 일도 없음
const MAX_FLURRY = 120;
const FLURRY_DRAG = 1.8;
const FLURRY_GRAVITY = 90;
const FLURRY_MAX_FALL = 90;
const FLURRY_MAX_SPEED = 900;
const FLURRY_LIFE = [1.8, 3.6];
let flurry = [];

// 날아 들어오는 알갱이 칸 크기
function flurryBox(k) {
  return pixelArt ? PX_BOX[k] ?? PX_BOX.snow : (KINDS[k] || KINDS.snow).box;
}

// 날아 들어오는 알갱이 k의 스프라이트들
function flurrySprites(k) {
  if (tumblers[k]) return tumblerSprites(k);
  const heavy = k === "heavy_snow";
  const key = `${pixelArt}:${heavy ? "heavy_flake" : "flake"}`;
  if (!tumblerCache[key]) tumblerCache[key] = flakeSprites(heavy ? k : "snow", flurryBox(heavy ? k : "snow"));
  return tumblerCache[key];
}

// 친구가 쓸어 보낸 알갱이 뭉치를 더함 (속도·수 상한 있음)
function addFlurry(f) {
  if (!running || !KINDS[f.kind]) return;
  const speed = Math.hypot(f.vx, f.vy);
  const limit = speed > FLURRY_MAX_SPEED ? FLURRY_MAX_SPEED / speed : 1;
  const count = Math.min(f.count, MAX_FLURRY);
  const look = flurrySprites(f.kind);
  const box = flurryBox(f.kind);
  const tumbling = !!tumblers[f.kind];
  // 넘치면 가장 오래된 것부터 (지울 자리는 이번 프레임에 함께 지움)
  const over = flurry.length + count - MAX_FLURRY;
  if (over > 0) {
    for (const p of flurry.splice(0, over)) fallCtx.clearRect(p.oldX, p.oldY, p.box, p.box);
  }
  for (let i = 0; i < count; i++) {
    const spread = 0.55 + Math.random() * 0.9;
    flurry.push({
      x: f.x + (Math.random() - 0.5) * 50,
      y: f.y + (Math.random() - 0.5) * 50,
      vx: f.vx * limit * spread + (Math.random() - 0.5) * 120,
      vy: f.vy * limit * spread - Math.random() * 110,
      age: 0,
      life: between(FLURRY_LIFE),
      look,
      first: Math.floor(Math.random() * look.length),
      tumbling,
      box,
      oldX: -box * 2,
      oldY: -box * 2,
    });
  }
}

// 날아 들어오는 알갱이의 지난 자리를 지움
function clearFlurry() {
  for (const p of flurry) fallCtx.clearRect(p.oldX, p.oldY, p.box, p.box);
}

// 날아 들어오는 알갱이를 dt만큼 움직이고 다 된 것은 뺌
function stepFlurry(dt) {
  const drag = Math.max(0, 1 - FLURRY_DRAG * dt);
  let kept = 0;
  for (const p of flurry) {
    p.age += dt;
    p.vx *= drag;
    p.vy = Math.min(p.vy * drag + FLURRY_GRAVITY * dt, FLURRY_MAX_FALL + Math.max(0, p.vy * drag));
    p.x += p.vx * dt;
    p.y += p.vy * dt;
    if (p.age >= p.life || p.x < -p.box || p.x > width + p.box || p.y > height + p.box) continue;
    flurry[kept++] = p;
    // 도는 그림은 같은 색 8장 안에서 돎
    const sprite = p.tumbling ? p.look[p.first - (p.first % 8) + ((p.first % 8) + Math.floor(p.age * 9)) % 8] : p.look[p.first];
    const x = pixelArt ? Math.floor((p.x - p.box / 2) / PX_DOT) * PX_DOT : (p.x - p.box / 2) | 0;
    const y = pixelArt ? Math.floor((p.y - p.box / 2) / PX_DOT) * PX_DOT : (p.y - p.box / 2) | 0;
    fallCtx.drawImage(sprite, x, y);
    p.oldX = x;
    p.oldY = y;
  }
  flurry.length = kept;
}

// 매 프레임 알갱이를 움직여 그림
function frame(now) {
  if (!running) return;
  const dt = Math.min((now - lastT) / 1000, 0.1);
  lastT = now;
  clearFlurry();
  const box = spec.box;
  const half = box / 2;
  const sway = spec.sway;
  const flutter = spec.flutter || FLUTTER;
  // 🌀 태풍만 장면의 바람을 곱함 (다른 날씨는 1 그대로)
  wind += (windTarget - wind) * Math.min(1, dt * 3);
  const blow = kind === "typhoon" ? wind : 1;
  clearFunnel();
  for (let i = 0; i < n; i++) fallCtx.clearRect(oldX[i], oldY[i], box, box);
  for (let k = 0; k < RELAND_PER_FRAME && n > 0; k++) {
    cursor = (cursor + 1) % n;
    if (rest[cursor] === 0) land[cursor] = landAt(px[cursor], py[cursor]);
  }
  for (let i = 0; i < n; i++) {
    if (rest[i] > 0) {
      rest[i] -= dt;
      if (rest[i] <= 0) respawn(i, false);
    } else {
      phase[i] += dt * flutter;
      px[i] += (vx[i] * blow + Math.sin(phase[i]) * sway) * dt;
      py[i] += vy[i] * dt;
      if (px[i] < -box) px[i] = width + box / 2;
      else if (px[i] > width + box) px[i] = -box / 2;
      if (py[i] >= land[i]) {
        py[i] = land[i];
        rest[i] = spec.rest;
      }
    }
    if (py[i] < -box) {
      oldX[i] = -box * 2;
      oldY[i] = -box * 2;
      continue;
    }
    const sprite = spriteFor(i);
    // 그림 아래쪽이 내려앉는 높이에 닿음
    // 픽셀: 자리도 칸에 맞춤
    const x = pixelArt ? Math.floor((px[i] - half) / PX_DOT) * PX_DOT : (px[i] - half) | 0;
    const y = pixelArt ? Math.floor((py[i] - box + 2) / PX_DOT) * PX_DOT : (py[i] - box + 2) | 0;
    fallCtx.drawImage(sprite, x, y);
    oldX[i] = x;
    oldY[i] = y;
  }
  stepFlurry(dt);
  drawFunnel(now);
  schedule();
}

// ---- 🌀 회오리 깔때기 ----

// 지난번 깔때기 자리를 지움
function clearFunnel() {
  if (!funnelBox) return;
  fallCtx.clearRect(funnelBox[0], funnelBox[1], funnelBox[2], funnelBox[3]);
  funnelBox = null;
}

// 뭉게구름 한 덩이를 이루는 둥근 덩이들 (0~1 칸 기준 {x, y, r}), 아래 것 먼저 — 위 덩이가 밝게 덮음
function billows(seed) {
  const out = [{ x: 0.5, y: 0.55, r: 0.3 }];
  for (let i = 1; i < 5; i++) {
    const a = ((i - 1) / 4 + 0.25 * hash(seed + 1, i)) * Math.PI * 2;
    const d = 0.1 + 0.07 * hash(i, seed + 2);
    out.push({ x: 0.5 + Math.cos(a) * d, y: 0.5 + Math.sin(a) * d * 0.6, r: 0.18 + 0.08 * hash(seed + 3, i) });
  }
  return out.sort((p, q) => q.y - p.y);
}

// "#rrggbb" → rgba 문자열
function rgba(hex, a) {
  const v = parseInt(hex.slice(1), 16);
  return `rgba(${v >> 16},${(v >> 8) & 255},${v & 255},${a})`;
}

// 동글 뭉치: 덩이마다 왼쪽 위가 밝은 둥근 번짐 (가장자리는 투명하게 녹음)
function softPuff(tone, seed) {
  return (g, s) => {
    for (const b of billows(seed)) {
      const x = b.x * s, y = b.y * s, r = b.r * s;
      const grad = g.createRadialGradient(x - r * 0.3, y - r * 0.35, r * 0.1, x, y, r);
      grad.addColorStop(0, rgba(tone[0], 0.85));
      grad.addColorStop(0.45, rgba(tone[1], 0.7));
      grad.addColorStop(0.75, rgba(tone[2], 0.3));
      grad.addColorStop(1, rgba(tone[2], 0));
      g.fillStyle = grad;
      g.beginPath();
      g.arc(x, y, r, 0, Math.PI * 2);
      g.fill();
    }
  };
}

// 픽셀 뭉치: dots×dots 칸을 덩이 안이면 세 단 그늘로 채움 (계단 모양 가장자리)
function pxPuff(tone, seed, dots) {
  const bill = billows(seed);
  return (g) => {
    for (let j = 0; j < dots; j++) {
      for (let i = 0; i < dots; i++) {
        const u = (i + 0.5) / dots;
        const v = (j + 0.5) / dots;
        let b = null;
        for (let k = bill.length - 1; k >= 0 && !b; k--) if ((u - bill[k].x) ** 2 + (v - bill[k].y) ** 2 < bill[k].r ** 2) b = bill[k];
        if (!b) continue;
        const lit = (-(u - b.x) * 0.6 - (v - b.y) * 0.8) / b.r;
        g.fillStyle = tone[lit > 0.3 ? 0 : lit > -0.4 ? 1 : 2];
        g.fillRect(i * PX_DOT, j * PX_DOT, PX_DOT, PX_DOT);
      }
    }
  };
}

// 뭉치 그림 (처음 한 번만 그려 둠): 동글은 PUFF_BOX 하나를 늘려 찍고, 픽셀은 크기(짝수 칸)마다 따로
const puffCache = {};
function puffSprite(tone, seed, size) {
  const dots = Math.max(6, Math.round(size / PX_DOT / 2) * 2);
  const key = pixelArt ? `px:${tone}:${seed}:${dots}` : `${tone}:${seed}`;
  if (!puffCache[key]) {
    puffCache[key] = pixelArt ? makeSprite(dots * PX_DOT, pxPuff(PUFF_TONES[tone], seed, dots)) : makeSprite(PUFF_BOX, softPuff(PUFF_TONES[tone], seed));
  }
  return puffCache[key];
}

// 이번 프레임의 뭉치들 (자리 x·y = 가운데, 지름, 깊이 −1 뒤 ~ 1 앞, 투명도, 색, 모양) — 프레임마다 새로 만들지 않고 다시 씀
const funnelPuffs = [];
let puffCount = 0;
function addPuff(x, y, size, depth, alpha, tone, seed) {
  if (alpha < PUFF_MIN_ALPHA) return;
  const p = funnelPuffs[puffCount] || (funnelPuffs[puffCount] = {});
  puffCount++;
  p.x = x;
  p.y = y;
  p.size = size;
  p.depth = depth;
  p.alpha = Math.min(alpha, 0.9);
  p.tone = tone;
  p.seed = seed;
}

// 깊이에 따른 먹구름 색 (뒤는 어둡게)
function toneAt(depth) {
  return depth < -0.3 ? 0 : depth < 0.4 ? 1 : 2;
}

// 회오리를 그림: 반투명 먹구름 뭉치들이 축을 돌며 나선으로 올라감 (위가 넓고 바닥으로 좁아짐), 꼭대기는 먹구름 지붕, 바닥은 흙먼지.
// 앱이 준 자리에서 vx로 이어 감. 그린 칸을 모두 모아 funnelBox로 — 다음 프레임에 그대로 지움 (회색 잔상 없음)
function drawFunnel(now) {
  if (!tornado || kind !== "typhoon") return;
  const t = now / 1000;
  // 모니터 끝에 걸치면 x가 화면 밖일 수 있음 (반쪽만 보임) — 깔때기 폭만큼만 밖으로
  const out = FUNNEL_R[1] + 30;
  const x = Math.min(Math.max(tornado.x + (tornado.vx * (now - tornado.at)) / 1000, -out), width + out);
  const y = tornado.y;
  const spin = t * FUNNEL_SPIN;
  // 축이 뱀처럼 휨 (위로 갈수록 더)
  const axis = (f) => x + Math.sin(t * 1.6 + f * 2.4) * 9 * f;
  const radius = (f) => FUNNEL_R[0] + (FUNNEL_R[1] - FUNNEL_R[0]) * Math.pow(f, 1.8);
  puffCount = 0;
  // 나선 줄기: 층 높이 f가 계속 올라가고 (꼭대기에서 바닥으로 돌아옴), 각도는 f로만 정해져 돌아와도 튀지 않음. 위·아래 끝은 옅게
  const rise = (t * FUNNEL_RISE) % 1;
  for (let k = 0; k < FUNNEL_LEVELS; k++) {
    const f = (k + rise) / FUNNEL_LEVELS;
    const r = radius(f);
    const fade = Math.min(1, 0.4 + f * 8, (1 - f) * 4);
    const cy = y - 12 - f * (FUNNEL_HEIGHT - 12);
    // 가운데 몸통 (틈을 메워 이어진 깔때기로 보이게)
    addPuff(axis(f), cy, r * 1.25 + 8, 0, 0.5 * fade, 1, (k % 3) + 6);
    for (let j = 0; j < FUNNEL_STRANDS; j++) {
      const a = spin + f * FUNNEL_LEVELS * FUNNEL_TWIST + (j * Math.PI * 2) / FUNNEL_STRANDS;
      const depth = Math.sin(a);
      const size = (r * 1.1 + 8) * (1 + 0.12 * depth);
      addPuff(axis(f) + Math.cos(a) * r * 0.5, cy + depth * r * 0.16, size, depth, (0.55 + 0.25 * depth) * fade, toneAt(depth), j);
    }
  }
  // 꼭대기 먹구름 지붕: 넓게 천천히 돎 (어두운 색)
  for (let j = 0; j < FUNNEL_CAP; j++) {
    const a = spin * 0.5 + (j * Math.PI * 2) / FUNNEL_CAP;
    const depth = Math.sin(a);
    const r = FUNNEL_R[1];
    addPuff(axis(1) + Math.cos(a) * r * 0.62, y - FUNNEL_HEIGHT + depth * r * 0.14, (r * 0.95 + 10) * (1 + 0.1 * depth), depth, 0.65 + 0.15 * depth, depth < 0.2 ? 0 : 1, j + 3);
  }
  // 바닥 흙먼지: 작고 빠르게 돎
  for (let j = 0; j < FUNNEL_DUST; j++) {
    const a = -spin * 1.6 + (j * Math.PI * 2) / FUNNEL_DUST;
    const depth = Math.sin(a);
    addPuff(x + Math.cos(a) * FUNNEL_R[0] * 1.5, y - 14 + depth * 5, 42 + 8 * depth, depth, 0.35 + 0.12 * depth, 3, j);
  }
  // 뒤 → 앞 차례로 찍고, 찍은 칸을 모두 감싸는 상자를 남김
  const list = funnelPuffs.slice(0, puffCount).sort((p, q) => p.depth - q.depth);
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  const g = fallCtx;
  g.save();
  for (const p of list) {
    const sprite = puffSprite(p.tone, p.seed, p.size);
    // 픽셀: 칸에 맞춰 늘리지 않고 찍음, 동글: 지름만큼 늘려 찍음
    const s = pixelArt ? sprite.width : p.size;
    const dx = pixelArt ? Math.floor((p.x - s / 2) / PX_DOT) * PX_DOT : p.x - s / 2;
    const dy = pixelArt ? Math.floor((p.y - s / 2) / PX_DOT) * PX_DOT : p.y - s / 2;
    g.globalAlpha = p.alpha;
    if (pixelArt) g.drawImage(sprite, dx, dy);
    else g.drawImage(sprite, dx, dy, s, s);
    x0 = Math.min(x0, dx);
    y0 = Math.min(y0, dy);
    x1 = Math.max(x1, dx + s);
    y1 = Math.max(y1, dy + s);
  }
  g.restore();
  // 늘려 찍은 가장자리 번짐까지 1 px 여유
  if (list.length) funnelBox = [Math.floor(x0) - 1, Math.floor(y0) - 1, Math.ceil(x1) - Math.floor(x0) + 2, Math.ceil(y1) - Math.floor(y0) + 2];
}

// rAF는 창이 가려지면 쉬고, 타이머가 24 fps 아래로 묶어 둠 (rAF만 쓰면 화면 주사율마다 깨어남)
function schedule() {
  timer = setTimeout(() => {
    frameRequest = requestAnimationFrame(frame);
  }, FRAME_MS);
}

// 그리기를 시작함
function start() {
  if (running || !spec) return;
  running = true;
  lastT = performance.now();
  schedule();
}

// 그리기를 멈추고 캔버스를 비움
function stop() {
  running = false;
  clearTimeout(timer);
  // 타이머가 이미 지나 rAF를 기다리는 중일 수 있음 (가려진 창은 오래 기다림) — 안 끊으면 다시 켤 때 고리가 둘이 됨
  cancelAnimationFrame(frameRequest);
  fallCtx.clearRect(0, 0, width, height);
  pilesCtx.clearRect(0, 0, width, height);
  lastPiles = "";
  flurry = [];
  funnelBox = null;
}

// (a, b)로 정해지는 0~1 의사 난수
function hash(a, b) {
  const v = Math.sin(a * 127.1 + b * 311.7) * 43758.5453;
  return v - Math.floor(v);
}

// 눈 더미 하나를 매끈한 언덕으로 그림
function drawSnowPile(p) {
  const y = p.y;
  const g = pilesCtx;
  g.beginPath();
  g.moveTo(p.x, y);
  let prevX = p.x;
  let prevY = y;
  for (let i = 0; i < p.h.length; i++) {
    const cx = p.x + (i + 0.5) * p.cell;
    const cy = y - p.h[i] / 10;
    g.quadraticCurveTo(prevX, prevY, (prevX + cx) / 2, (prevY + cy) / 2);
    prevX = cx;
    prevY = cy;
  }
  g.quadraticCurveTo(prevX, prevY, p.x + p.h.length * p.cell, y);
  g.closePath();
  g.fillStyle = "#fdfeff";
  g.fill();
  g.lineWidth = 1;
  g.strokeStyle = "rgba(120,150,190,0.8)";
  g.stroke();
}

// 픽셀 눈 더미: 칸 너비 기둥을 칸 높이로 끊어 쌓음 (곡선 없음). 맨 위 칸은 옅은 파랑 — 흰 창 위에서도 보이게
function drawSnowPilePixel(p) {
  const g = pilesCtx;
  const n = p.h.length;
  const heightAt = (x) => {
    const u = (x - p.x) / p.cell - 0.5;
    const i = Math.max(0, Math.min(n - 1, Math.floor(u)));
    const j = Math.min(n - 1, i + 1);
    const t = Math.max(0, Math.min(1, u - i));
    return (p.h[i] + (p.h[j] - p.h[i]) * t) / 10;
  };
  const left = Math.floor(p.x / PX_DOT) * PX_DOT;
  for (let x = left; x < p.x + n * p.cell; x += PX_DOT) {
    const h = Math.round(heightAt(x + PX_DOT / 2) / PX_DOT) * PX_DOT;
    if (h <= 0) continue;
    g.fillStyle = PX_COLORS.w;
    g.fillRect(x, p.y - h, PX_DOT, h);
    g.fillStyle = PX_COLORS.c;
    g.fillRect(x, p.y - h, PX_DOT, PX_DOT);
  }
}

// 꽃잎·잎 더미 하나를 스프라이트로 그림
function drawSpritePile(p) {
  const g = pilesCtx;
  const look = tumblerSprites(pileKind);
  const half = (pixelArt ? PX_BOX[pileKind] : KINDS[pileKind].box) >> 1;
  for (let i = 0; i < p.h.length; i++) {
    const count = Math.round((p.h[i] / 10) * 1.1);
    for (let k = 0; k < count; k++) {
      const x = p.x + (i + hash(i, k)) * p.cell;
      const y = p.y - 3 - hash(k, i + 7) * (p.h[i] / 10);
      const dx = pixelArt ? Math.floor((x - half) / PX_DOT) * PX_DOT : (x - half) | 0;
      const dy = pixelArt ? Math.floor((y - half) / PX_DOT) * PX_DOT : (y - half) | 0;
      g.drawImage(look[Math.floor(hash(i + 3, k) * look.length)], dx, dy);
    }
  }
}

// 장면이 바뀔 때만 (많아야 1초에 몇 번)
function drawPiles() {
  const g = pilesCtx;
  g.clearRect(0, 0, width, height);
  for (const p of piles) {
    g.save();
    g.beginPath();
    for (const c of p.clips) g.rect(c[0], p.y - 32, c[1] - c[0], 33);
    g.clip();
    if (tumblers[pileKind]) drawSpritePile(p);
    else if (pixelArt) drawSnowPilePixel(p);
    else drawSnowPile(p);
    g.restore();
  }
}

// 받은 장면(멈춤, 종류, 더미)을 적용함
function applyScene(s) {
  if (s.paused) {
    stop();
    return;
  }
  tops = s.tops;
  piles = s.piles;
  pileKind = s.pile_kind;
  // 🌀 태풍 바람·회오리 (다른 날씨의 장면엔 없음)
  windTarget = typeof s.wind === "number" ? s.wind : 1;
  tornado = Array.isArray(s.tornado) ? { x: s.tornado[0], y: s.tornado[1], vx: s.tornado[2], at: performance.now() } : null;
  if (s.kind !== kind) startKind(s.kind);
  else {
    for (let i = 0; i < n; i++) {
      if (rest[i] === 0) land[i] = landAt(px[i], py[i]);
    }
  }
  const pilesNow = JSON.stringify([pileKind, piles]);
  if (pilesNow !== lastPiles) {
    lastPiles = pilesNow;
    drawPiles();
  }
  start();
}

// 캔버스를 창 크기에 맞춤
function fitCanvas() {
  width = window.innerWidth;
  height = window.innerHeight;
  pilesCanvas.width = fallCanvas.width = width;
  pilesCanvas.height = fallCanvas.height = height;
  // 동글은 예전 그대로 (캔버스 스타일을 건드리지 않음)
  if (pixelArt || pilesCanvas.style?.imageRendering) pilesCanvas.style.imageRendering = fallCanvas.style.imageRendering = pixelArt ? "pixelated" : "";
  if (kind) {
    const k = kind;
    kind = null;
    startKind(k);
  }
  // 크기를 바꾸면 캔버스가 지워짐 — 장면이 다시 오길 기다리지 않음 (멈춘 동안은 빈 채로, 그때 lastPiles도 비어 있음)
  if (running) drawPiles();
}

// 스타일이 바뀌면 알갱이·더미를 새 그림으로
function setPixelArt(on) {
  if (on === pixelArt) return;
  pixelArt = on;
  fitCanvas();
}

window.addEventListener("resize", fitCanvas);
window.addEventListener("contextmenu", (e) => e.preventDefault());
fitCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 장면만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("art-style", (e) => setPixelArt(e.payload === "pixel"));
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("weather-flurry", (e) => addFlurry(e.payload));
  window.__TAURI__.webviewWindow.getCurrentWebviewWindow().listen("weather-scene", (e) => applyScene(e.payload));
} else {
  devPreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 창 두 개 + 자란 더미
function devPreview() {
  const wanted = new URLSearchParams(location.search).get("preview") || "snow";
  if (new URLSearchParams(location.search).get("pixel")) setPixelArt(true);
  const wet = isWet(wanted);
  document.documentElement.style.background = "#3a4258";
  const windows = [
    [Math.round(width * 0.08), Math.round(height * 0.3), Math.round(width * 0.5), Math.round(height * 0.86)],
    [Math.round(width * 0.42), Math.round(height * 0.16), Math.round(width * 0.9), Math.round(height * 0.64)],
  ];
  for (const [l, t, r, b] of windows) {
    const el = document.createElement("div");
    el.style.cssText = `position:absolute;left:${l}px;top:${t}px;width:${r - l}px;height:${b - t}px;background:#f4f5f8;border-radius:6px;box-shadow:0 4px 18px #0005`;
    document.body.insertBefore(el, pilesCanvas);
  }
  const back = windows[0];
  const front = windows[1];
  const tops = [
    [front[0], front[2], front[1]],
    [back[0], front[0], back[1]],
    [0, width, height],
  ];
  const cell = 20;
  const mound = (x0, x1, i) => Math.round(30 + 34 * (0.5 + 0.5 * Math.sin((i * cell) / 90 + x0)) + 12 * Math.sin(i * 1.7));
  const pile = (x0, x1, y, clips) => {
    const h = [];
    for (let i = 0; i < Math.ceil((x1 - x0) / cell); i++) h.push(tumblers[wanted] ? Math.round(mound(x0, x1, i) * 0.5) : mound(x0, x1, i));
    return { x: x0, y, cell, h, clips };
  };
  const piles = wet ? [] : [pile(front[0], front[2], front[1], [[front[0], front[2]]]), pile(back[0], back[2], back[1], [[back[0], front[0]]]), pile(0, width, height, [[0, width]])];
  const storm = wanted === "typhoon" ? { wind: -1.3, tornado: [Math.round(width * 0.3), height, 90] } : {};
  applyScene({ paused: false, kind: wanted, pile_kind: wet ? null : wanted, tops, piles, ...storm });
}
