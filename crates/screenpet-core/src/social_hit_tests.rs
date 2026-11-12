//! 날다가 부딪히기 (social.rs thrown_hits·object_hits + brain_hit.rs) — social_tests의 자식 모듈 (도우미를 같이 쓴다)

use super::*;
use crate::behavior::Move;
use crate::brain::control_moves::ControlInput;
use crate::collide::{HIT_CALM_SECS, HIT_MIN_SPEED};
use crate::object::{Object, ObjectKind};
use crate::visit::ExitEdges;
use crate::test_util::idle;

const FLOOR_TOP: f64 = GROUND - 0.8 * 128.0;

/// 땅 위 x_b의 동물과, 왼쪽에서 (vx, vy)로 날아오는 던진 동물 (발 높이 y)
fn side_shot(w: &World, x_b: f64, vx: f64, vy: f64, y: f64) -> Vec<Pet> {
    let mut a = Pet::new(friendly(), x_b - 100.0, y, 1);
    a.force_throw(vx, vy);
    vec![a, ground(w, friendly(), x_b, 2)]
}

fn tick_until_hit(s: &mut Social, pets: &mut [Pet], w: &World) -> Option<Speech> {
    for _ in 0..30 {
        s.tick(DT, pets);
        if pets[1].wobbling() {
            return pets[0].take_speech();
        }
        pets[0].update(DT, w);
        pets[1].update(DT, w);
    }
    None
}

#[test]
fn a_fast_side_hit_bounces_back_slower_and_the_other_tumbles_away() {
    let w = world(vec![]);
    let mut pets = side_shot(&w, 800.0, 1200.0, 0.0, GROUND - 20.0);
    let mut s = Social::new(1);
    let said = tick_until_hit(&mut s, &mut pets, &w);
    assert_eq!(said, Some(Speech::Ting));
    assert!(pets[0].body.vx < 0.0, "rebounds away: {}", pets[0].body.vx);
    assert!(pets[0].body.vx.hypot(pets[0].body.vy) < 1200.0);
    assert!(matches!(pets[1].state, State::Tumble { dir, .. } if dir == 1.0), "{:?}", pets[1].state);
    assert_eq!(pets[1].take_speech(), Some(Speech::Whoa));
    // 일어선다, 둘 다 땅에 있다
    for _ in 0..240 {
        pets[0].update(DT, &w);
        pets[1].update(DT, &w);
    }
    assert!(pets[0].standing.is_some() && pets[1].standing.is_some());
    assert!(!matches!(pets[1].state, State::Tumble { .. }));
}

#[test]
fn a_harder_throw_bounces_harder() {
    let w = world(vec![]);
    let back = |v: f64| {
        let mut pets = side_shot(&w, 800.0, v, 0.0, GROUND - 20.0);
        let mut s = Social::new(1);
        tick_until_hit(&mut s, &mut pets, &w).unwrap();
        -pets[0].body.vx
    };
    assert!(back(1500.0) > back(600.0) * 1.5);
}

#[test]
fn a_slow_hit_flinches_with_ow_and_keeps_doing_what_it_did() {
    let w = world(vec![]);
    let mut pets = side_shot(&w, 800.0, 600.0, 0.0, GROUND - 20.0);
    let before = pets[1].state;
    let mut s = Social::new(1);
    assert_eq!(tick_until_hit(&mut s, &mut pets, &w), Some(Speech::Ting));
    assert_eq!(pets[1].take_speech(), Some(Speech::Ow));
    assert!(pets[1].wobbling());
    assert!(matches!(pets[1].state, State::Idle { .. }), "{before:?} -> {:?}", pets[1].state);
}

#[test]
fn falling_onto_a_head_bounces_up_and_squashes_the_other() {
    let w = world(vec![]);
    let mut a = Pet::new(friendly(), 800.0, FLOOR_TOP - 200.0, 1);
    a.force_throw(0.0, 900.0);
    let mut pets = vec![a, ground(&w, friendly(), 810.0, 2)];
    let mut s = Social::new(1);
    let (mut boing, mut oof, mut bounced) = (false, false, false);
    for _ in 0..240 {
        step(&mut s, &mut pets, &w);
        bounced |= pets[0].body.vy < -100.0;
        boing |= pets[0].take_speech() == Some(Speech::Boing);
        oof |= pets[1].take_speech() == Some(Speech::Oof);
    }
    assert!(boing && oof && bounced);
    assert!(pets[0].standing.is_some() && pets[1].standing.is_some());
    assert_eq!(pets[0].body.y, GROUND);
}

#[test]
fn being_let_go_gently_does_nothing() {
    let w = world(vec![]);
    // 머리 옆 바로 위에서 놓는다 (머리 폭 안이면 올라선다, social_tower_tests)
    let mut a = Pet::new(friendly(), 860.0, FLOOR_TOP - 5.0, 1);
    a.force_throw(0.0, 0.0);
    let mut pets = vec![a, ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    for _ in 0..120 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[1].wobbling());
        // 내려선 뒤 인사는 괜찮다
        assert!(!matches!(pets[1].take_speech(), Some(Speech::Ow | Speech::Oof | Speech::Whoa)));
    }
    // 겹친 채 놓으면 떨어지며 빨라져도 그대로
    let mut a = Pet::new(friendly(), 800.0, GROUND - 60.0, 1);
    a.force_throw(0.0, 0.0);
    let mut pets = vec![a, ground(&w, friendly(), 790.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    for _ in 0..60 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[1].wobbling());
    }
}

#[test]
fn the_threshold_is_in_logical_pixels() {
    let w = world(vec![]);
    let hits = |dpi: f64, logical: f64| {
        let c = PetConfig { dpi, ..friendly() };
        let mut a = Pet::new(c, 650.0, GROUND - 30.0, 1);
        a.force_throw(logical * dpi, 0.0);
        let mut pets = vec![a, ground(&w, friendly(), 800.0, 2)];
        let mut s = Social::new(1);
        s.tick(DT, &mut pets);
        for _ in 0..30 {
            s.tick(DT, &mut pets);
            if pets[1].wobbling() {
                return true;
            }
            // 중력 없이 곧게 (높이가 그대로여야 빠르기만 본다)
            pets[0].body.x += pets[0].body.vx * DT;
        }
        false
    };
    for dpi in [1.0, 1.5, 2.0] {
        assert!(hits(dpi, HIT_MIN_SPEED + 20.0), "{dpi}");
        assert!(!hits(dpi, HIT_MIN_SPEED - 20.0), "{dpi}");
    }
}

#[test]
fn hit_distance_uses_the_real_body_sizes() {
    let w = world(vec![]);
    let big = PetConfig { size: 256.0, half_w: 76.8, ..friendly() };
    let shot = |target: PetConfig| {
        // 왼쪽에서 곧게 — 두 반 폭의 합에 닿을 때 맞는다
        let mut a = Pet::new(friendly(), 500.0, GROUND - 30.0, 1);
        a.force_throw(1000.0, 0.0);
        let mut pets = vec![a, ground(&w, target, 800.0, 2)];
        let mut s = Social::new(1);
        s.tick(DT, &mut pets);
        for _ in 0..60 {
            pets[0].body.x += pets[0].body.vx * DT;
            s.tick(DT, &mut pets);
            if pets[1].wobbling() {
                return pets[0].body.x;
            }
        }
        panic!("no hit");
    };
    let (at_small, at_big) = (shot(friendly()), shot(big));
    assert!((800.0 - at_small - 76.8).abs() < 20.0, "{at_small}");
    assert!((800.0 - at_big - 115.2).abs() < 20.0, "{at_big}");
    // 큰 동물은 머리도 높다
    let mut a = Pet::new(friendly(), 800.0, GROUND - 150.0, 1);
    a.force_throw(0.0, 900.0);
    let mut pets = vec![a, ground(&w, big, 800.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    assert!(pets[0].body.y > GROUND - 0.8 * 256.0);
    assert!(!pets[1].wobbling(), "already inside the big body");
}

#[test]
fn a_flier_is_pushed_through_the_air_and_recovers() {
    let w = world(vec![]);
    let mut a = Pet::new(friendly(), 700.0, 600.0, 1);
    a.force_throw(1200.0, 0.0);
    let mut pets = vec![a, bee_at(800.0, 600.0, 2)];
    let mut s = Social::new(1);
    let mut pushed = false;
    for _ in 0..30 {
        s.tick(DT, &mut pets);
        pets[0].update(DT, &w);
        pets[1].update(DT, &w);
        if pets[1].wobbling() {
            pushed = true;
            break;
        }
    }
    assert!(pushed);
    assert!(matches!(pets[1].state, State::Air { .. }), "{:?}", pets[1].state);
    assert!(!pets[1].is_flung(), "a pushed flier is not a thrown one — no chain");
    let x0 = pets[1].body.x;
    for _ in 0..240 {
        pets[1].update(DT, &w);
    }
    assert!(pets[1].body.x > x0 + 40.0, "pushed away: {}", pets[1].body.x);
    assert!(!matches!(pets[1].state, State::Air { .. } | State::Tumble { .. }), "{:?}", pets[1].state);
    assert!(pets[1].body.y < GROUND, "stays in the air");
}

#[test]
fn a_pair_does_not_hit_again_right_away_and_a_hit_pet_is_calm_for_a_moment() {
    let w = world(vec![]);
    let mut pets = side_shot(&w, 800.0, 1200.0, 0.0, GROUND - 20.0);
    let mut s = Social::new(1);
    tick_until_hit(&mut s, &mut pets, &w).unwrap();
    pets[1].take_speech();
    // 다른 던진 동물: 방금 맞았으니 지나간다
    let mut c = Pet::new(friendly(), 700.0, GROUND - 20.0, 3);
    c.force_throw(1200.0, 0.0);
    c.body.x = pets[1].body.x - 100.0;
    let mut trio = vec![pets.remove(0), pets.remove(0), c];
    trio[0].state = idle();
    s.tick(DT, &mut trio);
    for _ in 0..8 {
        trio[2].update(DT, &w);
        trio[1].update(DT, &w);
        s.tick(DT, &mut trio);
        assert!(trio[1].take_speech().is_none());
        assert!(trio[2].body.vx > 0.0, "passes through");
    }
    assert!(HIT_CALM_SECS > 0.5);
    // 시간이 지나면 다시 맞는다: 같은 짝이 다시 던져져서
    let w2 = world(vec![]);
    let mut pets = side_shot(&w2, 800.0, 1200.0, 0.0, GROUND - 20.0);
    let mut s = Social::new(1);
    tick_until_hit(&mut s, &mut pets, &w2).unwrap();
    pets[1].take_speech();
    run_pin(&mut s, &mut pets, &w2, 0.5, 800.0);
    pets[0].body = Body { x: 700.0, y: GROUND - 20.0, vx: 0.0, vy: 0.0 };
    pets[0].force_throw(1200.0, 0.0);
    assert!(tick_until_hit_quiet(&mut s, &mut pets, &w2).is_none(), "pair cooldown");
    run_pin(&mut s, &mut pets, &w2, 2.0, 800.0);
    pets[0].body = Body { x: 700.0, y: GROUND - 20.0, vx: 0.0, vy: 0.0 };
    pets[0].force_throw(1200.0, 0.0);
    s.tick(DT, &mut pets);
    assert!(tick_until_hit_quiet(&mut s, &mut pets, &w2).is_some(), "after the cooldown");
}

fn run_pin(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64, x: f64) {
    for _ in 0..(secs / DT) as usize {
        pets[1].state = idle();
        pets[1].body.x = x;
        pets[1].standing = ground(w, friendly(), x, 9).standing;
        step(s, pets, w);
        s.tick(0.0, pets);
    }
    pets[1].state = idle();
    pets[0].take_speech();
    pets[1].take_speech();
}

fn tick_until_hit_quiet(s: &mut Social, pets: &mut [Pet], w: &World) -> Option<Speech> {
    for _ in 0..12 {
        pets[1].state = idle();
        s.tick(DT, pets);
        if let Some(said) = pets[1].take_speech() {
            return Some(said);
        }
        pets[0].update(DT, w);
        pets[1].update(DT, w);
    }
    None
}

#[test]
fn busy_pets_are_never_hit() {
    let w = world(vec![w1()]);
    let busy: Vec<State> = vec![
        State::Dragged,
        State::Pressed { start_cx: 0.0, start_cy: 0.0, grounded: true, was_sleeping: false },
        State::Away,
        State::Enter { remaining: 2.0, side: Side::Left },
        State::Exit { remaining: 2.0, side: Side::Right },
        State::Farewell { remaining: 2.0 },
        State::Eating { remaining: 3.0, favorite: false, level_up: false },
        State::Housed,
        State::Air { anim: Anim::Fall },
    ];
    for state in busy {
        let mut pets = side_shot(&w, 800.0, 1200.0, 0.0, GROUND - 60.0);
        pets[1].state = state;
        let mut s = Social::new(1);
        s.tick(DT, &mut pets);
        for _ in 0..12 {
            pets[0].update(DT, &w);
            s.tick(DT, &mut pets);
            assert_eq!(pets[1].state, state);
            assert!(!pets[1].wobbling() && pets[0].body.vx > 0.0, "{state:?}");
        }
    }
    let mut pets = side_shot(&w, 800.0, 1200.0, 0.0, GROUND - 20.0);
    pets[1].enter_house();
    assert!(!pets[1].hit_feels());
}

#[test]
fn control_mode_is_only_tumbled_and_fliers_keep_flying() {
    let w = world(vec![]);
    // 걷는 동물: 빠르면 데굴, 끝나면 키가 다시 듣는다
    let mut pets = side_shot(&w, 800.0, 1200.0, 0.0, GROUND - 20.0);
    pets[1].set_control(true);
    let mut s = Social::new(1);
    tick_until_hit(&mut s, &mut pets, &w).unwrap();
    assert!(matches!(pets[1].state, State::Tumble { .. }), "{:?}", pets[1].state);
    for _ in 0..120 {
        pets[1].update(DT, &w);
    }
    pets[1].control_keys(ControlInput { right: true, ..Default::default() }, DT);
    let x = pets[1].body.x;
    for _ in 0..30 {
        pets[1].update(DT, &w);
    }
    assert!(pets[1].body.x > x, "keys work again");
    assert!(matches!(pets[1].state, State::Move { kind: Move::Walk, .. }), "{:?}", pets[1].state);
    // 머리를 맞아도 납작하지 않고 키를 뺏지 않는다 (느리면 움찔만)
    let mut a = Pet::new(friendly(), 800.0, FLOOR_TOP - 8.0, 1);
    a.force_throw(0.0, 600.0);
    let mut pets = vec![a, ground(&w, friendly(), 800.0, 2)];
    pets[1].set_control(true);
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    pets[0].update(DT, &w);
    s.tick(DT, &mut pets);
    assert_eq!(pets[0].take_speech(), Some(Speech::Boing));
    assert_eq!(pets[1].state, idle());
    assert_eq!(pets[1].take_speech(), Some(Speech::Ow));
    // 나는 동물: 덜덜만
    let mut a = Pet::new(friendly(), 700.0, 600.0, 1);
    a.force_throw(1200.0, 0.0);
    let mut pets = vec![a, bee_at(780.0, 600.0, 2)];
    pets[1].set_control(true);
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    pets[0].body.x += 20.0;
    s.tick(DT, &mut pets);
    assert!(pets[1].wobbling());
    assert_eq!(pets[1].state, idle());
}

#[test]
fn crossing_still_works_when_nobody_is_in_the_way_and_a_hit_forgets_the_aim_below() {
    let w = world(vec![]);
    let edges = ExitEdges { left: false, right: true, up: false, down: true };
    // 비껴가면 그대로 넘어간다
    let mut a = Pet::new(friendly(), 1800.0, 600.0, 1);
    a.force_throw(1200.0, 0.0);
    a.set_exit_edges(false, true);
    a.exit_edges_test(edges);
    let mut pets = vec![a, ground(&w, friendly(), 300.0, 2)];
    let mut s = Social::new(1);
    let mut exited = false;
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
        exited |= pets[0].take_exit() == Some(Side::Right);
    }
    assert!(exited);
    assert!(pets[0].take_fling().is_some());
    // 아래 친구를 노리고 던져도 머리에 맞으면 그 뜻을 잊는다
    let mut a = Pet::new(friendly(), 800.0, FLOOR_TOP - 40.0, 1);
    a.force_throw_down(0.0, 1200.0);
    a.exit_edges_test(edges);
    let mut pets = vec![a, ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    assert!(pets[0].aims_down());
    pets[0].update(DT, &w);
    s.tick(DT, &mut pets);
    pets[0].update(DT, &w);
    s.tick(DT, &mut pets);
    assert!(!pets[0].aims_down());
    assert!(pets[0].body.vy < 0.0);
}

// ---- 물건 ----

fn thrown_object(kind: ObjectKind, x: f64, y: f64, vx: f64, vy: f64) -> Object {
    let mut o = Object::new(kind, x, y, 1.0, 1);
    o.force_throw(vx, vy);
    o
}

/// 물건을 곧게 움직이며 (물리는 object_tests) 부딪힐 때까지
fn fly_object(s: &mut Social, o: &mut Object, pets: &mut [Pet], ticks: usize) -> bool {
    s.tick(DT, pets);
    for _ in 0..ticks {
        let v = (o.body.vx, o.body.vy);
        o.body.x += v.0 * DT;
        o.body.y += v.1 * DT;
        s.tick(DT, pets);
        s.object_hits(7, o, DT, pets);
        if (o.body.vx, o.body.vy) != v {
            return true;
        }
    }
    false
}

#[test]
fn a_thrown_object_bounces_off_and_the_pet_reacts() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    let mut o = thrown_object(ObjectKind::Can, 600.0, GROUND - 20.0, 1200.0, 0.0);
    assert!(fly_object(&mut s, &mut o, &mut pets, 30));
    assert!(o.body.vx < 0.0 && o.body.vx.hypot(o.body.vy) < 1200.0);
    assert!(matches!(pets[0].state, State::Tumble { dir, .. } if dir == 1.0));
    assert_eq!(pets[0].take_speech(), Some(Speech::Whoa));
    assert!(o.in_flight(), "flight goes on");

    let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    let mut o = thrown_object(ObjectKind::Stone, 600.0, GROUND - 20.0, 600.0, 0.0);
    assert!(fly_object(&mut s, &mut o, &mut pets, 40));
    assert_eq!(pets[0].take_speech(), Some(Speech::Ow));
    assert_eq!(pets[0].state, idle());
}

#[test]
fn a_real_throw_counts_too() {
    let w = world(vec![]);
    let mut o = Object::new(ObjectKind::Can, 500.0, GROUND - 200.0, 1.0, 1);
    o.pointer_down(500.0, GROUND - 200.0);
    assert!(!o.in_flight(), "held");
    let edges = ExitEdges::default();
    for i in 1..=8 {
        o.tick(DT, &w, edges, false);
        o.pointer_move(500.0 + 3000.0 * i as f64 * DT, GROUND - 200.0);
    }
    assert!(!o.in_flight(), "held");
    o.pointer_up();
    assert!(o.in_flight());
}

/// 게임 루프 순서 그대로: 손으로 던진 물건이 Object::tick으로 날아가 Social::object_hits로 동물을 맞힌다
#[test]
fn a_hand_thrown_object_flies_by_its_own_physics_and_hits_the_pet() {
    let w = world(vec![]);
    let edges = ExitEdges::default();
    let throw = || {
        let mut o = Object::new(ObjectKind::Can, 500.0, GROUND - 40.0, 1.0, 1);
        o.pointer_down(500.0, GROUND - 40.0);
        for i in 1..=8 {
            o.tick(DT, &w, edges, false);
            o.pointer_move(500.0 + 1500.0 * i as f64 * DT, GROUND - 60.0);
        }
        o.pointer_up();
        assert!(o.in_flight());
        o
    };
    let mut pets = vec![ground(&w, friendly(), 900.0, 2)];
    let mut s = Social::new(1);
    let mut o = throw();
    let mut hit = false;
    for _ in 0..120 {
        s.tick(DT, &mut pets);
        let before = o.body.vx;
        o.tick(DT, &w, edges, false);
        s.object_hits(7, &mut o, DT, &mut pets);
        if pets[0].wobbling() {
            assert!(before > 0.0 && o.body.vx < before, "bounced back: {before} -> {}", o.body.vx);
            assert!(o.body.x < 900.0, "{}", o.body.x);
            hit = true;
            break;
        }
    }
    assert!(hit, "{:?}", o.body);
    assert!(matches!(pets[0].take_speech(), Some(Speech::Whoa | Speech::Ow)));
    // 비교: 동물이 없는 쪽으로 던지면 그냥 날아간다
    let mut far = vec![ground(&w, friendly(), 100.0, 2)];
    let mut s = Social::new(1);
    let mut o = throw();
    for _ in 0..120 {
        s.tick(DT, &mut far);
        o.tick(DT, &w, edges, false);
        s.object_hits(7, &mut o, DT, &mut far);
        assert!(!far[0].wobbling());
    }
}

#[test]
fn poop_also_disgusts() {
    let w = world(vec![]);
    for (v, fast) in [(1200.0, true), (600.0, false)] {
        let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
        let mut s = Social::new(1);
        let mut o = thrown_object(ObjectKind::Poop, 600.0, GROUND - 20.0, v, 0.0);
        assert!(fly_object(&mut s, &mut o, &mut pets, 40));
        assert_eq!(pets[0].take_speech(), Some(Speech::Ew));
        assert_eq!(matches!(pets[0].state, State::Tumble { .. }), fast);
    }
}

#[test]
fn slow_resting_rolling_and_held_objects_do_nothing() {
    let w = world(vec![]);
    let quiet = |mut o: Object| {
        let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
        let mut s = Social::new(1);
        assert!(!fly_object(&mut s, &mut o, &mut pets, 40));
        assert!(!pets[0].wobbling() && pets[0].state == idle());
    };
    quiet(thrown_object(ObjectKind::Can, 600.0, GROUND - 20.0, HIT_MIN_SPEED - 30.0, 0.0));
    // 떨어지는 중 (던진 게 아니다)
    let mut falling = Object::new(ObjectKind::Can, 700.0, GROUND - 20.0, 1.0, 1);
    falling.body.vx = 1200.0;
    assert!(!falling.in_flight());
    quiet(falling);
    // 들렸다
    let mut held = Object::new(ObjectKind::Can, 700.0, GROUND - 20.0, 1.0, 1);
    held.pointer_down(700.0, GROUND - 20.0);
    held.body.vx = 1200.0;
    assert!(!held.in_flight());
    quiet(held);
    // 바람에 구른다
    let f = world(vec![]);
    let mut o = Object::new(ObjectKind::Can, 700.0, GROUND, 1.0, 1);
    for _ in 0..240 {
        o.tick(DT, &f, ExitEdges::default(), false);
    }
    assert!(o.is_asleep());
    o.start_gust(1.0);
    assert!(o.is_rolling() && !o.in_flight());
    assert!(o.body.vx > 0.0);
    quiet(o);
}

#[test]
fn hit_distance_uses_the_drawn_size_of_a_poop() {
    let w = world(vec![]);
    let hit_at = |scale: f64| {
        let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
        let mut s = Social::new(1);
        let mut o = thrown_object(ObjectKind::Poop, 500.0, GROUND - 20.0, 900.0, 0.0);
        o.set_scale(scale);
        assert!(fly_object(&mut s, &mut o, &mut pets, 60));
        // 되튀기 전 자리 (이번 틱에 닿았다)
        o.body.x
    };
    let (full, small) = (hit_at(1.0), hit_at(0.4));
    assert!(small > full + 8.0, "a small poop must come closer: {small} vs {full}");
    assert!((800.0 - small - (38.4 + 28.8 * 0.4)).abs() <= 16.0, "{small}");
}

#[test]
fn an_object_does_not_hit_the_same_pet_again_at_once() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    let mut o = thrown_object(ObjectKind::Can, 600.0, GROUND - 20.0, 600.0, 0.0);
    assert!(fly_object(&mut s, &mut o, &mut pets, 40));
    pets[0].take_speech();
    o.body.x = 600.0;
    o.force_throw(1000.0, 0.0);
    assert!(!fly_object(&mut s, &mut o, &mut pets, 20), "pair cooldown and calm");
}

#[test]
fn busy_pets_are_not_hit_by_objects() {
    let w = world(vec![]);
    let hit = |busy: bool| {
        let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
        if busy {
            pets[0].state = State::Dragged;
        }
        let mut s = Social::new(1);
        let mut o = thrown_object(ObjectKind::Can, 600.0, GROUND - 20.0, 1200.0, 0.0);
        let hit = fly_object(&mut s, &mut o, &mut pets, 30);
        if busy {
            assert_eq!(pets[0].state, State::Dragged);
        }
        hit
    };
    assert!(hit(false), "the same throw hits a free pet");
    assert!(!hit(true));
}

/// 🤹 흐물이가 던진 물건은 던진 동물 자신을 잠깐 맞히지 않는다 (손에서 막 떠난 물건이 제 몸과 겹친다)
#[test]
fn a_juggled_object_spares_its_thrower_for_a_moment() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 2)];
    let key = pets[0].social_key();
    let mut s = Social::new(1);
    let mut o = thrown_object(ObjectKind::Can, 600.0, GROUND - 20.0, 1200.0, 0.0);
    s.spare(7, key, 2.0);
    assert!(!fly_object(&mut s, &mut o, &mut pets, 30), "던진 동물은 맞지 않는다");
    assert!(!pets[0].wobbling());
}
