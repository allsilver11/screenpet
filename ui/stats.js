// 📊 펫 통계 (개발자 컴퓨터만 — spec 2026-11-23-pet-stats). 릴레이의 날짜별 익명 합계를 받아 그림:
// 기간 합계 표(종류마다 키운 사람 · 새로 고름 · 바꿔 들임 · 바꿔 떠남)와 막대그래프, 날짜별 표.
// 받기: stats_list() → `stats-days` {ok, reason, days: [{day, reports, kept, chosen, changed: [{from, to, n}], versions}]} (오래된 날 먼저).
// 이름은 릴레이가 아는 종류이거나 "other"뿐이지만 그래도 textContent로만 넣음.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const emptyEl = document.getElementById("empty");
const totalsEl = document.getElementById("totals");
const chartEl = document.getElementById("chart");
const dailyEl = document.getElementById("daily");
const daysEl = document.getElementById("days");
let all = [];

// 종류 이모지와 한글 이름 (모르는 것은 그대로)
const PET_NAME = { cat: "🐱 고양이", rabbit: "🐰 토끼", penguin: "🐧 펭귄", squirrel: "🐿️ 다람쥐", heumul: "🤸 흐물이", spider: "🕷️ 거미인간", bat: "🦇 박쥐인간", super: "🦸 슈퍼인간", musoe: "🛡️ 무쇠인간", harry: "🧙 해리포터", hermione: "📚 헤르미온느", voldemort: "🐍 볼드모트", dementor: "👻 디멘터", hagrid: "🧔 해그리드", pumpkin: "🎃 호박", santa: "🎅 산타", rudolph: "🦌 루돌프", snowman: "⛄ 눈사람", dino_green: "🦖 초록 공룡", dino_yellow: "🦕 노란 공룡", other: "❔ 기타" };

// 클래스와 글자를 넣은 요소를 만듦
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

// 종류 이름을 보이는 글로
function petName(id) {
  return PET_NAME[id] || id;
}

// 못 받은 이유 → 한 줄
function reasonText(reason) {
  switch (reason) {
    case "offline":
      return "인터넷 친구 서버에 연결되어 있지 않아요";
    case "old":
      return "서버가 아직 통계를 모르는 옛 버전이에요";
    default:
      return "통계를 받지 못했어요 (이 컴퓨터가 편지함 독자인지 확인해 주세요)";
  }
}

// 하루치들을 종류별로 더함: {species: {kept, chosen, into, outof}}, 보고 수
function sumUp(days) {
  const rows = {};
  const row = (s) => (rows[s] ||= { kept: 0, chosen: 0, into: 0, outof: 0 });
  let reports = 0;
  for (const d of days) {
    reports += d.reports || 0;
    for (const [s, n] of Object.entries(d.kept || {})) row(s).kept += n;
    for (const [s, n] of Object.entries(d.chosen || {})) row(s).chosen += n;
    for (const c of d.changed || []) {
      row(c.to).into += c.n;
      row(c.from).outof += c.n;
    }
  }
  return { rows, reports };
}

// 머리줄과 줄들로 표를 만듦
function table(head, lines) {
  const t = el("table");
  const tr = el("tr");
  for (const h of head) tr.append(el("th", "", h));
  t.append(tr);
  for (const line of lines) {
    const r = el("tr");
    for (const v of line) r.append(el("td", "", String(v)));
    t.append(r);
  }
  return t;
}

// 고른 기간의 합계·그래프·날짜별 표를 그림
function render() {
  const n = Number(daysEl.value) || 30;
  const days = all.slice(-n);
  totalsEl.replaceChildren();
  chartEl.replaceChildren();
  dailyEl.replaceChildren();
  if (!days.length) {
    emptyEl.textContent = "아직 받은 통계가 없어요";
    emptyEl.style.display = "block";
    return;
  }
  emptyEl.style.display = "none";
  const { rows, reports } = sumUp(days);
  const order = Object.keys(rows).sort((a, b) => rows[b].kept - rows[a].kept || a.localeCompare(b));
  totalsEl.append(el("h3", "", `기간 합계 (${days[0].day} ~ ${days[days.length - 1].day}, 보고 ${reports}개)`));
  totalsEl.append(table(["종류", "키운 날·사람", "새로 고름", "바꿔 들임", "바꿔 떠남"], order.map((s) => [petName(s), rows[s].kept, rows[s].chosen, rows[s].into, rows[s].outof])));
  // 막대그래프: 키운 수
  chartEl.append(el("h3", "", "키운 수 막대그래프"));
  const max = Math.max(1, ...order.map((s) => rows[s].kept));
  for (const s of order) {
    const bar = el("div", "bar");
    const fill = el("div", "fill");
    fill.style.width = `${Math.round((rows[s].kept / max) * 300)}px`;
    bar.append(el("span", "name", petName(s)), fill, el("span", "n", String(rows[s].kept)));
    chartEl.append(bar);
  }
  // 날짜별 (최신 먼저)
  dailyEl.append(el("h3", "", "날짜별"));
  for (const d of [...days].reverse()) {
    const one = sumUp([d]).rows;
    const names = Object.keys(one).sort((a, b) => one[b].kept - one[a].kept || a.localeCompare(b));
    const versions = Object.entries(d.versions || {}).map(([v, c]) => `v${v} ${c}`).join(", ");
    dailyEl.append(el("div", "meta", `${d.day} · 보고 ${d.reports}개${versions ? " · " + versions : ""}`));
    dailyEl.append(table(["종류", "키움", "새로 고름", "바꿔 들임"], names.map((s) => [petName(s), one[s].kept, one[s].chosen, one[s].into])));
  }
}

// 백엔드에 새로 청함 (대답은 `stats-days`로 옴)
function ask() {
  emptyEl.textContent = "불러오는 중…";
  emptyEl.style.display = "block";
  invoke("stats_list");
}

// 받은 대답을 그림. 못 받았으면 까닭을 보임
function onDays(v) {
  if (!v.ok) {
    all = [];
    totalsEl.replaceChildren();
    chartEl.replaceChildren();
    dailyEl.replaceChildren();
    emptyEl.textContent = reasonText(v.reason);
    emptyEl.style.display = "block";
    return;
  }
  all = v.days || [];
  render();
}

daysEl.addEventListener("change", render);
document.getElementById("reload").addEventListener("click", ask);
document.getElementById("close").addEventListener("click", () => invoke("stats_close"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape") invoke("stats_close");
});

(async () => {
  await here.listen("stats-days", (e) => onDays(e.payload));
  // 열 때마다 새로
  await here.listen("stats-open", () => ask());
  ask();
})();
