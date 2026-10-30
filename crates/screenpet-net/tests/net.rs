//! Localhost integration tests: Net instances on distinct discovery ports,
//! each "broadcasting" (UDP unicast) to the other's port, plus hand-rolled
//! Noise clients (via `snow`) for the protocol / attack cases.

use blake2::{Blake2s256, Digest};
use screenpet_net::{new_install_id, Friend, Net, NetConfig, NetEvent, Side, Timing, Visit, MAX_FRIENDS, SAS_EMOJI};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---- helpers ---------------------------------------------------------------

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

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn key() -> [u8; 32] {
    Net::generate_static_key()
}

fn friend(id: &str, key: &[u8; 32]) -> Friend {
    Friend { id: id.into(), public_key: Net::public_key(key) }
}

fn cfg(id: &str, name: &str, key: [u8; 32], port: u16, to: u16, friends: Vec<Friend>) -> NetConfig {
    NetConfig {
        id: id.into(),
        name: name.into(),
        static_key: key,
        discovery_port: port,
        broadcast_to: local(to),
        friends,
        timing: fast(),
        relay_url: None,
        relay_key: None,
    }
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

fn is_connected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Connected { .. })
}
fn is_disconnected(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Disconnected { .. })
}

struct Pair {
    pa: u16,
    pb: u16,
    ka: [u8; 32],
    kb: [u8; 32],
    a: Net,
    arx: Receiver<NetEvent>,
    b: Net,
    brx: Receiver<NetEvent>,
}

/// Two instances "aaaa" < "bbbb" pointed at each other.
fn pair(friends: bool) -> Pair {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (fa, fb) = if friends { (vec![friend("bbbb", &kb)], vec![friend("aaaa", &ka)]) } else { (vec![], vec![]) };
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, fa)).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, fb)).unwrap();
    Pair { pa, pb, ka, kb, a, arx, b, brx }
}

fn connected_pair() -> Pair {
    let p = pair(true);
    wait_for(&p.arx, 5.0, is_connected).expect("a connected");
    wait_for(&p.brx, 5.0, is_connected).expect("b connected");
    p
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
        via: None,
        homesick_in: None,
        fling: None,
        note: None,
    }
}

/// Start a Net "mmmm" (discovery on an OS-chosen port) that "broadcasts" to
/// a sniffer; returns its TCP port from the announcement.
fn start_sniffed(net_key: [u8; 32], friends: Vec<Friend>) -> (Net, Receiver<NetEvent>, u16) {
    let sniffer = UdpSocket::bind("127.0.0.1:0").unwrap();
    sniffer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let to = sniffer.local_addr().unwrap().port();
    let (net, rx) = Net::start(cfg("mmmm", "Mia", net_key, 0, to, friends)).unwrap();
    (net, rx, sniff_port(&sniffer, "mmmm"))
}

fn sniff_port(sniffer: &UdpSocket, id: &str) -> u16 {
    let mut buf = [0u8; 2048];
    loop {
        let (n, _) = sniffer.recv_from(&mut buf).unwrap();
        let v: Value = serde_json::from_slice(&buf[..n]).unwrap();
        if v["id"] == id {
            assert_eq!(v["app"], "screenpet");
            return v["port"].as_u64().unwrap() as u16;
        }
    }
}

/// True once the other side has closed the socket.
fn closed_by_peer(s: &mut TcpStream) -> bool {
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0u8; 4096];
    loop {
        match s.read(&mut buf) {
            Ok(0) => return true,
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => return true,
            Err(_) => return false,
        }
    }
}

// ---- minimal Noise client ----------------------------------------------------

fn write_frame(s: &mut TcpStream, b: &[u8]) -> io::Result<()> {
    let mut v = (b.len() as u16).to_be_bytes().to_vec();
    v.extend_from_slice(b);
    s.write_all(&v)
}

fn read_frame(s: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut l = [0u8; 2];
    s.read_exact(&mut l)?;
    let mut b = vec![0u8; usize::from(u16::from_be_bytes(l))];
    s.read_exact(&mut b)?;
    Ok(b)
}

struct Peer {
    s: TcpStream,
    t: snow::TransportState,
    hash: [u8; 32],
    remote: [u8; 32],
}

fn handshake(mut s: TcpStream, key: &[u8; 32], initiator: bool) -> io::Result<Peer> {
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    let b = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap())
        .local_private_key(key)
        .unwrap()
        .prologue(b"screenpet-net/1")
        .unwrap();
    let mut hs = if initiator { b.build_initiator() } else { b.build_responder() }.unwrap();
    let mut buf = vec![0u8; 65535];
    while !hs.is_handshake_finished() {
        if hs.is_my_turn() {
            let n = hs.write_message(&[], &mut buf).unwrap();
            write_frame(&mut s, &buf[..n])?;
        } else {
            let f = read_frame(&mut s)?;
            hs.read_message(&f, &mut buf).map_err(|_| io::Error::other("handshake"))?;
        }
    }
    let hash = hs.get_handshake_hash().try_into().unwrap();
    let remote = hs.get_remote_static().unwrap().try_into().unwrap();
    Ok(Peer { s, t: hs.into_transport_mode().unwrap(), hash, remote })
}

impl Peer {
    fn send(&mut self, v: Value) -> io::Result<()> {
        let p = serde_json::to_vec(&v).unwrap();
        let mut buf = vec![0u8; p.len() + 16];
        let n = self.t.write_message(&p, &mut buf).unwrap();
        write_frame(&mut self.s, &buf[..n])
    }
    fn recv(&mut self) -> io::Result<Value> {
        let f = read_frame(&mut self.s)?;
        let mut buf = vec![0u8; f.len()];
        let n = self.t.read_message(&f, &mut buf).map_err(|_| io::Error::other("decrypt"))?;
        Ok(serde_json::from_slice(&buf[..n]).unwrap())
    }
    /// Next message that is not a ping.
    fn recv_msg(&mut self) -> io::Result<Value> {
        loop {
            let v = self.recv()?;
            if v["type"] != "ping" {
                return Ok(v);
            }
        }
    }
}

fn hello(id: &str, name: &str) -> Value {
    json!({"type": "hello", "id": id, "name": name, "v": 1})
}

/// Hello from a peer that has the receiver pinned as a friend.
fn friend_hello(id: &str, name: &str) -> Value {
    json!({"type": "hello", "id": id, "name": name, "v": 1, "friend": true})
}

/// Connect to `port`, handshake with `key`, exchange hellos claiming `id`.
fn raw_connect(port: u16, key: &[u8; 32], id: &str) -> Peer {
    raw_connect_as(port, key, hello(id, "Raw"))
}

/// Like `raw_connect`, as a friend that has the server pinned.
fn raw_friend_connect(port: u16, key: &[u8; 32], id: &str) -> Peer {
    raw_connect_as(port, key, friend_hello(id, "Raw"))
}

fn raw_connect_as(port: u16, key: &[u8; 32], hello_msg: Value) -> Peer {
    let mut p = handshake(TcpStream::connect(local(port)).unwrap(), key, true).unwrap();
    p.send(hello_msg).unwrap();
    let h = p.recv_msg().unwrap();
    assert_eq!(h["type"], "hello");
    p
}

/// Like `raw_connect`, but does not wait for the server's hello (for links
/// the server is expected to reject right after our hello).
fn raw_connect_claiming(port: u16, key: &[u8; 32], id: &str) -> Peer {
    let mut p = handshake(TcpStream::connect(local(port)).unwrap(), key, true).unwrap();
    p.send(hello(id, "Raw")).unwrap();
    p
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn blake2s(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Blake2s256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// Independent implementation of the documented SAS:
/// first 36 bits of BLAKE2s(handshake_hash || nonce_i || r_r), 6 bits/symbol.
fn expected_sas(hash: &[u8], nonce_i: &[u8], r_r: &[u8]) -> String {
    let d = blake2s(&[hash, nonce_i, r_r]);
    let bits = d[..5].iter().fold(0u64, |a, b| (a << 8) | u64::from(*b));
    (0..6).map(|i| SAS_EMOJI[((bits >> (34 - 6 * i)) & 63) as usize]).collect::<Vec<_>>().join(" ")
}

fn random32() -> [u8; 32] {
    let mut b = [0u8; 32];
    for x in b.iter_mut() {
        *x = fastrand::u8(..);
    }
    b
}

fn pairing_of(e: &NetEvent) -> u64 {
    match e {
        NetEvent::Invited { pairing, .. } | NetEvent::InviteSent { pairing, .. } => *pairing,
        _ => panic!("no pairing in {e:?}"),
    }
}

fn sas_of(e: &NetEvent) -> String {
    match e {
        NetEvent::Invited { sas, .. } | NetEvent::InviteSent { sas, .. } => sas.clone(),
        _ => panic!("no sas in {e:?}"),
    }
}

// ---- basics -------------------------------------------------------------------

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
fn static_key_identity_is_what_peers_see() {
    let k = key();
    assert_ne!(k, key());
    let (net, _rx, port) = start_sniffed(k, vec![]);
    let p = raw_connect(port, &key(), "zzzz");
    assert_eq!(p.remote, Net::public_key(&k));
    net.shutdown();
}

#[test]
fn discovery_sees_peer_and_notices_it_gone() {
    let p = pair(false);
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })),
        Some(NetEvent::PeerSeen { id: "bbbb".into(), name: "Bob".into() })
    );
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })),
        Some(NetEvent::PeerSeen { id: "aaaa".into(), name: "Ann".into() })
    );
    p.b.shutdown();
    assert_eq!(
        wait_for(&p.arx, 6.0, |e| matches!(e, NetEvent::PeerGone { .. })),
        Some(NetEvent::PeerGone { id: "bbbb".into() })
    );
    p.a.shutdown();
}

#[test]
fn own_announcement_is_ignored() {
    let port = free_udp_port();
    let (n, rx) = Net::start(cfg("self", "Me", key(), port, port, vec![])).unwrap();
    assert_eq!(wait_for(&rx, 1.0, |e| matches!(e, NetEvent::PeerSeen { .. })), None);
    n.shutdown();
}

#[test]
fn shutdown_is_prompt_and_frees_discovery_port() {
    let (n, _rx) = Net::start(cfg("solo", "Solo", key(), 0, free_udp_port(), vec![])).unwrap();
    let p = n.discovery_port();
    let t = Instant::now();
    n.shutdown();
    assert!(t.elapsed() < Duration::from_secs(3));
    UdpSocket::bind(local(p)).unwrap();
}

// ---- pairing --------------------------------------------------------------------

/// A invites B; returns (A's InviteSent, B's Invited).
fn start_pairing(p: &Pair) -> (NetEvent, NetEvent) {
    wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    p.a.invite("bbbb", "cat");
    let sent = wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::InviteSent { .. })).expect("InviteSent");
    let invited = wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    (sent, invited)
}

#[test]
fn pairing_shows_equal_sas_and_needs_both_confirmations() {
    let p = pair(false);
    let (sent, invited) = start_pairing(&p);
    let sas = sas_of(&sent);
    assert_eq!(sent, NetEvent::InviteSent { to: "bbbb".into(), sas: sas.clone(), pairing: pairing_of(&sent) });
    assert_eq!(
        invited,
        NetEvent::Invited {
            from: "aaaa".into(),
            name: "Ann".into(),
            pet: "cat".into(),
            sas: sas.clone(),
            pairing: pairing_of(&invited)
        }
    );
    let symbols: Vec<_> = sas.split(' ').collect();
    assert_eq!(symbols.len(), 6);
    assert!(symbols.iter().all(|s| SAS_EMOJI.contains(s)));

    // Invitee confirms: the inviter learns it, but nothing is pinned yet.
    p.b.answer_invite(pairing_of(&invited), true);
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "bbbb".into(), accepted: true, pairing: pairing_of(&sent) })
    );
    assert_eq!(wait_for(&p.arx, 0.5, is_connected), None);
    assert!(p.a.friends().is_empty() && p.b.friends().is_empty());

    // Inviter confirms too: both pin each other's static public key.
    p.a.answer_invite(pairing_of(&sent), true);
    assert_eq!(wait_for(&p.arx, 5.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert_eq!(wait_for(&p.brx, 5.0, is_connected), Some(NetEvent::Connected { id: "aaaa".into(), name: "Ann".into() }));
    assert_eq!(p.a.friends(), vec![friend("bbbb", &p.kb)]);
    assert_eq!(p.b.friends(), vec![friend("aaaa", &p.ka)]);
    assert_eq!(wait_for(&p.arx, 1.5, is_disconnected), None, "link stays up");
    p.a.shutdown();
    p.b.shutdown();
}

fn many_friends(n: usize) -> Vec<Friend> {
    (0..n).map(|i| friend(&format!("f{i:03}"), &key())).collect()
}

#[test]
fn a_full_friend_list_refuses_new_pairings_both_ways() {
    // b is full: a's invite is refused before any SAS is shown
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", key(), pa, pb, vec![])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", key(), pb, pa, many_friends(MAX_FRIENDS))).unwrap();
    wait_for(&arx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    a.invite("bbbb", "cat");
    assert_eq!(wait_for(&brx, 2.0, |e| matches!(e, NetEvent::Invited { .. })), None);
    assert_eq!(wait_for(&arx, 0.5, |e| matches!(e, NetEvent::InviteSent { .. })), None);
    assert!(a.friends().is_empty());
    assert_eq!(b.friends().len(), MAX_FRIENDS);
    // a full inviter doesn't even dial
    b.invite("aaaa", "cat");
    assert_eq!(wait_for(&arx, 2.0, |e| matches!(e, NetEvent::Invited { .. })), None);
    a.shutdown();
    b.shutdown();
}

#[test]
fn invitee_refusal_leaves_no_friendship() {
    let p = pair(false);
    let (sent, invited) = start_pairing(&p);
    p.b.answer_invite(pairing_of(&invited), false);
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "bbbb".into(), accepted: false, pairing: pairing_of(&sent) })
    );
    assert!(p.a.friends().is_empty() && p.b.friends().is_empty());
    assert_eq!(wait_for(&p.brx, 0.5, is_connected), None);
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn inviter_refusal_after_invitee_accept_leaves_no_friendship() {
    let p = pair(false);
    let (sent, invited) = start_pairing(&p);
    p.b.answer_invite(pairing_of(&invited), true);
    wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::InviteAnswered { .. })).unwrap();
    p.a.answer_invite(pairing_of(&sent), false); // e.g. the SAS did not match
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "aaaa".into(), accepted: false, pairing: pairing_of(&invited) })
    );
    assert!(p.a.friends().is_empty() && p.b.friends().is_empty());
    assert_eq!(wait_for(&p.brx, 0.3, is_connected), None);
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn unconfirmed_pairing_times_out() {
    let p = pair(false);
    let (sent, _) = start_pairing(&p);
    assert_eq!(
        wait_for(&p.arx, 8.0, |e| matches!(e, NetEvent::InviteAnswered { .. })),
        Some(NetEvent::InviteAnswered { from: "bbbb".into(), accepted: false, pairing: pairing_of(&sent) })
    );
    assert!(p.a.friends().is_empty() && p.b.friends().is_empty());
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn paired_keys_survive_restart() {
    let p = pair(false);
    let (sent, invited) = start_pairing(&p);
    p.b.answer_invite(pairing_of(&invited), true);
    p.a.answer_invite(pairing_of(&sent), true);
    wait_for(&p.arx, 5.0, is_connected).unwrap();
    wait_for(&p.brx, 5.0, is_connected).unwrap();
    let (fa, fb) = (p.a.friends(), p.b.friends());
    p.a.shutdown();
    p.b.shutdown();
    // The app persisted static keys + friends(); restart reconnects silently.
    let (a2, arx2) = Net::start(cfg("aaaa", "Ann", p.ka, p.pa, p.pb, fa)).unwrap();
    let (b2, brx2) = Net::start(cfg("bbbb", "Bob", p.kb, p.pb, p.pa, fb)).unwrap();
    let pairing = |e: &NetEvent| is_connected(e) || matches!(e, NetEvent::Invited { .. } | NetEvent::InviteSent { .. });
    assert_eq!(wait_for(&arx2, 5.0, pairing), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert_eq!(wait_for(&brx2, 5.0, pairing), Some(NetEvent::Connected { id: "aaaa".into(), name: "Ann".into() }));
    a2.shutdown();
    b2.shutdown();
}

#[test]
fn commitment_mismatch_aborts_pairing_and_sas_formula_matches() {
    // Raw stranger dials Net (so Net is the invitee).
    let (net, rx, port) = start_sniffed(key(), vec![]);

    // 1. Wrong reveal: aborted, no Invited.
    let mut p = raw_connect(port, &key(), "zzz1");
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    let r = p.recv_msg().unwrap();
    assert_eq!(r["type"], "pair_random");
    p.send(json!({"type": "pair_reveal", "nonce": hex(&random32())})).unwrap();
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Invited { .. })), None);

    // 2. Correct reveal: Invited with SAS = BLAKE2s(hash || nonce_i || r_r).
    let mut p = raw_connect(port, &key(), "zzz2");
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    let r = p.recv_msg().unwrap();
    let r_r = unhex(r["random"].as_str().unwrap());
    p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
    let ev = wait_for(&rx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    assert_eq!(sas_of(&ev), expected_sas(&p.hash, &n, &r_r));
    net.shutdown();
}

#[test]
fn confirmation_before_sas_is_a_protocol_error() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&random32()]))})).unwrap();
    p.recv_msg().unwrap(); // pair_random
    p.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.3, |e| matches!(e, NetEvent::Invited { .. } | NetEvent::Connected { .. })), None);
    net.shutdown();
}

#[test]
fn stranger_invites_are_capped() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut admitted = 0;
    let mut peers = Vec::new();
    for i in 0..6 {
        let Ok(s) = TcpStream::connect(local(port)) else { continue };
        let Ok(mut p) = handshake(s, &key(), true) else { continue }; // over the inbound cap
        if p.send(hello(&format!("strange{i}"), "S")).is_err() || p.recv_msg().is_err() {
            continue;
        }
        let n = random32();
        p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
        match p.recv_msg() {
            Ok(v) if v["type"] == "pair_random" => {
                p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
                admitted += 1;
            }
            Ok(v) => assert_eq!(v, json!({"type": "invite_reply", "accepted": false})),
            Err(_) => {}
        }
        peers.push(p);
    }
    let mut invited = 0;
    while wait_for(&rx, 1.0, |e| matches!(e, NetEvent::Invited { .. })).is_some() {
        invited += 1;
    }
    assert!(admitted >= 1);
    assert_eq!(invited, admitted);
    assert!(invited <= 3, "per-address pairing limit (got {invited})");
    net.shutdown();
}

// ---- friends ------------------------------------------------------------------

#[test]
fn friends_auto_reconnect_after_restart() {
    let p = connected_pair();
    p.a.shutdown();
    assert_eq!(wait_for(&p.brx, 6.0, is_disconnected), Some(NetEvent::Disconnected { id: "aaaa".into() }));
    let (a2, arx2) = Net::start(cfg("aaaa", "Ann", p.ka, p.pa, p.pb, vec![friend("bbbb", &p.kb)])).unwrap();
    assert_eq!(wait_for(&arx2, 5.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert!(wait_for(&p.brx, 5.0, is_connected).is_some());
    // Restart the larger id too (it never dials; the smaller one must).
    p.b.shutdown();
    wait_for(&arx2, 6.0, is_disconnected).unwrap();
    let (b2, brx2) = Net::start(cfg("bbbb", "Bob", p.kb, p.pb, p.pa, vec![friend("aaaa", &p.ka)])).unwrap();
    assert!(wait_for(&brx2, 5.0, is_connected).is_some());
    a2.shutdown();
    b2.shutdown();
}

#[test]
fn visit_messages_round_trip() {
    let p = connected_pair();
    p.a.send("bbbb", sample_visit());
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "aaaa".into(), msg: sample_visit() })
    );
    p.b.send("aaaa", Visit::leave(42));
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "bbbb".into(), msg: Visit::leave(42) })
    );
    // 다른 친구에게 더 놀러 가는 손님
    let hop = Visit::Leave { visit_id: 42, next: Some("cccc".into()), side: Some(Side::Right), homesick_in: Some(77.0), fling: None };
    p.b.send("aaaa", hop.clone());
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "bbbb".into(), msg: hop })
    );
    // 편지를 들려 보낸 방문, 그리고 주인이 다음 친구에게 이어 보낼 때도 같은 편지를 다시 싣는다 (via)
    for via in [None, Some("cccc".to_string())] {
        let Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, homesick_in, fling, .. } = sample_visit() else { unreachable!() };
        let letter = Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, via, homesick_in, fling, note: Some("놀러 왔어 💌".into()) };
        p.a.send("bbbb", letter.clone());
        assert_eq!(
            wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
            Some(NetEvent::Received { from: "aaaa".into(), msg: letter })
        );
    }
    p.a.send("bbbb", Visit::ComeHome { visit_id: 42 });
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "aaaa".into(), msg: Visit::ComeHome { visit_id: 42 } })
    );
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn rename_reaches_connected_friend_and_later_hellos() {
    let p = connected_pair();
    p.a.set_name("Annie");
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Renamed { .. })),
        Some(NetEvent::Renamed { id: "aaaa".into(), name: "Annie".into() })
    );
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn friend_rehello_with_same_id_renames_and_is_clamped() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 다른 id를 대는 hello는 무시 (이름을 바꾸지 않는다)
    p.send(friend_hello("yyyy", "Mallory")).unwrap();
    p.send(friend_hello("zzzz", &"z".repeat(100))).unwrap();
    let ev = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Renamed { .. })).expect("renamed");
    assert_eq!(ev, NetEvent::Renamed { id: "zzzz".into(), name: "z".repeat(64) });
    net.shutdown();
}

#[test]
fn new_links_hello_with_the_new_name() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    net.set_name("Neo");
    let mut p = handshake(TcpStream::connect(local(port)).unwrap(), &kz, true).unwrap();
    p.send(friend_hello("zzzz", "Raw")).unwrap();
    let h = p.recv_msg().unwrap();
    assert_eq!((h["type"].as_str(), h["name"].as_str()), (Some("hello"), Some("Neo")));
    wait_for(&rx, 5.0, is_connected).unwrap();
    net.shutdown();
}

#[test]
fn asks_reach_connected_friends_only() {
    let p = connected_pair();
    p.a.ask("bbbb", "invite");
    assert_eq!(
        wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Asked { .. })),
        Some(NetEvent::Asked { from: "aaaa".into(), ask: "invite".into() })
    );
    p.b.ask("aaaa", "busy");
    assert_eq!(wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::Asked { .. })), Some(NetEvent::Asked { from: "bbbb".into(), ask: "busy".into() }));
    // 모르는 상대에게는 보내지 않는다
    p.a.ask("zzzz", "invite");
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn stranger_ask_is_ignored_and_long_asks_are_clamped() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(json!({"type": "ping", "ask": "invite"})).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Asked { .. })), None);
    net.shutdown();
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "ping", "ask": "x".repeat(500)})).unwrap();
    let Some(NetEvent::Asked { ask, .. }) = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Asked { .. })) else { panic!("asked") };
    assert_eq!(ask.chars().count(), 64);
    net.shutdown();
}

#[test]
fn unchanged_or_flooded_renames_and_asks_are_limited() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 같은 이름으로 다시 hello — 이벤트 없음
    p.send(friend_hello("zzzz", "Raw")).unwrap();
    assert_eq!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Renamed { .. })), None);
    // 이름 바꾸기·부탁을 마구 보내도 2초에 하나 꼴로만 (처음 둘까지는 바로)
    for i in 0..20 {
        p.send(friend_hello("zzzz", &format!("N{i}"))).unwrap();
        p.send(json!({"type": "ping", "ask": "invite"})).unwrap();
    }
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut events = 0;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::Renamed { .. } | NetEvent::Asked { .. }) => events += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert!((1..=3).contains(&events), "rate limited: {events}");
    net.shutdown();
}

#[test]
fn stranger_rehello_does_not_rename() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(hello("zzzz", "Eve")).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Renamed { .. })), None);
    net.shutdown();
}

#[test]
fn peer_shutdown_is_noticed_within_six_seconds() {
    let p = connected_pair();
    p.b.shutdown();
    assert_eq!(wait_for(&p.arx, 6.0, is_disconnected), Some(NetEvent::Disconnected { id: "bbbb".into() }));
    p.a.shutdown();
}

#[test]
fn unfriend_disconnects_and_forgets_on_both_sides() {
    let p = connected_pair();
    p.b.unfriend("aaaa");
    assert_eq!(wait_for(&p.arx, 5.0, is_disconnected), Some(NetEvent::Disconnected { id: "bbbb".into() }));
    assert!(wait_for(&p.brx, 5.0, is_disconnected).is_some());
    assert!(p.a.friends().is_empty());
    assert!(p.b.friends().is_empty());
    assert_eq!(wait_for(&p.arx, 1.0, is_connected), None, "no reconnection");
    p.a.shutdown();
    p.b.shutdown();
}

// ---- raw protocol / defensive parsing --------------------------------------------

#[test]
fn raw_friend_with_pinned_key_connects_and_gets_pings() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    assert_eq!(wait_for(&rx, 5.0, is_connected), Some(NetEvent::Connected { id: "zzzz".into(), name: "Raw".into() }));
    assert_eq!(p.recv().unwrap(), json!({"type": "ping"}));
    p.send(json!({"type": "leave", "visit_id": 9})).unwrap();
    assert_eq!(
        wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "zzzz".into(), msg: Visit::leave(9) })
    );
    net.shutdown();
}

#[test]
fn impersonator_with_friends_id_but_other_key_is_rejected() {
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &key())]);
    let mut p = raw_connect_claiming(port, &key(), "zzzz");
    let _ = p.send(json!({"type": "leave", "visit_id": 1}));
    let _ = p.send(json!({"type": "unfriend"}));
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.5, |e| is_connected(e) || matches!(e, NetEvent::Received { .. })), None);
    assert_eq!(net.friends().len(), 1, "attacker could not unfriend");
    net.shutdown();
}

#[test]
fn friends_key_under_another_id_is_rejected() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_connect_claiming(port, &kz, "yyyy");
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.3, is_connected), None);
    net.shutdown();
}

#[test]
fn unknown_type_drops_connection() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "bogus"})).unwrap();
    assert!(wait_for(&rx, 2.0, is_disconnected).is_some());
    assert!(closed_by_peer(&mut p.s));
    net.shutdown();
}

#[test]
fn garbage_and_zero_length_frames_drop_connection() {
    let (net, _rx, port) = start_sniffed(key(), vec![]);
    let mut s = TcpStream::connect(local(port)).unwrap();
    write_frame(&mut s, b"definitely not a noise message").unwrap();
    assert!(closed_by_peer(&mut s));
    let mut s = TcpStream::connect(local(port)).unwrap();
    s.write_all(&[0, 0]).unwrap();
    assert!(closed_by_peer(&mut s));
    net.shutdown();
}

#[test]
fn forged_transport_frame_drops_connection() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    write_frame(&mut p.s, &random32()).unwrap();
    assert!(wait_for(&rx, 2.0, is_disconnected).is_some());
    net.shutdown();
}

#[test]
fn visit_from_stranger_is_ignored() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(serde_json::to_value(sample_visit()).unwrap()).unwrap();
    p.send(json!({"type": "come_home", "visit_id": 42})).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Received { .. } | NetEvent::Connected { .. })), None);
    assert!(closed_by_peer(&mut p.s), "idle stranger link is closed");
    net.shutdown();
}

#[test]
fn silent_friend_times_out() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let _p = raw_friend_connect(port, &kz, "zzzz"); // never sends again
    wait_for(&rx, 5.0, is_connected).unwrap();
    let t = Instant::now();
    assert_eq!(wait_for(&rx, 6.0, is_disconnected), Some(NetEvent::Disconnected { id: "zzzz".into() }));
    assert!(t.elapsed() >= Duration::from_millis(800));
    net.shutdown();
}

// ---- attacks ------------------------------------------------------------------

/// Keep announcing `id` at `tcp_port` to `to` until `stop`.
fn spoof_announcements(id: &'static str, tcp_port: u16, to: u16, every: Duration, stop: Arc<AtomicBool>) {
    let u = UdpSocket::bind("127.0.0.1:0").unwrap();
    std::thread::spawn(move || {
        let msg = format!(r#"{{"app":"screenpet","v":1,"id":"{id}","name":"Bob","port":{tcp_port}}}"#);
        while !stop.load(Ordering::SeqCst) {
            let _ = u.send_to(msg.as_bytes(), local(to));
            std::thread::sleep(every);
        }
    });
}

#[test]
fn mitm_in_the_middle_of_pairing_gets_different_sas() {
    let stop = Arc::new(AtomicBool::new(false));
    // B only announces to a sniffer, so A can reach "bbbb" only via the attacker.
    let sniffer = UdpSocket::bind("127.0.0.1:0").unwrap();
    sniffer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", key(), 0, sniffer.local_addr().unwrap().port(), vec![])).unwrap();
    let b_port = sniff_port(&sniffer, "bbbb");
    let (a, arx) = Net::start(cfg("aaaa", "Ann", key(), 0, free_udp_port(), vec![])).unwrap();
    let evil = TcpListener::bind("127.0.0.1:0").unwrap();
    spoof_announcements("bbbb", evil.local_addr().unwrap().port(), a.discovery_port(), Duration::from_millis(100), stop.clone());
    wait_for(&arx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();

    let mitm = std::thread::spawn(move || -> io::Result<()> {
        let (s, _) = evil.accept()?;
        let mut to_a = handshake(s, &key(), false)?; // attacker as "bbbb" toward A
        to_a.recv_msg()?; // A's hello
        to_a.send(hello("bbbb", "Bob"))?;
        let invite = to_a.recv_msg()?;
        let mut to_b = handshake(TcpStream::connect(local(b_port))?, &key(), true)?; // as "aaaa" toward B
        to_b.send(hello("aaaa", "Ann"))?;
        to_b.recv_msg()?; // B's hello
        to_b.send(invite)?; // relay commit
        let random = to_b.recv_msg()?;
        to_a.send(random)?; // relay r_r
        let reveal = to_a.recv_msg()?;
        to_b.send(reveal)?; // relay nonce_i
        std::thread::sleep(Duration::from_secs(1));
        Ok(())
    });
    a.invite("bbbb", "cat");
    let sent = wait_for(&arx, 5.0, |e| matches!(e, NetEvent::InviteSent { .. })).expect("A shows a SAS");
    let invited = wait_for(&brx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("B shows a SAS");
    assert_ne!(sas_of(&sent), sas_of(&invited), "users would notice the mismatch");
    mitm.join().unwrap().unwrap();
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

#[test]
fn relay_attacker_cannot_inject_or_replay() {
    let stop = Arc::new(AtomicBool::new(false));
    let (ka, kb) = (key(), key());
    let sniffer = UdpSocket::bind("127.0.0.1:0").unwrap();
    sniffer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let (b, brx) =
        Net::start(cfg("bbbb", "Bob", kb, 0, sniffer.local_addr().unwrap().port(), vec![friend("aaaa", &ka)])).unwrap();
    let b_port = sniff_port(&sniffer, "bbbb");
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, 0, free_udp_port(), vec![friend("bbbb", &kb)])).unwrap();
    // A reaches B only through a forwarding proxy (two sockets).
    let proxy = TcpListener::bind("127.0.0.1:0").unwrap();
    spoof_announcements("bbbb", proxy.local_addr().unwrap().port(), a.discovery_port(), Duration::from_millis(100), stop.clone());
    let recorded: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let (to_b_tx, to_b_rx) = std::sync::mpsc::channel::<TcpStream>();
    {
        let recorded = recorded.clone();
        std::thread::spawn(move || {
            let (mut from_a, _) = proxy.accept().unwrap();
            let mut to_b = TcpStream::connect(local(b_port)).unwrap();
            let (mut b_r, mut a_w) = (to_b.try_clone().unwrap(), from_a.try_clone().unwrap());
            to_b_tx.send(to_b.try_clone().unwrap()).unwrap();
            std::thread::spawn(move || {
                let _ = io::copy(&mut b_r, &mut a_w);
            });
            while let Ok(f) = read_frame(&mut from_a) {
                recorded.lock().unwrap().push(f.clone());
                if write_frame(&mut to_b, &f).is_err() {
                    break;
                }
            }
        });
    }
    // The proxied link works end to end: it is just transport.
    wait_for(&arx, 6.0, is_connected).expect("A connected via proxy");
    wait_for(&brx, 6.0, is_connected).expect("B connected via proxy");
    a.send("bbbb", Visit::leave(1));
    assert!(wait_for(&brx, 3.0, |e| matches!(e, NetEvent::Received { .. })).is_some());
    // The attacker replays an earlier A->B transport frame (A's hello).
    let mut to_b = to_b_rx.recv().unwrap();
    let replay = recorded.lock().unwrap()[2].clone();
    write_frame(&mut to_b, &replay).unwrap();
    assert_eq!(
        wait_for(&brx, 3.0, |e| is_disconnected(e) || matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Disconnected { id: "aaaa".into() }),
        "replayed frame fails authentication and is never delivered"
    );
    assert_eq!(b.friends(), vec![friend("aaaa", &ka)], "friendship untouched");
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

/// A fake peer: accepts TCP connections and answers with garbage / silence.
fn fake_listener(stop: Arc<AtomicBool>) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let mut held = Vec::new();
        while !stop.load(Ordering::SeqCst) {
            if let Ok((s, _)) = l.accept() {
                held.push(s);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
    port
}

#[test]
fn spoofed_friend_address_does_not_block_real_friend() {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &kb)])).unwrap();
    let fake = fake_listener(stop.clone());
    spoof_announcements("bbbb", fake, pa, Duration::from_millis(100), stop.clone());
    assert_eq!(wait_for(&arx, 2.0, is_connected), None);
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    assert_eq!(wait_for(&arx, 10.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert!(wait_for(&brx, 5.0, is_connected).is_some());
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

#[test]
fn dialed_peer_answering_with_other_id_is_rejected() {
    // The spoofed "bbbb" address is a real Net whose id is "cccc".
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &kb)])).unwrap();
    let (c, _crx, c_port) = start_sniffed(key(), vec![]);
    spoof_announcements("bbbb", c_port, pa, Duration::from_millis(100), stop.clone());
    assert_eq!(wait_for(&arx, 2.0, is_connected), None);
    let (b, _brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    assert_eq!(wait_for(&arx, 10.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
    c.shutdown();
}

#[test]
fn connection_flood_does_not_kill_accept_loop() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut flood: Vec<TcpStream> = (0..60).filter_map(|_| TcpStream::connect(local(port)).ok()).collect();
    // Closed = evicted from the bounded unverified pool by newer connections;
    // a read timeout = still held (awaiting handshake).
    let refused = flood
        .iter_mut()
        .map(|s| {
            let s: &mut TcpStream = s;
            s.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
            match s.read(&mut [0u8; 1]) {
                Ok(0) => true,
                Err(e) => e.kind() == io::ErrorKind::ConnectionReset,
                Ok(_) => false,
            }
        })
        .filter(|refused| *refused)
        .count();
    assert!(refused >= 50, "only {refused} of 60 flood connections were evicted");
    drop(flood);
    std::thread::sleep(Duration::from_millis(300));
    let _p = raw_friend_connect(port, &kz, "zzzz");
    assert!(wait_for(&rx, 5.0, is_connected).is_some(), "accept loop still serves friends");
    net.shutdown();
}

#[test]
fn announcement_flood_does_not_block_reconnect() {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &kb)])).unwrap();
    let flooder = UdpSocket::bind("127.0.0.1:0").unwrap();
    for i in 0..400 {
        let msg = format!(r#"{{"app":"screenpet","v":1,"id":"flood{i}","name":"F","port":{}}}"#, 1000 + i);
        let _ = flooder.send_to(msg.as_bytes(), local(pa));
    }
    let fake = fake_listener(stop.clone());
    spoof_announcements("bbbb", fake, pa, Duration::from_millis(50), stop.clone());
    let (b, _brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    let deadline = Instant::now() + Duration::from_secs(12);
    let (mut seen, mut got) = (0, None);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match arx.recv_timeout(left) {
            Ok(NetEvent::PeerSeen { .. }) => seen += 1,
            Ok(e @ NetEvent::Connected { .. }) => {
                got = Some(e);
                break;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert_eq!(got, Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert!(seen <= 64, "seen-ids map is capped (got {seen})");
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

// ---- round 3: availability, terminal pairing, squatters, backoff ------------------

/// UDP tee: records each announcement's (id → tcp port) and forwards it to
/// `to`, so tests learn TCP ports while peers still hear each other.
fn tee(to: u16, stop: Arc<AtomicBool>) -> (u16, Arc<Mutex<std::collections::HashMap<String, u16>>>) {
    let u = UdpSocket::bind("127.0.0.1:0").unwrap();
    u.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let port = u.local_addr().unwrap().port();
    let ports: Arc<Mutex<std::collections::HashMap<String, u16>>> = Arc::default();
    let p = ports.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 2048];
        while !stop.load(Ordering::SeqCst) {
            if let Ok((n, _)) = u.recv_from(&mut buf) {
                if let Ok(v) = serde_json::from_slice::<Value>(&buf[..n]) {
                    if let (Some(id), Some(port)) = (v["id"].as_str(), v["port"].as_u64()) {
                        p.lock().unwrap().insert(id.to_string(), port as u16);
                    }
                }
                let _ = u.send_to(&buf[..n], local(to));
            }
        }
    });
    (port, ports)
}

/// TCP connect to 127.0.0.1:`port` from a chosen loopback source address.
fn connect_from(src: [u8; 4], port: u16) -> Option<TcpStream> {
    use socket2::{Domain, Socket, Type};
    let s = Socket::new(Domain::IPV4, Type::STREAM, None).ok()?;
    s.bind(&SocketAddr::from((src, 0)).into()).ok()?;
    s.connect(&local(port).into()).ok()?;
    Some(s.into())
}

fn wait_port(ports: &Mutex<std::collections::HashMap<String, u16>>, id: &str) -> u16 {
    let t = Instant::now();
    loop {
        if let Some(p) = ports.lock().unwrap().get(id) {
            return *p;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "no announcement from {id}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn friends_reconnect_despite_unverified_connection_flood() {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (tee_a, ports) = tee(pb, stop.clone());
    let (tee_b, ports_b) = tee(pa, stop.clone());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, tee_a, vec![friend("bbbb", &kb)])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, tee_b, vec![friend("aaaa", &ka)])).unwrap();
    wait_for(&arx, 5.0, is_connected).expect("initial connect");
    wait_for(&brx, 5.0, is_connected).expect("initial connect");
    // Attacker keeps renewing idle inbound connections to both peers, holding
    // far more than the unverified caps at all times.
    let attacker = {
        let (stop, ports, ports_b) = (stop.clone(), ports.clone(), ports_b.clone());
        std::thread::spawn(move || {
            let mut held = std::collections::VecDeque::new();
            let mut n = 0u32;
            while !stop.load(Ordering::SeqCst) {
                let targets: Vec<u16> =
                    ports.lock().unwrap().values().chain(ports_b.lock().unwrap().values()).copied().collect();
                for p in targets {
                    // Several attacker source addresses (not the friends' 127.0.0.1).
                    n += 1;
                    if let Some(s) = connect_from([127, 0, 0, 2 + (n % 8) as u8], p) {
                        held.push_back(s);
                    }
                }
                // Hold each connection ~2 s (longer than the pre-auth deadline).
                while held.len() > 600 {
                    held.pop_front();
                }
                std::thread::sleep(Duration::from_millis(7));
            }
        })
    };
    std::thread::sleep(Duration::from_millis(500));
    // Restart A: B's listener has been under flood all along, and A2's
    // links must still get through.
    a.shutdown();
    wait_for(&brx, 6.0, is_disconnected).expect("A gone");
    let (a2, arx2) = Net::start(cfg("aaaa", "Ann", ka, pa, tee_a, vec![friend("bbbb", &kb)])).unwrap();
    assert_eq!(
        wait_for(&arx2, 3.0, is_connected),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }),
        "friends reconnect within a few seconds while the flood continues"
    );
    assert!(wait_for(&brx, 5.0, is_connected).is_some());
    stop.store(true, Ordering::SeqCst);
    attacker.join().unwrap();
    a2.shutdown();
    b.shutdown();
}

#[test]
fn friend_announcements_processed_under_spoofed_source_flood() {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    // B never hears A (A announces to a dead port), so only A can dial: A
    // must process B's announcement despite the flood.
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, free_udp_port(), vec![friend("bbbb", &kb)])).unwrap();
    let flood = {
        let stop = stop.clone();
        // Many distinct loopback source addresses fill the rate table; one
        // more floods from 127.0.0.1 (B's address) past its per-IP limit.
        let sources: Vec<UdpSocket> = (0..300u32)
            .filter_map(|i| UdpSocket::bind(SocketAddr::from(([127, 0, 1 + (i / 250) as u8, 1 + (i % 250) as u8], 0))).ok())
            .collect();
        let local_flood = UdpSocket::bind("127.0.0.1:0").unwrap();
        std::thread::spawn(move || {
            let mut n = 0u32;
            while !stop.load(Ordering::SeqCst) {
                for s in &sources {
                    n += 1;
                    let msg = format!(r#"{{"app":"screenpet","v":1,"id":"spoof{n}","name":"F","port":9}}"#);
                    let _ = s.send_to(msg.as_bytes(), local(pa));
                }
                for _ in 0..40 {
                    n += 1;
                    let msg = format!(r#"{{"app":"screenpet","v":1,"id":"local{n}","name":"F","port":9}}"#);
                    let _ = local_flood.send_to(msg.as_bytes(), local(pa));
                }
                // ~1500/s from spoofed sources (table full) + ~200/s from
                // 127.0.0.1 (4x its budget) — over every app-level limit but
                // below what overflows the kernel socket buffer (an OS limit
                // no app can fix).
                std::thread::sleep(Duration::from_millis(200));
            }
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    let (b, _brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    assert_eq!(
        wait_for(&arx, 8.0, is_connected),
        Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() })
    );
    stop.store(true, Ordering::SeqCst);
    flood.join().unwrap();
    a.shutdown();
    b.shutdown();
}

/// Raw stranger `id` pairs with Net (Net = invitee) up to the SAS stage.
/// Returns the link and the Invited event.
fn raw_pairing(port: u16, rx: &Receiver<NetEvent>, id: &str) -> (Peer, NetEvent) {
    let mut p = raw_connect(port, &key(), id);
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "pair_random");
    p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
    let ev = wait_for(rx, 5.0, |e| matches!(e, NetEvent::Invited { from, .. } if from == id)).expect("Invited");
    (p, ev)
}

#[test]
fn confirmations_after_pairing_timeout_never_pin() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    // Case 1: Net confirms in time, the remote confirmation arrives late.
    let (mut late_remote, ev1) = raw_pairing(port, &rx, "zzz1");
    net.answer_invite(pairing_of(&ev1), true);
    // Case 2: the remote confirms in time, Net's confirmation comes late.
    let (mut early_remote, ev2) = raw_pairing(port, &rx, "zzz2");
    early_remote.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    // Keep both links alive (pings) past the pairing deadline.
    let until = Instant::now() + fast().invite_timeout + Duration::from_millis(500);
    while Instant::now() < until {
        let _ = late_remote.send(json!({"type": "ping"}));
        let _ = early_remote.send(json!({"type": "ping"}));
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = late_remote.send(json!({"type": "invite_reply", "accepted": true}));
    net.answer_invite(pairing_of(&ev2), true);
    assert_eq!(wait_for(&rx, 1.5, is_connected), None, "nothing pins after the deadline");
    assert!(net.friends().is_empty());
    net.shutdown();
}

#[test]
fn squatters_pairing_under_claimed_ids_do_not_block_or_hijack_pairing() {
    let stop = Arc::new(AtomicBool::new(false));
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (tee_a, ports_a) = tee(pb, stop.clone());
    let (tee_b, ports_b) = tee(pa, stop.clone());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, tee_a, vec![])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, tee_b, vec![])).unwrap();
    let (a_port, b_port) = (wait_port(&ports_a, "aaaa"), wait_port(&ports_b, "bbbb"));
    wait_for(&arx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })).unwrap();
    // Squatters claim each other's ids and even start pairings under them.
    let (_squat_a, _) = raw_pairing(a_port, &arx, "bbbb");
    let (_squat_b, squat_invited) = raw_pairing(b_port, &brx, "aaaa");
    // An inbound pairing claiming "bbbb" does not block A's own invite.
    a.invite("bbbb", "cat");
    let sent = wait_for(&arx, 5.0, |e| matches!(e, NetEvent::InviteSent { .. } | NetEvent::InviteAnswered { .. }));
    assert!(matches!(sent, Some(NetEvent::InviteSent { .. })), "invite proceeds: {sent:?}");
    let sent = sent.unwrap();
    // B now has two requests "from aaaa"; the user answers the one whose SAS
    // matches A's screen, by its pairing token.
    let real = wait_for(&brx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("B is asked");
    assert_eq!(sas_of(&real), sas_of(&sent));
    assert_ne!(pairing_of(&real), pairing_of(&squat_invited));
    b.answer_invite(pairing_of(&real), true);
    a.answer_invite(pairing_of(&sent), true);
    assert_eq!(wait_for(&arx, 5.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert_eq!(wait_for(&brx, 5.0, is_connected), Some(NetEvent::Connected { id: "aaaa".into(), name: "Ann".into() }));
    assert_eq!(b.friends(), vec![friend("aaaa", &ka)], "B pinned the real A, not the squatter");
    assert_eq!(a.friends(), vec![friend("bbbb", &kb)]);
    // The squatter's pairing token is dead now.
    b.answer_invite(pairing_of(&squat_invited), true);
    assert_eq!(b.friends(), vec![friend("aaaa", &ka)]);
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

fn count_events(rx: &Receiver<NetEvent>, secs: f64, pred: impl Fn(&NetEvent) -> bool) -> (usize, bool) {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    let (mut n, mut connected) = (0, false);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(e) => {
                connected |= is_connected(&e);
                if pred(&e) {
                    n += 1;
                }
            }
            Err(_) => break,
        }
    }
    (n, connected)
}

#[test]
fn one_sided_pin_reports_friend_forgot_us_and_backs_off() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    // A pinned B, but B never pinned A (e.g. pairing finished on one side).
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &kb)])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![])).unwrap();
    let (mismatches, connected) =
        count_events(&arx, 4.0, |e| *e == NetEvent::FriendForgotUs { id: "bbbb".into() });
    assert!(!connected);
    // Announcements arrive every 150 ms; backoff (1.5 s, doubling) bounds redials.
    assert!((1..=3).contains(&mismatches), "{mismatches} redials in 4 s");
    assert_eq!(wait_for(&brx, 0.1, is_connected), None);
    a.shutdown();
    b.shutdown();
}

#[test]
fn wrong_key_answer_for_friend_is_silent_and_backed_off() {
    // A spoofed address answers for friend "bbbb" with a different key (and
    // even claims to have A pinned). Not proven: no event, just backoff.
    let stop = Arc::new(AtomicBool::new(false));
    let (ka, kb) = (key(), key());
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, 0, free_udp_port(), vec![friend("bbbb", &kb)])).unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.set_nonblocking(true).unwrap();
    let fake_port = l.local_addr().unwrap().port();
    let accepts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let (stop, accepts) = (stop.clone(), accepts.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::SeqCst) {
                match l.accept() {
                    Ok((s, _)) => {
                        accepts.fetch_add(1, Ordering::SeqCst);
                        s.set_nonblocking(false).unwrap();
                        std::thread::spawn(move || {
                            if let Ok(mut p) = handshake(s, &key(), false) {
                                let _ = p.send(friend_hello("bbbb", "Evil"));
                                let _ = p.recv();
                            }
                        });
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
    }
    spoof_announcements("bbbb", fake_port, a.discovery_port(), Duration::from_millis(100), stop.clone());
    let (events, connected) = count_events(&arx, 4.0, |e| matches!(e, NetEvent::FriendForgotUs { .. }));
    assert!(!connected);
    assert_eq!(events, 0, "unproven wrong key is not reported");
    let n = accepts.load(Ordering::SeqCst);
    assert!((1..=3).contains(&n), "{n} dials to the bad candidate in 4 s");
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
}
