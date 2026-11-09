"""펭귄 시트 검사: 모든 동작·투명 여백·pet.json(걷는 장난꾸러기, 새우)·검은 등과 흰 배·주황 부리와 발·회색 솜털 아기·
뒤뚱 걸음·배 미끄럼(구르기 대신)·콕콕 먹기·어른 나비넥타이·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_penguin
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, cell, count_near
from sprite_spec import ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "penguin"
STANDING = ("walk", "run", "roll", "idle", "look", "sleep", "annoyed", "lookup", "sit_front", "wave", "tilt", "eat", "hungry", "heart")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"penguin": draw_penguin.PENGUIN})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "penguin" / f"sheet_{st}.png"), names(draw_penguin.PENGUIN).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def count(im, col, tol=20):
    return count_near(im, [col], tol)


def test_registered_with_its_own_poses():
    assert draw_pets.SPECIES["penguin"] is draw_penguin.PENGUIN
    assert names(draw_penguin.PENGUIN) == [a for a, *_ in ANIMS] + ["lookup"]
    assert all(a in draw_penguin.PENGUIN.poses for a in names(draw_penguin.PENGUIN))


def test_every_frame_is_drawn_inside_the_cell(out):
    # 배 미끄럼(roll)도 돌리지 않으니 칸 안에, 바닥에 닿는다
    assert_frames_inside_cell(out / "penguin", draw_penguin.PENGUIN, STAGES, grounded=STANDING)


def test_manifest_is_a_playful_walker_that_loves_shrimp(out):
    m = json.loads((out / "penguin" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "펭귄"
    assert m["favorite_food"] == {"emoji": "🦐", "name": "새우"}
    assert m["personality"] == "playful"
    assert "locomotion" not in m and "hop_share" not in m and "house" not in m
    assert "spin" not in m["animations"]["roll"], "구르는 대신 배로 미끄러진다 (앱이 돌리지 않는다)"
    assert "climb" not in m["animations"] and "sting" not in m["animations"]
    assert "waddle~" in m["speech"]["chatter"]
    assert set(m["stage_size"].values()) == {0.85}


def test_the_build_reproduces_the_committed_penguin(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "penguin").iterdir())
    for name in names_:
        assert (out / "penguin" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", ["child", "teenager", "adult"])
@pytest.mark.parametrize("anim", ["sit_front", "idle", "walk"])
def test_black_back_white_belly_orange_beak_and_feet(out, st, anim):
    im = frame(out, st, anim, 0)
    area = int(alpha(im).sum())
    back = count(im, draw_penguin.back_color(st))
    belly = count(im, draw_penguin.WHITE_BELLY, 12)
    orange = count(im, draw_penguin.BEAK, 24) + count(im, draw_penguin.FEET, 24)
    assert back > 0.18 * area, (st, anim, back, area)
    assert belly > 0.15 * area, (st, anim, belly, area)
    assert orange > 15, (st, anim, orange)
    assert sum(draw_penguin.back_color(st)) < 300, "검은(짙은) 등"


def grey(im):
    """회색 솜털 칸 수: 채도가 낮고 중간~연한 밝기 (회색 몸·연회색 배·몸 음영, 새하얀 얼굴은 빼고)."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    rgb = a[..., :3]
    lum = rgb.mean(axis=2)
    return int(((rgb.max(axis=2) - rgb.min(axis=2) < 30) & (lum > 140) & (lum < 236) & (a[..., 3] > 200)).sum())


@pytest.mark.parametrize("anim", ["sit_front", "idle"])
def test_baby_is_a_grey_fluffy_chick(out, anim):
    """아기: 회색 솜털 병아리 (검은 등이 아직 없다) · 흰 얼굴 · 정수리 솜털이 삐죽 (머리 꼭대기가 뾰족하게 좁다)."""
    baby = frame(out, "baby", anim, 0)
    area = int(alpha(baby).sum())
    assert grey(baby) > 0.2 * area  # 외곽선·검은 모자·흰 얼굴을 빼면 몸 대부분
    assert count(baby, draw_penguin.back_color("adult")) < 0.01 * area  # 검은 등은 아직 없다 (외곽선 섞인 몇 점만)
    assert grey(baby) > 3 * grey(frame(out, "adult", anim, 0))  # 어른은 회색 솜털 없이 검정·흰색 (경계 섞인 칸만)
    assert count(baby, draw_penguin.BEAK, 24) > 4

    def top_width(st):
        a = alpha(frame(out, st, anim, 0))
        rows = np.nonzero(a.any(axis=1))[0]
        return int(a[rows[0] + 2].sum())
    assert 1.5 * top_width("baby") < top_width("teenager"), (top_width("baby"), top_width("teenager"))  # 솜털 가닥만 삐죽


@pytest.mark.parametrize("st", STAGES)
def test_walk_is_a_side_to_side_waddle(out, st):
    """뒤뚱뒤뚱: 0·2 장은 몸이 서로 반대쪽으로 기우뚱 (윗부분 무게중심이 반대쪽), 1·3 장은 가운데."""
    def lean(i):
        a = alpha(frame(out, st, "walk", i))
        ys, xs = np.nonzero(a)
        top = ys < np.percentile(ys, 35)
        bottom = ys > np.percentile(ys, 80)
        return xs[top].mean() - xs[bottom].mean()
    l0, l1, l2 = lean(0), lean(1), lean(2)
    assert abs(l0 - l2) >= 5, (st, l0, l1, l2)
    assert min(l0, l2) < l1 < max(l0, l2), (st, l0, l1, l2)


@pytest.mark.parametrize("st", STAGES)
def test_roll_is_a_belly_slide_on_the_floor(out, st):
    """구르기 대신 배로 쭉 미끄러진다: 바닥에 길게 눕고, 흰 배가 아래 · 검은 등이 위, 두 장이 다르다 (얼음 가루)."""
    frames = [frame(out, st, "roll", c) for c in range(2)]
    for im in frames:
        x0, y0, x1, y1 = im.getchannel("A").getbbox()
        assert y1 >= 118 and x1 - x0 >= 70 and y1 - y0 <= 70 and x1 - x0 > 1.3 * (y1 - y0), (st, (x0, y0, x1, y1))
        a = np.asarray(im.convert("RGBA")).astype(int)
        white = (np.abs(a[..., :3] - draw_penguin.WHITE_BELLY).sum(axis=2) < 12) & (a[..., 3] > 200)
        dark = (np.abs(a[..., :3] - draw_penguin.back_color(st)).sum(axis=2) < 20) & (a[..., 3] > 200)
        if st not in ("baby", "kinder"):
            assert np.nonzero(white)[0].mean() > np.nonzero(dark)[0].mean(), st
    assert frames[0].tobytes() != frames[1].tobytes()


@pytest.mark.parametrize("st", STAGES)
def test_eat_pecks_down_to_the_bowl_and_back_up(out, st):
    """콕콕: 0·2 = 부리 끝이 그릇 밥 더미(프레임 y 84) 근처, 1·3 = 고개를 들고 냠 (부리가 더 위)."""
    def beak_bottom(i):
        a = np.asarray(frame(out, st, "eat", i).convert("RGBA")).astype(int)
        m = (np.abs(a[..., :3] - np.array(draw_penguin.BEAK)).sum(axis=2) < 24) & (a[..., 3] > 200)
        m[:, :52] = m[:, 77:] = False
        m[100:] = False  # 발은 빼고 부리만
        assert m.sum() > 3, (st, i)
        ys, xs = np.nonzero(m)
        assert abs(xs.mean() - 64) <= 4, (st, i, xs.mean())
        return ys.max()
    down, up = beak_bottom(0), beak_bottom(1)
    assert 78 <= down <= 92, (st, down)
    assert down - up >= 6, (st, down, up)


@pytest.mark.parametrize("st", STAGES)
def test_eat_has_no_bowl(out, st):
    assert_eat_has_no_bowl(out / "penguin" / f"sheet_{st}.png", draw_penguin.PENGUIN)


def test_bow_tie_only_on_the_adult(out):
    for anim in ("sit_front", "idle"):
        assert count(frame(out, "adult", anim, 0), draw_penguin.BOWTIE) > 10, anim
        assert count(frame(out, "teenager", anim, 0), draw_penguin.BOWTIE) == 0, anim
