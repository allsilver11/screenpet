//! End to end through an in-process `screenpet-relay`: Net instances whose
//! LAN discovery is pointed at nobody, so they can only meet via the relay.

use screenpet_net::{Friend, Net, NetConfig, NetEvent, Side, Timing, Visit};
use screenpet_relay::{Relay, RelayConfig};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn fast() -> Timing {
    Timing {
        announce_interval: Duration::from_millis(150),
        peer_timeout: Duration::from_millis(1500),
        ping_interval: Duration::from_millis(200),
        conn_timeout: Duration::from_millis(1500),
        invite_timeout: Duration::from_millis(3000),
        relay_retry: Duration::from_millis(300),
    }
}

fn free_udp_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

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
    })
    .unwrap()
}

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

fn is_connected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Connected { .. })
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
        via: None,
        homesick_in: None,
        fling: None,
        note: None,
    }
}

fn received(rx: &Receiver<NetEvent>, id: u64) -> bool {
    wait_for(rx, 3.0, |e| matches!(e, NetEvent::Received { msg: Visit::Visit { visit_id, .. }, .. } if *visit_id == id))
        .is_some()
}

fn friends_of(ka: &[u8; 32], kb: &[u8; 32]) -> (Vec<Friend>, Vec<Friend>) {
    (
        vec![Friend { id: "bbbb".into(), public_key: Net::public_key(kb) }],
        vec![Friend { id: "aaaa".into(), public_key: Net::public_key(ka) }],
    )
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
    assert_eq!(a.friends(), vec![Friend { id: "bbbb".into(), public_key: Net::public_key(&kb) }]);
    assert_eq!(b.friends(), vec![Friend { id: "aaaa".into(), public_key: Net::public_key(&ka) }]);

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
    let relay = start_relay("127.0.0.1:0".parse().unwrap());
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
    let addr = start_relay("127.0.0.1:0".parse().unwrap()).local_addr();
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
