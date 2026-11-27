// ui/stats.js(📊 펫 통계 창)를 가짜 DOM 위에서 그대로 돌려 그린 표 글자를 적는다 (브라우저 없이, tools/test_pet_stats_ui.py가 쓴다).
// node tools/stats_harness.js '<stats-days JSON>' [기간 일수] → {totals, chart, daily, legend} JSON (요소마다 [태그, 글자, 자식들])

const fs = require("fs");
const path = require("path");
const vm = require("vm");
const { El, elementStore } = require("./fake_dom");

const UI = path.join(__dirname, "..", "ui");
const reply = JSON.parse(process.argv[2]);
const byId = elementStore();
byId("days").value = process.argv[3] || "30";
const listeners = {};
const sandbox = {
  window: {
    addEventListener() {},
    __TAURI__: {
      core: { invoke: async () => null },
      webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (ev, f) => (listeners[ev] = f) }) },
    },
  },
  document: { getElementById: byId, createElement: (t) => new El(t) },
  console,
  Number,
  Math,
  Object,
  String,
  JSON,
};
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(UI, "stats.js"), "utf8"), sandbox);

// 요소 나무를 [태그, 글자, 자식들]로 (그린 글자만 비교하게)
const dump = (e) => [e.id, e.textContent, e.children.map(dump)];

setTimeout(() => {
  listeners["stats-days"]({ payload: reply });
  const out = {};
  for (const id of ["totals", "chart", "daily", "empty"]) out[id] = dump(byId(id));
  process.stdout.write(JSON.stringify(out));
}, 0);
