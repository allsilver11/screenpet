//! Operator switches and bans (`control.json` in the data directory), set with
//! `screenpet-relay admin`. Kept apart from `relay.json`: losing the install
//! data is harmless, so that file may fall back to empty, but a torn control
//! file must never silently lift a ban, so this one stops the relay starting.

use crate::proto;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

const CONTROL_FILE: &str = "control.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Control {
    /// New installs are accepted.
    pub registration: bool,
    /// Friend codes are issued and redeemed.
    pub friendcode: bool,
    /// Anyone may connect (`/healthz` answers regardless).
    pub relay: bool,
    /// Install keys that may not log in.
    pub banned_keys: BTreeSet<String>,
    /// Address keys (IPv4 address, IPv6 /64) that may not connect.
    pub banned_ips: BTreeSet<String>,
}

impl Default for Control {
    fn default() -> Control {
        Control { registration: true, friendcode: true, relay: true, banned_keys: BTreeSet::new(), banned_ips: BTreeSet::new() }
    }
}

fn valid_ip_key(key: &str) -> bool {
    key.parse::<std::net::Ipv4Addr>().is_ok() || key.strip_suffix("::/64").is_some_and(|p| format!("{p}::").parse::<std::net::Ipv6Addr>().is_ok())
}

impl Control {
    /// The saved state, or the defaults if there is no file. Anything
    /// unreadable is an error.
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

    /// Write it durably: private temp file, rename, fsync the directory.
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

#[cfg(test)]
mod tests {
    use super::*;

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
