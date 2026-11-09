from pathlib import Path

import pytest
from PIL import Image

import draw_props
from conftest import assert_folder_matches_shipped

ROOT = Path(__file__).resolve().parent.parent
NAMES = ("bowl_food", "bowl_water", "cookie")


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("props")
    return out, draw_props.build(out)


def test_shipped_props_match_the_generator(built, hand_built, seasonal_built):
    """들어 있는 그림 == 생성기 출력 (그릇·손 음식·눈 소품)."""
    for out, _ in (built, hand_built, seasonal_built):
        assert_folder_matches_shipped(out, ROOT / "ui" / "effects")


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


@pytest.mark.parametrize("name,lo,hi", [("bowl_food", 40, 56), ("bowl_water", 40, 56), ("cookie", 20, 40)])
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
HAND = ("hand_meal", "hand_cookie", "hand_fish", "hand_carrot", "hand_fly",
        "hand_candy", "hand_lollipop")
HAND += ("hand_acorn",)  # 다람쥐
HAND += ("hand_apple", "hand_milk")  # 크리스마스
HAND += ("hand_bread", "hand_leaf")  # 농장
HAND += ("hand_watermelon", "hand_broccoli", "hand_grapes")  # 공룡
HAND += ("hand_shaved_ice",)  # 눈사람
HAND += ("hand_seeds",)  # 새
HAND += ("hand_shrimp",)  # 펭귄
HAND += ("hand_bamboo",)  # 레서판다
HAND += ("hand_special",)  # 특식 🍱
HAND += ("hand_broom",)  # 🧹 빗자루


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


# ---- 계절 날씨: 눈덩이·눈사람 (아래 가운데에 앉는다) ----
SEASONAL = {"snowball": (40, 40), "snowman": (64, 80)}


@pytest.fixture(scope="module")
def seasonal_built(tmp_path_factory):
    out = tmp_path_factory.mktemp("seasonal")
    return out, draw_props.build_seasonal(out)


def test_seasonal_sizes_match_the_app():
    rs = (ROOT / "src-tauri" / "src" / "weather.rs").read_text()
    assert "const SNOWMAN_SIZE: (f64, f64) = (64.0, 80.0);" in rs
    assert "const SNOWBALL_SIZE: (f64, f64) = (40.0, 40.0);" in rs


@pytest.mark.parametrize("name", SEASONAL)
def test_seasonal_files_are_transparent_outlined_and_sit_on_the_bottom(seasonal_built, name):
    out, arts = seasonal_built
    w, h = SEASONAL[name]
    for d in (out, ROOT / "ui" / "effects"):
        im = Image.open(d / f"{name}.png")
        assert im.size == (w, h) and im.mode == "RGBA"
    im = arts[name]
    a = im.getchannel("A")
    for corner in ((0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)):
        assert a.getpixel(corner) == 0, corner
    x0, y0, x1, y1 = a.getbbox()
    assert x0 >= 1 and x1 <= w - 1 and y0 >= 0
    assert h - 3 <= y1 <= h
    assert abs((x0 + x1) / 2 - w / 2) <= 2
    solid = [p for x in range(w) for y in range(h) if (p := im.getpixel((x, y)))[3] == 255]
    assert sum(1 for p in solid if p[0] < 170 and p[1] < 140 and p[2] < 140) >= 30  # 외곽선
    assert sum(1 for p in solid if min(p[:3]) > 235) >= 200  # 눈은 하얗다


def test_snowman_has_a_red_hat_and_a_carrot_nose(seasonal_built):
    _, arts = seasonal_built
    im = arts["snowman"]
    px = [p for x in range(64) for y in range(80) if (p := im.getpixel((x, y)))[3] == 255]
    assert sum(1 for p in px if p[0] > 200 and p[1] < 120 and p[2] < 120) >= 40
    assert sum(1 for p in px if p[0] > 220 and 120 < p[1] < 170 and p[2] < 100) >= 8
