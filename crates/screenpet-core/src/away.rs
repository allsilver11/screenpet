//! 친구 화면에 간 내 동물이 아직 거기 있는지 확인함 (친구 앱이 다시 켜지면 손님을 잃는데, 링크는 몇 초 만에 다시 붙어 끊김 소식이 없음)
//! - 가 있는 동안 `visit?<id>`를 묻고, 새 판 친구는 `visit_here:<id>` / `visit_gone:<id>`로 답함 (예전 판은 모르는 부탁을 무시함)
//! - 없다고 하면 바로 집으로, 있다고 하던 친구가 조용해지면 일찍 부름. 한 번도 답하지 않은(예전 판) 친구는 visit::AWAY_SILENCE_LIMIT_SECS
//! - 내 동물이 집에 없을 때 친구의 대답(바쁨)을 띄울 창도 고름

use crate::visit::AWAY_SILENCE_LIMIT_SECS;

/// 보낸 뒤 이만큼(초) 지나서부터 물음 (Visit이 먼저 닿아 손님이 생긴 뒤)
pub const PROBE_AFTER_SECS: f64 = 20.0;
/// 묻는 간격 (초)
pub const PROBE_EVERY_SECS: f64 = 30.0;
/// "있어요"라고 하던 친구가 이만큼(초) 답이 없으면 집으로 부름 (답 몇 개가 빠져도 끊지 않을 만큼 넉넉히)
pub const HEARD_LIMIT_SECS: f64 = 150.0;
/// 묻는 부탁 앞머리 (`visit?<visit_id>`)
pub const WHERE_PREFIX: &str = "visit?";
/// "여기 있어요" 답 앞머리
pub const HERE_PREFIX: &str = "visit_here:";
/// "여기 없어요" 답 앞머리 (친구 앱이 다시 켜져 손님을 잃음)
pub const GONE_PREFIX: &str = "visit_gone:";
/// 잃어버린 방문에서 바로 돌아온 내 동물의 한마디
pub const LOST_LINE: &str = "oops, back home!";
/// "펫 데리고 오기"를 눌렀는데 그 친구 펫이 이미 내 화면에 있을 때 그 손님의 한마디
pub const ALREADY_HERE_LINE: &str = "I'm already here!";
/// 내 동물이 놀러 가 있고 보이는 손님도 없어 미뤄 둔 한마디는 이만큼(초) 안에 돌아오면 말함
pub const HELD_LINE_SECS: f64 = 120.0;

/// 친구에게 내 동물(visit_id)이 아직 있는지 묻는 부탁 글자임
pub fn where_ask(visit_id: u64) -> String {
    format!("{WHERE_PREFIX}{visit_id}")
}

/// 앞머리 뒤 숫자만 visit_id로 읽음 (숫자 아닌 것·너무 긴 것은 None)
fn parse_id(rest: &str) -> Option<u64> {
    if rest.is_empty() || rest.len() > 20 || !rest.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    rest.parse().ok()
}

/// 받은 부탁이 "내 동물 아직 있어?"면 그 visit_id임
pub fn parse_where(ask: &str) -> Option<u64> {
    parse_id(ask.strip_prefix(WHERE_PREFIX)?)
}

/// 물음에 대한 답 글자임: present = 그 방문의 손님이 지금 내 화면에 있음
pub fn answer(visit_id: u64, present: bool) -> String {
    let prefix = if present { HERE_PREFIX } else { GONE_PREFIX };
    format!("{prefix}{visit_id}")
}

/// 친구가 알려 준 내 동물 소식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum News {
    /// 그 방문으로 아직 와 있음
    Here(u64),
    /// 그 방문의 손님이 없음 — 바로 집으로
    Gone(u64),
}

/// 받은 부탁이 내 동물 소식이면 읽음
pub fn parse_news(ask: &str) -> Option<News> {
    if let Some(rest) = ask.strip_prefix(HERE_PREFIX) {
        return parse_id(rest).map(News::Here);
    }
    parse_id(ask.strip_prefix(GONE_PREFIX)?).map(News::Gone)
}

/// 지금 물을 때인가: secs_away = 보낸 뒤 지난 초, since_probe = 지난번 물은 뒤 지난 초 (안 물었으면 None)
pub fn probe_due(secs_away: f64, since_probe: Option<f64>) -> bool {
    secs_away >= PROBE_AFTER_SECS && since_probe.is_none_or(|s| s >= PROBE_EVERY_SECS)
}

/// 집으로 부를 만큼 조용한가: since_heard = 마지막 "있어요" 뒤 지난 초 (한 번도 못 들었으면 None — 예전 판 친구는 긴 한도)
pub fn too_quiet(secs_away: f64, since_heard: Option<f64>) -> bool {
    secs_away >= AWAY_SILENCE_LIMIT_SECS || since_heard.is_some_and(|s| s >= HEARD_LIMIT_SECS)
}

/// 친구의 대답(바쁨 등)을 띄울 창: 내 동물이 집에 보이면 그 창(own), 친구 화면에 가 있으면 와 있는 손님 하나,
/// 아무도 없으면 None (돌아오면 말함)
pub fn speaker<'a>(own_home: bool, own: &'a str, guests: &[&'a str]) -> Option<&'a str> {
    if own_home {
        Some(own)
    } else {
        guests.first().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_and_answers_round_trip() {
        assert_eq!(parse_where(&where_ask(42)), Some(42));
        assert_eq!(parse_news(&answer(7, true)), Some(News::Here(7)));
        assert_eq!(parse_news(&answer(u64::MAX, false)), Some(News::Gone(u64::MAX)));
        // net이 64자로 자르므로 가장 긴 것도 들어감
        assert!(where_ask(u64::MAX).len() <= 64 && answer(u64::MAX, false).len() <= 64);
    }

    #[test]
    fn junk_is_not_a_visit_ask() {
        assert_eq!(parse_where("visit?"), None);
        assert_eq!(parse_where("visit?12a"), None);
        assert_eq!(parse_where("visit?-1"), None);
        assert_eq!(parse_where("visit?99999999999999999999"), None, "over u64");
        assert_eq!(parse_where("invite"), None);
        assert_eq!(parse_news("visit_gone:"), None);
        assert_eq!(parse_news("visit_lost:3"), None);
        assert_eq!(parse_news("busy"), None);
        // 다른 부탁과 겹치지 않음
        assert_eq!(crate::dodge::parse_score_ask(&where_ask(5)), None);
    }

    #[test]
    fn probes_start_after_arrival_and_repeat_slowly() {
        assert!(!probe_due(0.0, None), "the Visit may not have landed yet");
        assert!(!probe_due(PROBE_AFTER_SECS - 1.0, None));
        assert!(probe_due(PROBE_AFTER_SECS, None));
        assert!(!probe_due(100.0, Some(PROBE_EVERY_SECS - 1.0)));
        assert!(probe_due(100.0, Some(PROBE_EVERY_SECS)));
    }

    #[test]
    fn a_host_that_answered_then_went_quiet_calls_the_pet_home_early() {
        // 몇 번 답이 빠져도 (net 부탁 예산 등) 끊지 않음
        assert!(HEARD_LIMIT_SECS >= 4.0 * PROBE_EVERY_SECS);
        assert!(!too_quiet(600.0, Some(HEARD_LIMIT_SECS - 1.0)));
        assert!(too_quiet(600.0, Some(HEARD_LIMIT_SECS)));
        assert!(HEARD_LIMIT_SECS + PROBE_AFTER_SECS < AWAY_SILENCE_LIMIT_SECS / 5.0, "much sooner than the old limit");
    }

    #[test]
    fn an_old_host_that_never_answers_keeps_the_long_limit() {
        assert!(!too_quiet(AWAY_SILENCE_LIMIT_SECS - 1.0, None));
        assert!(too_quiet(AWAY_SILENCE_LIMIT_SECS, None));
        // 답을 들었어도 전체 한도는 그대로
        assert!(too_quiet(AWAY_SILENCE_LIMIT_SECS, Some(0.0)));
    }

    #[test]
    fn lines_are_short() {
        for line in [LOST_LINE, ALREADY_HERE_LINE] {
            assert!(line.chars().count() <= 20 && line.is_ascii(), "{line}");
        }
    }

    #[test]
    fn a_refusal_shows_where_someone_is_visible() {
        assert_eq!(speaker(true, "pet", &["guest-1"]), Some("pet"));
        assert_eq!(speaker(false, "pet", &["guest-1", "guest-2"]), Some("guest-1"), "own pet is away: a guest says it");
        assert_eq!(speaker(false, "pet", &[]), None, "nobody on screen: say it when back");
    }
}
