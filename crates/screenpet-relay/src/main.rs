//! 릴레이 실행 파일임: `screenpet-relay [--bind ADDR] [--data DIR] [--trusted-proxy IP]...`
//!
//! 기본값: `--bind 0.0.0.0:9411`, `--data $XDG_DATA_HOME/screenpet-relay`(또는 `~/.local/share/screenpet-relay`),
//! 믿는 프록시 `192.168.0.52`(Nginx Proxy Manager, 루프백은 늘 믿음).
//! 환경 변수: `SCREENPET_RELAY_BIND`, `SCREENPET_RELAY_DATA`, `SCREENPET_RELAY_TRUSTED_PROXIES`(쉼표 구분, 빈 값 = 루프백만),
//! `SCREENPET_RELAY_WEATHER=off`(Open-Meteo 받아 오기 안 함). 상태 확인: `GET /healthz`, WebSocket: `/ws`.
//!
//! `screenpet-relay admin [--data DIR] <command>`는 그 데이터 폴더를 쓰는 릴레이에 운영 명령을 돌림 (`admin::USAGE` 참고).

use screenpet_log::log;
use screenpet_relay::{Relay, RelayConfig, DEFAULT_PORT, DEFAULT_TRUSTED_PROXY};
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;

/// 기본 데이터 폴더를 돌려줌 (`$XDG_DATA_HOME`, 없으면 `~/.local/share` 아래)
fn default_data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("screenpet-relay");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".local/share/screenpet-relay")
}

/// 사용법을 찍고 끝냄
fn usage() -> ! {
    eprintln!("usage: screenpet-relay [--bind ADDR:PORT] [--data DIR] [--trusted-proxy IP]...");
    eprintln!("       screenpet-relay admin [--data DIR] <command>   (admin --help lists them)");
    std::process::exit(2);
}

/// SIGTERM·SIGINT를 모든 스레드에서 막고 `wait`가 모음: 틀릴 시그널 핸들러가 없고 종료는 평범한 코드임
#[cfg(unix)]
mod signals {
    /// 두 시그널을 막고 그 집합을 돌려줌 (스레드를 띄우기 전에 부를 것)
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

    /// 두 시그널 중 하나가 올 때까지 기다림
    pub fn wait(set: &libc::sigset_t) {
        let mut sig = 0;
        // SAFETY: both pointers are valid for the call.
        unsafe { libc::sigwait(set, &mut sig) };
    }
}

/// `admin` 하위 명령을 돌리고 결과로 끝냄
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

/// 인자·환경 변수를 읽고 릴레이를 띄운 뒤 시그널이 오면 멈춤
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
    // 믿는 프록시도 가린 모양으로 (데이터 폴더 경로는 안 적음: 집 폴더 아래임)
    let proxies: Vec<String> = cfg.trusted_proxies.iter().map(|p| screenpet_log::mask_ip(&p.to_string())).collect();
    let proxies = if proxies.is_empty() { "loopback".to_string() } else { format!("loopback,{}", proxies.join(",")) };
    let weather = if cfg.weather_fetch.is_some() { "on" } else { "off" };
    match Relay::start(cfg) {
        Ok(relay) => {
            // 릴레이 공개키는 비밀이 아니지만 앞 16자만 (앱 설정과 견주기엔 충분)
            let relay_pub = screenpet_relay::proto::to_hex(&relay.public_key());
            log!(
                Info, "hub", "start",
                "bind" => relay.local_addr(),
                "v" => screenpet_relay::proto::PROTOCOL_VERSION,
                "relay_pub" => &relay_pub[..16],
                "proxies" => proxies,
                "weather" => weather,
            );
            // Dropping the relay tells clients, ends the threads and saves.
            #[cfg(unix)]
            {
                signals::wait(&stop_signals);
                log!(Info, "hub", "stop");
                drop(relay);
            }
            #[cfg(not(unix))]
            relay.wait();
        }
        Err(e) => {
            log!(Error, "hub", "start_fail", "err" => e);
            std::process::exit(1);
        }
    }
}
