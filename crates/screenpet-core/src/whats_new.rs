//! ✨ 업데이트 소식: 새 버전으로 처음 켜면 한 번, 그동안 나온 릴리즈 노트(`docs/releases/v*.md` — 앱에 같이 들어감)를 창으로 보임.
//! 어떤 노트를 보일지는 여기서 정하고 (설정 `seen_version`과 이 앱 버전), 창은 앱(`src-tauri/src/whats_new.rs`)이 띄움.
//! 처음 설치한 사람은 보이지 않음 (볼 것이 많은 첫 실행을 막지 않게 — 기록만 남김).

/// "0.7.6"·"v0.7.6" → (0, 7, 6). 모르는 모양이면 None
pub fn parse_version(v: &str) -> Option<(u32, u32, u32)> {
    let mut it = v.trim().trim_start_matches('v').split('.').map(|p| p.parse::<u32>().ok());
    let v = (it.next()??, it.next()??, it.next()??);
    it.next().is_none().then_some(v)
}

/// 보일 노트들 (최신 먼저): 처음 설치면 없음. 보던 버전을 모르면 (이 기능 전 버전에서 올라옴) 지금 버전 것만,
/// 알면 그 뒤로 지금 버전까지 나온 것 모두. notes = (버전, 내용)
pub fn notes_to_show<'a>(seen: Option<&str>, current: &str, fresh_install: bool, notes: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let Some(now) = parse_version(current) else { return Vec::new() };
    if fresh_install {
        return Vec::new();
    }
    let since = seen.and_then(parse_version);
    if since.is_some_and(|s| s >= now) {
        return Vec::new();
    }
    let mut out: Vec<(&str, &str, (u32, u32, u32))> = notes
        .iter()
        .filter_map(|(v, text)| parse_version(v).map(|p| (*v, *text, p)))
        .filter(|(_, _, p)| *p <= now && since.map_or(*p == now, |s| *p > s))
        .collect();
    out.sort_by(|a, b| b.2.cmp(&a.2));
    out.into_iter().map(|(v, t, _)| (v, t)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTES: [(&str, &str); 4] = [("v0.7.4", "four"), ("v0.7.5", "five"), ("v0.7.6", "six"), ("v0.7.7", "seven")];

    #[test]
    fn versions_parse_with_or_without_a_v() {
        assert_eq!(parse_version("v0.7.6"), Some((0, 7, 6)));
        assert_eq!(parse_version("0.10.2"), Some((0, 10, 2)));
        assert_eq!(parse_version("0.7"), None);
        assert_eq!(parse_version("0.7.6.1"), None);
        assert_eq!(parse_version("x"), None);
    }

    #[test]
    fn after_an_update_every_note_since_the_last_seen_version_shows_newest_first() {
        assert_eq!(notes_to_show(Some("0.7.4"), "0.7.7", false, &NOTES), [("v0.7.7", "seven"), ("v0.7.6", "six"), ("v0.7.5", "five")]);
        assert_eq!(notes_to_show(Some("0.7.6"), "0.7.7", false, &NOTES), [("v0.7.7", "seven")]);
    }

    #[test]
    fn nothing_again_once_seen_and_nothing_for_a_new_install() {
        assert!(notes_to_show(Some("0.7.7"), "0.7.7", false, &NOTES).is_empty());
        assert!(notes_to_show(None, "0.7.7", true, &NOTES).is_empty());
        // 앞 버전으로 되돌려 깔아도 다시 보이지 않음
        assert!(notes_to_show(Some("0.7.8"), "0.7.7", false, &NOTES).is_empty());
    }

    #[test]
    fn coming_from_a_version_before_this_feature_shows_just_the_current_notes() {
        assert_eq!(notes_to_show(None, "0.7.7", false, &NOTES), [("v0.7.7", "seven")]);
        assert_eq!(notes_to_show(Some("garbage"), "0.7.7", false, &NOTES), [("v0.7.7", "seven")]);
        // 이 버전 노트가 없으면 아무것도
        assert!(notes_to_show(None, "0.7.9", false, &NOTES).is_empty());
    }
}
