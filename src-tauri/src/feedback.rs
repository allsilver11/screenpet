//! 💌 개발자에게 건의하기 (spec 2026-11-10-feedback-mailbox §4): 누구나 쓰는 편지 창.
//! 편지는 개발자 공개키(`letter::DEV_KEY`)로 봉해 릴레이 편지함에 맡김 — 릴레이도 읽지 못함.
//! 하루 5통·글자 수 같은 제한은 릴레이가 셈 (앱을 고쳐도 풀 수 없게). 결과는 창에 한 줄로.

use screenpet_net::letter::{self, Letter};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::slots::Slot;
use crate::AppState;

/// 건의하기 창 라벨
pub const FEEDBACK_LABEL: &str = "feedback";
/// 건의하기 창 모양
const FEEDBACK_FORM: Form = Form { label: FEEDBACK_LABEL, page: "feedback.html", title: "ScreenPet 건의하기", size: (370.0, 345.0) };

/// 창이 받는 결과 (`feedback-result`)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FeedbackResult {
    /// 릴레이가 편지를 받았음
    pub ok: bool,
    /// 릴레이가 알려 준 이유 (daily, ip, new, global, full, size, blocked) 또는 offline·busy·timeout
    pub reason: String,
}

/// 트레이 메뉴: 창을 엶
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &FEEDBACK_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(FEEDBACK_LABEL, "feedback-open", ());
    });
}

/// 내 펫과 앱 정보를 담은 편지 (IP·이메일·친구 목록은 넣지 않음)
pub fn letter_for(text: &str, name: &str, own: &Slot) -> Letter {
    Letter {
        text: text.to_string(),
        name: name.to_string(),
        species: own.entry.id.clone(),
        stage: own.shown.stage.name().to_string(),
        style: own.shown.style.to_wire().unwrap_or_else(|| "round".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
    }
}

/// 개발자 열쇠로 봉함 (쓸 말이 없거나 너무 크면 None)
pub fn seal(letter: &Letter) -> Option<Vec<u8>> {
    letter::seal(letter, &letter::dev_key()?)
}

/// 결과를 창에 알림
pub fn result(app: &AppHandle, result: Result<(), String>) {
    let r = match result {
        Ok(()) => FeedbackResult { ok: true, reason: String::new() },
        Err(reason) => FeedbackResult { ok: false, reason },
    };
    let _ = app.emit_to(FEEDBACK_LABEL, "feedback-result", r);
}

/// 창의 "보내기": 게임 루프(네트워크)로 넘김. 아주 긴 글은 먼저 자름 (letter::clean이 다시 1000자로)
#[tauri::command]
pub fn feedback_submit(window: WebviewWindow, state: tauri::State<AppState>, text: String) {
    if window.label() != FEEDBACK_LABEL {
        return;
    }
    let text: String = text.chars().take(4 * letter::MAX_LETTER_CHARS).collect();
    state.send(Input::Net(NetInput::SendFeedback(text)));
}

/// 창의 "닫기"
#[tauri::command]
pub fn feedback_close(window: WebviewWindow) {
    if window.label() == FEEDBACK_LABEL {
        let _ = window.hide();
    }
}

/// 편지 봉인 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sealed_feedback_letter_is_for_the_developer_key_only() {
        let l = Letter { text: "고마워요".into(), ..Default::default() };
        let sealed = seal(&l).unwrap();
        assert!(sealed.len() <= letter::MAX_SEALED);
        assert!(seal(&Letter::default()).is_none(), "nothing to say");
    }
}
