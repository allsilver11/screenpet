//! 🔄 다시 읽는 웹뷰(새로고침·💤 깨어남 — game_loop.rs refresh_windows)가 준비될 때까지 한 번뿐인 알림을 모아 둠.
//! 말풍선(pet-say·pet-say-text)·음식 고르기·💌 답장·🏠 손 흔들기는 보낸 순간에 듣는 페이지가 없으면 사라짐 —
//! 다시 읽기 시작할 때 `begin`, 페이지가 듣기를 마치고 `page_ready`를 부르면 모아 둔 것을 차례로 보냄.
//! 준비 알림이 READY_WAIT 안에 오지 않으면 그냥 보냄 (페이지가 옛 판이어도 잃는 것은 예전과 같음).
//! 페이지가 준비되면 게임 루프도 그 창의 지금 모습을 다시 보냄 (Input::PageReady).
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::game_loop::Input;
use crate::AppState;

/// 준비 알림을 기다리는 가장 긴 시간
const READY_WAIT: Duration = Duration::from_secs(10);
/// 창마다 모아 두는 알림 수 상한 (넘치면 오래된 것부터 버림)
const MAX_QUEUED: usize = 32;

/// 다시 읽는 중인 창 → (시작한 때, 모아 둔 (이벤트, 내용))
type Waiting = HashMap<String, (Instant, Vec<(&'static str, serde_json::Value)>)>;

/// 다시 읽는 중인 창들 (게임 루프·IPC 스레드가 같이 씀 — 잠금 안에서는 보내지 않음)
static WAITING: Mutex<Option<Waiting>> = Mutex::new(None);

/// 이 창을 다시 읽기 시작함: 페이지가 준비될 때까지 send로 보내는 것을 모아 둠
pub fn begin(label: &str) {
    WAITING.lock().unwrap().get_or_insert_with(HashMap::new).insert(label.to_string(), (Instant::now(), Vec::new()));
}

/// 한 번뿐인 알림을 보냄: 그 창이 다시 읽는 중이면 모아 두었다가 준비되면 보냄
pub fn send<S: Serialize + Clone>(app: &AppHandle, label: &str, event: &'static str, payload: S) {
    {
        let mut waiting = WAITING.lock().unwrap();
        if let Some((_, queue)) = waiting.as_mut().and_then(|w| w.get_mut(label)) {
            if let Ok(value) = serde_json::to_value(&payload) {
                if queue.len() >= MAX_QUEUED {
                    queue.remove(0);
                }
                queue.push((event, value));
            }
            return;
        }
    }
    let _ = app.emit_to(label, event, payload);
}

/// 그 창의 모아 둔 알림을 꺼내 보냄 (잠금은 꺼낼 때만)
fn flush(app: &AppHandle, label: &str) {
    let queued = WAITING.lock().unwrap().as_mut().and_then(|w| w.remove(label));
    for (event, value) in queued.map(|(_, q)| q).unwrap_or_default() {
        let _ = app.emit_to(label, event, value);
    }
}

/// 매 틱: 준비 알림이 너무 오래 안 온 창은 모아 둔 것을 그냥 보냄
pub fn tick(app: &AppHandle, now: Instant) {
    let late: Vec<String> = match WAITING.lock().unwrap().as_ref() {
        Some(w) => w.iter().filter(|(_, (at, _))| now.duration_since(*at) >= READY_WAIT).map(|(l, _)| l.clone()).collect(),
        None => return,
    };
    for label in late {
        flush(app, &label);
    }
}

/// 페이지가 듣기를 마쳤음 (펫·손님·하우스·물건·탈것·날씨·구름·썰매 창이 처음 뜰 때와 다시 읽힐 때마다):
/// 모아 둔 알림을 보내고, 게임 루프가 그 창의 지금 자리·모습·장면을 다시 보냄
#[tauri::command]
pub fn page_ready(window: WebviewWindow) {
    let label = window.label().to_string();
    if !crate::overlay::is_floating(&label) && label != crate::sleigh::LABEL {
        return;
    }
    let app = window.app_handle();
    flush(app, &label);
    app.state::<AppState>().send(Input::PageReady(label));
}
