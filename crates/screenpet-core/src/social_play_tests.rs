//! 🙌 하이파이브 · 🦖 큰 쿵 (v0.6.3 — social.rs + social_play.rs + brain_highfive·quake.rs)

use super::*;
use crate::behavior::Move;
use crate::brain::{Anim, Pet, PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::play::{self, ObjectPlay};
use crate::test_util::DT;
use crate::world::World;

const GROUND: f64 = 1040.0;

fn cfg() -> PetConfig {
    PetConfig { half_w: 38.4, ..crate::test_util::pet_cfg() }
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

fn at(w: &World, c: PetConfig, x: f64, facing: f64, state: State, seed: u64) -> Pet {
    let owner = Owner::Ground(0);
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = state;
    p.facing = facing;
    p
}

fn walking() -> State {
    State::Move { kind: Move::Walk, remaining: 60.0 }
}

fn resting() -> State {
    State::Idle { remaining: 100.0, look: false }
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
}

fn quiet_social(seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 0.0;
    s.one_way_chance = 0.0;
    s.set_high_five_chance(0.0);
    s
}

// ---- 🙌 하이파이브 ----

fn head_on(w: &World, seed: u64) -> Vec<Pet> {
    let mut pets = vec![at(w, cfg(), 800.0, 1.0, walking(), seed), at(w, cfg(), 800.0 + 1.6 * 128.0, -1.0, walking(), seed + 1000)];
    for p in &mut pets {
        p.update(DT, w);
    }
    pets
}

#[test]
fn two_pets_meeting_face_to_face_stop_face_each_other_jump_together_and_clap() {
    let w = floor();
    for seed in 0..6 {
        let mut pets = head_on(&w, seed);
        let mut s = Social::new(seed);
        s.set_high_five_chance(1.0);
        let mut jumped = [None, None];
        let mut claps = [0usize; 2];
        let mut said = 0;
        for k in 0..(60 * 5) {
            step(&mut s, &mut pets, &w);
            for (i, p) in pets.iter_mut().enumerate() {
                if p.render().anim == Anim::Hop && jumped[i].is_none() {
                    jumped[i] = Some(k);
                }
                claps[i] += usize::from(p.render().clap);
                said += usize::from(p.take_speech() == Some(Speech::HighFive));
            }
        }
        assert!(jumped[0].is_some() && jumped[0] == jumped[1], "같이 뛴다: {jumped:?}");
        assert!(claps[0] > 0 && claps[0] == claps[1]);
        assert_eq!(said, 1, "한 마리가 \"High five!\"");
        let gap = pets[1].body.x - pets[0].body.x;
        assert!(gap > 0.5 * 128.0 && gap < 0.75 * 128.0, "손이 닿는 거리: {gap}");
    }
}

#[test]
fn the_jump_lifts_the_sprite_but_keeps_feet_on_the_floor() {
    let w = floor();
    let mut pets = head_on(&w, 2);
    let mut s = Social::new(2);
    s.set_high_five_chance(1.0);
    let mut top = f64::INFINITY;
    for _ in 0..(60 * 4) {
        step(&mut s, &mut pets, &w);
        top = top.min(pets[0].window_pos().1);
        assert!(pets[0].standing.is_some());
        assert_eq!(pets[0].body.y, GROUND);
    }
    assert!(GROUND - 128.0 - top > 0.2 * 128.0, "{top}");
}

#[test]
fn high_fives_happen_about_a_quarter_of_meetings_and_not_again_for_minutes() {
    let w = floor();
    let n = 300;
    let mut fives = 0;
    for seed in 0..n {
        let mut pets = head_on(&w, seed);
        let mut s = Social::new(seed);
        s.meet_chance = 0.0;
        for _ in 0..10 {
            step(&mut s, &mut pets, &w);
        }
        fives += usize::from(pets.iter().any(Pet::in_high_five));
    }
    let share = fives as f64 / n as f64;
    assert!((share - play::HIGH_FIVE_CHANCE).abs() < 0.07, "{share}");
    // 한 번 한 짝은 다시 만나도 쉬는 동안엔 안 한다
    let mut pets = head_on(&w, 9);
    let mut s = Social::new(9);
    s.set_high_five_chance(1.0);
    for _ in 0..(60 * 4) {
        step(&mut s, &mut pets, &w);
    }
    for p in &mut pets {
        p.state = walking();
    }
    pets[0].facing = 1.0;
    pets[1].facing = -1.0;
    let (a, b) = (800.0, 800.0 + 1.6 * 128.0);
    for (p, x) in pets.iter_mut().zip([a, b]) {
        let d = x - p.body.x;
        p.body.x = x;
        if let Some(st) = p.standing.as_mut() {
            st.rel_x += d;
        }
    }
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
        assert!(!pets.iter().any(Pet::in_high_five));
    }
}

#[test]
fn a_partner_grabbed_before_the_jump_lets_the_other_go() {
    let w = floor();
    let mut pets = head_on(&w, 4);
    let mut s = Social::new(4);
    s.set_high_five_chance(1.0);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_high_five() && pets[1].in_high_five());
    let p = &mut pets[1];
    p.pointer_down(p.body.x, p.body.y - 60.0);
    step(&mut s, &mut pets, &w);
    step(&mut s, &mut pets, &w);
    assert!(!pets[0].in_high_five());
}

// ---- 🦖 큰 쿵 ----

fn dino() -> PetConfig {
    PetConfig { special: Some(crate::behavior::Special::Stomp), ..cfg() }
}

#[test]
fn a_big_stomp_pops_things_and_makes_friends_within_six_bodies_hop_and_sleepers_grumble() {
    let w = floor();
    let mut pets = vec![
        at(&w, dino(), 600.0, 1.0, resting(), 1),
        // 5.5몸: 예전 쿵쾅(5몸)보다 멀다
        at(&w, cfg(), 600.0 + 5.5 * 128.0, 1.0, walking(), 2),
        at(&w, cfg(), 600.0 - 2.0 * 128.0, 1.0, State::Sleep { remaining: 100.0 }, 3),
        at(&w, cfg(), 600.0 + 7.0 * 128.0, 1.0, resting(), 4),
    ];
    pets[0].set_quake_ready(true);
    pets[0].force_stomp();
    pets[0].begin_stomp_now();
    assert!(pets[0].stomp_is_big());
    assert_eq!(pets[0].take_speech(), Some(Speech::BigStomp));
    match pets[0].take_object_play() {
        Some(ObjectPlay::Quake { x, y, owner, reach }) => {
            assert_eq!((x, y, owner), (600.0, GROUND, Owner::Ground(0)));
            assert!((reach - play::QUAKE_BODIES * 128.0).abs() < 1e-9);
        }
        e => panic!("{e:?}"),
    }
    let mut s = quiet_social(1);
    s.tick(DT, &mut pets);
    assert!(matches!(pets[1].state, State::Bounce { .. }), "{:?}", pets[1].state);
    assert_eq!(pets[1].take_speech(), Some(Speech::Whoa));
    assert!(matches!(pets[2].state, State::Annoyed { .. }), "자던 친구는 투덜");
    assert!(matches!(pets[3].state, State::Idle { .. }), "너무 멀다");
    let mut top = f64::INFINITY;
    for _ in 0..60 {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        top = top.min(pets[1].window_pos().1);
    }
    assert!(GROUND - 128.0 - top > 0.12 * 128.0);
    assert!(!matches!(pets[1].state, State::Bounce { .. }));
}

#[test]
fn between_big_stomps_it_is_the_old_wobble_and_never_while_the_user_is_away() {
    let w = floor();
    let mut d = at(&w, dino(), 600.0, 1.0, resting(), 1);
    d.set_quake_ready(false);
    d.force_stomp();
    d.begin_stomp_now();
    assert!(!d.stomp_is_big() && d.take_object_play().is_none());
    let mut a = at(&w, dino(), 600.0, 1.0, resting(), 2);
    a.set_quake_ready(true);
    a.set_user_away(true);
    a.force_stomp();
    a.begin_stomp_now();
    assert!(!a.stomp_is_big() && a.take_object_play().is_none());
    // 큰 쿵 한 번 뒤엔 몇 분 쉰다
    let mut b = at(&w, dino(), 600.0, 1.0, resting(), 3);
    b.set_quake_ready(true);
    b.force_stomp();
    b.begin_stomp_now();
    assert!(b.stomp_is_big());
    b.force_stomp();
    b.begin_stomp_now();
    assert!(!b.stomp_is_big());
}
