//! 흐물이 (pet.json timid · energetic · rigging) — 겁먹어 만세 줄행랑(State::Panic), 기운 넘치는 달리기·큰 점프,
//! 창 옆면에 뱃사람처럼 매달리기(State::Rig). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{place, run, w1, win, world};
use super::*;
use super::heumul_moves::RigPhase;
use crate::test_util::{DT, idle, pet_cfg};

fn heumul() -> PetConfig {
    PetConfig { half_w: 38.4, walk_speed: 110.0, personality: Personality::Playful, timid: true, energetic: true, rigging: true, ..pet_cfg() }
}

fn on_ground(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, idle());
    p
}

fn panicking(p: &Pet) -> bool {
    matches!(p.state, State::Panic { .. })
}

// ---- 겁먹어 줄행랑 ----

#[test]
fn panic_hang_and_rope_are_optional_side_animations() {
    for (a, name) in [(Anim::Panic, "panic"), (Anim::Hang, "hang"), (Anim::Rope, "rope")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::OPTIONAL_ANIMS.contains(&name));
        assert!(!a.is_front() && !a.is_back(), "{name}");
    }
}

#[test]
fn a_scared_heumul_turns_its_back_throws_its_arms_up_and_runs_away_then_calms_down() {
    let w = world(vec![]);
    for (from, away) in [(900.0, -1.0), (700.0, 1.0)] {
        let mut p = on_ground(heumul(), 800.0, 3, &w);
        p.facing = -away; // 친구 쪽을 보고 있었다
        assert!(p.start_panic(from));
        assert!(panicking(&p));
        assert_eq!(p.facing, away, "등을 돌린다");
        assert_eq!(p.take_speech(), Some(Speech::Eek));
        let r = p.render();
        assert_eq!(r.anim, Anim::Panic);
        assert_eq!(r.flip, away < 0.0);
        let x0 = p.body.x;
        let mut secs = 0.0;
        while panicking(&p) {
            p.update(DT, &w);
            secs += DT;
            assert!(secs < 5.0, "panic should end");
        }
        assert!((2.0..=3.2).contains(&secs), "ran {secs:.2}s");
        let ran = (p.body.x - x0) * away;
        let run_speed = p.cfg.walk_speed * Move::Run.speed_mult();
        assert!(ran >= run_speed * 2.0 * 0.95, "ran {ran:.0}px in {secs:.2}s");
        assert!(p.standing.is_some());
        assert!(!matches!(p.state, State::Panic { .. }));
    }
}

#[test]
fn panic_stops_sensibly_at_the_end_of_a_window_top() {
    let w = world(vec![w1()]); // 창 윗면 x 500..1100
    let mut p = Pet::new(heumul(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Window(1), 1000.0, idle());
    assert!(p.start_panic(900.0));
    for _ in 0..(60 * 4) {
        p.update(DT, &w);
        assert!(p.body.x <= 1100.0 - p.cfg.half_w + 0.01, "x {}", p.body.x);
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "뛰어내리지 않는다");
    assert!(!panicking(&p), "끝에 닿으면 멈춰 숨을 고른다: {:?}", p.state);
}

#[test]
fn only_timid_free_pets_panic() {
    let w = world(vec![]);
    let mut cat = on_ground(pet_cfg(), 800.0, 1, &w);
    let dice = cat.rng.get_seed();
    assert!(!cat.start_panic(900.0));
    assert_eq!(cat.rng.get_seed(), dice, "겁쟁이가 아닌 동물의 주사위는 그대로");
    let mut p = on_ground(heumul(), 800.0, 1, &w);
    p.state = State::Sleep { remaining: 10.0 };
    assert!(!p.start_panic(900.0));
    let mut p = on_ground(heumul(), 800.0, 1, &w);
    p.pointer_down(p.body.x, p.body.y - 40.0);
    assert!(!p.start_panic(900.0));
}

#[test]
fn panic_without_the_panic_picture_is_never_configured() {
    let json = |extra: &str| {
        let anims = crate::manifest::REQUIRED_ANIMS.iter().map(|a| format!("\"{a}\":{{\"row\":0,\"frames\":1,\"fps\":1}}")).collect::<Vec<_>>().join(",");
        format!("{{\"name\":\"t\",\"frame_size\":128,\"walk_speed\":60,\"timid\":true,\"rigging\":true,\"energetic\":true,\"animations\":{{{anims}{extra}}}}}")
    };
    let m = crate::manifest::parse_manifest(&json("")).unwrap();
    let c = PetConfig::from_manifest(&m, 1.0, 1.0, "teenager");
    assert!(!c.timid && !c.rigging && c.energetic);
    let m = crate::manifest::parse_manifest(&json(",\"panic\":{\"row\":0,\"frames\":4,\"fps\":12},\"hang\":{\"row\":0,\"frames\":4,\"fps\":6},\"rope\":{\"row\":0,\"frames\":4,\"fps\":8}")).unwrap();
    let c = PetConfig::from_manifest(&m, 1.0, 1.0, "teenager");
    assert!(c.timid && c.rigging && c.energetic);
}

// ---- 기운 넘치게 ----

fn first_moves(c: PetConfig, n: u64) -> (usize, usize) {
    let w = world(vec![]);
    let (mut runs, mut walks) = (0, 0);
    for seed in 0..n {
        let mut p = on_ground(c, 800.0, seed, &w);
        match p.move_state(true, &w) {
            State::Move { kind: Move::Run, .. } => runs += 1,
            State::Move { kind: Move::Walk, .. } => walks += 1,
            State::Idle { .. } => match p.pacing.after_beat {
                Some(AfterBeat::Go(State::Move { kind: Move::Run, .. })) => runs += 1,
                Some(AfterBeat::Go(State::Move { kind: Move::Walk, .. })) => walks += 1,
                _ => {}
            },
            _ => {}
        }
    }
    (runs, walks)
}

#[test]
fn an_energetic_pet_runs_far_more_than_it_walks() {
    let calm = PetConfig { timid: false, energetic: false, rigging: false, ..heumul() };
    let (r0, w0) = first_moves(calm, 2000);
    let (r1, w1_) = first_moves(PetConfig { rigging: false, ..heumul() }, 2000);
    assert!(r1 as f64 > r0 as f64 * 1.5, "runs {r0} → {r1}");
    assert!(r1 > w1_, "runs {r1} walks {w1_}");
    assert!(w0 > r0, "보통 동물은 그대로 걷기가 더 많다: {w0} {r0}");
}

fn leaps(c: PetConfig, n: u64) -> usize {
    // 왼쪽 바닥에서 닿을 만한 낮은 창 윗면
    let w = world(vec![win(1, 900.0, 900.0, 1300.0, 1040.0)]);
    (0..n)
        .filter(|&seed| {
            let mut p = on_ground(c, 800.0, seed, &w);
            p.big_jump_state(&w).is_some()
        })
        .count()
}

#[test]
fn an_energetic_pet_leaps_onto_windows_about_twice_as_often() {
    let calm = PetConfig { timid: false, energetic: false, rigging: false, ..heumul() };
    let (a, b) = (leaps(calm, 3000), leaps(PetConfig { rigging: false, ..heumul() }, 3000));
    assert!(a > 0);
    let ratio = b as f64 / a as f64;
    assert!((1.6..=2.5).contains(&ratio), "{a} → {b}");
}

// ---- 창 옆면 매달리기 ----

/// 바닥에서 떠 있는 창: 왼쪽 옆면 x=1000, 위 500, 아래 900 (바닥 1040)
fn mast() -> World {
    world(vec![win(1, 1000.0, 500.0, 1400.0, 900.0)])
}

fn rigging(p: &Pet) -> Option<RigPhase> {
    match p.state {
        State::Rig { phase, .. } => Some(phase),
        _ => None,
    }
}

#[test]
fn near_a_window_side_the_heumul_jumps_and_hangs_from_it_like_a_sailor() {
    let w = mast();
    for (x, side, edge, facing) in [(880.0, Side::Left, 1000.0, 1.0), (1520.0, Side::Right, 1400.0, -1.0)] {
        let mut p = on_ground(heumul(), x, 7, &w);
        assert!(p.try_rig(&w), "{side:?}");
        assert_eq!(rigging(&p), Some(RigPhase::Jump));
        assert_eq!(p.render().anim, Anim::Hop);
        assert!(run_until_phase(&mut p, &w, |ph| ph == RigPhase::Hang), "{:?}", p.state);
        let State::Rig { side: s, .. } = p.state else { unreachable!() };
        assert_eq!(s, side);
        assert!((p.body.x - p.wall_x(edge, side)).abs() < 0.5, "x {}", p.body.x);
        assert!(p.body.y < 1040.0 - 0.3 * p.cfg.size && p.body.y > 500.0, "y {}", p.body.y);
        assert_eq!(p.facing, facing, "창 쪽을 본다");
        let r = p.render();
        assert_eq!((r.anim, r.flip), (Anim::Hang, facing < 0.0));
    }
}

fn run_until_phase(p: &mut Pet, w: &World, want: impl Fn(RigPhase) -> bool) -> bool {
    for _ in 0..(60 * 10) {
        p.update(DT, w);
        if rigging(p).is_some_and(&want) {
            return true;
        }
    }
    false
}

#[test]
fn it_hangs_climbs_the_side_like_a_rope_and_then_leaps_off_and_lands() {
    let w = mast();
    let mut saw_rope = 0;
    for seed in 0..12 {
        let mut p = on_ground(heumul(), 880.0, seed, &w);
        assert!(p.try_rig(&w));
        let (mut hang, mut rope, mut moved) = (0.0, 0.0, false);
        let mut last_y = None;
        let mut secs = 0.0;
        while rigging(&p).is_some() {
            p.update(DT, &w);
            secs += DT;
            assert!(secs < 30.0, "rigging should end");
            match rigging(&p) {
                Some(RigPhase::Hang) => {
                    hang += DT;
                    assert_eq!(p.render().anim, Anim::Hang);
                }
                Some(RigPhase::Rope { .. }) => {
                    rope += DT;
                    assert_eq!(p.render().anim, Anim::Rope);
                    if let Some(y) = last_y {
                        moved |= (p.body.y - y as f64).abs() > 0.01;
                    }
                    last_y = Some(p.body.y);
                    // 옆면 위아래 안에서만
                    assert!(p.body.y - p.cfg.size >= 500.0 - 1.0 - p.cfg.size && p.body.y <= 900.0 + 1.0, "y {}", p.body.y);
                }
                _ => {}
            }
        }
        assert!(hang >= 1.0, "hang {hang:.1}");
        if rope > 0.0 {
            saw_rope += 1;
            assert!(moved);
        }
        // 뛰어내려 떨어진 뒤 바닥에 내려선다
        assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
        assert!(p.body.vx * -1.0 > 0.0 || p.body.vx == 0.0, "창 반대쪽으로 뛰어내린다: vx {}", p.body.vx);
        assert!(run_until_standing(&mut p, &w), "{:?}", p.state);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    }
    assert!(saw_rope >= 6, "rope climbing in {saw_rope}/12");
}

fn run_until_standing(p: &mut Pet, w: &World) -> bool {
    for _ in 0..(60 * 10) {
        p.update(DT, w);
        if p.standing.is_some() {
            return true;
        }
    }
    false
}

#[test]
fn no_rigging_far_from_windows_for_pets_without_it_or_on_covered_sides() {
    let w = mast();
    let mut far = on_ground(heumul(), 300.0, 1, &w);
    assert!(!far.try_rig(&w));
    let mut cat = on_ground(PetConfig { rigging: false, ..heumul() }, 880.0, 1, &w);
    let dice = cat.rng.get_seed();
    assert!(!cat.try_rig(&w));
    assert_eq!(cat.rng.get_seed(), dice);
    // 앞쪽 창이 옆면을 덮었다
    let covered = world(vec![win(2, 900.0, 300.0, 1100.0, 1000.0), win(1, 1000.0, 500.0, 1400.0, 900.0)]);
    let mut p = on_ground(heumul(), 880.0, 1, &covered);
    assert!(!p.try_rig(&covered));
}

#[test]
fn if_the_window_goes_away_it_falls() {
    let w = mast();
    let mut p = on_ground(heumul(), 880.0, 2, &w);
    assert!(p.try_rig(&w));
    assert!(run_until_phase(&mut p, &w, |ph| ph == RigPhase::Hang));
    let empty = world(vec![]);
    p.update(DT, &empty);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
}

#[test]
fn rigging_is_one_of_its_everyday_activities_near_windows() {
    let w = mast();
    let mut rigged = 0;
    for seed in 0..300 {
        let mut p = on_ground(heumul(), 880.0, seed, &w);
        p.move_state(true, &w);
        rigged += rigging(&p).is_some() as usize;
    }
    assert!((15..=90).contains(&rigged), "rigged {rigged}/300");
    let calm = PetConfig { rigging: false, ..heumul() };
    for seed in 0..100 {
        let mut p = on_ground(calm, 880.0, seed, &w);
        p.move_state(true, &w);
        assert!(rigging(&p).is_none());
    }
}

#[test]
fn picking_it_up_while_hanging_lets_go() {
    let w = mast();
    let mut p = on_ground(heumul(), 880.0, 4, &w);
    assert!(p.try_rig(&w));
    assert!(run_until_phase(&mut p, &w, |ph| ph == RigPhase::Hang));
    p.pointer_down(p.body.x, p.body.y - 40.0);
    assert!(p.is_held());
    run(&mut p, &w, 5);
    assert!(rigging(&p).is_none());
}
