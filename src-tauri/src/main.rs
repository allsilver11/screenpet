#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game_loop;
mod house;
mod network;
mod pets;
mod slots;
mod tray;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use game_loop::Input;
use house::HouseState;
use network::{NetInput, PairView};
use pets::PetEntry;
use screenpet_core::care::Food;
use screenpet_core::config;
use slots::{Role, SlotInfo, OWN_LABEL};
use tauri::{AppHandle, Manager, State, WebviewWindow};

/// 설정이 없을 때 처음 데려오는 동물
const DEFAULT_PET: &str = "cat";
/// 없어진 동물 → 대신 데려올 동물 (저장된 선택과 돌보기 상태를 옮긴다)
const RENAMED_PETS: [(&str, &str); 1] = [("beaver", "bear")];

pub struct AppState {
    pub tx: Mutex<Sender<Input>>,
    pub pets: Vec<PetEntry>,
    pub config_dir: PathBuf,
    /// 창 라벨별 동물과 크기 — 게임 루프가 단계 변경·손님 생성/제거 때 갱신 (웹뷰 첫 로드용 `slot_info`)
    pub slots: Mutex<HashMap<String, SlotInfo>>,
    /// 트레이와 우클릭에 같이 쓰는 메뉴
    pub menu: tray::TrayMenu,
    /// 하우스 창에 보이는 상태 — 게임 루프가 갱신 (하우스 웹뷰 첫 로드용)
    pub house: Mutex<HouseState>,
    /// 동물이 하우스 안에 있음 — 게임 루프가 갱신 (트레이 숨기기/보이기가 어느 창을 다룰지)
    pub housed: AtomicBool,
    /// 내 동물이 친구 화면에 가 있음 — 게임 루프가 갱신 (그동안 펫 창은 숨긴 채로 둔다)
    pub away: AtomicBool,
    /// 짝맞추기 창에 보이는 상태 — 게임 루프가 갱신 (짝맞추기 웹뷰 첫 로드용)
    pub pair: Mutex<PairView>,
}

/// 창의 동물을 눌렀다 — 부른 창의 라벨로 어느 동물인지 찾는다
#[tauri::command]
fn pointer_down(window: WebviewWindow, state: State<AppState>) {
    let _ = state.tx.lock().unwrap().send(Input::PointerDown { label: window.label().to_string() });
}

/// 부른 창(또는 label 창)의 동물·단계·크기·역할·지금 모습 (웹뷰 첫 로드용 — 첫 pet-render는 웹뷰가 듣기 전에 지나갔을 수 있다).
/// 하우스 창은 label "pet"으로 내 동물의 지금 모습을 묻는다
#[tauri::command]
fn slot_info(window: WebviewWindow, state: State<AppState>, label: Option<String>) -> Result<SlotInfo, String> {
    let label = label.unwrap_or_else(|| window.label().to_string());
    state.slots.lock().unwrap().get(&label).cloned().ok_or_else(|| format!("no pet for window {label}"))
}

#[tauri::command]
fn house_state(state: State<AppState>) -> HouseState {
    state.house.lock().unwrap().clone()
}

/// 하우스 창을 눌렀다 — 배고프면 음식 고르기, 아니면 문간에서 손 흔들기
#[tauri::command]
fn house_click(state: State<AppState>) {
    let _ = state.tx.lock().unwrap().send(Input::HouseClick);
}

/// 음식 고르기 버튼 — 내 동물(펫 창·하우스 창)만 먹는다. 손님은 밥을 먹지 않는다
#[tauri::command]
fn feed(window: WebviewWindow, state: State<AppState>, food: String) {
    if slots::is_guest_label(window.label()) {
        return;
    }
    let food = match food.as_str() {
        "snack" => Food::Snack,
        "meal" => Food::Meal,
        "favorite" => Food::Favorite,
        _ => return,
    };
    let _ = state.tx.lock().unwrap().send(Input::Feed(food));
}

/// 펫을 우클릭하면 트레이와 같은 메뉴를, 손님을 우클릭하면 "<주인>에게 돌려보내기"만 커서 위치에 띄운다.
/// async라 IPC 콜백 밖(작업 스레드)에서 돌고, popup_menu는 메인 스레드에 맡겨진다 —
/// 동기 커맨드면 모달 메뉴 루프가 WebView2 IPC 콜백 안에서 돌아 멈출 수 있다.
#[tauri::command]
async fn show_menu(window: tauri::Window, state: State<'_, AppState>) -> Result<(), String> {
    let label = window.label().to_string();
    if !slots::is_guest_label(&label) {
        return window.popup_menu(&state.menu.menu).map_err(|e| e.to_string());
    }
    let owner = state.slots.lock().unwrap().get(&label).and_then(|i| i.owner.clone());
    let Some(owner) = owner else { return Ok(()) };
    let menu = tray::guest_menu(window.app_handle(), &label, &owner).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

/// 짝맞추기 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `pair-state` 이벤트)
#[tauri::command]
fn pair_state(state: State<AppState>) -> PairView {
    state.pair.lock().unwrap().clone()
}

/// 짝맞추기 창 버튼: true = 똑같아요, 친구 맺기 / false = 달라요·취소.
/// 어느 짝맞추기에 답하는지는 게임 루프가 안다 (창은 지금 보이는 것에만 답한다)
#[tauri::command]
fn pair_answer(window: WebviewWindow, state: State<AppState>, accept: bool) {
    if window.label() != network::PAIR_LABEL {
        return;
    }
    let _ = state.tx.lock().unwrap().send(Input::Net(NetInput::PairAnswer(accept)));
}

/// 트레이에서 동물을 바꿨을 때. 저장·크기·웹뷰 시트 교체는 게임 루프가 한다 (설정은 루프만 쓴다).
pub fn select_pet(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let Some(entry) = state.pets.iter().find(|p| p.id == id) else { return };
    let _ = state.tx.lock().unwrap().send(Input::SetPet { entry: entry.clone() });
}

fn main() {
    let pets = pets::load_pets();
    assert!(!pets.is_empty(), "no valid pets bundled");

    tauri::Builder::default()
        // 가장 먼저 등록: 두 번째 실행은 새 앱을 띄우지 않고 이미 있는 펫 창을 보여 준다
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 숨겨 둔 창을 모두 다시 보인다 (하우스 안이면 하우스를 구석에, 친구 화면에 가 있으면 손님만)
            tray::show_all(app);
            let state = app.state::<AppState>();
            if !state.housed.load(Ordering::Relaxed) && !state.away.load(Ordering::Relaxed) {
                if let Some(window) = app.get_webview_window(OWN_LABEL) {
                    let _ = window.set_focus();
                }
            }
        }))
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let mut cfg = config::load(&config_dir);
            for (from, to) in RENAMED_PETS {
                cfg.rename_pet(from, to);
            }
            let fallback = if pets.iter().any(|p| p.id == DEFAULT_PET) { DEFAULT_PET.to_string() } else { pets[0].id.clone() };
            let current = cfg.pet.clone().filter(|id| pets.iter().any(|p| &p.id == id)).unwrap_or(fallback);
            let entry = pets.iter().find(|p| p.id == current).unwrap().clone();
            let care = cfg.care.get(&current).cloned().unwrap_or_default();

            let (tx, rx) = mpsc::channel();
            let menu = tray::build(app.handle(), &pets, &current, &game_loop::favorite_of(&entry), care.locked, cfg.housed)?;
            let stage = game_loop::stage_payload(&entry, &care);
            let house_state = HouseState {
                pet: stage.pet.clone(),
                stage: stage.stage,
                scale: stage.scale,
                hungry: care.is_hungry(),
                housed: cfg.housed,
            };
            app.manage(AppState {
                tx: Mutex::new(tx),
                pets: pets.clone(),
                config_dir,
                slots: Mutex::new(HashMap::from([(OWN_LABEL.to_string(), SlotInfo::new(&stage, &Role::Own, None))])),
                menu,
                house: Mutex::new(house_state),
                housed: AtomicBool::new(cfg.housed),
                away: AtomicBool::new(false),
                pair: Mutex::new(PairView::default()),
            });
            // 숨긴 채로 만들어 두고, 하우스에 들어갈 때 구석에 놓아 보인다
            house::create(app.handle())?;
            game_loop::spawn(app.handle().clone(), rx, entry, cfg);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![pointer_down, slot_info, feed, show_menu, house_state, house_click, pair_state, pair_answer])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
