//! 📮 The public suggestion board: what everyone may see of a letter.
//!
//! Only the developer's app can open letters, so it is the one that makes a
//! [`Digest`] of each — the sender's name with every grapheme but the first
//! hidden (`김철수` → `김**`, one-grapheme names → `*`) and the first line of
//! the text, cut short — and publishes it to the relay. The body never leaves
//! the developer's computer. The relay checks both again.

use serde::Serialize;
use unicode_segmentation::UnicodeSegmentation;

/// Longest masked name (characters, the relay refuses longer).
pub const MAX_NAME: usize = 16;
/// The first line keeps at most this many characters before "…".
pub const LINE_CHARS: usize = 60;
/// A first grapheme longer than this (a pile of combining marks) shows only its first character.
const MAX_FIRST: usize = 8;
/// Entries per page (the relay pages the same way).
pub const PER_PAGE: usize = 5;
/// Digests per `board_put` message.
pub const PUT_CHUNK: usize = 20;

fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{2069}' | '\u{FEFF}' | '\u{061C}' | '\u{180E}' | '\u{00AD}' | '\u{3164}' | '\u{115F}' | '\u{1160}' | '\u{FFA0}' | '\u{2800}' | '\u{E0000}'..='\u{E007F}' | '\u{2028}' | '\u{2029}')
}

/// The name with only its first grapheme showing: `김철수` → `김**`, `A` →
/// `*`, empty → empty. At most [`MAX_NAME`] characters.
pub fn mask_name(name: &str) -> String {
    let clean: String = name.chars().filter(|&c| !hidden(c) || c == '\u{200D}').collect();
    let clean = clean.trim();
    let mut g = clean.graphemes(true);
    let Some(first) = g.next() else { return String::new() };
    let rest = g.count();
    if rest == 0 {
        return "*".into();
    }
    let first: String = if first.chars().count() > MAX_FIRST { first.chars().take(1).collect() } else { first.to_string() };
    let stars = rest.min(MAX_NAME - first.chars().count());
    format!("{first}{}", "*".repeat(stars))
}

/// The first line of `text`, trimmed; past [`LINE_CHARS`] characters it is
/// cut at a grapheme boundary and ends with "…".
pub fn first_line(text: &str) -> String {
    let line = text.split(['\n', '\r']).map(|l| l.chars().filter(|&c| !hidden(c) || c == '\u{200D}').collect::<String>()).find(|l| !l.trim().is_empty()).unwrap_or_default();
    let line = line.trim();
    let mut out = String::new();
    let mut n = 0;
    for g in line.graphemes(true) {
        let len = g.chars().count();
        if n + len > LINE_CHARS {
            return format!("{}…", out.trim_end());
        }
        out.push_str(g);
        n += len;
    }
    out
}

/// What the developer's app publishes for one letter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Digest {
    pub id: u64,
    pub name: String,
    pub line: String,
}

/// The public digest of an opened letter (`None` if nothing is left to show).
pub fn digest(id: u64, letter: &crate::letter::Letter) -> Option<Digest> {
    let line = first_line(&letter.text);
    (!line.is_empty()).then(|| Digest { id, name: mask_name(&letter.name), line })
}

/// The developer's mark on a letter: ✅ 반영됨, 🙅 반려됨 or nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Done,
    Rejected,
    /// (also anything a newer relay may say)
    #[default]
    #[serde(other)]
    None,
}

/// One entry of a board page, as the relay lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct BoardItem {
    pub id: u64,
    pub name: String,
    pub line: String,
    /// When the letter arrived (unix s).
    pub at: u64,
    #[serde(default)]
    pub status: Status,
}

/// A page of the board: `page` is 1-based, `pages` at least 1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct BoardPage {
    pub items: Vec<BoardItem>,
    pub page: u32,
    pub pages: u32,
    pub total: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::letter::Letter;

    #[test]
    fn names_show_only_their_first_grapheme() {
        assert_eq!(mask_name("김철수"), "김**");
        assert_eq!(mask_name("단니"), "단*");
        assert_eq!(mask_name("김"), "*", "a one-character name hides it all");
        assert_eq!(mask_name("A"), "*");
        assert_eq!(mask_name(""), "");
        assert_eq!(mask_name("  \u{200B} "), "");
        assert_eq!(mask_name("Danny"), "D****");
        assert_eq!(mask_name("  철수  "), "철*", "trimmed");
        // emoji count as one each, even when joined or with a variation selector
        assert_eq!(mask_name("🐱냥이"), "🐱**");
        assert_eq!(mask_name("❤️하트"), "❤️**");
        assert_eq!(mask_name("👨‍👩‍👧가족"), "👨‍👩‍👧**");
        assert_eq!(mask_name("가👨‍👩‍👧🐱"), "가**");
        // decomposed Hangul (as macOS may give) is still one per syllable
        assert_eq!(mask_name("\u{1100}\u{1175}\u{11B7}\u{110E}\u{1165}\u{11AF}"), "\u{1100}\u{1175}\u{11B7}*");
        // bidi tricks are dropped, long names are capped
        assert_eq!(mask_name("\u{202E}abc"), "a**");
        let long = mask_name(&"가".repeat(64));
        assert_eq!(long.chars().count(), MAX_NAME);
        assert!(long.starts_with('가') && long.chars().skip(1).all(|c| c == '*'));
        let zalgo = format!("a{}bc", "\u{0301}".repeat(30));
        assert_eq!(mask_name(&zalgo), "a**", "a huge first grapheme shows just its first character");
    }

    #[test]
    fn only_the_first_line_is_shown_and_long_ones_are_cut() {
        assert_eq!(first_line("펭귄 귀여워요\n고마워요"), "펭귄 귀여워요");
        assert_eq!(first_line("  안녕하세요  \r\n둘째 줄"), "안녕하세요");
        assert_eq!(first_line("\n\n  셋째 줄부터 \n"), "셋째 줄부터", "the first line with something on it");
        assert_eq!(first_line(""), "");
        let exact = "가".repeat(LINE_CHARS);
        assert_eq!(first_line(&exact), exact, "exactly the limit: not cut");
        let long = "가".repeat(LINE_CHARS + 1);
        let cut = first_line(&long);
        assert_eq!(cut, format!("{}…", "가".repeat(LINE_CHARS)));
        // never splits an emoji
        let emoji = format!("{}👨‍👩‍👧", "a".repeat(LINE_CHARS - 2));
        assert_eq!(first_line(&emoji), format!("{}…", "a".repeat(LINE_CHARS - 2)));
        assert_eq!(first_line("🐱 좋아요 🐶"), "🐱 좋아요 🐶");
        assert_eq!(first_line("a\u{202E}b\u{0007}c"), "abc", "control and bidi characters are dropped");
        assert_eq!(first_line(&format!("{} 끝", "가".repeat(LINE_CHARS - 1))), format!("{}…", "가".repeat(LINE_CHARS - 1)), "no space before the ellipsis");
    }

    #[test]
    fn a_digest_carries_no_more_than_the_mask_and_the_first_line() {
        let l = Letter { text: "펭귄 더 주세요\n비밀 이야기".into(), name: "김철수".into(), species: "penguin".into(), version: "0.6.3".into(), ..Default::default() };
        let d = digest(7, &l).unwrap();
        assert_eq!(d, Digest { id: 7, name: "김**".into(), line: "펭귄 더 주세요".into() });
        let json = serde_json::to_string(&d).unwrap();
        assert!(!json.contains("비밀") && !json.contains("penguin") && !json.contains("0.6.3"), "{json}");
        assert_eq!(digest(1, &Letter::default()), None);
    }
}
