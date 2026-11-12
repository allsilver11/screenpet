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

/// 🍎 macOS "입력 모니터링" 권한 묻기 — 키 누름 카운터는 이 권한이 있어야 오른다. 앱이 직접 묻는다 (사람들이 설정을 못 찾는다):
/// 타자 응원이 켜져 있고 권한이 없을 때만, 앱 버전마다 한 번 (켤 때 ASK_DELAY 뒤), 그리고 메뉴에서 끈 걸 다시 켤 때.
/// 묻기 전에 펫이 왜 묻는지 말풍선으로 먼저 말한다 (Explain → REQUEST_AFTER 뒤 Request).
/// 실행 중에 권한이 생기면 한 번 "다시 켜면 시작돼요" (RestartHint — macOS는 보통 앱을 다시 켜야 적용된다)
pub const ASK_DELAY: Duration = Duration::from_secs(10);
/// 메뉴에서 켠 뒤 말풍선까지
pub const TOGGLE_DELAY: Duration = Duration::from_millis(400);
/// 말풍선 뒤 시스템 창까지
pub const REQUEST_AFTER: Duration = Duration::from_millis(2500);

/// 이번 실행을 시작할 때 물을까: 켜져 있고, 권한이 없다고 확실하고 (None = 권한이 없는 OS), 이 버전에서 아직 안 물었다
/// (ad-hoc 서명이라 업데이트마다 권한이 풀릴 수 있다 — 버전마다 한 번이면 그때 다시 묻는다)
pub fn ask_at_start(cheer_on: bool, granted: Option<bool>, asked_version: Option<&str>, version: &str) -> bool {
    cheer_on && granted == Some(false) && asked_version != Some(version)
}

/// 메뉴에서 켰을 때 물을까 (끌 때는 절대 묻지 않는다)
pub fn ask_on_toggle(now_on: bool, granted: Option<bool>) -> bool {
    now_on && granted == Some(false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AskStep {
    /// 펫이 왜 묻는지 말한다
    Explain,
    /// 시스템 권한 창을 띄운다 (부른 쪽이 이 버전을 물었다고 적어 둔다)
    Request,
    /// 실행 중에 권한이 생겼다: 다시 켜면 시작된다고 한 번 말한다
    RestartHint,
}

/// 권한 묻기 순서 (게임 루프가 틱마다 tick)
#[derive(Debug, Default)]
pub struct PermissionAsk {
    explain_at: Option<Instant>,
    request_at: Option<Instant>,
    last: Option<bool>,
    restart_hinted: bool,
}

impl PermissionAsk {
    pub fn start(now: Instant, cheer_on: bool, granted: Option<bool>, asked_version: Option<&str>, version: &str) -> Self {
        let explain_at = ask_at_start(cheer_on, granted, asked_version, version).then_some(now + ASK_DELAY);
        PermissionAsk { explain_at, request_at: None, last: granted, restart_hinted: false }
    }

    /// 메뉴에서 타자 응원을 켜거나 껐다
    pub fn toggled(&mut self, now: Instant, now_on: bool, granted: Option<bool>) {
        self.request_at = None;
        self.explain_at = ask_on_toggle(now_on, granted).then_some(now + TOGGLE_DELAY);
    }

    /// 지금 할 일 (하나씩). granted = 지금 권한 (None = 필요 없는 OS)
    pub fn tick(&mut self, now: Instant, cheer_on: bool, granted: Option<bool>) -> Option<AskStep> {
        let was = std::mem::replace(&mut self.last, granted);
        if !cheer_on || granted != Some(false) {
            self.explain_at = None;
            self.request_at = None;
        }
        if was == Some(false) && granted == Some(true) && cheer_on && !self.restart_hinted {
            self.restart_hinted = true;
            return Some(AskStep::RestartHint);
        }
        if self.explain_at.is_some_and(|t| now >= t) {
            self.explain_at = None;
            self.request_at = Some(now + REQUEST_AFTER);
            return Some(AskStep::Explain);
        }
        if self.request_at.is_some_and(|t| now >= t) {
            self.request_at = None;
            return Some(AskStep::Request);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 시간을 흘리며 나온 단계들
    fn run(ask: &mut PermissionAsk, t0: Instant, secs: f64, cheer_on: bool, granted: Option<bool>) -> Vec<(u64, AskStep)> {
        let mut out = Vec::new();
        let mut ms = 0;
        while (ms as f64) < secs * 1000.0 {
            if let Some(step) = ask.tick(t0 + Duration::from_millis(ms), cheer_on, granted) {
                out.push((ms, step));
            }
            ms += 16;
        }
        out
    }

    #[test]
    fn asks_once_per_version_only_while_on_and_not_granted() {
        assert!(ask_at_start(true, Some(false), None, "0.6.3"));
        assert!(ask_at_start(true, Some(false), Some("0.6.2"), "0.6.3"));
        assert!(!ask_at_start(true, Some(false), Some("0.6.3"), "0.6.3"));
        assert!(!ask_at_start(false, Some(false), None, "0.6.3"));
        assert!(!ask_at_start(true, Some(true), None, "0.6.3"));
        // Windows (권한이 없는 OS)는 묻지 않는다
        assert!(!ask_at_start(true, None, None, "0.6.3"));
        assert!(ask_on_toggle(true, Some(false)));
        assert!(!ask_on_toggle(false, Some(false)));
        assert!(!ask_on_toggle(true, Some(true)) && !ask_on_toggle(true, None));
    }

    #[test]
    fn explains_after_ten_seconds_then_requests() {
        let t0 = Instant::now();
        let mut ask = PermissionAsk::start(t0, true, Some(false), None, "0.6.3");
        let steps = run(&mut ask, t0, 30.0, true, Some(false));
        assert_eq!(steps.len(), 2, "{steps:?}");
        assert_eq!(steps[0].1, AskStep::Explain);
        assert!((10_000..10_100).contains(&steps[0].0), "{steps:?}");
        assert_eq!(steps[1].1, AskStep::Request);
        assert!((12_500..12_600).contains(&steps[1].0), "{steps:?}");
        // 이미 이 버전에서 물었으면 조용
        let mut ask = PermissionAsk::start(t0, true, Some(false), Some("0.6.3"), "0.6.3");
        assert!(run(&mut ask, t0, 30.0, true, Some(false)).is_empty());
    }

    #[test]
    fn turning_it_on_asks_again_and_off_cancels() {
        let t0 = Instant::now();
        let mut ask = PermissionAsk::start(t0, true, Some(false), Some("0.6.3"), "0.6.3");
        ask.toggled(t0, true, Some(false));
        let steps: Vec<AskStep> = run(&mut ask, t0, 5.0, true, Some(false)).into_iter().map(|(_, s)| s).collect();
        assert_eq!(steps, vec![AskStep::Explain, AskStep::Request]);
        // 끄면 기다리던 것도 취소, 꺼진 동안엔 아무것도
        let mut ask = PermissionAsk::start(t0, true, Some(false), None, "0.6.3");
        ask.toggled(t0, false, Some(false));
        assert!(run(&mut ask, t0, 30.0, false, Some(false)).is_empty());
        // 말풍선 뒤 창을 띄우기 전에 꺼도 묻지 않는다
        let mut ask = PermissionAsk::start(t0, true, Some(false), None, "0.6.3");
        assert_eq!(ask.tick(t0 + Duration::from_secs(10), true, Some(false)), Some(AskStep::Explain));
        assert_eq!(ask.tick(t0 + Duration::from_secs(11), false, Some(false)), None);
        assert!(run(&mut ask, t0 + Duration::from_secs(11), 10.0, true, Some(false)).is_empty());
    }

    #[test]
    fn a_grant_while_running_says_restart_once() {
        let t0 = Instant::now();
        let mut ask = PermissionAsk::start(t0, true, Some(false), Some("0.6.3"), "0.6.3");
        assert_eq!(ask.tick(t0, true, Some(false)), None);
        assert_eq!(ask.tick(t0, true, Some(true)), Some(AskStep::RestartHint));
        assert_eq!(ask.tick(t0, true, Some(false)), None);
        assert_eq!(ask.tick(t0, true, Some(true)), None);
        // 처음부터 있던 권한은 말하지 않는다
        let mut ask = PermissionAsk::start(t0, true, Some(true), None, "0.6.3");
        assert!(run(&mut ask, t0, 15.0, true, Some(true)).is_empty());
    }

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
