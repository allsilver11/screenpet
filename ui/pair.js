// 짝맞추기 창: 상대 이름, 이모지 6개, "상대 화면에도 똑같은 그림이 보이나요?" + 버튼.
// 백엔드(게임 루프)가 상태를 `pair-state`로 보내고, 버튼은 `pair_answer`로 답한다 (어느 짝맞추기인지는 백엔드가 안다).
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const titleEl = document.getElementById("title");
const sasEl = document.getElementById("sas");
const noteEl = document.getElementById("note");
const buttonsEl = document.getElementById("buttons");
const yesBtn = document.getElementById("yes");
const noBtn = document.getElementById("no");

// 받침 있으면 a, 없으면 b (한글이 아니면 "a(b)")
function josa(word, a, b) {
  const c = word.charCodeAt(word.length - 1);
  if (c >= 0xac00 && c <= 0xd7a3) return word + ((c - 0xac00) % 28 ? a : b);
  return `${word}${a}(${b})`;
}

const FAILED = {
  unreachable: "연결하지 못했어요",
  declined: "친구 맺기가 취소되었어요",
  timeout: "시간이 지나 취소되었어요",
  busy: "지금은 친구 신청을 할 수 없어요",
  badcode: "그 친구 코드를 찾지 못했어요",
  offline: "인터넷 연결이 없거나 상대가 꺼져 있어요",
  selfcode: "내 친구 코드예요",
  already: "이미 친구예요",
};

function show(v) {
  const name = v.name || "친구";
  titleEl.textContent = v.incoming
    ? `${name}의 ${josa(v.pet || "동물", "이", "가")} 친구하고 싶어해요`
    : `${name}에게 친구 신청`;
  sasEl.textContent = (v.sas || []).join(" ");
  sasEl.hidden = !(v.sas && v.sas.length) || v.phase === "connecting";
  yesBtn.hidden = v.phase !== "confirm";
  noBtn.hidden = v.phase === "done" || v.phase === "failed";
  noBtn.textContent = v.phase === "confirm" ? "달라요 / 취소" : "취소";
  buttonsEl.hidden = yesBtn.hidden && noBtn.hidden;
  switch (v.phase) {
    case "connecting":
      noteEl.textContent = "연결하는 중…";
      break;
    case "confirm":
      noteEl.textContent = v.other_ok
        ? "상대는 똑같다고 했어요. 상대 화면에도 똑같은 그림이 보이나요?"
        : "상대 화면에도 똑같은 그림이 보이나요?";
      break;
    case "waiting":
      noteEl.textContent = "상대가 확인하기를 기다리는 중…";
      break;
    case "done":
      noteEl.textContent = "친구가 되었어요!";
      break;
    case "failed":
      noteEl.textContent = FAILED[v.reason] ?? "친구 맺기가 취소되었어요";
      break;
    default:
      noteEl.textContent = "";
  }
}

yesBtn.addEventListener("click", () => invoke("pair_answer", { accept: true }));
noBtn.addEventListener("click", () => invoke("pair_answer", { accept: false }));

window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  // 새로 고침 등 웹뷰 기본 동작은 막고, Esc는 취소
  if (e.key === "Escape") invoke("pair_answer", { accept: false });
  if (e.key !== "Tab" && e.key !== "Enter" && e.key !== " ") e.preventDefault();
});

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("pair-state", (e) => show(e.payload));
  show(await invoke("pair_state"));
})();
