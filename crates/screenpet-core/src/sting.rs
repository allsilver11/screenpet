//! 꿀벌 쏘기 (spec 2B §10) — 괴롭힘 판정과 쏘기 국면 수치. brain.rs가 쓴다.
//!
//! 괴롭힘: (a) 빠르게 문지르기(간질이기), (b) 들고 흔들기 — 1초 안에 커서 경로 600 px 이상, (c) 2초 안에 3번 클릭.
//! 쏘기: "!"(0.5초) → 화면 쪽으로 돌진(0.3초, 웹뷰가 크기를 키웠다 줄인다) → 콕! (화면 지점을 앱에 알림) → "흥!" → 다시 난다.

use std::collections::VecDeque;

/// "!" 하고 노려보기
pub const ALERT_SECS: f64 = 0.5;
/// 화면 쪽으로 돌진 — 끝나는 순간 콕!
pub const DASH_SECS: f64 = 0.3;
/// 콕! 하고 박힌 채
pub const STUCK_SECS: f64 = 0.6;
/// "흥!" 하고 물러남 — 콕!부터 다시 날기까지 STUCK + RECOIL = 1.5초
pub const RECOIL_SECS: f64 = 0.9;
/// 쏜 뒤 다시 쏘기까지 (쏘기 시작 기준)
pub const COOLDOWN_SECS: f64 = 8.0;
/// 들고 흔들기: SHAKE_WINDOW초 안의 커서 경로가 이 길이(px) 이상
pub const SHAKE_DIST: f64 = 600.0;
pub const SHAKE_WINDOW: f64 = 1.0;
/// 흔들기는 왔다 갔다 — 그 사이 실제로 옮겨진 거리가 경로의 이 비율 이하여야 한다 (빠르게 끌어 던지기는 흔들기가 아니다)
pub const SHAKE_MAX_NET_SHARE: f64 = 0.5;
/// PESTER_WINDOW초 안에 PESTER_CLICKS번 클릭
pub const PESTER_CLICKS: usize = 3;
pub const PESTER_WINDOW: f64 = 2.0;
/// 얼굴 높이 (발에서 몸 크기의 이 비율 위)
pub const FACE_HEIGHT: f64 = 0.55;

/// 쏘기 국면
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StingPhase {
    Alert,
    Dash,
    Stuck,
    Recoil,
}

impl StingPhase {
    pub fn secs(self) -> f64 {
        match self {
            StingPhase::Alert => ALERT_SECS,
            StingPhase::Dash => DASH_SECS,
            StingPhase::Stuck => STUCK_SECS,
            StingPhase::Recoil => RECOIL_SECS,
        }
    }

    pub fn next(self) -> Option<StingPhase> {
        match self {
            StingPhase::Alert => Some(StingPhase::Dash),
            StingPhase::Dash => Some(StingPhase::Stuck),
            StingPhase::Stuck => Some(StingPhase::Recoil),
            StingPhase::Recoil => None,
        }
    }
}

/// 최근 SHAKE_WINDOW초 동안 커서가 움직인 경로 길이
#[derive(Debug, Clone, Default)]
pub struct PathMeter {
    hist: VecDeque<(f64, f64, f64)>,
}

impl PathMeter {
    pub fn push(&mut self, t: f64, x: f64, y: f64) {
        self.hist.push_back((t, x, y));
        while self.hist.front().is_some_and(|&(t0, _, _)| t - t0 > SHAKE_WINDOW) {
            self.hist.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.hist.clear();
    }

    pub fn length(&self) -> f64 {
        self.hist.iter().zip(self.hist.iter().skip(1)).map(|(a, b)| (b.1 - a.1).hypot(b.2 - a.2)).sum()
    }

    /// 처음과 끝 사이 거리
    pub fn net(&self) -> f64 {
        match (self.hist.front(), self.hist.back()) {
            (Some(a), Some(b)) => (b.1 - a.1).hypot(b.2 - a.2),
            _ => 0.0,
        }
    }

    pub fn shaken(&self) -> bool {
        let len = self.length();
        len >= SHAKE_DIST && self.net() <= SHAKE_MAX_NET_SHARE * len
    }
}

/// 최근 클릭 시각들 — PESTER_WINDOW초 안에 PESTER_CLICKS번이면 괴롭힘
#[derive(Debug, Clone, Default)]
pub struct ClickMeter {
    times: VecDeque<f64>,
}

impl ClickMeter {
    /// 클릭 하나를 더하고, 괴롭힘이 되었는지
    pub fn push(&mut self, t: f64) -> bool {
        self.times.push_back(t);
        while self.times.front().is_some_and(|&t0| t - t0 > PESTER_WINDOW) {
            self.times.pop_front();
        }
        self.times.len() >= PESTER_CLICKS
    }

    pub fn clear(&mut self) {
        self.times.clear();
    }
}

/// 몸(발 x, 발 y, 크기)의 얼굴 — 쏜 자국이 박히는 화면 지점
pub fn face_point(x: f64, y: f64, size: f64) -> (f64, f64) {
    (x, y - FACE_HEIGHT * size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_run_alert_dash_stuck_recoil() {
        assert_eq!(StingPhase::Alert.next(), Some(StingPhase::Dash));
        assert_eq!(StingPhase::Dash.next(), Some(StingPhase::Stuck));
        assert_eq!(StingPhase::Stuck.next(), Some(StingPhase::Recoil));
        assert_eq!(StingPhase::Recoil.next(), None);
        assert_eq!((StingPhase::Alert.secs(), StingPhase::Dash.secs()), (0.5, 0.3));
        // 콕!부터 다시 날기까지 1.5초
        assert!((StingPhase::Stuck.secs() + StingPhase::Recoil.secs() - 1.5).abs() < 1e-9);
        assert_eq!(COOLDOWN_SECS, 8.0);
    }

    #[test]
    fn shaking_counts_only_the_last_second() {
        let mut m = PathMeter::default();
        for i in 0..=60 {
            // 1초 동안 60틱, 틱마다 12px 왕복 → 720px
            m.push(i as f64 / 60.0, if i % 2 == 0 { 0.0 } else { 12.0 }, 0.0);
        }
        assert!(m.shaken(), "{}", m.length());
        // 2초 뒤 한 점 → 이전 기록은 다 지나감
        m.push(3.0, 0.0, 0.0);
        assert_eq!(m.length(), 0.0);
        let mut slow = PathMeter::default();
        for i in 0..=60 {
            slow.push(i as f64 / 60.0, i as f64 * 9.0, 0.0);
        }
        assert!(!slow.shaken(), "{}", slow.length());
        slow.clear();
        assert_eq!(slow.length(), 0.0);
        // 빠르게 한쪽으로 끌기(던지기)는 흔들기가 아니다
        let mut fling = PathMeter::default();
        for i in 0..=30 {
            fling.push(i as f64 / 60.0, i as f64 * 30.0, 0.0);
        }
        assert!(fling.length() >= 900.0 && !fling.shaken());
    }

    #[test]
    fn three_clicks_within_two_seconds_pester() {
        let mut m = ClickMeter::default();
        assert!(!m.push(0.0));
        assert!(!m.push(0.9));
        assert!(m.push(1.9));
        let mut spread = ClickMeter::default();
        assert!(!spread.push(0.0));
        assert!(!spread.push(1.5));
        assert!(!spread.push(3.0));
        assert!(spread.push(3.4));
        spread.clear();
        assert!(!spread.push(3.5));
    }

    #[test]
    fn face_is_a_bit_above_the_middle_of_the_body() {
        assert_eq!(face_point(500.0, 800.0, 100.0), (500.0, 745.0));
    }
}
