use screenpet_core::care::Food;
use screenpet_core::manifest::FavoriteFood;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, Wry};

use crate::game_loop::Input;
use crate::pets::PetEntry;
use crate::slots::OWN_LABEL;
use crate::AppState;

/// 손님 우클릭 메뉴 항목 id 앞부분 (뒤는 손님 창 라벨)
const SEND_HOME_PREFIX: &str = "send-home:";

/// 트레이와 우클릭에 같이 쓰는 메뉴, 그리고 동물에 따라 바뀌는 항목
#[derive(Clone)]
pub struct TrayMenu {
    pub menu: Menu<Wry>,
    pub favorite: MenuItem<Wry>,
    pub lock: CheckMenuItem<Wry>,
    /// 하우스 들어가기 / 나오기 (글자가 바뀐다)
    pub house: MenuItem<Wry>,
}

/// 하우스 항목 글자: 밖에 있으면 "하우스", 하우스 안(또는 가는 중)이면 "하우스에서 나오기"
pub fn house_label(housed: bool) -> &'static str {
    if housed {
        "하우스에서 나오기"
    } else {
        "하우스"
    }
}

pub fn favorite_label(f: &FavoriteFood) -> String {
    format!("{} {}", f.name, f.emoji)
}

/// 손님을 우클릭하면 뜨는 메뉴: "<주인>에게 돌려보내기" 하나뿐 (누르면 손 흔들고 집으로).
/// 메뉴 이벤트는 앱 전체용 처리기(트레이의 on_menu_event)로 온다
pub fn guest_menu(app: &AppHandle, label: &str, owner: &str) -> tauri::Result<Menu<Wry>> {
    let item = MenuItem::with_id(app, format!("{SEND_HOME_PREFIX}{label}"), format!("{owner}에게 돌려보내기"), true, None::<&str>)?;
    Menu::with_items(app, &[&item])
}

fn send(app: &AppHandle, input: Input) {
    let _ = app.state::<AppState>().tx.lock().unwrap().send(input);
}

pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str, favorite: &FavoriteFood, locked: bool, housed: bool) -> tauri::Result<TrayMenu> {
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
    let house = MenuItem::with_id(app, "house", house_label(housed), true, None::<&str>)?;
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
    menu.append(&house)?;
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
                // 하우스 안에선 펫 창 대신 하우스 창을 숨기고 보인다 (보일 땐 지금 구석에 다시 놓는다).
                // 친구 화면에 가 있는 동안엔 펫 창이 숨어 있어야 하니 건드리지 않는다
                let state = app.state::<AppState>();
                let housed = state.housed.load(Ordering::Relaxed);
                if !housed && state.away.load(Ordering::Relaxed) {
                    return;
                }
                let label = if housed { crate::house::LABEL } else { OWN_LABEL };
                if let Some(w) = app.get_webview_window(label) {
                    if w.is_visible().unwrap_or(true) {
                        let _ = w.hide();
                    } else if housed {
                        crate::house::show_in_corner(app);
                    } else {
                        let _ = w.show();
                    }
                }
            }
            "feed:snack" => send(app, Input::Feed(Food::Snack)),
            "feed:meal" => send(app, Input::Feed(Food::Meal)),
            "feed:favorite" => send(app, Input::Feed(Food::Favorite)),
            "lock" => send(app, Input::ToggleLock),
            "house" => send(app, Input::ToggleHouse),
            "reset" => send(app, Input::ResetGrowth),
            other => {
                if let Some(label) = other.strip_prefix(SEND_HOME_PREFIX) {
                    send(app, Input::SendHome { label: label.to_string() });
                } else if let Some(id) = other.strip_prefix("pet:") {
                    // 체크된 항목을 다시 누르면 체크가 풀리므로 항상 다시 맞춘다
                    for (item, pid) in &checks {
                        let _ = item.set_checked(pid == id);
                    }
                    crate::select_pet(app, id);
                }
            }
        })
        .build(app)?;
    Ok(TrayMenu { menu, favorite: fav, lock, house })
}
