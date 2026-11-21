//! 💕 짱구의 작업 (social_flirt.rs): 헤르미온느가 같은 화면 (같은 디딤판)에 있으면 짱구가 가끔 쪼르르 달려가 "Hey pretty~" 하고
//! 쑥스러운 몸짓 (flirt) → 헤르미온느는 웃어 주거나 (하트, "Oh! Thank you~") 흥 하고 무시 ("Hmph!"). 헤르미온느마다 쿨다운

use super::campfire_tests::{idle_at, quiet, step};
use super::*;
use crate::brain::{Anim, Pet, Speech};
use crate::geom::Rect;
use crate::test_util::DT;
use crate::world::World;

const GROUND: f64 = 1040.0;

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 작업이 바로 시작되는 코디네이터 (smile = 웃어 줄 확률)
fn eager(smile: f64) -> Social {
    let mut s = quiet(4);
    s.set_flirt_rate(50.0);
    s.set_flirt_smile(smile);
    s
}

/// secs초 돌리며 들은 말 (누가, 무엇)·가장 가까운 거리·본 그림 (누가, 그림)
fn watch(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) -> (Vec<(usize, Speech)>, f64, Vec<(usize, Anim)>) {
    let (mut said, mut closest, mut anims) = (Vec::new(), f64::MAX, Vec::new());
    for _ in 0..(secs / DT) as usize {
        step(s, pets, w);
        closest = closest.min((pets[0].body.x - pets[1].body.x).abs());
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                said.push((i, sp));
            }
            let a = p.render().anim;
            if !anims.contains(&(i, a)) {
                anims.push((i, a));
            }
        }
    }
    (said, closest, anims)
}

#[test]
fn flirt_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::Flirt, "flirt"), (Speech::Flattered, "flattered"), (Speech::BrushOff, "brush_off")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 💕 짱구가 달려가 "Hey pretty~" + flirt → 헤르미온느가 웃어 줌 (하트) — 곧바로 또 하지 않음
#[test]
fn jjanggu_runs_over_and_hermione_smiles() {
    let w = floor();
    let mut pets = vec![idle_at(&w, "hermione", 1200.0, 1), idle_at(&w, "jjanggu", 300.0, 2)];
    let mut s = eager(1.0);
    let (said, closest, anims) = watch(&mut s, &mut pets, &w, 15.0);
    assert_eq!(s.flirts_started(), 1);
    assert!(closest < 1.4 * pets[1].cfg.size, "옆까지 옴 {closest}");
    assert!(anims.contains(&(1, Anim::Run)), "쪼르르 달려감 {anims:?}");
    assert!(anims.contains(&(1, Anim::Flirt)), "{anims:?}");
    assert!(said.contains(&(1, Speech::Flirt)), "{said:?}");
    assert!(said.contains(&(0, Speech::Flattered)), "{said:?}");
    assert!(anims.contains(&(0, Anim::Heart)), "{anims:?}");
    assert!(!pets[0].in_romp() && !pets[1].in_romp(), "끝나면 놓아 줌");
    let (again, _, _) = watch(&mut s, &mut pets, &w, 30.0);
    assert_eq!(s.flirts_started(), 1, "쿨다운");
    assert!(!again.iter().any(|l| l.1 == Speech::Flirt));
}

/// 💕 무시: "Hmph!" 하고 뾰로통 (annoyed) — 짱구는 그래도 쑥스럽게
#[test]
fn hermione_can_brush_him_off() {
    let w = floor();
    let mut pets = vec![idle_at(&w, "hermione", 900.0, 1), idle_at(&w, "jjanggu", 1500.0, 2)];
    let mut s = eager(0.0);
    let (said, _, anims) = watch(&mut s, &mut pets, &w, 15.0);
    assert!(said.contains(&(0, Speech::BrushOff)), "{said:?}");
    assert!(!said.contains(&(0, Speech::Flattered)));
    assert!(anims.contains(&(0, Anim::Annoyed)), "{anims:?}");
}

/// 💕 헤르미온느에게만 (고양이·해리에겐 안 함), 짱구만 (해리는 안 함)
#[test]
fn only_jjanggu_flirts_and_only_with_hermione() {
    let w = floor();
    for (a, b) in [("cat", "jjanggu"), ("harry", "jjanggu"), ("hermione", "harry")] {
        let mut pets = vec![idle_at(&w, a, 900.0, 1), idle_at(&w, b, 400.0, 2)];
        let mut s = eager(1.0);
        watch(&mut s, &mut pets, &w, 5.0);
        assert_eq!(s.flirts_started(), 0, "{a} {b}");
    }
}

/// 💕 다가가는 사이 짱구가 붙잡히면 그만두고 헤르미온느도 놓아 줌
#[test]
fn flirt_stops_when_jjanggu_is_grabbed() {
    let w = floor();
    let mut pets = vec![idle_at(&w, "hermione", 1500.0, 1), idle_at(&w, "jjanggu", 500.0, 2)];
    let mut s = eager(1.0);
    for _ in 0..(0.3 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(pets[1].in_romp(), "다가가는 중");
    let (x, y) = (pets[1].body.x, pets[1].body.y - 10.0);
    pets[1].pointer_down(x, y);
    for _ in 0..(0.5 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(!pets[0].in_romp(), "헤르미온느도 풀림");
}
