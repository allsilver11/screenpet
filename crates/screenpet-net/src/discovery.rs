//! LAN 찾기임: 주기적 UDP 알림, 만료되는 친구 후보 목록, 친구 자동 다이얼
//! (양쪽 다 다이얼하고 `peers::register`가 링크 하나만 남김).
//!
//! 알림은 인증 없는 힌트일 뿐임. id마다 알려진 주소는 모두 후보로 두고,
//! Noise 핸드셰이크가 고정된 신원을 증명해야 링크로 침. 모든 것에 상한이 있음:
//! 본 id, id·출처별 후보, 실패 기록, 출처 IP별 데이터그램, 미검증 후보로의 다이얼.
//! 진입점은 `run`(lib의 discovery 스레드)이고 다이얼은 `transport::dial`로 넘김.

use crate::peers::DialOutcome;
use crate::protocol::Announce;
use crate::transport::{self, Candidate};
use crate::{Backoff, NetEvent, Seen, Shared, State, MAX_UNVERIFIED_DIALS_PER_MIN, TICK};
use std::collections::HashMap;
use std::hash::Hash;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 한 번에 추적하는 id 수 상한 (친구는 늘 받아 줌)
pub(crate) const MAX_SEEN: usize = 64;
/// id당 후보 주소 수 상한
const MAX_CANDIDATES: usize = 8;
/// 한 id 안에서 알린 출처 IP당 후보 수 상한
const MAX_CANDIDATES_PER_SOURCE: usize = 2;
/// 출처 IP당 창 하나에 받는 데이터그램 수
const MAX_DATAGRAMS_PER_WINDOW: u32 = 50;
/// 친구 id를 주장하는 알림의 따로 둔 예산, (id, 출처 IP)마다
const MAX_FRIEND_CLAIMS_PER_WINDOW: u32 = 4;
/// 그 친구가 그 IP에서 인증한 뒤의 더 큰(하지만 유한한) (id, 출처 IP)당 예산
const MAX_TRUSTED_PER_WINDOW: u32 = 200;
/// 데이터그램 속도 제한 창
const RATE_WINDOW: Duration = Duration::from_secs(1);
/// 미검증 다이얼 상한을 세는 창
const DIAL_WINDOW: Duration = Duration::from_secs(60);
/// 속도 표와 실패 기록의 크기 상한
const MAX_TRACKED: usize = 256;

/// discovery 스레드 본체임: 알림 보내기·듣기와 만료·자동 다이얼 타이머를 돌림.
/// `udp`가 None이면 포트를 못 잡은 것이라 알리지도 듣지도 않지만 타이머는 돎 (릴레이 링크에도 씀)
pub(crate) fn run(shared: Arc<Shared>, udp: Option<UdpSocket>, broadcast_to: SocketAddr) {
    let Some(udp) = udp else {
        while !shared.stopping() {
            std::thread::sleep(TICK);
            crate::peers::flush_gone(&shared);
            crate::devices::expire_pending(&shared);
            crate::devices::expire_confirms(&shared);
            crate::devices::tick_active(&shared);
        }
        return;
    };
    let mut next_announce = Instant::now();
    let mut buf = [0u8; 2048];
    while !shared.stopping() {
        if Instant::now() >= next_announce {
            // 이름은 바뀔 수 있음 (Net::set_name) — 알릴 때마다 만듦
            let announce = serde_json::to_vec(&Announce::new(&shared.id, &shared.name(), shared.tcp_port)).expect("announce serializes");
            let _ = udp.send_to(&announce, broadcast_to);
            next_announce = Instant::now() + shared.timing.announce_interval;
        }
        match udp.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Some(a) = Announce::parse(&buf[..n]).filter(|_| local_source(src.ip())) {
                    let admitted = admit_announcement(&mut shared.state(), &a.id, src.ip(), Instant::now());
                    if admitted {
                        heard(&shared, a, src);
                    }
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
            // e.g. Windows reports ICMP port-unreachable as ConnectionReset.
            Err(_) => std::thread::sleep(TICK),
        }
        expire(&shared);
        crate::peers::flush_gone(&shared);
        crate::devices::expire_pending(&shared);
        crate::devices::expire_confirms(&shared);
        crate::devices::tick_active(&shared);
    }
}

/// 이 출처가 알림을 보내도 되는지 봄. 찾기는 한 로컬 네트워크용이라
/// 바깥(포워딩된 포트)에서 온 데이터그램은 다이얼 대상이 되면 안 됨
pub(crate) fn local_source(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_loopback(),
        // ::1, fc00::/7 (unique local), fe80::/10 (link local)
        IpAddr::V6(v6) => v6.is_loopback() || v6.segments()[0] & 0xfe00 == 0xfc00 || v6.segments()[0] & 0xffc0 == 0xfe80,
    }
}

/// 알림 하나의 속도 제한임. 출처 IP 예산 안이거나, 이 IP에서 인증했던 친구 id면(더 큰 유한 예산),
/// 또는 친구 id에 대해 (id, 출처) 쌍의 작은 예산 안이면 받음.
/// 친구 IP 예산을 같이 쓰는 폭주가 친구를 굶기지 못하고, 친구 id를 *주장*만 해서는 몇 개만 통함
pub(crate) fn admit_announcement(st: &mut State, id: &str, ip: IpAddr, now: Instant) -> bool {
    if rate_allow(&mut st.src_rate, ip, now, MAX_DATAGRAMS_PER_WINDOW) {
        return true;
    }
    if st.is_trusted(id, &ip.to_string())
        && rate_allow(&mut st.trusted_rate, (id.to_string(), ip), now, MAX_TRUSTED_PER_WINDOW)
    {
        return true;
    }
    st.is_friend(id) && rate_allow(&mut st.friend_rate, (id.to_string(), ip), now, MAX_FRIEND_CLAIMS_PER_WINDOW)
}

/// `key`에 사건 하나를 셈. 창마다 `limit`를 넘으면 `false`임.
/// 표는 상한이 있어서 살아 있는 항목(예: 위조 출처)으로 꽉 차면 새 키를 막는 대신 가장 오래된 것을 밀어냄
pub(crate) fn rate_allow<K: Hash + Eq + Clone>(table: &mut HashMap<K, (Instant, u32)>, key: K, now: Instant, limit: u32) -> bool {
    if table.len() >= MAX_TRACKED && !table.contains_key(&key) {
        table.retain(|_, (start, _)| now.duration_since(*start) < RATE_WINDOW);
        if table.len() >= MAX_TRACKED {
            let oldest = table.iter().min_by_key(|(_, (start, _))| *start).map(|(k, _)| k.clone());
            if let Some(o) = oldest {
                table.remove(&o);
            }
        }
    }
    let entry = table.entry(key).or_insert((now, 0));
    if now.duration_since(entry.0) >= RATE_WINDOW {
        *entry = (now, 0);
    }
    entry.1 += 1;
    entry.1 <= limit
}

/// 받은 알림 하나를 처리함: 후보를 기록하고 친구면 다이얼할지 정함
fn heard(shared: &Arc<Shared>, a: Announce, src: SocketAddr) {
    if a.id == shared.id {
        return;
    }
    let now = Instant::now();
    let cand = Candidate::Lan(SocketAddr::new(src.ip(), a.port));
    let mut st = shared.state();
    let friend = st.is_friend(&a.id);
    let is_new = !st.seen.contains_key(&a.id);
    if is_new && st.seen.len() >= MAX_SEEN {
        // Make room only for a friend, by dropping the stalest non-friend.
        let victim = if friend {
            st.seen.iter().filter(|(id, _)| !st.is_friend(id)).min_by_key(|(_, s)| s.last).map(|(id, _)| id.clone())
        } else {
            None
        };
        match victim {
            Some(v) => {
                st.seen.remove(&v);
                shared.emit(NetEvent::PeerGone { id: v });
            }
            None => return,
        }
    }
    let seen = st.seen.entry(a.id.clone()).or_insert_with(|| Seen {
        candidates: HashMap::new(),
        verified: None,
        tried: HashMap::new(),
        unverified_dials: HashMap::new(),
        last: now,
    });
    seen.last = now;
    add_candidate(seen, cand, src.ip(), now);
    if is_new {
        shared.emit(NetEvent::PeerSeen { id: a.id.clone(), name: a.name });
    }
    // Both sides dial friends: the smaller id at once, the larger one only
    // after a grace period without a link (a fallback, e.g. when the smaller
    // side's dials are being starved). Rare duplicates are resolved in
    // `register`.
    let unlinked_since = if friend && !st.friend_links.contains_key(&a.id) {
        Some(*st.friend_wait.entry(a.id.clone()).or_insert(now))
    } else {
        None
    };
    let grace = shared.timing.announce_interval * 3;
    let my_turn = shared.id < a.id || unlinked_since.is_some_and(|t| now.duration_since(t) >= grace);
    // 📱 On standby only my linked computer is dialed
    if st.standby_blocks(&a.id) {
        return;
    }
    if unlinked_since.is_some()
        && my_turn
        && !st.dialing.contains(&a.id)
        && !st.backed_off(&Backoff::Id(a.id.clone()), now)
    {
        if let Some(target) = next_dial(&mut st, &a.id, now) {
            drop(st);
            transport::dial(shared, a.id, target);
        }
    } else if friend && shared.id < a.id && st.relay_linked(&a.id) {
        // LAN preferred: a friend linked only through the relay is redialed
        // here by the smaller id; once its identity is proven the LAN link
        // replaces the relay link (`peers::register`). `dialing` is not
        // checked: it stays set while the relay link we dialed is alive.
        // Throttled instead, so a LAN that carries announcements but not TCP
        // costs little.
        let every = shared.timing.peer_timeout * 2;
        if st.lan_upgrade.get(&a.id).is_some_and(|t| now.duration_since(*t) < every) {
            return;
        }
        if let Some(target) = next_dial(&mut st, &a.id, now) {
            let friends = st.friends.clone();
            st.lan_upgrade.retain(|id, _| friends.iter().any(|f| &f.id == id));
            st.lan_upgrade.insert(a.id.clone(), now);
            drop(st);
            transport::dial(shared, a.id, target);
        }
    }
}

/// 지금 친구에게 다이얼할 후보를 고르고 예산이 허락하면 다이얼 중으로 표시함
fn next_dial(st: &mut State, id: &str, now: Instant) -> Option<Candidate> {
    let target = pick_candidate(st, id)?;
    note_dial(st, id, target, now, true).then(|| {
        st.dialing.insert(id.to_string());
        target
    })
}

/// 출처별·id별 상한 안에서 후보를 넣거나 새로 고침. 검증된 후보는 밀어내지 않음
fn add_candidate(seen: &mut Seen, cand: Candidate, src: IpAddr, now: Instant) {
    if let Some(entry) = seen.candidates.get_mut(&cand) {
        *entry = (now, src);
        return;
    }
    let verified = seen.verified.map(|(c, _)| c);
    let evict = |seen: &mut Seen, from_src: Option<IpAddr>| {
        let victim = seen
            .candidates
            .iter()
            .filter(|(c, (_, s))| Some(**c) != verified && from_src.is_none_or(|f| *s == f))
            .min_by_key(|(_, (t, _))| *t)
            .map(|(c, _)| *c);
        match victim {
            Some(v) => {
                seen.candidates.remove(&v);
                seen.tried.remove(&v);
                true
            }
            None => false,
        }
    };
    let same_src = seen.candidates.values().filter(|(_, s)| *s == src).count();
    if same_src >= MAX_CANDIDATES_PER_SOURCE && !evict(seen, Some(src)) {
        return;
    }
    if seen.candidates.len() >= MAX_CANDIDATES && !evict(seen, None) {
        return;
    }
    seen.candidates.insert(cand, (now, src));
}

/// `id`에 다이얼할 후보를 고름: 검증된 것이 그 뒤로 실패하지 않았으면 그것, 아니면 라운드 로빈
/// (가장 오래전에 다이얼한 것, 안 해 본 것 먼저)이라 쓰레기 후보가 다이얼을 독차지 못함.
/// 재다이얼 대기 중인 후보는 건너뜀
pub(crate) fn pick_candidate(st: &State, id: &str) -> Option<Candidate> {
    let now = Instant::now();
    let seen = st.seen.get(id)?;
    let usable = |c: &Candidate| !st.backed_off(&Backoff::Cand(*c), now);
    if let Some((c, at)) = seen.verified {
        let failed_since = st.failed.get(&c).is_some_and(|f| *f > at);
        if seen.candidates.contains_key(&c) && !failed_since && usable(&c) {
            return Some(c);
        }
    }
    seen.candidates
        .iter()
        .filter(|(c, _)| usable(c))
        .min_by_key(|(c, (announced, _))| (seen.tried.get(c).copied(), std::cmp::Reverse(*announced)))
        .map(|(c, _)| *c)
}

/// 라운드 로빈용으로 `cand`로의 다이얼을 기록함. `capped`면 미검증 후보 다이얼을 id·알린 출처·분당으로
/// 제한함(친구 id를 위조한 출처는 자기 예산만 씀). 넘으면 `false`. 어쨌든 차례는 지나가 다음 후보가 나옴
pub(crate) fn note_dial(st: &mut State, id: &str, cand: Candidate, now: Instant, capped: bool) -> bool {
    let Some(seen) = st.seen.get_mut(id) else { return true };
    seen.tried.insert(cand, now);
    if seen.verified.is_some_and(|(c, _)| c == cand) {
        return true;
    }
    let Some(&(_, source)) = seen.candidates.get(&cand) else { return true };
    seen.unverified_dials.retain(|_, dials| {
        dials.retain(|t| now.duration_since(*t) < DIAL_WINDOW);
        !dials.is_empty()
    });
    if seen.unverified_dials.len() >= MAX_TRACKED && !seen.unverified_dials.contains_key(&source) {
        let oldest = seen.unverified_dials.iter().min_by_key(|(_, d)| d.back().copied()).map(|(s, _)| *s);
        if let Some(o) = oldest {
            seen.unverified_dials.remove(&o);
        }
    }
    let dials = seen.unverified_dials.entry(source).or_default();
    if capped && dials.len() >= MAX_UNVERIFIED_DIALS_PER_MIN {
        return false;
    }
    dials.push_back(now);
    true
}

/// `id`의 `cand`로 한 다이얼 결과를 기록함
pub(crate) fn mark_candidate(shared: &Shared, id: &str, cand: Candidate, outcome: DialOutcome) {
    let now = Instant::now();
    let mut st = shared.state();
    if outcome == DialOutcome::Proven {
        st.failed.remove(&cand);
        st.backoff.remove(&Backoff::Cand(cand));
        if let Some(seen) = st.seen.get_mut(id) {
            seen.verified = Some((cand, now));
        }
    } else {
        if outcome == DialOutcome::WrongKey {
            st.back_off(Backoff::Cand(cand), shared.timing.peer_timeout, now);
        }
        if st.failed.len() >= MAX_TRACKED {
            let oldest = st.failed.iter().min_by_key(|(_, t)| **t).map(|(c, _)| *c);
            if let Some(o) = oldest {
                st.failed.remove(&o);
            }
        }
        st.failed.insert(cand, now);
    }
}

/// 오래 안 들린 후보·실패 기록·대기를 지움
fn expire(shared: &Shared) {
    let timeout = shared.timing.peer_timeout;
    let fail_memory = timeout * 10;
    let now = Instant::now();
    let mut st = shared.state();
    st.failed.retain(|_, t| t.elapsed() <= fail_memory);
    // Forget a backoff once it has been over for a full MAX_BACKOFF.
    st.backoff.retain(|_, (until, _)| now < *until + crate::MAX_BACKOFF);
    let mut gone = Vec::new();
    for (id, seen) in st.seen.iter_mut() {
        seen.candidates.retain(|_, (t, _)| t.elapsed() <= timeout);
        let live = &seen.candidates;
        seen.tried.retain(|c, _| live.contains_key(c));
        if seen.candidates.is_empty() {
            gone.push(id.clone());
        }
    }
    for id in gone {
        st.seen.remove(&id);
        st.friend_wait.remove(&id);
        shared.emit(NetEvent::PeerGone { id });
    }
}

/// 속도 제한·후보 라운드 로빈·다이얼 예산 시험임
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Friend;
    use std::net::Ipv4Addr;

    fn ip(n: u32) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n))
    }

    fn lan(n: u16) -> Candidate {
        Candidate::Lan(SocketAddr::from(([10, 0, 0, 1], n)))
    }

    fn with_friend() -> State {
        let mut st = State::default();
        st.friends.push(Friend { id: "ff".into(), public_key: [7; 32] });
        st
    }

    #[test]
    fn only_addresses_on_a_local_network_may_announce() {
        for good in ["10.1.2.3", "172.16.0.9", "172.31.255.1", "192.168.1.5", "169.254.10.10", "127.0.0.1"] {
            assert!(local_source(good.parse().unwrap()), "{good}");
        }
        for bad in ["8.8.8.8", "172.32.0.1", "192.169.0.1", "100.64.0.1", "0.0.0.0", "255.255.255.255", "224.0.0.1"] {
            assert!(!local_source(bad.parse().unwrap()), "{bad}");
        }
        for v6 in ["::1", "fe80::1", "fd00::1"] {
            assert!(local_source(v6.parse().unwrap()), "{v6}");
        }
        assert!(!local_source("2001:4860:4860::8888".parse().unwrap()));
    }

    /// 한 id에 서로 다른 출처가 알린 후보들이 있는 상태를 만듦 (검증된 것 없음)
    fn two_sources() -> State {
        let now = Instant::now();
        let mut st = State::default();
        let mut seen = Seen {
            candidates: HashMap::new(),
            verified: None,
            tried: HashMap::new(),
            unverified_dials: HashMap::new(),
            last: now,
        };
        seen.candidates.insert(lan(1), (now, ip(1)));
        seen.candidates.insert(lan(2), (now, ip(2)));
        st.seen.insert("x".into(), seen);
        st
    }

    #[test]
    fn a_source_burning_its_dial_budget_does_not_spend_anothers() {
        let mut st = two_sources();
        let now = Instant::now();
        let allowed = (0..100).filter(|_| note_dial(&mut st, "x", lan(1), now, true)).count();
        assert_eq!(allowed, MAX_UNVERIFIED_DIALS_PER_MIN);
        assert!(note_dial(&mut st, "x", lan(2), now, true), "the other source still gets dialed");
    }

    #[test]
    fn a_candidate_over_budget_loses_its_turn_in_the_round_robin() {
        let mut st = two_sources();
        let now = Instant::now();
        while note_dial(&mut st, "x", lan(1), now, true) {}
        // 예산이 바닥난 후보가 차례를 붙들고 있으면 다른 후보는 영영 불리지 않음
        let t = now + Duration::from_millis(1);
        st.seen.get_mut("x").unwrap().tried.insert(lan(2), now);
        assert!(!note_dial(&mut st, "x", lan(1), t, true));
        assert_eq!(pick_candidate(&st, "x"), Some(lan(2)), "next time the other one is picked");
    }

    #[test]
    fn dial_bookkeeping_stays_bounded_under_spoofed_sources() {
        let mut st = two_sources();
        let now = Instant::now();
        for n in 0..(MAX_TRACKED as u32 * 2) {
            let c = lan(1000 + n as u16);
            let seen = st.seen.get_mut("x").unwrap();
            seen.candidates.clear();
            seen.candidates.insert(c, (now, ip(5000 + n)));
            note_dial(&mut st, "x", c, now, true);
        }
        assert!(st.seen["x"].unverified_dials.len() <= MAX_TRACKED);
    }

    #[test]
    fn per_source_budget() {
        let mut t = HashMap::new();
        let now = Instant::now();
        for _ in 0..MAX_DATAGRAMS_PER_WINDOW {
            assert!(rate_allow(&mut t, ip(1), now, MAX_DATAGRAMS_PER_WINDOW));
        }
        assert!(!rate_allow(&mut t, ip(1), now, MAX_DATAGRAMS_PER_WINDOW));
        assert!(rate_allow(&mut t, ip(2), now, MAX_DATAGRAMS_PER_WINDOW), "other sources unaffected");
        assert!(rate_allow(&mut t, ip(1), now + RATE_WINDOW, MAX_DATAGRAMS_PER_WINDOW), "budget refills");
    }

    #[test]
    fn full_table_evicts_instead_of_failing_closed() {
        let mut t = HashMap::new();
        let now = Instant::now();
        for n in 0..MAX_TRACKED as u32 {
            assert!(rate_allow(&mut t, ip(1000 + n), now, MAX_DATAGRAMS_PER_WINDOW));
        }
        // Every entry is live; a new (e.g. real) source must still get in.
        assert!(rate_allow(&mut t, ip(1), now, MAX_DATAGRAMS_PER_WINDOW));
        assert!(t.len() <= MAX_TRACKED);
    }

    #[test]
    fn claimed_friend_id_buys_only_a_small_budget() {
        let mut st = with_friend();
        let now = Instant::now();
        // Exhaust the source's general budget.
        while admit_announcement(&mut st, "junk", ip(1), now) {}
        // Claiming the friend id is not an exemption: only a few get through.
        let extra = (0..100).filter(|_| admit_announcement(&mut st, "ff", ip(1), now)).count();
        assert_eq!(extra, MAX_FRIEND_CLAIMS_PER_WINDOW as usize);
        // Non-friend claims from the flooded source stay blocked.
        assert!(!admit_announcement(&mut st, "other", ip(1), now));
        // The friend's budget is per source: another source still gets in.
        assert!(admit_announcement(&mut st, "ff", ip(2), now));
    }

    #[test]
    fn trusted_ip_is_keyed_by_friend_id_and_finite() {
        let mut st = with_friend();
        st.friends.push(Friend { id: "gg".into(), public_key: [8; 32] });
        let now = Instant::now();
        st.trust_ip("ff", &ip(1).to_string(), now);
        while admit_announcement(&mut st, "junk", ip(1), now) {}
        // Only the friend that authenticated from this IP gets the big budget.
        let ff = (0..1000).filter(|_| admit_announcement(&mut st, "ff", ip(1), now)).count();
        assert_eq!(ff, MAX_TRUSTED_PER_WINDOW as usize + MAX_FRIEND_CLAIMS_PER_WINDOW as usize);
        // Other claimed ids (even other friends) from that IP get no exemption.
        assert!(!admit_announcement(&mut st, "anything", ip(1), now));
        let gg = (0..100).filter(|_| admit_announcement(&mut st, "gg", ip(1), now)).count();
        assert_eq!(gg, MAX_FRIEND_CLAIMS_PER_WINDOW as usize);
        // The budget refills next window.
        assert!(admit_announcement(&mut st, "ff", ip(1), now + RATE_WINDOW));
    }

    #[test]
    fn round_robin_prefers_verified_then_least_recently_tried() {
        let mut st = State::default();
        let now = Instant::now();
        let mut seen = Seen {
            candidates: HashMap::new(),
            verified: None,
            tried: HashMap::new(),
            unverified_dials: HashMap::new(),
            last: now,
        };
        for n in 1..=3 {
            seen.candidates.insert(lan(n), (now, ip(n as u32)));
        }
        st.seen.insert("x".into(), seen);
        let mut order = Vec::new();
        for _ in 0..6 {
            let c = pick_candidate(&st, "x").unwrap();
            order.push(c);
            assert!(note_dial(&mut st, "x", c, Instant::now(), false));
        }
        // Every candidate is tried once before any repeats.
        let first: std::collections::HashSet<_> = order[..3].iter().collect();
        assert_eq!(first.len(), 3);
        assert_eq!(order[3], order[0]);
        // A verified candidate wins outright.
        st.seen.get_mut("x").unwrap().verified = Some((lan(2), Instant::now()));
        assert_eq!(pick_candidate(&st, "x"), Some(lan(2)));
    }

    #[test]
    fn unverified_dials_are_capped_per_minute() {
        let mut st = State::default();
        let now = Instant::now();
        let mut seen = Seen {
            candidates: HashMap::new(),
            verified: Some((lan(9), now)),
            tried: HashMap::new(),
            unverified_dials: HashMap::new(),
            last: now,
        };
        seen.candidates.insert(lan(1), (now, ip(1)));
        seen.candidates.insert(lan(9), (now, ip(9)));
        st.seen.insert("x".into(), seen);
        let allowed = (0..100).filter(|_| note_dial(&mut st, "x", lan(1), now, true)).count();
        assert_eq!(allowed, MAX_UNVERIFIED_DIALS_PER_MIN);
        assert!(note_dial(&mut st, "x", lan(9), now, true), "verified candidate is never capped");
        assert!(note_dial(&mut st, "x", lan(1), now + DIAL_WINDOW, true), "window slides");
    }
}
