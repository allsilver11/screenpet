//! ☀️ 날씨 지역 표: 앱 메뉴와 릴레이가 같은 목록을 씀.
//!
//! 앱은 id("yongin")만 릴레이에 보내고, 릴레이는 이 표에서 좌표를 찾아 Open-Meteo에 물어봄.
//! 표 밖의 id는 릴레이가 무시함 (예전 릴레이는 새 도시를 몰라 답이 없고, 앱은 계절 날씨로 돌아감). 순서 = 앱 메뉴 순서.
//! 미국 도시도 있음: 날씨 서비스는 그 자리의 *지금* 날씨를 주므로 시간대와 상관없음 (낮·밤 꼬리표는 릴레이가 뗌)

/// 고를 수 있는 도시 하나 (좌표는 시청 근처, 서경은 음수)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// 설정·와이어 id
    pub id: &'static str,
    /// 메뉴 글자
    pub name: &'static str,
    /// 위도
    pub lat: f64,
    /// 경도
    pub lon: f64,
}

/// 표 한 줄을 만듦
const fn place(id: &'static str, name: &'static str, lat: f64, lon: f64) -> Place {
    Place { id, name, lat, lon }
}

/// 모든 지역 (메뉴 순서)
pub const PLACES: [Place; 22] = [
    place("seoul", "서울", 37.5665, 126.9780),
    place("yongin", "용인", 37.2411, 127.1776),
    place("suwon", "수원", 37.2636, 127.0286),
    place("seongnam", "성남", 37.4200, 127.1265),
    place("incheon", "인천", 37.4563, 126.7052),
    place("daejeon", "대전", 36.3504, 127.3845),
    place("daegu", "대구", 35.8714, 128.6014),
    place("gwangju", "광주", 35.1595, 126.8526),
    place("busan", "부산", 35.1796, 129.0756),
    place("ulsan", "울산", 35.5384, 129.3114),
    place("sejong", "세종", 36.4800, 127.2890),
    place("jeju", "제주", 33.4996, 126.5312),
    place("gangneung", "강릉", 37.7519, 128.8761),
    place("chuncheon", "춘천", 37.8813, 127.7298),
    place("jeonju", "전주", 35.8242, 127.1480),
    place("cheongju", "청주", 36.6424, 127.4890),
    place("changwon", "창원", 35.2280, 128.6811),
    place("pohang", "포항", 36.0190, 129.3435),
    // 미국: 뉴저지는 뉴어크, 펜실베이니아는 필라델피아
    place("new_jersey", "뉴저지", 40.7357, -74.1724),
    place("pennsylvania", "펜실베이니아", 39.9526, -75.1652),
    place("new_york", "뉴욕", 40.7128, -74.0060),
    place("san_francisco", "샌프란시스코", 37.7749, -122.4194),
];

/// 아무것도 고르지 않은 앱의 지역 (경기도 용인시)
pub const DEFAULT_PLACE: &str = "yongin";

/// id가 같은 지역을 찾음. 표에 없으면 None임
pub fn find_place(id: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.id == id)
}

/// 지역 표 무결성 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// id는 겹치지 않고 기본 지역은 표에 있어야 함
    #[test]
    fn ids_are_unique_and_the_default_is_listed() {
        for (i, p) in PLACES.iter().enumerate() {
            assert!(PLACES[..i].iter().all(|q| q.id != p.id), "{}", p.id);
            let korea = (33.0..39.0).contains(&p.lat) && (124.0..132.0).contains(&p.lon);
            let us = (36.0..42.0).contains(&p.lat) && (-123.0..-73.0).contains(&p.lon);
            assert!(korea || us, "{}", p.id);
        }
        assert_eq!(find_place(DEFAULT_PLACE).unwrap().name, "용인");
        assert_eq!(find_place("tokyo"), None);
    }

    /// 미국 도시 넷은 한국 도시 뒤에 있음 (뉴저지는 뉴어크 좌표)
    #[test]
    fn us_cities_follow_the_korean_ones() {
        let tail: Vec<(&str, &str)> = PLACES[PLACES.len() - 4..].iter().map(|p| (p.id, p.name)).collect();
        assert_eq!(tail, [("new_jersey", "뉴저지"), ("pennsylvania", "펜실베이니아"), ("new_york", "뉴욕"), ("san_francisco", "샌프란시스코")]);
        let newark = find_place("new_jersey").unwrap();
        assert!((newark.lat - 40.7357).abs() < 0.01 && (newark.lon + 74.1724).abs() < 0.01);
        let sf = find_place("san_francisco").unwrap();
        assert!(sf.lon < -122.0, "서경은 음수");
    }
}
