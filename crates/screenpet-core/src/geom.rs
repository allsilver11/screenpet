/// 가상 화면 좌표(물리 픽셀)의 사각형. right/bottom은 끝(미포함) 좌표.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn new(left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self { left, top, right, bottom }
    }

    pub fn width(&self) -> f64 {
        self.right - self.left
    }

    pub fn height(&self) -> f64 {
        self.bottom - self.top
    }

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
    pub id: u64,
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
