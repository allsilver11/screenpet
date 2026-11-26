//! 🚗 탈것 창 (`vehicle-<n>`, vehicle.html) — 탈것 움직임·펫 태우기·마주침은 코어 (screenpet_core::vehicle::Garage), 여기는 창·메뉴·누르기.
//! `🚗 탈것 꺼내기 ▸` 메뉴로 꺼내고 (한 화면에 4대), 우클릭 "이것만 치우기", "🧹 모든 물건 다 치우기"로 치움.
//! 📦 창은 물건 창처럼 게임 루프 밖 일꾼 스레드 (vehicle-windows)가 만들어 돌려보냄 — 오기 전에도 탈것은 움직이고 탈 수 있음 (창 일만 건너뜀).
//! 틱마다 코어 탈것 목록과 창을 맞춤: 새 탈것은 창을 주문하고, 없어진 탈것 (치움·친구 화면으로 감·오래된 남의 것)의 창은 닫음.
//! 끼익·쿵은 그 창에 `vehicle-fx`로 알림 (웹뷰가 먼지·💥). 펫 창은 탈것 좌석에 앉고 다리 몫은 펫 웹뷰가 자름 (render.seat),
//! 기운 탈것 (🚁 날며 숙임)과 같이 기욺 (render.lean). 탈것 배율은 창의 배율을 따름 (set_dpi_hint).

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use screenpet_core::brain::Pet;
use screenpet_core::geom::Rect;
use screenpet_core::vehicle::{window_logical, Garage, VehicleEvent, VehicleKind, VehiclePose, VehicleSpot, MAX_VEHICLES};
use screenpet_core::world::{Side, World};
use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

use crate::game_loop::Input;
use crate::AppState;

/// 탈것 창 라벨 앞부분
const PREFIX: &str = "vehicle-";
/// 메뉴 id: 탈것 꺼내기 (뒤에 종류)
const MENU_SPAWN: &str = "vehicle:spawn:";
/// 메뉴 id: 이 탈것만 치우기 (뒤에 창 라벨)
const MENU_REMOVE: &str = "vehicle:remove:";
/// 메뉴 id: 모든 물건 다 치우기 (물건 메뉴와 같은 id — 탈것도 같이 치움)
const MENU_CLEAR: &str = "object:clear";
/// 뿅 사라지는 동안 기다렸다 닫음 (vehicle.html poof 0.3초)
const POOF: Duration = Duration::from_millis(400);

/// `🚗 탈것 꺼내기 ▸` 하위 메뉴 (글자에 탈것 수)
static MENU: OnceLock<Submenu<Wry>> = OnceLock::new();

/// `🚗 탈것 꺼내기 ▸` 메뉴 글자
fn menu_label(n: usize) -> String {
    format!("🚗 탈것 꺼내기 ({n}/{MAX_VEHICLES})")
}

/// 게임 루프로 들어오는 탈것 입력
pub enum VehicleInput {
    Spawn(VehicleKind),
    /// 창의 탈것을 눌렀음
    Down { label: String },
    /// 우클릭 "이것만 치우기"
    Remove { label: String },
}

/// 웹뷰에 보내는 끼익·쿵 (`vehicle-fx`)
#[derive(Debug, Clone, Copy, Serialize)]
struct FxPayload {
    /// "screech" 또는 "crash"
    kind: &'static str,
}

/// 화면에 있는 탈것 창 하나
struct Live {
    id: u64,
    label: String,
    /// 일꾼이 만드는 동안엔 None
    window: Option<WebviewWindow>,
    hwnd: Option<isize>,
    last_pos: Option<(i32, i32)>,
    last_pose: Option<VehiclePose>,
}

/// 📦 일꾼에게 보내는 주문
struct WindowOrder {
    id: u64,
    kind: VehicleKind,
}

/// 📦 일꾼이 만든 창 (못 만들었으면 None — 탈것도 치움)
struct BuiltWindow {
    id: u64,
    window: Option<WebviewWindow>,
    hwnd: Option<isize>,
}

/// 📦 탈것 창 일꾼과 잇는 줄
struct WindowWorker {
    orders: Sender<WindowOrder>,
    built: Receiver<BuiltWindow>,
}

/// 🚗 탈것 상태 (게임 루프가 가짐): 코어 차고와 창들
pub struct Vehicles {
    garage: Garage,
    live: Vec<Live>,
    worker: Option<WindowWorker>,
    fading: crate::overlay::Fading,
    spots: Vec<VehicleSpot>,
    dpi_hint: f64,
    shown: usize,
    /// 🪃 친구 화면 쪽으로 날아간 던진 탈것 (take_thrown이 꺼냄)
    thrown: Vec<(VehicleKind, Side, screenpet_core::visit::Fling)>,
}

impl Vehicles {
    /// 🔄 탈것 창 웹뷰를 다시 읽음 (새로고침·깨어남). 자리·모습은 resend가 다시 보냄
    pub fn reload_windows(&mut self) {
        for w in self.live.iter().filter_map(|l| l.window.as_ref()) {
            let _ = w.reload();
        }
    }

    /// 다시 읽은 탈것 창(label)에 자리·모습을 다시 보냄 (페이지가 듣기를 마쳤을 때)
    pub fn resend(&mut self, label: &str) {
        for l in self.live.iter_mut().filter(|l| l.label == label) {
            l.last_pos = None;
            l.last_pose = None;
        }
    }

    /// 빈 상태
    pub fn new() -> Vehicles {
        Vehicles { garage: Garage::new(fastrand::u64(..)), live: Vec::new(), worker: None, fading: Default::default(), spots: Vec::new(), dpi_hint: 1.0, shown: 0, thrown: Vec::new() }
    }

    /// 새 탈것에 쓸 화면 배율 (내 동물 창의 배율). 창이 온 탈것은 그 창의 배율로 맞춤 —
    /// 배율이 다른 모니터로 넘어가 창 크기가 바뀌어도 좌석 (펫 창 자리)이 그림에 맞게 (WORLD_REFRESH마다)
    pub fn set_dpi_hint(&mut self, dpi: f64) {
        if dpi.is_finite() && dpi > 0.0 {
            self.dpi_hint = dpi;
        }
        for l in &self.live {
            if let Some(d) = l.window.as_ref().and_then(|w| w.scale_factor().ok()) {
                self.garage.set_dpi(l.id, d);
            }
        }
    }

    /// 사용자가 탈것을 잡고 있음
    pub fn is_held(&self) -> bool {
        self.garage.is_held()
    }

    /// 펫들에게 보일 탈것 자리 (update 전에 notice_vehicles로)
    pub fn spots(&mut self) -> &[VehicleSpot] {
        self.garage.spots(&mut self.spots);
        &self.spots
    }

    /// 입력 처리. cursor = 커서, busy = 펫을 잡고 있거나 음식을 들고 있음
    /// wa = 커서 쪽 작업 영역
    pub fn handle<T: AsRef<Pet> + AsMut<Pet>>(&mut self, app: &AppHandle, input: VehicleInput, wa: Rect, cursor: (f64, f64), busy: bool, pets: &mut [T]) {
        match input {
            VehicleInput::Spawn(kind) => {
                // 커서 쪽 작업 영역 위에서 떨어뜨림 (탈것 높이만큼 위)
                let half = kind.spec().w / 2.0 * self.dpi_hint;
                let x = cursor.0.clamp(wa.left + half, (wa.right - half).max(wa.left + half));
                let y = (cursor.1 - 40.0 * self.dpi_hint).max(wa.top + kind.spec().h * self.dpi_hint);
                self.garage.spawn(kind, x, y, self.dpi_hint);
            }
            VehicleInput::Remove { label } => {
                if let Some(id) = self.live.iter().find(|l| l.label == label).map(|l| l.id) {
                    self.garage.remove(id);
                }
            }
            VehicleInput::Down { label } => {
                if busy || self.garage.is_held() {
                    return;
                }
                if let Some(id) = self.live.iter().find(|l| l.label == label).map(|l| l.id) {
                    self.garage.pointer_down(id, cursor.0, cursor.1, pets);
                }
            }
        }
        self.sync(app);
    }

    /// 🧹 모두 치움 (모든 물건 다 치우기)
    pub fn clear_all(&mut self, app: &AppHandle) {
        self.garage.clear();
        self.sync(app);
    }

    /// 사용자가 방금 놓은 펫: 탈것 위면 태움
    pub fn offer_drop(&mut self, pet: &mut Pet) -> bool {
        self.garage.offer_drop(pet)
    }

    /// 친구 화면으로 몰고 갔던 내 탈것 (펫이 돌아올 때 한 번)
    pub fn take_parked_away(&mut self, driver: u64) -> Option<VehicleKind> {
        self.garage.take_parked_away(driver)
    }

    /// 친구 화면으로 몰고 간 탈것을 기억함 (돌아올 때 다시 몰고 옴)
    pub fn remember_parked_away(&mut self, driver: u64, kind: VehicleKind) {
        self.garage.remember_parked_away(driver, kind);
    }

    /// 탈것을 몰고 화면 끝(side)으로 들어옴 (친구 펫이 몰고 옴 / 내 펫이 돌아옴). 못 놓으면 false (그냥 걸어 들어옴)
    pub fn arrive(&mut self, app: &AppHandle, kind: VehicleKind, world: &World, side: Side, dpi: f64, pet: &mut Pet) -> bool {
        let ok = self.garage.arrive(kind, world, side, dpi, pet).is_some();
        self.sync(app);
        ok
    }

    /// 🪃 친구가 던진 탈것: side 끝으로 날아 들어옴 (내 것이 됨). 자리가 없으면 false
    pub fn catch_thrown(&mut self, app: &AppHandle, kind: VehicleKind, world: &World, side: Side, fling: screenpet_core::visit::Fling, dpi: f64) -> bool {
        let ok = self.garage.catch_thrown(kind, world, side, fling, dpi).is_some();
        self.sync(app);
        ok
    }

    /// 🪃 던진 탈것을 받을 친구가 연결된 끝 (게임 루프가 매 틱)
    pub fn set_throw_edges(&mut self, (left, right): (bool, bool)) {
        self.garage.set_throw_edges(left, right);
    }

    /// 🪃 지난 tick에 친구 화면 쪽 끝을 넘어간 던진 탈것 (종류, 끝, 이어 갈 움직임) — 게임 루프가 친구에게 보냄
    pub fn take_thrown(&mut self) -> Vec<(VehicleKind, Side, screenpet_core::visit::Fling)> {
        std::mem::take(&mut self.thrown)
    }

    /// 손님이 떠났음: 그 손님이 몰고 온 탈것도 뿅
    pub fn guest_left(&mut self, app: &AppHandle, key: u64) {
        if self.garage.guest_left(key).is_some() {
            self.sync(app);
        }
    }

    /// 한 틱: 잡은 탈것 끌기·놓기, 코어 차고 한 틱, 창 맞추기·옮기기, 끼익·쿵. 친구 화면으로 몰고 나간 것을 돌려줌
    pub fn tick<T: AsRef<Pet> + AsMut<Pet>>(&mut self, app: &AppHandle, world: &World, dt: f64, cursor: (f64, f64), left_down: bool, pets: &mut [T]) -> Vec<(u64, VehicleKind, Side)> {
        if self.garage.is_held() {
            if left_down {
                self.garage.pointer_move(cursor.0, cursor.1);
            } else {
                self.garage.pointer_up();
            }
        }
        self.fading.tick(Instant::now());
        if self.garage.vehicles().is_empty() && self.live.is_empty() {
            return Vec::new();
        }
        self.garage.tick(dt, world, pets);
        let mut out = Vec::new();
        for e in self.garage.take_events() {
            match e {
                VehicleEvent::Screech { id, .. } => self.fx(app, id, "screech"),
                VehicleEvent::Crash { id, .. } => self.fx(app, id, "crash"),
                VehicleEvent::DroveOut { id, side, kind, driver } => {
                    // 화면 밖으로 다 나갔음: 뿅 없이 닫음
                    if let Some(i) = self.live.iter().position(|l| l.id == id) {
                        if let Some(w) = self.live.remove(i).window {
                            let _ = w.destroy();
                        }
                    }
                    out.push((driver, kind, side));
                }
                VehicleEvent::ThrownOut { id, side, kind, fling } => {
                    // 🪃 친구 화면으로 날아갔음: 뿅 없이 닫고 게임 루프가 보냄 (take_thrown)
                    if let Some(i) = self.live.iter().position(|l| l.id == id) {
                        if let Some(w) = self.live.remove(i).window {
                            let _ = w.destroy();
                        }
                    }
                    self.thrown.push((kind, side, fling));
                }
            }
        }
        let _ = self.garage.take_expired();
        self.sync(app);
        self.place();
        out
    }

    /// 끼익·쿵을 그 창에 알림
    fn fx(&self, app: &AppHandle, id: u64, kind: &'static str) {
        if let Some(l) = self.live.iter().find(|l| l.id == id) {
            let _ = app.emit_to(l.label.as_str(), "vehicle-fx", FxPayload { kind });
        }
    }

    /// 코어 탈것 목록과 창을 맞춤: 새것은 창을 주문하고, 없어진 것은 뿅, 만든 창은 붙임. 메뉴 글자도
    fn sync(&mut self, app: &AppHandle) {
        self.attach_built(app);
        let ids: Vec<(u64, VehicleKind)> = self.garage.vehicles().iter().map(|v| (v.id(), v.kind())).collect();
        let mut i = 0;
        while i < self.live.len() {
            if ids.iter().any(|&(id, _)| id == self.live[i].id) {
                i += 1;
            } else {
                let l = self.live.remove(i);
                if let Some(w) = l.window {
                    self.fading.start(w, "poof", Instant::now() + POOF);
                }
            }
        }
        for (id, kind) in ids {
            if self.live.iter().any(|l| l.id == id) {
                continue;
            }
            if !self.order_window(app, WindowOrder { id, kind }) {
                self.garage.remove(id);
                continue;
            }
            self.live.push(Live { id, label: format!("{PREFIX}{id}"), window: None, hwnd: None, last_pos: None, last_pose: None });
        }
        let n = self.garage.vehicles().len();
        if n != self.shown {
            self.shown = n;
            if let Some(m) = MENU.get() {
                let _ = m.set_text(menu_label(n));
            }
        }
    }

    /// 📦 일꾼에게 창을 주문함 (일꾼이 없거나 죽었으면 다시 띄움). 못 보냈으면 false
    fn order_window(&mut self, app: &AppHandle, order: WindowOrder) -> bool {
        if self.worker.is_none() {
            self.worker = start_worker(app);
        }
        let Some(worker) = self.worker.as_ref() else { return false };
        match worker.orders.send(order) {
            Ok(()) => true,
            Err(mpsc::SendError(order)) => {
                self.worker = start_worker(app);
                self.worker.as_ref().is_some_and(|w| w.orders.send(order).is_ok())
            }
        }
    }

    /// 📦 일꾼이 만든 창을 붙임. 그 사이 없어진 탈것의 창은 바로 닫고, 못 만들었으면 탈것도 치움
    fn attach_built(&mut self, app: &AppHandle) {
        let Some(worker) = self.worker.as_ref() else { return };
        let mut arrived = Vec::new();
        loop {
            match worker.built.try_recv() {
                Ok(b) => arrived.push(b),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.worker = None;
                    break;
                }
            }
        }
        for mut b in arrived {
            let Some(l) = self.live.iter_mut().find(|l| l.id == b.id) else {
                if let Some(w) = b.window.take() {
                    let _ = w.destroy();
                }
                continue;
            };
            match b.window.take() {
                Some(w) => {
                    if crate::windows_hidden(app) {
                        crate::overlay::hide_quietly(&w);
                    }
                    l.window = Some(w);
                    l.hwnd = b.hwnd;
                    l.last_pos = None;
                    l.last_pose = None;
                }
                None => {
                    let id = b.id;
                    self.live.retain(|l| l.id != id);
                    self.garage.remove(id);
                }
            }
        }
    }

    /// 창을 탈것 자리로 옮기고, 모습이 바뀌었으면 웹뷰에 보냄
    fn place(&mut self) {
        for l in &mut self.live {
            let Some(window) = &l.window else { continue };
            let Some(v) = self.garage.get(l.id) else { continue };
            let (x, y, _, _) = v.window_rect();
            let pos = (x.round() as i32, y.round() as i32);
            if l.last_pos != Some(pos) {
                crate::overlay::move_quietly(window, l.hwnd, pos.0, pos.1);
                l.last_pos = Some(pos);
            }
            let pose = v.pose();
            if l.last_pose != Some(pose) {
                let _ = window.emit_to(l.label.as_str(), "vehicle-pose", pose);
                l.last_pose = Some(pose);
            }
        }
    }
}

/// 📦 탈것 창 일꾼 스레드를 띄움 (주문을 하나씩 만들어 돌려보냄)
fn start_worker(app: &AppHandle) -> Option<WindowWorker> {
    let (orders, order_rx) = mpsc::channel::<WindowOrder>();
    let (built_tx, built) = mpsc::channel::<BuiltWindow>();
    let app = app.clone();
    let started = std::thread::Builder::new().name("vehicle-windows".into()).spawn(move || {
        for order in order_rx {
            let window = build_window(&app, order.id, order.kind);
            let hwnd = window.as_ref().and_then(crate::overlay::hwnd_of);
            if built_tx.send(BuiltWindow { id: order.id, window, hwnd }).is_err() {
                break;
            }
        }
    });
    started.ok().map(|_| WindowWorker { orders, built })
}

/// 탈것 창 하나를 만듦 (화면 밖에 조용히 — 일꾼 스레드에서만: build()가 메인 스레드를 기다림)
fn build_window(app: &AppHandle, id: u64, kind: VehicleKind) -> Option<WebviewWindow> {
    let label = format!("{PREFIX}{id}");
    let (w, h) = window_logical(kind);
    // kind.id()는 코어 목록의 값뿐임 (친구가 보낸 글자가 아님)
    let built = crate::overlay::finish_quiet(
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("vehicle.html".into()))
            .title("ScreenPet Vehicle")
            .initialization_script(&format!("window.VEHICLE = {{ kind: {:?} }};", kind.id()))
            .inner_size(w, h)
            .accept_first_mouse(true),
    );
    match built {
        Ok(window) => {
            let dpi = window.scale_factor().unwrap_or(1.0);
            crate::overlay::resize_quietly(&window, crate::overlay::hwnd_of(&window), (w * dpi).round() as u32, (h * dpi).round() as u32);
            if crate::windows_hidden(app) {
                crate::overlay::hide_quietly(&window);
            }
            Some(window)
        }
        Err(e) => {
            eprintln!("failed to create vehicle window: {e}");
            None
        }
    }
}

/// `🚗 탈것 꺼내기 ▸` 하위 메뉴 (tray::build가 부름)
pub fn build_menu(app: &AppHandle) -> tauri::Result<Submenu<Wry>> {
    let sub = Submenu::new(app, menu_label(0), true)?;
    for kind in VehicleKind::ALL {
        sub.append(&MenuItem::with_id(app, format!("{MENU_SPAWN}{}", kind.id()), kind.label(), true, None::<&str>)?)?;
    }
    sub.append(&PredefinedMenuItem::separator(app)?)?;
    sub.append(&MenuItem::with_id(app, MENU_CLEAR, "🧹 모든 물건 다 치우기", true, None::<&str>)?)?;
    let _ = MENU.set(sub.clone());
    Ok(sub)
}

/// 탈것 메뉴 항목이면 처리하고 true (트레이의 메뉴 처리기에서)
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let input = if let Some(kind) = id.strip_prefix(MENU_SPAWN) {
        let Some(kind) = VehicleKind::from_id(kind) else { return true };
        VehicleInput::Spawn(kind)
    } else if let Some(label) = id.strip_prefix(MENU_REMOVE) {
        VehicleInput::Remove { label: label.to_string() }
    } else {
        return false;
    };
    app.state::<AppState>().send(Input::Vehicle(input));
    true
}

/// 탈것 창 라벨인지
fn is_vehicle_label(label: &str) -> bool {
    label.starts_with(PREFIX)
}

/// 탈것을 눌렀음 (왼쪽 버튼) — 부른 창의 라벨로 어느 탈것인지 찾음
#[tauri::command]
pub fn vehicle_down(window: WebviewWindow, state: State<AppState>) {
    if is_vehicle_label(window.label()) {
        state.send(Input::Vehicle(VehicleInput::Down { label: window.label().to_string() }));
    }
}

/// 탈것 우클릭 메뉴 (async — 모달 메뉴 루프가 WebView2 IPC 콜백 안에서 돌지 않게)
#[tauri::command]
pub async fn vehicle_menu(window: tauri::Window) -> Result<(), String> {
    let label = window.label().to_string();
    if !is_vehicle_label(&label) {
        return Ok(());
    }
    let app = window.app_handle().clone();
    let build = || -> tauri::Result<Menu<Wry>> {
        let remove = MenuItem::with_id(&app, format!("{MENU_REMOVE}{label}"), "🗑️ 이것만 치우기", true, None::<&str>)?;
        let clear = MenuItem::with_id(&app, MENU_CLEAR, "🧹 모든 물건 다 치우기", true, None::<&str>)?;
        Menu::with_items(&app, &[&remove, &clear])
    };
    let menu = build().map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}
