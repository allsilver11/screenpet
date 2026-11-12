//! ☀️ 앱용 진짜 날씨임 (v0.6.3): 릴레이가 접속한 앱이 원하는 기본 *지역*(한국 도시, `PLACES`)마다 Open-Meteo에
//! 지금 WMO 날씨 코드를 지역당 최대 30분마다 묻고 `weather {place, code, at}`로 나눠 줌.
//! 앱은 날씨 서비스를 직접 부르지 않고 좌표도 안 보냄: 고정 목록의 지역 id만 보냄 (모르는 id는 무시).
//! Open-Meteo는 도시 좌표만 봄.
//!
//! 원하는 것을 말하지 않은 앱은 용인(`DEFAULT_PLACE`)을 받음. 2시간 동안 아무도 원하지 않은 지역은 잊으므로
//! 최대 `PLACES.len()`개만 둠. 받아 오기에 실패하면 마지막 좋은 값을 두고 5분 뒤 다시 해 봄.
//! 받아 오기는 lib의 `weather_loop`가 잠금 밖에서 하고, 앱별 지역은 `hub::weather`가 다룸.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 좋은 값을 받은 뒤 이만큼 지나면 다시 받아 옴
pub const POLL_EVERY: Duration = Duration::from_secs(30 * 60);
/// … 받아 오기에 실패한 뒤엔 이만큼 뒤에
pub const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// 접속한 앱 누구도 원하지 않는 지역은 이만큼 뒤 잊음
pub const FORGET_AFTER: Duration = Duration::from_secs(2 * 3600);
/// 한 바퀴에 받아 오는 지역 수 상한 (나머지는 다음 바퀴를 기다림)
pub const FETCHES_PER_ROUND: usize = 10;
/// 읽는 Open-Meteo 답의 최대 크기 (진짜 답은 1 KiB보다 훨씬 작음)
pub const BODY_LIMIT: u64 = 16 * 1024;
/// 연결 시간 제한
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// 받아 오기 전체 시간 제한
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// 기본 지역 표: 앱 메뉴와 같은 `screenpet_proto::weather`를 그대로 씀
pub use screenpet_proto::weather::{find_place, Place, DEFAULT_PLACE, PLACES};

/// 지역의 Open-Meteo 주소를 만듦 (`Place`가 다른 크레이트 타입이라 트레이트로 붙임)
pub trait OpenMeteoUrl {
    /// 도시 좌표만 담은 현재 날씨 요청 주소임
    fn url(&self) -> String;
}

impl OpenMeteoUrl for Place {
    /// 도시 좌표와 `current=weather_code`만 담은 Open-Meteo 주소를 만듦
    fn url(&self) -> String {
        format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=weather_code&timezone=GMT",
            self.lat, self.lon
        )
    }
}

/// Open-Meteo 예보 답의 `current.weather_code`(WMO 코드, 0–99)를 읽음. 없거나 범위 밖이면 None임
pub fn parse_open_meteo(body: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = v.get("current")?.get("weather_code")?;
    // Open-Meteo sends an integer; tolerate `3.0`.
    let code = code.as_u64().or_else(|| code.as_f64().filter(|c| c.fract() == 0.0 && *c >= 0.0).map(|c| c as u64))?;
    (code <= 99).then_some(code as u16)
}

/// 지역 하나의 날씨 코드를 받아 오는 함수임 (막힘, 시간 제한으로 묶임)
pub type Fetch = Arc<dyn Fn(&Place) -> Result<u16, String> + Send + Sync>;

/// 진짜 받아 오기임: rustls로 Open-Meteo에 HTTPS GET을 함
pub fn open_meteo() -> Fetch {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(format!("screenpet-relay/{}", env!("CARGO_PKG_VERSION")))
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(FETCH_TIMEOUT))
        .build()
        .into();
    Arc::new(move |place: &Place| {
        let mut resp = agent.get(&place.url()).call().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let body = resp.body_mut().with_config().limit(BODY_LIMIT).read_to_string().map_err(|e| e.to_string())?;
        parse_open_meteo(&body).ok_or_else(|| "no weather_code in the answer".to_string())
    })
}

/// 받은 값 하나임: WMO 코드와 릴레이가 받은 때 (유닉스 초)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// WMO 날씨 코드
    pub code: u16,
    /// 받은 때 (유닉스 초)
    pub at: u64,
}

/// 지역 하나의 상태임
#[derive(Debug)]
struct Entry {
    /// 마지막 좋은 값
    reading: Option<Reading>,
    /// 다음에 받아 올 때 (None = 쓰이자마자)
    next: Option<Instant>,
    /// 접속한 앱이 마지막으로 원한 때
    wanted: Instant,
    /// 지금 받아 오는 중
    busy: bool,
}

/// 원하는 지역(id별)과 마지막 값임. 기본 지역만 들어오므로 `PLACES.len()`개를 넘지 않음
#[derive(Debug, Default)]
pub struct Weathers {
    places: HashMap<&'static str, Entry>,
}

impl Weathers {
    /// 기억하는 지역 수를 돌려줌
    pub fn len(&self) -> usize {
        self.places.len()
    }

    /// 앱이 지금 지역 `id`를 원함: 표준 id를 돌려주고, 기본 지역이 아니면 None임 (아무것도 안 둠)
    pub fn want(&mut self, id: &str, now: Instant) -> Option<&'static str> {
        let id = find_place(id)?.id;
        self.places.entry(id).and_modify(|e| e.wanted = now).or_insert(Entry { reading: None, next: None, wanted: now, busy: false });
        Some(id)
    }

    /// 지역의 마지막 값을 돌려줌
    pub fn reading(&self, id: &str) -> Option<Reading> {
        self.places.get(id).and_then(|e| e.reading)
    }

    /// 지금 받아 올 지역들을 돌려줌 (최대 `FETCHES_PER_ROUND`개, 받는 중으로 표시). `in_use`는 접속한 앱들이 원하는 지역이고,
    /// `FORGET_AFTER` 동안 아무도 원하지 않은 지역은 잊음
    pub fn due(&mut self, now: Instant, in_use: &HashSet<&'static str>) -> Vec<&'static Place> {
        for (id, e) in self.places.iter_mut() {
            if in_use.contains(id) {
                e.wanted = now;
            }
        }
        self.places.retain(|_, e| e.busy || now.duration_since(e.wanted) < FORGET_AFTER);
        let mut due: Vec<&'static str> =
            self.places.iter().filter(|(id, e)| in_use.contains(*id) && !e.busy && e.next.is_none_or(|t| now >= t)).map(|(id, _)| *id).collect();
        due.sort_by_key(|id| (self.places[id].next, *id));
        due.truncate(FETCHES_PER_ROUND);
        for id in &due {
            if let Some(e) = self.places.get_mut(id) {
                e.busy = true;
            }
        }
        due.into_iter().filter_map(find_place).collect()
    }

    /// 받아 오기가 끝남. 좋은 코드는 새 값이 되어 돌려주고, 실패면 옛 값을 두고 더 빨리 다시 해 봄
    pub fn fetched(&mut self, id: &str, code: Option<u16>, now: Instant, unix: u64) -> Option<Reading> {
        let e = self.places.get_mut(id)?;
        e.busy = false;
        e.next = Some(now + if code.is_some() { POLL_EVERY } else { RETRY_AFTER });
        let code = code?;
        e.reading = Some(Reading { code, at: unix });
        e.reading
    }
}

/// 답 읽기·주소·지역 고르기·주기 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_open_meteo_answer() {
        // A real answer (2026-11-12, Yongin)
        let real = r#"{"latitude":37.2,"longitude":127.1875,"generationtime_ms":0.0865,"utc_offset_seconds":0,"timezone":"GMT","timezone_abbreviation":"GMT","elevation":148.0,"current_units":{"time":"iso8601","interval":"seconds","weather_code":"wmo code"},"current":{"time":"2026-11-12T09:45","interval":900,"weather_code":61}}"#;
        assert_eq!(parse_open_meteo(real), Some(61));
        assert_eq!(parse_open_meteo(r#"{"current":{"weather_code":3.0}}"#), Some(3));
        for bad in [
            "",
            "not json",
            r#"{"error":true,"reason":"Latitude must be in range of -90 to 90°."}"#,
            r#"{"current":{}}"#,
            r#"{"current":{"weather_code":"61"}}"#,
            r#"{"current":{"weather_code":-1}}"#,
            r#"{"current":{"weather_code":100}}"#,
            r#"{"current":{"weather_code":2.5}}"#,
        ] {
            assert_eq!(parse_open_meteo(bad), None, "{bad}");
        }
    }

    #[test]
    fn urls_carry_only_the_citys_coordinates() {
        assert_eq!(
            find_place(DEFAULT_PLACE).unwrap().url(),
            "https://api.open-meteo.com/v1/forecast?latitude=37.2411&longitude=127.1776&current=weather_code&timezone=GMT"
        );
    }

    #[test]
    fn unknown_or_garbage_places_are_ignored_and_add_nothing() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        for bad in ["", "tokyo", "YONGIN", " yongin", "37.2,127.2", "yongin\0", &"x".repeat(10_000)] {
            assert_eq!(w.want(bad, t0), None, "{bad:?}");
        }
        assert_eq!(w.len(), 0);
        for p in PLACES {
            assert_eq!(w.want(p.id, t0), Some(p.id));
            assert_eq!(w.want(p.id, t0), Some(p.id));
        }
        assert_eq!(w.len(), PLACES.len(), "never more than the built-in places");
    }

    #[test]
    fn only_wanted_places_are_fetched_every_half_hour_and_failures_retry_sooner() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        w.want(DEFAULT_PLACE, t0);
        w.want("seoul", t0);
        let in_use: HashSet<&'static str> = [DEFAULT_PLACE].into();
        let ids = |v: Vec<&'static Place>| v.iter().map(|p| p.id).collect::<Vec<_>>();
        assert_eq!(ids(w.due(t0, &in_use)), vec![DEFAULT_PLACE], "Seoul is not in use");
        assert!(w.due(t0, &in_use).is_empty(), "busy");
        assert_eq!(w.fetched(DEFAULT_PLACE, Some(71), t0, 1000), Some(Reading { code: 71, at: 1000 }));
        assert!(w.due(t0 + POLL_EVERY - Duration::from_secs(1), &in_use).is_empty());
        assert_eq!(ids(w.due(t0 + POLL_EVERY, &in_use)), vec![DEFAULT_PLACE]);
        let t1 = t0 + POLL_EVERY;
        assert_eq!(w.fetched(DEFAULT_PLACE, None, t1, 2000), None);
        assert_eq!(w.reading(DEFAULT_PLACE), Some(Reading { code: 71, at: 1000 }), "the last good reading stays");
        assert_eq!(ids(w.due(t1 + RETRY_AFTER, &in_use)), vec![DEFAULT_PLACE]);
    }

    #[test]
    fn unwanted_places_are_forgotten() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        for p in PLACES {
            w.want(p.id, t0);
        }
        let kept: HashSet<&'static str> = ["busan"].into();
        w.due(t0 + FORGET_AFTER - Duration::from_secs(1), &kept);
        assert_eq!(w.len(), PLACES.len());
        w.due(t0 + FORGET_AFTER, &HashSet::new());
        assert_eq!(w.len(), 1, "only the place still wanted lately is left");
    }

    #[test]
    fn a_round_fetches_a_bounded_number_of_places() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        let all: HashSet<&'static str> = PLACES.iter().map(|p| p.id).collect();
        for p in PLACES {
            w.want(p.id, t0);
        }
        assert_eq!(w.due(t0, &all).len(), FETCHES_PER_ROUND);
        assert_eq!(w.due(t0, &all).len(), PLACES.len() - FETCHES_PER_ROUND);
        assert_eq!(w.due(t0, &all).len(), 0);
    }

    /// 실제 망 시험임: `cargo test -p screenpet-relay -- --ignored live_open_meteo`
    #[test]
    #[ignore]
    fn live_open_meteo() {
        let code = open_meteo()(find_place(DEFAULT_PLACE).unwrap()).expect("Open-Meteo answers");
        println!("Yongin weather code {code}");
        assert!(code <= 99);
    }
}
