"""계절 날씨 창의 약속 — 사용자는 CPU에 민감하다: 마우스 통과, 낮은 fps, 적은 알갱이, 권한 등록."""

import json
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
JS = (ROOT / "ui" / "weather.js").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "weather.rs").read_text(encoding="utf-8")


def const(name):
    return int(re.search(rf"const {name} = (\d+);", JS).group(1))


def test_frame_rate_and_particle_budget():
    assert const("FPS") <= 30
    assert const("MAX_PARTICLES") <= 80
    assert const("MIN_PARTICLES") >= 30
    counts = [int(c) for c in re.findall(r"count: (\d+)", JS)]
    assert len(counts) == 4 and all(30 <= c <= 80 for c in counts)


def test_the_overlay_never_takes_a_click():
    assert RS.count("set_ignore_cursor_events(true)") == 2  # 날씨 창, 눈덩이·눈사람 창
    assert RS.count(".focused(false)") == 2
    assert "pointer-events: none" in (ROOT / "ui" / "weather.html").read_text(encoding="utf-8")


def test_nothing_ticks_while_paused():
    # 멈추면 타이머를 끊고, 다음 장면이 올 때까지 다시 걸지 않는다
    stop = JS[JS.index("function stop()"):JS.index("function hash(")]
    assert "clearTimeout(timer)" in stop and "running = false" in stop
    assert "if (!running) return;" in JS
    assert "setInterval" not in JS


def test_the_overlay_may_listen_for_scenes():
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text())
    assert "weather-*" in caps["windows"]


def test_preview_is_not_in_the_menu():
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert "날씨 효과" in tray
    assert "SCREENPET_WEATHER" not in tray and "preview" not in tray


# ---- ui/weather.js를 가짜 캔버스로 돌려 보기 (tools/weather_harness.js) — 호출 수만, 실제 렌더링 비용은 아니다 ----
@pytest.fixture(scope="module", params=["light_snow", "snow", "rain", "petals"])
def run(request):
    if not shutil.which("node"):
        pytest.skip("node not installed")
    out = subprocess.run(["node", str(ROOT / "tools" / "weather_harness.js"), request.param], capture_output=True, text=True, check=True, timeout=60)
    return json.loads(out.stdout)


def test_each_frame_stays_within_budget(run):
    # 지우기 수 = 알갱이 수 (화면 전체를 지우지 않는다). 그리기는 아직 화면 위에 있는 알갱이만큼 더 적을 수 있다
    assert 30 <= run["maxClears"] <= 80
    assert run["maxDraws"] <= run["maxClears"]
    assert run["minDelay"] >= 1000 / 30


def test_particles_stop_at_the_first_top(run):
    # 화면 전체 폭의 윗면이 있으면 아무것도 그 아래로 지나가지 않는다 (옆으로 흘러도 2~3 px 안)
    assert run["deepest"] <= run["lineY"] + 4, run["deepest"]


def test_piles_are_not_repainted_without_a_change(run):
    assert run["repaintedPiles"] == 0


def test_pausing_stops_all_work_and_resuming_restarts(run):
    assert run["pendingAfterPause"] == 0 and run["timersAfterRaf"] == 0
    assert run["drawsWhilePaused"] == 0
    assert run["drawsAfterResume"] > 0
