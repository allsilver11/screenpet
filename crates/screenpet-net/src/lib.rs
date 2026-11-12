//! ScreenPet 네트워크임: LAN 찾기(UDP 브로드캐스트), SAS로 확인하는 짝짓기,
//! Noise 프로토콜로 끝에서 끝까지 암호화·인증되는 오래 가는 친구 링크.
//!
//! 모두 평범한 std 스레드로 돎. 앱은 [`Net`] 메서드(네트워크를 기다리지 않음)로 말하고
//! mpsc 채널로 [`NetEvent`]를 받음 (src-tauri의 `network.rs`가 씀).
//!
//! 계층:
//! - `protocol`: JSON 메시지(`hello`, `invite`, `visit`, ...)와 찾기 알림
//! - `transport`: 프레임 통로 이음매(`FrameLink`/`FrameReader`)와 전송 중립 다이얼(`Candidate`). `tcp`가 LAN 전송임
//! - `noise`: XX 핸드셰이크, 정적 키, 이모지 SAS
//! - `secure`: 핸드셰이크 뒤 암호화·큐 방식 메시지 보내기
//! - `peers`: 링크별 세션 구동기 (신원 확인, 짝짓기, 친구, ping, 시간 초과), 전송과 무관
//! - `discovery`: LAN UDP 알림 (힌트일 뿐: 후보 주소)
//! - `relay`: LAN 밖 친구용 릴레이 전송(`screenpet-relay`, WebSocket), 친구 코드, LAN 우선 링크 고르기

mod devices;
mod discovery;
pub mod ladder;
pub mod board;
pub mod letter;
mod noise;
mod peers;
mod protocol;
mod relay;
mod secure;
mod tcp;
mod transport;

pub use noise::{hex32, to_hex, SAS_EMOJI};
pub use protocol::{Carry, Fling, LadderMsg, Side, Steer, Toss, VSide, Visit, WireCare, WirePlayer, WireResult, CARRY_MIN_VERSION, DEVICES_MIN_VERSION, LADDER_MIN_VERSION, MAX_DEVICES, MAX_FRIENDS, MAX_NOTE, MAX_RALLY, PROTOCOL_VERSION};
pub use relay::{normalize_code, parse_relay_key, DEFAULT_RELAY_KEY, DEFAULT_RELAY_URL};

use protocol::{Control, Message};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use transport::Candidate;

/// 스펙 기본 UDP 찾기 포트
pub const DEFAULT_DISCOVERY_PORT: u16 = 47321;

/// 워커 스레드가 타이머와 멈춤 깃발을 보러 깨는 주기
pub(crate) const TICK: Duration = Duration::from_millis(50);
/// 친구로 (아직) 인증되지 않은 들어온 링크(인증 전·낯선 링크) 동시 수 상한. 차면 가장 오래된 것을 밀어냄.
/// 인증된 친구 링크는 여기에 안 셈
pub(crate) const MAX_UNVERIFIED_INBOUND: usize = 6;
/// 그중 한 출처 주소에서 온 것 상한 (가장 오래된 것을 밀어냄)
pub(crate) const MAX_UNVERIFIED_PER_IP: usize = 2;
/// 전에 친구로 인증했던 주소에서 온 인증 전 링크는 낯선 풀 밖에 둠(따로 잡아 둔 자리). 주소당 이만큼까지
pub(crate) const MAX_TRUSTED_PER_IP: usize = 4;
/// 친구로 인증했던 주소로 기억하는 수 상한
pub(crate) const MAX_TRUSTED_IPS: usize = 64;
/// 출처 주소당 초당 새 들어오는 연결 수. 넘으면 스레드나 키 교환 전에 버림
pub(crate) const MAX_CONNS_PER_ADDR: u32 = 20;
/// 친구로 인증했던 주소의 같은 한도
pub(crate) const MAX_CONNS_PER_TRUSTED_ADDR: u32 = 100;
/// hello 전인 들어온 세션 스레드 수 상한, 링크 등급마다 (`handshake_class` 참고).
/// 밀려난 링크는 틱 하나 안에 끝나므로 폭주가 그보다 빨라야만 여기에 닿음
pub(crate) const MAX_HANDSHAKES: usize = 32;
/// 친구가 인증했던 주소에서 온 미검증 링크 전체 상한
pub(crate) const MAX_TRUSTED_INBOUND: usize = 32;
/// 핸드셰이크 + hello는 이 안에 끝나야 함 (`conn_timeout`이 더 짧으면 그것)
pub(crate) const PRE_AUTH_MAX: Duration = Duration::from_secs(3);
/// 신원 확인이 계속 실패하는 친구의 가장 긴 재다이얼 대기
pub(crate) const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// 동시에 들어오는 짝짓기 요청 수 상한
pub(crate) const MAX_INVITES_IN: usize = 4;
/// 출처 주소당 동시에 들어오는 짝짓기 요청 수 상한
pub(crate) const MAX_INVITES_IN_PER_ADDR: usize = 2;
/// 한 id의 미검증 후보로 하는 다이얼 수, 알린 출처당·분당
pub(crate) const MAX_UNVERIFIED_DIALS_PER_MIN: usize = 20;
/// `INVITED_WINDOW`마다 `Invited` 이벤트 수 상한
pub(crate) const MAX_INVITED_PER_WINDOW: usize = 10;
/// `Invited` 이벤트 수를 세는 창
pub(crate) const INVITED_WINDOW: Duration = Duration::from_secs(60);
/// 창마다 상대 주소당 들어오는 짝짓기 시도 수 상한
pub(crate) const MAX_PAIR_ATTEMPTS_PER_ADDR: usize = 3;

/// 시간 상수 모음임 (스펙 기본값은 `Default`, 시험은 짧은 값을 씀)
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    /// 찾기 알림 주기
    pub announce_interval: Duration,
    /// 이만큼 조용한 찾은 상대는 `PeerGone`으로 알림
    pub peer_timeout: Duration,
    /// 링크마다 `ping` 주기
    pub ping_interval: Duration,
    /// 이만큼 아무것도 안 받은 링크는 끊음. 핸드셰이크 + hello 마감이기도 함
    pub conn_timeout: Duration,
    /// 두 사용자가 이 안에 확인하지 않은 짝짓기는 거절됨
    pub invite_timeout: Duration,
    /// 릴레이 재연결 첫 대기 (60초까지 두 배씩). 릴레이 링크가 실패한 친구의 첫 재다이얼 지연이기도 함
    pub relay_retry: Duration,
    /// 📱 연결 코드가 맞은 뒤 두 사용자가 이 안에 확인을 눌러야 함 ([`Net::confirm_device`]). 아니면 양쪽 다 취소됨
    pub device_confirm_timeout: Duration,
    /// 📱 대기로 가는 컴퓨터는 친구 링크를 이만큼 유지한 뒤 닫음 (앱이 펫을 집으로 부를 수 있게)
    pub standby_grace: Duration,
    /// 📱 노는 컴퓨터는 바뀐 펫·돌봄을 최대 이 주기로 나눔 (그사이 바뀐 것은 모음) …
    pub device_state_every: Duration,
    /// … 그리고 같은 상태를 이 주기로 다시 보냄 (사라졌을 때 대비)
    pub device_state_refresh: Duration,
}

impl Default for Timing {
    /// 스펙 기본 시간 값을 돌려줌
    fn default() -> Timing {
        Timing {
            announce_interval: Duration::from_secs(2),
            peer_timeout: Duration::from_secs(6),
            ping_interval: Duration::from_secs(2),
            conn_timeout: Duration::from_secs(6),
            invite_timeout: Duration::from_secs(20),
            relay_retry: Duration::from_secs(5),
            device_confirm_timeout: Duration::from_secs(60),
            standby_grace: Duration::from_secs(5),
            device_state_every: Duration::from_secs(2),
            device_state_refresh: Duration::from_secs(60),
        }
    }
}

/// 친구임: 설치 id(라벨)를 친구의 Noise 정적 공개키(신원)에 고정한 것. 앱이 (설정 등에) 저장함
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    /// 친구 설치 id
    pub id: String,
    /// 고정된 정적 공개키
    pub public_key: [u8; 32],
}

/// [`Net::start`]에 넘기는 설정임
#[derive(Debug, Clone)]
pub struct NetConfig {
    /// 이 설치의 id ([`new_install_id`] 참고)
    pub id: String,
    /// 남에게 보이는 표시 이름 (64자로 자름)
    pub name: String,
    /// 이 설치의 장기 X25519 개인키 ([`Net::generate_static_key`] 참고). 설정과 함께 저장함
    pub static_key: [u8; 32],
    /// 알림을 들을 UDP 포트 (0 = 아무거나, [`Net::discovery_port`] 참고)
    pub discovery_port: u16,
    /// 알림을 보낼 곳 (보통 `255.255.255.255:47321`)
    pub broadcast_to: SocketAddr,
    /// 전에 짝지은 친구들 ([`Net::friends`]가 돌려준 그대로)
    pub friends: Vec<Friend>,
    /// 시간 상수
    pub timing: Timing,
    /// LAN 밖 친구용 릴레이 (`wss://…`나 `ws://…`), 보통 [`DEFAULT_RELAY_URL`]. None이면 LAN만 씀
    pub relay_url: Option<String>,
    /// 고정한 릴레이 정적 공개키 (보통 [`DEFAULT_RELAY_URL`]과 함께 [`DEFAULT_RELAY_KEY`]).
    /// 다른 키를 내미는 릴레이는 거절함. None이면 연결마다 믿음
    pub relay_key: Option<[u8; 32]>,
    /// 내 연결된 다른 컴퓨터와 저장된 비밀 ([`Net::new_link_code`] 참고). 친구와 함께 저장함. 친구가 아니면 무시함
    pub device: Option<(String, DeviceSecret)>,
    /// 친구가 자기 다른 컴퓨터로 소개한 친구들: (기기 id, 소개한 친구 id). 그 친구와 함께 빠짐
    pub introduced: Vec<(String, String)>,
    /// 다른 컴퓨터나 소개로 더했지만 아직 링크에서 우리를 안다고 보이지 않은 친구들 ([`FriendInfo::pending`] 참고)
    pub pending: Vec<String>,
    /// `pending` 각각을 더한 때 (유닉스 초). 재시작해도 시계가 다시 안 돌게 함. 여기 없는 것은 이번 시작부터 셈
    pub pending_added: Vec<(String, u64)>,
}

/// 연결된 두 컴퓨터가 나누는 비밀임 (저장하되 보이거나 로그에 남기지 말 것: `Debug`는 바이트를 안 찍음)
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeviceSecret(pub [u8; 32]);

impl std::fmt::Debug for DeviceSecret {
    /// 바이트 없이 `DeviceSecret(..)`만 찍음
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceSecret(..)")
    }
}

/// [`Net::new_link_code`] / [`Net::enter_link_code`]를 시작하지 못한 까닭임
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// 친구가 아님
    NotFriend,
    /// 이미 다른 컴퓨터와 연결됨 (먼저 끊을 것): 컴퓨터는 둘까지임
    Taken(String),
    /// 연결 코드가 아님
    Invalid,
    /// 친구가 지금 연결돼 있지 않음
    Offline,
    /// 친구 빌드가 연결하기를 모름
    Old,
}

/// [`Net::friend_info`]가 키 말고 친구에 대해 아는 것임
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FriendInfo {
    /// 내 연결된 다른 컴퓨터임
    pub device: bool,
    /// 이 친구가 자기 컴퓨터로 소개했음
    pub via: Option<String>,
    /// 다른 컴퓨터나 소개로 더했고 아직 우리를 아는 모습을 못 봄. 모른다고 해도 (아직은) 안 지움
    pub pending: bool,
    /// 미확인으로 더한 때 (유닉스 초, 확인됐으면 None)
    pub pending_since: Option<u64>,
}

impl NetConfig {
    /// 스펙 기본값으로 만듦: 포트 47321, 255.255.255.255:47321로 브로드캐스트, 스펙 시간, 릴레이 없음 (쓰려면 `relay_url`을 설정)
    pub fn new(id: impl Into<String>, name: impl Into<String>, static_key: [u8; 32], friends: Vec<Friend>) -> NetConfig {
        NetConfig {
            id: id.into(),
            name: name.into(),
            static_key,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            broadcast_to: SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT)),
            friends,
            timing: Timing::default(),
            relay_url: None,
            relay_key: None,
            device: None,
            introduced: Vec::new(),
            pending: Vec::new(),
            pending_added: Vec::new(),
        }
    }
}

/// 네트워크가 앱에 알리는 이벤트임 (mpsc 채널로 감)
#[derive(Debug, Clone, PartialEq)]
pub enum NetEvent {
    /// LAN에서 상대가 보임
    PeerSeen { id: String, name: String },
    /// 보이던 상대가 조용해짐
    PeerGone { id: String },
    /// 낯선 이가 짝짓기를 청함. `sas`를 보여 초대자 화면과 비교하게 하고,
    /// 이 `pairing`(정확히 이 요청을 가리킴)을 넘겨 [`Net::answer_invite`]로 답함
    Invited { from: String, name: String, pet: String, sas: String, pairing: u64 },
    /// 우리 초대가 `to`에 닿음. `sas`를 보이고 사용자가 상대 화면과 같다고 확인하면 이 `pairing`으로 [`Net::answer_invite`]를 부름
    InviteSent { to: String, sas: String, pairing: u64 },
    /// 상대가 확인함(`true`) 또는 짝짓기가 친구 없이 끝남(`false`: 거절, 시간 초과, 링크 끊김).
    /// `pairing`은 마무리되는 `Invited`/`InviteSent`의 토큰이고, 알린 적 없는 짝짓기는 결과도 안 냄
    InviteAnswered { from: String, accepted: bool, pairing: u64 },
    /// 친구 링크가 이어짐
    Connected { id: String, name: String },
    /// 친구 `id`가 표시 이름을 바꿈 (인증된 친구 링크의 반복 `hello`, 64자로 자름).
    /// 옛 버전은 반복 `hello`를 무시하므로 하위 호환임
    Renamed { id: String, name: String },
    /// 친구 `from`이 [`Net::ask`]로 앱 요청을 보냄 (64자로 자름, 인증된 친구 링크에서만)
    Asked { from: String, ask: String },
    /// 친구 `from`이 우리에게 놀러 온 자기 펫을 조종함 ([`Net::steer`], 인증된 친구 링크에서만, 속도 제한).
    /// 앱은 `steer.visit_id`가 `from`의 손님인지 확인해야 함
    Steered { from: String, steer: Steer },
    /// 친구 `from`이 우리 화면에 물건을 던짐 ([`Net::toss`], 인증된 친구 링크에서만, 한 번에 10개 뒤 친구당 6초에 하나,
    /// 재연결해도 이어짐). 앱은 `kind`, `fling`, `note`를 검사해야 함
    Tossed { from: String, toss: Toss },
    /// 친구 링크가 끊김 (유예 뒤)
    Disconnected { id: String },
    /// [`Net::send_letter`]에 대한 릴레이 답임: `Err`는 그 까닭(`daily`, `ip`, `new`, `global`, `full`, `size`, `blocked`)
    /// 또는 `offline`, `busy`, `timeout`
    LetterSent { result: Result<(), String> },
    /// 개발자 편지함 전체 (릴레이가 편지함 독자로 둔 설치에만), 최신 먼저, 그리고 이미 📮 공개 건의함에 올라간 편지 id들
    /// (v3 전 릴레이면 빈 목록)
    Mailbox { letters: Vec<letter::MailItem>, board: Vec<u64> },
    /// 📮 [`Net::board_list`]의 답: 공개 건의함 한 페이지, 또는 안 된 까닭 (`offline`, `old` = 릴레이가 v3보다 옛것, `timeout`)
    Board { result: Result<board::BoardPage, String> },
    /// 우리가 독자로 접속해 있는 동안 개발자 편지함에 새 편지가 옴
    LetterArrived { item: letter::MailItem },
    /// 친구에게서 놀러 가기 메시지가 옴
    Received { from: String, msg: Visit },
    /// 연결된 친구에게서 사다리 게임 메시지가 옴 (`ladder`가 검사함)
    Ladder { from: String, msg: LadderMsg },
    /// 친구 `id`가 고정된 키를 증명했지만 더는 우리를 친구로 모름 (예: 짝짓기가 한쪽만 끝났거나 우리를 지움).
    /// 그래서 여기서도 지웠고 [`Net::friends`]에 더는 없음. 고정된 키만 이를 일으킬 수 있고,
    /// `id`로 *다른* 키를 내미는 주소는 조용히 나쁜 후보로만 다룸
    FriendForgotUs { id: String },
    /// ☀️ 우리가 원하는 지역의 릴레이 현재 날씨 ([`Net::want_weather`], 그전엔 용인): WMO 날씨 코드와 릴레이가 받아 온 때 (유닉스 초).
    /// 연결 뒤와 새 값마다(약 30분마다) 옴. 옛 릴레이에서는 안 옴
    Weather { code: u16, at: i64 },
    /// 릴레이 연결이 올라옴(`true`) 또는 내려감(`false`)
    RelayStatus { connected: bool },
    /// 릴레이에서 받은 지금 친구 코드 (연결 때와 교체 때)
    FriendCode { code: String },
    /// [`Net::redeem_code`]에 준 코드가 `id`의 것임. 짝짓기가 시작됨 (`invite`처럼 `InviteSent` / `InviteAnswered`가 옴)
    CodeRedeemed { code: String, id: String },
    /// 코드를 못 씀. `reason`: `invalid` (코드 아님), `unknown` (그런 코드 없음), `offline` (릴레이나 주인 미접속), `self`,
    /// `already_friend`, `busy`, `rate`, `timeout`, `full` (`MAX_FRIENDS`만큼 있음), `disabled` (릴레이가 친구 코드를 끔)
    CodeFailed { code: String, reason: String },
    /// 짝짓기 없이 친구를 고정함: 내 연결된 다른 컴퓨터의 `device_friends`에서(`via: None`, `name`과 `side`는 첫값으로
    /// 자르기만 하고 검사 안 함), 또는 친구 `via`가 자기 컴퓨터로 소개함(`name`은 빈 값). [`Net::friends`]에 나옴
    FriendAdded { id: String, name: String, via: Option<String>, side: Option<String> },
    /// 짝짓기 없이 더한 친구를 뺌: 소개한 친구가 빠졌거나, 끝내 확인되지 않음 (일주일, 또는 모른다고 세 번 말함)
    FriendDropped { id: String },
    /// 친구 `id`와 연결 코드가 맞음: 사용자에게 "본인의 PC가 맞습니까?"를 묻고 [`Net::confirm_device`]로 답함.
    /// 두 컴퓨터가 다 확인하기 전엔 연결·저장·공유가 없음 (그 뒤 [`NetEvent::DeviceLinked`]).
    /// 아니오·링크 끊김·`Timing::device_confirm_timeout`이면 양쪽에 [`NetEvent::DeviceLinkCancelled`]가 옴
    DeviceConfirm { id: String },
    /// 둘 다 확인하기 전에 친구 `id`와의 연결이 취소됨 (한쪽이 아니오, 시간 안에 답 없음, 링크 끊김). 연결·저장된 것 없음
    DeviceLinkCancelled { id: String },
    /// 친구 `id`가 이제 내 연결된 다른 컴퓨터임: 코드가 맞고 두 사용자가 확인함 ([`Net::device_secret`]을 저장할 것).
    /// 이제 이 링크로 친구 목록이 오감: 내 것을 보낼 것 ([`Net::send_device_friends`])
    DeviceLinked { id: String },
    /// 친구 `id`와 코드로 연결하기가 안 됨: `reason` = `wrong` (코드 쪽에 `left`번 남음), `locked` (너무 많이 틀려 코드가 사라짐),
    /// `none` (거기에 우리용 살아 있는 코드 없음), `taken` (한쪽이 다른 컴퓨터와 연결됨)
    DeviceLinkFailed { id: String, reason: String, left: u32 },
    /// 내 연결된 컴퓨터 `id`가 새 링크에서 저장된 비밀을 증명함: 이제 친구 목록이 오감, 내 것을 보낼 것 ([`Net::send_device_friends`])
    DeviceProven { id: String },
    /// 친구 `id`가 더는 내 연결된 컴퓨터가 아님 (끊었거나 우리 비밀이 없음). 친구로는 남음. 저장된 비밀을 잊을 것
    DeviceUnlinked { id: String },
    /// 내 다른 컴퓨터 `from`이 친구들을 보냄 (이미 합침). `want` = 내 것을 달라는 뜻 ([`Net::send_device_friends`])
    DeviceSynced { from: String, want: bool },
    /// 📱 연결된 두 컴퓨터 중 누가 노는지 바뀜. `me` = 이 컴퓨터가 대기: 친구 링크는 `Timing::standby_grace` 뒤 닫히고
    /// (지금 친구들에게서 펫을 집으로 부를 것) 다시 놀 때까지 안 열림. `device` = 연결된 컴퓨터가 대기 (내가 놂, 내 펫을 보내지 말 것).
    /// 서로 닿지 않으면 둘 다 `false`: 각자 놂
    Standby { me: bool, device: bool },
    /// 📱 노는 내 연결된 컴퓨터 `from`이 고른 펫과 모든 펫의 돌봄을 나눔 (또는 내가 차지할 때 넘겨줌).
    /// 모양은 검사됨(펫 id, 최대 `MAX_CARE`개, 펫당 하나). 앱이 종을 검사하고 값을 자른 뒤 받아들임.
    /// 어느 쪽으로든 마지막에 나눈 상태와 다를 때만 옴
    DeviceState { from: String, pet: Option<String>, care: Vec<WireCare> },
}

/// 찾기로 들은 상대 id임. 알림은 인증 없는 힌트임
pub(crate) struct Seen {
    /// 후보 → (마지막 알린 때, 알린 출처 IP)
    pub candidates: HashMap<Candidate, (Instant, IpAddr)>,
    /// 마지막으로 신원 확인을 통과한 후보와 그 때
    pub verified: Option<(Candidate, Instant)>,
    /// 후보마다 마지막 다이얼 때 (라운드 로빈 순서)
    pub tried: HashMap<Candidate, Instant>,
    /// 알린 출처 IP별 최근 미검증 후보 다이얼 (분당 상한)
    pub unverified_dials: HashMap<IpAddr, VecDeque<Instant>>,
    /// 마지막으로 들은 때
    pub last: Instant,
}

/// 짝짓기가 commit -> random -> reveal -> confirm 교환 중 어디인지임
pub(crate) enum PairStage {
    /// 초대자: `invite{commit}`을 보냈고 초대받은 쪽 난수를 기다림
    AwaitRandom { nonce_i: [u8; 32] },
    /// 초대받은 쪽: 난수를 보냈고 초대자의 reveal을 기다림
    AwaitReveal { commit: [u8; 32], r_r: [u8; 32], pet: String },
    /// 양쪽에 SAS를 보임. 두 사용자의 확인을 기다림
    Confirm,
    /// 거절됐거나 시간 초과. 끝임: 이 링크에서는 아무것도 고정 못 함
    Failed,
}

/// 진행 중인 짝짓기임. 초대를 실은 링크에 묶임
pub(crate) struct Pairing {
    /// 진행 단계
    pub stage: PairStage,
    /// 우리 사용자가 확인함
    pub local_ok: bool,
    /// 상대 사용자가 확인함
    pub remote_ok: bool,
    /// 단계가 시작된 때 (마감 계산용)
    pub since: Instant,
    /// 마지막 `InviteAnswered{accepted:false}`를 이미 냈음
    pub failure_reported: bool,
    /// `Invited`/`InviteSent`를 냈음: 그때만 결과를 알림
    pub surfaced: bool,
}

/// 링크 종류임
pub(crate) enum Kind {
    /// 인증됨: 상대 정적 키가 고정된 키와 같음
    Friend,
    /// 친구 아님. 짝짓기만 할 수 있음 (링크에 초대가 오기 전엔 None)
    Stranger(Option<Pairing>),
}

/// 토큰 버킷임: 한 번에 `burst`개, 그 뒤 `every`마다 하나
pub(crate) struct Bucket {
    tokens: f64,
    at: Instant,
    burst: f64,
    every: Duration,
}

impl Bucket {
    /// 가득 찬 버킷을 만듦
    fn new(now: Instant, burst: f64, every: Duration) -> Bucket {
        Bucket { tokens: burst, at: now, burst, every }
    }

    /// 토큰이 있으면 하나 씀
    pub fn take(&mut self, now: Instant) -> bool {
        let refill = now.saturating_duration_since(self.at).as_secs_f64() / self.every.as_secs_f64();
        self.tokens = (self.tokens + refill).min(self.burst);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// 이름 바꾸기와 요청: 한 번에 2개, 그 뒤 2초에 하나
const ASK_BURST: f64 = 2.0;
/// 요청 토큰이 하나 차는 주기
const ASK_EVERY: Duration = Duration::from_secs(2);
/// 조종(조종 모드): 한 번에 30개, 그 뒤 초당 30개
const STEER_BURST: f64 = 30.0;
/// 조종 토큰이 하나 차는 주기
const STEER_EVERY: Duration = Duration::from_micros(33_334);
/// 던진 물건: 한 번에 50개(다섯 번 던지기), 그 뒤 분당 50개
const TOSS_BURST: f64 = 50.0;
/// 던지기 토큰이 하나 차는 주기
const TOSS_EVERY: Duration = Duration::from_millis(1200);
/// 놀러 가기 메시지: 펫이 오가며 보내는 것보다 훨씬 넉넉함
const VISIT_BURST: f64 = 20.0;
/// 놀러 가기 토큰이 하나 차는 주기
const VISIT_EVERY: Duration = Duration::from_millis(500);
/// 사다리 메시지: 게임 하나에 친구당 몇 개뿐임
const LADDER_BURST: f64 = 20.0;
/// 사다리 토큰이 하나 차는 주기
const LADDER_EVERY: Duration = Duration::from_millis(500);
/// 내 컴퓨터끼리의 친구 목록과 소개: 연결이나 변경마다 몇 개
const LINKS_BURST: f64 = 6.0;
/// 친구 목록 토큰이 하나 차는 주기
const LINKS_EVERY: Duration = Duration::from_secs(5);
/// 코드로 연결하기(시도당 메시지 둘, 코드 하나에 `devices::CODE_TRIES`번)와 연결마다의 비밀 증명
const CODES_BURST: f64 = 16.0;
/// 코드 토큰이 하나 차는 주기
const CODES_EVERY: Duration = Duration::from_secs(2);
/// 📱 나눈 펫과 돌봄: 보내는 쪽이 `Timing::device_state_every`마다 하나로 모으므로(차지의 넘겨주기 더함) 한 번에 몇 개
const STATES_BURST: f64 = 4.0;
/// 상태 토큰이 하나 차는 주기
const STATES_EVERY: Duration = Duration::from_secs(1);

/// 친구 하나가 우리에게 처리시킬 수 있는 양임 (넘으면 버리고 링크는 유지: 폭주하는 친구가 앱을 이벤트로 도배 못 함).
/// 링크가 아니라 친구마다 둠: 재연결해도 남은 토큰으로 이어감. 종류마다 버킷이 따로라 한 종류 폭주가 다른 것을 굶기지 않음
pub(crate) struct Budget {
    /// 요청과 이름 바꾸기
    pub asks: Bucket,
    /// 조종
    pub steers: Bucket,
    /// 던진 물건
    pub tosses: Bucket,
    /// 놀러 가기 메시지
    pub visits: Bucket,
    /// 사다리 메시지
    pub ladder: Bucket,
    /// 친구 목록과 소개
    pub links: Bucket,
    /// `device_*` 메시지 (코드로 연결하기, 저장된 비밀 증명)
    pub codes: Bucket,
    /// `device_state` (버킷이 따로라 나눈 펫이 차지를 굶기지 않음)
    pub states: Bucket,
}

impl Budget {
    /// `device_*` 메시지 하나만큼 씀 (`state` = `device_state`)
    pub fn device(&mut self, state: bool, now: Instant) -> bool {
        if state {
            self.states.take(now)
        } else {
            self.codes.take(now)
        }
    }

    /// 버킷을 모두 가득 채워 만듦
    fn new(now: Instant) -> Budget {
        Budget {
            asks: Bucket::new(now, ASK_BURST, ASK_EVERY),
            steers: Bucket::new(now, STEER_BURST, STEER_EVERY),
            tosses: Bucket::new(now, TOSS_BURST, TOSS_EVERY),
            visits: Bucket::new(now, VISIT_BURST, VISIT_EVERY),
            ladder: Bucket::new(now, LADDER_BURST, LADDER_EVERY),
            links: Bucket::new(now, LINKS_BURST, LINKS_EVERY),
            codes: Bucket::new(now, CODES_BURST, CODES_EVERY),
            states: Bucket::new(now, STATES_BURST, STATES_EVERY),
        }
    }
}

/// 살아 있는 링크 하나임 (hello 뒤). 낯선 링크는 여럿이 같은 id를 주장할 수 있고,
/// 친구 링크는 id당 하나뿐임 (`State::friend_links` 참고)
pub(crate) struct Conn {
    /// 상대가 말한 id
    pub peer_id: String,
    /// 암호화된 메시지 통로
    pub link: Arc<secure::SecureLink>,
    /// 이 링크에서 마지막으로 들은 표시 이름 (hello, 그 뒤 이름 바꾸기)
    pub name: String,
    /// 상대 hello의 프로토콜 버전 (그 빌드의 `PROTOCOL_VERSION`)
    pub v: u32,
    /// 친구인지 낯선 이인지
    pub kind: Kind,
    /// 이 상대에 대해 `Connected`를 냈음 (그래서 `Disconnected`를 내야 함)
    pub announced: bool,
    /// 우리가 연 링크임
    pub outgoing: bool,
    /// 링크를 연 쪽의 id
    pub dialer: String,
    /// 상대 정적 공개키
    pub remote_key: [u8; 32],
    /// 상대 전송 주소 (신뢰·속도 제한용)
    pub peer_addr: String,
    /// 이 링크의 Noise 핸드셰이크 해시 (짝짓기 SAS 입력)
    pub hs_hash: [u8; 32],
    /// 링크가 생긴 때
    pub since: Instant,
    /// 켜져 있는 동안 `MAX_UNVERIFIED_INBOUND`에 셈
    pub unverified: Arc<AtomicBool>,
    /// 이 링크에서의 코드 연결·저장된 비밀 증명 상태
    pub dev: devices::DevLink,
}

/// 재다이얼 대기가 걸리는 대상임
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Backoff {
    /// 친구(인증됨)가 우리를 친구로 안 둔다고 말함
    Id(String),
    /// 이 주소가 친구 id에 틀린 키로 답함
    Cand(Candidate),
}

/// 네트워크 전체 상태임 (`Shared::state` 잠금 하나로 지킴)
#[derive(Default)]
pub(crate) struct State {
    /// 고정된 친구들
    pub friends: Vec<Friend>,
    /// 찾기로 들은 id들
    pub seen: HashMap<String, Seen>,
    /// 후보별 실패 기록 (`seen`과 따로, 만료됨)
    pub failed: HashMap<Candidate, Instant>,
    /// 재다이얼 대기: 키 → (이전엔 다이얼 안 함, 지금 지연)
    pub backoff: HashMap<Backoff, (Instant, Duration)>,
    /// 세션 토큰별 모든 링크
    pub conns: HashMap<u64, Conn>,
    /// 친구 id → 인증된 링크의 토큰
    pub friend_links: HashMap<String, u64>,
    /// 이번 실행에서 짝지은 친구마다 고정한 때 (`just_paired` 참고)
    pub paired_at: HashMap<String, Instant>,
    /// 친구 id → 링크 없이 들린 지 언제부터인지 (큰 id는 유예 뒤에만 다이얼해서 양쪽이 동시에 거는 일이 드묾)
    pub friend_wait: HashMap<String, Instant>,
    /// 지금 다이얼 중인 id들
    pub dialing: HashSet<String>,
    /// 나가는 링크를 기다리는 초대: 상대 id → 펫
    pub invites_out: HashMap<String, String>,
    /// 최근 `Invited` 이벤트 시각들 (속도 제한)
    pub invited_log: VecDeque<Instant>,
    /// 출처 IP별 찾기 데이터그램: (창 시작, 수)
    pub src_rate: HashMap<IpAddr, (Instant, u32)>,
    /// 친구 id를 주장하는 알림, (id, 출처 IP)별
    pub friend_rate: HashMap<(String, IpAddr), (Instant, u32)>,
    /// 상대 주소별 최근 들어온 짝짓기 시도
    pub pair_attempts: HashMap<String, VecDeque<Instant>>,
    /// 친구로 인증한 주소 → 그 때 (상한 있음). 키는 (친구 id, 주소)
    pub trusted_ips: HashMap<(String, String), Instant>,
    /// 출처 주소별 새 들어오는 연결 (`Shared::allow_inbound` 참고)
    pub conn_rate: HashMap<String, (Instant, u32)>,
    /// 믿는 (id, 출처 IP)의 알림, 따로 둔 유한 예산
    pub trusted_rate: HashMap<(String, IpAddr), (Instant, u32)>,
    /// 친구 id → 릴레이 링크를 LAN으로 옮기려 한 마지막 시도
    pub lan_upgrade: HashMap<String, Instant>,
    /// 친구 id → 내야 할 `Disconnected`를 낼 때. 끝난 친구 링크는 유예(`conn_timeout`) 뒤에만 알림.
    /// 그 안에 새 링크가 등록되면(예: 릴레이 ↔ LAN 전환, 빠른 재시작) 조용히 이어받음: Disconnected/Connected 쌍이 없음
    pub pending_gone: HashMap<String, Instant>,
    /// 친구 id → 아직 처리시킬 수 있는 양. 인증된 친구만 들고 친구와 함께 빠짐
    pub budgets: HashMap<String, Budget>,
    /// 내 연결된 다른 컴퓨터와 비밀 (하나까지: 컴퓨터 둘)
    pub device: Option<(String, [u8; 32])>,
    /// 내가 보여 주는 연결 코드 (친구 하나용)
    pub link_code: Option<devices::LinkCode>,
    /// 소개된 기기 id → 소개한 친구
    pub via: HashMap<String, String>,
    /// 짝짓기 없이 더했고 아직 우리를 아는 모습을 못 본 친구 → 더한 때 (유닉스 초, `devices::expire_pending` 참고)
    pub pending: HashMap<String, u64>,
    /// 미확인 친구 → 우리를 모른다는 인증된 hello 수
    pub pending_denied: HashMap<String, u32>,
    /// 📱 내가 아는 한 누가 노는지: (term, 설치 id), 가장 높은 쪽이 이김 (`devices::tick_active` 참고). (0, 나)로 시작함
    pub active: (u64, String),
    /// 사용자가 이 컴퓨터를 마지막으로 쓴 때 (`Net::set_idle`)
    pub last_input: Option<Instant>,
    /// 연결된 컴퓨터로의 링크가 살아 있고 증명된 마지막 때
    pub device_seen: Option<Instant>,
    /// 마지막으로 알린 대로 (`NetEvent::Standby`): 이 컴퓨터가 대기 중임
    pub standby: bool,
    /// 마지막으로 알린 대로: 연결된 컴퓨터가 대기 중임
    pub device_standby: bool,
    /// 이 컴퓨터가 대기로 간 때 (유예 뒤 친구 링크를 닫음)
    pub standby_since: Option<Instant>,
    /// 📱 두 컴퓨터가 나눈 가장 최근의 고른 펫과 돌봄: 내 것(`Net::share_state`)이나 연결된 컴퓨터가 나눈 것
    pub dstate: Option<devices::SharedState>,
    /// 마지막으로 보낸 뒤 바뀜 (`Timing::device_state_every` 안에 보냄)
    pub dstate_dirty: bool,
    /// 연결된 컴퓨터에서 마지막으로 받은 뒤 여기서 바꿈 (그러면 넘겨주기가 덮어쓰지 않음)
    pub dstate_edited: bool,
    /// 마지막으로 보낸 것: (링크 토큰, 놀던 term, 때)
    pub dstate_sent: Option<(u64, u64, Instant)>,
    /// 방금 놀기를 차지함: 그때까지 연결된 컴퓨터의 넘겨주기가 올 수 있음
    pub handover_until: Option<Instant>,
}

impl State {
    /// 친구 `id`의 예산을 돌려줌 (인증된 친구에만 부를 것)
    pub fn budget(&mut self, id: &str, now: Instant) -> &mut Budget {
        self.budgets.entry(id.to_string()).or_insert_with(|| Budget::new(now))
    }

    /// `id`가 고정된 친구인지 봄
    pub fn is_friend(&self, id: &str) -> bool {
        self.friends.iter().any(|f| f.id == id)
    }
    /// 이 id의 키를 친구 소개만 보증하는지 봄 (확인 여부 무관: 그 키를 가진 이는 스스로 확인할 수 있음).
    /// 그렇다면 진짜 SAS 확인 짝짓기가 그 키를 바꿀 수 있음
    pub fn introduced_only(&self, id: &str) -> bool {
        self.via.contains_key(id)
    }
    /// 친구 `id`의 고정된 키를 돌려줌
    pub fn friend_key(&self, id: &str) -> Option<[u8; 32]> {
        self.friends.iter().find(|f| f.id == id).map(|f| f.public_key)
    }
    /// 친구를 고정함 (같은 id나 같은 키의 예전 항목은 지움)
    pub fn add_friend(&mut self, id: &str, key: [u8; 32]) {
        self.friends.retain(|f| f.id != id && f.public_key != key);
        self.friends.push(Friend { id: id.to_string(), public_key: key });
    }
    /// 이 친구를 너무 최근에 고정해서, 상대가 우리를 고정하기 전에 보낸 hello가 아직 올 수 있는지 봄
    /// (hello는 세션 시작에 나가고 세션은 PRE_AUTH_MAX 안에 전함. 양쪽이 거의 함께 고정함)
    pub fn just_paired(&self, id: &str, now: Instant) -> bool {
        self.paired_at.get(id).is_some_and(|t| now.duration_since(*t) < PRE_AUTH_MAX * 2)
    }
    /// 더는 친구가 아닌 이의 친구 링크를 보통 링크로 바꿔 호출자가 닫게 함 (`announced`는 남음: Disconnected가 따라옴)
    pub fn demote_friend_link(&mut self, id: &str) -> Option<Arc<secure::SecureLink>> {
        let token = self.friend_links.remove(id)?;
        self.conns.get_mut(&token).map(|c| {
            c.kind = Kind::Stranger(None);
            c.link.clone()
        })
    }
    /// 친구와 그에 딸린 기록(예산, 기기 연결, 코드, 소개, 미확인, 신뢰 주소)을 지움
    pub fn remove_friend(&mut self, id: &str) {
        self.friends.retain(|f| f.id != id);
        self.paired_at.remove(id);
        self.budgets.remove(id);
        if self.device.as_ref().is_some_and(|(d, _)| d == id) {
            self.device = None; // unfriending also unlinks
        }
        if self.link_code.as_ref().is_some_and(|c| c.friend == id) {
            self.link_code = None;
        }
        self.via.remove(id);
        self.pending.remove(id);
        self.pending_denied.remove(id);
        // Its address is no reason for trust any more
        self.trusted_ips.retain(|(friend, _), _| friend != id);
        self.trusted_rate.retain(|(friend, _), _| friend != id);
        self.friend_rate.retain(|(friend, _), _| friend != id);
    }
    /// 📱 대기 중엔 내 연결된 컴퓨터만 링크를 가질 수 있음: `id`가 막히는지 봄
    pub fn standby_blocks(&self, id: &str) -> bool {
        self.standby && !self.device.as_ref().is_some_and(|(d, _)| d == id)
    }
    /// 📱 대기 중엔 친구 키(연결된 컴퓨터 것 제외)에서 온 링크를 hello 전에 닫음: `key`가 그런지 봄
    pub fn standby_refuses_key(&self, key: &[u8; 32]) -> bool {
        self.standby && self.friends.iter().any(|f| f.public_key == *key && self.standby_blocks(&f.id))
    }
    /// 친구 `id`가 이 주소에서 인증한 적 있는지 봄
    pub fn is_trusted(&self, id: &str, ip: &str) -> bool {
        self.trusted_ips.contains_key(&(id.to_string(), ip.to_string()))
    }
    /// 어떤 친구든 이 주소에서 인증한 적 있는지 봄
    pub fn ip_trusted(&self, ip: &str) -> bool {
        self.trusted_ips.keys().any(|(_, i)| i == ip)
    }
    /// 친구 `id`가 이 주소에서 인증했음을 기억함 (꽉 차면 가장 오래된 것을 밀어냄)
    pub fn trust_ip(&mut self, id: &str, ip: &str, now: Instant) {
        let key = (id.to_string(), ip.to_string());
        if self.trusted_ips.len() >= MAX_TRUSTED_IPS && !self.trusted_ips.contains_key(&key) {
            let oldest = self.trusted_ips.iter().min_by_key(|(_, t)| **t).map(|(i, _)| i.clone());
            if let Some(o) = oldest {
                self.trusted_ips.remove(&o);
            }
        }
        self.trusted_ips.insert(key, now);
    }
    /// 친구 `id`의 인증된 링크를 돌려줌
    pub fn friend_conn(&self, id: &str) -> Option<&Conn> {
        self.friend_links.get(id).and_then(|t| self.conns.get(t))
    }
    /// 친구 `id`가 릴레이로만 이어졌는지 봄
    pub fn relay_linked(&self, id: &str) -> bool {
        self.friend_conn(id).is_some_and(|c| c.peer_addr.starts_with(relay::ADDR_PREFIX))
    }
    /// `key`가 아직 재다이얼 대기 중인지 봄
    pub fn backed_off(&self, key: &Backoff, now: Instant) -> bool {
        self.backoff.get(key).is_some_and(|(until, _)| now < *until)
    }
    /// `key`의 대기를 시작하거나 두 배로 늘림 (처음 `base`, 최대 `MAX_BACKOFF`)
    pub fn back_off(&mut self, key: Backoff, base: Duration, now: Instant) {
        let delay = match self.backoff.get(&key) {
            Some((_, d)) => (*d * 2).min(MAX_BACKOFF),
            None => base.min(MAX_BACKOFF),
        };
        if self.backoff.len() >= 256 && !self.backoff.contains_key(&key) {
            let oldest = self.backoff.iter().min_by_key(|(_, (u, _))| *u).map(|(k, _)| k.clone());
            if let Some(o) = oldest {
                self.backoff.remove(&o);
            }
        }
        self.backoff.insert(key, (now + delay, delay));
    }
}

/// 모든 스레드가 같이 쓰는 네트워크 몸통임
pub(crate) struct Shared {
    /// 이 설치 id
    pub id: String,
    /// 표시 이름 ([`Net::set_name`]으로 바뀜)
    name: Mutex<String>,
    /// 장기 정적 개인키
    pub static_key: [u8; 32],
    /// `static_key`의 공개 반쪽, 한 번만 계산함: 스칼라 곱셈이라 상태 잠금 안에서 메시지마다 하기엔 느림
    pub public_key: [u8; 32],
    /// 시간 상수
    pub timing: Timing,
    /// LAN 링크를 받는 TCP 포트
    pub tcp_port: u16,
    /// 설정된 경우의 릴레이 전송
    pub relay: Option<Arc<relay::RelayClient>>,
    /// 멈춤 깃발
    pub stop: AtomicBool,
    /// 네트워크 상태 (잠금 하나)
    pub state: Mutex<State>,
    events: Sender<NetEvent>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    next_token: AtomicU64,
    unverified: Mutex<Vec<Unverified>>,
    /// hello 전인 들어온 세션 스레드 수, `handshake_class`마다 (각각 `MAX_HANDSHAKES`): 낯선 이는 자기 등급만 채울 수 있음
    handshakes: [AtomicUsize; 3],
}

/// 링크가 어느 `Shared::handshakes` 칸에 세지는지 돌려줌: LAN 낯선 이, 릴레이 낯선 이, 친구가 인증했던 주소
fn handshake_class(addr: &str, trusted: bool) -> usize {
    match (trusted, addr.starts_with(relay::ADDR_PREFIX)) {
        (true, _) => 2,
        (false, true) => 1,
        (false, false) => 0,
    }
}

/// 친구로 (아직) 인증되지 않은 들어온 링크임
struct Unverified {
    ip: String,
    /// 전에 친구로 인증했던 주소에서 옴
    trusted: bool,
    since: Instant,
    frames: Arc<dyn transport::FrameLink>,
    /// 링크가 친구로 인증되거나 끝나면 꺼짐
    active: Arc<AtomicBool>,
    /// 사람이 이 링크에서 이모지를 비교하는 동안 켜짐 (`SessionGuard` 참고)
    surfaced: Arc<AtomicBool>,
}

/// 미검증 들어온 링크 자리(있으면)를 풀리거나 버려질 때까지 쥠
pub(crate) struct SessionGuard {
    shared: Arc<Shared>,
    /// 켜져 있는 동안 미검증 풀에 셈
    pub unverified: Arc<AtomicBool>,
    /// 링크가 사용자에게 짝짓기를 보임: 밀려나지 않고 풀에도 안 셈 (그쪽은 `MAX_INVITES_IN`으로 제한).
    /// 그래서 두 사람이 이모지를 비교하는 동안 폭주가 짝짓기를 끝내지 못함
    pub surfaced: Arc<AtomicBool>,
    /// 켜져 있는 동안 `MAX_HANDSHAKES`에 셈
    handshaking: AtomicBool,
    class: usize,
}

impl SessionGuard {
    /// 상대 hello가 옴: 이 스레드는 더는 핸드셰이크로 안 셈
    pub fn handshake_done(&self) {
        if self.handshaking.swap(false, Ordering::SeqCst) {
            self.shared.handshakes[self.class].fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for SessionGuard {
    /// 핸드셰이크 셈과 미검증 자리를 풂
    fn drop(&mut self) {
        self.handshake_done();
        self.shared.release_unverified(&self.unverified);
    }
}

impl Shared {
    /// 지금 표시 이름을 돌려줌 (`hello`와 알림에 실림)
    pub fn name(&self) -> String {
        self.name.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    /// 상태 잠금을 잡음 (독이 든 잠금도 그대로 씀)
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// 앱에 이벤트를 보냄 (받는 쪽이 없어도 괜찮음)
    pub fn emit(&self, ev: NetEvent) {
        let _ = self.events.send(ev);
    }
    /// 멈추는 중인지 봄
    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    /// 새 세션 토큰을 줌
    pub fn token(&self) -> u64 {
        self.next_token.fetch_add(1, Ordering::Relaxed)
    }
    /// `shutdown`이 기다릴 워커 스레드를 띄움. OS가 스레드를 거절하면 `false`이고 `f`는 버려짐
    pub fn spawn(&self, f: impl FnOnce() + Send + 'static) -> bool {
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        threads.retain(|t| !t.is_finished());
        match std::thread::Builder::new().name("screenpet-net".into()).spawn(f) {
            Ok(h) => {
                threads.push(h);
                true
            }
            Err(_) => false,
        }
    }
    /// `addr`에서 온 새 연결이 세션을 받아도 되는지 봄. 한 주소의 폭주는 자기 예산만 태우고 다른 곳 친구는 멀쩡하며,
    /// 친구가 인증했던 주소는 예산이 더 큼
    pub fn allow_inbound(&self, addr: &str, now: Instant) -> bool {
        let mut st = self.state();
        let limit = if st.ip_trusted(addr) { MAX_CONNS_PER_TRUSTED_ADDR } else { MAX_CONNS_PER_ADDR };
        discovery::rate_allow(&mut st.conn_rate, addr.to_string(), now, limit)
    }
    /// 들어온 링크를 미검증 풀에 들임. 같은 주소에서 온 가장 오래된 미검증 링크(`MAX_UNVERIFIED_PER_IP`만큼 있으면)나
    /// 전체에서 가장 오래된 것(풀이 차면)을 밀어내므로 폭주는 친구를 못 막고 다른 미검증 링크만 갈아치움.
    /// 그 링크 등급의 `MAX_HANDSHAKES` 스레드가 이미 핸드셰이크 중일 때만 None이고, 링크는 버려지고 상대가 다시 시도함.
    /// 그래서 LAN 낯선 이 폭주는 릴레이나 친구가 썼던 주소의 링크를 절대 거절하게 만들지 못함.
    /// 친구로 인증했던 주소의 링크는 주소당 따로 둔 몫을 써서 낯선 이가 밀어낼 수 없음
    pub fn admit_inbound(self: &Arc<Self>, frames: Arc<dyn transport::FrameLink>) -> Option<SessionGuard> {
        let ip = frames.peer_addr();
        let trusted = self.state().ip_trusted(&ip);
        let class = handshake_class(&ip, trusted);
        if self.handshakes[class].fetch_add(1, Ordering::SeqCst) >= MAX_HANDSHAKES {
            self.handshakes[class].fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        let active = Arc::new(AtomicBool::new(true));
        let surfaced = Arc::new(AtomicBool::new(false));
        let mut evicted = Vec::new();
        {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
            let mut evict_oldest = |pool: &mut Vec<Unverified>, same_ip: bool| {
                let victim = pool
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.trusted == trusted && !u.surfaced.load(Ordering::SeqCst) && (!same_ip || u.ip == ip))
                    .min_by_key(|(_, u)| u.since)
                    .map(|(i, _)| i);
                if let Some(i) = victim {
                    let u = pool.swap_remove(i);
                    u.active.store(false, Ordering::SeqCst);
                    evicted.push(u.frames);
                }
            };
            let counted = |u: &&Unverified| u.trusted == trusted && !u.surfaced.load(Ordering::SeqCst);
            let same_ip = pool.iter().filter(counted).filter(|u| u.ip == ip).count();
            if trusted {
                if same_ip >= MAX_TRUSTED_PER_IP {
                    evict_oldest(&mut pool, true);
                } else if pool.iter().filter(counted).count() >= MAX_TRUSTED_INBOUND {
                    evict_oldest(&mut pool, false);
                }
            } else if same_ip >= MAX_UNVERIFIED_PER_IP {
                evict_oldest(&mut pool, true);
            } else if pool.iter().filter(counted).count() >= MAX_UNVERIFIED_INBOUND {
                evict_oldest(&mut pool, false);
            }
            pool.push(Unverified { ip, trusted, since: Instant::now(), frames, active: active.clone(), surfaced: surfaced.clone() });
        }
        for f in evicted {
            f.close();
        }
        Some(SessionGuard { shared: self.clone(), unverified: active, surfaced, handshaking: AtomicBool::new(true), class })
    }
    /// 나가는 링크용 가드임. 미검증 자리를 쥐지 않음
    pub fn outbound_guard(self: &Arc<Self>) -> SessionGuard {
        SessionGuard { shared: self.clone(), unverified: Arc::new(AtomicBool::new(false)), surfaced: Arc::new(AtomicBool::new(false)), handshaking: AtomicBool::new(false), class: 0 }
    }
    /// 링크가 친구로 드러남(또는 끝남): 미검증 자리를 풂
    pub fn release_unverified(&self, flag: &AtomicBool) {
        if flag.swap(false, Ordering::SeqCst) {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
        }
    }
}

/// 브로드캐스트·읽기 시간 제한을 켠 찾기 UDP 소켓을 엶
fn discovery_socket(port: u16) -> std::io::Result<UdpSocket> {
    let udp = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
    udp.set_broadcast(true)?;
    udp.set_read_timeout(Some(TICK))?;
    Ok(udp)
}

/// 돌고 있는 네트워크의 손잡이임. 버리거나 `shutdown`하면 모든 스레드를 멈추고 소켓을 닫음
pub struct Net {
    shared: Arc<Shared>,
    main_threads: Vec<JoinHandle<()>>,
    discovery_port: u16,
}

impl Net {
    /// OS 암호 난수로 새 장기 정적 개인키를 만듦. 설치마다 한 번 만들어 저장할 것
    pub fn generate_static_key() -> [u8; 32] {
        noise::generate_static_key()
    }

    /// 정적 개인키의 공개키를 돌려줌 (친구가 고정하는 것)
    pub fn public_key(static_key: &[u8; 32]) -> [u8; 32] {
        noise::public_key(static_key)
    }

    /// 네트워크를 시작함: 찾기·TCP 받기·릴레이 스레드를 띄우고 이벤트 채널을 돌려줌.
    /// 찾기 포트를 못 잡아도 릴레이가 있으면 릴레이만으로 돎 (없으면 오류)
    pub fn start(cfg: NetConfig) -> std::io::Result<(Net, Receiver<NetEvent>)> {
        // The discovery port can be taken by another program: friends through
        // the relay don't need it. Without a relay nothing could work.
        let udp = match discovery_socket(cfg.discovery_port) {
            Ok(u) => Some(u),
            Err(e) if cfg.relay_url.as_deref().is_some_and(|u| !u.is_empty()) => {
                eprintln!("screenpet discovery off (port {}): {e}; relay only", cfg.discovery_port);
                None
            }
            Err(e) => return Err(e),
        };
        let discovery_port = udp.as_ref().and_then(|u| u.local_addr().ok()).map_or(cfg.discovery_port, |a| a.port());
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        listener.set_nonblocking(true)?;
        let tcp_port = listener.local_addr()?.port();

        let (tx, rx) = channel();
        let mut state = State::default();
        let own_public = noise::public_key(&cfg.static_key);
        for f in &cfg.friends {
            if f.id != cfg.id && !f.id.is_empty() && f.id.len() <= protocol::MAX_ID && f.public_key != own_public {
                state.add_friend(&f.id, f.public_key);
            }
        }
        if let Some((id, secret)) = cfg.device.as_ref().filter(|(id, _)| state.is_friend(id)) {
            state.device = Some((id.clone(), secret.0));
        }
        for (id, via) in &cfg.introduced {
            let mine = state.device.as_ref().is_some_and(|(d, _)| d == id);
            if id != via && state.is_friend(id) && state.is_friend(via) && !mine {
                state.via.insert(id.clone(), via.clone());
            }
        }
        state.active = (0, cfg.id.clone());
        state.last_input = Some(Instant::now());
        let now = devices::unix_now();
        for id in &cfg.pending {
            if state.is_friend(id) {
                let added = cfg.pending_added.iter().find(|(p, _)| p == id).map_or(now, |(_, t)| (*t).min(now));
                state.pending.insert(id.clone(), added);
            }
        }
        let shared = Arc::new(Shared {
            id: cfg.id,
            name: Mutex::new(protocol::clamp_name(&cfg.name)),
            static_key: cfg.static_key,
            public_key: own_public,
            timing: cfg.timing,
            tcp_port,
            relay: cfg.relay_url.filter(|u| !u.is_empty()).map(|u| relay::RelayClient::new(u, cfg.relay_key)),
            stop: AtomicBool::new(false),
            state: Mutex::new(state),
            events: tx,
            threads: Mutex::new(Vec::new()),
            next_token: AtomicU64::new(1),
            unverified: Mutex::new(Vec::new()),
            handshakes: Default::default(),
        });
        let s1 = shared.clone();
        let s2 = shared.clone();
        let broadcast_to = cfg.broadcast_to;
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| std::thread::Builder::new().name(name.into()).spawn(f);
        let mut main_threads = vec![spawn("screenpet-discovery", Box::new(move || discovery::run(s1, udp, broadcast_to)))?];
        let mut started = spawn("screenpet-accept", Box::new(move || tcp::accept_loop(s2, listener))).map(|h| main_threads.push(h));
        if let (Ok(()), Some(client)) = (&started, shared.relay.clone()) {
            let s3 = shared.clone();
            started = spawn("screenpet-relay", Box::new(move || relay::run(s3, client))).map(|h| main_threads.push(h));
        }
        if let Err(e) = started {
            shared.stop.store(true, Ordering::SeqCst);
            for t in main_threads {
                let _ = t.join();
            }
            return Err(e);
        }
        Ok((Net { shared, main_threads, discovery_port }, rx))
    }

    /// 찾기가 묶인 UDP 포트를 돌려줌
    pub fn discovery_port(&self) -> u16 {
        self.discovery_port
    }

    /// 찾은 낯선 이에게 짝짓기를 청함. `InviteSent{sas}`(그 뒤 `answer_invite`로 확인)와 상대의 `InviteAnswered`가 옴
    pub fn invite(&self, peer_id: &str, pet: &str) {
        peers::invite(&self.shared, peer_id, pet);
    }

    /// 릴레이 친구 코드(예: 사용자가 입력한 `ABCD-1234`)의 주인과 짝지음.
    /// `CodeRedeemed` 뒤 보통의 `InviteSent` / `InviteAnswered`, 또는 `CodeFailed`가 옴. 막지 않음
    pub fn redeem_code(&self, code: &str, pet: &str) {
        relay::redeem(&self.shared, code, pet);
    }

    /// 릴레이 연결이 살아 있는지 봄
    pub fn relay_connected(&self) -> bool {
        self.shared.relay.as_ref().is_some_and(|r| r.connected())
    }

    /// 릴레이에 새 친구 코드를 청함 (예: 옛 코드를 너무 널리 나눈 뒤). 미접속이면 `false`. `FriendCode`가 옴
    pub fn new_friend_code(&self) -> bool {
        relay::new_code(&self.shared)
    }

    /// 💌 봉인된 건의 편지([`letter::seal`])를 릴레이의 개발자 편지함으로 보냄. 답은 [`NetEvent::LetterSent`]로 옴
    pub fn send_letter(&self, sealed: &[u8]) -> bool {
        match &self.shared.relay {
            Some(c) => relay::send_letter(&self.shared, c, sealed),
            None => {
                self.shared.emit(NetEvent::LetterSent { result: Err("offline".into()) });
                false
            }
        }
    }

    /// ☀️ 이제부터 기본 지역(도시 id, 예: "yongin", `screenpet_core::real_weather::Place::id`)의 날씨를 원함.
    /// 릴레이에 알리고(지금과 재연결마다, 날씨를 아는 릴레이에만) [`NetEvent::Weather`]로 답이 옴.
    /// id 말고는 이 컴퓨터를 떠나지 않고, 릴레이는 모르는 id를 무시함
    pub fn want_weather(&self, place: &str) {
        if let Some(c) = &self.shared.relay {
            relay::want_weather(c, place);
        }
    }

    /// 📬 개발자 편지함을 청함 (릴레이가 이 설치를 독자로 둘 때만 [`NetEvent::Mailbox`]로 답함)
    pub fn mailbox_list(&self) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_list(c))
    }

    /// 📬 편지를 읽음으로 표시함
    pub fn mailbox_read(&self, id: u64) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Read(id)))
    }

    /// 편지에 반영함 체크를 하거나 풂
    pub fn mailbox_done(&self, id: u64, done: bool) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Done(id, done)))
    }

    /// 🙅 편지를 반려하거나 풂. v3 릴레이만 (아니면 `false`)
    pub fn mailbox_reject(&self, id: u64, rejected: bool) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Reject(id, rejected)))
    }

    /// 🔒 편지를 공개 건의함에서 숨기거나(비공개) 다시 보임. v3 릴레이만 (아니면 `false`)
    pub fn mailbox_private(&self, id: u64, private: bool) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Private(id, private)))
    }

    /// 📮 이 개발자 앱이 연 편지의 공개 요약([`board::digest`])을 올림. v3 릴레이, 편지함 독자만
    pub fn board_put(&self, items: &[board::Digest]) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::board_put(c, items))
    }

    /// 📮 공개 건의함 `page`쪽(1부터)을 청함. 답은 [`NetEvent::Board`]로 옴 (청할 수 없으면 바로)
    pub fn board_list(&self, page: u32) {
        match &self.shared.relay {
            Some(c) => relay::board_list(&self.shared, c, page),
            None => self.shared.emit(NetEvent::Board { result: Err("offline".into()) }),
        }
    }

    /// 📬 편지를 지움
    pub fn mailbox_delete(&self, id: u64) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Delete(id)))
    }

    /// 📬 이 보낸이 키의 편지를 거절하고 그 편지들을 지움
    pub fn mailbox_block(&self, from: &str) -> bool {
        self.shared.relay.as_ref().is_some_and(|c| relay::mailbox_edit(c, relay::MailboxEdit::Block(from.to_string())))
    }

    /// 연결된 뒤의 지금 릴레이 친구 코드를 돌려줌
    pub fn friend_code(&self) -> Option<String> {
        self.shared.relay.as_ref().and_then(|r| r.code())
    }

    /// `pairing`(`Invited` / `InviteSent`에서 옴)으로 가리킨 짝짓기를 확인(`true`, SAS가 맞음)하거나 거절함.
    /// 두 사용자가 다 확인해야 양쪽이 서로 키를 고정하고 `Connected`가 따라옴
    pub fn answer_invite(&self, pairing: u64, accept: bool) {
        peers::answer_invite(&self.shared, pairing, accept);
    }

    /// 연결된 친구의 링크를 돌려줌 (짝짓기 중이거나 다른 이는 아님)
    fn friend_link(&self, peer_id: &str) -> Option<Arc<secure::SecureLink>> {
        let st = self.shared.state();
        st.friend_conn(peer_id).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.link.clone())
    }

    /// 연결된 친구에게 놀러 가기 메시지를 큐에 넣음 (아니면 버림). 막지 않음
    pub fn send(&self, peer_id: &str, msg: Visit) {
        if let Some(link) = self.friend_link(peer_id) {
            link.send(&Message::Visit(msg));
        }
    }

    /// 연결된 친구 빌드가 사다리 메시지를 알 때만 큐에 넣음 (옛 빌드는 모르는 `type`에 링크를 끊음).
    /// `false`면 안 보냄. 막지 않음
    pub fn send_ladder(&self, peer_id: &str, msg: LadderMsg) -> bool {
        let old = self.peer_version(peer_id).is_none_or(|v| v < LADDER_MIN_VERSION);
        match self.friend_link(peer_id) {
            Some(link) if !old => link.send(&Message::Ladder(msg)),
            _ => false,
        }
    }

    /// 연결된 친구 빌드의 프로토콜 버전을 돌려줌 (미접속이면 None)
    pub fn peer_version(&self, peer_id: &str) -> Option<u32> {
        let st = self.shared.state();
        st.friend_conn(peer_id).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.v)
    }

    /// 연결된 친구에게 앱 요청(예: "invite", "busy")을 보냄 (아니면 버림). ping에 실어 옛 버전은 무시함. 막지 않음
    pub fn ask(&self, peer_id: &str, ask: &str) {
        self.ping(peer_id, Some(protocol::clamp_name(ask)), None, None);
    }

    /// 연결된 친구에게 놀러 간 우리 펫을 조종함 (조종 모드, 아니면 버림). ping에 실어 옛 버전은 무시함. 막지 않음
    pub fn steer(&self, peer_id: &str, steer: Steer) {
        self.ping(peer_id, None, Some(steer), None);
    }

    /// 연결된 친구 화면에 물건을 던짐 (아니면 버림). ping에 실어 옛 버전은 무시함. 막지 않음
    pub fn toss(&self, peer_id: &str, toss: Toss) {
        self.ping(peer_id, None, None, Some(toss));
    }

    /// 덧붙일 것을 실은 ping을 친구 링크로 보냄
    fn ping(&self, peer_id: &str, ask: Option<String>, steer: Option<Steer>, toss: Option<Toss>) {
        if let Some(link) = self.friend_link(peer_id) {
            link.send(&Message::Control(Control::Ping { ask, steer, toss }));
        }
    }

    /// 친구를 잊고 알린 뒤 (우아하게) 끊음. 그가 소개한 기기도 함께 빠짐 ([`NetEvent::FriendDropped`]). 막지 않음
    pub fn unfriend(&self, peer_id: &str) {
        let (link, dropped) = {
            let mut st = self.shared.state();
            st.remove_friend(peer_id);
            st.invites_out.remove(peer_id);
            let dropped = devices::drop_introduced(&self.shared, &mut st, peer_id);
            (st.demote_friend_link(peer_id), dropped)
        };
        if let Some(l) = link {
            l.send(&Message::Control(Control::Unfriend));
            l.finish();
        }
        devices::close_dropped(dropped);
    }

    /// 친구 `id`용 새 연결 코드를 보임 (`ABCD-EFGH`, 10분 유효, 한 번만, 5번 틀리면 사라짐, 예전 코드는 바뀜).
    /// 사용자가 그 다른 컴퓨터에 입력하면([`Net::enter_link_code`]) 둘 다 [`NetEvent::DeviceLinked`]를 받음.
    /// 이미 다른 컴퓨터와 연결됐으면 거절함 (컴퓨터는 둘까지)
    pub fn new_link_code(&self, id: &str) -> Result<String, LinkError> {
        let mut st = self.shared.state();
        if !st.is_friend(id) {
            return Err(LinkError::NotFriend);
        }
        if let Some((other, _)) = st.device.as_ref().filter(|(d, _)| d != id) {
            return Err(LinkError::Taken(other.clone()));
        }
        let (shown, code) = devices::new_code(id);
        st.link_code = Some(code);
        Ok(shown)
    }

    /// 사용자가 친구 `id` 컴퓨터가 우리에게 보여 주는 코드를 입력함. 코드를 보내지 않고 친구 링크에서 증명함.
    /// [`NetEvent::DeviceLinked`]나 [`NetEvent::DeviceLinkFailed`]가 옴. 막지 않음
    pub fn enter_link_code(&self, id: &str, code: &str) -> Result<(), LinkError> {
        let key = devices::typed_code_key(code).ok_or(LinkError::Invalid)?;
        let mut st = self.shared.state();
        if !st.is_friend(id) {
            return Err(LinkError::NotFriend);
        }
        if let Some((other, _)) = st.device.as_ref().filter(|(d, _)| d != id) {
            return Err(LinkError::Taken(other.clone()));
        }
        let Some((token, v, link)) = st.friend_links.get(id).and_then(|t| st.conns.get(t).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| (*t, c.v, c.link.clone()))) else {
            return Err(LinkError::Offline);
        };
        if v < DEVICES_MIN_VERSION {
            return Err(LinkError::Old);
        }
        let msg = devices::start_entering(&mut st, token, key).ok_or(LinkError::Offline)?;
        drop(st);
        link.send(&Message::Control(msg));
        Ok(())
    }

    /// 내 다른 컴퓨터 `id`와 연결을 끊음: 비밀을 잊고 알림 (그쪽도 잊음). 친구로는 남음. 연결 안 돼 있었으면 `false`
    pub fn unlink_device(&self, id: &str) -> bool {
        {
            let mut st = self.shared.state();
            if !st.device.as_ref().is_some_and(|(d, _)| d == id) {
                return false;
            }
            st.device = None;
            if let Some(c) = st.friend_links.get(id).copied().and_then(|t| st.conns.get_mut(&t)) {
                c.dev.proven = false;
            }
        }
        if let Some(link) = self.devices_link(id) {
            link.send(&Message::Control(Control::DeviceUnlink));
        }
        true
    }

    /// 친구 `id`에 대한 [`NetEvent::DeviceConfirm`]의 사용자 답 (`true` = 확인, `false` = 취소나 창 닫음).
    /// 그 친구에게서 답을 기다리는 게 없으면 `false`
    pub fn confirm_device(&self, id: &str, yes: bool) -> bool {
        let told = devices::answer(&self.shared, &mut self.shared.state(), id, yes);
        let Some((link, msg)) = told else { return false };
        link.send(&Message::Control(msg));
        true
    }

    /// 📱 이 컴퓨터에서 놂: 연결된 컴퓨터는 대기로 감 (증명된 링크로 알림). 사용자가 이 컴퓨터를 쓴 것으로도 침.
    /// 그 링크가 없으면 `false` (그럼 서로 닿지 않아 어차피 둘 다 놀고, 다시 만나면 마지막에 쓴 쪽이 놂)
    pub fn claim_active(&self) -> bool {
        let Some((link, msg)) = devices::claim(&self.shared) else { return false };
        link.send(&Message::Control(msg))
    }

    /// 📱 사용자가 이 컴퓨터를 마지막으로 쓴 지 얼마인지 (앱이 주기적으로 알림). 두 컴퓨터가 다시 만날 때 누가 놀지 정함
    pub fn set_idle(&self, idle: Duration) {
        let now = Instant::now();
        self.shared.state().last_input = Some(now.checked_sub(idle).unwrap_or(now));
    }

    /// 📱 고른 펫과 모든 펫의 돌봄이 바뀜 (또는 주기 새로 고침): 이 컴퓨터가 노는 동안 연결된 컴퓨터와 나눔
    /// (`Timing::device_state_every`로 모음). 대기 중에 바꾸면 앱이 차지도 해야만 여기서 노는 것으로 침
    pub fn share_state(&self, pet: &str, care: Vec<WireCare>) {
        devices::share(&mut self.shared.state(), pet, care);
    }

    /// 📱 이 컴퓨터가 대기 중인지 봄: 다른 컴퓨터가 놂
    pub fn standby(&self) -> bool {
        self.shared.state().standby
    }

    /// 📱 연결된 컴퓨터가 대기 중인지 봄: 이 컴퓨터가 놂 (내 펫을 보내지 말 것)
    pub fn device_standby(&self) -> bool {
        self.shared.state().device_standby
    }

    /// 내 연결된 다른 컴퓨터를 돌려줌
    pub fn linked_device(&self) -> Option<String> {
        self.shared.state().device.as_ref().map(|(d, _)| d.clone())
    }

    /// 연결된 컴퓨터 `id`와 나눈 비밀을 돌려줌 (저장용, 연결됐을 때만)
    pub fn device_secret(&self, id: &str) -> Option<DeviceSecret> {
        self.shared.state().device.as_ref().filter(|(d, _)| d == id).map(|(_, s)| DeviceSecret(*s))
    }

    /// 친구 `id`의 기기 표시와 출처를 돌려줌 (친구가 아니면 None)
    pub fn friend_info(&self, id: &str) -> Option<FriendInfo> {
        let st = self.shared.state();
        st.is_friend(id).then(|| FriendInfo { device: st.device.as_ref().is_some_and(|(d, _)| d == id), via: st.via.get(id).cloned(), pending: st.pending.contains_key(id), pending_since: st.pending.get(id).copied() })
    }

    /// 빌드가 기기 메시지를 아는 연결된 친구의 링크를 돌려줌
    fn devices_link(&self, peer_id: &str) -> Option<Arc<secure::SecureLink>> {
        let new = self.peer_version(peer_id).is_some_and(|v| v >= DEVICES_MIN_VERSION);
        self.friend_link(peer_id).filter(|_| new)
    }

    /// `peer_id`가 내 연결된 컴퓨터이고 지금 링크에서 비밀을 증명했으면 내 친구들을 보냄 (아니면 `false`, 안 보냄).
    /// 소개된·미확인 친구와 받는 쪽 자신은 뺌 (그들은 자기 출처에서 옴). `describe(id)`가 친구마다 이름과 가장자리를 줌
    /// (받는 쪽 첫값). `want`면 상대 목록도 달라고 함. 막지 않음
    pub fn send_device_friends(&self, peer_id: &str, want: bool, describe: impl Fn(&str) -> (String, Option<String>)) -> bool {
        let proven = {
            let st = self.shared.state();
            st.device.as_ref().is_some_and(|(d, _)| d == peer_id) && st.friend_conn(peer_id).is_some_and(|c| c.dev.proven)
        };
        if !proven {
            return false;
        }
        let Some(link) = self.devices_link(peer_id) else { return false };
        let ids: Vec<(String, [u8; 32])> = {
            let st = self.shared.state();
            st.friends
                .iter()
                .filter(|f| f.id != peer_id && !st.via.contains_key(&f.id) && !st.pending.contains_key(&f.id))
                .take(MAX_FRIENDS)
                .map(|f| (f.id.clone(), f.public_key))
                .collect()
        };
        let friends = ids
            .into_iter()
            .map(|(id, key)| {
                let (name, side) = describe(&id);
                protocol::WireFriend { key: to_hex(&key), name: protocol::clamp_name(&name), side, id }
            })
            .collect();
        link.send(&Message::Control(Control::DeviceFriends { friends, want }))
    }

    /// 친구 `peer_id`에게 내 연결된 컴퓨터를 소개함 (`MAX_DEVICES`). 그가 그 컴퓨터이거나 빌드가 메시지를 모르면 안 함.
    /// `false` = 안 보냄 (다른 컴퓨터가 없을 때도). 막지 않음
    pub fn send_my_devices(&self, peer_id: &str) -> bool {
        let devices: Vec<protocol::WireDevice> = {
            let st = self.shared.state();
            let mine = st.device.as_ref().map(|(d, _)| d.as_str());
            if mine == Some(peer_id) {
                return false;
            }
            st.friends
                .iter()
                .filter(|f| f.id != peer_id && mine == Some(f.id.as_str()) && !st.pending.contains_key(&f.id))
                .take(MAX_DEVICES)
                .map(|f| protocol::WireDevice { id: f.id.clone(), key: to_hex(&f.public_key) })
                .collect()
        };
        if devices.is_empty() {
            return false;
        }
        match self.devices_link(peer_id) {
            Some(link) => link.send(&Message::Control(Control::MyDevices { devices })),
            None => false,
        }
    }

    /// 표시 이름을 바꿈: 이후 `hello`와 알림이 쓰고, 연결된 친구 모두에게 지금 알림
    /// (친구 링크의 반복 `hello`, 친구 쪽은 [`NetEvent::Renamed`]를 냄, 옛 버전은 무시함). 막지 않음
    pub fn set_name(&self, name: &str) {
        let name = protocol::clamp_name(name);
        *self.shared.name.lock().unwrap_or_else(|e| e.into_inner()) = name.clone();
        let links: Vec<_> = {
            let st = self.shared.state();
            st.friend_links.values().filter_map(|t| st.conns.get(t)).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.link.clone()).collect()
        };
        let hello = Message::Control(Control::Hello { id: self.shared.id.clone(), name, v: protocol::PROTOCOL_VERSION, friend: true });
        for l in links {
            l.send(&hello);
        }
    }

    /// 지금 친구들 (id + 고정 공개키), 짝짓기로 더한 것 포함. 앱이 저장해 `NetConfig`로 다시 넘김
    pub fn friends(&self) -> Vec<Friend> {
        self.shared.state().friends.clone()
    }

    /// 네트워크를 멈춤 (`Drop`과 같음)
    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for Net {
    /// 멈춤 깃발을 세우고 링크를 닫은 뒤 모든 스레드를 기다림
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let links: Vec<_> = self.shared.state().conns.values().map(|c| c.link.clone()).collect();
        for l in links {
            l.close();
        }
        for t in self.main_threads.drain(..) {
            let _ = t.join();
        }
        // Workers can still spawn link writers; drain until none are left.
        loop {
            let workers: Vec<_> =
                std::mem::take(&mut *self.shared.threads.lock().unwrap_or_else(|e| e.into_inner()));
            if workers.is_empty() {
                break;
            }
            for t in workers {
                let _ = t.join();
            }
        }
    }
}

/// 무작위 128비트 설치 id를 소문자 hex 32자리로 만듦
pub fn new_install_id() -> String {
    format!("{:032x}", fastrand::u128(..))
}

/// 아무도 안 듣는 돌고 있는 Net임. 부분 단위 시험용
#[cfg(test)]
pub(crate) fn solo_net(friends: Vec<Friend>) -> Net {
    let mut cfg = NetConfig::new("solo", "Solo", Net::generate_static_key(), friends);
    cfg.discovery_port = 0;
    cfg.broadcast_to = SocketAddr::from(([127, 0, 0, 1], 9));
    Net::start(cfg).unwrap().0
}

/// 친구별 예산 시험임
#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn the_budget_is_per_friend_and_leaves_with_the_friend() {
        let now = Instant::now();
        let mut st = State::default();
        for _ in 0..50 {
            assert!(st.budget("a", now).tosses.take(now));
        }
        assert!(!st.budget("a", now).tosses.take(now));
        // 다른 친구의 몫은 따로, 다른 종류의 몫도 따로
        assert!(st.budget("b", now).tosses.take(now));
        assert!(st.budget("a", now).visits.take(now));
        // 시간이 지나면 다시 참
        assert!(st.budget("a", now).tosses.take(now + TOSS_EVERY));
        st.remove_friend("a");
        assert!(!st.budgets.contains_key("a"));
        assert!(st.budgets.contains_key("b"));
    }
}

/// 들어온 링크 제한(핸드셰이크 상한, 미검증 풀, 주소별 예산) 시험임
#[cfg(test)]
mod inbound_tests {
    use super::*;

    /// 아무것도 안 하는 가짜 링크
    struct NoLink;
    impl transport::FrameLink for NoLink {
        fn send_frame(&self, _: &[u8]) -> bool {
            true
        }
        fn finish(&self) {}
        fn close(&self) {}
        fn peer_addr(&self) -> String {
            "10.0.0.1".into()
        }
    }

    #[test]
    fn handshake_threads_are_capped_until_one_says_hello() {
        let net = solo_net(vec![]);
        let link = || -> Arc<dyn transport::FrameLink> { Arc::new(NoLink) };
        let mut held: Vec<_> = (0..MAX_HANDSHAKES).map(|_| net.shared.admit_inbound(link()).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(link()).is_none(), "the cap is reached");
        // 하나가 끝나면 자리가 남
        held.pop();
        let again = net.shared.admit_inbound(link()).expect("a slot came free");
        assert!(net.shared.admit_inbound(link()).is_none());
        // hello까지 온 링크는 세지 않음 (끝날 때 두 번 빼지도 않음)
        again.handshake_done();
        let next = net.shared.admit_inbound(link()).expect("hello frees the slot");
        drop(again);
        assert!(net.shared.admit_inbound(link()).is_none(), "released once");
        drop(next);
        drop(held);
        assert_eq!(net.shared.handshakes[0].load(Ordering::SeqCst), 0);
    }

    /// 주소만 가진 가짜 링크
    struct At(String);
    impl transport::FrameLink for At {
        fn send_frame(&self, _: &[u8]) -> bool {
            true
        }
        fn finish(&self) {}
        fn close(&self) {}
        fn peer_addr(&self) -> String {
            self.0.clone()
        }
    }

    /// `addr`에서 온 가짜 링크를 만듦
    fn at(addr: &str) -> Arc<dyn transport::FrameLink> {
        Arc::new(At(addr.to_string()))
    }

    #[test]
    fn a_stranger_flood_never_refuses_a_trusted_or_relay_link() {
        let net = solo_net(vec![]);
        net.shared.state().trust_ip("friend", "10.0.0.9", Instant::now());
        // LAN의 낯선 주소들이 핸드셰이크 자리를 다 채움
        let flood: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("10.1.0.{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("10.1.1.1")).is_none(), "strangers are at their cap");
        assert!(net.shared.admit_inbound(at("10.0.0.9")).is_some(), "a friend's address has its own");
        assert!(net.shared.admit_inbound(at("relay:abcd")).is_some(), "the relay has its own");
        drop(flood);
    }

    #[test]
    fn relay_strangers_do_not_use_up_lan_handshakes() {
        let net = solo_net(vec![]);
        let flood: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("relay:{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("relay:x")).is_none());
        assert!(net.shared.admit_inbound(at("10.1.1.1")).is_some(), "LAN is untouched");
        drop(flood);
    }

    #[test]
    fn trusted_handshakes_and_pool_entries_are_capped_too() {
        let net = solo_net(vec![]);
        let now = Instant::now();
        for i in 0..MAX_TRUSTED_IPS {
            net.shared.state().trust_ip("friend", &format!("10.2.0.{i}"), now);
        }
        let held: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("10.2.0.{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("10.2.0.40")).is_none(), "trusted links are bounded as well");
        drop(held);
        // 풀에도 신뢰 주소 전체의 상한이 있음 (오래된 것이 밀려남)
        let mut held = Vec::new();
        for i in 0..MAX_TRUSTED_IPS {
            let g = net.shared.admit_inbound(at(&format!("10.2.0.{i}"))).expect("slot");
            g.handshake_done();
            held.push(g);
        }
        let live = net.shared.unverified.lock().unwrap().iter().filter(|u| u.trusted && u.active.load(Ordering::SeqCst)).count();
        assert!(live <= MAX_TRUSTED_INBOUND, "{live}");
    }

    #[test]
    fn removing_a_friend_forgets_where_it_authenticated_from() {
        let now = Instant::now();
        let ip: IpAddr = "10.0.0.5".parse().unwrap();
        let mut st = State::default();
        st.add_friend("a", [1; 32]);
        st.add_friend("b", [2; 32]);
        st.trust_ip("a", "10.0.0.5", now);
        st.trust_ip("b", "10.0.0.6", now);
        st.trusted_rate.insert(("a".into(), ip), (now, 1));
        st.friend_rate.insert(("a".into(), ip), (now, 1));
        st.remove_friend("a");
        assert!(!st.ip_trusted("10.0.0.5"), "no more trust from the removed friend's address");
        assert!(st.ip_trusted("10.0.0.6"), "other friends stay");
        assert!(st.trusted_rate.is_empty() && st.friend_rate.is_empty());
    }

    #[test]
    fn each_address_has_its_own_connection_budget_and_a_friends_is_larger() {
        let net = solo_net(vec![]);
        let now = Instant::now();
        let allow = |addr: &str| net.shared.allow_inbound(addr, now);
        assert_eq!((0..200).filter(|_| allow("10.0.0.1")).count(), MAX_CONNS_PER_ADDR as usize);
        assert!(allow("10.0.0.2"), "another address is untouched");
        net.shared.state().trust_ip("friend", "10.0.0.3", now);
        assert_eq!((0..200).filter(|_| allow("10.0.0.3")).count(), MAX_CONNS_PER_TRUSTED_ADDR as usize);
        assert!(net.shared.allow_inbound("10.0.0.1", now + Duration::from_secs(1)), "the budget refills");
    }
}
