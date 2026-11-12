//! 📮 개발자 건의함 (spec 2026-11-10-feedback-mailbox §5, 예전 이름 📬 편지함): config.json에 개발자 비밀키(dev_key)가 있을 때만.
//! 릴레이가 이 설치를 "편지함 읽는 설치"로 알면 봉한 편지 목록을 주고, 여기서 비밀키로 엶.
//! 연 편지마다 공개 요약(이름 첫 글자 + *, 첫 줄 — screenpet_net::board::digest)을 만들어 릴레이 📮 건의함(v3)에 올림:
//! 목록을 받을 때마다 아직 안 올라간 것만. 본문은 이 컴퓨터를 떠나지 않음 (누구나 보는 목록은 board.rs).
//! 트레이 "📮 건의함"은 개발자 컴퓨터에서 이 창을, 다른 컴퓨터에서는 공개 목록을 엶.
//! 새 편지가 오면 (그리고 앱을 켤 때 안 읽은 편지 몇 통은) 보낸 사람 펫이 💌을 들고 걸어 들어옴 —
//! 네트워크와 상관없는 로컬 손님 (visit_id "letter-<id>", guest_left는 숫자 visit_id만 친구에게 알림).

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use screenpet_core::care::Stage;
use screenpet_core::style::PetStyle;
use screenpet_core::tint::BodyTint;
use screenpet_core::visit::{self, Side, MAX_GUESTS};
use screenpet_net::board::{self, Digest};
use screenpet_net::letter::{self, MailItem};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::slots::{self, GuestInfo, Role, Slot};
use crate::AppState;

/// 개발자 건의함 창 라벨
pub const MAILBOX_LABEL: &str = "mailbox";
/// 개발자 건의함 창 모양
const MAILBOX_FORM: Form = Form { label: MAILBOX_LABEL, page: "mailbox.html", title: "ScreenPet 건의함", size: (480.0, 560.0) };
/// 편지를 든 펫이 머무는 시간 (초) — 지나거나 그 편지를 읽으면 손 흔들고 감
pub const LETTER_VISIT_SECS: u64 = 60;
/// 편지를 든 펫이 차례로 들어오는 간격 (초)
pub const LETTER_GAP_SECS: u64 = 30;
/// 앱을 켤 때 들어오는 안 읽은 편지 펫 수 (나머지는 건의함에만)
pub const LETTERS_ON_START: usize = 3;
/// 손님 머리 위 💌에 보이는 앞부분 (편지 전문은 건의함 창에서)
const NOTE_CHARS: usize = 80;

/// 건의함 창이 보는 편지 한 통 (열지 못한 편지는 ok: false)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LetterView {
    /// 편지 id
    pub id: u64,
    /// 받은 때 (유닉스 초)
    pub at: u64,
    /// 보낸 설치 키 (hex)
    pub from: String,
    /// 읽음
    pub read: bool,
    /// 반영함 (개발자가 체크 — 나만 보는 용도, 릴레이에 저장돼 두 컴퓨터가 같음)
    pub done: bool,
    /// 🙅 반려함 (done과 함께는 아님)
    pub rejected: bool,
    /// 🔒 비공개: 공개 건의함에 안 보임 (개발자에게는 그대로)
    pub private: bool,
    /// 열렸음 (아니면 아래 글은 비어 있음)
    pub ok: bool,
    /// 본문
    pub text: String,
    /// 보낸이 이름
    pub name: String,
    /// 편지를 나르는 펫
    pub species: String,
    /// 펫 성장 단계
    pub stage: String,
    /// 펫 스타일
    pub style: String,
    /// 앱 버전
    pub version: String,
    /// OS
    pub os: String,
}

/// 창이 처음 열릴 때 읽는 편지 목록
static VIEW: Mutex<Vec<LetterView>> = Mutex::new(Vec::new());

/// 📮 아직 건의함에 없는 편지의 공개 요약 (연 편지만, 오래된 것부터). 🔒 비공개 편지는 요약도 보내지 않음
/// (릴레이도 비공개 편지는 요약이 있어도 목록에 넣지 않음)
pub fn unpublished(letters: &[LetterView], published: &HashSet<u64>) -> Vec<Digest> {
    letters
        .iter()
        .rev()
        .filter(|l| l.ok && !l.private && !published.contains(&l.id))
        .filter_map(|l| board::digest(l.id, &letter::Letter { text: l.text.clone(), name: l.name.clone(), ..Default::default() }))
        .collect()
}

/// 봉인 편지를 비밀키로 열어 창에 보일 편지로 만듦 (못 열면 ok: false)
pub fn view_of(item: &MailItem, secret: [u8; 32]) -> LetterView {
    let opened = letter::open(&item.sealed, &secret);
    let l = opened.clone().unwrap_or_default();
    LetterView {
        id: item.id,
        at: item.at,
        from: item.from.clone(),
        read: item.read,
        done: item.done,
        rejected: item.rejected && !item.done,
        private: item.private,
        ok: opened.is_some(),
        text: if opened.is_some() { l.text } else { "(열 수 없는 편지)".into() },
        name: l.name,
        species: l.species,
        stage: l.stage,
        style: l.style,
        version: l.version,
        os: l.os,
    }
}

/// 개발자 건의함 상태임 (게임 루프가 가짐): 편지, 편지 펫 손님, 올린 요약
#[derive(Default)]
pub struct Mailbox {
    secret: Option<[u8; 32]>,
    letters: Vec<LetterView>,
    /// 첫 목록을 받았음 (그때 안 읽은 편지 몇 통을 손님으로 들임)
    listed: bool,
    /// 들어올 편지 펫 (편지 id)
    queue: VecDeque<u64>,
    next_visit: Option<Instant>,
    /// 와 있는 편지 펫: (편지 id, 창 라벨, 떠날 때)
    visits: Vec<(u64, String, Instant)>,
    /// 📮 건의함에 올라간 (또는 방금 올린) 편지 id
    published: HashSet<u64>,
}

impl Mailbox {
    /// 비밀키(있으면)로 만듦
    pub fn new(secret: Option<[u8; 32]>) -> Mailbox {
        Mailbox { secret, ..Mailbox::default() }
    }

    /// 개발자 컴퓨터임 (비밀키가 있음)
    pub fn enabled(&self) -> bool {
        self.secret.is_some()
    }

    /// 안 읽은 편지 수
    pub fn unread(&self) -> usize {
        self.letters.iter().filter(|l| !l.read).count()
    }

    /// 릴레이가 준 전체 목록 (최신순)과 이미 건의함에 있는 편지 id. 돌려주는 것은 건의함에 올릴 요약
    /// (보내고 나면 `published`로 알림)
    pub fn on_list(&mut self, app: &AppHandle, items: Vec<MailItem>, on_board: Vec<u64>) -> Vec<Digest> {
        let Some(secret) = self.secret else { return Vec::new() };
        self.letters = items.iter().map(|i| view_of(i, secret)).collect();
        self.published = on_board.into_iter().collect();
        if !self.listed {
            self.listed = true;
            let unread: Vec<u64> = self.letters.iter().filter(|l| !l.read && l.ok).take(LETTERS_ON_START).map(|l| l.id).collect();
            self.queue.extend(unread.into_iter().rev()); // 오래된 것부터
        }
        self.publish(app);
        unpublished(&self.letters, &self.published)
    }

    /// 📮 이 요약들을 릴레이에 보냈음
    pub fn published(&mut self, digests: &[Digest]) {
        self.published.extend(digests.iter().map(|d| d.id));
    }

    /// 켜져 있는 동안 새 편지 — 돌려주는 것은 건의함에 올릴 요약
    pub fn on_arrived(&mut self, app: &AppHandle, item: MailItem) -> Vec<Digest> {
        let Some(secret) = self.secret else { return Vec::new() };
        if self.letters.iter().any(|l| l.id == item.id) {
            return Vec::new();
        }
        let v = view_of(&item, secret);
        if v.ok {
            self.queue.push_back(v.id);
        }
        self.letters.insert(0, v);
        self.publish(app);
        unpublished(&self.letters[..1], &self.published)
    }

    /// 읽음 (릴레이에도 알렸음) — 그 편지를 든 펫은 이제 감
    pub fn read_local(&mut self, app: &AppHandle, id: u64) {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            l.read = true;
        }
        for v in self.visits.iter_mut().filter(|v| v.0 == id) {
            v.2 = Instant::now();
        }
        self.publish(app);
    }

    /// ✅ 반영함 (끄면 반려도 같이 지움 — 릴레이와 같음)
    pub fn done_local(&mut self, app: &AppHandle, id: u64, done: bool) {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            set_status(l, done, false);
        }
        self.publish(app);
    }

    /// 🙅 반려함 (켜면 반영함이 꺼짐)
    pub fn reject_local(&mut self, app: &AppHandle, id: u64, rejected: bool) {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            set_status(l, false, rejected);
        }
        self.publish(app);
    }

    /// 🔒 비공개 (끄면 다시 공개 — 아직 안 올린 편지면 돌려주는 요약을 올림)
    pub fn private_local(&mut self, app: &AppHandle, id: u64, private: bool) -> Vec<Digest> {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            l.private = private;
        }
        self.publish(app);
        let one: Vec<LetterView> = self.letters.iter().filter(|l| l.id == id).cloned().collect();
        unpublished(&one, &self.published)
    }

    /// 지운 편지는 릴레이가 건의함에서도 뺌
    pub fn delete_local(&mut self, app: &AppHandle, id: u64) {
        self.letters.retain(|l| l.id != id);
        self.queue.retain(|q| *q != id);
        self.published.remove(&id);
        self.publish(app);
    }

    /// 보낸이를 막은 뒤 그 편지를 여기서도 지움
    pub fn block_local(&mut self, app: &AppHandle, from: &str) {
        let gone: Vec<u64> = self.letters.iter().filter(|l| l.from == from).map(|l| l.id).collect();
        self.letters.retain(|l| l.from != from);
        self.queue.retain(|q| !gone.contains(q));
        self.published.retain(|p| !gone.contains(p));
        self.publish(app);
    }

    /// 창·트레이에 알림 (바뀐 게 없어도 — 체크칸을 릴레이 상태로 되돌림)
    pub fn publish(&self, app: &AppHandle) {
        *VIEW.lock().unwrap() = self.letters.clone();
        let _ = app.emit_to(MAILBOX_LABEL, "mailbox-state", &self.letters);
        let n = self.unread();
        if let Some(item) = &app.state::<AppState>().menu.mailbox {
            let _ = item.set_text(crate::tray::board_label(n));
        }
    }

    /// 매 틱: 머문 편지 펫은 보내고, 차례가 된 편지 펫을 들임
    pub fn tick(&mut self, app: &AppHandle, slots: &mut Vec<Slot>, now: Instant) {
        if !self.enabled() {
            return;
        }
        self.visits.retain(|(_, label, until)| {
            let Some(i) = slots::find(slots, label) else { return false };
            if now >= *until {
                slots[i].pet.go_home_now();
                return false;
            }
            true
        });
        if self.queue.is_empty() || self.next_visit.is_some_and(|t| now < t) {
            return;
        }
        // 집중·하우스 중엔 손님이 오지 않음 — 건의함 숫자만 오르고, 나중에 들어옴
        let guests = slots.iter().filter(|s| matches!(s.role, Role::Guest(_))).count();
        if slots[0].pet.refuses_arrivals() || guests >= MAX_GUESTS {
            return;
        }
        let Some(id) = self.queue.pop_front() else { return };
        let Some(l) = self.letters.iter().find(|l| l.id == id && !l.read).cloned() else { return };
        self.next_visit = Some(now + Duration::from_secs(LETTER_GAP_SECS));
        if let Some(label) = spawn_letter_pet(app, slots, &l) {
            self.visits.push((id, label, now + Duration::from_secs(LETTER_VISIT_SECS)));
        }
    }
}

/// 반영·반려는 하나만 (둘 다 끄면 없음)
fn set_status(l: &mut LetterView, done: bool, rejected: bool) {
    l.done = done;
    l.rejected = rejected && !done;
}

/// 보낸 사람 펫을 로컬 손님으로 들임 (모르는 동물은 고양이)
fn spawn_letter_pet(app: &AppHandle, slots: &mut Vec<Slot>, l: &LetterView) -> Option<String> {
    let state = app.state::<AppState>();
    let entry = state.pets.iter().find(|p| p.id == l.species).or_else(|| state.pets.iter().find(|p| p.id == "cat")).or(state.pets.first())?.clone();
    let stage: Stage = serde_json::from_value(serde_json::Value::String(l.stage.clone())).unwrap_or(Stage::Teenager);
    let scale = stage.scale() * entry.manifest.size_mult(stage);
    let name = if l.name.is_empty() { "누군가".to_string() } else { l.name.clone() };
    let note: String = l.text.chars().take(NOTE_CHARS).collect();
    let guest = GuestInfo { visit_id: format!("letter-{}", l.id), owner_id: "letter".into(), owner_name: name, note: visit::sanitize_note(&note), can_grow: false, tint: BodyTint::Default };
    let side = if fastrand::bool() { Side::Left } else { Side::Right };
    let own = &slots[0];
    match slots::spawn_guest(app, guest, entry, stage, scale, PetStyle::from_wire(Some(l.style.as_str())), own.dpi, side, &own.world, fastrand::u64(..), own.size()) {
        Ok(slot) => {
            let label = slot.label.clone();
            slots.push(slot);
            Some(label)
        }
        Err(e) => {
            eprintln!("letter pet {}: {e}", l.id);
            None
        }
    }
}

/// 트레이 메뉴: 창을 엶
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &MAILBOX_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(MAILBOX_LABEL, "mailbox-state", VIEW.lock().unwrap().clone());
    });
}

/// 이 창이 개발자 건의함 창인지 봄
fn mine(window: &WebviewWindow) -> bool {
    window.label() == MAILBOX_LABEL
}

/// 창의 지금 편지 목록 (첫 로드용)
#[tauri::command]
pub fn mailbox_state(window: WebviewWindow) -> Result<Vec<LetterView>, String> {
    if !mine(&window) {
        return Err("wrong window".into());
    }
    Ok(VIEW.lock().unwrap().clone())
}

/// 읽음 표시 (NetInput::MailboxRead)
#[tauri::command]
pub fn mailbox_read(window: WebviewWindow, state: tauri::State<AppState>, id: u64) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxRead(id)));
    }
}

/// 반영함 체크 (NetInput::MailboxDone(id, done))
#[tauri::command]
pub fn mailbox_done(window: WebviewWindow, state: tauri::State<AppState>, id: u64, done: bool) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxDone(id, done)));
    }
}

/// 🙅 반려 (NetInput::MailboxReject(id, rejected)) — v3 릴레이만
#[tauri::command]
pub fn mailbox_reject(window: WebviewWindow, state: tauri::State<AppState>, id: u64, rejected: bool) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxReject(id, rejected)));
    }
}

/// 🔒 비공개 (NetInput::MailboxPrivate(id, private)) — v3 릴레이만
#[tauri::command]
pub fn mailbox_private(window: WebviewWindow, state: tauri::State<AppState>, id: u64, private: bool) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxPrivate(id, private)));
    }
}

/// 편지 지우기 (NetInput::MailboxDelete)
#[tauri::command]
pub fn mailbox_delete(window: WebviewWindow, state: tauri::State<AppState>, id: u64) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxDelete(id)));
    }
}

/// 보낸이 막기 (hex 64자 키만, NetInput::MailboxBlock)
#[tauri::command]
pub fn mailbox_block(window: WebviewWindow, state: tauri::State<AppState>, from: String) {
    if mine(&window) && from.len() == 64 && from.chars().all(|c| c.is_ascii_hexdigit()) {
        state.send(Input::Net(NetInput::MailboxBlock(from)));
    }
}

/// 창의 "닫기" (숨기기만 함)
#[tauri::command]
pub fn mailbox_close(window: WebviewWindow) {
    if mine(&window) {
        let _ = window.hide();
    }
}

/// 편지 열기·요약 올리기·표시 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_open_with_the_secret_and_others_show_as_unreadable() {
        let (secret_hex, public_hex) = letter::new_dev_keypair();
        let secret = screenpet_net::letter::secret_from_hex(&secret_hex).unwrap();
        let public: [u8; 32] = (0..32).map(|i| u8::from_str_radix(&public_hex[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<_>>().try_into().unwrap();
        let sealed = letter::seal(&letter::Letter { text: "안녕".into(), name: "단니".into(), ..Default::default() }, &public).unwrap();
        let item = MailItem { id: 3, at: 9, from: "ab".repeat(32), read: false, done: false, rejected: true, private: true, sealed };
        let v = view_of(&item, secret);
        assert!(v.ok);
        assert_eq!((v.text.as_str(), v.name.as_str()), ("안녕", "단니"));
        let other = view_of(&item, [7; 32]);
        assert!(!other.ok);
        assert_eq!(other.text, "(열 수 없는 편지)");
        assert!(v.rejected && !v.done && v.private);
    }

    fn view(id: u64, ok: bool, name: &str, text: &str) -> LetterView {
        LetterView { id, at: id, from: String::new(), read: false, done: false, rejected: false, private: false, ok, text: text.into(), name: name.into(), species: "cat".into(), stage: String::new(), style: String::new(), version: "0.6.3".into(), os: "windows".into() }
    }

    #[test]
    fn only_opened_letters_not_yet_on_the_board_are_published_masked_and_cut() {
        // 최신순 목록
        let letters = vec![view(3, true, "김철수", "펭귄 더 주세요\n비밀 이야기"), view(2, false, "", "(열 수 없는 편지)"), view(1, true, "단", "고마워요")];
        let digests = unpublished(&letters, &HashSet::new());
        assert_eq!(digests, vec![Digest { id: 1, name: "*".into(), line: "고마워요".into() }, Digest { id: 3, name: "김**".into(), line: "펭귄 더 주세요".into() }], "oldest first, unopened skipped");
        assert_eq!(unpublished(&letters, &HashSet::from([1, 3])), vec![], "already on the board");
        let mut hidden = letters.clone();
        hidden[0].private = true;
        assert_eq!(unpublished(&hidden, &HashSet::new()).iter().map(|d| d.id).collect::<Vec<_>>(), vec![1], "🔒 private: no digest at all");
        let all = serde_json::to_string(&digests).unwrap();
        assert!(!all.contains("비밀") && !all.contains("철수") && !all.contains("cat"), "{all}");
    }

    #[test]
    fn done_and_rejected_are_one_status() {
        let mut l = view(1, true, "", "x");
        set_status(&mut l, true, false);
        assert!(l.done && !l.rejected);
        set_status(&mut l, false, true);
        assert!(!l.done && l.rejected);
        set_status(&mut l, false, false);
        assert!(!l.done && !l.rejected);
    }
}
