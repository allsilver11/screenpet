//! 🔄 `/downloads/…` on a real relay over TCP: the installers and latest.json
//! the app updates from.

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
        let root = std::env::temp_dir().join(format!("screenpet-dl-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("downloads")).unwrap();
        let mut cfg = RelayConfig::new(SocketAddr::from(([127, 0, 0, 1], 0)), Some(root.clone()));
        cfg.limits = Limits { max_downloads, ..Limits::default() };
        Fixture { relay: Some(Relay::start(cfg).unwrap()), root }
    }

    fn file(&self, name: &str) -> PathBuf {
        self.root.join("downloads").join(name)
    }

    fn addr(&self) -> SocketAddr {
        self.relay.as_ref().unwrap().local_addr()
    }

    fn send(&self, method: &str, path: &str) -> TcpStream {
        let mut s = TcpStream::connect(self.addr()).unwrap();
        write!(s, "{method} {path} HTTP/1.1\r\nHost: pet.danni.day\r\n\r\n").unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s
    }

    /// Whole response: (head, body).
    fn get(&self, method: &str, path: &str) -> (String, Vec<u8>) {
        let mut s = self.send(method, path);
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
