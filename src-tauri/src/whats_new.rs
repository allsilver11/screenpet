//! ✨ 업데이트 소식 창: 새 버전으로 처음 켜면 한 번, 그동안 나온 릴리즈 노트(`docs/releases/v*.md`, 빌드 때 들어감)를 보임.
//! 무엇을 보일지는 코어 `whats_new::notes_to_show` (설정 `seen_version`), 창은 `ui/whats_new.html` (글자만 넣어 그림).
//! 트레이 "✨ 업데이트 소식"으로 지금 버전 노트를 다시 볼 수 있음.
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::overlay::Form;

include!(concat!(env!("OUT_DIR"), "/release_notes.rs"));

/// 소식 창 라벨
pub const LABEL: &str = "whats-new";
/// 소식 창 모양
const FORM: Form = Form { label: LABEL, page: "whats_new.html", title: "ScreenPet 업데이트 소식", size: (460.0, 560.0) };
/// 이 앱 버전
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 노트 하나 (버전, 마크다운 글)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Note {
    /// "v0.7.7"
    pub version: String,
    /// 노트 글 (마크다운 — 창이 머리·굵은 줄·목록만 글자로 그림)
    pub text: String,
}

/// 창이 처음 열릴 때 읽는 노트들 (그 뒤로는 `whats-new` 이벤트)
static NOTES: Mutex<Vec<Note>> = Mutex::new(Vec::new());

/// 시작할 때: 보일 노트가 있으면 창을 띄움. 설정에 지금 버전을 적었으면 true (저장은 부른 쪽이)
pub fn on_start(app: &AppHandle, config: &mut screenpet_core::config::Config, fresh_install: bool) -> bool {
    let notes = screenpet_core::whats_new::notes_to_show(config.seen_version.as_deref(), VERSION, fresh_install, RELEASE_NOTES);
    if !notes.is_empty() {
        show_notes(app, notes.into_iter().map(|(v, t)| Note { version: v.to_string(), text: t.to_string() }).collect());
    }
    if config.seen_version.as_deref() == Some(VERSION) {
        return false;
    }
    config.seen_version = Some(VERSION.to_string());
    true
}

/// 트레이 "✨ 업데이트 소식": 지금 버전 노트를 다시 보임 (없으면 가장 새 것)
pub fn show(app: &AppHandle) {
    let current = RELEASE_NOTES.iter().find(|(v, _)| v.trim_start_matches('v') == VERSION).or(RELEASE_NOTES.last());
    show_notes(app, current.map(|(v, t)| Note { version: v.to_string(), text: t.to_string() }).into_iter().collect());
}

/// 노트들을 창에 보임 (처음이면 만듦)
fn show_notes(app: &AppHandle, notes: Vec<Note>) {
    *NOTES.lock().unwrap() = notes.clone();
    crate::overlay::show_form(app, &FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(LABEL, "whats-new", notes);
    });
}

/// 소식 창이 처음 열릴 때 노트를 읽음
#[tauri::command]
pub fn whats_new_state(window: WebviewWindow) -> Result<Vec<Note>, String> {
    if window.label() != LABEL {
        return Err("wrong window".into());
    }
    Ok(NOTES.lock().unwrap().clone())
}

/// 소식 창의 "확인"
#[tauri::command]
pub fn whats_new_close(window: WebviewWindow) {
    if window.label() == LABEL {
        let _ = window.hide();
    }
}

/// 빌드에 들어간 노트 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_versions_notes_are_bundled() {
        assert!(RELEASE_NOTES.iter().all(|(v, t)| v.starts_with('v') && !t.trim().is_empty()));
        assert!(RELEASE_NOTES.iter().any(|(v, _)| *v == "v0.7.6"));
    }
}
