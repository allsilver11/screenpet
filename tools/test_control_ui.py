"""조작 모드: 켤 때 안내 말풍선을 띄우지 않는다 (좁은 창에서 양옆이 잘렸다) — 안내는 도움말에. 🎮 표시는 그대로."""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")


def test_no_hint_bubble_when_control_turns_on():
    toggle = LOOP[LOOP.index("Input::ToggleControl =>"):LOOP.index("Input::ToggleWeather =>")]
    assert "pet-say" not in toggle
    assert "CONTROL_HINT" not in TRAY


def test_help_explains_the_keys_and_the_caveats():
    line = next(x for x in HELP.splitlines() if "🎮 조작 모드" in x)
    assert "걷기" in line and "점프" in line and "앉기" in line and "다른 창에도" in line and "입력 모니터링" in line


def test_mac_permission_hint_fits_the_window_in_one_line():
    hint = re.search(r'KEY_PERMISSION_HINT: &str = "([^"]+)"', TRAY).group(1)
    # 12px 굵은 글꼴: 한글 약 12px, 공백 약 3px + 안쪽 여백·테두리 22px, 창 최소 너비 128px
    width = sum(3 if c == " " else 12 for c in hint) + 22
    assert width <= 128, (hint, width)


def test_controller_badge_stays():
    assert "ctltagEl.hidden = !render.ctl" in (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
