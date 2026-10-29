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

/// Sliding-window counter: `true` (and counted) if fewer than `limit` in `window`.
fn allow(table: &mut HashMap<String, VecDeque<Instant>>, key: &str, limit: usize, window: Duration) -> bool {
    let now = Instant::now();
    if table.len() > 4096 {
        table.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < window));
    }
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

pub struct Hub {
    pub limits: Limits,
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
    state: Mutex<State>,
    stop: AtomicBool,
    next_conn: AtomicU64,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Hub {
    pub(crate) fn new(limits: Limits, private_key: [u8; 32], store: Store) -> Hub {
        Hub {
            limits,
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
        if !known
            && !allow(&mut st.ip_registrations, ip, self.limits.registrations_per_ip_per_hour, Duration::from_secs(3600))
        {
            return Err("busy");
        }
        if let Some(old) = st.peers.remove(key) {
            let _ = old.out.try_send(Out::Kick("replaced".into()));
            for ch in old.channels {
                st.close_channel(ch, Some(key));
            }
        }
        st.store().register(key, id, friends, now);
        let (code, code_ttl, _) = st.store().code(key, now, self.limits.code_ttl.as_secs()).expect("registered");
        st.peers.insert(key.to_string(), Peer { conn, id: id.to_string(), out, kill, channels: HashSet::new() });
        let online = st.mutual_online(key);
        for f in &online {
            st.peers[f].send(ServerMsg::Presence { key: key.to_string(), online: true });
        }
        Ok(ServerMsg::Welcome { code, code_ttl, online })
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
        if !allow(&mut st.ip_redeems, ip, self.limits.redeems_per_ip_per_min, Duration::from_secs(60)) {
            return fail("rate");
        }
        let Some(norm) = proto::normalize_code(code) else { return fail("unknown") };
        let now = unix_now();
        let Some(target) = st.store_ref().by_code(&norm, now, self.limits.code_ttl.as_secs()).map(str::to_string) else {
            return fail("unknown");
        };
        if target == key {
            return fail("self");
        }
        let Some(id) = st.peers.get(&target).map(|p| p.id.clone()) else { return fail("offline") };
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
    pub(crate) fn maintain(&self) {
        let mut st = self.state();
        let now = Instant::now();
        st.grants.retain(|g| g.until > now);
        st.store().prune(unix_now());
        if let Err(e) = st.store().flush() {
            eprintln!("screenpet-relay: cannot save data: {e}");
        }
    }

    /// Number of installs online (for logs/tests).
    pub fn online_count(&self) -> usize {
        self.state().peers.len()
    }
}
