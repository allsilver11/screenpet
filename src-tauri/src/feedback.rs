//! 💌 개발자에게 건의하기 (spec 2026-11-10-feedback-mailbox §4): 누구나 쓰는 편지 창.
//! 편지는 개발자 공개키(`letter::DEV_KEY`)로 봉해 릴레이 편지함에 맡김 — 릴레이도 읽지 못함.
//! 하루 5통·글자 수 같은 제한은 릴레이가 셈 (앱을 고쳐도 풀 수 없게). 결과는 창에 한 줄로.
//! 🖼️ 사진 한 장(창이 줄여 다시 인코딩한 WebP/JPEG, ≤ `letter::MAX_IMAGE`)을 base64로 받아 다시 확인하고 같이 봉함.

use screenpet_net::letter::{self, Letter};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::slots::Slot;
use crate::AppState;

/// 건의하기 창 라벨
pub const FEEDBACK_LABEL: &str = "feedback";
/// 건의하기 창 모양
const FEEDBACK_FORM: Form = Form { label: FEEDBACK_LABEL, page: "feedback.html", title: "ScreenPet 건의하기", size: (370.0, 430.0) };

/// 창이 받는 결과 (`feedback-result`)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FeedbackResult {
    /// 릴레이가 편지를 받았음
    pub ok: bool,
    /// 릴레이가 알려 준 이유 (daily, ip, new, global, full, size, blocked) 또는 offline·busy·timeout·photo(사진이 틀림)
    pub reason: String,
    /// 🖼️ 옛 서버라 사진은 빼고 글만 보냈음
    pub photo_dropped: bool,
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
        // 픽셀이 빠진 v0.7.0부터 늘 동글 (편지 모양은 옛 버전과 같게 남김)
        style: "round".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        // 🖼️ 사진 크기는 seal_letter가 채움
        img: 0,
    }
}

/// 개발자 열쇠로 봉함 (쓸 말이 없거나 너무 크면 None)
#[cfg(test)]
pub fn seal(letter: &Letter) -> Option<Vec<u8>> {
    letter::seal(letter, &letter::dev_key()?)
}

/// 🖼️ 편지와 (있으면) 사진을 개발자 열쇠로 봉함: 사진 없는 봉투와 사진 봉투 둘 다 (옛 서버엔 앞의 것만 감)
pub fn seal_with(letter: &Letter, photo: Option<&[u8]>) -> Option<letter::Sealed> {
    letter::seal_letter(letter, photo, &letter::dev_key()?)
}

/// 🖼️ 창이 보낸 base64 사진을 다시 확인함: 엄격한 base64, `MAX_IMAGE` 이하, WebP/JPEG 머리와 그림 크기(`photo_ok`). 아니면 None
pub fn decode_photo(b64: &str) -> Option<Vec<u8>> {
    if b64.len() > letter::MAX_IMAGE.div_ceil(3) * 4 {
        return None;
    }
    let bytes = screenpet_core::icon::from_base64(b64)?;
    (!bytes.is_empty() && bytes.len() <= letter::MAX_IMAGE && letter::photo_ok(&bytes)).then_some(bytes)
}

/// 결과를 창에 알림 (`photo_dropped` = 옛 서버라 글만 갔음)
pub fn result(app: &AppHandle, result: Result<(), String>, photo_dropped: bool) {
    let r = match result {
        Ok(()) => FeedbackResult { ok: true, reason: String::new(), photo_dropped },
        Err(reason) => FeedbackResult { ok: false, reason, photo_dropped: false },
    };
    let _ = app.emit_to(FEEDBACK_LABEL, "feedback-result", r);
}

/// 창의 "보내기": 게임 루프(네트워크)로 넘김. 아주 긴 글은 먼저 자름 (letter::clean이 다시 1000자로).
/// 🖼️ 사진(base64)이 틀리면 보내지 않고 `photo`로 알림
#[tauri::command]
pub fn feedback_submit(window: WebviewWindow, state: tauri::State<AppState>, text: String, photo: Option<String>) {
    if window.label() != FEEDBACK_LABEL {
        return;
    }
    let text: String = text.chars().take(4 * letter::MAX_LETTER_CHARS).collect();
    let photo = match photo.as_deref().map(decode_photo) {
        None => None,
        Some(Some(p)) => Some(p),
        Some(None) => return result(window.app_handle(), Err("photo".into()), false),
    };
    state.send(Input::Net(NetInput::SendFeedback(text, photo)));
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

    #[test]
    fn a_photo_from_the_window_is_checked_again() {
        let webp = letter::sample_webp(640, 360, 1000);
        let b64 = screenpet_core::icon::base64(&webp);
        assert_eq!(decode_photo(&b64), Some(webp.clone()));
        let mut big = webp.clone();
        big.resize(letter::MAX_IMAGE + 1, 5);
        assert_eq!(decode_photo(&screenpet_core::icon::base64(&big)), None, "too big");
        assert_eq!(decode_photo(&screenpet_core::icon::base64(b"<svg onload=alert(1)>")), None, "not WebP/JPEG");
        assert_eq!(decode_photo("not base64!"), None);
        let bomb = letter::sample_webp(16000, 16000, 1000);
        assert_eq!(decode_photo(&screenpet_core::icon::base64(&bomb)), None, "a small file that unpacks huge");
        assert_eq!(decode_photo(&"A".repeat(4 * letter::MAX_IMAGE)), None, "a huge string is not even decoded");
        let l = Letter { text: "사진이에요".into(), ..Default::default() };
        let sealed = seal_with(&l, Some(&webp)).unwrap();
        assert!(sealed.photo.is_some() && sealed.plain.len() <= letter::MAX_SEALED);
    }
}
