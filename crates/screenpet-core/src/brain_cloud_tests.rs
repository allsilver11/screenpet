//! 눈사람의 눈구름 (brain_cloud.rs): 몇 분에 한 번, 쉬는 박자에 10~20초 — 날씨와 상관없이, 집·드래그·떠나는 길엔 없다.

use super::cloud_moves::{CLOUD_FIRST_WAIT_SECS, CLOUD_SECS, CLOUD_WAIT_SECS};
use super::tests::{cfg, place, sim_cfg, w1, win, world};
use super::*;
use crate::test_util::idle;

const DT: f64 = 1.0 / 30.0;

fn snowman() -> PetConfig {
    PetConfig { snow_cloud: true, looks_up: true, ..sim_cfg() }
}

/// 넓은 바닥에서 secs초 사는 동안 구름이 뜬 구간들 (시작 시각, 길이)
fn cloud_spans(c: PetConfig, seed: u64, secs: f64) -> Vec<(f64, f64)> {
    let w = world(vec![win(1, 0.0, 400.0, 1920.0, 900.0)]);
    let mut p = Pet::new(c, 960.0, 300.0, seed);
    let mut spans = Vec::new();
    let mut open: Option<f64> = None;
    let mut t = 0.0;
    while t < secs {
        p.update(DT, &w);
        match (p.render().cloud, open) {
            (true, None) => open = Some(t),
            (false, Some(s)) => {
                spans.push((s, t - s));
                open = None;
            }
            _ => {}
        }
        t += DT;
    }
    spans
}

#[test]
fn the_cloud_comes_every_few_minutes_for_ten_to_twenty_seconds() {
    for seed in 0..4 {
        let spans = cloud_spans(snowman(), seed, 3600.0);
        assert!((6..=20).contains(&spans.len()), "{} clouds in an hour", spans.len());
        assert!(spans[0].0 >= CLOUD_FIRST_WAIT_SECS.0 - 1.0, "first cloud at {:.0}s", spans[0].0);
        for &(_, secs) in &spans {
            assert!((CLOUD_SECS.0 - 0.2..=CLOUD_SECS.1 + 0.2).contains(&secs), "lasted {secs:.1}s");
        }
        for pair in spans.windows(2) {
            let gap = pair[1].0 - (pair[0].0 + pair[0].1);
            assert!(gap >= CLOUD_WAIT_SECS.0 - 1.0, "gap {gap:.0}s");
        }
    }
}

#[test]
fn other_pets_never_get_a_cloud_and_their_dice_stay_the_same() {
    assert!(cloud_spans(PetConfig { looks_up: true, ..sim_cfg() }, 3, 1200.0).is_empty());
    // 구름 틱이 난수를 쓰지 않는다: 같은 씨앗이면 끝까지 똑같이 움직인다
    let mut p = Pet::new(sim_cfg(), 960.0, 300.0, 9);
    let before = p.rng.clone();
    p.tick_snow_cloud(1.0);
    assert_eq!(p.rng.clone().u64(..), before.clone().u64(..));
}

/// 쉬는 중인 눈사람의 구름 시계를 끝까지 돌린다 — 구름이 뜨고 올려다본다
fn under_cloud() -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(snowman(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.cloud.wait = Some(0.0);
    p.tick_snow_cloud(DT);
    assert!(p.render().cloud, "{:?}", p.state);
    (p, w)
}

#[test]
fn the_snowman_looks_up_when_the_cloud_arrives() {
    let (p, _) = under_cloud();
    assert_eq!(p.render().anim, Anim::LookUp);
}

#[test]
fn the_cloud_waits_for_an_idle_beat() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(snowman(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Move { kind: Move::Walk, remaining: 100.0 });
    p.cloud.wait = Some(0.0);
    p.tick_snow_cloud(DT);
    assert!(!p.render().cloud);
    p.state = State::Idle { remaining: 5.0, look: false };
    p.tick_snow_cloud(DT);
    assert!(p.render().cloud);
}

#[test]
fn the_cloud_keeps_following_while_the_snowman_walks() {
    let (mut p, w) = under_cloud();
    p.gaze_up = false;
    p.state = State::Move { kind: Move::Walk, remaining: 3.0 };
    for _ in 0..30 {
        p.update(DT, &w);
    }
    assert!(p.render().cloud);
}

#[test]
fn the_cloud_hides_while_dragged_and_does_not_come_back_right_away() {
    let (mut p, _) = under_cloud();
    p.state = State::Dragged;
    assert!(!p.render().cloud, "gone in the very same frame");
    p.tick_snow_cloud(DT);
    p.state = State::Idle { remaining: 5.0, look: false };
    p.tick_snow_cloud(DT);
    assert!(!p.render().cloud);
}

#[test]
fn the_cloud_hides_in_the_house_and_when_sent_away() {
    let (mut p, _) = under_cloud();
    p.housed = true;
    p.state = State::Housed;
    assert!(!p.render().cloud);
    let (mut p, _) = under_cloud();
    p.state = State::Away;
    assert!(!p.render().cloud);
    let (mut p, _) = under_cloud();
    p.state = State::Farewell { remaining: 2.0 };
    assert!(!p.render().cloud);
}

#[test]
fn switching_to_another_pet_clears_the_cloud() {
    let (mut p, _) = under_cloud();
    p.cfg = cfg();
    p.tick_snow_cloud(DT);
    assert!(!p.render().cloud);
}

#[test]
fn looks_up_more_often_under_the_cloud() {
    let (p, _) = under_cloud();
    assert!(p.look_up_mult() > 1.0);
    assert_eq!(Pet::new(snowman(), 0.0, 0.0, 1).look_up_mult(), 1.0);
}
