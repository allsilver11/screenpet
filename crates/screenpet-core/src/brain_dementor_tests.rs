//! 👻 디멘터 (brain_dementor.rs): 곁의 펫이 시무룩 (annoyed 그림, "brrr...") + 배가 더 고픔 · 가까운 화면 구석에 🥶 서리가 끼고 녹음 ·
//! 🦌 패트로누스에 쫓기면 반대쪽 위 구석으로 날아가 한동안 얌전 · 가끔 다른 펫 머리 위로 다가감

use super::dementor_moves::sync_gloom;
use super::hoist_moves::{notice_pets, sync_lifts};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::magic::MagicFx;
use crate::test_util::{pet_cfg, DT};
use crate::wizard as wz;

/// 디멘터 (테스트 설정): 나는 동물 + dementor
pub(super) fn dementor_cfg() -> PetConfig {
    PetConfig { flies: true, dementor: true, ..pet_cfg() }
}

/// (x, y)에 떠 있는 디멘터
pub(super) fn floating(x: f64, y: f64, seed: u64) -> Pet {
    let mut p = Pet::new(dementor_cfg(), x, y, seed);
    p.hush_stage();
    p.state = State::Idle { remaining: 1000.0, look: false };
    p
}

/// 👻 곁의 펫: 시무룩 (annoyed 그림 + "brrr...") GLOOM_SECS 뒤 평소대로, 곁에 있는 동안 배가 더 고픔 (take_chill), 곧바로 다시 시무룩하진 않음.
/// 멀리 있는 펫은 그대로
#[test]
fn a_nearby_pet_gets_gloomy_and_hungrier_while_a_far_one_is_fine() {
    let w = world(vec![]);
    let mut near = resting(pet_cfg(), 900.0, 1, &w);
    near.hush_stage();
    near.social_key = 1;
    let mut far = resting(pet_cfg(), 200.0, 2, &w);
    far.hush_stage();
    far.social_key = 2;
    let mut d = floating(950.0, 1040.0 - 60.0, 3);
    d.social_key = 3;
    let mut pets = vec![near, far, d];
    let (mut gloomy_for, mut said, mut chill, mut near_for) = (0.0, 0, 0.0, 0.0);
    for _ in 0..(10.0 / DT) as usize {
        sync_gloom(&mut pets);
        near_for += if pets[0].dementor.near { DT } else { 0.0 };
        pets[0].update(DT, &w);
        pets[1].update(DT, &w);
        // 디멘터는 그 자리에 둠 (날아가지 않게)
        pets[2].clock += DT;
        if pets[0].is_gloomy() {
            gloomy_for += DT;
            assert_eq!(pets[0].render().anim, Anim::Annoyed);
        }
        said += (pets[0].take_speech() == Some(Speech::Gloom)) as usize;
        chill += pets[0].take_chill();
        assert!(!pets[1].is_gloomy() && pets[1].take_chill() == 0.0);
    }
    assert_eq!(said, 1, "한 번만 (GLOOM_CALM_SECS)");
    assert!((gloomy_for - wz::GLOOM_SECS).abs() < 0.1, "{gloomy_for}");
    assert!(near_for > 4.0, "{near_for}");
    assert!((chill - wz::GLOOM_HUNGER * near_for).abs() < 1e-9, "{chill} {near_for}");
    assert!(pets[2].take_chill() == 0.0 && !pets[2].dementor.near, "디멘터 자신은 아님");
}

/// 🥶 디멘터가 화면 왼쪽 위 구석 가까이 떠 있으면 그 구석에만 서리가 끼고 (덮개 창 Frost), 떠나면 천천히 녹아 없어짐
#[test]
fn frost_creeps_into_the_nearby_corner_and_melts_after_it_leaves() {
    let w = world(vec![]);
    let mut d = floating(250.0, 260.0, 4);
    for _ in 0..(8.0 / DT) as usize {
        d.dementor_tick(DT, &w);
        d.clock += DT;
    }
    let frost = d.frost().to_vec();
    assert_eq!(frost.len(), 1, "{frost:?}");
    assert!((frost[0].x, frost[0].y, frost[0].inward) == (0.0, 0.0, (1.0, 1.0)) && frost[0].level > 0.7, "{frost:?}");
    let mut fx = Vec::new();
    d.magic_fx(&mut fx);
    assert!(matches!(fx.as_slice(), [MagicFx::Frost { level, size, .. }] if *level > 0.7 && *size > 100.0), "{fx:?}");
    d.body.x = 960.0;
    d.body.y = 520.0;
    let before = d.frost()[0].level;
    d.dementor_tick(1.0, &w);
    assert!(d.frost()[0].level < before, "녹기 시작");
    for _ in 0..(60.0 / DT) as usize {
        d.dementor_tick(DT, &w);
    }
    assert!(d.frost().is_empty(), "다 녹음");
}

/// 🦌 쫓긴 디멘터: "hsss!" 하고 패트로누스 반대쪽 위 구석으로 빠르게 날아가고, 도망가 있는 동안 시무룩·서리 없음, 또 쫓기지 않음
#[test]
fn a_repelled_dementor_flees_to_the_far_top_corner_and_behaves_for_a_while() {
    let w = world(vec![]);
    let mut d = floating(700.0, 700.0, 5);
    d.social_key = 9;
    for _ in 0..(3.0 / DT) as usize {
        d.dementor_tick(DT, &w);
    }
    assert!(d.be_repelled(500.0));
    assert!(d.dementor_fleeing() && !d.dementor_active());
    assert_eq!(d.take_speech(), Some(Speech::Repelled));
    assert!(!d.be_repelled(500.0), "이미 도망 중");
    let (x0, y0) = (d.body.x, d.body.y);
    d.update(DT, &w);
    assert!(matches!(d.state, State::Move { kind: Move::Run, .. }), "{:?}", d.state);
    for _ in 0..(6.0 / DT) as usize {
        d.update(DT, &w);
    }
    assert!(d.body.x > x0 + 400.0 && d.body.y < y0 - 200.0, "오른쪽 위로: {:?} → {:?}", (x0, y0), (d.body.x, d.body.y));
    let mut pet = resting(pet_cfg(), d.body.x, 6, &w);
    pet.social_key = 1;
    let mut pets = vec![pet, d];
    sync_gloom(&mut pets);
    assert!(!pets[0].dementor.near, "도망 중엔 시무룩하게 하지 않음");
    let lvl: f64 = pets[1].frost().iter().map(|f| f.level).sum();
    pets[1].dementor_tick(1.0, &w);
    assert!(pets[1].frost().iter().map(|f| f.level).sum::<f64>() < lvl || lvl == 0.0, "서리가 녹음");
}

/// 👻 새로 날 때 가끔 같은 화면의 다른 펫 머리 위로 다가감
#[test]
fn it_drifts_over_other_pets_now_and_then() {
    let w = world(vec![]);
    let mut d = floating(300.0, 300.0, 7);
    d.social_key = 1;
    let mut pet = resting(pet_cfg(), 1400.0, 8, &w);
    pet.social_key = 2;
    let mut pets = vec![d, pet];
    notice_pets(&mut pets);
    let mut hits = 0;
    for _ in 0..40 {
        if let Some((x, y)) = pets[0].stalk_target(&w) {
            assert!((x - 1400.0).abs() <= 0.6 * pets[0].cfg.size + 1.0, "{x}");
            assert!(y < 1040.0 - pets[1].cfg.size, "머리 위: {y}");
            hits += 1;
        }
    }
    assert!(hits > 4 && hits < 30, "가끔: {hits}");
    let mut alone = vec![floating(300.0, 300.0, 9)];
    notice_pets(&mut alone);
    assert!((0..40).all(|_| alone[0].stalk_target(&w).is_none()), "다른 펫이 없으면 그대로");
    sync_lifts(&mut alone);
}
