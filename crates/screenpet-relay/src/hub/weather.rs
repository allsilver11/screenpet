//! ☀️ 날씨 연결부: 앱마다 원하는 지역을 기억하고, 받아 온 날씨를 그 지역 앱에 나눠 줌.
//!
//! 받아 오기 자체(HTTP)는 잠금 밖 `weather_loop`에서 함. 여기는 메모리 일만 함.

use super::*;

/// 날씨 한 번을 와이어 메시지로 만듦
pub(super) fn weather_msg(place: &str, r: Reading) -> ServerMsg {
    ServerMsg::Weather { place: place.to_string(), code: r.code, at: r.at as i64 }
}

impl Hub {
    /// ☀️ 앱이 원하는 지역을 바꿈. 아는 날씨가 있으면 바로 줌. 모르는 id면 아무것도 안 바뀌고 None임
    pub(crate) fn weather_want(&self, key: &str, conn: u64, place: &str) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let place = st.weathers.want(place, Instant::now())?;
        st.peers.get_mut(key).expect("current").weather = place;
        st.weathers.reading(place).map(|r| weather_msg(place, r))
    }

    /// ☀️ 지금 받아 올 지역들 (`Weathers::due`)
    pub(crate) fn weather_due(&self) -> Vec<&'static crate::weather::Place> {
        let mut st = self.state();
        let in_use: HashSet<&'static str> = st.peers.values().map(|p| p.weather).collect();
        st.weathers.due(Instant::now(), &in_use)
    }

    /// ☀️ 받아 오기가 끝남. 새 날씨를 그 지역 앱 모두에게 보냄
    pub(crate) fn weather_fetched(&self, place: &str, code: Option<u16>) {
        let mut st = self.state();
        let Some(r) = st.weathers.fetched(place, code, Instant::now(), unix_now()) else { return };
        let m = weather_msg(place, r);
        for p in st.peers.values().filter(|p| p.weather == place) {
            p.send(m.clone());
        }
    }
}
