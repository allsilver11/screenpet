//! 🕷️ 거미인간 줄 놀이 (brain_web.rs): 거미집·그네·영웅 착지·끌어오기·🌀 휘둘러 던지기·🙃 거꾸로 매달리기

use super::hoist_moves::{notice_pets, sync_lifts, PetSpot};
use super::swing_tests::resting;
use super::tests::world;
use super::web_moves::{WebAim, WebPhase};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::juggle::{JuggleEvent, ObjectSpot};
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
    p.web.hang_ready_at = f64::INFINITY;
    // 🕸️⭕ 여기서는 구석 거미집만 봄 (둥근 거미집은 brain_orb_tests.rs)
    p.web.orb_odds = 0.0;
    p
}

#[test]
fn web_animations_are_optional_side_views_with_their_sheet_names() {
    for (a, name) in [(Anim::Shoot, "shoot"), (Anim::WebSwing, "web_swing"), (Anim::HeroLand, "hero_land"), (Anim::WebPull, "web_pull"), (Anim::WebWhirl, "web_whirl")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(!a.is_front() && !a.is_back());
        assert!(crate::manifest::WEB_ANIMS.contains(&name));
    }
    for (s, name) in [(Speech::Thwip, "thwip"), (Speech::Web, "web"), (Speech::WebSwing, "web_swing"), (Speech::HeroLand, "hero_land"), (Speech::Yeet, "yeet")] {
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

/// 🕸️ 거미집은 한 번에 휙 생기지 않고 줄을 여러 번 (휙휙휙) 쏴서 짬 — 줄 끝이 매번 다른 자리, 앱에는 짜는 시간과 횟수를 알림
#[test]
fn a_web_is_woven_with_several_shots_aimed_around_the_web() {
    let w = world(vec![]);
    let mut p = calm_spider(900.0, 1, &w);
    assert!(p.start_web_spin(&w));
    let mut tips: Vec<(f64, f64)> = Vec::new();
    let mut spun = Vec::new();
    let mut shoot_starts = 0;
    let mut was_t = f64::INFINITY;
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        if let State::Web { t, .. } = p.state {
            if t < was_t {
                shoot_starts += 1;
            }
            was_t = t;
        }
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
    assert!((web::WEAVE_SHOTS.0..=web::WEAVE_SHOTS.1).contains(&s.shots), "{}", s.shots);
    assert!((s.build - f64::from(s.shots - 1) * web::WEAVE_SHOT_SECS).abs() < 1e-9, "짜는 시간 {}", s.build);
    assert!(shoot_starts >= s.shots as usize, "쏠 때마다 쏘는 그림을 처음부터: {shoot_starts}");
    // 줄 끝이 거미집 여기저기 (모서리 안쪽 부채꼴 안)
    let (sx, sy) = match s.corner { Corner::TopLeft => (1.0, 1.0), Corner::TopRight => (-1.0, 1.0), Corner::BottomLeft => (1.0, -1.0), Corner::BottomRight => (-1.0, -1.0), Corner::Center => unreachable!() };
    let mut aims: Vec<(f64, f64)> = Vec::new();
    for k in 1..s.shots {
        let a = web::weave_target(&s, k);
        assert!((a.0 - s.x) * sx >= -1e-9 && (a.1 - s.y) * sy >= -1e-9, "안쪽 {a:?}");
        assert!((a.0 - s.x).hypot(a.1 - s.y) <= s.size + 1e-9);
        assert!(aims.iter().all(|q| (q.0 - a.0).hypot(q.1 - a.1) > 0.1 * s.size), "매번 다른 자리");
        aims.push(a);
    }
}

/// 🕸️ 거미집 크기는 몸집과 상관없이 제각각 — 작게도, 아주 크게도 (논리 px 범위 × 화면 배율). 아기도 큰 걸 침. 창 모서리는 창보다 크지 않게
#[test]
fn web_sizes_vary_from_small_to_big_whatever_the_body_size_and_fit_their_window() {
    let w = world(vec![]);
    for stage in [crate::care::Stage::Baby, crate::care::Stage::Teenager, crate::care::Stage::Adult] {
        let mut sizes = Vec::new();
        for seed in 0..60 {
            let mut p = calm_spider(900.0, seed, &w);
            p.cfg.stage = stage;
            let spot = web::corner_spots(&w, (0.0, 1040.0), 1e9, p.cfg.size)[0];
            let s = p.sized_web(spot, &w);
            sizes.push(s.size / p.cfg.dpi);
        }
        let (lo, hi) = sizes.iter().fold((f64::INFINITY, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
        assert!(lo >= web::WEB_RADIUS_LOGICAL.0 - 1e-9 && hi <= web::WEB_RADIUS_LOGICAL.1 + 1e-9, "{stage:?} {lo}..{hi}");
        assert!(lo < 120.0 && hi > 360.0, "{stage:?} 작게도 아주 크게도: {lo}..{hi}");
    }
    // 작은 창: 창의 짧은 변 WEB_WINDOW_FIT배를 넘지 않음
    let small = world(vec![WindowInfo { id: 7, rect: Rect::new(700.0, 500.0, 1100.0, 800.0) }]);
    for seed in 0..20 {
        let mut p = calm_spider(800.0, seed, &small);
        let spot = web::corner_spots(&small, (750.0, 700.0), 1e9, p.cfg.size).into_iter().find(|s| s.window == Some(7)).unwrap();
        let s = p.sized_web(spot, &small);
        assert!(s.size <= web::WEB_WINDOW_FIT * 300.0 + 1e-9, "{}", s.size);
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
            // 거미집 빈도만 잼 — 🌱 연령대 특기는 끔
            p.hush_stage();
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

/// 바닥(y 1040)의 물건 하나 (거미에서 dx만큼)
fn floor_object(p: &Pet, dx: f64) -> ObjectSpot {
    ObjectSpot { id: 42, x: p.body.x + dx, y: 1040.0, owner: Owner::Ground(0), half_w: 12.0, giant: false, local: false }
}

/// 🌀 친구가 있는 끝이 있으면 물건을 줄로 낚아 머리 위로 빙빙 돌리다가 그 끝 너머로 휙 (앱이 친구 화면으로 보냄)
#[test]
fn with_a_friend_edge_it_lassos_an_object_overhead_and_flings_it_past_that_screen_edge() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 4, &w);
    p.set_prank_edges(false, true);
    let spot = floor_object(&p, -4.0 * p.cfg.size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    assert_eq!(p.facing, -1.0, "쏠 때는 물건 쪽을 봄");
    assert!(p.carried_object().is_none(), "줄이 닿기 전엔 들고 있지 않음");
    let size = p.cfg.size;
    let (mut points, mut releases) = (Vec::new(), Vec::new());
    let (mut gotcha, mut whirl_secs, mut last) = (false, 0.0, None);
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        p.update(DT, &w);
        releases.extend(p.take_juggle());
        if let State::Web { phase: WebPhase::Whirl { .. }, t } = p.state {
            gotcha |= p.speech == Some(Speech::Gotcha);
            whirl_secs = t;
            assert_eq!(p.render().anim, Anim::WebWhirl);
            assert_eq!(p.facing, 1.0, "돌릴 때는 친구 쪽을 봄");
            assert_eq!(p.carried_object(), Some(42), "줄 끝에 매달려 있음");
            let at = p.carry_point().expect("앱이 옮길 자리");
            let line = p.web_line().expect("손에서 물건까지 줄");
            assert!((line.to.0 - at.0).abs() < 1e-6 && (line.to.1 + spot.half_w - at.1).abs() < 1e-6, "줄 끝 = 물건 가운데 {line:?} {at:?}");
            if t > web::WHIRL_REEL_SECS {
                points.push((at.0, at.1 - spot.half_w, line.from));
            }
            last = Some(at);
        }
    }
    assert!(gotcha, "낚아채면 gotcha");
    assert!((whirl_secs - web::WHIRL_SECS).abs() < 0.1, "{whirl_secs}");
    assert_eq!(p.speech, Some(Speech::Yeet));
    assert!(p.carried_object().is_none() && p.web_line().is_none(), "놓은 뒤엔 빈손");
    assert_eq!(releases.len(), 1, "{releases:?}");
    let JuggleEvent::Release { id, vx, vy } = releases[0] else { panic!("{releases:?}") };
    assert_eq!(id, 42);
    assert!(vx > 0.0, "친구 쪽 (오른쪽)으로 {vx}");
    // 머리 위에서 빙빙: 손 위 높이에서 두 바퀴 넘게, 몸 크기만 한 원
    let n = points.len() as f64;
    let (cx, cy) = (points.iter().map(|q| q.0).sum::<f64>() / n, points.iter().map(|q| q.1).sum::<f64>() / n);
    let mut turned = 0.0;
    for v in points.windows(2) {
        let (a0, a1) = ((v[0].1 - cy).atan2(v[0].0 - cx), (v[1].1 - cy).atan2(v[1].0 - cx));
        let mut d = a1 - a0;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        turned += d;
    }
    assert!(turned.abs() >= 2.0 * std::f64::consts::TAU - 0.3 && turned.abs() <= 3.5 * std::f64::consts::TAU, "{turned}");
    let spread = points.iter().map(|q| q.0).fold(f64::NEG_INFINITY, f64::max) - points.iter().map(|q| q.0).fold(f64::INFINITY, f64::min);
    assert!(spread > 1.5 * size, "넓게 돎 {spread}");
    assert!(points.iter().all(|&(_, y, hand)| y < hand.1 + 0.3 * size), "손보다 위에서 돎");
    assert!(points.iter().all(|&(_, y, _)| y < p.body.y - 0.75 * size), "머리 위 {points:?}");
    // 놓은 자리에서 포물선으로 날아 화면 오른쪽 끝을 바닥보다 위로 넘어감
    let (x0, y0) = last.unwrap();
    let (x, _) = crate::juggle::descent_x(x0, y0, vx, vy, 1040.0, p.cfg.dpi).unwrap();
    assert!(x > 1920.0 + size, "끝 너머로 {x}");
    let t_edge = (1920.0 - x0) / vx;
    let g = crate::physics::THROW_GRAVITY_LOGICAL * p.cfg.dpi;
    assert!(y0 + vy * t_edge + 0.5 * g * t_edge * t_edge < 1040.0 - size, "바닥에 닿기 전에 끝을 넘음");
}

/// 친구 끝이 없으면 언제나 옛 끌어오기, 있으면 자주 휘둘러 던지기 (그 끝 쪽으로)
#[test]
fn it_only_whirls_objects_toward_friend_edges_and_often_when_there_is_one() {
    let w = world(vec![]);
    let n = 400;
    let mut whirls = 0;
    for seed in 0..n {
        let mut p = calm_spider(800.0, seed, &w);
        let spot = floor_object(&p, 4.0 * p.cfg.size);
        assert!(p.start_pull_object(&spot, &w));
        assert!(matches!(p.state, State::Web { phase: WebPhase::Shoot { aim: WebAim::Object { .. }, .. }, .. }), "{:?}", p.state);
        let mut p = calm_spider(800.0, seed, &w);
        p.set_prank_edges(true, false);
        assert!(p.start_pull_object(&spot, &w));
        match p.state {
            State::Web { phase: WebPhase::Shoot { aim: WebAim::Lasso { side, .. }, .. }, .. } => {
                assert_eq!(side, Side::Left);
                whirls += 1;
            }
            State::Web { phase: WebPhase::Shoot { aim: WebAim::Object { .. }, .. }, .. } => {}
            s => panic!("{s:?}"),
        }
    }
    let share = whirls as f64 / n as f64;
    assert!((share - web::WHIRL_CHANCE).abs() < 0.08, "{share}");
}

/// 🌀 옆에 다른 동물 (친구 펫 포함)이 있으면 휘두른 물건을 그 동물에게 던지기도 함 — 친구 끝이 없어도
#[test]
fn it_also_whirls_objects_at_a_nearby_pet_and_the_throw_comes_down_on_it() {
    let w = world(vec![]);
    let (mut at_pet, mut n) = (0, 0);
    for seed in 0..200 {
        let mut p = calm_spider(800.0, seed, &w);
        let size = p.cfg.size;
        let target = PetSpot { key: 77, x: 800.0 - 6.0 * size, y: 1040.0, owner: Some(Owner::Ground(0)), size, liftable: true, guest: true, carrying: false, reading: false };
        p.web.pets = vec![target];
        let spot = floor_object(&p, 3.0 * size);
        p.notice_play_objects(&[spot], &w);
        assert!(p.start_pull_object(&spot, &w));
        n += 1;
        if !matches!(p.state, State::Web { phase: WebPhase::Shoot { aim: WebAim::Lasso { .. }, .. }, .. }) {
            continue;
        }
        at_pet += 1;
        let (mut releases, mut last) = (Vec::new(), None);
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Web { .. }) {
            guard.step();
            p.update(DT, &w);
            releases.extend(p.take_juggle());
            if let Some(at) = p.carry_point() {
                last = Some(at);
            }
        }
        let JuggleEvent::Release { vx, vy, .. } = releases[0] else { panic!("{releases:?}") };
        assert!(vx < 0.0, "동물 쪽 (왼쪽)으로 {vx}");
        let (x0, y0) = last.unwrap();
        let (x, _) = crate::juggle::descent_x(x0, y0, vx, vy, 1040.0 - 0.5 * size, p.cfg.dpi).unwrap();
        assert!((x - target.x).abs() < 0.6 * size, "동물에게 떨어짐 {x} vs {}", target.x);
    }
    let share = at_pet as f64 / n as f64;
    assert!(share > 0.5, "자주 던짐 {share}");
}

/// 🕷️ 친구 펫이 옆에 있으면 줄 장난 (끌어오기·고치·휘둘러 던지기)을 꽤 자주 함 — 5분에 여러 번
#[test]
fn a_spider_with_a_pet_nearby_plays_web_pranks_often() {
    let w = world(vec![]);
    let mut total = 0;
    for seed in 0..4 {
        let mut p = resting(spider(), 800.0, seed, &w);
        p.web.web_ready_at = f64::INFINITY;
        p.web.swing_ready_at = f64::INFINITY;
        let size = p.cfg.size;
        let mut was = false;
        for _ in 0..(60 * 300) {
            p.web.pets = vec![PetSpot { key: 77, x: (p.body.x - 4.0 * size).max(100.0), y: 1040.0, owner: Some(Owner::Ground(0)), size, liftable: true, guest: true, carrying: false, reading: false }];
            p.update(DT, &w);
            let _ = p.take_juggle();
            let _ = p.take_web();
            p.web.yank_pet = None;
            let pranking = matches!(p.state, State::Web { phase: WebPhase::Shoot { aim: WebAim::Pet { .. } | WebAim::Lasso { .. }, .. } | WebPhase::Wrap { .. }, .. });
            total += usize::from(pranking && !was);
            was = pranking;
        }
    }
    assert!(total >= 4 * 6, "5분에 6번 넘게: {total} (4마리)");
}

/// 줄이 닿기 전에 물건이 바닥에서 사라졌으면 (누가 집었거나 치움) 낚지 않음
#[test]
fn a_lasso_that_lands_after_the_object_is_gone_catches_nothing() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 4, &w);
    p.set_prank_edges(false, true);
    let spot = floor_object(&p, 4.0 * p.cfg.size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    p.notice_play_objects(&[], &w);
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) {
        guard.step();
        p.update(DT, &w);
        assert!(p.carried_object().is_none() && p.take_juggle().is_none());
        assert!(!matches!(p.state, State::Web { phase: WebPhase::Whirl { .. }, .. }));
    }
}

/// 돌리던 물건을 사용자가 낚아채면 "hmph!" 하고 그만 (놓는 일은 없음)
#[test]
fn snatching_the_object_mid_whirl_ends_it_with_a_hmph() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 4, &w);
    p.set_prank_edges(false, true);
    let spot = floor_object(&p, 4.0 * p.cfg.size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    let mut guard = LoopLimit::new();
    while !matches!(p.state, State::Web { phase: WebPhase::Whirl { .. }, t } if t > 0.5) {
        guard.step();
        p.update(DT, &w);
    }
    p.lose_carry();
    assert_eq!(p.speech, Some(Speech::Hmph));
    assert!(!matches!(p.state, State::Web { .. }) && p.carried_object().is_none() && p.web_line().is_none());
    for _ in 0..120 {
        p.update(DT, &w);
        assert!(p.take_juggle().is_none());
    }
}

/// 돌리던 중 다른 일이 끼어들면 줄을 놓음 — 들고 있지 않으니 앱이 물건을 떨어뜨림
#[test]
fn an_interruption_mid_whirl_lets_the_object_drop() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 4, &w);
    p.set_prank_edges(false, true);
    let spot = floor_object(&p, 4.0 * p.cfg.size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    let mut guard = LoopLimit::new();
    while !matches!(p.state, State::Web { phase: WebPhase::Whirl { .. }, t } if t > 0.5) {
        guard.step();
        p.update(DT, &w);
    }
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(!matches!(p.state, State::Web { .. }), "{:?}", p.state);
    assert!(p.carried_object().is_none() && p.carry_point().is_none());
    assert!(p.take_juggle().is_none(), "던지지 않음");
}

/// 🌉🌉 줄 잇기 한 판을 끝까지 돌림: (그네마다 닻, 영웅 착지 했나). 매 틱 화면·모니터 안, 그네 중엔 발이 바닥 위인지 봄
fn ride_chain(p: &mut Pet, w: &World) -> (Vec<web::Anchor>, bool) {
    let wa = w.work_area_at(p.body.x, p.body.y - 1.0);
    let floor = wa.bottom - web::SWING_CLEAR * p.cfg.size;
    let mut anchors: Vec<web::Anchor> = Vec::new();
    let mut hero = false;
    let mut guard = LoopLimit::new();
    while p.standing.is_none() || matches!(p.state, State::Web { .. } | State::Air { .. }) {
        guard.step();
        let before = p.body;
        p.update(DT, w);
        assert!(wa.left <= p.body.x && p.body.x <= wa.right && p.body.y <= wa.bottom + 1.0, "모니터 안 {:?}", p.body);
        assert!((p.body.x - before.x).hypot(p.body.y - before.y) < 0.6 * p.cfg.size, "순간이동 없음 {before:?} → {:?}", p.body);
        if let State::Web { phase: WebPhase::Swing { anchor, .. }, .. } = p.state {
            assert!(p.body.y <= floor + 1.0, "그네 중 발은 바닥 위 {}", p.body.y);
            assert!(anchor.len <= web::SWING_MAX_LEN * p.cfg.size + 1e-6);
            assert!(wa.left <= anchor.x && anchor.x <= wa.right && wa.top <= anchor.y, "닻도 같은 모니터 {anchor:?}");
            assert_eq!(p.web_line().unwrap().to, (anchor.x, anchor.y));
            if anchors.last() != Some(&anchor) {
                anchors.push(anchor);
            }
        }
        hero |= p.render().anim == Anim::HeroLand;
    }
    (anchors, hero)
}

/// 🌉🌉 거미인간처럼 줄을 이어 탐: 놓고 날아가다 공중에서 앞 위로 새 줄을 쏴 또 흔들림 — 닻은 갈수록 앞, 끝엔 영웅 착지
#[test]
fn with_room_ahead_it_chains_swings_mid_air_moving_forward_and_lands_like_a_hero() {
    let w = world(vec![]);
    for seed in 0..6 {
        let mut p = calm_spider(150.0, seed, &w);
        p.facing = 1.0;
        assert!(p.start_web_swing(&w));
        let (anchors, hero) = ride_chain(&mut p, &w);
        assert!(anchors.len() >= 2, "seed {seed}: 줄을 이어 탐 {anchors:?}");
        assert!(anchors.windows(2).all(|a| a[1].x > a[0].x + p.cfg.size), "seed {seed}: 닻이 앞으로 {anchors:?}");
        assert!(hero, "seed {seed}: 끝엔 영웅 착지");
        assert!(p.standing.is_some());
    }
}

/// 🌉🌉 넓은 화면에서 이어 타는 그네 수는 CHAIN_SWINGS 안에서 제각각
#[test]
fn the_number_of_chained_swings_stays_in_range_and_varies() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 9000.0, 1040.0)], 128.0);
    let mut counts = std::collections::BTreeSet::new();
    for seed in 0..16 {
        let mut p = calm_spider(300.0, seed, &w);
        p.facing = 1.0;
        assert!(p.start_web_swing(&w));
        let (anchors, hero) = ride_chain(&mut p, &w);
        let n = anchors.len() as u32;
        assert!((web::CHAIN_SWINGS.0..=web::CHAIN_SWINGS.1).contains(&n), "seed {seed}: {n}번");
        assert!(hero);
        counts.insert(n);
    }
    assert!(counts.len() >= 2, "횟수가 제각각 {counts:?}");
}

/// 🌉🌉 앞에 닻 댈 자리가 없으면 (화면 끝) 더 잇지 않고 그냥 떨어져 내려섬
#[test]
fn it_stops_chaining_when_there_is_no_anchor_ahead() {
    let w = world(vec![]);
    let mut p = calm_spider(800.0, 2, &w);
    p.facing = 1.0;
    assert!(p.start_web_swing(&w));
    p.web.chain_left = 4;
    let (anchors, _) = ride_chain(&mut p, &w);
    assert_eq!(anchors.len(), 1, "화면 끝이라 한 번만 {anchors:?}");
    assert!(p.standing.is_some() && p.web.chain_left == 0);
}

// ---- 🙃 거꾸로 매달려 내려오기 (spec 8부) ----

/// 🙃 매달리기 주사위도 끈 거미인간 (start_hang_down으로만)
fn hang_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = calm_spider(x, seed, w);
    p.hush_web_play();
    p
}

/// 🙃 한 번 매달려 내려왔다 끝날 때까지 돌림: (닻들, 매달린 동안 머리 높이 (바닥에서, 몸 크기 배), 매달린 동안 x 폭, 다시 올라갔음, 영웅 착지, 한 말)
struct HangRun {
    anchors: Vec<(f64, f64)>,
    head_clear: Vec<f64>,
    sway: (f64, f64),
    climbed: bool,
    hero: bool,
    said: Vec<Speech>,
    hung_secs: f64,
}

fn run_hang(p: &mut Pet, w: &World) -> HangRun {
    let size = p.cfg.size;
    let floor = w.work_area_at(p.body.x, p.body.y - 1.0).bottom;
    let mut r = HangRun { anchors: Vec::new(), head_clear: Vec::new(), sway: (f64::INFINITY, f64::NEG_INFINITY), climbed: false, hero: false, said: Vec::new(), hung_secs: 0.0 };
    let mut guard = LoopLimit::new();
    while p.standing.is_none() || matches!(p.state, State::Web { .. } | State::Air { .. }) {
        guard.step();
        p.update(DT, w);
        r.said.extend(p.take_speech());
        r.hero |= p.render().anim == Anim::HeroLand;
        if let State::Web { phase: WebPhase::HangSlide { line, .. } | WebPhase::Hang { line, .. } | WebPhase::HangClimb { line, .. }, .. } = p.state {
            assert_eq!(p.render().anim, Anim::HangDown, "거꾸로 매달린 그림");
            let l = p.web_line().expect("매달린 동안 줄이 보임");
            assert_eq!(l.to, (line.x, line.y), "줄은 닻까지");
            assert!(l.to.1 <= l.from.1 + 1e-6, "줄은 위로 {l:?}");
            assert!((l.to.0 - l.from.0).abs() < 0.2 * size, "거의 똑바로 선 줄 {l:?}");
            let (_, top) = p.window_pos();
            assert!(l.from.1 < top + 0.3 * size, "줄은 몸 위쪽 (손발)에 붙음 {l:?}, top {top}");
            if r.anchors.last() != Some(&(line.x, line.y)) {
                r.anchors.push((line.x, line.y));
            }
            r.climbed |= matches!(p.state, State::Web { phase: WebPhase::HangClimb { .. }, .. });
        }
        if let State::Web { phase: WebPhase::Hang { .. }, .. } = p.state {
            r.head_clear.push((floor - p.body.y) / size);
            r.sway = (r.sway.0.min(p.body.x), r.sway.1.max(p.body.x));
            r.hung_secs += DT;
        }
    }
    r
}

#[test]
fn hang_down_is_a_web_animation_with_its_grip_at_the_top_and_its_own_speech() {
    assert_eq!(serde_json::to_string(&Anim::HangDown).unwrap(), "\"hang_down\"");
    assert!(!Anim::HangDown.is_front() && !Anim::HangDown.is_back());
    assert!(crate::manifest::WEB_ANIMS.contains(&"hang_down"));
    assert_eq!(serde_json::to_string(&Speech::HangHey).unwrap(), "\"hang_hey\"");
    let g = &spider().grip;
    assert!(g.hang_down.n >= 2, "칸마다 손발 자리");
    assert!((0..g.hang_down.n).all(|i| g.hang_down.pts[i].1 < 0.35), "손발 (줄 잡은 곳)은 그림 위쪽 {:?}", g.hang_down);
}

/// 🙃 머리 위 (화면 윗변)에 줄을 쏴 올라가 뒤집어 거꾸로 스르륵 내려옴 → 머리가 바닥에서 몸 1–2배 높이에서 살랑살랑 → 끝나면 내려섬
#[test]
fn it_shoots_up_flips_and_slides_down_upside_down_to_hang_above_the_floor() {
    let w = world(vec![]);
    for seed in 0..6 {
        let mut p = hang_spider(900.0, seed, &w);
        assert!(p.start_hang_down(&w), "seed {seed}");
        assert_eq!(p.render().anim, Anim::Shoot);
        let size = p.cfg.size;
        let r = run_hang(&mut p, &w);
        assert_eq!(r.anchors.len(), 1, "seed {seed}: {:?}", r.anchors);
        assert_eq!(r.anchors[0].1, 0.0, "창이 없으면 화면 윗변");
        assert!((r.anchors[0].0 - 900.0).abs() < size, "바로 머리 위 {:?}", r.anchors);
        assert!(!r.head_clear.is_empty(), "seed {seed}: 매달림");
        assert!(r.head_clear.iter().all(|h| (0.9..=2.1).contains(h)), "seed {seed}: 머리는 바닥에서 몸 1–2배 {:?}", r.head_clear);
        assert!((web::HANG_SECS.0 - 0.1..=web::HANG_SECS.1 + 0.1).contains(&r.hung_secs), "seed {seed}: 몇 초 {}", r.hung_secs);
        let sway = r.sway.1 - r.sway.0;
        assert!(sway > 0.02 * size && sway < 0.3 * size, "seed {seed}: 살랑살랑 {sway}");
        assert!(p.standing.is_some() && p.web_line().is_none());
        if !r.climbed {
            assert!(!r.hero, "seed {seed}: 사뿐 내려앉음 (영웅 착지·콰당 없음)");
            assert!((p.body.x - 900.0).abs() < size, "seed {seed}: 그 자리에 내려섬");
        }
    }
}

/// 🙃 끝은 둘 중 하나: 몸을 뒤집어 사뿐 내려앉거나, 줄을 타고 다시 올라가 높이서 놓고 영웅 착지
#[test]
fn it_either_drops_softly_or_climbs_back_up_and_leaves() {
    let w = world(vec![]);
    let (mut soft, mut climbed) = (0, 0);
    for seed in 0..16 {
        let mut p = hang_spider(900.0, seed, &w);
        assert!(p.start_hang_down(&w));
        let r = run_hang(&mut p, &w);
        if r.climbed {
            climbed += 1;
            assert!(r.hero, "seed {seed}: 높이서 놓아 영웅 착지");
        } else {
            soft += 1;
        }
    }
    assert!(soft >= 4 && climbed >= 2, "soft {soft}, climbed {climbed}");
}

/// 🙃 머리 위 창이 있으면 그 창 아랫변이나 창 위 아무 데 (창 면)에도 줄을 붙임 — 창이 닫히면 놓고 떨어짐
#[test]
fn it_hangs_from_a_window_bottom_or_face_overhead_and_lets_go_when_the_window_closes() {
    let win = WindowInfo { id: 5, rect: Rect::new(600.0, 150.0, 1300.0, 520.0) };
    let w = world(vec![win]);
    let mut ys = Vec::new();
    for seed in 0..24 {
        let mut p = hang_spider(900.0, seed, &w);
        assert!(p.start_hang_down(&w));
        let r = run_hang(&mut p, &w);
        ys.push(r.anchors[0].1);
    }
    assert!(ys.iter().any(|&y| y == 520.0), "창 아랫변 {ys:?}");
    assert!(ys.iter().any(|&y| y > 150.0 && y < 520.0), "창 면 {ys:?}");
    assert!(ys.iter().all(|&y| y == 0.0 || (150.0..=520.0).contains(&y)), "{ys:?}");
    // 창에 매달린 동안 창이 닫히면 놓고 떨어짐
    let gone = world(vec![]);
    for seed in 0..24 {
        let mut p = hang_spider(900.0, seed, &w);
        assert!(p.start_hang_down(&w));
        let mut guard = LoopLimit::new();
        while !matches!(p.state, State::Web { phase: WebPhase::Hang { .. }, .. }) {
            guard.step();
            p.update(DT, &w);
        }
        let State::Web { phase: WebPhase::Hang { line, .. }, .. } = p.state else { unreachable!() };
        if line.window != Some(5) {
            continue;
        }
        p.update(DT, &gone);
        assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
        assert!(p.web_line().is_none());
        return;
    }
    panic!("창에 매달린 경우가 없음");
}

/// 🙃 매달린 동안 가끔 가까운 펫이나 커서 쪽을 보며 "hey there"
#[test]
fn while_hanging_it_sometimes_turns_to_a_nearby_pet_or_the_cursor_and_says_hey() {
    let w = world(vec![]);
    let mut greeted = 0;
    for seed in 0..12 {
        let mut p = hang_spider(900.0, seed, &w);
        p.facing = 1.0;
        let pal = PetSpot { key: 99, x: 900.0 - 2.5 * p.cfg.size, y: 1040.0, owner: Some(Owner::Ground(0)), size: p.cfg.size, liftable: true, guest: false, carrying: false, reading: false };
        p.web.pets = vec![pal];
        assert!(p.start_hang_down(&w));
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Web { .. }) {
            guard.step();
            p.update(DT, &w);
            if p.speech == Some(Speech::HangHey) {
                assert!(matches!(p.state, State::Web { phase: WebPhase::Hang { .. }, .. }));
                assert_eq!(p.facing, -1.0, "seed {seed}: 펫 쪽 (왼쪽)을 봄");
                greeted += 1;
                p.speech = None;
            }
        }
    }
    assert!((2..=11).contains(&greeted), "가끔 {greeted}");
    // 펫이 없으면 커서 쪽
    let mut said = false;
    for seed in 0..12 {
        let mut p = hang_spider(900.0, seed, &w);
        p.facing = -1.0;
        p.cursor = (900.0 + 2.0 * p.cfg.size, 1040.0 - 2.0 * p.cfg.size);
        assert!(p.start_hang_down(&w));
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Web { .. }) {
            guard.step();
            p.update(DT, &w);
            if p.speech == Some(Speech::HangHey) {
                assert_eq!(p.facing, 1.0, "커서 쪽 (오른쪽)을 봄");
                said = true;
            }
        }
    }
    assert!(said);
}

/// 🙃 매달린 동안 끼어들면 (자리 비움·잡기·조작 모드) 깔끔히 놓음 — 공중에 멈추지 않고 줄도 사라짐
#[test]
fn an_interruption_mid_hang_lets_go_of_the_line() {
    let w = world(vec![]);
    for how in 0..3 {
        let mut p = hang_spider(900.0, 1, &w);
        assert!(p.start_hang_down(&w));
        let mut guard = LoopLimit::new();
        while !matches!(p.state, State::Web { phase: WebPhase::Hang { .. }, .. }) {
            guard.step();
            p.update(DT, &w);
        }
        match how {
            0 => p.set_user_away(true),
            1 => p.set_control(true),
            _ => {
                let (x, y) = (p.body.x, p.body.y - 10.0);
                p.pointer_down(x, y);
                assert!(matches!(p.state, State::Pressed { .. }), "{:?}", p.state);
                assert!(p.web_line().is_none());
                continue;
            }
        }
        p.update(DT, &w);
        assert!(!matches!(p.state, State::Web { .. }), "how {how}: {:?}", p.state);
        assert!(p.web_line().is_none());
    }
}

/// 🙃 내버려 두면 몇 분에 한 번쯤 매달려 내려옴 (다른 펫은 안 함)
#[test]
fn left_alone_it_hangs_down_every_few_minutes() {
    let w = world(vec![]);
    let mut hangs = 0;
    for seed in 0..4 {
        let mut p = resting(spider(), 900.0, seed, &w);
        p.hush_web_play();
        p.web.hang_ready_at = 0.0;
        let mut was = false;
        for _ in 0..(60 * 900) {
            p.update(DT, &w);
            let _ = p.take_web();
            let now = matches!(p.state, State::Web { phase: WebPhase::Hang { .. }, .. });
            hangs += usize::from(now && !was);
            was = now;
        }
    }
    // 4마리 × 15분 — 몇 분에 한 번
    assert!((4..=40).contains(&hangs), "{hangs}");
}

// ---- 💪🕷️ 거대한 것을 줄에 매달아 끌고 친구 화면으로 (v0.7.1) ----

/// 바닥의 거대한 것 (반 폭 236 px)
fn floor_giant(p: &Pet, dx: f64) -> ObjectSpot {
    ObjectSpot { half_w: 236.0, giant: true, ..floor_object(p, dx) }
}

/// 거대한 것을 받는 친구 끝 (set_giant_edges)이 던져 닿지 않게 멀면: 낚은 거대한 것을 뒤에 줄로 매달아 바닥으로 끌며 그 끝으로 걸어 나감
#[test]
fn toward_a_far_friend_edge_that_takes_giants_it_drags_a_lassoed_giant_behind_it_and_leaves_with_it() {
    let w = world(vec![]);
    let mut p = calm_spider(600.0, 4, &w);
    let size = p.cfg.size;
    assert!(1920.0 - 600.0 > web::GIANT_THROW_REACH * size);
    p.set_exit_edges(false, true);
    p.set_prank_edges(false, true);
    p.set_giant_edges(false, true);
    let spot = floor_giant(&p, -3.0 * size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    let (mut dragged, mut walked_out) = (0, 0);
    let mut guard = LoopLimit::new();
    let mut last_x = p.body.x;
    let mut last_point = None;
    let exited = loop {
        guard.step();
        p.update(DT, &w);
        assert!(p.take_juggle().is_none(), "던지지 않음");
        if let Some(side) = p.take_exit() {
            break side;
        }
        let dragging = matches!(p.state, State::Web { phase: WebPhase::Drag { .. }, t } if t > web::WHIRL_REEL_SECS);
        let leaving = matches!(p.state, State::Exit { .. });
        if dragging || leaving {
            assert!(!matches!(p.state, State::Web { phase: WebPhase::Whirl { .. }, .. }));
            assert_eq!(p.carried_object(), Some(42), "{:?}", p.state);
            assert_eq!(p.facing, 1.0);
            assert!(p.body.x >= last_x - 1e-9, "친구 쪽으로만 감");
            let at = p.carry_point().expect("끌 자리");
            assert!(at.0 + spot.half_w < p.body.x, "몸 뒤에 매달려 끌려옴 {at:?} {}", p.body.x);
            assert!((at.1 - p.body.y).abs() <= crate::object::CARRY_SINK_MAX * p.cfg.dpi + 1e-6, "바닥에 닿은 채 {at:?}");
            let line = p.web_line().expect("손에서 거대한 것까지 줄");
            assert!((line.to.0 - at.0).abs() < 1e-6 && line.to.1 < at.1, "줄 끝 = 거대한 것 {line:?}");
            assert!(!matches!(p.render().anim, Anim::Carry | Anim::CarryRun | Anim::Hold));
            dragged += usize::from(dragging);
            walked_out += usize::from(leaving);
            last_point = Some(at);
        }
        last_x = p.body.x;
    };
    assert!(dragged > 30 && walked_out > 0, "{dragged} {walked_out}");
    assert_eq!(exited, Side::Right);
    assert_eq!(p.carried_object(), Some(42), "앱이 Visit에 실음");
    let (x, _) = last_point.unwrap();
    assert!(x - spot.half_w > 1920.0 - 30.0, "거대한 것도 거의 다 나갔음 {x}");
}

/// 친구 앱이 옛 버전이라 거대한 것을 못 받거나 끝이 가까우면: 예전처럼 휘둘러 던짐 (끌지 않음)
#[test]
fn without_a_giant_taking_friend_or_with_a_close_edge_it_still_whirls_the_giant() {
    let w = world(vec![]);
    for (x, giant_edge) in [(600.0, false), (1920.0 - 200.0, true)] {
        let mut p = calm_spider(x, 4, &w);
        p.set_exit_edges(false, true);
        p.set_prank_edges(false, true);
        p.set_giant_edges(false, giant_edge);
        let spot = floor_giant(&p, -2.0 * p.cfg.size);
        p.notice_play_objects(&[spot], &w);
        assert!(p.start_whirl_object(&spot, Side::Right));
        let (mut whirled, mut released) = (false, false);
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Web { .. }) {
            guard.step();
            p.update(DT, &w);
            released |= p.take_juggle().is_some();
            assert!(!matches!(p.state, State::Web { phase: WebPhase::Drag { .. }, .. }), "x={x}");
            whirled |= matches!(p.state, State::Web { phase: WebPhase::Whirl { .. }, .. });
        }
        assert!(whirled && released, "x={x}");
    }
}

/// 거대한 것을 끌고 친구 화면에 들어옴 (hold_giant): 거대한 것도 뒤따라 다 들어오면 줄을 풀어 그 자리에 내려놓음 ("tada!")
#[test]
fn arriving_with_a_giant_it_drags_it_in_and_lets_go_once_it_is_inside() {
    let w = world(vec![]);
    let mut p = Pet::new_entering(spider(), &w, Side::Left, 3);
    p.hold_giant(42, 236.0);
    let mut ev = None;
    let mut last = None;
    let mut guard = LoopLimit::new();
    while ev.is_none() {
        guard.step();
        p.update(DT, &w);
        ev = p.take_juggle();
        if ev.is_none() {
            assert_eq!(p.carried_object(), Some(42), "{:?}", p.state);
            let at = p.carry_point().unwrap();
            assert!(at.0 < p.body.x, "뒤 (바깥쪽)에서 끌려 들어옴");
            assert!(p.web_line().is_some());
            last = Some(at);
        }
    }
    let Some(JuggleEvent::Release { id, vx, vy }) = ev else { panic!("{ev:?}") };
    assert_eq!(id, 42);
    assert!(vx.abs() < 1e-9 && vy.abs() < 1e-9, "그 자리에 내려놓음");
    assert_eq!(p.speech, Some(Speech::Tada));
    assert!(last.unwrap().0 - 236.0 >= -1.0, "다 들어온 뒤");
    assert!(p.carried_object().is_none() && p.web_line().is_none());
}

/// 끌던 거대한 것을 사용자가 낚아채면 "hmph!" 하고 그만
#[test]
fn snatching_a_dragged_giant_ends_it_with_a_hmph() {
    let w = world(vec![]);
    let mut p = calm_spider(600.0, 4, &w);
    p.set_exit_edges(false, true);
    p.set_prank_edges(false, true);
    p.set_giant_edges(false, true);
    let spot = floor_giant(&p, -3.0 * p.cfg.size);
    p.notice_play_objects(&[spot], &w);
    assert!(p.start_whirl_object(&spot, Side::Right));
    let mut guard = LoopLimit::new();
    while !matches!(p.state, State::Web { phase: WebPhase::Drag { .. }, t } if t > 0.5) {
        guard.step();
        p.update(DT, &w);
    }
    p.lose_carry();
    assert_eq!(p.speech, Some(Speech::Hmph));
    assert!(!matches!(p.state, State::Web { .. }) && p.carried_object().is_none() && p.web_line().is_none());
    for _ in 0..120 {
        p.update(DT, &w);
        assert!(p.take_juggle().is_none());
    }
}
