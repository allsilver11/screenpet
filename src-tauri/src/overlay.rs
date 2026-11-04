//! 펫·하우스·손님·침 자국·밥그릇처럼 화면 위에 떠 있는 창의 OS별 마무리.
//! Windows는 빌더 설정(투명·항상 위·작업표시줄 숨김)으로 충분해서 아무 일도 하지 않는다.

use std::time::Instant;

use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry};

/// 사라지는 모습을 보여 주는 동안 기다렸다 닫을 창들
#[derive(Default)]
pub struct Fading(Vec<(WebviewWindow, Instant)>);

impl Fading {
    /// 웹뷰 body에 class를 붙여 (사라지는 그림) at에 닫는다
    pub fn start(&mut self, window: WebviewWindow, class: &str, at: Instant) {
        let _ = window.eval(format!("document.body.classList.add('{class}')"));
        self.0.push((window, at));
    }

    /// 때가 된 창을 닫는다 (매 틱)
    pub fn tick(&mut self, now: Instant) {
        self.0.retain(|(w, at)| {
            if now >= *at {
                let _ = w.destroy();
                false
            } else {
                true
            }
        });
    }
}

/// 화면 위에 떠 있는 창을 만든다: 투명, 테두리·그림자 없음, 항상 위, 작업표시줄 숨김, 포커스 안 받음, 숨긴 채 시작
pub fn finish<M: Manager<Wry>>(builder: WebviewWindowBuilder<'_, Wry, M>) -> tauri::Result<WebviewWindow> {
    builder
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
        .inspect(float_everywhere)
}

/// 이미 있는 창을 앞으로 가져온다. 없으면 false (도움말·도감처럼 정말 닫히는 창)
pub fn focus_existing(app: &AppHandle, label: &str) -> bool {
    let Some(window) = app.get_webview_window(label) else { return false };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    true
}

/// 닫기(X)를 숨기기로 바꾼다 (창은 다시 쓴다)
pub fn hide_on_close(window: &WebviewWindow) {
    let handle = window.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
}

/// 테두리 없이 화면 위에 떠 있는 창 (나머지 창은 제목 표시줄이 있고 닫으면 숨거나 정말 닫힌다)
pub fn is_floating(label: &str) -> bool {
    const PREFIXES: [&str; 6] = ["guest-", "prop-", "hand-", "sting-", "object-", "weather-"];
    label == crate::slots::OWN_LABEL || label == crate::house::LABEL || PREFIXES.iter().any(|p| label.starts_with(p))
}

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
