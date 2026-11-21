"""슈퍼인간 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 💪 들기 일곱 + ✈️ 날기 넷)·모든 칸 여백·바닥선 (나는 칸은 바닥과 틈)·
pet.json(super_flier, juggler, 우유, grip)·사람 모양 꼬마 영웅 (파란 슈트, 빨간 망토·장화, 노란 방패 로고, 까만 머리, 가면 없는 살색 얼굴,
앞모습 두 다리, 옆모습은 오른쪽)·날기 (가로로 누운 몸, 머리는 앞, 망토는 뒤)·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_super as dsu
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "super"
JUGGLE = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]  # = Rust manifest::JUGGLE_ANIMS
SOAR = ["fly", "fly_fast", "fly_flip", "hover"]  # = Rust manifest::SOAR_ANIMS
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed",
            "lift", "hold", "carry", "throw", "strain", "present", "slump")
FLYING = ("fly", "fly_fast", "fly_flip")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """슈퍼인간만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"super": dsu.SUPER_HERO})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "super" / f"sheet_{st}.png"), names(dsu.SUPER_HERO).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_juggle_and_the_four_flight_rows_at_the_end():
    assert draw_pets.SPECIES["super"] is dsu.SUPER_HERO
    assert [o[0] for o in OPTIONAL_ANIMS[-25:-21]] == SOAR == dsu.SOAR_ANIMS and dsu.JUGGLE_ANIMS == JUGGLE
    assert names(dsu.SUPER_HERO) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + JUGGLE + SOAR


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "super", dsu.SUPER_HERO, STAGES, grounded=STANDING, hover=FLYING)
    assert_roll_fits_circle(out / "super", dsu.SUPER_HERO, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "super" / f"sheet_{st}.png", dsu.SUPER_HERO)


def test_manifest_is_a_playful_super_flier_that_lifts_and_loves_milk(out):
    m = json.loads((out / "super" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "슈퍼인간"
    assert m["favorite_food"] == {"emoji": "🥛", "name": "우유"}
    assert m["personality"] == "playful" and m["super_flier"] is True and m["juggler"] is True
    assert "web_slinger" not in m and "bike_rider" not in m and "tint" not in m and "taskbar_play" not in m and "locomotion" not in m
    assert m["animations"]["fly"]["frames"] == 3 and m["animations"]["fly_flip"]["frames"] == 4 and m["animations"]["hover"]["frames"] == 3
    for key in ("take_off", "rescue", "air_flip", "soft_land", "hero_lift", "chatter"):
        assert m["speech"][key] and all(len(s) <= 16 for s in m["speech"][key]), key
    assert "up, up and away!" in m["speech"]["take_off"] and "to the rescue!" in m["speech"]["rescue"] and "easy!" in m["speech"]["hero_lift"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)
    for st in STAGES:
        assert set(m["grip"][st]) == {"lift", "hold", "carry", "throw", "present"}, st


@pytest.mark.parametrize("st", STAGES)
def test_blue_suit_red_cape_and_boots_yellow_shield_black_hair_and_a_bare_face(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dsu.SUIT, 30).sum() > 80, st
    assert near(im, dsu.CAPE, 30).sum() > 150, st  # 망토 (양옆) + 장화
    assert near(im, dsu.LOGO_BG, 40)[50:118, 44:84].sum() > 5, st  # 가슴 방패 노란 바탕
    assert near(im, dsu.HAIR, 20)[:64].sum() > 250, st  # 까만 머리 (위쪽)
    assert near(im, dsu.SKIN, 30).sum() > 500, st  # 가면 없는 얼굴
    assert near(im, (255, 255, 255), 20).sum() < 60, st  # 하얀 가면 눈 없음 (눈동자 반짝이만)
    boots = near(im, dsu.BOOT, 30)[112:126]
    xs = np.nonzero(boots)[1]
    assert (xs < 64).sum() > 10 and (xs > 64).sum() > 10, st  # 빨간 장화 둘


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    """옆모습은 오른쪽을 본다: 얼굴 살색이 몸 가운데보다 오른쪽 (뒤통수는 까만 머리)."""
    for anim in ("walk", "run", "carry", "hover", "fly"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        body_x = np.nonzero(a)[1].mean()
        skin_x = np.nonzero(near(im, dsu.SKIN, 30))[1].mean()
        hair_x = np.nonzero(near(im, dsu.HAIR, 20))[1].mean()
        assert skin_x > body_x + 2 and skin_x > hair_x, (st, anim, skin_x, body_x, hair_x)


@pytest.mark.parametrize("st", STAGES)
def test_a_person_stands_on_two_legs_in_front_view(out, st):
    for anim in ("sit_front", "wave"):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        runs = [int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(118, 125)]
        assert max(runs) == 2, (st, anim, runs)


@pytest.mark.parametrize("st", STAGES)
def test_flying_lies_flat_head_first_with_the_cape_streaming_behind(out, st):
    for anim in ("fly", "fly_fast"):
        for i in range(3):
            im = frame(out, st, anim, i)
            a = np.asarray(im.getchannel("A")) > 128
            ys, xs = np.nonzero(a)
            assert xs.max() - xs.min() > 1.3 * (ys.max() - ys.min()), (st, anim, i)  # 가로로 누운 몸
            assert ys.max() <= 116, (st, anim, i)  # 바닥에서 떠 있음
            cape_x = np.nonzero(near(im, dsu.CAPE, 30))[1]
            skin_x = np.nonzero(near(im, dsu.SKIN, 30))[1]
            assert len(cape_x) > 60 and cape_x.mean() < skin_x.mean() - 15, (st, anim, i)  # 망토는 뒤, 얼굴·주먹은 앞
        frames = [np.asarray(frame(out, st, anim, i).convert("RGBA")) for i in range(3)]
        assert not np.array_equal(frames[0], frames[1]) and not np.array_equal(frames[1], frames[2]), "망토가 펄럭"


@pytest.mark.parametrize("st", STAGES)
def test_the_flip_turns_round_and_hover_floats_upright(out, st):
    tops = []
    for i in range(4):
        im = frame(out, st, "fly_flip", i)
        hair_y = np.nonzero(near(im, dsu.HAIR, 20))[0].mean()
        boot_y = np.nonzero(near(im, dsu.BOOT, 30))[0].mean() if near(im, dsu.BOOT, 30).any() else 64
        tops.append(hair_y < boot_y)
    assert tops[0] and not tops[2], (st, tops)  # 반 바퀴 뒤엔 머리가 아래
    for i in range(3):
        im = frame(out, st, "hover", i)
        hair_y = np.nonzero(near(im, dsu.HAIR, 20))[0].mean()
        boot_y = np.nonzero(near(im, dsu.BOOT, 30))[0].mean()
        assert hair_y < boot_y - 30, (st, i)  # 똑바로 섬
        assert np.nonzero(np.asarray(im.getchannel("A")) > 128)[0].max() < 125, (st, i)  # 바닥에서 살짝 뜸


def test_the_build_reproduces_the_committed_super(out):
    assert_folder_matches_shipped(out / "super", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨 (예전 다른 펫들에서도 고쳤던 실수)."""
    seen = []
    real = dsu.dp.compose
    monkeypatch.setattr(dsu.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        dsu.super_back(r)
        parts = seen.pop()
        assert parts[-1].base == dsu.SKIN, "머리가 맨 나중 (팔보다 위)"
        assert all(p.base != dsu.SKIN for p in parts[:-1])
    hip = (64.0, 92.0)
    dsu.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=True)
    forced = seen.pop()
    dsu.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=False)
    behind = seen.pop()
    order = lambda parts: [(p.base, p.outline, len(p.shapes)) for p in parts]  # noqa: E731
    assert order(forced) == order(behind), "뒷모습에선 arms_front를 무시하고 팔을 머리 뒤에 (머리를 맨 나중에)"
    assert forced[-1].base == dsu.SKIN
