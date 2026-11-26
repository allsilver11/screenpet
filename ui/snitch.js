// 🟡 해리포터의 골든 스니치 그리기. 🌈 무지개 구름·🖍️ 낙서·🕷️ 거미집·🦇 박쥐 신호와 같은 모니터 덮개 창 (trail.html)의 맨 위 캔버스에 그림.
// 앱(src-tauri/src/trail.rs)이 보내는 것 (논리 px, 이 창 왼쪽 위가 0,0):
//   trail-snitch  지금 나는 스니치들 [{x, y, size, caught}] — x·y는 공 가운데, size는 공 지름, caught면 해리 손에 잡힘.
//                 바뀔 때만 (최대 30번/초) 오고, 다 사라지면 빈 목록 한 번
// 그림: 금빛 공 (반짝 윤기 + 줄무늬 한 줄) + 양옆의 하얀 날개가 빠르게 파닥, 날면 금빛 자취. 잡혀 있으면 날개를 접고,
// 잡혔던 스니치가 사라지면 그 자리에 금빛 반짝이가 퐁 터짐. 받은 자리 사이는 부드럽게 따라감 (SN_EASE).
// CPU를 아끼는 규칙: 스니치나 반짝이가 있는 동안만 requestAnimationFrame. 한 번에 SN_MAX개까지.
// 브라우저에서 trail.html?snitch=1로 앱 없이 볼 수 있음.

const SN_MAX = 4;
// 받은 자리로 따라가는 몫 (프레임당)
const SN_EASE = 0.35;
// 날개 파닥 (초당 번) · 날개 길이 (공 지름 배)
const SN_FLAP = 14;
const SN_WING = 1.15;
// 자취 길이 (프레임 수) · 잡혔다 사라질 때 반짝이 시간 (초)·개수
const SN_TRAIL = 8;
const SN_BURST_SECS = 0.7;
const SN_BURST_N = 8;
const SN_GOLD = "#f2c230";
const SN_GOLD_DARK = "#b8860b";

const snCanvas = document.getElementById("snitch");
const snCtx = snCanvas.getContext("2d");
// 그리는 스니치들 [{x, y, tx, ty, size, caught, trail}] · 반짝이 [{x, y, size, born}]
let snitches = [];
let snBursts = [];
let snFrame = 0;

// 새 목록을 받음 (이상한 값은 버림): 가까운 것을 이어 그리고, 잡힌 채 사라진 것은 반짝 터뜨림
function setSnitches(list) {
  const want = (Array.isArray(list) ? list : [])
    .filter((p) => p && Number.isFinite(p.x) && Number.isFinite(p.y) && p.size > 0 && p.size < 500)
    .slice(0, SN_MAX);
  const next = [];
  for (const p of want) {
    let best = -1;
    let bestD = Infinity;
    snitches.forEach((s, i) => {
      const d = Math.hypot(s.tx - p.x, s.ty - p.y);
      if (d < bestD && !next.includes(s)) {
        best = i;
        bestD = d;
      }
    });
    const old = best >= 0 && bestD < p.size * 40 ? snitches[best] : null;
    next.push(old ? Object.assign(old, { tx: p.x, ty: p.y, size: p.size, caught: !!p.caught }) : { x: p.x, y: p.y, tx: p.x, ty: p.y, size: p.size, caught: !!p.caught, trail: [] });
  }
  const now = performance.now();
  for (const s of snitches) if (!next.includes(s) && s.caught) snBursts.push({ x: s.x, y: s.y, size: s.size, born: now });
  snitches = next;
  wakeSnitches();
}

// 날개 하나 (side ±1, 펼침 0…1)
function snWing(s, side, open) {
  const r = s.size / 2;
  const len = s.size * SN_WING * (0.35 + 0.65 * open);
  const lift = r * (0.2 + 0.9 * open);
  snCtx.beginPath();
  snCtx.moveTo(s.x + side * r * 0.6, s.y - r * 0.2);
  snCtx.quadraticCurveTo(s.x + side * (r + len * 0.6), s.y - r - lift, s.x + side * (r + len), s.y - lift * 0.4);
  snCtx.quadraticCurveTo(s.x + side * (r + len * 0.5), s.y - r * 0.1, s.x + side * r * 0.6, s.y + r * 0.25);
  snCtx.closePath();
  snCtx.fill();
  snCtx.stroke();
}

// 스니치 하나 (t = 초)
function drawSnitch(s, t) {
  const r = s.size / 2;
  // 금빛 자취
  s.trail.forEach(([x, y], i) => {
    snCtx.fillStyle = `rgba(255,214,90,${(0.32 * (i + 1)) / s.trail.length})`;
    snCtx.beginPath();
    snCtx.arc(x, y, r * (0.3 + (0.5 * (i + 1)) / s.trail.length), 0, Math.PI * 2);
    snCtx.fill();
  });
  // 날개 (잡히면 접힘)
  const open = s.caught ? 0.15 : 0.5 + 0.5 * Math.sin(t * SN_FLAP * Math.PI * 2);
  snCtx.fillStyle = "rgba(255,255,255,0.92)";
  snCtx.strokeStyle = "rgba(170,160,130,0.9)";
  snCtx.lineWidth = Math.max(1, r * 0.12);
  snWing(s, -1, open);
  snWing(s, 1, open);
  // 공 (가는 테 + 윤기 + 줄무늬)
  snCtx.fillStyle = SN_GOLD;
  snCtx.strokeStyle = SN_GOLD_DARK;
  snCtx.lineWidth = Math.max(1, r * 0.18);
  snCtx.beginPath();
  snCtx.arc(s.x, s.y, r, 0, Math.PI * 2);
  snCtx.fill();
  snCtx.stroke();
  snCtx.beginPath();
  snCtx.arc(s.x, s.y + r * 0.15, r * 0.82, Math.PI * 0.15, Math.PI * 0.85);
  snCtx.stroke();
  snCtx.fillStyle = "rgba(255,250,220,0.9)";
  snCtx.beginPath();
  snCtx.arc(s.x - r * 0.35, s.y - r * 0.35, r * 0.28, 0, Math.PI * 2);
  snCtx.fill();
}

// 잡힌 스니치가 사라진 자리의 금빛 반짝이 (t = 초)
function drawSnBurst(b, t) {
  const k = t / SN_BURST_SECS;
  snCtx.fillStyle = `rgba(255,220,90,${1 - k})`;
  for (let i = 0; i < SN_BURST_N; i++) {
    const a = (i / SN_BURST_N) * Math.PI * 2;
    const d = b.size * (0.4 + 1.6 * k);
    snCtx.beginPath();
    snCtx.arc(b.x + Math.cos(a) * d, b.y + Math.sin(a) * d, b.size * 0.18 * (1 - k * 0.6), 0, Math.PI * 2);
    snCtx.fill();
  }
}

// 한 프레임: 받은 자리로 다가가며 자취를 남기고, 남았으면 다음 프레임
function drawSnitches() {
  snFrame = 0;
  const now = performance.now();
  snBursts = snBursts.filter((b) => (now - b.born) / 1000 < SN_BURST_SECS);
  snCtx.clearRect(0, 0, snCanvas.width, snCanvas.height);
  for (const s of snitches) {
    s.trail.push([s.x, s.y]);
    if (s.trail.length > SN_TRAIL || s.caught) s.trail.shift();
    s.x += (s.tx - s.x) * SN_EASE;
    s.y += (s.ty - s.y) * SN_EASE;
    drawSnitch(s, now / 1000);
  }
  for (const b of snBursts) drawSnBurst(b, (now - b.born) / 1000);
  if (snitches.length || snBursts.length) snFrame = requestAnimationFrame(drawSnitches);
}

// 그리기를 깨움 (이미 돌고 있으면 그대로)
function wakeSnitches() {
  if (!snFrame && (snitches.length || snBursts.length)) snFrame = requestAnimationFrame(drawSnitches);
  else if (!snitches.length && !snBursts.length) snCtx.clearRect(0, 0, snCanvas.width, snCanvas.height);
}

// 캔버스를 창 크기에 맞춤
function fitSnitchCanvas() {
  snCanvas.width = window.innerWidth;
  snCanvas.height = window.innerHeight;
  wakeSnitches();
}

window.addEventListener("resize", fitSnitchCanvas);
fitSnitchCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 것만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  (window.spOn || ((t, e, h) => t.listen(e, h)))(window.__TAURI__.webviewWindow.getCurrentWebviewWindow(), "trail-snitch", (e) => setSnitches(e.payload));
} else if (new URLSearchParams(location.search).get("snitch")) {
  // 앱 없이 브라우저에서 보기 (개발용): 화면을 맴도는 스니치 하나
  document.documentElement.style.background = "#1d2030";
  const t0 = performance.now();
  const spin = () => {
    const t = (performance.now() - t0) / 1000;
    setSnitches([{ x: window.innerWidth / 2 + Math.cos(t * 1.7) * 300, y: window.innerHeight / 3 + Math.sin(t * 2.3) * 120, size: 20, caught: false }]);
    requestAnimationFrame(spin);
  };
  spin();
}
