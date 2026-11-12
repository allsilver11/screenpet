//! 📮 건의함 보기 (누구나): 개발자가 받은 건의의 공개 목록 — 이름 첫 글자(나머지는 *)와 첫 줄, 날짜, ✅ 반영됨 / 🙅 반려됨.
//! 읽기 전용: 읽음·지우기·막기·반영 같은 것은 없다. 편지는 개발자만 열 수 있어서, 개발자 앱이 가린 이름과 첫 줄만
//! 릴레이에 올린다 (mailbox.rs) — 본문은 개발자 컴퓨터를 떠나지 않는다. 한 쪽에 5개, 릴레이(v3)가 쪽을 나눠 준다.
//! 개발자 컴퓨터에서는 트레이 "📮 건의함"이 이 창 대신 개발자 건의함(mailbox.rs)을 연다.

use screenpet_net::board::{BoardItem, BoardPage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::AppState;

pub const BOARD_LABEL: &str = "board";
const BOARD_FORM: Form = Form { label: BOARD_LABEL, page: "board.html", title: "ScreenPet 건의함", size: (480.0, 520.0) };

/// 창이 받는 한 쪽 (`board-page`). 못 받았으면 ok: false와 이유 (offline · old · timeout)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BoardView {
    pub ok: bool,
    pub reason: String,
    pub items: Vec<BoardItem>,
    pub page: u32,
    pub pages: u32,
    pub total: u32,
}

pub fn view_of(result: Result<BoardPage, String>) -> BoardView {
    match result {
        Ok(p) => BoardView { ok: true, reason: String::new(), items: p.items, page: p.page.max(1), pages: p.pages.max(1), total: p.total },
        Err(reason) => BoardView { ok: false, reason, items: Vec::new(), page: 1, pages: 1, total: 0 },
    }
}

/// 트레이 메뉴: 창을 연다 (열 때마다 첫 쪽을 새로 받는다 — 창이 `board-open`을 듣고 board_list(1))
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &BOARD_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(BOARD_LABEL, "board-open", ());
    });
}

/// 릴레이의 대답을 창에 알린다
pub fn result(app: &AppHandle, result: Result<BoardPage, String>) {
    let _ = app.emit_to(BOARD_LABEL, "board-page", view_of(result));
}

fn mine(window: &WebviewWindow) -> bool {
    window.label() == BOARD_LABEL
}

/// 창이 page쪽(1부터)을 달라고 한다 — 대답은 `board-page`로
#[tauri::command]
pub fn board_list(window: WebviewWindow, state: tauri::State<AppState>, page: u32) {
    if mine(&window) {
        state.send(Input::Net(NetInput::BoardList(page.clamp(1, 1000))));
    }
}

#[tauri::command]
pub fn board_close(window: WebviewWindow) {
    if mine(&window) {
        let _ = window.hide();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use screenpet_net::board::Status;

    #[test]
    fn a_page_or_why_not() {
        let item = BoardItem { id: 1, name: "김**".into(), line: "펭귄".into(), at: 5, status: Status::Rejected };
        let v = view_of(Ok(BoardPage { items: vec![item.clone()], page: 2, pages: 3, total: 11 }));
        assert_eq!((v.ok, v.page, v.pages, v.total, v.items), (true, 2, 3, 11, vec![item]));
        let v = view_of(Err("old".into()));
        assert_eq!((v.ok, v.reason.as_str(), v.page, v.pages, v.items.len()), (false, "old", 1, 1, 0));
        let json = serde_json::to_string(&view_of(Ok(BoardPage { items: vec![], page: 0, pages: 0, total: 0 }))).unwrap();
        assert!(json.contains(r#""page":1,"pages":1"#), "{json}");
    }
}
