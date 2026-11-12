"""💌 건의하기 창 (ui/feedback.html · ui/feedback.js, src-tauri/src/feedback.rs) — spec 2026-11-10-feedback-mailbox §4.
글자 수·거절 문구는 node로 그대로 돌려 보고, 커맨드·권한·메뉴 연결은 글자로 맞춰 본다."""

import json
import re
import subprocess
from pathlib import Path

from conftest import needs_node

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
HTML = (UI / "feedback.html").read_text(encoding="utf-8")
JS = (UI / "feedback.js").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "feedback.rs").read_text(encoding="utf-8")
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
MAIN = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
BUILD = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))

# spec §4 문구 (거절 이유 → 한 줄)
REASONS = {
    "daily": "건의는 하루에 5통까지 보낼 수 있어요 — 내일 또 보내 주세요",
    "new": "앱을 쓴 지 하루가 지나야 보낼 수 있어요",
    "ip": "오늘은 편지가 많이 왔어요 — 내일 보내 주세요",
    "global": "오늘은 편지가 많이 왔어요 — 내일 보내 주세요",
    "full": "건의함이 꽉 찼어요 — 나중에 보내 주세요",
    "size": "너무 길어요",
    "blocked": "지금은 보낼 수 없어요",
    "offline": "지금은 보낼 수 없어요",
    "timeout": "지금은 보낼 수 없어요",
    "busy": "지금은 보낼 수 없어요",
    "who-knows": "지금은 보낼 수 없어요",
}


def test_the_menu_opens_the_feedback_window_for_everyone():
    assert '"💌 개발자에게 건의하기…"' in TRAY
    assert "FEEDBACK_ID" in TRAY and "Input::ShowFeedback" in TRAY


def test_a_big_box_for_up_to_a_thousand_characters():
    assert re.search(r'<textarea id="input"[^>]*maxlength="1000"', HTML)
    assert 'id="count"' in HTML and 'id="ok"' in HTML and 'id="cancel"' in HTML
    assert "MAX_CHARS = 1000" in JS


def test_commands_and_permissions_are_wired_only_for_the_feedback_window():
    for cmd in ("feedback_submit", "feedback_close"):
        assert f'"{cmd}"' in BUILD, cmd
        assert f"feedback::{cmd}" in MAIN, cmd
    cap = next(c for c in CAPS["capabilities"] if c["identifier"] == "feedback-window")
    assert cap["windows"] == ["feedback"]
    assert sorted(cap["permissions"]) == ["allow-feedback-close", "allow-feedback-submit"]
    events = next(c for c in CAPS["capabilities"] if c["identifier"] == "events")
    assert "feedback" in events["windows"]
    # 다른 창에서 부르면 아무것도 하지 않는다
    assert RS.count('window.label() != FEEDBACK_LABEL') + RS.count('window.label() == FEEDBACK_LABEL') >= 2


def test_the_letter_is_sealed_to_the_developer_and_carries_no_address():
    assert "letter::seal" in RS and "letter::dev_key()" in RS
    for field in ("text", "name", "species", "stage", "style", "version", "os"):
        assert re.search(rf"\b{field}\b", RS), field
    assert "ip" not in re.findall(r"Letter \{([^}]*)\}", RS, re.S)[0].split()


@needs_node
def test_every_relay_answer_reads_as_the_spec_says():
    start = JS.index("function reasonText")
    end = JS.index("\n}\n", start) + 3
    script = JS[start:end] + "console.log(JSON.stringify(%s.map(reasonText)));" % json.dumps(list(REASONS))
    out = subprocess.run(["node", "-e", script], capture_output=True, text=True, check=True).stdout
    assert json.loads(out) == list(REASONS.values())


def test_success_thanks_and_closes_after_two_seconds():
    assert "보냈어요 💌 고마워요!" in JS
    assert 'setTimeout(() => invoke("feedback_close"), 2000)' in JS
    assert 'listen("feedback-result"' in JS
