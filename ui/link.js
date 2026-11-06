// 📱 내 다른 컴퓨터 연결 창: 이 컴의 연결 코드를 크게 보이고, 저쪽 컴퓨터가 보여 주는 코드를 받는다.
// 백엔드가 `link-state`({name, code, phase, note})로 상태를 알리고, 코드는 `link_submit`으로 보낸다 (꼴 확인도 백엔드가 한다).
// phase: "ready" | "waiting" | "done" | "failed" | "blocked" (이미 다른 컴퓨터와 연결됨)
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const titleEl = document.getElementById("title");
const mineBox = document.getElementById("mineBox");
const codeEl = document.getElementById("code");
const hintEl = document.getElementById("hint");
const inputEl = document.getElementById("input");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");

let view = { name: "", code: "", phase: "ready", note: "" };

function show(v) {
  view = v || view;
  const phase = view.phase;
  titleEl.textContent = `📱 "${view.name}" 컴퓨터를 내 다른 컴퓨터로`;
  codeEl.textContent = view.code || "";
  mineBox.hidden = !view.code;
  const typing = phase === "ready" || phase === "failed" || phase === "waiting";
  hintEl.hidden = !typing;
  inputEl.hidden = !typing;
  okBtn.hidden = !typing;
  okBtn.disabled = phase === "waiting";
  noteEl.textContent = view.note || "";
  noteEl.className = phase === "failed" || phase === "blocked" ? "bad" : "";
  if (phase === "done") inputEl.value = "";
  if (typing && phase !== "waiting") inputEl.focus();
}

function submit() {
  if (view.phase === "waiting" || inputEl.hidden) return;
  const code = inputEl.value.trim();
  if (!code) {
    inputEl.focus();
    return;
  }
  invoke("link_submit", { code });
}

okBtn.addEventListener("click", submit);
cancelBtn.addEventListener("click", () => invoke("link_close"));

setupFormKeys({ submit, close: () => invoke("link_close") });

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("link-state", (e) => show(e.payload));
  show(await invoke("link_state"));
})();
