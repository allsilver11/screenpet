use screenpet_core::care::Food;
use screenpet_core::manifest::FavoriteFood;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::game_loop::Input;
use crate::pets::PetEntry;
use crate::AppState;

/// 트레이와 우클릭에 같이 쓰는 메뉴, 그리고 동물에 따라 바뀌는 항목
#[derive(Clone)]
pub struct TrayMenu {
    pub menu: Menu<Wry>,
    pub favorite: MenuItem<Wry>,
    pub lock: CheckMenuItem<Wry>,
}

pub fn favorite_label(f: &FavoriteFood) -> String {
    format!("{} {}", f.name, f.emoji)
}

fn send(app: &AppHandle, input: Input) {
    let _ = app.state::<AppState>().tx.lock().unwrap().send(input);
}

pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str, favorite: &FavoriteFood, locked: bool) -> tauri::Result<TrayMenu> {
    let mut checks: Vec<(CheckMenuItem<Wry>, String)> = Vec::new();
    for p in pets {
        let item = CheckMenuItem::with_id(app, format!("pet:{}", p.id), &p.manifest.name, true, p.id == current, None::<&str>)?;
        checks.push((item, p.id.clone()));
    }
    let snack = MenuItem::with_id(app, "feed:snack", "간식 🍪", true, None::<&str>)?;
    let meal = MenuItem::with_id(app, "feed:meal", "밥 🍚", true, None::<&str>)?;
    let fav = MenuItem::with_id(app, "feed:favorite", favorite_label(favorite), true, None::<&str>)?;
    let feed = Submenu::with_items(app, "밥 주기", true, &[&snack, &meal, &fav])?;
    let lock = CheckMenuItem::with_id(app, "lock", "성장 멈추기", true, locked, None::<&str>)?;
    let reset_item = MenuItem::with_id(app, "reset", "정말 처음부터 키울까요? (새끼로 돌아가요)", true, None::<&str>)?;
    let reset = Submenu::with_items(app, "처음부터 다시 키우기", true, &[&reset_item])?;
    let toggle = MenuItem::with_id(app, "toggle", "숨기기 / 보이기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;

    let menu = Menu::new(app)?;
    for (item, _) in &checks {
        menu.append(item)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&feed)?;
    menu.append(&lock)?;
    menu.append(&reset)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&toggle)?;
    menu.append(&quit)?;

    // 트레이의 메뉴 이벤트 처리기는 앱 전체용이라 우클릭(popup_menu)으로 띄운 같은 메뉴도 여기로 온다
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().cloned().expect("bundle icon"))
        .tooltip("ScreenPet")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            // 게임 루프가 돌보기 상태를 저장한 뒤 종료한다 (루프가 없으면 바로 종료)
            "quit" => {
                if app.state::<AppState>().tx.lock().unwrap().send(Input::Quit).is_err() {
                    app.exit(0);
                }
            }
            "toggle" => {
                if let Some(w) = app.get_webview_window("pet") {
                    if w.is_visible().unwrap_or(true) {
                        let _ = w.hide();
                    } else {
                        let _ = w.show();
                    }
                }
            }
            "feed:snack" => send(app, Input::Feed(Food::Snack)),
            "feed:meal" => send(app, Input::Feed(Food::Meal)),
            "feed:favorite" => send(app, Input::Feed(Food::Favorite)),
            "lock" => send(app, Input::ToggleLock),
            "reset" => send(app, Input::ResetGrowth),
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
    Ok(TrayMenu { menu, favorite: fav, lock })
}
