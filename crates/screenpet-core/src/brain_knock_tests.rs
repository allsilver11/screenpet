//! 🐱 고양이 툭툭 (v0.6.3)

use super::knock_moves::KnockPhase;
use super::tests::{place, w1, world};
use super::*;
use crate::juggle::ObjectSpot;
use crate::play::{self, ObjectPlay};
use crate::test_util::{pet_cfg, DT};

const TOP: f64 = 400.0;

fn cat() -> PetConfig {
    PetConfig { knocks: true, half_w: 38.4, personality: Personality::Aloof, ..pet_cfg() }
}

/// 창(500..1100) 윗면 x에서 쉬는 고양이 — 처음 쉬는 때는 지났다
fn cat_on_window(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Window(1), x, State::Idle { remaining: 100.0, look: false });
    p.clock = 1000.0;
    p.play.knock_ready_at = 0.0;
    p
}

fn can(id: u64, x: f64) -> ObjectSpot {
    ObjectSpot { id, x, y: TOP, owner: Owner::Window(1), half_w: 24.0, giant: false, local: false }
}

fn knocking(p: &Pet) -> bool {
    matches!(p.state, State::Knock { .. })
}

#[test]
fn it_walks_behind_the_thing_stares_taps_a_few_times_pushes_it_off_the_near_edge_and_watches_it_fall() {
    let w = world(vec![w1()]);
    let mut p = cat_on_window(cat(), 700.0, 1, &w);
    let mut spot = can(7, 1000.0);
    assert!(p.start_knock(&spot, &w));
    let (mut taps, mut pushed, mut said, mut stared, mut pawed, mut watched) = (0, None, Vec::new(), false, false, false);
    let mut stood_at = None;
    for _ in 0..(60 * 15) {
        let spots: Vec<ObjectSpot> = if pushed.is_some() { vec![] } else { vec![spot] };
        p.notice_play_objects(&spots, &w);
        p.update(DT, &w);
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        match p.state {
            State::Knock { phase: KnockPhase::Look { .. }, .. } => {
                stared |= p.render().anim == Anim::Look;
                stood_at.get_or_insert(p.body.x);
            }
            State::Knock { phase: KnockPhase::Tap { .. } | KnockPhase::Push { .. }, .. } => pawed |= p.render().anim == Anim::Paw,
            State::Knock { phase: KnockPhase::Watch { looking: true, .. }, .. } => watched = true,
            _ => {}
        }
        match p.take_object_play() {
            Some(ObjectPlay::Nudge { id: 7, dir, reach }) if pushed.is_none() => {
                assert_eq!(dir, 1.0, "가까운 끝(오른쪽)으로");
                if reach < 20.0 {
                    taps += 1;
                    spot.x += reach;
                } else {
                    // 마지막 밀기는 끝(1100)을 넘게
                    assert!(spot.x + reach > 1100.0 + 0.1 * 128.0, "{} + {reach}", spot.x);
                    pushed = Some(p.clock);
                }
            }
            Some(e) => panic!("{e:?}"),
            None => {}
        }
        if pushed.is_some() && !knocking(&p) {
            break;
        }
    }
    assert!(stared && pawed && watched);
    assert!((play::TAPS.0..=play::TAPS.1).contains(&taps), "{taps}");
    assert!(pushed.is_some());
    let stood = stood_at.unwrap();
    assert!(stood < 1000.0 - 24.0 && 1000.0 - stood < 0.6 * 128.0, "물건 뒤에 붙어 선다: {stood}");
    assert_eq!(said.first(), Some(&Speech::Tap));
    assert!(said.contains(&Speech::Oops), "{said:?}");
    // 끝까지 가서 고개를 내민다 (몸 가운데는 윗면 안)
    assert!(p.body.x > 1100.0 - 0.4 * 128.0 && p.body.x <= 1100.0, "{}", p.body.x);
    assert!(p.standing.is_some_and(|s| s.owner == Owner::Window(1)));
    assert!(p.play.knock_ready_at - p.clock >= play::KNOCK_CALM_SECS.0 - 1.0, "한 번 하면 한참 쉰다");
}

#[test]
fn it_goes_for_a_thing_it_notices_on_its_window_now_and_then() {
    let w = world(vec![w1()]);
    let n = 400;
    let mut went = 0;
    for seed in 0..n {
        let mut p = cat_on_window(cat(), 700.0, seed, &w);
        p.notice_play_objects(&[can(1, 900.0)], &w);
        went += usize::from(knocking(&p));
        // 같은 물건엔 한 번만 굴린다
        if !knocking(&p) {
            p.notice_play_objects(&[can(1, 900.0)], &w);
            assert!(!knocking(&p));
        }
    }
    let share = went as f64 / n as f64;
    assert!((share - play::KNOCK_CHANCE).abs() < 0.07, "{share}");
}

#[test]
fn never_on_the_ground_never_giants_toys_or_far_things_and_not_while_resting_away_or_busy() {
    let w = world(vec![w1()]);
    let tries = |p: &mut Pet, spot: ObjectSpot| -> bool {
        for k in 0..60 {
            p.notice_play_objects(&[ObjectSpot { id: 100 + k, ..spot }], &w);
            if knocking(p) {
                return true;
            }
        }
        false
    };
    assert!(tries(&mut cat_on_window(cat(), 700.0, 1, &w), can(1, 900.0)), "평소엔 한다");
    // 바닥 (창 윗면이 아니다)
    let mut ground = Pet::new(cat(), 300.0, 0.0, 1);
    place(&mut ground, &w, Owner::Ground(0), 300.0, State::Idle { remaining: 100.0, look: false });
    ground.clock = 1000.0;
    ground.play.knock_ready_at = 0.0;
    assert!(!tries(&mut ground, ObjectSpot { owner: Owner::Ground(0), y: 1040.0, ..can(1, 400.0) }));
    assert!(!tries(&mut cat_on_window(cat(), 700.0, 1, &w), ObjectSpot { giant: true, ..can(1, 900.0) }));
    assert!(!tries(&mut cat_on_window(cat(), 700.0, 1, &w), ObjectSpot { local: true, ..can(1, 900.0) }));
    assert!(!tries(&mut cat_on_window(cat(), 600.0, 1, &w), can(1, 600.0 + 4.5 * 128.0)));
    assert!(!tries(&mut cat_on_window(pet_cfg(), 700.0, 1, &w), can(1, 900.0)), "고양이만");
    let mut calm = cat_on_window(cat(), 700.0, 1, &w);
    calm.play.knock_ready_at = calm.clock + 100.0;
    assert!(!tries(&mut calm, can(1, 900.0)));
    let mut away = cat_on_window(cat(), 700.0, 1, &w);
    away.set_user_away(true);
    assert!(!tries(&mut away, can(1, 900.0)));
    let mut focus = cat_on_window(cat(), 700.0, 1, &w);
    focus.set_focus(true);
    assert!(!tries(&mut focus, can(1, 900.0)));
    let mut sleepy = cat_on_window(cat(), 700.0, 1, &w);
    sleepy.state = State::Sleep { remaining: 100.0 };
    assert!(!tries(&mut sleepy, can(1, 900.0)));
    // 놀러 온 고양이도 한다
    let mut guest = cat_on_window(cat(), 700.0, 1, &w);
    guest.set_guest(crate::visit::Side::Left, 1000.0);
    assert!(tries(&mut guest, can(1, 900.0)));
}

#[test]
fn it_gives_up_when_the_thing_is_taken_away_or_the_user_steps_away() {
    let w = world(vec![w1()]);
    let mut p = cat_on_window(cat(), 700.0, 1, &w);
    assert!(p.start_knock(&can(3, 1000.0), &w));
    p.notice_play_objects(&[can(3, 1000.0)], &w);
    p.update(DT, &w);
    p.notice_play_objects(&[], &w);
    assert!(!knocking(&p), "누가 치웠다");
    let mut p = cat_on_window(cat(), 700.0, 2, &w);
    assert!(p.start_knock(&can(3, 1000.0), &w));
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(!knocking(&p));
}

#[test]
fn a_thing_near_the_left_end_goes_off_the_left_end() {
    let w = world(vec![w1()]);
    let mut p = cat_on_window(cat(), 900.0, 4, &w);
    assert!(p.start_knock(&can(5, 600.0), &w));
    let State::Knock { phase: KnockPhase::Approach { x, dir, .. }, .. } = p.state else { panic!() };
    assert_eq!(dir, -1.0);
    assert!(x > 600.0, "오른쪽 뒤에 선다");
}
