"""⌨️ 타자 응원 배선: ⚙️ 설정 체크 (기본 켜짐) → 게임 루프가 켜져 있고 내 흐물이가 있을 때만 키 누름 *개수*를 읽어
초당 키 수를 흐물이에게 넘긴다. 꺼져 있으면 읽지도 않는다. 도움말은 얼마나 빨리 치는지만 센다고 말한다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
CONFIG = (ROOT / "crates" / "screenpet-core" / "src" / "config.rs").read_text(encoding="utf-8")
WIN = (ROOT / "crates" / "screenpet-core" / "src" / "platform" / "win32.rs").read_text(encoding="utf-8")
MAC = (ROOT / "crates" / "screenpet-core" / "src" / "platform" / "macos.rs").read_text(encoding="utf-8")


def test_settings_has_a_typing_cheer_check_that_starts_on():
    assert 'const TYPING_CHEER_LABEL: &str = "⌨️ 타자 응원";' in TRAY
    assert "CheckMenuItem::with_id(app, TYPING_CHEER_ID, TYPING_CHEER_LABEL, true, true, None::<&str>)" in TRAY
    assert "TYPING_CHEER_ID => send(app, Input::ToggleTypingCheer)," in TRAY
    assert "pub typing_cheer: CheckMenuItem<Wry>," in TRAY
    assert "pub typing_cheer_off: bool," in CONFIG


def test_the_loop_flips_saves_and_resyncs_the_check():
    arm = LOOP[LOOP.index("Input::ToggleTypingCheer =>") :]
    arm = arm[: arm.index("Input::", 30)]
    assert "config.typing_cheer_off = !config.typing_cheer_off;" in arm
    assert "menu.typing_cheer.set_checked(!config.typing_cheer_off)" in arm
    assert "save_config(" in arm
    # 시작할 때도 설정으로 맞춘다
    assert "menu.typing_cheer.set_checked(!config.typing_cheer_off);" in LOOP[: LOOP.index("let mut pacer")]


def test_keys_are_counted_only_when_on_and_only_for_my_heumul():
    i = LOOP.index("source.key_presses()")
    block = LOOP[i - 900 : i + 900]
    # 꺼져 있거나 내 흐물이가 없으면 센서를 부르지 않는다
    assert "!config.typing_cheer_off" in block
    assert "typing_meter.clear()" in block
    assert "set_typing_rate(" in block
    # 손님(친구 펫)에게는 넘기지 않는다
    assert "is_own_heumul" in block
    assert LOOP.count("source.key_presses()") == 1


def test_sensors_never_hook_or_ask_for_permission():
    for src in (WIN, MAC):
        assert "SetWindowsHookEx" not in src and "CGEventTapCreate" not in src
        assert "CGRequestListenEventAccess" not in src and "IOHIDRequestAccess" not in src
    assert "counter_for_event_type(CGEventSourceStateID::HIDSystemState, CGEventType::KeyDown)" in MAC
    assert "GetAsyncKeyState(vk as i32)" in WIN[WIN.index("// ---- ⌨️ typing sensor") :]


def test_help_says_it_counts_speed_not_keys():
    i = HELP.index("⌨️ 타자 응원")
    text = HELP[i : i + 600]
    assert "⚙️ 설정 ▸" in text
    assert "얼마나 빨리" in text and "무엇을" in text
    assert re.search(r"저장하지|남기지", text)
