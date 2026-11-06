//! 탑 쌓기 (social_tower.rs + brain_tower.rs) — social_tests의 자식 모듈 (도우미를 같이 쓴다)

use super::tower::{MAX_TOWER, TOWER_CLIMB_CHANCE};
use super::*;
use crate::behavior::Move;
use crate::brain::control_moves::ControlInput;
use crate::brain::tower_moves::{RIDE_MAX_SECS, RIDE_SECS};
use crate::poop::PoopSpot;
use crate::test_util::{LoopLimit, idle};

const HEAD: f64 = 0.8 * 128.0;

fn walking(p: &mut Pet, facing: f64, kind: Move) {
    p.state = State::Move { kind, remaining: 300.0 };
    p.facing = facing;
}

/// 땅 위에서 마주 오는 짝: 0번(x 800)은 kind로, 1번(x 1000)은 걸어서. 한 틱 걸어 걷는 조각을 안다
fn head_on_pair(w: &World, seed_a: u64, seed_b: u64, kind: Move) -> Vec<Pet> {
    let mut a = ground(w, friendly(), 800.0, seed_a);
    let mut b = ground(w, friendly(), 1000.0, seed_b);
    walking(&mut a, 1.0, kind);
    walking(&mut b, -1.0, Move::Walk);
    let mut pets = vec![a, b];
    for p in &mut pets {
        p.update(DT, w);
    }
    pets
}

/// 땅 위 x에 n층 탑 (0번이 맨 아래)
fn tower(s: &mut Social, w: &World, n: usize, x: f64) -> Vec<Pet> {
    let mut pets: Vec<Pet> = (0..n).map(|i| ground(w, friendly(), x, 100 + i as u64)).collect();
    for i in 1..n {
        s.force_board(&mut pets, i, i - 1);
    }
    s.carry(&mut pets);
    pets
}

/// 쓰러지지 않게 잠깐 돌리고 진짜 디딤판에 섰는지 본다
fn all_land(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) {
    for _ in 0..(secs / DT) as usize {
        step(s, pets, w);
        for p in pets.iter_mut() {
            // 돌아다니며 다시 올라가지 않게: 땅에 서면 앉아 있는다
            if p.standing.is_some() && !p.is_held() && !matches!(p.state, State::Land { .. }) {
                p.state = idle();
            }
        }
    }
}

fn on_real_surface(p: &Pet, w: &World) -> bool {
    let Some(st) = p.standing else { return false };
    let Some((ax, ay)) = w.anchor(st.owner) else { return false };
    (p.body.x - (ax + st.rel_x)).abs() < 1e-6 && (p.body.y - ay).abs() < 1e-6 && !p.is_on_head()
}

#[test]
fn the_whole_tower_is_carried_without_lag() {
    let w = world(vec![]);
    let mut s = Social::new(1);
    let mut pets = tower(&mut s, &w, 5, 300.0);
    walking(&mut pets[0], 1.0, Move::Walk);
    pets[0].update(DT, &w);
    let x0 = pets[0].body.x;
    for _ in 0..200 {
        for p in &mut pets[1..] {
            // 시간이 되어 내려오지 않게
            p.state = State::OnHead { remaining: 50.0, age: 0.0 };
        }
        step(&mut s, &mut pets, &w);
        for (i, p) in pets.iter().enumerate().skip(1) {
            assert_eq!(p.body.x, pets[0].body.x, "level {i} same tick");
            assert!((p.body.y - (GROUND - i as f64 * HEAD)).abs() < 1e-6, "level {i}: {}", p.body.y);
        }
    }
    assert!(pets[0].body.x > x0 + 50.0, "carried along");
    assert!(matches!(pets[0].state, State::Move { .. }), "the bottom one keeps walking");
    assert_eq!(s.tower_height(pets[0].social_key()), 5);
}

#[test]
fn real_sizes_set_the_height_of_each_level() {
    let w = world(vec![]);
    let big = PetConfig { size: 256.0, half_w: 76.8, ..friendly() };
    let mut pets = vec![ground(&w, big, 500.0, 1), ground(&w, friendly(), 500.0, 2), ground(&w, friendly(), 500.0, 3)];
    let mut s = Social::new(1);
    s.force_board(&mut pets, 1, 0);
    s.force_board(&mut pets, 2, 1);
    step(&mut s, &mut pets, &w);
    assert!((pets[1].body.y - (GROUND - 0.8 * 256.0)).abs() < 1e-6);
    assert!((pets[2].body.y - (GROUND - 0.8 * 256.0 - HEAD)).abs() < 1e-6);
}

#[test]
fn only_the_top_hops_down_on_its_timer_and_everyone_ends_on_the_ground_in_turn() {
    let w = world(vec![]);
    let mut s = Social::new(3);
    let mut pets = tower(&mut s, &w, 3, 900.0);
    pets[1].expire_ride();
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
        assert!(pets[1].is_on_head(), "waits for the one above");
    }
    pets[2].expire_ride();
    step(&mut s, &mut pets, &w);
    assert_eq!(pets[2].state, State::Air { anim: Anim::Hop });
    s.tick(DT, &mut pets);
    assert!(!pets[1].has_rider());
    pets[1].update(DT, &w);
    assert!(!pets[1].is_on_head(), "its time was already up: {:?}", pets[1].state);
    all_land(&mut s, &mut pets, &w, 3.0);
    for p in &pets {
        assert!(on_real_surface(p, &w), "{:?}", p.state);
    }
    assert_eq!(s.tower_height(pets[0].social_key()), 1);
}

#[test]
fn a_rider_comes_down_by_itself_within_its_few_seconds() {
    let w = world(vec![]);
    let mut s = Social::new(4);
    let mut pets = tower(&mut s, &w, 2, 900.0);
    let mut t = 0.0;
    let mut limit = LoopLimit::new();
    while pets[1].is_on_head() {
        limit.step();
        pets[0].state = idle();
        step(&mut s, &mut pets, &w);
        t += DT;
        assert!(t <= RIDE_SECS.1 + 0.1, "stayed too long");
    }
    assert!(t >= RIDE_SECS.0 - 0.1, "{t}");
}

#[test]
fn no_pet_stays_up_past_the_hard_limit_even_under_another() {
    let w = world(vec![]);
    let mut s = Social::new(5);
    let mut pets = tower(&mut s, &w, 3, 900.0);
    let mut t = 0.0;
    let mut limit = LoopLimit::new();
    while pets[1].is_on_head() {
        limit.step();
        // 맨 위는 영영 안 내려간다고 치자
        if let Some((_, age)) = pets[2].ride_clock() {
            pets[2].state = State::OnHead { remaining: 50.0, age };
        }
        pets[0].state = idle();
        step(&mut s, &mut pets, &w);
        t += DT;
        assert!(t <= RIDE_MAX_SECS + 0.1, "{t}");
    }
    assert!(t >= RIDE_MAX_SECS - 0.1, "{t}");
    all_land(&mut s, &mut pets, &w, 3.0);
    assert!(pets.iter().all(|p| on_real_surface(p, &w)));
}

/// 맨 아래가 빠지는 모든 경우: 위층은 다 떨어져 진짜 디딤판에 선다
#[test]
fn every_way_the_lower_pet_leaves_drops_the_riders() {
    let windowed = world(vec![w1()]);
    let cases: [(&str, fn(&mut Pet)); 9] = [
        ("grabbed", |p| p.pointer_down(p.body.x, p.body.y - 10.0)),
        ("thrown", |p| p.force_throw(300.0, -200.0)),
        ("tumbles", |p| {
            assert!(p.feel_stomp(p.body.x - 50.0, true));
        }),
        ("exit", |p| p.state = State::Exit { remaining: 3.0, side: Side::Right }),
        ("away", |p| p.state = State::Away),
        ("house", |p| p.enter_house()),
        ("heads home", |p| p.go_home(Owner::Ground(0), 100.0)),
        ("resizes", |p| p.cfg.size = 160.0),
        ("flies now", |p| p.cfg.flies = true),
    ];
    for (name, leave) in cases {
        let mut s = Social::new(6);
        let mut pets = tower(&mut s, &windowed, 3, 300.0);
        step(&mut s, &mut pets, &windowed);
        leave(&mut pets[0]);
        // 다음 틱에 첫 층이 떨어지고, 번져서 맨 위까지 떨어진다
        s.tick(DT, &mut pets);
        assert!(!pets[1].is_on_head() && !pets[2].is_on_head(), "{name}: {:?}", pets[1].state);
        assert_eq!(s.tower_height(pets[1].social_key()), 1, "{name}");
        pets[0].state = State::Away;
        pets[0].cfg.flies = false;
        for _ in 0..(3.0 / DT) as usize {
            pets[1].update(DT, &windowed);
            pets[2].update(DT, &windowed);
            s.tick(DT, &mut pets);
            for p in &mut pets[1..] {
                if p.standing.is_some() {
                    p.state = idle();
                }
            }
        }
        assert!(on_real_surface(&pets[1], &windowed), "{name}: {:?}", pets[1].state);
        assert!(on_real_surface(&pets[2], &windowed), "{name}: {:?}", pets[2].state);
    }
}

#[test]
fn a_closed_window_drops_the_whole_tower_to_the_ground() {
    let with = world(vec![w1()]);
    let gone = world(vec![]);
    let mut s = Social::new(7);
    let mut pets: Vec<Pet> = (0..3).map(|i| pet_on(&with, friendly(), Owner::Window(1), 800.0, i)).collect();
    s.force_board(&mut pets, 1, 0);
    s.force_board(&mut pets, 2, 1);
    step(&mut s, &mut pets, &with);
    assert!((pets[2].body.y - (400.0 - 2.0 * HEAD)).abs() < 1e-6);
    all_land(&mut s, &mut pets, &gone, 3.0);
    for p in &pets {
        assert!(on_real_surface(p, &gone), "{:?}", p.state);
        assert_eq!(p.body.y, GROUND);
    }
}

#[test]
fn a_rider_that_changes_size_comes_off_and_a_removed_carrier_too() {
    let w = world(vec![]);
    let mut s = Social::new(8);
    let mut pets = tower(&mut s, &w, 2, 800.0);
    step(&mut s, &mut pets, &w);
    assert!(pets[1].is_on_head() && pets[0].has_rider());
    pets[1].cfg.size = 90.0;
    s.tick(DT, &mut pets);
    assert!(!pets[1].is_on_head() && !pets[0].has_rider());
    // 태운 쪽이 화면에서 사라지면 (떠나거나 친구가 끊겼다) 위에 있던 쪽은 내려온다
    let mut s = Social::new(8);
    let mut pets = tower(&mut s, &w, 2, 800.0);
    step(&mut s, &mut pets, &w);
    assert!(pets[1].is_on_head());
    pets.remove(0);
    s.tick(DT, &mut pets);
    assert!(!pets[0].is_on_head(), "{:?}", pets[0].state);
    assert_eq!(s.tower_height(pets[0].social_key()), 1);
}

#[test]
fn a_five_high_tower_collapses_from_the_middle() {
    let w = world(vec![]);
    let mut s = Social::new(9);
    let mut pets = tower(&mut s, &w, 5, 800.0);
    step(&mut s, &mut pets, &w);
    let (bottom, one) = (pets[0].social_key(), pets[1].social_key());
    let (x, y) = (pets[2].body.x, pets[2].body.y);
    pets[2].pointer_down(x, y - 40.0);
    s.tick(DT, &mut pets);
    assert!(pets[2].is_held());
    assert!(!pets[3].is_on_head() && !pets[4].is_on_head());
    assert!(pets[1].is_on_head(), "pets below stay");
    assert!(pets[1].has_rider() == false && pets[0].has_rider());
    assert_eq!(s.tower_height(bottom), 2);
    assert_eq!(s.top_of(bottom), one);
    // 떨어지는 동물은 남은 머리에 다시 서지 않고 땅까지 간다
    let mut upper = true;
    for _ in 0..(1.5 / DT) as usize {
        pets[0].state = idle();
        step(&mut s, &mut pets, &w);
        upper &= pets[1].is_on_head();
        for p in &mut pets[3..] {
            assert!(!p.is_on_head(), "no re-landing while falling off");
            if p.standing.is_some() {
                p.state = idle();
            }
        }
    }
    assert!(upper);
    assert!(on_real_surface(&pets[3], &w) && on_real_surface(&pets[4], &w));
    assert_eq!((pets[3].body.y, pets[4].body.y), (GROUND, GROUND));
}

#[test]
fn removing_the_bottom_of_a_five_high_tower_drops_everyone_to_a_surface() {
    let w = world(vec![]);
    let mut s = Social::new(10);
    let mut pets = tower(&mut s, &w, 5, 800.0);
    step(&mut s, &mut pets, &w);
    pets.remove(0);
    all_land(&mut s, &mut pets, &w, 3.0);
    for p in &pets {
        assert!(on_real_surface(p, &w), "{:?}", p.state);
    }
}

#[test]
fn a_rider_grabbed_behaves_as_usual_and_a_throw_ignores_its_carrier() {
    let w = world(vec![]);
    let mut s = Social::new(11);
    let mut pets = tower(&mut s, &w, 3, 800.0);
    step(&mut s, &mut pets, &w);
    let (x, y) = (pets[1].body.x, pets[1].body.y);
    pets[1].pointer_down(x, y - 40.0);
    assert!(pets[1].is_held());
    s.tick(DT, &mut pets);
    assert!(!pets[2].is_on_head(), "the one above falls");
    assert!(!pets[0].has_rider());
    // 곧장 아래 친구 쪽으로 세게 던져도 방금 내린 짝은 부딪히지 않는다
    pets[0].take_speech();
    pets[1].take_speech();
    pets[1].force_throw(0.0, 1200.0);
    s.tick(DT, &mut pets);
    pets[1].update(DT, &w);
    s.tick(DT, &mut pets);
    assert!(pets[1].body.vy > 0.0, "no bounce");
    assert_eq!(pets[0].take_speech(), None, "pair ignored just after riding");
    assert!(matches!(pets[0].state, State::Idle { .. }), "{:?}", pets[0].state);
    // 그냥 클릭: 평소처럼 떨어져 내려선다
    let mut s = Social::new(12);
    let mut pets = tower(&mut s, &w, 2, 800.0);
    step(&mut s, &mut pets, &w);
    let (x, y) = (pets[1].body.x, pets[1].body.y);
    pets[1].pointer_down(x, y - 40.0);
    pets[1].pointer_up(x, y - 40.0);
    all_land(&mut s, &mut pets, &w, 2.0);
    assert!(on_real_surface(&pets[1], &w), "a click does not put it back: {:?}", pets[1].state);
}

#[test]
fn a_control_key_makes_the_rider_hop_off() {
    let w = world(vec![]);
    let mut s = Social::new(13);
    let mut pets = tower(&mut s, &w, 3, 800.0);
    pets[1].set_control(true);
    for _ in 0..10 {
        step(&mut s, &mut pets, &w);
    }
    assert!(pets[1].is_on_head(), "no key, stays");
    pets[1].control_keys(ControlInput { right: true, ..Default::default() }, DT);
    step(&mut s, &mut pets, &w);
    assert!(matches!(pets[1].state, State::Air { .. }), "{:?}", pets[1].state);
    s.tick(DT, &mut pets);
    assert!(!pets[2].is_on_head());
}

#[test]
fn food_home_guests_and_the_user_going_away_end_the_ride() {
    let w = world(vec![]);
    let cases: [(&str, fn(&mut Pet)); 3] = [
        ("hand", |p| p.offer_food_test()),
        ("away", |p| p.set_user_away(true)),
        ("home", |p| p.go_home(Owner::Ground(0), 100.0)),
    ];
    for (name, f) in cases {
        let mut s = Social::new(14);
        let mut pets = tower(&mut s, &w, 2, 800.0);
        step(&mut s, &mut pets, &w);
        f(&mut pets[1]);
        step(&mut s, &mut pets, &w);
        assert!(!pets[1].is_on_head(), "{name}");
    }
    // 손님에게 "집에 와": 내려선 뒤 손 흔든다
    let mut s = Social::new(15);
    let mut pets = tower(&mut s, &w, 2, 800.0);
    pets[1].set_guest(Side::Left, 300.0);
    step(&mut s, &mut pets, &w);
    pets[1].go_home_now();
    let mut waved = false;
    for _ in 0..180 {
        pets[0].state = idle();
        step(&mut s, &mut pets, &w);
        waved |= matches!(pets[1].state, State::Farewell { .. });
        assert!(waved || pets[1].standing.is_none() || pets[1].body.y == GROUND);
    }
    assert!(waved);
}

#[test]
fn a_rider_does_not_poop_notice_poop_or_socialise() {
    let w = world(vec![]);
    for seed in 0..50 {
        let mut s = Social::new(seed);
        let mut pets = tower(&mut s, &w, 2, 800.0);
        pets.push(ground(&w, friendly(), 860.0, seed + 500));
        step(&mut s, &mut pets, &w);
        assert!(pets[1].is_on_head(), "{:?}", pets[1].state);
        assert!(pets[1].try_poop().is_none());
        assert!(!pets[1].social_free());
        let before = pets[1].state;
        let y = pets[1].body.y;
        pets[1].notice_poops(&[PoopSpot { id: 1, x: 810.0, y, owner: Owner::Ground(0) }], &w);
        assert_eq!(pets[1].state, before);
        assert_eq!(pets[1].social_role(), None, "no greeting up there");
    }
}

#[test]
fn fliers_take_no_part() {
    let w = world(vec![]);
    // 떠 있는 꿀벌 머리에 살며시: 그대로 지나간다
    let mut a = Pet::new(friendly(), 800.0, 600.0 - HEAD - 3.0, 1);
    a.force_throw(0.0, 0.0);
    let mut pets = vec![a, bee_at(800.0, 600.0, 2)];
    let mut s = Social::new(1);
    for _ in 0..20 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[0].is_on_head());
    }
    // 꿀벌이 떨어져도 머리에 서지 않는다
    let mut b = bee_at(800.0, GROUND - HEAD - 3.0, 3);
    b.force_throw(0.0, 10.0);
    assert!(!b.can_perch());
    let mut pets = vec![b, ground(&w, friendly(), 800.0, 4)];
    let mut s = Social::new(2);
    for _ in 0..20 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[0].is_on_head());
    }
}

// ---- 살며시 내려놓기 ----

/// 커서로 잡아서 놓는 진짜 길 (y = 발 높이)
fn dropped_at(x: f64, y: f64, seed: u64) -> Pet {
    let mut p = Pet::new(friendly(), x, y, seed);
    p.standing = None;
    p.state = State::Air { anim: Anim::Fall };
    p.body = Body { x, y, vx: 0.0, vy: 0.0 };
    let (cx, cy) = (x, y - 60.0);
    p.pointer_down(cx, cy);
    p.pointer_move(cx + 10.0, cy);
    p.pointer_move(cx, cy);
    p
}

#[test]
fn a_drop_from_high_up_still_stacks_when_it_lines_up_but_not_when_it_misses() {
    let w = world(vec![]);
    for (x, want) in [(805.0, true), (800.0 + 200.0, false)] {
        let mut p = dropped_at(x, GROUND - HEAD - 600.0, 1);
        let (cx, cy) = (p.body.x, p.body.y - 60.0);
        p.update(DT, &w);
        let mut pets = vec![p, ground(&w, friendly(), 800.0, 2)];
        let mut s = Social::new(1);
        s.tick(DT, &mut pets);
        pets[0].pointer_up(cx, cy);
        let (mut stacked, mut fastest) = (false, 0.0f64);
        for _ in 0..120 {
            pets[1].state = idle();
            step(&mut s, &mut pets, &w);
            fastest = fastest.max(pets[0].body.vy);
            stacked |= pets[0].is_on_head();
        }
        assert_eq!(stacked, want, "x {x}");
        assert!(fastest > crate::collide::HIT_MIN_SPEED, "fell fast: {fastest}");
        if !want {
            assert!(on_real_surface(&pets[0], &w));
        }
    }
}

#[test]
fn a_gentle_drop_stacks_and_ends_on_a_real_surface() {
    let w = world(vec![]);
    let mut p = dropped_at(805.0, GROUND - HEAD - 20.0, 1);
    let (cx, cy) = (p.body.x, p.body.y - 60.0);
    p.update(DT, &w);
    let mut pets = vec![p, ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    pets[0].pointer_up(cx, cy);
    assert!(pets[0].is_flung());
    let mut stacked = false;
    for _ in 0..30 {
        pets[1].state = idle();
        step(&mut s, &mut pets, &w);
        stacked |= pets[0].is_on_head();
    }
    assert!(stacked);
    assert!(pets[0].is_on_head() && pets[1].has_rider());
    assert_eq!(pets[0].body.x, pets[1].body.x);
    assert_eq!(pets[0].body.y, GROUND - HEAD);
    assert!(pets[1].wobbling() || pets[1].take_speech().is_some());
    // 시간이 되면 내려와 땅에 선다
    pets[0].expire_ride();
    all_land(&mut s, &mut pets, &w, 2.0);
    assert!(on_real_surface(&pets[0], &w));
}

#[test]
fn setting_a_pet_down_with_its_feet_in_the_top_of_the_head_also_stacks() {
    let w = world(vec![]);
    let mut p = dropped_at(800.0, GROUND - HEAD + 10.0, 1);
    let (cx, cy) = (p.body.x, p.body.y - 60.0);
    p.update(DT, &w);
    let mut pets = vec![p, ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    pets[0].pointer_up(cx, cy);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].is_on_head());
}

#[test]
fn a_sleeping_pet_stays_asleep_when_something_lands_on_it() {
    let w = world(vec![]);
    let mut a = Pet::new(friendly(), 800.0, GROUND - HEAD - 15.0, 1);
    a.force_throw(0.0, 0.0);
    let mut b = ground(&w, friendly(), 800.0, 2);
    b.state = State::Sleep { remaining: 100.0 };
    let mut pets = vec![a, b];
    let mut s = Social::new(1);
    for _ in 0..20 {
        step(&mut s, &mut pets, &w);
    }
    assert!(pets[0].is_on_head());
    assert!(matches!(pets[1].state, State::Sleep { .. }));
    assert!(!pets[1].wobbling());
    assert_eq!(pets[1].take_speech(), None);
}

#[test]
fn a_fast_throw_bounces_off_the_head_instead() {
    let w = world(vec![]);
    let mut a = Pet::new(friendly(), 800.0, GROUND - HEAD - 60.0, 1);
    a.force_throw(0.0, 900.0);
    let mut pets = vec![a, ground(&w, friendly(), 800.0, 2)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    let mut boing = false;
    for _ in 0..15 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[0].is_on_head() && !pets[1].has_rider());
        boing |= pets[0].take_speech() == Some(Speech::Boing);
    }
    assert!(boing);
}

#[test]
fn a_fast_throw_onto_a_tower_top_bounces_and_the_top_stays() {
    let w = world(vec![]);
    let mut s = Social::new(2);
    let mut pets = tower(&mut s, &w, 3, 800.0);
    let top = GROUND - 3.0 * HEAD;
    let mut a = Pet::new(friendly(), 800.0, top - 60.0, 1);
    a.force_throw(0.0, 900.0);
    pets.push(a);
    let mut boing = false;
    s.tick(DT, &mut pets);
    for _ in 0..15 {
        step(&mut s, &mut pets, &w);
        boing |= pets[3].take_speech() == Some(Speech::Boing);
        assert!(!pets[3].is_on_head());
    }
    assert!(boing);
    assert!(pets[2].is_on_head());
    assert_eq!(s.tower_height(pets[0].social_key()), 3);
}

#[test]
fn a_drop_on_a_full_tower_does_not_stack_and_lands_on_the_ground() {
    let w = world(vec![]);
    let mut s = Social::new(3);
    let mut pets = tower(&mut s, &w, MAX_TOWER, 800.0);
    let mut a = Pet::new(friendly(), 800.0, GROUND - MAX_TOWER as f64 * HEAD - 15.0, 1);
    a.force_throw(0.0, 0.0);
    pets.push(a);
    s.tick(DT, &mut pets);
    for _ in 0..12 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[5].is_on_head());
    }
    assert_eq!(s.tower_height(pets[0].social_key()), MAX_TOWER);
    all_land(&mut s, &mut pets, &w, 5.0);
    assert!(on_real_surface(&pets[5], &w));
}

#[test]
fn a_drop_past_the_top_onto_the_middle_only_passes() {
    let w = world(vec![]);
    let little = PetConfig { size: 64.0, half_w: 19.2, ..friendly() };
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 800.0, 2), ground(&w, little, 800.0, 3)];
    let mut s = Social::new(4);
    s.force_board(&mut pets, 1, 0);
    s.force_board(&mut pets, 2, 1);
    s.carry(&mut pets);
    // 맨 위 머리 폭 바깥, 가운데 층 머리 바로 위에서 놓는다
    let mut a = Pet::new(friendly(), 830.0, GROUND - 2.0 * HEAD - 10.0, 9);
    a.force_throw(0.0, 0.0);
    pets.push(a);
    s.tick(DT, &mut pets);
    for _ in 0..15 {
        step(&mut s, &mut pets, &w);
        assert!(!pets[3].is_on_head());
    }
    assert_eq!(s.tower_height(pets[0].social_key()), 3);
    assert!(pets[1].is_on_head() && pets[2].is_on_head());
}

#[test]
fn the_work_area_top_limits_the_height() {
    // 작업 영역 높이 400: 128 몸은 2층까지 (머리 102 + 몸 128)
    let w = World::new(vec![], vec![Rect::new(0.0, 640.0, 1920.0, GROUND)], 128.0);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 800.0, 2), ground(&w, friendly(), 800.0, 3), ground(&w, friendly(), 800.0, 4)];
    let mut s = Social::new(5);
    for p in &mut pets {
        p.update(DT, &w);
        p.state = idle();
    }
    assert!(s.can_host(&pets, 0, 128.0));
    s.force_board(&mut pets, 1, 0);
    s.carry(&mut pets);
    pets[1].update(DT, &w);
    assert!(s.can_host(&pets, 1, 128.0), "head at 836, room for a body");
    s.force_board(&mut pets, 2, 1);
    s.carry(&mut pets);
    pets[2].update(DT, &w);
    assert!(!s.can_host(&pets, 2, 128.0), "would go above the work area");
    // 작은 몸이면 들어간다 (head 734 - 64 >= 640)
    assert!(s.can_host(&pets, 2, 64.0), "real sizes");
}

// ---- 뛰어오르기 ----

#[test]
fn a_meet_can_end_standing_on_the_head_while_the_other_walks_on() {
    let w = world(vec![]);
    for seed in 0..8 {
        let mut pets = head_on_pair(&w, seed, seed + 40, Move::Walk);
        let mut s = Social::new(seed);
        s.meet_chance = 1.0;
        s.meet_only = Some(MeetKind::Climb);
        let mut rider = None;
        for _ in 0..120 {
            step(&mut s, &mut pets, &w);
            rider = pets.iter().position(Pet::is_on_head);
            if rider.is_some() {
                break;
            }
        }
        let r = rider.expect("climbed");
        let c = 1 - r;
        assert!(matches!(pets[c].state, State::Move { .. }), "{:?}", pets[c].state);
        let x = pets[c].body.x;
        for _ in 0..30 {
            pets[r].state = State::OnHead { remaining: 50.0, age: 0.0 };
            step(&mut s, &mut pets, &w);
            assert_eq!(pets[r].body.x, pets[c].body.x);
            assert_eq!(pets[r].body.y, GROUND - HEAD);
        }
        assert!((pets[c].body.x - x).abs() > 5.0, "carried");
    }
}

#[test]
fn climbing_takes_its_share_of_the_meet_outcomes() {
    let w = world(vec![]);
    let (mut climbs, n) = (0, 800);
    for seed in 0..n {
        let mut pets = head_on_pair(&w, seed, seed + 5000, Move::Walk);
        let mut s = Social::new(seed);
        s.meet_chance = 1.0;
        s.tick(DT, &mut pets);
        climbs += usize::from(!s.climbs.is_empty());
    }
    let share = climbs as f64 / n as f64;
    let want = MEET_KINDS.iter().find(|(k, _)| *k == MeetKind::Climb).unwrap().1;
    assert!((share - want).abs() < 0.05, "{share}");
}

#[test]
fn a_walker_or_runner_climbs_a_tower_it_walks_into() {
    let w = world(vec![]);
    for kind in [Move::Walk, Move::Run] {
        let mut s = Social::new(1);
        s.tower_chance = 1.0;
        let mut pets = tower(&mut s, &w, 3, 800.0);
        let mut a = ground(&w, friendly(), 620.0, 9);
        walking(&mut a, 1.0, kind);
        a.update(DT, &w);
        pets.push(a);
        let mut up = false;
        for _ in 0..240 {
            pets[0].state = idle();
            for p in &mut pets[1..3] {
                p.state = State::OnHead { remaining: 50.0, age: 0.0 };
            }
            step(&mut s, &mut pets, &w);
            if pets[3].is_on_head() {
                up = true;
                // 닿은 다음 틱에 코디네이터가 태운다
                step(&mut s, &mut pets, &w);
                break;
            }
        }
        assert!(up, "{kind:?}");
        assert_eq!(s.tower_height(pets[0].social_key()), 4);
        assert_eq!(s.top_of(pets[0].social_key()), pets[3].social_key());
        assert!((pets[3].body.y - (GROUND - 3.0 * HEAD)).abs() < 1e-6, "{}", pets[3].body.y);
    }
}

#[test]
fn walking_into_a_tower_climbs_it_at_the_stated_chance_and_a_full_one_never() {
    let w = world(vec![]);
    let (mut started, n) = (0, 600);
    for seed in 0..n {
        let mut s = Social::new(seed);
        let mut pets = tower(&mut s, &w, 2, 800.0);
        let mut a = ground(&w, friendly(), 650.0, seed + 9000);
        walking(&mut a, 1.0, Move::Walk);
        a.update(DT, &w);
        pets.push(a);
        s.tick(DT, &mut pets);
        started += usize::from(!s.climbs.is_empty());
    }
    let share = started as f64 / n as f64;
    assert!((share - TOWER_CLIMB_CHANCE).abs() < 0.06, "{share}");

    let mut s = Social::new(1);
    s.tower_chance = 1.0;
    let mut pets = tower(&mut s, &w, MAX_TOWER, 800.0);
    let mut a = ground(&w, friendly(), 650.0, 9);
    walking(&mut a, 1.0, Move::Walk);
    a.update(DT, &w);
    pets.push(a);
    s.tick(DT, &mut pets);
    assert!(s.climbs.is_empty(), "full");
}

#[test]
fn a_pet_with_a_rider_is_not_vaulted_or_stomped() {
    let w = world(vec![]);
    let mut s = Social::new(1);
    s.meet_chance = 1.0;
    s.meet_only = Some(MeetKind::Stomp);
    s.tower_chance = 0.0;
    let mut pets = tower(&mut s, &w, 2, 1000.0);
    walking(&mut pets[0], -1.0, Move::Walk);
    pets[0].update(DT, &w);
    let mut a = ground(&w, friendly(), 800.0, 9);
    walking(&mut a, 1.0, Move::Walk);
    a.update(DT, &w);
    pets.push(a);
    s.tick(DT, &mut pets);
    assert!(!pets[2].is_meeting() && !pets[0].is_meeting());
}

#[test]
fn a_climb_whose_target_left_just_falls_to_a_surface() {
    let w = world(vec![]);
    let mut a = ground(&w, friendly(), 800.0, 1);
    let mut b = ground(&w, friendly(), 1000.0, 2);
    walking(&mut a, 1.0, Move::Walk);
    walking(&mut b, -1.0, Move::Walk);
    let mut pets = vec![a, b];
    for p in &mut pets {
        p.update(DT, &w);
    }
    let mut s = Social::new(1);
    s.meet_chance = 1.0;
    s.meet_only = Some(MeetKind::Climb);
    s.tick(DT, &mut pets);
    let j = if pets[0].is_climbing() { 0 } else { 1 };
    assert!(pets[j].is_climbing(), "{:?}", pets[j].state);
    let t = 1 - j;
    pets[t].state = State::Dragged;
    all_land_held(&mut s, &mut pets, &w, 3.0, t);
    assert!(on_real_surface(&pets[j], &w), "{:?}", pets[j].state);
}

fn all_land_held(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64, held: usize) {
    for _ in 0..(secs / DT) as usize {
        pets[held].state = State::Dragged;
        step(s, pets, w);
    }
}

/// 많이 놀려도: 높이 제한을 지키고, 아무도 상한을 넘겨 머리 위에 있지 않고, 그만 놀게 하면 모두 땅에 선다
#[test]
fn soak_towers_form_stay_bounded_and_always_come_down() {
    let w = world(vec![w1()]);
    let mut tallest = 0;
    for seed in 0..6 {
        let mut pets: Vec<Pet> = (0..11).map(|i| ground(&w, friendly(), 200.0 + 150.0 * i as f64, seed * 100 + i)).collect();
        let mut s = Social::new(seed);
        s.meet_chance = 1.0;
        s.tower_chance = 1.0;
        s.meet_only = Some(MeetKind::Climb);
        for _ in 0..(120.0 / DT) as usize {
            step(&mut s, &mut pets, &w);
            for p in &pets {
                let h = s.tower_height(p.social_key());
                assert!(h <= MAX_TOWER, "{h}");
                tallest = tallest.max(h);
                if let Some((_, age)) = p.ride_clock() {
                    assert!(age <= RIDE_MAX_SECS, "{age}");
                }
                assert!(p.is_on_head() || p.standing.is_some() || !matches!(p.state, State::Idle { .. } | State::Move { .. }), "{:?}", p.state);
            }
        }
        s.tower_chance = 0.0;
        s.meet_chance = 0.0;
        s.one_way_chance = 0.0;
        for _ in 0..((RIDE_MAX_SECS + 5.0) / DT) as usize {
            step(&mut s, &mut pets, &w);
        }
        for p in &pets {
            assert!(!p.is_on_head());
            assert!(p.standing.is_some() || matches!(p.state, State::Air { .. } | State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::Leap { .. }), "{:?}", p.state);
            assert!(p.body.y <= GROUND + 1e-9 && p.body.x >= 0.0 && p.body.x <= 1920.0, "{:?}", p.body);
        }
    }
    assert!(tallest >= 3, "towers should form: {tallest}");
}

// ---- 달리기 ----

#[test]
fn a_running_jumper_lands_farther_and_leaves_sooner_than_a_walking_one() {
    let w = world(vec![]);
    // 0번이 뛰는 시드를 찾는다
    let outcome = |run: bool| {
        for seed in 0..40 {
            let mut pets = head_on_pair(&w, seed, seed + 40, if run { Move::Run } else { Move::Walk });
            let mut s = Social::new(seed);
            s.meet_chance = 1.0;
            s.meet_only = Some(MeetKind::Stomp);
            let (mut at, mut other, mut crouch) = (None, 0.0, 0);
            for _ in 0..600 {
                s.tick(DT, &mut pets);
                if crouch == 0 && matches!(pets[1].state, State::Meet { .. }) && !matches!(pets[0].state, State::Meet { phase: crate::brain::meet_moves::MeetPhase::Crouch, .. }) {
                    break;
                }
                if matches!(pets[0].state, State::Meet { phase: crate::brain::meet_moves::MeetPhase::Crouch, .. }) {
                    crouch += 1;
                }
                for p in &mut pets {
                    p.update(DT, &w);
                }
                s.carry(&mut pets);
                if crouch > 0 && matches!(pets[0].state, State::Meet { phase: crate::brain::meet_moves::MeetPhase::Landing, .. }) {
                    at = Some(pets[0].body.x);
                    other = pets[1].body.x;
                    break;
                }
            }
            if let Some(x) = at {
                return (x - other, crouch);
            }
        }
        panic!("no jump");
    };
    let (run_gap, run_crouch) = outcome(true);
    let (walk_gap, walk_crouch) = outcome(false);
    assert!(run_gap > walk_gap + 40.0, "{run_gap} vs {walk_gap}");
    assert!(run_crouch < walk_crouch, "{run_crouch} vs {walk_crouch}");
}

#[test]
fn a_running_landing_still_needs_room_on_the_surface() {
    let w = world(vec![w1()]);
    // 창 오른쪽 끝 근처: 달린 만큼은 자리가 없고, 바로 너머엔 있다
    let mut a = pet_on(&w, friendly(), Owner::Window(1), 860.0, 3);
    let mut b = pet_on(&w, friendly(), Owner::Window(1), 950.0, 4);
    walking(&mut a, 1.0, Move::Run);
    walking(&mut b, -1.0, Move::Walk);
    a.update(DT, &w);
    b.update(DT, &w);
    let sa = Snap::of(&a);
    let sb = Snap::of(&b);
    assert!(sa.run_carry > 0.0);
    let x = landing_past(&sa, &sb).expect("room right past");
    assert!(x <= 1100.0 - a.cfg.half_w);
    assert!((x - (sb.x + a.cfg.half_w + b.cfg.half_w + MEET_GAP * 128.0)).abs() < 1e-6, "falls back to the near spot: {x}");
    // 아주 끝이면 못 뛴다
    let mut c = pet_on(&w, friendly(), Owner::Window(1), 1060.0, 5);
    walking(&mut c, 1.0, Move::Run);
    c.update(DT, &w);
    let sc = Snap::of(&c);
    let sd = Snap { x: 1050.0, ..sb };
    assert!(landing_past(&sc, &sd).is_none());
}
