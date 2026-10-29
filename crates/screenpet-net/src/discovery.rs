//! LAN discovery: periodic UDP announcements, peer list with expiry, and
//! auto-dial of friends (both sides dial; `peers::register` keeps one link).
//!
//! Announcements are unauthenticated hints. Every address announced for an id
//! is kept as a candidate; a link only counts once the Noise handshake proves
//! the pinned identity. Bounded everywhere: seen ids, candidates per id and
//! per source, failure history, datagrams per source IP, and dials to
//! not-yet-verified candidates.

use crate::peers::DialOutcome;
use crate::protocol::Announce;
use crate::transport::{self, Candidate};
use crate::{Backoff, NetEvent, Seen, Shared, State, MAX_UNVERIFIED_DIALS_PER_MIN, TICK};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Most ids tracked at once (friends are always admitted).
pub(crate) const MAX_SEEN: usize = 64;
/// Most candidate addresses per id, and per announcing source IP within it.
const MAX_CANDIDATES: usize = 8;
const MAX_CANDIDATES_PER_SOURCE: usize = 2;
/// Most datagrams accepted per source IP per rate window.
const MAX_DATAGRAMS_PER_WINDOW: u32 = 50;
/// Separate budget for announcements claiming a friend id, per (id, source IP).
const MAX_FRIEND_CLAIMS_PER_WINDOW: u32 = 4;
const RATE_WINDOW: Duration = Duration::from_secs(1);
const DIAL_WINDOW: Duration = Duration::from_secs(60);
/// Bound on the rate tables and the failure history.
const MAX_TRACKED: usize = 256;

pub(crate) fn run(shared: Arc<Shared>, udp: UdpSocket, broadcast_to: SocketAddr) {
    let announce = serde_json::to_vec(&Announce::new(&shared.id, &shared.name, shared.tcp_port))
        .expect("announce serializes");
    let mut next_announce = Instant::now();
    let mut buf = [0u8; 2048];
    while !shared.stopping() {
        if Instant::now() >= next_announce {
            let _ = udp.send_to(&announce, broadcast_to);
            next_announce = Instant::now() + shared.timing.announce_interval;
        }
        match udp.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Some(a) = Announce::parse(&buf[..n]) {
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
    }
}

/// Rate limiting for one announcement. Admitted if the source IP is within
/// its budget, or the source previously authenticated as a friend, or (for
/// an id that is a friend) the (id, source) pair is within its own small
/// budget — so a flood sharing the friend's IP budget cannot starve the
/// friend, while a *claimed* friend id alone buys only a few datagrams.
pub(crate) fn admit_announcement(st: &mut State, id: &str, ip: IpAddr, now: Instant) -> bool {
    if rate_allow(&mut st.src_rate, ip, now, MAX_DATAGRAMS_PER_WINDOW) {
        return true;
    }
    if st.trusted_ips.contains_key(&ip.to_string()) {
        return true;
    }
    st.is_friend(id) && rate_allow(&mut st.friend_rate, (id.to_string(), ip), now, MAX_FRIEND_CLAIMS_PER_WINDOW)
}

/// Count one event for `key`; `false` once it exceeds `limit` per window.
/// The table is bounded: when full of live entries (e.g. spoofed sources) the
/// oldest entry is evicted rather than refusing every new key.
fn rate_allow<K: Hash + Eq + Clone>(table: &mut HashMap<K, (Instant, u32)>, key: K, now: Instant, limit: u32) -> bool {
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
        unverified_dials: VecDeque::new(),
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
    if unlinked_since.is_some()
        && my_turn
        && !st.dialing.contains(&a.id)
        && !st.backed_off(&Backoff::Id(a.id.clone()), now)
    {
        if let Some(target) = pick_candidate(&st, &a.id) {
            if note_dial(&mut st, &a.id, target, now, true) {
                st.dialing.insert(a.id.clone());
                drop(st);
                transport::dial(shared, a.id, target);
            }
        }
    }
}

/// Insert/refresh a candidate within the per-source and per-id caps, never
/// evicting the verified candidate.
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

/// Candidate to dial for `id`: the verified one if it has not failed since;
/// otherwise round-robin — the candidate dialed least recently (never-dialed
/// first), so junk candidates cannot monopolize dialing. Candidates in
/// redial backoff are skipped.
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

/// Record a dial to `cand` for round-robin. Dials to non-verified candidates
/// are capped per id per minute when `capped`; returns `false` if over the cap.
pub(crate) fn note_dial(st: &mut State, id: &str, cand: Candidate, now: Instant, capped: bool) -> bool {
    let Some(seen) = st.seen.get_mut(id) else { return true };
    let verified = seen.verified.is_some_and(|(c, _)| c == cand);
    if !verified {
        seen.unverified_dials.retain(|t| now.duration_since(*t) < DIAL_WINDOW);
        if capped && seen.unverified_dials.len() >= MAX_UNVERIFIED_DIALS_PER_MIN {
            return false;
        }
        seen.unverified_dials.push_back(now);
    }
    seen.tried.insert(cand, now);
    true
}

/// Record the outcome of a dial to `cand` for `id`.
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
    fn previously_authenticated_sources_are_exempt() {
        let mut st = with_friend();
        let now = Instant::now();
        st.trust_ip(&ip(1).to_string(), now);
        assert!((0..500).all(|_| admit_announcement(&mut st, "anything", ip(1), now)));
    }

    #[test]
    fn round_robin_prefers_verified_then_least_recently_tried() {
        let mut st = State::default();
        let now = Instant::now();
        let mut seen = Seen {
            candidates: HashMap::new(),
            verified: None,
            tried: HashMap::new(),
            unverified_dials: VecDeque::new(),
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
            unverified_dials: VecDeque::new(),
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
