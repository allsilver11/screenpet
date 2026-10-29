//! LAN discovery: periodic UDP announcements, peer list with expiry, and
//! auto-dial of friends (the smaller id dials, so both sides never race).

use crate::protocol::Announce;
use crate::{tcp, NetEvent, Seen, Shared, TICK};
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::Instant;

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
    if a.id == shared.id {
        return;
    }
    let addr = SocketAddr::new(src.ip(), a.port);
    let mut st = shared.state();
    let changed = st.seen.get(&a.id).is_none_or(|s| s.name != a.name);
    st.seen.insert(a.id.clone(), Seen { name: a.name.clone(), addr, last: Instant::now() });
    if changed {
        shared.emit(NetEvent::PeerSeen { id: a.id.clone(), name: a.name });
    }
    if st.is_friend(&a.id) && shared.id < a.id && !st.conns.contains_key(&a.id) && st.dialing.insert(a.id.clone()) {
        drop(st);
        tcp::dial(shared, a.id, addr);
    }
}

fn expire(shared: &Shared) {
    let timeout = shared.timing.peer_timeout;
    let mut st = shared.state();
    let gone: Vec<String> =
        st.seen.iter().filter(|(_, s)| s.last.elapsed() > timeout).map(|(id, _)| id.clone()).collect();
    for id in gone {
        st.seen.remove(&id);
        shared.emit(NetEvent::PeerGone { id });
    }
}
