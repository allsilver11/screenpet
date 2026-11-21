//! 🌗 루모스·녹스로 화면 밝기 바꾸기 (0.7.2 — ⚙️ 설정 "🌗 루모스·녹스로 화면 밝기 바꾸기", 기본 끔 — Config.lumos_light 그대로): 이 화면에서 누가
//! (내 펫이든 놀러 온 펫이든) 💡 "Lumos!"를 켜면 (Pet::take_lumos) 다크 모드인 이 PC를, 🌑 "Nox!"로 끄면 (Pet::take_nox) 라이트 모드인 이 PC를
//! FLIP_SECS 동안 반대 모드로 바꿨다가 원래대로 되돌림.
//!
//! - 안전: 이미 그 모드면 안 바꿈 · 바꾸기 전에 원래 값을 표시 파일 (config 폴더 MARKER_FILE)에 먼저 적고 (못 적으면 안 바꿈), 되돌리면 지움 →
//!   앱이 그 사이 꺼져도 다음 실행 때 (recover) 표시 파일의 값으로 되돌림 · 설정을 끄거나 앱을 끄면 바로 되돌림 ·
//!   두 주문을 합쳐 한 번에 하나만 바꾸고 MIN_GAP_SECS에 한 번까지
//! - LumosLight: 상태 기계 (테마는 ThemeBackend로 — 테스트는 가짜, OS는 platform::theme_backend — Windows 레지스트리·macOS osascript)
//! - spawn: 일꾼 스레드에서 돌림 (osascript·WM_SETTINGCHANGE 알림이 게임 루프를 막지 않게) — 앱은 LumosHandle로 소식만 보냄

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 반대 모드로 있는 시간 (초 — 루모스·녹스 같음)
pub const FLIP_SECS: f64 = 10.0;
/// 한 번 바꾼 뒤 다시 바꿀 수 있기까지 (초 — 루모스·녹스 합쳐서)
pub const MIN_GAP_SECS: f64 = 60.0;
/// 바꾼 동안 원래 값을 적어 두는 표시 파일 (config 폴더 안 — 녹스도 같은 파일)
pub const MARKER_FILE: &str = "lumos-theme.json";

/// OS 테마 (Windows: 앱·시스템 따로, macOS: 둘이 같음)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    /// 앱 라이트 (Windows AppsUseLightTheme)
    pub apps_light: bool,
    /// 시스템 라이트 (Windows SystemUsesLightTheme — 작업표시줄·시작 메뉴)
    pub system_light: bool,
}

impl Theme {
    /// 다 라이트
    pub const LIGHT: Theme = Theme { apps_light: true, system_light: true };
    /// 다 다크
    pub const DARK: Theme = Theme { apps_light: false, system_light: false };

    /// 이미 다 라이트
    pub fn is_light(self) -> bool {
        self.apps_light && self.system_light
    }

    /// 이미 다 다크
    pub fn is_dark(self) -> bool {
        !self.apps_light && !self.system_light
    }
}

/// OS 테마를 읽고 쓰는 곳 (platform::theme_backend · 테스트 가짜)
pub trait ThemeBackend {
    /// 지금 테마 (못 읽으면 None)
    fn read(&mut self) -> Option<Theme>;
    /// 테마를 바꿈 (됐으면 true)
    fn write(&mut self, theme: Theme) -> bool;
}

/// 🌗 화면 밝기 바꾸기 상태 기계 (now = 초 — 일꾼은 시작한 뒤 흐른 시간)
#[derive(Debug)]
pub struct LumosLight {
    marker: PathBuf,
    enabled: bool,
    /// 바꾸기 전 원래 테마와 되돌릴 시각 (바꿔 둔 동안만 — 루모스·녹스 하나뿐)
    flipped: Option<(Theme, f64)>,
    /// 마지막으로 바꾼 시각 (루모스·녹스 합쳐서)
    last_at: Option<f64>,
}

impl LumosLight {
    /// marker = 표시 파일 자리, enabled = ⚙️ 설정
    pub fn new(marker: PathBuf, enabled: bool) -> Self {
        LumosLight { marker, enabled, flipped: None, last_at: None }
    }

    /// 지금 반대 모드로 바꿔 둔 동안 (루모스든 녹스든)
    pub fn is_flipped(&self) -> bool {
        self.flipped.is_some()
    }

    /// 앱을 켤 때: 지난번에 되돌리지 못한 표시 파일이 있으면 그 값으로 되돌리고 지움. 되돌렸으면 true
    pub fn recover(&mut self, b: &mut dyn ThemeBackend) -> bool {
        let Ok(text) = std::fs::read_to_string(&self.marker) else { return false };
        let restored = match serde_json::from_str::<Theme>(&text) {
            Ok(orig) => b.write(orig),
            // 망가진 표시 파일은 지움 (무엇으로 되돌릴지 모름)
            Err(_) => false,
        };
        let _ = std::fs::remove_file(&self.marker);
        restored
    }

    /// ⚙️ 설정을 켜고 끔 — 끄면 바꿔 둔 테마를 바로 되돌림
    pub fn set_enabled(&mut self, on: bool, b: &mut dyn ThemeBackend) {
        self.enabled = on;
        if !on {
            self.restore(b);
        }
    }

    /// 💡 이 화면에서 Lumos가 켜짐: 지금 다크 (다 라이트가 아님)면 잠깐 라이트로 (flip). 바꿨으면 true
    pub fn lumos(&mut self, now: f64, b: &mut dyn ThemeBackend) -> bool {
        self.flip(now, b, Theme::LIGHT)
    }

    /// 🌑 이 화면에서 Nox로 불이 꺼짐: 지금 라이트 (다 다크가 아님)면 잠깐 다크로 (flip). 바꿨으면 true
    pub fn nox(&mut self, now: f64, b: &mut dyn ThemeBackend) -> bool {
        self.flip(now, b, Theme::DARK)
    }

    /// 설정이 켜져 있고, 바꿔 둔 게 없고, MIN_GAP_SECS가 지났고, 이미 to가 아니면 원래 값을 표시 파일에 적고 to로. 바꿨으면 true
    fn flip(&mut self, now: f64, b: &mut dyn ThemeBackend, to: Theme) -> bool {
        if !self.enabled || self.flipped.is_some() || self.last_at.is_some_and(|at| now - at < MIN_GAP_SECS) {
            return false;
        }
        let Some(orig) = b.read() else { return false };
        if orig == to {
            return false;
        }
        if write_marker(&self.marker, orig).is_err() {
            return false;
        }
        self.last_at = Some(now);
        if !b.write(to) {
            let _ = std::fs::remove_file(&self.marker);
            return false;
        }
        self.flipped = Some((orig, now + FLIP_SECS));
        true
    }

    /// 때가 되면 원래 테마로 되돌림
    pub fn tick(&mut self, now: f64, b: &mut dyn ThemeBackend) {
        if self.flipped.is_some_and(|(_, until)| now >= until) {
            self.restore(b);
        }
    }

    /// 바꿔 둔 테마를 바로 되돌리고 표시 파일을 지움 (앱을 끌 때도). 못 되돌리면 표시 파일을 남겨 다음 실행 때 다시
    pub fn restore(&mut self, b: &mut dyn ThemeBackend) {
        let Some((orig, _)) = self.flipped.take() else { return };
        if b.write(orig) {
            let _ = std::fs::remove_file(&self.marker);
        }
    }
}

/// 표시 파일에 원래 테마를 적음
fn write_marker(path: &Path, theme: Theme) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string(&theme).map_err(std::io::Error::other)?)
}

/// 일꾼 스레드에 보내는 소식
enum Msg {
    Lumos,
    Nox,
    Enabled(bool),
    Quit(mpsc::Sender<()>),
}

/// 🌗 일꾼 스레드 손잡이 (앱이 가짐)
pub struct LumosHandle {
    tx: mpsc::Sender<Msg>,
}

impl LumosHandle {
    /// 💡 이 화면에서 Lumos가 켜졌음
    pub fn lumos(&self) {
        let _ = self.tx.send(Msg::Lumos);
    }

    /// 🌑 이 화면에서 Nox로 불이 꺼졌음
    pub fn nox(&self) {
        let _ = self.tx.send(Msg::Nox);
    }

    /// ⚙️ 설정이 바뀜
    pub fn set_enabled(&self, on: bool) {
        let _ = self.tx.send(Msg::Enabled(on));
    }

    /// 앱을 끔: 바꿔 둔 테마를 되돌릴 때까지 조금 (wait까지) 기다림
    pub fn quit(&self, wait: Duration) {
        let (tx, rx) = mpsc::channel();
        if self.tx.send(Msg::Quit(tx)).is_ok() {
            let _ = rx.recv_timeout(wait);
        }
    }
}

/// 🌗 일꾼 스레드를 띄움: 먼저 지난번 표시 파일을 되돌리고 (recover), 소식을 받으며 때가 되면 되돌림. 스레드를 못 띄우면 None
pub fn spawn(mut backend: Box<dyn ThemeBackend + Send>, marker: PathBuf, enabled: bool) -> Option<LumosHandle> {
    let (tx, rx) = mpsc::channel::<Msg>();
    std::thread::Builder::new()
        .name("lumos-theme".into())
        .spawn(move || {
            let start = Instant::now();
            let mut light = LumosLight::new(marker, enabled);
            light.recover(backend.as_mut());
            loop {
                let msg = rx.recv_timeout(Duration::from_millis(250));
                let now = start.elapsed().as_secs_f64();
                match msg {
                    Ok(Msg::Lumos) => {
                        light.lumos(now, backend.as_mut());
                    }
                    Ok(Msg::Nox) => {
                        light.nox(now, backend.as_mut());
                    }
                    Ok(Msg::Enabled(on)) => light.set_enabled(on, backend.as_mut()),
                    Ok(Msg::Quit(done)) => {
                        light.restore(backend.as_mut());
                        let _ = done.send(());
                        return;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        light.restore(backend.as_mut());
                        return;
                    }
                }
                light.tick(now, backend.as_mut());
            }
        })
        .ok()?;
    Some(LumosHandle { tx })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 가짜 테마 (쓴 것을 적어 둠, fail이면 쓰기 실패)
    struct Fake {
        now: Theme,
        writes: Vec<Theme>,
        fail: bool,
    }

    impl ThemeBackend for Fake {
        fn read(&mut self) -> Option<Theme> {
            Some(self.now)
        }
        fn write(&mut self, theme: Theme) -> bool {
            if self.fail {
                return false;
            }
            self.now = theme;
            self.writes.push(theme);
            true
        }
    }

    const DARK: Theme = Theme::DARK;

    fn fake(now: Theme) -> Fake {
        Fake { now, writes: Vec::new(), fail: false }
    }

    /// 테스트마다 따로 쓰는 표시 파일 자리
    fn marker(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("screenpet-lumos-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(MARKER_FILE)
    }

    #[test]
    fn lumos_turns_a_dark_pc_light_for_ten_seconds_then_restores_the_original() {
        let m = marker("cycle");
        let mixed = Theme { apps_light: false, system_light: true };
        let mut b = fake(mixed);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.lumos(0.0, &mut b));
        assert_eq!(b.now, Theme::LIGHT);
        assert!(l.is_flipped() && m.exists(), "표시 파일에 원래 값");
        let saved: Theme = serde_json::from_str(&std::fs::read_to_string(&m).unwrap()).unwrap();
        assert_eq!(saved, mixed);
        l.tick(FLIP_SECS - 0.1, &mut b);
        assert_eq!(b.now, Theme::LIGHT);
        l.tick(FLIP_SECS, &mut b);
        assert_eq!(b.now, mixed, "원래대로 (앱·시스템 따로)");
        assert!(!l.is_flipped() && !m.exists(), "표시 파일을 지움");
    }

    #[test]
    fn never_flips_when_off_already_light_or_too_soon() {
        let m = marker("rules");
        let mut b = fake(DARK);
        let mut off = LumosLight::new(m.clone(), false);
        assert!(!off.lumos(0.0, &mut b), "설정이 꺼져 있음");
        let mut light = fake(Theme::LIGHT);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(!l.lumos(0.0, &mut light) && light.writes.is_empty(), "이미 라이트");
        assert!(l.lumos(1.0, &mut b));
        assert!(!l.lumos(2.0, &mut b), "바꿔 둔 동안");
        l.tick(20.0, &mut b);
        assert_eq!(b.now, DARK);
        assert!(!l.lumos(30.0, &mut b), "1분에 한 번까지");
        assert!(l.lumos(1.0 + MIN_GAP_SECS, &mut b));
    }

    #[test]
    fn turning_the_setting_off_or_quitting_restores_right_away() {
        let m = marker("off");
        let mut b = fake(DARK);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.lumos(0.0, &mut b));
        l.set_enabled(false, &mut b);
        assert_eq!(b.now, DARK);
        assert!(!m.exists());
        assert!(!l.lumos(100.0, &mut b));
        l.set_enabled(true, &mut b);
        assert!(l.lumos(100.0, &mut b));
        l.restore(&mut b);
        assert_eq!(b.now, DARK);
        assert!(!m.exists());
    }

    #[test]
    fn a_left_over_marker_is_restored_on_the_next_start() {
        let m = marker("crash");
        let mut b = fake(DARK);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.lumos(0.0, &mut b));
        drop(l);
        // 앱이 그 사이 꺼짐 — 다음 실행
        assert_eq!(b.now, Theme::LIGHT);
        let mut again = LumosLight::new(m.clone(), false);
        assert!(again.recover(&mut b));
        assert_eq!(b.now, DARK);
        assert!(!m.exists());
        assert!(!again.recover(&mut b), "두 번은 안 함");
        std::fs::write(&m, "not json").unwrap();
        assert!(!again.recover(&mut b) && !m.exists(), "망가진 표시 파일은 지움");
    }

    #[test]
    fn a_failed_switch_leaves_nothing_behind_and_a_failed_restore_keeps_the_marker() {
        let m = marker("fail");
        let mut b = fake(DARK);
        b.fail = true;
        let mut l = LumosLight::new(m.clone(), true);
        assert!(!l.lumos(0.0, &mut b));
        assert!(!m.exists() && !l.is_flipped());
        b.fail = false;
        assert!(l.lumos(MIN_GAP_SECS, &mut b));
        b.fail = true;
        l.tick(MIN_GAP_SECS + FLIP_SECS, &mut b);
        assert!(m.exists(), "못 되돌렸으면 다음 실행 때 다시");
    }

    #[test]
    fn nox_turns_a_light_pc_dark_for_ten_seconds_then_restores_the_original() {
        let m = marker("nox-cycle");
        let mixed = Theme { apps_light: true, system_light: false };
        let mut b = fake(mixed);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.nox(0.0, &mut b));
        assert_eq!(b.now, Theme::DARK);
        assert!(l.is_flipped() && m.exists(), "표시 파일에 원래 값");
        let saved: Theme = serde_json::from_str(&std::fs::read_to_string(&m).unwrap()).unwrap();
        assert_eq!(saved, mixed);
        l.tick(FLIP_SECS - 0.1, &mut b);
        assert_eq!(b.now, Theme::DARK);
        l.tick(FLIP_SECS, &mut b);
        assert_eq!(b.now, mixed, "원래대로 (앱·시스템 따로)");
        assert!(!l.is_flipped() && !m.exists(), "표시 파일을 지움");
    }

    #[test]
    fn nox_never_flips_when_off_or_already_dark() {
        let m = marker("nox-rules");
        let mut b = fake(Theme::LIGHT);
        let mut off = LumosLight::new(m.clone(), false);
        assert!(!off.nox(0.0, &mut b) && b.writes.is_empty(), "설정이 꺼져 있음");
        let mut dark = fake(DARK);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(!l.nox(0.0, &mut dark) && dark.writes.is_empty() && !m.exists(), "이미 다크");
        assert!(l.nox(1.0, &mut b), "이미 다크일 땐 1분을 쓰지 않음");
    }

    #[test]
    fn lumos_and_nox_share_one_switch_and_one_minute_gap() {
        let m = marker("shared");
        let mut b = fake(Theme::LIGHT);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.nox(0.0, &mut b));
        assert!(!l.lumos(1.0, &mut b), "바꿔 둔 동안엔 루모스도 안 함 (다크인데도)");
        assert!(!l.nox(1.0, &mut b));
        l.tick(FLIP_SECS, &mut b);
        assert_eq!(b.now, Theme::LIGHT);
        b.now = DARK;
        assert!(!l.lumos(30.0, &mut b), "녹스 뒤 1분 안엔 루모스도 안 함");
        assert!(l.lumos(MIN_GAP_SECS, &mut b));
        assert_eq!(b.now, Theme::LIGHT);
        l.tick(MIN_GAP_SECS + FLIP_SECS, &mut b);
        assert_eq!(b.now, DARK);
        b.now = Theme::LIGHT;
        assert!(!l.nox(MIN_GAP_SECS + 30.0, &mut b), "루모스 뒤 1분 안엔 녹스도 안 함");
        assert!(l.nox(2.0 * MIN_GAP_SECS, &mut b));
    }

    #[test]
    fn a_left_over_nox_marker_is_restored_and_setting_off_restores_nox() {
        let m = marker("nox-crash");
        let mut b = fake(Theme::LIGHT);
        let mut l = LumosLight::new(m.clone(), true);
        assert!(l.nox(0.0, &mut b));
        drop(l);
        assert_eq!(b.now, Theme::DARK);
        let mut again = LumosLight::new(m.clone(), true);
        assert!(again.recover(&mut b));
        assert_eq!(b.now, Theme::LIGHT);
        assert!(!m.exists());
        assert!(again.nox(0.0, &mut b));
        again.set_enabled(false, &mut b);
        assert_eq!(b.now, Theme::LIGHT);
        assert!(!m.exists() && !again.is_flipped());
    }

    #[test]
    fn the_worker_thread_switches_and_restores_on_quit() {
        use std::sync::{Arc, Mutex};
        struct Shared(Arc<Mutex<Theme>>);
        impl ThemeBackend for Shared {
            fn read(&mut self) -> Option<Theme> {
                Some(*self.0.lock().unwrap())
            }
            fn write(&mut self, theme: Theme) -> bool {
                *self.0.lock().unwrap() = theme;
                true
            }
        }
        let theme = Arc::new(Mutex::new(DARK));
        let h = spawn(Box::new(Shared(theme.clone())), marker("worker"), true).unwrap();
        h.lumos();
        let t0 = Instant::now();
        while *theme.lock().unwrap() != Theme::LIGHT && t0.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*theme.lock().unwrap(), Theme::LIGHT);
        h.quit(Duration::from_secs(3));
        assert_eq!(*theme.lock().unwrap(), DARK);
        // 🌑 녹스도 일꾼이 받음
        *theme.lock().unwrap() = Theme::LIGHT;
        let h = spawn(Box::new(Shared(theme.clone())), marker("worker-nox"), true).unwrap();
        h.nox();
        let t0 = Instant::now();
        while *theme.lock().unwrap() != Theme::DARK && t0.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*theme.lock().unwrap(), Theme::DARK);
        h.quit(Duration::from_secs(3));
        assert_eq!(*theme.lock().unwrap(), Theme::LIGHT);
    }
}
