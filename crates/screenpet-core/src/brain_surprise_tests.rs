//! 레서판다 깜짝 놀라기 (Special::Surprise, pet.json surprise) — 클릭하거나 다른 동물·물건이 살짝 부딪히면
//! 뒷발로 벌떡 서서 두 팔을 번쩍 든다 ("!!"). 잠깐 그대로 있다가 평소대로 돌아간다.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, w1, world};
use super::*;
use crate::collide::{Face, Hit};
use crate::test_util::{DT, idle};

fn panda() -> PetConfig {
    PetConfig { special: Some(Special::Surprise), personality: Personality::Playful, ..cfg() }
}

fn standing(c: PetConfig, seed: u64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(c, 0.0, 0.0, seed);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    (p, w)
}

fn click(p: &mut Pet) {
    let (x, y) = (p.body.x, p.body.y - 0.5 * p.cfg.size);
    p.pointer_down(x, y);
    p.pointer_up(x, y);
}

fn bump(fast: bool, poop: bool) -> Hit {
    Hit { face: Face::Side, vx: 600.0, vy: 0.0, dir: 1.0, fast, poop }
}

fn surprised(p: &Pet) -> bool {
    matches!(p.state, State::Special { kind: Special::Surprise, .. })
}

/// 깜짝 자세: 앞모습 (뒤집지 않는다), 1초 남짓 버티고 끝나면 평소대로
fn holds_then_ends(p: &mut Pet, w: &World) {
    p.facing = -1.0;
    assert_eq!(p.render().anim, Anim::Surprise);
    assert!(p.render().anim.is_front() && !p.render().flip);
    let mut held = 0.0;
    for _ in 0..(60 * 4) {
        p.update(DT, w);
        if !surprised(p) {
            break;
        }
        held += DT;
    }
    assert!(!surprised(p), "the surprise pose should end: {:?}", p.state);
    assert!((0.9..=2.0).contains(&held), "held {held:.2}s");
    assert!(p.standing.is_some());
}

#[test]
fn the_surprise_pose_is_an_optional_front_animation_named_surprise() {
    assert_eq!(Special::from_manifest(|n| n == "surprise"), Some(Special::Surprise));
    assert_eq!(Special::Surprise.anim_name(), "surprise");
    assert_eq!(Anim::from(Special::Surprise), Anim::Surprise);
    assert_eq!(serde_json::to_string(&Anim::Surprise).unwrap(), "\"surprise\"");
    assert!(crate::manifest::OPTIONAL_ANIMS.contains(&"surprise"));
    // 쉬거나 간질일 때 저절로 하지는 않는다 — 놀랄 일이 있어야 한다
    assert_eq!((Special::Surprise.rest_chance(), Special::Surprise.tickle_chance()), (0.0, 0.0));
    assert!(!Special::Surprise.replaces_roll() && !Special::Surprise.is_dino());
}

#[test]
fn clicking_the_red_panda_often_startles_it_into_the_surprise_pose() {
    let mut startled = 0;
    for seed in 0..100 {
        let (mut p, w) = standing(panda(), seed);
        click(&mut p);
        if surprised(&p) {
            startled += 1;
            assert_eq!(p.take_speech(), Some(Speech::Surprise));
            holds_then_ends(&mut p, &w);
        } else {
            assert!(matches!(p.state, State::Air { .. } | State::Trick { .. }), "{:?}", p.state);
        }
    }
    assert!((40..=80).contains(&startled), "startled {startled}/100");
}

#[test]
fn other_pets_never_strike_the_surprise_pose() {
    for seed in 0..60 {
        let (mut p, w) = standing(cfg(), seed);
        click(&mut p);
        assert!(!surprised(&p));
        p.feel_hit(bump(false, false));
        assert!(!surprised(&p));
        assert_eq!(p.take_speech(), Some(Speech::Ow));
        p.update(DT, &w);
    }
}

#[test]
fn a_gentle_bump_from_a_friend_startles_it() {
    let (mut p, w) = standing(panda(), 3);
    p.feel_hit(bump(false, false));
    assert!(surprised(&p), "{:?}", p.state);
    assert_eq!(p.take_speech(), Some(Speech::Surprise));
    assert!(p.wobbling());
    holds_then_ends(&mut p, &w);
    // 똥에 맞아도 벌떡 — 말은 "ew!"
    let (mut p, _) = standing(panda(), 4);
    p.feel_hit(bump(false, true));
    assert!(surprised(&p));
    assert_eq!(p.take_speech(), Some(Speech::Ew));
}

#[test]
fn a_hard_hit_still_tumbles_and_a_sleeper_still_grumbles() {
    let (mut p, _) = standing(panda(), 5);
    p.feel_hit(bump(true, false));
    assert!(matches!(p.state, State::Tumble { .. }), "{:?}", p.state);
    let (mut p, _) = standing(panda(), 6);
    p.state = State::Sleep { remaining: 30.0 };
    p.feel_hit(bump(false, false));
    assert!(matches!(p.state, State::Annoyed { .. }), "{:?}", p.state);
}

#[test]
fn the_bundled_red_panda_has_the_surprise_pose_and_climbs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/redpanda/pet.json");
    let m = crate::manifest::parse_manifest(&std::fs::read_to_string(dir).unwrap()).unwrap();
    assert_eq!(m.special(), Some(Special::Surprise));
    assert!(m.can_climb() && m.can_look_up());
    assert!(m.animations["surprise"].front);
}
