// ui/<창>.html이 읽는 스크립트를 그 순서 그대로 가짜 DOM 위에서 돌린다 (브라우저 없이, tools/test_window_ui.py가 쓴다).
// 한 창의 스크립트는 한 전역을 나눠 쓰므로 최상위 이름이 겹치면 여기서도 SyntaxError가 난다.
// node tools/window_harness.js <시나리오> → 결과 JSON. 그림·CSS·실제 CSP는 재지 못한다.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui");
// 웹뷰에서 처리되지 않은 거부는 콘솔에 남을 뿐이다 (첫 로드 실패 시나리오). 여기서도 프로세스를 죽이지 않는다
process.on("unhandledRejection", () => {});
const tick = async () => {
  for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r));
};

function htmlScripts(html) {
  return [...fs.readFileSync(path.join(UI, html), "utf8").matchAll(/<script\s+src="([^"]+)"/g)].map((m) => m[1]);
}

function openWindow(html, { replies = {}, failFetch = () => false, globals = {} } = {}) {
  const page = fs.readFileSync(path.join(UI, html), "utf8");
  const els = new Map();
  const channels = {};
  const windowHandlers = {};
  const invoked = [];
  const copied = [];
  const timers = new Map();
  let nextTimer = 1;
  let raf = null;
  let now = 1000;
  // style 쓰기를 속성별로 센다
  const writes = {};
  const styleOf = (owner) =>
    new Proxy(
      { setProperty() {} },
      {
        set(t, k, v) {
          t[k] = v;
          (writes[owner] ??= {})[k] = (writes[owner][k] ?? 0) + 1;
          return true;
        },
      },
    );
  class El {
    constructor(id) {
      this.id = id;
      this.style = styleOf(id);
      this.dataset = {};
      this.children = [];
      this.handlers = {};
      this.hidden = false;
      this.disabled = false;
      this.value = "";
      this.textContent = "";
      this.className = "";
      this.clientWidth = 128;
      this.clientHeight = 164;
      this.offsetWidth = 128;
      this.classes = new Set();
      const self = this;
      this.classList = {
        add: (...c) => c.forEach((x) => self.classes.add(x)),
        remove: (...c) => c.forEach((x) => self.classes.delete(x)),
        toggle: (c, on) => (on ? self.classes.add(c) : self.classes.delete(c)),
        contains: (c) => self.classes.has(c),
      };
    }
    set innerHTML(_) {
      this.children = [new El("a"), new El("b"), new El("c")];
    }
    addEventListener(ev, f) {
      (this.handlers[ev] ??= []).push(f);
    }
    fire(ev, e = {}) {
      for (const f of this.handlers[ev] ?? []) f({ target: this, stopPropagation() {}, preventDefault() {}, ...e });
    }
    append(...xs) {
      this.children.push(...xs);
    }
    appendChild(x) {
      this.children.push(x);
    }
    after() {}
    remove() {}
    focus() {}
    select() {}
    insertBefore(x) {
      this.children.push(x);
    }
    replaceChildren(...xs) {
      this.children = xs;
      this.textContent = xs.map((x) => x.textContent).join("");
    }
    setAttribute() {}
    removeAttribute(a) {
      if (a === "hidden") this.hidden = false;
    }
    toggleAttribute() {}
    querySelectorAll() {
      return [];
    }
    getBoundingClientRect() {
      return { left: 0, top: 0, right: 128, bottom: 164, width: 128, height: 164 };
    }
    getContext() {
      return new Proxy({}, { get: () => () => ({ data: [] }), set: () => true });
    }
  }
  const byId = (id) => {
    if (!els.has(id)) {
      const el = new El(id);
      el.hidden = new RegExp(`id="${id}"[^>]*\\shidden`).test(page);
      els.set(id, el);
    }
    return els.get(id);
  };
  const sandbox = {
    document: {
      getElementById: byId,
      createElement: (t) => new El(t),
      createElementNS: (_, t) => new El(t),
      createTextNode: (t) => ({ textContent: t }),
      addEventListener() {},
      querySelector: byId,
      querySelectorAll: () => [],
      visibilityState: "visible",
      body: byId("body"),
      documentElement: byId("html"),
    },
    performance: { now: () => now },
    requestAnimationFrame: (f) => {
      raf = f;
      return 1;
    },
    cancelAnimationFrame() {},
    setTimeout: (f, ms) => {
      timers.set(nextTimer, { f, ms });
      return nextTimer++;
    },
    clearTimeout: (id) => timers.delete(id),
    setInterval: () => 0,
    clearInterval() {},
    Image: class {
      constructor() {
        this.width = 512;
        this.height = 3328;
      }
      async decode() {}
    },
    fetch: async (url) => {
      if (failFetch(url)) throw new Error(`no ${url}`);
      return { json: async () => JSON.parse(fs.readFileSync(path.join(UI, url), "utf8")) };
    },
    navigator: {
      clipboard: {
        writeText: async (t) => {
          copied.push(t);
        },
      },
    },
    location: { search: "" },
    URLSearchParams,
    console,
    Math,
    JSON,
    Promise,
    Map,
    Set,
    Array,
    Object,
    Float32Array,
    Uint8Array,
    devicePixelRatio: 1,
    innerWidth: 128,
    innerHeight: 164,
    scrollY: 0,
    scrollTo() {},
    addEventListener: (ev, f) => (windowHandlers[ev] ??= []).push(f),
    __TAURI__: {
      core: {
        invoke: async (cmd, args) => {
          invoked.push({ cmd, args });
          return replies[cmd] ?? null;
        },
      },
      webviewWindow: {
        getCurrentWebviewWindow: () => ({
          listen: async (ev, f) => {
            channels[ev] = f;
          },
        }),
      },
    },
    ...globals,
  };
  sandbox.window = sandbox;
  vm.createContext(sandbox);
  for (const f of htmlScripts(html)) vm.runInContext(fs.readFileSync(path.join(UI, f), "utf8"), sandbox, { filename: f });
  return {
    byId,
    sandbox,
    invoked,
    copied,
    writes,
    emit: (ev, payload) => channels[ev]({ payload }),
    rafPending: () => raf !== null,
    frame(ms = 1) {
      now += ms;
      const f = raf;
      raf = null;
      f(now);
    },
    // 창 전체에 온 키. 막혔는지(preventDefault) 돌려준다
    key(key, targetTag, extra = {}) {
      let blocked = false;
      const target = { tagName: targetTag };
      for (const f of windowHandlers.keydown ?? []) f({ key, target, preventDefault: () => (blocked = true), stopPropagation() {}, ...extra });
      return blocked;
    },
    menu() {
      let blocked = false;
      for (const f of windowHandlers.contextmenu ?? []) f({ preventDefault: () => (blocked = true) });
      return blocked;
    },
    calls: (cmd) => invoked.filter((c) => c.cmd === cmd).length,
    runTimers() {
      const due = [...timers.values()];
      timers.clear();
      due.forEach((t) => t.f());
    },
  };
}

const CAT = { pet: "cat", stage: "teenager", scale: 1, style: "round" };
const HOUSED = { ...CAT, hungry: false, housed: true };

const scenarios = {
  // 스크립트가 다 읽히는가 (최상위 이름 겹침, 문법 오류)
  async load([html]) {
    const replies = { slot_info: CAT, house_state: HOUSED, letter_state: { name: "친구", max: 80 }, postit_max: 80, name_state: { target: "me", current: "", fallback: "친구", max: 20 }, code_state: { code: "ABCD-1234", connected: true, enabled: true }, pair_state: {}, book_state: { sections: [], collected: 0, total: 0, shiny: 0 } };
    openWindow(html, { replies });
    await tick();
    return { scripts: htmlScripts(html) };
  },

  // 첫 그림 읽기에 실패해도 그리기 고리는 돌고, 다음 pet-stage로 살아난다
  async firstLoadFails() {
    let fail = true;
    const w = openWindow("index.html", { replies: { slot_info: CAT }, failFetch: () => fail });
    await tick();
    const running = w.rafPending();
    fail = false;
    w.emit("pet-stage", CAT);
    await tick();
    if (running) w.frame();
    return { running, drew: w.byId("pet").style.backgroundPosition ?? null };
  },
  async firstLoadFailsHouse() {
    let fail = true;
    const w = openWindow("house.html", { replies: { slot_info: CAT, house_state: HOUSED }, failFetch: () => fail });
    await tick();
    const running = w.rafPending();
    fail = false;
    w.emit("house-state", HOUSED);
    await tick();
    if (running) w.frame();
    return { running, drew: w.byId("door-pet").style.backgroundPosition ?? null };
  },

  // 말풍선 글자는 안쪽 span에 (긴 글은 span이 …로 자른다)
  async bubbleText() {
    const w = openWindow("index.html", { replies: { slot_info: CAT } });
    await tick();
    w.emit("pet-say-text", "off to 아주아주아주긴이름's~ brb!");
    const bubble = w.byId("bubble");
    return { tag: bubble.children[0]?.id, text: bubble.textContent, hidden: bubble.hidden };
  },

  // 스타일 쓰기: 칸이 그대로면 쓰지 않는다. 몸이 도는 동안은 프레임마다 쓴다
  async drawWrites() {
    const w = openWindow("index.html", { replies: { slot_info: { ...CAT, render: { anim: "idle", flip: false, heart: false } } } });
    await tick();
    const count = (k) => w.writes.pet?.[k] ?? 0;
    w.frame();
    const before = [count("backgroundPosition"), count("transform"), count("transformOrigin")];
    for (let i = 0; i < 5; i++) w.frame(1); // 5ms: 같은 칸
    const idle = [count("backgroundPosition") - before[0], count("transform") - before[1], count("transformOrigin") - before[2]];
    // 다음 칸까지 가면 쓴다
    const posBefore = count("backgroundPosition");
    for (let i = 0; i < 100; i++) w.frame(16);
    const advanced = count("backgroundPosition") - posBefore;
    // 구르기: 프레임마다 각도가 다르다
    w.emit("pet-render", { anim: "roll", flip: false, heart: false });
    w.frame(16);
    const spins = [];
    const tBefore = count("transform");
    for (let i = 0; i < 10; i++) {
      w.frame(16);
      spins.push(w.byId("pet").style.transform);
    }
    return { idle, advanced, spinWrites: count("transform") - tBefore, spinDistinct: new Set(spins).size, spin: spins[0] };
  },
  // 픽셀 구르기: 배로 미끄러지는 동물(펭귄·눈사람)은 roll 줄을 돌리지 않고 틀고, 구르기 그림이 없는 동물은 idle 줄 (역시 안 돈다)
  async pixelRoll([pet]) {
    const slot = { pet, stage: "adult", scale: 1, style: "pixel" };
    const w = openWindow("index.html", { replies: { slot_info: { ...slot, render: { anim: "roll", flip: false, heart: false } } } });
    await tick();
    w.emit("pet-render", { anim: "roll", flip: false, heart: false });
    const transforms = [];
    for (let i = 0; i < 20; i++) {
      w.frame(16);
      transforms.push(w.byId("pet").style.transform ?? "");
    }
    const [m, display] = vm.runInContext("[manifest, display]", w.sandbox);
    const pos = w.byId("pet").style.backgroundPosition;
    return { hasRoll: !!m.animations.roll, row: Math.round(-parseFloat(pos.split(" ")[1]) / display), rollRow: m.animations.roll?.row ?? null, idleRow: m.animations.idle.row, rotates: transforms.some((t) => t.includes("rotate")) };
  },
  async drawWritesHouse() {
    const w = openWindow("house.html", { replies: { slot_info: CAT, house_state: HOUSED } });
    await tick();
    w.frame();
    const before = w.writes["door-pet"].backgroundPosition;
    for (let i = 0; i < 5; i++) w.frame(1);
    return { idle: w.writes["door-pet"].backgroundPosition - before };
  },

  // animStart가 이번 프레임 시각보다 늦어도 (이벤트가 rAF 시각 뒤에 옴) 칸 번호는 0부터
  async frameIndex() {
    const w = openWindow("index.html", { replies: { slot_info: CAT } });
    const f = w.sandbox.frameIndex;
    return { loop: f({ fps: 10, frames: 4 }, -5), once: f({ fps: 10, frames: 4, loop: false }, -5), later: f({ fps: 10, frames: 4 }, 450) };
  },

  // 글 쓰는 창의 키: 버튼 위 Enter는 그 버튼만, 한글 조합 중 Enter는 글자 확정, Esc는 닫기
  async formKeys([html, submitCmd, closeCmd]) {
    const replies = { letter_state: { name: "친구", max: 80 }, postit_max: 80, name_state: { target: "me", current: "", fallback: "친구", max: 20 }, code_state: { code: "ABCD-1234", connected: true, enabled: true }, link_state: { name: "친구", code: "WXYZ-2345", phase: "ready", note: "" } };
    const w = openWindow(html, { replies });
    await tick();
    w.byId("input").value = "ABCD-1234";
    w.key("Enter", "BUTTON");
    await tick();
    const onButton = w.calls(submitCmd);
    w.key("Enter", "INPUT", { isComposing: true });
    await tick();
    const composing = w.calls(submitCmd);
    w.key("Enter", "INPUT");
    await tick();
    const onInput = w.calls(submitCmd);
    w.key("Escape", "INPUT");
    await tick();
    return { onButton, composing, onInput, closed: w.calls(closeCmd), f5: w.key("F5", "INPUT"), ctrlR: w.key("r", "INPUT", { ctrlKey: true }), typing: w.key("a", "INPUT"), menu: w.menu() };
  },
  async bookKeys() {
    const w = openWindow("book.html", { replies: { book_state: { sections: [], collected: 0, total: 0, shiny: 0 } } });
    await tick();
    return { f5: w.key("F5", "BODY"), menu: w.menu(), escape: w.key("Escape", "BODY") };
  },

  // 📱 코드가 맞은 뒤 "본인의 PC가 맞습니까?": 확인은 저쪽을 기다리고, 취소·Esc는 취소한다
  async linkConfirm() {
    const w = openWindow("link.html", { replies: { link_state: { name: "민수", code: "", phase: "confirm", note: "" } } });
    await tick();
    const look = () => ({
      title: w.byId("title").textContent,
      ask: w.byId("ask").hidden ? null : w.byId("ask").textContent,
      who: w.byId("who").hidden ? null : w.byId("who").textContent,
      ok: w.byId("ok").hidden ? null : w.byId("ok").textContent,
      okDisabled: w.byId("ok").disabled,
      cancel: w.byId("cancel").textContent,
      input: !w.byId("input").hidden,
      code: !w.byId("mineBox").hidden,
      note: w.byId("note").textContent,
    });
    const confirm = look();
    w.byId("ok").fire("click");
    await tick();
    const yes = w.invoked.filter((c) => c.cmd === "link_confirm").map((c) => c.args);
    w.emit("link-state", { name: "민수", code: "", phase: "confirmed", note: "저쪽 컴퓨터에서도 확인을 눌러 주세요" });
    const waiting = look();
    w.byId("ok").fire("click");
    w.byId("cancel").fire("click");
    await tick();
    const after = w.invoked.filter((c) => c.cmd === "link_confirm").map((c) => c.args);
    w.key("Escape", "BODY");
    await tick();
    w.emit("link-state", { name: "민수", code: "", phase: "cancelled", note: "연결을 취소했어요" });
    const cancelled = look();
    return { confirm, yes, waiting, after, closed: w.calls("link_close"), cancelled };
  },

  // 새 코드를 기다리는 동안 복사하면 이미 못 쓰게 된 옛 코드가 복사된다
  async renewCopy() {
    const w = openWindow("code.html", { replies: { code_state: { code: "OLD1-2345", connected: true, enabled: true } } });
    await tick();
    const renew = w.byId("renew");
    renew.fire("click");
    renew.fire("click");
    const copyDisabled = w.byId("copy").disabled;
    w.byId("copy").fire("click");
    await tick();
    const whilePending = [...w.copied];
    w.emit("code-state", { code: "NEW1-2345", connected: true, enabled: true });
    const copyDisabledAfter = w.byId("copy").disabled;
    w.byId("copy").fire("click");
    await tick();
    return { renewed: w.calls("code_renew"), copyDisabled, whilePending, copyDisabledAfter, copied: w.copied };
  },
};

scenarios[process.argv[2]](process.argv.slice(3)).then((r) => console.log(JSON.stringify(r)));
