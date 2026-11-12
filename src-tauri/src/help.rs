//! 도움말 창: 메뉴 "도움말 ❓" → 기능 설명 (ui/help.html, IPC 없음). 이미 열려 있으면 앞으로 가져옴
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

/// 도움말 창 라벨
const HELP_LABEL: &str = "help";
/// 도움말 창 크기 (논리 px)
const HELP_SIZE: (f64, f64) = (400.0, 560.0);

/// 도움말 창을 열거나 이미 열린 창을 앞으로 가져옴
pub fn show(app: &AppHandle) {
    if crate::overlay::focus_existing(app, HELP_LABEL) {
        return;
    }
    let built = WebviewWindowBuilder::new(app, HELP_LABEL, WebviewUrl::App("help.html".into()))
        .title("ScreenPet 도움말")
        .inner_size(HELP_SIZE.0, HELP_SIZE.1)
        .resizable(true)
        .maximizable(false)
        .always_on_top(true)
        .center()
        .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(e) => eprintln!("failed to open help window: {e}"),
    }
}
