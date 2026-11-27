"""디멘터 시트 검사: 필수 21줄 + lookup (벽 타기 없음 — 나는 동물)·모든 칸 여백·떠 있음 (바닥에 닿지 않음)·pet.json(locomotion fly, dementor, 빙수)·
사람 치비가 아닌 떠다니는 후드 망토 (얼굴 없는 캄캄한 후드 속, 앙상한 회색 해골 손, 들쭉날쭉한 아랫단이 펄럭임)·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_dementor as dd
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "dementor"
FLOATING = ("walk", "run", "idle", "look", "sit_front", "wave", "tilt", "heart", "hungry", "annoyed", "level_up", "fall", "dragged")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """디멘터만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"dementor": dd.DEMENTOR})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "dementor" / f"sheet_{st}.png"), names(dd.DEMENTOR).index(anim), i)


def near(im, col, tol=30):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_only_the_basic_rows():
    assert draw_pets.SPECIES["dementor"] is dd.DEMENTOR
    assert names(dd.DEMENTOR) == [a for a, *_ in ANIMS] + ["lookup"], "벽 타기·빗자루 없음 (올려다보기는 모두)"


def test_every_frame_is_drawn_inside_the_cell_and_floats(out):
    assert_frames_inside_cell(out / "dementor", dd.DEMENTOR, STAGES, hover=FLOATING)
    assert_roll_fits_circle(out / "dementor", dd.DEMENTOR, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "dementor" / f"sheet_{st}.png", dd.DEMENTOR)


def test_the_hood_top_is_never_cut_off_as_it_grows(out):
    # 사용자: 친구 디멘터가 어느 크기 이상 되니까 맨 위가 잘린다 — 큰 단계의 후드(와 솟을 때 번쩍 든 손)가 칸 위를 넘어 그릴 때 잘리고,
    # 잘린 채로 1px 여백 안에 밀려 들어와 납작한 윗면이 됐었음. 모든 칸의 맨 윗줄은 위에서 2px 이상 떨어져야 함
    for st in STAGES:
        sheet = sheet_128(out / "dementor" / f"sheet_{st}.png")
        for anim in names(dd.DEMENTOR):
            for i in range(4):
                a = np.asarray(cell(sheet, names(dd.DEMENTOR).index(anim), i).convert("RGBA"))[..., 3] > 100
                rows = np.where(a.any(axis=1))[0]
                if len(rows):
                    assert rows[0] >= 2, f"{st} {anim} {i}: top row {rows[0]} ({int(a[rows[0]].sum())} px wide)"


def test_manifest_is_a_flying_dementor(out):
    m = json.loads((out / "dementor" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "디멘터"
    assert m["locomotion"] == "fly" and m["dementor"] is True
    assert m["favorite_food"] == {"emoji": "🍧", "name": "빙수"}
    for key in ("broom_rider", "spells", "grip", "patronus", "dark_lord"):
        assert key not in m, key
    assert m["speech"]["repelled"] and m["speech"]["chatter"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.95)


@pytest.mark.parametrize("st", STAGES)
def test_a_faceless_hood_bony_hands_and_a_ragged_cloak(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dd.CLOAK, 16).sum() > 800, st  # 망토
    void = near(im, dd.VOID, 12)
    assert void[:70].sum() > 150, st  # 후드 속 캄캄한 어둠 (위쪽)
    assert near(im, (255, 224, 202), 40).sum() == 0, st  # 살색 얼굴 없음
    bone = np.nonzero(near(im, dd.BONE, 30))
    assert len(bone[1]) > 10 and (bone[1] < 64).sum() > 3 and (bone[1] > 64).sum() > 3, st  # 해골 손 둘
    a = np.asarray(im.getchannel("A")) > 128
    bottom = max(y for y in range(128) if a[y].any())
    runs = max(int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(bottom - 6, bottom))
    assert runs >= 3, (st, runs)  # 들쭉날쭉 찢어진 아랫단 (조각 끝이 여럿)
    assert bottom <= 116, (st, bottom)  # 바닥보다 떠 있음


@pytest.mark.parametrize("st", STAGES)
def test_the_hem_flutters_and_side_views_face_right(out, st):
    walk = [np.asarray(frame(out, st, "walk", i).convert("RGBA")) for i in range(4)]
    assert not np.array_equal(walk[0], walk[1]), st
    im = frame(out, st, "walk", 0)
    void_x = np.nonzero(near(im, dd.VOID, 12))[1].mean()
    body_x = np.nonzero(np.asarray(im.getchannel("A")) > 128)[1].mean()
    assert void_x > body_x, st


def test_the_build_reproduces_the_committed_dementor(out):
    assert_folder_matches_shipped(out / "dementor", SHIPPED)
