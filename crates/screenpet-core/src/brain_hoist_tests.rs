//! 🙆 다른 동물 번쩍 들기 (흐물이 v0.6.3)

use super::hoist_moves::{notice_pets, sync_lifts, CarriedOut, HoistPhase, LIFT_CHANCE};
use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::test_util::{flyer, pet_cfg, DT};

/// 앱처럼 한 틱: 보여 주고, 움직이고, 맞춘다
fn step(pets: &mut [Pet], w: &World) -> Vec<CarriedOut> {
    notice_pets(pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets)
}

fn pair(target: PetConfig, seed: u64, w: &World) -> Vec<Pet> {
    let mut h = resting(heumul(), 700.0, seed, w);
    h.hoist.ready_at = 0.0;
    h.facing = 1.0;
    let mut t = resting(target, 900.0, seed + 1000, w);
    t.state = State::Idle { remaining: 1000.0, look: false };
    vec![h, t]
}

fn start(pets: &mut [Pet], w: &World) -> bool {
    notice_pets(pets);
    let spot = pets[1].pet_spot();
    pets[0].start_hoist(&spot, w)
}

fn hoisting(p: &Pet) -> bool {
    matches!(p.state, State::Hoist { .. })
}

#[test]
fn it_walks_over_lifts_the_pet_overhead_carries_it_a_bit_and_puts_it_down_gently() {
    let w = world(vec![]);
    let mut pets = pair(pet_cfg(), 1, &w);
    assert!(start(&mut pets, &w));
    let (mut lifted, mut pleaded, mut wiggled) = (false, false, false);
    for _ in 0..(60 * 12) {
        step(&mut pets, &w);
        if pets[1].is_lifted() {
            lifted = true;
            let at = pets[0].hoist_point().unwrap();
            assert!((pets[1].body.x - at.0).abs() < 1e-6 && (pets[1].body.y - at.1).abs() < 1e-6, "손 위에");
            assert!(pets[1].body.y < 1040.0 - 0.6 * 128.0, "머리 위로 번쩍");
            assert_eq!(pets[1].render().anim, Anim::Dragged);
            wiggled |= pets[1].render().wobble;
        }
        pleaded |= pets[1].take_speech() == Some(Speech::PutMeDown);
        if lifted && !pets[1].is_lifted() && !hoisting(&pets[0]) {
            break;
        }
    }
    assert!(lifted && pleaded && wiggled);
    // 던지지 않고 사뿐 — 내려서서 선다
    assert!(pets[1].gentle_drop || pets[1].standing.is_some());
    for _ in 0..120 {
        step(&mut pets, &w);
    }
    assert!(pets[1].standing.is_some() && !pets[1].thrown, "{:?}", pets[1].state);
    assert!((pets[1].body.x - pets[0].body.x).abs() < 2.0 * 128.0, "가까이 내려놓는다");
}

#[test]
fn it_notices_pets_on_its_floor_now_and_then() {
    let w = world(vec![]);
    let mut went = 0;
    let n = 400;
    for seed in 0..n {
        let mut pets = pair(pet_cfg(), seed, &w);
        notice_pets(&mut pets);
        pets[0].update(DT, &w);
        went += usize::from(hoisting(&pets[0]));
    }
    let share = went as f64 / n as f64;
    assert!((share - LIFT_CHANCE).abs() < 0.07, "{share}");
}

#[test]
fn never_lifts_held_housed_eating_airborne_flying_or_carrying_pets() {
    let w = world(vec![]);
    for case in 0..6 {
        let target = if case == 4 { flyer() } else { pet_cfg() };
        let mut pets = pair(target, 50 + case, &w);
        match case {
            0 => pets[1].pointer_down(900.0, 1000.0),
            1 => pets[1].enter_house(),
            2 => pets[1].state = State::Eating { remaining: 5.0, favorite: false, level_up: false },
            3 | 4 => {
                pets[1].standing = None;
                pets[1].body.y = 500.0;
                pets[1].state = State::Air { anim: Anim::Fall };
            }
            _ => {
                pets[1] = resting(heumul(), 900.0, 77, &w);
                pets[1].juggle.carry = Some(3);
                pets[1].state = State::Juggle { phase: super::juggle_moves::JugglePhase::Hold { secs: 100.0 }, t: 0.0 };
            }
        }
        if case == 4 {
            pets[1].state = State::Move { kind: Move::Walk, remaining: 10.0 };
        }
        assert!(!pets[1].lift_ok(), "case {case}");
        start(&mut pets, &w);
        for _ in 0..(60 * 6) {
            step(&mut pets, &w);
            assert!(!pets[1].is_lifted(), "case {case}");
        }
    }
}

#[test]
fn a_pet_twice_its_size_is_too_heavy_strain_then_slump() {
    let w = world(vec![]);
    let big = PetConfig { size: 300.0, half_w: 90.0, ..pet_cfg() };
    let mut pets = pair(big, 3, &w);
    pets[1].body.x = 1000.0;
    if let Some(st) = pets[1].standing.as_mut() {
        st.rel_x = 1000.0;
    }
    assert!(start(&mut pets, &w));
    let mut phases = Vec::new();
    for _ in 0..(60 * 10) {
        step(&mut pets, &w);
        assert!(!pets[1].is_lifted());
        if let State::Hoist { phase, .. } = pets[0].state {
            let name = match phase {
                HoistPhase::Approach { .. } => "approach",
                HoistPhase::Strain { .. } => "strain",
                HoistPhase::Slump => "slump",
                _ => "other",
            };
            if phases.last() != Some(&name) {
                phases.push(name);
            }
        }
    }
    assert_eq!(phases, ["approach", "strain", "slump"]);
}

#[test]
fn a_visiting_heumul_carries_the_hosts_own_pet_off_through_a_friend_edge_and_both_leave_together() {
    let w = world(vec![]);
    let mut carried = None;
    for seed in 0..20 {
        let mut pets = pair(pet_cfg(), 100 + seed, &w);
        pets[0].set_guest(Side::Left, 1e9);
        pets[0].set_exit_edges(false, true);
        pets[0].set_lift_edges(false, true);
        assert!(start(&mut pets, &w));
        for _ in 0..(60 * 40) {
            let out = step(&mut pets, &w);
            if pets[0].hoisting().is_some() && matches!(pets[0].state, State::Exit { .. }) {
                assert_eq!(pets[0].render().anim, Anim::CarryRun, "들고 달려 나간다");
            }
            if let Some(&(i, j, side)) = out.first() {
                carried = Some((i, j, side));
                assert_eq!(pets[0].take_exit(), Some(Side::Right));
                assert_eq!(pets[1].take_exit(), Some(Side::Right), "같은 끝으로 함께");
                assert!(pets[1].is_away());
                break;
            }
            if !hoisting(&pets[0]) && !pets[1].is_lifted() && !matches!(pets[0].state, State::Exit { .. }) && pets[0].clock > 1010.0 {
                break;
            }
        }
        if carried.is_some() {
            break;
        }
    }
    assert_eq!(carried, Some((0, 1, Side::Right)));
}

#[test]
fn guests_are_never_carried_away_only_put_down() {
    let w = world(vec![]);
    for seed in 0..15 {
        let mut pets = pair(pet_cfg(), 200 + seed, &w);
        pets[1].set_guest(Side::Left, 1e9);
        pets[0].set_exit_edges(false, true);
        pets[0].set_lift_edges(false, true);
        assert!(start(&mut pets, &w));
        for _ in 0..(60 * 20) {
            assert!(step(&mut pets, &w).is_empty());
            assert!(!matches!(pets[0].state, State::Hoist { phase: HoistPhase::CarryOff { .. }, .. }));
        }
    }
}

#[test]
fn grabbing_the_lifted_pet_makes_heumul_drop_it_with_a_hmph() {
    let w = world(vec![]);
    let mut pets = pair(pet_cfg(), 4, &w);
    assert!(start(&mut pets, &w));
    while !pets[1].is_lifted() {
        step(&mut pets, &w);
    }
    let (x, y) = (pets[1].body.x, pets[1].body.y - 40.0);
    pets[1].pointer_down(x, y);
    pets[0].take_speech();
    step(&mut pets, &w);
    assert!(!hoisting(&pets[0]) || pets[0].hoisting().is_none());
    assert_eq!(pets[0].take_speech(), Some(Speech::Hmph));
}
