//! 콰당! 💥 (brain.rs land_or_recover): 높이(몸 THUD_FALL_SIZES배 이상)에서 떨어지거나 던져져 세게 내려서면 바로 일어나지 않고
//! 납작 엎어져 THUD_SECS 동안 어지러워함 (render.thud — 웹뷰가 찌그러짐·💫), 말풍선 "oof!". 살짝 내려앉으면 평소처럼 사뿐.

use super::tests::{cfg, place, win, world};
use super::*;
use crate::test_util::{DT, flyer, idle};

/// 바닥(1040) 위 높이 h에서 놓아 떨어뜨리고, 내려설 때까지 돌림 → (콰당이었나, 그동안 나온 말)
fn drop_from(c: PetConfig, h: f64) -> (Pet, World, bool, Vec<Speech>) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 800.0, 1040.0 - h, 1);
    p.start_fall();
    let (mut thud, mut said) = (false, Vec::new());
    for _ in 0..(60 * 4) {
        p.update(DT, &w);
        thud |= p.render().thud;
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        if p.standing.is_some() && !p.render().thud {
            break;
        }
    }
    (p, w, thud, said)
}

#[test]
fn a_long_fall_lands_with_a_thud_and_an_oof() {
    let (_, _, thud, said) = drop_from(cfg(), 600.0);
    assert!(thud, "fell from high up");
    assert_eq!(said, vec![Speech::Oof]);
}

#[test]
fn a_small_drop_still_lands_softly() {
    let (_, _, thud, said) = drop_from(cfg(), cfg().size * 0.8);
    assert!(!thud);
    assert!(said.is_empty());
}

#[test]
fn the_thud_lasts_a_moment_then_the_pet_gets_up() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 800.0, 400.0, 2);
    p.start_fall();
    while !p.render().thud {
        p.update(DT, &w);
    }
    let mut t = 0.0;
    while p.render().thud {
        assert!(matches!(p.state, State::Land { .. }), "stays down: {:?}", p.state);
        assert_eq!(p.render().anim, Anim::Land);
        p.update(DT, &w);
        t += DT;
    }
    assert!((t - THUD_SECS).abs() < 0.1, "{t}");
    for _ in 0..30 {
        p.update(DT, &w);
    }
    assert!(!p.render().thud);
}

#[test]
fn a_hard_throw_onto_a_window_top_thuds() {
    let w = world(vec![win(1, 400.0, 600.0, 1400.0, 1000.0)]);
    let mut p = Pet::new(cfg(), 800.0, 590.0, 3);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.pointer_down(800.0, 560.0);
    for i in 0..8 {
        p.pointer_move(800.0, 560.0 - 60.0 * i as f64);
        p.update(DT, &w);
    }
    p.pointer_up(800.0, 140.0);
    let mut thud = false;
    for _ in 0..(60 * 5) {
        p.update(DT, &w);
        thud |= p.render().thud;
    }
    assert!(thud, "thrown up high, came down hard");
}

#[test]
fn flying_pets_never_thud() {
    let (_, _, thud, _) = drop_from(flyer(), 600.0);
    assert!(!thud);
}

#[test]
fn no_thud_while_the_user_is_away_the_pet_just_sleeps_on() {
    // 자리 비움 중엔 잠만 잔다 — 떨어져도 말 없이 다시 잠든다 (brain_away_tests)
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 800.0, 300.0, 4);
    p.set_user_away(true);
    p.start_fall();
    for _ in 0..(60 * 3) {
        p.update(DT, &w);
        assert!(!p.render().thud);
    }
}
