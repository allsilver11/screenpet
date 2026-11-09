//! 커서 장난 · 창 흔들림 · 창 닫힘 (brain_cursor.rs). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::cursor_moves::*;
use super::tests::{cfg, place, sim_cfg, w1, win, world};
use super::*;
use crate::geom::Rect;
use crate::test_util::{DT, LoopLimit, idle, species};

fn tick(p: &mut Pet, w: &World, cursor: (f64, f64)) {
    p.hover(cursor.0, cursor.1, false);
    p.cursor_input(false, false, true);
    p.update(DT, w);
}

/// 쿨다운이 끝난 채 땅(x = 800)에 쉬고 있는 동물
fn ready_pet(c: PetConfig, seed: u64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 800.0, 1040.0, seed);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    p.cursor_mood.ready_at = 0.0;
    (p, w)
}

/// 커서가 가만히 있는 동안 장난을 시작할 때까지 (최대 30초). 시작했으면 true
fn wait_for_play(p: &mut Pet, w: &World, cursor: (f64, f64)) -> bool {
    for _ in 0..1800 {
        tick(p, w, cursor);
        if matches!(p.state, State::CursorPlay { .. }) {
            return true;
        }
        if !matches!(p.state, State::Idle { .. }) {
            p.state = idle();
        }
    }
    false
}

/// 시뮬레이션 설정 + 그 동물의 몸놀림 (pet.json locomotion에 맞춘다)
fn species_pet(id: &str, seed: u64) -> (Pet, World) {
    let c = PetConfig { flies: matches!(id, "ghost" | "bird"), swift: id == "bird", hops: id == "frog", ..cfg() };
    let (mut p, w) = ready_pet(c, seed);
    p.set_cursor_species(id);
    (p, w)
}

/// slots::window_logical_size와 같은 창 (왼쪽, 오른쪽) — 몸이 창 가운데
fn window_span(p: &Pet) -> (f64, f64) {
    let width = p.cfg.size.max(128.0 * p.cfg.dpi);
    (p.body.x - width / 2.0, p.body.x + width / 2.0)
}

// ── 흔들기 재기 ──

#[test]
fn shake_meter_counts_reversals_of_big_enough_swings_inside_the_window() {
    let mut m = ShakeMeter::new(10.0, 0.8);
    // 한쪽으로 쭉: 뒤집힘 없음
    for i in 0..30 {
        m.push(i as f64 * 0.01, i as f64 * 5.0, 0.0);
    }
    assert_eq!(m.reversals(), 0);
    // 왔다 갔다 세 번
    let mut t = 0.3;
    for x in [100.0, 150.0, 100.0] {
        m.push(t, x, 0.0);
        t += 0.05;
    }
    assert_eq!(m.reversals(), 3);
    // 시간이 지나면 잊는다
    m.push(5.0, 100.0, 0.0);
    assert_eq!(m.reversals(), 0);
}

#[test]
fn shake_meter_ignores_tiny_jitter_and_reads_the_vertical_axis_too() {
    let mut m = ShakeMeter::new(10.0, 0.8);
    for i in 0..20 {
        m.push(i as f64 * 0.02, if i % 2 == 0 { 0.0 } else { 3.0 }, 0.0);
    }
    assert_eq!(m.reversals(), 0);
    let mut v = ShakeMeter::new(10.0, 0.8);
    for (i, y) in [0.0, 40.0, 0.0, 40.0, 0.0].iter().enumerate() {
        v.push(i as f64 * 0.05, 0.0, *y);
    }
    assert_eq!(v.reversals(), 3);
}

// ── 언제 시작하나 ──

#[test]
fn a_resting_cursor_nearby_makes_the_pet_playful_and_says_so() {
    let (mut p, w) = ready_pet(cfg(), 1);
    assert!(wait_for_play(&mut p, &w, (1000.0, 900.0)));
    assert_eq!(p.take_speech(), Some(Speech::CursorPlay));
}

#[test]
fn it_never_starts_when_off_busy_dragging_controlled_or_otherwise_occupied() {
    let cursor = (900.0, 900.0);
    let attempts: [(&str, fn(&mut Pet)); 6] = [
        ("control", |p| p.set_control(true)),
        ("hand", |p| p.hand = Some((1.0, 1.0))),
        ("home", |p| p.home_trip = Some(HomeTrip::new(Owner::Ground(0), 100.0, 800.0))),
        ("asleep", |p| p.state = State::Sleep { remaining: 1000.0 }),
        ("cooling down", |p| p.cursor_mood.ready_at = 1e9),
        ("back to the house after a meal", |p| p.return_home = Some((Owner::Ground(0), 100.0))),
    ];
    for (name, setup) in attempts {
        let (mut p, w) = ready_pet(cfg(), 2);
        setup(&mut p);
        for _ in 0..600 {
            tick(&mut p, &w, cursor);
            assert!(!matches!(p.state, State::CursorPlay { .. } | State::CursorLeap { .. } | State::CursorHang { .. }), "{name}");
        }
    }
    for (left, busy, on) in [(true, false, true), (false, true, true), (false, false, false)] {
        let (mut p, w) = ready_pet(cfg(), 3);
        for _ in 0..1200 {
            p.hover(cursor.0, cursor.1, false);
            p.cursor_input(left, busy, on);
            p.update(DT, &w);
            assert!(!matches!(p.state, State::CursorPlay { .. }), "left {left} busy {busy} on {on}");
            if !matches!(p.state, State::Idle { .. }) {
                p.state = idle();
            }
        }
    }
}

#[test]
fn without_the_app_feeding_the_cursor_nothing_ever_starts() {
    // 다른 테스트·시뮬레이션의 난수 순서를 건드리지 않는다
    let (mut p, w) = ready_pet(cfg(), 4);
    let dice = p.rng.get_seed();
    for _ in 0..1200 {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::CursorPlay { .. }));
    }
    assert_eq!(p.rng.get_seed(), dice, "no dice were rolled while it rested");
    // 비교: 같은 자리에서 커서를 먹이면 주사위를 굴린다
    let (mut q, w) = ready_pet(cfg(), 4);
    let dice = q.rng.get_seed();
    for _ in 0..1200 {
        tick(&mut q, &w, (900.0, 900.0));
    }
    assert_ne!(q.rng.get_seed(), dice);
}

#[test]
fn a_far_or_fast_or_low_cursor_does_not_tempt_it() {
    let (mut p, w) = ready_pet(cfg(), 5);
    // 너무 멀리
    for _ in 0..900 {
        tick(&mut p, &w, (1800.0, 900.0));
        assert!(!matches!(p.state, State::CursorPlay { .. }));
        p.state = idle();
    }
    // 빠르게 움직이는 중
    let (mut p, w) = ready_pet(cfg(), 6);
    for i in 0..900 {
        let x = 700.0 + (i % 40) as f64 * 15.0;
        tick(&mut p, &w, (x, 900.0));
        assert!(!matches!(p.state, State::CursorPlay { .. }));
        p.state = idle();
    }
    // 창 위에 서 있는데 커서는 발 아래
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 400.0, 7);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.cursor_mood.ready_at = 0.0;
    for _ in 0..900 {
        tick(&mut p, &w, (800.0, 700.0));
        assert!(!matches!(p.state, State::CursorPlay { .. }));
        p.state = idle();
    }
}

// ── 종마다 다른 놀이 ──

#[test]
fn every_species_has_its_own_style_with_animations_it_really_has() {
    let all = species();
    let styles: Vec<CursorStyle> = all.iter().map(|id| CursorStyle::of(id).unwrap_or_else(|| panic!("{id}"))).collect();
    for (i, a) in styles.iter().enumerate() {
        for (j, b) in styles.iter().enumerate() {
            assert!(i == j || a != b, "{} and {} play the same", all[i], all[j]);
        }
    }
    assert!(CursorStyle::of("mystery").is_none());
    for (id, style) in all.iter().zip(&styles) {
        let path = format!("{}/../../ui/pets/{id}/pet.json", env!("CARGO_MANIFEST_DIR"));
        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for anim in style.anims() {
            let name = serde_json::to_value(anim).unwrap();
            let name = name.as_str().unwrap();
            assert!(manifest["animations"].get(name).is_some(), "{id} has no {name}");
        }
    }
}

/// 다람쥐: 호기심 많고 날쌔다 — 깡충깡충 빠르게 와서 고개를 갸웃
#[test]
fn squirrel_scampers_over_in_quick_hops_and_tilts_its_head() {
    let s = CursorStyle::of("squirrel").expect("squirrel");
    // anims(): 걸음 · 뛰기 · 째려보기 · (준비) · 도착
    assert_eq!(s.anims(), vec![Anim::Hop, Anim::Hop, Anim::Annoyed, Anim::Tilt]);
}

#[test]
fn every_species_gets_to_the_cursor_and_hangs_from_it() {
    for (n, id) in species().iter().enumerate() {
        let (mut p, w) = species_pet(id, 100 + n as u64);
        // 몸 둘 옆, 머리 위 몸 하나쯤 높이
        let cursor = (1056.0, 1040.0 - 128.0 * 1.6);
        assert!(wait_for_play(&mut p, &w, cursor), "{id} never played");
        let mut hung = false;
        for _ in 0..(60 * 20) {
            tick(&mut p, &w, cursor);
            if matches!(p.state, State::CursorHang { .. }) {
                hung = true;
                break;
            }
        }
        assert!(hung, "{id} never reached the cursor: {:?}", p.state);
        assert_eq!(p.render().anim, Anim::Dragged, "{id}");
    }
}

#[test]
fn walkers_stay_on_their_surface_while_approaching() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 400.0, 8);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.cursor_mood.ready_at = 0.0;
    p.set_cursor_species("duck");
    let cursor = (1250.0, 200.0);
    assert!(wait_for_play(&mut p, &w, cursor));
    let mut limit = LoopLimit::new();
    while matches!(p.state, State::CursorPlay { .. }) {
        limit.step();
        tick(&mut p, &w, cursor);
        if matches!(p.state, State::CursorPlay { .. }) {
            assert_eq!(p.body.y, 400.0);
            assert!(p.body.x >= 500.0 + p.cfg.half_w - 0.01 && p.body.x <= 1100.0 - p.cfg.half_w + 0.01, "x {}", p.body.x);
        }
    }
}

#[test]
fn hoppers_bounce_on_the_way_and_flyers_leave_their_perch() {
    let (mut p, w) = species_pet("rabbit", 9);
    let cursor = (1300.0, 900.0);
    assert!(wait_for_play(&mut p, &w, cursor));
    let mut lifted = false;
    for _ in 0..60 {
        tick(&mut p, &w, cursor);
        if matches!(p.state, State::CursorPlay { phase: PlayPhase::Approach, .. }) {
            lifted |= p.window_pos().1 < p.body.y - p.cfg.size - 1.0;
            assert_eq!(p.body.y, 1040.0, "only the sprite is lifted");
        }
    }
    assert!(lifted);

    let (mut p, w) = species_pet("ghost", 10);
    let cursor = (1000.0, 700.0);
    assert!(wait_for_play(&mut p, &w, cursor));
    assert_eq!(p.standing, None);
}

#[test]
fn a_cursor_too_high_makes_it_give_up_looking_up() {
    let (mut p, w) = ready_pet(PetConfig { looks_up: true, ..cfg() }, 11);
    // 머리 위 3.5몸 — 알아채지만 닿지 않는다
    let cursor = (800.0, 1040.0 - 128.0 * 4.3);
    assert!(wait_for_play(&mut p, &w, cursor));
    for _ in 0..(60 * 15) {
        tick(&mut p, &w, cursor);
        assert!(!matches!(p.state, State::CursorHang { .. } | State::CursorLeap { .. }));
        if matches!(p.state, State::Idle { .. }) {
            break;
        }
    }
    assert_eq!(p.render().anim, Anim::LookUp);
}

// ── 매달리기 ──

fn hanging_pet(c: PetConfig, seed: u64, cursor: (f64, f64)) -> (Pet, World) {
    let (mut p, w) = ready_pet(c, seed);
    assert!(wait_for_play(&mut p, &w, cursor));
    for _ in 0..(60 * 20) {
        tick(&mut p, &w, cursor);
        if matches!(p.state, State::CursorHang { .. }) {
            return (p, w);
        }
    }
    panic!("never hung: {:?}", p.state);
}

#[test]
fn the_pet_hangs_right_under_the_cursor_and_the_mouse_always_passes_through() {
    for (size, dpi) in [(128.0, 1.0), (58.0, 1.0), (211.0, 1.0), (256.0, 2.0), (96.0, 1.5)] {
        let c = PetConfig { size, half_w: size * 0.3, dpi, hang_grip: (0.08, 0.06), ..cfg() };
        let start = (800.0 + size * 0.5, 1040.0 - size * 1.5);
        let (mut p, w) = hanging_pet(c, 20, start);
        assert!(p.cursor_through());
        assert_ne!(p.render().hang, 0);
        // 화면 한가운데: 커서 끝 = 머리 꼭대기 (창이 커서를 덮어도 클릭은 통과한다)
        let cursor = (960.0, 300.0);
        p.state = State::CursorHang { remaining: 4.0 };
        p.hover(cursor.0, cursor.1, false);
        p.cursor_input(false, false, true);
        p.update(DT, &w);
        assert!((p.body.x + size * 0.08 - cursor.0).abs() < 1e-6, "size {size}: x {}", p.body.x);
        assert!((p.body.y - size * 0.94 - cursor.1).abs() < 1e-6, "size {size}: y {}", p.body.y);
        let (left, right) = window_span(&p);
        assert!(cursor.0 >= left && cursor.0 < right);
        assert!(p.cursor_through());
        // 화면 끝에서는 안쪽으로 밀려 잘리지 않는다
        for cursor in [start, (5.0, 5.0), (1915.0, 500.0), (1919.0, 1039.0), (960.0, 1039.0), (0.0, 1039.0), (1900.0, 2.0)] {
            p.state = State::CursorHang { remaining: 4.0 };
            p.hover(cursor.0, cursor.1, false);
            p.cursor_input(false, false, true);
            p.update(DT, &w);
            let (left, right) = window_span(&p);
            assert!(p.body.y <= 1040.0, "off the bottom: {}", p.body.y);
            assert!(left >= -0.01 && right <= 1920.01, "off the side: {left}..{right}");
            assert!(p.cursor_through());
        }
    }
}

#[test]
fn the_mouse_keeps_passing_through_until_the_falling_pet_has_cleared_the_cursor_or_landed() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 40, cursor);
    p.state = State::CursorHang { remaining: 0.001 };
    let mut fell = false;
    let mut landed = false;
    let mut cleared = false;
    let mut falling_over_cursor = 0;
    for _ in 0..(60 * 8) {
        tick(&mut p, &w, cursor);
        if matches!(p.state, State::CursorHang { .. }) {
            assert!(p.cursor_through());
            continue;
        }
        if !fell {
            fell = true;
            assert!(matches!(p.state, State::Air { .. }));
            assert!(p.cursor_through(), "still under the cursor on the first falling tick");
        }
        if matches!(p.state, State::Air { .. }) {
            // 떨어지는 동안: 창이 커서를 덮는 사이엔 계속 통과, 벗어나면 다음 틱부터 다시 클릭을 받는다
            if p.window_covers_cursor() {
                assert!(p.cursor_through(), "{:?}", p.body);
                falling_over_cursor += 1;
            } else {
                assert!(!cleared || !p.cursor_through(), "{:?}", p.body);
                cleared = true;
            }
        } else {
            landed = true;
            assert!(!p.cursor_through(), "off again after landing: {:?}", p.state);
        }
    }
    assert!(fell && landed && cleared);
    assert!(falling_over_cursor > 1, "checks the middle of the fall too: {falling_over_cursor}");
    assert!(!p.cursor_through());
}

#[test]
fn a_flyer_gliding_onto_the_cursor_already_lets_the_mouse_through() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = ready_pet(PetConfig { flies: true, ..cfg() }, 41);
    assert!(wait_for_play(&mut p, &w, cursor));
    let mut saw_glide = false;
    for _ in 0..(60 * 20) {
        tick(&mut p, &w, cursor);
        match p.state {
            State::CursorPlay { phase: PlayPhase::Approach, .. } => {
                saw_glide = true;
                assert!(p.cursor_through());
            }
            State::CursorHang { .. } => break,
            _ => {}
        }
    }
    assert!(saw_glide);
}

#[test]
fn the_mouse_passes_through_during_the_leap_too_but_not_on_the_way_or_after() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = ready_pet(cfg(), 21);
    assert!(!p.cursor_through());
    assert!(wait_for_play(&mut p, &w, cursor));
    assert!(!p.cursor_through());
    let mut saw_leap = false;
    for _ in 0..(60 * 20) {
        tick(&mut p, &w, cursor);
        match p.state {
            State::CursorPlay { .. } => assert!(!p.cursor_through()),
            State::CursorLeap { .. } => {
                saw_leap = true;
                assert!(p.cursor_through());
            }
            State::CursorHang { .. } => break,
            _ => {}
        }
    }
    assert!(saw_leap);
    for _ in 0..(60 * 6) {
        tick(&mut p, &w, cursor);
    }
    assert!(!p.cursor_through());
}

#[test]
fn it_follows_the_cursor_then_lets_go_within_five_seconds_and_falls() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 22, cursor);
    p.update(DT, &w);
    // 커서를 천천히 옮기면 따라온다
    let mut c = cursor;
    let x0 = p.body.x;
    for _ in 0..30 {
        c.0 -= 2.0;
        tick(&mut p, &w, c);
    }
    assert!((p.body.x - (x0 - 60.0)).abs() < 3.0, "{} vs {}", p.body.x, x0 - 60.0);
    let mut secs = 30.0 * DT;
    let mut limit = LoopLimit::new();
    while matches!(p.state, State::CursorHang { .. }) {
        limit.step();
        tick(&mut p, &w, c);
        secs += DT;
        assert!(secs <= HANG_SECS.1 + 0.1, "hung too long");
    }
    assert!(matches!(p.state, State::Air { .. }));
    for _ in 0..240 {
        tick(&mut p, &w, c);
    }
    assert_eq!(p.body.y, 1040.0, "landed");
}

#[test]
fn shaking_the_mouse_shakes_it_off_early_with_a_whoa() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 23, cursor);
    p.take_speech();
    let mut x = cursor.0;
    let mut dir = 1.0;
    let mut ticks = 0;
    let mut limit = LoopLimit::new();
    while matches!(p.state, State::CursorHang { .. }) {
        limit.step();
        if ticks % 5 == 4 {
            dir = -dir;
        }
        x += dir * 25.0;
        tick(&mut p, &w, (x, cursor.1));
        ticks += 1;
        assert!(ticks < 60, "a hard shake takes well under a second");
    }
    assert!(matches!(p.state, State::Air { .. }));
    assert_eq!(p.take_speech(), Some(Speech::Whoa));
}

#[test]
fn a_gentle_back_and_forth_does_not_shake_it_off() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 24, cursor);
    p.state = State::CursorHang { remaining: 3.0 };
    for i in 0..120 {
        let x = cursor.0 + ((i / 10) % 2) as f64 * 20.0;
        tick(&mut p, &w, (x, cursor.1));
        assert!(matches!(p.state, State::CursorHang { .. }));
    }
}

#[test]
fn a_cursor_snatched_away_mid_leap_makes_it_miss() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = ready_pet(cfg(), 25);
    assert!(wait_for_play(&mut p, &w, cursor));
    let mut limit = LoopLimit::new();
    while !matches!(p.state, State::CursorLeap { .. }) {
        limit.step();
        tick(&mut p, &w, cursor);
    }
    let away = (100.0, 100.0);
    let mut limit = LoopLimit::new();
    while matches!(p.state, State::CursorLeap { .. }) {
        limit.step();
        tick(&mut p, &w, away);
    }
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
}

#[test]
fn turning_it_off_or_grabbing_or_calling_it_home_ends_the_hang() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 26, cursor);
    p.hover(cursor.0, cursor.1, false);
    p.cursor_input(false, false, false);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }));

    // 창이 아직 마우스를 통과시키기 전에 눌렸어도 평소처럼 잡힌다
    let (mut p, _w) = hanging_pet(cfg(), 27, cursor);
    p.pointer_down(cursor.0, cursor.1);
    assert!(p.is_held());
    assert!(!p.cursor_through());

    let (mut p, _w) = hanging_pet(cfg(), 28, cursor);
    p.go_home(Owner::Ground(0), 100.0);
    assert!(matches!(p.state, State::Air { .. }));
}

#[test]
fn holding_the_button_while_it_hangs_changes_nothing_clicks_are_the_users() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = hanging_pet(cfg(), 29, cursor);
    p.state = State::CursorHang { remaining: 2.0 };
    for _ in 0..60 {
        p.hover(cursor.0, cursor.1, false);
        p.cursor_input(true, true, true);
        p.update(DT, &w);
        assert!(matches!(p.state, State::CursorHang { .. }));
    }
}

#[test]
fn pressing_the_button_on_the_way_ends_the_play() {
    let cursor = (1300.0, 900.0);
    let (mut p, w) = ready_pet(cfg(), 30);
    assert!(wait_for_play(&mut p, &w, cursor));
    p.hover(cursor.0, cursor.1, false);
    p.cursor_input(true, false, true);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Idle { .. }), "{:?}", p.state);
}

#[test]
fn control_mode_stops_the_play() {
    let cursor = (1300.0, 900.0);
    let (mut p, w) = ready_pet(cfg(), 31);
    assert!(wait_for_play(&mut p, &w, cursor));
    p.remote_keys(true, Default::default());
    tick(&mut p, &w, cursor);
    assert!(!matches!(p.state, State::CursorPlay { .. }));

    let (mut p, w) = hanging_pet(cfg(), 32, (1000.0, 800.0));
    p.remote_keys(true, Default::default());
    tick(&mut p, &w, (1000.0, 800.0));
    assert!(!matches!(p.state, State::CursorHang { .. }));
}

#[test]
fn the_cooldown_lasts_two_to_four_minutes() {
    let cursor = (1000.0, 800.0);
    let (mut p, w) = ready_pet(cfg(), 33);
    let before = p.clock;
    assert!(wait_for_play(&mut p, &w, cursor));
    let wait = p.cursor_mood.ready_at - before;
    assert!((COOLDOWN_SECS.0..=COOLDOWN_SECS.1 + 31.0).contains(&wait), "{wait}");
    assert!(COOLDOWN_SECS.0 >= 120.0 && COOLDOWN_SECS.1 <= 240.0);
}

// ── 창 흔들림 ──

/// 창 1을 좌우로 흔든다 (틱마다 x만 — 앱의 patch_windows처럼)
fn shake_window(p: &mut Pet, w: &mut World, ticks: usize, amp: f64, every: usize) {
    let base = w1().rect;
    for i in 0..ticks {
        let dx = if (i / every) % 2 == 0 { 0.0 } else { amp };
        w.patch_windows(&[(1, Rect::new(base.left + dx, base.top, base.right + dx, base.bottom))]);
        p.update(DT, w);
    }
}

fn on_window(c: PetConfig, seed: u64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(c, 800.0, 400.0, seed);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    (p, w)
}

#[test]
fn shaking_the_window_throws_most_pets_off_and_some_cling_to_the_edge() {
    let (mut fell, mut clung) = (0, 0);
    for seed in 0..200 {
        let (mut p, mut w) = on_window(cfg(), seed);
        shake_window(&mut p, &mut w, 30, 60.0, 3);
        assert_eq!(p.take_speech(), Some(Speech::Whoa), "seed {seed}: {:?}", p.state);
        match p.state {
            State::Air { .. } => fell += 1,
            State::EdgeCling { .. } => clung += 1,
            s => panic!("seed {seed}: {s:?}"),
        }
    }
    assert!((90..=150).contains(&fell), "fell {fell}");
    assert!((50..=110).contains(&clung), "clung {clung}");
}

#[test]
fn a_tumble_falls_past_the_shaken_window_to_what_is_below() {
    for seed in 0..40 {
        let (mut p, mut w) = on_window(cfg(), seed);
        shake_window(&mut p, &mut w, 30, 60.0, 3);
        if !matches!(p.state, State::Air { .. }) {
            continue;
        }
        for _ in 0..180 {
            p.update(DT, &w);
        }
        assert_eq!(p.body.y, 1040.0, "seed {seed} lands on the ground, not back on the window");
        return;
    }
    panic!("nobody fell");
}

#[test]
fn clinging_rides_the_window_then_climbs_back_or_drops() {
    let (mut back, mut dropped) = (0, 0);
    for seed in 0..100 {
        let (mut p, mut w) = on_window(cfg(), seed);
        shake_window(&mut p, &mut w, 30, 60.0, 3);
        let State::EdgeCling { window, .. } = p.state else { continue };
        assert_eq!(window, 1);
        assert_eq!(p.ridden_window(), Some(1), "follows the window every tick");
        assert_eq!(p.render().anim, Anim::Dragged);
        let moved = Rect::new(600.0, 300.0, 1200.0, 800.0);
        w.patch_windows(&[(1, moved)]);
        p.update(DT, &w);
        assert!(p.body.y > 300.0 && p.body.y < 300.0 + 128.0, "dangles below the top edge: {}", p.body.y);
        let mut secs = 0.0;
        let mut limit = LoopLimit::new();
        while matches!(p.state, State::EdgeCling { .. }) {
            limit.step();
            p.update(DT, &w);
            secs += DT;
            assert!(secs < QUAKE_CLING_SECS + 0.1);
        }
        if p.standing.map(|s| s.owner) == Some(Owner::Window(1)) {
            assert_eq!(p.body.y, 300.0);
            back += 1;
        } else {
            assert!(matches!(p.state, State::Air { .. }));
            dropped += 1;
        }
    }
    assert!(back > 0 && dropped > 0, "back {back} dropped {dropped}");
}

#[test]
fn a_window_closed_while_clinging_drops_the_pet() {
    for seed in 0..40 {
        let (mut p, mut w) = on_window(cfg(), seed);
        shake_window(&mut p, &mut w, 30, 60.0, 3);
        if !matches!(p.state, State::EdgeCling { .. }) {
            continue;
        }
        let gone = world(vec![]);
        p.update(DT, &gone);
        assert!(matches!(p.state, State::Air { .. }));
        return;
    }
    panic!("nobody clung");
}

/// 그대로 창 위에 있다: 떨어져 바닥에 서거나 끝에 매달리지 않았고 "으아!"도 없다
fn assert_stayed_on_window(p: &mut Pet) {
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "{:?}", p.state);
    assert!(!matches!(p.state, State::EdgeCling { .. } | State::Air { .. }), "{:?}", p.state);
    assert_ne!(p.take_speech(), Some(Speech::Whoa));
}

#[test]
fn moving_a_window_normally_or_trembling_it_does_nothing() {
    // 한 방향으로 끌기
    let (mut p, mut w) = on_window(cfg(), 40);
    let base = w1().rect;
    for i in 0..120 {
        let dx = i as f64 * 4.0;
        w.patch_windows(&[(1, Rect::new(base.left + dx, base.top, base.right + dx, base.bottom))]);
        p.update(DT, &w);
        assert_stayed_on_window(&mut p);
    }
    // 천천히 왔다 갔다 (0.8초에 뒤집힘 세 번 안 됨)
    let (mut p, mut w) = on_window(cfg(), 41);
    for _ in 0..4 {
        shake_window(&mut p, &mut w, 60, 60.0, 40);
        assert_stayed_on_window(&mut p);
    }
    // 아주 작은 떨림
    let (mut p, mut w) = on_window(cfg(), 42);
    for _ in 0..6 {
        shake_window(&mut p, &mut w, 10, 2.0, 3);
        assert_stayed_on_window(&mut p);
    }
}

#[test]
fn a_perched_flyer_just_takes_off() {
    let c = PetConfig { flies: true, ..cfg() };
    let (mut p, mut w) = on_window(c, 43);
    shake_window(&mut p, &mut w, 30, 60.0, 3);
    assert_eq!(p.standing, None);
    assert_eq!(p.take_speech(), Some(Speech::Whoa));
    assert!(!matches!(p.state, State::EdgeCling { .. }));
}

#[test]
fn held_or_eating_pets_are_not_shaken_off() {
    let (mut p, mut w) = on_window(cfg(), 44);
    p.state = State::Eating { remaining: 100.0, favorite: false, level_up: false };
    shake_window(&mut p, &mut w, 30, 60.0, 3);
    assert!(matches!(p.state, State::Eating { .. }));

    let (mut p, mut w) = on_window(cfg(), 45);
    p.pointer_down(800.0, 350.0);
    shake_window(&mut p, &mut w, 30, 60.0, 3);
    assert!(p.is_held());
}

// ── 창 닫힘 ──

#[test]
fn a_closed_window_makes_it_eek_fall_and_glare_after_landing() {
    let (mut p, _) = on_window(cfg(), 50);
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert_eq!(p.take_speech(), Some(Speech::Eek));
    p.update(DT, &gone);
    assert!(matches!(p.state, State::Air { .. }));
    let mut glared = 0.0;
    let mut said = false;
    for _ in 0..(60 * 5) {
        p.update(DT, &gone);
        said |= p.take_speech() == Some(Speech::Hmph);
        if matches!(p.state, State::Glare { .. }) {
            assert_eq!(p.render().anim, Anim::Annoyed);
            assert_eq!(p.body.y, 1040.0);
            glared += DT;
        }
    }
    assert!(said);
    assert!((glared - GLARE_SECS).abs() < 0.1, "glared {glared}");
    assert!(!matches!(p.state, State::Glare { .. }));
}

#[test]
fn a_cat_huffs_when_it_glares() {
    let (mut p, _) = on_window(sim_cfg(), 51);
    p.set_cursor_species("cat");
    let gone = world(vec![]);
    for _ in 0..(60 * 2) {
        p.update(DT, &gone);
        if matches!(p.state, State::Glare { .. }) {
            assert_eq!(p.render().anim, Anim::Huff);
            return;
        }
    }
    panic!("never glared");
}

#[test]
fn a_window_merely_covered_is_not_a_closed_window() {
    // 앞 창에 가려졌다 — 떨어지긴 해도 놀라거나 째려보지 않는다
    let (mut p, _) = on_window(cfg(), 52);
    let covered = world(vec![win(2, 400.0, 200.0, 1200.0, 900.0), w1()]);
    let mut speeches = vec![];
    for _ in 0..(60 * 4) {
        p.update(DT, &covered);
        speeches.extend(p.take_speech());
        assert!(!matches!(p.state, State::Glare { .. }));
    }
    assert!(!speeches.contains(&Speech::Eek));
}

#[test]
fn a_perched_flyer_glares_right_where_it_floats() {
    let c = PetConfig { flies: true, ..cfg() };
    let (mut p, _) = on_window(c, 53);
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert_eq!(p.take_speech(), Some(Speech::Eek));
    assert_eq!(p.standing, None);
    assert!(matches!(p.state, State::Glare { .. }));
}

#[test]
fn being_picked_up_while_falling_cancels_the_glare() {
    let (mut p, _) = on_window(cfg(), 54);
    let gone = world(vec![]);
    p.update(DT, &gone);
    p.update(DT, &gone);
    p.pointer_down(800.0, 400.0);
    p.update(DT, &gone);
    p.pointer_up(800.0, 400.0);
    for _ in 0..(60 * 3) {
        p.update(DT, &gone);
        assert!(!matches!(p.state, State::Glare { .. }));
    }
}

// ── 조작 모드 ──

#[test]
fn a_shaken_window_does_not_take_a_controlled_pet_off_its_keys() {
    for c in [cfg(), PetConfig { flies: true, ..cfg() }] {
        for seed in 0..12 {
            let (mut p, mut w) = on_window(c, seed);
            p.set_control(true);
            shake_window(&mut p, &mut w, 30, 60.0, 3);
            assert!(p.take_speech() != Some(Speech::Whoa), "seed {seed}");
            assert!(!matches!(p.state, State::EdgeCling { .. } | State::Air { .. }), "seed {seed}: {:?}", p.state);
        }
    }
}

#[test]
fn a_controlled_walker_only_falls_when_its_window_closes() {
    let (mut p, _) = on_window(cfg(), 50);
    p.set_control(true);
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert_eq!(p.take_speech(), Some(Speech::Eek));
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    for _ in 0..(60 * 5) {
        p.update(DT, &gone);
        assert!(!matches!(p.state, State::Glare { .. }), "{:?}", p.state);
    }
    assert_eq!(p.body.y, 1040.0, "landed");
    // 내려선 뒤 바로 키를 따른다
    let x = p.body.x;
    let right = control_moves::ControlInput { right: true, ..Default::default() };
    for _ in 0..30 {
        p.control_keys(right, DT);
        p.update(DT, &gone);
    }
    assert!(p.body.x > x, "{} -> {}", x, p.body.x);
}

#[test]
fn a_controlled_flyer_does_not_glare_when_its_window_closes() {
    let (mut p, _) = on_window(PetConfig { flies: true, ..cfg() }, 53);
    p.set_control(true);
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert_eq!(p.standing, None, "floats up");
    assert!(!matches!(p.state, State::Glare { .. }), "{:?}", p.state);
}

#[test]
fn turning_control_on_ends_a_glare_or_a_cling_that_was_running() {
    let (mut p, w) = on_window(cfg(), 7);
    p.state = State::Glare { remaining: GLARE_SECS };
    p.set_control(true);
    p.update(DT, &w);
    assert!(!matches!(p.state, State::Glare { .. }), "{:?}", p.state);
    assert!(p.standing.is_some(), "stays on its window");

    let (mut p, w) = on_window(cfg(), 7);
    p.standing = None;
    p.body.y = w1().rect.top + 0.7 * p.cfg.size; // 매달린 자리: 윗면 아래
    p.state = State::EdgeCling { window: 1, rel_x: 100.0, remaining: QUAKE_CLING_SECS };
    p.set_control(true);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }), "lets go and falls: {:?}", p.state);
}

#[test]
fn control_turned_on_during_the_fall_cancels_the_glare_waiting_for_the_landing() {
    let (mut p, _) = on_window(cfg(), 50);
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    p.set_control(true);
    for _ in 0..(60 * 5) {
        p.update(DT, &gone);
        assert!(!matches!(p.state, State::Glare { .. }), "{:?}", p.state);
    }
}
