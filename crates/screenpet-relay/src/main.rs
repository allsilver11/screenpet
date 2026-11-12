//! `screenpet-relay [--bind ADDR] [--data DIR] [--trusted-proxy IP]...`
//!
//! Defaults: `--bind 0.0.0.0:9411`, `--data $XDG_DATA_HOME/screenpet-relay`
//! (or `~/.local/share/screenpet-relay`), trusted proxy `192.168.0.52` (the
//! Nginx Proxy Manager; loopback is always trusted). Environment:
//! `SCREENPET_RELAY_BIND`, `SCREENPET_RELAY_DATA`,
//! `SCREENPET_RELAY_TRUSTED_PROXIES` (comma-separated; empty = loopback only),
//! `SCREENPET_RELAY_WEATHER=off` (no Open-Meteo fetches).
//! Health check: `GET /healthz`; WebSocket: `/ws`.
//!
//! `screenpet-relay admin [--data DIR] <command>` runs an operator command on
//! the relay using that data directory (see `admin::USAGE`).

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
    eprintln!("       screenpet-relay admin [--data DIR] <command>   (admin --help lists them)");
    std::process::exit(2);
}

/// SIGTERM and SIGINT, blocked in every thread and collected by `wait`: no
/// signal handler to get wrong, and shutting down is ordinary code.
#[cfg(unix)]
mod signals {
    pub fn block() -> libc::sigset_t {
        // SAFETY: the set is initialized by sigemptyset before use.
        unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut set);
            libc::sigaddset(&mut set, libc::SIGTERM);
            libc::sigaddset(&mut set, libc::SIGINT);
            libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
            set
        }
    }

    pub fn wait(set: &libc::sigset_t) {
        let mut sig = 0;
        // SAFETY: both pointers are valid for the call.
        unsafe { libc::sigwait(set, &mut sig) };
    }
}

#[cfg(unix)]
fn run_admin(mut data: PathBuf, mut words: Vec<String>) -> ! {
    if words.first().is_some_and(|w| w == "--data") && words.len() >= 2 {
        data = PathBuf::from(words.remove(1));
        words.remove(0);
    }
    if words.is_empty() || words[0] == "-h" || words[0] == "--help" {
        println!("usage: screenpet-relay admin [--data DIR] <command>\n{}", screenpet_relay::admin::USAGE);
        std::process::exit(if words.is_empty() { 2 } else { 0 });
    }
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match screenpet_relay::admin::request(&data, &words) {
        Ok((true, text)) => {
            println!("{}", text.trim_end());
            std::process::exit(0);
        }
        Ok((false, text)) => {
            eprintln!("{}", text.trim_end());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("cannot reach the relay through {}: {e}", data.display());
            std::process::exit(2);
        }
    }
}

fn main() {
    // Before any thread starts, so all of them inherit the mask.
    #[cfg(unix)]
    let stop_signals = signals::block();
    let mut bind: SocketAddr = std::env::var("SCREENPET_RELAY_BIND")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT)));
    let mut data = std::env::var_os("SCREENPET_RELAY_DATA").map(PathBuf::from).unwrap_or_else(default_data_dir);
    let mut proxies: Option<Vec<IpAddr>> = std::env::var("SCREENPET_RELAY_TRUSTED_PROXIES").ok().map(|s| {
        s.split(',').filter(|p| !p.trim().is_empty()).map(|p| p.trim().parse().unwrap_or_else(|_| usage())).collect()
    });
    if std::env::args().nth(1).as_deref() == Some("admin") {
        #[cfg(unix)]
        run_admin(data, std::env::args().skip(2).collect());
        #[cfg(not(unix))]
        {
            eprintln!("admin commands need a Unix socket");
            std::process::exit(2);
        }
    }
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
    // ☀️ Real weather for the apps (SCREENPET_RELAY_WEATHER=off turns it off)
    if std::env::var("SCREENPET_RELAY_WEATHER").as_deref() != Ok("off") {
        cfg.weather_fetch = Some(screenpet_relay::weather::open_meteo());
    }
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
            // Dropping the relay tells clients, ends the threads and saves.
            #[cfg(unix)]
            {
                signals::wait(&stop_signals);
                eprintln!("screenpet-relay: stopping");
                drop(relay);
            }
            #[cfg(not(unix))]
            relay.wait();
        }
        Err(e) => {
            eprintln!("screenpet-relay: cannot start: {e}");
            std::process::exit(1);
        }
    }
}
