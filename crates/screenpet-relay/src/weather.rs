//! ☀️ 앱용 진짜 날씨임 (v0.6.3): 릴레이가 접속한 앱이 원하는 기본 *지역*(한국 도시, `PLACES`)마다 날씨 서비스에
//! 지금 WMO 날씨 코드를 지역당 최대 30분마다 묻고 `weather {place, code, at}`로 나눠 줌.
//! 앱은 날씨 서비스를 직접 부르지 않고 좌표도 안 보냄: 고정 목록의 지역 id만 보냄 (모르는 id는 무시).
//! 날씨 서비스는 도시 좌표만 봄.
//!
//! 서비스는 둘임 (`Providers`): Open-Meteo를 먼저, 실패하면 곧바로 MET Norway(Locationforecast)를 씀.
//! MET의 `symbol_code`는 같은 뜻의 WMO 코드로 바꾸므로 앱은 차이를 모름. Open-Meteo가 3번 잇달아 실패하면
//! 1시간 동안 MET을 먼저 쓰고 그 뒤 Open-Meteo를 다시 떠봄. HTTP 429/403을 준 곳은 1시간 쉼.
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
/// 읽는 MET Norway 답의 최대 크기 (진짜 compact 답은 40 KiB쯤)
pub const MET_BODY_LIMIT: u64 = 256 * 1024;
/// Open-Meteo가 잇달아 이만큼 실패하면 MET을 먼저 씀
pub const SWITCH_AFTER_FAILS: u32 = 3;
/// … 이 동안 MET을 먼저 쓰고, 그 뒤 Open-Meteo를 다시 먼저 떠봄
pub const MET_FIRST_FOR: Duration = Duration::from_secs(3600);
/// HTTP 429/403을 준 서비스는 이만큼 안 부름
pub const BACKOFF: Duration = Duration::from_secs(3600);
/// MET 이용 약관이 요구하는 연락처 (User-Agent에 들어감)
pub const MET_CONTACT: &str = "https://pet.danni.day";
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

/// MET compact 답 `properties.timeseries[0]`의 다음 1시간 (없으면 6시간) `symbol_code`를 WMO 코드로 바꿈.
/// 실패 까닭을 글로 돌려줌 (모르는 기호도 실패임)
pub fn parse_met(body: &str) -> Result<u16, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "answer is not JSON".to_string())?;
    let data = v.pointer("/properties/timeseries/0/data").ok_or("no timeseries in the answer")?;
    let symbol = ["next_1_hours", "next_6_hours"]
        .iter()
        .find_map(|k| data.get(k)?.get("summary")?.get("symbol_code")?.as_str())
        .ok_or("no symbol_code in the answer")?;
    met_symbol_wmo(symbol).ok_or_else(|| format!("unknown symbol {}", symbol.chars().take(40).collect::<String>()))
}

/// MET 기호(`rain_day` 같은)를 같은 뜻의 WMO 코드로 바꿈. `_day`/`_night`/`_polartwilight`를 떼고 봄. 모르면 None임
pub fn met_symbol_wmo(symbol: &str) -> Option<u16> {
    let s = ["_day", "_night", "_polartwilight"].iter().find_map(|x| symbol.strip_suffix(x)).unwrap_or(symbol);
    if s.ends_with("thunder") {
        return Some(95);
    }
    Some(match s {
        "clearsky" => 0,
        "fair" => 1,
        "partlycloudy" => 2,
        "cloudy" => 3,
        "fog" => 45,
        "lightrain" => 61,
        "rain" => 63,
        "heavyrain" => 65,
        "lightrainshowers" => 80,
        "rainshowers" => 81,
        "heavyrainshowers" => 82,
        "lightsleet" | "lightsleetshowers" => 66,
        "sleet" | "heavysleet" | "sleetshowers" | "heavysleetshowers" => 67,
        "lightsnow" => 71,
        "snow" => 73,
        "heavysnow" => 75,
        "lightsnowshowers" => 85,
        "snowshowers" | "heavysnowshowers" => 86,
        _ => return None,
    })
}

/// 좌표를 소수 넷째 자리까지로 줄임 (MET 약관). 뒤 0은 안 붙임
fn coord(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

/// 지역의 MET Locationforecast compact 주소를 만듦 (도시 좌표만 담음)
pub fn met_url(place: &Place) -> String {
    format!("https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={}&lon={}", coord(place.lat), coord(place.lon))
}

/// MET에 보내는 User-Agent임: 앱 이름·릴레이 버전·연락처 (약관이 요구함)
pub fn met_user_agent() -> String {
    format!("ScreenPet-relay/{} {MET_CONTACT}", env!("CARGO_PKG_VERSION"))
}

/// 지역 하나의 날씨 코드를 받아 오는 함수임 (막힘, 시간 제한으로 묶임). HTTP 오류는 `HTTP <상태>` 글로 돌려줌
pub type Fetch = Arc<dyn Fn(&Place) -> Result<u16, String> + Send + Sync>;

/// HTTPS만, 리다이렉트 없이, 시간 제한을 둔 ureq 클라이언트를 만듦
fn agent(user_agent: String) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true)
        .max_redirects(0)
        .http_status_as_error(false)
        .user_agent(user_agent)
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(FETCH_TIMEOUT))
        .build()
        .into()
}

/// GET 한 번을 하고 답 글을 `limit`까지 읽음. 성공 상태가 아니면 `HTTP <상태>` 오류임
fn get(agent: &ureq::Agent, url: &str, limit: u64) -> Result<String, String> {
    let mut resp = agent.get(url).call().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    resp.body_mut().with_config().limit(limit).read_to_string().map_err(|e| e.to_string())
}

/// 진짜 받아 오기임: rustls로 Open-Meteo에 HTTPS GET을 함
pub fn open_meteo() -> Fetch {
    let agent = agent(format!("screenpet-relay/{}", env!("CARGO_PKG_VERSION")));
    Arc::new(move |place: &Place| {
        let body = get(&agent, &place.url(), BODY_LIMIT)?;
        parse_open_meteo(&body).ok_or_else(|| "no weather_code in the answer".to_string())
    })
}

/// 진짜 예비 받아 오기임: MET Norway Locationforecast compact에 약관대로 UA를 달아 HTTPS GET을 함
pub fn met() -> Fetch {
    let agent = agent(met_user_agent());
    Arc::new(move |place: &Place| parse_met(&get(&agent, &met_url(place), MET_BODY_LIMIT)?))
}

/// 날씨 서비스 이름임 (로그의 `src=`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Open-Meteo (먼저 씀)
    OpenMeteo,
    /// MET Norway (예비)
    Met,
}

impl Source {
    /// 로그에 쓰는 이름임
    pub fn as_str(self) -> &'static str {
        match self {
            Source::OpenMeteo => "open_meteo",
            Source::Met => "met",
        }
    }
}

impl std::fmt::Display for Source {
    /// `as_str`과 같음
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 서비스 하나에 물어본 결과임
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attempt {
    /// 물어본 곳
    pub src: Source,
    /// WMO 코드 또는 실패 까닭
    pub result: Result<u16, String>,
    /// 걸린 시간 (ms, 쉬는 중이라 안 물었으면 0)
    pub took_ms: u128,
}

/// 쉬는 중이라 안 물었을 때의 실패 글
pub const BACKING_OFF: &str = "backing off after HTTP 429/403";

/// 두 날씨 서비스와 고르는 상태임: 차례 (Open-Meteo 먼저, 잇단 실패 뒤엔 잠시 MET 먼저)와 429/403 뒤 쉬기.
/// 날씨 스레드 하나만 씀 (잠금 없음)
pub struct Providers {
    /// Open-Meteo 받아 오기
    open_meteo: Fetch,
    /// MET 받아 오기 (None = 예비 없음)
    met: Option<Fetch>,
    /// Open-Meteo 잇단 실패 수 (모든 지역 합)
    om_fails: u32,
    /// 이때까지 MET을 먼저 씀
    met_first_until: Option<Instant>,
    /// 서비스별 쉬기 끝 (429/403 뒤)
    backoff: HashMap<Source, Instant>,
}

impl Providers {
    /// Open-Meteo와 (있으면) MET으로 만듦
    pub fn new(open_meteo: Fetch, met: Option<Fetch>) -> Providers {
        Providers { open_meteo, met, om_fails: 0, met_first_until: None, backoff: HashMap::new() }
    }

    /// 지금 물어볼 차례임. 보통 Open-Meteo 먼저, MET 먼저 기간이면 MET 먼저
    pub fn order(&self, now: Instant) -> Vec<Source> {
        match (&self.met, self.met_first_until) {
            (None, _) => vec![Source::OpenMeteo],
            (Some(_), Some(t)) if now < t => vec![Source::Met, Source::OpenMeteo],
            (Some(_), _) => vec![Source::OpenMeteo, Source::Met],
        }
    }

    /// 지역 하나를 차례대로 물어 처음 좋은 값에서 멈춤. 물어본 (또는 쉬느라 건너뛴) 결과를 차례대로 돌려줌
    pub fn fetch(&mut self, place: &Place, now: Instant) -> Vec<Attempt> {
        let mut out = Vec::new();
        for src in self.order(now) {
            if self.backoff.get(&src).is_some_and(|t| now < *t) {
                out.push(Attempt { src, result: Err(BACKING_OFF.to_string()), took_ms: 0 });
                continue;
            }
            let fetch = match src {
                Source::OpenMeteo => &self.open_meteo,
                Source::Met => self.met.as_ref().expect("in the order only when set"),
            };
            let started = Instant::now();
            let result = fetch(place);
            let took_ms = started.elapsed().as_millis();
            self.note(src, &result, now);
            let ok = result.is_ok();
            out.push(Attempt { src, result, took_ms });
            if ok {
                break;
            }
        }
        out
    }

    /// 결과 하나를 기억함: 429/403이면 쉬기, Open-Meteo면 잇단 실패 수와 MET 먼저 기간을 고침
    fn note(&mut self, src: Source, result: &Result<u16, String>, now: Instant) {
        if let Err(e) = result {
            if e == "HTTP 429" || e == "HTTP 403" {
                self.backoff.insert(src, now + BACKOFF);
            }
        }
        if src != Source::OpenMeteo {
            return;
        }
        if result.is_ok() {
            self.om_fails = 0;
            self.met_first_until = None;
        } else {
            self.om_fails += 1;
            if self.om_fails >= SWITCH_AFTER_FAILS {
                self.met_first_until = Some(now + MET_FIRST_FOR);
            }
        }
    }
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

/// 답 읽기·MET 기호·주소·UA·서비스 차례·지역 고르기·주기 시험임
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

    /// 진짜 MET compact 답 (2026-11-13, 용인, 필요한 칸만 남김)
    const MET_SAMPLE: &str = include_str!("../tests/fixtures/met_compact_yongin.json");

    #[test]
    fn parses_a_real_met_answer() {
        // next_1_hours = partlycloudy_day
        assert_eq!(parse_met(MET_SAMPLE), Ok(2));
        // Without next_1_hours (later steps of the forecast), next_6_hours is used
        let six = r#"{"properties":{"timeseries":[{"time":"2026-11-15T18:00:00Z","data":{"instant":{"details":{"air_temperature":6.2}},"next_12_hours":{"summary":{"symbol_code":"partlycloudy_day"}},"next_6_hours":{"summary":{"symbol_code":"lightrain"},"details":{"precipitation_amount":0.4}}}}]}}"#;
        assert_eq!(parse_met(six), Ok(61));
        // next_1_hours wins over next_6_hours
        let both = r#"{"properties":{"timeseries":[{"data":{"next_1_hours":{"summary":{"symbol_code":"heavysnow"}},"next_6_hours":{"summary":{"symbol_code":"clearsky_day"}}}}]}}"#;
        assert_eq!(parse_met(both), Ok(75));
        for bad in [
            "",
            "not json",
            r#"{"properties":{"timeseries":[]}}"#,
            r#"{"properties":{"timeseries":[{"data":{"instant":{}}}]}}"#,
            r#"{"properties":{"timeseries":[{"data":{"next_12_hours":{"summary":{"symbol_code":"rain"}}}}]}}"#,
            r#"{"properties":{"timeseries":[{"data":{"next_1_hours":{"summary":{"symbol_code":7}}}}]}}"#,
            r#"{"properties":{"timeseries":[{"data":{"next_1_hours":{"summary":{"symbol_code":"volcano_day"}}}}]}}"#,
        ] {
            assert!(parse_met(bad).is_err(), "{bad}");
        }
        assert_eq!(
            parse_met(r#"{"properties":{"timeseries":[{"data":{"next_1_hours":{"summary":{"symbol_code":"volcano"}}}}]}}"#),
            Err("unknown symbol volcano".to_string())
        );
    }

    #[test]
    fn met_symbols_map_to_wmo_codes() {
        let table = [
            ("clearsky", 0),
            ("fair", 1),
            ("partlycloudy", 2),
            ("cloudy", 3),
            ("fog", 45),
            ("lightrain", 61),
            ("rain", 63),
            ("heavyrain", 65),
            ("lightrainshowers", 80),
            ("rainshowers", 81),
            ("heavyrainshowers", 82),
            ("lightsleet", 66),
            ("lightsleetshowers", 66),
            ("sleet", 67),
            ("heavysleet", 67),
            ("sleetshowers", 67),
            ("heavysleetshowers", 67),
            ("lightsnow", 71),
            ("snow", 73),
            ("heavysnow", 75),
            ("lightsnowshowers", 85),
            ("snowshowers", 86),
            ("heavysnowshowers", 86),
            ("rainandthunder", 95),
            ("lightrainshowersandthunder", 95),
            ("heavysnowandthunder", 95),
            ("lightssleetshowersandthunder", 95),
        ];
        for (sym, code) in table {
            assert_eq!(met_symbol_wmo(sym), Some(code), "{sym}");
            for suffix in ["_day", "_night", "_polartwilight"] {
                assert_eq!(met_symbol_wmo(&format!("{sym}{suffix}")), Some(code), "{sym}{suffix}");
            }
        }
        for bad in ["", "_day", "volcano", "Rain", "rain_evening", "rain_day_night", "clearsky ", "lightrain_dayx"] {
            assert_eq!(met_symbol_wmo(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn met_codes_look_like_the_sky_in_the_app() {
        // The app's mapping (screenpet-core real_weather::precipitation), copied: light → shower, heavy → rain, …
        fn app(code: u16) -> &'static str {
            match code {
                51 | 53 | 55 | 56 | 57 | 61 | 66 | 80 => "shower",
                63 | 65 | 67 | 81 | 82 | 95 | 96 | 99 => "rain",
                71 | 77 | 85 => "light_snow",
                73 | 75 | 86 => "snow",
                _ => "dry",
            }
        }
        let look = |sym: &str| app(met_symbol_wmo(sym).unwrap());
        for sym in ["clearsky_day", "fair_night", "partlycloudy_day", "cloudy", "fog"] {
            assert_eq!(look(sym), "dry", "{sym}");
        }
        for sym in ["lightrain", "lightrainshowers_day", "lightsleet", "lightsleetshowers_night"] {
            assert_eq!(look(sym), "shower", "{sym}");
        }
        for sym in ["rain", "heavyrain", "rainshowers_day", "heavyrainshowers_night", "sleet", "heavysleetshowers_day", "rainandthunder"] {
            assert_eq!(look(sym), "rain", "{sym}");
        }
        for sym in ["lightsnow", "lightsnowshowers_day"] {
            assert_eq!(look(sym), "light_snow", "{sym}");
        }
        for sym in ["snow", "heavysnow", "snowshowers_night", "heavysnowshowers_day"] {
            assert_eq!(look(sym), "snow", "{sym}");
        }
    }

    #[test]
    fn met_urls_carry_only_rounded_coordinates_and_the_agreed_user_agent() {
        assert_eq!(
            met_url(find_place(DEFAULT_PLACE).unwrap()),
            "https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=37.2411&lon=127.1776"
        );
        let odd = Place { id: "x", name: "x", lat: 37.123456789, lon: 127.0 };
        assert_eq!(met_url(&odd), "https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=37.1235&lon=127");
        for p in PLACES {
            let url = met_url(&p);
            let query = url.split_once('?').unwrap().1;
            for part in query.split('&') {
                let v = part.split_once('=').unwrap().1;
                assert!(v.split_once('.').is_none_or(|(_, d)| d.len() <= 4), "{url}");
            }
        }
        assert_eq!(met_user_agent(), format!("ScreenPet-relay/{} https://pet.danni.day", env!("CARGO_PKG_VERSION")));
        assert_eq!(met_user_agent(), "ScreenPet-relay/0.1.0 https://pet.danni.day");
    }

    /// 부른 곳을 적고 정한 답을 주는 가짜 서비스임
    fn fake(log: &Arc<std::sync::Mutex<Vec<&'static str>>>, name: &'static str, answer: Result<u16, &'static str>) -> Fetch {
        let log = log.clone();
        Arc::new(move |_: &Place| {
            log.lock().unwrap().push(name);
            answer.map_err(str::to_string)
        })
    }

    /// 가짜 서비스 둘과 부른 기록을 만듦
    fn providers(om: Result<u16, &'static str>, met: Result<u16, &'static str>) -> (Providers, Arc<std::sync::Mutex<Vec<&'static str>>>) {
        let log: Arc<std::sync::Mutex<Vec<&'static str>>> = Arc::default();
        (Providers::new(fake(&log, "om", om), Some(fake(&log, "met", met))), log)
    }

    /// 시도들을 (곳, 결과)로
    fn brief(a: &[Attempt]) -> Vec<(Source, Result<u16, String>)> {
        a.iter().map(|a| (a.src, a.result.clone())).collect()
    }

    #[test]
    fn open_meteo_goes_first_and_met_steps_in_right_away() {
        let yongin = find_place(DEFAULT_PLACE).unwrap();
        let t0 = Instant::now();
        let (mut p, log) = providers(Ok(61), Ok(3));
        assert_eq!(brief(&p.fetch(yongin, t0)), vec![(Source::OpenMeteo, Ok(61))], "MET is not asked when Open-Meteo answers");
        let (mut p, log2) = providers(Err("timeout: connect"), Ok(3));
        assert_eq!(brief(&p.fetch(yongin, t0)), vec![(Source::OpenMeteo, Err("timeout: connect".into())), (Source::Met, Ok(3))]);
        let (mut p, _) = providers(Err("HTTP 500"), Err("unknown symbol volcano"));
        let a = p.fetch(yongin, t0);
        assert_eq!(a.len(), 2);
        assert!(a.iter().all(|a| a.result.is_err()));
        assert_eq!(*log.lock().unwrap(), vec!["om"]);
        assert_eq!(*log2.lock().unwrap(), vec!["om", "met"]);
        // Without MET there is only Open-Meteo, even after many failures
        let (_, log3) = providers(Ok(0), Ok(0));
        let mut only = Providers::new(fake(&log3, "om", Err("down")), None);
        for i in 0..5 {
            assert_eq!(only.fetch(yongin, t0 + Duration::from_secs(i)).len(), 1);
        }
    }

    #[test]
    fn when_both_fail_the_last_good_reading_stays_and_it_retries_soon() {
        let yongin = find_place(DEFAULT_PLACE).unwrap();
        let t0 = Instant::now();
        let mut w = Weathers::default();
        w.want(DEFAULT_PLACE, t0);
        let in_use: HashSet<&'static str> = [DEFAULT_PLACE].into();
        let (mut p, _) = providers(Err("down"), Ok(73));
        assert_eq!(w.due(t0, &in_use).len(), 1);
        let code = p.fetch(yongin, t0).iter().find_map(|a| a.result.clone().ok());
        assert_eq!(w.fetched(DEFAULT_PLACE, code, t0, 1000), Some(Reading { code: 73, at: 1000 }), "MET's reading is used");
        let (mut p, _) = providers(Err("down"), Err("down too"));
        let t1 = t0 + POLL_EVERY;
        assert_eq!(w.due(t1, &in_use).len(), 1);
        let code = p.fetch(yongin, t1).iter().find_map(|a| a.result.clone().ok());
        assert_eq!(code, None);
        assert_eq!(w.fetched(DEFAULT_PLACE, code, t1, 2000), None);
        assert_eq!(w.reading(DEFAULT_PLACE), Some(Reading { code: 73, at: 1000 }));
        assert!(w.due(t1 + RETRY_AFTER - Duration::from_secs(1), &in_use).is_empty());
        assert_eq!(w.due(t1 + RETRY_AFTER, &in_use).len(), 1);
    }

    #[test]
    fn after_three_open_meteo_failures_met_goes_first_for_an_hour_then_open_meteo_is_probed() {
        let yongin = find_place(DEFAULT_PLACE).unwrap();
        let t0 = Instant::now();
        let om_ok = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let log: Arc<std::sync::Mutex<Vec<&'static str>>> = Arc::default();
        let (l, ok) = (log.clone(), om_ok.clone());
        let om: Fetch = Arc::new(move |_: &Place| {
            l.lock().unwrap().push("om");
            if ok.load(std::sync::atomic::Ordering::SeqCst) { Ok(0) } else { Err("timeout: connect".into()) }
        });
        let mut p = Providers::new(om, Some(fake(&log, "met", Ok(3))));
        let take = || std::mem::take(&mut *log.lock().unwrap());
        for i in 0..SWITCH_AFTER_FAILS as u64 {
            assert_eq!(p.order(t0 + Duration::from_secs(i)), vec![Source::OpenMeteo, Source::Met]);
            p.fetch(yongin, t0 + Duration::from_secs(i));
            assert_eq!(take(), vec!["om", "met"]);
        }
        let t1 = t0 + Duration::from_secs(SWITCH_AFTER_FAILS as u64 - 1);
        // MET first now: Open-Meteo is not asked while MET answers
        for m in [1, 30, 59] {
            p.fetch(yongin, t1 + Duration::from_secs(m * 60));
            assert_eq!(take(), vec!["met"], "{m} min");
        }
        // After an hour Open-Meteo is probed first; still down → another hour of MET first
        let t2 = t1 + MET_FIRST_FOR;
        assert_eq!(p.order(t2), vec![Source::OpenMeteo, Source::Met]);
        p.fetch(yongin, t2);
        assert_eq!(take(), vec!["om", "met"]);
        p.fetch(yongin, t2 + Duration::from_secs(60));
        assert_eq!(take(), vec!["met"]);
        // Open-Meteo is back: the probe after the next hour succeeds and it goes first again
        om_ok.store(true, std::sync::atomic::Ordering::SeqCst);
        let t3 = t2 + MET_FIRST_FOR;
        assert_eq!(brief(&p.fetch(yongin, t3)), vec![(Source::OpenMeteo, Ok(0))]);
        assert_eq!(take(), vec!["om"]);
        assert_eq!(p.order(t3 + Duration::from_secs(1)), vec![Source::OpenMeteo, Source::Met]);
        // A single failure after that does not switch
        om_ok.store(false, std::sync::atomic::Ordering::SeqCst);
        p.fetch(yongin, t3 + Duration::from_secs(2));
        assert_eq!(p.order(t3 + Duration::from_secs(3)), vec![Source::OpenMeteo, Source::Met]);
    }

    #[test]
    fn a_service_that_answers_429_or_403_is_left_alone_for_a_while() {
        let yongin = find_place(DEFAULT_PLACE).unwrap();
        let t0 = Instant::now();
        for status in ["HTTP 429", "HTTP 403"] {
            let (mut p, log) = providers(Err("timeout: connect"), Err(status));
            p.fetch(yongin, t0);
            assert_eq!(std::mem::take(&mut *log.lock().unwrap()), vec!["om", "met"]);
            let a = p.fetch(yongin, t0 + RETRY_AFTER);
            assert_eq!(std::mem::take(&mut *log.lock().unwrap()), vec!["om"], "{status}: MET is not asked");
            assert_eq!(a[1], Attempt { src: Source::Met, result: Err(BACKING_OFF.to_string()), took_ms: 0 });
            p.fetch(yongin, t0 + BACKOFF);
            assert!(log.lock().unwrap().contains(&"met"), "{status}: asked again after the back-off");
        }
        // Other errors do not back off
        let (mut p, log) = providers(Err("HTTP 500"), Err("HTTP 503"));
        p.fetch(yongin, t0);
        p.fetch(yongin, t0 + Duration::from_secs(1));
        assert_eq!(*log.lock().unwrap(), vec!["om", "met", "om", "met"]);
        // Open-Meteo throttling too: MET alone answers meanwhile
        let (mut p, log) = providers(Err("HTTP 429"), Ok(1));
        p.fetch(yongin, t0);
        p.fetch(yongin, t0 + Duration::from_secs(1));
        assert_eq!(*log.lock().unwrap(), vec!["om", "met", "met"]);
    }

    /// 실제 망 시험임: `cargo test -p screenpet-relay -- --ignored live_met`
    #[test]
    #[ignore]
    fn live_met() {
        let code = met()(find_place(DEFAULT_PLACE).unwrap()).expect("MET answers");
        println!("Yongin weather code from MET {code}");
        assert!(code <= 99);
    }

    /// 실제 망 시험임: 두 서비스를 차례대로 (`cargo test -p screenpet-relay -- --ignored live_providers`)
    #[test]
    #[ignore]
    fn live_providers() {
        let mut p = Providers::new(open_meteo(), Some(met()));
        for a in p.fetch(find_place(DEFAULT_PLACE).unwrap(), Instant::now()) {
            println!("src={} took_ms={} result={:?}", a.src, a.took_ms, a.result);
        }
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
