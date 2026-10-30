use screenpet_core::care::{growth_label, Care, Food};
use screenpet_core::food::FeedMode;
use screenpet_core::manifest::FavoriteFood;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use std::sync::atomic::Ordering;

use tauri::{AppHandle, Manager, WebviewWindow, Wry};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::pets::PetEntry;
use screenpet_core::visit::Side;
use crate::slots::{self, OWN_LABEL};
use crate::AppState;

/// 이스터에그 항목 (트레이·펫 우클릭 메뉴 맨 아래, 지금은 누를 수 없음). 나중에 링크를 열게 될 수 있어 id를 따로 둔다
const EASTER_EGG_LABEL: &str = "danni❤️";
const EASTER_EGG_ID: &str = "easter-egg";

/// 손님 우클릭 메뉴 항목 id 앞부분 (뒤는 손님 창 라벨)
const SEND_HOME_PREFIX: &str = "send-home:";
/// 손님 우클릭 "편지 써서 돌려보내기…" (+ 손님 창 label)
const REPLY_PREFIX: &str = "reply:";
/// 손님 우클릭 "<친구>에게 보내기" (뒤는 "<손님 창 라벨>|<친구 id>" — 라벨에는 |가 없다)
const SEND_ON_PREFIX: &str = "send-on:";
/// `친구랑 놀기` 항목 id 앞부분 (뒤는 상대 id)
const INVITE_PREFIX: &str = "net:invite:";
const LEFT_PREFIX: &str = "net:left:";
const RIGHT_PREFIX: &str = "net:right:";
const UNFRIEND_PREFIX: &str = "net:unfriend:";
const REPAIR_PREFIX: &str = "net:repair:";
const NICKNAME_PREFIX: &str = "net:nickname:";
const MY_NAME_ID: &str = "net:my-name";
const SEND_PREFIX: &str = "net:send:";
const BRING_PREFIX: &str = "net:bring:";
/// 친구 하위 메뉴 "편지 들려 보내기…" (뒤는 친구 id)
const LETTER_PREFIX: &str = "net:letter:";
/// "놀러 보내기 ▸" 목록 항목 (친구 하위 메뉴의 항목과 id가 겹치면 안 된다)
const SEND_LIST_PREFIX: &str = "net:send-list:";
const COME_HOME_ID: &str = "net:come-home";
/// 🎮 조작 모드 체크 항목
const CONTROL_ID: &str = "control";
const CONTROL_LABEL: &str = "🎮 조작 모드 (방향키·Space·Shift, Esc로 끝)";
/// 조작 모드를 켤 때 내 동물 머리 위에 잠깐 (키가 다른 창에도 들어간다는 것도)
pub const CONTROL_HINT: &str = "🎮 ←→ 걷기 · Shift 달리기 · Space 점프 · ↓ 앉기 (방향키가 다른 창에도 입력돼요)";
/// macOS: 키가 늘 안 눌림으로 읽힐 때 한 번
pub const KEY_PERMISSION_HINT: &str = "키 입력 권한이 필요해요 (시스템 설정 → 개인정보 보호 → 입력 모니터링)";
const SHOW_CODE_ID: &str = "net:show-code";
const ENTER_CODE_ID: &str = "net:enter-code";

/// `친구랑 놀기 ▸`에 보일 것 (게임 루프의 네트워크 상태에서 만든다 — 바뀔 때만 다시 그린다)
#[derive(Debug, Clone, PartialEq)]
pub struct FriendsMenu {
    /// 네트워크가 시작됨
    pub available: bool,
    /// 친구들에게 보이는 내 이름 ("내 이름: … ✏️")
    pub my_name: String,
    /// 발견된 사람 중 친구가 아닌 사람 (id, 이름)
    pub strangers: Vec<(String, String)>,
    pub friends: Vec<FriendItem>,
    /// 내 동물이 친구 화면에 가 있음 ("집에 와" 활성)
    pub away: bool,
    /// 인터넷 친구(중계 서버): None = 쓰지 않음, Some(연결됨?)
    pub relay: Option<bool>,
    /// 손님을 더 받을 자리가 있음 ("펫 데리고 오기" 활성)
    pub room: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FriendItem {
    pub id: String,
    /// 보일 이름 (내가 붙인 별명, 없으면 친구가 정한 이름)
    pub name: String,
    pub side: Side,
    pub connected: bool,
    /// 지금 내 동물을 이 친구에게 보낼 수 있음 (연결됨 + 내 동물이 놀러 가 있지 않음)
    pub can_send: bool,
    /// 상대가 나를 더 이상 친구로 알아보지 못함
    pub forgot: bool,
}

/// 트레이와 우클릭에 같이 쓰는 메뉴, 그리고 동물에 따라 바뀌는 항목
#[derive(Clone)]
pub struct TrayMenu {
    pub menu: Menu<Wry>,
    /// 좋아하는 음식 항목 (손으로 주기 ▸ / 그릇에 주기 ▸ 두 군데 — 동물이 바뀌면 둘 다 글자를 바꾼다)
    pub favorite: [MenuItem<Wry>; 2],
    /// 성장 단계 한 줄 (누를 수 없음, 단계·성장 멈춤이 바뀌면 글자만 바꾼다)
    pub growth: MenuItem<Wry>,
    pub lock: CheckMenuItem<Wry>,
    /// 하우스 들어가기 / 나오기 (글자가 바뀐다)
    pub house: MenuItem<Wry>,
    /// 🎮 조작 모드 (게임 루프가 실제 상태로 다시 맞춘다 — Esc·3분 무입력으로 끝나면 체크가 풀린다)
    pub control: CheckMenuItem<Wry>,
    /// 동물 고르기 항목 (항목, 동물 id) — `캐릭터: <지금 동물> ▸` 하위 메뉴 안
    pub pets: Vec<(CheckMenuItem<Wry>, String)>,
    pub pets_menu: Submenu<Wry>,
    /// `친구랑 놀기 ▸` (네트워크 상태에 따라 다시 채운다)
    pub friends: Submenu<Wry>,
    /// 집에 와 (내 동물이 친구 화면에 가 있을 때만 누를 수 있다 — 하우스 옆 바깥 메뉴)
    pub come_home: MenuItem<Wry>,
    /// 밥 주기 방법별 하위 메뉴: 손으로 주기 ▸ / 그릇에 주기 ▸ (마지막에 쓴 쪽에 ✓ — 배고플 때 고르기도 그 방법을 따른다)
    pub hand_mode: Submenu<Wry>,
    pub auto_mode: Submenu<Wry>,
}

/// 밥 주기 방법 체크를 지금 설정에 맞춘다 (체크 항목은 누를 때 스스로 뒤집히므로 늘 다시 맞춘다)
pub fn sync_feed_mode(menu: &TrayMenu, mode: FeedMode) {
    let _ = menu.hand_mode.set_text(feed_mode_label(FeedMode::Hand, mode));
    let _ = menu.auto_mode.set_text(feed_mode_label(FeedMode::Auto, mode));
}

fn feed_mode_label(which: FeedMode, current: FeedMode) -> String {
    let name = match which {
        FeedMode::Hand => "✋ 손으로 주기",
        FeedMode::Auto => "🥣 그릇에 주기",
    };
    if which == current {
        format!("{name} ✓")
    } else {
        name.to_string()
    }
}

/// 좋아하는 음식 두 항목의 글자를 바꾼다 (동물 교체)
pub fn set_favorite_label(menu: &TrayMenu, f: &FavoriteFood) {
    for item in &menu.favorite {
        let _ = item.set_text(favorite_label(f));
    }
}

/// 다른 사람이 정한 이름을 메뉴 글자로: 제어 문자(\t는 Windows에서 단축키 칸, \n 등)를 빼고,
/// &는 Windows에서 밑줄 단축키 표시라 두 번 쓴다
fn menu_text(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().replace('&', "&&")
}

/// `친구랑 놀기 ▸`을 다시 채운다: 내 이름, 친구별 하위 메뉴, 발견된 사람(누르면 친구 신청), 인터넷 친구 (+ 바깥 "집에 와" 켜고 끄기)
pub fn fill_friends(app: &AppHandle, model: &FriendsMenu) {
    let submenu = &app.state::<AppState>().menu.friends;
    if let Err(e) = try_fill_friends(app, submenu, model) {
        eprintln!("failed to update friends menu: {e}");
    }
}

fn try_fill_friends(app: &AppHandle, submenu: &Submenu<Wry>, model: &FriendsMenu) -> tauri::Result<()> {
    while submenu.remove_at(0)?.is_some() {}
    if !model.available {
        submenu.append(&MenuItem::new(app, "친구 찾기를 쓸 수 없어요", false, None::<&str>)?)?;
        return Ok(());
    }
    // 친구들에게 보이는 내 이름 (누르면 바꾸기 창)
    submenu.append(&MenuItem::with_id(app, MY_NAME_ID, format!("내 이름: {} ✏️", menu_text(&model.my_name)), true, None::<&str>)?)?;
    // 놀러 보내기 ▸ 연결된 친구 (펫 우클릭에서도 같은 메뉴)
    let online: Vec<&FriendItem> = model.friends.iter().filter(|f| f.connected).collect();
    if !online.is_empty() {
        let send = Submenu::new(app, "놀러 보내기", online.iter().any(|f| f.can_send))?;
        for f in online {
            send.append(&MenuItem::with_id(app, format!("{SEND_LIST_PREFIX}{}", f.id), menu_text(&f.name), f.can_send, None::<&str>)?)?;
        }
        submenu.append(&send)?;
    }
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    for f in &model.friends {
        let dot = if f.connected { "●" } else { "○" };
        let mut title = format!("{dot} {}", menu_text(&f.name));
        if f.forgot {
            title.push_str(" (다시 친구 맺기 필요)");
        }
        let left = CheckMenuItem::with_id(app, format!("{LEFT_PREFIX}{}", f.id), "왼쪽에", true, f.side == Side::Left, None::<&str>)?;
        let right = CheckMenuItem::with_id(app, format!("{RIGHT_PREFIX}{}", f.id), "오른쪽에", true, f.side == Side::Right, None::<&str>)?;
        let send = MenuItem::with_id(app, format!("{SEND_PREFIX}{}", f.id), "놀러 보내기", f.can_send, None::<&str>)?;
        // 편지를 실어 보내기: "놀러 보내기"와 같은 때만 누를 수 있다
        let letter = MenuItem::with_id(app, format!("{LETTER_PREFIX}{}", f.id), "편지 들려 보내기…", f.can_send, None::<&str>)?;
        let bring_label = format!("{} 펫 데리고 오기", menu_text(&f.name));
        let bring = MenuItem::with_id(app, format!("{BRING_PREFIX}{}", f.id), bring_label, f.connected && model.room, None::<&str>)?;
        let nickname = MenuItem::with_id(app, format!("{NICKNAME_PREFIX}{}", f.id), "이름 바꾸기…", true, None::<&str>)?;
        let unfriend = MenuItem::with_id(app, format!("{UNFRIEND_PREFIX}{}", f.id), "연결 끊기", true, None::<&str>)?;
        let sep = PredefinedMenuItem::separator(app)?;
        let sep2 = PredefinedMenuItem::separator(app)?;
        let sub = Submenu::with_items(app, title, true, &[&send, &letter, &bring, &sep, &left, &right, &sep2, &nickname, &unfriend])?;
        if f.forgot {
            sub.append(&MenuItem::with_id(app, format!("{REPAIR_PREFIX}{}", f.id), "다시 친구 맺기", true, None::<&str>)?)?;
        }
        submenu.append(&sub)?;
    }
    if !model.friends.is_empty() && !model.strangers.is_empty() {
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    for (id, name) in &model.strangers {
        submenu.append(&MenuItem::with_id(app, format!("{INVITE_PREFIX}{id}"), format!("{} — 친구 신청", menu_text(name)), true, None::<&str>)?)?;
    }
    if model.friends.is_empty() && model.strangers.is_empty() {
        submenu.append(&MenuItem::new(app, "같은 네트워크에 ScreenPet이 없어요", false, None::<&str>)?)?;
    }
    // 인터넷 친구 (중계 서버): 상태 한 줄, 내 코드 보기, 코드 입력
    submenu.append(&PredefinedMenuItem::separator(app)?)?;
    let status = match model.relay {
        Some(true) => "인터넷 연결: 연결됨",
        Some(false) => "인터넷 연결: 끊김",
        None => "인터넷 연결: 꺼짐",
    };
    submenu.append(&MenuItem::new(app, status, false, None::<&str>)?)?;
    let relay_on = model.relay.is_some();
    submenu.append(&MenuItem::with_id(app, SHOW_CODE_ID, "내 친구 코드 보기", relay_on, None::<&str>)?)?;
    submenu.append(&MenuItem::with_id(app, ENTER_CODE_ID, "친구 코드 입력…", relay_on, None::<&str>)?)?;
    app.state::<AppState>().menu.come_home.set_enabled(model.away)?;
    Ok(())
}

/// 내 동물이 친구 화면에 가 있거나 가는 중이면 하우스·동물 바꾸기를 막는다
pub fn set_own_busy(menu: &TrayMenu, busy: bool) {
    let _ = menu.house.set_enabled(!busy);
    for (item, _) in &menu.pets {
        let _ = item.set_enabled(!busy);
    }
}

/// 동물 고르기 체크를 지금 동물에 맞춘다 (막힌 동안 눌렀을 때 되돌리기)
pub fn sync_pet_checks(menu: &TrayMenu, current: &str) {
    for (item, id) in &menu.pets {
        let _ = item.set_checked(id == current);
    }
    set_pets_label(&menu.pets_menu, &menu.pets, current);
}

/// `캐릭터 ▸` 하위 메뉴 글자: 지금 동물 이름을 붙인다
fn pets_label(name: Option<&str>) -> String {
    match name {
        Some(n) => format!("캐릭터: {}", menu_text(n)),
        None => "캐릭터".to_string(),
    }
}

fn set_pets_label(sub: &Submenu<Wry>, pets: &[(CheckMenuItem<Wry>, String)], current: &str) {
    let name = pets.iter().find(|(_, id)| id == current).and_then(|(item, _)| item.text().ok());
    let _ = sub.set_text(pets_label(name.as_deref()));
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

/// 손님을 우클릭하면 뜨는 메뉴: "<주인>에게 돌려보내기" (누르면 손 흔들고 집으로), "편지 써서 돌려보내기…" (답장 창), 그리고 연결된 내 친구마다
/// "<친구>에게 보내기" (targets = 주인을 뺀 (id, 이름)). 메뉴 이벤트는 앱 전체용 처리기(트레이의 on_menu_event)로 온다
pub fn guest_menu(app: &AppHandle, label: &str, owner: &str, targets: &[(String, String)]) -> tauri::Result<Menu<Wry>> {
    let item = MenuItem::with_id(app, format!("{SEND_HOME_PREFIX}{label}"), format!("{}에게 돌려보내기", menu_text(owner)), true, None::<&str>)?;
    let reply = MenuItem::with_id(app, format!("{REPLY_PREFIX}{label}"), "편지 써서 돌려보내기…", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&item, &reply])?;
    if !targets.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    for (id, name) in targets {
        menu.append(&MenuItem::with_id(app, format!("{SEND_ON_PREFIX}{label}|{id}"), format!("{}에게 보내기", menu_text(name)), true, None::<&str>)?)?;
    }
    Ok(menu)
}

/// 내 동물이 보이는 창: 하우스 안이면 하우스 창, 친구 화면에 가 있으면 없음 (펫 창은 숨어 있어야 한다)
fn own_window(app: &AppHandle) -> Option<WebviewWindow> {
    let state = app.state::<AppState>();
    if state.housed.load(Ordering::Relaxed) {
        app.get_webview_window(crate::house::LABEL)
    } else if state.away.load(Ordering::Relaxed) {
        None
    } else {
        app.get_webview_window(OWN_LABEL)
    }
}

fn guest_windows(app: &AppHandle) -> Vec<WebviewWindow> {
    app.webview_windows().into_iter().filter(|(label, _)| slots::is_guest_label(label)).map(|(_, w)| w).collect()
}

/// 밥그릇·꽃·쿠키(prop-), 손에 든 음식(hand-), 침 자국(sting-) 창
fn effect_windows(app: &AppHandle) -> Vec<WebviewWindow> {
    const PREFIXES: [&str; 3] = ["prop-", "hand-", "sting-"];
    app.webview_windows().into_iter().filter(|(label, _)| PREFIXES.iter().any(|p| label.starts_with(p))).map(|(_, w)| w).collect()
}

/// 내 동물 창(또는 하우스 창)과 손님 창을 모두 보인다. 하우스는 지금 구석에 다시 놓는다
pub fn show_all(app: &AppHandle) {
    app.state::<AppState>().hidden.store(false, Ordering::Relaxed);
    if app.state::<AppState>().housed.load(Ordering::Relaxed) {
        crate::house::show_in_corner(app);
    } else if let Some(w) = own_window(app) {
        let _ = w.show();
    }
    for w in guest_windows(app).into_iter().chain(effect_windows(app)) {
        let _ = w.show();
    }
}

/// 숨기기 / 보이기: 하나라도 보이면 모두 숨기고, 다 숨어 있으면 모두 보인다 (손님 창도 같이)
fn toggle_windows(app: &AppHandle) {
    let windows: Vec<WebviewWindow> = own_window(app).into_iter().chain(guest_windows(app)).collect();
    if windows.iter().any(|w| w.is_visible().unwrap_or(true)) {
        // 숨겨 둔 동안 게임 루프가 다시 보이지 않게 먼저 적는다
        app.state::<AppState>().hidden.store(true, Ordering::Relaxed);
        for w in windows.iter().chain(effect_windows(app).iter()) {
            let _ = w.hide();
        }
    } else {
        show_all(app);
    }
}

fn send(app: &AppHandle, input: Input) {
    let _ = app.state::<AppState>().tx.lock().unwrap().send(input);
}

#[allow(clippy::too_many_arguments)]
pub fn build(app: &AppHandle, pets: &[PetEntry], current: &str, favorite: &FavoriteFood, care: &Care, housed: bool, feed_mode: FeedMode) -> tauri::Result<TrayMenu> {
    let mut checks: Vec<(CheckMenuItem<Wry>, String)> = Vec::new();
    let pets_menu = Submenu::new(app, pets_label(pets.iter().find(|p| p.id == current).map(|p| p.manifest.name.as_str())), true)?;
    for p in pets {
        let item = CheckMenuItem::with_id(app, format!("pet:{}", p.id), &p.manifest.name, true, p.id == current, None::<&str>)?;
        pets_menu.append(&item)?;
        checks.push((item, p.id.clone()));
    }
    // 밥·좋아하는 음식만 자라게 한다. 간식·물은 언제든 (배불러도) 받는다
    // 방법을 고르고 음식을 고르는 걸 한 번에 (메뉴는 누르면 닫히므로): 손으로 주기 ▸ 음식 / 그릇에 주기 ▸ 음식.
    // 손으로: 음식이 커서 끝에 붙어 동물 입에 갖다 대면 먹는다. 그릇에: 그릇(꿀벌은 꽃)이 놓인다. 물은 늘 물그릇
    let mut favs = Vec::new();
    let mut by_mode = Vec::new();
    for (mode, tag) in [(FeedMode::Hand, "hand"), (FeedMode::Auto, "auto")] {
        let meal = MenuItem::with_id(app, format!("feed:{tag}:meal"), "밥 🍚", true, None::<&str>)?;
        let fav = MenuItem::with_id(app, format!("feed:{tag}:favorite"), favorite_label(favorite), true, None::<&str>)?;
        let snack = MenuItem::with_id(app, format!("feed:{tag}:snack"), "간식 🍪", true, None::<&str>)?;
        by_mode.push(Submenu::with_items(app, feed_mode_label(mode, feed_mode), true, &[&meal, &fav, &snack])?);
        favs.push(fav);
    }
    let water = MenuItem::with_id(app, "feed:water", "물 💧", true, None::<&str>)?;
    let auto_mode = by_mode.pop().expect("auto submenu");
    let hand_mode = by_mode.pop().expect("hand submenu");
    let fav_auto = favs.pop().expect("auto favorite");
    let fav_hand = favs.pop().expect("hand favorite");
    let feed = Submenu::with_items(app, "밥 주기", true, &[&hand_mode, &auto_mode, &PredefinedMenuItem::separator(app)?, &water])?;
    let growth = MenuItem::new(app, growth_label(care.stage, care.locked), false, None::<&str>)?;
    let lock = CheckMenuItem::with_id(app, "lock", "성장 멈추기", true, care.locked, None::<&str>)?;
    let reset_item = MenuItem::with_id(app, "reset", "정말 처음부터 키울까요? (새끼로 돌아가요)", true, None::<&str>)?;
    let reset = Submenu::with_items(app, "처음부터 다시 키우기", true, &[&reset_item])?;
    let house = MenuItem::with_id(app, "house", house_label(housed), true, None::<&str>)?;
    // 방향키로 내 동물 움직이기 (친구 화면에 가 있어도). 키는 읽기만 해서 앞에 있는 창에도 그대로 입력된다 — 켤 때 말풍선으로 알린다
    let control = CheckMenuItem::with_id(app, CONTROL_ID, CONTROL_LABEL, true, false, None::<&str>)?;
    let come_home = MenuItem::with_id(app, COME_HOME_ID, "집에 와", false, None::<&str>)?;
    // 게임 루프가 네트워크를 시작한 뒤 채운다
    let friends = Submenu::with_items(app, "친구랑 놀기", true, &[&MenuItem::new(app, "친구를 찾는 중…", false, None::<&str>)?])?;
    let toggle = MenuItem::with_id(app, "toggle", "숨기기 / 보이기", true, None::<&str>)?;
    let help = MenuItem::with_id(app, "help", "도움말 ❓", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let easter_egg = MenuItem::with_id(app, EASTER_EGG_ID, EASTER_EGG_LABEL, false, None::<&str>)?;

    let menu = Menu::new(app)?;
    // 트레이와 펫 우클릭은 같은 메뉴 — 둘 다 맨 위에 보인다
    menu.append(&easter_egg)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&pets_menu)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&growth)?;
    menu.append(&feed)?;
    menu.append(&lock)?;
    menu.append(&reset)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&house)?;
    menu.append(&come_home)?;
    menu.append(&control)?;
    menu.append(&friends)?;
    menu.append(&toggle)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&help)?;
    menu.append(&quit)?;

    let pet_items = checks.clone();
    let pets_menu_handle = pets_menu.clone();
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
            "toggle" => toggle_windows(app),
            "help" => crate::help::show(app),
            "feed:water" => send(app, Input::Feed(Food::Water)),
            "lock" => send(app, Input::ToggleLock),
            "house" => send(app, Input::ToggleHouse),
            CONTROL_ID => send(app, Input::ToggleControl),
            "reset" => send(app, Input::ResetGrowth),
            other => {
                let net = |input| send(app, Input::Net(input));
                if let Some(label) = other.strip_prefix(SEND_HOME_PREFIX) {
                    send(app, Input::SendHome { label: label.to_string() });
                } else if let Some(label) = other.strip_prefix(REPLY_PREFIX) {
                    net(NetInput::EditReply(label.to_string()));
                } else if let Some((label, friend)) = other.strip_prefix(SEND_ON_PREFIX).and_then(|r| r.split_once('|')) {
                    net(NetInput::SendOn { label: label.to_string(), friend: friend.to_string() });
                } else if other == COME_HOME_ID {
                    net(NetInput::ComeHome);
                } else if other == SHOW_CODE_ID {
                    net(NetInput::ShowCode);
                } else if other == ENTER_CODE_ID {
                    net(NetInput::EnterCode);
                } else if other == MY_NAME_ID {
                    net(NetInput::EditMyName);
                } else if let Some(id) = other.strip_prefix(SEND_PREFIX).or_else(|| other.strip_prefix(SEND_LIST_PREFIX)) {
                    net(NetInput::SendTo(id.to_string()));
                } else if let Some(id) = other.strip_prefix(LETTER_PREFIX) {
                    net(NetInput::EditLetter(id.to_string()));
                } else if let Some(id) = other.strip_prefix(BRING_PREFIX) {
                    net(NetInput::BringPet(id.to_string()));
                } else if let Some(id) = other.strip_prefix(NICKNAME_PREFIX) {
                    net(NetInput::EditNickname(id.to_string()));
                } else if let Some(id) = other.strip_prefix(INVITE_PREFIX) {
                    net(NetInput::Invite(id.to_string()));
                } else if let Some(id) = other.strip_prefix(LEFT_PREFIX) {
                    net(NetInput::SetSide { id: id.to_string(), side: Side::Left });
                } else if let Some(id) = other.strip_prefix(RIGHT_PREFIX) {
                    net(NetInput::SetSide { id: id.to_string(), side: Side::Right });
                } else if let Some(id) = other.strip_prefix(UNFRIEND_PREFIX) {
                    net(NetInput::Unfriend(id.to_string()));
                } else if let Some(id) = other.strip_prefix(REPAIR_PREFIX) {
                    net(NetInput::Repair(id.to_string()));
                } else if let Some((mode, food)) = other.strip_prefix("feed:").and_then(|rest| rest.split_once(':')) {
                    // 방법을 기억하고 (배고플 때 고르기도 따른다) 바로 먹인다
                    let mode = if mode == "hand" { FeedMode::Hand } else { FeedMode::Auto };
                    let food = match food {
                        "meal" => Food::Meal,
                        "favorite" => Food::Favorite,
                        _ => Food::Snack,
                    };
                    send(app, Input::SetFeedMode(mode));
                    send(app, Input::Feed(food));
                } else if let Some(id) = other.strip_prefix("pet:") {
                    // 체크된 항목을 다시 누르면 체크가 풀리므로 항상 다시 맞춘다
                    for (item, pid) in &checks {
                        let _ = item.set_checked(pid == id);
                    }
                    set_pets_label(&pets_menu_handle, &checks, id);
                    crate::select_pet(app, id);
                }
            }
        })
        .build(app)?;
    Ok(TrayMenu { menu, favorite: [fav_hand, fav_auto], growth, lock, house, control, pets: pet_items, pets_menu, friends, come_home, hand_mode, auto_mode })
}
