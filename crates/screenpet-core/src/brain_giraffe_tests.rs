//! 기린 🦒 — 구르지 않고, 구를 차례에 멈춰 서서 목을 쭉 늘린다 (Special::NeckStretch, pet.json에 neckstretch 동작).
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{place, sim_cfg, w1, win, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn giraffe() -> PetConfig {
    PetConfig { special: Some(Special::NeckStretch), ..sim_cfg() }
}

/// 기린 10분 × 4 — 한 번도 구르지 않고, 목 늘이기는 여러 번 나온다
#[test]
fn giraffe_never_rolls_but_stretches_its_neck_instead() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0), win(2, 1300.0, 350.0, 1800.0, 700.0)]);
    let mut stretches = 0usize;
    for seed in 0..4 {
        let mut p = Pet::new(giraffe(), 960.0, 128.0, seed);
        let mut was = false;
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Move { kind: Move::Roll, .. }), "seed {seed}: rolling {:?}", p.state);
            let anim = p.render().anim;
            assert_ne!(anim, Anim::Roll, "seed {seed}");
            let now = anim == Anim::NeckStretch;
            if now && !was {
                stretches += 1;
            }
            was = now;
        }
    }
    assert!(stretches >= 8, "only {stretches} stretches in 40 minutes");
}

/// 다른 동물은 그대로 구른다 (같은 시뮬레이션에서)
#[test]
fn other_pets_still_roll_and_never_neck_stretch() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0)]);
    let mut rolled = false;
    for seed in 0..3 {
        let mut p = Pet::new(sim_cfg(), 960.0, 128.0, seed);
        for _ in 0..(60 * 300) {
            p.update(DT, &w);
            rolled |= p.render().anim == Anim::Roll;
            assert_ne!(p.render().anim, Anim::NeckStretch);
        }
    }
    assert!(rolled);
}

/// 목 늘이기: 제자리에서 정해진 시간(앱 애니메이션과 같은 길이)만큼, 옆모습(방향 따라 반전), 말풍선 없음
#[test]
fn neck_stretch_stands_still_for_its_fixed_time() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(giraffe(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
    p.start_special(Special::NeckStretch);
    assert_eq!(p.take_speech(), None);
    let State::Special { remaining, .. } = p.state else { panic!("{:?}", p.state) };
    assert_eq!(remaining, behavior::NECK_STRETCH_SECS);
    p.facing = -1.0;
    assert_eq!(p.render().anim, Anim::NeckStretch);
    assert!(p.render().flip);
    let x = p.body.x;
    let ticks = (behavior::NECK_STRETCH_SECS / DT) as usize - 2;
    for _ in 0..ticks {
        p.update(DT, &w);
        assert_eq!(p.render().anim, Anim::NeckStretch);
        assert_eq!(p.body.x, x);
    }
    for _ in 0..4 {
        p.update(DT, &w);
    }
    assert_ne!(p.render().anim, Anim::NeckStretch);
}

/// 쉬다가·간질일 때는 목을 늘이지 않는다 (구르기 자리에서만)
#[test]
fn neck_stretch_only_replaces_rolling() {
    assert_eq!((Special::NeckStretch.rest_chance(), Special::NeckStretch.tickle_chance()), (0.0, 0.0));
    let w = world(vec![w1()]);
    for seed in 0..200 {
        let mut p = Pet::new(giraffe(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        for i in 0..10 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, &w);
        }
        assert!(!matches!(p.state, State::Special { .. }), "seed {seed}");
    }
}

/// 목 늘이기가 끝나면 바로 또 늘이지 않고 걷는다
#[test]
fn no_back_to_back_stretches() {
    let w = world(vec![w1()]);
    for seed in 0..300 {
        let mut p = Pet::new(giraffe(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Special { kind: Special::NeckStretch, remaining: DT / 2.0 });
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Special { .. }), "seed {seed}: {:?}", p.state);
    }
}
