//! In-process relay tests: a real relay on a localhost port and hand-rolled
//! WebSocket clients speaking the relay protocol.

use screenpet_relay::proto::{self, HEADER, MAX_FRAME};
use screenpet_relay::{Limits, Relay, RelayConfig};
use serde_json::{json, Value};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tungstenite::{Error as WsError, Message, WebSocket};

/// A place to connect to: an in-process relay or a spawned binary.
trait Target {
    fn addr(&self) -> SocketAddr;
}
impl Target for Relay {
    fn addr(&self) -> SocketAddr {
        self.local_addr()
    }
}
impl Target for SocketAddr {
    fn addr(&self) -> SocketAddr {
        *self
    }
}

fn start(limits: Limits) -> Relay {
    let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), None);
    cfg.limits = limits;
    Relay::start(cfg).unwrap()
}

fn key() -> [u8; 32] {
    proto::random32()
}

fn pubhex(k: &[u8; 32]) -> String {
    proto::to_hex(&proto::public_key(k))
}

struct Client {
    ws: WebSocket<TcpStream>,
    challenge: Value,
}

impl Client {
    /// Connect and read the challenge (not yet authenticated).
    fn raw(relay: &impl Target) -> Client {
        Client::raw_as(relay, None)
    }

    /// As `raw`, appearing (via the trusted loopback proxy header) to come
    /// from `ip`.
    fn raw_as(relay: &impl Target, ip: Option<&str>) -> Client {
        Client::try_raw(relay, ip, None).expect("upgrade")
    }

    /// As `raw_as`; `Err(HTTP status)` if the relay refused the upgrade.
    fn try_raw(relay: &impl Target, ip: Option<&str>, origin: Option<&str>) -> Result<Client, u16> {
        use tungstenite::client::IntoClientRequest;
        let addr = relay.addr();
        let tcp = TcpStream::connect(addr).unwrap();
        let mut req = format!("ws://{addr}/ws").into_client_request().unwrap();
        if let Some(ip) = ip {
            req.headers_mut().insert("X-Real-IP", ip.parse().unwrap());
        }
        if let Some(origin) = origin {
            req.headers_mut().insert("Origin", origin.parse().unwrap());
        }
        let mut ws = match tungstenite::client::client(req, tcp) {
            Ok((ws, _)) => ws,
            Err(tungstenite::HandshakeError::Failure(WsError::Http(r))) => return Err(r.status().as_u16()),
            Err(e) => panic!("handshake: {e}"),
        };
        ws.get_mut().set_read_timeout(Some(Duration::from_millis(20))).unwrap();
        let mut c = Client { ws, challenge: Value::Null };
        c.challenge = c.next_json(2.0).expect("challenge");
        assert_eq!(c.challenge["t"], "challenge");
        Ok(c)
    }

    fn auth_msg(&self, priv_key: &[u8; 32], id: &str, friends: &[String]) -> Value {
        self.auth_msg_with(priv_key, id, friends, None)
    }

    fn auth_msg_with(&self, priv_key: &[u8; 32], id: &str, friends: &[String], cnonce: Option<&[u8; 32]>) -> Value {
        let relay_pub = proto::hex32(self.challenge["relay"].as_str().unwrap()).unwrap();
        let nonce = proto::hex32(self.challenge["nonce"].as_str().unwrap()).unwrap();
        let shared = proto::dh(priv_key, &relay_pub).unwrap();
        let client_pub = proto::public_key(priv_key);
        let mac = proto::auth_mac(&shared, &nonce, &relay_pub, &client_pub, id);
        let mut v = json!({"t": "auth", "id": id, "key": proto::to_hex(&client_pub), "mac": proto::to_hex(&mac), "friends": friends});
        if let Some(c) = cnonce {
            v["cnonce"] = json!(proto::to_hex(c));
        }
        v
    }

    /// Connect and authenticate; returns the client and its `welcome`.
    fn login(relay: &impl Target, priv_key: &[u8; 32], id: &str, friends: &[String]) -> (Client, Value) {
        Client::login_as(relay, None, priv_key, id, friends).expect("welcome")
    }

    /// Log in from `ip`; `Err(reason)` if refused.
    fn login_as(relay: &impl Target, ip: Option<&str>, priv_key: &[u8; 32], id: &str, friends: &[String]) -> Result<(Client, Value), String> {
        let mut c = Client::raw_as(relay, ip);
        let auth = c.auth_msg(priv_key, id, friends);
        c.send(&auth);
        match c.next_json(2.0) {
            Some(w) if w["t"] == "welcome" => Ok((c, w)),
            Some(e) => Err(e["reason"].as_str().unwrap_or("?").to_string()),
            None => Err("closed".into()),
        }
    }

    fn send(&mut self, v: &Value) {
        // The relay may already have closed us (rate tests): ignore send errors.
        let _ = self.ws.send(Message::text(v.to_string()));
    }

    fn send_bin(&mut self, ch: u32, payload: &[u8]) {
        let mut b = ch.to_be_bytes().to_vec();
        b.extend_from_slice(payload);
        let _ = self.ws.send(Message::binary(b));
    }

    /// Next message of any kind; `Err` once the connection is closed.
    fn next(&mut self, secs: f64) -> Result<Option<Message>, ()> {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        while Instant::now() < deadline {
            match self.ws.read() {
                Ok(Message::Close(_)) => return Err(()),
                Ok(m @ (Message::Text(_) | Message::Binary(_))) => return Ok(Some(m)),
                Ok(_) => {}
                Err(WsError::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(_) => return Err(()),
            }
        }
        Ok(None)
    }

    fn next_json(&mut self, secs: f64) -> Option<Value> {
        match self.next(secs) {
            Ok(Some(Message::Text(t))) => Some(serde_json::from_str(t.as_str()).unwrap()),
            _ => None,
        }
    }

    /// Wait for a text message of type `t` (skipping others).
    fn expect(&mut self, t: &str, secs: f64) -> Value {
        self.wait(t, secs).unwrap_or_else(|| panic!("no {t} message"))
    }

    fn wait(&mut self, t: &str, secs: f64) -> Option<Value> {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        loop {
            let left = deadline.checked_duration_since(Instant::now())?.as_secs_f64();
            match self.next(left) {
                Ok(Some(Message::Text(s))) => {
                    let v: Value = serde_json::from_str(s.as_str()).unwrap();
                    if v["t"] == t {
                        return Some(v);
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) | Err(()) => return None,
            }
        }
    }

    /// Next binary message as (channel, payload), skipping text.
    fn recv_bin(&mut self, secs: f64) -> Option<(u32, Vec<u8>)> {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        loop {
            let left = deadline.checked_duration_since(Instant::now())?.as_secs_f64();
            match self.next(left) {
                Ok(Some(Message::Binary(b))) => {
                    return Some((u32::from_be_bytes([b[0], b[1], b[2], b[3]]), b[HEADER..].to_vec()));
                }
                Ok(Some(_)) => {}
                _ => return None,
            }
        }
    }

    /// The relay closed (or refused) this connection within `secs`; returns
    /// the `error` reason if one was sent.
    fn closed(&mut self, secs: f64) -> Option<Option<String>> {
        let deadline = Instant::now() + Duration::from_secs_f64(secs);
        let mut reason = None;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?.as_secs_f64();
            match self.next(left) {
                Ok(Some(Message::Text(s))) => {
                    let v: Value = serde_json::from_str(s.as_str()).unwrap();
                    if v["t"] == "error" {
                        reason = v["reason"].as_str().map(String::from);
                    }
                }
                Ok(Some(_)) => {}
                Ok(None) => return None,
                Err(()) => return Some(reason),
            }
        }
    }

    /// Redeem `code` and open a channel to its owner. Returns the channel.
    fn pair_open(&mut self, code: &str, req: u32) -> (u32, Value) {
        self.send(&json!({"t": "redeem", "code": code}));
        let r = self.expect("redeemed", 2.0);
        self.send(&json!({"t": "open", "req": req, "to": r["key"]}));
        let o = self.expect("opened", 2.0);
        assert_eq!(o["req"], req);
        (o["ch"].as_u64().unwrap() as u32, r)
    }
}

fn http_get(relay: &impl Target, path: &str) -> String {
    let mut s = TcpStream::connect(relay.addr()).unwrap();
    write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut out = String::new();
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let _ = s.read_to_string(&mut out);
    out
}

// ---- tests -----------------------------------------------------------------

#[test]
fn health_check_and_unknown_paths() {
    let relay = start(Limits::default());
    let ok = http_get(&relay, "/healthz");
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.ends_with("ok\n"));
    assert!(http_get(&relay, "/nope").starts_with("HTTP/1.1 404"));
    assert!(http_get(&relay, "/ws").starts_with("HTTP/1.1 426"), "plain GET on /ws is not an upgrade");
}

#[test]
fn register_gets_a_stable_friend_code() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (a, wa) = Client::login(&relay, &ka, "aaaa", &[]);
    let (_b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    let (ca, cb) = (wa["code"].as_str().unwrap().to_string(), wb["code"].as_str().unwrap().to_string());
    assert_eq!(proto::normalize_code(&ca).as_deref(), Some(ca.as_str()));
    assert_ne!(ca, cb);
    assert!(wa["code_ttl"].as_u64().unwrap() > 23 * 3600);
    drop(a);
    // Reconnecting with the same key keeps the code.
    let (_a2, wa2) = Client::login(&relay, &ka, "aaaa", &[]);
    assert_eq!(wa2["code"], ca.as_str());
}

#[test]
fn friend_code_pairing_session_forwards_both_ways() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[]);
    let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    // Lower-case / no dash input works too.
    let code = wb["code"].as_str().unwrap().replace('-', "").to_lowercase();
    let (ch, r) = a.pair_open(&code, 7);
    assert_eq!(r["id"], "bbbb");
    assert_eq!(r["key"], pubhex(&kb).as_str());
    let inc = b.expect("incoming", 2.0);
    assert_eq!(inc["ch"], ch);
    assert_eq!(inc["from"], pubhex(&ka).as_str());
    assert_eq!(inc["id"], "aaaa");
    a.send_bin(ch, b"hello b");
    assert_eq!(b.recv_bin(2.0), Some((ch, b"hello b".to_vec())));
    b.send_bin(ch, b"hello a");
    assert_eq!(a.recv_bin(2.0), Some((ch, b"hello a".to_vec())));
    // The grant was used up: a second channel needs friendship or a new code.
    a.send(&json!({"t": "open", "req": 8, "to": pubhex(&kb)}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "not_friends");
    // Close reaches the other side.
    a.send(&json!({"t": "close", "ch": ch}));
    assert_eq!(b.expect("close", 2.0)["ch"], ch);
}

#[test]
fn bad_codes_and_strangers_are_refused() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, wa) = Client::login(&relay, &ka, "aaaa", &[]);
    let (_b, _) = Client::login(&relay, &kb, "bbbb", &[]);
    a.send(&json!({"t": "redeem", "code": "ZZZZ-ZZZZ"}));
    assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    a.send(&json!({"t": "redeem", "code": "not a code"}));
    assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    // Own code and an offline owner's code look exactly like a wrong code.
    a.send(&json!({"t": "redeem", "code": wa["code"]}));
    assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    let kc = key();
    let (c, wc) = Client::login(&relay, &kc, "cccc", &[]);
    drop(c);
    std::thread::sleep(Duration::from_millis(200));
    a.send(&json!({"t": "redeem", "code": wc["code"]}));
    assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    // Not friends, no code: no channel.
    a.send(&json!({"t": "open", "req": 1, "to": pubhex(&kb)}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "not_friends");
    // A stranger's key answers the same whether it is online or not: nobody
    // can probe who is online.
    a.send(&json!({"t": "open", "req": 2, "to": pubhex(&key())}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "not_friends");
    a.send(&json!({"t": "open", "req": 3, "to": pubhex(&kc)}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "not_friends");
}

#[test]
fn offline_is_only_said_about_friends() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[pubhex(&kb)]);
    let (b, _) = Client::login(&relay, &kb, "bbbb", &[pubhex(&ka)]);
    drop(b);
    let deadline = Instant::now() + Duration::from_secs(3);
    while a.wait("presence", 2.0).is_some_and(|p| p["online"] != false) {
        assert!(Instant::now() < deadline);
    }
    a.send(&json!({"t": "open", "req": 1, "to": pubhex(&kb)}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "offline");
}

#[test]
fn friends_reconnect_by_key_with_presence() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, wa) = Client::login(&relay, &ka, "aaaa", &[pubhex(&kb)]);
    assert_eq!(wa["online"], json!([]));
    // Only one side lists the other: not mutual, no presence.
    let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    assert_eq!(wb["online"], json!([]));
    assert!(a.wait("presence", 0.3).is_none());
    // B adds A: both now see each other online.
    b.send(&json!({"t": "friends", "keys": [pubhex(&ka)]}));
    assert_eq!(b.expect("online", 2.0)["keys"], json!([pubhex(&ka)]));
    let p = a.expect("presence", 2.0);
    assert_eq!((p["key"].as_str().unwrap(), p["online"].as_bool().unwrap()), (pubhex(&kb).as_str(), true));
    // Friends open channels without any code.
    a.send(&json!({"t": "open", "req": 1, "to": pubhex(&kb)}));
    let ch = a.expect("opened", 2.0)["ch"].as_u64().unwrap() as u32;
    assert_eq!(b.expect("incoming", 2.0)["ch"], ch);
    // B goes away: A sees it offline and the channel closed.
    drop(b);
    let mut saw_close = false;
    let mut saw_offline = false;
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline && !(saw_close && saw_offline) {
        if let Some(v) = a.next_json(0.5) {
            saw_close |= v["t"] == "close" && v["ch"] == ch;
            saw_offline |= v["t"] == "presence" && v["online"] == false;
        }
    }
    assert!(saw_close && saw_offline);
    // B comes back (the relay remembered its friend list): presence again.
    let (_b2, wb2) = Client::login(&relay, &kb, "bbbb", &[pubhex(&ka)]);
    assert_eq!(wb2["online"], json!([pubhex(&ka)]));
    assert_eq!(a.expect("presence", 2.0)["online"], true);
}

#[test]
fn auth_needs_the_private_key() {
    let relay = start(Limits::default());
    let (ka, kc) = (key(), key());
    // C claims A's public key but can only compute the MAC with its own key.
    let mut c = Client::raw(&relay);
    let mut forged = c.auth_msg(&kc, "aaaa", &[]);
    forged["key"] = json!(pubhex(&ka));
    c.send(&forged);
    assert_eq!(c.closed(2.0), Some(Some("auth".into())));
    // Replaying A's genuine auth on another connection (new nonce) fails too.
    let a = Client::raw(&relay);
    let genuine = a.auth_msg(&ka, "aaaa", &[]);
    let mut replay = Client::raw(&relay);
    replay.send(&genuine);
    assert_eq!(replay.closed(2.0), Some(Some("auth".into())));
    // Anything but auth first is refused.
    let mut d = Client::raw(&relay);
    d.send(&json!({"t": "open", "req": 1, "to": pubhex(&ka)}));
    assert_eq!(d.closed(2.0), Some(Some("unexpected".into())));
    let mut e = Client::raw(&relay);
    e.send_bin(1, b"x");
    assert_eq!(e.closed(2.0), Some(Some("unexpected".into())));
}

#[test]
fn third_party_cannot_inject_or_close_a_channel() {
    let relay = start(Limits::default());
    let (ka, kb, kc) = (key(), key(), key());
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[]);
    let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    let (mut c, _) = Client::login(&relay, &kc, "cccc", &[]);
    let (ch, _) = a.pair_open(wb["code"].as_str().unwrap(), 1);
    b.expect("incoming", 2.0);
    // C guesses the channel id: its frames go nowhere and it cannot close it.
    for guess in ch.saturating_sub(2)..ch + 3 {
        c.send_bin(guess, b"evil");
        c.send(&json!({"t": "close", "ch": guess}));
    }
    a.send_bin(ch, b"real");
    assert_eq!(b.recv_bin(1.0), Some((ch, b"real".to_vec())));
    assert_eq!(b.recv_bin(0.3), None);
    b.send_bin(ch, b"reply");
    assert_eq!(a.recv_bin(1.0), Some((ch, b"reply".to_vec())));
    assert_eq!(a.recv_bin(0.3), None);
    // A second login with B's key (proving the key) replaces B; C still can't.
    let (_b2, _) = Client::login(&relay, &kb, "bbbb", &[]);
    assert_eq!(b.closed(2.0), Some(Some("replaced".into())));
    assert_eq!(a.expect("close", 2.0)["ch"], ch);
}

#[test]
fn channel_rate_limit_closes_the_channel() {
    let limits = Limits { chan_msgs_per_sec: 5.0, chan_msgs_burst: 5.0, ..Limits::default() };
    let relay = start(limits);
    let (ka, kb) = (key(), key());
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[]);
    let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    let (ch, _) = a.pair_open(wb["code"].as_str().unwrap(), 1);
    b.expect("incoming", 2.0);
    for _ in 0..20 {
        a.send_bin(ch, b"spam");
    }
    let mut got = 0;
    let mut closed = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && !closed {
        match b.next(0.2) {
            Ok(Some(Message::Binary(_))) => got += 1,
            Ok(Some(Message::Text(t))) => closed = serde_json::from_str::<Value>(t.as_str()).unwrap()["t"] == "close",
            _ => {}
        }
    }
    assert!(closed, "channel closed for exceeding its rate");
    assert_eq!(got, 5, "only the burst got through");
    assert_eq!(a.expect("close", 2.0)["ch"], ch);
}

#[test]
fn connection_rate_limit_disconnects() {
    let limits = Limits { conn_msgs_per_sec: 10.0, conn_msgs_burst: 10.0, ..Limits::default() };
    let relay = start(limits);
    let (mut a, _) = Client::login(&relay, &key(), "aaaa", &[]);
    for _ in 0..30 {
        a.send(&json!({"t": "ping"}));
    }
    assert_eq!(a.closed(2.0), Some(Some("rate".into())));
}

#[test]
fn frames_are_limited_to_64k() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[]);
    let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    let (ch, _) = a.pair_open(wb["code"].as_str().unwrap(), 1);
    b.expect("incoming", 2.0);
    let max = vec![7u8; MAX_FRAME];
    a.send_bin(ch, &max);
    assert_eq!(b.recv_bin(2.0).map(|(_, p)| p.len()), Some(MAX_FRAME));
    // One byte more: the connection is dropped.
    a.send_bin(ch, &vec![7u8; MAX_FRAME + 1]);
    assert!(a.closed(2.0).is_some());
    assert!(b.recv_bin(0.3).is_none());
}

#[test]
fn friend_list_is_capped_at_50() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let mut list: Vec<String> = (0..50).map(|_| pubhex(&key())).collect();
    list.push(pubhex(&kb)); // the 51st is ignored
    let (mut a, _) = Client::login(&relay, &ka, "aaaa", &list);
    let (_b, wb) = Client::login(&relay, &kb, "bbbb", &[pubhex(&ka)]);
    assert_eq!(wb["online"], json!([]));
    a.send(&json!({"t": "open", "req": 1, "to": pubhex(&kb)}));
    assert_eq!(a.expect("open_failed", 2.0)["reason"], "not_friends");
}

#[test]
fn redeem_attempts_are_rate_limited() {
    let relay = start(Limits::default());
    let (mut a, _) = Client::login(&relay, &key(), "aaaa", &[]);
    let mut reasons = Vec::new();
    for _ in 0..12 {
        a.send(&json!({"t": "redeem", "code": "ZZZZ-ZZZZ"}));
        reasons.push(a.expect("redeem_failed", 2.0)["reason"].as_str().unwrap().to_string());
    }
    assert_eq!(reasons.iter().filter(|r| *r == "unknown").count(), 10);
    assert_eq!(reasons.last().unwrap(), "rate");
}

#[test]
fn idle_and_unauthenticated_connections_time_out() {
    let limits =
        Limits { idle_timeout: Duration::from_millis(400), auth_timeout: Duration::from_millis(300), ..Limits::default() };
    let relay = start(limits);
    let mut pre = Client::raw(&relay);
    assert_eq!(pre.closed(2.0), Some(Some("timeout".into())));
    let (mut a, _) = Client::login(&relay, &key(), "aaaa", &[]);
    assert_eq!(a.closed(2.0), Some(Some("idle".into())));
    assert_eq!(relay.online_count(), 0);
}

#[test]
fn welcome_proves_the_relay_key() {
    let relay = start(Limits::default());
    let k = key();
    let mut c = Client::raw(&relay);
    let cnonce = proto::random32();
    let auth = c.auth_msg_with(&k, "aaaa", &[], Some(&cnonce));
    c.send(&auth);
    let w = c.expect("welcome", 2.0);
    let relay_pub = relay.public_key();
    assert_eq!(c.challenge["relay"], proto::to_hex(&relay_pub).as_str());
    let nonce = proto::hex32(c.challenge["nonce"].as_str().unwrap()).unwrap();
    let shared = proto::dh(&k, &relay_pub).unwrap();
    let expected = proto::welcome_proof(&shared, &cnonce, &nonce, &proto::public_key(&k));
    assert_eq!(w["proof"], proto::to_hex(&expected).as_str());
}

#[test]
fn new_code_on_request() {
    let relay = start(Limits::default());
    let (ka, kb) = (key(), key());
    let (mut a, wa) = Client::login(&relay, &ka, "aaaa", &[]);
    let (mut b, _) = Client::login(&relay, &kb, "bbbb", &[]);
    a.send(&json!({"t": "new_code"}));
    let fresh = a.expect("code", 2.0)["code"].as_str().unwrap().to_string();
    assert_ne!(fresh, wa["code"].as_str().unwrap());
    b.send(&json!({"t": "redeem", "code": wa["code"]}));
    assert_eq!(b.expect("redeem_failed", 2.0)["reason"], "unknown", "old code is dead");
    b.send(&json!({"t": "redeem", "code": fresh}));
    b.expect("redeemed", 2.0);
}

#[test]
fn redeems_per_target_count_distinct_requesters_per_code() {
    let relay = start(Limits { redeems_per_target_per_hour: 2, ..Limits::default() });
    let (mut b, wb) = Client::login(&relay, &key(), "bbbb", &[]);
    let code = wb["code"].clone();
    let redeem = |c: &mut Client, code: &Value| {
        c.send(&json!({"t": "redeem", "code": code}));
        c.wait("redeemed", 1.0).is_some()
    };
    // One requester trying again (e.g. a retry) counts once.
    let (mut x, _) = Client::login(&relay, &key(), "x", &[]);
    assert!(redeem(&mut x, &code) && redeem(&mut x, &code) && redeem(&mut x, &code));
    let (mut y, _) = Client::login(&relay, &key(), "y", &[]);
    assert!(redeem(&mut y, &code), "second distinct requester");
    let (mut z, _) = Client::login(&relay, &key(), "z", &[]);
    assert!(!redeem(&mut z, &code), "third distinct requester of this code is capped");
    // A new code starts a fresh budget.
    b.send(&json!({"t": "new_code"}));
    let fresh = b.expect("code", 2.0)["code"].clone();
    assert!(redeem(&mut z, &fresh));
}

#[test]
fn new_install_budget_is_per_network_bucket() {
    let relay = start(Limits { registrations_per_ip_per_hour: 2, ..Limits::default() });
    assert!(Client::login_as(&relay, Some("203.0.113.1"), &key(), "a", &[]).is_ok());
    assert!(Client::login_as(&relay, Some("203.0.113.2"), &key(), "b", &[]).is_ok());
    // Same /24: shares the budget.
    assert_eq!(Client::login_as(&relay, Some("203.0.113.3"), &key(), "c", &[]).err().as_deref(), Some("busy"));
    // Another network is not affected.
    assert!(Client::login_as(&relay, Some("198.51.100.7"), &key(), "d", &[]).is_ok());
    // IPv6: the /64 is the bucket.
    assert!(Client::login_as(&relay, Some("2001:db8::1"), &key(), "e", &[]).is_ok());
    assert!(Client::login_as(&relay, Some("2001:db8::2"), &key(), "f", &[]).is_ok());
    assert_eq!(Client::login_as(&relay, Some("2001:db8::3"), &key(), "g", &[]).err().as_deref(), Some("busy"));
}

#[test]
fn one_source_cannot_starve_the_global_redeem_budget() {
    let relay = start(Limits { redeems_per_min: 4, ..Limits::default() });
    let (_b, wb) = Client::login_as(&relay, Some("198.51.100.9"), &key(), "bbbb", &[]).unwrap();
    let (mut a, _) = Client::login_as(&relay, Some("203.0.113.1"), &key(), "aaaa", &[]).unwrap();
    // Malformed input costs nothing globally.
    for _ in 0..5 {
        a.send(&json!({"t": "redeem", "code": "hello"}));
        assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    }
    // Well-formed guesses use the global budget, then this heavy source is cut off…
    let mut reasons = Vec::new();
    for _ in 0..5 {
        a.send(&json!({"t": "redeem", "code": "ZZZZ-ZZZZ"}));
        reasons.push(a.expect("redeem_failed", 2.0)["reason"].as_str().unwrap().to_string());
    }
    assert_eq!(reasons.iter().filter(|r| *r == "rate").count(), 1, "{reasons:?}");
    // …but a light source still gets through.
    let (mut c, _) = Client::login_as(&relay, Some("192.0.2.5"), &key(), "cccc", &[]).unwrap();
    c.send(&json!({"t": "redeem", "code": wb["code"]}));
    c.expect("redeemed", 2.0);
}

#[test]
fn new_installs_are_capped_globally_and_in_total() {
    let relay = start(Limits { registrations_per_hour: 2, ..Limits::default() });
    let k1 = key();
    let _ = Client::login(&relay, &k1, "a", &[]);
    let _ = Client::login(&relay, &key(), "b", &[]);
    let mut c = Client::raw(&relay);
    let auth = c.auth_msg(&key(), "c", &[]);
    c.send(&auth);
    assert_eq!(c.closed(2.0), Some(Some("busy".into())));
    // Known installs still get in.
    let _ = Client::login(&relay, &k1, "a", &[]);

    // Full of online installs: refused. Once the friendless one is offline
    // it is evicted to make room.
    let relay = start(Limits { max_installs: 1, ..Limits::default() });
    let online = Client::login(&relay, &k1, "a", &[]);
    let mut c = Client::raw(&relay);
    let auth = c.auth_msg(&key(), "c", &[]);
    c.send(&auth);
    assert_eq!(c.closed(2.0), Some(Some("full".into())));
    drop(online);
    std::thread::sleep(Duration::from_millis(200));
    let d = Client::login_as(&relay, None, &key(), "d", &[]);
    assert!(d.is_ok(), "stale friendless install evicted: {:?}", d.err());
}

#[test]
fn the_fair_share_is_per_network() {
    let relay = start(Limits { redeems_per_min: 2, ..Limits::default() });
    let (_t, wt) = Client::login_as(&relay, Some("192.0.2.1"), &key(), "tttt", &[]).unwrap();
    let (mut a, _) = Client::login_as(&relay, Some("203.0.113.1"), &key(), "aaaa", &[]).unwrap();
    for _ in 0..2 {
        a.send(&json!({"t": "redeem", "code": "ZZZZ-ZZZZ"}));
        assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "unknown");
    }
    // A neighbour in the same /24 shares A's fair share: refused.
    let (mut b, _) = Client::login_as(&relay, Some("203.0.113.2"), &key(), "bbbb", &[]).unwrap();
    b.send(&json!({"t": "redeem", "code": wt["code"]}));
    assert_eq!(b.expect("redeem_failed", 2.0)["reason"], "rate");
    // Another network still gets through.
    let (mut c, _) = Client::login_as(&relay, Some("198.51.100.3"), &key(), "cccc", &[]).unwrap();
    c.send(&json!({"t": "redeem", "code": wt["code"]}));
    c.expect("redeemed", 2.0);
}

#[test]
fn light_sources_cannot_exceed_the_hard_global_ceiling() {
    // Budget 2/min → hard ceiling 4/min, however many networks share it.
    let relay = start(Limits { redeems_per_min: 2, ..Limits::default() });
    let mut reasons = Vec::new();
    for n in 1..=6 {
        let (mut c, _) = Client::login_as(&relay, Some(&format!("198.51.{n}.1")), &key(), &format!("c{n}"), &[]).unwrap();
        c.send(&json!({"t": "redeem", "code": "ZZZZ-ZZZZ"}));
        reasons.push(c.expect("redeem_failed", 2.0)["reason"].as_str().unwrap().to_string());
    }
    assert_eq!(reasons, ["unknown", "unknown", "unknown", "unknown", "rate", "rate"]);
}

#[test]
fn web_pages_are_refused_apps_send_no_origin() {
    let relay = start(Limits::default());
    assert_eq!(Client::try_raw(&relay, None, Some("https://evil.example")).err(), Some(403));
    assert!(Client::try_raw(&relay, None, None).is_ok());
}

/// Log in `n` installs from `ip`; how many were let in. Keeps them connected.
fn fill(relay: &Relay, ip: &str, n: usize, held: &mut Vec<Client>) -> usize {
    let mut admitted = 0;
    for _ in 0..n {
        if let Ok(mut c) = Client::try_raw(relay, Some(ip), None) {
            let auth = c.auth_msg(&key(), "x", &[]);
            c.send(&auth);
            c.expect("welcome", 2.0);
            held.push(c);
            admitted += 1;
        }
    }
    admitted
}

#[test]
fn connections_are_capped_per_network() {
    let relay = start(Limits { max_conns_per_ip: 2, max_conns_per_network: 3, ..Limits::default() });
    let mut held = Vec::new();
    assert_eq!(fill(&relay, "203.0.113.1", 5, &mut held), 2, "address cap");
    assert_eq!(fill(&relay, "203.0.113.2", 5, &mut held), 1, "same /24: network cap");
    assert_eq!(fill(&relay, "198.51.100.1", 1, &mut held), 1, "another network is not affected");
    // IPv6: distinct /64s in one /48 share the network cap.
    assert_eq!(fill(&relay, "2001:db8:1:1::1", 2, &mut held), 2);
    assert_eq!(fill(&relay, "2001:db8:1:2::1", 2, &mut held), 1);
    assert_eq!(fill(&relay, "2001:db8:2:1::1", 1, &mut held), 1);
    // A freed slot can be used again.
    drop(held.remove(0));
    let deadline = Instant::now() + Duration::from_secs(2);
    while Client::try_raw(&relay, Some("203.0.113.1"), None).is_err() {
        assert!(Instant::now() < deadline, "slot not released");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_full_challenge_pool_drops_its_oldest_not_the_newcomer() {
    let relay = start(Limits { max_unauthed: 4, max_unauthed_per_ip: 2, max_unauthed_per_network: 3, ..Limits::default() });
    let raw = |ip: &str| Client::try_raw(&relay, Some(ip), None).expect("never refused for the pool");
    // Per address.
    let mut first = raw("203.0.113.1");
    let mut second = raw("203.0.113.1");
    let _third = raw("203.0.113.1");
    assert_eq!(first.closed(2.0), Some(Some("busy".into())));
    // Per network: three between its addresses.
    let _fourth = raw("203.0.113.2");
    let _fifth = raw("203.0.113.3");
    assert_eq!(second.closed(2.0), Some(Some("busy".into())));
    // In all: a flood from elsewhere can't keep a real install out…
    let mut flood: Vec<Client> = ["198.51.100.1", "192.0.2.1", "198.18.0.1", "198.19.0.1"].iter().map(|ip| raw(ip)).collect();
    let (mut real, _) = Client::login_as(&relay, Some("100.64.0.1"), &key(), "real", &[]).expect("real client gets in");
    // …and installs that have authenticated are never in the pool.
    let late: Vec<Client> = (1..=8).map(|n| raw(&format!("10.{n}.0.1"))).collect();
    assert_eq!(flood[0].closed(2.0), Some(Some("busy".into())));
    real.send(&json!({"t": "ping"}));
    real.expect("pong", 2.0);
    drop(late);
}

/// `a` and `b`, mutual through a redeemed code, with a channel open.
fn linked(relay: &Relay, ip_a: Option<&str>, ip_b: Option<&str>) -> (Client, Client, u32) {
    let (mut a, _) = Client::login_as(relay, ip_a, &key(), "aaaa", &[]).unwrap();
    let (mut b, wb) = Client::login_as(relay, ip_b, &key(), "bbbb", &[]).unwrap();
    let (ch, _) = a.pair_open(wb["code"].as_str().unwrap(), 1);
    b.expect("incoming", 2.0);
    (a, b, ch)
}

#[test]
fn delivered_frames_stop_counting_against_the_byte_cap() {
    let relay = start(Limits { max_outbox_bytes: 50_000, chan_msgs_per_sec: 1000.0, chan_msgs_burst: 1000.0, ..Limits::default() });
    let (mut a, mut b, ch) = linked(&relay, None, None);
    for _ in 0..30 {
        a.send_bin(ch, &[7u8; 10_000]);
        assert_eq!(b.recv_bin(2.0).map(|(_, p)| p.len()), Some(10_000));
    }
}

#[test]
fn bandwidth_over_the_address_cap_closes_the_channel() {
    let relay = start(Limits { ip_bytes_per_sec: 1.0, ip_bytes_burst: 25_000.0, ..Limits::default() });
    let (mut a, mut b, ch) = linked(&relay, Some("203.0.113.1"), Some("198.51.100.1"));
    for _ in 0..3 {
        a.send_bin(ch, &[7u8; 10_000]);
    }
    assert_eq!(a.expect("close", 2.0)["ch"], ch);
    assert_eq!(b.recv_bin(2.0).map(|(_, p)| p.len()), Some(10_000));
    assert_eq!(b.expect("close", 2.0)["ch"], ch);
}

#[test]
fn bandwidth_over_the_global_cap_closes_the_channel() {
    let relay = start(Limits { global_bytes_per_sec: 1.0, global_bytes_burst: 25_000.0, ..Limits::default() });
    let (mut a, _b, ch) = linked(&relay, Some("203.0.113.1"), Some("198.51.100.1"));
    for _ in 0..3 {
        a.send_bin(ch, &[7u8; 10_000]);
    }
    assert_eq!(a.expect("close", 2.0)["ch"], ch);
}

/// A fresh data directory (short path: it holds a Unix socket).
fn data_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{tag}-{}", proto::to_hex(&key()[..4])));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn start_in(dir: &std::path::Path, limits: Limits) -> Relay {
    let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), Some(dir.to_path_buf()));
    cfg.limits = limits;
    Relay::start(cfg).unwrap()
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ok() {
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn new_installs_are_capped_per_wider_network_per_day() {
    let relay = start(Limits { registrations_per_network_per_day: 2, ..Limits::default() });
    assert!(Client::login_as(&relay, Some("2001:db8:1:1::1"), &key(), "a", &[]).is_ok());
    assert!(Client::login_as(&relay, Some("2001:db8:1:2::1"), &key(), "b", &[]).is_ok());
    let third = Client::login_as(&relay, Some("2001:db8:1:3::1"), &key(), "c", &[]);
    assert_eq!(third.err().as_deref(), Some("busy"), "another /64 in the same /48");
    assert!(Client::login_as(&relay, Some("2001:db8:2:1::1"), &key(), "d", &[]).is_ok());
}

#[test]
fn one_network_cannot_use_up_the_per_code_table() {
    let relay = start(Limits { new_codes_per_network_per_hour: 1, ..Limits::default() });
    let (_b, wb) = Client::login_as(&relay, Some("192.0.2.1"), &key(), "b", &[]).unwrap();
    let (_c, wc) = Client::login_as(&relay, Some("192.0.2.2"), &key(), "c", &[]).unwrap();
    let (mut a, _) = Client::login_as(&relay, Some("203.0.113.1"), &key(), "a", &[]).unwrap();
    a.send(&json!({"t": "redeem", "code": wb["code"]}));
    a.expect("redeemed", 2.0);
    a.send(&json!({"t": "redeem", "code": wc["code"]}));
    assert_eq!(a.expect("redeem_failed", 2.0)["reason"], "rate");
    // The same code again adds nothing to the table.
    a.send(&json!({"t": "redeem", "code": wb["code"]}));
    a.expect("redeemed", 2.0);
    // Another network has its own allowance.
    let (mut d, _) = Client::login_as(&relay, Some("198.51.100.1"), &key(), "d", &[]).unwrap();
    d.send(&json!({"t": "redeem", "code": wc["code"]}));
    d.expect("redeemed", 2.0);
}

#[test]
fn friend_list_updates_are_rate_limited() {
    let relay = start(Limits::default());
    let (mut a, _) = Client::login(&relay, &key(), "aaaa", &[]);
    for _ in 0..40 {
        a.send(&json!({"t": "friends", "keys": [pubhex(&key())]}));
    }
    assert_eq!(a.closed(2.0), Some(Some("rate".into())));
}

#[test]
fn data_is_written_on_the_timer_not_on_every_change() {
    let dir = data_dir("save");
    let file = dir.join("relay.json");
    let k = key();
    {
        let relay = start_in(&dir, Limits { save_every: Duration::from_secs(3600), ..Limits::default() });
        let (mut a, _) = Client::login(&relay, &k, "aaaa", &[]);
        a.send(&json!({"t": "friends", "keys": [pubhex(&key())]}));
        a.expect("online", 2.0);
        // The old loop wrote within two seconds.
        std::thread::sleep(Duration::from_millis(2500));
        assert!(!file.exists(), "no write yet");
    }
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(&pubhex(&k)), "written when the relay stopped");
    assert!(!text.contains('\n'), "compact");
    {
        let relay = start_in(&dir, Limits { save_every: Duration::from_millis(100), ..Limits::default() });
        let k2 = key();
        let _a = Client::login(&relay, &k2, "bbbb", &[]);
        wait_until("timed save", || std::fs::read_to_string(&file).is_ok_and(|t| t.contains(&pubhex(&k2))));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restart_does_not_forget_installs_that_were_online() {
    let dir = data_dir("restart");
    let k = key();
    let two_days_ago = screenpet_relay::store::unix_now() - 2 * 24 * 3600;
    let now = screenpet_relay::store::unix_now();
    let file = json!({"installs": {pubhex(&k): {"id": "a", "code": "ABCD-1234", "code_at": now, "last_seen": two_days_ago}}});
    std::fs::write(dir.join("relay.json"), file.to_string()).unwrap();
    let relay = start_in(&dir, Limits::default());
    // The old loop pruned two seconds after start, before clients came back.
    std::thread::sleep(Duration::from_millis(2500));
    let (_a, w) = Client::login(&relay, &k, "a", &[]);
    assert_eq!(w["code"], "ABCD-1234", "still known, friend code kept");
    drop(relay);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real binary on a free port.
#[cfg(unix)]
struct Bin {
    child: std::process::Child,
    addr: SocketAddr,
}

#[cfg(unix)]
impl Bin {
    fn start(dir: &std::path::Path) -> Bin {
        use std::io::BufRead;
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_screenpet-relay"))
            .args(["--bind", "127.0.0.1:0", "--data"])
            .arg(dir)
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut lines = std::io::BufReader::new(child.stderr.take().unwrap()).lines();
        let addr = loop {
            let line = lines.next().expect("relay output").unwrap();
            if let Some(rest) = line.strip_prefix("screenpet-relay listening on ") {
                break rest.split(' ').next().unwrap().parse().unwrap();
            }
        };
        // Keep the pipe drained so its logging can't block.
        std::thread::spawn(move || lines.for_each(drop));
        Bin { child, addr }
    }

    fn signal(&self, name: &str) {
        let pid = self.child.id().to_string();
        assert!(std::process::Command::new("kill").args([name, &pid]).status().unwrap().success());
    }

    fn wait_exit(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "relay did not stop");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[cfg(unix)]
impl Drop for Bin {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
#[test]
fn sigterm_tells_clients_and_saves() {
    let dir = data_dir("term");
    let mut bin = Bin::start(&dir);
    let k = key();
    let (mut a, _) = Client::login(&bin.addr, &k, "aaaa", &[]);
    bin.signal("-TERM");
    assert_eq!(a.closed(5.0), Some(Some("shutdown".into())));
    assert!(bin.wait_exit().success());
    let text = std::fs::read_to_string(dir.join("relay.json")).expect("saved on the way out");
    assert!(text.contains(&pubhex(&k)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The default connection cap can't run into a low soft descriptor limit.
#[cfg(target_os = "linux")]
#[test]
fn the_binary_raises_its_file_descriptor_limit() {
    let dir = data_dir("fds");
    let bin = Bin::start(&dir);
    let limits = std::fs::read_to_string(format!("/proc/{}/limits", bin.child.id())).unwrap();
    let line = limits.lines().find(|l| l.starts_with("Max open files")).unwrap();
    let mut fields = line.split_whitespace().skip(3);
    let (soft, hard): (u64, u64) = (fields.next().unwrap().parse().unwrap(), fields.next().unwrap().parse().unwrap());
    assert!(soft >= (Limits::default().max_conns as u64 + 64).min(hard), "{line}");
    drop(bin);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reconnecting_does_not_refill_the_byte_budget() {
    let relay = start(Limits { ip_bytes_per_sec: 1.0, ip_bytes_burst: 25_000.0, ..Limits::default() });
    let (mut a, mut b, ch) = linked(&relay, Some("203.0.113.1"), Some("198.51.100.1"));
    for _ in 0..2 {
        a.send_bin(ch, &[7u8; 10_000]);
        assert!(b.recv_bin(2.0).is_some());
    }
    drop((a, b));
    let (mut a, _b, ch) = linked(&relay, Some("203.0.113.1"), Some("198.51.100.1"));
    a.send_bin(ch, &[7u8; 10_000]);
    assert_eq!(a.expect("close", 2.0)["ch"], ch);
}

#[test]
fn one_source_over_the_global_byte_budget_leaves_others_alone() {
    let limits = Limits { global_bytes_per_sec: 1.0, global_bytes_burst: 320_000.0, chan_msgs_burst: 1000.0, ..Limits::default() };
    let relay = start(limits);
    let (mut heavy, _h, ch) = linked(&relay, Some("203.0.113.1"), Some("198.51.100.1"));
    let (mut light, mut other, ch2) = linked(&relay, Some("192.0.2.1"), Some("198.18.0.1"));
    for _ in 0..40 {
        heavy.send_bin(ch, &[7u8; 10_000]);
    }
    assert_eq!(heavy.expect("close", 2.0)["ch"], ch);
    light.send_bin(ch2, &[7u8; 10_000]);
    assert_eq!(other.recv_bin(2.0).map(|(c, _)| c), Some(ch2));
}

#[test]
fn a_wide_network_cannot_use_up_the_per_code_table_through_many_48s() {
    let relay = start(Limits { new_codes_per_network_per_hour: 1, ..Limits::default() });
    let codes: Vec<(Client, Value)> =
        (1..=6).map(|n| Client::login_as(&relay, Some(&format!("192.0.2.{n}")), &key(), "t", &[]).unwrap()).collect();
    let reasons: Vec<String> = codes
        .iter()
        .enumerate()
        .map(|(n, (_, w))| {
            let (mut a, _) = Client::login_as(&relay, Some(&format!("2001:db8:{:x}::1", n + 1)), &key(), "a", &[]).unwrap();
            a.send(&json!({"t": "redeem", "code": w["code"]}));
            let reply = a.next_json(2.0).unwrap();
            if reply["t"] == "redeemed" { "ok".to_string() } else { reply["reason"].as_str().unwrap().to_string() }
        })
        .collect();
    assert_eq!(reasons, ["ok", "ok", "ok", "ok", "rate", "rate"], "four networks' worth per /32");
}

// ---- admin commands --------------------------------------------------------

#[cfg(unix)]
mod admin {
    use super::*;
    use screenpet_relay::admin::request;
    use std::path::Path;

    fn ok(dir: &Path, words: &[&str]) -> String {
        let (good, text) = request(dir, words).unwrap();
        assert!(good, "{words:?}: {text}");
        text
    }

    fn refused(dir: &Path, words: &[&str]) -> String {
        let (good, text) = request(dir, words).unwrap();
        assert!(!good, "{words:?}: {text}");
        text
    }

    #[test]
    fn status_shows_who_is_online_and_counts() {
        let dir = data_dir("status");
        let relay = start_in(&dir, Limits::default());
        let (ka, kb) = (key(), key());
        let (_a, _) = Client::login_as(&relay, Some("203.0.113.5"), &ka, "first", &[pubhex(&kb)]).unwrap();
        let (_b, _) = Client::login_as(&relay, Some("2001:db8:1:2::9"), &kb, "second", &[pubhex(&ka)]).unwrap();
        let mut bad = Client::raw(&relay);
        let mut forged = bad.auth_msg(&key(), "x", &[]);
        forged["key"] = json!(pubhex(&ka));
        bad.send(&forged);
        bad.closed(2.0);
        wait_until("auth counted", || ok(&dir, &["status"]).contains("auth_failed 1"));
        let text = ok(&dir, &["status"]);
        for part in ["relay on, registration on, friendcode on", "online 2", "stored 2", &pubhex(&ka)[..12], "\"first\"", "203.0.113.5", "2001:db8:1:2::/64", "friends 1 (1 mutual)", "conns_admitted 3", "registrations 2"] {
            assert!(text.contains(part), "{part} missing in:\n{text}");
        }
        assert!(refused(&dir, &["nonsense"]).contains("unknown command"));
        drop(relay);
        assert!(!dir.join("admin.sock").exists(), "socket removed at stop");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_socket_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = data_dir("sock");
        let _relay = start_in(&dir, Limits::default());
        assert_eq!(std::fs::metadata(dir.join("admin.sock")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn kick_disconnects_with_a_reason_and_it_may_come_back() {
        let dir = data_dir("kick");
        let relay = start_in(&dir, Limits::default());
        let (ka, kb) = (key(), key());
        let (mut a, _) = Client::login(&relay, &ka, "kickme", &[pubhex(&kb)]);
        let (mut b, _) = Client::login(&relay, &kb, "other", &[pubhex(&ka)]);
        ok(&dir, &["kick", &pubhex(&ka)[..10]]);
        assert_eq!(a.closed(2.0), Some(Some("kicked".into())));
        let deadline = Instant::now() + Duration::from_secs(3);
        while b.wait("presence", 2.0).is_some_and(|p| p["online"] != false) {
            assert!(Instant::now() < deadline);
        }
        let (_a2, _) = Client::login(&relay, &ka, "kickme", &[]);
        ok(&dir, &["kick", "kickme"]);
        assert!(refused(&dir, &["kick", "kickme"]).contains("not online"));
        assert!(refused(&dir, &["kick", "nobody-here"]).contains("no install"));
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_prefix_matching_several_refuses_and_lists_them() {
        let dir = data_dir("ambig");
        let relay = start_in(&dir, Limits::default());
        let (ka, kb) = (key(), key());
        let (mut a, _) = Client::login(&relay, &ka, "same-1", &[]);
        let (mut b, _) = Client::login(&relay, &kb, "same-2", &[]);
        for command in ["kick", "ban", "purge"] {
            let text = refused(&dir, &[command, "same"]);
            assert!(text.contains("matches 2") && text.contains(&pubhex(&ka)) && text.contains(&pubhex(&kb)), "{text}");
        }
        assert!(refused(&dir, &["kick", "s"]).contains("no install"), "too short to be a prefix");
        assert!(a.closed(0.3).is_none() && b.closed(0.1).is_none(), "nothing happened");
        assert!(ok(&dir, &["bans"]).contains("keys (0)"));
        ok(&dir, &["kick", "same-1"]);
        assert!(a.closed(2.0).is_some());
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_ban_disconnects_refuses_and_survives_a_restart() {
        let dir = data_dir("ban");
        let k = key();
        let unseen = key();
        {
            let relay = start_in(&dir, Limits::default());
            let (mut a, _) = Client::login(&relay, &k, "banme", &[]);
            ok(&dir, &["ban", "banme"]);
            assert_eq!(a.closed(2.0), Some(Some("banned".into())));
            assert_eq!(Client::login_as(&relay, None, &k, "banme", &[]).err().as_deref(), Some("banned"));
            // A whole key works before it has ever been seen, and registers nothing.
            ok(&dir, &["ban", &pubhex(&unseen)]);
            assert_eq!(Client::login_as(&relay, None, &unseen, "u", &[]).err().as_deref(), Some("banned"));
            let bans = ok(&dir, &["bans"]);
            assert!(bans.contains(&pubhex(&k)) && bans.contains("banme") && bans.contains("keys (2)"), "{bans}");
        }
        let relay = start_in(&dir, Limits::default());
        assert_eq!(Client::login_as(&relay, None, &k, "banme", &[]).err().as_deref(), Some("banned"), "still banned");
        ok(&dir, &["unban", &pubhex(&k)[..10]]);
        assert!(Client::login_as(&relay, None, &k, "banme", &[]).is_ok());
        assert_eq!(Client::login_as(&relay, None, &unseen, "u", &[]).err().as_deref(), Some("banned"), "the other stays");
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_address_ban_refuses_there_and_never_the_proxy() {
        let dir = data_dir("banip");
        {
            let relay = start_in(&dir, Limits::default());
            let (mut a, _) = Client::login_as(&relay, Some("203.0.113.7"), &key(), "a", &[]).unwrap();
            let mut pre = Client::raw_as(&relay, Some("203.0.113.7"));
            let (mut other, _) = Client::login_as(&relay, Some("203.0.113.8"), &key(), "b", &[]).unwrap();
            ok(&dir, &["ban-ip", "203.0.113.7"]);
            assert_eq!(a.closed(2.0), Some(Some("banned".into())));
            assert_eq!(pre.closed(2.0), Some(Some("banned".into())), "unauthenticated too");
            assert_eq!(Client::try_raw(&relay, Some("203.0.113.7"), None).err(), Some(403));
            assert!(other.closed(0.2).is_none(), "its neighbour is fine");
            for proxy in ["127.0.0.1", "::1", "192.168.0.52", "::ffff:192.168.0.52"] {
                assert!(refused(&dir, &["ban-ip", proxy]).contains("trusted proxy"), "{proxy}");
            }
            assert!(refused(&dir, &["ban-ip", "not-an-ip"]).contains("not an address"));
            ok(&dir, &["ban-ip", "2001:db8:1:2::9"]);
            assert_eq!(Client::try_raw(&relay, Some("2001:db8:1:2:ffff::1"), None).err(), Some(403), "the whole /64");
        }
        let relay = start_in(&dir, Limits::default());
        assert_eq!(Client::try_raw(&relay, Some("203.0.113.7"), None).err(), Some(403), "after a restart");
        assert!(ok(&dir, &["bans"]).contains("addresses (2)"));
        ok(&dir, &["unban", "203.0.113.7"]);
        assert!(Client::try_raw(&relay, Some("203.0.113.7"), None).is_ok());
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_corrupt_control_file_stops_the_start() {
        let dir = data_dir("corrupt");
        drop(start_in(&dir, Limits::default()));
        std::fs::write(dir.join("control.json"), "{\"banned_keys\": [").unwrap();
        let cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), Some(dir.clone()));
        let err = Relay::start(cfg).err().expect("must not start");
        assert!(err.to_string().contains("control.json"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registration_off_lets_existing_installs_in() {
        let dir = data_dir("reg");
        let known = key();
        {
            let relay = start_in(&dir, Limits::default());
            drop(Client::login(&relay, &known, "known", &[]));
            ok(&dir, &["registration", "off"]);
            assert_eq!(Client::login_as(&relay, None, &key(), "new", &[]).err().as_deref(), Some("registration_closed"));
            assert!(Client::login_as(&relay, None, &known, "known", &[]).is_ok());
        }
        let relay = start_in(&dir, Limits::default());
        assert!(ok(&dir, &["status"]).contains("registration off"));
        assert!(Client::login_as(&relay, None, &key(), "new", &[]).is_err(), "still off after a restart");
        ok(&dir, &["registration", "on"]);
        assert!(Client::login_as(&relay, None, &key(), "new", &[]).is_ok());
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn friendcode_off_stops_codes_for_everyone_friends_still_connect() {
        let dir = data_dir("code");
        let relay = start_in(&dir, Limits::default());
        let (ka, kb, kc) = (key(), key(), key());
        let (mut a, _) = Client::login(&relay, &ka, "aaaa", &[pubhex(&kb)]);
        let (mut b, wb) = Client::login(&relay, &kb, "bbbb", &[pubhex(&ka)]);
        let (mut c, _) = Client::login(&relay, &kc, "cccc", &[]);
        // Redeemed just before: the grant must not outlive the switch.
        c.send(&json!({"t": "redeem", "code": wb["code"]}));
        let granted = c.expect("redeemed", 2.0);
        ok(&dir, &["friendcode", "off"]);
        assert_eq!(a.expect("code", 2.0)["code"], "----", "online installs are told");
        c.send(&json!({"t": "open", "req": 1, "to": granted["key"]}));
        assert_eq!(c.expect("open_failed", 2.0)["reason"], "not_friends");
        c.send(&json!({"t": "redeem", "code": wb["code"]}));
        assert_eq!(c.expect("redeem_failed", 2.0)["reason"], "disabled");
        a.send(&json!({"t": "new_code"}));
        assert_eq!(a.expect("code", 2.0)["code"], "----");
        let (_d, wd) = Client::login(&relay, &key(), "dddd", &[]);
        assert_eq!(wd["code"], "----", "welcome carries no code");
        // Friends are not affected.
        a.send(&json!({"t": "open", "req": 2, "to": pubhex(&kb)}));
        let ch = a.expect("opened", 2.0)["ch"].as_u64().unwrap() as u32;
        a.send_bin(ch, b"hi");
        assert_eq!(b.recv_bin(2.0), Some((ch, b"hi".to_vec())));
        // Back on: online installs get their code again.
        ok(&dir, &["friendcode", "on"]);
        let code = c.expect("code", 2.0)["code"].clone();
        assert_ne!(code, "----");
        let code_b = b.expect("code", 2.0)["code"].clone();
        assert_eq!(code_b, wb["code"]);
        c.send(&json!({"t": "redeem", "code": code_b}));
        c.expect("redeemed", 2.0);
        drop((a, b, c, _d, relay));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn relay_off_refuses_everyone_but_the_health_check() {
        let dir = data_dir("off");
        {
            let relay = start_in(&dir, Limits::default());
            let k = key();
            let (mut a, _) = Client::login(&relay, &k, "aaaa", &[]);
            let mut pre = Client::raw(&relay);
            ok(&dir, &["relay", "off"]);
            assert_eq!(a.closed(2.0), Some(Some("relay_off".into())));
            assert_eq!(pre.closed(2.0), Some(Some("relay_off".into())));
            assert_eq!(Client::try_raw(&relay, None, None).err(), Some(503));
            assert!(http_get(&relay, "/healthz").starts_with("HTTP/1.1 200"));
            wait_until("everyone gone", || relay.online_count() == 0);
        }
        let relay = start_in(&dir, Limits::default());
        assert_eq!(Client::try_raw(&relay, None, None).err(), Some(503), "still off after a restart");
        assert!(http_get(&relay, "/healthz").starts_with("HTTP/1.1 200"));
        ok(&dir, &["relay", "on"]);
        assert!(Client::login_as(&relay, None, &key(), "a", &[]).is_ok());
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn purge_forgets_an_install_and_its_code() {
        let dir = data_dir("purge");
        let relay = start_in(&dir, Limits::default());
        let k = key();
        let (mut a, wa) = Client::login(&relay, &k, "aaaa", &[]);
        ok(&dir, &["purge", "aaaa"]);
        assert_eq!(a.closed(2.0), Some(Some("purged".into())));
        assert!(ok(&dir, &["status"]).contains("stored 0"));
        let (_a2, wa2) = Client::login(&relay, &k, "aaaa", &[]);
        assert_ne!(wa2["code"], wa["code"], "registered afresh");
        drop((_a2, relay));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn purging_groups_and_idle_installs_needs_yes() {
        let dir = data_dir("isl");
        let (ka, kb, kc) = (key(), key(), key());
        let now = screenpet_relay::store::unix_now();
        let install = |id: &str, friends: Vec<String>, last_seen: u64| json!({"id": id, "friends": friends, "code": format!("ABCD-{id}"), "code_at": now, "last_seen": last_seen});
        let file = json!({"installs": {
            pubhex(&ka): install("1111", vec![pubhex(&kb)], now),
            pubhex(&kb): install("2222", vec![pubhex(&ka)], now),
            pubhex(&kc): install("3333", vec![], now - 40 * 24 * 3600),
        }});
        std::fs::write(dir.join("relay.json"), file.to_string()).unwrap();
        let relay = start_in(&dir, Limits::default());
        assert!(ok(&dir, &["purge-islands", "2"]).contains("would purge 2"));
        assert!(ok(&dir, &["purge-idle", "30"]).contains("would purge 1"));
        assert!(ok(&dir, &["status"]).contains("stored 3"));
        assert!(refused(&dir, &["purge-islands", "1"]).contains("at least 2"));
        // An online member protects its group.
        let online = Client::login(&relay, &ka, "1111", &[pubhex(&kb)]);
        assert!(ok(&dir, &["purge-islands", "2", "--yes"]).contains("purged 0"));
        drop(online);
        wait_until("offline", || relay.online_count() == 0);
        assert!(ok(&dir, &["purge-idle", "30", "--yes"]).contains("purged 1"));
        assert!(ok(&dir, &["purge-islands", "2", "--yes"]).contains("purged 2"));
        assert!(ok(&dir, &["status"]).contains("stored 0"));
        drop(relay);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_command_line_reaches_the_running_binary() {
        let dir = data_dir("cli");
        let bin = Bin::start(&dir);
        let cli = |words: &[&str]| {
            let out = std::process::Command::new(env!("CARGO_BIN_EXE_screenpet-relay")).args(["admin", "--data"]).arg(&dir).args(words).output().unwrap();
            (out.status.code(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
        };
        let (code, stdout, _) = cli(&["status"]);
        assert_eq!(code, Some(0));
        assert!(stdout.contains("relay on"), "{stdout}");
        let (code, _, stderr) = cli(&["kick", "nobody-here"]);
        assert_eq!(code, Some(1));
        assert!(stderr.contains("no install"), "{stderr}");
        drop(bin);
        let (code, _, stderr) = cli(&["status"]);
        assert_eq!(code, Some(2));
        assert!(stderr.contains("cannot reach"), "{stderr}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// ☀️ A relay with a fake weather service (no network): Yongin snows, Busan
/// is clear, Jeju's service is down; the fetched places are recorded.
#[test]
fn weather_comes_after_the_welcome_and_follows_the_place_the_app_wants() {
    use screenpet_relay::weather::{Fetch, Place};
    use std::sync::{Arc, Mutex};
    let asked: Arc<Mutex<Vec<&'static str>>> = Arc::default();
    let log = asked.clone();
    let fetch: Fetch = Arc::new(move |place: &Place| {
        log.lock().unwrap().push(place.id);
        match place.id {
            "jeju" => Err("down".into()),
            "yongin" => Ok(75),
            _ => Ok(0),
        }
    });
    let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), None);
    cfg.weather_fetch = Some(fetch);
    let relay = Relay::start(cfg).unwrap();
    let (mut a, welcome) = Client::login(&relay, &key(), "a", &[]);
    assert_eq!(welcome["t"], "welcome");
    assert!(a.challenge["v"].as_u64() >= Some(2), "apps send weather_want only to v2 relays");
    let w = a.next_json(5.0).expect("weather for Yongin");
    assert_eq!((w["t"].as_str(), w["place"].as_str(), w["code"].as_u64()), (Some("weather"), Some("yongin"), Some(75)), "{w}");
    assert!(w["at"].as_u64().unwrap() > 1_700_000_000);
    a.send(&json!({"t": "weather_want", "place": "busan"}));
    let w = a.next_json(5.0).expect("weather for Busan");
    assert_eq!((w["place"].as_str(), w["code"].as_u64()), (Some("busan"), Some(0)), "{w}");
    // A second app on Yongin gets the known reading without a new fetch
    let (mut b, _) = Client::login(&relay, &key(), "b", &[]);
    assert_eq!(b.next_json(2.0).expect("known reading")["code"], 75);
    assert_eq!(*asked.lock().unwrap(), vec!["yongin", "busan"], "once each");
    // Unknown places are ignored (no fetch), and a weather service that is down hurts nobody
    a.send(&json!({"t": "weather_want", "place": "tokyo"}));
    a.send(&json!({"t": "weather_want", "place": "jeju"}));
    let deadline = Instant::now() + Duration::from_secs(5);
    while asked.lock().unwrap().len() < 3 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(*asked.lock().unwrap(), vec!["yongin", "busan", "jeju"]);
    a.send(&json!({"t": "ping"}));
    assert_eq!(a.next_json(2.0).expect("still connected")["t"], "pong");
    // Coordinates are not a place: the message does not even parse, so the relay closes like for any bad message
    let (mut c, _) = Client::login(&relay, &key(), "c", &[]);
    let _ = c.next_json(2.0);
    c.send(&json!({"t": "weather_want", "lat": 37.2, "lon": 127.2}));
    assert_eq!(c.next_json(2.0).map(|e| e["reason"].clone()), Some(json!("bad_message")));
}

/// 📮 The public board over the wire: a v3 relay, anyone can page it, too
/// fast is ignored (not disconnected), and a non-reader's `board_put` is ignored.
#[test]
fn anyone_can_page_the_board_on_a_v3_relay() {
    let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), None);
    cfg.weather_fetch = Some(std::sync::Arc::new(|_: &screenpet_relay::weather::Place| Err("off".into())));
    let relay = Relay::start(cfg).unwrap();
    let (mut a, _) = Client::login(&relay, &key(), "a", &[]);
    assert_eq!(a.challenge["v"], 3, "apps send board messages only to v3 relays");
    a.send(&json!({"t": "board_put", "items": [{"id": 1, "name": "김철수", "line": "x"}]}));
    a.send(&json!({"t": "board_list", "page": 1}));
    let p = a.next_json(5.0).expect("a page");
    assert_eq!(p, json!({"t": "board_page", "items": [], "page": 1, "pages": 1, "total": 0}));
    a.send(&json!({"t": "board_list", "page": 2}));
    a.send(&json!({"t": "ping"}));
    assert_eq!(a.next_json(2.0).expect("still connected")["t"], "pong", "a page asked too soon gets nothing");
    std::thread::sleep(Duration::from_millis(550));
    a.send(&json!({"t": "board_list"}));
    assert_eq!(a.next_json(5.0).expect("a page")["t"], "board_page");
}
