//! 물건 창 (`object-<n>`, object.html) — 물리와 규칙은 코어(screenpet_core::object), 여기는 창·메뉴·친구에게 던지기.
//! 물건은 대부분 잠들어 있음: 잠든 물건은 창도 웹뷰도 건드리지 않음 (틱마다 바람 시계만 줄어듦).
//! 창 목록이 그대로면 세계도 다시 만들지 않고, 창 크기·화면 배율도 깨어 있는 물건만 다시 읽음.
//! 받은 물건은 인증된 친구 링크에서 온 것만, 종류·움직임·글자를 코어가 다듬은 뒤에 씀 (글자는 웹뷰에서 textContent로만).
//! 💩 똥도 물건임 (끌기·던지기·치우기·친구에게 넘기기·집중 중 되돌리기가 그대로): 메뉴로 못 꺼내고, 수는 따로 10개까지, 30분이면 스르륵 사라지고, 빗자루에 쓸림.
//! 🪨💩 거대한 돌·똥은 메뉴로 꺼내는 쓰레기임 (내 물건 10개에 들어감): 아기 동물의 15배 — 놓이는 화면 높이의 80%까지 — 로 그리고, 바람에 안 구르고, 포스트잇이 없음.
//! 거대한 물건 창은 그림 밖(투명한 곳)의 클릭을 아래로 보냄 (커서를 틱마다 코어의 돌린 볼록 껍질에 대 봄).
//! 친구에게는 보통 돌·똥 이름에 look "giant"를 붙여 던짐 (옛 버전 친구는 보통 크기로 받음). 거대한 똥은 빗자루에 쓸리지만 저절로 사라지지 않음.
//! 🤹 흐물이가 주워 든 물건은 틱마다 그 손 자리로 옮기고 (코어가 정한 자리), 놓으면 코어가 준 속도로 던짐 — 그 뒤는 손으로 던진 것과 같음.
//! 아무도 들고 있지 않게 된 물건은 그 자리에서 떨어짐. 친구에게 들고 간 물건은 이 화면에서 치우고 Visit에 실림 (network.rs).
//! 📌 흐물이가 작업표시줄에서 끌어내는 앱 아이콘 장난감 (코어 taskbar.rs): Pull이면 열린 창 몇 개의 앱 아이콘을 다른 스레드에서 읽어 (platform app_icon)
//! 하나로 장난감 창을 만들어 흐물이 손에 쥐여 주고, Stow면 뿅 치움. 한 번에 하나, 몇 분 뒤 저절로 사라짐. 그림은 창의 초기화 스크립트로만 가고
//! (웹뷰가 canvas에 그림 — CSP 그대로) 어디에도 저장·기록하지 않음. 🔓 (v0.6.5, 사용자 결정) 친구에게 던지거나 들려 보낼 수 있음: 그때만 그림을
//! 48 px로 줄여 Toss·Carry의 icon에 실음 (코어 icon::to_wire). 받은 장난감은 코어가 그림을 엄격히 확인한 것만 (accept_toy_toss·accept_toy_carry) 만듦.
//! ⛄ 눈덩이 (v0.6.5): 쌓인 눈을 커서로 문지르거나 (game_loop) 동물이 발밑 눈으로 뭉쳐 만듦. 따로 5개까지, 몇 분 뒤 스르륵 녹음.
//! 던진 눈덩이는 동물·바닥·창에 닿으면 퍽 부서짐 — 뿅 치우고 그 자리를 take_bursts로 알림 (게임 루프가 눈가루를 날림).
//! 🌀 태풍 (v0.6.8): 돌풍에 바닥의 작은 것이 휙 날리고 (Object::blow — 친구 화면으로는 안 감), 회오리가 빨아들여 돌리다
//! 내던짐 (weather.rs가 코어 typhoon을 돌려 storm_apply로 시킴). 친구 쪽으로 내던진 것은 launch — 끝을 넘으면 평소처럼 toss_away (예산 그대로)
//! 🕸️ 거미집에 걸림 (spec 5부): 게임 루프가 틱마다 화면의 거미집 자리를 set_webs로 주고, tick이 물건마다 넘김 — 걸리기·매달리기·떨어지기는 코어 (Object::set_webs)
//! 📦 물건 창은 게임 루프가 만들지 않음: 창 만들기(build)는 메인 스레드가 웹뷰를 다 띄울 때까지 부른 스레드를 막아서 (Windows 0.1–0.3초)
//! "쓰레기 10개"가 오면 게임 루프가 열 번 멈췄음. 그래서 물건은 바로 들여 움직이고 (부딪히기·걸리기·치우기 모두), 창은 일꾼 스레드
//! (object-windows — start_window_worker)가 하나씩 만들어 돌려보내면 다음 틱에 붙임 (attach_built). 창이 오기 전엔 창 일만 건너뛰고,
//! 그 사이 치운 물건의 창은 오자마자 닫음. 받은 물건 줄은 일꾼이 한가할 때만 다음 것을 들임 (한 번에 하나)

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use screenpet_core::config::Config;
use screenpet_log::{log, short};
use screenpet_core::geom::{Rect, WindowInfo};
use screenpet_core::icon::{self, IconImage};
use screenpet_core::taskbar::{Toys, CAPTURE_BACKOFF_SECS, TOY_ICON_PX};
use screenpet_core::object::{accept_toss, accept_toy_toss, AcceptedToss, Exit, Object, ObjectKind, Origin, Postit, Release, Roster, TossBudget, TossInbox, MAX_OWN_OBJECTS, MAX_POOPS, OBJECT_LOGICAL_SIZE, Volley, volley_piece};
use screenpet_core::brain::Pet;
use screenpet_core::juggle::{JuggleEvent, ObjectSpot};
use screenpet_core::cursor_rally::CursorRallyEvent;
use screenpet_core::play::ObjectPlay;
use screenpet_core::broom::Broom;
use screenpet_core::social::Social;
use screenpet_core::poop::PoopSpot;
use screenpet_core::tumble::shape_of;
use screenpet_core::typhoon::{self, Gust, Loose, Thing, Whirl};
use screenpet_core::platform::WindowSource;
use screenpet_core::web::WebSpot;
use screenpet_core::visit::{self, Side, MAX_NOTE_CHARS};
use screenpet_core::world::World;
use screenpet_net::Toss;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

use crate::game_loop::Input;
use crate::network::{from_net_fling, from_net_side, to_net_fling, to_net_side, Network};
use crate::AppState;

/// 물건 창 라벨 앞부분
const PREFIX: &str = "object-";
/// 포스트잇 쓰기 창 라벨
pub const POSTIT_LABEL: &str = "postit";
/// 포스트잇 쓰기 창 모양
const POSTIT_FORM: crate::overlay::Form = crate::overlay::Form { label: POSTIT_LABEL, page: "postit.html", title: "ScreenPet 포스트잇", size: (300.0, 176.0) };
/// 포스트잇을 읽는 동안의 창: 너비, 위로 더하는 자리 (논리 px)
const NOTE_WIDTH: f64 = 232.0;
/// 포스트잇을 읽는 동안 위로 더하는 자리 (논리 px)
const NOTE_SPACE: f64 = 84.0;
/// 뿅 사라지는 동안 기다렸다 닫음 (object.html poof 0.3초)
const POOF: Duration = Duration::from_millis(400);
/// 똥이 스르륵 사라지는 동안 기다렸다 닫음 (object.html fade 1.5초)
const FADE: Duration = Duration::from_millis(1600);
/// 앉은 창이 움직인 뒤 이만큼은 매 틱 따라감 (사이엔 창 목록을 100 ms마다 읽을 때만 봄)
const RIDE_FOLLOW: Duration = Duration::from_secs(1);

/// 메뉴 id: 물건 꺼내기 (뒤에 종류)
const MENU_SPAWN: &str = "object:spawn:";
/// 메뉴 id: 모두 치우기
const MENU_CLEAR: &str = "object:clear";
/// 메뉴 id: 포스트잇 쓰기 (뒤에 창 라벨)
const MENU_NOTE: &str = "object:note:";
/// 메뉴 id: 포스트잇 떼기
const MENU_PEEL: &str = "object:peel:";
/// 메뉴 id: 물건 치우기
const MENU_REMOVE: &str = "object:remove:";

/// `물건 ▸` 하위 메뉴 (글자에 내 물건 수)
static MENU: OnceLock<Submenu<Wry>> = OnceLock::new();
/// 창 라벨 → 포스트잇 상태 (우클릭 메뉴는 게임 루프 밖에서 만듦). 글자는 없음
static POSTITS: Mutex<Option<HashMap<String, Postit>>> = Mutex::new(None);

/// 우클릭 메뉴가 읽을 포스트잇 상태를 적거나 지움
fn set_postit_state(label: &str, postit: Option<Postit>) {
    let mut map = POSTITS.lock().unwrap();
    let map = map.get_or_insert_with(HashMap::new);
    match postit {
        Some(p) => {
            map.insert(label.to_string(), p);
        }
        None => {
            map.remove(label);
        }
    }
}

/// `물건 ▸` 메뉴 글자 (내 물건 수)
fn menu_label(own: usize) -> String {
    format!("🗑️ 쓰레기 꺼내기 ({own}/{MAX_OWN_OBJECTS})")
}

/// 게임 루프로 들어오는 물건 입력
pub enum ObjectInput {
    Spawn(ObjectKind),
    ClearAll,
    /// 창의 물건을 눌렀음
    Down { label: String },
    Remove { label: String },
    /// 우클릭 "포스트잇 붙이기…"
    EditPostit { label: String },
    /// 우클릭 "포스트잇 떼기"
    PeelPostit { label: String },
    /// 포스트잇 창 "붙이기"
    SubmitPostit(String),
    /// 친구 메뉴 "물건 10개 던지기"
    ThrowVolley(String),
}

/// 크기 조절 창이 바꾼 그림 크기와 창 한 변 (`object-scale`)
#[derive(Debug, Clone, Copy, Serialize)]
struct ScalePayload {
    scale: f64,
    side: f64,
}

/// 웹뷰가 그릴 모습 (`object-pose`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
struct PosePayload {
    angle: f64,
    /// 포스트잇을 읽느라 넓어진 창이 화면 끝에서 밀렸을 때 그림을 옮기는 거리 (논리 px)
    dx: f64,
}

/// 화면에 있는 물건 창 하나임
struct Live {
    id: u64,
    label: String,
    object: Object,
    /// 물건 창 (일꾼 스레드가 만드는 동안엔 None — 그동안도 물건은 움직이고 부딪히고 치워질 수 있음, 창 일만 건너뜀)
    window: Option<WebviewWindow>,
    last_pos: Option<(i32, i32)>,
    last_pose: PosePayload,
    note_open: bool,
    /// Windows 창 핸들 (포커스를 뺏지 않고 옮기려고)
    hwnd: Option<isize>,
    /// 창이 마우스를 통과시키는 중 (거대한 물건의 투명한 곳 위에 커서가 있음)
    click_through: bool,
    /// 📌 장난감에 그린 앱 아이콘 (친구에게 던지거나 들고 갈 때 같이 보냄 — 메모리에만)
    icon: Option<IconImage>,
}

impl Live {
    /// 창은 도는 축을 가운데에 두는 정사각형 (어떤 각도로 돌아도 그림이 잘리지 않음, 종류마다 크기가 다름).
    /// 쪽지를 읽는 동안엔 넓어지고 위로 자리가 더 생김 — 축은 아래쪽 정사각형의 가운데 그대로
    fn logical_size(&self) -> (f64, f64) {
        let side = shape_of(self.object.kind).window_side_at(self.object.scale());
        if self.note_open {
            (NOTE_WIDTH, side + NOTE_SPACE)
        } else {
            (side, side)
        }
    }

    /// 창을 몸 자리로, 처음이면 보임. 달라졌을 때만 OS에 알림
    fn place(&mut self, world: &World) {
        // 창을 기다리는 중: 옮길 창이 없음 (last_pos를 비워 둬야 창이 오면 바로 제자리로 감)
        let Some(window) = &self.window else { return };
        let dpi = self.object.dpi();
        let (w, h) = self.logical_size();
        let (cx, cy) = self.object.pivot();
        let mut x = cx - w * dpi / 2.0;
        if self.note_open {
            // 읽는 동안: 쪽지가 화면 밖으로 잘리지 않게 창만 안쪽으로 밂
            let wa = world.work_area_at(cx, self.object.body.y);
            x = x.clamp(wa.left, (wa.right - w * dpi).max(wa.left));
        }
        let side = shape_of(self.object.kind).window_side_at(self.object.scale());
        let pos = (x.round() as i32, (cy + (side / 2.0 - h) * dpi).round() as i32);
        if self.last_pos != Some(pos) {
            crate::overlay::move_quietly(window, self.hwnd, pos.0, pos.1);
            self.last_pos = Some(pos);
        }
    }

    /// 웹뷰에 보낼 기울기와 그림 옮김
    fn pose(&self) -> PosePayload {
        let dpi = self.object.dpi();
        let (w, _) = self.logical_size();
        let dx = self.last_pos.map_or(0.0, |(x, _)| (self.object.pivot().0 - (x as f64 + w * dpi / 2.0)) / dpi);
        let dx = if self.note_open { dx } else { 0.0 };
        PosePayload { angle: self.object.angle(), dx }
    }

    /// 크기 배율을 바꾸고 창 크기를 맞춤
    fn set_scale(&mut self, app: &AppHandle, scale: f64) {
        self.object.set_scale(scale);
        // 창을 기다리는 중이면 크기는 창이 올 때 맞춤 (attach)
        let Some(window) = &self.window else { return };
        let (w, h) = self.logical_size();
        let dpi = self.object.dpi();
        crate::overlay::resize_quietly(window, self.hwnd, (w * dpi).round() as u32, (h * dpi).round() as u32);
        self.last_pos = None;
        let side = shape_of(self.object.kind).window_side_at(scale);
        let _ = app.emit_to(self.label.as_str(), "object-scale", ScalePayload { scale, side });
    }

    /// 거대한 물건·📌 장난감: 커서가 그림 밖(투명한 곳)이면 클릭이 아래로 지나가게 함 — 창이 화면을 크게 덮거나 작업표시줄 위에 걸치니까.
    /// 판정은 코어(돌린 그림의 볼록 껍질), 바뀔 때만 OS에 알림 (slots.rs sync_click_through처럼). 작은 물건은 그대로
    fn sync_click_through(&mut self, cursor: (f64, f64)) {
        if !self.object.kind.is_giant() && !self.object.kind.is_toy() {
            return;
        }
        // 창을 기다리는 중: click_through는 처음 값(통과) 그대로 두고 창이 오면 그대로 적용함
        let Some(window) = &self.window else { return };
        let through = self.object.passes_clicks_at(cursor.0, cursor.1);
        if through != self.click_through {
            self.click_through = through;
            crate::overlay::set_click_through(window, through);
        }
    }

    /// 포스트잇 읽기를 열거나 닫고 창 크기를 맞춤
    fn set_note_open(&mut self, open: bool) {
        if self.note_open == open {
            return;
        }
        self.note_open = open;
        let Some(window) = &self.window else { return };
        let (w, h) = self.logical_size();
        let dpi = self.object.dpi();
        crate::overlay::resize_quietly(window, self.hwnd, (w * dpi).round() as u32, (h * dpi).round() as u32);
        self.last_pos = None;
    }

    /// 📦 일꾼이 만든 창을 붙임 (아직 화면 밖 — 다음 place가 제자리로 옮김). 거대한 것·장난감의 클릭 통과는 화면 밖에 있을 때 켬 (깜빡임 없게).
    /// 기다리는 사이 크기·화면 배율이 바뀌었으면 창 크기를 다시 맞추고, 모습은 꼭 한 번 다시 보냄
    fn attach(&mut self, app: &AppHandle, window: WebviewWindow, built: &BuiltWindow) {
        if self.click_through {
            crate::overlay::set_click_through(&window, true);
        }
        // 일꾼이 숨긴 뒤 "모두 숨기기"가 눌렸을 수도 있음 (모두 숨긴 동안 온 물건은 "보이기"가 함께 보임)
        if crate::windows_hidden(app) {
            crate::overlay::hide_quietly(&window);
        }
        let scaled = built.scale != self.object.scale();
        if scaled || built.dpi != self.object.dpi() {
            let (w, h) = self.logical_size();
            let dpi = self.object.dpi();
            crate::overlay::resize_quietly(&window, built.hwnd, (w * dpi).round() as u32, (h * dpi).round() as u32);
        }
        if scaled {
            let side = shape_of(self.object.kind).window_side_at(self.object.scale());
            let _ = app.emit_to(self.label.as_str(), "object-scale", ScalePayload { scale: self.object.scale(), side });
        }
        self.hwnd = built.hwnd;
        self.window = Some(window);
        self.last_pos = None;
        // NaN은 어떤 모습과도 같지 않음: 다음 틱에 지금 모습을 꼭 보냄
        self.last_pose = PosePayload { angle: f64::NAN, dx: 0.0 };
    }
}

/// 📦 물건 창 일꾼에게 보내는 주문 (창 하나 — build_window의 재료)
struct WindowOrder {
    id: u64,
    kind: ObjectKind,
    /// 포스트잇이 붙은 채 옴
    sticker: bool,
    /// 주문할 때의 그림 크기 배율 (창이 오기 전에 바뀌면 attach가 다시 맞춤)
    scale: f64,
    /// 📌 장난감에 그릴 앱 아이콘
    icon: Option<IconImage>,
}

/// 📦 일꾼이 만든 물건 창 (못 만들었으면 window = None — 물건도 치움)
struct BuiltWindow {
    id: u64,
    window: Option<WebviewWindow>,
    /// 만들 때의 그림 크기 배율 (WindowOrder.scale)
    scale: f64,
    /// 창을 만들고 크기를 맞출 때 쓴 화면 배율
    dpi: f64,
    /// Windows 창 핸들 (hwnd()도 메인 스레드에 물어보므로 일꾼이 읽어 둠)
    hwnd: Option<isize>,
}

/// 📦 물건 창을 게임 루프 밖에서 만드는 일꾼 스레드와 잇는 줄. 창 만들기(build)는 메인 스레드가 웹뷰를 다 띄울 때까지
/// 부른 스레드를 막음 (Windows WebView2는 하나에 0.1–0.3초) — 게임 루프가 부르면 "쓰레기 10개"가 오는 동안 펫·물건이 뚝뚝 끊겼음
struct WindowWorker {
    orders: Sender<WindowOrder>,
    built: Receiver<BuiltWindow>,
    /// 보냈지만 아직 돌아오지 않은 주문 수 (받은 물건 줄은 0일 때만 다음 것을 들임 — 메인 스레드를 한꺼번에 몰아붙이지 않게)
    waiting: usize,
}

/// 창을 기다리는 똥 (일꾼이 한 번에 하나씩 만듦). 받은 쓰레기 줄과 따로 세어 서로 밀어내지 않음
enum WaitingPoop {
    /// 동물이 쌌음
    Dropped { kind: ObjectKind, x: f64, y: f64, scale: f64 },
    /// 친구가 던졌음
    Tossed(AcceptedToss, Side, f64),
}

/// 🗑️ 물건 상태임 (게임 루프가 가짐): 살아 있는 창, 받은 것, 똥 대기열, 던지기 예산
pub struct Objects {
    next: u64,
    live: Vec<Live>,
    roster: Roster,
    /// 받았지만 아직 들이지 않은 물건 (창 일꾼이 한가할 때 하나씩)
    inbox: TossInbox<(AcceptedToss, Side, u32, Option<IconImage>)>,
    poop_queue: VecDeque<WaitingPoop>,
    fading: crate::overlay::Fading,
    /// 물건이 있을 때만 맞춤 (물건 키에 맞춘 세계)
    world: Option<World>,
    /// 친구별 던지기 예산 (받는 쪽 net의 친구별 제한과 같음 — 넘치면 받는 쪽이 버리므로 미리 튕김)
    budgets: HashMap<String, TossBudget>,
    /// "물건 10개 던지기"에서 아직 못 던진 것 (친구별)
    volleys: HashMap<String, Volley>,
    /// 크기 조절 창의 물건 크기 배율
    size: f64,
    /// 친구별 마지막으로 던진 때 (되돌아온 물건이 정말 내 것인지 봄)
    threw_at: HashMap<String, Instant>,
    /// 📌 친구별 마지막으로 보낸 아이콘 그림 — 옛 버전 친구는 그림을 모른 채 되돌려 보내므로, 되돌아오면 이걸로 다시 붙임
    sent_icons: HashMap<String, (Instant, screenpet_net::WireIcon)>,
    started: Instant,
    ride_until: Instant,
    frame: u64,
    /// 포스트잇 창이 다루는 물건
    posting: Option<String>,
    shown_own: usize,
    /// 📌 창 목록의 창 id (장난감 아이콘을 고를 후보 — refresh가 채움)
    window_ids: Vec<u64>,
    /// 📌 다른 스레드에서 읽는 중인 장난감 아이콘
    capture: Option<Receiver<Option<IconImage>>>,
    /// 📌 아이콘을 못 읽었으면 이때까지 다시 묻지 않음
    capture_backoff: Instant,
    /// ⛄ 방금 퍽 부서진 눈덩이 자리 (물리 px — 게임 루프가 눈가루를 날림)
    bursts: Vec<(f64, f64)>,
    /// 🧔 🪨 해그리드가 내려놓은 락 케이크 (물건 종류, 자리 — 물리 px): 게임 루프가 take_spawns로 꺼내 만듦
    spawns: Vec<(ObjectKind, f64, f64)>,
    /// 🕸️ 화면의 거미집 자리 (게임 루프가 set_webs로 — 물건이 걸림)
    webs: Vec<WebSpot>,
    /// 📦 물건 창을 만드는 일꾼 (처음 물건이 생길 때 띄움, 죽었으면 다음 주문 때 다시)
    windows: Option<WindowWorker>,
    /// 📦 창이 오기 전 물건에 쓰는 화면 배율 (내 동물의 배율 — 게임 루프가 set_dpi_hint로, 창이 오면 refresh가 고침)
    dpi_hint: f64,
}

impl Objects {
    /// 빈 상태로 만듦
    pub fn new() -> Objects {
        let now = Instant::now();
        Objects {
            next: 0,
            live: Vec::new(),
            roster: Roster::default(),
            inbox: TossInbox::default(),
            poop_queue: VecDeque::new(),
            fading: Default::default(),
            world: None,
            budgets: HashMap::new(),
            volleys: HashMap::new(),
            size: 1.0,
            threw_at: HashMap::new(),
            sent_icons: HashMap::new(),
            started: now,
            ride_until: now,
            frame: 0,
            posting: None,
            shown_own: 0,
            window_ids: Vec::new(),
            capture: None,
            capture_backoff: now,
            bursts: Vec::new(),
            spawns: Vec::new(),
            webs: Vec::new(),
            windows: None,
            dpi_hint: 1.0,
        }
    }

    /// 📦 창이 오기 전 물건에 쓸 화면 배율 (내 동물 창의 배율)
    pub fn set_dpi_hint(&mut self, dpi: f64) {
        if dpi.is_finite() && dpi > 0.0 {
            self.dpi_hint = dpi;
        }
    }

    /// 🕸️ 이번 틱의 거미집 자리 (trail.web_spots — 펫에게 준 것과 같음). 다음 tick이 물건마다 넘김
    pub fn set_webs(&mut self, webs: &[WebSpot]) {
        self.webs.clear();
        self.webs.extend_from_slice(webs);
    }

    /// 끌고 있는 물건이 있음 (동물을 동시에 잡거나 간질이지 않음)
    pub fn is_held(&self) -> bool {
        self.live.iter().any(|l| l.object.is_held())
    }

    /// 라벨로 물건을 찾음
    fn find(&mut self, label: &str) -> Option<&mut Live> {
        self.live.iter_mut().find(|l| l.label == label)
    }

    /// busy = 동물을 잡고 있거나 음식을 들고 있음 (그동안의 누름은 무시). base = 지금 세계, cursor = 커서
    pub fn handle(&mut self, app: &AppHandle, input: ObjectInput, base: &World, cursor: (f64, f64), busy: bool) {
        match input {
            ObjectInput::Spawn(kind) => self.spawn_own(app, kind, base, cursor),
            ObjectInput::ClearAll => {
                self.roster.clear();
                self.inbox.clear();
                self.poop_queue.clear();
                for l in std::mem::take(&mut self.live) {
                    self.poof(l);
                }
            }
            ObjectInput::ThrowVolley(friend) => self.volleys.entry(friend).or_default().request(),
            ObjectInput::Remove { label } => self.remove(&label),
            ObjectInput::Down { label } => {
                if !busy && !self.is_held() {
                    if let Some(l) = self.find(&label) {
                        l.object.pointer_down(cursor.0, cursor.1);
                    }
                }
            }
            ObjectInput::EditPostit { label } => {
                if self.find(&label).is_some_and(|l| l.object.postit() == Postit::None) {
                    self.posting = Some(label);
                    show_postit_window(app);
                }
            }
            ObjectInput::PeelPostit { label } => {
                if let Some(l) = self.find(&label) {
                    if l.object.peel_note() {
                        set_postit_state(&l.label, Some(l.object.postit()));
                        let _ = app.emit_to(l.label.as_str(), "object-sticker", false);
                    }
                }
            }
            ObjectInput::SubmitPostit(text) => {
                let Some(label) = self.posting.take() else { return };
                if let Some(l) = self.find(&label) {
                    if l.object.attach_note(&text) {
                        set_postit_state(&l.label, Some(l.object.postit()));
                        let _ = app.emit_to(l.label.as_str(), "object-sticker", true);
                    }
                }
            }
        }
        self.sync_menu();
    }

    /// (x, y)에 놓을 그림 크기 배율: 쓰레기는 크기 조절 창을 따르고 (똥은 싼 동물을 따라 따로 정함),
    /// 거대한 것은 그 자리 작업 영역 높이에 맞춤 — 창을 만들기 전이라 화면 배율은 그 자리 모니터에서 읽음
    fn scale_at(&self, app: &AppHandle, kind: ObjectKind, base: &World, x: f64, y: f64) -> f64 {
        let height = if kind.is_giant() {
            let dpi = app.monitor_from_point(x, y).ok().flatten().map_or(1.0, |m| m.scale_factor());
            base.work_area_at(x, y).height() / dpi
        } else {
            f64::NAN
        };
        kind.scale_on(self.size, height)
    }

    /// 크기 조절 창: 화면 위 쓰레기까지 바로 바꿈
    pub fn set_size(&mut self, app: &AppHandle, size: f64) {
        if size == self.size {
            return;
        }
        self.size = size;
        for l in self.live.iter_mut().filter(|l| !l.object.kind.is_poop() && !l.object.kind.is_giant() && !l.object.kind.is_toy()) {
            l.set_scale(app, l.object.kind.base_scale() * size);
        }
    }

    /// 메뉴에서 만들기: 커서 근처(트레이에서 눌렀으면 그 위)에서 떨어짐. 넘치면 가장 오래된 내 물건이 뿅
    fn spawn_own(&mut self, app: &AppHandle, kind: ObjectKind, base: &World, cursor: (f64, f64)) {
        // 💩 "거대한 똥"은 갈색·핑크·무지개 중 하나로 나옴
        let kind = kind.menu_pick(fastrand::f64());
        let id = self.next_id();
        let scale = self.scale_at(app, kind, base, cursor.0, cursor.1);
        let dpi = self.dpi_hint;
        let world = self.world_for(base, dpi);
        let b = world.bounds;
        // 거대한 것은 몸 전체가 화면 안에서 떨어지기 시작함
        let size = OBJECT_LOGICAL_SIZE * dpi * scale.max(1.0);
        let x = cursor.0.clamp(b.left + size / 2.0, (b.right - size / 2.0).max(b.left + size / 2.0));
        let y = (cursor.1 - size / 2.0).clamp(b.top + size, b.bottom.max(b.top + size));
        let mut object = Object::new(kind, x, y, dpi, fastrand::u64(..));
        object.set_scale(scale);
        self.admit(app, id, object, Origin::Own, false, None);
    }

    /// ⛄ 눈덩이 창을 at(커서·동물 손 자리, 물리 px)에 만듦: 거기서 떨어지기 시작함 (동물이 던지면 그 뒤 launch).
    /// 눈덩이는 따로 MAX_SNOWBALLS개까지 — 넘치면 가장 오래된 것이 뿅. 만들었으면 그 id
    pub fn spawn_snowball(&mut self, app: &AppHandle, base: &World, at: (f64, f64)) -> Option<u64> {
        let kind = ObjectKind::Snowball;
        let id = self.next_id();
        let scale = self.scale_at(app, kind, base, at.0, at.1);
        let dpi = self.dpi_hint;
        let world = self.world_for(base, dpi);
        let b = world.bounds;
        let margin = OBJECT_LOGICAL_SIZE * dpi * scale / 2.0;
        let x = at.0.clamp(b.left + margin, (b.right - margin).max(b.left + margin));
        let mut object = Object::new(kind, x, at.1, dpi, fastrand::u64(..));
        object.set_scale(scale);
        self.admit(app, id, object, Origin::Snow, false, None).then_some(id)
    }

    /// 🧔 🪨 동물이 내려놓은 물건들 (play_with가 모음 — 한 번 꺼냄)
    pub fn take_spawns(&mut self) -> Vec<(ObjectKind, f64, f64)> {
        std::mem::take(&mut self.spawns)
    }

    /// 🧔 🪨 해그리드의 락 케이크 같은 동물이 내려놓은 물건을 at (발 앞, 물리 px)에 만듦: 거기서 살짝 떨어져 바닥에 놓임.
    /// 내 물건 수에 듦 (넘치면 가장 오래된 내 물건이 뿅 — admit). 만들었으면 그 id
    pub fn spawn_gift(&mut self, app: &AppHandle, base: &World, kind: ObjectKind, at: (f64, f64)) -> Option<u64> {
        let id = self.next_id();
        let scale = self.scale_at(app, kind, base, at.0, at.1);
        let dpi = self.dpi_hint;
        let world = self.world_for(base, dpi);
        let b = world.bounds;
        let margin = OBJECT_LOGICAL_SIZE * dpi * scale / 2.0;
        let x = at.0.clamp(b.left + margin, (b.right - margin).max(b.left + margin));
        let mut object = Object::new(kind, x, at.1, dpi, fastrand::u64(..));
        object.set_scale(scale);
        self.admit(app, id, object, Origin::Own, false, None).then_some(id)
    }

    /// 🤹⛄ 동물이 눈덩이를 (vx, vy)로 던졌음 (물리 px/s): 던진 동물(social 키)은 잠깐 맞히지 않음
    pub fn launch_snowball(&mut self, social: &mut Social, id: u64, thrower: u64, vx: f64, vy: f64) {
        if let Some(l) = self.live.iter_mut().find(|l| l.id == id) {
            l.object.launch(vx, vy);
            social.spare(id, thrower, THROWER_SPARED_SECS);
        }
    }

    /// ⛄ 방금 퍽 부서진 눈덩이 자리들 (게임 루프가 눈가루를 날림)
    pub fn take_bursts(&mut self) -> Vec<(f64, f64)> {
        std::mem::take(&mut self.bursts)
    }

    /// 동물이 쌌음: (x, y)에서 떨어지기 시작함. tick이 들이고 창은 일꾼 스레드가 만듦
    pub fn drop_poop(&mut self, kind: ObjectKind, x: f64, y: f64, scale: f64) {
        self.queue_poop(WaitingPoop::Dropped { kind, x, y, scale });
    }

    /// 똥을 대기열에 넣음 (넘치면 가장 오래된 것을 버림)
    fn queue_poop(&mut self, poop: WaitingPoop) {
        if self.poop_queue.len() >= MAX_POOPS {
            self.poop_queue.pop_front();
        }
        self.poop_queue.push_back(poop);
    }

    /// 바닥에 놓인 똥 자리를 out에 다시 채움 (동물이 알아챔, 틱마다 할당하지 않음)
    pub fn poop_spots(&self, out: &mut Vec<PoopSpot>) {
        out.clear();
        out.extend(self.live.iter().filter_map(|l| l.object.poop_spot(l.id)));
    }

    /// 🤹 바닥에 놓인 물건 자리를 out에 다시 채움 (흐물이가 주우러 감)
    pub fn object_spots(&self, out: &mut Vec<ObjectSpot>) {
        out.clear();
        out.extend(self.live.iter().filter_map(|l| l.object.spot(l.id)));
    }

    /// ⚾ 친구 화면 끝에서 던져져 날아 들어오는 물건들 (흐물이가 받으러 감 — notice_incoming)
    pub fn incoming(&self, out: &mut Vec<screenpet_core::juggle::Incoming>) {
        out.clear();
        out.extend(self.live.iter().filter_map(|l| l.object.incoming(l.id)));
    }

    /// 🤹 동물이 든 물건: 놓았으면 그 속도로 던지고 (던진 동물은 잠깐 맞히지 않음), 들고 있으면 손 자리로 옮기고,
    /// 사라졌거나 사용자가 잡고 있으면 동물에게 알림. 아무도 들고 있지 않은 물건은 떨어짐. 동물 update 뒤, tick 전에
    pub fn sync_carries<T: AsRef<Pet> + AsMut<Pet>>(&mut self, social: &mut Social, pets: &mut [T]) {
        for p in pets.iter_mut() {
            let pet = p.as_mut();
            match pet.take_juggle() {
                Some(JuggleEvent::Release { id, vx, vy }) => {
                    if let Some(l) = self.live.iter_mut().find(|l| l.id == id) {
                        l.object.launch(vx, vy);
                        social.spare(id, pet.social_key(), THROWER_SPARED_SECS);
                        // ⚾ 캐치볼로 되던졌음: 주고받은 횟수를 적음 (친구에게 보내는 Toss.rally — toss_away)
                        if let Some((_, n)) = pet.take_rally().filter(|(got, _)| *got == id) {
                            l.object.set_rally(n);
                        }
                    }
                }
                // ⚾ 날아온 것을 받았음 — 아래에서 손 자리로 옮김 (carry_at이 날던 물건도 손에 쥐여 줌)
                Some(JuggleEvent::Catch { .. }) => {}
                // 📌 장난감 아이콘을 읽기 시작함 (내 동물만 — 손님은 하지 않음)
                Some(JuggleEvent::Pull) => self.request_toy(),
                Some(JuggleEvent::Stow { id }) => self.stow(id),
                None => {}
            }
            let Some(id) = pet.carried_object() else { continue };
            match self.live.iter_mut().find(|l| l.id == id) {
                Some(l) if !l.object.is_held() => {
                    if let Some((x, y)) = pet.carry_point() {
                        l.object.carry_at(x, y);
                    }
                }
                // 사용자가 낚아챘거나 치웠음
                _ => pet.lose_carry(),
            }
        }
        for l in self.live.iter_mut().filter(|l| l.object.is_carried()) {
            if !pets.iter().any(|p| p.as_ref().carried_object() == Some(l.id)) {
                l.object.let_fall();
            }
        }
    }

    /// 🐱🐿️🦖 다른 동물들의 물건 놀이 (v0.6.3 — 코어 play.rs): 고양이가 툭 밀고, 다람쥐가 볼에 넣어 뿅 치우고, 공룡의 큰 쿵에
    /// 같은 바닥의 물건이 톡 튐. 🕷️ 거미인간이 줄로 끌어온 것·✨ Expelliarmus에 맞은 것은 날아가고, ✨ Leviosa에 맞은 것은 둥둥 뜨고,
    /// ✨ Reparo·👜 헤르미온느 가방에 들어간 것은 뿅 치움. 동물 update 뒤, tick 전에
    pub fn play_with<T: AsMut<Pet>>(&mut self, pets: &mut [T]) {
        for p in pets.iter_mut() {
            match p.as_mut().take_object_play() {
                Some(ObjectPlay::Nudge { id, dir, reach }) => {
                    if let Some(l) = self.live.iter_mut().find(|l| l.id == id) {
                        l.object.nudge(dir, reach);
                    }
                }
                Some(ObjectPlay::Gulp { id }) => {
                    // 📌 장난감·거대한 것은 코어가 고르지 않음 — 혹시 와도 치우지 않음
                    if let Some(i) = self.live.iter().position(|l| l.id == id && !l.object.kind.is_toy() && !l.object.kind.is_giant()) {
                        let l = self.live.remove(i);
                        self.roster.remove(l.id);
                        self.poof(l);
                        self.sync_menu();
                    }
                }
                // 🕷️ 거미인간이 줄로 끌어옴: 바닥에 놓인 것만 (잡혀 있거나 누가 들고 있거나 날고 있으면 그대로) — 그 뒤는 던진 것과 같음
                Some(ObjectPlay::Yank { id, vx, vy }) => {
                    if let Some(l) = self.live.iter_mut().find(|l| l.id == id && l.object.spot(l.id).is_some()) {
                        l.object.launch(vx, vy);
                    }
                }
                // ✨ Leviosa: 바닥에 놓인 것만 둥둥 (잡혀 있거나 누가 들고 있거나 날고 있으면 그대로 — 코어 brain_spell.rs)
                Some(ObjectPlay::Levitate { id, rise, secs }) => {
                    if let Some(l) = self.live.iter_mut().find(|l| l.id == id) {
                        l.object.levitate(rise, secs);
                    }
                }
                // ✨ Reparo·👜 구슬 가방: 바닥에 놓인 것을 뿅 치움 — 거대한 것도, 📌 장난감은 안 치움 (코어 brain_spell.rs·brain_study.rs)
                Some(ObjectPlay::Vanish { id }) => {
                    if let Some(i) = self.live.iter().position(|l| l.id == id && !l.object.kind.is_toy() && l.object.spot(l.id).is_some()) {
                        let l = self.live.remove(i);
                        self.roster.remove(l.id);
                        self.poof(l);
                        self.sync_menu();
                    }
                }
                // 🧔 🪨 락 케이크: 창을 만들려면 앱 핸들이 있어야 해서 게임 루프가 take_spawns로 꺼내 spawn_gift로 만듦
                Some(ObjectPlay::Spawn { kind, x, y }) => self.spawns.push((kind, x, y)),
                Some(ObjectPlay::Quake { x, y, owner, reach }) => {
                    for l in self.live.iter_mut() {
                        let near = l.object.spot(l.id).is_some_and(|s| s.owner == owner && (s.y - y).abs() <= screenpet_core::juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach);
                        if near {
                            l.object.pop(screenpet_core::play::QUAKE_POP_VY);
                        }
                    }
                }
                None => {}
            }
        }
    }

    /// ⚾🖱️ 커서와 캐치볼 (코어 cursor_rally.rs): 흐물이가 커서 쪽으로 던진 것은 횟수를 적고 커서 랠리로 표시하고,
    /// 다른 동물이 놀자고 민 것은 바닥에 있을 때만 그 속도로 날림 (잡혀 있거나 누가 들고 있으면 그대로)
    pub fn cursor_rally(&mut self, ev: CursorRallyEvent) {
        match ev {
            CursorRallyEvent::Back { id, rally } => {
                if let Some(l) = self.live.iter_mut().find(|l| l.id == id && l.object.in_flight()) {
                    l.object.set_rally(rally);
                    l.object.mark_cursor_rally();
                }
            }
            CursorRallyEvent::Boop { id, vx, vy, rally } => {
                if let Some(l) = self.live.iter_mut().find(|l| l.id == id && l.object.spot(l.id).is_some()) {
                    l.object.boop_back(vx, vy, rally);
                }
            }
        }
    }

    /// 🤹 친구에게 들고 갈 물건의 이름 (Toss와 같은 이름·색·크기). 거대한 것은 giants (💪 든 동물이 영웅 — Pet::carries_giants)일 때만:
    /// 거대한 것을 던질 때처럼 look "giant"로 실음 (크기는 받는 앱이 제 화면 높이로 정함)
    pub fn carry_wire(&self, id: u64, giants: bool) -> Option<screenpet_net::Carry> {
        let l = self.live.iter().find(|l| l.id == id)?;
        let kind = l.object.kind;
        // 📌 장난감은 그림이 있을 때만 그림과 함께 (🔓 v0.6.5)
        let icon = l.icon.as_ref().map(wire_icon);
        if (kind.is_giant() && !giants) || (kind.is_toy() && icon.is_none()) {
            return None;
        }
        Some(screenpet_net::Carry { kind: kind.wire_id().to_string(), look: kind.look().map(str::to_string), scale: kind.is_poop().then(|| l.object.scale()), icon: icon.filter(|_| kind.is_toy()) })
    }

    /// 💪 물건의 반 폭 (물리 px — 들고 온 거대한 것을 동물 손에 쥐여 줄 때 hold_giant에 줌)
    pub fn half_w_of(&self, id: u64) -> Option<f64> {
        self.live.iter().find(|l| l.id == id).map(|l| l.object.half_w())
    }

    /// 🤹 친구에게 들고 갔음: 이 화면에서는 치움 (뿅 없이 — 동물 손에 들려 화면 밖으로 나갔음)
    pub fn hand_over(&mut self, id: u64) {
        if let Some(i) = self.live.iter().position(|l| l.id == id) {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            set_postit_state(&l.label, None);
            if let Some(w) = &l.window {
                let _ = w.destroy();
            }
        }
        self.sync_menu();
    }

    /// 🤹 동물 손에 쥐여 줄 물건 창을 at(머리 위 언저리)에 만듦: 친구의 흐물이가 들고 왔으면 받은 물건, 못 들어가 들고 돌아왔으면 내 물건.
    /// 똥은 똥 수에 들어감 (scale = 다듬은 크기). 만들었으면 그 id — 동물의 hold_object에 줌
    /// icon = 📌 들고 온 앱 아이콘 장난감의 그림 (확인된 것 — 장난감일 때만)
    pub fn spawn_carried(&mut self, app: &AppHandle, base: &World, kind: ObjectKind, scale: Option<f64>, own: bool, at: (f64, f64), icon: Option<IconImage>) -> Option<u64> {
        if kind.is_toy() && icon.is_none() {
            return None;
        }
        let id = self.next_id();
        let scale = match scale.filter(|_| kind.is_poop()) {
            Some(s) => s,
            None => self.scale_at(app, kind, base, at.0, at.1),
        };
        let dpi = self.dpi_hint;
        self.world_for(base, dpi);
        let mut object = Object::new(kind, at.0, at.1, dpi, fastrand::u64(..));
        object.set_scale(scale);
        object.carry_at(at.0, at.1);
        let origin = if kind.is_poop() {
            Origin::Poop
        } else if kind.is_snowball() {
            Origin::Snow
        } else if own {
            Origin::Own
        } else {
            Origin::Foreign
        };
        if !self.admit(app, id, object, origin, false, icon.clone()) {
            return None;
        }
        self.set_icon(id, icon);
        self.sync_menu();
        Some(id)
    }

    /// 📌 장난감 창에 그린 아이콘을 기억해 둠 (친구에게 보낼 때 씀)
    fn set_icon(&mut self, id: u64, icon: Option<IconImage>) {
        if let Some(l) = self.live.iter_mut().find(|l| l.id == id && l.object.kind.is_toy()) {
            l.icon = icon;
        }
    }

    /// 📌 지금 장난감 사정 (내 흐물이가 매 틱 봄): 아이콘을 읽는 OS이고, 화면에 장난감이 없고, 읽는 중이 아니고, 모두 숨기지 않았을 때만 만들 수 있음
    pub fn toys(&self, app: &AppHandle) -> Toys {
        let live = self.live.iter().find(|l| l.object.kind.is_toy()).map(|l| l.id);
        let ready = screenpet_core::platform::APP_ICONS && live.is_none() && self.capture.is_none() && Instant::now() >= self.capture_backoff && !self.window_ids.is_empty() && !crate::windows_hidden(app);
        Toys { ready, live }
    }

    /// 📌 열린 창 몇 개를 골라 다른 스레드에서 앱 아이콘을 읽음 (게임 루프를 막지 않음 — Windows는 창에 묻는 데 100 ms까지 걸릴 수 있음)
    fn request_toy(&mut self) {
        if self.capture.is_some() || self.live.iter().any(|l| l.object.kind.is_toy()) {
            return;
        }
        let mut ids = self.window_ids.clone();
        fastrand::shuffle(&mut ids);
        ids.truncate(TOY_CANDIDATES);
        let (tx, rx) = mpsc::channel();
        let started = std::thread::Builder::new().name("toy-icon".into()).spawn(move || {
            let mut source = screenpet_core::platform::default_source();
            let _ = tx.send(ids.into_iter().find_map(|id| source.app_icon(id, TOY_ICON_PX)));
        });
        if started.is_ok() {
            self.capture = Some(rx);
        }
    }

    /// 📌 읽은 아이콘이 왔으면: 흐물이가 아직 끙차 당기고 있으면 그 손에 장난감 창을 만들어 쥐여 줌 · 🪶 마법사가 Leviosa로 기다리면 발 앞에 만들어 띄움.
    /// 못 읽었으면 한동안 묻지 않음 (조용히 — 마법사는 보통 물건을 띄움)
    pub fn deliver_toy(&mut self, app: &AppHandle, base: &World, pet: &mut Pet) {
        let Some(rx) = &self.capture else { return };
        let icon = match rx.try_recv() {
            Ok(icon) => icon,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => None,
        };
        self.capture = None;
        let Some(icon) = icon else {
            self.capture_backoff = Instant::now() + Duration::from_secs_f64(CAPTURE_BACKOFF_SECS);
            // 🪶 Leviosa로 아이콘을 기다리던 마법사는 곁의 보통 물건을 띄움
            pet.icon_lift_failed();
            return;
        };
        // 🪶 해리·헤르미온느가 Leviosa로 뽑은 아이콘: 발 앞 바닥에 장난감 아이콘을 만들어 바로 둥둥 띄움 (내려앉으면 보통 장난감)
        if let Some(lift) = pet.icon_lift_wanted() {
            let kind = ObjectKind::AppIcon;
            let id = self.next_id();
            let dpi = self.dpi_hint;
            self.world_for(base, dpi);
            let margin = 40.0 * dpi;
            let x = lift.x.clamp(base.bounds.left + margin, (base.bounds.right - margin).max(base.bounds.left + margin));
            let mut object = Object::new(kind, x, lift.y, dpi, fastrand::u64(..));
            object.set_scale(kind.base_scale());
            object.float_up(lift.rise, lift.secs);
            if self.admit(app, id, object, Origin::Toy, false, Some(icon.clone())) {
                self.set_icon(id, Some(icon));
                pet.icon_lifted(id);
            } else {
                pet.icon_lift_failed();
            }
            return;
        }
        if !pet.wants_toy() {
            return;
        }
        let at = pet.carry_point().unwrap_or((pet.body.x, pet.body.y - pet.cfg.size));
        let kind = ObjectKind::AppIcon;
        let id = self.next_id();
        let scale = kind.base_scale();
        let dpi = self.dpi_hint;
        self.world_for(base, dpi);
        let mut object = Object::new(kind, at.0, at.1, dpi, fastrand::u64(..));
        object.set_scale(scale);
        object.carry_at(at.0, at.1);
        if !self.admit(app, id, object, Origin::Toy, false, Some(icon.clone())) {
            return;
        }
        self.set_icon(id, Some(icon));
        if !pet.hold_toy(id) {
            self.stow(id);
        }
    }

    /// 📌 장난감을 작업표시줄에 도로 넣었음: 뿅
    fn stow(&mut self, id: u64) {
        if let Some(i) = self.live.iter().position(|l| l.id == id && l.object.kind.is_toy()) {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            self.poof(l);
        }
    }

    /// 🧹 빗자루가 지나간 물건(쓰레기·똥·장난감·눈덩이·거대한 것)은 빗자루 쪽으로 밀려 날아감 (평소의 던지기 — 친구가 연결된 끝을 넘으면
    /// tick이 친구에게 보냄, 예산도 같음). 들렸거나 동물이 든 것·날아가는 것·포스트잇을 읽는 중인 것은 코어 sweep이 그대로 둠
    pub fn sweep_objects(&mut self, broom: &Broom) {
        let (vx, vy) = broom.velocity();
        for l in &mut self.live {
            if broom.passes_over(l.object.body.x, l.object.body.y, l.object.dpi()) {
                l.object.sweep(vx, vy);
            }
        }
    }

    /// 던져서 날아가는 물건이 동물에 부딪힘 (같은 화면만 — 판정과 쿨다운은 코어 social.rs, 통신할 것이 없음)
    pub fn hit_pets<T: AsRef<Pet> + AsMut<Pet>>(&mut self, social: &mut Social, dt: f64, pets: &mut [T]) {
        for l in &mut self.live {
            social.object_hits(l.id, &mut l.object, dt, pets);
        }
    }

    /// 친구가 던진 물건 (인증된 친구 링크 — net이 확인 — 에서 온 내 친구의 것만). 받은 물건은 기다리는 것까지 20개, 넘치면 가장 오래된 것부터 치움.
    /// 여기서는 줄만 세움 — tick이 하나씩 들이고 창은 일꾼 스레드가 만듦 (한꺼번에 와도 게임 루프가 멈추지 않게)
    pub fn receive(&mut self, config: &Config, from: &str, toss: Toss) {
        if config.friend(from).is_none() {
            return;
        }
        let fling = toss.fling.map(from_net_fling);
        // 📌 친구가 던진 앱 아이콘 장난감: 그림이 엄격한 확인을 지나야만 (코어 accept_toy_toss — 이상하면 버림)
        if toss.kind == screenpet_core::object::TOY_ID {
            // 바쁜 옛 버전 친구가 되돌려 보낸 장난감은 그림이 빠져 옴: 방금 그 친구에게 보낸 그림을 다시 붙임 (사라지지 않게)
            let remembered = toss.bounced.then(|| self.sent_icons.get(from).filter(|(t, _)| t.elapsed() < BOUNCE_WINDOW).map(|(_, i)| i.clone())).flatten();
            let wire = toss.icon.clone().or(remembered);
            let Some((mut accepted, icon)) = accept_toy_toss(fling, wire.as_ref().map(|i| (i.size, i.rgba.as_str()))) else {
                log!(Warn, "toss", "toss_bad_icon", "friend" => short(from));
                return;
            };
            log!(Info, "toss", "toss_in", "friend" => short(from), "kind" => screenpet_core::object::TOY_ID, "with_note" => 0u8);
            accepted.returned = toss.bounced;
            let side = from_net_side(toss.side, toss.vside);
            if let Some(old) = self.inbox.push((accepted, side, toss.rally.unwrap_or(0), Some(icon)), &mut self.roster) {
                if let Some(i) = self.live.iter().position(|l| l.id == old) {
                    let l = self.live.remove(i);
                    self.poof(l);
                }
            }
            return;
        }
        let kind = ObjectKind::wire_kind(&toss.kind, toss.look.as_deref()).map_or(toss.kind.clone(), |k| k.id().to_string());
        let Some(mut accepted) = accept_toss(&kind, fling, toss.note.as_deref()) else {
            log!(Warn, "toss", "toss_unknown", "friend" => short(from));
            return;
        };
        // ⚾ 바쁜 친구가 되돌려 보낸 것이면 흐물이가 받아치지 않음 (혼자 랠리를 세지 않게)
        accepted.returned = toss.bounced;
        // 포스트잇 글은 안 적음 (있었는지만)
        log!(Info, "toss", "toss_in", "friend" => short(from), "kind" => kind, "with_note" => u8::from(toss.note.is_some()));
        let side = from_net_side(toss.side, toss.vside);
        if accepted.kind.is_poop() {
            // 메뉴로 꺼도 친구가 던진 똥은 받음 (끄는 건 동물이 싸는 것뿐, 받기 싫으면 집중·하우스)
            self.queue_poop(WaitingPoop::Tossed(accepted, side, screenpet_core::poop::sanitize_size(toss.scale)));
            return;
        }
        if let Some(old) = self.inbox.push((accepted, side, toss.rally.unwrap_or(0), None), &mut self.roster) {
            if let Some(i) = self.live.iter().position(|l| l.id == old) {
                let l = self.live.remove(i);
                self.poof(l);
            }
        }
    }

    /// 방금 그 친구에게 물건을 던졌음
    pub fn threw_lately(&self, friend: &str) -> bool {
        self.threw_at.get(friend).is_some_and(|t| t.elapsed() < BOUNCE_WINDOW)
    }

    /// 줄의 맨 앞 물건 하나를 들이고 창을 주문함. 창 일꾼이 아직 만드는 중이면 기다림 — 한 번에 하나만 만들게 (메인 스레드를 몰아붙이지 않고,
    /// 받은 물건이 창 없이 오래 날지 않게). 게임 루프는 창을 기다리지 않음
    fn admit_next_toss(&mut self, app: &AppHandle, base: &World) {
        if self.windows.as_ref().is_some_and(|w| w.waiting > 0) {
            return;
        }
        if let Some(poop) = self.poop_queue.pop_front() {
            self.admit_poop(app, base, poop);
            return;
        }
        let Some((accepted, side, rally, icon)) = self.inbox.pop() else { return };
        let id = self.next_id();
        let (x, y) = entry_point(base, side);
        let scale = self.scale_at(app, accepted.kind, base, x, y);
        let sticker = accepted.note.is_some();
        let dpi = self.dpi_hint;
        let world = self.world_for(base, dpi);
        let mut object = Object::flown_in(accepted.kind, world, side, accepted.fling, dpi, fastrand::u64(..));
        // ⚾ 친구가 보낸 캐치볼 랠리 (흐물이가 받아 되던지면 코어가 하나 올림). 되돌아온 것은 받아치지 않음
        object.set_rally(rally);
        if accepted.returned {
            object.mark_returned();
        }
        object.set_scale(scale);
        object.receive_note(accepted.note);
        // ⛄ 친구가 던진 눈덩이도 눈덩이 수에 듦
        let origin = if accepted.kind.is_snowball() { Origin::Snow } else { Origin::Foreign };
        if self.admit(app, id, object, origin, sticker, icon.clone()) {
            self.set_icon(id, icon);
        }
    }

    /// 기다리던 똥 하나를 들이고 창을 주문함
    fn admit_poop(&mut self, app: &AppHandle, base: &World, poop: WaitingPoop) {
        let id = self.next_id();
        let scale = match &poop {
            WaitingPoop::Dropped { scale, .. } | WaitingPoop::Tossed(_, _, scale) => *scale,
        };
        let dpi = self.dpi_hint;
        let world = self.world_for(base, dpi);
        let object = match poop {
            WaitingPoop::Dropped { kind, x, y, .. } => {
                let b = world.bounds;
                let margin = OBJECT_LOGICAL_SIZE * dpi / 2.0;
                Object::new(kind, x.clamp(b.left + margin, (b.right - margin).max(b.left + margin)), y, dpi, fastrand::u64(..))
            }
            WaitingPoop::Tossed(accepted, side, _) => {
                let mut o = Object::flown_in(accepted.kind, world, side, accepted.fling, dpi, fastrand::u64(..));
                // ⚾ 바쁜 친구가 되돌려 보낸 똥은 흐물이가 받아치지 않음
                if accepted.returned {
                    o.mark_returned();
                }
                o
            }
        };
        let mut object = object;
        object.set_scale(scale);
        self.admit(app, id, object, Origin::Poop, false, None);
    }

    /// 새 물건 번호를 줌
    fn next_id(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// 물건 키에 맞춘 세계 (없으면 만듦)
    fn world_for(&mut self, base: &World, dpi: f64) -> &World {
        self.world.get_or_insert_with(|| World::new(base.windows.clone(), base.work_areas.clone(), OBJECT_LOGICAL_SIZE * dpi))
    }

    /// 물건 수를 지키며 들임: 넘치면 그 쪽(내 것 10 / 받은 것 20 / 똥 10)의 가장 오래된 것이 뿅 사라짐.
    /// 물건은 바로 움직이기 시작하고 창은 일꾼에게 주문함 (attach_built가 붙임). 주문을 못 넣었으면 들이지 않고 false
    fn admit(&mut self, app: &AppHandle, id: u64, object: Object, origin: Origin, sticker: bool, icon: Option<IconImage>) -> bool {
        let order = WindowOrder { id, kind: object.kind, sticker, scale: object.scale(), icon };
        if !self.order_window(app, order) {
            return false;
        }
        // 🪨💩 거대한 것은 MAX_GIANTS개까지: 넘치면 가장 오래된 거대한 것이 뿅 (화면만 한 창이 쌓이면 앱이 느려짐)
        if object.kind.is_giant() {
            let giants: Vec<u64> = self.live.iter().filter(|l| l.object.kind.is_giant()).map(|l| l.id).collect();
            if let Some(old) = screenpet_core::object::giant_to_drop(&giants) {
                if let Some(i) = self.live.iter().position(|l| l.id == old) {
                    let l = self.live.remove(i);
                    self.roster.remove(l.id);
                    self.poof(l);
                }
            }
        }
        // 거대한 것·📌 장난감은 처음부터 클릭이 지나감 (창이 오면 화면 밖일 때 attach가 켬) — 나중에 바꾸면 Windows가 창을 통째로 다시 그려 깜빡임
        let click_through = object.kind.is_giant() || object.kind.is_toy();
        if let Some(old) = self.roster.add(id, origin) {
            if let Some(i) = self.live.iter().position(|l| l.id == old) {
                let l = self.live.remove(i);
                self.poof(l);
            }
        }
        let label = label_of(id);
        set_postit_state(&label, Some(object.postit()));
        self.live.push(Live {
            id,
            label,
            object,
            window: None,
            last_pos: None,
            last_pose: PosePayload { angle: 0.0, dx: 0.0 },
            note_open: false,
            hwnd: None,
            click_through,
            icon: None,
        });
        true
    }

    /// 📦 창 일꾼에게 창 하나를 주문함 (일꾼이 없거나 죽었으면 새로 띄움). 게임 루프는 기다리지 않음. 못 넣었으면 false
    fn order_window(&mut self, app: &AppHandle, order: WindowOrder) -> bool {
        if self.windows.is_none() {
            self.windows = start_window_worker(app);
        }
        let Some(worker) = self.windows.as_mut() else { return false };
        match worker.orders.send(order) {
            Ok(()) => {
                worker.waiting += 1;
                true
            }
            Err(mpsc::SendError(order)) => {
                // 일꾼이 죽었음 (창 만들다 패닉 등): 이미 만든 창은 붙이고 기다리던 물건은 치운 뒤 (attach_built), 새 일꾼에게 다시 넣어 봄
                self.attach_built(app);
                self.windows = None;
                self.drop_windowless();
                self.windows = start_window_worker(app);
                let Some(worker) = self.windows.as_mut() else { return false };
                if worker.orders.send(order).is_err() {
                    return false;
                }
                worker.waiting += 1;
                true
            }
        }
    }

    /// 📦 일꾼이 만든 창을 기다리던 물건에 붙임 (매 틱, 기다리지 않고 온 것만). 그 사이 치운 물건(뿅·던짐·들고 감·꿀꺽)의 창은 바로 닫고,
    /// 창을 못 만들었으면 물건도 치움 (예전에 build가 실패하면 물건이 생기지 않던 것과 같게)
    fn attach_built(&mut self, app: &AppHandle) {
        let Some(worker) = self.windows.as_mut() else { return };
        let mut arrived = Vec::new();
        let mut gone = false;
        loop {
            match worker.built.try_recv() {
                Ok(built) => {
                    worker.waiting = worker.waiting.saturating_sub(1);
                    arrived.push(built);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    gone = true;
                    break;
                }
            }
        }
        for mut built in arrived {
            let Some(i) = self.live.iter().position(|l| l.id == built.id) else {
                // 창을 기다리는 사이 치웠음
                if let Some(w) = built.window.take() {
                    let _ = w.destroy();
                }
                continue;
            };
            match built.window.take() {
                Some(window) => self.live[i].attach(app, window, &built),
                None => self.drop_at(i),
            }
        }
        if gone {
            log!(Warn, "toss", "window_worker_gone");
            self.windows = None;
            self.drop_windowless();
        }
    }

    /// 📦 창이 끝내 오지 않는 물건을 모두 치움 (일꾼이 죽었음)
    fn drop_windowless(&mut self) {
        while let Some(i) = self.live.iter().position(|l| l.window.is_none()) {
            self.drop_at(i);
        }
    }

    /// 📦 창 없는 물건 하나를 조용히 치움 (보일 창이 없으니 뿅도 없음)
    fn drop_at(&mut self, i: usize) {
        let l = self.live.remove(i);
        self.roster.remove(l.id);
        set_postit_state(&l.label, None);
        if let Some(w) = l.window {
            let _ = w.destroy();
        }
        self.sync_menu();
    }

    /// 물건 하나를 치움 (뿅)
    fn remove(&mut self, label: &str) {
        if let Some(i) = self.live.iter().position(|l| l.label == label) {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            self.poof(l);
        }
    }

    /// 창을 뿅 사라지게 하고 곧 닫음
    fn poof(&mut self, l: Live) {
        set_postit_state(&l.label, None);
        // 창을 기다리던 물건이면 창이 올 때 attach_built가 닫음
        if let Some(w) = l.window {
            self.fading.start(w, "poof", Instant::now() + POOF);
        }
    }

    /// 다 된 똥이 스르륵 사라짐
    fn fade(&mut self, l: Live) {
        set_postit_state(&l.label, None);
        if let Some(w) = l.window {
            self.fading.start(w, "fade", Instant::now() + FADE);
        }
    }

    /// `🗑️ 쓰레기 꺼내기 ▸` 글자의 내 물건 수 (바뀔 때만)
    fn sync_menu(&mut self) {
        let own = self.roster.own_count();
        if own != self.shown_own {
            self.shown_own = own;
            if let Some(m) = MENU.get() {
                let _ = m.set_text(menu_label(own));
            }
        }
    }

    /// 창 목록을 새로 읽었을 때 (100 ms마다). 달라진 게 없으면 할 일이 없음 — 물건은 계속 잠
    pub fn refresh(&mut self, windows: &[WindowInfo], work_areas: &[Rect], now: Instant) {
        self.window_ids.clear();
        self.window_ids.extend(windows.iter().map(|w| w.id));
        if self.live.is_empty() {
            self.world = None;
            return;
        }
        // 깨어 있는 물건만 화면 배율을 다시 읽음 (창에 묻는 일은 메인 스레드를 거침 — 창을 기다리는 물건은 건너뜀)
        for l in self.live.iter_mut().filter(|l| !l.object.is_asleep()) {
            if let Some(dpi) = l.window.as_ref().and_then(|w| w.scale_factor().ok()) {
                l.object.set_dpi(dpi);
            }
        }
        let same = self.world.as_ref().is_some_and(|w| w.windows == windows && w.work_areas == work_areas);
        if same {
            return;
        }
        let dpi = self.live.first().map_or(1.0, |l| l.object.dpi());
        self.world = Some(World::new(windows.to_vec(), work_areas.to_vec(), OBJECT_LOGICAL_SIZE * dpi));
        self.reseat_all(now);
    }

    /// 세계가 바뀌었음: 앉은 창을 따라가거나 떨어지기 시작함. 창을 따라 움직였으면 잠시 매 틱 따라감
    fn reseat_all(&mut self, now: Instant) {
        let Some(world) = self.world.as_ref() else { return };
        for l in &mut self.live {
            if l.object.reseat(world) && l.object.ridden_window().is_some() {
                self.ride_until = now + RIDE_FOLLOW;
            }
        }
    }

    /// 앉은 창이 움직이는 중이면 그 창의 경계만 새로 읽음 (창을 끌어 옮길 때 100 ms 계단이 아니게)
    fn follow_rides(&mut self, source: &mut dyn WindowSource, now: Instant) {
        if now >= self.ride_until {
            return;
        }
        let mut ids: Vec<u64> = self.live.iter().filter_map(|l| l.object.ridden_window()).collect();
        ids.sort_unstable();
        ids.dedup();
        let rects: Vec<(u64, Rect)> = ids.into_iter().filter_map(|id| source.rect_of(id).map(|r| (id, r))).collect();
        if self.world.as_mut().is_some_and(|w| w.patch_windows(&rects)) {
            self.reseat_all(now);
        }
    }

    /// 매 틱 (게임 루프)
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, app: &AppHandle, base: &World, dt: f64, now: Instant, cursor: (f64, f64), left_down: bool, away: bool, source: &mut dyn WindowSource, network: &Network, config: &Config) {
        self.fading.tick(now);
        self.attach_built(app);
        self.admit_next_toss(app, base);
        let secs = now.duration_since(self.started).as_secs_f64();
        self.release_volleys(secs, network, config);
        if self.live.is_empty() {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        self.follow_rides(source, now);
        let hidden = crate::windows_hidden(app);

        // 끌기·놓기
        if let Some(l) = self.live.iter_mut().find(|l| l.object.is_held()) {
            if left_down {
                l.object.pointer_move(cursor.0, cursor.1);
            } else if l.object.pointer_up() == Release::Click {
                // 받은 포스트잇을 눌렀음: 글자는 이때 처음 웹뷰로 가고, 코어에서는 지워짐
                if let Some(text) = l.object.read_note() {
                    l.set_note_open(true);
                    set_postit_state(&l.label, Some(l.object.postit()));
                    let _ = app.emit_to(l.label.as_str(), "object-sticker", false);
                    let _ = app.emit_to(l.label.as_str(), "object-note", text);
                }
            }
        }

        // 던져 넘길 수 있는 끝은 날아가는 물건이 있을 때만 계산함
        let open = {
            let budgets = &self.budgets;
            move |id: &str| network.is_connected(id) && budgets.get(id).is_none_or(|b| b.can_spend(secs))
        };
        let edges = if self.live.iter().all(|l| l.object.is_asleep()) { Default::default() } else { config.exit_edges(&open) };

        let Some(world) = self.world.as_ref() else { return };
        let mut exits: Vec<(usize, Exit)> = Vec::new();
        for (i, l) in self.live.iter_mut().enumerate() {
            let was_asleep = l.object.is_asleep();
            // 🕸️ 보이는 거미집 안으로 들어오면 걸림 (코어 object.rs)
            l.object.set_webs(&self.webs);
            if let Some(exit) = l.object.tick(dt, world, edges, screenpet_core::object::gusts_ok(hidden, away)) {
                exits.push((i, exit));
                continue;
            }
            if l.object.take_note_ended() {
                // 10초 끝: 글자를 지우고 창을 줄임 — 다시 볼 수 없음
                let _ = app.emit_to(l.label.as_str(), "object-note-end", ());
                l.set_note_open(false);
                set_postit_state(&l.label, Some(l.object.postit()));
            }
            let asleep = l.object.is_asleep();
            // 잠든 물건은 창이 제자리에 있으면 아무것도 하지 않음 (reseat가 옮겼으면 last_pos와 달라짐)
            l.place(world);
            // 커서는 잠든 물건 위로도 움직임 (거대한 것만 — 바뀔 때만 OS에 알림)
            l.sync_click_through(cursor);
            // 📦 창을 기다리는 물건: 움직임만 (모습은 창이 오면 attach가 꼭 다시 보내게 함)
            if l.window.is_none() {
                continue;
            }
            if asleep && was_asleep && l.last_pose == l.pose() {
                continue;
            }
            // 모습은 30 Hz면 충분함 (잠들 때의 마지막 모습은 꼭 보냄)
            let pose = l.pose();
            if pose != l.last_pose && (self.frame % 2 == 0 || asleep) {
                l.last_pose = pose;
                let _ = app.emit_to(l.label.as_str(), "object-pose", pose);
            }
        }
        for (i, exit) in exits.into_iter().rev() {
            let l = self.live.remove(i);
            self.roster.remove(l.id);
            self.toss_away(&l, exit, secs, network, config);
            set_postit_state(&l.label, None);
            if let Some(w) = &l.window {
                let _ = w.destroy();
            }
        }
        // ⛄ 던진 눈덩이가 동물(hit_pets)이나 바닥·창에 닿아 퍽: 뿅 치우고 그 자리를 알림 (눈가루)
        let mut i = 0;
        while i < self.live.len() {
            if self.live[i].object.take_burst() {
                let l = self.live.remove(i);
                self.roster.remove(l.id);
                let (_, height) = l.object.hit_box();
                self.bursts.push((l.object.body.x, l.object.body.y - height / 2.0));
                self.poof(l);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.live.len() {
            if self.live[i].object.expired() {
                let l = self.live.remove(i);
                self.roster.remove(l.id);
                self.fade(l);
            } else {
                i += 1;
            }
        }
        self.sync_menu();
    }

    /// 🌀 회오리가 빨아들일 수 있는 물건 (바닥에 놓인 작은 것과 이미 도는 것, 허리케인급만 드는 거대한 것은 heavy).
    /// cross = 지금 던져 넘길 수 있는 가까운 친구 쪽 끝 (예산이 남은 친구만, 거대한 것은 없음)
    pub fn storm_loose(&self, bounds: Rect, network: &Network, config: &Config, out: &mut Vec<Loose>) {
        if self.live.is_empty() {
            return;
        }
        let secs = self.started.elapsed().as_secs_f64();
        let open = |id: &str| network.is_connected(id) && self.budgets.get(id).is_none_or(|b| b.can_spend(secs));
        let edges = config.exit_edges(open);
        for l in self.live.iter().filter(|l| l.object.is_loose() || l.object.is_whirled() || l.object.is_heavy_loose()) {
            // 📌 그림이 없는 장난감은 친구에게 보내지 않음 (toss_away). 거대한 것도
            let heavy = l.object.kind.is_giant();
            let sendable = (!l.object.kind.is_toy() || l.icon.is_some()) && !heavy;
            let x = l.object.body.x;
            let near_left = x - bounds.left <= bounds.right - x;
            let cross = match (edges.left && sendable, edges.right && sendable) {
                (true, true) => Some(if near_left { Side::Left } else { Side::Right }),
                (true, false) => Some(Side::Left),
                (false, true) => Some(Side::Right),
                (false, false) => None,
            };
            out.push(Loose { thing: Thing::Object(l.id), x, y: l.object.body.y, cross, heavy });
        }
    }

    /// 🌀 회오리가 시킨 것: 붙들어 돌리거나, 친구 쪽으로 던지거나 (launch — 끝을 넘으면 toss_away), 이 화면에 휙 (blow)
    pub fn storm_apply(&mut self, whirl: &Whirl) {
        let Thing::Object(id) = whirl.thing() else { return };
        let Some(l) = self.live.iter_mut().find(|l| l.id == id) else { return };
        match *whirl {
            Whirl::Hold { x, y, angle, .. } => {
                l.object.whirl_at(x, y, angle);
            }
            Whirl::Fling { vx, vy, cross: true, .. } => {
                if l.object.is_whirled() {
                    l.object.launch(vx, vy);
                }
            }
            Whirl::Fling { vx, vy, cross: false, .. } => {
                l.object.blow(vx, vy);
            }
        }
    }

    /// 🌀 돌풍이 막 일었음: 바닥에 놓인 작은 것 대부분이 바람 쪽으로 휙 (모두 숨겨 둔 동안엔 앱이 부르지 않음)
    pub fn gust(&mut self, gust: Gust) {
        for l in self.live.iter_mut().filter(|l| l.object.is_loose()) {
            if fastrand::f64() < 0.7 {
                let (vx, vy) = typhoon::blow_velocity(gust.dir, gust.strength, l.object.dpi(), fastrand::f64());
                l.object.blow(vx, vy);
            }
        }
    }

    /// "물건 10개 던지기": 예산이 되는 만큼 던짐. 그 사이 연결이 끊기거나 친구가 지워지면 남은 것은 버림
    fn release_volleys(&mut self, secs: f64, network: &Network, config: &Config) {
        let (budgets, threw_at) = (&mut self.budgets, &mut self.threw_at);
        self.volleys.retain(|id, volley| {
            let Some(friend) = config.friend(id).filter(|_| network.is_connected(id)) else { return false };
            let count = volley.release(budgets.entry(id.clone()).or_default(), secs);
            if count > 0 {
                threw_at.insert(id.clone(), Instant::now());
                log!(Info, "toss", "toss_volley", "friend" => short(id), "n" => count);
            }
            let (side, vside) = to_net_side(visit::opposite(friend.side));
            let mut rng = fastrand::Rng::new();
            for _ in 0..count {
                let (kind, fling) = volley_piece(&mut rng);
                network.toss_to(id, Toss { kind: kind.wire_id().to_string(), side, vside, fling: Some(to_net_fling(fling)), note: None, bounced: false, scale: None, look: None, rally: None, icon: None });
            }
            !volley.is_done()
        });
    }

    /// 물건이 side 끝을 넘어갔음: 그 끝의 연결된 친구 하나에게 날려 보냄 (친구 쪽에서 반대편 끝으로 들어옴). 돌아오지 않음
    fn toss_away(&mut self, l: &Live, exit: Exit, secs: f64, network: &Network, config: &Config) {
        // 📌 장난감은 그림과 함께만 (🔓 v0.6.5 — 그림이 없으면 보내지 않음)
        let icon = l.icon.as_ref().map(wire_icon).filter(|_| l.object.kind.is_toy());
        if l.object.kind.is_toy() && icon.is_none() {
            return;
        }
        let friends = {
            let budgets = &self.budgets;
            config.friends_at(exit.side, |id| network.is_connected(id) && budgets.get(id).is_none_or(|b| b.can_spend(secs)))
        };
        if friends.is_empty() {
            return;
        }
        let friend = friends[fastrand::usize(..friends.len())].id.clone();
        self.budgets.entry(friend.clone()).or_default().spend(secs);
        self.threw_at.insert(friend.clone(), Instant::now());
        if let Some(i) = &icon {
            self.sent_icons.insert(friend.clone(), (Instant::now(), i.clone()));
        }
        let (side, vside) = to_net_side(visit::opposite(exit.side));
        let toss = Toss { kind: l.object.kind.wire_id().to_string(), side, vside, fling: Some(to_net_fling(exit.fling)), note: l.object.note_to_send(), bounced: false, scale: l.object.kind.is_poop().then(|| l.object.scale()), look: l.object.kind.look().map(str::to_string), rally: (l.object.rally() > 0).then(|| l.object.rally()), icon };
        log!(Info, "toss", "toss_out", "friend" => short(&friend), "kind" => l.object.kind.id(), "with_note" => u8::from(toss.note.is_some()));
        network.toss_to(&friend, toss);
    }
}

/// 친구가 던진 물건이 들어올 화면 끝 언저리 (거대한 물건의 크기를 그 화면에 맞춤)
fn entry_point(world: &World, side: Side) -> (f64, f64) {
    let b = world.bounds;
    match side {
        Side::Left => (b.left + 1.0, b.bottom - 1.0),
        Side::Right => (b.right - 1.0, b.bottom - 1.0),
        _ => ((b.left + b.right) / 2.0, b.bottom - 1.0),
    }
}

/// 📌 친구에게 보낼 아이콘 그림 (48 px로 줄인 base64 RGBA — 코어 icon::to_wire)
fn wire_icon(icon: &IconImage) -> screenpet_net::WireIcon {
    let (size, rgba) = icon::to_wire(icon);
    screenpet_net::WireIcon { size, rgba }
}

/// 🤹 흐물이가 던진 물건은 이만큼 그 흐물이를 맞히지 않음 (손에서 막 떠난 물건이 제 몸과 겹침)
const THROWER_SPARED_SECS: f64 = 1.0;

/// 📌 장난감 아이콘을 읽어 볼 창 수 (앞에서부터 읽히는 것 하나)
const TOY_CANDIDATES: usize = 8;

/// 던지고 이만큼 안에 되돌아온 물건만 내 것으로 봄
const BOUNCE_WINDOW: Duration = Duration::from_secs(30);

/// 물건 창 라벨 (`object-<id>` — 창이 오기 전에도 정해져 있음)
fn label_of(id: u64) -> String {
    format!("{PREFIX}{id}")
}

/// 📦 물건 창 일꾼 스레드를 띄움: 주문을 하나씩 받아 build_window로 만들고 돌려보냄 (한 번에 하나만 만듦).
/// Objects가 사라지면 (주문 줄이 끊기면) 끝남. 스레드를 못 띄웠으면 None
fn start_window_worker(app: &AppHandle) -> Option<WindowWorker> {
    let (orders, order_rx) = mpsc::channel::<WindowOrder>();
    let (built_tx, built) = mpsc::channel::<BuiltWindow>();
    let app = app.clone();
    let started = std::thread::Builder::new().name("object-windows".into()).spawn(move || {
        for order in order_rx {
            if built_tx.send(make_window(&app, order)).is_err() {
                break;
            }
        }
    });
    started.ok().map(|_| WindowWorker { orders, built, waiting: 0 })
}

/// 📦 일꾼 스레드에서 주문 하나의 창을 만듦 (메인 스레드를 기다리는 일 — 창 만들기·배율·핸들 읽기 — 은 모두 여기서)
fn make_window(app: &AppHandle, order: WindowOrder) -> BuiltWindow {
    let made = build_window(app, order.id, order.kind, order.sticker, order.scale, order.icon.as_ref());
    let hwnd = made.as_ref().and_then(|(w, _)| crate::overlay::hwnd_of(w));
    let (window, dpi) = match made {
        Some((w, dpi)) => (Some(w), dpi),
        None => (None, 1.0),
    };
    BuiltWindow { id: order.id, window, scale: order.scale, dpi, hwnd }
}

/// 물건 창 하나를 만듦 (화면 밖에 조용히 — 일꾼 스레드에서만 부름: build()가 메인 스레드를 기다림). 만들었으면 창과 그 화면 배율.
/// icon = 📌 장난감에 그릴 앱 아이콘 (그 창의 초기화 스크립트에만 들어감 — base64 글자뿐이라 JS 문자열에 그대로 넣음)
fn build_window(app: &AppHandle, id: u64, kind: ObjectKind, sticker: bool, scale: f64, icon: Option<&IconImage>) -> Option<(WebviewWindow, f64)> {
    let label = label_of(id);
    let shape = shape_of(kind);
    let (w, h) = (shape.window_side_at(scale), shape.window_side_at(scale));
    let (px, py) = shape.pivot_logical();
    // kind.id()와 숫자는 코어 목록의 값뿐임 (친구가 보낸 글자가 아님)
    let icon = icon.map_or("null".to_string(), |i| format!("{{ size: {}, rgba: \"{}\" }}", i.size, icon::base64(&i.rgba)));
    let built = crate::overlay::finish_quiet(
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("object.html".into()))
            .title("ScreenPet Object")
            .initialization_script(&format!("window.OBJECT = {{ kind: {:?}, sticker: {sticker}, side: {w}, pivot: [{px}, {py}], scale: {scale}, icon: {icon} }};", kind.id()))
            .inner_size(w, h)
            .accept_first_mouse(true),
    );
    match built {
        Ok(window) => {
            // 다른 배율의 모니터에서도 논리 크기를 맞춤
            let dpi = window.scale_factor().unwrap_or(1.0);
            crate::overlay::resize_quietly(&window, crate::overlay::hwnd_of(&window), (w * dpi).round() as u32, (h * dpi).round() as u32);
            // 모두 숨긴 동안 온 물건은 "보이기"가 함께 보임
            if crate::windows_hidden(app) {
                crate::overlay::hide_quietly(&window);
            }
            Some((window, dpi))
        }
        Err(e) => {
            eprintln!("failed to create object window: {e}");
            None
        }
    }
}

/// `물건 ▸` 하위 메뉴 (tray::build가 부름)
pub fn build_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let sub = Submenu::new(app, menu_label(0), true)?;
    // 똥은 메뉴로 꺼내지 않음 (동물이 쌈). 거대한 돌·똥은 쓰레기 다음에 줄을 긋고
    for kind in ObjectKind::MENU {
        if kind == ObjectKind::GIANTS[0] {
            sub.append(&PredefinedMenuItem::separator(app)?)?;
        }
        sub.append(&MenuItem::with_id(app, format!("{MENU_SPAWN}{}", kind.id()), kind.label(), true, None::<&str>)?)?;
    }
    sub.append(&PredefinedMenuItem::separator(app)?)?;
    sub.append(&MenuItem::with_id(app, MENU_CLEAR, "🧹 모든 물건 다 치우기", true, None::<&str>)?)?;
    let _ = MENU.set(sub.clone());
    Ok(sub)
}

/// 물건 메뉴 항목이면 처리하고 true (트레이의 메뉴 처리기에서 먼저 부름)
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let input = if id == MENU_CLEAR {
        ObjectInput::ClearAll
    } else if let Some(kind) = id.strip_prefix(MENU_SPAWN) {
        let Some(kind) = ObjectKind::from_id(kind) else { return true };
        ObjectInput::Spawn(kind)
    } else if let Some(label) = id.strip_prefix(MENU_NOTE) {
        ObjectInput::EditPostit { label: label.to_string() }
    } else if let Some(label) = id.strip_prefix(MENU_PEEL) {
        ObjectInput::PeelPostit { label: label.to_string() }
    } else if let Some(label) = id.strip_prefix(MENU_REMOVE) {
        ObjectInput::Remove { label: label.to_string() }
    } else {
        return false;
    };
    app.state::<AppState>().send(Input::Object(input));
    true
}

/// 물건 창 라벨인지 봄
fn is_object_label(label: &str) -> bool {
    label.starts_with(PREFIX)
}

/// 물건을 눌렀음 (왼쪽 버튼) — 부른 창의 라벨로 어느 물건인지 찾음
#[tauri::command]
pub fn object_down(window: WebviewWindow, state: State<AppState>) {
    if is_object_label(window.label()) {
        state.send(Input::Object(ObjectInput::Down { label: window.label().to_string() }));
    }
}

/// 물건 우클릭 메뉴. show_menu처럼 async — 모달 메뉴 루프가 WebView2 IPC 콜백 안에서 돌지 않게
#[tauri::command]
pub async fn object_menu(window: tauri::Window) -> Result<(), String> {
    let label = window.label().to_string();
    if !is_object_label(&label) {
        return Ok(());
    }
    let postit = POSTITS.lock().unwrap().as_ref().and_then(|m| m.get(&label).copied()).unwrap_or_default();
    let app = window.app_handle().clone();
    let build = || -> tauri::Result<Menu<Wry>> {
        let note = match postit {
            // 똥엔 포스트잇이 없음
            Postit::Never => None,
            Postit::Mine => Some(MenuItem::with_id(&app, format!("{MENU_PEEL}{label}"), "📝 포스트잇 떼기", true, None::<&str>)?),
            // 친구가 붙인 포스트잇은 덮어쓰거나 뗄 수 없음 (읽는 중에도)
            other => Some(MenuItem::with_id(&app, format!("{MENU_NOTE}{label}"), "📝 포스트잇 붙이기…", other == Postit::None, None::<&str>)?),
        };
        let remove = MenuItem::with_id(&app, format!("{MENU_REMOVE}{label}"), "🗑️ 이것만 치우기", true, None::<&str>)?;
        let clear = MenuItem::with_id(&app, MENU_CLEAR, "🧹 모든 물건 다 치우기", true, None::<&str>)?;
        match &note {
            Some(note) => Menu::with_items(&app, &[note, &remove, &clear]),
            None => Menu::with_items(&app, &[&remove, &clear]),
        }
    };
    let menu = build().map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// 포스트잇 쓰기 창
fn show_postit_window(app: &AppHandle) {
    let center = |_: &AppHandle, window: &WebviewWindow, _: (f64, f64)| {
        let _ = window.center();
    };
    crate::overlay::show_form(app, &POSTIT_FORM, center, |_| {
        let _ = app.emit_to(POSTIT_LABEL, "postit-open", MAX_NOTE_CHARS);
    });
}

/// 포스트잇 창 최대 글자 수 (첫 로드용)
#[tauri::command]
pub fn postit_max(window: WebviewWindow) -> Result<usize, String> {
    if window.label() != POSTIT_LABEL {
        return Err("wrong window".into());
    }
    Ok(MAX_NOTE_CHARS)
}

/// 포스트잇 창 "붙이기": 게임 루프로 보내고 창을 숨김 (다듬기·80자 제한은 코어가 함)
#[tauri::command]
pub fn postit_submit(window: WebviewWindow, state: State<AppState>, text: String) {
    if window.label() != POSTIT_LABEL {
        return;
    }
    let text: String = text.chars().take(256).collect();
    state.send(Input::Object(ObjectInput::SubmitPostit(text)));
    let _ = window.hide();
}

/// 포스트잇 창 "닫기"
#[tauri::command]
pub fn postit_close(window: WebviewWindow) {
    if window.label() == POSTIT_LABEL {
        let _ = window.hide();
    }
}
