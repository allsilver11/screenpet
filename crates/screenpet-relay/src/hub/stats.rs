//! 허브 카운터: `admin status`에 보이는 시작 뒤 누계 (주소·id 없음). 원자 값이라 잠금이 필요 없음.
//! 최근 5분·1시간 창은 `monitor`가 로그 줄로 셈.

use super::*;

/// 카운터 이름 목록으로 `Counters` 구조체와 `all`을 만드는 매크로임
macro_rules! counters {
    ($($name:ident),* $(,)?) => {
        /// 시작 뒤 누계 (`admin status`용). 주소·id 없음
        #[derive(Default)]
        pub(crate) struct Counters {
            $(pub $name: AtomicU64,)*
        }

        impl Counters {
            /// (이름, 값) 목록을 줌
            pub fn all(&self) -> Vec<(&'static str, u64)> {
                vec![$((stringify!($name), self.$name.load(Ordering::Relaxed)),)*]
            }
        }
    };
}

counters!(
    conns_admitted,
    conns_refused_limit,
    conns_refused_origin,
    conns_refused_banned,
    conns_refused_relay_off,
    logins_refused_banned,
    logins_replaced,
    registrations_refused_closed,
    unauthed_evicted,
    threads_refused,
    accept_errors,
    auth_failed,
    registrations,
    registrations_refused,
    store_full,
    kicked_rate,
    kicked_slow,
    redeems_ok,
    redeems_failed,
    channels_closed_rate,
    channels_closed_queue,
    frames_forwarded,
    bytes_forwarded,
    proxy_without_address,
    save_errors,
);

/// 카운터 하나를 1 올림
pub(crate) fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

impl Hub {
    /// `admin status`: 스위치, 연결 수, 접속 중인 설치, 카운터
    pub(crate) fn admin_status(&self) -> String {
        let st = self.state();
        let switch = |on| if on { "on" } else { "off" };
        let c = &st.control;
        let mut out = format!(
            "relay {}, registration {}, friendcode {}\nonline {}, connections {} ({} unauthenticated), stored {}, channels {}, grants {}, banned keys {}, banned addresses {}\n",
            switch(c.relay),
            switch(c.registration),
            switch(c.friendcode),
            st.peers.len(),
            st.all_conns.total,
            st.all_conns.unauthed,
            st.store_ref().count(),
            st.channels.len(),
            st.grants.len(),
            c.banned_keys.len(),
            c.banned_ips.len(),
        );
        let now = unix_now();
        let mut peers: Vec<(&String, &Peer)> = st.peers.iter().collect();
        peers.sort_by_key(|(_, p)| p.since);
        for (key, p) in peers {
            let friends = st.store_ref().get(key).map_or(0, |i| i.friends.len());
            let up = now.saturating_sub(p.since);
            out += &format!(
                "  {} {:?} since {} ({}h{:02}m) from {} friends {} ({} mutual)\n",
                &key[..12],
                p.id,
                p.since,
                up / 3600,
                up % 3600 / 60,
                p.ip,
                friends,
                st.store_ref().mutual_count(key)
            );
        }
        out += "counters:\n";
        for (name, value) in self.counters.all() {
            out += &format!("  {name} {value}\n");
        }
        out += &crate::monitor::global().status_text(now);
        out
    }
}
