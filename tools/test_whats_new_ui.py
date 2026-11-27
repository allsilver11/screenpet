"""✨ 업데이트 소식 창: 업데이트 뒤 처음 켤 때 한 번 릴리즈 노트를 보임 (코어 whats_new.rs가 고르고 src-tauri/src/whats_new.rs가 띄움).
창(ui/whats_new.*)은 노트 마크다운을 글자로만 그림 — tools/whats_new_harness.js로 가짜 DOM에서 돌려 봄."""

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
SRC = ROOT / "src-tauri" / "src"
JS = (UI / "whats_new.js").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
TRAY = (SRC / "tray.rs").read_text(encoding="utf-8")
BUILD = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))
HARNESS = ROOT / "tools" / "whats_new_harness.js"


def _draw(notes):
    out = subprocess.run(["node", str(HARNESS), json.dumps(notes)], capture_output=True, text=True, check=True)
    return json.loads(out.stdout)


def _flat(node, depth=0):
    """(태그, 글자, 깊이) 차례"""
    tag, text, kids = node
    out = [(tag, text, depth)]
    for k in kids:
        out += _flat(k, depth + 1)
    return out


NOTE = """## 🎄 크리스마스 친구들 — 0.7.7

**🆕 새 캐릭터**
- 🎅 산타가 와요.
- **루돌프**는 코가 반짝여요.

**🔧 고친 것**
- 디멘터 모자가 잘리던 문제를 고쳤어요.
"""


def test_a_note_draws_its_title_group_headings_and_lists_with_bold_kept():
    tree = _flat(_draw([{"version": "v0.7.7", "text": NOTE}]))
    tags = [(t, x) for t, x, _ in tree if t in ("h2", "h3", "li", "b")]
    assert ("h2", "🎄 크리스마스 친구들 — 0.7.7") in tags
    assert ("h3", "🆕 새 캐릭터") in tags and ("h3", "🔧 고친 것") in tags
    assert sum(1 for t, _ in tags if t == "li") == 3
    assert ("b", "루돌프") in tags
    # 별표 글자는 남지 않음
    assert not any("**" in x for _, x, _ in tree)


def test_several_versions_show_newest_first_and_untitled_notes_get_the_version():
    tree = _flat(_draw([{"version": "v0.7.8", "text": "- 하나"}, {"version": "v0.7.7", "text": NOTE}]))
    h2 = [x for t, x, _ in tree if t == "h2"]
    assert h2 == ["v0.7.8", "🎄 크리스마스 친구들 — 0.7.7"]


def test_text_only_no_html_injection():
    assert "innerHTML" not in JS and "insertAdjacentHTML" not in JS
    tree = _flat(_draw([{"version": "v1.0.0", "text": "- <img src=x onerror=alert(1)>"}]))
    assert any(t == "span" and "<img" in x for t, x, _ in tree)


def test_shown_once_after_an_update_and_again_from_the_tray():
    assert "crate::whats_new::on_start(&app, &mut config, new_id)" in LOOP
    assert '"✨ 업데이트 소식"' in TRAY and "Input::ShowWhatsNew" in TRAY
    assert "docs/releases" in BUILD and "release_notes.rs" in BUILD


def test_only_its_own_window_may_call_its_commands():
    caps = [c for c in CAPS["capabilities"] if c["windows"] == ["whats-new"]]
    assert caps and set(caps[0]["permissions"]) == {"allow-whats-new-state", "allow-whats-new-close"}
