//! 📊 익명 펫 통계 (spec 2026-11-23-pet-stats): 처음 켤 때 한 번 묻는 창(`stats_ask.html`)과
//! 개발자 컴퓨터(dev_key)에만 있는 "📊 펫 통계" 창(`stats.html` — 날짜별 표·기간 합계·막대그래프).
//! 모으기·보고 만들기는 코어 `pet_stats::StatsBook`(설정에 남음, 게임 루프가 씀), 보내기는 net(v4 릴레이에만).
//! 답하기 전에는 아무것도 모으거나 보내지 않음. 보내는 것: 지난날 키운 종류·새로 고른 종류·바꾼 횟수·앱 버전·익명 번호
//! (앱이 무작위로 만든 번호 — 누구인지는 모르지만 서로 다른 사람을 구분해 셈, 끄면 지움).

use screenpet_net::pet_stats::StatsDay;
use serde::Serialize;
use tauri::{AppHandle, Emitter, WebviewWindow};

use crate::game_loop::Input;
use crate::network::NetInput;
use crate::overlay::Form;
use crate::AppState;

/// 처음 묻는 창 라벨
pub const ASK_LABEL: &str = "stats-ask";
/// 처음 묻는 창 모양
const ASK_FORM: Form = Form { label: ASK_LABEL, page: "stats_ask.html", title: "ScreenPet 익명 통계", size: (400.0, 310.0) };
/// 📊 펫 통계 창 라벨 (개발자만)
pub const STATS_LABEL: &str = "stats";
/// 📊 펫 통계 창 모양
const STATS_FORM: Form = Form { label: STATS_LABEL, page: "stats.html", title: "ScreenPet 펫 통계", size: (560.0, 620.0) };

/// 오늘 지역 날짜 (`YYYY-MM-DD`) — 하루치 기록의 날짜
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// 이 앱 버전 (보고에 실림)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 📊 창이 받는 합계 (`stats-days`). 못 받았으면 ok: false와 이유 (offline · old · timeout)
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StatsView {
    /// 받았음
    pub ok: bool,
    /// 못 받은 까닭
    pub reason: String,
    /// 날짜별 합계 (오래된 날 먼저)
    pub days: Vec<StatsDay>,
}

/// 릴레이 대답을 창에 보낼 모양으로 바꿈
pub fn view_of(result: Result<Vec<StatsDay>, String>) -> StatsView {
    match result {
        Ok(days) => StatsView { ok: true, reason: String::new(), days },
        Err(reason) => StatsView { ok: false, reason, days: Vec::new() },
    }
}

/// 처음 켤 때 (아직 답이 없으면) 묻는 창을 엶
pub fn show_ask(app: &AppHandle) {
    crate::overlay::show_form(app, &ASK_FORM, crate::overlay::place_on_primary, |_| {});
}

/// 트레이 "📊 펫 통계" (개발자만): 창을 엶 — 열 때마다 새로 받음 (창이 `stats-open`을 듣고 stats_list)
pub fn show(app: &AppHandle) {
    crate::overlay::show_form(app, &STATS_FORM, crate::overlay::place_on_primary, |_| {
        let _ = app.emit_to(STATS_LABEL, "stats-open", ());
    });
}

/// 릴레이의 대답을 창에 알림
pub fn result(app: &AppHandle, result: Result<Vec<StatsDay>, String>) {
    let _ = app.emit_to(STATS_LABEL, "stats-days", view_of(result));
}

/// 묻는 창의 "보낼게요"(true) / "안 보낼래요"(false) — 설정에 남기고 창을 닫음
#[tauri::command]
pub fn stats_answer(window: WebviewWindow, state: tauri::State<AppState>, yes: bool) {
    if window.label() == ASK_LABEL {
        state.send(Input::StatsConsent(yes));
        let _ = window.hide();
    }
}

/// 📊 창이 합계를 달라고 함 — 대답은 `stats-days`로
#[tauri::command]
pub fn stats_list(window: WebviewWindow, state: tauri::State<AppState>) {
    if window.label() == STATS_LABEL {
        state.send(Input::Net(NetInput::StatsList));
    }
}

/// 📊 창의 "닫기"
#[tauri::command]
pub fn stats_close(window: WebviewWindow) {
    if window.label() == STATS_LABEL {
        let _ = window.hide();
    }
}

/// 릴레이 대답 → 창 모양 변환 시험임
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn totals_or_why_not() {
        let day = StatsDay { day: "2026-11-22".into(), reports: 2, ..Default::default() };
        let v = view_of(Ok(vec![day.clone()]));
        assert_eq!((v.ok, v.days), (true, vec![day]));
        let v = view_of(Err("old".into()));
        assert_eq!((v.ok, v.reason.as_str(), v.days.len()), (false, "old", 0));
        assert_eq!(today().len(), 10);
    }
}
