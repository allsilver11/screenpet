//! 🕷️ 거미인간 더 (brain_web.rs): 🌉🚪 친구 화면으로 그네 타고 넘어가기·줄 타고 날아 들어오기 · 🕸️🧗 거미집에 올라가 놀기

use super::swing_tests::resting;
use super::tests::{place, world};
use super::web_moves::{WebAim, WebPhase};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::test_util::{bundled, LoopLimit, DT};
use crate::visit::Fling;
use crate::web::{self, Corner, WebSpot};

/// 번들 pet.json 그대로의 거미인간
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

/// 바닥(x)에 쉬는 거미인간 — 주사위 놀이는 모두 꺼 둠 (start_*로만)
fn calm_spider(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(spider(), x, seed, w);
    p.hush_stage();
    p.web.web_ready_at = f64::INFINITY;
    p.web.swing_ready_at = f64::INFINITY;
    p.web.pull_ready_at = f64::INFINITY;
    p.web.cocoon_ready_at = f64::INFINITY;
    p.web.play_ready_at = f64::INFINITY;
    p
}

/// 오른쪽 끝(친구 연결됨)으로 걸어가다 나가기로 한 거미인간 (시드를 돌며 찾음) — 그 뒤 첫 상태와 함께
fn leaving_spider(w: &World) -> Pet {
    for seed in 0..200 {
        let mut p = calm_spider(900.0, seed, w);
        let x = w.bounds.right - p.cfg.half_w - 0.5;
        place(&mut p, w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.set_exit_edges(false, true);
        p.update(DT, w);
        if matches!(p.state, State::Exit { .. } | State::Web { .. }) {
            return p;
        }
    }
    panic!("200 시드 동안 안 나감");
}

/// 🌉🚪 친구가 연결된 끝으로 나갈 때 걸어 나가지 않고 그 끝 위로 줄을 쏴 그네를 타고 끝 너머로 휙 — 나간 끝은 한 번, 넘어간 움직임도 실음
#[test]
fn a_spider_leaves_for_a_friend_by_swinging_past_the_edge() {
    let w = world(vec![]);
    let mut p = leaving_spider(&w);
    let State::Web { phase: WebPhase::Shoot { aim: WebAim::Anchor(a), .. }, .. } = p.state else { panic!("줄부터 쏨 {:?}", p.state) };
    assert!(a.x > w.bounds.right - p.cfg.size && a.y == 0.0, "끝 바로 안쪽 위 {a:?}");
    let floor = p.body.y;
    let mut swung = false;
    let mut highest = floor;
    let mut exits = Vec::new();
    let mut guard = LoopLimit::new();
    while exits.is_empty() {
        guard.step();
        assert!(!matches!(p.state, State::Exit { .. }), "걸어 나가지 않음");
        swung |= matches!(p.state, State::Web { phase: WebPhase::Swing { .. }, .. });
        highest = highest.min(p.body.y);
        p.update(DT, &w);
        exits.extend(p.take_exit());
    }
    assert_eq!(exits, vec![Side::Right]);
    assert!(swung, "그네를 탐");
    assert!(highest < floor - p.cfg.size, "몸이 높이 떴음 {highest}");
    assert!(p.is_away());
    let fling = p.take_fling().expect("넘어간 움직임");
    assert!(fling.vx > 0.0 && fling.height < 0.9, "{fling:?}");
    for _ in 0..120 {
        p.update(DT, &w);
        assert_eq!(p.take_exit(), None, "한 번만");
    }
}

/// 🌉🚪 줄을 붙일 데가 없으면 (천장이 너무 낮음) 평소처럼 걸어 나감
#[test]
fn with_no_room_to_swing_it_walks_out_as_usual() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 300.0)], 128.0);
    let mut p = leaving_spider(&w);
    assert!(matches!(p.state, State::Exit { side: Side::Right, .. }), "{:?}", p.state);
    let mut guard = LoopLimit::new();
    while p.take_exit().is_none() {
        guard.step();
        p.update(DT, &w);
    }
    assert!(p.is_away());
}

/// 🌉🚪 친구 화면에 넘어온 거미인간은 줄을 쏴 그네 타고 날아 들어와 영웅 착지 (던져진 것처럼 굴지 않음)
#[test]
fn an_arriving_spider_swings_in_and_lands_like_a_hero() {
    let w = world(vec![]);
    let mut swings = 0;
    let mut heroes = 0;
    for seed in 0..10 {
        let mut p = Pet::new(spider(), 0.0, 0.0, seed);
        p.hush_stage();
        p.web.play_ready_at = f64::INFINITY;
        p.enter_flung(&w, Side::Left, Fling { height: 0.5, vx: 700.0, vy: -150.0 });
        assert!(!p.thrown, "던져진 게 아님");
        let mut swung = false;
        let mut hero = false;
        let mut guard = LoopLimit::new();
        while p.standing.is_none() || matches!(p.state, State::Web { .. }) {
            guard.step();
            p.update(DT, &w);
            swung |= matches!(p.state, State::Web { phase: WebPhase::Swing { .. }, .. });
            hero |= p.speech == Some(Speech::HeroLand);
            assert!(p.body.x > -p.cfg.size && p.body.x < 1920.0 + p.cfg.size);
        }
        swings += usize::from(swung);
        heroes += usize::from(hero);
    }
    assert!(swings >= 8, "대개 줄 타고 들어옴 {swings}");
    assert!(heroes >= 8, "영웅 착지 {heroes}");
}

/// 바닥 오른쪽 아래 구석 거미집 (반지름 r 몸 크기 배)
fn corner_web(id: u64, r: f64, size: f64) -> WebSpot {
    WebSpot { id, x: 1920.0, y: 1040.0, corner: Corner::BottomRight, size: r * size, hidden: false }
}

/// 거미집 놀이를 끝까지 (최대 30초): (거미집 안에 있던 틱 수, 본 그림들, 마지막에 디딤판 위)
fn play_out(p: &mut Pet, w: &World, webs: &[WebSpot]) -> (usize, Vec<Anim>, bool) {
    let mut inside = 0;
    let mut anims = Vec::new();
    for _ in 0..(30.0 / DT) as usize {
        p.set_webs(webs);
        p.update(DT, w);
        assert!(!p.is_stuck(), "거미인간은 안 걸림");
        assert!(p.take_torn_web().is_none(), "거미집을 안 찢음");
        let c = (p.body.x, p.body.y - 0.5 * p.cfg.size);
        if webs.iter().any(|s| web::in_web(c.0, c.1, s.x, s.y, s.corner, s.size)) && p.standing.is_none() {
            inside += 1;
        }
        anims.push(p.render().anim);
        if !matches!(p.state, State::Web { .. }) && p.standing.is_some() && inside > 0 {
            return (inside, anims, true);
        }
    }
    (inside, anims, p.standing.is_some())
}

/// 🕸️🧗 가까운 거미집에 줄을 쏴 올라가 살 위를 기어 다니다 매달렸다가 내려옴 — 거미집은 그대로
#[test]
fn a_spider_climbs_onto_a_nearby_web_crawls_around_and_comes_down() {
    let w = world(vec![]);
    let size = spider().size;
    let webs = [corner_web(7, 3.0, size)];
    let mut p = calm_spider(1920.0 - 6.0 * size, 1, &w);
    p.set_webs(&webs);
    assert!(p.start_web_play(&w), "놀러 감");
    let (inside, anims, down) = play_out(&mut p, &w, &webs);
    assert!(inside as f64 * DT > 1.5, "거미집 위에서 한참 놂 {inside}");
    assert!(anims.contains(&Anim::Climb), "기어 다님");
    assert!(down, "내려와 섬");
}

/// 🕸️🧗 높은 창 모서리 거미집에서 놀다가 뚝 떨어지면 영웅 착지
#[test]
fn coming_down_from_a_high_web_ends_in_a_hero_landing() {
    let win = WindowInfo { id: 3, rect: Rect::new(1000.0, 500.0, 1700.0, 900.0) };
    let w = world(vec![win]);
    let size = spider().size;
    let webs = [WebSpot { id: 8, x: 1700.0, y: 500.0, corner: Corner::TopRight, size: 2.5 * size, hidden: false }];
    let mut p = calm_spider(1850.0, 2, &w);
    p.set_webs(&webs);
    assert!(p.start_web_play(&w));
    let mut hero = false;
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Web { .. }) || p.standing.is_none() {
        guard.step();
        p.set_webs(&webs);
        p.update(DT, &w);
        hero |= p.speech == Some(Speech::HeroLand);
    }
    assert!(hero, "영웅 착지");
}

/// 🕸️🧗 놀던 거미집이 사라지면 그냥 떨어짐
#[test]
fn if_the_web_vanishes_mid_play_it_just_drops() {
    let w = world(vec![]);
    let size = spider().size;
    let webs = [corner_web(7, 3.0, size)];
    let mut p = calm_spider(1920.0 - 6.0 * size, 1, &w);
    p.set_webs(&webs);
    assert!(p.start_web_play(&w));
    let mut guard = LoopLimit::new();
    while !matches!(p.state, State::Web { phase: WebPhase::Crawl { .. }, .. }) {
        guard.step();
        p.update(DT, &w);
    }
    p.set_webs(&[]);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
}

/// 🕸️🧗 가까이 보이는 거미집이 있으면 가끔 (몇십 초에 한 번쯤) 놀러 가고, 멀거나 가려졌거나 없으면 안 감. 다른 동물은 안 감
#[test]
fn near_a_web_a_free_spider_plays_on_it_now_and_then() {
    let w = world(vec![]);
    let size = spider().size;
    let count = |webs: &[WebSpot], cfg: PetConfig| count_plays(&w, webs, cfg);
    let near = count(&[corner_web(7, 3.0, size)], spider());
    assert!((8..=30).contains(&near), "5분 × 4마리에 {near}번");
    // 아주 높은 화면의 맨 위 구석 — 바닥 어디서도 몸 WEB_PLAY_REACH배 밖
    let tall = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0 + 12.0 * size)], 128.0);
    let far = [WebSpot { id: 7, x: 0.0, y: 0.0, corner: Corner::TopLeft, size, hidden: false }];
    assert_eq!(count_plays(&tall, &far, spider()), 0, "멀리");
    assert_eq!(count(&[WebSpot { hidden: true, ..corner_web(7, 3.0, size) }], spider()), 0, "가려짐");
    assert_eq!(count(&[], spider()), 0, "없음");
    assert_eq!(count(&[corner_web(7, 3.0, size)], crate::test_util::pet_cfg()), 0, "거미인간만");
}

/// 4마리 × 5분 동안 거미집 놀이를 시작한 수 (주사위로만)
fn count_plays(w: &World, webs: &[WebSpot], cfg: PetConfig) -> usize {
        let mut total = 0;
        let size = cfg.size;
        for seed in 0..4 {
            let mut p = resting(cfg.clone(), 1920.0 - 6.0 * size, seed, w);
            p.hush_stage();
            p.web.web_ready_at = f64::INFINITY;
            p.web.swing_ready_at = f64::INFINITY;
            p.web.pull_ready_at = f64::INFINITY;
            p.web.cocoon_ready_at = f64::INFINITY;
            p.web.play_ready_at = 0.0;
            let mut was = false;
            for _ in 0..(300.0 / DT) as usize {
                p.set_webs(webs);
                p.update(DT, w);
                let now = matches!(p.state, State::Web { phase: WebPhase::Crawl { .. }, .. });
                total += usize::from(now && !was);
                was = now;
            }
        }
        total
}
