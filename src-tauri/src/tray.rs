use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::pets::PetEntry;

pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str) -> tauri::Result<()> {
    let mut checks: Vec<(CheckMenuItem<Wry>, String)> = Vec::new();
    for p in pets {
        let item = CheckMenuItem::with_id(app, format!("pet:{}", p.id), &p.manifest.name, true, p.id == current, None::<&str>)?;
        checks.push((item, p.id.clone()));
    }
    let toggle = MenuItem::with_id(app, "toggle", "숨기기 / 보이기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;

    let menu = Menu::new(app)?;
    for (item, _) in &checks {
        menu.append(item)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&toggle)?;
    menu.append(&quit)?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("ScreenPet")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "toggle" => {
                if let Some(w) = app.get_webview_window("pet") {
                    if w.is_visible().unwrap_or(true) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                    }
                }
            }
            other => {
                if let Some(id) = other.strip_prefix("pet:") {
                    // 체크된 항목을 다시 누르면 체크가 풀리므로 항상 다시 맞춘다
                    for (item, pid) in &checks {
                        let _ = item.set_checked(pid == id);
                    }
                    crate::select_pet(app, id);
                }
            }
        })
        .build(app)?;
    Ok(())
}
