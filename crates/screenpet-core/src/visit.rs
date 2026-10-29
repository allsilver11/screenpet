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
}

/// 방문 시간 3~6분 중 하나 (그리움까지)
pub fn pick_homesick_after(rng: &mut fastrand::Rng) -> f64 {
    behavior::range(rng, HOMESICK_SECS)
}

/// 화면 끝 edge에 닿았을 때 걸어 나갈 확률. 손님은 집 방향 끝으로만 나간다 (다른 끝은 0 → 뒤돈다)
pub fn exit_chance(edge: Side, exits: ExitEdges, guest: Option<Guest>) -> f64 {
    match guest {
        Some(g) if g.home != edge => 0.0,
        Some(g) if g.is_homesick() => 1.0,
        Some(_) => GUEST_EXIT_CHANCE,
        None if exits.has(edge) => EXIT_CHANCE,
        None => 0.0,
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
    fn guests_only_leave_toward_home() {
        let none = ExitEdges::default();
        let guest = Guest { home: Side::Right, homesick_in: 100.0 };
        assert_eq!(exit_chance(Side::Right, none, Some(guest)), 0.5);
        assert_eq!(exit_chance(Side::Left, ExitEdges { left: true, right: true }, Some(guest)), 0.0);
        let homesick = Guest { homesick_in: 0.0, ..guest };
        assert!(homesick.is_homesick() && !guest.is_homesick());
        assert_eq!(exit_chance(Side::Right, none, Some(homesick)), 1.0);
        assert_eq!(exit_chance(Side::Left, none, Some(homesick)), 0.0);
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
