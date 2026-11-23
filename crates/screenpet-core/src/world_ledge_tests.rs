//! 🌐 웹 페이지 디딤돌이 세상에 들어간 모습 — 그 브라우저 창의 디딤판(Owner::Ledge)이지만 창은 아님.
//! world.rs의 자식 모듈

use super::*;
use crate::hop;
use crate::ledge::LedgeSet;
use crate::surfaces::EndKind;

/// 테스트: 브라우저 창 (id 7)
fn browser() -> WindowInfo {
    WindowInfo { id: 7, rect: Rect::new(100.0, 100.0, 1300.0, 1000.0) }
}

/// 테스트: 브라우저 안 글 덩어리 하나 (300..900, 윗변 500)
fn set() -> LedgeSet {
    LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(300.0, 500.0, 900.0, 540.0)] }
}

/// 테스트: 창들과 디딤돌을 넣은 세상
fn lworld(windows: Vec<WindowInfo>, set: Option<LedgeSet>) -> World {
    let mut w = World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    w.set_ledges(set);
    w
}

/// 테스트: 디딤돌 디딤판 조각들
fn ledge_pieces(w: &World) -> Vec<Surface> {
    w.surfaces.iter().copied().filter(|s| matches!(s.owner, Owner::Ledge { .. })).collect()
}

#[test]
fn a_ledge_is_a_stand_only_top_edge_of_its_browser() {
    let w = lworld(vec![browser()], Some(set()));
    assert_eq!(w.ledges.len(), 1);
    let owner = Owner::Ledge { window: 7, id: w.ledges[0].id };
    let p = w.piece_at(owner, 600.0).copied().unwrap();
    assert_eq!((p.y, p.x_start, p.x_end), (500.0, 300.0, 900.0));
    assert_eq!((p.start_kind, p.end_kind), (EndKind::Edge, EndKind::Edge));
    assert_eq!(w.anchor(owner), Some((300.0, 500.0)));
    // 떨어지다 내려섬
    assert_eq!(w.landing(600.0, 400.0, 600.0).map(|s| s.owner), Some(owner));
    assert_eq!(w.fall_landing(600.0, 400.0, 600.0, 100.0, false).map(|s| s.owner), Some(owner));
    // 아래 친구에게 던진 것은 지나감
    assert_eq!(w.fall_landing(600.0, 400.0, 600.0, 100.0, true), None);
}

#[test]
fn a_ledge_is_not_a_window() {
    let w = lworld(vec![browser()], Some(set()));
    let plain = lworld(vec![browser()], None);
    assert_eq!(w.windows, plain.windows);
    assert_eq!(w.front_window_at(600.0, 520.0).map(|f| f.id), Some(7));
    assert_eq!(w.front_window_at(50.0, 520.0), None);
    // 디딤돌 옆면에는 부딪히지 않음 (300을 넘어가도)
    assert_eq!(w.side_hit(255.0, 265.0, 530.0, 38.0), None);
    // 창 윗면·안쪽 바닥은 그대로
    let not_ledge = |w: &World| w.surfaces.iter().copied().filter(|s| !matches!(s.owner, Owner::Ledge { .. })).collect::<Vec<_>>();
    assert_eq!(not_ledge(&w), plain.surfaces);
    assert!(!inside::has_floor(&w, 99));
}

#[test]
fn a_window_in_front_cuts_the_ledge() {
    let front = WindowInfo { id: 2, rect: Rect::new(500.0, 450.0, 700.0, 800.0) };
    let w = lworld(vec![front, browser()], Some(set()));
    let p = ledge_pieces(&w);
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].x_start, p[0].x_end, p[0].end_kind), (300.0, 500.0, EndKind::Blocked));
    assert_eq!((p[1].x_start, p[1].x_end, p[1].start_kind), (700.0, 900.0, EndKind::Blocked));
    // 다 덮으면 없음
    let cover = WindowInfo { id: 2, rect: Rect::new(0.0, 0.0, 1920.0, 1040.0) };
    assert!(ledge_pieces(&lworld(vec![cover, browser()], Some(set()))).is_empty());
    // 뒤에 있는 창은 가리지 않음
    assert_eq!(ledge_pieces(&lworld(vec![browser(), front], Some(set()))).len(), 1);
}

#[test]
fn ledges_need_their_browser_head_room_and_a_spot_inside_it() {
    assert!(lworld(vec![], Some(set())).ledges.is_empty(), "browser gone");
    assert!(ledge_pieces(&lworld(vec![], Some(set()))).is_empty());
    let high = LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(300.0, 110.0, 900.0, 140.0)] };
    assert!(ledge_pieces(&lworld(vec![browser()], Some(high))).is_empty(), "no head room under the screen top");
    let below = LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(300.0, 1000.0, 900.0, 1040.0)] };
    assert!(ledge_pieces(&lworld(vec![browser()], Some(below))).is_empty(), "not inside the browser");
    let wide = LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(0.0, 500.0, 1600.0, 540.0)] };
    let p = ledge_pieces(&lworld(vec![browser()], Some(wide)));
    assert_eq!((p[0].x_start, p[0].x_end), (100.0, 1300.0), "clipped to the browser");
}

#[test]
fn ledges_follow_a_dragged_browser_and_go_when_it_is_resized_or_cleared() {
    let mut w = lworld(vec![browser()], Some(set()));
    let id = w.ledges[0].id;
    let owner = Owner::Ledge { window: 7, id };
    assert!(w.patch_windows(&[(7, Rect::new(150.0, 120.0, 1350.0, 1020.0))]));
    assert_eq!(w.anchor(owner), Some((350.0, 520.0)));
    assert_eq!(w.piece_at(owner, 650.0).map(|s| s.y), Some(520.0));
    assert!(w.patch_windows(&[(7, Rect::new(150.0, 120.0, 1000.0, 1020.0))]));
    assert_eq!(w.anchor(owner), None);
    assert!(ledge_pieces(&w).is_empty());
    let mut w = lworld(vec![browser()], Some(set()));
    w.set_ledges(None);
    assert_eq!(w.anchor(owner), None);
    assert!(ledge_pieces(&w).is_empty());
}

#[test]
fn pets_can_leap_onto_a_ledge() {
    let low = LedgeSet { window: 7, frame: browser().rect, rects: vec![Rect::new(300.0, 700.0, 900.0, 740.0)] };
    let w = lworld(vec![browser()], Some(low));
    let owner = Owner::Ledge { window: 7, id: w.ledges[0].id };
    let t = hop::jump_targets(&w.surfaces, Owner::Ground(0), 200.0, 1040.0, 128.0, 38.0);
    assert!(t.iter().any(|t| t.owner == owner && t.y == 700.0), "{t:?}");
}
