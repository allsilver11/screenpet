//! 💤 떠 있는 창을 새로 고칠 때를 1초마다 봄: 화면 잠금을 풀었음 (OS 알림 — Windows WTS_SESSION_UNLOCK),
//! 컴퓨터가 잤다 깼음 (OS 알림 또는 벽시계가 훌쩍 뜀), 모니터 구성(테두리·배율)이 바뀌고 잠잠해짐.
//! Windows WebView2는 긴 잠금·절전·모니터 끊김 뒤 투명 창의 그림을 잃어 (창은 "보임"인데 텅 빔) 펫이 안 보이던 버그가 있었음 —
//! 앱이 이때 웹뷰를 다시 읽고 창을 다시 보임 (game_loop.rs refresh_windows).
//! 잠겨 있는 동안엔 미뤘다가 풀린 뒤 한 번만, 깬 직후(GPU가 덜 깼을 때)는 조금 기다렸다가, 모니터 바뀜은 자주 하지 않음.
//! 그냥 오래 자리를 비운 것(유휴 시간)으로는 새로 고치지 않음 — 다시 읽으면 웹뷰가 들고 있던 것이 사라질 수 있음.
use crate::geom::Rect;

/// 두 번 본 사이 벽시계가 이만큼 넘게 흘렀으면 컴퓨터가 잤던 것 (보통 1초)
pub const SLEEP_GAP_SECS: f64 = 30.0;
/// 잠금을 푼 뒤 이만큼 기다렸다가 새로 고침 (화면이 다시 그려질 틈)
pub const UNLOCK_DELAY_SECS: f64 = 1.5;
/// 깬 뒤 이만큼 기다렸다가 새로 고침 (GPU·디스플레이가 다 깨길 기다림)
pub const SLEEP_SETTLE_SECS: f64 = 4.0;
/// 모니터 구성이 바뀐 뒤 이만큼 그대로면 새로 고침 (모니터가 하나씩 깨어나는 동안 여러 번 하지 않게)
pub const DISPLAY_SETTLE_SECS: f64 = 3.0;
/// 모니터 바뀜으로 하는 새로 고침은 지난 새로 고침에서 이만큼 지난 뒤에만
pub const DISPLAY_MIN_GAP_SECS: f64 = 60.0;
/// 새로 고친 뒤 이만큼 안에 또 할 일이 생기면 버림 (깸·풀기·모니터가 한꺼번에 와도 한 번)
pub const DEDUPE_SECS: f64 = 10.0;

/// OS가 알려 준 세션 알림 수 (앱이 켜진 뒤 센 누적 값 — 줄지 않음)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SessionEvents {
    /// 화면을 잠갔음
    pub locks: u32,
    /// 잠금을 풀었음
    pub unlocks: u32,
    /// 절전에서 깼음
    pub resumes: u32,
}

/// 모니터 하나 (테두리 전체 — 작업 영역이 아님: 작업 표시줄·메뉴 막대 숨김으로는 바뀌지 않게 — 와 배율)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Monitor {
    /// 모니터 테두리 (물리 px)
    pub frame: Rect,
    /// 화면 배율
    pub scale: f64,
}

/// 새로 고치는 까닭임 (기록용 — 앞의 것이 더 셈)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WakeReason {
    /// 모니터 구성이 바뀌었음
    Display,
    /// 컴퓨터가 잤다 깼음
    Slept,
    /// 화면 잠금을 풀었음
    Unlocked,
}

impl WakeReason {
    /// 기록에 남길 짧은 이름
    pub fn name(self) -> &'static str {
        match self {
            WakeReason::Slept => "slept",
            WakeReason::Unlocked => "unlocked",
            WakeReason::Display => "display",
        }
    }
}

/// 깨어남 감시 상태임 (게임 루프가 가짐)
#[derive(Debug, Clone, Default)]
pub struct WakeWatch {
    /// 지난번 벽시계 (초)
    last_wall: Option<f64>,
    /// 지난번 세션 알림 수
    last_events: SessionEvents,
    /// 지금 화면이 잠겨 있음 (OS가 알려 줄 때만 — 아니면 늘 false)
    locked: bool,
    /// 할 새로 고침 (때, 까닭)
    pending: Option<(f64, WakeReason)>,
    /// 지난번 모니터들
    monitors: Option<Vec<Monitor>>,
    /// 모니터 구성이 바뀐 때 (잠잠해지길 기다리는 중)
    changed_at: Option<f64>,
    /// 지난 새로 고침 때
    last_refresh: Option<f64>,
}

impl WakeWatch {
    /// 할 새로 고침을 잡음: 이미 있으면 늦은 때·센 까닭으로 합침
    fn schedule(&mut self, due: f64, reason: WakeReason) {
        self.pending = Some(match self.pending {
            Some((d, r)) => (d.max(due), r.max(reason)),
            None => (due, reason),
        });
    }

    /// 1초에 한 번쯤. now = 앱이 켜진 뒤 초 (단조), wall = 벽시계 초 (UNIX), events = OS 세션 알림 누적 수, monitors = 지금 모니터들.
    /// 지금 새로 고칠 때면 그 까닭
    pub fn observe(&mut self, now: f64, wall: f64, events: SessionEvents, monitors: &[Monitor]) -> Option<WakeReason> {
        let unlocked = events.unlocks > self.last_events.unlocks;
        let resumed = events.resumes > self.last_events.resumes;
        self.last_events = events;
        self.locked = events.locks > events.unlocks;
        if unlocked {
            self.schedule(now + UNLOCK_DELAY_SECS, WakeReason::Unlocked);
        }
        let jumped = self.last_wall.replace(wall).is_some_and(|last| wall - last > SLEEP_GAP_SECS);
        if resumed || jumped {
            self.schedule(now + SLEEP_SETTLE_SECS, WakeReason::Slept);
        }
        match &self.monitors {
            Some(old) if old.as_slice() != monitors => {
                self.monitors = Some(monitors.to_vec());
                self.changed_at = Some(now);
            }
            None => self.monitors = Some(monitors.to_vec()),
            _ => {}
        }
        if self.changed_at.is_some_and(|at| now - at >= DISPLAY_SETTLE_SECS) && !monitors.is_empty() {
            self.changed_at = None;
            let gap = self.last_refresh.map_or(now, |t| t + DISPLAY_MIN_GAP_SECS);
            self.schedule(now.max(gap), WakeReason::Display);
        }
        // 잠겨 있는 동안엔 미룸 (풀리면 한 번에)
        let (due, reason) = self.pending?;
        if self.locked || now < due {
            return None;
        }
        self.pending = None;
        if self.last_refresh.is_some_and(|t| now - t < DEDUPE_SECS) {
            return None;
        }
        self.last_refresh = Some(now);
        Some(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 모니터 하나 (1920×1080, 배율 1)
    fn one() -> Vec<Monitor> {
        vec![Monitor { frame: Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1080.0 }, scale: 1.0 }]
    }

    /// 모니터 둘
    fn two() -> Vec<Monitor> {
        let mut m = one();
        m.push(Monitor { frame: Rect { left: 1920.0, top: 0.0, right: 3840.0, bottom: 1080.0 }, scale: 1.0 });
        m
    }

    /// 1초씩 from..to를 돌며 나온 까닭들 (때, 까닭)
    fn run(w: &mut WakeWatch, from: u32, to: u32, events: SessionEvents, monitors: &[Monitor]) -> Vec<(u32, WakeReason)> {
        (from..to).filter_map(|t| w.observe(t as f64, 1000.0 + t as f64, events, monitors).map(|r| (t, r))).collect()
    }

    #[test]
    fn a_normal_day_is_nothing_even_after_a_long_idle() {
        // 유휴 시간은 보지 않음 (긴 동영상·점심시간마다 다시 읽지 않게)
        let mut w = WakeWatch::default();
        assert!(run(&mut w, 0, 4000, SessionEvents::default(), &one()).is_empty());
    }

    #[test]
    fn unlocking_refreshes_once_a_moment_later() {
        let mut w = WakeWatch::default();
        let locked = SessionEvents { locks: 1, ..Default::default() };
        let open = SessionEvents { locks: 1, unlocks: 1, resumes: 0 };
        assert!(run(&mut w, 0, 10, SessionEvents::default(), &one()).is_empty());
        assert!(run(&mut w, 10, 5000, locked, &one()).is_empty());
        assert_eq!(run(&mut w, 5000, 5010, open, &one()), vec![(5002, WakeReason::Unlocked)]);
    }

    #[test]
    fn waking_from_sleep_waits_for_the_gpu() {
        let mut w = WakeWatch::default();
        assert_eq!(w.observe(0.0, 1000.0, SessionEvents::default(), &one()), None);
        // 벽시계가 8시간 뜀 (단조 시계는 거의 그대로일 수 있음)
        assert_eq!(w.observe(1.0, 1000.0 + 8.0 * 3600.0, SessionEvents::default(), &one()), None);
        let wall = 1000.0 + 8.0 * 3600.0;
        let got: Vec<_> = (2..10).filter_map(|t| w.observe(t as f64, wall + t as f64, SessionEvents::default(), &one()).map(|r| (t, r))).collect();
        assert_eq!(got, vec![(5, WakeReason::Slept)]);
    }

    #[test]
    fn the_os_resume_notice_also_counts_as_sleep() {
        let mut w = WakeWatch::default();
        run(&mut w, 0, 5, SessionEvents::default(), &one());
        let woke = SessionEvents { resumes: 1, ..Default::default() };
        assert_eq!(run(&mut w, 5, 20, woke, &one()), vec![(9, WakeReason::Slept)]);
    }

    #[test]
    fn sleeping_while_locked_refreshes_only_once_after_unlocking() {
        let mut w = WakeWatch::default();
        let locked = SessionEvents { locks: 1, ..Default::default() };
        run(&mut w, 0, 10, locked, &two());
        // 자는 동안 모니터가 끊겼다가 깸 — 아직 잠겨 있음
        let woke = SessionEvents { locks: 1, unlocks: 0, resumes: 1 };
        assert!(run(&mut w, 10, 100, woke, &one()).is_empty());
        let open = SessionEvents { locks: 1, unlocks: 1, resumes: 1 };
        assert_eq!(run(&mut w, 100, 300, open, &one()), vec![(102, WakeReason::Unlocked)]);
    }

    #[test]
    fn a_monitor_change_refreshes_once_it_has_settled() {
        let mut w = WakeWatch::default();
        let e = SessionEvents::default();
        run(&mut w, 0, 100, e, &two());
        // 꺼졌다 켜지는 동안엔 기다림
        assert!(run(&mut w, 100, 101, e, &one()).is_empty());
        assert_eq!(run(&mut w, 101, 120, e, &two()), vec![(104, WakeReason::Display)]);
    }

    #[test]
    fn only_the_monitor_frame_and_scale_count() {
        let mut w = WakeWatch::default();
        let e = SessionEvents::default();
        run(&mut w, 0, 100, e, &one());
        let mut hidpi = one();
        hidpi[0].scale = 1.5;
        assert_eq!(run(&mut w, 100, 120, e, &hidpi), vec![(103, WakeReason::Display)]);
    }

    #[test]
    fn monitor_changes_do_not_storm() {
        let mut w = WakeWatch::default();
        let e = SessionEvents::default();
        run(&mut w, 0, 100, e, &one());
        let mut got = Vec::new();
        // 10초마다 모니터가 바뀜 (도킹을 들락날락)
        for k in 0..12u32 {
            let m = if k % 2 == 0 { two() } else { one() };
            got.extend(run(&mut w, 100 + k * 10, 110 + k * 10, e, &m));
        }
        got.extend(run(&mut w, 220, 400, e, &one()));
        assert!(got.len() <= 3, "{got:?}");
        for pair in got.windows(2) {
            assert!(pair[1].0 - pair[0].0 >= DISPLAY_MIN_GAP_SECS as u32, "{got:?}");
        }
    }

    #[test]
    fn waking_with_a_new_monitor_set_refreshes_only_once() {
        let mut w = WakeWatch::default();
        run(&mut w, 0, 5, SessionEvents::default(), &two());
        let woke = SessionEvents { resumes: 1, ..Default::default() };
        assert_eq!(run(&mut w, 5, 200, woke, &one()), vec![(9, WakeReason::Slept)]);
    }
}
