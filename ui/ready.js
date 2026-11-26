// 🔄 떠 있는 창(펫·손님·하우스·물건·탈것·날씨·구름·썰매)이 앱 알림을 다 듣기 시작했으면 앱에 알림 (page_ready — src-tauri/src/pages.rs).
// 처음 뜰 때와 🔄 새로고침·💤 잠금 풀기 뒤 다시 읽힐 때마다: 앱이 그동안 모아 둔 말풍선·💌 답장을 보내고 이 창의 지금 모습을 다시 보냄.
// 페이지 스크립트는 listen 대신 spOn(창, 이벤트, 처리)으로 들음 (이 파일이 없는 시험 하네스에선 그냥 listen) — 그 약속을 모아 두었다가 모든 스크립트가 돈 뒤(load) 다 끝나면 한 번 부름
window.spListening = [];
// 듣기를 시작하고 그 약속을 모아 둠 (돌려주는 것은 listen과 같음)
window.spOn = (target, event, handler) => {
  const p = target.listen(event, handler);
  window.spListening.push(p);
  return p;
};
window.addEventListener("load", () => {
  if (!window.__TAURI__) return;
  Promise.allSettled(window.spListening)
    .then(() => window.__TAURI__.core.invoke("page_ready"))
    .catch(() => {});
});
