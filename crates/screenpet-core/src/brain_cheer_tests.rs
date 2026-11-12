//! 🔄 새 버전이 나왔다: "new version!!" 하고 10초쯤 신나게 이리저리 뛴다 (brain_cheer.rs)

use super::tests::{place, sim_cfg, world};
use super::*;
use crate::test_util::{flyer, idle, DT};

const SECS: f64 = 10.0;

fn on_ground(c: PetConfig) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 0.0, 0.0, 11);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    (p, w)
}

/// secs초 동안 돌리며 (한 말들, 달린 틱 수, 방향을 바꾼 횟수, x 범위)
fn run_for(p: &mut Pet, w: &World, secs: f64) -> (Vec<Speech>, usize, usize, (f64, f64)) {
    let (mut said, mut running, mut turns) = (Vec::new(), 0, 0);
    let mut facing = p.facing;
    let mut xs = (p.body.x, p.body.x);
    for _ in 0..(secs / DT).round() as usize {
        p.update(DT, w);
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        if matches!(p.state, State::Move { kind: Move::Run, .. }) {
            running += 1;
        }
        if p.facing != facing {
            turns += 1;
            facing = p.facing;
        }
        xs = (xs.0.min(p.body.x), xs.1.max(p.body.x));
    }
    (said, running, turns, xs)
}

#[test]
fn the_speech_key_is_new_version() {
    assert_eq!(serde_json::to_string(&Speech::NewVersion).unwrap(), "\"new_version\"");
}

#[test]
fn it_shouts_and_runs_back_and_forth_for_about_ten_seconds_then_calms_down() {
    let (mut p, w) = on_ground(sim_cfg());
    assert!(p.cheer(SECS));
    let (said, running, turns, xs) = run_for(&mut p, &w, SECS - 0.3);
    assert_eq!(said.first(), Some(&Speech::NewVersion), "{said:?}");
    assert_eq!(said.iter().filter(|&&s| s == Speech::NewVersion).count(), 1, "said once");
    assert!(p.is_cheering());
    let ticks = ((SECS - 0.3) / DT) as usize;
    assert!(running * 10 >= ticks * 8, "runs most of the time: {running}/{ticks}");
    assert!(turns >= 4, "back and forth: {turns} turns");
    assert!(xs.1 - xs.0 > 150.0, "really runs around: {xs:?}");
    let (_, running_after, _, _) = run_for(&mut p, &w, 1.0);
    assert!(!p.is_cheering(), "over after ~10 s");
    assert!(running_after < 30, "stops running right away: {running_after}");
    // 그다음은 평소 생활 그대로
    run_for(&mut p, &w, 30.0);
    assert!(!p.is_cheering());
}

#[test]
fn a_second_call_while_cheering_changes_nothing() {
    let (mut p, w) = on_ground(sim_cfg());
    assert!(p.cheer(SECS));
    run_for(&mut p, &w, 5.0);
    p.cheer(SECS);
    let (said, _, _, _) = run_for(&mut p, &w, 5.5);
    assert!(said.iter().all(|&s| s != Speech::NewVersion), "{said:?}");
    assert!(!p.is_cheering(), "not stretched to 15 s");
}

#[test]
fn guests_never_cheer() {
    let (mut p, w) = on_ground(sim_cfg());
    p.set_guest(Side::Left, 1000.0);
    assert!(!p.cheer(SECS));
    let (said, running, _, _) = run_for(&mut p, &w, 3.0);
    assert!(!said.contains(&Speech::NewVersion));
    assert!(!p.is_cheering());
    assert!(running < 60, "{running}");
}

#[test]
fn it_waits_while_the_user_is_away_and_cheers_when_they_are_back() {
    let (mut p, w) = on_ground(sim_cfg());
    p.set_user_away(true);
    assert!(p.cheer(SECS), "kept for later");
    let (said, running, _, _) = run_for(&mut p, &w, 5.0);
    assert!(!said.contains(&Speech::NewVersion));
    assert_eq!(running, 0);
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    p.set_user_away(false);
    let (said, running, _, _) = run_for(&mut p, &w, 3.0);
    assert!(said.contains(&Speech::NewVersion), "{said:?}");
    assert!(running > 60);
}

#[test]
fn it_waits_while_held_or_in_the_house() {
    let (mut p, w) = on_ground(sim_cfg());
    p.state = State::Dragged;
    assert!(p.cheer(SECS));
    for _ in 0..60 {
        p.update(DT, &w);
    }
    assert_ne!(p.take_speech(), Some(Speech::NewVersion));
    assert!(matches!(p.state, State::Dragged));
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    let (said, _, _, _) = run_for(&mut p, &w, 1.0);
    assert!(said.contains(&Speech::NewVersion));

    let (mut p, w) = on_ground(sim_cfg());
    p.housed = true;
    p.state = State::Housed;
    assert!(p.cheer(SECS));
    let (said, _, _, _) = run_for(&mut p, &w, 2.0);
    assert!(!said.contains(&Speech::NewVersion));
    assert!(matches!(p.state, State::Housed));
}

#[test]
fn a_normal_nap_is_woken_for_it() {
    let (mut p, w) = on_ground(sim_cfg());
    p.state = State::Sleep { remaining: 100.0 };
    assert!(p.cheer(SECS));
    let (said, running, _, _) = run_for(&mut p, &w, 2.0);
    assert!(said.contains(&Speech::NewVersion));
    assert!(running > 60);
}

#[test]
fn flyers_fly_around_back_and_forth() {
    let w = world(vec![]);
    let mut p = Pet::new(PetConfig { half_w: 38.4, walk_speed: 90.0, ..flyer() }, 900.0, 500.0, 5);
    p.state = idle();
    run_for(&mut p, &w, 0.5);
    assert!(p.cheer(SECS));
    let (said, running, turns, xs) = run_for(&mut p, &w, SECS - 0.3);
    assert!(said.contains(&Speech::NewVersion));
    assert!(running * 10 >= ((SECS - 0.3) / DT) as usize * 7, "{running}");
    assert!(turns >= 3, "{turns}");
    assert!(xs.1 - xs.0 > 150.0, "{xs:?}");
    assert!(p.standing.is_none(), "flies, doesn't walk");
    run_for(&mut p, &w, 1.0);
    assert!(!p.is_cheering());
}

#[test]
fn hoppers_hop_while_cheering() {
    let (mut p, w) = on_ground(PetConfig { hops: true, ..sim_cfg() });
    assert!(p.cheer(SECS));
    let mut hops = 0;
    for _ in 0..(3.0 / DT) as usize {
        p.update(DT, &w);
        if p.render().anim == Anim::Hop {
            hops += 1;
        }
        assert_ne!(p.render().anim, Anim::Run, "hoppers hop, never run");
    }
    assert!(hops > 20, "{hops}");
}

#[test]
fn cheering_never_walks_off_to_a_friends_screen() {
    let (mut p, w) = on_ground(sim_cfg());
    p.set_exit_edges(true, true);
    assert!(p.cheer(SECS));
    for _ in 0..(SECS / DT) as usize {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Exit { .. } | State::Away), "{:?}", p.state);
    }
}

#[test]
fn it_waits_through_focus_time() {
    let (mut p, w) = on_ground(sim_cfg());
    p.set_focus(true);
    assert!(p.cheer(SECS));
    let (said, _, _, _) = run_for(&mut p, &w, 2.0);
    assert!(!said.contains(&Speech::NewVersion));
    p.set_focus(false);
    // 구석으로 가던 길(최대 5초)을 마치고 나서
    let (said, _, _, _) = run_for(&mut p, &w, 7.0);
    assert!(said.contains(&Speech::NewVersion), "{:?} {:?}", p.state, said);
}

#[test]
fn no_cursor_play_or_friend_games_while_cheering() {
    let (mut p, w) = on_ground(sim_cfg());
    assert!(p.cheer(SECS));
    p.update(DT, &w);
    assert!(p.is_cheering() && p.presence_quiet());
}

#[test]
fn every_species_cheers_and_moves_around() {
    for (name, c) in crate::test_util::bundled() {
        let w = world(vec![]);
        let mut p = Pet::new(c, 900.0, 500.0, 3);
        if !p.cfg.flies {
            place(&mut p, &w, Owner::Ground(0), 900.0, idle());
        } else {
            p.state = idle();
        }
        assert!(p.cheer(SECS), "{name}");
        let (said, _, _, xs) = run_for(&mut p, &w, SECS - 0.5);
        assert!(said.contains(&Speech::NewVersion), "{name}: {said:?}");
        assert!(xs.1 - xs.0 > 100.0, "{name}: {xs:?}");
        run_for(&mut p, &w, 1.0);
        assert!(!p.is_cheering(), "{name}");
    }
}
