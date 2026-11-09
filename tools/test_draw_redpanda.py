"""레서판다 시트 검사: 모든 동작·투명 여백·pet.json(잘 놀라는 장난꾸러기, 대나무잎, 깜짝·벽 타기)·
고리 무늬 꼬리·흰 얼굴 무늬(볼·눈썹 점·귀 테두리)·짙은 다리·깜짝 자세(뒷발로 서서 두 팔 번쩍, 앉은 모습보다 크다)·
들어 올리면 두 팔 번쩍·창백한 아기·어른 대나무잎 핀·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_redpanda as rp
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, cell, count_near
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "redpanda"
STANDING = ("walk", "run", "idle", "look", "sleep", "annoyed", "lookup", "sit_front", "wave", "tilt", "eat", "hungry", "heart",
            "stretch", "tickle", "level_up", "surprise")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"redpanda": rp.RED_PANDA})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "redpanda" / f"sheet_{st}.png"), names(rp.RED_PANDA).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def near(im, col, tol=24):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def blobs(mask):
    """4-이웃으로 이어진 덩어리 수 (작은 점 3칸 이하는 빼고)."""
    seen = np.zeros_like(mask, dtype=bool)
    n = 0
    for y, x in zip(*np.nonzero(mask)):
        if seen[y, x]:
            continue
        stack, size = [(y, x)], 0
        seen[y, x] = True
        while stack:
            cy, cx = stack.pop()
            size += 1
            for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                ny, nx = cy + dy, cx + dx
                if 0 <= ny < mask.shape[0] and 0 <= nx < mask.shape[1] and mask[ny, nx] and not seen[ny, nx]:
                    seen[ny, nx] = True
                    stack.append((ny, nx))
        n += size > 3
    return n


def test_registered_with_surprise_climb_and_lookup():
    assert draw_pets.SPECIES["redpanda"] is rp.RED_PANDA
    assert OPTIONAL_ANIMS[-1] == ("surprise", 3, 8, False, True)  # = Rust manifest::OPTIONAL_ANIMS 맨 끝, 앞모습 한 번
    assert names(rp.RED_PANDA) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup", "surprise"]


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "redpanda", rp.RED_PANDA, STAGES, grounded=STANDING)


def test_manifest_is_a_playful_jumpy_climber_that_loves_bamboo(out):
    m = json.loads((out / "redpanda" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "레서판다"
    assert m["favorite_food"] == {"emoji": "🎋", "name": "대나무잎"}
    assert m["personality"] == "playful"
    assert "locomotion" not in m and "house" not in m
    assert m["animations"]["surprise"] == {"row": len(ANIMS) + 3, "frames": 3, "fps": 8, "loop": False, "front": True}
    assert "climb" in m["animations"] and "slide" in m["animations"]
    assert m["speech"]["surprise"] == ["!!", "eep!"]
    assert set(m["stage_size"].values()) == {0.88}


def test_the_build_reproduces_the_committed_red_panda(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "redpanda").iterdir())
    for name in names_:
        assert (out / "redpanda" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", STAGES)
def test_rusty_red_fur_with_dark_legs(out, st):
    """몸은 녹슨 주황빨강, 다리·발은 짙은 적갈색 (앉은 앞모습의 맨 아래 발 줄)."""
    im = frame(out, st, "sit_front", 0)
    area = int(alpha(im).sum())
    assert count_near(im, [rp.fur("rust", st)], 24) > 0.25 * area, st
    feet = near(im, rp.fur("limb", st))[112:122]
    assert feet.sum() > 40, (st, int(feet.sum()))
    r, g, b = rp.fur("rust", st)
    assert r > g + 50 and g > b, "주황빨강"


@pytest.mark.parametrize("st", STAGES)
def test_white_face_marks_on_cheeks_brows_and_ear_rims(out, st):
    """앞모습 얼굴: 흰 볼(눈 아래 양쪽)·흰 눈썹 점(눈 위 양쪽)·흰 귀 테두리(머리 꼭대기 양쪽)."""
    im = frame(out, st, "sit_front", 0)
    white = near(im, rp.fur("white", st), 18)
    ys, xs = np.nonzero(alpha(im))
    top = ys.min()
    ey = int(np.median(np.nonzero(near(im, draw_pets.EYE, 30)[: top + 70])[0]))  # 눈 높이
    for lo, hi in ((0, 64), (64, 128)):
        assert white[ey + 4: ey + 26, lo:hi].sum() > 25, (st, "cheek", lo)
        assert white[top + 8: ey - 4, lo:hi].sum() > 6, (st, "brow / ear rim", lo)
        assert white[top: top + 16, lo:hi].sum() > 4, (st, "ear rim", lo)


@pytest.mark.parametrize("st", ["kinder", "child", "teenager", "adult"])
@pytest.mark.parametrize("anim", ["walk", "idle", "sit_front"])
def test_long_bushy_tail_with_darker_rings(out, st, anim):
    """꼬리: 짙은 고리가 둘 이상 떨어져 보인다 (꼬리 쪽 — 옆모습은 왼쪽, 앞모습은 오른쪽)."""
    im = frame(out, st, anim, 0)
    ring = near(im, rp.fur("ring", st), 26)
    assert blobs(ring) >= 2, (st, anim, blobs(ring))
    tail = near(im, rp.fur("tail", st), 22)
    assert tail.sum() > 40, (st, anim)


def height(im):
    ys = np.nonzero(alpha(im))[0]
    return int(ys.max() - ys.min())


@pytest.mark.parametrize("st", STAGES)
def test_surprise_stands_up_tall_with_both_arms_raised(out, st):
    """깜짝: 앞모습, 뒷발로 서서 앉은 모습(idle·sit_front)보다 크고, 두 팔(짙은 색)이 머리 높이 위쪽 양옆에 번쩍."""
    idle, sit = frame(out, st, "idle", 0), frame(out, st, "sit_front", 0)
    for i in (1, 2):
        im = frame(out, st, "surprise", i)
        assert height(im) >= height(idle) + 8 and height(im) >= height(sit) + 6, (st, i, height(im), height(idle), height(sit))
        ys = np.nonzero(alpha(im))[0]
        top = ys.min()
        limb = near(im, rp.fur("limb", st), 26)[top: top + 22]
        assert limb[:, :52].sum() > 12 and limb[:, 76:].sum() > 12, (st, i, int(limb[:, :52].sum()), int(limb[:, 76:].sum()))
    assert frame(out, st, "surprise", 0).tobytes() != frame(out, st, "surprise", 1).tobytes()


@pytest.mark.parametrize("st", ["baby", "teenager", "adult"])
def test_picked_up_it_throws_both_arms_up_too(out, st):
    for i in (0, 1):
        im = frame(out, st, "dragged", i)
        ys = np.nonzero(alpha(im))[0]
        limb = near(im, rp.fur("limb", st), 26)[ys.min(): ys.min() + 22]
        assert limb[:, :52].sum() > 8 and limb[:, 76:].sum() > 8, (st, i)


def test_baby_is_paler_and_fluffier(out):
    baby, adult = rp.fur("rust", "baby"), rp.fur("rust", "adult")
    assert sum(baby) > sum(adult) + 60, (baby, adult)


def test_bamboo_leaf_pin_only_on_the_adult(out):
    for anim in ("sit_front", "idle"):
        assert count_near(frame(out, "adult", anim, 0), [rp.LEAF], 20) > 8, anim
        assert count_near(frame(out, "teenager", anim, 0), [rp.LEAF], 20) == 0, anim


@pytest.mark.parametrize("st", STAGES)
def test_eat_has_no_bowl(out, st):
    assert_eat_has_no_bowl(out / "redpanda" / f"sheet_{st}.png", rp.RED_PANDA)


def test_climb_is_a_back_view_with_a_ringed_tail(out):
    for st in ("child", "adult"):
        im = frame(out, st, "climb", 0)
        assert count_near(im, [rp.fur("ring", st)], 26) > 10, st
