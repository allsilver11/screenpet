//! 📬 개발자 편지함 (spec 2026-11-10-feedback-mailbox §5): config.json에 개발자 비밀키(dev_key)가 있을 때만.
//! 릴레이가 이 설치를 "편지함 읽는 설치"로 알면 봉한 편지 목록을 주고, 여기서 비밀키로 연다.
//! 새 편지가 오면 (그리고 앱을 켤 때 안 읽은 편지 몇 통은) 보낸 사람 펫이 💌을 들고 걸어 들어온다 —
//! 네트워크와 상관없는 로컬 손님 (visit_id "letter-<id>", guest_left는 숫자 visit_id만 친구에게 알린다).

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use screenpet_core::care::Stage;
use screenpet_core::style::PetStyle;
use screenpet_core::visit::{self, Side, MAX_GUESTS};
use screenpet_net::letter::{self, MailItem};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::slots::{self, GuestInfo, Role, Slot};
use crate::AppState;

pub const MAILBOX_LABEL: &str = "mailbox";
const MAILBOX_FORM: Form = Form { label: MAILBOX_LABEL, page: "mailbox.html", title: "ScreenPet 편지함", size: (480.0, 560.0) };
/// 편지를 든 펫이 머무는 시간 (초) — 지나거나 그 편지를 읽으면 손 흔들고 간다
pub const LETTER_VISIT_SECS: u64 = 60;
/// 편지를 든 펫이 차례로 들어오는 간격 (초)
pub const LETTER_GAP_SECS: u64 = 30;
/// 앱을 켤 때 들어오는 안 읽은 편지 펫 수 (나머지는 편지함에만)
pub const LETTERS_ON_START: usize = 3;
/// 손님 머리 위 💌에 보이는 앞부분 (편지 전문은 편지함 창에서)
const NOTE_CHARS: usize = 80;

/// 편지함 창이 보는 편지 한 통 (열지 못한 편지는 ok: false)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LetterView {
    pub id: u64,
    pub at: u64,
    pub from: String,
    pub read: bool,
    /// 반영함 (개발자가 체크 — 나만 보는 용도, 릴레이에 저장돼 두 컴퓨터가 같다)
    pub done: bool,
    pub ok: bool,
    pub text: String,
    pub name: String,
    pub species: String,
    pub stage: String,
    pub style: String,
    pub version: String,
    pub os: String,
}

static VIEW: Mutex<Vec<LetterView>> = Mutex::new(Vec::new());

pub fn view_of(item: &MailItem, secret: [u8; 32]) -> LetterView {
    let opened = letter::open(&item.sealed, &secret);
    let l = opened.clone().unwrap_or_default();
    LetterView {
        id: item.id,
        at: item.at,
        from: item.from.clone(),
        read: item.read,
        done: item.done,
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

#[derive(Default)]
pub struct Mailbox {
    secret: Option<[u8; 32]>,
    letters: Vec<LetterView>,
    /// 첫 목록을 받았다 (그때 안 읽은 편지 몇 통을 손님으로 들인다)
    listed: bool,
    /// 들어올 편지 펫 (편지 id)
    queue: VecDeque<u64>,
    next_visit: Option<Instant>,
    /// 와 있는 편지 펫: (편지 id, 창 라벨, 떠날 때)
    visits: Vec<(u64, String, Instant)>,
}

impl Mailbox {
    pub fn new(secret: Option<[u8; 32]>) -> Mailbox {
        Mailbox { secret, ..Mailbox::default() }
    }

    /// 개발자 컴퓨터다 (비밀키가 있다)
    pub fn enabled(&self) -> bool {
        self.secret.is_some()
    }

    pub fn unread(&self) -> usize {
        self.letters.iter().filter(|l| !l.read).count()
    }

    /// 릴레이가 준 전체 목록 (최신순)
    pub fn on_list(&mut self, app: &AppHandle, items: Vec<MailItem>) {
        let Some(secret) = self.secret else { return };
        self.letters = items.iter().map(|i| view_of(i, secret)).collect();
        if !self.listed {
            self.listed = true;
            let unread: Vec<u64> = self.letters.iter().filter(|l| !l.read && l.ok).take(LETTERS_ON_START).map(|l| l.id).collect();
            self.queue.extend(unread.into_iter().rev()); // 오래된 것부터
        }
        self.publish(app);
    }

    /// 켜져 있는 동안 새 편지
    pub fn on_arrived(&mut self, app: &AppHandle, item: MailItem) {
        let Some(secret) = self.secret else { return };
        if self.letters.iter().any(|l| l.id == item.id) {
            return;
        }
        let v = view_of(&item, secret);
        if v.ok {
            self.queue.push_back(v.id);
        }
        self.letters.insert(0, v);
        self.publish(app);
    }

    /// 읽음 (릴레이에도 알렸다) — 그 편지를 든 펫은 이제 간다
    pub fn read_local(&mut self, app: &AppHandle, id: u64) {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            l.read = true;
        }
        for v in self.visits.iter_mut().filter(|v| v.0 == id) {
            v.2 = Instant::now();
        }
        self.publish(app);
    }

    pub fn done_local(&mut self, app: &AppHandle, id: u64, done: bool) {
        if let Some(l) = self.letters.iter_mut().find(|l| l.id == id) {
            l.done = done;
        }
        self.publish(app);
    }

    pub fn delete_local(&mut self, app: &AppHandle, id: u64) {
        self.letters.retain(|l| l.id != id);
        self.queue.retain(|q| *q != id);
        self.publish(app);
    }

    pub fn block_local(&mut self, app: &AppHandle, from: &str) {
        let gone: Vec<u64> = self.letters.iter().filter(|l| l.from == from).map(|l| l.id).collect();
        self.letters.retain(|l| l.from != from);
        self.queue.retain(|q| !gone.contains(q));
        self.publish(app);
    }

    /// 창·트레이에 알린다
    fn publish(&self, app: &AppHandle) {
        *VIEW.lock().unwrap() = self.letters.clone();
        let _ = app.emit_to(MAILBOX_LABEL, "mailbox-state", &self.letters);
        let n = self.unread();
        if let Some(item) = &app.state::<AppState>().menu.mailbox {
            let _ = item.set_text(if n == 0 { "📬 편지함".to_string() } else { format!("📬 편지함 ({n})") });
        }
    }

    /// 매 틱: 머문 편지 펫은 보내고, 차례가 된 편지 펫을 들인다
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
        // 집중·하우스 중엔 손님이 오지 않는다 — 편지함 숫자만 오르고, 나중에 들어온다
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

/// 보낸 사람 펫을 로컬 손님으로 들인다 (모르는 동물은 고양이)
fn spawn_letter_pet(app: &AppHandle, slots: &mut Vec<Slot>, l: &LetterView) -> Option<String> {
    let state = app.state::<AppState>();
    let entry = state.pets.iter().find(|p| p.id == l.species).or_else(|| state.pets.iter().find(|p| p.id == "cat")).or(state.pets.first())?.clone();
    let stage: Stage = serde_json::from_value(serde_json::Value::String(l.stage.clone())).unwrap_or(Stage::Teenager);
    let scale = stage.scale() * entry.manifest.size_mult(stage);
    let name = if l.name.is_empty() { "누군가".to_string() } else { l.name.clone() };
    let note: String = l.text.chars().take(NOTE_CHARS).collect();
    let guest = GuestInfo { visit_id: format!("letter-{}", l.id), owner_id: "letter".into(), owner_name: name, note: visit::sanitize_note(&note), can_grow: false };
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

/// 트레이 메뉴: 창을 연다
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &MAILBOX_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(MAILBOX_LABEL, "mailbox-state", VIEW.lock().unwrap().clone());
    });
}

fn mine(window: &WebviewWindow) -> bool {
    window.label() == MAILBOX_LABEL
}

#[tauri::command]
pub fn mailbox_state(window: WebviewWindow) -> Result<Vec<LetterView>, String> {
    if !mine(&window) {
        return Err("wrong window".into());
    }
    Ok(VIEW.lock().unwrap().clone())
}

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

#[tauri::command]
pub fn mailbox_delete(window: WebviewWindow, state: tauri::State<AppState>, id: u64) {
    if mine(&window) {
        state.send(Input::Net(NetInput::MailboxDelete(id)));
    }
}

#[tauri::command]
pub fn mailbox_block(window: WebviewWindow, state: tauri::State<AppState>, from: String) {
    if mine(&window) && from.len() == 64 && from.chars().all(|c| c.is_ascii_hexdigit()) {
        state.send(Input::Net(NetInput::MailboxBlock(from)));
    }
}

#[tauri::command]
pub fn mailbox_close(window: WebviewWindow) {
    if mine(&window) {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_open_with_the_secret_and_others_show_as_unreadable() {
        let (secret_hex, public_hex) = letter::new_dev_keypair();
        let secret = screenpet_net::letter::secret_from_hex(&secret_hex).unwrap();
        let public: [u8; 32] = (0..32).map(|i| u8::from_str_radix(&public_hex[2 * i..2 * i + 2], 16).unwrap()).collect::<Vec<_>>().try_into().unwrap();
        let sealed = letter::seal(&letter::Letter { text: "안녕".into(), name: "단니".into(), ..Default::default() }, &public).unwrap();
        let item = MailItem { id: 3, at: 9, from: "ab".repeat(32), read: false, done: false, sealed };
        let v = view_of(&item, secret);
        assert!(v.ok);
        assert_eq!((v.text.as_str(), v.name.as_str()), ("안녕", "단니"));
        let other = view_of(&item, [7; 32]);
        assert!(!other.ok);
        assert_eq!(other.text, "(열 수 없는 편지)");
    }
}
