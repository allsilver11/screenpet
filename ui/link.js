// 📱 내 다른 컴퓨터 연결 창: 이 컴의 연결 코드를 크게 보이고, 저쪽 컴퓨터가 보여 주는 코드를 받는다.
// 백엔드가 `link-state`({name, code, phase, note})로 상태를 알리고, 코드는 `link_submit`으로 보낸다 (꼴 확인도 백엔드가 한다).
// phase: "ready" | "waiting" | "done" | "failed" | "blocked" (이미 다른 컴퓨터와 연결됨)
//        | "confirm" (코드가 맞았다: "본인의 PC가 맞습니까?" 확인/취소) | "confirmed" (확인함, 저쪽을 기다림) | "cancelled"
// 확인/취소는 `link_confirm`({yes}), 창 닫기(Esc·X)는 `link_close` — 확인 단계면 백엔드가 취소한다
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const titleEl = document.getElementById("title");
const askEl = document.getElementById("ask");
const whoEl = document.getElementById("who");
const mineBox = document.getElementById("mineBox");
const codeEl = document.getElementById("code");
const hintEl = document.getElementById("hint");
const inputEl = document.getElementById("input");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");

let view = { name: "", code: "", phase: "ready", note: "" };

function asking() {
  return view.phase === "confirm" || view.phase === "confirmed";
}

function show(v) {
  view = v || view;
  const phase = view.phase;
  const ask = asking();
  titleEl.textContent = ask ? "본인의 PC가 맞습니까?" : `📱 "${view.name}" 컴퓨터를 내 다른 컴퓨터로`;
  titleEl.className = ask ? "ask" : "";
  askEl.hidden = !ask;
  askEl.textContent = "스크린펫의 모든 데이터가 공유됩니다.";
  whoEl.hidden = !ask;
  whoEl.textContent = `저쪽 컴퓨터: "${view.name}"`;
  codeEl.textContent = view.code || "";
  mineBox.hidden = ask || !view.code;
  const typing = phase === "ready" || phase === "failed" || phase === "waiting";
  hintEl.hidden = !typing;
  inputEl.hidden = !typing;
  okBtn.hidden = !(typing || ask);
  okBtn.textContent = ask ? "확인" : "연결";
  okBtn.disabled = phase === "waiting" || phase === "confirmed";
  cancelBtn.textContent = ask ? "취소" : "닫기";
  noteEl.textContent = view.note || "";
  noteEl.className = phase === "failed" || phase === "blocked" || phase === "cancelled" ? "bad" : "";
  if (phase === "done") inputEl.value = "";
  if (typing && phase !== "waiting") inputEl.focus();
}

function submit() {
  if (view.phase === "waiting" || inputEl.hidden || asking()) return;
  const code = inputEl.value.trim();
  if (!code) {
    inputEl.focus();
    return;
  }
  invoke("link_submit", { code });
}

okBtn.addEventListener("click", () => {
  if (view.phase === "confirm") {
    // 한 번만: 백엔드가 "confirmed"로 바꿀 때까지도 다시 누르지 않는다
    okBtn.disabled = true;
    invoke("link_confirm", { yes: true });
  } else if (!asking()) submit();
});
cancelBtn.addEventListener("click", () => (asking() ? invoke("link_confirm", { yes: false }) : invoke("link_close")));

setupFormKeys({ submit, close: () => invoke("link_close") });

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("link-state", (e) => show(e.payload));
  show(await invoke("link_state"));
})();
