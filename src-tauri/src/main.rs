#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game_loop;
mod pets;
mod tray;

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use game_loop::{Input, StagePayload};
use pets::PetEntry;
use screenpet_core::care::Food;
use screenpet_core::config;
use tauri::{AppHandle, Manager, State};

/// 설정이 없을 때 처음 데려오는 동물
const DEFAULT_PET: &str = "cat";
/// 없어진 동물 → 대신 데려올 동물 (저장된 선택과 돌보기 상태를 옮긴다)
const RENAMED_PETS: [(&str, &str); 1] = [("beaver", "bear")];

pub struct AppState {
    pub tx: Mutex<Sender<Input>>,
    pub pets: Vec<PetEntry>,
    pub config_dir: PathBuf,
    /// 지금 보이는 동물과 크기 — 게임 루프가 apply_stage 때 갱신 (웹뷰 첫 로드용)
    pub stage: Mutex<StagePayload>,
    /// 트레이와 우클릭에 같이 쓰는 메뉴
    pub menu: tray::TrayMenu,
}

#[tauri::command]
fn pointer_down(state: State<AppState>) {
    let _ = state.tx.lock().unwrap().send(Input::PointerDown);
}

#[tauri::command]
fn current_stage(state: State<AppState>) -> StagePayload {
    state.stage.lock().unwrap().clone()
}

#[tauri::command]
fn feed(state: State<AppState>, food: String) {
    let food = match food.as_str() {
        "snack" => Food::Snack,
        "meal" => Food::Meal,
        "favorite" => Food::Favorite,
        _ => return,
    };
    let _ = state.tx.lock().unwrap().send(Input::Feed(food));
}

/// 펫을 우클릭하면 트레이와 같은 메뉴를 커서 위치에 띄운다
#[tauri::command]
fn show_menu(window: tauri::Window, state: State<AppState>) {
    if let Err(e) = window.popup_menu(&state.menu.menu) {
        eprintln!("failed to show menu: {e}");
    }
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
            let menu = tray::build(app.handle(), &pets, &current, &game_loop::favorite_of(&entry), care.locked)?;
            app.manage(AppState {
                tx: Mutex::new(tx),
                pets: pets.clone(),
                config_dir,
                stage: Mutex::new(game_loop::stage_payload(&entry, &care)),
                menu,
            });
            game_loop::spawn(app.handle().clone(), rx, entry, cfg);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![pointer_down, current_stage, feed, show_menu])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
