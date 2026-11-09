//! 조작 모드 — 방향키로 내 동물을 움직인다 (brain_control.rs). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::control_moves::{ControlInput, CONTROL_IDLE_SECS, JUMP_BODIES, REMOTE_STALE_SECS};
use super::tests::{place, sim_cfg, win, world};
use super::*;
use crate::test_util::{DT, LoopLimit, idle};
use super::tests::{sim_flyer as flyer, sim_frog as frog};


fn keys(f: impl FnOnce(&mut ControlInput)) -> ControlInput {
    let mut k = ControlInput::default();
    f(&mut k);
    k
}

const NONE: ControlInput = ControlInput { left: false, right: false, up: false, down: false, jump: false, run: false };

/// 내 동물: 키를 누른 채 secs초 (게임 루프처럼 매 틱 키를 먼저 알려 준다)
fn hold(p: &mut Pet, w: &World, k: ControlInput, secs: f64) {
    let n = (secs / DT).round() as usize;
    for _ in 0..n {
        p.control_keys(k, DT);
        p.update(DT, w);
    }
}

fn controlled_on_ground(cfg: PetConfig, x: f64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(cfg, 0.0, 0.0, 7);
    place(&mut p, &w, Owner::Ground(0), x, idle());
    p.set_control(true);
    (p, w)
}

fn walk_speed(cfg: &PetConfig, kind: Move) -> f64 {
    cfg.walk_speed * kind.speed_mult()
}

fn cat() -> PetConfig {
    PetConfig { can_climb: true, ..sim_cfg() }
}

#[test]
fn keys_round_trip_through_bits() {
    for bits in 0..64u8 {
        assert_eq!(ControlInput::from_bits(bits).bits(), bits);
    }
    let k = keys(|k| {
        k.left = true;
        k.run = true;
    });
    assert_eq!(ControlInput::from_bits(k.bits()), k);
    assert!(k.any());
    assert!(!NONE.any());
    // 모르는 비트(새 판)는 무시한다
    assert_eq!(ControlInput::from_bits(0xC0), NONE);
}

#[test]
fn arrows_walk_and_release_stops_facing_the_last_direction() {
    let cfg = sim_cfg();
    let (mut p, w) = controlled_on_ground(cfg, 700.0);
    assert!(p.is_controlled());
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    let expect = walk_speed(&cfg, Move::Walk);
    assert!((p.body.x - 700.0 - expect).abs() < 3.0, "walked one second right: {}", p.body.x);
    assert_eq!(p.render().anim, Anim::Walk);
    assert_eq!(p.facing, 1.0);
    // 왼쪽
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| k.left = true), 0.5);
    assert!(p.body.x < x - expect * 0.4, "walked back left");
    assert_eq!(p.facing, -1.0);
    // 놓으면 멈춰 쉬고, 마지막 방향을 본다
    let x = p.body.x;
    hold(&mut p, &w, NONE, 3.0);
    assert_eq!(p.body.x, x);
    assert!(matches!(p.state, State::Idle { .. }), "{:?}", p.state);
    assert_eq!(p.facing, -1.0);
    assert!(p.render().flip);
}

#[test]
fn shift_runs() {
    let cfg = sim_cfg();
    let (mut p, w) = controlled_on_ground(cfg, 300.0);
    hold(&mut p, &w, keys(|k| {
        k.right = true;
        k.run = true;
    }), 1.0);
    let expect = walk_speed(&cfg, Move::Run);
    assert!((p.body.x - 300.0 - expect).abs() < 5.0, "ran one second: {}", p.body.x);
    assert_eq!(p.render().anim, Anim::Run);
}

#[test]
fn controlled_pet_does_not_wander_sleep_or_play_tricks_on_its_own() {
    for seed in 1..=6 {
        let w = world(vec![]);
        let mut p = Pet::new(PetConfig { can_climb: true, ..sim_cfg() }, 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 800.0, State::Move { kind: Move::Walk, remaining: 5.0 });
        p.set_control(true);
        for _ in 0..(90.0 / DT) as usize {
            p.control_keys(NONE, DT);
            p.update(DT, &w);
            assert!(
                matches!(p.state, State::Idle { .. }),
                "seed {seed}: only rests while no key is held: {:?}",
                p.state
            );
        }
        assert_eq!(p.body.x, 800.0, "seed {seed}: stayed put");
        assert!(p.is_controlled(), "a key-less minute and a half is not the timeout yet");
    }
}

/// 점프해서 가장 높이 올라간 발 높이 (출발 높이 기준)와 내려선 디딤판
fn jump_peak(p: &mut Pet, w: &World, k: ControlInput) -> (f64, Option<Owner>) {
    let y0 = p.body.y;
    let mut top = y0;
    p.control_keys(k, DT);
    p.update(DT, w);
    assert!(matches!(p.state, State::Air { .. }), "jumped: {:?}", p.state);
    for _ in 0..300 {
        p.control_keys(NONE, DT);
        p.update(DT, w);
        top = top.min(p.body.y);
        if p.standing.is_some() {
            break;
        }
    }
    (y0 - top, p.standing.map(|s| s.owner))
}

#[test]
fn space_jumps_about_two_bodies_high_and_lands_again() {
    let cfg = sim_cfg();
    let (mut p, w) = controlled_on_ground(cfg, 700.0);
    let (h, owner) = jump_peak(&mut p, &w, keys(|k| k.jump = true));
    let want = JUMP_BODIES * cfg.size;
    assert!((h - want).abs() < want * 0.06, "jump height {h} ≈ {want}");
    assert_eq!(owner, Some(Owner::Ground(0)));
    assert_eq!(p.body.x, 700.0, "straight up and down");
    // ↑도 점프 (걷는 동물)
    let (h_up, _) = jump_peak(&mut p, &w, keys(|k| k.up = true));
    assert!((h_up - want).abs() < want * 0.06);
}

#[test]
fn jump_height_scales_with_size_and_frogs_jump_higher() {
    let big = PetConfig { size: 256.0, half_w: 76.8, ..sim_cfg() };
    let (mut p, w) = controlled_on_ground(big, 700.0);
    let (h_big, _) = jump_peak(&mut p, &w, keys(|k| k.jump = true));
    assert!((h_big - JUMP_BODIES * 256.0).abs() < 256.0 * 0.15, "{h_big}");
    let f = frog();
    let (mut p, w) = controlled_on_ground(f, 700.0);
    let (h_frog, _) = jump_peak(&mut p, &w, keys(|k| k.jump = true));
    assert!(h_frog > JUMP_BODIES * f.size * 1.3, "frog jumps higher: {h_frog}");
}

#[test]
fn jump_lands_on_a_window_top_it_reaches() {
    let win = win(1, 500.0, 840.0, 1100.0, 1000.0);
    let w = world(vec![win]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    p.set_control(true);
    let (_, owner) = jump_peak(&mut p, &w, keys(|k| k.jump = true));
    assert_eq!(owner, Some(Owner::Window(1)), "lands on the window top on the way down");
    assert_eq!(p.body.y, 840.0);
}

#[test]
fn arrows_steer_a_little_in_the_air() {
    let cfg = sim_cfg();
    let (mut p, w) = controlled_on_ground(cfg, 700.0);
    p.control_keys(keys(|k| k.jump = true), DT);
    p.update(DT, &w);
    let mut max_vx: f64 = 0.0;
    let mut limit = LoopLimit::new();
    while p.standing.is_none() {
        limit.step();
        p.control_keys(keys(|k| k.right = true), DT);
        p.update(DT, &w);
        max_vx = max_vx.max(p.body.vx.abs());
    }
    assert!(p.body.x > 700.0 + 10.0, "drifted right: {}", p.body.x);
    assert!(max_vx <= walk_speed(&cfg, Move::Run) + 1e-9, "only a little: {max_vx}");
}

#[test]
fn down_sits_while_held() {
    let (mut p, w) = controlled_on_ground(sim_cfg(), 700.0);
    hold(&mut p, &w, keys(|k| k.down = true), 1.0);
    assert_eq!(p.render().anim, Anim::SitFront);
    hold(&mut p, &w, NONE, 0.4);
    assert!(matches!(p.state, State::Idle { .. }), "{:?}", p.state);
}

#[test]
fn double_down_hops_off_a_window_top_and_single_down_does_not() {
    let win = win(1, 500.0, 600.0, 1100.0, 1000.0);
    let w = world(vec![win]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.set_control(true);
    let down = keys(|k| k.down = true);
    hold(&mut p, &w, down, 0.1);
    hold(&mut p, &w, NONE, 0.6);
    hold(&mut p, &w, down, 0.1);
    hold(&mut p, &w, NONE, 0.5);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "two slow taps stay");
    hold(&mut p, &w, down, 0.1);
    hold(&mut p, &w, NONE, 0.1);
    hold(&mut p, &w, down, 0.05);
    assert!(p.standing.is_none(), "quick double tap drops off: {:?}", p.state);
    hold(&mut p, &w, NONE, 2.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)), "fell through to the floor");
}

#[test]
fn climbers_climb_a_wall_with_up_and_slide_down_with_down() {
    let c = cat();
    let x = 1920.0 - c.half_w - 30.0;
    let (mut p, w) = controlled_on_ground(c, x);
    // 벽까지 걸어가 벽 쪽 + ↑ = 기어오르기
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!((p.body.x - (1920.0 - c.half_w)).abs() < 1.0, "stopped at the wall: {}", p.body.x);
    assert!(matches!(p.state, State::Idle { .. }), "blocked by the wall: {:?}", p.state);
    let y0 = p.body.y;
    hold(&mut p, &w, keys(|k| {
        k.right = true;
        k.up = true;
    }), 1.0);
    assert_eq!(p.render().anim, Anim::Climb);
    assert!(p.body.y < y0 - 40.0, "climbed: {}", p.body.y);
    // ↑만 눌러도 계속 오른다, 놓으면 매달려 있다
    let y1 = p.body.y;
    hold(&mut p, &w, keys(|k| k.up = true), 0.5);
    assert!(p.body.y < y1);
    let y2 = p.body.y;
    hold(&mut p, &w, NONE, 2.0);
    assert_eq!(p.body.y, y2, "hangs while no key is held");
    assert_eq!(p.render().anim, Anim::Climb);
    // ↓ = 미끄러져 내려와 바닥에 선다
    hold(&mut p, &w, keys(|k| k.down = true), 0.1);
    assert_eq!(p.render().anim, Anim::Slide);
    hold(&mut p, &w, NONE, 2.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
}

#[test]
fn non_climbers_jump_at_a_wall_instead_of_climbing() {
    let c = sim_cfg();
    let (mut p, w) = controlled_on_ground(c, 1920.0 - c.half_w);
    p.control_keys(keys(|k| {
        k.right = true;
        k.up = true;
    }), DT);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
}

#[test]
fn flyers_move_in_two_dimensions_without_gravity() {
    let b = flyer();
    let w = world(vec![]);
    let mut p = Pet::new(b, 900.0, 500.0, 5);
    p.state = idle();
    p.set_control(true);
    hold(&mut p, &w, keys(|k| k.up = true), 0.5);
    assert!(p.body.y < 500.0 - b.walk_speed * 0.4, "flew up: {}", p.body.y);
    assert!((p.body.x - 900.0).abs() < 1.0);
    let (x, y) = (p.body.x, p.body.y);
    hold(&mut p, &w, keys(|k| {
        k.left = true;
        k.down = true;
    }), 0.5);
    assert!(p.body.x < x - 10.0 && p.body.y > y + 10.0, "flew diagonally");
    // 놓으면 그 자리에 떠 있다 (떨어지지 않는다)
    let y = p.body.y;
    hold(&mut p, &w, NONE, 3.0);
    assert!((p.body.y - y).abs() < b.size * 0.2, "hovers in place: {} vs {y}", p.body.y);
    assert!(p.standing.is_none());
    // Shift = 더 빠르게
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| {
        k.right = true;
        k.run = true;
    }), 0.5);
    assert!(p.body.x - x > b.walk_speed * 0.5 * 1.3, "faster with shift");
}

#[test]
fn driving_into_a_linked_edge_walks_off_and_an_unlinked_one_stops() {
    let c = sim_cfg();
    let (mut p, w) = controlled_on_ground(c, 1920.0 - c.half_w - 20.0);
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!(matches!(p.state, State::Idle { .. }), "no friend there: stops at the wall");
    p.set_exit_edges(false, true);
    let mut gone = None;
    for _ in 0..600 {
        p.control_keys(keys(|k| k.right = true), DT);
        p.update(DT, &w);
        if let Some(side) = p.take_exit() {
            gone = Some(side);
            break;
        }
    }
    assert_eq!(gone, Some(Side::Right));
    // 날아다니는 동물도
    let w = world(vec![]);
    let mut p = Pet::new(flyer(), 1700.0, 500.0, 5);
    p.state = idle();
    p.set_control(true);
    p.set_exit_edges(true, false);
    let mut gone = None;
    for _ in 0..(15.0 / DT) as usize {
        p.control_keys(keys(|k| {
            k.left = true;
            k.run = true;
        }), DT);
        p.update(DT, &w);
        if let Some(side) = p.take_exit() {
            gone = Some(side);
            break;
        }
    }
    assert_eq!(gone, Some(Side::Left));
}

#[test]
fn control_ends_after_three_key_less_minutes() {
    let (mut p, w) = controlled_on_ground(sim_cfg(), 700.0);
    hold(&mut p, &w, NONE, CONTROL_IDLE_SECS - 10.0);
    hold(&mut p, &w, keys(|k| k.right = true), 0.1);
    hold(&mut p, &w, NONE, CONTROL_IDLE_SECS - 10.0);
    assert!(p.is_controlled(), "a key press restarts the clock");
    assert!(!p.take_control_ended());
    hold(&mut p, &w, NONE, 11.0);
    assert!(!p.is_controlled());
    assert!(p.take_control_ended(), "the app is told once (unchecks the menu)");
    assert!(!p.take_control_ended());
    assert!(!p.render().ctl);
}

#[test]
fn turning_control_off_gives_the_pet_back_its_own_mind() {
    let (mut p, w) = controlled_on_ground(sim_cfg(), 700.0);
    assert!(p.render().ctl, "badge while controlled");
    hold(&mut p, &w, keys(|k| k.down = true), 0.5);
    p.set_control(false);
    assert!(!p.is_controlled());
    assert!(!p.take_control_ended(), "turned off by the user, not the timeout");
    let mut moved = false;
    for _ in 0..(60.0 / DT) as usize {
        p.update(DT, &w);
        moved |= matches!(p.state, State::Move { .. });
    }
    assert!(moved, "wanders again");
}

#[test]
fn control_off_while_climbing_slides_down() {
    let c = cat();
    let (mut p, w) = controlled_on_ground(c, 1920.0 - c.half_w);
    hold(&mut p, &w, keys(|k| {
        k.right = true;
        k.up = true;
    }), 1.0);
    assert_eq!(p.render().anim, Anim::Climb);
    p.set_control(false);
    p.update(DT, &w);
    assert_eq!(p.render().anim, Anim::Slide);
}

#[test]
fn feeding_still_works_while_controlled() {
    let (mut p, w) = controlled_on_ground(sim_cfg(), 700.0);
    p.set_hungry(true);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
    let mut ate = false;
    for _ in 0..(15.0 / DT) as usize {
        p.control_keys(NONE, DT);
        p.update(DT, &w);
        ate |= matches!(p.state, State::Eating { .. });
    }
    assert!(ate, "walks to the bowl and eats");
    assert!(matches!(p.state, State::Idle { .. }), "then rests under control again: {:?}", p.state);
}

/// 손님 (친구 화면): 키는 친구 링크로 온다
fn guest_on_ground(x: f64, homesick_in: f64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 9);
    place(&mut p, &w, Owner::Ground(0), x, State::Idle { remaining: 0.5, look: false });
    p.set_guest(Side::Left, homesick_in);
    (p, w)
}

#[test]
fn remote_keys_drive_a_guest_and_go_stale_after_a_second() {
    let (mut p, w) = guest_on_ground(900.0, 600.0);
    p.remote_keys(true, keys(|k| k.right = true));
    assert!(p.is_controlled());
    for _ in 0..(0.5 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(p.body.x > 900.0 + 10.0, "walks right");
    assert_eq!(p.render().anim, Anim::Walk);
    // 1초 넘게 소식이 없으면 키를 다 놓은 것으로 본다 (조작은 계속)
    for _ in 0..((REMOTE_STALE_SECS + 0.1) / DT) as usize {
        p.update(DT, &w);
    }
    let x = p.body.x;
    for _ in 0..(1.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert_eq!(p.body.x, x, "stopped");
    assert!(p.is_controlled());
    // 새 키가 오면 다시 움직인다, 꺼지면 손님 마음대로
    p.remote_keys(true, keys(|k| k.left = true));
    p.update(DT, &w);
    assert!(p.body.x < x);
    p.remote_keys(false, NONE);
    assert!(!p.is_controlled());
}

#[test]
fn remote_keys_wake_a_guest_sleeping_because_the_host_is_away() {
    let (mut p, w) = guest_on_ground(900.0, 600.0);
    p.set_user_away(true);
    for _ in 0..(3.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    p.remote_keys(true, keys(|k| k.right = true));
    for _ in 0..(1.5 / DT) as usize {
        p.remote_keys(true, keys(|k| k.right = true));
        p.update(DT, &w);
    }
    assert!(p.body.x > 900.0 + 10.0, "walks right: {} {:?}", p.body.x, p.state);
    // 조작이 끝나면 다시 잔다
    p.remote_keys(false, NONE);
    for _ in 0..(3.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
}

#[test]
fn remote_keys_steer_a_homesick_guest_and_keep_it_from_leaving() {
    // 친구 화면에 오래 둬서 벌써 집이 그립다 (왼쪽이 집 쪽)
    let (mut p, w) = guest_on_ground(900.0, 0.0);
    for _ in 0..(1.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(p.is_heading_out());
    let x = p.body.x;
    // 3분 넘게 몰아도 포기하고 떠나지 않고, 키를 따른다
    for tick in 0..(240.0 / DT) as usize {
        let right = tick % 240 < 120;
        p.remote_keys(true, keys(|k| if right { k.right = true } else { k.left = true }));
        p.update(DT, &w);
        assert!(!p.leaving(), "left at tick {tick}");
    }
    p.remote_keys(true, keys(|k| k.right = true));
    let before = p.body.x;
    for _ in 0..(0.5 / DT) as usize {
        p.remote_keys(true, keys(|k| k.right = true));
        p.update(DT, &w);
    }
    assert!(p.body.x > before, "still obeys: {x} {before} {}", p.body.x);
    // 놓으면 다시 집으로
    p.remote_keys(false, NONE);
    assert!(p.is_heading_out());
}

#[test]
fn remote_keys_cannot_hold_a_guest_on_the_host_screen_forever() {
    let (mut p, w) = guest_on_ground(900.0, 0.0);
    let limit = crate::visit::CONTROLLED_HOMESICK_LIMIT_SECS;
    let mut sent_home = None;
    for tick in 0..((limit + 60.0) / DT) as usize {
        let right = tick % 240 < 120;
        p.remote_keys(true, keys(|k| if right { k.right = true } else { k.left = true }));
        p.update(DT, &w);
        if sent_home.is_none() && (p.leaving() || p.take_exit().is_some() || p.home_trip.is_some() || p.farewell_pending) {
            sent_home = Some(tick as f64 * DT);
        }
    }
    let at = sent_home.unwrap_or_else(|| panic!("{:?}", p.state));
    assert!((limit - 1.0..limit + 60.0).contains(&at), "friend keeps it for the whole limit, then no longer: {at}");
}

#[test]
fn pressing_a_key_brings_a_guest_out_of_a_window_and_then_it_obeys() {
    let win = crate::geom::WindowInfo { id: 1, rect: Rect::new(500.0, 300.0, 1300.0, 800.0) };
    let w = World::new(vec![win], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let mut p = Pet::new(sim_cfg(), 900.0, 0.0, 9);
    p.set_guest(Side::Left, 600.0);
    // 창 안에 내려놓는다
    p.body = Body { x: 900.0, y: 700.0, vx: 0.0, vy: 0.0 };
    p.state = State::Air { anim: Anim::Fall };
    p.standing = None;
    p.inside = Some(crate::inside::Stay::new(1, crate::inside::Mode::In, &mut p.rng));
    for _ in 0..(2.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(p.inside.is_some(), "settled inside: {:?}", p.state);
    let mut out_at = None;
    for _ in 0..(6.0 / DT) as usize {
        p.remote_keys(true, keys(|k| k.right = true));
        p.update(DT, &w);
        if p.inside.is_none() && p.standing.is_some() {
            out_at.get_or_insert(p.body.x);
        }
    }
    let out_at = out_at.unwrap_or_else(|| panic!("still inside: {:?} {:?}", p.state, p.body));
    // 나온 뒤엔 키대로 오른쪽으로 걷는다
    assert!(p.is_controlled());
    assert!(p.body.x > out_at + 10.0, "obeys: {} from {out_at}", p.body.x);
}

#[test]
fn remote_control_times_out_after_three_key_less_minutes() {
    let (mut p, w) = guest_on_ground(900.0, 10_000.0);
    p.remote_keys(true, NONE);
    for _ in 0..((CONTROL_IDLE_SECS - 1.0) / DT) as usize {
        p.update(DT, &w);
    }
    assert!(p.is_controlled(), "not before the time is up");
    for _ in 0..(2.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(!p.is_controlled());
}

#[test]
fn come_home_and_send_home_win_over_control() {
    let (mut p, w) = guest_on_ground(900.0, 600.0);
    p.remote_keys(true, keys(|k| k.right = true));
    p.update(DT, &w);
    p.go_home_now();
    let mut gone = None;
    for _ in 0..(10.0 / DT) as usize {
        p.remote_keys(true, keys(|k| k.right = true));
        p.update(DT, &w);
        if let Some(side) = p.take_exit() {
            gone = Some(side);
            break;
        }
    }
    assert_eq!(gone, Some(Side::Left));
}

#[test]
fn a_controlled_guest_can_still_be_grabbed() {
    let (mut p, w) = guest_on_ground(900.0, 600.0);
    p.remote_keys(true, keys(|k| k.right = true));
    p.update(DT, &w);
    let (x, y) = (p.body.x, p.body.y - 50.0);
    p.pointer_down(x, y);
    p.pointer_move(x + 100.0, y - 200.0);
    p.update(DT, &w);
    assert!(p.is_held(), "the host user can drag it: {:?}", p.state);
    p.pointer_move(x + 120.0, y - 220.0);
    p.update(DT, &w);
    assert!(p.is_held());
}

#[test]
fn controlled_pets_do_not_start_greetings_or_tag() {
    let (mut p, w) = controlled_on_ground(sim_cfg(), 700.0);
    p.update(DT, &w);
    assert!(!p.social_free(), "busy being driven");
    p.set_control(false);
    p.state = State::Idle { remaining: 5.0, look: false };
    assert!(p.social_free());
}

#[test]
fn steering_off_is_sent_twice_and_then_stays_quiet() {
    use super::control_moves::SteerPacer;
    use std::time::{Duration, Instant};
    let ka = Duration::from_millis(200);
    let t0 = Instant::now();
    let at = |ms: u64| t0 + Duration::from_millis(ms);
    let mut p = SteerPacer::default();
    // 이번 방문에 아직 보낸 적 없고 꺼져 있으면 아무것도 보내지 않는다
    assert!(!p.should_send(7, false, 0, at(0), ka));
    // 켜면 알리고, 키가 바뀌면 바로, 누르는 동안 keepalive마다
    assert!(p.should_send(7, true, 0, at(10), ka));
    assert!(!p.should_send(7, true, 0, at(1000), ka), "no keys: nothing to keep alive");
    assert!(p.should_send(7, true, 1, at(1010), ka));
    assert!(!p.should_send(7, true, 1, at(1100), ka));
    assert!(p.should_send(7, true, 1, at(1210), ka));
    // 끄면 바로 한 번, keepalive 뒤에 한 번 더, 그 뒤로는 조용히
    assert!(p.should_send(7, false, 0, at(1300), ka));
    assert!(!p.should_send(7, false, 0, at(1400), ka));
    assert!(p.should_send(7, false, 0, at(1500), ka));
    assert!(!p.should_send(7, false, 0, at(1700), ka));
    assert!(!p.should_send(7, false, 0, at(9000), ka));
    // 다시 켜면 다시 처음부터, 끈 직후 다시 켜면 두 번째 끄기는 없다
    assert!(p.should_send(7, true, 0, at(9100), ka));
    assert!(p.should_send(7, false, 0, at(9200), ka));
    assert!(p.should_send(7, true, 0, at(9250), ka));
    assert!(!p.should_send(7, true, 0, at(9500), ka));
    // 새 방문은 켜져 있을 때만
    assert!(!p.should_send(8, false, 0, at(9600), ka));
    p.reset();
    assert!(p.should_send(9, true, 0, at(9700), ka));
}
