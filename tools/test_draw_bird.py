"""새(제비) 시트 검사: 모든 동작·투명 여백·pet.json(빠른 나는 동물)·갈라진 꼬리·파닥이는 날개·콕콕 쪼아 먹기·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_bird
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, FRAME, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "bird"
FLYING = ("walk", "run")
PERCHED = ("idle", "look", "sleep", "annoyed", "lookup", "sit_front", "eat", "hungry")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"bird": draw_bird.BIRD})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "bird" / f"sheet_{st}.png"), names(draw_bird.BIRD).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def test_registered_with_its_own_poses():
    assert draw_pets.SPECIES["bird"] is draw_bird.BIRD
    assert names(draw_bird.BIRD) == [a for a, *_ in ANIMS] + ["lookup"]
    assert all(a in draw_bird.BIRD.poses for a in names(draw_bird.BIRD))


def test_every_frame_is_drawn_inside_the_cell(out):
    # 나는 그림(walk·run)은 바닥에서 떠 있고, 앉은 그림은 발이 바닥(창 윗면)에 닿는다
    assert_frames_inside_cell(out / "bird", draw_bird.BIRD, STAGES, free=("roll",), grounded=PERCHED, hover=FLYING)


def test_roll_ball_fits_rotation_circle(out):
    assert_roll_fits_circle(out / "bird", draw_bird.BIRD, STAGES)


def test_manifest_is_a_swift_flyer_that_loves_seeds(out):
    m = json.loads((out / "bird" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "새"
    assert m["favorite_food"] == {"emoji": "🌾", "name": "모이"}
    assert m["locomotion"] == "fly" and m["swift"] is True
    assert "sting" not in m["animations"] and "house" not in m
    assert {"chirp!", "whoosh!"} <= set(m["speech"]["chatter"])
    # 유령(둥실 떠다니는 나는 동물)보다 훨씬 빠르다 — 나는 동물 중 가장 빠름 (코어 brain_swift_tests도 본다)
    ghost = json.loads((ROOT / "ui" / "pets" / "ghost" / "pet.json").read_text(encoding="utf-8"))
    assert m["walk_speed"] >= 1.5 * ghost["walk_speed"]
    assert set(m["stage_size"].values()) == {0.8}


def test_the_build_reproduces_the_committed_bird(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "bird").iterdir())
    for name in names_:
        assert (out / "bird" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", ["child", "teenager", "adult"])
@pytest.mark.parametrize("anim", ["idle", "walk"])
def test_swallow_tail_is_forked(out, st, anim):
    """제비 꼬리: 꽁지 끝 쪽 세로줄을 보면 두 갈래 (불투명 구간이 둘)."""
    a = alpha(frame(out, st, anim, 0))
    xs = np.nonzero(a.any(axis=0))[0]
    def runs(x):
        col = a[:, x].astype(int)
        return int(((np.diff(col) == 1).sum()) + col[0])
    # 왼쪽(꼬리) 끝에서 몇 줄 안에 두 갈래가 보인다
    assert max(runs(x) for x in range(xs[0], xs[0] + 14)) >= 2, (st, anim)


def test_baby_tail_is_still_a_stub(out):
    """아기는 갈래가 거의 없는 짧은 꽁지 — 몸 길이에 비해 꼬리가 크게 짧다."""
    def tail_len(st):
        a = alpha(frame(out, st, "idle", 0))
        xs = np.nonzero(a.any(axis=0))[0]
        return (xs[-1] - xs[0]) / a.any(axis=1).sum()
    assert tail_len("baby") < 0.8 * tail_len("adult")


@pytest.mark.parametrize("st", STAGES)
def test_wings_flap_up_and_down_while_flying(out, st):
    """walk(날기) 0 = 날개 위로, 2 = 날개 아래로: 날개 색 덩어리(외곽선 섞인 가장자리는 빼고)의 높이가 다르다."""
    def wing_y(i):
        a = np.asarray(frame(out, st, "walk", i).convert("RGBA")).astype(int)
        m = (np.abs(a[..., :3] - np.array(draw_bird.wing_color(st))).sum(axis=2) < 12) & (a[..., 3] > 250)
        assert m.sum() > 30, (st, i)
        return np.median(np.nonzero(m)[0])
    assert wing_y(2) - wing_y(0) >= 5, (st, wing_y(0), wing_y(2))
    assert wing_y(2) > wing_y(1) >= wing_y(0) and wing_y(1) == wing_y(3)


@pytest.mark.parametrize("st", STAGES)
def test_eat_pecks_down_to_the_bowl_and_back_up(out, st):
    """콕콕: 0·2 = 부리 끝이 그릇 밥 더미(프레임 y 84) 근처, 1·3 = 고개를 들었다 (부리가 더 위)."""
    def beak_bottom(i):
        a = np.asarray(frame(out, st, "eat", i).convert("RGBA")).astype(int)
        m = (np.abs(a[..., :3] - np.array(draw_bird.BEAK)).sum(axis=2) < 24) & (a[..., 3] > 200)
        m[:, :54] = m[:, 75:] = False  # 가운데 부리만 (날개 끝 외곽선이 섞인 한두 점은 뺀다)
        assert m.sum() > 3, (st, i)
        ys, xs = np.nonzero(m)
        assert abs(xs.mean() - 64) <= 4, (st, i, xs.mean())
        return ys.max()
    down, up = beak_bottom(0), beak_bottom(1)
    assert 78 <= down <= 90, (st, down)
    assert down - up >= 6, (st, down, up)
    assert abs(beak_bottom(2) - down) <= 2


@pytest.mark.parametrize("st", STAGES)
def test_eat_has_no_bowl(out, st):
    assert_eat_has_no_bowl(out / "bird" / f"sheet_{st}.png", draw_bird.BIRD)


def test_red_throat_from_child_and_goggles_only_on_the_adult(out):
    def count(st, col, anim="sit_front"):
        a = np.asarray(frame(out, st, anim, 0).convert("RGBA")).astype(int)
        return int(((np.abs(a[..., :3] - np.array(col)).sum(axis=2) < 20) & (a[..., 3] > 200)).sum())
    assert count("baby", draw_bird.THROAT) == 0
    assert count("child", draw_bird.THROAT) > 10 and count("adult", draw_bird.THROAT) > 10
    assert count("adult", draw_bird.GOGGLE) > 10 and count("teenager", draw_bird.GOGGLE) == 0
