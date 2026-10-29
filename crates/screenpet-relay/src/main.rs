//! `screenpet-relay [--bind ADDR] [--data DIR]`
//!
//! Defaults: `--bind 0.0.0.0:9411`, `--data $XDG_DATA_HOME/screenpet-relay`
//! (or `~/.local/share/screenpet-relay`). Environment: `SCREENPET_RELAY_BIND`,
//! `SCREENPET_RELAY_DATA`. Health check: `GET /healthz`; WebSocket: `/ws`.

use screenpet_relay::{Relay, RelayConfig, DEFAULT_PORT};
use std::net::SocketAddr;
use std::path::PathBuf;

fn default_data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("screenpet-relay");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/screenpet-relay")
}

fn usage() -> ! {
    eprintln!("usage: screenpet-relay [--bind ADDR:PORT] [--data DIR]");
    std::process::exit(2);
}

fn main() {
    let mut bind: SocketAddr = std::env::var("SCREENPET_RELAY_BIND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)));
    let mut data = std::env::var_os("SCREENPET_RELAY_DATA").map(PathBuf::from).unwrap_or_else(default_data_dir);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--bind" => bind = args.next().and_then(|s| s.parse().ok()).unwrap_or_else(|| usage()),
            "--data" => data = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }
    match Relay::start(RelayConfig::new(bind, Some(data.clone()))) {
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
