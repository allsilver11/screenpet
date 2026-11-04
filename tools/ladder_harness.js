// ui/ladder.js의 계산(LADDER)을 브라우저 없이 돌려 결과를 JSON으로 적는다 (tools/test_ladder_ui.py가 쓴다).
// node tools/ladder_harness.js → 결과 JSON.
// 게임 판은 코어 ladder::trace와 같은 규칙으로 여기서 만든다 (가로줄 → 칸마다 길).

const path = require("path");
const L = require(path.join(__dirname, "..", "ui", "ladder.js"));

// 코어 ladder::trace와 같다
function trace(rows, start) {
  let col = start;
  const out = [col];
  for (const row of rows) {
    if (row.includes(col)) col += 1;
    else if (col > 0 && row.includes(col - 1)) col -= 1;
    out.push(col);
  }
  return out;
}

function game(n, column, rows) {
  const names = ["나비", "나비 ②", "곰돌이와친구들", "Bob", "🐱고양이", "가", "나", "다", "라", "마"].slice(0, n);
  const results = Array.from({ length: n }, (_, i) => ({ text: i === 0 ? "커피 쏘기 당첨!" : "통과", mood: i === 0 ? "bad" : "good" }));
  return {
    game_id: "g",
    players: names.map((name) => ({ name, pet: "cat", style: "" })),
    results,
    rows,
    paths: names.map((_, i) => trace(rows, i)),
    me: 1,
    host: 0,
    print: "🐱🍎🎈🌙",
    column,
    skip: false,
    host_name: "방장",
  };
}

const rows10 = [[0, 2, 4, 6, 8], [1, 3, 5, 7], [0, 4, 8], [], [2, 6], [1, 5], [3, 7], [0, 8], [4]];

function routes(g, W, H) {
  const lay = L.layout(g, W, H);
  return {
    lay,
    routes: g.players.map((_, i) => {
      const pts = L.route(g, lay, i);
      const end = L.pointAt(pts, L.length(pts) + 5);
      const axis = pts.slice(1).every((p, k) => p.x === pts[k].x || p.y === pts[k].y);
      return { first: pts[0], last: pts[pts.length - 1], end: end.p, axis, endCol: g.paths[i][g.paths[i].length - 1] };
    }),
  };
}

const out = {};

out.ellipsize = ["여섯글자요", "일곱글자입니다", "🐱🐶🐰🐻🐸🦆🦒", ""].map((s) => L.ellipsize(s, 6));
out.clean = ["  당첨  ", "", "줄\n바꿈", "열세글자열세글자열세글자!", "🍕 피자"].map(L.cleanText);
out.preset = { four_one: L.presetSlots(4, 1), two_five: L.presetSlots(2, 5) };
out.grow = L.growSlots([{ text: "a", mood: "good" }], 3);
out.noShrink = L.growSlots([{ text: "a", mood: "good" }, { text: "b", mood: "bad" }, { text: "c", mood: "none" }], 2).length;
out.problems = {
  extra: L.slotProblem(L.presetSlots(4, 1), 3),
  missing: L.slotProblem(L.presetSlots(2, 1), 3),
  blank: L.slotProblem([{ text: "a", mood: "good" }, { text: " ", mood: "bad" }], 2),
  ok: L.slotProblem(L.presetSlots(3, 1), 3),
};
out.warnings = [L.slotWarning({ extra: 1 }), L.slotWarning({ missing: 2 }), L.slotWarning({ blank: [1] }), L.slotWarning(null)];
out.cancel = [
  { reason: "left", who: "나비" },
  { reason: "left", who: "곰" },
  { reason: "left", who: null },
  { reason: "timeout", who: "방장" },
  { reason: "host" },
  { reason: "invalid" },
].map(L.cancelText);
out.answers = ["waiting", "joined", "busy", "timeout", "weird"].map((a) => L.answerLabel(a)[0]);

// 10명 판: 칸 너비 48 / 56 / 72에서 이름표가 겹치지 않는지 (같은 줄 이웃 = 두 칸 건너)
out.tags = [48, 56, 72].map((col) => {
  const width = L.tagWidth(col);
  return { col, chars: L.nameChars(col), width, sameRowGap: 2 * col - width, lifts: [0, 1, 2, 3].map(L.tagLift) };
});

const g10 = game(10, 48, rows10);
// 창 너비 = 칸 × 인원 + 여백 (코어 ladder::WINDOW_MARGIN)
const MARGIN = 60;
const W = g10.column * 10 + MARGIN;
out.W = W;
out.board = routes(g10, W, 540);
out.boardSmall = routes(game(2, 72, [[0], [], [0], []]), 72 * 4 + MARGIN, 540);
out.results = L.resultRows(g10);
out.copy = L.copyLines("방장", "🐱🍎🎈🌙", L.resultRows(game(3, 72, [[0], [1], [0]])));
out.keys = [["invite", "Enter"], ["invite", " "], ["invite", "Escape"], ["setup", "Enter"]].map(([v, k]) => L.ignoreKey(v, k));
out.moods = ["good", "bad", "none"].map(L.nextMood);

process.stdout.write(JSON.stringify(out));
