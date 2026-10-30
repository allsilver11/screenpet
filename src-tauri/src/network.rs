//! 친구 네트워크(screenpet-net)와 앱 사이 (spec phase 3 §1–§5).
//! - 시작: 설치 id·비밀키·친구 목록으로 `Net::start` (실패해도 앱은 동작 — 로그만)
//! - 네트워크 이벤트 → 게임 루프 입력 (`Input::Net`)으로 넘겨, 설정·슬롯은 게임 루프 스레드에서만 고친다
//! - 짝맞추기 창(`pair`): 양쪽에 이모지 6개를 보이고 두 사람이 모두 확인해야 친구가 된다
//! - 놀러 가기: 내 동물이 연결된 끝으로 나가면 `Visit`, 손님이 떠나면 `Leave`, 끊기면 바로 귀환
//! - 인터넷 친구 (spec §1b): 중계 서버(기본 `wss://pet.danni.day/ws`)로 같은 망 밖의 친구와도 연결된다.
//!   친구 코드: `내 친구 코드 보기`(펫 머리 위 띠, 누르면 복사) / `친구 코드 입력…`(작은 창 `code`) →
//!   코드 주인과 같은 짝맞추기(이모지 6개 확인)가 시작된다

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use screenpet_core::behavior::Personality;
use screenpet_core::brain::State;
use screenpet_core::care::Stage;
use screenpet_core::config::{Config, MAX_NAME_CHARS};
use screenpet_core::visit::{self, Side, MAX_GUESTS};
use screenpet_net::{normalize_code, parse_relay_key, Net, NetConfig, NetEvent, Visit, DEFAULT_RELAY_KEY, DEFAULT_RELAY_URL, MAX_FRIENDS};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::game_loop::Input;
use crate::slots::{self, GuestInfo, Role, Slot, OWN_LABEL};
use crate::tray::{FriendItem, FriendsMenu};
use crate::AppState;

/// 짝맞추기 창 라벨
pub const PAIR_LABEL: &str = "pair";
const PAIR_SIZE: (f64, f64) = (320.0, 168.0);
/// 초대를 보냈는데 이만큼 동안 InviteSent가 없으면 "연결하지 못했어요" (net은 그 전 실패를 알려 주지 않는다)
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 이모지를 보인 뒤 두 사람이 모두 확인해야 하는 시간 (net의 invite_timeout과 같다)
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(20);
/// 결과("친구가 되었어요" / 실패)를 보여 준 뒤 창을 닫기까지
const RESULT_SECS: Duration = Duration::from_millis(2500);
/// "집에 와"를 보냈는데 Leave가 이만큼 안 오면 그냥 돌아온다
const COME_HOME_FALLBACK: Duration = Duration::from_secs(15);
/// 한 친구에게서 받아들인 방문 사이 최소 간격 (그보다 잦은 Visit은 Leave로 돌려보낸다)
const VISIT_COOLDOWN: Duration = Duration::from_secs(5);
/// 보낸 방문이 이만큼 안에 그냥 돌아오면(던지기·"집에 와" 아님) 그 친구 화면이 꽉 찼다고 본다
const QUICK_DECLINE: Duration = Duration::from_secs(3);
/// 꽉 찬 친구 화면으로는 이만큼 동안 걸어 나가거나 던져 넘기지 않는다
const FULL_FOR: Duration = Duration::from_secs(60);
/// 거절 로그는 친구마다 이 간격에 한 번만
const DECLINE_LOG_EVERY: Duration = Duration::from_secs(5);
/// 표시 이름이 없을 때
const DEFAULT_NAME: &str = "친구";
/// 친구 코드 입력 창 라벨
pub const CODE_LABEL: &str = "code";
const CODE_SIZE: (f64, f64) = (300.0, 178.0);
/// 친구 코드로 신청했을 때 InviteSent까지 기다리는 시간 (중계 서버 왕복이 더 있다)
const CODE_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// 이름 바꾸기 창 라벨 (내 이름 / 친구 별명)
pub const NAME_LABEL: &str = "name";
const NAME_SIZE: (f64, f64) = (300.0, 150.0);
/// config 폴더의 이 파일이 있으면 첫 줄 주소의 중계 서버를 쓴다 (둘째 줄: 그 서버의 공개키 hex — 있으면 고정).
/// 비어 있거나 "off"면 중계를 쓰지 않는다
const RELAY_URL_FILE: &str = "relay_url.txt";

/// 게임 루프로 들어오는 네트워크 관련 입력
pub enum NetInput {
    Event(NetEvent),
    /// 트레이: 발견된 사람에게 친구 신청
    Invite(String),
    /// 짝맞추기 창 버튼 (true = 똑같아요, 친구 맺기 / false = 달라요·취소·창 닫기)
    PairAnswer(bool),
    SetSide { id: String, side: Side },
    Unfriend(String),
    /// "다시 친구 맺기": 친구에서 지우고 새로 신청
    Repair(String),
    /// 트레이 "집에 와"
    ComeHome,
    /// 트레이 "내 친구 코드 보기"
    ShowCode,
    /// 트레이 "친구 코드 입력…"
    EnterCode,
    /// 코드 입력 창에서 확인 (이미 `ABCD-1234` 꼴로 맞춘 코드)
    SubmitCode(String),
    /// 코드 창 "새 코드": 중계 서버에 새 친구 코드를 받는다 (예전 코드는 못 쓰게 된다)
    NewCode,
    /// 트레이 "내 이름: … ✏️": 친구들에게 보일 내 이름 바꾸기 창
    EditMyName,
    /// 트레이 친구 하위 메뉴 "이름 바꾸기…": 그 친구에게 별명 붙이기 창
    EditNickname(String),
    /// 이름 바꾸기 창에서 확인 (빈 글자 = 기본 이름으로). 누구 이름인지는 창을 연 쪽(Network::naming)이 안다
    SubmitName(String),
    /// 트레이 "놀러 보내기": 내 동물을 그 친구 화면으로 지금 보낸다 (게임 루프가 하우스에서 꺼낸 뒤 Network::send_to)
    SendTo(String),
    /// 손님 우클릭 "<친구>에게 보내기": 손님(창 label)을 내 친구(friend) 화면으로 더 보낸다 (주인이 그 친구를 알면 이어 간다)
    SendOn { label: String, friend: String },
    /// 트레이 "<친구> 펫 데리고 오기": 그 친구에게 동물을 보내 달라고 부탁한다
    BringPet(String),
}

/// 친구에게 하는 부탁 (net의 ask): 펫 데리고 오기 / 지금은 못 가요
const ASK_INVITE: &str = "invite";
const ASK_BUSY: &str = "busy";
/// 부탁한 뒤 이만큼 안에 온 "busy"만 내 동물 머리 위에 보인다
const ASK_ANSWER_WINDOW: Duration = Duration::from_secs(30);

/// 손님 우클릭 메뉴용: 지금 연결된 내 친구 (id, 보일 이름) — 게임 루프가 메뉴를 다시 계산할 때 맞춘다
static ONLINE_FRIENDS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// 손님(주인 owner_id)을 더 보낼 수 있는 친구: 연결된 내 친구 중 그 주인을 뺀 친구들 (Config::send_on_targets와 같은 규칙)
pub fn send_on_targets(owner_id: &str) -> Vec<(String, String)> {
    ONLINE_FRIENDS.lock().unwrap().iter().filter(|(id, _)| id != owner_id).cloned().collect()
}

/// 이름 바꾸기 창이 다루는 이름
#[derive(Debug, Clone, PartialEq)]
enum Naming {
    /// 친구들에게 보일 내 이름
    Me,
    /// 친구(id)에게 붙인 별명
    Friend(String),
}

/// 이름 바꾸기 창에 보이는 것 (`name-state` 이벤트, `name_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct NameView {
    /// "me" | "friend"
    pub target: &'static str,
    /// 지금 정해 둔 이름·별명 (없으면 빈 글자)
    pub current: String,
    /// 비워 두면 쓰일 이름 (컴퓨터 사용자 이름 / 친구가 정한 이름)
    pub fallback: String,
    /// 최대 글자 수
    pub max: usize,
}

/// 이름 창 첫 로드용 (커맨드는 게임 루프 밖에서 불린다)
static NAME_VIEW: Mutex<NameView> = Mutex::new(NameView { target: "me", current: String::new(), fallback: String::new(), max: 0 });

/// 친구 코드 창에 보이는 상태 (`code-state` 이벤트, `code_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct CodeView {
    /// 내 친구 코드 (중계 서버에 연결된 뒤에 생긴다)
    pub code: Option<String>,
    /// 중계 서버에 연결됨
    pub connected: bool,
    /// 중계를 쓰도록 설정됨
    pub enabled: bool,
}

/// 코드 창 첫 로드용 (커맨드는 게임 루프 밖에서 불린다)
static CODE_VIEW: Mutex<CodeView> = Mutex::new(CodeView { code: None, connected: false, enabled: false });

/// 짝맞추기 창에 보이는 상태 (`pair-state` 이벤트, `pair_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct PairView {
    /// "connecting" | "confirm" | "waiting" | "done" | "failed"
    pub phase: &'static str,
    /// 상대가 먼저 신청했음
    pub incoming: bool,
    /// 상대 표시 이름
    pub name: String,
    /// 상대 동물 이름 (받은 신청일 때)
    pub pet: String,
    /// 이모지 6개
    pub sas: Vec<String>,
    /// 상대가 이미 "똑같아요"를 눌렀음
    pub other_ok: bool,
    /// 실패 이유: "unreachable" | "declined" | "timeout" | "busy" | 친구 코드: "badcode" | "offline" | "selfcode" | "already"
    pub reason: &'static str,
}

struct Pairing {
    /// 상대 id (친구 코드로 신청하면 중계 서버가 알려 줄 때까지 비어 있다)
    peer: String,
    /// 친구 코드로 시작한 신청
    by_code: bool,
    /// net의 짝맞추기 토큰 (InviteSent/Invited 뒤에 생긴다)
    token: Option<u64>,
    view: PairView,
    /// 지금 단계가 끝나는 때 (연결 대기 10초, 확인 20초, 결과 표시 2.5초)
    deadline: Instant,
}

/// 손님이 도착한 방식 (Visit 메시지에서)
struct Arrival {
    /// 주인이 정한 들어올 끝
    enter_side: Side,
    /// 바로 전에 있던 친구 화면 (주인의 친구 id)
    via: Option<String>,
    /// 남은 그리움 시간 (건너온 손님)
    homesick_in: Option<f64>,
    /// 던져서 넘어옴 — 그 움직임 그대로 날아 들어온다 (없으면 걸어 들어온다)
    fling: Option<visit::Fling>,
}

/// 내 동물이 친구 화면에 가 있음
struct AwayVisit {
    friend: String,
    visit_id: u64,
    /// 나갔던 끝 (돌아올 때 여기서 들어온다)
    side: Side,
    /// "집에 와"를 보낸 뒤 그냥 돌아올 때
    come_home_by: Option<Instant>,
    /// 보낸 시각 (바로 돌아오면 그 친구 화면이 꽉 찬 것)
    since: Instant,
}

pub struct Network {
    net: Option<Net>,
    /// 내 표시 이름 (친구들에게 보이는 이름 — 설정의 owner_name, 없으면 컴퓨터 사용자 이름)
    name: String,
    /// 컴퓨터 사용자 이름 (내 이름을 비우면 이것으로)
    system_name: String,
    /// 이름 바꾸기 창이 지금 다루는 이름
    naming: Option<Naming>,
    /// 발견된 사람 id → 이름 (친구 포함)
    seen: BTreeMap<String, String>,
    connected: HashSet<String>,
    /// 나를 더 이상 친구로 알아보지 못하는 친구
    forgot: HashSet<String>,
    pairing: Option<Pairing>,
    away: Option<AwayVisit>,
    /// "놀러 보내기"로 그 친구(id)가 연결된 끝(side)으로 가는 중 — 그 끝으로 나가면 이 친구에게 간다
    sending: Option<(String, Side)>,
    /// 지금 내 동물을 보낼 수 있음 (메뉴 "놀러 보내기" 활성 — 바뀌면 메뉴를 다시 그린다)
    can_send: bool,
    last_menu: Option<FriendsMenu>,
    /// 트레이 메뉴를 다시 계산할 일이 있었음 (네트워크 입력·놀러 가기/돌아오기)
    menu_dirty: bool,
    /// 인증된 연결(Connected — 고정된 키를 증명한 링크의 hello)에서 들은 이름만 — 설정의 친구 이름을 맞춘다.
    /// 발견 알림·신청의 이름(인증 안 됨)은 여기 넣지 않는다
    names: HashMap<String, String>,
    /// 친구별 마지막으로 받아들인 방문 시각
    last_visit: HashMap<String, Instant>,
    /// 친구별 마지막 방문 거절 로그 시각
    last_decline_log: HashMap<String, Instant>,
    /// 꽉 찬 것 같은 친구 화면 → 이때까지는 보내지 않는다 (걸어 나가기·던져 넘기기)
    full_until: HashMap<String, Instant>,
    /// "<친구>에게 보내기"로 보낸 손님: visit_id → (그 친구, 그 친구가 연결된 끝)
    guest_sends: HashMap<String, (String, Side)>,
    /// 친구별 마지막으로 받아들인 "펫 데리고 오기" 부탁 시각 (60초에 한 번)
    invite_accepted: HashMap<String, Instant>,
    /// 친구별 내가 "펫 데리고 오기"를 부탁한 시각
    invite_asked: HashMap<String, Instant>,
    /// 지금 와 있는 손님 수 (꽉 차면 "펫 데리고 오기"를 막는다)
    guests: usize,
    /// 중계 서버를 쓰도록 설정됨 / 지금 연결됨
    relay_enabled: bool,
    relay_connected: bool,
    /// 내 친구 코드 (중계 서버가 준 것, 24시간마다 바뀐다)
    friend_code: Option<String>,
}

/// 중계 서버 주소와 고정할 공개키. 기본값은 배포된 서버 주소 + 그 공개키(다른 키를 내미는 서버는 거절).
/// config 폴더의 `relay_url.txt`가 있으면 첫 줄 주소(비었거나 "off"면 중계 안 씀), 둘째 줄이 있으면 그 키로 고정
fn relay_settings(config_dir: &Path) -> (Option<String>, Option<[u8; 32]>) {
    match std::fs::read_to_string(config_dir.join(RELAY_URL_FILE)) {
        Ok(s) => {
            let mut lines = s.lines().map(str::trim).filter(|l| !l.is_empty());
            let url = lines.next().filter(|u| !u.eq_ignore_ascii_case("off")).map(str::to_string);
            let key = lines.next().and_then(parse_relay_key);
            (url, key)
        }
        Err(_) => (Some(DEFAULT_RELAY_URL.to_string()), parse_relay_key(DEFAULT_RELAY_KEY)),
    }
}

/// net의 CodeFailed 이유 → 짝맞추기 창 실패 이유
fn code_failure(reason: &str) -> &'static str {
    match reason {
        "invalid" | "unknown" => "badcode",
        "offline" => "offline",
        "self" => "selfcode",
        "already_friend" => "already",
        "busy" => "busy",
        "full" => "full",
        _ => "unreachable",
    }
}

fn to_net_side(s: Side) -> screenpet_net::Side {
    match s {
        Side::Left => screenpet_net::Side::Left,
        Side::Right => screenpet_net::Side::Right,
    }
}

fn from_net_side(s: screenpet_net::Side) -> Side {
    match s {
        screenpet_net::Side::Left => Side::Left,
        screenpet_net::Side::Right => Side::Right,
    }
}

use visit::opposite;

fn to_net_fling(f: visit::Fling) -> screenpet_net::Fling {
    screenpet_net::Fling { height: f.height, vx: f.vx, vy: f.vy }
}

/// 받은 던지기 (다듬기는 enter_flung이 한다)
fn from_net_fling(f: screenpet_net::Fling) -> visit::Fling {
    visit::Fling { height: f.height, vx: f.vx, vy: f.vy }
}

fn personality_name(p: Personality) -> &'static str {
    match p {
        Personality::Friendly => "friendly",
        Personality::Aloof => "aloof",
        Personality::Playful => "playful",
    }
}

/// 다른 사람이 정한 글자: 제어 문자(\t, \n …)를 빼고 64자로
fn clamp(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(64).collect()
}

/// Windows 사용자 이름 (spec §1). 없으면 "친구"
fn display_name() -> String {
    let name = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let name = clamp(name.trim());
    if name.is_empty() {
        DEFAULT_NAME.to_string()
    } else {
        name
    }
}

/// 내 동물이 화면 끝으로 걸어 나가는 중
fn is_exiting(own: &Slot) -> bool {
    matches!(own.pet.state, State::Exit { .. })
}

impl Network {
    /// 네트워크를 시작한다. 설치 id가 새로 생기면 config를 고치므로 true를 돌려준다 (저장 필요).
    /// 비밀키는 config.json이 아니라 config 폴더의 `identity.key`에 있다.
    pub fn start(app: &AppHandle, config: &mut Config) -> (Network, bool) {
        let dirty = config.ensure_install_id(screenpet_net::new_install_id);
        let system_name = display_name();
        let name = config.my_name(&system_name);
        let mut network = Network {
            net: None,
            name: name.clone(),
            system_name,
            naming: None,
            seen: BTreeMap::new(),
            connected: HashSet::new(),
            forgot: HashSet::new(),
            pairing: None,
            away: None,
            sending: None,
            can_send: false,
            last_menu: None,
            menu_dirty: true,
            names: HashMap::new(),
            last_visit: HashMap::new(),
            last_decline_log: HashMap::new(),
            full_until: HashMap::new(),
            guest_sends: HashMap::new(),
            invite_accepted: HashMap::new(),
            invite_asked: HashMap::new(),
            guests: 0,
            relay_enabled: false,
            relay_connected: false,
            friend_code: None,
        };
        let state = app.state::<AppState>();
        let key = match screenpet_core::config::load_or_create_identity_key(&state.config_dir, Net::generate_static_key) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("network disabled: cannot read or create the identity key ({e})");
                return (network, dirty);
            }
        };
        let friends = config.friends.iter().map(|f| screenpet_net::Friend { id: f.id.clone(), public_key: f.public_key }).collect();
        let id = config.install_id.clone().unwrap_or_default();
        let mut net_config = NetConfig::new(id, name, key, friends);
        (net_config.relay_url, net_config.relay_key) = relay_settings(&state.config_dir);
        network.relay_enabled = net_config.relay_url.is_some();
        network.update_code_view(app);
        match Net::start(net_config) {
            Ok((net, rx)) => {
                // 네트워크 이벤트를 게임 루프 입력으로 옮긴다 (Net이 없어지면 rx가 끝나 스레드도 끝난다)
                let tx: Sender<Input> = state.tx.lock().unwrap().clone();
                thread::spawn(move || {
                    for ev in rx {
                        if tx.send(Input::Net(NetInput::Event(ev))).is_err() {
                            break;
                        }
                    }
                });
                network.net = Some(net);
            }
            Err(e) => eprintln!("network disabled: {e}"),
        }
        (network, dirty)
    }

    fn is_connected(&self, id: &str) -> bool {
        self.connected.contains(id)
    }

    /// 놀러 갈 수 있는 친구: 연결됐고, 방금 꽉 찼다고 돌려보내지 않음
    fn is_open(&self, id: &str) -> bool {
        self.is_connected(id) && !self.full_until.get(id).is_some_and(|t| Instant::now() < *t)
    }

    /// 내 동물이 친구 화면에 가 있거나 가는 중 — 하우스·동물 바꾸기를 막는다
    pub fn own_busy(&self, own: &Slot) -> bool {
        self.away.is_some() || own.pet.is_away() || is_exiting(own)
    }

    /// 매 틱 update 전에: 연결된 친구가 있는 끝으로만 나갈 수 있다. 하우스 안·가는 중·하우스 토글 중엔 나가지 않는다.
    /// 손님(slots[1..])은 주인이 아닌 연결된 친구가 있는 끝으로 더 놀러 갈 수 있다 (그 친구가 주인의 친구인지는 주인이 본다)
    pub fn set_exit_edges(&mut self, config: &Config, slots: &mut [Slot]) {
        // 손님 수가 바뀌면 "펫 데리고 오기" 활성을 다시 그린다
        let count = slots.len().saturating_sub(1);
        if count != self.guests {
            self.guests = count;
            self.menu_dirty = true;
        }
        let Some((own, guests)) = slots.split_first_mut() else { return };
        let blocked = self.net.is_none() || config.housed || own.pet.is_housed() || own.pet.is_going_home();
        let edges = if blocked { Default::default() } else { config.exit_edges(|id| self.is_open(id)) };
        own.pet.set_exit_edges(edges.left, edges.right);
        for s in guests {
            let Role::Guest(g) = &s.role else { continue };
            let edges = if self.net.is_none() { Default::default() } else { config.exit_edges(|id| id != g.owner_id && self.is_open(id)) };
            s.pet.set_exit_edges(edges.left, edges.right);
        }
    }

    /// 메뉴 "놀러 보내기" 활성: 네트워크가 있고 내 동물이 놀러 가 있거나 가는 중이 아님 (하우스 안이면 나와서 간다).
    /// 들려 있는지는 누를 때 본다 (잡을 때마다 메뉴를 다시 그리지 않는다 — 들고 있는 동안엔 메뉴를 열 수 없다)
    fn own_free(&self, own: &Slot) -> bool {
        self.net.is_some() && !self.own_busy(own)
    }

    /// "놀러 보내기": 그 친구가 연결돼 있고 내 동물이 보낼 수 있는 상태 (들려 있지 않음)
    pub fn can_send(&self, own: &Slot, id: &str) -> bool {
        self.own_free(own) && !own.pet.is_held() && self.is_connected(id)
    }

    /// "놀러 보내기": 내 동물이 그 친구가 연결된 끝으로 곧장 가서 나간다 (게임 루프가 하우스에서 먼저 꺼낸다)
    pub fn send_to(&mut self, config: &Config, own: &mut Slot, id: &str) {
        let Some(f) = config.friend(id) else { return };
        if !self.can_send(own, id) || !own.pet.send_to(f.side) {
            return;
        }
        eprintln!("sending own pet to {id} through the {:?} edge", f.side);
        self.sending = Some((id.to_string(), f.side));
        self.menu_dirty = true;
    }

    /// 손님(label)을 내 친구(friend) 화면으로 더 보낸다: 그 친구가 연결된 끝으로 곧장 가서 나간다 ("bye~!").
    /// 그 끝으로 나가면 guest_left가 Leave{next: friend}를 주인에게 보낸다 — 이어 갈지는 주인이 정한다
    fn send_on(&mut self, config: &Config, slots: &mut [Slot], label: &str, friend: &str) {
        let Some(slot) = slots.iter_mut().find(|s| s.label == label) else { return };
        let Role::Guest(g) = &slot.role else { return };
        let Some(f) = config.friend(friend) else { return };
        if g.owner_id == friend || !self.is_open(friend) || !slot.pet.send_to(f.side) {
            return;
        }
        eprintln!("sending guest {} ({}) on to {friend}", g.visit_id, g.owner_id);
        self.guest_sends.insert(g.visit_id.clone(), (friend.to_string(), f.side));
    }

    /// 내 동물이 side 끝으로 다 나갔다: 그 끝의 연결된 친구 하나에게 Visit ("놀러 보내기"였으면 그 친구에게).
    /// 아무도 없으면(그새 끊김) 바로 다시 들어온다
    pub fn own_exited(&mut self, config: &Config, own: &mut Slot, side: Side) {
        let chosen = self.sending.take().filter(|(id, s)| *s == side && self.is_connected(id)).map(|(id, _)| id);
        // 던져서 넘어갔으면 그 움직임을 실어 보낸다
        let fling = own.pet.take_fling();
        let friends = config.friends_at(side, |id| self.is_open(id));
        let (Some(net), false) = (&self.net, friends.is_empty() && chosen.is_none()) else {
            own.pet.enter(&own.world, side);
            return;
        };
        let friend = chosen.unwrap_or_else(|| friends[fastrand::usize(..friends.len())].id.clone());
        let visit_id = self.send_visit(net, own, &friend, opposite(side), None, None, fling);
        eprintln!("own pet went to visit {friend} (visit {visit_id})");
        self.away = Some(AwayVisit { friend, visit_id, side, come_home_by: None, since: Instant::now() });
        self.menu_dirty = true;
    }

    /// 내 동물을 friend 화면으로 보낸다 (enter_side 끝에서 들어온다). via = 바로 전에 있던 친구 화면. 새 visit_id를 돌려준다
    #[allow(clippy::too_many_arguments)]
    fn send_visit(&self, net: &Net, own: &Slot, friend: &str, enter_side: Side, via: Option<String>, homesick_in: Option<f64>, fling: Option<visit::Fling>) -> u64 {
        let visit_id = fastrand::u64(..);
        net.send(
            friend,
            Visit::Visit {
                visit_id,
                species: own.entry.id.clone(),
                stage: own.shown.stage.to_string(),
                scale: own.shown.scale,
                personality: personality_name(own.entry.manifest.personality).to_string(),
                owner: self.name.clone(),
                enter_side: to_net_side(enter_side),
                via,
                homesick_in,
                fling: fling.map(to_net_fling),
            },
        );
        visit_id
    }

    /// 손님이 떠났다 (side 끝으로 걸어 나감 / 손 흔들고 사라짐): 주인에게 Leave.
    /// 집 쪽이 아닌 끝으로 나갔으면 그 끝의 (주인 아닌) 연결된 친구 하나를 next로 — 주인이 그 친구에게 이어 보낸다.
    /// 그새 아무도 없으면 next 없이 (집으로)
    pub fn guest_left(&mut self, config: &Config, guest: &GuestInfo, side: Side, state: Option<visit::Guest>, fling: Option<visit::Fling>) {
        let sent = self.guest_sends.remove(&guest.visit_id);
        let (Some(net), Ok(visit_id)) = (&self.net, guest.visit_id.parse::<u64>()) else { return };
        // "<친구>에게 보내기"로 보냈으면 그 친구, 아니면 돌아다니기 규칙 (주인이 자기 친구일 때만 이어 보낸다)
        let open: Vec<&str> = config.friends_at(side, |id| id != guest.owner_id && self.is_open(id)).into_iter().map(|f| f.id.as_str()).collect();
        let sent = sent.as_ref().map(|(id, s)| (id.as_str(), *s));
        let next = visit::next_host(sent, side, state, &open, |n| fastrand::usize(..n)).map(str::to_string);
        let msg = match next {
            Some(next) => {
                eprintln!("guest {visit_id} ({}) walks on to {next}", guest.owner_id);
                Visit::Leave { visit_id, next: Some(next), side: Some(to_net_side(side)), homesick_in: state.map(|g| g.homesick_in), fling: fling.map(to_net_fling) }
            }
            // 집으로 (던져졌으면 그 움직임 그대로 주인 화면으로 날아간다)
            None => Visit::Leave { visit_id, next: None, side: None, homesick_in: None, fling: fling.map(to_net_fling) },
        };
        net.send(&guest.owner_id, msg);
    }

    /// 손님으로 가 있던 내 동물이 friend 화면에서 next 친구 화면으로 걸어 나갔다: next가 내 연결된 친구면 그리로 이어 보낸다.
    /// 아니면(모르는 사람·끊김·"집에 와" 중) false — 집으로 돌아온다
    fn forward_visit(&mut self, config: &Config, own: &Slot, next: &str, exited: Option<screenpet_net::Side>, homesick_in: Option<f64>, fling: Option<visit::Fling>) -> bool {
        let Some(a) = self.away.as_ref() else { return false };
        let (Some(net), Some(f)) = (&self.net, config.friend(next)) else { return false };
        if a.come_home_by.is_some() || a.friend == next || !self.is_connected(next) {
            return false;
        }
        // 그 친구 화면에서 이전 친구(via)가 연결된 끝을 모르면 이전 화면에서 나간 끝의 반대편으로
        let exited = exited.map(from_net_side).unwrap_or(a.side);
        let homesick_in = homesick_in.filter(|t| t.is_finite());
        let visit_id = self.send_visit(net, own, next, opposite(exited), Some(a.friend.clone()), homesick_in, fling);
        eprintln!("own pet walks on from {} to {next} (visit {visit_id})", a.friend);
        // 돌아올 땐 내 화면에서 그 친구가 연결된 끝에서 들어온다
        self.away = Some(AwayVisit { friend: next.to_string(), visit_id, side: f.side, come_home_by: None, since: Instant::now() });
        self.menu_dirty = true;
        true
    }

    /// 내 동물이 돌아온다 (나갔던 끝에서 걸어 들어온다)
    fn come_back(&mut self, own: &mut Slot, fling: Option<visit::Fling>) {
        if let Some(a) = self.away.take() {
            match fling {
                // 친구 화면에서 던져져 날아 돌아온다
                Some(f) => own.pet.enter_flung(&own.world, a.side, f),
                None => own.pet.enter(&own.world, a.side),
            }
            self.menu_dirty = true;
        }
    }

    /// 네트워크 입력 처리. config가 바뀌었으면 true (저장 필요)
    pub fn handle(&mut self, app: &AppHandle, input: NetInput, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        self.menu_dirty = true;
        let dirty = self.handle_input(app, input, config, slots);
        // net이 고정한 친구 목록이 설정과 다르면 맞춘다 (짝맞추기 완료, 상대의 연결 끊기 …)
        let dirty = self.sync_friends(config) || dirty;
        // 친구가 이름을 바꿨거나 내가 별명을 붙였으면 와 있는 손님 이름표도 바로 바꾼다
        refresh_guest_tags(app, config, slots);
        dirty
    }

    fn handle_input(&mut self, app: &AppHandle, input: NetInput, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        match input {
            NetInput::Event(ev) => self.on_event(app, ev, config, slots),
            NetInput::Invite(id) => {
                self.start_invite(app, &id, &slots[0].entry.id);
                false
            }
            NetInput::PairAnswer(accept) => {
                self.on_pair_answer(app, accept);
                false
            }
            NetInput::SetSide { id, side } => {
                // 이미 체크된 항목을 누르면 체크가 풀리므로 메뉴를 항상 다시 그린다
                self.last_menu = None;
                config.set_friend_side(&id, side)
            }
            NetInput::Unfriend(id) => self.unfriend(&id, config),
            NetInput::Repair(id) => {
                let dirty = self.unfriend(&id, config);
                self.start_invite(app, &id, &slots[0].entry.id);
                dirty
            }
            NetInput::ComeHome => {
                if let (Some(net), Some(a)) = (&self.net, self.away.as_mut()) {
                    net.send(&a.friend, Visit::ComeHome { visit_id: a.visit_id });
                    a.come_home_by.get_or_insert(Instant::now() + COME_HOME_FALLBACK);
                }
                false
            }
            NetInput::ShowCode => {
                self.show_code(app);
                false
            }
            NetInput::EnterCode => {
                self.update_code_view(app);
                show_code_window(app);
                false
            }
            NetInput::SubmitCode(code) => {
                self.start_code_invite(app, &code, &slots[0].entry.id);
                false
            }
            NetInput::NewCode => {
                // 새 코드는 FriendCode 이벤트로 온다
                if !self.net.as_ref().is_some_and(|n| n.new_friend_code()) {
                    eprintln!("cannot get a new friend code: not connected to the relay");
                }
                false
            }
            NetInput::EditMyName => {
                let view = NameView { target: "me", current: config.owner_name.clone().unwrap_or_default(), fallback: self.system_name.clone(), max: MAX_NAME_CHARS };
                self.naming = Some(Naming::Me);
                show_name_window(app, view);
                false
            }
            NetInput::EditNickname(id) => {
                let Some(f) = config.friend(&id) else { return false };
                let view = NameView { target: "friend", current: f.nickname.clone().unwrap_or_default(), fallback: f.name.clone(), max: MAX_NAME_CHARS };
                self.naming = Some(Naming::Friend(id));
                show_name_window(app, view);
                false
            }
            NetInput::SubmitName(name) => match self.naming.take() {
                Some(Naming::Me) => {
                    let dirty = config.set_owner_name(&name);
                    let shown = config.my_name(&self.system_name);
                    if shown != self.name {
                        self.name = shown;
                        // 연결된 친구들에게 바로 알린다 (다음 연결·발견 알림도 새 이름)
                        if let Some(net) = &self.net {
                            net.set_name(&self.name);
                        }
                    }
                    dirty
                }
                Some(Naming::Friend(id)) => config.set_friend_nickname(&id, &name),
                None => false,
            },
            // 게임 루프가 하우스에서 꺼낸 뒤 send_to를 직접 부른다 (여기 오지 않는다)
            NetInput::SendTo(_) => false,
            NetInput::SendOn { label, friend } => {
                self.send_on(config, slots, &label, &friend);
                false
            }
            NetInput::BringPet(id) => {
                if let (Some(net), true) = (&self.net, self.is_connected(&id) && self.guests < MAX_GUESTS) {
                    net.ask(&id, ASK_INVITE);
                    self.invite_asked.insert(id, Instant::now());
                }
                false
            }
        }
    }

    fn unfriend(&mut self, id: &str, config: &mut Config) -> bool {
        if let Some(net) = &self.net {
            // 연결돼 있었다면 Disconnected가 뒤따라 손님 보내기·귀환을 처리한다
            net.unfriend(id);
        }
        self.forgot.remove(id);
        config.remove_friend(id)
    }

    fn on_event(&mut self, app: &AppHandle, ev: NetEvent, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        match ev {
            NetEvent::PeerSeen { id, name } => {
                // 발견 알림의 이름은 인증되지 않았다 — 친구 아닌 사람 메뉴에만 쓰고 설정에는 적지 않는다
                self.seen.insert(id, clamp(&name));
            }
            NetEvent::PeerGone { id } => {
                self.seen.remove(&id);
            }
            NetEvent::Invited { from, name, pet, sas, pairing } => self.on_invited(app, from, name, pet, sas, pairing),
            NetEvent::InviteSent { to, sas, pairing } => self.on_invite_sent(app, &to, &sas, pairing),
            NetEvent::InviteAnswered { from: _, accepted, pairing } => self.on_invite_answered(app, accepted, pairing),
            NetEvent::Connected { id, name } => {
                self.connected.insert(id.clone());
                self.forgot.remove(&id);
                // 고정된 키를 증명한 링크의 이름 (방금 끝난 짝맞추기도 그 링크에서 Connected가 온다)
                self.note_name(&id, &clamp(&name));
                if let Some(p) = self.pairing.as_mut().filter(|p| p.peer == id && matches!(p.view.phase, "confirm" | "waiting")) {
                    p.view.phase = "done";
                    p.deadline = Instant::now() + RESULT_SECS;
                    self.show_pair(app);
                }
            }
            NetEvent::Renamed { id, name } => {
                // 인증된 친구 링크에서 온 이름만 (net이 확인한다) — 설정은 handle 끝의 sync_friends가 맞춘다
                if config.friend(&id).is_some() {
                    self.note_name(&id, &clamp(&name));
                }
            }
            NetEvent::Asked { from, ask } => self.on_ask(app, config, slots, &from, &ask),
            NetEvent::Disconnected { id } => {
                self.connected.remove(&id);
                // 그 친구의 손님은 손 흔들고 사라진다
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if g.owner_id == id) {
                        s.pet.go_home_now();
                    }
                }
                // 내 동물이 그 친구에게 가 있었으면 바로 돌아온다
                if self.away.as_ref().is_some_and(|a| a.friend == id) {
                    self.come_back(&mut slots[0], None);
                }
                // 상대가 친구를 끊었으면(unfriend) net은 조용히 지운다 — 설정에서는 handle 끝의 sync_friends가 지운다
            }
            NetEvent::Received { from, msg } => self.on_visit_message(app, config, slots, &from, msg),
            NetEvent::FriendForgotUs { id } => {
                self.forgot.insert(id);
            }
            NetEvent::RelayStatus { connected } => {
                self.relay_connected = connected;
                self.update_code_view(app);
            }
            NetEvent::FriendCode { code } => {
                self.friend_code = Some(code);
                self.update_code_view(app);
            }
            NetEvent::CodeRedeemed { id, .. } => {
                // 코드 주인을 찾았다 — 이제 그 id로 InviteSent가 온다
                if let Some(p) = self.pairing.as_mut().filter(|p| p.by_code && p.peer.is_empty() && p.view.phase == "connecting") {
                    p.peer = id;
                }
            }
            NetEvent::CodeFailed { reason, .. } => {
                if let Some(p) = self.pairing.as_mut().filter(|p| p.by_code && p.token.is_none() && p.view.phase == "connecting") {
                    p.view.phase = "failed";
                    p.view.reason = code_failure(&reason);
                    p.deadline = Instant::now() + RESULT_SECS;
                    self.show_pair(app);
                }
            }
        }
        false
    }

    /// 친구의 부탁 (인증된 친구 링크에서만 온다 — net이 확인한다)
    fn on_ask(&mut self, app: &AppHandle, config: &Config, slots: &mut [Slot], from: &str, ask: &str) {
        let Some(f) = config.friend(from) else { return };
        match ask {
            // "펫 데리고 오기": 보낼 수 있고 너무 잦지 않으면 말풍선을 띄우고 "놀러 보내기"처럼 그 친구에게 간다
            ASK_INVITE => {
                let available = self.can_send(&slots[0], from);
                let since = self.invite_accepted.get(from).map(|t| t.elapsed().as_secs_f64());
                match visit::answer_invite(available, since) {
                    visit::InviteAnswer::Go => {
                        self.invite_accepted.insert(from.to_string(), Instant::now());
                        let target = if slots[0].pet.is_housed() { crate::house::LABEL } else { OWN_LABEL };
                        let _ = app.emit_to(target, "pet-say-text", visit::invite_bubble(f.shown_name()));
                        eprintln!("{from} invited our pet over; going");
                        // 하우스에서 꺼내는 일은 게임 루프가 한다 (트레이 "놀러 보내기"와 같은 길)
                        let _ = app.state::<AppState>().tx.lock().unwrap().send(Input::Net(NetInput::SendTo(from.to_string())));
                    }
                    visit::InviteAnswer::Busy => {
                        if let Some(net) = &self.net {
                            net.ask(from, ASK_BUSY);
                        }
                    }
                }
            }
            // 내가 부탁했는데 지금은 못 온대요 — 내 동물 머리 위에 작게
            ASK_BUSY => {
                if self.invite_asked.remove(from).is_some_and(|t| t.elapsed() < ASK_ANSWER_WINDOW) {
                    let _ = app.emit_to(OWN_LABEL, "pet-say-text", format!("{}: busy~", f.shown_name()));
                }
            }
            // 모르는 부탁 (새 판) — 무시
            _ => {}
        }
    }

    fn note_name(&mut self, id: &str, name: &str) {
        if !name.is_empty() {
            self.names.insert(id.to_string(), name.to_string());
        }
    }

    /// 설정의 친구 목록을 net이 고정한 친구(id·공개키)에 맞춘다: 없어진 친구는 지우고, 새 친구는 더하고,
    /// 이름은 인증된 연결에서 들은 것, 없으면 저장된 이름 (새 친구는 Connected 전까지 "친구"). 바뀌었으면 true
    fn sync_friends(&mut self, config: &mut Config) -> bool {
        let Some(net) = &self.net else { return false };
        let pinned = net.friends();
        let mut dirty = false;
        let gone: Vec<String> = config.friends.iter().filter(|f| !pinned.iter().any(|p| p.id == f.id)).map(|f| f.id.clone()).collect();
        for id in gone {
            self.forgot.remove(&id);
            dirty |= config.remove_friend(&id);
        }
        for p in pinned {
            let name = self
                .names
                .get(&p.id)
                .cloned()
                .or_else(|| config.friend(&p.id).map(|f| f.name.clone()))
                .unwrap_or_else(|| DEFAULT_NAME.to_string());
            dirty |= config.upsert_friend(&p.id, &name, p.public_key);
        }
        dirty
    }

    fn on_visit_message(&mut self, app: &AppHandle, config: &Config, slots: &mut Vec<Slot>, from: &str, msg: Visit) {
        match msg {
            Visit::Visit { visit_id, species, stage, scale, owner, enter_side, via, homesick_in, fling, .. } => {
                let now = Instant::now();
                let result = if self.last_visit.get(from).is_some_and(|t| now.duration_since(*t) < VISIT_COOLDOWN) {
                    Err("too soon after the last visit".to_string())
                } else {
                    let arrival = Arrival { enter_side: from_net_side(enter_side), via, homesick_in, fling: fling.map(from_net_fling) };
                    self.welcome_guest(app, config, slots, from, visit_id, &species, &stage, scale, &owner, arrival)
                };
                match result {
                    Ok(()) => {
                        self.last_visit.insert(from.to_string(), now);
                    }
                    Err(e) => {
                        // 받을 수 없는 손님은 바로 돌려보낸다 (주인 쪽에서 원래 동물이 돌아온다)
                        if !self.last_decline_log.get(from).is_some_and(|t| now.duration_since(*t) < DECLINE_LOG_EVERY) {
                            eprintln!("declined visit {visit_id} from {from}: {e}");
                            self.last_decline_log.insert(from.to_string(), now);
                        }
                        if let Some(net) = &self.net {
                            net.send(from, Visit::leave(visit_id));
                        }
                    }
                }
            }
            Visit::Leave { visit_id, next, side, homesick_in, fling } => {
                let Some(a) = self.away.as_ref().filter(|a| a.friend == from && a.visit_id == visit_id) else { return };
                let fling = fling.map(from_net_fling);
                // 보내자마자 그냥 돌아왔다 — 그 친구 화면이 꽉 찼다고 보고 잠깐 보내지 않는다
                if next.is_none() && fling.is_none() && a.come_home_by.is_none() && a.since.elapsed() < QUICK_DECLINE {
                    self.full_until.insert(from.to_string(), Instant::now() + FULL_FOR);
                }
                let forwarded = next.is_some_and(|n| self.forward_visit(config, &slots[0], &n, side, homesick_in, fling));
                if !forwarded {
                    self.come_back(&mut slots[0], fling);
                }
            }
            Visit::ComeHome { visit_id } => {
                let id = visit_id.to_string();
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if g.owner_id == from && g.visit_id == id) {
                        s.pet.go_home_now();
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn welcome_guest(
        &self,
        app: &AppHandle,
        config: &Config,
        slots: &mut Vec<Slot>,
        from: &str,
        visit_id: u64,
        species: &str,
        stage: &str,
        scale: f64,
        owner: &str,
        arrival: Arrival,
    ) -> Result<(), String> {
        let state = app.state::<AppState>();
        let entry = state.pets.iter().find(|p| p.id == species).cloned().ok_or_else(|| format!("unknown species {species:?}"))?;
        let stage: Stage = serde_json::from_value(serde_json::Value::String(stage.to_string())).map_err(|_| format!("unknown stage {stage:?}"))?;
        let guests = slots.iter().filter(|s| !s.is_own());
        if guests.clone().count() >= MAX_GUESTS {
            return Err("too many guests".into());
        }
        if guests.into_iter().any(|s| matches!(&s.role, Role::Guest(g) if g.owner_id == from)) {
            return Err("already visiting".into());
        }
        let scale = visit::guest_scale(scale, stage.scale() * entry.manifest.size_mult(stage));
        let owner_name = config.friend(from).map(|f| f.shown_name().to_string()).unwrap_or_else(|| clamp(owner));
        let guest = GuestInfo { visit_id: visit_id.to_string(), owner_id: from.to_string(), owner_name };
        // 다른 친구 화면에서 건너왔으면: 그 친구가 연결된 끝에서 들어오고, 집 쪽은 이 화면에서 주인이 연결된 끝,
        // 그리움 시간은 이어서 센다. 바로 왔으면 들어온 끝이 집 쪽
        let hop = arrival.via.as_deref().filter(|v| *v != from);
        let side = match hop {
            Some(v) => visit::hop_enter_side(config.friend(v).map(|f| f.side), opposite(arrival.enter_side)),
            None => arrival.enter_side,
        };
        let home = match hop {
            Some(_) => config.friend(from).map_or(side, |f| f.side),
            None => side,
        };
        let seed = fastrand::u64(..);
        let mut slot = slots::spawn_guest(app, guest, entry, stage, scale, side, &slots[0].world, seed)?;
        if hop.is_some() || arrival.homesick_in.is_some() {
            let mut rng = fastrand::Rng::with_seed(seed);
            slot.pet.set_guest(home, visit::carried_homesick(arrival.homesick_in, &mut rng));
        }
        // 던져져 넘어왔으면 옆 모니터에서 날아오듯 그 높이·속도로 들어온다
        if let Some(f) = arrival.fling {
            slot.pet.enter_flung(&slot.world, side, f);
        }
        eprintln!("guest {visit_id} from {from} arrived{}", hop.map(|v| format!(" (via {v})")).unwrap_or_default());
        slots.push(slot);
        Ok(())
    }

    // ---- 짝맞추기 ----

    fn pet_name(app: &AppHandle, species: &str) -> String {
        let state = app.state::<AppState>();
        state.pets.iter().find(|p| p.id == species).map(|p| p.manifest.name.clone()).unwrap_or_else(|| "동물".to_string())
    }

    fn start_invite(&mut self, app: &AppHandle, peer: &str, own_species: &str) {
        let Some(net) = &self.net else { return };
        if self.pairing.as_ref().is_some_and(|p| !matches!(p.view.phase, "done" | "failed")) {
            // 한 번에 하나 — 진행 중인 창을 다시 보인다
            self.show_pair(app);
            return;
        }
        let name = self.seen.get(peer).cloned().unwrap_or_else(|| DEFAULT_NAME.to_string());
        // 친구가 이미 MAX_FRIENDS명이면 net이 신청하지 않는다 — 바로 알린다
        let full = net.friends().len() >= MAX_FRIENDS;
        let view = if full {
            PairView { phase: "failed", reason: "full", name, ..PairView::default() }
        } else {
            net.invite(peer, own_species);
            PairView { phase: "connecting", name, ..PairView::default() }
        };
        let wait = if full { RESULT_SECS } else { CONNECT_TIMEOUT };
        self.pairing = Some(Pairing { peer: peer.to_string(), by_code: false, token: None, view, deadline: Instant::now() + wait });
        self.show_pair(app);
    }

    /// 친구 코드로 신청: 중계 서버가 코드 주인을 찾아 주면(CodeRedeemed) 보통 신청처럼 이어진다
    fn start_code_invite(&mut self, app: &AppHandle, code: &str, own_species: &str) {
        let Some(net) = &self.net else { return };
        if self.pairing.as_ref().is_some_and(|p| !matches!(p.view.phase, "done" | "failed")) {
            self.show_pair(app);
            return;
        }
        net.redeem_code(code, own_species);
        self.pairing = Some(Pairing {
            peer: String::new(),
            by_code: true,
            token: None,
            view: PairView { phase: "connecting", name: code.to_string(), ..PairView::default() },
            deadline: Instant::now() + CODE_CONNECT_TIMEOUT,
        });
        self.show_pair(app);
    }

    /// 코드 창 상태를 맞추고 (열려 있으면) 창에 알린다
    fn update_code_view(&self, app: &AppHandle) {
        let view = CodeView { code: self.friend_code.clone(), connected: self.relay_connected, enabled: self.relay_enabled };
        *CODE_VIEW.lock().unwrap() = view.clone();
        if app.get_webview_window(CODE_LABEL).is_some() {
            let _ = app.emit_to(CODE_LABEL, "code-state", view);
        }
    }

    /// "내 친구 코드 보기": 펫이 화면에 보이면 머리 위 띠에 (누르면 복사), 아니면(하우스 안·놀러 감·연결 끊김) 코드 창에
    fn show_code(&self, app: &AppHandle) {
        let state = app.state::<AppState>();
        let pet_shown = !state.housed.load(Ordering::Relaxed)
            && !state.away.load(Ordering::Relaxed)
            && app.get_webview_window(OWN_LABEL).is_some_and(|w| w.is_visible().unwrap_or(false));
        match &self.friend_code {
            Some(code) if pet_shown => {
                let _ = app.emit_to(OWN_LABEL, "show-code", code.clone());
            }
            _ => {
                self.update_code_view(app);
                show_code_window(app);
            }
        }
    }

    fn on_invite_sent(&mut self, app: &AppHandle, to: &str, sas: &str, token: u64) {
        match self.pairing.as_mut() {
            Some(p) if p.peer == to && p.token.is_none() && p.view.phase == "connecting" => {
                p.token = Some(token);
                p.view.phase = "confirm";
                p.view.sas = sas.split(' ').map(str::to_string).collect();
                p.deadline = Instant::now() + CONFIRM_TIMEOUT;
                self.show_pair(app);
            }
            // 취소했거나 다른 신청 중 — 이 짝맞추기는 거절
            _ => {
                if let Some(net) = &self.net {
                    net.answer_invite(token, false);
                }
            }
        }
    }

    fn on_invited(&mut self, app: &AppHandle, from: String, name: String, pet: String, sas: String, token: u64) {
        let Some(net) = &self.net else { return };
        if self.pairing.as_ref().is_some_and(|p| !matches!(p.view.phase, "done" | "failed")) {
            // 한 번에 하나만 — 나머지는 거절. 서로 동시에 신청했어도 내 신청은 지킨다:
            // 들어온 신청의 id는 인증되지 않았으므로 그걸 근거로 내 짝맞추기를 거두지 않는다 (필요하면 다시 신청)
            net.answer_invite(token, false);
            return;
        }
        // 신청에 실린 이름은 인증되지 않았다 — 이 창에만 보이고 설정에는 연결(Connected) 때 이름을 적는다
        let name = clamp(&name);
        let name = if name.is_empty() { DEFAULT_NAME.to_string() } else { name };
        self.pairing = Some(Pairing {
            peer: from,
            by_code: false,
            token: Some(token),
            view: PairView {
                phase: "confirm",
                incoming: true,
                name,
                pet: Self::pet_name(app, &pet),
                sas: sas.split(' ').map(str::to_string).collect(),
                ..PairView::default()
            },
            deadline: Instant::now() + CONFIRM_TIMEOUT,
        });
        self.show_pair(app);
    }

    fn on_invite_answered(&mut self, app: &AppHandle, accepted: bool, token: u64) {
        let Some(p) = self.pairing.as_mut().filter(|p| p.token == Some(token)) else { return };
        if !matches!(p.view.phase, "confirm" | "waiting") {
            return;
        }
        if accepted {
            // 상대가 확인했다 — 나도 확인했으면 곧 Connected가 온다
            p.view.other_ok = true;
        } else {
            p.view.phase = "failed";
            p.view.reason = "declined";
            p.deadline = Instant::now() + RESULT_SECS;
        }
        self.show_pair(app);
    }

    fn on_pair_answer(&mut self, app: &AppHandle, accept: bool) {
        let Some(p) = self.pairing.as_mut() else {
            Self::hide_pair(app);
            return;
        };
        if accept {
            if p.view.phase == "confirm" {
                if let (Some(net), Some(t)) = (&self.net, p.token) {
                    net.answer_invite(t, true);
                }
                p.view.phase = "waiting";
                self.show_pair(app);
            }
            return;
        }
        // 달라요·취소·창 닫기: 확인 단계였다면 거절을 보낸다 (연결 중이었다면 뒤늦은 InviteSent를 거절한다)
        if matches!(p.view.phase, "confirm" | "waiting") {
            if let (Some(net), Some(t)) = (&self.net, p.token) {
                net.answer_invite(t, false);
            }
        }
        self.pairing = None;
        Self::hide_pair(app);
    }

    /// 짝맞추기 시간 재기
    fn tick_pairing(&mut self, app: &AppHandle, now: Instant) {
        let Some(p) = self.pairing.as_mut() else { return };
        if now < p.deadline {
            return;
        }
        match p.view.phase {
            "connecting" => {
                p.view.phase = "failed";
                p.view.reason = "unreachable";
                p.deadline = now + RESULT_SECS;
                self.show_pair(app);
            }
            "confirm" | "waiting" => {
                if let (Some(net), Some(t)) = (&self.net, p.token) {
                    net.answer_invite(t, false);
                }
                p.view.phase = "failed";
                p.view.reason = "timeout";
                p.deadline = now + RESULT_SECS;
                self.show_pair(app);
            }
            _ => {
                self.pairing = None;
                Self::hide_pair(app);
            }
        }
    }

    /// 짝맞추기 창에 지금 상태를 보이고 앞으로 가져온다 (처음이면 만든다 — 게임 루프 스레드에서만)
    fn show_pair(&self, app: &AppHandle) {
        let Some(p) = &self.pairing else { return };
        *app.state::<AppState>().pair.lock().unwrap() = p.view.clone();
        let window = match app.get_webview_window(PAIR_LABEL) {
            Some(w) => w,
            None => match build_pair_window(app) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("failed to create pairing window: {e}");
                    return;
                }
            },
        };
        let _ = app.emit_to(PAIR_LABEL, "pair-state", p.view.clone());
        if !window.is_visible().unwrap_or(false) {
            place_on_primary(app, &window, PAIR_SIZE);
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    fn hide_pair(app: &AppHandle) {
        if let Some(w) = app.get_webview_window(PAIR_LABEL) {
            let _ = w.hide();
        }
    }

    /// 매 틱: 짝맞추기 시간, "집에 와" 대비, 트레이 `친구랑 놀기` 메뉴
    pub fn tick(&mut self, app: &AppHandle, now: Instant, config: &Config, own: &mut Slot) {
        self.tick_pairing(app, now);
        // 놀러 보내는 중: 그 친구가 끊겼으면 그만두고, 동물 쪽에서 끝났으면(하우스로 감 등) 잊는다
        if let Some((id, _)) = &self.sending {
            if !self.is_connected(id) || config.friend(id).is_none() {
                own.pet.cancel_send();
                self.sending = None;
            } else if own.pet.sending().is_none() && !is_exiting(own) && !own.pet.is_away() {
                self.sending = None;
            }
        }
        let can_send = self.own_free(own);
        if can_send != self.can_send {
            self.can_send = can_send;
            self.menu_dirty = true;
        }
        if self.away.as_ref().and_then(|a| a.come_home_by).is_some_and(|t| now >= t) {
            eprintln!("no answer to come-home; coming back anyway");
            self.come_back(own, None);
        }
        // 메뉴는 네트워크 입력이나 놀러 가기/돌아오기가 있었을 때만 다시 계산한다
        if self.menu_dirty {
            self.menu_dirty = false;
            let menu = self.menu_model(config);
            if self.last_menu.as_ref() != Some(&menu) {
                crate::tray::fill_friends(app, &menu);
                self.last_menu = Some(menu);
            }
        }
    }

    fn menu_model(&self, config: &Config) -> FriendsMenu {
        let strangers = self.seen.iter().filter(|(id, _)| config.friend(id).is_none()).map(|(id, name)| (id.clone(), name.clone())).collect();
        let friends = config
            .friends
            .iter()
            .map(|f| FriendItem {
                id: f.id.clone(),
                name: f.shown_name().to_string(),
                side: f.side,
                connected: self.is_connected(&f.id),
                can_send: self.can_send && self.is_connected(&f.id),
                forgot: self.forgot.contains(&f.id),
            })
            .collect();
        // 손님 우클릭 "<친구>에게 보내기" 목록
        *ONLINE_FRIENDS.lock().unwrap() =
            config.friends.iter().filter(|f| self.is_open(&f.id)).map(|f| (f.id.clone(), f.shown_name().to_string())).collect();
        let relay = self.relay_enabled.then_some(self.relay_connected);
        let room = self.guests < MAX_GUESTS;
        FriendsMenu { available: self.net.is_some(), my_name: self.name.clone(), strangers, friends, away: self.away.is_some(), relay, room }
    }
}

/// 손님 이름표를 설정에 맞춘다 (친구가 이름을 바꿨거나 내가 별명을 붙였을 때)
fn refresh_guest_tags(app: &AppHandle, config: &Config, slots: &mut [Slot]) {
    for s in slots.iter_mut() {
        let Role::Guest(g) = &s.role else { continue };
        if let Some(f) = config.friend(&g.owner_id) {
            let name = f.shown_name().to_string();
            s.set_owner_name(app, name);
        }
    }
}

/// 작은 창을 주 모니터 가운데에 놓는다 (주 모니터를 모르면 지금 모니터 가운데)
fn place_on_primary(app: &AppHandle, window: &WebviewWindow, inner: (f64, f64)) {
    let Some(m) = app.primary_monitor().ok().flatten() else {
        let _ = window.center();
        return;
    };
    let (pos, size, dpi) = (m.position(), m.size(), m.scale_factor());
    // 숨긴 창의 바깥 크기(제목 표시줄 포함)를 모르면 안쪽 크기로 어림한다
    let (w, h) = match window.outer_size() {
        Ok(s) if s.width > 0 && s.height > 0 => (s.width as f64, s.height as f64),
        _ => (inner.0 * dpi, inner.1 * dpi),
    };
    let x = pos.x as f64 + (size.width as f64 - w) / 2.0;
    let y = pos.y as f64 + (size.height as f64 - h) / 2.0;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// 짝맞추기 창: 작고 테두리 있는 창, 항상 위. 닫기(X)는 숨기기 + 취소로 바꾼다 (창은 다시 쓴다)
fn build_pair_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, PAIR_LABEL, WebviewUrl::App("pair.html".into()))
        .title("ScreenPet 친구 맺기")
        .inner_size(PAIR_SIZE.0, PAIR_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .visible(false)
        .build()?;
    let handle = app.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = handle.state::<AppState>().tx.lock().unwrap().send(Input::Net(NetInput::PairAnswer(false)));
        }
    });
    Ok(window)
}

/// 친구 코드 창을 보이고 앞으로 가져온다 (처음이면 만든다 — 게임 루프 스레드에서만)
fn show_code_window(app: &AppHandle) {
    let window = match app.get_webview_window(CODE_LABEL) {
        Some(w) => w,
        None => match build_code_window(app) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create friend code window: {e}");
                return;
            }
        },
    };
    let _ = app.emit_to(CODE_LABEL, "code-state", CODE_VIEW.lock().unwrap().clone());
    if !window.is_visible().unwrap_or(false) {
        place_on_primary(app, &window, CODE_SIZE);
        let _ = window.show();
    }
    let _ = window.set_focus();
}

/// 친구 코드 창: 내 코드(복사) + 상대 코드 입력. 닫기(X)는 숨기기로 바꾼다 (창은 다시 쓴다)
fn build_code_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, CODE_LABEL, WebviewUrl::App("code.html".into()))
        .title("ScreenPet 친구 코드")
        .inner_size(CODE_SIZE.0, CODE_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .visible(false)
        .build()?;
    let handle = window.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
    Ok(window)
}

/// 코드 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `code-state` 이벤트)
#[tauri::command]
pub fn code_state() -> CodeView {
    CODE_VIEW.lock().unwrap().clone()
}

/// 코드 창의 "친구 신청": 코드 꼴이 맞으면 게임 루프로 보내고 창을 숨긴다 (짝맞추기 창이 이어 받는다).
/// 꼴이 틀리면 Err("invalid") — 창에 안내한다
#[tauri::command]
pub fn code_submit(window: WebviewWindow, state: tauri::State<AppState>, code: String) -> Result<(), String> {
    if window.label() != CODE_LABEL {
        return Err("wrong window".into());
    }
    let Some(code) = normalize_code(&code) else { return Err("invalid".into()) };
    let _ = state.tx.lock().unwrap().send(Input::Net(NetInput::SubmitCode(code)));
    let _ = window.hide();
    Ok(())
}

/// 코드 창의 "새 코드": 중계 서버에 새 친구 코드를 받는다 (새 코드는 `code-state`로 온다)
#[tauri::command]
pub fn code_renew(window: WebviewWindow, state: tauri::State<AppState>) {
    if window.label() == CODE_LABEL {
        let _ = state.tx.lock().unwrap().send(Input::Net(NetInput::NewCode));
    }
}

/// 코드 창의 "닫기"
#[tauri::command]
pub fn code_close(window: WebviewWindow) {
    if window.label() == CODE_LABEL {
        let _ = window.hide();
    }
}

/// 이름 바꾸기 창을 보이고 앞으로 가져온다 (처음이면 만든다 — 게임 루프 스레드에서만)
fn show_name_window(app: &AppHandle, view: NameView) {
    *NAME_VIEW.lock().unwrap() = view.clone();
    let window = match app.get_webview_window(NAME_LABEL) {
        Some(w) => w,
        None => match build_name_window(app) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("failed to create name window: {e}");
                return;
            }
        },
    };
    let title = if view.target == "me" { "ScreenPet 내 이름" } else { "ScreenPet 친구 이름" };
    let _ = window.set_title(title);
    let _ = app.emit_to(NAME_LABEL, "name-state", view);
    if !window.is_visible().unwrap_or(false) {
        place_on_primary(app, &window, NAME_SIZE);
        let _ = window.show();
    }
    let _ = window.set_focus();
}

/// 이름 바꾸기 창: 글자 칸 하나 + 저장/닫기. 닫기(X)는 숨기기로 바꾼다 (창은 다시 쓴다)
fn build_name_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, NAME_LABEL, WebviewUrl::App("name.html".into()))
        .title("ScreenPet 내 이름")
        .inner_size(NAME_SIZE.0, NAME_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .always_on_top(true)
        .skip_taskbar(false)
        .visible(false)
        .build()?;
    let handle = window.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
    Ok(window)
}

/// 이름 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `name-state` 이벤트)
#[tauri::command]
pub fn name_state() -> NameView {
    NAME_VIEW.lock().unwrap().clone()
}

/// 이름 창의 "저장": 게임 루프로 보내고 창을 숨긴다 (다듬기·길이 제한은 설정이 한다 — 빈 글자 = 기본 이름)
#[tauri::command]
pub fn name_submit(window: WebviewWindow, state: tauri::State<AppState>, name: String) {
    if window.label() != NAME_LABEL {
        return;
    }
    // 아주 긴 글자는 보내기 전에 자른다 (설정이 다시 20자로 자른다)
    let name: String = name.chars().take(256).collect();
    let _ = state.tx.lock().unwrap().send(Input::Net(NetInput::SubmitName(name)));
    let _ = window.hide();
}

/// 이름 창의 "닫기"
#[tauri::command]
pub fn name_close(window: WebviewWindow) {
    if window.label() == NAME_LABEL {
        let _ = window.hide();
    }
}
