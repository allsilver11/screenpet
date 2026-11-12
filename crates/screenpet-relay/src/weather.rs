//! ☀️ Real weather for the apps (v0.6.3): the relay asks Open-Meteo for the
//! current WMO weather code of each built-in *place* (a Korean city, `PLACES`)
//! some connected app wants, at most every 30 minutes per place, and hands it
//! out as `weather {place, code, at}`. Apps never call the weather service
//! themselves and send no coordinates: only a place id from the fixed list
//! (an unknown id is ignored). Open-Meteo sees only the city's coordinates.
//!
//! An app that never says what it wants gets Yongin (`DEFAULT_PLACE`).
//! Places nobody has wanted for 2 h are forgotten, so at most `PLACES.len()`
//! are ever kept. A failed fetch keeps the last good reading and tries again
//! in 5 minutes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A place is fetched again this long after a good reading.
pub const POLL_EVERY: Duration = Duration::from_secs(30 * 60);
/// … and this long after a failed fetch.
pub const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// A place no connected app wants is forgotten after this.
pub const FORGET_AFTER: Duration = Duration::from_secs(2 * 3600);
/// Most places fetched in one round (the rest wait for the next one).
pub const FETCHES_PER_ROUND: usize = 10;
/// Largest Open-Meteo answer read (the real one is well under 1 KiB).
pub const BODY_LIMIT: u64 = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// A built-in place: id (as the app sends it), latitude, longitude.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    pub id: &'static str,
    pub lat: f64,
    pub lon: f64,
}

const fn place(id: &'static str, lat: f64, lon: f64) -> Place {
    Place { id, lat, lon }
}

/// The places the relay serves — the same list as the app's
/// `screenpet_core::real_weather::Place` (a test checks they match).
pub const PLACES: [Place; 18] = [
    place("seoul", 37.5665, 126.9780),
    place("yongin", 37.2411, 127.1776),
    place("suwon", 37.2636, 127.0286),
    place("seongnam", 37.4200, 127.1265),
    place("incheon", 37.4563, 126.7052),
    place("daejeon", 36.3504, 127.3845),
    place("daegu", 35.8714, 128.6014),
    place("gwangju", 35.1595, 126.8526),
    place("busan", 35.1796, 129.0756),
    place("ulsan", 35.5384, 129.3114),
    place("sejong", 36.4800, 127.2890),
    place("jeju", 33.4996, 126.5312),
    place("gangneung", 37.7519, 128.8761),
    place("chuncheon", 37.8813, 127.7298),
    place("jeonju", 35.8242, 127.1480),
    place("cheongju", 36.6424, 127.4890),
    place("changwon", 35.2280, 128.6811),
    place("pohang", 36.0190, 129.3435),
];

/// Yongin-si, Gyeonggi-do.
pub const DEFAULT_PLACE: &str = "yongin";

/// The built-in place with this id; `None` for anything else.
pub fn find_place(id: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.id == id)
}

impl Place {
    pub fn url(&self) -> String {
        format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=weather_code&timezone=GMT",
            self.lat, self.lon
        )
    }
}

/// `current.weather_code` of an Open-Meteo forecast answer (a WMO code, 0–99).
pub fn parse_open_meteo(body: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = v.get("current")?.get("weather_code")?;
    // Open-Meteo sends an integer; tolerate `3.0`.
    let code = code.as_u64().or_else(|| code.as_f64().filter(|c| c.fract() == 0.0 && *c >= 0.0).map(|c| c as u64))?;
    (code <= 99).then_some(code as u16)
}

/// Fetches the weather code of a place (blocking, bounded by timeouts).
pub type Fetch = Arc<dyn Fn(&Place) -> Result<u16, String> + Send + Sync>;

/// The real fetcher: HTTPS GET to Open-Meteo through rustls.
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

/// A reading: WMO code and when the relay got it (unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub code: u16,
    pub at: u64,
}

#[derive(Debug)]
struct Entry {
    reading: Option<Reading>,
    /// Fetch again at (`None` = as soon as it is in use).
    next: Option<Instant>,
    /// Last time a connected app wanted it.
    wanted: Instant,
    /// Being fetched right now.
    busy: bool,
}

/// The wanted places (by id) and their last readings. Only built-in places
/// get in, so this never holds more than `PLACES.len()`.
#[derive(Debug, Default)]
pub struct Weathers {
    places: HashMap<&'static str, Entry>,
}

impl Weathers {
    pub fn len(&self) -> usize {
        self.places.len()
    }

    /// An app wants the place `id` now: its canonical id, or `None` (and
    /// nothing kept) if it is not a built-in place.
    pub fn want(&mut self, id: &str, now: Instant) -> Option<&'static str> {
        let id = find_place(id)?.id;
        self.places.entry(id).and_modify(|e| e.wanted = now).or_insert(Entry { reading: None, next: None, wanted: now, busy: false });
        Some(id)
    }

    pub fn reading(&self, id: &str) -> Option<Reading> {
        self.places.get(id).and_then(|e| e.reading)
    }

    /// Places to fetch now (at most `FETCHES_PER_ROUND`, marked busy), given
    /// the places connected apps want; forgets places nobody wanted for
    /// `FORGET_AFTER`.
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

    /// A fetch finished. A good code becomes the reading (returned); a
    /// failure keeps the old one and tries again sooner.
    pub fn fetched(&mut self, id: &str, code: Option<u16>, now: Instant, unix: u64) -> Option<Reading> {
        let e = self.places.get_mut(id)?;
        e.busy = false;
        e.next = Some(now + if code.is_some() { POLL_EVERY } else { RETRY_AFTER });
        let code = code?;
        e.reading = Some(Reading { code, at: unix });
        e.reading
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_places_are_exactly_the_apps_list() {
        use screenpet_core::real_weather::Place as AppPlace;
        assert_eq!(PLACES.len(), AppPlace::ALL.len());
        for (relay, app) in PLACES.iter().zip(AppPlace::ALL) {
            assert_eq!((relay.id, relay.lat, relay.lon), (app.id(), app.coords().0, app.coords().1));
        }
        assert_eq!(DEFAULT_PLACE, AppPlace::default().id());
    }

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

    /// Real network: `cargo test -p screenpet-relay -- --ignored live_open_meteo`
    #[test]
    #[ignore]
    fn live_open_meteo() {
        let code = open_meteo()(find_place(DEFAULT_PLACE).unwrap()).expect("Open-Meteo answers");
        println!("Yongin weather code {code}");
        assert!(code <= 99);
    }
}
