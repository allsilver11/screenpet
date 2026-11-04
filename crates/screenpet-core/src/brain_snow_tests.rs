//! 눈사람 만들기·벚꽃 올려다보기 (brain_snow.rs). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::snow_moves::{SnowPhase, PETAL_LOOK_UP_MULT, ROLL_SECS};
use super::tests::{cfg, place, sim_cfg, w1, world};
use super::*;
use crate::test_util::{DT, LoopLimit, idle};


fn on_ground(c: PetConfig, x: f64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), x, idle());
    (p, w)
}

/// 눈사람이 나올 때까지 (또는 secs초) 돌린다
fn run_until_built(p: &mut Pet, w: &World, secs: f64) -> Option<(f64, f64)> {
    let mut t = 0.0;
    while t < secs {
        p.update(DT, w);
        if let Some(at) = p.take_snowman() {
            return Some(at);
        }
        t += DT;
    }
    None
}

#[test]
fn the_pet_builds_a_snowman_on_the_ground_when_snow_has_settled() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    assert_eq!(p.render().anim, Anim::Roll, "rolls the snowball");
    let ball = p.snowball_at().expect("a snowball while rolling");
    assert_eq!(ball.1, p.body.y, "ball sits on the ground");
    let (x, y) = run_until_built(&mut p, &w, 20.0).expect("a snowman");
    assert_eq!(y, w.work_areas[0].bottom);
    assert!((x - p.body.x).abs() < p.cfg.size * 1.5, "next to the pet: {x} vs {}", p.body.x);
    assert_eq!(p.take_snowman(), None, "only once");
}

#[test]
fn rolling_then_stacking_then_a_happy_snowman_speech() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    let x0 = p.body.x;
    for _ in 0..(ROLL_SECS / DT) as usize - 2 {
        p.update(DT, &w);
        assert!(matches!(p.state, State::Snowman { phase: SnowPhase::Roll, .. }), "{:?}", p.state);
    }
    assert!((p.body.x - x0).abs() > 10.0, "pushes the ball along");
    let mut saw_stack = false;
    let mut said = false;
    for _ in 0..600 {
        p.update(DT, &w);
        saw_stack |= matches!(p.state, State::Snowman { phase: SnowPhase::Stack, .. }) && p.snowball_at().is_some();
        said |= p.take_speech() == Some(Speech::Snowman);
        if said {
            break;
        }
    }
    assert!(saw_stack);
    assert!(said, "snowman!");
    assert!(p.render().heart, "proud of it");
    assert_eq!(p.snowball_at(), None, "the ball became the snowman");
    // 곧 평소대로 돌아간다
    for _ in 0..600 {
        p.update(DT, &w);
    }
    assert!(!matches!(p.state, State::Snowman { .. }), "{:?}", p.state);
}

#[test]
fn it_happens_now_and_then_by_itself_only_while_ready() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    // 눈이 쌓이지 않았으면 절대
    for _ in 0..(600.0 / 0.05) as usize {
        p.update(0.05, &w);
        assert!(!matches!(p.state, State::Snowman { .. }), "{:?}", p.state);
        if !matches!(p.standing.map(|s| s.owner), Some(Owner::Ground(_))) {
            place(&mut p, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 1.0, look: false });
        }
    }
    let mut built = 0;
    for seed in 0..8 {
        let (mut p, w) = on_ground(sim_cfg(), 800.0);
        p.rng = fastrand::Rng::with_seed(seed);
        p.set_snow_ready(true);
        // 30분 안에는 대개 만든다
        let mut t = 0.0;
        while t < 1800.0 {
            p.update(0.05, &w);
            if p.take_snowman().is_some() {
                built += 1;
                break;
            }
            t += 0.05;
        }
    }
    assert!(built >= 6, "{built}/8");
}

#[test]
fn only_a_walking_own_pet_on_the_ground_builds() {
    // 나는 동물
    let (mut p, w) = on_ground(PetConfig { flies: true, ..sim_cfg() }, 800.0);
    p.set_snow_ready(true);
    assert!(!p.can_build_snowman(&w));
    // 손님
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.set_guest(Side::Left, 1000.0);
    assert!(!p.can_build_snowman(&w));
    // 창 윗면
    let w = world(vec![w1()]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    p.set_snow_ready(true);
    assert!(!p.can_build_snowman(&w));
    // 준비 안 됨 / 됨
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    assert!(!p.can_build_snowman(&w));
    p.set_snow_ready(true);
    assert!(p.can_build_snowman(&w));
    // 조작 모드 중엔 저절로 하지 않는다
    p.set_control(true);
    assert!(!p.can_build_snowman(&w));
}

#[test]
fn the_snowball_stays_on_the_ground_near_a_wall() {
    let (mut p, w) = on_ground(sim_cfg(), 60.0);
    p.set_snow_ready(true);
    p.start_snowman(-1.0);
    let b = w.work_areas[0];
    let mut limit = LoopLimit::new();
    while matches!(p.state, State::Snowman { phase: SnowPhase::Roll, .. }) {
        limit.step();
        p.update(DT, &w);
        let ball = p.snowball_at().unwrap();
        assert!(ball.0 >= b.left && ball.0 <= b.right, "{ball:?}");
        assert!(p.body.x - p.cfg.half_w >= b.left - 1.0, "{}", p.body.x);
    }
    let (x, _) = run_until_built(&mut p, &w, 10.0).unwrap();
    assert!(x >= b.left && x <= b.right);
}

#[test]
fn picking_the_pet_up_drops_the_ball_and_no_snowman_appears() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    p.update(DT, &w);
    assert!(p.snowball_at().is_some());
    p.pointer_down(p.body.x, p.body.y - 40.0);
    p.pointer_move(p.body.x + 100.0, p.body.y - 300.0);
    p.update(DT, &w);
    assert!(p.is_held());
    assert_eq!(p.snowball_at(), None);
    assert_eq!(p.take_snowman(), None);
    // 내려놓고 기다려도 눈사람은 나오지 않는다
    p.pointer_up(p.body.x, p.body.y - 40.0);
    assert_eq!(run_until_built(&mut p, &w, 30.0), None);
    assert_eq!(p.snowball_at(), None);
}

#[test]
fn snow_going_away_mid_build_ends_it_without_a_snowman() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    p.update(DT, &w);
    p.set_snow_ready(false);
    p.update(DT, &w);
    assert!(!matches!(p.state, State::Snowman { .. }), "{:?}", p.state);
    assert_eq!(p.snowball_at(), None);
    assert_eq!(run_until_built(&mut p, &w, 5.0), None);
}

#[test]
fn a_giraffe_walks_instead_of_rolling() {
    let (mut p, w) = on_ground(PetConfig { special: Some(Special::NeckStretch), ..sim_cfg() }, 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    p.update(DT, &w);
    assert_eq!(p.render().anim, Anim::Walk);
}

#[test]
fn petals_make_a_pet_look_up_more_often() {
    let looker = PetConfig { looks_up: true, ..cfg() };
    let count = |petals: bool| {
        let mut p = Pet::new(looker, 0.0, 0.0, 7);
        p.set_petals(petals);
        (0..5000).filter(|_| p.roll_look_up()).count()
    };
    let (calm, blossom) = (count(false), count(true));
    assert!(blossom as f64 > calm as f64 * (PETAL_LOOK_UP_MULT - 1.0), "{calm} -> {blossom}");
    // lookup이 없는 동물은 그대로
    let mut p = Pet::new(cfg(), 0.0, 0.0, 7);
    p.set_petals(true);
    assert!(!(0..1000).any(|_| p.roll_look_up()));
}

#[test]
fn switching_species_ends_the_build_and_the_ball_on_the_spot() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_snow_ready(true);
    p.start_snowman(1.0);
    p.update(DT, &w);
    assert!(p.snowball_at().is_some());
    p.end_snowman();
    assert!(matches!(p.state, State::Idle { .. }), "{:?}", p.state);
    assert_eq!(p.snowball_at(), None, "the app removes the ball window");
    assert!(p.standing.is_some(), "still on the ground");
    // 만들고 있지 않으면 아무 일도 없다
    p.state = State::Sleep { remaining: 10.0 };
    p.end_snowman();
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
}

#[test]
fn a_flyer_left_in_the_snowman_state_recovers_by_itself() {
    // 눌러 둔 동안 종이 바뀌어 눌림이 끝나면 되돌아오는 길도 같다 — 나는 동물은 update_snowman을 돌지 않는다
    for phase in [SnowPhase::Roll, SnowPhase::Stack, SnowPhase::Admire] {
        let (mut p, w) = on_ground(PetConfig { flies: true, ..sim_cfg() }, 800.0);
        p.state = State::Snowman { phase, remaining: 100.0, dir: 1.0 };
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Snowman { .. }), "{phase:?}: {:?}", p.state);
        assert_eq!(p.snowball_at(), None);
    }
}
