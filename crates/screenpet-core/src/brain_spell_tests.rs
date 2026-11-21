//! ✨ 주문 (brain_spell.rs): Expelliarmus (든 것 떨어뜨리기·바닥의 물건 휙) · Wingardium Leviosa (물건·작은 펫 둥둥 — 해리는 가끔 실패) ·
//! Reparo (쓰레기 반짝 치움) · 📌 장난감·거대한 것은 안 노림 · 둥둥 뜬 펫은 내려와 평소대로

use super::hoist_moves::{notice_pets, sync_lifts};
use super::juggle_moves::JugglePhase;
use super::spell_moves::SpellTarget;
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::juggle::ObjectSpot;
use crate::magic::MagicFx;
use crate::play::ObjectPlay;
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{self as wz, Spell, SpellSet};

const GROUND: f64 = 1040.0;

/// 주문을 쓰는 마법사 (테스트 설정): spells, 책벌레면 Leviosa가 늘 됨
fn caster(spells: &[Spell], bookworm: bool) -> PetConfig {
    PetConfig { spells: SpellSet::of(spells), bookworm, rides_broom: true, ..pet_cfg() }
}

/// 바닥(x)에 쉬는 마법사 — 빗자루·책·스스로 주문은 꺼 둠
fn calm(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(c, x, seed, w);
    p.hush_stage();
    p.broom.ready_at = f64::INFINITY;
    p.broom.snitch_ready_at = f64::INFINITY;
    p.spell.ready_at = f64::INFINITY;
    p.study.read_ready_at = f64::INFINITY;
    p.study.bag_ready_at = f64::INFINITY;
    p
}

/// 바닥의 물건 자리
fn thing(id: u64, x: f64, giant: bool, local: bool) -> ObjectSpot {
    ObjectSpot { id, x, y: GROUND, owner: Owner::Ground(0), half_w: if giant { 300.0 } else { 24.0 }, giant, local }
}

/// 주문 하나를 끝까지 (앱처럼 물건 일은 꺼냄): 꺼낸 물건 일들
fn finish_cast(p: &mut Pet, w: &World) -> Vec<ObjectPlay> {
    let mut out = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        p.update(DT, w);
        out.extend(p.take_object_play());
        if !matches!(p.state, State::Cast { .. }) {
            break;
        }
    }
    out
}

#[test]
fn cast_animation_and_spell_lines_have_their_sheet_names() {
    assert_eq!(serde_json::to_string(&Anim::Cast).unwrap(), "\"cast\"");
    assert!(crate::manifest::SPELL_ANIMS.contains(&"cast"));
    for (s, name) in [(Speech::Expelliarmus, "expelliarmus"), (Speech::Leviosa, "leviosa"), (Speech::Fizzle, "fizzle"), (Speech::Reparo, "reparo"), (Speech::Whoa, "whoa")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// ✨ Expelliarmus: 바닥의 물건을 보는 쪽으로 휙 날림 (지팡이를 휘두르고 끝에 반짝)
#[test]
fn expelliarmus_flicks_a_floor_object_away() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Expelliarmus], false), 900.0, 1, &w);
    p.notice_play_objects(&[thing(5, 700.0, false, false)], &w);
    assert!(p.start_spell());
    assert_eq!(p.speech, Some(Speech::Expelliarmus));
    assert_eq!(p.casting(), Some(Spell::Expelliarmus));
    assert_eq!((p.render().anim, p.facing), (Anim::Cast, -1.0));
    let mut sparkled = false;
    let mut events = Vec::new();
    for _ in 0..(2.0 / DT) as usize {
        p.update(DT, &w);
        sparkled |= p.render().sparkle;
        events.extend(p.take_object_play());
    }
    assert!(sparkled);
    assert!(matches!(events.as_slice(), [ObjectPlay::Yank { id: 5, vx, vy }] if *vx < 0.0 && *vy < 0.0), "{events:?}");
    assert!(p.casting().is_none(), "끝나면 평소대로");
}

/// ✨ Expelliarmus: 물건을 든 펫이 가까이 있으면 그 펫을 먼저 노려 떨어뜨리게 함 ("hmph!")
#[test]
fn expelliarmus_knocks_the_thing_out_of_a_carrying_pets_hands() {
    let w = world(vec![]);
    let mut harry = calm(caster(&[Spell::Expelliarmus, Spell::Leviosa], false), 900.0, 2, &w);
    let mut other = resting(pet_cfg(), 700.0, 3, &w);
    other.juggle.carry = Some(42);
    other.state = State::Juggle { phase: JugglePhase::Hold { secs: 30.0 }, t: 0.0 };
    harry.social_key = 1;
    other.social_key = 2;
    let mut pets = vec![harry, other];
    assert!(pets[1].carried_object().is_some());
    notice_pets(&mut pets);
    assert!(pets[0].spell.pets.iter().any(|s| s.key == 2 && s.carrying));
    pets[0].notice_play_objects(&[thing(5, 800.0, false, false)], &w);
    assert!(pets[0].start_spell());
    assert!(matches!(pets[0].state, State::Cast { cast, .. } if cast.spell == Spell::Expelliarmus && cast.target == SpellTarget::Pet { key: 2, x: 700.0 }));
    for _ in 0..60 {
        pets[0].update(DT, &w);
        sync_lifts(&mut pets);
    }
    assert!(pets[1].carried_object().is_none(), "떨어뜨림");
    assert!(pets[0].take_object_play().is_none(), "바닥의 물건은 그대로");
}

/// ✨ Leviosa: 가까운 작은 펫이 둥둥 떠올라 (창만 — 몸은 디딤판에, 반짝이) 몇 초 뒤 내려와 평소대로. 큰 펫은 안 띄움
#[test]
fn leviosa_floats_a_small_pet_up_and_sets_it_back_down() {
    let w = world(vec![]);
    let mut hermione = calm(caster(&[Spell::Leviosa], true), 900.0, 2, &w);
    let other = resting(pet_cfg(), 700.0, 3, &w);
    hermione.social_key = 1;
    let mut pets = vec![hermione, other];
    pets[1].social_key = 2;
    pets[1].hush_stage();
    notice_pets(&mut pets);
    assert!(pets[0].start_spell());
    assert_eq!(pets[0].speech, Some(Speech::Leviosa));
    let ground_top = GROUND - pets[1].cfg.size;
    let (mut floated, mut highest): (bool, f64) = (false, 0.0);
    for _ in 0..(10.0 / DT) as usize {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
        if matches!(pets[1].state, State::Float { .. }) {
            floated = true;
            assert_eq!(pets[1].render().anim, Anim::Dragged);
            assert!(pets[1].render().sparkle);
            assert!(pets[1].standing.is_some() && pets[1].body.y == GROUND, "몸은 디딤판에");
            highest = highest.max(ground_top - pets[1].window_pos().1);
        }
    }
    assert!(floated);
    let rise = wz::FLOAT_RISE * pets[1].cfg.size;
    assert!(highest > rise * 0.9 && highest < rise * 1.1, "{highest}");
    assert!(!matches!(pets[1].state, State::Float { .. }), "내려옴");
    assert!((pets[1].window_pos().1 - ground_top).abs() < 1e-6);
    // 큰 펫은 안 띄움
    let mut h = calm(caster(&[Spell::Leviosa], true), 900.0, 2, &w);
    let big = resting(PetConfig { size: 200.0, ..pet_cfg() }, 700.0, 3, &w);
    h.social_key = 1;
    let mut pets = vec![h, big];
    pets[1].social_key = 2;
    notice_pets(&mut pets);
    assert!(!pets[0].start_spell(), "띄울 게 없음");
}

/// ✨ Leviosa로 물건을 띄움 (앱이 Object::levitate) — 해리는 가끔 "Leviosaaa?" 실패, 헤르미온느는 늘 됨
#[test]
fn harrys_leviosa_sometimes_fizzles_but_hermiones_never_does() {
    let w = world(vec![]);
    let n = 200;
    let mut fizzles = [0, 0];
    for (k, bookworm) in [false, true].into_iter().enumerate() {
        for seed in 0..n {
            let mut p = calm(caster(&[Spell::Leviosa], bookworm), 900.0, seed, &w);
            p.notice_play_objects(&[thing(5, 700.0, false, false)], &w);
            assert!(p.start_spell());
            let events = finish_cast(&mut p, &w);
            match events.as_slice() {
                [ObjectPlay::Levitate { id: 5, rise, secs }] => {
                    assert!((rise - wz::FLOAT_RISE * p.cfg.size).abs() < 1e-9);
                    assert!((wz::FLOAT_SECS.0..=wz::FLOAT_SECS.1).contains(secs));
                }
                [] => fizzles[k] += 1,
                other => panic!("{other:?}"),
            }
        }
    }
    let share = fizzles[0] as f64 / n as f64;
    assert!((wz::FIZZLE_CHANCE - 0.1..=wz::FIZZLE_CHANCE + 0.1).contains(&share), "{share}");
    assert_eq!(fizzles[1], 0);
}

/// ✨ Reparo: 바닥의 물건 하나를 반짝 치움 (앱이 뿅)
#[test]
fn reparo_tidies_a_floor_object_away() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Reparo], true), 900.0, 1, &w);
    p.notice_play_objects(&[thing(9, 1000.0, false, false)], &w);
    assert!(p.start_spell());
    assert_eq!(p.speech, Some(Speech::Reparo));
    assert_eq!(finish_cast(&mut p, &w), vec![ObjectPlay::Vanish { id: 9 }]);
}

/// 📌 장난감 아이콘·거대한 것·너무 먼 것은 노리지 않음
#[test]
fn spells_leave_toys_giants_and_far_things_alone() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Expelliarmus, Spell::Leviosa, Spell::Reparo], true), 900.0, 1, &w);
    let far = 900.0 - (wz::SPELL_REACH + 1.0) * p.cfg.size;
    p.notice_play_objects(&[thing(1, 700.0, false, true), thing(2, 1300.0, true, false), thing(3, far, false, false)], &w);
    assert!(!p.start_spell());
}

/// 가까이 물건이 있으면 몇 분에 한 번은 스스로 주문 — 주문을 모르는 동물은 안 함
#[test]
fn wizards_cast_now_and_then_on_their_own() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Leviosa, Spell::Reparo], true), 900.0, 4, &w);
    p.spell.ready_at = 0.0;
    let mut other = calm(pet_cfg(), 900.0, 4, &w);
    let mut casts = 0;
    let mut spots = vec![thing(5, 760.0, false, false)];
    for _ in 0..(60 * 60 * 15) {
        p.notice_play_objects(&spots, &w);
        other.notice_play_objects(&spots, &w);
        p.update(DT, &w);
        other.update(DT, &w);
        if let Some(ObjectPlay::Vanish { .. }) = p.take_object_play() {
            spots = vec![thing(5, 760.0, false, false)];
        }
        if let Some(s) = p.take_speech() {
            casts += matches!(s, Speech::Leviosa | Speech::Reparo) as u32;
        }
        assert!(other.casting().is_none());
    }
    assert!((4..=25).contains(&casts), "15분에 {casts}번");
}

/// 💡 "Lumos!": 지팡이를 들고 몇 초 동안 끝에 밝은 불빛 (MagicFx::Glow — 지팡이 끝, 밝아졌다 흐려짐), 맞는 때 한 번 알림 (take_lumos — 🌗 앱이 화면 밝히기)
#[test]
fn lumos_lights_the_wand_tip_for_a_few_seconds_and_tells_the_app_once() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Expelliarmus], false), 600.0, 1, &w);
    p.begin_lumos();
    assert_eq!((p.casting(), p.speech), (Some(Spell::Lumos), Some(Speech::Lumos)));
    let (mut lit, mut glow, mut told, mut peak) = (0.0, 0.0, 0, 0.0f64);
    for _ in 0..(8.0 / DT) as usize {
        p.update(DT, &w);
        if p.take_lumos() {
            told += 1;
        }
        if p.casting() == Some(Spell::Lumos) {
            lit += DT;
        }
        let mut fx = Vec::new();
        p.magic_fx(&mut fx);
        if let Some(MagicFx::Glow { x, y, size, level }) = fx.first().copied() {
            glow += DT;
            peak = peak.max(level);
            assert!(size > 0.0 && (0.0..=1.0).contains(&level));
            assert_eq!((x, y), p.wand_tip(), "지팡이 끝");
        }
    }
    assert_eq!(told, 1, "한 번만 알림");
    assert!(lit >= wz::CAST_SECS + wz::LUMOS_SECS.0 - 0.05 && lit <= wz::CAST_SECS + wz::LUMOS_SECS.1 + 0.05, "몇 초: {lit}");
    assert!(glow > wz::LUMOS_SECS.0 - 0.1 && (peak - 1.0).abs() < 1e-9, "{glow} {peak}");
    assert!(p.casting().is_none());
}

/// 💡 마법사는 한가할 때 가끔 스스로 Lumos (주문 주사위가 맞았을 때 LUMOS_CHANCE) — 주문을 모르는 펫은 안 함
#[test]
fn wizards_sometimes_cast_lumos_on_their_own() {
    let w = world(vec![]);
    let mut seen = 0;
    for seed in 0..40 {
        let mut p = calm(caster(&[Spell::Leviosa], true), 600.0, seed, &w);
        p.spell.ready_at = 0.0;
        for _ in 0..(60.0 / DT) as usize {
            p.update(DT, &w);
            if p.casting() == Some(Spell::Lumos) {
                seen += 1;
                break;
            }
        }
    }
    assert!(seen > 0, "가끔은 Lumos");
    let mut muggle = calm(pet_cfg(), 600.0, 1, &w);
    muggle.begin_lumos();
    assert!(muggle.casting().is_none(), "주문을 모르면 안 함");
}

/// 🌑 "Nox!": 지팡이 끝 불이 훅 꺼지며 (짧게 흐려지는 Glow) 작은 검은 연기 (MagicFx::Puff — 지팡이 끝, k가 0→1로 퍼짐), 맞는 때 한 번 알림
/// (take_nox — 🌗 앱이 화면 어둡게, 루모스 알림은 없음)
#[test]
fn nox_snuffs_the_wand_light_with_a_dark_puff_and_tells_the_app_once() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Expelliarmus], false), 600.0, 1, &w);
    assert!(p.begin_nox());
    assert_eq!((p.casting(), p.speech), (Some(Spell::Nox), Some(Speech::Nox)));
    let (mut told, mut puffed, mut last_k, mut last_glow, mut glow_secs) = (0, 0.0, -1.0f64, f64::INFINITY, 0.0);
    for _ in 0..(5.0 / DT) as usize {
        p.update(DT, &w);
        if p.take_nox() {
            told += 1;
        }
        assert!(!p.take_lumos(), "루모스가 아님");
        let mut fx = Vec::new();
        p.magic_fx(&mut fx);
        for f in fx {
            match f {
                MagicFx::Puff { x, y, size, k } => {
                    puffed += DT;
                    assert!(size > 0.0 && (0.0..=1.0).contains(&k) && k >= last_k, "{k}");
                    last_k = k;
                    assert_eq!((x, y), p.wand_tip(), "지팡이 끝");
                }
                MagicFx::Glow { level, .. } => {
                    glow_secs += DT;
                    assert!(level <= last_glow && level < 1.0, "꺼지기만 함");
                    last_glow = level;
                }
                _ => {}
            }
        }
    }
    assert_eq!(told, 1, "한 번만 알림");
    assert!((puffed - wz::NOX_HOLD_SECS - (wz::CAST_SECS - wz::CAST_HIT)).abs() < 0.1, "연기 {puffed}");
    assert!(glow_secs > 0.0 && glow_secs <= wz::NOX_SNUFF_SECS + 0.05, "불은 짧게 {glow_secs}");
    assert!(last_k > 0.95, "다 퍼짐");
    assert!(p.casting().is_none());
}

/// 🌑 마법사는 한가할 때 가끔 스스로 Nox (주문 주사위가 맞았을 때 NOX_CHANCE) — 주문을 모르는 펫은 안 함
#[test]
fn wizards_sometimes_cast_nox_on_their_own() {
    let w = world(vec![]);
    let mut seen = 0;
    for seed in 0..40 {
        let mut p = calm(caster(&[Spell::Leviosa], true), 600.0, seed, &w);
        p.spell.ready_at = 0.0;
        for _ in 0..(60.0 / DT) as usize {
            p.update(DT, &w);
            if p.casting() == Some(Spell::Nox) {
                seen += 1;
                break;
            }
        }
    }
    assert!(seen > 0, "가끔은 Nox");
    let mut muggle = calm(pet_cfg(), 600.0, 1, &w);
    assert!(!muggle.begin_nox());
    assert!(muggle.casting().is_none(), "주문을 모르면 안 함");
}

/// 🪶 Leviosa로 작업표시줄 앱 아이콘 띄우기: 아이콘을 만들 수 있으면 (set_icon_lift_ok) 바닥에서 "Wingardium Leviosa!" — 흐물이처럼
/// 아이콘을 달라고 하고 (JuggleEvent::Pull), 앱이 발 앞 바닥 (icon_lift_wanted)에 장난감 아이콘을 만들어 띄우면 (icon_lifted) 끝
#[test]
fn leviosa_can_pull_an_app_icon_from_the_taskbar_and_float_it() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Leviosa], true), 600.0, 1, &w);
    p.facing = 1.0;
    assert!(!p.begin_icon_lift(), "아이콘을 만들 수 없으면 안 함");
    p.set_icon_lift_ok(true);
    assert!(p.begin_icon_lift());
    assert_eq!((p.casting(), p.speech), (Some(Spell::Leviosa), Some(Speech::Leviosa)));
    assert_eq!(p.take_juggle(), Some(crate::juggle::JuggleEvent::Pull), "아이콘을 달라고 함");
    let want = p.icon_lift_wanted().expect("아이콘 자리");
    assert!(want.x > p.body.x && (want.y - p.body.y).abs() < 1e-9, "발 앞 바닥: {want:?}");
    assert!(want.rise > 0.0 && want.secs >= wz::FLOAT_SECS.0);
    p.icon_lifted(77);
    assert!(p.icon_lift_wanted().is_none());
    let plays = finish_cast(&mut p, &w);
    assert!(plays.is_empty(), "물건 일은 없음 (앱이 이미 띄움): {plays:?}");
}

/// 🪶 아이콘을 못 받으면 (OS가 못 줌·늦음) 곁의 보통 물건을 띄움
#[test]
fn leviosa_falls_back_to_a_normal_object_when_no_icon_comes() {
    let w = world(vec![]);
    let mut p = calm(caster(&[Spell::Leviosa], true), 600.0, 1, &w);
    p.notice_play_objects(&[thing(5, 700.0, false, false)], &w);
    p.set_icon_lift_ok(true);
    assert!(p.begin_icon_lift());
    p.take_juggle();
    p.icon_lift_failed();
    assert!(p.icon_lift_wanted().is_none());
    let plays = finish_cast(&mut p, &w);
    assert!(plays.iter().any(|e| matches!(e, ObjectPlay::Levitate { id: 5, .. })), "{plays:?}");

    let mut late = calm(caster(&[Spell::Leviosa], true), 600.0, 2, &w);
    late.notice_play_objects(&[thing(6, 700.0, false, false)], &w);
    late.set_icon_lift_ok(true);
    assert!(late.begin_icon_lift());
    let mut plays = Vec::new();
    for _ in 0..((wz::ICON_WAIT_SECS + 1.0) / DT) as usize {
        late.update(DT, &w);
        late.set_icon_lift_ok(false);
        plays.extend(late.take_object_play());
    }
    assert!(plays.iter().any(|e| matches!(e, ObjectPlay::Levitate { id: 6, .. })), "끝내 안 오면: {plays:?}");
    assert!(late.icon_lift_wanted().is_none());
}
