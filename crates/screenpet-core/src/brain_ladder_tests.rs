//! 사다리타기 결과 반응 (spec 2026-11-04 §1-6) — brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, world};
use super::*;

fn grounded(state: State) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 600.0, state);
    (p, w)
}

#[test]
fn good_results_cheer_and_bad_ones_tilt() {
    let (mut p, _) = grounded(State::Idle { remaining: 10.0, look: false });
    assert!(p.react_to_result(true));
    assert!(matches!(p.state, State::LevelUp { .. }));
    let (mut p, _) = grounded(State::Move { kind: Move::Walk, remaining: 3.0 });
    assert!(p.react_to_result(false));
    assert!(matches!(p.state, State::Trick { kind: Trick::Tilt, .. }));
}

#[test]
fn busy_or_away_pets_skip_the_reaction() {
    for state in [State::Sleep { remaining: 5.0 }, State::Dragged, State::Eating { remaining: 1.0, favorite: false, level_up: false }, State::Away] {
        let (mut p, _) = grounded(state.clone());
        assert!(!p.react_to_result(true), "{state:?}");
        assert_eq!(p.state, state);
    }
    let (mut p, _) = grounded(State::Idle { remaining: 10.0, look: false });
    p.housed = true;
    assert!(!p.react_to_result(true));
    // 조작 모드로 움직이는 중이면 끊지 않는다
    let (mut p, _) = grounded(State::Move { kind: Move::Walk, remaining: 3.0 });
    p.set_control(true);
    assert!(p.is_controlled());
    assert!(!p.react_to_result(true));
}
