//! ScreenPet 릴레이 서버임 (스펙 phase 3 §1b).
//!
//! 같은 네트워크에 없는 친구 사이의 믿지 않는 통로임. 설치마다 릴레이로 WebSocket 하나를 두고 Noise 정적 키로
//! 인증하므로(챌린지 + X25519 유도 MAC, [`proto`] 참고) 아무도 남의 우편함을 가로챌 수 없음. 설치는 정적 공개키로 가리킴.
//! 릴레이는 두 설치가 서로를 친구로 둘 때만, 또는 한쪽이 상대 친구 코드를 쓴 뒤 한 번(짝짓기용) *채널*로 이어 줌.
//! 채널 프레임은 불투명함: 친구들이 릴레이를 거쳐 끝에서 끝으로 Noise XX 세션을 돌리므로 릴레이는 암호문만 보고
//! 친구 트래픽을 위조할 수 없음. 짝짓기는 릴레이가 아니라 양쪽 화면의 이모지 6개 SAS로 확인함.
//!
//! 저장(`relay.json`): 설치 키마다 id 라벨, 친구 키 목록, 지금 친구 코드. 그 밖엔 없고 메시지 내용도 없음.
//!
//! 평범한 std 스레드와 동기 `tungstenite`를 씀. TLS는 앞단 리버스 프록시(Nginx Proxy Manager)에서 끝남.
//! 진입점: [`Relay::start`]. accept·upkeep·weather 스레드를 띄우고 연결마다 `http::handle`로 넘김.

#[cfg(unix)]
pub mod admin;
mod conn;
pub mod control;
pub(crate) mod board;
pub(crate) mod mailbox;
mod downloads;
mod http;
mod monitor;
// The admin commands live here but are only reachable over a Unix socket.
#[cfg_attr(not(unix), allow(dead_code))]
mod hub;
pub mod proto;
pub mod store;
pub mod weather;

use hub::Hub;
use monitor::Cid;
use screenpet_log::log;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 기본 대기 포트
pub const DEFAULT_PORT: u16 = 9411;

/// 한도 모음임 (스펙 §1b 기본값은 `Default`, 시험은 더 작은 값을 씀)
#[derive(Debug, Clone)]
pub struct Limits {
    /// 가장 큰 제어(텍스트) 메시지
    pub max_text: usize,
    /// 연결당 초당 메시지 수 (모든 종류)
    pub conn_msgs_per_sec: f64,
    /// 연결당 메시지 몰아 받기
    pub conn_msgs_burst: f64,
    /// 연결당 초당 바이트
    pub conn_bytes_per_sec: f64,
    /// 연결당 바이트 몰아 받기
    pub conn_bytes_burst: f64,
    /// 채널 방향당 초당 프레임 수 (스펙: 20/s)
    pub chan_msgs_per_sec: f64,
    /// 채널 방향당 프레임 몰아 받기
    pub chan_msgs_burst: f64,
    /// 설치당 친구 키 수 (스펙: 50)
    pub max_friends: usize,
    /// 연결당 열린 채널 수
    pub max_channels: usize,
    /// 설치 한 쌍 사이의 열린 채널 수
    pub max_channels_per_pair: usize,
    /// 이만큼 아무것도 안 받으면 끊음 (클라이언트는 20초마다 ping함)
    pub idle_timeout: Duration,
    /// 인증은 이 안에 끝나야 함
    pub auth_timeout: Duration,
    /// 친구 코드 수명 (스펙: 24시간)
    pub code_ttl: Duration,
    /// 쓴 코드는 이 시간 안에 채널 하나를 열게 해 줌
    pub grant_ttl: Duration,
    /// 주소당 분당 코드 사용 수
    pub redeems_per_ip_per_min: usize,
    /// 분당 코드 사용 수 (릴레이 전체)
    pub redeems_per_min: usize,
    /// 설치 하나의 코드가 시간당 성공적으로 쓰이는 수
    pub redeems_per_target_per_hour: usize,
    /// 시간당 새 설치 수 (릴레이 전체, 느슨한 뒷받침)
    pub registrations_per_hour: usize,
    /// 저장하는 설치 수 상한 (친구 없는 오프라인 설치부터 내보냄)
    pub max_installs: usize,
    /// 전체 연결 수 상한
    pub max_conns: usize,
    /// 주소당 연결 수 상한
    pub max_conns_per_ip: usize,
    /// 네트워크당 연결 수 (IPv4 /24, IPv6 /48)
    pub max_conns_per_network: usize,
    /// 아직 인증하지 않은 연결 수: 전체
    pub max_unauthed: usize,
    /// 아직 인증하지 않은 연결 수: 주소당
    pub max_unauthed_per_ip: usize,
    /// 아직 인증하지 않은 연결 수: 네트워크당
    pub max_unauthed_per_network: usize,
    /// 주소당 초당 넘기는 친구 프레임 바이트
    pub ip_bytes_per_sec: f64,
    /// 주소당 바이트 몰아 받기
    pub ip_bytes_burst: f64,
    /// 전체 초당 넘기는 친구 프레임 바이트
    pub global_bytes_per_sec: f64,
    /// 전체 바이트 몰아 받기
    pub global_bytes_burst: f64,
    /// 연결 하나에 기다리는 친구 프레임 바이트 상한 (넘으면 채널을 닫음)
    pub max_outbox_bytes: usize,
    /// 모든 연결에 기다리는 친구 프레임 바이트 상한
    pub max_total_outbox_bytes: usize,
    /// 큐가 `max_outbox_bytes`를 넘고 이만큼 안 움직인 연결은 `slow`로 끊음
    pub slow_after: Duration,
    /// 네트워크(IPv4 /24, IPv6 /64)당 시간당 새 설치 수
    pub registrations_per_ip_per_hour: usize,
    /// 더 넓은 네트워크(IPv4 /24, IPv6 /48)당 하루 새 설치 수
    pub registrations_per_network_per_day: usize,
    /// 네트워크(IPv4 /24, IPv6 /48)당 시간당 쓰인 서로 다른 친구 코드 수
    pub new_codes_per_network_per_hour: usize,
    /// 바뀐 데이터를 쓰는 주기 (종료 때도 씀)
    pub save_every: Duration,
    /// 📮 한 연결에서 이보다 가깝게 온 `board_list`는 무시함
    pub board_gap: Duration,
    /// 🔄 `/downloads/`에서 동시에 보내는 설치 파일 수 (넘으면 `503 busy`)
    pub max_downloads: usize,
    /// … 그리고 클라이언트 주소 하나에 (IPv6는 그 /64)
    pub max_downloads_per_ip: usize,
    /// 다운로드는 전체 이만큼 지나면 끊음, …
    pub download_deadline: Duration,
    /// … 또는 `download_grace` 뒤 평균이 초당 이 바이트보다 느리면, …
    pub download_min_rate: u64,
    /// 속도를 보기 전 유예
    pub download_grace: Duration,
    /// … 또는 쓰기 하나가 이만큼 멈추면 (읽기를 멈춘 클라이언트)
    pub download_stall: Duration,
    /// 오래된 설치를 잊는 주기. 첫 정리는 시작 뒤 이만큼 뒤에 함 (접속해 있던 클라이언트가 돌아올 시간을 줌)
    pub prune_every: Duration,
}

impl Default for Limits {
    /// 스펙 기본 한도를 돌려줌
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

/// [`Relay::start`]에 넘기는 설정임
#[derive(Clone)]
pub struct RelayConfig {
    /// 대기 주소
    pub bind: SocketAddr,
    /// `relay.json`과 `relay.key`가 사는 곳. None이면 메모리에서 무작위 릴레이 키로 돎 (시험)
    pub data_dir: Option<PathBuf>,
    /// 한도
    pub limits: Limits,
    /// `X-Real-IP` / `X-Forwarded-For` 머리를 믿는 리버스 프록시들 (루프백 말고). 다른 상대는 소켓 주소로 제한함
    pub trusted_proxies: Vec<IpAddr>,
    /// ☀️ 지역 날씨를 받아 오는 함수 (실제 릴레이는 `weather::open_meteo`). None이면 날씨를 안 받고 안 보냄 (시험)
    pub weather_fetch: Option<weather::Fetch>,
    /// ☀️ 첫 함수가 실패하면 쓰는 예비 (실제 릴레이는 `weather::met`). None이면 예비 없음
    pub weather_fallback: Option<weather::Fetch>,
}

/// 사용자의 Nginx Proxy Manager임
pub const DEFAULT_TRUSTED_PROXY: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 52));

impl RelayConfig {
    /// 기본 한도·프록시로 설정을 만듦 (날씨 없음)
    pub fn new(bind: SocketAddr, data_dir: Option<PathBuf>) -> RelayConfig {
        RelayConfig { bind, data_dir, limits: Limits::default(), trusted_proxies: vec![DEFAULT_TRUSTED_PROXY], weather_fetch: None, weather_fallback: None }
    }
}

/// 돌고 있는 릴레이임. 버리면 모든 스레드를 멈춤
pub struct Relay {
    /// 공유 허브
    hub: Arc<Hub>,
    /// 실제로 묶인 주소
    addr: SocketAddr,
    /// 띄운 주 스레드들
    threads: Vec<JoinHandle<()>>,
    /// 데이터 폴더 (종료 때 저장용)
    data_dir: Option<PathBuf>,
}

impl Relay {
    /// 릴레이를 시작함: 데이터·키를 읽고, 소켓을 열고, accept·upkeep·관리·날씨 스레드를 띄움
    pub fn start(mut cfg: RelayConfig) -> std::io::Result<Relay> {
        // 로그 줄을 stderr(journald)로 내고 `admin status`·`admin recent`용으로 셈
        monitor::install();
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
            let providers = weather::Providers::new(fetch, cfg.weather_fallback.clone());
            threads.push(std::thread::Builder::new().name("relay-weather".into()).spawn(move || weather_loop(h, providers))?);
        }
        // The relay is worth running even if its admin socket can't be made.
        #[cfg(unix)]
        if let Some(dir) = &cfg.data_dir {
            match admin::bind(dir) {
                Ok(listener) => {
                    let h = hub.clone();
                    threads.push(std::thread::Builder::new().name("relay-admin".into()).spawn(move || admin::serve(h, listener))?);
                }
                Err(e) => log!(Error, "admin", "socket_fail", "err" => e),
            }
        }
        Ok(Relay { hub, addr, threads, data_dir: cfg.data_dir })
    }

    /// 실제로 묶인 주소를 돌려줌
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// 릴레이 정적 공개키를 돌려줌 (클라이언트는 챌린지에서 앎)
    pub fn public_key(&self) -> [u8; 32] {
        self.hub.public_key
    }

    /// 접속 중인 설치 수를 돌려줌
    pub fn online_count(&self) -> usize {
        self.hub.online_count()
    }

    /// 프로세스가 죽을 때까지 기다림
    pub fn wait(mut self) {
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Relay {
    /// 멈추라고 알리고 스레드를 기다린 뒤 데이터를 저장함
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

/// 연결 말고 다른 것(리스너, 데이터 파일, `max_conns`를 넘어 곧 거절할 연결 스레드)에 남겨 두는 파일 기술자 수
const FDS_RESERVED: usize = 64;

/// 파일 기술자 한도 `soft` 안에 들어가는 연결 수를 돌려줌
fn conns_for_fds(max_conns: usize, soft: u64) -> usize {
    max_conns.min(usize::try_from(soft).unwrap_or(usize::MAX).saturating_sub(FDS_RESERVED))
}

/// 부드러운 파일 기술자 한도를 `max_conns`가 필요하고 딱딱한 한도가 허락하는 만큼 올림 (보통 기본값은 1024).
/// 그래도 모자라면 연결을 더 적게 받아, 바닥나는 일이 모두의 `accept` 실패가 아니라 `busy`로 보이게 함
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
        log!(Warn, "hub", "fd_limit", "limit" => lim.rlim_cur, "n" => conns);
    }
    conns
}

/// Unix 밖에서는 그대로 돌려줌
#[cfg(not(unix))]
fn conns_within_fd_limit(max_conns: usize) -> usize {
    max_conns
}

/// 종료까지 연결을 받아 연결마다 스레드를 띄움 (accept 오류 로그는 가끔만)
fn accept_loop(hub: Arc<Hub>, downloads: Arc<downloads::Downloads>, listener: TcpListener) {
    let mut last_accept_log = None;
    while !hub.stopping() {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                // 연결 번호는 여기서 줌: accept부터 close까지 모든 줄에 `conn=`으로 붙음
                let conn = hub.conn_id();
                let (h, d) = (hub.clone(), downloads.clone());
                if !hub.spawn("relay-conn", move || http::handle(h, d, stream, conn)) {
                    hub::bump(&hub.counters.threads_refused);
                    log!(Warn, "conn", "thread_refused", "conn" => Cid(conn));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                // Out of file descriptors, most likely: say so, but not 10 times a second.
                hub::bump(&hub.counters.accept_errors);
                if last_accept_log.is_none_or(|t: Instant| t.elapsed() >= Duration::from_secs(60)) {
                    last_accept_log = Some(Instant::now());
                    log!(Error, "conn", "accept_fail", "err" => e);
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// 시간에 맞춰 하는 일을 따로 스레드에서 돌림 (느린 디스크가 `accept`를 붙잡지 않게): 정리, 저장, 오래된 설치 잊기
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

/// 날씨 스레드가 받아 올 지역을 찾는 주기
const WEATHER_ROUND: Duration = Duration::from_secs(2);

/// 두 서비스 다 실패했고 그 서비스가 그 지역에서 이만큼 잇달아 실패했으면 WARN 대신 ERROR로 적음
const WEATHER_FAILS_ERROR: u32 = 3;

/// ☀️ 때가 된 지역 날씨를 하나씩 따로 스레드에서 받아 옴: 느린 날씨 서비스가 연결을 붙잡지 않음.
/// 지역마다 `Providers::fetch`로 Open-Meteo → MET (또는 반대) 차례로 묻고, 시도마다 `fetch_ok`/`fetch_fail`(src, took_ms)을 적음.
/// 오류가 나도 릴레이를 멈추지 않음
fn weather_loop(hub: Arc<Hub>, mut providers: weather::Providers) {
    let mut last_round = Instant::now() - WEATHER_ROUND;
    let mut fails: std::collections::HashMap<(&'static str, weather::Source), u32> = Default::default();
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
            let attempts = providers.fetch(place, Instant::now());
            let code = attempts.iter().find_map(|a| a.result.as_ref().ok().copied());
            for a in &attempts {
                match &a.result {
                    Ok(wmo) => {
                        fails.remove(&(place.id, a.src));
                        log!(Info, "weather", "fetch_ok", "src" => a.src, "place" => place.id, "wmo" => wmo, "took_ms" => a.took_ms);
                    }
                    Err(e) => {
                        let n = fails.entry((place.id, a.src)).or_default();
                        *n += 1;
                        if code.is_none() && *n >= WEATHER_FAILS_ERROR {
                            log!(Error, "weather", "fetch_fail", "src" => a.src, "place" => place.id, "n" => *n, "took_ms" => a.took_ms, "err" => e);
                        } else {
                            log!(Warn, "weather", "fetch_fail", "src" => a.src, "place" => place.id, "n" => *n, "took_ms" => a.took_ms, "err" => e);
                        }
                    }
                }
            }
            hub.weather_fetched(place.id, code);
        }
        // Left over after a stop: not busy any more (nothing to wait for).
        for place in places {
            hub.weather_fetched(place.id, None);
        }
    }
}

/// 파일 기술자 한도 계산 시험임
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
