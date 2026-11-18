//! 눈·낙엽 더미에서 놀기 (brain_snowplay.rs) 테스트 — 눈 뭉쳐 던지기·되던지기·데굴·낙엽 뛰어들기, 드물게만
use super::tests::{place, sim_cfg, world};
use super::*;
use crate::juggle::Target;
use crate::snowplay::{Play, SnowPlayEvent, PLAY_EVERY_SECS};
use crate::test_util::{flyer, DT};
use crate::weather::Weather;

const SNOW: Option<(Weather, f32)> = Some((Weather::Snow, 4.0));
const LEAVES: Option<(Weather, f32)> = Some((Weather::Leaves, 3.0));

/// 테스트: 바닥 x에 쉬는 동물
fn on_ground(c: PetConfig, x: f64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 0.0, 0.0, 7);
    place(&mut p, &w, Owner::Ground(0), x, State::Idle { remaining: 0.5, look: false });
    (p, w)
}

/// 테스트: secs초 돌리며 (매 틱 더미·노릴 동물을 알려 줌) 처음 나온 놀이
fn run_until_play(p: &mut Pet, w: &World, pile: Option<(Weather, f32)>, targets: &[Target], secs: f64) -> Option<SnowPlayEvent> {
    let mut t = 0.0;
    while t < secs {
        p.notice_pile(pile, targets, false);
        p.update(DT, w);
        if let Some(ev) = p.take_snow_play() {
            return Some(ev);
        }
        t += DT;
    }
    None
}

#[test]
fn a_pet_scoops_snow_at_its_feet_and_throws_it_at_a_friend_nearby() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let friend = Target { x: 1200.0, y: p.body.y, size: 128.0 };
    p.force_snow_play(Play::Throw);
    let ev = run_until_play(&mut p, &w, SNOW, &[friend], 3.0).expect("a snowball");
    let SnowPlayEvent::Throw { foot_x, foot_y, x, y, vx, vy } = ev else { panic!("{ev:?}") };
    assert_eq!((foot_y, y < foot_y), (p.body.y, true), "scooped at the feet, thrown from up high");
    assert!((foot_x - p.body.x).abs() < 1.0 && (x - p.body.x).abs() < p.cfg.size);
    assert!(vx > 0.0 && vy < 0.0, "toward the friend: {vx} {vy}");
    assert_eq!(p.take_speech(), Some(Speech::Snowball));
}

#[test]
fn nothing_to_scoop_without_snow_and_fliers_never_play() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    assert_eq!(run_until_play(&mut p, &w, None, &[], 300.0), None);
    let (mut p, w) = on_ground(flyer(), 800.0);
    assert_eq!(run_until_play(&mut p, &w, SNOW, &[], 300.0), None);
}

#[test]
fn snow_play_is_a_rare_treat() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut plays = 0;
    let secs = 1200.0;
    let mut t = 0.0;
    while t < secs {
        p.notice_pile(SNOW, &[], false);
        p.update(DT, &w);
        plays += usize::from(p.take_snow_play().is_some());
        t += DT;
    }
    assert!(plays >= 1, "it does happen");
    assert!(plays as f64 <= secs / PLAY_EVERY_SECS.0 + 1.0, "{plays} in {secs}s");
}

#[test]
fn a_pet_hit_by_a_snowball_throws_one_back() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let thrower = Target { x: 500.0, y: p.body.y, size: 128.0 };
    // 방금 놀았어도 맞으면 되던진다
    p.force_snow_play(Play::Throw);
    assert!(run_until_play(&mut p, &w, SNOW, &[thrower], 3.0).is_some());
    let mut back = 0;
    for seed in 0..10 {
        let (mut q, _) = on_ground(sim_cfg(), 800.0);
        q.snowplay.rng = fastrand::Rng::with_seed(seed);
        q.feel_hit(crate::collide::Hit { face: crate::collide::Face::Side, vx: -500.0, vy: 0.0, dir: 1.0, fast: false, poop: false, snow: true });
        if let Some(SnowPlayEvent::Throw { vx, .. }) = run_until_play(&mut q, &w, SNOW, &[thrower], 20.0) {
            assert!(vx < 0.0, "back at the thrower");
            back += 1;
        }
    }
    assert!(back >= 6, "{back}/10");
}

#[test]
fn a_pet_dives_into_leaves_and_they_burst_where_it_lands() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.force_snow_play(Play::Dive);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    let ev = run_until_play(&mut p, &w, LEAVES, &[], 3.0).expect("a burst");
    let SnowPlayEvent::Burst { x, y } = ev else { panic!("{ev:?}") };
    assert_eq!(y, w.work_areas[0].bottom);
    assert!((x - p.body.x).abs() < 1.0 && x != 800.0, "landed a little way off");
}

#[test]
fn a_pet_flops_in_the_snow_with_a_puff() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.force_snow_play(Play::Flop);
    assert!(matches!(p.take_snow_play(), Some(SnowPlayEvent::Burst { .. })));
    assert_eq!(p.take_speech(), Some(Speech::Flop));
    // 못 구르는 동물은 배를 보이며 뒹군다
    let (mut q, _) = on_ground(PetConfig { no_roll: true, ..sim_cfg() }, 800.0);
    q.force_snow_play(Play::Flop);
    assert!(matches!(q.state, State::Trick { kind: Trick::Belly, .. }), "{:?}", q.state);
    let _ = w;
}

#[test]
fn an_interrupted_scoop_throws_nothing() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.force_snow_play(Play::Throw);
    p.pointer_down(p.body.x, p.body.y - 10.0);
    assert_eq!(run_until_play(&mut p, &w, SNOW, &[], 2.0), None);
}

#[test]
fn own_pets_sometimes_lob_a_snowball_over_to_a_friend() {
    let w = world(vec![]);
    let mut lobs = 0;
    for seed in 0..40 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 1600.0, State::Idle { remaining: 0.5, look: false });
        p.snowplay.rng = fastrand::Rng::with_seed(seed);
        p.set_exit_edges(false, true);
        p.force_snow_play(Play::Throw);
        if let Some(SnowPlayEvent::Throw { vx, vy, .. }) = run_until_play(&mut p, &w, SNOW, &[], 3.0) {
            if vx > 0.0 && -vy >= crate::juggle::LOB_VY * 0.99 {
                lobs += 1;
            }
        }
    }
    assert!(lobs >= 5, "{lobs}/40");
}
