//! 게임 루프 스레드임: 16ms 틱마다 창 목록·커서를 읽고 동물 두뇌(코어)를 돌린 뒤 창을 놓음.
//! 웹뷰·트레이·네트워크에서 오는 것은 모두 [`Input`]으로 받아 이 스레드에서만 설정과 슬롯을 고침.
//! 진입점 `spawn`은 `main.rs`가 부르고, 효과들(날씨·물건·선물·사다리 등)과 `network.rs`를 틱마다 돌림.
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::brain::control_moves::ControlInput;
use screenpet_core::brain::Pet;
use screenpet_core::care::{Care, FeedResult, Food, Stage};
use screenpet_core::config::{self, Config};
use screenpet_core::broom::Broom;
use screenpet_core::food::{self, FeedMode, HandFeed, HandInput};
use screenpet_core::geom::Rect;
use screenpet_core::linked;
use screenpet_core::manifest::FavoriteFood;
use screenpet_core::style::PetStyle;
use screenpet_core::typing::{AskStep, PermissionAsk};
use screenpet_core::tint::BodyTint;
use screenpet_core::pacing::Pacer;
use screenpet_core::platform::{default_source, WindowSource};
use screenpet_core::presence::FocusLength;
use screenpet_core::social::Social;
use screenpet_core::visit::Side;
use screenpet_core::world::World;
use tauri::{AppHandle, Emitter, Manager};

use crate::gifts::Gifts;
use crate::house::{self, HouseState};
use crate::network::{NetInput, Network};
use crate::objects::{ObjectInput, Objects};
use crate::pets::PetEntry;
use crate::presence::Presence;
use crate::prop::{HandWindow, Props};
use crate::slots::{self, GuestInfo, Role, Slot, OWN_LABEL};
use crate::timeline::TimelineFx;
use crate::weather::WeatherFx;
use screenpet_core::real_weather::Reading;
use crate::AppState;

/// 한 틱 길이
const TICK: Duration = Duration::from_millis(16);
/// 전체 창 목록 다시 읽기 (EnumWindows). 사이 틱엔 동물이 기대고 있는 창만 읽음 (follow_ridden_windows)
const WORLD_REFRESH: Duration = Duration::from_millis(100);
/// 설정을 저장하는 주기 (바뀌었을 때만)
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// 한 틱에 넘기는 시간 상한 (초): 잠들었다 깬 뒤 큰 점프를 막음
const MAX_DT: f64 = 0.05;
/// 한 틱에 처리하는 입력 수의 상한 (틱 16ms면 초당 4000개, 친구 메시지 예산을 다 더한 것보다 훨씬 많음)
const MAX_INPUTS_PER_TICK: usize = 64;
/// slots[OWN] = 내 동물 (항상 맨 앞), 그 뒤는 손님
const OWN: usize = 0;
/// macOS: 조작 모드를 켠 뒤 이만큼 동안 키가 한 번도 눌린 것으로 읽히지 않으면 입력 모니터링 권한 안내 (실행마다 한 번)
const KEY_PERMISSION_HINT_AFTER: Duration = Duration::from_secs(12);
/// 🍎 ⌨️ 타자 응원 권한(macOS 입력 모니터링)이 생겼는지 보는 간격 (켜져 있고 권한이 없을 때만 일함)
const TYPING_PERMISSION_CHECK: Duration = Duration::from_secs(2);
/// 이 환경 변수가 있으면 시작할 때 번들 동물 하나를 손님으로 들임 (네트워크 없이 여러 창 확인용)
const DEBUG_GUEST_ENV: &str = "SCREENPET_DEBUG_GUEST";

/// 말풍선은 줄바꿈 없이 128px 창에 잘리니 한 줄로 짧게
const NO_PIXEL_ART_HINT: &str = "픽셀 그림 준비 중!";

/// 게임 루프로 들어오는 입력임 (웹뷰 커맨드, 트레이 메뉴, 네트워크 이벤트)
pub enum Input {
    /// 창(label)의 동물을 눌렀음
    PointerDown { label: String },
    /// 동물 교체 — 돌보기 상태는 게임 루프가 가진 설정에서 찾음.
    /// remote = 📱 노는 내 다른 컴퓨터가 바꿨음 (다시 알리지 않고, 트레이 체크를 맞춤)
    SetPet { entry: PetEntry, remote: bool },
    /// 캐릭터 스타일 (동글동글 / 픽셀, 설정에 남음) — 바로 그림이 바뀜
    SetStyle(PetStyle),
    /// 🎨 흐물이 몸 색 (설정에 남음) — 바로 칠해짐
    SetTint(BodyTint),
    Feed(Food),
    /// 밥 주기 방법 (직접 주기 / 자동 주기)
    SetFeedMode(FeedMode),
    ToggleLock,
    ResetGrowth,
    /// 하우스로 들어가기 / 나오기
    ToggleHouse,
    /// 🎮 조작 모드 켜기 / 끄기 (트레이·우클릭 체크 항목)
    ToggleControl,
    /// 🐾 커서 장난 켜기 / 끄기 (설정에 남음)
    ToggleCursorPlay,
    /// ⌨️ 타자 응원 켜기·끄기
    ToggleTypingCheer,
    /// 💩 똥 싸기 켜기 / 끄기 (설정에 남음)
    TogglePoop,
    /// 🚀 컴퓨터 켜면 자동 실행 켜기 / 끄기 (설정에 남음, 바로 OS에 적용)
    ToggleAutostart,
    /// 🧹 빗자루 들기 / 내려놓기 (트레이·우클릭 체크 항목)
    ToggleBroom,
    /// 🌨️ 날씨 효과 켜기 / 끄기 (트레이·우클릭 체크 항목)
    ToggleWeather,
    /// ☀️ 실제 날씨 / 📅 계절 날씨 (설정에 남음, 바로 바뀜)
    SetWeatherSource(screenpet_core::real_weather::WeatherSource),
    /// 📍 실제 날씨를 볼 도시 (설정에 남음 — 중계 서버에 그 도시를 달라고 함)
    SetWeatherPlace(screenpet_core::real_weather::Place),
    /// ⏱️ 집중 친구 시작 / 그만 (켜져 있는지는 설정에 남지 않음)
    ToggleFocus,
    /// ⏱️ 집중 친구 길이 (설정에 남음, 켜져 있으면 새 길이로 다시 시작)
    SetFocusLength(FocusLength),
    /// 선물 창(label)을 눌렀음 — 도감에 적음 (설정은 게임 루프만 씀)
    CollectGift { label: String },
    /// 하우스 창을 눌렀음 (배고프면 음식 고르기, 아니면 손 흔들기)
    HouseClick,
    /// 하우스를 끌어 옮겼음 (house.rs). save = 다 놓았음 — 새 자리를 설정에 적음
    HouseMoved { save: bool },
    /// 손님(label)을 주인에게 돌려보냄 (손님 우클릭 메뉴)
    SendHome { label: String },
    /// 🍚 놀러 온 친구 동물에게 밥 주기: 먹으면 일시적으로 커짐 (성장과 상관없음)
    FeedGuest { label: String },
    /// 손님 창(label)이 편지 쪽지를 펼침 / 접음 — 그동안 창이 위로 큼
    NoteSpace { label: String, open: bool },
    /// 친구 네트워크 (네트워크 이벤트, 트레이 `친구랑 놀기`, 짝맞추기 창)
    Net(NetInput),
    /// 물건 (메뉴·누르기·포스트잇)
    Object(ObjectInput),
    /// 메뉴 "도움말": 창 만들기는 메뉴 처리기 안에서 하면 Windows에서 멈춤 (Tauri 문서) — 게임 루프 스레드에서
    ShowHelp,
    /// 💌 개발자에게 건의하기 창
    ShowFeedback,
    /// 📮 개발자 건의함 창 (개발자 컴퓨터의 트레이 "📮 건의함", 편지를 든 펫 우클릭)
    ShowMailbox,
    /// 📮 건의함 보기 창 (다른 모든 컴퓨터의 트레이 "📮 건의함" — 읽기 전용 공개 목록)
    ShowBoard,
    /// 메뉴 "🎁 선물 도감" (창 만들기는 게임 루프 스레드에서)
    ShowGiftBook,
    /// 메뉴 "💩 똥피하기"
    ShowDodge,
    /// 똥피하기 창에서
    Dodge(crate::dodge::DodgeInput),
    /// 📏 크기 조절 창을 엶
    ShowSizes,
    /// 크기 조절 창의 값 (%). save = 슬라이더를 놓았음: 파일에 남김
    SetSizes { pet: u32, object: u32, save: bool },
    /// 메뉴 "🕐 최근 한 시간" (기록은 게임 루프가 가짐)
    ShowTimeline,
    /// 🔄 업데이트 확인이 새 버전을 찾았음 (update.rs) — 내 동물들이 "new version!!" 하고 뜀
    NewVersion(screenpet_core::update::Version),
    /// 저장하고 앱 종료
    Quit,
    /// 앱이 끝나는 중 (창이 닫힘·로그오프 등): 저장만 하고 알린 뒤 루프를 끝냄
    SaveAndStop(Sender<()>),
}

/// 좋아하는 음식 (매니페스트에 없으면 별)
pub fn favorite_of(entry: &PetEntry) -> FavoriteFood {
    entry.manifest.favorite_food.clone().unwrap_or_else(|| FavoriteFood { emoji: "⭐".into(), name: "좋아하는 것".into() })
}

/// 게임 루프 스레드를 띄움. 루프가 패닉하면 앱을 끝냄
pub fn spawn(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config) {
    thread::spawn(move || {
        let handle = app.clone();
        // 게임 루프가 죽으면 펫은 멈춘 채 남음 — 기록은 패닉 훅이 남겼으니 앱을 끝냄
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(app, rx, entry, config))).is_err() {
            handle.exit(1);
        }
    });
}

/// 📱 내 펫·돌보기 상태가 바뀌었음: 내 다른 컴퓨터와 나눔 (edit = 사용자가 바꿈 — 쉬는 중이면 이 컴퓨터로 놂)
fn share_care(network: &Network, config: &Config, own: &Slot, care: &Care, edit: bool) {
    network.share_state(config, &own.entry.id, care, edit);
}

/// ⌨️ 타자 응원을 받는 동물: 내 흐물이 (손님 흐물이는 아님)
fn is_own_heumul(s: &Slot) -> bool {
    s.is_own() && s.entry.id == "heumul"
}

/// 설정 저장 — 게임 루프만 설정을 씀 (단일 작성자). 하우스 여부는 config.housed에 직접 적어 둠
fn save_config(app: &AppHandle, config: &mut Config, pet_id: &str, care: &Care) {
    config.pet = Some(pet_id.to_string());
    config.care.insert(pet_id.to_string(), care.clone());
    let state = app.state::<AppState>();
    if let Err(e) = config::save(&state.config_dir, config) {
        eprintln!("failed to save config: {e}");
    }
}

/// 트레이·우클릭 메뉴의 돌보기·하우스 항목을 현재 동물과 상태에 맞춤
fn sync_menu(app: &AppHandle, entry: &PetEntry, care: &Care, housed: bool) {
    let state = app.state::<AppState>();
    crate::tray::set_favorite_label(&state.menu, &favorite_of(entry));
    let _ = state.menu.lock.set_checked(care.locked);
    sync_growth(app, care);
    let _ = state.menu.house.set_text(crate::tray::house_label(housed));
    crate::tray::sync_tint_shown(&state.menu, entry.manifest.tint);
}

/// 메뉴의 성장 단계 한 줄을 지금 돌보기 상태에 맞춤 (동물마다 단계가 따로임)
fn sync_growth(app: &AppHandle, care: &Care) {
    let _ = app.state::<AppState>().menu.growth.set_text(screenpet_core::care::growth_label(care.stage, care.locked));
}

/// 지금 내 동물의 말풍선·음식 고르기를 띄울 창: 하우스 안이면 하우스 창, 아니면 펫 창
pub(crate) fn speech_target(pet: &Pet) -> &'static str {
    if pet.is_housed() {
        house::LABEL
    } else {
        OWN_LABEL
    }
}

/// 내 동물의 단계(또는 동물)가 바뀌었을 때: 창 크기, 펫 크기, 웹뷰 시트
fn apply_own_stage(app: &AppHandle, own: &mut Slot, care: &Care) {
    let payload = slots::stage_payload(&own.entry, care.stage, own.shown.wanted, own.dpi, own.size());
    own.set_stage(app, payload);
    sync_growth(app, care);
}

/// 직접 주기: 커서 끝에 음식 창을 띄움 (있던 것은 닫힘)
fn show_hand(app: &AppHandle, hand_win: &mut HandWindow, food: Food, (cx, cy): (f64, f64), own: &Slot) {
    let off = food::HAND_OFFSET * own.dpi;
    hand_win.show(app, food::hand_icon(food, &own.entry.id), (cx + off, cy + off), own.dpi);
}

/// 🧹 빗자루 창의 가운데: 쓰는 끝(왼쪽 아래)이 커서에 옴
fn broom_at((cx, cy): (f64, f64), dpi: f64) -> (f64, f64) {
    (cx + food::HAND_OFFSET * dpi, cy - food::HAND_OFFSET * dpi)
}

/// 픽셀을 골랐는데 지금 동물은 아직 그림이 없어 동글로 보임 — 한 줄 알림
fn hint_no_pixel_art(app: &AppHandle, own: &Slot) {
    if own.shown.wanted == PetStyle::Pixel && own.shown.style == PetStyle::Round {
        let _ = app.emit_to(speech_target(&own.pet), "pet-say-text", NO_PIXEL_ART_HINT);
    }
}

/// 사이 틱: 동물(내 동물·손님)이 서 있거나 매달린 창의 경계만 새로 읽어 세계에 고쳐 넣음 —
/// 창을 끌어 옮기면 100 ms 계단이 아니라 매 틱 따라감 (선물이 앉은 창도). 기대고 있는 창이 없으면 아무것도 읽지 않음
fn follow_ridden_windows(source: &mut dyn WindowSource, slots: &mut [Slot], gifts: impl Iterator<Item = u64>) {
    let mut ids: Vec<u64> = slots.iter().filter_map(|s| s.pet.ridden_window()).chain(gifts).collect();
    if ids.is_empty() {
        return;
    }
    ids.sort_unstable();
    ids.dedup();
    let rects: Vec<(u64, Rect)> = ids.into_iter().filter_map(|id| source.rect_of(id).map(|r| (id, r))).collect();
    for s in slots {
        s.world.patch_windows(&rects);
    }
}

/// 하우스 안, 집으로 가는 중, 또는 하우스에서 나와 먹는 중(다 먹으면 들어갈 참)
fn in_or_going_home(pet: &Pet) -> bool {
    pet.is_housed() || pet.is_going_home() || pet.returns_home()
}

/// 하우스에서 꺼냄: 문 앞에서 걸어 나옴 (가는 중이었다면 그 자리에서 멈춤). 집 창은 숨김
fn leave_house_now(app: &AppHandle, own: &mut Slot, house_window: Option<&tauri::WebviewWindow>) {
    let spot = house::spot(app, &own.world.work_areas, own.dpi);
    let was_inside = own.pet.is_housed();
    own.pet.leave_house(&own.world, spot.door_x, spot.ground);
    if was_inside {
        own.need_show = true;
    }
    if let Some(hw) = house_window {
        let _ = hw.hide();
    }
}

/// 지금 시각으로 난수 씨앗을 만듦
fn seed_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1)
}

/// 디버그: 번들 동물 하나(되도록 내 동물과 다른 종)를 오른쪽 끝에서 손님으로 들임
fn spawn_debug_guest(app: &AppHandle, slots: &mut Vec<Slot>) {
    let state = app.state::<AppState>();
    let own_id = &slots[OWN].entry.id;
    let Some(entry) = state.pets.iter().find(|p| &p.id != own_id).or(state.pets.first()).cloned() else { return };
    let stage = Stage::Teenager;
    let scale = stage.scale() * entry.manifest.size_mult(stage);
    let guest = GuestInfo { visit_id: "debug".into(), owner_id: "debug".into(), owner_name: "디버그 친구".into(), note: None, can_grow: true, tint: BodyTint::Default };
    match slots::spawn_guest(app, guest, entry, stage, scale, slots[OWN].shown.wanted, slots[OWN].dpi, Side::Right, &slots[OWN].world, seed_now(), slots[OWN].size()) {
        Ok(slot) => slots.push(slot),
        Err(e) => eprintln!("failed to create debug guest window: {e}"),
    }
}

/// 게임 루프 본체임: 상태를 만들고 끝날 때까지 틱을 돌림
fn run(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config) {
    let Some(window) = app.get_webview_window(OWN_LABEL) else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let house_window = app.get_webview_window(house::LABEL);
    let mut config = config;
    house::restore(config.house_spot.clone());
    let mut care = config.care.get(&entry.id).cloned().unwrap_or_default();
    let dpi = window.scale_factor().unwrap_or(1.0);
    let mut source = default_source();

    let shown = slots::stage_payload(&entry, care.stage, config.style, dpi, config.pet_size());
    let cfg = slots::pet_config(dpi, &entry, shown.scale, shown.stage, shown.style);
    let world = World::new(source.windows(), source.work_areas(), cfg.size);
    let b = world.bounds;
    let pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + cfg.size, seed_now());
    let mut slots = vec![Slot::new(OWN_LABEL.to_string(), Role::Own, entry, pet, window, world, dpi, shown.clone(), config.pet_size())];
    slots[OWN].tint = config.tint;
    crate::tray::sync_tint(&app.state::<AppState>().menu, config.tint);
    // 창 크기·웹뷰 시트를 맞춤
    slots[OWN].set_stage(&app, shown);
    slots[OWN].pet.set_hungry(care.is_hungry());
    if config.housed {
        // 지난번에 하우스 안에서 끝났음 — 하우스 안에서 시작함
        let own = &mut slots[OWN];
        let spot = house::spot(&app, &own.world.work_areas, own.dpi);
        // 몸은 문 앞 바닥에 둠 (화면 위에서 떨어지던 자리가 아니라)
        own.pet.body.x = spot.door_x;
        if let Some((_, ay)) = own.world.anchor(spot.ground) {
            own.pet.body.y = ay;
        }
        own.pet.enter_house();
        if let Some(hw) = &house_window {
            house::show_at(hw, &spot);
        }
    }
    sync_menu(&app, &slots[OWN].entry, &care, config.housed);
    let _ = app.state::<AppState>().menu.cursor_play.set_checked(!config.cursor_play_off);
    let _ = app.state::<AppState>().menu.typing_cheer.set_checked(!config.typing_cheer_off);
    let _ = app.state::<AppState>().menu.poop.set_checked(!config.poop_off);
    // 🚀 이 버전을 처음 켜면 자동 실행을 켜고 적어 둠. 그 뒤로는 고른 대로 OS에 다시 맞춤
    if crate::autostart::apply_at_start(&app, &mut config) {
        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
    }
    let mut was_housed = slots[OWN].pet.is_housed();
    let mut was_away = false;
    let mut last_house: Option<HouseState> = None;
    if std::env::var_os(DEBUG_GUEST_ENV).is_some() {
        spawn_debug_guest(&app, &mut slots);
    }
    let (mut network, new_id) = Network::start(&app, &mut config);
    if new_id {
        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
    }
    share_care(&network, &config, &slots[OWN], &care, false);
    // ☀️ 실제 날씨: 중계 서버에 고른 도시를 알림 (붙을 때마다 다시 — 옛 서버엔 보내지 않음)
    network.want_weather(config.weather_place);
    // 내 동물이 친구 화면에 가 있거나 가는 중 (하우스·동물 바꾸기 메뉴를 막음)
    let mut was_busy = false;

    let mut last = Instant::now();
    let mut last_refresh = last;
    // 틱은 정해진 시각마다 (일한 시간만큼 늦어지지 않게)
    let mut pacer = Pacer::new(TICK, last);
    let mut last_save = last;
    // 먹고 한 단계 컸지만 아직 먹는 중 — 먹기가 끝나면(축하 시작 또는 중단) 크기를 바꿈
    let mut pending_stage = false;
    // 내 동물이 먹을 때 앞에 놓이는 밥그릇·물그릇·쿠키 창
    let mut props = Props::default();
    // 직접 주기: 커서 끝에 붙은 음식과 그 그림 창
    let mut hand: Option<HandFeed> = None;
    let mut hand_win = HandWindow::default();
    let mut broom: Option<Broom> = None;
    let mut broom_win = HandWindow::default();
    // 계절 날씨 (철이 아니거나 꺼 두면 창도 일도 없음)
    let mut weather = WeatherFx::new(seed_now());
    let _ = app.state::<AppState>().menu.weather.set_checked(config.weather_on);
    weather.set_source(config.weather_source);
    crate::tray::sync_weather_kind(&app.state::<AppState>().menu, config.weather_source, config.weather_place);
    // 🌈 무지개 구름 자국 (무지개 동물이 달릴 때만 창이 생김)
    let mut trail = crate::trail::TrailFx::new();
    // ⌨️ 타자 응원: 최근 2초의 키 누름 *개수* (어떤 키인지는 읽지 않음)
    let mut typing_meter = screenpet_core::typing::TypingMeter::new();
    // 🍎 macOS 입력 모니터링 권한 (Windows는 None — 묻지도 않음). 없으면 이유를 말한 뒤 버전마다 한 번 물음
    let mut typing_permission = source.typing_permission();
    let mut typing_ask = PermissionAsk::start(Instant::now(), !config.typing_cheer_off, typing_permission, config.input_monitoring_asked.as_deref(), env!("CARGO_PKG_VERSION"));
    let mut next_permission_check = Instant::now() + TYPING_PERMISSION_CHECK;
    crate::tray::sync_typing_permission(&app.state::<AppState>().menu, !config.typing_cheer_off && typing_permission == Some(false));
    // 🎁 선물 (없으면 1초에 한 번 시계만 봄)
    let mut gifts = Gifts::new(&config.gifts, seed_now());
    // 자리 비움·돌아옴, 하루 일과, ⏱️ 집중 친구 (1초에 한 번)
    let mut presence = Presence::new(&app, config.focus_length);
    // 물건 (쓰레기 장난감 — 없거나 잠들어 있으면 일이 없음)
    let mut objects = Objects::new();
    let mut dodge = crate::dodge::DodgeFx::new();
    let mut sleigh = crate::sleigh::SleighFx::new();
    objects.set_size(&app, config.object_size());
    // 🕐 최근 한 시간 (메모리에만)
    let mut timeline = TimelineFx::new();
    // 🔄 새 버전 응원 (버전마다 한 번)
    let mut cheer_gate = screenpet_core::update::CheerGate::default();
    // 💩 왼쪽 버튼을 새로 누른 순간만 씀 (어디를 무엇을 눌렀는지는 보지도 남기지도 않음)
    let mut was_left_down = false;
    let mut poop_spots = Vec::new();
    // 🤹 흐물이가 주울 물건 자리, 노릴 동물 자리 (틱마다 다시 채움)
    let mut object_spots = Vec::new();
    let mut pet_targets: Vec<Option<screenpet_core::juggle::Target>> = Vec::new();
    let mut juggle_targets = Vec::new();
    // ⚾ 친구 화면에서 날아 들어오는 물건 (흐물이가 받으러 감)
    let mut incoming = Vec::new();
    // 한 화면의 동물끼리(내 동물 + 손님) 인사·술래 놀이
    let mut social = Social::new(seed_now());
    // 🎮 조작 모드: 켜진 동안만 키를 읽음 (읽기만 — 앞에 있는 창에도 그대로 감)
    let mut control_on = false;
    // macOS 권한 안내: 켠 시각, 키를 한 번이라도 읽었는지, 이미 안내했는지
    let mut control_since = Instant::now();
    let mut saw_key = false;
    let mut key_hint_shown = false;

    loop {
        // 한 틱에 처리할 입력은 정해 둠 (남은 건 다음 틱에) — 입력이 쏟아져도 화면이 멈추지 않음
        for _ in 0..MAX_INPUTS_PER_TICK {
            let Ok(input) = rx.try_recv() else { break };
            match input {
                Input::PointerDown { label } => {
                    // 한 번에 한 마리만 잡음. 음식을 들고 있는 동안의 누름은 주기·거두기 (아래 매 틱 처리)
                    if hand.is_some() || objects.is_held() || slots.iter().any(|s| s.pet.is_held()) {
                        continue;
                    }
                    if let Some(i) = slots::find(&slots, &label) {
                        let (cx, cy) = source.cursor();
                        slots[i].pet.pointer_down(cx, cy);
                        slots[i].start_glow();
                    }
                }
                Input::SetPet { entry: next, remote } => {
                    let own = &mut slots[OWN];
                    // 친구 화면에 가 있거나 가는 중엔 바꾸지 않음 (트레이 체크만 되돌림)
                    if network.own_busy(own) {
                        crate::tray::sync_pet_checks(&app.state::<AppState>().menu, &own.entry.id);
                        continue;
                    }
                    save_config(&app, &mut config, &own.entry.id, &care);
                    // 들고 있던 음식은 거둠 (좋아하는 음식 그림이 동물마다 다름)
                    if hand.take().is_some() {
                        own.pet.end_hand();
                        hand_win.drop_back();
                        app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
                    }
                    own.entry = next;
                    // 일시 확대는 그 동물 것임
                    own.boost = 1.0;
                    care = config.care.get(&own.entry.id).cloned().unwrap_or_default();
                    pending_stage = false;
                    apply_own_stage(&app, own, &care);
                    // 이전 동물의 종 전용 동작·벽 타기가 새 동물에 없을 수 있으니 진행 중이면 끝냄
                    own.pet.set_special(own.pet.cfg.special);
                    own.pet.set_can_climb(own.pet.cfg.can_climb);
                    // 만들던 눈사람도 — 눈덩이 창은 다음 틱에 weather가 치움 (새 동물이 날면 다시는 끝내지 못함)
                    own.pet.end_snowman();
                    own.pet.set_hungry(care.is_hungry());
                    // 하우스 안이면 새 동물도 하우스 안에 있음 (같은 Pet이므로 상태가 그대로 이어짐)
                    sync_menu(&app, &own.entry, &care, config.housed);
                    save_config(&app, &mut config, &own.entry.id, &care);
                    hint_no_pixel_art(&app, own);
                    if !remote {
                        share_care(&network, &config, own, &care, true);
                    } else {
                        // 📱 저쪽에서 바꿨음: 트레이 체크도 맞춤 (누른 항목만 스스로 뒤집힘)
                        crate::tray::sync_pet_checks(&app.state::<AppState>().menu, &own.entry.id);
                    }
                }
                Input::SetTint(tint) => {
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 다시 맞춤
                    crate::tray::sync_tint(&app.state::<AppState>().menu, tint);
                    if tint == config.tint {
                        continue;
                    }
                    config.tint = tint;
                    let own = &mut slots[OWN];
                    own.set_tint(&app, tint);
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                Input::SetStyle(style) => {
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 다시 맞춤
                    crate::tray::sync_style(&app.state::<AppState>().menu, style);
                    // 이미 고른 스타일이면 체크만 — 다시 그리면 하던 특별 동작이 끊김
                    if style == config.style {
                        continue;
                    }
                    config.style = style;
                    crate::art::set(style);
                    gifts.restyle();
                    let _ = app.emit("art-style", if style == PetStyle::Pixel { "pixel" } else { "round" });
                    let own = &mut slots[OWN];
                    own.restyle(&app, style);
                    // 소품·손 음식·눈덩이·눈사람 창은 그림을 만들 때 정함 — 새 스타일로 다시 만듦
                    props.restyle(&app, own.dpi, own.shown.scale, own.pixel_dot());
                    if let Some(h) = &hand {
                        show_hand(&app, &mut hand_win, h.food, source.cursor(), own);
                    }
                    if broom.is_some() {
                        broom_win.show(&app, food::BROOM_ICON, broom_at(source.cursor(), own.dpi), own.dpi);
                    }
                    weather.restyle(&app, own.dpi);
                    hint_no_pixel_art(&app, own);
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                Input::Feed(food) => {
                    let own = &mut slots[OWN];
                    // 직접 주기: 음식이 커서 끝에 붙음 (먹이기는 동물 입에 닿을 때). 하우스 안·친구 화면에선 그릇으로
                    if config.feed_mode.by_hand(food) && own.on_screen() {
                        if hand.is_some() {
                            own.pet.end_hand();
                        }
                        hand = Some(HandFeed::new(food));
                        show_hand(&app, &mut hand_win, food, source.cursor(), own);
                        app.state::<AppState>().hand_feeding.store(true, Ordering::Relaxed);
                        let _ = app.emit_to(OWN_LABEL, "hide-picker", ());
                        continue;
                    }
                    if own.pet.is_housed() {
                        // 하우스 안이면 문 앞에 그릇이 놓이고 동물이 나와서 먹음
                        let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                        own.pet.set_house_door(spot.ground, spot.door_x);
                    }
                    let before = care.clone();
                    let result = care.feed(food);
                    if own.pet.feed_result(result, food) {
                        if let FeedResult::Ate { leveled_up } = result {
                            pending_stage |= leveled_up;
                            timeline.ate(&app, food, &favorite_of(&own.entry), &config);
                        }
                        save_config(&app, &mut config, &own.entry.id, &care);
                        share_care(&network, &config, own, &care, true);
                    } else {
                        // 공중·드래그·놀러 가는 중이라 못 먹었음 — 먹지 않은 것으로 되돌림
                        care = before;
                    }
                    // 먹는 도중 클릭이 다시 음식을 조르지 않도록 바로 반영
                    own.pet.set_hungry(care.is_hungry());
                }
                Input::SetFeedMode(mode) => {
                    config.feed_mode = mode;
                    crate::tray::sync_feed_mode(&app.state::<AppState>().menu, mode);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleLock => {
                    care.locked = !care.locked;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    sync_menu(&app, &slots[OWN].entry, &care, config.housed);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    share_care(&network, &config, &slots[OWN], &care, true);
                }
                Input::ResetGrowth => {
                    let own = &mut slots[OWN];
                    care.reset();
                    pending_stage = false;
                    apply_own_stage(&app, own, &care);
                    own.pet.set_hungry(care.is_hungry());
                    save_config(&app, &mut config, &own.entry.id, &care);
                    share_care(&network, &config, own, &care, true);
                }
                Input::ToggleHouse => {
                    let own = &mut slots[OWN];
                    // 친구 화면에 가 있거나 가는 중엔 하우스를 다루지 않음
                    if network.own_busy(own) {
                        sync_menu(&app, &own.entry, &care, config.housed);
                        continue;
                    }
                    let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                    if in_or_going_home(&own.pet) {
                        leave_house_now(&app, own, house_window.as_ref());
                        config.housed = false;
                    } else {
                        // 집이 먼저 나타나고, 동물이 집으로 달려감 (모두 숨겨 뒀으면 보일 때 나타남)
                        if let Some(hw) = house_window.as_ref().filter(|_| !crate::windows_hidden(&app)) {
                            house::show_at(hw, &spot);
                        }
                        own.pet.go_home(spot.ground, spot.door_x);
                        // 펫 창에 떠 있던 음식 고르기는 닫음
                        let _ = app.emit_to(OWN_LABEL, "hide-picker", ());
                        config.housed = true;
                    }
                    sync_menu(&app, &own.entry, &care, config.housed);
                    save_config(&app, &mut config, &own.entry.id, &care);
                }
                // 똥피하기 창이 열려 있으면 조작 모드는 켜지 않음 (게임이 키를 줌)
                Input::ToggleControl if dodge.is_open() => {
                    let _ = app.state::<AppState>().menu.control.set_checked(false);
                }
                Input::ToggleControl => {
                    control_on = !control_on;
                    let own = &mut slots[OWN];
                    own.pet.set_control(control_on);
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.control.set_checked(control_on);
                    if control_on {
                        control_since = Instant::now();
                        saw_key = false;
                    }
                }
                Input::ToggleBroom => {
                    if broom.take().is_some() {
                        broom_win.drop_back();
                    } else {
                        let cursor = source.cursor();
                        broom = Some(Broom::new(cursor));
                        broom_win.show(&app, food::BROOM_ICON, broom_at(cursor, slots[OWN].dpi), slots[OWN].dpi);
                    }
                    let _ = app.state::<AppState>().menu.broom.set_checked(broom.is_some());
                }
                Input::ToggleWeather => {
                    config.weather_on = !config.weather_on;
                    weather.sync_menu(&app, !config.weather_on);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::SetWeatherSource(source) => {
                    crate::tray::sync_weather_kind(&app.state::<AppState>().menu, source, config.weather_place);
                    if source == config.weather_source {
                        continue;
                    }
                    config.weather_source = source;
                    weather.set_source(source);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::SetWeatherPlace(place) => {
                    crate::tray::sync_weather_kind(&app.state::<AppState>().menu, config.weather_source, place);
                    if place == config.weather_place {
                        continue;
                    }
                    config.weather_place = place;
                    // 예전 도시의 날씨는 버리고 (새 값이 올 때까지 달력 날씨) 서버에 새 도시를 달라고 함
                    weather.set_reading(None);
                    network.want_weather(place);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleFocus => presence.toggle(&app, config.focus_length, &mut slots[OWN].pet),
                Input::SetFocusLength(length) => {
                    config.focus_length = length;
                    presence.set_length(&app, length, &mut slots[OWN].pet);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleCursorPlay => {
                    config.cursor_play_off = !config.cursor_play_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.cursor_play.set_checked(!config.cursor_play_off);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleTypingCheer => {
                    config.typing_cheer_off = !config.typing_cheer_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.typing_cheer.set_checked(!config.typing_cheer_off);
                    // 🍎 다시 켜면 (권한이 없을 때) 또 물음, 끄면 기다리던 묻기도 취소
                    typing_permission = source.typing_permission();
                    typing_ask.toggled(Instant::now(), !config.typing_cheer_off, typing_permission);
                    crate::tray::sync_typing_permission(&app.state::<AppState>().menu, !config.typing_cheer_off && typing_permission == Some(false));
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::TogglePoop => {
                    config.poop_off = !config.poop_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.poop.set_checked(!config.poop_off);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::ToggleAutostart => {
                    crate::autostart::toggle(&app, &mut config);
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                }
                Input::CollectGift { label } => {
                    if gifts.collect(&app, &label, &mut config.gifts, &slots[OWN]) {
                        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    }
                }
                Input::HouseClick => {
                    if slots[OWN].pet.is_housed() {
                        if care.is_hungry() {
                            let _ = app.emit_to(house::LABEL, "food-picker", favorite_of(&slots[OWN].entry));
                        } else {
                            let _ = app.emit_to(house::LABEL, "house-wave", ());
                        }
                    }
                }
                Input::HouseMoved { save } => {
                    // 문 자리가 집을 따라감: 집에 가는 중·나와서 먹는 중이면 새 문으로 (하던 일은 끊지 않음),
                    // 안에 있으면 그대로 안에 (먹이 그릇은 새 문 앞에)
                    let own = &mut slots[OWN];
                    let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                    own.pet.set_house_door(spot.ground, spot.door_x);
                    if save {
                        config.house_spot = house::placed();
                        save_config(&app, &mut config, &own.entry.id, &care);
                    }
                }
                Input::FeedGuest { label } => {
                    // 성장 멈추기 친구 동물은 키우지 않음 (메뉴도 막혀 있음)
                    if let Some(i) = slots::find(&slots, &label).filter(|&i| matches!(&slots[i].role, Role::Guest(g) if g.can_grow)) {
                        let slot = &mut slots[i];
                        // 친구 동물은 내 Care를 건드리지 않음. 못 먹는 때(공중·떠나는 중)면 그대로
                        if slot.pet.feed_result(FeedResult::Ate { leveled_up: false }, Food::Meal) {
                            let grown = screenpet_core::visit::next_boost(slot.boost, slot.boost_cap());
                            if grown != slot.boost {
                                slot.set_boost(&app, grown);
                            }
                        }
                    }
                }
                Input::SendHome { label } => {
                    if let Some(i) = slots::find(&slots, &label).filter(|&i| !slots[i].is_own()) {
                        network.send_guest_home(&mut slots[i]);
                    }
                }
                Input::NoteSpace { label, open } => {
                    // 손님이 들고 온 편지, 또는 돌아온 내 동물이 들고 온 답장
                    if let Some(i) = slots::find(&slots, &label) {
                        slots[i].set_note_open(open);
                    }
                }
                // 답장 창 "보내기": 그 손님이 답장을 들고 손 흔들고 집으로 (편지 창이 답장 창으로 열려 있을 때)
                Input::Net(NetInput::SubmitLetter(text)) if network.is_replying() => {
                    let sent = network.submit_reply(&text, &mut slots);
                    network.finish_letter(&app, sent);
                }
                input @ (Input::Net(NetInput::SendTo(_)) | Input::Net(NetInput::SubmitLetter(_))) => {
                    // 놀러 보내기 (편지 들려 보내기도 같은 길 — Visit에 편지를 실음):
                    // 하우스 안이면 먼저 문 앞으로 나온 뒤 그 친구 쪽 끝으로 감
                    let (id, note, is_letter) = match input {
                        Input::Net(NetInput::SubmitLetter(text)) => {
                            let Some((id, note)) = network.letter_target(&text) else { continue };
                            (id, note, true)
                        }
                        Input::Net(NetInput::SendTo(id)) => (id, None, false),
                        _ => continue,
                    };
                    let own = &mut slots[OWN];
                    let mut sent = false;
                    if network.can_send(own, &id) {
                        if in_or_going_home(&own.pet) {
                            leave_house_now(&app, own, house_window.as_ref());
                            config.housed = false;
                            sync_menu(&app, &own.entry, &care, config.housed);
                            save_config(&app, &mut config, &own.entry.id, &care);
                        }
                        // 그 자리에서 손 흔들고("bye~!", 편지를 들었으면 "💌!") 뿅 사라짐 — 사라지면 own_exited가 Visit을 보냄
                        sent = network.send_to(&config, own, &id, note);
                    }
                    if is_letter {
                        network.finish_letter(&app, sent);
                    }
                }
                Input::Object(input) => {
                    let busy = hand.is_some() || slots.iter().any(|s| s.pet.is_held());
                    objects.handle(&app, input, &slots[OWN].world, source.cursor(), busy);
                }
                // 친구가 던진 물건 (net이 인증된 친구 링크·친구별 한도를 확인했고, 받은 물건 20개 상한과 다듬기는 objects가 지키고, 창은 틱마다 하나씩 만듦)
                Input::Net(NetInput::Event(screenpet_net::NetEvent::Tossed { from, toss })) => {
                    // 🧹 친구가 쓸어 보낸 더미: 내 친구의 것만, 집중·하우스 중이면 조용히 버림
                    if let Some(kind) = screenpet_core::weather::parse_flurry_kind(&toss.kind) {
                        if config.friend(&from).is_some() && !slots[OWN].pet.refuses_arrivals() {
                            let at = toss.fling.as_ref().map_or(0.5, |f| f.height);
                            weather.receive_flurry(&app, kind, crate::network::from_net_side(toss.side, toss.vside), at);
                        }
                        continue;
                    }
                    // 집중·하우스 중이면 받지 않고 되돌림
                    match screenpet_core::visit::toss_answer(slots[OWN].pet.refuses_arrivals(), toss.bounced, objects.threw_lately(&from)) {
                        screenpet_core::visit::TossAnswer::Take => objects.receive(&config, &from, toss),
                        screenpet_core::visit::TossAnswer::SendBack => network.bounce_toss(&from, toss),
                        screenpet_core::visit::TossAnswer::Drop => {}
                    }
                }
                // 📱 노는 내 다른 컴퓨터의 펫과 성장 (net이 그 컴퓨터가 놀 때만, 모양을 확인해 넘김):
                // 아는 동물만 다듬어 합치고 (보내지 않은 동물은 그대로), 화면의 내 펫·단계·배고픔도 바로 맞춤. 다시 보내지 않음
                // ☀️ 중계 서버가 준 고른 도시의 지금 날씨 (net이 다른 도시 값은 거름)
                Input::Net(NetInput::Event(screenpet_net::NetEvent::Weather { code, at })) => {
                    weather.set_reading(Some(Reading { code, at }));
                }
                Input::Net(NetInput::Event(screenpet_net::NetEvent::DeviceState { from, pet, care: shared })) => {
                    if config.linked_device().map(|(id, _)| id) != Some(from.as_str()) {
                        continue;
                    }
                    let state = app.state::<AppState>();
                    let known = |p: &str| state.pets.iter().any(|e| e.id == p);
                    let list: Vec<linked::SharedCare> = shared
                        .into_iter()
                        .map(|c| linked::SharedCare { pet: c.pet, stage: c.stage, stamina: c.stamina, fullness: c.fullness, locked: c.locked })
                        .collect();
                    let own = &mut slots[OWN];
                    config.care.insert(own.entry.id.clone(), care.clone());
                    let got = linked::adopt_shared(&mut config.care, &own.entry.id, pet.as_deref(), &list, known);
                    if got.changed {
                        let shown = care.stage;
                        care = config.care.get(&own.entry.id).cloned().unwrap_or_default();
                        if care.stage != shown || pending_stage {
                            pending_stage = false;
                            apply_own_stage(&app, own, &care);
                        }
                        own.pet.set_hungry(care.is_hungry());
                        sync_menu(&app, &own.entry, &care, config.housed);
                        save_config(&app, &mut config, &own.entry.id, &care);
                    }
                    // 펫 바꾸기는 평소 길로 (친구에게 가 있으면 다음 소식 때 다시)
                    if let Some(entry) = got.switch_to.and_then(|id| state.pets.iter().find(|e| e.id == id).cloned()) {
                        state.send(Input::SetPet { entry, remote: true });
                    }
                }
                Input::Net(input) => {
                    if network.handle(&app, input, &mut config, &mut slots) {
                        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    }
                    // 🤹 흐물이가 들고 온 물건 (손님), 못 들어가 들고 돌아온 물건 (내 동물): 그 손에 쥐여 줌
                    for c in network.take_carry_arrivals() {
                        let Some(s) = slots.iter_mut().find(|s| s.label == c.label) else { continue };
                        let at = (s.pet.body.x, s.pet.body.y - s.pet.cfg.size);
                        if let Some(id) = objects.spawn_carried(&app, &s.world, c.kind, c.scale, s.is_own(), at) {
                            s.pet.hold_object(id);
                        }
                    }
                }
                Input::ShowHelp => crate::help::show(&app),
                Input::ShowFeedback => crate::feedback::show(&app),
                Input::ShowMailbox => crate::mailbox::show(&app),
                Input::ShowBoard => crate::board::show(&app),
                Input::ShowGiftBook => crate::gifts::show_book(&app),
                Input::ShowDodge => {
                    // 조작 모드는 게임이 대신함
                    if control_on {
                        control_on = false;
                        slots[OWN].pet.set_control(false);
                        let _ = app.state::<AppState>().menu.control.set_checked(false);
                    }
                    dodge.open(&app, &slots[OWN], &config);
                }
                Input::Dodge(d) => dodge.handle(&app, d, &mut slots[OWN], &config),
                Input::ShowSizes => crate::sizes::show(&app, &config),
                Input::SetSizes { pet, object, save } => {
                    config.set_sizes(pet, object);
                    crate::sizes::remember(&config);
                    for slot in slots.iter_mut() {
                        slot.set_size(&app, config.pet_size());
                    }
                    objects.set_size(&app, config.object_size());
                    if save {
                        // 밥그릇·손 음식처럼 펫 크기로 만든 창을 맞추고 남김
                        let own = &slots[OWN];
                        props.restyle(&app, own.dpi, own.shown.scale, own.pixel_dot());
                        save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    }
                }
                Input::ShowTimeline => timeline.show(&app, &config),
                Input::NewVersion(v) => cheer_gate.found(v),
                Input::Quit => {
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    app.exit(0);
                    return;
                }
                Input::SaveAndStop(done) => {
                    save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                    let _ = done.send(());
                    return;
                }
            }
        }

        let now = Instant::now();
        // 타임라인은 실제로 흐른 시간을 씀 (컴퓨터가 잤으면 그 시간도 지금 자리에 더함)
        let real_dt = (now - last).as_secs_f64();
        let dt = real_dt.min(MAX_DT);
        last = now;
        care.tick(dt);
        if now - last_refresh >= WORLD_REFRESH {
            let (windows, work_areas) = (source.windows(), source.work_areas());
            for s in &mut slots {
                s.refresh(&windows, &work_areas);
            }
            objects.refresh(&windows, &work_areas, now);
            slots[OWN].pet.set_hungry(care.is_hungry());
            last_refresh = now;
        } else {
            follow_ridden_windows(source.as_mut(), &mut slots, gifts.ridden_windows());
        }
        if now - last_save >= SAVE_EVERY {
            save_config(&app, &mut config, &slots[OWN].entry.id, &care);
            // 📱 노는 동안 가끔 지금 포만감까지 (쉬는 컴퓨터도 같은 시간만큼 배고파짐)
            share_care(&network, &config, &slots[OWN], &care, false);
            last_save = now;
        }

        // 커서·간질이기·드래그는 커서 아래 맨 위 한 마리에게만
        let (cx, cy) = source.cursor();
        // 이번 틱의 Esc가 직접 주기 거두기라면 조작 모드는 끝내지 않음
        let hand_active = hand.is_some();
        // 직접 주기: 음식이 커서를 따라가고, 내 동물 입에 닿으면 먹음. 오른쪽 버튼·Esc·빈 곳 클릭·30초면 거둠
        if let Some(h) = hand.as_mut() {
            let own = &mut slots[OWN];
            let off = food::HAND_OFFSET * own.dpi;
            let at = (cx + off, cy + off);
            hand_win.move_to(at, own.dpi);
            let input = h.tick(dt, source.left_button_down(), source.right_button_down(), source.escape_down());
            let food = h.food;
            let ready = input != HandInput::Cancel && own.on_screen() && own.pet.offer_hand(at.0, at.1);
            let mut done = false;
            if ready {
                let before = care.clone();
                let result = care.feed(food);
                if own.pet.eat_from_hand(result, food) {
                    if let FeedResult::Ate { leveled_up } = result {
                        pending_stage |= leveled_up;
                        timeline.ate(&app, food, &favorite_of(&own.entry), &config);
                        hand_win.bite();
                        done = true;
                        save_config(&app, &mut config, &own.entry.id, &care);
                        share_care(&network, &config, own, &care, true);
                    }
                    // 배불러 갸웃 — 음식은 손에 남음
                } else {
                    care = before;
                }
                own.pet.set_hungry(care.is_hungry());
            } else {
                // 동물 위를 누른 건 거두기가 아님 (닿을 때까지 더 가까이)
                let empty_click = input == HandInput::Click && !own.cursor_inside(cx, cy);
                if input == HandInput::Cancel || empty_click || !own.on_screen() {
                    own.pet.end_hand();
                    hand_win.drop_back();
                    done = true;
                }
            }
            if done {
                hand = None;
                app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
            }
        }
        // 🧹 빗자루: 커서를 따라다니며 지나가는 더미를 씀. 쓴 것을 친구 쪽 끝으로 밀면 친구 화면에 흩날림
        if let Some(b) = broom.as_mut() {
            let dpi = slots[OWN].dpi;
            broom_win.move_to(broom_at((cx, cy), dpi), dpi);
            if b.tick(dt, (cx, cy), source.left_button_down(), source.right_button_down(), source.escape_down()) {
                broom = None;
                broom_win.drop_back();
                let _ = app.state::<AppState>().menu.broom.set_checked(false);
            } else {
                objects.sweep_poops(b);
                let (x0, x1) = b.span(dpi);
                if let Some((amount, kind)) = weather.sweep(x0, x1, cy, Broom::reach_y(dpi)) {
                    if let Some(count) = b.swept(amount) {
                        weather.puff(&app, kind, (cx, cy), b.velocity(), count);
                    }
                    let world = &slots[OWN].world;
                    let edges = screenpet_core::visit::column_bounds(world, cx).map_or(world.bounds, |c| Rect::new(world.bounds.left, c.top, world.bounds.right, c.bottom));
                    if let Some(side) = b.crossing(&edges, dpi, |s| !config.friends_at(s, |id| network.is_connected(id)).is_empty()) {
                        network.flurry_to(&config, side, kind, b.fling(side, &edges, dpi));
                    }
                }
            }
        }
        // 🎮 조작 모드: 키를 내 동물에게 (친구 화면에 가 있으면 친구 링크로도). Esc(직접 주기 중이 아닐 때)·키 없이 3분이면 끝
        let mut keys = ControlInput::default();
        // 💩 똥피하기: 게임 창이 받은 키를 내 동물에게
        dodge.steer(&mut slots[OWN], dt);
        if control_on {
            keys = source.control_keys();
            saw_key |= keys.any();
            let own = &mut slots[OWN];
            if !hand_active && source.escape_down() {
                own.pet.set_control(false);
            } else {
                own.pet.control_keys(keys, dt);
            }
            if !own.pet.is_controlled() {
                control_on = false;
                own.pet.take_control_ended();
                keys = ControlInput::default();
                let _ = app.state::<AppState>().menu.control.set_checked(false);
            } else if cfg!(target_os = "macos") && !saw_key && !key_hint_shown && now - control_since >= KEY_PERMISSION_HINT_AFTER {
                // 입력 모니터링 권한이 없으면 다른 앱이 앞에 있을 때 키가 늘 안 눌림으로 읽힘
                key_hint_shown = true;
                if own.on_screen() {
                    let _ = app.emit_to(speech_target(&own.pet), "pet-say-text", crate::tray::KEY_PERMISSION_HINT);
                }
            }
        }
        network.steer(now, control_on, keys);
        let held = slots.iter().position(|s| s.pet.is_held());
        let hovered = if held.is_some() || objects.is_held() { None } else { slots::hover_target(&slots, cx, cy) };
        // 🐾 커서 장난: 무언가 끌고 있거나 음식을 들고 있거나 모두 숨겨 둔 동안엔 새로 시작하지 않음 (조작 모드 등은 코어가 봄)
        let left_down = source.left_button_down();
        // 💩 새로 누른 틱에만: 화면 위 동물(내 동물·손님)마다 낮은 확률로 쌈. 자리 비움·전체 화면(그 동물이 있는 모니터)·숨김·집중 중엔 안 함
        if std::mem::replace(&mut was_left_down, left_down) != left_down
            && left_down
            && screenpet_core::poop::may_poop(config.poop_off, crate::windows_hidden(&app), slots[OWN].pet.is_focusing())
        {
            for s in &mut slots {
                let wa = s.world.work_area_at(s.pet.body.x, s.pet.body.y);
                if screenpet_core::weather::overlay_paused(presence.is_away(), &s.world.windows, &wa) {
                    continue;
                }
                if let Some((x, y)) = s.pet.try_poop() {
                    objects.drop_poop(screenpet_core::poop::pick_kind(&s.entry.id, fastrand::f64()), x, y, screenpet_core::poop::size_for(s.shown.scale));
                }
            }
        }
        let play_busy = held.is_some() || hand.is_some() || objects.is_held() || crate::windows_hidden(&app);
        // ⌨️ 타자 응원: 켜져 있고 내 흐물이가 있을 때만 키 누름 *개수*를 읽음 (어떤 키인지는 읽지도 남기지도 않음 —
        // screenpet_core::typing). 꺼져 있으면 센서를 부르지 않음. 손님에게는 넘기지 않음
        // 🍎 권한이 없으면 세지 않음 (몇 초마다 확인만 — 생기면 바로 셈)
        if typing_permission == Some(false) && !config.typing_cheer_off && now >= next_permission_check {
            next_permission_check = now + TYPING_PERMISSION_CHECK;
            let now_permission = source.typing_permission();
            if now_permission != typing_permission {
                crate::tray::sync_typing_permission(&app.state::<AppState>().menu, now_permission == Some(false));
            }
            typing_permission = now_permission;
        }
        match typing_ask.tick(now, !config.typing_cheer_off, typing_permission) {
            Some(AskStep::Explain) => {
                if slots[OWN].on_screen() {
                    let _ = app.emit_to(speech_target(&slots[OWN].pet), "pet-say-text", crate::tray::TYPING_PERMISSION_EXPLAIN);
                }
            }
            Some(AskStep::Request) => {
                config.input_monitoring_asked = Some(env!("CARGO_PKG_VERSION").to_string());
                save_config(&app, &mut config, &slots[OWN].entry.id, &care);
                source.request_typing_permission();
            }
            Some(AskStep::RestartHint) => {
                if slots[OWN].on_screen() {
                    let _ = app.emit_to(speech_target(&slots[OWN].pet), "pet-say-text", crate::tray::TYPING_RESTART_HINT);
                }
            }
            None => {}
        }
        // 새로 눌린 수 (개수뿐)는 속도계와 흐물이 손(⌨️ 키보드 앞에서 같이 톡) 둘 다에
        let (typing_rate, typing_presses) = if !config.typing_cheer_off && typing_permission != Some(false) && slots.iter().any(is_own_heumul) {
            let presses = source.key_presses().unwrap_or(0);
            typing_meter.add(now, presses);
            (typing_meter.rate(now), presses)
        } else {
            typing_meter.clear();
            (0.0, 0)
        };
        for s in slots.iter_mut().filter(|s| is_own_heumul(s)) {
            s.pet.set_typing_rate(typing_rate);
            s.pet.typing_keys(typing_presses);
        }
        for (i, s) in slots.iter_mut().enumerate() {
            s.pet.cursor_input(left_down, play_busy || s.note_is_open(), !config.cursor_play_off);
            if Some(i) == held {
                if source.left_button_down() {
                    s.pet.pointer_move(cx, cy);
                } else {
                    s.pet.pointer_up(cx, cy);
                }
            } else {
                s.pet.hover(cx, cy, Some(i) == hovered);
            }
        }

        // 가까이 마주친 동물끼리 인사·술래 놀이 (손길·먹이·집 생각은 각 동물 update가 먼저 챙김)
        social.tick(dt, &mut slots);
        // 던진 물건·똥이 동물에 부딪힘 (동물 update 전에, 말풍선은 아래에서 나감)
        objects.hit_pets(&mut social, dt, &mut slots);
        // 연결된 친구가 있는 끝으로만 나감
        network.set_exit_edges(&config, &mut slots);
        // 똥피하기 창이 열려 있는 동안엔 화면 끝으로 나가지 않음
        if dodge.is_open() {
            slots[OWN].pet.set_exit_edges(false, false);
            slots[OWN].pet.set_vertical_edges(false, false);
        }
        let mut own_exit = None;
        let mut left = Vec::new();
        objects.poop_spots(&mut poop_spots);
        objects.object_spots(&mut object_spots);
        // 📌 작업표시줄 장난감을 지금 만들 수 있나 (내 흐물이만 봄)
        let toys = objects.toys(&app);
        objects.incoming(&mut incoming);
        let all_hidden = crate::windows_hidden(&app);
        // 🙆 흐물이에게 같은 화면의 다른 동물을 보여 줌 (번쩍 들러 감)
        screenpet_core::brain::hoist_moves::notice_pets(&mut slots);
        pet_targets.clear();
        pet_targets.extend(slots.iter().map(|s| {
            (!s.pet.is_away() && s.pet.hit_feels()).then(|| screenpet_core::juggle::Target { x: s.pet.body.x, y: s.pet.body.y, size: s.pet.cfg.size })
        }));
        for (i, s) in slots.iter_mut().enumerate() {
            // 친구 화면에 가 있거나 🛷 썰매에 탄 동안엔 물리·행동이 멈춤
            if !s.pet.is_away() && !sleigh.is_riding(&s.label) {
                // 🤹 흐물이: 바닥의 물건을 주우러 가고, 같은 화면의 다른 동물을 노림 (다른 동물은 할 일이 없음)
                if s.pet.cfg.juggler {
                    juggle_targets.clear();
                    juggle_targets.extend(pet_targets.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, t)| *t));
                    s.pet.notice_objects(&object_spots, &juggle_targets, &s.world);
                    s.pet.notice_incoming(&incoming, &s.world);
                }
                s.pet.notice_poops(&poop_spots, &s.world);
                // 🐱 고양이 툭툭 · 🐿️ 다람쥐 볼 빵빵 (v0.6.3): 바닥의 물건 — 모두 숨겨 둔 동안엔 보여 주지 않음
                s.pet.notice_play_objects(if all_hidden { &[] } else { &object_spots }, &s.world);
                // 📌 흐물이: 가끔 작업표시줄(Dock)에서 열린 앱 아이콘 장난감을 끌어내고, 갖고 놀다 도로 넣음 (손님은 하지 않음)
                if s.pet.cfg.taskbar_play && s.is_own() {
                    s.pet.notice_taskbar(&object_spots, toys, &s.world);
                }
                s.pet.update(dt, &s.world);
            }
            if let Some(speech) = s.pet.take_speech() {
                let target = if s.is_own() { speech_target(&s.pet) } else { s.label.as_str() };
                let _ = app.emit_to(target, "pet-say", speech);
            }
            // ⚾ 받은 횟수 ("3!") 같은 글자 그대로의 말
            if let Some(text) = s.pet.take_speech_text() {
                let target = if s.is_own() { speech_target(&s.pet) } else { s.label.as_str() };
                let _ = app.emit_to(target, "pet-say-text", text);
            }
            // 밥그릇 놓기·거두기 (손님은 먹이를 받지 않음)
            if let Some(ev) = s.pet.take_prop() {
                if s.is_own() {
                    props.apply(&app, ev, s.dpi, s.shown.scale, s.pixel_dot());
                }
            }
            if let Some(side) = s.pet.take_exit() {
                if s.is_own() {
                    own_exit = Some(side);
                } else {
                    // 손님이 집 쪽(또는 다른 친구 쪽) 끝으로 걸어 나갔거나 손 흔들고 사라졌음
                    left.push((i, side));
                }
            }
        }
        // 🙆 흐물이가 든 동물을 손 자리로 옮기고, 내려놓고, 함께 나간 것을 맞춤 (손님을 닫기 전에)
        for (lifter, carried, side) in screenpet_core::brain::hoist_moves::sync_lifts(&mut slots) {
            // 손님 흐물이가 이 화면 주인의 동물을 들고 친구 쪽 끝으로 나갔음: 둘 다 같은 친구에게 가게 함
            if carried == OWN {
                if let Role::Guest(g) = &slots[lifter].role {
                    network.pair_lift(&config, g, side);
                }
            }
        }
        if own_exit.is_none() {
            own_exit = slots[OWN].pet.take_exit();
        }
        // 머리 위에 선 동물은 아래 친구를 따라감 (창을 옮기기 전, 아래층부터)
        social.carry(&mut slots);
        for (i, side) in left.into_iter().rev() {
            let mut slot = slots.remove(i);
            // 던져서 넘어갔으면 그 움직임 (주인 화면이나 다음 친구 화면으로 날아감)
            let fling = slot.pet.take_fling();
            if let Role::Guest(g) = &slot.role {
                eprintln!("guest {} ({}) left", g.visit_id, g.owner_id);
                network.guest_left(&config, &slot, g, side, slot.pet.guest(), fling);
            }
            slots::close(&app, slot);
        }
        network.forget_gone_replies(&slots);

        // 🤹 흐물이가 든 물건을 손 자리로 옮기고, 놓은 것은 던짐
        objects.sync_carries(&mut social, &mut slots);
        // 🐱 툭 밀기 · 🐿️ 뿅 · 🦖 큰 쿵에 톡 (v0.6.3)
        objects.play_with(&mut slots);
        // 📌 읽은 앱 아이콘이 왔으면 장난감을 만들어 내 흐물이 손에 쥐여 줌
        {
            let own = &mut slots[OWN];
            objects.deliver_toy(&app, &own.world, &mut own.pet);
        }
        if let Some(side) = own_exit {
            // 내 동물이 화면 끝으로 다 나갔음 — 그 끝의 친구에게 놀러 감 (물건을 들고 나갔으면 새 버전 친구에게 실음)
            let carried = slots[OWN].pet.carried_object();
            let carry = carried.and_then(|id| objects.carry_wire(id));
            let took = network.own_exited(&config, &mut slots[OWN], side, carry);
            if let Some(id) = carried {
                if took {
                    objects.hand_over(id);
                }
                // 못 실었으면 이 화면 끝에 떨어짐 (다시 들어오는 중이면 그대로 들고 옴)
                if took || slots[OWN].pet.is_away() {
                    slots[OWN].pet.carry_handed_over();
                }
            }
        }
        network.tick(&app, now, &config, &mut slots[OWN]);
        network.tick_mailbox(&app, &mut slots, now);
        timeline.tick(&app, real_dt, &slots[OWN], network.away_friend(), &config);
        if dodge.tick(&app, dt, &mut slots[OWN], &mut config, &network) {
            save_config(&app, &mut config, &slots[OWN].entry.id, &care);
        }
        objects.tick(&app, &slots[OWN].world, dt, now, (cx, cy), left_down, presence.is_away(), source.as_mut(), &network, &config);
        props.tick(now);
        hand_win.tick(now);
        broom_win.tick(now);
        network.growth_locked = care.locked;
        network.tick_boost(&app, &mut slots[OWN], now);
        weather.tick(&app, now, !config.weather_on, presence.is_away(), &mut slots);
        trail.tick(&app, now, dt, presence.is_away(), &mut slots);
        for g in network.take_guest_gifts() {
            gifts.guest_arrived(g, now);
        }
        gifts.tick(&app, now, dt, &slots, &care);
        if presence.tick(&app, now, source.as_mut(), &mut slots[OWN], &mut care, &mut config.daily) {
            save_config(&app, &mut config, &slots[OWN].entry.id, &care);
            share_care(&network, &config, &slots[OWN], &care, false);
        }
        // 📱 대기 중인 이 컴퓨터를 다시 쓰기 시작했으면 이 컴퓨터로 놂 (쓴 지 얼마나 됐는지는 net에도)
        if let Some((secs, idle)) = presence.take_sample() {
            network.tick_presence(secs, idle);
        }
        // 손님도 이 화면 주인이 자리를 비운 동안엔 잠만 잠
        for s in &mut slots[OWN + 1..] {
            s.pet.set_user_away(presence.is_away());
        }
        // 🔄 새 버전: 내 동물들이 "new version!!" 하고 10초쯤 뜀 — 손님은 아님. 창을 숨겨 둔 동안엔 다시 보일 때,
        // 자리 비움이면 돌아왔을 때 (동물 쪽이 미룸)
        if cheer_gate.due(crate::windows_hidden(&app)) {
            for s in slots.iter_mut().filter(|s| s.is_own()) {
                s.pet.cheer(screenpet_core::update::CHEER_SECS);
            }
        }
        let own = &mut slots[OWN];
        let busy = network.own_busy(own);
        if busy != was_busy {
            was_busy = busy;
            crate::tray::set_own_busy(&app.state::<AppState>().menu, busy);
        }
        if own.pet.take_food_request() {
            let _ = app.emit_to(speech_target(&own.pet), "food-picker", favorite_of(&own.entry));
        }
        if own.pet.is_housed() != was_housed {
            was_housed = own.pet.is_housed();
            app.state::<AppState>().housed.store(was_housed, Ordering::Relaxed);
            if was_housed {
                // 문에 닿았음 — 펫 창을 숨기고 하우스 창 문간에 보임
                let _ = own.window.hide();
                // 집으로 가는 동안 모두 숨겼다가 다시 보였으면 집이 아직 안 보임
                if let Some(hw) = house_window.as_ref().filter(|w| !crate::windows_hidden(&app) && !w.is_visible().unwrap_or(true)) {
                    let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                    house::show_at(hw, &spot);
                }
            } else {
                // 하우스에서 나왔음 (나오기, 또는 문 앞 그릇으로 먹으러) — 문 앞 자리로 옮긴 뒤 보임
                own.need_show = true;
            }
        }
        if own.pet.is_away() != was_away {
            was_away = own.pet.is_away();
            app.state::<AppState>().away.store(was_away, Ordering::Relaxed);
            if was_away {
                // 화면 끝으로 걸어 나갔음 — 돌아올 때까지 펫 창을 숨김
                let _ = own.window.hide();
            } else {
                // 돌아왔음 — 들어오는 자리로 옮긴 뒤 보임
                own.need_show = true;
            }
        }
        // 지금 보이는 시트·크기 (성장 직후 먹는 동안엔 care.stage보다 한 단계 늦음)
        let house_state = HouseState {
            pet: own.shown.pet.clone(),
            stage: own.shown.stage,
            scale: own.shown.base,
            style: own.shown.style,
            hungry: care.is_hungry(),
            housed: own.pet.is_housed(),
        };
        if last_house.as_ref() != Some(&house_state) {
            *app.state::<AppState>().house.lock().unwrap() = house_state.clone();
            let _ = app.emit_to(house::LABEL, "house-state", house_state.clone());
            // 스타일이 바뀌면 집 크기와 문 자리도 바뀜 — 집으로 가는 동물이 새 문으로 감
            let restyled = last_house.as_ref().is_some_and(|h| h.style != house_state.style);
            last_house = Some(house_state);
            if restyled {
                let spot = house::spot(&app, &own.world.work_areas, own.dpi);
                own.pet.set_house_door(spot.ground, spot.door_x);
            }
        }

        // 먹다가 눌린 동안(Pressed)도 먹는 중으로 봄 — 손을 떼면 계속 먹으므로 크기를 미리 바꾸지 않음.
        // 공중·축하 중에 받아 기억해 둔 먹이(간식 포함)가 성장을 안고 있어도 먹고 축하할 때까지 기다림
        if own.pet.stage_change_due(pending_stage) {
            pending_stage = false;
            apply_own_stage(&app, own, &care);
        }

        let hidden = crate::windows_hidden(&app);
        // 🛷 루돌프와 산타가 만나면 썰매로 한 바퀴 (탄 동안 둘의 창은 치워 둠)
        sleigh.tick(&app, dt, &mut slots, hidden || dodge.is_open());
        for s in &mut slots {
            if sleigh.is_riding(&s.label) {
                continue;
            }
            s.place(hidden);
            if let Some(render) = s.take_render(&app) {
                let _ = app.emit_to(s.label.as_str(), "pet-render", render);
                if s.is_own() {
                    // 하우스 창 문간도 내 동물의 먹기·축하를 보여 줌
                    let _ = app.emit_to(house::LABEL, "pet-render", render);
                }
            }
        }

        let wait = pacer.wait(Instant::now());
        if !wait.is_zero() {
            thread::sleep(wait);
        }
    }
}
