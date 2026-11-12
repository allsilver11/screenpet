//! 운영자 스위치와 차단임 (데이터 폴더의 `control.json`, `screenpet-relay admin`으로 바꿈).
//! `relay.json`과 따로 둠: 설치 데이터를 잃는 건 해롭지 않아 그 파일은 빈 것으로 돌아가도 되지만,
//! 찢어진 control 파일이 조용히 차단을 풀면 안 되므로 이 파일이 깨지면 릴레이가 시작하지 않음.

use crate::proto;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

/// 데이터 폴더 안 파일 이름
const CONTROL_FILE: &str = "control.json";

/// 운영자 스위치와 차단 목록임
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Control {
    /// 새 설치를 받음
    pub registration: bool,
    /// 친구 코드를 주고 받아 줌
    pub friendcode: bool,
    /// 누구나 연결할 수 있음 (`/healthz`는 상관없이 답함)
    pub relay: bool,
    /// 로그인할 수 없는 설치 키
    pub banned_keys: BTreeSet<String>,
    /// 연결할 수 없는 주소 키 (IPv4 주소, IPv6 /64)
    pub banned_ips: BTreeSet<String>,
    /// 건의 편지를 나열·관리할 수 있는 설치 키 (개발자의 컴퓨터들)
    pub mailbox_readers: BTreeSet<String>,
}

impl Default for Control {
    /// 모두 켜고 차단·독자는 비운 기본값임
    fn default() -> Control {
        Control { registration: true, friendcode: true, relay: true, banned_keys: BTreeSet::new(), banned_ips: BTreeSet::new(), mailbox_readers: BTreeSet::new() }
    }
}

/// 주소 키 모양(IPv4 주소나 `…::/64`)인지 봄
fn valid_ip_key(key: &str) -> bool {
    key.parse::<std::net::Ipv4Addr>().is_ok() || key.strip_suffix("::/64").is_some_and(|p| format!("{p}::").parse::<std::net::Ipv6Addr>().is_ok())
}

impl Control {
    /// 저장된 상태를 읽음. 파일이 없으면 기본값이고, 못 읽는 것은 모두 오류임
    pub fn load(dir: &Path) -> io::Result<Control> {
        let path = dir.join(CONTROL_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Control::default()),
            Err(e) => return Err(e),
        };
        let invalid = |what: String| io::Error::new(io::ErrorKind::InvalidData, format!("{} is corrupt ({what}); fix or remove it", path.display()));
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
        if !value.is_object() {
            return Err(invalid("not an object".into()));
        }
        let control: Control = serde_json::from_value(value).map_err(|e| invalid(e.to_string()))?;
        if control.banned_keys.iter().any(|k| proto::norm_key(k).as_deref() != Some(k.as_str())) {
            return Err(invalid("a banned key is malformed".into()));
        }
        if !control.banned_ips.iter().all(|k| valid_ip_key(k)) {
            return Err(invalid("a banned address is malformed".into()));
        }
        Ok(control)
    }

    /// 튼튼하게 씀: 비공개 임시 파일, 이름 바꾸기, 폴더 fsync
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let tmp = dir.join(format!("{CONTROL_FILE}.tmp"));
        write_private(&tmp, &serde_json::to_vec_pretty(self).map_err(io::Error::other)?)?;
        std::fs::rename(&tmp, dir.join(CONTROL_FILE))?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

/// 읽기·쓰기와 파일 권한 시험임
#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용 빈 임시 폴더를 만듦
    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("screenpet-control-{tag}-{}", proto::to_hex(&proto::random32()[..8])));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_is_defaults_and_saved_comes_back() {
        let dir = tmp("rt");
        assert_eq!(Control::load(&dir).unwrap(), Control::default());
        let mut c = Control { friendcode: false, ..Control::default() };
        c.banned_keys.insert(proto::to_hex(&[1; 32]));
        c.banned_ips.insert("203.0.113.1".into());
        c.banned_ips.insert("2001:db8:1:2::/64".into());
        c.save(&dir).unwrap();
        assert_eq!(Control::load(&dir).unwrap(), c);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn anything_unreadable_is_an_error() {
        let dir = tmp("bad");
        for bad in ["{\"relay\": tr", "[]", r#"{"banned_keys": ["nope"]}"#, r#"{"banned_ips": ["nope"]}"#] {
            std::fs::write(dir.join(CONTROL_FILE), bad).unwrap();
            assert!(Control::load(&dir).is_err(), "{bad}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn it_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("perm");
        Control::default().save(&dir).unwrap();
        assert_eq!(std::fs::metadata(dir.join(CONTROL_FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
