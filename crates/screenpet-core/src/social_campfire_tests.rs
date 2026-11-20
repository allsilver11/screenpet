//! 🔥 모닥불 (social_campfire.rs): 해그리드·해리·헤르미온느가 같은 화면에 한가하면 엄청 반가워하고 (폴짝·하트) → 해그리드가 모닥불을 피우고 →
//! 셋이 둘러앉아 말풍선을 주고받다가 (가끔 다 같이 웃음) → 불을 끄고 흩어짐. 같은 셋은 몇 분 동안 다시 안 함. 누가 빠지면 불도 꺼짐

use super::*;
use crate::brain::{Anim, Pet, PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::magic::MagicFx;
use crate::test_util::{bundled, DT};
use crate::world::World;

const GROUND: f64 = 1040.0;

/// 번들 pet.json 그대로의 펫
fn cfg(id: &str) -> PetConfig {
    bundled().into_iter().find(|(pid, _)| pid == id).unwrap_or_else(|| panic!("ui/pets/{id}")).1
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 바닥 x에 쉬는 펫 — 혼자 하는 일 (연령대 특기·줄 놀이·마법사 놀이)은 꺼 둠
pub(super) fn idle_at(w: &World, id: &str, x: f64, seed: u64) -> Pet {
    let owner = Owner::Ground(0);
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(cfg(id), x, ay, seed);
    p.hush_stage();
    p.hush_web_play();
    p.hush_wizardry();
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = State::Idle { remaining: 1e6, look: false };
    p.update(0.0, w);
    p
}

/// 다른 어울림 (인사·만남·하이파이브·펫끼리 놀이)은 끈 코디네이터
pub(super) fn quiet(seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 0.0;
    s.one_way_chance = 0.0;
    s.set_high_five_chance(0.0);
    s.set_romp_rate(0.0);
    s.hold_greets(1e9);
    s
}

/// 앱처럼 한 틱 (쉬는 펫은 계속 쉬게 둠)
pub(super) fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
        if matches!(p.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. }) && !p.in_romp() {
            p.state = State::Idle { remaining: 1e6, look: false };
        }
    }
}

/// 해그리드가 내놓은 모닥불 (x, 밝기)
fn fire(p: &Pet) -> Option<(f64, f64)> {
    let mut fx = Vec::new();
    p.magic_fx(&mut fx);
    fx.into_iter().find_map(|f| match f {
        MagicFx::Campfire { x, level, .. } => Some((x, level)),
        _ => None,
    })
}

#[derive(Default)]
struct Seen {
    lines: Vec<(usize, Speech)>,
    hearts: usize,
    hopped: bool,
    lit: Option<f64>,
    sat_secs: f64,
    fire_between: bool,
    ended_at: Option<f64>,
}

/// secs초 돌리며 본 것 (pets[2] = 해그리드)
fn watch(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) -> Seen {
    let mut seen = Seen::default();
    let mut started = false;
    for k in 0..(secs / DT) as usize {
        step(s, pets, w);
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                seen.lines.push((i, sp));
            }
            let r = p.render();
            seen.hearts += usize::from(r.heart && p.in_romp());
            seen.hopped |= p.in_romp() && r.anim == Anim::Heart && p.window_pos().1 < p.body.y - p.cfg.size - 1.0;
        }
        if let Some((x, level)) = fire(&pets[2]) {
            if level >= 1.0 && seen.lit.is_none() {
                seen.lit = Some(k as f64 * DT);
            }
            let (lo, hi) = pets.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.body.x), hi.max(p.body.x)));
            seen.fire_between |= lo < x && x < hi;
        }
        if pets.iter().all(|p| p.in_romp() && p.render().anim == Anim::SitFront) {
            seen.sat_secs += DT;
        }
        started |= pets.iter().all(|p| p.in_romp());
        if started && seen.ended_at.is_none() && pets.iter().all(|p| !p.in_romp()) {
            seen.ended_at = Some(k as f64 * DT);
        }
    }
    seen
}

#[test]
fn campfire_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::Reunion, "reunion"), (Speech::LightFire, "light_fire"), (Speech::Fireside, "fireside"), (Speech::HaHa, "ha_ha"), (Speech::Douse, "douse")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    let fx = serde_json::to_value(MagicFx::Campfire { x: 1.0, y: 2.0, size: 3.0, level: 0.5 }).unwrap();
    assert_eq!(fx["kind"], "campfire");
}

/// 🔥 셋이 반가워 폴짝·하트 → 해그리드가 불을 피움 (셋 사이 바닥) → 둘러앉아 (sit_front) 20초 넘게 번갈아 도란도란 (가끔 다 같이 "Ha ha!") →
/// 불을 끄고 "see ya!" 흩어짐 — 불도 사라짐
#[test]
fn hagrid_harry_and_hermione_sit_around_a_campfire_and_chat() {
    let w = floor();
    let mut pets = vec![idle_at(&w, "harry", 500.0, 1), idle_at(&w, "hermione", 800.0, 2), idle_at(&w, "hagrid", 1200.0, 3)];
    let mut s = quiet(1);
    let seen = watch(&mut s, &mut pets, &w, 70.0);
    assert_eq!(s.campfires_started(), 1);
    assert!(seen.hearts > 0 && seen.hopped, "반가워 폴짝·하트");
    let reunion: Vec<usize> = seen.lines.iter().filter(|l| l.1 == Speech::Reunion).map(|l| l.0).collect();
    assert!(reunion.len() >= 3, "{:?}", seen.lines);
    assert!(seen.lines.iter().any(|l| l == &(2, Speech::LightFire)), "해그리드가 불을 피움 {:?}", seen.lines);
    assert!(seen.lit.is_some() && seen.fire_between, "셋 사이에 불");
    assert!(seen.sat_secs >= 18.0, "둘러앉음 {}", seen.sat_secs);
    let chat: Vec<usize> = seen.lines.iter().filter(|l| matches!(l.1, Speech::Fireside | Speech::RockCake)).map(|l| l.0).collect();
    assert!(chat.len() >= 6, "도란도란 {:?}", seen.lines);
    assert!(chat.windows(2).all(|c| c[0] != c[1]), "번갈아 {chat:?}");
    for who in 0..3 {
        assert!(chat.contains(&who), "모두 한마디씩 {chat:?}");
    }
    assert!(seen.lines.iter().any(|l| l.1 == Speech::HaHa), "가끔 웃음");
    assert!(seen.lines.iter().any(|l| l == &(2, Speech::Douse)), "불을 끔");
    assert!(seen.lines.iter().filter(|l| l.1 == Speech::SeeYa).count() >= 3);
    assert!(seen.ended_at.is_some(), "흩어짐");
    assert!(fire(&pets[2]).is_none(), "불이 사라짐");
    // 같은 셋은 쿨다운 동안 다시 안 함
    let again = watch(&mut s, &mut pets, &w, 90.0);
    assert_eq!(s.campfires_started(), 1);
    assert!(again.lines.iter().all(|l| l.1 != Speech::LightFire));
}

/// 🔥 셋이 다 있어야 함 (둘만 · 다른 동물은 안 낌)
#[test]
fn no_campfire_without_all_three() {
    let w = floor();
    for ids in [["harry", "hermione", "cat"], ["hagrid", "harry", "harry"], ["hagrid", "hermione", "voldemort"]] {
        let mut pets: Vec<Pet> = ids.iter().enumerate().map(|(k, id)| idle_at(&w, id, 500.0 + 300.0 * k as f64, k as u64 + 1)).collect();
        let mut s = quiet(1);
        watch(&mut s, &mut pets, &w, 20.0);
        assert_eq!(s.campfires_started(), 0, "{ids:?}");
    }
}

/// 🔥 누가 빠지면 (친구 화면으로 등) 남은 둘은 풀려나고 불도 꺼짐
#[test]
fn campfire_breaks_up_when_someone_leaves() {
    let w = floor();
    let mut pets = vec![idle_at(&w, "harry", 500.0, 1), idle_at(&w, "hermione", 800.0, 2), idle_at(&w, "hagrid", 1200.0, 3)];
    let mut s = quiet(1);
    for _ in 0..(40.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        if fire(&pets[2]).is_some_and(|f| f.1 >= 1.0) {
            break;
        }
    }
    assert!(fire(&pets[2]).is_some(), "불이 켜짐");
    pets.remove(0);
    for _ in 0..(1.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
    }
    assert!(pets.iter().all(|p| !p.in_romp()), "풀려남");
    assert!(fire(&pets[1]).is_none(), "불이 꺼짐");
}
