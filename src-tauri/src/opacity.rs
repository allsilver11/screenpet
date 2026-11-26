//! 👻 전체 투명도: 떠 있는 창(펫·손님·하우스·물건·소품·날씨·무지개·썰매·탈것) 모두의 투명도를 막대로 한 번에 바꿈.
//! 떠 있는 페이지는 ui/opacity.js를 읽어 `app-opacity` 이벤트를 듣고, 새로 만드는 창은 overlay::finish가 넣는 init_script로 지금 값에서 시작함.
//! 일반 창(설정·편지·도움말 등)은 opacity.js를 읽지 않아 그대로임. 클릭 판정은 바뀌지 않음.

use std::sync::Mutex;

use screenpet_core::config::{Config, OPACITY_DEFAULT};
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::game_loop::Input;
use crate::overlay::Form;
use crate::AppState;

/// 투명도 창 라벨
const LABEL: &str = "opacity";
/// 투명도 창 모양
const FORM: Form = Form { label: LABEL, page: "opacity.html", title: "ScreenPet 투명도", size: (320.0, 140.0) };
/// 떠 있는 창들에 보내는 이벤트 (값 0.2~1.0)
const EVENT: &str = "app-opacity";

/// 지금 투명도 (0.2~1.0). 새 창의 init_script와 app_opacity가 읽음
static CURRENT: Mutex<f64> = Mutex::new(OPACITY_DEFAULT);

/// 지금 투명도
pub fn current() -> f64 {
    *CURRENT.lock().unwrap()
}

/// 설정의 지금 값을 적어 둠 (시작할 때 — 창이 뜨기 전에)
pub fn remember(config: &Config) {
    *CURRENT.lock().unwrap() = config.opacity();
}

/// 설정의 값을 적어 두고 모든 떠 있는 창에 알림 (막대를 움직일 때마다)
pub fn apply(app: &AppHandle, config: &Config) {
    remember(config);
    let _ = app.emit(EVENT, current());
}

/// 새 떠 있는 창이 지금 값에서 시작하게 하는 스크립트 (ui/opacity.js가 읽음 — 처음 그릴 때 번쩍 불투명하지 않게)
pub fn init_script() -> String {
    format!("window.__SCREENPET_OPACITY__ = {};", current())
}

/// 막대 % (20~100)
fn pct() -> u32 {
    (current() * 100.0).round() as u32
}

/// 투명도 창을 열거나 앞으로 가져오고 지금 값을 알림
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(LABEL, "opacity-state", pct());
    });
}

/// 떠 있는 창의 첫 로드용 지금 투명도 (그 뒤로는 `app-opacity` 이벤트)
#[tauri::command]
pub fn app_opacity() -> f64 {
    current()
}

/// 투명도 창의 첫 로드용 지금 값 (%) (그 뒤로는 `opacity-state` 이벤트)
#[tauri::command]
pub fn opacity_state(window: WebviewWindow) -> Result<u32, String> {
    if window.label() != LABEL {
        return Err("wrong window".into());
    }
    Ok(pct())
}

/// 막대를 움직였음 (%, 범위는 설정이 다시 다듬음). save = 놓았음: 파일에 남김
#[tauri::command]
pub fn opacity_set(window: WebviewWindow, state: tauri::State<AppState>, pct: u32, save: bool) {
    if window.label() == LABEL {
        state.send(Input::SetOpacity { pct, save });
    }
}

/// 창의 "닫기"
#[tauri::command]
pub fn opacity_close(window: WebviewWindow) {
    if window.label() == LABEL {
        let _ = window.hide();
    }
}
