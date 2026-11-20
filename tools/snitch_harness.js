// ui/snitch.js(🟡 골든 스니치)를 가짜 캔버스·rAF 위에서 돌려 보는 도구 (브라우저 없이, tools/test_snitch_ui.py가 쓴다).
// node tools/snitch_harness.js → 결과 JSON. 실제 그림은 보지 못한다 — 프레임 수, 남은 스니치·반짝이 수, 그린 동그라미 수만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "snitch.js"), "utf8");

/** 가짜 창 하나에 snitch.js를 올림 */
function load() {
  const counts = {};
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        return (...args) => {
          counts[name] = (counts[name] || 0) + 1;
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
    arcs: () => counts.arc || 0,
    alive: () => vm.runInContext("snitches.length", sandbox),
    bursts: () => vm.runInContext("snBursts.length", sandbox),
    at: () => vm.runInContext("snitches.map((s) => [s.x, s.y])", sandbox),
  };
}

const sn = (extra = {}) => ({ x: 500, y: 300, size: 20, caught: false, ...extra });
const out = {};
let h = load();
out.listeners = h.listeners();
out.idle = h.rafs();
// 이상한 값은 버리고, 많아도 4개까지
h.send("trail-snitch", [sn({ x: NaN }), sn({ size: 0 }), sn({ size: 9999 }), null, sn()]);
out.filtered = h.alive();
h = load();
h.send("trail-snitch", Array.from({ length: 9 }, (_, i) => sn({ x: 100 * i })));
out.capped = h.alive();
// 나는 동안 프레임을 돌며 받은 자리로 따라감
h = load();
h.send("trail-snitch", [sn()]);
h.send("trail-snitch", [sn({ x: 600 })]);
out.moving_frames = h.frames(30);
out.followed = h.at()[0][0];
out.arcs = h.arcs();
// 그냥 사라지면 반짝이 없이 바로 멈춤
h.send("trail-snitch", []);
out.after_gone_frames = h.frames(100);
out.rafs_after_gone = h.rafs();
// 잡힌 채 사라지면 반짝 터지고 잠깐 뒤 멈춤
h = load();
h.send("trail-snitch", [sn({ caught: true })]);
h.frames(5);
h.send("trail-snitch", []);
out.bursts = h.bursts();
out.burst_frames = h.frames(500);
out.rafs_end = h.rafs();
out.bursts_end = h.bursts();
process.stdout.write(JSON.stringify(out));
