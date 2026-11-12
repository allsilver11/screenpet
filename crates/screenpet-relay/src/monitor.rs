//! 감시 도우미: 구조화 로그 줄을 stderr(journald)로 내면서 최근 5분·1시간 사건 수(`admin status`)와
//! 최근 WARN/ERROR 줄 고리(`admin recent`)를 메모리에 둠. 주소·키 없음 (줄은 이미 가려진 모양임).
//!
//! 로그 싱크는 프로세스에 하나라 감시 상태도 전역 하나임 ([`global`]). `Relay::start`가 [`install`]을 부름.

use screenpet_log::{Level, Record};
use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

/// 고리에 두는 WARN/ERROR 줄 수
pub(crate) const RECENT_MAX: usize = 200;
/// `admin recent`의 기본 줄 수
pub(crate) const RECENT_DEFAULT: usize = 20;
/// 창 칸 하나의 길이 (초)
const BUCKET_SECS: u64 = 60;
/// 칸을 두는 시간 (가장 긴 창 = 1시간)
const KEEP_BUCKETS: u64 = 60;
/// 짧은 창 (초)
pub(crate) const SHORT: u64 = 5 * 60;
/// 긴 창 (초)
pub(crate) const LONG: u64 = 60 * 60;

/// 로그 영역 목록 (릴레이). `admin status`의 영역별 오류 순서임
pub(crate) const AREAS: [&str; 10] = ["conn", "hub", "presence", "route", "mail", "board", "weather", "dl", "admin", "store"];

/// `admin status` 창 행: (행 이름, 영역 (`*` = 아무 영역), 사건)
const ROWS: [(&str, &str, &str); 12] = [
    ("conns_accepted", "conn", "accept"),
    ("conns_closed", "conn", "close"),
    ("logins", "hub", "login"),
    ("bad_message", "*", "bad_message"),
    ("rate_limited", "*", "rate_limited"),
    ("letters", "mail", "letter_in"),
    ("letters_refused", "mail", "letter_refused"),
    ("board_lists", "board", "board_list"),
    ("weather_ok", "weather", "fetch_ok"),
    ("weather_fail", "weather", "fetch_fail"),
    ("downloads_started", "dl", "download_start"),
    ("downloads_cut", "dl", "download_cut"),
];

/// 분 칸으로 센 사건 수임: 이름마다 (분 번호, 수)를 최근 1시간만 둠
#[derive(Default)]
pub(crate) struct Windows {
    /// 이름 → (분 번호, 수), 오래된 것 먼저
    counts: Mutex<HashMap<String, VecDeque<(u64, u32)>>>,
}

impl Windows {
    /// `now`(유닉스 초)에 `name` 하나를 셈
    pub fn add(&self, name: &str, now: u64) {
        let minute = now / BUCKET_SECS;
        let mut counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        let q = counts.entry(name.to_string()).or_default();
        match q.back_mut() {
            Some((m, n)) if *m == minute => *n += 1,
            _ => q.push_back((minute, 1)),
        }
        while q.front().is_some_and(|(m, _)| *m + KEEP_BUCKETS <= minute) {
            q.pop_front();
        }
    }

    /// `now`까지 지난 `span`초(분 칸 단위, 지금 칸 포함) 동안의 `name` 수
    pub fn sum(&self, name: &str, now: u64, span: u64) -> u64 {
        let minute = now / BUCKET_SECS;
        let buckets = span.div_ceil(BUCKET_SECS);
        let counts = self.counts.lock().unwrap_or_else(|e| e.into_inner());
        counts.get(name).map_or(0, |q| q.iter().filter(|(m, _)| *m + buckets > minute).map(|(_, n)| u64::from(*n)).sum())
    }
}

/// 최근 WARN/ERROR 줄 고리임
pub(crate) struct Recent {
    /// 줄들, 오래된 것 먼저
    lines: Mutex<VecDeque<String>>,
    /// 최대 줄 수
    max: usize,
}

impl Recent {
    /// 최대 `max`줄을 두는 고리를 만듦
    pub fn new(max: usize) -> Recent {
        Recent { lines: Mutex::new(VecDeque::new()), max }
    }

    /// 줄 하나를 넣음 (차면 가장 오래된 것을 버림)
    pub fn push(&self, line: &str) {
        let mut lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        if lines.len() >= self.max {
            lines.pop_front();
        }
        lines.push_back(line.to_string());
    }

    /// 마지막 `n`줄을 오래된 것부터 돌려줌
    pub fn last(&self, n: usize) -> Vec<String> {
        let lines = self.lines.lock().unwrap_or_else(|e| e.into_inner());
        lines.iter().skip(lines.len().saturating_sub(n)).cloned().collect()
    }
}

/// 창과 고리를 묶은 감시 상태임
pub(crate) struct Monitor {
    /// 사건 수 창
    pub windows: Windows,
    /// 최근 WARN/ERROR 줄
    pub recent: Recent,
}

impl Monitor {
    /// 빈 감시 상태를 만듦
    pub fn new() -> Monitor {
        Monitor { windows: Windows::default(), recent: Recent::new(RECENT_MAX) }
    }

    /// 로그 줄 하나를 셈: 맞는 창 행, ERROR면 영역별 오류, WARN/ERROR면 고리에 넣음
    pub fn record(&self, level: Level, area: &str, event: &str, line: &str, now: u64) {
        for (row, a, e) in ROWS {
            if (a == "*" || a == area) && e == event {
                self.windows.add(row, now);
            }
        }
        if level == Level::Error {
            self.windows.add(&format!("errors {area}"), now);
        }
        if level != Level::Info {
            self.recent.push(line);
        }
    }

    /// `admin status`에 붙는 창 글: 행마다 `이름 5분수 / 1시간수`, 그다음 영역별 오류
    pub fn status_text(&self, now: u64) -> String {
        let row = |name: &str| format!("{} / {}", self.windows.sum(name, now, SHORT), self.windows.sum(name, now, LONG));
        let mut out = String::from("rates (last 5m / 1h):\n");
        for (name, _, _) in ROWS {
            out += &format!("  {name} {}\n", row(name));
        }
        out += "errors by area (last 5m / 1h):\n";
        for area in AREAS {
            out += &format!("  {area} {}\n", row(&format!("errors {area}")));
        }
        out
    }

    /// `admin recent [N]`의 답: 마지막 N줄 (오래된 것부터)
    pub fn recent_text(&self, n: usize) -> String {
        let lines = self.recent.last(n);
        if lines.is_empty() {
            return "no warnings or errors since the relay started".to_string();
        }
        lines.join("\n")
    }
}

/// 연결 번호의 로그 모양 (`c42`): 릴레이 실행마다 accept에서 하나씩 줌
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cid(pub u64);

impl std::fmt::Display for Cid {
    /// `c` + 번호로 씀
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.0)
    }
}

/// 걸린 밀리초 (`took_ms=` 값)
pub(crate) fn took_ms(since: std::time::Instant) -> u128 {
    since.elapsed().as_millis()
}

/// 프로세스의 감시 상태
static MONITOR: OnceLock<Monitor> = OnceLock::new();

/// 전역 감시 상태를 돌려줌 (처음 부르면 만듦)
pub(crate) fn global() -> &'static Monitor {
    MONITOR.get_or_init(Monitor::new)
}

/// 지금 유닉스 초
fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 로그 싱크를 정함: 줄을 stderr에 쓰고 전역 감시 상태에 셈. 두 번째부터는 아무것도 안 함
pub(crate) fn install() {
    screenpet_log::set_sink(|r: &Record| {
        eprintln!("{}", r.line);
        global().record(r.level, r.area, r.event, r.line, unix_now());
    });
}

/// 창·고리·글 모양 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 시험 기준 시각 (분 경계에서 30초 지남)
    const NOW: u64 = 1_794_549_630;

    #[test]
    fn windows_count_the_last_5_minutes_and_the_last_hour() {
        let w = Windows::default();
        w.add("x", NOW);
        w.add("x", NOW);
        w.add("x", NOW - 4 * 60); // 5분 창 안 (4칸 전)
        w.add("x", NOW - 5 * 60); // 5분 창 밖, 1시간 안
        w.add("x", NOW - 59 * 60); // 1시간 안 마지막 칸
        w.add("y", NOW);
        assert_eq!(w.sum("x", NOW, SHORT), 3);
        assert_eq!(w.sum("x", NOW, LONG), 5);
        assert_eq!(w.sum("y", NOW, SHORT), 1);
        assert_eq!(w.sum("nope", NOW, LONG), 0);
        // 시간이 흐르면 빠짐
        assert_eq!(w.sum("x", NOW + 60 * 60, LONG), 0);
        assert_eq!(w.sum("x", NOW + 60, SHORT), 2);
    }

    #[test]
    fn windows_forget_buckets_older_than_an_hour() {
        let w = Windows::default();
        for i in 0..200 {
            w.add("x", NOW + i * 60);
        }
        let counts = w.counts.lock().unwrap();
        assert!(counts["x"].len() <= KEEP_BUCKETS as usize, "{}", counts["x"].len());
    }

    #[test]
    fn recent_keeps_the_last_lines_in_order() {
        let r = Recent::new(3);
        assert!(r.last(10).is_empty());
        for i in 0..5 {
            r.push(&format!("l{i}"));
        }
        assert_eq!(r.last(10), ["l2", "l3", "l4"]);
        assert_eq!(r.last(2), ["l3", "l4"]);
        assert_eq!(r.last(0), Vec::<String>::new());
    }

    #[test]
    fn record_counts_rows_errors_and_keeps_only_warnings() {
        let m = Monitor::new();
        m.record(Level::Info, "conn", "accept", "a", NOW);
        m.record(Level::Info, "conn", "accept", "b", NOW);
        m.record(Level::Warn, "conn", "bad_message", "c", NOW);
        m.record(Level::Warn, "mail", "rate_limited", "d", NOW);
        m.record(Level::Error, "store", "save_fail", "e", NOW);
        m.record(Level::Info, "dl", "download_start", "f", NOW - 30 * 60);
        assert_eq!(m.windows.sum("conns_accepted", NOW, SHORT), 2);
        assert_eq!(m.windows.sum("bad_message", NOW, SHORT), 1);
        assert_eq!(m.windows.sum("rate_limited", NOW, SHORT), 1);
        assert_eq!(m.windows.sum("errors store", NOW, LONG), 1);
        assert_eq!(m.recent.last(10), ["c", "d", "e"]);
        let s = m.status_text(NOW);
        assert!(s.starts_with("rates (last 5m / 1h):\n"), "{s}");
        assert!(s.contains("\n  conns_accepted 2 / 2\n"), "{s}");
        assert!(s.contains("\n  downloads_started 0 / 1\n"), "{s}");
        assert!(s.contains("\nerrors by area (last 5m / 1h):\n"), "{s}");
        assert!(s.contains("\n  store 1 / 1\n"), "{s}");
        assert!(s.contains("\n  conn 0 / 0\n"), "{s}");
        for (name, _, _) in ROWS {
            assert!(s.contains(&format!("  {name} ")), "{name}");
        }
    }

    #[test]
    fn connection_ids_look_like_c42() {
        assert_eq!(Cid(42).to_string(), "c42");
    }

    #[test]
    fn recent_text_says_when_empty() {
        let m = Monitor::new();
        assert_eq!(m.recent_text(5), "no warnings or errors since the relay started");
        m.record(Level::Warn, "conn", "bad_message", "w1", NOW);
        m.record(Level::Error, "store", "save_fail", "e1", NOW);
        assert_eq!(m.recent_text(5), "w1\ne1");
        assert_eq!(m.recent_text(1), "e1");
    }
}
