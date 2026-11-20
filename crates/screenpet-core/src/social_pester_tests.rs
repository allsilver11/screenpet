//! 📖 공부 방해 (social_pester.rs): 헤르미온느가 앉아 책을 읽을 때 같은 디딤판의 해리·해그리드가 가끔 다가가 귀엽게 방해 (콕·손 흔들기·폴짝) →
//! 헤르미온느는 "Shh! I'm studying!" 하고 계속 읽거나 (반쯤) 책을 덮고 같이 놂 (술래잡기·달리기 시합 — 반쯤). 헤르미온느마다 쿨다운

use super::campfire_tests::{idle_at, quiet, step};
use super::*;
use crate::brain::{Pet, Speech, State};
use crate::geom::Rect;
use crate::test_util::DT;
use crate::world::World;

const GROUND: f64 = 1040.0;

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 읽고 있는 헤르미온느 (x)와 방해꾼 (id, bx)
fn scene(w: &World, pest: &str, bx: f64) -> Vec<Pet> {
    let mut her = idle_at(w, "hermione", 900.0, 1);
    assert!(her.start_reading());
    if let State::Study { act: crate::brain::study_moves::StudyAct::Read { len }, .. } = &mut her.state {
        *len = 1e6;
    }
    vec![her, idle_at(w, pest, bx, 2)]
}

/// 방해가 바로 시작되는 코디네이터 (play = 책을 덮고 놀 확률)
fn eager(play: f64) -> Social {
    let mut s = quiet(3);
    s.set_pester_rate(50.0);
    s.set_pester_play(play);
    s
}

/// secs초 돌리며 들은 말 (누가, 무엇)과 가장 가까이 온 거리
fn watch(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) -> (Vec<(usize, Speech)>, f64) {
    let (mut said, mut closest) = (Vec::new(), f64::MAX);
    for _ in 0..(secs / DT) as usize {
        step(s, pets, w);
        closest = closest.min((pets[0].body.x - pets[1].body.x).abs());
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                said.push((i, sp));
            }
        }
    }
    (said, closest)
}

#[test]
fn pester_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::Pester, "pester"), (Speech::Shh, "shh"), (Speech::OhAlright, "oh_alright")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 📖 해리가 다가가 방해 → "Shh! I'm studying!" → 헤르미온느는 계속 읽고 해리는 물러남. 곧바로 또 하지 않음
#[test]
fn harry_bothers_hermione_and_she_shushes_him() {
    let w = floor();
    let mut pets = scene(&w, "harry", 400.0);
    let mut s = eager(0.0);
    let (said, closest) = watch(&mut s, &mut pets, &w, 15.0);
    assert_eq!(s.pesters_started(), 1);
    assert!(closest < 1.3 * pets[1].cfg.size, "옆까지 옴 {closest}");
    assert!(said.contains(&(1, Speech::Pester)), "{said:?}");
    assert!(said.contains(&(0, Speech::Shh)), "{said:?}");
    assert!(pets[0].is_reading(), "계속 읽음");
    assert!(!pets[1].in_romp(), "해리는 물러남");
    let (again, _) = watch(&mut s, &mut pets, &w, 30.0);
    assert_eq!(s.pesters_started(), 1, "쿨다운");
    assert!(!again.iter().any(|l| l.1 == Speech::Shh));
}

/// 📖 해그리드도 방해하러 옴 — 이번엔 책을 덮고 "Oh, alright!" 같이 놂 (술래잡기 또는 달리기 시합)
#[test]
fn she_closes_the_book_and_plays_with_hagrid() {
    let w = floor();
    let mut pets = scene(&w, "hagrid", 1500.0);
    let mut s = eager(1.0);
    let mut played = false;
    let mut said = Vec::new();
    for _ in 0..(20.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                said.push((i, sp));
            }
        }
        let busy = |p: &Pet| p.social_role().is_some() || p.in_romp();
        if said.contains(&(0, Speech::OhAlright)) && busy(&pets[0]) && busy(&pets[1]) {
            played = true;
            assert!(!pets[0].is_reading());
        }
    }
    assert!(said.contains(&(1, Speech::Pester)), "{said:?}");
    assert!(played, "같이 놂 {said:?}");
}

/// 📖 다른 동물·멀리 다른 디딤판 친구는 방해하지 않음, 안 읽을 때도 안 함
#[test]
fn only_harry_or_hagrid_bother_a_reading_hermione() {
    let w = floor();
    let mut cat = scene(&w, "cat", 500.0);
    let mut s = eager(0.0);
    watch(&mut s, &mut cat, &w, 5.0);
    assert_eq!(s.pesters_started(), 0, "고양이는 안 함");
    let mut idle = scene(&w, "harry", 500.0);
    idle[0].state = State::Idle { remaining: 1e6, look: false };
    let mut s = eager(0.0);
    watch(&mut s, &mut idle, &w, 5.0);
    assert_eq!(s.pesters_started(), 0, "안 읽으면 안 함");
}

/// 📖 다가가는 사이 헤르미온느가 책을 그만 읽으면 방해도 그만둠
#[test]
fn pester_stops_when_she_stops_reading() {
    let w = floor();
    let mut pets = scene(&w, "harry", 200.0);
    let mut s = eager(0.0);
    for _ in 0..(0.3 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(pets[1].in_romp(), "다가가는 중");
    pets[0].state = State::Idle { remaining: 1e6, look: false };
    for _ in 0..(0.5 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(!pets[1].in_romp());
}
