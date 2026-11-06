//! 🕐 최근 한 시간 — 내 동물이 어디에(집에서 놀기·하우스·친구네) 얼마나 있었는지, 무엇을 먹었는지
//! (규칙은 screenpet_core::timeline, 여기는 창). 기록은 메모리에만 있다: 저장하지 않고 친구에게 보내지 않는다.
use std::sync::Mutex;
use std::time::{Duration, Instant};

use screenpet_core::care::Food;
use screenpet_core::config::Config;
use screenpet_core::manifest::FavoriteFood;
use screenpet_core::timeline::{self, Timeline, TimelineView};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::slots::Slot;

pub const LABEL: &str = "timeline";
/// 처음 크기 (논리 px). 높이는 웹뷰가 내용을 재서 `timeline_fit`으로 맞춘다
const SIZE: (f64, f64) = (380.0, 320.0);
/// 내용에 맞출 때 높이 범위 (논리 px) — 넘치면 창 안에서 스크롤
const FIT_HEIGHT: (f64, f64) = (160.0, 760.0);
/// 화면(작업 영역) 위아래로 남길 틈 (논리 px)
const SCREEN_MARGIN: f64 = 24.0;
/// 창이 열려 있으면 이만큼마다 새 내용을 보낸다
const PUBLISH_EVERY: Duration = Duration::from_secs(1);

/// 창이 처음 열릴 때 읽는 지금 내용 (그 뒤로는 `timeline-state` 이벤트)
static VIEW: Mutex<Option<TimelineView>> = Mutex::new(None);

#[derive(Default)]
pub struct TimelineFx {
    timeline: Timeline,
    last_publish: Option<Instant>,
}

impl TimelineFx {
    pub fn new() -> Self {
        Self::default()
    }

    /// 매 틱. real_dt = 지난 틱부터 실제로 흐른 초 (게임 루프의 dt처럼 자르지 않는다 — 컴퓨터가 잠들었던 긴 틈도 지금 자리에 더한다)
    pub fn tick(&mut self, app: &AppHandle, real_dt: f64, own: &Slot, away_friend: Option<&str>, config: &Config) {
        // 하우스에서 나와 문 앞에서 먹고 다시 들어갈 참이면 하우스로 센다 (잠깐 나온 걸 놀기로 쪼개지 않는다)
        let housed = own.pet.is_housed() || own.pet.returns_home();
        self.timeline.tick(timeline::place_of(housed, own.pet.is_away(), away_friend), real_dt);
        let now = Instant::now();
        if self.last_publish.is_some_and(|t| now - t < PUBLISH_EVERY) {
            return;
        }
        self.last_publish = Some(now);
        if app.get_webview_window(LABEL).is_some() {
            self.publish(app, config);
        }
    }

    /// 내 동물이 먹었다 — 바로 창에 보인다
    pub fn ate(&mut self, app: &AppHandle, food: Food, favorite: &FavoriteFood, config: &Config) {
        self.timeline.ate(food, favorite);
        if app.get_webview_window(LABEL).is_some() {
            self.publish(app, config);
        }
    }

    /// 지금 내용을 창에 보낸다 (창이 없으면 첫 로드용으로 남기기만)
    fn publish(&self, app: &AppHandle, config: &Config) {
        let view = self.timeline.view(&|id| config.person_name(id).map(str::to_string));
        *VIEW.lock().unwrap() = Some(view.clone());
        let _ = app.emit_to(LABEL, "timeline-state", view);
    }

    /// 메뉴 "🕐 최근 한 시간". 이미 열려 있으면 앞으로 가져온다
    pub fn show(&self, app: &AppHandle, config: &Config) {
        self.publish(app, config);
        if crate::overlay::focus_existing(app, LABEL) {
            return;
        }
        let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("timeline.html".into()))
            .title("🕐 최근 한 시간")
            .inner_size(SIZE.0, SIZE.1)
            .resizable(true)
            .maximizable(false)
            .always_on_top(true)
            .center()
            .build();
        match built {
            Ok(window) => {
                let _ = window.set_focus();
            }
            Err(e) => eprintln!("failed to open timeline window: {e}"),
        }
    }
}

/// 타임라인 창의 지금 내용 (첫 로드용)
#[tauri::command]
pub fn timeline_state(window: WebviewWindow) -> Result<TimelineView, String> {
    if window.label() != LABEL {
        return Err("wrong window".into());
    }
    Ok(VIEW.lock().unwrap().clone().unwrap_or_default())
}

/// 타임라인 창 높이를 내용(논리 px)에 맞춘다. 너비는 그대로, 화면 밖으로 나가면 위로 올린다
#[tauri::command]
pub fn timeline_fit(window: WebviewWindow, height: f64) {
    if window.label() != LABEL || !height.is_finite() {
        return;
    }
    let dpi = window.scale_factor().unwrap_or(1.0);
    let (Ok(inner), Ok(outer)) = (window.inner_size(), window.outer_size()) else { return };
    // 제목 표시줄·테두리 (논리 px)
    let frame = (outer.height as f64 - inner.height as f64).max(0.0) / dpi;
    let area = window.current_monitor().ok().flatten().map(|m| *m.work_area());
    let screen_max = area.map_or(f64::MAX, |a| a.size.height as f64 / dpi - frame - 2.0 * SCREEN_MARGIN);
    let h = height.clamp(FIT_HEIGHT.0, FIT_HEIGHT.1.min(screen_max).max(FIT_HEIGHT.0));
    if (h - inner.height as f64 / dpi).abs() < 1.0 {
        return;
    }
    // Windows: tao의 set_size·set_position은 보이는 창을 다시 활성화한다 (다른 앱에서 치던 글이 끊긴다)
    let hwnd = crate::overlay::hwnd_of(&window);
    #[cfg(windows)]
    crate::overlay::resize_quietly(&window, hwnd, outer.width, ((h + frame) * dpi).round() as u32);
    #[cfg(not(windows))]
    let _ = window.set_size(tauri::LogicalSize::new(inner.width as f64 / dpi, h));
    // 커져서 아래가 작업 영역 밖이면 위로 (위쪽 끝보다 위로는 안 간다)
    if let (Some(a), Ok(pos)) = (area, window.outer_position()) {
        let bottom = (a.position.y + a.size.height as i32) as f64 - SCREEN_MARGIN * dpi;
        let over = pos.y as f64 + (h + frame) * dpi - bottom;
        if over > 0.0 {
            let y = (pos.y as f64 - over).max(a.position.y as f64 + SCREEN_MARGIN * dpi);
            crate::overlay::move_quietly(&window, hwnd, pos.x, y.round() as i32);
        }
    }
}
