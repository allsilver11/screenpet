//! macOS 좌표 변환 (순수 함수 — 리눅스에서도 테스트가 돈다).
//!
//! 좌표계 세 가지:
//! - **Cocoa** (NSScreen.frame/visibleFrame, NSWindow.frame): 포인트, 원점은 주 화면 왼쪽 *아래*, y는 위로.
//! - **CG 전역** (CGWindowList의 kCGWindowBounds, CGEventGetLocation, CGDisplayBounds): 포인트,
//!   원점은 주 화면 왼쪽 *위*, y는 아래로.
//! - **게임 세계 = tao/Tauri의 "물리 px"**: tao는 macOS에서 창 위치를 `CG 전역 포인트 × 그 창이 있는 화면의
//!   backingScaleFactor`로 내주고 (`outer_position`), `set_position(PhysicalPosition)`은 같은 배율로 나눠
//!   포인트로 되돌린다. 모니터 위치도 `CGDisplayBounds 원점 × 그 모니터 배율`이다.
//!
//! 그래서 이 모듈은 모든 사각형·점을 **그것이 놓인 화면의 배율**로 곱한다. 배율이 모두 같은 구성
//! (모두 Retina 2×, 또는 모두 1×)에서는 이 공간이 연속이라 Windows의 물리 px처럼 쓸 수 있다.
//! 배율이 섞인 다중 모니터에서는 tao 자신의 물리 좌표도 화면 경계에서 끊기므로 (틈·겹침) 여기서도
//! 그대로 따라간다 — 화면 경계를 걸치는 창은 중심이 있는 화면의 배율을 쓴다.

use crate::geom::Rect;

/// 화면 하나 — 둘 다 CG 전역 포인트 (왼쪽 위 원점).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    /// 화면 전체
    pub frame: Rect,
    /// 메뉴 막대·Dock을 뺀 영역
    pub visible: Rect,
    /// backingScaleFactor (Retina = 2.0)
    pub scale: f64,
}

/// Cocoa 사각형 (왼쪽 아래 원점, y 위로) → CG 전역 포인트 사각형.
/// primary_height = 주 화면 (Cocoa 원점이 있는 화면, NSScreen.screens[0]) 높이.
pub fn cocoa_to_top_left(x: f64, y: f64, w: f64, h: f64, primary_height: f64) -> Rect {
    let top = primary_height - (y + h);
    Rect::new(x, top, x + w, top + h)
}

fn contains(r: &Rect, x: f64, y: f64) -> bool {
    r.left <= x && x < r.right && r.top <= y && y < r.bottom
}

fn distance_sq(r: &Rect, x: f64, y: f64) -> f64 {
    let dx = (r.left - x).max(x - r.right).max(0.0);
    let dy = (r.top - y).max(y - r.bottom).max(0.0);
    dx * dx + dy * dy
}

/// 포인트 (x, y)가 있는 화면의 배율 — 화면 사이 틈이면 가장 가까운 화면, 화면을 모르면 1.0
pub fn scale_at(screens: &[Screen], x: f64, y: f64) -> f64 {
    if let Some(s) = screens.iter().find(|s| contains(&s.frame, x, y)) {
        return s.scale;
    }
    screens
        .iter()
        .min_by(|a, b| distance_sq(&a.frame, x, y).total_cmp(&distance_sq(&b.frame, x, y)))
        .map_or(1.0, |s| s.scale)
}

/// CG 전역 포인트 → 게임 세계 (tao 물리 px)
pub fn point_to_physical(screens: &[Screen], x: f64, y: f64) -> (f64, f64) {
    let k = scale_at(screens, x, y);
    (x * k, y * k)
}

/// CG 전역 포인트 사각형 → 게임 세계. 중심이 있는 화면의 배율을 쓴다.
pub fn rect_to_physical(screens: &[Screen], r: Rect) -> Rect {
    let k = scale_at(screens, (r.left + r.right) / 2.0, (r.top + r.bottom) / 2.0);
    Rect::new(r.left * k, r.top * k, r.right * k, r.bottom * k)
}

/// 화면별 작업 영역 (메뉴 막대·Dock 제외) → 게임 세계. 각 화면은 자기 배율로.
pub fn work_areas_physical(screens: &[Screen]) -> Vec<Rect> {
    screens
        .iter()
        .map(|s| {
            let v = s.visible;
            Rect::new(v.left * s.scale, v.top * s.scale, v.right * s.scale, v.bottom * s.scale)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_screen_flips_to_top_left() {
        // 1440×900 주 화면, 메뉴 막대 25pt, 아래 Dock 70pt → visibleFrame (0, 70, 1440, 805)
        let r = cocoa_to_top_left(0.0, 70.0, 1440.0, 805.0, 900.0);
        assert_eq!(r, Rect::new(0.0, 25.0, 1440.0, 830.0));
        let full = cocoa_to_top_left(0.0, 0.0, 1440.0, 900.0, 900.0);
        assert_eq!(full, Rect::new(0.0, 0.0, 1440.0, 900.0));
    }

    #[test]
    fn secondary_screen_above_and_right() {
        // 오른쪽 위에 걸친 1920×1080 화면: Cocoa y=600 (주 화면 아래끝에서 600pt 위)
        let r = cocoa_to_top_left(1440.0, 600.0, 1920.0, 1080.0, 900.0);
        assert_eq!(r, Rect::new(1440.0, -780.0, 3360.0, 300.0));
    }

    fn two_screens() -> Vec<Screen> {
        let a = Rect::new(0.0, 0.0, 1440.0, 900.0);
        let b = Rect::new(1440.0, 0.0, 3360.0, 1080.0);
        vec![
            Screen { frame: a, visible: Rect::new(0.0, 25.0, 1440.0, 830.0), scale: 2.0 },
            Screen { frame: b, visible: Rect::new(1440.0, 25.0, 3360.0, 1080.0), scale: 1.0 },
        ]
    }

    #[test]
    fn scale_follows_the_screen_under_the_point() {
        let s = two_screens();
        assert_eq!(scale_at(&s, 100.0, 100.0), 2.0);
        assert_eq!(scale_at(&s, 2000.0, 100.0), 1.0);
        // 주 화면 아래 틈 (y 1000) — 가장 가까운 화면은 오른쪽 화면 (x 1500)
        assert_eq!(scale_at(&s, 1500.0, 1000.0), 1.0);
        assert_eq!(scale_at(&[], 5.0, 5.0), 1.0);
    }

    #[test]
    fn points_and_rects_scale_like_tao() {
        let s = two_screens();
        assert_eq!(point_to_physical(&s, 100.0, 50.0), (200.0, 100.0));
        assert_eq!(point_to_physical(&s, 2000.0, 50.0), (2000.0, 50.0));
        let w = rect_to_physical(&s, Rect::new(100.0, 100.0, 600.0, 500.0));
        assert_eq!(w, Rect::new(200.0, 200.0, 1200.0, 1000.0));
        // 중심이 오른쪽 화면에 있으면 1×
        let w = rect_to_physical(&s, Rect::new(1400.0, 100.0, 2000.0, 500.0));
        assert_eq!(w, Rect::new(1400.0, 100.0, 2000.0, 500.0));
    }

    #[test]
    fn work_areas_use_each_screens_scale() {
        let s = two_screens();
        assert_eq!(
            work_areas_physical(&s),
            vec![Rect::new(0.0, 50.0, 2880.0, 1660.0), Rect::new(1440.0, 25.0, 3360.0, 1080.0)]
        );
    }
}
