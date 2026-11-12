//! 🧹 빗자루: 메뉴로 켜면 커서를 따라다니고, 움직이는 것만으로 지나가는 더미(눈·꽃잎·낙엽)를 씀. 클릭은 그 아래로 감
//! 왼쪽·오른쪽 버튼·Esc·20초 동안 안 움직이면 내려놓음
//! 쓴 것을 들고 친구가 연결된 화면 끝으로 계속 밀면 친구 화면에 흩날림 (물건 던지기 통로로 보냄, weather::flurry_kind).

use crate::geom::Rect;
use crate::visit::{edge_fraction, Fling, Side};

/// 안 움직이면 내려놓는 시간 (초)
pub const IDLE_SECS: f64 = 20.0;
/// 쓸리는 폭: 커서에서 좌우로 (논리 px)
const REACH_X: f64 = 36.0;
/// 쓸리는 높이: 더미 윗면에서 위아래로 (논리 px)
const REACH_Y: f64 = 40.0;
/// 쓴 양이 이만큼 있어야 친구에게 넘어감 (더미 높이의 합, 논리 px)
const MIN_CARRY: f32 = 4.0;
/// 들고 있는 양이 반으로 주는 시간 (초): 방금 쓴 것만 밀고 갈 수 있음
const CARRY_HALF_LIFE: f64 = 3.0;
/// 화면 끝에서 이만큼 안쪽부터 넘어감 (논리 px, 커서는 화면 끝에서 멈추므로 닿기 전에)
const EDGE_ZONE: f64 = 48.0;
/// 끝으로 이 속도 이상 밀어야 넘어감 (논리 px/s)
const PUSH_SPEED: f64 = 120.0;
/// 쓸 때 날리는 알갱이: 간격(초), 한 번에 최대
const PUFF_EVERY: f64 = 0.1;
const PUFF_MAX: u32 = 8;

/// 커서를 따라다니는 빗자루 하나
#[derive(Debug, Clone)]
pub struct Broom {
    at: (f64, f64),
    from_x: f64,
    velocity: (f64, f64),
    idle: f64,
    /// 켤 때 누르고 있던 버튼은 내려놓기가 아님 (메뉴를 누른 손)
    left_was: bool,
    right_was: bool,
    carried: f32,
    puff: f32,
    puff_wait: f64,
}

impl Broom {
    /// 커서 자리에서 빗자루를 듦 (켤 때 누르고 있던 버튼은 세지 않음)
    pub fn new(cursor: (f64, f64)) -> Self {
        Self { at: cursor, from_x: cursor.0, velocity: (0.0, 0.0), idle: 0.0, left_was: true, right_was: true, carried: 0.0, puff: 0.0, puff_wait: 0.0 }
    }

    /// 매 틱. true면 내려놓음
    pub fn tick(&mut self, dt: f64, cursor: (f64, f64), left: bool, right: bool, escape: bool) -> bool {
        let clicked = (left && !self.left_was) || (right && !self.right_was);
        self.left_was = left;
        self.right_was = right;
        let moved = cursor != self.at;
        self.from_x = self.at.0;
        if dt > 0.0 {
            self.velocity = ((cursor.0 - self.at.0) / dt, (cursor.1 - self.at.1) / dt);
        }
        self.at = cursor;
        self.idle = if moved { 0.0 } else { self.idle + dt };
        self.carried *= 0.5f32.powf((dt / CARRY_HALF_LIFE) as f32);
        self.puff_wait = (self.puff_wait - dt).max(0.0);
        clicked || escape || self.idle >= IDLE_SECS
    }

    /// 이번 틱에 지나간 폭 (물리 px): (왼쪽, 오른쪽)
    pub fn span(&self, dpi: f64) -> (f64, f64) {
        let r = REACH_X * dpi;
        (self.at.0.min(self.from_x) - r, self.at.0.max(self.from_x) + r)
    }

    /// 쓸리는 높이 (물리 px)
    pub fn reach_y(dpi: f64) -> f64 {
        REACH_Y * dpi
    }

    /// 이번 틱에 (x, y)의 바닥에 놓인 것 위를 지나갔음 (움직이는 중일 때만)
    pub fn passes_over(&self, x: f64, y: f64, dpi: f64) -> bool {
        let (x0, x1) = self.span(dpi);
        self.velocity != (0.0, 0.0) && x0 <= x && x <= x1 && (self.at.1 - y).abs() <= Self::reach_y(dpi)
    }

    /// 빗자루 속도 (물리 px/s)
    pub fn velocity(&self) -> (f64, f64) {
        self.velocity
    }

    /// 쓴 양을 듦. 알갱이를 날릴 때가 됐으면 그 수
    pub fn swept(&mut self, amount: f32) -> Option<u32> {
        self.carried += amount;
        self.puff += amount;
        if self.puff <= 0.0 || self.puff_wait > 0.0 {
            return None;
        }
        self.puff_wait = PUFF_EVERY;
        let count = (self.puff.ceil() as u32).min(PUFF_MAX);
        self.puff = 0.0;
        Some(count)
    }

    /// 쓴 것을 들고 edges의 끝으로 밀고 있으면 그 끝 (open: 그 끝에 연결된 친구가 있음). 넘겼으니 든 것은 비움
    /// 세로 끝은 edges에 그 줄의 끝을 줌 (visit::column_bounds)
    pub fn crossing(&mut self, edges: &Rect, dpi: f64, open: impl Fn(Side) -> bool) -> Option<Side> {
        if self.carried < MIN_CARRY {
            return None;
        }
        let (vx, vy) = self.velocity;
        let (zone, push) = (EDGE_ZONE * dpi, PUSH_SPEED * dpi);
        let side = if vx.abs() >= vy.abs() {
            if vx <= -push && self.at.0 <= edges.left + zone {
                Side::Left
            } else if vx >= push && self.at.0 >= edges.right - zone {
                Side::Right
            } else {
                return None;
            }
        } else if vy <= -push && self.at.1 <= edges.top + zone {
            Side::Up
        } else if vy >= push && self.at.1 >= edges.bottom - zone {
            Side::Down
        } else {
            return None;
        };
        if !open(side) {
            return None;
        }
        self.carried = 0.0;
        Some(side)
    }

    /// 넘어가는 움직임 (논리 px/s): height = 끝을 따라 어디쯤 (왼쪽·오른쪽은 위→아래, 위·아래는 왼쪽→오른쪽 비율)
    pub fn fling(&self, side: Side, edges: &Rect, dpi: f64) -> Fling {
        let height = match side {
            Side::Up | Side::Down => edge_fraction(edges, self.at.0),
            Side::Left | Side::Right if edges.height() > 0.0 => ((self.at.1 - edges.top) / edges.height()).clamp(0.0, 1.0),
            _ => 0.5,
        };
        let dpi = dpi.max(0.01);
        Fling { height, vx: self.velocity.0 / dpi, vy: self.velocity.1 / dpi }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;
    const SCREEN: Rect = Rect { left: 0.0, top: 0.0, right: 1920.0, bottom: 1040.0 };

    /// 테스트: 한 틱 움직여도 내려놓지 않음
    fn moving(b: &mut Broom, to: (f64, f64)) {
        assert!(!b.tick(DT, to, false, false, false));
    }

    #[test]
    fn it_ends_on_a_new_click_escape_or_when_left_alone() {
        // 메뉴를 누르던 손을 떼기 전까지는 끝나지 않는다
        let mut b = Broom::new((10.0, 10.0));
        assert!(!b.tick(DT, (11.0, 10.0), true, false, false));
        assert!(!b.tick(DT, (12.0, 10.0), false, false, false));
        assert!(b.tick(DT, (13.0, 10.0), true, false, false));
        let mut b = Broom::new((10.0, 10.0));
        b.tick(DT, (10.0, 10.0), false, false, false);
        assert!(b.tick(DT, (10.0, 10.0), false, true, false));
        let mut b = Broom::new((10.0, 10.0));
        assert!(b.tick(DT, (10.0, 10.0), false, false, true));
        let mut b = Broom::new((10.0, 10.0));
        b.tick(DT, (10.0, 10.0), false, false, false);
        assert!(!b.tick(IDLE_SECS - 1.0, (10.0, 10.0), false, false, false));
        assert!(!b.tick(0.5, (11.0, 10.0), false, false, false), "moving keeps it");
        assert!(!b.tick(IDLE_SECS - 1.0, (11.0, 10.0), false, false, false));
        assert!(b.tick(1.0, (11.0, 10.0), false, false, false));
    }

    #[test]
    fn the_span_covers_the_whole_stroke_even_on_a_fast_flick() {
        let mut b = Broom::new((100.0, 0.0));
        moving(&mut b, (400.0, 0.0));
        assert_eq!(b.span(1.0), (64.0, 436.0));
        assert_eq!(b.span(2.0), (28.0, 472.0));
    }

    #[test]
    fn it_only_passes_over_what_is_under_a_moving_stroke() {
        let mut b = Broom::new((100.0, 1000.0));
        moving(&mut b, (200.0, 1000.0));
        assert!(b.passes_over(150.0, 1040.0, 1.0), "under the stroke");
        assert!(b.passes_over(236.0, 1000.0, 1.0), "at the end of the reach");
        assert!(!b.passes_over(300.0, 1000.0, 1.0), "too far ahead");
        assert!(!b.passes_over(150.0, 700.0, 1.0), "on a window far above");
        // 멈춰 있으면 쓸지 않는다
        b.tick(DT, (200.0, 1000.0), false, false, false);
        assert!(!b.passes_over(200.0, 1000.0, 1.0));
    }

    #[test]
    fn puffs_are_limited_in_rate_and_size() {
        let mut b = Broom::new((0.0, 0.0));
        assert_eq!(b.swept(0.0), None);
        assert_eq!(b.swept(100.0), Some(PUFF_MAX));
        assert_eq!(b.swept(1.0), None, "too soon");
        moving(&mut b, (1.0, 0.0));
        for _ in 0..7 {
            b.tick(DT, (1.0, 0.0), false, false, false);
        }
        assert_eq!(b.swept(0.0), Some(1), "what was held back comes out");
    }

    #[test]
    fn only_a_recent_sweep_pushed_at_a_linked_edge_crosses() {
        let open = |_: Side| true;
        let mut b = Broom::new((1800.0, 900.0));
        moving(&mut b, (1890.0, 900.0));
        assert_eq!(b.crossing(&SCREEN, 1.0, open), None, "nothing swept");
        b.swept(10.0);
        assert_eq!(b.crossing(&SCREEN, 1.0, |_| false), None, "no friend there");
        assert_eq!(b.crossing(&SCREEN, 1.0, open), Some(Side::Right));
        assert_eq!(b.crossing(&SCREEN, 1.0, open), None, "it was handed over");
        // 안쪽으로 밀거나 끝에서 먼 곳은 넘어가지 않는다
        let mut b = Broom::new((1890.0, 900.0));
        b.swept(10.0);
        moving(&mut b, (1800.0, 900.0));
        assert_eq!(b.crossing(&SCREEN, 1.0, open), None);
        moving(&mut b, (1000.0, 900.0));
        moving(&mut b, (1100.0, 900.0));
        assert_eq!(b.crossing(&SCREEN, 1.0, open), None);
        // 쓴 지 오래면 다 날아갔다
        let mut b = Broom::new((1800.0, 900.0));
        b.swept(5.0);
        b.tick(CARRY_HALF_LIFE, (1800.0, 900.0), false, false, false);
        moving(&mut b, (1890.0, 900.0));
        assert_eq!(b.crossing(&SCREEN, 1.0, open), None);
    }

    #[test]
    fn it_crosses_every_edge_and_says_where_along_it() {
        for (from, to, side) in [
            ((100.0, 520.0), (20.0, 520.0), Side::Left),
            ((1800.0, 520.0), (1900.0, 520.0), Side::Right),
            ((480.0, 100.0), (480.0, 10.0), Side::Up),
            ((480.0, 950.0), (480.0, 1030.0), Side::Down),
        ] {
            let mut b = Broom::new(from);
            b.swept(10.0);
            moving(&mut b, to);
            assert_eq!(b.crossing(&SCREEN, 1.0, |_| true), Some(side));
            let f = b.fling(side, &SCREEN, 1.0);
            let want = if crate::visit::is_vertical(side) { 0.25 } else { 0.5 };
            assert!((f.height - want).abs() < 1e-9, "{side:?}: {}", f.height);
        }
    }
}
