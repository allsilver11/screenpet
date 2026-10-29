//! 밥그릇·물그릇·꽃·쿠키 (phase 3 먹이 바꾸기) — 그릇으로 걸어가 먹기, 하우스 문 앞 그릇, 꿀벌 꽃.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, run, w1, world};
use super::*;
use crate::care::{FeedResult, Food};
use crate::food::{Prop, PropEvent, FOOD_TIMEOUT_SECS};

const DT: f64 = 1.0 / 60.0;
const ATE: FeedResult = FeedResult::Ate { leveled_up: false };
/// 집 문 x (기본 화면 바닥 Ground(0) 위)
const DOOR: f64 = 1500.0;
const HOME: Owner = Owner::Ground(0);

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 창 1 (500..1100, 윗면 400) 위 x에 서서 facing쪽을 보는 동물
fn standing_at(x: f64, facing: f64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), x, idle());
    p.facing = facing;
    (p, w)
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

/// 돌리면서 나온 소품 변화를 모은다
fn run_props(p: &mut Pet, w: &World, ticks: usize) -> Vec<PropEvent> {
    let mut seen = Vec::new();
    for _ in 0..ticks {
        p.update(DT, w);
        seen.extend(p.take_prop());
    }
    seen
}

fn eating(p: &Pet) -> bool {
    matches!(p.state, State::Eating { .. })
}

#[test]
fn meal_puts_a_bowl_in_front_and_the_pet_walks_there_and_eats() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.is_eating(), "on the way to the bowl counts as eating");
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 875.0, y: 400.0 }));
    assert_eq!(p.render().anim, Anim::Walk);
    assert!(!p.render().flip);
    assert!(run_until(&mut p, &w, 60 * 5, eating), "{:?}", p.state);
    assert_eq!(p.body.x, 875.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!(p.render().anim, Anim::Eat);
    // 다 먹으면 그릇이 사라진다
    let seen = run_props(&mut p, &w, 60 * 4);
    assert_eq!(seen, vec![PropEvent::Hide]);
    assert!(!p.is_eating());
}

#[test]
fn bowl_faces_the_way_the_pet_looks_and_stays_on_the_surface() {
    let (mut p, w) = standing_at(800.0, -1.0);
    p.feed_result(ATE, Food::Favorite);
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 725.0, y: 400.0 }));
    assert!(p.render().flip);
    // 창 오른쪽 끝 가까이: 동물이 설 수 있는 끝(1100 - half_w)까지만
    let (mut p, w) = standing_at(1050.0, 1.0);
    p.feed_result(ATE, Food::Meal);
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 1100.0 - 38.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 5, eating));
    assert_eq!(p.body.x, 1062.0);
}

#[test]
fn water_brings_a_water_bowl_and_ends_with_a_heart() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(ATE, Food::Water));
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::WaterBowl, x: 875.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 5, eating));
    assert!(run_until(&mut p, &w, 60 * 5, |p| matches!(p.state, State::Trick { kind: Trick::Heart, .. })), "{:?}", p.state);
}

#[test]
fn snack_is_munched_in_place_with_a_cookie_then_a_heart() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(ATE, Food::Snack));
    assert!(eating(&p));
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::Cookie, x: 800.0, y: 400.0 - 128.0 * 0.3 }));
    let mut seen = Vec::new();
    let mut hearted = false;
    for _ in 0..(60 * 5) {
        p.update(DT, &w);
        seen.extend(p.take_prop());
        hearted |= matches!(p.state, State::Trick { kind: Trick::Heart, .. });
        assert_eq!(p.body.x, 800.0);
    }
    assert_eq!(seen, vec![PropEvent::Hide]);
    assert!(hearted);
}

#[test]
fn unreachable_bowl_times_out_and_the_pet_eats_where_it_is() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.cfg.walk_speed = 1.0;
    p.feed_result(ATE, Food::Meal);
    p.update(DT, &w);
    assert!(matches!(p.take_prop(), Some(PropEvent::Show { .. })));
    assert!(run_until(&mut p, &w, (FOOD_TIMEOUT_SECS / DT) as usize + 10, eating));
    let x = p.body.x;
    assert!(x < 820.0, "{x}");
    // 그릇을 지금 자리로 옮긴다
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x, y: 400.0 }));
}

#[test]
fn feeding_again_on_the_way_keeps_one_bowl_and_the_celebration() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
    run(&mut p, &w, 10);
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.feed_result(ATE, Food::Snack));
    let seen = run_props(&mut p, &w, 0);
    assert!(seen.is_empty());
    assert!(run_until(&mut p, &w, 60 * 5, eating));
    assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }), "{:?}", p.state);
    assert!(run_until(&mut p, &w, 60 * 5, |p| matches!(p.state, State::LevelUp { .. })));
}

#[test]
fn dropped_on_another_surface_the_bowl_moves_with_the_pet() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.feed_result(ATE, Food::Meal);
    p.update(DT, &w);
    p.take_prop();
    // 들어서 화면 바닥 위로 옮겨 놓는다
    p.pointer_down(800.0, 350.0);
    p.pointer_move(300.0, 800.0);
    p.update(DT, &w);
    p.pointer_up(300.0, 800.0);
    assert!(p.is_eating(), "the meal is still waiting");
    let mut seen = Vec::new();
    for _ in 0..(60 * 10) {
        p.update(DT, &w);
        seen.extend(p.take_prop());
        if eating(&p) {
            break;
        }
    }
    assert!(eating(&p), "{:?}", p.state);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    match seen.last() {
        Some(PropEvent::Show { prop: Prop::FoodBowl, x, y }) => assert_eq!((*x, *y), (p.body.x, 1040.0)),
        other => panic!("{other:?} {seen:?}"),
    }
}

#[test]
fn going_home_on_the_way_to_the_bowl_takes_the_bowl_away() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 400.0, idle());
    p.feed_result(ATE, Food::Meal);
    p.update(DT, &w);
    assert!(matches!(p.take_prop(), Some(PropEvent::Show { .. })));
    p.go_home(HOME, DOOR);
    let seen = run_props(&mut p, &w, 2);
    assert_eq!(seen, vec![PropEvent::Hide]);
    assert_eq!(p.state, State::GoHome);
    assert!(!p.is_eating());
}

fn housed_with_door(w: &World, c: PetConfig) -> Pet {
    let mut p = Pet::new(c, 0.0, 0.0, 3);
    place(&mut p, w, HOME, DOOR, idle());
    p.enter_house();
    p.set_house_door(HOME, DOOR);
    p
}

#[test]
fn housed_pet_comes_out_to_a_bowl_at_the_door_then_goes_back_in() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, cfg());
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
    assert!(p.is_eating());
    p.update(DT, &w);
    assert!(!p.is_housed(), "comes out of the house");
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: DOOR - 75.0, y: 1040.0 }));
    assert!(p.returns_home());
    assert!(run_until(&mut p, &w, 60 * 5, eating), "{:?}", p.state);
    assert_eq!(p.body.x, DOOR - 75.0);
    let mut seen = Vec::new();
    let mut anims = Vec::new();
    for _ in 0..(60 * 15) {
        p.update(DT, &w);
        seen.extend(p.take_prop());
        anims.push(p.render().anim);
        if p.is_housed() {
            break;
        }
    }
    assert!(anims.contains(&Anim::LevelUp));
    assert_eq!(seen, vec![PropEvent::Hide]);
    assert!(p.is_housed() && !p.returns_home());
    assert_eq!(p.body.x, DOOR);
}

#[test]
fn leaving_the_house_while_eating_outside_stays_out() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, cfg());
    p.feed_result(ATE, Food::Meal);
    run(&mut p, &w, 5);
    assert!(p.returns_home());
    p.leave_house(&w, DOOR, HOME);
    assert!(!p.returns_home());
    run(&mut p, &w, 60 * 20);
    assert!(!p.is_housed() && !p.is_going_home());
}

#[test]
fn housed_snack_is_eaten_in_the_house() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, cfg());
    assert!(p.feed_result(ATE, Food::Snack));
    for _ in 0..(60 * 6) {
        p.update(DT, &w);
        assert!(p.is_housed());
        assert_eq!(p.take_prop(), None);
    }
    assert_eq!(p.state, State::Housed);
}

#[test]
fn arriving_home_remembers_the_door() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 1300.0, idle());
    p.go_home(HOME, DOOR);
    assert!(run_until(&mut p, &w, 60 * 10, |p| p.is_housed()));
    p.feed_result(ATE, Food::Water);
    p.update(DT, &w);
    assert!(!p.is_housed());
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::WaterBowl, x: DOOR - 75.0, y: 1040.0 }));
}

fn bee() -> PetConfig {
    PetConfig { flies: true, stings: true, ..cfg() }
}

#[test]
fn hovering_bee_gets_a_flower_below_and_sips_on_it() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(bee(), 800.0, 250.0, 1);
    p.state = idle();
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.is_eating());
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::Flower, x: 800.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!((p.body.x, p.body.y), (800.0, 400.0));
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    let seen = run_props(&mut p, &w, 60 * 4);
    assert_eq!(seen, vec![PropEvent::Hide]);
}

#[test]
fn perched_bee_flies_to_a_flower_in_front_and_perches_there() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(bee(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.facing = -1.0;
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::Flower, x: 725.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!((p.body.x, p.body.y), (725.0, 400.0));
    assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }));
}

#[test]
fn housed_bee_comes_out_to_a_flower_by_the_hive() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, bee());
    assert!(p.feed_result(ATE, Food::Meal));
    p.update(DT, &w);
    assert!(!p.is_housed());
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::Flower, x: DOOR - 75.0, y: 1040.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!(p.body.x, DOOR - 75.0);
    assert!(run_until(&mut p, &w, 60 * 30, |p| p.is_housed()), "{:?}", p.state);
}
