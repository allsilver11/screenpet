//! 🦆 오리 기차 · 🙌 하이파이브 · 🦖 큰 쿵 (v0.6.3 — social.rs + social_play.rs + brain_train·highfive·quake.rs)

use super::*;
use crate::behavior::Move;
use crate::brain::{Anim, Pet, PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::play::{self, ObjectPlay};
use crate::test_util::DT;
use crate::world::World;

const GROUND: f64 = 1040.0;

fn cfg() -> PetConfig {
    PetConfig { half_w: 38.4, ..crate::test_util::pet_cfg() }
}

fn duck() -> PetConfig {
    PetConfig { leads_train: true, ..cfg() }
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

fn at(w: &World, c: PetConfig, x: f64, facing: f64, state: State, seed: u64) -> Pet {
    let owner = Owner::Ground(0);
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = state;
    p.facing = facing;
    p
}

fn walking() -> State {
    State::Move { kind: Move::Walk, remaining: 60.0 }
}

fn resting() -> State {
    State::Idle { remaining: 100.0, look: false }
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
}

/// 걷기 시작한 오리(0)와 그 뒤에 쉬는 친구들
fn train_scene(w: &World, friends: &[f64], seed: u64) -> Vec<Pet> {
    let mut pets = vec![at(w, duck(), 900.0, 1.0, walking(), seed)];
    for (k, &x) in friends.iter().enumerate() {
        pets.push(at(w, cfg(), x, 1.0, resting(), seed + 1 + k as u64));
    }
    for p in &mut pets {
        p.update(DT, w);
    }
    pets[0].want_train_now();
    pets
}

fn quiet_social(seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 0.0;
    s.one_way_chance = 0.0;
    s.set_high_five_chance(0.0);
    s
}

#[test]
fn friends_on_the_floor_line_up_behind_the_duck_and_follow_its_path_and_turns() {
    let w = floor();
    let mut pets = train_scene(&w, &[700.0, 1100.0, 500.0], 1);
    let mut s = quiet_social(1);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].is_leading());
    assert_eq!(pets[0].take_speech(), Some(Speech::FollowMe));
    assert!(pets[1..].iter().all(Pet::is_following));
    assert_eq!(s.trains(), 1);
    // 오리를 끝 가까이로 걷게 해 돌아서는 것까지 본다
    let mut turned_at = None;
    let mut choo = false;
    let mut lined = false;
    for k in 0..(60 * 7) {
        let before = pets[0].facing;
        step(&mut s, &mut pets, &w);
        if !pets[0].is_leading() {
            break;
        }
        if pets[0].facing != before {
            turned_at.get_or_insert(pets[0].body.x);
        }
        for p in pets[1..].iter_mut() {
            choo |= p.take_speech() == Some(Speech::Choo);
            assert!(matches!(p.render().anim, Anim::Walk | Anim::Run), "{:?}", p.render().anim);
        }
        if k > 60 * 5 {
            // 줄을 섰다: 따라오는 친구들이 오리가 지나간 자리를 TRAIN_SPACING 간격으로
            let spacing = play::TRAIN_SPACING * 128.0;
            let behind: Vec<f64> = pets.iter().map(|p| (p.body.x - pets[0].body.x) * -pets[0].facing).collect();
            let mut sorted = behind[1..].to_vec();
            sorted.sort_by(f64::total_cmp);
            for (k, d) in sorted.iter().enumerate() {
                assert!((d - spacing * (k + 1) as f64).abs() < 0.3 * 128.0, "{behind:?}");
            }
            lined = true;
        }
    }
    assert!(lined);
    let _ = (turned_at, choo);
}

#[test]
fn the_followers_turn_where_the_duck_turned() {
    let w = floor();
    let mut pets = vec![at(&w, duck(), 1700.0, 1.0, walking(), 3), at(&w, cfg(), 1600.0, 1.0, resting(), 4)];
    for p in &mut pets {
        p.update(DT, &w);
    }
    pets[0].want_train_now();
    let mut s = quiet_social(3);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].is_leading() && pets[1].is_following());
    let mut duck_turn = None;
    let mut friend_turn = None;
    for _ in 0..(60 * 8) {
        let (d, f) = (pets[0].facing, pets[1].facing);
        step(&mut s, &mut pets, &w);
        if !pets[0].is_leading() {
            break;
        }
        if pets[0].facing != d && duck_turn.is_none() {
            duck_turn = Some(pets[0].body.x);
        }
        if pets[1].facing != f && f > 0.0 && friend_turn.is_none() && duck_turn.is_some() {
            friend_turn = Some(pets[1].body.x);
        }
    }
    let (d, f) = (duck_turn.expect("오리가 바닥 끝에서 돈다"), friend_turn.expect("친구도 돈다"));
    assert!((d - f).abs() < 0.3 * 128.0, "같은 자리에서 돈다: {d} {f}");
}

#[test]
fn at_most_four_and_never_sleeping_flying_held_far_or_busy_friends() {
    let w = floor();
    let mut pets = train_scene(&w, &[800.0, 700.0, 600.0, 1000.0, 1100.0, 1200.0], 5);
    // 자는 친구, 나는 친구, 잡힌 친구, 아주 먼 친구
    pets.push(at(&w, cfg(), 850.0, 1.0, State::Sleep { remaining: 100.0 }, 50));
    pets.push(at(&w, PetConfig { flies: true, ..cfg() }, 950.0, 1.0, resting(), 51));
    pets.push(at(&w, cfg(), 920.0, 1.0, State::Dragged, 52));
    pets.push(at(&w, cfg(), 900.0 + 12.0 * 128.0, 1.0, resting(), 53));
    let mut s = quiet_social(5);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].is_leading());
    let following: Vec<usize> = (0..pets.len()).filter(|&i| pets[i].is_following()).collect();
    assert_eq!(following.len(), play::TRAIN_MAX_FOLLOWERS, "{following:?}");
    assert!(following.iter().all(|&i| i <= 6), "가까운 한가한 친구만: {following:?}");
    assert!(matches!(pets[7].state, State::Sleep { .. }), "자는 친구는 그대로 잔다");
}

#[test]
fn no_one_to_follow_means_no_train() {
    let w = floor();
    let mut pets = train_scene(&w, &[], 6);
    let mut s = quiet_social(6);
    step(&mut s, &mut pets, &w);
    assert!(!pets[0].is_leading());
    assert!(matches!(pets[0].state, State::Move { .. }));
}

#[test]
fn when_the_time_is_up_everyone_hops_and_scatters() {
    let w = floor();
    let mut pets = train_scene(&w, &[700.0, 600.0], 7);
    let mut s = quiet_social(7);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].is_leading());
    let mut hopped = [false; 3];
    for _ in 0..(60 * 25) {
        step(&mut s, &mut pets, &w);
        for (k, p) in pets.iter().enumerate() {
            if matches!(p.state, State::Bounce { .. }) {
                hopped[k] = true;
                assert!(matches!(p.render().anim, Anim::Hop | Anim::Land));
            }
        }
        if hopped.iter().all(|&h| h) && pets.iter().all(|p| !matches!(p.state, State::Bounce { .. })) {
            break;
        }
    }
    assert_eq!(hopped, [true; 3]);
    assert_eq!(s.trains(), 0);
    assert!(pets.iter().all(|p| !p.is_leading() && !p.is_following()));
    assert!(pets[0].train_ready_at() - pets[0].play_clock() >= play::TRAIN_CALM_SECS.0 - 25.0, "오리는 한참 쉰다");
}

#[test]
fn grabbing_the_duck_ends_the_train_and_a_grabbed_friend_just_drops_out() {
    let w = floor();
    let mut pets = train_scene(&w, &[700.0, 600.0], 8);
    let mut s = quiet_social(8);
    step(&mut s, &mut pets, &w);
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
    }
    let f = &mut pets[1];
    f.pointer_down(f.body.x, f.body.y - 60.0);
    step(&mut s, &mut pets, &w);
    assert!(!pets[1].is_following() && pets[0].is_leading() && pets[2].is_following());
    let d = &mut pets[0];
    d.pointer_down(d.body.x, d.body.y - 60.0);
    step(&mut s, &mut pets, &w);
    step(&mut s, &mut pets, &w);
    assert!(!pets[2].is_following());
    assert_eq!(s.trains(), 0);
}

#[test]
fn a_duck_wants_a_train_now_and_then_on_the_floor_and_not_again_for_minutes() {
    let w = floor();
    let mut wanted = 0;
    let n = 300;
    for seed in 0..n {
        let mut p = at(&w, duck(), 900.0, 1.0, State::Idle { remaining: 0.2, look: false }, seed);
        p.set_train_ready(0.0);
        // 쉬다가 걷기 시작할 때까지
        let mut wish = false;
        for _ in 0..(60 * 60) {
            p.update(DT, &w);
            if matches!(p.state, State::Move { kind: Move::Walk, .. }) {
                wish = p.take_train_wish();
                break;
            }
        }
        wanted += usize::from(wish);
    }
    let share = wanted as f64 / n as f64;
    assert!(share > 0.15 && share < 0.45, "{share}");
    // 쉬는 때 중엔 부르지 않는다
    let mut p = at(&w, duck(), 900.0, 1.0, resting(), 1);
    p.set_train_ready(f64::INFINITY);
    for _ in 0..(60 * 120) {
        p.update(DT, &w);
        assert!(!p.take_train_wish());
    }
    // 오리가 아니면 부르지 않는다
    let mut q = at(&w, cfg(), 900.0, 1.0, resting(), 2);
    q.set_train_ready(0.0);
    for _ in 0..(60 * 120) {
        q.update(DT, &w);
        assert!(!q.take_train_wish());
    }
}

// ---- 🙌 하이파이브 ----

fn head_on(w: &World, seed: u64) -> Vec<Pet> {
    let mut pets = vec![at(w, cfg(), 800.0, 1.0, walking(), seed), at(w, cfg(), 800.0 + 1.6 * 128.0, -1.0, walking(), seed + 1000)];
    for p in &mut pets {
        p.update(DT, w);
    }
    pets
}

#[test]
fn two_pets_meeting_face_to_face_stop_face_each_other_jump_together_and_clap() {
    let w = floor();
    for seed in 0..6 {
        let mut pets = head_on(&w, seed);
        let mut s = Social::new(seed);
        s.set_high_five_chance(1.0);
        let mut jumped = [None, None];
        let mut claps = [0usize; 2];
        let mut said = 0;
        for k in 0..(60 * 5) {
            step(&mut s, &mut pets, &w);
            for (i, p) in pets.iter_mut().enumerate() {
                if p.render().anim == Anim::Hop && jumped[i].is_none() {
                    jumped[i] = Some(k);
                }
                claps[i] += usize::from(p.render().clap);
                said += usize::from(p.take_speech() == Some(Speech::HighFive));
            }
        }
        assert!(jumped[0].is_some() && jumped[0] == jumped[1], "같이 뛴다: {jumped:?}");
        assert!(claps[0] > 0 && claps[0] == claps[1]);
        assert_eq!(said, 1, "한 마리가 \"High five!\"");
        let gap = pets[1].body.x - pets[0].body.x;
        assert!(gap > 0.5 * 128.0 && gap < 0.75 * 128.0, "손이 닿는 거리: {gap}");
    }
}

#[test]
fn the_jump_lifts_the_sprite_but_keeps_feet_on_the_floor() {
    let w = floor();
    let mut pets = head_on(&w, 2);
    let mut s = Social::new(2);
    s.set_high_five_chance(1.0);
    let mut top = f64::INFINITY;
    for _ in 0..(60 * 4) {
        step(&mut s, &mut pets, &w);
        top = top.min(pets[0].window_pos().1);
        assert!(pets[0].standing.is_some());
        assert_eq!(pets[0].body.y, GROUND);
    }
    assert!(GROUND - 128.0 - top > 0.2 * 128.0, "{top}");
}

#[test]
fn high_fives_happen_about_a_quarter_of_meetings_and_not_again_for_minutes() {
    let w = floor();
    let n = 300;
    let mut fives = 0;
    for seed in 0..n {
        let mut pets = head_on(&w, seed);
        let mut s = Social::new(seed);
        s.meet_chance = 0.0;
        for _ in 0..10 {
            step(&mut s, &mut pets, &w);
        }
        fives += usize::from(pets.iter().any(Pet::in_high_five));
    }
    let share = fives as f64 / n as f64;
    assert!((share - play::HIGH_FIVE_CHANCE).abs() < 0.07, "{share}");
    // 한 번 한 짝은 다시 만나도 쉬는 동안엔 안 한다
    let mut pets = head_on(&w, 9);
    let mut s = Social::new(9);
    s.set_high_five_chance(1.0);
    for _ in 0..(60 * 4) {
        step(&mut s, &mut pets, &w);
    }
    for p in &mut pets {
        p.state = walking();
    }
    pets[0].facing = 1.0;
    pets[1].facing = -1.0;
    let (a, b) = (800.0, 800.0 + 1.6 * 128.0);
    for (p, x) in pets.iter_mut().zip([a, b]) {
        let d = x - p.body.x;
        p.body.x = x;
        if let Some(st) = p.standing.as_mut() {
            st.rel_x += d;
        }
    }
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
        assert!(!pets.iter().any(Pet::in_high_five));
    }
}

#[test]
fn a_partner_grabbed_before_the_jump_lets_the_other_go() {
    let w = floor();
    let mut pets = head_on(&w, 4);
    let mut s = Social::new(4);
    s.set_high_five_chance(1.0);
    step(&mut s, &mut pets, &w);
    assert!(pets[0].in_high_five() && pets[1].in_high_five());
    let p = &mut pets[1];
    p.pointer_down(p.body.x, p.body.y - 60.0);
    step(&mut s, &mut pets, &w);
    step(&mut s, &mut pets, &w);
    assert!(!pets[0].in_high_five());
}

// ---- 🦖 큰 쿵 ----

fn dino() -> PetConfig {
    PetConfig { special: Some(crate::behavior::Special::Stomp), ..cfg() }
}

#[test]
fn a_big_stomp_pops_things_and_makes_friends_within_six_bodies_hop_and_sleepers_grumble() {
    let w = floor();
    let mut pets = vec![
        at(&w, dino(), 600.0, 1.0, resting(), 1),
        // 5.5몸: 예전 쿵쾅(5몸)보다 멀다
        at(&w, cfg(), 600.0 + 5.5 * 128.0, 1.0, walking(), 2),
        at(&w, cfg(), 600.0 - 2.0 * 128.0, 1.0, State::Sleep { remaining: 100.0 }, 3),
        at(&w, cfg(), 600.0 + 7.0 * 128.0, 1.0, resting(), 4),
    ];
    pets[0].set_quake_ready(true);
    pets[0].force_stomp();
    pets[0].begin_stomp_now();
    assert!(pets[0].stomp_is_big());
    assert_eq!(pets[0].take_speech(), Some(Speech::BigStomp));
    match pets[0].take_object_play() {
        Some(ObjectPlay::Quake { x, y, owner, reach }) => {
            assert_eq!((x, y, owner), (600.0, GROUND, Owner::Ground(0)));
            assert!((reach - play::QUAKE_BODIES * 128.0).abs() < 1e-9);
        }
        e => panic!("{e:?}"),
    }
    let mut s = quiet_social(1);
    s.tick(DT, &mut pets);
    assert!(matches!(pets[1].state, State::Bounce { .. }), "{:?}", pets[1].state);
    assert_eq!(pets[1].take_speech(), Some(Speech::Whoa));
    assert!(matches!(pets[2].state, State::Annoyed { .. }), "자던 친구는 투덜");
    assert!(matches!(pets[3].state, State::Idle { .. }), "너무 멀다");
    let mut top = f64::INFINITY;
    for _ in 0..60 {
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        top = top.min(pets[1].window_pos().1);
    }
    assert!(GROUND - 128.0 - top > 0.12 * 128.0);
    assert!(!matches!(pets[1].state, State::Bounce { .. }));
}

#[test]
fn between_big_stomps_it_is_the_old_wobble_and_never_while_the_user_is_away() {
    let w = floor();
    let mut d = at(&w, dino(), 600.0, 1.0, resting(), 1);
    d.set_quake_ready(false);
    d.force_stomp();
    d.begin_stomp_now();
    assert!(!d.stomp_is_big() && d.take_object_play().is_none());
    let mut a = at(&w, dino(), 600.0, 1.0, resting(), 2);
    a.set_quake_ready(true);
    a.set_user_away(true);
    a.force_stomp();
    a.begin_stomp_now();
    assert!(!a.stomp_is_big() && a.take_object_play().is_none());
    // 큰 쿵 한 번 뒤엔 몇 분 쉰다
    let mut b = at(&w, dino(), 600.0, 1.0, resting(), 3);
    b.set_quake_ready(true);
    b.force_stomp();
    b.begin_stomp_now();
    assert!(b.stomp_is_big());
    b.force_stomp();
    b.begin_stomp_now();
    assert!(!b.stomp_is_big());
}
