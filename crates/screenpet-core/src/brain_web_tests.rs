//! 🕷️ 거미인간 줄 놀이 (brain_web.rs): 거미집·그네·영웅 착지·끌어오기

use super::hoist_moves::{notice_pets, sync_lifts};
use super::swing_tests::resting;
use super::tests::world;
use super::web_moves::WebPhase;
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::juggle::ObjectSpot;
use crate::play::ObjectPlay;
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};
use crate::web::{self, Corner};

/// 번들 pet.json 그대로의 거미인간 (청소년, 손 자리 grip 포함)
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

/// 바닥(x)에 쉬는 거미인간 — 주사위 놀이는 꺼 둠 (start_*로만)
fn calm_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(spider(), x, seed, w);
    p.web.web_ready_at = f64::INFINITY;
    p.web.swing_ready_at = f64::INFINITY;
    p.web.pull_ready_at = f64::INFINITY;
    p
}

#[test]
fn web_animations_are_optional_side_views_with_their_sheet_names() {
    for (a, name) in [(Anim::Shoot, "shoot"), (Anim::WebSwing, "web_swing"), (Anim::HeroLand, "hero_land"), (Anim::WebPull, "web_pull")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(!a.is_front() && !a.is_back());
        assert!(crate::manifest::WEB_ANIMS.contains(&name));
    }
    for (s, name) in [(Speech::Thwip, "thwip"), (Speech::Web, "web"), (Speech::WebSwing, "web_swing"), (Speech::HeroLand, "hero_land")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    assert!(spider().slings_webs && !pet_cfg().slings_webs);
}

#[test]
fn it_shoots_a_line_at_a_corner_and_the_web_blooms_once_when_the_line_lands() {
    let w = world(vec![WindowInfo { id: 7, rect: Rect::new(700.0, 500.0, 1300.0, 900.0) }]);
    let mut p = calm_spider(900.0, 1, &w);
    assert!(p.start_web_spin(&w));
    assert_eq!(p.render().anim, Anim::Shoot);
    assert_eq!(p.speech, Some(Speech::Thwip));
    let mut lens = Vec::new();
    let mut spun = Vec::new();
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        let line = p.web_line().expect("쏘는 동안 줄이 보임");
        lens.push((line.to.0 - line.from.0).hypot(line.to.1 - line.from.1));
        p.update(DT, &w);
        spun.extend(p.take_web());
    }
    assert!(p.web_line().is_none(), "끝나면 줄도 없음");
    assert_eq!(spun.len(), 1, "거미집은 한 번");
    let s = spun[0];
    assert!(lens.windows(2).take(10).all(|v| v[1] >= v[0] - 1e-6), "줄이 뻗어 나감 {lens:?}");
    assert!((s.x - 900.0).hypot(s.y - p.body.y) <= web::WEB_REACH * p.cfg.size + p.cfg.size);
    let on_window = s.window == Some(7) && s.y == 500.0 && (s.x == 700.0 || s.x == 1300.0);
    let on_screen = s.window.is_none() && (s.x == 0.0 || s.x == 1920.0) && (s.y == 0.0 || s.y == 1040.0);
    assert!(on_window || on_screen, "{s:?}");
    if s.window.is_some() {
        assert!(matches!(s.corner, Corner::TopLeft | Corner::TopRight));
    }
}

/// 손님일 때만 자주 — 집에서는 가끔 (쿨다운도 김)
#[test]
fn a_visiting_spider_spins_webs_here_and_there_much_more_often_than_at_home() {
    let w = world(vec![]);
    let count = |guest: bool| {
        let mut n = 0;
        for seed in 0..4 {
            let mut p = resting(spider(), 900.0, seed, &w);
            p.web.swing_ready_at = f64::INFINITY;
            p.web.pull_ready_at = f64::INFINITY;
            p.web.web_ready_at = 0.0;
            if guest {
                p.set_guest(Side::Left, 1e9);
            }
            for _ in 0..(60 * 240) {
                p.update(DT, &w);
                n += usize::from(p.take_web().is_some());
            }
        }
        n
    };
    let (home, guest) = (count(false), count(true));
    assert!(guest >= 24 && guest > 2 * home.max(1), "home {home}, guest {guest}");
}

/// 집에서도 거미인간 티가 나게 — 10분이면 거미집 몇 번, 그네 여러 번 (걷기만 하지 않음)
#[test]
fn at_home_it_spins_webs_and_swings_often_enough_to_look_like_a_spider_hero() {
    let w = world(vec![WindowInfo { id: 7, rect: Rect::new(500.0, 300.0, 1400.0, 800.0) }]);
    let (mut webs, mut swings) = (0, 0);
    for seed in 0..4 {
        let mut p = resting(spider(), 900.0, seed, &w);
        p.web.pull_ready_at = f64::INFINITY;
        let mut was_swinging = false;
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            webs += usize::from(p.take_web().is_some());
            let swinging = p.render().anim == Anim::WebSwing;
            swings += usize::from(swinging && !was_swinging);
            was_swinging = swinging;
        }
    }
    assert!(webs >= 4 * 4 && swings >= 4 * 5, "webs {webs}, swings {swings} (4마리 × 10분)");
}

#[test]
fn it_swings_on_a_line_across_the_screen_without_touching_the_floor_and_lands_like_a_hero() {
    let w = world(vec![]);
    let mut p = calm_spider(500.0, 3, &w);
    p.facing = 1.0;
    assert!(p.start_web_swing(&w));
    let start_x = p.body.x;
    let mut swung = false;
    let mut lowest = f64::NEG_INFINITY;
    let mut hero = false;
    let mut guard = LoopLimit::new();
    while p.standing.is_none() || matches!(p.state, State::Web { .. }) || matches!(p.state, State::Air { .. }) {
        guard.step();
        p.update(DT, &w);
        if let State::Web { phase: WebPhase::Swing { anchor, .. }, .. } = p.state {
            swung = true;
            assert_eq!(p.render().anim, Anim::WebSwing);
            let line = p.web_line().unwrap();
            assert_eq!(line.to, (anchor.x, anchor.y), "줄은 닻까지");
            assert_eq!(anchor.y, 0.0, "창이 없으면 화면 윗변");
            lowest = lowest.max(p.body.y);
        }
        hero |= p.render().anim == Anim::HeroLand;
    }
    assert!(swung);
    assert!(lowest <= 1040.0 - web::SWING_CLEAR * p.cfg.size + 1.0, "진자 맨 아래도 바닥 위 {lowest}");
    assert!(p.body.x - start_x > 4.0 * p.cfg.size, "앞으로 멀리 {start_x} → {}", p.body.x);
    assert!(hero, "높이서 놓아 영웅 착지");
}

#[test]
fn a_long_fall_ends_in_a_hero_landing_for_the_spider_only() {
    let w = world(vec![]);
    for (cfg, want) in [(spider(), true), (pet_cfg(), false)] {
        let mut p = Pet::new(cfg, 800.0, 300.0, 9);
        p.clock = 100.0;
        p.state = State::Air { anim: Anim::Fall };
        p.standing = None;
        let mut guard = LoopLimit::new();
        while p.standing.is_none() {
            guard.step();
            p.update(DT, &w);
        }
        assert_eq!(p.render().anim == Anim::HeroLand, want);
        if want {
            assert_eq!(p.speech, Some(Speech::HeroLand));
            assert!(!p.render().thud, "콰당 대신");
        }
    }
    // 낮게 폴짝 내려서면 그냥 착지
    let mut p = Pet::new(spider(), 800.0, 1040.0 - 30.0, 9);
    p.state = State::Air { anim: Anim::Fall };
    let mut guard = LoopLimit::new();
    while p.standing.is_none() {
        guard.step();
        p.update(DT, &w);
    }
    assert_eq!(p.render().anim, Anim::Land);
}

#[test]
fn it_pulls_an_object_on_the_floor_with_a_line_so_it_flies_over() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 4, &w);
    let spot = ObjectSpot { id: 42, x: 800.0 + 4.0 * p.cfg.size, y: 1040.0, owner: Owner::Ground(0), half_w: 12.0, giant: false, local: false };
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_pull_object(&spot, &w));
    assert_eq!(p.facing, 1.0, "물건 쪽을 봄");
    let mut yank = None;
    let mut pulled = false;
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        p.update(DT, &w);
        pulled |= p.render().anim == Anim::WebPull;
        if let Some(ObjectPlay::Yank { id, vx, vy }) = p.take_object_play() {
            assert!(yank.is_none());
            yank = Some((id, vx, vy));
        }
    }
    let (id, vx, vy) = yank.expect("줄이 닿으면 끌어옴");
    assert!(pulled);
    assert_eq!(id, 42);
    assert!(vx < 0.0 && vy < 0.0, "거미 쪽 위로 휙 ({vx}, {vy})");
    let (x, _) = crate::juggle::descent_x(spot.x, spot.y, vx, vy, 1040.0, 1.0).unwrap();
    let ahead = (x - (800.0 + web::PULL_LAND_AHEAD * p.cfg.size)).abs() < 2.0;
    let behind = (x - (800.0 - web::FLING_LAND_BEHIND * p.cfg.size)).abs() < 2.0;
    assert!(ahead || behind, "앞에 떨어지거나 머리 너머 뒤로 {x}");
}

#[test]
fn it_yanks_a_nearby_pet_which_flies_over_and_lands() {
    let w = world(vec![]);
    let size = spider().size;
    let mut pets = vec![calm_spider(600.0, 5, &w), resting(pet_cfg(), 600.0 - 5.0 * size, 6, &w)];
    pets[1].state = State::Idle { remaining: 100.0, look: false };
    notice_pets(&mut pets);
    let target = pets[0].web.pets.iter().find(|s| s.key == pets[1].social_key).copied().expect("다른 동물을 봄");
    assert!(pets[0].start_pull_pet(&target, &w));
    assert_eq!(pets[0].facing, -1.0);
    let mut flew = false;
    let mut guard = LoopLimit::new();
    for _ in 0..(60 * 3) {
        guard.step();
        notice_pets(&mut pets);
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
        flew |= matches!(pets[1].state, State::Air { .. });
    }
    assert!(flew, "끌려 날아옴");
    assert!(pets[1].standing.is_some(), "내려섬");
    assert!((pets[1].body.x - 600.0).abs() < 3.0 * size, "거미 가까이 (앞 또는 머리 너머 뒤) {}", pets[1].body.x);
}

#[test]
fn left_alone_it_plays_with_lines_now_and_then_and_other_pets_never_do() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(600.0, 300.0, 1400.0, 800.0) }]);
    let mut kinds = std::collections::HashSet::new();
    for seed in 0..3 {
        let mut p = resting(spider(), 900.0, seed, &w);
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            let _ = p.take_web();
            if let State::Web { phase, .. } = p.state {
                kinds.insert(std::mem::discriminant(&phase));
            }
        }
    }
    assert!(kinds.len() >= 2, "거미집·그네 둘 다 나옴 {kinds:?}");
    let mut cat = resting(pet_cfg(), 900.0, 1, &w);
    for _ in 0..(60 * 300) {
        cat.update(DT, &w);
        assert!(cat.take_web().is_none() && cat.web_line().is_none());
        assert!(!matches!(cat.state, State::Web { .. }));
    }
}

/// 그네를 타던 중 사용자가 자리를 비우면 공중에서 잠들지 않고 떨어져 내려섬
#[test]
fn going_away_mid_swing_drops_it_instead_of_sleeping_in_the_air() {
    let w = world(vec![]);
    let mut p = calm_spider(500.0, 3, &w);
    p.facing = 1.0;
    assert!(p.start_web_swing(&w));
    let mut guard = LoopLimit::new();
    while !p.web_airborne() {
        guard.step();
        p.update(DT, &w);
    }
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    assert!(p.web_line().is_none());
}
