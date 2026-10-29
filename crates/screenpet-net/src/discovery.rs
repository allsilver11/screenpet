//! LAN discovery: periodic UDP announcements, peer list with expiry, and
//! auto-dial of friends (both sides dial; `peers::register` keeps one link).
//!
//! Announcements are unauthenticated hints. Every address announced for an id
//! is kept as a candidate; a link only counts once the Noise handshake proves
//! the pinned identity. Bounded everywhere: seen ids, candidates per id and
//! per source, failure history, and datagrams per source IP.

use crate::protocol::Announce;
use crate::transport::{self, Candidate};
use crate::peers::DialOutcome;
use crate::{Backoff, NetEvent, Seen, Shared, State, TICK};
use std::collections::HashMap;
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
const RATE_WINDOW: Duration = Duration::from_secs(1);
/// Bound on the rate table and the failure history.
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
                    // Friends' announcements are always processed (identity is
                    // still verified by Noise on connect); others are rate-limited.
                    let friend = shared.state().is_friend(&a.id);
                    if allow_source(&shared, src.ip()) || friend {
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

/// Per-source-IP datagram rate limit.
fn allow_source(shared: &Shared, ip: IpAddr) -> bool {
    rate_allow(&mut shared.state().src_rate, ip, Instant::now())
}

/// Count a datagram from `ip`; `false` once it exceeds its window budget.
/// The table is bounded: when full of live entries (e.g. spoofed sources) the
/// oldest entry is evicted rather than refusing every new source.
fn rate_allow(table: &mut HashMap<IpAddr, (Instant, u32)>, ip: IpAddr, now: Instant) -> bool {
    if table.len() >= MAX_TRACKED && !table.contains_key(&ip) {
        table.retain(|_, (start, _)| now.duration_since(*start) < RATE_WINDOW);
        if table.len() >= MAX_TRACKED {
            let oldest = table.iter().min_by_key(|(_, (start, _))| *start).map(|(ip, _)| *ip);
            if let Some(o) = oldest {
                table.remove(&o);
            }
        }
    }
    let entry = table.entry(ip).or_insert((now, 0));
    if now.duration_since(entry.0) >= RATE_WINDOW {
        *entry = (now, 0);
    }
    entry.1 += 1;
    entry.1 <= MAX_DATAGRAMS_PER_WINDOW
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
    let seen = st
        .seen
        .entry(a.id.clone())
        .or_insert_with(|| Seen { candidates: HashMap::new(), verified: None, last: now });
    seen.last = now;
    add_candidate(seen, cand, src.ip(), now);
    if is_new {
        shared.emit(NetEvent::PeerSeen { id: a.id.clone(), name: a.name });
    }
    // Both sides dial friends; duplicate links are resolved in `register`.
    if friend && !st.friend_links.contains_key(&a.id) && !st.backed_off(&Backoff::Id(a.id.clone()), now) {
        let target = pick_candidate(&st, &a.id);
        if let Some(target) = target {
            if st.dialing.insert(a.id.clone()) {
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

/// Best candidate to dial for `id`: the verified one if it has not failed
/// since, else the most recently announced one without a recent failure,
/// else the one whose failure is oldest.
/// Candidates in redial backoff are skipped entirely.
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
    let fresh = seen
        .candidates
        .iter()
        .filter(|(c, _)| !st.failed.contains_key(c) && usable(c))
        .max_by_key(|(_, (t, _))| *t)
        .map(|(c, _)| *c);
    fresh.or_else(|| {
        seen.candidates
            .keys()
            .filter(|c| usable(c))
            .min_by_key(|c| st.failed.get(c).copied())
            .copied()
    })
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
        if seen.candidates.is_empty() {
            gone.push(id.clone());
        }
    }
    for id in gone {
        st.seen.remove(&id);
        shared.emit(NetEvent::PeerGone { id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn ip(n: u32) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n))
    }

    #[test]
    fn per_source_budget() {
        let mut t = HashMap::new();
        let now = Instant::now();
        for _ in 0..MAX_DATAGRAMS_PER_WINDOW {
            assert!(rate_allow(&mut t, ip(1), now));
        }
        assert!(!rate_allow(&mut t, ip(1), now));
        assert!(rate_allow(&mut t, ip(2), now), "other sources unaffected");
        assert!(rate_allow(&mut t, ip(1), now + RATE_WINDOW), "budget refills");
    }

    #[test]
    fn full_table_evicts_instead_of_failing_closed() {
        let mut t = HashMap::new();
        let now = Instant::now();
        for n in 0..MAX_TRACKED as u32 {
            assert!(rate_allow(&mut t, ip(1000 + n), now));
        }
        // Every entry is live; a new (e.g. real) source must still get in.
        assert!(rate_allow(&mut t, ip(1), now));
        assert!(t.len() <= MAX_TRACKED);
    }
}
