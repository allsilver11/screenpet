//! 잠깐 끊긴 연결은 바로 "오프라인"으로 보이지 않게 (회사 망처럼 인터넷이 끊겼다 붙었다 하면 친구가 깜빡이고,
//! 트레이 메뉴가 새로 그려지며 열린 메뉴가 저절로 닫혔다). 끊긴 뒤 OFFLINE_GRACE_SECS 안에 다시 붙으면 바뀐 게 없는 것으로 본다.
//! 시각은 부르는 쪽의 초 (앱 시작부터).

use std::collections::HashMap;

/// 끊겨도 이만큼은 온라인으로 보여 준다 (초)
pub const OFFLINE_GRACE_SECS: f64 = 30.0;

#[derive(Debug, Default, Clone)]
pub struct Flicker {
    /// 끊긴 것(친구 id, 또는 "relay") → 끊긴 시각
    gone: HashMap<String, f64>,
}

impl Flicker {
    /// 끊겼다 (이미 끊겨 있었으면 처음 끊긴 시각 그대로)
    pub fn down(&mut self, id: &str, now: f64) {
        self.gone.entry(id.to_string()).or_insert(now);
    }

    /// 다시 붙었다
    pub fn up(&mut self, id: &str) {
        self.gone.remove(id);
    }

    /// 보여 줄 상태: 살아 있거나, 끊긴 지 OFFLINE_GRACE_SECS가 안 됐다
    pub fn shown(&self, id: &str, live: bool, now: f64) -> bool {
        live || self.gone.get(id).is_some_and(|t| now - t < OFFLINE_GRACE_SECS)
    }

    /// 유예가 이번에 끝난 것들 (메뉴를 다시 그려야 한다) — 끝난 것은 잊는다
    pub fn expired(&mut self, now: f64) -> Vec<String> {
        let done: Vec<String> = self.gone.iter().filter(|(_, t)| now - **t >= OFFLINE_GRACE_SECS).map(|(k, _)| k.clone()).collect();
        for k in &done {
            self.gone.remove(k);
        }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_drop_still_shows_online_and_nothing_changes() {
        let mut f = Flicker::default();
        f.down("a", 10.0);
        assert!(f.shown("a", false, 10.0) && f.shown("a", false, 39.9));
        f.up("a");
        assert!(f.expired(100.0).is_empty(), "came back in time: no menu change");
        assert!(!f.shown("a", false, 100.0), "down again later without a new drop: offline");
    }

    #[test]
    fn a_long_drop_goes_offline_once_after_the_grace() {
        let mut f = Flicker::default();
        f.down("a", 10.0);
        f.down("a", 25.0); // 다시 끊겨도 처음 시각
        assert!(f.expired(39.0).is_empty());
        assert_eq!(f.expired(40.0), vec!["a".to_string()]);
        assert!(f.expired(41.0).is_empty(), "only once");
        assert!(!f.shown("a", false, 41.0));
        assert!(f.shown("a", true, 41.0), "live is always shown");
    }
}
