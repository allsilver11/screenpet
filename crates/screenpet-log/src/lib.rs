//! ScreenPet 구조화 로그 한 줄 형식임 (릴레이·앱 공용, 의존성 없음).
//!
//! ```text
//! 2026-11-13T15:04:05.123+09:00 INFO  hub      login        conn=c42 peer=f7da89ae ip=14.33.162.x v=3
//! ```
//! - 시각: KST(+09:00 고정) ISO-8601, 밀리초까지. UTC 안 씀
//! - 수준 `INFO`/`WARN`/`ERROR`, 영역(짧은 고정 낱말), 사건(snake_case 고정 이름), 그 뒤 `key=value`
//! - 값에 빈칸·따옴표·`=`가 있으면 따옴표로 감쌈. `err` 값은 집 폴더 경로를 지우고 짧게 자름
//!
//! 쓰기는 [`log!`] 매크로 → [`emit`] → 싱크. 싱크를 안 정하면 stderr로 감 (릴레이는 journald가 받음).
//! 앱은 [`set_sink`]로 `network.log`에 씀. 개인정보 규칙은 `docs/architecture/logging.md` 참고.

use std::fmt::Display;
use std::sync::OnceLock;

/// 로그 수준임
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// 보통 사건
    Info,
    /// 수상하거나 되살릴 수 있는 일 (한도, 나쁜 메시지, 다시 해 보기)
    Warn,
    /// 고장 (I/O 실패, 다시 해 봐도 실패)
    Error,
}

impl Level {
    /// 줄에 찍는 글자를 돌려줌
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// KST 오프셋 (밀리초)
const KST_MS: i64 = 9 * 3600 * 1000;
/// 하루 밀리초
const DAY_MS: i64 = 86_400_000;
/// `err` 값의 최대 글자 수
const MAX_ERR: usize = 160;

/// 1970-01-01부터 지난 날 수를 (연, 월, 일)로 바꿈 (H. Hinnant의 civil_from_days)
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

/// 유닉스 밀리초를 KST ISO-8601 글자로 바꿈 (`2026-11-13T15:04:05.123+09:00`)
pub fn kst_time(unix_ms: i64) -> String {
    let local = unix_ms + KST_MS;
    let (days, ms) = (local.div_euclid(DAY_MS), local.rem_euclid(DAY_MS));
    let (y, mo, d) = civil_from_days(days);
    let s = ms / 1000;
    format!("{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}.{:03}+09:00", s / 3600, s / 60 % 60, s % 60, ms % 1000)
}

/// 지금 유닉스 밀리초를 돌려줌
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// 값 하나를 줄에 넣을 모양으로 만듦: 빈칸·따옴표·`=`·제어 문자가 있거나 비었으면 따옴표로 감싸고 이스케이프함
pub fn quote(v: &str) -> String {
    let plain = !v.is_empty() && !v.chars().any(|c| c.is_whitespace() || c.is_control() || c == '"' || c == '=' || c == '\\');
    if plain {
        return v.to_string();
    }
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\x{:02x}", c as u32 & 0xff)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 낱말 하나가 파일 경로처럼 보이는지 봄 (`/…`, `~…`, `C:\…`, 역슬래시 포함)
fn looks_like_path(word: &str) -> bool {
    let w = word.trim_matches(|c: char| matches!(c, '(' | ')' | '"' | '\'' | ',' | ':' | ';'));
    let b = w.as_bytes();
    w.starts_with('/') || w.starts_with('~') || w.contains('\\') || (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
}

/// 오류 글을 로그에 맞게 줄임: 경로 낱말은 `<path>`로 바꾸고 줄바꿈을 없애고 `MAX_ERR`자로 자름
pub fn scrub_err(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().map(|w| if looks_like_path(w) { "<path>" } else { w }).collect();
    let joined = words.join(" ");
    match joined.char_indices().nth(MAX_ERR) {
        Some((i, _)) => format!("{}…", &joined[..i]),
        None => joined,
    }
}

/// 주소를 가림: IPv4는 마지막 옥텟을 `x`로, IPv6은 앞 네 묶음 + `::x`. `/64` 같은 접미사와 포트도 받음. 못 읽으면 `?`
pub fn mask_ip(ip: &str) -> String {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    let s = ip.trim();
    let s = s.split_once('/').map_or(s, |(a, _)| a);
    let addr = s.parse::<IpAddr>().ok().or_else(|| s.parse::<SocketAddr>().ok().map(|a| a.ip()));
    let v4 = |a: Ipv4Addr| {
        let o = a.octets();
        format!("{}.{}.{}.x", o[0], o[1], o[2])
    };
    let v6 = |a: Ipv6Addr| match a.to_ipv4_mapped() {
        Some(m) => v4(m),
        None => {
            let g = a.segments();
            format!("{:x}:{:x}:{:x}:{:x}::x", g[0], g[1], g[2], g[3])
        }
    };
    match addr {
        Some(IpAddr::V4(a)) => v4(a),
        Some(IpAddr::V6(a)) => v6(a),
        None => "?".to_string(),
    }
}

/// 설치 키·id의 앞 8글자 (`peer=`·`friend=` 값)
pub fn short(key: &str) -> &str {
    match key.char_indices().nth(8) {
        Some((i, _)) => &key[..i],
        None => key,
    }
}

/// 한 줄을 만듦 (시각은 `unix_ms`). 필드는 받은 순서대로 쓰고 `err` 값은 [`scrub_err`]를 거침
pub fn format_line(unix_ms: i64, level: Level, area: &str, event: &str, fields: &[(&str, &str)]) -> String {
    let mut out = format!("{} {:<5} {:<8} {:<12}", kst_time(unix_ms), level.as_str(), area, event);
    for (k, v) in fields {
        out.push(' ');
        out.push_str(k);
        out.push('=');
        if *k == "err" {
            out.push_str(&quote(&scrub_err(v)));
        } else {
            out.push_str(&quote(v));
        }
    }
    out.truncate(out.trim_end().len());
    out
}

/// 지금 시각으로 한 줄을 만듦
pub fn log_line(level: Level, area: &str, event: &str, fields: &[(&str, &str)]) -> String {
    format_line(now_ms(), level, area, event, fields)
}

/// 싱크가 받는 줄 하나임
pub struct Record<'a> {
    /// 수준
    pub level: Level,
    /// 영역
    pub area: &'a str,
    /// 사건
    pub event: &'a str,
    /// 다 만든 줄 (줄바꿈 없음)
    pub line: &'a str,
}

/// 싱크 함수 꼴
type Sink = Box<dyn Fn(&Record) + Send + Sync>;
/// 정한 싱크 (한 번만)
static SINK: OnceLock<Sink> = OnceLock::new();

/// 줄을 받을 곳을 정함. 프로세스마다 한 번만 됨 (이미 있으면 false)
pub fn set_sink(f: impl Fn(&Record) + Send + Sync + 'static) -> bool {
    SINK.set(Box::new(f)).is_ok()
}

/// 줄을 만들어 싱크(없으면 stderr)로 보냄. 보통 [`log!`]로 부름
pub fn emit(level: Level, area: &str, event: &str, fields: &[(&str, &dyn Display)]) {
    let owned: Vec<String> = fields.iter().map(|(_, v)| v.to_string()).collect();
    let pairs: Vec<(&str, &str)> = fields.iter().zip(&owned).map(|((k, _), v)| (*k, v.as_str())).collect();
    let line = log_line(level, area, event, &pairs);
    match SINK.get() {
        Some(sink) => sink(&Record { level, area, event, line: &line }),
        None => eprintln!("{line}"),
    }
}

/// 구조화 로그 한 줄을 냄: `log!(Warn, "conn", "bad_message", "conn" => c, "reason" => "json")`
///
/// 영역·사건·키는 글자 그대로여야 함 (`tools/test_logging.py`가 목록과 개인정보 규칙을 검사함)
#[macro_export]
macro_rules! log {
    ($lvl:ident, $area:literal, $event:literal $(, $k:literal => $v:expr)* $(,)?) => {
        $crate::emit($crate::Level::$lvl, $area, $event, &[$(($k, &$v as &dyn ::std::fmt::Display)),*])
    };
}

/// 형식·시각·따옴표·주소 가리기 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-13T06:04:05.123Z (= KST 15:04:05.123)
    const T: i64 = 1_794_549_845_123;

    #[test]
    fn time_is_kst_with_milliseconds() {
        assert_eq!(kst_time(T), "2026-11-13T15:04:05.123+09:00");
        assert_eq!(kst_time(0), "1970-01-01T09:00:00.000+09:00");
        // UTC 15:00 넘으면 KST로는 다음 날
        assert_eq!(kst_time(1_794_582_000_000 - 1), "2026-11-13T23:59:59.999+09:00");
        assert_eq!(kst_time(1_794_582_000_000), "2026-11-14T00:00:00.000+09:00");
        // 윤년 2월 29일, 해 넘김
        assert_eq!(kst_time(1_709_164_800_000), "2024-02-29T09:00:00.000+09:00");
        assert_eq!(kst_time(1_798_729_200_000), "2027-01-01T00:00:00.000+09:00");
        assert!(kst_time(now_ms()).ends_with("+09:00"));
    }

    #[test]
    fn line_has_padded_columns_and_fields_in_order() {
        let l = format_line(T, Level::Info, "hub", "login", &[("conn", "c42"), ("peer", "f7da89ae"), ("ip", "14.33.162.x"), ("v", "3")]);
        assert_eq!(l, "2026-11-13T15:04:05.123+09:00 INFO  hub      login        conn=c42 peer=f7da89ae ip=14.33.162.x v=3");
        let e = format_line(T, Level::Error, "store", "save_fail", &[]);
        assert_eq!(e, "2026-11-13T15:04:05.123+09:00 ERROR store    save_fail");
        let w = format_line(T, Level::Warn, "conn", "login_replaced_x", &[("b", "2"), ("a", "1")]);
        assert!(w.ends_with("WARN  conn     login_replaced_x b=2 a=1"), "{w}");
    }

    #[test]
    fn values_with_spaces_or_quotes_are_quoted() {
        assert_eq!(quote("idle"), "idle");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("two words"), "\"two words\"");
        assert_eq!(quote("a=b"), "\"a=b\"");
        assert_eq!(quote("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quote("a\nb"), "\"a\\nb\"");
        assert_eq!(quote("back\\slash"), "\"back\\\\slash\"");
        let l = format_line(T, Level::Warn, "weather", "fetch_fail", &[("err", "timed out")]);
        assert!(l.ends_with("err=\"timed out\""), "{l}");
        assert!(!l.contains('\n'));
    }

    #[test]
    fn err_values_lose_home_paths_and_are_short() {
        assert_eq!(scrub_err("/home/jde/.local/share/screenpet-relay/relay.json is corrupt"), "<path> is corrupt");
        assert_eq!(scrub_err(r"C:\Users\kim\AppData\x failed"), "<path> failed");
        assert_eq!(scrub_err("reading ~/x (os error 2)"), "reading <path> (os error 2)");
        assert_eq!(scrub_err("GET https://pet.danni.day/downloads/latest.json: 404"), "GET https://pet.danni.day/downloads/latest.json: 404");
        assert_eq!(scrub_err("a\nb"), "a b");
        assert_eq!(scrub_err(&"가".repeat(500)).chars().count(), MAX_ERR + 1);
        let l = format_line(T, Level::Error, "store", "save_fail", &[("err", "/home/u/x denied")]);
        assert!(l.ends_with("err=\"<path> denied\""), "{l}");
    }

    #[test]
    fn ip_masking() {
        assert_eq!(mask_ip("14.33.162.66"), "14.33.162.x");
        assert_eq!(mask_ip("14.33.162.66:5555"), "14.33.162.x");
        assert_eq!(mask_ip("::ffff:10.0.0.1"), "10.0.0.x");
        assert_eq!(mask_ip("2001:db8:1:2:aaaa::1"), "2001:db8:1:2::x");
        // 릴레이의 주소 키 (/64)
        assert_eq!(mask_ip("2001:db8:1:2::/64"), "2001:db8:1:2::x");
        assert_eq!(mask_ip("[2001:db8::1]:443"), "2001:db8:0:0::x");
        assert_eq!(mask_ip("not an ip"), "?");
        assert_eq!(mask_ip(""), "?");
    }

    #[test]
    fn short_keeps_eight_characters() {
        assert_eq!(short("f7da89ae45a0beef"), "f7da89ae");
        assert_eq!(short("abc"), "abc");
        assert_eq!(short("가나다라마바사아자"), "가나다라마바사아");
    }

    #[test]
    fn the_macro_keeps_keys_and_values() {
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let s = seen.clone();
        assert!(set_sink(move |r| s.lock().unwrap().push(format!("{}|{}|{}|{}", r.level.as_str(), r.area, r.event, r.line))));
        let n = 3usize;
        log!(Warn, "conn", "bad_message", "conn" => format!("c{n}"), "reason" => "json");
        log!(Info, "hub", "stop");
        let got = seen.lock().unwrap().clone();
        assert_eq!(got.len(), 2);
        assert!(got[0].starts_with("WARN|conn|bad_message|"), "{}", got[0]);
        assert!(got[0].ends_with("WARN  conn     bad_message  conn=c3 reason=json"), "{}", got[0]);
        assert!(got[1].ends_with("INFO  hub      stop"), "{}", got[1]);
        assert!(!set_sink(|_| {}), "only once");
    }
}
