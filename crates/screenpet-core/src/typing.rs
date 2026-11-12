//! ⌨️ 타자 응원 — 얼마나 빨리 치는지만 센다 (초당 키 수). *어떤* 키인지, 무슨 글자인지는 읽지도 남기지도 않는다.
//!
//! 플랫폼 쪽 (`WindowSource::key_presses`)은 "지난번 뒤로 새로 눌린 키 수" 하나만 돌려준다. Windows는 타자 키들의
//! 눌림 상태를 매 틱 읽어 올라가는 순간만 세고 (`KeyEdges` — 지난 틱의 눌림 여부 비트 256개가 전부다),
//! macOS는 시스템의 키 누름 카운터를 읽는다. 여기 `TypingMeter`는 그 수를 최근 2초 창으로 모아 초당 키 수로 바꾼다.
//! 저장·로그·네트워크로 가는 것은 없다 — 게임 루프가 그 순간의 속도를 흐물이 뇌에 넘길 뿐이다.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// 속도를 재는 미끄럼 창
pub const WINDOW: Duration = Duration::from_secs(2);
/// 한 번 읽기에서 받아들이는 최대 누름 수 (카운터가 튀거나 처음 읽을 때 큰 값이 와도 응원이 폭주하지 않게)
pub const MAX_PER_POLL: u32 = 12;
/// 보고하는 최고 속도 (초당 키) — 사람이 아무리 빨라도 이 정도
pub const MAX_RATE: f64 = 25.0;

/// 키마다 지난 읽기 때 눌려 있었는지 (비트 하나씩, 가상 키 코드 0..256). 올라가는 순간을 찾는 데만 쓰고,
/// 어떤 키였는지는 어디에도 남기지 않는다 — 결과는 개수뿐이다
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KeyEdges {
    down: [u64; 4],
}

impl KeyEdges {
    /// 키 하나의 지금 상태를 넣고, 새로 눌렸으면 true. `tapped` = 지난 읽기 뒤 눌렸다 뗀 적이 있다 (빨리 친 키를 놓치지 않게)
    pub fn press(&mut self, key: u8, down: bool, tapped: bool) -> bool {
        let (word, bit) = ((key >> 6) as usize, 1u64 << (key & 63));
        let was = self.down[word] & bit != 0;
        if down {
            self.down[word] |= bit;
        } else {
            self.down[word] &= !bit;
        }
        !was && (down || tapped)
    }

    /// 지금 눌린 것으로 기억하는 키가 있다
    pub fn any_down(&self) -> bool {
        self.down.iter().any(|w| *w != 0)
    }
}

/// 최근 2초 동안의 누름 수 → 초당 키 수. 누름이 있던 틱만 (시각, 수)로 남는다 — 키 이름은 없다
#[derive(Debug, Default)]
pub struct TypingMeter {
    recent: VecDeque<(Instant, u32)>,
}

impl TypingMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// 이번 틱에 센 누름 수
    pub fn add(&mut self, now: Instant, presses: u32) {
        self.forget(now);
        if presses > 0 {
            self.recent.push_back((now, presses.min(MAX_PER_POLL)));
        }
    }

    /// 지금 초당 키 수 (0 ..= MAX_RATE)
    pub fn rate(&mut self, now: Instant) -> f64 {
        self.forget(now);
        let total: u32 = self.recent.iter().map(|(_, n)| n).sum();
        (total as f64 / WINDOW.as_secs_f64()).min(MAX_RATE)
    }

    /// 다 잊는다 (타자 응원을 껐을 때)
    pub fn clear(&mut self) {
        self.recent.clear();
    }

    fn forget(&mut self, now: Instant) {
        while self.recent.front().is_some_and(|(at, _)| now.saturating_duration_since(*at) >= WINDOW) {
            self.recent.pop_front();
        }
    }
}

/// 카운터 두 값 사이의 새 누름 수 (macOS 카운터처럼 계속 오르는 u32). 처음 읽기는 0, 되감기면 0
pub fn counter_delta(last: Option<u32>, now: u32) -> u32 {
    match last {
        Some(last) if now >= last => (now - last).min(MAX_PER_POLL),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_count_only_the_moment_a_key_goes_down() {
        let mut e = KeyEdges::default();
        assert!(e.press(b'A', true, false));
        // 누르고 있는 동안(자동 반복 포함)은 더 세지 않는다
        assert!(!e.press(b'A', true, false));
        assert!(!e.press(b'A', true, true));
        assert!(e.any_down());
        assert!(!e.press(b'A', false, false));
        assert!(!e.any_down());
        assert!(e.press(b'A', true, false));
        // 두 읽기 사이에 눌렀다 뗀 키도 한 번 센다
        assert!(e.press(b'B', false, true));
        assert!(!e.press(b'B', false, false));
        // 키마다 따로, 255까지
        assert!(e.press(255, true, false));
        assert!(e.press(0, true, false));
        assert!(!e.press(255, true, false));
    }

    #[test]
    fn edges_remember_nothing_but_one_bit_per_key() {
        // 개인정보: 키 순서·글자·시각을 담을 자리가 없다 — 256비트가 전부다
        assert_eq!(std::mem::size_of::<KeyEdges>(), 32);
    }

    #[test]
    fn rate_is_presses_over_the_last_two_seconds() {
        let t0 = Instant::now();
        let mut m = TypingMeter::new();
        assert_eq!(m.rate(t0), 0.0);
        // 2초 동안 초당 8타
        for i in 0..16 {
            m.add(t0 + Duration::from_millis(i * 125), 1);
        }
        let now = t0 + Duration::from_millis(1990);
        assert!((m.rate(now) - 8.0).abs() < 1e-9, "{}", m.rate(now));
        // 손을 떼면 2초 안에 0으로
        assert!(m.rate(t0 + Duration::from_millis(3000)) < 8.0);
        assert_eq!(m.rate(t0 + Duration::from_millis(4000)), 0.0);
        // 누름 없는 틱은 아무것도 남기지 않는다
        for i in 0..500 {
            m.add(t0 + Duration::from_secs(5) + Duration::from_millis(i), 0);
        }
        assert!(m.recent.is_empty());
    }

    #[test]
    fn rate_is_bounded_and_can_be_cleared() {
        let t0 = Instant::now();
        let mut m = TypingMeter::new();
        m.add(t0, 1_000_000);
        assert_eq!(m.rate(t0), MAX_PER_POLL as f64 / 2.0);
        for i in 0..200 {
            m.add(t0 + Duration::from_millis(i * 5), MAX_PER_POLL);
        }
        assert_eq!(m.rate(t0 + Duration::from_secs(1)), MAX_RATE);
        m.clear();
        assert_eq!(m.rate(t0 + Duration::from_secs(1)), 0.0);
    }

    #[test]
    fn counter_deltas_skip_the_first_read_and_jumps() {
        assert_eq!(counter_delta(None, 12345), 0);
        assert_eq!(counter_delta(Some(10), 13), 3);
        assert_eq!(counter_delta(Some(10), 10), 0);
        assert_eq!(counter_delta(Some(10), 9), 0);
        assert_eq!(counter_delta(Some(0), 10_000), MAX_PER_POLL);
    }

    /// 개인정보: 플랫폼의 센서 코드는 개수만 다룬다 — 키 코드를 모으거나 (Vec·push·문자열) 찍거나 (로그) 하지 않는다
    #[test]
    fn the_sensor_code_keeps_no_keys() {
        for (file, src) in [("win32.rs", include_str!("platform/win32.rs")), ("macos.rs", include_str!("platform/macos.rs"))] {
            let start = src.find("// ---- ⌨️ typing sensor").unwrap_or_else(|| panic!("{file}: no typing section"));
            let end = src[start..].find("// ---- end typing sensor").map(|i| start + i).unwrap_or_else(|| panic!("{file}: no end"));
            let section = &src[start..end];
            for banned in ["Vec", "push", "String", "format!", "print", "log::", "eprint", "write", "File", "static mut", "char"] {
                assert!(!section.contains(banned), "{file}: typing sensor mentions {banned}");
            }
            assert!(section.contains("fn key_presses"), "{file}");
            assert!(!section.contains("SetWindowsHookEx") && !section.contains("CGEventTapCreate"), "{file}: no hooks or taps");
        }
    }
}
