"""짱구 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🖍️ butt_dance·action_pose·roll_lazy·lie·flirt·snack)·모든 칸 여백·바닥선·
pet.json (rascal, 초코비, 해리보다 작은 꼬마, 짧은 영어 대사)·사람 모양 꼬마 (굵은 까만 눈썹, 아래가 넓은 감자 머리, 까만 머리, 빨간 반팔 티,
노란 반바지, 하얀 양말·운동화, 지팡이·목도리 없음, 앞모습 두 다리, 옆모습은 오른쪽)·특기 그림·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_jjanggu as dj
import draw_pets
import draw_wizard as wz
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "jjanggu"
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed",
            "butt_dance", "action_pose", "roll_lazy", "lie", "flirt", "snack")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """짱구만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"jjanggu": dj.JJANGGU})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "jjanggu" / f"sheet_{st}.png"), names(dj.JJANGGU).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_and_the_jjanggu_rows_at_the_end():
    assert draw_pets.SPECIES["jjanggu"] is dj.JJANGGU
    assert [o[0] for o in OPTIONAL_ANIMS[-6:]] == dj.JJANGGU_ANIMS
    assert names(dj.JJANGGU) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + dj.JJANGGU_ANIMS
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "manifest.rs").read_text(encoding="utf-8")
    assert 'JJANGGU_ANIMS: [&str; 6] = ["butt_dance", "action_pose", "roll_lazy", "lie", "flirt", "snack"]' in rs


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "jjanggu", dj.JJANGGU, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "jjanggu", dj.JJANGGU, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "jjanggu" / f"sheet_{st}.png", dj.JJANGGU)


def test_manifest_is_a_small_chocobi_loving_rascal(out):
    m = json.loads((out / "jjanggu" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "짱구"
    assert m["favorite_food"] == {"emoji": "🍪", "name": "초코비"}
    assert m["rascal"] is True and "broom_rider" not in m and "spells" not in m and "gamekeeper" not in m and "house" not in m
    for key in ("buri_buri", "action_beam", "chocobi", "lazy", "flirt", "shiro", "chatter"):
        assert m["speech"][key] and all(len(s) <= 16 for s in m["speech"][key]), key
    assert "buri buri~" in m["speech"]["buri_buri"] and "Action beam!" in m["speech"]["action_beam"] and "Hey pretty~" in m["speech"]["flirt"]
    assert m["stage_size"] == dict.fromkeys(STAGES, dj.SIZE) and dj.SIZE < 0.85
    a = m["animations"]
    assert a["flirt"]["front"] and a["snack"]["front"] and not a["action_pose"].get("loop", True) and "front" not in a["butt_dance"]
    her = json.loads((ROOT / "ui" / "pets" / "hermione" / "pet.json").read_text(encoding="utf-8"))
    assert her["speech"]["flattered"] and her["speech"]["brush_off"]


@pytest.mark.parametrize("st", STAGES)
def test_thick_brows_potato_head_red_tee_yellow_shorts_white_socks(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dj.BROW, 30)[: 64].sum() > 30, st  # 아주 굵은 까만 눈썹 (머리 위쪽 반 — 까만 머리와 같은 색 계열)
    assert near(im, dj.SHIRT, 40).sum() > 60, st  # 빨간 티
    assert near(im, dj.SHORTS, 40).sum() > 30, st  # 노란 반바지
    assert near(im, dj.SOCKS, 12).sum() > 20, st  # 하얀 양말·운동화
    assert near(im, wz.SCARF_GOLD, 12).sum() <= 4, st  # 목도리 없음 (반바지 가장자리 몇 px만)
    sil = np.asarray(im.getchannel("A")) > 128
    ys = np.nonzero(sil.any(axis=1))[0]
    head = [np.ptp(np.nonzero(sil[y])[0]) for y in range(ys.min(), ys.min() + int((ys.max() - ys.min()) * 0.55))]
    k = len(head)
    assert max(head[k // 2:]) >= max(head[: k // 3]) + 2, f"감자 머리: 아래 (볼)가 위보다 넓음 {st}"
    shoes = near(im, dj.SHOE, 12)[108:126]
    xs = np.nonzero(shoes)[1]
    assert (xs < 64).sum() > 5 and (xs > 64).sum() > 5, st  # 두 다리


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    for anim in ("walk", "idle", "butt_dance", "action_pose"):
        im = frame(out, st, anim, 0 if anim != "action_pose" else 1)
        a = np.asarray(im.getchannel("A")) > 128
        face = np.nonzero(near(im, dj.SKIN, 20))[1]
        assert face.size and face.mean() > np.nonzero(a)[1].mean(), (st, anim)


@pytest.mark.parametrize("st", STAGES)
def test_special_moves(out, st):
    # 🍑 엉덩이 춤: 노란 엉덩이가 머리보다 뒤 (왼쪽), 칸마다 씰룩 (다름)
    for i in range(4):
        im = frame(out, st, "butt_dance", i)
        yel = np.nonzero(near(im, dj.SHORTS, 40))[1]
        hair = np.nonzero(near(im, dj.HAIR, 20))[1]
        assert yel.size > 40 and yel.mean() < hair.mean(), (st, i)
    assert len({frame(out, st, "butt_dance", i).tobytes() for i in range(4)}) == 4
    # 🦸 빔: 마지막 칸에 노란 빔이 오른쪽 끝까지
    beam = np.nonzero(near(frame(out, st, "action_pose", 2), dj.BEAM, 40))[1]
    assert beam.size > 30 and beam.max() > 112, st
    # 😴 누움: 뒹굴은 몸이 가로로 넓고 낮음, 엎드림도 가로로 길게 — 둘 다 바닥에 닿음
    for anim, wide in (("roll_lazy", 0), ("lie", 70)):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        ys, xs = np.nonzero(a)
        assert np.ptp(xs) > max(wide, np.ptp(ys) if not wide else 0) and ys.max() >= 118, (st, anim)
    # 💕 쑥스러움: 진한 볼 + 하트 · 🍪 초코비 상자
    assert near(frame(out, st, "flirt", 1), dj.FLIRT_BLUSH, 30).sum() > 10, st
    assert near(frame(out, st, "snack", 0), dj.BOX, 30).sum() > 30 and near(frame(out, st, "snack", 0), dj.BOX_BAND, 30).sum() > 5, st


def test_the_build_reproduces_the_committed_jjanggu(out):
    assert_folder_matches_shipped(out / "jjanggu", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 맨 나중 (팔보다 위) — 팔이 머리를 덮으면 안 됨."""
    seen = []
    real = wz.dp.compose
    monkeypatch.setattr(wz.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    wz._LOOK["cur"] = dj.JJ_LOOK
    try:
        for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
            wz.wizard_back(r)
            parts = seen.pop()
            head = parts[-1]
            assert head.base == dj.SKIN and head.marks and head.marks[0][1] == dj.HAIR, "머리 (까만 뒤통수)가 맨 나중"
            assert not any(p.marks and p.marks[0][1] == dj.HAIR for p in parts[:-1])
    finally:
        wz._LOOK.pop("cur", None)
