// ui/trail.js를 가짜 캔버스·rAF 위에서 돌려 보는 도구 (브라우저 없이, tools/test_trail_ui.py가 쓴다).
// node tools/trail_harness.js → 결과 JSON. 실제 그림·GPU 비용은 재지 못한다 — 호출 수와 구름 수만.

const fs = require("fs");
const path = require("path");
const vm = require("vm");

const SRC = fs.readFileSync(path.join(__dirname, "..", "ui", "trail.js"), "utf8");

function load() {
  // 캔버스마다 따로 센다: 화면 캔버스(main)와 미리 그린 구름 그림(sprite)
  const counts = { main: {}, sprite: {} };
  const ctxFor = (kind) =>
    new Proxy(
      {},
      {
        get(_, name) {
          return (...args) => {
            counts[kind][name] = (counts[kind][name] || 0) + 1;
          };
        },
        set() {
          return true;
        },
      },
    );
  const canvas = (kind) => ({ width: 0, height: 0, getContext: () => ctxFor(kind) });
  let rafs = [];
  let now = 0;
  let listener = null;
  const sandbox = {
    document: { getElementById: () => canvas("main"), createElement: () => canvas("sprite"), documentElement: { style: {} } },
    window: {
      innerWidth: 1920,
      innerHeight: 1040,
      addEventListener() {},
      __TAURI__: { webviewWindow: { getCurrentWebviewWindow: () => ({ listen: (name, f) => name === "trail-puffs" && (listener = f) }) } },
    },
    performance: { now: () => now },
    requestAnimationFrame: (f) => rafs.push(f),
    setTimeout() {},
    location: { search: "" },
    URLSearchParams,
    Math,
    Map,
    console,
  };
  vm.runInNewContext(SRC, sandbox);
  return {
    counts,
    send: (list) => listener({ payload: list }),
    // 한 프레임 (16 ms): 이번 프레임의 화면 그리기 수
    frame() {
      now += 16;
      const before = { ...counts.main };
      const r = rafs;
      rafs = [];
      r.forEach((f) => f(now));
      const delta = (k) => (counts.main[k] || 0) - (before[k] || 0);
      return { draws: delta("drawImage"), clears: delta("clearRect") };
    },
    rafs: () => rafs.length,
    alive: () => vm.runInContext("puffs.length", sandbox),
  };
}

const puff = (i, extra = {}) => ({ x: 100 + i * 10, y: 500, size: 40, color: i % 7, pixel: false, dir: -1, age: 0, ...extra });

const out = {};

// 구름이 오기 전엔 아무것도 돌지 않는다
let h = load();
out.idle_rafs = h.rafs();

// 한꺼번에 많이 와도 상한까지만
h.send(Array.from({ length: 100 }, (_, i) => puff(i)));
out.max_alive = h.frame().draws;
// 다 흐려지면 마지막 프레임은 지우기만 하고, 다음 프레임을 걸지 않는다
let last = null;
for (let f = 0; f < 200 && h.rafs(); f++) last = h.frame();
out.alive_after = h.alive();
out.rafs_after = h.rafs();
out.cleared_last = !!last && last.draws === 0 && last.clears > 0;

// 웹뷰가 듣기 전에 모아 둔 구름: 반쯤 지난 것은 그리고, 수명이 지난 것은 버린다
h = load();
h.send([puff(0, { age: 0.5 }), puff(1, { age: 5 }), puff(2, { color: 99 })]);
out.aged_alive = h.alive();

// 두 스타일
for (const style of ["round", "pixel"]) {
  h = load();
  h.send(Array.from({ length: 7 }, (_, i) => puff(i, { pixel: style === "pixel" })));
  h.frame();
  out[`${style}_arcs`] = (h.counts.sprite.arc || 0) + (h.counts.sprite.ellipse || 0);
  out[`${style}_rects`] = h.counts.sprite.fillRect || 0;
}

process.stdout.write(JSON.stringify(out));
