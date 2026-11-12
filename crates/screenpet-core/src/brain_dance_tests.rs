//! 🕺 연타 춤 (흐물이 v0.6.3)

use super::dance_moves::{CLICK_WINDOW, DANCE_SECS, MAX_SECS};
use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::test_util::{pet_cfg, DT};

/// 몸을 한 번 눌렀다 뗌 (끌지 않고), 그 뒤 gap초 흐름
fn click(p: &mut Pet, w: &World, gap: f64) {
    let (x, y) = (p.body.x, p.body.y - 40.0);
    p.pointer_down(x, y);
    p.update(DT, w);
    p.pointer_up(x, y);
    let mut t = DT;
    while t < gap {
        p.update(DT, w);
        t += DT;
    }
}

fn dancing(p: &Pet) -> bool {
    matches!(p.state, State::Dance { .. })
}

#[test]
fn dance_animations_are_front_views() {
    for (a, name) in [(Anim::Dance, "dance"), (Anim::Dance2, "dance2"), (Anim::CheerType, "cheer_type"), (Anim::Type, "type")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(a.is_front() && crate::manifest::ANTICS_ANIMS.contains(&name));
    }
}

#[test]
fn four_quick_clicks_make_it_dance() {
    let w = world(vec![]);
    let mut danced = 0;
    for seed in 0..20 {
        let mut p = resting(heumul(), 800.0, seed, &w);
        for _ in 0..4 {
            click(&mut p, &w, 0.3);
        }
        // 폴짝 중에 받은 클릭이면 내려서서 춘다
        for _ in 0..90 {
            if dancing(&p) {
                break;
            }
            p.update(DT, &w);
        }
        danced += usize::from(dancing(&p));
        if dancing(&p) {
            assert_eq!(p.render().anim, Anim::Dance);
            assert!(!p.render().sparkle);
        }
    }
    assert_eq!(danced, 20);
}

#[test]
fn three_clicks_slow_clicks_and_drags_do_not() {
    let w = world(vec![]);
    for seed in 0..10 {
        let mut p = resting(heumul(), 800.0, seed, &w);
        for _ in 0..3 {
            click(&mut p, &w, 0.3);
        }
        for _ in 0..120 {
            p.update(DT, &w);
        }
        assert!(!dancing(&p), "세 번");
        let mut p = resting(heumul(), 800.0, seed, &w);
        for _ in 0..5 {
            click(&mut p, &w, CLICK_WINDOW * 0.6);
            assert!(!dancing(&p), "느린 클릭");
        }
        // 끌기는 세지 않는다
        let mut p = resting(heumul(), 800.0, seed, &w);
        for _ in 0..6 {
            let (x, y) = (p.body.x, p.body.y - 40.0);
            p.pointer_down(x, y);
            p.pointer_move(x + 40.0, y);
            p.pointer_up(x + 40.0, y);
            p.update(DT, &w);
            assert!(!dancing(&p), "끌기");
        }
    }
}

#[test]
fn more_clicks_make_a_fancier_longer_dance_and_it_stops_a_few_seconds_after_the_last() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 3, &w);
    p.facing = 1.0;
    // 가만히 서서 받게 (첫 클릭 반응이 폴짝이어도 상관없게 춤이 될 때까지)
    while !dancing(&p) {
        click(&mut p, &w, 0.25);
    }
    let start = p.clock;
    let State::Dance { remaining } = p.state else { unreachable!() };
    assert!(remaining <= DANCE_SECS && remaining > DANCE_SECS - 0.4);
    for _ in 0..4 {
        click(&mut p, &w, 0.2);
    }
    assert!(dancing(&p) && p.render().anim == Anim::Dance2 && !p.render().sparkle, "8번째부터 dance2");
    for _ in 0..4 {
        click(&mut p, &w, 0.2);
    }
    assert!(dancing(&p) && p.render().anim == Anim::Dance2 && p.render().sparkle, "12번째부터 반짝이");
    let last = p.clock;
    while dancing(&p) {
        p.update(DT, &w);
    }
    let after = p.clock - last;
    assert!(after >= DANCE_SECS - 0.3 && after <= MAX_SECS + 0.1, "{after}");
    assert!(p.clock - start > DANCE_SECS);
    assert!(!p.render().sparkle);
}

#[test]
fn only_heumul_dances_and_sleeping_or_hungry_clicks_stay_the_same() {
    let w = world(vec![]);
    let mut cat = resting(pet_cfg(), 800.0, 1, &w);
    for _ in 0..8 {
        click(&mut cat, &w, 0.2);
    }
    assert!(!dancing(&cat));
    // 자는 흐물이: 세 번 누르면 깬다 (춤이 아니다)
    let mut p = resting(heumul(), 800.0, 2, &w);
    p.state = State::Sleep { remaining: 100.0 };
    for _ in 0..3 {
        click(&mut p, &w, 0.1);
    }
    assert!(matches!(p.state, State::Trick { kind: Trick::Stretch, .. }), "{:?}", p.state);
    // 배고픈 흐물이: 음식 고르기가 먼저
    let mut p = resting(heumul(), 800.0, 3, &w);
    p.set_hungry(true);
    for _ in 0..5 {
        click(&mut p, &w, 0.1);
    }
    assert!(!dancing(&p) && p.take_food_request());
}
