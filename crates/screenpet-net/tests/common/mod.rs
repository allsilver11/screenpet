//! Shared by the integration tests (each test binary uses only part of it).
#![allow(dead_code)]

use blake2::{Blake2s256, Digest};
use screenpet_net::{Friend, Net, NetEvent, Timing};
use std::net::UdpSocket;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub fn fast() -> Timing {
    Timing {
        announce_interval: Duration::from_millis(150),
        peer_timeout: Duration::from_millis(1500),
        ping_interval: Duration::from_millis(200),
        conn_timeout: Duration::from_millis(1500),
        invite_timeout: Duration::from_millis(3000),
        relay_retry: Duration::from_millis(300),
        device_confirm_timeout: Duration::from_millis(2500),
        standby_grace: Duration::from_millis(300),
        device_state_every: Duration::from_millis(300),
        device_state_refresh: Duration::from_millis(1500),
    }
}

/// A UDP port for a Net's discovery that nothing else in this test process
/// will take: the tests run in parallel and need each other's ports before
/// starting, so the OS can't pick them (port 0).
///
/// Asking the OS for a free port and closing it again is racy: the OS hands
/// that same port to the next `bind(…:0)` (another test's Net or socket), and
/// a test that restarts a Net on its old port loses it the same way. So ports
/// come from below every OS's ephemeral range (Linux 32768+, Windows/macOS
/// 49152+), where port-0 binds never land, and each is handed out at most
/// once per process. A port another program holds is skipped.
pub fn free_udp_port() -> u16 {
    // The Net binds 0.0.0.0; free there means free on 127.0.0.1 too.
    reserved_port(|port| UdpSocket::bind(("0.0.0.0", port)).is_ok())
}

/// The same for a TCP listener that is restarted on its old port (a relay).
pub fn free_tcp_port() -> u16 {
    reserved_port(|port| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok())
}

fn reserved_port(free: impl Fn(u16) -> bool) -> u16 {
    use std::sync::atomic::{AtomicU32, Ordering};
    const FIRST: u32 = 20000;
    const COUNT: u32 = 12000;
    // A per-process start, so two test processes at once rarely meet.
    static NEXT: AtomicU32 = AtomicU32::new(u32::MAX);
    let _ = NEXT.compare_exchange(u32::MAX, std::process::id().wrapping_mul(7919) % COUNT, Ordering::SeqCst, Ordering::SeqCst);
    for _ in 0..COUNT {
        let port = (FIRST + NEXT.fetch_add(1, Ordering::SeqCst) % COUNT) as u16;
        if free(port) {
            return port;
        }
    }
    panic!("no free port in {FIRST}..{}", FIRST + COUNT);
}

pub fn friend(id: &str, key: &[u8; 32]) -> Friend {
    Friend { id: id.into(), public_key: Net::public_key(key) }
}

/// Wait until an event matching `pred` arrives (skipping others).
pub fn wait_for(rx: &Receiver<NetEvent>, secs: f64, pred: impl Fn(&NetEvent) -> bool) -> Option<NetEvent> {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        match rx.recv_timeout(left) {
            Ok(ev) if pred(&ev) => return Some(ev),
            Ok(_) => continue,
            Err(_) => return None,
        }
    }
}

pub fn is_connected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Connected { .. })
}

pub fn is_disconnected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Disconnected { .. })
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

pub fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}
