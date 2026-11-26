//! 릴레이 ↔ 앱 사이의 약속을 한곳에 모음.
//!
//! - [`relay`]: 릴레이 WebSocket(`/ws`)의 JSON 메시지 (양방향)
//! - [`board`]: 📮 공개 건의함 한도 (양쪽이 같이 지킴)
//! - [`weather`]: ☀️ 날씨 지역 표 (id, 한글 이름, 위도, 경도)
//! - [`pet_stats`]: 📊 익명 펫 통계 보고·하루 합계와 그 한도
//! - 프로토콜 버전과 양쪽이 같이 쓰는 크기 한도
//!
//! serde만 씀 (ring 등 무거운 의존성 없음). 와이어 형식은 `tests/golden.rs`가 글자 그대로 고정함.

pub mod board;
pub mod pet_stats;
pub mod relay;
pub mod weather;

/// 릴레이 프로토콜 버전 (challenge의 `v`). 2 = `weather_want`, 3 = 📮 `board_put`·`board_list`·`letter_reject`,
/// 4 = 📊 `stats_report`·`stats_get`, 5 = 🖼️ 사진 붙은 편지 (`letter.img`·`letter_img`).
pub const RELAY_PROTOCOL_VERSION: u32 = 5;
/// ☀️ `weather_want`를 받는 가장 낮은 릴레이 버전 (더 옛 릴레이는 모르는 메시지에 연결을 끊음)
pub const WEATHER_MIN_VERSION: u32 = 2;
/// 📮 `board_put`·`board_list`·`letter_reject`·`letter_private`를 받는 가장 낮은 릴레이 버전
pub const BOARD_MIN_VERSION: u32 = 3;
/// 📊 `stats_report`·`stats_get`을 받는 가장 낮은 릴레이 버전 (옛 릴레이에는 아예 안 보냄 — 통계가 조용히 꺼짐)
pub const STATS_MIN_VERSION: u32 = 4;
/// 🖼️ 사진 붙은 편지를 받는 가장 낮은 릴레이 버전 (옛 릴레이는 큰 메시지에 연결을 끊음 — 앱은 사진 없이 보냄)
pub const IMAGE_MIN_VERSION: u32 = 5;

/// 채널 하나로 나르는 가장 큰 불투명 프레임 (LAN 전송과 같음)
pub const MAX_FRAME: usize = 65535;
/// 받아 주는 가장 긴 설치 id
pub const MAX_ID: usize = 128;
/// 릴레이에 알리는 친구 수 상한 (릴레이 기본 한도와 같음)
pub const MAX_FRIENDS: usize = 50;
/// 봉인된 건의 편지 하나의 최대 바이트 (앱이 만들 수 있는 크기 = 릴레이가 받는 크기)
pub const MAX_SEALED: usize = 8192;
/// 🖼️ 편지에 붙이는 사진 한 장의 최대 바이트 (앱이 다시 인코딩한 WebP/JPEG, 긴 변 640px 정도)
pub const MAX_IMAGE: usize = 120 * 1024;
/// 🖼️ 봉인한 사진 덩어리의 최대 바이트 (조각마다 길이 2바이트 + 태그 16바이트가 붙음)
pub const MAX_IMG_SEALED: usize = MAX_IMAGE + 256;
/// 🖼️ 사진 붙은 `letter` 텍스트 메시지와 `letter_img` 답의 최대 길이 (hex 두 덩어리 + JSON). 다른 메시지는 릴레이 `max_text`(16 KiB) 그대로
pub const MAX_LETTER_TEXT: usize = 2 * (MAX_SEALED + MAX_IMG_SEALED) + 1024;
/// 🏐 주고받기 랠리 수 상한 (코어 계산과 P2P 메시지가 같이 씀)
pub const MAX_RALLY: u32 = 20;
