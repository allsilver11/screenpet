//! Persistent relay data (`relay.json` in the data directory): per install
//! (keyed by its static public key) the self-declared install id, the friend
//! keys it wants to reach, and its current friend code. No message content is
//! ever stored. Also the relay's own static key (`relay.key`).

use crate::proto::{self, new_code};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const DATA_FILE: &str = "relay.json";
const KEY_FILE: &str = "relay.key";
/// Installs not seen for this long are forgotten.
const FORGET_AFTER_SECS: u64 = 180 * 24 * 3600;
/// Installs that never listed a friend nor took part in a code redeem are
/// forgotten after this long unseen (so registrations can't fill the store
/// for good).
pub const IDLE_FORGET_SECS: u64 = 24 * 3600;
/// Befriended installs seen on fewer than `ESTABLISHED_DAYS` days are
/// forgotten after this long unseen: keys that only befriend each other can't
/// buy half a year. Being forgotten costs a real install little, its next
/// login registers it again with the same friend list.
const NEW_FRIEND_FORGET_SECS: u64 = 14 * 24 * 3600;
const ESTABLISHED_DAYS: u32 = 7;
const DAY_SECS: u64 = 24 * 3600;
/// `last_seen` alone makes the store dirty at most this often per install.
const TOUCH_EVERY_SECS: u64 = 600;
/// Default hard cap on stored installs.
pub const MAX_INSTALLS: usize = 20_000;

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn established_days() -> u32 {
    ESTABLISHED_DAYS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Install {
    pub id: String,
    #[serde(default)]
    pub friends: Vec<String>,
    pub code: String,
    /// When `code` was issued (unix seconds).
    pub code_at: u64,
    #[serde(default)]
    pub last_seen: u64,
    /// Redeemed a code or had its code redeemed (kept like an install with a
    /// mutual friend).
    #[serde(default)]
    pub active: bool,
    /// Distinct days this install has been seen on. Files from before this
    /// count was kept hold real installs: they start out established.
    #[serde(default = "established_days")]
    pub seen_days: u32,
}



#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Data {
    #[serde(default)]
    installs: HashMap<String, Install>,
}

/// A copy of the data to write outside the hub lock (see [`write_snapshot`]).
pub struct Snapshot {
    dir: PathBuf,
    data: Data,
}

pub struct Store {
    dir: Option<PathBuf>,
    data: Data,
    /// code → install key
    by_code: HashMap<String, String>,
    dirty: bool,
    /// Hard cap on stored installs (new keys are refused when full).
    pub max_installs: usize,
}

/// Parse a data file; `None` if missing or torn.
fn read_data(path: &Path) -> Option<Data> {
    let bytes = std::fs::read(path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(d) => Some(d),
        Err(e) => {
            eprintln!("screenpet-relay: {} is unreadable ({e})", path.display());
            None
        }
    }
}

/// Create `dir` (0700 on unix).
fn private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl Store {
    /// In-memory only (tests).
    pub fn memory() -> Store {
        Store { dir: None, data: Data::default(), by_code: HashMap::new(), dirty: false, max_installs: MAX_INSTALLS }
    }

    /// Load `relay.json`, else its backup `relay.json.bak`, else start empty
    /// (logged) — a torn file never stops the relay from starting.
    pub fn open(dir: &Path) -> io::Result<Store> {
        private_dir(dir)?;
        let main = dir.join(DATA_FILE);
        let bak = dir.join(format!("{DATA_FILE}.bak"));
        let data = match read_data(&main) {
            Some(d) => d,
            None => match read_data(&bak) {
                Some(d) => {
                    eprintln!("screenpet-relay: using {}", bak.display());
                    d
                }
                None => {
                    if main.exists() || bak.exists() {
                        eprintln!("screenpet-relay: no readable data file; starting with an empty store");
                    }
                    Data::default()
                }
            },
        };
        let mut store = Store { dir: Some(dir.to_path_buf()), data, by_code: HashMap::new(), dirty: false, max_installs: MAX_INSTALLS };
        store.data.installs.retain(|k, _| proto::norm_key(k).as_deref() == Some(k.as_str()));
        let codes: Vec<(String, String)> = store.data.installs.iter().map(|(k, i)| (i.code.clone(), k.clone())).collect();
        for (code, key) in codes {
            if store.by_code.insert(code, key.clone()).is_some() {
                // Duplicate code in a hand-edited file: give this one a new code.
                store.issue_code(&key, unix_now());
            }
        }
        Ok(store)
    }

    pub fn get(&self, key: &str) -> Option<&Install> {
        self.data.installs.get(key)
    }

    pub fn count(&self) -> usize {
        self.data.installs.len()
    }

    pub fn is_full(&self) -> bool {
        self.data.installs.len() >= self.max_installs
    }

    fn issue_code(&mut self, key: &str, now: u64) {
        let code = loop {
            let c = new_code();
            if !self.by_code.contains_key(&c) {
                break c;
            }
        };
        if let Some(inst) = self.data.installs.get_mut(key) {
            if self.by_code.get(&inst.code).map(String::as_str) == Some(key) {
                self.by_code.remove(&inst.code);
            }
            inst.code = code.clone();
            inst.code_at = now;
            self.by_code.insert(code, key.to_string());
            self.dirty = true;
        }
    }

    /// A fresh code for `key` right now (user asked for a new one).
    pub fn rotate(&mut self, key: &str, now: u64) {
        self.issue_code(key, now);
    }

    /// Record a successful authentication. Creates the install (with a code)
    /// if new — `false` if the store is full; updates its id label and
    /// friend list (written only if something changed). When full, first
    /// evicts the least recently seen install that has no friends and is not
    /// `online`.
    pub fn register_with(&mut self, key: &str, id: &str, friends: Vec<String>, now: u64, online: &dyn Fn(&str) -> bool) -> bool {
        if self.is_full() && !self.data.installs.contains_key(key) {
            let stalest = |keep: &dyn Fn(&str, &Install) -> bool| {
                let offline = self.data.installs.iter().filter(|(k, i)| !online(k) && !keep(k, i));
                offline.min_by_key(|(_, i)| i.last_seen).map(|(k, _)| k.clone())
            };
            // Friendless first, then friends that are still new.
            let victim = stalest(&|k, i| self.has_friends(k, i)).or_else(|| stalest(&|_, i| i.seen_days >= ESTABLISHED_DAYS));
            if let Some(v) = victim {
                self.remove(&v);
            }
        }
        let full = self.is_full();
        match self.data.installs.get_mut(key) {
            Some(inst) => {
                if inst.id != id || inst.friends != friends {
                    inst.id = id.to_string();
                    inst.friends = friends;
                    self.dirty = true;
                }
                self.touch(key, now);
                true
            }
            None if full => false,
            None => {
                let inst = Install { id: id.to_string(), friends, code: String::new(), code_at: 0, last_seen: now, active: false, seen_days: 1 };
                self.data.installs.insert(key.to_string(), inst);
                self.dirty = true;
                self.issue_code(key, now);
                true
            }
        }
    }

    pub fn set_friends(&mut self, key: &str, friends: Vec<String>) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if inst.friends != friends {
                inst.friends = friends;
                self.dirty = true;
            }
        }
    }

    /// The install took part in a code redeem: keep it like a befriended one.
    pub fn mark_active(&mut self, key: &str) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if !inst.active {
                inst.active = true;
                self.dirty = true;
            }
        }
    }

    /// Note activity; only dirties the store every `TOUCH_EVERY_SECS`.
    pub fn touch(&mut self, key: &str, now: u64) {
        if let Some(inst) = self.data.installs.get_mut(key) {
            if now.saturating_sub(inst.last_seen) >= TOUCH_EVERY_SECS {
                if now / DAY_SECS != inst.last_seen / DAY_SECS {
                    inst.seen_days = inst.seen_days.saturating_add(1);
                }
                inst.last_seen = now;
                self.dirty = true;
            }
        }
    }

    /// The install's current code and its remaining lifetime (seconds),
    /// rotating it first if it is `ttl` old. `rotated` = a new code was issued.
    pub fn code(&mut self, key: &str, now: u64, ttl: u64) -> Option<(String, u64, bool)> {
        let due = self.data.installs.get(key).map(|i| now >= i.code_at.saturating_add(ttl))?;
        if due {
            self.issue_code(key, now);
        }
        let inst = self.data.installs.get(key)?;
        let left = inst.code_at.saturating_add(ttl).saturating_sub(now).max(1);
        Some((inst.code.clone(), left, due))
    }

    /// Install key for a (normalized) code, if it is current.
    pub fn by_code(&self, code: &str, now: u64, ttl: u64) -> Option<&str> {
        let key = self.by_code.get(code)?;
        let inst = self.data.installs.get(key)?;
        (inst.code == code && now < inst.code_at.saturating_add(ttl)).then_some(key.as_str())
    }

    /// Worth keeping: took part in a redeem, or has at least one *mutual*
    /// friend (a one-sided, self-declared list doesn't count).
    pub fn befriended(&self, key: &str) -> bool {
        self.data.installs.get(key).is_some_and(|i| i.active || i.friends.iter().any(|f| self.mutual(key, f)))
    }

    /// Does `a` list `b` and `b` list `a`?
    pub fn mutual(&self, a: &str, b: &str) -> bool {
        let wants = |x: &str, y: &str| self.data.installs.get(x).is_some_and(|i| i.friends.iter().any(|f| f == y));
        wants(a, b) && wants(b, a)
    }

    /// Forget an install (operator clean-up).
    pub fn remove(&mut self, key: &str) -> bool {
        match self.data.installs.remove(key) {
            Some(i) => {
                self.by_code.remove(&i.code);
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Worth keeping as someone's friend: `befriended`, or established with a
    /// friend list, which stays so when a friend happens to be forgotten.
    fn has_friends(&self, key: &str, inst: &Install) -> bool {
        self.befriended(key) || (inst.seen_days >= ESTABLISHED_DAYS && !inst.friends.is_empty())
    }

    /// How long `key` is kept unseen.
    fn keep_for(&self, key: &str, inst: &Install) -> u64 {
        if !self.has_friends(key, inst) {
            IDLE_FORGET_SECS
        } else if inst.seen_days < ESTABLISHED_DAYS {
            NEW_FRIEND_FORGET_SECS
        } else {
            FORGET_AFTER_SECS
        }
    }

    /// Forget installs unseen for longer than `keep_for` says; never an
    /// `online` one. Friendships are only looked at for installs at least
    /// `IDLE_FORGET_SECS` unseen, so the usual pass is a plain scan.
    pub fn prune_with(&mut self, now: u64, online: &dyn Fn(&str) -> bool) {
        let expired: Vec<String> = self
            .data
            .installs
            .iter()
            .filter(|(k, i)| {
                let idle = now.saturating_sub(i.last_seen);
                idle >= IDLE_FORGET_SECS && !online(k) && idle >= self.keep_for(k, i)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in expired {
            self.remove(&k);
        }
    }

    #[cfg(test)]
    pub(crate) fn touch_at_for_test(&mut self, key: &str, when: u64) {
        self.data.installs.get_mut(key).unwrap().last_seen = when;
    }

    /// If anything changed, a copy to write (marks the store clean; call
    /// [`Store::mark_dirty`] if the write fails).
    pub fn snapshot(&mut self) -> Option<Snapshot> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let dir = self.dir.clone()?;
        Some(Snapshot { dir, data: self.data.clone() })
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

}

/// Write a snapshot: private temp file, fsync, keep the old file as
/// `relay.json.bak`, rename into place, fsync the directory (best effort).
pub fn write_snapshot(snap: &Snapshot) -> io::Result<()> {
    let main = snap.dir.join(DATA_FILE);
    let tmp = snap.dir.join(format!("{DATA_FILE}.tmp"));
    let bytes = serde_json::to_vec(&snap.data).map_err(io::Error::other)?;
    write_private(&tmp, &bytes)?;
    if main.exists() {
        std::fs::rename(&main, snap.dir.join(format!("{DATA_FILE}.bak")))?;
    }
    std::fs::rename(&tmp, &main)?;
    if let Ok(d) = std::fs::File::open(&snap.dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// The relay's long-term X25519 private key from `relay.key` (hex), created
/// on first start.
pub fn load_or_create_key(dir: &Path) -> io::Result<[u8; 32]> {
    private_dir(dir)?;
    let path = dir.join(KEY_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
            proto::hex32(s.trim()).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "relay.key is corrupt"))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let key = proto::random32();
            let tmp = dir.join(format!("{KEY_FILE}.tmp"));
            write_private(&tmp, proto::to_hex(&key).as_bytes())?;
            std::fs::rename(&tmp, &path)?;
            Ok(key)
        }
        Err(e) => Err(e),
    }
}

/// Write `bytes` to a new 0600 file (unix) and fsync it.
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?; // file may have existed
    }
    f.write_all(bytes)?;
    f.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(n: u8) -> String {
        proto::to_hex(&[n; 32])
    }

    impl Store {
        fn register(&mut self, key: &str, id: &str, friends: Vec<String>, now: u64) -> bool {
            self.register_with(key, id, friends, now, &|_| false)
        }

        fn prune(&mut self, now: u64) {
            self.prune_with(now, &|_| false)
        }

        fn flush(&mut self) -> io::Result<()> {
            match self.snapshot() {
                Some(snap) => write_snapshot(&snap),
                None => Ok(()),
            }
        }

        fn establish(&mut self, key: &str) {
            self.data.installs.get_mut(key).unwrap().seen_days = ESTABLISHED_DAYS;
        }
    }

    #[test]
    fn codes_rotate_after_ttl_and_old_codes_stop_working() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        let (c1, left, rotated) = s.code(&k(1), 1000, 100).unwrap();
        assert!(!rotated);
        assert_eq!(left, 100);
        assert_eq!(s.by_code(&c1, 1050, 100), Some(k(1).as_str()));
        let (c2, _, rotated) = s.code(&k(1), 1100, 100).unwrap();
        assert!(rotated);
        assert_ne!(c1, c2);
        assert_eq!(s.by_code(&c1, 1100, 100), None);
        assert_eq!(s.by_code(&c2, 1100, 100), Some(k(1).as_str()));
        // Without rotation having run, an expired code is still refused.
        assert_eq!(s.by_code(&c2, 1200, 100), None);
    }

    #[test]
    fn mutual_needs_both_sides() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(2)], 0);
        s.register(&k(2), "b", vec![], 0);
        assert!(!s.mutual(&k(1), &k(2)));
        s.set_friends(&k(2), vec![k(1)]);
        assert!(s.mutual(&k(1), &k(2)) && s.mutual(&k(2), &k(1)));
    }

    #[test]
    fn persists_and_reloads() {
        let dir = std::env::temp_dir().join(format!("screenpet-relay-store-{}", proto::to_hex(&proto::random32()[..8])));
        {
            let mut s = Store::open(&dir).unwrap();
            s.register(&k(1), "a", vec![k(2)], unix_now());
            s.flush().unwrap();
        }
        let s = Store::open(&dir).unwrap();
        let i = s.get(&k(1)).unwrap();
        assert_eq!(i.id, "a");
        assert_eq!(i.friends, vec![k(2)]);
        assert_eq!(s.by_code(&i.code.clone(), unix_now(), 3600), Some(k(1).as_str()));
        let key = load_or_create_key(&dir).unwrap();
        assert_eq!(load_or_create_key(&dir).unwrap(), key);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_forgets_long_unseen_installs() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 0);
        s.register(&k(2), "b", vec![], FORGET_AFTER_SECS);
        s.prune(FORGET_AFTER_SECS + 1);
        assert!(s.get(&k(1)).is_none());
        assert!(s.get(&k(2)).is_some());
    }

    fn tmpdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("screenpet-relay-{tag}-{}", proto::to_hex(&proto::random32()[..8])))
    }

    #[test]
    fn torn_data_file_falls_back_to_backup_then_empty() {
        let dir = tmpdir("torn");
        {
            let mut s = Store::open(&dir).unwrap();
            s.register(&k(1), "a", vec![], unix_now());
            s.flush().unwrap();
            s.register(&k(2), "b", vec![], unix_now());
            s.flush().unwrap(); // the first state is now the backup
        }
        std::fs::write(dir.join(DATA_FILE), b"{\"installs\": {").unwrap();
        let s = Store::open(&dir).unwrap();
        assert!(s.get(&k(1)).is_some(), "loaded from relay.json.bak");
        std::fs::write(dir.join(format!("{DATA_FILE}.bak")), b"garbage").unwrap();
        let s = Store::open(&dir).unwrap();
        assert_eq!(s.count(), 0, "both torn: start empty instead of refusing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn data_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmpdir("perm");
        let mut s = Store::open(&dir).unwrap();
        s.register(&k(1), "a", vec![], unix_now());
        s.flush().unwrap();
        load_or_create_key(&dir).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&dir.join(DATA_FILE)), 0o600);
        assert_eq!(mode(&dir.join(KEY_FILE)), 0o600);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installs_are_capped() {
        let mut s = Store::memory();
        s.max_installs = 2;
        assert!(s.register(&k(1), "a", vec![k(2)], 0));
        assert!(s.register(&k(2), "b", vec![k(1)], 0));
        s.establish(&k(1));
        s.establish(&k(2));
        assert!(!s.register(&k(3), "c", vec![], 0), "full of established mutual friends");
        assert!(s.register(&k(1), "a", vec![k(2)], 0), "known installs still log in");
    }

    #[test]
    fn touch_dirties_at_most_every_ten_minutes() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        s.dirty = false;
        s.touch(&k(1), 1100);
        assert!(!s.dirty);
        s.register(&k(1), "a", vec![], 1200);
        assert!(!s.dirty, "unchanged login is not a write");
        s.touch(&k(1), 1000 + 601);
        assert!(s.dirty);
    }

    #[test]
    fn idle_installs_without_friends_expire_after_a_day() {
        let mut s = Store::memory();
        s.register(&k(1), "lonely", vec![], 0);
        s.register(&k(2), "friendly", vec![k(4)], 0);
        s.register(&k(4), "its friend", vec![k(2)], 0);
        s.register(&k(3), "redeemer", vec![], 0);
        s.mark_active(&k(3));
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_none(), "never befriended, not back within a day");
        assert!(s.get(&k(2)).is_some());
        assert!(s.get(&k(3)).is_some());
    }

    #[test]
    fn keys_befriending_each_other_get_no_long_retention() {
        let mut s = Store::memory();
        s.register(&k(1), "new", vec![k(2)], 0);
        s.register(&k(2), "new", vec![k(1)], 0);
        s.register(&k(3), "old", vec![k(4)], 0);
        s.register(&k(4), "old", vec![k(3)], 0);
        s.establish(&k(3));
        s.establish(&k(4));
        s.prune(NEW_FRIEND_FORGET_SECS - 1);
        assert!(s.get(&k(1)).is_some(), "kept past the friendless day");
        s.prune(NEW_FRIEND_FORGET_SECS);
        assert!(s.get(&k(1)).is_none() && s.get(&k(2)).is_none());
        assert!(s.get(&k(3)).is_some() && s.get(&k(4)).is_some(), "seen on several days");
        s.prune(FORGET_AFTER_SECS);
        assert!(s.get(&k(3)).is_none());
    }

    #[test]
    fn a_full_store_gives_way_to_new_installs_over_new_friends() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(1), "clique", vec![k(2)], 500);
        s.register(&k(2), "clique", vec![k(1)], 600);
        s.register(&k(3), "old", vec![k(4)], 0);
        s.establish(&k(3));
        s.register(&k(7), "lonely", vec![], 650);
        assert_eq!(s.count(), 3, "the stalest friendless install went");
        assert!(s.get(&k(3)).is_some(), "established, so its friend list still counts");
        assert!(s.get(&k(1)).is_none() || s.get(&k(2)).is_none(), "a new friend made room for lonely");
        assert!(!s.register_with(&k(6), "x", vec![], 800, &|key| key == k(7) || key == k(2)), "only established ones left");
    }

    #[test]
    fn days_seen_count_distinct_days() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![], 1000);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 1);
        s.touch(&k(1), 2000);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 1, "same day");
        s.touch(&k(1), DAY_SECS + 5);
        assert_eq!(s.get(&k(1)).unwrap().seen_days, 2);
    }

    #[test]
    fn older_data_files_still_load() {
        let dir = tmpdir("old");
        std::fs::create_dir_all(&dir).unwrap();
        let file = format!(r#"{{"installs":{{"{}":{{"id":"a","code":"ABCD-1234","code_at":5,"last_seen":7}}}}}}"#, k(1));
        std::fs::write(dir.join(DATA_FILE), file).unwrap();
        let s = Store::open(&dir).unwrap();
        assert_eq!(s.get(&k(1)).unwrap().seen_days, ESTABLISHED_DAYS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fields of a real relay.json from before `seen_days`.
    #[test]
    fn friends_from_an_older_file_keep_their_long_retention() {
        let dir = tmpdir("legacy");
        std::fs::create_dir_all(&dir).unwrap();
        let install = |friend: String, code: &str| serde_json::json!({"id": "a", "friends": [friend], "code": code, "code_at": 5, "last_seen": 0, "active": false});
        let file = serde_json::json!({"installs": {k(1): install(k(2), "ABCD-1234"), k(2): install(k(1), "ABCD-1235")}});
        std::fs::write(dir.join(DATA_FILE), file.to_string()).unwrap();
        let mut s = Store::open(&dir).unwrap();
        s.prune(NEW_FRIEND_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_some() && s.get(&k(2)).is_some(), "past 14 days");
        s.register(&k(3), "new", vec![], 0);
        assert_eq!(s.get(&k(3)).unwrap().seen_days, 1, "new installs still start new");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_established_install_keeps_its_retention_when_its_friend_goes() {
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(2)], 0);
        s.register(&k(2), "b", vec![k(1)], 0);
        s.establish(&k(1));
        s.remove(&k(2));
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_some());
        s.prune(FORGET_AFTER_SECS);
        assert!(s.get(&k(1)).is_none());
    }

    #[test]
    fn a_full_store_evicts_the_stalest_friendless_install_first() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(1), "old", vec![], 0);
        s.register(&k(2), "newer", vec![], 1000);
        s.register(&k(3), "redeemer", vec![], 0);
        s.mark_active(&k(3));
        s.establish(&k(3));
        assert!(s.register_with(&k(4), "d", vec![], 2000, &|_| false));
        assert!(s.get(&k(1)).is_none(), "least recently seen friendless install evicted");
        assert!(s.get(&k(4)).is_some());
        // Online or befriended installs are never evicted: then it refuses.
        assert!(!s.register_with(&k(5), "e", vec![], 3000, &|key| key == k(2) || key == k(4)));
    }

    #[test]
    fn only_mutual_friendships_protect_an_install() {
        let mut s = Store::memory();
        s.max_installs = 3;
        s.register(&k(2), "b", vec![k(3)], 0);
        s.register(&k(3), "c", vec![k(2)], 0);
        s.establish(&k(2));
        s.establish(&k(3));
        // Self-declared friend that does not list it back: no protection.
        s.register(&k(1), "a", vec![k(9)], 500);
        assert!(s.register_with(&k(4), "d", vec![], 1000, &|_| false));
        assert!(s.get(&k(1)).is_none(), "one-sided list evicted before older mutual friends");
        assert!(s.get(&k(2)).is_some() && s.get(&k(3)).is_some());
        // Nothing evictable left (k4 online): refuse.
        assert!(!s.register_with(&k(5), "e", vec![], 2000, &|key| key == k(4)));
        // A one-sided list doesn't buy the long retention either.
        let mut s = Store::memory();
        s.register(&k(1), "a", vec![k(9)], 0);
        s.prune(IDLE_FORGET_SECS + 1);
        assert!(s.get(&k(1)).is_none());
    }
}
