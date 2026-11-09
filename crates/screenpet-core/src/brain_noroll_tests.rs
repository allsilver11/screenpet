//! 구르기 그림이 없는 동물 (픽셀 그림, PetConfig::no_roll): 구르기를 고르지 않고 대신하는 동작도 없다. 어떤 길로도 `roll` 그림을 보내지 않는다.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{place, sim_cfg, win, world};
use super::*;
use crate::test_util::DT;


fn pixel() -> PetConfig {
    PetConfig { no_roll: true, ..sim_cfg() }
}

/// 걷고 나는 동물 몇 분 — 구르는 상태도 그림도 없다
fn assert_never_rolls(c: PetConfig, minutes: usize, seeds: u64) -> Vec<Anim> {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0), win(2, 1300.0, 350.0, 1800.0, 700.0)]);
    let mut seen = Vec::new();
    for seed in 0..seeds {
        let mut p = Pet::new(c, 960.0, 128.0, seed);
        for _ in 0..(60 * 60 * minutes) {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Move { kind: Move::Roll, .. }), "seed {seed}: {:?}", p.state);
            let anim = p.render().anim;
            assert_ne!(anim, Anim::Roll, "seed {seed}: {:?}", p.state);
            if !seen.contains(&anim) {
                seen.push(anim);
            }
        }
    }
    seen
}

#[test]
fn a_pixel_walker_never_rolls_but_still_walks_and_runs() {
    let seen = assert_never_rolls(pixel(), 5, 3);
    assert!(seen.contains(&Anim::Walk) && seen.contains(&Anim::Run), "{seen:?}");
}

#[test]
fn a_pixel_flier_skips_its_somersault() {
    assert_never_rolls(PetConfig { flies: true, ..pixel() }, 5, 2);
}

/// 픽셀 기린이 목 늘이기 층을 갖고 있으면 그대로 늘인다
#[test]
fn a_pixel_giraffe_with_layers_still_stretches() {
    let seen = assert_never_rolls(PetConfig { special: Some(Special::NeckStretch), ..pixel() }, 5, 3);
    assert!(seen.contains(&Anim::NeckStretch), "{seen:?}");
}

/// 없으면 그냥 걷는다 (코어에는 special 없이 온다 — manifest::special)
#[test]
fn no_replacement_animation_is_invented() {
    let seen = assert_never_rolls(PetConfig { special: None, ..pixel() }, 3, 2);
    assert!(!seen.contains(&Anim::NeckStretch), "{seen:?}");
}

/// 산타의 커서 놀이 썰매(gait Roll)도 없다 — 그림은 걷기
#[test]
fn santas_sled_is_gone_in_cursor_play() {
    let w = world(vec![]);
    let mut p = Pet::new(pixel(), 0.0, 0.0, 1);
    p.set_cursor_species("santa");
    place(&mut p, &w, Owner::Ground(0), 800.0, State::CursorPlay { phase: cursor_moves::PlayPhase::Approach, remaining: 5.0 });
    assert_eq!(p.render().anim, Anim::Walk);
    let mut round = Pet::new(sim_cfg(), 0.0, 0.0, 1);
    round.set_cursor_species("santa");
    place(&mut round, &w, Owner::Ground(0), 800.0, State::CursorPlay { phase: cursor_moves::PlayPhase::Approach, remaining: 5.0 });
    assert_eq!(round.render().anim, Anim::Roll, "round santa keeps the sled");
}

/// 쿵쾅에 넘어지거나 창에서 굴러떨어질 때 (구르기 그림을 빌려 쓰던 상태들): 픽셀은 떨어지는 그림
#[test]
fn tumbling_borrows_the_fall_picture() {
    let w = world(vec![]);
    let mut p = Pet::new(pixel(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Tumble { remaining: 1.0, dir: 1.0 });
    assert_eq!(p.render().anim, Anim::Fall);
    p.state = State::Air { anim: Anim::Roll };
    assert_eq!(p.render().anim, Anim::Fall);
}

/// 픽셀로 바꾸는 순간 구르던 중이어도 (스타일은 언제든 바뀐다) `roll` 그림은 나가지 않는다
#[test]
fn switching_style_mid_roll_shows_walking() {
    let w = world(vec![]);
    let mut p = Pet::new(pixel(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Move { kind: Move::Roll, remaining: 2.0 });
    assert_eq!(p.render().anim, Anim::Walk);
}

/// 눈사람 만들기의 눈덩이 굴리기: 몸은 걷기 (기린과 같다)
#[test]
fn rolling_a_snowball_walks_instead() {
    let w = world(vec![]);
    let mut p = Pet::new(pixel(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Snowman { phase: snow_moves::SnowPhase::Roll, remaining: 1.0, dir: 1.0 });
    assert_eq!(p.render().anim, Anim::Walk);
}
