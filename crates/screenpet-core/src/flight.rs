//! 날아다니는 동물(꿀벌)의 비행 (spec 2B §10) — 수치와 순수 계산. brain_fly.rs(Pet)가 쓴다.
//!
//! 나는 동물은 디딤판 위를 걷지 않는다. "이동"은 목표 지점까지 완만한 사인 물결을 그리며 나는 것이고,
//! 쉬기는 제자리에서 살랑살랑 떠 있거나, 가끔 창 윗면·바닥에 내려앉아(perch) 쉬거나 잔다.
//! 좌표는 다른 동물과 같다: x = 몸 가운데, y = 발바닥 (화면 아래로 +).

use std::f64::consts::{PI, TAU};

use crate::behavior::{self, Move};
use crate::geom::Rect;
use crate::surfaces::{Owner, Surface};
use crate::visit::{self, Side};
use crate::world::World;

/// 빠르게 날기(Run) 확률 — 나머지는 보통 날기, 가끔 공중제비(Roll)
pub const RUN_CHANCE: f64 = 0.3;
pub const ROLL_CHANCE: f64 = 0.1;
/// 떠서 쉬기로 정할 때 대신 어딘가에 내려앉으러 갈 확률
pub const PERCH_CHANCE: f64 = 0.35;
/// 내려앉아 쉬다가 다음 할 일을 정할 때 잠들 확률 (떠 있을 땐 자지 않는다)
pub const PERCH_SLEEP_CHANCE: f64 = 0.35;
/// 사인 물결 진폭·파장 (몸 크기 배)
pub const WAVE_AMP: f64 = 0.15;
pub const WAVE_LEN: f64 = 3.0;
/// 공중제비 원 반지름 (몸 크기 배 — 지름 1.5 몸)
pub const LOOP_RADIUS: f64 = 0.75;
/// 공중제비를 시작하는 지점 (전체 거리의 비율)
pub const LOOP_AT: f64 = 0.4;
/// 떠 있을 때 위아래로 살랑이는 폭(몸 크기 배)과 빠르기(Hz)
pub const BOB_AMP: f64 = 0.04;
pub const BOB_HZ: f64 = 0.8;
/// 던져진 뒤 공기 저항 (1/s) — 이 속도(px/s)보다 느려지면 다시 제자리에 뜬다
pub const DRAG: f64 = 3.0;
pub const SETTLE_SPEED: f64 = 40.0;
/// 바깥에서 날 때 머리 위로 남기는 말풍선 자리 (몸 크기 배) — 말풍선 띠가 화면 밖으로 나가지 않게
pub const TOP_SPACE: f64 = 0.3;
/// 목표 높이가 화면 위쪽 2/3 안일 확률
pub const UPPER_SHARE: f64 = 0.8;
/// 화면 가장 바깥 끝에서 이 거리(몸 크기 배) 안으로 들어와 그쪽으로 날면 나가기를 정한다
pub const EDGE_ZONE: f64 = 1.0;
/// 목표까지 최소 거리 (몸 크기 배)
pub const MIN_FLIGHT: f64 = 1.0;
/// 집으로 날 때 활처럼 솟는 높이: 거리의 이 비율, 최대 몸 크기의 HOME_ARC_MAX배
pub const HOME_ARC: f64 = 0.2;
pub const HOME_ARC_MAX: f64 = 1.5;
/// 날기 한 번의 최대 시간 (안전장치)
pub const MAX_FLIGHT_SECS: f64 = 30.0;
/// 날아갈 목표를 지금 모니터 안에서 고를 확률 (나머지는 아무 모니터)
pub const SAME_AREA_CHANCE: f64 = 0.75;

/// 걷기 속도(cfg.walk_speed = pet.json walk_speed × √배율) 대비 나는 속도
pub fn speed_mult(kind: Move) -> f64 {
    match kind {
        Move::Walk => 1.0,
        Move::Run => 1.8,
        Move::Roll => 1.2,
    }
}

/// 다음 날기 방식: 30% 빠르게, 10% 공중제비, 나머지 보통
pub fn pick_move(rng: &mut fastrand::Rng) -> Move {
    let r = rng.f64();
    if r < RUN_CHANCE {
        Move::Run
    } else if r < RUN_CHANCE + ROLL_CHANCE {
        Move::Roll
    } else {
        Move::Walk
    }
}

/// area 안에서 몸(발 x, 발 y)이 있을 수 있는 범위: 좌우 반 폭, 위로 head(발에서 머리 위 여유까지)
pub fn fly_bounds(area: &Rect, half_w: f64, head: f64) -> Rect {
    let (mut l, mut r) = (area.left + half_w, area.right - half_w);
    if l > r {
        l = (area.left + area.right) / 2.0;
        r = l;
    }
    let t = (area.top + head).min(area.bottom);
    Rect::new(l, t, r, area.bottom)
}

pub fn clamp(b: &Rect, x: f64, y: f64) -> (f64, f64) {
    (x.clamp(b.left, b.right), y.clamp(b.top, b.bottom))
}

/// 날아갈 지점: x는 고르게, 높이는 대개(80%) 위쪽 2/3
pub fn pick_target(rng: &mut fastrand::Rng, b: &Rect) -> (f64, f64) {
    let x = behavior::range(rng, (b.left, b.right));
    let h = b.height();
    let u = rng.f64();
    let y = if rng.f64() < UPPER_SHARE { b.top + u * h * 2.0 / 3.0 } else { b.top + (2.0 + u) / 3.0 * h };
    (x, y)
}

/// 날아가는 길의 모양
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wave {
    /// 진행 방향에 수직으로 출렁이는 사인 물결 (amp는 부호로 처음 출렁일 쪽을 정한다)
    Sine { amp: f64, len: f64 },
    /// 위로 솟는 활 모양 (집으로 갈 때)
    Arc { height: f64 },
}

/// from → to 직선을 s만큼 간 지점에 물결을 더한 점, 그리고 다 왔는지.
/// 물결은 양 끝에서 0이 되도록 sin(πu)로 줄인다 — 출발점과 목표에 정확히 닿는다
pub fn path_point(from: (f64, f64), to: (f64, f64), s: f64, wave: Wave) -> ((f64, f64), bool) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    if len < 1e-6 || s >= len {
        return (to, true);
    }
    let u = (s / len).max(0.0);
    let envelope = (PI * u).sin();
    let (bx, by) = (from.0 + dx * u, from.1 + dy * u);
    match wave {
        Wave::Sine { amp, len: wl } => {
            let off = amp * (TAU * s / wl).sin() * envelope;
            ((bx - dy / len * off, by + dx / len * off), false)
        }
        Wave::Arc { height } => ((bx, by - height * envelope), false),
    }
}

/// 공중제비: start에서 진행 방향(facing)으로 원을 한 바퀴 (up이면 위로 돈다). theta = 0 → start
pub fn loop_point(start: (f64, f64), r: f64, theta: f64, facing: f64, up: bool) -> (f64, f64) {
    let x = start.0 + facing * r * theta.sin();
    let y = if up { start.1 - r + r * theta.cos() } else { start.1 + r - r * theta.cos() };
    (x, y)
}

/// 공기 저항으로 줄어든 속도
pub fn drift(vx: f64, vy: f64, dt: f64) -> (f64, f64) {
    let k = (-DRAG * dt).exp();
    (vx * k, vy * k)
}

/// 떠 있을 때 위아래 살랑임 (기준 높이에 더한다)
pub fn bob(clock: f64, size: f64) -> f64 {
    BOB_AMP * size * (TAU * BOB_HZ * clock).sin()
}

/// 내려앉을 자리: 디딤판과 발 위치
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perch {
    pub owner: Owner,
    pub x: f64,
    pub y: f64,
}

/// 내려앉을 수 있는 디딤판이면 발 x 범위. 창 안(inside)이면 그 창 안쪽 바닥만, 바깥이면 창 윗면·바닥만.
/// 몸 폭이 들어가야 하고, 앉았을 때 머리와 말풍선 자리(head)가 화면 안이어야 한다
fn perch_range(s: &Surface, inside: Option<u64>, top: f64, half_w: f64, head: f64) -> Option<(f64, f64)> {
    let owner_ok = match (inside, s.owner) {
        (Some(id), Owner::Inside(w)) => id == w,
        (None, Owner::Window(_) | Owner::Ground(_)) => true,
        _ => false,
    };
    (owner_ok && s.width() >= 2.0 * half_w && s.y - head >= top).then_some((s.x_start + half_w, s.x_end - half_w))
}

/// 아무 데나 내려앉을 자리 하나 (없으면 None)
pub fn pick_perch(rng: &mut fastrand::Rng, world: &World, inside: Option<u64>, half_w: f64, head: f64) -> Option<Perch> {
    let spots: Vec<(Owner, f64, (f64, f64))> = world
        .surfaces
        .iter()
        .filter_map(|s| perch_range(s, inside, world.bounds.top, half_w, head).map(|r| (s.owner, s.y, r)))
        .collect();
    if spots.is_empty() {
        return None;
    }
    let (owner, y, range) = spots[rng.usize(..spots.len())];
    Some(Perch { owner, x: behavior::range(rng, range), y })
}

/// (x, y)에서 가장 가까운 내려앉을 자리 — 발보다 아래(같은 높이 포함)에 있는 자리가 있으면 그중에서
pub fn nearest_perch(world: &World, inside: Option<u64>, x: f64, y: f64, half_w: f64, head: f64) -> Option<Perch> {
    let spots: Vec<Perch> = world
        .surfaces
        .iter()
        .filter_map(|s| perch_range(s, inside, world.bounds.top, half_w, head).map(|(lo, hi)| Perch { owner: s.owner, x: x.clamp(lo, hi), y: s.y }))
        .collect();
    let below = spots.iter().any(|p| p.y >= y);
    spots
        .into_iter()
        .filter(|p| !below || p.y >= y)
        .min_by(|a, b| (a.x - x).hypot(a.y - y).total_cmp(&(b.x - x).hypot(b.y - y)))
}

/// 그 높이에 화면 가장 바깥 끝(side)까지 닿는 작업 영역 (발 y 기준)
fn area_at_edge(world: &World, side: Side, y: Option<f64>) -> Option<Rect> {
    let b = world.bounds;
    let mut areas = world.work_areas.iter().filter(|r| match side {
        Side::Left => (r.left - b.left).abs() <= 2.0,
        Side::Right => (r.right - b.right).abs() <= 2.0,
    });
    match y {
        Some(y) => areas.find(|r| r.top < y && y <= r.bottom).copied(),
        // 그 끝에 닿은 영역이 여럿이면 가장 큰 것
        None => areas.max_by(|a, b| a.height().total_cmp(&b.height())).copied(),
    }
}

/// 몸이 화면 가장 바깥 끝에서 몸 하나(EDGE_ZONE) 안으로 들어와 그쪽(facing)으로 날고 있으면 그 끝.
/// 그 높이의 작업 영역이 그 끝까지 닿아 있어야 한다 (모니터 사이 경계는 끝이 아니다)
pub fn edge_ahead(world: &World, x: f64, y: f64, half_w: f64, size: f64, facing: f64) -> Option<Side> {
    let b = world.bounds;
    let side = visit::side_of(facing);
    let gap = match side {
        Side::Left => x - half_w - b.left,
        Side::Right => b.right - (x + half_w),
    };
    (gap <= EDGE_ZONE * size && area_at_edge(world, side, Some(y)).is_some()).then_some(side)
}

/// 화면 끝(side) 쪽으로 나가려고 날아갈 지점 — 그 끝의 작업 영역 안, 끝에서 몸 반 개 안쪽까지
pub fn edge_target(rng: &mut fastrand::Rng, world: &World, side: Side, half_w: f64, head: f64, size: f64) -> Option<(f64, f64)> {
    let area = area_at_edge(world, side, None)?;
    let fb = fly_bounds(&area, half_w, head);
    let (_, y) = pick_target(rng, &fb);
    let inset = rng.f64() * 0.5 * size;
    let x = match side {
        Side::Left => (fb.left + inset).min(fb.right),
        Side::Right => (fb.right - inset).max(fb.left),
    };
    Some((x, y))
}

/// 화면 끝(side) 바깥에서 날아 들어올 출발점: 몸이 막 화면 밖에 있는 자리, 그 끝 작업 영역의 위쪽 40% 높이
pub fn entry_point(world: &World, side: Side, half_w: f64, head: f64) -> (f64, f64) {
    let b = world.bounds;
    let area = area_at_edge(world, side, None).unwrap_or(b);
    let fb = fly_bounds(&area, half_w, head);
    let y = fb.top + 0.4 * fb.height();
    let x = match side {
        Side::Left => b.left - half_w,
        Side::Right => b.right + half_w,
    };
    (x, y)
}

/// 어디로 날아가는지
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    /// 그냥 그 지점까지 (도착하면 떠서 쉰다)
    Point(f64, f64),
    /// 디딤판 owner의 기준점에서 rel_x인 자리에 내려앉는다 (창이 움직이면 목표도 따라간다)
    Perch { owner: Owner, rel_x: f64 },
    /// 하우스 문으로
    Home,
}

/// 공중제비 진행
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loop {
    pub start: (f64, f64),
    pub theta: f64,
    pub up: bool,
}

/// 날아가는 길 하나
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub goal: Goal,
    pub from: (f64, f64),
    /// 지금까지 간 거리 (공중제비는 빼고)
    pub s: f64,
    pub wave: Wave,
    /// 이 거리에서 공중제비를 돈다 (Roll)
    pub loop_at: Option<f64>,
    pub looping: Option<Loop>,
    /// 화면 끝 나가기를 이미 정했는지 (날기마다 한 번)
    pub edge_checked: bool,
    pub elapsed: f64,
}

impl Flight {
    pub fn new(goal: Goal, from: (f64, f64), wave: Wave) -> Self {
        Self { goal, from, s: 0.0, wave, loop_at: None, looping: None, edge_checked: false, elapsed: 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::WindowInfo;

    fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    #[test]
    fn numbers() {
        assert_eq!((RUN_CHANCE, PERCH_CHANCE, LOOP_RADIUS, EDGE_ZONE), (0.3, 0.35, 0.75, 1.0));
        assert_eq!((speed_mult(Move::Walk), speed_mult(Move::Run)), (1.0, 1.8));
    }

    #[test]
    fn move_mix_is_mostly_plain_flight_with_30_percent_fast() {
        let mut rng = fastrand::Rng::with_seed(4);
        let n = 20_000;
        let picks: Vec<Move> = (0..n).map(|_| pick_move(&mut rng)).collect();
        let share = |m: Move| picks.iter().filter(|&&p| p == m).count() as f64 / n as f64;
        assert!((0.28..=0.32).contains(&share(Move::Run)));
        assert!((0.08..=0.12).contains(&share(Move::Roll)));
        assert!((0.58..=0.62).contains(&share(Move::Walk)));
    }

    #[test]
    fn fly_bounds_keep_the_body_and_head_room_inside() {
        let b = fly_bounds(&Rect::new(0.0, 0.0, 1920.0, 1040.0), 38.0, 166.4);
        assert_eq!(b, Rect::new(38.0, 166.4, 1882.0, 1040.0));
        // 너무 좁거나 낮은 영역은 한 점·한 줄로
        let tiny = fly_bounds(&Rect::new(100.0, 500.0, 150.0, 600.0), 38.0, 166.4);
        assert_eq!((tiny.left, tiny.right, tiny.top, tiny.bottom), (125.0, 125.0, 600.0, 600.0));
        assert_eq!(clamp(&b, -5.0, 2000.0), (38.0, 1040.0));
    }

    #[test]
    fn targets_are_inside_and_mostly_in_the_upper_two_thirds() {
        let b = Rect::new(38.0, 166.0, 1882.0, 1040.0);
        let mut rng = fastrand::Rng::with_seed(9);
        let ts: Vec<(f64, f64)> = (0..5000).map(|_| pick_target(&mut rng, &b)).collect();
        assert!(ts.iter().all(|&(x, y)| x >= b.left && x <= b.right && y >= b.top && y <= b.bottom));
        let upper = ts.iter().filter(|t| t.1 <= b.top + b.height() * 2.0 / 3.0).count() as f64 / 5000.0;
        assert!((0.77..=0.83).contains(&upper), "{upper}");
    }

    #[test]
    fn path_starts_and_ends_exactly_and_waves_in_between() {
        let wave = Wave::Sine { amp: 20.0, len: 384.0 };
        let (from, to) = ((100.0, 500.0), (900.0, 300.0));
        assert_eq!(path_point(from, to, 0.0, wave), (from, false));
        let len = (800.0f64).hypot(200.0);
        assert_eq!(path_point(from, to, len, wave), (to, true));
        assert_eq!(path_point(from, to, len + 50.0, wave), (to, true));
        // 중간엔 직선에서 벗어난다 (진폭 이내)
        let mut max_off: f64 = 0.0;
        for i in 1..100 {
            let s = len * i as f64 / 100.0;
            let ((x, y), done) = path_point(from, to, s, wave);
            assert!(!done);
            let u = s / len;
            let off = (x - (100.0 + 800.0 * u)).hypot(y - (500.0 - 200.0 * u));
            assert!(off <= 20.0 + 1e-9);
            max_off = max_off.max(off);
        }
        assert!(max_off > 10.0, "{max_off}");
        // 활: 위로 솟는다
        let ((_, y), _) = path_point((0.0, 1000.0), (1000.0, 1000.0), 500.0, Wave::Arc { height: 100.0 });
        assert!((y - 900.0).abs() < 1e-9);
    }

    #[test]
    fn loop_goes_round_and_comes_back() {
        let start = (500.0, 500.0);
        assert_eq!(loop_point(start, 96.0, 0.0, 1.0, true), start);
        let top = loop_point(start, 96.0, PI, 1.0, true);
        assert!((top.0 - 500.0).abs() < 1e-9 && (top.1 - 308.0).abs() < 1e-9);
        let fwd = loop_point(start, 96.0, PI / 2.0, -1.0, true);
        assert!((fwd.0 - 404.0).abs() < 1e-9 && (fwd.1 - 404.0).abs() < 1e-9);
        let down = loop_point(start, 96.0, PI, 1.0, false);
        assert!((down.1 - 692.0).abs() < 1e-9);
        let back = loop_point(start, 96.0, TAU, 1.0, true);
        assert!((back.0 - 500.0).abs() < 1e-9 && (back.1 - 500.0).abs() < 1e-9);
    }

    #[test]
    fn drift_slows_down_and_bob_is_small() {
        let (vx, vy) = drift(1000.0, -500.0, 1.0);
        assert!((vx - 1000.0 * (-3.0f64).exp()).abs() < 1e-9 && vy < 0.0 && vy > -30.0);
        assert!((0..1000).all(|i| bob(i as f64 * 0.01, 128.0).abs() <= 128.0 * BOB_AMP + 1e-9));
    }

    #[test]
    fn perches_are_window_tops_or_ground_with_head_room() {
        // 창 2는 화면 꼭대기에 너무 가까워 앉으면 말풍선이 화면 밖
        let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0), win(2, 1300.0, 100.0, 1700.0, 600.0)]);
        let mut rng = fastrand::Rng::with_seed(2);
        let head = 128.0 * 1.3;
        let mut owners = std::collections::HashSet::new();
        for _ in 0..500 {
            let p = pick_perch(&mut rng, &w, None, 38.0, head).unwrap();
            assert!(matches!(p.owner, Owner::Window(1) | Owner::Ground(0)), "{p:?}");
            let s = w.piece_at(p.owner, p.x).unwrap();
            assert!(p.x - 38.0 >= s.x_start && p.x + 38.0 <= s.x_end && p.y == s.y);
            owners.insert(p.owner);
        }
        assert_eq!(owners.len(), 2);
        // 창 안이면 그 창 안쪽 바닥만
        let p = pick_perch(&mut rng, &w, Some(1), 38.0, head).unwrap();
        assert_eq!((p.owner, p.y), (Owner::Inside(1), 896.0));
    }

    #[test]
    fn nearest_perch_prefers_spots_below() {
        let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
        let head = 128.0 * 1.3;
        assert_eq!(nearest_perch(&w, None, 800.0, 300.0, 38.0, head).map(|p| (p.owner, p.x, p.y)), Some((Owner::Window(1), 800.0, 400.0)));
        // 창 윗면보다 아래면 바닥으로 (위로 날아 올라가지 않는다)
        assert_eq!(nearest_perch(&w, None, 1050.0, 600.0, 38.0, head).map(|p| p.owner), Some(Owner::Ground(0)));
        // 창 옆 바깥: 윗면 끝 안쪽으로 맞춘다
        let p = nearest_perch(&w, None, 400.0, 350.0, 38.0, head).unwrap();
        assert_eq!((p.owner, p.x), (Owner::Window(1), 538.0));
    }

    #[test]
    fn edge_ahead_needs_the_zone_the_direction_and_an_outer_edge() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 200.0, 3840.0, 1240.0)], 128.0);
        // 오른쪽 끝(3840)에서 몸 하나 안
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 600.0, 38.0, 128.0, 1.0), Some(Side::Right));
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 140.0, 600.0, 38.0, 128.0, 1.0), None);
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 600.0, 38.0, 128.0, -1.0), None);
        // 오른쪽 모니터가 없는 높이 (y 150)
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 150.0, 38.0, 128.0, 1.0), None);
        // 모니터 사이 경계는 끝이 아니다
        assert_eq!(edge_ahead(&w, 1920.0 - 38.0 - 50.0, 600.0, 38.0, 128.0, 1.0), None);
        assert_eq!(edge_ahead(&w, 38.0 + 10.0, 600.0, 38.0, 128.0, -1.0), Some(Side::Left));
    }

    #[test]
    fn edge_targets_and_entry_points_sit_at_that_edge() {
        let w = world(vec![]);
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..200 {
            let (x, y) = edge_target(&mut rng, &w, Side::Right, 38.0, 166.4, 128.0).unwrap();
            assert!(x <= 1882.0 && x >= 1882.0 - 64.0 && (166.4..=1040.0).contains(&y));
            assert_eq!(edge_ahead(&w, x, y, 38.0, 128.0, 1.0), Some(Side::Right));
        }
        let (x, y) = entry_point(&w, Side::Left, 38.0, 166.4);
        assert_eq!(x, -38.0);
        assert!((y - (166.4 + 0.4 * (1040.0 - 166.4))).abs() < 1e-9);
        assert_eq!(entry_point(&w, Side::Right, 38.0, 166.4).0, 1958.0);
    }
}
