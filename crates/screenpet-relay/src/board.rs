//! 📮 공개 건의함 (v0.6.3): 편지 중 모든 앱이 봐도 되는 부분임.
//!
//! 편지를 열 수 있는 건 개발자 앱뿐이라 그 앱이 편지마다 요약을 올림: 첫 그래핌만 남기고 가린 보낸이 이름
//! (`김**`, 한 그래핌 이름은 `*`)과 편지 첫 줄. 항목은 `letters.json`에 그 편지 옆에 살고 편지와 함께 사라짐 (지우기, 차단).
//! 앱은 고쳐질 수 있으므로 여기서 모두 다시 검사함: 가림이 진짜 가림이어야 하고, 길이는 자르고, 제어·방향 문자는 버림.
//! `hub::mail`이 씀.

use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

/// 페이지 크기, 이름·첫 줄 길이, `board_put` 하나의 요약 수 (앱과 같이 씀)
pub use screenpet_proto::board::{MAX_LINE, MAX_NAME, PER_PAGE, PUT_MAX};
/// 페이지 항목과 편지 표시: 와이어 타입 그대로임 (`at`, `status`는 편지에서 옴)
pub use screenpet_proto::relay::{BoardItem as Item, BoardStatus as Status};
/// 이름 첫 그래핌의 최대 문자 수
use screenpet_proto::board::MAX_FIRST;

/// 편지 하나의 저장된 공개 요약임
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 가린 보낸이 이름
    pub name: String,
    /// 첫 줄
    pub line: String,
}

/// 보이지 않는·제어·방향 바꾸는 문자인지 봄 (이모지 안의 폭 없는 결합자는 남김)
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200B}'..='\u{200C}' | '\u{200E}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}' | '\u{00AD}' | '\u{3164}' | '\u{115F}' | '\u{1160}' | '\u{FFA0}' | '\u{2800}' | '\u{E0000}'..='\u{E007F}' | '\u{2028}' | '\u{2029}')
}

/// 숨은 문자를 빼고 앞뒤 공백을 다듬음
fn strip(s: &str) -> String {
    let t: String = s.chars().filter(|&c| !hidden(c)).collect();
    t.trim().to_string()
}

/// 가린 이름을 다듬어 검사함: 빈 이름, 또는 그래핌 하나 뒤에 `*`만 (`김**`, `*`).
/// 첫 그래핌보다 더 보이거나 너무 길면 None임
pub fn check_name(raw: &str) -> Option<String> {
    let name = strip(raw);
    if name.chars().count() > MAX_NAME {
        return None;
    }
    let mut g = name.graphemes(true);
    if let Some(first) = g.next() {
        if first.chars().count() > MAX_FIRST || first.chars().all(char::is_whitespace) {
            return None;
        }
    }
    g.all(|x| x == "*").then_some(name)
}

/// 첫 줄을 다듬어 검사함: 비어 있지 않고, 한 줄이고, 최대 [`MAX_LINE`]글자
pub fn check_line(raw: &str) -> Option<String> {
    let line = strip(raw);
    (!line.is_empty() && line.chars().count() <= MAX_LINE).then_some(line)
}

/// `total`개 중 `want`쪽(1부터, 범위로 자름)의 (첫 번호, 끝, 쪽, 전체 쪽)을 돌려줌. 쪽 수는 최소 1이라 빈 건의함은 1쪽 중 1쪽임
pub fn page_range(total: usize, want: u32) -> (usize, usize, u32, u32) {
    let pages = total.div_ceil(PER_PAGE).max(1) as u32;
    let page = want.clamp(1, pages);
    let start = (page as usize - 1) * PER_PAGE;
    (start.min(total), (start + PER_PAGE).min(total), page, pages)
}

/// 가림 검사·첫 줄·쪽 계산 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_real_masks_are_taken() {
        assert_eq!(check_name("김**").as_deref(), Some("김**"));
        assert_eq!(check_name("*").as_deref(), Some("*"));
        assert_eq!(check_name("").as_deref(), Some(""), "no name");
        assert_eq!(check_name("🐱**").as_deref(), Some("🐱**"));
        assert_eq!(check_name("👨‍👩‍👧**").as_deref(), Some("👨‍👩‍👧**"));
        assert_eq!(check_name("❤️*").as_deref(), Some("❤️*"));
        assert_eq!(check_name(" 김** ").as_deref(), Some("김**"));
        assert_eq!(check_name("\u{202E}김**").as_deref(), Some("김**"), "bidi characters are dropped");
        assert_eq!(check_name("김철수"), None, "not masked");
        assert_eq!(check_name("김*수"), None);
        assert_eq!(check_name("김철*"), None);
        assert_eq!(check_name("김 **"), None);
        assert_eq!(check_name("김**\u{0007}x"), None, "a control character can't hide a letter");
        assert_eq!(check_name(&format!("김{}", "*".repeat(MAX_NAME))), None, "too long");
        assert!(check_name(&format!("김{}", "*".repeat(MAX_NAME - 1))).is_some());
        assert_eq!(check_name(&format!("a{}*", "\u{0301}".repeat(10))), None, "a huge first grapheme");
    }

    #[test]
    fn lines_are_one_short_clean_line() {
        assert_eq!(check_line(" 펭귄 더 주세요 ").as_deref(), Some("펭귄 더 주세요"));
        assert_eq!(check_line("a\u{202E}b\u{2066}c").as_deref(), Some("abc"));
        assert_eq!(check_line("한 줄\n두 줄").as_deref(), Some("한 줄두 줄"), "newlines are dropped like any control character");
        assert_eq!(check_line(""), None);
        assert_eq!(check_line(" \u{200B} "), None);
        assert!(check_line(&"가".repeat(MAX_LINE)).is_some());
        assert_eq!(check_line(&"가".repeat(MAX_LINE + 1)), None);
    }

    #[test]
    fn paging_math() {
        assert_eq!(page_range(0, 1), (0, 0, 1, 1), "empty: page 1 of 1");
        assert_eq!(page_range(0, 9), (0, 0, 1, 1));
        assert_eq!(page_range(5, 1), (0, 5, 1, 1));
        assert_eq!(page_range(6, 1), (0, 5, 1, 2));
        assert_eq!(page_range(6, 2), (5, 6, 2, 2));
        assert_eq!(page_range(6, 3), (5, 6, 2, 2), "past the end: the last page");
        assert_eq!(page_range(6, 0), (0, 5, 1, 2), "page 0 is page 1");
        assert_eq!(page_range(200, 40), (195, 200, 40, 40));
        assert_eq!(page_range(200, u32::MAX), (195, 200, 40, 40));
    }
}
