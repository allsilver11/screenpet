"""👻 전체 투명도: 떠 있는 창은 모두 ui/opacity.js를 읽어 `app-opacity`를 듣고 문서 루트 opacity를 바꾼다.
일반 창(설정·편지·도움말·친구 맺기·똥피하기 등)은 읽지 않는다. 트레이 ⚙️ 설정 ▸ 👻 투명도… 창은 5~100% 막대."""
import json
import re
from pathlib import Path

from conftest import needs_node, run_node

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
SRC = ROOT / "src-tauri" / "src"
TRAY = (SRC / "tray.rs").read_text(encoding="utf-8")
HELP = (UI / "help.html").read_text(encoding="utf-8")

# 테두리 있는 일반 창 (투명도를 따르지 않음). 새 일반 창을 만들면 여기에 적는다
NORMAL_PAGES = {
    "board.html", "code.html", "dodge.html", "feedback.html", "help.html", "ladder.html", "letter.html", "link.html",
    "mailbox.html", "name.html", "opacity.html", "pair.html", "postit.html", "size.html", "stats.html", "stats_ask.html", "timeline.html",
}
# 지금 있는 떠 있는 창 (탈것 등 새 떠 있는 창은 NORMAL_PAGES에 없으면 저절로 검사됨)
KNOWN_FLOATING = {"index.html", "house.html", "object.html", "prop.html", "weather.html", "trail.html", "sleigh.html"}


def floating_pages():
    return sorted(p for p in UI.glob("*.html") if p.name not in NORMAL_PAGES)


def scripts(page):
    return re.findall(r'<script src="([^"]+)"', page.read_text(encoding="utf-8"))


def test_known_floating_pages_are_all_there():
    assert KNOWN_FLOATING <= {p.name for p in floating_pages()}


def test_every_floating_page_reads_the_opacity_helper_first():
    for page in floating_pages():
        assert scripts(page)[:1] == ["opacity.js"], page.name


def test_normal_windows_keep_full_opacity():
    for name in NORMAL_PAGES:
        assert "opacity.js" not in scripts(UI / name), name


def test_helper_listens_and_asks_for_the_start_value():
    js = (UI / "opacity.js").read_text(encoding="utf-8")
    assert '"app-opacity"' in js and 'invoke("app_opacity")' in js
    assert "document.documentElement.style.opacity" in js
    # 다른 스크립트의 최상위 이름과 겹치지 않게 함수 하나로 감쌈
    assert "(() => {" in js


@needs_node
def test_helper_applies_the_start_value_and_live_changes(tmp_path):
    harness = tmp_path / "h.js"
    harness.write_text(
        """
const listeners = {};
const root = { style: {} };
global.document = { documentElement: root };
global.window = {
  __SCREENPET_OPACITY__: 0.4,
  __TAURI__: {
    core: { invoke: async (cmd) => (cmd === "app_opacity" ? 0.25 : null) },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async (ev, h) => { listeners[ev] = h; } }) },
  },
};
require(process.argv[2]);
const seen = [root.style.opacity];
setTimeout(() => {
  seen.push(root.style.opacity);
  listeners["app-opacity"]({ payload: 0.05 });
  seen.push(root.style.opacity);
  listeners["app-opacity"]({ payload: 1 });
  seen.push(root.style.opacity);
  console.log(JSON.stringify(seen));
}, 20);
""",
        encoding="utf-8",
    )
    out = run_node(harness, UI / "opacity.js").stdout
    assert json.loads(out) == ["0.4", "0.25", "0.05", "1"]


def test_tray_settings_has_the_opacity_window():
    assert '"👻 투명도…"' in TRAY and "OPACITY_ID => send(app, Input::ShowOpacity)" in TRAY
    assert re.search(r'Submenu::with_items\(app, "⚙️ 설정", true, &\[[^\]]*&sizes, &opacity,', TRAY)
    assert "⚙️ 설정 ▸ 👻 투명도…" in HELP


def test_opacity_window_is_a_5_to_100_slider_with_a_live_label():
    html = (UI / "opacity.html").read_text(encoding="utf-8")
    assert re.search(r'<input id="opacity" type="range" min="5" max="100"', html)
    assert "<output" in html and "opacity_window.js" in html
    win = (UI / "opacity_window.js").read_text(encoding="utf-8")
    assert 'addEventListener("input", () => send(false))' in win and 'addEventListener("change", () => send(true))' in win
    assert 'invoke("opacity_set"' in win and "%" in win


def test_new_floating_windows_start_at_the_current_opacity():
    overlay = (SRC / "overlay.rs").read_text(encoding="utf-8")
    for fn in ("pub fn finish<", "pub fn finish_quiet<"):
        body = overlay[overlay.index(fn) :]
        body = body[: body.index("\n}\n")]
        assert "crate::opacity::init_script()" in body, fn
    loop = (SRC / "game_loop.rs").read_text(encoding="utf-8")
    assert "Input::SetOpacity" in loop and "crate::opacity::apply(" in loop
    opacity = (SRC / "opacity.rs").read_text(encoding="utf-8")
    assert 'const EVENT: &str = "app-opacity";' in opacity and "app.emit(EVENT," in opacity


def test_floating_windows_may_ask_for_the_opacity():
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    floating = next(c for c in caps if c["identifier"] == "floating-windows")
    assert "allow-app-opacity" in floating["permissions"] and "core:event:allow-listen" in floating["permissions"]
    for label in ("pet", "guest-*", "house", "object-*", "prop-*", "hand-*", "weather-*", "trail-*", "sleigh"):
        assert label in floating["windows"], label
