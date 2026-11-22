"""헤르미온느 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🧹 broom + ✨ cast + 📖 read·🙋 raise_hand + 👜 bag_walk·bag_drag·bag_stuff)·
모든 칸 여백·바닥선·pet.json(broom_rider, spells, bookworm, bag_cleaner, 사과, grip)·사람 모양 꼬마 마법사 (부스스한 갈색 긴 머리, 검은 로브,
그리핀도르 목도리, 앞모습 두 다리, 옆모습은 오른쪽)·책 읽기 (책장 넘김)·손들기 (손이 머리 위)·가방은 시트에 없음 (앱이 grip에 그림 — 끌 때 손은 몸 뒤)·
뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_hermione as dhe
import draw_wizard as wz
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "hermione"
BOOK = ["read", "raise_hand"]  # = Rust manifest::BOOK_ANIMS
BAG = ["bag_walk", "bag_drag", "bag_stuff"]  # = Rust manifest::BAG_ANIMS
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "cast",
            "read", "bag_walk", "bag_drag", "bag_stuff")
PURPLE = (128, 70, 170)  # 구슬 가방 보라 (시트엔 없어야 함)


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """헤르미온느만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"hermione": dhe.HERMIONE})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "hermione" / f"sheet_{st}.png"), names(dhe.HERMIONE).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def manifest(out):
    """만든 pet.json."""
    return json.loads((out / "hermione" / "pet.json").read_text(encoding="utf-8"))


def test_registered_with_climb_broom_cast_book_and_bag_rows():
    assert draw_pets.SPECIES["hermione"] is dhe.HERMIONE
    assert [o[0] for o in OPTIONAL_ANIMS[-14:-12]] == BOOK == dhe.BOOK_ANIMS and [o[0] for o in OPTIONAL_ANIMS[-12:-9]] == BAG == dhe.BAG_ANIMS
    assert names(dhe.HERMIONE) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + wz.PRANK_ANIMS + ["broom", "cast"] + BOOK + BAG


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "hermione", dhe.HERMIONE, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "hermione", dhe.HERMIONE, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "hermione" / f"sheet_{st}.png", dhe.HERMIONE)


def test_manifest_is_a_friendly_bookworm_with_a_bag_and_two_spells(out):
    m = manifest(out)
    assert m["name"] == "헤르미온느"
    assert m["favorite_food"] == {"emoji": "🍎", "name": "사과"}
    assert m["personality"] == "friendly" and m["broom_rider"] is True and m["bookworm"] is True and m["bag_cleaner"] is True
    assert m["spells"] == ["leviosa", "reparo"]
    assert "seeker" not in m and "locomotion" not in m
    assert m["juggler"] is True and m["pranks"] == ["hoist", "hide"]
    a = m["animations"]
    assert (a["read"]["frames"], a["raise_hand"]["frames"], a["bag_walk"]["frames"], a["bag_drag"]["frames"], a["bag_stuff"]["frames"]) == (4, 2, 4, 4, 2)
    for key in ("broom_up", "broom_land", "broom_away", "leviosa", "fizzle", "reparo", "i_know", "study", "tidy_up", "all_clean", "chatter"):
        assert m["speech"][key] and all(len(s) <= 20 for s in m["speech"][key]), key
    assert "It's Levi-O-sa!" in m["speech"]["leviosa"] and "I know!" in m["speech"]["i_know"] and "Phew! All clean!" in m["speech"]["all_clean"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)
    for st in STAGES:
        assert set(m["grip"][st]) == {"cast", "bag_walk", "bag_drag", "bag_stuff", "lift", "hold", "carry", "throw", "present"}, st


@pytest.mark.parametrize("st", STAGES)
def test_bushy_brown_hair_black_robe_and_the_gryffindor_scarf(out, st):
    im = frame(out, st, "sit_front", 0)
    hair = near(im, dhe.HAIR, 40)
    assert hair.sum() > 1200, st  # 부스스한 큰 머리
    ys, xs = np.nonzero(hair)
    head_w = xs.max() - xs.min()
    skin_xs = np.nonzero(near(im, dhe.HERMIONE_LOOK.skin, 30))[1]
    assert head_w > (skin_xs.max() - skin_xs.min()) + 10, st  # 머리숱이 얼굴보다 훨씬 넓음
    assert ys.max() > np.nonzero(near(im, dhe.HERMIONE_LOOK.skin, 30))[0].mean(), st  # 어깨까지 내려옴
    assert near(im, wz.ROBE, 24).sum() > 200, st
    assert near(im, wz.SCARF, 60).sum() > 12 and near(im, wz.SCARF_GOLD, 60).sum() > 8, st


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    for anim in ("walk", "run", "broom", "cast", "read", "bag_walk"):
        im = frame(out, st, anim, 0)
        skin_x = np.nonzero(near(im, dhe.HERMIONE_LOOK.skin, 30))[1].mean()
        hair_x = np.nonzero(near(im, dhe.HAIR, 40))[1].mean()
        assert skin_x > hair_x, (st, anim, skin_x, hair_x)


@pytest.mark.parametrize("st", STAGES)
def test_reading_turns_a_page_and_the_hand_goes_up(out, st):
    pages = [near(frame(out, st, "read", i), dhe.PAGE, 20).sum() for i in range(4)]
    assert min(pages) > 15, (st, pages)  # 펼친 책이 보임
    assert pages[3] > pages[0] and pages[2] != pages[0], (st, pages)  # 넘기는 책장
    m = manifest(out)
    for i in range(2):
        im = frame(out, st, "raise_hand", i)
        skin = np.nonzero(near(im, dhe.HERMIONE_LOOK.skin, 30))
        hair_top = np.nonzero(near(im, dhe.HAIR, 40))[0].min()
        assert skin[0].min() < hair_top + 12, (st, i)  # 손이 머리 꼭대기 높이까지
    assert m["animations"]["raise_hand"]["front"] is True


@pytest.mark.parametrize("st", STAGES)
def test_the_bag_is_not_in_the_sheet_and_the_drag_hand_is_behind(out, st):
    m = manifest(out)
    u = 2 if st == "adult" else 1
    for anim in BAG:
        for i, (x, y) in enumerate(m["grip"][st][anim]):
            im = frame(out, st, anim, i)
            assert near(im, PURPLE, 60).sum() == 0, (st, anim, i)  # 가방은 앱이 그림
            a = np.asarray(im.getchannel("A")) > 128
            body_x = np.nonzero(a)[1].mean()
            if anim == "bag_drag":
                assert x / u < body_x, (st, i, x, body_x)  # 끄는 손은 몸 뒤
                assert y / u > 80, (st, i, y)
            if anim == "bag_stuff":
                assert x / u > body_x, (st, i, x, body_x)  # 넣는 손은 몸 앞


def test_the_build_reproduces_the_committed_hermione(out):
    assert_folder_matches_shipped(out / "hermione", SHIPPED)


def test_back_view_hair_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리(부스스한 머리숱 포함)가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨."""
    seen = []
    real = wz.dp.compose
    monkeypatch.setattr(wz.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    wz._LOOK["cur"] = dhe.HERMIONE_LOOK
    skin = dhe.HERMIONE_LOOK.skin
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        wz.wizard_back(r)
        parts = seen.pop()
        assert parts[-1].base == skin, "머리가 맨 나중 (팔보다 위)"
        assert all(p.base != skin for p in parts[:-1])


@pytest.mark.parametrize("st", STAGES)
def test_climbing_hands_peek_out_beside_the_hair(out, st):
    """뒷모습 벽 타기: 부푼 머리에 가려도 벽 짚은 손은 양옆으로 보임."""
    im = frame(out, st, "climb", 1)
    xs = np.nonzero(near(im, dhe.HERMIONE_LOOK.skin, 30))[1]
    assert (xs < 50).sum() > 8 and (xs > 78).sum() > 8, st
