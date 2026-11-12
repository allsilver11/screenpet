"""🚀 컴퓨터 켜면 자동 실행 + 🔄 업데이트 (우리 서버 pet.danni.day/downloads의 latest.json을 보고 받아 설치). 순수한 부분은 screenpet-core::update의 Rust 시험이 본다."""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"
TRAY = (SRC / "tray.rs").read_text(encoding="utf-8")
MAIN = (SRC / "main.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
CARGO = (ROOT / "src-tauri" / "Cargo.toml").read_text(encoding="utf-8")


def update_rs():
    return (SRC / "update.rs").read_text(encoding="utf-8")


def autostart_rs():
    return (SRC / "autostart.rs").read_text(encoding="utf-8")


def test_autostart_is_a_settings_checkbox_that_the_game_loop_applies():
    assert 'const AUTOSTART_ID: &str = "autostart";' in TRAY
    assert 'CheckMenuItem::with_id(app, AUTOSTART_ID, "🚀 컴퓨터 켜면 자동 실행", true, true, None::<&str>)' in TRAY
    assert "AUTOSTART_ID => send(app, Input::ToggleAutostart)," in TRAY
    assert "pub autostart: CheckMenuItem<Wry>," in TRAY
    settings = re.search(r'Submenu::with_items\(app, "⚙️ 설정", true, &\[([^\]]*)\]', TRAY).group(1)
    assert settings.rstrip().endswith("&autostart"), settings
    # 처음엔 켜고 (config.settle_autostart), 그 뒤로는 고른 대로 OS에 다시 맞춘다. 누르면 바로 바뀌고 저장
    assert "crate::autostart::apply_at_start(&app, &mut config)" in LOOP
    arm = LOOP[LOOP.index("Input::ToggleAutostart =>") :]
    arm = arm[: arm.index("Input::", 30)]
    assert "crate::autostart::toggle(&app, &mut config)" in arm and "save_config(" in arm
    src = autostart_rs()
    assert "settle_autostart()" in src
    assert "autolaunch().enable()" in src and "autolaunch().disable()" in src
    assert "menu.autostart.set_checked(" in src


def test_autostart_plugin_is_registered_after_single_instance():
    assert 'tauri-plugin-autostart = "2"' in CARGO
    single = MAIN.index("tauri_plugin_single_instance::init(")
    auto = MAIN.index("tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None)")
    assert single < auto
    # 창(JS)은 자동 실행 커맨드를 부르지 않는다: Rust에서만 켜고 끈다 — 권한을 주지 않는다
    caps = (ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8")
    assert "autostart" not in caps


def test_update_item_sits_under_help_and_starts_the_updater():
    assert 'const UPDATE_ID: &str = "update";' in TRAY
    assert 'MenuItem::with_id(app, UPDATE_ID, "🔄 업데이트", true, None::<&str>)' in TRAY
    assert "UPDATE_ID => crate::update::on_click(app)," in TRAY
    assert "pub update: MenuItem<Wry>," in TRAY
    assert "menu.append(&help)?;\n    menu.append(&update)?;\n    menu.append(&quit)?;" in TRAY
    assert "update::start_quiet_checks(app.handle())" in MAIN


def test_update_uses_one_tls_stack_and_a_plain_user_agent():
    dep = re.search(r"^ureq = (.*)$", CARGO, re.M).group(1)
    assert "default-features = false" in dep and '"rustls"' in dep
    src = update_rs()
    assert 'format!("ScreenPet/{}", env!("CARGO_PKG_VERSION"))' in src
    assert "update::LATEST_URL" in src and "update::check(" in src
    # 넘겨주기는 직접 따라가며 한 번마다 허용된 곳인지 본다
    assert ".max_redirects(0)" in src and "update::redirect_target(" in src and "update::MAX_REDIRECTS" in src


def test_updates_come_from_our_server_only_never_github_or_the_relay_setting():
    src = update_rs()
    core = (ROOT / "crates" / "screenpet-core" / "src" / "update.rs").read_text(encoding="utf-8")
    assert 'pub const LATEST_URL: &str = "https://pet.danni.day/downloads/latest.json";' in core
    assert 'pub const ALLOWED_HOST: &str = "pet.danni.day";' in core
    for text in (src, core.split("#[cfg(test)]")[0]):
        assert not re.search(r"github\.com|githubusercontent|X-GitHub|vnd\.github", text, re.I)
    # 주소는 고정: 사용자가 바꾸는 릴레이 주소(relay_url.txt)에서 가져오지 않는다
    assert "relay_url" not in src and "network::" not in src
    # 처음 주소도 넘겨받은 주소도 pet.danni.day https인지 본다
    get = src[src.index("fn get(") :]
    get = get[: get.index("\n}\n")]
    assert get.index("update::allowed_url(") < get.index(".call()")
    assert '"application/json"' in src


def test_latest_json_is_trusted_only_after_its_signature_checks_out():
    src = update_rs()
    fetch = src[src.index("fn fetch_latest(") :]
    fetch = fetch[: fetch.index("\n}\n")]
    signed = src[src.index("fn fetch_signed(") :]
    signed = signed[: signed.index("\n}\n")]
    assert "update::LATEST_SIG_URL" in signed and "update::verify_latest(&json, &sig)" in signed
    # 서명 확인이 끝난 바이트만 check로 간다
    assert fetch.index("fetch_signed(") < fetch.index("update::check(")
    assert "update::LATEST_URL" not in fetch


def test_download_is_size_capped_and_sha256_verified_before_install():
    src = update_rs()
    assert "Sha256::new()" in src and "release.asset.sha256" in src
    assert "UpdateError::DigestMismatch" in src and "UpdateError::SizeMismatch" in src
    assert "release.asset.size" in src
    # 확인이 끝나기 전엔 .part — 확인 뒤에야 이름을 바꾼다
    assert ".part" in src
    verify = src.index("UpdateError::DigestMismatch")
    assert verify < src.index("fn install_windows") or verify < src.index("install(")


def test_windows_runs_the_nsis_installer_with_passive_flags_without_a_shell():
    src = update_rs()
    win = src[src.index("fn install_windows") :]
    win = win[: win.index("\n}\n")]
    assert "Command::new(&installer)" in win and ".args(update::NSIS_ARGS)" in win
    assert "cmd" not in win.lower().replace("command", "")
    assert "quit_app(" in win


def test_macos_swaps_the_bundle_with_a_helper_that_takes_paths_as_arguments():
    src = update_rs()
    mac = src[src.index("fn install_macos") :]
    mac = mac[: mac.index("\n}\n")]
    for arg in ('"attach"', '"-nobrowse"', '"-readonly"', '"-mountpoint"'):
        assert arg in mac, arg
    assert "update::app_bundle_of(" in mac and "update::pick_app(" in mac
    assert 'Command::new("/bin/sh")' in mac and ".arg(update::MAC_SWAP_SCRIPT)" in mac
    assert "format!(" not in mac[mac.index('Command::new("/bin/sh")') :], "no paths spliced into the script"
    assert "process_group(0)" in mac and "quit_app(" in mac


def test_quiet_checks_never_install():
    src = update_rs()
    quiet = src[src.index("pub fn start_quiet_checks") :]
    quiet = quiet[: quiet.index("\n}\n")]
    assert "update::FIRST_CHECK_AFTER" in quiet and "update::CHECK_EVERY" in quiet
    assert "install" not in quiet and "download" not in quiet


def test_help_and_release_notes_mention_both():
    assert "🚀 컴퓨터 켜면 자동 실행" in HELP and "🔄 업데이트" in HELP
    notes = (ROOT / "docs" / "releases" / "v0.6.3.md").read_text(encoding="utf-8")
    assert "🚀" in notes and "🔄" in notes
