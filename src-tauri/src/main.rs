#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pets;

#[tauri::command]
fn current_pet() -> String {
    pets::load_pets().first().map(|p| p.id.clone()).unwrap_or_default()
}

#[tauri::command]
fn pointer_down() {}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![current_pet, pointer_down])
        .run(tauri::generate_context!())
        .expect("error while running ScreenPet");
}
