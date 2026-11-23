//! 🎒 가방에 숨기 (brain_tuck.rs): 해리·헤르미온느·볼드모트가 한 화면에 모이면 헤르미온느가 해리를 끝없는 구슬 가방에 쏙 숨기고
//! ("In you go!") 대신 볼드모트와 결투 · 결투가 끝나면 해리가 가방에서 튀어나옴 ("Phew!") · 헤르미온느가 떠나거나 해리가 불려 가면 바로 튀어나옴

use super::dark_tests::{still, voldemort};
use super::hoist_moves::sync_lifts;
use super::tests::world;
use super::*;
use crate::magic::{BeamColor, MagicFx};
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{self as wz, Spell, SpellSet};

/// 해리 (테스트 설정): Expelliarmus·Leviosa + 패트로누스
fn harry() -> PetConfig {
    PetConfig { spells: SpellSet::of(&[Spell::Expelliarmus, Spell::Leviosa]), patronus: Some(crate::wizard::PatronusAnimal::Stag), ..pet_cfg() }
}

/// 헤르미온느 (테스트 설정): 책벌레 + 가방
fn hermione() -> PetConfig {
    PetConfig { spells: SpellSet::of(&[Spell::Leviosa, Spell::Reparo]), bookworm: true, bag_cleaner: true, ..pet_cfg() }
}

/// 해리 (hx) · 헤르미온느 (mx) · 볼드모트 (vx) — 결투만 켬 (key 1·2·3)
fn trio(hx: f64, mx: f64, vx: f64, w: &World) -> Vec<Pet> {
    let mut pets = vec![still(harry(), hx, 1, w), still(hermione(), mx, 2, w), still(voldemort(), vx, 3, w)];
    for (k, p) in pets.iter_mut().enumerate() {
        p.social_key = k as u64 + 1;
        p.duel.ready_at = 0.0;
    }
    pets
}

/// 앱처럼 한 틱 (들은 말은 모음)
fn tick(pets: &mut [Pet], w: &World, said: &mut Vec<(u64, Speech)>) {
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets);
    for p in pets.iter_mut() {
        if let Some(s) = p.take_speech() {
            said.push((p.social_key, s));
        }
    }
}

#[test]
fn tuck_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::InYouGo, "in_you_go"), (Speech::PoppedOut, "popped_out")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 🎒 셋이 모이면 해리끼리 결투하지 않고: 헤르미온느가 "In you go!" — 해리는 작아지며 가방 쪽으로 쏙 (안 보임) →
/// 헤르미온느 (금빛)·볼드모트 결투 → 끝나면 해리가 "Phew!" 하며 튀어나와 내려섬. 곧바로 또 결투하지 않음
#[test]
fn hermione_hides_harry_in_her_bag_duels_voldemort_and_harry_pops_out() {
    let w = world(vec![]);
    let mut pets = trio(500.0, 700.0, 1200.0, &w);
    sync_lifts(&mut pets);
    assert_eq!(pets[1].dueling(), Some(3), "헤르미온느가 결투");
    assert_eq!(pets[2].dueling(), Some(2));
    assert_eq!(pets[0].dueling(), None, "해리는 결투하지 않음");
    assert_eq!(pets[0].tucked_in(), Some(2), "해리는 헤르미온느 가방 속");
    assert_eq!(pets[1].speech, Some(Speech::InYouGo));
    assert!(pets[1].render().bag > 0.0, "가방을 꺼냄");
    assert_eq!(pets[1].render().anim, Anim::BagStuff, "가방을 벌려 듦");
    let mut said = Vec::new();
    let (mut tucked_full, mut gold, mut bag_all_along) = (false, false, true);
    let mut popped_at = None;
    for k in 0..(26.0 / DT) as usize {
        tick(&mut pets, &w, &mut said);
        if pets[1].dueling().is_some() {
            bag_all_along &= pets[1].render().bag > 0.0;
            let mut fx = Vec::new();
            pets[1].magic_fx(&mut fx);
            if let Some(MagicFx::Duel { light, reach, .. }) = fx.first().copied() {
                gold |= light == BeamColor::Gold && reach >= 1.0;
            }
        }
        if pets[0].tucked_in().is_some() {
            let r = pets[0].render();
            if r.tuck >= 1.0 {
                tucked_full = true;
                assert!((pets[0].body.x - pets[1].body.x).abs() < pets[1].cfg.size, "가방과 함께 다님");
            }
        } else if popped_at.is_none() {
            popped_at = Some(k);
            assert!(pets[1].dueling().is_none(), "결투가 끝나야 나옴");
            assert_eq!(pets[0].render().tuck, 0.0);
        }
    }
    assert!(tucked_full && gold && bag_all_along, "쏙 숨고 ({tucked_full}) 금빛 결투 ({gold}) 동안 가방은 손에 ({bag_all_along})");
    assert!(popped_at.is_some(), "튀어나옴");
    assert!(said.contains(&(1, Speech::PoppedOut)), "{said:?}");
    assert!(pets[0].standing.is_some(), "내려섬");
    for _ in 0..(5.0 / DT) as usize {
        tick(&mut pets, &w, &mut said);
        assert!(pets[0].dueling().is_none() && pets[0].tucked_in().is_none(), "곧바로 또 하지 않음");
    }
}

/// 헤르미온느가 해리와 멀리 떨어져 있으면 숨기지 않고 평소 결투 (해리 vs 볼드모트)
#[test]
fn far_apart_harry_duels_as_usual() {
    let w = world(vec![]);
    let mut pets = trio(800.0, 100.0, 1300.0, &w);
    sync_lifts(&mut pets);
    assert_eq!(pets[0].dueling(), Some(3));
    assert_eq!(pets[0].tucked_in(), None);
}

/// 🎒 헤르미온느가 사라지면 (친구 화면으로 등) 해리가 바로 제자리에서 튀어나옴
#[test]
fn harry_pops_out_when_hermione_leaves() {
    let w = world(vec![]);
    let mut pets = trio(500.0, 700.0, 1200.0, &w);
    sync_lifts(&mut pets);
    assert_eq!(pets[0].tucked_in(), Some(2));
    let mut said = Vec::new();
    for _ in 0..(1.5 / DT) as usize {
        tick(&mut pets, &w, &mut said);
    }
    pets.remove(1);
    tick(&mut pets, &w, &mut said);
    assert_eq!(pets[0].tucked_in(), None);
    assert!(said.contains(&(1, Speech::PoppedOut)));
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut pets, &w, &mut said);
    }
    assert!(pets[0].standing.is_some() && pets[0].render().tuck == 0.0);
}

/// 🎒 가방 속 해리가 불려 가면 (간질이기·볼일 등 — romp_interrupted) 혼자 튀어나옴, 헤르미온느는 결투를 이어 감
#[test]
fn harry_pops_out_when_called_away() {
    let w = world(vec![]);
    let mut pets = trio(500.0, 700.0, 1200.0, &w);
    sync_lifts(&mut pets);
    let mut said = Vec::new();
    for _ in 0..(1.0 / DT) as usize {
        tick(&mut pets, &w, &mut said);
    }
    pets[0].tickling = true;
    tick(&mut pets, &w, &mut said);
    pets[0].tickling = false;
    assert_eq!(pets[0].tucked_in(), None);
    assert!(said.contains(&(1, Speech::PoppedOut)));
    assert_eq!(pets[1].dueling(), Some(3), "결투는 그대로");
}

/// 🎒 가방 속 해리는 결투가 아주 길어져도 TUCK_MAX_SECS 넘게 있지 않음
#[test]
fn harry_never_stays_in_the_bag_forever() {
    let w = world(vec![]);
    let mut pets = trio(500.0, 700.0, 1200.0, &w);
    sync_lifts(&mut pets);
    for i in [1, 2] {
        if let Some(b) = pets[i].duel.bout.as_mut() {
            b.len = 1e6;
        }
    }
    let mut said = Vec::new();
    for _ in 0..((wz::TUCK_MAX_SECS + 1.0) / DT) as usize {
        tick(&mut pets, &w, &mut said);
    }
    assert_eq!(pets[0].tucked_in(), None);
}
