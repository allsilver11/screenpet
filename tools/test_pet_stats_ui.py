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
    assert 'info.append(&MenuItem::with_id(app, STATS_VIEW_ID, "📊 펫 통계", true, None::<&str>)?)?;' in TRAY
    assert TRAY.index("if dev {\n        info.append(&MenuItem::with_id(app, STATS_VIEW_ID") > 0
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


# 사용자 정책 (2026-11-27): "누구인지는 모르지만 각 숫자로 서로 다른 사람들끼리는 구분된다"
ANON = "앱이 만든 무작위 익명 번호도 같이 보내요. 이름·친구 코드·컴퓨터 id가 아니라서 누구인지는 알 수 없지만, 서로 다른 사람을 따로 셀 수 있어요."
ANON_OFF = "통계 보내기를 끄면 이 번호도 지워요."


def test_the_ask_window_and_help_state_the_anon_number_policy():
    for page in (ASK_HTML, HELP):
        assert ANON in page
        assert ANON_OFF in page
    assert "설치 id" not in HELP, "an install number is sent now: don't say none is"
    assert "서로 다른 번호" in HTML or "익명 번호" in HTML


# ---- 📊 통계 창이 그리는 숫자 (tools/stats_harness.js — 가짜 DOM에서 ui/stats.js를 그대로 돌림) ----

import subprocess

HARNESS = ROOT / "tools" / "stats_harness.js"


def _draw(days, period="30"):
    out = subprocess.run(["node", str(HARNESS), json.dumps({"ok": True, "days": days}), period], capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def _texts(node):
    """요소 나무의 글자를 차례로 모두 (빈 것은 뺌)"""
    _, text, kids = node
    out = [text] if text else []
    for k in kids:
        out += _texts(k)
    return out


def _tables(node):
    """요소 나무의 표들 — 표마다 줄들, 줄마다 칸 글자"""
    tag, _, kids = node
    if tag == "table":
        return [[[c[1] for c in row[2]] for row in kids]]
    return [t for k in kids for t in _tables(k)]


# 사흘: 앱을 켠 사람 4·4·2명, 무쇠인간은 날마다 1명(아마 한 사람), 고양이는 3·3·1명
DAYS = [
    {"day": "2026-11-24", "reports": 4, "kept": {"musoe": 1, "cat": 3}, "chosen": {"cat": 1}, "changed": [], "versions": {}},
    {"day": "2026-11-25", "reports": 4, "kept": {"musoe": 1, "cat": 3}, "chosen": {}, "changed": [{"from": "cat", "to": "musoe", "n": 1}], "versions": {}},
    {"day": "2026-11-26", "reports": 2, "kept": {"musoe": 1, "cat": 1}, "chosen": {}, "changed": [], "versions": {}},
]


def test_totals_show_how_many_keep_each_kind_on_an_average_day_and_what_share_that_is():
    # 사용자: "무쇠인간 키운날·사람이 7이면 한 명이 7일 키웠다고? 누굴 기준으로?" — 날×사람 합은 뜻을 알 수 없었음
    totals = _tables(_draw(DAYS)["totals"])[0]
    assert totals[0] == ["종류", "키운 사람 (서로 다른 번호)", "하루 평균 키운 사람", "키우는 비율", "새로 데려옴", "바꿔 옴", "바꿔 감"]
    rows = {r[0]: r[1:] for r in totals[1:]}
    # 고양이: (3+3+1)/3일 = 2.3명, 7/(4+4+2) = 70%. 무쇠인간: 하루 1명, 3/10 = 30%. 번호가 없는 옛 날들이라 서로 다른 사람은 "-"
    assert rows["🐱 고양이"] == ["-", "2.3명", "70%", "1", "0", "1"]
    assert rows["🛡️ 무쇠인간"] == ["-", "1명", "30%", "0", "1", "0"]
    # 많이 키우는 것부터
    assert [r[0] for r in totals[1:]] == ["🐱 고양이", "🛡️ 무쇠인간"]


def test_the_heading_says_how_many_days_came_in_and_how_many_people_open_the_app_a_day():
    texts = _texts(_draw(DAYS)["totals"])
    assert any("2026-11-24 ~ 2026-11-26" in t and "3일" in t and "하루 평균 3.3명" in t for t in texts), texts


def test_every_column_is_explained_in_plain_words_under_the_table():
    texts = " ".join(_texts(_draw(DAYS)["totals"]))
    for words in ("하루 평균 키운 사람", "키우는 비율", "새로 데려옴", "바꿔 옴", "바꿔 감"):
        assert f"{words}:" in texts, words
    assert "키운 사람 (서로 다른 번호):" in texts
    # 날마다 센 값은 같은 사람이 여러 날 세질 수 있다고 솔직히
    assert "같은 사람이 여러 날 세질 수 있어요" in texts


def test_the_bar_chart_uses_the_same_daily_average():
    chart = _texts(_draw(DAYS)["chart"])
    assert "하루 평균 키운 사람" in chart[0]
    assert "2.3명" in chart and "1명" in chart


def test_each_day_lists_people_who_kept_each_kind_that_day():
    daily = _draw(DAYS)["daily"]
    tables = _tables(daily)
    # 최신 날 먼저
    assert tables[0][0] == ["종류", "키운 사람", "새로 데려옴", "바꿔 옴"]
    assert {r[0]: r[1] for r in tables[0][1:]} == {"🐱 고양이": "1명", "🛡️ 무쇠인간": "1명"}
    assert any("2026-11-26" in t and "앱을 켠 사람 2명" in t for t in _texts(daily))


def test_the_period_picks_the_latest_days_only():
    totals = _tables(_draw(DAYS, "7")["totals"])[0]
    assert len(totals) == 3
    one = _tables(_draw(DAYS[-1:], "7")["totals"])[0]
    assert {r[0]: r[2] for r in one[1:]} == {"🐱 고양이": "1명", "🛡️ 무쇠인간": "1명"}


# 익명 번호가 온 사흘: A·B·C 세 사람. 무쇠인간은 날마다 A 한 사람, 고양이는 A·B, B·C, C (서로 다른 사람 3명)
A, B, C, D = "000000000000000a", "000000000000000b", "000000000000000c", "000000000000000d"
ID_DAYS = [
    {"day": "2026-11-24", "reports": 2, "kept": {"musoe": 1, "cat": 2}, "people": [A, B], "keepers": {"musoe": [0], "cat": [0, 1]}},
    {"day": "2026-11-25", "reports": 3, "kept": {"musoe": 1, "cat": 2}, "people": [C, A, B], "keepers": {"musoe": [1], "cat": [0, 2]}},
    {"day": "2026-11-26", "reports": 2, "kept": {"musoe": 1, "cat": 1}, "people": [A, C], "keepers": {"musoe": [0], "cat": [1]}},
]


def test_distinct_people_are_counted_by_anon_number_over_the_period():
    drawn = _draw(ID_DAYS)
    totals = _tables(drawn["totals"])[0]
    rows = {r[0]: r[1] for r in totals[1:]}
    assert rows == {"🐱 고양이": "3명", "🛡️ 무쇠인간": "1명"}
    texts = _texts(drawn["totals"])
    assert any("서로 다른 사람 3명이 앱을 켬" in t for t in texts), texts
    assert not any("예전 보고" in t for t in texts), "every report had a number"
    # 기간을 줄이면 그 날들만
    one = {r[0]: r[1] for r in _tables(_draw(ID_DAYS, "1")["totals"])[0][1:]}
    assert one == {"🐱 고양이": "1명", "🛡️ 무쇠인간": "1명"}


def test_days_with_reports_lacking_a_number_get_a_note():
    # 앞의 옛 날(번호 없음) + 번호 일부만 있는 날
    mixed = DAYS[:1] + [{**ID_DAYS[0], "reports": 3, "people": [A, B], "keepers": {"cat": [0, 1]}}]
    drawn = _draw(mixed)
    texts = _texts(drawn["totals"])
    assert "익명 번호가 없는 예전 보고는 사람 수에 안 들어가요" in " ".join(texts)
    assert any("서로 다른 사람 2명" in t for t in texts), texts
    rows = {r[0]: r[1] for r in _tables(drawn["totals"])[0][1:]}
    assert rows["🐱 고양이"] == "2명" and rows["🛡️ 무쇠인간"] == "0명"


def test_odd_keeper_slots_are_ignored():
    odd = [{"day": "2026-11-24", "reports": 1, "kept": {"cat": 1}, "people": [D], "keepers": {"cat": [0, 5, -1, "x"]}}]
    rows = {r[0]: r[1] for r in _tables(_draw(odd)["totals"])[0][1:]}
    assert rows == {"🐱 고양이": "1명"}
