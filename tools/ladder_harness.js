// ui/ladder.js를 브라우저 없이 돌려 결과를 JSON으로 적는다 (tools/test_ladder_ui.py가 쓴다).
// LADDER_MARGIN=60 node tools/ladder_harness.js → 결과 JSON. 여백은 시험이 코어 ladder::WINDOW_MARGIN에서 읽어 넘긴다.
// 앞쪽은 계산(LADDER), 뒤쪽은 창 스크립트 전체를 가짜 DOM에 올려 화면 상태를 넣고 버튼·키를 눌러 본다.
// 게임 판은 코어 ladder::trace와 같은 규칙으로 여기서 만든다 (가로줄 → 칸마다 길).

const fs = require("fs");
const path = require("path");
const vm = require("vm");
const SCRIPT = path.join(__dirname, "..", "ui", "ladder.js");
const PAGE = path.join(__dirname, "..", "ui", "ladder.html");
const L = require(SCRIPT);

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

function game(n, column, rows, names = ["나비", "나비 ②", "곰돌이와친구들", "Bob", "🐱고양이", "가", "나", "다", "라", "마"]) {
  const results = Array.from({ length: n }, (_, i) => ({ text: i === 0 ? "커피 쏘기 당첨!" : "통과", mood: i === 0 ? "bad" : "good" }));
  return {
    game_id: "g",
    players: names.slice(0, n).map((name) => ({ name, pet: "cat", style: "" })),
    results,
    rows,
    paths: names.slice(0, n).map((_, i) => trace(rows, i)),
    me: 1,
    host: 0,
    print: "🐱🍎🎈🌙",
    column,
    skip: false,
    host_name: "방장",
  };
}

const rows10 = [[0, 2, 4, 6, 8], [1, 3, 5, 7], [0, 4, 8], [], [2, 6], [1, 5], [3, 7], [0, 8], [4]];

// 가로 구간마다 그 줄에 진짜 가로줄이 있는지 (옆 칸으로만, 있는 가로줄로만), 아래로만 가는지
function walksTheRungs(g, lay, pts) {
  return pts.slice(1).every((b, k) => {
    const a = pts[k];
    if (b.y < a.y) return false;
    if (a.x === b.x) return true;
    if (a.y !== b.y) return false;
    const r = lay.rowYs.indexOf(a.y);
    const [from, to] = [lay.xs.indexOf(a.x), lay.xs.indexOf(b.x)];
    return r >= 0 && Math.abs(from - to) === 1 && g.rows[r].includes(Math.min(from, to));
  });
}

// 가로줄을 몇 개 지났는지: 길에 든 가로 구간 수와 칸마다 길이 바뀐 수가 같아야 한다
function routes(g, W, H) {
  const lay = L.layout(g, W, H);
  return {
    lay,
    routes: g.players.map((_, i) => {
      const pts = L.route(g, lay, i);
      const end = L.pointAt(pts, L.length(pts) + 5);
      const path = g.paths[i];
      return {
        first: pts[0],
        last: pts[pts.length - 1],
        end: end.p,
        walks: walksTheRungs(g, lay, pts),
        crossings: pts.slice(1).filter((p, k) => p.y === pts[k].y && p.x !== pts[k].x).length,
        turns: path.slice(1).filter((c, k) => c !== path[k]).length,
        endCol: path[path.length - 1],
      };
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

const MARGIN = Number(process.env.LADDER_MARGIN);
if (!MARGIN) throw new Error("LADDER_MARGIN이 필요해요");

// 10명 판, 이름이 모두 길다 (번호 달린 이름까지): 칸 너비 48 / 56 / 72에서 실제 글자로 이름표 폭을 재 본다
const longNames = Array.from({ length: 10 }, (_, i) => `아주아주기다란이름${i ? ` ${String.fromCodePoint(0x2460 + i)}` : ""}`);
out.tags = [48, 56, 72].map((col) => {
  const g = game(10, col, rows10, longNames);
  const W = col * 10 + MARGIN;
  const { xs } = L.layout(g, W, 540);
  const max = L.nameChars(col);
  const texts = longNames.map((n) => L.tagText(n, max));
  const widths = texts.map((t) => Array.from(t).length * L.TAG_CHAR_PX + L.TAG_PAD_PX);
  return {
    col,
    max,
    texts,
    // 같은 줄(두 칸 건너) 이웃끼리 겹치는 이름표
    overlaps: xs.slice(2).filter((x, k) => x - xs[k] < (widths[k + 2] + widths[k]) / 2).length,
    outside: xs.filter((x, i) => x - widths[i] / 2 < 0 || x + widths[i] / 2 > W).length,
    lifts: [0, 1, 2, 3].map(L.tagLift),
  };
});
out.tagText = [["곰돌이와친구들", 6], ["나비 ②", 6], ["곰돌이와친구들 ②", 6], ["곰돌이와친구들 ③", 6], ["일곱글자입니다 (21)", 6]].map(([n, m]) => L.tagText(n, m));

const g10 = game(10, 48, rows10);
// 창 너비 = 칸 × 인원 + 여백
const W = g10.column * 10 + MARGIN;
out.board = routes(g10, W, 540);
out.boardLow = routes(g10, W, 300);
out.boardSmall = routes(game(2, 72, [[0], [], [0], []]), 72 * 4 + MARGIN, 540);
out.results = L.resultRows(g10);
out.copy = L.copyLines("방장", "🐱🍎🎈🌙", L.resultRows(game(3, 72, [[0], [1], [0]])));
out.moods = ["good", "bad", "none"].map(L.nextMood);
out.moodEmoji = ["good", "bad", "none", "constructor", "__proto__", "toString", undefined].map(L.moodEmoji);
out.answerOfOddKey = ["constructor", "__proto__"].map((a) => L.answerLabel(a)[0]);
const custom = (texts) => texts.map((text) => ({ text, mood: "none" }));
out.inviting = {
  startingEarly: L.invitingSlots(custom(["a", "b", "c", "d"]), 1, 2),
  afterDeleting: L.invitingSlots(custom(["a", "b"]), 1, 2),
  nobodyYet: L.invitingSlots(custom(["a", "b", "c", "d"]), 0, 3),
  someoneDeclined: L.invitingSlots(custom(["a", "b", "c", "d"]), 2, 0),
  blank: L.invitingSlots(custom(["a", " ", "c"]), 2, 0),
};

// ---- 창 스크립트 전체를 가짜 DOM에서 ------------------------------------------------------

class FakeElement {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.style = {};
    this.dataset = {};
    this.listeners = {};
    this.hidden = false;
    this.disabled = false;
    this.textContent = "";
    this.value = "";
    this.classList = { add() {} };
  }
  append(...kids) {
    this.children.push(...kids);
  }
  replaceChildren(...kids) {
    this.children = kids;
  }
  remove() {}
  setAttribute() {}
  addEventListener(type, fn) {
    (this.listeners[type] ??= []).push(fn);
  }
  fire(type, event = {}) {
    for (const fn of this.listeners[type] ?? []) fn(event);
  }
  click() {
    if (!this.disabled) {
      this.fire("click");
      this.onclick?.();
    }
  }
  contains() {
    return false;
  }
  querySelector(sel) {
    return (this[`q ${sel}`] ??= new FakeElement("div"));
  }
  querySelectorAll() {
    return [];
  }
  // 자식 가운데 조건에 맞는 것 (손자까지)
  find(pred) {
    return this.children.flatMap((c) => (c instanceof FakeElement ? [...(pred(c) ? [c] : []), ...c.find(pred)] : []));
  }
}

async function bootWindow() {
  const ids = new Map([...fs.readFileSync(PAGE, "utf-8").matchAll(/id="([\w-]+)"/g)].map(([, id]) => [id, new FakeElement("div")]));
  for (const id of ["join", "decline", "invite", "setup-close", "small-close", "all", "copy", "game-close"]) ids.get(id).tagName = "BUTTON";
  const radios = ["preset", "custom"].map((value) => Object.assign(new FakeElement("input"), { value, checked: value === "preset" }));
  const invoked = [];
  const channels = {};
  const windowListeners = {};
  const timers = [];
  const frames = [];
  const sandbox = {
    console,
    performance: { now: () => 0 },
    setTimeout: (fn) => timers.push(fn),
    requestAnimationFrame: (fn) => (frames[0] = fn),
    loadPetSheet: () => new Promise(() => {}),
    setPixelBody() {},
    window: {
      innerWidth: 780,
      innerHeight: 540,
      addEventListener: (type, fn) => (windowListeners[type] ??= []).push(fn),
      __TAURI__: {
        core: {
          invoke: async (cmd, args) => {
            if (cmd !== "ladder_state") invoked.push([cmd, args]);
            return null;
          },
        },
        webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (name, fn) => (channels[name] = fn) }) },
      },
    },
    document: {
      activeElement: null,
      getElementById: (id) => ids.get(id) ?? (() => { throw new Error(`no #${id} in the page`); })(),
      createElement: (tag) => new FakeElement(tag),
      createElementNS: (_, tag) => new FakeElement(tag),
      querySelectorAll: (sel) => (sel.includes('name="mode"') ? radios : []),
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(fs.readFileSync(SCRIPT, "utf-8"), sandbox);
  await new Promise(setImmediate);
  return {
    $: (id) => ids.get(id),
    radios,
    invoked,
    timers,
    frames,
    show: async (view) => {
      channels["ladder-state"]({ payload: { friends: [], draft: [], joined: 0, ...view } });
      await new Promise(setImmediate);
    },
    // 키를 누른다: 기본 동작을 막았으면 true (막지 않은 단추에서 Enter는 클릭이 된다)
    key: (tagName, key) => {
      let prevented = false;
      for (const fn of windowListeners.keydown) fn({ key, target: { tagName }, preventDefault: () => (prevented = true), stopPropagation() {} });
      return prevented;
    },
  };
}

(async () => {
  const friend = (id, answer = "") => ({ id, name: id, can_invite: true, answer });
  const ui = {};

  // 직접 적기: 한 명만 답했을 때 ✕로 칸을 맞춰 바로 시작한다
  const w = await bootWindow();
  await w.show({ view: "setup", friends: [friend("a"), friend("b"), friend("c")] });
  for (const li of w.$("friends").children) {
    const box = li.children[0];
    box.checked = true;
    box.fire("change");
  }
  w.radios[1].checked = true;
  w.radios[1].fire("change");
  const slotCount = () => w.$("slots").children.length;
  ui.setupSlots = slotCount();
  w.$("invite").click();
  ui.hostInvite = w.invoked.at(-1);
  await w.show({ view: "inviting", joined: 1, all_answered: false, friends: [friend("a", "joined"), friend("b", "waiting"), friend("c", "waiting")] });
  const crosses = () => w.$("slots").find((e) => e.textContent === "✕");
  const snapshot = () => ({ slots: slotCount(), crosses: crosses().length, startDisabled: w.$("invite").disabled, warn: w.$("setup-warn").textContent });
  ui.early = snapshot();
  crosses()[3]?.click();
  crosses()[2]?.click();
  ui.deleted = snapshot();
  w.$("invite").click();
  ui.begin = w.invoked.at(-1);
  // 그 사이 한 명 더 참가하면 빈 칸이 생기고 다시 막힌다
  await w.show({ view: "inviting", joined: 2, all_answered: false, friends: [friend("a", "joined"), friend("b", "joined"), friend("c", "waiting")] });
  ui.anotherJoined = snapshot();

  // 키: 단추 위 Enter·Space는 눌리고, 초대 화면은 키로 답하지 않고, 나머지는 웹뷰 기본 동작을 막는다
  await w.show({ view: "cancelled", cancel: { reason: "host" } });
  ui.keys = {
    enterOnButton: w.key("BUTTON", "Enter"),
    spaceOnButton: w.key("BUTTON", " "),
    typing: w.key("INPUT", "a"),
    enterElsewhere: w.key("DIV", "Enter"),
    refresh: w.key("BUTTON", "F5"),
    tab: w.key("BUTTON", "Tab"),
  };
  await w.show({ view: "invite", host_name: "방장", invited: 2, draft: [{ text: "당첨", mood: "constructor" }, { text: "꽝", mood: "bad" }] });
  ui.keys.enterOnInvite = w.key("BUTTON", "Enter");
  ui.keys.spaceOnInvite = w.key("BUTTON", " ");
  ui.keys.escapeOnInvite = w.key("BUTTON", "Escape");
  ui.chips = w.$("invite-slots").children.map((c) => c.textContent);

  // 결과 칸이 바뀐 잠그기: 최종 칸을 보여 주고 답을 받는다 (키로는 답하지 않는다)
  await w.show({ view: "confirm", host_name: "방장", invited: 2, draft: [{ text: "벌칙 커피", mood: "bad" }, { text: "꽝", mood: "bad" }] });
  ui.confirm = {
    shown: !w.$("v-invite").hidden,
    title: w.$("invite-title").textContent,
    chips: w.$("invite-slots").children.map((c) => c.textContent),
    buttons: [w.$("join").textContent, w.$("decline").textContent],
    notes: [w.$("invite-note").hidden, w.$("confirm-note").hidden],
    enter: w.key("BUTTON", "Enter"),
  };
  w.$("join").click();
  ui.confirm.answered = w.invoked.at(-1);
  await w.show({ view: "invite", host_name: "방장", invited: 2, draft: [{ text: "꽝", mood: "bad" }] });
  ui.confirm.backToInvite = [w.$("join").textContent, w.$("invite-note").hidden, w.$("confirm-note").hidden];

  // 게임: 줄어든 이름표에서도 번호가 남는다
  const g = game(3, 48, [[0], [1], [0]], ["곰돌이와친구들", "곰돌이와친구들 ②", "곰돌이와친구들 ③"]);
  await w.show({ view: "game", game: g });
  ui.nametags = w.$("board").find((e) => e.className?.startsWith("nametag")).map((e) => e.textContent);

  // 모두 한꺼번에 → 다 내려가면 도착 칸이 켜지고, 내 펫이 닿았다고 알리고, 결과 표가 뜬다
  const hits = [];
  w.$("board").find((e) => e.className?.startsWith("slot")).forEach((slot, i) => (slot.classList.add = () => hits.push(i)));
  w.$("all").click();
  w.timers.length = 0;
  w.frames[0](1e7);
  ui.finish = { hits: [...hits].sort(), seen: w.invoked.at(-1)[0], tableBefore: w.$("table").hidden };
  w.timers.forEach((fn) => fn());
  ui.finish.tableAfter = w.$("table").hidden;
  ui.finish.rows = w.$("table-rows").children.length;

  out.ui = ui;
  process.stdout.write(JSON.stringify(out));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
