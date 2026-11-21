//! 🛡️ 무쇠인간: 🚀 제트팩으로 낮게 날기 (슈퍼인간과 같은 brain_soar.rs — 그림만 제트팩)·🪖 쉬다가 가끔 투구 장착 (Special::SuitUp,
//! 앞모습, "Suit up!")·🛡️ 간질이면 투구 장착 대신 늘 팔뚝 막기 (tickle 그림)·🔋 좋아하는 음식은 배터리·🐾 커서 장난은 날아와 삐삐 손 흔들기

use super::swing_tests::resting;
use super::tests::{place, world};
use super::*;
use crate::behavior::Special;
use crate::geom::{Rect, WindowInfo};
use crate::test_util::{bundled, pet_cfg, DT};

/// 번들 pet.json 그대로의 무쇠인간 (청소년)
fn musoe() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "musoe").expect("ui/pets/musoe").1
}

/// 바닥(x)에 쉬는 무쇠인간 — 연령대 몸짓은 꺼 둠
fn calm(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(c, x, seed, w);
    p.hush_stage();
    p
}

#[test]
fn the_bundled_musoe_soars_on_a_jetpack_and_suits_up() {
    let c = musoe();
    assert!(c.soars && !c.flies && !c.juggler && !c.rides_bike && !c.slings_webs);
    assert_eq!(c.special, Some(Special::SuitUp));
    assert_eq!(serde_json::to_string(&Anim::SuitUp).unwrap(), "\"suit_up\"");
    assert_eq!(serde_json::to_string(&Speech::SuitUp).unwrap(), "\"suit_up\"");
    assert!(Anim::SuitUp.is_front() && !Anim::SuitUp.is_back());
    assert_eq!(Special::SuitUp.anim_name(), "suit_up");
    assert_eq!(Anim::from(Special::SuitUp), Anim::SuitUp);
    // 다른 번들 동물은 투구를 쓰지 않음
    for (id, cfg) in bundled() {
        assert_eq!(cfg.special == Some(Special::SuitUp), id == "musoe", "{id}");
    }
}

/// 🚀 걷기 대신 바닥 조금 위를 날아감 (슈퍼인간과 같은 날기 뇌 — 그림 fly·fly_fast·fly_flip)
#[test]
fn it_flies_low_like_the_super_hero() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 500.0, 1700.0, 900.0) }]);
    let mut p = calm(musoe(), 400.0, 3, &w);
    place(&mut p, &w, Owner::Window(1), 400.0, State::Move { kind: Move::Walk, remaining: 4.0 });
    p.facing = 1.0;
    p.soar.force_fly = true;
    let mut gap_max: f64 = 0.0;
    for _ in 0..90 {
        p.update(DT, &w);
        if !matches!(p.state, State::Move { .. }) {
            break;
        }
        assert!(matches!(p.render().anim, Anim::Fly | Anim::FlyFast | Anim::FlyFlip), "{:?}", p.render().anim);
        gap_max = gap_max.max(p.body.y - (p.window_pos().1 + p.cfg.size));
    }
    assert!(gap_max > 0.2 * p.cfg.size, "{gap_max}");
}

/// 🪖 투구 장착: "Suit up!", 앞모습 (반전 없음), 정해진 시간 안에 끝남
#[test]
fn suit_up_speaks_faces_front_and_ends() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 500.0, 1700.0, 900.0) }]);
    let mut p = calm(musoe(), 800.0, 1, &w);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
    p.start_special(Special::SuitUp);
    assert_eq!(p.take_speech(), Some(Speech::SuitUp));
    p.facing = -1.0;
    assert_eq!(p.render().anim, Anim::SuitUp);
    assert!(!p.render().flip, "앞모습은 반전하지 않음");
    let (lo, hi) = Special::SuitUp.secs();
    assert!(lo >= 1.5 && hi <= 3.5, "4칸 4fps 한 번 (1초) 뒤 렌즈 켠 채 잠깐");
    for _ in 0..(60.0 * (hi + 0.5)) as usize {
        p.update(DT, &w);
    }
    assert!(!matches!(p.state, State::Special { .. }));
}

/// 🪖 쉬는 박자의 몇 %만 투구 장착, 🛡️ 간질이면 투구 장착 대신 늘 팔뚝 막기 (tickle)
#[test]
fn suit_up_is_a_rare_rest_beat_and_tickles_always_get_the_arm_block() {
    assert!((0.04..=0.08).contains(&Special::SuitUp.rest_chance()));
    assert_eq!(Special::SuitUp.tickle_chance(), 0.0);
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 500.0, 1700.0, 900.0) }]);
    for seed in 0..200 {
        let mut p = Pet::new(PetConfig { special: Some(Special::SuitUp), ..pet_cfg() }, 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        for i in 0..10 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, &w);
        }
        assert!(!matches!(p.state, State::Special { .. }), "{seed}");
        assert_eq!(p.render().anim, Anim::Tickle, "{seed}");
    }
}

/// 🔋 배터리를 좋아함 · 🐾 커서 장난은 날아와서 삐삐 손 흔들기
#[test]
fn battery_hand_and_a_flying_wave_for_the_cursor() {
    assert_eq!(crate::food::hand_icon(crate::care::Food::Favorite, "musoe"), "hand_battery");
    let s = cursor_moves::CursorStyle::of("musoe").expect("musoe");
    let anims = s.anims();
    assert!(anims[0] == Anim::Fly && anims.contains(&Anim::Wave), "{anims:?}");
    assert_ne!(Some(s), cursor_moves::CursorStyle::of("super"));
}
