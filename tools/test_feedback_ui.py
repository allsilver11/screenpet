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


# ---- 🖼️ 사진 한 장 (작은 캡처: 긴 변 640px, 120 KiB까지, 다시 인코딩해 메타데이터 없음) ----

PHOTO_JS = (UI / "photo.js").read_text(encoding="utf-8") if (UI / "photo.js").exists() else ""
CONF = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
NETWORK = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")


def test_a_photo_is_attached_by_button_or_paste_with_a_thumbnail_and_a_remove_button():
    assert re.search(r'<input id="photo-file" type="file" accept="image/\*"', HTML)
    assert 'id="photo-pick"' in HTML and 'id="thumb"' in HTML and 'id="photo-remove"' in HTML and "✕" in HTML
    assert "Ctrl+V" in HTML, "the window says a screenshot can be pasted"
    assert HTML.index('src="photo.js"') < HTML.index('src="feedback.js"')
    assert 'addEventListener("paste"' in JS and "clipboardData" in JS
    assert "shrinkPhoto(" in JS and "innerHTML" not in JS


def test_the_photo_goes_as_base64_and_the_backend_checks_it_again():
    assert 'invoke("feedback_submit", { text: inputEl.value, photo' in JS
    assert "photo: Option<String>" in RS and "icon::from_base64" in RS
    assert "letter::photo_kind" in RS and "letter::MAX_IMAGE" in RS
    assert "letter::seal_letter" in RS
    assert "NetInput::SendFeedback(text, photo)" in RS
    assert "photo_dropped" in RS and "photo_dropped" in NETWORK


def test_the_photo_is_read_only_through_same_origin_blob_urls():
    """CSP는 img-src에 blob:만 더한다 (스크립트가 만든 같은 출처 Blob만 — data:·http는 그대로 막힘)"""
    img = CONF["app"]["security"]["csp"]["img-src"]
    assert img == "'self' blob:", img
    assert "createObjectURL" in PHOTO_JS and "revokeObjectURL" in PHOTO_JS


@needs_node
def test_sent_and_photo_answers_read_as_planned():
    start = JS.index("function reasonText")
    end = JS.index("\n}\n", start) + 3
    start2 = JS.index("function sentText")
    end2 = JS.index("\n}\n", start2) + 3
    script = JS[start:end] + JS[start2:end2] + 'console.log(JSON.stringify([reasonText("photo"), sentText(false), sentText(true)]));'
    out = subprocess.run(["node", "-e", script], capture_output=True, text=True, check=True).stdout
    assert json.loads(out) == ["사진을 붙일 수 없어요 — 다른 사진을 골라 주세요", "보냈어요 💌 고마워요!", "보냈어요 💌 (서버가 아직 사진을 못 받아서 글만 갔어요)"]


@needs_node
def test_photos_shrink_to_a_small_screenshot_or_are_refused():
    from conftest import node_json

    out = node_json(Path(__file__).parent / "photo_harness.js")
    assert (out["MAX_SIDE"], out["MAX_BYTES"]) == (640, 120 * 1024)
    big = out["big"]
    assert (big["w"], big["h"], big["type"]) == (640, 360, "image/webp"), "the long side to 640, aspect kept, WebP"
    assert big["tries"][0] == [640, 360, "image/webp", 0.8], "first try: quality 0.8"
    assert big["bytes"] <= 120 * 1024
    small = out["small"]
    assert (small["w"], small["h"]) == (300, 200), "never scaled up"
    busy = out["busy"]
    assert busy["bytes"] <= 120 * 1024 and busy["w"] < 640, "lower quality, then smaller, until it fits"
    assert all(t[3] >= 0.5 for t in busy["tries"])
    assert out["safari"]["type"] == "image/jpeg", "a webview that cannot write WebP falls back to JPEG"
    assert out["never"] is None, "too big even at the smallest: refused"
    assert out["kinds"] == ["image/webp", "image/jpeg", None, None, None]
    assert out["b64"] == [0, 1, 2, 250, 255, 7], "base64 round trip"
