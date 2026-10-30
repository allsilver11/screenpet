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


# ---- 단계별 집 + 벌통 + 귀신의 집 ----
ARTS = [(kind, stage) for kind in ("house", "hive", "haunted", "xmas") for stage in STAGES]


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
    # 벌통 입구는 꿀벌 몸 폭(프레임 안 몸 폭 × 몸 배율 0.8, 고양이 대비)에 맞춘 작은 구멍,
    # 귀신의 집 문은 호박·유령 몸 폭(평균)에 맞춘다
    body = {"hive": draw_house.HIVE_FILL, "haunted": draw_house.HAUNTED_FILL, "xmas": draw_house.XMAS_FILL}.get(kind, {}).get(stage, 1.0)
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


def _body_width(pet_id, stage):
    """sit_front 첫 프레임의 몸 폭 (프레임 px) × 몸 배율."""
    frame = draw_house.sit_frame(pet_id, stage)
    x0, _, x1, _ = frame.getchannel("A").getbbox()
    return (x1 - x0) * draw_house.body_scale(pet_id, stage)


def test_kinds_include_haunted():
    assert draw_house.KINDS == ("house", "hive", "haunted", "xmas")
    assert set(draw_house.DOORS) == set(draw_house.KINDS)


@pytest.mark.parametrize("pet", ["pumpkin", "ghost"])
@pytest.mark.parametrize("stage", STAGES)
def test_haunted_door_fits_pumpkin_and_ghost(built, pet, stage):
    """실제 시트로 잰 호박·유령 몸 폭이 귀신의 집 문을 teenager 집의 고양이처럼 (±12 %) 채운다."""
    if draw_house.sit_frame(pet, stage) is None:
        pytest.skip(f"no {pet} sheet")
    teen = _door(built, "house", "teenager")
    teen_fill = draw_house.door_pet_size("teenager") * _body_width("cat", "teenager") / teen["w"]
    door = _door(built, "haunted", stage)
    fill = draw_house.door_pet_size(stage) * _body_width(pet, stage) / door["w"]
    # 고양이 몸 폭은 단계마다 조금 달라 고양이 자신도 기준 단계와 비교한다
    cat_fill = draw_house.door_pet_size(stage) * _body_width("cat", stage) / _door(built, "house", stage)["w"]
    assert abs(fill / cat_fill - 1) < 0.12, (pet, stage, fill, cat_fill, teen_fill)


def test_haunted_house_is_purple_with_warm_glow(built):
    """귀신의 집: 보라·회청 벽이 많고, 따뜻한 주황 불빛(다락 창·잭오랜턴)이 있다 — 무섭지 않게 파스텔."""
    for stage in STAGES:
        art = Image.open(built / f"haunted_{stage}.png").convert("RGBA")
        px = [tuple(art.getpixel((x, y))) for y in range(FRAME) for x in range(FRAME) if art.getpixel((x, y))[3] == 255]
        purple = sum(1 for r, g, b, _ in px if b > g + 15 and b >= r - 10 and r + g + b > 300)
        warm = sum(1 for r, g, b, _ in px if r > 230 and 120 < g < 225 and b < 140)
        assert purple > 0.25 * len(px), (stage, purple, len(px))
        assert warm > 40, (stage, warm)


def test_haunted_art_grows_with_stage(built):
    """아기 오두막은 작고 어른 집은 더 크다 (그림 안 불투명 넓이)."""
    areas = [sum(1 for a in Image.open(built / f"haunted_{s}.png").getchannel("A").tobytes() if a > 128) for s in STAGES]
    assert areas[0] < areas[2] < areas[4], areas


# ---- 크리스마스 벽돌집 (루돌프·산타) ----
def _rgba(im):
    b = im.convert("RGBA").tobytes()
    return [tuple(b[k:k + 4]) for k in range(0, len(b), 4)]


@pytest.mark.parametrize("stage", STAGES)
def test_xmas_door_fits_rudolph_and_santa(built, stage):
    """루돌프(넓은 귀)는 문을 고양이처럼 (±30 %) 채운다. 산타는 몸 배율 1.65로 커져서 (사용자 요청 "1.5배만") 문을 꽉 채운다 —
    고양이보다 넓지만 1.6배는 넘지 않는다 (문 그림은 그대로라 산타에겐 아늑하게 꼭 맞는 문)."""
    if draw_house.sit_frame("rudolph", stage) is None or draw_house.sit_frame("santa", stage) is None:
        pytest.skip("no christmas sheets")
    door = _door(built, "xmas", stage)
    cat_fill = draw_house.door_pet_size(stage) * _body_width("cat", stage) / _door(built, "house", stage)["w"]
    rudolph, santa = (draw_house.door_pet_size(stage) * _body_width(p, stage) / door["w"] for p in ("rudolph", "santa"))
    assert abs(rudolph / cat_fill - 1) < 0.3, (stage, rudolph, cat_fill)
    assert 1.0 <= santa / cat_fill < 1.6, (stage, santa, cat_fill)


def test_xmas_house_is_red_brick_with_snow_and_warm_windows(built):
    for stage in STAGES:
        art = Image.open(built / f"xmas_{stage}.png").convert("RGBA")
        px = [p for p in _rgba(art) if p[3] == 255]
        brick = sum(1 for r, g, b, _ in px if r > 180 and 70 < g < 130 and 60 < b < 120)
        snow = sum(1 for r, g, b, _ in px if r > 240 and g > 240 and b > 240)
        warm = sum(1 for r, g, b, _ in px if r > 245 and 190 < g < 245 and b < 200)
        assert brick > 100 and snow > 0.2 * len(px) and warm > 8, (stage, brick, snow, warm, len(px))


def test_xmas_smoke_point_sits_on_the_chimney(built):
    """<art>.json "smoke": 굴뚝 입구 (불투명 굴뚝 바로 위) — 앱이 xmas_smoke.png를 여기서 피워 올린다."""
    puff = Image.open(built / "xmas_smoke.png")
    assert puff.size == (32, 32) and puff.mode == "RGBA"
    dark = [p for p in _rgba(puff) if p[3] > 200 and max(p[:3]) < 170]
    assert len(dark) > 150  # 짙은 회색 연기
    for stage in STAGES:
        meta = json.loads((built / f"xmas_{stage}.json").read_text())
        sx, sy = meta["smoke"]["x"], meta["smoke"]["y"]
        art = Image.open(built / f"xmas_{stage}.png").getchannel("A")
        assert 4 <= sy <= 60 and 60 <= sx <= 120, (stage, sx, sy)
        assert art.getpixel((round(sx), round(sy) + 6)) == 255  # 바로 아래가 굴뚝
    for kind in ("house", "hive", "haunted"):
        assert "smoke" not in json.loads((built / f"{kind}_teenager.json").read_text())
