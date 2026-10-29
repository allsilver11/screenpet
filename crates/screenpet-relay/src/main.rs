//! `screenpet-relay [--bind ADDR] [--data DIR] [--trusted-proxy IP]...`
//!
//! Defaults: `--bind 0.0.0.0:9411`, `--data $XDG_DATA_HOME/screenpet-relay`
//! (or `~/.local/share/screenpet-relay`), trusted proxy `192.168.0.52` (the
//! Nginx Proxy Manager; loopback is always trusted). Environment:
//! `SCREENPET_RELAY_BIND`, `SCREENPET_RELAY_DATA`,
//! `SCREENPET_RELAY_TRUSTED_PROXIES` (comma-separated; empty = loopback only).
//! Health check: `GET /healthz`; WebSocket: `/ws`.

use screenpet_relay::{Relay, RelayConfig, DEFAULT_PORT, DEFAULT_TRUSTED_PROXY};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

fn default_data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("screenpet-relay");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/screenpet-relay")
}

fn usage() -> ! {
    eprintln!("usage: screenpet-relay [--bind ADDR:PORT] [--data DIR] [--trusted-proxy IP]...");
    std::process::exit(2);
}

fn main() {
    let mut bind: SocketAddr = std::env::var("SCREENPET_RELAY_BIND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)));
    let mut data = std::env::var_os("SCREENPET_RELAY_DATA").map(PathBuf::from).unwrap_or_else(default_data_dir);
    let mut proxies: Option<Vec<IpAddr>> = std::env::var("SCREENPET_RELAY_TRUSTED_PROXIES").ok().map(|s| {
        s.split(',').filter(|p| !p.trim().is_empty()).map(|p| p.trim().parse().unwrap_or_else(|_| usage())).collect()
    });
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--trusted-proxy" => {
                let ip: IpAddr = args.next().and_then(|s| s.parse().ok()).unwrap_or_else(|| usage());
                proxies.get_or_insert_with(Vec::new).push(ip);
            }
            "--bind" => bind = args.next().and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()),
            "--data" => data = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }
    let mut cfg = RelayConfig::new(bind, Some(data.clone()));
    cfg.trusted_proxies = proxies.unwrap_or_else(|| vec![DEFAULT_TRUSTED_PROXY]);
    let shown: Vec<String> = cfg.trusted_proxies.iter().map(IpAddr::to_string).collect();
    eprintln!("screenpet-relay: trusting forwarded client addresses from loopback and [{}]", shown.join(", "));
    match Relay::start(cfg) {
        Ok(relay) => {
            eprintln!(
                "screenpet-relay listening on {} (data {}), relay key {}",
                relay.local_addr(),
                data.display(),
                screenpet_relay::proto::to_hex(&relay.public_key())
            );
            relay.wait();
        }
        Err(e) => {
            eprintln!("screenpet-relay: cannot start: {e}");
            std::process::exit(1);
        }
    }
}
