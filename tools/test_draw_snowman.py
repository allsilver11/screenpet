"""눈사람 시트 검사: 모든 동작·칸 안·pet.json·두 덩이/세 덩이·단계별 소품·밥그릇 없음·뒷모습 벽 타기·배 미끄럼·눈구름·손 음식·선물."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_props
import draw_snowman
from conftest import assert_eat_has_no_bowl, assert_frames_inside_cell
from sprite_spec import ANIMS, FRAME, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
GROUNDED = {"walk", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed", "hungry", "eat", "lookup"}


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    draw_pets.write_species(draw_snowman.SNOWMAN, root / "snowman")
    return root


def names():
    return [a for a, *_ in draw_pets.rows_for(draw_snowman.SNOWMAN)]


def frame(out, anim, col=0, st="teenager"):
    r = names().index(anim)
    return sheet_128(out / "snowman" / f"sheet_{st}.png").convert("RGBA").crop((col * FRAME, r * FRAME, (col + 1) * FRAME, (r + 1) * FRAME))


def count(im, col, tol=24):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return int(((a[..., 3] > 200) & (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol)).sum())


def test_registered_with_every_anim():
    sp = draw_pets.SPECIES["snowman"]
    assert sp is draw_snowman.SNOWMAN
    assert names() == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"]
    assert all(a in sp.poses for a in names())


def test_every_frame_fits_the_cell(out):
    assert_frames_inside_cell(out / "snowman", draw_snowman.SNOWMAN, STAGES, grounded=GROUNDED, ground_y=116)


@pytest.mark.parametrize("st", ["baby", "teenager", "adult"])
def test_nothing_is_clipped_by_the_cell_before_fitting(st):
    """_fit이 안쪽으로 밀어도 칸 밖으로 잘린 그림은 돌아오지 않는다 — 민 적이 없어야 한다."""
    sp = draw_snowman.SNOWMAN
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        with draw_pets.view(scale=sp.scale * draw_pets._ST["fit"], lift=draw_pets.LIFT + 1.7 * draw_pets.OUTLINE * (draw_pets._ST["line"] - 1)):
            for anim, n, *_ in draw_pets.rows_for(sp):
                for c in range(n):
                    x0, y0, x1, y1 = draw_snowman.RAW_POSES[anim](sp, c).getchannel("A").getbbox()
                    assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, (x0, y0, x1, y1))


def test_manifest(out):
    m = json.loads((out / "snowman" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "눈사람" and m["personality"] == "playful" and m["house"] == "xmas" and m["hop_share"] == 0.5
    assert m["favorite_food"] == {"emoji": "🍧", "name": "빙수"} and m["snow_cloud"] is True
    assert m["speech"]["chatter"] == ["brrr~", "snow!", "♪"] and m["speech"]["tickle"] == ["hehe!"] and m["speech"]["grumpy"] == ["hmph!"]
    assert m["speech"]["hungry"] and m["speech"]["full"] and m["speech"]["level_up"]
    assert set(m["animations"]) == {a for a, *_ in ANIMS} | {"climb", "slide", "lookup"} and m["stages"] == STAGES
    assert "spin" not in m["animations"]["roll"] and "locomotion" not in m
    assert m["animations"]["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6}


def test_two_balls_when_small_three_when_grown():
    for lvl, st in enumerate(STAGES):
        assert len(draw_snowman.SN_STAGE[lvl]["balls"]) + 1 == (2 if st in ("baby", "kinder", "child") else 3), st


def test_accessories_grow_with_age(out):
    scarf = [count(frame(out, "sit_front", 0, st), draw_snowman.SCARF) for st in STAGES]
    pail = [count(frame(out, "sit_front", 0, st), draw_snowman.PAIL) for st in STAGES]
    holly = [count(frame(out, "sit_front", 0, st), draw_snowman.HOLLY) for st in STAGES]
    buttons = [draw_snowman.SN_STAGE[lvl]["buttons"] for lvl in range(len(STAGES))]
    assert scarf[0] == 0 and all(s >= 40 for s in scarf[1:]), scarf
    assert pail[0] == pail[1] == 0 and all(p >= 45 for p in pail[2:]), pail
    assert holly[3] == 0 and holly[4] >= 5, holly
    assert buttons == [0, 1, 2, 3, 3]  # 단추가 늘어난다
    assert count(frame(out, "sit_front", 0, "baby").crop((30, 96, 98, FRAME)), draw_snowman.COAL, 16) == 0
    assert count(frame(out, "sit_front", 0, "teenager").crop((30, 72, 98, FRAME)), draw_snowman.COAL, 16) >= 15


def test_readable_on_white(out):
    """눈사람은 흰색이지만 파랑 회색 외곽선과 연한 파랑 그늘이 있어 밝은 바탕에서도 보인다."""
    for st in STAGES:
        im = frame(out, "sit_front", 0, st)
        assert count(im, draw_snowman.SNOW_LINE, 40) >= 200, st
        assert count(im, draw_snowman.SNOW_SHADE, 14) >= 100, st


def test_eat_has_no_bowl_and_the_mouth_sits_over_the_food(out):
    for st in STAGES:
        im = frame(out, "eat", 0, st)
        assert_eat_has_no_bowl(out / "snowman" / f"sheet_{st}.png", draw_snowman.SNOWMAN, frames=1, most=60)
        assert count(im.crop((50, 70, 79, 97)), draw_pets.MOUTH_IN, 40) >= 6, st


def test_belly_slide_instead_of_rolling(out):
    for st in STAGES:
        frames = [frame(out, "roll", c, st) for c in range(2)]
        for im in frames:
            bbox = im.getchannel("A").getbbox()
            assert bbox[3] >= 118 and bbox[2] - bbox[0] >= 85 and bbox[3] - bbox[1] <= 88, (st, bbox)  # 바닥에 길게 눕는다
        assert frames[0].tobytes() != frames[1].tobytes()


def test_hops_squash_and_stretch(out):
    for st in STAGES:
        crouch, air = (frame(out, "hop", c, st).getchannel("A").getbbox() for c in (0, 1))
        assert crouch[3] - crouch[1] < air[3] - air[1], st
        assert crouch[2] - crouch[0] > air[2] - air[0] - 18, st


def test_climbs_with_the_back_to_us(out):
    """뒷모습: 얼굴이 없다 (석탄 눈·당근·볼 없음, 단추도 앞에 있어 안 보임). 나뭇가지 손끝만 머리 옆·위로, 발은 번갈아."""
    for st in STAGES:
        frames = [frame(out, "climb", c, st) for c in range(4)] + [frame(out, "slide", c, st) for c in range(2)]
        for k, im in enumerate(frames):
            assert count(im, draw_snowman.COAL, 16) == 0, (st, k)
            assert count(im, draw_snowman.CARROT, 40) == 0, (st, k)
            assert count(im, draw_snowman.SNOW, 12) >= 800, (st, k)
            assert count(im.crop((0, 0, FRAME, 60)), draw_snowman.TWIG, 50) >= 3 or k in (1, 3), (st, k)
        low = [(count(f.crop((0, 92, 44, FRAME)), draw_snowman.SNOW, 12), count(f.crop((84, 92, FRAME, FRAME)), draw_snowman.SNOW, 12)) for f in frames[:4]]
        assert len({f.tobytes() for f in frames[:4]}) >= 3, st
        assert low[0] != low[2], (st, low)


def test_lookup_is_happy(out):
    for st in STAGES:
        ups = [frame(out, "lookup", c, st) for c in range(3)]
        assert len({f.tobytes() for f in ups}) == 3, st


def test_snow_cloud_art_and_ui():
    cloud = draw_snowman.draw_cloud()
    assert cloud.size == (draw_snowman.CLOUD_W, draw_snowman.CLOUD_H)
    x0, y0, x1, y1 = cloud.getchannel("A").getbbox()
    assert x0 >= 1 and y0 >= 1 and x1 <= cloud.width - 1 and y1 <= cloud.height - 1, (x0, y0, x1, y1)
    assert count(cloud, draw_snowman.CLOUD_LINE, 60) >= 40  # 밝은 바탕에서도 보인다
    assert (ROOT / "ui" / "effects" / "snow_cloud.png").is_file()
    index = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
    css = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    assert 'id="snowcloud" hidden' in index
    assert "#snowcloud[hidden]" in css and "display: none" in css.split("#snowcloud[hidden]")[1][:40]  # 안 보이면 애니메이션도 멈춘다
    assert "if (sheetImg && render.cloud) placeCloud" in js  # 프레임마다 도는 일은 구름이 있을 때만


def test_favorite_food_icon():
    assert "hand_shaved_ice" in draw_props.HAND_NAMES
    assert (ROOT / "ui" / "effects" / "hand_shaved_ice.png").is_file()
    assert '"snowman" => "hand_shaved_ice"' in (ROOT / "crates" / "screenpet-core" / "src" / "food.rs").read_text(encoding="utf-8")
