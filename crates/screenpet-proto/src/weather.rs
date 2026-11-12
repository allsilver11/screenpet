//! ☀️ 날씨 지역 표: 앱 메뉴와 릴레이가 같은 목록을 씀.
//!
//! 앱은 id("yongin")만 릴레이에 보내고, 릴레이는 이 표에서 좌표를 찾아 Open-Meteo에 물어봄.
//! 표 밖의 id는 릴레이가 무시함. 순서 = 앱 메뉴 순서.

/// 고를 수 있는 도시 하나 (좌표는 시청 근처)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// 설정·와이어 id
    pub id: &'static str,
    /// 메뉴 글자
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
}

/// 표 한 줄을 만듦
const fn place(id: &'static str, name: &'static str, lat: f64, lon: f64) -> Place {
    Place { id, name, lat, lon }
}

/// 모든 지역 (메뉴 순서)
pub const PLACES: [Place; 18] = [
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
];

/// 아무것도 고르지 않은 앱의 지역 (경기도 용인시)
pub const DEFAULT_PLACE: &str = "yongin";

/// id가 같은 지역을 찾음. 표에 없으면 None임
pub fn find_place(id: &str) -> Option<&'static Place> {
    PLACES.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// id는 겹치지 않고 기본 지역은 표에 있어야 함
    #[test]
    fn ids_are_unique_and_the_default_is_listed() {
        for (i, p) in PLACES.iter().enumerate() {
            assert!(PLACES[..i].iter().all(|q| q.id != p.id), "{}", p.id);
            assert!((33.0..39.0).contains(&p.lat) && (124.0..132.0).contains(&p.lon), "{}", p.id);
        }
        assert_eq!(find_place(DEFAULT_PLACE).unwrap().name, "용인");
        assert_eq!(find_place("tokyo"), None);
    }
}
