#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod game_loop;
mod pets;
mod tray;

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use game_loop::Input;
use pets::PetEntry;
use screenpet_core::behavior::Special;
use screenpet_core::config::{self, Config};
use tauri::{AppHandle, Emitter, Manager, State};

pub struct AppState {
    pub tx: Mutex<Sender<Input>>,
    pub current: Mutex<String>,
    pub pets: Vec<PetEntry>,
    pub config_dir: PathBuf,
}

#[tauri::command]
fn pointer_down(state: State<AppState>) {
    let _ = state.tx.lock().unwrap().send(Input::PointerDown);
}

#[tauri::command]
fn current_pet(state: State<AppState>) -> String {
    state.current.lock().unwrap().clone()
}

/// 트레이에서 동물을 바꿨을 때: 저장 → 걷기 속도 반영 → 웹뷰에 새 시트 로드 요청
pub fn select_pet(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let Some(entry) = state.pets.iter().find(|p| p.id == id) else { return };
    *state.current.lock().unwrap() = id.to_string();
    if let Err(e) = config::save(&state.config_dir, &Config { pet: Some(id.to_string()) }) {
        eprintln!("failed to save config: {e}");
    }
    let _ = state.tx.lock().unwrap().send(Input::SetPet {
        walk_speed: entry.manifest.walk_speed,
        special: Special::from_manifest(|n| entry.manifest.has_anim(n)),
    });
    let _ = app.emit("pet-change", id.to_string());
}

fn main() {
    let pets = pets::load_pets();
    assert!(!pets.is_empty(), "no valid pets bundled");

    tauri::Builder::default()
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let saved = config::load(&config_dir).pet;
            let current = saved
                .filter(|id| pets.iter().any(|p| &p.id == id))
                .unwrap_or_else(|| pets[0].id.clone());
            let entry = pets.iter().find(|p| p.id == current).unwrap();
            let walk_speed = entry.manifest.walk_speed;
            let special = Special::from_manifest(|n| entry.manifest.has_anim(n));

            let (tx, rx) = mpsc::channel();
            tray::build(app.handle(), &pets, &current)?;
            app.manage(AppState { tx: Mutex::new(tx), current: Mutex::new(current), pets: pets.clone(), config_dir });
            game_loop::spawn(app.handle().clone(), rx, walk_speed, special);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![pointer_down, current_pet])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
