//! 🔄 TCP 위 실제 릴레이의 `/downloads/…` 시험임: 앱이 업데이트를 받는 설치 파일과 latest.json.

use screenpet_relay::{Limits, Relay, RelayConfig};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Fixture {
    relay: Option<Relay>,
    root: PathBuf,
}

impl Fixture {
    fn new(tag: &str, max_downloads: usize) -> Fixture {
        // Held-open test downloads end 2 s after they stop moving (not 10).
        Fixture::with(tag, Limits { max_downloads, max_downloads_per_ip: max_downloads, download_stall: Duration::from_secs(2), ..Limits::default() })
    }

    fn with(tag: &str, limits: Limits) -> Fixture {
        let root = std::env::temp_dir().join(format!("screenpet-dl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("downloads")).unwrap();
        let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), Some(root.clone()));
        cfg.limits = limits;
        Fixture { relay: Some(Relay::start(cfg).unwrap()), root }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.root.join("downloads").join(name)
    }

    fn addr(&self) -> SocketAddr {
        self.relay.as_ref().unwrap().local_addr()
    }

    fn send(&self, method: &str, path: &str) -> TcpStream {
        self.send_from(method, path, None)
    }

    /// `send`처럼 하되 (믿는 루프백 프록시 머리로) `ip`에서 온 것처럼 보냄
    fn send_from(&self, method: &str, path: &str, ip: Option<&str>) -> TcpStream {
        let mut s = TcpStream::connect(self.addr()).unwrap();
        let fwd = ip.map(|ip| format!("X-Real-IP: {ip}\r\n")).unwrap_or_default();
        write!(s, "{method} {path} HTTP/1.1\r\nHost: pet.danni.day\r\n{fwd}\r\n").unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s
    }

    /// 응답 전체를 읽음: (머리, 본문)
    fn get(&self, method: &str, path: &str) -> (String, Vec<u8>) {
        self.get_from(method, path, None)
    }

    fn get_from(&self, method: &str, path: &str, ip: Option<&str>) -> (String, Vec<u8>) {
        let mut s = self.send_from(method, path, ip);
        let mut all = Vec::new();
        s.read_to_end(&mut all).unwrap();
        let end = all.windows(4).position(|w| w == b"\r\n\r\n").expect("head") + 4;
        (String::from_utf8_lossy(&all[..end]).into_owned(), all[end..].to_vec())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.relay.take());
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn status(head: &str) -> u16 {
    head.split(' ').nth(1).unwrap().parse().unwrap()
}

fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.split("\r\n").find_map(|l| l.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim()))
}

#[test]
fn latest_json_is_served_uncached() {
    let f = Fixture::new("latest", 8);
    let json = r#"{"version":"0.6.3","files":{}}"#;
    std::fs::write(f.file("latest.json"), json).unwrap();
    let (head, body) = f.get("GET", "/downloads/latest.json");
    assert_eq!(status(&head), 200, "{head}");
    assert_eq!(body, json.as_bytes());
    assert_eq!(header(&head, "content-type"), Some("application/json"));
    assert_eq!(header(&head, "content-length"), Some(json.len().to_string().as_str()));
    assert_eq!(header(&head, "cache-control"), Some("no-cache"));
    // A query string (cache buster) is ignored.
    assert_eq!(status(&f.get("GET", "/downloads/latest.json?t=1").0), 200);
    // Its signature, made by deploy/publish-release.sh.
    let sig = "ab".repeat(64);
    std::fs::write(f.file("latest.json.sig"), &sig).unwrap();
    let (head, body) = f.get("GET", "/downloads/latest.json.sig");
    assert_eq!((status(&head), body), (200, sig.into_bytes()));
    assert_eq!(header(&head, "cache-control"), Some("no-cache"));
}

#[test]
fn a_large_installer_streams_whole_and_head_sends_no_body() {
    let f = Fixture::new("large", 8);
    let name = "ScreenPet_0.6.3_universal.dmg";
    let data: Vec<u8> = (0..40 * 1024 * 1024 + 123).map(|i: u32| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    std::fs::write(f.file(name), &data).unwrap();
    let (head, body) = f.get("GET", &format!("/downloads/{name}"));
    assert_eq!(status(&head), 200);
    assert_eq!(header(&head, "content-length"), Some(data.len().to_string().as_str()));
    assert_eq!(header(&head, "content-type"), Some("application/x-apple-diskimage"));
    assert!(body == data, "body differs ({} bytes)", body.len());
    let (head, body) = f.get("HEAD", &format!("/downloads/{name}"));
    assert_eq!(status(&head), 200);
    assert_eq!(header(&head, "content-length"), Some(data.len().to_string().as_str()));
    assert!(body.is_empty());
}

#[test]
fn only_allowed_names_and_methods() {
    let f = Fixture::new("names", 8);
    std::fs::write(f.file("ScreenPet_0.6.3_x64-setup.exe"), b"MZ").unwrap();
    std::fs::write(f.file("notes.txt"), b"private").unwrap();
    std::fs::write(f.file("ScreenPet_0.6.3_x64-setup.exe.part"), b"partial").unwrap();
    let (head, body) = f.get("GET", "/downloads/ScreenPet_0.6.3_x64-setup.exe");
    assert_eq!((status(&head), body.as_slice()), (200, &b"MZ"[..]));
    assert_eq!(header(&head, "content-type"), Some("application/octet-stream"));
    for path in [
        "/downloads/",
        "/downloads",
        "/downloads/notes.txt",
        "/downloads/ScreenPet_0.6.3_x64-setup.exe.part",
        "/downloads/../relay.key",
        "/downloads/..%2frelay.key",
        "/downloads/%2e%2e/relay.key",
        "/downloads/./ScreenPet_0.6.3_x64-setup.exe",
        "/downloads//ScreenPet_0.6.3_x64-setup.exe",
        "/downloads/ScreenPet_0.6.4_x64-setup.exe",
        "/downloads/latest.json",
        "/relay.key",
    ] {
        let (head, body) = f.get("GET", path);
        assert_eq!(status(&head), 404, "{path}");
        assert!(!String::from_utf8_lossy(&body).contains("private"), "{path}");
    }
    assert_eq!(status(&f.get("POST", "/downloads/ScreenPet_0.6.3_x64-setup.exe").0), 405);
    assert_eq!(status(&f.get("DELETE", "/downloads/ScreenPet_0.6.3_x64-setup.exe").0), 405);
    // The rest of the relay is as before.
    assert_eq!(status(&f.get("GET", "/healthz").0), 200);
    assert_eq!(status(&f.get("HEAD", "/healthz").0), 405);
}

#[test]
fn too_many_downloads_at_once_get_busy() {
    let f = Fixture::new("busy", 2);
    let name = "ScreenPet_0.6.3_x64-setup.exe";
    // Big enough that the relay blocks on full socket buffers.
    std::fs::File::create(f.file(name)).unwrap().set_len(64 * 1024 * 1024).unwrap();
    std::fs::write(f.file("latest.json"), "{}").unwrap();
    let path = format!("/downloads/{name}");
    let mut held = Vec::new();
    for _ in 0..2 {
        let mut s = f.send("GET", &path);
        let mut first = [0u8; 12];
        s.read_exact(&mut first).unwrap();
        assert_eq!(&first, b"HTTP/1.1 200");
        held.push(s);
    }
    let (head, _) = f.get("GET", &path);
    assert_eq!(status(&head), 503, "{head}");
    // latest.json is tiny and never waits for a slot.
    assert_eq!(status(&f.get("GET", "/downloads/latest.json").0), 200);
    // Hanging up gives the slot back.
    drop(held.remove(0));
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let (head, _) = f.get("HEAD", &path);
        if status(&head) == 200 {
            break;
        }
        assert!(Instant::now() < deadline, "slot never came back");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn the_names_the_app_asks_for_are_allowed() {
    let f = Fixture::new("appnames", 8);
    let v = screenpet_core::update::Version::parse("0.6.3").unwrap();
    for p in [screenpet_core::update::Platform::Windows, screenpet_core::update::Platform::MacOs] {
        let name = p.asset_name(v);
        std::fs::write(f.file(&name), b"x").unwrap();
        assert_eq!(status(&f.get("GET", &format!("/downloads/{name}")).0), 200, "{name}");
        // And the URL the app builds points here.
        assert_eq!(screenpet_core::update::download_url(&name), format!("https://pet.danni.day/downloads/{name}"));
    }
}

/// 읽지 않고 다운로드를 열어 둠 (200 머리 뒤)
fn hold(f: &Fixture, path: &str, ip: &str) -> TcpStream {
    let mut s = f.send_from("GET", path, Some(ip));
    let mut first = [0u8; 12];
    s.read_exact(&mut first).unwrap();
    assert_eq!(&first, b"HTTP/1.1 200");
    s
}

#[test]
fn one_client_address_gets_at_most_two_downloads() {
    let f = Fixture::with("perip", Limits { download_stall: Duration::from_secs(2), ..Limits::default() });
    assert_eq!(Limits::default().max_downloads_per_ip, 2);
    let name = "ScreenPet_0.6.3_x64-setup.exe";
    std::fs::File::create(f.file(name)).unwrap().set_len(64 * 1024 * 1024).unwrap();
    let path = format!("/downloads/{name}");
    let _a = hold(&f, &path, "203.0.113.7");
    let _b = hold(&f, &path, "203.0.113.7");
    assert_eq!(status(&f.get_from("GET", &path, Some("203.0.113.7")).0), 503);
    // Another address still gets one.
    let _c = hold(&f, &path, "198.51.100.1");
}

#[test]
fn a_client_too_slow_to_read_is_cut_off_and_frees_its_slot() {
    let limits = Limits {
        max_downloads: 1,
        download_min_rate: 50 * 1024 * 1024,
        download_grace: Duration::from_millis(300),
        download_stall: Duration::from_millis(500),
        ..Limits::default()
    };
    let f = Fixture::with("slow", limits);
    let name = "ScreenPet_0.6.3_universal.dmg";
    let size = 64 * 1024 * 1024u64;
    std::fs::File::create(f.file(name)).unwrap().set_len(size).unwrap();
    let path = format!("/downloads/{name}");
    let started = Instant::now();
    let mut s = f.send("GET", &path);
    // Trickle: 64 KiB every 10 ms (~6 MB/s), far under the minimum.
    let mut got = 0u64;
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n as u64,
        }
        std::thread::sleep(Duration::from_millis(10));
        assert!(started.elapsed() < Duration::from_secs(20), "never cut off");
    }
    assert!(got < size, "the whole file went out");
    // The slot is free again (busy only while the first transfer ran).
    let deadline = Instant::now() + Duration::from_secs(10);
    while status(&f.get("HEAD", &path).0) != 200 {
        assert!(Instant::now() < deadline, "slot never came back");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn downloads_have_an_overall_deadline() {
    let limits = Limits { download_deadline: Duration::from_millis(300), download_min_rate: 0, ..Limits::default() };
    let f = Fixture::with("deadline", limits);
    let name = "ScreenPet_0.6.3_universal.dmg";
    let size = 64 * 1024 * 1024u64;
    std::fs::File::create(f.file(name)).unwrap().set_len(size).unwrap();
    let mut s = f.send("GET", &format!("/downloads/{name}"));
    let mut got = 0u64;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n as u64,
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(got < size, "the whole file went out");
}
