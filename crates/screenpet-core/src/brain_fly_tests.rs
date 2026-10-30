//! 꿀벌 (spec 2B §10) — 날기·내려앉기·쏘기 brain 동작 테스트. brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, run, sim_cfg, w1, win, world};
use super::*;
use crate::care::{FeedResult, Food};
use crate::flight::{self, Goal};
use crate::sting::StingPhase;

const DT: f64 = 1.0 / 60.0;

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, ..cfg() }
}

fn sim_bee() -> PetConfig {
    PetConfig { flies: true, stings: true, walk_speed: 70.0, ..sim_cfg() }
}

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// (x, y)에 떠 있는 벌
fn hovering(x: f64, y: f64, state: State, seed: u64) -> Pet {
    let mut p = Pet::new(bee(), x, y, seed);
    p.state = state;
    p
}

/// 바깥에서 나는 범위 (몸·머리 위 말풍선 자리가 화면 안)
fn limits(p: &Pet, w: &World) -> Rect {
    flight::fly_bounds(&w.bounds, p.cfg.half_w, p.cfg.size * (1.0 + flight::TOP_SPACE))
}

fn run_until(p: &mut Pet, w: &World, ticks: usize, done: impl Fn(&Pet) -> bool) -> bool {
    for _ in 0..ticks {
        p.update(DT, w);
        if done(p) {
            return true;
        }
    }
    false
}

/// 커서를 40px씩 좌우로 흔들며 몸 위를 간질인다 — 쏘기가 시작되면 멈춘다
fn tickle_until_sting(p: &mut Pet, w: &World, ticks: usize) -> bool {
    for i in 0..ticks {
        let (x, y) = (p.body.x, p.body.y - 60.0);
        p.hover(x + if i % 2 == 0 { 0.0 } else { 40.0 }, y, true);
        p.update(DT, w);
        if matches!(p.state, State::Sting { .. }) {
            return true;
        }
    }
    false
}

fn click(p: &mut Pet, w: &World) {
    let (x, y) = (p.body.x, p.body.y - 60.0);
    p.pointer_down(x, y);
    p.update(DT, w);
    p.pointer_up(x, y);
}

/// 10분 — 늘 화면 안, 대개 날고, 가끔 내려앉아 쉬고, 잠은 내려앉았을 때만. 걷기·화면 타기·뛰어들기 없음
#[test]
fn bee_ten_minutes_flies_within_bounds_perches_and_sleeps_only_perched() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0), win(2, 1300.0, 350.0, 1800.0, 700.0)]);
    let (mut flying, mut perched, mut sleeping, mut rolls, mut landings, mut total) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for seed in 0..8 {
        let mut p = Pet::new(sim_bee(), 960.0, 128.0, seed);
        let b = limits(&p, &w);
        let mut was_perched = false;
        for tick in 0..(60 * 600) {
            p.update(DT, &w);
            let (x, y) = (p.body.x, p.body.y);
            assert!(x >= b.left - 1e-9 && x <= b.right + 1e-9, "seed {seed}: x {x} {:?}", p.state);
            assert!(y >= b.top - 1e-9 && y <= b.bottom + 1e-9, "seed {seed}: y {y} {:?}", p.state);
            let anim = p.render().anim;
            if let State::Move { .. } = p.state {
                assert!(p.standing.is_none(), "seed {seed}: walking {:?}", p.standing);
            }
            match anim {
                Anim::Walk | Anim::Run | Anim::Roll => flying += 1,
                Anim::Sleep => {
                    assert!(p.standing.is_some(), "seed {seed}: sleeping in the air");
                    sleeping += 1;
                }
                Anim::Climb | Anim::Slide | Anim::Hop => panic!("seed {seed}: {anim:?}"),
                Anim::Fall => assert!(tick < 5, "seed {seed}: fell at {tick}"),
                _ => {}
            }
            rolls += (anim == Anim::Roll) as usize;
            assert!(p.inside_window().is_none());
            let now = p.standing.is_some();
            perched += now as usize;
            landings += (now && !was_perched) as usize;
            was_perched = now;
            total += 1;
        }
    }
    let (f, s, pr) = (flying as f64 / total as f64, sleeping as f64 / total as f64, perched as f64 / total as f64);
    assert!((0.35..=0.85).contains(&f), "flying {f:.3} perched {pr:.3} sleeping {s:.3}");
    assert!(pr > 0.05 && s > 0.0 && rolls > 0, "perched {pr:.3} sleeping {s:.3} rolls {rolls}");
    assert!(landings >= 8 * 5, "landings {landings}");
}

#[test]
fn hovering_bee_does_not_fall() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 300.0, idle(), 1);
    for _ in 0..300 {
        p.update(DT, &w);
        assert!((p.body.y - 300.0).abs() <= 128.0 * flight::BOB_AMP + 1e-9, "y {}", p.body.y);
        assert!(p.standing.is_none() && !matches!(p.state, State::Air { .. }));
    }
    assert_eq!(p.body.x, 800.0);
}

#[test]
fn flying_never_falls_or_walks_on_a_surface() {
    let w = world(vec![w1()]);
    // 창 윗면 바로 위에서 날기 시작해도 디딤판에 내려서지 않는다 (내려앉기로 정한 게 아니면)
    let mut p = hovering(600.0, 399.0, idle(), 1);
    p.start_flight(Goal::Point(1000.0, 399.0), Move::Walk);
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.flight.is_none()));
    assert!(p.standing.is_none());
    assert!((p.body.x - 1000.0).abs() < 1.0 && (p.body.y - 399.0).abs() < 6.0, "{:?}", p.body);
}

#[test]
fn flight_speed_is_walk_speed_and_fast_flight_is_faster() {
    let w = world(vec![]);
    for (kind, mult) in [(Move::Walk, 1.0), (Move::Run, 1.8)] {
        let mut p = hovering(300.0, 500.0, idle(), 1);
        p.start_flight(Goal::Point(1800.0, 500.0), kind);
        run(&mut p, &w, 60);
        let s = p.flight.unwrap().s;
        assert!((s - 60.0 * mult).abs() < 1.5, "{kind:?} {s}");
        assert_eq!(p.facing, 1.0);
        assert_eq!(p.render().anim, Anim::from(kind));
    }
}

#[test]
fn thrown_bee_glides_to_a_stop_and_hovers() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 300.0, idle(), 1);
    p.pointer_down(800.0, 250.0);
    for i in 1..=6 {
        p.update(DT, &w);
        p.pointer_move(800.0 + 20.0 * i as f64, 250.0);
    }
    p.pointer_up(920.0, 250.0);
    assert_eq!(p.state, State::Air { anim: Anim::Fall });
    let mut max_y: f64 = 0.0;
    assert!(run_until(&mut p, &w, 120, |p| !matches!(p.state, State::Air { .. })), "still drifting {:?}", p.body);
    for _ in 0..120 {
        p.update(DT, &w);
        max_y = max_y.max(p.body.y);
        assert!(p.standing.is_none());
    }
    assert!(max_y <= 300.0 + 128.0 * flight::BOB_AMP + 1e-9, "fell to {max_y}");
    assert!(p.body.x > 950.0 && p.body.x < 1400.0, "x {}", p.body.x);
}

#[test]
fn bee_perches_on_a_window_top_and_rides_it() {
    let w = world(vec![w1()]);
    let mut p = hovering(300.0, 250.0, idle(), 1);
    p.start_flight(Goal::Perch { owner: Owner::Window(1), rel_x: 300.0 }, Move::Walk);
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.standing.is_some()));
    assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
    assert_eq!((p.body.x, p.body.y), (800.0, 400.0));
    assert!(matches!(p.state, State::Land { .. }));
    let moved = world(vec![win(1, 600.0, 450.0, 1200.0, 950.0)]);
    p.update(DT, &moved);
    assert_eq!((p.body.x, p.body.y), (900.0, 450.0));
}

#[test]
fn perch_target_follows_a_moving_window() {
    let w = world(vec![w1()]);
    let mut p = hovering(300.0, 250.0, idle(), 1);
    p.start_flight(Goal::Perch { owner: Owner::Window(1), rel_x: 300.0 }, Move::Walk);
    run(&mut p, &w, 30);
    let moved = world(vec![win(1, 700.0, 500.0, 1300.0, 1000.0)]);
    assert!(run_until(&mut p, &moved, 60 * 20, |p| p.standing.is_some()));
    assert_eq!((p.body.x, p.body.y), (1000.0, 500.0));
}

#[test]
fn perched_bee_takes_off_to_move_and_when_its_perch_goes() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(bee(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Move { kind: Move::Walk, remaining: 10.0 });
    p.update(DT, &w);
    assert!(p.standing.is_none() && p.flight.is_some(), "{:?}", p.state);
    // 자다가 창이 사라지면 떨어지지 않고 떠오른다
    let mut p = Pet::new(bee(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 10.0 });
    p.update(DT, &world(vec![]));
    assert!(p.standing.is_none());
    assert!(matches!(p.state, State::Idle { .. }), "{:?}", p.state);
    run(&mut p, &world(vec![]), 30);
    assert!((p.body.y - 400.0).abs() <= 128.0 * flight::BOB_AMP + 1e-9);
}

#[test]
fn about_a_third_of_hovering_rests_go_perch() {
    let w = world(vec![w1()]);
    let n = 2000;
    let mut perches = 0;
    for seed in 0..n {
        let mut p = hovering(800.0, 300.0, idle(), seed);
        let next = p.rest_state(&w);
        assert!(!matches!(next, State::Trick { .. } | State::Sleep { .. }), "{next:?}");
        if matches!(p.flight, Some(f) if matches!(f.goal, Goal::Perch { .. })) {
            assert!(matches!(next, State::Move { .. }));
            perches += 1;
        }
    }
    let share = perches as f64 / n as f64;
    assert!((0.31..=0.39).contains(&share), "{share}");
}

#[test]
fn perched_bee_rests_with_tricks_and_sometimes_sleeps() {
    let w = world(vec![w1()]);
    let (mut tricks, mut sleeps) = (0, 0);
    for seed in 0..400 {
        let mut p = Pet::new(bee(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tricks += matches!(p.rest_state(&w), State::Trick { .. }) as usize;
        sleeps += matches!(p.sleep_or_move(&w), State::Sleep { .. }) as usize;
    }
    assert!((180..=260).contains(&tricks), "tricks {tricks}");
    assert!((110..=170).contains(&sleeps), "sleeps {sleeps}");
    // 떠 있으면 자지 않는다
    for seed in 0..200 {
        let mut p = hovering(800.0, 300.0, idle(), seed);
        assert!(!matches!(p.sleep_or_move(&w), State::Sleep { .. }));
    }
}

#[test]
fn roll_is_a_loop_de_loop_on_the_way() {
    let w = world(vec![]);
    let mut p = hovering(300.0, 700.0, idle(), 1);
    p.start_flight(Goal::Point(1500.0, 700.0), Move::Roll);
    let (mut looped, mut min_y, mut start_y) = (0, f64::MAX, None);
    assert_eq!(p.render().anim, Anim::Walk);
    while p.flight.is_some() {
        p.update(DT, &w);
        if p.render().anim == Anim::Roll {
            looped += 1;
            min_y = min_y.min(p.body.y);
            start_y.get_or_insert(p.body.y);
        }
    }
    // 반지름 0.75 몸 = 96px 원 → 192px 위까지, 둘레 603px ÷ 72px/s ≈ 8.4초
    assert!((480..=540).contains(&looped), "loop ticks {looped}");
    let start_y = start_y.unwrap();
    assert!(min_y < start_y - 180.0, "loop top {min_y} from {start_y}");
    assert!((p.body.x - 1500.0).abs() < 1.0);
}

#[test]
fn bee_flies_home_in_an_arc_and_goes_in() {
    let w = world(vec![w1()]);
    let mut p = hovering(300.0, 300.0, idle(), 1);
    p.go_home(Owner::Ground(0), 1500.0);
    let mut ticks = 0;
    while !p.is_housed() {
        p.update(DT, &w);
        ticks += 1;
        assert!(ticks < 60 * 20, "never got home: {:?} {:?}", p.state, p.body);
        if !p.is_housed() {
            assert_eq!(p.state, State::GoHome);
            assert!(p.standing.is_none() && p.body.y < 1040.0);
        }
    }
    assert_eq!((p.body.x, p.body.y), (1500.0, 1040.0));
    // 하우스에서 나오면 날아 나간다
    p.leave_house(&w, 1500.0, Owner::Ground(0));
    run(&mut p, &w, 30);
    assert!(p.standing.is_none() && !p.is_housed());
    assert!(matches!(p.state, State::Move { .. }) && p.flight.is_some(), "{:?}", p.state);
}

#[test]
fn bee_dragged_on_the_way_home_flies_home_from_where_it_was_dropped() {
    let w = world(vec![]);
    let mut p = hovering(300.0, 300.0, idle(), 1);
    p.go_home(Owner::Ground(0), 1500.0);
    run(&mut p, &w, 60);
    p.pointer_down(p.body.x, p.body.y - 40.0);
    p.pointer_move(1700.0, 160.0);
    run(&mut p, &w, 30);
    p.pointer_up(1700.0, 160.0);
    let mut prev = (p.body.x, p.body.y);
    let mut ticks = 0;
    while !p.is_housed() {
        p.update(DT, &w);
        // 놓인 자리에서 이어서 난다 (예전 길로 순간이동하지 않는다)
        assert!((p.body.x - prev.0).hypot(p.body.y - prev.1) < 10.0, "jump {prev:?} -> {:?}", p.body);
        prev = (p.body.x, p.body.y);
        ticks += 1;
        assert!(ticks < 60 * 20);
    }
}

#[test]
fn perched_bee_goes_home_by_air_too() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(bee(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.go_home(Owner::Ground(0), 1500.0);
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.is_housed()));
}

/// 오른쪽 끝(1920)으로 날아가는 벌: (나간 수, 안 나간 수)
fn edge_flights(n: u64, edges: (bool, bool)) -> (usize, usize) {
    let w = world(vec![]);
    let (mut exits, mut stays) = (0, 0);
    for seed in 0..n {
        let mut p = hovering(1500.0, 500.0, idle(), seed);
        p.set_exit_edges(edges.0, edges.1);
        p.start_flight(Goal::Point(1882.0, 500.0), Move::Walk);
        if run_until(&mut p, &w, 60 * 20, |p| matches!(p.state, State::Exit { .. }) || p.flight.is_none()) && matches!(p.state, State::Exit { .. }) {
            exits += 1;
        } else {
            stays += 1;
        }
    }
    (exits, stays)
}

#[test]
fn bee_near_a_connected_edge_flies_out_about_30_percent() {
    let (exits, stays) = edge_flights(400, (false, true));
    assert!((90..=150).contains(&exits), "exits {exits}");
    assert_eq!(exits + stays, 400);
    assert_eq!(edge_flights(100, (true, false)).0, 0);
}

#[test]
fn exiting_bee_flies_off_the_screen_and_reports_once() {
    let w = world(vec![]);
    let mut p = (0..100)
        .map(|seed| {
            let mut p = hovering(1500.0, 500.0, idle(), seed);
            p.set_exit_edges(false, true);
            p.start_flight(Goal::Point(1882.0, 500.0), Move::Walk);
            run_until(&mut p, &w, 60 * 20, |p| matches!(p.state, State::Exit { .. }) || p.flight.is_none());
            p
        })
        .find(|p| matches!(p.state, State::Exit { .. }))
        .expect("some seed exits");
    let y = p.body.y;
    assert!(run_until(&mut p, &w, 60 * 10, |p| p.is_away()));
    assert!(p.body.x - p.cfg.half_w >= 1920.0 && (p.body.y - y).abs() < 1e-9);
    assert_eq!(p.take_exit(), Some(Side::Right));
    assert_eq!(p.take_exit(), None);
}

#[test]
fn guest_bee_flies_in_from_the_edge() {
    let w = world(vec![w1()]);
    let mut p = Pet::new_entering(bee(), &w, Side::Right, 3);
    assert!(matches!(p.state, State::Enter { .. }));
    assert!(p.body.x - p.cfg.half_w >= 1920.0 && p.standing.is_none());
    assert!(p.body.y < 1040.0 - 200.0);
    let mut ticks = 0;
    while matches!(p.state, State::Enter { .. }) {
        p.update(DT, &w);
        ticks += 1;
        assert!(ticks < 60 * 10 && p.standing.is_none());
        assert_eq!(p.facing, -1.0);
    }
    assert!(p.body.x + p.cfg.half_w <= 1920.0);
    assert!(matches!(p.state, State::Move { .. }), "{:?}", p.state);
}

#[test]
fn homesick_guest_bee_flies_home() {
    let w = world(vec![w1()]);
    for seed in 0..10 {
        let mut p = Pet::new_entering(bee(), &w, Side::Left, seed);
        p.set_guest(Side::Left, 3.0);
        assert!(run_until(&mut p, &w, 60 * 90, |p| p.is_away()), "seed {seed}: {:?} {:?}", p.state, p.body);
        assert_eq!(p.take_exit(), Some(Side::Left));
    }
}

#[test]
fn fed_hovering_bee_flies_down_and_eats_perched() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 250.0, idle(), 1);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
    assert!(p.is_eating());
    assert!(run_until(&mut p, &w, 60 * 20, |p| matches!(p.state, State::Eating { .. })));
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!(p.body.y, 400.0);
    assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }));
    // 드래그 중·던져지는 중엔 먹지 않는다
    let mut held = hovering(800.0, 250.0, idle(), 1);
    held.pointer_down(800.0, 200.0);
    assert!(!held.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
    // 배부르면 떠서 갸웃
    let mut full = hovering(800.0, 250.0, idle(), 1);
    assert!(full.feed_result(FeedResult::Refused, Food::Meal));
    assert!(matches!(full.state, State::Trick { kind: Trick::Tilt, .. }) && full.standing.is_none());
}

#[test]
fn bee_dropped_into_a_window_flies_inside_it_then_leaves() {
    let w = world(vec![w1()]);
    let mut p = hovering(300.0, 300.0, idle(), 1);
    p.pointer_down(300.0, 260.0);
    p.pointer_move(800.0, 760.0);
    assert_eq!(p.state, State::Dragged);
    run(&mut p, &w, 30);
    p.pointer_up(800.0, 760.0);
    p.update(DT, &w);
    assert_eq!(p.inside_window(), Some(1));
    let r = w1().rect;
    let mut left = false;
    for _ in 0..(60 * 200) {
        p.update(DT, &w);
        if p.inside_window().is_none() {
            left = true;
            break;
        }
        assert!(p.body.x - p.cfg.half_w >= r.left && p.body.x + p.cfg.half_w <= r.right, "x {}", p.body.x);
        assert!(p.body.y <= r.bottom && p.body.y - p.cfg.size >= r.top - 1e-9, "y {} {:?}", p.body.y, p.state);
    }
    assert!(left, "never left the window");
}

#[test]
fn hovered_flying_bee_stops_and_looks_at_the_cursor() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.start_flight(Goal::Point(1500.0, 500.0), Move::Walk);
    for _ in 0..20 {
        p.hover(p.body.x - 30.0, p.body.y - 60.0, true);
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Idle { look: true, .. }), "{:?}", p.state);
    assert_eq!(p.facing, -1.0);
    assert_eq!(p.render().anim, Anim::Look);
}

// ---- 쏘기 ----

/// 쏘기를 끝까지 돌리며 (콕! 틱, 쏜 지점, 들린 말풍선들)
fn run_sting(p: &mut Pet, w: &World) -> (usize, (f64, f64), Vec<Speech>) {
    let (mut hit, mut at, mut said) = (None, None, Vec::new());
    for tick in 0..(60 * 3) {
        p.update(DT, w);
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        if let Some(point) = p.take_sting() {
            assert!(hit.is_none(), "stung twice");
            hit = Some(tick);
            at = Some(point);
        }
        if !matches!(p.state, State::Sting { .. }) {
            break;
        }
    }
    (hit.expect("never hit"), at.unwrap(), said)
}

#[test]
fn tickled_bee_gets_angry_and_stings_the_screen() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    assert!(tickle_until_sting(&mut p, &w, 60));
    assert_eq!(p.state, State::Sting { phase: StingPhase::Alert, remaining: StingPhase::Alert.secs() });
    assert_eq!(p.render(), RenderState { anim: Anim::Sting, flip: false, heart: false, poof: false, ctl: false });
    let (x, y) = (p.body.x, p.body.y);
    let (hit, at, said) = run_sting(&mut p, &w);
    // "!" 0.5초 + 돌진 0.3초 뒤 콕!
    assert!((47..=49).contains(&hit), "hit at tick {hit}");
    assert!((at.0 - x).abs() < 1e-6 && (at.1 - (y - 0.55 * 128.0)).abs() < 6.0, "{at:?}");
    assert_eq!(said, vec![Speech::Sting, Speech::Grumpy]);
    assert!(p.take_sting().is_none());
    assert!(!matches!(p.state, State::Sting { .. }));
}

#[test]
fn recoil_looks_grumpy_and_the_bee_flies_on_after_1_5_s() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.start_sting();
    assert!(run_until(&mut p, &w, 60, |p| p.sting_at.is_some()));
    assert!(p.take_sting().is_some());
    let mut ticks = 0;
    while matches!(p.state, State::Sting { .. }) {
        if let State::Sting { phase: StingPhase::Recoil, .. } = p.state {
            assert_eq!(p.render().anim, Anim::Annoyed);
        }
        p.update(DT, &w);
        ticks += 1;
    }
    assert!((88..=92).contains(&ticks), "{ticks}");
    // 사이 박자(떠서) 뒤 다시 난다
    assert!(run_until(&mut p, &w, 120, |p| matches!(p.state, State::Move { .. })), "{:?}", p.state);
}

#[test]
fn bee_stings_at_most_every_8_seconds() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    assert!(tickle_until_sting(&mut p, &w, 60));
    let started = p.clock;
    run_sting(&mut p, &w);
    // 쿨다운 중엔 간질이면 투덜대기만 한다
    assert!(!tickle_until_sting(&mut p, &w, 60 * 2));
    assert!(matches!(p.state, State::Tickled { .. }), "{:?}", p.state);
    // 쿨다운이 끝나면 다시 쏜다
    let wait = (60.0 * (started + 8.0 - p.clock)) as usize;
    run(&mut p, &w, wait);
    assert!(tickle_until_sting(&mut p, &w, 60 * 2));
    assert!(p.clock - started >= 8.0);
}

#[test]
fn cooldown_tickle_grumbles_instead_of_giggling() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.sting_ready_at = 100.0;
    for i in 0..30 {
        p.hover(p.body.x + if i % 2 == 0 { 0.0 } else { 40.0 }, p.body.y - 60.0, true);
        p.update(DT, &w);
        if matches!(p.state, State::Tickled { .. }) {
            break;
        }
    }
    assert!(matches!(p.state, State::Tickled { .. }));
    assert_eq!(p.take_speech(), Some(Speech::Grumpy));
}

#[test]
fn shaking_a_held_bee_makes_it_sting_and_escape() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.pointer_down(800.0, 450.0);
    let mut stung = false;
    for i in 1..=60 {
        p.update(DT, &w);
        p.pointer_move(800.0 + if i % 2 == 0 { 0.0 } else { 15.0 }, 450.0);
        if matches!(p.state, State::Sting { .. }) {
            stung = true;
            // 600px ÷ 15px = 40번째 움직임 근처
            assert!((38..=42).contains(&i), "stung at {i}");
            break;
        }
    }
    assert!(stung);
    assert!(!p.is_held());
    let (_, at, _) = run_sting(&mut p, &w);
    assert!((at.0 - p.body.x).abs() < 20.0);
}

#[test]
fn flinging_a_bee_across_the_screen_is_not_shaking() {
    let w = world(vec![]);
    let mut p = hovering(300.0, 500.0, idle(), 1);
    p.pointer_down(300.0, 450.0);
    for i in 1..=30 {
        p.update(DT, &w);
        p.pointer_move(300.0 + 40.0 * i as f64, 450.0);
        assert_eq!(p.state, State::Dragged);
    }
    p.pointer_up(1500.0, 450.0);
    assert_eq!(p.state, State::Air { anim: Anim::Fall });
}

#[test]
fn slow_dragging_does_not_sting() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.pointer_down(800.0, 450.0);
    for i in 1..=180 {
        p.update(DT, &w);
        p.pointer_move(800.0 + 5.0 * i as f64, 450.0);
        assert_eq!(p.state, State::Dragged);
    }
}

#[test]
fn three_quick_clicks_make_a_bee_sting_but_slow_clicks_do_not() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    click(&mut p, &w);
    run(&mut p, &w, 20);
    click(&mut p, &w);
    run(&mut p, &w, 20);
    click(&mut p, &w);
    assert!(matches!(p.state, State::Sting { .. }), "{:?}", p.state);

    let mut slow = hovering(800.0, 500.0, idle(), 1);
    for _ in 0..4 {
        click(&mut slow, &w);
        assert!(!matches!(slow.state, State::Sting { .. }));
        run(&mut slow, &w, 70);
    }
}

#[test]
fn perched_bee_stings_where_it_sits() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(bee(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    assert!(tickle_until_sting(&mut p, &w, 60));
    let (_, at, _) = run_sting(&mut p, &w);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert!((at.0 - 800.0).abs() < 1e-9 && (at.1 - (400.0 - 0.55 * 128.0)).abs() < 1e-9);
}

#[test]
fn walkers_never_sting() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    assert!(!tickle_until_sting(&mut p, &w, 60));
    assert!(matches!(p.state, State::Tickled { .. }));
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    for _ in 0..3 {
        click(&mut p, &w);
        run(&mut p, &w, 10);
        if matches!(p.state, State::Air { .. }) {
            run(&mut p, &w, 60);
        }
        assert!(!matches!(p.state, State::Sting { .. }));
    }
    p.pointer_down(p.body.x, p.body.y - 60.0);
    for i in 1..=60 {
        p.update(DT, &w);
        p.pointer_move(800.0 + if i % 2 == 0 { 0.0 } else { 30.0 }, 300.0);
    }
    assert_eq!(p.state, State::Dragged);
    assert!(p.take_sting().is_none());
    // 벌인데 쏘지 않는 동물(sting 동작 없음)도 쏘지 않는다
    let mut fly_only = hovering(800.0, 300.0, idle(), 1);
    fly_only.cfg.stings = false;
    assert!(!tickle_until_sting(&mut fly_only, &w, 60));
}

#[test]
fn bee_does_not_sting_on_the_way_home() {
    let w = world(vec![]);
    let mut p = hovering(800.0, 500.0, idle(), 1);
    p.go_home(Owner::Ground(0), 1500.0);
    assert!(!tickle_until_sting(&mut p, &w, 60));
    assert_eq!(p.state, State::GoHome);
}

#[test]
fn bee_switching_to_a_walker_falls_and_a_walker_switching_to_a_bee_takes_off() {
    let w = world(vec![w1()]);
    let mut p = hovering(200.0, 300.0, idle(), 1);
    p.cfg = cfg();
    assert!(run_until(&mut p, &w, 120, |p| p.standing.is_some()));
    assert_eq!(p.body.y, 1040.0);
    p.cfg = bee();
    p.state = State::Move { kind: Move::Walk, remaining: 5.0 };
    p.update(DT, &w);
    assert!(p.standing.is_none());
}

// ---- fix round 1 ----

#[test]
fn clicks_during_the_sting_wind_up_cannot_cancel_it() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 250.0, idle(), 1);
    p.start_sting();
    let mut hits = 0;
    // 0.5초 동안 다섯 번 빠르게 누른다 (누른 채 끌기도 한 번)
    for i in 0..5 {
        let (x, y) = (p.body.x, p.body.y - 60.0);
        p.pointer_down(x, y);
        assert!(!p.is_held(), "click {i}: {:?}", p.state);
        if i == 2 {
            p.pointer_move(x + 50.0, y + 50.0);
        }
        for _ in 0..6 {
            p.update(DT, &w);
            hits += p.take_sting().iter().count();
        }
        p.pointer_up(x, y);
    }
    for _ in 0..(60 * 3) {
        p.update(DT, &w);
        hits += p.take_sting().iter().count();
    }
    assert_eq!(hits, 1);
}

#[test]
fn hungry_bee_clicked_for_food_does_not_sting() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 250.0, idle(), 1);
    p.set_hungry(true);
    for _ in 0..4 {
        click(&mut p, &w);
        assert!(!matches!(p.state, State::Sting { .. }), "{:?}", p.state);
        for _ in 0..6 {
            p.update(DT, &w);
        }
    }
    assert!(p.take_food_request());
    assert!(p.take_sting().is_none());
}

#[test]
fn sting_started_above_the_flying_area_eases_back_instead_of_jumping() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 250.0, idle(), 1);
    let top = limits(&p, &w).top;
    p.body.y = top - 200.0;
    p.start_sting();
    p.update(DT, &w);
    let jump = p.body.y - (top - 200.0);
    assert!(jump > 0.0 && jump < 60.0, "jump {jump}");
    run(&mut p, &w, 30);
    assert!((p.body.y - top).abs() < 1.0, "{} vs {top}", p.body.y);
}

#[test]
fn switching_a_fed_bee_to_a_walker_keeps_the_meal_but_drops_flight_state() {
    let w = world(vec![w1()]);
    let mut p = hovering(800.0, 250.0, idle(), 1);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal));
    run(&mut p, &w, 5);
    assert!(p.meal_pending.is_some() && p.flight.is_some());
    // 동물 바꾸기: 걷는 동물이 된다
    p.cfg = cfg();
    p.update(DT, &w);
    assert!(p.meal_pending.is_none() && p.flight.is_none() && p.hover_at.is_none());
    assert!(p.is_eating(), "the meal still waits");
    assert!(run_until(&mut p, &w, 60 * 15, |p| matches!(p.state, State::Eating { .. })), "{:?}", p.state);
    assert!(run_until(&mut p, &w, 60 * 10, |p| !p.is_eating()), "{:?}", p.state);
}

// ---- 꼬불꼬불 비행 (v0.3.3) ----

/// 10분 비행 통계 (실제 화면 크기·배율, 창 여러 개 — 하나는 최대화)
#[derive(Debug, Default)]
struct FlyStats {
    left: f64,
    right: f64,
    up: f64,
    down: f64,
    /// 날면서 방향이 바뀐 양 (rad)
    turned: f64,
    /// 날아간 거리 (몸 크기 단위)
    path: f64,
    fly_secs: f64,
    flights: usize,
    /// 날기마다 (곧은 거리 / 실제 거리)의 합
    straightness: f64,
    /// 한 바퀴(2π) 넘게 감은 날기
    circles: usize,
    /// 같은 날기 안에서 좌우가 바뀐 횟수, 그 사이 가장 짧은 시간
    flips: usize,
    min_flip_gap: f64,
    max_step: f64,
    /// 원·8자·살랑을 도는 동안 방향이 바뀐 양, 그 시간
    fig_turned: f64,
    fig_secs: f64,
    /// 원 모양 날기에서 원을 도는 시간과 바퀴 수 (감은 양 / 2π)
    ring_secs: f64,
    ring_turns: f64,
}

fn real_screen(dpi: f64) -> (World, PetConfig) {
    let (sw, sh) = if dpi == 1.0 { (1920.0, 1040.0) } else { (2560.0, 1400.0) };
    let wins = vec![
        win(1, 0.0, 0.0, sw, sh),
        win(2, 0.12 * sw, 0.08 * sh, 0.75 * sw, 0.85 * sh),
        win(3, 0.55 * sw, 0.3 * sh, 0.95 * sw, 0.95 * sh),
    ];
    let w = World::new(wins, vec![Rect::new(0.0, 0.0, sw, sh)], 128.0 * dpi);
    let cfg = PetConfig { size: 128.0 * dpi, half_w: 38.0 * dpi, walk_speed: 50.0 * dpi, dpi, ..bee() };
    (w, cfg)
}

fn fly_stats(dpi: f64, seeds: u64) -> FlyStats {
    use std::f64::consts::{PI, TAU};
    let (w, cfg) = real_screen(dpi);
    let mut st = FlyStats { min_flip_gap: f64::MAX, ..FlyStats::default() };
    for seed in 0..seeds {
        // 하우스(오른쪽 아래)에서 나온 것처럼 바닥에서 시작
        let mut p = Pet::new(cfg, 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), w.bounds.right - 200.0 * dpi, idle());
        let b = limits(&p, &w);
        let mut prev = (p.body.x, p.body.y);
        let (mut heading, mut wind, mut wmin, mut wmax): (Option<f64>, f64, f64, f64) = (None, 0.0, 0.0, 0.0);
        let (mut start, mut walked) = (None::<(f64, f64)>, 0.0);
        let (mut facing, mut last_flip) = (p.facing, 0.0);
        for tick in 0..(60 * 600) {
            let was_flying = p.flight.is_some() && matches!(p.state, State::Move { .. });
            p.update(DT, &w);
            let pos = (p.body.x, p.body.y);
            let (dx, dy) = (pos.0 - prev.0, pos.1 - prev.1);
            st.max_step = st.max_step.max(dx.hypot(dy));
            assert!(pos.0 >= b.left - 1e-9 && pos.0 <= b.right + 1e-9 && pos.1 >= b.top - 1e-9 && pos.1 <= b.bottom + 1e-9, "seed {seed}: {pos:?}");
            let flying = p.flight.is_some() && matches!(p.state, State::Move { .. });
            if flying && was_flying {
                if dx < 0.0 { st.left -= dx } else { st.right += dx }
                if dy < 0.0 { st.up -= dy } else { st.down += dy }
                st.fly_secs += DT;
                walked += dx.hypot(dy);
                // 공중제비(세로 원)는 따로 — 여기선 새 비행 길만 잰다
                let looping = p.flight.is_some_and(|f| f.looping.is_some());
                if looping {
                    heading = None;
                } else if dx.hypot(dy) > 1e-9 {
                    let h = dy.atan2(dx);
                    if let Some(ph) = heading {
                        let dh = (h - ph + PI).rem_euclid(TAU) - PI;
                        st.turned += dh.abs();
                        if let Some(f) = p.flight.filter(|f| f.path.is_some_and(|q| q.in_figure())) {
                            st.fig_turned += dh.abs();
                            st.fig_secs += DT;
                            if f.style == flight::Style::Circle {
                                st.ring_secs += DT;
                                st.ring_turns += dh.abs() / TAU;
                            }
                        }
                        wind += dh;
                        wmin = wmin.min(wind);
                        wmax = wmax.max(wind);
                    }
                    heading = Some(h);
                }
                if p.facing != facing {
                    st.flips += 1;
                    st.min_flip_gap = st.min_flip_gap.min(tick as f64 * DT - last_flip);
                    last_flip = tick as f64 * DT;
                }
                start.get_or_insert(prev);
            } else if let Some(s) = start.take() {
                st.flights += 1;
                if walked > 1e-6 {
                    st.straightness += (prev.0 - s.0).hypot(prev.1 - s.1) / walked;
                }
                st.path += walked / cfg.size;
                st.circles += (wmax - wmin >= TAU - 0.3) as usize;
                (heading, wind, wmin, wmax, walked) = (None, 0.0, 0.0, 0.0, 0.0);
            }
            if p.facing != facing && !(flying && was_flying) {
                last_flip = tick as f64 * DT;
            }
            facing = p.facing;
            prev = pos;
        }
    }
    st
}

/// 실제 화면(1920×1080, 2560×1440@1.5)에서: 곧게 위로만 오르지 않고 꼬불꼬불, 좌우·위아래 고르게, 가끔 원을 그린다
#[test]
fn bee_flies_curvy_balanced_paths_with_circles_on_real_screens() {
    for dpi in [1.0, 1.5] {
        let st = fly_stats(dpi, 6);
        let msg = format!("dpi {dpi}: {st:?}");
        let (lr, ud) = (st.left / st.right, st.up / st.down);
        assert!((0.75..=1.33).contains(&lr), "left/right {lr:.2} {msg}");
        assert!((0.75..=1.33).contains(&ud), "up/down {ud:.2} {msg}");
        // 방향이 계속 바뀐다: 몸 하나 날 때마다 평균 0.6 rad 넘게, 초당 0.25 rad 넘게 (예전: 0.40 rad/몸, 0.18 rad/s)
        let per_size = st.turned / st.path;
        let per_sec = st.turned / st.fly_secs;
        eprintln!("dpi {dpi}: lr {lr:.2} ud {ud:.2} turn {per_size:.2} rad/size {per_sec:.2} rad/s straight {:.2} circles {} flips {} gap {:.2} step {:.2}", st.straightness / st.flights as f64, st.circles, st.flips, st.min_flip_gap, st.max_step);
        // 원·8자·살랑을 도는 동안은 확실히 빙글: 초당 0.75 rad 넘게 (예전 원: 0.3 rad/s 남짓)
        let fig = st.fig_turned / st.fig_secs;
        let per_ring = st.ring_secs / st.ring_turns;
        eprintln!("dpi {dpi}: figures {fig:.2} rad/s, one circle {per_ring:.1} s");
        assert!(fig > 0.75, "figures {fig:.2} rad/s {msg}");
        // 원 한 바퀴는 걷는 빠르기에서도 몇 초 (예전: 10~16초)
        assert!((4.0..=8.0).contains(&per_ring), "one circle {per_ring:.1} s {msg}");
        assert!(per_size > 0.6 && per_sec > 0.25, "turn {per_size:.2} rad/size {per_sec:.2} rad/s {msg}");
        // 곧은 선이 아니다 (예전: 0.95)
        let straight = st.straightness / st.flights as f64;
        assert!(straight < 0.75, "straightness {straight:.2} {msg}");
        assert!(st.circles >= 10, "circles {} {msg}", st.circles);
        // 순간이동 없음, 좌우 뒤집기는 천천히
        assert!(st.max_step < 6.0 * dpi, "step {} {msg}", st.max_step);
        assert!(st.flips > 20 && st.min_flip_gap >= flight::FLIP_HOLD - 1e-6, "{msg}");
    }
}
