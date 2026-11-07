//! 친구 네트워크(screenpet-net)와 앱 사이 (spec phase 3 §1–§5).
//! - 시작: 설치 id·비밀키·친구 목록으로 `Net::start` (실패해도 앱은 동작 — 로그만)
//! - 네트워크 이벤트 → 게임 루프 입력 (`Input::Net`)으로 넘겨, 설정·슬롯은 게임 루프 스레드에서만 고친다
//! - 짝맞추기 창(`pair`): 양쪽에 이모지 6개를 보이고 두 사람이 모두 확인해야 친구가 된다
//! - 놀러 가기: 내 동물이 연결된 끝으로 나가면 `Visit`, 손님이 떠나면 `Leave`, 끊기면 바로 귀환
//! - 인터넷 친구 (spec §1b): 중계 서버(기본 `wss://pet.danni.day/ws`)로 같은 망 밖의 친구와도 연결된다.
//!   친구 코드: `친구 코드 보기`(펫 머리 위 띠, 누르면 복사) / `코드 입력…`(작은 창 `code`) →
//!   코드 주인과 같은 짝맞추기(이모지 6개 확인)가 시작된다

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use screenpet_core::behavior::Personality;
use screenpet_core::brain::control_moves::{ControlInput, SteerPacer};
use screenpet_core::brain::State;
use screenpet_core::care::{Care, Stage};
use screenpet_core::config::{Config, MAX_NAME_CHARS};
use screenpet_core::dodge;
use screenpet_core::gift::{self, FriendLimit};
use screenpet_core::linked::{self, ClaimWatch};
use screenpet_core::style::PetStyle;
use screenpet_core::visit::{self, Side, MAX_GUESTS};
use screenpet_net::{normalize_code, parse_relay_key, DeviceSecret, LinkError, Net, NetConfig, NetEvent, Steer, VSide, Visit, WireCare, DEFAULT_RELAY_KEY, DEFAULT_RELAY_URL, DEVICES_MIN_VERSION, MAX_FRIENDS};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};

use crate::game_loop::Input;
use crate::gifts::GuestGift;
use crate::overlay::Form;
use crate::slots::{self, GuestInfo, Role, Slot, OWN_LABEL};
use crate::tray::{FriendItem, FriendsMenu, LadderMenu};
use crate::AppState;

/// 짝맞추기 창 라벨
pub const PAIR_LABEL: &str = "pair";
const PAIR_FORM: Form = Form { label: PAIR_LABEL, page: "pair.html", title: "ScreenPet 친구 맺기", size: (320.0, 168.0) };
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
/// 꽉 찬 친구 화면으로는 이만큼 동안 걸어 나가거나 던져 넘기지 않는다
const FULL_FOR: Duration = Duration::from_secs(60);
/// 거절 로그는 친구마다 이 간격에 한 번만
const DECLINE_LOG_EVERY: Duration = Duration::from_secs(5);
/// 조작 모드: 친구 화면에 가 있는 내 동물에게 키를 누르고 있는 동안 이 간격으로 다시 보낸다 (받는 쪽은 1초 소식이 없으면 키를 놓는다)
const STEER_KEEPALIVE: Duration = Duration::from_millis(200);
/// 표시 이름이 없을 때
use screenpet_core::config::DEFAULT_OWNER_NAME as DEFAULT_NAME;
/// 친구 코드 입력 창 라벨
pub const CODE_LABEL: &str = "code";
const CODE_FORM: Form = Form { label: CODE_LABEL, page: "code.html", title: "ScreenPet 친구 코드", size: (320.0, 178.0) };
/// 친구 코드로 신청했을 때 InviteSent까지 기다리는 시간 (중계 서버 왕복이 더 있다)
const CODE_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
/// 이름 바꾸기 창 라벨 (내 이름 / 친구 별명)
pub const NAME_LABEL: &str = "name";
const NAME_FORM: Form = Form { label: NAME_LABEL, page: "name.html", title: "ScreenPet 내 이름", size: (300.0, 150.0) };
/// 📱 내 다른 컴퓨터 연결 창 라벨 (이 컴의 연결 코드 보이기 + 저쪽 코드 넣기)
pub const LINK_LABEL: &str = "link";
const LINK_FORM: Form = Form { label: LINK_LABEL, page: "link.html", title: "ScreenPet 내 다른 컴퓨터", size: (330.0, 220.0) };
/// 코드를 넣고 저쪽 대답을 기다리는 시간
const LINK_WAIT: Duration = Duration::from_secs(15);
/// 편지 쓰기 창 라벨 ("편지 들려 보내기…")
pub const LETTER_LABEL: &str = "letter";
const LETTER_FORM: Form = Form { label: LETTER_LABEL, page: "letter.html", title: "ScreenPet 편지", size: (320.0, 170.0) };
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
    /// 친구 하위 메뉴 "📱 내 다른 컴퓨터로 연결…": 이 컴의 연결 코드를 만들어 창에 보이고, 저쪽 코드도 받는다
    LinkDevice(String),
    /// 친구 하위 메뉴 "📱 연결 끊기 (내 컴퓨터)"
    UnlinkDevice(String),
    /// 연결 창 "연결": 저쪽 컴퓨터가 보여 주는 코드 (꼴은 net이 본다). 누구와인지는 창을 연 쪽(Network::linking)이 안다
    SubmitLinkCode(String),
    /// 연결 창 "본인의 PC가 맞습니까?"의 확인(true) / 취소(false)
    ConfirmLink(bool),
    /// 연결 창 닫기 (닫기·Esc·X): 확인 단계였으면 취소한다
    LinkClose,
    /// 트레이 "집에 와"
    ComeHome,
    /// 메뉴 "친구 코드 보기"
    ShowCode,
    /// 메뉴 "코드 입력…"
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
    /// 친구 하위 메뉴 "편지 들려 보내기…": 그 친구에게 보낼 편지 쓰기 창
    EditLetter(String),
    /// 편지 창 "보내기": "놀러 보내기"처럼 보내고 Visit에 편지를 싣는다. 누구에게인지는 창을 연 쪽(Network::lettering)이 안다.
    /// 답장 창이었으면(Network::replying) 그 손님이 답장을 들고 집으로 간다
    SubmitLetter(String),
    /// 손님 우클릭 "편지 써서 돌려보내기…": 그 손님(창 label) 주인에게 보낼 답장 쓰기 창 (같은 편지 창)
    EditReply(String),
    /// 손님 우클릭 "<친구>에게 보내기": 손님(창 label)을 내 친구(friend) 화면으로 더 보낸다 (주인이 그 친구를 알면 이어 간다)
    SendOn { label: String, friend: String },
    /// 트레이 "<친구> 펫 데리고 오기": 그 친구에게 동물을 보내 달라고 부탁한다
    BringPet(String),
    /// 메뉴 "🎲 사다리타기 ▸ 새 사다리…": 판이 있으면 그 창, 없으면 설정 화면
    LadderOpen,
    /// 메뉴 "🎲 사다리타기 ▸ 지난 결과"
    LadderShowLast,
    /// 메뉴 "🎲 사다리타기 ▸ 초대 받기" (체크)
    ToggleLadderInvites,
    /// 설정 화면 "초대": 고른 친구 id, 결과 칸 미리보기
    LadderHost { friends: Vec<String>, results: Vec<crate::ladder::SlotRow> },
    /// 방장 "시작": 최종 결과 칸 (참가 인원 + 방장)
    LadderBegin(Vec<crate::ladder::SlotRow>),
    /// 초대 화면 "참가"(true) / "안 할래요"(false)
    LadderAnswer(bool),
    /// 사다리 창 닫기·그만두기
    LadderClose,
    /// 사다리 창: 내 펫이 결과 칸에 닿았다 (진짜 펫도 반응)
    LadderReact,
    /// 📱 대기 중인 컴퓨터의 "이 컴퓨터로 놀기"
    PlayHere,
    /// 📱 "다른 컴퓨터 펫 데리고 오기": 내 다른 컴퓨터에 펫을 보내 달라고 부탁한다 (그 컴퓨터가 대기 중이어도)
    BringDevicePet,
}

/// 사다리 상태 기계에 넘길 네트워크 사정 (필드를 따로 빌린다 — self.ladder는 따로 &mut)
macro_rules! ladder_ctx {
    ($s:expr, $config:expr) => {
        crate::ladder::Ctx { net: $s.net.as_ref(), config: $config, connected: &$s.connected, my_name: &$s.name }
    };
}

/// 친구에게 하는 부탁 (net의 ask): 펫 데리고 오기 / 지금은 못 가요
const ASK_INVITE: &str = "invite";
const ASK_BUSY: &str = "busy";
/// 던졌는데 받지 않았어요 (집중·하우스 중) — 던진 쪽 동물 머리 위에 작게
const ASK_NOT_NOW: &str = "not_now";
/// 부탁한 뒤 이만큼 안에 온 "busy"만 내 동물 머리 위에 보인다
const ASK_ANSWER_WINDOW: Duration = Duration::from_secs(30);
/// 친구 목록이 바뀌었을 때 내 다른 컴퓨터에 알리는 간격 (받는 쪽 예산보다 넉넉히)
const SHARE_EVERY: Duration = Duration::from_secs(5);

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
    /// 비워 두면 쓰일 이름 ("친구" / 친구가 정한 이름)
    pub fallback: String,
    /// 최대 글자 수
    pub max: usize,
}

/// 편지 쓰기 창에 보이는 것 (`letter-state` 이벤트, `letter_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct LetterView {
    /// 받는 친구 (보일 이름)
    pub name: String,
    /// 최대 글자 수
    pub max: usize,
    /// 와 있는 손님 주인에게 쓰는 답장 (손님이 들고 집으로 간다)
    pub reply: bool,
}

/// 돌아온 내 동물이 들고 온 답장 (`pet-mail` 이벤트 — 내 펫 창)
#[derive(Debug, Clone, Serialize)]
pub struct PetMail {
    /// 답장을 쓴 친구 (별명 또는 친구가 정한 이름)
    pub from: String,
    /// 다듬은 답장
    pub note: String,
}

/// 📱 연결 창에 보이는 것 (`link-state` 이벤트, `link_state` 커맨드)
#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkView {
    /// 그 친구 (보일 이름)
    pub name: String,
    /// 이 컴의 연결 코드 (없으면 빈 글자)
    pub code: String,
    /// "ready" (코드 보이기·입력) | "waiting" (저쪽 대답 기다림) | "done" | "failed" (다시 넣을 수 있다) | "blocked" (이미 다른 컴퓨터와 연결됨)
    /// | "confirm" (코드가 맞았다: 본인의 PC가 맞는지 묻는다) | "confirmed" (확인함, 저쪽 확인을 기다림) | "cancelled"
    pub phase: &'static str,
    /// 안내 한 줄
    pub note: String,
}

/// 연결 창 첫 로드용
static LINK_VIEW: Mutex<LinkView> = Mutex::new(LinkView { name: String::new(), code: String::new(), phase: "ready", note: String::new() });

/// 📱 연결 창이 다루는 친구와, 코드를 넣고 대답을 기다리는 끝
struct Linking {
    friend: String,
    until: Option<Instant>,
    /// 코드가 맞아 "본인의 PC가 맞습니까?"를 묻는 중 (net이 아직 아무것도 연결·저장하지 않았다)
    asking: bool,
}

/// 편지 창 첫 로드용
static LETTER_VIEW: Mutex<LetterView> = Mutex::new(LetterView { name: String::new(), max: 0, reply: false });

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
    /// 실패 이유: "unreachable" | "declined" | "timeout" | "busy" | 친구 코드: "badcode" | "offline" | "selfcode" | "already" | "disabled"
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
    /// 창에 한 번이라도 보였다 (받은 신청은 조용해질 때까지 미룬다)
    shown: bool,
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
    /// 주인이 들려 보낸 편지 (다듬은 것)
    note: Option<String>,
    /// 다른 친구가 밥을 줘서 커진 배율 (아직 다듬지 않은 값)
    boost: Option<f64>,
    /// 주인이 보여 주는 스타일 (이쪽에 그 그림이 없으면 동글)
    style: PetStyle,
}

/// 📱 내 다른 컴퓨터의 펫(손님)을 이 컴퓨터가 친구 화면으로 데려다줬다 (그 컴퓨터는 대기 중이라 친구와 연결이 없을 수 있다).
/// 친구가 돌려보내면 그 컴퓨터에 그대로 전한다
struct Carried {
    /// 내 다른 컴퓨터 (그 펫의 주인)
    owner: String,
    /// 지금 그 펫이 가 있는 친구
    host: String,
    /// 그 친구에게 보낸 방문 (다음 친구에게 이어 데려다줄 때 다시 쓴다)
    visit: Visit,
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
    /// 들려 보낸 편지 — 다른 친구 화면으로 이어 보낼 때도 그대로 다시 싣는다 (중간 친구는 바꿀 수 없다)
    note: Option<String>,
}

pub struct Network {
    net: Option<Net>,
    /// 내 표시 이름 (친구들에게 보이는 이름 — 설정의 owner_name, 없으면 "친구")
    name: String,
    /// 이름 바꾸기 창이 지금 다루는 이름
    naming: Option<Naming>,
    /// 발견된 사람 id → 이름 (친구 포함)
    seen: visit::SeenPeers,
    connected: HashSet<String>,
    pairing: Option<Pairing>,
    away: Option<AwayVisit>,
    /// "놀러 보내기"로 그 친구(id)가 연결된 끝(side)으로 가는 중 — 그 끝으로 나가면 이 친구에게 간다
    sending: Option<(String, Side)>,
    /// "편지 들려 보내기"로 보내는 중이면 그 편지 (sending과 같이 지운다)
    send_note: Option<String>,
    /// 편지 쓰기 창이 지금 다루는 친구 id
    lettering: Option<String>,
    /// 편지 창이 답장 창으로 열려 있으면 그 손님 (창 label, visit_id) — lettering과 둘 중 하나만
    replying: Option<(String, String)>,
    /// 손님 주인에게 실어 보낼 답장 (방문 하나에 한 통 — 손님이 집으로 떠날 때 Leave에 싣는다)
    replies: visit::Replies,
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
    /// 내 동물의 일시 확대가 끝나는 때
    boost_until: Option<Instant>,
    /// "<친구>에게 보내기"로 보낸 손님: visit_id → (그 친구, 그 친구가 연결된 끝)
    guest_sends: HashMap<String, (String, Side)>,
    /// 친구별 마지막으로 받아들인 "펫 데리고 오기" 부탁 시각 (60초에 한 번)
    invite_accepted: HashMap<String, Instant>,
    /// 누구든 마지막으로 받아들인 "펫 데리고 오기" 시각 (2분에 한 번)
    invite_accepted_any: Option<Instant>,
    /// 친구별 내가 "펫 데리고 오기"를 부탁한 시각
    invite_asked: HashMap<String, Instant>,
    /// 친구별로 지난번 "지금은 못 받아요"가 온 때 (여러 개가 돌아와도 말풍선은 한 번)
    not_now_at: HashMap<String, Instant>,
    /// 지금 와 있는 손님 수 (꽉 차면 "펫 데리고 오기"를 막는다)
    guests: usize,
    /// 중계 서버를 쓰도록 설정됨 / 지금 연결됨
    relay_enabled: bool,
    relay_connected: bool,
    /// 내 친구 코드 (중계 서버가 준 것, 24시간마다 바뀐다)
    friend_code: Option<String>,
    /// 조작 모드: 마지막으로 친구 화면의 내 동물에게 보낸 키 (visit_id, 켜짐, 키 비트, 보낸 시각)
    steer_pacer: SteerPacer,
    /// 친구마다(친구 id별) 선물을 받는 횟수 제한 — 다시 연결해도 채워지지 않는다
    gift_limit: FriendLimit,
    /// 마지막으로 내 동물에게 선물을 들려 보낸 시각
    gift_sent: Option<Instant>,
    /// 방금 들어온 손님이 들고 온 선물 (게임 루프가 꺼내 간다)
    guest_gifts: Vec<GuestGift>,
    /// 사다리타기 (판 하나와 창)
    ladder: crate::ladder::Ladders,
    /// (내 펫이 지금 손님을 받지 않음 — 집중 중·하우스 안, 사다리 판이 있음) — 바뀌면 메뉴의 사다리 항목을 다시 맞춘다
    own_refuses: (bool, bool),
    /// 집중 중이거나 전체 화면 앱이 떠 있음 — 받은 친구 신청 창을 미룬다
    quiet: bool,
    /// 친구 목록이 바뀌어 내 다른 컴퓨터들에게 다시 알릴 것이 있다 (마지막으로 알린 때)
    share_due: bool,
    last_share: Option<Instant>,
    /// 📱 연결 창 (열었던 친구)
    linking: Option<Linking>,
    /// 친구별 연결된 때 (한 사람의 두 컴퓨터가 다 연결돼 있으면 나중 것이 보낼 곳)
    connected_at: HashMap<String, Instant>,
    started: Instant,
    /// 📱 이 컴퓨터는 대기 중 (내 다른 컴퓨터에서 논다): 친구와 연결하지 않는다
    standby: bool,
    /// 📱 대기 중인 내 다른 컴퓨터 (이 컴퓨터가 논다) — 내 펫·물건을 그리로 보내지 않는다
    device_standby: Option<String>,
    /// 📱 대기 중에 다시 쓰기 시작했는지
    claim_watch: ClaimWatch,
    /// 📱 친구 화면으로 데려다준 내 다른 컴퓨터의 펫: visit_id →
    carried: HashMap<u64, Carried>,
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
        "disabled" => "disabled",
        _ => "unreachable",
    }
}

/// 보낼 끝: (예전 판도 아는 왼쪽/오른쪽, 위/아래면 vside). 위/아래는 예전 판에겐 왼쪽으로 보인다 (그 판은 왼쪽에서 걸어 들어온다)
pub(crate) fn to_net_side(s: Side) -> (screenpet_net::Side, Option<VSide>) {
    match s {
        Side::Left => (screenpet_net::Side::Left, None),
        Side::Right => (screenpet_net::Side::Right, None),
        Side::Up => (screenpet_net::Side::Left, Some(VSide::Up)),
        Side::Down => (screenpet_net::Side::Left, Some(VSide::Down)),
    }
}

/// 받은 끝: vside가 있으면 위/아래, 없으면 왼쪽/오른쪽
pub(crate) fn from_net_side(s: screenpet_net::Side, v: Option<VSide>) -> Side {
    match (v, s) {
        (Some(VSide::Up), _) => Side::Up,
        (Some(VSide::Down), _) => Side::Down,
        (None, screenpet_net::Side::Left) => Side::Left,
        (None, screenpet_net::Side::Right) => Side::Right,
    }
}

use visit::opposite;

pub(crate) fn to_net_fling(f: visit::Fling) -> screenpet_net::Fling {
    screenpet_net::Fling { height: f.height, vx: f.vx, vy: f.vy }
}

/// 받은 던지기 (다듬기는 enter_flung이 한다)
pub(crate) fn from_net_fling(f: screenpet_net::Fling) -> visit::Fling {
    visit::Fling { height: f.height, vx: f.vx, vy: f.vy }
}

fn personality_name(p: Personality) -> &'static str {
    match p {
        Personality::Friendly => "friendly",
        Personality::Aloof => "aloof",
        Personality::Playful => "playful",
    }
}

/// 다른 사람이 정한 글자: 보이지 않는 글자(제어·방향·폭 없는 글자)를 빼고 64자로
fn clamp(s: &str) -> String {
    // 제어 문자·방향 바꾸기·폭 없는 글자를 빼고, 앞뒤 공백을 지운다 (공백뿐이면 빈 글자 → 기본 이름)
    screenpet_core::config::clean_text(s, 64)
}

/// 내 동물이 화면 끝으로 걸어 나가거나, 보내져 손 흔들고 사라지는 중
fn is_exiting(own: &Slot) -> bool {
    matches!(own.pet.state, State::Exit { .. } | State::Farewell { .. })
}

impl Network {
    /// 네트워크를 시작한다. 설치 id가 새로 생기면 config를 고치므로 true를 돌려준다 (저장 필요).
    /// 비밀키는 config.json이 아니라 config 폴더의 `identity.key`에 있다.
    pub fn start(app: &AppHandle, config: &mut Config) -> (Network, bool) {
        let dirty = config.ensure_install_id(screenpet_net::new_install_id);
        let name = config.my_name();
        let mut network = Network {
            net: None,
            name: name.clone(),
            naming: None,
            seen: visit::SeenPeers::default(),
            connected: HashSet::new(),
            pairing: None,
            away: None,
            sending: None,
            send_note: None,
            lettering: None,
            replying: None,
            replies: visit::Replies::default(),
            can_send: false,
            last_menu: None,
            menu_dirty: true,
            names: HashMap::new(),
            last_visit: HashMap::new(),
            last_decline_log: HashMap::new(),
            full_until: HashMap::new(),
            boost_until: None,
            guest_sends: HashMap::new(),
            invite_accepted: HashMap::new(),
            invite_accepted_any: None,
            invite_asked: HashMap::new(),
            not_now_at: HashMap::new(),
            guests: 0,
            relay_enabled: false,
            relay_connected: false,
            friend_code: None,
            steer_pacer: SteerPacer::default(),
            gift_limit: FriendLimit::default(),
            gift_sent: None,
            guest_gifts: Vec::new(),
            ladder: crate::ladder::Ladders::default(),
            own_refuses: (false, false),
            quiet: false,
            share_due: false,
            last_share: None,
            linking: None,
            connected_at: HashMap::new(),
            started: Instant::now(),
            standby: false,
            device_standby: None,
            claim_watch: ClaimWatch::default(),
            carried: HashMap::new(),
        };
        let state = app.state::<AppState>();
        // 설정을 못 읽은 실행: 친구 목록이 빈 채로 나가면 친구들이 "나를 잊었다"고 보고 나를 지운다
        if config.keep_file {
            eprintln!("network disabled: the config file could not be read");
            return (network, dirty);
        }
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
        // 📱 연결된 내 다른 컴퓨터와 나눈 비밀, 친구가 소개한 기기, 아직 확인 못 한 친구 (net이 다시 확인한다)
        net_config.device = config.linked_device().map(|(id, secret)| (id.to_string(), DeviceSecret(secret)));
        net_config.introduced = config.friends.iter().filter_map(|f| Some((f.id.clone(), f.via.clone()?))).collect();
        net_config.pending = config.friends.iter().filter(|f| f.pending).map(|f| f.id.clone()).collect();
        net_config.pending_added = config.friends.iter().filter(|f| f.pending).filter_map(|f| Some((f.id.clone(), f.pending_since?))).collect();
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

    /// 💩 똥피하기 새 최고 기록을 연결된 친구들에게 알린다
    pub fn share_dodge_best(&self, best: u32) {
        let Some(net) = &self.net else { return };
        for id in &self.connected {
            net.ask(id, &dodge::score_ask(best));
        }
    }

    /// 내 동물·물건을 보낼 수 있는 친구: 연결됐고, 대기 중인 내 다른 컴퓨터가 아님 (물건은 손님 자리와 상관없이 던져 보낼 수 있다)
    pub fn is_connected(&self, id: &str) -> bool {
        linked::playable(id, self.connected.contains(id), self.device_standby.as_deref())
    }

    /// 링크가 살아 있음 (대기 중인 내 다른 컴퓨터도) — 목록 나누기, 그 컴퓨터 펫 데려오기, 메뉴의 ●
    fn link_up(&self, id: &str) -> bool {
        self.connected.contains(id)
    }

    /// 📱 연결된 내 다른 컴퓨터
    fn is_my_device(config: &Config, id: &str) -> bool {
        config.friend(id).is_some_and(|f| f.device_secret.is_some())
    }

    /// 📱 1초에 한 번 (게임 루프): 쓴 지 얼마나 됐는지 net에 알리고, 대기 중에 다시 쓰기 시작했으면 이 컴퓨터로 논다
    pub fn tick_presence(&mut self, secs: f64, idle: Option<f64>) {
        let Some(net) = &self.net else { return };
        if let Some(i) = idle.filter(|i| i.is_finite() && *i >= 0.0) {
            net.set_idle(Duration::from_secs_f64(i.min(1e9)));
        }
        if self.claim_watch.observe(secs, idle, self.standby) {
            eprintln!("this computer is in use again: playing here");
            net.claim_active();
        }
    }

    /// 📱 내 펫과 모든 펫의 돌보기 상태를 내 다른 컴퓨터와 나눈다 (`pet`의 지금 상태는 `care`).
    /// net은 이 컴퓨터가 노는 동안만 모아서 보낸다. edit = 사용자가 바꿨다 (먹이기·펫 바꾸기·성장 멈추기·처음부터):
    /// 쉬는 중이면 그것만 알리고 이 컴퓨터로 논다 (저쪽의 넘겨주기가 덮지 않게). 쉬는 중의 그냥 새로 고침은 알리지 않는다
    pub fn share_state(&self, config: &Config, pet: &str, care: &Care, edit: bool) {
        let Some(net) = self.net.as_ref().filter(|_| config.linked_device().is_some()) else { return };
        if self.standby && !edit {
            return;
        }
        let mut all = config.care.clone();
        all.insert(pet.to_string(), care.clone());
        let list = linked::shared_care(&all)
            .into_iter()
            .map(|c| WireCare { pet: c.pet, stage: c.stage, stamina: c.stamina, fullness: c.fullness, locked: c.locked })
            .collect();
        net.share_state(pet, list);
        if self.standby {
            eprintln!("changed the pet on this standby computer: playing here");
            net.claim_active();
        }
    }

    /// 📱 이 컴퓨터가 대기가 됐다: 친구 화면에 가 있는 내 펫을 부른다 (친구 링크는 잠깐 뒤 닫힌다).
    /// 내 다른 컴퓨터에 가 있으면 그대로 둔다 (그 링크는 남는다)
    fn call_home_from_friends(&mut self, config: &Config) {
        let (Some(net), Some(a)) = (&self.net, self.away.as_mut()) else { return };
        if Self::is_my_device(config, &a.friend) {
            return;
        }
        eprintln!("this computer goes on standby: calling the pet home from {}", a.friend);
        net.send(&a.friend, Visit::ComeHome { visit_id: a.visit_id });
        a.come_home_by.get_or_insert(Instant::now() + COME_HOME_FALLBACK);
    }

    /// 🧹 쓸어 모은 더미를 그 끝의 친구 한 명에게 흩날려 보낸다 (친구 쪽에서 반대편 끝으로 들어온다)
    pub fn flurry_to(&self, config: &Config, side: Side, kind: screenpet_core::weather::Weather, fling: visit::Fling) {
        let Some(net) = &self.net else { return };
        let friends = config.friends_at(side, |id| self.is_connected(id));
        if friends.is_empty() {
            return;
        }
        let friend = &friends[fastrand::usize(..friends.len())].id;
        let (side, vside) = to_net_side(visit::opposite(side));
        net.toss(friend, screenpet_net::Toss { kind: screenpet_core::weather::flurry_kind(kind), side, vside, fling: Some(to_net_fling(fling)), note: None, bounced: false, scale: None, look: None });
    }

    /// 집에 돌아온 내 동물의 일시 확대가 끝날 때가 됐으면 원래 크기로 (친구 화면에 가 있으면 돌아온 뒤에)
    pub fn tick_boost(&mut self, app: &AppHandle, own: &mut Slot, now: Instant) {
        if self.away.is_none() && self.boost_until.is_some_and(|t| now >= t) {
            self.boost_until = None;
            own.set_boost(app, 1.0);
            eprintln!("own pet is back to its usual size");
        }
    }

    /// 받지 않는 물건을 던진 친구에게 되돌린다: 그 친구 화면에서 나간 끝으로 다시 날아 들어간다 (포스트잇도 그대로)
    pub fn bounce_toss(&self, friend: &str, toss: screenpet_net::Toss) {
        let Some(net) = &self.net else { return };
        let (side, vside) = to_net_side(visit::opposite(from_net_side(toss.side, toss.vside)));
        net.toss(friend, screenpet_net::Toss { side, vside, bounced: true, ..toss });
        net.ask(friend, ASK_NOT_NOW);
    }

    /// 물건을 친구 화면으로 던진다 (objects.rs). 연결이 없으면 버려진다
    pub fn toss_to(&self, friend: &str, toss: screenpet_net::Toss) {
        if let Some(net) = &self.net {
            net.toss(friend, toss);
        }
    }

    /// 놀러 갈 수 있는 친구: 연결됐고, 방금 꽉 찼다고 돌려보내지 않음
    fn is_open(&self, id: &str) -> bool {
        self.is_connected(id) && !self.full_until.get(id).is_some_and(|t| Instant::now() < *t)
    }

    /// 내 동물이 지금 가 있는 친구 id (다른 친구에게 이어 가면 그 친구로 바뀐다 — 이어 보내기는 늘 내가 한다)
    pub fn away_friend(&self) -> Option<&str> {
        self.away.as_ref().map(|a| a.friend.as_str())
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
        own.pet.set_vertical_edges(edges.up, edges.down);
        for s in guests {
            let Role::Guest(g) = &s.role else { continue };
            let edges = if self.net.is_none() { Default::default() } else { config.exit_edges(|id| id != g.owner_id && self.is_open(id)) };
            s.pet.set_exit_edges(edges.left, edges.right);
            s.pet.set_vertical_edges(edges.up, edges.down);
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

    /// "놀러 보내기": 내 동물이 그 자리에서 손 흔들고 뿅 사라진다 — 그 친구가 연결된 끝으로 나간 것으로 친다
    /// (게임 루프가 하우스에서 먼저 꺼낸다). 가기 시작했으면 true.
    /// note = "편지 들려 보내기"의 편지 (다듬은 것) — 그 친구에게 가는 Visit에 실린다
    pub fn send_to(&mut self, config: &Config, own: &mut Slot, id: &str, note: Option<String>) -> bool {
        let Some(f) = config.friend(id) else { return false };
        if !self.can_send(own, id) || !own.pet.send_to(f.side, note.is_some()) {
            return false;
        }
        eprintln!("sending own pet to {id} through the {:?} edge{}", f.side, if note.is_some() { " with a note" } else { "" });
        self.sending = Some((id.to_string(), f.side));
        self.send_note = note;
        self.menu_dirty = true;
        true
    }

    /// 편지 창 "보내기": 창을 연 친구 id와 다듬은 편지 (비었으면 편지 없이 그냥 놀러 보내기). 창이 다루던 친구가 없으면 None.
    /// 창이 다루던 친구는 그대로 둔다 — 못 보내면 창이 남아 다시 누를 수 있다
    pub fn letter_target(&self, text: &str) -> Option<(String, Option<String>)> {
        let id = self.lettering.clone()?;
        Some((id, visit::sanitize_note(text)))
    }

    /// 편지를 보냈으면 창을 닫고, 못 보냈으면 창을 둔 채 알린다 (쓴 글이 사라지지 않게)
    pub fn finish_letter(&mut self, app: &AppHandle, sent: bool) {
        if sent {
            self.lettering = None;
            self.replying = None;
            if let Some(w) = app.get_webview_window(LETTER_LABEL) {
                let _ = w.hide();
            }
        } else {
            let _ = app.emit_to(LETTER_LABEL, "letter-failed", ());
        }
    }

    /// 편지 창이 답장 창으로 열려 있음 ("보내기"가 submit_reply로 간다)
    pub fn is_replying(&self) -> bool {
        self.replying.is_some()
    }

    /// 답장 창 "보내기": 그 손님이 아직 그 방문으로 와 있으면 답장(방문 하나에 한 통, 비면 없이)을 맡기고
    /// 그 자리에서 손 흔들고("bye~! 💌") 뿅 집으로 — 떠날 때 guest_left가 Leave에 싣는다
    /// 그새 떠난 손님이면 false (답장을 받을 사람이 없다)
    pub fn submit_reply(&mut self, text: &str, slots: &mut [Slot]) -> bool {
        let Some((label, visit_id)) = self.replying.clone() else { return false };
        let Some(slot) = slots.iter_mut().find(|s| s.label == label && matches!(&s.role, Role::Guest(g) if g.visit_id == visit_id)) else { return false };
        let wrote = self.replies.write(&visit_id, text);
        eprintln!("sending guest {visit_id} home{}", if wrote { " with a reply" } else { "" });
        self.guest_sends.remove(&visit_id);
        if !(wrote && slot.pet.send_home_with_reply()) {
            slot.pet.go_home_now();
        }
        true
    }

    /// 매 틱: Leave 없이 사라진 손님의 답장은 버린다 (평소엔 guest_left가 꺼낸다 — 비어 있으면 아무 일도 없다)
    pub fn forget_gone_replies(&mut self, slots: &[Slot]) {
        self.replies.retain(|id| slots.iter().any(|s| matches!(&s.role, Role::Guest(g) if g.visit_id == id)));
    }

    /// 손님을 주인에게 돌려보낸다 (손님 우클릭 "<주인>에게 돌려보내기"): 걸어 둔 "<친구>에게 보내기"는 지운다
    pub fn send_guest_home(&mut self, slot: &mut Slot) {
        if let Role::Guest(g) = &slot.role {
            self.guest_sends.remove(&g.visit_id);
        }
        slot.pet.go_home_now();
    }

    /// 걸어 둔 "<친구>에게 보내기" 중 drop(주인 id, 보낼 친구 id)이 참인 것을 지우고 그 손님은 그만 간다
    fn drop_guest_sends(&mut self, slots: &mut [Slot], drop: impl Fn(&str, &str) -> bool) {
        for s in slots.iter_mut() {
            let Role::Guest(g) = &s.role else { continue };
            if self.guest_sends.get(&g.visit_id).is_some_and(|(to, _)| drop(&g.owner_id, to)) {
                self.guest_sends.remove(&g.visit_id);
                s.pet.cancel_send();
            }
        }
    }

    /// 손님(label)을 내 친구(friend) 화면으로 더 보낸다: 그 자리에서 손 흔들고("bye~!") 뿅 사라진다 (그 친구가 연결된 끝으로 나간 것).
    /// 그 끝으로 나가면 guest_left가 Leave{next: friend}를 주인에게 보낸다 — 이어 갈지는 주인이 정한다
    fn send_on(&mut self, config: &Config, slots: &mut [Slot], label: &str, friend: &str) {
        let Some(slot) = slots.iter_mut().find(|s| s.label == label) else { return };
        let Role::Guest(g) = &slot.role else { return };
        let Some(f) = config.friend(friend) else { return };
        if g.owner_id == friend || !self.is_open(friend) || !slot.pet.send_to(f.side, false) {
            return;
        }
        eprintln!("sending guest {} ({}) on to {friend}", g.visit_id, g.owner_id);
        self.guest_sends.insert(g.visit_id.clone(), (friend.to_string(), f.side));
    }

    /// 내 동물이 side 끝으로 다 나갔다: 그 끝의 연결된 친구 하나에게 Visit ("놀러 보내기"였으면 그 친구에게).
    /// 아무도 없으면(그새 끊김) 바로 다시 들어온다
    pub fn own_exited(&mut self, config: &Config, own: &mut Slot, side: Side) {
        let chosen = self.sending.take().filter(|(id, s)| *s == side && self.is_connected(id)).map(|(id, _)| id);
        // 편지는 그 친구에게 가는 길에만 싣는다
        let note = self.send_note.take().filter(|_| chosen.is_some());
        // 던져서 넘어갔으면 그 움직임을 실어 보낸다
        let fling = own.pet.take_fling();
        let friends = config.friends_at(side, |id| self.is_open(id));
        let (Some(net), false) = (&self.net, friends.is_empty() && chosen.is_none()) else {
            own.pet.enter(&own.world, side);
            return;
        };
        let friend = chosen.unwrap_or_else(|| friends[fastrand::usize(..friends.len())].id.clone());
        // 가끔 선물을 들려 보낸다 (이어 가는 방문에는 다시 싣지 않는다)
        let now = Instant::now();
        let ready = !self.gift_sent.is_some_and(|t| now.duration_since(t) < gift::VISIT_GIFT_EVERY);
        let gift = if ready { gift::roll_visit_gift(&own.entry.id, &mut fastrand::Rng::new()) } else { None };
        let visit_id = self.send_visit(net, own, &friend, opposite(side), None, None, fling, note.clone(), gift.map(|g| g.wire()));
        if gift.is_some() {
            self.gift_sent = Some(now);
        }
        eprintln!("own pet went to visit {friend} (visit {visit_id})");
        self.away = Some(AwayVisit { friend, visit_id, side, come_home_by: None, since: Instant::now(), note });
        self.menu_dirty = true;
    }

    /// 내 동물을 friend 화면으로 보낸다 (enter_side 끝에서 들어온다). via = 바로 전에 있던 친구 화면. 새 visit_id를 돌려준다
    #[allow(clippy::too_many_arguments)]
    fn send_visit(&self, net: &Net, own: &Slot, friend: &str, enter_side: Side, via: Option<String>, homesick_in: Option<f64>, fling: Option<visit::Fling>, note: Option<String>, gift: Option<String>) -> u64 {
        let visit_id = fastrand::u64(..);
        let (enter_side, vside) = to_net_side(enter_side);
        net.send(
            friend,
            Visit::Visit {
                visit_id,
                species: own.entry.id.clone(),
                stage: own.shown.stage.name().to_string(),
                // 동글일 때의 크기: 예전 버전은 동글로 그리고, 새 버전은 픽셀이면 이쪽 화면에 맞춰 다시 잰다
                scale: own.shown.base,
                personality: personality_name(own.entry.manifest.personality).to_string(),
                owner: self.name.clone(),
                enter_side,
                vside,
                via,
                homesick_in,
                fling: fling.map(to_net_fling),
                note,
                gift,
                style: own.shown.style.to_wire(),
                boost: (own.boost > 1.0).then_some(own.boost),
                of: None,
            },
        );
        visit_id
    }

    /// 손님이 떠났다 (side 끝으로 걸어 나감 / 손 흔들고 사라짐): 주인에게 Leave.
    /// 집 쪽이 아닌 끝으로 나갔으면 그 끝의 (주인 아닌) 연결된 친구 하나를 next로 — 주인이 그 친구에게 이어 보낸다.
    /// 그새 아무도 없으면 next 없이 (집으로)
    pub fn guest_left(&mut self, config: &Config, slot: &Slot, guest: &GuestInfo, side: Side, state: Option<visit::Guest>, fling: Option<visit::Fling>) {
        let boost = (slot.boost > 1.0).then_some(slot.boost);
        let sent = self.guest_sends.remove(&guest.visit_id);
        // 이 화면 주인이 쓴 답장은 집으로 갈 때만 싣는다 (다음 친구에게 가면 버린다)
        let reply = self.replies.take(&guest.visit_id);
        let (true, Ok(visit_id)) = (self.net.is_some(), guest.visit_id.parse::<u64>()) else { return };
        // "<친구>에게 보내기"로 보냈으면 그 친구, 아니면 돌아다니기 규칙 (주인이 자기 친구일 때만 이어 보낸다)
        let open: Vec<&str> = config.friends_at(side, |id| id != guest.owner_id && self.is_open(id)).into_iter().map(|f| f.id.as_str()).collect();
        let sent = sent.as_ref().map(|(id, s)| (id.as_str(), *s));
        let next = visit::next_host(sent, side, state, &open, |n| fastrand::usize(..n)).map(str::to_string);
        // 📱 내 다른 컴퓨터의 펫이 친구에게 간다: 그 컴퓨터는 대기 중이라 친구와 연결이 없을 수 있다 — 이 컴퓨터가 데려다준다
        if let Some(n) = next.as_deref() {
            if self.carry_for_device(config, slot, guest, visit_id, n, side, state, fling, boost) {
                return;
            }
        }
        let msg = match next {
            Some(next) => {
                eprintln!("guest {visit_id} ({}) walks on to {next}", guest.owner_id);
                let (side, vside) = to_net_side(side);
                Visit::Leave { visit_id, next: Some(next), side: Some(side), vside, homesick_in: state.map(|g| g.homesick_in), fling: fling.map(to_net_fling), note: None, boost }
            }
            // 집으로 (던져졌으면 그 움직임 그대로 주인 화면으로 날아간다)
            // 나간 끝을 실어 주인이 반대편으로 받게 한다 (예전 주인은 무시하고 나갔던 끝으로 받는다)
            None => {
                let (side, vside) = to_net_side(side);
                Visit::Leave { visit_id, next: None, side: Some(side), vside, homesick_in: None, fling: fling.map(to_net_fling), note: reply, boost }
            }
        };
        // 주인의 다른 컴퓨터가 데려온 손님이면 그 컴퓨터로 (📱 한 사람)
        let Some(net) = &self.net else { return };
        net.send(config.reach(&guest.owner_id, |id| self.link_up(id)), msg);
    }

    /// 📱 내 다른 컴퓨터(손님 주인)의 펫을 친구 next에게 이 컴퓨터가 데려다준다 (`of` = 그 컴퓨터).
    /// 받는 친구는 한 사람의 두 컴퓨터로 알고 내 이름표를 단다. 친구가 돌려보내면 on_visit_message가 그 컴퓨터로 전한다
    #[allow(clippy::too_many_arguments)]
    fn carry_for_device(&mut self, config: &Config, slot: &Slot, guest: &GuestInfo, visit_id: u64, next: &str, side: Side, state: Option<visit::Guest>, fling: Option<visit::Fling>, boost: Option<f64>) -> bool {
        let Some(net) = &self.net else { return false };
        let new = net.peer_version(next).is_some_and(|v| v >= DEVICES_MIN_VERSION);
        if !new || next == guest.owner_id || !Self::is_my_device(config, &guest.owner_id) {
            return false;
        }
        let (enter_side, vside) = to_net_side(opposite(side));
        let visit = Visit::Visit {
            visit_id,
            species: slot.entry.id.clone(),
            stage: slot.shown.stage.name().to_string(),
            scale: slot.shown.base,
            personality: personality_name(slot.entry.manifest.personality).to_string(),
            owner: self.name.clone(),
            enter_side,
            vside,
            via: None,
            homesick_in: state.map(|g| g.homesick_in).filter(|t| t.is_finite()),
            fling: fling.map(to_net_fling),
            note: guest.note.clone(),
            gift: None,
            style: slot.shown.style.to_wire(),
            boost,
            of: Some(guest.owner_id.clone()),
        };
        net.send(next, visit.clone());
        eprintln!("carrying my other computer's pet {visit_id} on to {next}");
        self.carried.insert(visit_id, Carried { owner: guest.owner_id.clone(), host: next.to_string(), visit });
        true
    }

    /// 📱 데려다준 내 다른 컴퓨터의 펫을 그 친구(from)가 돌려보냈다: 다음 친구가 있으면 이어 데려다주고, 아니면 그 컴퓨터로 전한다.
    /// 이 방문이 아니면 false
    #[allow(clippy::too_many_arguments)]
    fn carried_left(&mut self, from: &str, visit_id: u64, next: Option<String>, side: Option<screenpet_net::Side>, vside: Option<VSide>, homesick_in: Option<f64>, fling: Option<screenpet_net::Fling>, note: Option<String>, boost: Option<f64>) -> bool {
        if !self.carried.get(&visit_id).is_some_and(|c| c.host == from) {
            return false;
        }
        let Some(mut c) = self.carried.remove(&visit_id) else { return false };
        let Some(net) = &self.net else { return true };
        if let Some(n) = next.filter(|n| *n != c.owner && self.is_open(n) && net.peer_version(n).is_some_and(|v| v >= DEVICES_MIN_VERSION)) {
            if let Visit::Visit { enter_side, vside: vs, via, homesick_in: h, fling: f, boost: b, .. } = &mut c.visit {
                let exited = side.map_or(Side::Right, |s| from_net_side(s, vside));
                (*enter_side, *vs) = to_net_side(opposite(exited));
                *via = Some(from.to_string());
                *h = homesick_in.filter(|t| t.is_finite());
                *f = fling;
                *b = boost.or(*b);
            }
            net.send(&n, c.visit.clone());
            eprintln!("carrying my other computer's pet {visit_id} on from {from} to {n}");
            self.carried.insert(visit_id, Carried { host: n, ..c });
            return true;
        }
        eprintln!("my other computer's pet {visit_id} goes home from {from}");
        net.send(&c.owner, Visit::Leave { visit_id, next: None, side, vside, homesick_in: None, fling, note, boost });
        true
    }

    /// 손님으로 가 있던 내 동물이 friend 화면에서 next 친구 화면으로 걸어 나갔다: next가 내 연결된 친구면 그리로 이어 보낸다.
    /// 아니면(모르는 사람·끊김·"집에 와" 중) false — 집으로 돌아온다
    fn forward_visit(&mut self, config: &Config, own: &Slot, next: &str, exited: Option<Side>, homesick_in: Option<f64>, fling: Option<visit::Fling>) -> bool {
        let Some(a) = self.away.as_ref() else { return false };
        let (Some(net), Some(f)) = (&self.net, config.friend(next)) else { return false };
        if a.come_home_by.is_some() || a.friend == next || !self.is_connected(next) {
            return false;
        }
        // 그 친구 화면에서 이전 친구(via)가 연결된 끝을 모르면 이전 화면에서 나간 끝의 반대편으로
        let exited = exited.unwrap_or(a.side);
        let homesick_in = homesick_in.filter(|t| t.is_finite());
        // 들려 보낸 편지는 다음 친구에게도 그대로 (주인인 내가 다시 싣는다 — 중간 친구가 고친 것이 아니다)
        let note = a.note.clone();
        let visit_id = self.send_visit(net, own, next, opposite(exited), Some(a.friend.clone()), homesick_in, fling, note.clone(), None);
        eprintln!("own pet walks on from {} to {next} (visit {visit_id})", a.friend);
        // 돌아올 땐 내 화면에서 그 친구가 연결된 끝에서 들어온다
        self.away = Some(AwayVisit { friend: next.to_string(), visit_id, side: f.side, come_home_by: None, since: Instant::now(), note });
        self.menu_dirty = true;
        true
    }

    /// 내 동물이 돌아온다: enter = 들어올 끝 (친구 화면에서 나간 끝의 반대편, 모르면 나갔던 끝)
    fn come_back(&mut self, own: &mut Slot, fling: Option<visit::Fling>, enter: Option<Side>) {
        if let Some(a) = self.away.take() {
            // 집에 돌아오고 한참 뒤 원래 크기로 (게임 루프가 tick_boost를 부른다) — 어떻게 돌아왔든
            if own.boost > 1.0 {
                self.boost_until = Some(Instant::now() + Duration::from_secs(visit::BOOST_HOME_SECS));
            }
            let side = enter.unwrap_or(a.side);
            match fling {
                // 친구 화면에서 던져져 날아 돌아온다
                Some(f) => own.pet.enter_flung(&own.world, side, f),
                None => own.pet.enter(&own.world, side),
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
            NetInput::Event(ev) => return self.on_event(app, ev, config, slots),
            NetInput::Invite(id) => self.start_invite(app, &id, &slots[0].entry.id),
            NetInput::PairAnswer(accept) => self.on_pair_answer(app, accept),
            NetInput::SetSide { id, side } => {
                // 이미 체크된 항목을 누르면 체크가 풀리므로 메뉴를 항상 다시 그린다
                self.last_menu = None;
                return config.set_friend_side(&id, side);
            }
            NetInput::Unfriend(id) => return self.unfriend(&id, config),
            NetInput::LinkDevice(id) => self.open_link(app, config, &id),
            NetInput::UnlinkDevice(id) => {
                // 이쪽 비밀을 지우고 저쪽에도 알린다 (저쪽도 지운다). 설정은 handle 끝의 sync_friends가 맞춘다
                if let Some(net) = &self.net {
                    net.unlink_device(&id);
                }
            }
            NetInput::SubmitLinkCode(code) => self.submit_link_code(app, config, &code),
            NetInput::ConfirmLink(yes) => self.answer_link(app, yes),
            NetInput::LinkClose => {
                // 확인 단계에서 창을 닫으면 취소 (저쪽에도 알리고, 결과는 DeviceLinkCancelled로 온다)
                if self.linking.as_ref().is_some_and(|l| l.asking) {
                    self.answer_link(app, false);
                }
                if let Some(w) = app.get_webview_window(LINK_LABEL) {
                    let _ = w.hide();
                }
            }
            NetInput::ComeHome => {
                if let (Some(net), Some(a)) = (&self.net, self.away.as_mut()) {
                    net.send(&a.friend, Visit::ComeHome { visit_id: a.visit_id });
                    a.come_home_by.get_or_insert(Instant::now() + COME_HOME_FALLBACK);
                }
            }
            NetInput::ShowCode => self.show_code(app),
            NetInput::EnterCode => {
                self.update_code_view(app);
                show_code_window(app);
            }
            NetInput::SubmitCode(code) => self.start_code_invite(app, &code, &slots[0].entry.id),
            NetInput::NewCode => {
                // 새 코드는 FriendCode 이벤트로 온다
                if !self.net.as_ref().is_some_and(|n| n.new_friend_code()) {
                    eprintln!("cannot get a new friend code: not connected to the relay");
                }
            }
            NetInput::EditMyName => {
                let view = NameView { target: "me", current: config.owner_name.clone().unwrap_or_default(), fallback: DEFAULT_NAME.to_string(), max: MAX_NAME_CHARS };
                self.naming = Some(Naming::Me);
                show_name_window(app, view);
            }
            NetInput::EditNickname(id) => {
                let Some(f) = config.friend(&id) else { return false };
                let view = NameView { target: "friend", current: f.nickname.clone().unwrap_or_default(), fallback: f.name.clone(), max: MAX_NAME_CHARS };
                self.naming = Some(Naming::Friend(id));
                show_name_window(app, view);
            }
            NetInput::SubmitName(name) => {
                return match self.naming.take() {
                    Some(Naming::Me) => {
                        let dirty = config.set_owner_name(&name);
                        let shown = config.my_name();
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
                };
            }
            // 게임 루프가 하우스에서 꺼낸 뒤 send_to를 직접 부른다 (여기 오지 않는다)
            NetInput::SendTo(_) | NetInput::SubmitLetter(_) => {}
            NetInput::EditLetter(id) => {
                let Some(f) = config.friend(&id) else { return false };
                let view = LetterView { name: config.person_name(&id).unwrap_or(f.shown_name()).to_string(), max: visit::MAX_NOTE_CHARS, reply: false };
                self.lettering = Some(id);
                self.replying = None;
                show_letter_window(app, view);
            }
            NetInput::EditReply(label) => {
                // 이미 답장을 맡겼거나 떠나는 중인 손님에겐 열지 않는다
                let Some((g, leaving)) = slots.iter().find(|s| s.label == label).and_then(|s| match &s.role {
                    Role::Guest(g) => Some((g.clone(), (is_exiting(s) && s.pet.sending().is_none()) || s.pet.is_away())),
                    Role::Own => None,
                }) else {
                    return false;
                };
                if leaving || self.replies.has(&g.visit_id) {
                    return false;
                }
                let view = LetterView { name: g.owner_name.clone(), max: visit::MAX_NOTE_CHARS, reply: true };
                self.replying = Some((label, g.visit_id));
                self.lettering = None;
                show_letter_window(app, view);
            }
            NetInput::SendOn { label, friend } => self.send_on(config, slots, &label, &friend),
            NetInput::BringPet(id) => {
                if let (Some(net), true) = (&self.net, self.link_up(&id) && self.guests < MAX_GUESTS) {
                    net.ask(&id, ASK_INVITE);
                    self.invite_asked.insert(id, Instant::now());
                }
            }
            // 📱 내 다른 컴퓨터의 펫을 이 화면으로 (그 컴퓨터가 대기 중이어도 둘 사이 링크는 남아 있다)
            NetInput::BringDevicePet => {
                if let Some(id) = config.linked_device().map(|(id, _)| id.to_string()) {
                    return self.handle_input(app, NetInput::BringPet(id), config, slots);
                }
            }
            // 📱 이 컴퓨터로 놀기: 저쪽은 대기가 된다 (결과는 Standby 이벤트로)
            NetInput::PlayHere => {
                if !self.net.as_ref().is_some_and(|n| n.claim_active()) {
                    eprintln!("cannot reach my other computer; this one plays anyway");
                }
            }
            NetInput::LadderOpen => self.ladder.open(app, &ladder_ctx!(self, config)),
            NetInput::LadderShowLast => self.ladder.show_last(app, &ladder_ctx!(self, config)),
            NetInput::ToggleLadderInvites => {
                config.ladder_invites_off = !config.ladder_invites_off;
                return true;
            }
            NetInput::LadderHost { friends, results } => self.ladder.host(app, &ladder_ctx!(self, config), &slots[0], friends, results),
            NetInput::LadderBegin(results) => self.ladder.begin(app, &ladder_ctx!(self, config), results),
            NetInput::LadderAnswer(join) => self.ladder.answer(app, &ladder_ctx!(self, config), &slots[0], join),
            NetInput::LadderClose => self.ladder.close(app, &ladder_ctx!(self, config)),
            NetInput::LadderReact => self.ladder.react(app, &mut slots[0]),
        }
        false
    }

    fn unfriend(&mut self, id: &str, config: &mut Config) -> bool {
        if let Some(net) = &self.net {
            // 연결돼 있었다면 Disconnected가 뒤따라 손님 보내기·귀환을 처리한다
            net.unfriend(id);
        }
        config.remove_friend(id)
    }

    fn on_event(&mut self, app: &AppHandle, ev: NetEvent, config: &mut Config, slots: &mut Vec<Slot>) -> bool {
        match ev {
            NetEvent::PeerSeen { id, name } => {
                // 발견 알림의 이름은 인증되지 않았다 — 친구 아닌 사람 메뉴에만 쓰고 설정에는 적지 않는다
                self.seen.note(id, clamp(&name));
            }
            NetEvent::PeerGone { id } => {
                self.seen.forget(&id);
            }
            NetEvent::Invited { from, name, pet, sas, pairing } => self.on_invited(app, from, name, pet, sas, pairing),
            NetEvent::InviteSent { to, sas, pairing } => self.on_invite_sent(app, &to, &sas, pairing),
            NetEvent::InviteAnswered { from: _, accepted, pairing } => self.on_invite_answered(app, accepted, pairing),
            NetEvent::Connected { id, name } => {
                self.connected.insert(id.clone());
                self.connected_at.insert(id.clone(), Instant::now());
                // 고정된 키를 증명한 링크의 이름 (방금 끝난 짝맞추기도 그 링크에서 Connected가 온다)
                self.note_name(&id, &name);
                // 💩 똥피하기 최고 기록을 알린다 (예전 판은 모르는 부탁을 무시한다)
                if let Some(net) = self.net.as_ref().filter(|_| config.dodge_best > 0) {
                    net.ask(&id, &dodge::score_ask(config.dodge_best));
                }
                // 📱 다른 친구에겐 내 다른 컴퓨터를 알린다 (예전 판에는 net이 보내지 않는다).
                // 내 다른 컴퓨터와는 저장한 비밀을 서로 증명한 뒤(DeviceProven)에 친구 목록을 나눈다
                if let Some(net) = &self.net {
                    net.send_my_devices(&id);
                }
                if let Some(p) = self.pairing.as_mut().filter(|p| p.peer == id && matches!(p.view.phase, "confirm" | "waiting")) {
                    p.view.phase = "done";
                    p.deadline = Instant::now() + RESULT_SECS;
                    self.show_pair(app);
                }
            }
            NetEvent::Renamed { id, name } => {
                // 인증된 친구 링크에서 온 이름만 (net이 확인한다) — 설정은 handle 끝의 sync_friends가 맞춘다
                if config.friend(&id).is_some() {
                    self.note_name(&id, &name);
                }
            }
            NetEvent::Asked { from, ask } => match dodge::parse_score_ask(&ask) {
                // 친구의 💩 똥피하기 최고 기록
                Some(score) => return config.set_friend_dodge_best(&from, score),
                None => self.on_ask(app, config, slots, &from, &ask),
            },
            NetEvent::Steered { from, steer } => {
                // 그 친구(인증된 링크 from) 자신의 손님에게만 — 다른 사람의 손님 visit_id를 대도 무시한다
                let id = steer.visit_id.to_string();
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if g.owner_id == from && g.visit_id == id) {
                        s.pet.remote_keys(steer.on, ControlInput::from_bits(steer.keys));
                    }
                }
            }
            // 던져 넘어온 물건은 게임 루프가 objects.rs로 바로 보낸다 (여기 오지 않는다)
            NetEvent::Tossed { .. } => {}
            // 사다리타기: 판이 끝나 결과가 나오면 설정에 남긴다 (메뉴 "지난 결과")
            NetEvent::Ladder { from, msg } => {
                let last = self.ladder.on_message(app, &ladder_ctx!(self, config), &slots[0], &from, msg);
                if let Some(last) = last {
                    config.last_ladder = Some(last);
                    return true;
                }
            }
            NetEvent::Disconnected { id } => {
                self.connected.remove(&id);
                self.connected_at.remove(&id);
                // 📱 그 친구에게 데려다준 내 다른 컴퓨터의 펫은 그 컴퓨터로 돌아간다 (그 컴퓨터가 끊겼으면 잊는다)
                let gone: Vec<u64> = self.carried.iter().filter(|(_, c)| c.host == id || c.owner == id).map(|(v, _)| *v).collect();
                for v in gone {
                    if let (Some(c), Some(net)) = (self.carried.remove(&v), &self.net) {
                        if c.host == id {
                            net.send(&c.owner, Visit::leave(v));
                        }
                    }
                }
                // 사다리 판의 그 친구 (방장이면 판이 끝난다, 참가자면 빠지거나 판 취소)
                let _ = self.ladder.on_disconnected(app, &ladder_ctx!(self, config), &slots[0], &id);
                // 그 친구에게 보내던 손님은 그만 가고, 그 친구의 손님에게 걸어 둔 보내기는 지운다
                self.drop_guest_sends(slots, |owner, to| owner == id || to == id);
                // 그 친구의 손님은 손 흔들고 사라진다 (📱 그 친구가 데려온 그 친구의 다른 컴퓨터 펫도, 그 컴퓨터와 연결이 없으면)
                for s in slots.iter_mut() {
                    let Role::Guest(g) = &s.role else { continue };
                    let stranded = config.same_person(&g.owner_id, &id) && !self.link_up(config.reach(&g.owner_id, |c| self.link_up(c)));
                    if g.owner_id == id || stranded {
                        s.pet.go_home_now();
                    }
                }
                // 내 동물이 그 친구에게 가 있었으면 바로 돌아온다
                if self.away.as_ref().is_some_and(|a| a.friend == id) {
                    self.come_back(&mut slots[0], None, None);
                }
                // 상대가 친구를 끊었으면(unfriend) net은 조용히 지운다 — 설정에서는 handle 끝의 sync_friends가 지운다
            }
            NetEvent::Received { from, msg } => self.on_visit_message(app, config, slots, &from, msg),
            // 고정한 키를 증명한 친구가 나를 지웠다: net이 지웠고, 설정에서는 handle 끝의 sync_friends가 지운다
            NetEvent::FriendForgotUs { id } => eprintln!("friend {id} no longer has us; removed them too"),
            // 📱 짝맞추기 없이 들어온 친구 (내 다른 컴퓨터가 나눈 친구, 또는 친구가 소개한 그 친구의 다른 컴퓨터)
            NetEvent::FriendAdded { id, name, via, side } => return self.on_friend_added(config, &id, &name, via.as_deref(), side.as_deref()),
            // 소개해 준 친구가 지워져 그 친구의 다른 컴퓨터도 지웠다 — 설정에서는 handle 끝의 sync_friends가 지운다
            // (또는 확인 못 한 친구가 일주일 넘게, 또는 거듭 나를 모른다고 해서 지웠다)
            NetEvent::FriendDropped { id } => eprintln!("removed {id} (introduced by a removed friend, or never confirmed)"),
            // 📱 연결 코드가 양쪽에서 맞았다: 비밀은 handle 끝의 sync_friends가 설정에 적는다. 바로 목록을 나누고 친구들에게 알린다
            NetEvent::DeviceLinked { id } => {
                if let Some(net) = &self.net {
                    net.send_device_friends(&id, false, describe_friend(config));
                    for f in config.friends.iter().filter(|f| f.id != id && self.link_up(&f.id)) {
                        net.send_my_devices(&f.id);
                    }
                }
                if self.linking.as_ref().is_some_and(|l| l.friend == id) {
                    self.linking = None;
                    self.update_link_view(app, |v| {
                        v.phase = "done";
                        v.code.clear();
                        v.note = "연결됐어요! 이제 두 컴퓨터가 친구 목록을 나눠요.".into();
                    });
                }
            }
            NetEvent::DeviceLinkFailed { id, reason, left } => self.on_link_failed(app, &id, &reason, left),
            // 📱 코드가 맞았다: 두 컴퓨터 모두에서 "본인의 PC가 맞습니까?" — 둘 다 확인해야 연결된다 (그 전엔 비밀도 목록도 없다)
            NetEvent::DeviceConfirm { id } => {
                let Some(f) = config.friend(&id) else { return false };
                let view = LinkView { name: f.shown_name().to_string(), code: String::new(), phase: "confirm", note: String::new() };
                self.linking = Some(Linking { friend: id, until: None, asking: true });
                *LINK_VIEW.lock().unwrap() = view.clone();
                show_link_window(app, view);
            }
            // 어느 쪽이 취소했거나, 대답이 없었거나, 연결이 끊겼다: 아무것도 연결·저장되지 않았다
            NetEvent::DeviceLinkCancelled { id } => {
                if let Some(l) = self.linking.as_mut().filter(|l| l.friend == id) {
                    l.asking = false;
                    l.until = None;
                    self.update_link_view(app, |v| {
                        v.phase = "cancelled";
                        v.code.clear();
                        v.note = "연결을 취소했어요".into();
                    });
                }
            }
            // 연결된 내 다른 컴퓨터가 새 연결에서 저장한 비밀을 증명했다: 이제 목록을 나눈다
            NetEvent::DeviceProven { id } => {
                if let Some(net) = &self.net {
                    net.send_device_friends(&id, false, describe_friend(config));
                }
            }
            // 저쪽이 연결을 끊었거나 비밀이 맞지 않았다 (그냥 친구로 남는다) — 설정은 handle 끝의 sync_friends가 지운다
            NetEvent::DeviceUnlinked { id } => eprintln!("{id} is no longer linked as my other computer"),
            // 내 다른 컴퓨터가 친구 목록을 보냈다 (net이 이미 더했다). 달라고 하면 내 목록도 보낸다
            NetEvent::DeviceSynced { from, want } => {
                if let Some(net) = self.net.as_ref().filter(|_| want) {
                    net.send_device_friends(&from, false, describe_friend(config));
                }
            }
            // 📱 어느 컴퓨터가 노는지 바뀌었다
            NetEvent::Standby { me, device } => {
                eprintln!("this computer {}; my other computer {}", if me { "is on standby" } else { "plays" }, if device { "is on standby" } else { "plays or is out of reach" });
                let was = self.standby;
                self.standby = me;
                self.device_standby = device.then(|| config.linked_device().map(|(id, _)| id.to_string())).flatten();
                if me && !was {
                    self.call_home_from_friends(config);
                }
                // 대기 중인 컴퓨터로 가던 내 동물은 그만 간다
                if let Some((id, _)) = &self.sending {
                    if !self.is_connected(id) {
                        slots[0].pet.cancel_send();
                        self.sending = None;
                        self.send_note = None;
                    }
                }
            }
            // 📱 내 다른 컴퓨터의 펫과 성장은 게임 루프가 바로 받는다 (돌보기 상태는 루프가 가진다)
            NetEvent::DeviceState { .. } => {}
            NetEvent::RelayStatus { connected } => {
                self.relay_connected = connected;
                self.update_code_view(app);
            }
            NetEvent::FriendCode { code } => {
                // 릴레이가 친구 코드를 꺼 두면 "----"가 온다: 코드가 없는 것으로 둔다
                self.friend_code = Some(code).filter(|c| c.chars().any(char::is_alphanumeric));
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
            // "펫 데리고 오기": 보낼 수 있고 너무 잦지 않으면 말풍선을 띄우고 "놀러 보내기"처럼 손 흔들고 뿅 그 친구에게 간다
            ASK_INVITE => {
                // 하우스 안·집에 가는 중·먹는 중·성장 축하·들림·이미 가는 중이면 가지 않는다 (하우스에서 끌어내지 않는다)
                let available = slots[0].pet.is_free_to_go() && self.can_send(&slots[0], from);
                let since_friend = self.invite_accepted.get(from).map(|t| t.elapsed().as_secs_f64());
                let since_any = self.invite_accepted_any.map(|t| t.elapsed().as_secs_f64());
                let name = config.person_name(from).unwrap_or(f.shown_name()).to_string();
                let go = visit::answer_invite(available, since_friend, since_any) == visit::InviteAnswer::Go
                    && self.send_to(config, &mut slots[0], from, None);
                if go {
                    // 실제로 가기 시작한 뒤에만 기록하고 말풍선을 띄운다
                    let now = Instant::now();
                    self.invite_accepted.insert(from.to_string(), now);
                    self.invite_accepted_any = Some(now);
                    // 손 흔들 때 "bye~!" 대신 이 말풍선이 보인다
                    slots[0].pet.hush_send();
                    let _ = app.emit_to(OWN_LABEL, "pet-say-text", visit::invite_bubble(&name));
                    eprintln!("{from} invited our pet over; going");
                } else if let Some(net) = &self.net {
                    net.ask(from, ASK_BUSY);
                }
            }
            // 내가 부탁했는데 지금은 못 온대요 — 내 동물 머리 위에 작게
            ASK_BUSY => {
                if self.invite_asked.remove(from).is_some_and(|t| t.elapsed() < ASK_ANSWER_WINDOW) {
                    let name = screenpet_core::config::sanitize_name(config.person_name(from).unwrap_or(f.shown_name())).unwrap_or_else(|| DEFAULT_NAME.to_string());
                    let _ = app.emit_to(OWN_LABEL, "pet-say-text", format!("{name}: busy~"));
                }
            }
            // 던졌는데 그 친구가 지금 받지 않는다 (집중·하우스 중)
            ASK_NOT_NOW => {
                let last = self.not_now_at.insert(from.to_string(), Instant::now());
                if visit::not_now_is_news(last.map(|t| t.elapsed().as_secs_f64())) {
                    let name = screenpet_core::config::sanitize_name(config.person_name(from).unwrap_or(f.shown_name())).unwrap_or_else(|| DEFAULT_NAME.to_string());
                    let _ = app.emit_to(OWN_LABEL, "pet-say-text", format!("{name}: busy~"));
                }
            }
            // 모르는 부탁 (새 판) — 무시
            _ => {}
        }
    }

    /// net이 짝맞추기 없이 고정한 친구를 설정에 넣는다. 소개받은 기기는 소개한 친구와 같은 이름으로,
    /// 내 다른 컴퓨터가 나눈 친구는 그 컴이 알던 이름·화면 끝을 처음 값으로 (이름은 연결되면 친구가 정한 이름으로 바뀐다)
    fn on_friend_added(&mut self, config: &mut Config, id: &str, name: &str, via: Option<&str>, side: Option<&str>) -> bool {
        let Some(key) = self.net.as_ref().and_then(|n| n.friends().into_iter().find(|f| f.id == id)).map(|f| f.public_key) else { return false };
        let name = match via {
            Some(v) => config.friend(v).map(|f| f.name.clone()),
            None => screenpet_core::config::sanitize_name(name),
        }
        .unwrap_or_else(|| DEFAULT_NAME.to_string());
        let mut dirty = config.upsert_friend(id, &name, key);
        if let Some(side) = side.and_then(visit::side_choice).filter(|_| via.is_none()) {
            dirty |= config.set_friend_side(id, side);
        }
        self.share_due = true;
        dirty
    }

    /// 친구 이름은 내가 정하는 이름과 같은 규칙으로 (보이지 않는 글자 빼고 20자까지, 비면 그대로 둔다)
    fn note_name(&mut self, id: &str, name: &str) {
        if let Some(name) = screenpet_core::config::sanitize_name(name) {
            self.names.insert(id.to_string(), name);
        }
    }

    /// 설정의 친구 목록을 net이 고정한 친구(id·공개키)에 맞춘다: 없어진 친구는 지우고, 새 친구는 더하고,
    /// 이름은 인증된 연결에서 들은 것, 없으면 저장된 이름 (새 친구는 Connected 전까지 "친구"). 바뀌었으면 true
    fn sync_friends(&mut self, config: &mut Config) -> bool {
        let Some(net) = &self.net else { return false };
        let pinned = net.friends();
        let mut dirty = false;
        let gone: Vec<String> = config.friends.iter().filter(|f| !pinned.iter().any(|p| p.id == f.id)).map(|f| f.id.clone()).collect();
        // 친구가 늘거나 줄었으면 내 다른 컴퓨터들에게 다시 알린다 (tick에서, 너무 잦지 않게)
        if !gone.is_empty() || pinned.iter().any(|p| config.friend(&p.id).is_none()) {
            self.share_due = true;
        }
        for id in gone {
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
            // 📱 내 다른 컴퓨터와 나눈 비밀·소개한 친구·확인 전 (net이 아는 대로)
            if let Some(info) = net.friend_info(&p.id) {
                dirty |= config.set_friend_links(&p.id, info.via.as_deref(), info.pending);
                dirty |= config.set_friend_pending_since(&p.id, info.pending_since);
            }
            dirty |= config.set_friend_device_secret(&p.id, net.device_secret(&p.id).map(|s| s.0));
        }
        dirty
    }

    fn on_visit_message(&mut self, app: &AppHandle, config: &Config, slots: &mut Vec<Slot>, from: &str, msg: Visit) {
        match msg {
            Visit::Visit { visit_id, species, stage, scale, owner, enter_side, vside, via, homesick_in, fling, note, gift, style, boost, of, .. } => {
                // 📱 친구가 데려온 그 친구의 다른 컴퓨터 펫이면 그 컴퓨터의 손님 (한 사람 — 이름표도 같다). 대답은 링크(from)로
                let link = from;
                let from = of.as_deref().filter(|o| config.same_person(o, link)).unwrap_or(link);
                // 집중·하우스 중엔 새 손님을 받지 않는다: 던져졌으면 날아온 그대로, 걸어왔으면 걸어서 되돌아간다
                // (걸어온 쪽은 바로 돌아온 것을 보고 잠깐 보내지 않는다 — QUICK_DECLINE)
                if slots[0].pet.refuses_arrivals() {
                    eprintln!("sent back visit {visit_id} from {from}");
                    if let Some(net) = &self.net {
                        net.send(link, Visit::Leave { visit_id, next: None, side: None, vside: None, homesick_in: None, fling, note: None, boost: None });
                        net.ask(link, ASK_NOT_NOW);
                    }
                    return;
                }
                let now = Instant::now();
                // 방금 왔다 간 친구: 꽉 찬 게 아니라 잠깐 이른 것 — 들어오려던 끝을 실어 주면(꽉 찼다는 뜻이 아니다) 던져졌으면 날아온 그대로 돌아간다
                if self.last_visit.get(from).is_some_and(|t| now.duration_since(*t) < VISIT_COOLDOWN) {
                    if !self.last_decline_log.get(from).is_some_and(|t| now.duration_since(*t) < DECLINE_LOG_EVERY) {
                        eprintln!("sent back visit {visit_id} from {from}: too soon after the last visit");
                        self.last_decline_log.insert(from.to_string(), now);
                    }
                    if let Some(net) = &self.net {
                        net.send(link, Visit::Leave { visit_id, next: None, side: Some(enter_side), vside, homesick_in: None, fling, note: None, boost: None });
                    }
                    return;
                }
                let note = note.as_deref().and_then(visit::sanitize_note);
                let arrival = Arrival { enter_side: from_net_side(enter_side, vside), via, homesick_in, fling: fling.map(from_net_fling), note, style: PetStyle::from_wire(style.as_deref()), boost };
                let result = self.welcome_guest(app, config, slots, from, visit_id, &species, &stage, scale, &owner, arrival);
                match result {
                    Ok(()) => {
                        self.last_visit.insert(from.to_string(), now);
                        // 인증된 친구 링크의 Visit에 실린, 목록에 있는 그 동물 종의 선물만 (친구마다 횟수 제한)
                        if let Some(friend) = config.friend(from) {
                            if let Some(gift) = gift::accept_visit_gift(gift.as_deref(), &species).filter(|_| self.gift_limit.allow(from, now)) {
                                self.guest_gifts.push(GuestGift { visit_id: visit_id.to_string(), owner_id: from.to_string(), owner_name: config.person_name(from).unwrap_or(friend.shown_name()).to_string(), gift });
                            }
                        }
                    }
                    Err(e) => {
                        // 받을 수 없는 손님은 바로 돌려보낸다 (주인 쪽에서 원래 동물이 돌아온다)
                        if !self.last_decline_log.get(from).is_some_and(|t| now.duration_since(*t) < DECLINE_LOG_EVERY) {
                            eprintln!("declined visit {visit_id} from {from}: {e}");
                            self.last_decline_log.insert(from.to_string(), now);
                        }
                        if let Some(net) = &self.net {
                            net.send(link, Visit::leave(visit_id));
                        }
                    }
                }
            }
            Visit::Leave { visit_id, next, side, vside, homesick_in, fling, note, boost } => {
                // 📱 데려다준 내 다른 컴퓨터의 펫
                if self.carried_left(from, visit_id, next.clone(), side, vside, homesick_in, fling, note.clone(), boost) {
                    return;
                }
                // 답장은 내 동물이 가 있던 그 친구(인증된 링크)가 그 방문을 집으로 돌려보낼 때만
                let reply = visit::accept_reply(self.away.as_ref().map(|a| (a.friend.as_str(), a.visit_id)), from, visit_id, next.is_none(), note.as_deref());
                let Some(a) = self.away.as_ref().filter(|a| a.friend == from && a.visit_id == visit_id) else { return };
                let fling = fling.map(from_net_fling);
                // 친구가 밥을 줘서 커졌다 (다듬어서, 더 작아지지는 않는다). 다음 친구에게 가도 그 크기다
                let grown = visit::sanitize_boost(boost, slots[0].boost_cap());
                if grown > slots[0].boost {
                    slots[0].set_boost(app, grown);
                }
                // 보내자마자 그냥 돌아왔다 — 그 친구 화면이 꽉 찼다고 보고 잠깐 보내지 않는다
                if visit::return_means_full(next.is_some(), fling.is_some(), a.come_home_by.is_some(), side.is_some(), a.since.elapsed().as_secs_f64()) {
                    self.full_until.insert(from.to_string(), Instant::now() + FULL_FOR);
                }
                let forwarded = next.is_some_and(|n| self.forward_visit(config, &slots[0], &n, side.map(|s| from_net_side(s, vside)), homesick_in, fling));
                if !forwarded {
                    self.come_back(&mut slots[0], fling, side.map(|s| opposite(from_net_side(s, vside))));
                    if let Some(note) = reply {
                        // 돌아온 내 동물 머리 옆에 💌 — 누르면 "💌 <친구>: <답장>" (웹뷰는 textContent로만)
                        let from_name = config.person_name(from).map(str::to_string).unwrap_or_else(|| DEFAULT_NAME.to_string());
                        eprintln!("own pet came home from {from} with a reply");
                        let _ = app.emit_to(OWN_LABEL, "pet-mail", PetMail { from: from_name, note });
                    }
                }
            }
            Visit::ComeHome { visit_id } => {
                // 📱 내 다른 컴퓨터가 친구에게 데려다준 자기 펫을 부른다: 그 친구에게 전한다
                if let Some(c) = self.carried.get(&visit_id).filter(|c| c.owner == from) {
                    if let Some(net) = &self.net {
                        net.send(&c.host, Visit::ComeHome { visit_id });
                    }
                    return;
                }
                let id = visit_id.to_string();
                // 주인이 불렀다 — 다음 친구에게 보내던 길은 그만
                self.guest_sends.remove(&id);
                // (친구의 다른 컴퓨터가 데려온 손님은 그 친구가 부른다 — 한 사람)
                for s in slots.iter_mut() {
                    if matches!(&s.role, Role::Guest(g) if (g.owner_id == from || config.same_person(&g.owner_id, from)) && g.visit_id == id) {
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
        let stage = Stage::from_name(stage).ok_or_else(|| format!("unknown stage {stage:?}"))?;
        let guests = slots.iter().filter(|s| !s.is_own());
        if guests.clone().count() >= MAX_GUESTS {
            return Err("too many guests".into());
        }
        if guests.into_iter().any(|s| matches!(&s.role, Role::Guest(g) if g.owner_id == from)) {
            return Err("already visiting".into());
        }
        let scale = visit::guest_scale(scale, stage.scale() * entry.manifest.size_mult(stage));
        // 📱 친구의 다른 컴퓨터에서 온 동물도 그 친구 이름표 (한 사람)
        let owner_name = config.person_name(from).map(str::to_string).unwrap_or_else(|| clamp(owner));
        // 편지는 그 동물의 주인(인증된 친구 링크 from)이 실은 것만 — Visit은 주인만 보낸다
        let guest = GuestInfo { visit_id: visit_id.to_string(), owner_id: from.to_string(), owner_name, note: arrival.note.clone() };
        // 화면을 떠날 땐 그 화면 주인의 설정을 따르고, 받는 쪽은 나간 끝의 반대편으로 받는다 (내 설정은 쓰지 않는다).
        // 그래서 들어오는 끝은 보낸 쪽이 정한 그대로이고, 집으로 갈 때 나갈 끝은 이 화면에서 주인이 연결된 끝이다. 그리움 시간은 이어서 센다
        let hop = arrival.via.as_deref().filter(|v| *v != from);
        let side = arrival.enter_side;
        let home = config.friend(from).map_or(side, |f| f.side);
        let seed = fastrand::u64(..);
        let mut slot = slots::spawn_guest(app, guest, entry, stage, scale, arrival.style, slots[0].dpi, side, &slots[0].world, seed, slots[0].size())?;
        if home != side || arrival.homesick_in.is_some() {
            let mut rng = fastrand::Rng::with_seed(seed);
            slot.pet.set_guest(home, visit::carried_homesick(arrival.homesick_in, &mut rng));
        }
        let boost = visit::sanitize_boost(arrival.boost, slot.boost_cap());
        if boost > 1.0 {
            slot.set_boost(app, boost);
        }
        // 던져져 넘어왔으면 옆 모니터에서 날아오듯 그 높이·속도로 들어온다
        if let Some(f) = arrival.fling {
            slot.pet.enter_flung(&slot.world, side, f);
        }
        eprintln!("guest {visit_id} from {from} arrived{}{}", if arrival.note.is_some() { " with a note" } else { "" }, hop.map(|v| format!(" (via {v})")).unwrap_or_default());
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
        let name = self.seen.name(peer).unwrap_or(DEFAULT_NAME).to_string();
        // 친구가 이미 MAX_FRIENDS명이면 net이 신청하지 않는다 — 바로 알린다
        let full = net.friends().len() >= MAX_FRIENDS;
        let view = if full {
            PairView { phase: "failed", reason: "full", name, ..PairView::default() }
        } else {
            net.invite(peer, own_species);
            PairView { phase: "connecting", name, ..PairView::default() }
        };
        let wait = if full { RESULT_SECS } else { CONNECT_TIMEOUT };
        self.pairing = Some(Pairing { peer: peer.to_string(), by_code: false, token: None, view, deadline: Instant::now() + wait, shown: false });
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
            shown: false,
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

    /// "친구 코드 보기": 펫이 화면에 보이면 머리 위 띠에 (누르면 복사), 아니면(하우스 안·놀러 감·연결 끊김) 코드 창에
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

    /// 📱 "내 다른 컴퓨터로 연결…": 이 컴의 새 연결 코드를 창에 보인다 (저쪽 코드도 이 창에 넣는다).
    /// 이미 다른 컴퓨터와 연결돼 있으면 코드 없이 먼저 끊으라고 알린다 (컴퓨터는 둘까지)
    fn open_link(&mut self, app: &AppHandle, config: &Config, id: &str) {
        let (Some(net), Some(f)) = (&self.net, config.friend(id)) else { return };
        let name = f.shown_name().to_string();
        let view = match net.new_link_code(id) {
            Ok(code) => LinkView { name, code, phase: "ready", note: String::new() },
            Err(LinkError::Taken(other)) => LinkView { name, code: String::new(), phase: "blocked", note: taken_note(config, &other) },
            Err(e) => {
                eprintln!("cannot make a link code for {id}: {e:?}");
                return;
            }
        };
        if self.linking.as_ref().is_some_and(|l| l.asking && l.friend != id) {
            self.answer_link(app, false); // 다른 친구와 확인하던 것은 취소
        }
        self.linking = Some(Linking { friend: id.to_string(), until: None, asking: false });
        *LINK_VIEW.lock().unwrap() = view.clone();
        show_link_window(app, view);
    }

    /// "본인의 PC가 맞습니까?"의 확인/취소 (창 닫기도 취소). 둘 다 확인하면 net이 DeviceLinked를 낸다
    fn answer_link(&mut self, app: &AppHandle, yes: bool) {
        let (Some(net), Some(l)) = (&self.net, self.linking.as_mut().filter(|l| l.asking)) else { return };
        let answered = net.confirm_device(&l.friend, yes);
        if yes && answered {
            self.update_link_view(app, |v| {
                v.phase = "confirmed";
                v.note = "저쪽 컴퓨터에서도 확인을 눌러 주세요".into();
            });
        } else {
            // 취소했다 (또는 이미 늦었다): 결과(DeviceLinkCancelled)를 기다리지 않고 바로 보인다
            l.asking = false;
            self.update_link_view(app, |v| {
                v.phase = "cancelled";
                v.code.clear();
                v.note = "연결을 취소했어요".into();
            });
        }
    }

    /// 연결 창 "연결": 저쪽 컴퓨터가 보여 주는 코드를 넣었다 (코드는 net이 선에 싣지 않고 증명만 한다)
    fn submit_link_code(&mut self, app: &AppHandle, config: &Config, code: &str) {
        let (Some(net), Some(l)) = (&self.net, self.linking.as_mut()) else { return };
        let (phase, note) = match net.enter_link_code(&l.friend, code) {
            Ok(()) => {
                l.until = Some(Instant::now() + LINK_WAIT);
                ("waiting", "저쪽 컴퓨터와 확인하는 중…".to_string())
            }
            Err(LinkError::Invalid) => ("failed", "코드는 8글자예요 (예: ABCD-EFGH). 저쪽 창의 코드를 확인해 주세요.".into()),
            Err(LinkError::Offline) => ("failed", "저쪽 컴퓨터가 지금 연결돼 있지 않아요. 둘 다 켜 두고 다시 해 주세요.".into()),
            Err(LinkError::Old) => ("failed", "저쪽 앱이 예전 버전이에요. 둘 다 새 버전으로 올려 주세요.".into()),
            Err(LinkError::Taken(other)) => ("blocked", taken_note(config, &other)),
            Err(LinkError::NotFriend) => ("failed", "이제 친구가 아니에요.".into()),
        };
        self.update_link_view(app, |v| {
            v.phase = phase;
            v.note = note;
        });
    }

    /// 코드로 연결하지 못했다. 코드를 넣은 쪽(기다리던 쪽)과 코드를 보여 준 쪽의 안내가 다르다
    fn on_link_failed(&mut self, app: &AppHandle, id: &str, reason: &str, left: u32) {
        let Some(l) = self.linking.as_mut().filter(|l| l.friend == id) else { return };
        let typed_here = l.until.take().is_some();
        let note = match (reason, typed_here) {
            ("wrong", true) if left > 0 => format!("코드가 틀렸어요. 저쪽 창의 코드를 다시 확인해 주세요 ({left}번 남음)."),
            ("wrong", true) => "저쪽의 확인이 맞지 않아요. 저쪽 창을 다시 열어 새 코드로 해 주세요.".to_string(),
            ("wrong", false) => format!("저쪽에서 넣은 코드가 틀렸어요 ({left}번 남음)."),
            ("locked", true) => "너무 많이 틀려서 저쪽 코드가 없어졌어요. 저쪽에서 창을 다시 열어 새 코드를 받아 주세요.".to_string(),
            ("locked", false) => "너무 많이 틀려서 이 코드는 없앴어요. 메뉴에서 다시 열면 새 코드가 나와요.".to_string(),
            ("taken", _) => "저쪽 컴퓨터는 이미 다른 컴퓨터와 연결돼 있어요. 저쪽에서 먼저 연결을 끊어 주세요.".to_string(),
            _ => "저쪽에 이 컴퓨터용 코드가 없어요 (10분이 지났거나 이미 썼어요). 저쪽에서 창을 열어 새 코드를 받아 주세요.".to_string(),
        };
        let locked_here = reason == "locked" && !typed_here;
        self.update_link_view(app, |v| {
            v.phase = "failed";
            v.note = note;
            if locked_here {
                v.code.clear();
            }
        });
    }

    /// 연결 창의 상태를 고쳐 보낸다 (창이 열려 있으면)
    fn update_link_view(&self, app: &AppHandle, change: impl FnOnce(&mut LinkView)) {
        let view = {
            let mut v = LINK_VIEW.lock().unwrap();
            change(&mut v);
            v.clone()
        };
        if app.get_webview_window(LINK_LABEL).is_some() {
            let _ = app.emit_to(LINK_LABEL, "link-state", view);
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
            shown: false,
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
    fn show_pair(&mut self, app: &AppHandle) {
        let quiet = self.quiet;
        let Some(p) = self.pairing.as_mut() else { return };
        *app.state::<AppState>().pair.lock().unwrap() = p.view.clone();
        // 받은 신청은 인증되지 않은 누구든 보낼 수 있다: 하던 일을 끊지 않게 포커스를 뺏지 않고, 집중·전체 화면이 끝날 때까지 미룬다
        if p.view.incoming && !p.shown && !visit::incoming_pair_may_show(p.view.phase == "confirm", quiet) {
            return;
        }
        p.shown = true;
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
            crate::overlay::place_on_primary(app, &window, PAIR_FORM.size);
            let _ = window.show();
            // 내가 시작한 신청만 앞으로 가져온다
            if !p.view.incoming {
                let _ = window.set_focus();
            }
        }
    }

    fn hide_pair(app: &AppHandle) {
        if let Some(w) = app.get_webview_window(PAIR_LABEL) {
            let _ = w.hide();
        }
    }

    /// 매 틱 (게임 루프): 조작 모드 키를 친구 화면에 가 있는 내 동물에게 — 바뀔 때, 그리고 누르고 있는 동안 STEER_KEEPALIVE마다.
    /// 끄면 on = false를 두 번 (STEER_KEEPALIVE 뒤 한 번 더 — 첫 번째를 흘려도 손님이 조작 모드에 남지 않게). 내 동물이 집에 있으면 아무것도 보내지 않는다
    pub fn steer(&mut self, now: Instant, on: bool, keys: ControlInput) {
        let (Some(net), Some(a)) = (&self.net, &self.away) else {
            self.steer_pacer.reset();
            return;
        };
        let bits = if on { keys.bits() } else { 0 };
        if self.steer_pacer.should_send(a.visit_id, on, bits, now, STEER_KEEPALIVE) {
            net.steer(&a.friend, Steer { visit_id: a.visit_id, on, keys: bits });
        }
    }

    /// 방금 들어온 손님이 들고 온 선물들 (게임 루프가 Gifts로 넘긴다)
    pub fn take_guest_gifts(&mut self) -> Vec<GuestGift> {
        std::mem::take(&mut self.guest_gifts)
    }

    /// 매 틱: 짝맞추기 시간, "집에 와" 대비, 트레이 `친구랑 놀기` 메뉴
    pub fn tick(&mut self, app: &AppHandle, now: Instant, config: &Config, own: &mut Slot) {
        self.quiet = own.pet.is_focusing() || screenpet_core::weather::any_fullscreen(&own.world.windows, &own.world.work_areas);
        self.tick_pairing(app, now);
        // 📱 코드를 넣었는데 저쪽이 대답하지 않는다
        if let Some(l) = self.linking.as_mut().filter(|l| l.until.is_some_and(|t| now >= t)) {
            l.until = None;
            self.update_link_view(app, |v| {
                v.phase = "failed";
                v.note = "저쪽 컴퓨터가 대답하지 않아요. 잠시 뒤 다시 해 주세요.".into();
            });
        }
        // 미뤄 둔 받은 신청: 조용해졌으면 지금 띄운다
        if !self.quiet && self.pairing.as_ref().is_some_and(|p| p.view.incoming && !p.shown && p.view.phase == "confirm") {
            self.show_pair(app);
        }
        self.ladder.tick(app, &ladder_ctx!(self, config), own);
        let refuses = (own.pet.refuses_arrivals(), self.ladder.busy());
        if refuses != self.own_refuses {
            self.own_refuses = refuses;
            self.menu_dirty = true;
        }
        // 놀러 보내는 중: 그 친구가 끊겼으면 그만두고, 동물 쪽에서 끝났으면(하우스로 감 등) 잊는다
        if let Some((id, _)) = &self.sending {
            if !self.is_connected(id) || config.friend(id).is_none() {
                own.pet.cancel_send();
                self.sending = None;
                self.send_note = None;
            } else if own.pet.sending().is_none() && !is_exiting(own) && !own.pet.is_away() {
                self.sending = None;
                self.send_note = None;
            }
        }
        let can_send = self.own_free(own);
        if can_send != self.can_send {
            self.can_send = can_send;
            self.menu_dirty = true;
        }
        // 연결은 살아 있는데 친구 쪽 방문이 너무 오래 끝나지 않는다: 평소 "집에 와"처럼 부른다
        if let (Some(net), Some(a)) = (&self.net, self.away.as_mut()) {
            if a.come_home_by.is_none() && a.since.elapsed().as_secs_f64() >= visit::AWAY_SILENCE_LIMIT_SECS {
                eprintln!("no word from {} for too long; calling the pet home", a.friend);
                net.send(&a.friend, Visit::ComeHome { visit_id: a.visit_id });
                a.come_home_by = Some(now + COME_HOME_FALLBACK);
            }
        }
        if self.away.as_ref().and_then(|a| a.come_home_by).is_some_and(|t| now >= t) {
            eprintln!("no answer to come-home; coming back anyway");
            self.come_back(own, None, None);
        }
        // 📱 친구 목록이 바뀌었으면 연결된 내 다른 컴퓨터들에게 알린다 (net이 서로 표시한 새 판에만 보낸다)
        if self.share_due && self.last_share.is_none_or(|t| now.duration_since(t) >= SHARE_EVERY) {
            self.share_due = false;
            self.last_share = Some(now);
            if let Some(net) = &self.net {
                for f in config.friends.iter().filter(|f| f.device_secret.is_some() && self.link_up(&f.id)) {
                    net.send_device_friends(&f.id, false, describe_friend(config));
                }
            }
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
        let strangers = self.seen.menu_strangers(|id| config.friend(id).is_some());
        // 📱 사람마다 한 줄: 친구와 그 친구가 소개한 그 친구의 다른 컴퓨터는 하나 (지금 노는 컴퓨터로 보낸다)
        let at = |id: &str| self.connected_at.get(id).filter(|_| self.link_up(id)).map(|t| t.duration_since(self.started).as_secs_f64());
        let friends = config.people(at)
            .into_iter()
            .filter_map(|p| {
                let f = config.friend(&p.id)?;
                let playable = self.is_connected(&p.target);
                Some(FriendItem {
                    id: p.id.clone(),
                    name: f.shown_name().to_string(),
                    side: f.side,
                    connected: p.connected,
                    can_send: self.can_send && playable,
                    can_throw: playable,
                    device: f.device_secret.is_some(),
                    target: p.target,
                })
            })
            .collect();
        // 손님 우클릭 "<친구>에게 보내기" 목록
        *ONLINE_FRIENDS.lock().unwrap() =
            config.friends.iter().filter(|f| self.is_open(&f.id)).map(|f| (f.id.clone(), config.person_name(&f.id).unwrap_or(f.shown_name()).to_string())).collect();
        let relay = self.relay_enabled.then_some(self.relay_connected);
        let room = self.guests < MAX_GUESTS;
        // 사다리타기: 판이 있으면 언제든 그 창을, 없으면 사다리를 아는 친구가 연결돼 있고 내가 바쁘지 않을 때
        let ladder_friend = self.net.as_ref().is_some_and(|n| {
            config.friends.iter().any(|f| self.is_connected(&f.id) && n.peer_version(&f.id).is_some_and(|v| v >= screenpet_net::LADDER_MIN_VERSION))
        });
        let ladder = LadderMenu {
            open: self.ladder.busy() || (ladder_friend && !self.own_refuses.0),
            invites: !config.ladder_invites_off,
            last: config.last_ladder.is_some() && !self.ladder.busy(),
        };
        // 📱 내 다른 컴퓨터와 연결돼 있고 이 컴퓨터가 놀면 그 컴퓨터 펫을 데려올 수 있다
        let bring_device = config.linked_device().filter(|(id, _)| self.link_up(id) && !self.standby).map(|_| room);
        FriendsMenu { available: self.net.is_some(), my_name: self.name.clone(), strangers, friends, away: self.away.is_some(), relay, room, ladder, standby: self.standby, bring_device }
    }
}

/// 내 다른 컴퓨터에 친구를 알릴 때 붙이는 처음 값: 친구가 정한 이름과 내 화면의 끝
fn describe_friend(config: &Config) -> impl Fn(&str) -> (String, Option<String>) + '_ {
    move |id| match config.friend(id) {
        Some(f) => (f.name.clone(), visit::SIDE_CHOICES.iter().find(|c| c.0 == f.side).map(|c| c.1.to_string())),
        None => (DEFAULT_NAME.to_string(), None),
    }
}

/// 손님 이름표를 설정에 맞춘다 (친구가 이름을 바꿨거나 내가 별명을 붙였을 때)
fn refresh_guest_tags(app: &AppHandle, config: &Config, slots: &mut [Slot]) {
    for s in slots.iter_mut() {
        let Role::Guest(g) = &s.role else { continue };
        if let Some(name) = config.person_name(&g.owner_id) {
            s.set_owner_name(app, name.to_string());
        }
    }
}

/// 짝맞추기 창: 작고 테두리 있는 창, 항상 위. 닫기(X)는 숨기기 + 취소로 바꾼다 (창은 다시 쓴다)
fn build_pair_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = crate::overlay::form_builder(app, &PAIR_FORM).focused(false).build()?;
    let handle = app.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            handle.state::<AppState>().send(Input::Net(NetInput::PairAnswer(false)));
        }
    });
    Ok(window)
}

/// 친구 코드 창: 내 코드(복사) + 상대 코드 입력
fn show_code_window(app: &AppHandle) {
    crate::overlay::show_form(app, &CODE_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(CODE_LABEL, "code-state", CODE_VIEW.lock().unwrap().clone());
    });
}

/// 코드 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `code-state` 이벤트)
#[tauri::command]
pub fn code_state(window: WebviewWindow) -> Result<CodeView, String> {
    if window.label() != CODE_LABEL {
        return Err("wrong window".into());
    }
    Ok(CODE_VIEW.lock().unwrap().clone())
}

/// 코드 창의 "친구 신청": 코드 꼴이 맞으면 게임 루프로 보내고 창을 숨긴다 (짝맞추기 창이 이어 받는다).
/// 꼴이 틀리면 Err("invalid") — 창에 안내한다
#[tauri::command]
pub fn code_submit(window: WebviewWindow, state: tauri::State<AppState>, code: String) -> Result<(), String> {
    if window.label() != CODE_LABEL {
        return Err("wrong window".into());
    }
    let Some(code) = normalize_code(&code) else { return Err("invalid".into()) };
    state.send(Input::Net(NetInput::SubmitCode(code)));
    let _ = window.hide();
    Ok(())
}

/// 코드 창의 "새 코드": 중계 서버에 새 친구 코드를 받는다 (새 코드는 `code-state`로 온다)
#[tauri::command]
pub fn code_renew(window: WebviewWindow, state: tauri::State<AppState>) {
    if window.label() == CODE_LABEL {
        state.send(Input::Net(NetInput::NewCode));
    }
}

/// 코드 창의 "닫기"
#[tauri::command]
pub fn code_close(window: WebviewWindow) {
    if window.label() == CODE_LABEL {
        let _ = window.hide();
    }
}

/// 📱 이미 다른 컴퓨터와 연결돼 있을 때 (컴퓨터는 둘까지)
fn taken_note(config: &Config, other: &str) -> String {
    let name = config.friend(other).map_or("다른", |f| f.shown_name());
    format!("이미 \"{name}\" 컴퓨터와 연결돼 있어요. 컴퓨터는 둘까지라, 먼저 그 친구 메뉴에서 📱 연결 끊기를 해 주세요.")
}

/// 연결 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `link-state` 이벤트)
#[tauri::command]
pub fn link_state(window: WebviewWindow) -> Result<LinkView, String> {
    if window.label() != LINK_LABEL {
        return Err("wrong window".into());
    }
    Ok(LINK_VIEW.lock().unwrap().clone())
}

/// 연결 창의 "연결": 저쪽 코드를 게임 루프로 보낸다 (창은 그대로 — 결과가 `link-state`로 온다)
#[tauri::command]
pub fn link_submit(window: WebviewWindow, state: tauri::State<AppState>, code: String) {
    if window.label() != LINK_LABEL {
        return;
    }
    let code: String = code.chars().take(32).collect();
    state.send(Input::Net(NetInput::SubmitLinkCode(code)));
}

/// 연결 창의 "닫기"·Esc: 숨기고, 확인 단계였으면 게임 루프가 취소한다
#[tauri::command]
pub fn link_close(window: WebviewWindow, state: tauri::State<AppState>) {
    if window.label() == LINK_LABEL {
        let _ = window.hide();
        state.send(Input::Net(NetInput::LinkClose));
    }
}

/// 연결 창의 "본인의 PC가 맞습니까?" 확인(true) / 취소(false)
#[tauri::command]
pub fn link_confirm(window: WebviewWindow, state: tauri::State<AppState>, yes: bool) {
    if window.label() == LINK_LABEL {
        state.send(Input::Net(NetInput::ConfirmLink(yes)));
    }
}

/// 📱 연결 창을 보인다 (처음이면 만든다 — 게임 루프 스레드에서만)
fn show_link_window(app: &AppHandle, view: LinkView) {
    if app.get_webview_window(LINK_LABEL).is_none() {
        if let Err(e) = build_link_window(app) {
            eprintln!("failed to create link window: {e}");
            return;
        }
    }
    crate::overlay::show_form(app, &LINK_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(LINK_LABEL, "link-state", view);
    });
}

/// 연결 창: 닫기(X)는 숨기기 + (확인 단계면) 취소로 바꾼다 (창은 다시 쓴다)
fn build_link_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = crate::overlay::form_builder(app, &LINK_FORM).build()?;
    let handle = app.clone();
    let shown = window.clone();
    window.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = shown.hide();
            handle.state::<AppState>().send(Input::Net(NetInput::LinkClose));
        }
    });
    Ok(window)
}

/// 이름 바꾸기 창: 글자 칸 하나 + 저장/닫기
fn show_name_window(app: &AppHandle, view: NameView) {
    *NAME_VIEW.lock().unwrap() = view.clone();
    crate::overlay::show_form(app, &NAME_FORM, crate::overlay::place_on_primary, |window| {
        let _ = window.set_title(if view.target == "me" { "ScreenPet 내 이름" } else { "ScreenPet 친구 이름" });
        let _ = app.emit_to(NAME_LABEL, "name-state", view);
    });
}

/// 이름 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `name-state` 이벤트)
#[tauri::command]
pub fn name_state(window: WebviewWindow) -> Result<NameView, String> {
    if window.label() != NAME_LABEL {
        return Err("wrong window".into());
    }
    Ok(NAME_VIEW.lock().unwrap().clone())
}

/// 이름 창의 "저장": 게임 루프로 보내고 창을 숨긴다 (다듬기·길이 제한은 설정이 한다 — 빈 글자 = 기본 이름)
#[tauri::command]
pub fn name_submit(window: WebviewWindow, state: tauri::State<AppState>, name: String) {
    if window.label() != NAME_LABEL {
        return;
    }
    // 아주 긴 글자는 보내기 전에 자른다 (설정이 다시 20자로 자른다)
    let name: String = name.chars().take(256).collect();
    state.send(Input::Net(NetInput::SubmitName(name)));
    let _ = window.hide();
}

/// 이름 창의 "닫기"
#[tauri::command]
pub fn name_close(window: WebviewWindow) {
    if window.label() == NAME_LABEL {
        let _ = window.hide();
    }
}

/// 편지 쓰기 창: 한 줄 글자 칸 + 보내기/닫기
fn show_letter_window(app: &AppHandle, view: LetterView) {
    *LETTER_VIEW.lock().unwrap() = view.clone();
    crate::overlay::show_form(app, &LETTER_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(LETTER_LABEL, "letter-state", view);
    });
}

/// 편지 창의 지금 상태 (웹뷰 첫 로드용 — 그 뒤로는 `letter-state` 이벤트)
#[tauri::command]
pub fn letter_state(window: WebviewWindow) -> Result<LetterView, String> {
    if window.label() != LETTER_LABEL {
        return Err("wrong window".into());
    }
    Ok(LETTER_VIEW.lock().unwrap().clone())
}

/// 편지 창의 "보내기": 게임 루프로 보낸다. 창은 보낸 뒤 게임 루프가 닫는다 — 못 보내면 쓴 글이 남는다 (다듬기·80자 제한도 게임 루프가 한다)
#[tauri::command]
pub fn letter_submit(window: WebviewWindow, state: tauri::State<AppState>, text: String) {
    if window.label() != LETTER_LABEL {
        return;
    }
    // 아주 긴 글자는 보내기 전에 자른다 (sanitize_note가 다시 80자로 자른다)
    let text: String = text.chars().take(256).collect();
    state.send(Input::Net(NetInput::SubmitLetter(text)));
}

/// 편지 창의 "닫기"
#[tauri::command]
pub fn letter_close(window: WebviewWindow) {
    if window.label() == LETTER_LABEL {
        let _ = window.hide();
    }
}
