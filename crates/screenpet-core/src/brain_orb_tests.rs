//! 🕷️ 거미인간 (brain_web.rs): 🕸️ 아무 데나 치는 둥근 거미집 (spec 6부) · 💪 거대한 것도 끌어오고 휘둘러 던짐 (spec 7부)

use super::snare_moves::StuckKind;
use super::swing_tests::resting;
use super::tests::world;
use super::web_moves::{WebAim, WebPhase};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::juggle::{JuggleEvent, ObjectSpot};
use crate::play::ObjectPlay;
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};
use crate::web::{self, Corner, WebSpin, WebSpot};

/// 번들 pet.json 그대로의 거미인간
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

/// 바닥(x)에 쉬는 거미인간 — 주사위 놀이는 모두 꺼 둠 (start_*로만)
fn calm_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(spider(), x, seed, w);
    p.hush_stage();
    p.hush_web_play();
    p
}

/// 거미집을 치기 시작한 자리 (Shoot 중인 거미집 자리)
fn spinning(p: &Pet) -> Option<WebSpin> {
    match p.state {
        State::Web { phase: WebPhase::Shoot { aim: WebAim::Corner(s), .. }, .. } => Some(s),
        _ => None,
    }
}

/// 둥근 거미집 자리 (앱이 알려 주는 모양)
fn orb_spot(id: u64, x: f64, y: f64, r: f64) -> WebSpot {
    WebSpot { id, x, y, corner: Corner::Center, size: r, hidden: false }
}

/// 🕸️ 거미집을 칠 때 가끔 (15–25%) 구석 대신 손이 닿는 아무 자리에 둥근 거미집 — 화면 안에 다 들고, 반지름 몸 1.2–2.5배
#[test]
fn sometimes_a_spider_spins_a_round_web_anywhere_within_reach_instead_of_in_a_corner() {
    let w = world(vec![]);
    let (n, mut orbs) = (400, Vec::new());
    for seed in 0..n {
        let mut p = calm_spider(900.0, seed, &w);
        assert!(p.start_web_spin(&w));
        let s = spinning(&p).expect("줄을 쏨");
        if s.corner == Corner::Center {
            orbs.push((s, p.cfg.size, p.body));
        }
    }
    let share = orbs.len() as f64 / f64::from(n as u32);
    assert!((0.15..=0.25).contains(&share), "{share}");
    let mut spots = std::collections::HashSet::new();
    for (s, size, body) in &orbs {
        assert_eq!(s.window, None);
        assert_eq!(web::orb_fit(&w, (body.x, body.y - 0.6 * size), s.x, s.y, s.size), Some(None), "화면 안에 다 들어감 {s:?}");
        assert!((web::ORB_RADIUS_LOGICAL.0 - 1e-9..=web::ORB_RADIUS_LOGICAL.1 + 1e-9).contains(&s.size), "몸집과 상관없는 반지름 {}", s.size);
        assert!((s.x - body.x).hypot(s.y - (body.y - 0.6 * size)) <= web::orb_reach(*size, s.size) + 1e-6, "손이 닿는 거리 {s:?}");
        assert!(s.shots >= 2 && s.build > 0.0, "줄을 여러 번 쏴서 짬");
        spots.insert(((s.x / 50.0) as i64, (s.y / 50.0) as i64));
    }
    assert!(spots.len() > orbs.len() / 2, "정가운데만이 아니라 여기저기");
}

/// 🕸️ 한 화면에 둥근 거미집이 이미 3개면 더 안 침 (구석 거미집은 그대로)
#[test]
fn with_three_round_webs_on_the_screen_it_spins_only_corner_webs() {
    let w = world(vec![]);
    let three = [orb_spot(1, 300.0, 300.0, 150.0), orb_spot(2, 900.0, 300.0, 150.0), orb_spot(3, 1500.0, 300.0, 150.0)];
    let two = &three[..2];
    let (mut with_three, mut with_two) = (0, 0);
    for seed in 0..200 {
        let mut p = calm_spider(900.0, seed, &w);
        p.set_webs(&three);
        assert!(p.start_web_spin(&w));
        with_three += usize::from(spinning(&p).unwrap().corner == Corner::Center);
        let mut p = calm_spider(900.0, seed, &w);
        p.set_webs(two);
        assert!(p.start_web_spin(&w));
        with_two += usize::from(spinning(&p).unwrap().corner == Corner::Center);
    }
    assert_eq!(with_three, 0);
    assert!(with_two > 10, "{with_two}");
}

/// 🕸️ 큰 창 앞이면 둥근 거미집이 그 창 위에 (창 안에 다 들고 앞 창에 안 가려짐) — 창을 따라가게 창 id를 붙임
#[test]
fn in_front_of_a_big_window_the_round_web_hangs_on_the_window_face() {
    let w = world(vec![WindowInfo { id: 5, rect: Rect::new(100.0, 40.0, 1800.0, 1000.0) }]);
    let mut on_window = 0;
    for seed in 0..300 {
        let mut p = calm_spider(900.0, seed, &w);
        assert!(p.start_web_spin(&w));
        let s = spinning(&p).unwrap();
        if s.corner != Corner::Center {
            continue;
        }
        assert_eq!(s.window, Some(5), "{s:?}");
        assert!(s.x - s.size >= 100.0 && s.x + s.size <= 1800.0 && s.y - s.size >= 40.0 && s.y + s.size <= 1000.0, "{s:?}");
        on_window += 1;
    }
    assert!(on_window > 20, "{on_window}");
}

/// 둥근 거미집을 치는 거미인간 (시드를 돌며 찾음)
fn orb_spider(w: &World) -> Pet {
    for seed in 0..400 {
        let mut p = calm_spider(900.0, seed, w);
        assert!(p.start_web_spin(w));
        if spinning(&p).unwrap().corner == Corner::Center {
            return p;
        }
    }
    panic!("400 시드 동안 둥근 거미집이 없음");
}

/// 🕸️ 둥근 거미집도 줄을 여러 번 쏴서 짬 — 가운데에 먼저, 그 뒤 사방의 살 끝 → 고리. 거미집은 한 번 핌
#[test]
fn a_round_web_is_woven_shot_by_shot_all_around() {
    let w = world(vec![]);
    let mut p = orb_spider(&w);
    let mut tips: Vec<(f64, f64)> = Vec::new();
    let mut spun = Vec::new();
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        if let Some(l) = p.web_line() {
            if tips.last().is_none_or(|q| (q.0 - l.to.0).hypot(q.1 - l.to.1) > 1.0) {
                tips.push(l.to);
            }
        }
        p.update(DT, &w);
        spun.extend(p.take_web());
    }
    assert_eq!(spun.len(), 1, "거미집은 한 번");
    let s = spun[0];
    assert_eq!(s.corner, Corner::Center);
    let aims: Vec<(f64, f64)> = (1..s.shots).map(|k| web::weave_target(&s, k)).collect();
    assert!(aims.iter().all(|a| (a.0 - s.x).hypot(a.1 - s.y) <= s.size + 1e-9));
    assert!(aims.iter().any(|a| a.0 < s.x - 0.5 * s.size) && aims.iter().any(|a| a.0 > s.x + 0.5 * s.size), "양쪽으로 {aims:?}");
    for a in &aims {
        assert!(tips.iter().any(|q| (q.0 - a.0).hypot(q.1 - a.1) < 1.0), "줄 끝이 그 자리에 닿음 {a:?}");
    }
}

/// 🕸️ 둥근 거미집에도 동물이 걸림 (원 어디로 들어가든)
#[test]
fn a_pet_walking_through_a_round_web_gets_snared() {
    let w = world(vec![]);
    let size = pet_cfg().size;
    let r = 1.5 * size;
    let webs = [orb_spot(4, 900.0, 1040.0 - r - 2.0, r)];
    for dir in [1.0, -1.0] {
        let mut p = resting(pet_cfg(), 900.0 - dir * 5.0 * size, 3, &w);
        p.snare.chance = 1.0;
        p.set_webs(&webs);
        p.facing = dir;
        p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
        let mut guard = LoopLimit::new();
        while !matches!(p.state, State::Stuck { kind: StuckKind::Snare { .. }, .. }) {
            guard.step();
            p.update(DT, &w);
            if !matches!(p.state, State::Move { .. } | State::Stuck { .. }) {
                p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
                p.facing = dir;
            }
        }
        assert!((p.body.x - 900.0).abs() <= r + p.cfg.half_w, "{dir} {}", p.body.x);
    }
}

/// 🕸️🧗 둥근 거미집에도 올라가 기어 다니다 내려옴 — 거미집은 그대로
#[test]
fn a_spider_climbs_and_crawls_on_a_round_web() {
    let w = world(vec![]);
    let size = spider().size;
    let webs = [orb_spot(6, 1200.0, 600.0, 2.0 * size)];
    let mut p = calm_spider(900.0, 2, &w);
    p.set_webs(&webs);
    assert!(p.start_web_play(&w));
    let (mut on, mut climb) = (0, false);
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) || p.standing.is_none() {
        guard.step();
        p.set_webs(&webs);
        p.update(DT, &w);
        assert!(p.take_torn_web().is_none());
        // 처음 바닥에서 기어오르는 동안 말고는 (다음 자리로 기어갈 때·매달렸을 때) 몸 가운데가 원 안
        let c = (p.body.x, p.body.y - 0.5 * size);
        let inside = (c.0 - 1200.0).hypot(c.1 - 600.0) <= 2.0 * size + 1e-6;
        if let State::Web { phase: WebPhase::Dangle { .. } | WebPhase::Crawl { .. }, .. } = p.state {
            on += usize::from(inside);
            if matches!(p.state, State::Web { phase: WebPhase::Dangle { .. }, .. }) {
                assert!(inside, "매달린 자리 {c:?}");
            }
        }
        climb |= p.render().anim == Anim::Climb;
    }
    assert!(on as f64 * DT > 1.0 && climb);
}

/// 바닥의 거대한 물건 (반 폭 half 몸 크기 배)
fn giant_at(p: &Pet, dx: f64, half: f64) -> ObjectSpot {
    ObjectSpot { id: 99, x: p.body.x + dx, y: 1040.0, owner: Owner::Ground(0), half_w: half * p.cfg.size, giant: true, local: false }
}

/// 💪 거대한 것도 줄로 끌어옴 — 무거워서 조금만 (몸 GIANT_PULL_MAX배까지) 끌려오고, 머리 너머로는 못 넘김 (늘 그 쪽 앞에)
#[test]
fn a_spider_also_pulls_giant_things_but_only_a_little_way() {
    let w = world(vec![]);
    let mut pulled = 0;
    for seed in 0..30 {
        let mut p = calm_spider(700.0, seed, &w);
        let size = p.cfg.size;
        let g = giant_at(&p, 6.0 * size, 2.0);
        p.web.pull_ready_at = 0.0;
        let mut yank = None;
        for _ in 0..(120.0 / DT) as usize {
            p.notice_play_objects(&[g], &w);
            p.update(DT, &w);
            if let Some(ObjectPlay::Yank { id, vx, vy }) = p.take_object_play() {
                yank = Some((id, vx, vy));
                break;
            }
        }
        let Some((id, vx, _)) = yank else { continue };
        pulled += 1;
        assert_eq!(id, 99);
        assert!(vx < 0.0, "거미인간 쪽으로 {vx}");
        let travel = vx.abs() * web::GIANT_YANK_FLIGHT_SECS;
        assert!(travel <= web::GIANT_PULL_MAX * size + 1e-6, "조금만 {}", travel / size);
        assert!(g.x - travel - g.half_w > p.body.x, "거미인간을 덮치거나 넘어가지 않음");
    }
    assert!(pulled >= 20, "거대한 것도 끌어옴 {pulled}");
}

/// 💪 거대한 것을 휘두르기: 천천히 (whirl_secs(true)) 돌리다 던지는데, 먼 친구 끝까지는 못 가고 그 앞 바닥에 떨어짐
#[test]
fn a_whirled_giant_turns_slowly_and_falls_short_of_a_far_friend_edge() {
    let w = world(vec![]);
    let mut p = calm_spider(700.0, 4, &w);
    let size = p.cfg.size;
    let g = giant_at(&p, -4.0 * size, 2.0);
    p.notice_play_objects(&[g], &w);
    assert!(p.start_whirl_object(&g, Side::Right));
    let (secs, release, last) = whirl_out(&mut p, &w);
    assert!((secs - web::whirl_secs(true)).abs() < 0.1, "{secs}");
    let JuggleEvent::Release { id, vx, vy } = release else { panic!() };
    assert_eq!(id, 99);
    assert!(vx > 0.0);
    let (x, _) = crate::juggle::descent_x(last.0, last.1, vx, vy, 1040.0, p.cfg.dpi).unwrap();
    assert!(x < 1920.0 - size && x > p.body.x, "친구 끝까지 못 가고 앞 바닥에 {x}");
    assert!(x - p.body.x <= (web::GIANT_THROW_REACH + 0.5) * size, "{}", (x - p.body.x) / size);
}

/// 💪 친구 끝이 가까우면 거대한 것도 그 끝 너머로 (친구에게 감)
#[test]
fn a_whirled_giant_still_reaches_a_near_friend_edge() {
    let w = world(vec![]);
    let mut p = calm_spider(1920.0 - 3.0 * spider().size, 4, &w);
    let size = p.cfg.size;
    let g = giant_at(&p, -4.0 * size, 2.0);
    p.notice_play_objects(&[g], &w);
    assert!(p.start_whirl_object(&g, Side::Right));
    let (_, release, last) = whirl_out(&mut p, &w);
    let JuggleEvent::Release { vx, vy, .. } = release else { panic!() };
    let (x, _) = crate::juggle::descent_x(last.0, last.1, vx, vy, 1040.0, p.cfg.dpi).unwrap();
    assert!(x > 1920.0, "끝 너머로 {x}");
}

/// 휘두르기를 끝까지: (휘두른 시간, 놓은 일, 마지막 들고 있던 자리)
fn whirl_out(p: &mut Pet, w: &World) -> (f64, JuggleEvent, (f64, f64)) {
    let (mut secs, mut releases, mut last) = (0.0, Vec::new(), None);
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        p.update(DT, w);
        releases.extend(p.take_juggle());
        if let State::Web { phase: WebPhase::Whirl { .. }, t } = p.state {
            secs = t;
        }
        if let Some(at) = p.carry_point() {
            last = Some(at);
        }
    }
    assert_eq!(releases.len(), 1, "{releases:?}");
    (secs, releases[0], last.expect("휘둘렀음"))
}
