"""🖍️ 분필 낙서 — 흐물이가 바닥에 그린 하트·웃는 얼굴·별·이름. 🌈 무지개 구름과 같은 모니터 창 (trail.html)에
두 번째 캔버스 (ui/doodle.js)로 그린다: 마우스 통과, 쓸 때만 창, 6개까지, 가만히 있는 동안은 그림을 멈춘다,
이름은 캔버스 글자로만 (HTML로 넣지 않는다, 12글자)."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "doodle.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")


def test_doodles_share_the_trail_window():
    assert '<script src="doodle.js"></script>' in HTML and '<canvas id="doodle">' in HTML
    # 낙서 캔버스가 구름 아래
    assert HTML.index('id="doodle"') < HTML.index('id="trail"')
    assert 'getCurrentWebviewWindow().listen("trail-doodle"' in JS
    assert '"trail-doodle"' in RS
    # 다른 창 종류를 만들지 않는다
    assert RS.count("screen_cover(") == 1


def test_timing_and_budget():
    nums = {k: float(v) for k, v in re.findall(r"const (DRAW|HOLD|FADE|MAX_DOODLES|MAX_NAME) = ([\d.]+);", JS)}
    # 그리기 시간은 흐물이 그리기 동작과 같다: 앱이 screenpet_core::brain::DOODLE_SECS(2초)를 낙서마다 보내고, 기본값도 같다
    assert nums["DRAW"] == 2.0 and 18 <= nums["HOLD"] <= 22 and 2.5 <= nums["FADE"] <= 3.5
    assert "draw: DOODLE_SECS" in RS and "DOODLE_SECS" in RS[RS.index("const DOODLE_LIFE") :][:200]
    core = (ROOT / "crates" / "screenpet-core" / "src" / "brain_doodle.rs").read_text(encoding="utf-8")
    assert "pub const DOODLE_SECS: f64 = 2.0;" in core
    assert nums["MAX_DOODLES"] == 6 and nums["MAX_NAME"] == 12
    assert "setInterval" not in JS


def test_name_is_canvas_text_only():
    assert "innerHTML" not in JS and "insertAdjacentHTML" not in JS and "outerHTML" not in JS
    assert "fillText(" in JS


def test_rust_side_keeps_doodles_alive_hidden_rules_and_guests():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 구름 하나를")]
    # 숨기기·자리 비움이면 닫는다 (낙서도 같이 사라진다)
    assert "away || crate::windows_hidden(app)" in tick
    # 손님 것도 (slots 전부), 화면 위에 있을 때만, 전체 화면이면 그리지 않는다
    assert "take_doodle()" in tick
    taking = RS[RS.index("fn doodles(") : RS.index("fn place_doodle(")]
    assert "take_doodle()" in taking and "s.on_screen()" in taking and "overlay_paused(" in taking
    assert "for i in 0..slots.len()" in taking
    # 숨김·자리 비움 동안 그린 것은 버린다 (쌓이지 않게)
    assert "let _ = s.pet.take_doodle();" in tick
    # 낙서가 남아 있는 동안 창을 닫지 않는다
    assert "doodles_until" in RS
    assert "MAX_NAME" in RS and "chars().take(" in RS


# ---- ui/doodle.js를 가짜 캔버스 위에서 (tools/doodle_harness.js) ----


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "doodle_harness.js")


def test_nothing_runs_until_a_doodle_arrives(run):
    assert run["idle"] == 0


def test_doodles_are_capped_and_checked(run):
    assert run["capped"] == 6
    assert run["filtered"] == 1


def test_frames_only_while_drawing_and_fading(run):
    # 보낸 그리기 시간(2초)만큼 그린다 (60fps면 ~125프레임), 가만히 있는 동안은 타이머 하나
    assert 115 <= run["draw_frames"] <= 135
    # 다른 그리기 시간도 따른다 (1초 → ~63프레임), 엉뚱한 값은 막는다
    assert 55 <= run["draw_frames_1s"] <= 70
    assert 115 <= run["draw_frames_junk"] <= 135
    assert run["hold_rafs"] == 0 and run["hold_timers"] == 1
    assert 120 <= run["fade_frames"] <= 260
    assert run["alive_end"] == 0 and run["end_rafs"] == 0 and run["cleared_last"]


def test_name_goes_in_as_text_and_is_cut_to_12(run):
    for style in ("round", "pixel"):
        texts = run[f"name_{style}"]
        assert texts, style
        assert all(len(t) <= 12 for t in texts), texts
        assert texts[0] == "<img src=x o"
        assert run[f"html_{style}"] == 0


def test_both_styles_draw_chalk(run):
    assert run["round_specks"] > 200 and run["pixel_specks"] > 30
    assert run["round_draws"] > 0 and run["pixel_draws"] > 0
