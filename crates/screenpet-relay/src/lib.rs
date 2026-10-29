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

mod conn;
mod http;
mod hub;
pub mod proto;
pub mod store;

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
    /// New installs per hour per network (IPv4 /24, IPv6 /64).
    pub registrations_per_ip_per_hour: usize,
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
            max_friends: 50,
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
            registrations_per_ip_per_hour: 30,
        }
    }
}

#[derive(Debug, Clone)]
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
}

/// The user's Nginx Proxy Manager.
pub const DEFAULT_TRUSTED_PROXY: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 52));

impl RelayConfig {
    pub fn new(bind: SocketAddr, data_dir: Option<PathBuf>) -> RelayConfig {
        RelayConfig { bind, data_dir, limits: Limits::default(), trusted_proxies: vec![DEFAULT_TRUSTED_PROXY] }
    }
}

/// A running relay. Dropping it stops all threads.
pub struct Relay {
    hub: Arc<Hub>,
    addr: SocketAddr,
    accept: Option<JoinHandle<()>>,
}

impl Relay {
    pub fn start(cfg: RelayConfig) -> std::io::Result<Relay> {
        let (key, store) = match &cfg.data_dir {
            Some(dir) => (store::load_or_create_key(dir)?, store::Store::open(dir)?),
            None => (proto::random32(), store::Store::memory()),
        };
        let listener = TcpListener::bind(cfg.bind)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let hub = Arc::new(Hub::new(cfg.limits, cfg.trusted_proxies, key, store));
        let h = hub.clone();
        let accept = std::thread::Builder::new().name("relay-accept".into()).spawn(move || accept_loop(h, listener))?;
        Ok(Relay { hub, addr, accept: Some(accept) })
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

    /// Block until the process is killed (the binary's main loop).
    pub fn wait(mut self) {
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.hub.stop();
        if let Some(t) = self.accept.take() {
            let _ = t.join();
        }
        self.hub.join_all();
        self.hub.maintain();
    }
}

fn accept_loop(hub: Arc<Hub>, listener: TcpListener) {
    let mut last_maintain = Instant::now();
    while !hub.stopping() {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let h = hub.clone();
                hub.spawn("relay-conn", move || http::handle(h, stream));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
        if last_maintain.elapsed() >= Duration::from_secs(2) {
            last_maintain = Instant::now();
            hub.maintain();
        }
    }
}
