//! 새 (빠른 나는 동물, pet.json `swift`) — 꿀벌보다 빠르게 날고, 더 자주 창 위·바닥에 내려앉아 쉬고,
//! 친구 화면으로 갈 땐 멀리서부터 "whoosh!" 하고 쌩 날아 나간다 (들어올 때도 쌩). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{place, run, sim_cfg, w1, world};
use super::*;
use crate::flight::{self, Goal};
use crate::test_util::{DT, flyer, idle, run_until};

/// 새: 날고, 빠르다 (쏘지 않는다)
fn bird() -> PetConfig {
    PetConfig { flies: true, swift: true, ..crate::test_util::pet_cfg() }
}

fn sim_bird() -> PetConfig {
    PetConfig { flies: true, swift: true, walk_speed: 110.0, ..sim_cfg() }
}

fn hovering(cfg: PetConfig, x: f64, y: f64, seed: u64) -> Pet {
    let mut p = Pet::new(cfg, x, y, seed);
    p.state = idle();
    p
}

/// 화면 오른쪽 끝(친구가 연결됨) 쪽으로 날다가 나가기로 정하면 그 순간의 펫 (그 틱의 말풍선 포함). 안 나가면 None
fn exit_decision(cfg: PetConfig, from_x: f64, seed: u64) -> Option<(Pet, Option<Speech>)> {
    let w = world(vec![]);
    let mut p = hovering(cfg, from_x, 500.0, seed);
    p.set_exit_edges(false, true);
    p.start_flight(Goal::Point(1882.0, 500.0), Move::Walk);
    for _ in 0..(60 * 60) {
        p.update(DT, &w);
        let said = p.take_speech();
        if matches!(p.state, State::Exit { .. }) {
            return Some((p, said));
        }
        if p.flight.is_none() {
            return None;
        }
    }
    None
}

#[test]
fn bundled_bird_is_a_swift_flyer_faster_than_the_ghost() {
    let all = crate::test_util::bundled();
    let get = |id: &str| all.iter().find(|(s, _)| s == id).map(|(_, c)| *c).unwrap_or_else(|| panic!("no {id}"));
    let (bird, ghost) = (get("bird"), get("ghost"));
    assert!(bird.flies && bird.swift, "{bird:?}");
    assert!(!ghost.swift);
    // 나는 동물 중 가장 빠르다 (화면 속도 = pet.json walk_speed × √몸 배율)
    assert!(bird.walk_speed > 1.5 * ghost.walk_speed, "bird {} ghost {}", bird.walk_speed, ghost.walk_speed);
    assert!(all.iter().filter(|(id, c)| c.flies && id != "bird").all(|(_, c)| c.walk_speed < bird.walk_speed));
}

#[test]
fn swift_flyers_go_perch_more_often_than_the_bee() {
    let w = world(vec![w1()]);
    let n = 2000;
    let mut perches = 0;
    for seed in 0..n {
        let mut p = hovering(bird(), 800.0, 300.0, seed);
        let next = p.rest_state(&w);
        if matches!(p.flight, Some(f) if matches!(f.goal, Goal::Perch { .. })) {
            assert!(matches!(next, State::Move { .. }));
            perches += 1;
        }
    }
    let share = perches as f64 / n as f64;
    let want = flight::SWIFT_PERCH_CHANCE;
    assert!(want > flight::PERCH_CHANCE + 0.2, "{want}");
    assert!((want - 0.04..=want + 0.04).contains(&share), "{share}");
}

#[test]
fn hovering_bird_flutters_and_a_perched_bird_sits() {
    let w = world(vec![w1()]);
    // 떠서 쉬면 파닥파닥 (날기 그림) — 앉은 그림으로 공중에 떠 있지 않는다
    let mut p = hovering(bird(), 800.0, 300.0, 1);
    p.update(DT, &w);
    assert!(p.standing.is_none() && matches!(p.state, State::Idle { .. }), "{:?}", p.state);
    assert_eq!(p.render().anim, Anim::Walk);
    // 창 위에 내려앉아 쉬면 앉은 그림
    let mut p = Pet::new(bird(), 800.0, 400.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.update(DT, &w);
    assert!(p.standing.is_some() && matches!(p.state, State::Idle { .. }), "{:?}", p.state);
    assert_eq!(p.render().anim, Anim::Idle);
    // 꿀벌은 그대로 (떠 있는 그림이 idle)
    let mut b = hovering(flyer(), 800.0, 300.0, 1);
    b.update(DT, &w);
    assert_eq!(b.render().anim, Anim::Idle);
}

#[test]
fn perched_bird_sits_a_while_then_takes_off_again() {
    let w = world(vec![w1()]);
    let mut seen_takeoff = false;
    for seed in 0..6 {
        let mut p = Pet::new(sim_bird(), 800.0, 400.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: LAND_SECS });
        // 내려앉은 뒤 적어도 몇 초는 앉아 있다
        run(&mut p, &w, 60 * 2);
        assert!(p.standing.is_some(), "seed {seed}: {:?}", p.state);
        if run_until(&mut p, &w, 60 * 120, |p| p.standing.is_none()) {
            seen_takeoff = true;
        }
    }
    assert!(seen_takeoff);
}

#[test]
fn bird_decides_to_leave_from_farther_away_and_dashes_out_with_a_whoosh() {
    let bird_cfg = sim_bird();
    let exits: Vec<(Pet, Option<Speech>)> = (0..200).filter_map(|seed| exit_decision(bird_cfg, 700.0, seed)).collect();
    assert!(exits.len() >= 30, "{}", exits.len());
    for (p, said) in &exits {
        let gap = 1920.0 - (p.body.x + p.cfg.half_w);
        // 꿀벌은 끝에서 몸 하나 안쪽에서 정하지만, 새는 몸 몇 개 떨어진 데서부터 쌩
        assert!(gap > 2.0 * p.cfg.size, "gap {gap}");
        assert_eq!(*said, Some(Speech::Whoosh));
        assert_eq!(p.render().anim, Anim::Run);
    }
    // 나가는 속도: 보통 날기의 SWIFT_DASH배, 높이는 그대로
    let w = world(vec![]);
    let (mut p, _) = exits.into_iter().next().unwrap();
    let (x0, y0) = (p.body.x, p.body.y);
    p.update(DT, &w);
    let v = (p.body.x - x0) / DT;
    assert!((v - p.cfg.walk_speed * flight::SWIFT_DASH).abs() < 1.0, "v {v}");
    assert!(flight::SWIFT_DASH >= 3.0);
    assert_eq!(p.body.y, y0);
    assert!(run_until(&mut p, &w, 60 * 5, |p| p.is_away()));
    assert_eq!(p.take_exit(), Some(Side::Right));
}

#[test]
fn a_plain_flyer_still_decides_near_the_edge_and_flies_out_at_its_usual_pace() {
    let exits: Vec<(Pet, Option<Speech>)> = (0..200).filter_map(|seed| exit_decision(super::tests::sim_flyer(), 700.0, seed)).collect();
    assert!(!exits.is_empty());
    let w = world(vec![]);
    for (p, said) in &exits {
        assert!(1920.0 - (p.body.x + p.cfg.half_w) <= flight::EDGE_ZONE * p.cfg.size + 1.0);
        assert_ne!(*said, Some(Speech::Whoosh));
        assert_eq!(p.render().anim, Anim::Walk);
    }
    let (mut p, _) = exits.into_iter().next().unwrap();
    let x0 = p.body.x;
    p.update(DT, &w);
    assert!(((p.body.x - x0) / DT - p.cfg.walk_speed).abs() < 1.0);
}

#[test]
fn bird_dashes_in_from_a_friends_screen_with_a_whoosh() {
    let w = world(vec![w1()]);
    let mut p = Pet::new_entering(sim_bird(), &w, Side::Right, 3);
    assert_eq!(p.take_speech(), Some(Speech::Whoosh));
    assert_eq!(p.render().anim, Anim::Run);
    let x0 = p.body.x;
    p.update(DT, &w);
    assert!(((x0 - p.body.x) / DT - p.cfg.walk_speed * flight::SWIFT_DASH).abs() < 1.0);
    assert!(run_until(&mut p, &w, 60 * 5, |p| !matches!(p.state, State::Enter { .. })));
    assert!(p.body.x + p.cfg.half_w <= 1920.0);
    // 꿀벌은 그대로 천천히, 말없이
    let mut b = Pet::new_entering(flyer(), &w, Side::Right, 3);
    assert_eq!(b.take_speech(), None);
    assert_eq!(b.render().anim, Anim::Walk);
}

#[test]
fn homesick_guest_bird_whooshes_home() {
    let w = world(vec![w1()]);
    for seed in 0..10 {
        let mut p = Pet::new_entering(sim_bird(), &w, Side::Left, seed);
        p.set_guest(Side::Left, 3.0);
        assert!(run_until(&mut p, &w, 60 * 90, |p| p.is_away()), "seed {seed}: {:?} {:?}", p.state, p.body);
        assert_eq!(p.take_exit(), Some(Side::Left));
    }
}

#[test]
fn fed_bird_lands_and_pecks_at_its_bowl() {
    use crate::care::{FeedResult, Food};
    let w = world(vec![w1()]);
    let mut p = hovering(sim_bird(), 800.0, 300.0, 4);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Favorite));
    assert!(run_until(&mut p, &w, 60 * 20, |p| matches!(p.state, State::Eating { .. })), "{:?}", p.state);
    assert!(p.standing.is_some());
    assert_eq!(p.render().anim, Anim::Eat);
}
