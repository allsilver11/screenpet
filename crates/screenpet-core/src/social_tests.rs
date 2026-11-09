//! 동물끼리 인사·술래 놀이 (social.rs + brain_social.rs) 테스트

use super::*;
use crate::behavior::Personality;
use crate::brain::{Anim, Pet, PetConfig, Speech, Standing, State};
use crate::geom::{Rect, WindowInfo};
use crate::physics::Body;
use crate::visit::Side;
use crate::world::World;
use crate::test_util::{DT, idle};

const GROUND: f64 = 1040.0;

fn cfg(personality: Personality) -> PetConfig {
    PetConfig { personality, half_w: 38.4, ..crate::test_util::pet_cfg() }
}

fn friendly() -> PetConfig {
    cfg(Personality::Friendly)
}

fn flyer() -> PetConfig {
    PetConfig { flies: true, ..friendly() }
}

fn frog() -> PetConfig {
    PetConfig { hops: true, ..cfg(Personality::Playful) }
}

fn world(windows: Vec<WindowInfo>) -> World {
    World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 창 1: 윗면 y=400, x 500..1100
fn w1() -> WindowInfo {
    WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) }
}

/// owner 위 x에 쉬는 동물
fn pet_on(w: &World, c: PetConfig, owner: Owner, x: f64, seed: u64) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    let (ax, ay) = w.anchor(owner).unwrap();
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
    p.state = idle();
    p
}

/// 떠 있는 꿀벌
fn bee_at(x: f64, y: f64, seed: u64) -> Pet {
    let mut p = Pet::new(flyer(), x, y, seed);
    p.standing = None;
    p.body = Body { x, y, vx: 0.0, vy: 0.0 };
    p.state = idle();
    p
}

fn ground(w: &World, c: PetConfig, x: f64, seed: u64) -> Pet {
    pet_on(w, c, Owner::Ground(0), x, seed)
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
    s.carry(pets);
}

fn run(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) {
    for _ in 0..(secs / DT).round() as usize {
        step(s, pets, w);
    }
}

fn roles(pets: &[Pet]) -> Vec<Option<SocialRole>> {
    pets.iter().map(|p| p.social_role()).collect()
}

fn is_greet(p: &Pet) -> bool {
    p.social_role() == Some(SocialRole::Greet)
}

/// 놀이 밖이면 제자리에 다시 앉힌다 (돌아다니지 않게 — 쿨다운만 본다)
fn pin(p: &mut Pet, w: &World, x: f64) {
    if p.social_role().is_none() {
        let fresh = ground(w, friendly(), x, 1);
        p.body = fresh.body;
        p.standing = fresh.standing;
        p.state = idle();
    }
}

#[test]
fn close_pets_on_the_same_surface_stop_face_each_other_and_say_hi() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2)];
    pets[0].facing = -1.0;
    pets[1].facing = 1.0;
    let mut s = Social::new(7);
    step(&mut s, &mut pets, &w);
    assert_eq!(roles(&pets), vec![Some(SocialRole::Greet); 2]);
    assert_eq!(s.active(), 1);
    assert_eq!(pets[0].facing, 1.0);
    assert_eq!(pets[1].facing, -1.0);
    for p in &mut pets {
        assert_eq!(p.take_speech(), Some(Speech::Greet));
        // 먼저 친구 쪽을 쳐다보고, 작은 하트
        assert_eq!(p.render().anim, Anim::Look);
        assert!(p.render().heart);
    }
    run(&mut s, &mut pets, &w, 0.8);
    for p in &pets {
        // 인사는 1.5초 넘게 가니 아직 인사 중이다
        assert!(is_greet(p), "{:?}", p.state);
        assert!(matches!(p.render().anim, Anim::Wave | Anim::Tilt), "{:?}", p.render());
    }
    // 걷지 않고 제자리
    assert_eq!(pets[0].body.x, 800.0);
    assert_eq!(pets[1].body.x, 950.0);
}

#[test]
fn far_apart_or_on_different_surfaces_they_do_not_greet() {
    let w = world(vec![w1()]);
    // 같은 바닥이지만 몸 5개보다 멀다
    let mut pets = vec![ground(&w, friendly(), 200.0, 1), ground(&w, friendly(), 900.0, 2)];
    let mut s = Social::new(1);
    step(&mut s, &mut pets, &w);
    assert_eq!(roles(&pets), vec![None, None]);
    // 같은 x라도 창 위와 바닥은 다른 디딤판
    let mut pets = vec![pet_on(&w, friendly(), Owner::Window(1), 800.0, 1), ground(&w, friendly(), 1150.0, 2)];
    pets[1].body.x = 800.0;
    step(&mut s, &mut pets, &w);
    assert_eq!(roles(&pets), vec![None, None]);
    assert_eq!(s.active(), 0);
}

#[test]
fn greeting_lasts_about_two_seconds_and_the_pair_cools_down() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2)];
    let mut s = Social::new(3);
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut was = false;
    let mut greet_end = None;
    let ticks = (400.0 / DT) as usize;
    for t in 0..ticks {
        s.tick(DT, &mut pets);
        let now = s.active() > 0;
        // 술래 놀이로 이어져도 인사 자체는 여기서 끝이다
        if !starts.is_empty() && greet_end.is_none() && !pets.iter().all(is_greet) {
            greet_end = Some(t as f64 * DT);
        }
        if now != was {
            if now { starts.push(t as f64 * DT) } else { ends.push(t as f64 * DT) }
            was = now;
        }
        for (p, x) in pets.iter_mut().zip([800.0, 950.0]) {
            p.update(DT, &w);
            pin(p, &w, x);
        }
    }
    assert!(starts.len() >= 2, "greets again after the cooldown: {starts:?}");
    for (k, &end) in ends.iter().enumerate() {
        if let Some(&next) = starts.get(k + 1) {
            assert!(next - end >= GREET_COOLDOWN.0 - 0.1, "cooldown too short: {starts:?} {ends:?}");
        }
    }
    // 처음 인사 1.5–2.5초
    let first = greet_end.unwrap() - starts[0];
    assert!((GREET_SECS.0 - 0.05..=GREET_SECS.1 + 0.05).contains(&first), "{first}");
}

#[test]
fn busy_pets_are_ignored() {
    let w = world(vec![]);
    let busy: Vec<Box<dyn Fn(&mut Pet)>> = vec![
        Box::new(|p: &mut Pet| p.state = State::Eating { remaining: 100.0, favorite: false, level_up: false }),
        Box::new(|p: &mut Pet| p.pointer_down(p.body.x, p.body.y - 20.0)),
        Box::new(|p: &mut Pet| p.enter_house()),
        Box::new(|p: &mut Pet| p.state = State::Climb { target_y: 300.0, hang: 1.0 }),
        Box::new(|p: &mut Pet| p.state = State::LevelUp { remaining: 100.0 }),
        Box::new(|p: &mut Pet| p.state = State::Air { anim: Anim::Fall }),
    ];
    for (k, make_busy) in busy.iter().enumerate() {
        let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2)];
        make_busy(&mut pets[1]);
        let mut s = Social::new(k as u64);
        s.tick(DT, &mut pets);
        assert_eq!(pets[0].social_role(), None, "case {k}");
        assert_eq!(s.active(), 0, "case {k}");
    }
}

#[test]
fn a_sleeping_pet_only_gets_a_quiet_look() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2)];
    pets[1].state = State::Sleep { remaining: 100.0 };
    let mut s = Social::new(1);
    step(&mut s, &mut pets, &w);
    assert!(is_greet(&pets[0]));
    assert_eq!(pets[0].render().anim, Anim::Look);
    assert_eq!(pets[0].take_speech(), Some(Speech::Ignore));
    assert!(matches!(pets[1].state, State::Sleep { .. }));
    run(&mut s, &mut pets, &w, PEEK_SECS + 0.2);
    assert_eq!(pets[0].social_role(), None);
    assert!(matches!(pets[1].state, State::Sleep { .. }), "never woken");
    assert_eq!(pets[1].take_speech(), None);
    assert_eq!(s.active(), 0);
}

#[test]
fn chase_starts_with_play_and_ends_when_caught_with_hearts() {
    let w = world(vec![]);
    // 도망자는 화면 끝에서 돌아서 술래와 마주친다
    let mut pets = vec![ground(&w, friendly(), 1500.0, 1), ground(&w, friendly(), 1700.0, 2)];
    let mut s = Social::new(5);
    s.start_play(&mut pets, 0, 1, true);
    assert_eq!(pets[0].social_role(), Some(SocialRole::Chase));
    assert_eq!(pets[1].social_role(), Some(SocialRole::Flee));
    assert_eq!(pets[0].take_speech(), Some(Speech::Play));
    assert_eq!(pets[1].take_speech(), Some(Speech::Flee));
    assert_eq!(pets[1].facing, 1.0, "flees away from the chaser");
    let mut caught_at = None;
    for t in 0..(5.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        if pets.iter().all(|p| p.render().anim == Anim::Heart && p.render().heart) {
            caught_at = Some(t);
            break;
        }
        assert_eq!(pets[0].render().anim, Anim::Run);
    }
    assert!(caught_at.is_some(), "caught within 5 s");
    assert!((pets[0].body.x - pets[1].body.x).abs() <= CATCH_BODIES * 128.0 + 10.0);
    // 하트 뒤 (술래를 바꾸지 않았다면) 놀이 끝 — 바꿨다면 한 번 더 쫓고 끝
    run(&mut s, &mut pets, &w, CAUGHT_SECS + CHASE_SECS.1 + CAUGHT_SECS + 0.5);
    assert_eq!(s.active(), 0);
    assert_eq!(roles(&pets), vec![None, None]);
}

#[test]
fn chase_times_out_when_the_friend_cannot_be_reached() {
    let w = world(vec![w1()]);
    // 술래는 바닥, 도망자는 창 위 — 높이가 달라 못 잡는다
    let mut pets = vec![ground(&w, friendly(), 300.0, 1), pet_on(&w, friendly(), Owner::Window(1), 800.0, 2)];
    let mut s = Social::new(9);
    s.start_play(&mut pets, 0, 1, true);
    run(&mut s, &mut pets, &w, CHASE_SECS.1 + 0.1);
    assert_eq!(s.active(), 0);
    assert_eq!(roles(&pets), vec![None, None]);
    // 그 짝은 한동안 다시 어울리지 않는다 (같은 동물을 바닥에 나란히)
    let w0 = world(vec![]);
    for (p, x) in pets.iter_mut().zip([1200.0, 1300.0]) {
        pin(p, &w0, x);
    }
    s.tick(DT, &mut pets);
    assert_eq!(s.active(), 0, "pair is cooling down");
}

#[test]
fn fleeing_turns_at_window_edges_and_never_falls() {
    let w = world(vec![w1()]);
    let mut pets = vec![ground(&w, friendly(), 1500.0, 1), pet_on(&w, friendly(), Owner::Window(1), 600.0, 2)];
    let mut s = Social::new(2);
    s.start_play(&mut pets, 0, 1, true);
    let mut turns = 0;
    let mut facing = pets[1].facing;
    for _ in 0..(CHASE_SECS.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        let p = &pets[1];
        if p.social_role() != Some(SocialRole::Flee) {
            break;
        }
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
        assert_eq!(p.body.y, 400.0);
        assert!(p.body.x >= 500.0 + 38.4 - 1e-6 && p.body.x <= 1100.0 - 38.4 + 1e-6, "{}", p.body.x);
        if p.facing != facing {
            turns += 1;
            facing = p.facing;
        }
    }
    assert!(turns >= 2, "turned around at both ends: {turns}");
}

#[test]
fn the_chaser_stops_at_the_end_of_its_surface() {
    let w = world(vec![w1()]);
    // 술래는 창 위, 도망자는 오른쪽 아래 바닥
    let mut pets = vec![pet_on(&w, friendly(), Owner::Window(1), 900.0, 1), ground(&w, friendly(), 1300.0, 2)];
    let mut s = Social::new(4);
    s.start_play(&mut pets, 0, 1, true);
    for _ in 0..(3.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        let p = &pets[0];
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
        assert!(p.body.x <= 1100.0 - 38.4 + 1e-6);
    }
    // 끝까지 갔다
    assert!(pets[0].body.x >= 1100.0 - 38.4 - 1.0, "{}", pets[0].body.x);
}

#[test]
fn personality_biases_greetings_and_chasing() {
    let (aloof, friendly_p, playful) = (Personality::Aloof, Personality::Friendly, Personality::Playful);
    assert!(play_chance(aloof, aloof) < play_chance(friendly_p, friendly_p));
    assert!(play_chance(friendly_p, friendly_p) < play_chance(playful, friendly_p));
    // 새침한 고양이도 놀이에 끼면 대개 쫓기는 쪽
    assert!((play_chance(aloof, friendly_p) - GREET_PLAY_CHANCE).abs() < 1e-9);
    let mut rng = fastrand::Rng::with_seed(1);
    let n = 2000;
    let cat_chases = (0..n).filter(|_| pick_chaser_a(&mut rng, aloof, friendly_p)).count();
    let frog_chases = (0..n).filter(|_| pick_chaser_a(&mut rng, playful, friendly_p)).count();
    assert!(cat_chases < n / 3, "{cat_chases}");
    assert!(frog_chases > n / 2, "{frog_chases}");

    // 고양이는 가끔 "흥" 하고 인사한다
    let w = world(vec![]);
    let (mut hmph, mut hello) = (0, 0);
    for seed in 0..40 {
        let mut pets = vec![ground(&w, cfg(aloof), 800.0, seed), ground(&w, friendly(), 950.0, seed + 100)];
        let mut s = Social::new(seed);
        step(&mut s, &mut pets, &w);
        match pets[0].take_speech() {
            Some(Speech::Grumpy) => {
                hmph += 1;
                run(&mut s, &mut pets, &w, 0.7);
                assert_eq!(pets[0].render().anim, Anim::Huff);
                assert!(!pets[0].render().heart);
            }
            Some(Speech::Greet) => hello += 1,
            other => panic!("{other:?}"),
        }
    }
    assert!(hmph > 5 && hello > 5, "{hmph} {hello}");
}

#[test]
fn playful_frog_plays_more_often_than_an_aloof_cat() {
    // 인사 뒤 놀이로 이어지는 횟수
    let count = |a: PetConfig, b: PetConfig| {
        let w = world(vec![]);
        let mut played = 0;
        for seed in 0..60 {
            let mut pets = vec![ground(&w, a, 800.0, seed), ground(&w, b, 950.0, seed + 1000)];
            let mut s = Social::new(seed * 31 + 7);
            for _ in 0..(3.0 / DT) as usize {
                step(&mut s, &mut pets, &w);
                if pets.iter().any(|p| p.social_role() == Some(SocialRole::Chase)) {
                    played += 1;
                    break;
                }
            }
        }
        played
    };
    let cats = count(cfg(Personality::Aloof), cfg(Personality::Aloof));
    let frogs = count(frog(), friendly());
    assert!(frogs > cats + 15, "frogs {frogs} cats {cats}");
}

#[test]
fn a_homesick_guest_is_not_drawn_into_play() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 950.0, 2)];
    pets[1].set_guest(Side::Left, 0.0);
    let mut s = Social::new(1);
    run(&mut s, &mut pets, &w, 1.0);
    assert_eq!(s.active(), 0);
    assert_eq!(pets[0].social_role(), None);

    // 놀던 중 집이 그리워지면 놀이를 그만둔다 (짝도 풀린다)
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    pets[1].set_guest(Side::Left, 0.5);
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    run(&mut s, &mut pets, &w, 0.6);
    assert_eq!(pets[1].social_role(), None);
    run(&mut s, &mut pets, &w, 0.05);
    assert_eq!(pets[0].social_role(), None);
    assert_eq!(s.active(), 0);
}

#[test]
fn grabbing_a_player_interrupts_the_game_for_both() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    run(&mut s, &mut pets, &w, 0.5);
    let (x, y) = (pets[0].body.x, pets[0].body.y - 30.0);
    pets[0].pointer_down(x, y);
    pets[0].pointer_move(x + 50.0, y - 50.0);
    assert_eq!(pets[0].state, State::Dragged);
    step(&mut s, &mut pets, &w);
    assert_eq!(pets[1].social_role(), None, "partner released");
    assert_eq!(s.active(), 0);
    // 놓아도 놀이로 돌아가지 않는다
    pets[0].pointer_up(x + 50.0, y - 50.0);
    assert_eq!(pets[0].social_role(), None);
}

#[test]
fn tickling_or_feeding_interrupts_play() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    // 직접 주기: 손에 음식
    pets[1].offer_hand(1300.0, 900.0);
    run(&mut s, &mut pets, &w, 0.1);
    assert_eq!(pets[1].social_role(), None);
    assert_eq!(pets[0].social_role(), None);
    assert_eq!(s.active(), 0);
    // 간질이기: 커서로 도망자 위를 문지른다
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    assert_eq!(s.active(), 1);
    for i in 0..60 {
        let (x, y) = (pets[1].body.x, pets[1].body.y - 40.0);
        pets[1].hover(x + if i % 2 == 0 { -30.0 } else { 30.0 }, y, true);
        step(&mut s, &mut pets, &w);
        if s.active() == 0 {
            break;
        }
    }
    assert_eq!(s.active(), 0);
    assert_eq!(pets[0].social_role(), None);
    assert_eq!(pets[1].social_role(), None);
}

#[test]
fn many_pets_pair_up_one_game_each() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, friendly(), 800.0, 1), ground(&w, friendly(), 900.0, 2), ground(&w, friendly(), 1000.0, 3)];
    let mut s = Social::new(1);
    s.tick(DT, &mut pets);
    assert_eq!(s.active(), 1);
    assert_eq!(pets.iter().filter(|p| is_greet(p)).count(), 2);

    let mut pets: Vec<Pet> = (0..4).map(|k| ground(&w, friendly(), 800.0 + 100.0 * k as f64, k as u64)).collect();
    s.tick(DT, &mut pets);
    assert_eq!(s.active(), 2);
    assert!(pets.iter().all(is_greet));

    // 11마리 — 한 마리는 짝이 없다
    let mut pets: Vec<Pet> = (0..11).map(|k| ground(&w, friendly(), 100.0 + 150.0 * k as f64, k as u64)).collect();
    let mut s = Social::new(2);
    run(&mut s, &mut pets, &w, 5.0);
    assert!((1..=5).contains(&s.active()), "{}", s.active());
    // 한 동물은 한 놀이에만: 역할이 있는 동물 수 = 짝 수 × 2
    assert_eq!(pets.iter().filter(|p| p.social_role().is_some()).count(), 2 * s.active());
}

// ---- 인사 없이 바로 술래 놀이 · 술래 바꾸기 · 놀이 뒤 쿨다운 ----

/// 몸 3개쯤 떨어진 짝은 (인사 거리 밖) 드물게 바로 술래 놀이를 시작한다: 1초에 PLAY_RATE × 더 놀고 싶은 쪽의 eagerness
#[test]
fn a_pair_in_play_range_sometimes_starts_a_chase_without_greeting_at_the_promised_rate() {
    let w = world(vec![]);
    let (n, secs, dt) = (600u64, 30.0, 0.1);
    let share = |c: PetConfig| {
        let mut started = 0;
        for seed in 0..n {
            let mut pets = vec![ground(&w, c, 800.0, seed), ground(&w, c, 800.0 + 3.0 * 128.0, seed + 1000)];
            let mut s = Social::new(seed);
            for _ in 0..(secs / dt) as usize {
                s.tick(dt, &mut pets);
                if s.active() > 0 {
                    let mut got = roles(&pets);
                    got.sort_by_key(|r| *r == Some(SocialRole::Flee));
                    assert_eq!(got, vec![Some(SocialRole::Chase), Some(SocialRole::Flee)], "straight to the chase, no greeting");
                    started += 1;
                    break;
                }
            }
        }
        started as f64 / n as f64
    };
    let expected = |rate: f64| 1.0 - (-rate * secs).exp();
    let friendly_share = share(friendly());
    assert!((friendly_share - expected(PLAY_RATE)).abs() < 0.08, "{friendly_share}");
    let playful_share = share(cfg(Personality::Playful));
    assert!((playful_share - expected(2.0 * PLAY_RATE)).abs() < 0.08, "{playful_share}");
    // 놀이 거리 밖이면 시작하지 않는다
    let mut pets = vec![ground(&w, friendly(), 300.0, 1), ground(&w, friendly(), 300.0 + 5.5 * 128.0, 2)];
    let mut s = Social::new(1);
    for _ in 0..3000 {
        s.tick(dt, &mut pets);
    }
    assert_eq!(s.active(), 0);
}

/// 코앞의 짝: 시작하자마자 잡는다. 하트 뒤 SWAP_CHANCE로 잡혔던 쪽이 술래가 되고, 한 번만 바꾼다
#[test]
fn after_a_catch_the_caught_one_sometimes_becomes_the_chaser_but_only_once() {
    let w = world(vec![]);
    let n = 500u64;
    let mut swapped = 0;
    for seed in 0..n {
        let mut pets = vec![ground(&w, friendly(), 800.0, seed), ground(&w, friendly(), 840.0, seed + 1000)];
        let mut s = Social::new(seed);
        s.start_play(&mut pets, 0, 1, true);
        step(&mut s, &mut pets, &w);
        assert!(pets.iter().all(is_greet), "caught at once: {:?}", roles(&pets));
        // 하트가 끝나는 틱까지
        for _ in 0..((CAUGHT_SECS + 0.2) / DT) as usize {
            if !pets.iter().all(is_greet) {
                break;
            }
            step(&mut s, &mut pets, &w);
        }
        if s.active() == 0 {
            assert_eq!(roles(&pets), vec![None, None]);
            continue;
        }
        swapped += 1;
        assert_eq!(roles(&pets), vec![Some(SocialRole::Flee), Some(SocialRole::Chase)], "seed {seed}");
        // 코앞이라 곧 다시 잡힌다: 이번엔 더 바꾸지 않고 끝난다
        step(&mut s, &mut pets, &w);
        assert!(pets.iter().all(is_greet), "seed {seed}: {:?}", roles(&pets));
        run(&mut s, &mut pets, &w, CAUGHT_SECS + 0.1);
        assert_eq!(s.active(), 0, "seed {seed}");
    }
    let share = swapped as f64 / n as f64;
    assert!((share - SWAP_CHANCE).abs() < 0.07, "{share}");
}

#[test]
fn a_game_cools_the_pair_down_longer_than_a_greeting() {
    let w = world(vec![]);
    let mut greeted_only = 0;
    for seed in 0..40 {
        // 술래 놀이 뒤: PLAY_COOLDOWN
        let mut pets = vec![ground(&w, friendly(), 800.0, seed), ground(&w, friendly(), 840.0, seed + 1000)];
        let keys = pair(pets[0].social_key(), pets[1].social_key());
        let mut s = Social::new(seed);
        s.start_play(&mut pets, 0, 1, true);
        // 잡힌 뒤 (술래 바꾸기가 나오면 끝날 때까지 다시 잡히게 둔다)
        for _ in 0..(60.0 / DT) as usize {
            if s.active() == 0 {
                break;
            }
            pets[0].body.x = pets[1].body.x + 40.0;
            step(&mut s, &mut pets, &w);
        }
        assert_eq!(s.active(), 0, "seed {seed}");
        let left = s.cooldown[&keys] - s.clock;
        assert!((PLAY_COOLDOWN.0 - 0.1..=PLAY_COOLDOWN.1).contains(&left), "seed {seed}: {left}");
        // 그 시간 동안은 코앞에서 쉬고 있어도 다시 어울리지 않는다
        for (p, x) in pets.iter_mut().zip([800.0, 840.0]) {
            let fresh = ground(&w, friendly(), x, 1);
            (p.body, p.standing, p.state) = (fresh.body, fresh.standing, idle());
            assert!(p.social_free(), "{:?}", p.state);
        }
        for _ in 0..((PLAY_COOLDOWN.0 - 1.0) / 0.5) as usize {
            s.tick(0.5, &mut pets);
            assert_eq!(s.active(), 0, "seed {seed}");
        }
        // 인사만 한 짝은 더 짧다 (술래 놀이로 이어지지 않은 경우)
        let mut pets = vec![ground(&w, friendly(), 800.0, seed), ground(&w, friendly(), 950.0, seed + 1000)];
        let keys = pair(pets[0].social_key(), pets[1].social_key());
        let mut s = Social::new(seed);
        run(&mut s, &mut pets, &w, GREET_SECS.1 + 0.1);
        if s.active() == 0 {
            greeted_only += 1;
            let left = s.cooldown[&keys] - s.clock;
            assert!((GREET_COOLDOWN.0 - 1.0..=GREET_COOLDOWN.1).contains(&left), "seed {seed}: {left}");
        }
    }
    assert!(greeted_only > 5, "{greeted_only}");
}

#[test]
fn the_bee_flies_after_its_friend_without_landing() {
    let w = world(vec![]);
    let mut pets = vec![bee_at(600.0, 700.0, 1), ground(&w, friendly(), 1000.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    let d0 = (pets[0].body.x - pets[1].body.x).hypot(pets[0].body.y - pets[1].body.y);
    run(&mut s, &mut pets, &w, 1.0);
    assert_eq!(pets[0].social_role(), Some(SocialRole::Chase));
    assert!(pets[0].standing.is_none());
    let d1 = (pets[0].body.x - pets[1].body.x).hypot(pets[0].body.y - pets[1].body.y);
    assert!(d1 < d0, "{d0} → {d1}");
    // 도망자가 화면 끝에서 돌아서면 잡는다 — 그동안 화면 안, 내려앉지 않는다
    let mut caught = false;
    for _ in 0..(12.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        let b = &pets[0];
        assert!(b.body.x >= 0.0 && b.body.x <= 1920.0 && b.body.y <= GROUND, "{:?}", b.body);
        caught |= b.render().anim == Anim::Heart;
    }
    assert!(caught, "the bee caught its friend");
    assert_eq!(s.active(), 0);

    // 쫓기는 꿀벌은 친구 위에서 좌우로 오간다
    let mut pets = vec![ground(&w, friendly(), 900.0, 1), bee_at(1000.0, 800.0, 2)];
    let mut s = Social::new(3);
    s.start_play(&mut pets, 0, 1, true);
    let x0 = pets[1].body.x;
    run(&mut s, &mut pets, &w, 1.0);
    assert!(pets[1].body.x > x0 + 50.0, "flies away from the chaser");
    assert!(!pets[1].is_held());
}

#[test]
fn a_frog_hops_while_playing() {
    let w = world(vec![]);
    let mut pets = vec![ground(&w, frog(), 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    let mut hopped = false;
    for _ in 0..(1.5 / DT) as usize {
        step(&mut s, &mut pets, &w);
        let r = pets[0].render();
        assert!(matches!(r.anim, Anim::Hop | Anim::Land | Anim::Idle), "{r:?}");
        hopped |= r.anim == Anim::Hop && pets[0].window_pos().1 < GROUND - 128.0 - 1.0;
        assert_eq!(pets[0].body.y, GROUND, "feet stay on the ground");
    }
    assert!(hopped);
    assert!(pets[0].body.x > 900.0);
}

/// 깡총거리는 토끼(hop_share)도 술래 놀이는 그냥 달린다 — 쫓아가 놀이가 끝난다
#[test]
fn a_rabbit_runs_while_playing_chase() {
    let w = world(vec![]);
    let rabbit = PetConfig { walk_speed: 120.0, hop_share: 0.5, ..friendly() };
    let mut pets = vec![ground(&w, rabbit, 800.0, 1), ground(&w, friendly(), 1200.0, 2)];
    let mut s = Social::new(1);
    s.start_play(&mut pets, 0, 1, true);
    for _ in 0..(1.5 / DT) as usize {
        step(&mut s, &mut pets, &w);
        if pets[0].social_role().is_none() {
            break;
        }
        assert_eq!(pets[0].render().anim, Anim::Run);
        assert_eq!(pets[0].window_pos().1, GROUND - 128.0, "no hop lift while chasing");
        assert_eq!(pets[0].body.y, GROUND);
    }
    assert!(pets[0].body.x > 900.0);
}

#[test]
fn a_flee_without_a_coordinator_ends_by_itself() {
    let w = world(vec![]);
    let mut p = ground(&w, friendly(), 800.0, 1);
    p.start_flee((700.0, GROUND), 2.0);
    for _ in 0..(3.5 / DT) as usize {
        p.update(DT, &w);
    }
    assert_eq!(p.social_role(), None);
}

/// 공룡 쿵쾅에 흔들리기
#[path = "social_stomp_tests.rs"]
mod stomp;

#[path = "social_hit_tests.rs"]
mod hit;

#[path = "social_tower_tests.rs"]
mod tower_tests;
