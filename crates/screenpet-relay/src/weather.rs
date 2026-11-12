//! ☀️ Real weather for the apps (v0.6.3): the relay asks Open-Meteo for the
//! current WMO weather code of each *cell* (latitude/longitude rounded to
//! 0.1°) some connected app wants, at most every 30 minutes per cell, and
//! hands it out as `weather {lat, lon, code, at}`. Apps never call the
//! weather service themselves, and nothing about them is sent there: only
//! the cell's coordinates.
//!
//! An app that never says what it wants gets the Yongin cell
//! (`DEFAULT_CELL`). Cells nobody has wanted for 2 h are forgotten; at most
//! `MAX_CELLS` are kept (a new cell past that is not served until one is
//! forgotten). A failed fetch keeps the last good reading and tries again in
//! 5 minutes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A cell is fetched again this long after a good reading.
pub const POLL_EVERY: Duration = Duration::from_secs(30 * 60);
/// … and this long after a failed fetch.
pub const RETRY_AFTER: Duration = Duration::from_secs(5 * 60);
/// A cell no connected app wants is forgotten after this.
pub const FORGET_AFTER: Duration = Duration::from_secs(2 * 3600);
/// Most cells kept.
pub const MAX_CELLS: usize = 50;
/// Most cells fetched in one round (the rest wait for the next one).
pub const FETCHES_PER_ROUND: usize = 10;
/// Largest Open-Meteo answer read (the real one is well under 1 KiB).
pub const BODY_LIMIT: u64 = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// Latitude and longitude in tenths of a degree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub lat: i32,
    pub lon: i32,
}

/// Yongin-si, Gyeonggi-do (37.2411, 127.1776).
pub const DEFAULT_CELL: Cell = Cell { lat: 372, lon: 1272 };

impl Cell {
    /// The cell of a place an app asked for; `None` unless both are real
    /// coordinates.
    pub fn from_degrees(lat: f64, lon: f64) -> Option<Cell> {
        if !lat.is_finite() || !lon.is_finite() || lat.abs() > 90.0 || lon.abs() > 180.0 {
            return None;
        }
        Some(Cell { lat: (lat * 10.0).round() as i32, lon: (lon * 10.0).round() as i32 })
    }

    pub fn lat_deg(self) -> f64 {
        f64::from(self.lat) / 10.0
    }

    pub fn lon_deg(self) -> f64 {
        f64::from(self.lon) / 10.0
    }

    pub fn url(self) -> String {
        format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.1}&longitude={:.1}&current=weather_code&timezone=GMT",
            self.lat_deg(),
            self.lon_deg()
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

/// Fetches the weather code of a cell (blocking, bounded by timeouts).
pub type Fetch = Arc<dyn Fn(Cell) -> Result<u16, String> + Send + Sync>;

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
    Arc::new(move |cell: Cell| {
        let mut resp = agent.get(&cell.url()).call().map_err(|e| e.to_string())?;
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

/// The cells and their last readings.
#[derive(Debug, Default)]
pub struct Weathers {
    cells: HashMap<Cell, Entry>,
}

impl Weathers {
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// An app wants `cell` now. `false` if it is new and there is no room
    /// (the default cell always has room).
    pub fn want(&mut self, cell: Cell, now: Instant) -> bool {
        if let Some(e) = self.cells.get_mut(&cell) {
            e.wanted = now;
            return true;
        }
        if cell != DEFAULT_CELL && self.cells.keys().filter(|c| **c != DEFAULT_CELL).count() >= MAX_CELLS - 1 {
            return false;
        }
        self.cells.insert(cell, Entry { reading: None, next: None, wanted: now, busy: false });
        true
    }

    pub fn reading(&self, cell: Cell) -> Option<Reading> {
        self.cells.get(&cell).and_then(|e| e.reading)
    }

    /// Cells to fetch now (at most `FETCHES_PER_ROUND`, marked busy), given
    /// the cells connected apps want; forgets cells nobody wanted for
    /// `FORGET_AFTER`.
    pub fn due(&mut self, now: Instant, in_use: &HashSet<Cell>) -> Vec<Cell> {
        for (cell, e) in self.cells.iter_mut() {
            if in_use.contains(cell) {
                e.wanted = now;
            }
        }
        self.cells.retain(|_, e| e.busy || now.duration_since(e.wanted) < FORGET_AFTER);
        let mut due: Vec<Cell> = self
            .cells
            .iter()
            .filter(|(cell, e)| in_use.contains(*cell) && !e.busy && e.next.is_none_or(|t| now >= t))
            .map(|(cell, _)| *cell)
            .collect();
        due.sort_by_key(|c| (self.cells[c].next, c.lat, c.lon));
        due.truncate(FETCHES_PER_ROUND);
        for c in &due {
            if let Some(e) = self.cells.get_mut(c) {
                e.busy = true;
            }
        }
        due
    }

    /// A fetch finished. A good code becomes the reading (returned); a
    /// failure keeps the old one and tries again sooner.
    pub fn fetched(&mut self, cell: Cell, code: Option<u16>, now: Instant, unix: u64) -> Option<Reading> {
        let e = self.cells.get_mut(&cell)?;
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
    fn parses_the_open_meteo_answer() {
        // A real answer (2026-11-12, Yongin cell)
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
    fn cells_round_to_a_tenth_of_a_degree_and_reject_nonsense() {
        assert_eq!(Cell::from_degrees(37.2411, 127.1776), Some(DEFAULT_CELL));
        assert_eq!(Cell::from_degrees(-33.86, 151.21), Some(Cell { lat: -339, lon: 1512 }));
        for (lat, lon) in [(f64::NAN, 0.0), (0.0, f64::INFINITY), (90.1, 0.0), (0.0, -180.5)] {
            assert_eq!(Cell::from_degrees(lat, lon), None);
        }
        assert_eq!(
            DEFAULT_CELL.url(),
            "https://api.open-meteo.com/v1/forecast?latitude=37.2&longitude=127.2&current=weather_code&timezone=GMT"
        );
        assert_eq!(Cell { lat: -5, lon: 0 }.url(), "https://api.open-meteo.com/v1/forecast?latitude=-0.5&longitude=0.0&current=weather_code&timezone=GMT");
    }

    #[test]
    fn only_wanted_cells_are_fetched_every_half_hour_and_failures_retry_sooner() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        let seoul = Cell { lat: 376, lon: 1270 };
        assert!(w.want(DEFAULT_CELL, t0) && w.want(seoul, t0));
        let in_use: HashSet<Cell> = [DEFAULT_CELL].into();
        assert_eq!(w.due(t0, &in_use), vec![DEFAULT_CELL], "Seoul is not in use");
        assert_eq!(w.due(t0, &in_use), Vec::<Cell>::new(), "busy");
        assert_eq!(w.fetched(DEFAULT_CELL, Some(71), t0, 1000), Some(Reading { code: 71, at: 1000 }));
        assert_eq!(w.due(t0 + POLL_EVERY - Duration::from_secs(1), &in_use), Vec::<Cell>::new());
        assert_eq!(w.due(t0 + POLL_EVERY, &in_use), vec![DEFAULT_CELL]);
        let t1 = t0 + POLL_EVERY;
        assert_eq!(w.fetched(DEFAULT_CELL, None, t1, 2000), None);
        assert_eq!(w.reading(DEFAULT_CELL), Some(Reading { code: 71, at: 1000 }), "the last good reading stays");
        assert_eq!(w.due(t1 + RETRY_AFTER, &in_use), vec![DEFAULT_CELL]);
    }

    #[test]
    fn unwanted_cells_are_forgotten_and_the_cap_holds() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        let none = HashSet::new();
        for i in 0..MAX_CELLS as i32 - 1 {
            assert!(w.want(Cell { lat: i, lon: 0 }, t0), "{i}");
        }
        assert!(!w.want(Cell { lat: 999, lon: 0 }, t0), "full");
        assert!(w.want(DEFAULT_CELL, t0), "the default always fits");
        assert_eq!(w.len(), MAX_CELLS);
        let kept: HashSet<Cell> = [Cell { lat: 0, lon: 0 }].into();
        w.due(t0 + FORGET_AFTER - Duration::from_secs(1), &kept);
        assert_eq!(w.len(), MAX_CELLS);
        w.due(t0 + FORGET_AFTER, &none);
        assert_eq!(w.len(), 1, "only the cell still wanted lately is left");
        assert!(w.want(Cell { lat: 999, lon: 0 }, t0 + FORGET_AFTER));
    }

    #[test]
    fn a_round_fetches_a_bounded_number_of_cells() {
        let t0 = Instant::now();
        let mut w = Weathers::default();
        let cells: HashSet<Cell> = (0..30).map(|i| Cell { lat: i, lon: 0 }).collect();
        for c in &cells {
            w.want(*c, t0);
        }
        assert_eq!(w.due(t0, &cells).len(), FETCHES_PER_ROUND);
        assert_eq!(w.due(t0, &cells).len(), FETCHES_PER_ROUND);
        assert_eq!(w.due(t0, &cells).len(), 10);
        assert_eq!(w.due(t0, &cells).len(), 0);
    }

    /// Real network: `cargo test -p screenpet-relay -- --ignored live_open_meteo`
    #[test]
    #[ignore]
    fn live_open_meteo() {
        let code = open_meteo()(DEFAULT_CELL).expect("Open-Meteo answers");
        println!("Yongin weather code {code}");
        assert!(code <= 99);
    }
}
