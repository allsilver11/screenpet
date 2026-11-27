//! 화면 위 동물 한 마리 = 슬롯 하나 = 투명 창 하나.
//! 내 첫 펫(대표)은 "pet" 창(tauri.conf.json), 🐾🐾🐾 둘째·셋째는 "pet-2"·"pet-3" 창, 친구 동물(손님)은 "guest-<visit_id>" 창을 그때그때 만듦.
//! slots 맨 앞은 내 펫들 (Role::Own(uid)), 그 뒤는 손님.
//! 웹뷰는 자기 창 라벨로 `slot_info`를 불러 자기 동물을 알고, 이벤트는 창별로(emit_to) 받음.

use screenpet_core::brain::{Pet, PetConfig, RenderState};
use screenpet_core::care::Stage;
use screenpet_core::geom::{Rect, WindowInfo};
use screenpet_core::tint::BodyTint;
use screenpet_core::visit::{self, Side};
use screenpet_core::world::World;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::pets::PetEntry;
use crate::AppState;

/// 내 동물 창 (첫 펫 — 대표)
pub const OWN_LABEL: &str = "pet";
/// 🐾🐾🐾 둘째·셋째 펫 창 라벨 앞부분 (뒤는 2·3)
pub const OWN_PREFIX: &str = "pet-";
/// 손님 창 라벨 앞부분
const GUEST_PREFIX: &str = "guest-";
pub use screenpet_core::brain::PET_LOGICAL_SIZE;
/// 스프라이트 위 말풍선·음식 버튼·이름표 자리 (논리 px)
pub const BUBBLE_SPACE: f64 = 36.0;
/// 손님 편지(💌)를 펼친 동안 창 위에 더하는 자리와 최소 창 너비 (논리 px) — 편지 말풍선(평소 말풍선, 여러 줄)이 5줄까지 들어감
pub const NOTE_SPACE: f64 = 60.0;
/// 편지를 펼친 동안의 최소 창 너비 (논리 px)
pub const NOTE_WIDTH: f64 = 232.0;
/// 루돌프 코·호박 빛이 몸 아래로 번지는 자리 (논리 px): 들어 올린 채 빛나는 동안만 창 아래를 이만큼 늘림
/// (바닥에 서 있을 땐 늘리지 않음 — 그 투명한 띠가 바닥 밑 작업 표시줄의 클릭을 막음)
pub const GLOW_SPACE: f64 = 24.0;
/// 누르면 빛나는 동물과 빛이 이어지는 시간 (ui/main.js PET_FX, style.css 애니메이션 2.2초 안팎)
const GLOW_PETS: [&str; 2] = ["rudolph", "pumpkin"];
/// 빛이 이어지는 시간 (초)
const GLOW_SECS: f64 = 2.6;
/// 보여야 할 창이 정말 보이는지 이만큼(place 횟수 — 60 Hz면 1초)마다 OS에 물음
const SHOWN_CHECK_TICKS: u32 = 60;

/// 손님 창 라벨. visit_id는 Tauri 창 라벨 글자(영숫자와 - / : _)만 — 아니면 None
pub fn guest_label(visit_id: &str) -> Option<String> {
    let valid = !visit_id.is_empty() && visit_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | ':' | '_'));
    valid.then(|| format!("{GUEST_PREFIX}{visit_id}"))
}

/// 손님 창 라벨인지 봄
pub fn is_guest_label(label: &str) -> bool {
    label.starts_with(GUEST_PREFIX)
}

/// 내 펫 창 라벨인지 봄 ("pet", "pet-2", "pet-3" …)
pub fn is_own_label(label: &str) -> bool {
    label == OWN_LABEL || label.strip_prefix(OWN_PREFIX).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// 새 펫에게 줄 빈 창 라벨 ("pet-2"부터, 쓰는 것은 건너뜀 — 닫는 중이라 아직 남은 창(destroy는 나중에 끝남)의 라벨도)
pub fn free_own_label(app: &AppHandle, slots: &[Slot]) -> String {
    (2..)
        .map(|n| format!("{OWN_PREFIX}{n}"))
        .find(|l| !slots.iter().any(|s| &s.label == l) && app.get_webview_window(l).is_none())
        .expect("an endless range always has a free label")
}

/// 🐾🐾🐾 내 펫 중 하나라도 지금 손님·물건·편지를 받지 않음 (집중 중·하우스 안 — 하우스·집중은 모두 한 번에)
pub fn own_refuses(slots: &[Slot]) -> bool {
    slots.iter().take_while(|s| s.is_own()).any(|s| s.pet.refuses_arrivals())
}

/// 지금 보이는 시트와 몸 크기 배율 (성장 직후 먹는 동안엔 돌보기 단계보다 한 단계 늦음)
#[derive(Debug, Clone, PartialEq)]
pub struct StagePayload {
    /// 동물 id
    pub pet: String,
    /// 시트 이름 (`sheet_<stage>.png`)
    pub stage: Stage,
    /// 몸 크기 배율: 단계 배율 × 동물별 배율 × 펫 크기
    pub scale: f64,
    /// 펫 크기를 곱하기 전 몸 크기 배율 — 친구에게 보내는 값, 하우스 문간
    pub base: f64,
}

/// stage 단계의 시트·크기.
/// size = 크기 조절 창의 펫 크기 배율 (친구에게 보내는 base에는 들어가지 않음 — 내 화면에만)
pub fn stage_payload(entry: &PetEntry, stage: Stage, size: f64) -> StagePayload {
    let base = stage.scale() * entry.manifest.size_mult(stage);
    StagePayload { pet: entry.id.clone(), stage, scale: base * size, base }
}

/// 놀러 온 친구 동물
#[derive(Debug, Clone)]
pub struct GuestInfo {
    /// 방문 id
    pub visit_id: String,
    /// 주인 친구 id
    pub owner_id: String,
    /// 주인 이름
    pub owner_name: String,
    /// 주인이 들려 보낸 편지 (다듬은 글자 — 없으면 봉투도 없음)
    pub note: Option<String>,
    /// 밥을 줘서 잠깐 키울 수 있음 (주인이 성장 멈추기를 켰으면 false)
    pub can_grow: bool,
    /// 🎨 주인이 고른 몸 색 (Visit.tint를 확인한 것 — 친구 화면에서도 같은 색)
    pub tint: BodyTint,
    /// 🐾🐾🐾 주인의 몇째 펫인지 (Visit.pet, 없으면 0) — 한 친구의 펫은 번호마다 하나씩 받음
    pub pet: u8,
}

/// 창에 든 동물이 누구 것인지임
#[derive(Debug, Clone)]
pub enum Role {
    /// 내 펫 (설정 pets의 uid)
    Own(String),
    /// 놀러 온 친구 동물
    Guest(GuestInfo),
}

/// 🐾🐾🐾 내 펫마다 게임 루프가 틱 사이에 기억해 두는 것 (손님은 쓰지 않음)
#[derive(Debug, Clone, Default)]
pub struct OwnTrack {
    /// 먹고 한 단계 컸지만 아직 먹는 중 — 먹기가 끝나면 크기를 바꿈
    pub pending_stage: bool,
    /// 지난 틱에 하우스 안이었음
    pub was_housed: bool,
    /// 지난 틱에 친구 화면에 가 있었음
    pub was_away: bool,
    /// 🚗 탈것째 친구 화면으로 막 나갔음 (다음 own_exited가 Visit에 실음)
    pub ride: Option<screenpet_core::vehicle::VehicleKind>,
    /// 성장 멈추기 중 (게임 루프가 매 틱 돌보기에서 맞춤 — 친구에게 키우지 말라고 알림)
    pub locked: bool,
    /// 이름 차례 (설정 자리, 전체 펫 수 — 💤 쉬는 펫도 셈. 게임 루프가 매 틱 맞춤 — own_name)
    pub ord: (usize, usize),
}

/// 웹뷰가 아는 자기 동물 (`slot_info` 커맨드, `pet-stage` 이벤트)
#[derive(Debug, Clone, Serialize)]
pub struct SlotInfo {
    /// 동물 id
    pub pet: String,
    /// 시트 단계
    pub stage: Stage,
    /// 몸 크기 배율
    pub scale: f64,
    /// "own" | "guest"
    pub role: &'static str,
    /// 손님이면 주인 이름 (이름표 "🏠 <owner>")
    pub owner: Option<String>,
    /// 손님이면 주인 id (우클릭 "<친구>에게 보내기"에서 주인을 뺌 — 웹뷰에는 보내지 않음)
    #[serde(skip)]
    pub owner_id: Option<String>,
    /// 손님이 들고 온 편지 (머리 위 💌, 누르면 "💌 <owner>: <note>" — 웹뷰는 textContent로만 씀)
    pub note: Option<String>,
    /// 손님에게 밥을 줘서 키울 수 있음 (우클릭 메뉴 — 웹뷰에는 보내지 않음)
    #[serde(skip)]
    pub can_grow: bool,
    /// 마지막으로 보낸 모습 — 첫 `pet-render`가 웹뷰가 듣기 전에 지나갔을 수 있어 첫 로드 때 같이 줌
    pub render: Option<RenderState>,
    /// 🎨 몸 색 id (잉크 선 동물만 칠함 — ui/main.js setupTint)
    pub tint: BodyTint,
    /// 💌 내 펫이 들고 돌아온 아직 안 읽은 답장 (다시 읽은 페이지도 봉투를 다시 띄움 — 읽으면 NoteSpace가 지움)
    pub mail: Option<crate::network::PetMail>,
}

impl SlotInfo {
    /// 지금 모습과 역할로 웹뷰용 정보를 만듦
    pub fn new(shown: &StagePayload, role: &Role, render: Option<RenderState>) -> Self {
        let (role, owner, owner_id, note, can_grow, tint) = match role {
            Role::Own(_) => ("own", None, None, None, false, BodyTint::Default),
            Role::Guest(g) => ("guest", Some(g.owner_name.clone()), Some(g.owner_id.clone()), g.note.clone(), g.can_grow, g.tint),
        };
        Self { pet: shown.pet.clone(), stage: shown.stage, scale: shown.scale, role, owner, owner_id, note, can_grow, render, tint, mail: None }
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

/// 동물 매니페스트로 두뇌 설정을 만듦 (몸 크기 배율·단계 반영)
pub fn pet_config(dpi: f64, entry: &PetEntry, body_scale: f64, stage: Stage) -> PetConfig {
    PetConfig::from_manifest(&entry.manifest, dpi, body_scale, stage.name())
}

/// 동물 창 하나임: 동물 두뇌, 창, 크기·배율, 창 맞추기 상태
pub struct Slot {
    /// 친구가 밥을 줘서 일시적으로 커진 배율 (1 = 평소)
    pub boost: f64,
    /// 지금 몸 크기에 실제로 곱해진 값 (되돌릴 때 나눔)
    boost_applied: f64,
    /// 창 라벨
    pub label: String,
    /// 내 동물인지 손님인지
    pub role: Role,
    /// 동물 종류
    pub entry: PetEntry,
    /// 동물 두뇌 (코어)
    pub pet: Pet,
    /// 동물 창
    pub window: WebviewWindow,
    /// 이 동물 키에 맞춘 세계 (디딤판은 몸 크기에 따라 다름)
    pub world: World,
    /// 창이 있는 모니터 배율
    pub dpi: f64,
    /// 화면에 적용된 시트·몸 배율
    pub shown: StagePayload,
    /// 창은 visible:false로 만들고, 위치를 잡은 뒤에야 보임 (OS 기본 위치 깜빡임 방지)
    pub need_show: bool,
    /// place를 부른 수 (창이 정말 보이는지 가끔 OS에 물음 — check_shown_due)
    shown_checks: u32,
    last_pos: Option<(i32, i32)>,
    last_render: Option<RenderState>,
    /// 편지 말풍선을 띄워 창이 커져 있음 (웹뷰 `note_space` — 손님이 들고 온 편지 / 돌아온 내 동물의 답장)
    note_open: bool,
    /// 빛나는 동안 창 아래를 늘렸음 / 언제까지
    glow_open: bool,
    glow_until: Option<std::time::Instant>,
    /// 커서에 매달리느라 창이 마우스를 통과시키고 있음 (그동안 클릭은 사용자 것)
    click_through: bool,
    /// 크기 조절 창의 펫 크기 배율
    size: f64,
    /// 🎨 몸 색 (내 동물은 설정, 손님은 주인이 보낸 것)
    pub tint: BodyTint,
    /// 🐾🐾🐾 내 펫마다 틱 사이에 기억해 두는 것
    pub track: OwnTrack,
    /// Windows: 창 핸들 (만들 때 한 번 읽어 둠 — hwnd()는 메인 스레드에 물어봄). 매 틱 SetWindowPos로 바로 옮김
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl Slot {
    /// 새 동물 창 슬롯을 만듦 (커서 종류·크기를 맞춤)
    #[allow(clippy::too_many_arguments)]
    pub fn new(label: String, role: Role, entry: PetEntry, mut pet: Pet, window: WebviewWindow, world: World, dpi: f64, shown: StagePayload, size: f64) -> Self {
        pet.set_cursor_species(&entry.id);
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
            boost: 1.0,
            boost_applied: 1.0,
            need_show: true,
            shown_checks: 0,
            last_pos: None,
            last_render: None,
            note_open: false,
            glow_open: false,
            glow_until: None,
            click_through: false,
            size,
            tint: BodyTint::Default,
            track: OwnTrack::default(),
            #[cfg(windows)]
            hwnd,
        }
    }

    /// 창을 물리 px (x, y)로. Windows는 이벤트 루프를 거치지 않고 바로 (창을 따라갈 때 덜 끊김), 실패하거나 다른 OS면 Tauri로
    fn move_window(&self, x: i32, y: i32) {
        #[cfg(windows)]
        {
            if self.hwnd.is_some_and(|h| screenpet_core::platform::move_window(h, x, y)) {
                return;
            }
        }
        let _ = self.window.set_position(PhysicalPosition::new(x, y));
    }

    /// 내 동물인지 봄
    pub fn is_own(&self) -> bool {
        matches!(self.role, Role::Own(_))
    }

    /// 메뉴·타임라인에 보일 내 펫 이름 ("둘째 고양이") — 차례는 설정 자리라 💤 쉬는 펫이 있어도 그대로
    pub fn own_name(&self) -> String {
        crate::tray::own_pet_name(self.track.ord.0, self.track.ord.1, &self.entry.manifest.name)
    }

    /// 내 펫이면 그 uid
    pub fn uid(&self) -> Option<&str> {
        match &self.role {
            Role::Own(uid) => Some(uid),
            Role::Guest(_) => None,
        }
    }

    /// 🐾🐾🐾 두 슬롯의 창을 맞바꿈 (대표 펫을 보내 주면 다음 펫이 "pet" 창을 물려받음). 두 창 모두 다음 place에서 제자리로 옮겨 다시 보임
    pub fn swap_window(&mut self, other: &mut Slot) {
        std::mem::swap(&mut self.label, &mut other.label);
        std::mem::swap(&mut self.window, &mut other.window);
        #[cfg(windows)]
        std::mem::swap(&mut self.hwnd, &mut other.hwnd);
        for s in [self, other] {
            s.last_pos = None;
            s.last_render = None;
            s.need_show = true;
            s.note_open = false;
            s.glow_open = false;
            s.click_through = false;
            crate::overlay::set_click_through(&s.window, false);
            // 놀러 간·하우스 안 펫이 받은 창은 숨김 (place는 숨기지 않음 — 돌아오면 need_show로 다시 보임)
            if !s.on_screen() {
                crate::overlay::hide_quietly(&s.window);
            }
        }
    }

    /// 웹뷰용 정보를 지금 값으로 만듦
    pub fn info(&self) -> SlotInfo {
        SlotInfo { tint: self.tint, ..SlotInfo::new(&self.shown, &self.role, self.last_render) }
    }

    /// 🎨 몸 색을 바꿈: 웹뷰가 pet-stage로 다시 읽음 (같은 색이면 아무것도 안 함)
    pub fn set_tint(&mut self, app: &AppHandle, tint: BodyTint) {
        if self.tint == tint {
            return;
        }
        self.tint = tint;
        let info = self.info();
        if let Some(stored) = app.state::<AppState>().slots.lock().unwrap().get_mut(&self.label) {
            stored.tint = tint;
        }
        let _ = app.emit_to(self.label.as_str(), "pet-stage", info);
    }

    /// 단계(또는 동물)가 바뀌었음: 창 크기, 펫 크기, 웹뷰 시트
    pub fn set_stage(&mut self, app: &AppHandle, mut shown: StagePayload) {
        // 친구가 밥을 줘서 일시적으로 커졌으면 어떤 그림·단계든 그만큼 크게
        self.boost_applied = self.boost_factor(&shown);
        shown.scale *= self.boost_applied;
        let (w, h) = self.logical_size(shown.scale);
        self.resize_window(w, h);
        self.pet.cfg = pet_config(self.dpi, &self.entry, shown.scale, shown.stage);
        self.pet.set_cursor_species(&self.entry.id);
        self.shown = shown;
        let info = self.info();
        {
            // 안 읽은 💌 답장은 그대로 둠
            let state = app.state::<AppState>();
            let mut infos = state.slots.lock().unwrap();
            let mail = infos.get(&self.label).and_then(|i| i.mail.clone());
            infos.insert(self.label.clone(), SlotInfo { mail, ..info.clone() });
        }
        let _ = app.emit_to(self.label.as_str(), "pet-stage", info);
    }

    /// 창 크기를 바꿈 (논리 px). 포커스를 뺏지 않음 — 슬라이더를 끄는 동안에도 불림
    fn resize_window(&self, w: f64, h: f64) {
        #[cfg(windows)]
        let hwnd = self.hwnd;
        #[cfg(not(windows))]
        let hwnd = None;
        crate::overlay::resize_quietly(&self.window, hwnd, (w * self.dpi).round() as u32, (h * self.dpi).round() as u32);
    }

    /// 크기 조절 창: 펫 크기 배율을 바꿈. 일시 확대는 그대로 두고 그 아래만 바꿈
    pub fn set_size(&mut self, app: &AppHandle, size: f64) {
        if size == self.size {
            return;
        }
        let mut plain = self.shown.clone();
        plain.scale = plain.scale / self.boost_applied / self.size * size;
        self.size = size;
        self.set_stage(app, plain);
    }

    /// 지금 몸 크기 (논리 px)
    pub fn size(&self) -> f64 {
        self.size
    }

    /// 지금 일시 확대 배율이 몸 크기에 곱해지는 값
    fn boost_factor(&self, shown: &StagePayload) -> f64 {
        visit::round_boost(shown.scale, shown.stage.scale(), self.boost)
    }

    /// 이 동물의 최대 일시 확대 배율
    pub fn boost_cap(&self) -> f64 {
        visit::boost_cap(self.entry.manifest.size_mult(self.shown.stage))
    }

    /// 일시 확대 배율을 바꿈: 창 크기·몸 크기·웹뷰 그림 (성장과 상관없음, 저장하지 않음)
    pub fn set_boost(&mut self, app: &AppHandle, boost: f64) {
        let mut plain = self.shown.clone();
        plain.scale /= self.boost_applied;
        self.boost = boost;
        self.set_stage(app, plain);
    }

    /// 지금 창 크기 (논리 px) — 편지를 펼친 동안은 위로 높고 넓음
    fn logical_size(&self, body_scale: f64) -> (f64, f64) {
        let (w, h) = if self.note_open { note_window_logical_size(body_scale) } else { window_logical_size(body_scale) };
        (w, h + self.glow_pad())
    }

    /// 눌렸음: 빛나는 동물이면 그동안 창 아래에 빛 번질 자리를 냄
    pub fn start_glow(&mut self) {
        if GLOW_PETS.contains(&self.entry.id.as_str()) {
            self.glow_until = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(GLOW_SECS));
        }
    }

    /// 아래 빛 자리 (논리 px)
    fn glow_pad(&self) -> f64 {
        if self.glow_open {
            GLOW_SPACE
        } else {
            0.0
        }
    }

    /// 들어 올린 채 빛나기 시작·끝나면 창을 아래로 늘리거나 줄이고, 웹뷰가 펫을 그만큼 올려 그림 (`pet-pad`)
    fn sync_glow(&mut self) {
        let on = self.pet.is_held() && self.glow_until.is_some_and(|t| std::time::Instant::now() < t);
        if on == self.glow_open {
            return;
        }
        self.glow_open = on;
        if !on {
            self.glow_until = None;
        }
        let (w, h) = self.logical_size(self.shown.scale);
        self.resize_window(w, h);
        self.last_pos = None;
        let _ = self.window.emit_to(self.label.as_str(), "pet-pad", self.glow_pad());
    }

    /// 커서로 뛰어오르고 매달리고 떨어지는 동안 창이 마우스를 통과시킴 — 몸이 커서 끝 바로 아래라 창이 커서를 덮음 (코어 hang_spot).
    /// place에서 창을 옮기기 전에 불러 첫 틱부터 클릭을 뺏지 않음. 바뀔 때만 OS에 알림
    fn sync_click_through(&mut self) {
        let through = self.pet.cursor_through();
        if through != self.click_through {
            self.click_through = through;
            crate::overlay::set_click_through(&self.window, through);
        }
    }

    /// 편지 말풍선이 펼쳐져 있음 (창이 넓어져 커서 장난은 쉼)
    pub fn note_is_open(&self) -> bool {
        self.note_open
    }

    /// 편지 말풍선을 띄움 / 접음: 창을 키우거나 줄이고, 몸이 그 자리에 있도록 바로 다시 놓음.
    /// 손님은 편지를 들고 왔을 때만, 내 동물은 답장을 들고 돌아왔을 때 (`pet-mail` — 언제 왔는지는 웹뷰가 앎)
    pub fn set_note_open(&mut self, open: bool) {
        let has_note = match &self.role {
            Role::Guest(g) => g.note.is_some(),
            Role::Own(_) => true,
        };
        if self.note_open == open || (open && !has_note) {
            return;
        }
        self.note_open = open;
        let (w, h) = self.logical_size(self.shown.scale);
        self.resize_window(w, h);
        let pos = self.window_target();
        self.move_window(pos.0, pos.1);
        self.last_pos = Some(pos);
    }

    /// 🔄 새로고침·💤 깨어남: 웹뷰를 다시 읽고 (그림을 잃은 WebView2를 되살림 — 새 페이지는 slot_info로 지금 모습을 받음),
    /// 창을 숨겼다가 다음 place에서 제자리에 다시 보임 (화면 위에 있어야 할 때만). 펼친 편지는 접음 (새 페이지엔 없음)
    pub fn reload_view(&mut self) {
        // 페이지가 듣기를 마칠 때까지 말풍선·답장 같은 한 번뿐인 알림을 모아 둠 (pages.rs)
        crate::pages::begin(&self.label);
        self.set_note_open(false);
        let _ = self.window.reload();
        self.forget_sent();
        crate::overlay::hide_quietly(&self.window);
        self.need_show = true;
    }

    /// 웹뷰에 보낸 자리·모습을 잊음: 다음 place·take_render가 지금 것을 다시 보냄 (다시 읽은 페이지가 놓쳤을 수 있음)
    pub fn forget_sent(&mut self) {
        self.last_pos = None;
        self.last_render = None;
    }

    /// 창 왼쪽 위 (물리 px): 몸이 창 아래 가운데에 오도록
    fn window_target(&self) -> (i32, i32) {
        let (_, sy) = self.pet.window_pos();
        let (w, h) = self.logical_size(self.shown.scale);
        // 몸 아래 빛 자리는 창이 바닥 쪽으로 늘어난 만큼 (몸은 그대로)
        let top = h - PET_LOGICAL_SIZE * self.shown.scale - self.glow_pad();
        ((self.pet.body.x - w * self.dpi / 2.0).round() as i32, (sy - top * self.dpi).round() as i32)
    }

    /// 손님 이름표의 주인 이름을 바꿈 (친구가 이름을 바꿨거나 내가 별명을 붙임). 달라졌을 때만 웹뷰에 알림
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
        // pet-stage는 이름표(applyRole)도 다시 그림
        let _ = app.emit_to(self.label.as_str(), "pet-stage", info);
    }

    /// 화면 배율과 세계를 다시 읽음 (WORLD_REFRESH마다)
    pub fn refresh(&mut self, windows: &[WindowInfo], work_areas: &[Rect]) {
        let dpi = self.window.scale_factor().unwrap_or(self.dpi);
        if dpi != self.dpi {
            self.dpi = dpi;
            self.pet.cfg = pet_config(dpi, &self.entry, self.shown.scale, self.shown.stage);
        }
        self.world = World::new(windows.to_vec(), work_areas.to_vec(), self.pet.cfg.size);
    }

    /// 커서가 스프라이트 위에 있음. 걷는 펫이 커서 밑에서 빠져나가지 않도록 사방 20% 여유를 둠
    pub fn cursor_inside(&self, cx: f64, cy: f64) -> bool {
        let (sx, sy) = self.pet.window_pos();
        let side = self.pet.cfg.size;
        let pad = side * 0.2;
        cx >= sx - pad && cx < sx + side + pad && cy >= sy - pad && cy < sy + side + pad
    }

    /// 화면 위에 있어야 하는 동물 (하우스 안이거나 친구 화면에 가 있으면 아님)
    pub fn on_screen(&self) -> bool {
        !self.pet.is_housed() && !self.pet.is_away()
    }

    /// 🛷 썰매에 탔음: 창을 화면 밖으로 치워 둠 (보이기·숨기기 없이 — 포커스를 뺏지 않음)
    pub fn park(&mut self) {
        /// 화면 밖 좌표
        const AWAY: i32 = -32000;
        self.move_window(AWAY, AWAY);
        self.last_pos = Some((AWAY, AWAY));
    }

    /// 썰매에서 내렸음: 다음 place가 몸 자리로 되돌림
    pub fn unpark(&mut self) {
        self.last_pos = None;
    }

    /// 창을 몸 위치로 옮기고, 처음(또는 다시 나타날 때) 보임. hidden = 사용자가 트레이로 모두 숨겨 둠 (그동안은 보이지 않음)
    pub fn place(&mut self, hidden: bool) {
        self.sync_glow();
        self.sync_click_through();
        let pos = self.window_target();
        if self.last_pos != Some(pos) {
            self.move_window(pos.0, pos.1);
            self.last_pos = Some(pos);
        }
        if self.need_show && self.on_screen() && !hidden {
            // 친구 펫이 들어와도 지금 쓰던 앱의 포커스를 뺏지 않음
            crate::overlay::show_quietly(&self.window);
            self.need_show = false;
        } else if !self.need_show && self.on_screen() && !hidden && self.check_shown_due() && !crate::overlay::is_shown(&self.window) {
            // 보여야 하는데 숨어 있음 (다른 무엇이 우리 몰래 숨겼음 — 예: tao가 창 플래그를 다시 적용하며 SW_HIDE): 조용히 다시 보임
            crate::overlay::show_quietly(&self.window);
        }
    }

    /// 창이 정말 보이는지 OS에 물을 때 (1초에 한 번쯤 — 매 틱 묻지 않음)
    fn check_shown_due(&mut self) -> bool {
        self.shown_checks = self.shown_checks.wrapping_add(1);
        self.shown_checks % SHOWN_CHECK_TICKS == 0
    }

    /// 지난번과 달라졌으면 지금 그릴 모습. 새로 뜬 웹뷰(`slot_info`)를 위해 AppState에도 적어 둠 — emit보다 먼저
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

// 어울림 코디네이터(social.rs)가 슬롯 목록을 그대로 받음
impl AsRef<Pet> for Slot {
    /// 동물 두뇌를 빌려줌
    fn as_ref(&self) -> &Pet {
        &self.pet
    }
}

impl AsMut<Pet> for Slot {
    /// 동물 두뇌를 빌려줌 (바꿀 수 있게)
    fn as_mut(&mut self) -> &mut Pet {
        &mut self.pet
    }
}

/// 손님 창: "pet" 창과 같은 설정 (투명·테두리 없음·항상 위·작업표시줄 숨김·포커스 안 받음·숨긴 채 시작)
fn build_guest_window(app: &AppHandle, label: &str, body_scale: f64) -> tauri::Result<WebviewWindow> {
    let (w, h) = window_logical_size(body_scale);
    crate::overlay::finish(
        WebviewWindowBuilder::new(app, label, WebviewUrl::App(format!("index.html?role=guest&slot={label}").into()))
            .title("ScreenPet Guest")
            .inner_size(w, h)
            .accept_first_mouse(true),
    )
}

/// 🐾🐾🐾 둘째·셋째 펫 창: "pet" 창과 같은 설정 (overlay::finish — 투명·항상 위·작업표시줄 숨김·포커스 안 받음·숨긴 채 시작)
fn build_own_window(app: &AppHandle, label: &str, body_scale: f64) -> tauri::Result<WebviewWindow> {
    let (w, h) = window_logical_size(body_scale);
    crate::overlay::finish(WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into())).title("ScreenPet").inner_size(w, h).accept_first_mouse(true))
}

/// 🐾🐾🐾 내 펫 하나를 새 창(label)으로 들임: 화면 위 가운데쯤 (x_frac = 화면 너비 중 자리)에서 떨어지며 나타남.
/// base = 지금 세계 (창·작업 영역만 씀). 게임 루프 스레드에서만 부름 (창 만들기가 메인 스레드를 기다림)
#[allow(clippy::too_many_arguments)]
pub fn spawn_own(app: &AppHandle, label: String, uid: String, entry: PetEntry, stage: Stage, size: f64, dpi_hint: f64, base: &World, x_frac: f64, tint: BodyTint) -> Result<Slot, String> {
    let role = Role::Own(uid);
    let shown = stage_payload(&entry, stage, size);
    let state = app.state::<AppState>();
    {
        let mut infos = state.slots.lock().unwrap();
        if infos.contains_key(&label) || app.get_webview_window(&label).is_some() {
            return Err(format!("{label} already exists"));
        }
        infos.insert(label.clone(), SlotInfo { tint, ..SlotInfo::new(&shown, &role, None) });
    }
    let window = match build_own_window(app, &label, shown.scale) {
        Ok(w) => w,
        Err(e) => {
            state.slots.lock().unwrap().remove(&label);
            return Err(e.to_string());
        }
    };
    let dpi = window.scale_factor().unwrap_or(dpi_hint);
    let cfg = pet_config(dpi, &entry, shown.scale, shown.stage);
    let world = World::new(base.windows.clone(), base.work_areas.clone(), cfg.size);
    let b = world.bounds;
    let pet = Pet::new(cfg, b.left + b.width() * x_frac.clamp(0.1, 0.9), b.top + cfg.size, fastrand::u64(..));
    let mut slot = Slot::new(label, role, entry, pet, window, world, dpi, shown.clone(), size);
    slot.tint = tint;
    slot.set_stage(app, shown);
    Ok(slot)
}

/// 친구 동물이 화면 끝(side)에서 걸어 들어옴. 집(주인 화면)은 들어온 쪽임.
/// scale = 주인 쪽에서 정한 몸 배율 (단계 × 동물별). 옛 버전 친구가 픽셀 스타일을 보내도 늘 동글로 그림
/// (dpi_hint = 창 배율을 못 읽을 때 쓰는 지금 화면 배율 — 창이 다른 모니터에 뜨면 다음 refresh가 고침). base = 지금 세계 (창·작업 영역만 씀). 게임 루프 스레드에서만 부름 (창 만들기가 메인 스레드를 기다림)
#[allow(clippy::too_many_arguments)]
pub fn spawn_guest(
    app: &AppHandle,
    guest: GuestInfo,
    entry: PetEntry,
    stage: Stage,
    scale: f64,
    dpi_hint: f64,
    side: Side,
    base: &World,
    seed: u64,
    size: f64,
) -> Result<Slot, String> {
    let label = guest_label(&guest.visit_id).ok_or_else(|| format!("invalid visit id {:?}", guest.visit_id))?;
    let tint = guest.tint;
    let role = Role::Guest(guest);
    let mut shown = stage_payload(&entry, stage, size);
    shown.scale = scale * size;
    // 웹뷰가 뜨자마자 slot_info를 부르므로 창보다 먼저 적어 둠. 같은 라벨이 이미 있으면 (같은 방문이 두 번) 거절
    let state = app.state::<AppState>();
    {
        let mut infos = state.slots.lock().unwrap();
        if infos.contains_key(&label) || app.get_webview_window(&label).is_some() {
            return Err(format!("{label} already exists"));
        }
        infos.insert(label.clone(), SlotInfo::new(&shown, &role, None));
    }
    let window = match build_guest_window(app, &label, shown.scale) {
        Ok(w) => w,
        Err(e) => {
            // 위에서 우리가 넣은 것만 지움 (같은 라벨은 없다고 확인했음)
            state.slots.lock().unwrap().remove(&label);
            return Err(e.to_string());
        }
    };
    let dpi = window.scale_factor().unwrap_or(dpi_hint);
    let cfg = pet_config(dpi, &entry, shown.scale, shown.stage);
    let world = World::new(base.windows.clone(), base.work_areas.clone(), cfg.size);
    let mut pet = Pet::new_entering(cfg, &world, side, seed);
    let mut rng = fastrand::Rng::with_seed(seed ^ 0x9e37_79b9_7f4a_7c15);
    pet.set_guest(side, visit::pick_homesick_after(&mut rng));
    let mut slot = Slot::new(label, role, entry, pet, window, world, dpi, shown, size);
    slot.tint = tint;
    Ok(slot)
}

/// 손님이 떠났음 (또는 🐾🐾🐾 내 펫을 보내 줌) — 창을 닫음
pub fn close(app: &AppHandle, slot: Slot) {
    app.state::<AppState>().slots.lock().unwrap().remove(&slot.label);
    let _ = slot.window.destroy();
}

/// 라벨로 슬롯 번호를 찾음
pub fn find(slots: &[Slot], label: &str) -> Option<usize> {
    slots.iter().position(|s| s.label == label)
}

/// 커서 아래 있는 맨 위 슬롯 — 나중에 만든 창이 위에 있으므로 뒤에서부터 찾음
pub fn hover_target(slots: &[Slot], cx: f64, cy: f64) -> Option<usize> {
    slots.iter().rposition(|s| s.on_screen() && s.cursor_inside(cx, cy))
}
