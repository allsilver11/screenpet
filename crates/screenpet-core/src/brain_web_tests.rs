//! 🕷️ 거미인간 줄 놀이 (brain_web.rs): 거미집·그네·영웅 착지·끌어오기·🌀 휘둘러 던지기

use super::hoist_moves::{notice_pets, sync_lifts};
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
    let (sx, sy) = match s.corner { Corner::TopLeft => (1.0, 1.0), Corner::TopRight => (-1.0, 1.0), Corner::BottomLeft => (1.0, -1.0), Corner::BottomRight => (-1.0, -1.0) };
    let mut aims: Vec<(f64, f64)> = Vec::new();
    for k in 1..s.shots {
        let a = web::weave_target(&s, k);
        assert!((a.0 - s.x) * sx >= -1e-9 && (a.1 - s.y) * sy >= -1e-9, "안쪽 {a:?}");
        assert!((a.0 - s.x).hypot(a.1 - s.y) <= s.size + 1e-9);
        assert!(aims.iter().all(|q| (q.0 - a.0).hypot(q.1 - a.1) > 0.1 * s.size), "매번 다른 자리");
        aims.push(a);
    }
}

/// 🕸️ 거미집 크기는 제각각 — 작게도, 꽤 크게도. 창 모서리는 창보다 크지 않게
#[test]
fn web_sizes_vary_from_small_to_big_and_fit_their_window() {
    let w = world(vec![]);
    let mut sizes = Vec::new();
    for seed in 0..40 {
        let mut p = calm_spider(900.0, seed, &w);
        let size = p.cfg.size;
        let spot = web::corner_spots(&w, (0.0, 1040.0), 1e9, size)[0];
        let s = p.sized_web(spot, &w);
        sizes.push(s.size / size);
    }
    let (lo, hi) = sizes.iter().fold((f64::INFINITY, 0.0f64), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(lo >= web::WEB_SIZE_RANGE.0 - 1e-9 && hi <= web::WEB_SIZE_RANGE.1 + 1e-9);
    assert!(lo < 1.0 && hi > 2.0, "작게도 크게도: {lo}..{hi}");
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
