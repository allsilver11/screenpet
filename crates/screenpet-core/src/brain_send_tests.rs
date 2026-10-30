//! 트레이 "놀러 보내기" — 내 동물이 그 친구가 연결된 화면 끝으로 곧장 가서 나간다 (brain_send.rs).
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::send_moves::SEND_GIVE_UP_SECS;
use super::tests::{cfg, place, sim_cfg, w1, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 나갈 때까지 돌린다 (최대 secs초). 나간 쪽과 걸린 시간
fn run_until_exit(p: &mut Pet, w: &World, secs: f64) -> Option<(Side, f64)> {
    let mut t = 0.0;
    while t < secs {
        p.update(DT, w);
        t += DT;
        if let Some(side) = p.take_exit() {
            return Some((side, t));
        }
    }
    None
}

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, walk_speed: 70.0, ..sim_cfg() }
}

fn frog() -> PetConfig {
    let size = 108.8;
    PetConfig { size, half_w: 0.3 * size, walk_speed: 83.0, personality: Personality::Playful, can_climb: true, hops: true, ..sim_cfg() }
}

#[test]
fn sent_pet_runs_to_that_edge_and_leaves_through_it() {
    let w = world(vec![]);
    for seed in 1..=12 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 700.0, idle());
        p.facing = -1.0;
        assert!(p.send_to(Side::Right));
        assert_eq!(p.sending(), Some(Side::Right));
        // 바로 그쪽을 보고 움직이기 시작한다
        p.update(DT, &w);
        assert_eq!(p.facing, 1.0, "seed {seed}");
        assert!(matches!(p.state, State::Move { .. }), "seed {seed}: {:?}", p.state);
        let (side, t) = run_until_exit(&mut p, &w, 30.0).unwrap_or_else(|| panic!("seed {seed} never left"));
        assert_eq!(side, Side::Right, "seed {seed}");
        assert!(t < SEND_GIVE_UP_SECS + 15.0, "seed {seed}: {t}");
        assert_eq!(p.sending(), None, "trip is over once it left");
        assert!(p.is_away());
    }
}

#[test]
fn sent_pet_ignores_the_other_linked_edge() {
    let w = world(vec![]);
    for seed in 1..=12 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        p.set_exit_edges(true, true);
        // 왼쪽 끝에 붙어 왼쪽으로 걷던 중
        place(&mut p, &w, Owner::Ground(0), 60.0, State::Move { kind: Move::Walk, remaining: 5.0 });
        p.facing = -1.0;
        p.send_to(Side::Right);
        assert_eq!(run_until_exit(&mut p, &w, 40.0).map(|r| r.0), Some(Side::Right), "seed {seed}");
    }
}

#[test]
fn sent_pet_on_a_window_top_gets_down_and_goes() {
    let w = world(vec![w1()]);
    for seed in 1..=8 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.send_to(Side::Left);
        assert_eq!(run_until_exit(&mut p, &w, 40.0).map(|r| r.0), Some(Side::Left), "seed {seed}");
    }
}

#[test]
fn bee_flies_there_and_frog_hops_there() {
    let w = world(vec![w1()]);
    for seed in 1..=8 {
        let mut b = Pet::new(bee(), 900.0, 500.0, seed);
        b.state = idle();
        b.send_to(Side::Right);
        assert_eq!(run_until_exit(&mut b, &w, 40.0).map(|r| r.0), Some(Side::Right), "bee seed {seed}");

        let mut f = Pet::new(frog(), 0.0, 0.0, seed);
        place(&mut f, &w, Owner::Ground(0), 1200.0, idle());
        f.send_to(Side::Left);
        assert_eq!(run_until_exit(&mut f, &w, 40.0).map(|r| r.0), Some(Side::Left), "frog seed {seed}");
    }
}

#[test]
fn without_a_route_it_walks_off_that_side_after_the_give_up_time() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Left);
    // 길이 막혀 아직 못 갔다고 치고 시간을 다 쓴다
    p.send.as_mut().unwrap().remaining = 0.0;
    p.state = State::Sleep { remaining: 100.0 };
    p.update(DT, &w);
    assert!(matches!(p.state, State::Exit { side: Side::Left, .. }), "{:?}", p.state);
    assert_eq!(run_until_exit(&mut p, &w, 60.0).map(|r| r.0), Some(Side::Left));
}

#[test]
fn give_up_waits_while_held_or_in_the_air() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right);
    p.send.as_mut().unwrap().remaining = 0.0;
    p.pointer_down(900.0, 1000.0);
    for _ in 0..30 {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Exit { .. }));
    }
    assert_eq!(p.sending(), Some(Side::Right), "still on its way after being held");
}

#[test]
fn only_a_free_pet_can_be_sent() {
    let w = world(vec![]);
    // 들려 있음
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.pointer_down(900.0, 1000.0);
    assert!(!p.send_to(Side::Right));
    // 하우스 안
    let mut h = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut h, &w, Owner::Ground(0), 900.0, idle());
    h.enter_house();
    assert!(!h.send_to(Side::Right));
    // 이미 떠나는 중
    let mut e = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut e, &w, Owner::Ground(0), 900.0, State::Exit { remaining: 1.0, side: Side::Left });
    assert!(!e.send_to(Side::Right));
}

#[test]
fn only_a_pet_with_nothing_else_going_on_can_be_invited_over() {
    let w = world(vec![]);
    let fresh = || {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, idle());
        p
    };
    assert!(fresh().is_free_to_go());
    let mut housed = fresh();
    housed.enter_house();
    assert!(!housed.is_free_to_go(), "stays in the house");
    let mut going = fresh();
    going.go_home(Owner::Ground(0), 1700.0);
    assert!(!going.is_free_to_go(), "on the way home");
    let mut eating = fresh();
    eating.state = State::Eating { remaining: 1.0, favorite: false, level_up: false };
    assert!(!eating.is_free_to_go());
    let mut leveling = fresh();
    leveling.state = State::LevelUp { remaining: 1.0 };
    assert!(!leveling.is_free_to_go());
    let mut held = fresh();
    held.pointer_down(900.0, 1000.0);
    assert!(!held.is_free_to_go());
    let mut guest = fresh();
    guest.set_guest(Side::Left, 100.0);
    assert!(!guest.is_free_to_go());
    let mut away = fresh();
    away.state = State::Exit { remaining: 1.0, side: Side::Left };
    assert!(!away.is_free_to_go());
}

#[test]
fn come_home_now_drops_a_hosts_send_on() {
    let w = world(vec![]);
    let mut g = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut g, &w, Owner::Ground(0), 900.0, idle());
    g.set_guest(Side::Left, 100.0);
    g.send_to(Side::Right);
    g.go_home_now();
    assert_eq!(g.sending(), None);
    assert_eq!(run_until_exit(&mut g, &w, 10.0).map(|r| r.0), Some(Side::Left), "waves and goes home");
}

#[test]
fn cancel_send_stops_heading_there() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right);
    p.cancel_send();
    assert_eq!(p.sending(), None);
    assert!(!p.is_heading_out());
}

#[test]
fn going_home_to_the_house_cancels_the_trip() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right);
    p.go_home(Owner::Ground(0), 1700.0);
    p.update(DT, &w);
    assert_eq!(p.sending(), None);
}

#[test]
fn pet_in_the_house_comes_out_then_goes() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 1600.0, idle());
    p.enter_house();
    assert!(!p.send_to(Side::Left), "not from inside the house");
    // 앱이 먼저 하우스에서 꺼낸다 (트레이 "하우스에서 나오기"와 같은 길)
    p.leave_house(&w, 1600.0, Owner::Ground(0));
    assert!(p.send_to(Side::Left));
    assert_eq!(run_until_exit(&mut p, &w, 40.0).map(|r| r.0), Some(Side::Left));
}

#[test]
fn host_can_send_a_guest_on_and_it_says_bye() {
    let w = world(vec![]);
    for seed in 1..=8 {
        let mut g = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut g, &w, Owner::Ground(0), 900.0, idle());
        g.set_guest(Side::Left, 100.0);
        assert!(g.send_to(Side::Right), "seed {seed}");
        assert_eq!(g.take_speech(), Some(Speech::Bye));
        // 집 쪽(왼쪽)이 아니라 보낸 쪽으로 나간다
        assert_eq!(run_until_exit(&mut g, &w, 40.0).map(|r| r.0), Some(Side::Right), "seed {seed}");
    }
    assert_eq!(serde_json::to_string(&Speech::Bye).unwrap(), "\"bye\"");
}
