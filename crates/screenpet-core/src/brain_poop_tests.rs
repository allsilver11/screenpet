//! 💩 똥 싸기와 똥 앞에서의 반응 (brain_poop.rs). brain.rs의 자식 모듈 (비공개 필드에 접근)

use super::tests::{cfg, place, sim_cfg, world};
use super::*;
use crate::geom::WindowInfo;
use crate::poop::{self, PoopSpot};
use crate::test_util::{DT, LoopLimit, idle};

const FLOOR: Owner = Owner::Ground(0);
const GROUND: f64 = 1040.0;

fn walking() -> State {
    State::Move { kind: Move::Walk, remaining: 100.0 }
}

fn walker(w: &World, x: f64, seed: u64) -> Pet {
    let mut p = Pet::new(sim_cfg(), x, GROUND, seed);
    place(&mut p, w, FLOOR, x, walking());
    p.facing = 1.0;
    p
}

fn poop_at(id: u64, x: f64) -> PoopSpot {
    PoopSpot { id, x, y: GROUND, owner: FLOOR }
}

/// 조건을 건드리지 않고 누르기를 n번: 쌌으면 true (앞의 쉬는 시간은 없음)
fn clicks(p: &mut Pet, n: usize) -> bool {
    (0..n).any(|_| p.try_poop().is_some())
}

#[test]
fn a_click_rarely_poops_and_then_the_pet_waits_ten_to_twenty_minutes() {
    let w = world(vec![]);
    let (mut hits, n) = (0, 200_000);
    let mut p = walker(&w, 800.0, 3);
    for _ in 0..n {
        if p.try_poop().is_some() {
            hits += 1;
            p.poop.ready_at = 0.0;
        }
    }
    let rate = hits as f64 / n as f64;
    assert!((rate - poop::CLICK_CHANCE).abs() < 0.003, "{rate}");
    for seed in 0..40 {
        let mut p = walker(&w, 800.0, seed);
        assert!(clicks(&mut p, 2000));
        let wait = p.poop.ready_at - p.clock;
        assert!((poop::COOLDOWN_SECS.0..=poop::COOLDOWN_SECS.1).contains(&wait), "{wait}");
        assert!(!clicks(&mut p, 5000), "resting after a poop");
        p.clock = p.poop.ready_at;
        assert!(clicks(&mut p, 2000));
    }
}

/// 1분에 10번 누르는 사용자: 한 마리당 10~20분에 한 번쯤 (brief의 목표)
#[test]
fn an_active_user_sees_one_every_ten_to_twenty_minutes_per_pet() {
    let w = world(vec![]);
    for clicks_per_min in [6.0, 10.0, 20.0] {
        let (mut poops, mut secs) = (0, 0.0);
        for seed in 0..20 {
            let mut p = walker(&w, 800.0, seed);
            let gap = 60.0 / clicks_per_min;
            for _ in 0..(8.0 * 3600.0 / gap) as usize {
                p.clock += gap;
                secs += gap;
                poops += p.try_poop().is_some() as usize;
            }
        }
        let every_min = secs / poops as f64 / 60.0;
        assert!((10.0..=20.0).contains(&every_min), "{clicks_per_min}/min → one in {every_min} min");
    }
}

#[test]
fn it_falls_from_just_behind_the_body_on_whichever_way_it_faces() {
    let w = world(vec![]);
    let mut p = walker(&w, 800.0, 1);
    let at = (0..2000).find_map(|_| p.try_poop()).unwrap();
    assert!(at.0 < 800.0 && at.0 > 760.0, "{at:?}");
    assert!(at.1 < GROUND, "above the feet so it lands on what the pet stands on");
    let mut p = walker(&w, 800.0, 1);
    p.facing = -1.0;
    assert!((0..2000).find_map(|_| p.try_poop()).unwrap().0 > 800.0);
}

#[test]
fn no_pooping_while_busy_or_away_from_the_keyboard() {
    let w = world(vec![]);
    let busy: Vec<(&str, Box<dyn Fn(&mut Pet)>)> = vec![
        ("focus", Box::new(|p| p.set_focus(true))),
        ("user away", Box::new(|p| p.set_user_away(true))),
        ("housed", Box::new(|p| p.housed = true)),
        ("eating", Box::new(|p| p.state = State::Eating { remaining: 9.0, favorite: false, level_up: false })),
        ("food trip", Box::new(|p| p.food = Some(FoodTrip { food: Food::Meal, heart: false, level_up: false, target: None, from_house: false }))),
        ("in the air", Box::new(|p| { p.standing = None; p.state = State::Air { anim: Anim::Fall } })),
        ("pressed", Box::new(|p| p.pointer_down(800.0, 1000.0))),
        ("dragged", Box::new(|p| p.state = State::Dragged)),
        ("leaving", Box::new(|p| p.state = State::Exit { remaining: 9.0, side: Side::Left })),
        ("farewell", Box::new(|p| p.state = State::Farewell { remaining: 9.0 })),
        ("away at a friend", Box::new(|p| p.state = State::Away)),
        ("asleep", Box::new(|p| p.state = State::Sleep { remaining: 9.0 })),
    ];
    for (name, make_busy) in busy {
        let mut p = walker(&w, 800.0, 5);
        make_busy(&mut p);
        assert!(!clicks(&mut p, 3000), "{name}");
    }
    let mut p = walker(&w, 800.0, 5);
    p.state = idle();
    assert!(clicks(&mut p, 3000), "resting pets may");
}

#[test]
fn fliers_drop_it_from_the_air_but_not_while_thrown_or_leaving() {
    let flier = PetConfig { flies: true, ..cfg() };
    let mut p = Pet::new(flier, 800.0, 500.0, 2);
    p.state = idle();
    assert_eq!(p.standing, None);
    let at = (0..3000).find_map(|_| p.try_poop()).expect("hovering pets may");
    assert!(at.1 < 500.0 && at.1 > 400.0, "{at:?}");
    let mut p = Pet::new(flier, 800.0, 500.0, 2);
    p.state = State::Exit { remaining: 9.0, side: Side::Left };
    assert!(!clicks(&mut p, 3000));
}

/// 코드 수준 약속: 누른 자리를 받거나 저장하지 않음 (시그니처가 곧 증거)
#[test]
fn trying_takes_nothing_but_the_pet_itself() {
    let _: fn(&mut Pet) -> Option<(f64, f64)> = Pet::try_poop;
}

/// 걷는 동물을 똥 앞까지 몰아 봄: 무슨 일이 일어났나
#[derive(Debug, PartialEq)]
enum Seen {
    Turned,
    Hopped,
    Slipped,
    Nothing,
}

fn encounter(seed: u64) -> (Seen, Option<Speech>) {
    let w = world(vec![]);
    let spots = [poop_at(1, 900.0)];
    let mut p = walker(&w, 800.0, seed);
    let mut said = None;
    for _ in 0..(60 * 8) {
        p.notice_poops(&spots, &w);
        said = said.or(p.take_speech());
        match p.state {
            State::Turn { .. } => return (Seen::Turned, said),
            State::Air { .. } => return (Seen::Hopped, said),
            State::Slip { .. } => return (Seen::Slipped, said),
            _ => {}
        }
        p.update(DT, &w);
    }
    (Seen::Nothing, said)
}

#[test]
fn most_pets_say_ew_and_avoid_it_some_slip_and_fall() {
    let (mut turned, mut hopped, mut slipped) = (0, 0, 0);
    let n = 600;
    for seed in 0..n {
        let (seen, said) = encounter(seed);
        match seen {
            Seen::Turned => {
                turned += 1;
                assert_eq!(said, Some(Speech::Ew), "{seed}");
            }
            Seen::Hopped => {
                hopped += 1;
                assert_eq!(said, Some(Speech::Ew), "{seed}");
            }
            Seen::Slipped => {
                slipped += 1;
                assert_eq!(said, Some(Speech::Oof), "{seed}");
            }
            Seen::Nothing => panic!("{seed}: walked for 8 s and met nothing"),
        }
    }
    let share = |k: i32| k as f64 / n as f64;
    assert!((share(slipped) - poop::SLIP_CHANCE).abs() < 0.06, "slipped {slipped}/{n}");
    assert!(share(turned) > 0.25 && share(hopped) > 0.25, "turned {turned}, hopped {hopped}");
}

#[test]
fn a_hop_over_really_lands_past_the_poop_and_a_turn_walks_away() {
    let w = world(vec![]);
    let spots = [poop_at(1, 900.0)];
    let (mut hops, mut turns) = (0, 0);
    for seed in 0..80 {
        let mut p = walker(&w, 800.0, seed);
        p.poop.calm_until = 0.0;
        let mut limit = LoopLimit::new();
        loop {
            limit.step();
            p.notice_poops(&spots, &w);
            if !matches!(p.state, State::Move { .. }) {
                break;
            }
            p.update(DT, &w);
        }
        match p.state {
            State::Air { .. } => {
                hops += 1;
                for _ in 0..120 {
                    p.update(DT, &w);
                    if p.standing.is_some() {
                        break;
                    }
                }
                assert!(p.standing.is_some());
                assert!(p.body.x > 900.0 + p.cfg.half_w * 0.9, "{seed}: landed at {}", p.body.x);
                assert_eq!(p.body.y, GROUND);
            }
            State::Turn { .. } => {
                turns += 1;
                let turned_at = p.body.x;
                for _ in 0..40 {
                    p.update(DT, &w);
                }
                assert_eq!(p.facing, -1.0, "{seed}: {:?}", p.state);
                // 돌아서 걸어간다 (똥 쪽으로 다시 가지 않는다)
                for _ in 0..60 {
                    p.update(DT, &w);
                }
                assert!(p.body.x < turned_at, "{seed}: walks away {} from {turned_at} {:?}", p.body.x, p.state);
            }
            State::Slip { .. } => {}
            other => panic!("{other:?}"),
        }
    }
    assert!(hops > 10 && turns > 10, "{hops} {turns}");
}

#[test]
fn no_room_to_land_past_it_means_turning_back() {
    // 창 윗면 500..1100, 똥이 끝 바로 앞
    let window = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    let w = world(vec![window]);
    for seed in 0..80 {
        let mut p = Pet::new(sim_cfg(), 1000.0, 400.0, seed);
        place(&mut p, &w, Owner::Window(1), 1000.0, walking());
        p.facing = 1.0;
        let spots = [PoopSpot { id: 1, x: 1060.0, y: 400.0, owner: Owner::Window(1) }];
        p.notice_poops(&spots, &w);
        assert!(!matches!(p.state, State::Air { .. }), "{seed}");
    }
}

#[test]
fn slipping_falls_over_recovers_and_stays_on_the_surface() {
    let w = world(vec![]);
    let spots = [poop_at(1, 900.0)];
    let mut slips = 0;
    for seed in 0..120 {
        let mut p = walker(&w, 800.0, seed);
        for _ in 0..(60 * 8) {
            p.notice_poops(&spots, &w);
            if matches!(p.state, State::Slip { .. }) {
                break;
            }
            p.update(DT, &w);
        }
        if !matches!(p.state, State::Slip { .. }) {
            continue;
        }
        slips += 1;
        let at = p.body.x;
        assert!((at - 900.0).abs() < p.cfg.size * 0.3, "it stepped on it: {at}");
        let mut anims = Vec::new();
        for _ in 0..(60 * 3) {
            p.update(DT, &w);
            let a = p.render().anim;
            if anims.last() != Some(&a) && matches!(p.state, State::Slip { .. }) {
                anims.push(a);
            }
            assert_eq!(p.body.y, GROUND);
            assert!(p.standing.is_some());
        }
        assert_eq!(anims, vec![Anim::Fall, Anim::Land, Anim::Annoyed], "{seed}");
        assert!(p.body.x > at, "it skids forward");
        assert!(!matches!(p.state, State::Slip { .. }), "it got up");
    }
    assert!(slips > 5, "{slips}");
}

#[test]
fn slipping_stops_at_the_end_of_the_window() {
    let window = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    let w = world(vec![window]);
    let mut p = Pet::new(sim_cfg(), 1050.0, 400.0, 1);
    place(&mut p, &w, Owner::Window(1), 1050.0, State::Slip { remaining: 1.7, dir: 1.0 });
    for _ in 0..120 {
        p.update(DT, &w);
        assert!(p.standing.is_some(), "does not skid off the edge");
    }
}

#[test]
fn one_decision_per_encounter_and_no_loop_on_the_same_poop() {
    let w = world(vec![]);
    let spots = [poop_at(1, 900.0)];
    for seed in 0..60 {
        let mut p = walker(&w, 860.0, seed);
        p.notice_poops(&spots, &w);
        let first = p.poop.seen.len();
        assert_eq!(first, 1);
        let state = p.state;
        // 다시 불러도 다시 정하지 않는다 (밟으러 가는 중이거나 이미 피하는 중)
        for _ in 0..30 {
            p.notice_poops(&spots, &w);
        }
        assert_eq!(p.poop.seen.len(), 1);
        if matches!(state, State::Turn { .. }) {
            assert_eq!(p.state, state, "the same decision still stands");
        }
        // 쿨다운이 지나기 전엔 다시 걸어와도 그냥 지나간다
        let mut again = walker(&w, 860.0, seed);
        again.poop.seen.push((1, again.clock + poop::SAME_POOP_SECS));
        for _ in 0..(60 * 3) {
            again.notice_poops(&spots, &w);
            assert_eq!(again.take_speech(), None);
            again.update(DT, &w);
            if !matches!(again.state, State::Move { .. }) {
                break;
            }
        }
    }
}

#[test]
fn a_clump_of_poops_does_not_make_it_ew_over_and_over() {
    let w = world(vec![]);
    let spots = [poop_at(1, 880.0), poop_at(2, 900.0), poop_at(3, 920.0)];
    let mut p = walker(&w, 860.0, 4);
    p.notice_poops(&spots, &w);
    assert_eq!(p.poop.seen.iter().map(|s| s.0).collect::<Vec<_>>(), vec![1], "the nearest one");
    assert!(p.clock < p.poop.calm_until);
}

#[test]
fn it_only_cares_about_poop_ahead_nearby_and_on_its_own_surface() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) }]);
    let far = 800.0 + poop::LOOK_AHEAD * sim_cfg().size + 5.0;
    for spot in [
        poop_at(1, 760.0),                                                         // 뒤
        poop_at(2, far),                                                           // 멀다
        PoopSpot { id: 3, x: 850.0, y: 400.0, owner: Owner::Window(1) },           // 다른 디딤판
        PoopSpot { id: 4, x: 850.0, y: GROUND - 50.0, owner: FLOOR },              // 다른 높이
    ] {
        let mut p = walker(&w, 800.0, 7);
        let before = p.state;
        p.notice_poops(&[spot], &w);
        assert_eq!(p.state, before, "{spot:?}");
        assert!(p.poop.seen.is_empty());
    }
}

#[test]
fn it_never_interferes_with_anything_else_the_pet_is_doing() {
    let w = world(vec![]);
    let spots = [poop_at(1, 850.0)];
    let cases: Vec<(&str, Box<dyn Fn(&mut Pet)>)> = vec![
        ("control", Box::new(|p| p.set_control(true))),
        ("held", Box::new(|p| p.state = State::Dragged)),
        ("food trip", Box::new(|p| p.food = Some(FoodTrip { food: Food::Meal, heart: false, level_up: false, target: None, from_house: false }))),
        ("to food", Box::new(|p| p.state = State::ToFood { remaining: 9.0 })),
        ("home trip", Box::new(|p| p.home_trip = Some(HomeTrip::new(FLOOR, 1500.0, 800.0)))),
        ("going home", Box::new(|p| p.state = State::GoHome)),
        ("farewell", Box::new(|p| p.farewell_pending = true)),
        ("sleep", Box::new(|p| p.state = State::Sleep { remaining: 9.0 })),
        ("focus", Box::new(|p| p.set_focus(true))),
        ("user away", Box::new(|p| p.set_user_away(true))),
        ("homesick", Box::new(|p| { p.set_guest(Side::Right, 0.0); p.tick_guest(1.0); })),
        ("exit", Box::new(|p| p.state = State::Exit { remaining: 9.0, side: Side::Right })),
        ("rolling", Box::new(|p| p.state = State::Move { kind: Move::Roll, remaining: 9.0 })),
        ("chasing", Box::new(|p| p.state = State::Chase { remaining: 9.0 })),
    ];
    for (name, setup) in cases {
        for seed in 0..12 {
            let mut p = walker(&w, 800.0, seed);
            setup(&mut p);
            let before = p.state;
            p.notice_poops(&spots, &w);
            assert_eq!(p.state, before, "{name}");
            assert!(p.poop.seen.is_empty(), "{name}");
            assert_eq!(p.take_speech(), None, "{name}");
        }
    }
}

#[test]
fn a_pet_going_to_slip_forgets_it_when_something_else_comes_up() {
    let w = world(vec![]);
    let spots = [poop_at(1, 880.0)];
    let mut p = walker(&w, 800.0, 1);
    p.poop.stepping = Some(1);
    p.state = State::Sleep { remaining: 9.0 };
    p.notice_poops(&spots, &w);
    assert_eq!(p.poop.stepping, None);
    // 똥이 치워졌다
    let mut p = walker(&w, 800.0, 1);
    p.poop.stepping = Some(1);
    p.notice_poops(&[], &w);
    assert_eq!(p.poop.stepping, None);
    assert!(matches!(p.state, State::Move { .. }));
}

#[test]
fn fliers_ignore_it() {
    let w = world(vec![]);
    let spots = [poop_at(1, 850.0)];
    let mut p = Pet::new(PetConfig { flies: true, ..cfg() }, 800.0, GROUND, 1);
    place(&mut p, &w, FLOOR, 800.0, walking());
    p.notice_poops(&spots, &w);
    assert!(matches!(p.state, State::Move { .. }));
    assert!(p.poop.seen.is_empty());
}

#[test]
fn being_picked_up_or_called_home_ends_a_slip() {
    let w = world(vec![]);
    let mut p = walker(&w, 800.0, 1);
    p.state = State::Slip { remaining: 1.7, dir: 1.0 };
    p.pointer_down(800.0, 1000.0);
    assert!(p.is_held());
    let mut p = walker(&w, 800.0, 1);
    p.state = State::Slip { remaining: 1.7, dir: 1.0 };
    p.go_home(FLOOR, 1500.0);
    p.update(DT, &w);
    assert!(matches!(p.state, State::GoHome), "{:?}", p.state);
    // 잠들어야 하면 그대로 잔다
    let mut p = walker(&w, 800.0, 1);
    p.state = State::Slip { remaining: 1.7, dir: 1.0 };
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
}
