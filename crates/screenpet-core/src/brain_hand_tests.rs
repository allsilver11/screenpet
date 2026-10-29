//! 직접 주기 (손으로 먹이 주기) — 커서 끝의 음식을 알아채고 다가와, 입에 닿으면 받아먹는다.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, w1, world};
use super::*;
use crate::care::{Care, FeedResult, Food};
use crate::food;

const DT: f64 = 1.0 / 60.0;

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 창 1 (500..1100, 윗면 400) 위 x에 서서 오른쪽을 보는 동물
fn standing_at(x: f64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), x, idle());
    (p, w)
}

/// 입 바로 앞 (닿는 자리)
fn at_mouth(p: &Pet) -> (f64, f64) {
    let (mx, my) = p.mouth();
    (mx + 5.0, my + 5.0)
}

/// 매 틱 음식을 내밀며 돌린다 — 닿아서 받을 준비가 되면 true
fn offer_until(p: &mut Pet, w: &World, ticks: usize, at: impl Fn(&Pet) -> (f64, f64)) -> bool {
    for _ in 0..ticks {
        let (x, y) = at(p);
        if p.offer_hand(x, y) {
            return true;
        }
        p.update(DT, w);
    }
    false
}

#[test]
fn pet_notices_the_food_turns_and_waits() {
    let (mut p, w) = standing_at(800.0);
    p.facing = 1.0;
    // 왼쪽 위 멀리 (같은 바닥 근처가 아님) — 돌아보지만 걸어가지 않는다
    for _ in 0..60 {
        assert!(!p.offer_hand(100.0, 50.0));
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::WantFood { walking: false }), "{:?}", p.state);
    assert_eq!(p.facing, -1.0);
    assert_eq!(p.body.x, 800.0);
    assert_eq!(p.render().anim, Anim::Look);
}

#[test]
fn pet_walks_a_little_toward_food_on_its_surface_and_eats_from_the_hand() {
    let (mut p, w) = standing_at(800.0);
    let mut care = Care { fullness: 0.1, ..Care::default() };
    // 오른쪽 150px, 머리 높이 — 걸어와서 입에 닿는다
    let food_at = (950.0, 400.0 - 0.45 * 128.0);
    let mut walked = false;
    let mut ready = false;
    for _ in 0..(60 * 5) {
        if p.offer_hand(food_at.0, food_at.1) {
            ready = true;
            break;
        }
        p.update(DT, &w);
        walked |= p.render().anim == Anim::Walk;
    }
    assert!(ready && walked, "{:?} x {}", p.state, p.body.x);
    assert!(p.body.x > 900.0);
    // 받아먹기: 그릇과 똑같이 돌보기에 반영된다
    let result = care.feed(Food::Meal);
    assert!(p.eat_from_hand(result, Food::Meal));
    assert_eq!((care.fullness, care.stamina), (1.0, 10.0));
    assert_eq!(p.render().anim, Anim::Eat);
    assert!(p.take_prop().is_none(), "no bowl");
    let mut hearted = false;
    for _ in 0..(60 * 4) {
        p.update(DT, &w);
        assert!(p.take_prop().is_none());
        hearted |= matches!(p.state, State::Trick { kind: Trick::Heart, .. });
    }
    assert!(hearted);
    assert!(!p.is_eating() && !matches!(p.state, State::WantFood { .. }));
}

#[test]
fn snacks_from_the_hand_never_grow_even_when_full() {
    let (mut p, w) = standing_at(800.0);
    let mut care = Care::default();
    assert!(offer_until(&mut p, &w, 120, at_mouth));
    let result = care.feed(Food::Snack);
    assert_eq!(result, FeedResult::Ate { leveled_up: false });
    assert!(p.eat_from_hand(result, Food::Snack));
    assert_eq!(care.stamina, 0.0);
    assert!(matches!(p.state, State::Eating { .. }));
}

#[test]
fn full_pet_refuses_a_meal_and_the_food_stays_on_the_cursor() {
    let (mut p, w) = standing_at(800.0);
    let mut care = Care::default();
    assert!(offer_until(&mut p, &w, 120, at_mouth));
    let result = care.feed(Food::Meal);
    assert_eq!(result, FeedResult::Refused);
    assert!(p.eat_from_hand(result, Food::Meal));
    assert_eq!(p.take_speech(), Some(Speech::Full));
    assert_eq!(p.render().anim, Anim::Tilt);
    // 갸웃하는 동안은 다시 내밀어도 받지 않고, 끝나면 다시 음식을 본다
    for _ in 0..60 {
        let (x, y) = at_mouth(&p);
        assert!(!p.offer_hand(x, y));
        p.update(DT, &w);
    }
    assert!(offer_until(&mut p, &w, 60 * 3, at_mouth));
}

#[test]
fn cancelling_puts_the_pet_back_to_normal_without_eating() {
    let (mut p, w) = standing_at(800.0);
    for _ in 0..30 {
        p.offer_hand(100.0, 50.0);
        p.update(DT, &w);
    }
    p.end_hand();
    p.update(DT, &w);
    assert!(!matches!(p.state, State::WantFood { .. }), "{:?}", p.state);
    assert!(!p.is_eating());
    // 다시 내밀지 않으면 받을 수 없다
    assert!(!p.eat_from_hand(FeedResult::Ate { leveled_up: false }, Food::Meal));
}

#[test]
fn hungry_pet_reacts_eagerly_once() {
    let (mut p, w) = standing_at(800.0);
    p.set_hungry(true);
    let mut said = Vec::new();
    for _ in 0..120 {
        p.offer_hand(100.0, 50.0);
        p.update(DT, &w);
        said.extend(p.take_speech());
    }
    assert_eq!(said.iter().filter(|s| **s == Speech::Eager).count(), 1, "{said:?}");
    assert_eq!(serde_json::to_string(&Speech::Eager).unwrap(), "\"eager\"");
}

#[test]
fn no_tickling_while_holding_food() {
    let (mut p, w) = standing_at(800.0);
    for i in 0..120 {
        p.offer_hand(100.0, 50.0);
        let (x, y) = (p.body.x + if i % 2 == 0 { 0.0 } else { 60.0 }, p.body.y - 60.0);
        p.hover(x, y, true);
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Tickled { .. }), "{:?}", p.state);
    }
}

#[test]
fn busy_pets_ignore_the_food_until_free() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
    p.update(DT, &w);
    let (x, y) = (800.0, 100.0);
    assert!(!p.offer_hand(x, y), "falling");
    assert!(!p.eat_from_hand(FeedResult::Ate { leveled_up: false }, Food::Meal));
    // 내려서면 알아챈다
    assert!(offer_until(&mut p, &w, 60 * 3, at_mouth));
}

#[test]
fn bee_flies_to_the_food() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(PetConfig { flies: true, stings: true, ..cfg() }, 800.0, 250.0, 1);
    p.state = idle();
    let food_at = (1200.0, 500.0);
    assert!(offer_until(&mut p, &w, 60 * 10, |_| food_at), "{:?} {:?}", p.state, p.body);
    assert!(food::in_reach(p.mouth(), food_at, p.cfg.size));
    assert!(p.eat_from_hand(FeedResult::Ate { leveled_up: true }, Food::Favorite));
    assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }));
    assert!(p.standing.is_none(), "eats hovering");
}
