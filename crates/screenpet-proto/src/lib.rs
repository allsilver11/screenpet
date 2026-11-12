//! 릴레이 ↔ 앱 사이의 약속을 한곳에 모음.
//!
//! - [`relay`]: 릴레이 WebSocket(`/ws`)의 JSON 메시지 (양방향)
//! - [`board`]: 📮 공개 건의함 한도 (양쪽이 같이 지킴)
//! - [`weather`]: ☀️ 날씨 지역 표 (id, 한글 이름, 위도, 경도)
//! - 프로토콜 버전과 양쪽이 같이 쓰는 크기 한도
//!
//! serde만 씀 (ring 등 무거운 의존성 없음). 와이어 형식은 `tests/golden.rs`가 글자 그대로 고정함.

pub mod board;
pub mod relay;
pub mod weather;

/// 릴레이 프로토콜 버전 (challenge의 `v`). 2 = `weather_want`, 3 = 📮 `board_put`·`board_list`·`letter_reject`.
pub const RELAY_PROTOCOL_VERSION: u32 = 3;
/// ☀️ `weather_want`를 받는 가장 낮은 릴레이 버전 (더 옛 릴레이는 모르는 메시지에 연결을 끊음)
pub const WEATHER_MIN_VERSION: u32 = 2;
/// 📮 `board_put`·`board_list`·`letter_reject`·`letter_private`를 받는 가장 낮은 릴레이 버전
pub const BOARD_MIN_VERSION: u32 = 3;

/// 채널 하나로 나르는 가장 큰 불투명 프레임 (LAN 전송과 같음)
pub const MAX_FRAME: usize = 65535;
/// 받아 주는 가장 긴 설치 id
pub const MAX_ID: usize = 128;
/// 릴레이에 알리는 친구 수 상한 (릴레이 기본 한도와 같음)
pub const MAX_FRIENDS: usize = 50;
/// 봉인된 건의 편지 하나의 최대 바이트 (앱이 만들 수 있는 크기 = 릴레이가 받는 크기)
pub const MAX_SEALED: usize = 8192;
/// 🏐 주고받기 랠리 수 상한 (코어 계산과 P2P 메시지가 같이 씀)
pub const MAX_RALLY: u32 = 20;
