//! Localhost integration tests: two Net instances on distinct discovery ports,
//! each "broadcasting" (UDP unicast) to the other's port, plus raw TCP clients
//! for the defensive-parsing cases.

use hmac::{Hmac, Mac};
use screenpet_net::{new_install_id, Friend, Net, NetConfig, NetEvent, Side, Timing, Visit};
use sha2::Sha256;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fast() -> Timing {
    Timing {
        announce_interval: Duration::from_millis(100),
        peer_timeout: Duration::from_millis(600),
        ping_interval: Duration::from_millis(100),
        conn_timeout: Duration::from_millis(700),
        invite_timeout: Duration::from_millis(1500),
    }
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

/// Shared secret used for pre-configured friendships in these tests.
const SECRET: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const WRONG: &str = "2222222222222222222222222222222222222222222222222222222222222222";
const NONCE: &str = "3333333333333333333333333333333333333333333333333333333333333333";

fn friends(ids: &[&str]) -> Vec<Friend> {
    ids.iter().map(|id| Friend { id: id.to_string(), secret: SECRET.into() }).collect()
}

fn cfg(id: &str, name: &str, port: u16, to: u16, fr: &[&str]) -> NetConfig {
    cfg_with(id, name, port, to, friends(fr))
}

fn cfg_with(id: &str, name: &str, port: u16, to: u16, friends: Vec<Friend>) -> NetConfig {
    NetConfig {
        id: id.into(),
        name: name.into(),
        discovery_port: port,
        broadcast_to: local(to),
        friends,
        timing: fast(),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

/// Independent re-implementation of the documented auth response:
/// HMAC-SHA256(secret, "screenpet-auth-v1\0" || nonce || len|own_id || len|peer_id).
fn auth_mac(secret: &str, nonce: &str, own: &str, peer: &str) -> String {
    let mut m = Hmac::<Sha256>::new_from_slice(&unhex(secret)).unwrap();
    m.update(b"screenpet-auth-v1\0");
    m.update(&unhex(nonce));
    for id in [own, peer] {
        m.update(&(id.len() as u32).to_be_bytes());
        m.update(id.as_bytes());
    }
    hex(&m.finalize().into_bytes())
}

/// Wait until an event matching `pred` arrives (skipping others).
fn wait_for(rx: &Receiver<NetEvent>, secs: f64, pred: impl Fn(&NetEvent) -> bool) -> Option<NetEvent> {
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

/// Two instances "a" < "b" pointed at each other.
fn pair(a_friends: &[&str], b_friends: &[&str]) -> (u16, u16, (Net, Receiver<NetEvent>), (Net, Receiver<NetEvent>)) {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let a = Net::start(cfg("aaaa", "Ann", pa, pb, a_friends)).unwrap();
    let b = Net::start(cfg("bbbb", "Bob", pb, pa, b_friends)).unwrap();
    (pa, pb, a, b)
}

fn sample_visit() -> Visit {
    Visit::Visit {
        visit_id: 42,
        species: "cat".into(),
        stage: "child".into(),
        scale: 1.25,
        personality: "playful".into(),
        owner: "Ann".into(),
        enter_side: Side::Left,
    }
}

#[test]
fn install_id_is_32_hex_and_random() {
    let a = new_install_id();
    assert_eq!(a.len(), 32);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, new_install_id());
}

#[test]
fn default_timing_matches_spec() {
    let t = Timing::default();
    assert_eq!(t.announce_interval, Duration::from_secs(2));
    assert_eq!(t.peer_timeout, Duration::from_secs(6));
    assert_eq!(t.ping_interval, Duration::from_secs(2));
    assert_eq!(t.conn_timeout, Duration::from_secs(6));
    assert_eq!(t.invite_timeout, Duration::from_secs(20));
}

#[test]
fn discovery_sees_peer_and_notices_it_gone() {
    let (_, _, (a, arx), (b, brx)) = pair(&[], &[]);
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })),
        Some(NetEvent::PeerSeen { id: "bbbb".into(), name: "Bob".into() })
    );
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })),
        Some(NetEvent::PeerSeen { id: "aaaa".into(), name: "Ann".into() })
    );
    b.shutdown();
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::PeerGone { .. })),
        Some(NetEvent::PeerGone { id: "bbbb".into() })
    );
    a.shutdown();
}

#[test]
fn own_announcement_is_ignored() {
    let p = free_udp_port();
    // Broadcasting to itself: must never report itself.
    let (n, rx) = Net::start(cfg("self", "Me", p, p, &[])).unwrap();
    assert_eq!(wait_for(&rx, 0.6, |e| matches!(e, NetEvent::PeerSeen { .. })), None);
    n.shutdown();
}

#[test]
fn invite_accepted_makes_friends_and_connects() {
    let (_, _, (a, arx), (b, brx)) = pair(&[], &[]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    a.invite("bbbb", "cat");
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Invited { .. })),
        Some(NetEvent::Invited { from: "aaaa".into(), name: "Ann".into(), pet: "cat".into() })
    );
    b.answer_invite("aaaa", true);
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "bbbb".into(), accepted: true })
    );
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() })
    );
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })),
        Some(NetEvent::Connected { id: "aaaa".into(), name: "Ann".into() })
    );
    let (fa, fb) = (a.friends(), b.friends());
    assert_eq!(fa.len(), 1);
    assert_eq!(fa[0].id, "bbbb");
    assert_eq!(fb.len(), 1);
    assert_eq!(fb[0].id, "aaaa");
    // Same fresh 32-byte secret on both sides.
    assert_eq!(fa[0].secret, fb[0].secret);
    assert_eq!(fa[0].secret.len(), 64);
    assert_ne!(fa[0].secret, SECRET);
    // Connection stays up across several ping intervals.
    assert_eq!(wait_for(&arx, 1.0, |e| matches!(e, NetEvent::Disconnected { .. })), None);
    a.shutdown();
    b.shutdown();
}

#[test]
fn invite_declined_leaves_no_friendship() {
    let (_, _, (a, arx), (b, brx)) = pair(&[], &[]);
    // The larger id invites here, so the dial does not depend on id order.
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    b.invite("aaaa", "bear");
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Invited { .. })).unwrap();
    a.answer_invite("bbbb", false);
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "aaaa".into(), accepted: false })
    );
    assert!(a.friends().is_empty());
    assert!(b.friends().is_empty());
    assert_eq!(wait_for(&arx, 0.5, |e| matches!(e, NetEvent::Connected { .. })), None);
    assert_eq!(wait_for(&brx, 0.1, |e| matches!(e, NetEvent::Connected { .. })), None);
    a.shutdown();
    b.shutdown();
}

#[test]
fn unanswered_invite_times_out_as_declined() {
    let (_, _, (a, arx), (b, brx)) = pair(&[], &[]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    a.invite("bbbb", "cat");
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Invited { .. })).unwrap();
    assert_eq!(
        wait_for(&arx, 4.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "bbbb".into(), accepted: false })
    );
    assert!(a.friends().is_empty());
    a.shutdown();
    b.shutdown();
}

#[test]
fn friends_auto_reconnect_after_restart() {
    let (pa, pb, (a, arx), (b, brx)) = pair(&["bbbb"], &["aaaa"]);
    assert!(wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).is_some());
    assert!(wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).is_some());
    a.shutdown();
    assert_eq!(
        wait_for(&brx, 6.0, |e| matches!(e, NetEvent::Disconnected { .. })),
        Some(NetEvent::Disconnected { id: "aaaa".into() })
    );
    let (a2, arx2) = Net::start(cfg("aaaa", "Ann", pa, pb, &["bbbb"])).unwrap();
    assert_eq!(
        wait_for(&arx2, 3.0, |e| matches!(e, NetEvent::Connected { .. })),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() })
    );
    assert!(wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).is_some());
    // Restart the larger id too (it never dials; the smaller one must).
    b.shutdown();
    wait_for(&arx2, 6.0, |e| matches!(e, NetEvent::Disconnected { .. })).unwrap();
    let (b2, brx2) = Net::start(cfg("bbbb", "Bob", pb, pa, &["aaaa"])).unwrap();
    assert!(wait_for(&brx2, 3.0, |e| matches!(e, NetEvent::Connected { .. })).is_some());
    a2.shutdown();
    b2.shutdown();
}

#[test]
fn visit_messages_round_trip() {
    let (_, _, (a, arx), (b, brx)) = pair(&["bbbb"], &["aaaa"]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    a.send("bbbb", sample_visit());
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "aaaa".into(), msg: sample_visit() })
    );
    b.send("aaaa", Visit::Leave { visit_id: 42 });
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "bbbb".into(), msg: Visit::Leave { visit_id: 42 } })
    );
    a.send("bbbb", Visit::ComeHome { visit_id: 42 });
    assert_eq!(
        wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "aaaa".into(), msg: Visit::ComeHome { visit_id: 42 } })
    );
    a.shutdown();
    b.shutdown();
}

#[test]
fn peer_shutdown_is_noticed_within_six_seconds() {
    let (_, _, (a, arx), (b, brx)) = pair(&["bbbb"], &["aaaa"]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    b.shutdown();
    assert_eq!(
        wait_for(&arx, 6.0, |e| matches!(e, NetEvent::Disconnected { .. })),
        Some(NetEvent::Disconnected { id: "bbbb".into() })
    );
    a.shutdown();
}

#[test]
fn unfriend_disconnects_and_forgets_on_both_sides() {
    let (_, _, (a, arx), (b, brx)) = pair(&["bbbb"], &["aaaa"]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    b.unfriend("aaaa");
    assert_eq!(
        wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Disconnected { .. })),
        Some(NetEvent::Disconnected { id: "bbbb".into() })
    );
    assert!(wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    assert!(a.friends().is_empty());
    assert!(b.friends().is_empty());
    // No automatic reconnection afterwards.
    assert_eq!(wait_for(&arx, 0.8, |e| matches!(e, NetEvent::Connected { .. })), None);
    a.shutdown();
    b.shutdown();
}

// ---- raw-client tests ----------------------------------------------------

/// Start a Net ("mmmm") that "broadcasts" to a raw UDP socket and return the
/// TCP port learned from its announcement.
fn start_sniffed(net_friends: Vec<Friend>) -> (Net, Receiver<NetEvent>, u16) {
    let sniffer = UdpSocket::bind("127.0.0.1:0").unwrap();
    sniffer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let to = sniffer.local_addr().unwrap().port();
    let (net, rx) = Net::start(cfg_with("mmmm", "Mia", free_udp_port(), to, net_friends)).unwrap();
    let mut buf = [0u8; 2048];
    let (n, _) = sniffer.recv_from(&mut buf).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&buf[..n]).unwrap();
    assert_eq!(v["app"], "screenpet");
    assert_eq!(v["id"], "mmmm");
    (net, rx, v["port"].as_u64().unwrap() as u16)
}

/// Raw TCP client claiming `id`: says hello, reads Mia's hello, and if
/// `secret` is given answers Mia's challenge with it.
fn raw_client(
    net_friends: &[&str],
    id: &str,
    secret: Option<&str>,
) -> (Net, Receiver<NetEvent>, TcpStream, BufReader<TcpStream>) {
    let (net, rx, port) = start_sniffed(friends(net_friends));
    let mut s = TcpStream::connect(local(port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    writeln!(s, r#"{{"type":"hello","id":"{id}","name":"Raw","v":1,"nonce":"{NONCE}"}}"#).unwrap();
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let hello: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["id"], "mmmm");
    if let Some(sec) = secret {
        let mac = auth_mac(sec, hello["nonce"].as_str().unwrap(), id, "mmmm");
        writeln!(s, r#"{{"type":"auth","mac":"{mac}"}}"#).unwrap();
    }
    (net, rx, s, r)
}

/// Raw friend "zzzz" with the right secret.
fn raw_friend() -> (Net, Receiver<NetEvent>, TcpStream, BufReader<TcpStream>) {
    raw_client(&["zzzz"], "zzzz", Some(SECRET))
}

/// True once the server side has closed the socket.
fn closed_by_peer(s: &mut TcpStream) -> bool {
    let mut buf = [0u8; 4096];
    loop {
        match s.read(&mut buf) {
            Ok(0) => return true,
            Ok(_) => continue, // pings / hello
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => return true,
            Err(_) => return false,
        }
    }
}

#[test]
fn raw_friend_with_secret_authenticates_and_gets_pings() {
    let (net, rx, _s, mut r) = raw_friend();
    assert_eq!(
        wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })),
        Some(NetEvent::Connected { id: "zzzz".into(), name: "Raw".into() })
    );
    // Mia proves the secret too: her auth answers our nonce.
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["type"], "auth");
    assert_eq!(v["mac"], auth_mac(SECRET, NONCE, "mmmm", "zzzz"));
    line.clear();
    r.read_line(&mut line).unwrap();
    assert_eq!(line, "{\"type\":\"ping\"}\n");
    net.shutdown();
}

#[test]
fn impersonator_without_secret_is_rejected() {
    // Right friend id, but never answers the challenge (keeps pinging).
    let (net, rx, mut s, _r) = raw_client(&["zzzz"], "zzzz", None);
    for _ in 0..6 {
        let _ = writeln!(s, r#"{{"type":"ping"}}"#);
        let _ = writeln!(s, r#"{{"type":"leave","visit_id":1}}"#);
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        wait_for(&rx, 1.0, |e| matches!(e, NetEvent::Connected { .. } | NetEvent::Received { .. })),
        None
    );
    assert!(closed_by_peer(&mut s));
    net.shutdown();
}

#[test]
fn impersonator_with_wrong_secret_is_rejected() {
    let (net, rx, mut s, _r) = raw_client(&["zzzz"], "zzzz", Some(WRONG));
    assert!(closed_by_peer(&mut s));
    assert_eq!(wait_for(&rx, 0.3, |e| matches!(e, NetEvent::Connected { .. })), None);
    net.shutdown();
}

#[test]
fn replayed_auth_for_another_nonce_is_rejected() {
    // A mac valid for some other nonce (e.g. sniffed earlier) does not verify.
    let (net, _rx, port) = start_sniffed(friends(&["zzzz"]));
    let mut s = TcpStream::connect(local(port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    writeln!(s, r#"{{"type":"hello","id":"zzzz","name":"Raw","v":1,"nonce":"{NONCE}"}}"#).unwrap();
    let mac = auth_mac(SECRET, NONCE, "zzzz", "mmmm");
    writeln!(s, r#"{{"type":"auth","mac":"{mac}"}}"#).unwrap();
    assert!(closed_by_peer(&mut s));
    net.shutdown();
}

#[test]
fn silent_peer_times_out() {
    let (net, rx, _s, _r) = raw_friend();
    wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    // Raw client never pings back: conn_timeout (700 ms) must drop it.
    let t = Instant::now();
    assert_eq!(
        wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Disconnected { .. })),
        Some(NetEvent::Disconnected { id: "zzzz".into() })
    );
    assert!(t.elapsed() >= Duration::from_millis(400));
    net.shutdown();
}

#[test]
fn oversized_line_drops_connection() {
    let (net, rx, mut s, _r) = raw_friend();
    wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    let big = format!(r#"{{"type":"leave","visit_id":1,"pad":"{}"}}"#, "x".repeat(70 * 1024));
    let _ = writeln!(s, "{big}");
    assert!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    assert!(closed_by_peer(&mut s));
    net.shutdown();
}

#[test]
fn oversized_line_without_newline_drops_connection() {
    let (net, rx, mut s, _r) = raw_friend();
    wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    let _ = s.write_all(&vec![b'x'; 70 * 1024]);
    assert!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    net.shutdown();
}

#[test]
fn unknown_type_drops_connection() {
    let (net, rx, mut s, _r) = raw_friend();
    wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    writeln!(s, r#"{{"type":"bogus"}}"#).unwrap();
    assert!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    assert!(closed_by_peer(&mut s));
    net.shutdown();
}

#[test]
fn friend_raw_visit_is_received() {
    let (net, rx, mut s, _r) = raw_friend();
    wait_for(&rx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    writeln!(s, r#"{{"type":"leave","visit_id":9}}"#).unwrap();
    assert_eq!(
        wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "zzzz".into(), msg: Visit::Leave { visit_id: 9 } })
    );
    net.shutdown();
}

#[test]
fn visit_from_non_friend_is_ignored() {
    let (net, rx, mut s, _r) = raw_client(&[], "zzzz", None);
    let line = serde_json::to_string(&sample_visit()).unwrap();
    writeln!(s, "{line}").unwrap();
    writeln!(s, r#"{{"type":"come_home","visit_id":42}}"#).unwrap();
    assert_eq!(
        wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Received { .. } | NetEvent::Connected { .. })),
        None
    );
    assert!(net.friends().is_empty());
    net.shutdown();
}

#[test]
fn shutdown_is_prompt_and_frees_discovery_port() {
    let p = free_udp_port();
    let (n, _rx) = Net::start(cfg("solo", "Solo", p, free_udp_port(), &[])).unwrap();
    let t = Instant::now();
    n.shutdown();
    assert!(t.elapsed() < Duration::from_secs(2));
    // The port is free again immediately.
    UdpSocket::bind(local(p)).unwrap();
}

#[test]
fn paired_secret_survives_restart() {
    let (pa, pb, (a, arx), (b, brx)) = pair(&[], &[]);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    a.invite("bbbb", "cat");
    wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Invited { .. })).unwrap();
    b.answer_invite("aaaa", true);
    wait_for(&arx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).unwrap();
    let (fa, fb) = (a.friends(), b.friends());
    a.shutdown();
    b.shutdown();
    // The app persisted `friends()`; restarting with them reconnects silently.
    let (a2, arx2) = Net::start(cfg_with("aaaa", "Ann", pa, pb, fa)).unwrap();
    let (b2, brx2) = Net::start(cfg_with("bbbb", "Bob", pb, pa, fb)).unwrap();
    assert_eq!(
        wait_for(&arx2, 3.0, |e| matches!(e, NetEvent::Connected { .. } | NetEvent::Invited { .. })),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() })
    );
    assert_eq!(
        wait_for(&brx2, 3.0, |e| matches!(e, NetEvent::Connected { .. } | NetEvent::Invited { .. })),
        Some(NetEvent::Connected { id: "aaaa".into(), name: "Ann".into() })
    );
    a2.shutdown();
    b2.shutdown();
}

#[test]
fn mismatched_secrets_never_connect() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", pa, pb, &["bbbb"])).unwrap();
    let wrong = vec![Friend { id: "aaaa".into(), secret: WRONG.into() }];
    let (b, brx) = Net::start(cfg_with("bbbb", "Bob", pb, pa, wrong)).unwrap();
    assert_eq!(wait_for(&arx, 1.5, |e| matches!(e, NetEvent::Connected { .. })), None);
    assert_eq!(wait_for(&brx, 0.1, |e| matches!(e, NetEvent::Connected { .. })), None);
    a.shutdown();
    b.shutdown();
}

/// A fake peer: accepts TCP connections, says hello as `hello_id` (it cannot
/// answer any challenge) and keeps pinging. Counts accepted connections.
fn fake_peer(hello_id: &'static str, stop: Arc<AtomicBool>) -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let port = l.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let h = hits.clone();
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            match l.accept() {
                Ok((mut s, _)) => {
                    h.fetch_add(1, Ordering::SeqCst);
                    let stop = stop.clone();
                    std::thread::spawn(move || {
                        s.set_nonblocking(false).unwrap();
                        let hello = format!(
                            r#"{{"type":"hello","id":"{hello_id}","name":"Evil","v":1,"nonce":"{NONCE}"}}"#
                        );
                        if writeln!(s, "{hello}").is_err() {
                            return;
                        }
                        while !stop.load(Ordering::SeqCst) && writeln!(s, r#"{{"type":"ping"}}"#).is_ok() {
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    });
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    });
    (port, hits)
}

/// Keep announcing `id` at `tcp_port` to `to` until `stop`.
fn spoof_announcements(id: &'static str, tcp_port: u16, to: u16, stop: Arc<AtomicBool>) {
    let u = UdpSocket::bind("127.0.0.1:0").unwrap();
    std::thread::spawn(move || {
        let msg = format!(r#"{{"app":"screenpet","v":1,"id":"{id}","name":"Bob","port":{tcp_port}}}"#);
        while !stop.load(Ordering::SeqCst) {
            let _ = u.send_to(msg.as_bytes(), local(to));
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

/// A spoofed announcement for friend "bbbb" points at a fake peer answering
/// with `fake_hello_id`. It must not yield `Connected`, and must not stop the
/// real "bbbb" from connecting afterwards.
fn spoofed_friend_scenario(fake_hello_id: &'static str) {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", pa, pb, &["bbbb"])).unwrap();
    let (fake_port, hits) = fake_peer(fake_hello_id, stop.clone());
    spoof_announcements("bbbb", fake_port, pa, stop.clone());
    let t = Instant::now();
    while hits.load(Ordering::SeqCst) == 0 && t.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(hits.load(Ordering::SeqCst) > 0, "A dialed the spoofed address");
    assert_eq!(wait_for(&arx, 1.0, |e| matches!(e, NetEvent::Connected { .. })), None);
    // The real friend shows up while spoofing continues.
    let (b, brx) = Net::start(cfg("bbbb", "Bob", pb, pa, &["aaaa"])).unwrap();
    assert_eq!(
        wait_for(&arx, 5.0, |e| matches!(e, NetEvent::Connected { .. })),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() })
    );
    assert!(wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Connected { .. })).is_some());
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

#[test]
fn spoofed_friend_address_with_right_id_but_no_secret() {
    spoofed_friend_scenario("bbbb");
}

#[test]
fn dialed_peer_answering_with_other_id_is_rejected() {
    spoofed_friend_scenario("cccc");
}
