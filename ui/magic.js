// ✨ 마법 효과 그리기 (🐍 볼드모트·👻 디멘터·⚔️ 결투·🦌🦦 패트로누스·🧔 해그리드의 용알·🔥 모닥불). 🌈 무지개 구름·🖍️ 낙서·🕷️ 거미집·🦇 박쥐 신호·🟡 스니치와 같은
// 모니터 덮개 창 (trail.html)의 스니치 아래 캔버스에 그림. 앱(src-tauri/src/trail.rs)이 보내는 것 (논리 px, 이 창 왼쪽 위가 0,0 — 코어 magic.rs):
//   trail-magic  지금 효과들 [{kind, ...}] — 바뀔 때만 (최대 30번/초) 오고, 다 사라지면 빈 목록 한 번
//     bolt  {from, to, color, width}             ⚡ Stupefy 초록 빛줄기 (사라지면 MG_FADE 동안 흐려짐)
//     duel  {red, green, clash, reach, width, light}  ⚔️ 밝은 쪽 (red 자리 — light = red 해리·gold 헤르미온느)·초록 빛줄기가 clash까지
//                                                (reach 0…1), 맞부딪힌 자리에 반짝 (사라지면 펑)
//     snake {x, y, size, dir, t, alpha}          🐍 나기니 — 머리 (x, 바닥 y)에서 뒤로 꿈틀대는 몸, 혀 날름
//     stag  {x, y, size, dir, alpha}             🦌 은빛 사슴 — 달리는 다리, 큰 뿔, 은빛 번짐
//     otter {x, y, size, dir, alpha, tilt, t}    🦦 은빛 수달 (헤르미온느) — 긴 몸을 tilt만큼 기울여 공중을 헤엄침, 젓는 발·흔들리는 꼬리·물방울 자취
//     soul  {from, to, width, level, t}          👻 디멘터가 빼앗는 영혼 줄기 — 펫 가슴 (from)에서 디멘터 (to)로 일렁이며 빨려 올라가는 옅은 은빛 가닥
//                                                + 가슴의 희미한 빛 + 올라가는 작은 빛 (level 0…1 짙기)
//     frost {x, y, size, inward, level}          🥶 화면 구석의 서리 — 김 서린 듯 구석에서 안쪽으로 번지는 뿌연 흰빛
//     frost_wipe {x, y, r, clear}                🖐️ 펫을 들고 문질러 닦은 서리 자리 — 서리를 그린 바로 뒤에 그 둘레 r을 clear (0…1)만큼 지움
//                                                (김 서린 유리에 손가락으로 그은 듯. 다시 뿌예질수록 clear가 줄어듦. MG_MAX와 따로 MG_MAX_WIPES개까지)
//     egg   {x, y, size, crack}                  🥚 해그리드가 품은 용알 (바닥 가운데 x, y — 높이 size) — 금이 갈수록 (crack 0…1) 지그재그 금·달달 떨림
//     hatchling {x, y, size, t, len, dir}        🐉 깨어난 아기 용 — 알 껍데기가 갈라져 튀고, 알 자리 둘레를 파닥파닥 날다가 끝에 불을 퐁 뿜고 사라짐
//     campfire {x, y, size, level}               🔥 해그리드가 피운 모닥불 (바닥 가운데 x, y — 장작 폭 size) — 엇갈린 장작 + 둥근 돌 + 일렁이는
//                                                불꽃 세 겹 (level 0…1만큼 큼 — 피울 때 커지고 끌 때 줄어듦) + 튀어 오르는 불티 + 은은한 불빛
//     wand  {from, to, k, size}                  🪄 ⚔️ Expelliarmus에 맞아 날아간 지팡이 — 손 (from)에서 땅 (to)까지 k (0…1)만큼 포물선을 그리며 빙글빙글
//     glow  {x, y, size, level}                  💡 "Lumos!" 지팡이 끝 불빛 — 하얗게 빛나는 속 + 크게 번지는 따뜻한 빛 (level 0…1 밝기, 살짝 일렁임)
//     puff  {x, y, size, k}                      🌑 "Nox!" 불이 꺼진 지팡이 끝의 작은 검은 연기 — 덩이 몇 개가 피어올라 커지며 흐려짐 (k 0…1 퍼진 몫)
// CPU를 아끼는 규칙: 움직이는 효과 (빛줄기·뱀·사슴·흐려짐·펑)가 있는 동안만 requestAnimationFrame. 서리만 있으면 받을 때 한 번만 그림.
// 한 번에 MG_MAX개까지. 브라우저에서 trail.html?magic=1로 앱 없이 볼 수 있음 (결투·사슴·수달·영혼 줄기·뱀·서리).

const MG_MAX = 24;
// 🖐️ 서리 닦은 자리는 따로 이만큼까지 (구석 넷 × 코어 FROST_MAX_WIPES보다 조금 적게 — 많아도 원 몇십 개)
const MG_MAX_WIPES = 160;
// 사라진 빛줄기가 흐려지는 시간 · 결투 펑 시간 (초)
const MG_FADE = 0.2;
const MG_POP_SECS = 0.6;
const MG_RED = ["rgba(255,70,60,", "rgba(255,200,190,"];
const MG_GREEN = ["rgba(60,230,90,", "rgba(210,255,200,"];
const MG_GOLD = ["rgba(255,196,60,", "rgba(255,246,200,"];
const MG_KINDS = ["bolt", "duel", "snake", "stag", "otter", "soul", "frost", "frost_wipe", "egg", "hatchling", "campfire", "wand", "glow", "puff"];

const mgCanvas = document.getElementById("magic");
const mgCtx = mgCanvas.getContext("2d");
// 지금 효과들 · 흐려지는 빛줄기 [{fx, born}] · 펑 [{x, y, size, born}]
let mgFx = [];
let mgGhosts = [];
let mgPops = [];
let mgFrame = 0;

// 점 [x, y]가 쓸 만함
function mgPt(p) {
  return Array.isArray(p) && p.length === 2 && Number.isFinite(p[0]) && Number.isFinite(p[1]);
}

// 효과 하나가 쓸 만함 (이상한 값은 버림)
function mgOk(f) {
  if (!f || !MG_KINDS.includes(f.kind)) return false;
  if (f.kind === "bolt") return mgPt(f.from) && mgPt(f.to) && f.width > 0 && f.width < 200;
  if (f.kind === "wand") return mgPt(f.from) && mgPt(f.to) && f.size > 0 && f.size < 2000 && Number.isFinite(f.k);
  if (f.kind === "puff") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.k);
  if (f.kind === "glow") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.level);
  if (f.kind === "soul") return mgPt(f.from) && mgPt(f.to) && f.width > 0 && f.width < 500 && Number.isFinite(f.level) && Number.isFinite(f.t);
  if (f.kind === "otter")
    return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.alpha) && Number.isFinite(f.tilt) && Number.isFinite(f.t);
  if (f.kind === "duel") return mgPt(f.red) && mgPt(f.green) && mgPt(f.clash) && f.width > 0 && f.width < 200 && Number.isFinite(f.reach);
  if (f.kind === "frost_wipe") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.r > 0 && f.r < 1000 && f.clear > 0 && f.clear <= 1;
  if (f.kind === "frost") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 5000 && mgPt(f.inward) && f.level > 0;
  if (f.kind === "egg") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.crack);
  if (f.kind === "campfire") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.level);
  if (f.kind === "hatchling") return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.t) && f.len > 0;
  return Number.isFinite(f.x) && Number.isFinite(f.y) && f.size > 0 && f.size < 2000 && Number.isFinite(f.alpha);
}

// 새 목록을 받음: 사라진 빛줄기는 흐려지게, 맞부딪혀 있던 결투는 펑
function setMagic(list) {
  const ok = (Array.isArray(list) ? list : []).filter(mgOk);
  const want = [...ok.filter((f) => f.kind !== "frost_wipe").slice(0, MG_MAX), ...ok.filter((f) => f.kind === "frost_wipe").slice(0, MG_MAX_WIPES)];
  const now = performance.now();
  for (const f of mgFx) {
    const still = want.some((w) => w.kind === f.kind);
    if (still) continue;
    if (f.kind === "bolt") mgGhosts.push({ fx: f, born: now });
    if (f.kind === "duel" && f.reach >= 1) mgPops.push({ x: f.clash[0], y: f.clash[1], size: f.width, born: now });
  }
  mgFx = want;
  wakeMagic();
}

// 움직이는 것이 있음 (서리·닦은 자리는 그대로 서 있음)
function mgMoving() {
  return mgFx.some((f) => f.kind !== "frost" && f.kind !== "frost_wipe") || mgGhosts.length > 0 || mgPops.length > 0;
}

// 빛줄기 한 줄 (a → b, 바깥 번짐 + 밝은 속, 살짝 지그재그)
function mgBeam(a, b, width, col, t, alpha) {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const len = Math.hypot(dx, dy) || 1;
  const nx = -dy / len;
  const ny = dx / len;
  const pts = [];
  const n = Math.max(2, Math.round(len / 24));
  for (let i = 0; i <= n; i++) {
    const k = i / n;
    const j = i === 0 || i === n ? 0 : Math.sin(t * 40 + i * 2.1) * width * 0.35;
    pts.push([a[0] + dx * k + nx * j, a[1] + dy * k + ny * j]);
  }
  const line = (w, style) => {
    mgCtx.strokeStyle = style;
    mgCtx.lineWidth = w;
    mgCtx.beginPath();
    mgCtx.moveTo(pts[0][0], pts[0][1]);
    for (const p of pts.slice(1)) mgCtx.lineTo(p[0], p[1]);
    mgCtx.stroke();
  };
  mgCtx.lineCap = "round";
  line(width * 2.6, `${col[0]}${0.25 * alpha})`);
  line(width, `${col[0]}${0.85 * alpha})`);
  line(width * 0.35, `${col[1]}${alpha})`);
}

// 빛줄기 색 이름 → 바깥·속 색
function mgColor(name) {
  return name === "green" ? MG_GREEN : name === "gold" ? MG_GOLD : MG_RED;
}

// 맞부딪힌 자리의 반짝 공 + 튀는 불꽃
function mgClash(x, y, w, t) {
  const r = w * (2.0 + 0.5 * Math.sin(t * 30));
  const g = mgCtx.createRadialGradient(x, y, 0, x, y, r * 2.2);
  g.addColorStop(0, "rgba(255,255,240,1)");
  g.addColorStop(0.35, "rgba(255,240,160,0.8)");
  g.addColorStop(1, "rgba(255,200,120,0)");
  mgCtx.fillStyle = g;
  mgCtx.beginPath();
  mgCtx.arc(x, y, r * 2.2, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.fillStyle = "rgba(255,250,210,0.9)";
  for (let i = 0; i < 6; i++) {
    const a = t * 7 + i * 1.7;
    const d = r * (1.3 + ((t * 5 + i * 0.37) % 1) * 1.6);
    mgCtx.beginPath();
    mgCtx.arc(x + Math.cos(a) * d, y + Math.sin(a) * d, w * 0.22, 0, Math.PI * 2);
    mgCtx.fill();
  }
}

// ⚔️ 결투: 두 빛줄기가 clash까지 뻗고, 다 뻗었으면 맞부딪힌 반짝
function mgDuel(f, t) {
  const end = (tip) => [tip[0] + (f.clash[0] - tip[0]) * f.reach, tip[1] + (f.clash[1] - tip[1]) * f.reach];
  mgBeam(f.red, end(f.red), f.width, mgColor(f.light || "red"), t, 1);
  mgBeam(f.green, end(f.green), f.width, MG_GREEN, t + 0.5, 1);
  if (f.reach >= 1) mgClash(f.clash[0], f.clash[1], f.width, t);
}

// ⚔️ 펑 (맞부딪힌 자리에서 퍼지는 고리 + 불꽃, k = 0…1)
function mgPop(p, k) {
  mgCtx.strokeStyle = `rgba(255,245,200,${1 - k})`;
  mgCtx.lineWidth = p.size * (1 - k) + 1;
  mgCtx.beginPath();
  mgCtx.arc(p.x, p.y, p.size * (2 + 10 * k), 0, Math.PI * 2);
  mgCtx.stroke();
  for (let i = 0; i < 10; i++) {
    const a = (i / 10) * Math.PI * 2;
    const d = p.size * (2 + 14 * k);
    mgCtx.fillStyle = i % 2 ? `rgba(255,90,70,${1 - k})` : `rgba(90,240,110,${1 - k})`;
    mgCtx.beginPath();
    mgCtx.arc(p.x + Math.cos(a) * d, p.y + Math.sin(a) * d, p.size * 0.5 * (1 - k * 0.5), 0, Math.PI * 2);
    mgCtx.fill();
  }
}

// 🐍 나기니: 머리 (x, 바닥 y)에서 뒤로 (dir 반대) 꿈틀대는 몸 — 굵기가 꼬리로 갈수록 가늘어짐, 등 무늬, 노란 눈, 혀 날름
function mgSnake(f) {
  const L = f.size;
  const thick = L * 0.11;
  const n = 26;
  const pts = [];
  for (let i = 0; i <= n; i++) {
    const k = i / n;
    const wave = Math.sin(k * 9 - f.t * 6) * thick * 0.9 * Math.min(1, k * 3);
    pts.push([f.x - f.dir * k * L, f.y - thick * 0.55 - Math.abs(wave) * 0.35 - wave * 0.25]);
  }
  mgCtx.save();
  mgCtx.globalAlpha = Math.max(0, Math.min(1, f.alpha));
  mgCtx.lineCap = "round";
  for (let i = n; i > 0; i--) {
    const w = thick * (1 - (i / n) * 0.8);
    mgCtx.strokeStyle = "#28331f";
    mgCtx.lineWidth = w + 2.4;
    mgCtx.beginPath();
    mgCtx.moveTo(pts[i][0], pts[i][1]);
    mgCtx.lineTo(pts[i - 1][0], pts[i - 1][1]);
    mgCtx.stroke();
  }
  for (let i = n; i > 0; i--) {
    const w = thick * (1 - (i / n) * 0.8);
    mgCtx.strokeStyle = i % 4 < 2 ? "#5d7a45" : "#4b6638";
    mgCtx.lineWidth = w;
    mgCtx.beginPath();
    mgCtx.moveTo(pts[i][0], pts[i][1]);
    mgCtx.lineTo(pts[i - 1][0], pts[i - 1][1]);
    mgCtx.stroke();
  }
  // 등의 짙은 마름모 무늬
  mgCtx.fillStyle = "#2f4224";
  for (let i = 3; i < n - 2; i += 3) {
    const w = thick * (1 - (i / n) * 0.8) * 0.3;
    mgCtx.beginPath();
    mgCtx.moveTo(pts[i][0] - w, pts[i][1]);
    mgCtx.lineTo(pts[i][0], pts[i][1] - w);
    mgCtx.lineTo(pts[i][0] + w, pts[i][1]);
    mgCtx.lineTo(pts[i][0], pts[i][1] + w);
    mgCtx.fill();
  }
  // 머리 (납작한 타원) + 눈 + 혀
  const [hx, hy] = pts[0];
  mgCtx.fillStyle = "#5d7a45";
  mgCtx.strokeStyle = "#28331f";
  mgCtx.lineWidth = 1.6;
  mgCtx.beginPath();
  mgCtx.ellipse(hx + f.dir * thick * 0.5, hy, thick * 1.05, thick * 0.7, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.fillStyle = "#f2d23c";
  mgCtx.beginPath();
  mgCtx.arc(hx + f.dir * thick * 0.75, hy - thick * 0.25, thick * 0.2, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.fillStyle = "#1b1b12";
  mgCtx.fillRect(hx + f.dir * thick * 0.75 - 0.6, hy - thick * 0.4, 1.2, thick * 0.3);
  if (Math.sin(f.t * 5) > 0.2) {
    const tx = hx + f.dir * thick * 1.5;
    mgCtx.strokeStyle = "#d8344a";
    mgCtx.lineWidth = 1.4;
    mgCtx.beginPath();
    mgCtx.moveTo(tx, hy + thick * 0.1);
    mgCtx.lineTo(tx + f.dir * thick * 0.7, hy + thick * 0.1);
    mgCtx.lineTo(tx + f.dir * thick * 1.0, hy - thick * 0.15);
    mgCtx.moveTo(tx + f.dir * thick * 0.7, hy + thick * 0.1);
    mgCtx.lineTo(tx + f.dir * thick * 1.0, hy + thick * 0.35);
    mgCtx.stroke();
  }
  mgCtx.restore();
}

// 🦌 은빛 사슴 (가운데 x, y, 크기 size — 오른쪽을 보고 그린 뒤 dir로 뒤집음): 몸·목·머리·큰 뿔·달리는 네 다리, 은빛 번짐
function mgStag(f, t) {
  const s = f.size / 100;
  mgCtx.save();
  mgCtx.globalAlpha = Math.max(0, Math.min(1, f.alpha));
  mgCtx.translate(f.x, f.y);
  mgCtx.scale(f.dir < 0 ? -s : s, s);
  const glow = mgCtx.createRadialGradient(0, 0, 5, 0, 0, 70);
  glow.addColorStop(0, "rgba(210,235,255,0.55)");
  glow.addColorStop(1, "rgba(210,235,255,0)");
  mgCtx.fillStyle = glow;
  mgCtx.beginPath();
  mgCtx.arc(0, 0, 70, 0, Math.PI * 2);
  mgCtx.fill();
  const silver = "rgba(236,246,255,0.92)";
  const edge = "rgba(150,200,255,0.95)";
  mgCtx.fillStyle = silver;
  mgCtx.strokeStyle = edge;
  mgCtx.lineWidth = 2;
  mgCtx.lineCap = "round";
  mgCtx.lineJoin = "round";
  // 달리는 다리 (앞·뒤 짝이 엇갈려 흔들림)
  const gallop = Math.sin(t * 14);
  const leg = (x, y, a, len) => {
    mgCtx.lineWidth = 5;
    mgCtx.strokeStyle = silver;
    mgCtx.beginPath();
    mgCtx.moveTo(x, y);
    const kx = x + Math.sin(a) * len * 0.55;
    const ky = y + Math.cos(a) * len * 0.55;
    mgCtx.lineTo(kx, ky);
    mgCtx.lineTo(kx + Math.sin(a * 0.4) * len * 0.5, ky + len * 0.5);
    mgCtx.stroke();
  };
  leg(-22, 8, -0.6 + gallop * 0.6, 34);
  leg(-14, 10, -0.3 - gallop * 0.6, 34);
  leg(16, 8, 0.5 - gallop * 0.7, 34);
  leg(24, 8, 0.2 + gallop * 0.7, 34);
  // 몸 · 목 · 머리 · 귀 · 꼬리
  mgCtx.fillStyle = silver;
  mgCtx.strokeStyle = edge;
  mgCtx.lineWidth = 2;
  mgCtx.beginPath();
  mgCtx.ellipse(0, 0, 34, 15, -0.05, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.moveTo(18, -8);
  mgCtx.quadraticCurveTo(26, -18, 30, -26);
  mgCtx.lineTo(40, -22);
  mgCtx.quadraticCurveTo(36, -10, 32, 4);
  mgCtx.closePath();
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.ellipse(40, -28, 12, 7, 0.45, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.moveTo(-32, -6);
  mgCtx.lineTo(-40, -12);
  mgCtx.lineTo(-34, -1);
  mgCtx.fill();
  mgCtx.beginPath();
  mgCtx.ellipse(32, -35, 5, 2.5, -0.6, 0, Math.PI * 2);
  mgCtx.fill();
  // 큰 뿔 (가지가 갈라짐)
  mgCtx.strokeStyle = silver;
  mgCtx.lineWidth = 3;
  const antler = (x0, y0, lean) => {
    mgCtx.beginPath();
    mgCtx.moveTo(x0, y0);
    mgCtx.quadraticCurveTo(x0 - 6 + lean, y0 - 16, x0 - 16 + lean, y0 - 30);
    mgCtx.moveTo(x0 - 3 + lean * 0.3, y0 - 9);
    mgCtx.lineTo(x0 + 6 + lean, y0 - 18);
    mgCtx.moveTo(x0 - 9 + lean * 0.6, y0 - 19);
    mgCtx.lineTo(x0 - 2 + lean, y0 - 30);
    mgCtx.moveTo(x0 - 13 + lean * 0.8, y0 - 26);
    mgCtx.lineTo(x0 - 22 + lean, y0 - 32);
    mgCtx.stroke();
  };
  antler(36, -34, 0);
  antler(40, -33, 6);
  // 반짝이 자취 (뒤로)
  mgCtx.fillStyle = "rgba(220,240,255,0.8)";
  for (let i = 0; i < 6; i++) {
    const k = ((t * 2 + i / 6) % 1) * 1;
    mgCtx.beginPath();
    mgCtx.arc(-40 - k * 50, -10 + Math.sin(i * 2.3) * 14, 2.5 * (1 - k), 0, Math.PI * 2);
    mgCtx.fill();
  }
  mgCtx.restore();
}

// 🦦 은빛 수달 (가운데 x, y, 크기 size — 오른쪽을 보고 그린 뒤 dir로 뒤집고 tilt만큼 기울임): 은빛 번짐, 젓는 네 발, 긴 몸, 둥근 머리·작은 귀·
// 주둥이·수염, 굵다가 가늘어지며 살랑대는 꼬리, 뒤로 떠오르는 물방울
function mgOtter(f, t) {
  const s = f.size / 100;
  mgCtx.save();
  mgCtx.globalAlpha = Math.max(0, Math.min(1, f.alpha));
  mgCtx.translate(f.x, f.y);
  mgCtx.scale(f.dir < 0 ? -s : s, s);
  mgCtx.rotate(f.tilt);
  const glow = mgCtx.createRadialGradient(0, 0, 4, 0, 0, 62);
  glow.addColorStop(0, "rgba(205,232,255,0.5)");
  glow.addColorStop(1, "rgba(205,232,255,0)");
  mgCtx.fillStyle = glow;
  mgCtx.beginPath();
  mgCtx.arc(0, 0, 62, 0, Math.PI * 2);
  mgCtx.fill();
  const silver = "rgba(232,244,255,0.92)";
  const edge = "rgba(140,195,255,0.95)";
  mgCtx.lineCap = "round";
  mgCtx.lineJoin = "round";
  // 젓는 발 (앞·뒤가 엇갈려 앞뒤로)
  const paddle = Math.sin(f.t * 10);
  mgCtx.fillStyle = silver;
  for (const [px, k] of [[16, 1], [22, -1], [-14, -1], [-8, 1]]) {
    mgCtx.beginPath();
    mgCtx.ellipse(px + k * paddle * 4, 10 + Math.abs(paddle) * 2, 5, 3, 0.5 * k * paddle, 0, Math.PI * 2);
    mgCtx.fill();
  }
  // 꼬리 (몸 뒤에서 굵게 나와 가늘어짐, 살랑)
  const sway = Math.sin(f.t * 6) * 8;
  mgCtx.beginPath();
  mgCtx.moveTo(-24, -6);
  mgCtx.quadraticCurveTo(-44, -4 + sway * 0.5, -60, 2 + sway);
  mgCtx.quadraticCurveTo(-44, 6 + sway * 0.5, -24, 6);
  mgCtx.closePath();
  mgCtx.fill();
  // 긴 몸 · 머리 · 귀 · 주둥이
  mgCtx.strokeStyle = edge;
  mgCtx.lineWidth = 2;
  mgCtx.beginPath();
  mgCtx.ellipse(0, 0, 30, 11, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.ellipse(31, -5, 11, 9, -0.15, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.ellipse(27, -14, 3, 2.5, 0, 0, Math.PI * 2);
  mgCtx.ellipse(34, -14, 3, 2.5, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.beginPath();
  mgCtx.ellipse(40, -2, 5, 4, 0, 0, Math.PI * 2);
  mgCtx.fill();
  // 코 · 눈 (조금 짙은 은빛) · 수염
  mgCtx.fillStyle = edge;
  mgCtx.beginPath();
  mgCtx.arc(44, -3, 1.6, 0, Math.PI * 2);
  mgCtx.arc(34, -8, 1.3, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.strokeStyle = "rgba(225,240,255,0.7)";
  mgCtx.lineWidth = 0.8;
  mgCtx.beginPath();
  for (const dy of [-2, 1]) {
    mgCtx.moveTo(41, dy);
    mgCtx.lineTo(52, dy * 2.2 - 1);
  }
  mgCtx.stroke();
  // 물방울 자취 (뒤로 떠오르며 작아짐)
  mgCtx.strokeStyle = "rgba(215,236,255,0.75)";
  mgCtx.lineWidth = 1.2;
  for (let i = 0; i < 5; i++) {
    const k = (f.t * 1.5 + i / 5) % 1;
    mgCtx.beginPath();
    mgCtx.arc(-30 - k * 45, -8 + Math.sin(i * 2.1 + f.t * 3) * 10 - k * 8, 3 * (1 - k) + 0.5, 0, Math.PI * 2);
    mgCtx.stroke();
  }
  mgCtx.restore();
}

// 👻 영혼 줄기: 펫 가슴 (from)의 희미한 빛 + 디멘터 (to)로 일렁이며 올라가는 옅은 은빛 가닥 셋 + 빨려 올라가는 작은 빛 (level 0이면 안 그림)
function mgSoul(f, t) {
  const level = Math.max(0, Math.min(1, f.level));
  if (level <= 0) return;
  const [x0, y0] = f.from;
  const [x1, y1] = f.to;
  const dx = x1 - x0;
  const dy = y1 - y0;
  const len = Math.hypot(dx, dy) || 1;
  // 줄기와 수직인 방향
  const nx = -dy / len;
  const ny = dx / len;
  const w = f.width;
  const g = mgCtx.createRadialGradient(x0, y0, 0, x0, y0, w * 1.4);
  g.addColorStop(0, `rgba(225,238,255,${0.45 * level})`);
  g.addColorStop(1, "rgba(200,220,255,0)");
  mgCtx.fillStyle = g;
  mgCtx.beginPath();
  mgCtx.arc(x0, y0, w * 1.4, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.lineCap = "round";
  for (let i = 0; i < 3; i++) {
    const ph = f.t * (2.2 + i * 0.7) + i * 2.1;
    const a = w * (0.35 + 0.15 * i);
    const o1 = Math.sin(ph) * a;
    const o2 = Math.sin(ph + 2.4) * a;
    mgCtx.strokeStyle = `rgba(${200 + i * 15},${225 + i * 8},255,${(0.32 - i * 0.07) * level})`;
    mgCtx.lineWidth = Math.max(1, w * (0.42 - i * 0.12));
    mgCtx.beginPath();
    mgCtx.moveTo(x0 + nx * o2 * 0.2, y0 + ny * o2 * 0.2);
    mgCtx.bezierCurveTo(x0 + dx * 0.33 + nx * o1, y0 + dy * 0.33 + ny * o1, x0 + dx * 0.66 + nx * o2, y0 + dy * 0.66 + ny * o2, x1, y1);
    mgCtx.stroke();
  }
  mgCtx.fillStyle = `rgba(235,244,255,${0.7 * level})`;
  for (let i = 0; i < 4; i++) {
    const k = (t * 0.9 + i / 4) % 1;
    const wob = Math.sin(t * 4 + i * 1.9) * w * 0.4 * (1 - k);
    mgCtx.beginPath();
    mgCtx.arc(x0 + dx * k + nx * wob, y0 + dy * k + ny * wob, Math.max(0.8, w * 0.09 * (1 - k * 0.6)), 0, Math.PI * 2);
    mgCtx.fill();
  }
}

// 🥶 서리: 구석에서 안쪽으로 번지는 반투명 흰빛 + 얼음 가지 (구석마다 같은 모양 — 자리로 씨앗을 정함)
function mgFrost(f) {
  // 🌫️ 김 서린 유리처럼: 구석에서 안쪽으로 부드럽게 번지는 뿌연 흰빛만 (얼음 가지 없음 — 2026-11-23)
  const a = Math.min(1, f.level);
  const r = f.size * (0.45 + 0.55 * a);
  const g = mgCtx.createRadialGradient(f.x, f.y, 0, f.x, f.y, r);
  g.addColorStop(0, `rgba(236,242,248,${0.5 * a})`);
  g.addColorStop(0.45, `rgba(230,238,246,${0.34 * a})`);
  g.addColorStop(0.8, `rgba(226,235,244,${0.12 * a})`);
  g.addColorStop(1, "rgba(226,235,244,0)");
  mgCtx.fillStyle = g;
  mgCtx.beginPath();
  mgCtx.arc(f.x, f.y, r, 0, Math.PI * 2);
  mgCtx.fill();
  // 입김이 번진 듯 가장자리를 살짝 울퉁불퉁하게: 안쪽으로 작은 뿌연 덩이 몇 개 (자리는 구석마다 같게)
  let seed = (Math.round(f.x) * 7 + Math.round(f.y) * 13) % 97;
  const rnd = () => {
    seed = (seed * 37 + 11) % 101;
    return seed / 101;
  };
  const [ix, iy] = f.inward;
  for (let i = 0; i < 5; i++) {
    const ang = ((i + 0.5) / 5) * (Math.PI / 2);
    const d = r * (0.45 + 0.25 * rnd());
    const cx = f.x + Math.cos(ang) * ix * d;
    const cy = f.y + Math.sin(ang) * iy * d;
    const pr = r * (0.28 + 0.12 * rnd());
    const pg = mgCtx.createRadialGradient(cx, cy, 0, cx, cy, pr);
    pg.addColorStop(0, `rgba(232,240,247,${0.16 * a})`);
    pg.addColorStop(1, "rgba(232,240,247,0)");
    mgCtx.fillStyle = pg;
    mgCtx.beginPath();
    mgCtx.arc(cx, cy, pr, 0, Math.PI * 2);
    mgCtx.fill();
  }
}

// 🖐️ 서리 닦은 자리: 서리를 그린 바로 뒤에 둘레 r을 부드러운 가장자리로 지움 (김 서린 유리에 손가락으로 그은 듯) — 그다음 효과는 보통대로 그리게 되돌림
function mgFrostWipe(f) {
  mgCtx.globalCompositeOperation = "destination-out";
  const g = mgCtx.createRadialGradient(f.x, f.y, 0, f.x, f.y, f.r);
  g.addColorStop(0, `rgba(0,0,0,${f.clear})`);
  g.addColorStop(0.6, `rgba(0,0,0,${0.85 * f.clear})`);
  g.addColorStop(1, "rgba(0,0,0,0)");
  mgCtx.fillStyle = g;
  mgCtx.beginPath();
  mgCtx.arc(f.x, f.y, f.r, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.globalCompositeOperation = "source-over";
}

// 효과 하나 그리기 (t = 초)
// 🥚 용알 (바닥 가운데 x, y · 높이 size): 크림색 알 + 초록 점, 금 (crack)이 갈수록 지그재그 금이 늘고 달달 떨림
function mgEgg(f, t) {
  const h = f.size;
  const w = h * 0.76;
  const crack = Math.max(0, Math.min(1, f.crack));
  mgCtx.save();
  mgCtx.translate(f.x, f.y);
  mgCtx.rotate(Math.sin(t * 22) * 0.12 * crack);
  mgCtx.fillStyle = "#fff4dc";
  mgCtx.strokeStyle = "#5a4630";
  mgCtx.lineWidth = Math.max(1.2, h * 0.06);
  mgCtx.beginPath();
  mgCtx.ellipse(0, -h * 0.5, w * 0.5, h * 0.5, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.fillStyle = "#7fb36a";
  for (const [dx, dy, r] of [[-0.18, -0.62, 0.09], [0.16, -0.38, 0.11], [0.05, -0.8, 0.06], [-0.12, -0.25, 0.06]]) {
    mgCtx.beginPath();
    mgCtx.arc(dx * h, dy * h, r * h, 0, Math.PI * 2);
    mgCtx.fill();
  }
  if (crack > 0) {
    mgCtx.strokeStyle = "#5a4630";
    mgCtx.lineWidth = Math.max(1, h * 0.04);
    mgCtx.beginPath();
    const n = 2 + Math.round(crack * 4);
    for (let i = 0; i <= n; i++) {
      const x = -w * 0.48 + (w * 0.96 * i) / n;
      const y = -h * 0.55 + (i % 2 ? -1 : 1) * h * 0.08;
      if (i === 0) mgCtx.moveTo(x, y);
      else mgCtx.lineTo(x, y);
    }
    mgCtx.stroke();
  }
  mgCtx.restore();
}

// 🐉 아기 용 (알 자리 x, y · 크기 size · 깨어난 뒤 t초 / len초): 처음엔 껍데기 두 쪽이 튀고, 둘레를 8자로 파닥파닥 날다가
// 끝 1.2초에 작은 불을 퐁 뿜고 마지막 0.5초에 흐려지며 사라짐
function mgHatchling(f) {
  const s = f.size;
  const t = f.t;
  const dir = f.dir < 0 ? -1 : 1;
  const left = f.len - t;
  mgCtx.save();
  mgCtx.globalAlpha = Math.max(0, Math.min(1, left / 0.5));
  if (t < 0.6) {
    // 갈라진 껍데기 두 쪽이 양옆으로 튐
    const k = t / 0.6;
    mgCtx.fillStyle = "#fff4dc";
    mgCtx.strokeStyle = "#5a4630";
    mgCtx.lineWidth = 1.2;
    for (const side of [-1, 1]) {
      mgCtx.beginPath();
      mgCtx.arc(f.x + side * s * 0.6 * k, f.y - s * 0.25 - Math.sin(k * Math.PI) * s * 0.6, s * 0.22, 0, Math.PI);
      mgCtx.closePath();
      mgCtx.fill();
      mgCtx.stroke();
    }
  }
  // 날아다니는 자리: 알 자리 위에서 8자 (반지름이 처음 0.8초 동안 커짐)
  const grow = Math.min(1, t / 0.8);
  const a = t * 2.2;
  const px = f.x + dir * Math.sin(a) * s * 1.8 * grow;
  const py = f.y - s * (0.4 + 1.4 * grow) - Math.sin(a * 2) * s * 0.35 * grow;
  const face = Math.cos(a) * dir >= 0 ? 1 : -1;
  const flap = Math.sin(t * 20);
  mgCtx.translate(px, py);
  mgCtx.scale(face, 1);
  mgCtx.lineJoin = "round";
  mgCtx.strokeStyle = "#2d4a26";
  mgCtx.lineWidth = Math.max(1, s * 0.06);
  // 날개 (뒤) — 파닥
  mgCtx.fillStyle = "#b7dd96";
  mgCtx.beginPath();
  mgCtx.moveTo(-s * 0.05, -s * 0.12);
  mgCtx.lineTo(-s * 0.35, -s * (0.25 + 0.35 * flap));
  mgCtx.lineTo(-s * 0.45, -s * 0.05);
  mgCtx.closePath();
  mgCtx.fill();
  mgCtx.stroke();
  // 꼬리 · 몸 · 머리
  mgCtx.fillStyle = "#6fae5f";
  mgCtx.beginPath();
  mgCtx.moveTo(-s * 0.25, 0);
  mgCtx.quadraticCurveTo(-s * 0.55, s * 0.05, -s * 0.6, -s * 0.15);
  mgCtx.lineTo(-s * 0.48, -s * 0.02);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.ellipse(0, 0, s * 0.3, s * 0.2, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.beginPath();
  mgCtx.arc(s * 0.3, -s * 0.18, s * 0.17, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.stroke();
  mgCtx.fillStyle = "#e8c27a";
  mgCtx.beginPath();
  mgCtx.ellipse(s * 0.02, s * 0.06, s * 0.2, s * 0.1, 0, 0, Math.PI * 2);
  mgCtx.fill();
  mgCtx.fillStyle = "#1b1b12";
  mgCtx.beginPath();
  mgCtx.arc(s * 0.36, -s * 0.22, s * 0.04, 0, Math.PI * 2);
  mgCtx.fill();
  // 앞 날개 — 파닥
  mgCtx.fillStyle = "#cdeab0";
  mgCtx.beginPath();
  mgCtx.moveTo(s * 0.02, -s * 0.12);
  mgCtx.lineTo(-s * 0.2, -s * (0.3 + 0.4 * flap));
  mgCtx.lineTo(-s * 0.3, -s * 0.02);
  mgCtx.closePath();
  mgCtx.fill();
  mgCtx.stroke();
  // 끝에 작은 불 퐁
  if (left < 1.2 && left > 0.4) {
    const k = (1.2 - left) / 0.8;
    const fx = s * 0.48;
    const fy = -s * 0.16;
    const g = mgCtx.createRadialGradient(fx + s * 0.2 * k, fy, 0, fx + s * 0.2 * k, fy, s * 0.35);
    g.addColorStop(0, "rgba(255,240,150,1)");
    g.addColorStop(0.5, "rgba(255,150,50,0.9)");
    g.addColorStop(1, "rgba(255,80,30,0)");
    mgCtx.fillStyle = g;
    mgCtx.beginPath();
    mgCtx.ellipse(fx + s * 0.2 * k, fy, s * 0.35 * (0.5 + k), s * 0.16 * (0.5 + k), 0, 0, Math.PI * 2);
    mgCtx.fill();
  }
  mgCtx.restore();
}

// 🔥 모닥불 (바닥 가운데 x, y · 장작 폭 size · 타오른 몫 level): 은은한 불빛 → 둥근 돌 → 엇갈린 장작 → 일렁이는 불꽃 세 겹
// (빨강·주황·노랑 — 시간 t로 너울너울, level만큼 큼) → 튀어 오르는 불티 (시간으로 정해 — 따로 기억하지 않음)
const MG_SPARKS = 7;
function mgCampfire(f, t) {
  const s = f.size;
  const lv = Math.max(0, Math.min(1, f.level));
  const x = f.x;
  const y = f.y;
  mgCtx.save();
  mgCtx.lineJoin = "round";
  if (lv > 0) {
    const glow = mgCtx.createRadialGradient(x, y - s * 0.3, 0, x, y - s * 0.3, s * 1.6);
    glow.addColorStop(0, `rgba(255,190,90,${0.35 * lv})`);
    glow.addColorStop(1, "rgba(255,150,60,0)");
    mgCtx.fillStyle = glow;
    mgCtx.beginPath();
    mgCtx.ellipse(x, y - s * 0.3, s * 1.6, s * 1.2, 0, 0, Math.PI * 2);
    mgCtx.fill();
  }
  // 둥근 돌 테두리 (앞쪽 반원)
  mgCtx.strokeStyle = "#4a3f3a";
  mgCtx.lineWidth = Math.max(1, s * 0.04);
  mgCtx.fillStyle = "#a59c96";
  for (let i = 0; i < 5; i++) {
    const a = Math.PI * (0.1 + (0.8 * i) / 4);
    mgCtx.beginPath();
    mgCtx.ellipse(x - Math.cos(a) * s * 0.55, y - s * 0.06 + Math.sin(a) * s * 0.04, s * 0.13, s * 0.09, 0, 0, Math.PI * 2);
    mgCtx.fill();
    mgCtx.stroke();
  }
  // 엇갈린 장작 둘
  mgCtx.fillStyle = "#9a6a44";
  mgCtx.strokeStyle = "#4f3423";
  for (const tilt of [-0.28, 0.28]) {
    mgCtx.save();
    mgCtx.translate(x, y - s * 0.12);
    mgCtx.rotate(tilt);
    mgCtx.beginPath();
    mgCtx.roundRect(-s * 0.5, -s * 0.07, s, s * 0.14, s * 0.07);
    mgCtx.fill();
    mgCtx.stroke();
    mgCtx.restore();
  }
  if (lv > 0) {
    // 불꽃 세 겹: 바깥 빨강 → 주황 → 노란 속 (시간으로 너울너울)
    const layers = [
      ["#f0583a", 1.0, 0],
      ["#ff9a3c", 0.72, 1.7],
      ["#ffe27a", 0.42, 3.1],
    ];
    for (const [col, k, ph] of layers) {
      const h = s * (0.95 + 0.12 * Math.sin(t * 9 + ph)) * k * lv;
      const w = s * 0.42 * k * (0.6 + 0.4 * lv);
      const lean = s * 0.08 * Math.sin(t * 6.3 + ph);
      const by = y - s * 0.16;
      mgCtx.fillStyle = col;
      mgCtx.beginPath();
      mgCtx.moveTo(x - w, by);
      mgCtx.quadraticCurveTo(x - w * 0.9, by - h * 0.55, x + lean, by - h);
      mgCtx.quadraticCurveTo(x + w * 0.9, by - h * 0.55, x + w, by);
      mgCtx.quadraticCurveTo(x, by + s * 0.08, x - w, by);
      mgCtx.fill();
    }
    // 불티: 위로 튀며 흔들리고 사라짐 (하나마다 시간이 엇갈림)
    for (let i = 0; i < MG_SPARKS; i++) {
      const life = 1.1 + (i % 3) * 0.25;
      const k = (t / life + i / MG_SPARKS) % 1;
      const sx = x + Math.sin(i * 2.4 + k * 5) * s * 0.35;
      const sy = y - s * 0.3 - k * s * 1.7 * lv;
      mgCtx.fillStyle = `rgba(255,${200 - i * 10},90,${(1 - k) * lv})`;
      mgCtx.beginPath();
      mgCtx.arc(sx, sy, Math.max(0.8, s * 0.035 * (1 - k * 0.5)), 0, Math.PI * 2);
      mgCtx.fill();
    }
  }
  mgCtx.restore();
}

// 🪄 날아간 지팡이: 손에서 땅까지 포물선 (위로 size의 2.5배), 날아가는 동안 빙글빙글 — 땅에 닿으면 비스듬히 누움
function mgWand(f) {
  const k = Math.max(0, Math.min(1, f.k));
  if (k <= 0) return;
  const x = f.from[0] + (f.to[0] - f.from[0]) * k;
  const y = f.from[1] + (f.to[1] - f.from[1]) * k - Math.sin(Math.PI * k) * f.size * 2.5;
  const a = k >= 1 ? 0.12 : k * Math.PI * 5;
  const half = f.size / 2;
  mgCtx.save();
  mgCtx.translate(x, y);
  mgCtx.rotate(a);
  mgCtx.lineCap = "round";
  mgCtx.strokeStyle = "#5a3b22";
  mgCtx.lineWidth = Math.max(2, f.size * 0.13);
  mgCtx.beginPath();
  mgCtx.moveTo(-half, 0);
  mgCtx.lineTo(half, 0);
  mgCtx.stroke();
  // 손잡이 (굵고 진함)
  mgCtx.strokeStyle = "#3a2414";
  mgCtx.lineWidth = Math.max(3, f.size * 0.2);
  mgCtx.beginPath();
  mgCtx.moveTo(-half, 0);
  mgCtx.lineTo(-half * 0.45, 0);
  mgCtx.stroke();
  mgCtx.restore();
}

// 💡 루모스 불빛: 크게 번지는 따뜻한 빛 + 작고 하얀 속 (level만큼, 살짝 일렁임)
function mgGlow(f, t) {
  const level = Math.max(0, Math.min(1, f.level));
  if (level <= 0) return;
  const r = f.size * (0.92 + 0.08 * Math.sin(t * 9));
  const g = mgCtx.createRadialGradient(f.x, f.y, 0, f.x, f.y, r);
  g.addColorStop(0, `rgba(255,255,255,${0.95 * level})`);
  g.addColorStop(0.12, `rgba(255,252,225,${0.85 * level})`);
  g.addColorStop(0.35, `rgba(255,240,180,${0.35 * level})`);
  g.addColorStop(1, "rgba(255,230,160,0)");
  mgCtx.fillStyle = g;
  mgCtx.beginPath();
  mgCtx.arc(f.x, f.y, r, 0, Math.PI * 2);
  mgCtx.fill();
  // 반짝 빛살 넷
  mgCtx.strokeStyle = `rgba(255,255,240,${0.6 * level})`;
  mgCtx.lineWidth = Math.max(1, f.size * 0.025);
  mgCtx.lineCap = "round";
  for (let i = 0; i < 4; i++) {
    const a = t * 0.8 + (i * Math.PI) / 2;
    const l = r * (0.45 + 0.1 * Math.sin(t * 7 + i));
    mgCtx.beginPath();
    mgCtx.moveTo(f.x + Math.cos(a) * r * 0.08, f.y + Math.sin(a) * r * 0.08);
    mgCtx.lineTo(f.x + Math.cos(a) * l, f.y + Math.sin(a) * l);
    mgCtx.stroke();
  }
}

// 🌑 녹스 연기: 지팡이 끝에서 검은 연기 덩이 다섯이 저마다 조금씩 비스듬히 피어올라 커지며 흐려짐 (k 0…1, 살랑 흔들림)
function mgPuff(f, t) {
  const k = Math.max(0, Math.min(1, f.k));
  const fade = 1 - k;
  if (fade <= 0) return;
  for (let i = 0; i < 5; i++) {
    const lag = i * 0.08;
    const p = Math.max(0, Math.min(1, (k - lag) / (1 - lag)));
    if (p <= 0) continue;
    const side = (i % 2 ? 1 : -1) * (0.15 + 0.12 * i);
    const x = f.x + f.size * (side * p + 0.08 * Math.sin(t * 3 + i * 1.7));
    const y = f.y - f.size * (0.2 + 1.3 * p) * (0.8 + 0.1 * i);
    const r = f.size * (0.18 + 0.35 * p) * (1 - 0.08 * i);
    const a = 0.55 * fade * Math.min(1, p * 4);
    const g = mgCtx.createRadialGradient(x, y, 0, x, y, r);
    g.addColorStop(0, `rgba(40,36,48,${a})`);
    g.addColorStop(0.6, `rgba(60,56,70,${a * 0.6})`);
    g.addColorStop(1, "rgba(70,66,80,0)");
    mgCtx.fillStyle = g;
    mgCtx.beginPath();
    mgCtx.arc(x, y, r, 0, Math.PI * 2);
    mgCtx.fill();
  }
}

function mgDraw(f, t) {
  if (f.kind === "bolt") mgBeam(f.from, f.to, f.width, mgColor(f.color), t, 1);
  else if (f.kind === "duel") mgDuel(f, t);
  else if (f.kind === "snake") mgSnake(f);
  else if (f.kind === "stag") mgStag(f, t);
  else if (f.kind === "otter") mgOtter(f, t);
  else if (f.kind === "soul") mgSoul(f, t);
  else if (f.kind === "frost") mgFrost(f);
  else if (f.kind === "frost_wipe") mgFrostWipe(f);
  else if (f.kind === "egg") mgEgg(f, t);
  else if (f.kind === "hatchling") mgHatchling(f);
  else if (f.kind === "campfire") mgCampfire(f, t);
  else if (f.kind === "wand") mgWand(f);
  else if (f.kind === "glow") mgGlow(f, t);
  else if (f.kind === "puff") mgPuff(f, t);
}

// 한 프레임: 서리 → 서리 닦은 자리 → 뱀 → 영혼 줄기 → 빛줄기·사슴·수달 순으로, 흐려지는 빛줄기·펑까지. 움직이는 것이 남았으면 다음 프레임
function drawMagic() {
  mgFrame = 0;
  const now = performance.now();
  const t = now / 1000;
  mgGhosts = mgGhosts.filter((g) => (now - g.born) / 1000 < MG_FADE);
  mgPops = mgPops.filter((p) => (now - p.born) / 1000 < MG_POP_SECS);
  mgCtx.clearRect(0, 0, mgCanvas.width, mgCanvas.height);
  for (const kind of ["frost", "frost_wipe", "campfire", "snake", "egg", "hatchling", "wand", "soul", "bolt", "duel", "stag", "otter", "glow", "puff"]) for (const f of mgFx) if (f.kind === kind) mgDraw(f, t);
  for (const g of mgGhosts) mgBeam(g.fx.from, g.fx.to, g.fx.width, mgColor(g.fx.color), t, 1 - (now - g.born) / 1000 / MG_FADE);
  for (const p of mgPops) mgPop(p, (now - p.born) / 1000 / MG_POP_SECS);
  if (mgMoving()) mgFrame = requestAnimationFrame(drawMagic);
}

// 그리기를 깨움: 움직이는 것이 있으면 프레임을 돌리고 (이미 돌면 그대로), 아니면 한 번만 그림 (서리) 또는 지움
function wakeMagic() {
  if (mgFrame) return;
  if (mgMoving()) mgFrame = requestAnimationFrame(drawMagic);
  else drawMagic();
}

// 캔버스를 창 크기에 맞춤
function fitMagicCanvas() {
  mgCanvas.width = window.innerWidth;
  mgCanvas.height = window.innerHeight;
  if (!mgFrame) drawMagic();
}

window.addEventListener("resize", fitMagicCanvas);
fitMagicCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 것만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  (window.spOn || ((t, e, h) => t.listen(e, h)))(window.__TAURI__.webviewWindow.getCurrentWebviewWindow(), "trail-magic", (e) => setMagic(e.payload));
} else if (new URLSearchParams(location.search).get("magic")) {
  // 앱 없이 브라우저에서 보기 (개발용): 결투 · 달리는 사슴 · 헤엄치는 수달 · 영혼 줄기 · 기는 뱀 · 구석의 서리
  document.documentElement.style.background = "#1d2030";
  const t0 = performance.now();
  const demo = () => {
    const t = (performance.now() - t0) / 1000;
    const W = window.innerWidth;
    const H = window.innerHeight;
    const share = 0.5 + 0.2 * Math.sin(t * 2.3);
    const red = [W * 0.25, H * 0.35];
    const green = [W * 0.75, H * 0.35];
    setMagic([
      { kind: "frost", x: 0, y: 0, size: 360, inward: [1, 1], level: 1 },
      { kind: "frost", x: W, y: H, size: 360, inward: [-1, -1], level: 0.6 },
      ...[0, 1, 2, 3, 4, 5].map((i) => ({ kind: "frost_wipe", x: 40 + 22 * i, y: 70 + 18 * Math.sin(i), r: 26, clear: 1 })),
      { kind: "duel", red, green, clash: [red[0] + (green[0] - red[0]) * share, red[1]], reach: Math.min(1, t), width: 9, light: new URLSearchParams(location.search).get("light") || "red" },
      { kind: "stag", x: W * 0.2 + ((t * 220) % (W * 0.6)), y: H * 0.62, size: 130, dir: 1, alpha: 1 },
      { kind: "otter", x: W * 0.8 - ((t * 200) % (W * 0.6)), y: H * 0.5 - 45 * Math.sin(t * 10), size: 130, dir: -1, alpha: 1, tilt: -0.45 * Math.cos(t * 10), t },
      { kind: "soul", from: [W * 0.3, H * 0.88], to: [W * 0.3, H * 0.7], width: 38, level: 1, t },
      { kind: "snake", x: W * 0.55 + Math.sin(t * 0.4) * 150, y: H * 0.92, size: 190, dir: Math.cos(t * 0.4) >= 0 ? 1 : -1, t, alpha: 1 },
      { kind: "bolt", from: [W * 0.62, H * 0.75], to: [W * 0.82, H * 0.8], color: "green", width: 7 },
      { kind: "campfire", x: W * 0.12, y: H * 0.95, size: 70, level: Math.min(1, t / 2) },
      { kind: "glow", x: W * 0.4, y: H * 0.6, size: 110, level: 1 },
      { kind: "puff", x: W * 0.5, y: H * 0.6, size: 50, k: (t % 1.5) / 1.5 },
      { kind: "wand", from: [W * 0.85, H * 0.55], to: [W * 0.92, H * 0.68], k: (t % 2) / 2, size: 40 },
    ]);
    requestAnimationFrame(demo);
  };
  demo();
}
