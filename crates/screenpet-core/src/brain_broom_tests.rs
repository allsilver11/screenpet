//! 🧹 빗자루 비행 (brain_broom.rs): 가끔 빗자루로 화면 허공을 낮고 높게 날다 아무 디딤판에 사뿐 · 친구 화면으로 날아 나가기 ·
//! 빗자루로 오가기 · 끊기면 빗자루째 내려옴 · 🟡 골든 스니치 쫓아 잡기 (해리만)

use super::broom_moves::{BroomPhase, Dest};
use super::swing_tests::resting;
use super::tests::{place, w1, world};
use super::*;
use crate::broom_fly as bf;
use crate::test_util::{pet_cfg, LoopLimit, DT};
use crate::wizard::{Spell, SpellSet};

/// 빗자루를 타는 마법사 (테스트 설정 — 벽 타기·올려다보기 그림 있음, 주문 없음)
pub(super) fn rider() -> PetConfig {
    PetConfig { rides_broom: true, can_climb: true, looks_up: true, ..pet_cfg() }
}

/// 🟡 스니치를 쫓는 해리 (테스트 설정)
fn seeker() -> PetConfig {
    PetConfig { seeker: true, spells: SpellSet::of(&[Spell::Expelliarmus, Spell::Leviosa]), ..rider() }
}

/// 바닥(x)에 쉬는 마법사 — 연령대 몸짓은 꺼 둠, 스스로 빗자루·스니치는 안 함 (테스트가 시작)
fn calm(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(c, x, seed, w);
    p.hush_stage();
    p.broom.ready_at = f64::INFINITY;
    p.broom.snitch_ready_at = f64::INFINITY;
    p.spell.ready_at = f64::INFINITY;
    p
}

/// 날다 내려앉을 때까지 (최대 60초) 돌리며 살펴봄: (가장 높이 뜬 높이, 내내 화면 안이었나, 내려앉은 디딤판)
fn fly_until_landed(p: &mut Pet, w: &World) -> (f64, bool, Option<Owner>) {
    let ground = w.anchor(Owner::Ground(0)).unwrap().1;
    let mut highest: f64 = 0.0;
    let mut inside = true;
    for _ in 0..(60 * 60) {
        p.update(DT, w);
        let (_, top) = p.window_pos();
        inside &= top >= w.bounds.top - 1.0 && p.body.x >= w.bounds.left && p.body.x <= w.bounds.right;
        highest = highest.max(ground - p.body.y);
        if matches!(p.state, State::Land { .. }) {
            assert_eq!(p.render().anim, Anim::Broom, "빗자루째 사뿐");
            assert!(!p.render().thud, "콰당 없음");
            return (highest, inside, p.standing.map(|s| s.owner));
        }
        if matches!(p.state, State::Broom { phase: BroomPhase::Leg { .. }, .. }) {
            assert!(p.standing.is_none());
            assert_eq!(p.render().anim, Anim::Broom);
        }
    }
    (highest, inside, None)
}

#[test]
fn broom_animations_and_lines_have_their_sheet_names() {
    assert_eq!(serde_json::to_string(&Anim::Broom).unwrap(), "\"broom\"");
    assert!(!Anim::Broom.is_front() && !Anim::Broom.is_back());
    assert!(crate::manifest::BROOM_ANIMS.contains(&"broom"));
    for (s, name) in [(Speech::BroomUp, "broom_up"), (Speech::BroomLand, "broom_land"), (Speech::BroomAway, "broom_away"), (Speech::Snitch, "snitch"), (Speech::GotIt, "got_it")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 빗자루에 올라 디딤판을 떠나 허공을 낮게·높게 날다가 (화면 밖으로 안 나감) 아무 디딤판에 사뿐 내려앉음
#[test]
fn a_broom_flight_leaves_the_platforms_arcs_through_the_air_and_lands_softly() {
    let w = world(vec![w1()]);
    let mut tops = Vec::new();
    let mut on_window = 0;
    for seed in 0..30 {
        let mut p = calm(rider(), 300.0, seed, &w);
        assert!(p.start_broom_flight());
        assert_eq!(p.render().anim, Anim::Broom, "오르는 동안도 빗자루");
        assert!(p.on_broom());
        let (highest, inside, landed) = fly_until_landed(&mut p, &w);
        assert!(inside, "{seed}: 창이 화면 안");
        let owner = landed.unwrap_or_else(|| panic!("{seed}: 내려앉지 않음 {:?}", p.state));
        on_window += matches!(owner, Owner::Window(1)) as u32;
        tops.push(highest);
        // 내려앉은 뒤엔 평소대로
        crate::test_util::run_until(&mut p, &w, 120, |p| !matches!(p.state, State::Land { .. }));
        assert!(!p.on_broom() && p.standing.is_some());
    }
    let size = rider().size;
    assert!(tops.iter().all(|&h| h > 1.0 * size), "디딤판을 따라가지 않고 허공으로 {tops:?}");
    assert!(tops.iter().any(|&h| h > 4.0 * size), "높은 호도 {tops:?}");
    assert!(on_window > 0 && on_window < 30, "창 윗면에도 바닥에도 내려앉음 {on_window}");
}

/// 가끔 스스로 빗자루를 탐 (몇 분에 한 번) — 빗자루가 없는 동물은 안 탐
#[test]
fn flights_happen_now_and_then_on_their_own() {
    let w = world(vec![]);
    let mut p = calm(rider(), 900.0, 5, &w);
    p.broom.ready_at = 0.0;
    let mut other = calm(pet_cfg(), 900.0, 5, &w);
    let (mut flights, mut was) = (0, false);
    for _ in 0..(60 * 60 * 15) {
        p.update(DT, &w);
        other.update(DT, &w);
        let now = matches!(p.state, State::Broom { .. });
        flights += (now && !was) as u32;
        was = now;
        assert!(!matches!(other.state, State::Broom { .. }));
    }
    assert!((3..=20).contains(&flights), "15분에 {flights}번");
}

/// 집이 그리운 손님은 빗자루로 친구 화면 끝 너머로 날아 나감 (걸어 나간 것과 같이 take_exit)
#[test]
fn a_homesick_guest_flies_out_over_the_edge_on_the_broom() {
    let w = world(vec![]);
    let mut p = calm(rider(), 900.0, 2, &w);
    p.set_exit_edges(false, true);
    p.set_guest(Side::Right, 0.0);
    assert!(p.start_broom_flight());
    let mut guard = LoopLimit::new();
    let side = loop {
        guard.step();
        p.update(DT, &w);
        if let Some(side) = p.take_exit() {
            break side;
        }
        if let State::Broom { phase: BroomPhase::Leg { leg, .. }, .. } = p.state {
            if matches!(leg.to, Dest::Out { .. }) {
                assert!(p.body.y < w.bounds.bottom - p.cfg.size * 0.5, "날아서 나감");
            }
        }
    };
    assert_eq!(side, Side::Right);
    assert!(p.is_away());
}

/// 🚪 걸어서 친구 화면으로 나가고 들어올 때도 빗자루 (창을 조금 띄움, "off I go!")
#[test]
fn it_rides_the_broom_out_to_a_friend_and_back_in() {
    let w = world(vec![]);
    let mut out = None;
    for seed in 0..300 {
        let mut p = calm(rider(), 900.0, seed, &w);
        let x = w.bounds.right - p.cfg.half_w - 0.5;
        place(&mut p, &w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.set_exit_edges(false, true);
        p.update(DT, &w);
        if matches!(p.state, State::Exit { .. }) {
            out = Some(p);
            break;
        }
    }
    let mut p = out.expect("300 시드 동안 안 나감");
    assert_eq!(p.render().anim, Anim::Broom);
    assert_eq!(p.speech, Some(Speech::BroomAway));
    let ground = w.anchor(Owner::Ground(0)).unwrap().1;
    crate::test_util::run_until(&mut p, &w, 30, |_| false);
    assert!(ground - p.cfg.size - p.window_pos().1 > 0.1 * p.cfg.size, "창이 떠 있음");
    let mut g = Pet::new_entering(rider(), &w, Side::Left, 3);
    assert!(matches!(g.state, State::Enter { .. }));
    assert_eq!(g.render().anim, Anim::Broom);
    assert!((ground - g.cfg.size - g.window_pos().1 - bf::TRANSIT_LIFT * g.cfg.size).abs() < 1.0, "떠서 들어옴");
    let mut guard = LoopLimit::new();
    while matches!(g.state, State::Enter { .. }) {
        guard.step();
        g.update(DT, &w);
    }
    crate::test_util::run_until(&mut g, &w, 120, |_| false);
    assert!((ground - g.cfg.size - g.window_pos().1).abs() < 1.0, "들어온 뒤 내려섬");
}

/// 날다 끊기면 (집중 시작) 빗자루째 천천히 내려와 사뿐 (콰당 없음)
#[test]
fn an_interrupted_flight_floats_down_on_the_broom() {
    let w = world(vec![]);
    let mut p = calm(rider(), 900.0, 7, &w);
    assert!(p.start_broom_flight());
    let ground = w.anchor(Owner::Ground(0)).unwrap().1;
    assert!(crate::test_util::run_until(&mut p, &w, 60 * 20, |p| ground - p.body.y > 2.0 * p.cfg.size));
    p.set_focus(true);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }));
    assert_eq!(p.render().anim, Anim::Broom);
    let mut guard = LoopLimit::new();
    while p.standing.is_none() {
        guard.step();
        p.update(DT, &w);
        assert!(p.body.vy <= bf::DESCENT_MAX_VY * p.cfg.size + 1e-6, "천천히");
    }
    assert!(!p.render().thud, "콰당 없음");
}

/// 🟡 해리: 스니치가 나타나면 올려다보고 빗자루로 쫓아 잡음 ("Got it!") — 잡은 스니치는 손에, 그 뒤 내려앉고 스니치는 사라짐
#[test]
fn harry_spots_chases_and_catches_the_golden_snitch() {
    let w = world(vec![]);
    let mut caught = 0;
    for seed in 0..12 {
        let mut p = calm(seeker(), 900.0, seed, &w);
        assert!(p.start_snitch(&w));
        assert_eq!(p.speech, Some(Speech::Snitch));
        assert_eq!(p.render().anim, Anim::LookUp);
        let s = p.snitch_spot().expect("스니치가 보임");
        assert!(!s.caught && s.y < w.bounds.bottom - 2.0 * p.cfg.size && s.size > 0.0);
        let mut got = false;
        let mut guard = LoopLimit::new();
        while !matches!(p.state, State::Land { .. }) {
            guard.step();
            p.update(DT, &w);
            if matches!(p.state, State::Broom { phase: BroomPhase::Caught, .. }) {
                got = true;
                assert_eq!(p.speech, Some(Speech::GotIt));
                assert!(p.snitch_spot().is_some_and(|s| s.caught), "손에 쥠");
            }
            if matches!(p.state, State::Broom { phase: BroomPhase::Chase, .. }) {
                assert_eq!(p.render().anim, Anim::Broom);
                assert!(p.snitch_spot().is_some());
            }
        }
        assert!(p.snitch_spot().is_none());
        caught += got as u32;
    }
    assert!(caught >= 10, "거의 늘 잡음 {caught}/12");
    // 스니치는 해리만
    let mut h = calm(rider(), 900.0, 1, &w);
    h.broom.snitch_ready_at = 0.0;
    for _ in 0..(60 * 60 * 10) {
        h.update(DT, &w);
        assert!(h.snitch_spot().is_none());
    }
}

/// 🟡 스니치는 가끔 저절로 나옴 (해리)
#[test]
fn the_snitch_shows_up_every_few_minutes() {
    let w = world(vec![]);
    let mut p = calm(seeker(), 900.0, 4, &w);
    p.broom.snitch_ready_at = 0.0;
    let (mut n, mut was) = (0, false);
    for _ in 0..(60 * 60 * 20) {
        p.update(DT, &w);
        let now = p.snitch_spot().is_some();
        n += (now && !was) as u32;
        was = now;
    }
    assert!((1..=10).contains(&n), "20분에 {n}번");
}
