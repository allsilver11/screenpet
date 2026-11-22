"""볼드모트 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🧹 broom + ✨ cast)·모든 칸 여백·바닥선·pet.json(broom_rider, spells stupefy, dark_lord, 생선, grip)·
사람 모양 꼬마 마법사 (까만 로브 + 초록 안감, 목도리 없음, 창백한 대머리, 붉은 눈, 납작한 콧구멍, 하얀 지팡이, 앞모습 두 다리, 옆모습은 오른쪽)·
빗자루·주문 (지팡이 끝 = grip)·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_voldemort as dv
import draw_wizard as wz
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "voldemort"
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "cast")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """볼드모트만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"voldemort": dv.VOLDEMORT})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "voldemort" / f"sheet_{st}.png"), names(dv.VOLDEMORT).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_broom_and_cast():
    assert draw_pets.SPECIES["voldemort"] is dv.VOLDEMORT
    assert names(dv.VOLDEMORT) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + wz.PRANK_ANIMS + ["broom", "cast"]


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "voldemort", dv.VOLDEMORT, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "voldemort", dv.VOLDEMORT, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "voldemort" / f"sheet_{st}.png", dv.VOLDEMORT)


def test_manifest_is_a_broom_riding_dark_lord_with_stupefy(out):
    m = json.loads((out / "voldemort" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "볼드모트"
    assert m["favorite_food"] == {"emoji": "🐟", "name": "생선"}
    assert m["broom_rider"] is True and m["dark_lord"] is True and m["spells"] == ["stupefy"]
    assert "seeker" not in m and "patronus" not in m and "locomotion" not in m
    assert m["juggler"] is True and m["pranks"] == ["hoist", "hide"]
    for key in ("broom_up", "broom_land", "broom_away", "stupefy", "nagini", "hehehe", "duel_call", "chatter"):
        assert m["speech"][key] and all(len(s) <= 20 for s in m["speech"][key]), key
    assert "Stupefy!" in m["speech"]["stupefy"] and "hehehe" in m["speech"]["hehehe"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)
    for st in STAGES:
        assert set(m["grip"][st]) == {"cast", "lift", "hold", "carry", "throw", "present"} and len(m["grip"][st]["cast"]) == 3, st


@pytest.mark.parametrize("st", STAGES)
def test_pale_bald_red_eyed_noseless_in_a_black_robe_without_a_scarf(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dv.ROBE, 20).sum() > 300, st  # 까만 로브
    assert near(im, wz.SCARF, 50)[88:].sum() == 0 and near(im, wz.SCARF_GOLD, 50).sum() == 0, st  # 목도리 없음 (붉은 눈 아래)
    y0 = im.getchannel("A").getbbox()[1]
    crown = near(im, dv.SKIN, 24)[y0 + 3 : y0 + 15]
    assert crown.sum() > 150, st  # 정수리까지 창백한 대머리 (까만 머리카락 없음)
    assert near(im, dv.EYE_RED, 70)[40:90].sum() > 10, st  # 붉은 눈
    assert near(im, dv.SLIT, 30)[50:100, 40:90].sum() >= 3, st  # 납작한 콧구멍
    shoes = near(im, wz.SHOE, 24)[114:126]
    xs = np.nonzero(shoes)[1]
    assert (xs < 64).sum() > 5 and (xs > 64).sum() > 5, st  # 구두 둘


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right_and_the_wand_is_white(out, st):
    for anim in ("walk", "idle", "cast"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        eyes = np.nonzero(near(im, dv.EYE_RED, 70))[1]
        assert eyes.mean() > np.nonzero(a)[1].mean(), (st, anim)
    assert near(frame(out, st, "cast", 2), dv.WAND_TIP, 20).sum() > 3, st


@pytest.mark.parametrize("st", STAGES)
def test_the_broom_runs_under_him(out, st):
    im = frame(out, st, "broom", 0)
    wood = np.nonzero(near(im, wz.BROOM_WOOD, 40))
    skin = np.nonzero(near(im, dv.SKIN, 24))
    assert wood[1].max() - wood[1].min() > 60 and skin[0].mean() < wood[0].mean() - 10, st


def test_the_build_reproduces_the_committed_voldemort(out):
    assert_folder_matches_shipped(out / "voldemort", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨."""
    seen = []
    real = wz.dp.compose
    monkeypatch.setattr(wz.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    wz._LOOK["cur"] = dv.VOLDEMORT_LOOK
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        wz.wizard_back(r)
        parts = seen.pop()
        assert parts[-1].base == dv.SKIN, "머리가 맨 나중 (팔보다 위)"
        assert all(p.base != dv.SKIN for p in parts[:-1])



def test_he_never_smiles(monkeypatch, tmp_path):
    """웃지 않음: 모든 칸의 입은 일자·비웃음·찡그림 (smile·open·big·grin 없음), 웃는 ^^·질끈 눈도 없음."""
    mouths, eyes = set(), set()
    real_m, real_e = wz.mouth_ops, draw_pets.eye
    monkeypatch.setattr(wz, "mouth_ops", lambda kind, *a: (mouths.add(kind), real_m(kind, *a))[1])
    monkeypatch.setattr(draw_pets, "eye", lambda kind, *a, **k: (eyes.add(kind), real_e(kind, *a, **k))[1])
    monkeypatch.setattr(draw_pets, "SPECIES", {"voldemort": dv.VOLDEMORT})
    monkeypatch.setattr(draw_pets, "_RENDERED", {})  # 그려 둔 칸 캐시를 비워 새로 그리게
    draw_pets.build(tmp_path)
    assert mouths and not mouths & {"smile", "open", "big", "grin"}, mouths
    assert eyes and "happy" not in eyes and "tight" not in eyes, eyes
