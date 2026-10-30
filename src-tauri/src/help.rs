//! 도움말 창: 메뉴 "도움말 ❓" → 기능 설명 (ui/help.html, IPC 없음). 이미 열려 있으면 앞으로 가져온다
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const HELP_LABEL: &str = "help";
const HELP_SIZE: (f64, f64) = (400.0, 560.0);

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(HELP_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
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
