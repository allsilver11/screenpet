//! 🔄 앱 업데이트임: `<data dir>/downloads/`의 읽기 전용 정적 파일을 내줌.
//!
//! `GET|HEAD /downloads/latest.json`과 `/downloads/<installer>`만 받고, 설치 파일 이름은 정확히
//! `ScreenPet_X.Y.Z_x64-setup.exe`나 `ScreenPet_X.Y.Z_universal.dmg`여야 함. 그 밖엔 안 내줌:
//! 목록 없음, 다른 이름 없음(경로 장난 불가), 심볼릭 링크 없음, 다른 메서드 없음.
//! 파일은 디스크에서 작은 조각으로 흘려보냄. 설치 파일은 동시에 최대 `Limits::max_downloads`개,
//! 클라이언트 주소 하나에 `max_downloads_per_ip`개까지 나가고 나머지는 `503 busy`를 받음.
//! 너무 느리거나(`download_grace` 뒤 평균 `download_min_rate` 미만) 멈추거나 `download_deadline`을 넘는 다운로드는 끊어
//! 느린 클라이언트가 자리를 못 잡게 함. 파일은 `deploy/publish-release.sh`가 놓고, `latest.json.sig`는 그 서명임
//! (릴레이는 서명 키를 갖지 않음). `http::handle`이 부름.

use crate::Limits;
use std::collections::HashMap;
use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 이 모듈이 답하는 URL 접두어
pub(crate) const PREFIX: &str = "/downloads/";
/// 업데이트 정보 파일 이름
const LATEST: &str = "latest.json";
/// 업데이트 정보 서명 파일 이름
const LATEST_SIG: &str = "latest.json.sig";
/// 한 번에 흘려보내는 조각 크기
const CHUNK: usize = 64 * 1024;

/// `path`가 다운로드 경로(`/downloads`나 그 아래)인지 봄
pub(crate) fn is_downloads(path: &str) -> bool {
    path == "/downloads" || path.starts_with(PREFIX)
}

/// `latest.json`, `latest.json.sig`, 또는 `ScreenPet_<d+>.<d+>.<d+>_(x64-setup.exe|universal.dmg)`만 받음
pub(crate) fn allowed_name(name: &str) -> bool {
    if name == LATEST || name == LATEST_SIG {
        return true;
    }
    let Some(rest) = name.strip_prefix("ScreenPet_") else { return false };
    let Some((version, kind)) = rest.split_once('_') else { return false };
    let parts: Vec<&str> = version.split('.').collect();
    let numeric = parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()));
    numeric && (kind == "x64-setup.exe" || kind == "universal.dmg")
}

/// 파일 이름에 맞는 Content-Type을 돌려줌
fn content_type(name: &str) -> &'static str {
    if name.ends_with(".json") {
        "application/json"
    } else if name.ends_with(".sig") {
        "text/plain; charset=utf-8"
    } else if name.ends_with(".dmg") {
        "application/x-apple-diskimage"
    } else {
        "application/octet-stream"
    }
}

/// 다운로드 경로 처리기임: 폴더, 한도, 진행 중인 다운로드 수
pub(crate) struct Downloads {
    /// 파일 폴더 (None이면 모두 404)
    dir: Option<PathBuf>,
    /// 동시 설치 파일 다운로드 상한
    max: usize,
    /// 주소당 동시 다운로드 상한
    max_per_ip: usize,
    /// 다운로드 하나의 최대 시간
    deadline: Duration,
    /// 유예 뒤 최소 평균 속도 (바이트/초)
    min_rate: u64,
    /// 속도를 보기 전 유예
    grace: Duration,
    /// 쓰기가 이만큼 못 움직이면 끊음
    stall: Duration,
    /// 나가는 설치 파일 수: 전체, 그리고 클라이언트 주소별
    active: Mutex<(usize, HashMap<String, usize>)>,
}

/// 다운로드 자리 하나임 (전체와 그 주소 몫). 버리면 돌려줌
struct Slot<'a> {
    /// 자리를 준 처리기
    owner: &'a Downloads,
    /// 클라이언트 주소
    ip: String,
}

impl Drop for Slot<'_> {
    /// 자리를 전체와 주소 몫에 돌려줌
    fn drop(&mut self) {
        let mut a = self.owner.active.lock().unwrap_or_else(|e| e.into_inner());
        a.0 -= 1;
        if let Some(n) = a.1.get_mut(&self.ip) {
            *n -= 1;
            if *n == 0 {
                a.1.remove(&self.ip);
            }
        }
    }
}

/// 짧은 글 본문으로 상태 응답을 씀
fn status<W: Write>(out: &mut W, status: &str, extra: &str, body: &str) {
    let _ = write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = out.flush();
}

impl Downloads {
    /// 처리기를 만듦. `dir` = `<data dir>/downloads` (None이면 모든 이름이 404)
    pub(crate) fn new(dir: Option<PathBuf>, limits: &Limits) -> Downloads {
        Downloads {
            dir,
            max: limits.max_downloads,
            max_per_ip: limits.max_downloads_per_ip,
            deadline: limits.download_deadline,
            min_rate: limits.download_min_rate,
            grace: limits.download_grace,
            stall: limits.download_stall,
            active: Mutex::new((0, HashMap::new())),
        }
    }

    /// 전체·주소 한도 안이면 자리를 하나 줌
    fn slot(&self, ip: &str) -> Option<Slot<'_>> {
        let mut a = self.active.lock().unwrap_or_else(|e| e.into_inner());
        let mine = a.1.get(ip).copied().unwrap_or(0);
        if a.0 >= self.max || mine >= self.max_per_ip {
            return None;
        }
        a.0 += 1;
        *a.1.entry(ip.to_string()).or_insert(0) += 1;
        Some(Slot { owner: self, ip: ip.to_string() })
    }

    /// 다운로드용 소켓 설정임: `download_stall` 동안 못 움직이는 쓰기는 끝냄
    pub(crate) fn prepare(&self, stream: &std::net::TcpStream) {
        let _ = stream.set_write_timeout(Some(self.stall));
    }

    /// `elapsed` 동안 `sent`바이트를 보낸 전송을 멈춰야 하는지 봄
    fn too_slow(&self, sent: u64, elapsed: Duration) -> bool {
        if elapsed >= self.deadline {
            return true;
        }
        elapsed >= self.grace && (sent as f64) < self.min_rate as f64 * elapsed.as_secs_f64()
    }

    /// 클라이언트 주소 `ip`(릴레이의 주소별 묶음)에서 온 다운로드 경로 요청 하나에 답함.
    /// `stopping`이 참이 되면(릴레이 종료) 전송을 일찍 끝냄
    pub(crate) fn serve<W: Write>(&self, method: &str, path: &str, ip: &str, out: &mut W, stopping: &dyn Fn() -> bool) {
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
        let installer = name != LATEST && name != LATEST_SIG;
        let _slot = if installer {
            match self.slot(ip) {
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
        let started = Instant::now();
        while left > 0 && !stopping() && !self.too_slow(len - left, started.elapsed()) {
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

/// 이름·경로·자리 한도·느린 전송·심볼릭 링크 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_release_installers_and_latest_json_are_allowed() {
        for ok in ["latest.json", "latest.json.sig", "ScreenPet_0.6.3_x64-setup.exe", "ScreenPet_0.6.3_universal.dmg", "ScreenPet_10.20.300_universal.dmg"] {
            assert!(allowed_name(ok), "{ok}");
        }
        for bad in [
            "",
            "latest.json.tmp",
            "latest.json.sig.tmp",
            "latest.sig",
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
        assert_eq!(content_type("latest.json.sig"), "text/plain; charset=utf-8");
        assert_eq!(content_type("ScreenPet_0.6.3_universal.dmg"), "application/x-apple-diskimage");
        assert_eq!(content_type("ScreenPet_0.6.3_x64-setup.exe"), "application/octet-stream");
    }

    fn limits(max: usize, per_ip: usize) -> Limits {
        Limits { max_downloads: max, max_downloads_per_ip: per_ip, ..Limits::default() }
    }

    fn at(d: &Downloads) -> (usize, usize) {
        let a = d.active.lock().unwrap();
        (a.0, a.1.len())
    }

    #[test]
    fn the_cap_hands_out_slots_and_takes_them_back() {
        let d = Downloads::new(None, &limits(2, 8));
        let a = d.slot("1.1.1.1").unwrap();
        let b = d.slot("2.2.2.2").unwrap();
        assert!(d.slot("3.3.3.3").is_none());
        drop(a);
        let c = d.slot("3.3.3.3").unwrap();
        assert!(d.slot("1.1.1.1").is_none());
        drop((b, c));
        assert_eq!(at(&d), (0, 0));
    }

    #[test]
    fn one_address_gets_at_most_its_share() {
        let d = Downloads::new(None, &limits(8, 2));
        let a = d.slot("1.1.1.1").unwrap();
        let _b = d.slot("1.1.1.1").unwrap();
        assert!(d.slot("1.1.1.1").is_none());
        let _c = d.slot("2.2.2.2").unwrap();
        drop(a);
        assert!(d.slot("1.1.1.1").is_some());
        assert_eq!(Limits::default().max_downloads_per_ip, 2);
    }

    #[test]
    fn slow_or_long_downloads_are_cut_off() {
        let d = Downloads::new(None, &Limits::default());
        let s = Duration::from_secs;
        // Grace first: nothing sent yet is fine for 20 s.
        assert!(!d.too_slow(0, s(19)));
        assert!(d.too_slow(0, s(20)));
        assert!(!d.too_slow(64 * 1024 * 30, s(30)));
        assert!(d.too_slow(64 * 1024 * 30 - 1, s(30)));
        // Fast enough, but not forever.
        assert!(!d.too_slow(u64::MAX / 2, s(30 * 60 - 1)));
        assert!(d.too_slow(u64::MAX / 2, s(30 * 60)));
        // A 100 MB installer at the minimum rate fits in the deadline.
        assert!(100 * 1024 * 1024 / Limits::default().download_min_rate < Limits::default().download_deadline.as_secs());
    }

    fn answer(d: &Downloads, method: &str, path: &str) -> String {
        let mut out = Vec::new();
        d.serve(method, path, "127.0.0.1", &mut out, &|| false);
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn methods_names_and_missing_files() {
        let dir = std::env::temp_dir().join(format!("screenpet-dl-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("latest.json"), "{}").unwrap();
        std::fs::write(dir.join("relay.key"), "secret").unwrap();
        let d = Downloads::new(Some(dir.clone()), &Limits::default());
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
        assert!(answer(&Downloads::new(None, &Limits::default()), "GET", "/downloads/latest.json").starts_with("HTTP/1.1 404"));
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
        let d = Downloads::new(Some(dir.clone()), &Limits::default());
        assert!(answer(&d, "GET", "/downloads/ScreenPet_0.6.3_universal.dmg").starts_with("HTTP/1.1 404"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
