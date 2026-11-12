"""📮 개발자 건의함 (예전 "📬 편지함") (ui/mailbox.html · ui/mailbox.js, src-tauri/src/mailbox.rs) — spec 2026-11-10-feedback-mailbox §5.
개발자 비밀키(config.json dev_key)가 있을 때만 메뉴가 생기고, 편지는 그 키로 연다. 보낸 사람이 쓴 글은 textContent로만 넣는다."""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
HTML = (UI / "mailbox.html").read_text(encoding="utf-8")
JS = (UI / "mailbox.js").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "mailbox.rs").read_text(encoding="utf-8")
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
MAIN = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
NET = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
BUILD = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))

COMMANDS = ("mailbox_state", "mailbox_read", "mailbox_delete", "mailbox_block", "mailbox_done", "mailbox_reject", "mailbox_close")


def test_the_mailbox_opens_from_the_board_item_only_with_a_developer_key():
    assert '"📮 건의함"' in TRAY and '"📬 편지함"' not in TRAY
    # build가 dev 여부를 받아, 개발자 컴퓨터에서만 "📮 건의함"이 개발자 건의함을 연다
    assert "BOARD_ID => send(app, if dev { Input::ShowMailbox } else { Input::ShowBoard })" in TRAY
    assert "cfg.dev_secret().is_some()" in MAIN
    assert '<div id="title">📮 건의함</div>' in HTML and "편지함" not in HTML


def test_commands_and_permissions_only_for_the_mailbox_window():
    for cmd in COMMANDS:
        assert f'"{cmd}"' in BUILD, cmd
        assert f"mailbox::{cmd}" in MAIN, cmd
    cap = next(c for c in CAPS["capabilities"] if c["identifier"] == "mailbox-window")
    assert cap["windows"] == ["mailbox"]
    assert sorted(cap["permissions"]) == sorted("allow-" + c.replace("_", "-") for c in COMMANDS)
    assert "mailbox" in next(c for c in CAPS["capabilities"] if c["identifier"] == "events")["windows"]


def test_letters_are_opened_with_the_developer_secret_only():
    assert "letter::open(&item.sealed, &secret)" in RS
    assert "dev_secret()" in NET


def test_sender_text_never_becomes_html():
    assert "innerHTML" not in JS
    assert "textContent" in JS


def test_each_letter_can_be_read_deleted_and_blocked():
    for cmd in ("mailbox_read", "mailbox_delete", "mailbox_block"):
        assert f'invoke("{cmd}"' in JS, cmd
    assert "정말 막을까요?" in JS and "confirm(" not in JS, "block asks with a second click (no confirm dialog)"
    assert 'id="list"' in HTML and 'id="empty"' in HTML


def test_letter_pets_visit_but_never_touch_the_network():
    # 편지를 든 펫은 로컬 손님 ("letter-<id>") — guest_left는 숫자 visit_id만 친구에게 알린다
    assert 'format!("letter-{}"' in RS
    assert "LETTER_VISIT_SECS: u64 = 60" in RS and "LETTER_GAP_SECS: u64 = 30" in RS and "LETTERS_ON_START: usize = 3" in RS
    assert "refuses_arrivals()" in RS


def test_the_developer_can_reject_a_letter_instead():
    """🙅 반려: 반영함 옆, 하나만 (릴레이 v3에 letter_reject로)"""
    assert "🙅 반려" in JS and 'invoke("mailbox_reject", { id: l.id, rejected: reject.checked })' in JS
    assert "if (check.checked) reject.checked = false;" in JS and "if (reject.checked) check.checked = false;" in JS
    assert ".letter.rejected" in HTML
    assert "pub rejected: bool" in RS and "NetInput::MailboxReject(id, rejected)" in RS
    assert "net.mailbox_reject(id, rejected)" in NET


def test_the_developer_can_tick_letters_as_done():
    """반영/미반영 (나만 보는 용도): 편지마다 체크칸, 반영한 편지는 흐리게. 상태는 릴레이에 (두 컴퓨터가 같게)"""
    assert 'type = "checkbox"' in JS and "✅ 반영함" in JS
    assert 'invoke("mailbox_done", { id: l.id, done: box.checked })' in JS or 'invoke("mailbox_done", { id: l.id, done: check.checked })' in JS
    assert ".letter.done" in HTML, "acted-on letters are dimmed"
    assert "pub done: bool" in RS and "NetInput::MailboxDone(id, done)" in RS
