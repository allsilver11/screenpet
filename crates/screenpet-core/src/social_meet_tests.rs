//! 마주 오던 두 동물이 만날 때 (social.rs + brain_meet.rs)

use super::*;
use crate::brain::{Anim, Pet, PetConfig, Speech, State};
use crate::behavior::Move;
use crate::brain::meet_moves::MeetPhase;
use crate::geom::{Rect, WindowInfo};
use crate::world::World;
use crate::test_util::DT;

const GROUND: f64 = 1040.0;

fn cfg() -> PetConfig {
    PetConfig { half_w: 38.4, ..crate::test_util::pet_cfg() }
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

fn walker(w: &World, c: PetConfig, owner: Owner, x: f64, facing: f64, seed: u64) -> Pet {
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(crate::brain::Standing { owner, rel_x: x - ax });
    p.state = State::Move { kind: Move::Walk, remaining: 60.0 };
    p.facing = facing;
    p
}

/// 서로 마주 보고 걷는 둘 (한 틱 걸어 걷는 조각을 안다)
fn head_on_pair(w: &World, gap: f64, seed: u64) -> Vec<Pet> {
    let mut pets = vec![walker(w, cfg(), Owner::Ground(0), 800.0, 1.0, seed), walker(w, cfg(), Owner::Ground(0), 800.0 + gap, -1.0, seed + 1000)];
    for p in &mut pets {
        p.update(DT, w);
    }
    pets
}

fn always(kind: Option<MeetKind>, seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 1.0;
    s.meet_only = kind;
    s
}

fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
    }
}

fn run(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) {
    for _ in 0..(secs / DT) as usize {
        step(s, pets, w);
    }
}

fn phase(p: &Pet) -> Option<MeetPhase> {
    match p.state {
        State::Meet { phase, .. } => Some(phase),
        _ => None,
    }
}

/// 뛰는 쪽, 밟히는 쪽
fn roles(pets: &[Pet]) -> (usize, usize) {
    if matches!(phase(&pets[0]), Some(MeetPhase::Brace)) {
        (1, 0)
    } else {
        (0, 1)
    }
}

#[test]
fn a_stomp_bumps_the_head_lands_past_and_both_carry_on() {
    let w = floor();
    for seed in 0..12 {
        let mut pets = head_on_pair(&w, 200.0, seed);
        let mut s = always(Some(MeetKind::Stomp), seed);
        step(&mut s, &mut pets, &w);
        let (j, t) = roles(&pets);
        assert_eq!(phase(&pets[j]), Some(MeetPhase::Crouch));
        assert_eq!(phase(&pets[t]), Some(MeetPhase::Brace));
        assert_eq!(pets[j].render().anim, Anim::Land);
        assert_eq!(pets[t].render().anim, Anim::Look);
        let (jx, tx, dir) = (pets[j].body.x, pets[t].body.x, pets[j].facing);
        let head = GROUND - 0.8 * 128.0;
        let (mut top, mut touched, mut oof, mut boing) = (f64::MAX, false, false, false);
        for _ in 0..(60 * 3) {
            step(&mut s, &mut pets, &w);
            top = top.min(pets[j].body.y);
            // 머리에 닿는 순간: 발이 친구 머리 꼭대기, 친구는 납작
            if (pets[j].body.x - tx).abs() < 1.0 && (pets[j].body.y - head).abs() < 1.0 {
                touched = true;
            }
            oof |= pets[t].take_speech() == Some(Speech::Oof);
            boing |= pets[j].take_speech() == Some(Speech::Boing);
            if matches!(pets[j].state, State::Move { .. }) && matches!(pets[t].state, State::Move { .. }) {
                break;
            }
        }
        assert!(touched, "{seed}");
        assert!(oof && boing, "{seed}: {oof} {boing}");
        assert!(top < head, "it bounces above the head: {top}");
        // 둘 다 서로의 건너편에서, 가던 쪽을 보고 다시 걷는다
        assert_eq!(pets[j].body.y, GROUND);
        assert!(pets[j].standing.is_some() && pets[t].standing.is_some());
        assert!((pets[j].body.x - pets[t].body.x) * dir > 0.0, "{seed}");
        assert_eq!(pets[j].facing, dir);
        assert_eq!(pets[t].facing, -dir);
        assert!((pets[j].body.x - jx) * dir > 200.0);
        assert!(matches!(pets[j].state, State::Move { .. }) && matches!(pets[t].state, State::Move { .. }));
    }
}

fn top_of_bounce(kind: MeetKind) -> f64 {
    let w = floor();
    let mut pets = head_on_pair(&w, 200.0, 5);
    let mut s = always(Some(kind), 5);
    step(&mut s, &mut pets, &w);
    let (j, _) = roles(&pets);
    let mut top = f64::MAX;
    for _ in 0..(60 * 3) {
        step(&mut s, &mut pets, &w);
        top = top.min(pets[j].body.y);
    }
    assert_eq!(pets[j].body.y, GROUND);
    top
}

#[test]
fn a_high_bounce_flies_much_higher_than_a_stomp() {
    let (low, high) = (top_of_bounce(MeetKind::Stomp), top_of_bounce(MeetKind::HighBounce));
    let head = GROUND - 0.8 * 128.0;
    assert!(head - high > (head - low) * 3.0, "{low} {high}");
}

#[test]
fn passing_means_both_hop_and_never_touch() {
    let w = floor();
    for seed in 0..12 {
        let mut pets = head_on_pair(&w, 200.0, seed);
        let mut s = always(Some(MeetKind::Pass), seed);
        step(&mut s, &mut pets, &w);
        assert_eq!(phase(&pets[0]), Some(MeetPhase::Crouch));
        assert_eq!(phase(&pets[1]), Some(MeetPhase::Crouch));
        let (a0, b0) = (pets[0].body.x, pets[1].body.x);
        let mut air = [false; 2];
        let mut crossed = false;
        for _ in 0..(60 * 3) {
            step(&mut s, &mut pets, &w);
            let (a, b) = (&pets[0], &pets[1]);
            air[0] |= a.standing.is_none();
            air[1] |= b.standing.is_none();
            if (a.body.x - b.body.x).abs() < a.cfg.half_w + b.cfg.half_w {
                crossed = true;
                // 몸이 겹치는 동안 한쪽 발은 다른 쪽 머리 꼭대기 위에 있다
                let gap = (a.body.y - (b.body.y - 0.8 * 128.0)).min(b.body.y - (a.body.y - 0.8 * 128.0));
                let a_over = a.body.y <= b.body.y - 0.8 * 128.0;
                let b_over = b.body.y <= a.body.y - 0.8 * 128.0;
                assert!(a_over || b_over, "{seed}: touching at {:?} {:?} ({gap})", a.body, b.body);
            }
        }
        assert!(air[0] && air[1], "{seed}: both hop");
        assert!(crossed);
        assert!(pets[0].body.x > b0 && pets[1].body.x < a0, "{seed}: they swapped sides");
        for p in &pets {
            assert_eq!(p.body.y, GROUND);
            assert!(p.standing.is_some());
        }
    }
}

#[test]
fn they_only_meet_walking_at_each_other_on_the_same_stretch() {
    let window = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    let w = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let on = |x, facing, seed| {
        let mut p = walker(&w, cfg(), Owner::Ground(0), x, facing, seed);
        p.update(DT, &w);
        p
    };
    let mut cases = vec![
        ("back to back", vec![on(1200.0, -1.0, 1), on(1400.0, 1.0, 2)]),
        ("same way", vec![on(1200.0, 1.0, 1), on(1400.0, 1.0, 2)]),
        ("too far", vec![on(1200.0, 1.0, 1), on(1600.0, -1.0, 2)]),
    ];
    let mut up = walker(&w, cfg(), Owner::Window(1), 850.0, -1.0, 2);
    up.update(DT, &w);
    cases.push(("other surface", vec![on(800.0, 1.0, 1), up]));
    let mut idle = on(1400.0, -1.0, 2);
    idle.state = State::Idle { remaining: 9.0, look: false };
    cases.push(("one is resting", vec![on(1200.0, 1.0, 1), idle]));
    let mut rolling = on(1400.0, -1.0, 2);
    rolling.state = State::Move { kind: Move::Roll, remaining: 9.0 };
    cases.push(("one is rolling", vec![on(1200.0, 1.0, 1), rolling]));
    let mut flier = walker(&w, PetConfig { flies: true, ..cfg() }, Owner::Ground(0), 1400.0, -1.0, 2);
    flier.update(DT, &w);
    cases.push(("flier", vec![on(1200.0, 1.0, 1), flier]));
    for (name, mut pets) in cases {
        let mut s = always(None, 1);
        s.tick(DT, &mut pets);
        assert!(pets.iter().all(|p| phase(p).is_none()), "{name}");
    }
}

/// 바쁜 일은 그대로 둔 채, 겉모습만 x에서 왼쪽으로 걷는 동물로 돌려놓는다 (만날 수 있는 자리·자세)
fn back_to_walking_at(p: &mut Pet, x: f64, w: &World) {
    let (ax, _) = w.anchor(Owner::Ground(0)).unwrap();
    p.standing = Some(crate::brain::Standing { owner: Owner::Ground(0), rel_x: x - ax });
    p.body.x = x;
    p.state = State::Move { kind: Move::Walk, remaining: 60.0 };
    p.facing = -1.0;
}

#[test]
fn control_food_home_focus_and_homesick_pets_do_not_take_part() {
    let w = floor();
    let setups: Vec<(&str, Box<dyn Fn(&mut Pet)>)> = vec![
        ("control", Box::new(|p| p.set_control(true))),
        ("home", Box::new(|p| p.go_home(Owner::Ground(0), 1800.0))),
        ("focus", Box::new(|p| p.set_focus(true))),
        ("user away", Box::new(|p| p.set_user_away(true))),
        ("homesick", Box::new(|p| p.set_guest(crate::visit::Side::Right, 0.0))),
        ("fed", Box::new(|p| { p.feed_result(crate::care::FeedResult::Ate { leveled_up: false }, crate::care::Food::Meal); })),
    ];
    // 비교: 아무 일 없는 짝은 같은 자리·같은 순서로 만난다
    let mut pets = head_on_pair(&w, 200.0, 3);
    back_to_walking_at(&mut pets[1], 1000.0, &w);
    let mut s = always(None, 3);
    s.tick(DT, &mut pets);
    assert!(pets.iter().any(|p| phase(p).is_some()), "the control pair meets");
    for (name, setup) in setups {
        let mut pets = head_on_pair(&w, 200.0, 3);
        setup(&mut pets[1]);
        // 손님의 집 생각은 시간이 지나야 든다
        if name == "homesick" {
            pets[1].update(1.0, &w);
        }
        // 여전히 바쁜 채로 다시 걷는 자세로 돌려놓는다 (틱을 돌리면 걷는 조각을 잊어 바쁜 판정까지 가지 못한다)
        back_to_walking_at(&mut pets[1], 1000.0, &w);
        assert!(pets[1].walks_on_surface() && pets[1].walk_span().is_some(), "{name}");
        let mut s = always(None, 3);
        s.tick(DT, &mut pets);
        assert!(pets.iter().all(|p| phase(p).is_none()), "{name}");
    }
}

#[test]
fn a_bounce_back_bumps_the_head_and_hops_back_the_way_it_came() {
    let w = floor();
    for seed in 0..8 {
        let mut pets = head_on_pair(&w, 200.0, seed);
        let mut s = always(Some(MeetKind::BounceBack), seed);
        step(&mut s, &mut pets, &w);
        let (j, t) = roles(&pets);
        let (tx, dir) = (pets[t].body.x, pets[j].facing);
        let (mut squashed, mut boing) = (false, false);
        for _ in 0..(60 * 3) {
            step(&mut s, &mut pets, &w);
            squashed |= phase(&pets[t]) == Some(MeetPhase::Squash);
            boing |= pets[j].take_speech() == Some(Speech::Boing);
            if phase(&pets[j]).is_none() && phase(&pets[t]).is_none() {
                break;
            }
        }
        assert!(squashed && boing, "{seed}: {squashed} {boing}");
        // 온 쪽에 내려서서 돌아서 간다 — 친구 몸 안이 아니다
        assert!(pets[j].standing.is_some());
        assert!((pets[j].body.x - tx) * dir < -(pets[j].cfg.half_w + pets[t].cfg.half_w), "{seed}");
        assert_eq!(pets[j].facing, -dir, "{seed}");
        assert!(matches!(pets[j].state, State::Move { .. }));
    }
}

#[test]
fn a_walker_stomps_a_friend_standing_still_and_the_friend_goes_flat() {
    let w = floor();
    for seed in 0..8 {
        let mut pets = head_on_pair(&w, 200.0, seed);
        pets[1].state = State::Idle { remaining: 30.0, look: false };
        let mut s = always(Some(MeetKind::Stomp), seed);
        s.one_way_chance = 1.0;
        step(&mut s, &mut pets, &w);
        // 다가간 쪽이 뛰고, 서 있던 쪽은 기다린다
        assert_eq!(phase(&pets[0]), Some(MeetPhase::Crouch), "{seed}");
        assert_eq!(phase(&pets[1]), Some(MeetPhase::Brace), "{seed}");
        let mut squashed = false;
        for _ in 0..(60 * 3) {
            step(&mut s, &mut pets, &w);
            squashed |= phase(&pets[1]) == Some(MeetPhase::Squash);
        }
        assert!(squashed);
        assert!(pets[0].body.x > pets[1].body.x + pets[0].cfg.half_w, "landed past: {} {}", pets[0].body.x, pets[1].body.x);
    }
}

#[test]
fn a_perched_flyer_can_be_stomped_on_but_never_jumps() {
    let w = floor();
    let bird = PetConfig { flies: true, ..cfg() };
    let mut pets = head_on_pair(&w, 200.0, 4);
    let x = pets[1].body.x;
    pets[1] = walker(&w, bird, Owner::Ground(0), x, -1.0, 9);
    pets[1].state = State::Idle { remaining: 30.0, look: false };
    let mut s = always(Some(MeetKind::Stomp), 4);
    s.one_way_chance = 1.0;
    step(&mut s, &mut pets, &w);
    assert_eq!(phase(&pets[1]), Some(MeetPhase::Brace));
    let mut squashed = false;
    for _ in 0..(60 * 3) {
        step(&mut s, &mut pets, &w);
        squashed |= phase(&pets[1]) == Some(MeetPhase::Squash);
    }
    assert!(squashed, "the bird goes flat");
    assert!(phase(&pets[1]).is_none() && pets[1].standing.is_some(), "{:?}", pets[1].state);
    // 나는 동물은 걷지 않으니 다가가 밟지 않는다
    let mut pets = vec![walker(&w, bird, Owner::Ground(0), 800.0, 1.0, 3), walker(&w, cfg(), Owner::Ground(0), 900.0, 1.0, 4)];
    pets[1].state = State::Idle { remaining: 30.0, look: false };
    let mut s = always(Some(MeetKind::Stomp), 3);
    s.one_way_chance = 1.0;
    s.tick(DT, &mut pets);
    assert!(pets.iter().all(|p| phase(p).is_none()));
}

#[test]
fn it_is_rolled_once_per_meeting_and_then_cools_down() {
    let w = floor();
    // 안 하기로 했으면 이번 마주침엔 다시 묻지 않고 평소대로 인사한다
    let mut pets = head_on_pair(&w, 200.0, 2);
    let mut s = always(None, 2);
    s.meet_chance = 0.0;
    s.tick(DT, &mut pets);
    assert!(pets.iter().all(|p| phase(p).is_none()));
    assert_eq!(s.active(), 1, "the usual greeting");
    assert_eq!(s.meet_cooldown.len(), 1);
    // 했으면 긴 쿨다운, 내려서도 곧장 또 하거나 인사하지 않는다
    let mut pets = head_on_pair(&w, 200.0, 2);
    let mut s = always(None, 2);
    step(&mut s, &mut pets, &w);
    assert!(pets.iter().any(|p| phase(p).is_some()));
    run(&mut s, &mut pets, &w, 3.0);
    assert_eq!(s.active(), 0);
    let until = s.meet_cooldown.values().next().copied().unwrap();
    assert!(until - s.clock > MEET_COOLDOWN.0 - 5.0);
    // 다시 마주 보게 놓아도 아무 일 없다
    pets = vec![walker(&w, cfg(), Owner::Ground(0), 800.0, 1.0, 40), walker(&w, cfg(), Owner::Ground(0), 1000.0, -1.0, 41)];
    let keys = (pets[0].social_key(), pets[1].social_key());
    let wait = s.meet_cooldown.values().next().copied().unwrap();
    s.meet_cooldown.insert(pair(keys.0, keys.1), wait);
    s.cooldown.insert(pair(keys.0, keys.1), wait);
    for p in &mut pets {
        p.update(DT, &w);
    }
    assert!(pets.iter().all(|p| p.walks_on_surface() && p.walk_span().is_some()));
    s.tick(DT, &mut pets);
    assert!(pets.iter().all(|p| phase(p).is_none()));
    assert_eq!(s.active(), 0);
}

#[test]
fn it_happens_about_as_often_as_promised_and_every_kind_shows_up() {
    let w = floor();
    let (mut met, n) = (0, 400);
    let mut kinds = [0; 5];
    let all = [MeetKind::Stomp, MeetKind::HighBounce, MeetKind::Pass, MeetKind::Climb, MeetKind::BounceBack];
    for seed in 0..n {
        let mut pets = head_on_pair(&w, 200.0, seed);
        let mut s = Social::new(seed);
        s.tick(DT, &mut pets);
        if pets.iter().any(|p| phase(p).is_some()) {
            met += 1;
            let kind = pets.iter().find_map(Pet::meet_kind).expect("a jumper has a plan");
            kinds[all.iter().position(|k| *k == kind).unwrap()] += 1;
        } else {
            assert_eq!(s.active(), 1, "otherwise they greet as before");
        }
    }
    let share = met as f64 / n as f64;
    assert!((share - MEET_CHANCE).abs() < 0.08, "{share}");
    assert!(kinds.iter().all(|&k| k > 30), "every kind shows up: {kinds:?}");
    assert!(kinds[1..].iter().all(|&k| k < kinds[0]), "the stomp is the usual meet: {kinds:?}");
}

#[test]
fn nobody_lands_off_the_window_they_were_walking_on() {
    // 창 윗면 500..1100에서 끝 쪽으로 몰린 짝
    let window = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    let w = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let mut jumped = 0;
    for seed in 0..40 {
        for kind in [MeetKind::Stomp, MeetKind::HighBounce, MeetKind::Pass] {
            let mut pets = vec![walker(&w, cfg(), Owner::Window(1), 900.0, 1.0, seed), walker(&w, cfg(), Owner::Window(1), 1050.0, -1.0, seed + 500)];
            for p in &mut pets {
                p.update(DT, &w);
            }
            let mut s = always(Some(kind), seed);
            s.tick(DT, &mut pets);
            jumped += pets.iter().any(|p| phase(p).is_some()) as usize;
            // 만남이 끝나는 순간 그 창 위에 서 있다 (그 뒤에 걷다가 창 끝에서 내리는 것은 평소 걸음이다)
            let mut was = [phase(&pets[0]).is_some(), phase(&pets[1]).is_some()];
            for _ in 0..(60 * 3) {
                step(&mut s, &mut pets, &w);
                for (i, p) in pets.iter().enumerate() {
                    let now = phase(p).is_some();
                    if was[i] && !now {
                        assert_eq!(p.standing.map(|st| st.owner), Some(Owner::Window(1)), "{seed} {kind:?}: {:?} {:?}", p.state, p.body);
                        assert_eq!(p.body.y, 400.0);
                        assert!((500.0 + p.cfg.half_w..=1100.0 - p.cfg.half_w).contains(&p.body.x), "{seed} {kind:?}: {}", p.body.x);
                    }
                    was[i] = now;
                }
            }
        }
    }
    assert!(jumped > 0, "the stomp from the roomy side still works");
    // 어느 쪽으로도 내릴 자리가 없으면 시작하지 않는다
    let narrow = World::new(vec![WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 760.0, 900.0) }], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let mut pets = vec![walker(&narrow, cfg(), Owner::Window(1), 560.0, 1.0, 1), walker(&narrow, cfg(), Owner::Window(1), 700.0, -1.0, 2)];
    for p in &mut pets {
        p.update(DT, &narrow);
    }
    let mut s = always(Some(MeetKind::Stomp), 1);
    s.tick(DT, &mut pets);
    assert!(pets.iter().all(|p| phase(p).is_none()));
}

#[test]
fn a_window_closing_mid_jump_still_ends_on_a_real_surface() {
    let window = WindowInfo { id: 1, rect: Rect::new(300.0, 400.0, 1500.0, 900.0) };
    let w = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let gone = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let mut pets = vec![walker(&w, cfg(), Owner::Window(1), 800.0, 1.0, 1), walker(&w, cfg(), Owner::Window(1), 1000.0, -1.0, 2)];
    for p in &mut pets {
        p.update(DT, &w);
    }
    let mut s = always(Some(MeetKind::HighBounce), 1);
    step(&mut s, &mut pets, &w);
    for _ in 0..30 {
        step(&mut s, &mut pets, &w);
    }
    run(&mut s, &mut pets, &gone, 4.0);
    for p in &pets {
        assert_eq!(p.body.y, GROUND, "{:?}", p.state);
        assert_eq!(p.standing.map(|st| st.owner), Some(Owner::Ground(0)));
        assert!(!p.is_meeting());
    }
}

#[test]
fn grabbing_the_jumper_frees_the_one_waiting_and_grabbing_the_target_ends_it_safely() {
    let w = floor();
    let mut pets = head_on_pair(&w, 200.0, 8);
    let mut s = always(Some(MeetKind::Stomp), 8);
    step(&mut s, &mut pets, &w);
    let (j, t) = roles(&pets);
    run(&mut s, &mut pets, &w, 0.1);
    let (x, y) = (pets[j].body.x, pets[j].body.y);
    pets[j].pointer_down(x, y - 50.0);
    assert!(pets[j].is_held());
    step(&mut s, &mut pets, &w);
    step(&mut s, &mut pets, &w);
    assert!(matches!(pets[t].state, State::Move { .. }), "{:?}", pets[t].state);
    assert_eq!(pets[t].take_speech(), None);

    let mut pets = head_on_pair(&w, 200.0, 8);
    let mut s = always(Some(MeetKind::Stomp), 8);
    step(&mut s, &mut pets, &w);
    let (j, t) = roles(&pets);
    let (x, y) = (pets[t].body.x, pets[t].body.y);
    pets[t].pointer_down(x, y - 50.0);
    run(&mut s, &mut pets, &w, 3.0);
    assert_eq!(pets[j].body.y, GROUND, "the jumper still lands");
    assert!(!pets[j].is_meeting());
    assert!(pets[t].is_held(), "still in hand");
}

#[test]
fn a_jumper_whose_target_left_does_not_keep_the_head_bump_for_later() {
    let w = floor();
    let mut pets = head_on_pair(&w, 200.0, 8);
    let mut s = always(Some(MeetKind::Stomp), 8);
    step(&mut s, &mut pets, &w);
    let (j, t) = roles(&pets);
    pets.remove(t);
    let j = if t < j { j - 1 } else { j };
    run(&mut s, &mut pets, &w, 3.0);
    assert!(!pets[j].is_meeting());
    assert!(!pets[j].take_head_bump(), "a stale bump would end a later climb early");
}

#[test]
fn big_and_little_pets_meet_too() {
    let w = floor();
    let big = PetConfig { size: 211.0, half_w: 63.3, ..cfg() };
    let little = PetConfig { size: 64.0, half_w: 19.2, ..cfg() };
    // 올라서기(Climb)는 지나가지 않아서 social_tower_tests가 본다
    for seed in 0..30 {
        let mut pets = vec![walker(&w, big, Owner::Ground(0), 800.0, 1.0, seed), walker(&w, little, Owner::Ground(0), 950.0, -1.0, seed + 50)];
        for p in &mut pets {
            p.update(DT, &w);
        }
        let kind = [MeetKind::Stomp, MeetKind::HighBounce, MeetKind::Pass][seed as usize % 3];
        let mut s = always(Some(kind), seed);
        step(&mut s, &mut pets, &w);
        assert!(pets.iter().any(|p| phase(p).is_some()), "{seed}");
        run(&mut s, &mut pets, &w, 4.0);
        assert!(pets[0].body.x > pets[1].body.x, "{seed}: swapped");
        for p in &pets {
            assert_eq!(p.body.y, GROUND);
            assert!(!p.is_meeting());
        }
    }
}
