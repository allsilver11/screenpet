//! 📊 익명 펫 통계 (spec 2026-11-23-pet-stats): 앱이 하루 한 번 보내는 지난날 보고([`StatsReport`])와
//! 릴레이가 개발자에게만 주는 날짜별 합계([`StatsDay`]), 그리고 양쪽이 같이 지키는 한도·이름 다듬기·날짜 계산.
//! 보고에는 펫 종류와 횟수, 앱 버전, 그리고 앱이 무작위로 만든 익명 번호([`StatsReport::anon`])만 있음 (이름·네트워크 id·공개키·친구·창 제목 없음).
//! 익명 번호로 누구인지는 모르지만 서로 다른 사람끼리는 구분해 셈 (정책, 2026-11-27). 날짜는 보고하는 앱의 지역 날짜임.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 릴레이가 아는 펫 종류 (ui/pets 폴더 이름). 여기 없는 이름은 모두 [`OTHER`]로 셈 — 아무 글자나 쌓이지 않게
pub const SPECIES: &[&str] = &[
    "bat", "cat", "dementor", "dino_green", "dino_yellow", "hagrid", "harry", "hermione", "heumul", "musoe", "penguin", "pumpkin", "rabbit", "rudolph", "santa", "snowman", "spider", "squirrel", "super", "voldemort",
];
/// 모르는 종류·이상한 버전을 세는 칸
pub const OTHER: &str = "other";
/// 보고 하나의 `kept` 종류 수 상한 (펫은 최대 3마리지만 하루에 바꾼 것까지)
pub const MAX_KEPT: usize = 8;
/// 보고 하나의 `chosen` 종류 수 상한
pub const MAX_CHOSEN: usize = 16;
/// 보고 하나의 `changed` (바꾼 쌍) 수 상한
pub const MAX_CHANGED: usize = 32;
/// 보고 하나의 한 칸 횟수 상한 (넘으면 이 값으로 셈 — 한 사람이 합계를 부풀리지 못함)
pub const MAX_COUNT: u32 = 20;
/// 이름·버전·날짜 글자 길이 상한 (넘으면 other)
pub const MAX_TEXT: usize = 24;
/// 릴레이가 하루 합계를 두는 날 수 (더 지난 날은 지움)
pub const KEEP_DAYS: i64 = 90;
/// 앱이 끝난 날(앱의 지역 날짜)을 이만큼 늦게까지 보고함 (그보다 오래된 기록은 버림)
pub const LATE_DAYS: i64 = 2;
/// 릴레이는 UTC 오늘보다 이만큼 앞선 날까지 받음: `LATE_DAYS` + 하루 (UTC-12인 곳의 지역 날짜는 UTC보다 하루 늦음)
pub const ACCEPT_DAYS: i64 = LATE_DAYS + 1;
/// `stats_days` 한 번의 JSON 크기 상한 (앱이 받는 메시지 64 KiB보다 넉넉히 작게 — 넘는 날은 다음 쪽으로)
pub const REPLY_MAX: usize = 24 * 1024;
/// 하루 합계의 바꾼 쌍 수 상한 (넘는 새 쌍은 other→other로)
pub const MAX_DAY_PAIRS: usize = 64;
/// 하루 합계의 버전 수 상한 (넘는 새 버전은 other로)
pub const MAX_DAY_VERSIONS: usize = 24;
/// 익명 번호 글자 수 (소문자 16진수 — 64비트 무작위)
pub const ANON_LEN: usize = 16;
/// 하루 합계에 적는 익명 번호 수 상한 (넘는 보고는 합계에만 셈 — 하루가 앱 메시지 한도 안에 들게)
pub const MAX_DAY_PEOPLE: usize = 500;

/// 익명 번호 모양: 정확히 [`ANON_LEN`]글자 소문자 16진수
pub fn is_anon(s: &str) -> bool {
    s.len() == ANON_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// 바꾼 것 하나: `from`에서 `to`로 `n`번
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    /// 바꾸기 전 종류
    pub from: String,
    /// 바꾼 뒤 종류
    pub to: String,
    /// 횟수
    pub n: u32,
}

/// 앱이 하루 한 번 보내는 지난날 보고 (`stats_report`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatsReport {
    /// 보고하는 날 (`YYYY-MM-DD`, 앱의 그 지역 날짜)
    pub day: String,
    /// 앱 버전 (`0.7.4`)
    pub v: String,
    /// 그날 키운 종류들 (펫 여러 마리면 각각, 겹치지 않게)
    #[serde(default)]
    pub kept: Vec<String>,
    /// 그날 새로 고른 종류 (처음·펫 추가) → 횟수
    #[serde(default)]
    pub chosen: BTreeMap<String, u32>,
    /// 그날 바꾼 것들
    #[serde(default)]
    pub changed: Vec<Change>,
    /// 이 설치의 익명 번호 (앱이 무작위로 만듦 — 네트워크 id·공개키와 상관없음, 통계를 끄면 지움). 옛 앱은 없음
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anon: Option<String>,
}

/// 릴레이가 두는 하루 합계 (개발자에게 `stats_days`로 감). 누가 보냈는지는 없고 익명 번호만 있음
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsDay {
    /// 날짜 (`YYYY-MM-DD`)
    pub day: String,
    /// 받은 보고 수 (= 그날 통계를 보낸 설치 수)
    pub reports: u32,
    /// 종류 → 그 종류를 키운 설치 수
    pub kept: BTreeMap<String, u32>,
    /// 종류 → 새로 고른 수
    pub chosen: BTreeMap<String, u32>,
    /// 바꾼 쌍과 수
    pub changed: Vec<Change>,
    /// 앱 버전 → 보고 수
    pub versions: BTreeMap<String, u32>,
    /// 그날 보고한 익명 번호들 (온 차례, 최대 [`MAX_DAY_PEOPLE`]). 옛 보고·옛 날은 빠짐
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub people: Vec<String>,
    /// 종류 → 그 종류를 키운 번호들의 `people` 안 자리 (번호를 되풀이하지 않아 하루가 작게)
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keepers: BTreeMap<String, Vec<u16>>,
}

/// 아는 종류면 그 이름, 아니면 [`OTHER`]
pub fn species_or_other(s: &str) -> &'static str {
    SPECIES.iter().find(|k| **k == s).copied().unwrap_or(OTHER)
}

/// `0.7.4`처럼 숫자 세 마디(각 1–3자리)면 그대로, 아니면 [`OTHER`]
pub fn version_or_other(s: &str) -> String {
    let parts: Vec<&str> = s.split('.').collect();
    let ok = s.len() <= MAX_TEXT && parts.len() == 3 && parts.iter().all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    if ok {
        s.to_string()
    } else {
        OTHER.to_string()
    }
}

/// 1970-01-01부터 날 수 → (년, 월, 일)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// (년, 월, 일) → 1970-01-01부터 날 수
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// 날 수를 `YYYY-MM-DD`로
pub fn day_string(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// 정확히 `YYYY-MM-DD`(있는 날짜)면 1970-01-01부터 날 수, 아니면 None
pub fn day_number(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r).filter(|t| t.bytes().all(|c| c.is_ascii_digit())).and_then(|t| t.parse::<u32>().ok());
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || d == 0 {
        return None;
    }
    let n = days_from_civil(i64::from(y), m, d);
    (day_string(n) == s).then_some(n)
}

/// 유닉스 초의 UTC 날 수
pub fn utc_day(unix_secs: u64) -> i64 {
    (unix_secs / 86_400) as i64
}

/// 이름·날짜 계산·다듬기 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_and_odd_dates_are_refused() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2026-11-23"), Some(20_780));
        assert_eq!(day_string(20_780), "2026-11-23");
        assert_eq!(day_number("2024-02-29").map(day_string).as_deref(), Some("2024-02-29"));
        for bad in ["2025-02-29", "2026-13-01", "2026-00-10", "2026-11-31", "2026-1-01", "2026/11/23", "２026-11-23", "", "2026-11-23x", "+026-11-23"] {
            assert_eq!(day_number(bad), None, "{bad}");
        }
        assert_eq!(utc_day(20_780 * 86_400 + 86_399), 20_780);
        for n in [-1, 0, 59, 365, 11_016, 20_000, 30_000] {
            assert_eq!(day_number(&day_string(n)), Some(n));
        }
    }

    #[test]
    fn anon_numbers_are_exactly_sixteen_lowercase_hex() {
        assert!(is_anon("0123456789abcdef"));
        for bad in ["0123456789ABCDEF", "0123456789abcde", "0123456789abcdef0", "0123456789abcdeg", "", "０123456789abcd", "0123456789abcd f"] {
            assert!(!is_anon(bad), "{bad}");
        }
        assert_eq!(ANON_LEN, 16);
    }

    #[test]
    fn old_reports_and_days_without_anon_numbers_still_parse() {
        let r: StatsReport = serde_json::from_str(r#"{"day":"2026-11-22","v":"0.7.4"}"#).unwrap();
        assert_eq!(r.anon, None);
        let d: StatsDay = serde_json::from_str(r#"{"day":"2026-11-22","reports":2}"#).unwrap();
        assert!(d.people.is_empty() && d.keepers.is_empty());
    }

    #[test]
    fn unknown_species_and_versions_become_other() {
        assert_eq!(species_or_other("penguin"), "penguin");
        assert_eq!(species_or_other("Penguin"), OTHER);
        assert_eq!(species_or_other("<script>"), OTHER);
        assert_eq!(species_or_other(""), OTHER);
        assert_eq!(version_or_other("0.7.4"), "0.7.4");
        assert_eq!(version_or_other("10.20.300"), "10.20.300");
        for bad in ["0.7", "0.7.4.1", "v0.7.4", "0.7.x", "0..4", "1000.0.0", "", "0.7.4 "] {
            assert_eq!(version_or_other(bad), OTHER, "{bad}");
        }
    }

    #[test]
    fn every_bundled_pet_is_a_known_species() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
        let mut ids: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("pet.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        ids.sort();
        let mut known: Vec<String> = SPECIES.iter().map(|s| s.to_string()).collect();
        known.sort();
        assert_eq!(ids, known, "add a new pet to pet_stats::SPECIES too (or the relay counts it as other)");
        assert!(SPECIES.iter().all(|s| s.len() <= MAX_TEXT && *s != OTHER));
    }
}
