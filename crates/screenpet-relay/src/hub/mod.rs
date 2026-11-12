//! 릴레이 허브: 누가 접속했는지, 그 사이의 채널, 친구 코드로 받은 허가, 주소별 한도를 들고 있음.
//! 연결 스레드끼리는 `Out` 큐로만 말함 (허브 잠금 안에서 막히지 않음).
//!
//! 영역별 파일:
//! - `presence`: 연결 받기, 로그인, 친구·온라인 알림, 친구 코드, 저장소
//! - `routing`: 채널 열기·닫기, 프레임 넘기기, 대역폭·큐 한도
//! - `mail`: 📮 편지함과 공개 건의함
//! - `weather`: ☀️ 지역별 날씨 나눠 주기
//! - `admin`: 관리 명령
//! - `stats`: 카운터와 `admin status`
//! - `limits`: 토큰 버킷과 속도 표
//!
//! ## 잠금
//! - `state`: 접속·채널·허가·한도 표·스위치·날씨. 거의 모든 일이 이것 하나로 묶임
//!   (접속과 채널은 서로 얽혀 있어 나누면 순서가 꼬일 수 있음).
//! - `mail`: 편지함. 언제나 *마지막*에 잡는 잎 잠금임: `state`를 잡은 채 `mail`을 잡을 수는 있지만
//!   `mail`을 잡은 채 다른 잠금을 잡지 않음.
//! - `mailbox_save`: `letters.json` 쓰기 순서. `state`를 놓은 뒤에만 잡고, 그 안에서는 `mail`만 잠깐 잡음.
//! - `threads`: 스레드 목록. 다른 잠금과 섞이지 않음.
//!
//! 순서: `mailbox_save` → `mail`, `state` → `mail`. 거꾸로는 없음.
//! 카운터와 큐 크기는 원자 값이라 잠금이 필요 없음.

mod admin;
mod limits;
mod mail;
mod presence;
mod routing;
mod stats;
mod weather;

pub(crate) use limits::Bucket;
pub(crate) use stats::{bump, Counters};
use limits::*;
use weather::weather_msg;

use crate::control::Control;
use crate::mailbox::Mailbox;
use crate::proto::{self, ServerMsg};
use crate::store::{unix_now, Store};
use crate::weather::{Reading, Weathers, DEFAULT_PLACE};
use crate::Limits;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Something for a connection thread to send.
pub(crate) enum Out {
    Text(String),
    Binary(Vec<u8>),
    /// Send `error{reason}` and disconnect.
    Kick(String),
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
    /// 연결 스레드의 큐로 손잡이를 만듦
    pub fn new(out: SyncSender<Out>, admission: &Admission) -> Link {
        Link {
            out,
            kill: admission.kill.clone(),
            queued: Arc::default(),
            net_queued: admission.net_queued.clone(),
            drained_at: Arc::default(),
        }
    }

    /// 지금 `reason`으로 끊음. 큐가 막혀 있을 수 있어 `kill`이 바로 끊게 함
    fn kick(&self, reason: &'static str) {
        let _ = self.out.try_send(Out::Kick(reason.into()));
        let _ = self.kill.set(reason);
    }
}

struct Peer {
    conn: u64,
    id: String,
    /// Address key and connect time: only for `admin status`, never stored.
    ip: String,
    since: u64,
    link: Link,
    channels: HashSet<u32>,
    /// ☀️ The built-in place whose weather this app wants (Yongin until it says otherwise).
    weather: &'static str,
    /// 📮 When this connection last got a board page.
    board_at: Option<Instant>,
}

impl Peer {
    /// 텍스트 메시지를 큐에 넣음. 큐가 차 있으면 `slow`로 끊음
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

/// Why `admit_ip` said no.
pub(crate) enum Refused {
    Busy,
    Banned,
    RelayOff,
}

/// What `welcome` and `code` carry while friend codes are off. The apps
/// require the field and show it, so it should read as "no code".
pub(crate) const NO_CODE: &str = "----";

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
    /// 연결 하나를 셈 (아직 인증 전)
    fn admit(&mut self) {
        self.total += 1;
        self.unauthed += 1;
    }
    /// 인증 전 연결 수를 하나 줄임
    fn authed(&mut self) {
        self.unauthed = self.unauthed.saturating_sub(1);
    }
    /// 연결 하나가 끝남. 남은 것이 없으면 true임
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
    control: Control,
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
    /// ☀️ Weather places and their last readings.
    weathers: Weathers,
}

impl State {
    /// 설치 저장소 (늘 있음)
    fn store(&mut self) -> &mut Store {
        self.store.as_mut().expect("store present")
    }
    /// 설치 저장소 (읽기만)
    fn store_ref(&self) -> &Store {
        self.store.as_ref().expect("store present")
    }
    /// `key`와 서로 친구이면서 접속 중인 설치들
    fn mutual_online(&self, key: &str) -> Vec<String> {
        let Some(inst) = self.store_ref().get(key) else { return Vec::new() };
        inst.friends.iter().filter(|f| self.peers.contains_key(*f) && self.store_ref().mutual(key, f)).cloned().collect()
    }
    /// 연결 `conn`이 더는 인증 전으로 세지 않게 함 (인증, 끝남, 내보냄). 이미 그랬으면 None임
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
    /// `pick`이 고른 가장 오래된 인증 전 연결을 내보냄 (진짜 앱은 한 왕복 안에 답하니 가장 오래된 것이 가짜일 가능성이 큼)
    fn evict_unauthed(&mut self, counters: &Counters, pick: impl Fn(&Unauthed) -> bool) {
        let oldest = self.unauthed.iter().find(|(_, u)| pick(u)).map(|(conn, _)| *conn);
        if let Some(u) = oldest.and_then(|conn| self.no_longer_unauthed(conn)) {
            let _ = u.kill.set("busy");
            bump(&counters.unauthed_evicted);
        }
    }
    /// `key`를 지금 내보냄: `reason`을 알리고 채널을 닫고 친구에게 알림. 접속 중이 아니었으면 false임
    fn drop_peer(&mut self, key: &str, reason: &'static str) -> bool {
        let online = self.mutual_online(key);
        let Some(peer) = self.peers.remove(key) else { return false };
        peer.link.kick(reason);
        for ch in peer.channels {
            self.close_channel(ch, Some(key));
        }
        for f in online {
            if let Some(p) = self.peers.get(&f) {
                p.send(ServerMsg::Presence { key: key.to_string(), online: false });
            }
        }
        self.store().touch(key, unix_now());
        true
    }
    /// `who`(키나 id 앞부분)가 가리키는 설치 하나. 없거나 여럿이면 Err임
    fn resolve(&self, who: &str) -> Result<String, String> {
        let matches = self.matching(who);
        match matches.as_slice() {
            [] => Err(format!("no install matches {who:?}")),
            [one] => Ok(one.clone()),
            many => Err(format!("{who:?} matches {} installs, nothing done:\n{}", many.len(), self.describe(many))),
        }
    }
    /// `who`에 맞는 설치 키들 (짧은 키 앞부분은 받지 않고, 짧은 id는 정확히 같아야 함)
    fn matching(&self, who: &str) -> Vec<String> {
        let lower = who.to_ascii_lowercase();
        // Short key prefixes are too easy to get wrong; a short id must be exact.
        let long = who.len() >= MIN_PREFIX;
        let hit = |k: &str, id: &str| id == who || (long && (k.starts_with(&lower) || id.starts_with(who)));
        self.store_ref().installs().filter(|(k, i)| hit(k, &i.id)).map(|(k, _)| k.clone()).collect()
    }
    /// 키 목록을 사람이 읽게 한 줄씩 적음
    fn describe(&self, keys: &[String]) -> String {
        let line = |k: &String| {
            let id = self.store_ref().get(k).map_or("", |i| i.id.as_str());
            format!("  {k} {id:?}{}", if self.peers.contains_key(k) { " online" } else { "" })
        };
        keys.iter().map(line).collect::<Vec<_>>().join("\n")
    }
    /// 채널 `ch`를 지우고 다른 끝에 알림 (`except` = 닫자고 한 끝, 알릴 필요 없음)
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

/// Shortest prefix `admin` accepts for a key or id.
pub(super) const MIN_PREFIX: usize = 4;

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
    data_dir: Option<PathBuf>,
    /// Mirrors `control.relay` for connection threads, which don't lock to check.
    relay_off: AtomicBool,
    next_conn: AtomicU64,
    threads: Mutex<Vec<JoinHandle<()>>>,
    /// 📮 개발자에게 온 건의 편지 (`letters.json`). 잎 잠금임 (모듈 설명 참고)
    mail: Mutex<Mailbox>,
    /// Held across taking a mailbox snapshot and writing it, so two
    /// connection threads can't write an older snapshot over a newer one.
    mailbox_save: Mutex<()>,
}

impl Hub {
    /// 허브를 만듦. 편지함은 `data_dir`에서 읽음 (없으면 빈 편지함)
    pub(crate) fn new(
        limits: Limits,
        trusted_proxies: Vec<std::net::IpAddr>,
        private_key: [u8; 32],
        mut store: Store,
        control: Control,
        data_dir: Option<PathBuf>,
    ) -> Hub {
        store.max_installs = limits.max_installs;
        let relay_off = !control.relay;
        let global_bytes = Bucket::new(limits.global_bytes_per_sec, limits.global_bytes_burst);
        let global_bytes_hard = Bucket::new(2.0 * limits.global_bytes_per_sec, 2.0 * limits.global_bytes_burst);
        Hub {
            limits,
            trusted_proxies,
            private_key,
            public_key: proto::public_key(&private_key),
            state: Mutex::new(State {
                store: Some(store),
                next_ch: 1,
                global_bytes,
                global_bytes_hard,
                control,
                ..State::default()
            }),
            mail: Mutex::new(data_dir.as_deref().map(Mailbox::load).unwrap_or_default()),
            data_dir,
            relay_off: AtomicBool::new(relay_off),
            stop: AtomicBool::new(false),
            proxy_warned: AtomicBool::new(false),
            queued_total: AtomicUsize::new(0),
            started: Instant::now(),
            counters: Counters::default(),
            next_conn: AtomicU64::new(1),
            threads: Mutex::new(Vec::new()),
            mailbox_save: Mutex::new(()),
        }
    }

    /// 상태 잠금을 잡음 (독이 든 잠금도 그대로 씀)
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 편지함 잠금을 잡음. 잎 잠금이라 잡은 채 다른 잠금을 잡지 말 것
    fn mail(&self) -> MutexGuard<'_, Mailbox> {
        self.mail.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 멈추는 중인지 알려 줌
    pub fn stopping(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// `admin relay off` 상태인지 알려 줌 (잠금 없이)
    pub(crate) fn relay_is_off(&self) -> bool {
        self.relay_off.load(Ordering::SeqCst)
    }

    /// 모든 스레드에 멈추라고 알림
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// 새 연결 번호를 줌
    pub fn conn_id(&self) -> u64 {
        self.next_conn.fetch_add(1, Ordering::Relaxed)
    }

    /// 이름 붙은 스레드를 띄움. 스레드가 너무 많으면 띄우지 않고 false임
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

    /// 띄운 스레드가 모두 끝날 때까지 기다림
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

    /// `key`의 현재 연결이 `conn`인지 봄 (밀려난 연결이면 false임)
    fn is_current(st: &State, key: &str, conn: u64) -> bool {
        st.peers.get(key).is_some_and(|p| p.conn == conn)
    }

    /// 접속 중인 설치 수 (로그·시험용)
    pub fn online_count(&self) -> usize {
        self.state().peers.len()
    }
}

#[cfg(test)]
mod tests;
