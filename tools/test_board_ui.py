"""📮 건의함 (ui/board.html · board.js 공개 목록, ui/mailbox.html · mailbox.js 개발자 건의함, 같이 쓰는 pager.js · list.css).
트레이 "📮 건의함"은 누구나 한 항목: 개발자 컴퓨터(dev_key)에서는 개발자 건의함, 다른 컴퓨터에서는 읽기 전용 공개 목록.
공개 목록은 가린 이름(김**)·첫 줄·날짜·✅ 반영됨/🙅 반려됨만, 버튼은 쪽 넘기기와 닫기뿐. 글자는 textContent로만."""

import json
import re
from pathlib import Path

from conftest import needs_node, node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
SRC = ROOT / "src-tauri" / "src"
HTML = (UI / "board.html").read_text(encoding="utf-8")
JS = (UI / "board.js").read_text(encoding="utf-8")
PAGER = (UI / "pager.js").read_text(encoding="utf-8")
LIST_CSS = (UI / "list.css").read_text(encoding="utf-8")
MAILBOX_HTML = (UI / "mailbox.html").read_text(encoding="utf-8")
MAILBOX_JS = (UI / "mailbox.js").read_text(encoding="utf-8")
RS = (SRC / "board.rs").read_text(encoding="utf-8")
MAILBOX_RS = (SRC / "mailbox.rs").read_text(encoding="utf-8")
TRAY = (SRC / "tray.rs").read_text(encoding="utf-8")
MAIN = (SRC / "main.rs").read_text(encoding="utf-8")
NET = (SRC / "network.rs").read_text(encoding="utf-8")
GAME_LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
OVERLAY = (SRC / "overlay.rs").read_text(encoding="utf-8")
BUILD = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
HELP = (UI / "help.html").read_text(encoding="utf-8")
NOTES = (ROOT / "docs" / "releases" / "v0.6.3.md").read_text(encoding="utf-8")
FEEDBACK_HTML = (UI / "feedback.html").read_text(encoding="utf-8")


def test_one_tray_item_for_everyone_that_opens_the_right_window():
    assert 'const BOARD_ID: &str = "board";' in TRAY
    assert 'BOARD_ID => send(app, if dev { Input::ShowMailbox } else { Input::ShowBoard })' in TRAY
    assert "let mailbox = dev.then(|| board.clone());" in TRAY, "the developer's item carries the unread count"
    assert '"📮 건의함".to_string()' in TRAY and 'format!("📮 건의함 ({unread})")' in TRAY
    assert '"📬 편지함' not in TRAY + MAILBOX_RS, "the old item is gone"
    assert "crate::tray::board_label(n)" in MAILBOX_RS
    assert "Input::ShowBoard => crate::board::show(&app)" in GAME_LOOP


def test_commands_and_permissions_only_for_the_board_window():
    for cmd in ("board_list", "board_close"):
        assert f'"{cmd}"' in BUILD, cmd
        assert f"board::{cmd}" in MAIN, cmd
    cap = next(c for c in CAPS if c["identifier"] == "board-window")
    assert cap["windows"] == ["board"]
    assert sorted(cap["permissions"]) == ["allow-board-close", "allow-board-list"]
    assert "board" in next(c for c in CAPS if c["identifier"] == "events")["windows"]
    assert RS.count("if mine(&window)") == 2 and 'window.label() == BOARD_LABEL' in RS
    # 제목 표시줄이 있는 보통 창 (떠 있는 창이 아니다)
    floating = OVERLAY[OVERLAY.index("pub fn is_floating") :]
    assert '"board' not in floating[: floating.index("\n}\n")]
    assert 'title: "ScreenPet 건의함"' in RS and 'title: "ScreenPet 건의함"' in MAILBOX_RS


def test_the_public_list_has_no_controls_and_never_uses_html():
    for src in (JS, PAGER, MAILBOX_JS):
        assert "innerHTML" not in src and "outerHTML" not in src and "insertAdjacentHTML" not in src
        assert "textContent" in src
    assert set(re.findall(r'invoke\("([a-z_]+)"', JS)) == {"board_list", "board_close"}
    code = re.sub(r"^\s*//.*$", "", JS, flags=re.M)
    for word in ("checkbox", "지우기", "막기", "반영함", "읽음", "mailbox_", "<input", "<textarea", "비공개", "private"):
        assert word not in code + HTML, word
    assert '<button id="close">닫기</button>' in HTML


def test_the_disclosure_is_in_the_feedback_window():
    # 사용자: 애매하게 줄바꿈되지 않게 "이름 첫 글자는" 뒤에서 줄을 바꾼다
    assert "보낸 편지의 첫 줄과 이름 첫 글자는<br>📮 건의함에서 다른 사용자에게도 보여요." in FEEDBACK_HTML
    # 사용자: "바랐으면 하는 것"은 어색하다
    assert 'placeholder="추가하고 싶은 기능, 불편한 것, 하고 싶은 말을 적어 주세요"' in FEEDBACK_HTML
    assert "바랐으면" not in FEEDBACK_HTML
    assert "하루에 5통까지" in FEEDBACK_HTML and "한 통" not in FEEDBACK_HTML


def test_both_windows_share_the_pager_and_the_look():
    for html in (HTML, MAILBOX_HTML):
        assert '<script src="pager.js"></script>' in html and '<link rel="stylesheet" href="list.css" />' in html
        assert 'id="pager"' in html and 'id="list"' in html and 'id="empty"' in html
        assert html.index("pager.js") < html.index("board.js" if html is HTML else "mailbox.js")
        assert '<div id="title">📮 건의함</div>' in html
    assert "const PER_PAGE = 5;" in PAGER
    assert "pub const PER_PAGE: usize = 5;" in (ROOT / "crates" / "screenpet-net" / "src" / "board.rs").read_text(encoding="utf-8")
    assert "pub const PER_PAGE: usize = 5;" in (ROOT / "crates" / "screenpet-relay" / "src" / "board.rs").read_text(encoding="utf-8")
    assert ".letter {" in LIST_CSS and "#pager" in LIST_CSS


def test_the_developer_publishes_digests_and_nothing_else():
    assert "board::digest(" in MAILBOX_RS and "fn unpublished(" in MAILBOX_RS
    assert "net.board_put(digests)" in NET and "self.mailbox.published(digests)" in NET
    assert "NetEvent::Mailbox { letters, board } =>" in NET
    assert "NetEvent::Board { result } => crate::board::result(app, result)" in NET
    assert "NetInput::BoardList(page) => match &self.net" in NET


def test_help_and_release_note():
    assert "📮 건의함" in HELP and "김**" in HELP and "🙅 반려됨" in HELP
    assert "하루에 5통" in HELP and "하루에 한 통" not in HELP
    assert "📮 건의함" in NOTES and "🙅" in NOTES and "5통" in NOTES and "🔒 비공개" in NOTES
    assert "🔒 비공개로 둔 건의는 목록에 나오지 않아요" in HELP


@needs_node
def test_the_public_list_shows_masked_names_first_lines_dates_and_status():
    out = node_json(Path(__file__).parent / "board_harness.js", "board")
    assert out["asked"] == [["board_list", {"page": 1}]], "opens on page 1"
    first = out["first"]
    assert first["cards"] == ["💌 김**✅ 반영됨 2026-11-09<b>펭귄</b> 더 주세요", "💌 누군가🙅 반려됨 2026-11-09고마워요", "💌 *2026-11-09그냥"], "text stays text"
    assert (first["pager"], first["prevDisabled"], first["nextDisabled"]) == ("‹ 이전1 / 3다음 ›", True, False)
    nxt = out["next"]
    assert nxt["asked"] == [["board_list", {"page": 2}]]
    assert nxt["prevDisabled"] and nxt["nextDisabled"], "no clicking while a page loads"
    assert out["last"]["pager"] == "‹ 이전3 / 3다음 ›" and not out["last"]["prevDisabled"] and out["last"]["nextDisabled"]
    assert (out["empty"]["empty"], out["empty"]["pager"]) == ("아직 올라온 건의가 없어요", None)
    assert out["offline"]["empty"] == "인터넷 친구 서버에 연결되어 있지 않아요"
    assert out["old"]["empty"] == "서버가 아직 준비되지 않았어요"
    assert out["timeout"]["empty"].startswith("지금은 건의함을 볼 수 없어요")
    assert out["reopen"] == [["board_list", {"page": 1}]], "reopening starts on page 1 again"


@needs_node
def test_the_developer_list_pages_by_five_and_stays_on_its_page():
    out = node_json(Path(__file__).parent / "board_harness.js", "mailbox")
    assert len(out["first"]["cards"]) == 5 and out["first"]["pager"] == "‹ 이전1 / 3다음 ›"
    assert [c.split("편지 ")[1][:2].rstrip("✅") for c in out["third"]["cards"]] == ["2", "1"]
    assert out["afterDelete"]["pager"] == "‹ 이전2 / 2다음 ›", "the last page went empty: one page back"
    assert out["afterRead"]["pager"] == "‹ 이전2 / 2다음 ›" and out["afterRead"]["classes"][1] == "letter unread", "stays on the page"
    # ✅ 반영함 / 🙅 반려: 하나만, 끄면 없음
    assert out["status"] == {"invoked": [["mailbox_done", {"id": 7, "done": True}], ["mailbox_reject", {"id": 7, "rejected": True}]], "done": False, "rejected": True}
    assert out["statusOff"] == {"invoked": [["mailbox_reject", {"id": 7, "rejected": False}]], "done": False, "rejected": False}
    assert out["private"] == {"invoked": [["mailbox_private", {"id": 7, "private": True}]], "labels": ["✅ 반영함", "🙅 반려", "🔒 비공개"]}
    assert out["marked"]["classes"] == ["letter rejected private", "letter done"]
    # 비공개 편지엔 표시 하나 + 체크칸 글자, 아닌 편지엔 체크칸 글자만
    assert [c.count("🔒 비공개") for c in out["marked"]["cards"]] == [2, 1]
    assert (out["none"]["empty"], out["none"]["pager"]) == ("아직 건의가 없어요", None)


@needs_node
def test_pager_math():
    from conftest import run_node

    script = PAGER + "\nconsole.log(JSON.stringify([pageCount(0), pageCount(5), pageCount(6), clampPage(3, 10), clampPage(0, 10), clampPage(9, 0), pageSlice([1,2,3,4,5,6,7], 2), ymd(new Date(2026, 0, 5, 23).getTime() / 1000)]));"
    out = json.loads(run_node("-e", script).stdout)
    assert out == [1, 1, 2, 2, 1, 1, [6, 7], "2026-01-05"]
