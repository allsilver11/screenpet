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

/// ⚔️ 마주 보고 ("Expelliarmus!" / "You again, Potter?") 빨강·초록 빛줄기가 뻗어 가운데에서 맞부딪혀 밀고 당김 → 약 15초 동안 여러 합:
/// 저주가 날아가고 (빨강·초록 Bolt), 맞으면 튕겨 💫 누웠다 일어나고, 달아나고 쫓고, 폴짝·숙여 피함 → 숨 고르고 서로 반대쪽으로 걸어가 흩어짐
#[test]
fn harry_and_voldemort_fight_a_long_fierce_duel_then_part() {
    let w = world(vec![]);
    let mut pets = pair(600.0, 1300.0, &w);
    pets[0].facing = -1.0;
    pets[1].facing = 1.0;
    sync_lifts(&mut pets);
    assert_eq!((pets[0].dueling(), pets[1].dueling()), (Some(2), Some(1)));
    assert_eq!((pets[0].facing, pets[1].facing), (1.0, -1.0), "마주 봄");
    assert_eq!((pets[0].speech, pets[1].speech), (Some(Speech::Expelliarmus), Some(Speech::DuelCall)));
    assert_eq!((pets[0].render().anim, pets[1].render().anim), (Anim::Cast, Anim::Cast));
    let len = pets[0].duel.bout.as_ref().unwrap().len;
    assert_eq!(pets[1].duel.bout.as_ref().map(|b| b.len), Some(len), "같은 판");
    let (mut reached, mut lo, mut hi, mut dueled) = (false, 1.0f64, 0.0f64, 0.0);
    let (mut bolts, mut dazed, mut ran, mut lifted, mut said) = (Vec::new(), [false; 2], [false; 2], false, Vec::new());
    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    for _ in 0..(25.0 / DT) as usize {
        tick(&mut pets, &w);
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(s) = p.take_speech() {
                said.push((i, s));
            }
        }
        if pets[0].dueling().is_none() {
            break;
        }
        dueled += DT;
        for (i, p) in pets.iter().enumerate() {
            let r = p.render();
            dazed[i] |= r.thud && r.anim == Anim::Sleep;
            ran[i] |= r.anim == Anim::Run;
            lifted |= p.duel_lift() > 0.0;
            min_x = min_x.min(p.body.x);
            max_x = max_x.max(p.body.x);
            let mut fx = Vec::new();
            p.magic_fx(&mut fx);
            for f in fx {
                match f {
                    MagicFx::Bolt { color, .. } => bolts.push(color),
                    MagicFx::Duel { red, green, clash, reach, light, .. } => {
                        assert_eq!((i, light), (0, crate::magic::BeamColor::Red), "맞부딪힘 빛줄기는 빨강 쪽만");
                        let k = (clash.0 - red.0) / (green.0 - red.0);
                        if reach >= 1.0 {
                            reached = true;
                            lo = lo.min(k);
                            hi = hi.max(k);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(reached && hi - lo > 0.1, "밀고 당김: {lo} {hi}");
    assert!((dueled - len).abs() < 0.1 && dueled >= 13.0, "약 15초: {dueled} / {len}");
    assert!(bolts.contains(&crate::magic::BeamColor::Red) && bolts.contains(&crate::magic::BeamColor::Green), "저주를 서로 날림");
    assert!(dazed.iter().any(|&d| d), "맞으면 💫 누움");
    assert!(ran == [true, true], "달아나고 쫓음");
    assert!(lifted, "튕기거나 폴짝");
    assert!(max_x - min_x > 700.0 + 128.0, "좌우로 오감: {min_x} … {max_x}");
    let lines: Vec<Speech> = said.iter().map(|&(_, s)| s).collect();
    assert!(lines.contains(&Speech::DuelRest) && lines.contains(&Speech::DuelFlee) && lines.contains(&Speech::DuelChase), "{lines:?}");
    assert!(lines.contains(&Speech::Stupefy) || lines.contains(&Speech::AvadaKedavra), "{lines:?}");
    assert!(matches!(pets[0].state, State::Move { .. }) && matches!(pets[1].state, State::Move { .. }), "흩어짐");
    assert!(pets[0].facing * (pets[1].body.x - pets[0].body.x) < 0.0 && pets[1].facing * (pets[0].body.x - pets[1].body.x) < 0.0, "서로 반대쪽으로");
    assert!(pets[0].duel.ready_at >= pets[0].clock + wz::DUEL_CALM_SECS.0 - 1.0, "쿨다운이 김");
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut pets, &w);
        assert!(pets[0].dueling().is_none(), "곧바로 또 하지 않음");
    }
}

/// ⚔️ 맞은 쪽은 짧게 튕겨 나가 2초 💫 누웠다가 일어나 다시 결투를 이어 감 — 헤르미온느의 Petrificus는 통나무처럼 뻣뻣하게 쓰러짐 (render.topple)
#[test]
fn hits_knock_back_faint_two_seconds_and_petrificus_topples_like_a_log() {
    use crate::duel_bout::{Bout, Curse, Move as Bm, Outcome, Side, Step};
    let w = world(vec![]);
    let mut pets = pair(600.0, 1100.0, &w);
    pets[0].cfg = PetConfig { spells: SpellSet::of(&[Spell::Leviosa, Spell::Reparo]), bookworm: true, ..pet_cfg() };
    sync_lifts(&mut pets);
    assert_eq!(pets[0].speech, Some(Speech::Petrificus), "헤르미온느의 저주");
    // 판을 바꿔 끼움: 헤르미온느가 쏜 Petrificus가 볼드모트에게 맞음
    let step = Step { at: 0.0, mv: Bm::Shot { by: Side::Light, outcome: Outcome::Hit }, curse: Curse::Petrificus };
    let b = Bout { steps: vec![step], rest_at: step.len(), len: step.len() + crate::duel_bout::REST_SECS };
    for p in pets.iter_mut() {
        p.duel.bout = Some(b.clone());
    }
    let x0 = pets[1].body.x;
    let (mut lying, mut topple_max, mut gold) = (0.0, 0.0f64, false);
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut pets, &w);
        let r = pets[1].render();
        if r.thud {
            lying += DT;
        }
        topple_max = topple_max.max(r.topple);
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        gold |= fx.iter().any(|f| matches!(f, MagicFx::Bolt { color: crate::magic::BeamColor::Gold, .. }));
    }
    assert!(gold, "금빛 저주");
    assert!((lying - crate::duel_bout::HIT_FAINT_SECS).abs() < 0.1, "2초: {lying}");
    assert!((topple_max - 90.0).abs() < 1.0, "볼드모트는 왼쪽을 보다 뒤 (오른쪽)로 넘어짐: {topple_max}");
    assert!(pets[1].body.x > x0 + 0.5 * 128.0, "뒤로 튕김: {x0} → {}", pets[1].body.x);
    assert_eq!(pets[1].render().topple, 0.0, "다시 일어남");
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
    let mut gold = false;
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut pets, &w);
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        if let Some(MagicFx::Duel { light, .. }) = fx.first().copied() {
            assert_eq!(light, crate::magic::BeamColor::Gold);
            gold = true;
        }
    }
    assert!(gold, "금빛 빛줄기");
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
