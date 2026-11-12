//! 🫣 숨바꼭질 (흐물이 v0.6.3)

use super::hide_moves::{HidePhase, HIDE_INSET, HIDE_SECS};
use super::swing_tests::{heumul, resting};
use super::tests::{win, world};
use super::*;
use crate::test_util::{pet_cfg, DT};

/// 바닥까지 내려온 창 (왼쪽 옆면 x 1000)
fn tall() -> crate::geom::WindowInfo {
    win(1, 1000.0, 500.0, 1500.0, 1040.0)
}

fn run_until(p: &mut Pet, w: &World, secs: f64, cursor: (f64, f64), done: impl Fn(&Pet) -> bool) -> bool {
    let mut t = 0.0;
    while t < secs {
        p.hover(cursor.0, cursor.1, false);
        p.update(DT, w);
        if done(p) {
            return true;
        }
        t += DT;
    }
    false
}

fn peeking(p: &Pet) -> bool {
    matches!(p.state, State::Hide { phase: HidePhase::Peek { .. }, .. })
}

#[test]
fn it_runs_behind_the_window_side_and_peeks_out_with_only_its_head() {
    let w = world(vec![tall()]);
    let mut p = resting(heumul(), 700.0, 1, &w);
    assert!(p.start_hide(&w));
    assert_eq!(p.render().anim, Anim::Run);
    assert!(run_until(&mut p, &w, 8.0, (100.0, 100.0), peeking));
    assert!((p.body.x - (1000.0 + HIDE_INSET * 128.0)).abs() < 0.5, "몸은 창 옆면 안쪽: {}", p.body.x);
    assert_eq!(p.render().anim, Anim::Peek);
    assert!(!p.render().flip, "창이 오른쪽 — 그림 그대로 (머리는 왼쪽으로 쏙)");
    assert!(p.standing.is_some());
    // 오른쪽 옆면이면 뒤집는다
    let w2 = world(vec![win(1, 300.0, 500.0, 600.0, 1040.0)]);
    let mut q = resting(heumul(), 900.0, 2, &w2);
    assert!(q.start_hide(&w2));
    assert!(run_until(&mut q, &w2, 8.0, (1800.0, 100.0), peeking));
    assert!((q.body.x - (600.0 - HIDE_INSET * 128.0)).abs() < 0.5 && q.render().flip);
}

#[test]
fn the_cursor_near_its_head_finds_it_and_it_runs_away() {
    let w = world(vec![tall()]);
    let mut p = resting(heumul(), 700.0, 3, &w);
    p.start_hide(&w);
    assert!(run_until(&mut p, &w, 8.0, (100.0, 100.0), peeking));
    let head = (980.0, 1040.0 - 80.0);
    p.hover(head.0 - 150.0, head.1, false);
    p.update(DT, &w);
    assert_eq!(p.take_speech(), Some(Speech::Found));
    match p.state {
        State::Panic { dir, .. } => assert_eq!(dir, 1.0, "커서 반대쪽으로"),
        s => panic!("{s:?}"),
    }
}

#[test]
fn if_nobody_finds_it_it_comes_out_saying_peekaboo() {
    let w = world(vec![tall()]);
    for seed in 0..4 {
        let mut p = resting(heumul(), 700.0, 10 + seed, &w);
        p.start_hide(&w);
        assert!(run_until(&mut p, &w, 8.0, (100.0, 100.0), peeking));
        let start = p.clock;
        run_until(&mut p, &w, 45.0, (100.0, 100.0), |p| !peeking(p));
        let said = p.take_speech();
        let hid = p.clock - start;
        assert!(hid >= HIDE_SECS.0 - 0.1 && hid <= HIDE_SECS.1 + 0.1, "{hid}");
        assert_eq!(said, Some(Speech::Peekaboo));
        assert!(matches!(p.state, State::Move { .. }) && p.facing < 0.0, "창 바깥쪽으로 걸어 나온다");
    }
}

#[test]
fn it_comes_out_when_the_window_goes_away() {
    let w = world(vec![tall()]);
    let mut p = resting(heumul(), 700.0, 20, &w);
    p.start_hide(&w);
    assert!(run_until(&mut p, &w, 8.0, (100.0, 100.0), peeking));
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert!(!peeking(&p));
}

#[test]
fn no_hiding_without_a_window_that_covers_the_body_or_for_other_pets() {
    // 바닥에 닿지 않는 창 (몸을 가리지 못한다)
    let w = world(vec![win(1, 1000.0, 400.0, 1500.0, 800.0)]);
    let mut p = resting(heumul(), 700.0, 1, &w);
    assert!(!p.start_hide(&w));
    // 너무 멀다
    let w = world(vec![tall()]);
    let mut p = resting(heumul(), 100.0, 2, &w);
    assert!(!p.start_hide(&w));
    // 앞을 다른 창이 덮고 있다
    let w = world(vec![win(2, 900.0, 300.0, 1200.0, 1040.0), tall()]);
    let p = resting(heumul(), 700.0, 3, &w);
    assert!(p.hide_spot_for_test(&w).is_none_or(|(id, _)| id == 2));
    // 다른 동물은 저절로 숨지 않는다
    let w = world(vec![tall()]);
    let mut cat = resting(pet_cfg(), 700.0, 4, &w);
    cat.hide.ready_at = 0.0;
    assert!(!run_until(&mut cat, &w, 300.0, (100.0, 100.0), |p| matches!(p.state, State::Hide { .. })));
}

#[test]
fn heumul_hides_now_and_then_on_its_own() {
    let w = world(vec![tall()]);
    let mut hid = 0;
    for seed in 0..10 {
        let mut p = resting(heumul(), 800.0, seed, &w);
        p.hide.ready_at = 0.0;
        hid += usize::from(run_until(&mut p, &w, 300.0, (100.0, 100.0), |p| matches!(p.state, State::Hide { .. })));
    }
    assert!(hid >= 5, "{hid}");
}
