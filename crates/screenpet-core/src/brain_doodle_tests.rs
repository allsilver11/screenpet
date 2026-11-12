//! 🖍️ 바닥 낙서 (흐물이 v0.6.3)

use super::doodle_moves::{COOLDOWN_SECS, DOODLE_AHEAD};
use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::test_util::{pet_cfg, DT};

#[test]
fn it_crouches_draws_for_two_seconds_and_hands_out_one_doodle_in_front_when_it_starts() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 1, &w);
    p.facing = -1.0;
    assert!(p.start_doodle(&w));
    assert_eq!(p.render().anim, Anim::Draw);
    let d = p.take_doodle().expect("그리기 시작하면 바로");
    assert!(p.take_doodle().is_none(), "한 번만");
    assert_eq!(d.facing, -1.0);
    assert!((d.x - (800.0 - DOODLE_AHEAD * 128.0)).abs() < 1e-9 && d.y == 1040.0);
    assert!((d.scale - 1.0).abs() < 1e-9);
    let start = p.clock;
    while matches!(p.state, State::Doodle { .. }) {
        p.update(DT, &w);
        assert!(p.take_doodle().is_none());
    }
    assert!(((p.clock - start) - DOODLE_SECS).abs() < 0.05);
    assert_eq!(serde_json::to_string(&d.kind).unwrap().trim_matches('"').len() > 2, true);
}

#[test]
fn every_kind_shows_up_and_it_draws_now_and_then_with_a_cooldown() {
    let w = world(vec![]);
    let mut kinds = std::collections::HashSet::new();
    let mut gaps = Vec::new();
    for seed in 0..8 {
        let mut p = resting(heumul(), 900.0, seed, &w);
        p.doodle.ready_at = 0.0;
        let mut last: Option<f64> = None;
        for _ in 0..(60 * 900) {
            p.update(DT, &w);
            if let Some(d) = p.take_doodle() {
                kinds.insert(format!("{:?}", d.kind));
                if let Some(t) = last {
                    gaps.push(p.clock - t);
                }
                last = Some(p.clock);
            }
        }
    }
    assert_eq!(kinds.len(), 4, "{kinds:?}");
    assert!(!gaps.is_empty() && gaps.iter().all(|&g| g >= COOLDOWN_SECS.0), "{gaps:?}");
}

#[test]
fn other_pets_and_busy_or_airborne_heumuls_never_draw() {
    let w = world(vec![]);
    let mut cat = resting(pet_cfg(), 800.0, 1, &w);
    cat.doodle.ready_at = 0.0;
    for _ in 0..(60 * 300) {
        cat.update(DT, &w);
        assert!(cat.take_doodle().is_none());
    }
    let mut p = resting(heumul(), 800.0, 2, &w);
    p.standing = None;
    p.state = State::Air { anim: Anim::Fall };
    assert!(!p.start_doodle(&w));
}
