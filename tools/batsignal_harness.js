// ui/batsignal.js(🦇 박쥐 신호 빛줄기)를 가짜 캔버스·rAF 위에서 돌려 보는 도구 (브라우저 없이, tools/test_batsignal_ui.py가 쓴다).
// node tools/batsignal_harness.js → 결과 JSON. 실제 그림은 보지 못한다 — 프레임 수, 남은 신호 수, 채운 도형 수만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "batsignal.js"), "utf8");

/** 가짜 창 하나에 batsignal.js를 올림 */
function load() {
  const counts = {};
  const ctx = new Proxy(
    {},
    {
      get(_, name) {
        if (name === "createLinearGradient") return () => ({ addColorStop() {} });
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
    setTimeout() {},
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
    /** 걸린 프레임이 없어질 때까지 (최대 n) — 돈 프레임 수 */
    settle(n = 5000) {
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
    fills: () => counts.fill || 0,
    alive: () => vm.runInContext("signals.length", sandbox),
  };
}

const sig = (extra = {}) => ({ x: 500, y: 900, size: 110, secs: 5, pixel: false, age: 0, ...extra });
const out = {};
let h = load();
out.listeners = h.listeners();
out.idle = h.rafs();
// 이상한 값은 버리고, 많이 와도 4개까지
h.send("trail-signal", [sig({ x: NaN }), sig({ size: -1 }), sig({ secs: 999 }), sig({ age: 99 }), null, sig()]);
out.filtered = h.alive();
h = load();
h.send("trail-signal", Array.from({ length: 9 }, () => sig()));
out.capped = h.alive();
// 비추는 동안만 프레임 (5초 + 흐려지기 0.8초), 그 뒤엔 멈춤
h = load();
h.send("trail-signal", [sig()]);
out.frames = h.settle();
out.fills = h.fills();
out.rafs_after = h.rafs();
out.alive_end = h.alive();
process.stdout.write(JSON.stringify(out));
