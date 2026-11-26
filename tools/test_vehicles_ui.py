"""🚗 탈것 창·메뉴·펫 좌석 연결: vehicle.html/js, 권한, 펫 말풍선·다리 자르기, 도움말."""
import json
import re
from pathlib import Path

import draw_vehicles as d

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
HTML = (UI / "vehicle.html").read_text(encoding="utf-8")
JS = (UI / "vehicle.js").read_text(encoding="utf-8")
MAIN = (UI / "main.js").read_text(encoding="utf-8")
APP = (ROOT / "src-tauri" / "src" / "vehicles.rs").read_text(encoding="utf-8")


def test_window_page_loads_opacity_then_its_script():
    assert HTML.index('src="opacity.js"') < HTML.index('src="vehicle.js"')
    assert "<script>" not in HTML


def test_webview_frames_match_the_sheet():
    assert f"const FRAMES = {d.FRAMES};" in JS
    assert "vehicles/${info.kind}.png" in JS
    assert 'invoke("vehicle_down")' in JS and 'invoke("vehicle_menu")' in JS
    assert '"vehicle-pose"' in JS and '"vehicle-fx"' in JS


def test_vehicle_windows_may_only_press_and_open_their_menu():
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]
    mine = [c for c in caps if "vehicle-*" in c["windows"]]
    ids = {c["identifier"] for c in mine}
    assert ids == {"events", "floating-windows", "vehicle-windows"}
    own = next(c for c in mine if c["identifier"] == "vehicle-windows")
    assert own["permissions"] == ["allow-vehicle-down", "allow-vehicle-menu"]
    build = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
    assert '"vehicle_down"' in build and '"vehicle_menu"' in build


def test_windows_are_made_off_the_game_loop():
    assert "vehicle-windows" in APP and "fn start_worker" in APP and "finish_quiet" in APP


def test_pets_say_short_english_lines_and_hide_legs_behind_the_door():
    for key, line in (("drive", "vroom!"), ("honk", "beep beep!"), ("hop_in", "hop in!")):
        m = re.search(rf"\n  {key}: \[([^\]]*)\]", MAIN)
        assert m and f'"{line}"' in m.group(1), key
    assert "syncSeat(render.seat" in MAIN and "clipPath" in MAIN


def test_help_explains_vehicles():
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    assert "🚗 탈것 꺼내기" in help_html and "🛻" in help_html and "🚁" in help_html


def test_riders_tilt_with_the_vehicle_around_the_same_ground_point():
    # 탈것 그림은 바퀴 바닥 가운데 (창 아래 UNDER × 0.7 px 위)를 축으로 기울고 — 코어 Vehicle::seat_point도 그 점을 축으로 돌림
    under = d.UNDER * 0.7
    assert f"transform-origin: 50% calc(100% - {under:g}px);" in HTML
    assert f"pub const VEHICLE_SCALE: f64 = 0.7;" in (ROOT / "crates" / "screenpet-core" / "src" / "vehicle.rs").read_text(encoding="utf-8")
    # 펫 웹뷰는 render.lean (= 탈것 tilt × dir) 만큼 발밑 (그림 아래 가운데 = 좌석 자리)을 축으로 같이 돌림 — 반전보다 바깥 (화면 기준)
    assert "render.lean" in MAIN
    m = re.search(r"const lean = ([^;]+);", MAIN)
    assert m and "render.lean" in m.group(1)
    assert re.search(r"lean \? \"50% 100%\"", MAIN)
    assert re.search(r"parts\.unshift\(`rotate\(\$\{lean", MAIN)
