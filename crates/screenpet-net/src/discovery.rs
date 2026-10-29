//! LAN discovery: periodic UDP announcements, peer list with expiry, and
//! auto-dial of friends (the smaller id dials, so both sides never race).
//!
//! Announcements are unauthenticated hints. Every address announced for an id
//! is kept as a candidate; a link only counts once the Noise handshake proves
//! the pinned identity. Bounded everywhere: seen ids, candidates per id and
//! per source, failure history, and datagrams per source IP.

use crate::protocol::Announce;
use crate::transport::{self, Candidate};
use crate::{NetEvent, Seen, Shared, State, TICK};
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
                if allow_source(&shared, src.ip()) {
                    if let Some(a) = Announce::parse(&buf[..n]) {
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
    let now = Instant::now();
    let mut st = shared.state();
    if st.src_rate.len() >= MAX_TRACKED && !st.src_rate.contains_key(&ip) {
        st.src_rate.retain(|_, (start, _)| now.duration_since(*start) < RATE_WINDOW);
        if st.src_rate.len() >= MAX_TRACKED {
            return false;
        }
    }
    let entry = st.src_rate.entry(ip).or_insert((now, 0));
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
    if friend && shared.id < a.id && !st.conns.contains_key(&a.id) {
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
pub(crate) fn pick_candidate(st: &State, id: &str) -> Option<Candidate> {
    let seen = st.seen.get(id)?;
    if let Some((c, at)) = seen.verified {
        let failed_since = st.failed.get(&c).is_some_and(|f| *f > at);
        if seen.candidates.contains_key(&c) && !failed_since {
            return Some(c);
        }
    }
    let fresh = seen
        .candidates
        .iter()
        .filter(|(c, _)| !st.failed.contains_key(c))
        .max_by_key(|(_, (t, _))| *t)
        .map(|(c, _)| *c);
    fresh.or_else(|| seen.candidates.keys().min_by_key(|c| st.failed.get(c).copied()).copied())
}

/// Record the outcome of a dial to `cand` for `id`.
pub(crate) fn mark_candidate(shared: &Shared, id: &str, cand: Candidate, ok: bool) {
    let now = Instant::now();
    let mut st = shared.state();
    if ok {
        st.failed.remove(&cand);
        if let Some(seen) = st.seen.get_mut(id) {
            seen.verified = Some((cand, now));
        }
    } else {
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
    let mut st = shared.state();
    st.failed.retain(|_, t| t.elapsed() <= fail_memory);
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
