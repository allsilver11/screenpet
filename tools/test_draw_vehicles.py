"""🚗 탈것 그림 (tools/draw_vehicles.py): 코어 vehicle.rs와 같은 이름·크기·좌석, 칸 크기, 바닥 자리, 들어 있는 그림과 같음."""
import re
from pathlib import Path

import pytest
from PIL import Image

import draw_vehicles as d
from conftest import assert_folder_matches_shipped

ROOT = Path(__file__).resolve().parent.parent
CORE = (ROOT / "crates" / "screenpet-core" / "src" / "vehicle.rs").read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("vehicles")
    return out, d.build(out)


def spec_of(kind):
    """vehicle.rs Spec 한 줄의 숫자들"""
    line = re.search(rf"VehicleKind::{kind} => Spec {{([^}}]*)}}", CORE).group(1)
    return {k: v for k, v in re.findall(r"(\w+): (-?[\d.]+)", line)}


def test_names_match_the_core_kinds():
    assert tuple(re.findall(r'VehicleKind::\w+ => "([a-z_]+)",', CORE)) == d.NAMES


@pytest.mark.parametrize("name,kind", [("car", "Car"), ("heli", "Heli"), ("scooter", "Scooter"), ("truck", "Truck")])
def test_sizes_and_seats_match_the_core(name, kind):
    s = spec_of(kind)
    assert (float(s["w"]), float(s["h"])) == d.SIZE[name]
    assert (float(s["seat_x"]), float(s["cover"]), float(s["seat_min"])) == d.SEAT[name]
    assert f"pub const WINDOW_PAD: f64 = {d.PAD:.1f};" in CORE
    assert f"pub const LEG_SHARE: f64 = {d.LEG_SHARE};" in CORE
    if name in d.BED:
        xs, cover = d.BED[name]
        line = re.search(rf"VehicleKind::{kind} => Spec {{([^}}]*)}}", CORE).group(1)
        assert f"bed_x: [{xs[0]:.1f}, {xs[1]:.1f}]" in line and f"bed_cover: {cover:.1f}" in line


@pytest.mark.parametrize("name", d.NAMES)
def test_sheets_are_window_sized_frames_standing_on_the_bottom(built, name):
    _, arts = built
    ww, wh = d.window(name)
    im = arts[name]
    assert im.size == (ww * d.OUT * d.FRAMES, wh * d.OUT) and im.mode == "RGBA"
    for n in range(d.FRAMES):
        cell = im.crop((n * ww * d.OUT, 0, (n + 1) * ww * d.OUT, wh * d.OUT))
        left, top, right, bottom = cell.getchannel("A").getbbox()
        # 그림이 창 여유 안쪽에 있고, 바퀴(썰매 다리) 바닥은 칸 아래 UNDER 여유 안 (외곽선만 그 아래로)
        assert left > 0 and right < cell.width and top > 0
        assert (wh - d.UNDER - 3) * d.OUT <= bottom <= wh * d.OUT, (name, n, bottom)


@pytest.mark.parametrize("name", d.NAMES)
def test_frames_differ_so_wheels_or_rotor_turn(built, name):
    _, arts = built
    ww, wh = d.window(name)
    cells = [arts[name].crop((n * ww * d.OUT, 0, (n + 1) * ww * d.OUT, wh * d.OUT)).tobytes() for n in range(d.FRAMES)]
    assert len(set(cells)) == d.FRAMES


def test_matches_shipped(built):
    out, _ = built
    assert_folder_matches_shipped(out, ROOT / "ui" / "vehicles")
