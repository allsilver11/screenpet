//! 개구리 (locomotion hop) — 폴짝 걸음·큰 점프·벽 폴짝 오르기 brain 동작 테스트. brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{place, run, sim_cfg, win, world};
use super::*;
use crate::hop;

const DT: f64 = 1.0 / 60.0;
const SIZE: f64 = 108.8;

fn frog() -> PetConfig {
    PetConfig {
        size: SIZE,
        half_w: 0.3 * SIZE,
        walk_speed: 83.0,
        personality: Personality::Playful,
        can_climb: true,
        hops: true,
        ..sim_cfg()
    }
}

fn walking(kind: Move) -> State {
    State::Move { kind, remaining: 1000.0 }
}

/// 걷는 개구리: 발은 늘 바닥에, 그림만 낮게(≤ 0.35 × 크기) 폴짝, 떠 있을 때만 앞으로, 평균 속도는 그대로
#[test]
fn frog_walks_in_low_hops_keeping_its_feet_on_the_surface() {
    for kind in [Move::Walk, Move::Run] {
        let w = world(vec![]);
        let mut p = Pet::new(frog(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 300.0, walking(kind));
        p.facing = 1.0;
        let (mut max_lift, mut hops, mut still_ticks, mut was) = (0.0f64, 0, 0, Anim::Idle);
        let ticks = 180;
        for _ in 0..ticks {
            let x0 = p.body.x;
            p.update(DT, &w);
            assert_eq!(p.body.y, 1040.0, "{kind:?}");
            assert!(p.standing.is_some());
            let lift = 1040.0 - SIZE - p.window_pos().1;
            max_lift = max_lift.max(lift);
            let a = p.render().anim;
            if a == Anim::Hop && was != Anim::Hop {
                hops += 1;
            }
            if p.body.x == x0 {
                still_ticks += 1;
                assert!(lift.abs() < 1e-9, "moving only in the air");
            }
            was = a;
        }
        assert!(max_lift > 0.25 * SIZE && max_lift <= 0.35 * SIZE, "{kind:?} lift {max_lift}");
        assert!(hops >= 2 && still_ticks > 0, "{kind:?}: hops {hops} still {still_ticks}");
        let speed = 83.0 * kind.speed_mult();
        let avg = (p.body.x - 300.0) / (ticks as f64 * DT);
        assert!((avg - speed).abs() < 0.2 * speed, "{kind:?}: avg {avg} vs {speed}");
    }
}

#[test]
fn non_hoppers_walk_flat_as_before() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 300.0, walking(Move::Walk));
    p.facing = 1.0;
    for _ in 0..60 {
        let x0 = p.body.x;
        p.update(DT, &w);
        assert!(p.body.x > x0);
        assert_eq!(p.window_pos().1, 1040.0 - 128.0);
        assert_eq!(p.render().anim, Anim::Walk);
    }
}

/// 땅 옆 낮은 창 (윗면 800) — 땅(1040)에서 240 위
fn jump_world() -> World {
    world(vec![win(1, 400.0, 800.0, 900.0, 1000.0)])
}

#[test]
fn big_jump_crouches_then_lands_on_the_window_top() {
    let w = jump_world();
    let mut p = Pet::new(frog(), 0.0, 0.0, 1);
    let inset = 0.3 * SIZE + hop::LANDING_INSET * SIZE;
    place(&mut p, &w, Owner::Ground(0), 250.0, State::Idle { remaining: 10.0, look: false });
    p.state = State::Leap { crouch: hop::BIG_JUMP_CROUCH_SECS, remaining: 0.0, owner: Owner::Window(1), rel_x: inset };
    // 웅크리는 동안 제자리
    run(&mut p, &w, 20);
    assert!(matches!(p.state, State::Leap { crouch, .. } if crouch > 0.0));
    assert_eq!((p.body.x, p.body.y), (250.0, 1040.0));
    assert_eq!(p.render().anim, Anim::BigJump);
    let mut top = f64::MAX;
    for _ in 0..200 {
        p.update(DT, &w);
        top = top.min(p.body.y);
        if !matches!(p.state, State::Leap { .. }) {
            break;
        }
        assert_eq!(p.render().anim, Anim::BigJump);
    }
    assert!(matches!(p.state, State::Land { .. }), "{:?}", p.state);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert!((p.body.x - (400.0 + inset)).abs() < 1e-9 && p.body.y == 800.0);
    // 꼭대기는 창 윗면보다 RISE만큼 위
    assert!((top - (800.0 - hop::BIG_JUMP_RISE * SIZE)).abs() < 5.0, "apex {top}");
    assert_eq!(p.facing, 1.0);
}

#[test]
fn big_jump_falls_if_the_target_window_vanishes_mid_air() {
    let w = jump_world();
    let mut p = Pet::new(frog(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 250.0, State::Idle { remaining: 10.0, look: false });
    p.state = State::Leap { crouch: 0.1, remaining: 0.0, owner: Owner::Window(1), rel_x: 100.0 };
    run(&mut p, &w, 15);
    assert!(matches!(p.state, State::Leap { crouch, .. } if crouch == 0.0));
    let empty = world(vec![]);
    for _ in 0..600 {
        p.update(DT, &empty);
        if matches!(p.state, State::Land { .. }) {
            break;
        }
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
}

/// 이동을 정할 때 닿는 창 윗면이 있으면 12% 정도 큰 점프, 없으면 안 한다
#[test]
fn big_jumps_happen_on_about_12_percent_of_move_decisions_when_a_window_is_in_reach() {
    let share = |w: &World, x: f64| {
        let mut n = 0;
        for seed in 0..3000 {
            let mut p = Pet::new(frog(), 0.0, 0.0, seed);
            place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 10.0, look: false });
            if let Some(State::Leap { owner, .. }) = p.big_jump_state(w) {
                assert_eq!(owner, Owner::Window(1));
                n += 1;
            }
        }
        n as f64 / 3000.0
    };
    let w = jump_world();
    let s = share(&w, 250.0);
    assert!((0.09..=0.15).contains(&s), "{s}");
    // 너무 멀다 / 너무 높다
    assert_eq!(share(&w, 1500.0), 0.0);
    assert_eq!(share(&world(vec![win(1, 400.0, 500.0, 900.0, 1000.0)]), 250.0), 0.0);
    // 폴짝 동물이 아니면 안 한다
    let mut p = Pet::new(PetConfig { hops: false, ..frog() }, 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 250.0, State::Idle { remaining: 10.0, look: false });
    assert!((0..200).all(|_| p.big_jump_state(&w).is_none()));
}

/// 화면 타기: 같은 기계(목표까지 오르고 매달렸다가 미끄러져 내려옴)지만 폴짝 오르고 매달리기를 되풀이한다 (뒷모습)
#[test]
fn frog_climbs_the_screen_in_hop_ups_then_slides_down() {
    let w = world(vec![]);
    let mut p = Pet::new(frog(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 10.0, look: false });
    assert!(p.start_climb(&w));
    let State::Climb { target_y, .. } = p.state else { unreachable!() };
    let (mut clinging, mut rising, mut fast) = (0, 0, 0);
    let normal = behavior::CLIMB_SPEED * SIZE / 128.0 * DT;
    for _ in 0..(60 * 60) {
        let y0 = p.body.y;
        p.update(DT, &w);
        if !matches!(p.state, State::Climb { .. }) {
            break;
        }
        assert_eq!(p.render().anim, Anim::Climb);
        if p.body.y > target_y {
            if p.body.y == y0 {
                clinging += 1;
            } else {
                rising += 1;
                if y0 - p.body.y > 1.5 * normal {
                    fast += 1;
                }
            }
        }
    }
    assert!(clinging > 10 && rising > 10 && fast > 10, "cling {clinging} rise {rising} fast {fast}");
    assert_eq!(p.state, State::Slide);
    assert!(p.render().anim.is_back());
    for _ in 0..600 {
        p.update(DT, &w);
        if p.standing.is_some() {
            break;
        }
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
}

#[test]
fn thrown_frog_hop_climbs_a_window_side_and_stands_on_top() {
    let w = world(vec![win(1, 800.0, 300.0, 1300.0, 900.0)]);
    let mut p = Pet::new(frog(), 700.0, 600.0, 1);
    p.body.vx = 600.0;
    p.state = State::Air { anim: Anim::Fall };
    let mut still = 0;
    for _ in 0..(60 * 30) {
        let y0 = p.body.y;
        p.update(DT, &w);
        if matches!(p.state, State::WallClimb { .. }) && p.body.y == y0 {
            still += 1;
        }
        if p.standing.is_some() {
            break;
        }
    }
    assert!(still > 5, "clings between hop-ups ({still})");
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
}

/// 개구리 10분: 디딤판을 뚫고 떨어지지 않고, 폴짝은 낮게, 큰 점프가 나오고 늘 창 윗면에 내려선다, 벽도 오른다
#[test]
fn frog_ten_minutes() {
    let w = world(vec![win(1, 300.0, 750.0, 900.0, 1000.0), win(2, 1200.0, 500.0, 1700.0, 800.0)]);
    let (mut leaps, mut hops, mut climbs, mut moving, mut total) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for seed in 0..10 {
        let mut p = Pet::new(frog(), 960.0, 128.0, seed);
        let mut leap_to: Option<Owner> = None;
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            let b = w.bounds;
            assert!(p.body.x >= b.left && p.body.x <= b.right, "seed {seed}: x {}", p.body.x);
            assert!(p.body.y >= b.top + SIZE - 1e-9 && p.body.y <= b.bottom, "seed {seed}: y {} {:?}", p.body.y, p.state);
            let lift = p.body.y - SIZE - p.window_pos().1;
            assert!((0.0..=0.35 * SIZE + 1e-9).contains(&lift), "seed {seed}: lift {lift}");
            if let State::Move { .. } = p.state {
                let st = p.standing.expect("moving frog stands on something");
                assert_eq!(w.anchor(st.owner).unwrap().1, p.body.y, "seed {seed}: feet on the surface");
                moving += 1;
            }
            match (p.state, leap_to) {
                (State::Leap { owner, .. }, None) => leap_to = Some(owner),
                (State::Leap { .. }, Some(_)) => {}
                (s, Some(owner)) => {
                    assert!(matches!(s, State::Land { .. }), "seed {seed}: leap ended in {s:?}");
                    assert_eq!(p.standing.map(|s| s.owner), Some(owner), "seed {seed}");
                    assert!(matches!(owner, Owner::Window(_)));
                    leaps += 1;
                    leap_to = None;
                }
                _ => {}
            }
            match p.render().anim {
                Anim::Hop => hops += 1,
                Anim::Climb => climbs += 1,
                _ => {}
            }
            total += 1;
        }
    }
    assert!(leaps >= 10, "big jumps {leaps}");
    assert!(hops > 0 && climbs > 0, "hops {hops} climbs {climbs}");
    let m = moving as f64 / total as f64;
    assert!(m > 0.3, "moving share {m:.3}");
}
