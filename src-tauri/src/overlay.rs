//! 펫·하우스·손님·침 자국·밥그릇처럼 화면 위에 떠 있는 창의 OS별 마무리.
//! Windows는 빌더 설정(투명·항상 위·작업표시줄 숨김)으로 충분해서 아무 일도 하지 않는다.

use tauri::WebviewWindow;

/// macOS: 모든 Space에 보이고 (canJoinAllSpaces), 다른 앱의 전체 화면 위에도 뜨고 (fullScreenAuxiliary),
/// Mission Control에서 제자리에 있고 (stationary), ⌘` 창 순환에서 빠진다 (ignoresCycle). 그림자 없음.
#[cfg(target_os = "macos")]
pub fn float_everywhere(window: &WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    let w = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Ok(ptr) = w.ns_window() else { return };
        if ptr.is_null() {
            return;
        }
        // SAFETY: tauri가 돌려준 NSWindow 포인터이고, 창이 살아 있는 메인 스레드에서만 쓴다.
        let ns: &NSWindow = unsafe { &*(ptr as *const NSWindow) };
        let behavior = ns.collectionBehavior()
            | NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle;
        ns.setCollectionBehavior(behavior);
        ns.setHasShadow(false);
    });
}

#[cfg(not(target_os = "macos"))]
pub fn float_everywhere(_window: &WebviewWindow) {}
