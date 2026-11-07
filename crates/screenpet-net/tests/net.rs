//! Localhost integration tests: Net instances on distinct discovery ports,
//! each "broadcasting" (UDP unicast) to the other's port, plus hand-rolled
//! Noise clients (via `snow`) for the protocol / attack cases.

mod common;

use common::{blake2s, fast, free_udp_port, friend, hex, is_connected, is_disconnected, unhex, wait_for};
use screenpet_net::{new_install_id, DeviceSecret, Fling, Friend, LadderMsg, LinkError, Net, NetConfig, NetEvent, Side, Steer, Timing, Toss, VSide, Visit, WireCare, MAX_DEVICES, MAX_FRIENDS, SAS_EMOJI};
use serde_json::{json, Value};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---- helpers ---------------------------------------------------------------

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn key() -> [u8; 32] {
    Net::generate_static_key()
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
        device: None,
        introduced: vec![],
        pending: vec![],
        pending_added: vec![],
    }
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
        vside: None,
        via: None,
        homesick_in: None,
        fling: None,
        note: None,
        gift: None,
        style: None, boost: None, of: None }
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
fn a_taken_discovery_port_stops_a_lan_only_net() {
    let holder = UdpSocket::bind("0.0.0.0:0").unwrap();
    let port = holder.local_addr().unwrap().port();
    assert!(Net::start(cfg("solo", "Solo", key(), port, free_udp_port(), vec![])).is_err(), "nothing could work without discovery");
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
    // Only the caps are under test. The held pairings stay silent while the
    // next ones handshake, so on a slow runner `fast()` timeouts would free
    // their slots mid-test and let a later one in.
    let mut c = cfg("mmmm", "Mia", key(), 0, 0, vec![]);
    c.timing.conn_timeout = Duration::from_secs(30);
    c.timing.invite_timeout = Duration::from_secs(30);
    let (net, rx, port) = sniffed(c);
    let mut peers = Vec::new();
    let mut admitted = Vec::new();
    // 주소마다 3번씩: 한 주소에서 2개까지, 전체 4개까지 (127.0.0.6은 전체 상한에 걸린다)
    for src in [1u8, 5, 6] {
        let mut from_here = 0;
        for i in 0..3 {
            let s = connect_from([127, 0, 0, src], port).expect("connection");
            let mut p = handshake(s, &key(), true).expect("under every connection limit");
            p.send(hello(&format!("strange{src}x{i}"), "S")).unwrap();
            assert_eq!(p.recv_msg().unwrap()["type"], "hello");
            let n = random32();
            p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
            let v = p.recv_msg().unwrap();
            if v["type"] == "pair_random" {
                p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
                from_here += 1;
                // 다음 시도 전에 표시까지 끝낸다 (표시된 짝맞추기만 자리를 지킨다)
                wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
            } else {
                assert_eq!(v, json!({"type": "invite_reply", "accepted": false}));
            }
            peers.push(p);
        }
        admitted.push(from_here);
    }
    assert_eq!(admitted, vec![2, 2, 0], "per address, then overall");
    assert_eq!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::Invited { .. })), None, "nothing else reached the user");
    net.shutdown();
}

// ---- friends ------------------------------------------------------------------

#[test]
fn the_larger_id_dials_after_a_grace_when_the_smaller_cannot_hear_it() {
    // A(작은 id)는 B의 알림을 못 듣는다: B가 유예 뒤에 대신 건다
    let (ka, kb) = (key(), key());
    let pb = free_udp_port();
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, 0, pb, vec![friend("bbbb", &kb)])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, free_udp_port(), vec![friend("aaaa", &ka)])).unwrap();
    assert!(wait_for(&brx, 5.0, is_connected).is_some(), "B dialed");
    assert!(wait_for(&arx, 5.0, is_connected).is_some());
    a.shutdown();
    b.shutdown();
}

#[test]
fn friends_auto_reconnect_after_restart() {
    let p = connected_pair();
    p.a.shutdown();
    assert_eq!(wait_for(&p.brx, 6.0, is_disconnected), Some(NetEvent::Disconnected { id: "aaaa".into() }));
    let (a2, arx2) = Net::start(cfg("aaaa", "Ann", p.ka, p.pa, p.pb, vec![friend("bbbb", &p.kb)])).unwrap();
    assert_eq!(wait_for(&arx2, 5.0, is_connected), Some(NetEvent::Connected { id: "bbbb".into(), name: "Bob".into() }));
    assert!(wait_for(&p.brx, 5.0, is_connected).is_some());
    // Restart the larger id too (the smaller one dials it at once).
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
    let hop = Visit::Leave { visit_id: 42, next: Some("cccc".into()), side: Some(Side::Right), vside: None, homesick_in: Some(77.0), fling: None, note: None, boost: None };
    p.b.send("aaaa", hop.clone());
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "bbbb".into(), msg: hop })
    );
    // 편지를 들려 보낸 방문, 그리고 주인이 다음 친구에게 이어 보낼 때도 같은 편지를 다시 싣는다 (via)
    for via in [None, Some("cccc".to_string())] {
        let Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, homesick_in, fling, .. } = sample_visit() else { unreachable!() };
        let letter = Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, vside: None, via, homesick_in, fling, note: Some("놀러 왔어 💌".into()), gift: None, style: None, boost: None, of: None };
        p.a.send("bbbb", letter.clone());
        assert_eq!(
            wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
            Some(NetEvent::Received { from: "aaaa".into(), msg: letter })
        );
    }
    // 친구가 답장을 써서 돌려보낸 귀환
    let reply = Visit::Leave { visit_id: 42, next: None, side: None, vside: None, homesick_in: None, fling: None, note: Some("또 와 💌".into()), boost: None };
    p.b.send("aaaa", reply.clone());
    assert_eq!(
        wait_for(&p.arx, 5.0, |e| matches!(e, NetEvent::Received { .. })),
        Some(NetEvent::Received { from: "bbbb".into(), msg: reply })
    );
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
fn steers_reach_connected_friends_only() {
    let p = connected_pair();
    let st = Steer { visit_id: 7, on: true, keys: 3 };
    p.a.steer("bbbb", st);
    assert_eq!(wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Steered { .. })), Some(NetEvent::Steered { from: "aaaa".into(), steer: st }));
    // 모르는 상대에게는 보내지 않는다
    p.a.steer("zzzz", st);
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn stranger_steer_is_ignored() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(json!({"type": "ping", "steer": {"visit_id": 1, "on": true, "keys": 2}})).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Steered { .. })), None);
    net.shutdown();
}

#[test]
fn a_ping_can_carry_an_ask_and_a_steer() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "ping", "ask": "invite", "steer": {"visit_id": 9, "on": true, "keys": 1}})).unwrap();
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Asked { .. })).is_some());
    assert_eq!(
        wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Steered { .. })),
        Some(NetEvent::Steered { from: "zzzz".into(), steer: Steer { visit_id: 9, on: true, keys: 1 } })
    );
    net.shutdown();
}

#[test]
fn flooded_steers_are_rate_limited() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    let start = Instant::now();
    for i in 0..300u64 {
        p.send(json!({"type": "ping", "steer": {"visit_id": 1, "on": true, "keys": i % 4}})).unwrap();
    }
    let deadline = Instant::now() + Duration::from_millis(600);
    let mut events = 0;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::Steered { .. }) => events += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    // 처음 30개까지는 바로, 그 뒤엔 초당 30개 꼴 — 300개를 다 넘기지 않는다. The refill
    // is counted over the measured time (a slow runner takes longer to send and handle them).
    let allowed = 31 + (30.0 * start.elapsed().as_secs_f64()).ceil() as usize;
    assert!(allowed < 300, "too slow to tell");
    assert!((30..=allowed).contains(&events), "rate limited: {events} > {allowed}");
    net.shutdown();
}

#[test]
fn tosses_reach_connected_friends_with_kind_fling_and_note() {
    let p = connected_pair();
    let toss = Toss { kind: "can".into(), side: Side::Right, vside: Some(VSide::Up), fling: Some(Fling { height: 0.5, vx: -300.0, vy: 20.0 }), note: Some("hi".into()), bounced: false, scale: None, look: None };
    p.a.toss("bbbb", toss.clone());
    assert_eq!(wait_for(&p.brx, 5.0, |e| matches!(e, NetEvent::Tossed { .. })), Some(NetEvent::Tossed { from: "aaaa".into(), toss }));
    // 모르는 상대에게는 보내지 않는다
    p.a.toss("zzzz", Toss { kind: "can".into(), side: Side::Left, vside: None, fling: None, note: None, bounced: false, scale: None, look: None });
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn a_retired_object_kind_from_an_older_friend_keeps_the_link() {
    // 종류 확인은 앱 몫 (object::accept_toss가 버린다) — net은 그대로 넘기고 연결을 끊지 않는다
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "ping", "toss": {"kind": "banana", "side": "left"}})).unwrap();
    let got = wait_for(&rx, 5.0, |e| matches!(e, NetEvent::Tossed { .. } | NetEvent::Disconnected { .. }));
    assert!(matches!(&got, Some(NetEvent::Tossed { toss, .. }) if toss.kind == "banana"), "{got:?}");
    p.send(json!({"type": "ping", "toss": {"kind": "can", "side": "left"}})).unwrap();
    let got = wait_for(&rx, 5.0, |e| matches!(e, NetEvent::Tossed { .. } | NetEvent::Disconnected { .. }));
    assert!(matches!(&got, Some(NetEvent::Tossed { toss, .. }) if toss.kind == "can"), "{got:?}");
    net.shutdown();
}

#[test]
fn stranger_toss_is_ignored_and_the_link_stays() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(json!({"type": "ping", "toss": {"kind": "can", "side": "left", "note": "psst"}})).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Tossed { .. })), None);
    assert!(p.send(json!({"type": "ping"})).is_ok());
    net.shutdown();
}

#[test]
fn flooded_tosses_are_rate_limited_without_dropping_the_link() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    for _ in 0..100 {
        p.send(json!({"type": "ping", "toss": {"kind": "can", "side": "left"}})).unwrap();
    }
    let deadline = Instant::now() + Duration::from_millis(600);
    let mut events = 0;
    let mut disconnected = false;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::Tossed { .. }) => events += 1,
            Ok(NetEvent::Disconnected { .. }) => disconnected = true,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    // 처음 50개까지만 바로 (그 뒤엔 1.2초에 하나) — 연결은 끊지 않는다
    assert_eq!(events, 50, "rate limited");
    assert!(!disconnected);
    net.shutdown();
}

/// 한 링크로 물건을 count개 보내고 window 동안 받은 Tossed 수를 센다
fn count_tosses(p: &mut Peer, rx: &Receiver<NetEvent>, count: usize, window: Duration) -> usize {
    for _ in 0..count {
        p.send(json!({"type": "ping", "toss": {"kind": "can", "side": "left"}})).unwrap();
    }
    let deadline = Instant::now() + window;
    let mut events = 0;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::Tossed { .. }) => events += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    events
}

#[test]
fn reconnecting_does_not_refill_the_toss_budget() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut first = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(count_tosses(&mut first, &rx, 60, Duration::from_millis(500)), 50);
    drop(first);
    std::thread::sleep(Duration::from_millis(200));
    // 같은 친구의 새 링크: 남은 토큰을 이어받는다 (1.2초에 하나만 다시 찬다)
    let mut second = raw_friend_connect(port, &kz, "zzzz");
    std::thread::sleep(Duration::from_millis(200));
    second.send(json!({"type": "ping", "ask": "invite"})).unwrap();
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Asked { .. })).is_some(), "the new link works");
    let again = count_tosses(&mut second, &rx, 60, Duration::from_millis(500));
    assert!(again <= 2, "budget carried over the reconnect: {again}");
    net.shutdown();
}

/// 윈도우 동안 조건에 맞는 이벤트 수와, 끊겼는지
fn tally(rx: &Receiver<NetEvent>, window: Duration, pred: impl Fn(&NetEvent) -> bool) -> (usize, bool) {
    let deadline = Instant::now() + window;
    let (mut events, mut disconnected) = (0, false);
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::Disconnected { .. }) => disconnected = true,
            Ok(e) if pred(&e) => events += 1,
            Ok(_) => {}
            Err(_) => break,
        }
    }
    (events, disconnected)
}

fn visit_json(i: u64) -> Value {
    json!({"type": "leave", "visit_id": i})
}

fn ladder_json(i: u64) -> Value {
    json!({"type": "ladder_start", "game_id": format!("g{i}"), "mode": "all"})
}

#[test]
fn flooded_visits_are_rate_limited_and_keep_the_link() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    for i in 0..500 {
        p.send(visit_json(i)).unwrap();
    }
    // 처음 20개까지는 바로, 그 뒤엔 0.5초에 하나 꼴 — 연결은 끊지 않는다
    let (events, disconnected) = tally(&rx, Duration::from_millis(600), |e| matches!(e, NetEvent::Received { .. }));
    assert!((20..=23).contains(&events), "rate limited: {events}");
    assert!(!disconnected);
    p.send(json!({"type": "ping", "ask": "invite"})).unwrap();
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Asked { .. })).is_some(), "other kinds have their own budget");
    net.shutdown();
}

#[test]
fn flooded_ladder_messages_are_rate_limited_and_keep_the_link() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    for i in 0..500 {
        p.send(ladder_json(i)).unwrap();
    }
    let (events, disconnected) = tally(&rx, Duration::from_millis(600), |e| matches!(e, NetEvent::Ladder { .. }));
    assert!((20..=23).contains(&events), "rate limited: {events}");
    assert!(!disconnected);
    p.send(visit_json(1)).unwrap();
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Received { .. })).is_some(), "visits have their own budget");
    net.shutdown();
}

#[test]
fn reconnecting_does_not_refill_any_budget() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let is_counted = |e: &NetEvent| matches!(e, NetEvent::Received { .. } | NetEvent::Ladder { .. } | NetEvent::Steered { .. } | NetEvent::Asked { .. });
    let flood = |p: &mut Peer| {
        for i in 0..40 {
            p.send(visit_json(i)).unwrap();
            p.send(ladder_json(i)).unwrap();
            p.send(json!({"type": "ping", "steer": {"visit_id": 1, "on": true, "keys": 1}})).unwrap();
            p.send(json!({"type": "ping", "ask": "invite"})).unwrap();
        }
    };
    let mut first = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    let drained = Instant::now(); // the budgets are spent after this
    flood(&mut first);
    let (spent, _) = tally(&rx, Duration::from_millis(500), is_counted);
    assert!(spent >= 72, "the whole budget went through: {spent}"); // 방문 20 + 사다리 20 + 조작 30 + 부탁 2
    drop(first);
    std::thread::sleep(Duration::from_millis(200));
    let mut second = raw_friend_connect(port, &kz, "zzzz");
    std::thread::sleep(Duration::from_millis(200));
    flood(&mut second);
    // 시간이 지나 찬 만큼만: 0.5초에 방문·사다리 하나씩, 부탁 2초에 하나. Counted from
    // the measured time, so a slow runner (more time, more refill) can't fail it.
    // Carried over, these would come back in full (42); 조작 (30 a second) is
    // full again within a second anyway, so it can't show a carry-over.
    let slow = |e: &NetEvent| matches!(e, NetEvent::Received { .. } | NetEvent::Ladder { .. } | NetEvent::Asked { .. });
    let (again, _) = tally(&rx, Duration::from_millis(500), slow);
    let e = drained.elapsed().as_secs_f64();
    let refilled = |every: f64, burst: f64| (1.0 + e / every).min(burst).floor() as usize;
    let allowed = 2 * refilled(0.5, 20.0) + refilled(2.0, 2.0);
    assert!(allowed < 42, "too slow to tell ({e:.1}s)");
    assert!(again <= allowed, "budgets carried over the reconnect: {again} > {allowed} after {e:.1}s");
    net.shutdown();
}

#[test]
fn a_friend_with_a_new_key_stays_offline_until_both_remove_each_other_and_pair_again() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, old_kb, new_kb) = (key(), key(), key());
    // B가 키를 잃고 새 키로 시작했다: A는 예전 키를 고정하고 있고, B는 A를 친구로 기억한다
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &old_kb)])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", new_kb, pb, pa, vec![friend("aaaa", &ka)])).unwrap();
    let silent = |e: &NetEvent| is_connected(e) || matches!(e, NetEvent::Invited { .. } | NetEvent::InviteSent { .. });
    assert_eq!(wait_for(&arx, 2.5, silent), None, "A never takes the new key by itself");
    assert_eq!(a.friends(), vec![friend("bbbb", &old_kb)], "the pinned key stays");
    // A가 손으로 지운다. B는 (인증된) A가 자기를 모른다는 걸 알게 되고 저절로 지운다
    a.unfriend("bbbb");
    assert!(a.friends().is_empty());
    assert!(wait_for(&brx, 8.0, |e| *e == NetEvent::FriendForgotUs { id: "aaaa".into() }).is_some());
    assert!(b.friends().is_empty(), "the authenticated friend said it does not know us");
    // 처음부터 다시 짝맞추기
    let deadline = Instant::now() + Duration::from_secs(8);
    let sent = loop {
        a.invite("bbbb", "cat");
        if let Some(e) = wait_for(&arx, 0.3, |e| matches!(e, NetEvent::InviteSent { .. })) {
            break e;
        }
        assert!(Instant::now() < deadline, "no InviteSent");
    };
    let invited = wait_for(&brx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    assert_eq!(sas_of(&sent), sas_of(&invited));
    a.answer_invite(pairing_of(&sent), true);
    b.answer_invite(pairing_of(&invited), true);
    assert!(wait_for(&arx, 5.0, is_connected).is_some());
    assert!(wait_for(&brx, 5.0, is_connected).is_some());
    assert_eq!(a.friends(), vec![friend("bbbb", &new_kb)], "the new key, from a pairing both people confirmed");
    assert_eq!(b.friends(), vec![friend("aaaa", &ka)]);
    a.shutdown();
    b.shutdown();
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
    // 사용자가 다르다고 거절하면 아무도 공격자를 고정하지 않는다
    a.answer_invite(pairing_of(&sent), false);
    b.answer_invite(pairing_of(&invited), false);
    assert_eq!(wait_for(&arx, 0.5, is_connected), None);
    assert_eq!(wait_for(&brx, 0.5, is_connected), None);
    assert!(a.friends().is_empty() && b.friends().is_empty(), "the attacker's keys are not pinned");
    mitm.join().unwrap().unwrap();
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
    b.shutdown();
}

/// The relay version of this is `relay_attacker_cannot_inject_or_replay` in tests/relay.rs.
#[test]
fn lan_proxy_attacker_cannot_inject_or_replay() {
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

// ---- the inviter's side of commit -> random -> reveal ----

/// Net "aaaa" invites a raw "bbbb" (reached by a spoofed announcement); returns
/// it all up to the moment the invite{commit} has arrived.
fn invited_by_a_net() -> (Net, Receiver<NetEvent>, Peer, Vec<u8>, Arc<AtomicBool>) {
    let stop = Arc::new(AtomicBool::new(false));
    let (net, rx) = Net::start(cfg("aaaa", "Ann", key(), 0, free_udp_port(), vec![])).unwrap();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    spoof_announcements("bbbb", l.local_addr().unwrap().port(), net.discovery_port(), Duration::from_millis(100), stop.clone());
    wait_for(&rx, 5.0, |e| matches!(e, NetEvent::PeerSeen { .. })).expect("heard the announcement");
    net.invite("bbbb", "cat");
    let (s, _) = l.accept().unwrap();
    let mut p = handshake(s, &key(), false).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "hello");
    p.send(hello("bbbb", "Bob")).unwrap();
    let invite = p.recv_msg().unwrap();
    assert_eq!(invite["type"], "invite", "{invite}");
    let commit = unhex(invite["commit"].as_str().unwrap());
    (net, rx, p, commit, stop)
}

fn finish_case(net: Net, stop: Arc<AtomicBool>) {
    stop.store(true, Ordering::SeqCst);
    net.shutdown();
}

#[test]
fn inviter_commits_first_and_reveals_only_after_the_random() {
    let (net, rx, mut p, commit, stop) = invited_by_a_net();
    // random을 받기 전에는 nonce가 나오지 않는다
    p.s.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(400) {
        match p.recv() {
            Ok(v) => assert_eq!(v["type"], "ping", "nothing but pings before the random"),
            Err(e) => assert!(matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut), "{e}"),
        }
    }
    assert_eq!(wait_for(&rx, 0.1, |e| matches!(e, NetEvent::InviteSent { .. })), None, "no SAS yet");
    p.s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let r_r = random32();
    p.send(json!({"type": "pair_random", "random": hex(&r_r)})).unwrap();
    let reveal = p.recv_msg().unwrap();
    assert_eq!(reveal["type"], "pair_reveal", "{reveal}");
    let nonce = unhex(reveal["nonce"].as_str().unwrap());
    assert_eq!(blake2s(&[&nonce]).to_vec(), commit, "the reveal opens the commitment");
    let sent = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::InviteSent { .. })).expect("InviteSent");
    assert_eq!(sas_of(&sent), expected_sas(&p.hash, &nonce, &r_r));
    finish_case(net, stop);
}

#[test]
fn inviter_drops_any_step_out_of_order() {
    let out_of_order = [
        json!({"type": "pair_reveal", "nonce": hex(&random32())}),                  // reveal is its own to send
        json!({"type": "invite_reply", "accepted": true}),                          // confirmation before any SAS
        json!({"type": "invite", "pet": "cat", "commit": hex(&random32())}),        // invites only on links the inviter dialed
        json!({"type": "pair_random", "random": "zz"}),                             // not a random
    ];
    for bad in out_of_order {
        let (net, rx, mut p, _, stop) = invited_by_a_net();
        p.send(bad.clone()).unwrap();
        assert!(closed_by_peer(&mut p.s), "{bad}");
        assert_eq!(wait_for(&rx, 0.2, |e| matches!(e, NetEvent::InviteSent { .. } | NetEvent::Connected { .. })), None, "{bad}");
        assert!(net.friends().is_empty(), "{bad}");
        finish_case(net, stop);
    }
}

#[test]
fn inviter_takes_the_random_only_once() {
    let (net, rx, mut p, _, stop) = invited_by_a_net();
    let first = random32();
    p.send(json!({"type": "pair_random", "random": hex(&first)})).unwrap();
    wait_for(&rx, 2.0, |e| matches!(e, NetEvent::InviteSent { .. })).expect("InviteSent");
    assert_eq!(p.recv_msg().unwrap()["type"], "pair_reveal");
    // 공개를 본 뒤 random을 바꾸려 하면 (SAS를 고르려는 시도) 끊는다
    p.send(json!({"type": "pair_random", "random": hex(&random32())})).unwrap();
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.2, |e| matches!(e, NetEvent::InviteSent { .. })), None, "only ever one SAS");
    assert!(net.friends().is_empty());
    finish_case(net, stop);
}

// ---- frames at the edge, slow senders, handshake floods ----

#[test]
fn the_largest_message_passes_and_the_largest_frame_of_garbage_only_ends_its_link() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 암호화해서 정확히 65535바이트가 되는 메시지
    let shell = json!({"type": "ping", "ask": ""}).to_string().len();
    let msg = json!({"type": "ping", "ask": "x".repeat(65535 - 16 - shell)});
    assert_eq!(msg.to_string().len(), 65535 - 16);
    p.send(msg).unwrap();
    let Some(NetEvent::Asked { ask, .. }) = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Asked { .. })) else { panic!("asked") };
    assert_eq!(ask.chars().count(), 64, "clamped");
    // 같은 크기의 쓰레기: 그 링크만 끝난다
    write_frame(&mut p.s, &vec![0xAB; 65535]).unwrap();
    assert!(closed_by_peer(&mut p.s));
    let mut again = raw_friend_connect(port, &kz, "zzzz");
    again.send(json!({"type": "leave", "visit_id": 5})).unwrap();
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Received { .. })).is_some(), "the friend still gets in");
    net.shutdown();
}

#[test]
fn a_message_too_big_for_a_frame_is_dropped_and_the_link_stays() {
    let p = connected_pair();
    let Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, .. } = sample_visit() else { unreachable!() };
    let huge = Visit::Visit { visit_id, species, stage, scale, personality, owner, enter_side, vside: None, via: None, homesick_in: None, fling: None, note: Some("x".repeat(70_000)), gift: None, style: None, boost: None, of: None };
    p.a.send("bbbb", huge);
    p.a.send("bbbb", sample_visit());
    assert_eq!(
        wait_for(&p.brx, 3.0, |e| matches!(e, NetEvent::Received { .. } | NetEvent::Disconnected { .. })),
        Some(NetEvent::Received { from: "aaaa".into(), msg: sample_visit() }),
        "only the one that fits arrives, on the same link"
    );
    p.a.shutdown();
    p.b.shutdown();
}

/// Writes `bytes` one per `every` until the peer closes; how long that took.
fn dribble(s: &mut TcpStream, bytes: &[u8], every: Duration) -> Option<Duration> {
    let t = Instant::now();
    s.set_read_timeout(Some(every)).unwrap();
    for b in bytes {
        if s.write_all(&[*b]).is_err() {
            return Some(t.elapsed());
        }
        match s.read(&mut [0u8; 1024]) {
            Ok(0) => return Some(t.elapsed()),
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => return Some(t.elapsed()),
            _ => {}
        }
    }
    None
}

#[test]
fn a_slow_sender_cannot_hold_a_handshake_past_the_deadline() {
    let (net, _rx, port) = start_sniffed(key(), vec![]);
    let mut s = TcpStream::connect(local(port)).unwrap();
    // 끝나지 않는 큰 프레임을 한 바이트씩: 바이트가 와도 마감은 늘어나지 않는다
    let mut slow = vec![0xFF, 0xFF];
    slow.extend(std::iter::repeat_n(0u8, 400));
    let at = dribble(&mut s, &slow, Duration::from_millis(25)).expect("closed");
    assert!(at <= fast().conn_timeout + Duration::from_secs(1), "held for {at:?}");
    net.shutdown();
}

#[test]
fn a_friend_dribbling_a_frame_counts_as_silent() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    let mut slow = vec![0xFF, 0xFF];
    slow.extend(std::iter::repeat_n(0u8, 400));
    let at = dribble(&mut p.s, &slow, Duration::from_millis(25)).expect("closed");
    assert!(at <= fast().conn_timeout + Duration::from_secs(1), "held for {at:?}");
    net.shutdown();
}

#[test]
fn a_handshake_flood_does_not_keep_a_friend_out() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let stop = Arc::new(AtomicBool::new(false));
    // 여러 주소에서 Noise 첫 메시지까지 보내고 그대로 두는 연결을 계속 연다
    let flood = {
        let stop = stop.clone();
        std::thread::spawn(move || {
            let mut opened = 0u32;
            let mut held = std::collections::VecDeque::new();
            let k = key();
            // 같은 첫 메시지를 되풀이한다: 공격자는 거의 공짜, 받는 쪽은 매번 키를 계산한다
            let mut hs = snow::Builder::new("Noise_XX_25519_ChaChaPoly_BLAKE2s".parse().unwrap()).local_private_key(&k).unwrap().prologue(b"screenpet-net/1").unwrap().build_initiator().unwrap();
            let mut buf = [0u8; 128];
            let n = hs.write_message(&[], &mut buf).unwrap();
            while !stop.load(Ordering::SeqCst) {
                let Some(mut s) = connect_from([127, 0, 0, 20 + (opened % 10) as u8], port) else { continue };
                let _ = write_frame(&mut s, &buf[..n]);
                held.push_back(s);
                opened += 1;
                if held.len() > 200 {
                    held.pop_front();
                }
            }
            opened
        })
    };
    std::thread::sleep(Duration::from_millis(300));
    // The friend's address never authenticated here, so its link sits in the
    // unverified pool with the flood's and one attempt can be churned out
    // before its handshake ends (a slow runner makes the handshake outlast the
    // pool's turnover). A real friend's Net dials again on its next
    // announcement; so does this one, and the flood must not stop all of them.
    let deadline = Instant::now() + Duration::from_secs(5);
    let (mut friend, mut attempts) = (None, 0);
    while friend.is_none() && Instant::now() < deadline {
        attempts += 1;
        let attempt = handshake(TcpStream::connect(local(port)).unwrap(), &kz, true).and_then(|mut p| {
            p.send(friend_hello("zzzz", "Raw"))?;
            p.recv_msg().map(|_| p)
        });
        friend = attempt.ok().filter(|_| wait_for(&rx, 1.0, is_connected).is_some());
        if friend.is_none() {
            std::thread::sleep(fast().announce_interval);
        }
    }
    assert!(friend.is_some(), "the friend got in during the flood ({attempts} attempts)");
    stop.store(true, Ordering::SeqCst);
    let opened = flood.join().unwrap();
    assert!(opened > 50, "there was no real flood: {opened} connections");
    net.shutdown();
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
    // From another address than the friend's (a flood from the friend's own
    // address costs it that second: see the next test).
    let mut flood: Vec<TcpStream> = (0..60).map(|_| connect_from([127, 0, 0, 2], port).expect("flood connection")).collect();
    // Closed = over the address's budget, or evicted from the bounded
    // unverified pool by newer connections; a read timeout = still held.
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
fn a_flood_from_one_address_is_cut_off_and_recovers_within_a_second() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let flood: Vec<TcpStream> = (0..25).map(|_| connect_from([127, 0, 0, 3], port).expect("flood connection")).collect();
    // 예산이 바닥난 주소의 새 연결은 Noise를 시작하지도 못한다
    let probe = connect_from([127, 0, 0, 3], port).expect("probe connection");
    assert!(handshake(probe, &key(), true).is_err(), "no key exchange past the address's budget");
    // 다른 주소는 그대로 받는다
    let other = connect_from([127, 0, 0, 4], port).expect("other address");
    assert!(handshake(other, &key(), true).is_ok(), "another address is served");
    drop(flood);
    // 그 주소도 잠시 뒤에는 다시 들어온다 (친구가 같은 공유기 뒤에 있어도 영영 막히지 않는다)
    std::thread::sleep(Duration::from_millis(1100));
    let _p = raw_friend_connect(port, &kz, "zzzz");
    assert!(wait_for(&rx, 5.0, is_connected).is_some());
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
            let mut opened = 0u32;
            while !stop.load(Ordering::SeqCst) {
                let targets: Vec<u16> =
                    ports.lock().unwrap().values().chain(ports_b.lock().unwrap().values()).copied().collect();
                for p in targets {
                    // Several attacker source addresses (not the friends' 127.0.0.1).
                    n += 1;
                    if let Some(s) = connect_from([127, 0, 0, 2 + (n % 8) as u8], p) {
                        held.push_back(s);
                        opened += 1;
                    }
                }
                // Hold each connection ~2 s (longer than the pre-auth deadline).
                while held.len() > 600 {
                    held.pop_front();
                }
                std::thread::sleep(Duration::from_millis(7));
            }
            opened
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
    let opened = attacker.join().unwrap();
    assert!(opened > 100, "there was no real flood: {opened} connections");
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
            .map(|i| UdpSocket::bind(SocketAddr::from(([127, 0, 1 + (i / 250) as u8, 1 + (i % 250) as u8], 0))).expect("a loopback source"))
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
fn pairings_finishing_together_cannot_overfill_the_friend_list() {
    // 자리가 하나 남았을 때 두 짝맞추기가 나란히 끝까지 간다
    let (net, rx, port) = start_sniffed(key(), many_friends(MAX_FRIENDS - 1));
    let (mut p1, ev1) = raw_pairing(port, &rx, "zzz1");
    let (mut p2, ev2) = raw_pairing(port, &rx, "zzz2");
    p1.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    p2.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    net.answer_invite(pairing_of(&ev1), true);
    net.answer_invite(pairing_of(&ev2), true);
    assert_eq!(wait_for(&rx, 2.0, is_connected), Some(NetEvent::Connected { id: "zzz1".into(), name: "Raw".into() }));
    assert_eq!(net.friends().len(), MAX_FRIENDS, "never more than the cap");
    assert!(!net.friends().iter().any(|f| f.id == "zzz2"));
    assert_eq!(
        wait_for(&rx, 2.0, |e| matches!(e, NetEvent::InviteAnswered { from, accepted: false, .. } if from == "zzz2")),
        Some(NetEvent::InviteAnswered { from: "zzz2".into(), accepted: false, pairing: pairing_of(&ev2) }),
        "the one that didn't fit is told so"
    );
    assert_eq!(p2.recv_msg().unwrap(), json!({"type": "invite_reply", "accepted": false}));
    net.shutdown();
}

#[test]
fn a_flood_cannot_evict_a_pairing_waiting_for_the_user() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let (mut p, ev) = raw_pairing(port, &rx, "zzzz");
    // 사람이 이모지를 비교하는 동안 낯선 연결이 풀을 몇 바퀴 돈다 (다른 주소들과 같은 주소에서)
    let mut flood = Vec::new();
    for i in 0..40u8 {
        flood.push(connect_from([127, 0, 0, 10 + i % 10], port).expect("flood connection"));
        flood.push(connect_from([127, 0, 0, 1], port).expect("flood connection"));
        std::thread::sleep(Duration::from_millis(5));
    }
    p.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    net.answer_invite(pairing_of(&ev), true);
    assert_eq!(wait_for(&rx, 2.0, is_connected), Some(NetEvent::Connected { id: "zzzz".into(), name: "Raw".into() }), "the pairing survived");
    net.shutdown();
}

#[test]
fn an_inviter_that_never_reveals_does_not_hold_a_slot_for_the_whole_pairing_time() {
    let (net, _rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&random32()]))})).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "pair_random");
    p.s.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let t = Instant::now();
    let mut refused = None;
    while refused.is_none() && t.elapsed() < fast().invite_timeout {
        let _ = p.send(json!({"type": "ping"})); // 연결은 살아 있다
        if let Ok(v) = p.recv() {
            if v["type"] == "invite_reply" {
                refused = Some(t.elapsed());
            }
        }
    }
    let at = refused.expect("refused before the pairing deadline");
    assert!(at <= fast().conn_timeout + Duration::from_millis(500), "{at:?}");
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
fn a_proven_friend_that_forgot_us_is_removed_and_reported_once() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    // A pinned B, but B never pinned A (e.g. pairing finished on one side).
    let (a, arx) = Net::start(cfg("aaaa", "Ann", ka, pa, pb, vec![friend("bbbb", &kb)])).unwrap();
    let (b, brx) = Net::start(cfg("bbbb", "Bob", kb, pb, pa, vec![])).unwrap();
    let (mismatches, connected) =
        count_events(&arx, 4.0, |e| *e == NetEvent::FriendForgotUs { id: "bbbb".into() });
    assert!(!connected);
    assert_eq!(mismatches, 1, "it stops being a friend, so nothing redials");
    assert!(a.friends().is_empty(), "the right key said it does not know us");
    assert_eq!(wait_for(&brx, 0.1, is_connected), None);
    a.shutdown();
    b.shutdown();
}

#[test]
fn only_the_pinned_key_can_say_it_forgot_us() {
    let (ka, kb, stranger) = (key(), key(), key());
    let (a, arx, port) = start_sniffed(ka, vec![friend("bbbb", &kb)]);
    // 틀린 키로 같은 말을 해도 (인증되지 않았다) 아무 일도 없다
    let _wrong = raw_connect_claiming(port, &stranger, "bbbb");
    assert_eq!(wait_for(&arx, 1.0, |e| matches!(e, NetEvent::FriendForgotUs { .. })), None);
    assert_eq!(a.friends(), vec![friend("bbbb", &kb)]);
    // 고정한 키로 말하면 지운다
    let _right = raw_connect_claiming(port, &kb, "bbbb");
    assert!(wait_for(&arx, 3.0, |e| *e == NetEvent::FriendForgotUs { id: "bbbb".into() }).is_some());
    assert!(a.friends().is_empty());
    a.shutdown();
}

#[test]
fn a_friend_that_restarted_without_us_ends_the_link_it_left_behind() {
    let (ka, kb) = (key(), key());
    let (a, arx, port) = start_sniffed(ka, vec![friend("bbbb", &kb)]);
    let _old = raw_friend_connect(port, &kb, "bbbb");
    assert!(wait_for(&arx, 3.0, is_connected).is_some());
    let _new = raw_connect_claiming(port, &kb, "bbbb");
    assert!(wait_for(&arx, 3.0, |e| *e == NetEvent::FriendForgotUs { id: "bbbb".into() }).is_some());
    assert!(wait_for(&arx, 3.0, |e| *e == NetEvent::Disconnected { id: "bbbb".into() }).is_some());
    assert!(a.friends().is_empty());
    a.shutdown();
}

#[test]
fn a_hello_sent_just_before_the_friend_pinned_us_does_not_remove_it() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &kz, "zzzz");
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    p.recv_msg().unwrap(); // pair_random
    p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
    let ev = wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    p.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    net.answer_invite(pairing_of(&ev), true);
    assert!(wait_for(&rx, 2.0, is_connected).is_some());
    // 상대가 나를 고정하기 전에 보낸 인사가 이제야 온다
    let _late = raw_connect_claiming(port, &kz, "zzzz");
    assert_eq!(wait_for(&rx, 1.0, |e| matches!(e, NetEvent::FriendForgotUs { .. })), None);
    assert_eq!(net.friends().len(), 1, "too soon after pairing to mean anything");
    net.shutdown();
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
    assert_eq!(a.friends(), vec![friend("bbbb", &kb)], "an unproven answer never removes a friend");
    let n = accepts.load(Ordering::SeqCst);
    assert!((1..=3).contains(&n), "{n} dials to the bad candidate in 4 s");
    stop.store(true, Ordering::SeqCst);
    a.shutdown();
}

// ---- ladder messages and peer versions -------------------------------------------

fn sample_ladder() -> LadderMsg {
    LadderMsg::LadderReveal { game_id: "00112233445566778899aabbccddeeff".into(), nonce: "ab".into() }
}

#[test]
fn ladder_messages_flow_between_current_builds() {
    let p = connected_pair();
    assert_eq!(p.a.peer_version("bbbb"), Some(screenpet_net::PROTOCOL_VERSION));
    assert!(p.a.send_ladder("bbbb", sample_ladder()));
    assert_eq!(
        wait_for(&p.brx, 2.0, |e| matches!(e, NetEvent::Ladder { .. })),
        Some(NetEvent::Ladder { from: "aaaa".into(), msg: sample_ladder() })
    );
    assert!(!p.a.send_ladder("nobody", sample_ladder()), "not connected");
    assert_eq!(p.a.peer_version("nobody"), None);
    p.a.shutdown();
    p.b.shutdown();
}

#[test]
fn ladder_messages_are_never_sent_to_an_older_build() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    // 예전 판 흉내: hello v 1, 모르는 type을 받으면 연결을 끊는 쪽
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(net.peer_version("zzzz"), Some(1));
    assert!(!net.send_ladder("zzzz", sample_ladder()));
    for _ in 0..4 {
        let m = p.recv().unwrap();
        assert_eq!(m["type"], "ping", "only pings reach the old build: {m}");
    }
    assert_eq!(wait_for(&rx, 0.5, is_disconnected), None);
    net.shutdown();
}

#[test]
fn ladder_message_from_a_stranger_is_ignored() {
    let (net, rx, port) = start_sniffed(key(), vec![]);
    let mut p = raw_connect(port, &key(), "zzzz");
    p.send(serde_json::to_value(sample_ladder()).unwrap()).unwrap();
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::Ladder { .. })), None);
    net.shutdown();
}

#[test]
fn a_rehello_updates_the_peer_version() {
    let kz = key();
    let (net, rx, port) = start_sniffed(key(), vec![friend("zzzz", &kz)]);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(net.peer_version("zzzz"), Some(1));
    p.send(json!({"type": "hello", "id": "zzzz", "name": "Raw", "v": 2, "friend": true})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while net.peer_version("zzzz") != Some(2) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(net.peer_version("zzzz"), Some(2));
    assert!(net.send_ladder("zzzz", sample_ladder()));
    net.shutdown();
}

// ---- linked devices (📱 내 다른 컴퓨터) -------------------------------------------

/// An install id in the real format (32 lowercase hex digits), distinct per `n`.
fn hid(n: u8) -> String {
    format!("{n:02x}").repeat(16)
}

/// Start `c` on an OS-chosen discovery port, "broadcasting" to a sniffer;
/// returns its TCP port from the announcement.
fn sniffed(mut c: NetConfig) -> (Net, Receiver<NetEvent>, u16) {
    let sniffer = UdpSocket::bind("127.0.0.1:0").unwrap();
    sniffer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    c.discovery_port = 0;
    c.broadcast_to = local(sniffer.local_addr().unwrap().port());
    let id = c.id.clone();
    let (net, rx) = Net::start(c).unwrap();
    (net, rx, sniff_port(&sniffer, &id))
}

/// Hello from a current (protocol 3) peer that has the receiver pinned.
fn v3_hello(id: &str) -> Value {
    json!({"type": "hello", "id": id, "name": "Raw", "v": 3, "friend": true})
}

/// Every non-ping message `p` receives within `window`.
fn messages_within(p: &mut Peer, window: Duration) -> Vec<Value> {
    p.s.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let deadline = Instant::now() + window;
    let mut out = Vec::new();
    while Instant::now() < deadline {
        match p.recv() {
            Ok(v) if v["type"] != "ping" => out.push(v),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {}
            Err(_) => break,
        }
    }
    p.s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    out
}

/// Messages each of `peers` gets within `window`, read one peer after the
/// other. All of them ping before each read: a raw peer sends nothing on its
/// own, and one left silent past `conn_timeout` while the others are read is
/// dropped as dead, which would make the checks on it pass vacuously.
fn messages_of_each(peers: &mut [(&mut Peer, &str)], window: Duration) -> Vec<(String, Vec<Value>)> {
    let mut out = Vec::new();
    for i in 0..peers.len() {
        for (q, who) in peers.iter_mut() {
            q.send(json!({"type": "ping"})).unwrap_or_else(|e| panic!("{who} was dropped: {e}"));
        }
        let (p, who) = &mut peers[i];
        out.push((who.to_string(), messages_within(p, window)));
    }
    out
}

fn wire(id: &str, key: &[u8; 32]) -> Value {
    json!({"id": id, "key": hex(key)})
}

/// Messages other than `device_active` (my state, told on each proven device link).
fn but_active(msgs: Vec<Value>) -> Vec<Value> {
    msgs.into_iter().filter(|m| m["type"] != "device_active").collect()
}

fn added(e: &NetEvent) -> bool {
    matches!(e, NetEvent::FriendAdded { .. })
}

/// Every `FriendAdded` id within `secs`.
fn added_ids(rx: &Receiver<NetEvent>, secs: f64) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    let mut ids = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(NetEvent::FriendAdded { id, .. }) => ids.push(id),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    ids
}

fn names(n: &str) -> (String, Option<String>) {
    (format!("name of {n}"), Some("left".into()))
}

// The documented device-link scheme, written out independently here.
const L_CODE: &[u8] = b"screenpet-device/1 code";
const L_CODE_PROOF: &[u8] = b"screenpet-device/1 code proof";
const L_SECRET: &[u8] = b"screenpet-device/1 secret";
const L_PROOF: &[u8] = b"screenpet-device/1 proof";

/// K = BLAKE2s(label || the 8 code symbols, upper case, no dash).
fn code_key(code: &str) -> [u8; 32] {
    let c: String = code.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_uppercase()).collect();
    blake2s(&[L_CODE, c.as_bytes()])
}

/// The proof of whoever typed the code (sent first; the holder answers only if it is right).
fn enter_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], enterer: &[u8; 32], holder: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"enter", k, nb, na, enterer, holder])
}

/// The code holder's proof, after it checked the enterer's.
fn hold_proof(k: &[u8; 32], nb: &[u8; 32], na: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_CODE_PROOF, b"hold", k, nb, na, holder, enterer])
}

/// The long-term secret both store.
fn derive_secret(k: &[u8; 32], holder: &[u8; 32], enterer: &[u8; 32], na: &[u8; 32], nb: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_SECRET, k, holder, enterer, na, nb])
}

/// Proof of the stored secret on a later link (to the verifier's fresh nonce).
fn secret_proof(s: &[u8; 32], prover: &[u8; 32], verifier: &[u8; 32], n_verifier: &[u8; 32], n_prover: &[u8; 32]) -> [u8; 32] {
    blake2s(&[L_PROOF, s, prover, verifier, n_verifier, n_prover])
}

fn b32(v: &Value, field: &str) -> [u8; 32] {
    unhex(v[field].as_str().unwrap_or_else(|| panic!("{field} in {v}"))).try_into().unwrap()
}

fn linked(c: &mut NetConfig, id: &str, s: [u8; 32]) {
    c.device = Some((id.to_string(), DeviceSecret(s)));
}

fn proven(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceProven { .. })
}

/// As the raw linked device `p` (private key `mine`): take the net's
/// `device_nonce`, send ours, check the net's proof, send ours.
fn prove_as_device(p: &mut Peer, s: &[u8; 32], mine: &[u8; 32]) {
    let theirs = p.recv_msg().unwrap();
    assert_eq!(theirs["type"], "device_nonce", "{theirs}");
    let nn = b32(&theirs, "nonce");
    let nm = random32();
    p.send(json!({"type": "device_nonce", "nonce": hex(&nm)})).unwrap();
    let (me, net) = (Net::public_key(mine), p.remote);
    let proof = p.recv_msg().unwrap();
    assert_eq!(proof["type"], "device_proof", "{proof}");
    assert_eq!(b32(&proof, "proof"), secret_proof(s, &net, &me, &nm, &nn), "the net proves the same secret");
    p.send(json!({"type": "device_proof", "proof": hex(&secret_proof(s, &me, &net, &nn, &nm))})).unwrap();
}

/// As the raw peer `p` (private key `mine`), type `code` that the net is
/// holding for us. `Ok(secret)` when linked; `Err(the net's answer)` if not.
fn raw_enter(p: &mut Peer, code: &str, mine: &[u8; 32]) -> Result<[u8; 32], Value> {
    let nb = random32();
    p.send(json!({"type": "device_code", "nonce": hex(&nb)})).unwrap();
    let r = p.recv_msg().unwrap();
    if r["type"] != "device_code_nonce" {
        return Err(r);
    }
    let na = b32(&r, "nonce");
    let (k, me, net) = (code_key(code), Net::public_key(mine), p.remote);
    p.send(json!({"type": "device_code_proof", "proof": hex(&enter_proof(&k, &nb, &na, &me, &net))})).unwrap();
    let r = p.recv_msg().unwrap();
    if r["type"] != "device_code_ok" {
        return Err(r);
    }
    assert_eq!(b32(&r, "proof"), hold_proof(&k, &nb, &na, &net, &me), "the holder proves the code too");
    Ok(derive_secret(&k, &net, &me, &na, &nb))
}

fn confirm_ask(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceConfirm { .. })
}

fn cancelled(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceLinkCancelled { .. })
}

/// The code matched with raw peer `p` (friend `id` of `net`): both sides press 확인.
fn confirm_both(net: &Net, rx: &Receiver<NetEvent>, p: &mut Peer, id: &str) {
    assert_eq!(wait_for(rx, 5.0, confirm_ask), Some(NetEvent::DeviceConfirm { id: id.into() }));
    p.send(json!({"type": "device_confirm"})).unwrap();
    assert!(net.confirm_device(id, true));
}

#[test]
fn device_messages_are_never_sent_to_an_older_build() {
    let kz = key();
    let me = hid(1);
    let mut c = cfg(&me, "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(2), &key())]);
    linked(&mut c, &hid(2), random32());
    let (net, rx, port) = sniffed(c);
    // 예전 판 흉내 (hello v 1, 모르는 type이면 연결을 끊는다): 새 메시지는 하나도 가지 않는다
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert!(!net.send_device_friends("zzzz", true, names));
    assert!(!net.send_my_devices("zzzz"), "an old build is never told about my devices");
    let got = messages_within(&mut p, Duration::from_millis(700));
    assert!(got.is_empty(), "only pings reach the old build: {got:?}");
    assert_eq!(wait_for(&rx, 0.3, is_disconnected), None);
    // 같은 친구가 새 판이 되면 (hello v 3) 보낸다
    p.send(v3_hello("zzzz")).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while net.peer_version("zzzz") != Some(3) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(net.send_my_devices("zzzz"));
    let got = messages_within(&mut p, Duration::from_millis(500));
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["type"], "my_devices");
    assert_eq!(got[0]["devices"][0]["id"], hid(2));
    net.shutdown();
}

#[test]
fn a_linked_device_on_an_older_build_is_never_asked_for_proof_or_told_to_unlink() {
    let kz = key();
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]);
    linked(&mut c, "zzzz", random32());
    let (net, rx, port) = sniffed(c);
    let mut p = raw_friend_connect(port, &kz, "zzzz");
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(net.enter_link_code("zzzz", "ABCD-EFGH"), Err(LinkError::Old));
    assert!(net.new_link_code("zzzz").is_ok(), "a code can be shown, but the old build cannot use it");
    assert!(net.unlink_device("zzzz"));
    assert_eq!(messages_within(&mut p, Duration::from_millis(600)), Vec::<Value>::new());
    assert_eq!(wait_for(&rx, 0.3, is_disconnected), None);
    net.shutdown();
}

#[test]
fn device_friends_go_only_to_a_proven_device_and_leave_out_introduced_and_unconfirmed_friends() {
    let (kz, ky) = (key(), key());
    let me = hid(1);
    let (plain, introduced, unconfirmed) = (hid(2), hid(3), hid(4));
    let kp = key();
    let s = random32();
    let mut c = cfg(&me, "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("yyyy", &ky), friend(&plain, &kp), friend(&introduced, &key()), friend(&unconfirmed, &key())]);
    c.introduced = vec![(introduced.clone(), "yyyy".into())];
    c.pending = vec![unconfirmed.clone()];
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 비밀을 증명하기 전에는 그냥 친구다
    assert!(!net.send_device_friends("zzzz", false, names), "not proven yet");
    prove_as_device(&mut p, &s, &kz);
    assert_eq!(wait_for(&rx, 2.0, proven), Some(NetEvent::DeviceProven { id: "zzzz".into() }));
    assert!(net.send_device_friends("zzzz", true, names));
    let got = but_active(messages_within(&mut p, Duration::from_millis(500)));
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0]["type"], "device_friends");
    assert_eq!(got[0]["want"], true);
    let ids: Vec<&str> = got[0]["friends"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec!["yyyy", plain.as_str()], "not the receiver itself, not introduced, not unconfirmed");
    let f = &got[0]["friends"][1];
    assert_eq!((f["key"].as_str().unwrap(), f["name"].as_str().unwrap(), f["side"].as_str().unwrap()), (hex(&Net::public_key(&kp)).as_str(), format!("name of {plain}").as_str(), "left"));
    net.shutdown();
}

#[test]
fn device_friends_are_accepted_only_from_a_proven_device_and_only_add() {
    let (kz, kk) = (key(), key());
    let (me, mine) = (hid(1), key());
    let known = hid(2);
    let s = random32();
    let mut c = cfg(&me, "Me", mine, 0, 0, vec![friend("zzzz", &kz), friend(&known, &kk)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let new = hid(3);
    let kn = key();
    // 비밀을 증명하지 않은 링크의 목록은 버린다 (연결은 그대로)
    p.send(json!({"type": "device_friends", "friends": [wire(&new, &kn)]})).unwrap();
    assert_eq!(wait_for(&rx, 0.6, added), None);
    assert_eq!(net.friends().len(), 2);
    prove_as_device(&mut p, &s, &kz);
    assert!(wait_for(&rx, 2.0, proven).is_some());
    let other_key = key();
    p.send(json!({"type": "device_friends", "want": true, "friends": [
        {"id": new, "key": hex(&kn), "name": "N", "side": "up"},
        wire(&known, &other_key),          // 있는 친구의 키를 바꾸려 한다
        wire(&me, &key()),                  // 내 id
        wire(&hid(4), &Net::public_key(&mine)), // 내 키
        wire(&hid(5), &Net::public_key(&kk)), // 있는 친구의 키를 다른 id로
        wire("zzzz", &key()),               // 보낸 기기 자신
        {"id": hid(6), "key": "zz"},
        {"id": "not an id", "key": hex(&key())},
        {"id": hid(7)},
        5,
    ]})).unwrap();
    let ev = wait_for(&rx, 2.0, added).expect("the new friend is added");
    assert_eq!(ev, NetEvent::FriendAdded { id: new.clone(), name: "N".into(), via: None, side: Some("up".into()) });
    assert_eq!(wait_for(&rx, 1.0, |e| matches!(e, NetEvent::DeviceSynced { .. })), Some(NetEvent::DeviceSynced { from: "zzzz".into(), want: true }));
    assert_eq!(added_ids(&rx, 0.5), Vec::<String>::new(), "nothing else");
    let mut friends = net.friends();
    friends.sort_by(|a, b| a.id.cmp(&b.id));
    let mut expected = vec![friend("zzzz", &kz), friend(&known, &kk), Friend { id: new.clone(), public_key: kn }];
    expected.sort_by(|a, b| a.id.cmp(&b.id));
    assert_eq!(friends, expected, "keys are never overwritten");
    let info = net.friend_info(&new).unwrap();
    assert!(info.pending && info.via.is_none() && !info.device);
    // 목록에 없는 친구도 지우지 않는다 (지우기는 각 컴에서)
    p.send(json!({"type": "device_friends", "friends": []})).unwrap();
    assert_eq!(wait_for(&rx, 1.0, |e| matches!(e, NetEvent::DeviceSynced { .. })), Some(NetEvent::DeviceSynced { from: "zzzz".into(), want: false }));
    assert_eq!(net.friends().len(), 3);
    assert_eq!(wait_for(&rx, 0.3, is_disconnected), None, "the link stays");
    net.shutdown();
}

/// A net linked to raw device "zzzz" that proved its secret on this link.
fn proven_device() -> (Net, Receiver<NetEvent>, Peer) {
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    prove_as_device(&mut p, &s, &kz);
    assert!(wait_for(&rx, 2.0, proven).is_some());
    (net, rx, p)
}

#[test]
fn device_friends_stop_at_the_friend_limit_and_names_are_clamped() {
    let (net, rx, mut p) = proven_device();
    let list: Vec<Value> = (0..60u8).map(|i| json!({"id": hid(100 + i), "key": hex(&key()), "name": "가".repeat(100)})).collect();
    p.send(json!({"type": "device_friends", "friends": list})).unwrap();
    let first = wait_for(&rx, 2.0, added).unwrap();
    let NetEvent::FriendAdded { name, .. } = first else { unreachable!() };
    assert_eq!(name.chars().count(), 64);
    let more = added_ids(&rx, 1.0);
    assert_eq!(more.len() + 1, MAX_FRIENDS - 1);
    assert_eq!(net.friends().len(), MAX_FRIENDS);
    net.shutdown();
}

#[test]
fn device_friend_floods_are_rate_limited_and_keep_the_link() {
    let (net, rx, mut p) = proven_device();
    for i in 0..40u8 {
        p.send(json!({"type": "device_friends", "friends": [wire(&hid(100 + i), &key())]})).unwrap();
    }
    let (synced, disconnected) = tally(&rx, Duration::from_millis(600), |e| matches!(e, NetEvent::DeviceSynced { .. }));
    assert!((1..=7).contains(&synced), "rate limited: {synced}");
    assert!(!disconnected);
    net.shutdown();
}

#[test]
fn an_introduction_adds_at_most_one_device_and_it_leaves_with_the_introducer() {
    // 한 사람의 컴퓨터는 둘까지: 친구가 소개하는 다른 컴퓨터는 하나뿐
    let (kz, kk) = (key(), key());
    let known = hid(2);
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&known, &kk)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let devs: Vec<String> = (10..13u8).map(hid).collect();
    assert_eq!(MAX_DEVICES, 1);
    // 첫 항목만 본다: 내 친구의 키를 바꾸려는 것이면 아무도 더하지 않는다
    p.send(json!({"type": "my_devices", "devices": [wire(&known, &key()), wire(&devs[0], &key())]})).unwrap();
    assert_eq!(added_ids(&rx, 0.8), Vec::<String>::new(), "only the first entry is looked at");
    p.send(json!({"type": "my_devices", "devices": [wire(&devs[0], &key()), wire(&devs[1], &key())]})).unwrap();
    let ev = wait_for(&rx, 2.0, added).unwrap();
    assert!(matches!(&ev, NetEvent::FriendAdded { id, via: Some(v), name, .. } if *id == devs[0] && v == "zzzz" && name.is_empty()), "{ev:?}");
    assert_eq!(added_ids(&rx, 0.8), Vec::<String>::new());
    // 다시 소개해도 그 친구가 소개한 기기는 모두 합쳐 하나까지
    p.send(json!({"type": "my_devices", "devices": [wire(&devs[2], &key())]})).unwrap();
    assert_eq!(added_ids(&rx, 0.8), Vec::<String>::new());
    assert_eq!(net.friend_info(&devs[0]).unwrap().via.as_deref(), Some("zzzz"));
    assert_eq!(net.friend_info(&known).unwrap().via, None);
    assert_eq!(net.friends().iter().find(|f| f.id == known).unwrap().public_key, Net::public_key(&kk));
    assert_eq!(net.friends().len(), 3);
    // 소개한 친구를 지우면 그 기기도 함께
    net.unfriend("zzzz");
    assert_eq!(wait_for(&rx, 1.0, |e| matches!(e, NetEvent::FriendDropped { .. })), Some(NetEvent::FriendDropped { id: devs[0].clone() }));
    assert_eq!(net.friends(), vec![friend(&known, &kk)]);
    net.shutdown();
}

#[test]
fn an_introduced_device_cannot_introduce_more() {
    // 소개는 한 다리만: 친구가 소개한 기기가 또 기기를 소개하면 믿음이 사슬처럼 번진다
    let (kz, kd) = (key(), key());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(10), &kd)]);
    c.introduced = vec![(hid(10), "zzzz".to_string())];
    let (net, rx, port) = sniffed(c);
    let mut d = raw_connect_as(port, &kd, v3_hello(&hid(10)));
    wait_for(&rx, 5.0, is_connected).unwrap();
    d.send(json!({"type": "my_devices", "devices": [wire(&hid(11), &key())]})).unwrap();
    assert!(added_ids(&rx, 0.8).is_empty(), "a device introduced by a friend introduces no one");
    assert_eq!(net.friends().len(), 2);
    net.shutdown();
}

#[test]
fn introduced_devices_leave_when_the_introducer_removes_me() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "my_devices", "devices": [wire(&hid(10), &key())]})).unwrap();
    assert!(wait_for(&rx, 2.0, added).is_some());
    p.send(json!({"type": "unfriend"})).unwrap();
    assert_eq!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::FriendDropped { .. })), Some(NetEvent::FriendDropped { id: hid(10) }));
    assert!(net.friends().is_empty());
    net.shutdown();
}

#[test]
fn introductions_are_ignored_from_strangers_old_messages_and_my_own_device() {
    let kz = key();
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]);
    linked(&mut c, "zzzz", random32());
    let (net, rx, port) = sniffed(c);
    // 낯선 사람
    let mut s = raw_connect(port, &key(), &hid(9));
    let _ = s.send(json!({"type": "my_devices", "devices": [wire(&hid(10), &key())]}));
    let _ = s.send(json!({"type": "device_friends", "friends": [wire(&hid(11), &key())]}));
    assert_eq!(wait_for(&rx, 0.6, added), None);
    drop(s);
    // 내 다른 컴퓨터의 소개는 받지 않는다 (그 컴의 친구는 device_friends로 온다)
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "my_devices", "devices": [wire(&hid(12), &key())]})).unwrap();
    assert_eq!(wait_for(&rx, 0.6, added), None);
    assert_eq!(net.friends().len(), 1);
    net.shutdown();
}

#[test]
fn linking_an_introduced_device_makes_it_mine_and_not_someone_elses() {
    let (kz, kd) = (key(), key());
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "my_devices", "devices": [wire(&hid(10), &Net::public_key(&kd))]})).unwrap();
    assert!(wait_for(&rx, 2.0, added).is_some());
    drop(p); // 같은 주소의 raw 링크는 하나씩
    wait_for(&rx, 5.0, is_disconnected).unwrap();
    let code = net.new_link_code(&hid(10)).unwrap();
    let mut d = raw_connect_as(port, &kd, v3_hello(&hid(10)));
    wait_for(&rx, 5.0, is_connected).unwrap();
    raw_enter(&mut d, &code, &kd).expect("linked");
    confirm_both(&net, &rx, &mut d, &hid(10));
    assert_eq!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::DeviceLinked { .. })), Some(NetEvent::DeviceLinked { id: hid(10) }));
    let info = net.friend_info(&hid(10)).unwrap();
    assert!(info.device && info.via.is_none());
    assert_eq!(net.new_link_code("nobody"), Err(LinkError::NotFriend), "only friends");
    net.unfriend("zzzz");
    assert_eq!(wait_for(&rx, 0.8, |e| matches!(e, NetEvent::FriendDropped { .. })), None);
    assert_eq!(net.friends(), vec![Friend { id: hid(10), public_key: Net::public_key(&kd) }]);
    net.shutdown();
}

#[test]
fn an_unconfirmed_friend_that_does_not_know_us_yet_is_kept_until_it_does() {
    let kq = key();
    let q = hid(5);
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend(&q, &kq)]);
    c.pending = vec![q.clone()];
    let (net, rx, port) = sniffed(c);
    // 아직 나를 모른다 (소개가 늦게 도착하는 중): 지우지 않는다
    let mut p = handshake(TcpStream::connect(local(port)).unwrap(), &kq, true).unwrap();
    p.send(json!({"type": "hello", "id": q, "name": "Q", "v": 3})).unwrap();
    assert!(closed_by_peer(&mut p.s));
    assert_eq!(wait_for(&rx, 0.5, |e| matches!(e, NetEvent::FriendForgotUs { .. })), None);
    assert_eq!(net.friends().len(), 1);
    assert!(net.friend_info(&q).unwrap().pending);
    // 이제 나를 안다: 연결되고 확인된 친구가 된다
    let _p = raw_connect_as(port, &kq, json!({"type": "hello", "id": q, "name": "Q", "v": 3, "friend": true}));
    assert!(wait_for(&rx, 5.0, is_connected).is_some());
    assert!(!net.friend_info(&q).unwrap().pending);
    net.shutdown();
}

fn event_of(rx: &Receiver<NetEvent>, pred: impl Fn(&NetEvent) -> bool) -> NetEvent {
    wait_for(rx, 5.0, pred).expect("event")
}

#[test]
fn two_computers_link_with_a_code_share_friends_and_prove_it_again_after_a_restart() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb, kf) = (key(), key(), key());
    let (a_id, b_id, f_id) = (hid(0xa1), hid(0xb2), hid(0xf3));
    let ca = cfg(&a_id, "Ann", ka, pa, pb, vec![friend(&b_id, &kb), friend(&f_id, &kf)]);
    let cb = cfg(&b_id, "Ann", kb, pb, pa, vec![friend(&a_id, &ka)]);
    let (a, arx) = Net::start(ca.clone()).unwrap();
    let (b, brx) = Net::start(cb.clone()).unwrap();
    wait_for(&arx, 5.0, is_connected).unwrap();
    wait_for(&brx, 5.0, is_connected).unwrap();
    assert_eq!(a.peer_version(&b_id), Some(3));
    // 서로 친구일 뿐: 친구 목록은 가지 않는다
    assert!(!a.send_device_friends(&b_id, false, |_| ("Fred".into(), None)));
    // A가 코드를 보여 주고 B에서 그 코드를 넣는다
    let code = a.new_link_code(&b_id).unwrap();
    assert_eq!(code.len(), 9, "{code}");
    assert_eq!(&code[4..5], "-");
    assert_eq!(b.enter_link_code(&a_id, &code.to_lowercase()), Ok(()));
    // 코드가 맞으면 양쪽에서 "본인의 PC가 맞습니까?"에 확인을 누른다
    assert_eq!(event_of(&arx, confirm_ask), NetEvent::DeviceConfirm { id: b_id.clone() });
    assert_eq!(event_of(&brx, confirm_ask), NetEvent::DeviceConfirm { id: a_id.clone() });
    assert!(a.confirm_device(&b_id, true));
    assert!(b.confirm_device(&a_id, true));
    assert_eq!(event_of(&arx, |e| matches!(e, NetEvent::DeviceLinked { .. })), NetEvent::DeviceLinked { id: b_id.clone() });
    assert_eq!(event_of(&brx, |e| matches!(e, NetEvent::DeviceLinked { .. })), NetEvent::DeviceLinked { id: a_id.clone() });
    let (sa, sb) = (a.device_secret(&b_id).expect("A stores it"), b.device_secret(&a_id).expect("B stores it"));
    assert_eq!(sa, sb, "both store the same secret");
    assert_eq!(format!("{sa:?}"), "DeviceSecret(..)", "never printed");
    assert_eq!((a.linked_device(), b.linked_device()), (Some(b_id.clone()), Some(a_id.clone())));
    assert!(a.send_device_friends(&b_id, false, |_| ("Fred".into(), None)));
    assert_eq!(wait_for(&brx, 2.0, added), Some(NetEvent::FriendAdded { id: f_id.clone(), name: "Fred".into(), via: None, side: None }));
    assert!(b.friends().contains(&friend(&f_id, &kf)));
    assert!(!b.send_my_devices(&a_id), "my devices are not introduced to my own devices");
    // 그 코드는 한 번만
    assert_eq!(b.enter_link_code(&a_id, &code), Ok(()));
    assert!(matches!(event_of(&brx, |e| matches!(e, NetEvent::DeviceLinkFailed { .. })), NetEvent::DeviceLinkFailed { reason, .. } if reason == "none"));
    // 껐다 켜면 저장한 비밀을 다시 증명한다
    let b_friends = b.friends();
    a.shutdown();
    b.shutdown();
    let (mut ca2, mut cb2) = (ca.clone(), cb.clone());
    ca2.device = Some((b_id.clone(), sa));
    cb2.device = Some((a_id.clone(), sb));
    cb2.friends = b_friends;
    let (a, arx) = Net::start(ca2).unwrap();
    let (b, brx) = Net::start(cb2).unwrap();
    assert_eq!(event_of(&arx, proven), NetEvent::DeviceProven { id: b_id.clone() });
    assert_eq!(event_of(&brx, proven), NetEvent::DeviceProven { id: a_id.clone() });
    assert!(a.send_device_friends(&b_id, false, |_| ("Fred".into(), None)));
    // 연결 끊기는 저쪽에도 알린다
    assert!(a.unlink_device(&b_id));
    assert_eq!(event_of(&brx, |e| matches!(e, NetEvent::DeviceUnlinked { .. })), NetEvent::DeviceUnlinked { id: a_id.clone() });
    assert_eq!((a.linked_device(), b.linked_device()), (None, None));
    assert_eq!((a.device_secret(&b_id), b.device_secret(&a_id)), (None, None));
    assert!(!a.send_device_friends(&b_id, false, |_| ("Fred".into(), None)));
    assert!(b.friends().iter().any(|f| f.id == a_id), "still friends");
    a.shutdown();
    b.shutdown();
}

#[test]
fn unfriending_my_other_computer_unlinks_both() {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let (a_id, b_id) = (hid(0xa1), hid(0xb2));
    let s = random32();
    let mut ca = cfg(&a_id, "Ann", ka, pa, pb, vec![friend(&b_id, &kb)]);
    let mut cb = cfg(&b_id, "Ann", kb, pb, pa, vec![friend(&a_id, &ka)]);
    linked(&mut ca, &b_id, s);
    linked(&mut cb, &a_id, s);
    let (a, arx) = Net::start(ca).unwrap();
    let (b, brx) = Net::start(cb).unwrap();
    event_of(&arx, proven);
    event_of(&brx, proven);
    a.unfriend(&b_id);
    wait_for(&brx, 5.0, is_disconnected).unwrap();
    assert_eq!((a.linked_device(), b.linked_device()), (None, None));
    assert!(b.friends().is_empty());
    a.shutdown();
    b.shutdown();
}
// ---- linked devices: introductions cannot squat someone's id -----------------

/// Announce `id` once on `net`'s discovery port (as a stranger on the LAN).
fn announce_to(net: &Net, id: &str) {
    let u = UdpSocket::bind("127.0.0.1:0").unwrap();
    let msg = json!({"app": "screenpet", "v": 3, "id": id, "name": "S", "port": 1}).to_string();
    u.send_to(msg.as_bytes(), local(net.discovery_port())).unwrap();
}

#[test]
fn an_introduction_cannot_take_the_id_of_someone_i_see_or_am_pairing_with() {
    // 고친 앱의 친구가 아직 친구가 아닌 사람의 id를 가짜 키로 "내 기기"라고 소개해, 그 사람과 짝맞추기를 못 하게 하려 한다
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let (seen, pairing, other) = (hid(20), hid(21), hid(22));
    // 지금 짝맞추기 중인 낯선 사람 (이모지 확인 단계)
    let (mut pp, _ev) = raw_pairing(port, &rx, &pairing);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 같은 와이파이에서 보이는 낯선 사람
    announce_to(&net, &seen);
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::PeerSeen { id, .. } if *id == seen)).is_some());
    // The raw pairing sends nothing on its own: keep it alive (silent past
    // `conn_timeout` it would be dropped and its id free for an introduction)
    pp.send(json!({"type": "ping"})).unwrap();
    for d in [&seen, &pairing, &other] {
        p.send(json!({"type": "my_devices", "devices": [wire(d, &key())]})).unwrap();
    }
    assert_eq!(added_ids(&rx, 1.0), vec![other.clone()], "only the one I know nothing about");
    assert!(!net.friends().iter().any(|f| f.id == seen || f.id == pairing));
    net.shutdown();
}

#[test]
fn a_real_pairing_replaces_a_friends_fake_introduction_of_that_id() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let (victim, real) = (hid(20), key());
    let mut z = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 친구가 아직 모르는 사람의 id를 가짜 키로 소개한다 (그 사람은 아직 안 보인다)
    z.send(json!({"type": "my_devices", "devices": [wire(&victim, &key())]})).unwrap();
    assert_eq!(added_ids(&rx, 1.0), vec![victim.clone()]);
    // 진짜 그 사람이 짝맞추기를 건다: 이모지를 확인하고 둘 다 받으면 진짜 키로 바뀐다
    let mut p = raw_connect(port, &real, &victim);
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "pair_random");
    p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
    let ev = wait_for(&rx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    p.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    net.answer_invite(pairing_of(&ev), true);
    assert_eq!(wait_for(&rx, 2.0, is_connected), Some(NetEvent::Connected { id: victim.clone(), name: "Raw".into() }));
    assert!(net.friends().contains(&friend(&victim, &real)));
    assert_eq!(net.friends().len(), 2);
    let info = net.friend_info(&victim).unwrap();
    assert!(info.via.is_none() && !info.pending && !info.device, "a plain friend now: {info:?}");
    // 확인된 친구의 키는 다시는 바뀌지 않는다
    let mut q = raw_connect_claiming(port, &key(), &victim);
    assert!(closed_by_peer(&mut q.s));
    assert!(net.friends().contains(&friend(&victim, &real)));
    net.shutdown();
}

#[test]
fn a_fake_introduction_confirmed_by_its_own_key_is_still_replaced_by_a_real_pairing() {
    // 고친 앱의 친구가 남의 id를 자기 키로 소개하고, 그 키로 직접 붙어 확인까지 마쳤다 (pending이 풀렸다).
    // 그래도 소개로만 들어온 항목이라, 진짜 그 사람과 이모지 확인을 거쳐 맺으면 진짜 키로 바뀐다
    let (kz, fake) = (key(), key());
    let (victim, real) = (hid(20), key());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&victim, &fake)]);
    c.introduced = vec![(victim.clone(), "zzzz".to_string())];
    let (net, rx, port) = sniffed(c);
    assert!(!net.friend_info(&victim).unwrap().pending);
    let mut p = raw_connect(port, &real, &victim);
    let n = random32();
    p.send(json!({"type": "invite", "pet": "cat", "commit": hex(&blake2s(&[&n]))})).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "pair_random");
    p.send(json!({"type": "pair_reveal", "nonce": hex(&n)})).unwrap();
    let ev = wait_for(&rx, 5.0, |e| matches!(e, NetEvent::Invited { .. })).expect("Invited");
    p.send(json!({"type": "invite_reply", "accepted": true})).unwrap();
    net.answer_invite(pairing_of(&ev), true);
    assert!(wait_for(&rx, 2.0, |e| matches!(e, NetEvent::Connected { id, .. } if *id == victim)).is_some());
    assert!(net.friends().contains(&friend(&victim, &real)));
    assert!(net.friend_info(&victim).unwrap().via.is_none());
    net.shutdown();
}

#[test]
fn i_can_still_invite_someone_a_friend_falsely_introduced() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let victim: &'static str = Box::leak(hid(20).into_boxed_str());
    let real = key();
    let mut z = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    z.send(json!({"type": "my_devices", "devices": [wire(victim, &key())]})).unwrap();
    assert_eq!(added_ids(&rx, 1.0), vec![victim.to_string()]);
    // 그 사람이 같은 와이파이에 나타난다: 내가 짝맞추기를 걸 수 있다
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    spoof_announcements(victim, listener.local_addr().unwrap().port(), net.discovery_port(), Duration::from_millis(100), stop.clone());
    let invited = Arc::new(AtomicBool::new(false));
    let inv = invited.clone();
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let Ok(s) = s else { break };
            let Ok(mut p) = handshake(s, &real, false) else { continue };
            if p.recv_msg().is_err() {
                continue; // its hello
            }
            let _ = p.send(json!({"type": "hello", "id": victim, "name": "V", "v": 3}));
            if p.recv_msg().is_ok_and(|m| m["type"] == "invite") {
                inv.store(true, Ordering::SeqCst);
                break;
            }
        }
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !invited.load(Ordering::SeqCst) && Instant::now() < deadline {
        net.invite(victim, "cat");
        std::thread::sleep(Duration::from_millis(200));
    }
    stop.store(true, Ordering::SeqCst);
    assert!(invited.load(Ordering::SeqCst), "an invite reaches the real person");
    net.shutdown();
}

// ---- linked devices: unconfirmed friends do not stay forever -------------------

const DAY: u64 = 24 * 60 * 60;

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
}

fn dropped(e: &NetEvent) -> bool {
    matches!(e, NetEvent::FriendDropped { .. })
}

#[test]
fn an_unconfirmed_friend_is_dropped_after_a_week() {
    let now = unix_now();
    let (old, fresh, ageless) = (hid(5), hid(6), hid(7));
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend(&old, &key()), friend(&fresh, &key()), friend(&ageless, &key())]);
    c.pending = vec![old.clone(), fresh.clone(), ageless.clone()];
    // 일주일 넘게 확인 못 한 친구, 엿새 된 친구, 언제 들어왔는지 모르는 (예전 설정의) 친구
    c.pending_added = vec![(old.clone(), now - 8 * DAY), (fresh.clone(), now - 6 * DAY)];
    let (net, rx, _) = sniffed(c);
    assert_eq!(wait_for(&rx, 2.0, dropped), Some(NetEvent::FriendDropped { id: old.clone() }));
    assert_eq!(wait_for(&rx, 0.5, dropped), None);
    let mut ids: Vec<String> = net.friends().into_iter().map(|f| f.id).collect();
    ids.sort();
    assert_eq!(ids, vec![fresh.clone(), ageless.clone()]);
    assert_eq!(net.friend_info(&fresh).unwrap().pending_since, Some(now - 6 * DAY), "the clock survives a restart");
    let since = net.friend_info(&ageless).unwrap().pending_since.expect("counts from this start");
    assert!(since >= now && since <= unix_now(), "{since} vs {now}");
    net.shutdown();
}

#[test]
fn an_introduced_friend_gets_its_clock_when_it_comes_in() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let before = unix_now();
    p.send(json!({"type": "my_devices", "devices": [wire(&hid(10), &key())]})).unwrap();
    assert_eq!(added_ids(&rx, 1.0), vec![hid(10)]);
    let since = net.friend_info(&hid(10)).unwrap().pending_since.expect("unconfirmed since now");
    assert!(since >= before && since <= unix_now());
    assert_eq!(net.friend_info("zzzz").unwrap().pending_since, None);
    net.shutdown();
}

#[test]
fn an_unconfirmed_friend_that_keeps_saying_it_does_not_know_us_is_dropped() {
    let kq = key();
    let q = hid(5);
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend(&q, &kq)]);
    c.pending = vec![q.clone()];
    let (net, rx, port) = sniffed(c);
    let deny = || {
        let mut p = handshake(TcpStream::connect(local(port)).unwrap(), &kq, true).unwrap();
        p.send(json!({"type": "hello", "id": q, "name": "Q", "v": 3})).unwrap();
        assert!(closed_by_peer(&mut p.s));
    };
    // 두 번까지는 소개가 늦는 중일 수 있다
    deny();
    deny();
    assert_eq!(wait_for(&rx, 0.3, dropped), None);
    assert!(net.friend_info(&q).unwrap().pending);
    // 세 번째: 정말 나를 모르는 친구다
    deny();
    assert_eq!(wait_for(&rx, 1.0, dropped), Some(NetEvent::FriendDropped { id: q.clone() }));
    assert!(net.friends().is_empty());
    net.shutdown();
}

// ---- linked devices: a code links two computers, a stored secret keeps them linked --

fn linked_ev(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceLinked { .. })
}

fn failed_ev(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceLinkFailed { .. })
}

/// Everything in `msgs` as one string, to look for leaks.
fn all_text(msgs: &[Value]) -> String {
    msgs.iter().map(Value::to_string).collect::<Vec<_>>().join("\n").to_uppercase()
}

#[test]
fn link_codes_are_eight_unambiguous_symbols_and_fresh_each_time() {
    let (net, _rx, _) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &key())]));
    let a = net.new_link_code("zzzz").unwrap();
    let b = net.new_link_code("zzzz").unwrap();
    assert_ne!(a, b);
    for c in [&a, &b] {
        let (left, right) = c.split_once('-').unwrap();
        assert_eq!((left.len(), right.len()), (4, 4), "{c}");
        assert!(c.chars().filter(|&ch| ch != '-').all(|ch| "ABCDEFGHJKMNPQRSTUVWXYZ23456789".contains(ch)), "{c}");
    }
    assert_eq!(net.enter_link_code("zzzz", "ABCD-EFG"), Err(LinkError::Invalid));
    assert_eq!(net.enter_link_code("zzzz", "ABCD-EFGO"), Err(LinkError::Invalid), "O is not a code symbol");
    assert_eq!(net.enter_link_code("zzzz", "ABCD-EFGH"), Err(LinkError::Offline));
    net.shutdown();
}

#[test]
fn the_right_code_links_and_the_code_never_crosses_the_wire() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(2), &key())]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let code = net.new_link_code("zzzz").unwrap();
    // 코드를 넣는 쪽이 먼저 증명한다: 맞는 증명이 오기 전엔 지키는 쪽은 증명을 내지 않는다
    let nb = random32();
    p.send(json!({"type": "device_code", "nonce": hex(&nb)})).unwrap();
    let r1 = p.recv_msg().unwrap();
    assert_eq!(r1["type"], "device_code_nonce");
    assert_eq!(r1.as_object().unwrap().len(), 2, "only a nonce: {r1}");
    let na = b32(&r1, "nonce");
    let (k, me, net_pub) = (code_key(&code), Net::public_key(&kz), p.remote);
    p.send(json!({"type": "device_code_proof", "proof": hex(&enter_proof(&k, &nb, &na, &me, &net_pub))})).unwrap();
    let r2 = p.recv_msg().unwrap();
    assert_eq!(r2["type"], "device_code_ok");
    assert_eq!(b32(&r2, "proof"), hold_proof(&k, &nb, &na, &net_pub, &me));
    confirm_both(&net, &rx, &mut p, "zzzz");
    assert_eq!(event_of(&rx, linked_ev), NetEvent::DeviceLinked { id: "zzzz".into() });
    let s = derive_secret(&k, &net_pub, &me, &na, &nb);
    assert_eq!(net.device_secret("zzzz"), Some(DeviceSecret(s)));
    assert_eq!(net.linked_device().as_deref(), Some("zzzz"));
    assert!(net.friend_info("zzzz").unwrap().device);
    // 연결된 그 링크에서 바로 친구 목록을 나눈다
    assert!(net.send_device_friends("zzzz", false, names));
    let rest = messages_within(&mut p, Duration::from_millis(400));
    // 코드도, 코드에서 나온 키도, 비밀도 선에 실리지 않는다
    let text = all_text(&[vec![r1, r2], rest].concat());
    for leak in [code.clone(), code.replace('-', ""), hex(&k).to_uppercase(), hex(&s).to_uppercase()] {
        assert!(!text.contains(&leak), "{leak} on the wire");
    }
    net.shutdown();
}

#[test]
fn typing_a_code_proves_it_without_sending_it() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let code = "QRST-2345";
    assert_eq!(net.enter_link_code("zzzz", "qrst 2345"), Ok(()));
    let m = p.recv_msg().unwrap();
    assert_eq!(m["type"], "device_code");
    let nb = b32(&m, "nonce");
    let na = random32();
    p.send(json!({"type": "device_code_nonce", "nonce": hex(&na)})).unwrap();
    let m2 = p.recv_msg().unwrap();
    assert_eq!(m2["type"], "device_code_proof");
    let (k, me, net_pub) = (code_key(code), Net::public_key(&kz), p.remote);
    assert_eq!(b32(&m2, "proof"), enter_proof(&k, &nb, &na, &net_pub, &me));
    assert!(!all_text(&[m, m2.clone()]).contains("QRST"), "the code is not on the wire");
    // 지키는 쪽의 증명이 틀리면 연결하지 않는다
    p.send(json!({"type": "device_code_ok", "proof": hex(&random32())})).unwrap();
    assert!(matches!(event_of(&rx, failed_ev), NetEvent::DeviceLinkFailed { reason, .. } if reason == "wrong"));
    assert_eq!(net.linked_device(), None);
    // 다시: 이번엔 맞다
    assert_eq!(net.enter_link_code("zzzz", code), Ok(()));
    let nb = b32(&p.recv_msg().unwrap(), "nonce");
    let na = random32();
    p.send(json!({"type": "device_code_nonce", "nonce": hex(&na)})).unwrap();
    assert_eq!(b32(&p.recv_msg().unwrap(), "proof"), enter_proof(&k, &nb, &na, &net_pub, &me));
    p.send(json!({"type": "device_code_ok", "proof": hex(&hold_proof(&k, &nb, &na, &me, &net_pub))})).unwrap();
    confirm_both(&net, &rx, &mut p, "zzzz");
    assert_eq!(event_of(&rx, linked_ev), NetEvent::DeviceLinked { id: "zzzz".into() });
    assert_eq!(net.device_secret("zzzz"), Some(DeviceSecret(derive_secret(&k, &me, &net_pub, &na, &nb))));
    net.shutdown();
}

#[test]
fn a_wrong_code_does_not_link_and_five_wrong_tries_cancel_the_code() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let code = net.new_link_code("zzzz").unwrap();
    let wrong = if code.starts_with('A') { "BBBB-BBBB" } else { "AAAA-AAAA" };
    for left in (1..5).rev() {
        let no = raw_enter(&mut p, wrong, &kz).unwrap_err();
        assert_eq!(no, json!({"type": "device_no", "reason": "wrong", "left": left}));
        assert_eq!(event_of(&rx, failed_ev), NetEvent::DeviceLinkFailed { id: "zzzz".into(), reason: "wrong".into(), left });
    }
    // 다섯 번째로 틀리면 코드가 없어진다: 이제 맞는 코드도 안 된다
    assert_eq!(raw_enter(&mut p, wrong, &kz).unwrap_err(), json!({"type": "device_no", "reason": "locked"}));
    assert_eq!(event_of(&rx, failed_ev), NetEvent::DeviceLinkFailed { id: "zzzz".into(), reason: "locked".into(), left: 0 });
    assert_eq!(raw_enter(&mut p, &code, &kz).unwrap_err(), json!({"type": "device_no", "reason": "none"}));
    assert_eq!(net.linked_device(), None);
    assert_eq!(wait_for(&rx, 0.3, is_disconnected), None, "the link stays");
    // 새 코드는 다시 쓸 수 있다
    let code = net.new_link_code("zzzz").unwrap();
    raw_enter(&mut p, &code, &kz).expect("a fresh code links");
    net.shutdown();
}

#[test]
fn a_code_is_only_for_the_friend_it_was_made_for() {
    let (kz, ky) = (key(), key());
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("yyyy", &ky)]));
    let code = net.new_link_code("yyyy").unwrap();
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(raw_enter(&mut p, &code, &kz).unwrap_err(), json!({"type": "device_no", "reason": "none"}));
    assert_eq!(net.linked_device(), None);
    net.shutdown();
}

#[test]
fn a_stored_secret_is_proven_again_on_every_link() {
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(2), &key())]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    prove_as_device(&mut p, &s, &kz);
    assert!(wait_for(&rx, 2.0, proven).is_some());
    assert!(net.send_device_friends("zzzz", false, names));
    drop(p);
    wait_for(&rx, 5.0, is_disconnected).expect("the link ends");
    // 새 링크: 다시 증명하기 전에는 목록을 보내지도 받지도 않는다
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "device_nonce");
    assert!(!net.send_device_friends("zzzz", false, names));
    p.send(json!({"type": "device_friends", "friends": [wire(&hid(30), &key())]})).unwrap();
    assert_eq!(wait_for(&rx, 0.6, added), None);
    net.shutdown();
}

#[test]
fn a_peer_with_a_wrong_secret_gets_no_friend_list_and_both_are_unlinked() {
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(2), &key())]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    // 비밀을 모르는 쪽: 다른 비밀로 증명한다
    prove_as_device_badly(&mut p, &kz);
    assert_eq!(event_of(&rx, |e| matches!(e, NetEvent::DeviceUnlinked { .. } | NetEvent::DeviceProven { .. })), NetEvent::DeviceUnlinked { id: "zzzz".into() });
    assert_eq!(net.linked_device(), None);
    assert!(!net.send_device_friends("zzzz", false, names));
    let got = messages_within(&mut p, Duration::from_millis(400));
    assert_eq!(got, vec![json!({"type": "device_unlink"})]);
    assert!(net.friends().iter().any(|f| f.id == "zzzz"), "still a friend");
    net.shutdown();
}

/// Like `prove_as_device`, but with a secret that is not the net's.
fn prove_as_device_badly(p: &mut Peer, mine: &[u8; 32]) {
    let theirs = p.recv_msg().unwrap();
    assert_eq!(theirs["type"], "device_nonce");
    let nn = b32(&theirs, "nonce");
    let nm = random32();
    p.send(json!({"type": "device_nonce", "nonce": hex(&nm)})).unwrap();
    assert_eq!(p.recv_msg().unwrap()["type"], "device_proof");
    let fake = random32();
    p.send(json!({"type": "device_proof", "proof": hex(&secret_proof(&fake, &Net::public_key(mine), &p.remote, &nn, &nm))})).unwrap();
}

#[test]
fn a_computer_without_our_secret_is_told_to_forget_it() {
    // 저쪽이 연결을 끊는 동안 내가 꺼져 있었다: 다음 연결에서 저쪽이 잊으라고 한다
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    p.send(json!({"type": "device_nonce", "nonce": hex(&random32())})).unwrap();
    assert_eq!(messages_within(&mut p, Duration::from_millis(500)), vec![json!({"type": "device_unlink"})]);
    net.shutdown();
}

#[test]
fn unlinking_is_told_to_the_other_computer_and_its_unlink_is_heard() {
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    prove_as_device(&mut p, &s, &kz);
    assert!(wait_for(&rx, 2.0, proven).is_some());
    assert!(net.unlink_device("zzzz"));
    assert!(!net.unlink_device("zzzz"), "nothing left to unlink");
    assert_eq!(but_active(messages_within(&mut p, Duration::from_millis(400))), vec![json!({"type": "device_unlink"})]);
    assert_eq!(net.linked_device(), None);
    assert!(!net.send_device_friends("zzzz", false, names));
    // 저쪽이 끊어도 듣는다
    let code = net.new_link_code("zzzz").unwrap();
    raw_enter(&mut p, &code, &kz).expect("linked again");
    confirm_both(&net, &rx, &mut p, "zzzz");
    assert!(wait_for(&rx, 2.0, linked_ev).is_some());
    assert_eq!(but_active(messages_within(&mut p, Duration::from_millis(300))), vec![json!({"type": "device_confirm"})]);
    p.send(json!({"type": "device_unlink"})).unwrap();
    assert_eq!(event_of(&rx, |e| matches!(e, NetEvent::DeviceUnlinked { .. })), NetEvent::DeviceUnlinked { id: "zzzz".into() });
    assert_eq!(net.linked_device(), None);
    assert_eq!(wait_for(&rx, 0.3, is_disconnected), None, "still friends");
    net.shutdown();
}

#[test]
fn only_one_other_computer_can_be_linked() {
    let (kz, ky) = (key(), key());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("yyyy", &ky)]);
    linked(&mut c, "zzzz", random32());
    let (net, rx, port) = sniffed(c);
    assert_eq!(net.new_link_code("yyyy"), Err(LinkError::Taken("zzzz".into())));
    let mut p = raw_connect_as(port, &ky, v3_hello("yyyy"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(net.enter_link_code("yyyy", "ABCD-EFGH"), Err(LinkError::Taken("zzzz".into())));
    // 셋째 컴퓨터가 코드를 넣어 봐도: 코드가 없다
    assert_eq!(raw_enter(&mut p, "ABCD-EFGH", &ky).unwrap_err()["type"], "device_no");
    assert_eq!(net.linked_device().as_deref(), Some("zzzz"));
    // 첫째를 끊으면 이제 된다
    assert!(net.unlink_device("zzzz"));
    let code = net.new_link_code("yyyy").unwrap();
    raw_enter(&mut p, &code, &ky).expect("linked");
    confirm_both(&net, &rx, &mut p, "yyyy");
    assert!(wait_for(&rx, 2.0, linked_ev).is_some());
    assert_eq!(net.linked_device().as_deref(), Some("yyyy"));
    net.shutdown();
}

#[test]
fn a_code_made_before_another_link_cannot_add_a_third_computer() {
    let (kz, ky) = (key(), key());
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("yyyy", &ky)]));
    let code_y = net.new_link_code("yyyy").unwrap();
    // zzzz가 먼저 이 컴의 코드를 받아 연결된다 (그 사이 yyyy 코드는 아직 살아 있다)
    let mut z = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(net.enter_link_code("zzzz", "ABCD-EFGH"), Ok(()));
    let nb = b32(&z.recv_msg().unwrap(), "nonce");
    let na = random32();
    z.send(json!({"type": "device_code_nonce", "nonce": hex(&na)})).unwrap();
    let _ = z.recv_msg().unwrap();
    let (k, me, net_pub) = (code_key("ABCD-EFGH"), Net::public_key(&kz), z.remote);
    z.send(json!({"type": "device_code_ok", "proof": hex(&hold_proof(&k, &nb, &na, &me, &net_pub))})).unwrap();
    confirm_both(&net, &rx, &mut z, "zzzz");
    assert!(wait_for(&rx, 2.0, linked_ev).is_some());
    drop(z);
    wait_for(&rx, 5.0, is_disconnected).unwrap();
    let mut y = raw_connect_as(port, &ky, v3_hello("yyyy"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert_eq!(raw_enter(&mut y, &code_y, &ky).unwrap_err(), json!({"type": "device_no", "reason": "taken"}));
    assert_eq!(net.linked_device().as_deref(), Some("zzzz"));
    net.shutdown();
}

// ---- linked devices: "본인의 PC가 맞습니까?" on both computers before anything is shared --

/// Two real nets that are friends: A (friend F too) and B. Returns them with A's code entered on B.
fn two_ready_to_confirm() -> (Net, Receiver<NetEvent>, Net, Receiver<NetEvent>, NetConfig, NetConfig, [String; 3]) {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb, kf) = (key(), key(), key());
    let (a_id, b_id, f_id) = (hid(0xa1), hid(0xb2), hid(0xf3));
    let ca = cfg(&a_id, "Ann", ka, pa, pb, vec![friend(&b_id, &kb), friend(&f_id, &kf)]);
    let cb = cfg(&b_id, "Ann", kb, pb, pa, vec![friend(&a_id, &ka)]);
    let (a, arx) = Net::start(ca.clone()).unwrap();
    let (b, brx) = Net::start(cb.clone()).unwrap();
    wait_for(&arx, 5.0, is_connected).unwrap();
    wait_for(&brx, 5.0, is_connected).unwrap();
    (a, arx, b, brx, ca, cb, [a_id, b_id, f_id])
}

/// A shows a code, B types it: both are asked to confirm.
fn match_code(a: &Net, arx: &Receiver<NetEvent>, b: &Net, brx: &Receiver<NetEvent>, a_id: &str, b_id: &str) {
    let code = a.new_link_code(b_id).unwrap();
    assert_eq!(b.enter_link_code(a_id, &code), Ok(()));
    assert_eq!(event_of(arx, confirm_ask), NetEvent::DeviceConfirm { id: b_id.into() });
    assert_eq!(event_of(brx, confirm_ask), NetEvent::DeviceConfirm { id: a_id.into() });
}

fn nothing_linked(a: &Net, b: &Net, a_id: &str, b_id: &str) {
    assert_eq!((a.linked_device(), b.linked_device()), (None, None));
    assert_eq!((a.device_secret(b_id), b.device_secret(a_id)), (None, None), "no secret to save");
    assert!(!a.friend_info(b_id).unwrap().device && !b.friend_info(a_id).unwrap().device, "the menu shows no link");
    assert!(!a.send_device_friends(b_id, false, names) && !b.send_device_friends(a_id, false, names));
}

#[test]
fn nothing_is_shared_or_saved_until_both_computers_confirm() {
    let (a, arx, b, brx, ca, cb, [a_id, b_id, f_id]) = two_ready_to_confirm();
    match_code(&a, &arx, &b, &brx, &a_id, &b_id);
    // 코드는 맞았지만 아직 아무도 확인을 누르지 않았다
    assert_eq!(wait_for(&arx, 0.5, linked_ev), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    assert!(!a.send_my_devices(&f_id), "no computer to introduce yet");
    // A만 확인했다: 저쪽을 기다린다
    assert!(a.confirm_device(&b_id, true));
    assert_eq!(wait_for(&arx, 0.5, linked_ev), None);
    assert_eq!(wait_for(&brx, 0.1, linked_ev), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    assert_eq!(wait_for(&brx, 0.3, added), None, "no friend list moved");
    // 확인 단계에서 꺼졌다 켜면 아무것도 연결돼 있지 않다
    b.shutdown();
    assert_eq!(event_of(&arx, cancelled), NetEvent::DeviceLinkCancelled { id: b_id.clone() });
    assert!(!a.confirm_device(&b_id, true), "nothing left to confirm");
    let (b, brx) = Net::start(cb.clone()).unwrap();
    wait_for(&brx, 5.0, is_connected).unwrap();
    assert_eq!(wait_for(&brx, 0.8, |e| proven(e) || linked_ev(e) || added(e)), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    a.shutdown();
    b.shutdown();
    let _ = ca;
}

#[test]
fn a_raw_peer_cannot_skip_the_confirm() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend(&hid(2), &key())]));
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    let code = net.new_link_code("zzzz").unwrap();
    raw_enter(&mut p, &code, &kz).expect("the code matched");
    assert_eq!(event_of(&rx, confirm_ask), NetEvent::DeviceConfirm { id: "zzzz".into() });
    // 저쪽이 먼저 확인하고 바로 친구 목록을 보내도: 이쪽이 확인하기 전에는 받지 않는다
    p.send(json!({"type": "device_confirm"})).unwrap();
    p.send(json!({"type": "device_friends", "friends": [wire(&hid(30), &key())]})).unwrap();
    assert_eq!(wait_for(&rx, 0.6, |e| added(e) || linked_ev(e)), None);
    assert_eq!(net.linked_device(), None);
    assert_eq!(net.device_secret("zzzz"), None);
    assert!(!net.send_device_friends("zzzz", false, names));
    assert_eq!(messages_within(&mut p, Duration::from_millis(300)), Vec::<Value>::new(), "nothing goes out before I confirm");
    // 이쪽도 확인: 이제 연결된다 (저쪽에도 확인을 알린다)
    assert!(net.confirm_device("zzzz", true));
    assert_eq!(event_of(&rx, linked_ev), NetEvent::DeviceLinked { id: "zzzz".into() });
    assert_eq!(but_active(messages_within(&mut p, Duration::from_millis(300))), vec![json!({"type": "device_confirm"})]);
    assert!(net.device_secret("zzzz").is_some());
    net.shutdown();
}

#[test]
fn cancel_on_either_side_cancels_both_and_saves_nothing() {
    let (a, arx, b, brx, _ca, _cb, [a_id, b_id, _]) = two_ready_to_confirm();
    // B가 취소 (A는 이미 확인했어도)
    match_code(&a, &arx, &b, &brx, &a_id, &b_id);
    assert!(a.confirm_device(&b_id, true));
    assert!(b.confirm_device(&a_id, false));
    assert_eq!(event_of(&brx, cancelled), NetEvent::DeviceLinkCancelled { id: a_id.clone() });
    assert_eq!(event_of(&arx, cancelled), NetEvent::DeviceLinkCancelled { id: b_id.clone() });
    assert_eq!(wait_for(&arx, 0.4, linked_ev), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    assert!(!a.confirm_device(&b_id, true) && !b.confirm_device(&a_id, true), "the confirm is gone");
    // A가 취소
    match_code(&a, &arx, &b, &brx, &a_id, &b_id);
    assert!(b.confirm_device(&a_id, true));
    assert!(a.confirm_device(&b_id, false));
    assert_eq!(event_of(&arx, cancelled), NetEvent::DeviceLinkCancelled { id: b_id.clone() });
    assert_eq!(event_of(&brx, cancelled), NetEvent::DeviceLinkCancelled { id: a_id.clone() });
    assert_eq!(wait_for(&brx, 0.4, linked_ev), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    assert_eq!(wait_for(&brx, 0.1, |e| matches!(e, NetEvent::Disconnected { .. })), None, "still friends, still connected");
    // 다시 하면 된다
    match_code(&a, &arx, &b, &brx, &a_id, &b_id);
    assert!(a.confirm_device(&b_id, true) && b.confirm_device(&a_id, true));
    event_of(&arx, linked_ev);
    event_of(&brx, linked_ev);
    assert_eq!(a.device_secret(&b_id), b.device_secret(&a_id));
    assert!(a.device_secret(&b_id).is_some());
    a.shutdown();
    b.shutdown();
}

#[test]
fn an_unanswered_confirm_is_cancelled_on_both_after_a_while() {
    let (a, arx, b, brx, _ca, _cb, [a_id, b_id, _]) = two_ready_to_confirm();
    let t0 = Instant::now();
    match_code(&a, &arx, &b, &brx, &a_id, &b_id);
    assert!(a.confirm_device(&b_id, true));
    // B는 대답하지 않는다
    assert_eq!(wait_for(&arx, 5.0, cancelled), Some(NetEvent::DeviceLinkCancelled { id: b_id.clone() }));
    assert_eq!(wait_for(&brx, 2.0, cancelled), Some(NetEvent::DeviceLinkCancelled { id: a_id.clone() }));
    assert!(t0.elapsed() >= fast().device_confirm_timeout, "{:?}", t0.elapsed());
    assert!(!b.confirm_device(&a_id, true), "too late");
    assert_eq!(wait_for(&arx, 0.4, linked_ev), None);
    nothing_linked(&a, &b, &a_id, &b_id);
    a.shutdown();
    b.shutdown();
}

#[test]
fn the_confirm_message_has_its_wire_shape_and_is_never_sent_to_an_older_build() {
    let kz = key();
    let (net, rx, port) = sniffed(cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]));
    let mut p = raw_connect_as(port, &kz, json!({"type": "hello", "id": "zzzz", "name": "Old", "v": 2, "friend": true}));
    wait_for(&rx, 5.0, is_connected).unwrap();
    assert!(!net.confirm_device("zzzz", true), "nothing to confirm");
    assert!(!net.confirm_device("zzzz", false));
    p.send(json!({"type": "device_confirm"})).unwrap();
    assert_eq!(messages_within(&mut p, Duration::from_millis(400)), Vec::<Value>::new());
    assert_eq!(wait_for(&rx, 0.2, |e| linked_ev(e) || cancelled(e)), None);
    net.shutdown();
}

// ---- 📱 one of my two computers plays at a time ------------------------------------

fn standby_ev(e: &NetEvent) -> bool {
    matches!(e, NetEvent::Standby { .. })
}

/// Wait until `net.standby()` is `want` (polling; events may have been read elsewhere).
fn standby_becomes(net: &Net, want: bool, secs: f64) -> bool {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < deadline {
        if net.standby() == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    net.standby() == want
}

/// Like `standby_becomes`, keeping raw peer `p` alive meanwhile (a raw peer sends no pings by itself).
fn standby_becomes_p(net: &Net, want: bool, secs: f64, p: &mut Peer) -> bool {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    loop {
        let _ = p.send(json!({"type": "ping"}));
        if net.standby() == want || Instant::now() >= deadline {
            return net.standby() == want;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait for an event, keeping raw peer `p` alive meanwhile.
fn wait_p(rx: &Receiver<NetEvent>, secs: f64, p: &mut Peer, pred: impl Fn(&NetEvent) -> bool) -> Option<NetEvent> {
    let deadline = Instant::now() + Duration::from_secs_f64(secs);
    while Instant::now() < deadline {
        let _ = p.send(json!({"type": "ping"}));
        if let Some(e) = wait_for(rx, 0.2, &pred) {
            return Some(e);
        }
    }
    None
}

/// As raw device `p`: my state, once per link (you play, I was used just now).
fn raw_sync(p: &mut Peer) {
    p.send(json!({"type": "device_active", "term": 0, "you": true, "idle": 0})).unwrap();
}

fn connected_to(rx: &Receiver<NetEvent>, secs: f64, id: &str) -> bool {
    wait_for(rx, secs, |e| matches!(e, NetEvent::Connected { id: i, .. } if i == id)).is_some()
}

fn disconnected_from(rx: &Receiver<NetEvent>, secs: f64, id: &str) -> bool {
    wait_for(rx, secs, |e| matches!(e, NetEvent::Disconnected { id: i } if i == id)).is_some()
}

/// A raw proven device "zzzz" claims the play: the net goes standby, tells the
/// app, and from then on lets no other friend in (closed before any hello).
#[test]
fn a_claim_from_my_other_computer_puts_this_one_on_standby_and_keeps_friends_out() {
    let ky = key();
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("yyyy", &ky)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    prove_as_device(&mut p, &s, &kz);
    assert!(wait_for(&rx, 2.0, proven).is_some());
    // 증명되면 내 상태를 알린다: 누가 놀고 있는지(term, 받는 쪽인지)와 내 유휴 시간
    let sync = p.recv_msg().unwrap();
    assert_eq!(sync["type"], "device_active", "{sync}");
    assert_eq!(sync["term"], 0);
    assert!(sync["idle"].as_u64().is_some(), "{sync}");
    raw_sync(&mut p);
    assert!(!net.standby());
    // 다른 친구는 아직 들어온다
    let y = raw_connect_as(port, &ky, v3_hello("yyyy"));
    assert!(connected_to(&rx, 5.0, "yyyy"));
    // 저쪽이 놀기 시작했다
    p.send(json!({"type": "device_active", "term": 1})).unwrap();
    // (내 상태를 나눈 직후엔 "저쪽이 쉰다"가 먼저 올 수 있다 — 이 컴퓨터가 쉬게 되는 것을 기다린다)
    assert_eq!(wait_p(&rx, 5.0, &mut p, |e| matches!(e, NetEvent::Standby { me: true, .. })), Some(NetEvent::Standby { me: true, device: false }));
    assert!(net.standby() && !net.device_standby());
    // 친구 링크는 잠깐 뒤 닫힌다 (그 사이에 내 펫을 불러올 수 있다)
    let mut y = y;
    assert!(closed_by_peer(&mut y.s));
    assert!(wait_p(&rx, 5.0, &mut p, |e| matches!(e, NetEvent::Disconnected { id } if id == "yyyy")).is_some());
    // 다시 들어오려 해도 hello 전에 닫힌다
    let mut again = handshake(TcpStream::connect(local(port)).unwrap(), &ky, true).unwrap();
    let _ = again.send(v3_hello("yyyy"));
    let got = again.recv();
    assert!(got.is_err(), "no hello for a friend while on standby: {got:?}");
    assert_eq!(wait_p(&rx, 0.5, &mut p, |e| matches!(e, NetEvent::Connected { id, .. } if id == "yyyy")), None);
    // 내 다른 컴퓨터와의 링크는 그대로
    assert_eq!(wait_p(&rx, 0.3, &mut p, |e| matches!(e, NetEvent::Disconnected { id } if id == "zzzz")), None);
    // 이 컴퓨터로 놀기: 저쪽에 알리고 대기에서 벗어난다
    assert!(net.claim_active());
    let claim = loop {
        let m = p.recv_msg().unwrap();
        if m["type"] != "hello" {
            break m;
        }
    };
    assert_eq!((claim["type"].as_str(), claim["term"].as_u64(), claim.get("you")), (Some("device_active"), Some(2), None), "{claim}");
    assert!(!net.standby() && net.device_standby(), "at once, as soon as the claim is made");
    assert_eq!(wait_for(&rx, 2.0, standby_ev), Some(NetEvent::Standby { me: false, device: true }));
    let _y = raw_connect_as(port, &ky, v3_hello("yyyy"));
    assert!(connected_to(&rx, 5.0, "yyyy"), "friends come back in");
    net.shutdown();
}

#[test]
fn an_older_claim_changes_nothing_and_my_other_computer_going_away_makes_this_one_play() {
    let (kz, s) = (key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    let mut p = raw_connect_as(port, &kz, v3_hello("zzzz"));
    wait_for(&rx, 5.0, is_connected).unwrap();
    prove_as_device(&mut p, &s, &kz);
    raw_sync(&mut p);
    p.send(json!({"type": "device_active", "term": 5})).unwrap();
    assert!(standby_becomes_p(&net, true, 2.0, &mut p));
    // 오래된 주장(작은 term)이나 "네가 논다"(you)도 순서대로 따진다
    p.send(json!({"type": "device_active", "term": 3, "you": true})).unwrap();
    assert!(!standby_becomes_p(&net, false, 0.4, &mut p), "an older claim is ignored");
    p.send(json!({"type": "device_active", "term": 6, "you": true})).unwrap();
    assert!(standby_becomes_p(&net, false, 2.0, &mut p), "a newer one that names me");
    p.send(json!({"type": "device_active", "term": 7})).unwrap();
    assert!(standby_becomes_p(&net, true, 2.0, &mut p));
    // 엉터리 값은 무시한다 (연결은 남는다)
    p.send(json!({"type": "device_active", "term": "lots"})).unwrap();
    p.send(json!({"type": "device_active", "term": -1, "you": true})).unwrap();
    p.send(json!({"type": "device_active", "term": 1u64 << 60, "you": true})).unwrap();
    assert!(!standby_becomes_p(&net, false, 0.4, &mut p));
    // 저쪽과 닿지 않으면 (잠깐 기다린 뒤) 이 컴퓨터가 논다
    drop(p);
    assert!(standby_becomes(&net, false, 5.0));
    assert!(!net.device_standby());
    net.shutdown();
}

#[test]
fn only_a_proven_device_link_may_claim_and_claims_never_reach_others() {
    let (kz, kw, ky, s) = (key(), key(), key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("wwww", &kw), friend("yyyy", &ky)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    // 그냥 친구(새 판)가 device_active를 보내도 아무 일도 없다
    let mut y = raw_connect_as(port, &ky, v3_hello("yyyy"));
    assert!(connected_to(&rx, 5.0, "yyyy"));
    y.send(json!({"type": "device_active", "term": 9})).unwrap();
    // 예전 판(v2)의 내 기기는 증명도 못 하고 아무것도 받지 않는다
    let mut z = raw_connect_as(port, &kz, json!({"type": "hello", "id": "zzzz", "name": "Old", "v": 2, "friend": true}));
    assert!(connected_to(&rx, 5.0, "zzzz"));
    z.send(json!({"type": "device_active", "term": 9})).unwrap();
    assert!(!standby_becomes(&net, true, 0.5));
    assert!(!net.claim_active(), "no proven link to tell");
    // raw peers stay silent otherwise: keep their links alive
    y.send(json!({"type": "ping"})).unwrap();
    z.send(json!({"type": "ping"})).unwrap();
    let w = raw_connect_as(port, &kw, json!({"type": "hello", "id": "wwww", "name": "Old", "v": 2, "friend": true}));
    assert!(connected_to(&rx, 5.0, "wwww"));
    let mut w = w;
    assert!(!net.claim_active());
    for (who, got) in messages_of_each(&mut [(&mut y, "v3 friend"), (&mut z, "v2 device"), (&mut w, "v2 friend")], Duration::from_millis(500)) {
        assert!(!all_text(&got).contains("DEVICE_ACTIVE"), "{who}: {got:?}");
    }
    assert_eq!(wait_for(&rx, 0.2, is_disconnected), None);
    net.shutdown();
}

#[test]
fn device_claim_floods_are_rate_limited_and_keep_the_link() {
    let (net, rx, mut p) = proven_device();
    raw_sync(&mut p);
    for t in 1..=60u64 {
        p.send(json!({"type": "device_active", "term": t})).unwrap();
    }
    assert!(standby_becomes_p(&net, true, 5.0, &mut p));
    // 홍수가 다 처리될 만큼 링크를 살려 둔다 (예산을 넘은 것은 그동안 버려진다)
    assert!(!standby_becomes_p(&net, false, 0.5, &mut p));
    assert_eq!(wait_for(&rx, 0.1, is_disconnected), None);
    // 예산(16개)을 넘은 것은 버려졌다: 마지막 주장까지 가지 않았다
    assert!(net.claim_active());
    let claim = messages_within(&mut p, Duration::from_millis(1500)).into_iter().find(|v| v["type"] == "device_active" && v.get("idle").is_none()).expect("claim");
    assert!(claim["term"].as_u64().unwrap() < 60, "{claim}");
    net.shutdown();
}

/// My computers A and B (linked) and friend F, all real Nets on one machine.
struct Three {
    a: Net,
    arx: Receiver<NetEvent>,
    b: Net,
    brx: Receiver<NetEvent>,
    f: Net,
    frx: Receiver<NetEvent>,
    ids: [String; 3],
}

fn three(a_idle: Option<u64>, b_idle: Option<u64>) -> Three {
    let (pa, pb, pf) = (free_udp_port(), free_udp_port(), free_udp_port());
    let (ka, kb, kf) = (key(), key(), key());
    let [a_id, b_id, f_id] = [hid(0xa1), hid(0xb2), hid(0xf3)];
    let s = random32();
    // A → B, B → F, F → A로 알린다: 셋 다 서로 연결된다
    let mut ca = cfg(&a_id, "Ann", ka, pa, pb, vec![friend(&b_id, &kb), friend(&f_id, &kf)]);
    let mut cb = cfg(&b_id, "Ann", kb, pb, pf, vec![friend(&a_id, &ka), friend(&f_id, &kf)]);
    let cf = cfg(&f_id, "Fred", kf, pf, pa, vec![friend(&a_id, &ka), friend(&b_id, &kb)]);
    linked(&mut ca, &b_id, s);
    linked(&mut cb, &a_id, s);
    let (a, arx) = Net::start(ca).unwrap();
    if let Some(i) = a_idle {
        a.set_idle(Duration::from_secs(i));
    }
    let (b, brx) = Net::start(cb).unwrap();
    if let Some(i) = b_idle {
        b.set_idle(Duration::from_secs(i));
    }
    let (f, frx) = Net::start(cf).unwrap();
    Three { a, arx, b, brx, f, frx, ids: [a_id, b_id, f_id] }
}

#[test]
fn friends_only_ever_see_the_computer_i_play_on_and_the_play_moves_with_a_claim() {
    // A를 오래 안 썼고 B를 방금 썼다: 둘이 닿으면 B가 논다
    let t = three(Some(600), Some(1));
    let [a_id, b_id, _] = t.ids.clone();
    assert_eq!(wait_for(&t.arx, 5.0, standby_ev), Some(NetEvent::Standby { me: true, device: false }));
    assert_eq!(wait_for(&t.brx, 5.0, standby_ev), Some(NetEvent::Standby { me: false, device: true }));
    assert!(t.a.standby() && !t.b.standby());
    // 친구는 B만 본다 (A가 먼저 붙었어도 끊기고, 다시 붙지 않는다)
    assert!(connected_to(&t.frx, 5.0, &b_id));
    let deadline = Instant::now() + Duration::from_secs(4);
    while t.f.peer_version(&a_id).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(t.f.peer_version(&a_id), None, "the standby computer is not connected to friends");
    assert!(!connected_to(&t.frx, 1.5, &a_id), "and stays off");
    assert!(t.f.peer_version(&b_id).is_some());
    // A에서 "이 컴퓨터로 놀기": 친구는 B를 잃고 A를 얻는다
    assert!(t.a.claim_active());
    assert!(standby_becomes(&t.b, true, 3.0));
    assert!(!t.a.standby());
    assert!(connected_to(&t.frx, 8.0, &a_id), "the computer I play on reconnects");
    assert!(disconnected_from(&t.frx, 8.0, &b_id));
    assert!(!connected_to(&t.frx, 1.5, &b_id));
    // 둘 사이 링크는 남는다
    assert!(t.a.peer_version(&b_id).is_some());
}

#[test]
fn on_reconnect_the_computer_used_last_plays_whichever_started_first() {
    let t = three(Some(1), Some(600));
    assert!(standby_becomes(&t.b, true, 5.0));
    assert!(!t.a.standby());
    assert!(t.a.device_standby() && !t.b.device_standby());
}

#[test]
fn two_claims_at_once_settle_on_exactly_one_computer() {
    let t = three(None, None);
    let [a_id, b_id, _] = t.ids.clone();
    // Each side proves the other on its own: both must be done before either can claim
    wait_for(&t.arx, 5.0, |e| matches!(e, NetEvent::DeviceProven { id } if *id == b_id)).unwrap();
    wait_for(&t.brx, 5.0, |e| matches!(e, NetEvent::DeviceProven { id } if *id == a_id)).unwrap();
    // 두 쪽이 같은 답에 닿을 때까지 (메시지가 오가는 시간만큼) 기다린다: 한 쪽만 놀고, 서로 그렇게 안다
    let settled = |t: &Three| {
        let (a, b) = (t.a.standby(), t.b.standby());
        a != b && t.a.device_standby() == b && t.b.device_standby() == a
    };
    for round in 0..5 {
        assert!(t.a.claim_active());
        assert!(t.b.claim_active());
        let deadline = Instant::now() + Duration::from_secs(5);
        while !settled(&t) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(settled(&t), "round {round}: a standby {}, b standby {}", t.a.standby(), t.b.standby());
        // 그리고 그대로 머문다 (다시 뒤집히지 않는다)
        let (a, b) = (t.a.standby(), t.b.standby());
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!((t.a.standby(), t.b.standby()), (a, b), "round {round}: it stays");
    }
}

#[test]
fn my_standby_computer_still_visits_the_one_i_play_on_and_its_pet_is_carried_on_to_friends() {
    let t = three(Some(1), Some(600));
    let [a_id, b_id, f_id] = t.ids.clone();
    assert!(standby_becomes(&t.b, true, 5.0));
    // B는 쉬어도 A와의 링크는 남는다: A가 B의 펫을 부르고, B의 펫이 A에 놀러 간다
    t.a.ask(&b_id, "invite");
    assert!(wait_for(&t.brx, 3.0, |e| matches!(e, NetEvent::Asked { from, ask } if *from == a_id && ask == "invite")).is_some());
    let mut v = sample_visit();
    t.b.send(&a_id, v.clone());
    assert!(wait_for(&t.arx, 3.0, |e| matches!(e, NetEvent::Received { from, .. } if *from == b_id)).is_some());
    // A가 그 펫을 친구 F에게 데려다준다: F는 B의 펫이라는 것(of)을 받는다
    assert!(connected_to(&t.frx, 5.0, &a_id));
    if let Visit::Visit { of, .. } = &mut v {
        *of = Some(b_id.clone());
    }
    t.a.send(&f_id, v);
    let got = wait_for(&t.frx, 3.0, |e| matches!(e, NetEvent::Received { .. })).expect("carried visit");
    assert!(matches!(&got, NetEvent::Received { from, msg: Visit::Visit { of: Some(o), .. } } if *from == a_id && *o == b_id), "{got:?}");
    // F에게 B는 연결되지 않는다 (쉬는 컴퓨터 — 둘이 서로 확인하기 전에 잠깐 붙었던 링크도 닫힌다)
    let deadline = Instant::now() + Duration::from_secs(4);
    while t.f.peer_version(&b_id).is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(t.f.peer_version(&b_id), None);
}

// ---- 📱 my two computers share the pet and its growth --------------------------------

fn care(pet: &str, stage: &str, stamina: f64) -> WireCare {
    WireCare { pet: pet.into(), stage: stage.into(), stamina, fullness: 0.5, locked: false }
}

fn state_ev(e: &NetEvent) -> bool {
    matches!(e, NetEvent::DeviceState { .. })
}

/// The `DeviceState` event's (pet, care), if `e` is one from `from`.
fn state_of(e: &NetEvent, from: &str) -> Option<(Option<String>, Vec<WireCare>)> {
    match e {
        NetEvent::DeviceState { from: f, pet, care } if f == from => Some((pet.clone(), care.clone())),
        _ => None,
    }
}

/// Wait until `rx` brings my other computer `from`'s state equal to (`pet`, `care`).
fn gets_state(rx: &Receiver<NetEvent>, secs: f64, from: &str, pet: &str, care: &[WireCare]) -> bool {
    wait_for(rx, secs, |e| state_of(e, from).is_some_and(|(p, c)| p.as_deref() == Some(pet) && c == care)).is_some()
}

/// My two linked computers A and B (real Nets), no friends besides each other.
struct Two {
    a: Net,
    arx: Receiver<NetEvent>,
    b: Net,
    brx: Receiver<NetEvent>,
    ids: [String; 2],
}

fn two(a_idle: u64, b_idle: u64, before: impl Fn(&Net, &Net)) -> Two {
    let (pa, pb) = (free_udp_port(), free_udp_port());
    let (ka, kb) = (key(), key());
    let [a_id, b_id] = [hid(0xa1), hid(0xb2)];
    let s = random32();
    let mut ca = cfg(&a_id, "Ann", ka, pa, pb, vec![friend(&b_id, &kb)]);
    let mut cb = cfg(&b_id, "Ann", kb, pb, pa, vec![friend(&a_id, &ka)]);
    linked(&mut ca, &b_id, s);
    linked(&mut cb, &a_id, s);
    let (a, arx) = Net::start(ca).unwrap();
    a.set_idle(Duration::from_secs(a_idle));
    let (b, brx) = Net::start(cb).unwrap();
    b.set_idle(Duration::from_secs(b_idle));
    before(&a, &b);
    Two { a, arx, b, brx, ids: [a_id, b_id] }
}

/// Wait until A plays and B is on standby, and both know it.
fn a_plays(t: &Two) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !(t.b.standby() && !t.a.standby() && t.a.device_standby()) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(t.b.standby() && !t.a.standby() && t.a.device_standby(), "A plays, B is on standby");
}

#[test]
fn the_computer_i_play_on_shares_its_pet_and_care_and_the_standby_one_takes_it() {
    let t = two(1, 600, |_, _| {});
    let [a_id, b_id] = t.ids.clone();
    a_plays(&t);
    let s1 = vec![care("cat", "child", 3.0), care("dog", "baby", 0.0)];
    t.a.share_state("cat", s1.clone());
    assert!(gets_state(&t.brx, 5.0, &a_id, "cat", &s1), "right away");
    // 펫을 바꾸고 먹이면 그것도 간다
    let s2 = vec![care("cat", "child", 3.0), care("dog", "kinder", 1.0)];
    t.a.share_state("dog", s2.clone());
    assert!(gets_state(&t.brx, 5.0, &a_id, "dog", &s2));
    // 같은 상태를 다시 알려도 새 소식은 없다
    t.a.share_state("dog", s2.clone());
    assert_eq!(wait_for(&t.brx, 0.8, state_ev), None);
    // 쉬는 컴퓨터의 상태는 노는 컴퓨터로 가지 않는다
    t.b.share_state("bird", vec![care("bird", "adult", 0.0)]);
    assert_eq!(wait_for(&t.arx, 1.0, |e| state_of(e, &b_id).is_some()), None);
}

#[test]
fn on_reconnect_the_state_of_the_computer_used_last_wins() {
    // 서로 닿지 않는 동안 둘 다 놀았다: 다시 만나면 마지막에 쓴 A의 상태를 B가 받는다
    let sa = vec![care("cat", "adult", 0.0)];
    let sb = vec![care("cat", "baby", 9.0), care("frog", "child", 1.0)];
    let t = two(1, 600, |a, b| {
        a.share_state("cat", sa.clone());
        b.share_state("frog", sb.clone());
    });
    let [a_id, b_id] = t.ids.clone();
    assert!(gets_state(&t.brx, 8.0, &a_id, "cat", &sa));
    a_plays(&t);
    assert_eq!(wait_for(&t.arx, 1.0, |e| state_of(e, &b_id).is_some()), None, "B's state never overwrites A's");
}

#[test]
fn taking_over_continues_from_the_latest_state_of_the_other_computer() {
    let t = two(1, 600, |_, _| {});
    let [a_id, b_id] = t.ids.clone();
    a_plays(&t);
    let s1 = vec![care("cat", "child", 3.0)];
    t.a.share_state("cat", s1.clone());
    assert!(gets_state(&t.brx, 5.0, &a_id, "cat", &s1));
    // A가 막 먹였는데(아직 모아 두는 중) B에서 "이 컴퓨터로 놀기": B는 A의 마지막 상태에서 이어 간다
    let s2 = vec![care("cat", "teenager", 0.0)];
    t.a.share_state("cat", s2.clone());
    assert!(t.b.claim_active());
    assert!(gets_state(&t.brx, 5.0, &a_id, "cat", &s2), "handed over");
    assert!(standby_becomes(&t.a, true, 5.0));
    // 이제 B가 놀고 B의 상태가 A로 간다
    let s3 = vec![care("cat", "adult", 0.0)];
    t.b.share_state("rabbit", s3.clone());
    assert!(gets_state(&t.arx, 5.0, &b_id, "rabbit", &s3));
}

#[test]
fn what_i_did_on_the_standby_computer_before_taking_over_is_kept() {
    let t = two(1, 600, |_, _| {});
    let [a_id, b_id] = t.ids.clone();
    a_plays(&t);
    let s1 = vec![care("cat", "child", 3.0)];
    t.a.share_state("cat", s1.clone());
    assert!(gets_state(&t.brx, 5.0, &a_id, "cat", &s1));
    // 쉬는 B에서 밥을 줬고 (앱은 그때 B로 논다고 알린다): A의 넘겨주기가 그것을 덮지 않는다
    let s2 = vec![care("cat", "child", 8.0)];
    t.b.share_state("cat", s2.clone());
    assert!(t.b.claim_active());
    assert!(gets_state(&t.arx, 5.0, &b_id, "cat", &s2), "A takes B's state");
    assert_eq!(wait_for(&t.brx, 0.8, |e| state_of(e, &a_id).is_some()), None, "B keeps its own");
}

#[test]
fn many_changes_go_out_as_a_few_messages_and_the_last_one_wins() {
    let (net, _rx, mut p) = proven_device();
    // 이 net이 논다 (저쪽: "네가 논다")
    raw_sync(&mut p);
    assert_eq!(messages_within(&mut p, Duration::from_millis(300)).iter().filter(|m| m["type"] == "device_state").count(), 0, "nothing to share yet");
    for i in 0..30 {
        net.share_state("cat", vec![care("cat", "baby", f64::from(i))]);
    }
    // (a raw peer sends no pings by itself: keep the link alive while listening)
    let got: Vec<Value> = (0..4)
        .flat_map(|_| {
            let _ = p.send(json!({"type": "ping"}));
            messages_within(&mut p, Duration::from_millis(250))
        })
        .filter(|m| m["type"] == "device_state")
        .collect();
    assert!(!got.is_empty() && got.len() <= 4, "coalesced: {got:?}");
    let last = got.last().unwrap();
    assert_eq!(last["care"][0]["stamina"], 29.0, "{last}");
    assert_eq!(last["pet"], "cat");
    assert!(last.get("handover").is_none());
    // 바뀐 게 없어도 가끔 다시 알린다 (상대가 놓쳤을 수도 있다)
    let again = (0..20).find_map(|_| {
        let _ = p.send(json!({"type": "ping"}));
        messages_within(&mut p, Duration::from_millis(300)).into_iter().find(|m| m["type"] == "device_state")
    });
    assert_eq!(again.as_ref().map(|m| &m["care"][0]["stamina"]), Some(&json!(29.0)), "slow refresh");
    net.shutdown();
}

#[test]
fn the_state_goes_only_to_my_proven_v3_computer_while_i_play_never_to_friends_or_older_builds() {
    let (kz, kw, ky, s) = (key(), key(), key(), random32());
    let mut c = cfg(&hid(1), "Me", key(), 0, 0, vec![friend("zzzz", &kz), friend("wwww", &kw), friend("yyyy", &ky)]);
    linked(&mut c, "zzzz", s);
    let (net, rx, port) = sniffed(c);
    net.share_state("cat", vec![care("cat", "child", 1.0)]);
    let mut y = raw_connect_as(port, &ky, v3_hello("yyyy"));
    assert!(connected_to(&rx, 5.0, "yyyy"));
    let mut w = raw_connect_as(port, &kw, json!({"type": "hello", "id": "wwww", "name": "Old", "v": 2, "friend": true}));
    assert!(connected_to(&rx, 5.0, "wwww"));
    y.send(json!({"type": "ping"})).unwrap(); // raw peers stay silent otherwise: keep y's link alive
    // 예전 판의 내 기기: 증명도 못 하고 상태도 받지 않는다
    let mut z = raw_connect_as(port, &kz, json!({"type": "hello", "id": "zzzz", "name": "Old", "v": 2, "friend": true}));
    assert!(connected_to(&rx, 5.0, "zzzz"));
    net.share_state("cat", vec![care("cat", "child", 2.0)]);
    for (who, got) in messages_of_each(&mut [(&mut y, "v3 friend"), (&mut z, "v2 device"), (&mut w, "v2 friend")], Duration::from_millis(600)) {
        assert!(!all_text(&got).contains("DEVICE_STATE"), "{who}: {got:?}");
    }
    // 그냥 친구가 보낸 상태는 무시한다
    y.send(json!({"type": "device_state", "pet": "dog", "care": [{"pet": "dog", "stage": "adult", "stamina": 0, "fullness": 1}]})).unwrap();
    assert_eq!(wait_for(&rx, 0.5, state_ev), None);
    assert_eq!(wait_for(&rx, 0.1, is_disconnected), None);
    net.shutdown();
}

#[test]
fn a_standby_computer_shares_nothing_and_takes_only_the_players_state() {
    let (net, rx, mut p) = proven_device();
    raw_sync(&mut p);
    // 아직 이 net이 논다: 저쪽 상태는 받지 않는다
    p.send(json!({"type": "device_state", "pet": "dog", "care": []})).unwrap();
    assert_eq!(wait_p(&rx, 0.5, &mut p, state_ev), None, "not from the computer on standby");
    // 저쪽이 논다
    p.send(json!({"type": "device_active", "term": 1})).unwrap();
    assert!(standby_becomes_p(&net, true, 3.0, &mut p));
    net.share_state("cat", vec![care("cat", "child", 1.0)]);
    assert!(!all_text(&messages_within(&mut p, Duration::from_millis(600))).contains("DEVICE_STATE"), "standby shares nothing");
    p.send(json!({"type": "device_state", "pet": "dog", "care": [{"pet": "dog", "stage": "adult", "stamina": 0, "fullness": 1}]})).unwrap();
    let got = wait_p(&rx, 3.0, &mut p, state_ev).expect("the player's state");
    assert_eq!(state_of(&got, "zzzz"), Some((Some("dog".into()), vec![WireCare { pet: "dog".into(), stage: "adult".into(), stamina: 0.0, fullness: 1.0, locked: false }])));
    net.shutdown();
}

#[test]
fn a_bad_state_is_trimmed_and_floods_are_rate_limited() {
    let (net, rx, mut p) = proven_device();
    raw_sync(&mut p);
    p.send(json!({"type": "device_active", "term": 1})).unwrap();
    assert!(standby_becomes_p(&net, true, 3.0, &mut p));
    let mut list: Vec<Value> = (0..300).map(|i| json!({"pet": format!("p{i}"), "stage": "baby", "stamina": 0, "fullness": 1})).collect();
    list.insert(0, json!({"pet": "Bad Pet!", "stage": "baby", "stamina": 0, "fullness": 1}));
    list.insert(0, json!({"pet": "x".repeat(200), "stage": "baby", "stamina": 0, "fullness": 1}));
    list.insert(0, json!({"pet": "cat", "stage": "s".repeat(200), "stamina": 0, "fullness": 1}));
    list.insert(0, json!({"pet": "cat", "stage": "baby", "stamina": 1e300, "fullness": -1e300}));
    p.send(json!({"type": "device_state", "pet": "y".repeat(500), "care": list})).unwrap();
    let got = wait_p(&rx, 3.0, &mut p, state_ev).expect("trimmed, not dropped");
    let Some((pet, care)) = state_of(&got, "zzzz") else { unreachable!() };
    assert_eq!(pet, None, "a pet id that is no pet id");
    assert!(care.len() <= 64, "{}", care.len());
    assert_eq!(care[0].pet, "cat", "numbers are the app's to clamp");
    assert!(care.iter().all(|c| c.pet.len() <= 32 && c.stage.len() <= 16 && c.pet.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')), "{care:?}");
    assert_eq!(care.iter().filter(|c| c.pet == "cat").count(), 1, "one entry per pet");
    // 홍수: 예산 안의 것만 (연결은 남는다)
    for i in 0..100 {
        p.send(json!({"type": "device_state", "pet": "cat", "care": [{"pet": "cat", "stage": "baby", "stamina": i, "fullness": 1}]})).unwrap();
    }
    let mut n = 0;
    while wait_p(&rx, 0.6, &mut p, state_ev).is_some() {
        n += 1;
    }
    assert!(n < 30, "{n} states taken");
    assert_eq!(wait_for(&rx, 0.1, is_disconnected), None);
    net.shutdown();
}
