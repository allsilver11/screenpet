// ui/whats_new.js(✨ 업데이트 소식 창)를 가짜 DOM 위에서 그대로 돌려 그린 것을 적는다 (브라우저 없이, tools/test_whats_new_ui.py가 쓴다).
// node tools/whats_new_harness.js '<[{version, text}] JSON>' → #notes 나무 JSON ([태그, 글자, 자식들])

const fs = require("fs");
const path = require("path");
const vm = require("vm");
const { El, elementStore } = require("./fake_dom");

const notes = JSON.parse(process.argv[2]);
const byId = elementStore();
const sandbox = {
  window: {
    addEventListener() {},
    __TAURI__: {
      core: { invoke: async (cmd) => (cmd === "whats_new_state" ? notes : null) },
      webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async () => {} }) },
    },
  },
  document: { getElementById: byId, createElement: (t) => new El(t) },
  String,
};
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(__dirname, "..", "ui", "whats_new.js"), "utf8"), sandbox);

const dump = (e) => [e.id, e.textContent, e.children.map(dump)];
setTimeout(() => process.stdout.write(JSON.stringify(dump(byId("notes")))), 10);
