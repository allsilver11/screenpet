// ui/common.js + ui/main.js를 가짜 DOM 위에서 그대로 돌려 수면 모자·집중 고리가 움직임마다 어떻게 되는지 적는다 (브라우저 없이, tools/test_nightcap.py가 쓴다).
// node tools/nightcap_harness.js <동물> <단계> → 결과 JSON. 실제 그림은 그리지 않는다 — 어느 층의 자식인지, 어떤 변환을 받는지만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui");
const [pet, stageName] = process.argv.slice(2);

class El {
  constructor(id) {
    this.id = id;
    this.style = { setProperty() {} };
    this.children = [];
    this.parentNode = null;
    this.dataset = {};
    this.hidden = false;
    this.clientWidth = 128;
    this.clientHeight = 164;
    this.offsetWidth = 128;
    this.classes = new Set();
    const self = this;
    this.classList = { add: (...c) => c.forEach((x) => self.classes.add(x)), remove: (...c) => c.forEach((x) => self.classes.delete(x)), contains: (c) => self.classes.has(c) };
  }
  set innerHTML(_) {
    // #neck의 세 층
    this.children = ["nk-body", "nk-tile", "nk-head"].map((n) => {
      const c = new El(n);
      c.parentNode = this;
      return c;
    });
  }
  append(...xs) {
    for (const x of xs) {
      if (x.parentNode) x.parentNode.children = x.parentNode.children.filter((c) => c !== x);
      x.parentNode = this;
      this.children.push(x);
    }
  }
  appendChild(x) {
    this.append(x);
  }
  replaceChildren() {}
  after() {}
  remove() {}
  focus() {}
  toggleAttribute(name, on) {
    if (name === "hidden") this.hidden = on;
  }
  setAttribute() {}
  addEventListener() {}
  getBoundingClientRect() {
    return { left: 0, top: 0, width: 34, height: 26, bottom: 26, right: 34 };
  }
  querySelector() {
    return new El("q");
  }
  querySelectorAll() {
    return [];
  }
  getContext() {
    return { drawImage() {}, getImageData: () => ({ data: new Uint8ClampedArray(128 * 128 * 4) }) };
  }
}

const els = new Map();
const byId = (id) => {
  if (!els.has(id)) els.set(id, new El(id));
  return els.get(id);
};
// index.html과 같은 부모 — 테스트가 html도 따로 확인한다
const html = fs.readFileSync(path.join(UI, "index.html"), "utf8");
for (const id of ["nightcap", "focusring", "scarf"]) {
  const inPet = new RegExp(`<div id="pet">(?:(?!</div>)[\\s\\S])*<svg id="${id}"`).test(html);
  byId(inPet ? "pet" : "stage").append(byId(id));
}

const listeners = {};
const created = [];
let now = 0;
let raf = null;
const timers = new Map();
let nextTimer = 1;
const visibility = { state: "visible" };
const windowObj = {
  innerWidth: 128,
  innerHeight: 164,
  addEventListener() {},
  __TAURI__: {
    core: { invoke: async (cmd) => (cmd === "slot_info" ? { pet, stage: stageName, scale: 1, label: "pet" } : null) },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (ev, f) => (listeners[ev] = f), label: "pet" }) },
  },
};
const sandbox = {
  window: windowObj,
  document: {
    getElementById: byId,
    createElement: (t) => {
      const e = new El(t);
      created.push(e);
      return e;
    },
    addEventListener() {},
    get visibilityState() {
      return visibility.state;
    },
    body: new El("body"),
  },
  performance: { now: () => now },
  requestAnimationFrame: (f) => (raf = f),
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
  fetch: async (url) => ({ json: async () => JSON.parse(fs.readFileSync(path.join(UI, url), "utf8")) }),
  navigator: {},
  console,
  Math,
  JSON,
  Promise,
  Map,
  Set,
  Uint8Array,
};
vm.createContext(sandbox);
for (const f of ["common.js", "main.js"]) vm.runInContext(fs.readFileSync(path.join(UI, f), "utf8"), sandbox, { filename: f });

const emit = (ev, payload) => listeners[ev]({ payload });
const step = (ms = 1) => {
  now += ms;
  const f = raf;
  raf = null;
  f(now);
};
const look = () => ({
  capParent: byId("nightcap").parentNode.id,
  ringParent: byId("focusring").parentNode.id,
  cap: byId("nightcap").style.transform,
  ring: byId("focusring").style.transform,
  capHidden: byId("nightcap").hidden,
  capClasses: [...byId("nightcap").classes],
  scarf: byId("scarf").style.transform,
  scarfParent: byId("scarf").parentNode.id,
  scarfHidden: byId("scarf").hidden,
  scarfClasses: [...byId("scarf").classes],
  scarfView: byId("scarf").dataset.view,
  pet: byId("pet").style.transform,
  petOrigin: byId("pet").style.transformOrigin,
  sparks: created.filter((e) => e.className === "spark").length,
});
const flushTimers = () => {
  for (const [id, t] of [...timers]) {
    timers.delete(id);
    t.f();
  }
};

(async () => {
  for (let i = 0; i < 40 && !raf; i++) await new Promise((r) => setImmediate(r));
  const out = {};
  const show = (name, render, { width } = {}) => {
    emit("pet-render", { anim: "idle", flip: false, heart: false, ...render });
    if (width) {
      windowObj.innerWidth = width;
      byId("stage").clientWidth = width;
    }
    step();
    out[name] = look();
    windowObj.innerWidth = 128;
    byId("stage").clientWidth = 128;
  };
  // 켜기 전에는 효과 없이 숨어 있다
  emit("pet-presence", { nightcap: false, ring: null });
  out.startOff = look();
  emit("pet-presence", { nightcap: true, ring: 40 });
  show("base", {});
  // 모습마다 (목도리 모양)
  show("sleep", { anim: "sleep" });
  show("sitFront", { anim: "sit_front" });
  show("climb", { anim: "climb" });
  show("sled", { anim: "roll" });
  show("hangRight", { hang: 1 }, { width: 400 });
  show("hangLeft", { hang: -1 }, { width: 400 });
  show("wobble", { wobble: true });
  show("bubbleWindow", {}, { width: 300 });
  show("noteWindow", {}, { width: 520 });
  show("flip", { flip: true });
  show("poofGone", { poof: true, anim: "wave" });
  show("base2", {});
  show("roll", { anim: "roll" });
  now += 300;
  show("roll2", { anim: "roll" });
  show("sting", { anim: "sting" });
  show("stomp", { anim: "stomp" });
  show("hop", { anim: "hop" });
  show("bigjump", { anim: "bigjump" });
  show("unknownAnim", { anim: "no_such_anim" });
  now += 1000;
  show("neck", { anim: "neckstretch" });
  now += 1000;
  show("neckMid", { anim: "neckstretch" });
  show("afterNeck", {});
  // 아침 8시 30분: 뿅
  emit("pet-presence", { nightcap: false, ring: null });
  step();
  out.morning = look();
  flushTimers();
  step();
  out.morningDone = look();
  // 창이 숨어 있으면 효과 없이 바로 벗는다
  emit("pet-presence", { nightcap: true, ring: null });
  step();
  visibility.state = "hidden";
  emit("pet-presence", { nightcap: false, ring: null });
  step();
  out.hiddenWindow = look();
  console.log(JSON.stringify(out));
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
