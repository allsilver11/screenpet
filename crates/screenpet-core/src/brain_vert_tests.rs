//! 위/아래 친구 화면 (brain_vert.rs). brain.rs의 자식 모듈.

use super::tests::{cfg, place, w1, world};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::visit::Fling;

const DT: f64 = 1.0 / 60.0;

fn frog() -> PetConfig {
    PetConfig { hops: true, ..cfg() }
}

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, ..cfg() }
}

/// 바닥(Ground 0) x에 서 있는 동물
fn on_ground(cfg: PetConfig, w: &World, x: f64, seed: u64) -> Pet {
    let mut p = Pet::new(cfg, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 1.0, look: false });
    p
}

/// 나갈 때까지 (최대 ticks틱) — 나간 쪽과 그동안 가장 높이(작게)/낮게(크게) 간 발 y
fn exit_within(p: &mut Pet, w: &World, ticks: usize) -> (Option<Side>, f64, f64) {
    let (mut lo, mut hi) = (p.body.y, p.body.y);
    for _ in 0..ticks {
        p.update(DT, w);
        lo = lo.min(p.body.y);
        hi = hi.max(p.body.y);
        if let Some(side) = p.take_exit() {
            return (Some(side), lo, hi);
        }
    }
    (None, lo, hi)
}

/// 이동을 n번 정할 때 위/아래로 나간 횟수 (Up, Down)
fn vertical_decisions(cfg: PetConfig, n: u64, setup: impl Fn(&mut Pet)) -> (usize, usize) {
    let w = world(vec![]);
    let (mut up, mut down) = (0, 0);
    for seed in 0..n {
        let mut p = on_ground(cfg, &w, 900.0, seed);
        setup(&mut p);
        match p.move_state(true, &w) {
            State::Exit { side: Side::Up, .. } => up += 1,
            State::Exit { side: Side::Down, .. } => down += 1,
            _ => {}
        }
    }
    (up, down)
}

#[test]
fn no_vertical_exits_without_a_linked_connected_friend_up_or_down() {
    assert_eq!(vertical_decisions(cfg(), 2000, |_| {}), (0, 0));
    // 왼쪽/오른쪽 친구만 있으면 위/아래로 나가지 않는다
    assert_eq!(vertical_decisions(cfg(), 2000, |p| p.set_exit_edges(true, true)), (0, 0));
    // 위/아래를 끊으면 다시 안 나간다 (set_exit_edges가 위/아래를 지우지 않는 것도)
    assert_eq!(
        vertical_decisions(cfg(), 2000, |p| {
            p.set_vertical_edges(true, true);
            p.set_exit_edges(true, false);
            p.set_vertical_edges(false, false);
        }),
        (0, 0)
    );
}

#[test]
fn linked_up_and_down_friends_get_occasional_sky_jumps_and_dives() {
    let n = 4000;
    let (up, down) = vertical_decisions(cfg(), n, |p| p.set_vertical_edges(true, false));
    assert_eq!(down, 0);
    // 이동을 정할 때마다 3% 쯤
    let rate = up as f64 / n as f64;
    assert!((0.015..0.05).contains(&rate), "up rate {rate}");
    let (up, down) = vertical_decisions(cfg(), n, |p| p.set_vertical_edges(false, true));
    assert_eq!(up, 0);
    assert!((0.015..0.05).contains(&(down as f64 / n as f64)), "down {down}");
    // 개구리는 위로 더 자주
    let (frog_up, _) = vertical_decisions(frog(), n, |p| p.set_vertical_edges(true, false));
    assert!(frog_up as f64 > 1.5 * up as f64, "frog {frog_up} vs {up}");
}

#[test]
fn sky_jump_crouches_boings_and_leaves_through_the_top() {
    let w = world(vec![]);
    let mut p = on_ground(cfg(), &w, 900.0, 1);
    p.set_vertical_edges(true, false);
    p.state = p.vertical_exit(Side::Up);
    // 웅크리는 동안은 바닥에 그대로
    p.update(DT, &w);
    assert!(p.standing.is_some());
    assert_eq!(p.render().anim, Anim::Land);
    let (side, lo, _) = exit_within(&mut p, &w, 300);
    assert_eq!(side, Some(Side::Up));
    assert_eq!(p.take_speech(), Some(Speech::Boing));
    assert!(lo <= w.bounds.top, "feet went above the top: {lo}");
    assert!(p.is_away());
    // 개구리는 큰 점프 그림
    let mut f = on_ground(frog(), &w, 900.0, 1);
    f.state = f.vertical_exit(Side::Up);
    f.update(DT, &w);
    assert_eq!(f.render().anim, Anim::BigJump);
}

#[test]
fn sky_jump_needs_headroom_and_the_outer_top() {
    // 머리 바로 위에 창이 있으면 뛰지 않는다
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(800.0, 800.0, 1000.0, 980.0) }]);
    let mut p = on_ground(cfg(), &w, 900.0, 1);
    assert!(!p.can_go_vertical(&w, Side::Up));
    p.body.x = 1500.0;
    p.standing = Some(Standing { owner: Owner::Ground(0), rel_x: 1500.0 });
    assert!(p.can_go_vertical(&w, Side::Up));
    // 창 윗면에서도 (머리 위가 비면)
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 1.0, look: false });
    assert!(p.can_go_vertical(&w, Side::Up));
    // 그러나 창 윗면에서 바닥 밑으로는 못 간다
    assert!(!p.can_go_vertical(&w, Side::Down));
    // 위에 쌓인 모니터가 없는 줄(아래 모니터만)도 가장 바깥 위 끝이 아니면 못 뛴다
    let stacked = World::new(vec![], vec![Rect::new(0.0, 0.0, 1000.0, 1040.0), Rect::new(1000.0, 500.0, 2000.0, 1040.0)], 128.0);
    let mut q = Pet::new(cfg(), 1500.0, 0.0, 1);
    place(&mut q, &stacked, Owner::Ground(1), 1500.0, State::Idle { remaining: 1.0, look: false });
    assert!(!q.can_go_vertical(&stacked, Side::Up));
    assert!(q.can_go_vertical(&stacked, Side::Down));
}

#[test]
fn dive_hops_whees_and_sinks_out_of_the_bottom() {
    let w = world(vec![]);
    let mut p = on_ground(cfg(), &w, 900.0, 2);
    p.set_vertical_edges(false, true);
    assert!(p.can_go_vertical(&w, Side::Down));
    p.state = p.vertical_exit(Side::Down);
    let (side, lo, hi) = exit_within(&mut p, &w, 300);
    assert_eq!(side, Some(Side::Down));
    assert_eq!(p.take_speech(), Some(Speech::Whee));
    // 폴짝 뛰었다가 바닥 밑으로 몸이 다 빠진다
    assert!(lo < w.bounds.bottom - 10.0, "hopped up first: {lo}");
    assert!(hi - 128.0 >= w.bounds.bottom, "sank below: {hi}");
}

#[test]
fn flyers_fly_straight_up_or_down_and_out() {
    let w = world(vec![]);
    for (side, seed) in [(Side::Up, 1), (Side::Down, 2)] {
        let mut b = Pet::new(bee(), 900.0, 500.0, seed);
        b.state = b.vertical_exit(side);
        let (out, lo, hi) = exit_within(&mut b, &w, 600);
        assert_eq!(out, Some(side));
        assert_eq!(b.take_speech(), None);
        match side {
            Side::Up => assert!(lo <= 0.0),
            _ => assert!(hi - 128.0 >= 1040.0),
        }
    }
}

#[test]
fn thrown_into_a_linked_top_edge_flies_across_carrying_the_x_fraction() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 480.0, 600.0, 1);
    p.state = State::Air { anim: Anim::Fall };
    p.body = Body { x: 480.0, y: 600.0, vx: 200.0, vy: -1600.0 };
    p.thrown = true;
    p.set_vertical_edges(true, false);
    let (side, _, _) = exit_within(&mut p, &w, 60);
    assert_eq!(side, Some(Side::Up));
    let f = p.take_fling().expect("fling");
    assert!((0.2..0.35).contains(&f.height), "x fraction {}", f.height);
    assert!(f.vy < 0.0 && f.vx > 0.0);
    // 위가 연결되지 않았으면 천장에 막힌다
    let mut q = Pet::new(cfg(), 480.0, 600.0, 1);
    q.state = State::Air { anim: Anim::Fall };
    q.body = Body { x: 480.0, y: 600.0, vx: 200.0, vy: -1600.0 };
    q.thrown = true;
    assert_eq!(exit_within(&mut q, &w, 120).0, None);
}

#[test]
fn thrown_hard_into_a_linked_floor_falls_through_but_a_drop_lands() {
    let w = world(vec![]);
    let thrown = |vy: f64, thrown: bool| {
        let mut p = Pet::new(cfg(), 1500.0, 900.0, 1);
        p.state = State::Air { anim: Anim::Fall };
        p.body = Body { x: 1500.0, y: 900.0, vx: -100.0, vy };
        p.thrown = thrown;
        p.set_vertical_edges(false, true);
        p
    };
    let mut p = thrown(1400.0, true);
    assert_eq!(exit_within(&mut p, &w, 60).0, Some(Side::Down));
    let f = p.take_fling().unwrap();
    assert!((f.height - 1500.0 / 1920.0).abs() < 0.02 && f.vy > 0.0);
    // 살살 떨어뜨리거나 그냥 놓으면 바닥에 내려선다
    for mut q in [thrown(100.0, true), thrown(1400.0, false)] {
        assert_eq!(exit_within(&mut q, &w, 120).0, None);
        assert!(q.standing.is_some());
    }
}

#[test]
fn thrown_flyers_cross_up_and_down_too() {
    let w = world(vec![]);
    // 공기 저항으로 금방 느려지니 (던진 속도 / DRAG 만큼 간다) 끝 가까이에서
    for (y, vy, side) in [(500.0, -1500.0, Side::Up), (800.0, 1500.0, Side::Down)] {
        let mut b = Pet::new(bee(), 900.0, y, 1);
        b.state = State::Air { anim: Anim::Fall };
        b.body = Body { x: 900.0, y, vx: 0.0, vy };
        b.thrown = true;
        b.set_vertical_edges(true, true);
        assert_eq!(exit_within(&mut b, &w, 120).0, Some(side));
    }
}

/// 들어온 뒤 내려설 때까지 (최대 ticks틱) — 그동안 가장 높이 간 발 y
fn settle(p: &mut Pet, w: &World, ticks: usize) -> f64 {
    let mut lo = p.body.y;
    for _ in 0..ticks {
        p.update(DT, w);
        lo = lo.min(p.body.y);
        if p.standing.is_some() {
            break;
        }
    }
    lo
}

#[test]
fn arriving_from_above_falls_in_from_the_top_and_lands() {
    let w = world(vec![]);
    let mut p = Pet::new_entering(cfg(), &w, Side::Up, 3);
    assert!(matches!(p.state, State::Enter { side: Side::Up, .. }));
    // 몸은 막 화면 위에 (안 보인다)
    assert!(p.body.y <= w.bounds.top);
    assert_eq!(p.render().anim, Anim::Fall);
    settle(&mut p, &w, 400);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert!((p.body.y - 1040.0).abs() < 1e-6);
    // 던져져 왔으면 그 폭 비율 자리에서
    let mut q = Pet::new(cfg(), 0.0, 0.0, 4);
    q.enter_flung(&w, Side::Up, Fling { height: 0.75, vx: 120.0, vy: 300.0 });
    assert!((q.body.x - 0.75 * 1920.0).abs() < 1.0, "{}", q.body.x);
    assert!(q.body.vy > 0.0 && q.body.vx > 0.0);
    settle(&mut q, &w, 400);
    assert!(q.standing.is_some());
}

#[test]
fn arriving_from_below_pops_up_with_a_boing_and_lands() {
    let w = world(vec![]);
    let mut p = Pet::new_entering(cfg(), &w, Side::Down, 5);
    assert!(matches!(p.state, State::Enter { side: Side::Down, .. }));
    assert!(p.body.y - 128.0 >= w.bounds.bottom, "starts fully below");
    assert_eq!(p.take_speech(), Some(Speech::Boing));
    let lo = settle(&mut p, &w, 400);
    assert!(lo < 1040.0 - 128.0, "popped up above the floor: {lo}");
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    // 튀어 오르다 창 윗면에 내려설 수도 있다
    let low = world(vec![WindowInfo { id: 1, rect: Rect::new(600.0, 900.0, 1400.0, 1040.0) }]);
    let mut q = Pet::new(cfg(), 0.0, 0.0, 6);
    q.enter_flung(&low, Side::Down, Fling { height: 0.5, vx: 0.0, vy: -10.0 });
    settle(&mut q, &low, 400);
    assert_eq!(q.standing.map(|s| s.owner), Some(Owner::Window(1)));
}

#[test]
fn flyers_drift_in_from_above_or_below() {
    let w = world(vec![]);
    for side in [Side::Up, Side::Down] {
        let mut b = Pet::new_entering(bee(), &w, side, 7);
        for _ in 0..600 {
            b.update(DT, &w);
            if !matches!(b.state, State::Enter { .. }) {
                break;
            }
        }
        assert!(!matches!(b.state, State::Enter { .. }), "{side:?}: {:?}", b.state);
        assert!((0.0..=1040.0).contains(&b.body.y), "{side:?}: {}", b.body.y);
    }
}

#[test]
fn homesick_guest_whose_home_is_above_jumps_out_through_the_top() {
    let w = world(vec![]);
    for seed in 0..10 {
        let mut p = on_ground(cfg(), &w, 900.0, seed);
        p.set_guest(Side::Up, 0.0);
        let mut out = None;
        for _ in 0..(60 * 60) {
            p.update(DT, &w);
            // 옆으로 뒤돌거나 방향을 0으로 두지 않는다
            assert!(p.facing.abs() == 1.0, "seed {seed}: facing {}", p.facing);
            if let Some(s) = p.take_exit() {
                out = Some(s);
                break;
            }
        }
        assert_eq!(out, Some(Side::Up), "seed {seed}");
    }
}

#[test]
fn homesick_guest_whose_home_is_below_dives_down() {
    let w = world(vec![]);
    let mut p = on_ground(cfg(), &w, 900.0, 1);
    p.set_guest(Side::Down, 0.0);
    let (side, _, _) = exit_within(&mut p, &w, 60 * 60);
    assert_eq!(side, Some(Side::Down));
    // 보내진(손 흔들고 뿅) 손님도 집 쪽이 위면 위로 사라진다
    let mut g = on_ground(cfg(), &w, 900.0, 1);
    g.set_guest(Side::Up, 1000.0);
    g.go_home_now();
    assert_eq!(exit_within(&mut g, &w, 200).0, Some(Side::Up));
}

#[test]
fn guests_roam_up_or_down_to_another_friend_but_not_when_homesick() {
    let (up, down) = vertical_decisions(cfg(), 3000, |p| {
        p.set_guest(Side::Left, 1000.0);
        p.set_vertical_edges(true, true);
    });
    assert!(up > 50 && down > 0, "{up} {down}");
    // 그리우면 집(왼쪽) 말고는 안 간다
    assert_eq!(
        vertical_decisions(cfg(), 2000, |p| {
            p.set_guest(Side::Left, 0.0);
            p.set_vertical_edges(true, true);
        }),
        (0, 0)
    );
}

#[test]
fn sent_pets_wave_and_poof_toward_an_up_friend() {
    let w = world(vec![]);
    let mut p = on_ground(cfg(), &w, 900.0, 1);
    p.set_vertical_edges(true, false);
    assert!(p.send_to(Side::Up, false));
    assert_eq!(exit_within(&mut p, &w, 200).0, Some(Side::Up));
}
