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

mod devices;
mod discovery;
pub mod ladder;
mod noise;
mod peers;
mod protocol;
mod relay;
mod secure;
mod tcp;
mod transport;

pub use noise::{hex32, to_hex, SAS_EMOJI};
pub use protocol::{Fling, LadderMsg, Side, Steer, Toss, VSide, Visit, WireCare, WirePlayer, WireResult, DEVICES_MIN_VERSION, LADDER_MIN_VERSION, MAX_DEVICES, MAX_FRIENDS, MAX_NOTE, PROTOCOL_VERSION};
pub use relay::{normalize_code, parse_relay_key, DEFAULT_RELAY_KEY, DEFAULT_RELAY_URL};

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
/// New inbound connections per source address per second; past this they are
/// dropped before any thread or key exchange is spent on them.
pub(crate) const MAX_CONNS_PER_ADDR: u32 = 20;
/// The same for an address that previously authenticated as a friend.
pub(crate) const MAX_CONNS_PER_TRUSTED_ADDR: u32 = 100;
/// Most inbound session threads still before their hello, per class of link
/// (see `HandshakeClass`). Evicted links end within a tick, so a flood only
/// reaches this if it outruns even that.
pub(crate) const MAX_HANDSHAKES: usize = 32;
/// Most unverified links from addresses a friend authenticated from, in all.
pub(crate) const MAX_TRUSTED_INBOUND: usize = 32;
/// Handshake + hello must complete within this (or `conn_timeout` if shorter).
pub(crate) const PRE_AUTH_MAX: Duration = Duration::from_secs(3);
/// Longest redial backoff for a friend whose identity check keeps failing.
pub(crate) const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// Most concurrent incoming pairing requests, and per source address.
pub(crate) const MAX_INVITES_IN: usize = 4;
pub(crate) const MAX_INVITES_IN_PER_ADDR: usize = 2;
/// Dials to not-yet-verified candidates of one id, per announcing source, per minute.
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
    /// 📱 After a link code matched, both users must press 확인 within this
    /// time ([`Net::confirm_device`]); otherwise it is cancelled on both.
    pub device_confirm_timeout: Duration,
    /// 📱 A computer that goes on standby keeps its friend links this long
    /// (so the app can call its pet home) before it closes them.
    pub standby_grace: Duration,
    /// 📱 The computer that plays shares a changed pet or care at most this
    /// often (changes in between are coalesced) …
    pub device_state_every: Duration,
    /// … and the same state again this often (in case it went missing).
    pub device_state_refresh: Duration,
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
            device_confirm_timeout: Duration::from_secs(60),
            standby_grace: Duration::from_secs(5),
            device_state_every: Duration::from_secs(2),
            device_state_refresh: Duration::from_secs(60),
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
    /// The relay's static public key, if pinned (normally
    /// [`DEFAULT_RELAY_KEY`] with [`DEFAULT_RELAY_URL`]). A relay that
    /// presents another key is refused. `None` = trust on each connect.
    pub relay_key: Option<[u8; 32]>,
    /// My linked other computer and our stored secret (see
    /// [`Net::new_link_code`]); persist it with the friend. Ignored if no friend.
    pub device: Option<(String, DeviceSecret)>,
    /// Friends a friend introduced as its other computers: (device id, the
    /// introducing friend's id). They leave together with that friend.
    pub introduced: Vec<(String, String)>,
    /// Friends added from another computer or an introduction that have not
    /// yet shown on a link that they know us (see [`FriendInfo::pending`]).
    pub pending: Vec<String>,
    /// When each of `pending` was added (unix seconds), so a restart doesn't
    /// restart its clock. One missing here counts from this start.
    pub pending_added: Vec<(String, u64)>,
}

/// The secret two linked computers share (persist it; never show or log it:
/// `Debug` prints no bytes).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeviceSecret(pub [u8; 32]);

impl std::fmt::Debug for DeviceSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceSecret(..)")
    }
}

/// Why [`Net::new_link_code`] / [`Net::enter_link_code`] could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Not a friend.
    NotFriend,
    /// Already linked to this other computer (unlink it first): at most two computers.
    Taken(String),
    /// Not a link code.
    Invalid,
    /// The friend is not connected now.
    Offline,
    /// The friend's build does not know linking.
    Old,
}

/// What [`Net::friend_info`] knows about a friend besides its key.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FriendInfo {
    /// It is my linked other computer.
    pub device: bool,
    /// It was introduced by this friend as one of its computers.
    pub via: Option<String>,
    /// Added from another computer or an introduction and not yet seen
    /// knowing us; not removed when it says it doesn't (yet).
    pub pending: bool,
    /// When it was added unconfirmed (unix seconds; `None` if confirmed).
    pub pending_since: Option<u64>,
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
            relay_key: None,
            device: None,
            introduced: Vec::new(),
            pending: Vec::new(),
            pending_added: Vec::new(),
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
    /// Friend `id` changed its display name (a repeated `hello` on its
    /// authenticated friend link; clamped to 64 characters). Older versions
    /// ignore a repeated `hello`, so this stays backward compatible.
    Renamed { id: String, name: String },
    /// Friend `from` sent an app request with [`Net::ask`] (clamped to 64
    /// characters; only from authenticated friend links).
    Asked { from: String, ask: String },
    /// Friend `from` steers its pet visiting us (control mode, sent with
    /// [`Net::steer`]; authenticated friend links only, rate limited). The
    /// app must check that `steer.visit_id` is `from`'s own guest.
    Steered { from: String, steer: Steer },
    /// Friend `from` threw an object onto our screen (sent with [`Net::toss`];
    /// authenticated friend links only, at most 10 at once then one per 6 s per
    /// friend, across reconnects). The app must validate `kind`, `fling` and `note`.
    Tossed { from: String, toss: Toss },
    Disconnected { id: String },
    Received { from: String, msg: Visit },
    /// A ladder game message from a connected friend (validated by `ladder`).
    Ladder { from: String, msg: LadderMsg },
    /// Friend `id` proved its pinned key but no longer recognizes us as a
    /// friend (e.g. pairing finished on one side only, or it removed us), so
    /// it has been removed here too: [`Net::friends`] no longer lists it. Only
    /// the pinned key can cause this; an address that answers for `id` with a
    /// *different* key is just treated as a bad candidate, silently.
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
    /// `already_friend`, `busy`, `rate`, `timeout`, `full` (we have `MAX_FRIENDS`),
    /// `disabled` (the relay has friend codes switched off).
    CodeFailed { code: String, reason: String },
    /// A friend was pinned without pairing: from `device_friends` of my
    /// linked other computer (`via: None`; `name` and `side` are its
    /// first values, clamped but otherwise unchecked), or introduced by friend
    /// `via` as one of its computers (`name` empty). [`Net::friends`] lists it.
    FriendAdded { id: String, name: String, via: Option<String>, side: Option<String> },
    /// A friend added without pairing was removed: introduced by a friend
    /// that is now removed, or never confirmed (a week, or it said three
    /// times that it doesn't know us).
    FriendDropped { id: String },
    /// The link code matched with friend `id`: ask the user "본인의 PC가
    /// 맞습니까?" and answer with [`Net::confirm_device`]. Nothing is linked,
    /// stored or shared until both computers confirm (then
    /// [`NetEvent::DeviceLinked`]); a no, a lost link or
    /// `Timing::device_confirm_timeout` gives [`NetEvent::DeviceLinkCancelled`]
    /// on both.
    DeviceConfirm { id: String },
    /// Linking with friend `id` was cancelled before both confirmed (one of
    /// us said no, did not answer in time, or the link was lost). Nothing was
    /// linked or stored.
    DeviceLinkCancelled { id: String },
    /// Friend `id` is now my linked other computer: a code matched and both
    /// users confirmed (store [`Net::device_secret`]). Friend lists flow on this link
    /// now — send it mine ([`Net::send_device_friends`]).
    DeviceLinked { id: String },
    /// Linking with friend `id` by code did not work: `reason` = `wrong`
    /// (`left` tries remain on the code's side), `locked` (too many wrong
    /// tries: the code is gone), `none` (no live code for us there),
    /// `taken` (one of us is linked to another computer).
    DeviceLinkFailed { id: String, reason: String, left: u32 },
    /// My linked computer `id` proved our stored secret on its new link:
    /// friend lists flow now — send it mine ([`Net::send_device_friends`]).
    DeviceProven { id: String },
    /// Friend `id` is no longer my linked computer (it unlinked, or did not
    /// hold our secret); still a friend. Forget the stored secret.
    DeviceUnlinked { id: String },
    /// My other computer `from` sent its friends (already merged). `want` =
    /// it asks for mine ([`Net::send_device_friends`]).
    DeviceSynced { from: String, want: bool },
    /// 📱 Which of my two linked computers plays changed. `me` = this one is
    /// on standby: friend links close after `Timing::standby_grace` (call my
    /// pet home from friends now) and none open until it plays again.
    /// `device` = my linked computer is on standby (I play; don't send it my
    /// pet). Both `false` when the two can't reach each other: each plays.
    Standby { me: bool, device: bool },
    /// 📱 My linked computer `from`, which plays, shares its selected pet and
    /// every pet's care (or hands them over as I take over). Shape-checked
    /// (pet ids, at most `MAX_CARE` entries, one per pet); the app checks
    /// species and clamps values, then adopts it. Only when it differs from
    /// the last state shared either way.
    DeviceState { from: String, pet: Option<String>, care: Vec<WireCare> },
}

/// A peer id heard via discovery. Announcements are unauthenticated hints.
pub(crate) struct Seen {
    /// Candidate → (last announced, announcing source IP).
    pub candidates: HashMap<Candidate, (Instant, IpAddr)>,
    /// Candidate that last passed the identity check, and when.
    pub verified: Option<(Candidate, Instant)>,
    /// When each candidate was last dialed (round-robin order).
    pub tried: HashMap<Candidate, Instant>,
    /// Recent dials to non-verified candidates, per announcing source IP
    /// (capped per minute).
    pub unverified_dials: HashMap<IpAddr, VecDeque<Instant>>,
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

/// A token bucket: `burst` at once, then one per `every`.
pub(crate) struct Bucket {
    tokens: f64,
    at: Instant,
    burst: f64,
    every: Duration,
}

impl Bucket {
    fn new(now: Instant, burst: f64, every: Duration) -> Bucket {
        Bucket { tokens: burst, at: now, burst, every }
    }

    /// Spend one token if there is one.
    pub fn take(&mut self, now: Instant) -> bool {
        let refill = now.saturating_duration_since(self.at).as_secs_f64() / self.every.as_secs_f64();
        self.tokens = (self.tokens + refill).min(self.burst);
        self.at = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Renames and asks: 2 at once, then one per 2 s.
const ASK_BURST: f64 = 2.0;
const ASK_EVERY: Duration = Duration::from_secs(2);
/// Steering (control mode): up to 30 messages at once, then 30 per second.
const STEER_BURST: f64 = 30.0;
const STEER_EVERY: Duration = Duration::from_micros(33_334);
/// Thrown objects: up to 50 at once (five volleys), then 50 per minute.
const TOSS_BURST: f64 = 50.0;
const TOSS_EVERY: Duration = Duration::from_millis(1200);
/// Visit messages: far more than a pet walking back and forth ever sends.
const VISIT_BURST: f64 = 20.0;
const VISIT_EVERY: Duration = Duration::from_millis(500);
/// Ladder messages: a game takes a handful per friend.
const LADDER_BURST: f64 = 20.0;
const LADDER_EVERY: Duration = Duration::from_millis(500);
/// Friend lists between my computers and introductions: a few on each
/// connect or change.
const LINKS_BURST: f64 = 6.0;
const LINKS_EVERY: Duration = Duration::from_secs(5);
/// Linking by code (two messages per try; a code allows `devices::CODE_TRIES`)
/// and the secret proof on each connect.
const CODES_BURST: f64 = 16.0;
const CODES_EVERY: Duration = Duration::from_secs(2);
/// 📱 Shared pet and care: the sender coalesces to one per
/// `Timing::device_state_every` (plus a claim's handover), so a few at once.
const STATES_BURST: f64 = 4.0;
const STATES_EVERY: Duration = Duration::from_secs(1);

/// What one friend may make us process (over budget = dropped, the link stays:
/// a flooding friend can't spam the app with events). Kept per friend, not per
/// link: a reconnect continues with the tokens that were left. Each kind has
/// its own bucket, so a burst of one never starves another.
pub(crate) struct Budget {
    pub asks: Bucket,
    pub steers: Bucket,
    pub tosses: Bucket,
    pub visits: Bucket,
    pub ladder: Bucket,
    pub links: Bucket,
    /// `device_*` messages (linking by code, proving the stored secret).
    pub codes: Bucket,
    /// `device_state` (its own bucket: shared pets never starve a claim).
    pub states: Bucket,
}

impl Budget {
    /// Spend on a `device_*` message (`state` = a `device_state`).
    pub fn device(&mut self, state: bool, now: Instant) -> bool {
        if state {
            self.states.take(now)
        } else {
            self.codes.take(now)
        }
    }

    fn new(now: Instant) -> Budget {
        Budget {
            asks: Bucket::new(now, ASK_BURST, ASK_EVERY),
            steers: Bucket::new(now, STEER_BURST, STEER_EVERY),
            tosses: Bucket::new(now, TOSS_BURST, TOSS_EVERY),
            visits: Bucket::new(now, VISIT_BURST, VISIT_EVERY),
            ladder: Bucket::new(now, LADDER_BURST, LADDER_EVERY),
            links: Bucket::new(now, LINKS_BURST, LINKS_EVERY),
            codes: Bucket::new(now, CODES_BURST, CODES_EVERY),
            states: Bucket::new(now, STATES_BURST, STATES_EVERY),
        }
    }
}

/// One live link (after hello). Several stranger links may claim the same id;
/// at most one friend link per id (see `State::friend_links`).
pub(crate) struct Conn {
    pub peer_id: String,
    pub link: Arc<secure::SecureLink>,
    /// Last display name heard on this link (hello, then renames).
    pub name: String,
    /// Protocol version from the peer's hello (`PROTOCOL_VERSION` of its build).
    pub v: u32,
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
    /// Linking by code / proving the stored secret on this link.
    pub dev: devices::DevLink,
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
    /// When each friend paired in this run was pinned (see `just_paired`).
    pub paired_at: HashMap<String, Instant>,
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
    /// New inbound connections per source address (see `Shared::allow_inbound`).
    pub conn_rate: HashMap<String, (Instant, u32)>,
    /// Announcements from a trusted (id, source IP), own finite budget.
    pub trusted_rate: HashMap<(String, IpAddr), (Instant, u32)>,
    /// Friend id → last attempt to move its relay link onto the LAN.
    pub lan_upgrade: HashMap<String, Instant>,
    /// Friend id → when its owed `Disconnected` is emitted. A friend link
    /// that ends is reported only after a grace (`conn_timeout`); a new link
    /// registering in time (e.g. relay ↔ LAN switch, quick restart) takes
    /// over silently — no Disconnected/Connected pair.
    pub pending_gone: HashMap<String, Instant>,
    /// Friend id → what it may still make us process. Only ever holds
    /// authenticated friends, and leaves with the friend.
    pub budgets: HashMap<String, Budget>,
    /// My linked other computer and our secret (at most one: two computers).
    pub device: Option<(String, [u8; 32])>,
    /// The link code I am showing (for one friend).
    pub link_code: Option<devices::LinkCode>,
    /// Introduced device id → the friend that introduced it.
    pub via: HashMap<String, String>,
    /// Friends added without pairing, not yet seen knowing us → when they
    /// were added (unix seconds; see `devices::expire_pending`).
    pub pending: HashMap<String, u64>,
    /// Unconfirmed friend → authenticated hellos saying it doesn't know us.
    pub pending_denied: HashMap<String, u32>,
    /// 📱 Who plays, as far as I know: (term, install id) — the highest wins
    /// (see `devices::tick_active`). Starts as (0, me).
    pub active: (u64, String),
    /// When the user last used this computer (`Net::set_idle`).
    pub last_input: Option<Instant>,
    /// Last time the link to my linked computer was up and proven.
    pub device_seen: Option<Instant>,
    /// As last reported (`NetEvent::Standby`): this computer / my linked one is on standby.
    pub standby: bool,
    pub device_standby: bool,
    /// When this computer went on standby (friend links close after the grace).
    pub standby_since: Option<Instant>,
    /// 📱 The latest selected pet and care shared between my two computers:
    /// mine (`Net::share_state`) or the one my linked computer shared.
    pub dstate: Option<devices::SharedState>,
    /// It changed since it was last sent (sent within `Timing::device_state_every`).
    pub dstate_dirty: bool,
    /// I changed it here since I last took one from my linked computer (a
    /// handover then does not overwrite it).
    pub dstate_edited: bool,
    /// Last sent: (link token, term I played in, when).
    pub dstate_sent: Option<(u64, u64, Instant)>,
    /// I just claimed the play: my linked computer's handover may come until then.
    pub handover_until: Option<Instant>,
}

impl State {
    /// Friend `id`'s budget (only call for an authenticated friend).
    pub fn budget(&mut self, id: &str, now: Instant) -> &mut Budget {
        self.budgets.entry(id.to_string()).or_insert_with(|| Budget::new(now))
    }

    pub fn is_friend(&self, id: &str) -> bool {
        self.friends.iter().any(|f| f.id == id)
    }
    /// Only introduced by a friend and never seen knowing us: its key is that
    /// friend's word alone, so a real (SAS-confirmed) pairing may replace it.
    /// Only a friend's introduction vouches for this id's key (confirmed or not:
    /// whoever holds that key can confirm itself), so a real SAS-confirmed
    /// pairing may still replace it.
    pub fn introduced_only(&self, id: &str) -> bool {
        self.via.contains_key(id)
    }
    pub fn friend_key(&self, id: &str) -> Option<[u8; 32]> {
        self.friends.iter().find(|f| f.id == id).map(|f| f.public_key)
    }
    pub fn add_friend(&mut self, id: &str, key: [u8; 32]) {
        self.friends.retain(|f| f.id != id && f.public_key != key);
        self.friends.push(Friend { id: id.to_string(), public_key: key });
    }
    /// Was this friend pinned so recently that a hello sent before the peer
    /// pinned us could still arrive? (Hello goes out at session start, and a
    /// session has PRE_AUTH_MAX to deliver it; both sides pin about together.)
    pub fn just_paired(&self, id: &str, now: Instant) -> bool {
        self.paired_at.get(id).is_some_and(|t| now.duration_since(*t) < PRE_AUTH_MAX * 2)
    }
    /// Turn the friend link of a no-longer-friend into a plain link, for the
    /// caller to close (`announced` stays: Disconnected follows).
    pub fn demote_friend_link(&mut self, id: &str) -> Option<Arc<secure::SecureLink>> {
        let token = self.friend_links.remove(id)?;
        self.conns.get_mut(&token).map(|c| {
            c.kind = Kind::Stranger(None);
            c.link.clone()
        })
    }
    pub fn remove_friend(&mut self, id: &str) {
        self.friends.retain(|f| f.id != id);
        self.paired_at.remove(id);
        self.budgets.remove(id);
        if self.device.as_ref().is_some_and(|(d, _)| d == id) {
            self.device = None; // unfriending also unlinks
        }
        if self.link_code.as_ref().is_some_and(|c| c.friend == id) {
            self.link_code = None;
        }
        self.via.remove(id);
        self.pending.remove(id);
        self.pending_denied.remove(id);
        // Its address is no reason for trust any more
        self.trusted_ips.retain(|(friend, _), _| friend != id);
        self.trusted_rate.retain(|(friend, _), _| friend != id);
        self.friend_rate.retain(|(friend, _), _| friend != id);
    }
    /// 📱 On standby, only my linked computer may have a link.
    pub fn standby_blocks(&self, id: &str) -> bool {
        self.standby && !self.device.as_ref().is_some_and(|(d, _)| d == id)
    }
    /// 📱 On standby, a link from a friend's key (not my linked computer's) is closed before hello.
    pub fn standby_refuses_key(&self, key: &[u8; 32]) -> bool {
        self.standby && self.friends.iter().any(|f| f.public_key == *key && self.standby_blocks(&f.id))
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
    /// Display name (changes with [`Net::set_name`]).
    name: Mutex<String>,
    pub static_key: [u8; 32],
    /// Public half of `static_key`, computed once: deriving it costs a
    /// scalar multiplication, too slow to repeat per message under the state lock.
    pub public_key: [u8; 32],
    pub timing: Timing,
    pub tcp_port: u16,
    /// Relay transport, if configured.
    pub relay: Option<Arc<relay::RelayClient>>,
    pub stop: AtomicBool,
    pub state: Mutex<State>,
    events: Sender<NetEvent>,
    threads: Mutex<Vec<JoinHandle<()>>>,
    next_token: AtomicU64,
    unverified: Mutex<Vec<Unverified>>,
    /// Inbound session threads before their hello, per `handshake_class`
    /// (`MAX_HANDSHAKES` each): strangers can only fill their own class.
    handshakes: [AtomicUsize; 3],
}

/// Which `Shared::handshakes` counter a link counts against: a stranger on the
/// LAN, a stranger through the relay, or an address a friend authenticated from.
fn handshake_class(addr: &str, trusted: bool) -> usize {
    match (trusted, addr.starts_with(relay::ADDR_PREFIX)) {
        (true, _) => 2,
        (false, true) => 1,
        (false, false) => 0,
    }
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
    /// Set while a person is comparing the emoji on this link (see `SessionGuard`).
    surfaced: Arc<AtomicBool>,
}

/// Holds an unverified-inbound slot (if any) until released or dropped.
pub(crate) struct SessionGuard {
    shared: Arc<Shared>,
    pub unverified: Arc<AtomicBool>,
    /// The link shows a pairing to the user: not evicted and not counted in
    /// the pool (those are bounded by `MAX_INVITES_IN`), so a flood can't end a
    /// pairing while two people compare emoji.
    pub surfaced: Arc<AtomicBool>,
    /// Counts against `MAX_HANDSHAKES` while set.
    handshaking: AtomicBool,
    class: usize,
}

impl SessionGuard {
    /// The peer's hello arrived: this thread no longer counts as a handshake.
    pub fn handshake_done(&self) {
        if self.handshaking.swap(false, Ordering::SeqCst) {
            self.shared.handshakes[self.class].fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.handshake_done();
        self.shared.release_unverified(&self.unverified);
    }
}

impl Shared {
    /// Current display name (sent in `hello` and announcements).
    pub fn name(&self) -> String {
        self.name.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn emit(&self, ev: NetEvent) {
        let _ = self.events.send(ev);
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
    /// May a new inbound connection from `addr` get a session at all? A flood
    /// from one address burns its own budget; friends elsewhere are untouched,
    /// and an address a friend authenticated from has a larger one.
    pub fn allow_inbound(&self, addr: &str, now: Instant) -> bool {
        let mut st = self.state();
        let limit = if st.ip_trusted(addr) { MAX_CONNS_PER_TRUSTED_ADDR } else { MAX_CONNS_PER_ADDR };
        discovery::rate_allow(&mut st.conn_rate, addr.to_string(), now, limit)
    }
    /// Admit an inbound link into the unverified pool, evicting the oldest
    /// unverified link from the same address (if it has `MAX_UNVERIFIED_PER_IP`)
    /// or overall (if the pool is full), so a flood cannot lock friends out; it
    /// can only churn other unverified links. `None` only when `MAX_HANDSHAKES`
    /// threads of the link's own class are already handshaking: the link is
    /// dropped and the peer retries. A stranger flood on the LAN therefore never
    /// refuses a link through the relay or from an address a friend used.
    ///
    /// Links from an address that previously authenticated as a friend use a
    /// separate per-address allowance, so strangers can never crowd them out.
    pub fn admit_inbound(self: &Arc<Self>, frames: Arc<dyn transport::FrameLink>) -> Option<SessionGuard> {
        let ip = frames.peer_addr();
        let trusted = self.state().ip_trusted(&ip);
        let class = handshake_class(&ip, trusted);
        if self.handshakes[class].fetch_add(1, Ordering::SeqCst) >= MAX_HANDSHAKES {
            self.handshakes[class].fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        let active = Arc::new(AtomicBool::new(true));
        let surfaced = Arc::new(AtomicBool::new(false));
        let mut evicted = Vec::new();
        {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
            let mut evict_oldest = |pool: &mut Vec<Unverified>, same_ip: bool| {
                let victim = pool
                    .iter()
                    .enumerate()
                    .filter(|(_, u)| u.trusted == trusted && !u.surfaced.load(Ordering::SeqCst) && (!same_ip || u.ip == ip))
                    .min_by_key(|(_, u)| u.since)
                    .map(|(i, _)| i);
                if let Some(i) = victim {
                    let u = pool.swap_remove(i);
                    u.active.store(false, Ordering::SeqCst);
                    evicted.push(u.frames);
                }
            };
            let counted = |u: &&Unverified| u.trusted == trusted && !u.surfaced.load(Ordering::SeqCst);
            let same_ip = pool.iter().filter(counted).filter(|u| u.ip == ip).count();
            if trusted {
                if same_ip >= MAX_TRUSTED_PER_IP {
                    evict_oldest(&mut pool, true);
                } else if pool.iter().filter(counted).count() >= MAX_TRUSTED_INBOUND {
                    evict_oldest(&mut pool, false);
                }
            } else if same_ip >= MAX_UNVERIFIED_PER_IP {
                evict_oldest(&mut pool, true);
            } else if pool.iter().filter(counted).count() >= MAX_UNVERIFIED_INBOUND {
                evict_oldest(&mut pool, false);
            }
            pool.push(Unverified { ip, trusted, since: Instant::now(), frames, active: active.clone(), surfaced: surfaced.clone() });
        }
        for f in evicted {
            f.close();
        }
        Some(SessionGuard { shared: self.clone(), unverified: active, surfaced, handshaking: AtomicBool::new(true), class })
    }
    /// Outgoing links hold no unverified slot.
    pub fn outbound_guard(self: &Arc<Self>) -> SessionGuard {
        SessionGuard { shared: self.clone(), unverified: Arc::new(AtomicBool::new(false)), surfaced: Arc::new(AtomicBool::new(false)), handshaking: AtomicBool::new(false), class: 0 }
    }
    /// The link proved to be a friend (or ended): free its unverified slot.
    pub fn release_unverified(&self, flag: &AtomicBool) {
        if flag.swap(false, Ordering::SeqCst) {
            let mut pool = self.unverified.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|u| u.active.load(Ordering::SeqCst));
        }
    }
}

fn discovery_socket(port: u16) -> std::io::Result<UdpSocket> {
    let udp = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port)))?;
    udp.set_broadcast(true)?;
    udp.set_read_timeout(Some(TICK))?;
    Ok(udp)
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
        // The discovery port can be taken by another program: friends through
        // the relay don't need it. Without a relay nothing could work.
        let udp = match discovery_socket(cfg.discovery_port) {
            Ok(u) => Some(u),
            Err(e) if cfg.relay_url.as_deref().is_some_and(|u| !u.is_empty()) => {
                eprintln!("screenpet discovery off (port {}): {e}; relay only", cfg.discovery_port);
                None
            }
            Err(e) => return Err(e),
        };
        let discovery_port = udp.as_ref().and_then(|u| u.local_addr().ok()).map_or(cfg.discovery_port, |a| a.port());
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
        if let Some((id, secret)) = cfg.device.as_ref().filter(|(id, _)| state.is_friend(id)) {
            state.device = Some((id.clone(), secret.0));
        }
        for (id, via) in &cfg.introduced {
            let mine = state.device.as_ref().is_some_and(|(d, _)| d == id);
            if id != via && state.is_friend(id) && state.is_friend(via) && !mine {
                state.via.insert(id.clone(), via.clone());
            }
        }
        state.active = (0, cfg.id.clone());
        state.last_input = Some(Instant::now());
        let now = devices::unix_now();
        for id in &cfg.pending {
            if state.is_friend(id) {
                let added = cfg.pending_added.iter().find(|(p, _)| p == id).map_or(now, |(_, t)| (*t).min(now));
                state.pending.insert(id.clone(), added);
            }
        }
        let shared = Arc::new(Shared {
            id: cfg.id,
            name: Mutex::new(protocol::clamp_name(&cfg.name)),
            static_key: cfg.static_key,
            public_key: own_public,
            timing: cfg.timing,
            tcp_port,
            relay: cfg.relay_url.filter(|u| !u.is_empty()).map(|u| relay::RelayClient::new(u, cfg.relay_key)),
            stop: AtomicBool::new(false),
            state: Mutex::new(state),
            events: tx,
            threads: Mutex::new(Vec::new()),
            next_token: AtomicU64::new(1),
            unverified: Mutex::new(Vec::new()),
            handshakes: Default::default(),
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

    /// Ask the relay for a new friend code (e.g. after sharing the old one
    /// too widely). `false` if not connected. Expect `FriendCode`.
    pub fn new_friend_code(&self) -> bool {
        relay::new_code(&self.shared)
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

    /// The link of a connected friend (not a pairing, not someone else).
    fn friend_link(&self, peer_id: &str) -> Option<Arc<secure::SecureLink>> {
        let st = self.shared.state();
        st.friend_conn(peer_id).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.link.clone())
    }

    /// Queue a visit message to a connected friend (dropped otherwise).
    /// Never blocks.
    pub fn send(&self, peer_id: &str, msg: Visit) {
        if let Some(link) = self.friend_link(peer_id) {
            link.send(&Message::Visit(msg));
        }
    }

    /// Queue a ladder message to a connected friend, only if its build knows
    /// ladder messages (an older one would drop the link on the unknown
    /// `type`). `false` = not sent. Never blocks.
    pub fn send_ladder(&self, peer_id: &str, msg: LadderMsg) -> bool {
        let old = self.peer_version(peer_id).is_none_or(|v| v < LADDER_MIN_VERSION);
        match self.friend_link(peer_id) {
            Some(link) if !old => link.send(&Message::Ladder(msg)),
            _ => false,
        }
    }

    /// Protocol version of a connected friend's build (`None` if not connected).
    pub fn peer_version(&self, peer_id: &str) -> Option<u32> {
        let st = self.shared.state();
        st.friend_conn(peer_id).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.v)
    }

    /// Send an app request (e.g. "invite", "busy") to a connected friend
    /// (dropped otherwise). Carried in a ping, which older versions ignore.
    /// Never blocks.
    pub fn ask(&self, peer_id: &str, ask: &str) {
        self.ping(peer_id, Some(protocol::clamp_name(ask)), None, None);
    }

    /// Steer our pet visiting a connected friend (control mode; dropped
    /// otherwise). Carried in a ping, which older versions ignore. Never blocks.
    pub fn steer(&self, peer_id: &str, steer: Steer) {
        self.ping(peer_id, None, Some(steer), None);
    }

    /// Throw an object onto a connected friend's screen (dropped otherwise).
    /// Carried in a ping, which older versions ignore. Never blocks.
    pub fn toss(&self, peer_id: &str, toss: Toss) {
        self.ping(peer_id, None, None, Some(toss));
    }

    fn ping(&self, peer_id: &str, ask: Option<String>, steer: Option<Steer>, toss: Option<Toss>) {
        if let Some(link) = self.friend_link(peer_id) {
            link.send(&Message::Control(Control::Ping { ask, steer, toss }));
        }
    }

    /// Forget a friend, tell them, and disconnect (gracefully). The devices it
    /// introduced go with it ([`NetEvent::FriendDropped`]). Never blocks.
    pub fn unfriend(&self, peer_id: &str) {
        let (link, dropped) = {
            let mut st = self.shared.state();
            st.remove_friend(peer_id);
            st.invites_out.remove(peer_id);
            let dropped = devices::drop_introduced(&self.shared, &mut st, peer_id);
            (st.demote_friend_link(peer_id), dropped)
        };
        if let Some(l) = link {
            l.send(&Message::Control(Control::Unfriend));
            l.finish();
        }
        devices::close_dropped(dropped);
    }

    /// Show a fresh link code for friend `id` (`ABCD-EFGH`; valid 10 minutes,
    /// single use, gone after 5 wrong tries; replaces any earlier code). The
    /// user types it on that other computer ([`Net::enter_link_code`]); then
    /// both get [`NetEvent::DeviceLinked`]. Refused if I am linked to another
    /// computer (at most two computers).
    pub fn new_link_code(&self, id: &str) -> Result<String, LinkError> {
        let mut st = self.shared.state();
        if !st.is_friend(id) {
            return Err(LinkError::NotFriend);
        }
        if let Some((other, _)) = st.device.as_ref().filter(|(d, _)| d != id) {
            return Err(LinkError::Taken(other.clone()));
        }
        let (shown, code) = devices::new_code(id);
        st.link_code = Some(code);
        Ok(shown)
    }

    /// The user typed the code friend `id`'s computer is showing for us.
    /// Proves it over the friend link without sending it; expect
    /// [`NetEvent::DeviceLinked`] or [`NetEvent::DeviceLinkFailed`]. Never blocks.
    pub fn enter_link_code(&self, id: &str, code: &str) -> Result<(), LinkError> {
        let key = devices::typed_code_key(code).ok_or(LinkError::Invalid)?;
        let mut st = self.shared.state();
        if !st.is_friend(id) {
            return Err(LinkError::NotFriend);
        }
        if let Some((other, _)) = st.device.as_ref().filter(|(d, _)| d != id) {
            return Err(LinkError::Taken(other.clone()));
        }
        let Some((token, v, link)) = st.friend_links.get(id).and_then(|t| st.conns.get(t).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| (*t, c.v, c.link.clone()))) else {
            return Err(LinkError::Offline);
        };
        if v < DEVICES_MIN_VERSION {
            return Err(LinkError::Old);
        }
        let msg = devices::start_entering(&mut st, token, key).ok_or(LinkError::Offline)?;
        drop(st);
        link.send(&Message::Control(msg));
        Ok(())
    }

    /// Unlink my other computer `id`: forget our secret and tell it (it
    /// forgets too). It stays a friend. `false` if it was not linked.
    pub fn unlink_device(&self, id: &str) -> bool {
        {
            let mut st = self.shared.state();
            if !st.device.as_ref().is_some_and(|(d, _)| d == id) {
                return false;
            }
            st.device = None;
            if let Some(c) = st.friend_links.get(id).copied().and_then(|t| st.conns.get_mut(&t)) {
                c.dev.proven = false;
            }
        }
        if let Some(link) = self.devices_link(id) {
            link.send(&Message::Control(Control::DeviceUnlink));
        }
        true
    }

    /// The user's answer to [`NetEvent::DeviceConfirm`] for friend `id`
    /// (`true` = 확인, `false` = 취소 or the window closed). `false` if
    /// nothing waits for an answer from that friend.
    pub fn confirm_device(&self, id: &str, yes: bool) -> bool {
        let told = devices::answer(&self.shared, &mut self.shared.state(), id, yes);
        let Some((link, msg)) = told else { return false };
        link.send(&Message::Control(msg));
        true
    }

    /// 📱 Play on this computer: my linked computer goes on standby (told over
    /// our proven link). Also counts as the user using this computer. `false`
    /// if that link is not up (then the two can't reach each other and both
    /// play anyway; when they meet again the one used last plays).
    pub fn claim_active(&self) -> bool {
        let Some((link, msg)) = devices::claim(&self.shared) else { return false };
        link.send(&Message::Control(msg))
    }

    /// 📱 How long ago the user last used this computer (the app reports it
    /// regularly). Decides who plays when my two computers meet again.
    pub fn set_idle(&self, idle: Duration) {
        let now = Instant::now();
        self.shared.state().last_input = Some(now.checked_sub(idle).unwrap_or(now));
    }

    /// 📱 My selected pet and every pet's care changed (or the regular
    /// refresh): shared with my linked computer while this one plays
    /// (coalesced, `Timing::device_state_every`). Changing it while on
    /// standby counts as playing here only if the app also claims.
    pub fn share_state(&self, pet: &str, care: Vec<WireCare>) {
        devices::share(&mut self.shared.state(), pet, care);
    }

    /// 📱 This computer is on standby: my other computer plays.
    pub fn standby(&self) -> bool {
        self.shared.state().standby
    }

    /// 📱 My linked computer is on standby: this one plays (don't send it my pet).
    pub fn device_standby(&self) -> bool {
        self.shared.state().device_standby
    }

    /// My linked other computer, if any.
    pub fn linked_device(&self) -> Option<String> {
        self.shared.state().device.as_ref().map(|(d, _)| d.clone())
    }

    /// The secret shared with my linked computer `id` (to persist), if linked.
    pub fn device_secret(&self, id: &str) -> Option<DeviceSecret> {
        self.shared.state().device.as_ref().filter(|(d, _)| d == id).map(|(_, s)| DeviceSecret(*s))
    }

    /// Device marks and origin of friend `id` (`None` if no friend).
    pub fn friend_info(&self, id: &str) -> Option<FriendInfo> {
        let st = self.shared.state();
        st.is_friend(id).then(|| FriendInfo { device: st.device.as_ref().is_some_and(|(d, _)| d == id), via: st.via.get(id).cloned(), pending: st.pending.contains_key(id), pending_since: st.pending.get(id).copied() })
    }

    /// The link of a connected friend whose build knows the device messages.
    fn devices_link(&self, peer_id: &str) -> Option<Arc<secure::SecureLink>> {
        let new = self.peer_version(peer_id).is_some_and(|v| v >= DEVICES_MIN_VERSION);
        self.friend_link(peer_id).filter(|_| new)
    }

    /// Send my friends to `peer_id` if it is my linked computer and proved our
    /// secret on its current link (otherwise `false`, nothing sent). Introduced and
    /// unconfirmed friends are left out (they come from their own source), and
    /// so is the receiver. `describe(id)` gives each friend's name and side
    /// (first values for the receiver). `want` asks for its list back. Never blocks.
    pub fn send_device_friends(&self, peer_id: &str, want: bool, describe: impl Fn(&str) -> (String, Option<String>)) -> bool {
        let proven = {
            let st = self.shared.state();
            st.device.as_ref().is_some_and(|(d, _)| d == peer_id) && st.friend_conn(peer_id).is_some_and(|c| c.dev.proven)
        };
        if !proven {
            return false;
        }
        let Some(link) = self.devices_link(peer_id) else { return false };
        let ids: Vec<(String, [u8; 32])> = {
            let st = self.shared.state();
            st.friends
                .iter()
                .filter(|f| f.id != peer_id && !st.via.contains_key(&f.id) && !st.pending.contains_key(&f.id))
                .take(MAX_FRIENDS)
                .map(|f| (f.id.clone(), f.public_key))
                .collect()
        };
        let friends = ids
            .into_iter()
            .map(|(id, key)| {
                let (name, side) = describe(&id);
                protocol::WireFriend { key: to_hex(&key), name: protocol::clamp_name(&name), side, id }
            })
            .collect();
        link.send(&Message::Control(Control::DeviceFriends { friends, want }))
    }

    /// Introduce my linked computer (`MAX_DEVICES`) to friend `peer_id`,
    /// unless it is one of them or its build does not know the message. `false`
    /// = nothing sent (also when I have no other computer). Never blocks.
    pub fn send_my_devices(&self, peer_id: &str) -> bool {
        let devices: Vec<protocol::WireDevice> = {
            let st = self.shared.state();
            let mine = st.device.as_ref().map(|(d, _)| d.as_str());
            if mine == Some(peer_id) {
                return false;
            }
            st.friends
                .iter()
                .filter(|f| f.id != peer_id && mine == Some(f.id.as_str()) && !st.pending.contains_key(&f.id))
                .take(MAX_DEVICES)
                .map(|f| protocol::WireDevice { id: f.id.clone(), key: to_hex(&f.public_key) })
                .collect()
        };
        if devices.is_empty() {
            return false;
        }
        match self.devices_link(peer_id) {
            Some(link) => link.send(&Message::Control(Control::MyDevices { devices })),
            None => false,
        }
    }

    /// Change our display name: later `hello`s and announcements use it, and
    /// every connected friend is told now (a repeated `hello` on the friend
    /// link — friends emit [`NetEvent::Renamed`]; older versions ignore it).
    /// Never blocks.
    pub fn set_name(&self, name: &str) {
        let name = protocol::clamp_name(name);
        *self.shared.name.lock().unwrap_or_else(|e| e.into_inner()) = name.clone();
        let links: Vec<_> = {
            let st = self.shared.state();
            st.friend_links.values().filter_map(|t| st.conns.get(t)).filter(|c| matches!(c.kind, Kind::Friend)).map(|c| c.link.clone()).collect()
        };
        let hello = Message::Control(Control::Hello { id: self.shared.id.clone(), name, v: protocol::PROTOCOL_VERSION, friend: true });
        for l in links {
            l.send(&hello);
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

/// A running Net that hears nobody, for unit tests of its parts.
#[cfg(test)]
pub(crate) fn solo_net(friends: Vec<Friend>) -> Net {
    let mut cfg = NetConfig::new("solo", "Solo", Net::generate_static_key(), friends);
    cfg.discovery_port = 0;
    cfg.broadcast_to = SocketAddr::from(([127, 0, 0, 1], 9));
    Net::start(cfg).unwrap().0
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn the_budget_is_per_friend_and_leaves_with_the_friend() {
        let now = Instant::now();
        let mut st = State::default();
        for _ in 0..50 {
            assert!(st.budget("a", now).tosses.take(now));
        }
        assert!(!st.budget("a", now).tosses.take(now));
        // 다른 친구의 몫은 따로, 다른 종류의 몫도 따로
        assert!(st.budget("b", now).tosses.take(now));
        assert!(st.budget("a", now).visits.take(now));
        // 시간이 지나면 다시 찬다
        assert!(st.budget("a", now).tosses.take(now + TOSS_EVERY));
        st.remove_friend("a");
        assert!(!st.budgets.contains_key("a"));
        assert!(st.budgets.contains_key("b"));
    }
}

#[cfg(test)]
mod inbound_tests {
    use super::*;

    struct NoLink;
    impl transport::FrameLink for NoLink {
        fn send_frame(&self, _: &[u8]) -> bool {
            true
        }
        fn finish(&self) {}
        fn close(&self) {}
        fn peer_addr(&self) -> String {
            "10.0.0.1".into()
        }
    }

    #[test]
    fn handshake_threads_are_capped_until_one_says_hello() {
        let net = solo_net(vec![]);
        let link = || -> Arc<dyn transport::FrameLink> { Arc::new(NoLink) };
        let mut held: Vec<_> = (0..MAX_HANDSHAKES).map(|_| net.shared.admit_inbound(link()).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(link()).is_none(), "the cap is reached");
        // 하나가 끝나면 자리가 난다
        held.pop();
        let again = net.shared.admit_inbound(link()).expect("a slot came free");
        assert!(net.shared.admit_inbound(link()).is_none());
        // hello까지 온 링크는 세지 않는다 (끝날 때 두 번 빼지도 않는다)
        again.handshake_done();
        let next = net.shared.admit_inbound(link()).expect("hello frees the slot");
        drop(again);
        assert!(net.shared.admit_inbound(link()).is_none(), "released once");
        drop(next);
        drop(held);
        assert_eq!(net.shared.handshakes[0].load(Ordering::SeqCst), 0);
    }

    struct At(String);
    impl transport::FrameLink for At {
        fn send_frame(&self, _: &[u8]) -> bool {
            true
        }
        fn finish(&self) {}
        fn close(&self) {}
        fn peer_addr(&self) -> String {
            self.0.clone()
        }
    }

    fn at(addr: &str) -> Arc<dyn transport::FrameLink> {
        Arc::new(At(addr.to_string()))
    }

    #[test]
    fn a_stranger_flood_never_refuses_a_trusted_or_relay_link() {
        let net = solo_net(vec![]);
        net.shared.state().trust_ip("friend", "10.0.0.9", Instant::now());
        // LAN의 낯선 주소들이 핸드셰이크 자리를 다 채운다
        let flood: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("10.1.0.{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("10.1.1.1")).is_none(), "strangers are at their cap");
        assert!(net.shared.admit_inbound(at("10.0.0.9")).is_some(), "a friend's address has its own");
        assert!(net.shared.admit_inbound(at("relay:abcd")).is_some(), "the relay has its own");
        drop(flood);
    }

    #[test]
    fn relay_strangers_do_not_use_up_lan_handshakes() {
        let net = solo_net(vec![]);
        let flood: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("relay:{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("relay:x")).is_none());
        assert!(net.shared.admit_inbound(at("10.1.1.1")).is_some(), "LAN is untouched");
        drop(flood);
    }

    #[test]
    fn trusted_handshakes_and_pool_entries_are_capped_too() {
        let net = solo_net(vec![]);
        let now = Instant::now();
        for i in 0..MAX_TRUSTED_IPS {
            net.shared.state().trust_ip("friend", &format!("10.2.0.{i}"), now);
        }
        let held: Vec<_> = (0..MAX_HANDSHAKES).map(|i| net.shared.admit_inbound(at(&format!("10.2.0.{i}"))).expect("under the cap")).collect();
        assert!(net.shared.admit_inbound(at("10.2.0.40")).is_none(), "trusted links are bounded as well");
        drop(held);
        // 풀에도 신뢰 주소 전체의 상한이 있다 (오래된 것이 밀려난다)
        let mut held = Vec::new();
        for i in 0..MAX_TRUSTED_IPS {
            let g = net.shared.admit_inbound(at(&format!("10.2.0.{i}"))).expect("slot");
            g.handshake_done();
            held.push(g);
        }
        let live = net.shared.unverified.lock().unwrap().iter().filter(|u| u.trusted && u.active.load(Ordering::SeqCst)).count();
        assert!(live <= MAX_TRUSTED_INBOUND, "{live}");
    }

    #[test]
    fn removing_a_friend_forgets_where_it_authenticated_from() {
        let now = Instant::now();
        let ip: IpAddr = "10.0.0.5".parse().unwrap();
        let mut st = State::default();
        st.add_friend("a", [1; 32]);
        st.add_friend("b", [2; 32]);
        st.trust_ip("a", "10.0.0.5", now);
        st.trust_ip("b", "10.0.0.6", now);
        st.trusted_rate.insert(("a".into(), ip), (now, 1));
        st.friend_rate.insert(("a".into(), ip), (now, 1));
        st.remove_friend("a");
        assert!(!st.ip_trusted("10.0.0.5"), "no more trust from the removed friend's address");
        assert!(st.ip_trusted("10.0.0.6"), "other friends stay");
        assert!(st.trusted_rate.is_empty() && st.friend_rate.is_empty());
    }

    #[test]
    fn each_address_has_its_own_connection_budget_and_a_friends_is_larger() {
        let net = solo_net(vec![]);
        let now = Instant::now();
        let allow = |addr: &str| net.shared.allow_inbound(addr, now);
        assert_eq!((0..200).filter(|_| allow("10.0.0.1")).count(), MAX_CONNS_PER_ADDR as usize);
        assert!(allow("10.0.0.2"), "another address is untouched");
        net.shared.state().trust_ip("friend", "10.0.0.3", now);
        assert_eq!((0..200).filter(|_| allow("10.0.0.3")).count(), MAX_CONNS_PER_TRUSTED_ADDR as usize);
        assert!(net.shared.allow_inbound("10.0.0.1", now + Duration::from_secs(1)), "the budget refills");
    }
}
