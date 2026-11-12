//! ⌨️ 타자 응원 (흐물이 v0.6.3)

use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::typing_moves::{AFTER_SECS, BUDGET_SECS, BUDGET_WINDOW_SECS, TYPING_HOLD_SECS};
use super::*;
use crate::test_util::{pet_cfg, DT};

fn run(p: &mut Pet, w: &World, secs: f64, rate: f64) -> Vec<Speech> {
    let mut said = Vec::new();
    let mut t = 0.0;
    while t < secs {
        p.set_typing_rate(rate);
        p.update(DT, w);
        said.extend(p.take_speech());
        t += DT;
    }
    said
}

fn cheering(p: &Pet) -> bool {
    matches!(p.state, State::TypeCheer { .. })
}

#[test]
fn fast_typing_for_two_seconds_makes_it_face_the_screen_and_cheer() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 1, &w);
    run(&mut p, &w, TYPING_HOLD_SECS - 0.2, 6.0);
    assert!(!cheering(&p), "2초 전엔 아직");
    let said = run(&mut p, &w, 0.3, 6.0);
    assert!(cheering(&p));
    assert_eq!(p.render().anim, Anim::CheerType);
    assert!(Anim::CheerType.is_front(), "앞(화면)을 본다");
    assert!(said.contains(&Speech::Fighting));
    // 빠른 타자가 이어지는 동안 계속, "화이팅!"은 가끔 또
    let said = run(&mut p, &w, 12.0, 6.0);
    assert!(cheering(&p) && said.iter().filter(|s| **s == Speech::Fighting).count() >= 1);
    // 느려지면 2초 뒤 끝
    run(&mut p, &w, AFTER_SECS - 0.2, 0.5);
    assert!(cheering(&p));
    run(&mut p, &w, 0.4, 0.5);
    assert!(!cheering(&p));
}

#[test]
fn slow_typing_or_short_bursts_or_other_pets_do_nothing() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 2, &w);
    run(&mut p, &w, 10.0, 3.0);
    assert!(!cheering(&p), "초당 4타 아래");
    for _ in 0..5 {
        run(&mut p, &w, 1.5, 8.0);
        run(&mut p, &w, 0.5, 0.0);
    }
    assert!(!cheering(&p), "2초를 못 채운 타자");
    let mut cat = resting(pet_cfg(), 800.0, 3, &w);
    run(&mut cat, &w, 5.0, 9.0);
    assert!(!cheering(&cat));
}

#[test]
fn at_most_twenty_seconds_every_three_minutes() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 4, &w);
    let mut cheered = 0.0;
    let mut t = 0.0;
    while t < BUDGET_WINDOW_SECS - 5.0 {
        p.set_typing_rate(7.0);
        p.update(DT, &w);
        p.take_speech();
        if cheering(&p) {
            cheered += DT;
        }
        t += DT;
    }
    assert!(cheered <= BUDGET_SECS + 0.1 && cheered >= BUDGET_SECS - 0.5, "{cheered}");
    // 다음 3분 창에선 다시
    run(&mut p, &w, 10.0, 7.0);
    assert!(cheering(&p));
}

#[test]
fn not_while_away_focused_held_or_sleeping() {
    let w = world(vec![]);
    for case in 0..3 {
        let mut p = resting(heumul(), 800.0, 5, &w);
        match case {
            0 => p.set_focus(true),
            1 => p.state = State::Sleep { remaining: 100.0 },
            _ => p.pointer_down(p.body.x, p.body.y - 40.0),
        }
        run(&mut p, &w, 5.0, 8.0);
        assert!(!cheering(&p), "case {case}");
    }
    // 응원 중에 자리 비움이면 그만
    let mut p = resting(heumul(), 800.0, 6, &w);
    run(&mut p, &w, 3.0, 8.0);
    assert!(cheering(&p));
    p.set_user_away(true);
    run(&mut p, &w, 0.2, 8.0);
    assert!(!cheering(&p));
}
