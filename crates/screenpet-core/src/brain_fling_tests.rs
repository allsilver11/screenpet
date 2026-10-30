//! 던져서 친구 화면으로 넘어가기 (듀얼 모니터처럼, brain_fling.rs). brain.rs의 자식 모듈.

use super::tests::{cfg, place, world};
use super::*;
use crate::geom::Rect;
use crate::visit::Fling;

const DT: f64 = 1.0 / 60.0;

/// (x, y)에서 vx·vy(물리 px/s)로 던져진 동물
fn thrown_at(cfg: PetConfig, x: f64, y: f64, vx: f64, vy: f64) -> Pet {
    let mut p = Pet::new(cfg, x, y, 1);
    p.state = State::Air { anim: Anim::Fall };
    p.body = Body { x, y, vx, vy };
    p.thrown = true;
    p
}

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, ..cfg() }
}

/// 나갈 때까지 (최대 ticks틱) — 나간 쪽
fn exit_within(p: &mut Pet, w: &World, ticks: usize) -> Option<Side> {
    for _ in 0..ticks {
        p.update(DT, w);
        if let Some(side) = p.take_exit() {
            return Some(side);
        }
    }
    None
}

#[test]
fn thrown_pet_flies_across_a_connected_edge_carrying_height_and_speed() {
    let w = world(vec![]);
    let mut p = thrown_at(cfg(), 1800.0, 520.0, 900.0, -300.0);
    p.set_exit_edges(false, true);
    assert_eq!(exit_within(&mut p, &w, 30), Some(Side::Right));
    assert!(p.is_away());
    let f = p.take_fling().expect("fling carried");
    assert!(f.height > 0.3 && f.height < 0.6, "{f:?}");
    assert!(f.vx > 500.0 && f.vy < 0.0, "{f:?}");
    assert_eq!(p.take_fling(), None, "taken once");
}

#[test]
fn speed_is_carried_in_logical_px_and_clamped() {
    let dpi = 2.0;
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 3840.0, 2080.0)], 256.0);
    let c = PetConfig { size: 256.0, half_w: 76.0, walk_speed: 120.0, dpi, ..cfg() };
    let mut p = thrown_at(c, 200.0, 1000.0, -1200.0, 0.0);
    p.set_exit_edges(true, false);
    assert_eq!(exit_within(&mut p, &w, 30), Some(Side::Left));
    let f = p.take_fling().unwrap();
    assert!(f.vx < -500.0 && f.vx > -700.0, "logical px/s: {f:?}");
    let mut fast = thrown_at(c, 200.0, 1000.0, -100000.0, 0.0);
    fast.set_exit_edges(true, false);
    exit_within(&mut fast, &w, 5).unwrap();
    assert!(fast.take_fling().unwrap().vx >= -visit::MAX_FLING_SPEED - 1e-6);
}

#[test]
fn unlinked_edge_or_a_plain_fall_bounces_as_before() {
    let w = world(vec![]);
    // 연결된 친구가 없는 끝
    let mut p = thrown_at(cfg(), 1800.0, 520.0, 900.0, 0.0);
    p.set_exit_edges(true, false);
    assert_eq!(exit_within(&mut p, &w, 120), None);
    assert!(p.body.x < 1920.0 - 38.0 + 1e-6);
    // 던진 게 아니라 그냥 떨어지는 중
    let mut q = thrown_at(cfg(), 1800.0, 520.0, 900.0, 0.0);
    q.thrown = false;
    q.set_exit_edges(true, true);
    assert_eq!(exit_within(&mut q, &w, 120), None);
}

#[test]
fn guest_thrown_into_its_home_edge_flies_home_and_other_edges_follow_roaming_rules() {
    let w = world(vec![]);
    let mut g = thrown_at(cfg(), 150.0, 520.0, -900.0, 0.0);
    g.set_guest(Side::Left, 100.0);
    assert_eq!(exit_within(&mut g, &w, 30), Some(Side::Left));
    assert!(g.take_fling().is_some());
    // 집 반대쪽: 거기 (주인 아닌) 연결된 친구가 있으면 넘어간다
    let mut r = thrown_at(cfg(), 1800.0, 520.0, 900.0, 0.0);
    r.set_guest(Side::Left, 100.0);
    r.set_exit_edges(false, true);
    assert_eq!(exit_within(&mut r, &w, 30), Some(Side::Right));
    // 집이 그리운 손님은 다른 친구 쪽으로 넘어가지 않는다
    let mut h = thrown_at(cfg(), 1800.0, 520.0, 900.0, 0.0);
    h.set_guest(Side::Left, 0.0);
    h.set_exit_edges(false, true);
    assert_eq!(exit_within(&mut h, &w, 60), None);
}

#[test]
fn flung_in_pet_enters_airborne_at_that_height_and_lands() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    p.enter_flung(&w, Side::Left, Fling { height: 0.5, vx: 700.0, vy: -200.0 });
    assert_eq!(p.state, State::Air { anim: Anim::Fall });
    assert!(p.thrown);
    assert!((p.body.x - 38.0).abs() < 1e-6, "at the left edge: {:?}", p.body);
    assert!((p.body.y - 520.0).abs() < 1e-6, "{:?}", p.body);
    assert!(p.body.vx > 0.0 && p.body.vy < 0.0, "keeps moving inward: {:?}", p.body);
    assert_eq!(p.facing, 1.0);
    for _ in 0..600 {
        p.update(DT, &w);
        if p.standing.is_some() {
            break;
        }
    }
    assert!(p.standing.is_some() && p.body.x > 200.0, "{:?}", p.body);
    // 들어오는 쪽 끝은 그 방향과 상관없이 안쪽으로
    let mut q = Pet::new(cfg(), 0.0, 0.0, 1);
    q.enter_flung(&w, Side::Right, Fling { height: 0.0, vx: 700.0, vy: 0.0 });
    assert!(q.body.vx < 0.0 && q.body.x > 1800.0);
    assert!(q.body.y >= 128.0, "feet low enough for the head: {:?}", q.body);
}

#[test]
fn garbage_fling_falls_back_to_walking_in() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    p.enter_flung(&w, Side::Left, Fling { height: f64::NAN, vx: 1.0, vy: 1.0 });
    assert!(matches!(p.state, State::Enter { side: Side::Left, .. }));
}

#[test]
fn bee_drifts_in_and_settles() {
    let w = world(vec![]);
    let mut b = Pet::new(bee(), 0.0, 0.0, 1);
    b.enter_flung(&w, Side::Right, Fling { height: 0.3, vx: 600.0, vy: 0.0 });
    assert_eq!(b.state, State::Air { anim: Anim::Fall });
    let x0 = b.body.x;
    for _ in 0..600 {
        b.update(DT, &w);
    }
    assert!(b.body.x < x0 - 50.0, "drifted in: {:?}", b.body);
    assert!(!matches!(b.state, State::Air { .. }), "settled: {:?}", b.state);
}

#[test]
fn thrown_bee_crosses_a_linked_edge() {
    let w = world(vec![]);
    let mut b = thrown_at(bee(), 1750.0, 500.0, 1200.0, 0.0);
    b.set_exit_edges(false, true);
    assert_eq!(exit_within(&mut b, &w, 60), Some(Side::Right));
    assert!(b.take_fling().is_some());
}

#[test]
fn drag_release_on_the_ground_without_speed_does_not_cross() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 60.0, State::Idle { remaining: 100.0, look: false });
    p.set_exit_edges(true, true);
    for _ in 0..60 {
        p.update(DT, &w);
    }
    assert_eq!(p.take_exit(), None);
}
