//! 하우스 창 — 주 모니터 작업 영역 오른쪽 아래 구석 (작업표시줄 바로 위)에 놓인 작은 집.

use screenpet_core::geom::Rect;
use screenpet_core::platform::default_source;
use screenpet_core::surfaces::Owner;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Listener, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::AppState;

pub const LABEL: &str = "house";
/// 집 그림 한 변 (그림 좌표 = teenager 집의 논리 px). 단계마다 house_scale배로 보인다
pub const HOUSE_SIZE: f64 = 128.0;
/// 집 위 말풍선·음식 버튼 자리 (논리 px)
pub const BUBBLE_SPACE: f64 = 36.0;
/// 하우스 창 최소 너비 (논리 px) — 음식 고르기 버튼 셋(92px)이 작은 집 위에서도 잘리지 않게. 집은 창 오른쪽에 붙는다
pub const MIN_WIDTH: f64 = 100.0;
/// 작업 영역 오른쪽 끝과의 간격 (논리 px)
const RIGHT_MARGIN: f64 = 8.0;

/// 단계별 집 크기 배율 (ui/house.js HOUSE_SCALE, tools/draw_house.py HOUSE_SCALE와 같다)
pub fn house_scale(stage: &str) -> f64 {
    match stage {
        "baby" => 0.55,
        "kinder" => 0.68,
        "child" => 0.82,
        "adult" => 1.12,
        _ => 1.0,
    }
}

/// 꿀벌은 벌통에 산다 (ui/house.js는 pet.json locomotion "fly"로도 고른다 — 지금 나는 동물은 꿀벌뿐)
pub fn is_hive(pet: &str) -> bool {
    pet == "bee"
}

/// 집 그림 한 변 (논리 px)
pub fn house_side(stage: &str) -> f64 {
    HOUSE_SIZE * house_scale(stage)
}

/// 하우스 창 크기 (논리 px): 너비 = max(집, MIN_WIDTH), 높이 = 말풍선 자리 + 집
pub fn window_size(stage: &str) -> (f64, f64) {
    let side = house_side(stage);
    (side.max(MIN_WIDTH), side + BUBBLE_SPACE)
}

/// 집 그림(128×128) 안의 문 사각형 (그림 좌표) — ui/house/<house|hive>_<stage>.json `door` {x, y, w, h}.
/// 앱은 문 가운데 x만 쓴다 (문간 그리기는 house.js가 json을 직접 읽는다).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Door {
    pub x: f64,
    pub w: f64,
}

#[derive(Deserialize)]
struct HouseJson {
    door: Door,
}

/// 단계별 집·벌통 그림의 문 (tools/draw_house.py가 만든다)
const DOOR_JSON: [(&str, &str, &str); 10] = [
    ("house", "baby", include_str!("../../ui/house/house_baby.json")),
    ("house", "kinder", include_str!("../../ui/house/house_kinder.json")),
    ("house", "child", include_str!("../../ui/house/house_child.json")),
    ("house", "teenager", include_str!("../../ui/house/house_teenager.json")),
    ("house", "adult", include_str!("../../ui/house/house_adult.json")),
    ("hive", "baby", include_str!("../../ui/house/hive_baby.json")),
    ("hive", "kinder", include_str!("../../ui/house/hive_kinder.json")),
    ("hive", "child", include_str!("../../ui/house/hive_child.json")),
    ("hive", "teenager", include_str!("../../ui/house/hive_teenager.json")),
    ("hive", "adult", include_str!("../../ui/house/hive_adult.json")),
];

/// json이 깨졌을 때 쓰는 문 (teenager 집의 문)
const DEFAULT_DOOR: Door = Door { x: 30.0, w: 52.0 };

pub fn door(pet: &str, stage: &str) -> Door {
    let kind = if is_hive(pet) { "hive" } else { "house" };
    DOOR_JSON
        .iter()
        .find(|(k, s, _)| *k == kind && *s == stage)
        .or_else(|| DOOR_JSON.iter().find(|(k, s, _)| *k == kind && *s == "teenager"))
        .and_then(|(_, _, j)| serde_json::from_str::<HouseJson>(j).ok())
        .map_or(DEFAULT_DOOR, |h| h.door)
}

/// 하우스 창 웹뷰에 보내는 상태 (`house-state` 이벤트, `house_state` 커맨드)
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HouseState {
    pub pet: String,
    pub stage: &'static str,
    pub scale: f64,
    pub hungry: bool,
    /// 동물이 집 안에 있음 (달려오는 중엔 false — 문간이 비어 있다)
    pub housed: bool,
}

/// 집을 놓을 자리 (모두 물리 px)
#[derive(Debug, Clone, Copy)]
pub struct Spot {
    /// 하우스 창 왼쪽 위
    pub window: (i32, i32),
    /// 하우스 창 크기 (논리 px) — 지금 동물의 단계에 맞춘다
    pub size: (f64, f64),
    /// 문 가운데 x
    pub door_x: f64,
    /// 집이 놓인 바닥 (그 작업 영역의 Ground)
    pub ground: Owner,
}

/// 숨긴 채로 하우스 창을 만든다 (보일 때마다 구석에 다시 놓는다)
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    // 게임 루프가 하우스 상태를 바꿀 때마다(house-state) 동물·단계가 바뀌었으면 창 크기를 다시 맞춘다
    let handle = app.clone();
    let last = Mutex::new(shown(app));
    app.listen_any("house-state", move |_| {
        let now = shown(&handle);
        let changed = {
            let mut last = last.lock().unwrap();
            let changed = (is_hive(&now.0), now.1) != (is_hive(&last.0), last.1);
            *last = now;
            changed
        };
        if changed {
            refit(&handle);
        }
    });
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("house.html".into()))
        .title("ScreenPet House")
        .inner_size(HOUSE_SIZE, HOUSE_SIZE + BUBBLE_SPACE)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
}

/// 지금 하우스에 보이는 동물과 단계 (게임 루프가 AppState.house에 적는다)
fn shown(app: &AppHandle) -> (String, &'static str) {
    app.try_state::<AppState>().map_or((String::new(), "teenager"), |s| {
        let h = s.house.lock().unwrap();
        (h.pet.clone(), h.stage)
    })
}

/// 주 모니터 작업 영역의 오른쪽 아래 구석. work_areas = 게임 세계의 작업 영역 (Ground 번호가 같다).
/// 주 모니터를 못 찾으면 첫 작업 영역. 집 크기·문 위치는 지금 동물의 단계 (AppState.house)를 따른다.
pub fn spot(app: &AppHandle, work_areas: &[Rect], fallback_dpi: f64) -> Spot {
    let primary = app.primary_monitor().ok().flatten();
    let primary_rect = primary.as_ref().map(|m| {
        let wa = m.work_area();
        let (l, t) = (wa.position.x as f64, wa.position.y as f64);
        Rect::new(l, t, l + wa.size.width as f64, t + wa.size.height as f64)
    });
    let fallback = [primary_rect.unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0))];
    let work_areas = if work_areas.is_empty() { &fallback[..] } else { work_areas };
    let (index, dpi) = match &primary {
        Some(m) => {
            let wa = m.work_area();
            let cx = wa.position.x as f64 + wa.size.width as f64 / 2.0;
            let cy = wa.position.y as f64 + wa.size.height as f64 / 2.0;
            let i = work_areas.iter().position(|r| r.left <= cx && cx < r.right && r.top <= cy && cy < r.bottom);
            (i.unwrap_or(0), m.scale_factor())
        }
        None => (0, fallback_dpi),
    };
    let wa = work_areas[index.min(work_areas.len() - 1)];
    let (pet, stage) = shown(app);
    let (width, height) = window_size(stage);
    let k = house_scale(stage);
    let left = wa.right - (RIGHT_MARGIN + width) * dpi;
    let top = wa.bottom - height * dpi;
    // 집 그림은 창 오른쪽에 붙는다
    let house_left = left + (width - house_side(stage)) * dpi;
    let d = door(&pet, stage);
    Spot {
        window: (left.round() as i32, top.round() as i32),
        size: (width, height),
        door_x: house_left + (d.x + d.w / 2.0) * k * dpi,
        ground: Owner::Ground(index),
    }
}

/// 단계 크기로 맞춰 구석에 놓는다
fn place(window: &WebviewWindow, spot: &Spot) {
    let _ = window.set_size(LogicalSize::new(spot.size.0, spot.size.1));
    let _ = window.set_position(PhysicalPosition::new(spot.window.0, spot.window.1));
}

/// 구석에 놓고 보인다
pub fn show_at(window: &WebviewWindow, spot: &Spot) {
    place(window, spot);
    let _ = window.show();
}

/// 보이는 하우스 창을 지금 동물·단계 크기로 다시 맞춘다 (성장해서 집이 바뀌었을 때)
fn refit(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    if !window.is_visible().unwrap_or(false) {
        return; // 보일 때 show_at이 맞춘다
    }
    let dpi = window.scale_factor().unwrap_or(1.0);
    let spot = spot(app, &default_source().work_areas(), dpi);
    place(&window, &spot);
}

/// 게임 루프 밖(트레이 "보이기", 두 번째 실행)에서 하우스 창을 지금 구석에 다시 놓고 보인다
pub fn show_in_corner(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let dpi = window.scale_factor().unwrap_or(1.0);
    let spot = spot(app, &default_source().work_areas(), dpi);
    show_at(&window, &spot);
}
