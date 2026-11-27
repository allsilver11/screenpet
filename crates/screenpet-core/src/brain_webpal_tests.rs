//! 🕷️🤝 거미인간과 친구 (brain_webpal.rs): 🌉🤝 그네에 친구 태우기 · 🕸️🎣 구석 거미집으로 친구 낚아채기

use super::hoist_moves::{notice_pets, sync_lifts};
use super::snare_moves::StuckKind;
use super::swing_tests::resting;
use super::tests::world;
use super::web_moves::WebPhase;
use super::webpal_moves::{RIDE_CHANCE, SNATCH_CALM};
use super::*;
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};
use crate::web::WebSpot;

/// 번들 pet.json 그대로의 거미인간
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

/// 바닥(x)에 쉬는 거미인간 — 주사위 놀이는 꺼 둠 (start_*로만)
fn calm_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(spider(), x, seed, w);
    p.web.web_ready_at = f64::INFINITY;
    p.web.swing_ready_at = f64::INFINITY;
    p.web.pull_ready_at = f64::INFINITY;
    p.web.cocoon_ready_at = f64::INFINITY;
    p.web.play_ready_at = f64::INFINITY;
    p.web.hang_ready_at = f64::INFINITY;
    p.web.pal.snatch_ready_at = f64::INFINITY;
    p.web.orb_odds = 0.0;
    p
}

/// 거미인간과 그 왼쪽 gap(몸 크기 배)에 쉬는 고양이
fn spider_and_cat(gap: f64, seed: u64, w: &World) -> Vec<Pet> {
    let size = spider().size;
    let mut pets = vec![calm_spider(900.0, seed, w), resting(pet_cfg(), 900.0 - gap * size, seed + 1, w)];
    pets[1].state = State::Idle { remaining: 1000.0, look: false };
    pets
}

/// 앱처럼 한 틱: 다른 동물 보여 주기 → update → 동물 사이 일. 새 거미집은 webs에 쌓아 모두에게 알려 줌
fn step_all(pets: &mut [Pet], w: &World, webs: &mut Vec<WebSpot>) {
    notice_pets(pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    sync_lifts(pets);
    for p in pets.iter_mut() {
        if let Some(s) = p.take_web() {
            webs.push(WebSpot { id: webs.len() as u64 + 1, x: s.x, y: s.y, corner: s.corner, size: s.size, hidden: false });
        }
        if let Some(id) = p.take_torn_web() {
            webs.retain(|w| w.id != id);
        }
    }
    for p in pets.iter_mut() {
        p.set_webs(webs);
    }
}

/// 그네 줄에 매달려 있음
fn swinging(p: &Pet) -> bool {
    matches!(p.state, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. })
}

#[test]
fn a_swinging_spider_can_carry_a_nearby_friend_who_lets_go_at_the_end() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(1.5, 3, &w);
    pets[0].facing = 1.0;
    pets[0].web.pal.ride_odds = 1.0;
    notice_pets(&mut pets);
    assert!(pets[0].start_web_swing(&w));
    let (size, start_x) = (pets[0].cfg.size, pets[1].body.x);
    let mut webs = Vec::new();
    let mut rode = false;
    let mut guard = LoopLimit::new();
    while !matches!(pets[0].state, State::Air { .. }) {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
        if swinging(&pets[0]) {
            assert!(pets[1].is_lifted(), "그네 내내 매달림 {:?}", pets[1].state);
            let d = (pets[1].body.x - pets[0].body.x).hypot(pets[1].body.y - pets[0].body.y);
            assert!(d < 1.5 * size, "거미인간 곁 {d}");
            rode = true;
        }
    }
    assert!(rode);
    step_all(&mut pets, &w, &mut webs);
    assert!(!pets[1].is_lifted(), "그네 끝에서 놓음");
    assert!(matches!(pets[1].state, State::Air { .. }));
    assert!(pets[1].body.vx > 0.0, "그네 속도 그대로 날아감 {}", pets[1].body.vx);
    while pets[1].standing.is_none() {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
    }
    assert!(pets[1].body.x - start_x > 3.0 * size, "같이 멀리 감 {start_x} → {}", pets[1].body.x);
}

#[test]
fn far_pets_never_ride_and_only_some_swings_take_a_friend() {
    let w = world(vec![]);
    let mut far = spider_and_cat(8.0, 5, &w);
    far[0].web.pal.ride_odds = 1.0;
    notice_pets(&mut far);
    assert!(far[0].start_web_swing(&w));
    let mut webs = Vec::new();
    for _ in 0..(60 * 4) {
        step_all(&mut far, &w, &mut webs);
        assert!(!far[1].is_lifted());
    }
    let mut rides = 0;
    for seed in 0..200 {
        let mut pets = spider_and_cat(1.5, 10 + seed, &w);
        notice_pets(&mut pets);
        assert!(pets[0].start_web_swing(&w));
        step_all(&mut pets, &w, &mut webs);
        rides += usize::from(pets[1].is_lifted());
    }
    let share = rides as f64 / 200.0;
    assert!((share - RIDE_CHANCE).abs() < 0.12, "{share}");
}

#[test]
fn snatching_the_ridden_friend_mid_swing_just_lets_it_go() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(1.5, 7, &w);
    pets[0].facing = 1.0;
    pets[0].web.pal.ride_odds = 1.0;
    notice_pets(&mut pets);
    assert!(pets[0].start_web_swing(&w));
    let mut webs = Vec::new();
    let mut guard = LoopLimit::new();
    while !swinging(&pets[0]) {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
    }
    let (x, y) = (pets[1].body.x, pets[1].body.y);
    pets[1].pointer_down(x, y);
    pets[1].pointer_move(x + 30.0, y);
    for _ in 0..10 {
        step_all(&mut pets, &w, &mut webs);
        assert!(!pets[1].is_lifted());
    }
    assert!(pets[0].web.pal.rider.is_none(), "태운 친구를 잊음");
}

#[test]
fn it_spins_a_corner_web_then_yanks_a_friend_into_it_where_it_hangs_until_it_wriggles_free() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(3.0, 11, &w);
    notice_pets(&mut pets);
    let target = pets[0].web.pets.iter().find(|s| s.key == pets[1].social_key).copied().expect("고양이를 봄");
    assert!(pets[0].start_snatch(&target, &w));
    let mut webs = Vec::new();
    let mut guard = LoopLimit::new();
    let mut flew = false;
    while !pets[1].is_stuck() {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
        flew |= matches!(pets[1].state, State::Air { .. });
    }
    assert!(flew, "줄에 끌려 날아감");
    assert!(matches!(pets[1].state, State::Stuck { kind: StuckKind::Snare { .. }, .. }), "거미집에 걸림 {:?}", pets[1].state);
    assert_eq!(webs.len(), 1);
    let wb = webs[0];
    let center = (pets[1].body.x, pets[1].body.y - 0.5 * pets[1].cfg.size);
    assert!(crate::web::in_web(center.0, center.1, wb.x, wb.y, wb.corner, wb.size) || crate::web::in_web(pets[1].body.x, pets[1].body.y - 1.0, wb.x, wb.y, wb.corner, wb.size));
    while pets[1].is_stuck() {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
    }
    assert!(webs.is_empty(), "찢고 빠져나옴");
}

#[test]
fn left_alone_near_a_pet_it_snatches_now_and_then_with_a_long_cooldown_but_never_alone() {
    let w = world(vec![]);
    let mut pets = spider_and_cat(3.0, 21, &w);
    pets[0].web.pal.snatch_ready_at = 0.0;
    let mut webs = Vec::new();
    let mut guard = LoopLimit::new();
    while pets[0].web.pal.snatch.is_none() {
        guard.step();
        step_all(&mut pets, &w, &mut webs);
    }
    assert!(pets[0].web.pal.snatch_ready_at >= pets[0].clock + SNATCH_CALM.0 - 1.0, "몇 분 쉼");
    let mut alone = vec![calm_spider(900.0, 22, &w)];
    alone[0].web.pal.snatch_ready_at = 0.0;
    for _ in 0..(60 * 120) {
        step_all(&mut alone, &w, &mut webs);
        assert!(alone[0].web.pal.snatch.is_none());
    }
}
