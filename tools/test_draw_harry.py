"""해리포터 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🧹 broom + ✨ cast)·모든 칸 여백·바닥선·pet.json(broom_rider, spells, seeker, 사탕, grip)·
사람 모양 꼬마 마법사 (검은 로브, 진홍·금색 그리핀도르 목도리, 까만 머리, 둥근 까만 안경, 번개 흉터, 앞모습 두 다리, 옆모습은 오른쪽)·
빗자루 (자루가 몸 아래 가로로, 솔은 뒤)·주문 (지팡이 끝 = grip, 마지막 칸에 반짝)·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_harry as dh
import draw_wizard as wz
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "harry"
WIZARD = ["broom", "cast", "read", "raise_hand", "bag_walk", "bag_drag", "bag_stuff"]  # 🧙 마법사 줄 (sprite_spec 맨 끝)
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "cast")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """해리포터만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"harry": dh.HARRY})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "harry" / f"sheet_{st}.png"), names(dh.HARRY).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_and_the_wizard_rows_at_the_end():
    assert draw_pets.SPECIES["harry"] is dh.HARRY
    assert [o[0] for o in OPTIONAL_ANIMS[-15:-8]] == WIZARD  # 그 뒤는 🧔 해그리드 (test_draw_hagrid.py)
    assert wz.BROOM_ANIMS == ["broom"] and wz.SPELL_ANIMS == ["cast"]
    assert names(dh.HARRY) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup", "broom", "cast"]


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "harry", dh.HARRY, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "harry", dh.HARRY, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "harry" / f"sheet_{st}.png", dh.HARRY)


def test_manifest_is_a_playful_broom_riding_seeker_with_two_spells(out):
    m = json.loads((out / "harry" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "해리포터"
    assert m["favorite_food"] == {"emoji": "🍬", "name": "사탕"}
    assert m["personality"] == "playful" and m["broom_rider"] is True and m["seeker"] is True
    assert m["spells"] == ["expelliarmus", "leviosa"]
    assert "bookworm" not in m and "bag_cleaner" not in m and "juggler" not in m and "locomotion" not in m
    assert m["animations"]["broom"]["frames"] == 3 and m["animations"]["cast"]["frames"] == 3 and not m["animations"]["cast"]["loop"]
    for key in ("broom_up", "broom_land", "broom_away", "leviosa", "fizzle", "expelliarmus", "snitch", "got_it", "chatter"):
        assert m["speech"][key] and all(len(s) <= 20 for s in m["speech"][key]), key
    assert "Expelliarmus!" in m["speech"]["expelliarmus"] and "Got it!" in m["speech"]["got_it"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)
    for st in STAGES:
        assert set(m["grip"][st]) == {"cast"}, st
        assert len(m["grip"][st]["cast"]) == 3


@pytest.mark.parametrize("st", STAGES)
def test_black_robe_gryffindor_scarf_black_hair_round_glasses_and_a_scar(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, wz.ROBE, 24).sum() > 300, st
    assert near(im, wz.SCARF, 60).sum() > 12, st  # 진홍 목도리
    assert near(im, wz.SCARF_GOLD, 60).sum() > 8, st  # 금색 줄
    assert near(im, dh.HAIR, 20)[:60].sum() > 250, st  # 까만 머리 (위쪽)
    assert near(im, dh.HARRY_LOOK.skin, 30).sum() > 400, st  # 살색 얼굴
    face = near(im, dh.GLASSES, 30)[44:80, 36:92]
    assert face.sum() > 40, st  # 둥근 안경테 (얼굴 한가운데의 까만 고리)
    assert near(im, dh.SCAR, 50).sum() >= 3, st  # 번개 흉터
    shoes = near(im, wz.SHOE, 24)[114:126]
    xs = np.nonzero(shoes)[1]
    assert (xs < 64).sum() > 5 and (xs > 64).sum() > 5, st  # 구두 둘


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    """옆모습은 오른쪽을 본다: 얼굴 살색이 몸 가운데보다 오른쪽 (뒤통수는 까만 머리)."""
    for anim in ("walk", "run", "idle", "broom", "cast"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        body_x = np.nonzero(a)[1].mean()
        skin_x = np.nonzero(near(im, dh.HARRY_LOOK.skin, 30))[1].mean()
        hair_x = np.nonzero(near(im, dh.HAIR, 20))[1].mean()
        assert skin_x > body_x and skin_x > hair_x, (st, anim, skin_x, body_x, hair_x)


@pytest.mark.parametrize("st", STAGES)
def test_a_person_stands_on_two_legs_in_front_view(out, st):
    for anim in ("sit_front", "wave"):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        runs = [int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(119, 125)]
        assert max(runs) == 2, (st, anim, runs)


@pytest.mark.parametrize("st", STAGES)
def test_the_broom_runs_under_the_rider_with_bristles_behind(out, st):
    frames = []
    for i in range(3):
        im = frame(out, st, "broom", i)
        wood = np.nonzero(near(im, wz.BROOM_WOOD, 40))
        bristle = np.nonzero(near(im, wz.BRISTLE, 40))
        skin = np.nonzero(near(im, dh.HARRY_LOOK.skin, 30))
        assert wood[1].max() - wood[1].min() > 60, (st, i)  # 긴 자루가 가로로
        assert bristle[1].mean() < wood[1].mean() - 20, (st, i)  # 솔은 뒤 (왼쪽)
        assert skin[0].mean() < wood[0].mean() - 10, (st, i)  # 얼굴은 자루 위
        frames.append(np.asarray(im.convert("RGBA")))
    assert not np.array_equal(frames[0], frames[1]) and not np.array_equal(frames[1], frames[2]), "로브·목도리가 펄럭"


@pytest.mark.parametrize("st", STAGES)
def test_cast_flicks_the_wand_forward_and_sparkles(out, st):
    m = json.loads((out / "harry" / "pet.json").read_text(encoding="utf-8"))
    tips = m["grip"][st]["cast"]
    u = 2 if st == "adult" else 1
    assert tips[0][0] < tips[2][0], (st, tips)  # 뒤에서 앞으로
    assert tips[2][0] / u > 80, (st, tips)  # 마지막 칸은 앞으로 쭉
    im = frame(out, st, "cast", 2)
    assert near(im, wz.SPARK_EDGE, 60)[:, 80:].sum() > 5, st  # 끝에서 반짝


def test_the_build_reproduces_the_committed_harry(out):
    assert_folder_matches_shipped(out / "harry", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨."""
    seen = []
    real = wz.dp.compose
    monkeypatch.setattr(wz.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    wz._LOOK["cur"] = dh.HARRY_LOOK
    skin = dh.HARRY_LOOK.skin
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        wz.wizard_back(r)
        parts = seen.pop()
        assert parts[-1].base == skin, "머리가 맨 나중 (팔보다 위)"
        assert all(p.base != skin for p in parts[:-1])
    hip = (64.0, 92.0)
    wz.wizard(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=True)
    forced = seen.pop()
    wz.wizard(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=False)
    behind = seen.pop()
    order = lambda parts: [(p.base, p.outline, len(p.shapes)) for p in parts]  # noqa: E731
    assert order(forced) == order(behind), "뒷모습에선 arms_front를 무시하고 팔을 머리 뒤에 (머리를 맨 나중에)"
    assert forced[-1].base == skin
