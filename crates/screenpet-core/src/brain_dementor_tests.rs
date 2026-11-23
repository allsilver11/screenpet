//! 👻 디멘터 (brain_dementor.rs): 곁의 펫이 시무룩 (annoyed 그림, "brrr...") + 배가 더 고픔 · 가까운 화면 구석에 🥶 서리가 끼고 녹음 ·
//! 🦌 패트로누스에 쫓기면 반대쪽 위 구석으로 날아가 한동안 얌전 · 가끔 다른 펫 머리 위로 다가감 ·
//! 👻 곁의 펫 영혼 빼앗기 (머리 위에 떠서 은빛 영혼 줄기 → 펫이 약 3초 기절)

use super::dementor_moves::{sync_drain, sync_gloom};
use super::hoist_moves::{notice_pets, sync_lifts};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::geom::WindowInfo;
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

/// 👻 낮게 붙은 옆 모니터 (위쪽이 빈 틈)의 창 위 펫에게 다가가도 그 모니터 안에 뜸 — 합친 화면 위쪽 틈에 떠서 안 보이던 버그 (qa_sim seed 4)
#[test]
fn drifting_over_a_pet_stays_on_its_monitor_below_a_gap() {
    let low = Rect::new(1920.0, 180.0, 3200.0, 980.0);
    let win = WindowInfo { id: 1, rect: Rect::new(2300.0, 240.0, 2900.0, 700.0) };
    let w = World::new(vec![win], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), low], 128.0);
    let mut d = floating(2600.0, 600.0, 7);
    d.social_key = 1;
    let mut pet = Pet::new(pet_cfg(), 2600.0, 0.0, 8);
    let (ax, ay) = w.anchor(Owner::Window(1)).unwrap();
    pet.standing = Some(Standing { owner: Owner::Window(1), rel_x: 2600.0 - ax });
    pet.body = Body { x: 2600.0, y: ay, vx: 0.0, vy: 0.0 };
    pet.state = State::Idle { remaining: 100.0, look: false };
    pet.social_key = 2;
    let mut pets = vec![d, pet];
    notice_pets(&mut pets);
    let mut hits = 0;
    for _ in 0..200 {
        if let Some((x, y)) = pets[0].stalk_target(&w) {
            assert!(x >= low.left && x <= low.right, "{x}");
            assert!(y - pets[0].cfg.size >= low.top, "그림이 모니터 위 틈에 걸림: 발 {y}");
            hits += 1;
        }
    }
    assert!(hits > 0);
}

/// 👻 시무룩한 채 (제자리 상태) 서 있던 창을 작업표시줄 뒤·화면 아래로 끌어 내리면 같이 실려 가지 않고 떨어짐 — 디딤판이 사라졌는데
/// 창 자리만 따라가 화면 밖에 선 채 남던 버그 (qa_sim seed 100)
#[test]
fn a_gloomy_pet_does_not_ride_a_window_dragged_off_the_screen() {
    let mut rect = Rect::new(800.0, 600.0, 1400.0, 1000.0);
    let w = World::new(vec![WindowInfo { id: 1, rect }], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let mut p = Pet::new(pet_cfg(), 1000.0, 0.0, 9);
    p.standing = Some(Standing { owner: Owner::Window(1), rel_x: 200.0 });
    p.body = Body { x: 1000.0, y: 600.0, vx: 0.0, vy: 0.0 };
    p.state = State::Gloom { remaining: 3.0 };
    p.update(DT, &w);
    assert!(p.standing.is_some());
    rect = Rect::new(800.0, 1060.0, 1400.0, 1460.0);
    let below = World::new(vec![WindowInfo { id: 1, rect }], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    p.update(DT, &below);
    assert!(p.standing.map_or(true, |s| s.owner != Owner::Window(1)), "{:?} {:?}", p.state, p.standing);
    assert!(p.body.y <= 1040.0, "화면 아래로 실려 감: {}", p.body.y);
}

/// 🧹 빗자루로 서리를 쓸면 그 구석 서리가 바로 녹고, 디멘터가 곁에 있어도 FROST_SWEPT_SECS 동안 다시 끼지 않음
/// (거미줄처럼 보이는 서리가 빗자루로 안 지워지던 문제, 2026-11-23)
#[test]
fn sweeping_frost_with_the_broom_melts_it_for_a_while() {
    use crate::broom::Broom;
    let w = world(vec![]);
    let mut d = floating(250.0, 260.0, 4);
    let run = |d: &mut Pet, secs: f64| {
        for _ in 0..(secs / DT) as usize {
            d.dementor_tick(DT, &w);
            d.clock += DT;
        }
    };
    run(&mut d, 8.0);
    assert_eq!(d.frost().len(), 1);
    // 멀리서 쓸면 그대로
    let mut far = Broom::new((900.0, 600.0));
    far.tick(DT, (960.0, 600.0), false, false, false);
    assert!(!d.sweep_frost(&far, 1.0));
    assert_eq!(d.frost().len(), 1);
    // 구석을 쓸고 지나감
    let mut b = Broom::new((10.0, 30.0));
    b.tick(DT, (60.0, 30.0), false, false, false);
    assert!(d.sweep_frost(&b, 1.0));
    assert!(d.frost().is_empty());
    run(&mut d, wz::FROST_SWEPT_SECS - 2.0);
    assert!(d.frost().is_empty(), "곁에 있어도 한동안 다시 안 낌");
    run(&mut d, 6.0);
    assert!(!d.frost().is_empty(), "그 뒤엔 다시 낌");
}

/// 👻 영혼 빼앗기용: 900에 쉬는 펫 (key 1)과 그 머리 위 가까이 뜬 디멘터 (key 2, seed)
fn drain_pair(seed: u64, w: &World) -> Vec<Pet> {
    let mut v = resting(pet_cfg(), 900.0, 1, w);
    v.hush_stage();
    v.social_key = 1;
    let mut d = floating(950.0, 980.0, seed);
    d.social_key = 2;
    vec![v, d]
}

/// 👻 첫 만남에 영혼 빼앗기를 시작하는 디멘터 시드
fn draining_seed(w: &World) -> u64 {
    (0..60).find(|&s| {
        let mut p = drain_pair(s, w);
        sync_drain(&mut p);
        p[1].soul_draining() == Some(1)
    })
    .expect("어느 시드는 시작")
}

/// 👻 곁의 펫을 만나면 꽤 자주 (DRAIN_CHANCE) 영혼을 빼앗으려 함 — 먼 펫·나는 펫·도망 중인 디멘터는 아님, 같은 펫은 곧바로 다시 굴리지 않음
#[test]
fn a_dementor_often_tries_to_drain_a_pet_it_meets() {
    let w = world(vec![]);
    let started = (0..60)
        .filter(|&s| {
            let mut p = drain_pair(s, &w);
            sync_drain(&mut p);
            p[1].soul_draining() == Some(1) && p[0].soul_drained()
        })
        .count();
    assert!((25..=50).contains(&started), "꽤 자주: {started}/60");
    let seed = draining_seed(&w);
    // 먼 펫
    let mut p = drain_pair(seed, &w);
    p[0] = resting(pet_cfg(), 200.0, 1, &w);
    p[0].social_key = 1;
    sync_drain(&mut p);
    assert!(p[1].soul_draining().is_none() && !p[0].soul_drained());
    // 나는 펫 (기절할 수 없음)
    let mut p = drain_pair(seed, &w);
    let mut bird = Pet::new(crate::test_util::flyer(), 920.0, 1000.0, 1);
    bird.state = State::Idle { remaining: 100.0, look: false };
    bird.social_key = 1;
    p[0] = bird;
    sync_drain(&mut p);
    assert!(p[1].soul_draining().is_none());
    // 도망 중인 디멘터
    let mut p = drain_pair(seed, &w);
    assert!(p[1].be_repelled(0.0));
    sync_drain(&mut p);
    assert!(p[1].soul_draining().is_none());
    // 안 된 주사위도 그 펫은 한동안 다시 굴리지 않음
    let miss = (0..60)
        .find(|&s| {
            let mut p = drain_pair(s, &w);
            sync_drain(&mut p);
            p[1].soul_draining().is_none()
        })
        .unwrap();
    let mut p = drain_pair(miss, &w);
    for _ in 0..(20.0 / DT) as usize {
        sync_drain(&mut p);
        p[1].clock += DT;
    }
    assert!(p[1].soul_draining().is_none(), "한 만남에 한 번");
}

/// 👻 영혼을 빼앗기는 펫: 머리 위에 디멘터가 떠 있고 ("hhhaaah..."), 펫은 버둥 ("my soul...!" — dragged 그림·떨림·살짝 들림),
/// 펫에서 디멘터로 은빛 영혼 줄기 (MagicFx::Soul) DRAIN_SECS → 펫이 DRAIN_FAINT_SECS 기절했다 일어남, 곧바로 또 빼앗기지 않음
#[test]
fn the_drained_pet_struggles_under_the_dementor_then_faints_for_three_seconds() {
    let w = world(vec![]);
    let mut pets = drain_pair(draining_seed(&w), &w);
    sync_lifts(&mut pets);
    assert!(pets[0].soul_drained() && pets[1].soul_draining() == Some(1));
    assert_eq!(pets[0].take_speech(), Some(Speech::SoulTaken));
    assert_eq!(pets[1].take_speech(), Some(Speech::SoulDrain));
    let ground = pets[0].window_pos().1;
    let (mut soul_for, mut fainted_at, mut faint_for, mut hovered, mut rose) = (0.0, None, 0.0, false, false);
    for i in 0..(9.0 / DT) as usize {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
        let mut fx = Vec::new();
        pets[1].magic_fx(&mut fx);
        if let Some(MagicFx::Soul { from, to, width, level, .. }) = fx.iter().copied().find(|f| matches!(f, MagicFx::Soul { .. })) {
            soul_for += DT;
            assert!(to.1 < from.1 && width > 0.0 && level > 0.0 && level <= 1.0, "{fx:?}");
        }
        if pets[0].soul_drained() {
            let r = pets[0].render();
            assert!(r.anim == Anim::Dragged && r.wobble, "{r:?}");
            rose |= pets[0].window_pos().1 < ground - 3.0;
            if i as f64 * DT > 1.2 {
                let size = pets[0].cfg.size;
                hovered |= (pets[1].body.x - pets[0].body.x).abs() < 0.3 * size && pets[1].body.y < pets[0].body.y - size;
            }
        }
        if pets[0].is_fainted() {
            faint_for += DT;
            fainted_at.get_or_insert(i as f64 * DT);
        }
    }
    assert!(hovered && rose, "머리 위에 떠 있고 펫은 살짝 들림");
    let at = fainted_at.expect("기절");
    assert!((at - wz::DRAIN_SECS).abs() < 0.15, "{at}");
    assert!((soul_for - wz::DRAIN_SECS).abs() < 0.15, "{soul_for}");
    assert!((faint_for - wz::DRAIN_FAINT_SECS).abs() < 0.15, "{faint_for}");
    assert!(!pets[0].is_fainted() && !pets[0].soul_drained() && pets[1].soul_draining().is_none());
}

/// 👻 빼앗는 중에 디멘터를 집어 들면 그만 — 펫은 기절하지 않고 평소대로
#[test]
fn grabbing_the_dementor_stops_the_drain() {
    let w = world(vec![]);
    let mut pets = drain_pair(draining_seed(&w), &w);
    sync_lifts(&mut pets);
    for _ in 0..(0.5 / DT) as usize {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
    }
    pets[1].state = State::Dragged;
    for _ in 0..(3.0 / DT) as usize {
        pets[0].update(DT, &w);
        sync_lifts(&mut pets);
        assert!(!pets[0].is_fainted());
    }
    assert!(!pets[0].soul_drained() && pets[1].soul_draining().is_none());
}

/// 🥶 디멘터에서 다른 동물로 바꾸면 (앱이 cfg만 갈아 끼움) 남은 서리·영혼 줄기는 바로 사라짐 — 디멘터가 없는데 구석에 성에가 계속 끼던 버그 (2026-11-23)
#[test]
fn frost_goes_away_when_the_pet_is_no_longer_a_dementor() {
    let w = world(vec![]);
    let mut d = floating(250.0, 260.0, 4);
    for _ in 0..(8.0 / DT) as usize {
        d.dementor_tick(DT, &w);
        d.clock += DT;
    }
    assert!(!d.frost().is_empty());
    d.cfg = pet_cfg();
    d.dementor_tick(DT, &w);
    assert!(d.frost().is_empty());
    let mut fx = Vec::new();
    d.magic_fx(&mut fx);
    assert!(!fx.iter().any(|f| matches!(f, MagicFx::Frost { .. } | MagicFx::Soul { .. })), "{fx:?}");
}

/// 👻 집중 중이거나 자리 비움인 펫·디멘터는 영혼 빼앗기를 하지 않음
#[test]
fn no_drain_while_focusing() {
    let w = world(vec![]);
    let seed = draining_seed(&w);
    let mut p = drain_pair(seed, &w);
    p[0].set_focus(true);
    sync_drain(&mut p);
    assert!(p[1].soul_draining().is_none() && !p[0].soul_drained());
    let mut p = drain_pair(seed, &w);
    p[1].set_focus(true);
    sync_drain(&mut p);
    assert!(p[1].soul_draining().is_none());
}
