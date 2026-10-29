//! LAN discovery: periodic UDP announcements, peer list with expiry, and
//! auto-dial of friends (the smaller id dials, so both sides never race).
//!
//! Announcements are unauthenticated hints. Every address announced for an id
//! is kept as a candidate; a dial only counts once the peer proves the id
//! (exact id match + HMAC for friends, see `peers`), and a candidate that
//! fails is avoided in favour of the others.

use crate::protocol::Announce;
use crate::{tcp, NetEvent, Seen, Shared, TICK};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Instant;

/// Most candidate addresses remembered per id (oldest dropped first).
const MAX_CANDIDATES: usize = 8;

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
                    heard(&shared, a, src);
                }
            }
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted) => {}
            // e.g. Windows reports ICMP port-unreachable as ConnectionReset.
            Err(_) => std::thread::sleep(TICK),
        }
        expire(&shared);
    }
}

fn heard(shared: &Arc<Shared>, a: Announce, src: SocketAddr) {
    if a.id == shared.id || a.id.len() > 128 {
        return;
    }
    let addr = SocketAddr::new(src.ip(), a.port);
    let mut st = shared.state();
    let is_new = !st.seen.contains_key(&a.id);
    let seen = st.seen.entry(a.id.clone()).or_insert_with(|| Seen {
        candidates: HashMap::new(),
        bad: HashMap::new(),
    });
    seen.candidates.insert(addr, Instant::now());
    if seen.candidates.len() > MAX_CANDIDATES {
        let oldest = seen.candidates.iter().min_by_key(|(_, t)| **t).map(|(a, _)| *a);
        if let Some(o) = oldest {
            seen.candidates.remove(&o);
            seen.bad.remove(&o);
        }
    }
    if is_new {
        shared.emit(NetEvent::PeerSeen { id: a.id.clone(), name: a.name });
    }
    if st.is_friend(&a.id) && shared.id < a.id && !st.conns.contains_key(&a.id) {
        if let Some(target) = pick_candidate(&st.seen[&a.id]) {
            if st.dialing.insert(a.id.clone()) {
                drop(st);
                tcp::dial(shared, a.id, target);
            }
        }
    }
}

/// Best address to dial: a candidate without a recent failure (most recently
/// announced first), else the one whose failure is oldest.
pub(crate) fn pick_candidate(seen: &Seen) -> Option<SocketAddr> {
    let fresh = seen
        .candidates
        .iter()
        .filter(|(a, _)| !seen.bad.contains_key(a))
        .max_by_key(|(_, t)| **t)
        .map(|(a, _)| *a);
    fresh.or_else(|| {
        seen.candidates
            .keys()
            .min_by_key(|a| seen.bad.get(a).copied())
            .copied()
    })
}

/// Record the outcome of a dial to `addr` for `id`.
pub(crate) fn mark_candidate(shared: &Shared, id: &str, addr: SocketAddr, ok: bool) {
    let mut st = shared.state();
    if let Some(seen) = st.seen.get_mut(id) {
        if ok {
            seen.bad.remove(&addr);
        } else {
            seen.bad.insert(addr, Instant::now());
        }
    }
}

fn expire(shared: &Shared) {
    let timeout = shared.timing.peer_timeout;
    let mut st = shared.state();
    let mut gone = Vec::new();
    for (id, seen) in st.seen.iter_mut() {
        seen.candidates.retain(|_, t| t.elapsed() <= timeout);
        let live = &seen.candidates;
        seen.bad.retain(|a, _| live.contains_key(a));
        if seen.candidates.is_empty() {
            gone.push(id.clone());
        }
    }
    for id in gone {
        st.seen.remove(&id);
        shared.emit(NetEvent::PeerGone { id });
    }
}
