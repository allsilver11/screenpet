// 📊 익명 펫 통계 — 처음 켤 때 한 번 묻는 창 (spec 2026-11-23-pet-stats). 답은 설정에 남고 ⚙️ 설정 체크로 바꿈.
// 답하기 전엔 아무것도 모으거나 보내지 않음. 닫기(X)는 답이 아님: 다음에 켤 때 다시 물음.
const { invoke } = window.__TAURI__.core;

// 답을 백엔드에 알림 (창은 백엔드가 숨김)
function answer(yes) {
  invoke("stats_answer", { yes });
}

document.getElementById("yes").addEventListener("click", () => answer(true));
document.getElementById("no").addEventListener("click", () => answer(false));
window.addEventListener("contextmenu", (e) => e.preventDefault());
