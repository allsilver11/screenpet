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

/// 상태 때문에 건너뛰는 경우: 자는 중 · 들림 · 먹는 중 · 놀러 감 · 공중 · 이미 축하 중
#[test]
fn pets_in_other_states_skip_the_reaction() {
    for state in [
        State::Sleep { remaining: 5.0 },
        State::Dragged,
        State::Eating { remaining: 1.0, favorite: false, level_up: false },
        State::Away,
        State::Slide,
        State::LevelUp { remaining: 1.0 },
    ] {
        let (mut p, _) = grounded(state);
        assert!(!p.react_to_result(true), "{state:?}");
        assert_eq!(p.state, state);
    }
}

/// 볼일 때문에 건너뛰는 경우: 걷거나 쉬는 상태(그래서 상태 검사는 통과함)에서 볼일 하나만 있음
/// 끊으면 안 됨 — 머리 위 친구가 있으면 탑이 무너지고, 먹이 길이면 밥을 놓침
#[test]
fn pets_on_an_errand_or_under_a_rider_skip_the_reaction() {
    let cases: [(&str, fn(&mut Pet)); 12] = [
        ("rider", |p| p.set_has_rider(true)),
        ("food trip", |p| p.food = Some(FoodTrip { food: Food::Meal, heart: false, level_up: false, target: None, from_house: false })),
        ("meal pending", |p| p.meal_pending = Some((false, false))),
        ("queued food", |p| p.queued_food = Some((Food::Meal, false))),
        ("hand", |p| p.hand = Some((1.0, 1.0))),
        ("control", |p| p.set_control(true)),
        ("home trip", |p| p.home_trip = Some(HomeTrip::new(Owner::Ground(0), 100.0, 800.0))),
        ("back to the house", |p| p.return_home = Some((Owner::Ground(0), 100.0))),
        ("sending", |p| p.send = Some(send_moves::SendTrip { side: Side::Left, letter: false, reply: false, quiet: false })),
        ("farewell", |p| p.farewell_pending = true),
        ("inside a window", |p| p.inside = Some(crate::inside::Stay { window: 1, mode: crate::inside::Mode::In, leave_in: 100.0, climb_out: false, hops: 0 })),
        ("housed", |p| p.housed = true),
    ];
    for (name, setup) in cases {
        for good in [true, false] {
            let (mut p, _) = grounded(State::Idle { remaining: 10.0, look: false });
            setup(&mut p);
            assert!(!p.react_to_result(good), "{name}");
            assert!(matches!(p.state, State::Idle { .. }), "{name}: {:?}", p.state);
        }
    }
    // 손님으로 와 있어도 (자기 집이 아니다)
    let (mut p, _) = grounded(State::Idle { remaining: 10.0, look: false });
    p.guest = Some(crate::visit::Guest { home: Side::Left, homesick_in: 100.0 });
    assert!(!p.react_to_result(true));
}

/// 볼일이 끝나면 다시 반응함 (위 검사가 무조건 false라서 통과한 게 아님)
#[test]
fn a_rider_stepping_off_lets_the_pet_react_again() {
    let (mut p, _) = grounded(State::Idle { remaining: 10.0, look: false });
    p.set_has_rider(true);
    assert!(!p.react_to_result(true));
    p.set_has_rider(false);
    assert!(p.react_to_result(true));
}
