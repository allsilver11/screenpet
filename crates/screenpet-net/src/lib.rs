//! ScreenPet LAN networking: discovery (UDP broadcast), SAS-confirmed pairing,
//! and persistent friend links that are encrypted and authenticated end to
//! end with the Noise protocol.
//!
//! Everything runs on plain std threads. The app talks to [`Net`] through its
//! methods (none of which block on the network) and receives [`NetEvent`]s on
//! an mpsc channel.
//!
//! Layering:
//! - `protocol`: JSON messages (`hello`, `invite`, `visit`, ...) and the
//!   discovery announcement.
//! - `transport`: the frame-pipe seam (`FrameLink`/`FrameReader`) and
//!   transport-neutral dialing (`Candidate`). `tcp` is the LAN transport; a
//!   relay transport can be added beside it.
//! - `noise`: XX handshake, static keys, emoji SAS.
//! - `secure`: encrypted, queued message sending after the handshake.
//! - `peers`: per-link session driver (identity check, pairing, friends,
//!   ping, timeouts) — transport-independent.
//! - `discovery`: LAN UDP announcements — only hints (candidate addresses).
//! - `relay`: the relay transport (`screenpet-relay` over WebSocket) for
//!   friends outside the LAN, friend codes, LAN-preferred link choice.

mod discovery;
mod noise;
mod peers;
mod protocol;
mod relay;
mod secure;
mod tcp;
mod transport;

pub use noise::SAS_EMOJI;
pub use protocol::{Side, Visit};
pub use relay::{normalize_code, DEFAULT_RELAY_URL};

use protocol::{Control, Message};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use transport::Candidate;

/// Spec default UDP discovery port.
pub const DEFAULT_DISCOVERY_PORT: u16 = 47321;

/// How often worker threads wake up to check timers and the stop flag.
pub(crate) const TICK: Duration = Duration::from_millis(50);
/// Most concurrent inbound links not (yet) authenticated as a friend
/// (pre-auth and stranger links). When full, the oldest is evicted.
/// Authenticated friend links are never capped by this.
pub(crate) const MAX_UNVERIFIED_INBOUND: usize = 6;
/// Of those, at most this many from one source address (oldest evicted).
pub(crate) const MAX_UNVERIFIED_PER_IP: usize = 2;
/// Pre-auth links from an address that previously authenticated as a friend
/// are kept out of the stranger pool (reserved capacity), at most this many
/// per address.
pub(crate) const MAX_TRUSTED_PER_IP: usize = 4;
/// Addresses remembered as having authenticated as a friend.
pub(crate) const MAX_TRUSTED_IPS: usize = 64;
/// Handshake + hello must complete within this (or `conn_timeout` if shorter).
pub(crate) const PRE_AUTH_MAX: Duration = Duration::from_secs(3);
/// Longest redial backoff for a friend whose identity check keeps failing.
pub(crate) const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// Most concurrent incoming pairing requests, and per source address.
pub(crate) const MAX_INVITES_IN: usize = 4;
pub(crate) const MAX_INVITES_IN_PER_ADDR: usize = 2;
/// Dials to not-yet-verified candidates of one id per minute.
pub(crate) const MAX_UNVERIFIED_DIALS_PER_MIN: usize = 20;
/// At most this many `Invited` events per `INVITED_WINDOW`.
pub(crate) const MAX_INVITED_PER_WINDOW: usize = 10;
pub(crate) const INVITED_WINDOW: Duration = Duration::from_secs(60);
/// At most this many incoming pairing attempts per peer address per window.
pub(crate) const MAX_PAIR_ATTEMPTS_PER_ADDR: usize = 3;

/// Timing constants (spec defaults via `Default`; tests use short ones).
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    /// Discovery announcement period.
    pub announce_interval: Duration,
    /// A discovered peer silent this long is reported `PeerGone`.
    pub peer_timeout: Duration,
    /// `ping` period on each link.
    pub ping_interval: Duration,
    /// A link with nothing received this long is dropped; also the limit for
    /// completing the handshake + hello.
    pub conn_timeout: Duration,
    /// A pairing not confirmed by both users within this time is declined.
    pub invite_timeout: Duration,
    /// First wait before reconnecting to the relay (doubles up to 60 s); also
    /// the first redial delay for a friend whose relay link failed.
    pub relay_retry: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            announce_interval: Duration::from_secs(2),
            peer_timeout: Duration::from_secs(6),
            ping_interval: Duration::from_secs(2),
            conn_timeout: Duration::from_secs(6),
            invite_timeout: Duration::from_secs(20),
            relay_retry: Duration::from_secs(5),
        }
    }
}

/// A friend: install id (a label) pinned to the friend's Noise static public
/// key (the identity). The app persists these (e.g. in its config).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    pub public_key: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct NetConfig {
    /// This install's id (see [`new_install_id`]).
    pub id: String,
    /// Display name shown to others (clamped to 64 characters).
    pub name: String,
    /// This install's long-term X25519 private key (see
    /// [`Net::generate_static_key`]); persist it with the config.
    pub static_key: [u8; 32],
    /// UDP port to listen on for announcements (0 = any, see
    /// [`Net::discovery_port`]).
    pub discovery_port: u16,
    /// Where announcements are sent (normally `255.255.255.255:47321`).
    pub broadcast_to: SocketAddr,
    /// Friends paired earlier, as returned by [`Net::friends`].
    pub friends: Vec<Friend>,
    pub timing: Timing,
    /// Relay for friends outside the LAN (`wss://…` or `ws://…`), normally
    /// [`DEFAULT_RELAY_URL`]; `None` = LAN only.
    pub relay_url: Option<String>,
}

impl NetConfig {
    /// Spec defaults: port 47321, broadcast to 255.255.255.255:47321, spec
    /// timing, no relay (set `relay_url` to use one).
    pub fn new(id: impl Into<String>, name: impl Into<String>, static_key: [u8; 32], friends: Vec<Friend>) -> NetConfig {
        NetConfig {
            id: id.into(),
            name: name.into(),
            static_key,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            broadcast_to: SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT)),
            friends,
            timing: Timing::default(),
            relay_url: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetEvent {
    PeerSeen { id: String, name: String },
    PeerGone { id: String },
    /// A stranger asks to pair. Show `sas` and ask the user to compare it
    /// with the inviter's screen; answer with [`Net::answer_invite`] passing
    /// this `pairing` (it identifies exactly this request).
    Invited { from: String, name: String, pet: String, sas: String, pairing: u64 },
    /// Our invite reached `to`. Show `sas`; once the user confirms it matches
    /// the other screen, call [`Net::answer_invite`] with this `pairing`.
    InviteSent { to: String, sas: String, pairing: u64 },
    /// The other side confirmed (`true`) or the pairing ended without
    /// friendship (`false`: refused, timed out or link lost).
    /// `pairing` is the token of the `Invited`/`InviteSent` it concludes; no
    /// outcome is emitted for a pairing that was never surfaced.
    InviteAnswered { from: String, accepted: bool, pairing: u64 },
    Connected { id: String, name: String },
    Disconnected { id: String },
    Received { from: String, msg: Visit },
    /// Friend `id` proved its pinned key but no longer recognizes us as a
    /// friend (e.g. pairing finished on one side only, or it removed us).
    /// Redials back off; suggest re-pairing to the user. (An address that
    /// answers for `id` with a *different* key is just treated as a bad
    /// candidate, silently.)
    FriendForgotUs { id: String },
    /// The relay connection came up (`true`) or went down (`false`).
    RelayStatus { connected: bool },
    /// Our current friend code from the relay (on connect and on rotation).
    FriendCode { code: String },
    /// A code given to [`Net::redeem_code`] belongs to `id`; pairing with them
    /// has started (expect `InviteSent` / `InviteAnswered` as for `invite`).
    CodeRedeemed { code: String, id: String },
    /// A code could not be used. `reason`: `invalid` (not a code), `unknown`
    /// (no such code), `offline` (relay or owner not connected), `self`,
    /// `already_friend`, `busy`, `rate`, `timeout`.
    CodeFailed { code: String, reason: String },
}

/// A peer id heard via discovery. Announcements are unauthenticated hints.
pub(crate) struct Seen {
    /// Candidate → (last announced, announcing source IP).
    pub candidates: HashMap<Candidate, (Instant, IpAddr)>,
    /// Candidate that last passed the identity check, and when.
    pub verified: Option<(Candidate, Instant)>,
    /// When each candidate was last dialed (round-robin order).
    pub tried: HashMap<Candidate, Instant>,
    /// Recent dials to non-verified candidates (capped per minute).
    pub unverified_dials: VecDeque<Instant>,
    pub last: Instant,
}

/// Where a pairing is in the commit -> random -> reveal -> confirm exchange.
pub(crate) enum PairStage {
    /// Inviter: `invite{commit}` sent, waiting for the invitee's random.
    AwaitRandom { nonce_i: [u8; 32] },
    /// Invitee: random sent, waiting for the inviter's reveal.
    AwaitReveal { commit: [u8; 32], r_r: [u8; 32], pet: String },
    /// SAS shown on both sides; waiting for both users' confirmation.
    Confirm,
    /// Refused or timed out. Terminal: nothing on this link can pin.
    Failed,
}

/// An in-progress pairing, bound to the link that carried the invite.
pub(crate) struct Pairing {
    pub stage: PairStage,
    pub local_ok: bool,
    pub remote_ok: bool,
    pub since: Instant,
    /// A final `InviteAnswered{accepted:false}` was already emitted.
    pub failure_reported: bool,
    /// `Invited`/`InviteSent` was emitted: only then are outcomes reported.
    pub surfaced: bool,
}

pub(crate) enum Kind {
    /// Authenticated: remote static key equals the pinned key.
    Friend,
    /// Not a friend; may only pair (`None` until an invite is on the link).
    Stranger(Option<Pairing>),
}

/// One live link (after hello). Several stranger links may claim the same id;
/// at most one friend link per id (see `State::friend_links`).
pub(crate) struct Conn {
    pub peer_id: String,
    pub link: Arc<secure::SecureLink>,
    pub name: String,
    pub kind: Kind,
    /// `Connected` has been emitted for this peer (so `Disconnected` is owed).
    pub announced: bool,
    /// We opened this link.
    pub outgoing: bool,
    /// Id of the side that opened the link.
    pub dialer: String,
    pub remote_key: [u8; 32],
    /// Transport address of the peer (for trust / rate limiting).
    pub peer_addr: String,
    /// Noise handshake hash of this link (input to the pairing SAS).
    pub hs_hash: [u8; 32],
    pub since: Instant,
    /// Counts against `MAX_UNVERIFIED_INBOUND` while set.
    pub unverified: Arc<AtomicBool>,
}

/// What a redial backoff applies to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Backoff {
    /// The friend (authenticated) said it does not have us as a friend.
    Id(String),
    /// This address answered for a friend id with the wrong key.
    Cand(Candidate),
}

#[derive(Default)]
pub(crate) struct State {
    pub friends: Vec<Friend>,
    pub seen: HashMap<String, Seen>,
    /// Failure history per candidate (separate from `seen`, expires).
    pub failed: HashMap<Candidate, Instant>,
    /// Redial backoff: key → (no dial before, current delay).
    pub backoff: HashMap<Backoff, (Instant, Duration)>,
    /// All links, by session token.
    pub conns: HashMap<u64, Conn>,
    /// Friend id → token of its authenticated link.
    pub friend_links: HashMap<String, u64>,
    /// Friend id → since when it has been heard without a link (the larger
    /// id only dials after a grace period, so both sides rarely dial at once).
    pub friend_wait: HashMap<String, Instant>,
    pub dialing: HashSet<String>,
    /// Invites waiting for their outgoing link: peer id → pet.
    pub invites_out: HashMap<String, String>,
    /// Times of recent `Invited` events (rate limit).
    pub invited_log: VecDeque<Instant>,
    /// Discovery datagrams per source IP: (window start, count).
    pub src_rate: HashMap<IpAddr, (Instant, u32)>,
    /// Announcements claiming a friend id, per (id, source IP).
    pub friend_rate: HashMap<(String, IpAddr), (Instant, u32)>,
    /// Recent incoming pairing attempts per peer address.
    pub pair_attempts: HashMap<String, VecDeque<Instant>>,
    /// Addresses that authenticated as a friend → when (bounded).
    /// Keyed by (friend id, address).
    pub trusted_ips: HashMap<(String, String), Instant>,
    /// Announcements from a trusted (id, source IP), own finite budget.
    pub trusted_rate: HashMap<(String, IpAddr), (Instant, u32)>,
    /// Friend id → last attempt to move its relay link onto the LAN.
    pub lan_upgrade: HashMap<String, Instant>,
}

impl State {
    pub fn is_friend(&self, id: &str) -> bool {
        self.friends.iter().any(|f| f.id == id)
    }
    pub fn friend_key(&self, id: &str) -> Option<[u8; 32]> {
        self.friends.iter().find(|f| f.id == id).map(|f| f.public_key)
    }
    pub fn add_friend(&mut self, id: &str, key: [u8; 32]) {
        self.friends.retain(|f| f.id != id && f.public_key != key);
        self.friends.push(Friend { id: id.to_string(), public_key: key });
    }
    pub fn remove_friend(&mut self, id: &str) {
        self.friends.retain(|f| f.id != id);
    }
    pub fn is_trusted(&self, id: &str, ip: &str) -> bool {
        self.trusted_ips.contains_key(&(id.to_string(), ip.to_string()))
    }
    /// Has any friend authenticated from this address?
    pub fn ip_trusted(&self, ip: &str) -> bool {
        self.trusted_ips.keys().any(|(_, i)| i == ip)
    }
    pub fn trust_ip(&mut self, id: &str, ip: &str, now: Instant) {
        let key = (id.to_string(), ip.to_string());
        if self.trusted_ips.len() >= MAX_TRUSTED_IPS && !self.trusted_ips.contains_key(&key) {
            let oldest = self.trusted_ips.iter().min_by_key(|(_, t)| **t).map(|(i, _)| i.clone());
            if let Some(o) = oldest {
                self.trusted_ips.remove(&o);
            }
        }
        self.trusted_ips.insert(key, now);
    }
    pub fn friend_conn(&self, id: &str) -> Option<&Conn> {
        self.friend_links.get(id).and_then(|t| self.conns.get(t))
    }
    /// Is friend `id` linked only through the relay?
    pub fn relay_linked(&self, id: &str) -> bool {
        self.friend_conn(id).is_some_and(|c| c.peer_addr.starts_with(relay::ADDR_PREFIX))
    }
    pub fn backed_off(&self, key: &Backoff, now: Instant) -> bool {
        self.backoff.get(key).is_some_and(|(until, _)| now < *until)
    }
    /// Start or double the backoff for `key` (base `base`, up to `MAX_BACKOFF`).
    pub fn back_off(&mut self, key: Backoff, base: Duration, now: Instant) {
        let delay = match self.backoff.get(&key) {
            Some((_, d)) => (*d * 2).min(MAX_BACKOFF),
            None => base.min(MAX_BACKOFF),
        };
        if self.backoff.len() >= 256 && !self.backoff.contains_key(&key) {
            let oldest = self.backoff.iter().min_by_key(|(_, (u, _))| *u).map(|(k, _)| k.clone());
            if let Some(o) = oldest {
                self.backoff.remove(&o);
            }
        }
        self.backoff.insert(key, (now + delay, delay));
    }
}

pub(crate) struct Shared {
    pub id: String,
    pub name: String,
    pub static_key: [u8; 32],
    pub timing: Timing,
    pub tcp_port: u16,
    /// Relay transport, if configured.
    pub relay: Option<Arc<relay::RelayClient>>,
    pub stop: AtomicBool,
    pub state: Mutex<State>,
    events: Mutex<Sender<NetEvent>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    next_token: AtomicU64,
    unverified: Mutex<Vec<Unverified>>,
}

/// An inbound link not (yet) authenticated as a friend.
struct Unverified {
    ip: String,
    /// From an address that previously authenticated as a friend.
    trusted: bool,
    since: Instant,
    frames: Arc<dyn transport::FrameLink>,
    /// Cleared when the link authenticates as a friend or ends.
    active: Arc<AtomicBool>,
}

/// Holds an unverified-inbound slot (if any) until released or dropped.
pub(crate) struct SessionGuard {
    shared: Arc<Shared>,
    pub unverified: Arc<AtomicBool>,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.shared.release_unverified(&self.unverified);
    }
}

impl Shared {
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn emit(&self, ev: NetEvent) {
        let tx = self.events.lock().unwrap_or_else(|e| e.into_inner());
        let _ = tx.send(ev);
    }
    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    pub fn token(&self) -> u64 {
        self.next_token.fetch_add(1, Ordering::Relaxed)
    }
    /// Spawn a worker thread that `shutdown` will join. `false` (and `f`
    /// dropped) if the OS refused a thread.
    pub fn spawn(&self, f: impl FnOnce() + Send + 'static) -> bool {
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        threads.retain(|t| !t.is_finished());
        match std::thread::Builder::new().name("screenpet-net".into()).spawn(f) {
            Ok(h) => {
                threads.push(h);
                true
            }
            Err(_) => false,
        }
    }
    /// Admit an inbound link into the unverified pool, evicting the oldest
    /// unverified link from the same address (if it has `MAX_UNVERIFIED_PER_IP`)
    /// or overall (if the pool is full). New links are never refused, so a
    /// flood cannot lock friends out; it can only churn other unverified links.
    ///
    /// Links from an address that previously authenticated as a friend use a
    /// separate per-address allowance, so strangers can never crowd them out.
    pub fn admit_inbound(self: &Arc<Self>, frames: Arc<dyn transport::FrameLink>) -> SessionGuard {
        let ip = frames.peer_addr();
        let trusted = self.state().ip_trusted(&ip);
        let active = Arc::new(AtomicBool::new(true));
        let mut evicted = Vec::new();
        {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
            let mut evict_oldest = |pool: &mut Vec<Unverified>, same_ip: bool| {
                let victim = pool
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.trusted == trusted && (!same_ip || u.ip == ip))
                    .min_by_key(|(_, u)| u.since)
                    .map(|(i, _)| i);
                if let Some(i) = victim {
                    let u = pool.swap_remove(i);
                    u.active.store(false, Ordering::SeqCst);
                    evicted.push(u.frames);
                }
            };
            let same_ip = pool.iter().filter(|u| u.trusted == trusted && u.ip == ip).count();
            if trusted {
                if same_ip >= MAX_TRUSTED_PER_IP {
                    evict_oldest(&mut pool, true);
                }
            } else if same_ip >= MAX_UNVERIFIED_PER_IP {
                evict_oldest(&mut pool, true);
            } else if pool.iter().filter(|u| !u.trusted).count() >= MAX_UNVERIFIED_INBOUND {
                evict_oldest(&mut pool, false);
            }
            pool.push(Unverified { ip, trusted, since: Instant::now(), frames, active: active.clone() });
        }
        for f in evicted {
            f.close();
        }
        SessionGuard { shared: self.clone(), unverified: active }
    }
    /// Outgoing links hold no unverified slot.
    pub fn outbound_guard(self: &Arc<Self>) -> SessionGuard {
        SessionGuard { shared: self.clone(), unverified: Arc::new(AtomicBool::new(false)) }
    }
    /// The link proved to be a friend (or ended): free its unverified slot.
    pub fn release_unverified(&self, flag: &AtomicBool) {
        if flag.swap(false, Ordering::SeqCst) {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
        }
    }
}

/// Handle to the running network. Dropping it (or `shutdown`) stops all
/// threads and closes all sockets.
pub struct Net {
    shared: Arc<Shared>,
    main_threads: Vec<JoinHandle<()>>,
    discovery_port: u16,
}

impl Net {
    /// A new long-term static private key from the OS CSPRNG. Generate once
    /// per install and persist it.
    pub fn generate_static_key() -> [u8; 32] {
        noise::generate_static_key()
    }

    /// The public key for a static private key (what friends pin).
    pub fn public_key(static_key: &[u8; 32]) -> [u8; 32] {
        noise::public_key(static_key)
    }

    pub fn start(cfg: NetConfig) -> std::io::Result<(Net, Receiver<NetEvent>)> {
        let udp = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], cfg.discovery_port)))?;
        udp.set_broadcast(true)?;
        udp.set_read_timeout(Some(TICK))?;
        let discovery_port = udp.local_addr()?.port();
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        listener.set_nonblocking(true)?;
        let tcp_port = listener.local_addr()?.port();

        let (tx, rx) = channel();
        let mut state = State::default();
        let own_public = noise::public_key(&cfg.static_key);
        for f in &cfg.friends {
            if f.id != cfg.id && !f.id.is_empty() && f.id.len() <= protocol::MAX_ID && f.public_key != own_public {
                state.add_friend(&f.id, f.public_key);
            }
        }
        let shared = Arc::new(Shared {
            id: cfg.id,
            name: protocol::clamp_name(&cfg.name),
            static_key: cfg.static_key,
            timing: cfg.timing,
            tcp_port,
            relay: cfg.relay_url.filter(|u| !u.is_empty()).map(relay::RelayClient::new),
            stop: AtomicBool::new(false),
            state: Mutex::new(state),
            events: Mutex::new(tx),
            threads: Mutex::new(Vec::new()),
            next_token: AtomicU64::new(1),
            unverified: Mutex::new(Vec::new()),
        });
        let s1 = shared.clone();
        let s2 = shared.clone();
        let broadcast_to = cfg.broadcast_to;
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| std::thread::Builder::new().name(name.into()).spawn(f);
        let mut main_threads = vec![spawn("screenpet-discovery", Box::new(move || discovery::run(s1, udp, broadcast_to)))?];
        let mut started = spawn("screenpet-accept", Box::new(move || tcp::accept_loop(s2, listener))).map(|h| main_threads.push(h));
        if let (Ok(()), Some(client)) = (&started, shared.relay.clone()) {
            let s3 = shared.clone();
            started = spawn("screenpet-relay", Box::new(move || relay::run(s3, client))).map(|h| main_threads.push(h));
        }
        if let Err(e) = started {
            shared.stop.store(true, Ordering::SeqCst);
            for t in main_threads {
                let _ = t.join();
            }
            return Err(e);
        }
        Ok((Net { shared, main_threads, discovery_port }, rx))
    }

    /// The UDP port discovery is bound to.
    pub fn discovery_port(&self) -> u16 {
        self.discovery_port
    }

    /// Ask a discovered stranger to pair. Expect `InviteSent{sas}` (then
    /// confirm with `answer_invite`) and `InviteAnswered` from the other side.
    pub fn invite(&self, peer_id: &str, pet: &str) {
        peers::invite(&self.shared, peer_id, pet);
    }

    /// Pair with the owner of a relay friend code (e.g. `ABCD-1234`, typed by
    /// the user). Expect `CodeRedeemed` then the usual `InviteSent` /
    /// `InviteAnswered`, or `CodeFailed`. Never blocks.
    pub fn redeem_code(&self, code: &str, pet: &str) {
        relay::redeem(&self.shared, code, pet);
    }

    /// Is the relay connection up?
    pub fn relay_connected(&self) -> bool {
        self.shared.relay.as_ref().is_some_and(|r| r.connected())
    }

    /// Our current relay friend code, once connected.
    pub fn friend_code(&self) -> Option<String> {
        self.shared.relay.as_ref().and_then(|r| r.code())
    }

    /// Confirm (`true`, the SAS matched) or refuse the pairing identified by
    /// `pairing` (from `Invited` / `InviteSent`). Both users must confirm;
    /// then both sides pin each other's key and `Connected` follows.
    pub fn answer_invite(&self, pairing: u64, accept: bool) {
        peers::answer_invite(&self.shared, pairing, accept);
    }

    /// Queue a visit message to a connected friend (dropped otherwise).
    /// Never blocks.
    pub fn send(&self, peer_id: &str, msg: Visit) {
        let link = {
            let st = self.shared.state();
            match st.friend_conn(peer_id) {
                Some(c) if matches!(c.kind, Kind::Friend) => c.link.clone(),
                _ => return,
            }
        };
        link.send(&Message::Visit(msg));
    }

    /// Forget a friend, tell them, and disconnect (gracefully). Never blocks.
    pub fn unfriend(&self, peer_id: &str) {
        let link = {
            let mut st = self.shared.state();
            st.remove_friend(peer_id);
            st.invites_out.remove(peer_id);
            let token = st.friend_links.remove(peer_id);
            token.and_then(|t| st.conns.get_mut(&t)).map(|c| {
                c.kind = Kind::Stranger(None); // `announced` stays: Disconnected follows
                c.link.clone()
            })
        };
        if let Some(l) = link {
            l.send(&Message::Control(Control::Unfriend));
            l.finish();
        }
    }

    /// Current friends (id + pinned public key), including ones added by
    /// pairing — the app persists these and passes them back in `NetConfig`.
    pub fn friends(&self) -> Vec<Friend> {
        self.shared.state().friends.clone()
    }

    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let links: Vec<_> = self.shared.state().conns.values().map(|c| c.link.clone()).collect();
        for l in links {
            l.close();
        }
        for t in self.main_threads.drain(..) {
            let _ = t.join();
        }
        // Workers can still spawn link writers; drain until none are left.
        loop {
            let workers: Vec<_> =
                std::mem::take(&mut *self.shared.threads.lock().unwrap_or_else(|e| e.into_inner()));
            if workers.is_empty() {
                break;
            }
            for t in workers {
                let _ = t.join();
            }
        }
    }
}

/// Random 128-bit install id as 32 lowercase hex digits.
pub fn new_install_id() -> String {
    format!("{:032x}", fastrand::u128(..))
}
