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

mod discovery;
mod noise;
mod peers;
mod protocol;
mod secure;
mod tcp;
mod transport;

pub use noise::SAS_EMOJI;
pub use protocol::{Side, Visit};

use protocol::{Control, Message};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use transport::Candidate;

/// Spec default UDP discovery port.
pub const DEFAULT_DISCOVERY_PORT: u16 = 47321;

/// How often worker threads wake up to check timers and the stop flag.
pub(crate) const TICK: Duration = Duration::from_millis(50);
/// Most concurrent sessions (links) of any kind.
pub(crate) const MAX_SESSIONS: usize = 32;
/// Most concurrent inbound links not (yet) authenticated as a friend.
pub(crate) const MAX_UNVERIFIED_INBOUND: usize = 4;
/// Most concurrent incoming pairing requests.
pub(crate) const MAX_INVITES_IN: usize = 4;
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
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            announce_interval: Duration::from_secs(2),
            peer_timeout: Duration::from_secs(6),
            ping_interval: Duration::from_secs(2),
            conn_timeout: Duration::from_secs(6),
            invite_timeout: Duration::from_secs(20),
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
}

impl NetConfig {
    /// Spec defaults: port 47321, broadcast to 255.255.255.255:47321, spec timing.
    pub fn new(id: impl Into<String>, name: impl Into<String>, static_key: [u8; 32], friends: Vec<Friend>) -> NetConfig {
        NetConfig {
            id: id.into(),
            name: name.into(),
            static_key,
            discovery_port: DEFAULT_DISCOVERY_PORT,
            broadcast_to: SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT)),
            friends,
            timing: Timing::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetEvent {
    PeerSeen { id: String, name: String },
    PeerGone { id: String },
    /// A stranger asks to pair. Show `sas` and ask the user to compare it
    /// with the inviter's screen; answer with [`Net::answer_invite`].
    Invited { from: String, name: String, pet: String, sas: String },
    /// Our invite reached `to`. Show `sas`; once the user confirms it matches
    /// the other screen, call [`Net::answer_invite`] for `to` as well.
    InviteSent { to: String, sas: String },
    /// The other side confirmed (`true`) or the pairing ended without
    /// friendship (`false`: refused, timed out or link lost).
    InviteAnswered { from: String, accepted: bool },
    Connected { id: String, name: String },
    Disconnected { id: String },
    Received { from: String, msg: Visit },
}

/// A peer id heard via discovery. Announcements are unauthenticated hints.
pub(crate) struct Seen {
    /// Candidate → (last announced, announcing source IP).
    pub candidates: HashMap<Candidate, (Instant, IpAddr)>,
    /// Candidate that last passed the identity check, and when.
    pub verified: Option<(Candidate, Instant)>,
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
}

/// An in-progress pairing, bound to the link that carried the invite.
pub(crate) struct Pairing {
    pub stage: PairStage,
    pub local_ok: bool,
    pub remote_ok: bool,
    pub since: Instant,
    /// A final `InviteAnswered{accepted:false}` was already emitted.
    pub failure_reported: bool,
}

pub(crate) enum Kind {
    /// Authenticated: remote static key equals the pinned key.
    Friend,
    /// Not a friend; may only pair (`None` until an invite is on the link).
    Stranger(Option<Pairing>),
}

/// The live link to one peer (at most one per id).
pub(crate) struct Conn {
    pub token: u64,
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
    /// Noise handshake hash of this link (input to the pairing SAS).
    pub hs_hash: [u8; 32],
    pub since: Instant,
    /// Counts against `MAX_UNVERIFIED_INBOUND` while set.
    pub unverified: Arc<AtomicBool>,
}

#[derive(Default)]
pub(crate) struct State {
    pub friends: Vec<Friend>,
    pub seen: HashMap<String, Seen>,
    /// Failure history per candidate (separate from `seen`, expires).
    pub failed: HashMap<Candidate, Instant>,
    pub conns: HashMap<String, Conn>,
    pub dialing: HashSet<String>,
    /// Invites waiting for their outgoing link: peer id → pet.
    pub invites_out: HashMap<String, String>,
    /// Times of recent `Invited` events (rate limit).
    pub invited_log: VecDeque<Instant>,
    /// Discovery datagrams per source IP: (window start, count).
    pub src_rate: HashMap<IpAddr, (Instant, u32)>,
    /// Recent incoming pairing attempts per peer address.
    pub pair_attempts: HashMap<String, VecDeque<Instant>>,
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
}

pub(crate) struct Shared {
    pub id: String,
    pub name: String,
    pub static_key: [u8; 32],
    pub timing: Timing,
    pub tcp_port: u16,
    pub stop: AtomicBool,
    pub state: Mutex<State>,
    events: Mutex<Sender<NetEvent>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    next_token: AtomicU64,
    sessions: AtomicUsize,
    unverified_inbound: AtomicUsize,
}

/// Holds one session slot (and an unverified-inbound slot until released).
pub(crate) struct SessionGuard {
    shared: Arc<Shared>,
    pub unverified: Arc<AtomicBool>,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.shared.release_unverified(&self.unverified);
        self.shared.sessions.fetch_sub(1, Ordering::SeqCst);
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
    fn take_slot(counter: &AtomicUsize, max: usize) -> bool {
        let mut n = counter.load(Ordering::SeqCst);
        while n < max {
            match counter.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return true,
                Err(actual) => n = actual,
            }
        }
        false
    }
    pub fn try_inbound(self: &Arc<Self>) -> Option<SessionGuard> {
        if !Self::take_slot(&self.sessions, MAX_SESSIONS) {
            return None;
        }
        if !Self::take_slot(&self.unverified_inbound, MAX_UNVERIFIED_INBOUND) {
            self.sessions.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(SessionGuard { shared: self.clone(), unverified: Arc::new(AtomicBool::new(true)) })
    }
    pub fn try_outbound(self: &Arc<Self>) -> Option<SessionGuard> {
        Self::take_slot(&self.sessions, MAX_SESSIONS)
            .then(|| SessionGuard { shared: self.clone(), unverified: Arc::new(AtomicBool::new(false)) })
    }
    /// The link proved to be a friend: free its unverified-inbound slot.
    pub fn release_unverified(&self, flag: &AtomicBool) {
        if flag.swap(false, Ordering::SeqCst) {
            self.unverified_inbound.fetch_sub(1, Ordering::SeqCst);
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
            stop: AtomicBool::new(false),
            state: Mutex::new(state),
            events: Mutex::new(tx),
            threads: Mutex::new(Vec::new()),
            next_token: AtomicU64::new(1),
            sessions: AtomicUsize::new(0),
            unverified_inbound: AtomicUsize::new(0),
        });
        let s1 = shared.clone();
        let s2 = shared.clone();
        let broadcast_to = cfg.broadcast_to;
        let spawn = |name: &str, f: Box<dyn FnOnce() + Send>| std::thread::Builder::new().name(name.into()).spawn(f);
        let mut main_threads = vec![spawn("screenpet-discovery", Box::new(move || discovery::run(s1, udp, broadcast_to)))?];
        match spawn("screenpet-accept", Box::new(move || tcp::accept_loop(s2, listener))) {
            Ok(h) => main_threads.push(h),
            Err(e) => {
                shared.stop.store(true, Ordering::SeqCst);
                for t in main_threads {
                    let _ = t.join();
                }
                return Err(e);
            }
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

    /// Confirm (`true`, the SAS matched) or refuse a pairing — the invitee
    /// answers `Invited`, the inviter confirms after `InviteSent`. Friends are
    /// pinned (and `Connected` emitted) once both sides confirmed.
    pub fn answer_invite(&self, peer_id: &str, accept: bool) {
        peers::answer_invite(&self.shared, peer_id, accept);
    }

    /// Queue a visit message to a connected friend (dropped otherwise).
    /// Never blocks.
    pub fn send(&self, peer_id: &str, msg: Visit) {
        let link = {
            let st = self.shared.state();
            match st.conns.get(peer_id) {
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
            st.conns.get_mut(peer_id).map(|c| {
                if matches!(c.kind, Kind::Friend) {
                    c.kind = Kind::Stranger(None);
                }
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
