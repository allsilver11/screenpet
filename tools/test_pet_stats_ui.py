"""📊 익명 펫 통계 (spec 2026-11-23-pet-stats) — 앱 쪽 소스 검사: 처음 한 번 묻는 창(ui/stats_ask.*), ⚙️ 설정 체크,
개발자 컴퓨터에만 있는 📊 펫 통계 창(ui/stats.*, src-tauri/src/pet_stats.rs), 고르기·바꾸기를 세는 자리, 도움말."""

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
SRC = ROOT / "src-tauri" / "src"
ASK_HTML = (UI / "stats_ask.html").read_text(encoding="utf-8")
ASK_JS = (UI / "stats_ask.js").read_text(encoding="utf-8")
HTML = (UI / "stats.html").read_text(encoding="utf-8")
JS = (UI / "stats.js").read_text(encoding="utf-8")
RS = (SRC / "pet_stats.rs").read_text(encoding="utf-8")
TRAY = (SRC / "tray.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
NET = (SRC / "network.rs").read_text(encoding="utf-8")
MAIN = (SRC / "main.rs").read_text(encoding="utf-8")
BUILD = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
HELP = (UI / "help.html").read_text(encoding="utf-8")
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))

WHY = "어떤 펫이 인기 있는지 알아보고, 새 펫을 만들 때 참고하려고 모아요. 누가 무엇을 골랐는지는 남지 않아요."


def _cap(window):
    return [c for c in CAPS["capabilities"] if c["windows"] == [window]]


def test_the_ask_window_explains_why_and_offers_both_answers():
    assert "익명 펫 통계를 보낼까요?" in ASK_HTML
    assert "(키우는 펫 종류와 바꾼 횟수만, 하루에 한 번)" in ASK_HTML
    assert WHY in ASK_HTML
    assert ">보낼게요<" in ASK_HTML and ">안 보낼래요<" in ASK_HTML
    assert "📊 익명 통계 보내기" in ASK_HTML, "tells where to change it later"
    assert 'invoke("stats_answer", { yes })' in ASK_JS


def test_it_is_asked_once_when_there_is_no_answer_and_nothing_goes_before():
    assert "if config.pet_stats.consent.is_none() {\n        crate::pet_stats::show_ask(&app);" in LOOP
    # 보낼 보고는 동의했을 때만 (코어 StatsBook::report)
    assert "network.set_stats_report(config.pet_stats.report().cloned());" in LOOP
    assert "Input::StatsConsent(yes) => {" in LOOP
    assert "if window.label() == ASK_LABEL {\n        state.send(Input::StatsConsent(yes));" in RS


def test_settings_has_the_check_and_the_loop_keeps_it_true():
    assert 'pub const STATS_LABEL: &str = "📊 익명 통계 보내기";' in TRAY
    assert "&autostart, &pet_stats])?;" in TRAY, "inside ⚙️ 설정"
    assert "STATS_ID => send(app, Input::ToggleStats)," in TRAY
    assert "menu.pet_stats.set_checked(config.pet_stats.on())" in LOOP
    assert "let yes = !config.pet_stats.on();" in LOOP


def test_a_new_installs_first_pet_waits_in_the_config_for_the_answer():
    assert "if new_id {\n        config.pet_stats.mark_first_pick();\n        save_config(&app, &mut config);" in LOOP
    assert "config.pet_stats.answer(yes, &crate::pet_stats::today()" in LOOP
    assert "fresh_install" not in LOOP


def test_choosing_adding_and_changing_are_counted():
    assert "config.pet_stats.note_chosen(&crate::pet_stats::today(), crate::pet_stats::VERSION, &entry.id);" in LOOP
    assert "if config.set_pet_species(&uid, &next.id) && !remote {" in LOOP
    assert "config.pet_stats.note_changed(" in LOOP
    # 날짜 바뀜은 1분마다 (저장과 같이)
    assert "stats_tick(&mut config, &network);\n            save_config(&app, &mut config);" in LOOP


def test_the_stats_window_is_for_the_developer_only():
    assert 'menu.append(&MenuItem::with_id(app, STATS_VIEW_ID, "📊 펫 통계", true, None::<&str>)?)?;' in TRAY
    assert TRAY.index("if dev {\n        menu.append(&MenuItem::with_id(app, STATS_VIEW_ID") > 0
    assert "STATS_VIEW_ID => send(app, Input::ShowPetStats)," in TRAY
    assert "NetInput::StatsList => match (&self.net, self.mailbox.enabled()) {" in NET
    assert "NetEvent::PetStats { result } => crate::pet_stats::result(app, result)," in NET
    assert "NetEvent::StatsSent { day } => {" in NET


def test_commands_and_permissions_only_for_their_windows():
    for c in ("stats_answer", "stats_list", "stats_close"):
        assert f"pet_stats::{c}," in MAIN and f'"{c}",' in BUILD
    assert _cap("stats-ask")[0]["permissions"] == ["allow-stats-answer"]
    assert _cap("stats")[0]["permissions"] == ["allow-stats-list", "allow-stats-close"]
    events = next(c for c in CAPS["capabilities"] if c["identifier"] == "events")
    assert "stats-ask" in events["windows"] and "stats" in events["windows"]
    assert "if window.label() == STATS_LABEL {" in RS


def test_the_window_draws_totals_a_bar_chart_and_days_with_text_only():
    assert 'invoke("stats_list")' in JS and 'here.listen("stats-days"' in JS
    assert "innerHTML" not in JS and "insertAdjacentHTML" not in JS
    assert "기간 합계" in JS and "막대그래프" in JS and "날짜별" in JS
    assert 'id="days"' in HTML and 'id="chart"' in HTML
    for reason in ('"offline"', '"old"'):
        assert reason in JS


def test_help_says_what_is_sent_why_and_how_to_turn_it_off():
    assert "📊 익명 통계" in HELP
    assert WHY in HELP
    assert "📊 익명 통계 보내기" in HELP
