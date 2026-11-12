//! 🔄 App updates: read-only static files from `<data dir>/downloads/`.
//!
//! `GET|HEAD /downloads/latest.json` and `/downloads/<installer>`, where the
//! installer name must be exactly `ScreenPet_X.Y.Z_x64-setup.exe` or
//! `ScreenPet_X.Y.Z_universal.dmg`. Nothing else is served: no listing, no
//! other names (so no path tricks), no symlinks, no other methods. Files are
//! streamed from disk in small chunks; at most `Limits::max_downloads`
//! installers go out at once, the rest get `503 busy`. The files are put
//! there by `deploy/publish-release.sh`.

use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// The URL prefix this module answers.
pub(crate) const PREFIX: &str = "/downloads/";
const LATEST: &str = "latest.json";
const CHUNK: usize = 64 * 1024;
/// A download may stall this long per write before it is dropped (slow
/// links are fine as long as they move).
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Does `path` belong to the download routes (`/downloads` or below)?
pub(crate) fn is_downloads(path: &str) -> bool {
    path == "/downloads" || path.starts_with(PREFIX)
}

/// `latest.json`, or `ScreenPet_<d+>.<d+>.<d+>_(x64-setup.exe|universal.dmg)`.
pub(crate) fn allowed_name(name: &str) -> bool {
    if name == LATEST {
        return true;
    }
    let Some(rest) = name.strip_prefix("ScreenPet_") else { return false };
    let Some((version, kind)) = rest.split_once('_') else { return false };
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()));
    numeric && (kind == "x64-setup.exe" || kind == "universal.dmg")
}

fn content_type(name: &str) -> &'static str {
    if name.ends_with(".json") {
        "application/json"
    } else if name.ends_with(".dmg") {
        "application/x-apple-diskimage"
    } else {
        "application/octet-stream"
    }
}

pub(crate) struct Downloads {
    dir: Option<PathBuf>,
    max: usize,
    active: AtomicUsize,
}

/// One of the `max` download slots, given back on drop.
struct Slot<'a>(&'a AtomicUsize);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn status<W: Write>(out: &mut W, status: &str, extra: &str, body: &str) {
    let _ = write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = out.flush();
}

impl Downloads {
    /// `dir` = `<data dir>/downloads` (`None`: every name is 404).
    pub(crate) fn new(dir: Option<PathBuf>, max: usize) -> Downloads {
        Downloads { dir, max, active: AtomicUsize::new(0) }
    }

    fn slot(&self) -> Option<Slot<'_>> {
        let mut n = self.active.load(Ordering::SeqCst);
        loop {
            if n >= self.max {
                return None;
            }
            match self.active.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst) {
                Ok(_) => return Some(Slot(&self.active)),
                Err(now) => n = now,
            }
        }
    }

    /// Answer one request on a download route. `stopping` ends a transfer
    /// early when the relay shuts down.
    pub(crate) fn serve<W: Write>(&self, method: &str, path: &str, out: &mut W, stopping: &dyn Fn() -> bool) {
        let head = match method {
            "GET" => false,
            "HEAD" => true,
            _ => return status(out, "405 Method Not Allowed", "Allow: GET, HEAD\r\n", "method not allowed\n"),
        };
        let name = path.strip_prefix(PREFIX).unwrap_or("");
        let Some(dir) = self.dir.as_ref().filter(|_| allowed_name(name)) else {
            return status(out, "404 Not Found", "", "not found\n");
        };
        let file_path = dir.join(name);
        // A regular file only: never a symlink out of the folder.
        let is_file = std::fs::symlink_metadata(&file_path).is_ok_and(|m| m.file_type().is_file());
        if !is_file {
            return status(out, "404 Not Found", "", "not found\n");
        }
        let installer = name != LATEST;
        let _slot = if installer {
            match self.slot() {
                Some(s) => Some(s),
                None => return status(out, "503 Service Unavailable", "Retry-After: 60\r\n", "busy\n"),
            }
        } else {
            None
        };
        // Open before sizing: a newer release being published renames over
        // names, and the open file stays the one we measured.
        let Ok(mut file) = File::open(&file_path) else {
            return status(out, "404 Not Found", "", "not found\n");
        };
        let Ok(len) = file.metadata().map(|m| m.len()) else {
            return status(out, "500 Internal Server Error", "", "error\n");
        };
        let extra = if installer {
            format!("Cache-Control: public, max-age=3600\r\nContent-Disposition: attachment; filename=\"{name}\"\r\n")
        } else {
            "Cache-Control: no-cache\r\n".to_string()
        };
        let head_ok = write!(
            out,
            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {len}\r\n{extra}X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
            content_type(name)
        );
        if head_ok.is_err() || head {
            let _ = out.flush();
            return;
        }
        let mut buf = vec![0u8; CHUNK];
        let mut left = len;
        while left > 0 && !stopping() {
            let want = (left.min(CHUNK as u64)) as usize;
            let n = match file.read(&mut buf[..want]) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            };
            if out.write_all(&buf[..n]).is_err() {
                return;
            }
            left -= n as u64;
        }
        let _ = out.flush();
    }
}

/// Socket settings for a download (a big file to a slow link).
pub(crate) fn prepare(stream: &std::net::TcpStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_release_installers_and_latest_json_are_allowed() {
        for ok in ["latest.json", "ScreenPet_0.6.3_x64-setup.exe", "ScreenPet_0.6.3_universal.dmg", "ScreenPet_10.20.300_universal.dmg"] {
            assert!(allowed_name(ok), "{ok}");
        }
        for bad in [
            "",
            "latest.json.tmp",
            "relay.key",
            "relay.json",
            "../relay.key",
            "..",
            ".",
            "ScreenPet_0.6.3_x64-setup.exe.part",
            "ScreenPet_0.6_x64-setup.exe",
            "ScreenPet_0.6.3.1_universal.dmg",
            "ScreenPet_v0.6.3_universal.dmg",
            "ScreenPet_0.6.x_universal.dmg",
            "ScreenPet_0.6.3_x64_en-US.msi",
            "ScreenPet_0.6.3_universal.dmg/",
            "ScreenPet_0.6.3_universal.dmg/../../relay.key",
            "sub/ScreenPet_0.6.3_universal.dmg",
            "ScreenPet_0..3_universal.dmg",
            "ScreenPet_+0.6.3_universal.dmg",
            "screenpet_0.6.3_universal.dmg",
            "ScreenPet_0.6.3_universal.dmg%00",
            "%2e%2e/relay.key",
            "ScreenPet_0.6.3_universal.dmg ",
        ] {
            assert!(!allowed_name(bad), "{bad}");
        }
    }

    #[test]
    fn routes_are_the_downloads_folder_only() {
        assert!(is_downloads("/downloads/latest.json") && is_downloads("/downloads/") && is_downloads("/downloads"));
        assert!(!is_downloads("/downloadsx") && !is_downloads("/healthz") && !is_downloads("/ws"));
    }

    #[test]
    fn content_types() {
        assert_eq!(content_type("latest.json"), "application/json");
        assert_eq!(content_type("ScreenPet_0.6.3_universal.dmg"), "application/x-apple-diskimage");
        assert_eq!(content_type("ScreenPet_0.6.3_x64-setup.exe"), "application/octet-stream");
    }

    #[test]
    fn the_cap_hands_out_slots_and_takes_them_back() {
        let d = Downloads::new(None, 2);
        let a = d.slot().unwrap();
        let b = d.slot().unwrap();
        assert!(d.slot().is_none());
        drop(a);
        let c = d.slot().unwrap();
        assert!(d.slot().is_none());
        drop((b, c));
        assert_eq!(d.active.load(Ordering::SeqCst), 0);
    }

    fn answer(d: &Downloads, method: &str, path: &str) -> String {
        let mut out = Vec::new();
        d.serve(method, path, &mut out, &|| false);
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn methods_names_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("screenpet-dl-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("latest.json"), "{}").unwrap();
        std::fs::write(dir.join("relay.key"), "secret").unwrap();
        let d = Downloads::new(Some(dir.clone()), 8);
        let r = answer(&d, "GET", "/downloads/latest.json");
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n") && r.contains("Content-Length: 2\r\n") && r.contains("Cache-Control: no-cache\r\n") && r.ends_with("\r\n\r\n{}"), "{r}");
        assert!(r.contains("Content-Type: application/json\r\n"));
        let r = answer(&d, "HEAD", "/downloads/latest.json");
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n") && r.contains("Content-Length: 2\r\n") && r.ends_with("\r\n\r\n"), "{r}");
        assert!(answer(&d, "POST", "/downloads/latest.json").starts_with("HTTP/1.1 405"));
        assert!(answer(&d, "PUT", "/downloads/latest.json").starts_with("HTTP/1.1 405"));
        for p in ["/downloads/relay.key", "/downloads/../relay.key", "/downloads/", "/downloads", "/downloads/ScreenPet_0.6.3_universal.dmg"] {
            let r = answer(&d, "GET", p);
            assert!(r.starts_with("HTTP/1.1 404") && !r.contains("secret"), "{p}: {r}");
        }
        // No folder (in-memory relay): nothing at all.
        assert!(answer(&Downloads::new(None, 8), "GET", "/downloads/latest.json").starts_with("HTTP/1.1 404"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_not_followed() {
        let dir = std::env::temp_dir().join(format!("screenpet-dl-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("secret"), "secret").unwrap();
        std::os::unix::fs::symlink(dir.join("secret"), dir.join("ScreenPet_0.6.3_universal.dmg")).unwrap();
        let d = Downloads::new(Some(dir.clone()), 8);
        assert!(answer(&d, "GET", "/downloads/ScreenPet_0.6.3_universal.dmg").starts_with("HTTP/1.1 404"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
