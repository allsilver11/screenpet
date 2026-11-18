"""떠 있는 창 보이기·숨기기 — 포커스를 뺏지 않는 show_quietly로 보인 창은 hide_quietly로만 숨김.

Windows에서 tao는 보임 상태를 따로 기억해서 바뀐 것만 반영함. show_quietly(SW_SHOWNOACTIVATE)는 tao를 거치지 않으니
tao는 계속 "안 보임"으로 알고, 그 뒤 tao hide()는 아무것도 안 함 — 친구에게 간 펫이 화면 끝에 멈춘 채 남던 버그(v0.6.5).
그래서 떠 있는 창은 보이기·숨기기를 둘 다 overlay의 조용한 함수로만 함.
"""
import re
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "src-tauri" / "src"


def _text(name):
    return (SRC / name).read_text(encoding="utf-8")


def test_overlay_has_a_quiet_hide_that_goes_around_tao_on_windows():
    src = _text("overlay.rs")
    assert "pub fn hide_quietly(" in src
    assert "hide_window_quietly" in src


def test_own_pet_window_hides_quietly_when_it_leaves_or_goes_home():
    src = _text("game_loop.rs")
    assert "own.window.hide()" not in src
    assert "hw.hide()" not in src
    assert src.count("crate::overlay::hide_quietly(&own.window)") >= 2


def test_tray_hide_and_show_all_use_the_quiet_pair():
    src = _text("tray.rs")
    body = src[src.index("pub fn show_all"):]
    body = body[: body.index("\n}\n", body.index("fn toggle_windows"))]
    assert "w.show()" not in body and "w.hide()" not in body
    assert "show_quietly" in body and "hide_quietly" in body


def test_weather_and_trail_covers_pause_quietly():
    for name in ("weather.rs", "trail.rs"):
        src = _text(name)
        assert not re.search(r"\b[os]\.window\.hide\(\)", src), name
        assert "hide_quietly" in src, name
