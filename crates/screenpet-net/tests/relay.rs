//! End to end through an in-process `screenpet-relay`: Net instances whose
//! LAN discovery is pointed at nobody, so they can only meet via the relay.

use screenpet_net::{Friend, Net, NetConfig, NetEvent, Side, Timing, Visit};
use screenpet_relay::{Relay, RelayConfig};
mod common;

use common::{blake2s, fast, free_tcp_port, free_udp_port, friend, hex, is_connected, unhex, wait_for};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tungstenite::{Error as WsError, Message as WsMessage, WebSocket};

fn start_relay(addr: SocketAddr) -> Relay {
    Relay::start(RelayConfig::new(addr, None)).unwrap()
}

fn url(relay: &Relay) -> String {
    format!("ws://{}/ws", relay.local_addr())
}

/// A Net with discovery on `port` "broadcasting" to `to` (a dead port unless
/// a test wires it up) and the relay at `relay_url`.
fn start_net(id: &str, key: [u8; 32], port: u16, to: u16, friends: Vec<Friend>, relay_url: &str) -> (Net, Receiver<NetEvent>) {
    start_net_with(id, key, port, to, friends, relay_url, fast(), None)
}

#[allow(clippy::too_many_arguments)]
fn start_net_with(
    id: &str,
    key: [u8; 32],
    port: u16,
    to: u16,
    friends: Vec<Friend>,
    relay_url: &str,
    timing: Timing,
    relay_key: Option<[u8; 32]>,
) -> (Net, Receiver<NetEvent>) {
    Net::start(NetConfig {
        id: id.into(),
        name: id.to_uppercase(),
        static_key: key,
        discovery_port: port,
        broadcast_to: SocketAddr::from(([127, 0, 0, 1], to)),
        friends,
        timing,
        relay_url: Some(relay_url.into()),
        relay_key,
        device: None,
        introduced: vec![],
        pending: vec![],
        pending_added: vec![],
    })
    .unwrap()
}

/// Every event within `secs`.
fn collect(rx: &Receiver<NetEvent>, secs: f64) -> Vec<NetEvent> {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    let mut out = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => out.push(e),
            Err(_) => break,
        }
    }
    out
}

fn code_of(rx: &Receiver<NetEvent>) -> String {
    match wait_for(rx, 5.0, |e| matches!(e, NetEvent::FriendCode { .. })) {
        Some(NetEvent::FriendCode { code }) => code,
        other => panic!("no friend code: {other:?}"),
    }
}

fn visit(id: u64) -> Visit {
    Visit::Visit {
        visit_id: id,
        species: "cat".into(),
        stage: "child".into(),
        scale: 1.0,
        personality: "playful".into(),
        owner: "A".into(),
        enter_side: Side::Left,
        vside: None,
        via: None,
        homesick_in: None,
        fling: None,
        note: None,
        gift: None,
        style: None, boost: None, of: None }
}

fn received(rx: &Receiver<NetEvent>, id: u64) -> bool {
    wait_for(rx, 3.0, |e| matches!(e, NetEvent::Received { msg: Visit::Visit { visit_id, .. }, .. } if *visit_id == id))
        .is_some()
}

fn friends_of(ka: &[u8; 32], kb: &[u8; 32]) -> (Vec<Friend>, Vec<Friend>) {
    (vec![friend("bbbb", kb)], vec![friend("aaaa", ka)])
}

#[test]
fn pair_with_a_friend_code_then_visit_through_the_relay() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (a, arx) = start_net("aaaa", ka, free_udp_port(), free_udp_port(), vec![], &url(&relay));
    let (b, brx) = start_net("bbbb", kb, free_udp_port(), free_udp_port(), vec![], &url(&relay));
    let _ = code_of(&arx);
    let code_b = code_of(&brx);
    assert!(a.relay_connected());
    assert_eq!(b.friend_code().as_deref(), Some(code_b.as_str()));

    // Typed loosely: lower case, no dash.
    a.redeem_code(&code_b.replace('-', "").to_lowercase(), "cat");
    let redeemed = wait_for(&arx, 5.0, |e| matches!(e, NetEvent::CodeRedeemed { .. })).expect("redeemed");
    assert!(matches!(redeemed, NetEvent::CodeRedeemed { ref id, .. } if id == "bbbb"));
    let Some(NetEvent::InviteSent { to, sas: sas_a, pairing: pa }) =
        wait_for(&arx, 5.0, |e| matches!(e, NetEvent::InviteSent { .. }))
    else {
        panic!("no InviteSent")
    };
    assert_eq!(to, "bbbb");
    let Some(NetEvent::Invited { from, sas: sas_b, pairing: pb, .. }) =
        wait_for(&brx, 5.0, |e| matches!(e, NetEvent::Invited { .. }))
    else {
        panic!("no Invited")
    };
    assert_eq!(from, "aaaa");
    assert_eq!(sas_a, sas_b, "same SAS on both screens: the relay is not in the middle");
    a.answer_invite(pa, true);
    b.answer_invite(pb, true);
    assert!(wait_for(&arx, 5.0, is_connected).is_some());
    assert!(wait_for(&brx, 5.0, is_connected).is_some());
    assert_eq!(a.friends(), vec![friend("bbbb", &kb)]);
    assert_eq!(b.friends(), vec![friend("aaaa", &ka)]);

    a.send("bbbb", visit(1));
    assert!(received(&brx, 1));
    b.send("aaaa", visit(2));
    assert!(received(&arx, 2));
}

#[test]
fn bad_codes_are_reported() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let (a, arx) = start_net("aaaa", Net::generate_static_key(), free_udp_port(), free_udp_port(), vec![], &url(&relay));
    let own = code_of(&arx);
    let reason = |rx: &Receiver<NetEvent>| match wait_for(rx, 5.0, |e| matches!(e, NetEvent::CodeFailed { .. })) {
        Some(NetEvent::CodeFailed { reason, .. }) => reason,
        other => panic!("no CodeFailed: {other:?}"),
    };
    a.redeem_code("hello", "cat");
    assert_eq!(reason(&arx), "invalid");
    a.redeem_code("ZZZZ-ZZZZ", "cat");
    assert_eq!(reason(&arx), "unknown");
    // The relay answers "unknown" for its owner's own code too.
    a.redeem_code(&own, "cat");
    assert_eq!(reason(&arx), "unknown");
}

#[test]
fn friends_reconnect_through_the_relay_and_after_it_restarts() {
    // Restarted on its port below: one the OS could hand to another test meanwhile won't do
    let relay = start_relay(SocketAddr::from(([127, 0, 0, 1], free_tcp_port())));
    let addr = relay.local_addr();
    let u = url(&relay);
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (fa, fb) = friends_of(&ka, &kb);
    let (a, arx) = start_net("aaaa", ka, free_udp_port(), free_udp_port(), fa, &u);
    let (b, brx) = start_net("bbbb", kb, free_udp_port(), free_udp_port(), fb.clone(), &u);
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "friends linked through the relay");
    assert!(wait_for(&brx, 8.0, is_connected).is_some());
    a.send("bbbb", visit(3));
    assert!(received(&brx, 3));

    // B restarts: A notices, then B is back automatically.
    drop(b);
    assert!(wait_for(&arx, 5.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    let (b, brx) = start_net("bbbb", kb, free_udp_port(), free_udp_port(), fb, &u);
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "reconnected after B restarted");
    assert!(wait_for(&brx, 8.0, is_connected).is_some());

    // The relay restarts (new process, nothing remembered): both reconnect
    // to it with backoff and the friends find each other again.
    drop(relay);
    assert!(wait_for(&arx, 5.0, |e| matches!(e, NetEvent::RelayStatus { connected: false })).is_some());
    assert!(wait_for(&arx, 5.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
    let _relay = start_relay(addr);
    assert!(wait_for(&arx, 10.0, |e| matches!(e, NetEvent::RelayStatus { connected: true })).is_some());
    assert!(wait_for(&arx, 10.0, is_connected).is_some(), "linked again via the restarted relay");
    assert!(wait_for(&brx, 10.0, is_connected).is_some());
    b.send("aaaa", visit(4));
    assert!(received(&arx, 4));
}

#[test]
fn strangers_cannot_link_through_the_relay_without_a_code() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let u = url(&relay);
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    // A pins B, but B does not have A: the relay never joins them.
    let (fa, _) = friends_of(&ka, &kb);
    let (_a, arx) = start_net("aaaa", ka, free_udp_port(), free_udp_port(), fa, &u);
    let (_b, brx) = start_net("bbbb", kb, free_udp_port(), free_udp_port(), vec![], &u);
    let _ = code_of(&arx);
    let _ = code_of(&brx);
    assert!(wait_for(&arx, 2.0, is_connected).is_none());
    assert!(wait_for(&brx, 0.5, |e| matches!(e, NetEvent::Invited { .. } | NetEvent::Connected { .. })).is_none());
}

/// Forwards datagrams from a local socket to `to` while `on` is set.
fn gate(to: u16, on: Arc<AtomicBool>, stop: Arc<AtomicBool>) -> u16 {
    let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
    sock.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let port = sock.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while !stop.load(Ordering::SeqCst) {
            if let Ok((n, _)) = sock.recv_from(&mut buf) {
                if on.load(Ordering::SeqCst) {
                    let _ = sock.send_to(&buf[..n], ("127.0.0.1", to));
                }
            }
        }
    });
    port
}

#[test]
fn a_relay_link_moves_to_the_lan_when_the_friend_shows_up_there() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let u = url(&relay);
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (fa, fb) = friends_of(&ka, &kb);
    let pa = free_udp_port();
    let (on, stop) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)));
    // B's announcements reach A only once the gate opens.
    let g = gate(pa, on.clone(), stop.clone());
    let (a, arx) = start_net("aaaa", ka, pa, free_udp_port(), fa, &u);
    let (b, brx) = start_net("bbbb", kb, free_udp_port(), g, fb, &u);
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "linked through the relay first");
    assert!(wait_for(&brx, 8.0, is_connected).is_some());

    // A now hears B on the LAN: the link moves there without a disconnect.
    on.store(true, Ordering::SeqCst);
    let events = collect(&arx, 3.0);
    assert!(events.iter().any(|e| matches!(e, NetEvent::PeerSeen { .. })), "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, NetEvent::Disconnected { .. })), "no flap: {events:?}");
    assert!(!collect(&brx, 0.1).iter().any(|e| matches!(e, NetEvent::Disconnected { .. })));

    // Proof the link is on the LAN: the relay can go away and the link stays.
    drop(relay);
    let events = collect(&arx, 1.5);
    assert!(events.iter().any(|e| matches!(e, NetEvent::RelayStatus { connected: false })), "{events:?}");
    assert!(!events.iter().any(|e| matches!(e, NetEvent::Disconnected { .. })), "link was not on the relay: {events:?}");
    assert!(!collect(&brx, 0.1).iter().any(|e| matches!(e, NetEvent::Disconnected { .. })));
    a.send("bbbb", visit(5));
    assert!(received(&brx, 5), "delivered over the LAN");
    b.send("aaaa", visit(6));
    assert!(received(&arx, 6));
    assert!(wait_for(&arx, 1.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_none());
    stop.store(true, Ordering::SeqCst);
}

/// Against a deployed relay: `SCREENPET_LIVE_RELAY=wss://pet.danni.day/ws
/// cargo test -p screenpet-net --test relay -- --ignored`. Two throwaway
/// installs pair with a friend code and exchange a visit.
#[test]
#[ignore]
fn live_relay_smoke() {
    let u = std::env::var("SCREENPET_LIVE_RELAY").unwrap_or_else(|_| screenpet_net::DEFAULT_RELAY_URL.to_string());
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (ia, ib) = (screenpet_net::new_install_id(), screenpet_net::new_install_id());
    let (a, arx) = start_net(&ia, ka, free_udp_port(), free_udp_port(), vec![], &u);
    let (b, brx) = start_net(&ib, kb, free_udp_port(), free_udp_port(), vec![], &u);
    let _ = code_of(&arx);
    let code_b = code_of(&brx);
    a.redeem_code(&code_b, "cat");
    let Some(NetEvent::InviteSent { sas: sa, pairing: pa, .. }) =
        wait_for(&arx, 10.0, |e| matches!(e, NetEvent::InviteSent { .. } | NetEvent::CodeFailed { .. }))
    else {
        panic!("no InviteSent")
    };
    let Some(NetEvent::Invited { sas: sb, pairing: pb, .. }) = wait_for(&brx, 10.0, |e| matches!(e, NetEvent::Invited { .. }))
    else {
        panic!("no Invited")
    };
    assert_eq!(sa, sb);
    a.answer_invite(pa, true);
    b.answer_invite(pb, true);
    assert!(wait_for(&arx, 10.0, is_connected).is_some());
    assert!(wait_for(&brx, 10.0, is_connected).is_some());
    a.send(&ib, visit(9));
    assert!(received(&brx, 9));
}

#[test]
fn a_lan_link_replaced_by_a_relay_link_within_grace_does_not_flap() {
    // The relay is down at first, so the friends link on the LAN.
    // Restarted on its port below: one the OS could hand to another test meanwhile won't do
    let addr = start_relay(SocketAddr::from(([127, 0, 0, 1], free_tcp_port()))).local_addr();
    let u = format!("ws://{addr}/ws");
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (fa, fb) = friends_of(&ka, &kb);
    let timing = Timing { conn_timeout: Duration::from_secs(3), ..fast() };
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (a, arx) = start_net_with("aaaa", ka, pa, pb, fa, &u, timing.clone(), None);
    let (b, brx) = start_net_with("bbbb", kb, pb, pa, fb.clone(), &u, timing.clone(), None);
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "linked on the LAN");
    assert!(wait_for(&brx, 8.0, is_connected).is_some());
    let _relay = start_relay(addr);
    assert!(wait_for(&arx, 10.0, |e| matches!(e, NetEvent::RelayStatus { connected: true })).is_some());

    // B restarts somewhere A cannot see on the LAN: it comes back through
    // the relay well within the grace, so A never reports a disconnect.
    drop(b);
    drop(brx);
    let (b, brx) = start_net_with("bbbb", kb, free_udp_port(), free_udp_port(), fb, &u, timing, None);
    assert!(wait_for(&brx, 8.0, is_connected).is_some(), "B linked again (through the relay)");
    let events = collect(&arx, 1.0);
    assert!(!events.iter().any(|e| matches!(e, NetEvent::Disconnected { .. } | NetEvent::Connected { .. })), "no flap: {events:?}");
    a.send("bbbb", visit(7));
    assert!(received(&brx, 7));
    // A real loss is still reported, after the grace.
    drop(b);
    assert!(wait_for(&arx, 8.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
}

#[test]
fn a_taken_discovery_port_leaves_the_relay_working() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let u = url(&relay);
    let holder = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (fa, fb) = friends_of(&ka, &kb);
    // 다른 프로그램이 발견 포트를 쓰고 있어도 인터넷 친구는 만난다
    let (a, arx) = start_net("aaaa", ka, port, free_udp_port(), fa, &u);
    let (_b, brx) = start_net("bbbb", kb, free_udp_port(), free_udp_port(), fb, &u);
    assert_eq!(a.discovery_port(), port);
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "linked through the relay");
    assert!(wait_for(&brx, 8.0, is_connected).is_some());
    // 끊긴 친구 알림(유예 뒤 Disconnected)도 그대로 돈다
    drop(_b);
    assert!(wait_for(&arx, 8.0, |e| matches!(e, NetEvent::Disconnected { .. })).is_some());
}

#[test]
fn the_relay_key_is_pinned() {
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
    let u = url(&relay);
    let (_bad, brx) =
        start_net_with("bbbb", Net::generate_static_key(), free_udp_port(), free_udp_port(), vec![], &u, fast(), Some([7; 32]));
    assert!(wait_for(&brx, 2.0, |e| matches!(e, NetEvent::RelayStatus { connected: true })).is_none(), "wrong key refused");
    let (good, grx) = start_net_with(
        "aaaa",
        Net::generate_static_key(),
        free_udp_port(),
        free_udp_port(),
        vec![],
        &u,
        fast(),
        Some(relay.public_key()),
    );
    let first = code_of(&grx);
    assert!(good.new_friend_code());
    let second = code_of(&grx);
    assert_ne!(first, second, "a new code on request");
}

// ---- a relay we don't trust --------------------------------------------------

/// What a fake relay answers after `auth`.
#[derive(Clone, Copy, PartialEq)]
enum Proof {
    /// Computed with the relay's secret key.
    Real,
    /// Has the right public key but not the secret: can only guess.
    Forged,
    /// An old-style `welcome` without any proof.
    Missing,
}

/// A frame the relay carried: channel, sender's key, ciphertext.
type Carried = (u32, String, Vec<u8>);

#[derive(Default)]
struct RelaySeen {
    frames: Vec<Carried>,
    /// `close{ch}` the relay received: (channel, sender's key).
    closes: Vec<(u32, String)>,
    /// Sent to `.0` as they are (channel + ciphertext).
    inject: Vec<(String, Vec<u8>)>,
}

/// A hostile relay that speaks just enough of the protocol: it authenticates,
/// joins any two clients in a channel, and can read, drop and inject frames.
struct FakeRelay {
    addr: SocketAddr,
    public: [u8; 32],
    seen: Arc<Mutex<RelaySeen>>,
    stop: Arc<AtomicBool>,
}

impl Drop for FakeRelay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn unhex32(s: &str) -> [u8; 32] {
    unhex(s).try_into().unwrap()
}

struct RelayConn {
    ws: WebSocket<TcpStream>,
    key: String,
}

fn would_block(e: &WsError) -> bool {
    matches!(e, WsError::Io(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut))
}

fn read_json(ws: &mut WebSocket<TcpStream>) -> Option<Value> {
    match ws.read().ok()? {
        WsMessage::Text(t) => serde_json::from_str(t.as_str()).ok(),
        _ => None,
    }
}

/// Challenge, `auth`, then `welcome` carrying the chosen proof.
fn relay_greet(stream: TcpStream, secret: &[u8; 32], proof: Proof, online: Vec<String>) -> Option<RelayConn> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    let mut ws = tungstenite::accept(stream).ok()?;
    let public = Net::public_key(secret);
    let nonce = Net::generate_static_key();
    ws.send(WsMessage::text(json!({"t": "challenge", "relay": hex(&public), "nonce": hex(&nonce)}).to_string())).ok()?;
    let auth = read_json(&mut ws)?;
    let (key, cnonce) = (auth["key"].as_str()?.to_string(), unhex32(auth["cnonce"].as_str()?));
    let client = unhex32(&key);
    let shared = curve25519_dalek::MontgomeryPoint(client).mul_clamped(*secret).to_bytes();
    let real = hex(&blake2s(&[b"screenpet-relay/1 welcome", &shared, &cnonce, &nonce, &client]));
    let mut welcome = json!({"t": "welcome", "code": "ABCD-1234", "online": online});
    match proof {
        Proof::Real => welcome["proof"] = json!(real),
        Proof::Forged => welcome["proof"] = json!(hex(&Net::generate_static_key())),
        Proof::Missing => {}
    }
    ws.send(WsMessage::text(welcome.to_string())).ok()?;
    ws.get_ref().set_read_timeout(Some(Duration::from_millis(2))).ok()?;
    Some(RelayConn { ws, key })
}

fn start_fake_relay(proof: Proof) -> FakeRelay {
    let secret = Net::generate_static_key();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    let (seen, stop): (Arc<Mutex<RelaySeen>>, _) = (Arc::default(), Arc::new(AtomicBool::new(false)));
    let (seen2, stop2) = (seen.clone(), stop.clone());
    std::thread::spawn(move || {
        let mut conns: Vec<RelayConn> = Vec::new();
        let mut channels: HashMap<u32, (String, String)> = HashMap::new();
        let mut next_ch = 1u32;
        while !stop2.load(Ordering::SeqCst) {
            if let Ok((s, _)) = listener.accept() {
                let online = conns.iter().map(|c| c.key.clone()).collect();
                if let Some(new) = relay_greet(s, &secret, proof, online) {
                    conns.retain(|c| c.key != new.key); // a reconnect replaces the old connection
                    for c in conns.iter_mut() {
                        let _ = c.ws.send(WsMessage::text(json!({"t": "presence", "key": new.key, "online": true}).to_string()));
                    }
                    conns.push(new);
                }
            }
            let mut out: Vec<(String, WsMessage)> = Vec::new();
            for c in conns.iter_mut() {
                match c.ws.read() {
                    Ok(WsMessage::Text(t)) => {
                        let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                        match v["t"].as_str() {
                            Some("open") => {
                                let (ch, to) = (next_ch, v["to"].as_str().unwrap_or("").to_string());
                                next_ch += 1;
                                channels.insert(ch, (c.key.clone(), to.clone()));
                                out.push((c.key.clone(), WsMessage::text(json!({"t": "opened", "req": v["req"], "ch": ch}).to_string())));
                                out.push((to, WsMessage::text(json!({"t": "incoming", "ch": ch, "from": c.key}).to_string())));
                            }
                            Some("close") => {
                                let ch = v["ch"].as_u64().unwrap_or(0) as u32;
                                seen2.lock().unwrap().closes.push((ch, c.key.clone()));
                                if let Some((a, b)) = channels.get(&ch) {
                                    let other = if *a == c.key { b.clone() } else { a.clone() };
                                    out.push((other, WsMessage::text(json!({"t": "close", "ch": ch}).to_string())));
                                }
                            }
                            Some("ping") => out.push((c.key.clone(), WsMessage::text(json!({"t": "pong"}).to_string()))),
                            _ => {}
                        }
                    }
                    Ok(WsMessage::Binary(b)) if b.len() > 4 => {
                        let ch = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                        seen2.lock().unwrap().frames.push((ch, c.key.clone(), b[4..].to_vec()));
                        if let Some((x, y)) = channels.get(&ch) {
                            let other = if *x == c.key { y.clone() } else { x.clone() };
                            out.push((other, WsMessage::binary(b.to_vec())));
                        }
                    }
                    Ok(_) => {}
                    Err(e) if would_block(&e) => {}
                    Err(_) => {}
                }
            }
            for (to, bytes) in std::mem::take(&mut seen2.lock().unwrap().inject) {
                out.push((to, WsMessage::binary(bytes)));
            }
            for (to, m) in out {
                if let Some(c) = conns.iter_mut().find(|c| c.key == to) {
                    let _ = c.ws.send(m);
                }
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    FakeRelay { addr, public: Net::public_key(&secret), seen, stop }
}

impl FakeRelay {
    fn url(&self) -> String {
        format!("ws://{}/ws", self.addr)
    }
}

/// One of these relays, and an install pinning its public key.
fn relay_status_with(proof: Proof) -> Option<NetEvent> {
    let relay = start_fake_relay(proof);
    let (net, rx) = start_net_with("aaaa", Net::generate_static_key(), free_udp_port(), free_udp_port(), vec![], &relay.url(), fast(), Some(relay.public));
    let got = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::RelayStatus { connected: true } | NetEvent::FriendCode { .. }));
    let code = net.friend_code();
    drop(net);
    if got.is_some() {
        assert!(code.is_some(), "accepted: {got:?}");
    }
    got
}

#[test]
fn a_relay_with_the_right_public_key_but_no_secret_is_refused() {
    // 진짜 증명은 통과한다 (이 가짜 릴레이가 프로토콜을 제대로 말한다는 뜻)
    assert!(relay_status_with(Proof::Real).is_some(), "the fake relay works when it can prove its key");
    assert_eq!(relay_status_with(Proof::Forged), None, "a guessed proof is refused");
    assert_eq!(relay_status_with(Proof::Missing), None, "no proof is refused");
}

fn wait_until(secs: f64, mut f: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < deadline {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    f()
}

#[test]
fn relay_attacker_cannot_inject_or_replay() {
    let relay = start_fake_relay(Proof::Real);
    let (ka, kb) = (Net::generate_static_key(), Net::generate_static_key());
    let (key_a, key_b) = (hex(&Net::public_key(&ka)), hex(&Net::public_key(&kb)));
    let (fa, fb) = friends_of(&ka, &kb);
    let u = relay.url();
    let (a, arx) = start_net_with("aaaa", ka, free_udp_port(), free_udp_port(), fa.clone(), &u, fast(), Some(relay.public));
    let (b, brx) = start_net_with("bbbb", kb, free_udp_port(), free_udp_port(), fb.clone(), &u, fast(), Some(relay.public));
    assert!(wait_for(&arx, 8.0, is_connected).is_some(), "A linked through the channel");
    assert!(wait_for(&brx, 8.0, is_connected).is_some(), "B linked through the channel");
    a.send("bbbb", visit(1));
    assert!(received(&brx, 1), "the channel carries a real visit");

    // 릴레이는 암호문만 본다: 친구 사이의 어떤 프레임에도 평문이 없다
    let sent: Vec<Carried> = relay.seen.lock().unwrap().frames.clone();
    assert!(sent.len() >= 4, "{} frames", sent.len());
    for (_, _, f) in &sent {
        let text = String::from_utf8_lossy(f);
        assert!(!text.contains("visit_id") && !text.contains("hello"), "plaintext through the relay: {text}");
    }

    let closed_by_b = |seen: &RelaySeen, ch: u32| seen.closes.iter().any(|(c, k)| *c == ch && *k == key_b);
    let attack = |payload: Vec<u8>, ch: u32| {
        let mut bytes = ch.to_be_bytes().to_vec();
        bytes.extend_from_slice(&payload);
        relay.seen.lock().unwrap().inject.push((key_b.clone(), bytes));
        assert!(wait_until(3.0, || closed_by_b(&relay.seen.lock().unwrap(), ch)), "B drops the channel at once");
    };
    let last_a_to_b = || relay.seen.lock().unwrap().frames.iter().rev().find(|(_, k, _)| *k == key_a).cloned().expect("a frame from A");

    // 1. 지나간 프레임을 다시 보낸다 (A의 hello): 복호화에 실패하고 전달되지 않는다
    let (ch, _, _) = last_a_to_b();
    let replay = relay.seen.lock().unwrap().frames.iter().filter(|(c, k, _)| *c == ch && *k == key_a).nth(2).expect("A's hello").2.clone();
    attack(replay, ch);
    assert!(wait_for(&brx, 0.3, |e| matches!(e, NetEvent::Received { .. })).is_none(), "a replay is never delivered");

    // 끊긴 링크는 다시 맺어진다 (새 채널로)
    assert!(wait_until(10.0, || relay.seen.lock().unwrap().frames.iter().any(|(c, k, _)| *c > ch && *k == key_a) && a.peer_version("bbbb").is_some() && b.peer_version("aaaa").is_some()), "linked again");

    // 2. 평문으로 방문 메시지를 끼워 넣는다
    let (ch2, _, _) = last_a_to_b();
    assert!(ch2 > ch);
    let forged = serde_json::to_vec(&json!({"type": "leave", "visit_id": 66})).unwrap();
    attack(forged, ch2);
    assert!(wait_for(&brx, 0.3, |e| matches!(e, NetEvent::Received { .. })).is_none(), "an injected message is never delivered");

    assert_eq!(a.friends(), fa, "friendship untouched");
    assert_eq!(b.friends(), fb, "friendship untouched");
}
