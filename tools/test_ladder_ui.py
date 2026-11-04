"""사다리타기 창 (ui/ladder.html · ui/ladder.js): 계산은 node 하네스로, 화면·커맨드 연결은 글자로 맞춰 본다."""

import json
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


@pytest.fixture(scope="module")
def out():
    return json.loads(subprocess.run(["node", str(HARNESS)], capture_output=True, text=True, check=True, encoding="utf-8").stdout)


# ---- 화면·커맨드 연결 (node 없이) ---------------------------------------------------


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


def test_invite_buttons_cannot_be_pressed_by_keyboard():
    # 포커스를 뺏지 않는 초대 창: 버튼은 Tab으로 가지 않고, Enter·Space는 키 처리에서 막는다
    for bid in ("join", "decline"):
        tag = re.search(rf'<button id="{bid}"[^>]*>', HTML).group(0)
        assert 'tabindex="-1"' in tag, tag
    assert "ignoreKey" in JS


def test_the_invite_tells_about_shrinking_slots_and_privacy():
    assert "참가 인원에 따라 결과 칸이 줄어들 수 있어요" in HTML
    assert "참가하면 다른 참가자에게 내 이름과 동물이 보여요" in HTML


def test_window_margin_matches_between_core_app_and_harness():
    core = float(re.search(r"pub const WINDOW_MARGIN: f64 = ([\d.]+);", CORE_RS).group(1))
    assert "const GAME_MARGIN: f64 = draw::WINDOW_MARGIN;" in LADDER_RS
    harness = float(re.search(r"const MARGIN = (\d+);", HARNESS.read_text(encoding="utf-8")).group(1))
    assert core == harness == 60


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


def test_the_ladder_window_may_use_ipc():
    # Tauri 2: 창 라벨이 capability에 없으면 invoke·listen이 막혀 흰 화면만 뜬다 (첫 Windows 시험에서 실제로 그랬다)
    cap = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))
    label = re.search(r'pub const LABEL: &str = "(\w+)";', LADDER_RS).group(1)
    assert label in cap["windows"], cap["windows"]
    assert "core:default" in cap["permissions"]


def test_text_is_only_ever_set_as_text():
    assert "innerHTML" not in JS


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
def test_ten_name_tags_do_not_overlap(out):
    for t in out["tags"]:
        assert 2 <= t["chars"] <= 6
        # 같은 줄 이웃은 두 칸 건너 — 이름표가 두 칸 폭 안에 들어간다
        assert t["sameRowGap"] > 0, t
        assert t["lifts"] == [0, 18, 0, 18], "neighbours sit on different rows"
    xs = out["board"]["lay"]["xs"]
    widest = max(t["width"] for t in out["tags"] if t["col"] == 48)
    assert xs[0] - widest / 2 >= 0 and xs[-1] + widest / 2 <= out["W"], "edge tags stay inside the window"


@needs_node
def test_every_route_runs_along_the_ladder_to_its_result(out):
    for board in (out["board"], out["boardSmall"]):
        lay = board["lay"]
        for r in board["routes"]:
            assert r["axis"], "only vertical and horizontal segments"
            assert r["last"]["y"] == lay["bottom"]
            assert r["last"]["x"] == lay["xs"][r["endCol"]]
            assert r["end"] == r["last"], "walking past the end stops at the end"
            assert r["first"]["y"] < lay["top"], "starts from the pet's seat above the ladder"
        assert lay["bottom"] < lay["slotTop"]
        assert all(lay["top"] < y < lay["bottom"] for y in lay["rowYs"])
    assert sorted(r["endCol"] for r in out["board"]["routes"]) == list(range(10)), "a permutation"


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
def test_only_the_invite_ignores_enter_and_space(out):
    assert out["keys"] == [True, True, False, False]
    assert out["moods"] == ["bad", "none", "good"]
