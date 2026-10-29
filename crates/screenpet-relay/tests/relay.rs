//! In-process relay tests: a real relay on a localhost port and hand-rolled
//! WebSocket clients speaking the relay protocol.

use screenpet_relay::proto::{self, HEADER, MAX_FRAME};
use screenpet_relay::{Limits, Relay, RelayConfig};
use serde_json::{json, Value};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};
use tungstenite::{Error as WsError, Message, WebSocket};

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
    fn raw(relay: &Relay) -> Client {
        let addr = relay.local_addr();
        let tcp = TcpStream::connect(addr).unwrap();
        let (mut ws, _) = tungstenite::client::client(format!("ws://{addr}/ws"), tcp).unwrap();
        ws.get_mut().set_read_timeout(Some(Duration::from_millis(20))).unwrap();
        let mut c = Client { ws, challenge: Value::Null };
        c.challenge = c.next_json(2.0).expect("challenge");
        assert_eq!(c.challenge["t"], "challenge");
        c
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
    fn login(relay: &Relay, priv_key: &[u8; 32], id: &str, friends: &[String]) -> (Client, Value) {
        let mut c = Client::raw(relay);
        let auth = c.auth_msg(priv_key, id, friends);
        c.send(&auth);
        let w = c.expect("welcome", 2.0);
        (c, w)
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

fn http_get(relay: &Relay, path: &str) -> String {
    let mut s = TcpStream::connect(relay.local_addr()).unwrap();
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
    // Offline target.
    a.send(&json!({"t": "open", "req": 2, "to": pubhex(&key())}));
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
fn redeems_per_target_are_capped() {
    let relay = start(Limits { redeems_per_target_per_hour: 2, ..Limits::default() });
    let kb = key();
    let (_b, wb) = Client::login(&relay, &kb, "bbbb", &[]);
    let mut outcomes = Vec::new();
    for i in 0..4 {
        let (mut x, _) = Client::login(&relay, &key(), &format!("x{i}"), &[]);
        x.send(&json!({"t": "redeem", "code": wb["code"]}));
        outcomes.push(x.wait("redeemed", 1.0).is_some());
    }
    assert_eq!(outcomes, vec![true, true, false, false]);
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

    let relay = start(Limits { max_installs: 1, ..Limits::default() });
    let _ = Client::login(&relay, &k1, "a", &[]);
    let mut c = Client::raw(&relay);
    let auth = c.auth_msg(&key(), "c", &[]);
    c.send(&auth);
    assert_eq!(c.closed(2.0), Some(Some("full".into())));
}
