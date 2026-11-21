//! 🖍️ 짱구 (brain_jjanggu.rs): 🍑 엉덩이 춤 · 🦸 액션 빔 · 🍪 초코비 · 😴 뒹굴뒹굴 · 🐶 흰둥이 · 🍪 좋아하는 음식

use super::jjanggu_moves::{self as jj, JjangguAct};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::magic::MagicFx;
use crate::test_util::{pet_cfg, DT};

/// 번들 짱구 설정 (청소년, 진짜 pet.json)
pub(super) fn jjanggu_cfg() -> PetConfig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets/jjanggu/pet.json");
    let m = crate::manifest::parse_manifest(&std::fs::read_to_string(path).unwrap()).unwrap();
    PetConfig::from_manifest(&m, 1.0, m.size_mult(crate::care::Stage::Teenager), "teenager")
}

/// 바닥(x)에 쉬는 짱구 — 놀이·흰둥이는 테스트가 시작
fn calm(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(jjanggu_cfg(), x, seed, w);
    p.hush_stage();
    p.jjanggu.ready_at = f64::INFINITY;
    p.jjanggu.shiro_ready_at = f64::INFINITY;
    p
}

/// 흰둥이 덮개 (x, t, len)
fn shiro(p: &Pet) -> Option<(f64, f64, f64)> {
    let mut fx = Vec::new();
    p.magic_fx(&mut fx);
    fx.into_iter().find_map(|f| match f {
        MagicFx::Shiro { x, t, len, .. } => Some((x, t, len)),
        _ => None,
    })
}

#[test]
fn jjanggu_animations_and_lines_have_their_sheet_names() {
    for (a, name, front) in [(Anim::ButtDance, "butt_dance", false), (Anim::ActionPose, "action_pose", false), (Anim::RollLazy, "roll_lazy", false),
        (Anim::Lie, "lie", false), (Anim::Flirt, "flirt", true), (Anim::Snack, "snack", true)] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::JJANGGU_ANIMS.contains(&name) && a.is_front() == front && !a.is_back(), "{name}");
    }
    for (s, name) in [(Speech::BuriBuri, "buri_buri"), (Speech::ActionBeam, "action_beam"), (Speech::Chocobi, "chocobi"), (Speech::Lazy, "lazy"),
        (Speech::Flirt, "flirt"), (Speech::Shiro, "shiro"), (Speech::Flattered, "flattered"), (Speech::BrushOff, "brush_off")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    let c = jjanggu_cfg();
    assert!(c.rascal && !c.rides_broom && !c.gamekeeper && c.spells.is_empty());
    assert!(!pet_cfg().rascal);
}

/// 한가하면 가끔 저절로 무언가 함 (엉덩이 춤·액션 빔·초코비·뒹굴) — 다른 동물은 안 함
#[test]
fn he_gets_up_to_mischief_on_his_own() {
    let w = world(vec![]);
    let mut p = calm(900.0, 5, &w);
    p.jjanggu.ready_at = 0.0;
    let mut kinds = std::collections::HashSet::new();
    for _ in 0..(900.0 / DT) as usize {
        p.update(DT, &w);
        if let State::Jjanggu { act, .. } = p.state {
            kinds.insert(std::mem::discriminant(&act));
        }
        if matches!(p.state, State::Sleep { .. } | State::WantFood { .. }) {
            p.state = State::Idle { remaining: 1.0, look: false };
        }
    }
    assert!(kinds.len() >= 3, "여러 가지: {}", kinds.len());
    let mut cat = resting(pet_cfg(), 900.0, 5, &w);
    cat.hush_stage();
    cat.jjanggu.ready_at = 0.0;
    for _ in 0..(300.0 / DT) as usize {
        cat.update(DT, &w);
        assert!(!matches!(cat.state, State::Jjanggu { .. }));
    }
}

/// 🍑 엉덩이 춤: "buri buri~" 하고 제자리에서 butt_dance, 끝나면 쉼
#[test]
fn the_butt_dance_says_buri_buri() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    assert!(p.start_jjanggu(JjangguAct::ButtDance { len: 3.0 }));
    assert_eq!(p.speech, Some(Speech::BuriBuri));
    let x = p.body.x;
    for _ in 0..(2.9 / DT) as usize {
        p.update(DT, &w);
        assert_eq!(p.render().anim, Anim::ButtDance);
    }
    assert!((p.body.x - x).abs() < 1.0, "제자리");
    for _ in 0..20 {
        p.update(DT, &w);
    }
    assert!(!matches!(p.state, State::Jjanggu { .. }));
}

/// 🦸 액션 빔: action_pose 한 번 ("Action beam!") 뒤 끝
#[test]
fn the_action_beam_pose_plays_once() {
    let w = world(vec![]);
    let mut p = calm(900.0, 2, &w);
    assert!(p.start_jjanggu(JjangguAct::Action));
    assert_eq!(p.speech, Some(Speech::ActionBeam));
    p.update(DT, &w);
    assert_eq!(p.render().anim, Anim::ActionPose);
    for _ in 0..((jj::ACTION_SECS + 0.2) / DT) as usize {
        p.update(DT, &w);
    }
    assert!(!matches!(p.state, State::Jjanggu { .. }));
}

/// 🍪 초코비: 앉아서 snack ("Chocobi!")
#[test]
fn he_snacks_on_chocobi() {
    let w = world(vec![]);
    let mut p = calm(900.0, 3, &w);
    assert!(p.start_jjanggu(JjangguAct::Snack { len: 4.0 }));
    assert_eq!(p.speech, Some(Speech::Chocobi));
    for _ in 0..(3.9 / DT) as usize {
        p.update(DT, &w);
        assert_eq!(p.render().anim, Anim::Snack);
    }
    for _ in 0..20 {
        p.update(DT, &w);
    }
    assert!(!matches!(p.state, State::Jjanggu { .. }));
}

/// 😴 뒹굴뒹굴 → 엎드려 다리 까딱 ("So lazy~")
#[test]
fn he_rolls_lazily_then_lies_kicking_his_legs() {
    let w = world(vec![]);
    let mut p = calm(900.0, 4, &w);
    assert!(p.start_jjanggu(JjangguAct::Lazy { len: 8.0 }));
    assert_eq!(p.speech, Some(Speech::Lazy));
    let mut seen = Vec::new();
    for _ in 0..(7.9 / DT) as usize {
        p.update(DT, &w);
        let a = p.render().anim;
        if seen.last() != Some(&a) {
            seen.push(a);
        }
    }
    assert_eq!(seen, vec![Anim::RollLazy, Anim::Lie]);
}

/// 사용자가 집어 들면 하던 장난은 그만
#[test]
fn grabbing_him_stops_the_mischief() {
    let w = world(vec![]);
    let mut p = calm(900.0, 6, &w);
    assert!(p.start_jjanggu(JjangguAct::ButtDance { len: 5.0 }));
    p.pointer_down(p.body.x, p.body.y - 10.0);
    p.update(DT, &w);
    assert!(!matches!(p.state, State::Jjanggu { .. }));
}

/// 🐶 흰둥이: 가끔 옆에 나타나 ("Shiro!") 졸졸 따라다니다 (짱구 뒤쪽으로 따라옴) 사라짐
#[test]
fn shiro_appears_follows_him_and_vanishes() {
    let w = world(vec![]);
    let mut p = calm(600.0, 7, &w);
    assert!(shiro(&p).is_none());
    p.call_shiro(10.0);
    assert_eq!(p.speech, Some(Speech::Shiro));
    let (x0, _, len) = shiro(&p).expect("흰둥이");
    assert!((x0 - p.body.x).abs() < p.cfg.size, "옆에 나타남");
    assert_eq!(len, 10.0);
    // 짱구가 오른쪽으로 걸어가면 흰둥이도 따라옴 (뒤에서)
    p.facing = 1.0;
    p.state = State::Move { kind: crate::behavior::Move::Walk, remaining: 4.0 };
    for _ in 0..(4.0 / DT) as usize {
        p.update(DT, &w);
        if !matches!(p.state, State::Move { .. }) {
            p.facing = 1.0;
            p.state = State::Move { kind: crate::behavior::Move::Walk, remaining: 1.0 };
        }
    }
    let (x1, t, _) = shiro(&p).unwrap();
    assert!(x1 > x0 + 100.0, "따라옴 {x0} → {x1}");
    assert!(x1 < p.body.x + 0.2 * p.cfg.size, "뒤쪽: {x1} vs {}", p.body.x);
    assert!((t - 4.0).abs() < 0.1);
    for _ in 0..(6.5 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(shiro(&p).is_none(), "사라짐");
}

/// 🐶 흰둥이는 저절로도 가끔 옴
#[test]
fn shiro_comes_by_himself_now_and_then() {
    let w = world(vec![]);
    let mut p = calm(900.0, 8, &w);
    p.jjanggu.shiro_ready_at = 0.0;
    let mut came = false;
    for _ in 0..(1200.0 / DT) as usize {
        p.update(DT, &w);
        came |= shiro(&p).is_some();
        if matches!(p.state, State::Sleep { .. } | State::WantFood { .. }) {
            p.state = State::Idle { remaining: 1.0, look: false };
        }
    }
    assert!(came);
}
