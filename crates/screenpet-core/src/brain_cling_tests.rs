//! 🦎 조작 모드 벽 붙기 (Space Space — brain_cling.rs): 화면 끝·창 옆면 벽이나 화면 유리에 붙어 방향키로 기어 다님. brain.rs의 자식 모듈.

use super::cling_moves::{ClingWall, CLING_BOOST};
use super::control_moves::ControlInput;
use super::feat_moves::Feat;
use super::swing_tests::resting;
use super::tests::{place, win, world};
use super::web_moves::WebPhase;
use super::*;
use crate::test_util::{bundled, DT};

/// 번들 pet.json 그대로의 설정 (청소년)
fn pet(id: &str) -> PetConfig {
    bundled().into_iter().find(|(i, _)| i == id).unwrap_or_else(|| panic!("ui/pets/{id}")).1
}

/// 키 묶음
fn keys(f: impl FnOnce(&mut ControlInput)) -> ControlInput {
    let mut k = ControlInput::default();
    f(&mut k);
    k
}

const NONE: ControlInput = ControlInput { left: false, right: false, up: false, down: false, jump: false, run: false };
const SPACE: ControlInput = ControlInput { jump: true, ..NONE };
const UP: ControlInput = ControlInput { up: true, ..NONE };
const DOWN: ControlInput = ControlInput { down: true, ..NONE };
const LEFT: ControlInput = ControlInput { left: true, ..NONE };
const RIGHT: ControlInput = ControlInput { right: true, ..NONE };

/// 바닥(x)에 쉬며 조작 모드가 켜진 펫 (오른쪽을 봄)
fn controlled(id: &str, x: f64, w: &World) -> Pet {
    let mut p = resting(pet(id), x, 7, w);
    p.hush_stage();
    p.bike.signal_ready_at = f64::INFINITY;
    p.facing = 1.0;
    p.set_control(true);
    p
}

/// 내 동물: 키를 누른 채 secs초
fn hold(p: &mut Pet, w: &World, k: ControlInput, secs: f64) {
    let n = ((secs / DT).round() as usize).max(1);
    for _ in 0..n {
        p.control_keys(k, DT);
        p.update(DT, w);
        assert!(p.body.x.is_finite() && p.body.y.is_finite(), "NaN body: {:?}", p.body);
    }
}

/// 키를 두 번 빠르게 톡톡
fn twice(p: &mut Pet, w: &World, k: ControlInput) {
    for _ in 0..2 {
        hold(p, w, k, DT);
        hold(p, w, NONE, DT);
    }
}

/// Space Space — 벽 붙기
fn cling(p: &mut Pet, w: &World) {
    twice(p, w, SPACE);
}

/// 내려설 때까지 키 없이 (최대 secs초)
fn settle(p: &mut Pet, w: &World, secs: f64) -> bool {
    for _ in 0..(secs / DT) as usize {
        hold(p, w, NONE, DT);
        if p.standing.is_some() && !matches!(p.state, State::Air { .. }) {
            return true;
        }
    }
    false
}

/// 기는 빠르기 (px/s)
fn speed(p: &Pet) -> f64 {
    p.scaled(behavior::CLIMB_SPEED)
}

#[test]
fn space_space_on_open_floor_clings_to_the_glass_and_all_four_arrows_crawl() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    assert!(p.standing.is_none());
    assert_eq!(p.render().anim, Anim::Climb);
    // 키가 없으면 그대로 (미끄러지지 않음)
    let (x, y) = (p.body.x, p.body.y);
    hold(&mut p, &w, NONE, 2.0);
    assert_eq!((p.body.x, p.body.y), (x, y));
    let v = speed(&p);
    hold(&mut p, &w, UP, 1.0);
    assert!((y - p.body.y - v).abs() < 2.0, "up {}", y - p.body.y);
    let (x, y) = (p.body.x, p.body.y);
    hold(&mut p, &w, RIGHT, 1.0);
    assert!((p.body.x - x - v).abs() < 2.0 && p.body.y == y);
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| { k.left = true; k.run = true; }), 1.0);
    assert!((x - p.body.x - v * CLING_BOOST).abs() < 2.0, "shift faster");
    let y = p.body.y;
    hold(&mut p, &w, DOWN, 0.3);
    assert!(p.body.y > y);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
}

#[test]
fn a_single_space_just_jumps_and_slow_taps_do_not_cling() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    hold(&mut p, &w, SPACE, DT);
    hold(&mut p, &w, NONE, DT);
    assert!(matches!(p.state, State::Air { .. }));
    assert!(settle(&mut p, &w, 3.0));
    assert_eq!(p.cling_on(), None);
    hold(&mut p, &w, SPACE, DT);
    hold(&mut p, &w, NONE, 0.5);
    hold(&mut p, &w, SPACE, DT);
    hold(&mut p, &w, NONE, DT);
    assert_eq!(p.cling_on(), None);
}

#[test]
fn down_onto_the_floor_steps_off_the_glass() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    hold(&mut p, &w, DOWN, 4.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert_eq!(p.cling_on(), None);
    // 다시 걸음
    let x = p.body.x;
    hold(&mut p, &w, RIGHT, 0.5);
    assert!(p.body.x > x);
}

#[test]
fn space_lets_go_and_does_not_count_as_a_new_double_tap() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 2.0);
    // 놓고 바로 한 번 더 — 다시 붙지 않고 떨어짐
    twice(&mut p, &w, SPACE);
    assert_eq!(p.cling_on(), None);
    assert!(matches!(p.state, State::Air { .. }));
    assert!(settle(&mut p, &w, 5.0));
}

#[test]
fn near_a_screen_edge_it_clings_to_the_wall_and_moves_onto_the_glass_away_from_it() {
    let w = world(vec![]);
    let c = pet("cat");
    let mut p = controlled("cat", 1920.0 - c.half_w - 60.0, &w);
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Screen { dir: 1.0 }));
    assert!((p.body.x - (1920.0 - c.half_w)).abs() < 0.5, "snapped to the wall: {}", p.body.x);
    // 벽 쪽 방향키는 아무것도 안 함, ↑는 오름
    let x = p.body.x;
    hold(&mut p, &w, RIGHT, 0.5);
    assert_eq!(p.body.x, x);
    let y = p.body.y;
    hold(&mut p, &w, UP, 1.0);
    assert!(p.body.y < y - 30.0);
    // 벽 반대쪽 = 유리로 옮겨 감 (떨어지지 않음)
    hold(&mut p, &w, LEFT, 1.0);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    assert!(p.body.x < x - 30.0);
    assert!(p.standing.is_none());
}

#[test]
fn near_a_window_side_it_clings_and_climbs_over_onto_the_top() {
    let w = world(vec![win(1, 1000.0, 500.0, 1600.0, 1040.0)]);
    let c = pet("cat");
    let mut p = controlled("cat", 1000.0 - c.half_w - 80.0, &w);
    cling(&mut p, &w);
    assert!(matches!(p.cling_on(), Some(ClingWall::Window { id: 1, dir, .. }) if dir == 1.0), "{:?}", p.cling_on());
    assert!((p.body.x - (1000.0 - c.half_w)).abs() < 0.5);
    // 창이 옮겨 가면 따라감
    let moved = world(vec![win(1, 1100.0, 450.0, 1700.0, 1040.0)]);
    let y = p.body.y;
    hold(&mut p, &moved, NONE, DT);
    assert!((p.body.x - (1100.0 - c.half_w)).abs() < 0.5 && (p.body.y - (y - 50.0)).abs() < 0.5, "{:?}", p.body);
    hold(&mut p, &moved, UP, 15.0);
    // ↑를 계속 누르고 있어도 올라선 뒤 폴짝 뛰지 않음 (↑를 뗐다 다시 눌러야 점프)
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "climbed over: {:?}", p.state);
    assert_eq!(p.body.y, 450.0);
    hold(&mut p, &moved, NONE, DT);
    hold(&mut p, &moved, UP, DT);
    assert!(matches!(p.state, State::Air { .. }), "a fresh ↑ jumps again");
    assert_eq!(p.cling_on(), None);
}

#[test]
fn a_window_wall_that_closes_drops_the_pet() {
    let w = world(vec![win(1, 1000.0, 500.0, 1600.0, 1040.0)]);
    let c = pet("cat");
    let mut p = controlled("cat", 1000.0 - c.half_w - 80.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    let gone = world(vec![]);
    hold(&mut p, &gone, NONE, DT);
    assert_eq!(p.cling_on(), None);
    assert!(settle(&mut p, &gone, 5.0));
}

#[test]
fn crawling_up_the_glass_past_a_window_top_stands_on_it() {
    let w = world(vec![win(1, 600.0, 700.0, 1300.0, 1040.0)]);
    let mut p = controlled("cat", 400.0, &w);
    // 창 오른쪽 위: 왼쪽 모서리는 멀다 (닿지 않음 → 유리)
    place(&mut p, &w, Owner::Ground(0), 400.0, State::Idle { remaining: 1.0, look: false });
    p.facing = -1.0;
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    // 오른쪽으로 창 앞까지 가서 위로
    hold(&mut p, &w, keys(|k| { k.right = true; k.run = true; }), 4.0);
    assert!(p.body.x > 650.0 && p.body.x < 1250.0, "{}", p.body.x);
    hold(&mut p, &w, UP, 8.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "{:?}", p.state);
}

#[test]
fn a_pet_without_climb_frames_clings_with_a_fallback_picture() {
    let w = world(vec![]);
    let mut p = controlled("penguin", 900.0, &w);
    assert!(!p.cfg.can_climb);
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    assert_ne!(p.render().anim, Anim::Climb);
    hold(&mut p, &w, UP, 1.0);
    assert!(p.standing.is_none());
}

#[test]
fn glass_crawling_stays_in_the_work_areas_of_unequal_monitors() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 300.0, 3840.0, 1080.0)], 128.0);
    let mut p = controlled("cat", 1500.0, &w);
    p.facing = -1.0;
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    for (k, secs) in [(UP, 15.0), (keys(|k| { k.right = true; k.run = true; }), 6.0)] {
        for _ in 0..(secs / DT) as usize {
            hold(&mut p, &w, k, DT);
            let col = crate::visit::column_bounds(&w, p.body.x).expect("column");
            assert!(p.body.y <= col.bottom + 0.5 && p.body.y - p.cfg.size >= col.top - 0.5, "{:?}", p.body);
        }
    }
    assert!(p.body.x < 1920.0);
}

#[test]
fn up_up_on_the_wall_hands_over_to_the_special_and_down_down_just_crawls_down() {
    // 🛡️ 붙어 있다가 ↑↑ → 날기
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    cling(&mut p, &w);
    assert!(p.cling_on().is_some());
    twice(&mut p, &w, UP);
    assert_eq!(p.cling_on(), None);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
    // 🕷️ 붙어 있다가 ↑↑ → 그네
    let mut p = controlled("spider", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 2.0);
    hold(&mut p, &w, NONE, DT);
    twice(&mut p, &w, UP);
    assert_eq!(p.cling_on(), None);
    assert!(matches!(p.state, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. }), "{:?}", p.state);
    // 🐱 특기 없는 펫: ↑↑는 그냥 오름, ↓↓는 그냥 내려감 (뛰어내리지 않음)
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 2.0);
    let y = p.body.y;
    twice(&mut p, &w, UP);
    assert!(p.body.y < y && p.cling_on().is_some());
    let y = p.body.y;
    twice(&mut p, &w, DOWN);
    assert!(p.body.y > y && p.cling_on().is_some());
}

#[test]
fn flying_specials_ignore_space_space_but_the_bike_hops_off_onto_the_wall() {
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    twice(&mut p, &w, UP);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), None);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
    // 🦇 오토바이 → 벽에 붙으면 내림
    let mut p = controlled("bat", 900.0, &w);
    twice(&mut p, &w, UP);
    assert!(settle(&mut p, &w, 5.0));
    assert_eq!(p.feat_on(), Some(Feat::Bike));
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    assert_eq!(p.feat_on(), None);
    assert_eq!(p.render().anim, Anim::Climb);
}

#[test]
fn down_down_on_a_window_top_still_drops_through_when_not_clinging() {
    let w = world(vec![win(1, 600.0, 700.0, 1300.0, 1040.0)]);
    let mut p = controlled("cat", 900.0, &w);
    place(&mut p, &w, Owner::Window(1), 900.0, State::Idle { remaining: 1.0, look: false });
    twice(&mut p, &w, DOWN);
    assert!(matches!(p.state, State::Air { .. }));
    assert_eq!(p.cling_on(), None);
}

#[test]
fn ending_control_or_being_grabbed_lets_go() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    p.set_control(false);
    assert!(matches!(p.state, State::Air { anim: Anim::Fall }), "{:?}", p.state);
    let mut landed = false;
    for _ in 0..(5.0 / DT) as usize {
        p.update(DT, &w);
        if p.standing.is_some() {
            landed = true;
            break;
        }
    }
    assert!(landed);
    // 잡힘
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    p.state = State::Dragged;
    hold(&mut p, &w, NONE, DT);
    assert_eq!(p.cling_on(), None);
}

#[test]
fn my_pet_on_a_friends_screen_clings_from_forwarded_keys() {
    let w = world(vec![]);
    let mut p = resting(pet("cat"), 900.0, 7, &w);
    p.hush_stage();
    for k in [SPACE, NONE, SPACE, NONE] {
        p.remote_keys(true, k);
        p.allow_control_gestures(true);
        p.update(DT, &w);
    }
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    let y = p.body.y;
    for _ in 0..30 {
        p.remote_keys(true, UP);
        p.update(DT, &w);
    }
    assert!(p.body.y < y - 10.0);
}

#[test]
fn a_window_side_hidden_behind_another_window_is_not_a_wall() {
    // 창 1의 왼쪽 옆면(1000)은 앞의 창 2(800..1200)에 가려 있음 → 유리에 붙음
    let w = world(vec![win(2, 800.0, 400.0, 1200.0, 1040.0), win(1, 1000.0, 500.0, 1600.0, 1040.0)]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
    assert!((p.body.x - 900.0).abs() < 0.5, "no jump to a hidden side: {}", p.body.x);
}

#[test]
fn a_window_side_that_gets_covered_drops_the_pet() {
    let w = world(vec![win(1, 1000.0, 500.0, 1600.0, 1040.0)]);
    let c = pet("cat");
    let mut p = controlled("cat", 1000.0 - c.half_w - 80.0, &w);
    cling(&mut p, &w);
    assert!(matches!(p.cling_on(), Some(ClingWall::Window { .. })));
    hold(&mut p, &w, UP, 1.0);
    let covered = world(vec![win(3, 850.0, 300.0, 1150.0, 1040.0), win(1, 1000.0, 500.0, 1600.0, 1040.0)]);
    hold(&mut p, &covered, NONE, DT);
    assert_eq!(p.cling_on(), None);
    assert!(settle(&mut p, &covered, 5.0));
}

#[test]
fn a_seam_between_side_by_side_monitors_is_not_a_screen_wall() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 128.0);
    let c = pet("cat");
    let mut p = controlled("cat", 1920.0 - c.half_w - 60.0, &w);
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass), "the seam is not a wall");
    // 바깥 끝은 벽
    let mut p = controlled("cat", 3840.0 - c.half_w - 60.0, &w);
    place(&mut p, &w, Owner::Ground(1), 3840.0 - c.half_w - 60.0, State::Idle { remaining: 1.0, look: false });
    cling(&mut p, &w);
    assert_eq!(p.cling_on(), Some(ClingWall::Screen { dir: 1.0 }));
}

#[test]
fn a_screen_wall_follows_its_edge_and_turns_to_glass_when_it_is_gone() {
    let w = world(vec![]);
    let c = pet("cat");
    let mut p = controlled("cat", 1920.0 - c.half_w - 60.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    // 작업 영역이 조금 줄어듦 (작업표시줄을 오른쪽에 둠) → 새 끝에 다시 붙음
    let narrower = World::new(vec![], vec![Rect::new(0.0, 0.0, 1860.0, 1040.0)], 128.0);
    hold(&mut p, &narrower, NONE, DT);
    assert_eq!(p.cling_on(), Some(ClingWall::Screen { dir: 1.0 }));
    assert!((p.body.x - (1860.0 - c.half_w)).abs() < 0.5, "{}", p.body.x);
    // 이웃 모니터가 붙어 끝이 이음매가 됨 → 유리
    let joined = World::new(vec![], vec![Rect::new(0.0, 0.0, 1860.0, 1040.0), Rect::new(1860.0, 0.0, 3780.0, 1040.0)], 128.0);
    hold(&mut p, &joined, NONE, DT);
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
}

#[test]
fn tapping_down_onto_a_window_top_does_not_then_drop_through_it() {
    let w = world(vec![win(1, 600.0, 700.0, 1300.0, 1040.0)]);
    let mut p = controlled("cat", 400.0, &w);
    p.facing = -1.0;
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 8.0);
    hold(&mut p, &w, keys(|k| { k.right = true; k.run = true; }), 4.0);
    assert!(p.body.x > 650.0 && p.body.y < 690.0, "{:?}", p.body);
    // 창 윗면 바로 위까지 내려옴
    while p.body.y < 688.0 {
        hold(&mut p, &w, DOWN, DT);
    }
    hold(&mut p, &w, NONE, DT);
    hold(&mut p, &w, DOWN, 0.25);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    hold(&mut p, &w, NONE, DT);
    hold(&mut p, &w, DOWN, DT);
    hold(&mut p, &w, NONE, DT);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "a tap right after stepping down is not ↓↓: {:?}", p.state);
}

#[test]
fn food_given_while_clinging_lets_go_and_eats() {
    let w = world(vec![]);
    let mut p = controlled("cat", 900.0, &w);
    cling(&mut p, &w);
    hold(&mut p, &w, UP, 1.0);
    assert!(p.feed_result(crate::care::FeedResult::Ate { leveled_up: false }, crate::care::Food::Snack));
    let mut ate = false;
    for _ in 0..(5.0 / DT) as usize {
        hold(&mut p, &w, NONE, DT);
        if matches!(p.state, State::Eating { .. }) {
            ate = true;
            break;
        }
    }
    assert!(ate, "{:?}", p.state);
    assert_eq!(p.cling_on(), None);
}

#[test]
fn an_old_friend_steering_its_pet_here_gets_no_new_gestures() {
    let w = world(vec![]);
    // 예전 판 주인 (프로토콜 8 미만): Space Space는 그냥 점프, ↑↑도
    let mut p = resting(pet("musoe"), 900.0, 7, &w);
    p.hush_stage();
    for k in [SPACE, NONE, SPACE, NONE, UP, NONE, UP, NONE] {
        p.remote_keys(true, k);
        p.allow_control_gestures(false);
        p.update(DT, &w);
    }
    assert_eq!(p.cling_on(), None);
    assert_eq!(p.feat_on(), None);
    assert!(settle(&mut p, &w, 5.0));
    // 새 판 주인
    for k in [SPACE, NONE, SPACE, NONE] {
        p.remote_keys(true, k);
        p.allow_control_gestures(true);
        p.update(DT, &w);
    }
    assert_eq!(p.cling_on(), Some(ClingWall::Glass));
}
