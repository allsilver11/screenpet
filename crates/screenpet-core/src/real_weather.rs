//! ☀️ 실제 날씨 (v0.6.3) — 고른 도시(기본 용인)의 지금 날씨를 계절 날씨 대신 보여 준다.
//!
//! 앱은 날씨 서비스를 직접 부르지 않는다: 앱은 도시 id("yongin")만 중계 서버에 알리고, 서버가 같은 목록
//! (screenpet-relay weather::PLACES — 테스트가 같은지 본다)에서 좌표를 찾아 도시마다 30분에 한 번 Open-Meteo에서
//! WMO 날씨 코드를 받아 와 나눠 준다 (`weather` 메시지). 여기는 그 코드를 그림(Weather)으로 바꾸는
//! 순수한 계산뿐이다. 받은 값이 없거나 3시간보다 오래됐으면 예전처럼 달력 날씨(`weather::weather_at`)로 돌아간다.

use serde::{Deserialize, Serialize};

use crate::weather::{self, Weather};

/// 받은 날씨가 이보다 오래되면 (초) 달력 날씨로 돌아간다
pub const STALE_SECS: i64 = 3 * 3600;

/// ⚙️ 설정 ▸ 날씨: 실제 날씨 / 계절 날씨. 모르는 값이면 실제 날씨
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeatherSource {
    #[default]
    Real,
    Season,
}

impl WeatherSource {
    pub fn is_real(&self) -> bool {
        *self == WeatherSource::Real
    }
}

/// 📍 날씨 지역: 고를 수 있는 도시 (좌표는 시청 근처). 모르는 값이면 용인.
/// 바꾸면 screenpet-relay의 weather::PLACES도 같이 바꾼다 (중계 서버가 이 목록 밖의 id는 무시한다)
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
}

impl Place {
    /// 메뉴 순서
    pub const ALL: [Place; 18] = [
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
    ];

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

    fn info(self) -> (&'static str, &'static str, f64, f64) {
        match self {
            Place::Seoul => ("seoul", "서울", 37.5665, 126.9780),
            Place::Yongin => ("yongin", "용인", 37.2411, 127.1776),
            Place::Suwon => ("suwon", "수원", 37.2636, 127.0286),
            Place::Seongnam => ("seongnam", "성남", 37.4200, 127.1265),
            Place::Incheon => ("incheon", "인천", 37.4563, 126.7052),
            Place::Daejeon => ("daejeon", "대전", 36.3504, 127.3845),
            Place::Daegu => ("daegu", "대구", 35.8714, 128.6014),
            Place::Gwangju => ("gwangju", "광주", 35.1595, 126.8526),
            Place::Busan => ("busan", "부산", 35.1796, 129.0756),
            Place::Ulsan => ("ulsan", "울산", 35.5384, 129.3114),
            Place::Sejong => ("sejong", "세종", 36.4800, 127.2890),
            Place::Jeju => ("jeju", "제주", 33.4996, 126.5312),
            Place::Gangneung => ("gangneung", "강릉", 37.7519, 128.8761),
            Place::Chuncheon => ("chuncheon", "춘천", 37.8813, 127.7298),
            Place::Jeonju => ("jeonju", "전주", 35.8242, 127.1480),
            Place::Cheongju => ("cheongju", "청주", 36.6424, 127.4890),
            Place::Changwon => ("changwon", "창원", 35.2280, 128.6811),
            Place::Pohang => ("pohang", "포항", 36.0190, 129.3435),
        }
    }

    pub fn from_id(raw: &str) -> Option<Place> {
        Place::ALL.into_iter().find(|p| p.id() == raw)
    }
}

/// 중계 서버가 나눠 준 날씨 하나: WMO 코드와 서버가 받아 온 시각 (유닉스 초)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub code: u16,
    pub at: i64,
}

impl Reading {
    /// 지금(now, 유닉스 초) 믿을 만한가: 3시간 안 (시계가 조금 어긋나 앞날 값이어도 3시간까지는 받는다)
    pub fn fresh(&self, now: i64) -> bool {
        (now - self.at).abs() <= STALE_SECS
    }
}

/// WMO 날씨 코드 → 내리는 것. 이슬비·약한 비·약한 소나기는 소나기 그림, 보통 이상의 비·센 소나기·뇌우는 비,
/// 약한 눈·싸락눈·약한 눈 소나기는 가벼운 눈, 보통 이상의 눈은 눈. 맑음·구름·안개는 None
pub fn precipitation(code: u16) -> Option<Weather> {
    match code {
        51 | 53 | 55 | 56 | 57 | 61 | 66 | 80 => Some(Weather::Shower),
        63 | 65 | 67 | 81 | 82 | 95 | 96 | 99 => Some(Weather::Rain),
        71 | 77 | 85 => Some(Weather::LightSnow),
        73 | 75 | 86 => Some(Weather::Snow),
        _ => None,
    }
}

/// 아무것도 안 내릴 때 남기는 계절 장식: 3·4월 벚꽃잎, 9~11월 낙엽 (가짜 소나기·달력 눈은 없다)
pub fn dry_season(month: u32) -> Option<Weather> {
    match month {
        3 | 4 => Some(Weather::Petals),
        9..=11 => Some(Weather::Leaves),
        _ => None,
    }
}

/// 지금 보일 날씨. 실제 날씨를 골랐고 믿을 만한 값이 있으면 그 값 (안 내리면 계절 장식만), 아니면 달력 날씨
/// (now: 유닉스 초, 나머지는 지역 날짜와 자정부터 초)
pub fn weather_now(source: WeatherSource, reading: Option<Reading>, now: i64, year: i32, month: u32, day: u32, secs: u32) -> Option<Weather> {
    match reading.filter(|r| source.is_real() && r.fresh(now)) {
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
        for code in [73, 75, 86] {
            assert_eq!(precipitation(code), Some(Weather::Snow), "{code}");
        }
        for code in [0, 1, 2, 3, 45, 48, 4, 100, 999] {
            assert_eq!(precipitation(code), None, "{code}");
        }
    }

    const NOW: i64 = 1_800_000_000;

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
    fn clear_skies_keep_only_petals_and_leaves() {
        let clear = Some(Reading { code: 0, at: NOW });
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 4, 2, 0), Some(Weather::Petals));
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 10, 2, 0), Some(Weather::Leaves));
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 11, 2, 0), Some(Weather::Leaves), "no calendar snow");
        assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 1, 1, 0), None, "no calendar snow");
        for secs in (0..86_400).step_by(600) {
            assert_eq!(weather_now(WeatherSource::Real, clear, NOW, 2027, 7, 1, secs), None, "no fake summer showers");
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
    fn places_are_the_built_in_korean_cities_with_yongin_first_by_default() {
        assert_eq!(Place::default(), Place::Yongin);
        assert_eq!(Place::Yongin.coords(), (37.2411, 127.1776));
        let labels: Vec<&str> = Place::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(
            labels,
            ["서울", "용인", "수원", "성남", "인천", "대전", "대구", "광주", "부산", "울산", "세종", "제주", "강릉", "춘천", "전주", "청주", "창원", "포항"]
        );
        for p in Place::ALL {
            assert_eq!(Place::from_id(p.id()), Some(p));
            assert_eq!(serde_json::to_string(&p).unwrap(), format!("\"{}\"", p.id()), "config id = menu id");
            let (lat, lon) = p.coords();
            assert!((33.0..39.0).contains(&lat) && (126.0..130.0).contains(&lon), "{p:?} in Korea");
        }
        assert_eq!(Place::from_id("tokyo"), None);
    }
}
