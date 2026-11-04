"""tools 테스트가 같이 쓰는 도우미 (from conftest import ...). pytest가 이 폴더를 sys.path에 넣어 준다."""

import json
import os
import shutil
import subprocess
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

from sprite_spec import FRAME, ROLL_CENTER, ROLL_RADIUS, sheet_128

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"

# node 하네스 한 번의 시간 제한 (초): 멈춘 하네스가 CI 전체를 붙잡지 않게
NODE_TIMEOUT = 120

needs_node = pytest.mark.skipif(not shutil.which("node"), reason="node 없음")


def pytest_sessionstart(session):
    """CI에서 node가 없으면 UI 하네스 테스트가 조용히 건너뛰어지지 않고 실패한다."""
    if os.environ.get("CI") and not shutil.which("node"):
        pytest.exit("CI에 node가 없다: UI 하네스 테스트가 건너뛰어진다 (build.yml setup-node)", returncode=1)


def run_node(*args, check=True, **kwargs):
    """node를 시간 제한을 걸어 돌린다 (CompletedProcess)."""
    kwargs.setdefault("timeout", NODE_TIMEOUT)
    return subprocess.run(["node", *map(str, args)], capture_output=True, text=True, encoding="utf-8", check=check, **kwargs)


def node_json(*args, **kwargs):
    """node가 stdout에 쓴 JSON."""
    return json.loads(run_node(*args, **kwargs).stdout)


# ---- 시트 칸 ----


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def anim_names(sp):
    """이 동물 시트의 행 순서 (동작 이름)."""
    import draw_pets

    return [a for a, *_ in draw_pets.rows_for(sp)]


def assert_frames_inside_cell(sheet_dir, sp, stages, free=(), grounded=(), ground_y=118, hover=()):
    """모든 동작·칸이 그려져 있고 칸 안에 들어오고, 안 쓰는 칸은 완전히 투명하다.
    free: 칸 밖으로 나가도 되는 동작 (도는 공 등, 들어가는지는 따로 본다). grounded: 발이 바닥까지 닿는 동작. hover: 바닥과 틈이 있어야 하는 동작."""
    import draw_pets

    rows = draw_pets.rows_for(sp)
    for st in stages:
        sheet = sheet_128(sheet_dir / f"sheet_{st}.png")
        assert sheet.mode == "RGBA" and sheet.size == (4 * FRAME, len(rows) * FRAME), st
        for r, (anim, n, *_) in enumerate(rows):
            for c in range(n):
                box = cell(sheet, r, c).getchannel("A").getbbox()
                assert box, (st, anim, c)
                if anim in free:
                    continue
                x0, y0, x1, y1 = box
                assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, box)
                if anim in grounded:
                    assert y1 >= ground_y, (st, anim, c, box)
                if anim in hover:
                    assert y1 <= 116, (st, anim, c, box)
            for c in range(n, 4):
                assert cell(sheet, r, c).getchannel("A").getbbox() is None, (st, anim, c)
    assert not (sheet_dir / "sheet.png").exists()


def assert_roll_fits_circle(sheet_dir, sp, stages, slack=1):
    """구르기 칸은 돌려도 잘리지 않게 회전 원 안에 든다 (ROLL_CENTER·ROLL_RADIUS, 뿌연 가장자리는 slack px 봐준다)."""
    row = anim_names(sp).index("roll")
    cx, cy = ROLL_CENTER
    for st in stages:
        sheet = sheet_128(sheet_dir / f"sheet_{st}.png")
        for c in range(4):
            alpha = np.asarray(cell(sheet, row, c).getchannel("A"))
            ys, xs = np.nonzero(alpha > 40)
            if not len(xs):
                continue
            worst = float(np.hypot(xs - cx, ys - cy).max())
            assert worst <= ROLL_RADIUS + slack, (sp.id, st, c, worst)


def bowl_colors():
    """앱이 띄우는 진짜 그릇 몸통 색 (draw_props) — 시트 eat에 이 색이 많으면 그릇을 또 그린 것이다.
    테 색(BOWL_*_LIGHT)은 빼고 본다: 토끼 귓속·볼터치의 연분홍과 같아 수백 픽셀씩 나온다."""
    import draw_props

    return [draw_props.BOWL_PINK, draw_props.BOWL_BLUE]


def assert_eat_has_no_bowl(sheet_path, sp, frames=4, colors=None, tol=12, most=25):
    """밥그릇은 앱의 소품 창이 띄운다: 시트 eat에는 그릇 색이 (거의) 없다."""
    row = anim_names(sp).index("eat")
    sheet = sheet_128(sheet_path).convert("RGBA")
    for c in range(frames):
        a = np.asarray(cell(sheet, row, c)).astype(int)
        for col in colors or bowl_colors():
            near = (np.abs(a[..., :3] - col).sum(axis=2) < tol) & (a[..., 3] > 200)
            assert near.sum() < most, (sp.id, sheet_path.name, c, col, int(near.sum()))


def count_near(im, colors, tol=12):
    """이 색들 중 하나에 가까운 불투명 픽셀 수."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    hit = np.zeros(a.shape[:2], bool)
    for col in colors:
        hit |= np.abs(a[..., :3] - col).sum(axis=2) <= tol
    return int((hit & (a[..., 3] > 200)).sum())


def spy_compose(m, seen):
    """draw_pets.compose로 그려지는 부품 목록을 frame마다 seen(parts)에 넘긴다. 그림은 그대로 그린다."""
    import draw_pets

    compose = draw_pets.compose

    def spy(parts, *a, **k):
        seen(parts)
        return compose(parts, *a, **k)

    m.setattr(draw_pets, "compose", spy)


def record_draw_order(m, tagged, other=lambda part: None):
    """frame마다 부품을 그리는 순서를 이름표로 모은다. tagged: (모듈, 함수 이름, 이름표) — 그 함수가 만든 부품에 이름표를 붙인다. 돌려준 목록은 그릴 때마다 찬다."""
    tags, orders = {}, []

    def wrap(kind, fn):
        def f(*a, **k):
            r = fn(*a, **k)
            for p in r if isinstance(r, list) else [r[0] if isinstance(r, tuple) else r]:
                tags[id(p)] = kind
            return r
        return f

    for module, name, kind in tagged:
        m.setattr(module, name, wrap(kind, getattr(module, name)))

    def seen(parts):
        orders.append([tags.get(id(p), other(p)) for p in parts])
        tags.clear()  # id는 다음 프레임에서 다시 쓰일 수 있다

    spy_compose(m, seen)
    return orders


def diff_sides(a, b, tol=60):
    """두 칸이 다른 픽셀 수 (왼쪽 반, 오른쪽 반)."""
    d = np.abs(np.asarray(a).astype(int) - np.asarray(b).astype(int)).sum(axis=2) > tol
    return int(d[:, :FRAME // 2].sum()), int(d[:, FRAME // 2:].sum())


def assert_feet_only_peek(monkeypatch, sp, module, foot_fn, bare_foot, stages, most=0.09, staged=True):
    """발은 언뜻언뜻: 발을 뺀 그림과 비교하면 조금만(most 몫까지) 보이고, 오르는 동안 왼발·오른발이 번갈아 옆으로 나온다."""
    import draw_pets

    for st in stages:
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            ssp = draw_pets.staged_species(sp) if staged else sp
            full = draw_pets.frames(ssp, "climb", 4)
            with monkeypatch.context() as m:
                m.setattr(module, foot_fn, bare_foot)
                bare = draw_pets.frames(ssp, "climb", 4)
        seen = [diff_sides(f, b) for f, b in zip(full, bare)]
        for n, ((l, r), f) in enumerate(zip(seen, full)):
            area = int((np.asarray(f.getchannel("A")) > 128).sum())
            assert 0 < l + r < most * area, (sp.id, st, n, l, r, area)
        assert seen[0][1] > seen[0][0] and seen[2][0] > seen[2][1], (sp.id, st, seen)  # 앞발과 엇갈려 번갈아


# ---- 생성기 출력 == 들어 있는 그림 ----


def assert_same_png(built, shipped):
    """같은 그림 (크기·모드·픽셀 전부)."""
    a, b = Image.open(built).convert("RGBA"), Image.open(shipped).convert("RGBA")
    assert a.size == b.size, (shipped.name, a.size, b.size)
    if a.tobytes() != b.tobytes():
        diff = np.abs(np.asarray(a).astype(int) - np.asarray(b).astype(int)).sum(axis=2)
        raise AssertionError(f"{shipped} 가 생성기 출력과 다르다: {int((diff > 0).sum())}칸, 최대 차이 {int(diff.max())} — 다시 그려 커밋하거나 생성기를 고쳐라")


def assert_folder_matches_shipped(built_dir, shipped_dir):
    """생성기가 만든 파일 전부가 들어 있는 파일과 같다. (들어 있는 그림만 고치고 생성기를 안 고치면 다음 빌드에 사라진다)"""
    built = sorted(p for p in Path(built_dir).rglob("*") if p.is_file())
    assert built, built_dir
    for p in built:
        shipped = Path(shipped_dir) / p.relative_to(built_dir)
        assert shipped.is_file(), f"{shipped}: 들어 있지 않다"
        if p.suffix == ".png":
            assert_same_png(p, shipped)
        elif p.suffix == ".json":
            assert json.loads(p.read_text(encoding="utf-8")) == json.loads(shipped.read_text(encoding="utf-8")), shipped
        else:
            assert p.read_bytes() == shipped.read_bytes(), shipped
