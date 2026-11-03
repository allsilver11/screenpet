import math
import re
from pathlib import Path

import pytest
from PIL import Image

import draw_objects as d

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("objects")
    return out, d.build(out)


def test_names_match_the_core_kinds():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "object.rs").read_text(encoding="utf-8")
    assert tuple(re.findall(r'ObjectKind::\w+ => "([a-z]+)",', rs)) == d.NAMES


def test_postit_spots_match_the_webview():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    table = dict((k, (int(x), int(y))) for k, x, y in re.findall(r"(\w+): \[(\d+), (\d+)\]", js))
    assert table == d.POSTIT_AT


@pytest.mark.parametrize("name", d.NAMES)
def test_sprite_fits_the_cell_and_sits_on_the_bottom(built, name):
    out, arts = built
    im = arts[name]
    assert im.size == (128, 128) and im.mode == "RGBA"
    assert Image.open(out / f"{name}.png").size == (128, 128)
    a = im.getchannel("A")
    for corner in ((0, 0), (127, 0)):
        assert a.getpixel(corner) == 0, corner
    left, top, right, bottom = a.getbbox()
    # 64~96 언저리 크기, 바닥에 앉는다
    assert 48 <= right - left <= 124, (left, right)
    assert 124 <= bottom <= 128, bottom
    assert top > 0


@pytest.mark.parametrize("name", d.NAMES)
def test_postit_is_stuck_on_the_object(built, name):
    _, arts = built
    x, y = d.POSTIT_AT[name]
    assert arts[name].getchannel("A").getpixel((x, y)) > 0, "post-it would float in the air"


def test_postit_is_yellow_not_an_envelope_or_heart(built):
    _, arts = built
    im = arts["postit"]
    assert im.size == (d.POSTIT, d.POSTIT)
    r, g, b, a = im.getpixel((d.POSTIT // 2 - 6, d.POSTIT // 2 + 8))
    assert a == 255 and r > 230 and g > 200 and b < 150


def test_shipped_files_exist():
    for name in d.NAMES + ("postit",):
        assert (ROOT / "ui" / "objects" / f"{name}.png").exists()


def test_note_text_reaches_the_page_only_as_text():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    assert "note.textContent = text" in js
    for risky in ("innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval("):
        assert risky not in js


def np_alpha(im):
    import numpy as np

    return np.asarray(im.getchannel("A"))


def core_shapes():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "tumble.rs").read_text(encoding="utf-8")
    table = {}
    for kind, px, py, reach, hull in re.findall(
        r"ObjectKind::(\w+) => Shape \{ pivot: \(([\d.]+), ([\d.]+)\), reach: ([\d.]+), hull: &\[(.*?)\] \}", rs
    ):
        pts = [(float(x), float(y)) for x, y in re.findall(r"\(([\d.]+), ([\d.]+)\)", hull)]
        table[kind.lower()] = {"pivot": (float(px), float(py)), "reach": float(reach), "hull": pts}
    return table


def test_core_shape_table_is_the_one_printed_from_the_art():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "tumble.rs").read_text(encoding="utf-8")
    assert d.shape_table() in rs, "그림이 바뀌었다: python tools/draw_objects.py --shapes 로 다시 찍을 것"


@pytest.mark.parametrize("name", d.NAMES)
@pytest.mark.parametrize("deg", range(0, 360, 30))
def test_turning_sprite_touches_where_the_core_thinks_and_is_not_clipped(built, name, deg):
    """코어의 볼록 껍질이 말하는 가장 낮은 점 = 실제로 돌린 그림의 가장 낮은 점. 그림은 reach 반지름 안에 있다."""
    _, arts = built
    s = core_shapes()[name]
    (px, py), reach = s["pivot"], s["reach"]
    rad = math.radians(deg)
    lowest = max((x - px) * math.sin(rad) + (y - py) * math.cos(rad) for x, y in s["hull"])
    a = np_alpha(arts[name])
    ys, xs = a.nonzero()
    xs, ys = xs + 0.5 - px, ys + 0.5 - py
    assert (xs ** 2 + ys ** 2).max() <= reach ** 2
    solid_y, solid_x = (a >= 128).nonzero()
    drawn = ((solid_x + 0.5 - px) * math.sin(rad) + (solid_y + 0.5 - py) * math.cos(rad)).max()
    assert abs(drawn - lowest) <= 3.0, (drawn, lowest)


@pytest.mark.parametrize("name", d.NAMES)
def test_postit_stays_inside_the_turning_room(name):
    s = core_shapes()[name]
    x, y = d.POSTIT_AT[name]
    assert math.hypot(x - s["pivot"][0], y - s["pivot"][1]) + d.POSTIT_REACH <= s["reach"]
