//! 릴레이 WebSocket(`/ws`) 메시지: 앱→릴레이 [`ClientMsg`], 릴레이→앱 [`ServerMsg`].
//!
//! 텍스트 프레임은 `t`로 꼬리표를 단 JSON임. 친구끼리의 바이너리 프레임(4바이트 채널 id + 불투명 Noise
//! 프레임)은 여기서 다루지 않음. 한 enum을 양쪽이 같이 씀:
//! - 보내는 쪽은 모든 필드를 씀 (릴레이는 `ServerMsg`, 앱은 `ClientMsg`)
//! - 받는 쪽은 옛 버전이 빠뜨리던 필드와 자기가 안 쓰는 필드를 `#[serde(default)]`로 너그럽게 받음
//! - 모르는 `t`는 양쪽 다 파싱 오류임: 릴레이는 `bad_message`로 끊고, 앱은 그냥 무시함
//!
//! ```text
//! relay  -> client  challenge {relay: hex32 relay public key, nonce: hex32, v}
//! client -> relay   auth {id, key: hex32 static public key, mac: hex32, friends: [hex32]}
//!                   mac = BLAKE2s(AUTH_LABEL || X25519(client, relay) || nonce
//!                                 || relay_pub || client_pub || id)
//!                   (+ cnonce: hex32 client nonce, optional)
//! relay  -> client  welcome {code, code_ttl, online: [hex32], proof}
//!                   proof = BLAKE2s(WELCOME_LABEL || X25519 || cnonce || nonce
//!                                   || client_pub): the relay holds its key
//! client -> relay   new_code                  -> code {code, code_ttl}
//! client -> relay   friends {keys}            -> online {keys}
//! relay  -> client  presence {key, online}    (mutual friends only)
//! relay  -> client  code {code, code_ttl}     (24 h rotation)
//! client -> relay   redeem {code}             -> redeemed {code, key, id} | redeem_failed {code, reason}
//! client -> relay   open {req, to}            -> opened {req, ch} | open_failed {req, reason}
//! relay  -> client  incoming {ch, from, id}
//! both              close {ch}
//! client -> relay   letter {box: hex}          -> letter_ok | letter_refused {reason}
//!                   (a feedback letter sealed to the developer key; reasons:
//!                   daily, ip, new, global, full, size, blocked)
//!                   (v5: + img: hex sealed photo, ≤ MAX_IMG_SEALED; the whole text ≤ MAX_LETTER_TEXT —
//!                   only after letter_begin -> letter_go, which raises this connection's 64 KiB socket limit
//!                   for one letter; few slots relay-wide, else letter_refused {reason: photo_busy})
//! client -> relay   letters                    -> letters {items, board}  (mailbox readers only)
//!                   (v5: items with a photo carry img = its sealed size; the photo itself is not in the list)
//! client -> relay   letter_img {id}            (v5, readers only) -> letter_img {id, img: hex} ("" = none)
//! client -> relay   letter_read {id} | letter_delete {id} | letter_block {from} | letter_done {id, done}
//!                                              (readers only) -> letters_changed to every reader
//!                   (items carry `done` and, from v3, `rejected`; `letter_done false` clears both)
//! relay  -> client  letter_new {item}          (to readers online when a letter arrives)
//! client -> relay   letter_reject {id, rejected} (v3, readers only) 🙅 반려 — clears 반영; -> letters_changed
//! client -> relay   letter_private {id, private} (v3, readers only) 🔒 비공개; -> letters_changed
//! client -> relay   board_put {items: [{id, name, line}]}
//!                   (v3, readers only, ≤ 20 a message) 📮 public digest of a letter
//! client -> relay   board_list {page}          (v3, anyone, ≥ 0.5 s apart; faster ones are ignored)
//!                   -> board_page {items: [{id, name, line, at, status}], page, pages, total}
//!                   5 a page, newest first, page 1-based and clamped,
//!                   status none | done | rejected
//! client -> relay   stats_report {report: {day, v, kept, chosen, changed}}
//!                   (v4) 📊 yesterday's anonymous pet stats, once a day -> stats_ok {day}
//!                   (also when refused: retrying would not help); see `pet_stats`
//! client -> relay   stats_get {before?}        (v4, mailbox readers only) -> stats_days {days, more}
//!                   (one page, newest day first, ≤ REPLY_MAX bytes; ask again with before = oldest day while more)
//! client -> relay   weather_want {place}      (v2 relays only; -> weather if known)
//! relay  -> client  weather {place, code, at} ☀️ WMO code of a built-in place at `at` (unix s)
//! client -> relay   ping                      -> pong
//! relay  -> client  error {reason}            (then the connection closes)
//! ```

use crate::pet_stats::{StatsDay, StatsReport};
use serde::{Deserialize, Serialize};

/// 앱 → 릴레이. 릴레이가 받아 파싱하고 앱이 만들어 보냄
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ClientMsg {
    Auth {
        id: String,
        key: String,
        mac: String,
        #[serde(default)]
        friends: Vec<String>,
        /// 옛 앱은 보내지 않음. 앱은 늘 보냄
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cnonce: Option<String>,
    },
    Friends {
        keys: Vec<String>,
    },
    Redeem {
        code: String,
    },
    Open {
        req: u32,
        to: String,
    },
    Close {
        ch: u32,
    },
    NewCode,
    Ping,
    Letter {
        r#box: String,
        /// 🖼️ 봉인한 사진, hex (v5 릴레이에만 보냄. 없으면 빈 글자이고 와이어에 안 나옴)
        #[serde(default, skip_serializing_if = "String::is_empty")]
        img: String,
    },
    Letters,
    /// 🖼️ v5 릴레이에만 보냄: 사진 편지 하나 보낼 자리를 청함 (`letter_go` 뒤 큰 `letter` 하나)
    LetterBegin,
    /// 🖼️ v5 릴레이에만 보냄 (편지함 독자만 답받음): 편지 `id`의 봉인 사진을 줘
    LetterImg {
        id: u64,
    },
    LetterRead {
        id: u64,
    },
    LetterDelete {
        id: u64,
    },
    LetterDone {
        id: u64,
        done: bool,
    },
    LetterBlock {
        from: String,
    },
    /// ☀️ v2 릴레이에만 보냄
    WeatherWant {
        place: String,
    },
    /// 🙅 v3 릴레이에만 보냄
    LetterReject {
        id: u64,
        rejected: bool,
    },
    /// 🔒 v3 릴레이에만 보냄
    LetterPrivate {
        id: u64,
        private: bool,
    },
    /// 📮 v3 릴레이에만 보냄
    BoardPut {
        items: Vec<BoardIn>,
    },
    /// 📮 v3 릴레이에만 보냄
    BoardList {
        #[serde(default)]
        page: u32,
    },
    /// 📊 v4 릴레이에만 보냄: 지난날 익명 펫 통계 (하루 한 번)
    StatsReport {
        report: StatsReport,
    },
    /// 📊 v4 릴레이에만 보냄 (편지함 독자만 답받음): `before`보다 앞선 날들의 합계를 최신 날부터 한 쪽 줘 (None = 가장 최신부터)
    StatsGet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        before: Option<String>,
    },
}

/// 📮 개발자 앱이 올리는 편지 요약 (릴레이가 다시 검사함)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardIn {
    /// 편지 id
    pub id: u64,
    /// 가린 보낸이 이름 (`김**`)
    pub name: String,
    /// 편지 첫 줄 요약
    pub line: String,
}

/// 릴레이 → 앱. 릴레이가 만들어 보내고 앱이 너그럽게 받음
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ServerMsg {
    Challenge {
        relay: String,
        nonce: String,
        /// 릴레이 프로토콜 버전 (v1 전 릴레이는 보내지 않음: 0)
        #[serde(default)]
        v: u32,
    },
    Welcome {
        code: String,
        /// 앱은 안 씀
        #[serde(default)]
        code_ttl: u64,
        #[serde(default)]
        online: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof: Option<String>,
    },
    Online {
        keys: Vec<String>,
    },
    Presence {
        key: String,
        online: bool,
    },
    Code {
        code: String,
        /// 앱은 안 씀
        #[serde(default)]
        code_ttl: u64,
    },
    Redeemed {
        code: String,
        key: String,
        id: String,
    },
    RedeemFailed {
        code: String,
        reason: String,
    },
    Opened {
        req: u32,
        ch: u32,
    },
    OpenFailed {
        req: u32,
        /// 앱은 안 씀
        #[serde(default)]
        reason: String,
    },
    Incoming {
        ch: u32,
        from: String,
        /// 앱은 안 씀
        #[serde(default)]
        id: String,
    },
    Close {
        ch: u32,
    },
    Pong,
    Error {
        reason: String,
    },
    LetterOk,
    LetterRefused {
        reason: String,
    },
    Letters {
        items: Vec<Letter>,
        /// 📮 이미 공개 건의함에 올라간 편지 id (v2 릴레이는 보내지 않음)
        #[serde(default)]
        board: Vec<u64>,
    },
    LetterNew {
        item: Letter,
    },
    LettersChanged,
    /// 🖼️ `letter_begin`의 답: 이제 이 연결에서 큰 `letter` 하나를 받음 (잠시 안에)
    LetterGo,
    /// 🖼️ 편지 `id`의 봉인 사진, hex (독자에게만. 없거나 지워졌거나 너무 잦은 요청이면 빈 글자)
    LetterImg {
        id: u64,
        #[serde(default)]
        img: String,
    },
    /// ☀️ 기본 지역 하나의 WMO 날씨 코드, 릴레이가 `at`(유닉스 초)에 받아 옴
    Weather {
        place: String,
        code: u16,
        at: i64,
    },
    /// 📮 공개 건의함 한 페이지
    BoardPage {
        items: Vec<BoardItem>,
        page: u32,
        pages: u32,
        total: u32,
    },
    /// 📊 그날 보고를 받았음 (거절했어도 — 다시 보내도 소용없음)
    StatsOk {
        day: String,
    },
    /// 📊 날짜별 합계 한 쪽 (편지함 독자에게만, 최신 날 먼저, JSON ≤ `pet_stats::REPLY_MAX`). `more` = 더 앞선 날이 있음
    StatsDays {
        days: Vec<StatsDay>,
        #[serde(default)]
        more: bool,
    },
}

/// 저장된 건의 편지 하나 (편지함 독자에게만 감)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Letter {
    /// 편지 id (릴레이가 매김)
    pub id: u64,
    /// 유닉스 초
    pub at: u64,
    /// 보낸 설치의 키 (hex)
    pub from: String,
    /// 읽음 표시
    #[serde(default)]
    pub read: bool,
    /// ✅ 반영함 (옛 릴레이는 보내지 않음)
    #[serde(default)]
    pub done: bool,
    /// 🙅 반려 (v3 릴레이)
    #[serde(default)]
    pub rejected: bool,
    /// 🔒 비공개 (v3 릴레이)
    #[serde(default)]
    pub private: bool,
    /// 봉인된 편지, hex
    pub r#box: String,
    /// 🖼️ 봉인한 사진의 바이트 수 (v5 릴레이, 0 = 사진 없음. 사진은 `letter_img`로 따로 받음)
    #[serde(default, skip_serializing_if = "is_zero")]
    pub img: u32,
}

/// 0이면 와이어에 안 씀 (옛 모양 그대로)
fn is_zero(n: &u32) -> bool {
    *n == 0
}

/// 개발자가 편지에 남긴 표시: ✅ 반영됨, 🙅 반려됨, 또는 없음
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardStatus {
    /// ✅ 반영됨
    Done,
    /// 🙅 반려됨
    Rejected,
    /// (더 새 릴레이가 보내는 모르는 값도 없음으로 읽음)
    #[default]
    #[serde(other)]
    None,
}

/// 공개 건의함 페이지의 항목 하나 (`at`, `status`는 편지 자체에서 옴)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardItem {
    /// 편지 id
    pub id: u64,
    /// 가린 보낸이 이름
    pub name: String,
    /// 편지 첫 줄 요약
    pub line: String,
    /// 편지가 온 시각 (유닉스 초)
    pub at: u64,
    /// 개발자 표시 (옛 릴레이는 보내지 않음: 없음)
    #[serde(default)]
    pub status: BoardStatus,
}

impl ClientMsg {
    /// JSON 텍스트로 바꿈
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("client message serializes")
    }
}

impl ServerMsg {
    /// JSON 텍스트로 바꿈
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("server message serializes")
    }
}
