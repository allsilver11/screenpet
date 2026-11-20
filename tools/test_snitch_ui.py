"""🧙 해리포터·헤르미온느의 웹뷰 쪽: 🟡 골든 스니치 (ui/snitch.js — 무지개 구름·낙서·거미집·박쥐 신호와 같은 모니터 덮개 창 trail.html의 맨 위 캔버스),
👜 구슬 가방 (ui/main.js drawBag — 펫 창의 SVG), 새 말풍선 키의 기본 대사와 도움말 줄. 앱 쪽 연결 (trail.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "snitch.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
MAIN = (UI / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")


def test_the_snitch_has_the_top_canvas_of_the_trail_window():
    assert '<canvas id="snitch">' in HTML and '<script src="snitch.js"></script>' in HTML
    assert HTML.index('id="trail"') < HTML.index('id="snitch"'), "맨 위"
    assert '"trail-snitch"' in RS and '"trail-snitch"' in JS
    assert "setInterval" not in JS and "innerHTML" not in JS


def test_scripts_in_one_page_do_not_clash():
    """trail.html의 스크립트들은 한 페이지의 전역을 같이 쓴다 — 최상위 이름이 겹치면 페이지가 멈춘다."""
    names = {}
    for f in re.findall(r'<script src="(\w+\.js)"></script>', HTML):
        if f == "opacity.js":
            continue
        for n in re.findall(r"^(?:const|let|function) (\w+)", (UI / f).read_text(encoding="utf-8"), re.M):
            assert n not in names, (n, f, names.get(n))
            names[n] = f
    assert "snitch.js" in set(names.values())


def test_rust_side_sends_snitches_from_every_pet_and_keeps_the_window_open():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🖍️ 동물마다 새 낙서")]
    assert "self.snitches(app, now, slots);" in tick
    send = RS[RS.index("fn snitches(") : RS.index("/// 🧹 모든 물건 다 치우기")]
    assert "snitch_spot()" in send and "LINE_EVERY" in send and "self.snitch_at = Some(now);" in send
    house = RS[RS.index("fn housekeep(") :]
    assert "snitch_done" in house[: house.index("self.tear_down();")]


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "snitch_harness.js")


def test_listens_to_one_event_and_idles_until_a_snitch(run):
    assert run["listeners"] == ["trail-snitch"] and run["idle"] == 0


def test_bad_snitches_are_dropped_and_capped(run):
    assert run["filtered"] == 1 and run["capped"] == 4


def test_it_follows_the_snitch_and_stops_when_it_is_gone(run):
    assert run["moving_frames"] == 30 and abs(run["followed"] - 600) < 1
    assert run["arcs"] >= 30 * 3, "공·윤기·줄무늬"
    assert run["after_gone_frames"] <= 1 and run["rafs_after_gone"] == 0


def test_a_caught_snitch_bursts_into_sparkles_then_stops(run):
    assert run["bursts"] == 1
    assert 30 <= run["burst_frames"] <= 60 and run["rafs_end"] == 0 and run["bursts_end"] == 0


def test_the_bag_is_an_svg_drawn_at_the_grip_hand_and_poofs_away():
    index = (UI / "index.html").read_text(encoding="utf-8")
    pet = index[index.index('<div id="pet">') : index.index('<div id="envelope"')]
    assert '<svg id="bag" hidden viewBox="0 0 128 128"' in pet, "#pet 안 — 몸과 같이 뒤집힘"
    bag = MAIN[MAIN.index("function drawBag(") : MAIN.index("function bagPoof(")]
    assert "bag_drag" in bag and "BAG_FLOOR" in bag and "innerHTML" not in bag
    assert "manifest?.grip?.[stage.stage]?.[anim]?.[f]" in MAIN
    assert "drawBag(render.bag || 0," in MAIN and "bagPoof(render.flip)" in MAIN
    assert re.search(r"^    pub bag: f64,", BRAIN, re.M)


def test_wizard_speech_defaults_and_help_lines():
    """🧙 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), 도움말에 해리·헤르미온느 한 줄씩 (해요체)."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("BroomUp", "broom_up"), ("BroomLand", "broom_land"), ("BroomAway", "broom_away"), ("Expelliarmus", "expelliarmus"),
                   ("Leviosa", "leviosa"), ("Fizzle", "fizzle"), ("Reparo", "reparo"), ("IKnow", "i_know"), ("Study", "study"),
                   ("Snitch", "snitch"), ("GotIt", "got_it"), ("TidyUp", "tidy_up"), ("AllClean", "all_clean")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Phew! All clean!"' in extra and 'harry: ["' in MAIN and 'hermione: ["' in MAIN
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    harry = next(li for li in help_html.split("<li>") if li.startswith("🧙 해리포터"))
    assert "빗자루" in harry and "Expelliarmus" in harry and "스니치" in harry and harry.rstrip().endswith("요.</li>")
    hermione = next(li for li in help_html.split("<li>") if li.startswith("📚 헤르미온느"))
    assert "책" in hermione and "가방" in hermione and "Reparo" in hermione and hermione.rstrip().endswith("요.</li>")
