//! 🚀 컴퓨터 켜면 자동 실행 (⚙️ 설정 체크 항목). 설정은 게임 루프가 가지므로 게임 루프가 부른다.
//! 이 버전을 처음 켜면 켠다 (config.autostart = None → Some(true)). 그 뒤로는 고른 대로 켤 때마다 OS에 다시 맞춘다
//! (앱이 옮겨졌거나 업데이트로 경로가 바뀌어도 맞는 곳을 가리키게)
use screenpet_core::config::Config;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::AppState;

/// 앱을 켤 때: 처음이면 켜고, 아니면 고른 대로. 설정이 바뀌었으면 true (저장해야 한다)
pub fn apply_at_start(app: &AppHandle, config: &mut Config) -> bool {
    let (on, changed) = config.settle_autostart();
    // 개발 빌드(`tauri dev`)는 target/debug의 실행 파일을 로그인 항목에 넣지 않는다 — 체크만 맞춘다 (눌러서 바꾸는 건 된다)
    if cfg!(debug_assertions) {
        let _ = app.state::<AppState>().menu.autostart.set_checked(on);
    } else {
        set(app, on);
    }
    changed
}

/// 체크 항목을 눌렀다: 뒤집어서 바로 OS에 적용한다 (저장은 부른 쪽이)
pub fn toggle(app: &AppHandle, config: &mut Config) {
    let (on, _) = config.settle_autostart();
    config.autostart = Some(!on);
    set(app, !on);
}

fn set(app: &AppHandle, on: bool) {
    let result = if on { app.autolaunch().enable() } else { app.autolaunch().disable() };
    if let Err(e) = result {
        eprintln!("autostart {}: {e}", if on { "enable" } else { "disable" });
    }
    // 체크 항목은 누를 때 스스로 뒤집히므로 늘 고른 값으로 다시 맞춘다
    let _ = app.state::<AppState>().menu.autostart.set_checked(on);
}
