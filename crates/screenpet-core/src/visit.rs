//! 친구 화면 놀러 가기 규칙 (spec phase 3 §3) — 확률·시간 수치와 화면 끝 판정. brain.rs가 쓴다.

use crate::behavior;
use crate::geom::Rect;
use crate::surfaces::{EndKind, Owner, Surface};
pub use crate::world::Side;
use crate::world::World;

/// 연결된 화면 끝에 닿았을 때 걸어 나갈 확률 (내 동물)
pub const EXIT_CHANCE: f64 = 0.3;
/// 손님이 집 방향 끝에 닿았을 때 걸어 나갈 확률 (그리움 전)
pub const GUEST_EXIT_CHANCE: f64 = 0.5;
/// 나가기·들어오기 최소 시간 — 몸이 화면 밖으로 다 빠져나갈(안으로 다 들어올) 때까지 이어 간다
pub const EXIT_SECS: f64 = 1.0;
pub const ENTER_SECS: f64 = 1.0;
/// 집에 와: 손 흔들고 사라지기까지
pub const FAREWELL_SECS: f64 = 1.0;
/// 방문 뒤 이만큼(초) 지나면 집이 그리워진다
pub const HOMESICK_SECS: (f64, f64) = (180.0, 360.0);
/// 손님이 (집 쪽이 아닌) 다른 친구가 연결된 끝에 닿았을 때 그 친구에게 더 놀러 갈 확률 (그리우면 0)
pub const ROAM_CHANCE: f64 = 0.3;
/// 한 화면에 동시에 받는 손님 수 — 넘치면 오는 손님은 바로 집으로 돌아간다
pub const MAX_GUESTS: usize = 10;
/// 그리워진 뒤 이만큼(초) 지나도 집에 못 갔으면 그 자리에서 손 흔들고 집으로
pub const HOMESICK_HARD_CAP_SECS: f64 = 180.0;
/// 손님 몸 배율로 받아들이는 범위 (단계 배율 × 동물별 배율이 들어갈 만큼)
pub const GUEST_SCALE_RANGE: (f64, f64) = (0.3, 2.0);
/// 바닥 끝이 합친 화면 끝과 같다고 볼 x 오차 (px)
const EDGE_TOLERANCE: f64 = 2.0;

/// 친구가 연결된 내 화면 끝
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExitEdges {
    pub left: bool,
    pub right: bool,
}

impl ExitEdges {
    fn has(self, side: Side) -> bool {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}

/// 손님으로 와 있는 동물: 집으로 가는 끝과 그리워지기까지 남은 시간
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Guest {
    pub home: Side,
    pub homesick_in: f64,
}

impl Guest {
    pub fn is_homesick(&self) -> bool {
        self.homesick_in <= 0.0
    }

    /// 그리워진 지 HOMESICK_HARD_CAP_SECS가 지났다 — 더 기다리지 않고 집으로
    pub fn is_overdue(&self) -> bool {
        self.homesick_in <= -HOMESICK_HARD_CAP_SECS
    }
}

/// 방문 시간 3~6분 중 하나 (그리움까지)
pub fn pick_homesick_after(rng: &mut fastrand::Rng) -> f64 {
    behavior::range(rng, HOMESICK_SECS)
}

/// 다른 친구 화면에서 건너온 손님의 남은 그리움 시간 (건너도 다시 세지 않는다).
/// 보낸 값이 없거나 이상하면 새로 고르고, 범위 밖이면 0..=최대로
pub fn carried_homesick(sent: Option<f64>, rng: &mut fastrand::Rng) -> f64 {
    match sent {
        Some(t) if t.is_finite() => t.clamp(0.0, HOMESICK_SECS.1),
        _ => pick_homesick_after(rng),
    }
}

/// 건너온 손님이 들어올 끝: 이 화면에서 이전 친구(via)가 연결된 끝, 모르면 이전 화면에서 나간 끝의 반대편
pub fn hop_enter_side(via: Option<Side>, exited: Side) -> Side {
    via.unwrap_or(opposite(exited))
}

pub fn opposite(side: Side) -> Side {
    match side {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// 화면 끝 edge에 닿았을 때 걸어 나갈 확률.
/// 내 동물: 친구가 연결된 끝(exits)에서 30%. 손님: 집 방향 끝에서 50%(그리우면 반드시),
/// 다른 끝은 거기 다른 친구가 연결돼 있으면(exits — 주인은 뺀다) 30% 더 놀러 감, 아니면(또는 그리우면) 뒤돈다
pub fn exit_chance(edge: Side, exits: ExitEdges, guest: Option<Guest>) -> f64 {
    match guest {
        Some(g) if g.home == edge && g.is_homesick() => 1.0,
        Some(g) if g.home == edge => GUEST_EXIT_CHANCE,
        Some(g) if g.is_homesick() => 0.0,
        Some(_) if exits.has(edge) => ROAM_CHANCE,
        Some(_) => 0.0,
        None if exits.has(edge) => EXIT_CHANCE,
        None => 0.0,
    }
}

/// 손님 몸 배율: 주인이 보낸 값(sent)이 범위 안이면 그대로, 아니면(NaN 포함) 이쪽에서 계산한 값(local)을 범위 안으로
pub fn guest_scale(sent: f64, local: f64) -> f64 {
    let (lo, hi) = GUEST_SCALE_RANGE;
    if (lo..=hi).contains(&sent) {
        sent
    } else if local.is_finite() {
        local.clamp(lo, hi)
    } else {
        1.0
    }
}

/// +1 오른쪽, -1 왼쪽
pub fn dir(side: Side) -> f64 {
    match side {
        Side::Left => -1.0,
        Side::Right => 1.0,
    }
}

pub fn side_of(dir: f64) -> Side {
    if dir < 0.0 {
        Side::Left
    } else {
        Side::Right
    }
}

/// 디딤판 piece를 facing 방향 끝까지 갔을 때 그 끝이 합친 화면의 가장 바깥 바닥 끝(Wall)이면 그 쪽.
/// 모니터 경계(Seam), 창 윗면, 더 바깥으로 이어지는 화면이 있는 바닥 끝은 None.
pub fn screen_edge(world: &World, piece: &Surface, facing: f64) -> Option<Side> {
    if !matches!(piece.owner, Owner::Ground(_)) {
        return None;
    }
    let side = side_of(facing);
    let (x, kind, bound) = match side {
        Side::Left => (piece.x_start, piece.start_kind, world.bounds.left),
        Side::Right => (piece.x_end, piece.end_kind, world.bounds.right),
    };
    (kind == EndKind::Wall && (x - bound).abs() <= EDGE_TOLERANCE).then_some(side)
}

/// side 끝에서 들어올 바닥: (주인, 끝 x, 바닥 y). 그 끝에 닿은 바닥이 여럿이면 가장 아래쪽
pub fn entry_ground(world: &World, side: Side) -> Option<(Owner, f64, f64)> {
    let facing = dir(side);
    world
        .surfaces
        .iter()
        .filter(|s| screen_edge(world, s, facing).is_some())
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .map(|s| (s.owner, if side == Side::Left { s.x_start } else { s.x_end }, s.y))
}

/// 몸(중심 x, 반폭 half_w)이 side 끝 바깥으로 다 빠져나갔는지
pub fn fully_outside(bounds: &Rect, side: Side, x: f64, half_w: f64) -> bool {
    match side {
        Side::Left => x + half_w <= bounds.left,
        Side::Right => x - half_w >= bounds.right,
    }
}

/// 몸이 side 끝 안쪽으로 다 들어왔는지
pub fn fully_inside(bounds: &Rect, side: Side, x: f64, half_w: f64) -> bool {
    match side {
        Side::Left => x - half_w >= bounds.left,
        Side::Right => x + half_w <= bounds.right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{Rect, WindowInfo};
    use crate::surfaces::{EndKind, Owner};
    use crate::world::World;

    fn ground(w: &World, i: usize) -> Surface {
        *w.surfaces.iter().find(|s| s.owner == Owner::Ground(i)).unwrap()
    }

    #[test]
    fn guest_scale_uses_a_sane_sent_value_or_the_local_one() {
        assert_eq!(guest_scale(1.15, 1.0), 1.15);
        assert_eq!(guest_scale(f64::NAN, 0.78), 0.78);
        assert_eq!(guest_scale(f64::INFINITY, 0.78), 0.78);
        assert_eq!(guest_scale(50.0, 1.0), 1.0);
        assert_eq!(guest_scale(0.01, 0.45), 0.45);
        // 이쪽 값도 범위 안으로
        assert_eq!(guest_scale(0.0, 9.0), GUEST_SCALE_RANGE.1);
    }

    #[test]
    fn numbers() {
        assert_eq!((EXIT_CHANCE, GUEST_EXIT_CHANCE, EXIT_SECS, ENTER_SECS, FAREWELL_SECS), (0.3, 0.5, 1.0, 1.0, 1.0));
        assert_eq!(HOMESICK_SECS, (180.0, 360.0));
    }

    #[test]
    fn exit_chance_for_owner_needs_a_connected_edge() {
        let both = ExitEdges { left: true, right: true };
        let right = ExitEdges { left: false, right: true };
        assert_eq!(exit_chance(Side::Right, both, None), 0.3);
        assert_eq!(exit_chance(Side::Left, both, None), 0.3);
        assert_eq!(exit_chance(Side::Right, right, None), 0.3);
        assert_eq!(exit_chance(Side::Left, right, None), 0.0);
        assert_eq!(exit_chance(Side::Left, ExitEdges::default(), None), 0.0);
    }

    #[test]
    fn guests_leave_toward_home_or_roam_on_to_another_friend() {
        let none = ExitEdges::default();
        let both = ExitEdges { left: true, right: true };
        let guest = Guest { home: Side::Right, homesick_in: 100.0 };
        assert_eq!(exit_chance(Side::Right, none, Some(guest)), 0.5);
        // 집 쪽 끝은 다른 친구가 연결돼 있어도 집 규칙
        assert_eq!(exit_chance(Side::Right, both, Some(guest)), 0.5);
        // 다른 끝: 그 끝에 (주인 말고) 연결된 친구가 있으면 30% 더 놀러 간다
        assert_eq!(exit_chance(Side::Left, both, Some(guest)), ROAM_CHANCE);
        assert_eq!(exit_chance(Side::Left, none, Some(guest)), 0.0);
        let homesick = Guest { homesick_in: 0.0, ..guest };
        assert!(homesick.is_homesick() && !guest.is_homesick());
        assert_eq!(exit_chance(Side::Right, none, Some(homesick)), 1.0);
        assert_eq!(exit_chance(Side::Left, none, Some(homesick)), 0.0);
        // 그리우면 더 놀러 가지 않는다
        assert_eq!(exit_chance(Side::Left, both, Some(homesick)), 0.0);
    }

    #[test]
    fn caps_and_roaming_numbers() {
        assert_eq!((ROAM_CHANCE, MAX_GUESTS, HOMESICK_HARD_CAP_SECS), (0.3, 10, 180.0));
    }

    #[test]
    fn homesick_guest_is_overdue_three_minutes_after_onset() {
        let g = Guest { home: Side::Left, homesick_in: 0.0 };
        assert!(!g.is_overdue());
        assert!(!Guest { homesick_in: -179.9, ..g }.is_overdue());
        assert!(Guest { homesick_in: -180.0, ..g }.is_overdue());
    }

    #[test]
    fn carried_homesick_time_continues_or_is_picked_fresh() {
        let mut rng = fastrand::Rng::with_seed(1);
        assert_eq!(carried_homesick(Some(42.0), &mut rng), 42.0);
        // 이상한 값은 범위 안으로 (그리움이 이미 시작됐으면 바로 그리운 채로)
        assert_eq!(carried_homesick(Some(-5.0), &mut rng), 0.0);
        assert_eq!(carried_homesick(Some(1e9), &mut rng), HOMESICK_SECS.1);
        for v in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            let t = carried_homesick(v, &mut rng);
            assert!((180.0..=360.0).contains(&t), "{v:?} → {t}");
        }
    }

    #[test]
    fn hop_enters_from_the_edge_linked_to_the_previous_host() {
        assert_eq!(hop_enter_side(Some(Side::Right), Side::Right), Side::Right);
        // 이전 친구를 모르면 나간 쪽의 반대편에서
        assert_eq!(hop_enter_side(None, Side::Right), Side::Left);
        assert_eq!(hop_enter_side(None, Side::Left), Side::Right);
        assert_eq!((opposite(Side::Left), opposite(Side::Right)), (Side::Right, Side::Left));
    }

    #[test]
    fn side_directions() {
        assert_eq!((dir(Side::Left), dir(Side::Right)), (-1.0, 1.0));
        assert_eq!((side_of(-1.0), side_of(1.0)), (Side::Left, Side::Right));
    }

    #[test]
    fn homesick_after_is_three_to_six_minutes() {
        let mut rng = fastrand::Rng::with_seed(3);
        let ts: Vec<f64> = (0..2000).map(|_| pick_homesick_after(&mut rng)).collect();
        assert!(ts.iter().all(|t| (180.0..=360.0).contains(t)));
        assert!(ts.iter().any(|&t| t < 200.0) && ts.iter().any(|&t| t > 340.0));
    }

    #[test]
    fn screen_edge_is_only_the_outermost_ground_wall() {
        // 왼쪽 모니터 + 오른쪽 모니터(옆으로 맞닿음) + 위에 쌓인 좁은 모니터
        let w = World::new(
            vec![WindowInfo { id: 1, rect: Rect::new(3000.0, 400.0, 4200.0, 900.0) }],
            vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0), Rect::new(500.0, -1080.0, 1500.0, -40.0)],
            128.0,
        );
        let (g0, g1, g2) = (ground(&w, 0), ground(&w, 1), ground(&w, 2));
        assert_eq!(screen_edge(&w, &g0, -1.0), Some(Side::Left));
        // 모니터 경계는 끝이 아니다
        assert!(matches!(g0.end_kind, EndKind::Seam { .. }));
        assert_eq!(screen_edge(&w, &g0, 1.0), None);
        assert_eq!(screen_edge(&w, &g1, 1.0), Some(Side::Right));
        assert_eq!(screen_edge(&w, &g1, -1.0), None);
        // 위 모니터의 바닥 끝은 Wall이지만 가장 바깥이 아니다
        assert_eq!(g2.end_kind, EndKind::Wall);
        assert_eq!((screen_edge(&w, &g2, -1.0), screen_edge(&w, &g2, 1.0)), (None, None));
        // 화면 끝에서 잘린 창 윗면도 끝이 아니다
        let win = *w.surfaces.iter().find(|s| s.owner == Owner::Window(1)).unwrap();
        assert_eq!(win.end_kind, EndKind::Wall);
        assert_eq!(screen_edge(&w, &win, 1.0), None);
    }

    #[test]
    fn entry_ground_is_the_lowest_ground_touching_that_edge() {
        let w = World::new(
            vec![],
            vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 200.0, 3840.0, 1300.0), Rect::new(-1920.0, -500.0, 0.0, 500.0)],
            128.0,
        );
        assert_eq!(entry_ground(&w, Side::Right), Some((Owner::Ground(1), 3840.0, 1300.0)));
        assert_eq!(entry_ground(&w, Side::Left), Some((Owner::Ground(2), -1920.0, 500.0)));
        // 같은 끝에 바닥이 둘이면 더 아래쪽
        let stacked = World::new(vec![], vec![Rect::new(0.0, -1080.0, 1920.0, -40.0), Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
        assert_eq!(entry_ground(&stacked, Side::Left), Some((Owner::Ground(1), 0.0, 1040.0)));
    }

    #[test]
    fn fully_past_an_edge() {
        let b = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        assert!(fully_outside(&b, Side::Right, 1958.0, 38.0));
        assert!(!fully_outside(&b, Side::Right, 1957.0, 38.0));
        assert!(fully_outside(&b, Side::Left, -38.0, 38.0));
        assert!(!fully_outside(&b, Side::Left, -37.0, 38.0));
        assert!(fully_inside(&b, Side::Left, 38.0, 38.0));
        assert!(!fully_inside(&b, Side::Left, 37.0, 38.0));
        assert!(fully_inside(&b, Side::Right, 1882.0, 38.0));
        assert!(!fully_inside(&b, Side::Right, 1883.0, 38.0));
    }
}
