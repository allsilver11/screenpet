//! 게임 루프 스레드임: 16ms 틱마다 창 목록·커서를 읽고 동물 두뇌(코어)를 돌린 뒤 창을 놓음.
//! 웹뷰·트레이·네트워크에서 오는 것은 모두 [`Input`]으로 받아 이 스레드에서만 설정과 슬롯을 고침.
//! 진입점 `spawn`은 `main.rs`가 부르고, 효과들(날씨·물건·사다리 등)과 `network.rs`를 틱마다 돌림.
//! 🕸️ 거미집 자리 (trail.web_spots)는 틱마다 동물 뇌 (set_webs — 걸리기·🧗 거미집 놀이)와 물건 (objects.set_webs — 📦 걸려 매달림)에 같이 줌.
//! 🐾🐾🐾 내 펫은 slots 맨 앞 여럿 (최대 MAX_OWN_PETS, 첫 펫 = 대표 "pet" 창). 돌보기는 설정의 pets에 펫마다 있고,
//! 먹기·떠남/돌아옴·하우스 들어감·말풍선·먹이 조르기는 펫마다, 세계·화면 배율·커서 기준은 첫 펫 것을 씀.
//! 메뉴는 고른 펫 (AppState.picked) 하나를 다루고, 하우스·집중은 모두 한 번에.
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use screenpet_core::brain::control_moves::ControlInput;
use screenpet_core::brain::Pet;
use screenpet_core::care::{Care, FeedResult, Food, Stage};
use screenpet_core::config::{self, Config};
use screenpet_core::own_pets::MAX_OWN_PETS;
use screenpet_core::broom::Broom;
use screenpet_core::food::{self, FeedMode, HandFeed, HandInput};
use screenpet_core::geom::Rect;
use screenpet_core::linked;
use screenpet_core::manifest::FavoriteFood;
use screenpet_core::typing::{AskStep, PermissionAsk};
use screenpet_core::tint::BodyTint;
use screenpet_core::pacing::Pacer;
use screenpet_core::platform::{default_source, WindowSource};
use screenpet_core::presence::FocusLength;
use screenpet_core::social::Social;
use screenpet_core::visit::Side;
use screenpet_core::world::World;
use tauri::{AppHandle, Emitter, Manager};

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
/// 집으로 달려가는 동안 집 창이 정말 보이는지 OS에 묻는 간격 (틱 — 1초쯤)
const HOUSE_CHECK_TICKS: u32 = 60;
const WORLD_REFRESH: Duration = Duration::from_millis(100);
/// 설정을 저장하는 주기 (바뀌었을 때만)
const SAVE_EVERY: Duration = Duration::from_secs(60);
/// 한 틱에 넘기는 시간 상한 (초): 잠들었다 깬 뒤 큰 점프를 막음
const MAX_DT: f64 = 0.05;
/// 🌗 앱을 끌 때 바꿔 둔 테마를 되돌리기를 기다리는 시간 (못 되돌리면 다음 실행 때 표시 파일로)
const LUMOS_QUIT_WAIT: Duration = Duration::from_millis(1500);
/// 한 틱에 처리하는 입력 수의 상한 (틱 16ms면 초당 4000개, 친구 메시지 예산을 다 더한 것보다 훨씬 많음)
const MAX_INPUTS_PER_TICK: usize = 64;
/// slots[OWN] = 내 첫 펫 (대표 — 항상 맨 앞), 그 뒤로 내 다른 펫들, 그 뒤는 손님
const OWN: usize = 0;
/// macOS: 조작 모드를 켠 뒤 이만큼 동안 키가 한 번도 눌린 것으로 읽히지 않으면 입력 모니터링 권한 안내 (실행마다 한 번)
const KEY_PERMISSION_HINT_AFTER: Duration = Duration::from_secs(12);
/// 🍎 ⌨️ 타자 응원 권한(macOS 입력 모니터링)이 생겼는지 보는 간격 (켜져 있고 권한이 없을 때만 일함)
const TYPING_PERMISSION_CHECK: Duration = Duration::from_secs(2);
/// 이 환경 변수가 있으면 시작할 때 번들 동물 하나를 손님으로 들임 (네트워크 없이 여러 창 확인용)
const DEBUG_GUEST_ENV: &str = "SCREENPET_DEBUG_GUEST";

/// 게임 루프로 들어오는 입력임 (웹뷰 커맨드, 트레이 메뉴, 네트워크 이벤트)
pub enum Input {
    /// 창(label)의 동물을 눌렀음
    PointerDown { label: String },
    /// 동물 교체 — 돌보기 상태는 게임 루프가 가진 설정에서 찾음.
    /// remote = 📱 노는 내 다른 컴퓨터가 바꿨음 (다시 알리지 않고, 트레이 체크를 맞춤)
    SetPet { entry: PetEntry, remote: bool },
    /// 🎨 흐물이 몸 색 (설정에 남음) — 바로 칠해짐
    SetTint(BodyTint),
    /// 밥 주기. label = 음식을 고른 펫 창 (하우스 창이면 하우스 안의 펫), None = 트레이 (그릇이면 배고픈 펫 모두, 손이면 닿는 펫)
    Feed { food: Food, label: Option<String> },
    /// 🐾🐾🐾 펫 추가 (그 동물로 — 꽉 찼으면 아무 일 없음)
    AddPet(PetEntry),
    /// 🐾🐾🐾 고른 펫을 보내 줌 (마지막 한 마리·놀러 가 있는 펫은 못 뺌)
    RemovePet,
    /// 🐾🐾🐾 메뉴가 다룰 펫을 고름 (펫 창 라벨)
    PickPet(String),
    /// 🐾🐾🐾 "놀러 보내기 ▸ 친구 ▸ 펫": 그 펫(uid)을 그 친구에게
    SendPet { uid: String, friend: String },
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
    /// 🌗 루모스·녹스로 화면 밝기 바꾸기 켜기 / 끄기 (설정에 남음 — 끄면 바꿔 둔 테마를 바로 되돌림)
    ToggleLumosLight,
    /// 🌐 웹 페이지 위에도 올라가기 켜기 / 끄기 (설정에 남음 — 켜면 읽기 스레드를 띄우고 끄면 멈춤)
    ToggleWebLedges,
    /// 🚀 컴퓨터 켜면 자동 실행 켜기 / 끄기 (설정에 남음, 바로 OS에 적용)
    ToggleAutostart,
    /// 📊 처음 묻는 창의 답: 익명 통계를 보냄(true) / 안 보냄(false) — 설정에 남음 (pet_stats.rs)
    StatsConsent(bool),
    /// 📊 ⚙️ 설정 "익명 통계 보내기" 켜기 / 끄기
    ToggleStats,
    /// 📊 펫 통계 창 (개발자 컴퓨터만)
    ShowPetStats,
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
    /// 🚗 탈것 (메뉴·누르기·치우기 — vehicles.rs)
    Vehicle(crate::vehicles::VehicleInput),
    /// 메뉴 "도움말": 창 만들기는 메뉴 처리기 안에서 하면 Windows에서 멈춤 (Tauri 문서) — 게임 루프 스레드에서
    ShowHelp,
    /// 💌 개발자에게 건의하기 창
    ShowFeedback,
    /// 📮 개발자 건의함 창 (개발자 컴퓨터의 트레이 "📮 건의함", 편지를 든 펫 우클릭)
    ShowMailbox,
    /// 📮 건의함 보기 창 (다른 모든 컴퓨터의 트레이 "📮 건의함" — 읽기 전용 공개 목록)
    ShowBoard,
    /// 메뉴 "💩 똥피하기"
    ShowDodge,
    /// 똥피하기 창에서
    Dodge(crate::dodge::DodgeInput),
    /// 📏 크기 조절 창을 엶
    ShowSizes,
    /// 크기 조절 창의 값 (%). save = 슬라이더를 놓았음: 파일에 남김
    SetSizes { pet: u32, object: u32, save: bool },
    /// 👻 투명도 창을 엶
    ShowOpacity,
    /// 투명도 창의 값 (%). save = 막대를 놓았음: 파일에 남김
    SetOpacity { pct: u32, save: bool },
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

/// 게임 루프 스레드를 띄움. 루프가 패닉하면 앱을 끝냄. entry = 첫 펫의 동물, save_now = 시작하자마자 저장 (🐾🐾🐾 옛 설정을 펫 목록으로 옮겼음)
pub fn spawn(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config, save_now: bool) {
    thread::spawn(move || {
        let handle = app.clone();
        // 게임 루프가 죽으면 펫은 멈춘 채 남음 — 기록은 패닉 훅이 남겼으니 앱을 끝냄
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(app, rx, entry, config, save_now))).is_err() {
            handle.exit(1);
        }
    });
}

/// 📱 내 펫·돌보기 상태가 바뀌었음: 내 다른 컴퓨터와 나눔 (edit = 사용자가 바꿈 — 쉬는 중이면 이 컴퓨터로 놂).
/// 🐾🐾🐾 나누는 것은 첫 펫(대표)뿐임
fn share_care(network: &Network, config: &Config, edit: bool) {
    if let Some(lead) = config.lead_pet() {
        network.share_state(config, &lead.species, &lead.care, edit);
    }
}

/// 📊 오늘 날짜를 맞추고 지금 키우는 종류를 적은 뒤, 보낼 지난날 보고를 net에 넘김. 설정이 바뀌었으면 true
fn stats_tick(config: &mut Config, network: &Network) -> bool {
    let species: Vec<String> = config.pets.iter().map(|p| p.species.clone()).collect();
    let species: Vec<&str> = species.iter().map(String::as_str).collect();
    let dirty = config.pet_stats.note_kept(&crate::pet_stats::today(), crate::pet_stats::VERSION, &species);
    network.set_stats_report(config.pet_stats.report().cloned());
    dirty
}

/// 📊 익명 통계 동의를 적음: ⚙️ 설정 체크를 맞추고, 새로 설치해 고른 첫 펫을 아직 못 셌으면 (답이 다음 실행에 왔어도) "새로 고름"으로 셈. 설정이 바뀌었으면 true
fn set_stats_consent(app: &AppHandle, config: &mut Config, network: &Network, yes: bool) -> bool {
    let species: Vec<String> = config.pets.iter().map(|p| p.species.clone()).collect();
    let species: Vec<&str> = species.iter().map(String::as_str).collect();
    let changed = config.pet_stats.answer(yes, &crate::pet_stats::today(), crate::pet_stats::VERSION, &species);
    let _ = app.state::<AppState>().menu.pet_stats.set_checked(config.pet_stats.on());
    screenpet_log::log!(Info, "stats", "stats_consent", "on" => u8::from(yes));
    stats_tick(config, network);
    changed
}

/// 🐾🐾🐾 내 펫 슬롯 수 (slots 맨 앞 — 그 뒤는 손님)
fn own_count(slots: &[Slot]) -> usize {
    slots.iter().take_while(|s| s.is_own()).count()
}

use crate::slots::own_refuses;

/// 🐾🐾🐾 새 펫의 uid (설치 안에서만 씀)
pub fn new_pet_uid() -> String {
    format!("{:016x}", fastrand::u64(..))
}

/// 🐾🐾🐾 이 펫의 돌보기 상태 (설정의 pets에서 uid로 — 없으면 새끼)
fn care_of(config: &Config, s: &Slot) -> Care {
    s.uid().and_then(|u| config.own_pet(u)).map(|p| p.care.clone()).unwrap_or_default()
}

/// 🐾🐾🐾 이 펫의 돌보기 상태를 설정에 적음
fn set_care(config: &mut Config, s: &Slot, care: Care) {
    if let Some(p) = s.uid().and_then(|u| config.own_pet_mut(u)) {
        p.care = care;
    }
}

/// 🐾🐾🐾 메뉴가 다루는 펫의 슬롯 번호 (AppState.picked — 없으면 첫 펫)
fn picked(app: &AppHandle, slots: &[Slot]) -> usize {
    let label = app.state::<AppState>().picked.lock().unwrap().clone();
    slots.iter().take_while(|s| s.is_own()).position(|s| s.label == label).unwrap_or(OWN)
}

/// 🐾🐾🐾 그릇 밥을 받을 펫들: 음식을 고른 펫 창이면 그 펫, 하우스 창이면 하우스 안의 배고픈 펫 (없으면 하우스 안 첫 펫),
/// 트레이면 친구 화면에 가 있지 않은 배고픈 펫 모두 (아무도 안 고프면 고른 펫 — 배불러 갸웃하거나 간식은 먹음)
fn feed_targets(app: &AppHandle, slots: &[Slot], config: &Config, label: Option<&str>) -> Vec<usize> {
    let n = own_count(slots);
    let home = |i: &usize| !slots[*i].pet.is_away();
    let hungry = |i: &usize| care_of(config, &slots[*i]).is_hungry();
    match label {
        Some(l) if l == house::LABEL => {
            let inside: Vec<usize> = (0..n).filter(|&i| slots[i].pet.is_housed()).collect();
            inside.iter().copied().find(|i| hungry(i)).or(inside.first().copied()).into_iter().collect()
        }
        Some(l) => slots[..n].iter().position(|s| s.label == l).into_iter().collect(),
        None => {
            let all: Vec<usize> = (0..n).filter(home).filter(hungry).collect();
            if all.is_empty() {
                vec![picked(app, slots)]
            } else {
                all
            }
        }
    }
}

/// ⌨️ 타자 응원을 받는 동물: 내 흐물이 (손님 흐물이는 아님)
fn is_own_heumul(s: &Slot) -> bool {
    s.is_own() && s.entry.id == "heumul"
}

/// 설정 저장 — 게임 루프만 설정을 씀 (단일 작성자). 하우스 여부는 config.housed에 직접 적어 둠.
/// 🐾🐾🐾 돌보기는 펫마다 config.pets에 있고, 옛 필드(pet·care[첫 펫])도 첫 펫으로 맞춰 씀 (옛 버전으로 되돌려도 첫 펫이 그대로)
fn save_config(app: &AppHandle, config: &mut Config) {
    config.sync_legacy();
    let state = app.state::<AppState>();
    if let Err(e) = config::save(&state.config_dir, config) {
        eprintln!("failed to save config: {e}");
    }
}

/// 🐾🐾🐾 메뉴용 내 펫들의 지금 모습 (바뀌면 AppState.own에 적고 🐾 내 펫 ▸ 을 고른 펫에 맞춤)
fn own_views(slots: &[Slot], config: &Config, network: &Network) -> Vec<crate::tray::OwnView> {
    let n = own_count(slots);
    slots[..n]
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let care = care_of(config, s);
            crate::tray::OwnView {
                label: s.label.clone(),
                uid: s.uid().unwrap_or_default().to_string(),
                species: s.entry.id.clone(),
                name: crate::tray::own_pet_name(i, n, &s.entry.manifest.name),
                stage: care.stage,
                locked: care.locked,
                housed: s.pet.is_housed(),
                away: s.pet.is_away(),
                busy: network.own_busy(s),
            }
        })
        .collect()
}

/// 지금 내 동물의 말풍선·음식 고르기를 띄울 창: 하우스 안이면 하우스 창, 아니면 그 펫 창
pub(crate) fn speech_target(s: &Slot) -> &str {
    if s.pet.is_housed() {
        house::LABEL
    } else {
        s.label.as_str()
    }
}

/// 내 동물의 단계(또는 동물)가 바뀌었을 때: 창 크기, 펫 크기, 웹뷰 시트
fn apply_own_stage(app: &AppHandle, own: &mut Slot, stage: Stage) {
    let payload = slots::stage_payload(&own.entry, stage, own.size());
    own.set_stage(app, payload);
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

/// 사이 틱: 동물(내 동물·손님)이 서 있거나 매달린 창의 경계만 새로 읽어 세계에 고쳐 넣음 —
/// 창을 끌어 옮기면 100 ms 계단이 아니라 매 틱 따라감. 기대고 있는 창이 없으면 아무것도 읽지 않음
fn follow_ridden_windows(source: &mut dyn WindowSource, slots: &mut [Slot]) {
    let mut ids: Vec<u64> = slots.iter().filter_map(|s| s.pet.ridden_window()).collect();
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

/// 🌐 설정이 켜져 있으면 웹 페이지 디딤돌 읽기 스레드를 띄움 (꺼져 있거나 읽을 수 없는 OS면 None — 스레드도 없음)
fn start_web_ledges(config: &Config) -> Option<screenpet_core::ledge::LedgeProbe> {
    if config.web_ledges_on {
        screenpet_core::platform::web_ledge_probe()
    } else {
        None
    }
}

/// 하우스 안, 집으로 가는 중, 또는 하우스에서 나와 먹는 중(다 먹으면 들어갈 참)
fn in_or_going_home(pet: &Pet) -> bool {
    pet.is_housed() || pet.is_going_home() || pet.returns_home()
}

/// 하우스에서 꺼냄: 문 앞에서 걸어 나옴 (가는 중이었다면 그 자리에서 멈춤). 집 창은 hide_house_if_empty가 다룸
fn leave_house_now(app: &AppHandle, own: &mut Slot) {
    let spot = house::spot(app, &own.world.work_areas, own.dpi);
    let was_inside = own.pet.is_housed();
    own.pet.leave_house(&own.world, spot.door_x, spot.ground);
    if was_inside {
        own.need_show = true;
    }
}

/// 🐾🐾🐾 하우스 안이거나 하우스로 가는 내 펫이 하나도 없으면 집 창을 숨김 (한 마리만 꺼내 보내도 남은 펫들의 집은 그대로)
fn hide_house_if_empty(slots: &[Slot], house_window: Option<&tauri::WebviewWindow>) {
    if slots.iter().take_while(|s| s.is_own()).any(|s| in_or_going_home(&s.pet)) {
        return;
    }
    if let Some(hw) = house_window {
        crate::overlay::hide_quietly(hw);
    }
}

/// 🐾🐾🐾 창을 못 만든 펫을 다시 만들어 보는 간격
const SPAWN_RETRY: Duration = Duration::from_secs(5);

/// 🐾🐾🐾 설정에는 있는데 창(슬롯)이 없는 펫의 창을 만듦 (시작할 때, 그리고 못 만든 펫은 SPAWN_RETRY마다 다시).
/// 설정 차례대로 내 펫 슬롯 끝 (손님 앞)에 넣음. 하나라도 새로 만들었으면 true
fn spawn_missing_pets(app: &AppHandle, slots: &mut Vec<Slot>, config: &Config) -> bool {
    let mut made = false;
    for (i, p) in config.pets.iter().enumerate() {
        if slots.iter().take_while(|s| s.is_own()).any(|s| s.uid() == Some(p.uid.as_str())) {
            continue;
        }
        let Some(entry) = app.state::<AppState>().pets.iter().find(|e| e.id == p.species).cloned() else { continue };
        let label = slots::free_own_label(app, slots);
        let x = 0.5 + 0.18 * i as f64 * if i % 2 == 0 { 1.0 } else { -1.0 };
        let n = own_count(slots);
        match slots::spawn_own(app, label, p.uid.clone(), entry, p.care.stage, config.pet_size(), slots[OWN].dpi, &slots[OWN].world, x, config.tint) {
            Ok(mut slot) => {
                slot.pet.set_hungry(p.care.is_hungry());
                slot.track.locked = p.care.locked;
                slots.insert(n, slot);
                made = true;
            }
            Err(e) => eprintln!("failed to create a pet window (will retry): {e}"),
        }
    }
    made
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
    let guest = GuestInfo { visit_id: "debug".into(), owner_id: "debug".into(), owner_name: "디버그 친구".into(), note: None, can_grow: true, tint: BodyTint::Default, pet: 0 };
    match slots::spawn_guest(app, guest, entry, stage, scale, slots[OWN].dpi, Side::Right, &slots[OWN].world, seed_now(), slots[OWN].size()) {
        Ok(slot) => slots.push(slot),
        Err(e) => eprintln!("failed to create debug guest window: {e}"),
    }
}

/// 게임 루프 본체임: 상태를 만들고 끝날 때까지 틱을 돌림
fn run(app: AppHandle, rx: Receiver<Input>, entry: PetEntry, config: Config, save_now: bool) {
    let Some(window) = app.get_webview_window(OWN_LABEL) else {
        eprintln!("pet window not found; game loop not started");
        return;
    };
    let house_window = app.get_webview_window(house::LABEL);
    let mut config = config;
    house::restore(config.house_spot.clone());
    let Some(lead) = config.lead_pet().cloned() else {
        eprintln!("no pet in the config; game loop not started");
        return;
    };
    let care = lead.care.clone();
    let dpi = window.scale_factor().unwrap_or(1.0);
    let mut source = default_source();

    let shown = slots::stage_payload(&entry, care.stage, config.pet_size());
    let cfg = slots::pet_config(dpi, &entry, shown.scale, shown.stage);
    let world = World::new(source.windows(), source.work_areas(), cfg.size);
    let b = world.bounds;
    let pet = Pet::new(cfg, b.left + b.width() / 2.0, b.top + cfg.size, seed_now());
    let mut slots = vec![Slot::new(OWN_LABEL.to_string(), Role::Own(lead.uid.clone()), entry, pet, window, world, dpi, shown.clone(), config.pet_size())];
    slots[OWN].tint = config.tint;
    crate::tray::sync_tint(&app.state::<AppState>().menu, config.tint);
    // 창 크기·웹뷰 시트를 맞춤
    slots[OWN].set_stage(&app, shown);
    // 🐾🐾🐾 둘째·셋째 펫 창 (화면 위에서 조금씩 떨어진 자리로 떨어지며 나타남 — 못 만들면 틱마다 가끔 다시)
    spawn_missing_pets(&app, &mut slots, &config);
    let mut next_spawn_retry = Instant::now() + SPAWN_RETRY;
    for i in 0..own_count(&slots) {
        let care = care_of(&config, &slots[i]);
        slots[i].pet.set_hungry(care.is_hungry());
        slots[i].track.locked = care.locked;
    }
    if config.housed {
        // 지난번에 하우스 안에서 끝났음 — 하우스 안에서 시작함 (🐾🐾🐾 모두)
        let spot = house::spot(&app, &slots[OWN].world.work_areas, slots[OWN].dpi);
        for own in slots.iter_mut().filter(|s| s.is_own()) {
            // 몸은 문 앞 바닥에 둠 (화면 위에서 떨어지던 자리가 아니라)
            own.pet.body.x = spot.door_x;
            if let Some((_, ay)) = own.world.anchor(spot.ground) {
                own.pet.body.y = ay;
            }
            own.pet.enter_house();
            own.track.was_housed = true;
        }
        if let Some(hw) = &house_window {
            house::show_at(hw, &spot);
        }
    }
    // 🐾🐾🐾 메뉴(고른 펫·하우스 글자)는 틱 끝에서 펫들의 모습이 바뀌었거나 이것이 켜져 있으면 다시 맞춤
    let mut menu_dirty = true;
    let mut last_views: Vec<crate::tray::OwnView> = Vec::new();
    let _ = app.state::<AppState>().menu.cursor_play.set_checked(!config.cursor_play_off);
    let _ = app.state::<AppState>().menu.typing_cheer.set_checked(!config.typing_cheer_off);
    let _ = app.state::<AppState>().menu.poop.set_checked(!config.poop_off);
    let _ = app.state::<AppState>().menu.lumos_light.set_checked(config.lumos_light);
    let _ = app.state::<AppState>().menu.web_ledges.set_checked(config.web_ledges_on);
    // 🌐 웹 페이지 디딤돌 읽기 스레드 (켜 둔 때만 — 꺼져 있으면 아무 일도 안 함)
    let mut web_ledges = start_web_ledges(&config);
    // 🚀 이 버전을 처음 켜면 자동 실행을 켜고 적어 둠. 그 뒤로는 고른 대로 OS에 다시 맞춤
    if crate::autostart::apply_at_start(&app, &mut config) || save_now {
        save_config(&app, &mut config);
    }
    // 🏠 집으로 달려가는 동안 집이 보이는지 OS에 묻는 틱 세기 (HOUSE_CHECK_TICKS마다 한 번)
    let mut house_checks: u32 = 0;
    let mut last_house: Option<HouseState> = None;
    if std::env::var_os(DEBUG_GUEST_ENV).is_some() {
        spawn_debug_guest(&app, &mut slots);
    }
    let (mut network, new_id) = Network::start(&app, &mut config);
    if new_id {
        save_config(&app, &mut config);
    }
    // 📊 익명 통계: 처음 설치한 실행이면 첫 펫을 "새로 고름"으로 셀 수 있게 설정에 적어 둠 (답이 다음 실행에 와도).
    // 아직 답이 없으면 한 번 물음 (답하기 전엔 안 모으고 안 보냄)
    if new_id {
        config.pet_stats.mark_first_pick();
        save_config(&app, &mut config);
    }
    let _ = app.state::<AppState>().menu.pet_stats.set_checked(config.pet_stats.on());
    if stats_tick(&mut config, &network) {
        save_config(&app, &mut config);
    }
    if config.pet_stats.consent.is_none() {
        crate::pet_stats::show_ask(&app);
    }
    share_care(&network, &config, false);
    // ☀️ 실제 날씨: 중계 서버에 고른 도시를 알림 (붙을 때마다 다시 — 옛 서버엔 보내지 않음)
    network.want_weather(config.weather_place);

    let mut last = Instant::now();
    let mut last_refresh = last;
    // 틱은 정해진 시각마다 (일한 시간만큼 늦어지지 않게)
    let mut pacer = Pacer::new(TICK, last);
    let mut last_save = last;
    // 내 동물이 먹을 때 앞에 놓이는 밥그릇·물그릇·쿠키 창 (🐾🐾🐾 펫 uid마다 하나)
    let mut props: std::collections::HashMap<String, Props> = std::collections::HashMap::new();
    // 직접 주기: 커서 끝에 붙은 음식과 그 그림 창
    let mut hand: Option<HandFeed> = None;
    let mut hand_win = HandWindow::default();
    let mut broom: Option<Broom> = None;
    // ⛄ 쌓인 눈 위에서 커서를 좌우로 빠르게 문지르는지 봄 (코어 snowball.rs)
    let mut rub = screenpet_core::snowball::Rub::new();
    let mut broom_win = HandWindow::default();
    // 계절 날씨 (철이 아니거나 꺼 두면 창도 일도 없음)
    let mut weather = WeatherFx::new(seed_now());
    let _ = app.state::<AppState>().menu.weather.set_checked(config.weather_on);
    weather.set_source(config.weather_source);
    crate::tray::sync_weather_kind(&app.state::<AppState>().menu, config.weather_source, config.weather_place);
    // 🌈 무지개 구름 자국 (무지개 동물이 달릴 때만 창이 생김)
    let mut trail = crate::trail::TrailFx::new();
    // 🌗 루모스·녹스로 화면 밝기 바꾸기 일꾼 (테마를 바꿀 수 있는 OS만 — 켜자마자 지난번에 못 되돌린 테마부터 되돌림, 코어 lumos.rs)
    let lumos = screenpet_core::platform::theme_backend().and_then(|b| {
        let marker = app.state::<AppState>().config_dir.join(screenpet_core::lumos::MARKER_FILE);
        screenpet_core::lumos::spawn(b, marker, config.lumos_light)
    });
    // ⌨️ 타자 응원: 최근 2초의 키 누름 *개수* (어떤 키인지는 읽지 않음)
    let mut typing_meter = screenpet_core::typing::TypingMeter::new();
    // 🍎 macOS 입력 모니터링 권한 (Windows는 None — 묻지도 않음). 없으면 이유를 말한 뒤 버전마다 한 번 물음
    let mut typing_permission = source.typing_permission();
    let mut typing_ask = PermissionAsk::start(Instant::now(), !config.typing_cheer_off, typing_permission, config.input_monitoring_asked.as_deref(), env!("CARGO_PKG_VERSION"));
    let mut next_permission_check = Instant::now() + TYPING_PERMISSION_CHECK;
    crate::tray::sync_typing_permission(&app.state::<AppState>().menu, !config.typing_cheer_off && typing_permission == Some(false));
    // 자리 비움·돌아옴, 하루 일과, ⏱️ 집중 친구 (1초에 한 번)
    let mut presence = Presence::new(&app, config.focus_length);
    // 물건 (쓰레기 장난감 — 없거나 잠들어 있으면 일이 없음)
    let mut objects = Objects::new();
    // 🚗 탈것 (vehicles.rs — 코어 vehicle.rs Garage)
    let mut vehicles = crate::vehicles::Vehicles::new();
    let mut vehicle_spots: Vec<screenpet_core::vehicle::VehicleSpot> = Vec::new();
    let mut dodge = crate::dodge::DodgeFx::new();
    let mut sleigh = crate::sleigh::SleighFx::new();
    objects.set_size(&app, config.object_size());
    // 📦 창이 오기 전 물건의 화면 배율 (창은 일꾼 스레드가 만듦 — 오면 refresh가 고침)
    objects.set_dpi_hint(slots[OWN].dpi);
    // 🕐 최근 한 시간 (메모리에만)
    let mut timeline = TimelineFx::new();
    // 🔄 새 버전 응원 (버전마다 한 번)
    let mut cheer_gate = screenpet_core::update::CheerGate::default();
    // 💩 왼쪽 버튼을 새로 누른 순간만 씀 (어디를 무엇을 눌렀는지는 보지도 남기지도 않음)
    let mut was_left_down = false;
    let mut poop_spots = Vec::new();
    // 🤹 흐물이가 주울 물건 자리, 노릴 동물 자리 (틱마다 다시 채움)
    let mut object_spots = Vec::new();
    // 🧳 포트키가 될 수 있는 놓인 물건 (틱마다 다시 채움 — 모두 숨겨 둔 동안엔 빈 목록)
    let mut portkey_spots = Vec::new();
    let mut pet_targets: Vec<Option<screenpet_core::juggle::Target>> = Vec::new();
    let mut juggle_targets = Vec::new();
    // ⚾ 친구 화면에서 날아 들어오는 물건 (흐물이가 받으러 감)
    let mut incoming = Vec::new();
    // 한 화면의 동물끼리(내 동물 + 손님) 인사·술래 놀이
    let mut social = Social::new(seed_now());
    // 🎮 조작 모드: 켜진 동안만 키를 읽음 (읽기만 — 앞에 있는 창에도 그대로 감). 🐾🐾🐾 조종하는 펫 (창 라벨)
    let mut control_on = false;
    let mut control_label = OWN_LABEL.to_string();
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
                        // 🐾🐾🐾 누른 내 펫이 메뉴의 고른 펫이 됨
                        if slots[i].is_own() {
                            *app.state::<AppState>().picked.lock().unwrap() = label;
                            menu_dirty = true;
                        }
                    }
                }
                Input::SetPet { entry: next, remote } => {
                    // 🐾🐾🐾 고른 펫의 종류를 바꿈 (📱 내 다른 컴퓨터가 바꿨으면 첫 펫 — 그쪽도 첫 펫만 나눔)
                    let i = if remote { OWN } else { picked(&app, &slots) };
                    let own = &mut slots[i];
                    // 친구 화면에 가 있거나 가는 중엔 바꾸지 않음 (트레이 체크만 되돌림)
                    if network.own_busy(own) {
                        crate::tray::sync_pet_checks(&app.state::<AppState>().menu, &own.entry.id);
                        continue;
                    }
                    // 들고 있던 음식은 거둠 (좋아하는 음식 그림이 동물마다 다름)
                    if hand.take().is_some() {
                        for s in slots.iter_mut().filter(|s| s.is_own()) {
                            s.pet.end_hand();
                        }
                        hand_win.drop_back();
                        app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
                    }
                    let own = &mut slots[i];
                    let uid = own.uid().unwrap_or_default().to_string();
                    let before = own.entry.id.clone();
                    // 📊 이 컴퓨터에서 바꾼 것만 셈 (📱 다른 컴퓨터가 바꾼 것은 그쪽이 셈)
                    if config.set_pet_species(&uid, &next.id) && !remote {
                        config.pet_stats.note_changed(&crate::pet_stats::today(), crate::pet_stats::VERSION, &before, &next.id);
                    }
                    // 펫이 키운 만큼은 그대로 (📱 저쪽에서 바꿨으면 저쪽이 나눈 그 동물의 기록으로 — 첫 펫만 나눔)
                    if remote {
                        if let Some(shared) = config.care.get(&next.id).cloned() {
                            set_care(&mut config, &slots[i], shared);
                        }
                    }
                    let care = care_of(&config, &slots[i]);
                    let own = &mut slots[i];
                    own.entry = next;
                    // 일시 확대는 그 동물 것임
                    own.boost = 1.0;
                    own.track.pending_stage = false;
                    apply_own_stage(&app, own, care.stage);
                    // 이전 동물의 종 전용 동작·벽 타기가 새 동물에 없을 수 있으니 진행 중이면 끝냄
                    own.pet.set_special(own.pet.cfg.special);
                    own.pet.set_can_climb(own.pet.cfg.can_climb);
                    // 만들던 눈사람도 — 눈덩이 창은 다음 틱에 weather가 치움 (새 동물이 날면 다시는 끝내지 못함)
                    own.pet.end_snowman();
                    own.pet.set_hungry(care.is_hungry());
                    // 하우스 안이면 새 동물도 하우스 안에 있음 (같은 Pet이므로 상태가 그대로 이어짐)
                    menu_dirty = true;
                    save_config(&app, &mut config);
                    if !remote {
                        share_care(&network, &config, true);
                    } else {
                        // 📱 저쪽에서 바꿨음: 트레이 체크도 맞춤 (누른 항목만 스스로 뒤집힘)
                        crate::tray::sync_pet_checks(&app.state::<AppState>().menu, &slots[i].entry.id);
                    }
                }
                Input::AddPet(entry) => {
                    // 🐾🐾🐾 꽉 찼으면 아무 일 없음 (메뉴도 흐림)
                    let n = own_count(&slots);
                    if n >= MAX_OWN_PETS {
                        continue;
                    }
                    let uid = new_pet_uid();
                    if config.add_pet(&entry.id, uid.clone()).is_none() {
                        continue;
                    }
                    // 📊 새로 고른 펫 (창을 못 만들어 되돌려도 고른 것은 고른 것)
                    config.pet_stats.note_chosen(&crate::pet_stats::today(), crate::pet_stats::VERSION, &entry.id);
                    let stage = config.own_pet(&uid).map_or(Stage::Baby, |p| p.care.stage);
                    let label = slots::free_own_label(&app, &slots);
                    let x = 0.5 + 0.18 * n as f64 * if n % 2 == 0 { 1.0 } else { -1.0 };
                    match slots::spawn_own(&app, label.clone(), uid.clone(), entry, stage, config.pet_size(), slots[OWN].dpi, &slots[OWN].world, x, config.tint) {
                        Ok(mut slot) => {
                            slot.pet.set_hungry(care_of(&config, &slot).is_hungry());
                            // 집중 중이면 새 펫도 같이 집중
                            if slots[OWN].pet.is_focusing() {
                                slot.pet.set_focus(true);
                            }
                            slots.insert(n, slot);
                            *app.state::<AppState>().picked.lock().unwrap() = label;
                            save_config(&app, &mut config);
                        }
                        Err(e) => {
                            eprintln!("failed to create a pet window: {e}");
                            config.remove_pet(&uid);
                        }
                    }
                    menu_dirty = true;
                }
                Input::RemovePet => {
                    // 🐾🐾🐾 고른 펫을 보내 줌: 마지막 한 마리·친구 화면에 가 있거나 가는 펫은 못 뺌 (메뉴도 흐림)
                    let n = own_count(&slots);
                    let i = picked(&app, &slots);
                    // 들렸거나 탈것·썰매에 탔거나 물건을 든 펫도 (창·물건이 주인을 잃음). 첫 펫이면 창을 물려받을 다음 펫도 썰매에 타고 있으면 안 됨
                    let tied = |s: &Slot| s.pet.is_held() || s.pet.riding().is_some() || s.pet.boarding().is_some() || s.pet.carried_object().is_some() || sleigh.is_riding(&s.label);
                    if n <= 1 || network.own_busy(&slots[i]) || tied(&slots[i]) || (i == OWN && sleigh.is_riding(&slots[1].label)) {
                        menu_dirty = true;
                        continue;
                    }
                    let uid = slots[i].uid().unwrap_or_default().to_string();
                    if !config.remove_pet(&uid) {
                        continue;
                    }
                    if hand.take().is_some() {
                        for s in slots.iter_mut().filter(|s| s.is_own()) {
                            s.pet.end_hand();
                        }
                        hand_win.drop_back();
                        app.state::<AppState>().hand_feeding.store(false, Ordering::Relaxed);
                    }
                    if control_on && slots[i].label == control_label {
                        control_on = false;
                        slots[i].pet.set_control(false);
                        let _ = app.state::<AppState>().menu.control.set_checked(false);
                    }
                    // 첫 펫이면 다음 펫이 "pet" 창을 물려받음 (첫 펫 창은 앱 내내 있음)
                    if i == OWN {
                        let (a, b) = slots.split_at_mut(1);
                        // 조종하던 다음 펫은 "pet" 창으로 옮겨 가도 계속 조종됨
                        if control_label == b[0].label {
                            control_label = a[0].label.clone();
                        }
                        a[0].swap_window(&mut b[0]);
                        let info = slots::stage_payload(&b[0].entry, b[0].shown.stage, b[0].size());
                        b[0].set_stage(&app, info);
                    }
                    if let Some(mut p) = props.remove(&uid) {
                        p.apply(&app, screenpet_core::food::PropEvent::Hide, slots[i].dpi, slots[i].shown.scale);
                        p.tick(Instant::now() + Duration::from_secs(1));
                    }
                    let gone = slots.remove(i);
                    slots::close(&app, gone);
                    *app.state::<AppState>().picked.lock().unwrap() = OWN_LABEL.to_string();
                    menu_dirty = true;
                    save_config(&app, &mut config);
                }
                Input::PickPet(label) => {
                    if slots::find(&slots, &label).is_some_and(|i| slots[i].is_own()) {
                        *app.state::<AppState>().picked.lock().unwrap() = label;
                        menu_dirty = true;
                    }
                }
                Input::SetTint(tint) => {
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 다시 맞춤
                    crate::tray::sync_tint(&app.state::<AppState>().menu, tint);
                    if tint == config.tint {
                        continue;
                    }
                    config.tint = tint;
                    for own in slots.iter_mut().filter(|s| s.is_own()) {
                        own.set_tint(&app, tint);
                    }
                    save_config(&app, &mut config);
                }
                Input::Feed { food, label } => {
                    let n = own_count(&slots);
                    // 직접 주기: 음식이 커서 끝에 붙음 (먹이기는 닿는 펫 입에 닿을 때). 모두 하우스 안·친구 화면이면 그릇으로
                    if config.feed_mode.by_hand(food) && slots[..n].iter().any(|s| s.on_screen()) && label.as_deref() != Some(house::LABEL) {
                        if hand.is_some() {
                            for s in slots[..n].iter_mut() {
                                s.pet.end_hand();
                            }
                        }
                        hand = Some(HandFeed::new(food));
                        let first = slots[..n].iter().position(|s| s.on_screen()).unwrap_or(OWN);
                        show_hand(&app, &mut hand_win, food, source.cursor(), &slots[first]);
                        app.state::<AppState>().hand_feeding.store(true, Ordering::Relaxed);
                        for s in &slots[..n] {
                            let _ = app.emit_to(s.label.as_str(), "hide-picker", ());
                        }
                        continue;
                    }
                    // 🐾🐾🐾 그릇: 배고픈 펫 모두가 먹으러 옴 (펫마다 그릇 하나)
                    let mut ate_any = false;
                    for i in feed_targets(&app, &slots, &config, label.as_deref()) {
                        if slots[i].pet.is_housed() {
                            // 하우스 안이면 문 앞에 그릇이 놓이고 동물이 나와서 먹음
                            let spot = house::spot(&app, &slots[i].world.work_areas, slots[i].dpi);
                            slots[i].pet.set_house_door(spot.ground, spot.door_x);
                        }
                        let mut care = care_of(&config, &slots[i]);
                        let before = care.clone();
                        let result = care.feed(food);
                        let own = &mut slots[i];
                        if own.pet.feed_result(result, food) {
                            if let FeedResult::Ate { leveled_up } = result {
                                own.track.pending_stage |= leveled_up;
                                timeline.ate(&app, food, &favorite_of(&own.entry), &config);
                            }
                            ate_any = true;
                        } else {
                            // 공중·드래그·놀러 가는 중이라 못 먹었음 — 먹지 않은 것으로 되돌림
                            care = before;
                        }
                        // 먹는 도중 클릭이 다시 음식을 조르지 않도록 바로 반영
                        own.pet.set_hungry(care.is_hungry());
                        set_care(&mut config, &slots[i], care);
                    }
                    if ate_any {
                        save_config(&app, &mut config);
                        share_care(&network, &config, true);
                    }
                }
                Input::SetFeedMode(mode) => {
                    config.feed_mode = mode;
                    crate::tray::sync_feed_mode(&app.state::<AppState>().menu, mode);
                    save_config(&app, &mut config);
                }
                Input::ToggleLock => {
                    // 🐾🐾🐾 고른 펫
                    let i = picked(&app, &slots);
                    let mut care = care_of(&config, &slots[i]);
                    care.locked = !care.locked;
                    // 멈추면 바로 조르기를 그침 (풀면 배고픈 만큼 다시 조름)
                    slots[i].pet.set_hungry(care.is_hungry());
                    slots[i].track.locked = care.locked;
                    set_care(&mut config, &slots[i], care);
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    menu_dirty = true;
                    save_config(&app, &mut config);
                    share_care(&network, &config, true);
                }
                Input::ResetGrowth => {
                    // 🐾🐾🐾 고른 펫
                    let i = picked(&app, &slots);
                    let mut care = care_of(&config, &slots[i]);
                    care.reset();
                    let own = &mut slots[i];
                    own.track.pending_stage = false;
                    apply_own_stage(&app, own, care.stage);
                    own.pet.set_hungry(care.is_hungry());
                    set_care(&mut config, &slots[i], care);
                    menu_dirty = true;
                    save_config(&app, &mut config);
                    share_care(&network, &config, true);
                }
                Input::ToggleHouse => {
                    // 🐾🐾🐾 모두 한 번에: 하우스 모드면 모두 나오고, 아니면 모두 들어감 (친구 화면에 가 있거나 가는 펫은 빼고)
                    let n = own_count(&slots);
                    let home: Vec<usize> = (0..n).filter(|&i| !network.own_busy(&slots[i])).collect();
                    menu_dirty = true;
                    if home.is_empty() {
                        continue;
                    }
                    let spot = house::spot(&app, &slots[OWN].world.work_areas, slots[OWN].dpi);
                    if config.housed {
                        for i in 0..n {
                            if home.contains(&i) && in_or_going_home(&slots[i].pet) {
                                leave_house_now(&app, &mut slots[i]);
                            }
                        }
                        hide_house_if_empty(&slots, house_window.as_ref());
                        config.housed = false;
                    } else {
                        // 집이 먼저 나타나고, 동물들이 집으로 달려감 (모두 숨겨 뒀으면 보일 때 나타남)
                        if let Some(hw) = house_window.as_ref().filter(|_| !crate::windows_hidden(&app)) {
                            house::show_at(hw, &spot);
                        }
                        for i in 0..n {
                            if home.contains(&i) {
                                slots[i].pet.go_home(spot.ground, spot.door_x);
                                // 펫 창에 떠 있던 음식 고르기는 닫음
                                let _ = app.emit_to(slots[i].label.as_str(), "hide-picker", ());
                            }
                        }
                        config.housed = true;
                    }
                    save_config(&app, &mut config);
                }
                // 똥피하기 창이 열려 있으면 조작 모드는 켜지 않음 (게임이 키를 줌)
                Input::ToggleControl if dodge.is_open() => {
                    let _ = app.state::<AppState>().menu.control.set_checked(false);
                }
                Input::ToggleControl => {
                    control_on = !control_on;
                    // 🐾🐾🐾 고른 펫을 조종함 (끌 때는 조종하던 펫)
                    if control_on {
                        control_label = slots[picked(&app, &slots)].label.clone();
                    }
                    if let Some(i) = slots::find(&slots, &control_label) {
                        slots[i].pet.set_control(control_on);
                    }
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
                    save_config(&app, &mut config);
                }
                Input::SetWeatherSource(source) => {
                    crate::tray::sync_weather_kind(&app.state::<AppState>().menu, source, config.weather_place);
                    // 날씨를 직접 골랐으면 꺼져 있던 🌨️ 날씨 효과도 켬 (고른 걸 바로 보게)
                    let turn_on = source.manual().is_some() && !config.weather_on;
                    if source == config.weather_source && !turn_on {
                        continue;
                    }
                    if turn_on {
                        config.weather_on = true;
                        weather.sync_menu(&app, false);
                    }
                    config.weather_source = source;
                    weather.set_source(source);
                    save_config(&app, &mut config);
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
                    save_config(&app, &mut config);
                }
                // 🐾🐾🐾 집중은 내 펫 모두 한 번에
                Input::ToggleFocus => {
                    let n = own_count(&slots);
                    presence.toggle(&app, config.focus_length, &mut slots[..n]);
                }
                Input::SetFocusLength(length) => {
                    config.focus_length = length;
                    let n = own_count(&slots);
                    presence.set_length(&app, length, &mut slots[..n]);
                    save_config(&app, &mut config);
                }
                Input::ToggleCursorPlay => {
                    config.cursor_play_off = !config.cursor_play_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.cursor_play.set_checked(!config.cursor_play_off);
                    save_config(&app, &mut config);
                }
                Input::ToggleTypingCheer => {
                    config.typing_cheer_off = !config.typing_cheer_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.typing_cheer.set_checked(!config.typing_cheer_off);
                    // 🍎 다시 켜면 (권한이 없을 때) 또 물음, 끄면 기다리던 묻기도 취소
                    typing_permission = source.typing_permission();
                    typing_ask.toggled(Instant::now(), !config.typing_cheer_off, typing_permission);
                    crate::tray::sync_typing_permission(&app.state::<AppState>().menu, !config.typing_cheer_off && typing_permission == Some(false));
                    save_config(&app, &mut config);
                }
                Input::TogglePoop => {
                    config.poop_off = !config.poop_off;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.poop.set_checked(!config.poop_off);
                    save_config(&app, &mut config);
                }
                Input::ToggleLumosLight => {
                    config.lumos_light = !config.lumos_light;
                    let _ = app.state::<AppState>().menu.lumos_light.set_checked(config.lumos_light);
                    if let Some(l) = &lumos {
                        l.set_enabled(config.lumos_light);
                    }
                    save_config(&app, &mut config);
                }
                Input::ToggleWebLedges => {
                    config.web_ledges_on = !config.web_ledges_on;
                    // 체크 항목은 누를 때 스스로 뒤집히므로 항상 실제 값으로 다시 맞춤
                    let _ = app.state::<AppState>().menu.web_ledges.set_checked(config.web_ledges_on);
                    // 끄면 손잡이를 버려 스레드가 멈추고, 다음 세계 읽기부터 디딤돌이 사라짐 (그 위 펫은 떨어짐)
                    web_ledges = start_web_ledges(&config);
                    save_config(&app, &mut config);
                }
                Input::ToggleAutostart => {
                    crate::autostart::toggle(&app, &mut config);
                    save_config(&app, &mut config);
                }
                Input::StatsConsent(yes) => {
                    set_stats_consent(&app, &mut config, &network, yes);
                    save_config(&app, &mut config);
                }
                Input::ToggleStats => {
                    // 체크 항목은 누를 때 스스로 뒤집히므로 set_stats_consent가 실제 값으로 다시 맞춤
                    let yes = !config.pet_stats.on();
                    set_stats_consent(&app, &mut config, &network, yes);
                    save_config(&app, &mut config);
                }
                Input::HouseClick => {
                    // 🐾🐾🐾 하우스 안의 배고픈 펫이 있으면 그 펫의 음식 고르기
                    let n = own_count(&slots);
                    if slots[..n].iter().any(|s| s.pet.is_housed()) {
                        if let Some(s) = slots[..n].iter().find(|s| s.pet.is_housed() && care_of(&config, s).is_hungry()) {
                            let _ = app.emit_to(house::LABEL, "food-picker", favorite_of(&s.entry));
                        } else {
                            let _ = app.emit_to(house::LABEL, "house-wave", ());
                        }
                    }
                }
                Input::HouseMoved { save } => {
                    // 문 자리가 집을 따라감: 집에 가는 중·나와서 먹는 중이면 새 문으로 (하던 일은 끊지 않음),
                    // 안에 있으면 그대로 안에 (먹이 그릇은 새 문 앞에)
                    let spot = house::spot(&app, &slots[OWN].world.work_areas, slots[OWN].dpi);
                    for own in slots.iter_mut().filter(|s| s.is_own()) {
                        own.pet.set_house_door(spot.ground, spot.door_x);
                    }
                    if save {
                        config.house_spot = house::placed();
                        save_config(&app, &mut config);
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
                input @ (Input::Net(NetInput::SendTo(_)) | Input::Net(NetInput::SubmitLetter(_)) | Input::SendPet { .. }) => {
                    // 놀러 보내기 (편지 들려 보내기도 같은 길 — Visit에 편지를 실음):
                    // 하우스 안이면 먼저 문 앞으로 나온 뒤 그 친구 쪽 끝으로 감
                    let (id, note, is_letter, uid) = match input {
                        Input::Net(NetInput::SubmitLetter(text)) => {
                            let Some((id, note)) = network.letter_target(&text) else { continue };
                            (id, note, true, None)
                        }
                        Input::Net(NetInput::SendTo(id)) => (id, None, false, None),
                        Input::SendPet { uid, friend } => (friend, None, false, Some(uid)),
                        _ => continue,
                    };
                    // 🐾🐾🐾 고른 펫 (메뉴에서 펫을 골랐으면 그 펫). 고른 펫이 못 가면 갈 수 있는 다른 펫
                    let n = own_count(&slots);
                    let i = match uid {
                        Some(u) => slots[..n].iter().position(|s| s.uid() == Some(u.as_str())),
                        None => {
                            let p = picked(&app, &slots);
                            Some(p).filter(|&p| network.can_send(&slots[p], &id)).or_else(|| (0..n).find(|&k| network.can_send(&slots[k], &id)))
                        }
                    };
                    let Some(i) = i else {
                        if is_letter {
                            network.finish_letter(&app, false);
                        }
                        continue;
                    };
                    let mut sent = false;
                    if network.can_send(&slots[i], &id) {
                        if in_or_going_home(&slots[i].pet) {
                            leave_house_now(&app, &mut slots[i]);
                            hide_house_if_empty(&slots, house_window.as_ref());
                            // 다른 펫이 아직 하우스 안이면 하우스 모드는 그대로
                            config.housed = slots[..n].iter().any(|s| in_or_going_home(&s.pet));
                            menu_dirty = true;
                            save_config(&app, &mut config);
                        }
                        let own = &mut slots[i];
                        // 그 자리에서 손 흔들고("bye~!", 편지를 들었으면 "💌!") 뿅 사라짐 — 사라지면 own_exited가 Visit을 보냄
                        sent = network.send_to(&config, own, &id, note);
                    }
                    if is_letter {
                        network.finish_letter(&app, sent);
                    }
                }
                Input::Vehicle(input) => {
                    let busy = hand.is_some() || objects.is_held() || slots.iter().any(|s| s.pet.is_held());
                    let cursor = source.cursor();
                    let wa = slots[OWN].world.work_area_at(cursor.0, cursor.1);
                    vehicles.handle(&app, input, wa, cursor, busy, &mut slots);
                }
                Input::Object(input) => {
                    if matches!(input, ObjectInput::ClearAll) {
                        // 🚗 탈것도 같이 치움
                        vehicles.clear_all(&app);
                        weather.clear_piles();
                        // 🕷️ 거미인간이 친 거미집도 (trail.rs)
                        trail.clear_webs(&app);
                    }
                    let busy = hand.is_some() || slots.iter().any(|s| s.pet.is_held());
                    objects.handle(&app, input, &slots[OWN].world, source.cursor(), busy);
                }
                // 친구가 던진 물건 (net이 인증된 친구 링크·친구별 한도를 확인했고, 받은 물건 20개 상한과 다듬기는 objects가 지키고, 창은 틱마다 하나씩 만듦)
                Input::Net(NetInput::Event(screenpet_net::NetEvent::Tossed { from, toss })) => {
                    // 🧹 친구가 쓸어 보낸 더미: 내 친구의 것만, 집중·하우스 중이면 조용히 버림
                    if let Some(kind) = screenpet_core::weather::parse_flurry_kind(&toss.kind) {
                        if config.friend(&from).is_some() && !own_refuses(&slots) {
                            let at = toss.fling.as_ref().map_or(0.5, |f| f.height);
                            weather.receive_flurry(&app, kind, crate::network::from_net_side(toss.side, toss.vside), at);
                        }
                        continue;
                    }
                    // 🪃 친구가 던진 탈것: 집중·하우스 중이거나 자리가 없으면 되돌림 (모르는 종류는 버림)
                    if toss.kind == crate::network::VEHICLE_TOSS_KIND {
                        let Some(kind) = toss.look.as_deref().and_then(screenpet_core::vehicle::VehicleKind::from_id) else { continue };
                        let Some(fling) = toss.fling.clone().map(crate::network::from_net_fling) else { continue };
                        if config.friend(&from).is_none() {
                            continue;
                        }
                        let side = crate::network::from_net_side(toss.side, toss.vside);
                        let refuses = own_refuses(&slots);
                        let own = &slots[OWN];
                        let caught = !refuses && vehicles.catch_thrown(&app, kind, &own.world, side, fling, own.dpi);
                        if caught {
                            screenpet_log::log!(Info, "toss", "vehicle_in", "friend" => screenpet_log::short(&from), "kind" => kind.id());
                        } else if !toss.bounced {
                            network.bounce_toss(&from, toss);
                        }
                        continue;
                    }
                    // 집중·하우스 중이면 받지 않고 되돌림
                    match screenpet_core::visit::toss_answer(own_refuses(&slots), toss.bounced, objects.threw_lately(&from)) {
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
                    // 🐾🐾🐾 첫 펫만 나눔 (옛 필드 care[첫 펫 종류]를 거쳐)
                    config.sync_legacy();
                    let lead_species = slots[OWN].entry.id.clone();
                    let got = linked::adopt_shared(&mut config.care, &lead_species, pet.as_deref(), &list, known);
                    // 지금 키우지 않는 동물의 기록은 예전 기록으로 남김 (sync_legacy가 care를 새로 만들어도 남게)
                    config.keep_old_records();
                    if got.changed {
                        let shown = care_of(&config, &slots[OWN]).stage;
                        let care = config.care.get(&lead_species).cloned().unwrap_or_default();
                        set_care(&mut config, &slots[OWN], care.clone());
                        let own = &mut slots[OWN];
                        if care.stage != shown || own.track.pending_stage {
                            own.track.pending_stage = false;
                            apply_own_stage(&app, own, care.stage);
                        }
                        own.pet.set_hungry(care.is_hungry());
                        own.track.locked = care.locked;
                        menu_dirty = true;
                        save_config(&app, &mut config);
                    }
                    // 펫 바꾸기는 평소 길로 (친구에게 가 있으면 다음 소식 때 다시)
                    if let Some(entry) = got.switch_to.and_then(|id| state.pets.iter().find(|e| e.id == id).cloned()) {
                        state.send(Input::SetPet { entry, remote: true });
                    }
                }
                Input::Net(input) => {
                    if network.handle(&app, input, &mut config, &mut slots) {
                        save_config(&app, &mut config);
                    }
                    // 🤹 흐물이가 들고 온 물건 (손님), 못 들어가 들고 돌아온 물건 (내 동물): 그 손에 쥐여 줌
                    // 🚗 탈것을 몰고 온 손님: 들어오는 끝에 탈것을 꺼내 태움 (자리가 없으면 그냥 걸어 들어옴)
                    for (label, kind) in network.take_ride_arrivals() {
                        let Some(s) = slots.iter_mut().find(|s| s.label == label) else { continue };
                        if let Some(side) = s.pet.entering_from() {
                            vehicles.arrive(&app, kind, &s.world, side, s.dpi, &mut s.pet);
                        }
                    }
                    for c in network.take_carry_arrivals() {
                        let Some(s) = slots.iter_mut().find(|s| s.label == c.label) else { continue };
                        let at = (s.pet.body.x, s.pet.body.y - s.pet.cfg.size);
                        if let Some(id) = objects.spawn_carried(&app, &s.world, c.kind, c.scale, s.is_own(), at, c.icon) {
                            // 💪 거대한 것 (영웅이 들고 옴 — 이 화면 높이로 다시 맞춘 크기): 거대한 것까지 다 들어와야 섬
                            match objects.half_w_of(id).filter(|_| c.kind.is_giant()) {
                                Some(half) => s.pet.hold_giant(id, half),
                                None => s.pet.hold_object(id),
                            }
                        }
                    }
                }
                Input::ShowHelp => crate::help::show(&app),
                Input::ShowFeedback => crate::feedback::show(&app),
                Input::ShowMailbox => crate::mailbox::show(&app),
                Input::ShowBoard => crate::board::show(&app),
                Input::ShowPetStats => crate::pet_stats::show(&app),
                Input::ShowDodge => {
                    // 조작 모드는 게임이 대신함
                    if control_on {
                        control_on = false;
                        if let Some(i) = slots::find(&slots, &control_label) {
                            slots[i].pet.set_control(false);
                        }
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
                        // 밥그릇·손 음식처럼 펫 크기로 만든 창을 맞추고 남김 (🐾🐾🐾 펫마다)
                        for own in slots.iter().filter(|s| s.is_own()) {
                            if let Some(p) = own.uid().and_then(|u| props.get_mut(u)) {
                                p.resize(&app, own.dpi, own.shown.scale);
                            }
                        }
                        save_config(&app, &mut config);
                    }
                }
                Input::ShowOpacity => crate::opacity::show(&app),
                Input::SetOpacity { pct, save } => {
                    config.set_opacity(f64::from(pct) / 100.0);
                    crate::opacity::apply(&app, &config);
                    if save {
                        save_config(&app, &mut config);
                    }
                }
                Input::ShowTimeline => timeline.show(&app, &config),
                Input::NewVersion(v) => cheer_gate.found(v),
                Input::Quit => {
                    save_config(&app, &mut config);
                    // 🌗 바꿔 둔 테마를 되돌리고 끔
                    if let Some(l) = &lumos {
                        l.quit(LUMOS_QUIT_WAIT);
                    }
                    app.exit(0);
                    return;
                }
                Input::SaveAndStop(done) => {
                    save_config(&app, &mut config);
                    if let Some(l) = &lumos {
                        l.quit(LUMOS_QUIT_WAIT);
                    }
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
        // 🐾🐾🐾 펫마다 배고파짐
        for p in config.pets.iter_mut() {
            p.care.tick(dt);
        }
        // 🐾🐾🐾 창을 못 만든 펫이 있으면 가끔 다시 만들어 봄 (설정에만 남아 배만 고파지지 않게)
        if now >= next_spawn_retry {
            next_spawn_retry = now + SPAWN_RETRY;
            if config.pets.len() > own_count(&slots) && spawn_missing_pets(&app, &mut slots, &config) {
                menu_dirty = true;
            }
        }
        let n = own_count(&slots);
        // 👻 디멘터 곁에서 더 고파진 배 (코어 brain_dementor.rs — 손님 것은 버림)
        for s in slots.iter_mut() {
            let chill = s.pet.take_chill();
            if let Some(p) = s.uid().and_then(|u| config.own_pet_mut(u)) {
                p.care.chill(chill);
            }
        }
        if now - last_refresh >= WORLD_REFRESH {
            let (windows, work_areas) = (source.windows(), source.work_areas());
            // 🌐 마지막으로 읽은 웹 페이지 디딤돌 (꺼져 있거나 낡았으면 None) — 펫 세계에만 (물건 세계·친구에게는 안 감)
            let ledges = web_ledges.as_ref().and_then(|p| p.latest());
            for s in &mut slots {
                s.refresh(&windows, &work_areas);
                s.world.set_ledges(ledges.clone());
            }
            objects.set_dpi_hint(slots[OWN].dpi);
            vehicles.set_dpi_hint(slots[OWN].dpi);
            objects.refresh(&windows, &work_areas, now);
            for i in 0..n {
                let care = care_of(&config, &slots[i]);
                slots[i].pet.set_hungry(care.is_hungry());
                slots[i].track.locked = care.locked;
            }
            last_refresh = now;
        } else {
            follow_ridden_windows(source.as_mut(), &mut slots);
        }
        if now - last_save >= SAVE_EVERY {
            // 📊 날짜가 바뀌었으면 지난날 보고가 생김 — 다음 릴레이 잡일에 감
            stats_tick(&mut config, &network);
            save_config(&app, &mut config);
            // 📱 노는 동안 가끔 지금 포만감까지 (쉬는 컴퓨터도 같은 시간만큼 배고파짐)
            share_care(&network, &config, false);
            last_save = now;
        }

        // 커서·간질이기·드래그는 커서 아래 맨 위 한 마리에게만
        let (cx, cy) = source.cursor();
        // 이번 틱의 Esc가 직접 주기 거두기라면 조작 모드는 끝내지 않음
        let hand_active = hand.is_some();
        // 직접 주기: 음식이 커서를 따라가고, 내 동물 입에 닿으면 먹음. 오른쪽 버튼·Esc·빈 곳 클릭·30초면 거둠
        if let Some(h) = hand.as_mut() {
            // 🐾🐾🐾 음식에 닿는 펫이 먹음 (모두가 손을 봄 — 먼저 닿은 펫)
            let dpi = slots[OWN].dpi;
            let off = food::HAND_OFFSET * dpi;
            let at = (cx + off, cy + off);
            hand_win.move_to(at, dpi);
            let input = h.tick(dt, source.left_button_down(), source.right_button_down(), source.escape_down());
            let food = h.food;
            let mut ready = None;
            for i in 0..n {
                if input != HandInput::Cancel && slots[i].on_screen() && slots[i].pet.offer_hand(at.0, at.1) && ready.is_none() {
                    ready = Some(i);
                }
            }
            let mut done = false;
            if let Some(i) = ready {
                let mut care = care_of(&config, &slots[i]);
                let before = care.clone();
                let result = care.feed(food);
                let own = &mut slots[i];
                if own.pet.eat_from_hand(result, food) {
                    if let FeedResult::Ate { leveled_up } = result {
                        own.track.pending_stage |= leveled_up;
                        timeline.ate(&app, food, &favorite_of(&own.entry), &config);
                        hand_win.bite();
                        done = true;
                    }
                    // 배불러 갸웃 — 음식은 손에 남음
                } else {
                    care = before;
                }
                own.pet.set_hungry(care.is_hungry());
                set_care(&mut config, &slots[i], care);
                if done {
                    save_config(&app, &mut config);
                    share_care(&network, &config, true);
                }
            } else {
                // 동물 위를 누른 건 거두기가 아님 (닿을 때까지 더 가까이)
                let empty_click = input == HandInput::Click && !slots[..n].iter().any(|s| s.cursor_inside(cx, cy));
                if input == HandInput::Cancel || empty_click || !slots[..n].iter().any(|s| s.on_screen()) {
                    hand_win.drop_back();
                    done = true;
                }
            }
            if done {
                for s in slots[..n].iter_mut() {
                    s.pet.end_hand();
                }
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
                objects.sweep_objects(b);
                // 🕸️ 지나간 거미집도 걷음 (trail.rs)
                trail.sweep_webs(&app, b, dpi);
                // 🥶 디멘터가 낀 구석 서리도 녹음 (코어 brain_dementor.rs)
                for s in slots.iter_mut() {
                    s.pet.sweep_frost(b, dpi);
                }
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
        // ⛄ 문질러 뭉치기: 빗자루·음식·동물·물건을 들고 있지 않을 때, 커서 바로 아래 보이는 눈 더미 위에서 좌우로 빠르게 문지르면
        // 그 자리의 눈을 모아 눈덩이를 만듦 (날씨 창은 마우스가 통과하므로 클릭 없이). 눈이 모자라면 아무 일 없음
        {
            use screenpet_core::snowball::{PACK_MIN_SNOW, REACH_X, REACH_Y};
            let dpi = slots[OWN].dpi;
            let (rx, ry) = (REACH_X * dpi, REACH_Y * dpi);
            let free = broom.is_none() && hand.is_none() && !objects.is_held() && !slots.iter().any(|s| s.pet.is_held());
            let over_snow = free && weather.has_snow() && weather.snow_under(cx - rx, cx + rx, cy, ry) > 0.05;
            if let Some((lo, hi)) = rub.tick(dt, (cx, cy), dpi, over_snow) {
                if weather.snow_under(lo - rx, hi + rx, cy, ry) >= PACK_MIN_SNOW {
                    weather.pack_snow(lo - rx, hi + rx, cy, ry);
                    weather.puff(&app, screenpet_core::weather::Weather::Snow, (cx, cy), (0.0, -160.0 * dpi), 10);
                    // 더미 윗면 위에서 떨어뜨려 그 디딤판에 내려앉게 (커서는 윗면 아래로도 조금 내려가 있음)
                    objects.spawn_snowball(&app, &slots[OWN].world, (cx, cy - ry));
                }
            }
        }
        // 🎮 조작 모드: 키를 내 동물에게 (친구 화면에 가 있으면 친구 링크로도). Esc(직접 주기 중이 아닐 때)·키 없이 3분이면 끝
        let mut keys = ControlInput::default();
        // 💩 똥피하기: 게임 창이 받은 키를 내 동물에게
        dodge.steer(&mut slots[OWN], dt);
        // 🐾🐾🐾 조종하는 펫 (그새 보내 줬으면 끔)
        let control_i = slots::find(&slots, &control_label).filter(|&i| i < n);
        if control_on && control_i.is_none() {
            // 조종하던 펫이 없어졌음: 그 펫 두뇌도 조작 모드를 끔 (아무도 없으면 그냥 끔)
            for s in slots[..n].iter_mut() {
                s.pet.set_control(false);
            }
            control_on = false;
            let _ = app.state::<AppState>().menu.control.set_checked(false);
        }
        if let (true, Some(ci)) = (control_on, control_i) {
            keys = source.control_keys();
            saw_key |= keys.any();
            let own = &mut slots[ci];
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
                    let _ = app.emit_to(speech_target(own), "pet-say-text", crate::tray::KEY_PERMISSION_HINT);
                }
            }
        }
        let control_uid = control_i.and_then(|i| slots[i].uid().map(str::to_string));
        network.steer(now, control_on, keys, control_uid.as_deref());
        let held = slots.iter().position(|s| s.pet.is_held());
        let hovered = if held.is_some() || objects.is_held() || vehicles.is_held() { None } else { slots::hover_target(&slots, cx, cy) };
        // 🐾 커서 장난: 무언가 끌고 있거나 음식을 들고 있거나 모두 숨겨 둔 동안엔 새로 시작하지 않음 (조작 모드 등은 코어가 봄)
        let left_down = source.left_button_down();
        // 💩 새로 누른 틱에만: 화면 위 동물(내 동물·손님)마다 낮은 확률로 쌈. 자리 비움·전체 화면(그 동물이 있는 모니터)·숨김·집중 중엔 안 함
        if std::mem::replace(&mut was_left_down, left_down) != left_down
            && left_down
            && screenpet_core::poop::may_poop(config.poop_off, crate::windows_hidden(&app), slots[..n].iter().any(|s| s.pet.is_focusing()))
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
        let play_busy = held.is_some() || hand.is_some() || objects.is_held() || vehicles.is_held() || crate::windows_hidden(&app);
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
                    let _ = app.emit_to(speech_target(&slots[OWN]), "pet-say-text", crate::tray::TYPING_PERMISSION_EXPLAIN);
                }
            }
            Some(AskStep::Request) => {
                config.input_monitoring_asked = Some(env!("CARGO_PKG_VERSION").to_string());
                save_config(&app, &mut config);
                source.request_typing_permission();
            }
            Some(AskStep::RestartHint) => {
                if slots[OWN].on_screen() {
                    let _ = app.emit_to(speech_target(&slots[OWN]), "pet-say-text", crate::tray::TYPING_RESTART_HINT);
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
                    // 🚗 탈것 위에 떨궜으면 태움 (운전석이 비었으면 운전, 트럭이면 짐칸)
                    vehicles.offer_drop(&mut s.pet);
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
            // 🐾🐾🐾 내 펫 모두
            for s in slots[..n].iter_mut() {
                s.pet.set_exit_edges(false, false);
                s.pet.set_vertical_edges(false, false);
            }
        }
        // 🐾🐾🐾 화면 끝으로 다 나간 내 펫들 (슬롯 번호, 끝)
        let mut own_exits: Vec<(usize, Side)> = Vec::new();
        // 🧳 포트키로 사라져 친구 쪽을 물어볼 내 펫 (슬롯 번호, 짝 열쇠)
        let mut portkey_asks = Vec::new();
        let mut left = Vec::new();
        objects.poop_spots(&mut poop_spots);
        objects.object_spots(&mut object_spots);
        objects.portkey_spots(&mut portkey_spots);
        // 📌 작업표시줄 장난감을 지금 만들 수 있나 (내 흐물이만 봄)
        let toys = objects.toys(&app);
        objects.incoming(&mut incoming);
        let all_hidden = crate::windows_hidden(&app);
        if all_hidden {
            portkey_spots.clear();
        }
        // 🧳 친구가 포트키로 물어보면 이 화면의 같은 물건을 찾음 (network.tick이 받음)
        network.set_portkey_spots(&portkey_spots);
        // 🙆 흐물이에게 같은 화면의 다른 동물을 보여 줌 (번쩍 들러 감)
        screenpet_core::brain::hoist_moves::notice_pets(&mut slots);
        // 🕸️ 화면의 거미집을 모든 동물에게 알려 줌 — 거미인간이 아닌 동물이 들어가면 걸림 (코어 brain_snare.rs)
        let web_spots = trail.web_spots();
        for s in slots.iter_mut() {
            s.pet.set_webs(&web_spots);
        }
        // 🕸️📦 물건도 같은 거미집에 걸림 (코어 object.rs — 아래 objects.tick이 물건마다 넘김)
        objects.set_webs(&web_spots);
        pet_targets.clear();
        pet_targets.extend(slots.iter().map(|s| {
            (!s.pet.is_away() && s.pet.hit_feels()).then(|| screenpet_core::juggle::Target { x: s.pet.body.x, y: s.pet.body.y, size: s.pet.cfg.size })
        }));
        // 🚗 탈것 자리 (한가한 펫이 가까운 빈 탈것을 타러 감 — 코어 brain_vehicle.rs)
        vehicle_spots.clear();
        vehicle_spots.extend_from_slice(vehicles.spots());
        for (i, s) in slots.iter_mut().enumerate() {
            // 친구 화면에 가 있거나 🛷 썰매에 탄 동안엔 물리·행동이 멈춤
            if !s.pet.is_away() && !sleigh.is_riding(&s.label) {
                // 같은 화면의 다른 동물 (흐물이 던지기·⛄ 눈싸움이 노림)
                juggle_targets.clear();
                juggle_targets.extend(pet_targets.iter().enumerate().filter(|(j, _)| *j != i).filter_map(|(_, t)| *t));
                // ⛄🍂 발밑 눈·낙엽 더미 (모두 숨겨 둔 동안엔 없음) — 쉬는 박자가 끝날 때 가끔 놂 (코어 brain_snowplay.rs)
                let pile = if all_hidden { None } else { weather.pile_under(s.pet.body.x, s.pet.body.y, s.pet.cfg.half_w) };
                s.pet.notice_pile(pile, &juggle_targets, !play_busy && !config.cursor_play_off);
                // 🤹 흐물이: 바닥의 물건을 주우러 가고, 같은 화면의 다른 동물을 노림 (다른 동물은 할 일이 없음)
                if s.pet.cfg.juggler {
                    s.pet.notice_objects(&object_spots, &juggle_targets, &s.world);
                    s.pet.notice_incoming(&incoming, &s.world);
                }
                // ⚾🖱️ 커서와 캐치볼: 사용자가 있고 커서 장난이 켜져 있고 아무것도 들고 있지 않을 때만, 가끔 먼저 놀자고 함 (코어 brain_cursor_rally.rs)
                s.pet.set_cursor_rally_ok(!play_busy && !config.cursor_play_off && !presence.is_away());
                s.pet.notice_cursor_rally(if all_hidden { &[] } else { &object_spots }, &s.world);
                s.pet.notice_poops(&poop_spots, &s.world);
                s.pet.notice_vehicles(if all_hidden { &[] } else { &vehicle_spots }, &s.world);
                // 🐱 고양이 툭툭 · 🐿️ 다람쥐 볼 빵빵 (v0.6.3) · 🕷️ 거미인간 끌어오기: 바닥의 물건 — 모두 숨겨 둔 동안엔 보여 주지 않음
                s.pet.notice_play_objects(if all_hidden { &[] } else { &object_spots }, &s.world);
                // 📌 흐물이: 가끔 작업표시줄(Dock)에서 열린 앱 아이콘 장난감을 끌어내고, 갖고 놀다 도로 넣음 (손님은 하지 않음)
                if s.pet.cfg.taskbar_play && s.is_own() {
                    s.pet.notice_taskbar(&object_spots, toys, &s.world);
                }
                // 🧳 마법사: 놓인 물건을 포트키로 (짝 물건은 이 화면에, 또는 내 펫이면 새 버전 친구 화면에 — 손님은 이 화면 안에서만)
                if s.pet.cfg.portkey {
                    s.pet.notice_portkeys(&portkey_spots);
                    let friend_ok = s.is_own() && s.uid().is_some_and(|u| network.portkey_friend_ok(&config, u));
                    s.pet.set_portkey_friend(friend_ok);
                }
                // 🪶 해리·헤르미온느: Leviosa로 작업표시줄 앱 아이콘을 뽑아 띄울 수 있나 (흐물이 장난감과 같은 사정 — 내 펫만)
                if s.is_own() && s.pet.cfg.spells.has(screenpet_core::wizard::Spell::Leviosa) {
                    s.pet.set_icon_lift_ok(toys.ready);
                }
                s.pet.update(dt, &s.world);
            }
            // 🌗 이 화면에서 "Lumos!"가 켜짐 / "Nox!"로 꺼짐 (내 펫이든 놀러 온 펫이든) — 설정이 켜져 있으면 일꾼이 잠깐 라이트 / 다크 모드로
            if s.pet.take_lumos() && s.on_screen() {
                if let Some(l) = &lumos {
                    l.lumos();
                }
            }
            if s.pet.take_nox() && s.on_screen() {
                if let Some(l) = &lumos {
                    l.nox();
                }
            }
            // 🧳 포트키로 사라져 친구 화면의 짝 물건을 물어봄 (내 펫만 — 손님이 낸 것은 그 자리로 되돌림)
            if let Some(screenpet_core::brain::portkey_moves::PortkeyEvent::AskFriend { key }) = s.pet.take_portkey() {
                if s.is_own() {
                    portkey_asks.push((i, key));
                } else {
                    let Slot { pet, world, .. } = &mut *s;
                    pet.portkey_return(world);
                }
            }
            if let Some(speech) = s.pet.take_speech() {
                let target = if s.is_own() { speech_target(s) } else { s.label.as_str() };
                let _ = app.emit_to(target, "pet-say", speech);
            }
            // ⚾ 받은 횟수 ("3!") 같은 글자 그대로의 말
            if let Some(text) = s.pet.take_speech_text() {
                let target = if s.is_own() { speech_target(s) } else { s.label.as_str() };
                let _ = app.emit_to(target, "pet-say-text", text);
            }
            // 밥그릇 놓기·거두기 (손님은 먹이를 받지 않음 — 🐾🐾🐾 펫마다 그릇 하나)
            if let Some(ev) = s.pet.take_prop() {
                if let Some(uid) = s.uid() {
                    props.entry(uid.to_string()).or_default().apply(&app, ev, s.dpi, s.shown.scale);
                }
            }
            if let Some(side) = s.pet.take_exit() {
                if s.is_own() {
                    own_exits.push((i, side));
                } else {
                    // 손님이 집 쪽(또는 다른 친구 쪽) 끝으로 걸어 나갔거나 손 흔들고 사라졌음
                    left.push((i, side));
                }
            }
        }
        // 🙆 흐물이가 든 동물을 손 자리로 옮기고, 내려놓고, 함께 나간 것을 맞춤 (손님을 닫기 전에)
        for (lifter, carried, side) in screenpet_core::brain::hoist_moves::sync_lifts(&mut slots) {
            // 손님 흐물이가 이 화면 주인의 동물을 들고 친구 쪽 끝으로 나갔음: 둘 다 같은 친구에게 가게 함
            if carried < n {
                let uid = slots[carried].uid().unwrap_or_default().to_string();
                if let Role::Guest(g) = &slots[lifter].role {
                    network.pair_lift(&config, g, side, &uid);
                }
            }
        }
        // 🕸️ 걸렸다가 찢고 나온 거미집은 지움 (trail.rs)
        for s in slots.iter_mut() {
            if let Some(id) = s.pet.take_torn_web() {
                trail.tear_web(&app, id);
            }
        }
        for i in 0..n {
            if !own_exits.iter().any(|(k, _)| *k == i) {
                if let Some(side) = slots[i].pet.take_exit() {
                    own_exits.push((i, side));
                }
            }
        }
        // 머리 위에 선 동물은 아래 친구를 따라감 (창을 옮기기 전, 아래층부터)
        social.carry(&mut slots);
        for (i, side) in left.into_iter().rev() {
            let mut slot = slots.remove(i);
            // 🚗 손님이 몰고 온 탈것도 같이 떠남
            vehicles.guest_left(&app, slot.pet.social_key());
            // 던져서 넘어갔으면 그 움직임 (주인 화면이나 다음 친구 화면으로 날아감)
            let fling = slot.pet.take_fling();
            if let Role::Guest(g) = &slot.role {
                screenpet_log::log!(Info, "visit", "guest_left", "visit" => g.visit_id, "owner" => screenpet_log::short(&g.owner_id));
                network.guest_left(&config, &slot, g, side, slot.pet.guest(), fling);
            }
            slots::close(&app, slot);
        }
        network.forget_gone_replies(&slots);

        // 🚗 탈것: 태우기·달리기·마주침·좌석에 앉히기 (펫 창은 아래 place가 좌석 자리로 옮김)
        // (세계는 잠깐 빌려 옴 — 차고는 펫들을 고치는 동안 내 동물의 세계를 읽음)
        let vworld = std::mem::replace(&mut slots[OWN].world, World::new(Vec::new(), Vec::new(), 1.0));
        vehicles.set_throw_edges(network.vehicle_throw_edges(&config));
        let drove = vehicles.tick(&app, &vworld, dt, (cx, cy), left_down, &mut slots);
        slots[OWN].world = vworld;
        // 🪃 친구 화면 쪽 끝 너머로 던진 탈것: 그 끝의 친구에게 날려 보냄 (받는 친구 것이 됨)
        for (kind, side, fling) in vehicles.take_thrown() {
            network.toss_vehicle(&config, kind, side, fling);
        }
        for (driver, kind, _side) in drove {
            // 내 펫이 탈것째 친구 화면으로 나갔음: 다음 틱 take_exit로 놀러 감 — 돌아올 때 다시 몰고 옴
            vehicles.remember_parked_away(driver, kind);
            if let Some(s) = slots[..n].iter_mut().find(|s| s.pet.social_key() == driver) {
                s.track.ride = Some(kind);
            }
        }
        // 🤹 흐물이가 든 물건을 손 자리로 옮기고, 놓은 것은 던짐
        objects.sync_carries(&mut social, &mut slots);
        // 🐱 툭 밀기 · 🐿️ 뿅 · 🦖 큰 쿵에 톡 (v0.6.3)
        objects.play_with(&mut slots);
        // 🧔 🪨 해그리드가 발 앞에 내려놓은 락 케이크 (돌 물건)
        for (kind, x, y) in objects.take_spawns() {
            objects.spawn_gift(&app, &slots[OWN].world, kind, (x, y));
        }
        // ⚾🖱️ 커서 쪽으로 되던지거나 놀자고 민 물건: 횟수를 적고 커서 랠리로 표시함 (놓은 것은 위 sync_carries가 이미 날림)
        for s in slots.iter_mut() {
            if let Some(ev) = s.pet.take_cursor_rally() {
                objects.cursor_rally(ev);
            }
        }
        // ⛄🍂 눈·낙엽 놀이: 뭉친 눈은 발밑 눈을 비우고 눈덩이로 날리고 (던진 동물은 잠깐 안 맞음), 뛰어든·뒹군 더미는 확 흩어짐
        for i in 0..slots.len() {
            let Some(ev) = slots[i].pet.take_snow_play() else { continue };
            let s = &slots[i];
            let half = s.pet.cfg.size * screenpet_core::snowplay::SCOOP_REACH;
            match ev {
                screenpet_core::snowplay::SnowPlayEvent::Throw { foot_x, foot_y, x, y, vx, vy } => {
                    // 그 사이 녹았거나 치웠으면 아무것도 날리지 않음
                    if weather.pack_snow(foot_x - half, foot_x + half, foot_y, 8.0 * s.dpi) > 0.0 {
                        if let Some(id) = objects.spawn_snowball(&app, &s.world, (x, y)) {
                            objects.launch_snowball(&mut social, id, s.pet.social_key(), vx, vy);
                        }
                    }
                }
                screenpet_core::snowplay::SnowPlayEvent::Burst { x, y } => weather.burst_pile(&app, x, y, half, s.dpi),
            }
        }
        // 📌 읽은 앱 아이콘이 왔으면 장난감을 만들어 내 흐물이 손에 쥐여 줌
        {
            // 🐾🐾🐾 장난감 아이콘을 기다리던 펫 (없으면 첫 펫)
            let k = (0..n).find(|&i| slots[i].pet.icon_lift_wanted().is_some() || slots[i].pet.wants_toy()).unwrap_or(OWN);
            let (head, rest) = slots.split_at_mut(k);
            let Slot { pet, world, .. } = &mut rest[0];
            let base = head.first().map_or(&*world, |s| &s.world);
            objects.deliver_toy(&app, base, pet);
        }
        // 🧳 포트키로 친구 화면의 짝 물건을 물어봄 (물건은 싣지 않음 — 늘 제자리). 보낼 친구가 없으면 내 화면의 짝 물건으로, 없으면 제자리
        for (i, key) in portkey_asks {
            if !network.portkey_away(&config, &mut slots[i], key) {
                let Slot { pet, world, .. } = &mut slots[i];
                pet.portkey_return(world);
            }
        }
        for (i, side) in own_exits {
            // 내 동물이 화면 끝으로 다 나갔음 — 그 끝의 친구에게 놀러 감 (물건을 들고 나갔으면 새 버전 친구에게 실음)
            let carried = slots[i].pet.carried_object();
            // 💪 영웅이 든 거대한 것도 (거대한 것을 받는 친구에게만 — own_exited)
            let carry = carried.and_then(|id| objects.carry_wire(id, slots[i].pet.carries_giants()));
            let ride = slots[i].track.ride.take();
            let took = network.own_exited(&config, &mut slots[i], side, carry, ride);
            if let Some(id) = carried {
                if took {
                    objects.hand_over(id);
                }
                // 못 실었으면 이 화면 끝에 떨어짐 (다시 들어오는 중이면 그대로 들고 옴)
                if took || slots[i].pet.is_away() {
                    slots[i].pet.carry_handed_over();
                }
            }
        }
        network.tick(&app, now, &config, &mut slots[..n]);
        network.tick_mailbox(&app, &mut slots, now);
        timeline.tick(&app, real_dt, &slots[OWN], slots[OWN].uid().and_then(|u| network.away_friend_of(u)), &config);
        if dodge.tick(&app, dt, &mut slots[OWN], &mut config, &network) {
            save_config(&app, &mut config);
        }
        // 🌀 태풍: 돌풍에 휘청이고 회오리가 빨아들여 내던짐 (물건 창은 바로 아래 objects.tick이 옮김, 넘어간 내 동물은 다음 틱 take_exit)
        weather.storm(&app, now, presence.is_away(), &mut slots, &mut objects, &network, &config, |s| sleigh.is_riding(&s.label));
        objects.tick(&app, &slots[OWN].world, dt, now, (cx, cy), left_down, presence.is_away(), source.as_mut(), &network, &config);
        // ⛄ 퍽 부서진 눈덩이 자리에 눈가루 (날씨 창이 없으면 보이지 않음)
        for at in objects.take_bursts() {
            weather.puff(&app, screenpet_core::weather::Weather::Snow, at, (0.0, -220.0 * slots[OWN].dpi), 14);
        }
        for p in props.values_mut() {
            p.tick(now);
        }
        hand_win.tick(now);
        broom_win.tick(now);
        for own in slots[..n].iter_mut() {
            network.tick_boost(&app, own, now);
        }
        weather.tick(&app, now, !config.weather_on, presence.is_away(), &mut slots);
        trail.tick(&app, now, dt, presence.is_away(), &mut slots);
        let marks_changed = {
            let Config { pets, daily, .. } = &mut config;
            presence.tick(&app, now, source.as_mut(), &mut slots[..n], pets, daily)
        };
        if marks_changed {
            save_config(&app, &mut config);
            share_care(&network, &config, false);
        }
        // 📱 대기 중인 이 컴퓨터를 다시 쓰기 시작했으면 이 컴퓨터로 놂 (쓴 지 얼마나 됐는지는 net에도)
        if let Some((secs, idle)) = presence.take_sample() {
            network.tick_presence(secs, idle);
        }
        // 손님도 이 화면 주인이 자리를 비운 동안엔 잠만 잠
        for s in &mut slots[n..] {
            s.pet.set_user_away(presence.is_away());
        }
        // 🔄 새 버전: 내 동물들이 "new version!!" 하고 10초쯤 뜀 — 손님은 아님. 창을 숨겨 둔 동안엔 다시 보일 때,
        // 자리 비움이면 돌아왔을 때 (동물 쪽이 미룸)
        if cheer_gate.due(crate::windows_hidden(&app)) {
            for s in slots.iter_mut().filter(|s| s.is_own()) {
                s.pet.cheer(screenpet_core::update::CHEER_SECS);
            }
        }
        // 🐾🐾🐾 펫마다: 먹이 조르기, 하우스 들어감·나옴, 친구 화면으로 떠남·돌아옴, 크기 바꾸기
        let spot = house::spot(&app, &slots[OWN].world.work_areas, slots[OWN].dpi);
        for i in 0..n {
            let care = care_of(&config, &slots[i]);
            let own = &mut slots[i];
            if own.pet.take_food_request() {
                let _ = app.emit_to(speech_target(own), "food-picker", favorite_of(&own.entry));
            }
            if own.pet.is_housed() != own.track.was_housed {
                own.track.was_housed = own.pet.is_housed();
                if own.track.was_housed {
                    // 문에 닿았음 — 펫 창을 숨기고 하우스 창 문간에 보임
                    crate::overlay::hide_quietly(&own.window);
                    // 집으로 가는 동안 모두 숨겼다가 다시 보였으면 집이 아직 안 보임
                    // tao의 is_visible은 조용히 숨긴 창을 아직 "보임"으로 앎 — OS에 물음 (모두 숨긴 채 하우스로 보내면 집이 끝내 안 보이던 버그)
                    if let Some(hw) = house_window.as_ref().filter(|w| !crate::windows_hidden(&app) && !crate::overlay::is_shown(w)) {
                        house::show_at(hw, &spot);
                    }
                } else {
                    // 하우스에서 나왔음 (나오기, 또는 문 앞 그릇으로 먹으러) — 문 앞 자리로 옮긴 뒤 보임
                    own.need_show = true;
                }
            }
            if own.pet.is_away() != own.track.was_away {
                own.track.was_away = own.pet.is_away();
                if own.track.was_away {
                    // 화면 끝으로 걸어 나갔음 — 돌아올 때까지 펫 창을 숨김
                    crate::overlay::hide_quietly(&own.window);
                } else {
                    // 돌아왔음 — 들어오는 자리로 옮긴 뒤 보임
                    own.need_show = true;
                    // 🚗 탈것째 놀러 갔었으면 그 탈것을 몰고 들어옴
                    if let Some(kind) = vehicles.take_parked_away(own.pet.social_key()) {
                        if let Some(side) = own.pet.entering_from() {
                            vehicles.arrive(&app, kind, &own.world, side, own.dpi, &mut own.pet);
                        }
                    }
                }
            }
            // 🐾🐾🐾 하우스 모드인데 밖에 있음 (친구 화면에서 돌아왔음): 들어와 서면 하우스로 감
            if config.housed && own.on_screen() && !in_or_going_home(&own.pet) && own.pet.is_free_to_go() {
                if let Some(hw) = house_window.as_ref().filter(|w| !crate::windows_hidden(&app) && !crate::overlay::is_shown(w)) {
                    house::show_at(hw, &spot);
                }
                own.pet.go_home(spot.ground, spot.door_x);
            }
            // 먹다가 눌린 동안(Pressed)도 먹는 중으로 봄 — 손을 떼면 계속 먹으므로 크기를 미리 바꾸지 않음.
            // 공중·축하 중에 받아 기억해 둔 먹이(간식 포함)가 성장을 안고 있어도 먹고 축하할 때까지 기다림
            if own.pet.stage_change_due(own.track.pending_stage) {
                own.track.pending_stage = false;
                apply_own_stage(&app, own, care.stage);
            }
        }
        {
            let state = app.state::<AppState>();
            state.housed.store(slots[..n].iter().any(|s| s.pet.is_housed()), Ordering::Relaxed);
            state.away.store(slots[OWN].pet.is_away(), Ordering::Relaxed);
        }
        // 🏠 집으로 달려가는 중인데 집이 안 보임 (모두 숨긴 채 하우스로 보낸 뒤 다시 보였음): 집을 보임 — 1초에 한 번쯤만 OS에 물음
        house_checks = house_checks.wrapping_add(1);
        if slots[..n].iter().any(|s| s.pet.is_going_home()) && house_checks % HOUSE_CHECK_TICKS == 0 && !crate::windows_hidden(&app) {
            if let Some(hw) = house_window.as_ref().filter(|w| !crate::overlay::is_shown(w)) {
                house::show_at(hw, &spot);
            }
        }
        // 🐾🐾🐾 하우스 문간에 보이는 펫: 하우스 안의 첫 펫 (아무도 없으면 첫 펫). 시트·크기는 지금 보이는 것 (성장 직후 먹는 동안엔 한 단계 늦음)
        let house_pet = slots[..n].iter().position(|s| s.pet.is_housed()).unwrap_or(OWN);
        let house_label = slots[house_pet].label.clone();
        let house_state = {
            let own = &slots[house_pet];
            HouseState {
                pet: own.shown.pet.clone(),
                stage: own.shown.stage,
                scale: own.shown.base,
                hungry: slots[..n].iter().filter(|s| s.pet.is_housed()).any(|s| care_of(&config, s).is_hungry()) || (!own.pet.is_housed() && care_of(&config, own).is_hungry()),
                housed: own.pet.is_housed(),
            }
        };
        if last_house.as_ref() != Some(&house_state) {
            *app.state::<AppState>().house.lock().unwrap() = house_state.clone();
            let _ = app.emit_to(house::LABEL, "house-state", house_state.clone());
            last_house = Some(house_state);
        }
        // 🐾🐾🐾 메뉴: 펫들의 모습이 바뀌었으면 (단계·성장 멈춤·하우스·놀러 감·종류) 고른 펫 메뉴와 하우스 글자를 맞춤
        let views = own_views(&slots, &config, &network);
        if menu_dirty || views != last_views {
            menu_dirty = false;
            *app.state::<AppState>().own.lock().unwrap() = views.clone();
            last_views = views;
            let _ = app.state::<AppState>().menu.house.set_text(crate::tray::house_label(config.housed));
            crate::tray::sync_picked(&app);
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
                if s.label == house_label {
                    // 하우스 창 문간도 (문간에 보이는) 내 동물의 먹기·축하를 보여 줌
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
