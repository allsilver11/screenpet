//! 공룡 쿵쾅이 같은 바닥의 친구들을 흔든다 (social.rs stomps + brain_stomp.rs) — social_tests의 자식 모듈 (도우미를 같이 쓴다)

use super::*;
use crate::behavior::Special;

fn dino() -> PetConfig {
    PetConfig { special: Some(Special::Stomp), ..cfg(Personality::Playful) }
}

fn tumbling(p: &Pet) -> bool {
    matches!(p.state, State::Tumble { .. })
}

/// 같은 바닥 몸 5개 안의 친구만 흔들린다 — 다른 디딤판·멀리 있는 친구는 그대로
#[test]
fn only_friends_on_the_same_surface_nearby_feel_the_stomp() {
    let w = world(vec![w1()]);
    let mut pets = vec![
        ground(&w, dino(), 300.0, 1),
        ground(&w, friendly(), 500.0, 2),
        ground(&w, friendly(), 300.0 + 4.5 * 128.0, 3),
        ground(&w, friendly(), 300.0 + 6.0 * 128.0, 4),
        pet_on(&w, friendly(), Owner::Window(1), 600.0, 5),
    ];
    pets[0].force_stomp();
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    let felt: Vec<bool> = pets.iter().map(|p| p.wobbling()).collect();
    assert_eq!(felt, vec![false, true, true, false, false]);
    // 한 번만 알린다
    assert!(!pets[0].take_stomp());
    for p in &mut pets[1..3] {
        assert_eq!(p.take_speech(), Some(Speech::Whoa));
    }
}

/// 걷는 친구는 35%쯤 데굴 넘어지고 (공룡 반대쪽으로), 나머지는 흔들리기만. 나는 친구는 늘 흔들리기만
#[test]
fn about_a_third_tumble_away_and_flyers_only_wobble() {
    let w = world(vec![]);
    let (mut tumbles, n) = (0usize, 1000usize);
    for seed in 0..n as u64 {
        let mut pets = vec![ground(&w, dino(), 800.0, seed), ground(&w, friendly(), 950.0, seed + 1), bee_at(700.0, 900.0, seed + 2)];
        pets[0].force_stomp();
        let mut s = Social::new(seed);
        s.tick(DT, &mut pets);
        assert!(pets[1].wobbling() && pets[2].wobbling());
        assert!(!tumbling(&pets[2]), "flyers never tumble");
        if let State::Tumble { dir, .. } = pets[1].state {
            assert_eq!(dir, 1.0, "away from the dino");
            tumbles += 1;
        }
    }
    let share = tumbles as f64 / n as f64;
    assert!((share - TUMBLE_CHANCE).abs() < 0.05, "{share}");
}

/// 자던 친구는 투덜 깨고, 들려 있거나 먹는 친구는 그대로, 한 번 흔들리면 10초 동안은 다시 흔들리지 않는다
#[test]
fn sleepers_wake_grumpy_busy_pets_are_skipped_and_there_is_a_cooldown() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, dino(), 800.0, 1), ground(&w, friendly(), 900.0, 2), ground(&w, friendly(), 700.0, 3),
        ground(&w, friendly(), 1000.0, 4)];
    pets[1].state = State::Sleep { remaining: 30.0 };
    pets[2].state = State::Dragged;
    pets[3].state = State::Eating { remaining: 3.0, favorite: false, level_up: false };
    pets[0].force_stomp();
    let mut s = Social::new(3);
    s.tick(DT, &mut pets);
    assert!(matches!(pets[1].state, State::Annoyed { .. }) && pets[1].take_speech() == Some(Speech::Ignore));
    assert_eq!(pets[2].state, State::Dragged);
    assert!(matches!(pets[3].state, State::Eating { .. }) && !pets[3].wobbling());
    // 다시 쿵쾅 — 방금 깬 친구는 그대로
    pets[1].state = resting();
    pets[0].force_stomp();
    s.tick(DT, &mut pets);
    assert_eq!(pets[1].state, resting());
}

/// 데굴 넘어진 친구는 이번 틱에 인사·놀이를 시작하지 않고, 놀던 짝은 풀린다
#[test]
fn a_tumble_interrupts_greeting() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2), ground(&w, dino(), 600.0, 3)];
    let mut s = Social::new(5);
    step(&mut s, &mut pets, &w);
    assert_eq!(s.active(), 1);
    // 데굴이 나올 때까지 (35%) 새 공룡으로 다시 쿵쾅 — 짝 하나라도 넘어지면 놀이가 끝난다
    let mut done = false;
    for seed in 0..50 {
        let mut s2 = Social::new(seed);
        let mut ps = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2), ground(&w, dino(), 600.0, 3)];
        step(&mut s2, &mut ps, &w);
        ps[2].force_stomp();
        step(&mut s2, &mut ps, &w);
        if tumbling(&ps[0]) || tumbling(&ps[1]) {
            step(&mut s2, &mut ps, &w);
            assert_eq!(s2.active(), 0);
            assert!(ps.iter().all(|p| p.social_role().is_none()));
            done = true;
            break;
        }
    }
    assert!(done);
}
