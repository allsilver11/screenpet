//! Shared relay state: who is online, channels between them, pairing grants
//! from redeemed friend codes, and per-address limits. Connection threads
//! talk to each other only through `Out` queues (never blocking under the
//! hub lock).

use crate::proto::{self, ServerMsg};
use crate::store::{unix_now, Store};
use crate::Limits;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

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

impl Bucket {
    pub fn new(rate: f64, burst: f64) -> Bucket {
        Bucket { tokens: burst, rate, burst, last: Instant::now() }
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

struct Peer {
    conn: u64,
    id: String,
    out: SyncSender<Out>,
    /// Set when the peer's queue overflowed: its thread disconnects it.
    kill: Arc<AtomicBool>,
    channels: HashSet<u32>,
}

impl Peer {
    fn send(&self, m: ServerMsg) {
        if self.out.try_send(Out::Text(m.to_json())).is_err() {
            self.kill.store(true, Ordering::SeqCst);
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

#[derive(Default)]
struct State {
    store: Option<Store>,
    peers: HashMap<String, Peer>,
    channels: HashMap<u32, Channel>,
    next_ch: u32,
    grants: Vec<Grant>,
    ip_conns: HashMap<String, usize>,
    total_conns: usize,
    ip_redeems: HashMap<String, VecDeque<Instant>>,
    ip_registrations: HashMap<String, VecDeque<Instant>>,
    /// Per "target-key code": recent distinct requesters (cap per code, so
    /// a new code starts fresh and one requester retrying counts once).
    target_redeems: HashMap<String, VecDeque<(Instant, String)>>,
    /// Well-formed redeems per source address (fair share of the global budget).
    ip_wellformed: HashMap<String, VecDeque<Instant>>,
    /// Global windows (keyed by "*"): new installs per hour, redeems per minute.
    global_registrations: HashMap<String, VecDeque<Instant>>,
    global_redeems: HashMap<String, VecDeque<Instant>>,
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

/// Make room for `key` in a table holding at most `max` keys.
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

/// Sliding-window counter: `true` (and counted) if fewer than `limit` in
/// `window`. O(1) amortized; stale keys are dropped by `prune_table`.
fn allow(table: &mut HashMap<String, VecDeque<Instant>>, key: &str, limit: usize, window: Duration) -> bool {
    allow_in(table, key, limit, window, MAX_RATE_KEYS)
}

fn allow_in(table: &mut HashMap<String, VecDeque<Instant>>, key: &str, limit: usize, window: Duration, max_keys: usize) -> bool {
    let now = Instant::now();
    make_room(table, key, window, max_keys);
    let q = table.entry(key.to_string()).or_default();
    while q.front().is_some_and(|t| now.duration_since(*t) >= window) {
        q.pop_front();
    }
    if q.len() >= limit {
        return false;
    }
    q.push_back(now);
    true
}

/// `who` may count against `key` if already among its requesters in the
/// window or fewer than `limit` distinct requesters are; counted once.
fn allow_distinct(table: &mut HashMap<String, VecDeque<(Instant, String)>>, key: &str, who: &str, limit: usize, window: Duration) -> bool {
    let now = Instant::now();
    make_room(table, key, window, MAX_RATE_KEYS);
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

pub struct Hub {
    pub limits: Limits,
    /// Reverse proxies whose X-Real-IP / X-Forwarded-For are believed.
    pub trusted_proxies: Vec<std::net::IpAddr>,
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
    state: Mutex<State>,
    stop: AtomicBool,
    next_conn: AtomicU64,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Hub {
    pub(crate) fn new(limits: Limits, trusted_proxies: Vec<std::net::IpAddr>, private_key: [u8; 32], mut store: Store) -> Hub {
        store.max_installs = limits.max_installs;
        Hub {
            limits,
            trusted_proxies,
            private_key,
            public_key: proto::public_key(&private_key),
            state: Mutex::new(State { store: Some(store), next_ch: 1, ..State::default() }),
            stop: AtomicBool::new(false),
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
    pub fn admit_ip(&self, ip: &str) -> bool {
        let mut st = self.state();
        let n = st.ip_conns.get(ip).copied().unwrap_or(0);
        if st.total_conns >= self.limits.max_conns || n >= self.limits.max_conns_per_ip {
            return false;
        }
        st.total_conns += 1;
        *st.ip_conns.entry(ip.to_string()).or_default() += 1;
        true
    }

    pub fn release_ip(&self, ip: &str) {
        let mut st = self.state();
        st.total_conns = st.total_conns.saturating_sub(1);
        if let Some(n) = st.ip_conns.get_mut(ip) {
            *n -= 1;
            if *n == 0 {
                st.ip_conns.remove(ip);
            }
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
        out: SyncSender<Out>,
        kill: Arc<AtomicBool>,
    ) -> Result<ServerMsg, &'static str> {
        let friends = self.clean_friends(key, friends);
        let now = unix_now();
        let mut st = self.state();
        let known = st.store_ref().get(key).is_some();
        if !known {
            // (A full store is handled by `register_with`, which first evicts
            // a stale friendless install.)
            // Each network gets its own budget; the global ceiling is a
            // looser backstop.
            if !allow(&mut st.ip_registrations, &network_bucket(ip), self.limits.registrations_per_ip_per_hour, HOUR)
                || !allow(&mut st.global_registrations, "*", self.limits.registrations_per_hour, HOUR)
            {
                return Err("busy");
            }
        }
        if let Some(old) = st.peers.remove(key) {
            let _ = old.out.try_send(Out::Kick("replaced".into()));
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
            return Err("full");
        }
        let (code, code_ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs()).expect("registered");
        st.peers.insert(key.to_string(), Peer { conn, id: id.to_string(), out, kill, channels: HashSet::new() });
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
        if !allow(&mut st.ip_redeems, ip, self.limits.redeems_per_ip_per_min, MINUTE) {
            return fail("rate");
        }
        // One answer for "no such code", "owner offline" and "your own code":
        // a guess learns nothing about which codes exist.
        let Some(norm) = proto::normalize_code(code) else { return fail("unknown") };
        // Global budget, charged only for well-formed codes. A source within
        // its fair share (a tenth of the budget) is never refused by it, so
        // one heavy source cannot starve everyone else.
        let share = (self.limits.redeems_per_min / 10).max(1);
        let light = allow(&mut st.ip_wellformed, ip, share, MINUTE);
        let global_ok = allow(&mut st.global_redeems, "*", self.limits.redeems_per_min, MINUTE);
        if !light && !global_ok {
            return fail("rate");
        }
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
        if !allow_distinct(&mut st.target_redeems, &format!("{target} {norm}"), key, self.limits.redeems_per_target_per_hour, HOUR) {
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
        if !st.store_ref().mutual(key, &to) {
            let now = Instant::now();
            let Some(i) = st.grants.iter().position(|g| g.from == key && g.to == to && g.until > now) else {
                return fail("not_friends");
            };
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
    /// over its rate, or whose receiver is not keeping up, is closed.
    pub(crate) fn forward(&self, key: &str, conn: u64, msg: Vec<u8>) {
        let ch = u32::from_be_bytes([msg[0], msg[1], msg[2], msg[3]]);
        let mut st = self.state();
        let Some(c) = st.channels.get_mut(&ch) else { return };
        let Some(side) = c.ends.iter().position(|e| e.0 == key && e.1 == conn) else { return };
        if !c.buckets[side].take(1.0) {
            st.close_channel(ch, None);
            return;
        }
        let other = c.ends[1 - side].0.clone();
        let delivered = st.peers.get(&other).is_some_and(|p| p.out.try_send(Out::Binary(msg)).is_ok());
        if !delivered {
            st.close_channel(ch, None);
        }
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

    /// Periodic housekeeping: expire grants, prune and save the store.
    /// The snapshot is taken under the lock; serializing and writing happen
    /// outside it so connections are never blocked on the disk.
    pub(crate) fn maintain(&self) {
        let snapshot = {
            let mut st = self.state();
            let now = Instant::now();
            st.grants.retain(|g| g.until > now);
            prune_table(&mut st.ip_redeems, MINUTE);
            prune_table(&mut st.global_redeems, MINUTE);
            prune_table(&mut st.ip_registrations, HOUR);
            prune_table(&mut st.global_registrations, HOUR);
            prune_table(&mut st.target_redeems, HOUR);
            prune_table(&mut st.ip_wellformed, MINUTE);
            let st = &mut *st;
            let peers = &st.peers;
            st.store.as_mut().expect("store present").prune_with(unix_now(), &|k| peers.contains_key(k));
            st.store().snapshot()
        };
        if let Some(snap) = snapshot {
            if let Err(e) = crate::store::write_snapshot(&snap) {
                eprintln!("screenpet-relay: cannot save data: {e}");
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

    #[test]
    fn a_full_rate_table_evicts_instead_of_refusing_newcomers() {
        let mut t = HashMap::new();
        for i in 0..4 {
            assert!(allow_in(&mut t, &format!("k{i}"), 1, HOUR, 4));
        }
        // Table full of live keys: a new key still gets its own budget.
        assert!(allow_in(&mut t, "new", 1, HOUR, 4));
        assert!(t.len() <= 4);
        assert!(!allow_in(&mut t, "new", 1, HOUR, 4), "and its limit still applies");
    }
}
