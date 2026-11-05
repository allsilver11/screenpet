// 🕐 최근 한 시간 창: 백엔드가 만든 내용(`timeline_state` 커맨드, 1초마다 `timeline-state` 이벤트 — screenpet_core::timeline::TimelineView)을 그린다.
// 친구 이름·음식 이름이 들어 있으니 글자는 textContent로만 넣는다. 시각은 받은 때의 시계에서 "몇 초 전"을 빼서 만든다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

// 자리 색 (rows 순번 — 많이 있던 곳부터). 넘치면 돌려 쓴다. 순서대로 목록의 점이 막대 색을 알려 준다
const PLACE_COLORS = ["#f0a860", "#7cb8e0", "#9ccf7a", "#e58fb0", "#b49ae0", "#e6c35c", "#6fc7b8", "#d99a7a"];

function el(tag, className, text) {
  const e = document.createElement(tag);
  if (className) e.className = className;
  if (text !== undefined) e.textContent = text;
  return e;
}

function colorOf(row) {
  return PLACE_COLORS[row % PLACE_COLORS.length];
}

// 머문 시간: 분까지 ("14분"), 1분이 안 되면 null. 한 시간이면 "1시간"
function duration(secs) {
  if (secs >= 3600) return "1시간";
  return secs < 60 ? null : `${Math.floor(secs / 60)}분`;
}

// 창 높이를 내용에 맞춘다 (바뀔 때만 — 백엔드가 화면 크기 안으로 자른다)
let fitted = 0;
function fit() {
  const h = Math.ceil(document.body.getBoundingClientRect().height);
  if (h && h !== fitted) {
    fitted = h;
    invoke("timeline_fit", { height: h }).catch(() => {});
  }
}

function clock(ms) {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

// 막대에서의 왼쪽 위치 (%): 한 시간 전 = 0, 지금 = 100
function leftOf(ago, W) {
  return (100 * (W - ago)) / W;
}

function show(view, nowMs = Date.now()) {
  const W = view.window_secs || 3600;
  const rows = view.rows || [];
  const spans = view.spans || [];
  const foods = view.foods || [];
  const meals = view.meals || [];

  // 지금: 자리 + 그 자리에 온 지 얼마나
  const sum = document.getElementById("sum");
  sum.textContent = "";
  const last = spans[spans.length - 1];
  if (view.now != null && rows[view.now] && last) {
    const r = rows[view.now];
    const d = duration(last.ago_start - last.ago_end);
    sum.append("지금: ", el("b", "", `${r.emoji} ${r.label}`), d ? ` (${d}째)` : " (방금)");
  }

  const strip = document.getElementById("strip");
  strip.textContent = "";
  for (const s of spans) {
    const bit = el("i");
    bit.style.left = `${leftOf(s.ago_start, W)}%`;
    bit.style.width = `${(100 * (s.ago_start - s.ago_end)) / W}%`;
    bit.style.background = colorOf(s.row);
    const r = rows[s.row];
    if (r) bit.title = `${r.emoji} ${r.label} · ${duration(s.ago_start - s.ago_end) || "1분 미만"}`;
    strip.append(bit);
  }
  const bites = document.getElementById("bites");
  bites.textContent = "";
  for (const m of meals) {
    const b = el("span", "", m.emoji);
    b.style.left = `${Math.min(98, Math.max(2, leftOf(m.ago, W)))}%`;
    b.title = `${clock(nowMs - m.ago * 1000)} ${m.name}`;
    bites.append(b);
  }

  const foodList = document.getElementById("foods");
  foodList.textContent = "";
  for (const f of foods) {
    const chip = el("span", "", `${f.emoji} ${f.name} `);
    chip.append(el("b", "", `${f.count}번`));
    foodList.append(chip);
  }
  document.getElementById("food-box").hidden = !foods.length;

  // 순서대로 (최근 것부터): 자리 칸과 먹은 때를 섞는다
  const log = document.getElementById("log");
  log.textContent = "";
  const items = [
    ...spans.map((s) => ({ ago: s.ago_start, span: s })),
    ...meals.map((m) => ({ ago: m.ago, meal: m })),
  ].sort((a, b) => a.ago - b.ago);
  for (const it of items) {
    if (it.meal) {
      const entry = el("div", "entry meal");
      entry.append(el("span", "when", clock(nowMs - it.meal.ago * 1000)), el("span"), el("span", "name", `${it.meal.emoji} ${it.meal.name}`));
      log.append(entry);
      continue;
    }
    const s = it.span;
    const r = rows[s.row];
    if (!r) continue;
    const entry = el("div", "entry");
    const end = s.ago_end < 1 ? "지금" : clock(nowMs - s.ago_end * 1000);
    const dot = el("span", "dot");
    dot.style.background = colorOf(s.row);
    entry.append(el("span", "when", `${clock(nowMs - s.ago_start * 1000)} – ${end}`), dot, el("span", "name", `${r.emoji} ${r.label}`));
    log.append(entry);
  }
  log.hidden = !items.length;
  fit();
}

setupFormKeys();

(async () => {
  await here.listen("timeline-state", (e) => show(e.payload));
  show(await invoke("timeline_state"));
})();
