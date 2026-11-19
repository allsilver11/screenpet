"""거미인간 시트 검사: 줄 순서 (필수 21 + climb·slide + 특기 넷)·모든 칸 여백·바닥선·pet.json(web_slinger, 파리, grip)·
빨강·파랑 옷과 가슴의 까만 거미 무늬·앞모습 다리 여덟·줄 끝 손 자리·영웅 착지·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_spider as ds
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "spider"
WEB = ["shoot", "web_swing", "hero_land", "web_pull"]  # = Rust manifest::WEB_ANIMS
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "shoot",
            "hero_land", "web_pull")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """거미인간만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"spider": ds.SPIDER})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "spider" / f"sheet_{st}.png"), names(ds.SPIDER).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_and_the_four_web_rows_at_the_end():
    assert draw_pets.SPECIES["spider"] is ds.SPIDER
    assert [o[0] for o in OPTIONAL_ANIMS[-4:]] == WEB == ds.WEB_ANIMS
    assert names(ds.SPIDER) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + WEB


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "spider", ds.SPIDER, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "spider", ds.SPIDER, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "spider" / f"sheet_{st}.png", ds.SPIDER)


def test_manifest_is_a_playful_web_slinger_that_loves_flies(out):
    m = json.loads((out / "spider" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "거미인간"
    assert m["favorite_food"] == {"emoji": "🪰", "name": "파리"}
    assert m["personality"] == "playful" and m["web_slinger"] is True
    assert "juggler" not in m and "tint" not in m
    assert m["animations"]["shoot"] == {"row": len(ANIMS) + 3, "frames": 3, "fps": 10, "loop": False}
    assert m["animations"]["hero_land"]["loop"] is False and "front" not in m["animations"]["web_swing"]
    for key in ("thwip", "web", "web_swing", "hero_land", "gotcha"):
        assert m["speech"][key] and all(len(s) <= 16 for s in m["speech"][key]), key
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)


@pytest.mark.parametrize("st", STAGES)
def test_grip_marks_the_hand_at_the_end_of_the_line(out, st):
    """줄 끝 손 자리: 쏠 때는 몸 앞 멀리, 그네는 머리 위, 끌어올 때는 몸 앞 (pet.json grip — 코어 GripTable)."""
    m = json.loads((out / "spider" / "pet.json").read_text(encoding="utf-8"))
    px = m.get("frame_sizes", {}).get(st, 128) / 128
    g = {a: [[x / px, y / px] for x, y in cells] for a, cells in m["grip"][st].items()}
    assert set(g) == {"shoot", "web_swing", "web_pull"}
    assert [len(g[a]) for a in ("shoot", "web_swing", "web_pull")] == [3, 2, 4]
    assert all(x > 100 for x, _ in g["shoot"][1:]), g["shoot"]
    assert all(y < 20 for _, y in g["web_swing"]), g["web_swing"]
    assert all(x > 90 for x, _ in g["web_pull"]), g["web_pull"]


@pytest.mark.parametrize("st", STAGES)
def test_red_and_blue_suit_with_a_black_spider_on_the_chest(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, ds.RED).sum() > 600 and near(im, ds.BLUE).sum() > 600, st
    chest = near(im, ds.EMBLEM, 30)[70:118, 48:80]
    assert chest.sum() > 25, (st, int(chest.sum()))
    side = frame(out, st, "walk", 0)
    assert near(side, ds.RED).sum() > 500 and near(side, ds.BLUE).sum() > 500, st


@pytest.mark.parametrize("st", ["child", "teenager", "adult"])
def test_eight_legs_stand_on_the_floor_in_front_view(out, st):
    """앞모습: 바닥께 한 줄에 발 여덟이 따로따로."""
    a = np.asarray(frame(out, st, "sit_front", 0).getchannel("A")) > 128
    best = 0
    for y in range(114, 126):
        row = a[y]
        best = max(best, int(np.sum(row[1:] & ~row[:-1])))
    assert best >= 8, (st, best)


@pytest.mark.parametrize("st", STAGES)
def test_hero_landing_plants_a_fist_in_front_and_stretches_a_leg_back(out, st):
    a = np.asarray(frame(out, st, "hero_land", 1).getchannel("A")) > 128
    ys, xs = np.nonzero(a)
    assert xs.min() < 26, (st, xs.min())  # 뒤로 쭉 뻗은 다리
    floor = a[112:124]
    assert floor[:, 92:].any(), st  # 앞에 짚은 주먹
    assert ys.min() > 30, st  # 쪼그려 낮다


def test_the_build_reproduces_the_committed_spider(out):
    assert_folder_matches_shipped(out / "spider", SHIPPED)
