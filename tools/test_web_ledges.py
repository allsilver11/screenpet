"""🌐 웹 페이지 위에도 올라가기 배선: ⚙️ 설정 체크 (기본 끔, 실험) → 켜져 있을 때만 읽기 스레드 하나를 띄우고
(끄면 버려 멈춤), 세계를 새로 읽을 때마다 마지막 디딤돌을 펫 세계에만 넣는다. 물건 세계·친구에게는 가지 않는다.
OS 쪽은 사각형·종류만 읽는다 (이름·값·글자는 읽지 않음). 도움말·README 체크리스트에 적혀 있다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
NET = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
OBJECTS = (ROOT / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
README = (ROOT / "README.md").read_text(encoding="utf-8")
CONFIG = (ROOT / "crates" / "screenpet-core" / "src" / "config.rs").read_text(encoding="utf-8")
PLATFORM = ROOT / "crates" / "screenpet-core" / "src" / "platform"
WEB = (PLATFORM / "win32_web.rs").read_text(encoding="utf-8")
CARGO = (ROOT / "crates" / "screenpet-core" / "Cargo.toml").read_text(encoding="utf-8")


def test_settings_has_an_experimental_check_that_starts_off():
    assert 'const WEB_LEDGES_LABEL: &str = "🌐 웹 페이지 위에도 올라가기 (실험)";' in TRAY
    assert "CheckMenuItem::with_id(app, WEB_LEDGES_ID, WEB_LEDGES_LABEL, screenpet_core::platform::WEB_LEDGES, false, None::<&str>)" in TRAY
    assert "WEB_LEDGES_ID => send(app, Input::ToggleWebLedges)," in TRAY
    assert "pub web_ledges: CheckMenuItem<Wry>," in TRAY
    assert "pub web_ledges_on: bool," in CONFIG


def test_the_loop_flips_saves_resyncs_and_starts_or_drops_the_probe():
    arm = LOOP[LOOP.index("Input::ToggleWebLedges =>") :]
    arm = arm[: arm.index("Input::", 30)]
    assert "config.web_ledges_on = !config.web_ledges_on;" in arm
    assert "menu.web_ledges.set_checked(config.web_ledges_on)" in arm
    assert "web_ledges = start_web_ledges(&config);" in arm
    assert "save_config(" in arm
    start = LOOP[: LOOP.index("let mut pacer")]
    assert "menu.web_ledges.set_checked(config.web_ledges_on);" in start
    assert "let mut web_ledges = start_web_ledges(&config);" in start
    fn = LOOP[LOOP.index("fn start_web_ledges") :]
    fn = fn[: fn.index("\n}\n")]
    # 꺼져 있으면 스레드를 띄우지 않는다
    assert "config.web_ledges_on" in fn and "web_ledge_probe()" in fn


def test_ledges_go_into_pet_worlds_only_on_each_refresh():
    i = LOOP.index("if now - last_refresh >= WORLD_REFRESH")
    block = LOOP[i : LOOP.index("last_refresh = now;", i)]
    assert "let ledges = web_ledges.as_ref().and_then(|p| p.latest());" in block
    assert "s.world.set_ledges(ledges.clone());" in block
    # 물건 세계에는 넣지 않는다
    assert "set_ledges" not in OBJECTS
    # 친구에게 보내지 않는다
    assert "ledge" not in NET.lower()


def test_the_probe_reads_only_rectangles_and_control_types():
    assert "UIA_BoundingRectanglePropertyId" in WEB and "UIA_ControlTypePropertyId" in WEB
    for banned in ("UIA_NamePropertyId", "UIA_ValueValuePropertyId", "CurrentName", "CachedName", "TextPattern", "GetCurrentPropertyValue"):
        assert banned not in WEB, banned
    # COM은 읽기 스레드 안에서 열고, 시간 예산·격자가 있다
    assert "COINIT_MULTITHREADED" in WEB and "CoUninitialize" in WEB
    assert "const BUDGET: Duration" in WEB
    assert re.search(r"const COLS: usize = \d+;", WEB) and re.search(r"const ROWS: usize = \d+;", WEB)
    assert '"Win32_UI_Accessibility"' in CARGO and '"Win32_System_Com"' in CARGO


def test_help_and_readme_mention_it():
    assert "🌐 웹 페이지 위에도 올라가기 (실험)" in HELP
    assert "🌐 웹 페이지 위에도 올라가기" in README
