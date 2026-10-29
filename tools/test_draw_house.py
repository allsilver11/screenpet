import json
from pathlib import Path

import pytest
from PIL import Image

import draw_house
from sprite_spec import FRAME, STAGES

ROOT = Path(__file__).resolve().parent.parent


def _inside_arch(door, px, py, inset):
    """문 안쪽(위 반원 아치 + 아래 사각)을 inset만큼 안으로 줄인 영역인가 (픽셀 중심 기준)."""
    x, y, w = door["x"], door["y"], door["w"]
    r = w / 2
    cx, cy = x + r, y + r
    if not (x + inset <= px <= x + w - inset):
        return False
    if py >= cy:
        return True
    return (px - cx) ** 2 + (py - cy) ** 2 <= (r - inset) ** 2


def test_generated_files(tmp_path):
    draw_house.build(tmp_path)
    for out in (tmp_path, ROOT / "ui" / "house"):
        for name in ("house.png", "house_front.png"):
            im = Image.open(out / name)
            assert im.size == (FRAME, FRAME) and im.mode == "RGBA"
        assert json.loads((out / "house.json").read_text())["door"]


def test_house_sits_on_bottom_edge(tmp_path):
    draw_house.build(tmp_path)
    house = Image.open(tmp_path / "house.png")
    assert house.getpixel((0, 0))[3] == 0  # 투명 배경
    x0, y0, x1, y1 = house.getchannel("A").getbbox()
    assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1  # 1px 투명 여백
    assert 121 <= y1 <= FRAME  # 바닥은 프레임 아래 끝(y 120–127)에 앉는다


def test_door_rect_inside_frame_and_pet_sized(tmp_path):
    draw_house.build(tmp_path)
    door = json.loads((tmp_path / "house.json").read_text())["door"]
    assert set(door) == {"x", "y", "w", "h"}
    assert door["x"] >= 1 and door["y"] >= 1
    assert door["x"] + door["w"] <= FRAME - 1 and door["y"] + door["h"] <= FRAME - 1
    assert 48 <= door["w"] <= 56 and 56 <= door["h"] <= 64
    # 문 바닥 = 집 바닥: 문 안쪽 바로 위는 어둡고 불투명, 문턱 아래로는 집이 이어진다
    house = Image.open(tmp_path / "house.png")
    cx = door["x"] + door["w"] // 2
    r, g, b, a = house.getpixel((cx, door["y"] + door["h"] - 6))
    assert a == 255 and r + g + b < 3 * 140


def test_front_layer_transparent_inside_door(tmp_path):
    draw_house.build(tmp_path)
    door = json.loads((tmp_path / "house.json").read_text())["door"]
    front = Image.open(tmp_path / "house_front.png").getchannel("A")
    assert front.getbbox() is not None  # 문틀·문턱은 있다
    sill_top = door["y"] + door["h"] - 6  # 문턱 (외곽선 포함) (펫 앞에 와야 하는 부분)
    for py in range(door["y"], sill_top):
        for px in range(door["x"], door["x"] + door["w"]):
            if _inside_arch(door, px + 0.5, py + 0.5, inset=2):
                assert front.getpixel((px, py)) == 0, f"front layer covers the doorway at {(px, py)}"
    # 아치 바깥 모서리(문 사각형 안이지만 벽)는 앞 레이어가 덮어 펫 귀가 벽 위로 삐져나오지 않는다
    assert front.getpixel((door["x"] + 1, door["y"] + 1)) > 200
    assert front.getpixel((door["x"] + door["w"] - 2, door["y"] + 1)) > 200


# ---- 단계별 집 + 벌통 ----
ARTS = [(kind, stage) for kind in ("house", "hive") for stage in STAGES]


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("house")
    draw_house.build(out)
    return out


def _door(out, kind, stage):
    return json.loads((out / f"{kind}_{stage}.json").read_text())["door"]


def test_stage_files_exist_everywhere(built):
    for out in (built, ROOT / "ui" / "house"):
        for kind, stage in ARTS:
            for name in (f"{kind}_{stage}.png", f"{kind}_{stage}_front.png"):
                im = Image.open(out / name)
                assert im.size == (FRAME, FRAME) and im.mode == "RGBA", name
            assert set(_door(out, kind, stage)) == {"x", "y", "w", "h"}


def test_legacy_names_alias_teenager_house(built):
    assert _door(built, "house", "teenager") == json.loads((built / "house.json").read_text())["door"]
    for a, b in (("house.png", "house_teenager.png"), ("house_front.png", "house_teenager_front.png")):
        assert Image.open(built / a).tobytes() == Image.open(built / b).tobytes()


@pytest.mark.parametrize("kind,stage", ARTS)
def test_stage_art_sits_on_bottom_with_dark_doorway(built, kind, stage):
    art = Image.open(built / f"{kind}_{stage}.png")
    assert art.getpixel((0, 0))[3] == 0
    x0, y0, x1, y1 = art.getchannel("A").getbbox()
    assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1
    assert 121 <= y1 <= FRAME
    door = _door(built, kind, stage)
    assert door["x"] >= 1 and door["y"] >= 1
    assert door["x"] + door["w"] <= FRAME - 1 and door["y"] + door["h"] == 122  # 문 바닥 = 집 바닥
    r, g, b, a = art.getpixel((door["x"] + door["w"] // 2, door["y"] + door["h"] - 6))
    assert a == 255 and r + g + b < 3 * 140


@pytest.mark.parametrize("kind,stage", ARTS)
def test_stage_front_layer_open_in_doorway(built, kind, stage):
    door = _door(built, kind, stage)
    front = Image.open(built / f"{kind}_{stage}_front.png").getchannel("A")
    for py in range(door["y"], door["y"] + door["h"] - 6):
        for px in range(door["x"], door["x"] + door["w"]):
            if _inside_arch(door, px + 0.5, py + 0.5, inset=2):
                assert front.getpixel((px, py)) == 0, f"{kind}_{stage} front covers the doorway at {(px, py)}"
    assert front.getpixel((door["x"] + 1, door["y"] + 1)) > 200
    assert front.getpixel((door["x"] + door["w"] - 2, door["y"] + 1)) > 200


@pytest.mark.parametrize("kind,stage", ARTS)
def test_pet_fills_door_like_teenager(built, kind, stage):
    """앱 크기로 (집 128 × 집 배율, 문간 펫 DOOR_PET × 몸 배율) 펫이 문을 teenager 집과 같은 비율로 채운다."""
    teen = _door(built, "house", "teenager")
    teen_fill = draw_house.door_pet_size("teenager") / teen["w"]
    door = _door(built, kind, stage)
    # 벌통 입구는 꿀벌 몸 폭(프레임 안 몸 폭 × 몸 배율 0.8, 고양이 대비)에 맞춘 작은 구멍
    body = draw_house.HIVE_FILL[stage] if kind == "hive" else 1.0
    fill = draw_house.door_pet_size(stage) * body / door["w"]
    assert abs(fill / teen_fill - 1) < 0.04, (kind, stage, fill, teen_fill)
    assert abs(door["h"] / door["w"] - teen["h"] / teen["w"]) < 0.06


def test_house_scale_grows_with_stage():
    scales = [draw_house.HOUSE_SCALE[s] for s in STAGES]
    assert scales == sorted(scales) and draw_house.HOUSE_SCALE["teenager"] == 1.0
    assert draw_house.HOUSE_SCALE == {"baby": 0.55, "kinder": 0.68, "child": 0.82, "teenager": 1.0, "adult": 1.12}


def test_baby_pet_in_door_is_clearly_visible(built):
    """아기 집에서 고양이가 문 넓이의 절반 넘게 보인다 (예전: 128 집에 0.45배 → 겨우 보였다)."""
    door = _door(built, "house", "baby")
    sheet = Image.open(ROOT / "ui" / "pets" / "cat" / "sheet_baby.png")
    pet = sheet.crop((0, 10 * FRAME, FRAME, 11 * FRAME))
    layer = draw_house.pet_in_door(pet, door, draw_house.door_pet_size("baby"))
    x0, y0, x1, y1 = layer.getchannel("A").getbbox()
    assert (x1 - x0) >= 0.9 * door["w"] and (y1 - y0) >= 0.6 * door["h"]
