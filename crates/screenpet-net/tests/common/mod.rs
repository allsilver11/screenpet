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
    }
}

pub fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
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
