//! 펭귄의 안경 번쩍 (brain_flash.rs): 커서에 닿은 순간 · 친구와 놀 때 가끔 · 깨어 있을 때 몇 분에 한 번. 잘 때·잡힌 동안엔 없다.

use super::flash_moves::{FLASH_CALM_SECS, FLASH_PULSE_SECS, GLARE_SECS};
use super::social_moves::GreetKind;
use super::tests::{cfg, place, sim_cfg, win, world};
use super::*;
use crate::test_util::{DT, idle};

fn penguin() -> PetConfig {
    PetConfig { glasses_flash: true, ..sim_cfg() }
}

/// 넓은 바닥에서 secs초 사는 동안 render.flash가 켜진 시각들 (켜지는 순간)
fn flash_starts(c: PetConfig, seed: u64, secs: f64, mut each: impl FnMut(&mut Pet)) -> Vec<f64> {
    let w = world(vec![win(1, 0.0, 400.0, 1920.0, 900.0)]);
    let mut p = Pet::new(c, 960.0, 300.0, seed);
    let (mut starts, mut was, mut t) = (Vec::new(), false, 0.0);
    while t < secs {
        each(&mut p);
        p.update(DT, &w);
        let on = p.render().flash;
        if on && !was {
            starts.push(t);
        }
        was = on;
        t += DT;
    }
    starts
}

#[test]
fn an_awake_penguin_flashes_its_glasses_every_few_minutes() {
    for seed in 0..3 {
        let starts = flash_starts(penguin(), seed, 3600.0, |_| {});
        assert!((6..=30).contains(&starts.len()), "{} flashes in an hour", starts.len());
        assert!(starts[0] >= 60.0, "not right at start: {}", starts[0]);
        assert!(starts.windows(2).all(|g| g[1] - g[0] >= FLASH_CALM_SECS), "{starts:?}");
    }
}

#[test]
fn the_flash_is_a_short_pulse_the_webview_catches_once() {
    let w = world(vec![]);
    let mut p = Pet::new(penguin(), 800.0, 1040.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    p.glasses_flash();
    assert!(p.render().flash);
    let mut on = 0.0;
    while p.render().flash {
        p.update(DT, &w);
        on += DT;
        assert!(on < 1.0);
    }
    assert!((FLASH_PULSE_SECS - 2.0 * DT..=FLASH_PULSE_SECS + 2.0 * DT).contains(&on), "{on}");
    // 바로 다시 불러도 겹치지 않는다
    p.glasses_flash();
    assert!(!p.render().flash);
}

#[test]
fn never_while_asleep_or_held_and_never_for_pets_without_glasses() {
    let asleep = flash_starts(penguin(), 2, 1800.0, |p| p.state = State::Sleep { remaining: 1000.0 });
    assert!(asleep.is_empty(), "{asleep:?}");
    let held = flash_starts(penguin(), 3, 600.0, |p| {
        if !p.is_held() {
            p.pointer_down(p.body.x, p.body.y - 40.0);
        }
    });
    assert!(held.is_empty(), "{held:?}");
    let plain = flash_starts(sim_cfg(), 4, 3600.0, |_| {});
    assert!(plain.is_empty());
    // 플래그 없는 동물은 주사위를 굴리지 않는다 (다른 시뮬레이션의 난수 순서 그대로)
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 800.0, 1040.0, 5);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    let dice = p.rng.get_seed();
    p.maybe_glasses_flash();
    p.start_greet(900.0, GreetKind::Hello, 2.0);
    let mut q = Pet::new(cfg(), 800.0, 1040.0, 5);
    place(&mut q, &w, Owner::Ground(0), 800.0, idle());
    assert_eq!(q.rng.get_seed(), dice);
    q.start_greet(900.0, GreetKind::Hello, 2.0);
    assert_eq!(p.rng.get_seed(), q.rng.get_seed());
    assert!(!p.render().flash);
}

#[test]
fn playing_with_friends_sometimes_flashes() {
    let w = world(vec![]);
    let starts: [(&str, fn(&mut Pet)); 3] = [
        ("hello", |p| p.start_greet(900.0, GreetKind::Hello, 2.0)),
        ("caught", |p| p.start_greet(900.0, GreetKind::Caught { speak: true }, 2.0)),
        ("chase", |p| p.start_chase((900.0, 1040.0), 3.0)),
    ];
    for (name, start) in starts {
        let mut flashed = 0;
        for seed in 0..40 {
            let mut p = Pet::new(penguin(), 800.0, 1040.0, seed);
            place(&mut p, &w, Owner::Ground(0), 800.0, idle());
            start(&mut p);
            flashed += p.render().flash as u32;
        }
        assert!((5..=30).contains(&flashed), "{name}: {flashed}/40");
    }
    // 새침한 흥!·자는 친구 엿보기는 장난이 아니다
    for kind in [GreetKind::Hmph, GreetKind::Peek] {
        for seed in 0..20 {
            let mut p = Pet::new(penguin(), 800.0, 1040.0, seed);
            place(&mut p, &w, Owner::Ground(0), 800.0, idle());
            p.start_greet(900.0, kind, 2.0);
            assert!(!p.render().flash);
        }
    }
}

#[test]
fn reaching_the_cursor_flashes_the_glasses() {
    let w = world(vec![]);
    let mut p = Pet::new(penguin(), 800.0, 1040.0, 7);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    p.flash = Default::default();
    p.set_cursor_species("penguin");
    p.cursor_mood.ready_at = 0.0;
    let cursor = (1056.0, 1040.0 - 128.0 * 1.6);
    let mut flash_at = None;
    let mut reached = None;
    for i in 0..(60 * 40) {
        p.hover(cursor.0, cursor.1, false);
        p.cursor_input(false, false, true);
        // 혼자 번쩍은 이 시험에서 빼고 본다 (닿은 순간만)
        if !matches!(p.state, State::CursorPlay { .. } | State::CursorLeap { .. } | State::CursorHang { .. }) {
            p.flash.hold_off();
        }
        p.update(DT, &w);
        if p.render().flash && flash_at.is_none() {
            flash_at = Some(i);
        }
        if reached.is_none() && matches!(p.state, State::CursorPlay { phase: cursor_moves::PlayPhase::Reach, .. } | State::CursorLeap { .. }) {
            reached = Some(i);
        }
        if matches!(p.state, State::CursorHang { .. }) {
            break;
        }
        if matches!(p.state, State::Idle { .. } | State::Move { .. }) && reached.is_none() {
            p.state = idle();
        }
    }
    let (f, r) = (flash_at.expect("flashed"), reached.expect("reached the cursor"));
    assert_eq!(f, r, "flash on the arrival tick");
}

/// 넓은 바닥에서 secs초 사는 동안 (안경 반사 render.glare가 켜진 틱 수, 깨어 있던 틱 수, 반사가 이어진 길이들 초)
fn glare_time(c: PetConfig, seed: u64, secs: f64) -> (usize, usize, Vec<f64>) {
    let w = world(vec![win(1, 0.0, 400.0, 1920.0, 900.0)]);
    let mut p = Pet::new(c, 960.0, 300.0, seed);
    let (mut on, mut awake, mut runs, mut run, mut t) = (0, 0, Vec::new(), 0.0, 0.0);
    while t < secs {
        p.update(DT, &w);
        let r = p.render();
        if matches!(p.state, State::Sleep { .. }) {
            assert!(!r.glare, "no glare while asleep");
        } else {
            awake += 1;
        }
        if r.glare {
            on += 1;
            run += DT;
        } else if run > 0.0 {
            runs.push(run);
            run = 0.0;
        }
        t += DT;
    }
    (on, awake, runs)
}

#[test]
fn an_awake_penguin_wears_glaring_white_lenses_about_half_the_time() {
    // 사용자: 페이커처럼 — 평소 움직임의 절반쯤은 렌즈가 하얗게 차서 눈이 안 보인 채로 다닌다 (순간 번쩍이 아니라)
    for seed in 0..3 {
        let (on, awake, runs) = glare_time(penguin(), seed, 3600.0);
        let share = on as f64 / awake as f64;
        assert!((0.35..=0.65).contains(&share), "seed {seed}: glare {share:.2} of awake time");
        let mut sorted = runs.clone();
        sorted.sort_by(f64::total_cmp);
        let median = sorted[sorted.len() / 2];
        assert!((10.0..=45.5).contains(&median), "glare spells last a while: median {median:.1}s");
        assert!(runs.iter().all(|r| *r <= GLARE_SECS.1 + 0.1), "{runs:?}");
    }
}

#[test]
fn no_glare_for_pets_without_glasses_or_while_held() {
    let (on, _, _) = glare_time(sim_cfg(), 1, 600.0);
    assert_eq!(on, 0);
    let w = world(vec![]);
    let mut p = Pet::new(penguin(), 800.0, 1040.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    p.flash.force_glare();
    assert!(p.render().glare);
    p.pointer_down(p.body.x, p.body.y - 40.0);
    assert!(p.is_held());
    assert!(!p.render().glare, "held: eyes show");
}
