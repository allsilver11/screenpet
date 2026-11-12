//! 📐 기본 도형 — 사각형(Rect)과 다른 앱의 창(WindowInfo). 좌표는 가상 화면의 물리 px
/// 가상 화면 좌표(물리 픽셀)의 사각형. right/bottom은 끝(미포함) 좌표.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// 왼쪽 x
    pub left: f64,
    /// 위쪽 y
    pub top: f64,
    /// 오른쪽 x (미포함)
    pub right: f64,
    /// 아래쪽 y (미포함)
    pub bottom: f64,
}

impl Rect {
    /// 네 변으로 사각형을 만듦
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self { left, top, right, bottom }
    }

    /// 너비
    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    /// 높이
    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

    /// (x, y)가 안에 있음 (왼쪽·위 포함, 오른쪽·아래 제외)
    pub fn contains(&self, x: f64, y: f64) -> bool {
        self.left <= x && x < self.right && self.top <= y && y < self.bottom
    }

    /// 둘을 모두 덮는 가장 작은 사각형
    pub fn union(&self, o: &Rect) -> Rect {
        Rect::new(
            self.left.min(o.left),
            self.top.min(o.top),
            self.right.max(o.right),
            self.bottom.max(o.bottom),
        )
    }
}

/// 다른 앱의 최상위 창 하나.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowInfo {
    /// OS 창 id
    pub id: u64,
    /// 창 경계 (물리 px)
    pub rect: Rect,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_and_height() {
        let r = Rect::new(10.0, 20.0, 110.0, 70.0);
        assert_eq!(r.width(), 100.0);
        assert_eq!(r.height(), 50.0);
    }

    #[test]
    fn union_covers_both() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(50.0, -20.0, 300.0, 80.0);
        assert_eq!(a.union(&b), Rect::new(0.0, -20.0, 300.0, 100.0));
    }
}
