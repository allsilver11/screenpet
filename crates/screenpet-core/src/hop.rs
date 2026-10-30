//! 폴짝 뛰는 동물(개구리, pet.json `locomotion: hop`)의 순수 계산 —
//! 걸음 대신 작은 폴짝 박자·높이, 벽을 폴짝폴짝 오르는 박자, 큰 점프 포물선과 목표 고르기.
//! Pet 쪽 연결은 brain_hop.rs.

use crate::physics::GRAVITY;
use crate::surfaces::{Owner, Surface};

/// 한 번 폴짝 떠 있는 시간 (초) — hop 그림(3프레임 10fps)과 맞춘다
pub const AIR_SECS: f64 = 0.32;
/// 폴짝 사이 최소 웅크림 (초)
pub const MIN_SQUAT_SECS: f64 = 0.08;
/// 폴짝 한 박자 최소 길이 (초) — 아주 빠르면 한 번 거리가 길어지는 대신 박자가 이보다 짧아지지 않는다
pub const MIN_CYCLE_SECS: f64 = 0.3;
/// 한 번 폴짝 거리 (× 크기): 걷기 / 달리기
pub const WALK_HOP_LEN: f64 = 0.6;
pub const RUN_HOP_LEN: f64 = 0.8;
/// 폴짝 높이 (× 크기): 걷기 / 달리기 — 너무 높지 않게
pub const WALK_HOP_HEIGHT: f64 = 0.28;
pub const RUN_HOP_HEIGHT: f64 = 0.33;
/// 깡총 뛰는 토끼(pet.json `hop_share`)의 폴짝 — 개구리보다 조금 낮고 조금 짧게 떠 있다 (hop 그림 3프레임 10fps는 다 보인다)
pub const BUNNY_AIR_SECS: f64 = 0.28;
pub const BUNNY_WALK_HOP_HEIGHT: f64 = 0.24;
pub const BUNNY_RUN_HOP_HEIGHT: f64 = 0.27;
/// 내려선 직후 이 시간(초) 동안은 착지(land) 그림, 그 뒤 웅크림이 남으면 앉기(idle) 그림
pub const LAND_SHOW_SECS: f64 = 0.2;

/// 벽 오르기: 한 번 폴짝 오르는 시간과 매달려 쉬는 시간 (초)
pub const CLIMB_HOP_SECS: f64 = 0.3;
pub const CLIMB_CLING_SECS: f64 = 0.4;

/// 닿을 수 있는 창 윗면이 있을 때 이동 결정 중 큰 점프 확률
pub const BIG_JUMP_CHANCE: f64 = 0.12;
/// 큰 점프로 닿는 범위 (× 크기): 위로, 아래로, 옆으로 (착지 지점까지)
pub const BIG_JUMP_UP: f64 = 3.5;
pub const BIG_JUMP_DOWN: f64 = 4.0;
pub const BIG_JUMP_REACH: f64 = 3.5;
/// 큰 점프가 출발·도착 중 높은 쪽보다 더 솟는 높이 (× 크기)
pub const BIG_JUMP_RISE: f64 = 0.5;
/// 큰 점프 전 깊게 웅크리는 시간 (초)
pub const BIG_JUMP_CROUCH_SECS: f64 = 0.5;
/// 착지 지점: 창 끝에서 몸 반 폭 + 이만큼(× 크기) 안쪽
pub const LANDING_INSET: f64 = 0.25;

/// 폴짝 걸음 한 박자: 떠 있기(air) → 웅크리기(squat). 박자 안 위치 t는 0 ≤ t < cycle
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gait {
    pub air: f64,
    pub squat: f64,
    /// 폴짝 최고 높이 (px)
    pub height: f64,
}

impl Gait {
    /// 크기 size, 평균 속도 speed(px/s)로 걷기(run=false)·달리기 폴짝 박자.
    /// 한 번 거리가 WALK/RUN_HOP_LEN × 크기가 되도록 박자를 늘린다 (너무 빠르면 최소 웅크림을 지킨다)
    pub fn new(size: f64, speed: f64, run: bool) -> Self {
        let height = if run { RUN_HOP_HEIGHT } else { WALK_HOP_HEIGHT };
        Self::with(size, speed, run, AIR_SECS, height)
    }

    /// 토끼의 깡총 걸음 — 박자 거리는 개구리와 같고, 조금 낮게·짧게 떠 있다
    pub fn bunny(size: f64, speed: f64, run: bool) -> Self {
        let height = if run { BUNNY_RUN_HOP_HEIGHT } else { BUNNY_WALK_HOP_HEIGHT };
        Self::with(size, speed, run, BUNNY_AIR_SECS, height)
    }

    fn with(size: f64, speed: f64, run: bool, air_secs: f64, height: f64) -> Self {
        let len = if run { RUN_HOP_LEN } else { WALK_HOP_LEN };
        let cycle = if speed > 0.0 { (len * size / speed).max(MIN_CYCLE_SECS) } else { MIN_CYCLE_SECS };
        // 빨리 달릴 땐 박자가 짧아지므로 떠 있는 시간도 줄인다 (웅크림은 최소 MIN_SQUAT_SECS)
        let air = air_secs.min(cycle - MIN_SQUAT_SECS);
        Self { air, squat: cycle - air, height: height * size }
    }

    pub fn cycle(&self) -> f64 {
        self.air + self.squat
    }

    pub fn airborne(&self, t: f64) -> bool {
        t < self.air
    }

    /// 박자 t에서 발이 바닥에서 떠 있는 높이 (px, 포물선)
    pub fn lift(&self, t: f64) -> f64 {
        if !self.airborne(t) {
            return 0.0;
        }
        let p = (t / self.air).clamp(0.0, 1.0);
        4.0 * self.height * p * (1.0 - p)
    }

    /// 박자 t에서 평균 속도에 곱할 배율 — 떠 있을 때만 앞으로 간다 (한 박자 평균 1)
    pub fn pace(&self, t: f64) -> f64 {
        if self.airborne(t) {
            self.cycle() / self.air
        } else {
            0.0
        }
    }

    /// 평균 속도 speed일 때 한 번 폴짝 거리
    pub fn hop_length(&self, speed: f64) -> f64 {
        speed * self.cycle()
    }
}

/// 이어지는 폴짝 박자 시계 — 틱마다 흘리고, 한 틱이라도 쉬면 다음엔 처음(떠오르기)부터
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Beat {
    t: f64,
    /// 마지막으로 흘린 틱의 시각 (Pet clock)
    last: Option<f64>,
}

impl Beat {
    /// clock 시각 틱에서 박자(길이 cycle)를 dt만큼 흘린다 (바로 앞 틱에 이어서, 아니면 0부터). 이번 틱의 박자 안 위치를 돌려준다
    pub fn tick(&mut self, clock: f64, dt: f64, cycle: f64) -> f64 {
        let continued = self.last.is_some_and(|l| l < clock && clock - l <= dt * 1.5);
        self.t = if continued { (self.t + dt) % cycle } else { 0.0 };
        self.last = Some(clock);
        self.t
    }

    /// 이번 틱(clock)에 흘렸다면 그 박자 안 위치
    pub fn at(&self, clock: f64) -> Option<f64> {
        (self.last == Some(clock)).then_some(self.t)
    }
}

/// 걷다가 가끔 깡총거리는 동물(토끼)의 걸음 고르기 — 걷기·달리기 한 토막마다 한 번 정한다
/// (토막 안에서는 그대로라 자연스럽게 한동안 걷고 한동안 깡총거린다)
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stretch {
    /// 이번 토막은 깡총
    pub on: bool,
    /// 마지막으로 본 틱 (clock, 이동 종류 번호, 남은 시간)
    last: Option<(f64, u8, f64)>,
}

impl Stretch {
    /// clock 틱의 이동 토막(종류 kind, 남은 시간 remaining)에서 깡총인지. 앞 틱에 이어지지 않거나(쉬었다)
    /// 종류가 바뀌거나 남은 시간이 늘었으면 새 토막 — 그때만 pick()으로 새로 고른다
    pub fn tick(&mut self, clock: f64, dt: f64, kind: u8, remaining: f64, pick: impl FnOnce() -> bool) -> bool {
        let same = self.last.is_some_and(|(c, k, r)| c < clock && clock - c <= dt * 1.5 && k == kind && remaining <= r);
        if !same {
            self.on = pick();
        }
        self.last = Some((clock, kind, remaining));
        self.on
    }

    /// 이번 틱(clock)에 고른 토막이 깡총인지
    pub fn at(&self, clock: f64) -> bool {
        self.on && self.last.is_some_and(|(c, ..)| c == clock)
    }
}

/// 벽 오르기 박자 길이
pub fn climb_cycle() -> f64 {
    CLIMB_HOP_SECS + CLIMB_CLING_SECS
}

/// 벽 오르기 박자 t에서 오르는 속도 배율 — 폴짝 오를 때만 올라가고 매달려 있을 땐 멈춘다 (평균 1)
pub fn climb_pace(t: f64) -> f64 {
    if t < CLIMB_HOP_SECS {
        climb_cycle() / CLIMB_HOP_SECS
    } else {
        0.0
    }
}

/// 큰 점프 포물선 (충돌 없이 따라간다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub vx: f64,
    pub vy: f64,
    pub secs: f64,
}

impl Arc {
    /// 출발점 from에서 t초 뒤 위치
    pub fn at(&self, from: (f64, f64), t: f64) -> (f64, f64) {
        (from.0 + self.vx * t, from.1 + self.vy * t + 0.5 * GRAVITY * t * t)
    }

    /// 꼭대기 발 높이 (출발 높이 y0 기준)
    pub fn apex_y(&self, y0: f64) -> f64 {
        y0 - self.vy * self.vy / (2.0 * GRAVITY)
    }
}

/// from에서 to로 가는 포물선. 꼭대기는 둘 중 높은 쪽보다 rise만큼 위, 단 top_limit(발 높이)보다 위로는 안 간다
pub fn leap_arc(from: (f64, f64), to: (f64, f64), rise: f64, top_limit: f64) -> Arc {
    let low = from.1.min(to.1);
    let apex = (low - rise).max(top_limit).min(low);
    let vy = -(2.0 * GRAVITY * (from.1 - apex)).sqrt();
    let secs = (-vy / GRAVITY + (2.0 * (to.1 - apex) / GRAVITY).sqrt()).max(1e-3);
    Arc { vx: (to.0 - from.0) / secs, vy, secs }
}

/// 큰 점프로 내려설 곳
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpTarget {
    pub owner: Owner,
    /// 착지 발 x
    pub x: f64,
    /// 착지 발 높이 (창 윗면)
    pub y: f64,
}

/// (x, y)에 서 있는 동물(크기 size, 몸 반 폭 half_w, 지금 디딤판 from)이 큰 점프로 닿을 수 있는 다른 창 윗면들.
/// 창을 뚫고 지나가지 않도록 옆에서만 뛴다 (발 x가 그 조각 위·아래에 있으면 제외). 착지는 가까운 끝 안쪽
pub fn jump_targets(surfaces: &[Surface], from: Owner, x: f64, y: f64, size: f64, half_w: f64) -> Vec<JumpTarget> {
    let inset = half_w + LANDING_INSET * size;
    surfaces
        .iter()
        .filter(|s| matches!(s.owner, Owner::Window(_)) && s.owner != from)
        .filter(|s| {
            let up = y - s.y;
            up <= BIG_JUMP_UP * size && -up <= BIG_JUMP_DOWN * size
        })
        .filter(|s| x < s.x_start - half_w || x > s.x_end + half_w)
        .filter_map(|s| {
            let (a, b) = (s.x_start + inset, s.x_end - inset);
            if a > b {
                return None;
            }
            let tx = if x < a { a } else { b };
            ((tx - x).abs() <= BIG_JUMP_REACH * size).then_some(JumpTarget { owner: s.owner, x: tx, y: s.y })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surfaces::EndKind;

    const SIZE: f64 = 108.8;

    fn sample(g: &Gait, n: usize) -> impl Iterator<Item = f64> + '_ {
        (0..n).map(move |i| g.cycle() * i as f64 / n as f64)
    }

    #[test]
    fn hops_are_low_and_short_at_walk_and_run_speeds() {
        for speed in [40.0, 63.0, 90.0, 160.0, 225.0] {
            for run in [false, true] {
                let g = Gait::new(SIZE, speed, run);
                let max = sample(&g, 1000).map(|t| g.lift(t)).fold(0.0, f64::max);
                assert!(max > 0.25 * SIZE && max <= 0.35 * SIZE, "speed {speed} run {run}: height {max}");
                let len = g.hop_length(speed);
                assert!(len <= 0.8 * SIZE + 1e-9, "speed {speed}: length {len}");
                assert!(g.squat >= MIN_SQUAT_SECS - 1e-12);
            }
        }
        // 보통 걷기·달리기 속도면 한 번에 몸 0.5~0.8배
        for (speed, run) in [(63.0, false), (225.0, true)] {
            let len = Gait::new(SIZE, speed, run).hop_length(speed);
            assert!((0.5 * SIZE..=0.8 * SIZE + 1e-9).contains(&len), "{len}");
        }
    }

    #[test]
    fn bunny_hops_are_lower_and_quicker_than_the_frogs_at_the_same_pace() {
        for speed in [60.0, 120.0, 300.0] {
            for run in [false, true] {
                let (b, f) = (Gait::bunny(SIZE, speed, run), Gait::new(SIZE, speed, run));
                assert!(b.height < f.height && b.air <= f.air && (speed > 200.0 || b.air < f.air), "speed {speed} run {run}");
                assert!((0.22 * SIZE..=0.28 * SIZE).contains(&b.height), "{}", b.height);
                assert!((b.cycle() - f.cycle()).abs() < 1e-12, "same hop length");
                assert!(b.squat >= MIN_SQUAT_SECS - 1e-12);
                let n = 10_000;
                let avg = (0..n).map(|i| b.pace(b.cycle() * i as f64 / n as f64)).sum::<f64>() / n as f64;
                assert!((avg - 1.0).abs() < 1e-3, "{avg}");
                let max = sample(&b, 1000).map(|t| b.lift(t)).fold(0.0, f64::max);
                assert!(max <= b.height + 1e-9);
            }
        }
    }

    #[test]
    fn stretch_picks_once_per_segment() {
        let dt = 0.1;
        let mut s = Stretch::default();
        let mut picks = 0;
        let mut tick = |s: &mut Stretch, clock: f64, kind: u8, rem: f64, v: bool| {
            s.tick(clock, dt, kind, rem, || {
                picks += 1;
                v
            })
        };
        assert!(tick(&mut s, 1.0, 0, 5.0, true));
        assert!(tick(&mut s, 1.1, 0, 4.9, false), "same segment keeps its choice");
        assert!(s.at(1.1) && !s.at(1.2));
        assert!(!tick(&mut s, 1.2, 0, 6.0, false), "remaining grew: new segment");
        assert!(tick(&mut s, 1.3, 1, 5.9, true), "kind changed: new segment");
        assert!(!tick(&mut s, 1.6, 1, 5.5, false), "a gap: new segment");
        drop(tick);
        assert_eq!(picks, 4);
    }

    #[test]
    fn hop_lifts_only_in_the_air_and_lands_at_zero() {
        let g = Gait::new(SIZE, 63.0, false);
        assert_eq!(g.lift(0.0), 0.0);
        assert!((g.lift(g.air / 2.0) - g.height).abs() < 1e-9);
        assert!(g.lift(g.air - 1e-9) < 1e-3);
        assert_eq!(g.lift(g.air + 0.01), 0.0);
        assert!(g.airborne(0.1) && !g.airborne(g.air + 0.01));
    }

    #[test]
    fn hop_pace_keeps_the_average_speed() {
        for run in [false, true] {
            let g = Gait::new(SIZE, 90.0, run);
            let n = 10_000;
            let avg = sample(&g, n).map(|t| g.pace(t)).sum::<f64>() / n as f64;
            assert!((avg - 1.0).abs() < 1e-3, "{avg}");
            assert_eq!(g.pace(g.air + 0.01), 0.0);
        }
        let n = 10_000;
        let avg = (0..n).map(|i| climb_pace(climb_cycle() * i as f64 / n as f64)).sum::<f64>() / n as f64;
        assert!((avg - 1.0).abs() < 1e-3, "{avg}");
        assert_eq!(climb_pace(CLIMB_HOP_SECS + 0.01), 0.0);
    }

    #[test]
    fn beat_continues_tick_to_tick_and_restarts_after_a_gap() {
        let dt = 0.1;
        let mut b = Beat::default();
        assert_eq!(b.tick(1.0, dt, 1.0), 0.0);
        assert!((b.tick(1.1, dt, 1.0) - 0.1).abs() < 1e-9);
        assert!((b.tick(1.2, dt, 0.15) - 0.05).abs() < 1e-9, "wraps at the cycle");
        assert_eq!(b.at(1.2), Some(b.t));
        assert_eq!(b.at(1.3), None);
        assert_eq!(b.tick(1.5, dt, 1.0), 0.0, "skipped ticks restart the beat");
    }

    #[test]
    fn leap_arc_reaches_the_target_and_peaks_above_both_ends() {
        for (from, to) in [((100.0, 1040.0), (400.0, 700.0)), ((400.0, 700.0), (100.0, 1040.0)), ((0.0, 500.0), (200.0, 500.0))] {
            let arc = leap_arc(from, to, 0.5 * SIZE, 0.0);
            let (x, y) = arc.at(from, arc.secs);
            assert!((x - to.0).abs() < 1e-6 && (y - to.1).abs() < 1e-6, "{from:?}->{to:?}: {x},{y}");
            let apex = arc.apex_y(from.1);
            assert!((apex - (from.1.min(to.1) - 0.5 * SIZE)).abs() < 1e-6, "{apex}");
            assert!(arc.vy < 0.0);
        }
    }

    #[test]
    fn leap_arc_keeps_the_apex_below_the_top_limit() {
        let arc = leap_arc((0.0, 400.0), (300.0, 200.0), 200.0, 150.0);
        assert!((arc.apex_y(400.0) - 150.0).abs() < 1e-6);
        let (x, y) = arc.at((0.0, 400.0), arc.secs);
        assert!((x - 300.0).abs() < 1e-6 && (y - 200.0).abs() < 1e-6);
    }

    fn top(id: u64, y: f64, a: f64, b: f64) -> Surface {
        Surface { owner: Owner::Window(id), y, x_start: a, x_end: b, start_kind: EndKind::Edge, end_kind: EndKind::Edge }
    }

    #[test]
    fn jump_targets_are_window_tops_beside_and_within_reach() {
        let half_w = 0.3 * SIZE;
        let inset = half_w + LANDING_INSET * SIZE;
        let ground = Surface { owner: Owner::Ground(0), y: 1040.0, x_start: 0.0, x_end: 1920.0, start_kind: EndKind::Wall, end_kind: EndKind::Wall };
        let surfaces = vec![
            top(1, 800.0, 400.0, 900.0),   // 오른쪽 위, 닿음
            top(2, 500.0, 400.0, 900.0),   // 너무 높다 (540)
            top(3, 900.0, 1500.0, 1800.0), // 너무 멀다
            top(4, 950.0, 100.0, 300.0),   // 왼쪽, 닿음 (착지 x는 오른쪽 끝 안쪽)
            top(5, 700.0, 250.0, 400.0),   // 발 바로 위 — 창을 뚫으므로 제외
            top(6, 900.0, 360.0, 380.0),   // 좁아서 설 자리 없음
            ground,
        ];
        let got = jump_targets(&surfaces, Owner::Ground(0), 360.0, 1040.0, SIZE, half_w);
        let ids: Vec<Owner> = got.iter().map(|t| t.owner).collect();
        assert_eq!(ids, vec![Owner::Window(1), Owner::Window(4)]);
        assert!((got[0].x - (400.0 + inset)).abs() < 1e-9 && got[0].y == 800.0);
        assert!((got[1].x - (300.0 - inset)).abs() < 1e-9);
    }

    #[test]
    fn jump_targets_skip_the_current_window_and_include_lower_ones() {
        let half_w = 0.3 * SIZE;
        let surfaces = vec![top(1, 500.0, 400.0, 900.0), top(2, 850.0, 1000.0, 1400.0), top(3, 990.0, 1000.0, 1400.0)];
        let got = jump_targets(&surfaces, Owner::Window(1), 880.0, 500.0, SIZE, half_w);
        // 2는 350 아래 (닿음), 3은 490 아래 (> 4 × 108.8 = 435, 못 닿음)
        assert_eq!(got.iter().map(|t| t.owner).collect::<Vec<_>>(), vec![Owner::Window(2)]);
    }
}
