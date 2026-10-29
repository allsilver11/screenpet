// 친구 코드 창: 내 코드(복사 버튼) + 친구의 코드 입력 → "친구 신청" (짝맞추기 창이 이어 받는다).
// 백엔드가 상태를 `code-state`({code, connected, enabled})로 보내고, 신청은 `code_submit`으로 한다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const codeEl = document.getElementById("code");
const copyBtn = document.getElementById("copy");
const renewBtn = document.getElementById("renew");
const statusEl = document.getElementById("status");
const inputEl = document.getElementById("input");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");

let state = { code: null, connected: false, enabled: false };

function show(v) {
  state = v;
  codeEl.textContent = v.code ?? "—";
  copyBtn.disabled = !v.code;
  renewBtn.disabled = !v.connected;
  okBtn.disabled = !v.connected;
  statusEl.textContent = !v.enabled
    ? "인터넷 친구를 쓰지 않도록 설정되어 있어요"
    : v.connected
      ? "친구에게 받은 코드를 넣어 주세요"
      : "인터넷에 연결되지 않았어요 (다시 연결하는 중…)";
}

// 백엔드 normalize_code와 같은 규칙 (대소문자·띄어쓰기·- 무시, O→0, I/L→1, U 없음)
function looksLikeCode(s) {
  const t = s.toUpperCase().replace(/[\s-]/g, "").replace(/O/g, "0").replace(/[IL]/g, "1");
  return /^[0-9ABCDEFGHJKMNPQRSTVWXYZ]{8}$/.test(t);
}

async function submit() {
  const code = inputEl.value;
  if (!looksLikeCode(code)) {
    noteEl.textContent = "코드 8자리를 확인해 주세요 (예: ABCD-1234)";
    inputEl.focus();
    return;
  }
  try {
    await invoke("code_submit", { code });
    inputEl.value = "";
    noteEl.textContent = "";
  } catch {
    noteEl.textContent = "코드 8자리를 확인해 주세요 (예: ABCD-1234)";
  }
}

copyBtn.addEventListener("click", async () => {
  if (!state.code) return;
  const ok = await copyText(state.code);
  copyBtn.textContent = ok ? "복사했어요" : "복사 실패";
  setTimeout(() => (copyBtn.textContent = "복사"), 1500);
});
// 새 친구 코드 (코드를 너무 널리 알렸을 때) — 새 코드는 code-state로 온다
// 한 번 더 눌러야 바뀐다 (지금 코드는 더 이상 쓸 수 없게 된다)
let renewArmed = null;
renewBtn.addEventListener("click", () => {
  if (!state.connected) return;
  if (!renewArmed) {
    renewBtn.textContent = "정말 바꿀까요?";
    renewArmed = setTimeout(() => {
      renewArmed = null;
      renewBtn.textContent = "새 코드";
    }, 3000);
    return;
  }
  clearTimeout(renewArmed);
  renewArmed = null;
  renewBtn.textContent = "새 코드";
  invoke("code_renew");
  codeEl.textContent = "…";
});
okBtn.addEventListener("click", submit);
cancelBtn.addEventListener("click", () => invoke("code_close"));
inputEl.addEventListener("input", () => (noteEl.textContent = ""));

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Enter") submit();
  else if (e.key === "Escape") invoke("code_close");
  // 새로 고침 같은 웹뷰 기본 동작만 막는다 (입력칸 타자는 그대로)
  else if (e.key === "F5" || (e.ctrlKey && (e.key === "r" || e.key === "R"))) e.preventDefault();
});

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("code-state", (e) => {
    show(e.payload);
    inputEl.focus();
  });
  show(await invoke("code_state"));
  inputEl.focus();
})();
