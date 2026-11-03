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
