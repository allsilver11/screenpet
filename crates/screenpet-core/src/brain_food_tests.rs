//! 밥그릇·물그릇·꽃·쿠키 (phase 3 먹이 바꾸기) — 그릇으로 걸어가 먹기, 하우스 문 앞 그릇, 꿀벌 꽃.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, run, w1, world};
use super::*;
use crate::care::{FeedResult, Food};
use crate::food::{Prop, PropEvent, FOOD_TIMEOUT_SECS};
use crate::test_util::{DT, flyer, idle, run_until};

const ATE: FeedResult = FeedResult::Ate { leveled_up: false };
/// 집 문 x (기본 화면 바닥 Ground(0) 위)
const DOOR: f64 = 1500.0;
const HOME: Owner = Owner::Ground(0);

/// 창 1 (500..1100, 윗면 400) 위 x에 서서 facing쪽을 보는 동물
fn standing_at(x: f64, facing: f64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), x, idle());
    p.facing = facing;
    (p, w)
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
    assert!(matches!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, .. })));
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.feed_result(ATE, Food::Snack));
    // 그릇에 닿을 때까지 새 그릇이 나오지 않는다
    assert!(!eating(&p));
    let seen = run_props(&mut p, &w, 20);
    assert!(seen.is_empty(), "{seen:?}");
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
    assert!(p.is_eating(), "eats at home instead (fix round 1)");
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

#[test]
fn hovering_flyer_gets_a_bowl_below_and_eats_perched_on_it() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(flyer(), 800.0, 250.0, 1);
    p.state = idle();
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.is_eating());
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 800.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!((p.body.x, p.body.y), (800.0, 400.0));
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    let seen = run_props(&mut p, &w, 60 * 4);
    assert_eq!(seen, vec![PropEvent::Hide]);
}

#[test]
fn perched_flyer_flies_to_a_bowl_in_front_and_perches_there() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(flyer(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.facing = -1.0;
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Favorite));
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 725.0, y: 400.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!((p.body.x, p.body.y), (725.0, 400.0));
    assert!(matches!(p.state, State::Eating { favorite: true, level_up: true, .. }));
}

#[test]
fn housed_flyer_comes_out_to_a_bowl_by_the_house() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, flyer());
    assert!(p.feed_result(ATE, Food::Meal));
    p.update(DT, &w);
    assert!(!p.is_housed());
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: DOOR - 75.0, y: 1040.0 }));
    assert!(run_until(&mut p, &w, 60 * 20, eating), "{:?}", p.state);
    assert_eq!(p.body.x, DOOR - 75.0);
    assert!(run_until(&mut p, &w, 60 * 30, |p| p.is_housed()), "{:?}", p.state);
}

// ---- 유령: 둥실 나는 동물도 보통 밥그릇 ----

fn ghost() -> PetConfig {
    PetConfig { flies: true, walk_speed: 40.0, ..cfg() }
}

#[test]
fn hovering_ghost_gets_a_food_bowl_not_a_flower() {
    let w = world(vec![w1()]);
    for (food, prop) in [(Food::Meal, Prop::FoodBowl), (Food::Favorite, Prop::FoodBowl), (Food::Special, Prop::FoodBowl), (Food::Water, Prop::WaterBowl)] {
        let mut p = Pet::new(ghost(), 800.0, 250.0, 1);
        p.state = idle();
        assert!(p.feed_result(ATE, food));
        p.update(DT, &w);
        assert_eq!(p.take_prop(), Some(PropEvent::Show { prop, x: 800.0, y: 400.0 }), "{food:?}");
        assert!(run_until(&mut p, &w, 60 * 30, eating), "{food:?} {:?}", p.state);
        assert_eq!((p.body.x, p.body.y), (800.0, 400.0));
    }
}

#[test]
fn housed_ghost_comes_out_to_a_food_bowl_by_its_house() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, ghost());
    assert!(p.feed_result(ATE, Food::Favorite));
    p.update(DT, &w);
    assert!(!p.is_housed());
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: DOOR - 75.0, y: 1040.0 }));
    assert!(run_until(&mut p, &w, 60 * 30, eating), "{:?}", p.state);
    assert!(run_until(&mut p, &w, 60 * 40, |p| p.is_housed()), "{:?}", p.state);
}

// ---- fix round 1 ----

#[test]
fn sent_home_on_the_way_to_the_bowl_eats_at_home() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 1300.0, idle());
    p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal);
    p.update(DT, &w);
    p.take_prop();
    p.go_home(HOME, DOOR);
    assert!(p.is_eating(), "the meal is not lost");
    assert!(run_until(&mut p, &w, 60 * 10, |p| p.is_housed()));
    assert!(eating(&p), "eats in the house: {:?}", p.state);
    let mut anims = Vec::new();
    for _ in 0..(60 * 6) {
        p.update(DT, &w);
        anims.push(p.render().anim);
    }
    assert!(anims.contains(&Anim::LevelUp));
    assert!(p.is_housed() && !p.is_eating());
}

#[test]
fn cancelling_the_trip_home_goes_back_to_a_new_bowl() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 800.0, idle());
    p.feed_result(ATE, Food::Water);
    p.update(DT, &w);
    p.take_prop();
    p.go_home(HOME, DOOR);
    run(&mut p, &w, 20);
    assert_eq!(p.take_prop(), Some(PropEvent::Hide));
    p.leave_house(&w, DOOR, HOME);
    let seen = run_props(&mut p, &w, 60 * 3);
    assert!(matches!(seen.first(), Some(PropEvent::Show { prop: Prop::WaterBowl, .. })), "{seen:?}");
    assert!(run_until(&mut p, &w, 60 * 10, |p| !p.is_eating()));
}

#[test]
fn treats_given_mid_air_are_served_after_landing() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }));
    // 밥은 여전히 못 받지만, 간식은 기억해 뒀다가 내려서면 먹는다
    assert!(!p.feed_result(ATE, Food::Meal));
    assert!(p.feed_result(ATE, Food::Snack));
    assert!(matches!(p.state, State::Air { .. }));
    let mut seen = Vec::new();
    for _ in 0..(60 * 3) {
        p.update(DT, &w);
        seen.extend(p.take_prop());
        if eating(&p) {
            break;
        }
    }
    assert!(eating(&p) && p.standing.is_some(), "{:?}", p.state);
    assert!(matches!(seen.last(), Some(PropEvent::Show { prop: Prop::Cookie, .. })), "{seen:?}");
}

#[test]
fn water_given_while_dragged_is_served_after_the_drop() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.pointer_down(800.0, 350.0);
    p.pointer_move(700.0, 300.0);
    assert!(p.is_held());
    assert!(p.feed_result(ATE, Food::Water));
    p.pointer_up(700.0, 300.0);
    let seen = run_props(&mut p, &w, 60 * 8);
    assert!(seen.iter().any(|e| matches!(e, PropEvent::Show { prop: Prop::WaterBowl, .. })), "{seen:?}");
    assert_eq!(seen.last(), Some(&PropEvent::Hide));
}

#[test]
fn food_given_during_a_celebration_waits_until_it_ends() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.state = State::LevelUp { remaining: 1.0 };
    assert!(p.feed_result(ATE, Food::Snack));
    assert!(matches!(p.state, State::LevelUp { .. }), "{:?}", p.state);
    let mut anims = Vec::new();
    for _ in 0..(60 * 2) {
        p.update(DT, &w);
        anims.push(p.render().anim);
        if eating(&p) {
            break;
        }
    }
    assert!(eating(&p), "{:?}", p.state);
    let first_eat = anims.iter().position(|a| *a == Anim::Eat).unwrap();
    assert!(anims[..first_eat].iter().filter(|a| **a == Anim::LevelUp).count() >= 55, "celebration kept");
    // 하트 중 밥도 하트가 끝난 뒤
    let (mut p, w) = standing_at(800.0, 1.0);
    p.state = State::Trick { kind: Trick::Heart, remaining: 0.5 };
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
    assert!(matches!(p.state, State::Trick { kind: Trick::Heart, .. }));
    assert!(run_until(&mut p, &w, 60 * 2, |p| matches!(p.state, State::ToFood { .. })), "{:?}", p.state);
    assert!(run_until(&mut p, &w, 60 * 8, |p| matches!(p.state, State::LevelUp { .. })), "{:?}", p.state);
}

const LEVELED: FeedResult = FeedResult::Ate { leveled_up: true };

/// 성장 축하가 시작될 때까지 돌린다
fn run_to_level_up(p: &mut Pet, w: &World, ticks: usize) -> bool {
    run_until(p, w, ticks, |p| matches!(p.state, State::LevelUp { .. }))
}

#[test]
fn a_snack_that_levels_up_munches_the_cookie_then_celebrates() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(matches!(p.state, State::Eating { level_up: true, .. }), "{:?}", p.state);
    assert!(matches!(p.take_prop(), Some(PropEvent::Show { prop: Prop::Cookie, .. })));
    assert_eq!(p.render().anim, Anim::Eat);
    // 다 먹을 때까지 크기는 그대로
    assert!(p.stage_change_waits());
    assert!(run_to_level_up(&mut p, &w, 60 * 5), "{:?}", p.state);
    assert!(!p.stage_change_waits());
    assert_eq!(p.render().anim, Anim::LevelUp);
    assert_eq!(p.take_speech(), Some(Speech::LevelUp));
    // 쿠키는 치우고, 축하 뒤에 다시 돌아다닌다
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Hide));
    assert!(run_until(&mut p, &w, 60 * 10, |p| !matches!(p.state, State::LevelUp { .. })));
}

#[test]
fn a_snack_given_mid_air_holds_the_growth_until_it_is_eaten() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }));
    assert!(p.feed_result(LEVELED, Food::Snack));
    // 공중에선 아직 먹는 중이 아니지만 크기는 먹고 축하할 때 바뀐다
    assert!(!p.is_eating() && p.stage_change_waits() && !p.is_free_to_go());
    assert!(run_until(&mut p, &w, 60 * 3, eating), "{:?}", p.state);
    assert!(matches!(p.state, State::Eating { level_up: true, .. }) && p.standing.is_some());
    assert!(p.stage_change_waits());
    assert!(run_to_level_up(&mut p, &w, 60 * 5), "{:?}", p.state);
    assert!(!p.stage_change_waits());
}

#[test]
fn a_plain_queued_treat_does_not_hold_back_a_stage_change() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
    p.update(DT, &w);
    assert!(p.feed_result(ATE, Food::Snack));
    assert!(!p.stage_change_waits());
}

#[test]
fn a_snack_given_while_dragged_levels_up_after_the_drop() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.pointer_down(800.0, 350.0);
    p.pointer_move(700.0, 300.0);
    assert!(p.is_held());
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(p.stage_change_waits());
    p.pointer_up(700.0, 300.0);
    assert!(run_to_level_up(&mut p, &w, 60 * 10), "{:?}", p.state);
}

#[test]
fn a_snack_given_during_a_celebration_grows_after_it_ends() {
    for state in [State::LevelUp { remaining: 1.0 }, State::Trick { kind: Trick::Heart, remaining: 0.5 }] {
        let (mut p, w) = standing_at(800.0, 1.0);
        p.state = state;
        assert!(p.feed_result(LEVELED, Food::Snack));
        assert_eq!(p.state, state);
        assert!(!p.is_eating() && p.stage_change_waits(), "{state:?}");
        assert!(run_until(&mut p, &w, 60 * 3, eating), "{:?}", p.state);
        assert!(matches!(p.state, State::Eating { level_up: true, .. }), "{:?}", p.state);
        assert!(run_to_level_up(&mut p, &w, 60 * 5), "{:?}", p.state);
        assert!(!p.stage_change_waits());
    }
}

#[test]
fn queued_meals_outrank_treats_and_the_growth_is_kept() {
    // 밥을 기억해 둔 뒤 간식(성장): 밥그릇으로 가고 성장 축하도 남는다
    let (mut p, w) = standing_at(800.0, 1.0);
    p.state = State::Trick { kind: Trick::Heart, remaining: 0.5 };
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert_eq!(p.queued_food, Some((Food::Meal, true)));
    assert!(run_until(&mut p, &w, 60 * 2, |p| matches!(p.state, State::ToFood { .. })), "{:?}", p.state);
    assert!(run_to_level_up(&mut p, &w, 60 * 8), "{:?}", p.state);
    // 간식을 기억해 둔 뒤 밥: 역시 밥이 우선
    let (mut p, _) = standing_at(800.0, 1.0);
    p.state = State::Trick { kind: Trick::Heart, remaining: 0.5 };
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(p.feed_result(ATE, Food::Meal));
    assert_eq!(p.queued_food, Some((Food::Meal, true)));
}

#[test]
fn a_housed_snack_that_levels_up_celebrates_in_the_house() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, cfg());
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(matches!(p.state, State::Eating { level_up: true, .. }), "{:?}", p.state);
    assert!(p.stage_change_waits());
    let mut celebrated = false;
    for _ in 0..(60 * 10) {
        p.update(DT, &w);
        assert!(p.is_housed());
        assert_eq!(p.take_prop(), None);
        if matches!(p.state, State::LevelUp { .. }) {
            celebrated = true;
            assert!(!p.stage_change_waits());
            assert_eq!(p.render().anim, Anim::LevelUp);
        }
    }
    assert!(celebrated);
    assert_eq!(p.state, State::Housed);
}

#[test]
fn a_snack_given_on_the_way_home_levels_up_inside_the_house() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 1400.0, idle());
    p.go_home(HOME, DOOR);
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(p.stage_change_waits());
    assert!(run_to_level_up(&mut p, &w, 60 * 20), "{:?}", p.state);
    assert!(p.is_housed());
    assert!(run_until(&mut p, &w, 60 * 10, |p| p.state == State::Housed), "{:?}", p.state);
}

#[test]
fn the_new_stage_is_held_while_a_level_up_is_queued_or_being_eaten() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    assert!(!p.stage_change_due(false), "nothing pending");
    assert!(p.stage_change_due(true), "free: applied right away");
    // 날아가는 중에 받은 먹이가 성장을 안고 있다
    p.state = State::Air { anim: Anim::Fall };
    p.standing = None;
    assert!(p.feed_result(LEVELED, Food::Snack));
    assert!(!p.stage_change_due(true), "queued");
    // 먹는 동안에도 기다린다
    place(&mut p, &w, Owner::Ground(0), 800.0, idle());
    assert!(run_until(&mut p, &w, 60 * 5, eating), "{:?}", p.state);
    assert!(!p.stage_change_due(true), "eating");
    // 축하까지 끝나면 적용한다
    assert!(run_until(&mut p, &w, 60 * 30, |p| !p.stage_change_waits()), "{:?}", p.state);
    assert!(p.stage_change_due(true));
    // 게임 루프(src-tauri, 여기서 빌드 못 함)가 이 판단을 쓴다
    let game_loop = include_str!("../../../src-tauri/src/game_loop.rs");
    assert!(game_loop.contains("own.pet.stage_change_due(pending_stage)"));
}

#[test]
fn bowl_follows_its_window_when_the_window_moves() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.feed_result(ATE, Food::Meal);
    p.update(DT, &w);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 875.0, y: 400.0 }));
    run(&mut p, &w, 10);
    // 창이 오른쪽으로 100, 위로 50 옮겨졌다
    let moved = world(vec![super::tests::win(1, 600.0, 350.0, 1200.0, 850.0)]);
    p.update(DT, &moved);
    assert_eq!(p.take_prop(), Some(PropEvent::Show { prop: Prop::FoodBowl, x: 975.0, y: 350.0 }));
    assert!(run_until(&mut p, &moved, 60 * 5, eating));
    assert_eq!(p.body.x, 975.0);
    assert_eq!(p.take_prop(), None, "no repeat while still");
}

// ---- final polish ----

#[test]
fn moving_the_house_while_eating_outside_returns_to_the_new_door() {
    let w = world(vec![]);
    let mut p = housed_with_door(&w, cfg());
    p.feed_result(ATE, Food::Meal);
    run(&mut p, &w, 5);
    assert!(p.returns_home());
    // 먹는 사이 집을 끌어 옮겼다
    p.set_house_door(HOME, 800.0);
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.is_housed()), "{:?}", p.state);
    assert_eq!(p.body.x, 800.0);
}

#[test]
fn moving_the_house_retargets_the_trip_without_interrupting() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 5);
    place(&mut p, &w, HOME, 400.0, State::LevelUp { remaining: 1.0 });
    p.go_home(HOME, DOOR);
    let before = p.state;
    p.set_house_door(HOME, 1200.0);
    assert_eq!(p.state, before, "celebration is not cut short");
    run(&mut p, &w, 30);
    let st = p.state;
    p.set_house_door(HOME, 1250.0);
    assert_eq!(p.state, st, "the run home is not restarted");
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.is_housed()), "{:?}", p.state);
    assert_eq!(p.body.x, 1250.0);
}

#[test]
fn moving_the_house_while_a_bee_flies_home_keeps_its_flight() {
    let w = world(vec![]);
    let mut p = Pet::new(PetConfig { flies: true, ..cfg() }, 600.0, 500.0, 2);
    p.state = idle();
    p.go_home(HOME, DOOR);
    run(&mut p, &w, 20);
    assert_eq!(p.state, State::GoHome);
    p.set_house_door(HOME, 1300.0);
    assert_eq!(p.state, State::GoHome);
    assert!(run_until(&mut p, &w, 60 * 20, |p| p.is_housed()), "{:?}", p.state);
    assert_eq!(p.body.x, 1300.0);
}

#[test]
fn a_pet_that_vanishes_on_its_way_to_the_bowl_takes_the_bowl_away() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(ATE, Food::Meal));
    p.update(DT, &w);
    assert!(matches!(p.take_prop(), Some(PropEvent::Show { .. })));
    p.vanish(Side::Right);
    assert_eq!(p.take_prop(), Some(PropEvent::Hide));
    assert!(!p.is_eating());
}

#[test]
fn a_growth_on_the_way_to_the_bowl_survives_vanishing_and_is_celebrated_later() {
    let (mut p, w) = standing_at(800.0, 1.0);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: true }, Food::Meal));
    p.update(DT, &w);
    p.vanish(Side::Right);
    assert!(p.stage_change_waits(), "the size changes only when the celebration starts");
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    assert!(run_until(&mut p, &w, 60 * 10, |p| matches!(p.state, State::LevelUp { .. })), "{:?}", p.state);
    assert!(!p.stage_change_waits());
}

#[test]
fn dragging_a_pet_away_from_a_growing_meal_keeps_the_celebration() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.state = State::Eating { remaining: 1.0, favorite: false, level_up: true };
    p.pointer_down(800.0, 380.0);
    p.update(DT, &w);
    p.pointer_move(860.0, 300.0);
    assert!(matches!(p.state, State::Dragged), "{:?}", p.state);
    assert!(p.stage_change_waits());
    p.pointer_up(860.0, 300.0);
    assert!(run_until(&mut p, &w, 60 * 15, |p| matches!(p.state, State::LevelUp { .. })), "{:?}", p.state);
}

#[test]
fn losing_the_floor_during_a_growing_meal_keeps_the_celebration() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.state = State::Eating { remaining: 1.0, favorite: false, level_up: true };
    p.start_fall();
    assert!(p.stage_change_waits());
    assert!(run_until(&mut p, &w, 60 * 15, |p| matches!(p.state, State::LevelUp { .. })), "{:?}", p.state);
}

#[test]
fn a_fed_guest_eats_where_it_stands_because_the_bowl_is_only_on_the_owners_side() {
    let (mut p, w) = standing_at(800.0, 1.0);
    p.set_guest(Side::Right, 1000.0);
    assert!(p.feed_result(ATE, Food::Meal));
    assert!(eating(&p), "{:?}", p.state);
    assert!(p.food.is_none());
    p.update(DT, &w);
    assert_eq!(p.take_prop(), None);
    assert_eq!(p.body.x, 800.0);
}
