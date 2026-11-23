"""🔎 "엥 왜 이래?" 시트 검사 — 들어 있는 시트 그대로 (ui/pets/*/sheet_<단계>.png, 다시 그리지 않음) 모든 동물·단계·동작·칸을 봄:

- pet.json이 가리키는 칸이 시트 안에 있고 비어 있지 않음 (빈 칸 = 그 순간 동물이 사라짐)
- 칸 바깥 1px 테두리는 투명 (웹뷰가 칸을 키우고 줄일 때 옆 칸 그림이 가장자리에 비쳐 선·점이 보임)
- 주변 (가로세로 ±10px, 128 기준)에 아무것도 없는 아주 작은 덩어리 (6px 미만)가 없음 = 그림 옆에 떨어진 점·티끌
- 같은 동작의 이웃 칸끼리 발바닥 높이가 갑자기 뛰지 않음 (뛰기·날기·매달리기처럼 원래 들썩이는 동작은 뺌)

뒷모습 (climb·slide)은 머리를 팔 위에 그림 — 그리는 순서라 각 그림 테스트가 봄 (test_draw_bat·harry·hermione·hagrid·musoe·spider·super
back_view_head_covers_the_arms). 찾은 것은 실패 글에 동물·단계·동작·칸·자리로 나옴
"""
import json
from functools import lru_cache
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
PETS = ROOT / "ui" / "pets"
STAGES = ["baby", "kinder", "child", "teenager", "adult"]
# 보이는 픽셀 (알파) — 이보다 흐린 것은 화면에서 거의 안 보임
SEEN = 8
# 티끌: 128 칸 기준 ±SPECK_REACH px 안에 보이는 픽셀이 SPECK_PX개 미만
SPECK_REACH = 10
SPECK_PX = 6
# 발바닥 높이가 이웃 칸과 이만큼 (128 기준 px) 넘게 다르면 들썩임
BASELINE_JUMP = 6
# 원래 몸이 들썩이는 동작 (뛰기·떨어지기·날기·매달리기·오르기·달리기 도약·춤·만세)
BOUNCY = {"hop", "bigjump", "fall", "dragged", "roll", "climb", "slide", "rope", "hang", "hang_cursor", "hang_down", "run", "fly", "fly_fast",
          "fly_flip", "hover", "level_up", "dance", "dance2", "web_swing", "wheelie", "bike_jump", "broom", "dragon", "throw", "catch",
          "carry_run", "hero_land", "sidecar_fly", "lift", "stuff"}
# 둥둥 떠다니는 동물 (몸 전체가 오르내림)
FLOATERS = {"dementor"}


def species():
    return sorted(p.name for p in PETS.iterdir() if (p / "pet.json").is_file())


@lru_cache(maxsize=None)
def manifest(pid):
    return json.loads((PETS / pid / "pet.json").read_text(encoding="utf-8"))


def frame_size(pid, stage):
    m = manifest(pid)
    return m.get("frame_sizes", {}).get(stage, m["frame_size"])


@lru_cache(maxsize=8)
def alpha(pid, stage):
    return np.asarray(Image.open(PETS / pid / f"sheet_{stage}.png").convert("RGBA"))[:, :, 3]


def cells(pid, stage):
    """(동작, 칸 번호, 알파 칸) — pet.json이 가리키는 칸 모두. 시트 밖이면 알파 칸이 None"""
    a, fs = alpha(pid, stage), frame_size(pid, stage)
    for anim, spec in manifest(pid)["animations"].items():
        for f in range(spec["frames"]):
            y0, x0 = spec["row"] * fs, f * fs
            c = a[y0:y0 + fs, x0:x0 + fs]
            yield anim, f, (c if c.shape == (fs, fs) else None)


def specks(c, k):
    """티끌 픽셀 마스크: 보이는 픽셀인데 둘레 상자 안 보이는 픽셀이 아주 적음 (적분 영상으로 셈)"""
    vis = (c > SEEN).astype(np.int32)
    r = max(1, round(SPECK_REACH * k))
    n = 2 * r + 1
    ii = np.pad(vis, ((r + 1, r), (r + 1, r))).cumsum(0).cumsum(1)
    box = ii[n:, n:] - ii[:-n, n:] - ii[n:, :-n] + ii[:-n, :-n]
    return (vis > 0) & (box < SPECK_PX * k * k)


def cases():
    return [(pid, st) for pid in species() for st in STAGES]


@pytest.mark.parametrize("pid,stage", cases())
def test_every_frame_is_drawn_inside_its_cell(pid, stage):
    """빈 칸·시트 밖 칸 없음, 칸 테두리 1px는 투명 (옆 칸으로 비치지 않게), 떨어진 티끌 없음"""
    k = frame_size(pid, stage) / 128
    bad = []
    for anim, f, c in cells(pid, stage):
        if c is None:
            bad.append(f"{anim}[{f}]: outside the sheet")
            continue
        if not (c > SEEN).any():
            bad.append(f"{anim}[{f}]: blank — the pet vanishes for a frame")
            continue
        ring = np.concatenate([c[0], c[-1], c[:, 0], c[:, -1]])
        if (ring > 0).any():
            bad.append(f"{anim}[{f}]: touches the cell border ({int((ring > 0).sum())} px) — bleeds into neighbours")
        s = specks(c, k)
        if s.any():
            ys, xs = np.nonzero(s)
            bad.append(f"{anim}[{f}]: stray speck at ({int(xs[0] / k)}, {int(ys[0] / k)}) (128 px), {int(s.sum())} px")
    assert not bad, f"{pid} {stage}:\n" + "\n".join(bad)


def feet(c, k):
    """발바닥 높이 (128 기준): 보이는 픽셀이 몇 개 넘는 가장 아래 줄 (티끌은 뺌)"""
    vis = (c > SEEN) & ~specks(c, k)
    rows = np.nonzero(vis.sum(1) >= max(2, 3 * k))[0]
    return rows[-1] / k if len(rows) else None


@pytest.mark.parametrize("pid,stage", cases())
def test_feet_do_not_jump_between_frames(pid, stage):
    """서 있는 동작은 이웃 칸끼리 (반복이면 끝↔처음도) 발바닥이 BASELINE_JUMP px 넘게 뛰지 않음 — 넘으면 걷다가 몸이 들썩여 보임"""
    if pid in FLOATERS:
        pytest.skip("floats")
    k = frame_size(pid, stage) / 128
    by_anim = {}
    for anim, f, c in cells(pid, stage):
        if c is not None and anim not in BOUNCY:
            by_anim.setdefault(anim, []).append(feet(c, k))
    bad = []
    for anim, fs in by_anim.items():
        fs = [b for b in fs if b is not None]
        pairs = list(zip(fs, fs[1:]))
        if manifest(pid)["animations"][anim].get("loop", True) and len(fs) > 2:
            pairs.append((fs[-1], fs[0]))
        jump = max((abs(a - b) for a, b in pairs), default=0)
        if jump > BASELINE_JUMP:
            bad.append(f"{anim}: feet {[round(b) for b in fs]} jump {jump:.0f}px")
    assert not bad, f"{pid} {stage}:\n" + "\n".join(bad)


def test_the_speck_finder_finds_a_dot():
    """검사가 점 하나를 잡는지: 빈 칸 구석의 2px 점은 티끌, 몸 바로 옆 점은 아님"""
    c = np.zeros((128, 128), np.uint8)
    c[40:100, 30:90] = 255
    c[5:7, 120] = 255
    c[50, 93] = 255
    s = specks(c, 1.0)
    assert s[5, 120] and s[6, 120] and not s[50, 93] and not s[60, 60]
