//! 🕸️ 둥근 거미집 (web.rs Corner::Center — spec 6부) 순수 계산 · 💪 거대한 것 휘두르기 (spec 7부)

use super::*;

/// 1920×1040 화면 하나
fn world(windows: Vec<WindowInfo>) -> World {
    World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
}

/// 창 하나
fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
    WindowInfo { id, rect: Rect::new(l, t, r, b) }
}

/// 가운데 (x, y) 반지름 r인 둥근 거미집 자리
fn orb(x: f64, y: f64, r: f64) -> WebSpin {
    WebSpin { x, y, corner: Corner::Center, size: r, window: None, shots: 6, build: 5.0 * WEAVE_SHOT_SECS }
}

/// 🕸️ 둥근 거미집은 이름이 center로 웹뷰에 감
#[test]
fn the_orb_kind_goes_to_the_webview_as_center() {
    assert_eq!(serde_json::to_string(&Corner::Center).unwrap(), "\"center\"");
    assert!(Corner::Center.is_orb() && !Corner::TopLeft.is_orb());
}

/// 🕸️ 둥근 거미집 판정은 원 전체 (사방 어디서든)
#[test]
fn in_web_for_an_orb_is_the_whole_disk() {
    for (dx, dy) in [(50.0, 0.0), (-50.0, 0.0), (0.0, 50.0), (0.0, -50.0), (40.0, -40.0), (-40.0, 40.0)] {
        assert!(in_web(500.0 + dx, 400.0 + dy, 500.0, 400.0, Corner::Center, 80.0), "{dx},{dy}");
    }
    assert!(!in_web(590.0, 400.0, 500.0, 400.0, Corner::Center, 80.0), "반지름 밖");
    assert!(!in_web(500.0, 400.0, 500.0, 400.0, Corner::Center, 0.0), "반지름 0은 없음");
}

/// 🕸️ 둥근 거미집 자리: 화면 안에 다 들어가야 하고, 바탕화면 자리면 어떤 창과도 안 겹침,
/// 창 위면 그 창 안에 다 들어가고 앞 창에 안 가려져야 함 (그 창 id)
#[test]
fn an_orb_fits_fully_inside_its_screen_or_window_and_not_under_front_windows() {
    let empty = world(vec![]);
    let at = (900.0, 900.0);
    assert_eq!(orb_fit(&empty, at, 900.0, 500.0, 200.0), Some(None), "바탕화면 한가운데");
    assert_eq!(orb_fit(&empty, at, 100.0, 500.0, 200.0), None, "화면 왼쪽 밖으로 삐져나옴");
    assert_eq!(orb_fit(&empty, at, 900.0, 950.0, 200.0), None, "화면 아래 밖으로");
    assert_eq!(orb_fit(&empty, at, 900.0, 500.0, 0.0), None);
    // 창 하나: 그 위면 그 창 id, 창 가장자리에 걸치면 없음, 바탕화면 자리여도 원이 창에 걸치면 없음
    let w = world(vec![win(7, 400.0, 100.0, 1400.0, 800.0)]);
    assert_eq!(orb_fit(&w, at, 900.0, 450.0, 200.0), Some(Some(7)));
    assert_eq!(orb_fit(&w, at, 450.0, 450.0, 200.0), None, "창 왼쪽 밖으로 삐져나옴");
    assert_eq!(orb_fit(&w, at, 1600.0, 450.0, 220.0), None, "바탕화면이지만 창에 걸침");
    assert_eq!(orb_fit(&w, at, 1680.0, 450.0, 200.0), Some(None), "창과 안 겹치는 바탕화면");
    // 앞 창이 원을 조금이라도 가리면 없음
    let covered = world(vec![win(2, 1000.0, 300.0, 1300.0, 600.0), win(7, 400.0, 100.0, 1400.0, 800.0)]);
    assert_eq!(orb_fit(&covered, at, 800.0, 450.0, 220.0), None);
    assert_eq!(orb_fit(&covered, at, 700.0, 450.0, 200.0), Some(Some(7)));
    // 다른 모니터 (at이 없는 작업 영역)에는 안 침
    let two = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 128.0);
    assert_eq!(orb_fit(&two, at, 2500.0, 500.0, 200.0), None);
}

/// 🕸️ 창 위 둥근 거미집은 창 왼쪽 위 기준 자리로 따라가고, 앞 창이 원에 걸치면 숨고, 창이 닫히거나 작아져 안 들어가면 사라짐
#[test]
fn a_window_orb_follows_its_window_by_offset_hides_when_covered_and_goes_when_closed() {
    let w = vec![win(1, 300.0, 300.0, 1100.0, 900.0)];
    assert_eq!(orb_on_window(&w, 1, (400.0, 250.0), 150.0), WebAt::Shown(700.0, 550.0));
    let moved = vec![win(1, 500.0, 100.0, 1300.0, 700.0)];
    assert_eq!(orb_on_window(&moved, 1, (400.0, 250.0), 150.0), WebAt::Shown(900.0, 350.0));
    let covered = vec![win(2, 1000.0, 0.0, 1500.0, 360.0), win(1, 500.0, 100.0, 1300.0, 700.0)];
    assert_eq!(orb_on_window(&covered, 1, (400.0, 250.0), 150.0), WebAt::Hidden(900.0, 350.0));
    assert_eq!(orb_on_window(&[win(2, 0.0, 0.0, 100.0, 100.0)], 1, (400.0, 250.0), 150.0), WebAt::Gone, "닫힘·최소화");
    let shrunk = vec![win(1, 300.0, 300.0, 750.0, 900.0)];
    assert_eq!(orb_on_window(&shrunk, 1, (400.0, 250.0), 150.0), WebAt::Gone, "작아져 원이 창 밖으로");
    // 거미집 처음 자리 (창 왼쪽 위 기준)
    let spin = WebSpin { window: Some(1), ..orb(700.0, 550.0, 150.0) };
    assert_eq!(window_offset(&w, &spin), (400.0, 250.0));
    assert_eq!(window_offset(&w, &orb(700.0, 550.0, 150.0)), (0.0, 0.0), "바탕화면 거미집");
}

/// 🕸️ 둥근 거미집 짜기: 줄 끝이 사방의 살 끝 → 안쪽 고리 차례, 모두 원 안, 매번 다른 자리
#[test]
fn an_orb_is_woven_spokes_first_then_rings_all_around() {
    let s = orb(900.0, 500.0, 200.0);
    let aims: Vec<(f64, f64)> = (1..=8).map(|k| weave_target(&s, k)).collect();
    let dist = |a: &(f64, f64)| (a.0 - s.x).hypot(a.1 - s.y);
    assert!(aims.iter().all(|a| dist(a) <= s.size + 1e-9), "{aims:?}");
    assert!(aims[..4].iter().all(|a| dist(a) > 0.95 * s.size), "먼저 살 끝");
    assert!(aims[4..].iter().all(|a| dist(a) < 0.9 * s.size), "그다음 고리");
    let quads: std::collections::HashSet<(bool, bool)> = aims.iter().map(|a| (a.0 >= s.x, a.1 >= s.y)).collect();
    assert_eq!(quads.len(), 4, "사방으로 {aims:?}");
    for (i, a) in aims.iter().enumerate() {
        assert!(aims[..i].iter().all(|q| (q.0 - a.0).hypot(q.1 - a.1) > 0.1 * s.size), "매번 다른 자리");
    }
}

/// 🕸️🧗 둥근 거미집에서 기어 다닐 자리도 원 안, 사방으로
#[test]
fn perch_points_on_an_orb_lie_inside_it_all_around() {
    let w = WebSpot { id: 3, x: 900.0, y: 500.0, corner: Corner::Center, size: 250.0, hidden: false };
    let pts: Vec<(f64, f64)> = (0..10).map(|k| perch_point(&w, k)).collect();
    assert!(pts.iter().all(|&(x, y)| in_web(x, y, w.x, w.y, w.corner, w.size)), "{pts:?}");
    assert!(pts.windows(2).all(|p| p[0] != p[1]));
    assert!(pts.iter().any(|p| p.0 < w.x) && pts.iter().any(|p| p.0 > w.x) && pts.iter().any(|p| p.1 < w.y) && pts.iter().any(|p| p.1 > w.y));
}

/// 💪 거대한 것은 무거워서 더 천천히, 더 작게, 덜 돌다가 꼭대기에서 던질 쪽으로
#[test]
fn a_heavy_whirl_is_slower_smaller_and_turns_fewer_times() {
    assert!(whirl_secs(true) > whirl_secs(false) && whirl_secs(false) == WHIRL_SECS);
    let (hand, size) = ((500.0, 600.0), 100.0);
    for dir in [1.0, -1.0] {
        let end = whirl_point(hand, size, dir, whirl_secs(true), true);
        assert!((end.0 - hand.0).abs() < 1e-6, "꼭대기 {end:?}");
        let before = whirl_point(hand, size, dir, whirl_secs(true) - 0.01, true);
        assert!((end.0 - before.0) * dir > 0.0, "던질 쪽으로 움직이며 끝남");
        let spread = |heavy: bool| {
            let xs: Vec<f64> = (0..=200).map(|k| whirl_point(hand, size, dir, whirl_secs(heavy) * k as f64 / 200.0, heavy).0).collect();
            xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) - xs.iter().copied().fold(f64::INFINITY, f64::min)
        };
        assert!(spread(true) < 0.8 * spread(false), "더 작게 {} {}", spread(true), spread(false));
        // 도는 빠르기 (바퀴 ÷ 시간)도 느림
        assert!(GIANT_WHIRL_TURNS / whirl_secs(true) < 0.6 * WHIRL_TURNS / whirl_secs(false));
    }
}
