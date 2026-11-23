//! 🔄 동물 바꾸기 (brain_swap.rs): 그 종만의 일은 그만두고, 아무 동물이나 하는 일은 그대로

use super::swing_tests::{heumul, resting};
use super::tests::world;
use super::*;
use crate::test_util::{pet_cfg, DT};

/// 흐물이가 줄행랑치다 (panic) 루돌프가 되면 줄행랑을 그만둠 — 루돌프엔 panic 그림이 없음 (qa_sim seed 102)
#[test]
fn a_heumul_turned_into_another_pet_stops_its_panic_run() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 700.0, 1, &w);
    p.set_cursor_species("heumul");
    p.state = State::Panic { remaining: 2.0, dir: 1.0 };
    p.cfg = pet_cfg();
    p.set_cursor_species("rudolph");
    assert!(!matches!(p.state, State::Panic { .. }), "{:?}", p.state);
    p.update(DT, &w);
    assert_ne!(p.render().anim, Anim::Panic);
}

/// 벽을 타던 동물이 벽을 못 타는 동물이 되면 떨어짐 (qa_sim seed 112 — 펭귄이 climb 그림 없이 벽에 붙어 있었음)
#[test]
fn a_climber_turned_into_a_non_climber_falls() {
    let mut p = Pet::new(pet_cfg(), 300.0, 600.0, 2);
    p.set_cursor_species("cat");
    p.standing = None;
    p.state = State::WallClimb { window: 1, side: Side::Left, rel_y: 100.0 };
    p.set_cursor_species("penguin");
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
}

/// 쉬기·잠은 그대로, 처음 정할 때 (앱이 펫을 만들 때)와 같은 종을 다시 알릴 때 (성장)는 아무것도 바꾸지 않음
#[test]
fn plain_states_and_growth_are_left_alone() {
    let w = world(vec![]);
    let mut p = resting(heumul(), 700.0, 3, &w);
    p.state = State::Panic { remaining: 2.0, dir: 1.0 };
    p.set_cursor_species("heumul");
    assert!(matches!(p.state, State::Panic { .. }), "처음 정하기");
    p.set_cursor_species("heumul");
    assert!(matches!(p.state, State::Panic { .. }), "같은 종 (성장)");
    p.state = State::Sleep { remaining: 30.0 };
    p.set_cursor_species("cat");
    assert!(matches!(p.state, State::Sleep { .. }));
}

/// 🦦 패트로누스를 부르던 헤르미온느가 볼드모트가 되면 달리던 은빛 수달도 사라짐 (qa_sim seed 101)
#[test]
fn a_running_patronus_goes_away_with_the_caster_species() {
    let mut p = Pet::new(PetConfig { patronus: Some(crate::wizard::PatronusAnimal::Otter), ..pet_cfg() }, 500.0, 1040.0, 4);
    p.set_cursor_species("hermione");
    p.duel.stag = Some(duel_moves::Stag { key: 9, from: (500.0, 980.0), to: (900.0, 700.0), t: 0.5, dir: 1.0, hit: false, animal: crate::wizard::PatronusAnimal::Otter, bob: 4.0 });
    p.cfg = pet_cfg();
    p.set_cursor_species("voldemort");
    let mut fx = Vec::new();
    p.magic_fx(&mut fx);
    assert!(!fx.iter().any(|f| matches!(f, crate::magic::MagicFx::Otter { .. } | crate::magic::MagicFx::Stag { .. })), "{fx:?}");
}

/// 😾 고양이가 "흥" (huff) 하며 인사하던 중 다람쥐가 되면 인사는 이어 가되 고개 갸웃으로 — 다람쥐엔 huff 그림이 없음 (qa_sim seed 103·111)
#[test]
fn a_cat_huffing_hello_turned_into_another_pet_tilts_instead() {
    let mut p = Pet::new(pet_cfg(), 500.0, 1040.0, 5);
    p.set_cursor_species("cat");
    p.state = State::Greet { look: 0.0, anim: Anim::Huff, remaining: 1.0 };
    p.set_cursor_species("squirrel");
    assert!(matches!(p.state, State::Greet { anim: Anim::Tilt, .. }), "{:?}", p.state);
}
