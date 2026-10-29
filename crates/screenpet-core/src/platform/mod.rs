use crate::geom::{Rect, WindowInfo};

#[cfg(windows)]
mod win32;
#[cfg(windows)]
pub use win32::Win32Source;

pub trait WindowSource: Send {
    /// 다른 프로세스의 보이는 최상위 창, 앞→뒤(Z-order) 순.
    fn windows(&mut self) -> Vec<WindowInfo>;
    /// 모니터별 작업 영역 (작업표시줄 제외).
    fn work_areas(&mut self) -> Vec<Rect>;
    /// 전역 커서 위치 (물리 px).
    fn cursor(&mut self) -> (f64, f64);
    fn left_button_down(&mut self) -> bool;
    /// 오른쪽 버튼 (직접 주기 거두기)
    fn right_button_down(&mut self) -> bool {
        false
    }
    /// Esc 키 (직접 주기 거두기)
    fn escape_down(&mut self) -> bool {
        false
    }
}

/// 지원하지 않는 OS용. 바닥(기본 화면)만 있는 세계가 된다.
pub struct NullSource;

impl WindowSource for NullSource {
    fn windows(&mut self) -> Vec<WindowInfo> {
        Vec::new()
    }

    fn work_areas(&mut self) -> Vec<Rect> {
        Vec::new()
    }

    fn cursor(&mut self) -> (f64, f64) {
        (0.0, 0.0)
    }

    fn left_button_down(&mut self) -> bool {
        false
    }
}

pub fn default_source() -> Box<dyn WindowSource> {
    #[cfg(windows)]
    return Box::new(Win32Source);
    #[cfg(not(windows))]
    return Box::new(NullSource);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_source_is_empty_and_released() {
        let mut s = NullSource;
        assert!(s.windows().is_empty());
        assert!(s.work_areas().is_empty());
        assert!(!s.left_button_down());
    }
}
