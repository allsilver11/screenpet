//! "놀러 보내기" — 보낸 동물은 화면 끝으로 뛰어가지 않고 제자리에서 손 흔들고("bye~!") 뿅 사라진다 (brain_send.rs).
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::send_moves::{SEND_POOF_SECS, SEND_WAVE_SECS};
use super::tests::{cfg, place, sim_cfg, w1, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 사라질 때까지 돌린다 (최대 secs초). 사라진 쪽과 걸린 시간. 그동안 한 번도 화면 끝으로 걸어 나가지 않는다
fn run_until_gone(p: &mut Pet, w: &World, secs: f64) -> Option<(Side, f64)> {
    let mut t = 0.0;
    while t < secs {
        p.update(DT, w);
        t += DT;
        assert!(!matches!(p.state, State::Exit { .. }), "never walks to the edge: {:?}", p.state);
        if let Some(side) = p.take_exit() {
            return Some((side, t));
        }
    }
    None
}

const GONE_SECS: f64 = SEND_WAVE_SECS + SEND_POOF_SECS;

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, walk_speed: 70.0, ..sim_cfg() }
}

fn frog() -> PetConfig {
    let size = 108.8;
    PetConfig { size, half_w: 0.3 * size, walk_speed: 83.0, personality: Personality::Playful, can_climb: true, hops: true, ..sim_cfg() }
}

#[test]
fn sent_pet_waves_bye_in_place_then_poofs_away_through_that_side() {
    let w = world(vec![]);
    for seed in 1..=12 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 700.0, idle());
        p.facing = -1.0;
        assert!(p.send_to(Side::Right, false));
        assert_eq!(p.sending(), Some(Side::Right));
        p.update(DT, &w);
        // 바로 멈춰 이쪽(앞)을 보고 손 흔든다 — "bye~!"
        assert!(matches!(p.state, State::Farewell { .. }), "seed {seed}: {:?}", p.state);
        assert_eq!(p.render().anim, Anim::Wave);
        assert!(!p.render().flip, "faces the viewer");
        assert_eq!(p.take_speech(), Some(Speech::Bye), "seed {seed}");
        let x = p.body.x;
        let (side, t) = run_until_gone(&mut p, &w, 10.0).unwrap_or_else(|| panic!("seed {seed} never left"));
        assert_eq!(side, Side::Right, "seed {seed}");
        assert!((t - GONE_SECS).abs() < 0.1, "seed {seed}: gone after the wave and the poof ({t})");
        assert!(p.is_away());
        assert_eq!(p.sending(), None, "trip is over once it left");
        // 제자리에서 사라졌다 (움직이지 않았다)
        assert_eq!(p.body.x, x, "seed {seed}");
    }
}

#[test]
fn the_last_moment_of_the_wave_is_a_poof() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 700.0, idle());
    p.send_to(Side::Left, false);
    let mut t = 0.0;
    let mut first_poof = None;
    while p.take_exit().is_none() {
        p.update(DT, &w);
        t += DT;
        if p.render().poof && first_poof.is_none() {
            first_poof = Some(t);
        }
        assert!(t < 5.0);
    }
    let first = first_poof.expect("poofs before vanishing");
    assert!((first - SEND_WAVE_SECS).abs() < 0.1, "{first}");
    assert!(serde_json::to_value(p.render()).unwrap().get("poof").is_some());
    // 평소엔 poof 아님
    let mut q = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut q, &w, Owner::Ground(0), 700.0, idle());
    assert!(!q.render().poof);
}

#[test]
fn a_pet_carrying_a_note_says_so_when_leaving() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 700.0, idle());
    assert!(p.send_to(Side::Right, true));
    p.update(DT, &w);
    assert_eq!(p.take_speech(), Some(Speech::Letter));
    assert_eq!(serde_json::to_string(&Speech::Letter).unwrap(), "\"letter\"");
    assert_eq!(run_until_gone(&mut p, &w, 5.0).map(|r| r.0), Some(Side::Right));
}

#[test]
fn a_hushed_send_waves_without_a_bubble() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 700.0, idle());
    assert!(p.send_to(Side::Right, false));
    p.hush_send();
    p.update(DT, &w);
    assert!(matches!(p.state, State::Farewell { .. }));
    assert_eq!(p.take_speech(), None);
    assert_eq!(run_until_gone(&mut p, &w, 5.0).map(|r| r.0), Some(Side::Right));
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
        p.send_to(Side::Right, false);
        assert_eq!(run_until_gone(&mut p, &w, 5.0).map(|r| r.0), Some(Side::Right), "seed {seed}");
    }
}

#[test]
fn sent_pet_on_a_window_top_waves_from_there() {
    let w = world(vec![w1()]);
    for seed in 1..=8 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.send_to(Side::Left, false);
        let (side, t) = run_until_gone(&mut p, &w, 5.0).unwrap_or_else(|| panic!("seed {seed}"));
        assert_eq!(side, Side::Left);
        assert!(t < GONE_SECS + 0.1, "seed {seed}: {t}");
    }
}

#[test]
fn bee_and_frog_wave_and_poof_too() {
    let w = world(vec![w1()]);
    for seed in 1..=8 {
        let mut b = Pet::new(bee(), 900.0, 500.0, seed);
        b.state = idle();
        b.send_to(Side::Right, false);
        assert_eq!(run_until_gone(&mut b, &w, 5.0).map(|r| r.0), Some(Side::Right), "bee seed {seed}");

        let mut f = Pet::new(frog(), 0.0, 0.0, seed);
        place(&mut f, &w, Owner::Ground(0), 1200.0, idle());
        f.send_to(Side::Left, false);
        assert_eq!(run_until_gone(&mut f, &w, 5.0).map(|r| r.0), Some(Side::Left), "frog seed {seed}");
    }
}

#[test]
fn waits_while_held_then_waves_once_put_down() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right, false);
    p.pointer_down(900.0, 1000.0);
    for _ in 0..30 {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Farewell { .. } | State::Exit { .. }));
    }
    assert_eq!(p.sending(), Some(Side::Right), "still going after being held");
    p.pointer_up(900.0, 1000.0);
    assert_eq!(run_until_gone(&mut p, &w, 10.0).map(|r| r.0), Some(Side::Right));
}

#[test]
fn waits_until_done_eating() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, State::Eating { remaining: 0.5, favorite: false, level_up: false });
    p.send_to(Side::Right, false);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Eating { .. }), "{:?}", p.state);
    assert_eq!(run_until_gone(&mut p, &w, 10.0).map(|r| r.0), Some(Side::Right));
}

#[test]
fn only_a_free_pet_can_be_sent() {
    let w = world(vec![]);
    // 들려 있음
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.pointer_down(900.0, 1000.0);
    assert!(!p.send_to(Side::Right, false));
    // 하우스 안
    let mut h = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut h, &w, Owner::Ground(0), 900.0, idle());
    h.enter_house();
    assert!(!h.send_to(Side::Right, false));
    // 이미 떠나는 중 (걸어 나감 / 손 흔드는 중)
    let mut e = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut e, &w, Owner::Ground(0), 900.0, State::Exit { remaining: 1.0, side: Side::Left });
    assert!(!e.send_to(Side::Right, false));
    let mut twice = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut twice, &w, Owner::Ground(0), 900.0, idle());
    assert!(twice.send_to(Side::Left, false));
    twice.update(DT, &w);
    assert!(!twice.send_to(Side::Right, false), "already waving goodbye");
    assert_eq!(twice.sending(), Some(Side::Left));
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
    g.send_to(Side::Right, false);
    g.go_home_now();
    assert_eq!(g.sending(), None);
    assert_eq!(run_until_gone(&mut g, &w, 10.0).map(|r| r.0), Some(Side::Left), "waves and goes home");
}

#[test]
fn come_home_while_waving_to_the_next_friend_goes_home_instead() {
    let w = world(vec![]);
    let mut g = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut g, &w, Owner::Ground(0), 900.0, idle());
    g.set_guest(Side::Left, 100.0);
    g.send_to(Side::Right, false);
    g.update(DT, &w);
    assert!(matches!(g.state, State::Farewell { .. }));
    g.go_home_now();
    assert_eq!(g.sending(), None);
    assert_eq!(run_until_gone(&mut g, &w, 10.0).map(|r| r.0), Some(Side::Left));
}

#[test]
fn cancel_send_stops_the_goodbye() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right, false);
    p.cancel_send();
    assert_eq!(p.sending(), None);
    assert!(!p.is_heading_out());
    // 손 흔들다가 그 친구가 끊겨도 그 자리에 남는다
    let mut q = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut q, &w, Owner::Ground(0), 900.0, idle());
    q.send_to(Side::Right, false);
    q.update(DT, &w);
    assert!(matches!(q.state, State::Farewell { .. }));
    q.cancel_send();
    assert!(!matches!(q.state, State::Farewell { .. }), "{:?}", q.state);
    assert_eq!(run_until_gone(&mut q, &w, 5.0), None, "stays");
}

#[test]
fn going_home_to_the_house_cancels_the_trip() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 900.0, idle());
    p.send_to(Side::Right, false);
    p.go_home(Owner::Ground(0), 1700.0);
    p.update(DT, &w);
    assert_eq!(p.sending(), None);
}

#[test]
fn pet_in_the_house_comes_out_then_waves_and_poofs() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 1600.0, idle());
    p.enter_house();
    assert!(!p.send_to(Side::Left, false), "not from inside the house");
    // 앱이 먼저 하우스에서 꺼낸다 (트레이 "하우스에서 나오기"와 같은 길)
    p.leave_house(&w, 1600.0, Owner::Ground(0));
    assert!(p.send_to(Side::Left, false));
    let (side, t) = run_until_gone(&mut p, &w, 10.0).expect("leaves");
    assert_eq!(side, Side::Left);
    assert!(t < 5.0, "comes out of the door, waves and poofs ({t})");
    assert!((p.body.x - 1600.0).abs() < 200.0, "near the door, not at the edge ({})", p.body.x);
}

#[test]
fn host_can_send_a_guest_on_and_it_waves_bye_and_poofs_toward_that_friend() {
    let w = world(vec![]);
    for seed in 1..=8 {
        let mut g = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut g, &w, Owner::Ground(0), 900.0, idle());
        g.set_guest(Side::Left, 100.0);
        assert!(g.send_to(Side::Right, false), "seed {seed}");
        g.update(DT, &w);
        assert_eq!(g.take_speech(), Some(Speech::Bye));
        // 집 쪽(왼쪽)이 아니라 보낸 쪽으로, 제자리에서
        let (side, t) = run_until_gone(&mut g, &w, 5.0).unwrap_or_else(|| panic!("seed {seed}"));
        assert_eq!(side, Side::Right, "seed {seed}");
        assert!(t < GONE_SECS + 0.1);
        assert_eq!(g.body.x, 900.0);
    }
    assert_eq!(serde_json::to_string(&Speech::Bye).unwrap(), "\"bye\"");
}

#[test]
fn wandering_exits_still_walk_off_the_edge() {
    // 보내지 않은 동물은 예전처럼 연결된 끝에서 걸어 나간다
    let w = world(vec![]);
    let mut walked = false;
    for seed in 1..=30 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        p.set_exit_edges(false, true);
        place(&mut p, &w, Owner::Ground(0), 1800.0, State::Move { kind: Move::Walk, remaining: 30.0 });
        p.facing = 1.0;
        for _ in 0..(60 * 20) {
            p.update(DT, &w);
            if matches!(p.state, State::Exit { .. }) {
                walked = true;
                break;
            }
        }
    }
    assert!(walked);
}
