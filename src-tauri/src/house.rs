//! 하우스 창 — 작업 영역 바닥 (작업표시줄 바로 위)에 놓인 작은 집.
//! 처음엔 주 모니터 오른쪽 아래 구석에 있고, 마우스로 끌어 어느 모니터의 바닥 어디로든 옮길 수 있다 (설정에 남는다).

use screenpet_core::config::{clamp_house_left, HouseSpot};
use screenpet_core::geom::Rect;
use screenpet_core::platform::default_source;
use screenpet_core::surfaces::Owner;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Listener, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::game_loop::Input;
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

/// 나는 동물(pet.json locomotion "fly" — 꿀벌)은 벌통에 산다 (ui/house.js와 같은 기준). 번들에 없는 id면 꿀벌만
pub fn is_hive(app: &AppHandle, pet: &str) -> bool {
    let flies = app.try_state::<AppState>().and_then(|s| s.pets.iter().find(|p| p.id == pet).map(|p| p.manifest.flies()));
    flies.unwrap_or(pet == "bee")
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

pub fn door(hive: bool, stage: &str) -> Door {
    let kind = if hive { "hive" } else { "house" };
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
    /// 그 모니터의 배율 — 창 크기(논리 px)를 물리 px로 바꿀 때 (창이 아직 옛 모니터 배율을 갖고 있을 수 있다)
    pub dpi: f64,
}

/// 하우스 웹뷰가 집을 누른 채 5px 넘게 움직였다 (house_drag_start 커맨드, 하우스 창에서만) — 여기서부터 커서를 따라 창을 옮긴다
pub fn start_drag(app: &AppHandle) {
    if !DRAGGING.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        thread::spawn(move || {
            drag(&app);
            DRAGGING.store(false, Ordering::SeqCst);
        });
    }
}

/// 끌어다 놓은 자리 (없으면 주 모니터 구석). 게임 루프가 시작할 때 설정에서 채우고, 끌기가 바꾼다.
/// 설정 파일에 쓰는 것은 게임 루프다 (Input::HouseMoved { save: true }를 받으면 placed()를 저장한다)
static PLACED: Mutex<Option<HouseSpot>> = Mutex::new(None);
/// 지금 집을 끌고 있음 (끌기 스레드는 하나만)
static DRAGGING: AtomicBool = AtomicBool::new(false);
/// 끄는 동안 창을 옮기는 간격
const DRAG_TICK: Duration = Duration::from_millis(16);
/// 끄는 동안 게임 루프에 문 자리를 알리는 간격 (집으로 가는 동물이 새 문을 따라간다)
const DRAG_NOTIFY: Duration = Duration::from_millis(100);

/// 설정에 남아 있던 자리로 (게임 루프가 시작할 때, 첫 spot 전에)
pub fn restore(spot: Option<HouseSpot>) {
    *PLACED.lock().unwrap() = spot;
}

/// 지금 자리 (게임 루프가 설정에 적는다)
pub fn placed() -> Option<HouseSpot> {
    PLACED.lock().unwrap().clone()
}

/// 숨긴 채로 하우스 창을 만든다 (보일 때마다 놓아 둔 자리에 다시 놓는다)
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    // 게임 루프가 하우스 상태를 바꿀 때마다(house-state) 동물·단계가 바뀌었으면 창 크기를 다시 맞춘다
    let handle = app.clone();
    let last = Mutex::new(shown(app));
    app.listen_any("house-state", move |_| {
        let now = shown(&handle);
        let changed = {
            let mut last = last.lock().unwrap();
            let changed = (is_hive(&handle, &now.0), now.1) != (is_hive(&handle, &last.0), last.1);
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
        .accept_first_mouse(true)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .visible(false)
        .build()
        .inspect(crate::overlay::float_everywhere)
}

/// 지금 하우스에 보이는 동물과 단계 (게임 루프가 AppState.house에 적는다)
fn shown(app: &AppHandle) -> (String, &'static str) {
    app.try_state::<AppState>().map_or((String::new(), "teenager"), |s| {
        let h = s.house.lock().unwrap();
        (h.pet.clone(), h.stage)
    })
}

/// 모니터의 작업 영역 (물리 px)
fn work_rect(m: &Monitor) -> Rect {
    let wa = m.work_area();
    let (l, t) = (wa.position.x as f64, wa.position.y as f64);
    Rect::new(l, t, l + wa.size.width as f64, t + wa.size.height as f64)
}

/// 모니터를 찾는 열쇠 (이름, 왼쪽 위)
fn monitor_key(m: &Monitor) -> (&str, (i32, i32)) {
    (m.name().map_or("", |n| n.as_str()), (m.position().x, m.position().y))
}

/// 집을 놓을 자리. work_areas = 게임 세계의 작업 영역 (Ground 번호가 같다).
/// 끌어다 놓은 자리가 있고 그 모니터가 아직 있으면 그 모니터 바닥의 그 x (작업 영역 안으로 당긴다),
/// 아니면 주 모니터 작업 영역의 오른쪽 아래 구석. 모니터를 못 찾으면 첫 작업 영역.
/// 집 크기·문 위치는 지금 동물의 단계 (AppState.house)를 따른다.
pub fn spot(app: &AppHandle, work_areas: &[Rect], fallback_dpi: f64) -> Spot {
    let monitors = app.available_monitors().unwrap_or_default();
    let saved = placed().and_then(|s| {
        let keys: Vec<_> = monitors.iter().map(monitor_key).collect();
        s.pick(&keys).map(|i| (s, monitors[i].clone()))
    });
    let (saved, monitor) = match saved {
        Some((s, m)) => (Some(s), Some(m)),
        None => (None, app.primary_monitor().ok().flatten()),
    };
    let monitor_rect = monitor.as_ref().map(work_rect);
    let fallback = [monitor_rect.unwrap_or(Rect::new(0.0, 0.0, 1920.0, 1080.0))];
    let work_areas = if work_areas.is_empty() { &fallback[..] } else { work_areas };
    let (index, dpi) = match (&monitor, monitor_rect) {
        (Some(m), Some(r)) => {
            let (cx, cy) = ((r.left + r.right) / 2.0, (r.top + r.bottom) / 2.0);
            let i = work_areas.iter().position(|w| w.left <= cx && cx < w.right && w.top <= cy && cy < w.bottom);
            (i.unwrap_or(0), m.scale_factor())
        }
        _ => (0, fallback_dpi),
    };
    let wa = work_areas[index.min(work_areas.len() - 1)];
    let (pet, stage) = shown(app);
    let (width, height) = window_size(stage);
    let k = house_scale(stage);
    let left = match &saved {
        Some(s) => s.left_in(wa.left, wa.right, width * dpi),
        None => wa.right - (RIGHT_MARGIN + width) * dpi,
    };
    let top = wa.bottom - height * dpi;
    // 집 그림은 창 오른쪽에 붙는다
    let house_left = left + (width - house_side(stage)) * dpi;
    let d = door(is_hive(app, &pet), stage);
    Spot {
        window: (left.round() as i32, top.round() as i32),
        size: (width, height),
        door_x: house_left + (d.x + d.w / 2.0) * k * dpi,
        ground: Owner::Ground(index),
        dpi,
    }
}

/// monitors 중 커서(물리 px)가 있는 모니터 — 모니터 사이 틈이면 가장 가까운 모니터
fn monitor_at(monitors: &[Monitor], x: f64, y: f64) -> Option<&Monitor> {
    let distance = |m: &Monitor| {
        let (p, s) = (m.position(), m.size());
        let dx = (p.x as f64 - x).max(x - (p.x as f64 + s.width as f64)).max(0.0);
        let dy = (p.y as f64 - y).max(y - (p.y as f64 + s.height as f64)).max(0.0);
        dx * dx + dy * dy
    };
    monitors.iter().min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// 커서가 있는 모니터의 바닥, 창 왼쪽 left (물리 px, 창 너비 width)를 지금 자리로 적는다 (작업 영역 안으로 당겨서)
fn set_placed(monitors: &[Monitor], cursor: (f64, f64), left: f64, width: f64) {
    let Some(m) = monitor_at(monitors, cursor.0, cursor.1) else { return };
    let r = work_rect(m);
    let (name, monitor_pos) = monitor_key(m);
    let offset = clamp_house_left(left, width, r.left, r.right) - r.left;
    restore(Some(HouseSpot { monitor: name.to_string(), monitor_pos, offset }));
}

fn send(app: &AppHandle, input: Input) {
    if let Some(state) = app.try_state::<AppState>() {
        let _ = state.tx.lock().unwrap().send(input);
    }
}

/// 왼쪽 버튼을 놓을 때까지 창이 커서를 따라간다 (잡은 자리 그대로). 놓으면 그 모니터의 바닥으로 내려앉고
/// 게임 루프가 새 자리를 설정에 적는다. 끄는 동안에도 문 자리를 알려 집으로 가는 동물이 따라간다.
fn drag(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let (Ok(start), Ok(size)) = (window.outer_position(), window.outer_size()) else { return };
    let mut source = default_source();
    let (cx, cy) = source.cursor();
    let grab = (cx - start.x as f64, cy - start.y as f64);
    let mut cursor = (cx, cy);
    let mut notified = Instant::now();
    // 모니터 목록은 끌기 시작할 때 한 번 읽는다 (매 틱 메인 스레드에 묻지 않는다)
    let monitors = app.available_monitors().unwrap_or_default();
    while source.left_button_down() {
        thread::sleep(DRAG_TICK);
        cursor = source.cursor();
        let (x, y) = (cursor.0 - grab.0, cursor.1 - grab.1);
        let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
        set_placed(&monitors, cursor, x, size.width as f64);
        if notified.elapsed() >= DRAG_NOTIFY {
            notified = Instant::now();
            send(app, Input::HouseMoved { save: false });
        }
    }
    // 놓았다: 커서가 있는 모니터의 바닥으로 (크기는 그 모니터 배율로 다시 맞춘다)
    // 끄는 사이 모니터 구성이 바뀌었을 수 있다 — 놓을 때 한 번 더 읽는다
    let monitors = app.available_monitors().unwrap_or(monitors);
    set_placed(&monitors, cursor, cursor.0 - grab.0, size.width as f64);
    // 놓은 모니터의 배율 (창의 배율은 아직 옛 모니터 것일 수 있다)
    let dpi = monitor_at(&monitors, cursor.0, cursor.1).map(|m| m.scale_factor()).unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));
    let spot = spot(app, &source.work_areas(), dpi);
    place(&window, &spot);
    send(app, Input::HouseMoved { save: true });
}

/// 단계 크기로 맞춰 놓아 둔 자리에 놓는다
fn place(window: &WebviewWindow, spot: &Spot) {
    // 그 자리 모니터의 배율로 물리 크기를 정한다 (LogicalSize는 창의 지금 배율로 바뀌어 모니터를 옮길 때 틀린다)
    let (w, h) = (spot.size.0 * spot.dpi, spot.size.1 * spot.dpi);
    let _ = window.set_size(PhysicalSize::new(w.round() as u32, h.round() as u32));
    let _ = window.set_position(PhysicalPosition::new(spot.window.0, spot.window.1));
}

/// 놓아 둔 자리에 놓고 보인다
pub fn show_at(window: &WebviewWindow, spot: &Spot) {
    place(window, spot);
    let _ = window.show();
}

/// 보이는 하우스 창을 지금 동물·단계 크기로 다시 맞춘다 (성장해서 집이 바뀌었을 때)
fn refit(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    if !window.is_visible().unwrap_or(false) || DRAGGING.load(Ordering::SeqCst) {
        return; // 보일 때 show_at이, 끄는 중이면 놓을 때 drag가 맞춘다
    }
    let dpi = window.scale_factor().unwrap_or(1.0);
    let spot = spot(app, &default_source().work_areas(), dpi);
    place(&window, &spot);
}

/// 게임 루프 밖(트레이 "보이기", 두 번째 실행)에서 하우스 창을 놓아 둔 자리에 다시 놓고 보인다
pub fn show_in_corner(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else { return };
    let dpi = window.scale_factor().unwrap_or(1.0);
    let spot = spot(app, &default_source().work_areas(), dpi);
    show_at(&window, &spot);
}
