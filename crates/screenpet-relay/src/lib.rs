//! ScreenPet relay server (spec phase 3 §1b).
//!
//! An untrusted pipe between friends who are not on the same network. Each
//! install keeps one WebSocket to the relay and authenticates with its Noise
//! static key (challenge + X25519-derived MAC, see [`proto`]), so nobody can
//! take over another install's mailbox. Installs are addressed by their
//! static public key. The relay connects two installs with a *channel* only
//! if both list each other as friends, or once after one redeemed the other's
//! friend code (for pairing). Channel frames are opaque: the friends run their
//! Noise XX session end to end through the relay, so it sees only ciphertext
//! and cannot forge friend traffic; pairing is confirmed by the 6-emoji SAS
//! on both screens, never by the relay.
//!
//! Stored (`relay.json`): per install key the id label, its friend key list
//! and its current friend code. Nothing else, no message content.
//!
//! Plain std threads and synchronous `tungstenite`; TLS terminates at the
//! reverse proxy (Nginx Proxy Manager) in front of it.

#[cfg(unix)]
pub mod admin;
mod conn;
pub mod control;
pub(crate) mod board;
pub(crate) mod mailbox;
mod downloads;
mod http;
// The admin commands live here but are only reachable over a Unix socket.
#[cfg_attr(not(unix), allow(dead_code))]
mod hub;
pub mod proto;
pub mod store;
pub mod weather;

use hub::Hub;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Default listening port.
pub const DEFAULT_PORT: u16 = 9411;

/// Limits (spec §1b defaults via `Default`; tests use smaller ones).
#[derive(Debug, Clone)]
pub struct Limits {
    /// Largest control (text) message.
    pub max_text: usize,
    /// Messages per second per connection (all kinds) and burst.
    pub conn_msgs_per_sec: f64,
    pub conn_msgs_burst: f64,
    /// Bytes per second per connection and burst.
    pub conn_bytes_per_sec: f64,
    pub conn_bytes_burst: f64,
    /// Frames per second per channel direction (spec: 20/s) and burst.
    pub chan_msgs_per_sec: f64,
    pub chan_msgs_burst: f64,
    /// Friend keys per install (spec: 50).
    pub max_friends: usize,
    /// Open channels per connection, and between one pair of installs.
    pub max_channels: usize,
    pub max_channels_per_pair: usize,
    /// Nothing received for this long → disconnect (clients ping every 20 s).
    pub idle_timeout: Duration,
    /// Authentication must finish within this.
    pub auth_timeout: Duration,
    /// Friend code lifetime (spec: 24 h).
    pub code_ttl: Duration,
    /// A redeemed code allows one channel within this time.
    pub grant_ttl: Duration,
    pub redeems_per_ip_per_min: usize,
    /// All redeems per minute (whole relay).
    pub redeems_per_min: usize,
    /// Successful redeems of one install's codes per hour.
    pub redeems_per_target_per_hour: usize,
    /// New installs per hour (whole relay, a loose backstop), and stored
    /// installs at most (friendless offline installs are evicted first).
    pub registrations_per_hour: usize,
    pub max_installs: usize,
    pub max_conns: usize,
    pub max_conns_per_ip: usize,
    /// Connections per network (IPv4 /24, IPv6 /48).
    pub max_conns_per_network: usize,
    /// Connections that have not authenticated yet: in all, per address, per
    /// network.
    pub max_unauthed: usize,
    pub max_unauthed_per_ip: usize,
    pub max_unauthed_per_network: usize,
    /// Bytes of friend frames forwarded per second, per address and in all.
    pub ip_bytes_per_sec: f64,
    pub ip_bytes_burst: f64,
    pub global_bytes_per_sec: f64,
    pub global_bytes_burst: f64,
    /// Bytes of friend frames waiting for one connection and for all of them
    /// (over: the channel is closed).
    pub max_outbox_bytes: usize,
    pub max_total_outbox_bytes: usize,
    /// A connection whose queue is over `max_outbox_bytes` and hasn't moved
    /// for this long is disconnected as `slow`.
    pub slow_after: Duration,
    /// New installs per hour per network (IPv4 /24, IPv6 /64).
    pub registrations_per_ip_per_hour: usize,
    /// New installs per day per wider network (IPv4 /24, IPv6 /48).
    pub registrations_per_network_per_day: usize,
    /// Different friend codes redeemed per hour per network (IPv4 /24, IPv6 /48).
    pub new_codes_per_network_per_hour: usize,
    /// Write changed data this often (and at shutdown).
    pub save_every: Duration,
    /// 📮 `board_list` requests closer together than this on one connection are ignored.
    pub board_gap: Duration,
    /// 🔄 Installers sent at once from `/downloads/` (more: `503 busy`).
    pub max_downloads: usize,
    /// … and from one client address (IPv6: its /64).
    pub max_downloads_per_ip: usize,
    /// A download is cut off after this long in all, …
    pub download_deadline: Duration,
    /// … or when, after `download_grace`, it has averaged under this many
    /// bytes per second, …
    pub download_min_rate: u64,
    pub download_grace: Duration,
    /// … or when one write stalls this long (a client that stopped reading).
    pub download_stall: Duration,
    /// Forget stale installs this often; the first pass is this long after
    /// start, once clients that were online have had time to come back.
    pub prune_every: Duration,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_text: 16 * 1024,
            conn_msgs_per_sec: 200.0,
            conn_msgs_burst: 400.0,
            conn_bytes_per_sec: 512.0 * 1024.0,
            conn_bytes_burst: 1024.0 * 1024.0,
            chan_msgs_per_sec: 20.0,
            chan_msgs_burst: 40.0,
            max_friends: screenpet_proto::MAX_FRIENDS,
            max_channels: 64,
            max_channels_per_pair: 2,
            idle_timeout: Duration::from_secs(90),
            auth_timeout: Duration::from_secs(10),
            code_ttl: Duration::from_secs(24 * 3600),
            grant_ttl: Duration::from_secs(60),
            redeems_per_ip_per_min: 30,
            redeems_per_min: 600,
            redeems_per_target_per_hour: 10,
            registrations_per_hour: 2000,
            max_installs: store::MAX_INSTALLS,
            max_conns: 2048,
            max_conns_per_ip: 16,
            max_conns_per_network: 128,
            max_unauthed: 512,
            max_unauthed_per_ip: 8,
            max_unauthed_per_network: 32,
            ip_bytes_per_sec: 2.0 * 1024.0 * 1024.0,
            ip_bytes_burst: 4.0 * 1024.0 * 1024.0,
            global_bytes_per_sec: 16.0 * 1024.0 * 1024.0,
            global_bytes_burst: 32.0 * 1024.0 * 1024.0,
            max_outbox_bytes: 2 * 1024 * 1024,
            max_total_outbox_bytes: 256 * 1024 * 1024,
            slow_after: Duration::from_secs(5),
            registrations_per_ip_per_hour: 30,
            registrations_per_network_per_day: 100,
            new_codes_per_network_per_hour: 64,
            save_every: Duration::from_secs(30),
            board_gap: Duration::from_millis(500),
            max_downloads: 8,
            max_downloads_per_ip: 2,
            // 100 MB at the minimum rate takes 26 min.
            download_deadline: Duration::from_secs(30 * 60),
            download_min_rate: 64 * 1024,
            download_grace: Duration::from_secs(20),
            download_stall: Duration::from_secs(10),
            prune_every: Duration::from_secs(600),
        }
    }
}

#[derive(Clone)]
pub struct RelayConfig {
    pub bind: SocketAddr,
    /// Where `relay.json` and `relay.key` live; `None` = in memory with a
    /// random relay key (tests).
    pub data_dir: Option<PathBuf>,
    pub limits: Limits,
    /// Reverse proxies (besides loopback) whose `X-Real-IP` /
    /// `X-Forwarded-For` headers are believed; every other peer is limited
    /// by its socket address.
    pub trusted_proxies: Vec<IpAddr>,
    /// ☀️ Fetches the weather of a place (`weather::open_meteo` in the real
    /// relay). `None` = no weather is fetched or sent (tests).
    pub weather_fetch: Option<weather::Fetch>,
}

/// The user's Nginx Proxy Manager.
pub const DEFAULT_TRUSTED_PROXY: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 52));

impl RelayConfig {
    pub fn new(bind: SocketAddr, data_dir: Option<PathBuf>) -> RelayConfig {
        RelayConfig { bind, data_dir, limits: Limits::default(), trusted_proxies: vec![DEFAULT_TRUSTED_PROXY], weather_fetch: None }
    }
}

/// A running relay. Dropping it stops all threads.
pub struct Relay {
    hub: Arc<Hub>,
    addr: SocketAddr,
    threads: Vec<JoinHandle<()>>,
    data_dir: Option<PathBuf>,
}

impl Relay {
    pub fn start(mut cfg: RelayConfig) -> std::io::Result<Relay> {
        cfg.limits.max_conns = conns_within_fd_limit(cfg.limits.max_conns);
        let (key, store, control) = match &cfg.data_dir {
            // A bad control file ends the start here, before anyone connects.
            Some(dir) => (store::load_or_create_key(dir)?, store::Store::open(dir)?, control::Control::load(dir)?),
            None => (proto::random32(), store::Store::memory(), control::Control::default()),
        };
        let listener = TcpListener::bind(cfg.bind)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let downloads = Arc::new(downloads::Downloads::new(cfg.data_dir.as_ref().map(|d| d.join("downloads")), &cfg.limits));
        let hub = Arc::new(Hub::new(cfg.limits, cfg.trusted_proxies, key, store, control, cfg.data_dir.clone()));
        let h = hub.clone();
        let accept = std::thread::Builder::new().name("relay-accept".into()).spawn(move || accept_loop(h, downloads, listener))?;
        let h = hub.clone();
        let upkeep = std::thread::Builder::new().name("relay-upkeep".into()).spawn(move || upkeep_loop(h))?;
        let mut threads = vec![accept, upkeep];
        if let Some(fetch) = cfg.weather_fetch.clone() {
            let h = hub.clone();
            threads.push(std::thread::Builder::new().name("relay-weather".into()).spawn(move || weather_loop(h, fetch))?);
        }
        // The relay is worth running even if its admin socket can't be made.
        #[cfg(unix)]
        if let Some(dir) = &cfg.data_dir {
            match admin::bind(dir) {
                Ok(listener) => {
                    let h = hub.clone();
                    threads.push(std::thread::Builder::new().name("relay-admin".into()).spawn(move || admin::serve(h, listener))?);
                }
                Err(e) => eprintln!("screenpet-relay: no admin socket in {}: {e}", dir.display()),
            }
        }
        Ok(Relay { hub, addr, threads, data_dir: cfg.data_dir })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The relay's static public key (clients learn it from the challenge).
    pub fn public_key(&self) -> [u8; 32] {
        self.hub.public_key
    }

    pub fn online_count(&self) -> usize {
        self.hub.online_count()
    }

    /// Block until the process is killed.
    pub fn wait(mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.hub.stop();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        self.hub.join_all();
        self.hub.save();
        #[cfg(unix)]
        if let Some(dir) = &self.data_dir {
            let _ = std::fs::remove_file(admin::socket_path(dir));
        }
    }
}

/// File descriptors kept for everything but connections (the listener, data
/// files, the connection threads over `max_conns` that are about to refuse).
const FDS_RESERVED: usize = 64;

/// What fits in a file descriptor limit of `soft`.
fn conns_for_fds(max_conns: usize, soft: u64) -> usize {
    max_conns.min(usize::try_from(soft).unwrap_or(usize::MAX).saturating_sub(FDS_RESERVED))
}

/// Raise the soft file descriptor limit as far as `max_conns` needs and the
/// hard limit allows (the usual soft default is 1024); if that isn't enough,
/// fewer connections are admitted, so running out shows up as `busy` and not
/// as `accept` failing for everyone.
#[cfg(unix)]
fn conns_within_fd_limit(max_conns: usize) -> usize {
    let mut lim = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `lim` is valid for both calls.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut lim) != 0 {
            return max_conns;
        }
        let want = (max_conns + FDS_RESERVED) as libc::rlim_t;
        if lim.rlim_cur < want {
            let raised = libc::rlimit { rlim_cur: want.min(lim.rlim_max), rlim_max: lim.rlim_max };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &raised) == 0 {
                lim = raised;
            }
        }
    }
    let conns = conns_for_fds(max_conns, lim.rlim_cur as u64);
    if conns < max_conns {
        eprintln!("screenpet-relay: file descriptor limit {} allows only {conns} connections", lim.rlim_cur);
    }
    conns
}

#[cfg(not(unix))]
fn conns_within_fd_limit(max_conns: usize) -> usize {
    max_conns
}

fn accept_loop(hub: Arc<Hub>, downloads: Arc<downloads::Downloads>, listener: TcpListener) {
    let mut last_accept_log = None;
    while !hub.stopping() {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let (h, d) = (hub.clone(), downloads.clone());
                if !hub.spawn("relay-conn", move || http::handle(h, d, stream)) {
                    hub::bump(&hub.counters.threads_refused);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                // Out of file descriptors, most likely: say so, but not 10 times a second.
                hub::bump(&hub.counters.accept_errors);
                if last_accept_log.is_none_or(|t: Instant| t.elapsed() >= Duration::from_secs(60)) {
                    last_accept_log = Some(Instant::now());
                    eprintln!("screenpet-relay: cannot accept connections: {e}");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Timed work, on its own thread so a slow disk never holds up `accept`.
fn upkeep_loop(hub: Arc<Hub>) {
    const MAINTAIN_EVERY: Duration = Duration::from_secs(60);
    let (mut maintained, mut pruned, mut saved) = (Instant::now(), Instant::now(), Instant::now());
    while !hub.stopping() {
        std::thread::sleep(Duration::from_millis(50));
        if maintained.elapsed() >= MAINTAIN_EVERY {
            maintained = Instant::now();
            hub.maintain();
        }
        if pruned.elapsed() >= hub.limits.prune_every {
            pruned = Instant::now();
            hub.prune_store();
        }
        if saved.elapsed() >= hub.limits.save_every {
            saved = Instant::now();
            hub.save();
        }
    }
}

/// How often the weather thread looks for places to fetch.
const WEATHER_ROUND: Duration = Duration::from_secs(2);

/// ☀️ Fetches due weather places one at a time, on its own thread: a slow
/// weather service never holds up connections. Errors are logged (at most
/// once every 10 minutes) and never stop the relay.
fn weather_loop(hub: Arc<Hub>, fetch: weather::Fetch) {
    let mut last_round = Instant::now() - WEATHER_ROUND;
    let mut last_error_log: Option<Instant> = None;
    while !hub.stopping() {
        std::thread::sleep(Duration::from_millis(50));
        if last_round.elapsed() < WEATHER_ROUND {
            continue;
        }
        last_round = Instant::now();
        let due = hub.weather_due();
        let mut places = due.into_iter();
        for place in places.by_ref() {
            if hub.stopping() {
                hub.weather_fetched(place.id, None);
                break;
            }
            let code = match fetch(place) {
                Ok(code) => Some(code),
                Err(e) => {
                    if last_error_log.is_none_or(|t| t.elapsed() >= Duration::from_secs(600)) {
                        last_error_log = Some(Instant::now());
                        eprintln!("screenpet-relay: weather for {} failed: {e}", place.id);
                    }
                    None
                }
            };
            hub.weather_fetched(place.id, code);
        }
        // Left over after a stop: not busy any more (nothing to wait for).
        for place in places {
            hub.weather_fetched(place.id, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connections_fit_in_the_file_descriptor_limit() {
        assert_eq!(conns_for_fds(2048, 1024), 960);
        assert_eq!(conns_for_fds(2048, 8192), 2048);
        assert_eq!(conns_for_fds(2048, 10), 0);
    }
}
