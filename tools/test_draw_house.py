import json
from pathlib import Path

from PIL import Image

import draw_house
from sprite_spec import FRAME

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
