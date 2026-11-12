//! 📏 크기 조절 창: 펫·물건 크기 슬라이더. 내 화면에만 적용됨 (친구에게 보내는 크기엔 안 들어감).

use std::sync::Mutex;

use screenpet_core::config::{Config, SIZE_PCT_DEFAULT, SIZE_PCT_RANGE};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::game_loop::Input;
use crate::overlay::Form;
use crate::AppState;

/// 크기 조절 창 라벨
const LABEL: &str = "size";
/// 크기 조절 창 모양
const FORM: Form = Form { label: LABEL, page: "size.html", title: "ScreenPet 크기 조절", size: (340.0, 210.0) };

/// 창에 보이는 값 (`size-state` 이벤트, `size_state` 커맨드)
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SizeView {
    pet: u32,
    object: u32,
    min: u32,
    max: u32,
    default: u32,
}

/// 창이 처음 열릴 때 읽는 지금 값
static VIEW: Mutex<SizeView> =
    Mutex::new(SizeView { pet: SIZE_PCT_DEFAULT, object: SIZE_PCT_DEFAULT, min: SIZE_PCT_RANGE.0, max: SIZE_PCT_RANGE.1, default: SIZE_PCT_DEFAULT });

/// 설정의 지금 값을 창이 읽을 수 있게 적어 둠
pub fn remember(config: &Config) {
    let mut view = VIEW.lock().unwrap();
    view.pet = config.pet_size_pct();
    view.object = config.object_size_pct();
}

/// 크기 조절 창을 열거나 앞으로 가져오고 지금 값을 알림
pub fn show(app: &AppHandle, config: &Config) {
    remember(config);
    crate::overlay::show_form(app, &FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(LABEL, "size-state", *VIEW.lock().unwrap());
    });
}

/// 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `size-state` 이벤트)
#[tauri::command]
pub fn size_state(window: WebviewWindow) -> Result<SizeView, String> {
    if window.label() != LABEL {
        return Err("wrong window".into());
    }
    Ok(*VIEW.lock().unwrap())
}

/// 슬라이더를 움직였음 (범위는 설정이 다시 다듬음). save = 놓았음: 파일에 남김
#[tauri::command]
pub fn size_set(window: WebviewWindow, state: tauri::State<AppState>, pet: u32, object: u32, save: bool) {
    if window.label() == LABEL {
        state.send(Input::SetSizes { pet, object, save });
    }
}

/// 창의 "닫기"
#[tauri::command]
pub fn size_close(window: WebviewWindow) {
    if window.label() == LABEL {
        let _ = window.hide();
    }
}
