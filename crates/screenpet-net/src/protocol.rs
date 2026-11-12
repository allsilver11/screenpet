//! 와이어 형식임: UDP 찾기 알림과, Noise 전송 메시지 안에 (암호화돼) 실리는 JSON 메시지들.
//! 메시지마다 `type` 필드로 꼬리표를 닮. 최상위는 [`Message`] (`Control`·`Visit`·`LadderMsg`).
//! peers·devices·ladder가 만들고 읽으며, 앱은 `NetEvent::Received`로 `Visit`·`LadderMsg`를 받음.

use serde::{Deserialize, Serialize};

/// 알림과 `hello`에 싣는 프로토콜 버전임. 2 = `ladder_*` 메시지를 앎 (v0.5.0).
/// 3 = `device_friends`, `my_devices`, `device_*` 코드 연결 메시지를 앎 (연결된 기기, v0.5.3).
/// 4 = 놀러 가는 펫이 든 물건(`Visit::carry`)을 받음 (v0.6.2). 옛 버전은 모르는 `type`이 오면
/// 링크를 닫으므로, 새 메시지 종류를 쓰는 기능은 상대 버전부터 확인함 (`Net::peer_version`)
pub const PROTOCOL_VERSION: u32 = 4;
/// `LadderMsg`를 보내도 되는 가장 낮은 상대 버전
pub const LADDER_MIN_VERSION: u32 = 2;
/// `device_friends`, `my_devices`, `device_*`를 보내도 되는 가장 낮은 상대 버전
pub const DEVICES_MIN_VERSION: u32 = 3;
/// 물건을 들고 놀러 가도 되는 가장 낮은 상대 버전 (`Visit::carry`).
/// 옛 버전은 그 필드를 무시해 물건이 가는 길에 사라짐
pub const CARRY_MIN_VERSION: u32 = 4;
/// 친구 하나가 소개하는 기기 수 상한 (`my_devices`, 메시지 하나와 전체 모두).
/// 한 사람은 컴퓨터를 둘까지만 연결하므로 친구에게 다른 하나뿐임
pub const MAX_DEVICES: usize = 1;
/// 알림에 싣는 앱 꼬리표임. 관계없는 UDP 트래픽을 무시하려고 씀
pub const APP_TAG: &str = "screenpet";
/// 링크의 가장 큰 프레임 (Noise 메시지는 최대 65535바이트), 릴레이 채널 포함
pub use screenpet_proto::MAX_FRAME;
/// 가장 큰 JSON 메시지 (Noise 프레임에서 AEAD 태그 16바이트를 뺀 것)
pub const MAX_MESSAGE: usize = MAX_FRAME - 16;
/// 받아 주는 가장 긴 설치 id (바이트), 릴레이와 같음
pub use screenpet_proto::MAX_ID;
/// 표시 이름과 펫 라벨은 이 글자 수로 자름
pub const MAX_NAME: usize = 64;
/// 설치 하나가 두는 친구 수 상한. 차면 짝짓기를 (양방향) 거절함
pub use screenpet_proto::MAX_FRIENDS;

/// 놀러 온 펫이 걸어 들어오는 화면 가장자리임
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// 위·아래 화면 가장자리임 (위·아래 친구, v0.3.6). 옛 버전이 아는 `Side` 옆에 선택 필드 `vside`로 보냄
/// (옛 버전은 그냥 왼쪽·오른쪽으로 봄). 있으면 이것이 이김
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VSide {
    Up,
    Down,
}

/// 꼭 있는 `enter_side`를 읽음. 모르는 값(더 새 가장자리)은 메시지를 버리지 않고 `left`로 읽음
fn lenient_side<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Side, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or(Side::Left))
}

/// 선택 필드를 너그럽게 읽음: 안 맞는 값은 그 필드만 버리고 메시지 전체는 안 버림
/// (더 새거나 옛 상대가 다른 것을 보낼 수 있음)
fn lenient<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(d: D) -> Result<Option<T>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| serde_json::from_value(v).ok()))
}

/// 화면 가장자리 너머로 던진 펫임 (두 번째 모니터처럼): 발 높이는 작업 영역 높이의 비율
/// (0 위 … 1 아래), 속도는 논리 px/s. 받는 쪽이 다듬고, 옛 버전은 무시하고 평소처럼 걸어 들어옴
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fling {
    /// 발 높이 비율 (0 위 … 1 아래)
    pub height: f64,
    /// 가로 속도 (px/s)
    pub vx: f64,
    /// 세로 속도 (px/s)
    pub vy: f64,
}

/// 놀러 가는 펫이 드는 쪽지의 최대 글자 수. 받는 쪽이 다시 다듬고 자름
pub const MAX_NOTE: usize = 80;

/// 받는 쪽에 놀러 간 펫을 주인이 조종함 (조종 모드): `on` = 조종 모드 켬, `keys` = 누른 키 비트
/// (왼 1, 오른 2, 위 4, 아래 8, 점프 16, 달리기 32. 모르는 비트는 무시). ping에 실어 옛 버전은 무시함.
/// 바뀔 때와 키를 누르는 동안 5 Hz로 보냄. 받는 쪽은 `visit_id`인 그 친구의 손님에게만 적용하고
/// 갱신이 끊기면 키를 놓음
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Steer {
    /// 조종하는 방문 id
    pub visit_id: u64,
    /// 조종 모드 켬
    #[serde(default)]
    pub on: bool,
    /// 누른 키 비트
    #[serde(default)]
    pub keys: u8,
}

/// 받는 쪽 화면으로 날아가는 던진 물건(쓰레기 장난감)임. 물건은 주인이 없고 집에 안 감:
/// 보낸 쪽은 잊고 받은 쪽이 자기 물건으로 가짐. ping에 실어 옛 버전은 무시함(물건만 사라지고 링크는 그대로).
/// `kind`, `fling`, `note`는 받는 앱이 검사함. `side` = 날아 들어오는 받는 쪽 가장자리
/// (`Visit::enter_side`처럼 `vside`가 있으면 그것이 이김)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Toss {
    /// 물건 종류
    pub kind: String,
    /// 들어오는 왼쪽·오른쪽 가장자리
    #[serde(default = "left_side", deserialize_with = "lenient_side")]
    pub side: Side,
    /// 위·아래 가장자리 (있으면 이김)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub vside: Option<VSide>,
    /// 날아가는 움직임
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub fling: Option<Fling>,
    /// 물건에 붙인 포스트잇
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub note: Option<String>,
    /// 받는 쪽이 거절해(집중 중이거나 집 안) 보낸 쪽에 되던졌음. 다시는 안 튕김. 옛 버전은 무시함
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bounced: bool,
    /// 그리는 크기 (똥은 펫 크기를 따라감). 받는 쪽이 제한하고 옛 버전은 무시함
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scale: Option<f64>,
    /// 색 똥("pink", "rainbow")이나 거대한 돌·똥("giant")임. kind "poop"·"stone"으로 보내
    /// 옛 버전은 보통 것을 받음. 받는 앱이 이름을 검사함
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub look: Option<String>,
    /// ⚾ 캐치볼: 공이 친구 사이를 오간 횟수임. 받는 쪽 흐물이가 하나 더해 `MAX_RALLY`까지 되던짐.
    /// 받을 때 자르고 옛 버전은 무시함
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient_rally")]
    pub rally: Option<u32>,
}

/// ⚾ 가장 긴 캐치볼 랠리. 받은 수가 넘으면 이 값으로 읽음 (코어도 같은 값을 씀)
pub use screenpet_proto::MAX_RALLY;

/// 랠리 수를 읽음: 0 이상 정수면 `MAX_RALLY`로 자르고, 그 밖은 버림
fn lenient_rally<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<u32>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| v.as_u64()).map(|n| n.min(MAX_RALLY as u64) as u32))
}

/// 🤹 놀러 가는 펫이 머리 위에 들고 가 받는 쪽 화면에 떨어뜨리는 물건(쓰레기 장난감이나 똥)임 (`Visit::carry`).
/// 못 들어가면 집으로 도로 가져옴 (`Visit::Leave::carry`). 이름은 `Toss`와 같고(`kind`, `look`, `scale`)
/// 받는 앱이 검사해 모르는 것은 버림. `CARRY_MIN_VERSION` 상대에게만 보냄
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Carry {
    /// 물건 종류
    pub kind: String,
    /// 생김새 (색 똥, 거대 등)
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub look: Option<String>,
    /// 그리는 크기
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub scale: Option<f64>,
}

/// 잘못된 항목은 건너뛰는 목록을 읽음 (메시지 전체는 안 버림).
/// 배열이 아니면 빈 목록이고 최대 256개만 남김
fn lenient_list<'de, T: serde::de::DeserializeOwned, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    let items = match v {
        serde_json::Value::Array(items) => items,
        _ => Vec::new(),
    };
    Ok(items.into_iter().take(256).filter_map(|v| serde_json::from_value(v).ok()).collect())
}

/// bool을 너그럽게 읽음: 없거나 다른 것이면 false임
fn lenient_bool<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_bool().unwrap_or(false))
}

/// 문자열을 너그럽게 읽음: 없거나 다른 것이면 빈 문자열임
fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_str().map(str::to_string).unwrap_or_default())
}

/// 내 다른 컴퓨터와 나누는 내 친구 하나임 (`device_friends`).
/// `key` = 친구 정적 공개키 (hex 64자리). `name`과 `side`는 받는 쪽 목록의 첫값일 뿐임 (받는 쪽이 검사함)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireFriend {
    /// 친구 설치 id
    pub id: String,
    /// 친구 정적 공개키 (hex)
    pub key: String,
    /// 표시 이름
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// 친구가 있는 화면 가장자리
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
    pub side: Option<String>,
}

/// 📱 두 컴퓨터가 나누는 펫 하나의 돌봄임 (`device_state`).
/// 받는 쪽은 모양을(`devices::clean_state`), 앱은 값을 검사함
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireCare {
    /// 펫 id
    pub pet: String,
    /// 성장 단계
    pub stage: String,
    /// 기운
    pub stamina: f64,
    /// 배부름
    pub fullness: f64,
    /// 성장 멈춤
    #[serde(default, deserialize_with = "lenient_bool")]
    pub locked: bool,
}

/// 친구에게 소개하는 내 다른 컴퓨터 하나임 (`my_devices`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireDevice {
    /// 그 컴퓨터의 설치 id
    pub id: String,
    /// 그 컴퓨터의 정적 공개키 (hex)
    pub key: String,
}

/// `side` 기본값임: 왼쪽
fn left_side() -> Side {
    Side::Left
}

/// 놀러 가는 펫에 관한 메시지임. 앱에는 `NetEvent::Received`로 전해짐
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Visit {
    /// 펫이 놀러 감
    Visit {
        visit_id: u64,
        species: String,
        stage: String,
        scale: f64,
        personality: String,
        owner: String,
        #[serde(deserialize_with = "lenient_side")]
        enter_side: Side,
        /// 받는 쪽 위·아래 가장자리로 대신 들어옴 (보낸 쪽이 아래·위에 있음).
        /// 그때 `enter_side`는 옛 버전용 왼쪽·오른쪽 대체값임
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        vside: Option<VSide>,
        /// 이 친구 화면에서 걸어 넘어왔음 (주인의 친구 id). 받는 쪽은 그 친구와 이은 가장자리가 있으면 거기로 들임
        #[serde(default, skip_serializing_if = "Option::is_none")]
        via: Option<String>,
        /// 펫이 집을 그리워하기까지 남은 초 (넘어가도 이어짐)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// 던져서 넘어왔음: 이 움직임을 이어 날아 들어옴
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        fling: Option<Fling>,
        /// 주인이 펫에 붙인 짧은 쪽지임 (받는 쪽 화면에 💌로 보임).
        /// 펫이 다른 친구로 넘어가면 주인이 그대로 다시 보냄. 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        note: Option<String>,
        /// 펫이 받는 쪽 화면에 두고 갈 작은 선물 (카탈로그 id)임. 모르거나 남의 id는 받는 쪽이 버리고
        /// 친구 선물을 받는 빈도도 제한함. 넘어갈 때는 다시 안 보냄. 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        gift: Option<String>,
        /// 주인이 펫을 보이는 방식: "pixel" (없으면 둥근 그림). 꾸미기일 뿐이며
        /// 받는 쪽이 검사하고 모르거나 그림이 없으면 둥근 그림으로 보임. 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        style: Option<String>,
        /// 🎨 잉크선 펫의 주인 몸 색 (색 id, 예: "rainbow". 없으면 그린 잉크). 꾸미기일 뿐이며
        /// 받는 쪽이 아는 id와 대조하고 아니면 잉크로 보임. 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        tint: Option<String>,
        /// 친구가 먹이를 줘 잠깐 커졌음 (배율). 받는 쪽이 제한하고 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        boost: Option<f64>,
        /// 주인이 성장을 멈췄음: 받는 쪽은 크게 먹이기를 내밀지 않음. 옛 버전은 무시함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        no_grow: Option<bool>,
        /// 📱 보낸 쪽의 다른 컴퓨터(이 설치 id) 펫임. 그 컴퓨터는 대기 중일 수 있어 보낸 쪽이 대신 데려감
        /// (`DEVICES_MIN_VERSION` 상대에게만). 받는 쪽은 두 id를 한 사람의 컴퓨터로 알 때만 받고, 연결된 쪽으로 답함
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        of: Option<String>,
        /// 🤹 펫이 이 물건을 들고 가 받는 쪽 화면에 떨어뜨림 (`CARRY_MIN_VERSION` 상대에게만).
        /// 잘못된 값은 그 필드만 버리고 방문은 안 버림
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        carry: Option<Carry>,
    },
    /// 손님이 받는 쪽 친구 화면을 떠남. `next: None` = 집에 감 (옛 버전도 이렇게 보냄).
    /// `next: Some(id)` = 친구 `id`와 이은 가장자리(`side`)로 나가 그 친구에게 가고 싶음.
    /// 주인은 `id`가 자기와 연결된 친구일 때만 거기로 `Visit`을 넘기고, 아니면 집으로 데려옴
    Leave {
        visit_id: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        side: Option<Side>,
        /// 위·아래 가장자리로 대신 나감 (`side`는 옛 주인용 왼쪽·오른쪽 대체값)
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        vside: Option<VSide>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        homesick_in: Option<f64>,
        /// 손님이 그 가장자리로 던져져 나감 (집으로, 또는 `next`로)
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        fling: Option<Fling>,
        /// 펫을 집에 보내며 호스트가 주인에게 쓴 답장임 (집에 가는 `Leave`에만, 방문당 한 번까지.
        /// 주인은 그 호스트 링크의 자기 외출 펫에 대해서만 받음). 옛 주인은 무시하고 펫만 돌아옴
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        note: Option<String>,
        /// 호스트가 손님에게 먹이를 줌: 얼마나 더 커지는지 (주인이 제한함)
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        boost: Option<f64>,
        /// 🤹 호스트가 들이지 않고 바로 돌려보냄: 들고 간 물건(`Visit::carry`)이 손에 들려 돌아옴
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        carry: Option<Carry>,
    },
    /// 주인이 펫을 집으로 부름
    ComeHome {
        visit_id: u64,
    },
}

impl Visit {
    /// 그냥 "집에 감" `Leave`를 만듦 (돌아다니기 없는 프로토콜 v1과 같은 모양)
    pub fn leave(visit_id: u64) -> Visit {
        Visit::Leave { visit_id, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None, carry: None }
    }
}

/// net 크레이트 안에서 처리하는 연결 관리 메시지임
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Control {
    /// Noise 핸드셰이크 뒤 첫 메시지, 양방향. `friend` = 보낸 쪽이 받는 쪽 정적 키를 친구로 고정했음
    Hello {
        id: String,
        name: String,
        v: u32,
        #[serde(default)]
        friend: bool,
    },
    /// 짝짓기 요청 (초대자가 다이얼한 링크에서만). `commit` = 초대자의 비밀 32바이트 난수 nonce_i의 hex BLAKE2s(nonce_i)
    Invite { pet: String, commit: String },
    /// 초대받은 쪽의 32바이트 난수 r_r (hex). 커밋만 본 뒤 보냄
    PairRandom { random: String },
    /// 초대자가 nonce_i(hex)를 밝힘. 초대받은 쪽이 `commit`과 대조함.
    /// SAS = BLAKE2s(handshake_hash || nonce_i || r_r)의 앞 36비트
    PairReveal { nonce: String },
    /// 이 링크 짝짓기에 대한 각자의 SAS 확인(또는 거절)
    InviteReply { accepted: bool },
    /// 친구를 끊음
    Unfriend,
    /// 연결 유지임. `ask` = 친구에게 하는 앱 요청 (예: "invite": 펫 데려와, "busy": 지금은 안 됨).
    /// 옛 버전은 필드가 더 붙은 ping을 그냥 ping으로 읽고 무시함 (`steer`, `toss`에도 씀)
    Ping {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ask: Option<String>,
        /// 받는 쪽에 놀러 간 우리 펫 조종 (조종 모드)
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        steer: Option<Steer>,
        /// 받는 쪽 화면으로 던진 물건
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        toss: Option<Toss>,
    },
    /// 내 친구들임. 이 링크에서 우리 비밀을 증명한 내 연결된 컴퓨터에 보냄 (`DEVICES_MIN_VERSION` 상대에게만).
    /// 받는 쪽은 증명이 통과한 링크에서만 받고 모르는 친구만 더함. `want` = 네 것도 보내 달라는 뜻
    DeviceFriends {
        #[serde(default, deserialize_with = "lenient_list")]
        friends: Vec<WireFriend>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        want: bool,
    },
    /// 그 컴퓨터가 아닌 친구에게 소개하는 내 다른 컴퓨터 (최대 `MAX_DEVICES`, `DEVICES_MIN_VERSION` 상대에게만).
    /// 친구는 나와 함께 빠지는 친구로 더함
    MyDevices {
        #[serde(default, deserialize_with = "lenient_list")]
        devices: Vec<WireDevice>,
    },
    /// 📱 코드로 연결하기 (모든 `device_*` 메시지는 `DEVICES_MIN_VERSION` 상대에게만).
    /// 입력한 쪽이 받는 쪽이 보여 주는 코드를 입력함: `nonce` = 새 32바이트 nonce nb (hex). 코드 자체는 선에 안 실림
    DeviceCode { nonce: String },
    /// 코드를 쥔 쪽의 새 nonce na. 보낸 쪽용 유효한 코드를 쥐었을 때만 보냄 (아니면 `device_no`)
    DeviceCodeNonce { nonce: String },
    /// 입력한 쪽이 코드 K = BLAKE2s(label || code)를 안다는 증명:
    /// BLAKE2s(label || "enter" || K || nb || na || enterer key || holder key)
    DeviceCodeProof { proof: String },
    /// 쥔 쪽이 그 증명을 확인하고 이제 자기도 증명함 (먼저 증명하지 않으므로 틀린 추측은 아무것도 못 얻음):
    /// BLAKE2s(label || "hold" || K || nb || na || holder key || enterer key).
    /// 그 뒤 둘 다 S = BLAKE2s(label || K || holder key || enterer key || na || nb)를 저장함
    DeviceCodeOk { proof: String },
    /// 코드로 연결되지 않음: `reason` = "wrong" (`left`번 남음), "locked" (너무 많이 틀려 코드가 사라짐),
    /// "none" (너용 코드 없음, 만료나 사용됨), "taken" (다른 컴퓨터와 연결됨)
    DeviceNo {
        #[serde(default, deserialize_with = "lenient_string")]
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        left: Option<u32>,
    },
    /// 연결된 두 컴퓨터 사이의 친구 링크마다 각자 새 nonce를 보내고, 상대 것에 `device_proof`로 답함 =
    /// BLAKE2s(label || S || prover key || verifier key || verifier nonce || prover nonce).
    /// 그 증명이 통과해야 친구 목록이 오감
    DeviceNonce { nonce: String },
    /// 저장된 비밀 S 증명 (`DeviceNonce` 참고)
    DeviceProof { proof: String },
    /// 연결을 잊음 (보낸 쪽이 끊었거나 우리 비밀이 없음). 맞은 코드 확인 중의 "취소"로도 씀
    DeviceUnlink,
    /// 코드가 맞은 뒤 보낸 쪽 사용자가 확인("본인의 PC가 맞습니까?")을 누름. 둘 다 보내야 연결로 침
    DeviceConfirm,
    /// 📱 연결된 두 컴퓨터 중 누가 노는지임 (비밀을 양방향으로 증명한 링크에서만, `DEVICES_MIN_VERSION` 상대에게만,
    /// 친구에게는 절대 안 감). (`term`, 설치 id)가 가장 높은 쪽이 놀고 다른 쪽은 대기(친구 링크 없음).
    /// `you` = 그 term을 받는 쪽이 쥠, 아니면 보낸 쪽. 차지는 보낸 쪽 term + 1임. `idle`(보낸 쪽 사용자가 마지막으로
    /// 쓴 뒤의 초)은 증명된 링크마다 첫 메시지에만 있고, 그러면 마지막에 쓴 컴퓨터가 차지함
    DeviceActive {
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        term: Option<u64>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        you: bool,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        idle: Option<u64>,
        /// 이 차지는 다시 만나 누가 놀지(마지막에 쓴 쪽)를 정함: 받는 쪽은 아무것도 넘기지 않음 (`device_state` 참고)
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        meet: bool,
    },
    /// 📱 노는 컴퓨터가 내 다른 컴퓨터에 보내는 고른 펫과 모든 펫의 돌봄임 (`device_active`와 같은 규칙).
    /// 대기 쪽은 노는 컴퓨터에서만 받음. `handover` = 받는 쪽이 차지해서 보낸 쪽이 방금 놀기를 멈춤:
    /// 그 마지막 상태이며, 받는 쪽이 그사이 자기 것을 바꿨으면 안 받음
    DeviceState {
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]
        pet: Option<String>,
        #[serde(default, deserialize_with = "lenient_list")]
        care: Vec<WireCare>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not", deserialize_with = "lenient_bool")]
        handover: bool,
    },
}

/// 잠긴 사다리 게임의 참가자 한 명임 (호스트가 전달함). `token`은 게임마다의 난수 id임
/// (설치 id가 아님: 참가자끼리 친구가 아닐 수 있음)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WirePlayer {
    /// 게임별 참가자 토큰
    pub token: String,
    /// 표시 이름
    pub name: String,
    /// 펫 종류
    pub pet: String,
    /// 펫 그림 스타일
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// 참가자 nonce 커밋 (hex)
    pub commit: String,
}

/// 사다리 아래 결과 칸 하나임. `mood`: "good" | "bad" | "none"
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireResult {
    /// 결과 글
    pub text: String,
    /// 분위기
    #[serde(default)]
    pub mood: String,
}

/// 호스트와 참가자 사이의 사다리 게임 메시지임 (친구 링크에서만, `LADDER_MIN_VERSION` 이상 상대에게만).
/// 길이·hex 검사는 여기가 아니라 `ladder`의 상태 기계가 함
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LadderMsg {
    /// 호스트가 게임에 초대함
    LadderInvite { game_id: String, host: String, results: Vec<WireResult>, invited: u8 },
    /// 참가자가 초대에 응함 (자기 nonce 커밋을 실음)
    LadderJoin {
        game_id: String,
        token: String,
        commit: String,
        name: String,
        pet: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<String>,
    },
    /// 초대를 거절함. `reason`: "no" | "busy" | "playing"
    LadderDecline { game_id: String, reason: String },
    /// 참가자를 잠금. `host` = `players`(토큰 순 정렬) 안의 호스트 자리
    LadderLock { game_id: String, host: u8, players: Vec<WirePlayer>, results: Vec<WireResult> },
    /// 참가자가 자기 nonce를 밝힘
    LadderReveal { game_id: String, nonce: String },
    /// 모든 참가자의 nonce, `players` 순서 (호스트 것 포함)
    LadderReveals { game_id: String, nonces: Vec<String> },
    /// 지금 내려가기를 보임. `mode`: "one_by_one" | "all". 보이기만 다름
    LadderStart { game_id: String, mode: String },
    /// 게임 취소. `reason`: "host" | "left" | "timeout" | "invalid", `who` = 참가자 토큰
    LadderCancel {
        game_id: String,
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        who: Option<String>,
    },
}

/// 모든 메시지임. 이 단계는 꼬리표가 없음: 안쪽 enum마다 `type` 꼬리표가 있어 모르는 `type`은 어느 것으로도 안 읽힘
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Control(Control),
    Visit(Visit),
    Ladder(LadderMsg),
}

impl Message {
    /// JSON 바이트로 바꿈
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("message serializes")
    }

    /// 바이트를 메시지로 읽음. 잘못됐거나 모르는 `type`이면 None임
    pub fn parse(data: &[u8]) -> Option<Message> {
        serde_json::from_slice(data).ok()
    }
}

/// 상대가 준 표시 문자열을 `MAX_NAME`글자로 자름
pub fn clamp_name(s: &str) -> String {
    s.chars().take(MAX_NAME).collect()
}

/// UDP 찾기 알림임 (인증 없는 힌트)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announce {
    /// 앱 꼬리표 (`APP_TAG`)
    pub app: String,
    /// 프로토콜 버전
    pub v: u32,
    /// 설치 id
    pub id: String,
    /// 표시 이름
    pub name: String,
    /// 링크를 받는 TCP 포트
    pub port: u16,
}

impl Announce {
    /// 이 설치의 알림을 만듦
    pub fn new(id: &str, name: &str, port: u16) -> Announce {
        Announce {
            app: APP_TAG.to_string(),
            v: PROTOCOL_VERSION,
            id: id.to_string(),
            name: name.to_string(),
            port,
        }
    }

    /// 데이터그램을 읽음. 올바른 ScreenPet 알림이 아니면 None임
    pub fn parse(data: &[u8]) -> Option<Announce> {
        let mut a: Announce = serde_json::from_slice(data).ok()?;
        if a.app != APP_TAG || a.id.is_empty() || a.id.len() > MAX_ID || a.port == 0 {
            return None;
        }
        a.name = clamp_name(&a.name);
        Some(a)
    }
}

/// 와이어 형식과 옛 버전 호환 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 보통 `Visit::Visit` 메시지에서 이 필드들만 바꾼 것을 만듦
    macro_rules! visit {
        ($($field:ident: $value:expr),* $(,)?) => {{
            #[allow(unused_mut)]
            let mut v = Visit::Visit {
                visit_id: 8,
                species: "cat".into(),
                stage: "child".into(),
                scale: 1.0,
                personality: "friendly".into(),
                owner: "A".into(),
                enter_side: Side::Left,
                vside: None,
                via: None,
                homesick_in: None,
                fling: None,
                note: None,
                gift: None,
                style: None,
                tint: None,
                boost: None,
                no_grow: None,
                of: None,
                carry: None,
            };
            #[allow(unused_variables)]
            if let Visit::Visit { $($field,)* .. } = &mut v {
                $(*$field = $value;)*
            }
            Message::Visit(v)
        }};
    }

    #[test]
    fn visit_wire_format_uses_type_tag() {
        let m = visit!(visit_id: 7, scale: 1.5, personality: "playful".into(), owner: "Ann".into());
        let v: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(v["type"], "visit");
        assert_eq!(v["enter_side"], "left");
        assert_eq!(Message::parse(&m.to_json()), Some(m));
    }

    #[test]
    fn up_down_edges_ride_next_to_a_side_old_versions_understand() {
        let up = visit!(visit_id: 7, species: "frog".into(), personality: "playful".into(), owner: "Ann".into(), vside: Some(VSide::Down));
        let v: serde_json::Value = serde_json::from_slice(&up.to_json()).unwrap();
        assert_eq!((v["enter_side"].as_str(), v["vside"].as_str()), (Some("left"), Some("down")));
        assert_eq!(Message::parse(&up.to_json()), Some(up.clone()));
        // 예전 판은 vside를 모르고 왼쪽에서 들어오는 보통 방문으로 읽음
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, enter_side: Side },
            Leave { visit_id: u64, side: Option<Side> },
        }
        let OldVisit::Visit { visit_id, enter_side } = serde_json::from_slice::<OldVisit>(&up.to_json()).unwrap() else { panic!() };
        assert_eq!((visit_id, enter_side), (7, Side::Left));
        let leave = Message::Visit(Visit::Leave { visit_id: 9, next: Some("c".into()), side: Some(Side::Right), vside: Some(VSide::Up), homesick_in: None, fling: None, note: None, boost: None, carry: None });
        let j: serde_json::Value = serde_json::from_slice(&leave.to_json()).unwrap();
        assert_eq!((j["side"].as_str(), j["vside"].as_str()), (Some("right"), Some("up")));
        assert_eq!(Message::parse(&leave.to_json()), Some(leave.clone()));
        let OldVisit::Leave { visit_id, side } = serde_json::from_slice::<OldVisit>(&leave.to_json()).unwrap() else { panic!() };
        assert_eq!((visit_id, side), (9, Some(Side::Right)));
        // vside가 없는 예전 메시지는 그대로 (위/아래 아님)
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"right"}"#;
        let Some(Message::Visit(Visit::Visit { enter_side, vside, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!((enter_side, vside), (Side::Right, None));
    }

    #[test]
    fn unknown_edge_values_never_drop_the_message() {
        // 모르는 vside는 버리고 side를 씀, 모르는 enter_side는 왼쪽, 모르는 leave side는 없음
        let odd = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"diagonal","vside":"sideways"}"#;
        let Some(Message::Visit(Visit::Visit { enter_side, vside, .. })) = Message::parse(odd) else { panic!("odd visit must parse") };
        assert_eq!((enter_side, vside), (Side::Left, None));
        let up_side = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"up","vside":7}"#;
        assert!(matches!(Message::parse(up_side), Some(Message::Visit(Visit::Visit { enter_side: Side::Left, vside: None, .. }))));
        let leave = br#"{"type":"leave","visit_id":3,"next":"x","side":"up","vside":"north"}"#;
        assert_eq!(
            Message::parse(leave),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: Some("x".into()), side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None, carry: None }))
        );
        let leave = br#"{"type":"leave","visit_id":3,"side":"left","vside":"down"}"#;
        assert!(matches!(Message::parse(leave), Some(Message::Visit(Visit::Leave { side: Some(Side::Left), vside: Some(VSide::Down), .. }))));
    }

    #[test]
    fn control_messages_round_trip() {
        for m in [
            Message::Control(Control::Hello { id: "a".into(), name: "A".into(), v: 1, friend: true }),
            Message::Control(Control::Invite { pet: "cat".into(), commit: "ab".into() }),
            Message::Control(Control::PairRandom { random: "cd".into() }),
            Message::Control(Control::PairReveal { nonce: "ef".into() }),
            Message::Control(Control::InviteReply { accepted: true }),
            Message::Control(Control::Unfriend),
            Message::Control(Control::Ping { ask: None, steer: None, toss: None }),
            Message::Visit(Visit::leave(3)),
            Message::Visit(Visit::ComeHome { visit_id: 4 }),
        ] {
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })));
    }

    #[test]
    fn ladder_messages_round_trip() {
        let player = WirePlayer { token: "t1".into(), name: "나비".into(), pet: "cat".into(), style: Some("pixel".into()), commit: "c1".into() };
        let result = WireResult { text: "당첨".into(), mood: "good".into() };
        for m in [
            LadderMsg::LadderInvite { game_id: "g".into(), host: "Ann".into(), results: vec![result.clone()], invited: 3 },
            LadderMsg::LadderJoin { game_id: "g".into(), token: "t".into(), commit: "c".into(), name: "B".into(), pet: "dog".into(), style: None },
            LadderMsg::LadderDecline { game_id: "g".into(), reason: "busy".into() },
            LadderMsg::LadderLock { game_id: "g".into(), host: 0, players: vec![player], results: vec![result] },
            LadderMsg::LadderReveal { game_id: "g".into(), nonce: "n".into() },
            LadderMsg::LadderReveals { game_id: "g".into(), nonces: vec!["n1".into(), "n2".into()] },
            LadderMsg::LadderStart { game_id: "g".into(), mode: "all".into() },
            LadderMsg::LadderCancel { game_id: "g".into(), reason: "left".into(), who: Some("t".into()) },
        ] {
            let m = Message::Ladder(m);
            assert_eq!(Message::parse(&m.to_json()), Some(m));
        }
        let v: serde_json::Value =
            serde_json::from_slice(&Message::Ladder(LadderMsg::LadderReveal { game_id: "g".into(), nonce: "n".into() }).to_json()).unwrap();
        assert_eq!(v["type"], "ladder_reveal");
        // 모르는 type은 여전히 어느 쪽으로도 읽히지 않음 (예전 판이 연결을 끊는 이유)
        assert_eq!(Message::parse(br#"{"type":"ladder_dance","game_id":"g"}"#), None);
    }

    #[test]
    fn hello_from_either_version_parses() {
        for v in [1, 2, 99] {
            let raw = format!(r#"{{"type":"hello","id":"a","name":"A","v":{v}}}"#);
            assert!(matches!(Message::parse(raw.as_bytes()), Some(Message::Control(Control::Hello { v: got, .. })) if got == v));
        }
    }

    #[test]
    fn leave_can_name_the_next_friend_and_stays_backward_compatible() {
        let hop = Message::Visit(Visit::Leave { visit_id: 9, next: Some("xx".into()), side: Some(Side::Left), vside: None, homesick_in: Some(42.5), fling: None, note: None, boost: None, carry: None });
        let v: serde_json::Value = serde_json::from_slice(&hop.to_json()).unwrap();
        assert_eq!((v["next"].as_str(), v["side"].as_str(), v["homesick_in"].as_f64()), (Some("xx"), Some("left"), Some(42.5)));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 예전 판의 leave (next 없음) = 집으로
        assert_eq!(
            Message::parse(br#"{"type":"leave","visit_id":3}"#),
            Some(Message::Visit(Visit::Leave { visit_id: 3, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None, carry: None }))
        );
        // 집으로 가는 leave는 예전 판과 같은 모양
        assert_eq!(Message::Visit(Visit::leave(3)).to_json(), br#"{"type":"leave","visit_id":3}"#.to_vec());
    }

    #[test]
    fn visit_can_carry_the_previous_host_and_homesick_time() {
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { via, homesick_in, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!((via, homesick_in), (None, None));
        let hop = visit!(visit_id: 2, enter_side: Side::Right, via: Some("bb".into()), homesick_in: Some(10.0));
        assert_eq!(Message::parse(&hop.to_json()), Some(hop));
        // 새 필드를 모르는 받는 쪽도 읽을 수 있음 (모르는 필드는 무시)
        assert!(Message::parse(br#"{"type":"leave","visit_id":3,"next":"x","future":1}"#).is_some());
    }

    #[test]
    fn visit_and_leave_can_carry_a_throw_and_old_peers_still_parse() {
        let f = Fling { height: 0.25, vx: -640.0, vy: -120.5 };
        let v = visit!(visit_id: 5, enter_side: Side::Right, fling: Some(f));
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!((j["fling"]["height"].as_f64(), j["fling"]["vx"].as_f64(), j["fling"]["vy"].as_f64()), (Some(0.25), Some(-640.0), Some(-120.5)));
        assert_eq!(Message::parse(&v.to_json()), Some(v));
        let back = Message::Visit(Visit::Leave { visit_id: 5, next: None, side: Some(Side::Left), vside: None, homesick_in: None, fling: Some(f), note: None, boost: None, carry: None });
        assert_eq!(Message::parse(&back.to_json()), Some(back));
        // 던지기를 모르는 예전 판: 필드가 없으면 None (평소처럼 걸어 들어옴)
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { fling, .. })) = Message::parse(old) else { panic!() };
        assert_eq!(fling, None);
        // 던지지 않은 방문·떠남은 예전 판과 같은 모양 (fling 필드 없음)
        assert!(!String::from_utf8(Message::Visit(Visit::leave(3)).to_json()).unwrap().contains("fling"));
        // 깨진 fling 하나 때문에 방문 전체를 버리지 않음
        let bad = br#"{"type":"leave","visit_id":3,"fling":{"height":"x"}}"#;
        assert_eq!(Message::parse(bad), Some(Message::Visit(Visit::leave(3))));
    }

    #[test]
    fn a_visit_can_carry_a_gift_and_old_peers_ignore_it() {
        let v = visit!(gift: Some("cat_yarn_shiny".into()));
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!(j["gift"], "cat_yarn_shiny");
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        // 예전 판은 모르는 필드를 건너뛰고 방문으로 받음
        #[derive(serde::Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, owner: String },
        }
        let OldVisit::Visit { visit_id, owner } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!((visit_id, owner.as_str()), (8, "A"));
        // 선물 없는 방문은 예전과 같은 모양, 예전 판의 방문은 선물 없음
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("gift"));
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { gift, .. })) = Message::parse(old) else { panic!() };
        assert_eq!(gift, None);
        // 글자가 아닌 선물 하나 때문에 방문 전체를 버리지 않음
        let bad = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","gift":{"x":1}}"#;
        let Some(Message::Visit(Visit::Visit { gift, .. })) = Message::parse(bad) else { panic!("bad gift must not drop the visit") };
        assert_eq!(gift, None);
    }

    #[test]
    fn a_visit_can_carry_an_object_and_old_peers_ignore_it() {
        let rainbow = Carry { kind: "poop".into(), look: Some("rainbow".into()), scale: Some(0.8) };
        let v = visit!(carry: Some(rainbow.clone()));
        let j: serde_json::Value = serde_json::from_slice(&v.to_json()).unwrap();
        assert_eq!(j["carry"], serde_json::json!({"kind": "poop", "look": "rainbow", "scale": 0.8}));
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        // 예전 판은 모르는 필드를 건너뛰고 방문으로 받음
        #[derive(serde::Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, owner: String },
        }
        let OldVisit::Visit { visit_id, owner } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!((visit_id, owner.as_str()), (8, "A"));
        // 들고 오지 않은 방문은 예전과 같은 모양, 예전 판의 방문은 들고 온 것 없음
        assert!(!String::from_utf8(visit!().to_json()).unwrap().contains("carry"));
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { carry, .. })) = Message::parse(old) else { panic!() };
        assert_eq!(carry, None);
        // 이상한 값 하나 때문에 방문 전체를 버리지 않음 (그 값만 버림)
        for bad in [r#""carry":"can""#, r#""carry":{"kind":7}"#, r#""carry":[1]"#, r#""carry":{"look":"pink"}"#] {
            let msg = format!(r#"{{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left",{bad}}}"#);
            let Some(Message::Visit(Visit::Visit { carry, visit_id, .. })) = Message::parse(msg.as_bytes()) else { panic!("{bad} must not drop the visit") };
            assert_eq!((carry, visit_id), (None, 1), "{bad}");
        }
        // 이상한 색·크기는 그 칸만 버림
        let odd = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","carry":{"kind":"can","look":5,"scale":"big"}}"#;
        let Some(Message::Visit(Visit::Visit { carry, .. })) = Message::parse(odd) else { panic!() };
        assert_eq!(carry, Some(Carry { kind: "can".into(), look: None, scale: None }));
        // 못 들어가 돌려보내는 Leave에 실려 돌아옴 (예전 판의 Leave에는 없음)
        let back = Message::Visit(Visit::Leave { visit_id: 3, next: None, side: None, vside: None, homesick_in: None, fling: None, note: None, boost: None, carry: Some(carry_can()) });
        assert_eq!(Message::parse(&back.to_json()), Some(back.clone()));
        assert_eq!(Message::parse(br#"{"type":"leave","visit_id":3,"carry":true}"#), Some(Message::Visit(Visit::leave(3))));
        assert_eq!(Message::Visit(Visit::leave(3)).to_json(), br#"{"type":"leave","visit_id":3}"#.to_vec());
        assert!(CARRY_MIN_VERSION > DEVICES_MIN_VERSION && CARRY_MIN_VERSION <= PROTOCOL_VERSION);
    }

    fn carry_can() -> Carry {
        Carry { kind: "can".into(), look: None, scale: None }
    }

    fn visit_with_note(text: Option<&str>) -> Message {
        visit!(note: text.map(str::to_string))
    }

    #[test]
    fn visit_can_carry_a_note_and_old_peers_still_parse() {
        let m = visit_with_note(Some("안녕 🐱 놀러 왔어!"));
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(j["note"].as_str(), Some("안녕 🐱 놀러 왔어!"));
        assert_eq!(Message::parse(&m.to_json()), Some(m));
        // 편지 없는 방문은 예전 판과 같은 모양 (note 필드 없음)
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("note"));
        // 편지를 모르는 예전 판이 보낸 방문: note 없음
        let old = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left"}"#;
        assert_eq!(Message::parse(old), Some(visit_with_note(None)));
        // 예전 판(note를 모르는 Visit)도 편지가 실린 방문을 그냥 방문으로 읽음
        #[derive(Debug, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64, owner: String },
        }
        let OldVisit::Visit { visit_id, owner } = serde_json::from_slice::<OldVisit>(&visit_with_note(Some("hi")).to_json()).unwrap();
        assert_eq!((visit_id, owner.as_str()), (8, "A"));
        // 글자가 아닌 note 하나 때문에 방문 전체를 버리지 않음
        let bad = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","note":{"x":1}}"#;
        assert_eq!(Message::parse(bad), Some(visit_with_note(None)));
        let num = br#"{"type":"visit","visit_id":8,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left","note":5}"#;
        assert_eq!(Message::parse(num), Some(visit_with_note(None)));
    }

    #[test]
    fn leave_can_carry_a_reply_and_old_owners_still_parse() {
        let reply = |note: Option<&str>| Message::Visit(Visit::Leave { visit_id: 4, next: None, side: None, vside: None, homesick_in: None, fling: None, note: note.map(str::to_string), boost: None, carry: None });
        let m = reply(Some("잘 놀다 가 🐶"));
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!(j["note"].as_str(), Some("잘 놀다 가 🐶"));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // 답장 없는 귀환은 예전 판과 같은 모양
        assert_eq!(reply(None).to_json(), br#"{"type":"leave","visit_id":4}"#.to_vec());
        // 답장을 모르는 예전 주인도 그냥 귀환으로 읽음
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Leave { visit_id: u64, next: Option<String> },
        }
        let OldVisit::Leave { visit_id, next } = serde_json::from_slice::<OldVisit>(&m.to_json()).unwrap();
        assert_eq!((visit_id, next), (4, None));
        // 글자가 아닌 note 하나 때문에 귀환을 버리지 않음
        assert_eq!(Message::parse(br#"{"type":"leave","visit_id":4,"note":[1,2]}"#), Some(reply(None)));
        assert_eq!(Message::parse(br#"{"type":"leave","visit_id":4,"note":null}"#), Some(reply(None)));
    }

    #[test]
    fn ping_can_carry_an_ask_that_old_peers_ignore() {
        let ask = Message::Control(Control::Ping { ask: Some("invite".into()), steer: None, toss: None });
        assert_eq!(ask.to_json(), br#"{"type":"ping","ask":"invite"}"#.to_vec());
        assert_eq!(Message::parse(&ask.to_json()), Some(ask));
        // 평소 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        assert_eq!(Message::parse(br#"{"type":"ping"}"#), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })));
        // 예전 판의 Control(Ping은 필드 없는 변형)도 ask가 붙은 ping을 그냥 ping으로 읽음 (연결을 끊지 않음)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Unfriend,
            Ping,
        }
        assert_eq!(serde_json::from_slice::<OldControl>(br#"{"type":"ping","ask":"invite"}"#).unwrap(), OldControl::Ping);
    }

    #[test]
    fn ping_can_carry_a_steer_that_old_peers_ignore() {
        let st = Steer { visit_id: 42, on: true, keys: 0b10_0010 };
        let m = Message::Control(Control::Ping { ask: None, steer: Some(st), toss: None });
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!((j["type"].as_str(), j["steer"]["visit_id"].as_u64(), j["steer"]["on"].as_bool(), j["steer"]["keys"].as_u64()), (Some("ping"), Some(42), Some(true), Some(34)));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // ask와 함께 실려도 됨
        let both = Message::Control(Control::Ping { ask: Some("invite".into()), steer: Some(st), toss: None });
        assert_eq!(Message::parse(&both.to_json()), Some(both));
        // 조작 없는 ping은 예전 판과 같은 모양
        assert!(!String::from_utf8(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json()).unwrap().contains("steer"));
        // 예전 판의 Control도 steer가 붙은 ping을 그냥 ping으로 읽음 (연결을 끊지 않음)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Ping {
                #[serde(default)]
                ask: Option<String>,
            },
        }
        assert_eq!(serde_json::from_slice::<OldControl>(&m.to_json()).unwrap(), OldControl::Ping { ask: None });
        // 깨진 steer 하나 때문에 ping을 버리지 않음 (steer만 버림)
        for bad in [
            &br#"{"type":"ping","steer":{"visit_id":"x"}}"#[..],
            br#"{"type":"ping","steer":{"visit_id":1,"keys":300}}"#,
            br#"{"type":"ping","steer":5}"#,
        ] {
            assert_eq!(Message::parse(bad), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })), "{}", String::from_utf8_lossy(bad));
        }
        // on·keys가 없으면 꺼짐·안 눌림
        assert_eq!(
            Message::parse(br#"{"type":"ping","steer":{"visit_id":3}}"#),
            Some(Message::Control(Control::Ping { ask: None, steer: Some(Steer { visit_id: 3, on: false, keys: 0 }), toss: None }))
        );
    }

    #[test]
    fn ping_can_carry_a_toss_that_old_peers_ignore() {
        let t = Toss { kind: "can".into(), side: Side::Right, vside: Some(VSide::Down), fling: Some(Fling { height: 0.5, vx: 300.0, vy: -20.0 }), note: Some("붙였어!".into()), bounced: false, scale: None, look: None, rally: None };
        let m = Message::Control(Control::Ping { ask: None, steer: None, toss: Some(t) });
        let j: serde_json::Value = serde_json::from_slice(&m.to_json()).unwrap();
        assert_eq!((j["type"].as_str(), j["toss"]["kind"].as_str(), j["toss"]["side"].as_str(), j["toss"]["vside"].as_str(), j["toss"]["note"].as_str()), (Some("ping"), Some("can"), Some("right"), Some("down"), Some("붙였어!")));
        assert_eq!(Message::parse(&m.to_json()), Some(m.clone()));
        // 던지는 것 없는 ping은 예전 판과 같은 모양
        assert_eq!(Message::Control(Control::Ping { ask: None, steer: None, toss: None }).to_json(), br#"{"type":"ping"}"#.to_vec());
        // 예전 판의 Control도 toss가 붙은 ping을 그냥 ping으로 읽음 (연결을 끊지 않음)
        #[derive(Debug, PartialEq, Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldControl {
            Ping {
                #[serde(default)]
                ask: Option<String>,
            },
        }
        assert_eq!(serde_json::from_slice::<OldControl>(&m.to_json()).unwrap(), OldControl::Ping { ask: None });
        // 깨진 toss 하나 때문에 ping을 버리지 않음
        for bad in [&br#"{"type":"ping","toss":5}"#[..], br#"{"type":"ping","toss":{"side":"left"}}"#, br#"{"type":"ping","toss":{"kind":7}}"#] {
            assert_eq!(Message::parse(bad), Some(Message::Control(Control::Ping { ask: None, steer: None, toss: None })), "{}", String::from_utf8_lossy(bad));
        }
        // 안쪽 필드가 이상해도 물건은 살림: 모르는 끝은 왼쪽, 깨진 fling·note는 없음
        let odd = br#"{"type":"ping","toss":{"kind":"can","side":"north","vside":3,"fling":{"height":"x"},"note":[1]}}"#;
        assert_eq!(
            Message::parse(odd),
            Some(Message::Control(Control::Ping { ask: None, steer: None, toss: Some(Toss { kind: "can".into(), side: Side::Left, vside: None, fling: None, note: None, bounced: false, scale: None, look: None, rally: None }) }))
        );
        // side가 없어도 왼쪽
        // 똥 색 표시: 없으면 None, 있으면 그대로 (옛 버전은 모르는 필드를 무시함). 글자가 아니면 버림
        let pink = Message::parse(br#"{"type":"ping","toss":{"kind":"poop","look":"pink"}}"#);
        assert!(matches!(&pink, Some(Message::Control(Control::Ping { toss: Some(Toss { look: Some(l), .. }), .. })) if l == "pink"));
        let odd = Message::parse(br#"{"type":"ping","toss":{"kind":"poop","look":7}}"#);
        assert!(matches!(odd, Some(Message::Control(Control::Ping { toss: Some(Toss { look: None, .. }), .. }))));
        assert!(matches!(Message::parse(br#"{"type":"ping","toss":{"kind":"can"}}"#), Some(Message::Control(Control::Ping { toss: Some(Toss { side: Side::Left, .. }), .. }))));
    }

    #[test]
    fn a_giant_toss_reads_as_a_plain_stone_to_anyone_who_skips_the_look() {
        let t = Toss { kind: "stone".into(), side: Side::Right, vside: None, fling: None, note: None, bounced: false, scale: None, look: Some("giant".into()), rally: None };
        let json = serde_json::to_string(&t).unwrap();
        // 크기는 받는 쪽이 제 화면에 맞춤: scale은 싣지 않음
        assert!(json.contains(r#""look":"giant""#) && json.contains(r#""kind":"stone""#) && !json.contains("scale"), "{json}");
        let msg = Message::parse(format!(r#"{{"type":"ping","toss":{json}}}"#).as_bytes());
        assert!(matches!(&msg, Some(Message::Control(Control::Ping { toss: Some(Toss { kind, look: Some(l), scale: None, .. }), .. })) if kind == "stone" && l == "giant"));
        // look을 모르는 옛 모양의 메시지도 그대로 읽힘
        assert!(matches!(Message::parse(br#"{"type":"ping","toss":{"kind":"stone"}}"#), Some(Message::Control(Control::Ping { toss: Some(Toss { look: None, .. }), .. }))));
    }

    #[test]
    fn a_toss_carries_the_catch_ball_rally_that_old_peers_skip() {
        let t = Toss { kind: "ball".into(), side: Side::Left, vside: None, fling: None, note: None, bounced: false, scale: None, look: None, rally: Some(7) };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains(r#""rally":7"#), "{json}");
        assert_eq!(serde_json::from_str::<Toss>(&json).unwrap(), t);
        // 랠리가 없으면 싣지 않음 (예전 판과 같은 모양)
        let plain = Toss { rally: None, ..t.clone() };
        assert!(!serde_json::to_string(&plain).unwrap().contains("rally"));
        // 예전 판의 메시지(랠리 없음)는 그대로 읽힘
        assert!(matches!(Message::parse(br#"{"type":"ping","toss":{"kind":"ball"}}"#), Some(Message::Control(Control::Ping { toss: Some(Toss { rally: None, .. }), .. }))));
        // 예전 판의 Toss는 rally를 모르고 그냥 넘김
        #[derive(Debug, Deserialize)]
        struct OldToss {
            kind: String,
            #[serde(default)]
            note: Option<String>,
        }
        let old: OldToss = serde_json::from_str(&json).unwrap();
        assert_eq!((old.kind.as_str(), old.note), ("ball", None));
    }

    #[test]
    fn a_received_rally_is_clamped_and_junk_is_dropped() {
        let rally = |raw: &str| serde_json::from_str::<Toss>(&format!(r#"{{"kind":"ball","rally":{raw}}}"#)).unwrap().rally;
        assert_eq!(rally("0"), Some(0));
        assert_eq!(rally("20"), Some(MAX_RALLY));
        assert_eq!(rally("21"), Some(MAX_RALLY));
        assert_eq!(rally("99999999999"), Some(MAX_RALLY));
        assert_eq!(MAX_RALLY, 20);
        for junk in ["-1", "3.5", "\"x\"", "null", "[1]"] {
            assert_eq!(rally(junk), None, "{junk}");
        }
    }

    #[test]
    fn friend_cap() {
        assert_eq!(MAX_FRIENDS, 50);
    }

    #[test]
    fn unknown_or_malformed_messages_are_rejected() {
        assert_eq!(Message::parse(br#"{"type":"bogus"}"#), None);
        assert_eq!(Message::parse(br#"{"no_type":1}"#), None);
        assert_eq!(Message::parse(br#"{"type":"leave"}"#), None);
        assert_eq!(Message::parse(b"not json"), None);
    }

    #[test]
    fn announce_parsing_and_clamping() {
        let a = Announce::new("abc", "Ann", 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&a).unwrap()), Some(a));
        assert_eq!(Announce::parse(br#"{"app":"other","v":1,"id":"x","name":"n","port":1}"#), None);
        assert_eq!(Announce::parse(b"garbage"), None);
        let long = Announce::new("abc", &"n".repeat(500), 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&long).unwrap()).unwrap().name.chars().count(), MAX_NAME);
        let long_id = Announce::new(&"i".repeat(MAX_ID + 1), "n", 4000);
        assert_eq!(Announce::parse(&serde_json::to_vec(&long_id).unwrap()), None);
    }

    /// 픽셀 스타일 펫은 그렇다고 알림. 옛 상대는 무시하고, 이상한 값은 방문 전체가 아니라 그 필드만 버림
    #[test]
    fn visit_style_is_optional_and_backward_compatible() {
        let v = visit!(style: Some("pixel".into()));
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        // an old peer still reads it
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64 },
        }
        let OldVisit::Visit { visit_id } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!(visit_id, 8);
        // round sends nothing at all
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("style"));
        let old = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"baby","scale":1.0,"personality":"aloof","owner":"a","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { style, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!(style, None);
        let bad = br#"{"type":"visit","visit_id":1,"species":"cat","stage":"baby","scale":1.0,"personality":"aloof","owner":"a","enter_side":"left","style":7}"#;
        let Some(Message::Visit(Visit::Visit { style, .. })) = Message::parse(bad) else { panic!("bad style must not drop the visit") };
        assert_eq!(style, None);
    }

    /// 🎨 잉크선 펫의 주인 몸 색은 id로 감. 옛 상대는 무시하고 기본값은 안 보내며, 이상한 값은 그 필드만 버림
    #[test]
    fn visit_tint_is_optional_and_backward_compatible() {
        let v = visit!(tint: Some("rainbow".into()));
        assert_eq!(Message::parse(&v.to_json()), Some(v.clone()));
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case")]
        enum OldVisit {
            Visit { visit_id: u64 },
        }
        let OldVisit::Visit { visit_id } = serde_json::from_slice::<OldVisit>(&v.to_json()).unwrap();
        assert_eq!(visit_id, 8);
        assert!(!String::from_utf8(visit_with_note(None).to_json()).unwrap().contains("tint"));
        let old = br#"{"type":"visit","visit_id":1,"species":"heumul","stage":"baby","scale":1.0,"personality":"playful","owner":"a","enter_side":"left"}"#;
        let Some(Message::Visit(Visit::Visit { tint, .. })) = Message::parse(old) else { panic!("old visit must parse") };
        assert_eq!(tint, None);
        let bad = br#"{"type":"visit","visit_id":1,"species":"heumul","stage":"baby","scale":1.0,"personality":"playful","owner":"a","enter_side":"left","tint":[1]}"#;
        let Some(Message::Visit(Visit::Visit { tint, .. })) = Message::parse(bad) else { panic!("bad tint must not drop the visit") };
        assert_eq!(tint, None);
    }

    #[test]
    fn clamp_counts_characters() {
        assert_eq!(clamp_name(&"🐱".repeat(100)).chars().count(), MAX_NAME);
        assert_eq!(clamp_name("Ann"), "Ann");
    }

    #[test]
    fn boost_is_optional_and_a_bad_one_does_not_lose_the_message() {
        let leave = Message::Visit(Visit::leave(7));
        assert!(!String::from_utf8(leave.to_json()).unwrap().contains("boost"));
        let Some(Message::Visit(Visit::Leave { boost, .. })) = Message::parse(br#"{"type":"leave","visit_id":7,"boost":1.5}"#) else { panic!() };
        assert_eq!(boost, Some(1.5));
        let Some(Message::Visit(Visit::Leave { boost, visit_id, .. })) = Message::parse(br#"{"type":"leave","visit_id":7,"boost":"huge"}"#) else { panic!() };
        assert_eq!((boost, visit_id), (None, 7));
    }

    #[test]
    fn no_grow_is_optional_and_old_visits_still_parse() {
        let base = r#"{"type":"visit","visit_id":7,"species":"cat","stage":"child","scale":1.0,"personality":"friendly","owner":"A","enter_side":"left""#;
        let Some(Message::Visit(Visit::Visit { no_grow, .. })) = Message::parse(format!("{base}}}").as_bytes()) else { panic!() };
        assert_eq!(no_grow, None);
        let Some(Message::Visit(Visit::Visit { no_grow, .. })) = Message::parse(format!(r#"{base},"no_grow":true}}"#).as_bytes()) else { panic!() };
        assert_eq!(no_grow, Some(true));
        let Some(Message::Visit(Visit::Visit { no_grow, visit_id, .. })) = Message::parse(format!(r#"{base},"no_grow":"yes"}}"#).as_bytes()) else { panic!() };
        assert_eq!((no_grow, visit_id), (None, 7));
    }

    #[test]
    fn device_messages_skip_bad_entries_instead_of_failing() {
        let k = "ab".repeat(32);
        let raw = format!(r#"{{"type":"device_friends","friends":[{{"id":"x","key":"{k}","name":5,"side":7}},3,{{"id":"y"}},{{"id":"z","key":"{k}","name":"Z","side":"up"}}]}}"#);
        let Some(Message::Control(Control::DeviceFriends { friends, want })) = Message::parse(raw.as_bytes()) else { panic!() };
        assert!(!want);
        assert_eq!(friends.len(), 2);
        assert_eq!((friends[0].name.as_str(), friends[0].side.as_deref()), ("", None));
        assert_eq!((friends[1].name.as_str(), friends[1].side.as_deref()), ("Z", Some("up")));
        // 배열이 아니면 빈 목록, 없어도 빈 목록
        let Some(Message::Control(Control::MyDevices { devices })) = Message::parse(br#"{"type":"my_devices","devices":"lots"}"#) else { panic!() };
        assert!(devices.is_empty());
        let Some(Message::Control(Control::DeviceFriends { friends, .. })) = Message::parse(br#"{"type":"device_friends"}"#) else { panic!() };
        assert!(friends.is_empty());
        // 보내는 모양
        let m = Message::Control(Control::MyDevices { devices: vec![WireDevice { id: "d".into(), key: k.clone() }] });
        assert_eq!(String::from_utf8(m.to_json()).unwrap(), format!(r#"{{"type":"my_devices","devices":[{{"id":"d","key":"{k}"}}]}}"#));
        let m = Message::Control(Control::DeviceFriends { friends: vec![], want: false });
        assert_eq!(String::from_utf8(m.to_json()).unwrap(), r#"{"type":"device_friends","friends":[]}"#);
    }

    #[test]
    fn a_visit_may_say_it_is_my_other_computers_pet() {
        let v = visit!(of: Some("b2".into()));
        assert!(String::from_utf8(v.to_json()).unwrap().contains(r#""of":"b2""#));
        assert!(!String::from_utf8(visit!().to_json()).unwrap().contains(r#""of""#), "absent unless carried");
        let Some(Message::Visit(Visit::Visit { of, visit_id, .. })) = Message::parse(br#"{"type":"visit","visit_id":3,"species":"cat","stage":"child","scale":1,"personality":"p","owner":"A","enter_side":"left","of":7}"#) else { panic!() };
        assert_eq!((of, visit_id), (None, 3), "a bad value is dropped, not the visit");
    }

    #[test]
    fn device_link_messages_have_their_wire_shape_and_bad_fields_read_as_nothing() {
        let shape = |m: Control| String::from_utf8(Message::Control(m).to_json()).unwrap();
        assert_eq!(shape(Control::DeviceCode { nonce: "ab".into() }), r#"{"type":"device_code","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeNonce { nonce: "ab".into() }), r#"{"type":"device_code_nonce","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeProof { proof: "ab".into() }), r#"{"type":"device_code_proof","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceCodeOk { proof: "ab".into() }), r#"{"type":"device_code_ok","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceNo { reason: "wrong".into(), left: Some(3) }), r#"{"type":"device_no","reason":"wrong","left":3}"#);
        assert_eq!(shape(Control::DeviceNo { reason: "none".into(), left: None }), r#"{"type":"device_no","reason":"none"}"#);
        assert_eq!(shape(Control::DeviceNonce { nonce: "ab".into() }), r#"{"type":"device_nonce","nonce":"ab"}"#);
        assert_eq!(shape(Control::DeviceProof { proof: "ab".into() }), r#"{"type":"device_proof","proof":"ab"}"#);
        assert_eq!(shape(Control::DeviceUnlink), r#"{"type":"device_unlink"}"#);
        assert_eq!(shape(Control::DeviceConfirm), r#"{"type":"device_confirm"}"#);
        let Some(Message::Control(Control::DeviceNo { reason, left })) = Message::parse(br#"{"type":"device_no","reason":5,"left":"many"}"#) else { panic!() };
        assert_eq!((reason.as_str(), left), ("", None));
        assert_eq!(Message::parse(br#"{"type":"device_mark","on":true}"#), None, "gone before any release");
        // 📱 누가 노는지
        assert_eq!(shape(Control::DeviceActive { term: Some(2), you: false, idle: None, meet: false }), r#"{"type":"device_active","term":2}"#);
        assert_eq!(shape(Control::DeviceActive { term: Some(0), you: true, idle: Some(5), meet: false }), r#"{"type":"device_active","term":0,"you":true,"idle":5}"#);
        let Some(Message::Control(Control::DeviceActive { term, you, idle, .. })) = Message::parse(br#"{"type":"device_active","term":-3,"you":"yes","idle":"x"}"#) else { panic!() };
        assert_eq!((term, you, idle), (None, false, None));
    }

    #[test]
    fn device_state_and_the_meeting_claim_have_their_wire_shape() {
        let shape = |m: Control| String::from_utf8(Message::Control(m).to_json()).unwrap();
        let c = WireCare { pet: "cat".into(), stage: "child".into(), stamina: 3.0, fullness: 0.5, locked: true };
        assert_eq!(
            shape(Control::DeviceState { pet: Some("cat".into()), care: vec![c.clone()], handover: false }),
            r#"{"type":"device_state","pet":"cat","care":[{"pet":"cat","stage":"child","stamina":3.0,"fullness":0.5,"locked":true}]}"#
        );
        assert!(shape(Control::DeviceState { pet: None, care: vec![], handover: true }).ends_with(r#""care":[],"handover":true}"#));
        // 깨진 항목은 그 항목만 버림
        let Some(Message::Control(Control::DeviceState { pet, care, handover })) =
            Message::parse(br#"{"type":"device_state","pet":7,"care":[{"pet":"cat","stage":"baby","stamina":1,"fullness":1},{"pet":"dog"},5],"handover":"yes"}"#)
        else {
            panic!()
        };
        assert_eq!((pet, handover), (None, false));
        assert_eq!(care, vec![WireCare { pet: "cat".into(), stage: "baby".into(), stamina: 1.0, fullness: 1.0, locked: false }]);
        // 다시 만나서 정한 주장 (넘겨주기 없음)
        assert_eq!(shape(Control::DeviceActive { term: Some(2), you: false, idle: None, meet: true }), r#"{"type":"device_active","term":2,"meet":true}"#);
    }
}
