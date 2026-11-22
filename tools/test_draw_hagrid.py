"""해그리드 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🧔 dragon·sidecar·sidecar_fly·hug·hug_walk·pat·egg·offer)·모든 칸 여백·바닥선·
pet.json (broom_rider + 용, gamekeeper, 덩치 1.4배, 수박, 손 자리)·사람 모양 큰 꼬마 (갈색 코트, 덥수룩한 수염·머리, 분홍 우산, 목도리 없음,
앞모습 두 다리, 옆모습은 오른쪽)·용과 사이드카 그림·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_hagrid as dg
import draw_pets
import draw_wizard as wz
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "hagrid"
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "hug", "hug_walk",
            "pat", "egg", "offer", "sidecar")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """해그리드만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"hagrid": dg.HAGRID})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "hagrid" / f"sheet_{st}.png"), names(dg.HAGRID).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_and_the_hagrid_rows_at_the_end():
    assert draw_pets.SPECIES["hagrid"] is dg.HAGRID
    assert [o[0] for o in OPTIONAL_ANIMS[-9:-1]] == dg.HAGRID_ANIMS  # 그 뒤는 🛡️ 무쇠인간 suit_up
    assert names(dg.HAGRID) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + wz.PRANK_ANIMS + dg.HAGRID_ANIMS


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "hagrid", dg.HAGRID, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "hagrid", dg.HAGRID, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "hagrid" / f"sheet_{st}.png", dg.HAGRID)


def test_manifest_is_a_big_dragon_riding_gamekeeper(out):
    m = json.loads((out / "hagrid" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "해그리드"
    assert m["favorite_food"] == {"emoji": "🍉", "name": "수박"}
    assert m["broom_rider"] is True and m["gamekeeper"] is True and "spells" not in m
    assert m["juggler"] is True and m["pranks"] == ["hoist", "hide"]
    for key in ("broom_up", "broom_land", "vroom", "hop_in", "cuddle", "pat", "hatch", "rock_cake", "chatter"):
        assert m["speech"][key] and all(len(s) <= 20 for s in m["speech"][key]), key
    assert "Yer a wizard!" in m["speech"]["chatter"] and "Blimey!" in m["speech"]["chatter"] and "Rock cake?" in m["speech"]["rock_cake"]
    assert m["stage_size"] == dict.fromkeys(STAGES, dg.SIZE) and 1.3 <= dg.SIZE / 0.85 <= 1.5
    assert m["frame_sizes"] == {"teenager": 256, "adult": 256}
    for st in STAGES:
        g = m["grip"][st]
        assert set(g) == {"hug", "hug_walk", "sidecar", "sidecar_fly", "egg", "lift", "hold", "carry", "throw", "present"}, st
        assert [len(g[a]) for a in ("hug", "hug_walk", "sidecar", "sidecar_fly", "egg")] == [2, 4, 3, 2, 4], st
        px = m["frame_sizes"].get(st, 128)
        assert all(x > px * 0.62 for x, _ in g["sidecar"]), "사이드카는 앞쪽"


@pytest.mark.parametrize("st", STAGES)
def test_bushy_beard_and_hair_brown_coat_pink_umbrella_no_scarf(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dg.COAT, 24).sum() > 100, st  # 갈색 코트 (아기는 수염 덩어리 아래로 조금)
    assert near(im, dg.HAIR, 20).sum() > 500, st  # 머리·수염 한 덩어리 (같은 색)
    assert near(im, dg.UMBRELLA, 50).sum() + near(frame(out, st, "walk", 0), dg.UMBRELLA, 50).sum() > 8, st  # 분홍 우산 (아기는 작음)
    assert near(im, wz.SCARF, 40).sum() == 0 and near(im, wz.SCARF_GOLD, 40).sum() == 0, st
    face = near(im, dg.HAGRID_LOOK.skin, 24)
    assert face.sum() > 30, st  # 다정한 눈 둘레 얼굴이 빼꼼
    shoes = near(im, wz.SHOE, 24)[112:126]
    xs = np.nonzero(shoes)[1]
    assert (xs < 64).sum() > 5 and (xs > 64).sum() > 5, st  # 구두 둘


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    for anim in ("walk", "idle", "hug", "pat"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        nose = np.nonzero(near(im, dg.NOSE, 30))[1]
        assert nose.size and nose.mean() > np.nonzero(a)[1].mean(), (st, anim)


@pytest.mark.parametrize("st", STAGES)
def test_the_dragon_and_the_sidecar_bike_carry_him(out, st):
    for i in range(3):
        im = frame(out, st, "dragon", i)
        green = np.nonzero(near(im, dg.DRAGON, 40))
        coat = np.nonzero(near(im, dg.COAT, 24))
        assert green[1].max() - green[1].min() > 80, (st, i)  # 머리부터 꼬리까지 긴 용
        assert coat[0].mean() < green[0].mean(), (st, i)  # 해그리드는 등 위
    wings = [near(frame(out, st, "dragon", i), (150, 198, 120), 40).sum() for i in range(3)]
    assert len(set(wings)) == 3, "날개가 퍼덕임"
    for anim, n in (("sidecar", 3), ("sidecar_fly", 2)):
        for i in range(n):
            im = frame(out, st, anim, i)
            tub = np.nonzero(near(im, dg.TUB, 30))
            assert tub[1].mean() > 75 and near(im, dg.TYRE, 30).sum() > 60, (st, anim, i)


def test_the_build_reproduces_the_committed_hagrid(out):
    assert_folder_matches_shipped(out / "hagrid", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨 (뒷모습은 머리 덩어리만)."""
    seen = []
    real = wz.dp.compose
    monkeypatch.setattr(wz.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    wz._LOOK["cur"] = dg.HAGRID_LOOK
    try:
        for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
            wz.wizard_back(r)
            parts = seen.pop()
            skin = dg.HAGRID_LOOK.skin
            assert parts[-1].base == skin, "머리가 맨 나중 (팔보다 위)"
            assert all(p.base != skin for p in parts[:-1])
    finally:
        wz._LOOK.pop("cur", None)
