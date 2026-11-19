//! 🌀 태풍에 휘둘리기 (brain_typhoon.rs) — 돌풍에 버티기·데굴·날아가기, 회오리에 매달려 돌다 내던져지기, 친구 화면으로 넘어가기.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, world};
use super::*;
use crate::test_util::{DT, flyer, idle};
use crate::typhoon::Gust;

/// 바닥 x에 쉬는 동물
fn standing_pet(cfg: PetConfig, x: f64, seed: u64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(cfg, 0.0, 0.0, seed);
    place(&mut p, &w, Owner::Ground(0), x, idle());
    (p, w)
}

const STRONG: Gust = Gust { dir: 1.0, strength: 1.0 };

#[test]
fn a_strong_gust_sometimes_blows_a_pet_off_its_feet_sometimes_tumbles_it_sometimes_it_just_holds_on() {
    let (mut blown, mut tumbled, mut leaned) = (0, 0, 0);
    for seed in 0..60 {
        let (mut p, w) = standing_pet(cfg(), 800.0, seed);
        assert!(p.feel_gust(STRONG));
        assert!(!p.feel_gust(STRONG), "막 휘청인 뒤엔 잠깐 그대로");
        match p.state {
            State::Air { anim: Anim::Fall } => {
                blown += 1;
                assert!(p.body.vx > 0.0 && p.body.vy < 0.0, "바람 쪽으로 붕: {:?}", p.body);
                assert!(!p.thrown, "던진 게 아님 — 친구 화면으로 넘어가지 않음");
                assert_eq!(p.take_speech_text().as_deref(), Some("wheee!"));
                for _ in 0..240 {
                    p.update(DT, &w);
                }
                assert!(p.standing.is_some(), "다시 내려섬");
                assert!(p.body.x > 800.0);
            }
            State::Tumble { dir, .. } => {
                tumbled += 1;
                assert_eq!(dir, 1.0);
                assert_eq!(p.take_speech(), Some(Speech::Whoa));
            }
            _ => {
                leaned += 1;
                assert!(p.render().wobble, "덜덜 버팀");
                assert_eq!(p.take_speech_text().as_deref(), Some("hold on!"));
            }
        }
    }
    assert!(blown > 5 && tumbled > 5 && leaned > 5, "{blown} {tumbled} {leaned}");
}

#[test]
fn a_flying_pet_is_pushed_along_by_the_gust() {
    let (mut p, w) = standing_pet(flyer(), 800.0, 1);
    p.update(DT, &w);
    let x0 = p.body.x;
    assert!(p.feel_gust(Gust { dir: -1.0, strength: 0.8 }));
    for _ in 0..60 {
        p.update(DT, &w);
    }
    assert!(p.body.x < x0 - 40.0, "{x0} → {}", p.body.x);
}

#[test]
fn sheltered_or_busy_pets_are_not_tossed_about_and_sleepers_only_shiver() {
    // 하우스 안
    let (mut p, _) = standing_pet(cfg(), 800.0, 1);
    p.housed = true;
    assert!(!p.storm_free() && !p.feel_gust(STRONG));
    // 손에 들림
    let (mut p, w) = standing_pet(cfg(), 800.0, 1);
    p.pointer_down(800.0, 1000.0);
    p.update(DT, &w);
    assert!(!p.storm_free() && !p.feel_gust(STRONG) && !p.whirl_to(900.0, 900.0));
    // 조작 모드: 키를 뺏지 않음
    let (mut p, _) = standing_pet(cfg(), 800.0, 1);
    p.set_control(true);
    assert!(!p.storm_free() && !p.feel_gust(STRONG));
    // 자는 동물은 덜덜만, 계속 잠 — 회오리도 못 데려감 (무거운 잠)
    let (mut p, _) = standing_pet(cfg(), 800.0, 1);
    p.state = State::Sleep { remaining: 30.0 };
    assert!(p.feel_gust(STRONG));
    assert!(matches!(p.state, State::Sleep { .. }) && p.render().wobble);
    assert!(!p.storm_free());
    // 놀러 보내는 중
    let (mut p, _) = standing_pet(cfg(), 800.0, 1);
    p.send_to(Side::Right, false);
    assert!(!p.storm_free());
}

#[test]
fn a_whirled_pet_dangles_where_the_tornado_puts_it_until_flung() {
    let (mut p, w) = standing_pet(cfg(), 800.0, 2);
    assert!(p.storm_free());
    assert!(p.whirl_to(820.0, 900.0));
    assert!(p.is_whirled() && p.storm_free(), "도는 동안도 계속 회오리 것");
    assert!(p.take_speech_text().is_some(), "whoa!");
    for i in 0..30 {
        p.update(DT, &w);
        assert_eq!((p.body.x, p.body.y), (820.0 + i as f64, 900.0 - i as f64), "물리 없이 그 자리");
        assert_eq!(p.render().anim, Anim::Dragged);
        p.whirl_to(821.0 + i as f64, 899.0 - i as f64);
    }
    assert!(p.take_speech_text().is_none(), "한 번만 외침");
    p.whirl_release(-600.0, -500.0, false);
    assert!(!p.is_whirled() && !p.thrown);
    assert_eq!(p.state, State::Air { anim: Anim::Fall });
    for _ in 0..240 {
        p.update(DT, &w);
    }
    assert!(p.standing.is_some() && p.body.x < 850.0);
}

#[test]
fn a_whirled_pet_the_tornado_forgets_falls_by_itself_and_grabbing_it_frees_it() {
    let (mut p, w) = standing_pet(cfg(), 800.0, 3);
    p.whirl_to(800.0, 700.0);
    for _ in 0..30 {
        p.update(DT, &w);
    }
    assert!(!p.is_whirled(), "앱이 더 옮기지 않으면 놓임");
    for _ in 0..120 {
        p.update(DT, &w);
    }
    assert!(p.standing.is_some(), "떨어져 내려섬 — 잃어버리지 않음");
    let (mut p, w) = standing_pet(cfg(), 800.0, 3);
    p.whirl_to(800.0, 700.0);
    p.pointer_down(800.0, 650.0);
    p.update(DT, &w);
    assert!(!p.is_whirled() && !p.storm_free(), "사용자가 잡으면 회오리가 놓아줌");
}

#[test]
fn my_pet_flung_to_a_linked_friend_crosses_like_a_throw() {
    let (mut p, w) = standing_pet(cfg(), 1500.0, 4);
    p.set_exit_edges(false, true);
    assert_eq!(p.whirl_cross_side(&w), Some(Side::Right));
    p.whirl_to(1500.0, 800.0);
    let (vx, vy) = crate::typhoon::cross_throw((1500.0, 800.0), w.bounds.right, w.bounds.bottom, 1.0).unwrap();
    p.whirl_release(vx, vy, true);
    let mut exit = None;
    for _ in 0..240 {
        p.update(DT, &w);
        if let Some(side) = p.take_exit() {
            exit = Some(side);
            break;
        }
    }
    assert_eq!(exit, Some(Side::Right));
    assert!(p.take_fling().is_some(), "던진 움직임을 이어 감 (기존 own_exited 길)");
}

#[test]
fn only_linked_edges_count_and_a_guest_is_only_flung_toward_home() {
    let (mut p, w) = standing_pet(cfg(), 500.0, 5);
    assert_eq!(p.whirl_cross_side(&w), None, "친구가 없으면 이 화면에");
    p.set_exit_edges(true, true);
    assert_eq!(p.whirl_cross_side(&w), Some(Side::Left), "가까운 끝");
    let (mut g, w) = standing_pet(cfg(), 500.0, 5);
    g.set_guest(Side::Right, 600.0);
    g.set_exit_edges(true, false);
    assert_eq!(g.whirl_cross_side(&w), Some(Side::Right), "손님은 집 쪽으로만");
    g.set_guest(Side::Up, 600.0);
    assert_eq!(g.whirl_cross_side(&w), None, "집이 위면 그냥 이 화면에서 데굴");
}
