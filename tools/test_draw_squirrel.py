"""다람쥐 시트 검사: 모든 동작·투명 여백·pet.json(벽 타는 날쌘 장난꾸러기, 도토리)·적갈색 털과 크림색 배·
고양이보다 훨씬 큰 복슬 꼬리 (아기는 작게 → 어른은 큰 깃털 꼬리)·뒷모습 벽 타기·두 앞발로 쥐고 오물오물 먹기·어른 도토리 모자·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_squirrel
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle, cell, count_near
from sprite_spec import ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "squirrel"
CAT = ROOT / "ui" / "pets" / "cat"
STANDING = ("walk", "idle", "look", "sleep", "annoyed", "lookup", "sit_front", "wave", "tilt", "eat", "hungry", "heart")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"squirrel": draw_squirrel.SQUIRREL})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "squirrel" / f"sheet_{st}.png"), names(draw_squirrel.SQUIRREL).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def count(im, col, tol=20):
    return count_near(im, [col], tol)


def mask(im, col, tol=20):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) <= tol) & (a[..., 3] > 200)


def test_registered_with_its_own_poses_and_climbs():
    assert draw_pets.SPECIES["squirrel"] is draw_squirrel.SQUIRREL
    assert names(draw_squirrel.SQUIRREL) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"]
    assert all(a in draw_squirrel.SQUIRREL.poses for a in names(draw_squirrel.SQUIRREL))


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "squirrel", draw_squirrel.SQUIRREL, STAGES, free=("roll",), grounded=STANDING)


def test_roll_is_a_curled_ball_that_fits_the_spin_circle(out):
    assert_roll_fits_circle(out / "squirrel", draw_squirrel.SQUIRREL, STAGES)


def test_manifest_is_a_quick_curious_climber_that_loves_acorns(out):
    m = json.loads((out / "squirrel" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "다람쥐"
    assert m["favorite_food"] == {"emoji": "🌰", "name": "도토리"}
    assert m["personality"] == "playful"
    assert "climb" in m["animations"] and "slide" in m["animations"], "고양이처럼 벽·창 옆면을 탄다"
    assert m["animations"]["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6}  # 뒷모습 (front 아님)
    assert 0 < m["hop_share"] < 1, "걷다가 가끔 깡총깡총"
    assert m["walk_speed"] > json.loads((CAT / "pet.json").read_text(encoding="utf-8"))["walk_speed"], "고양이보다 날쌔다"
    assert "locomotion" not in m and "house" not in m
    assert "spin" in m["animations"]["roll"], "꼬리로 몸을 감싼 공이 데굴데굴 구른다"
    assert m["speech"]["chatter"] and all(s.isascii() for s in m["speech"]["chatter"])
    assert set(m["stage_size"].values()) == {draw_squirrel.SQUIRREL.stage_size["teenager"]}


def test_the_build_reproduces_the_committed_squirrel(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "squirrel").iterdir())
    for name in names_:
        assert (out / "squirrel" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim", ["sit_front", "idle", "walk"])
def test_reddish_brown_fur_and_cream_belly(out, st, anim):
    im = frame(out, st, anim, 0)
    area = int(alpha(im).sum())
    fur = count(im, draw_squirrel.fur_color(st)) + count(im, draw_squirrel.tail_color(st))
    cream = count(im, draw_squirrel.CREAM, 14)
    assert fur > 0.3 * area, (st, anim, fur, area)
    assert cream > (0.04 if anim == "sit_front" else 0.02) * area, (st, anim, cream, area)  # 옆모습은 배가 다리·머리에 가려 덜 보인다
    r, g, b = draw_squirrel.fur_color("teenager")
    assert r - g >= 60 and g - b >= 30, "적갈색 (빨강 > 초록 > 파랑)"


def _cat_idle():
    m = json.loads((CAT / "pet.json").read_text(encoding="utf-8"))
    return {st: cell(sheet_128(CAT / f"sheet_{st}.png"), m["animations"]["idle"]["row"], 0) for st in STAGES}


@pytest.mark.parametrize("st", ["child", "teenager", "adult"])
def test_big_fluffy_tail_dwarfs_the_cats(out, st):
    """옆모습 쉬기: 몸 뒤(왼쪽)로 솟은 큰 꼬리 — 꼬리 색 칸이 고양이 꼬리 쪽(x < 44) 전체의 두 배를 넘고, 머리 꼭대기보다 높이 솟는다."""
    im = frame(out, st, "idle", 0)
    tail = mask(im, draw_squirrel.tail_color(st))
    cat_tail_side = int(alpha(_cat_idle()[st])[:, :44].sum())
    assert tail.sum() > 2 * cat_tail_side, (st, int(tail.sum()), cat_tail_side)
    ys, xs = np.nonzero(tail)
    assert xs.mean() < 50, "꼬리는 몸 뒤"
    assert ys.min() < 45, ys.min()


def test_baby_tail_is_smaller_and_grows_into_a_big_plume(out):
    """꼬리 칸 / 몸 전체: 아기 < 청소년 < 어른 (아기는 작고 어른은 큰 깃털 꼬리)."""
    def share(st):
        im = frame(out, st, "idle", 0)
        return mask(im, draw_squirrel.tail_color(st)).sum() / alpha(im).sum()
    assert share("baby") < share("teenager") < share("adult"), [round(share(s), 3) for s in STAGES]
    assert share("adult") > 1.4 * share("baby")


@pytest.mark.parametrize("st", ["kinder", "teenager", "adult"])
def test_ear_tufts_poke_up_above_the_head(out, st):
    """귀 끝 술: 앞모습 윗선이 두 봉우리 (왼쪽 x 40~60 · 오른쪽 x 76~92의 귀 끝)이고 그 사이(머리 꼭대기, x 60~76)가 5px 넘게 움푹하다."""
    a = alpha(frame(out, st, "sit_front", 0))
    tops = [int(np.nonzero(a[:, x])[0][0]) if a[:, x].any() else 128 for x in range(128)]
    dip = max(tops[60:76])
    left, right = min(tops[40:60]), min(tops[76:92])
    assert left < dip - 5 and right < dip - 5, (st, left, dip, right)


@pytest.mark.parametrize("st", STAGES)
def test_climb_is_a_back_view_with_the_tail(out, st):
    """뒷모습: 얼굴(눈·크림 배)이 안 보이고 꼬리가 보이며, 네 장이 모두 다르다 (발을 번갈아)."""
    frames = [frame(out, st, "climb", i) for i in range(4)]
    for im in frames:
        area = alpha(im).sum()
        assert count(im, draw_pets.EYE, 20) < 6
        assert count(im, draw_squirrel.CREAM, 14) < 0.03 * area
        assert mask(im, draw_squirrel.tail_color(st)).sum() > 0.12 * area
    assert len({im.tobytes() for im in frames}) == 4
    for i in range(2):
        assert count(frame(out, st, "slide", i), draw_pets.EYE, 20) < 6


@pytest.mark.parametrize("st", STAGES)
def test_eat_holds_the_morsel_in_both_paws_and_nibbles(out, st):
    """0 = 몸을 숙여 두 앞발로 그릇에서 집기, 1·2·3 = 앉아서 두 앞발로 입 앞에 쥐고 오물오물 (부스러기가 바뀐다)."""
    def morsel(i):
        m = mask(frame(out, st, "eat", i), draw_squirrel.MORSEL, 16)
        assert m.sum() > 6, (st, i)
        ys, xs = np.nonzero(m)
        assert abs(xs.mean() - 64) <= 5, (st, i, xs.mean())
        return ys.mean()
    down, up = morsel(0), morsel(1)
    assert down - up >= 8, (st, down, up)
    assert 58 <= up <= 100, (st, up)
    for i in (1, 2, 3):
        im = frame(out, st, "eat", i)
        paws = mask(im, draw_squirrel.paw_color(st), 16)
        ys, xs = np.nonzero(paws)
        assert (xs < 64).any() and (xs > 64).any(), "두 앞발"
    assert frame(out, st, "eat", 1).tobytes() != frame(out, st, "eat", 3).tobytes()


@pytest.mark.parametrize("st", STAGES)
def test_eat_has_no_bowl(out, st):
    assert_eat_has_no_bowl(out / "squirrel" / f"sheet_{st}.png", draw_squirrel.SQUIRREL)


def test_acorn_cap_hat_only_on_the_adult(out):
    for anim in ("sit_front", "idle", "walk"):
        assert count(frame(out, "adult", anim, 0), draw_squirrel.CAP, 12) > 12, anim
        assert count(frame(out, "teenager", anim, 0), draw_squirrel.CAP, 12) < 4, anim  # 외곽선 섞인 점 몇 개뿐
