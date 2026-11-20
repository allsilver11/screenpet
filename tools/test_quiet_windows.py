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


def test_a_toy_bounced_by_an_old_busy_friend_gets_its_icon_back():
    """옛 버전 친구는 Toss.icon을 몰라 되돌릴 때 빠뜨림 — 보낸 그림을 기억했다가 다시 붙임 (사라지지 않게)."""
    src = _text("objects.rs")
    assert "sent_icons" in src
    receive = src[src.index("pub fn receive("):]
    receive = receive[: receive.index("\n    }\n")]
    assert "toss.bounced" in receive and "sent_icons" in receive


def test_click_through_never_goes_through_tao():
    """tao의 set_ignore_cursor_events는 창 플래그를 다시 적용하는데, tao가 "안 보임"으로 아는 창(show_quietly로 보인 창)은
    그때 SW_HIDE로 숨겨 버림 — 커서 캐치볼 중 흐물이 창이 사라지고 뇌만 돌던 버그(v0.6.6). 그래서 클릭 통과도 overlay의 조용한 함수로만."""
    for path in sorted(SRC.glob("*.rs")):
        src = path.read_text(encoding="utf-8")
        calls = src.count("set_ignore_cursor_events(")
        if path.name == "overlay.rs":
            body = src[src.index("pub fn set_click_through("):]
            body = body[: body.index("\n}\n")]
            assert calls == 1 and "set_ignore_cursor_events(" in body, "only the non-Windows fallback"
            assert "platform::set_click_through" in body
        else:
            assert calls == 0, path.name


def test_a_pet_window_hidden_behind_our_back_is_shown_again():
    src = _text("slots.rs")
    place = src[src.index("pub fn place("):]
    place = place[: place.index("\n    }\n")]
    assert "crate::overlay::is_shown(" in place and "show_quietly" in place


def test_the_house_comes_back_after_hiding_all_and_housing():
    """모두 숨긴 채 하우스로 보내고 다시 보이면 집이 끝내 안 나타나던 버그: tao의 is_visible은 조용히 숨긴 창을 아직
    "보임"으로 알아서 집을 다시 보이지 않았음 → OS에 묻는 overlay::is_shown으로 봄. 집으로 달려가는 동안에도 집이 보임."""
    src = _text("game_loop.rs")
    assert "!w.is_visible().unwrap_or(true)" not in src
    assert "!crate::overlay::is_shown(w)" in src
    assert "is_going_home()" in src[src.index("let hidden = crate::windows_hidden(&app);") - 4000 :]


def test_the_hide_item_says_what_it_will_do():
    """숨기기/보이기 항목은 지금 누르면 할 일만 보임: 보이는 동안 "🙈 숨기기", 숨겨 둔 동안 "🐾 보이기"."""
    src = _text("tray.rs")
    assert '"🐾 보이기"' in src and '"🙈 숨기기"' in src
    show = src[src.index("pub fn show_all("):][:400]
    assert "toggle.set_text(toggle_label(false))" in show
    hide = src[src.index("fn toggle_windows("):][:900]
    assert "toggle.set_text(toggle_label(true))" in hide
