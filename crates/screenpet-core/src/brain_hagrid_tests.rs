//! 🧔 해그리드 (brain_hagrid.rs): 🐉 빗자루 대신 용 · 🏍️ 사이드카 오토바이 (붕 날기, 작은 펫 태우기) · 🤗 안아 들고 걷다 내려놓고 🖐️ 쓰담 ·
//! 🥚 용알 → 🐉 아기 용 · 🪨 락 케이크 (돌 물건)

use super::hagrid_moves::{self as hg, HagridAct, Then};
use super::hoist_moves::{notice_pets, sync_lifts};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::magic::MagicFx;
use crate::object::ObjectKind;
use crate::play::ObjectPlay;
use crate::test_util::{pet_cfg, DT};

const GROUND: f64 = 1040.0;

/// 번들 해그리드 설정 (청소년, 진짜 pet.json — 손 자리 grip 포함)
fn hagrid_cfg() -> PetConfig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/hagrid/pet.json");
    let m = crate::manifest::parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
    PetConfig::from_manifest(&m, 1.0, m.size_mult(crate::care::Stage::Teenager), "teenager")
}

/// 바닥(x)에 쉬는 해그리드 — 놀이는 테스트가 시작
fn calm(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(hagrid_cfg(), x, seed, w);
    p.hush_stage();
    p.broom.ready_at = f64::INFINITY;
    p.hagrid.ready_at = f64::INFINITY;
    p.social_key = 1;
    p
}

/// 해그리드 옆 (x)에 쉬는 작은 펫 (social_key 2)
fn buddy(x: f64, w: &World) -> Pet {
    let mut p = resting(pet_cfg(), x, 7, w);
    p.hush_stage();
    p.social_key = 2;
    p
}

/// 앱처럼 한 틱: 서로 보고, 움직이고, 들기를 맞춤
fn step(pets: &mut [Pet], w: &World) {
    notice_pets(pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets);
}

#[test]
fn hagrid_animations_and_lines_have_their_sheet_names() {
    for (a, name) in [(Anim::Dragon, "dragon"), (Anim::Sidecar, "sidecar"), (Anim::SidecarFly, "sidecar_fly"), (Anim::Hug, "hug"), (Anim::HugWalk, "hug_walk"),
        (Anim::Pat, "pat"), (Anim::Egg, "egg"), (Anim::Offer, "offer")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::HAGRID_ANIMS.contains(&name) && !a.is_front() && !a.is_back());
    }
    for (s, name) in [(Speech::HopIn, "hop_in"), (Speech::Cuddle, "cuddle"), (Speech::Pat, "pat"), (Speech::Hatch, "hatch"), (Speech::RockCake, "rock_cake")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    let c = hagrid_cfg();
    assert!(c.gamekeeper && c.dragon && c.rides_broom && c.spells.is_empty());
}

/// 🐉 빗자루 비행이 용 그림으로 (오르기·날기·내려앉기 모두)
#[test]
fn he_flies_on_a_dragon_instead_of_a_broom() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    assert!(p.is_hagrid());
    assert!(p.start_broom_flight());
    let mut seen = Vec::new();
    for _ in 0..(30.0 / DT) as usize {
        p.update(DT, &w);
        if matches!(p.state, State::Broom { .. }) {
            seen.push(p.render().anim);
        }
    }
    assert!(!seen.is_empty() && seen.iter().all(|&a| a == Anim::Dragon), "{seen:?}");
}

/// 🏍️ 사이드카: 옆의 작은 펫이 "Hop in!"에 사이드카에 앉아 (앞모습, 아래는 통에 가려짐, 버둥 없음) 함께 달리고, 끝나면 내려섬
#[test]
fn a_small_pet_hops_into_the_sidecar_and_rides_along() {
    let w = world(vec![]);
    let mut pets = vec![calm(900.0, 3, &w), buddy(1000.0, &w)];
    pets[0].facing = 1.0;
    notice_pets(&mut pets);
    // HOP_IN_CHANCE 주사위가 태울 때까지 (시드를 바꿔 가며)
    let mut seed = 3;
    loop {
        pets[0].hagrid = hg::HagridMood::new(seed);
        assert!(pets[0].start_bike());
        if matches!(pets[0].state, State::Hagrid { act: HagridAct::Bike { passenger: Some(2), .. }, .. }) {
            break;
        }
        pets[0].state = State::Idle { remaining: 100.0, look: false };
        seed += 1;
    }
    assert_eq!(pets[0].speech, Some(Speech::HopIn));
    let start_x = pets[0].body.x;
    let (mut rode, mut moved) = (0, 0.0f64);
    for _ in 0..(hg::BIKE_SECS.1 + hg::FLY_HOP_SECS + 1.0) as usize * 60 {
        step(&mut pets, &w);
        if matches!(pets[0].state, State::Hagrid { .. }) {
            assert!(matches!(pets[0].render().anim, Anim::Sidecar | Anim::SidecarFly));
            if pets[1].is_lifted() {
                rode += 1;
                let r = pets[1].render();
                assert_eq!((r.anim, r.wobble, r.seat), (Anim::SitFront, false, hg::SIDECAR_COVER));
                let ahead = (pets[1].body.x - pets[0].body.x) * pets[0].facing;
                assert!(ahead > 0.1 * pets[0].cfg.size, "사이드카는 앞쪽: {ahead}");
            }
            moved = moved.max((pets[0].body.x - start_x).abs());
        }
    }
    assert!(rode > 60 * 5, "{rode}");
    assert!(moved > pets[0].cfg.size, "달림: {moved}");
    assert!(!pets[1].is_lifted() && !matches!(pets[0].state, State::Hagrid { .. }), "끝나면 내려섬");
    for _ in 0..120 {
        step(&mut pets, &w);
    }
    assert!(pets[1].standing.is_some());
}

/// 🏍️ 붕 날기: 창이 FLY_HOP_RISE만큼 떴다가 내려오고 그동안 sidecar_fly 그림
#[test]
fn the_bike_hops_into_the_air_now_and_then() {
    let w = world(vec![]);
    let mut p = calm(900.0, 4, &w);
    p.state = State::Hagrid { act: HagridAct::Bike { len: 10.0, passenger: None, hop_at: 0.0 }, t: 0.0 };
    let top = GROUND - p.cfg.size;
    let mut highest: f64 = 0.0;
    for _ in 0..(hg::FLY_HOP_SECS / DT) as usize - 2 {
        p.update(DT, &w);
        assert_eq!(p.render().anim, Anim::SidecarFly);
        highest = highest.max(top - p.window_pos().1);
    }
    let rise = hg::FLY_HOP_RISE * p.cfg.size;
    assert!(highest > rise * 0.95 && highest <= rise + 1e-6, "{highest}");
    for _ in 0..10 {
        p.update(DT, &w);
    }
    assert_eq!(p.render().anim, Anim::Sidecar);
    assert!((p.window_pos().1 - top).abs() < 1e-6);
    // 오래 타면 저절로도 남
    let mut q = calm(900.0, 5, &w);
    assert!(q.start_bike());
    let mut flew = false;
    for _ in 0..(hg::BIKE_SECS.1 / DT) as usize {
        q.update(DT, &w);
        flew |= q.render().anim == Anim::SidecarFly;
    }
    assert!(flew);
}

/// 🤗 다가가 번쩍 안고 걷다가 살며시 내려놓고 🖐️ 쓰담 — 안긴 펫은 얌전히 (버둥·"Put me down!" 없음), 쓰다듬긴 펫에 하트
#[test]
fn he_hugs_a_pet_carries_it_sets_it_down_and_pats_it() {
    let w = world(vec![]);
    let mut pets = vec![calm(800.0, 6, &w), buddy(1100.0, &w)];
    notice_pets(&mut pets);
    assert!(pets[0].start_cuddle(&w, Then::Hug));
    let (mut held, mut patted, mut heart) = (0, false, false);
    let mut said = Vec::new();
    for _ in 0..(20.0 / DT) as usize {
        step(&mut pets, &w);
        if let Some(s) = pets[1].take_speech() {
            said.push(s);
        }
        if pets[1].is_lifted() {
            held += 1;
            let r = pets[1].render();
            assert_eq!((r.anim, r.wobble, r.seat), (Anim::SitFront, false, 0.0));
            assert!(matches!(pets[0].render().anim, Anim::Hug | Anim::HugWalk));
        }
        patted |= pets[0].render().anim == Anim::Pat;
        heart |= pets[1].render().heart;
    }
    assert!(held > 60 * 2, "{held}");
    assert!(patted && heart, "쓰담 + 하트");
    assert!(!said.contains(&Speech::PutMeDown), "{said:?}");
    assert!(pets[1].standing.is_some() && !matches!(pets[0].state, State::Hagrid { .. }));
}

/// 🏰🤗 해리·헤르미온느를 만나면 (곁 HUG_REACH에 오면) MEET_HUG_CHANCE로 곧장 안으러 감 — 평소 놀이 주사위·쉬는 때와 따로.
/// 한 번 만난 펫은 MEET_CALM_SECS 동안 다시 굴리지 않음 (안 안았어도). 다른 펫은 만나도 그냥 (평소 놀이로만)
#[test]
fn he_often_hugs_harry_or_hermione_when_they_meet_but_not_over_and_over() {
    let w = world(vec![]);
    let pal = |x: f64, w: &World| {
        let mut p = buddy(x, w);
        p.cfg.patronus = Some(crate::wizard::PatronusAnimal::Stag);
        p.cfg.size = 0.85 / 1.2 * hagrid_cfg().size;
        p
    };
    let hugging = |p: &Pet| matches!(p.state, State::Hagrid { act: HagridAct::Approach { key: 2, then: Then::Hug, .. } | HagridAct::Pick { key: 2 }, .. });
    let mut hugged = 0;
    for seed in 1..=30 {
        let mut pets = vec![calm(800.0, seed, &w), pal(1000.0, &w)];
        let mut got = false;
        for _ in 0..(2.0 / DT) as usize {
            step(&mut pets, &w);
            got |= hugging(&pets[0]);
        }
        hugged += usize::from(got);
        // 굴린 뒤엔 (안았든 말든) 한동안 또 안지 않음
        while matches!(pets[0].state, State::Hagrid { .. }) {
            step(&mut pets, &w);
        }
        for _ in 0..((hg::MEET_CALM_SECS.0 - 15.0) / DT) as usize {
            step(&mut pets, &w);
            assert!(!hugging(&pets[0]), "seed {seed}: 또 안음");
        }
    }
    let want = hg::MEET_HUG_CHANCE * 30.0;
    assert!((hugged as f64 - want).abs() <= 7.0, "{hugged}/30번 안음 (기대 {want})");
    // 다른 펫 (해리·헤르미온느가 아님)은 만나도 곧장 안지 않음
    let mut pets = vec![calm(800.0, 3, &w), buddy(1000.0, &w)];
    for _ in 0..(30.0 / DT) as usize {
        step(&mut pets, &w);
        assert!(!matches!(pets[0].state, State::Hagrid { .. }));
    }
}

/// 🖐️ 쓰담만: 다가가 쓰다듬고 하트. 곁에 펫이 없으면 시작하지 않음. 너무 큰 펫은 안지 않음
#[test]
fn he_pats_nearby_pets_and_does_not_hug_big_ones() {
    let w = world(vec![]);
    let mut alone = calm(800.0, 8, &w);
    assert!(!alone.start_cuddle(&w, Then::Pat));
    let mut pets = vec![calm(800.0, 8, &w), buddy(950.0, &w)];
    notice_pets(&mut pets);
    assert!(pets[0].start_cuddle(&w, Then::Pat));
    let mut heart = false;
    for _ in 0..(8.0 / DT) as usize {
        step(&mut pets, &w);
        heart |= pets[1].render().heart;
        assert!(!pets[1].is_lifted());
    }
    assert!(heart);
    let mut big = buddy(950.0, &w);
    big.cfg.size = pets[0].cfg.size * 1.2;
    let mut pets = vec![calm(800.0, 8, &w), big];
    notice_pets(&mut pets);
    assert!(!pets[0].start_cuddle(&w, Then::Hug));
}

/// 🥚 알을 품으면 금이 가다가 → 🐉 아기 용이 나와 날다 사라짐 (덮개 창 효과), 그 뒤 평소대로
#[test]
fn the_egg_cracks_and_a_baby_dragon_flutters_off() {
    let w = world(vec![]);
    let mut p = calm(900.0, 9, &w);
    assert!(p.start_egg());
    let State::Hagrid { act: HagridAct::Egg { len }, .. } = p.state else { panic!() };
    let (mut cracks, mut hatched, mut said) = (Vec::new(), 0usize, Vec::new());
    for _ in 0..((len + hg::HATCH_SECS + 1.0) / DT) as usize {
        p.update(DT, &w);
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        let mut fx = Vec::new();
        p.magic_fx(&mut fx);
        for f in fx {
            match f {
                MagicFx::Egg { crack, x, .. } => {
                    assert_eq!(p.render().anim, Anim::Egg);
                    assert!((x - p.body.x).abs() < p.cfg.size * 0.6, "손 자리");
                    cracks.push(crack);
                }
                MagicFx::Hatchling { t, len, .. } => {
                    assert!(t <= len);
                    hatched += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(cracks.first(), Some(&0.0));
    assert!(*cracks.last().unwrap() > 0.95 && cracks.windows(2).all(|w| w[1] >= w[0]), "금이 차오름");
    assert!(hatched as f64 > (hg::HATCH_SECS - 0.2) / DT, "{hatched}");
    assert!(said.contains(&Speech::Hatch));
    assert!(!matches!(p.state, State::Hagrid { .. }));
    assert_eq!(hg::egg_crack(0.0), 0.0);
    assert_eq!(hg::egg_crack(hg::CRACK_AT), 0.0);
    assert_eq!(hg::egg_crack(1.0), 1.0);
}

/// 🪨 "Rock cake?" — 발 앞 바닥에 돌 물건이 한 번 놓임. 손님 해그리드는 안 함
#[test]
fn he_sets_down_a_rock_cake_as_a_stone() {
    let w = world(vec![]);
    let mut p = calm(900.0, 10, &w);
    p.facing = -1.0;
    assert!(p.start_offer());
    assert_eq!(p.speech, Some(Speech::RockCake));
    let mut spawned = Vec::new();
    for _ in 0..(hg::OFFER_SECS / DT) as usize + 5 {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Hagrid { .. }) || p.render().anim == Anim::Offer);
        if let Some(ObjectPlay::Spawn { kind, x, y }) = p.take_object_play() {
            spawned.push((kind, x, y));
        }
    }
    assert_eq!(spawned.len(), 1);
    let (kind, x, y) = spawned[0];
    assert_eq!(kind, ObjectKind::Stone);
    assert!(x < p.body.x - 0.3 * p.cfg.size && y < GROUND, "발 앞 (왼쪽을 봄)");
    let mut guest = calm(900.0, 11, &w);
    guest.guest = Some(crate::visit::Guest { home: Side::Left, homesick_in: 100.0 });
    assert!(!guest.start_offer());
}

/// 한가하면 저절로: 30분씩 세 번 동안 오토바이·안기·쓰담·알·락 케이크를 골고루 (몇 분에 한 번쯤). 해그리드가 아니면 아무것도 안 함
#[test]
fn left_alone_he_does_a_bit_of_everything() {
    let w = world(vec![]);
    let mut seen = std::collections::HashSet::new();
    let mut starts = 0;
    for seed in 12..15 {
        let mut pets = vec![calm(800.0, seed, &w), buddy(1000.0, &w)];
        pets[0].hagrid.ready_at = 0.0;
        let mut was = false;
        for _ in 0..(1800.0 / DT) as usize {
            step(&mut pets, &w);
            let now = matches!(pets[0].state, State::Hagrid { .. });
            starts += usize::from(now && !was);
            was = now;
            if let State::Hagrid { act, .. } = pets[0].state {
                seen.insert(std::mem::discriminant(&act));
            }
            let _ = pets[0].take_object_play();
        }
    }
    assert!((15..=120).contains(&starts), "몇 분에 한 번쯤: {starts}");
    for act in [HagridAct::Bike { len: 0.0, passenger: None, hop_at: 0.0 }, HagridAct::Egg { len: 0.0 }, HagridAct::Offer { placed: false },
        HagridAct::Pat { key: 0 }, HagridAct::Pick { key: 0 }] {
        assert!(seen.contains(&std::mem::discriminant(&act)), "{act:?}");
    }
    let mut other = calm(800.0, 12, &w);
    other.cfg.gamekeeper = false;
    other.hagrid.ready_at = 0.0;
    for _ in 0..(300.0 / DT) as usize {
        other.update(DT, &w);
        assert!(!matches!(other.state, State::Hagrid { .. }));
    }
}
