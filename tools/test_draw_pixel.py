"""픽셀 직접 그리기 스파이크 (고양이) — 도트가 진짜 도트인지, 기존 동글 그림이 바이트까지 그대로인지."""

import io
import json
import subprocess
import sys
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_pixel as px  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="module")
def baby():
    return px.render_pixel(dp.CAT, "baby")


@pytest.fixture(scope="module")
def adult():
    return px.render_pixel(dp.CAT, "adult")


def _a(im):
    return np.array(im.convert("RGBA"))


def _committed_frame(stage, anim, i):
    raw = subprocess.check_output(["git", "-C", str(ROOT), "show", f"HEAD:ui/pets/cat/sheet_{stage}.png"])
    man = json.loads(subprocess.check_output(["git", "-C", str(ROOT), "show", "HEAD:ui/pets/cat/pet.json"]))
    fs, row = man["frame_size"], man["animations"][anim]["row"]
    sheet = Image.open(io.BytesIO(raw)).convert("RGBA")
    return sheet.crop((i * fs, row * fs, (i + 1) * fs, (row + 1) * fs))


def test_stage_grid_is_integer_scale():
    assert px.STAGE_GRID == {"baby": (24, 3), "kinder": (28, 3), "child": (32, 3), "teenager": (32, 4), "adult": (36, 4)}


def test_every_cat_anim_but_roll(baby):
    want = {a for a, *_ in dp.rows_for(dp.CAT)} - {"roll"}
    assert set(baby) == want
    assert sum(len(v) for v in baby.values()) == 76


def test_frames_are_hard_dots(baby, adult):
    for stage, rendered in (("baby", baby), ("adult", adult)):
        n = px.STAGE_GRID[stage][0]
        for anim, ims in rendered.items():
            for im in ims:
                assert im.size == (n, n), anim
                assert set(np.unique(_a(im)[..., 3])) <= {0, 255}, anim  # 반투명 없음


def test_body_stays_inside_canvas(baby, adult):
    """맨 아랫줄은 발 외곽선, 좌우·위 가장자리에 몸이 잘리지 않는다 (모든 프레임의 실제 칸 수 — 효과는 제외)."""
    for stage in ("baby", "adult"):
        grid = px.stage_grid(dp.CAT, stage)
        assert grid.half_px + grid.cell <= grid.cell * grid.n / 2 + 1e-6, stage
        assert grid.oy <= grid.top_px - grid.cell + 1e-6, stage


def test_front_pose_is_mirror_symmetric(adult):
    """앞모습 앉기: 머리 실루엣과 눈 자리가 가운데선 대칭 (삐죽 털이 없는 어른 — 반짝이 색은 한쪽 빛이라 대칭이 아니다)."""
    a = _a(adult["sit_front"][0])
    n = a.shape[0]
    head = a[: n // 2]
    assert ((head[:, : n // 2, 3] > 0) == (head[:, n // 2:, 3] > 0)[:, ::-1]).mean() > 0.97
    dark = (head[..., :3] == np.array(px.EYE_COL)).all(-1)
    assert (dark[:, : n // 2].sum(1) == dark[:, n // 2:].sum(1)).all()


def test_eye_stamp_is_exact(baby):
    """변환이 없을 때 눈은 stage_eye가 받은 자리에 찍힌다 — 찾아낼 필요가 없다."""
    grid = px.stage_grid(dp.CAT, "baby")
    with dp.stage("baby", dp.CAT.stage_tweak.get("baby")), px.pixel_mode(grid):
        with dp.view(scale=1.0, lift=0):
            (fns, _), = px.stage_eye("dot", 44, 54)
    st = fns[0]
    assert st.rows and st.x == pytest.approx(grid.to_dot_x(44))
    assert st.y == pytest.approx(grid.to_dot_y(54 + dp.STAGE_PROPS["baby"]["eye_dy"]))


def test_few_colors_no_mud(adult):
    """칸마다 진짜 설정 색을 쓰므로 섞인 색이 생기지 않는다."""
    for anim, ims in adult.items():
        for im in ims:
            a = _a(im)
            assert len(np.unique(a[a[..., 3] > 0][:, :3], axis=0)) <= 16, anim


def test_sleep_z_does_not_touch_body(baby):
    a = _a(baby["sleep"][0])
    z = (a[..., :3] == np.array(px.Z_COL)).all(-1) & (a[..., 3] > 0)
    assert z.sum() >= 5


def test_patch_is_undone():
    before = dp.compose
    grid = px.stage_grid(dp.CAT, "baby")
    with px.pixel_mode(grid):
        assert dp.compose is not before
    assert dp.compose is before


@pytest.mark.parametrize("stage,anim,i", [("baby", "sleep", 0), ("teenager", "walk", 1), ("adult", "hungry", 2),
                                          ("child", "climb", 0), ("kinder", "heart", 1), ("adult", "roll", 0)])
def test_round_art_stays_byte_identical(stage, anim, i):
    """픽셀 모드를 한 번 돌린 뒤에도 기존 그림은 커밋된 시트와 바이트까지 같다."""
    px.render_pixel(dp.CAT, "kinder", anims=["walk"])
    with dp.stage(stage, dp.CAT.stage_tweak.get(stage)):
        ssp = dp.staged_species(dp.CAT)
        n = [r[1] for r in dp.rows_for(dp.CAT) if r[0] == anim][0]
        got = dp.frames(ssp, anim, n)[i]
    assert got.tobytes() == _committed_frame(stage, anim, i).tobytes()


@pytest.mark.parametrize("pid", ["dog", "rabbit", "bear"])
def test_same_skeleton_species_need_no_code(pid):
    """고양이와 뼈대가 같은 종은 종별 코드 없이 그대로 도트가 나온다 (재추정의 근거)."""
    n = px.STAGE_GRID["child"][0]
    r = px.render_pixel(dp.SPECIES[pid], "child", anims=["walk", "sit_front", "sleep"])
    for ims in r.values():
        assert all(im.size == (n, n) and set(np.unique(_a(im)[..., 3])) <= {0, 255} for im in ims)
