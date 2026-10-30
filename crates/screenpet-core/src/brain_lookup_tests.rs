//! 올려다보기 (Anim::LookUp, pet.json lookup) — 머리 위에 든 음식, 머리 위에 머무는 커서, 가끔 쉬다가·걷다가 문득.
//! lookup이 없는 동물은 똑같은 때에 그냥 쳐다본다 (예전 그대로).
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, sim_cfg, w1, win, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn looker() -> PetConfig {
    PetConfig { looks_up: true, ..cfg() }
}

/// 창 1 (500..1100, 윗면 400) 위 x에 서서 오른쪽을 보는 동물
fn standing_at(c: PetConfig, x: f64) -> (Pet, World) {
    let w = world(vec![w1()]);
    let mut p = Pet::new(c, 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), x, State::Idle { remaining: 100.0, look: false });
    p.facing = 1.0;
    (p, w)
}

/// 머리 꼭대기(발 400 − 0.8 × 128)보다 몸 0.5개 위, 옆으로 dx
fn above_head(x: f64, dx: f64) -> (f64, f64) {
    (x + dx, 400.0 - 0.8 * 128.0 - 0.5 * 128.0)
}

#[test]
fn food_held_above_the_head_makes_the_pet_stop_and_look_up() {
    let (mut p, w) = standing_at(looker(), 800.0);
    let food = above_head(800.0, 200.0);
    for _ in 0..60 {
        assert!(!p.offer_hand(food.0, food.1));
        p.update(DT, &w);
        assert_ne!(p.render().anim, Anim::Walk, "{:?}", p.state);
    }
    assert!(matches!(p.state, State::WantFood { walking: false }), "{:?}", p.state);
    assert_eq!(p.body.x, 800.0, "no walking toward food held overhead");
    assert_eq!(p.render().anim, Anim::LookUp);
    assert!(!p.render().anim.is_front());
    // 음식을 입 높이로 내리면 다시 평소대로 걸어온다
    let low = (950.0, 400.0 - 0.45 * 128.0);
    let mut walked = false;
    for _ in 0..60 {
        p.offer_hand(low.0, low.1);
        p.update(DT, &w);
        walked |= p.render().anim == Anim::Walk;
        assert_ne!(p.render().anim, Anim::LookUp);
    }
    assert!(walked);
    // 거두면 음식 보기도 끝
    p.end_hand();
    p.update(DT, &w);
    assert!(!matches!(p.state, State::WantFood { .. }), "{:?}", p.state);
}

#[test]
fn food_far_to_the_side_is_not_overhead() {
    let (mut p, w) = standing_at(looker(), 800.0);
    // 몸 3개보다 멀다 (높이는 머리 위) — 머리 위가 아니니 쳐다보기 (걸어갈 만큼 가깝지도 않다: 위로 몸 2.5개 넘게)
    let food = (800.0 - 3.5 * 128.0, 400.0 - 2.8 * 128.0);
    for _ in 0..30 {
        p.offer_hand(food.0, food.1);
        p.update(DT, &w);
    }
    assert_eq!(p.render().anim, Anim::Look);
    assert_eq!(p.facing, -1.0);
}

#[test]
fn without_a_lookup_anim_food_overhead_is_just_looked_at_like_before() {
    let (mut p, w) = standing_at(cfg(), 800.0);
    let food = above_head(800.0, 200.0);
    let mut walked = false;
    for _ in 0..300 {
        p.offer_hand(food.0, food.1);
        p.update(DT, &w);
        let a = p.render().anim;
        assert_ne!(a, Anim::LookUp);
        walked |= a == Anim::Walk;
    }
    // 예전처럼 그 아래까지 걸어가서 쳐다본다
    assert!(walked && p.body.x > 900.0, "x {}", p.body.x);
    assert_eq!(p.render().anim, Anim::Look);
}

#[test]
fn a_cursor_lingering_over_the_head_makes_the_pet_look_up_after_a_second() {
    let (mut p, w) = standing_at(looker(), 800.0);
    let (cx, cy) = above_head(800.0, 30.0);
    let mut first_up = None;
    for i in 0..150 {
        p.hover(cx, cy, false);
        p.update(DT, &w);
        if p.render().anim == Anim::LookUp && first_up.is_none() {
            first_up = Some(i);
        }
    }
    let t = first_up.expect("looked up") as f64 * DT;
    assert!((LOOK_UP_HOVER_SECS - 0.05..=LOOK_UP_HOVER_SECS + 0.1).contains(&t), "after {t:.2}s");
    // 커서가 머무는 동안엔 계속 올려다본다
    for _ in 0..(60 * 5) {
        p.hover(cx, cy, false);
        p.update(DT, &w);
        assert_eq!(p.render().anim, Anim::LookUp, "{:?}", p.state);
    }
    // 커서가 떠나면 곧 평소대로
    for _ in 0..(60 * 3) {
        p.hover(cx, 900.0, false);
        p.update(DT, &w);
    }
    assert_ne!(p.render().anim, Anim::LookUp);
}

#[test]
fn a_walking_pet_stops_under_a_lingering_cursor_and_looks_up() {
    let (mut p, w) = standing_at(looker(), 700.0);
    p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
    for _ in 0..80 {
        let (cx, cy) = above_head(p.body.x, 10.0);
        p.hover(cx, cy, false);
        p.update(DT, &w);
    }
    assert_eq!(p.render().anim, Anim::LookUp, "{:?}", p.state);
}

#[test]
fn a_brief_or_low_cursor_does_not_make_the_pet_look_up() {
    let (mut p, w) = standing_at(looker(), 800.0);
    let (cx, cy) = above_head(800.0, 0.0);
    for _ in 0..40 {
        p.hover(cx, cy, false);
        p.update(DT, &w);
        assert_ne!(p.render().anim, Anim::LookUp);
    }
    // 머리 옆 (머리 위가 아니다)
    for _ in 0..120 {
        p.hover(900.0, 400.0 - 0.6 * 128.0, false);
        p.update(DT, &w);
        assert_ne!(p.render().anim, Anim::LookUp);
    }
}

#[test]
fn without_a_lookup_anim_a_lingering_cursor_overhead_changes_nothing() {
    let (mut p, w) = standing_at(cfg(), 800.0);
    let (cx, cy) = above_head(800.0, 0.0);
    for _ in 0..180 {
        p.hover(cx, cy, false);
        p.update(DT, &w);
        assert_ne!(p.render().anim, Anim::LookUp);
    }
    assert!(matches!(p.state, State::Idle { look: false, .. }), "{:?}", p.state);
}

/// 10분 × 6: 가끔 올려다보지만 (쉬다가·걷다가) 많지 않다 — 그리고 올려다볼 땐 늘 제자리에 멈춰 있다
#[test]
fn idle_look_ups_happen_now_and_then_but_stay_rare() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0)]);
    let (mut up, mut total, mut episodes, mut mid_walk) = (0usize, 0usize, 0usize, 0usize);
    for seed in 0..6 {
        let mut p = Pet::new(PetConfig { looks_up: true, ..sim_cfg() }, 960.0, 128.0, seed);
        let mut was = false;
        for _ in 0..(60 * 600) {
            let before = p.state;
            let x = p.body.x;
            p.update(DT, &w);
            let now = p.render().anim == Anim::LookUp;
            if now {
                up += 1;
                assert!(matches!(p.state, State::Idle { look: true, .. }), "seed {seed}: {:?}", p.state);
                if was {
                    assert_eq!(p.body.x, x, "seed {seed}: looks up standing still");
                }
            }
            if now && !was {
                episodes += 1;
                mid_walk += matches!(before, State::Move { kind: Move::Walk, .. }) as usize;
            }
            was = now;
            total += 1;
        }
    }
    let share = up as f64 / total as f64;
    assert!(episodes >= 12, "look-ups {episodes}");
    assert!(mid_walk >= 1, "never stopped mid-walk to look up ({episodes} look-ups)");
    assert!((0.005..=0.06).contains(&share), "looking up {share:.3} of the time ({episodes} look-ups)");
}

/// lookup이 없는 동물은 한 번도 LookUp을 그리지 않는다 (주사위도 굴리지 않아 난수 순서·하는 일이 예전 그대로)
#[test]
fn pets_without_lookup_never_look_up() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0)]);
    for seed in 0..3 {
        let mut p = Pet::new(sim_cfg(), 960.0, 128.0, seed);
        for _ in 0..(60 * 300) {
            p.update(DT, &w);
            assert_ne!(p.render().anim, Anim::LookUp);
        }
    }
    // 같은 순간에 lookup이 있는 동물은 올려다보고, 없는 동물은 그냥 쳐다본다
    let (mut p, _) = standing_at(cfg(), 800.0);
    p.state = State::Idle { remaining: 1.0, look: true };
    p.gaze_up = true;
    assert_eq!(p.render().anim, Anim::Look);
    p.cfg.looks_up = true;
    assert_eq!(p.render().anim, Anim::LookUp);
    assert!(!p.render().flip);
    p.facing = -1.0;
    assert!(p.render().flip, "side view — flipped like look");
}

#[test]
fn lookup_serializes_as_the_manifest_name() {
    assert_eq!(serde_json::to_string(&Anim::LookUp).unwrap(), "\"lookup\"");
    assert!(!Anim::LookUp.is_front() && !Anim::LookUp.is_back());
}
