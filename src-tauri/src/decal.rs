//! 꿀벌이 쏜 침 자국 (spec 2B §10) — 쏜 자리마다 화면에 "박힌" 작은 투명 창 하나 (`sting-<n>`, sting.html).
//! 마우스는 통과하고(set_ignore_cursor_events), 10초 머문 뒤 1초 동안 사라지면 창을 닫는다. 한 번에 최대 3개.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use tauri::{AppHandle, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const PREFIX: &str = "sting-";
/// 침 자국 창 한 변 (논리 px)
const SIZE: f64 = 128.0;
/// 동시에 남아 있는 침 자국 수 — 넘치면 가장 오래된 것부터 닫는다
const MAX_DECALS: usize = 3;
/// sting.html: 10초 머문 뒤 1초 동안 흐려진다. 다 사라진 뒤 조금 더 기다렸다 닫는다
const LIFETIME: Duration = Duration::from_millis(11_300);

#[derive(Default)]
pub struct Decals {
    next: u64,
    live: VecDeque<(WebviewWindow, Instant)>,
}

impl Decals {
    /// (x, y) = 침이 박힌 화면 지점 (물리 px), dpi = 그 자리 화면 배율 (쏜 동물 창의 배율).
    /// 게임 루프 스레드에서만 부른다 (창 만들기가 메인 스레드를 기다린다)
    pub fn spawn(&mut self, app: &AppHandle, x: f64, y: f64, dpi: f64) {
        while self.live.len() >= MAX_DECALS {
            if let Some((w, _)) = self.live.pop_front() {
                let _ = w.destroy();
            }
        }
        let label = format!("{PREFIX}{}", self.next);
        self.next += 1;
        let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("sting.html".into()))
            .title("ScreenPet Sting")
            .inner_size(SIZE, SIZE)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .visible(false)
            .build();
        let window = match built {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create sting decal: {e}");
                return;
            }
        };
        let half = SIZE / 2.0 * dpi;
        let _ = window.set_position(PhysicalPosition::new((x - half).round() as i32, (y - half).round() as i32));
        // 클릭·드래그·간질이기는 침 자국을 지나 그 아래(동물·다른 앱)로 간다
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
        self.live.push_back((window, Instant::now()));
    }

    /// 다 사라진 침 자국 창을 닫는다 (매 틱)
    pub fn tick(&mut self, now: Instant) {
        while self.live.front().is_some_and(|(_, born)| now.duration_since(*born) >= LIFETIME) {
            if let Some((w, _)) = self.live.pop_front() {
                let _ = w.destroy();
            }
        }
    }
}
