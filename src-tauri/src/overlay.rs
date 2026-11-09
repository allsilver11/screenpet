//! 펫·하우스·손님·침 자국·밥그릇처럼 화면 위에 떠 있는 창의 OS별 마무리.
//! Windows는 빌더 설정(투명·항상 위·작업표시줄 숨김)으로 충분해서 아무 일도 하지 않는다.

use std::time::Instant;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry};

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

/// finish와 같되 포커스를 뺏지 않는다: tao는 만들 때 보인 창만 활성화 없이 띄우고, 나중의 show()는
/// 활성화한다. 그래서 화면 밖에 보이게 만들고, 그 뒤로는 move_quietly·resize_quietly로만 다룬다
pub fn finish_quiet<M: Manager<Wry>>(builder: WebviewWindowBuilder<'_, Wry, M>) -> tauri::Result<WebviewWindow> {
    builder
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .position(0.0, -20000.0)
        .visible(true)
        .build()
        .inspect(float_everywhere)
}

/// Windows 창 핸들 (만들 때 한 번 읽어 둔다 — hwnd()는 메인 스레드에 물어본다)
pub fn hwnd_of(window: &WebviewWindow) -> Option<isize> {
    #[cfg(windows)]
    {
        window.hwnd().ok().map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        None
    }
}

/// 창을 물리 px로 옮긴다. Windows에서 tao의 set_position은 보이는 창을 매번 활성화한다 (SW_SHOW)
pub fn move_quietly(window: &WebviewWindow, hwnd: Option<isize>, x: i32, y: i32) {
    #[cfg(windows)]
    if hwnd.is_some_and(|h| screenpet_core::platform::move_window(h, x, y)) {
        return;
    }
    let _ = hwnd;
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// 창 크기를 물리 px로 바꾼다 (이유는 move_quietly와 같다)
pub fn resize_quietly(window: &WebviewWindow, hwnd: Option<isize>, w: u32, h: u32) {
    #[cfg(windows)]
    if hwnd.is_some_and(|handle| screenpet_core::platform::resize_window(handle, w as i32, h as i32)) {
        return;
    }
    let _ = hwnd;
    let _ = window.set_size(tauri::PhysicalSize::new(w, h));
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

/// 작고 테두리 있는 입력 창
pub struct Form {
    pub label: &'static str,
    pub page: &'static str,
    pub title: &'static str,
    /// 안쪽 크기 (논리 px)
    pub size: (f64, f64),
}

/// 입력 창의 공통 설정: 크기 고정, 항상 위, 숨긴 채 시작
pub fn form_builder<'a>(app: &'a AppHandle, form: &Form) -> WebviewWindowBuilder<'a, Wry, AppHandle> {
    WebviewWindowBuilder::new(app, form.label, WebviewUrl::App(form.page.into()))
        .title(form.title)
        .inner_size(form.size.0, form.size.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .visible(false)
}

/// 입력 창을 보이고 앞으로 가져온다 (처음이면 만든다 — 게임 루프 스레드에서만). 닫기(X)는 숨기기로 바꾼다 (창은 다시 쓴다).
/// prepare = 보이기 전에 창에 지금 상태를 알린다. place = 숨어 있던 창을 놓을 자리
pub fn show_form(app: &AppHandle, form: &Form, place: fn(&AppHandle, &WebviewWindow, (f64, f64)), prepare: impl FnOnce(&WebviewWindow)) {
    let window = match app.get_webview_window(form.label) {
        Some(w) => w,
        None => match form_builder(app, form).build() {
            Ok(w) => {
                hide_on_close(&w);
                w
            }
            Err(e) => {
                eprintln!("failed to create {} window: {e}", form.label);
                return;
            }
        },
    };
    prepare(&window);
    if !window.is_visible().unwrap_or(false) {
        place(app, &window, form.size);
        let _ = window.show();
    }
    let _ = window.set_focus();
}

/// 작은 창을 주 모니터 가운데에 놓는다 (주 모니터를 모르면 지금 모니터 가운데)
pub fn place_on_primary(app: &AppHandle, window: &WebviewWindow, inner: (f64, f64)) {
    let Some(m) = app.primary_monitor().ok().flatten() else {
        let _ = window.center();
        return;
    };
    let (pos, size, dpi) = (m.position(), m.size(), m.scale_factor());
    // 숨긴 창의 바깥 크기(제목 표시줄 포함)를 모르면 안쪽 크기로 어림한다
    let (w, h) = match window.outer_size() {
        Ok(s) if s.width > 0 && s.height > 0 => (s.width as f64, s.height as f64),
        _ => (inner.0 * dpi, inner.1 * dpi),
    };
    let x = pos.x as f64 + (size.width as f64 - w) / 2.0;
    let y = pos.y as f64 + (size.height as f64 - h) / 2.0;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// 테두리 없이 화면 위에 떠 있는 창 (나머지 창은 제목 표시줄이 있고 닫으면 숨거나 정말 닫힌다)
pub fn is_floating(label: &str) -> bool {
    const PREFIXES: [&str; 5] = ["guest-", "prop-", "hand-", "object-", "weather-"];
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
