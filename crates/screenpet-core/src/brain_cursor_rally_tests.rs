//! ⚾🖱️ 커서와 캐치볼 (brain_cursor_rally.rs·brain_rally.rs) 테스트 — 흐물이가 받아 커서 쪽으로, 다른 동물은 콩, 먼저 놀자고 하기
use super::juggle_moves::JugglePhase;
use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::cursor_rally::{CursorRallyEvent, INVITE_EVERY_SECS, IGNORED_BACKOFF_SECS};
use crate::juggle::{Incoming, JuggleEvent, ObjectSpot, MAX_RALLY};
use crate::physics::THROW_GRAVITY_LOGICAL;
use crate::test_util::{pet_cfg, DT};

const CURSOR: (f64, f64) = (700.0, 500.0);

/// 커서(왼쪽 위)에서 오른쪽으로 던진 것
fn user_ball(rally: u32) -> Incoming {
    Incoming { id: 9, x: 900.0, y: 700.0, vx: 520.0, vy: -420.0, rally, from: Side::Left, giant: false, user: true }
}

/// 앱처럼 돌려 (커서는 CURSOR) 받고 놓을 때까지: (받음, 외친 글자, 말, 놓은 속도, 커서 랠리 알림)
fn play(p: &mut Pet, w: &World, mut o: Incoming, secs: f64) -> (bool, Vec<String>, Vec<Speech>, Option<(f64, f64)>, Option<CursorRallyEvent>) {
    let (mut caught, mut said, mut speech, mut released, mut back) = (false, Vec::new(), Vec::new(), None, None);
    let mut t = 0.0;
    while t < secs {
        let flying = !caught && o.y < 1040.0;
        p.hover(CURSOR.0, CURSOR.1, false);
        p.notice_incoming(if flying { std::slice::from_ref(&o) } else { &[] }, w);
        p.update(DT, w);
        match p.take_juggle() {
            Some(JuggleEvent::Catch { .. }) => caught = true,
            Some(JuggleEvent::Release { vx, vy, .. }) => {
                released = Some((vx, vy));
                back = p.take_cursor_rally();
                break;
            }
            _ => {}
        }
        said.extend(p.take_speech_text());
        speech.extend(p.take_speech());
        if flying {
            o.vy += THROW_GRAVITY_LOGICAL * DT;
            o.x += o.vx * DT;
            o.y += o.vy * DT;
        }
        t += DT;
    }
    (caught, said, speech, released, back)
}

#[test]
fn heumul_catches_a_cursor_throw_counts_and_throws_it_back_at_the_cursor() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1300.0, 1, &w);
    let (caught, said, _, released, back) = play(&mut p, &w, user_ball(0), 20.0);
    assert!(caught);
    assert_eq!(said, vec!["1!".to_string()]);
    let (vx, vy) = released.expect("throws back");
    assert!(vx < 0.0 && vy < 0.0, "back up toward the cursor on the left: {vx} {vy}");
    assert_eq!(back, Some(CursorRallyEvent::Back { id: 9, rally: 1 }));
    assert_eq!(p.take_rally(), None, "not a friend rally");
}

#[test]
fn the_count_goes_on_and_ends_at_the_max() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1300.0, 2, &w);
    let (_, said, _, _, back) = play(&mut p, &w, user_ball(4), 20.0);
    assert_eq!(said, vec!["5!".to_string()]);
    assert_eq!(back, Some(CursorRallyEvent::Back { id: 9, rally: 5 }));
    let mut p = resting(heumul(), 1300.0, 3, &w);
    let (caught, said, speech, _, back) = play(&mut p, &w, user_ball(MAX_RALLY - 1), 20.0);
    assert!(caught && said.is_empty() && speech.contains(&Speech::RallyEnd), "Done!");
    assert_eq!(back, None);
}

#[test]
fn a_pet_without_hands_can_boop_a_cursor_throw_but_not_asleep() {
    let w = world(vec![]);
    let mut p = resting(pet_cfg(), 800.0, 4, &w);
    assert!(p.can_boop());
    p.boop(3);
    assert_eq!(p.take_speech_text().as_deref(), Some("boop! 3"));
    p.boop(MAX_RALLY);
    assert_eq!(p.take_speech(), Some(Speech::RallyEnd));
    let mut p = resting(pet_cfg(), 800.0, 4, &w);
    p.state = State::Sleep { remaining: 30.0 };
    assert!(!p.can_boop());
    assert!(!resting(heumul(), 800.0, 4, &w).can_boop(), "heumul catches instead");
}

/// 테스트: x에 놓인 물건
fn spot(id: u64, x: f64, w: &World) -> ObjectSpot {
    ObjectSpot { id, x, y: w.work_areas[0].bottom, owner: Owner::Ground(0), half_w: 14.0, giant: false, local: false }
}

/// 앱처럼 secs초: 커서·물건을 알려 주고 처음 나온 커서 랠리 알림·놓은 속도
fn invite_run(p: &mut Pet, w: &World, spots: &[ObjectSpot], secs: f64) -> (Option<CursorRallyEvent>, Option<(f64, f64)>) {
    let mut t = 0.0;
    let mut released = None;
    while t < secs {
        p.hover(CURSOR.0, CURSOR.1, false);
        p.set_cursor_rally_ok(true);
        p.notice_objects(spots, &[], w);
        p.notice_cursor_rally(spots, w);
        p.update(DT, w);
        if let Some(JuggleEvent::Release { vx, vy, .. }) = p.take_juggle() {
            released = Some((vx, vy));
        }
        if let Some(ev) = p.take_cursor_rally() {
            return (Some(ev), released);
        }
        t += DT;
    }
    (None, released)
}

#[test]
fn now_and_then_heumul_picks_something_up_and_invites_the_cursor_to_play() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 1000.0, 5, &w);
    p.juggle.calm_until = f64::INFINITY;
    let s = [spot(3, 1100.0, &w)];
    let mut got = None;
    for _ in 0..20 {
        let (ev, released) = invite_run(&mut p, &w, &s, INVITE_EVERY_SECS.1 + 30.0);
        if let Some(ev) = ev {
            got = Some((ev, released));
            break;
        }
    }
    let (ev, released) = got.expect("an invite");
    assert_eq!(ev, CursorRallyEvent::Back { id: 3, rally: 1 }, "the first throw is 1");
    let (vx, _) = released.expect("thrown");
    assert!(vx < 0.0, "toward the cursor on the left");
}

#[test]
fn other_pets_boop_a_nearby_thing_to_the_cursor_and_back_off_when_ignored() {
    let w = world(vec![]);
    let mut p = resting(pet_cfg(), 1000.0, 6, &w);
    let s = [spot(3, 1040.0, &w)];
    let mut first = None;
    let mut t = 0.0;
    let mut events = Vec::new();
    let mut clock = 0.0;
    while clock < 3.0 * (INVITE_EVERY_SECS.1 + IGNORED_BACKOFF_SECS.1) {
        if let (Some(ev), _) = invite_run(&mut p, &w, &s, 10.0) {
            if let CursorRallyEvent::Boop { id, vx, .. } = ev {
                assert_eq!(id, 3);
                assert!(vx < 0.0);
            }
            first.get_or_insert(clock);
            events.push(clock);
        }
        clock += 10.0;
        t += 10.0;
    }
    assert!(first.is_some(), "it invites now and then");
    // 아무도 받아 주지 않으니 사이가 길다
    for pair in events.windows(2) {
        assert!(pair[1] - pair[0] >= IGNORED_BACKOFF_SECS.0, "{events:?}");
    }
    let _ = t;
    let _ = JugglePhase::Catch;
}

#[test]
fn no_invites_without_permission_or_while_asleep() {
    let w = world(vec![]);
    let s = [spot(3, 1040.0, &w)];
    let mut p = resting(pet_cfg(), 1000.0, 7, &w);
    let mut t = 0.0;
    while t < 2.0 * INVITE_EVERY_SECS.1 {
        p.set_cursor_rally_ok(false);
        p.notice_cursor_rally(&s, &w);
        p.update(DT * 10.0, &w);
        assert_eq!(p.take_cursor_rally(), None);
        t += DT * 10.0;
    }
}
