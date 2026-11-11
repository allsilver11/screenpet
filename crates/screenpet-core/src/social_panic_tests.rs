//! 겁쟁이 흐물이(pet.json timid)가 다른 동물과 마주치면: 반쯤은 깜짝 놀라 등을 돌려 만세 줄행랑 (State::Panic),
//! 나머지는 평소처럼 인사·만남. 겁 주사위는 따로 굴려 다른 동물들의 인사·놀이·만남 주사위는 그대로다.

use super::*;
use crate::brain::{PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::physics::Body;
use crate::test_util::{DT, idle, pet_cfg};
use crate::world::World;

fn world() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
}

fn timid() -> PetConfig {
    PetConfig { half_w: 38.4, timid: true, personality: Personality::Playful, ..pet_cfg() }
}

fn normal() -> PetConfig {
    PetConfig { half_w: 38.4, ..pet_cfg() }
}

fn at(w: &World, c: PetConfig, x: f64, seed: u64) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    let (ax, ay) = w.anchor(Owner::Ground(0)).unwrap();
    p.standing = Some(Standing { owner: Owner::Ground(0), rel_x: x - ax });
    p.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
    p.state = idle();
    p
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
}

fn panicked(p: &Pet) -> bool {
    matches!(p.state, State::Panic { .. })
}

/// 두 동물이 몸 하나 거리에 서 있을 때 첫 틱
fn meet(a: PetConfig, b: PetConfig, seed: u64) -> (Social, Vec<Pet>, World) {
    let w = world();
    let mut s = Social::new(seed);
    let mut pets = vec![at(&w, a, 800.0, seed), at(&w, b, 900.0, seed + 1000)];
    step(&mut s, &mut pets, &w);
    (s, pets, w)
}

#[test]
fn a_timid_pet_meeting_another_animal_panics_about_half_the_time() {
    let mut scared = 0;
    for seed in 0..300 {
        let (_, pets, _) = meet(timid(), normal(), seed);
        if panicked(&pets[0]) {
            scared += 1;
            assert_eq!(pets[0].facing, -1.0, "친구 반대쪽으로");
            assert!(!panicked(&pets[1]));
        } else {
            assert!(pets[0].social_role().is_some() || pets[0].is_meeting(), "평소처럼 인사·만남: {:?}", pets[0].state);
        }
    }
    assert!((110..=190).contains(&scared), "scared {scared}/300");
}

#[test]
fn one_roll_per_encounter_not_every_tick() {
    // 놀라지 않았으면 같이 서 있는 동안은 다시 놀라지 않는다 (인사가 끝나도)
    for seed in 0..60 {
        let (mut s, mut pets, w) = meet(timid(), normal(), seed);
        if panicked(&pets[0]) {
            continue;
        }
        for _ in 0..(60 * 6) {
            step(&mut s, &mut pets, &w);
            assert!(!panicked(&pets[0]), "seed {seed}: {:?}", pets[0].state);
        }
    }
}

#[test]
fn two_stick_people_never_scare_each_other() {
    for seed in 0..100 {
        let (_, pets, _) = meet(timid(), timid(), seed);
        assert!(!pets.iter().any(panicked), "seed {seed}");
    }
}

#[test]
fn the_scare_dice_leave_everyone_elses_dice_alone() {
    // 겁쟁이가 없는 짝: 겁 확인을 꺼도 켜도 코디네이터·동물의 주사위와 결과가 똑같다
    for seed in 0..40 {
        let run = |chance: f64| {
            let w = world();
            let mut s = Social::new(seed);
            s.scare_chance = chance;
            let mut pets = vec![at(&w, normal(), 800.0, seed), at(&w, normal(), 900.0, seed + 1000)];
            for _ in 0..(60 * 8) {
                step(&mut s, &mut pets, &w);
            }
            (s.rng.get_seed(), s.meet_rng.get_seed(), pets.iter().map(|p| (format!("{:?}", p.state), p.body.x.to_bits())).collect::<Vec<_>>())
        };
        assert_eq!(run(SCARE_CHANCE), run(0.0), "seed {seed}");
    }
}

#[test]
fn the_scared_pet_says_eek() {
    for seed in 0..80 {
        let (_, mut pets, _) = meet(timid(), normal(), seed);
        if panicked(&pets[0]) {
            assert_eq!(pets[0].take_speech(), Some(Speech::Eek));
            return;
        }
    }
    panic!("never scared");
}
