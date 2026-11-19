"""🕷️ 거미인간의 줄과 모서리 거미집 — 🌈 무지개 구름·🖍️ 낙서와 같은 모니터 덮개 창 (trail.html)의 가운데 캔버스 (ui/web.js).
새 창 없음 (마우스 통과·포커스 안 뺏음), 손님 거미인간 것도, 창이 움직이면 그 거미집만, 🧹 다 치우기·한 시간마다 전부 지움.
앱 쪽 연결 (trail.rs · game_loop.rs · objects.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "web.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
SRC = ROOT / "src-tauri" / "src"
RS = (SRC / "trail.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
OBJ = (SRC / "objects.rs").read_text(encoding="utf-8")
CORE_WEB = (ROOT / "crates" / "screenpet-core" / "src" / "web.rs").read_text(encoding="utf-8")


def test_webs_share_the_trail_window_between_doodles_and_clouds():
    assert '<canvas id="web">' in HTML and '<script src="web.js"></script>' in HTML
    assert HTML.index('id="doodle"') < HTML.index('id="web"') < HTML.index('id="trail"')
    # 다른 창 종류를 만들지 않는다 (덮개 창은 overlay::screen_cover 하나 — 포커스를 뺏지 않는 마우스 통과 창)
    assert RS.count("screen_cover(") == 1
    for ev in ("trail-webs", "trail-lines", "trail-web-drop", "trail-web-clear"):
        assert f'"{ev}"' in RS and f'"{ev}"' in JS, ev


def test_scripts_in_one_page_do_not_clash():
    """trail.js·doodle.js·web.js는 한 페이지의 전역을 같이 쓴다 — 최상위 이름이 겹치면 페이지가 멈춘다."""
    names = {}
    for f in ("trail.js", "doodle.js", "web.js"):
        for n in re.findall(r"^(?:const|let|function) (\w+)", (UI / f).read_text(encoding="utf-8"), re.M):
            assert n not in names, (n, f, names.get(n))
            names[n] = f


def test_timing_matches_the_core():
    nums = {k: float(v) for k, v in re.findall(r"const (WEB_LIFE|WEB_FADE|WEB_DROP|MAX_WEBS) = ([\d.]+);", JS)}
    core = {k: float(v) for k, v in re.findall(r"pub const (WEB_LIFE_SECS|MAX_WEBS): \w+ = ([\d.]+);", CORE_WEB)}
    assert nums["WEB_LIFE"] == core["WEB_LIFE_SECS"] and nums["MAX_WEBS"] == core["MAX_WEBS"]
    # 앱은 거미집이 다 흐려질 때까지 창을 둔다 (WEB_LIFE_SECS + WEB_FADE)
    assert f"WEB_LIFE_SECS as u64 + {int(nums['WEB_FADE'])}" in RS
    assert "setInterval" not in JS and "innerHTML" not in JS


def test_rust_side_takes_webs_and_lines_from_every_pet_and_keeps_the_window_alive():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🕷️ 동물마다 새 거미집")]
    assert "let _ = s.pet.take_web();" in tick, "숨김·자리 비움 동안 친 거미집은 버림"
    assert "self.spider_webs(app, now, slots);" in tick and "self.web_lines(app, now, slots);" in tick
    taking = RS[RS.index("fn spider_webs(") : RS.index("fn place_web(")]
    assert "for i in 0..slots.len()" in taking and "take_web()" in taking and "s.on_screen()" in taking and "overlay_paused(" in taking
    lines = RS[RS.index("fn web_lines(") : RS.index("pub fn clear_webs(")]
    assert "web_line()" in lines and "LINE_EVERY" in lines and "want == o.lines" in lines
    house = RS[RS.index("fn housekeep(") : RS.index("/// 덮개 창을 모두 닫음")]
    assert "webs_done" in house and "lines_done" in house and "drop_moved_webs" in house
    assert "AUTO_CLEAR" in house and "self.clear_webs(app)" in house


def test_clear_all_also_clears_webs_and_yanked_objects_fly():
    clear = LOOP[LOOP.index("if matches!(input, ObjectInput::ClearAll)") :][:300]
    assert "weather.clear_piles();" in clear and "trail.clear_webs(&app);" in clear
    yank = OBJ[OBJ.index("ObjectPlay::Yank") :][:300]
    assert "l.object.spot(l.id).is_some()" in yank and "l.object.launch(vx, vy)" in yank


def test_window_webs_follow_their_window():
    drop = RS[RS.index("fn drop_moved_webs(") : RS.index("/// 그 자리 모니터의 창")]
    assert "own.world.windows" in drop and '"trail-web-drop"' in drop and "t.window.is_none_or" in drop


# ---- ui/web.js를 가짜 캔버스 위에서 (tools/web_harness.js) ----


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "web_harness.js")


def test_listens_only_to_its_four_events_and_idles_until_one_arrives(run):
    assert run["listeners"] == ["trail-lines", "trail-web-clear", "trail-web-drop", "trail-webs"]
    assert run["idle"] == 0


def test_webs_are_capped_and_checked(run):
    assert run["capped"] == 12 and run["filtered"] == 1


def test_frames_only_while_growing_fading_or_with_a_line(run):
    assert 30 <= run["grow_frames"] <= 50, "피어나는 0.6초"
    assert run["hold_rafs"] == 0 and run["hold_timers"] == 1, "가만히 있는 동안은 타이머 하나"
    assert 200 <= run["fade_frames"] * 16 / 4 <= 700 and run["alive_end"] == 0
    assert run["line_strokes"] >= 20 and run["line_rafs"] == 1, "줄이 있는 동안 매 프레임 (그늘 + 실)"
    assert run["line_rafs_after"] == 0 and run["bad_line_rafs"] == 0


def test_drop_removes_one_web_and_clear_removes_all(run):
    assert run["after_drop"] == 1 and run["after_clear"] == 0
