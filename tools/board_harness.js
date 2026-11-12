// 📮 건의함 두 창(board.js 공개 · mailbox.js 개발자)을 웹뷰 없이 돌려 본다: 가짜 DOM·Tauri, 화면에 보이는 글자를 JSON으로.
// node board_harness.js board|mailbox  → stdout JSON (test_board_ui.py가 본다)
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui");
const mode = process.argv[2];

class El {
  constructor(tag, id) {
    this.tag = tag;
    this.id = id;
    this.children = [];
    this.style = {};
    this.className = "";
    this.handlers = {};
    this.disabled = false;
    this.checked = false;
    this.type = "";
    this._text = "";
  }
  set innerHTML(_) {
    throw new Error("innerHTML used");
  }
  set textContent(t) {
    this._text = String(t);
    this.children = [];
  }
  get textContent() {
    return this._text + this.children.map((c) => (typeof c === "string" ? c : c.textContent)).join("");
  }
  get childElementCount() {
    return this.children.filter((c) => typeof c !== "string").length;
  }
  append(...xs) {
    this.children.push(...xs);
  }
  replaceChildren(...xs) {
    this._text = "";
    this.children = xs;
  }
  addEventListener(ev, fn) {
    (this.handlers[ev] = this.handlers[ev] || []).push(fn);
  }
  fire(ev) {
    const e = { stopPropagation() {}, preventDefault() {} };
    for (const f of this.handlers[ev] || []) f(e);
  }
  all(pred, out = []) {
    for (const c of this.children) {
      if (typeof c === "string") continue;
      if (pred(c)) out.push(c);
      c.all(pred, out);
    }
    return out;
  }
}

const byId = {};
const document = {
  getElementById: (id) => (byId[id] = byId[id] || new El("div", id)),
  createElement: (tag) => new El(tag),
};
const invoked = [];
const listeners = {};
let mailboxState = [];
const window = {
  addEventListener() {},
  __TAURI__: {
    core: { invoke: async (cmd, args) => (invoked.push([cmd, args || null]), cmd === "mailbox_state" ? mailboxState : undefined) },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (ev, fn) => (listeners[ev] = fn) }) },
  },
};
const ctx = vm.createContext({ window, document, console, setTimeout, clearTimeout, Date, Math, String, Promise });
// 페이지에 처음 적힌 #empty 글자
const page = fs.readFileSync(path.join(UI, mode === "board" ? "board.html" : "mailbox.html"), "utf8");
document.getElementById("empty").textContent = page.match(/<div id="empty">([^<]*)<\/div>/)[1];
for (const f of ["pager.js", mode === "board" ? "board.js" : "mailbox.js"]) {
  vm.runInContext(fs.readFileSync(path.join(UI, f), "utf8"), ctx, { filename: f });
}

const tick = () => new Promise((r) => setTimeout(r, 0));
const cards = () => byId.list.all((e) => /\bletter\b/.test(e.className));
const pagerText = () => (byId.pager.style.display === "none" ? null : byId.pager.textContent);
const buttons = () => byId.pager.children;
const snap = () => ({ cards: cards().map((c) => c.textContent), classes: cards().map((c) => c.className), empty: byId.empty.style.display === "none" ? null : byId.empty.textContent, pager: pagerText(), prevDisabled: buttons()[0].disabled, nextDisabled: buttons()[2].disabled });

(async () => {
  const out = {};
  await tick();
  if (mode === "board") {
    out.asked = invoked.slice();
    const send = (p) => listeners["board-page"]({ payload: p });
    const item = (id, name, line, at, status) => ({ id, name, line, at, status });
    const at = Math.floor(new Date(2026, 10, 9, 12).getTime() / 1000);
    send({ ok: true, reason: "", items: [item(9, "김**", "<b>펭귄</b> 더 주세요", at, "done"), item(8, "", "고마워요", at, "rejected"), item(7, "*", "그냥", at, "none")], page: 1, pages: 3, total: 13 });
    out.first = snap();
    invoked.length = 0;
    buttons()[2].fire("click");
    out.next = { asked: invoked.slice(), ...snap() };
    send({ ok: true, reason: "", items: [item(1, "a*", "끝", at, "none")], page: 3, pages: 3, total: 13 });
    out.last = snap();
    send({ ok: true, reason: "", items: [], page: 1, pages: 1, total: 0 });
    out.empty = snap();
    send({ ok: false, reason: "offline", items: [], page: 1, pages: 1, total: 0 });
    out.offline = snap();
    send({ ok: false, reason: "old", items: [], page: 1, pages: 1, total: 0 });
    out.old = snap();
    send({ ok: false, reason: "timeout", items: [], page: 1, pages: 1, total: 0 });
    out.timeout = snap();
    invoked.length = 0;
    listeners["board-open"]();
    out.reopen = invoked.slice();
  } else {
    const letter = (id, extra = {}) => ({ id, at: 1_800_000_000 + id, from: "ab".repeat(32), read: true, done: false, rejected: false, ok: true, text: `편지 ${id}`, name: "단니", species: "cat", stage: "", style: "", version: "0.6.3", os: "windows", ...extra });
    const send = (ls) => listeners["mailbox-state"]({ payload: ls });
    const twelve = Array.from({ length: 12 }, (_, i) => letter(12 - i));
    send(twelve);
    out.first = snap();
    buttons()[2].fire("click");
    buttons()[2].fire("click");
    out.third = snap();
    // 3쪽의 두 통을 지웠다: 한 쪽 앞으로
    send(twelve.slice(0, 10));
    out.afterDelete = snap();
    // 2쪽에서 하나를 읽었다: 그 쪽 그대로
    send(twelve.slice(0, 10).map((l) => (l.id === 6 ? { ...l, read: false } : l)));
    out.afterRead = snap();
    // 반영 / 반려: 하나만
    const card = cards()[0];
    const [check, reject] = card.all((e) => e.tag === "input");
    invoked.length = 0;
    check.checked = true;
    check.fire("change");
    reject.checked = true;
    reject.fire("change");
    out.status = { invoked: invoked.slice(), done: check.checked, rejected: reject.checked };
    invoked.length = 0;
    reject.checked = false;
    reject.fire("change");
    out.statusOff = { invoked: invoked.slice(), done: check.checked, rejected: reject.checked };
    send([letter(1, { rejected: true }), letter(2, { done: true })]);
    out.marked = snap();
    send([]);
    out.none = snap();
  }
  console.log(JSON.stringify(out));
})();
