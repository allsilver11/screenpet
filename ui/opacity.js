// 👻 전체 투명도: 떠 있는 창(펫·손님·하우스·물건·소품·날씨·무지개·썰매·탈것 등)이 다른 스크립트보다 먼저 읽음.
// 처음 값은 창을 만들 때 넣어 준 window.__SCREENPET_OPACITY__ (overlay.rs finish), 없으면 app_opacity로 물음.
// 그 뒤로는 `app-opacity` 이벤트 (0.2~1.0)로 문서 루트 opacity를 바꿈. 클릭 판정은 그대로임.
// 일반 스크립트끼리 최상위 이름이 겹치지 않게 함수 하나로 감쌈
(() => {
  // 문서 루트에 투명도를 씀 (숫자가 아니면 그대로 둠)
  const apply = (value) => {
    const v = Number(value);
    if (Number.isFinite(v) && v > 0) document.documentElement.style.opacity = String(Math.min(v, 1));
  };
  if (window.__SCREENPET_OPACITY__ !== undefined) apply(window.__SCREENPET_OPACITY__);
  const tauri = window.__TAURI__;
  if (!tauri) return;
  (async () => {
    // 듣기 시작한 뒤에 지금 값을 물음 (그 사이 바뀐 값을 놓치지 않게)
    await tauri.webviewWindow.getCurrentWebviewWindow().listen("app-opacity", (e) => apply(e.payload));
    apply(await tauri.core.invoke("app_opacity"));
  })().catch(() => {});
})();
