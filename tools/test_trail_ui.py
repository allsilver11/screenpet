"""🌈 무지개 구름 자국 창의 약속 — 마우스 통과, 작업표시줄에 안 나옴, 쓸 때만 창이 있고, 구름 수 상한, 비면 그림을 멈춘다."""

import json
import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "trail.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
OVERLAY = (ROOT / "src-tauri" / "src" / "overlay.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
MAIN_JS = (UI / "main.js").read_text(encoding="utf-8")


def test_the_overlay_is_a_shared_click_through_screen_cover():
    # 날씨 창과 같은 만들기 (투명·항상 위·작업표시줄 숨김·포커스 없음·마우스 통과)
    cover = OVERLAY[OVERLAY.index("pub fn screen_cover("):]
    cover = cover[: cover.index("\n}\n")]
    assert "finish(" in cover and "set_ignore_cursor_events(true)" in cover and "scale_of(app, wa)" in cover
    assert ".skip_taskbar(true)" in OVERLAY and ".focused(false)" in OVERLAY
    assert 'screen_cover(app, &label, "trail.html"' in RS
    assert 'screen_cover(app, &label, "weather.html"' in (ROOT / "src-tauri" / "src" / "weather.rs").read_text(encoding="utf-8")
    assert "pointer-events: none" in HTML
    # Alt+F4로 닫히지 않는 떠 있는 창
    assert '"trail-"' in OVERLAY[OVERLAY.index("pub fn is_floating"):]


def test_the_overlay_may_listen_and_nothing_else():
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    holders = [c for c in caps if "trail-*" in c["windows"]]
    assert [c["identifier"] for c in holders] == ["events"]
    assert 'getCurrentWebviewWindow().listen("trail-puffs"' in JS
    assert '"trail-puffs"' in RS


def test_windows_exist_only_while_clouds_are_coming():
    assert "const IDLE_CLOSE: Duration = Duration::from_secs(8);" in RS
    tick = RS[RS.index("pub fn tick("):RS.index("/// 구름 하나를")]
    # 숨기기·자리 비움이면 창을 모두 닫는다
    assert "away || crate::windows_hidden(app)" in tick and "self.tear_down()" in tick
    # 무지개가 아니면 그 동물은 아무 일도 하지 않는다
    assert "trail::rainbow(s.tint, s.entry.manifest.tint)" in tick and "(false, None) => continue" in tick
    # 하우스·친구 화면·전체 화면이면 구름이 나지 않는다
    assert "s.on_screen()" in tick and "overlay_paused(" in tick
    # 손님도 (slots 전부)
    assert "for s in slots {" in tick
    assert "trail.tick(&app, now, dt, presence.is_away(), &slots);" in LOOP


def test_puffs_wait_for_a_new_webview_and_keep_their_age():
    assert "const WARMUP: Duration" in RS and "age: now.duration_since(at)" in RS
    assert "p.age" in JS


def test_colors_match_the_rainbow_body():
    main = re.search(r"const RAINBOW = (\[[^\]]+\]);", MAIN_JS).group(1)
    mine = re.search(r"const RAINBOW = (\[[^\]]+\]);", JS).group(1)
    assert json.loads(mine) == json.loads(main)
    assert len(json.loads(mine)) == 7  # screenpet_core::trail::COLORS


def test_budget():
    assert int(re.search(r"const MAX_PUFFS = (\d+);", JS).group(1)) <= 60
    life = float(re.search(r"const LIFE = ([\d.]+);", JS).group(1))
    assert 0.8 <= life <= 1.2
    assert "setInterval" not in JS


# ---- ui/trail.js를 가짜 캔버스·rAF 위에서 (tools/trail_harness.js) ----


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "trail_harness.js")


def test_nothing_runs_until_a_puff_arrives(run):
    assert run["idle_rafs"] == 0


def test_puffs_are_capped(run):
    assert run["max_alive"] == 60


def test_the_loop_stops_when_every_cloud_has_faded(run):
    assert run["alive_after"] == 0
    assert run["rafs_after"] == 0
    # 다 지운 자리 — 마지막 프레임은 지우기만 한다
    assert run["cleared_last"]


def test_old_puffs_start_part_faded_and_expired_ones_are_dropped(run):
    assert run["aged_alive"] == 1


def test_both_styles_draw(run):
    assert run["round_arcs"] > 0 and run["round_rects"] == 0
    assert run["pixel_rects"] > 0 and run["pixel_arcs"] == 0


def test_help_and_release_note_mention_the_trail():
    help_text = (UI / "help.html").read_text(encoding="utf-8")
    line = next(l for l in help_text.splitlines() if "흐물이 색" in l)
    assert "무지개 구름" in line
    notes = (ROOT / "docs" / "releases" / "v0.6.3.md").read_text(encoding="utf-8")
    assert "무지개 구름" in notes
