//! 🎣 커서 매달리기 (흐물이 v0.6.3)

use super::swing_moves::{SwingPhase, COOLDOWN_SECS, FLING_SPEED, HANG_SECS, NOTICE_SECS};
use super::tests::{place, world};
use super::*;
use crate::test_util::{pet_cfg, DT};

pub(super) fn heumul() -> PetConfig {
    PetConfig { half_w: 38.4, walk_speed: 110.0, personality: Personality::Playful, timid: true, energetic: true, rigging: true, juggler: true, antics: true, ..pet_cfg() }
}

/// 바닥(x)에 쉬고 있는 흐물이 — 첫 쿨다운은 지났다
pub(super) fn resting(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 100.0, look: false });
    p.clock = 1000.0;
    p.swing.ready_at = 0.0;
    p.hide.ready_at = f64::INFINITY;
    p.doodle.ready_at = f64::INFINITY;
    p.hoist.ready_at = f64::INFINITY;
    p
}

/// 앱처럼 한 틱: 커서 자리·버튼, 커서 장난 켜 둠
fn tick(p: &mut Pet, w: &World, cx: f64, cy: f64, down: bool) {
    p.cursor_input(down, false, true);
    p.hover(cx, cy, false);
    p.cursor = (cx, cy);
    p.update(DT, w);
}

fn above_head(p: &Pet) -> (f64, f64) {
    (p.body.x, p.body.y - p.cfg.size)
}

fn hanging(p: &Pet) -> bool {
    matches!(p.state, State::Swing { phase: SwingPhase::Hang, .. })
}

/// 머리 위에 커서를 두고 매달릴 때까지 (최대 2초)
fn grab(p: &mut Pet, w: &World) -> (f64, f64) {
    let (cx, cy) = above_head(p);
    for _ in 0..120 {
        tick(p, w, cx, cy, false);
        if hanging(p) {
            break;
        }
    }
    assert!(hanging(p), "{:?}", p.state);
    (cx, cy)
}

#[test]
fn swing_animation_is_an_optional_side_view() {
    assert_eq!(serde_json::to_string(&Anim::HangCursor).unwrap(), "\"hang_cursor\"");
    assert!(!Anim::HangCursor.is_front() && !Anim::HangCursor.is_back());
}

#[test]
fn a_cursor_resting_just_above_the_head_for_a_moment_makes_it_jump_up_and_grab() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 1, &w);
    let (cx, cy) = above_head(&p);
    let mut jumped_at = None;
    for i in 0..60 {
        tick(&mut p, &w, cx, cy, false);
        if matches!(p.state, State::Swing { .. }) && jumped_at.is_none() {
            jumped_at = Some(i as f64 * DT);
        }
    }
    let t = jumped_at.expect("뛰어올라 잡는다");
    assert!(t >= NOTICE_SECS - 2.0 * DT && t <= NOTICE_SECS + 3.0 * DT, "{t}");
    assert!(hanging(&p));
    assert_eq!(p.render().anim, Anim::HangCursor);
    // 두 손(hang_cursor grip)이 커서 끝에
    let (gx, gy) = p.cfg.grip.hang_cursor.at(0.0);
    let (_, top) = p.window_pos();
    assert!((p.body.x + gx * p.cfg.size - cx).abs() < 0.5 && (top + gy * p.cfg.size - cy).abs() < 0.5);
}

#[test]
fn a_passing_cursor_or_one_beside_it_or_other_pets_never_grab() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 2, &w);
    let (cx, cy) = above_head(&p);
    // 0.6초가 되기 전에 떠난다
    for _ in 0..30 {
        tick(&mut p, &w, cx, cy, false);
    }
    for _ in 0..30 {
        tick(&mut p, &w, cx + 400.0, cy, false);
    }
    assert!(!matches!(p.state, State::Swing { .. }));
    // 옆 (머리 위가 아니다)
    let mut p = resting(heumul(), 800.0, 3, &w);
    for _ in 0..120 {
        tick(&mut p, &w, 800.0 + 120.0, 1040.0 - 60.0, false);
    }
    assert!(!matches!(p.state, State::Swing { .. }));
    let mut cat = resting(pet_cfg(), 800.0, 4, &w);
    for _ in 0..120 {
        tick(&mut cat, &w, 800.0, 1040.0 - 128.0, false);
    }
    assert!(!matches!(cat.state, State::Swing { .. }), "흐물이만");
}

#[test]
fn never_while_busy_disabled_away_focused_or_cooling_down() {
    let w = world(vec![]);
    for case in 0..5 {
        let mut p = resting(heumul(), 800.0, 5, &w);
        match case {
            0 => p.set_user_away(true),
            1 => p.set_focus(true),
            2 => p.swing.ready_at = p.clock + 100.0,
            _ => {}
        }
        let (cx, cy) = above_head(&p);
        for _ in 0..120 {
            // 3: 숨김·잡힘 (바쁨) · 4: 커서 장난 꺼 둠
            p.cursor_input(false, case == 3, case != 4);
            p.hover(cx, cy, false);
            p.cursor = (cx, cy);
            p.update(DT, &w);
        }
        assert!(!matches!(p.state, State::Swing { .. }), "case {case}");
    }
}

#[test]
fn clicks_go_through_while_jumping_and_hanging_and_right_after_letting_go() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 6, &w);
    let (cx, cy) = above_head(&p);
    let mut jumped = false;
    for _ in 0..80 {
        tick(&mut p, &w, cx, cy, false);
        if matches!(p.state, State::Swing { phase: SwingPhase::Jump { .. }, .. }) {
            jumped = true;
            assert!(p.cursor_through(), "뛰는 동안");
        }
    }
    assert!(jumped && hanging(&p) && p.cursor_through());
    tick(&mut p, &w, cx, cy, true);
    assert!(matches!(p.state, State::Air { .. }), "클릭하면 놓는다: {:?}", p.state);
    assert!(p.cursor_through(), "떨어지며 커서에서 벗어날 때까지");
}

#[test]
fn moving_the_cursor_swings_it_like_a_pendulum_and_it_settles() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 7, &w);
    let (cx, cy) = grab(&mut p, &w);
    p.state = State::Swing { phase: SwingPhase::Hang, remaining: 100.0 };
    // 오른쪽으로 쭉 끌었다가 멈춘다 (던질 만큼 빠르지는 않게)
    let mut x = cx;
    for _ in 0..20 {
        x += 8.0;
        tick(&mut p, &w, x, cy, false);
    }
    let mut angles = Vec::new();
    for _ in 0..180 {
        tick(&mut p, &w, x, cy, false);
        angles.push(p.render().swing);
    }
    let max = angles.iter().fold(0.0f64, |m, a| m.max(a.abs()));
    assert!(max > 5.0 && max <= swing_moves::MAX_SWING_DEG + 1e-9, "{max}");
    assert!(angles.iter().any(|&a| a > 1.0) && angles.iter().any(|&a| a < -1.0), "이쪽저쪽 흔들린다");
    assert!(!p.render().flip, "흔들리는 동안 뒤집지 않는다");
    for _ in 0..600 {
        tick(&mut p, &w, x, cy, false);
    }
    assert!(p.render().swing.abs() < max * 0.5, "잦아든다");
    // 몸은 커서를 따라왔다
    let (gx, _) = p.cfg.grip.hang_cursor.at(0.0);
    assert!((p.body.x + gx * p.cfg.size - x).abs() < 0.5);
}

#[test]
fn it_says_wheee_and_a_fast_swing_makes_it_happy() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 8, &w);
    let (cx, cy) = above_head(&p);
    let mut said = Vec::new();
    for _ in 0..80 {
        tick(&mut p, &w, cx, cy, false);
        said.extend(p.take_speech());
    }
    assert!(said.contains(&Speech::Swing), "{said:?}");
    let (mut hearts, mut more) = (false, false);
    for i in 0..600 {
        let x = cx + 60.0 * (i as f64 * DT * 2.0 * std::f64::consts::PI / 1.1).sin();
        tick(&mut p, &w, x, cy, false);
        hearts |= p.render().heart;
        more |= p.take_speech() == Some(Speech::More);
        if !hanging(&p) {
            break;
        }
    }
    assert!(hearts && more, "흔들수록 신난다 (하트, More!)");
}

#[test]
fn it_lets_go_after_8_to_20_seconds_and_then_waits_three_minutes() {
    let w = world(vec![]);
    for seed in 0..6 {
        let mut p = resting(heumul(), 800.0, 10 + seed, &w);
        let (cx, cy) = grab(&mut p, &w);
        let start = p.clock;
        while hanging(&p) && p.clock - start < 30.0 {
            tick(&mut p, &w, cx, cy, false);
        }
        let held = p.clock - start;
        assert!(held >= HANG_SECS.0 - 0.5 && held <= HANG_SECS.1 + 0.1, "{held}");
        assert!(matches!(p.state, State::Air { .. }));
        assert!(p.swing.ready_at - p.clock >= COOLDOWN_SECS.0 - 1.0);
        // 내려서서 다시 머리 위에 커서 — 쿨다운 동안은 안 잡는다
        for _ in 0..(60 * 20) {
            let (hx, hy) = above_head(&p);
            tick(&mut p, &w, hx, hy, false);
            assert!(!matches!(p.state, State::Swing { .. }));
        }
    }
}

#[test]
fn shaking_the_cursor_hard_flings_it_with_that_velocity() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 20, &w);
    let (cx, cy) = grab(&mut p, &w);
    let fast = FLING_SPEED * 1.3 * DT; // px / 틱
    let mut x = cx;
    for _ in 0..10 {
        x -= fast;
        tick(&mut p, &w, x, cy - fast * 0.3, false);
        if !hanging(&p) {
            break;
        }
    }
    assert!(matches!(p.state, State::Air { anim: Anim::Fall }), "{:?}", p.state);
    assert!(p.thrown, "사람이 던진 것과 같다 (친구 끝을 넘으면 친구에게)");
    assert!(p.body.vx < -500.0, "휘두른 쪽으로 날아간다: {}", p.body.vx);
}

#[test]
fn it_lets_go_when_the_cursor_leaves_the_screen_or_goes_over_a_fullscreen_app() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 800.0, 21, &w);
    let (_, cy) = grab(&mut p, &w);
    tick(&mut p, &w, 3000.0, cy, false);
    assert!(matches!(p.state, State::Air { .. }), "화면 밖");
    let full = world(vec![super::tests::win(9, 0.0, 0.0, 1920.0, 1080.0)]);
    let mut p = resting(heumul(), 800.0, 22, &w);
    let (cx, cy) = grab(&mut p, &w);
    tick(&mut p, &full, cx, cy, false);
    assert!(matches!(p.state, State::Air { .. }), "전체 화면 앱 위");
}
