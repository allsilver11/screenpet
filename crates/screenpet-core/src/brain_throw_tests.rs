//! 던지기 — 화면 배율(dpi)이 달라도 같은 느낌, 예전보다 부드럽고 둥근 포물선.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::physics::{GRAVITY, MAX_THROW_SPEED_LOGICAL, THROW_FEEL, THROW_LIFT};

const DT: f64 = 1.0 / 60.0;

fn scaled_cfg(dpi: f64) -> PetConfig {
    PetConfig { size: 128.0 * dpi, half_w: 38.0 * dpi, walk_speed: 60.0 * dpi, dpi, ..cfg() }
}

fn scaled_world(dpi: f64, windows: Vec<WindowInfo>) -> World {
    World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0 * dpi, 1040.0 * dpi)], 128.0 * dpi)
}

/// 바닥에 선 동물을 (x, 바닥 − 40) 에서 잡아 (논리 px/틱) step씩 n틱 움직이고 놓는다. 놓인 커서 자리를 돌려준다
fn flick(p: &mut Pet, w: &World, dpi: f64, x: f64, step: (f64, f64), n: usize) -> (f64, f64) {
    place(p, w, Owner::Ground(0), x * dpi, State::Idle { remaining: 100.0, look: false });
    let (mut cx, mut cy) = (x * dpi, 1000.0 * dpi);
    p.pointer_down(cx, cy);
    for _ in 0..n {
        p.update(DT, w);
        cx += step.0 * dpi;
        cy += step.1 * dpi;
        p.pointer_move(cx, cy);
    }
    p.pointer_up(cx, cy);
    (cx, cy)
}

/// 놓은 뒤 내려설 때까지: (논리 px 가로 거리, 틱 수, 논리 px 최고 높이)
fn fly_out(p: &mut Pet, w: &World, dpi: f64) -> (f64, usize, f64) {
    let (x0, y0) = (p.body.x, p.body.y);
    let mut top = y0;
    for i in 0..600 {
        p.update(DT, w);
        top = top.min(p.body.y);
        if p.standing.is_some() {
            return ((p.body.x - x0) / dpi, i + 1, (y0 - top) / dpi);
        }
    }
    panic!("never landed {:?}", p.body);
}

#[test]
fn same_logical_flick_flies_the_same_on_every_screen() {
    let mut first: Option<(f64, usize, f64)> = None;
    for dpi in [1.0, 1.25, 1.5, 2.0] {
        let w = scaled_world(dpi, vec![]);
        let mut p = Pet::new(scaled_cfg(dpi), 0.0, 0.0, 1);
        flick(&mut p, &w, dpi, 300.0, (25.0, -15.0), 6);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        let got = fly_out(&mut p, &w, dpi);
        let (d0, t0, h0) = *first.get_or_insert(got);
        assert!((got.0 - d0).abs() <= 0.05 * d0, "dpi {dpi}: {got:?} vs {d0}");
        assert!((got.2 - h0).abs() <= 0.05 * h0, "dpi {dpi}: {got:?} vs {h0}");
        assert!(got.1.abs_diff(t0) <= 2, "dpi {dpi}: {got:?} vs {t0}");
    }
}

#[test]
fn typical_flick_is_gentler_than_the_old_throw_but_still_a_throw() {
    let w = scaled_world(1.0, vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    // 커서 (1500, −900) px/s ≈ 1750 px/s 의 휙
    flick(&mut p, &w, 1.0, 300.0, (25.0, -15.0), 6);
    let start = p.body;
    let vx = 1500.0 * THROW_FEEL;
    assert!((p.body.vx - vx).abs() < 1.0 && (p.body.vy + 900.0 * THROW_FEEL + THROW_LIFT * vx).abs() < 1.0, "{:?}", p.body);
    let (dist, ticks, apex) = fly_out(&mut p, &w, 1.0);

    // 예전 물리: 커서 속도 그대로, 중력 2500
    let mut old = Body { vx: 1500.0, vy: -900.0, ..start };
    let (mut old_ticks, mut old_top) = (0, start.y);
    while old.y < 1040.0 {
        crate::physics::step_air(&mut old, DT, &w.bounds, 38.0, 128.0, GRAVITY);
        old_top = old_top.min(old.y);
        old_ticks += 1;
    }
    let old_dist = old.x - start.x;
    let ratio = dist / old_dist;
    assert!((0.6..=0.75).contains(&ratio), "new {dist} / old {old_dist} = {ratio}");
    assert!(ticks as f64 >= 1.1 * old_ticks as f64, "slower: {ticks} vs {old_ticks}");
    assert!(dist > 500.0, "still a real throw: {dist}");
    // 더 높고 둥근 포물선: 꼭대기 높이 / 거리가 예전보다 확실히 크다
    let old_apex = start.y - old_top;
    assert!(apex >= old_apex - 5.0, "apex {apex} vs old {old_apex}");
    assert!(apex / dist > 1.3 * old_apex / old_dist, "{apex}/{dist} vs {old_apex}/{old_dist}");
}

#[test]
fn sideways_flick_still_arcs_up_a_little() {
    let w = scaled_world(1.0, vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    flick(&mut p, &w, 1.0, 300.0, (25.0, 0.0), 6);
    assert!(p.body.vy < -100.0, "{:?}", p.body);
    let (_, _, apex) = fly_out(&mut p, &w, 1.0);
    assert!(apex > 10.0, "apex {apex}");
}

#[test]
fn wild_flicks_are_clamped_in_logical_px() {
    for dpi in [1.0, 1.5, 2.0] {
        let w = scaled_world(dpi, vec![]);
        let mut p = Pet::new(scaled_cfg(dpi), 0.0, 0.0, 1);
        flick(&mut p, &w, dpi, 300.0, (200.0, -60.0), 4);
        let speed = p.body.vx.hypot(p.body.vy);
        assert!((speed - MAX_THROW_SPEED_LOGICAL * dpi).abs() < 1e-6, "dpi {dpi}: {speed}");
    }
}

#[test]
fn a_single_jittery_last_sample_does_not_spike_the_throw() {
    let w = scaled_world(1.0, vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    let (cx, cy) = flick(&mut p, &w, 1.0, 300.0, (20.0, 0.0), 8);
    // 다시 잡아서 같은 휙, 마지막 이벤트만 200 px 튐
    let mut q = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut q, &w, Owner::Ground(0), 300.0, State::Idle { remaining: 100.0, look: false });
    q.pointer_down(300.0, 1000.0);
    let mut x = 300.0;
    for _ in 0..7 {
        q.update(DT, &w);
        x += 20.0;
        q.pointer_move(x, 1000.0);
    }
    q.update(DT, &w);
    q.pointer_move(x + 200.0, 1000.0);
    q.pointer_up(x + 200.0, 1000.0);
    assert_eq!((cx, cy), (300.0 + 160.0, 1000.0));
    assert!((p.body.vx - 1200.0 * THROW_FEEL).abs() < 1.0, "steady {}", p.body.vx);
    assert!((q.body.vx - p.body.vx).abs() <= 0.1 * p.body.vx, "jittery {} vs steady {}", q.body.vx, p.body.vx);

    // 첫 샘플이 튄 경우도
    let mut r = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut r, &w, Owner::Ground(0), 300.0, State::Idle { remaining: 100.0, look: false });
    r.pointer_down(300.0, 1000.0);
    r.update(DT, &w);
    r.pointer_move(120.0, 1000.0);
    let mut x = 320.0;
    for _ in 0..7 {
        r.update(DT, &w);
        x += 20.0;
        r.pointer_move(x, 1000.0);
    }
    r.pointer_up(x, 1000.0);
    assert!((r.body.vx - p.body.vx).abs() <= 0.1 * p.body.vx, "jittery start {} vs steady {}", r.body.vx, p.body.vx);
}

#[test]
fn steady_accelerating_flick_is_not_mistaken_for_jitter() {
    let w = scaled_world(1.0, vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 300.0, State::Idle { remaining: 100.0, look: false });
    p.pointer_down(300.0, 1000.0);
    let mut x = 300.0;
    for i in 1..=8 {
        p.update(DT, &w);
        x += 10.0 + 4.0 * i as f64;
        p.pointer_move(x, 1000.0);
    }
    p.pointer_up(x, 1000.0);
    // 0.12 s 창: 마지막 7구간 (18..42 px/틱)의 평균
    let expect = (x - (300.0 + 14.0)) / (7.0 * DT) * THROW_FEEL;
    assert!((p.body.vx - expect).abs() < 1.0, "{} vs {expect}", p.body.vx);
}

#[test]
fn thrown_climber_sticks_to_a_window_side_on_every_screen() {
    for dpi in [1.0, 1.5, 2.0] {
        let win = WindowInfo { id: 1, rect: Rect::new(800.0 * dpi, 300.0 * dpi, 1300.0 * dpi, 900.0 * dpi) };
        let w = scaled_world(dpi, vec![win]);
        let mut p = Pet::new(PetConfig { can_climb: true, ..scaled_cfg(dpi) }, 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 600.0 * dpi, State::Idle { remaining: 100.0, look: false });
        p.pointer_down(600.0 * dpi, 1000.0 * dpi);
        // 들어 올려 잠깐 멈춘 뒤, 창 쪽으로 가볍게 휙 (커서 ≈ 900 논리 px/s)
        p.update(DT, &w);
        p.pointer_move(600.0 * dpi, 700.0 * dpi);
        for _ in 0..10 {
            p.update(DT, &w);
            p.pointer_move(600.0 * dpi, 700.0 * dpi);
        }
        let (mut cx, mut cy) = (600.0 * dpi, 700.0 * dpi);
        for _ in 0..6 {
            p.update(DT, &w);
            cx += 15.0 * dpi;
            cy -= 5.0 * dpi;
            p.pointer_move(cx, cy);
        }
        p.pointer_up(cx, cy);
        let mut stuck = false;
        for _ in 0..120 {
            p.update(DT, &w);
            if let State::WallClimb { side, .. } = p.state {
                assert_eq!(side, Side::Left);
                stuck = true;
                break;
            }
        }
        assert!(stuck, "dpi {dpi}: {:?} {:?}", p.state, p.body);
    }
}

#[test]
fn falls_and_hops_keep_the_normal_gravity() {
    let w = scaled_world(1.0, vec![]);
    let mut p = Pet::new(cfg(), 500.0, 300.0, 1);
    p.update(DT, &w);
    assert!((p.body.vy - GRAVITY * DT).abs() < 1e-9, "plain fall {}", p.body.vy);
    // 던진 뒤 내려서고, 다시 떨어지면 평소 중력
    flick(&mut p, &w, 1.0, 300.0, (10.0, -10.0), 6);
    let vy = p.body.vy;
    p.update(DT, &w);
    assert!((p.body.vy - vy - crate::physics::THROW_GRAVITY_LOGICAL * DT).abs() < 1e-9);
    fly_out(&mut p, &w, 1.0);
    p.update(DT, &w);
    p.standing = None;
    p.body.vy = 0.0;
    p.state = State::Air { anim: Anim::Fall };
    p.body.y = 500.0;
    p.update(DT, &w);
    assert!((p.body.vy - GRAVITY * DT).abs() < 1e-9, "after landing {}", p.body.vy);
}
