// 📊 펫 통계 (개발자 컴퓨터만 — spec 2026-11-23-pet-stats). 릴레이의 날짜별 익명 합계를 받아 그림:
// 기간 합계 표(종류마다 키운 사람(서로 다른 번호) · 하루 평균 키운 사람 · 키우는 비율 · 새로 데려옴 · 바꿔 옴 · 바꿔 감)와 칸 설명, 막대그래프, 날짜별 표.
// 보고에는 누구인지는 없고 앱이 만든 익명 번호만 있음 (정책 2026-11-27): 기간 동안 서로 다른 사람 수는 날마다 온 번호(`people`)와
// 종류마다 키운 번호의 자리(`keepers`)를 모아 겹치지 않게 셈. 번호가 없는 예전 보고는 사람 수에 안 들어가니 그런 날이 있으면 알림.
// 받기: stats_list() → `stats-days` {ok, reason, days: [{day, reports, kept, chosen, changed: [{from, to, n}], versions, people?, keepers?}]} (오래된 날 먼저).
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

// 기간 동안 서로 다른 익명 번호: {all: 앱을 켠 번호들, bySpecies: {종류: 번호들}, missing: 번호 없는 보고가 있었음}
function distinct(days) {
  const all = new Set();
  const bySpecies = {};
  let missing = false;
  for (const d of days) {
    const people = Array.isArray(d.people) ? d.people : [];
    if (people.length < (d.reports || 0)) missing = true;
    for (const p of people) all.add(p);
    for (const [s, slots] of Object.entries(d.keepers || {})) {
      const set = (bySpecies[s] ||= new Set());
      for (const i of Array.isArray(slots) ? slots : []) {
        if (Number.isInteger(i) && i >= 0 && i < people.length) set.add(people[i]);
      }
    }
  }
  return { all, bySpecies, missing };
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

// 소수 한 자리까지 (".0"은 뺌)
function one(x) {
  return String(Math.round(x * 10) / 10);
}

// 사람 수 글자
function people(x) {
  return `${one(x)}명`;
}

// 칸마다 쉬운 말 설명 (표 아래)
const LEGEND = [
  "키운 사람 (서로 다른 번호): 기간 동안 그 종류를 키운 서로 다른 사람 수예요. 앱이 만든 익명 번호로 세서, 누구인지는 모르지만 같은 사람을 두 번 세지 않아요.",
  "하루 평균 키운 사람: 그 종류를 키운 사람 수를 날마다 세서 평균낸 값이에요.",
  "키우는 비율: 앱을 켠 사람 중 그 종류를 키운 사람의 비율이에요 (기간 전체).",
  "새로 데려옴: 처음 설치해 고르거나 펫을 하나 더 데려온 횟수예요.",
  "바꿔 옴: 다른 종류에서 이 종류로 바꾼 횟수예요.",
  "바꿔 감: 이 종류에서 다른 종류로 바꾼 횟수예요.",
  "하루 평균 칸은 날마다 센 값이라 같은 사람이 여러 날 세질 수 있어요.",
];

// 번호 없는 보고가 섞였을 때 알림
const MISSING_NOTE = "익명 번호가 없는 예전 보고는 사람 수에 안 들어가요.";

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
  const avg = (s) => rows[s].kept / days.length;
  const share = (s) => (reports ? `${Math.round((rows[s].kept / reports) * 100)}%` : "-");
  const order = Object.keys(rows).sort((a, b) => rows[b].kept - rows[a].kept || a.localeCompare(b));
  const ids = distinct(days);
  // 번호가 하나도 없으면 서로 다른 사람은 셀 수 없음 ("-")
  const apart = (s) => (ids.all.size ? `${(ids.bySpecies[s] || new Set()).size}명` : "-");
  const whole = ids.all.size ? ` · 서로 다른 사람 ${ids.all.size}명이 앱을 켬` : "";
  totalsEl.append(el("h3", "", `기간 합계 (${days[0].day} ~ ${days[days.length - 1].day} · 통계가 온 날 ${days.length}일 · 하루 평균 ${people(reports / days.length)}이 앱을 켬${whole})`));
  totalsEl.append(table(["종류", "키운 사람 (서로 다른 번호)", "하루 평균 키운 사람", "키우는 비율", "새로 데려옴", "바꿔 옴", "바꿔 감"], order.map((s) => [petName(s), apart(s), people(avg(s)), share(s), rows[s].chosen, rows[s].into, rows[s].outof])));
  if (ids.missing) totalsEl.append(el("div", "legend", MISSING_NOTE));
  const legend = el("div", "legend");
  for (const line of LEGEND) legend.append(el("div", "", line));
  totalsEl.append(legend);
  // 막대그래프: 하루 평균 키운 사람
  chartEl.append(el("h3", "", "하루 평균 키운 사람 막대그래프"));
  const max = Math.max(1e-9, ...order.map(avg));
  for (const s of order) {
    const bar = el("div", "bar");
    const fill = el("div", "fill");
    fill.style.width = `${Math.round((avg(s) / max) * 300)}px`;
    bar.append(el("span", "name", petName(s)), fill, el("span", "n", people(avg(s))));
    chartEl.append(bar);
  }
  // 날짜별 (최신 먼저)
  dailyEl.append(el("h3", "", "날짜별"));
  for (const d of [...days].reverse()) {
    const day = sumUp([d]).rows;
    const names = Object.keys(day).sort((a, b) => day[b].kept - day[a].kept || a.localeCompare(b));
    const versions = Object.entries(d.versions || {}).map(([v, c]) => `v${v} ${c}`).join(", ");
    dailyEl.append(el("div", "meta", `${d.day} · 앱을 켠 사람 ${people(d.reports || 0)}${versions ? " · " + versions : ""}`));
    dailyEl.append(table(["종류", "키운 사람", "새로 데려옴", "바꿔 옴"], names.map((s) => [petName(s), people(day[s].kept), day[s].chosen, day[s].into])));
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
