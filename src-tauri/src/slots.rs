//! 화면 위 동물 한 마리 = 슬롯 하나 = 투명 창 하나.
//! 내 동물은 "pet" 창(tauri.conf.json), 친구 동물(손님)은 "guest-<visit_id>" 창을 그때그때 만든다.
//! 웹뷰는 자기 창 라벨로 `slot_info`를 불러 자기 동물을 알고, 이벤트는 창별로(emit_to) 받는다.

use screenpet_core::behavior::Special;
use screenpet_core::brain::{Anim, Pet, PetConfig, RenderState};
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
/// 손님 편지(💌)를 펼친 동안 창 위에 더하는 자리와 최소 창 너비 (논리 px) — 편지 말풍선(평소 말풍선, 여러 줄)이 5줄까지 들어간다
pub const NOTE_SPACE: f64 = 60.0;
pub const NOTE_WIDTH: f64 = 232.0;
/// 기린이 목을 늘이는 동안(Anim::NeckStretch, 코어 behavior::NECK_STRETCH_SECS) 창 위에 더하는 자리 (몸 배율 1 기준 논리 px).
/// 머리가 그림 칸 위로 pet.json neck_stretch.ext(≤ 48 그림 px)만큼 솟는다 — 말풍선 자리(BUBBLE_SPACE)와 합쳐 넉넉히 들어간다
pub const STRETCH_SPACE: f64 = 52.0;

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
    /// 주인이 들려 보낸 편지 (다듬은 글자 — 없으면 봉투도 없다)
    pub note: Option<String>,
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
    /// 손님이면 주인 id (우클릭 "<친구>에게 보내기"에서 주인을 뺀다 — 웹뷰에는 보내지 않는다)
    #[serde(skip)]
    pub owner_id: Option<String>,
    /// 손님이 들고 온 편지 (머리 위 💌, 누르면 "💌 <owner>: <note>" — 웹뷰는 textContent로만 쓴다)
    pub note: Option<String>,
    /// 마지막으로 보낸 모습 — 첫 `pet-render`가 웹뷰가 듣기 전에 지나갔을 수 있어 첫 로드 때 같이 준다
    pub render: Option<RenderState>,
}

impl SlotInfo {
    pub fn new(shown: &StagePayload, role: &Role, render: Option<RenderState>) -> Self {
        let (role, owner, owner_id, note) = match role {
            Role::Own => ("own", None, None, None),
            Role::Guest(g) => ("guest", Some(g.owner_name.clone()), Some(g.owner_id.clone()), g.note.clone()),
        };
        Self { pet: shown.pet.clone(), stage: shown.stage, scale: shown.scale, role, owner, owner_id, note, render }
    }
}

/// 펫 창 크기 (논리 px): 너비 128 × max(1, s), 높이 36 + 128 × s
pub fn window_logical_size(body_scale: f64) -> (f64, f64) {
    (PET_LOGICAL_SIZE * body_scale.max(1.0), BUBBLE_SPACE + PET_LOGICAL_SIZE * body_scale)
}

/// 편지를 펼친 동안의 창 크기: 위로 NOTE_SPACE만큼 높고, 너비는 NOTE_WIDTH 이상 (몸은 창 아래 가운데 그대로)
pub fn note_window_logical_size(body_scale: f64) -> (f64, f64) {
    let (w, h) = window_logical_size(body_scale);
    (w.max(NOTE_WIDTH), h + NOTE_SPACE)
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
        flies: m.flies(),
        stings: m.can_sting(),
        hops: m.hops(),
        hop_share: m.hop_share(),
        looks_up: m.can_look_up(),
        dpi,
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
    /// 편지 말풍선을 띄워 창이 커져 있음 (웹뷰 `note_space` — 손님이 들고 온 편지 / 돌아온 내 동물의 답장)
    note_open: bool,
    /// 기린이 목을 늘이는 중이라 창이 위로 커져 있음 (늘이는 동안만 — 끝나면 바로 줄인다)
    stretch_open: bool,
    /// Windows: 창 핸들 (만들 때 한 번 읽어 둔다 — hwnd()는 메인 스레드에 물어본다). 매 틱 SetWindowPos로 바로 옮긴다
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl Slot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(label: String, role: Role, entry: PetEntry, pet: Pet, window: WebviewWindow, world: World, dpi: f64, shown: StagePayload) -> Self {
        #[cfg(windows)]
        let hwnd = window.hwnd().ok().map(|h| h.0 as isize);
        Self {
            label,
            role,
            entry,
            pet,
            window,
            world,
            dpi,
            shown,
            need_show: true,
            last_pos: None,
            last_render: None,
            note_open: false,
            stretch_open: false,
            #[cfg(windows)]
            hwnd,
        }
    }

    /// 창을 물리 px (x, y)로. Windows는 이벤트 루프를 거치지 않고 바로 (창을 따라갈 때 덜 끊긴다), 실패하거나 다른 OS면 Tauri로
    fn move_window(&self, x: i32, y: i32) {
        #[cfg(windows)]
        {
            if self.hwnd.is_some_and(|h| screenpet_core::platform::move_window(h, x, y)) {
                return;
            }
        }
        let _ = self.window.set_position(PhysicalPosition::new(x, y));
    }

    pub fn is_own(&self) -> bool {
        matches!(self.role, Role::Own)
    }

    pub fn info(&self) -> SlotInfo {
        SlotInfo::new(&self.shown, &self.role, self.last_render)
    }

    /// 단계(또는 동물)가 바뀌었다: 창 크기, 펫 크기, 웹뷰 시트
    pub fn set_stage(&mut self, app: &AppHandle, shown: StagePayload) {
        let (w, h) = self.logical_size(shown.scale);
        let _ = self.window.set_size(tauri::LogicalSize::new(w, h));
        self.pet.cfg = pet_config(self.dpi, &self.entry, shown.scale);
        self.shown = shown;
        let info = self.info();
        app.state::<AppState>().slots.lock().unwrap().insert(self.label.clone(), info.clone());
        let _ = app.emit_to(self.label.as_str(), "pet-stage", info);
    }

    /// 지금 창 크기 (논리 px) — 편지를 펼친 동안은 위로 높고 넓다
    fn logical_size(&self, body_scale: f64) -> (f64, f64) {
        let (w, h) = if self.note_open { note_window_logical_size(body_scale) } else { window_logical_size(body_scale) };
        if self.stretch_open {
            (w, h + STRETCH_SPACE * body_scale)
        } else {
            (w, h)
        }
    }

    /// 기린 목 늘이기가 시작·끝나면 창을 위로 키우거나 줄인다 (몸은 창 아래 가운데 그대로 — 바로 다음 place가 옮긴다)
    fn sync_stretch(&mut self) {
        let stretch = self.pet.render().anim == Anim::NeckStretch;
        if stretch == self.stretch_open {
            return;
        }
        self.stretch_open = stretch;
        let (w, h) = self.logical_size(self.shown.scale);
        let _ = self.window.set_size(tauri::LogicalSize::new(w, h));
        self.last_pos = None;
    }

    /// 편지 말풍선을 띄운다 / 접는다: 창을 키우거나 줄이고, 몸이 그 자리에 있도록 바로 다시 놓는다.
    /// 손님은 편지를 들고 왔을 때만, 내 동물은 답장을 들고 돌아왔을 때 (`pet-mail` — 언제 왔는지는 웹뷰가 안다)
    pub fn set_note_open(&mut self, open: bool) {
        let has_note = match &self.role {
            Role::Guest(g) => g.note.is_some(),
            Role::Own => true,
        };
        if self.note_open == open || (open && !has_note) {
            return;
        }
        self.note_open = open;
        let (w, h) = self.logical_size(self.shown.scale);
        let _ = self.window.set_size(tauri::LogicalSize::new(w, h));
        let pos = self.window_target();
        self.move_window(pos.0, pos.1);
        self.last_pos = Some(pos);
    }

    /// 창 왼쪽 위 (물리 px): 몸이 창 아래 가운데에 오도록
    fn window_target(&self) -> (i32, i32) {
        let (_, sy) = self.pet.window_pos();
        let (w, h) = self.logical_size(self.shown.scale);
        let top = h - PET_LOGICAL_SIZE * self.shown.scale;
        ((self.pet.body.x - w * self.dpi / 2.0).round() as i32, (sy - top * self.dpi).round() as i32)
    }

    /// 손님 이름표의 주인 이름을 바꾼다 (친구가 이름을 바꿨거나 내가 별명을 붙임). 달라졌을 때만 웹뷰에 알린다
    pub fn set_owner_name(&mut self, app: &AppHandle, name: String) {
        let Role::Guest(g) = &mut self.role else { return };
        if g.owner_name == name {
            return;
        }
        g.owner_name = name;
        let info = self.info();
        if let Some(stored) = app.state::<AppState>().slots.lock().unwrap().get_mut(&self.label) {
            stored.owner = info.owner.clone();
        }
        // pet-stage는 이름표(applyRole)도 다시 그린다
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

    /// 창을 몸 위치로 옮기고, 처음(또는 다시 나타날 때) 보인다. hidden = 사용자가 트레이로 모두 숨겨 둠 (그동안은 보이지 않는다)
    pub fn place(&mut self, hidden: bool) {
        self.sync_stretch();
        let pos = self.window_target();
        if self.last_pos != Some(pos) {
            self.move_window(pos.0, pos.1);
            self.last_pos = Some(pos);
        }
        if self.need_show && self.on_screen() && !hidden {
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

// 어울림 코디네이터(social.rs)가 슬롯 목록을 그대로 받는다
impl AsRef<Pet> for Slot {
    fn as_ref(&self) -> &Pet {
        &self.pet
    }
}

impl AsMut<Pet> for Slot {
    fn as_mut(&mut self) -> &mut Pet {
        &mut self.pet
    }
}

/// 손님 창: "pet" 창과 같은 설정 (투명·테두리 없음·항상 위·작업표시줄 숨김·포커스 안 받음·숨긴 채 시작)
fn build_guest_window(app: &AppHandle, label: &str, body_scale: f64) -> tauri::Result<WebviewWindow> {
    let (w, h) = window_logical_size(body_scale);
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html?role=guest&slot={label}").into()))
        .title("ScreenPet Guest")
        .inner_size(w, h)
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
