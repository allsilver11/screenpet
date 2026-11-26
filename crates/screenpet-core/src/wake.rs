//! 💤 떠 있는 창을 새로 고칠 때를 1초마다 봄: 컴퓨터가 잤다 깼음 (벽시계가 훌쩍 뜀), 오래 비웠다 돌아옴 (잠금 풀기 — 유휴 시간이 길다가 짧아짐),
//! 모니터 구성이 바뀌고 잠잠해짐. Windows WebView2는 긴 잠금·절전·모니터 끊김 뒤 투명 창의 그림을 잃어 (창은 "보임"인데 텅 빔)
//! 펫이 안 보이던 버그가 있었음 — 앱이 이때 웹뷰를 다시 읽고 창을 다시 보임 (game_loop.rs refresh_windows).
use crate::geom::Rect;
use crate::presence::{AWAY_AFTER_SECS, BACK_BELOW_SECS};

/// 두 번 본 사이 벽시계가 이만큼 넘게 흘렀으면 컴퓨터가 잤던 것 (보통 1초)
pub const SLEEP_GAP_SECS: f64 = 30.0;
/// 모니터 구성이 바뀐 뒤 이만큼 그대로면 새로 고침 (모니터가 하나씩 깨어나는 동안 여러 번 하지 않게)
pub const DISPLAY_SETTLE_SECS: f64 = 3.0;

/// 새로 고치는 까닭임 (기록용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeReason {
    /// 컴퓨터가 잤다 깼음
    Slept,
    /// 오래 비웠다 돌아옴 (잠금 풀기 포함)
    Back,
    /// 모니터 구성이 바뀌었음
    Display,
}

impl WakeReason {
    /// 기록에 남길 짧은 이름
    pub fn name(self) -> &'static str {
        match self {
            WakeReason::Slept => "slept",
            WakeReason::Back => "back",
            WakeReason::Display => "display",
        }
    }
}

/// 깨어남 감시 상태임 (게임 루프가 가짐)
#[derive(Debug, Clone, Default)]
pub struct WakeWatch {
    /// 지난번 벽시계 (초)
    last_wall: Option<f64>,
    /// 오래 비워 둔 중 (유휴 시간이 AWAY_AFTER_SECS를 넘었음)
    away: bool,
    /// 지난번 모니터 작업 영역들
    monitors: Option<Vec<Rect>>,
    /// 모니터 구성이 바뀐 벽시계 (잠잠해지길 기다리는 중)
    changed_at: Option<f64>,
}

impl WakeWatch {
    /// 1초에 한 번쯤. wall = 벽시계 초 (UNIX), idle = 마지막 입력 뒤 초 (모르면 None), monitors = 지금 작업 영역들.
    /// 새로 고칠 때면 그 까닭
    pub fn observe(&mut self, wall: f64, idle: Option<f64>, monitors: &[Rect]) -> Option<WakeReason> {
        let mut reason = None;
        if let Some(last) = self.last_wall.replace(wall) {
            if wall - last > SLEEP_GAP_SECS {
                reason = Some(WakeReason::Slept);
            }
        }
        if let Some(idle) = idle {
            if idle >= AWAY_AFTER_SECS {
                self.away = true;
            } else if self.away && idle < BACK_BELOW_SECS {
                self.away = false;
                reason = reason.or(Some(WakeReason::Back));
            }
        }
        match &self.monitors {
            Some(old) if old.as_slice() != monitors => {
                self.monitors = Some(monitors.to_vec());
                self.changed_at = Some(wall);
            }
            None => self.monitors = Some(monitors.to_vec()),
            _ => {}
        }
        if let Some(at) = self.changed_at {
            if reason.is_some() {
                // 어차피 새로 고침 — 모니터 몫도 같이 끝남
                self.changed_at = None;
            } else if wall - at >= DISPLAY_SETTLE_SECS && !monitors.is_empty() {
                self.changed_at = None;
                reason = Some(WakeReason::Display);
            }
        }
        reason
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 모니터 하나 (1920×1040)
    fn one() -> Vec<Rect> {
        vec![Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1040.0 }]
    }

    /// 모니터 둘
    fn two() -> Vec<Rect> {
        let mut m = one();
        m.push(Rect { left: 1920.0, top: 0.0, right: 3840.0, bottom: 1040.0 });
        m
    }

    #[test]
    fn a_normal_second_is_nothing() {
        let mut w = WakeWatch::default();
        for t in 0..100 {
            assert_eq!(w.observe(1000.0 + t as f64, Some(1.0), &one()), None);
        }
    }

    #[test]
    fn a_jump_of_the_wall_clock_means_the_computer_slept() {
        let mut w = WakeWatch::default();
        assert_eq!(w.observe(1000.0, Some(1.0), &one()), None);
        assert_eq!(w.observe(1001.0, Some(1.0), &one()), None);
        assert_eq!(w.observe(1001.0 + 8.0 * 3600.0, Some(1.0), &one()), Some(WakeReason::Slept));
        assert_eq!(w.observe(1002.0 + 8.0 * 3600.0, Some(1.0), &one()), None);
    }

    #[test]
    fn coming_back_after_a_long_lock_refreshes_once() {
        let mut w = WakeWatch::default();
        let mut t = 0.0;
        // 밤새 잠금: 유휴 시간이 계속 늚 (컴퓨터는 깨어 있음)
        while t < 10.0 * 3600.0 {
            assert_eq!(w.observe(t, Some(t), &one()), None);
            t += 1.0;
        }
        // 잠금을 풀고 손을 댐
        assert_eq!(w.observe(t, Some(0.5), &one()), Some(WakeReason::Back));
        assert_eq!(w.observe(t + 1.0, Some(0.2), &one()), None);
    }

    #[test]
    fn a_short_break_is_not_a_long_away() {
        let mut w = WakeWatch::default();
        for t in 0..300 {
            w.observe(t as f64, Some(t as f64), &one());
        }
        assert_eq!(w.observe(301.0, Some(0.5), &one()), None);
    }

    #[test]
    fn an_os_that_cannot_tell_idle_only_uses_the_clock() {
        let mut w = WakeWatch::default();
        assert_eq!(w.observe(0.0, None, &one()), None);
        assert_eq!(w.observe(1.0, None, &one()), None);
        assert_eq!(w.observe(100.0, None, &one()), Some(WakeReason::Slept));
    }

    #[test]
    fn a_monitor_change_refreshes_once_it_has_settled() {
        let mut w = WakeWatch::default();
        assert_eq!(w.observe(0.0, Some(1.0), &two()), None);
        // 모니터가 꺼졌다가 (구성이 바뀜) 다시 켜지는 동안엔 기다림
        assert_eq!(w.observe(1.0, Some(1.0), &one()), None);
        assert_eq!(w.observe(2.0, Some(1.0), &two()), None);
        assert_eq!(w.observe(3.0, Some(1.0), &two()), None);
        assert_eq!(w.observe(4.0, Some(1.0), &two()), None);
        assert_eq!(w.observe(5.0, Some(1.0), &two()), Some(WakeReason::Display));
        assert_eq!(w.observe(6.0, Some(1.0), &two()), None);
    }

    #[test]
    fn no_monitor_at_all_waits_until_one_comes_back() {
        let mut w = WakeWatch::default();
        w.observe(0.0, Some(1.0), &one());
        for t in 1..20 {
            assert_eq!(w.observe(t as f64, Some(1.0), &[]), None);
        }
        assert_eq!(w.observe(20.0, Some(1.0), &one()), None);
        assert_eq!(w.observe(23.0, Some(1.0), &one()), Some(WakeReason::Display));
    }

    #[test]
    fn waking_with_a_new_monitor_set_refreshes_only_once() {
        let mut w = WakeWatch::default();
        w.observe(0.0, Some(1.0), &two());
        assert_eq!(w.observe(5000.0, Some(1.0), &one()), Some(WakeReason::Slept));
        for t in 1..10 {
            assert_eq!(w.observe(5000.0 + t as f64, Some(1.0), &one()), None);
        }
    }
}
