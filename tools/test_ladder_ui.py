"""사다리타기 창 (ui/ladder.html · ui/ladder.js): 계산과 버튼·키 동작은 node 하네스(가짜 DOM)로, 커맨드 연결은 글자로 맞춰 본다."""

import json
import os
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
HTML = (UI / "ladder.html").read_text(encoding="utf-8")
JS = (UI / "ladder.js").read_text(encoding="utf-8")
MAIN_RS = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
LADDER_RS = (ROOT / "src-tauri" / "src" / "ladder.rs").read_text(encoding="utf-8")
CORE_RS = (ROOT / "crates" / "screenpet-core" / "src" / "ladder.rs").read_text(encoding="utf-8")
HARNESS = ROOT / "tools" / "ladder_harness.js"
needs_node = pytest.mark.skipif(not shutil.which("node"), reason="node 없음")
MARGIN = re.search(r"pub const WINDOW_MARGIN: f64 = (\d+)\.0;", CORE_RS).group(1)


@pytest.fixture(scope="module")
def out():
    env = {**os.environ, "LADDER_MARGIN": MARGIN}
    return json.loads(subprocess.run(["node", str(HARNESS)], capture_output=True, text=True, check=True, encoding="utf-8", env=env).stdout)


# ---- 화면·커맨드 연결 (node 없이) ---------------------------------------------------


def test_help_does_not_promise_what_a_modified_host_can_break():
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    assert "방장도 미리 알거나" not in help_html
    assert "결과를 먼저 보고 판을 취소한 뒤 다시 열 수 있고" in help_html
    assert "방장이 알려 준 것이라 사실과 다를 수 있어요" in help_html
    assert "공정한 게임" not in HTML


def test_every_element_the_script_uses_is_in_the_page():
    ids = set(re.findall(r'\$\("([\w-]+)"\)', JS))
    missing = [i for i in sorted(ids) if f'id="{i}"' not in HTML]
    assert not missing, missing


def test_every_command_the_window_invokes_is_registered():
    used = set(re.findall(r'invoke\("(ladder_\w+)"', JS))
    assert used, "no ladder commands?"
    for cmd in sorted(used):
        assert f"ladder::{cmd}," in MAIN_RS, f"{cmd} not in generate_handler!"
        assert f"pub fn {cmd}(" in LADDER_RS, f"{cmd} not defined"


def test_invite_buttons_are_not_in_the_tab_order():
    # 포커스를 뺏지 않는 초대 창: 버튼은 Tab으로 가지 않는다 (Enter·Space는 아래 하네스 시험)
    for bid in ("join", "decline"):
        tag = re.search(rf'<button id="{bid}"[^>]*>', HTML).group(0)
        assert 'tabindex="-1"' in tag, tag


def test_the_invite_tells_about_shrinking_slots_and_privacy():
    assert "참가 인원에 따라 결과 칸이 줄어들 수 있어요" in HTML
    assert "참가하면 다른 참가자에게 내 이름과 동물이 보여요" in HTML


def test_the_app_sizes_the_game_window_with_the_core_margin():
    # 이름표가 창 안에 드는지는 같은 값(MARGIN)으로 하네스가 잰다
    assert "as f64 + draw::WINDOW_MARGIN, GAME_HEIGHT)" in LADDER_RS


def test_the_last_result_window_shows_every_row_and_its_buttons():
    # 10명: 줄 28 × 10 + 표 테두리 10 + 제목 20 + 지문 32 + 안내 16 + 버튼 28 + 사이 8 × 4 + 위아래 여백 28
    needed = 28 * 10 + 10 + 20 + 32 + 16 + 28 + 8 * 4 + 28
    height = float(re.search(r"const LAST_SIZE: \(f64, f64\) = \([\d.]+, ([\d.]+)\);", LADDER_RS).group(1))
    assert height >= needed
    # 그래도 낮은 화면에서는 표만 줄어 스크롤되고, 나머지는 줄지 않는다
    rule = re.search(r"#table > ul \{([^}]*)\}", HTML).group(1)
    assert "min-height: 0" in rule and "overflow-y: auto" in rule and "flex: 0 1 auto" in rule
    assert re.search(r"#table > \* \{ flex: none; \}", HTML)


def test_the_window_is_kept_on_screen_when_it_grows():
    # 보이는 채로 커질 때도 (초대 → 게임) 작업 영역 안으로 옮긴다
    show = re.search(r"\nfn show\(.*?\n}\n", LADDER_RS, re.S).group(0)
    assert "if hidden || resized {" in show and "keep_in_work_area(app, &window, size);" in show
    assert "draw::window_height(height, work_area(app).1)" in LADDER_RS


def test_names_from_friends_are_cleaned_for_the_screen():
    assert "unwrap_or_else(|| shown_name(&p.name))" in LADDER_RS, "player names in the game"
    assert "who.as_deref().map(shown_name)" in LADDER_RS, "the name in a cancel message"
    assert "map_or_else(|| shown_name(g.host_name())," in LADDER_RS, "the host name in an invite"
    assert "draw::shown_text(&r.text, draw::RESULT_MAX_CHARS)" in LADDER_RS, "result slots"


def test_the_ladder_window_may_use_ipc():
    # Tauri 2: 창 라벨이 capability에 없으면 invoke·listen이 막혀 흰 화면만 뜬다 (첫 Windows 시험에서 실제로 그랬다)
    cap = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))
    label = re.search(r'pub const LABEL: &str = "(\w+)";', LADDER_RS).group(1)
    assert label in cap["windows"], cap["windows"]
    assert "core:default" in cap["permissions"]


def test_text_is_only_ever_set_as_text():
    # 친구가 보낸 글자가 HTML로 해석될 길이 없다
    for sink in ("innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval(", "new Function"):
        assert sink not in JS, sink


# ---- 계산 (node 하네스) ---------------------------------------------------------------


@needs_node
def test_names_and_results_are_shortened_by_characters(out):
    assert out["ellipsize"] == ["여섯글자요", "일곱글자입니…", "🐱🐶🐰🐻🐸🦆…", ""]
    assert out["clean"] == ["당첨", None, None, None, "🍕 피자"]


@needs_node
def test_presets_keep_the_winners_and_custom_slots_never_shrink_by_themselves(out):
    assert [s["text"] for s in out["preset"]["four_one"]] == ["당첨", "꽝", "꽝", "꽝"]
    assert [s["mood"] for s in out["preset"]["four_one"]] == ["good", "bad", "bad", "bad"]
    assert [s["text"] for s in out["preset"]["two_five"]] == ["당첨", "당첨"], "winners capped at the player count"
    assert [s["text"] for s in out["grow"]] == ["a", "", ""]
    assert out["noShrink"] == 3, "the host picks which slot to delete (spec §1-3)"
    assert out["problems"] == {"extra": {"extra": 1}, "missing": {"missing": 1}, "blank": {"blank": [1]}, "ok": None}
    assert out["warnings"][0] == "칸이 1개 많아요 — 지울 칸을 골라 주세요 (✕)"
    assert out["warnings"][3] == ""


@needs_node
def test_cancel_messages_do_not_blame_and_use_the_right_particle(out):
    assert out["cancel"] == [
        "나비와 연결이 끊겨 취소됐어요",
        "곰과 연결이 끊겨 취소됐어요",
        "연결이 끊겨 취소됐어요",
        "방장에게서 답이 오지 않아 취소됐어요",
        "방장이 사다리타기를 끝냈어요",
        "사다리 정보가 맞지 않아 취소됐어요",
    ]
    for line in out["cancel"]:
        assert "공개하지 않" not in line and "일부러" not in line
    assert out["answers"] == ["기다리는 중", "참가", "바빠요", "답 없음", ""]


@needs_node
def test_ten_long_name_tags_do_not_overlap_and_stay_in_the_window(out):
    for t in out["tags"]:
        assert 2 <= t["max"] <= 6
        assert t["overlaps"] == 0, t
        assert t["outside"] == 0, t
        assert t["lifts"] == [0, 18, 0, 18], "neighbours sit on different rows"
        assert len(set(t["texts"])) == 10, "shortened names stay apart by their number"


@needs_node
def test_a_shortened_name_keeps_its_number(out):
    assert out["tagText"] == ["곰돌이와친구…", "나비 ②", "곰돌이와… ②", "곰돌이와… ③", "일… (21)"]
    assert out["ui"]["nametags"] == ["곰돌이와친구…", "곰돌이와… ②", "곰돌이와… ③"], "and that is what the game shows"


@needs_node
def test_every_route_runs_along_the_ladder_to_its_result(out):
    for board in (out["board"], out["boardLow"], out["boardSmall"]):
        lay = board["lay"]
        for r in board["routes"]:
            assert r["walks"], "only down, and sideways only along a rung of that row"
            assert r["crossings"] == r["turns"], "no rung skipped"
            assert r["last"]["y"] == lay["bottom"]
            assert r["last"]["x"] == lay["xs"][r["endCol"]]
            assert r["end"] == r["last"], "walking past the end stops at the end"
            assert r["first"]["y"] < lay["top"], "starts from the pet's seat above the ladder"
        assert lay["top"] < lay["bottom"] < lay["slotTop"]
        assert all(lay["top"] < y < lay["bottom"] for y in lay["rowYs"])
    assert sorted(r["endCol"] for r in out["board"]["routes"]) == list(range(10)), "a permutation"
    assert any(r["crossings"] for r in out["board"]["routes"]), "the board has rungs to check"


@needs_node
def test_result_table_and_copy_text(out):
    assert [r["me"] for r in out["results"]].count(True) == 1
    assert out["copy"].splitlines() == [
        "🎲 방장의 사다리타기 · 지문 🐱🍎🎈🌙",
        "나비 → 통과 😀",
        "나비 ② → 통과 😀",
        "곰돌이와친구들 → 커피 쏘기 당첨! 😢",
    ]


@needs_node
def test_changed_slots_are_shown_and_answered_by_click_only(out):
    c = out["ui"]["confirm"]
    assert c["shown"] and "초대 때와 달라요" in c["title"]
    assert c["chips"] == ["벌칙 커피 😢", "꽝 😢"]
    assert c["buttons"] == ["계속", "그만두기"]
    assert c["notes"] == [True, False]
    assert c["enter"], "a stray key does not answer"
    assert c["answered"] == ["ladder_answer", {"join": True}]
    assert c["backToInvite"] == ["참가", False, True], "the next invite reads as an invite again"
    # 칸이 많아도 전부 볼 수 있다
    assert "max-height: 64px; overflow-y: auto;" in re.search(r"\.chips \{([^}]*)\}", HTML).group(1)


@needs_node
def test_keys_reach_buttons_except_on_the_invite(out):
    keys = out["ui"]["keys"]
    assert not keys["enterOnButton"] and not keys["spaceOnButton"], "a focused button can be pressed by keyboard"
    assert not keys["typing"] and not keys["tab"]
    assert keys["enterElsewhere"] and keys["refresh"], "no webview defaults such as F5"
    assert keys["enterOnInvite"] and keys["spaceOnInvite"], "an invite is never answered by a stray key"
    assert keys["escapeOnInvite"]


@needs_node
def test_mood_labels_from_friends_are_only_ever_their_own_keys(out):
    assert out["moods"] == ["bad", "none", "good"]
    assert out["moodEmoji"] == ["😀", "😢", "·", "·", "·", "·", "·"]
    assert out["answerOfOddKey"] == ["", ""]
    assert out["ui"]["chips"] == ["당첨 ·", "꽝 😢"], "the invite screen uses it"


@needs_node
def test_custom_slots_can_be_cut_to_start_before_everyone_answers(out):
    ui = out["ui"]
    assert ui["setupSlots"] == 4
    assert ui["hostInvite"][0] == "ladder_host" and len(ui["hostInvite"][1]["results"]) == 4
    # 셋 초대, 한 명 참가, 둘 기다림: 지금 시작하려면 칸이 2개여야 한다 — ✕가 보여야 맞출 수 있다
    assert ui["early"] == {"slots": 4, "crosses": 4, "startDisabled": True, "warn": "지금 시작하면 2명이에요 — ✕로 칸을 2개로 맞춰 주세요"}
    # 지운 칸이 다시 생기지 않고 바로 시작할 수 있다
    assert ui["deleted"] == {"slots": 2, "crosses": 0, "startDisabled": False, "warn": ""}
    assert ui["begin"][0] == "ladder_begin" and len(ui["begin"][1]["results"]) == 2
    # 그 사이 한 명 더 오면 빈 칸이 생기고, 채울 때까지 막힌다
    assert ui["anotherJoined"] == {"slots": 3, "crosses": 0, "startDisabled": True, "warn": "빈 칸이 있어요 (12자까지)"}


@needs_node
def test_inviting_slot_rules(out):
    i = out["inviting"]
    assert i["startingEarly"] == {"deletable": True, "canStart": False, "warning": "지금 시작하면 2명이에요 — ✕로 칸을 2개로 맞춰 주세요"}
    assert i["afterDeleting"] == {"deletable": False, "canStart": True, "warning": ""}
    assert i["nobodyYet"] == {"deletable": False, "canStart": False, "warning": ""}, "nothing to pick before anyone joined"
    assert i["someoneDeclined"] == {"deletable": True, "canStart": False, "warning": "칸이 1개 많아요 — 지울 칸을 골라 주세요 (✕)"}
    assert i["blank"] == {"deletable": False, "canStart": False, "warning": "빈 칸이 있어요 (12자까지)"}
