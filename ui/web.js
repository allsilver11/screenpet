// 🕷️ 거미인간의 줄과 거미집 그리기. 🌈 무지개 구름·🖍️ 낙서와 같은 모니터 덮개 창 (trail.html)의 가운데 캔버스에 그림.
// 앱(src-tauri/src/trail.rs)이 보내는 것 (모두 논리 px, 이 창 왼쪽 위가 0,0):
//   trail-webs      새 거미집들 [{id, x, y, corner, size, pixel, age, life, shots, build}] — corner는 top_left·top_right·bottom_left·bottom_right,
//                   🕸️⭕ center면 둥근 거미집 (x·y가 가운데 — 살 7–9개를 사방으로, 대충 8각형인 비정형 고리, orbShape),
//                   x·y는 모서리 점, size는 반지름, age는 이미 지난 초, life는 그대로 남는 초 (그 뒤 WEB_FADE초 동안 흐려짐),
//                   shots·build: 줄을 shots번 쏘아 build초 동안 짬 — 쏠 때마다 살·고리가 차례로 생김 (build가 없으면 WEB_GROW초에 핌)
//   trail-lines     지금 뻗어 있는 줄 전부 [{x0, y0, x1, y1}] (손 → 줄 끝) — 바뀔 때마다 통째로, 없으면 빈 목록
//   trail-web-drop  창이 닫히거나 다른 모니터로 가서 지울 거미집 id들 — WEB_DROP초 동안 흐려짐
//   trail-web-move  창을 따라 옮길 거미집 [{id, x, y, hidden}] — hidden이면 앞 창에 가려져 그리지 않음
//   trail-web-clear 🧹 다 치우기·한 시간마다 — 모든 거미집이 WEB_DROP초 동안 흐려짐
// CPU를 아끼는 규칙: 거미집 MAX_WEBS개까지, 피어나거나 흐려지거나 줄이 있는 동안만 requestAnimationFrame, 가만히 있는 동안은 타이머 하나.
// 브라우저에서 trail.html?web=1 (&pixel=1, &light=1)로 앱 없이 볼 수 있음.

const WEB_GROW = 0.6;
const WEB_LIFE = 480;
// 받아들이는 남는 시간 (초) — 밖이면 WEB_LIFE
const WEB_LIFE_RANGE = [5, 3600];
const WEB_FADE = 4;
const WEB_DROP = 0.8;
const MAX_WEBS = 12;
// 받아들이는 짜는 시간 (초)·줄 횟수
const WEB_BUILD_MAX = 30;
const WEB_SHOTS_MAX = 12;
// 거미집 조각 수: 살 [최소, 최대] + 고리 [최소, 최대] — 거미집마다 (id로 정한 주사위) 다르게, 짤 때 살 → 고리 차례로 생김
const WEB_SPOKES = [4, 7];
const WEB_RINGS = [3, 5];
const WEB_CORNERS = { top_left: [1, 1], top_right: [-1, 1], bottom_left: [1, -1], bottom_right: [-1, -1] };
// 🕸️⭕ 둥근 거미집 (corner "center"): 살 [최소, 최대] + 고리 [최소, 최대]
const ORB_KIND = "center";
const ORB_SPOKES = [7, 9];
const ORB_RINGS = [3, 5];
// 실 (밝은 흰빛) 아래에 깔리는 그늘 — 밝은 창 위에서도 보이게
const SILK = "rgba(244,247,255,0.92)";
const SILK_SHADE = "rgba(34,38,56,0.45)";

const webCanvas = document.getElementById("web");
const wctx = webCanvas.getContext("2d");
let webs = [];
let webLines = [];
let webFrame = 0;
let webTimer = 0;

// 새 거미집들을 받음 (이상한 값은 버림)
function addWebs(list) {
  const now = performance.now();
  for (const p of list || []) {
    if (!(WEB_CORNERS[p.corner] || p.corner === ORB_KIND) || !Number.isFinite(p.x) || !Number.isFinite(p.y) || !(p.size > 0 && p.size < 2000)) continue;
    const life = p.life >= WEB_LIFE_RANGE[0] && p.life <= WEB_LIFE_RANGE[1] ? p.life : WEB_LIFE;
    if (!(p.age >= 0 && p.age < life + WEB_FADE)) continue;
    webs = webs.filter((w) => w.id !== p.id);
    const weave = p.build > 0 && p.build <= WEB_BUILD_MAX && Number.isInteger(p.shots) && p.shots > 1 && p.shots <= WEB_SHOTS_MAX;
    const build = weave ? p.build : 0;
    const shots = weave ? p.shots : 1;
    webs.push({ id: p.id, x: p.x, y: p.y, corner: p.corner, size: p.size, pixel: !!p.pixel, life, born: now - p.age * 1000, gone: 0, build, shots });
  }
  if (webs.length > MAX_WEBS) webs = webs.slice(webs.length - MAX_WEBS);
  wakeWebs();
}

// 지금 뻗은 줄을 통째로 바꿈
function setWebLines(list) {
  webLines = (list || []).filter((l) => [l.x0, l.y0, l.x1, l.y1].every(Number.isFinite));
  wakeWebs();
}

// 이 id들의 거미집을 곧 흐리게 지움 (ids가 없으면 전부)
function dropWebs(ids) {
  const now = performance.now();
  for (const w of webs) {
    if ((!ids || ids.includes(w.id)) && !w.gone) w.gone = now;
  }
  wakeWebs();
}

// 창을 따라 거미집을 옮기거나 숨김 (앞 창에 가려짐) — 이상한 값은 무시
function moveWebs(list) {
  for (const m of list) {
    if (!m || !Number.isFinite(m.x) || !Number.isFinite(m.y)) continue;
    const w = webs.find((w) => w.id === m.id);
    if (!w) continue;
    w.x = m.x;
    w.y = m.y;
    w.hidden = !!m.hidden;
  }
  wakeWebs();
}

// 그리기 타이머를 지우고 다음 프레임을 바로 그리게 함
function wakeWebs() {
  if (webTimer) clearTimeout(webTimer);
  webTimer = 0;
  if (!webFrame) webFrame = requestAnimationFrame(drawWebs);
}

// 짜는 거미집이 지금까지 생긴 몫 (0..1): 줄 하나 (build / (shots − 1)초)마다 앞 절반 동안 조각이 몇 개씩 생김
function wovenShare(w, t) {
  const step = w.build / (w.shots - 1);
  const k = Math.min(w.shots - 1, Math.floor(t / step));
  const local = Math.min(1, Math.max(0, (t - k * step) / (step * 0.5)));
  return Math.min(1, (k + local) / w.shots);
}

// 거미집 하나의 투명도와 피어난 몫 (0..1)
function webState(w, now) {
  const t = (now - w.born) / 1000;
  const grown = w.build > 0 ? wovenShare(w, t) : Math.min(1, t / WEB_GROW);
  let alpha = t < w.life ? 1 : Math.max(0, 1 - (t - w.life) / WEB_FADE);
  if (w.gone) alpha = Math.min(alpha, Math.max(0, 1 - (now - w.gone) / 1000 / WEB_DROP));
  if (w.pixel) alpha = Math.ceil(alpha * 4) / 4;
  return { t, grown, alpha };
}

// 모서리에서 안쪽으로 펼쳐지는 부채꼴 거미집 (살 다섯 + 처진 고리 넷) · 🕸️⭕ 둥근 거미집. 짜는 거미집은 조각이 차례로 (살 → 고리), 아니면 통째로 커짐
function webPaths(w, grown) {
  if (w.build > 0) return wovenPaths(w, grown);
  return shapePaths(w, w.size * grown);
}

// 반지름 r인 거미집의 선들 (모양 종류대로)
function shapePaths(w, r) {
  return w.corner === ORB_KIND ? orbPaths(w, r) : fanPaths(w, r);
}

// 짜는 중: 다 된 조각은 다, 짜는 조각은 그만큼 (살은 길이, 고리는 앞쪽 점들)
function wovenPaths(w, grown) {
  const all = shapePaths(w, w.size);
  const count = grown * all.length;
  const done = Math.floor(count);
  const out = all.slice(0, done);
  const frac = count - done;
  if (done < all.length && frac > 0) {
    const p = all[done];
    if (p.length === 2) {
      out.push([p[0], [p[0][0] + (p[1][0] - p[0][0]) * frac, p[0][1] + (p[1][1] - p[0][1]) * frac]]);
    } else {
      out.push(p.slice(0, Math.max(2, Math.ceil(p.length * frac))));
    }
  }
  return out;
}

// id로 정해지는 주사위 (0..1) — 같은 거미집은 늘 같은 모양 (창을 따라 옮겨도)
function webRng(id) {
  let a = (Number(id) * 2654435761) >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

// 거미집마다 다른 삐뚤빼뚤한 모양 (한 번 정해 w.shape에 둠): 살 각도·길이, 고리 높이·점마다 흔들림·처짐 — 모두 반지름 안
function webShape(w) {
  if (w.shape) return w.shape;
  const rnd = webRng(w.id);
  const n = WEB_SPOKES[0] + Math.floor(rnd() * (WEB_SPOKES[1] - WEB_SPOKES[0] + 1));
  const step = Math.PI / 2 / (n - 1);
  const angs = [];
  for (let k = 0; k < n; k++) {
    const jit = k === 0 || k === n - 1 ? 0 : (rnd() - 0.5) * 0.7 * step;
    angs.push(k * step + jit);
  }
  const lens = angs.map(() => 0.72 + 0.28 * rnd());
  const m = WEB_RINGS[0] + Math.floor(rnd() * (WEB_RINGS[1] - WEB_RINGS[0] + 1));
  const rings = [];
  for (let q = 1; q <= m; q++) {
    const base = (q / (m + 0.35)) * (0.9 + 0.2 * rnd());
    const pts = angs.map(() => base * (0.92 + 0.14 * rnd()));
    const sags = angs.map(() => 0.74 + 0.16 * rnd());
    rings.push({ pts, sags });
  }
  w.shape = { angs, lens, rings };
  return w.shape;
}

// 반지름 r인 부채꼴 거미집의 선들: 살 다음 고리 (모양은 webShape — 살 끝을 넘지 않음)
function fanPaths(w, r) {
  const [sx, sy] = WEB_CORNERS[w.corner];
  const { angs, lens, rings } = webShape(w);
  const out = [];
  const at = (ang, rr) => [w.x + sx * rr * Math.cos(ang), w.y + sy * rr * Math.sin(ang)];
  angs.forEach((a, k) => out.push([at(a, 0), at(a, r * lens[k])]));
  for (const ring of rings) {
    const line = [];
    for (let k = 0; k < angs.length; k++) {
      const rr = r * Math.min(ring.pts[k], lens[k] * 0.98);
      line.push(at(angs[k], rr));
      if (k + 1 < angs.length) {
        const mid = Math.min(rr, r * Math.min(ring.pts[k + 1], lens[k + 1] * 0.98));
        line.push(at((angs[k] + angs[k + 1]) / 2, mid * ring.sags[k]));
      }
    }
    out.push(line);
  }
  return out;
}

// 🕸️⭕ 둥근 거미집마다 다른 삐뚤빼뚤한 모양 (한 번 정해 w.shape에 둠): 살 7–9개를 사방으로 (각도·길이 제각각),
// 고리는 살마다 한 점 + 살 사이로 안쪽 처짐 — 대충 8각형이지만 점마다 흔들림. 모두 반지름 안
function orbShape(w) {
  if (w.shape) return w.shape;
  const rnd = webRng(w.id);
  const n = ORB_SPOKES[0] + Math.floor(rnd() * (ORB_SPOKES[1] - ORB_SPOKES[0] + 1));
  const step = (Math.PI * 2) / n;
  const turn = rnd() * step;
  const angs = [];
  for (let k = 0; k < n; k++) angs.push(turn + k * step + (rnd() - 0.5) * 0.55 * step);
  const lens = angs.map(() => 0.78 + 0.22 * rnd());
  const m = ORB_RINGS[0] + Math.floor(rnd() * (ORB_RINGS[1] - ORB_RINGS[0] + 1));
  const rings = [];
  for (let q = 1; q <= m; q++) {
    const base = (q / (m + 0.4)) * (0.78 + 0.12 * rnd());
    const pts = angs.map(() => base * (0.9 + 0.16 * rnd()));
    const sags = angs.map(() => 0.84 + 0.12 * rnd());
    rings.push({ pts, sags });
  }
  w.shape = { angs, lens, rings };
  return w.shape;
}

// 반지름 r인 둥근 거미집의 선들: 가운데에서 사방으로 살 다음, 안쪽부터 고리 (닫힌 줄 — 살 끝을 넘지 않음)
function orbPaths(w, r) {
  const { angs, lens, rings } = orbShape(w);
  const n = angs.length;
  const out = [];
  const at = (ang, rr) => [w.x + rr * Math.cos(ang), w.y + rr * Math.sin(ang)];
  angs.forEach((a, k) => out.push([at(a, 0), at(a, r * lens[k])]));
  for (const ring of rings) {
    const line = [];
    for (let k = 0; k <= n; k++) {
      const i = k % n;
      const rr = r * Math.min(ring.pts[i], lens[i] * 0.97);
      line.push(at(angs[i] + (k === n ? Math.PI * 2 : 0), rr));
      if (k < n) {
        const j = (i + 1) % n;
        const next = angs[j] + (j === 0 ? Math.PI * 2 : 0);
        const mid = Math.min(rr, r * Math.min(ring.pts[j], lens[j] * 0.97));
        line.push(at((angs[i] + next) / 2, mid * ring.sags[i]));
      }
    }
    out.push(line);
  }
  return out;
}

// 선 묶음을 그늘 → 실 순서로 그림
function strokeSilk(paths, width, pixel) {
  wctx.lineCap = pixel ? "square" : "round";
  wctx.lineJoin = pixel ? "miter" : "round";
  for (const [color, wd] of [[SILK_SHADE, width + 1.6], [SILK, width]]) {
    wctx.strokeStyle = color;
    wctx.lineWidth = pixel ? Math.max(1, Math.round(wd)) : wd;
    wctx.beginPath();
    for (const p of paths) {
      p.forEach(([x, y], i) => (i ? wctx.lineTo(pixel ? Math.round(x) : x, pixel ? Math.round(y) : y) : wctx.moveTo(pixel ? Math.round(x) : x, pixel ? Math.round(y) : y)));
    }
    wctx.stroke();
  }
}

// 줄과 거미집을 그림. 움직이는 것이 없으면 다음 흐려지기까지 잠
function drawWebs(now) {
  webFrame = 0;
  wctx.clearRect(0, 0, webCanvas.width, webCanvas.height);
  let moving = webLines.length > 0;
  let next = Infinity;
  webs = webs.filter((w) => webState(w, now).alpha > 0 || webState(w, now).t < WEB_GROW);
  for (const w of webs) {
    const s = webState(w, now);
    if (s.grown < 1 || s.t >= w.life || w.gone) moving = true;
    else next = Math.min(next, w.life - s.t);
    if (s.alpha <= 0 || w.hidden) continue;
    wctx.globalAlpha = s.alpha;
    strokeSilk(webPaths(w, s.grown), Math.max(1, w.size / 60), w.pixel);
  }
  wctx.globalAlpha = 1;
  if (webLines.length) strokeSilk(webLines.map((l) => [[l.x0, l.y0], [l.x1, l.y1]]), 1.6, false);
  if (moving) {
    webFrame = requestAnimationFrame(drawWebs);
  } else if (webs.length) {
    webTimer = setTimeout(() => {
      webTimer = 0;
      wakeWebs();
    }, Math.ceil(next * 1000) + 5);
  }
}

// 줄·거미집 캔버스를 창 크기에 맞춤
function fitWebCanvas() {
  webCanvas.width = window.innerWidth;
  webCanvas.height = window.innerHeight;
  wakeWebs();
}

window.addEventListener("resize", fitWebCanvas);
fitWebCanvas();

if (window.__TAURI__) {
  // 이 창으로 온 것만 (전역 listen은 다른 모니터 창으로 보낸 emit_to까지 받음)
  const me = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
  me.listen("trail-webs", (e) => addWebs(e.payload));
  me.listen("trail-lines", (e) => setWebLines(e.payload));
  me.listen("trail-web-drop", (e) => dropWebs(Array.isArray(e.payload) ? e.payload : []));
  me.listen("trail-web-clear", () => dropWebs(null));
  me.listen("trail-web-move", (e) => moveWebs(Array.isArray(e.payload) ? e.payload : []));
} else if (new URLSearchParams(location.search).get("web")) {
  webPreview();
}

// 앱 없이 브라우저에서 보기 (개발용): 네 모서리 거미집 + 🕸️⭕ 둥근 거미집 셋 (짜는 중부터 — &orb=1이면 둥근 것만 다 짠 채로) + 줄 하나
function webPreview() {
  const q = new URLSearchParams(location.search);
  const pixel = !!q.get("pixel");
  const orbOnly = !!q.get("orb");
  document.documentElement.style.background = q.get("light") ? "#eef1f6" : "#1d2030";
  const W = window.innerWidth;
  const H = window.innerHeight;
  if (!orbOnly) addWebs(Object.keys(WEB_CORNERS).map((corner, i) => ({ id: i + 1, corner, x: corner.endsWith("left") ? 0 : W, y: corner.startsWith("top") ? 0 : H, size: 90, pixel, age: 1, life: 60 })));
  const age = orbOnly ? 10 : 0;
  addWebs([[0.25, 0.4, 70], [0.55, 0.45, 110], [0.82, 0.55, 85]].map(([fx, fy, size], i) => ({ id: 11 + i, corner: ORB_KIND, x: W * fx, y: H * fy, size, pixel, age, life: 60, shots: 6, build: 1.6 })));
  if (!orbOnly) setWebLines([{ x0: W / 2, y0: H * 0.8, x1: W * 0.8, y1: 0 }]);
}
