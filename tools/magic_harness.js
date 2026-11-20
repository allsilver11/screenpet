// ui/magic.js(✨ 결투·빛줄기·나기니·사슴·서리)를 가짜 캔버스·rAF 위에서 돌려 보는 도구 (브라우저 없이, tools/test_magic_ui.py가 쓴다).
// node tools/magic_harness.js → 결과 JSON. 실제 그림은 보지 못한다 — 프레임 수, 남은 효과·흐려짐·펑 수, 그린 횟수만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "magic.js"), "utf8");

/** 가짜 창 하나에 magic.js를 올림 */
function load() {
  const counts = {};
  const grad = { addColorStop() {} };
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        return (...args) => {
          counts[name] = (counts[name] || 0) + 1;
          return name.startsWith("create") ? grad : undefined;
        };
      },
      set() {
        return true;
      },
    },
  );
  const canvas = { width: 0, height: 0, getContext: () => ctx };
  let rafs = [];
  let now = 0;
  const listeners = {};
  const sandbox = {
    document: { getElementById: () => canvas, documentElement: { style: {} } },
    window: {
      innerWidth: 1920,
      innerHeight: 1040,
      addEventListener() {},
      __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (name, f) => (listeners[name] = f) }) } },
    },
    performance: { now: () => now },
    requestAnimationFrame: (f) => rafs.push(f),
    location: { search: "" },
    URLSearchParams,
    Math,
    Number,
    Array,
    Object,
    console,
  };
  vm.runInNewContext(SRC, sandbox);
  return {
    listeners: () => Object.keys(listeners).sort(),
    send: (name, payload) => listeners[name]({ payload }),
    /** 프레임 n번 (걸린 것이 없으면 멈춤) — 돈 프레임 수 */
    frames(n) {
      let k = 0;
      while (rafs.length && k < n) {
        now += 16;
        const r = rafs;
        rafs = [];
        r.forEach((f) => f(now));
        k++;
      }
      return k;
    },
    rafs: () => rafs.length,
    count: (name) => counts[name] || 0,
    alive: () => vm.runInContext("mgFx.length", sandbox),
    ghosts: () => vm.runInContext("mgGhosts.length", sandbox),
    pops: () => vm.runInContext("mgPops.length", sandbox),
  };
}

const duel = (reach, light = "red") => ({ kind: "duel", red: [100, 300], green: [700, 300], clash: [400, 300], reach, width: 8, light });
const bolt = { kind: "bolt", from: [100, 100], to: [400, 200], color: "green", width: 6 };
const frost = { kind: "frost", x: 0, y: 0, size: 300, inward: [1, 1], level: 0.8 };
const stag = { kind: "stag", x: 300, y: 300, size: 120, dir: 1, alpha: 1 };
const snake = { kind: "snake", x: 500, y: 1000, size: 200, dir: -1, t: 1, alpha: 0.5 };
const out = {};
let h = load();
out.listeners = h.listeners();
out.idle = h.rafs();
// 이상한 값은 버리고, 많아도 MG_MAX까지
h.send("trail-magic", [{ kind: "nope", x: 1, y: 1 }, { ...bolt, from: [NaN, 0] }, { ...stag, size: 0 }, null, { ...frost, level: 0 }, stag]);
out.filtered = h.alive();
h.send("trail-magic", Array.from({ length: 40 }, () => stag));
out.capped = h.alive();
// 서리만이면 한 번만 그리고 프레임을 돌리지 않음
h = load();
h.send("trail-magic", [frost]);
out.frost_rafs = h.rafs();
out.frost_drawn = h.count("arc") > 0;
// 결투·사슴·뱀은 프레임을 돌림, 금빛 결투도
h = load();
h.send("trail-magic", [duel(1, "gold"), stag, snake, frost]);
out.moving_frames = h.frames(30);
out.ellipses = h.count("ellipse");
// 맞부딪혀 있던 결투가 사라지면 펑, 빛줄기는 흐려짐 → 잠깐 뒤 멈춤
h.send("trail-magic", [bolt]);
h.frames(3);
h.send("trail-magic", []);
out.pops = h.pops();
out.ghosts = h.ghosts();
out.end_frames = h.frames(500);
out.rafs_end = h.rafs();
out.pops_end = h.pops();
out.ghosts_end = h.ghosts();
// 아직 뻗는 중이던 결투가 사라지면 펑 없음
h = load();
h.send("trail-magic", [duel(0.5)]);
h.frames(2);
h.send("trail-magic", []);
out.no_pop = h.pops();
process.stdout.write(JSON.stringify(out));
