//! 개발자에게 오는 건의 편지함임 (`letters.json`). 릴레이는 읽지 못하는 봉인 상자를 두고
//! 모든 보내기 한도를 스스로 지킴: 앱은 고쳐질 수 있지만 릴레이는 아님.
//! 📮 공개 건의함 요약도 편지 옆에 둠. `hub::mail`이 `Hub::mail` 잠금 안에서 씀.

use crate::board::{self, Entry, Item, Status};
use crate::proto;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

/// 데이터 폴더 안 편지함 파일 이름
pub const LETTERS_FILE: &str = "letters.json";
/// 가장 큰 봉인 편지 (한국어 1000자와 약간의 JSON이 넉넉히 들어감)
pub const MAX_BOX: usize = screenpet_proto::MAX_SEALED;
/// 두는 편지 수. 넘으면 새 편지를 거절하고 옛 편지는 절대 안 버림 (폭주가 진짜 편지를 밀어내지 못함)
pub const MAX_LETTERS: usize = 200;
/// 하루(미닫이)당 설치 키 하나의 편지 수 (v0.6.3부터 5통)
pub const PER_KEY: usize = 5;
/// 하루당 주소 묶음 하나의 편지 수 (보낸이 5통과 같이 사는 이 몫까지)
pub const PER_IP: usize = 10;
/// 하루당 릴레이 전체 편지 수
pub const GLOBAL: usize = 50;
/// 설치가 이만큼 서로 다른 날에 보였어야 보낼 수 있음 (방금 만든 키는 못 보냄)
pub const MIN_SEEN_DAYS: u32 = 2;
/// 하루 한도를 세는 창 (초)
pub const WINDOW_SECS: u64 = 24 * 3600;

/// 저장된 편지 하나임
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredLetter {
    /// 편지 id
    pub id: u64,
    /// 받은 때 (유닉스 초)
    pub at: u64,
    /// 보낸 설치 키 (hex)
    pub from: String,
    /// 읽음 표시
    pub read: bool,
    /// 개발자가 반영함으로 체크했음. 옛 파일엔 없음
    #[serde(default)]
    pub done: bool,
    /// 🙅 개발자가 반려했음. `done`과 함께는 아님. 옛 파일·옛 앱은 `done`만 앎
    #[serde(default)]
    pub rejected: bool,
    /// 🔒 개발자가 공개 건의함에서 숨겼음 (비공개). 옛 파일엔 없음: 공개
    #[serde(default)]
    pub private: bool,
    /// 봉인된 편지, hex
    pub r#box: String,
}

/// 디스크 편지를 와이어 편지로 바꿈 (필드가 같아 JSON도 같음)
impl From<StoredLetter> for screenpet_proto::relay::Letter {
    /// 필드를 그대로 옮김
    fn from(l: StoredLetter) -> Self {
        screenpet_proto::relay::Letter { id: l.id, at: l.at, from: l.from, read: l.read, done: l.done, rejected: l.rejected, private: l.private, r#box: l.r#box }
    }
}

/// 편지를 거절한 까닭임 (와이어 `letter_refused`의 reason)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refuse {
    /// 이 키의 하루 한도
    Daily,
    /// 이 주소의 하루 한도
    Ip,
    /// 너무 새 설치
    New,
    /// 릴레이 전체 하루 한도
    Global,
    /// 편지함이 참
    Full,
    /// 크기가 틀림
    Size,
    /// 막힌 보낸이
    Blocked,
}

impl Refuse {
    /// 와이어 이유 글을 돌려줌
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

/// 하루 한도를 세려고 남기는 보낸 기록임
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Sent {
    /// 보낸 설치 키
    key: String,
    /// 보낸 주소 묶음
    ip: String,
    /// 보낸 때 (유닉스 초)
    at: u64,
}

/// `letters.json`에 쓰는 내용임
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Data {
    /// 다음 편지 id
    next_id: u64,
    /// 편지들
    letters: Vec<StoredLetter>,
    /// 하루 한도용 보낸 기록
    sent: Vec<Sent>,
    /// 막힌 보낸이 키
    blocked: BTreeSet<String>,
    /// 📮 편지 id별 공개 요약 (아직 있는 편지만)
    board: BTreeMap<u64, Entry>,
}

impl StoredLetter {
    /// 편지의 표시(없음·✅·🙅)를 돌려줌
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

/// 건의함 요약을 받지 않은 까닭임
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardRefuse {
    /// 그런 편지 없음 (지움, 차단, 처음부터 없음)
    NoLetter,
    /// 이름이 가림(`김**`)이 아니거나 너무 김
    Name,
    /// 줄이 비었거나 너무 김
    Line,
}

/// 편지함임: 내용과 저장할 게 있는지
#[derive(Debug, Default)]
pub struct Mailbox {
    data: Data,
    dirty: bool,
}

impl Mailbox {
    /// `from`(인증된 키)이 주소 묶음 `ip`에서 보낸 봉인 편지를 저장하거나 안 되는 까닭을 말함. `seen_days`는 설치 저장소에서 옴
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
        let letter = StoredLetter { id: self.data.next_id, at: now, from: from.to_string(), read: false, done: false, rejected: false, private: false, r#box: proto::to_hex(sealed) };
        self.data.letters.push(letter.clone());
        self.dirty = true;
        Ok(letter)
    }

    /// 모든 편지를 최신 먼저 돌려줌
    pub fn list(&self) -> Vec<StoredLetter> {
        let mut v = self.data.letters.clone();
        v.sort_by(|a, b| b.at.cmp(&a.at).then(b.id.cmp(&a.id)));
        v
    }

    /// 안 읽은 편지 수
    pub fn unread(&self) -> usize {
        self.data.letters.iter().filter(|l| !l.read).count()
    }

    /// 읽음으로 표시함. 그런 편지가 없으면 false임
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

    /// 편지 표시(없음, ✅ 반영, 🙅 반려 중 하나)를 정함. 그런 편지가 없으면 false임
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

    /// 반영 체크를 하거나 풂 (`letter_done`, 옛 앱도 씀): 풀면 반려도 지움. 그런 편지가 없으면 false임
    pub fn mark_done(&mut self, id: u64, done: bool) -> bool {
        self.set_status(id, if done { Status::Done } else { Status::None })
    }

    /// 🙅 편지를 반려하거나 표시를 지움
    pub fn mark_rejected(&mut self, id: u64, rejected: bool) -> bool {
        self.set_status(id, if rejected { Status::Rejected } else { Status::None })
    }

    /// 🔒 편지를 공개 건의함에서 숨기거나 다시 보임. 요약은 남겨 두어 개발자 앱이 다시 올리지 않게 함
    pub fn mark_private(&mut self, id: u64, private: bool) -> bool {
        match self.data.letters.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                if l.private != private {
                    l.private = private;
                    self.dirty = true;
                }
                true
            }
            None => false,
        }
    }

    /// 편지가 사라진 건의함 항목도 지움
    fn prune_board(&mut self) {
        let ids: BTreeSet<u64> = self.data.letters.iter().map(|l| l.id).collect();
        let before = self.data.board.len();
        self.data.board.retain(|id, _| ids.contains(id));
        self.dirty |= self.data.board.len() != before;
    }

    /// 편지를 지움 (요약도 같이). 없었으면 false임
    pub fn delete(&mut self, id: u64) -> bool {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.id != id);
        let gone = self.data.letters.len() != before;
        self.dirty |= gone;
        self.prune_board();
        gone
    }

    /// 이제부터 `key`를 거절하고 그 편지를 지움 (지운 수를 돌려줌)
    pub fn block(&mut self, key: &str) -> usize {
        let before = self.data.letters.len();
        self.data.letters.retain(|l| l.from != key);
        self.data.blocked.insert(key.to_string());
        self.dirty = true;
        self.prune_board();
        before - self.data.letters.len()
    }

    /// 📮 편지 `id`의 공개 요약을 저장함 (다시 검사: 진짜 가림, 짧은 한 줄). 바뀐 게 있으면 `Ok(true)`
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

    /// 건의함에 있는 편지 id들 (나머지는 개발자 앱이 올림)
    pub fn board_ids(&self) -> Vec<u64> {
        self.data.board.keys().copied().collect()
    }

    /// 📮 건의함 `want`쪽(1부터)을 최신 편지 먼저, 🔒 비공개 편지 없이(항목·전체·쪽 수 모두에서) 줌: (항목, 쪽, 전체 쪽, 전체)
    pub fn board_page(&self, want: u32) -> (Vec<Item>, u32, u32, u32) {
        let all: Vec<Item> = self
            .list()
            .into_iter()
            .filter(|l| !l.private)
            .filter_map(|l| self.data.board.get(&l.id).map(|e| Item { id: l.id, name: e.name.clone(), line: e.line.clone(), at: l.at, status: l.status() }))
            .collect();
        let (start, end, page, pages) = board::page_range(all.len(), want);
        (all[start..end].to_vec(), page, pages, all.len() as u32)
    }

    /// `key`가 막혔는지 봄 (시험용)
    #[cfg(test)]
    pub fn is_blocked(&self, key: &str) -> bool {
        self.data.blocked.contains(key)
    }

    /// 지난 스냅숏 뒤 바뀐 게 있으면 파일 바이트를 돌려줌
    pub fn snapshot(&mut self) -> Option<Vec<u8>> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        serde_json::to_vec(&self.data).ok()
    }

    /// 쓰기에 실패했을 때 다시 저장하게 표시함
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// `letters.json`, 없으면 `.bak`, 없으면 빈 편지함을 읽음 (편지를 잃을 수는 있어도 릴레이는 시작함)
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

    /// `relay.json`처럼 씀: 비공개 임시 파일, 옛 파일은 `.bak`으로 두고 이름 바꾸기
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

/// 보내기 한도·표시·건의함·저장 시험임
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
        // 옛 파일(done 칸 없음)도 읽힘
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

    #[test]
    fn private_letters_are_off_the_board_and_stay_off() {
        let dir = tmp_dir("private");
        let mut m = mb();
        let mut ids = Vec::new();
        for i in 0..6u64 {
            let id = m.submit(&format!("k{i}"), &format!("10.0.0.{i}"), 2, b"x", T + i).unwrap().id;
            m.board_put(id, "*", &format!("줄 {i}")).unwrap();
            ids.push(id);
        }
        assert_eq!(m.board_page(1).2, 2);
        assert!(m.mark_private(ids[5], true) && !m.mark_private(999, true));
        let (items, _, pages, total) = m.board_page(1);
        assert_eq!((pages, total), (1, 5), "not in total or pages");
        assert!(items.iter().all(|x| x.id != ids[5]));
        assert_eq!(m.board_put(ids[5], "*", "다시 올림"), Ok(true), "taken, but…");
        assert!(m.board_page(1).0.iter().all(|x| x.id != ids[5]), "…a new digest doesn't make it public");
        assert!(m.board_ids().contains(&ids[5]), "the app knows it's published and won't send it again");
        assert!(m.list()[0].private, "readers see the flag");
        Mailbox::save(&dir, &m.snapshot().unwrap()).unwrap();
        let mut back = Mailbox::load(&dir);
        assert_eq!(back.board_page(1).3, 5, "survives a restart");
        assert!(back.mark_private(ids[5], false));
        assert_eq!(back.board_page(1).3, 6, "public again");
        let old: StoredLetter = serde_json::from_str(r#"{"id":1,"at":5,"from":"a","read":true,"box":"00"}"#).unwrap();
        assert!(!old.private, "old letters are public");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
