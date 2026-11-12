//! 통합 시험들이 같이 쓰는 도우미임 (시험 바이너리마다 일부만 씀).
//! 빠른 시간 상수, 안 겹치는 포트, 이벤트 기다리기, hex·BLAKE2s 도우미를 줌.
#![allow(dead_code)]

use blake2::{Blake2s256, Digest};
use screenpet_net::{Friend, Net, NetEvent, Timing};
use std::net::UdpSocket;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// 시험용으로 짧게 줄인 시간 상수를 돌려줌
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

/// 이 시험 프로세스 안에서 아무도 안 가져갈 Net 찾기용 UDP 포트를 줌: 시험이 나란히 돌고 시작 전에 서로의 포트가 필요해서
/// OS가 고를 수 없음 (포트 0).
///
/// OS에 빈 포트를 받고 닫는 방식은 경쟁이 생김: OS가 다음 `bind(…:0)`(다른 시험의 Net이나 소켓)에 같은 포트를 주고,
/// 옛 포트로 Net을 다시 켜는 시험도 같은 식으로 잃음. 그래서 포트는 모든 OS의 임시 포트 범위
/// (Linux 32768+, Windows/macOS 49152+) 아래에서 가져와 포트 0 bind가 닿지 않게 하고, 프로세스당 한 번씩만 줌.
/// 다른 프로그램이 쥔 포트는 건너뜀
pub fn free_udp_port() -> u16 {
    // The Net binds 0.0.0.0; free there means free on 127.0.0.1 too.
    reserved_port(|port| UdpSocket::bind(("0.0.0.0", port)).is_ok())
}

/// 옛 포트로 다시 켜는 TCP 리스너(릴레이)용으로 같은 일을 함
pub fn free_tcp_port() -> u16 {
    reserved_port(|port| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok())
}

/// 범위 안에서 아직 안 준 포트 중 `free`가 비었다고 하는 첫 포트를 줌
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

/// 키 `key`의 공개키로 친구 `id`를 만듦
pub fn friend(id: &str, key: &[u8; 32]) -> Friend {
    Friend { id: id.into(), public_key: Net::public_key(key) }
}

/// `pred`에 맞는 이벤트가 올 때까지 기다림 (다른 것은 건너뜀). `secs` 안에 안 오면 None임
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

/// `Connected` 이벤트인지 봄
pub fn is_connected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Connected { .. })
}

/// `Disconnected` 이벤트인지 봄
pub fn is_disconnected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Disconnected { .. })
}

/// 바이트를 hex로 바꿈
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// hex를 바이트로 바꿈
pub fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// 조각들의 BLAKE2s-256 해시를 돌려줌
pub fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}
