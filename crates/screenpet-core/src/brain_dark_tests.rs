//! 🐍 볼드모트 (brain_dark.rs) + ⚡ Stupefy (brain_spell.rs): 초록 빛줄기에 맞은 펫이 💫 잠깐 기절 (쿵 → 누워 💫 → 일어남) ·
//! 물건은 휙 · 나기니가 발밑을 오가다 사라짐 · 가끔 "hehehe"

use super::hoist_moves::{notice_pets, sync_lifts};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::juggle::ObjectSpot;
use crate::magic::{BeamColor, MagicFx};
use crate::play::ObjectPlay;
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{self as wz, Spell, SpellSet};

const GROUND: f64 = 1040.0;

/// 볼드모트 (테스트 설정): Stupefy + 나기니·hehehe
pub(super) fn voldemort() -> PetConfig {
    PetConfig { spells: SpellSet::of(&[Spell::Stupefy]), dark_lord: true, rides_broom: true, ..pet_cfg() }
}

/// 바닥(x)에 쉬는 마법사 — 빗자루·스스로 주문·나기니·결투·패트로누스는 꺼 둠
pub(super) fn still(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(c, x, seed, w);
    p.hush_stage();
    p.broom.ready_at = f64::INFINITY;
    p.broom.snitch_ready_at = f64::INFINITY;
    p.spell.ready_at = f64::INFINITY;
    p.study.read_ready_at = f64::INFINITY;
    p.study.bag_ready_at = f64::INFINITY;
    p.dark.nagini_ready_at = f64::INFINITY;
    p.duel.ready_at = f64::INFINITY;
    p.duel.patronus_ready_at = f64::INFINITY;
    p
}

#[test]
fn dark_lines_have_their_sheet_names() {
    for (s, name) in [
        (Speech::Stupefy, "stupefy"),
        (Speech::Dazed, "dazed"),
        (Speech::Nagini, "nagini"),
        (Speech::Hehehe, "hehehe"),
        (Speech::DuelCall, "duel_call"),
        (Speech::Patronus, "patronus"),
        (Speech::Gloom, "gloom"),
        (Speech::Repelled, "repelled"),
        (Speech::SoulDrain, "soul_drain"),
        (Speech::SoulTaken, "soul_taken"),
        (Speech::SoulSaved, "soul_saved"),
    ] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// ⚡ Stupefy: 가까운 펫에게 지팡이 끝에서 초록 빛줄기 → 그 펫이 쿵 (land) → 누워 (sleep) 💫 (render.thud) → 몇 초 뒤 일어나 평소대로
#[test]
fn stupefy_shoots_a_green_bolt_and_the_pet_faints_briefly() {
    let w = world(vec![]);
    let mut v = still(voldemort(), 900.0, 1, &w);
    let mut other = resting(pet_cfg(), 700.0, 3, &w);
    other.hush_stage();
    v.social_key = 1;
    other.social_key = 2;
    let mut pets = vec![v, other];
    notice_pets(&mut pets);
    assert!(pets[0].start_spell());
    assert_eq!((pets[0].speech, pets[0].casting(), pets[0].facing), (Some(Speech::Stupefy), Some(Spell::Stupefy), -1.0));
    let (mut bolt, mut dropped, mut lay, mut said) = (None, false, false, false);
    let mut fainted_for = 0.0;
    for _ in 0..(8.0 / DT) as usize {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
        let mut fx = Vec::new();
        pets[0].magic_fx(&mut fx);
        if let Some(MagicFx::Bolt { from, to, color, width }) = fx.first().copied() {
            assert_eq!(color, BeamColor::Green);
            assert!(width > 0.0);
            bolt.get_or_insert((from, to));
        }
        said |= pets[1].take_speech() == Some(Speech::Dazed);
        if pets[1].is_fainted() {
            fainted_for += DT;
            let r = pets[1].render();
            assert!(r.thud, "💫");
            dropped |= r.anim == Anim::Land;
            lay |= r.anim == Anim::Sleep;
            assert!(pets[1].standing.is_some() && pets[1].body.y == GROUND);
        }
    }
    let (from, to) = bolt.expect("초록 빛줄기");
    assert!(from.0 < 900.0 && from.1 < GROUND && from.1 > GROUND - pets[0].cfg.size, "지팡이 끝 (왼쪽을 봄): {from:?}");
    assert!((to.0 - 700.0).abs() < 1.0 && (to.1 - (GROUND - 0.5 * pets[1].cfg.size)).abs() < 1.0, "맞는 펫 몸 가운데: {to:?}");
    assert!(dropped && lay && said);
    assert!(fainted_for >= wz::FAINT_SECS.0 - 0.1 && fainted_for <= wz::FAINT_SECS.1 + 0.1, "{fainted_for}");
    assert!(!pets[1].is_fainted() && pets[1].casting().is_none(), "일어나 평소대로");
    let mut fx = Vec::new();
    pets[0].magic_fx(&mut fx);
    assert!(fx.is_empty(), "빛줄기는 휘두르는 동안만");
}

/// ⚡ 바닥의 물건은 휙 날림 (기절은 펫만)
#[test]
fn stupefy_flicks_a_floor_object_away() {
    let w = world(vec![]);
    let mut v = still(voldemort(), 900.0, 2, &w);
    let thing = ObjectSpot { id: 5, x: 1100.0, y: GROUND, owner: Owner::Ground(0), half_w: 24.0, giant: false, local: false };
    v.notice_play_objects(&[thing], &w);
    assert!(v.start_spell());
    let mut events = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        v.update(DT, &w);
        events.extend(v.take_object_play());
    }
    assert!(matches!(events.as_slice(), [ObjectPlay::Yank { id: 5, vx, vy }] if *vx > 0.0 && *vy < 0.0), "{events:?}");
}

/// 💫 기절은 들 수 있는 때만 — 잡혀 있거나 나는 동물이면 그대로
#[test]
fn only_a_free_pet_on_the_ground_faints() {
    let w = world(vec![]);
    let mut p = resting(pet_cfg(), 700.0, 3, &w);
    p.state = State::Dragged;
    assert!(!p.be_stunned(3.0));
    let mut bird = resting(PetConfig { flies: true, ..pet_cfg() }, 700.0, 3, &w);
    assert!(!bird.be_stunned(3.0));
    let mut ok = resting(pet_cfg(), 700.0, 3, &w);
    assert!(ok.be_stunned(3.0) && ok.is_fainted());
}

/// 🐍 "Serpensortia!" → 맞는 때 발 앞에 나기니가 나타나 부른 자리 둘레를 오가며 기다가 (덮개 창: 스르륵 나타남·사라짐) 사라짐
#[test]
fn nagini_appears_slithers_around_him_and_vanishes() {
    let w = world(vec![]);
    let mut v = still(voldemort(), 900.0, 4, &w);
    v.facing = 1.0;
    v.call_nagini();
    assert_eq!((v.speech, v.casting()), (Some(Speech::Nagini), Some(Spell::Serpensortia)));
    let (mut seen, mut turned, mut first_dir, mut max_alpha): (f64, bool, Option<f64>, f64) = (0.0, false, None, 0.0);
    let reach = wz::NAGINI_RANGE * v.cfg.size;
    for _ in 0..(30.0 / DT) as usize {
        v.update(DT, &w);
        let Some(s) = v.nagini() else { continue };
        seen += DT;
        assert!(s.y == GROUND && (s.x - 900.0).abs() <= reach + 1.0, "{s:?}");
        let d = *first_dir.get_or_insert(s.dir);
        turned |= s.dir != d;
        let mut fx = Vec::new();
        v.magic_fx(&mut fx);
        let Some(MagicFx::Snake { alpha, size, .. }) = fx.iter().copied().find(|f| matches!(f, MagicFx::Snake { .. })) else { panic!("{fx:?}") };
        assert!((size - wz::NAGINI_LEN * v.cfg.size).abs() < 1e-9);
        max_alpha = max_alpha.max(alpha);
    }
    assert_eq!(first_dir, Some(1.0), "부른 쪽 (앞)으로 기어 나감");
    assert!(seen >= wz::NAGINI_SECS.0 - 0.1 && seen <= wz::NAGINI_SECS.1 + 0.1, "{seen}");
    assert!(turned, "오감");
    assert!((max_alpha - 1.0).abs() < 1e-9);
    assert!(v.nagini().is_none() && v.dark.nagini_ready_at > v.clock, "사라지고 한동안 안 나옴");
}

/// 🐍 한가하면 가끔 스스로 부름 · 😈 가끔 "hehehe" — 볼드모트만
#[test]
fn he_calls_nagini_and_cackles_now_and_then_on_his_own() {
    let w = world(vec![]);
    let mut v = still(voldemort(), 900.0, 5, &w);
    v.dark.nagini_ready_at = 0.0;
    let mut other = still(PetConfig { spells: SpellSet::of(&[Spell::Expelliarmus]), ..pet_cfg() }, 600.0, 6, &w);
    let (mut cackles, mut snakes, mut others) = (0, 0, 0);
    for _ in 0..(900.0 / DT) as usize {
        let had = v.nagini().is_some();
        v.update(DT, &w);
        other.update(DT, &w);
        cackles += (v.take_speech() == Some(Speech::Hehehe)) as usize;
        snakes += (!had && v.nagini().is_some()) as usize;
        others += matches!(other.take_speech(), Some(Speech::Hehehe | Speech::Nagini)) as usize;
    }
    assert!(cackles >= 2, "{cackles}");
    assert!(snakes >= 1, "{snakes}");
    assert_eq!(others, 0);
}
