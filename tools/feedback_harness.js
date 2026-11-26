// 💌 건의하기 창(feedback.js)을 웹뷰 없이 돌려 본다: 가짜 DOM·Tauri, 사진 줄이기(shrinkPhoto)는 손으로 끝냄.
// node feedback_harness.js → stdout JSON (test_feedback_ui.py가 본다)
const fs = require("fs");
const path = require("path");
const vm = require("vm");

class El {
  constructor(id) {
    this.id = id;
    this.style = {};
    this.handlers = {};
    this.disabled = false;
    this.value = "";
    this.textContent = "";
  }
  addEventListener(ev, fn) {
    (this.handlers[ev] = this.handlers[ev] || []).push(fn);
  }
  fire(ev, e = {}) {
    for (const f of this.handlers[ev] || []) f({ preventDefault() {}, stopPropagation() {}, ...e });
  }
  focus() {}
  click() {}
  removeAttribute(k) {
    delete this[k];
  }
}
const byId = {};
const document = { getElementById: (id) => (byId[id] = byId[id] || new El(id)) };
const win = new El("window");
const invoked = [];
// 줄이기를 기다리게 둠: pending[i](결과)로 끝냄
const pending = [];
win.__TAURI__ = {
  core: { invoke: async (cmd, args) => invoked.push([cmd, args || null]) },
  webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async () => {} }) },
};
const ctx = vm.createContext({
  window: win,
  document,
  console,
  setTimeout,
  Promise,
  Math,
  Blob,
  URL: { createObjectURL: () => "blob:test", revokeObjectURL() {} },
  shrinkPhoto: () => new Promise((done) => pending.push(done)),
  bytesToBase64: (b) => `b64:${b.length}`,
});
vm.runInContext(fs.readFileSync(path.join(__dirname, "..", "ui", "feedback.js"), "utf8"), ctx, { filename: "feedback.js" });
const tick = () => new Promise((r) => setTimeout(r, 0));
const paste = () => win.fire("paste", { clipboardData: { items: [{ kind: "file", type: "image/png", getAsFile: () => ({}) }] } });
const sends = () => invoked.filter(([c]) => c === "feedback_submit").map(([, a]) => a);
const ok = { bytes: new Uint8Array(3000), type: "image/webp", w: 640, h: 360 };

(async () => {
  const out = {};
  await tick();
  byId.input.value = "사진 보낼게요";
  paste();
  out.whileShrinking = { disabled: byId.ok.disabled, note: byId.note.textContent };
  byId.ok.fire("click");
  win.fire("keydown", { key: "Enter", ctrlKey: true });
  out.sentWhileShrinking = sends().length;
  // 두 번 붙임: 나중 것만 남음
  paste();
  pending[1](ok);
  await tick();
  pending[0]({ bytes: new Uint8Array(10), type: "image/jpeg", w: 10, h: 10 });
  await tick();
  out.afterShrink = { disabled: byId.ok.disabled, note: byId.note.textContent, thumb: byId["photo-box"].style.display };
  byId.ok.fire("click");
  out.sent = sends();
  // 보내는 중에 붙이면 무시
  paste();
  out.pastedWhileSending = pending.length;
  console.log(JSON.stringify(out));
})();
