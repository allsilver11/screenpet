//! ⚾ 캐치볼 (흐물이 v0.6.3)

use super::juggle_moves::JugglePhase;
use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::juggle::{Incoming, JuggleEvent, MAX_RALLY};
use crate::physics::THROW_GRAVITY_LOGICAL;
use crate::test_util::{pet_cfg, DT};

/// 오른쪽 끝에서 왼쪽으로 날아 들어오는 물건
fn ball(rally: u32) -> Incoming {
    Incoming { id: 9, x: 1880.0, y: 700.0, vx: -520.0, vy: -420.0, rally, from: Side::Right, giant: false }
}

struct Run {
    caught: bool,
    said: Vec<String>,
    speech: Vec<Speech>,
    released: Option<(f64, f64)>,
    rally: Option<(u64, u32)>,
}

/// 앱처럼: 날아오는 동안 넘기고, 받으면 손에 쥐여 준 것으로 (더 넘기지 않는다), 놓으면 끝
fn play(p: &mut Pet, w: &World, mut o: Incoming, secs: f64) -> Run {
    let mut r = Run { caught: false, said: Vec::new(), speech: Vec::new(), released: None, rally: None };
    let mut t = 0.0;
    while t < secs {
        let flying = !r.caught && o.y < 1040.0;
        p.notice_incoming(if flying { std::slice::from_ref(&o) } else { &[] }, w);
        p.update(DT, w);
        match p.take_juggle() {
            Some(JuggleEvent::Catch { id }) => {
                assert_eq!(id, o.id);
                r.caught = true;
            }
            Some(JuggleEvent::Release { vx, vy, .. }) => {
                r.released = Some((vx, vy));
                r.rally = p.take_rally();
                break;
            }
            _ => {}
        }
        r.said.extend(p.take_speech_text());
        r.speech.extend(p.take_speech());
        if flying {
            o.vy += THROW_GRAVITY_LOGICAL * DT;
            o.x += o.vx * DT;
            o.y += o.vy * DT;
        }
        t += DT;
    }
    r
}

#[test]
fn catch_animation_is_a_side_view_played_once() {
    assert_eq!(serde_json::to_string(&Anim::Catch).unwrap(), "\"catch\"");
    assert!(!Anim::Catch.is_front());
}

#[test]
fn it_runs_under_an_incoming_toss_catches_it_counts_and_lobs_it_back_where_it_came_from() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1350.0, 1, &w);
    let r = play(&mut p, &w, ball(2), 20.0);
    assert!(r.caught, "받는다");
    assert_eq!(r.said, vec!["3!".to_string()], "몇 번째인지");
    let (vx, vy) = r.released.expect("되던진다");
    assert!(vx > 0.0 && vy < 0.0, "날아온 쪽(오른쪽) 너머로 휙: {vx} {vy}");
    assert_eq!(r.rally, Some((9, 3)), "주고받은 횟수 + 1을 싣는다");
}

#[test]
fn it_shows_the_catch_pose_with_the_object_in_its_hands() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1350.0, 2, &w);
    let mut o = ball(0);
    let mut chased = false;
    for _ in 0..600 {
        p.notice_incoming(std::slice::from_ref(&o), &w);
        p.update(DT, &w);
        chased |= matches!(p.state, State::Juggle { phase: JugglePhase::Chase { .. }, .. });
        if p.take_juggle() == Some(JuggleEvent::Catch { id: 9 }) {
            assert_eq!(p.render().anim, Anim::Catch);
            assert_eq!(p.carried_object(), Some(9));
            assert!(p.carry_point().is_some());
            assert!(chased, "받으러 달려갔다");
            return;
        }
        o.vy += THROW_GRAVITY_LOGICAL * DT;
        o.x += o.vx * DT;
        o.y += o.vy * DT;
    }
    panic!("못 받았다");
}

#[test]
fn the_twentieth_catch_ends_the_rally() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1350.0, 3, &w);
    let r = play(&mut p, &w, ball(MAX_RALLY - 1), 20.0);
    assert!(r.caught);
    assert!(r.speech.contains(&Speech::RallyEnd) && r.said.is_empty(), "Done!");
    assert_eq!(r.rally, None, "되던지지 않는다");
}

#[test]
fn giants_far_tosses_busy_heumuls_and_other_pets_never_catch() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1350.0, 4, &w);
    assert!(!play(&mut p, &w, Incoming { giant: true, ..ball(0) }, 6.0).caught, "거대한 것");
    let mut p = resting(heumul(), 100.0, 5, &w);
    assert!(!play(&mut p, &w, ball(0), 6.0).caught, "너무 멀어 못 닿는다");
    let mut p = resting(heumul(), 1350.0, 6, &w);
    p.state = State::Sleep { remaining: 100.0 };
    assert!(!play(&mut p, &w, ball(0), 6.0).caught, "자는 중");
    let mut cat = resting(pet_cfg(), 1100.0, 7, &w);
    assert!(!play(&mut cat, &w, ball(0), 6.0).caught, "흐물이만");
}
