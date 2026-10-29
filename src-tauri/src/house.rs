//! 하우스 창 — 주 모니터 작업 영역 오른쪽 아래 구석 (작업표시줄 바로 위)에 놓인 작은 집.

use screenpet_core::surfaces::Owner;
use screenpet_core::world::World;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

include!(concat!(env!("OUT_DIR"), "/house.rs"));

pub const LABEL: &str = "house";
/// 집 그림 한 변 (논리 px)
pub const HOUSE_SIZE: f64 = 128.0;
/// 집 위 말풍선·음식 버튼 자리 (논리 px)
pub const BUBBLE_SPACE: f64 = 36.0;
/// 작업 영역 오른쪽 끝과의 간격 (논리 px)
const RIGHT_MARGIN: f64 = 8.0;

/// 집 그림(128×128) 안의 문 사각형 (논리 px) — ui/house/house.json `door` {x, y, w, h}.
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

/// house.json이 없거나 깨졌을 때 쓰는 문 (그림 가운데 아래)
const DEFAULT_DOOR: Door = Door { x: 44.0, w: 40.0 };

pub fn door() -> Door {
    HOUSE_JSON.and_then(|s| serde_json::from_str::<HouseJson>(s).ok()).map_or(DEFAULT_DOOR, |h| h.door)
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
    /// 문 가운데 x
    pub door_x: f64,
    /// 집이 놓인 바닥 (그 작업 영역의 Ground)
    pub ground: Owner,
}

/// 숨긴 채로 하우스 창을 만든다 (보일 때마다 구석에 다시 놓는다)
pub fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
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

/// 주 모니터 작업 영역의 오른쪽 아래 구석. 주 모니터를 못 찾으면 첫 작업 영역.
pub fn spot(app: &AppHandle, world: &World, fallback_dpi: f64) -> Spot {
    let primary = app.primary_monitor().ok().flatten();
    let (index, dpi) = match &primary {
        Some(m) => {
            let wa = m.work_area();
            let cx = wa.position.x as f64 + wa.size.width as f64 / 2.0;
            let cy = wa.position.y as f64 + wa.size.height as f64 / 2.0;
            let i = world.work_areas.iter().position(|r| r.left <= cx && cx < r.right && r.top <= cy && cy < r.bottom);
            (i.unwrap_or(0), m.scale_factor())
        }
        None => (0, fallback_dpi),
    };
    let wa = world.work_areas[index.min(world.work_areas.len() - 1)];
    let left = wa.right - (RIGHT_MARGIN + HOUSE_SIZE) * dpi;
    let top = wa.bottom - (HOUSE_SIZE + BUBBLE_SPACE) * dpi;
    let d = door();
    Spot {
        window: (left.round() as i32, top.round() as i32),
        door_x: left + (d.x + d.w / 2.0) * dpi,
        ground: Owner::Ground(index),
    }
}

/// 구석에 놓고 보인다
pub fn show_at(window: &WebviewWindow, spot: &Spot) {
    let _ = window.set_position(PhysicalPosition::new(spot.window.0, spot.window.1));
    let _ = window.show();
}
