//! 📮 편지함과 공개 건의함: 편지 받기, 독자의 표시·삭제·차단, 공개 요약, 페이지.
//!
//! 편지함은 자기 잠금(`Hub::mail`)을 씀. 그래서 `letters.json` 스냅숏(직렬화)이 접속·채널 일을 막지 않음.

use super::*;

/// 대소문자 hex를 바이트로 바꿈. 깨졌으면 None임
pub(super) fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

impl Hub {
    /// `key`가 편지함 독자인지 봄
    pub(super) fn is_reader(st: &State, key: &str) -> bool {
        st.control.mailbox_readers.contains(key)
    }

    /// 접속 중인 모든 편지함 독자에게 `m`을 보냄
    pub(super) fn tell_readers(st: &State, m: &ServerMsg) {
        for (k, p) in &st.peers {
            if Self::is_reader(st, k) {
                p.send(m.clone());
            }
        }
    }

    /// 편지함이 바뀌었으면 `letters.json`을 씀. 걸린 밀리초를 돌려줌 (실패면 `save_fail`을 적음)
    ///
    /// 저장 잠금이 스냅숏과 쓰기를 감싸 순서를 지킴. 편지함 잠금은 스냅숏 동안만 잡고, 상태 잠금은 잡지 않음.
    pub(super) fn save_mailbox(&self) -> u128 {
        let started = Instant::now();
        let _order = self.mailbox_save.lock().unwrap_or_else(|e| e.into_inner());
        let snap = self.mail().snapshot();
        if let (Some(bytes), Some(dir)) = (snap, self.data_dir.as_deref()) {
            if let Err(e) = crate::mailbox::Mailbox::save(dir, &bytes) {
                log!(Error, "mail", "save_fail", "took_ms" => took_ms(started), "err" => e);
                self.mail().mark_dirty();
            }
        }
        took_ms(started)
    }

    /// `key`가 보낸 봉인 편지(hex)를 받음. 독자에게 `letter_new`를 알리고 답을 돌려줌.
    /// `letter_in`(번호·크기·저장 took_ms)이나 `letter_refused`를 적음. 내용은 안 적음
    pub(crate) fn letter(&self, key: &str, conn: u64, ip: &str, hex: &str) -> Option<ServerMsg> {
        let mut stored: Option<u64> = None;
        let reply = {
            let st = self.state();
            if !Self::is_current(&st, key, conn) {
                return None;
            }
            let sealed = decode_hex(hex);
            let seen_days = st.store_ref().get(key).map_or(0, |i| i.seen_days);
            let result = match sealed {
                Some(b) => self.mail().submit(key, ip, seen_days, &b, unix_now()),
                None => Err(crate::mailbox::Refuse::Size),
            };
            match result {
                Ok(item) => {
                    stored = Some(item.id);
                    Self::tell_readers(&st, &ServerMsg::LetterNew { item: item.into() });
                    ServerMsg::LetterOk
                }
                Err(r) => {
                    log!(Warn, "mail", "letter_refused", "conn" => Cid(conn), "peer" => short(key), "reason" => r.as_str());
                    ServerMsg::LetterRefused { reason: r.as_str().into() }
                }
            }
        };
        let took = self.save_mailbox();
        let bytes = hex.len() / 2;
        if let Some(id) = stored {
            log!(Info, "mail", "letter_in", "conn" => Cid(conn), "peer" => short(key), "id" => id, "bytes" => bytes, "took_ms" => took);
        }
        Some(reply)
    }

    /// 편지함 전체를 줌 (독자만, 아니면 None임)
    pub(crate) fn letters(&self, key: &str, conn: u64) -> Option<ServerMsg> {
        let st = self.state();
        (Self::is_current(&st, key, conn) && Self::is_reader(&st, key)).then(|| {
            let mail = self.mail();
            let items: Vec<_> = mail.list().into_iter().map(Into::into).collect();
            log!(Info, "mail", "letters_list", "conn" => Cid(conn), "peer" => short(key), "n" => items.len());
            ServerMsg::Letters { items, board: mail.board_ids() }
        })
    }

    /// 독자가 편지함을 고침. 바뀌면 접속 중인 독자 모두가 `letters_changed`를 받음 (`mail_edit op= id=`를 적음, 차단은 id 0)
    pub(super) fn reader_edit(&self, key: &str, conn: u64, op: &'static str, id: u64, edit: impl FnOnce(&mut crate::mailbox::Mailbox) -> bool) {
        {
            let st = self.state();
            if !Self::is_current(&st, key, conn) || !Self::is_reader(&st, key) {
                return;
            }
            if !edit(&mut self.mail()) {
                return;
            }
            Self::tell_readers(&st, &ServerMsg::LettersChanged);
        }
        let took = self.save_mailbox();
        log!(Info, "mail", "mail_edit", "conn" => Cid(conn), "peer" => short(key), "op" => op, "id" => id, "took_ms" => took);
    }

    /// 읽음 표시
    pub(crate) fn letter_read(&self, key: &str, conn: u64, id: u64) {
        self.reader_edit(key, conn, "read", id, |m| m.mark_read(id));
    }

    /// ✅ 반영 표시를 켜거나 끔
    pub(crate) fn letter_done(&self, key: &str, conn: u64, id: u64, done: bool) {
        self.reader_edit(key, conn, "done", id, |m| m.mark_done(id, done));
    }

    /// 🙅 반려를 켜거나 끔 (v3 앱. 옛 앱은 `letter_done`만 씀)
    pub(crate) fn letter_reject(&self, key: &str, conn: u64, id: u64, rejected: bool) {
        self.reader_edit(key, conn, "reject", id, |m| m.mark_rejected(id, rejected));
    }

    /// 🔒 비공개를 켜거나 끔 (v3 앱, 독자만)
    pub(crate) fn letter_private(&self, key: &str, conn: u64, id: u64, private: bool) {
        self.reader_edit(key, conn, "private", id, |m| m.mark_private(id, private));
    }

    /// 📮 독자(개발자 앱)가 올린 공개 요약을 받음. 편지함이 하나씩 검사하고 나쁜 것은 건너뜀. 답은 없음
    pub(crate) fn board_put(&self, key: &str, conn: u64, items: &[proto::BoardIn]) {
        if items.len() > crate::board::PUT_MAX {
            return;
        }
        {
            let st = self.state();
            if !Self::is_current(&st, key, conn) || !Self::is_reader(&st, key) {
                return;
            }
            let mut changed = false;
            let mut mail = self.mail();
            for i in items {
                changed |= mail.board_put(i.id, &i.name, &i.line) == Ok(true);
            }
            // 이름·한 줄은 안 적음 (개수만)
            log!(Info, "board", "board_put", "conn" => Cid(conn), "peer" => short(key), "n" => items.len(), "changed" => u8::from(changed));
            if !changed {
                return;
            }
        }
        self.save_mailbox();
    }

    /// 📮 공개 건의함 한 페이지를 줌. 연결마다 `board_gap`에 한 번까지 (더 빠르면 None임)
    pub(crate) fn board_list(&self, key: &str, conn: u64, page: u32) -> Option<ServerMsg> {
        let mut st = self.state();
        if !Self::is_current(&st, key, conn) {
            return None;
        }
        let now = Instant::now();
        let peer = st.peers.get_mut(key).expect("current");
        if peer.board_at.is_some_and(|t| now.duration_since(t) < self.limits.board_gap) {
            return None;
        }
        peer.board_at = Some(now);
        let (items, page, pages, total) = self.mail().board_page(page);
        log!(Info, "board", "board_list", "conn" => Cid(conn), "peer" => short(key), "page" => page, "n" => items.len());
        Some(ServerMsg::BoardPage { items, page, pages, total })
    }

    /// 편지를 지움
    pub(crate) fn letter_delete(&self, key: &str, conn: u64, id: u64) {
        self.reader_edit(key, conn, "delete", id, |m| m.delete(id));
    }

    /// 보낸 설치를 막음 (그 편지와 요약도 같이 사라짐)
    pub(crate) fn letter_block(&self, key: &str, conn: u64, from: &str) {
        let Some(from) = proto::norm_key(from) else { return };
        self.reader_edit(key, conn, "block", 0, |m| {
            m.block(&from);
            true
        });
    }

    /// `admin mailbox-reader add|remove <key>`를 처리함
    pub(crate) fn admin_mailbox_reader(&self, op: &str, who: &str) -> Result<String, String> {
        let key = match proto::norm_key(who) {
            Some(k) => k,
            None => self.state().resolve(who)?,
        };
        let mut next = self.control();
        let msg = match op {
            "add" => {
                next.mailbox_readers.insert(key.clone());
                format!("mailbox reader added: {key}")
            }
            "remove" => {
                if !next.mailbox_readers.remove(&key) {
                    return Err(format!("not a mailbox reader: {key}"));
                }
                format!("mailbox reader removed: {key}")
            }
            _ => return Err("expected add or remove".into()),
        };
        drop(self.commit_control(next)?);
        Ok(msg)
    }

    /// `admin mailbox-readers`, `admin letters`의 답을 만듦
    pub(crate) fn admin_mailbox(&self) -> String {
        let st = self.state();
        let readers: Vec<&String> = st.control.mailbox_readers.iter().collect();
        let mail = self.mail();
        format!("letters: {} (unread {})\nmailbox readers: {}", mail.list().len(), mail.unread(), if readers.is_empty() { "none".to_string() } else { readers.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ") })
    }
}
