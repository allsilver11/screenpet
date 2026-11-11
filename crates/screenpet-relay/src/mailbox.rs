//! Feedback letters to the developer (`letters.json`). The relay keeps the
//! sealed boxes it can't read and enforces every sending limit itself — an
//! app can be modified, the relay can't.

use crate::proto;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io;
use std::path::Path;

pub const LETTERS_FILE: &str = "letters.json";
/// Largest sealed letter (1000 characters of Korean text and a little JSON
/// fit well under this).
pub const MAX_BOX: usize = 8192;
/// Letters kept; past this new letters are refused, old ones are never
/// dropped (a flood can't push out real letters).
pub const MAX_LETTERS: usize = 200;
/// Per rolling day: one install key, one address bucket, the whole relay.
pub const PER_KEY: usize = 1;
pub const PER_IP: usize = 3;
pub const GLOBAL: usize = 50;
/// An install must have been seen on this many distinct days (a key made a
/// minute ago can't send).
pub const MIN_SEEN_DAYS: u32 = 2;
pub const WINDOW_SECS: u64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredLetter {
    pub id: u64,
    /// Unix seconds.
    pub at: u64,
    /// Sender install key (hex).
    pub from: String,
    pub read: bool,
    /// The developer ticked it as acted on (반영함). Older files lack it.
    #[serde(default)]
    pub done: bool,
    /// The sealed letter, hex.
    pub r#box: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refuse {
    Daily,
    Ip,
    New,
    Global,
    Full,
    Size,
    Blocked,
}

impl Refuse {
    pub fn as_str(&self) -> &'static str {
        match self {
            Refuse::Daily => "daily",
            Refuse::Ip => "ip",
            Refuse::New => "new",
            Refuse::Global => "global",
            Refuse::Full => "full",
            Refuse::Size => "size",
            Refuse::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sent {
    key: String,
    ip: String,
    at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    next_id: u64,
    letters: Vec<StoredLetter>,
    sent: Vec<Sent>,
    blocked: BTreeSet<String>,
}

#[derive(Debug, Default)]
pub struct Mailbox {
    data: Data,
    dirty: bool,
}

impl Mailbox {
    /// Store a sealed letter from `from` (authenticated key) at address
    /// bucket `ip`, or say why not. `seen_days` comes from the install store.
    pub fn submit(&mut self, from: &str, ip: &str, seen_days: u32, sealed: &[u8], now: u64) -> Result<StoredLetter, Refuse> {
        if sealed.is_empty() || sealed.len() > MAX_BOX {
            return Err(Refuse::Size);
        }
        if self.data.blocked.contains(from) {
            return Err(Refuse::Blocked);
        }
        if seen_days < MIN_SEEN_DAYS {
            return Err(Refuse::New);
        }
        let before = self.data.sent.len();
        self.data.sent.retain(|s| now.saturating_sub(s.at) < WINDOW_SECS);
        if self.data.sent.len() != before {
            self.dirty = true;
        }
        let sent = &self.data.sent;
        if sent.iter().filter(|s| s.key == from).count() >= PER_KEY {
            return Err(Refuse::Daily);
        }
        if sent.iter().filter(|s| s.ip == ip).count() >= PER_IP {
            return Err(Refuse::Ip);
        }
        if sent.len() >= GLOBAL {
            return Err(Refuse::Global);
        }
        if self.data.letters.len() >= MAX_LETTERS {
            return Err(Refuse::Full);
        }
        self.data.sent.push(Sent { key: from.to_string(), ip: ip.to_string(), at: now });
        self.data.next_id += 1;
        let letter = StoredLetter { id: self.data.next_id, at: now, from: from.to_string(), read: false, done: false, r#box: proto::to_hex(sealed) };
        self.data.letters.push(letter.clone());
        self.dirty = true;
        Ok(letter)
    }

    /// All letters, newest first.
    pub fn list(&self) -> Vec<StoredLetter> {
        let mut v = self.data.letters.clone();
        v.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
        v
    }

    pub fn unread(&self) -> usize {
        self.data.letters.iter().filter(|l| !l.read).count()
    }

    pub fn mark_read(&mut self, id: u64) -> bool {
        match self.data.letters.iter_mut().find(|l| l.id == id) {
            Some(l) if !l.read => {
                l.read = true;
                self.dirty = true;
                true
            }
            Some(_) => true,
            None => false,
        }
    }

    /// Tick (or untick) a letter as acted on; false if there is no such letter.
    pub fn mark_done(&mut self, id: u64, done: bool) -> bool {
        match self.data.letters.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                if l.done != done {
                    l.done = done;
                    self.dirty = true;
                }
                true
            }
            None => false,
        }
    }

    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.id != id);
        let gone = self.data.letters.len() != before;
        self.dirty |= gone;
        gone
    }

    /// Refuse `key` from now on and delete its letters (returns how many).
    pub fn block(&mut self, key: &str) -> usize {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.from != key);
        self.data.blocked.insert(key.to_string());
        self.dirty = true;
        before - self.data.letters.len()
    }

    #[cfg(test)]
    pub fn is_blocked(&self, key: &str) -> bool {
        self.data.blocked.contains(key)
    }

    /// The file bytes if anything changed since the last snapshot.
    pub fn snapshot(&mut self) -> Option<Vec<u8>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        serde_json::to_vec(&self.data).ok()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `letters.json`, else its `.bak`, else an empty mailbox (letters may be
    /// lost, the relay still starts).
    pub fn load(dir: &Path) -> Mailbox {
        for name in [LETTERS_FILE.to_string(), format!("{LETTERS_FILE}.bak")] {
            if let Ok(bytes) = std::fs::read(dir.join(&name)) {
                if let Ok(data) = serde_json::from_slice::<Data>(&bytes) {
                    return Mailbox { data, dirty: false };
                }
                eprintln!("screenpet-relay: {name} is unreadable");
            }
        }
        Mailbox::default()
    }

    /// Write like `relay.json`: private temp file, keep the old one as `.bak`, rename.
    pub fn save(dir: &Path, bytes: &[u8]) -> io::Result<()> {
        let main = dir.join(LETTERS_FILE);
        let tmp = dir.join(format!("{LETTERS_FILE}.tmp"));
        write_private(&tmp, bytes)?;
        if main.exists() {
            std::fs::rename(&main, dir.join(format!("{LETTERS_FILE}.bak")))?;
        }
        std::fs::rename(&tmp, &main)?;
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mb() -> Mailbox {
        Mailbox::default()
    }
    const T: u64 = 1_000_000;

    fn tmp_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("screenpet-mailbox-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_known_install_can_send_one_letter_a_day() {
        let mut m = mb();
        assert!(m.submit("k1", "1.2.3.4", 2, b"x", T).is_ok());
        assert_eq!(m.submit("k1", "1.2.3.4", 2, b"x", T + 10), Err(Refuse::Daily));
        assert!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS).is_ok());
    }

    #[test]
    fn three_per_address_and_fifty_overall_per_day() {
        let mut m = mb();
        for k in 0..PER_IP {
            assert!(m.submit(&format!("a{k}"), "9.9.9.9", 5, b"x", T).is_ok());
        }
        assert_eq!(m.submit("a3", "9.9.9.9", 5, b"x", T), Err(Refuse::Ip));
        let mut m = mb();
        for k in 0..GLOBAL {
            assert!(m.submit(&format!("g{k}"), &format!("10.0.{}.{}", k / 200, k % 200), 5, b"x", T).is_ok());
        }
        assert_eq!(m.submit("late", "8.8.8.8", 5, b"x", T), Err(Refuse::Global));
    }

    #[test]
    fn brand_new_keys_bad_sizes_and_blocked_keys_are_refused() {
        let mut m = mb();
        assert_eq!(m.submit("new", "1.1.1.1", 1, b"x", T), Err(Refuse::New));
        assert_eq!(m.submit("k", "1.1.1.1", 2, &vec![0; MAX_BOX + 1], T), Err(Refuse::Size));
        assert_eq!(m.submit("k", "1.1.1.1", 2, b"", T), Err(Refuse::Size));
        let id = m.submit("bad", "2.2.2.2", 3, b"x", T).unwrap().id;
        assert_eq!(m.block("bad"), 1);
        assert!(m.list().iter().all(|l| l.id != id));
        assert_eq!(m.submit("bad", "3.3.3.3", 3, b"x", T + WINDOW_SECS), Err(Refuse::Blocked));
        assert_eq!(Refuse::Blocked.as_str(), "blocked");
    }

    #[test]
    fn a_full_inbox_refuses_new_letters_instead_of_dropping_old_ones() {
        let mut m = mb();
        for i in 0..MAX_LETTERS as u64 {
            m.submit(&format!("k{i}"), &format!("10.{i}.0.1"), 2, b"x", T + i * WINDOW_SECS).unwrap();
        }
        let late = T + 999 * WINDOW_SECS;
        assert_eq!(m.submit("one-more", "7.7.7.7", 2, b"x", late), Err(Refuse::Full));
        assert_eq!(m.list().len(), MAX_LETTERS);
        let oldest = m.list().last().unwrap().id;
        assert!(m.delete(oldest));
        assert!(m.submit("one-more", "7.7.7.7", 2, b"x", late).is_ok());
    }

    #[test]
    fn letters_list_newest_first_and_read_flags_stick() {
        let mut m = mb();
        let a = m.submit("a", "1.0.0.1", 2, b"aa", T).unwrap().id;
        let b = m.submit("b", "1.0.0.2", 2, b"bb", T + 5).unwrap().id;
        assert_eq!(m.list().iter().map(|l| l.id).collect::<Vec<_>>(), vec![b, a]);
        assert!(m.mark_read(a) && !m.mark_read(999));
        assert!(m.list().iter().find(|l| l.id == a).unwrap().read);
        assert_eq!(m.list().iter().find(|l| l.id == b).unwrap().r#box, "6262");
        assert_eq!(m.unread(), 1);
    }

    #[test]
    fn the_developer_can_tick_a_letter_as_done_and_it_is_kept() {
        let dir = tmp_dir("done");
        let mut m = mb();
        let id = m.submit("a", "1.0.0.1", 2, b"x", T).unwrap().id;
        assert!(!m.list()[0].done);
        assert!(m.mark_done(id, true) && !m.mark_done(999, true));
        assert!(m.list()[0].done);
        Mailbox::save(&dir, &m.snapshot().unwrap()).unwrap();
        assert!(Mailbox::load(&dir).list()[0].done, "survives a restart");
        assert!(m.mark_done(id, false));
        assert!(!m.list()[0].done);
        // 옛 파일(done 칸 없음)도 읽힌다
        let old = br#"{"next_id":1,"letters":[{"id":1,"at":5,"from":"a","read":true,"box":"00"}]}"#;
        std::fs::write(dir.join(LETTERS_FILE), old).unwrap();
        assert!(!Mailbox::load(&dir).list()[0].done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn it_survives_a_restart_and_a_torn_file() {
        let dir = tmp_dir("restart");
        let mut m = mb();
        m.submit("a", "1.0.0.1", 2, b"hello", T).unwrap();
        m.block("evil");
        Mailbox::save(&dir, &m.snapshot().unwrap()).unwrap();
        assert!(m.snapshot().is_none(), "clean after a snapshot");
        m.submit("b", "1.0.0.2", 2, b"x", T + 1).unwrap();
        Mailbox::save(&dir, &m.snapshot().unwrap()).unwrap();
        let back = Mailbox::load(&dir);
        assert_eq!(back.list().len(), 2);
        assert!(back.is_blocked("evil"));
        std::fs::write(dir.join(LETTERS_FILE), b"{torn").unwrap();
        let mut back = Mailbox::load(&dir);
        assert_eq!(back.list().len(), 1, "falls back to .bak");
        assert_eq!(back.submit("a", "1.0.0.9", 2, b"x", T + 2), Err(Refuse::Daily), "the day limit is remembered");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Mailbox::load(&dir).list().len(), 0, "nothing at all: empty inbox");
    }
}
