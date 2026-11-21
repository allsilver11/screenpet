// ui/magic.js(✨ 결투·빛줄기·나기니·사슴·서리·용알·🔥 모닥불)를 가짜 캔버스·rAF 위에서 돌려 보는 도구 (브라우저 없이, tools/test_magic_ui.py가 쓴다).
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
// 🥚 용알·🐉 아기 용 (해그리드): 이상한 값은 버리고 프레임을 돌림, 아기 용은 끝 무렵 불 퐁 (그라데이션)
const egg = { kind: "egg", x: 400, y: 900, size: 30, crack: 0.5 };
const baby = { kind: "hatchling", x: 400, y: 900, size: 50, t: 4.2, len: 5, dir: 1 };
h = load();
h.send("trail-magic", [egg, baby, { ...egg, crack: NaN }, { ...baby, len: 0 }]);
out.hagrid_filtered = h.alive();
out.hagrid_frames = h.frames(20);
out.hagrid_ellipses = h.count("ellipse");
out.hagrid_flame = h.count("createRadialGradient");
// 🔥 모닥불 (해그리드): 이상한 값은 버리고 프레임을 돌리며 불빛·돌·불꽃·불티를 그림, 꺼진 불은 장작만
const fire = { kind: "campfire", x: 600, y: 1000, size: 60, level: 1 };
h = load();
h.send("trail-magic", [fire, { ...fire, level: NaN }]);
out.fire_filtered = h.alive();
out.fire_frames = h.frames(20);
out.fire_glow = h.count("createRadialGradient");
out.fire_curves = h.count("quadraticCurveTo");
out.fire_arcs = h.count("arc");
h = load();
h.send("trail-magic", [{ ...fire, level: 0 }]);
h.frames(1);
out.out_glow = h.count("createRadialGradient");
out.out_rects = h.count("roundRect");
// 🐶 흰둥이 (짱구): 이상한 값은 버리고 프레임을 돌리며 몸·다리·머리를 그림, 갓 왔을 땐 뿅 (원)
const dog = { kind: "shiro", x: 500, y: 1000, size: 36, t: 0.2, len: 12, dir: -1 };
h = load();
h.send("trail-magic", [dog, { ...dog, len: 0 }, { ...dog, size: -1 }]);
out.shiro_filtered = h.alive();
out.shiro_frames = h.frames(10);
out.shiro_ellipses = h.count("ellipse");
out.shiro_arcs = h.count("arc");
// 🪄 날아간 지팡이 · 💡 루모스 불빛 (⚔️ 결투·마법사): 이상한 값은 버리고 프레임을 돌리며 지팡이 (선)·빛 번짐 (그라데이션)을 그림
const wand = { kind: "wand", from: [400, 900], to: [460, 1000], k: 0.5, size: 40 };
const glow = { kind: "glow", x: 300, y: 800, size: 90, level: 1 };
h = load();
h.send("trail-magic", [wand, glow, { ...wand, k: NaN }, { ...glow, size: 0 }]);
out.wizard_filtered = h.alive();
out.wizard_frames = h.frames(10);
out.wizard_glow = h.count("createRadialGradient");
out.wizard_turns = h.count("rotate");
// 🌑 녹스 연기: 이상한 값은 버리고 프레임을 돌리며 검은 연기 덩이 (그라데이션 원)를 그림
const puff = { kind: "puff", x: 300, y: 800, size: 40, k: 0.4 };
h = load();
h.send("trail-magic", [puff, { ...puff, k: NaN }, { ...puff, size: 0 }]);
out.puff_filtered = h.alive();
out.puff_frames = h.frames(10);
out.puff_gradients = h.count("createRadialGradient");
out.puff_arcs = h.count("arc");
process.stdout.write(JSON.stringify(out));
