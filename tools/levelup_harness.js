// ui/common.js + ui/main.js(또는 house.js)를 가짜 DOM 위에서 그대로 돌려 성장 축하를 적는다 (브라우저 없이, tools/test_levelup_ui.py가 쓴다).
// node tools/levelup_harness.js main|main-guest|house → 결과 JSON.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const UI = path.join(__dirname, "..", "ui");
const mode = process.argv[2];
const houseWindow = mode === "house" || mode === "house-pixel";
const page = houseWindow ? "house.js" : "main.js";

class El {
  constructor(id) {
    this.id = id;
    this.style = { setProperty: (k, v) => (this.style[k] = v) };
    this.children = [];
    this.parentNode = null;
    this.dataset = {};
    this.hidden = false;
    this.clientWidth = 128;
    this.clientHeight = 164;
    this.offsetWidth = 128;
    this.classes = new Set();
    this.handlers = {};
    this.textContent = "";
    const self = this;
    this.classList = { add: (...c) => c.forEach((x) => self.classes.add(x)), remove: (...c) => c.forEach((x) => self.classes.delete(x)), contains: (c) => self.classes.has(c) };
  }
  set innerHTML(_) {
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
  insertBefore(x) {
    this.append(x);
  }
  replaceChildren() {}
  after() {}
  remove() {
    if (this.parentNode) this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
    this.parentNode = null;
  }
  toggleAttribute(name, on) {
    if (name === "hidden") this.hidden = on;
  }
  setAttribute() {}
  addEventListener(ev, f) {
    this.handlers[ev] = f;
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, width: 34, height: 26, bottom: 26, right: 34 };
  }
  querySelector() {
    return new El("q");
  }
  querySelectorAll() {
    return [];
  }
}

const els = new Map();
const byId = (id) => {
  if (!els.has(id)) els.set(id, new El(id));
  return els.get(id);
};
byId("pet").append(byId("nightcap"), byId("focusring"));

const listeners = {};
let now = 0;
let raf = null;
const timers = new Map();
let nextTimer = 1;
const visibility = { state: "visible" };
const decoding = [];
let holdDecode = false;
const houseStateOf = (stage, housed = true) => ({ pet: "cat", stage, scale: stage === "adult" ? 1.15 : 1, hungry: false, housed, style: mode === "house-pixel" ? "pixel" : "round" });
const slotInfo = { pet: "cat", stage: "teenager", scale: 1, role: mode === "main-guest" ? "guest" : "own", owner: "친구", render: null };
const sandbox = {
  window: {
    innerWidth: 128,
    innerHeight: 164,
    addEventListener() {},
    __TAURI__: {
      core: { invoke: async (cmd) => (cmd === "slot_info" ? slotInfo : cmd === "house_state" ? houseStateOf("teenager") : null) },
      webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (ev, f) => (listeners[ev] = f), label: "pet" }) },
    },
  },
  document: {
    getElementById: byId,
    createElement: (t) => new El(t),
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
    decode() {
      if (!holdDecode) return Promise.resolve();
      return new Promise((r) => decoding.push(r));
    }
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
for (const f of ["common.js", page]) vm.runInContext(fs.readFileSync(path.join(UI, f), "utf8"), sandbox, { filename: f });

const emit = (ev, payload) => listeners[ev]({ payload });
const render = (anim, housed = false) => emit("pet-render", { anim, flip: false, heart: false, housed });
const tick = async () => {
  for (let i = 0; i < 40; i++) await new Promise((r) => setImmediate(r));
};

function mainBits() {
  return byId("stage").children.filter((c) => c.className === "burst-bit");
}

async function runMain() {
  const out = {};
  emit("pet-presence", { nightcap: true, ring: null });
  await tick();
  render("level_up", true); // 터지지 않는 같은 칸의 모자 자리
  raf(now);
  out.capBefore = byId("nightcap").style.transform;
  render("eat");
  out.beforeLevelUp = mainBits().length;
  render("level_up");
  out.onLevelUp = mainBits().length;
  out.glyphs = mainBits().map((b) => b.textContent);
  out.delays = mainBits().map((b) => parseFloat(b.style.animationDelay));
  out.dragEcho = (render("dragged"), render("level_up"), mainBits().length);
  out.petChildren = byId("pet").children.map((c) => c.id);
  raf(now);
  out.capAfter = byId("nightcap").style.transform;
  out.capHidden = byId("nightcap").hidden;
  const first = mainBits()[0];
  first.handlers.animationend();
  out.afterOneEnded = mainBits().length;
  for (const [id, t] of [...timers]) {
    timers.delete(id);
    if (t.ms >= 1000 && t.ms <= 2000) t.f();
  }
  out.afterFallback = mainBits().length;
  now += 6000;
  render("idle");
  render("level_up", true);
  out.housed = mainBits().length;
  render("idle");
  visibility.state = "hidden";
  render("level_up");
  out.hidden = mainBits().length;
  visibility.state = "visible";
  render("idle");
  now += 6000;
  render("level_up");
  out.nextLevelUp = mainBits().length;
  return out;
}

async function runHouse() {
  const out = {};
  await tick();
  const house = byId("house");
  byId("picker").hidden = true;
  out.artBefore = house.style.backgroundImage;
  render("level_up", false);
  out.hopOutside = house.classes.has("house-hop");
  render("idle", false);
  render("level_up", true);
  out.hopHoused = house.classes.has("house-hop");
  out.bits = byId("house").children.filter((c) => c.className === "burst-bit").length;
  house.handlers.animationend({ target: house });
  out.hopEnded = house.classes.has("house-hop");
  render("level_up", true);
  out.hopSameCelebration = house.classes.has("house-hop");
  // 새 단계: 그림을 받아 풀기 전엔 옛 그림을 그대로 둔다
  holdDecode = true;
  emit("house-state", houseStateOf("adult"));
  out.waiting = 0;
  out.artWhileLoading = [];
  for (let i = 0; i < 5; i++) {
    await tick();
    if (!decoding.length) break;
    out.waiting += decoding.length;
    out.artWhileLoading.push(house.style.backgroundImage);
    while (decoding.length) decoding.shift()();
  }
  out.artAfter = house.style.backgroundImage;
  out.houseWidth = house.style.width;
  out.doorWidth = byId("door").style.width;
  out.doorPetWidth = byId("door-pet").style.width;
  out.doorPetLeft = byId("door-pet").style.left;
  out.dot = sandbox.document.body.style["--dot"];
  out.pixelHouse = sandbox.document.body.classes.has("pixel-house");
  // 말풍선
  emit("pet-say", "level_up");
  out.bubble = byId("bubble").textContent;
  out.bubbleHidden = byId("bubble").hidden;
  return out;
}

(houseWindow ? runHouse() : runMain())
  .then((out) => console.log(JSON.stringify(out)))
  .catch((e) => {
    console.error(e);
    process.exit(1);
  });
