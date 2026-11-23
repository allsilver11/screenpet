//! 🧳 포트키 (brain_portkey.rs): 짝 물건이 있을 때만 "Portkey!" 하고 달려가 손 대고 빙글 사라져 짝 물건 옆에 나타남 · 물건은 제자리 ·
//! 친구 쪽은 앱에 물어보고, 거절되면 내 화면의 짝 또는 제자리 ("huh?") · 같이 잡은 마법사는 같이 감 · 바쁠 땐 안 함

use super::dark_tests::still;
use super::hoist_moves::sync_lifts;
use super::portkey_moves::{PortkeyEvent, PortkeyPhase};
use super::tests::world;
use super::*;
use crate::juggle::ObjectSpot;
use crate::object::ObjectKind;
use crate::portkey::{self as pk, PortkeyKey, PortkeySpot};
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{Spell, SpellSet};

const GROUND: f64 = 1040.0;

/// 마법사 (테스트 설정): 주문 + 포트키
fn wizard() -> PetConfig {
    PetConfig { spells: SpellSet::of(&[Spell::Leviosa]), portkey: true, ..pet_cfg() }
}

/// x에 쉬는 마법사 (key) — 포트키 쉬는 시간 없음
fn standing(x: f64, key: u64, w: &World) -> Pet {
    let mut p = still(wizard(), x, key, w);
    p.social_key = key;
    p.portkey.ready_at = 0.0;
    p
}

/// 바닥 x에 놓인 물건 (id, 종류)
fn thing(id: u64, kind: ObjectKind, x: f64) -> PortkeySpot {
    let half_w = if kind.is_giant() { 300.0 } else { 20.0 };
    PortkeySpot { spot: ObjectSpot { id, x, y: GROUND, owner: Owner::Ground(0), half_w, giant: kind.is_giant(), local: kind.is_toy() }, key: PortkeyKey::of(kind) }
}

/// 앱처럼 한 틱 (모두 같은 물건을 봄)
fn tick(pets: &mut [Pet], spots: &[PortkeySpot], w: &World) {
    for p in pets.iter_mut() {
        p.notice_portkeys(spots);
        if !p.is_away() {
            p.update(DT, w);
        }
    }
    sync_lifts(pets);
}

/// 🧳 신문지 둘 (800·1500)과 짝 없는 돌 (600): "Portkey!" 하고 신문지로 달려가 손 대고 (cast) 빙글 작아져 사라짐 (tuck → 1, twirl, 반짝) →
/// 다른 신문지 옆에 빙글 커지며 나타나 "whoa!" → 평소대로. 돌은 고르지 않음, 물건은 앱에 아무 일도 시키지 않음 (제자리)
#[test]
fn a_wizard_runs_to_a_paper_and_pops_out_next_to_the_other_paper() {
    let w = world(vec![]);
    let spots = [thing(1, ObjectKind::Paper, 800.0), thing(2, ObjectKind::Paper, 1500.0), thing(3, ObjectKind::Stone, 600.0)];
    for seed in 1..8 {
        let mut pets = vec![standing(500.0, seed, &w)];
        pets[0].notice_portkeys(&spots);
        assert!(pets[0].start_portkey(&w));
        assert_eq!(pets[0].take_speech(), Some(Speech::Portkey));
        let State::Portkey { phase: PortkeyPhase::Run { id, .. }, .. } = pets[0].state else { panic!("{:?}", pets[0].state) };
        assert!(id == 1 || id == 2, "돌은 짝이 없음: {id}");
        let (from, to) = if id == 1 { (800.0, 1500.0) } else { (1500.0, 800.0) };
        let (mut cast, mut gone, mut twirled, mut sparkled, mut landed) = (false, false, false, false, false);
        for _ in 0..(12.0 / DT) as usize {
            tick(&mut pets, &spots, &w);
            let r = pets[0].render();
            cast |= r.anim == Anim::Cast && pets[0].portkeying();
            gone |= r.tuck >= 0.999;
            twirled |= r.twirl > 10.0;
            sparkled |= r.sparkle;
            if pets[0].take_speech() == Some(Speech::PortkeyLand) {
                landed = true;
                assert!((pets[0].body.x - to).abs() < 1.5 * pets[0].cfg.size, "짝 옆: {} ({from} → {to})", pets[0].body.x);
            }
            assert!(pets[0].take_object_play().is_none(), "물건은 제자리");
            assert!(pets[0].take_portkey().is_none(), "내 화면 안");
        }
        assert!(cast && gone && twirled && sparkled && landed, "{seed}: {cast} {gone} {twirled} {sparkled} {landed}");
        assert!(!pets[0].portkeying() && pets[0].render().tuck == 0.0 && pets[0].standing.is_some());
        assert!(pets[0].portkey.ready_at > pets[0].clock + 100.0, "다음은 몇 분 뒤");
    }
}

/// 🧳 짝이 없으면 안 함 (똥 하나·돌 하나 — 친구도 없음). 새 버전 친구가 있으면 짝이 없어도 함. 바쁠 때 (하우스·집중·조작·쉬는 시간)는 안 함.
/// 💩 똥·🪨 거대한 돌·📌 같은 아이콘 장난감도 짝이 있으면 포트키 (거대한 것은 몸을 붙인 옆에 섬)
#[test]
fn only_objects_with_a_twin_and_only_when_free() {
    let w = world(vec![]);
    let lonely = [thing(1, ObjectKind::Poop, 800.0), thing(2, ObjectKind::Stone, 900.0), thing(3, ObjectKind::Pink, 700.0)];
    let mut p = standing(500.0, 1, &w);
    p.notice_portkeys(&lonely);
    assert!(!p.start_portkey(&w), "짝 없음");
    p.set_portkey_friend(true);
    assert!(p.start_portkey(&w), "친구 화면에 있을 수 있음");
    let pair = [thing(1, ObjectKind::Poop, 800.0), thing(2, ObjectKind::Poop, 1300.0)];
    let busy: [fn(&mut Pet); 4] = [|p| p.housed = true, |p| p.set_focus(true), |p| p.portkey.ready_at = p.clock + 10.0, |p| p.state = State::Dragged];
    for b in busy {
        let mut p = standing(500.0, 2, &w);
        p.notice_portkeys(&pair);
        b(&mut p);
        assert!(!p.start_portkey(&w));
    }
    for (kind, xs) in [(ObjectKind::Poop, (800.0, 1300.0)), (ObjectKind::GiantStone, (1000.0, 1600.0))] {
        let mut p = standing(500.0, 3, &w);
        let spots = [thing(1, kind, xs.0), thing(2, kind, xs.1)];
        p.notice_portkeys(&spots);
        assert!(p.start_portkey(&w), "{kind:?}");
        if let State::Portkey { phase: PortkeyPhase::Run { id, x }, .. } = p.state {
            let at = if id == 1 { xs.0 } else { xs.1 };
            assert!(!kind.is_giant() || (x - at).abs() >= 300.0, "거대한 것은 옆에 섬: {x} ({at})");
        }
    }
    let toy = |id: u64, look: u64, x: f64| PortkeySpot { key: PortkeyKey { kind: ObjectKind::AppIcon, look }, ..thing(id, ObjectKind::AppIcon, x) };
    let mut p = standing(500.0, 4, &w);
    p.notice_portkeys(&[toy(1, 7, 800.0), toy(2, 8, 1300.0)]);
    assert!(!p.start_portkey(&w), "다른 아이콘은 짝이 아님");
    p.notice_portkeys(&[toy(1, 7, 800.0), toy(2, 7, 1300.0)]);
    assert!(p.start_portkey(&w), "같은 아이콘");
}

/// 🧳 친구 쪽: 짝이 없으면 늘, 짝이 있으면 반쯤 앱에 물어봄 (AskFriend — 펫은 화면을 떠남).
/// 거절되면 (portkey_return) 내 화면의 짝 옆, 없으면 제자리에 다시 나타나 "huh?"
#[test]
fn friend_portkeys_ask_the_app_and_fall_back_to_a_twin_or_huh() {
    let w = world(vec![]);
    let run = |p: &mut Pet, spots: &[PortkeySpot]| {
        for _ in 0..(10.0 / DT) as usize {
            if p.is_away() {
                return;
            }
            p.notice_portkeys(spots);
            p.update(DT, &w);
        }
    };
    let one = [thing(1, ObjectKind::Can, 800.0)];
    let mut p = standing(500.0, 1, &w);
    p.set_portkey_friend(true);
    p.notice_portkeys(&one);
    assert!(p.start_portkey(&w));
    run(&mut p, &one);
    assert!(p.is_away());
    assert_eq!(p.take_portkey(), Some(PortkeyEvent::AskFriend { key: PortkeyKey::of(ObjectKind::Can) }));
    let gone_at = p.body.x;
    p.portkey_return(&w);
    assert_eq!(p.take_speech(), Some(Speech::PortkeyHuh));
    assert!(!p.is_away() && (p.body.x - gone_at).abs() < 1.0 && p.standing.is_some(), "제자리");
    let two = [thing(1, ObjectKind::Can, 800.0), thing(2, ObjectKind::Can, 1500.0)];
    let mut asked = 0;
    for seed in 0..40 {
        let mut p = standing(500.0, 100 + seed, &w);
        p.set_portkey_friend(true);
        p.notice_portkeys(&two);
        assert!(p.start_portkey(&w));
        run(&mut p, &two);
        if p.take_portkey().is_some() {
            asked += 1;
            let State::Away = p.state else { panic!() };
            p.portkey_return(&w);
            assert_eq!(p.take_speech(), Some(Speech::PortkeyLand), "거절돼도 내 화면의 짝으로");
            let other = if (p.body.x - 800.0).abs() < (p.body.x - 1500.0).abs() { 800.0 } else { 1500.0 };
            assert!((p.body.x - other).abs() < 1.5 * p.cfg.size);
        }
    }
    assert!((10..=30).contains(&asked), "반쯤: {asked}/40");
}

/// 🧳 같은 포트키 곁의 한가한 마법사 (쉬는 시간이어도)는 같이 잡고 같은 짝 옆에 나란히 나타남. 먼 마법사는 그대로
#[test]
fn wizards_by_the_same_portkey_go_together() {
    let w = world(vec![]);
    let spots = [thing(1, ObjectKind::Milk, 700.0), thing(2, ObjectKind::Milk, 1600.0)];
    let a = standing(640.0, 1, &w);
    let mut b = standing(760.0, 2, &w);
    b.portkey.ready_at = f64::INFINITY;
    let mut far = standing(1300.0, 3, &w);
    far.portkey.ready_at = f64::INFINITY;
    let mut pets = vec![a, b, far];
    for p in pets.iter_mut() {
        p.notice_portkeys(&spots);
    }
    // 1번 우유팩으로 가게 (가까운 것만 보이게 고른 뒤 다시 다 보임)
    pets[0].notice_portkeys(&spots[..1]);
    pets[0].set_portkey_friend(true);
    assert!(pets[0].start_portkey(&w));
    pets[0].set_portkey_friend(false);
    let mut landed = Vec::new();
    for _ in 0..(8.0 / DT) as usize {
        tick(&mut pets, &spots, &w);
        for p in pets.iter_mut() {
            if p.take_speech() == Some(Speech::PortkeyLand) {
                landed.push((p.social_key, p.body.x));
            }
        }
    }
    assert_eq!(landed.len(), 2, "{landed:?}");
    assert!(landed.iter().all(|&(k, x)| k != 3 && (x - 1600.0).abs() < 2.0 * pets[0].cfg.size), "{landed:?}");
    assert!((landed[0].1 - landed[1].1).abs() > 10.0, "나란히: {landed:?}");
    assert!((pets[2].body.x - 1300.0).abs() < 1.0, "먼 마법사는 그대로");
}

/// 🧳 친구 화면에서 포트키로 온 손님: 그 화면의 같은 물건 옆에 빙글 커지며 나타나 "whoa!"
#[test]
fn a_guest_arrives_next_to_the_matching_object() {
    let w = world(vec![]);
    let mut g = standing(100.0, 5, &w);
    let s = thing(9, ObjectKind::Umbrella, 1200.0);
    g.portkey_arrive(&s.spot, &w, 0);
    assert_eq!(g.take_speech(), Some(Speech::PortkeyLand));
    assert!(g.portkeying() && (g.body.x - 1200.0).abs() < 1.5 * g.cfg.size && g.standing.is_some());
    assert!(g.render().tuck > 0.9, "처음엔 안 보이다가");
    for _ in 0..(pk::APPEAR_SECS / DT) as usize + 2 {
        g.update(DT, &w);
    }
    assert!(!g.portkeying() && g.render().tuck == 0.0);
}

/// 🧳 말풍선 이름 (웹뷰 EXTRA_SPEECH·pet.json과 같음)
#[test]
fn portkey_lines_have_their_sheet_names() {
    for (s, name) in [(Speech::Portkey, "portkey"), (Speech::PortkeyLand, "portkey_land"), (Speech::PortkeyHuh, "portkey_huh")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}
