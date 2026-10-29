//! 창 안에서 놀기 (spec 2B §9) — 수치와 순수 판정. brain.rs가 쓴다.
//!
//! 창 안에 있는 동물은 그 창의 아래 테두리 바로 위(안쪽 바닥, `Owner::Inside`)를 디디고, 공중에선 창 사각형이
//! 경계(옆 = 벽, 위 = 천장)다. 바깥 동물은 안쪽 바닥을 디디지 않고, 안쪽 동물은 바깥 디딤판을 디디지 않는다.

use crate::behavior;
use crate::geom::Rect;
use crate::surfaces::Owner;
use crate::world::World;

/// 안쪽 바닥·안쪽 벽이 창 테두리에서 들어온 거리 (px)
pub const INSET: f64 = 4.0;
/// 몸 반 폭 / 크기 (앱이 쓰는 비율 — 세계는 크기만 안다)
pub const HALF_W_RATIO: f64 = 0.3;
/// 창 높이가 몸 크기의 이 배 이상이어야 들어간다
pub const MIN_HEIGHT_MULT: f64 = 1.2;
/// 창 윗면에서 쉬기를 정할 때 그 창 안으로 뛰어들 확률
pub const HOP_IN_CHANCE: f64 = 0.05;
/// 창 안에 머무는 시간 (초) — 지나면 나간다
pub const STAY_SECS: (f64, f64) = (60.0, 180.0);
/// 벽 타는 동물이 뛰어나가는 대신 안쪽 벽을 기어올라 나갈 확률
pub const CLIMB_OUT_CHANCE: f64 = 0.5;
/// 기어 나가기로 한 뒤 이만큼(초) 지나도 벽에 못 닿았으면 그냥 뛰어나간다
pub const CLIMB_OUT_GIVE_UP_SECS: f64 = 30.0;
/// 뛰어나갈 때 창 윗면 위로 더 솟는 높이 (크기 128 기준 px)
pub const LEAVE_CLEAR: f64 = 40.0;

/// 창 안에서의 공중 국면
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 창 안 (천장 있음)
    In,
    /// 창 윗면에서 뛰어드는 중 — 몸이 다 들어올 때까지 천장 없음
    HopIn,
    /// 윗면 위로 뛰어나가는 중 — 천장 없음, 발이 윗면 위로 올라가면 바깥
    HopOut,
    /// 뛰어나가기가 두 번 실패한 뒤의 확실한 도약 — 벽·천장 무시, 꼭대기에서 윗면 위로 나간다
    LeapOut,
}

/// 창 안에 있는 동물의 상태
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stay {
    pub window: u64,
    pub mode: Mode,
    /// 나가기까지 남은 시간 (음수 = 지남)
    pub leave_in: f64,
    /// 안쪽 벽을 기어올라 나가려고 벽으로 가는 중
    pub climb_out: bool,
    /// 뛰어나가기를 시도한 횟수
    pub hops: u32,
}

impl Stay {
    pub fn new(window: u64, mode: Mode, rng: &mut fastrand::Rng) -> Self {
        Self { window, mode, leave_in: behavior::range(rng, STAY_SECS), climb_out: false, hops: 0 }
    }
}

/// 안쪽 바닥 y
pub fn floor_y(rect: &Rect) -> f64 {
    rect.bottom - INSET
}

/// 크기 pet_height인 동물이 놀 만한 창인지 (세계 쪽 판정 — 반 폭은 HALF_W_RATIO로 어림)
pub fn has_room(rect: &Rect, pet_height: f64) -> bool {
    enterable(rect, HALF_W_RATIO * pet_height, pet_height)
}

/// 반 폭 half_w, 크기 size인 동물이 들어갈 수 있는 창인지: 안쪽 폭 ≥ 몸 폭, 높이 ≥ 1.2 × 크기
pub fn enterable(rect: &Rect, half_w: f64, size: f64) -> bool {
    rect.width() - 2.0 * INSET >= 2.0 * half_w && rect.height() >= MIN_HEIGHT_MULT * size
}

/// 창 안 공중 경계: 옆은 안쪽 벽, 위는 창 윗면(ceiling이 false면 화면 꼭대기). 화면 밖은 화면 경계로 자른다
pub fn air_bounds(rect: &Rect, screen: &Rect, ceiling: bool) -> Rect {
    Rect::new(
        (rect.left + INSET).max(screen.left),
        if ceiling { rect.top.max(screen.top) } else { screen.top },
        (rect.right - INSET).min(screen.right),
        rect.bottom.min(screen.bottom),
    )
}

/// 몸(발 x, 발 y, 반 폭, 크기)이 다 들어 있는 창 중 그 자리에서 가장 앞쪽 창 — 놓았을 때 그 창 안으로 들어간다.
/// 몸 가운데 지점의 맨 앞 창이어야 하고, 들어갈 만한 크기에 안쪽 바닥이 있어야 한다
pub fn drop_target(world: &World, x: f64, y: f64, half_w: f64, size: f64) -> Option<u64> {
    let w = world.front_window_at(x, y - size / 2.0)?;
    let r = w.rect;
    let fits = x - half_w >= r.left && x + half_w <= r.right && y - size >= r.top && y <= r.bottom;
    (fits && enterable(&r, half_w, size) && has_floor(world, w.id)).then_some(w.id)
}

/// 그 창에 안쪽 바닥 조각이 하나라도 있는지
pub fn has_floor(world: &World, window: u64) -> bool {
    world.surfaces.iter().any(|s| s.owner == Owner::Inside(window))
}

/// 발 y에서 창 윗면 top 위로 clear만큼 솟는 처음 속도 (위로 = 음수).
/// step_air(반암시적 오일러, 틱 dt)는 연속 공식보다 꼭대기가 최대 v·dt/2 낮으므로 그만큼 더 세게 뛴다:
/// v²/2g − v·dt/2 = h  →  v = (g·dt + √(g²dt² + 8gh)) / 2
pub fn leave_vy(y: f64, top: f64, clear: f64, dt: f64) -> f64 {
    let g = crate::physics::GRAVITY;
    let h = (y - top + clear).max(0.0);
    -(g * dt + (g * g * dt * dt + 8.0 * g * h).sqrt()) / 2.0
}

/// 작업 영역의 이 비율 이상을 덮는 창(최대화·거의 전체 화면)엔 들어가지 않는다
pub const MAX_AREA_SHARE: f64 = 0.9;

/// 창이 작업 영역 work_area의 90% 이상을 덮는지
pub fn too_big(rect: &Rect, work_area: &Rect) -> bool {
    let w = (rect.right.min(work_area.right) - rect.left.max(work_area.left)).max(0.0);
    let h = (rect.bottom.min(work_area.bottom) - rect.top.max(work_area.top)).max(0.0);
    w * h >= MAX_AREA_SHARE * work_area.width() * work_area.height()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::WindowInfo;
    use crate::physics::GRAVITY;
    use crate::surfaces::{EndKind, Surface};

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    fn inner(w: &World, id: u64) -> Vec<Surface> {
        w.surfaces.iter().copied().filter(|s| s.owner == Owner::Inside(id)).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!((INSET, HALF_W_RATIO, MIN_HEIGHT_MULT, HOP_IN_CHANCE), (4.0, 0.3, 1.2, 0.05));
        assert_eq!((STAY_SECS, CLIMB_OUT_CHANCE), ((60.0, 180.0), 0.5));
    }

    #[test]
    fn inner_floor_sits_inset_above_the_bottom_border_with_wall_ends() {
        let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
        let p = inner(&w, 1);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].y, p[0].x_start, p[0].x_end), (896.0, 504.0, 1096.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Wall, EndKind::Wall));
        assert_eq!(w.anchor(Owner::Inside(1)), Some((500.0, 896.0)));
        assert_eq!(w.anchor(Owner::Inside(9)), None);
    }

    #[test]
    fn front_window_cuts_the_inner_floor() {
        let w = world(vec![win(2, 700.0, 850.0, 800.0, 1000.0), win(1, 500.0, 400.0, 1100.0, 900.0)]);
        let p = inner(&w, 1);
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].x_start, p[0].x_end, p[0].start_kind, p[0].end_kind), (504.0, 700.0, EndKind::Wall, EndKind::Blocked));
        assert_eq!((p[1].x_start, p[1].x_end, p[1].start_kind, p[1].end_kind), (800.0, 1096.0, EndKind::Blocked, EndKind::Wall));
        // 앞쪽 창도 제 안쪽 바닥이 있다 (높이 150 < 1.2 × 128 이면 없음)
        assert!(inner(&w, 2).is_empty());
    }

    #[test]
    fn too_small_windows_have_no_inner_floor() {
        // 안쪽 폭 = 폭 − 8 < 2 × 0.3 × 128 = 76.8
        let narrow = world(vec![win(1, 500.0, 400.0, 584.0, 900.0)]);
        assert!(inner(&narrow, 1).is_empty());
        let ok = world(vec![win(1, 500.0, 400.0, 585.0, 900.0)]);
        assert_eq!(inner(&ok, 1).len(), 1);
        // 높이 < 1.2 × 128 = 153.6
        let short = world(vec![win(1, 500.0, 400.0, 1100.0, 553.0)]);
        assert!(inner(&short, 1).is_empty());
        let ok = world(vec![win(1, 500.0, 400.0, 1100.0, 554.0)]);
        assert_eq!(inner(&ok, 1).len(), 1);
        assert!(!enterable(&Rect::new(0.0, 0.0, 83.0, 500.0), 38.0, 128.0));
        assert!(enterable(&Rect::new(0.0, 0.0, 84.0, 500.0), 38.0, 128.0));
    }

    #[test]
    fn inner_floor_below_the_screen_is_dropped_and_offscreen_sides_are_clipped() {
        let w = world(vec![win(1, 500.0, 900.0, 1100.0, 1300.0), win(2, -300.0, 300.0, 600.0, 800.0)]);
        assert!(inner(&w, 1).is_empty());
        let p = inner(&w, 2);
        assert_eq!((p[0].x_start, p[0].x_end), (0.0, 596.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Wall, EndKind::Wall));
    }

    #[test]
    fn air_bounds_are_the_inner_walls_and_top() {
        let screen = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let r = Rect::new(500.0, 400.0, 1100.0, 900.0);
        assert_eq!(air_bounds(&r, &screen, true), Rect::new(504.0, 400.0, 1096.0, 900.0));
        assert_eq!(air_bounds(&r, &screen, false), Rect::new(504.0, 0.0, 1096.0, 900.0));
        let off = Rect::new(-300.0, -100.0, 600.0, 2000.0);
        assert_eq!(air_bounds(&off, &screen, true), Rect::new(0.0, 0.0, 596.0, 1040.0));
    }

    #[test]
    fn drop_target_is_the_front_window_holding_the_whole_body() {
        let back = win(1, 500.0, 400.0, 1100.0, 900.0);
        let front = win(2, 900.0, 300.0, 1400.0, 800.0);
        let w = world(vec![front, back]);
        // 몸 전체가 뒤 창 안, 그 자리에 앞 창 없음
        assert_eq!(drop_target(&w, 700.0, 850.0, 38.0, 128.0), Some(1));
        // 몸 가운데가 앞 창 안 → 앞 창 (몸 전체도 앞 창 안)
        assert_eq!(drop_target(&w, 1000.0, 700.0, 38.0, 128.0), Some(2));
        // 걸쳐 있음 → 아무 창도 아님
        assert_eq!(drop_target(&w, 520.0, 850.0, 38.0, 128.0), None);
        assert_eq!(drop_target(&w, 700.0, 450.0, 38.0, 128.0), None);
        assert_eq!(drop_target(&w, 200.0, 850.0, 38.0, 128.0), None);
        // 너무 작은 창
        let tiny = world(vec![win(1, 500.0, 400.0, 620.0, 540.0)]);
        assert_eq!(drop_target(&tiny, 560.0, 535.0, 38.0, 128.0), None);
    }

    #[test]
    fn leave_vy_clears_the_top_with_the_game_integrator() {
        // step_air와 같은 반암시적 적분으로도 꼭대기가 top − clear 위
        for (y, top, clear, dt) in [(1036.0, 60.0, 18.0, 1.0 / 60.0), (896.0, 400.0, 40.0, 1.0 / 30.0), (1036.0, 200.0, 0.0, 0.05)] {
            let mut b = crate::physics::Body { x: 0.0, y, vx: 0.0, vy: leave_vy(y, top, clear, dt) };
            let mut apex = y;
            let open = Rect::new(-1e9, -1e9, 1e9, 1e9);
            while b.vy < 0.0 {
                crate::physics::step_air(&mut b, dt, &open, 1.0, 1.0, crate::physics::GRAVITY);
                apex = apex.min(b.y);
            }
            assert!(apex <= top - clear, "{y} {top} {clear} {dt}: apex {apex}");
            assert!(apex >= top - clear - 60.0, "too high {apex}");
        }
    }

    #[test]
    fn maximized_test_rect_is_too_big() {
        let wa = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        assert!(too_big(&Rect::new(0.0, 0.0, 1920.0, 1040.0), &wa));
        assert!(too_big(&Rect::new(-8.0, -8.0, 1928.0, 1048.0), &wa));
        assert!(!too_big(&Rect::new(100.0, 100.0, 1800.0, 1000.0), &wa));
    }

    #[test]
    fn inner_floor_stays_inside_its_work_area() {
        // 오른쪽 모니터(바닥 900)에 있는 창의 안쪽 바닥이 그 모니터 바닥 아래면 없다
        let areas = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 900.0)];
        let w = World::new(vec![win(2, 1700.0, 300.0, 2300.0, 800.0), win(1, 2000.0, 300.0, 2600.0, 1000.0)], areas, 128.0);
        assert!(inner(&w, 1).is_empty());
        // 두 모니터에 걸친 창은 가운데가 있는 모니터 안으로 자른다
        let p = inner(&w, 2);
        assert_eq!((p[0].x_start, p[0].x_end), (1920.0, 2296.0));
    }

    #[test]
    fn leave_vy_clears_the_top() {
        let vy = leave_vy(896.0, 400.0, 40.0, 0.0);
        // dt → 0이면 꼭대기 높이 = v² / 2g
        assert!((vy * vy / (2.0 * GRAVITY) - 536.0).abs() < 1e-6);
        assert!(vy < 0.0);
    }

    #[test]
    fn stay_lasts_one_to_three_minutes() {
        let mut rng = fastrand::Rng::with_seed(5);
        let s: Vec<f64> = (0..2000).map(|_| Stay::new(1, Mode::In, &mut rng).leave_in).collect();
        assert!(s.iter().all(|t| (60.0..=180.0).contains(t)));
        assert!(s.iter().any(|&t| t < 70.0) && s.iter().any(|&t| t > 170.0));
    }
}
