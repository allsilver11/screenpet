//! ⚔️ 결투·🦌 패트로누스 (brain_duel.rs): 해리·볼드모트가 마주치면 마주 보고 빨강·초록 빛줄기가 맞부딪혀 밀고 당기다 펑 (둘 다 튕김) ·
//! 한쪽이 끊기면 그만 · 디멘터가 오면 "Expecto Patronum!" 은빛 사슴이 달려가 디멘터를 쫓음

use super::dark_tests::{still, voldemort};
use super::dementor_tests::floating;
use super::hoist_moves::sync_lifts;
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::magic::MagicFx;
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{self as wz, Spell, SpellSet};

/// 해리 (테스트 설정): Expelliarmus·Leviosa + 패트로누스
fn harry() -> PetConfig {
    PetConfig { spells: SpellSet::of(&[Spell::Expelliarmus, Spell::Leviosa]), patronus: true, rides_broom: true, seeker: true, ..pet_cfg() }
}

/// 해리 (x)와 볼드모트 (vx) — 결투만 켬
fn pair(x: f64, vx: f64, w: &World) -> Vec<Pet> {
    let mut h = still(harry(), x, 1, w);
    let mut v = still(voldemort(), vx, 2, w);
    h.social_key = 1;
    v.social_key = 2;
    h.duel.ready_at = 0.0;
    v.duel.ready_at = 0.0;
    vec![h, v]
}

/// 앱처럼 한 틱
fn tick(pets: &mut [Pet], w: &World) {
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets);
}

/// ⚔️ 마주 보고 ("Expelliarmus!" / "You again, Potter?") 빨강·초록 빛줄기가 뻗어 가운데에서 맞부딪혀 밀고 당기다 펑 — 둘 다 뒤로 튕김
#[test]
fn harry_and_voldemort_duel_when_they_meet_then_both_bounce_back() {
    let w = world(vec![]);
    let mut pets = pair(600.0, 1300.0, &w);
    pets[0].facing = -1.0;
    pets[1].facing = 1.0;
    sync_lifts(&mut pets);
    assert_eq!((pets[0].dueling(), pets[1].dueling()), (Some(2), Some(1)));
    assert_eq!((pets[0].facing, pets[1].facing), (1.0, -1.0), "마주 봄");
    assert_eq!((pets[0].speech, pets[1].speech), (Some(Speech::Expelliarmus), Some(Speech::DuelCall)));
    assert_eq!((pets[0].render().anim, pets[1].render().anim), (Anim::Cast, Anim::Cast));
    let (mut reached, mut lo, mut hi, mut dueled) = (false, 1.0f64, 0.0f64, 0.0);
    for _ in 0..(6.0 / DT) as usize {
        tick(&mut pets, &w);
        if pets[0].dueling().is_none() {
            break;
        }
        dueled += DT;
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        let Some(MagicFx::Duel { red, green, clash, reach, light, .. }) = fx.first().copied() else { panic!("{fx:?}") };
        assert_eq!(light, crate::magic::BeamColor::Red);
        assert!(red.0 > 600.0 && red.0 < 700.0 && green.0 < 1300.0 && green.0 > 1200.0, "지팡이 끝: {red:?} {green:?}");
        let k = (clash.0 - red.0) / (green.0 - red.0);
        if reach >= 1.0 {
            reached = true;
            lo = lo.min(k);
            hi = hi.max(k);
            assert!(pets[0].render().sparkle);
        }
        let mut other = Vec::new();
        pets[1].magic_fx(&mut other);
        assert!(other.is_empty(), "빛줄기는 빨강 쪽만 내놓음");
    }
    assert!(reached && lo < 0.45 && hi > 0.55, "밀고 당김: {lo} {hi}");
    assert!(dueled >= wz::DUEL_SECS.0 - 0.1 && dueled <= wz::DUEL_SECS.1 + 0.1, "{dueled}");
    assert!(matches!(pets[0].state, State::Air { .. }) && pets[0].body.vx < 0.0 && pets[0].body.vy < 0.0, "해리는 왼쪽 뒤로 튕김");
    assert!(matches!(pets[1].state, State::Air { .. }) && pets[1].body.vx > 0.0, "볼드모트는 오른쪽 뒤로");
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut pets, &w);
        assert!(pets[0].dueling().is_none(), "곧바로 또 하지 않음");
    }
    assert!(pets[0].standing.is_some() && pets[1].standing.is_some(), "내려섬");
}

/// ⚔️ 너무 멀거나 높이가 다르거나 한쪽이 바쁘면 시작하지 않음
#[test]
fn no_duel_when_far_apart_on_different_levels_or_busy() {
    let w = world(vec![]);
    let mut far = pair(100.0, 100.0 + 10.0 * 128.0, &w);
    sync_lifts(&mut far);
    assert!(far[0].dueling().is_none());
    let mut high = pair(600.0, 1000.0, &w);
    high[1].body.y -= 3.0 * 128.0;
    sync_lifts(&mut high);
    assert!(high[0].dueling().is_none());
    let mut busy = pair(600.0, 1000.0, &w);
    busy[1].be_stunned(3.0);
    sync_lifts(&mut busy);
    assert!(busy[0].dueling().is_none());
    let mut two_harries = pair(600.0, 1000.0, &w);
    two_harries[1].cfg = harry();
    sync_lifts(&mut two_harries);
    assert!(two_harries[0].dueling().is_none(), "밝은 쪽·초록이 만나야");
    let mut muggle = pair(600.0, 1000.0, &w);
    muggle[0].cfg = pet_cfg();
    sync_lifts(&mut muggle);
    assert!(muggle[1].dueling().is_none(), "주문을 모르는 펫과는 안 함");
}

/// ⚔️ 헤르미온느도 볼드모트와 마주치면 같은 결투 (금빛 빛줄기 vs 초록) — 해리 없이도
#[test]
fn hermione_duels_voldemort_with_a_gold_beam() {
    let w = world(vec![]);
    let mut pets = pair(600.0, 1100.0, &w);
    pets[0].cfg = PetConfig { spells: SpellSet::of(&[Spell::Leviosa, Spell::Reparo]), bookworm: true, ..pet_cfg() };
    sync_lifts(&mut pets);
    assert_eq!((pets[0].dueling(), pets[1].dueling()), (Some(2), Some(1)));
    let mut popped = false;
    for _ in 0..(6.0 / DT) as usize {
        tick(&mut pets, &w);
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        if let Some(MagicFx::Duel { light, .. }) = fx.first().copied() {
            assert_eq!(light, crate::magic::BeamColor::Gold);
        }
        popped |= matches!(pets[0].state, State::Air { .. }) && matches!(pets[1].state, State::Air { .. });
    }
    assert!(popped, "펑 — 둘 다 튕김");
}

/// ⚔️ 결투 중 한쪽이 잡히면 다른 쪽도 그만둠
#[test]
fn a_duel_ends_for_both_when_one_is_grabbed() {
    let w = world(vec![]);
    let mut pets = pair(600.0, 1000.0, &w);
    sync_lifts(&mut pets);
    for _ in 0..30 {
        tick(&mut pets, &w);
    }
    pets[1].state = State::Dragged;
    pets[1].standing = None;
    tick(&mut pets, &w);
    assert!(pets[0].dueling().is_none() && !matches!(pets[0].state, State::Air { .. }), "{:?}", pets[0].state);
}

/// 🦌 디멘터가 해리 곁에 오면 "Expecto Patronum!" — 은빛 사슴이 디멘터에게 달려가 닿으면 디멘터가 도망, 사슴은 흐려져 사라짐
#[test]
fn a_dementor_near_harry_is_chased_off_by_the_patronus_stag() {
    let w = world(vec![]);
    let mut h = still(harry(), 600.0, 1, &w);
    h.social_key = 1;
    h.duel.patronus_ready_at = 0.0;
    let mut d = floating(950.0, 900.0, 2);
    d.social_key = 2;
    let mut pets = vec![h, d];
    sync_lifts(&mut pets);
    assert_eq!((pets[0].casting(), pets[0].speech), (Some(Spell::Patronus), Some(Speech::Patronus)));
    let (mut stag_seen, mut first, mut last, mut repelled_at) = (0.0, None, None, None);
    for i in 0..(5.0 / DT) as usize {
        tick(&mut pets, &w);
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        if let Some(MagicFx::Stag { x, y, size, dir, alpha }) = fx.iter().copied().find(|f| matches!(f, MagicFx::Stag { .. })) {
            stag_seen += DT;
            assert!(size > 0.0 && alpha >= 0.0 && dir == 1.0);
            first.get_or_insert((x, y));
            last = Some((x, y));
        }
        if repelled_at.is_none() && pets[1].dementor_fleeing() {
            repelled_at = Some(i as f64 * DT);
            assert_eq!(pets[1].take_speech(), Some(Speech::Repelled));
        }
    }
    let (first, last) = (first.unwrap(), last.unwrap());
    assert!(first.0 < 750.0 && last.0 > 950.0, "해리 지팡이 끝에서 디멘터 너머로: {first:?} → {last:?}");
    assert!((stag_seen - wz::STAG_SECS).abs() < 0.1, "{stag_seen}");
    let at = repelled_at.expect("디멘터가 도망");
    assert!(at > wz::CAST_HIT + wz::STAG_RUN_SECS - 0.1 && at < wz::CAST_HIT + wz::STAG_RUN_SECS + 0.2, "{at}");
    assert!(pets[0].stag().is_none());
    assert!(pets[0].casting().is_none() && pets[0].duel.patronus_ready_at > pets[0].clock, "곧바로 또 쓰지 않음");
}

/// 🦌 해리에게서 먼 디멘터도 같은 화면에서 다른 펫을 시무룩하게 하면 패트로누스 — 아무도 곁에 없으면 그대로
#[test]
fn harry_also_saves_other_pets_on_screen_but_ignores_a_lonely_far_dementor() {
    let w = world(vec![]);
    let mut h = still(harry(), 200.0, 1, &w);
    h.social_key = 1;
    h.duel.patronus_ready_at = 0.0;
    let mut victim = resting(pet_cfg(), 1500.0, 3, &w);
    victim.social_key = 3;
    let mut d = floating(1550.0, 980.0, 2);
    d.social_key = 2;
    let mut pets = vec![h, victim, d];
    sync_lifts(&mut pets);
    assert_eq!(pets[0].casting(), Some(Spell::Patronus));

    let mut h = still(harry(), 200.0, 1, &w);
    h.social_key = 1;
    h.duel.patronus_ready_at = 0.0;
    let mut d = floating(1550.0, 500.0, 2);
    d.social_key = 2;
    let mut lonely = vec![h, d];
    sync_lifts(&mut lonely);
    assert!(lonely[0].casting().is_none());
}
