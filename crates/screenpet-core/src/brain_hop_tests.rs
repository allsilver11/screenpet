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

// ---- 토끼: 걷는 시간의 절반쯤 깡총 (pet.json hop_share 0.5) ----

fn rabbit() -> PetConfig {
    PetConfig { size: SIZE, half_w: 0.3 * SIZE, walk_speed: 120.0, hop_share: 0.5, ..sim_cfg() }
}

/// 깡총 토막: 발은 바닥에, 그림만 낮게(≤ 0.24 × 크기) 깡총, 평균 속도는 걷기 그대로, 그림은 hop → land
#[test]
fn rabbit_hop_segment_is_a_low_quick_hop_at_walking_speed() {
    let w = world(vec![]);
    let seed = (0..100)
        .find(|&s| {
            let mut p = Pet::new(rabbit(), 0.0, 0.0, s);
            place(&mut p, &w, Owner::Ground(0), 300.0, walking(Move::Walk));
            p.update(DT, &w);
            p.bunny_hopping()
        })
        .expect("some seed starts with a hop segment");
    let mut p = Pet::new(rabbit(), 0.0, 0.0, seed);
    place(&mut p, &w, Owner::Ground(0), 300.0, walking(Move::Walk));
    p.facing = 1.0;
    let (mut max_lift, mut hops, mut was) = (0.0f64, 0, Anim::Idle);
    let ticks = 180;
    for _ in 0..ticks {
        p.update(DT, &w);
        assert!(p.bunny_hopping(), "one segment keeps its gait");
        assert_eq!(p.body.y, 1040.0);
        let lift = 1040.0 - SIZE - p.window_pos().1;
        max_lift = max_lift.max(lift);
        let a = p.render().anim;
        assert!(matches!(a, Anim::Hop | Anim::Land | Anim::Idle), "{a:?}");
        if a == Anim::Hop && was != Anim::Hop {
            hops += 1;
        }
        was = a;
    }
    assert!(max_lift > 0.2 * SIZE && max_lift <= hop::BUNNY_WALK_HOP_HEIGHT * SIZE + 1e-9, "lift {max_lift}");
    assert!(hops >= 4, "hops {hops}");
    let avg = (p.body.x - 300.0) / (ticks as f64 * DT);
    let speed = 120.0 * Move::Walk.speed_mult();
    assert!((avg - speed).abs() < 0.2 * speed, "avg {avg} vs {speed}");
    assert!(!matches!(p.state, State::Leap { .. }));
}

/// 토끼 10분 × 10: 걷기·달리기 시간의 절반쯤(±10%) 깡총, 깡총은 낮고 늘 디딤판 위,
/// 큰 점프·벽 폴짝 오르기는 없고, 깡총 아닌 토막은 예전처럼 평평하게 걷는다
#[test]
fn rabbit_hops_about_half_of_its_walking_time() {
    let w = world(vec![win(1, 300.0, 750.0, 900.0, 1000.0), win(2, 1200.0, 500.0, 1700.0, 800.0)]);
    let (mut walking_ticks, mut hopping_ticks, mut switches) = (0usize, 0usize, 0usize);
    for seed in 0..10 {
        let mut p = Pet::new(rabbit(), 960.0, 128.0, seed);
        let mut prev: Option<(f64, bool, bool)> = None;
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Leap { .. }), "seed {seed}: no big jumps");
            let lift = p.body.y - SIZE - p.window_pos().1;
            assert!((0.0..=hop::BUNNY_RUN_HOP_HEIGHT * SIZE + 1e-9).contains(&lift), "seed {seed}: lift {lift}");
            match p.state {
                State::Move { kind: kind @ (Move::Walk | Move::Run), remaining } => {
                    let st = p.standing.expect("moving rabbit stands on something");
                    assert_eq!(w.anchor(st.owner).unwrap().1, p.body.y, "seed {seed}: feet on the surface");
                    walking_ticks += 1;
                    let on = p.bunny_hopping();
                    if on {
                        hopping_ticks += 1;
                    } else {
                        assert_eq!(lift, 0.0);
                        assert_eq!(p.render().anim, Anim::from(kind));
                    }
                    // 토막 첫 틱은 이동이 다른 상태에서 막 정해진 틱일 수 있어 걸음을 아직 안 골랐다 — 둘째 틱부터 비교
                    if let Some((r, was, first)) = prev {
                        if !first && remaining < r && was != on {
                            switches += 1;
                        }
                    }
                    let first = prev.is_none_or(|(r, ..)| remaining > r);
                    prev = Some((remaining, on, first));
                }
                _ => {
                    assert!(!p.bunny_hopping() || matches!(p.state, State::Move { .. }));
                    prev = None;
                }
            }
        }
    }
    let share = hopping_ticks as f64 / walking_ticks as f64;
    assert!(walking_ticks > 60 * 600, "walked {walking_ticks}");
    assert!((0.4..=0.6).contains(&share), "hop share {share:.3}");
    assert_eq!(switches, 0, "the gait changes only between segments");
}

/// 다른 걷는 동물(hop_share 0)은 절대 깡총 걸음을 하지 않는다
#[test]
fn other_walkers_never_hop_walk() {
    let w = world(vec![win(1, 300.0, 750.0, 900.0, 1000.0)]);
    for seed in 0..4 {
        let mut p = Pet::new(PetConfig { hop_share: 0.0, ..rabbit() }, 960.0, 128.0, seed);
        for _ in 0..(60 * 300) {
            p.update(DT, &w);
            assert!(!p.bunny_hopping());
            if let State::Move { kind, .. } = p.state {
                assert_eq!(p.window_pos().1, p.body.y - SIZE);
                assert_eq!(p.render().anim, Anim::from(kind));
            }
        }
    }
}

/// 토끼도 보내기·잡아 봐!는 그대로 된다 (보내면 손 흔들고 그 쪽으로 사라진다)
#[test]
fn rabbit_is_sent_off_and_plays_catch_me() {
    let w = world(vec![]);
    for seed in 1..=8 {
        let mut p = Pet::new(rabbit(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 1200.0, State::Idle { remaining: 100.0, look: false });
        assert!(p.send_to(Side::Left, false));
        let mut out = None;
        for _ in 0..(60 * 40) {
            p.update(DT, &w);
            if let Some(side) = p.take_exit() {
                out = Some(side);
                break;
            }
        }
        assert_eq!(out, Some(Side::Left), "seed {seed}");
    }
    let mut p = Pet::new(rabbit(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
    p.set_guest(Side::Left, 1000.0);
    p.start_catch_me();
    assert!(p.is_catch_me());
    for _ in 0..60 {
        p.update(DT, &w);
        assert_eq!(p.body.y, 1040.0);
        assert_eq!(p.window_pos().1, 1040.0 - SIZE, "the rabbit runs (no frog hop) while playing");
    }
}
