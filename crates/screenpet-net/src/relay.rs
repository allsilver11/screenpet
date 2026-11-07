//! Relay transport: friends outside the LAN, through `screenpet-relay`
//! (spec phase 3 §1b).
//!
//! One WebSocket (`wss://` with rustls, or `ws://` for tests) to the relay
//! carries every relay link. The relay is an untrusted pipe: it authenticates
//! this install by its Noise static key (challenge + X25519-derived MAC) so
//! nobody else can use our mailbox, routes by static public key, and joins
//! two installs with a *channel* only if both list each other as friends (or
//! once, after one redeemed the other's friend code). Each channel is a
//! `FrameLink`/`FrameReader` pair handed to `peers::run_session`, exactly like
//! a LAN TCP link, so the Noise XX handshake, friend pinning and SAS pairing
//! all run end to end and the relay sees only ciphertext.
//!
//! Reconnects to the relay with exponential backoff (`Timing::relay_retry`
//! doubling up to 60 s). Friends the relay reports online are dialed through
//! it only while there is no friend link; a friend also announced on the LAN
//! gets a grace period first, and a relay link is later upgraded to LAN by
//! the smaller id (see `discovery`).

use crate::peers::{self, dial_failed, run_session, DialOutcome};
use crate::protocol::{MAX_FRAME, MAX_FRIENDS, MAX_ID};
use crate::transport::{Candidate, FrameLink, FrameReader, Recv};
use crate::noise;
use screenpet_crypto::{auth_mac, welcome_proof};
use crate::{Backoff, NetEvent, Shared, TICK};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use tungstenite::handshake::HandshakeError;
use tungstenite::{Error as WsError, Message, WebSocket};

/// Default relay (TLS terminates at the user's Nginx Proxy Manager).
pub const DEFAULT_RELAY_URL: &str = "wss://pet.danni.day/ws";
/// Static public key of the relay deployed at [`DEFAULT_RELAY_URL`]
/// (hex). Pinned: a relay answering with another key is refused. Use
/// [`parse_relay_key`] to get the bytes.
pub const DEFAULT_RELAY_KEY: &str = "298487e73c11af7c1030ad16689942587707baa158e084aad6c388fedea8de30";

/// 32-byte relay key from 64 hex digits.
pub fn parse_relay_key(hex: &str) -> Option<[u8; 32]> {
    noise::hex32(hex.trim())
}
/// `peer_addr` prefix of relay links (followed by the hex key the relay named).
pub(crate) const ADDR_PREFIX: &str = "relay:";
/// Longest wait between reconnect attempts.
pub(crate) const MAX_RETRY: Duration = Duration::from_secs(60);
/// Read poll while connected: bounds the latency of queued sends.
const POLL: Duration = Duration::from_millis(20);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Read timeout while connecting: how often the handshake checks for shutdown.
const HANDSHAKE_POLL: Duration = Duration::from_millis(200);
const PING_EVERY: Duration = Duration::from_secs(20);
/// Nothing from the relay this long (it answers pings) → reconnect.
const DEAD_AFTER: Duration = Duration::from_secs(60);
/// An `open` or `redeem` unanswered this long fails.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Housekeeping period (friend list sync, auto-dial, timeouts).
const HOUSEKEEP: Duration = Duration::from_millis(200);
const OUTBOX: usize = 1024;
/// Frames queued per channel before it counts as stuck.
const CHANNEL_QUEUE: usize = 256;
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Debug, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum ClientMsg<'a> {
    Auth { id: &'a str, key: String, mac: String, friends: Vec<String>, cnonce: String },
    Friends { keys: Vec<String> },
    Redeem { code: &'a str },
    Open { req: u32, to: String },
    Close { ch: u32 },
    NewCode,
    Ping,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum ServerMsg {
    Challenge { relay: String, nonce: String },
    Welcome {
        code: String,
        #[serde(default)]
        online: Vec<String>,
        #[serde(default)]
        proof: Option<String>,
    },
    Online { keys: Vec<String> },
    Presence { key: String, online: bool },
    Code { code: String },
    Redeemed { code: String, key: String, id: String },
    RedeemFailed { code: String, reason: String },
    Opened { req: u32, ch: u32 },
    OpenFailed { req: u32 },
    Incoming { ch: u32, from: String },
    Close { ch: u32 },
    Pong,
    Error { reason: String },
}

fn json(m: &ClientMsg) -> String {
    serde_json::to_string(m).expect("relay message serializes")
}

/// Normalize a friend code typed by a user to `ABCD-1234`: case-insensitive,
/// spaces and dashes ignored, O→0 and I/L→1 (Crockford base32). `None` if
/// it cannot be a code.
pub fn normalize_code(input: &str) -> Option<String> {
    let mut chars = String::new();
    for c in input.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        if !c.is_ascii() || !CROCKFORD.contains(&(c as u8)) || chars.len() >= 8 {
            return None;
        }
        chars.push(c);
    }
    (chars.len() == 8).then(|| format!("{}-{}", &chars[..4], &chars[4..]))
}

enum Out {
    Text(String),
    Binary(Vec<u8>),
    /// Send `close{ch}`, then end the local reader (after frames already queued).
    Close(u32),
}

struct Pending {
    peer_id: String,
    key: [u8; 32],
    since: Instant,
}

struct Redeem {
    code: String,
    pet: String,
    since: Instant,
}

#[derive(Default)]
struct Inner {
    connected: bool,
    /// Mutual friends the relay reports online.
    online: HashSet<[u8; 32]>,
    pending: HashMap<u32, Pending>,
    next_req: u32,
    redeem: Option<Redeem>,
    code: Option<String>,
    /// Relay redial backoff per friend id: (not before, current delay).
    retry: HashMap<String, (Instant, Duration)>,
    /// Friend id → since when it has been online on the relay without a link.
    wait: HashMap<String, Instant>,
    /// Friend list last sent to the relay.
    sent_friends: Vec<String>,
}

/// Relay connection state shared by the IO thread and the relay links.
pub(crate) struct RelayClient {
    url: String,
    /// Expected relay static key (`None` = accept whatever key it proves).
    pinned: Option<[u8; 32]>,
    /// Connection generation: links from an earlier connection never send
    /// on a later one (channel ids are per connection).
    generation: AtomicU64,
    outbox: Mutex<Option<(u64, SyncSender<Out>)>>,
    channels: Mutex<HashMap<u32, SyncSender<Vec<u8>>>>,
    inner: Mutex<Inner>,
}

impl RelayClient {
    pub fn new(url: String, pinned: Option<[u8; 32]>) -> Arc<RelayClient> {
        Arc::new(RelayClient {
            url,
            pinned,
            generation: AtomicU64::new(0),
            outbox: Mutex::new(None),
            channels: Mutex::new(HashMap::new()),
            inner: Mutex::new(Inner::default()),
        })
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn channels(&self) -> MutexGuard<'_, HashMap<u32, SyncSender<Vec<u8>>>> {
        self.channels.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Queue for the current connection (`gen` = the caller's generation, or
    /// `None` for "whatever is current"). `false` if not connected or full.
    fn send(&self, gen: Option<u64>, m: Out) -> bool {
        let out = self.outbox.lock().unwrap_or_else(|e| e.into_inner());
        match &*out {
            Some((g, tx)) if gen.is_none_or(|x| x == *g) => tx.try_send(m).is_ok(),
            _ => false,
        }
    }

    fn drop_channel(&self, ch: u32) {
        self.channels().remove(&ch);
    }

    pub fn connected(&self) -> bool {
        self.inner().connected
    }

    pub fn code(&self) -> Option<String> {
        self.inner().code.clone()
    }
}

// ---- links -----------------------------------------------------------------

struct RelayLink {
    client: Arc<RelayClient>,
    gen: u64,
    ch: u32,
    peer: String,
    done: AtomicBool,
}

impl FrameLink for RelayLink {
    fn send_frame(&self, frame: &[u8]) -> bool {
        if self.done.load(Ordering::SeqCst) || frame.is_empty() || frame.len() > MAX_FRAME {
            return false;
        }
        let mut msg = Vec::with_capacity(frame.len() + 4);
        msg.extend_from_slice(&self.ch.to_be_bytes());
        msg.extend_from_slice(frame);
        if self.client.send(Some(self.gen), Out::Binary(msg)) {
            true
        } else {
            self.close();
            false
        }
    }
    fn finish(&self) {
        // Frames already queued go first; the IO thread then closes the
        // channel and ends our reader.
        if !self.done.swap(true, Ordering::SeqCst) && !self.client.send(Some(self.gen), Out::Close(self.ch)) {
            self.client.drop_channel(self.ch);
        }
    }
    fn close(&self) {
        if !self.done.swap(true, Ordering::SeqCst) {
            self.client.send(Some(self.gen), Out::Close(self.ch));
        }
        self.client.drop_channel(self.ch);
    }
    fn peer_addr(&self) -> String {
        self.peer.clone()
    }
}

struct RelayReader(Receiver<Vec<u8>>);

impl FrameReader for RelayReader {
    fn recv(&mut self) -> Recv {
        match self.0.recv_timeout(TICK) {
            Ok(f) => Recv::Frame(f),
            Err(RecvTimeoutError::Timeout) => Recv::Idle,
            Err(RecvTimeoutError::Disconnected) => Recv::Closed,
        }
    }
}

fn new_channel(client: &Arc<RelayClient>, gen: u64, ch: u32, key: &[u8; 32]) -> (Arc<dyn FrameLink>, Box<dyn FrameReader>) {
    let (tx, rx) = sync_channel(CHANNEL_QUEUE);
    client.channels().insert(ch, tx);
    let link = RelayLink {
        client: client.clone(),
        gen,
        ch,
        peer: format!("{ADDR_PREFIX}{}", noise::to_hex(key)),
        done: AtomicBool::new(false),
    };
    (Arc::new(link), Box::new(RelayReader(rx)))
}

// ---- dialing ---------------------------------------------------------------

/// Open a relay channel to `key` for `peer_id` (already in `State::dialing`).
pub(crate) fn dial(shared: &Arc<Shared>, peer_id: String, key: [u8; 32]) {
    let Some(client) = shared.relay.clone() else {
        dial_failed(shared, &peer_id);
        return;
    };
    let req = {
        let mut inner = client.inner();
        if !inner.connected {
            drop(inner);
            dial_failed(shared, &peer_id);
            return;
        }
        inner.next_req = inner.next_req.wrapping_add(1);
        let req = inner.next_req;
        inner.pending.insert(req, Pending { peer_id: peer_id.clone(), key, since: Instant::now() });
        req
    };
    if !client.send(None, Out::Text(json(&ClientMsg::Open { req, to: noise::to_hex(&key) }))) {
        client.inner().pending.remove(&req);
        dial_failed(shared, &peer_id);
    }
}

/// Ask the relay who owns `code`; on success pairing starts with them.
pub(crate) fn redeem(shared: &Arc<Shared>, code: &str, pet: &str) {
    let fail = |reason: &str| shared.emit(NetEvent::CodeFailed { code: code.to_string(), reason: reason.to_string() });
    let Some(norm) = normalize_code(code) else { return fail("invalid") };
    if shared.state().friends.len() >= MAX_FRIENDS {
        return fail("full");
    }
    let Some(client) = shared.relay.clone() else { return fail("offline") };
    {
        let mut inner = client.inner();
        if !inner.connected {
            drop(inner);
            return fail("offline");
        }
        if inner.redeem.is_some() {
            drop(inner);
            return fail("busy");
        }
        inner.redeem = Some(Redeem { code: norm.clone(), pet: pet.to_string(), since: Instant::now() });
    }
    if !client.send(None, Out::Text(json(&ClientMsg::Redeem { code: &norm }))) {
        client.inner().redeem = None;
        fail("offline");
    }
}

/// Ask the relay for a fresh friend code (the old one stops working).
/// The new code arrives as `NetEvent::FriendCode`.
pub(crate) fn new_code(shared: &Shared) -> bool {
    shared.relay.as_ref().is_some_and(|c| c.send(None, Out::Text(json(&ClientMsg::NewCode))))
}

/// Bookkeeping after a relay dial ended.
fn dial_done(shared: &Shared, client: &RelayClient, peer_id: &str, outcome: DialOutcome) {
    let mut inner = client.inner();
    if outcome == DialOutcome::Proven {
        inner.retry.remove(peer_id);
    } else {
        let base = shared.timing.relay_retry;
        let delay = inner.retry.get(peer_id).map_or(base, |(_, d)| (*d * 2).min(MAX_RETRY));
        inner.retry.insert(peer_id.to_string(), (Instant::now() + delay, delay));
    }
}

/// Dial friends that are online on the relay and have no link.
fn auto_dial(shared: &Arc<Shared>, client: &Arc<RelayClient>) {
    let now = Instant::now();
    let grace = shared.timing.announce_interval * 3;
    let lan_grace = shared.timing.announce_interval * 4;
    let mut to_dial = Vec::new();
    {
        let mut st = shared.state();
        let mut inner = client.inner();
        let friends = st.friends.clone();
        inner.wait.retain(|id, _| friends.iter().any(|f| &f.id == id));
        inner.retry.retain(|id, _| friends.iter().any(|f| &f.id == id));
        for f in friends {
            if !inner.online.contains(&f.public_key) || st.friend_links.contains_key(&f.id) {
                inner.wait.remove(&f.id);
                continue;
            }
            if st.dialing.contains(&f.id)
                || st.standby_blocks(&f.id)
                || st.backed_off(&Backoff::Id(f.id.clone()), now)
                || inner.retry.get(&f.id).is_some_and(|(until, _)| now < *until)
            {
                continue;
            }
            let since = *inner.wait.entry(f.id.clone()).or_insert(now);
            // The smaller id dials first; LAN gets a head start if announced.
            let mut delay = Duration::ZERO;
            if shared.id.as_str() > f.id.as_str() {
                delay += grace;
            }
            if st.seen.contains_key(&f.id) {
                delay += lan_grace;
            }
            if now.duration_since(since) >= delay {
                st.dialing.insert(f.id.clone());
                to_dial.push((f.id, f.public_key));
            }
        }
    }
    for (id, key) in to_dial {
        dial(shared, id, key);
    }
}

// ---- connection --------------------------------------------------------------

type Ws = WebSocket<Polled<Stream>>;

enum Stream {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Stream {
    fn tcp(&self) -> &TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Stream::Plain(s) => s.write(buf),
            Stream::Tls(s) => s.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Stream::Plain(s) => s.flush(),
            Stream::Tls(s) => s.flush(),
        }
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let cfg = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .expect("ring provider supports TLS 1.2/1.3")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(cfg)
        })
        .clone()
}

/// A socket whose read and write timeouts are only polls. Windows reports an
/// expired timeout as `TimedOut`, Unix as `WouldBlock`; tungstenite's handshake
/// resumes only on `WouldBlock`, so this makes them the same everywhere.
struct Polled<S>(S);

fn polled<T>(r: io::Result<T>) -> io::Result<T> {
    r.map_err(|e| if e.kind() == ErrorKind::TimedOut { io::Error::new(ErrorKind::WouldBlock, e) } else { e })
}

impl<S: Read> Read for Polled<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        polled(self.0.read(buf))
    }
}

impl<S: Write> Write for Polled<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        polled(self.0.write(buf))
    }
    fn flush(&mut self) -> io::Result<()> {
        polled(self.0.flush())
    }
}

/// `(tls, host, port)` of a `ws://` / `wss://` URL.
fn parse_url(url: &str) -> Option<(bool, String, u16)> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else {
        (false, url.strip_prefix("ws://")?)
    };
    let authority = rest.split('/').next()?;
    let default = if tls { 443 } else { 80 };
    let (host, port) = if let Some(r) = authority.strip_prefix('[') {
        let (h, after) = r.split_once(']')?;
        match after.strip_prefix(':') {
            Some(p) => (h, p.parse().ok()?),
            None if after.is_empty() => (h, default),
            None => return None,
        }
    } else {
        match authority.split_once(':') {
            Some((h, p)) => (h, p.parse().ok()?),
            None => (authority, default),
        }
    };
    (!host.is_empty()).then(|| (tls, host.to_string(), port))
}

/// Run a call that can block for long (name lookup: tens of seconds on a bad
/// network, and it cannot be interrupted) on its own thread, so `Net::drop`
/// is not held up. Given up on at shutdown, that thread ends by itself.
fn unless_stopped<T: Send + 'static>(stop: &AtomicBool, f: impl FnOnce() -> io::Result<T> + Send + 'static) -> io::Result<T> {
    let (tx, rx) = sync_channel(1);
    std::thread::Builder::new().name("screenpet-relay-connect".into()).spawn(move || {
        let _ = tx.send(f());
    })?;
    loop {
        match rx.recv_timeout(TICK) {
            Ok(r) => return r,
            Err(RecvTimeoutError::Disconnected) => return Err(io::Error::other("relay connect thread died")),
            Err(RecvTimeoutError::Timeout) if stop.load(Ordering::SeqCst) => {
                return Err(io::Error::new(ErrorKind::Interrupted, "shutting down"));
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Connect, TLS and WebSocket handshakes; gives up at shutdown or after
/// `HANDSHAKE_TIMEOUT` (reads poll so `Net::drop` is not held up).
fn connect(url: &str, stop: &AtomicBool) -> io::Result<Ws> {
    let bad = |m: &str| io::Error::new(ErrorKind::InvalidInput, m.to_string());
    let (tls, host, port) = parse_url(url).ok_or_else(|| bad("bad relay url"))?;
    let target = (host.clone(), port);
    let tcp = unless_stopped(stop, move || {
        let mut last = io::Error::new(ErrorKind::InvalidInput, "relay host not found");
        for addr in (target.0.as_str(), target.1).to_socket_addrs()? {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                Ok(s) => return Ok(s),
                Err(e) => last = e,
            }
        }
        Err(last)
    })?;
    tcp.set_nodelay(true)?;
    tcp.set_read_timeout(Some(HANDSHAKE_POLL))?;
    tcp.set_write_timeout(Some(Duration::from_secs(5)))?;
    let stream = if tls {
        let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| bad("bad relay host"))?;
        let conn = rustls::ClientConnection::new(tls_config(), name).map_err(io::Error::other)?;
        Stream::Tls(Box::new(rustls::StreamOwned::new(conn, tcp)))
    } else {
        Stream::Plain(tcp)
    };
    ws_handshake(url, Polled(stream), stop, HANDSHAKE_TIMEOUT)
}

/// The WebSocket upgrade over a polling socket: keeps going while reads time out.
fn ws_handshake<S: Read + Write>(url: &str, stream: S, stop: &AtomicBool, limit: Duration) -> io::Result<WebSocket<S>> {
    let cfg = tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(MAX_FRAME + 4))
        .max_frame_size(Some(MAX_FRAME + 4));
    let deadline = Instant::now() + limit;
    let mut attempt = tungstenite::client::client_with_config(url, stream, Some(cfg));
    loop {
        match attempt {
            Ok((ws, _)) => return Ok(ws),
            Err(HandshakeError::Interrupted(mid)) => {
                if stop.load(Ordering::SeqCst) || Instant::now() > deadline {
                    return Err(io::Error::new(ErrorKind::TimedOut, "relay handshake timed out"));
                }
                attempt = mid.handshake();
            }
            Err(HandshakeError::Failure(e)) => return Err(io::Error::other(e.to_string())),
        }
    }
}

fn would_block(e: &WsError) -> bool {
    matches!(e, WsError::Io(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut | ErrorKind::Interrupted))
}

/// Next control message before `deadline` or shutdown (handshake phase).
fn read_server(ws: &mut Ws, deadline: Instant, stop: &AtomicBool) -> Option<ServerMsg> {
    while Instant::now() < deadline && !stop.load(Ordering::SeqCst) {
        match ws.read() {
            Ok(Message::Text(t)) => return serde_json::from_str(t.as_str()).ok(),
            Ok(Message::Close(_)) => return None,
            Ok(_) => {}
            Err(e) if would_block(&e) => {}
            Err(_) => return None,
        }
    }
    None
}

fn friend_keys(shared: &Shared) -> Vec<String> {
    shared.state().friends.iter().take(MAX_FRIENDS).map(|f| noise::to_hex(&f.public_key)).collect()
}

/// The keys among `keys` that are friends'. The relay is untrusted: nothing it
/// names beyond them is worth remembering.
fn friends_among(shared: &Shared, keys: &[String]) -> HashSet<[u8; 32]> {
    let st = shared.state();
    keys.iter().filter_map(|k| noise::hex32(k)).filter(|k| st.friends.iter().any(|f| f.public_key == *k)).collect()
}

/// The relay IO thread: connect, serve, reconnect with backoff until shutdown.
pub(crate) fn run(shared: Arc<Shared>, client: Arc<RelayClient>) {
    let mut delay = shared.timing.relay_retry;
    let mut logged = false;
    while !shared.stopping() {
        let served = match connect(&client.url, &shared.stop) {
            Ok(ws) => serve(&shared, &client, ws),
            Err(e) => {
                if !logged {
                    eprintln!("screenpet relay {}: {e}", client.url);
                    logged = true;
                }
                false
            }
        };
        disconnected(&shared, &client);
        if served {
            delay = shared.timing.relay_retry;
            logged = false;
        }
        let until = Instant::now() + delay;
        while !shared.stopping() && Instant::now() < until {
            std::thread::sleep(TICK);
        }
        if !served {
            delay = (delay * 2).min(MAX_RETRY);
        }
    }
}

/// Forget everything tied to the ended connection.
fn disconnected(shared: &Shared, client: &RelayClient) {
    *client.outbox.lock().unwrap_or_else(|e| e.into_inner()) = None;
    client.channels().clear(); // readers end → sessions close
    let (was, pending, redeem) = {
        let mut inner = client.inner();
        let was = std::mem::take(&mut inner.connected);
        inner.online.clear();
        inner.wait.clear();
        inner.sent_friends.clear();
        let pending: Vec<String> = inner.pending.drain().map(|(_, p)| p.peer_id).collect();
        (was, pending, inner.redeem.take())
    };
    for id in pending {
        dial_failed(shared, &id);
    }
    if let Some(r) = redeem {
        shared.emit(NetEvent::CodeFailed { code: r.code, reason: "offline".into() });
    }
    if was {
        shared.emit(NetEvent::RelayStatus { connected: false });
    }
}

/// Authenticate and serve one connection. `true` if it got past `welcome`.
fn serve(shared: &Arc<Shared>, client: &Arc<RelayClient>, mut ws: Ws) -> bool {
    let deadline = Instant::now() + HANDSHAKE_TIMEOUT;
    let Some(ServerMsg::Challenge { relay, nonce }) = read_server(&mut ws, deadline, &shared.stop) else { return false };
    let (Some(relay_pub), Some(nonce)) = (noise::hex32(&relay), noise::hex32(&nonce)) else { return false };
    if client.pinned.is_some_and(|k| k != relay_pub) {
        eprintln!("screenpet relay {}: unexpected relay key {relay}; refusing", client.url);
        return false;
    }
    let own_pub = shared.public_key;
    let shared_secret = curve25519_dalek::MontgomeryPoint(relay_pub).mul_clamped(shared.static_key).to_bytes();
    if shared_secret.iter().all(|b| *b == 0) {
        return false;
    }
    let mac = auth_mac(&shared_secret, &nonce, &relay_pub, &own_pub, &shared.id);
    let friends = friend_keys(shared);
    let cnonce = noise::random32();
    let auth = ClientMsg::Auth {
        id: &shared.id,
        key: noise::to_hex(&own_pub),
        mac: noise::to_hex(&mac),
        friends: friends.clone(),
        cnonce: noise::to_hex(&cnonce),
    };
    if ws.send(Message::text(json(&auth))).is_err() {
        return false;
    }
    let (code, online) = match read_server(&mut ws, deadline, &shared.stop) {
        Some(ServerMsg::Welcome { code, online, proof }) => {
            // Only the holder of the relay key can compute this.
            let expected = welcome_proof(&shared_secret, &cnonce, &nonce, &own_pub);
            if proof.as_deref().and_then(noise::hex32) != Some(expected) {
                eprintln!("screenpet relay {}: relay did not prove its key; refusing", client.url);
                return false;
            }
            (code, online)
        }
        Some(ServerMsg::Error { reason }) => {
            eprintln!("screenpet relay refused us: {reason}");
            return false;
        }
        _ => return false,
    };
    if ws.get_ref().0.tcp().set_read_timeout(Some(POLL)).is_err() {
        return false;
    }
    let online = friends_among(shared, &online);
    let gen = client.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let (tx, rx) = sync_channel(OUTBOX);
    *client.outbox.lock().unwrap_or_else(|e| e.into_inner()) = Some((gen, tx));
    {
        let mut inner = client.inner();
        inner.connected = true;
        inner.online = online;
        inner.code = Some(code.clone());
        inner.sent_friends = friends;
    }
    shared.emit(NetEvent::RelayStatus { connected: true });
    shared.emit(NetEvent::FriendCode { code });

    let mut last_recv = Instant::now();
    let mut last_ping = Instant::now();
    let mut last_housekeep = Instant::now() - HOUSEKEEP;
    'conn: while !shared.stopping() {
        loop {
            let out = match rx.try_recv() {
                Ok(o) => o,
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            };
            let res = match out {
                Out::Text(t) => ws.write(Message::text(t)),
                Out::Binary(b) => ws.write(Message::binary(b)),
                Out::Close(ch) => {
                    client.drop_channel(ch);
                    ws.write(Message::text(json(&ClientMsg::Close { ch })))
                }
            };
            if res.is_err() {
                break 'conn;
            }
        }
        match ws.flush() {
            Ok(()) => {}
            Err(e) if would_block(&e) => {}
            Err(_) => break,
        }
        match ws.read() {
            Ok(Message::Text(t)) => {
                last_recv = Instant::now();
                match serde_json::from_str::<ServerMsg>(t.as_str()) {
                    Ok(ServerMsg::Error { reason }) => {
                        eprintln!("screenpet relay closed the connection: {reason}");
                        break;
                    }
                    Ok(m) => on_message(shared, client, gen, m),
                    Err(_) => {} // newer relay message: ignore
                }
            }
            Ok(Message::Binary(b)) => {
                last_recv = Instant::now();
                if b.len() > 4 {
                    let ch = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
                    let tx = client.channels().get(&ch).cloned();
                    if let Some(tx) = tx {
                        if tx.try_send(b[4..].to_vec()).is_err() {
                            // Reader not keeping up: drop the link.
                            client.drop_channel(ch);
                            let _ = ws.write(Message::text(json(&ClientMsg::Close { ch })));
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => last_recv = Instant::now(),
            Err(e) if would_block(&e) => {}
            Err(_) => break,
        }
        let now = Instant::now();
        if now.duration_since(last_recv) > DEAD_AFTER {
            break;
        }
        if now.duration_since(last_ping) >= PING_EVERY {
            last_ping = now;
            if !client.send(Some(gen), Out::Text(json(&ClientMsg::Ping))) {
                break;
            }
        }
        if now.duration_since(last_housekeep) >= HOUSEKEEP {
            last_housekeep = now;
            housekeep(shared, client, gen);
        }
    }
    true
}

/// Tell the relay a changed friend list. Counts as sent only once it is
/// queued: otherwise the next housekeeping tries again.
fn sync_friends(shared: &Shared, client: &RelayClient, gen: u64) {
    let keys = friend_keys(shared);
    if client.inner().sent_friends != keys && client.send(Some(gen), Out::Text(json(&ClientMsg::Friends { keys: keys.clone() }))) {
        client.inner().sent_friends = keys;
    }
}

fn housekeep(shared: &Arc<Shared>, client: &Arc<RelayClient>, gen: u64) {
    sync_friends(shared, client, gen);
    let now = Instant::now();
    let (expired, redeem) = {
        let mut inner = client.inner();
        let expired: Vec<u32> =
            inner.pending.iter().filter(|(_, p)| now.duration_since(p.since) > REQUEST_TIMEOUT).map(|(r, _)| *r).collect();
        let expired: Vec<String> = expired.into_iter().filter_map(|r| inner.pending.remove(&r)).map(|p| p.peer_id).collect();
        let redeem = if inner.redeem.as_ref().is_some_and(|r| now.duration_since(r.since) > REQUEST_TIMEOUT) {
            inner.redeem.take()
        } else {
            None
        };
        (expired, redeem)
    };
    for id in expired {
        dial_failed(shared, &id);
        dial_done(shared, client, &id, DialOutcome::Failed);
    }
    if let Some(r) = redeem {
        shared.emit(NetEvent::CodeFailed { code: r.code, reason: "timeout".into() });
    }
    auto_dial(shared, client);
}

fn on_message(shared: &Arc<Shared>, client: &Arc<RelayClient>, gen: u64, m: ServerMsg) {
    match m {
        ServerMsg::Online { keys } => {
            let online = friends_among(shared, &keys);
            client.inner().online = online;
        }
        ServerMsg::Presence { key, online } => {
            if !online {
                if let Some(k) = noise::hex32(&key) {
                    client.inner().online.remove(&k);
                }
            } else {
                let friends = friends_among(shared, std::slice::from_ref(&key));
                client.inner().online.extend(friends);
            }
        }
        ServerMsg::Code { code } => {
            client.inner().code = Some(code.clone());
            shared.emit(NetEvent::FriendCode { code });
        }
        ServerMsg::Redeemed { code, key, id } => {
            let pet = {
                let mut inner = client.inner();
                match inner.redeem.take_if(|r| r.code == code) {
                    Some(r) => r.pet,
                    None => return, // not ours, or timed out
                }
            };
            let fail = |reason: &str| shared.emit(NetEvent::CodeFailed { code: code.clone(), reason: reason.into() });
            let Some(key) = noise::hex32(&key) else { return fail("unknown") };
            if id.is_empty() || id.len() > MAX_ID || id == shared.id {
                return fail("unknown");
            }
            let known = {
                let st = shared.state();
                st.is_friend(&id) && !st.introduced_only(&id)
            };
            if known {
                return fail("already_friend");
            }
            if peers::invite_at(shared, &id, &pet, Candidate::Relay(key)) {
                shared.emit(NetEvent::CodeRedeemed { code, id });
            } else {
                fail("busy");
            }
        }
        ServerMsg::RedeemFailed { code, reason } => {
            let mine = client.inner().redeem.take_if(|r| r.code == code).is_some();
            if mine {
                shared.emit(NetEvent::CodeFailed { code, reason });
            }
        }
        ServerMsg::Opened { req, ch } => {
            let Some(p) = client.inner().pending.remove(&req) else {
                client.send(Some(gen), Out::Text(json(&ClientMsg::Close { ch })));
                return;
            };
            let (link, reader) = new_channel(client, gen, ch, &p.key);
            let guard = shared.outbound_guard();
            let (s, c, id) = (shared.clone(), client.clone(), p.peer_id.clone());
            let spawned = shared.spawn(move || {
                let outcome = run_session(s.clone(), link, reader, Some(id.clone()), guard);
                dial_done(&s, &c, &id, outcome);
            });
            if !spawned {
                client.drop_channel(ch);
                client.send(Some(gen), Out::Text(json(&ClientMsg::Close { ch })));
                dial_failed(shared, &p.peer_id);
            }
        }
        ServerMsg::OpenFailed { req } => {
            let p = client.inner().pending.remove(&req);
            if let Some(p) = p {
                dial_failed(shared, &p.peer_id);
                dial_done(shared, client, &p.peer_id, DialOutcome::Failed);
            }
        }
        ServerMsg::Incoming { ch, from } => {
            let Some(key) = noise::hex32(&from) else {
                client.send(Some(gen), Out::Text(json(&ClientMsg::Close { ch })));
                return;
            };
            if !shared.allow_inbound(&format!("{ADDR_PREFIX}{from}"), Instant::now()) {
                client.send(Some(gen), Out::Text(json(&ClientMsg::Close { ch })));
                return;
            }
            let (link, reader) = new_channel(client, gen, ch, &key);
            let Some(guard) = shared.admit_inbound(link.clone()) else {
                link.close();
                return;
            };
            let s = shared.clone();
            let l = link.clone();
            if !shared.spawn(move || {
                run_session(s, l, reader, None, guard);
            }) {
                link.close();
            }
        }
        ServerMsg::Close { ch } => client.drop_channel(ch),
        ServerMsg::Challenge { .. } | ServerMsg::Welcome { .. } | ServerMsg::Pong | ServerMsg::Error { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relay on a slow line: every read first times out `timeouts` times, the
    /// way a Windows socket with a 200 ms read timeout does, then answers the upgrade.
    struct SlowRelay {
        timeouts: usize,
        request: Vec<u8>,
        response: Option<io::Cursor<Vec<u8>>>,
    }

    impl Read for SlowRelay {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.timeouts > 0 {
                self.timeouts -= 1;
                return Err(io::Error::new(ErrorKind::TimedOut, "read timed out"));
            }
            let request = String::from_utf8_lossy(&self.request).to_string();
            let response = self.response.get_or_insert_with(|| {
                let key = request.lines().find_map(|l| l.strip_prefix("Sec-WebSocket-Key: ").or_else(|| l.strip_prefix("sec-websocket-key: "))).expect("upgrade request");
                let accept = tungstenite::handshake::derive_accept_key(key.trim().as_bytes());
                io::Cursor::new(format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n").into_bytes())
            });
            response.read(buf)
        }
    }

    impl Write for SlowRelay {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.request.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_read_timeout_during_the_upgrade_is_only_a_poll() {
        let stop = AtomicBool::new(false);
        let slow = SlowRelay { timeouts: 3, request: Vec::new(), response: None };
        let done = ws_handshake("ws://relay.test/ws", Polled(slow), &stop, Duration::from_secs(2));
        assert!(done.is_ok(), "{:?}", done.err());
    }

    #[test]
    fn a_stopped_or_silent_upgrade_still_gives_up() {
        let stop = AtomicBool::new(false);
        let silent = SlowRelay { timeouts: usize::MAX, request: Vec::new(), response: None };
        let e = ws_handshake("ws://relay.test/ws", Polled(silent), &stop, Duration::from_millis(30)).err().expect("gives up");
        assert_eq!(e.kind(), ErrorKind::TimedOut);
    }

    #[test]
    fn presence_of_anyone_but_a_friend_is_not_kept() {
        let friend_key = [7u8; 32];
        let net = crate::solo_net(vec![crate::Friend { id: "ff".into(), public_key: friend_key }]);
        let client = RelayClient::new("ws://relay.test/ws".into(), None);
        let say = |m| on_message(&net.shared, &client, 1, m);
        say(ServerMsg::Presence { key: noise::to_hex(&[9; 32]), online: true });
        assert!(client.inner().online.is_empty(), "a stranger is not tracked");
        say(ServerMsg::Presence { key: noise::to_hex(&friend_key), online: true });
        assert!(client.inner().online.contains(&friend_key));
        say(ServerMsg::Online { keys: vec![noise::to_hex(&[9; 32]), noise::to_hex(&friend_key)] });
        assert_eq!(client.inner().online.len(), 1, "same for the full list");
        say(ServerMsg::Presence { key: noise::to_hex(&friend_key), online: false });
        assert!(client.inner().online.is_empty());
    }

    #[test]
    fn a_friend_list_that_could_not_be_queued_is_sent_again() {
        let net = crate::solo_net(vec![crate::Friend { id: "ff".into(), public_key: [7; 32] }]);
        let client = RelayClient::new("ws://relay.test/ws".into(), None);
        // 연결이 없어(또는 큐가 차서) 못 보냈다: 보낸 것으로 치지 않는다
        sync_friends(&net.shared, &client, 1);
        assert!(client.inner().sent_friends.is_empty());
        let (tx, rx) = sync_channel(4);
        *client.outbox.lock().unwrap() = Some((1, tx));
        sync_friends(&net.shared, &client, 1);
        assert!(matches!(rx.try_recv(), Ok(Out::Text(t)) if t.contains("friends")));
        sync_friends(&net.shared, &client, 1);
        assert!(rx.try_recv().is_err(), "unchanged: nothing more to send");
    }

    #[test]
    fn a_slow_name_lookup_does_not_hold_up_shutdown() {
        let stop = Arc::new(AtomicBool::new(false));
        let setter = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            setter.store(true, Ordering::SeqCst);
        });
        let t = Instant::now();
        let got = unless_stopped(&stop, || {
            std::thread::sleep(Duration::from_secs(5)); // getaddrinfo가 오래 걸리는 망
            Ok(1)
        });
        assert!(got.is_err());
        assert!(t.elapsed() < Duration::from_secs(1), "{:?}", t.elapsed());
        assert_eq!(unless_stopped(&AtomicBool::new(false), || Ok(2)).unwrap(), 2);
    }

    /// 어떤 루트에도 없는 자체 서명 인증서 (localhost, 2126년까지). 시험 전용 키라 비밀이 아니다.
    const TEST_CERT: &str = "-----BEGIN CERTIFICATE-----\n\
MIIBsTCCAVagAwIBAgIUHLZ0vPs7S8i3qXDbZxx+ONL2+DAwCgYIKoZIzj0EAwIw\n\
IzEhMB8GA1UEAwwYc2NyZWVucGV0IHVudHJ1c3RlZCB0ZXN0MCAXDTI2MTEwNDE5\n\
Mzg0MVoYDzIxMjYxMDExMTkzODQxWjAjMSEwHwYDVQQDDBhzY3JlZW5wZXQgdW50\n\
cnVzdGVkIHRlc3QwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAAQ0sk9q8y54/vas\n\
zmxM8KjkMLgLIsCzRoiK+gDhY9cS6gsFC9oTEA3h5HGh+huoFghmkruTSR4bcG8k\n\
Pm0B4kv0o2YwZDAdBgNVHQ4EFgQUkUMkrMSZVgg9WIXkqRle4nl6Ng4wHwYDVR0j\n\
BBgwFoAUkUMkrMSZVgg9WIXkqRle4nl6Ng4wFAYDVR0RBA0wC4IJbG9jYWxob3N0\n\
MAwGA1UdEwEB/wQCMAAwCgYIKoZIzj0EAwIDSQAwRgIhAJei3ERvkGlO+qDHBCwf\n\
aT/r8tyoQc7MFx0Cla6ApmvAAiEAnfYyhIckgowyTyDTeKvoOE0tK0lSzNVsa6hr\n\
4mLyLUc=\n\
-----END CERTIFICATE-----";
    const TEST_KEY: &str = "-----BEGIN PRIVATE KEY-----\n\
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgo4ifw4MDbd3rkx7Q\n\
q7r589NxUGjSbyak2ry2O74rNYehRANCAAQ0sk9q8y54/vaszmxM8KjkMLgLIsCz\n\
RoiK+gDhY9cS6gsFC9oTEA3h5HGh+huoFghmkruTSR4bcG8kPm0B4kv0\n\
-----END PRIVATE KEY-----";

    fn test_cert() -> rustls::pki_types::CertificateDer<'static> {
        use rustls::pki_types::pem::PemObject;
        rustls::pki_types::CertificateDer::from_pem_slice(TEST_CERT.as_bytes()).unwrap()
    }

    /// 앱과 같은 암호 제공자로 TLS 서버를 한 번 받는다: (포트, 핸드셰이크 결과)
    fn tls_server(versions: &[&'static rustls::SupportedProtocolVersion]) -> (u16, std::thread::JoinHandle<io::Result<()>>) {
        use rustls::pki_types::pem::PemObject;
        let key = rustls::pki_types::PrivateKeyDer::from_pem_slice(TEST_KEY.as_bytes()).unwrap();
        let cfg = rustls::ServerConfig::builder_with_provider(tls_config().crypto_provider().clone())
            .with_protocol_versions(versions)
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![test_cert()], key)
            .unwrap();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut tcp, _) = listener.accept()?;
            tcp.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut conn = rustls::ServerConnection::new(Arc::new(cfg)).map_err(io::Error::other)?;
            while conn.is_handshaking() {
                conn.complete_io(&mut tcp)?;
            }
            Ok(())
        });
        (port, server)
    }

    #[test]
    fn a_certificate_outside_the_webpki_roots_is_refused() {
        let (port, server) = tls_server(rustls::ALL_VERSIONS);
        let url = format!("wss://localhost:{port}/ws");
        let err = connect(&url, &AtomicBool::new(false)).err().expect("an untrusted relay must not connect");
        // 다른 이유(연결 실패, 시간 초과)로 실패한 것을 통과로 치지 않는다
        assert!(err.to_string().contains("UnknownIssuer"), "{err}");
        let _ = server.join().unwrap();
    }

    /// 위 거부가 인증서 탓임을 보이는 대조군이자, 망 없이 도는 TLS 1.2/1.3 왕복이다.
    #[test]
    fn the_provider_completes_tls_12_and_13_with_a_trusted_certificate() {
        for (version, expected) in [
            (&rustls::version::TLS13, rustls::ProtocolVersion::TLSv1_3),
            (&rustls::version::TLS12, rustls::ProtocolVersion::TLSv1_2),
        ] {
            // 서버가 한 판만 받으니 클라이언트는 앱처럼 둘 다 내놓아야 한다
            let (port, server) = tls_server(&[version]);
            let mut roots = rustls::RootCertStore::empty();
            roots.add(test_cert()).unwrap();
            let cfg = rustls::ClientConfig::builder_with_provider(tls_config().crypto_provider().clone())
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
            let mut conn = rustls::ClientConnection::new(Arc::new(cfg), name).unwrap();
            let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            while conn.is_handshaking() {
                conn.complete_io(&mut tcp).unwrap();
            }
            assert_eq!(conn.protocol_version(), Some(expected));
            server.join().unwrap().unwrap();
        }
    }

    /// 실제 릴레이에 TLS와 WebSocket만 잇고 챌린지를 받으면 끊는다 (등록 없음, 키를 보내지 않음).
    /// `cargo test -p screenpet-net --lib live_relay_gives_a_challenge_over_tls -- --ignored`
    #[test]
    #[ignore]
    fn live_relay_gives_a_challenge_over_tls() {
        let stop = AtomicBool::new(false);
        let mut ws = connect(DEFAULT_RELAY_URL, &stop).expect("TLS and WebSocket handshake");
        if let Stream::Tls(tls) = &ws.get_ref().0 {
            println!("{:?} {:?}", tls.conn.protocol_version(), tls.conn.negotiated_cipher_suite().map(|s| s.suite()));
        } else {
            panic!("not TLS");
        }
        let challenge = read_server(&mut ws, Instant::now() + HANDSHAKE_TIMEOUT, &stop);
        let Some(ServerMsg::Challenge { relay, nonce }) = challenge else { panic!("no challenge") };
        assert_eq!(relay, DEFAULT_RELAY_KEY);
        assert!(noise::hex32(&nonce).is_some(), "{nonce}");
        let _ = ws.close(None);
        let _ = ws.flush();
    }

    #[test]
    fn urls() {
        assert_eq!(parse_url("wss://pet.danni.day/ws"), Some((true, "pet.danni.day".into(), 443)));
        assert_eq!(parse_url("ws://127.0.0.1:9411/ws"), Some((false, "127.0.0.1".into(), 9411)));
        assert_eq!(parse_url("ws://[::1]:9411/ws"), Some((false, "::1".into(), 9411)));
        assert_eq!(parse_url("https://x/ws"), None);
        assert_eq!(parse_url("ws:///ws"), None);
    }

    #[test]
    fn codes_normalize_like_the_relay() {
        assert_eq!(normalize_code("abcd-1234").as_deref(), Some("ABCD-1234"));
        assert_eq!(normalize_code("ab cd 1o2l").as_deref(), Some("ABCD-1021"));
        assert_eq!(normalize_code("ABCD-123"), None);
        assert_eq!(normalize_code("ABCD-12345"), None);
        assert_eq!(normalize_code("ABCU-1234"), None);
    }
}
