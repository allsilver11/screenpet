//! Shared relay state: who is online, channels between them, pairing grants
//! from redeemed friend codes, and per-address limits. Connection threads
//! talk to each other only through `Out` queues (never blocking under the
//! hub lock).

use crate::proto::{self, ServerMsg};
use crate::store::{unix_now, Store};
use crate::Limits;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

macro_rules! counters {
    ($($name:ident),* $(,)?) => {
        /// Running totals since start, for `admin status`. No addresses or ids.
        #[derive(Default)]
        pub(crate) struct Counters {
            $(pub $name: AtomicU64,)*
        }

        impl Counters {
            pub fn all(&self) -> Vec<(&'static str, u64)> {
                vec![$((stringify!($name), self.$name.load(Ordering::Relaxed)),)*]
            }
        }
    };
}

counters!(
    conns_admitted,
    conns_refused_limit,
    conns_refused_origin,
    unauthed_evicted,
    threads_refused,
    accept_errors,
    auth_failed,
    registrations,
    registrations_refused,
    store_full,
    kicked_rate,
    kicked_slow,
    redeems_ok,
    redeems_failed,
    channels_closed_rate,
    channels_closed_queue,
    frames_forwarded,
    bytes_forwarded,
    proxy_without_address,
    save_errors,
);

pub(crate) fn bump(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}

/// Something for a connection thread to send.
pub(crate) enum Out {
    Text(String),
    Binary(Vec<u8>),
    /// Send `error{reason}` and disconnect.
    Kick(String),
}

/// Token bucket: `rate` per second, holding at most `burst`.
#[derive(Debug, Clone)]
pub(crate) struct Bucket {
    tokens: f64,
    rate: f64,
    burst: f64,
    last: Instant,
}

impl Default for Bucket {
    fn default() -> Bucket {
        Bucket::new(0.0, 0.0)
    }
}

impl Bucket {
    pub fn new(rate: f64, burst: f64) -> Bucket {
        Bucket { tokens: burst, rate, burst, last: Instant::now() }
    }
    /// Would it be at `burst` by now? Then forgetting it loses nothing.
    fn is_full(&self) -> bool {
        self.tokens + self.last.elapsed().as_secs_f64() * self.rate >= self.burst
    }
    pub fn take(&mut self, n: f64) -> bool {
        let now = Instant::now();
        self.tokens = (self.tokens + now.duration_since(self.last).as_secs_f64() * self.rate).min(self.burst);
        self.last = now;
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            false
        }
    }
}

/// The hub's handle on a connection thread.
#[derive(Clone)]
pub(crate) struct Link {
    pub out: SyncSender<Out>,
    /// Once set, the thread sends this as `error{reason}` and disconnects.
    pub kill: Arc<OnceLock<&'static str>>,
    /// Bytes of friend frames waiting in `out`, and in the queues of its network.
    pub queued: Arc<AtomicUsize>,
    net_queued: Arc<AtomicUsize>,
    /// When `out` last made progress (ms since the hub started).
    drained_at: Arc<AtomicU64>,
}

impl Link {
    pub fn new(out: SyncSender<Out>, admission: &Admission) -> Link {
        Link {
            out,
            kill: admission.kill.clone(),
            queued: Arc::default(),
            net_queued: admission.net_queued.clone(),
            drained_at: Arc::default(),
        }
    }

    /// Disconnect now with `reason`. The queue may be stuck or full of frames,
    /// so `kill` is what makes it immediate.
    fn kick(&self, reason: &'static str) {
        let _ = self.out.try_send(Out::Kick(reason.into()));
        let _ = self.kill.set(reason);
    }
}

struct Peer {
    conn: u64,
    id: String,
    link: Link,
    channels: HashSet<u32>,
}

impl Peer {
    fn send(&self, m: ServerMsg) {
        if self.link.out.try_send(Out::Text(m.to_json())).is_err() {
            let _ = self.link.kill.set("slow");
        }
    }
}

struct Channel {
    /// (install key, connection) of the opener and of the other side.
    ends: [(String, u64); 2],
    /// Frames per second from each end.
    buckets: [Bucket; 2],
}

/// A redeemed friend code: `from` may open one channel to `to` until `until`.
struct Grant {
    from: String,
    to: String,
    until: Instant,
}

/// A connection let in by `admit_ip`.
pub(crate) struct Admission {
    pub conn: u64,
    pub kill: Arc<OnceLock<&'static str>>,
    net_queued: Arc<AtomicUsize>,
}

/// A connection that has not authenticated yet.
struct Unauthed {
    ip: String,
    net: String,
    kill: Arc<OnceLock<&'static str>>,
}

/// Connections, and how many of them have not authenticated yet.
#[derive(Default, Clone, Copy)]
struct Conns {
    total: usize,
    unauthed: usize,
}

impl Conns {
    fn admit(&mut self) {
        self.total += 1;
        self.unauthed += 1;
    }
    fn authed(&mut self) {
        self.unauthed = self.unauthed.saturating_sub(1);
    }
    /// One ended; `true` if none are left.
    fn release(&mut self) -> bool {
        self.total = self.total.saturating_sub(1);
        self.total == 0
    }
}

#[derive(Default)]
struct State {
    store: Option<Store>,
    peers: HashMap<String, Peer>,
    channels: HashMap<u32, Channel>,
    next_ch: u32,
    grants: Vec<Grant>,
    /// Connections per address key, per network (IPv4 /24, IPv6 /48), in all.
    ip_conns: HashMap<String, Conns>,
    net_conns: HashMap<String, Conns>,
    all_conns: Conns,
    /// Not authenticated yet, oldest first (keyed by connection id).
    unauthed: BTreeMap<u64, Unauthed>,
    /// Bytes of friend frames queued per network, while it has connections.
    net_queued: HashMap<String, Arc<AtomicUsize>>,
    /// Forwarded bytes per address key (a hard cap) and per network (a fair
    /// share kept inside the global budget). These outlive connections, so
    /// reconnecting doesn't refill them; `maintain` drops them once refilled.
    ip_bytes: HashMap<String, Bucket>,
    net_bytes: HashMap<String, Bucket>,
    /// Whole relay: the budget (past it only fair shares pass), and the
    /// ceiling for everyone at twice that.
    global_bytes: Bucket,
    global_bytes_hard: Bucket,
    ip_redeems: HashMap<String, VecDeque<Instant>>,
    ip_registrations: HashMap<String, VecDeque<Instant>>,
    /// Per "target-key code": recent distinct requesters (cap per code, so
    /// a new code starts fresh and one requester retrying counts once).
    target_redeems: HashMap<String, VecDeque<(Instant, String)>>,
    /// New codes in `target_redeems` per network: one network can't fill that
    /// fail-closed table and so block pairing for everyone.
    net_new_codes: HashMap<String, VecDeque<Instant>>,
    /// New installs per network (IPv4 /24, IPv6 /48) in the last day.
    net_registrations: HashMap<String, VecDeque<Instant>>,
    /// Well-formed redeems per source address (fair share of the global budget).
    ip_wellformed: HashMap<String, VecDeque<Instant>>,
    /// Whole relay: new installs in the last hour, redeems in the last minute.
    global_registrations: VecDeque<Instant>,
    global_redeems: VecDeque<Instant>,
}

impl State {
    fn store(&mut self) -> &mut Store {
        self.store.as_mut().expect("store present")
    }
    fn store_ref(&self) -> &Store {
        self.store.as_ref().expect("store present")
    }
    /// Online installs that are mutual friends of `key`.
    fn mutual_online(&self, key: &str) -> Vec<String> {
        let Some(inst) = self.store_ref().get(key) else { return Vec::new() };
        inst.friends.iter().filter(|f| self.peers.contains_key(*f) && self.store_ref().mutual(key, f)).cloned().collect()
    }
    /// Connection `conn` stops counting as unauthenticated (it authenticated,
    /// ended or was evicted). `None` if it already did.
    fn no_longer_unauthed(&mut self, conn: u64) -> Option<Unauthed> {
        let u = self.unauthed.remove(&conn)?;
        self.all_conns.authed();
        if let Some(c) = self.ip_conns.get_mut(&u.ip) {
            c.authed();
        }
        if let Some(c) = self.net_conns.get_mut(&u.net) {
            c.authed();
        }
        Some(u)
    }
    /// Disconnect the oldest unauthenticated connection that `pick` accepts.
    /// A real client answers the challenge within a round trip, so the oldest
    /// is the one least likely to be real.
    fn evict_unauthed(&mut self, counters: &Counters, pick: impl Fn(&Unauthed) -> bool) {
        let oldest = self.unauthed.iter().find(|(_, u)| pick(u)).map(|(conn, _)| *conn);
        if let Some(u) = oldest.and_then(|conn| self.no_longer_unauthed(conn)) {
            let _ = u.kill.set("busy");
            bump(&counters.unauthed_evicted);
        }
    }
    /// Remove channel `ch` and tell its other end(s) (`except` = the end that
    /// asked, which needs no notice).
    fn close_channel(&mut self, ch: u32, except: Option<&str>) {
        let Some(c) = self.channels.remove(&ch) else { return };
        for (key, conn) in c.ends {
            if let Some(p) = self.peers.get_mut(&key).filter(|p| p.conn == conn) {
                p.channels.remove(&ch);
                if except != Some(key.as_str()) {
                    p.send(ServerMsg::Close { ch });
                }
            }
        }
    }
}

/// The fair share of the global byte budget each network keeps.
const NET_BYTES_SHARE: f64 = 1.0 / 16.0;
/// Past the total queue cap, a network may still queue this part of it (and
/// everyone stops at twice the cap).
const NET_QUEUE_SHARE: usize = 64;

/// `key`'s bucket in a table of at most `max_keys`. When full, buckets that
/// have refilled go first, then the fullest one.
fn bucket_in<'a>(table: &'a mut HashMap<String, Bucket>, key: &str, rate: f64, burst: f64, max_keys: usize) -> &'a mut Bucket {
    if !table.contains_key(key) && table.len() >= max_keys {
        table.retain(|_, b| !b.is_full());
        while table.len() >= max_keys {
            let fullest = table.iter().max_by(|a, b| a.1.tokens.total_cmp(&b.1.tokens)).map(|(k, _)| k.clone());
            match fullest {
                Some(k) => table.remove(&k),
                None => break,
            };
        }
    }
    table.entry(key.to_string()).or_insert_with(|| Bucket::new(rate, burst))
}

/// A wide network (`wide_bucket`) may add this many networks' worth of codes
/// to the per-code table: /48s are cheap, /32s aren't.
const WIDE_NEW_CODES: usize = 4;

/// Most keys tracked per rate table. A new key arriving at a full table
/// evicts stale keys, else the least recently active one — newcomers are
/// never refused wholesale.
const MAX_RATE_KEYS: usize = 8192;

/// Things with a time stamp in a rate table.
trait Stamped {
    fn at(&self) -> Instant;
}
impl Stamped for Instant {
    fn at(&self) -> Instant {
        *self
    }
}
impl Stamped for (Instant, String) {
    fn at(&self) -> Instant {
        self.0
    }
}

/// Keys one network may create in a rate table (IPv4 addresses per /24,
/// IPv6 /64s per /48); past that its newcomers share the network's key.
const MAX_KEYS_PER_PARENT: usize = 16;

/// The enclosing network of an address key: IPv4 → /24, IPv6 /64 → /48.
fn parent_of(key: &str) -> Option<String> {
    if key.parse::<std::net::Ipv4Addr>().is_ok() {
        return Some(network_bucket(key));
    }
    let prefix = key.strip_suffix("::/64")?;
    let segs: Vec<&str> = prefix.split(':').collect();
    (segs.len() == 4).then(|| format!("{}:{}:{}::/48", segs[0], segs[1], segs[2]))
}

/// Bucket for the redeem fair share: IPv4 /24, IPv6 /48.
fn share_bucket(ip: &str) -> String {
    parent_of(ip).unwrap_or_else(|| ip.to_string())
}

/// A network holder's whole allocation, more or less: IPv4 /16, IPv6 /32.
fn wide_bucket(ip: &str) -> String {
    if let Ok(v4) = ip.parse::<std::net::Ipv4Addr>() {
        let o = v4.octets();
        return format!("{}.{}.0.0/16", o[0], o[1]);
    }
    let mut segs = ip.split(':');
    match (segs.next(), segs.next()) {
        (Some(a), Some(b)) => format!("{a}:{b}::/32"),
        _ => ip.to_string(),
    }
}

/// The key to count `key` under: itself, or its network once that network
/// already has `MAX_KEYS_PER_PARENT` keys in the table.
fn capped_key<V>(table: &HashMap<String, VecDeque<V>>, key: &str) -> String {
    if table.contains_key(key) {
        return key.to_string();
    }
    match parent_of(key) {
        Some(parent) if table.keys().filter(|k| parent_of(k).as_deref() == Some(parent.as_str())).count() >= MAX_KEYS_PER_PARENT => parent,
        _ => key.to_string(),
    }
}

/// Make room for `key` in a table holding at most `max` keys: stale keys
/// first, then the least recently active one.
fn make_room<V: Stamped>(table: &mut HashMap<String, VecDeque<V>>, key: &str, window: Duration, max: usize) {
    if table.contains_key(key) || table.len() < max {
        return;
    }
    prune_table(table, window);
    while table.len() >= max {
        let oldest = table.iter().min_by_key(|(_, q)| q.back().map(Stamped::at)).map(|(k, _)| k.clone());
        match oldest {
            Some(k) => table.remove(&k),
            None => break,
        };
    }
}

/// Drop entries older than `window`.
fn expire(q: &mut VecDeque<Instant>, window: Duration) {
    let now = Instant::now();
    while q.front().is_some_and(|t| now.duration_since(*t) >= window) {
        q.pop_front();
    }
}

/// One sliding window: `true` (and counted) if fewer than `limit` in `window`.
fn allow_window(q: &mut VecDeque<Instant>, limit: usize, window: Duration) -> bool {
    expire(q, window);
    if q.len() >= limit {
        return false;
    }
    q.push_back(Instant::now());
    true
}

/// Sliding window per key, in a table of at most `max_keys`. O(1) amortized;
/// stale keys are dropped by `prune_table`.
fn allow(table: &mut HashMap<String, VecDeque<Instant>>, key: &str, limit: usize, window: Duration, max_keys: usize) -> bool {
    let key = capped_key(table, key);
    make_room(table, &key, window, max_keys);
    allow_window(table.entry(key).or_default(), limit, window)
}

/// `who` may count against `key` if already among its requesters in the
/// window or fewer than `limit` distinct requesters are; counted once.
///
/// Security-relevant: live entries are never evicted. When the table is full
/// of live keys, *new* keys are refused (known keys keep working).
fn allow_distinct(
    table: &mut HashMap<String, VecDeque<(Instant, String)>>,
    key: &str,
    who: &str,
    limit: usize,
    window: Duration,
    max_keys: usize,
) -> bool {
    let now = Instant::now();
    if !table.contains_key(key) && table.len() >= max_keys {
        prune_table(table, window);
        if table.len() >= max_keys {
            return false;
        }
    }
    let q = table.entry(key.to_string()).or_default();
    while q.front().is_some_and(|(t, _)| now.duration_since(*t) >= window) {
        q.pop_front();
    }
    if q.iter().any(|(_, w)| w == who) {
        return true;
    }
    if q.len() >= limit {
        return false;
    }
    q.push_back((now, who.to_string()));
    true
}

/// Drop keys whose newest entry is older than `window`.
fn prune_table<V: Stamped>(table: &mut HashMap<String, VecDeque<V>>, window: Duration) {
    let now = Instant::now();
    table.retain(|_, q| q.back().is_some_and(|t| now.duration_since(t.at()) < window));
}

/// Bucket for the new-install budget: IPv4 by /24 (IPv6 addresses already
/// arrive as their /64).
fn network_bucket(ip: &str) -> String {
    match ip.parse::<std::net::Ipv4Addr>() {
        Ok(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.0/24", o[0], o[1], o[2])
        }
        Err(_) => ip.to_string(),
    }
}

const HOUR: Duration = Duration::from_secs(3600);
const MINUTE: Duration = Duration::from_secs(60);
const DAY: Duration = Duration::from_secs(24 * 3600);

pub struct Hub {
    pub limits: Limits,
    /// Reverse proxies whose X-Real-IP / X-Forwarded-For are believed.
    pub trusted_proxies: Vec<std::net::IpAddr>,
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
    state: Mutex<State>,
    stop: AtomicBool,
    proxy_warned: AtomicBool,
    /// Bytes of friend frames in all connection queues.
    queued_total: AtomicUsize,
    started: Instant,
    pub counters: Counters,
    next_conn: AtomicU64,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Hub {
    pub(crate) fn new(limits: Limits, trusted_proxies: Vec<std::net::IpAddr>, private_key: [u8; 32], mut store: Store) -> Hub {
        store.max_installs = limits.max_installs;
        let global_bytes = Bucket::new(limits.global_bytes_per_sec, limits.global_bytes_burst);
        let global_bytes_hard = Bucket::new(2.0 * limits.global_bytes_per_sec, 2.0 * limits.global_bytes_burst);
        Hub {
            limits,
            trusted_proxies,
            private_key,
            public_key: proto::public_key(&private_key),
            state: Mutex::new(State { store: Some(store), next_ch: 1, global_bytes, global_bytes_hard, ..State::default() }),
            stop: AtomicBool::new(false),
            proxy_warned: AtomicBool::new(false),
            queued_total: AtomicUsize::new(0),
            started: Instant::now(),
            counters: Counters::default(),
            next_conn: AtomicU64::new(1),
            threads: Mutex::new(Vec::new()),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn conn_id(&self) -> u64 {
        self.next_conn.fetch_add(1, Ordering::Relaxed)
    }

    pub fn spawn(&self, name: &str, f: impl FnOnce() + Send + 'static) -> bool {
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        threads.retain(|t| !t.is_finished());
        if threads.len() >= self.limits.max_conns + 16 {
            return false; // connection flood: refuse instead of spawning without bound
        }
        match std::thread::Builder::new().name(name.into()).spawn(f) {
            Ok(h) => {
                threads.push(h);
                true
            }
            Err(_) => false,
        }
    }

    pub fn join_all(&self) {
        loop {
            let threads: Vec<_> = std::mem::take(&mut *self.threads.lock().unwrap_or_else(|e| e.into_inner()));
            if threads.is_empty() {
                break;
            }
            for t in threads {
                let _ = t.join();
            }
        }
    }

    /// Count a new connection from `ip`; `false` if over a limit.
    ///
    /// Connections still in the challenge cost an attacker nothing, so they
    /// have their own smaller pools, per address, per network and in all. A
    /// full pool never refuses the newcomer: its oldest member goes instead,
    /// so holding the pool means opening it all again every round trip.
    pub(crate) fn admit_ip(&self, ip: &str) -> Option<Admission> {
        let lim = &self.limits;
        let net = share_bucket(ip);
        let mut guard = self.state();
        let st = &mut *guard;
        let by_ip = st.ip_conns.get(ip).copied().unwrap_or_default();
        let by_net = st.net_conns.get(&net).copied().unwrap_or_default();
        if by_ip.total >= lim.max_conns_per_ip || by_net.total >= lim.max_conns_per_network {
            bump(&self.counters.conns_refused_limit);
            return None;
        }
        if st.all_conns.total >= lim.max_conns {
            // Room for the next try, if any of them isn't a real install.
            st.evict_unauthed(&self.counters, |_| true);
            bump(&self.counters.conns_refused_limit);
            return None;
        }
        if by_ip.unauthed >= lim.max_unauthed_per_ip {
            st.evict_unauthed(&self.counters, |u| u.ip == ip);
        }
        if st.net_conns.get(&net).is_some_and(|c| c.unauthed >= lim.max_unauthed_per_network) {
            st.evict_unauthed(&self.counters, |u| u.net == net);
        }
        if st.all_conns.unauthed >= lim.max_unauthed {
            st.evict_unauthed(&self.counters, |_| true);
        }
        bump(&self.counters.conns_admitted);
        st.all_conns.admit();
        st.ip_conns.entry(ip.to_string()).or_default().admit();
        st.net_conns.entry(net.clone()).or_default().admit();
        let net_queued = st.net_queued.entry(net.clone()).or_default().clone();
        let conn = self.conn_id();
        let kill: Arc<OnceLock<&'static str>> = Arc::default();
        st.unauthed.insert(conn, Unauthed { ip: ip.to_string(), net, kill: kill.clone() });
        Some(Admission { conn, kill, net_queued })
    }

    /// A connection let in by `admit_ip` ended.
    pub fn release_ip(&self, ip: &str, conn: u64) {
        let net = share_bucket(ip);
        let mut st = self.state();
        st.no_longer_unauthed(conn);
        st.all_conns.release();
        if st.ip_conns.get_mut(ip).is_some_and(Conns::release) {
            st.ip_conns.remove(ip);
        }
        if st.net_conns.get_mut(&net).is_some_and(Conns::release) {
            st.net_conns.remove(&net);
            st.net_queued.remove(&net);
        }
    }

    /// Say once that the configured proxy isn't telling client addresses.
    pub fn warn_proxy_without_address(&self) {
        bump(&self.counters.proxy_without_address);
        if !self.proxy_warned.swap(true, Ordering::Relaxed) {
            eprintln!(
                "screenpet-relay: a trusted proxy sent no usable X-Real-IP or X-Forwarded-For; \
                 its clients all share one address's limits (check the proxy config)"
            );
        }
    }

    /// Valid, distinct, canonical friend keys (not `own`), at most `max_friends`.
    fn clean_friends(&self, own: &str, keys: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for k in keys.iter().filter_map(|k| proto::norm_key(k)) {
            if k != own && !out.contains(&k) && out.len() < self.limits.max_friends {
                out.push(k);
            }
        }
        out
    }

    /// An authenticated install comes online. A previous connection with the
    /// same key is kicked. Returns the `welcome` message.
    pub(crate) fn join(
        &self,
        key: &str,
        id: &str,
        friends: &[String],
        ip: &str,
        conn: u64,
        link: Link,
    ) -> Result<ServerMsg, &'static str> {
        let friends = self.clean_friends(key, friends);
        let now = unix_now();
        let mut st = self.state();
        if !st.unauthed.contains_key(&conn) {
            return Err("busy"); // evicted just now
        }
        let known = st.store_ref().get(key).is_some();
        if !known {
            // (A full store is handled by `register_with`, which first evicts
            // a stale friendless install.)
            // Each network gets its own budget; the global ceiling is a
            // looser backstop.
            if !allow(&mut st.ip_registrations, &network_bucket(ip), self.limits.registrations_per_ip_per_hour, HOUR, MAX_RATE_KEYS)
                || !allow(&mut st.net_registrations, &share_bucket(ip), self.limits.registrations_per_network_per_day, DAY, MAX_RATE_KEYS)
                || !allow_window(&mut st.global_registrations, self.limits.registrations_per_hour, HOUR)
            {
                bump(&self.counters.registrations_refused);
                return Err("busy");
            }
        }
        if let Some(old) = st.peers.remove(key) {
            old.link.kick("replaced");
            for ch in old.channels {
                st.close_channel(ch, Some(key));
            }
        }
        let registered = {
            let st = &mut *st;
            let peers = &st.peers;
            st.store.as_mut().expect("store present").register_with(key, id, friends, now, &|k| peers.contains_key(k))
        };
        if !registered {
            bump(&self.counters.store_full);
            return Err("full");
        }
        if !known {
            bump(&self.counters.registrations);
        }
        let (code, code_ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs()).expect("registered");
        st.peers.insert(key.to_string(), Peer { conn, id: id.to_string(), link, channels: HashSet::new() });
        st.no_longer_unauthed(conn);
        let online = st.mutual_online(key);
        for f in &online {
            st.peers[f].send(ServerMsg::Presence { key: key.to_string(), online: true });
        }
        Ok(ServerMsg::Welcome { code, code_ttl, online, proof: None })
    }

    /// The connection ended.
    pub(crate) fn leave(&self, key: &str, conn: u64) {
        let mut st = self.state();
        if st.peers.get(key).is_none_or(|p| p.conn != conn) {
            return; // already replaced by a newer connection
        }
        let online = st.mutual_online(key);
        let peer = st.peers.remove(key).expect("checked");
        for ch in peer.channels {
            st.close_channel(ch, Some(key));
        }
        for f in online {
            if let Some(p) = st.peers.get(&f) {
                p.send(ServerMsg::Presence { key: key.to_string(), online: false });
            }
        }
        st.store().touch(key, unix_now());
    }

    fn is_current(st: &State, key: &str, conn: u64) -> bool {
        st.peers.get(key).is_some_and(|p| p.conn == conn)
    }

    /// Replace `key`'s friend list; tell friends whose mutual status changed.
    pub(crate) fn set_friends(&self, key: &str, conn: u64, keys: &[String]) -> Option<ServerMsg> {
        let friends = self.clean_friends(key, keys);
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let before: HashSet<String> = st.mutual_online(key).into_iter().collect();
        st.store().set_friends(key, friends);
        let after = st.mutual_online(key);
        let after_set: HashSet<String> = after.iter().cloned().collect();
        for f in after_set.symmetric_difference(&before) {
            if let Some(p) = st.peers.get(f) {
                p.send(ServerMsg::Presence { key: key.to_string(), online: after_set.contains(f) });
            }
        }
        Some(ServerMsg::Online { keys: after })
    }

    /// Redeem a friend code: on success `key` may open one channel to its
    /// owner for `grant_ttl`.
    pub(crate) fn redeem(&self, key: &str, conn: u64, ip: &str, code: &str) -> Option<ServerMsg> {
        let fail = |reason: &str| Some(ServerMsg::RedeemFailed { code: code.to_string(), reason: reason.into() });
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        if !allow(&mut st.ip_redeems, ip, self.limits.redeems_per_ip_per_min, MINUTE, MAX_RATE_KEYS) {
            return fail("rate");
        }
        // One answer for "no such code", "owner offline" and "your own code":
        // a guess learns nothing about which codes exist.
        let Some(norm) = proto::normalize_code(code) else { return fail("unknown") };
        // Global budget, charged only for well-formed codes. Each network
        // (IPv4 /24, IPv6 /48) has a fair share (a tenth of the budget, never
        // more than the per-IP cap) reserved inside it: within its share a
        // network is not refused by the soft budget, so one heavy source
        // cannot starve the rest. A hard ceiling of twice the budget applies
        // to everyone.
        let per_min = self.limits.redeems_per_min;
        let share = (per_min / 10).min(self.limits.redeems_per_ip_per_min).max(1);
        let light = allow(&mut st.ip_wellformed, &share_bucket(ip), share, MINUTE, MAX_RATE_KEYS);
        expire(&mut st.global_redeems, MINUTE);
        let used = st.global_redeems.len();
        if used >= 2 * per_min || (!light && used >= per_min) {
            return fail("rate");
        }
        st.global_redeems.push_back(Instant::now());
        let now = unix_now();
        let Some(target) = st.store_ref().by_code(&norm, now, self.limits.code_ttl.as_secs()).map(str::to_string) else {
            return fail("unknown");
        };
        let Some(id) = st.peers.get(&target).map(|p| p.id.clone()).filter(|_| target != key) else {
            return fail("unknown");
        };
        // A code that keeps being redeemed is being guessed or spread: cap
        // distinct requesters per code (a new code starts fresh; a requester
        // retrying counts once).
        let per_code = format!("{target} {norm}");
        if !st.target_redeems.contains_key(&per_code)
            && !(allow(&mut st.net_new_codes, &share_bucket(ip), self.limits.new_codes_per_network_per_hour, HOUR, MAX_RATE_KEYS)
                && allow(&mut st.net_new_codes, &wide_bucket(ip), WIDE_NEW_CODES * self.limits.new_codes_per_network_per_hour, HOUR, MAX_RATE_KEYS))
        {
            return fail("rate");
        }
        if !allow_distinct(&mut st.target_redeems, &per_code, key, self.limits.redeems_per_target_per_hour, HOUR, MAX_RATE_KEYS) {
            return fail("unknown");
        }
        st.store().mark_active(key);
        st.store().mark_active(&target);
        let until = Instant::now() + self.limits.grant_ttl;
        let t = Instant::now();
        st.grants.retain(|g| g.until > t && !(g.from == key && g.to == target));
        if st.grants.iter().filter(|g| g.from == key).count() >= 4 {
            let oldest = st.grants.iter().position(|g| g.from == key).expect("counted");
            st.grants.remove(oldest);
        }
        st.grants.push(Grant { from: key.to_string(), to: target.clone(), until });
        Some(ServerMsg::Redeemed { code: norm, key: target, id })
    }

    /// Open a channel from `key` to `to`: they must be mutual friends, or
    /// `key` must hold a grant from a redeemed code (used up here).
    pub(crate) fn open(&self, key: &str, conn: u64, req: u32, to: &str) -> Option<ServerMsg> {
        let fail = |reason: &str| Some(ServerMsg::OpenFailed { req, reason: reason.into() });
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let Some(to) = proto::norm_key(to) else { return fail("bad_key") };
        if to == key {
            return fail("self");
        }
        // Allowed at all? Asked first: a stranger learns nothing about whether
        // a key is online.
        let now = Instant::now();
        let grant = st.grants.iter().position(|g| g.from == key && g.to == to && g.until > now);
        let mutual = st.store_ref().mutual(key, &to);
        if !mutual && grant.is_none() {
            return fail("not_friends");
        }
        let Some(target) = st.peers.get(&to) else { return fail("offline") };
        let target_conn = target.conn;
        let max = self.limits.max_channels;
        if st.peers[key].channels.len() >= max || target.channels.len() >= max {
            return fail("busy");
        }
        let between = st.channels.values().filter(|c| c.ends.iter().any(|e| e.0 == key) && c.ends.iter().any(|e| e.0 == to)).count();
        if between >= self.limits.max_channels_per_pair {
            return fail("busy");
        }
        if let (false, Some(i)) = (mutual, grant) {
            st.grants.remove(i);
        }
        let ch = loop {
            let ch = st.next_ch;
            st.next_ch = st.next_ch.checked_add(1).unwrap_or(1);
            if ch != 0 && !st.channels.contains_key(&ch) {
                break ch;
            }
        };
        let (rate, burst) = (self.limits.chan_msgs_per_sec, self.limits.chan_msgs_burst);
        st.channels.insert(
            ch,
            Channel { ends: [(key.to_string(), conn), (to.clone(), target_conn)], buckets: [Bucket::new(rate, burst), Bucket::new(rate, burst)] },
        );
        let id = st.peers[key].id.clone();
        st.peers.get_mut(key).expect("current").channels.insert(ch);
        let target = st.peers.get_mut(&to).expect("online");
        target.channels.insert(ch);
        target.send(ServerMsg::Incoming { ch, from: key.to_string(), id });
        Some(ServerMsg::Opened { req, ch })
    }

    pub(crate) fn close(&self, key: &str, conn: u64, ch: u32) {
        let mut st = self.state();
        let mine = st.channels.get(&ch).is_some_and(|c| c.ends.iter().any(|e| e.0 == key && e.1 == conn));
        if mine {
            st.close_channel(ch, Some(key));
        }
    }

    /// Forward one binary message (`ch` header + frame) to the channel's other
    /// end. Unknown channels are ignored (closed while in flight). A channel
    /// over its rate (or its sender's address, or the relay, over their
    /// bandwidth), or whose receiver has too much waiting, is closed.
    pub(crate) fn forward(&self, key: &str, conn: u64, ip: &str, msg: Vec<u8>) {
        let ch = u32::from_be_bytes([msg[0], msg[1], msg[2], msg[3]]);
        let len = msg.len();
        let mut guard = self.state();
        let st = &mut *guard;
        let Some(c) = st.channels.get_mut(&ch) else { return };
        let Some(side) = c.ends.iter().position(|e| e.0 == key && e.1 == conn) else { return };
        let rate_ok = c.buckets[side].take(1.0);
        let other = c.ends[1 - side].0.clone();
        if !rate_ok || !self.bandwidth_allows(st, ip, len as f64) {
            bump(&self.counters.channels_closed_rate);
            st.close_channel(ch, None);
            return;
        }
        let delivered = st.peers.get(&other).is_some_and(|p| self.enqueue_frame(&p.link, msg));
        if delivered {
            bump(&self.counters.frames_forwarded);
            self.counters.bytes_forwarded.fetch_add(len as u64, Ordering::Relaxed);
        } else {
            bump(&self.counters.channels_closed_queue);
            st.close_channel(ch, None);
        }
    }

    /// May `n` more bytes from `ip` go through? The address cap is hard. The
    /// global budget doesn't bind a network within its fair share, so one
    /// source using up the budget closes only its own channels; many networks
    /// together still stop at the ceiling.
    fn bandwidth_allows(&self, st: &mut State, ip: &str, n: f64) -> bool {
        let lim = &self.limits;
        if !bucket_in(&mut st.ip_bytes, ip, lim.ip_bytes_per_sec, lim.ip_bytes_burst, MAX_RATE_KEYS).take(n) || !st.global_bytes_hard.take(n) {
            return false;
        }
        let (rate, burst) = (lim.global_bytes_per_sec * NET_BYTES_SHARE, lim.global_bytes_burst * NET_BYTES_SHARE);
        let in_share = bucket_in(&mut st.net_bytes, &share_bucket(ip), rate, burst, MAX_RATE_KEYS).take(n);
        st.global_bytes.take(n) || in_share
    }

    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }

    /// Queue a friend frame for `link`; `false` if it doesn't fit. Memory is
    /// bounded here, but a full queue alone only costs the frame's channel: a
    /// sender can fill a healthy receiver's queue faster than it drains, and
    /// must not get it disconnected. Only a queue that isn't moving is `slow`.
    fn enqueue_frame(&self, link: &Link, msg: Vec<u8>) -> bool {
        let len = msg.len();
        let lim = &self.limits;
        let queued = link.queued.load(Ordering::Relaxed);
        if queued == 0 {
            link.drained_at.store(self.now_ms(), Ordering::Relaxed);
        }
        if queued + len > lim.max_outbox_bytes {
            let still = self.now_ms().saturating_sub(link.drained_at.load(Ordering::Relaxed));
            if still >= lim.slow_after.as_millis() as u64 {
                let _ = link.kill.set("slow");
            }
            return false;
        }
        // The total caps server memory; one network's stuck receivers using it
        // up stop only that network, until the ceiling of twice the cap.
        let total = self.queued_total.load(Ordering::Relaxed) + len;
        let net = link.net_queued.load(Ordering::Relaxed) + len;
        if total > lim.max_total_outbox_bytes && (net > lim.max_total_outbox_bytes / NET_QUEUE_SHARE || total > 2 * lim.max_total_outbox_bytes) {
            return false;
        }
        // Counted first: the connection thread may take it off at once.
        link.queued.fetch_add(len, Ordering::Relaxed);
        link.net_queued.fetch_add(len, Ordering::Relaxed);
        self.queued_total.fetch_add(len, Ordering::Relaxed);
        if link.out.try_send(Out::Binary(msg)).is_err() {
            self.dequeued(link, len);
            return false;
        }
        true
    }

    /// `len` bytes of friend frames left `link`'s queue.
    pub(crate) fn dequeued(&self, link: &Link, len: usize) {
        link.queued.fetch_sub(len, Ordering::Relaxed);
        link.net_queued.fetch_sub(len, Ordering::Relaxed);
        self.queued_total.fetch_sub(len, Ordering::Relaxed);
        link.drained_at.store(self.now_ms(), Ordering::Relaxed);
    }

    /// Issue a fresh code now (user asked for a new one).
    pub(crate) fn new_code(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let now = unix_now();
        st.store().rotate(key, now);
        let (code, code_ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs())?;
        Some(ServerMsg::Code { code, code_ttl })
    }

    /// A rotated friend code for `key`, if its code expired.
    pub(crate) fn rotated_code(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let (code, code_ttl, rotated) = st.store().code(key, unix_now(), self.limits.code_ttl.as_secs())?;
        rotated.then_some(ServerMsg::Code { code, code_ttl })
    }

    /// Periodic housekeeping of what's only in memory: grants, rate tables.
    pub(crate) fn maintain(&self) {
        let mut st = self.state();
        let now = Instant::now();
        st.grants.retain(|g| g.until > now);
        prune_table(&mut st.ip_redeems, MINUTE);
        expire(&mut st.global_redeems, MINUTE);
        prune_table(&mut st.ip_registrations, HOUR);
        prune_table(&mut st.net_registrations, DAY);
        expire(&mut st.global_registrations, HOUR);
        prune_table(&mut st.target_redeems, HOUR);
        prune_table(&mut st.net_new_codes, HOUR);
        prune_table(&mut st.ip_wellformed, MINUTE);
        st.ip_bytes.retain(|_, b| !b.is_full());
        st.net_bytes.retain(|_, b| !b.is_full());
    }

    /// Forget stale installs. Online ones are noted as seen first, so what's
    /// on disk stays fresh enough that a restart doesn't make them look gone.
    pub(crate) fn prune_store(&self) {
        let now = unix_now();
        let mut guard = self.state();
        let st = &mut *guard;
        let store = st.store.as_mut().expect("store present");
        for key in st.peers.keys() {
            store.touch(key, now);
        }
        let peers = &st.peers;
        store.prune_with(now, &|k| peers.contains_key(k));
    }

    /// Write the store if it changed. The snapshot is taken under the lock;
    /// serializing and writing happen outside it so connections are never
    /// blocked on the disk.
    pub(crate) fn save(&self) {
        let snapshot = self.state().store().snapshot();
        if let Some(snap) = snapshot {
            if let Err(e) = crate::store::write_snapshot(&snap) {
                eprintln!("screenpet-relay: cannot save data: {e}");
                bump(&self.counters.save_errors);
                self.state().store().mark_dirty();
            }
        }
    }

    /// Number of installs online (for logs/tests).
    pub fn online_count(&self) -> usize {
        self.state().peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{sync_channel, Receiver};

    fn hub(limits: Limits) -> Hub {
        Hub::new(limits, Vec::new(), proto::random32(), Store::memory())
    }

    /// Join install `n`, friends with `friends`, on a queue nobody drains.
    fn peer(hub: &Hub, n: u8, friends: &[u8], cap: usize) -> (String, u64, Link, Receiver<Out>) {
        peer_at(hub, "203.0.113.1", n, friends, cap)
    }

    fn peer_at(hub: &Hub, ip: &str, n: u8, friends: &[u8], cap: usize) -> (String, u64, Link, Receiver<Out>) {
        let key = proto::to_hex(&[n; 32]);
        let friends: Vec<String> = friends.iter().map(|f| proto::to_hex(&[*f; 32])).collect();
        let (tx, rx) = sync_channel(cap);
        let admission = hub.admit_ip(ip).expect("admitted");
        let link = Link::new(tx, &admission);
        hub.join(&key, "x", &friends, ip, admission.conn, link.clone()).unwrap();
        (key, admission.conn, link, rx)
    }

    /// Two mutual friends and a channel between them, `b` never reading.
    fn stuck_pair(hub: &Hub) -> (String, u64, Link, Link, u32, Receiver<Out>, Receiver<Out>) {
        let (ka, ca, la, ra) = peer(hub, 1, &[2], 512);
        let (kb, _, lb, rb) = peer(hub, 2, &[1], 512);
        let Some(ServerMsg::Opened { ch, .. }) = hub.open(&ka, ca, 1, &kb) else { panic!("no channel") };
        (ka, ca, la, lb, ch, ra, rb)
    }

    fn frame(ch: u32, len: usize) -> Vec<u8> {
        let mut m = ch.to_be_bytes().to_vec();
        m.resize(4 + len, 0);
        m
    }

    #[test]
    fn a_full_queue_closes_the_channel_and_only_a_stuck_one_disconnects() {
        let limits = Limits { max_outbox_bytes: 200_000, chan_msgs_burst: 1000.0, slow_after: Duration::from_millis(100), ..Limits::default() };
        let h = hub(limits);
        let (ka, ca, _la, lb, ch, _ra, _rb) = stuck_pair(&h);
        // The old limit was 512 messages, so 33 MB of 64 KB frames.
        for _ in 0..3 {
            h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        }
        assert_eq!(lb.queued.load(Ordering::Relaxed), 180_012);
        assert!(lb.kill.get().is_none());
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        assert!(lb.kill.get().is_none(), "a sender filling the queue fast can't get the receiver disconnected");
        assert_eq!(lb.queued.load(Ordering::Relaxed), 180_012, "the refused frame is not counted");
        assert!(h.state().channels.is_empty(), "its channel is closed");
        // A queue that drains is fine however long it has been around…
        std::thread::sleep(Duration::from_millis(150));
        h.dequeued(&lb, 4);
        let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, 2, &proto::to_hex(&[2; 32])) else { panic!("no channel") };
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        assert!(lb.kill.get().is_none());
        // …one that has not moved for `slow_after` is not.
        std::thread::sleep(Duration::from_millis(150));
        let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, 3, &proto::to_hex(&[2; 32])) else { panic!("no channel") };
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        assert_eq!(lb.kill.get(), Some(&"slow"));
    }

    #[test]
    fn one_network_using_up_the_total_queue_stops_only_itself() {
        let limits = Limits { max_total_outbox_bytes: 64_000, chan_msgs_burst: 1000.0, ..Limits::default() };
        let h = hub(limits);
        // 1 → 2 stuck in one network; 3 → 4 with its receiver in another.
        let (k1, c1, _, _r1) = peer_at(&h, "203.0.113.1", 1, &[2], 512);
        let (k2, _, _, _r2) = peer_at(&h, "203.0.113.2", 2, &[1], 512);
        let (k3, c3, _, _r3) = peer_at(&h, "192.0.2.1", 3, &[4], 512);
        let (k4, _, _, _r4) = peer_at(&h, "198.51.100.1", 4, &[3], 512);
        let Some(ServerMsg::Opened { ch: heavy, .. }) = h.open(&k1, c1, 1, &k2) else { panic!() };
        let Some(ServerMsg::Opened { ch: light, .. }) = h.open(&k3, c3, 1, &k4) else { panic!() };
        h.forward(&k1, c1, "203.0.113.1", frame(heavy, 63_000));
        h.forward(&k1, c1, "203.0.113.1", frame(heavy, 63_000));
        assert!(!h.state().channels.contains_key(&heavy), "over the total and its share");
        h.forward(&k3, c3, "192.0.2.1", frame(light, 500));
        assert!(h.state().channels.contains_key(&light), "within its share");
    }

    #[test]
    fn the_global_byte_budget_keeps_a_share_per_network_up_to_a_ceiling() {
        // Budget 160 KB (ceiling 320 KB), each network's share 10 KB.
        let limits = Limits { global_bytes_per_sec: 1.0, global_bytes_burst: 160_000.0, chan_msgs_burst: 1000.0, ..Limits::default() };
        let h = hub(limits);
        let (ka, ca, _, _ra, ) = peer(&h, 1, &[2], 512);
        let (kb, _, _, _rb) = peer(&h, 2, &[1], 512);
        let open = |req| {
            let Some(ServerMsg::Opened { ch, .. }) = h.open(&ka, ca, req, &kb) else { panic!("no channel") };
            ch
        };
        let alive = |ch| h.state().channels.contains_key(&ch);
        let ch = open(1);
        for _ in 0..16 {
            h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
        }
        // Budget nearly gone: this source, past its share, loses its channel.
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 9_000));
        assert!(!alive(ch));
        // Another network within its share goes through.
        let ch = open(2);
        h.forward(&ka, ca, "198.51.100.1", frame(ch, 9_000));
        assert!(alive(ch));
        // Enough different networks reach the ceiling.
        let mut n = 0;
        while alive(ch) {
            n += 1;
            assert!(n < 40, "no ceiling");
            h.forward(&ka, ca, &format!("10.{n}.0.1"), frame(ch, 9_000));
        }
    }

    #[test]
    fn byte_buckets_are_kept_until_refilled_in_a_bounded_table() {
        let h = hub(Limits { ip_bytes_per_sec: 1.0, ip_bytes_burst: 1000.0, ..Limits::default() });
        let a = h.admit_ip("203.0.113.1").unwrap();
        assert!(h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0));
        h.release_ip("203.0.113.1", a.conn);
        h.maintain();
        assert!(!h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0), "reconnecting doesn't refill it");

        let h = hub(Limits { ip_bytes_per_sec: 1e9, ip_bytes_burst: 1000.0, ..Limits::default() });
        assert!(h.bandwidth_allows(&mut h.state(), "203.0.113.1", 900.0));
        std::thread::sleep(Duration::from_millis(5));
        h.maintain();
        assert!(h.state().ip_bytes.is_empty(), "refilled: dropped");

        let mut t = HashMap::new();
        bucket_in(&mut t, "used", 1.0, 100.0, 2).take(90.0);
        bucket_in(&mut t, "fresh", 1.0, 100.0, 2);
        bucket_in(&mut t, "new", 1.0, 100.0, 2);
        assert_eq!(t.len(), 2);
        assert!(t.contains_key("used") && !t.contains_key("fresh"), "the fullest goes");
    }

    #[test]
    fn an_evicted_connection_cannot_join() {
        let h = hub(Limits { max_unauthed_per_ip: 1, ..Limits::default() });
        let first = h.admit_ip("203.0.113.1").unwrap();
        let second = h.admit_ip("203.0.113.1").unwrap();
        assert_eq!(first.kill.get(), Some(&"busy"));
        assert!(second.kill.get().is_none());
        let (tx, _rx) = sync_channel(1);
        let link = Link::new(tx, &first);
        assert_eq!(h.join(&proto::to_hex(&[1; 32]), "x", &[], "203.0.113.1", first.conn, link).err(), Some("busy"));
        // Its slot is counted once when it finally ends.
        h.release_ip("203.0.113.1", first.conn);
        h.release_ip("203.0.113.1", second.conn);
        let st = h.state();
        assert_eq!((st.all_conns.total, st.all_conns.unauthed), (0, 0));
        assert!(st.ip_conns.is_empty() && st.net_conns.is_empty() && st.net_queued.is_empty());
    }

    #[test]
    fn too_many_queued_bytes_in_all_close_the_channel_only() {
        let limits = Limits { max_total_outbox_bytes: 100_000, chan_msgs_burst: 1000.0, ..Limits::default() };
        let h = hub(limits);
        let (ka, ca, _la, lb, ch, _ra, _rb) = stuck_pair(&h);
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        h.forward(&ka, ca, "203.0.113.1", frame(ch, 60_000));
        assert!(lb.kill.get().is_none());
        assert!(h.state().channels.is_empty());
    }

    #[test]
    fn a_replaced_connection_with_a_full_queue_still_goes() {
        let h = hub(Limits::default());
        let (_, _, old, _rx) = peer(&h, 1, &[], 1);
        old.out.try_send(Out::Text("filler".into())).unwrap();
        let (_, _, _new, _rx2) = peer(&h, 1, &[], 8);
        assert_eq!(old.kill.get(), Some(&"replaced"), "it could not be told through the queue");
    }

    #[test]
    fn online_installs_are_kept_fresh_for_after_a_restart() {
        let h = hub(Limits::default());
        let (key, _, _, _rx) = peer(&h, 1, &[], 8);
        let long_ago = unix_now() - 2 * 24 * 3600;
        h.state().store().touch_at_for_test(&key, long_ago);
        h.prune_store();
        assert!(h.state().store_ref().get(&key).unwrap().last_seen > long_ago);
    }

    #[test]
    fn a_full_queue_is_slow() {
        let h = hub(Limits::default());
        let (_, _, a, _rx) = peer(&h, 1, &[2], 1);
        a.out.try_send(Out::Text("filler".into())).unwrap();
        let (_, _, _b, _rx2) = peer(&h, 2, &[1], 8); // presence for 1 doesn't fit
        assert_eq!(a.kill.get(), Some(&"slow"));
    }

    #[test]
    fn a_full_rate_table_evicts_instead_of_refusing_newcomers() {
        let mut t = HashMap::new();
        for i in 0..4 {
            assert!(allow(&mut t, &format!("k{i}"), 1, HOUR, 4));
        }
        // Table full of live keys: a new key still gets its own budget.
        assert!(allow(&mut t, "new", 1, HOUR, 4));
        assert!(t.len() <= 4);
        assert!(!allow(&mut t, "new", 1, HOUR, 4), "and its limit still applies");
    }

    #[test]
    fn one_network_cannot_fill_a_table_with_its_own_keys() {
        let mut t = HashMap::new();
        for i in 0..100u32 {
            let key = format!("2001:db8:1:{i:x}::/64");
            allow(&mut t, &key, 1000, HOUR, 4096);
        }
        assert!(t.len() <= MAX_KEYS_PER_PARENT + 1, "{} keys from one /48", t.len());
        let mut t = HashMap::new();
        for i in 0..100u32 {
            allow(&mut t, &format!("203.0.113.{i}"), 1000, HOUR, 4096);
        }
        assert!(t.len() <= MAX_KEYS_PER_PARENT + 1, "{} keys from one /24", t.len());
    }

    #[test]
    fn the_per_code_table_fails_closed_for_new_targets_only() {
        let mut t = HashMap::new();
        assert!(allow_distinct(&mut t, "t1 c", "x", 5, HOUR, 2));
        assert!(allow_distinct(&mut t, "t2 c", "x", 5, HOUR, 2));
        assert!(!allow_distinct(&mut t, "t3 c", "x", 5, HOUR, 2), "full: no eviction of live caps");
        assert!(allow_distinct(&mut t, "t1 c", "y", 5, HOUR, 2), "known targets keep working");
        assert!(t.contains_key("t1 c") && t.contains_key("t2 c"));
    }

    #[test]
    fn wide_buckets() {
        assert_eq!(wide_bucket("203.0.113.9"), "203.0.0.0/16");
        assert_eq!(wide_bucket("2001:db8:1:2::/64"), "2001:db8::/32");
    }

    #[test]
    fn share_buckets() {
        assert_eq!(share_bucket("203.0.113.9"), "203.0.113.0/24");
        assert_eq!(share_bucket("2001:db8:1:2::/64"), "2001:db8:1::/48");
    }
}
