//! 📮 공개 건의함 한도: 앱(요약 만들기)과 릴레이(다시 검사, 페이지 나누기)가 같이 지킴.

/// 한 페이지 항목 수
pub const PER_PAGE: usize = 5;
/// `board_put` 하나에 담는 요약 수
pub const PUT_MAX: usize = 20;
/// 가린 이름의 최대 글자 수 (`김**`)
pub const MAX_NAME: usize = 16;
/// 첫 줄의 최대 글자 수 (릴레이가 받는 한도. 앱은 더 짧게 자름)
pub const MAX_LINE: usize = 80;
/// 이름 첫 글자(그래핌)의 최대 문자 수 (이모지 가족은 5–7자)
pub const MAX_FIRST: usize = 8;
