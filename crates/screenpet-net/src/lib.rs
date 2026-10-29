//! ScreenPet LAN networking: discovery (UDP broadcast), one-time invites, and
//! persistent friend connections (TCP, line-delimited JSON, ping/timeout).
//!
//! Everything runs on plain std threads. The app talks to [`Net`] through its
//! methods and receives [`NetEvent`]s on an mpsc channel.
//!
//! Layering:
//! - `protocol`: wire messages (`hello`, `invite`, `visit`, ...), framing-free.
//! - `transport`: the `Link`/`LinkReader` seam — a bidirectional message pipe
//!   to one peer. `tcp` is the LAN implementation (line-delimited JSON); a
//!   relay (WebSocket) transport can implement the same traits later.
//! - `peers`: the per-peer session driver (handshake, invites, friends, ping,
//!   timeouts) — works on any `Link`, never sees bytes or sockets.
//! - `discovery`: LAN UDP announcements — only *hints* (candidate addresses)
//!   about where to dial; a link counts only after `auth` proves identity.
//! - `auth`: per-friendship secrets and the hello challenge-response.

mod auth;
mod discovery;
mod peers;
mod protocol;
mod tcp;
mod transport;

pub use protocol::{Side, Visit};

use protocol::{Control, Message};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Spec default UDP discovery port.
pub const DEFAULT_DISCOVERY_PORT: u16 = 47321;

/// How often worker threads wake up to check timers and the stop flag.
pub(crate) const TICK: Duration = Duration::from_millis(50);

/// Timing constants (spec defaults via `Default`; tests use short ones).
#[derive(Debug, Clone, PartialEq)]
pub struct Timing {
    /// Discovery announcement period.
    pub announce_interval: Duration,
    /// A discovered peer silent this long is reported `PeerGone`.
    pub peer_timeout: Duration,
    /// `ping` period on each connection.
    pub ping_interval: Duration,
    /// A connection with nothing received this long is dropped.
    pub conn_timeout: Duration,
    /// An incoming invite not answered this long is declined automatically.
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

#[derive(Debug, Clone)]
pub struct NetConfig {
    /// This install's id (see [`new_install_id`]).
    pub id: String,
    /// Display name shown to others.
    pub name: String,
    /// UDP port to listen on for announcements.
    pub discovery_port: u16,
    /// Where announcements are sent (normally `255.255.255.255:discovery_port`).
    pub broadcast_to: SocketAddr,
    /// Friends accepted earlier (auto-connect, no prompt), as returned by
    /// [`Net::friends`]. Entries with a malformed secret are ignored.
    pub friends: Vec<Friend>,
    pub timing: Timing,
}

impl NetConfig {
    /// Spec defaults: port 47321, broadcast to 255.255.255.255:47321, spec timing.
    pub fn new(id: impl Into<String>, name: impl Into<String>, friends: Vec<Friend>) -> NetConfig {
        NetConfig {
            id: id.into(),
            name: name.into(),
            discovery_port: DEFAULT_DISCOVERY_PORT,
            broadcast_to: SocketAddr::from(([255, 255, 255, 255], DEFAULT_DISCOVERY_PORT)),
            friends,
            timing: Timing::default(),
        }
    }
}

/// A friendship: the peer's install id and the shared secret (64 hex chars)
/// that authenticates it. The app persists these (e.g. in its config).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NetEvent {
    PeerSeen { id: String, name: String },
    PeerGone { id: String },
    Invited { from: String, name: String, pet: String },
    InviteAnswered { from: String, accepted: bool },
    Connected { id: String, name: String },
    Disconnected { id: String },
    Received { from: String, msg: Visit },
}

/// A peer id heard via discovery. Announcements are unauthenticated hints, so
/// every address announced for the id is kept as a candidate.
pub(crate) struct Seen {
    /// Candidate address → when it was last announced.
    pub candidates: HashMap<SocketAddr, Instant>,
    /// Candidates whose last dial failed (no connect / wrong id / failed auth)
    /// → when. Avoided while any other candidate is untried or good.
    pub bad: HashMap<SocketAddr, Instant>,
}

/// The live connection to one peer (at most one per id).
pub(crate) struct Conn {
    pub token: u64,
    pub link: Arc<dyn transport::Link>,
    pub name: String,
    /// `Connected` has been emitted for this peer (so `Disconnected` is owed).
    pub announced: bool,
    /// Id of the side that opened the connection.
    pub dialer: String,
    pub since: Instant,
}

/// An invite this side sent (or is about to send once connected).
pub(crate) struct OutInvite {
    pub pet: String,
    pub sent: bool,
    pub since: Instant,
}

#[derive(Default)]
pub(crate) struct State {
    pub friends: Vec<Friend>,
    pub seen: HashMap<String, Seen>,
    pub conns: HashMap<String, Conn>,
    pub dialing: HashSet<String>,
    pub invites_out: HashMap<String, OutInvite>,
    pub invites_in: HashMap<String, Instant>,
}

impl State {
    pub fn is_friend(&self, id: &str) -> bool {
        self.friends.iter().any(|f| f.id == id)
    }
    pub fn secret(&self, id: &str) -> Option<String> {
        self.friends.iter().find(|f| f.id == id).map(|f| f.secret.clone())
    }
    pub fn add_friend(&mut self, id: &str, secret: &str) {
        match self.friends.iter_mut().find(|f| f.id == id) {
            Some(f) => f.secret = secret.to_string(),
            None => self.friends.push(Friend { id: id.to_string(), secret: secret.to_string() }),
        }
    }
    pub fn remove_friend(&mut self, id: &str) {
        self.friends.retain(|f| f.id != id);
    }
}

pub(crate) struct Shared {
    pub id: String,
    pub name: String,
    pub timing: Timing,
    pub tcp_port: u16,
    pub stop: AtomicBool,
    pub state: Mutex<State>,
    events: Mutex<Sender<NetEvent>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    next_token: AtomicU64,
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
    /// Spawn a worker thread that `shutdown` will join.
    pub fn spawn(&self, f: impl FnOnce() + Send + 'static) {
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        threads.retain(|t| !t.is_finished());
        threads.push(std::thread::spawn(f));
    }
}

/// Handle to the running network. Dropping it (or `shutdown`) stops all
/// threads and closes all sockets.
pub struct Net {
    shared: Arc<Shared>,
    main_threads: Vec<JoinHandle<()>>,
}

impl Net {
    pub fn start(cfg: NetConfig) -> std::io::Result<(Net, Receiver<NetEvent>)> {
        let udp = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], cfg.discovery_port)))?;
        udp.set_broadcast(true)?;
        udp.set_read_timeout(Some(TICK))?;
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))?;
        listener.set_nonblocking(true)?;
        let tcp_port = listener.local_addr()?.port();

        let (tx, rx) = channel();
        let mut state = State::default();
        for f in &cfg.friends {
            if f.id != cfg.id && !f.id.is_empty() && auth::valid_random(&f.secret) {
                state.add_friend(&f.id, &f.secret);
            }
        }
        let shared = Arc::new(Shared {
            id: cfg.id,
            name: cfg.name,
            timing: cfg.timing,
            tcp_port,
            stop: AtomicBool::new(false),
            state: Mutex::new(state),
            events: Mutex::new(tx),
            threads: Mutex::new(Vec::new()),
            next_token: AtomicU64::new(1),
        });
        let s1 = shared.clone();
        let s2 = shared.clone();
        let broadcast_to = cfg.broadcast_to;
        let main_threads = vec![
            std::thread::spawn(move || discovery::run(s1, udp, broadcast_to)),
            std::thread::spawn(move || tcp::accept_loop(s2, listener)),
        ];
        Ok((Net { shared, main_threads }, rx))
    }

    /// Ask a discovered peer to become friends. The answer arrives as
    /// `InviteAnswered` (unreachable/unknown peer → declined).
    pub fn invite(&self, peer_id: &str, pet: &str) {
        peers::invite(&self.shared, peer_id, pet);
    }

    /// Reply to an `Invited` event. Accepting adds the peer as a friend and
    /// emits `Connected`.
    pub fn answer_invite(&self, peer_id: &str, accept: bool) {
        peers::answer_invite(&self.shared, peer_id, accept);
    }

    /// Send a visit message to a connected friend (dropped otherwise).
    pub fn send(&self, peer_id: &str, msg: Visit) {
        let link = {
            let st = self.shared.state();
            match st.conns.get(peer_id) {
                Some(c) if c.announced && st.is_friend(peer_id) => c.link.clone(),
                _ => return,
            }
        };
        link.send(&Message::Visit(msg));
    }

    /// Forget a friend, tell them, and disconnect.
    pub fn unfriend(&self, peer_id: &str) {
        let link = {
            let mut st = self.shared.state();
            st.remove_friend(peer_id);
            st.invites_in.remove(peer_id);
            st.invites_out.remove(peer_id);
            st.conns.get(peer_id).map(|c| c.link.clone())
        };
        if let Some(l) = link {
            l.send(&Message::Control(Control::Unfriend));
            l.close();
        }
    }

    /// Current friends (with secrets), including ones added by accepted
    /// invites — the app persists these and passes them back in `NetConfig`.
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
        // Main threads are gone, so nothing spawns new workers any more.
        let workers: Vec<_> =
            std::mem::take(&mut *self.shared.threads.lock().unwrap_or_else(|e| e.into_inner()));
        for t in workers {
            let _ = t.join();
        }
    }
}

/// Random 128-bit install id as 32 lowercase hex digits.
pub fn new_install_id() -> String {
    format!("{:032x}", fastrand::u128(..))
}
