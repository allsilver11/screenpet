"""🐍 볼드모트·👻 디멘터·⚔️ 결투·🦌 패트로누스의 웹뷰 쪽: ✨ 마법 효과 (ui/magic.js — 무지개 구름·낙서·거미집·박쥐 신호·스니치와 같은 모니터 덮개 창
trail.html의 스니치 아래 캔버스), 새 말풍선 키의 기본 대사와 도움말 줄. 앱 쪽 연결 (trail.rs·game_loop.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "magic.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
MAIN = (UI / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
MAGIC_RS = (ROOT / "crates" / "screenpet-core" / "src" / "magic.rs").read_text(encoding="utf-8")


def test_magic_has_the_canvas_just_under_the_snitch():
    assert '<canvas id="magic">' in HTML and '<script src="magic.js"></script>' in HTML
    assert HTML.index('id="trail"') < HTML.index('id="magic"') < HTML.index('id="snitch"')
    assert '"trail-magic"' in RS and '"trail-magic"' in JS
    assert "setInterval" not in JS and "innerHTML" not in JS


def test_the_webview_knows_every_effect_kind_of_the_core():
    kinds = re.findall(r"^    (\w+) \{", MAGIC_RS[MAGIC_RS.index("pub enum MagicFx") :], re.M)
    snake = [re.sub(r"(?<!^)([A-Z])", r"_\1", k).lower() for k in kinds]
    assert snake == ["bolt", "duel", "snake", "stag", "frost", "egg", "hatchling"]
    assert 'const MG_KINDS = ["bolt", "duel", "snake", "stag", "frost", "egg", "hatchling"];' in JS
    colors = MAGIC_RS[MAGIC_RS.index("pub enum BeamColor") :]
    for c in re.findall(r"^    (\w+),", colors[: colors.index("\n}")], re.M):
        assert f'"{c.lower()}"' in JS or c == "Red", c


def test_rust_side_sends_magic_from_every_pet_and_keeps_the_window_open_and_chill_reaches_care():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🖍️ 동물마다 새 낙서")]
    assert "self.magic(app, now, slots);" in tick
    send = RS[RS.index("fn magic(") : RS.index("/// 🧹 모든 물건 다 치우기")]
    assert "magic_fx(" in send and "LINE_EVERY" in send and "f.anchor()" in send and "f.mapped(" in send
    house = RS[RS.index("fn housekeep(") :]
    assert "magic_done" in house[: house.index("self.tear_down();")]
    assert "take_chill()" in LOOP and "care.chill(chill)" in LOOP


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "magic_harness.js")


def test_listens_to_one_event_and_idles_until_magic(run):
    assert run["listeners"] == ["trail-magic"] and run["idle"] == 0


def test_bad_effects_are_dropped_and_capped(run):
    assert run["filtered"] == 1 and run["capped"] == 24


def test_frost_alone_draws_once_without_animation_frames(run):
    assert run["frost_drawn"] and run["frost_rafs"] == 0


def test_moving_magic_animates_then_a_clashed_duel_pops_and_a_bolt_fades(run):
    assert run["moving_frames"] == 30 and run["ellipses"] >= 30 * 3, "사슴 몸·머리·뱀 머리"
    assert run["pops"] == 1 and run["ghosts"] == 1
    assert 10 <= run["end_frames"] <= 60 and run["rafs_end"] == 0 and run["pops_end"] == 0 and run["ghosts_end"] == 0
    assert run["no_pop"] == 0, "아직 뻗던 결투는 펑 없음"


def test_hagrid_egg_and_baby_dragon_animate_and_are_checked(run):
    """🥚 용알·🐉 아기 용: 이상한 값은 버리고, 있는 동안 프레임을 돌리며 그림 (깨어난 용은 끝에 불 퐁 — 그라데이션)."""
    assert run["hagrid_filtered"] == 2
    assert run["hagrid_frames"] == 20 and run["hagrid_ellipses"] >= 20 * 4 and run["hagrid_flame"] > 0


def test_dark_speech_defaults_and_help_lines():
    """🐍👻 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), 도움말에 볼드모트·디멘터 한 줄씩 (해요체)."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("Stupefy", "stupefy"), ("Dazed", "dazed"), ("Nagini", "nagini"), ("Hehehe", "hehehe"), ("DuelCall", "duel_call"),
                   ("Patronus", "patronus"), ("Gloom", "gloom"), ("Repelled", "repelled")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Expecto Patronum!"' in extra and 'voldemort: ["' in MAIN and 'dementor: ["' in MAIN
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    v = next(li for li in help_html.split("<li>") if li.startswith("🐍 볼드모트"))
    assert "나기니" in v and "기절" in v and "결투" in v and "헤르미온느" in v and v.rstrip().endswith("요.</li>")
    d = next(li for li in help_html.split("<li>") if li.startswith("👻 디멘터"))
    assert "서리" in d and "패트로누스" in d and d.rstrip().endswith("요.</li>")
