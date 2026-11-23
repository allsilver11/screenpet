//! 🌐 웹 페이지 디딤돌 위의 동물 — 내려서고, 따라 걷고, 사라지거나 스크롤되면 떨어지고, 브라우저를 끌면 같이 감.
//! brain.rs의 자식 모듈 (비공개 필드에 접근)

use super::tests::{cfg, place, run};
use super::*;
use crate::behavior::Move;
use crate::geom::{Rect, WindowInfo};
use crate::ledge::LedgeSet;
use crate::test_util::{idle, run_until, DT};

/// 테스트: 브라우저 창 (id 7)
fn browser() -> WindowInfo {
    WindowInfo { id: 7, rect: Rect::new(100.0, 100.0, 1300.0, 1000.0) }
}

/// 테스트: 윗변 y, 300..900인 글 덩어리 하나
fn set_at(y: f64) -> LedgeSet {
    LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(300.0, y, 900.0, y + 40.0)] }
}

/// 테스트: 브라우저와 디딤돌이 있는 세상
fn lworld(set: Option<LedgeSet>) -> World {
    let mut w = World::new(vec![browser()], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    w.set_ledges(set);
    w
}

/// 테스트: 세상의 첫 디딤돌 주인
fn ledge_owner(w: &World) -> Owner {
    Owner::Ledge { window: 7, id: w.ledges[0].id }
}

#[test]
fn a_falling_pet_lands_on_a_ledge() {
    let w = lworld(Some(set_at(500.0)));
    let mut p = Pet::new(cfg(), 600.0, 420.0, 1);
    p.state = State::Air { anim: Anim::Fall };
    assert!(run_until(&mut p, &w, 120, |p| p.standing.is_some()));
    assert_eq!(p.standing.map(|s| s.owner), Some(ledge_owner(&w)));
    assert_eq!(p.body.y, 500.0);
}

#[test]
fn a_pet_walks_along_a_ledge() {
    let w = lworld(Some(set_at(500.0)));
    let owner = ledge_owner(&w);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, owner, 500.0, State::Move { kind: Move::Walk, remaining: 1.0 });
    p.facing = 1.0;
    run(&mut p, &w, 30);
    assert_eq!(p.standing.map(|s| s.owner), Some(owner));
    assert!(p.body.x > 510.0, "{}", p.body.x);
    assert_eq!(p.body.y, 500.0);
}

#[test]
fn a_pet_falls_when_its_ledge_goes_or_scrolls_away() {
    for next in [None, Some(set_at(460.0))] {
        let mut w = lworld(Some(set_at(500.0)));
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, ledge_owner(&w), 600.0, idle());
        p.update(DT, &w);
        assert!(p.standing.is_some());
        w.set_ledges(next.clone());
        p.update(DT, &w);
        assert!(p.standing.is_none(), "{next:?}");
        assert!(run_until(&mut p, &w, 300, |p| p.standing.is_some()));
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)), "{next:?}");
    }
}

#[test]
fn a_pet_on_a_ledge_rides_a_dragged_browser() {
    let mut w = lworld(Some(set_at(500.0)));
    let owner = ledge_owner(&w);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, owner, 600.0, idle());
    assert_eq!(p.ridden_window(), Some(7));
    let mut rect = browser().rect;
    for tick in 0..20 {
        rect = Rect::new(rect.left + 10.0, rect.top + 2.0, rect.right + 10.0, rect.bottom + 2.0);
        assert!(w.patch_windows(&[(7, rect)]));
        p.update(DT, &w);
        assert_eq!(p.standing.map(|s| s.owner), Some(owner), "tick {tick}");
        assert_eq!(p.body.y, 500.0 + (rect.top - 100.0), "tick {tick}");
    }
}
