//! 화면 위 동물 한 마리 = 슬롯 하나 = 투명 창 하나.
//! 내 동물은 "pet" 창(tauri.conf.json), 친구 동물(손님)은 "guest-<visit_id>" 창을 그때그때 만든다.
//! 웹뷰는 자기 창 라벨로 `slot_info`를 불러 자기 동물을 알고, 이벤트는 창별로(emit_to) 받는다.

use screenpet_core::behavior::Special;
use screenpet_core::brain::{Pet, PetConfig, RenderState};
use screenpet_core::care::Stage;
use screenpet_core::geom::{Rect, WindowInfo};
use screenpet_core::visit::{self, Side};
use screenpet_core::world::World;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::pets::PetEntry;
use crate::AppState;

/// 내 동물 창
pub const OWN_LABEL: &str = "pet";
const GUEST_PREFIX: &str = "guest-";
/// teenager(배율 1.0) 스프라이트 한 변 (논리 px)
pub const PET_LOGICAL_SIZE: f64 = 128.0;
/// 몸 폭 절반 / 스프라이트 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 스프라이트 위 말풍선·음식 버튼·이름표 자리 (논리 px)
pub const BUBBLE_SPACE: f64 = 36.0;

/// 손님 창 라벨. visit_id는 Tauri 창 라벨 글자(영숫자와 - / : _)만 — 아니면 None
pub fn guest_label(visit_id: &str) -> Option<String> {
    let valid = !visit_id.is_empty() && visit_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_'));
    valid.then(|| format!("{GUEST_PREFIX}{visit_id}"))
}

pub fn is_guest_label(label: &str) -> bool {
    label.starts_with(GUEST_PREFIX)
}

/// 지금 보이는 시트와 몸 크기 배율 (성장 직후 먹는 동안엔 돌보기 단계보다 한 단계 늦다)
#[derive(Debug, Clone, PartialEq)]
pub struct StagePayload {
    pub pet: String,
    /// 시트 이름 (`sheet_<stage>.png`)
    pub stage: &'static str,
    /// 몸 크기 배율 = 단계 배율 × 동물별 배율
    pub scale: f64,
}

/// 놀러 온 친구 동물
#[derive(Debug, Clone)]
pub struct GuestInfo {
    pub visit_id: String,
    pub owner_id: String,
    pub owner_name: String,
}

#[derive(Debug, Clone)]
pub enum Role {
    Own,
    Guest(GuestInfo),
}

/// 웹뷰가 아는 자기 동물 (`slot_info` 커맨드, `pet-stage` 이벤트)
#[derive(Debug, Clone, Serialize)]
pub struct SlotInfo {
    pub pet: String,
    pub stage: &'static str,
    pub scale: f64,
    /// "own" | "guest"
    pub role: &'static str,
    /// 손님이면 주인 이름 (이름표 "🏠 <owner>")
    pub owner: Option<String>,
    /// 마지막으로 보낸 모습 — 첫 `pet-render`가 웹뷰가 듣기 전에 지나갔을 수 있어 첫 로드 때 같이 준다
    pub render: Option<RenderState>,
}

impl SlotInfo {
    pub fn new(shown: &StagePayload, role: &Role, render: Option<RenderState>) -> Self {
        let (role, owner) = match role {
            Role::Own => ("own", None),
            Role::Guest(g) => ("guest", Some(g.owner_name.clone())),
        };
        Self { pet: shown.pet.clone(), stage: shown.stage, scale: shown.scale, role, owner, render }
    }
}

/// 펫 창 크기 (논리 px): 너비 128 × max(1, s), 높이 36 + 128 × s
pub fn window_logical_size(body_scale: f64) -> (f64, f64) {
    (PET_LOGICAL_SIZE * body_scale.max(1.0), BUBBLE_SPACE + PET_LOGICAL_SIZE * body_scale)
}

pub fn pet_config(dpi: f64, entry: &PetEntry, body_scale: f64) -> PetConfig {
    let m = &entry.manifest;
    let size = PET_LOGICAL_SIZE * body_scale * dpi;
    PetConfig {
        size,
        half_w: size * HALF_WIDTH_RATIO,
        // 큰 몸은 조금 빠르게, 작은 몸은 조금 느리게 (크기에 정비례하면 너무 차이 난다)
        walk_speed: m.walk_speed * body_scale.sqrt() * dpi,
        special: Special::from_manifest(|n| m.has_anim(n)),
        personality: m.personality,
        can_climb: m.can_climb(),
    }
}

pub struct Slot {
    pub label: String,
    pub role: Role,
    pub entry: PetEntry,
    pub pet: Pet,
    pub window: WebviewWindow,
    /// 이 동물 키에 맞춘 세계 (디딤판은 몸 크기에 따라 다르다)
    pub world: World,
    pub dpi: f64,
    /// 화면에 적용된 시트·몸 배율
    pub shown: StagePayload,
    /// 창은 visible:false로 만들고, 위치를 잡은 뒤에야 보인다 (OS 기본 위치 깜빡임 방지)
    pub need_show: bool,
    last_pos: Option<(i32, i32)>,
    last_render: Option<RenderState>,
}

impl Slot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(label: String, role: Role, entry: PetEntry, pet: Pet, window: WebviewWindow, world: World, dpi: f64, shown: StagePayload) -> Self {
        Self { label, role, entry, pet, window, world, dpi, shown, need_show: true, last_pos: None, last_render: None }
    }

    pub fn is_own(&self) -> bool {
        matches!(self.role, Role::Own)
    }

    pub fn info(&self) -> SlotInfo {
        SlotInfo::new(&self.shown, &self.role, self.last_render)
    }

    /// 단계(또는 동물)가 바뀌었다: 창 크기, 펫 크기, 웹뷰 시트
    pub fn set_stage(&mut self, app: &AppHandle, shown: StagePayload) {
        let (w, h) = window_logical_size(shown.scale);
        let _ = self.window.set_size(tauri::LogicalSize::new(w, h));
        self.pet.cfg = pet_config(self.dpi, &self.entry, shown.scale);
        self.shown = shown;
        let info = self.info();
        app.state::<AppState>().slots.lock().unwrap().insert(self.label.clone(), info.clone());
        let _ = app.emit_to(self.label.as_str(), "pet-stage", info);
    }

    /// 화면 배율과 세계를 다시 읽는다 (WORLD_REFRESH마다)
    pub fn refresh(&mut self, windows: &[WindowInfo], work_areas: &[Rect]) {
        let dpi = self.window.scale_factor().unwrap_or(self.dpi);
        if dpi != self.dpi {
            self.dpi = dpi;
            self.pet.cfg = pet_config(dpi, &self.entry, self.shown.scale);
        }
        self.world = World::new(windows.to_vec(), work_areas.to_vec(), self.pet.cfg.size);
    }

    /// 커서가 스프라이트 위에 있음. 걷는 펫이 커서 밑에서 빠져나가지 않도록 사방 20% 여유를 둔다
    pub fn cursor_inside(&self, cx: f64, cy: f64) -> bool {
        let (sx, sy) = self.pet.window_pos();
        let side = self.pet.cfg.size;
        let pad = side * 0.2;
        cx >= sx - pad && cx < sx + side + pad && cy >= sy - pad && cy < sy + side + pad
    }

    /// 화면 위에 있어야 하는 동물 (하우스 안이거나 친구 화면에 가 있으면 아니다)
    pub fn on_screen(&self) -> bool {
        !self.pet.is_housed() && !self.pet.is_away()
    }

    /// 창을 몸 위치로 옮기고, 처음(또는 다시 나타날 때) 보인다
    pub fn place(&mut self) {
        let (_, sy) = self.pet.window_pos();
        let win_w = PET_LOGICAL_SIZE * self.shown.scale.max(1.0) * self.dpi;
        let pos = ((self.pet.body.x - win_w / 2.0).round() as i32, (sy - BUBBLE_SPACE * self.dpi).round() as i32);
        if self.last_pos != Some(pos) {
            let _ = self.window.set_position(PhysicalPosition::new(pos.0, pos.1));
            self.last_pos = Some(pos);
        }
        if self.need_show && self.on_screen() {
            let _ = self.window.show();
            self.need_show = false;
        }
    }

    /// 지난번과 달라졌으면 지금 그릴 모습. 새로 뜬 웹뷰(`slot_info`)를 위해 AppState에도 적어 둔다 — emit보다 먼저
    pub fn take_render(&mut self, app: &AppHandle) -> Option<RenderState> {
        let render = self.pet.render();
        if self.last_render == Some(render) {
            return None;
        }
        self.last_render = Some(render);
        if let Some(info) = app.state::<AppState>().slots.lock().unwrap().get_mut(&self.label) {
            info.render = Some(render);
        }
        Some(render)
    }
}

/// 손님 창: "pet" 창과 같은 설정 (투명·테두리 없음·항상 위·작업표시줄 숨김·포커스 안 받음·숨긴 채 시작)
fn build_guest_window(app: &AppHandle, label: &str, body_scale: f64) -> tauri::Result<WebviewWindow> {
    let (w, h) = window_logical_size(body_scale);
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html?role=guest&slot={label}").into()))
        .title("ScreenPet Guest")
        .inner_size(w, h)
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

/// 친구 동물이 화면 끝(side)에서 걸어 들어온다. 집(주인 화면)은 들어온 쪽이다.
/// scale = 주인 쪽에서 정한 몸 배율 (단계 × 동물별). base = 지금 세계 (창·작업 영역만 쓴다). 게임 루프 스레드에서만 부른다 (창 만들기가 메인 스레드를 기다린다)
#[allow(clippy::too_many_arguments)]
pub fn spawn_guest(
    app: &AppHandle,
    guest: GuestInfo,
    entry: PetEntry,
    stage: Stage,
    scale: f64,
    side: Side,
    base: &World,
    seed: u64,
) -> Result<Slot, String> {
    let label = guest_label(&guest.visit_id).ok_or_else(|| format!("invalid visit id {:?}", guest.visit_id))?;
    let role = Role::Guest(guest);
    let shown = StagePayload { pet: entry.id.clone(), stage: stage.name(), scale };
    // 웹뷰가 뜨자마자 slot_info를 부르므로 창보다 먼저 적어 둔다. 같은 라벨이 이미 있으면 (같은 방문이 두 번) 거절
    let state = app.state::<AppState>();
    {
        let mut infos = state.slots.lock().unwrap();
        if infos.contains_key(&label) || app.get_webview_window(&label).is_some() {
            return Err(format!("{label} already exists"));
        }
        infos.insert(label.clone(), SlotInfo::new(&shown, &role, None));
    }
    let window = match build_guest_window(app, &label, scale) {
        Ok(w) => w,
        Err(e) => {
            // 위에서 우리가 넣은 것만 지운다 (같은 라벨은 없다고 확인했다)
            state.slots.lock().unwrap().remove(&label);
            return Err(e.to_string());
        }
    };
    let dpi = window.scale_factor().unwrap_or(1.0);
    let cfg = pet_config(dpi, &entry, scale);
    let world = World::new(base.windows.clone(), base.work_areas.clone(), cfg.size);
    let mut pet = Pet::new_entering(cfg, &world, side, seed);
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x9e37_79b9_7f4a_7c15);
    pet.set_guest(side, visit::pick_homesick_after(&mut rng));
    Ok(Slot::new(label, role, entry, pet, window, world, dpi, shown))
}

/// 손님이 떠났다 — 창을 닫는다
pub fn close(app: &AppHandle, slot: Slot) {
    app.state::<AppState>().slots.lock().unwrap().remove(&slot.label);
    let _ = slot.window.destroy();
}

pub fn find(slots: &[Slot], label: &str) -> Option<usize> {
    slots.iter().position(|s| s.label == label)
}

/// 커서 아래 있는 맨 위 슬롯 — 나중에 만든 창이 위에 있으므로 뒤에서부터 찾는다
pub fn hover_target(slots: &[Slot], cx: f64, cy: f64) -> Option<usize> {
    slots.iter().rposition(|s| s.on_screen() && s.cursor_inside(cx, cy))
}
