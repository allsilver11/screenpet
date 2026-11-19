//! ☀️ 실제 날씨 (v0.6.3) — 고른 도시(기본 용인)의 지금 날씨를 계절 날씨 대신 보여 줌
//!
//! 앱은 날씨 서비스를 직접 부르지 않음: 앱은 도시 id("yongin")만 중계 서버에 알리고, 서버가 같은 목록
//! (screenpet_proto::weather::PLACES — 앱과 같은 표)에서 좌표를 찾아 도시마다 30분에 한 번 Open-Meteo에서
//! WMO 날씨 코드를 받아 와 나눠 줌 (`weather` 메시지). 여기는 그 코드를 그림(Weather)으로 바꾸는
//! 순수한 계산뿐임. 받은 값이 없거나 3시간보다 오래됐으면 예전처럼 달력 날씨(`weather::weather_at`)로 돌아감

use serde::{Deserialize, Serialize};

use crate::weather::{self, Weather};

/// 받은 날씨가 이보다 오래되면 (초) 달력 날씨로 돌아감
pub const STALE_SECS: i64 = 3 * 3600;

/// ⚙️ 설정 ▸ 🌦️ 날씨 종류: 실제 날씨 / 계절 날씨 / 직접 고른 날씨 (달력과 상관없이 계속 옴). 모르는 값이면 실제 날씨
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherSource {
    #[default]
    Real,
    Season,
    /// ❄️ 눈
    Snow,
    /// 🌨️ 함박눈 (예전 판은 모르는 값이라 실제 날씨로 돌아감)
    HeavySnow,
    /// 🌧️ 비
    Rain,
    /// 🍂 낙엽
    Leaves,
    /// 🌸 꽃잎
    Petals,
    /// 🌿 푸른 잎
    GreenLeaves,
    /// 🌀 태풍 (돌풍·회오리 — typhoon.rs)
    Typhoon,
}

impl WeatherSource {
    /// 메뉴 순서 (설정·메뉴 id는 id())
    pub const ALL: [WeatherSource; 9] = [
        WeatherSource::Real,
        WeatherSource::Season,
        WeatherSource::Snow,
        WeatherSource::HeavySnow,
        WeatherSource::Rain,
        WeatherSource::Leaves,
        WeatherSource::Petals,
        WeatherSource::GreenLeaves,
        WeatherSource::Typhoon,
    ];
    /// 직접 고르는 날씨들 (메뉴 순서)
    pub const MANUAL: [WeatherSource; 7] =
        [WeatherSource::Snow, WeatherSource::HeavySnow, WeatherSource::Rain, WeatherSource::Leaves, WeatherSource::Petals, WeatherSource::GreenLeaves, WeatherSource::Typhoon];

    /// 실제 날씨를 씀
    pub fn is_real(&self) -> bool {
        *self == WeatherSource::Real
    }

    /// 직접 고른 날씨면 그 날씨 (실제·계절이면 None)
    pub fn manual(self) -> Option<Weather> {
        match self {
            WeatherSource::Real | WeatherSource::Season => None,
            WeatherSource::Snow => Some(Weather::Snow),
            WeatherSource::HeavySnow => Some(Weather::HeavySnow),
            WeatherSource::Rain => Some(Weather::Rain),
            WeatherSource::Leaves => Some(Weather::Leaves),
            WeatherSource::Petals => Some(Weather::Petals),
            WeatherSource::GreenLeaves => Some(Weather::GreenLeaves),
            WeatherSource::Typhoon => Some(Weather::Typhoon),
        }
    }

    /// 설정·메뉴 id (serde 이름과 같음)
    pub fn id(self) -> &'static str {
        match self {
            WeatherSource::Real => "real",
            WeatherSource::Season => "season",
            WeatherSource::Snow => "snow",
            WeatherSource::HeavySnow => "heavy_snow",
            WeatherSource::Rain => "rain",
            WeatherSource::Leaves => "leaves",
            WeatherSource::Petals => "petals",
            WeatherSource::GreenLeaves => "green_leaves",
            WeatherSource::Typhoon => "typhoon",
        }
    }

    /// 메뉴 글자 (☀️ 실제 날씨는 트레이가 도시를 붙임)
    pub fn label(self) -> &'static str {
        match self {
            WeatherSource::Real => "☀️ 실제 날씨",
            WeatherSource::Season => "📅 계절 날씨",
            WeatherSource::Snow => "❄️ 눈",
            WeatherSource::HeavySnow => "🌨️ 함박눈",
            WeatherSource::Rain => "🌧️ 비",
            WeatherSource::Leaves => "🍂 낙엽",
            WeatherSource::Petals => "🌸 꽃잎",
            WeatherSource::GreenLeaves => "🌿 푸른 잎",
            WeatherSource::Typhoon => "🌀 태풍",
        }
    }

    /// id로 찾음 (모르는 id면 None)
    pub fn from_id(raw: &str) -> Option<WeatherSource> {
        WeatherSource::ALL.into_iter().find(|s| s.id() == raw)
    }
}

/// 📍 날씨 지역: 고를 수 있는 도시 (좌표는 시청 근처). 모르는 값이면 용인.
/// 이름·좌표는 screenpet_proto::weather::PLACES에 있음 (같은 순서). 중계 서버는 그 표 밖의 id는 무시함
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Seoul,
    #[default]
    Yongin,
    Suwon,
    Seongnam,
    Incheon,
    Daejeon,
    Daegu,
    Gwangju,
    Busan,
    Ulsan,
    Sejong,
    Jeju,
    Gangneung,
    Chuncheon,
    Jeonju,
    Cheongju,
    Changwon,
    Pohang,
    /// 뉴저지 (뉴어크)
    NewJersey,
    /// 펜실베이니아 (필라델피아)
    Pennsylvania,
    NewYork,
    SanFrancisco,
}

impl Place {
    /// 메뉴 순서
    pub const ALL: [Place; 22] = [
        Place::Seoul,
        Place::Yongin,
        Place::Suwon,
        Place::Seongnam,
        Place::Incheon,
        Place::Daejeon,
        Place::Daegu,
        Place::Gwangju,
        Place::Busan,
        Place::Ulsan,
        Place::Sejong,
        Place::Jeju,
        Place::Gangneung,
        Place::Chuncheon,
        Place::Jeonju,
        Place::Cheongju,
        Place::Changwon,
        Place::Pohang,
        Place::NewJersey,
        Place::Pennsylvania,
        Place::NewYork,
        Place::SanFrancisco,
    ];

    /// 기본 자리(용인)임
    pub fn is_default(&self) -> bool {
        *self == Place::Yongin
    }

    /// 설정·메뉴 id
    pub fn id(self) -> &'static str {
        self.info().0
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        self.info().1
    }

    /// (위도, 경도)
    pub fn coords(self) -> (f64, f64) {
        let (_, _, lat, lon) = self.info();
        (lat, lon)
    }

    /// 공유 지역 표의 같은 줄 (enum 순서 = 표 순서)
    fn info(self) -> (&'static str, &'static str, f64, f64) {
        let p = &screenpet_proto::weather::PLACES[self as usize];
        (p.id, p.name, p.lat, p.lon)
    }

    /// id로 자리를 찾음
    pub fn from_id(raw: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.id() == raw)
    }
}

/// 중계 서버가 나눠 준 날씨 하나: WMO 코드와 서버가 받아 온 시각 (유닉스 초)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    /// WMO 날씨 코드
    pub code: u16,
    /// 서버가 받아 온 시각 (유닉스 초)
    pub at: i64,
}

impl Reading {
    /// 지금(now, 유닉스 초) 믿을 만한가: 3시간 안 (시계가 조금 어긋나 앞날 값이어도 3시간까지는 받음)
    pub fn fresh(&self, now: i64) -> bool {
        (now - self.at).abs() <= STALE_SECS
    }
}

/// WMO 날씨 코드 → 내리는 것. 이슬비·약한 비·약한 소나기는 소나기 그림, 보통 이상의 비·센 소나기·뇌우는 비,
/// 약한 눈·싸락눈·약한 눈 소나기는 가벼운 눈, 보통 눈은 눈, 센 눈·센 눈 소나기는 함박눈. 맑음·구름·안개는 None
pub fn precipitation(code: u16) -> Option<Weather> {
    match code {
        51 | 53 | 55 | 56 | 57 | 61 | 66 | 80 => Some(Weather::Shower),
        63 | 65 | 67 | 81 | 82 | 95 | 96 | 99 => Some(Weather::Rain),
        71 | 77 | 85 => Some(Weather::LightSnow),
        73 => Some(Weather::Snow),
        75 | 86 => Some(Weather::HeavySnow),
        _ => None,
    }
}

/// 뇌우 코드 (WMO 95·96·99) — 중계 서버는 바람 세기를 주지 않아 태풍은 이것으로만 앎
fn is_thunderstorm(code: u16) -> bool {
    matches!(code, 95 | 96 | 99)
}

/// 🌀 태풍 철 (7~9월)
fn typhoon_season(month: u32) -> bool {
    (7..=9).contains(&month)
}

/// 아무것도 안 내릴 때 남기는 계절 장식: 3·4월 벚꽃잎, 5~8월 푸른 잎, 9~11월 낙엽 (가짜 소나기·달력 눈은 없음)
pub fn dry_season(month: u32) -> Option<Weather> {
    match month {
        3 | 4 => Some(Weather::Petals),
        5..=8 => Some(Weather::GreenLeaves),
        9..=11 => Some(Weather::Leaves),
        _ => None,
    }
}

/// 지금 보일 날씨. 직접 고른 날씨면 늘 그 날씨, 실제 날씨를 골랐고 믿을 만한 값이 있으면 그 값 (안 내리면 계절 장식만, 7~9월 뇌우는 🌀 태풍), 아니면 달력 날씨
/// (now: 유닉스 초, 나머지는 지역 날짜와 자정부터 초)
pub fn weather_now(source: WeatherSource, reading: Option<Reading>, now: i64, year: i32, month: u32, day: u32, secs: u32) -> Option<Weather> {
    if let Some(picked) = source.manual() {
        return Some(picked);
    }
    match reading.filter(|r| source.is_real() && r.fresh(now)) {
        Some(r) if is_thunderstorm(r.code) && typhoon_season(month) => Some(Weather::Typhoon),
        Some(r) => precipitation(r.code).or_else(|| dry_season(month)),
        None => weather::weather_at(year, month, day, secs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wmo_codes_map_to_what_falls() {
        for code in [51, 53, 55, 56, 57, 61, 66, 80] {
            assert_eq!(precipitation(code), Some(Weather::Shower), "{code}");
        }
        for code in [63, 65, 67, 81, 82, 95, 96, 99] {
            assert_eq!(precipitation(code), Some(Weather::Rain), "{code}");
        }
        for code in [71, 77, 85] {
            assert_eq!(precipitation(code), Some(Weather::LightSnow), "{code}");
        }
        assert_eq!(precipitation(73), Some(Weather::Snow));
        for code in [75, 86] {
            assert_eq!(precipitation(code), Some(Weather::HeavySnow), "{code}");
        }
        for code in [0, 1, 2, 3, 45, 48, 4, 100, 999] {
            assert_eq!(precipitation(code), None, "{code}");
        }
    }

    const NOW: i64 = 1_800_000_000;

    /// 중계 서버는 WMO 코드만 줌 (바람 세기 없음): 뇌우는 태풍 철(7~9월)에만 태풍, 그 밖엔 예전처럼 비
    #[test]
    fn thunderstorms_are_typhoons_only_in_typhoon_season() {
        for code in [95, 96, 99] {
            for month in [7, 8, 9] {
                let r = Some(Reading { code, at: NOW });
                assert_eq!(weather_now(WeatherSource::Real, r, NOW, 2027, month, 10, 0), Some(Weather::Typhoon), "{code} {month}");
            }
            for month in [1, 4, 6, 10, 12] {
                let r = Some(Reading { code, at: NOW });
                assert_eq!(weather_now(WeatherSource::Real, r, NOW, 2027, month, 10, 0), Some(Weather::Rain), "{code} {month}");
            }
        }
        // 센 비·소나기는 여름에도 비 그대로
        for code in [65, 82] {
            assert_eq!(weather_now(WeatherSource::Real, Some(Reading { code, at: NOW }), NOW, 2027, 8, 10, 0), Some(Weather::Rain));
        }
    }

    #[test]
    fn real_weather_wins_while_fresh() {
        let snow = Some(Reading { code: 73, at: NOW - 600 });
        // 7월 낮에 눈이 와도 실제가 이긴다
        assert_eq!(weather_now(WeatherSource::Real, snow, NOW, 2027, 7, 1, 12 * 3600), Some(Weather::Snow));
        let drizzle = Some(Reading { code: 53, at: NOW });
        // 소나기 그림이지만 달력의 소나기 칸이 아니어도 계속 내린다
        for secs in [0, 3 * 3600, 23 * 3600 + 59 * 60] {
            assert_eq!(weather_now(WeatherSource::Real, drizzle, NOW, 2027, 7, 1, secs), Some(Weather::Shower));
        }
    }

    #[test]
    fn clear_skies_keep_only_petals_and_leaves_and_green_leaves() {
        let clear = Some(Reading { code: 0, at: NOW });
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 4, 2, 0), Some(Weather::Petals));
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 10, 2, 0), Some(Weather::Leaves));
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 11, 2, 0), Some(Weather::Leaves), "no calendar snow");
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 1, 1, 0), None, "no calendar snow");
        for secs in (0..86_400).step_by(600) {
            assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 7, 1, secs), Some(Weather::GreenLeaves), "no fake summer showers, only green leaves");
        }
    }

    #[test]
    fn missing_old_or_unwanted_readings_fall_back_to_the_calendar() {
        let fresh = Some(Reading { code: 0, at: NOW - STALE_SECS });
        let old = Some(Reading { code: 0, at: NOW - STALE_SECS - 1 });
        let future = Some(Reading { code: 0, at: NOW + STALE_SECS + 1 });
        for (day, secs) in [(1, 0), (2, 0), (3, 0), (15, 40_000)] {
            let calendar = weather::weather_at(2027, 1, day, secs);
            assert_eq!(weather_now(WeatherSource::Real, None, NOW, 2027, 1, day, secs), calendar);
            assert_eq!(weather_now(WeatherSource::Real, old, NOW, 2027, 1, day, secs), calendar);
            assert_eq!(weather_now(WeatherSource::Real, future, NOW, 2027, 1, day, secs), calendar);
            assert_eq!(weather_now(WeatherSource::Season, fresh, NOW, 2027, 1, day, secs), calendar);
        }
        assert_eq!(weather_now(WeatherSource::Real, fresh, NOW, 2027, 1, 1, 0), None, "exactly three hours is still fresh");
    }

    #[test]
    fn a_picked_weather_falls_all_year_whatever_the_calendar_or_the_sky_says() {
        let clear = Some(Reading { code: 0, at: NOW });
        let snow = Some(Reading { code: 73, at: NOW });
        for (source, want) in [
            (WeatherSource::Snow, Weather::Snow),
            (WeatherSource::HeavySnow, Weather::HeavySnow),
            (WeatherSource::Rain, Weather::Rain),
            (WeatherSource::Leaves, Weather::Leaves),
            (WeatherSource::Petals, Weather::Petals),
            (WeatherSource::GreenLeaves, Weather::GreenLeaves),
            (WeatherSource::Typhoon, Weather::Typhoon),
        ] {
            assert_eq!(source.manual(), Some(want));
            for month in 1..=12 {
                for secs in [0, 12 * 3600, 86_399] {
                    for reading in [None, clear, snow] {
                        assert_eq!(weather_now(source, reading, NOW, 2027, month, 15, secs), Some(want), "{source:?} {month} {secs}");
                    }
                }
            }
        }
        assert_eq!(WeatherSource::Real.manual(), None);
        assert_eq!(WeatherSource::Season.manual(), None);
    }

    #[test]
    fn weather_sources_have_stable_ids_that_match_the_config_names() {
        let ids: Vec<&str> = WeatherSource::ALL.iter().map(|s| s.id()).collect();
        assert_eq!(ids, ["real", "season", "snow", "heavy_snow", "rain", "leaves", "petals", "green_leaves", "typhoon"]);
        for s in WeatherSource::ALL {
            assert_eq!(WeatherSource::from_id(s.id()), Some(s));
            assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{}\"", s.id()), "config id = menu id");
        }
        let labels: Vec<&str> = WeatherSource::MANUAL.iter().map(|s| s.label()).collect();
        assert_eq!(labels, ["❄️ 눈", "🌨️ 함박눈", "🌧️ 비", "🍂 낙엽", "🌸 꽃잎", "🌿 푸른 잎", "🌀 태풍"]);
        assert_eq!(WeatherSource::from_id("hail"), None);
    }

    #[test]
    fn places_are_the_built_in_korean_cities_with_yongin_first_by_default() {
        assert_eq!(Place::default(), Place::Yongin);
        assert_eq!(Place::Yongin.coords(), (37.2411, 127.1776));
        let labels: Vec<&str> = Place::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(
            labels,
            [
                "서울", "용인", "수원", "성남", "인천", "대전", "대구", "광주", "부산", "울산", "세종", "제주", "강릉", "춘천", "전주", "청주", "창원", "포항", "뉴저지",
                "펜실베이니아", "뉴욕", "샌프란시스코"
            ]
        );
        for p in Place::ALL {
            assert_eq!(Place::from_id(p.id()), Some(p));
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{}\"", p.id()), "config id = menu id");
            let (lat, lon) = p.coords();
            let korea = (33.0..39.0).contains(&lat) && (126.0..130.0).contains(&lon);
            let us = (36.0..42.0).contains(&lat) && (-123.0..-73.0).contains(&lon);
            assert!(korea || us, "{p:?} in Korea or the US");
        }
        assert_eq!(Place::from_id("tokyo"), None);
    }

    /// enum 순서가 공유 표 순서와 같아야 함 (info가 번호로 찾음)
    #[test]
    fn the_enum_follows_the_shared_table() {
        assert_eq!(Place::ALL.len(), screenpet_proto::weather::PLACES.len());
        for (i, p) in Place::ALL.into_iter().enumerate() {
            assert_eq!(p as usize, i);
        }
        assert_eq!(Place::default().id(), screenpet_proto::weather::DEFAULT_PLACE);
    }
}
