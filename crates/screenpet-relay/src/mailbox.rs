//! Feedback letters to the developer (`letters.json`). The relay keeps the
//! sealed boxes it can't read and enforces every sending limit itself — an
//! app can be modified, the relay can't.

use crate::board::{self, Entry, Item, Status};
use crate::proto;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

pub const LETTERS_FILE: &str = "letters.json";
/// Largest sealed letter (1000 characters of Korean text and a little JSON
/// fit well under this).
pub const MAX_BOX: usize = 8192;
/// Letters kept; past this new letters are refused, old ones are never
/// dropped (a flood can't push out real letters).
pub const MAX_LETTERS: usize = 200;
/// Per rolling day: one install key (5 letters since v0.6.3), one address
/// bucket (room for a sender's 5 and a housemate's), the whole relay.
pub const PER_KEY: usize = 5;
pub const PER_IP: usize = 10;
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
    /// 🙅 The developer turned it down (반려). Never together with `done`;
    /// older files and older apps know only `done`.
    #[serde(default)]
    pub rejected: bool,
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
    /// 📮 Public digests by letter id (only for letters still here).
    board: BTreeMap<u64, Entry>,
}

impl StoredLetter {
    pub fn status(&self) -> Status {
        if self.done {
            Status::Done
        } else if self.rejected {
            Status::Rejected
        } else {
            Status::None
        }
    }
}

/// Why a board digest was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardRefuse {
    /// No such letter (deleted, blocked, never was).
    NoLetter,
    /// The name is not a mask (`김**`) or too long.
    Name,
    /// The line is empty or too long.
    Line,
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
        let letter = StoredLetter { id: self.data.next_id, at: now, from: from.to_string(), read: false, done: false, rejected: false, r#box: proto::to_hex(sealed) };
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

    /// Set a letter's mark (none, ✅ done, 🙅 rejected — one at a time);
    /// false if there is no such letter.
    pub fn set_status(&mut self, id: u64, status: Status) -> bool {
        match self.data.letters.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                let (done, rejected) = (status == Status::Done, status == Status::Rejected);
                if (l.done, l.rejected) != (done, rejected) {
                    l.done = done;
                    l.rejected = rejected;
                    self.dirty = true;
                }
                true
            }
            None => false,
        }
    }

    /// Tick (or untick) a letter as acted on (`letter_done`, also from older
    /// apps): unticking clears a 반려 too. False if there is no such letter.
    pub fn mark_done(&mut self, id: u64, done: bool) -> bool {
        self.set_status(id, if done { Status::Done } else { Status::None })
    }

    /// 🙅 Turn a letter down (or clear its mark).
    pub fn mark_rejected(&mut self, id: u64, rejected: bool) -> bool {
        self.set_status(id, if rejected { Status::Rejected } else { Status::None })
    }

    /// Board entries whose letter is gone go too.
    fn prune_board(&mut self) {
        let ids: BTreeSet<u64> = self.data.letters.iter().map(|l| l.id).collect();
        let before = self.data.board.len();
        self.data.board.retain(|id, _| ids.contains(id));
        self.dirty |= self.data.board.len() != before;
    }

    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.id != id);
        let gone = self.data.letters.len() != before;
        self.dirty |= gone;
        self.prune_board();
        gone
    }

    /// Refuse `key` from now on and delete its letters (returns how many).
    pub fn block(&mut self, key: &str) -> usize {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.from != key);
        self.data.blocked.insert(key.to_string());
        self.dirty = true;
        self.prune_board();
        before - self.data.letters.len()
    }

    /// 📮 Store the public digest of letter `id` (checked again: a real mask,
    /// one short line). `Ok(true)` if it changed anything.
    pub fn board_put(&mut self, id: u64, name: &str, line: &str) -> Result<bool, BoardRefuse> {
        if !self.data.letters.iter().any(|l| l.id == id) {
            return Err(BoardRefuse::NoLetter);
        }
        let name = board::check_name(name).ok_or(BoardRefuse::Name)?;
        let line = board::check_line(line).ok_or(BoardRefuse::Line)?;
        if self.data.board.len() >= MAX_LETTERS && !self.data.board.contains_key(&id) {
            return Err(BoardRefuse::NoLetter); // can't happen while entries follow letters
        }
        let entry = Entry { name, line };
        if self.data.board.get(&id) == Some(&entry) {
            return Ok(false);
        }
        self.data.board.insert(id, entry);
        self.dirty = true;
        Ok(true)
    }

    /// Ids of the letters on the board (the developer's app publishes the rest).
    pub fn board_ids(&self) -> Vec<u64> {
        self.data.board.keys().copied().collect()
    }

    /// 📮 Page `want` (1-based) of the board, newest letter first:
    /// (items, page, pages, total).
    pub fn board_page(&self, want: u32) -> (Vec<Item>, u32, u32, u32) {
        let all: Vec<Item> = self
            .list()
            .into_iter()
            .filter_map(|l| self.data.board.get(&l.id).map(|e| Item { id: l.id, name: e.name.clone(), line: e.line.clone(), at: l.at, status: l.status() }))
            .collect();
        let (start, end, page, pages) = board::page_range(all.len(), want);
        (all[start..end].to_vec(), page, pages, all.len() as u32)
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
                    let mut m = Mailbox { data, dirty: false };
                    m.prune_board();
                    return m;
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
    fn a_known_install_can_send_five_letters_a_day() {
        let mut m = mb();
        assert_eq!(PER_KEY, 5);
        for i in 0..5 {
            assert!(m.submit("k1", "1.2.3.4", 2, b"x", T + i).is_ok(), "letter {}", i + 1);
        }
        assert_eq!(m.submit("k1", "1.2.3.4", 2, b"x", T + 10), Err(Refuse::Daily), "the 6th in a day");
        assert!(m.submit("k2", "1.2.3.4", 2, b"x", T + 10).is_ok(), "counted per sender");
        assert_eq!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS - 1), Err(Refuse::Daily));
        assert!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS).is_ok(), "the first one is a day old: one more");
        assert_eq!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS), Err(Refuse::Daily));
        for i in 1..5 {
            assert!(m.submit("k1", "1.2.3.4", 2, b"x", T + WINDOW_SECS + i).is_ok(), "the next day: five again");
        }
    }

    #[test]
    fn ten_per_address_and_fifty_overall_per_day() {
        assert!(PER_IP >= PER_KEY, "one sender's five never hit the address limit");
        let mut m = mb();
        for k in 0..PER_IP {
            assert!(m.submit(&format!("a{k}"), "9.9.9.9", 5, b"x", T).is_ok());
        }
        assert_eq!(m.submit("late", "9.9.9.9", 5, b"x", T), Err(Refuse::Ip));
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
        for i in 1..PER_KEY as u64 {
            assert!(back.submit("a", "1.0.0.9", 2, b"x", T + 1 + i).is_ok());
        }
        assert_eq!(back.submit("a", "1.0.0.9", 2, b"x", T + 9), Err(Refuse::Daily), "the day limit is remembered");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(Mailbox::load(&dir).list().len(), 0, "nothing at all: empty inbox");
    }

    #[test]
    fn done_and_rejected_are_one_status_and_old_done_false_clears_either() {
        let mut m = mb();
        let id = m.submit("a", "1.0.0.1", 2, b"x", T).unwrap().id;
        assert_eq!(m.list()[0].status(), Status::None);
        assert!(m.mark_rejected(id, true));
        assert_eq!(m.list()[0].status(), Status::Rejected);
        assert!(m.mark_done(id, true), "반영 replaces 반려");
        assert!((m.list()[0].done, m.list()[0].rejected) == (true, false));
        assert!(m.mark_rejected(id, true), "반려 replaces 반영");
        assert!((m.list()[0].done, m.list()[0].rejected) == (false, true));
        assert!(m.mark_done(id, false), "an older app's untick clears 반려 too");
        assert_eq!(m.list()[0].status(), Status::None);
        assert!(m.mark_rejected(id, true) && m.mark_rejected(id, false));
        assert_eq!(m.list()[0].status(), Status::None);
        assert!(!m.mark_rejected(999, true));
        let old: StoredLetter = serde_json::from_str(r#"{"id":1,"at":5,"from":"a","read":true,"done":true,"box":"00"}"#).unwrap();
        assert_eq!(old.status(), Status::Done, "a stored done=true is still done");
    }

    #[test]
    fn the_board_takes_only_masked_digests_of_letters_that_exist() {
        let mut m = mb();
        let a = m.submit("a", "1.0.0.1", 2, b"x", T).unwrap().id;
        assert_eq!(m.board_put(a, "김**", "펭귄 더 주세요"), Ok(true));
        assert_eq!(m.board_put(a, "김**", "펭귄 더 주세요"), Ok(false), "same again: nothing changes");
        assert_eq!(m.board_put(a, "김철수", "x"), Err(BoardRefuse::Name));
        assert_eq!(m.board_put(a, "김*수", "x"), Err(BoardRefuse::Name));
        assert_eq!(m.board_put(a, "김**", " "), Err(BoardRefuse::Line));
        assert_eq!(m.board_put(a, "김**", &"가".repeat(81)), Err(BoardRefuse::Line));
        assert_eq!(m.board_put(999, "김**", "x"), Err(BoardRefuse::NoLetter));
        assert_eq!(m.board_put(0, "김**", "x"), Err(BoardRefuse::NoLetter));
        assert_eq!(m.board_put(a, "\u{202E}*", "a\u{202E}b"), Ok(true), "bidi characters are dropped, not kept");
        let (items, ..) = m.board_page(1);
        assert_eq!((items[0].name.as_str(), items[0].line.as_str()), ("*", "ab"));
        assert_eq!(m.board_ids(), vec![a]);
    }

    #[test]
    fn board_pages_are_newest_first_with_the_letters_date_and_status() {
        let mut m = mb();
        let mut ids = Vec::new();
        for i in 0..12u64 {
            let id = m.submit(&format!("k{i}"), &format!("10.0.0.{i}"), 2, b"x", T + i).unwrap().id;
            ids.push(id);
            if i != 3 {
                m.board_put(id, "*", &format!("줄 {i}")).unwrap();
            }
        }
        m.mark_done(ids[11], true);
        m.mark_rejected(ids[10], true);
        let (items, page, pages, total) = m.board_page(1);
        assert_eq!((page, pages, total), (1, 3, 11), "the letter without a digest isn't listed");
        assert_eq!(items.iter().map(|x| x.line.as_str()).collect::<Vec<_>>(), vec!["줄 11", "줄 10", "줄 9", "줄 8", "줄 7"]);
        assert_eq!((items[0].at, items[0].status, items[1].status, items[2].status), (T + 11, Status::Done, Status::Rejected, Status::None));
        let (items, page, ..) = m.board_page(3);
        assert_eq!((page, items.len(), items[0].line.as_str()), (3, 1, "줄 0"));
        assert_eq!(m.board_page(99).1, 3, "past the end: the last page");
        assert_eq!(mb().board_page(1), (vec![], 1, 1, 0), "an empty board");
        let json = serde_json::to_string(&items[0]).unwrap();
        assert!(json.contains(r#""status":"none""#), "{json}");
    }

    #[test]
    fn deleting_or_blocking_takes_the_board_entry_away_and_it_all_survives_a_restart() {
        let dir = tmp_dir("board");
        let mut m = mb();
        let a = m.submit("a", "1.0.0.1", 2, b"x", T).unwrap().id;
        let b = m.submit("b", "1.0.0.2", 2, b"x", T + 1).unwrap().id;
        let c = m.submit("b", "1.0.0.2", 2, b"x", T + 2).unwrap().id;
        let d = m.submit("d", "1.0.0.3", 2, b"x", T + 3).unwrap().id;
        for id in [a, b, c, d] {
            m.board_put(id, "*", "x").unwrap();
        }
        assert!(m.delete(a));
        assert_eq!(m.board_ids(), vec![b, c, d]);
        assert_eq!(m.block("b"), 2);
        assert_eq!(m.board_ids(), vec![d]);
        m.mark_rejected(d, true);
        Mailbox::save(&dir, &m.snapshot().unwrap()).unwrap();
        let back = Mailbox::load(&dir);
        assert_eq!(back.board_ids(), vec![d]);
        assert_eq!(back.board_page(1).0[0].status, Status::Rejected);
        // an entry without its letter (hand-edited file) is dropped on load
        let orphan = br#"{"next_id":2,"letters":[{"id":1,"at":5,"from":"a","read":true,"box":"00"}],"board":{"1":{"name":"*","line":"a"},"7":{"name":"*","line":"b"}}}"#;
        std::fs::write(dir.join(LETTERS_FILE), orphan).unwrap();
        assert_eq!(Mailbox::load(&dir).board_ids(), vec![1]);
        // a file from before the board still loads
        let old = br#"{"next_id":1,"letters":[{"id":1,"at":5,"from":"a","read":true,"done":true,"box":"00"}]}"#;
        std::fs::write(dir.join(LETTERS_FILE), old).unwrap();
        let old = Mailbox::load(&dir);
        assert!(old.board_ids().is_empty() && old.list()[0].status() == Status::Done);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_board_never_holds_more_than_the_mailbox() {
        let mut m = mb();
        for i in 0..MAX_LETTERS as u64 {
            let id = m.submit(&format!("k{i}"), &format!("10.{i}.0.1"), 2, b"x", T + i * WINDOW_SECS).unwrap().id;
            m.board_put(id, "*", "x").unwrap();
        }
        assert_eq!(m.board_ids().len(), MAX_LETTERS);
        assert_eq!(m.board_page(u32::MAX).2, (MAX_LETTERS / board::PER_PAGE) as u32);
    }
}
