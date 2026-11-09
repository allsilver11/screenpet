//! 위/아래 친구 화면 (brain_vert.rs). brain.rs의 자식 모듈.

use super::tests::{cfg, place, w1, world, thrown_at};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::visit::Fling;
use crate::test_util::{DT, flyer, idle};


fn frog() -> PetConfig {
    PetConfig { hops: true, ..cfg() }
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
    // 그 위에 다른 모니터가 있으면 못 뛴다 (위 끝이 낮은 모니터라도 위가 비면 뛴다 — a_monitor_with_nothing_above_it_…)
    let stacked = World::new(vec![], vec![Rect::new(1000.0, -1000.0, 2000.0, -40.0), Rect::new(1000.0, 0.0, 2000.0, 1040.0)], 128.0);
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
        let mut b = Pet::new(flyer(), 900.0, 500.0, seed);
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
    let mut p = thrown_at(cfg(), 480.0, 600.0, 200.0, -1600.0);
    p.set_vertical_edges(true, false);
    let (side, _, _) = exit_within(&mut p, &w, 60);
    assert_eq!(side, Some(Side::Up));
    let f = p.take_fling().expect("fling");
    assert!((0.2..0.35).contains(&f.height), "x fraction {}", f.height);
    assert!(f.vy < 0.0 && f.vx > 0.0);
    // 위가 연결되지 않았으면 천장에 막힌다
    let mut q = thrown_at(cfg(), 480.0, 600.0, 200.0, -1600.0);
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
        let mut b = Pet::new(flyer(), 900.0, y, 1);
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
        let mut b = Pet::new_entering(flyer(), &w, side, 7);
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

// ---- 아래로 던지기: 작업표시줄·모니터 구성과 상관없이, 아래로 던지면 넘어간다 ----

/// 작업표시줄 있는 왼쪽 모니터 + 작업표시줄 없는(더 긴) 오른쪽 모니터
fn taskbar_and_bare() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1080.0)], 128.0)
}

/// 위아래로 쌓인 모니터: 위 모니터는 작업표시줄이 있어 아래 모니터와 틈이 있다
fn stacked() -> World {
    World::new(vec![], vec![Rect::new(0.0, -1080.0, 1920.0, -40.0), Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
}

fn scaled(cfg: PetConfig, dpi: f64) -> PetConfig {
    PetConfig { size: 128.0 * dpi, half_w: 38.0 * dpi, walk_speed: 60.0 * dpi, dpi, ..cfg }
}

/// 진짜 손놀림: 잡아서 발이 feet_y에 닿을 자리까지 옮겨 잠깐 멈춘 뒤, 커서를 (vx, vy) 물리 px/s로 8틱 끌고 놓는다 (놓는 순간 발이 feet_y)
fn throw_by_hand(p: &mut Pet, w: &World, owner: Owner, x: f64, feet_y: f64, v: (f64, f64)) {
    let (_, ay) = w.anchor(owner).unwrap();
    place(p, w, owner, x, idle());
    let grab = p.cfg.size / 2.0;
    p.pointer_down(x, ay - grab);
    let n = 8.0;
    let (mut cx, mut cy) = (x - v.0 * n * DT, feet_y - grab - v.1 * n * DT);
    for _ in 0..12 {
        p.update(DT, w);
        p.pointer_move(cx, cy);
    }
    for _ in 0..8 {
        p.update(DT, w);
        cx += v.0 * DT;
        cy += v.1 * DT;
        p.pointer_move(cx, cy);
    }
    assert!((p.body.y - feet_y).abs() < 1e-6, "feet {} vs {feet_y}", p.body.y);
    p.pointer_up(cx, cy);
    assert_eq!(p.state, State::Air { anim: Anim::Fall });
}

/// 놓은 뒤 어떻게 됐나: 넘어간 끝, 또는 내려서면 None
fn outcome(p: &mut Pet, w: &World) -> Option<Side> {
    for _ in 0..600 {
        p.update(DT, w);
        if let Some(side) = p.take_exit() {
            return Some(side);
        }
        if p.standing.is_some() {
            return None;
        }
    }
    panic!("neither crossed nor landed: {:?} {:?}", p.state, p.body);
}

fn hand_throw(w: &World, x: f64, feet_y: f64, v: (f64, f64), dpi: f64) -> Option<Side> {
    let mut p = Pet::new(scaled(cfg(), dpi), x, 0.0, 1);
    p.set_vertical_edges(true, true);
    throw_by_hand(&mut p, w, Owner::Ground(0), x, feet_y, v);
    outcome(&mut p, w)
}

#[test]
fn throwing_down_from_low_on_the_taskbar_monitor_crosses_though_the_other_monitor_is_longer() {
    let w = taskbar_and_bare();
    let mut p = thrown_at(cfg(), 500.0, 900.0, 0.0, 1500.0);
    p.set_vertical_edges(false, true);
    assert_eq!(exit_within(&mut p, &w, 60).0, Some(Side::Down));
    // 작업표시줄 없는 쪽 모니터도 그대로
    let mut q = thrown_at(cfg(), 2500.0, 900.0, 0.0, 1500.0);
    q.set_vertical_edges(false, true);
    assert_eq!(exit_within(&mut q, &w, 60).0, Some(Side::Down));
    // 걷다가 바닥 밑으로 쏙도 작업표시줄 모니터의 바닥에서 된다
    let mut d = Pet::new(cfg(), 500.0, 0.0, 1);
    place(&mut d, &w, Owner::Ground(0), 500.0, State::Idle { remaining: 1.0, look: false });
    assert!(d.can_go_vertical(&w, Side::Down));
    d.state = d.vertical_exit(Side::Down);
    let (side, _, hi) = exit_within(&mut d, &w, 300);
    assert_eq!(side, Some(Side::Down));
    assert!(hi - 128.0 < 1080.0, "gone just below its own floor, not the union's: {hi}");
}

#[test]
fn the_floor_above_another_monitor_is_not_an_outer_edge() {
    let w = stacked();
    // 위 모니터 바닥: 바로 아래 모니터가 있으니 넘어가지 않고 내려선다
    let mut p = thrown_at(cfg(), 500.0, -200.0, 0.0, 1600.0);
    p.set_vertical_edges(true, true);
    assert_eq!(exit_within(&mut p, &w, 60).0, None);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert!(!p.can_go_vertical(&w, Side::Down));
    assert!(p.can_go_vertical(&w, Side::Up));
    // 아래 모니터 바닥은 넘어가고, 아래 모니터에선 위로 뛰어 나가지 못한다
    let mut q = Pet::new(cfg(), 500.0, 0.0, 1);
    place(&mut q, &w, Owner::Ground(1), 500.0, State::Idle { remaining: 1.0, look: false });
    assert!(q.can_go_vertical(&w, Side::Down));
    assert!(!q.can_go_vertical(&w, Side::Up));
    assert_eq!(hand_throw_from(&w, Owner::Ground(1), 500.0, 900.0, (0.0, 1000.0)), Some(Side::Down));
}

fn hand_throw_from(w: &World, owner: Owner, x: f64, feet_y: f64, v: (f64, f64)) -> Option<Side> {
    let mut p = Pet::new(cfg(), x, 0.0, 1);
    p.set_vertical_edges(true, true);
    throw_by_hand(&mut p, w, owner, x, feet_y, v);
    outcome(&mut p, w)
}

#[test]
fn a_monitor_with_nothing_above_it_is_an_up_edge_even_if_a_taller_one_stands_beside_it() {
    // 오른쪽 모니터가 왼쪽보다 낮게 시작한다
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 300.0, 3840.0, 1040.0)], 128.0);
    let mut p = thrown_at(cfg(), 2500.0, 800.0, 0.0, -1500.0);
    p.set_vertical_edges(true, false);
    assert_eq!(exit_within(&mut p, &w, 60).0, Some(Side::Up));
    // 뛰어 나가기도: 자기 모니터 위 끝을 넘으면 사라진다 (합친 화면 위 끝까지 안 간다)
    let mut q = Pet::new(cfg(), 2500.0, 0.0, 1);
    place(&mut q, &w, Owner::Ground(1), 2500.0, State::Idle { remaining: 1.0, look: false });
    assert!(q.can_go_vertical(&w, Side::Up));
    q.state = q.vertical_exit(Side::Up);
    let (side, lo, _) = exit_within(&mut q, &w, 300);
    assert_eq!(side, Some(Side::Up));
    assert!((200.0..=300.0).contains(&lo), "vanished at its own top: {lo}");
}

#[test]
fn a_real_downward_throw_crosses_even_if_slow_at_the_floor_on_every_screen() {
    for dpi in [1.0, 1.5, 2.0] {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0 * dpi, 1040.0 * dpi)], 128.0 * dpi);
        // 커서 1000 px/s로 바닥 200 px 위에서 놓는다: 바닥에 닿을 때 1000쯤 (옛 기준 1400에 못 미친다)
        assert_eq!(hand_throw(&w, 500.0 * dpi, 840.0 * dpi, (0.0, 1000.0 * dpi), dpi), Some(Side::Down), "dpi {dpi}");
        // 살짝 비스듬해도 아래로 던진 것
        assert_eq!(hand_throw(&w, 500.0 * dpi, 840.0 * dpi, (500.0 * dpi, 1000.0 * dpi), dpi), Some(Side::Down), "dpi {dpi}");
    }
}

#[test]
fn tosses_that_merely_fall_on_the_floor_still_land() {
    for dpi in [1.0, 1.5, 2.0] {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0 * dpi, 1040.0 * dpi)], 128.0 * dpi);
        for v in [
            // 옆으로, 위로, 옆이 더 빠른 비스듬히, 살살 아래로, 그냥 놓기
            (1500.0, 0.0),
            (0.0, -1000.0),
            (1500.0, 500.0),
            (0.0, 200.0),
            (0.0, 0.0),
        ] {
            assert_eq!(hand_throw(&w, 300.0 * dpi, 840.0 * dpi, (v.0 * dpi, v.1 * dpi), dpi), None, "dpi {dpi} {v:?}");
        }
    }
}

#[test]
fn releasing_with_the_feet_already_over_the_taskbar_still_crosses() {
    let w = world(vec![]);
    // 커서가 작업표시줄 위에 있을 때 놓으면 발은 이미 바닥(1040) 아래다
    assert_eq!(hand_throw(&w, 500.0, 1070.0, (0.0, 1000.0), 1.0), Some(Side::Down));
    // 그 자리에서 그냥 놓으면 바닥에 내려선다
    assert_eq!(hand_throw(&w, 500.0, 1070.0, (0.0, 0.0), 1.0), None);
    let mut p = Pet::new(cfg(), 500.0, 0.0, 1);
    throw_by_hand(&mut p, &w, Owner::Ground(0), 500.0, 1070.0, (0.0, 1000.0));
    // 아래 친구가 없으면 바닥에 내려선다
    assert_eq!(outcome(&mut p, &w), None);
}

#[test]
fn the_crossing_is_not_missed_on_the_tick_that_gravity_carries_past_the_floor() {
    // 중력을 빼면 이번 틱에 바닥에 못 미치고(1039.8), 중력까지 더하면 바닥을 넘는다(1040.3)
    let w = world(vec![]);
    let mut p = thrown_at(cfg(), 500.0, 1014.8, 0.0, 1500.0);
    p.set_vertical_edges(false, true);
    p.update(DT, &w);
    assert_eq!(p.take_exit(), Some(Side::Down));
}

#[test]
fn single_monitor_with_the_taskbar_on_any_edge() {
    for wa in [
        Rect::new(0.0, 0.0, 1920.0, 1040.0),  // 아래
        Rect::new(0.0, 40.0, 1920.0, 1080.0), // 위
        Rect::new(60.0, 0.0, 1920.0, 1080.0), // 왼쪽
        Rect::new(0.0, 0.0, 1860.0, 1080.0),  // 오른쪽
    ] {
        let w = World::new(vec![], vec![wa], 128.0);
        assert_eq!(hand_throw(&w, 500.0, wa.bottom - 150.0, (0.0, 1000.0), 1.0), Some(Side::Down), "{wa:?}");
        // 위 끝 친구: 위로 던지면 넘어간다 (작업표시줄이 위에 있어도)
        assert_eq!(hand_throw(&w, 500.0, wa.top + 300.0, (0.0, -2000.0), 1.0), Some(Side::Up), "{wa:?}");
    }
}

#[test]
fn guests_throw_down_only_where_the_roaming_rules_allow() {
    let w = world(vec![]);
    let guest = |home: Side, friends_below: bool, homesick_in: f64| {
        let mut p = Pet::new(cfg(), 500.0, 0.0, 1);
        p.set_guest(home, homesick_in);
        p.set_vertical_edges(false, friends_below);
        throw_by_hand(&mut p, &w, Owner::Ground(0), 500.0, 900.0, (0.0, 1000.0));
        outcome(&mut p, &w)
    };
    // 집이 아래면 넘어가고, 다른 친구가 아래에 있으면 돌아다니고, 아무도 없거나 그리우면 내려선다
    assert_eq!(guest(Side::Down, false, 1000.0), Some(Side::Down));
    assert_eq!(guest(Side::Left, true, 1000.0), Some(Side::Down));
    assert_eq!(guest(Side::Left, false, 1000.0), None);
    assert_eq!(guest(Side::Left, true, 0.0), None);
}

#[test]
fn arriving_after_a_throw_does_not_count_as_a_throw_down_again() {
    // 위에서 넘어온 동물이 살살 떨어져 아래 친구까지 줄줄이 빠져나가지 않는다
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 500.0, 0.0, 1);
    p.set_vertical_edges(false, true);
    throw_by_hand(&mut p, &w, Owner::Ground(0), 500.0, 900.0, (0.0, 1000.0));
    assert_eq!(outcome(&mut p, &w), Some(Side::Down));
    let tall = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 400.0)], 128.0);
    p.enter_flung(&tall, Side::Up, Fling { height: 0.5, vx: 0.0, vy: 0.0 });
    assert_eq!(outcome(&mut p, &tall), None);
}

#[test]
fn thrown_flyers_cross_down_by_aim_though_drag_slows_them() {
    let w = world(vec![]);
    let fly = |x: f64, y: f64, v: (f64, f64)| {
        let mut b = Pet::new(flyer(), x, y, 1);
        b.state = State::Air { anim: Anim::Fall };
        b.body.x = x;
        b.body.y = y;
        b.set_vertical_edges(false, true);
        b.pointer_down(x, y - 60.0);
        let (mut cx, mut cy) = (x, y - 60.0 - v.1 * 8.0 * DT);
        for _ in 0..12 {
            b.update(DT, &w);
            b.pointer_move(cx, cy);
        }
        for _ in 0..8 {
            b.update(DT, &w);
            cx += v.0 * DT;
            cy += v.1 * DT;
            b.pointer_move(cx, cy);
        }
        b.pointer_up(cx, cy);
        exit_within(&mut b, &w, 240).0
    };
    // 커서 1000 px/s, 바닥 100 px 위: 공기 저항으로 바닥에서 250쯤 — 옛 기준 400 미만
    assert_eq!(fly(900.0, 940.0, (0.0, 1000.0)), Some(Side::Down));
    // 옆으로 던지면 뜬 채로
    assert_eq!(fly(300.0, 940.0, (1500.0, 0.0)), None);
    // 작업표시줄 있는 모니터 바닥도 (합친 화면 바닥보다 위)
    let dual = taskbar_and_bare();
    let mut b = thrown_at(flyer(), 900.0, 900.0, 0.0, 1500.0);
    b.set_vertical_edges(false, true);
    assert_eq!(exit_within(&mut b, &dual, 120).0, Some(Side::Down));
}

// ---- 시뮬레이션: Windows, 작업표시줄이 아래 (작업 영역 아래 끝 = 작업표시줄 위) ----

/// 단일 / 듀얼 (모든 화면에 작업표시줄, 주 모니터에만, 크기가 다른 모니터, 왼쪽에 놓인 보조 모니터 — 음수 좌표)
fn windows_bottom_taskbar_setups() -> Vec<(&'static str, World, f64)> {
    let w = |areas: Vec<Rect>| World::new(vec![], areas, 128.0);
    vec![
        ("single", w(vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)]), 500.0),
        ("dual, taskbar on both", w(vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)]), 500.0),
        ("dual, taskbar on the primary only", w(vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1080.0)]), 500.0),
        ("dual, the bare secondary", w(vec![Rect::new(1920.0, 0.0, 3840.0, 1080.0), Rect::new(0.0, 0.0, 1920.0, 1040.0)]), 2500.0),
        ("dual, larger secondary", w(vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, -200.0, 4480.0, 1240.0)]), 500.0),
        ("dual, secondary to the left", w(vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(-1920.0, 0.0, 0.0, 1080.0)]), 500.0),
    ]
}

#[test]
fn windows_bottom_taskbar_ground_surface_is_the_work_area_bottom() {
    for (name, w, x) in windows_bottom_taskbar_setups() {
        let wa = w.work_areas[0];
        let ground = w.surfaces.iter().find(|s| s.owner == Owner::Ground(0)).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(ground.y, wa.bottom, "{name}");
        assert_eq!(crate::visit::column_bounds(&w, x).map(|c| c.bottom), Some(wa.bottom), "{name}");
        // 모든 모니터: 그 모니터의 바닥 디딤판이 그 모니터 작업 영역 바닥이고, 거기서 넘어가는 끝도 그 바닥이다
        for (i, area) in w.work_areas.iter().enumerate() {
            let g = w.surfaces.iter().find(|s| s.owner == Owner::Ground(i)).unwrap_or_else(|| panic!("{name}: no Ground({i})"));
            assert_eq!(g.y, area.bottom, "{name} Ground({i})");
            let mid = (area.left + area.right) / 2.0;
            assert!(g.contains_x(mid), "{name} Ground({i}) covers its own monitor");
            assert_eq!(crate::visit::column_bounds(&w, mid).map(|c| c.bottom), Some(area.bottom), "{name} Ground({i})");
        }
    }
}

/// 둘째 모니터 바닥에 내려서고 그 위를 걷는다 (Ground(1))
#[test]
fn a_pet_dropped_on_the_second_monitor_lands_on_its_own_ground() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1080.0)], 128.0);
    let mut p = Pet::new(cfg(), 2500.0, 700.0, 1);
    for _ in 0..240 {
        p.update(DT, &w);
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)), "{:?}", p.state);
    assert_eq!(p.body.y, 1080.0);
}

/// 진짜 틱 순서: 놓은 뒤 매 틱 update — 넘어가는 틱까지 한 번도 바닥에 내려서지 않는다. 틱 길이와 놓는 높이를 바꿔 가며 (틱이 바닥을 지나는 자리가 다 다르게)
#[test]
fn windows_bottom_taskbar_downward_throw_crosses_before_any_landing_tick_by_tick() {
    for (name, w, x) in windows_bottom_taskbar_setups() {
        let floor = w.work_area_at(x, 900.0).bottom;
        // 0.05 = game_loop.rs MAX_DT: 가장 긴 틱
        for dt in [0.05, 1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            for k in 0..40 {
                let feet = floor - 150.0 - 0.37 * k as f64;
                let mut p = Pet::new(cfg(), x, 0.0, 1);
                p.set_vertical_edges(false, true);
                throw_by_hand(&mut p, &w, Owner::Ground(0), x, feet, (0.0, 1000.0));
                let mut crossed = false;
                for _ in 0..300 {
                    p.update(dt, &w);
                    assert!(p.standing.is_none() && !matches!(p.state, State::Land { .. }), "{name} dt {dt} k {k}: landed first {:?}", p.body);
                    if p.take_exit() == Some(Side::Down) {
                        crossed = true;
                        break;
                    }
                }
                assert!(crossed, "{name} dt {dt} k {k}: {:?}", p.body);
                let f = p.take_fling().expect("fling carried");
                assert!(f.vy > 0.0, "{name}");
            }
        }
    }
}

#[test]
fn windows_bottom_taskbar_release_over_the_taskbar_and_tosses_on_every_setup() {
    for (name, w, x) in windows_bottom_taskbar_setups() {
        let floor = w.work_area_at(x, 900.0).bottom;
        let throw = |feet: f64, v: (f64, f64)| {
            let mut p = Pet::new(cfg(), x, 0.0, 1);
            p.set_vertical_edges(false, true);
            throw_by_hand(&mut p, &w, Owner::Ground(0), x, feet, v);
            outcome(&mut p, &w)
        };
        // 커서가 작업표시줄 위까지 가서 놓았다 (발은 이미 바닥 아래)
        assert_eq!(throw(floor + 30.0, (0.0, 1000.0)), Some(Side::Down), "{name}");
        // 그냥 놓거나 옆으로 던지면 내려선다
        assert_eq!(throw(floor - 150.0, (0.0, 0.0)), None, "{name}");
        assert_eq!(throw(floor - 150.0, (1200.0, 0.0)), None, "{name}");
    }
}

#[test]
fn thrown_down_from_a_window_it_falls_past_window_tops_to_the_friend_below() {
    let win = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    let low = WindowInfo { id: 2, rect: Rect::new(600.0, 950.0, 1000.0, 1040.0) };
    let w = World::new(vec![win, low], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let throw = |feet: f64, v: (f64, f64), below: bool| {
        let mut p = Pet::new(cfg(), 800.0, 0.0, 1);
        p.set_vertical_edges(false, below);
        throw_by_hand(&mut p, &w, Owner::Window(1), 800.0, feet, v);
        let out = outcome(&mut p, &w);
        (out, p.standing.map(|s| s.owner))
    };
    // 창 위에 서 있다가, 윗면보다 위에서 놓아도 / 윗면을 지나서 놓아도 아래 친구에게로 (창 윗면들에 내려앉지 않는다)
    assert_eq!(throw(300.0, (0.0, 1000.0), true).0, Some(Side::Down));
    assert_eq!(throw(600.0, (0.0, 1000.0), true).0, Some(Side::Down));
    // 아래 친구가 없으면 창 윗면에 내려앉는다
    assert_eq!(throw(300.0, (0.0, 1000.0), false), (None, Some(Owner::Window(1))));
    // 옆으로 던지거나 살살 놓으면 친구가 있어도 창 윗면에
    assert_eq!(throw(300.0, (0.0, 200.0), true), (None, Some(Owner::Window(1))));
    assert_eq!(throw(300.0, (0.0, 0.0), true), (None, Some(Owner::Window(1))));
    assert_eq!(throw(250.0, (300.0, -800.0), true), (None, Some(Owner::Window(1))));
}
