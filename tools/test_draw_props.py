from pathlib import Path

import pytest
from PIL import Image

import draw_props

ROOT = Path(__file__).resolve().parent.parent
NAMES = ("bowl_food", "bowl_water", "cookie", "flower")


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("props")
    return out, draw_props.build(out)


def test_box_matches_the_app_window():
    assert (draw_props.WIDTH, draw_props.HEIGHT) == (56, 48)
    rs = (ROOT / "src-tauri" / "src" / "prop.rs").read_text()
    assert "const WIDTH: f64 = 56.0;" in rs and "const HEIGHT: f64 = 48.0;" in rs
    for name in NAMES:
        assert f'"{name}"' in rs


@pytest.mark.parametrize("name", NAMES)
def test_generated_files(built, name):
    out, _ = built
    for d in (out, ROOT / "ui" / "effects"):
        im = Image.open(d / f"{name}.png")
        assert im.size == (56, 48) and im.mode == "RGBA"


@pytest.mark.parametrize("name", NAMES)
def test_transparent_background_and_sits_on_the_bottom(built, name):
    _, arts = built
    im = arts[name]
    a = im.getchannel("A")
    for corner in ((0, 0), (55, 0), (0, 47), (55, 47)):
        assert a.getpixel(corner) == 0, corner
    x0, y0, x1, y1 = a.getbbox()
    assert x0 >= 1 and x1 <= 55  # 1px 투명 여백
    assert 45 <= y1 <= 48  # 바닥에 앉는다
    # 가운데 정렬
    assert abs((x0 + x1) / 2 - 28) <= 3


@pytest.mark.parametrize("name,lo,hi", [("bowl_food", 40, 56), ("bowl_water", 40, 56), ("cookie", 20, 40), ("flower", 22, 40)])
def test_reads_next_to_a_128px_pet(built, name, lo, hi):
    _, arts = built
    x0, y0, x1, y1 = arts[name].getchannel("A").getbbox()
    assert lo <= x1 - x0 <= hi, x1 - x0
    assert y1 - y0 >= 18


@pytest.mark.parametrize("name", NAMES)
def test_soft_outline_and_enough_ink(built, name):
    _, arts = built
    im = arts[name]
    px = [im.getpixel((x, y)) for x in range(56) for y in range(48)]
    solid = [p for p in px if p[3] == 255]
    assert len(solid) >= 250
    # 외곽선 색(LINE에 가까운 어두운 갈색)이 둘레에 있다
    dark = [p for p in solid if p[0] < 170 and p[1] < 140 and p[2] < 140]
    assert len(dark) >= 40


def test_bowls_differ_by_contents(built):
    _, arts = built
    food, water = arts["bowl_food"], arts["bowl_water"]
    # 물그릇은 파랗고, 밥그릇은 그렇지 않다
    def blue(im):
        return sum(1 for x in range(56) for y in range(48) if (p := im.getpixel((x, y)))[3] == 255 and p[2] > p[0] + 30)
    assert blue(water) > 150
    assert blue(food) < 20


# ---- 직접 주기: 커서 끝에 붙는 음식 (48×48, 가운데) ----
HAND = ("hand_meal", "hand_cookie", "hand_fish", "hand_bone", "hand_carrot", "hand_honey", "hand_nectar", "hand_fly",
        "hand_candy", "hand_lollipop")
HAND += ("hand_apple", "hand_milk")  # 크리스마스
HAND += ("hand_bread", "hand_leaf")  # 농장


@pytest.fixture(scope="module")
def hand_built(tmp_path_factory):
    out = tmp_path_factory.mktemp("hand")
    return out, draw_props.build_hand(out)


def test_hand_icons_match_the_core_mapping():
    assert draw_props.HAND_NAMES == HAND
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "food.rs").read_text()
    for name in HAND:
        assert f'"{name}"' in rs, name


@pytest.mark.parametrize("name", HAND)
def test_hand_icon_files(hand_built, name):
    out, _ = hand_built
    for d in (out, ROOT / "ui" / "effects"):
        im = Image.open(d / f"{name}.png")
        assert im.size == (48, 48) and im.mode == "RGBA"


@pytest.mark.parametrize("name", HAND)
def test_hand_icon_is_centered_kawaii_and_fits(hand_built, name):
    _, arts = hand_built
    im = arts[name]
    a = im.getchannel("A")
    for corner in ((0, 0), (47, 0), (0, 47), (47, 47)):
        assert a.getpixel(corner) == 0, corner
    x0, y0, x1, y1 = a.getbbox()
    assert x0 >= 1 and y0 >= 1 and x1 <= 47 and y1 <= 47  # 1px 투명 여백
    assert 28 <= max(x1 - x0, y1 - y0) <= 46, (x1 - x0, y1 - y0)
    assert abs((x0 + x1) / 2 - 24) <= 3 and abs((y0 + y1) / 2 - 24) <= 3
    solid = [p for x in range(48) for y in range(48) if (p := im.getpixel((x, y)))[3] == 255]
    assert len(solid) >= 250
    assert sum(1 for p in solid if p[0] < 170 and p[1] < 140 and p[2] < 140) >= 40  # 외곽선


def test_hand_icons_all_differ(hand_built):
    _, arts = hand_built
    blobs = {n: arts[n].tobytes() for n in HAND}
    assert len(set(blobs.values())) == len(HAND)
