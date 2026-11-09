"""픽셀 그림: 도트가 진짜 도트인지, 시트·pet.json이 규격에 맞는지, 기존 동글 그림이 바이트까지 그대로인지."""

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_pixel as px  # noqa: E402
from sprite_spec import FRAME, NO_SPIN, frame_px  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
PIXEL = ROOT / "ui" / "pets_pixel"
ALL = list(dp.SPECIES)
DISPLAY = {"baby": 72, "kinder": 84, "child": 96, "teenager": 96, "adult": 108}  # "동물이 너무 크다"는 피드백으로 ×3으로 통일
PALETTE_MAX = 32  # 한 프레임 색 수 상한 (효과 포함, 실측 최대 + 여유). 넘으면 어딘가 색이 섞이고 있다


def _a(im):
    return np.array(im.convert("RGBA"))


def _man(pid):
    return json.loads((PIXEL / pid / "pet.json").read_text(encoding="utf-8"))


def _sheet(pid, stage):
    return _a(Image.open(PIXEL / pid / f"sheet_{stage}.png"))


def _frames(pid, stage):
    m, sheet = _man(pid), _sheet(pid, stage)
    n = m["frame_sizes"][stage]
    for anim, spec in m["animations"].items():
        for i in range(spec["frames"]):
            yield anim, i, sheet[spec["row"] * n:(spec["row"] + 1) * n, i * n:(i + 1) * n]


def _shipped_frame(pid, stage, anim, i):
    """들어 있는 동글 시트의 한 칸 (이 테스트 파일이 돌기 전 그대로인지는 test_shipped_round_sheets_untouched가 본다)."""
    d = ROOT / "ui" / "pets" / pid
    man = json.loads((d / "pet.json").read_text(encoding="utf-8"))
    fs, row = man.get("frame_sizes", {}).get(stage, man["frame_size"]), man["animations"][anim]["row"]
    sheet = Image.open(d / f"sheet_{stage}.png").convert("RGBA")
    return sheet.crop((i * fs, row * fs, (i + 1) * fs, (row + 1) * fs))


def _round_sheets():
    """동글 시트 전부의 지문 (git 없이도, 고친 트리에서도 돈다)."""
    return {p: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT / "ui" / "pets").glob("*/*.png"))}


# 이 파일을 읽을 때의 시트: 픽셀 그리기가 끝난 뒤에도 그대로여야 한다
ROUND_SHEETS_AT_START = _round_sheets()


# ---- 격자
def test_stage_grid_is_integer_scale():
    assert px.STAGE_GRID == {"baby": (24, 3), "kinder": (28, 3), "child": (32, 3), "teenager": (32, 3), "adult": (36, 3)}
    assert {st: n * s for st, (n, s) in px.STAGE_GRID.items()} == DISPLAY


def test_dinos_alone_have_fewer_dots_at_the_same_dot_size():
    """픽셀 공룡만 조금 작게 (사용자 요청): 칸 크기는 같아 점이 고르고, 칸 수만 적다 — 60/72/84/84/96 px. 앱은 pet.json frame_sizes를 따른다."""
    want = {"baby": 20, "kinder": 24, "child": 28, "teenager": 28, "adult": 32}
    for pid in ALL:
        m = json.loads((PIXEL / pid / "pet.json").read_text(encoding="utf-8"))
        if pid.startswith("dino_"):
            assert m["frame_sizes"] == want, pid
            assert {st: n * 3 for st, n in want.items()} == {"baby": 60, "kinder": 72, "child": 84, "teenager": 84, "adult": 96}
        else:
            assert m["frame_sizes"] == {st: n for st, (n, _) in px.STAGE_GRID.items()}, pid


@pytest.mark.parametrize("pid", ALL)
def test_grid_fits_every_frame(pid):
    """칸 크기는 가장 큰 프레임도 사방 외곽선 한 칸을 남기고 들어가게 정해진다 — 몸이 잘리지 않는다 (옮기는 건 _fit_shift)."""
    sp = dp.SPECIES[pid]
    for st in dp.STAGES:
        g = px.stage_grid(sp, st)
        boxes = []
        with px._stage_ctx(sp, st), px.pixel_mode(px.Grid(g.n, 12, 0, 0), collect=boxes):
            ssp = px.pixel_species(dp.staged_species(sp))
            for anim, cnt in px._anims(sp):
                dp.frames(ssp, anim, cnt)
        assert max(max(b[2] - b[0], b[3] - b[1]) for b in boxes) <= g.c * (g.n - 2), st


def test_fit_shift_moves_whole_dots_and_keeps_margin():
    g = px.Grid(24, 20, 16, 16)
    moved, dx, dy = px._fit_shift(g, (16, 16, 200, 200))  # 왼쪽·위로 칸 밖
    assert (dx, dy) == (1, 1) and moved.x0 == g.x0 - 20 and moved.y0 == g.y0 - 20
    same, dx, dy = px._fit_shift(g, (100, 100, 200, 200))
    assert (dx, dy) == (0, 0) and same == g
    _, dx, dy = px._fit_shift(g, (300, 300, 16 + 20 * 24, 16 + 20 * 24))  # 오른쪽·아래로
    assert (dx, dy) == (-1, -1)


# ---- 종마다: 시트 규격
@pytest.mark.parametrize("pid", ALL)
def test_sheet_dims_and_frames_exist(pid):
    m = _man(pid)
    rows, cols = len(m["animations"]), max(s["frames"] for s in m["animations"].values())
    for st in px.STAGE_GRID:
        n = px.stage_dots(pid, st)
        assert m["frame_sizes"][st] == n
        with Image.open(PIXEL / pid / f"sheet_{st}.png") as im:
            assert im.size == (cols * n, rows * n), st


def test_only_belly_sliders_keep_their_roll_in_pixel():
    """도는 구르기는 도트로 돌리면 무너진다 — 픽셀에서는 빠진다. 돌지 않는 배 미끄럼(펭귄·눈사람)만 남는다."""
    assert px.PIXEL_ROLL == {"penguin", "snowman"}
    assert px.PIXEL_ROLL <= NO_SPIN
    for pid in ALL:
        assert ("roll" in [r[0] for r in px.pixel_rows(dp.SPECIES[pid])]) == (pid in px.PIXEL_ROLL), pid


@pytest.mark.parametrize("pid", ALL)
def test_anims_match_round_minus_spinning_roll(pid):
    m = _man(pid)
    round_m = json.loads((ROOT / "ui" / "pets" / pid / "pet.json").read_text(encoding="utf-8"))
    want = {a: s for a, s in round_m["animations"].items() if a != "roll" or pid in px.PIXEL_ROLL}
    assert set(m["animations"]) == set(want)
    assert ("roll" in m["animations"]) == (pid in px.PIXEL_ROLL)
    assert all("spin" not in s for s in m["animations"].values()), "픽셀은 돌지 않는다"
    assert sorted(s["row"] for s in m["animations"].values()) == list(range(len(want)))  # 빈 줄 없음
    for a, s in want.items():
        got = m["animations"][a]
        assert (got["frames"], got["fps"], got.get("loop", True), got.get("front", False)) == (s["frames"], s["fps"], s.get("loop", True), s.get("front", False)), a


@pytest.mark.parametrize("pid", ALL)
def test_hard_dots_no_empty_frames_palette_bound(pid):
    for st in dp.STAGES:
        for anim, i, f in _frames(pid, st):
            alpha = f[..., 3]
            assert set(np.unique(alpha)) <= {0, 255}, (st, anim, i)  # 반투명 없음
            assert (alpha > 0).sum() >= 40, (st, anim, i)
            assert len(np.unique(f[alpha > 0][:, :3], axis=0)) <= PALETTE_MAX, (st, anim, i)


@pytest.mark.parametrize("pid", ALL)
def test_big_enough_at_every_stage(pid):
    """앉은 모습이 캔버스에서 충분히 크다 — 아기 24칸에서도 알아볼 수 있게."""
    for st in dp.STAGES:
        n = px.stage_dots(pid, st)
        f = next(f for a, i, f in _frames(pid, st) if a == "sit_front")
        ys, xs = np.nonzero(f[..., 3] > 0)
        assert ys.max() - ys.min() + 1 >= n * 0.6 and xs.max() - xs.min() + 1 >= n * 0.45, (st, int(xs.max() - xs.min()), int(ys.max() - ys.min()))


# ---- 종마다: 머리·목 표
@pytest.mark.parametrize("pid", ALL)
def test_head_table_covers_every_cell_in_dots(pid):
    m = _man(pid)
    for st in dp.STAGES:
        n = m["frame_sizes"][st]
        for anim, spec in m["animations"].items():
            cells = m["head"][st][anim]
            assert len(cells) == spec["frames"], (st, anim)
            for x, y, w, _r in cells:
                assert -n * 0.25 <= x <= n * 1.25 and -n * 0.25 <= y <= n, (st, anim, x, y)
                assert 0 < w <= n * 1.2, (st, anim, w)


@pytest.mark.parametrize("pid", ALL)
def test_head_mark_sits_on_the_body(pid):
    """모자 입구 가운데가 몸 칸 근처 (2칸 안)."""
    m = _man(pid)
    for st in ("baby", "adult"):
        for anim, i, f in _frames(pid, st):
            x, y, *_ = m["head"][st][anim][i]
            n = f.shape[0]
            cx, cy = int(np.clip(round(x), 0, n - 1)), int(np.clip(round(y), 0, n - 1))
            window = f[max(cy - 3, 0):cy + 4, max(cx - 3, 0):cx + 4, 3]
            assert window.any(), (st, anim, i, x, y)


def test_santa_has_a_neck_table_and_nobody_else():
    for pid in ALL:
        assert ("neck" in _man(pid)) == (pid == "santa"), pid
    m = _man("santa")
    for st in dp.STAGES:
        for anim, spec in m["animations"].items():
            assert len(m["neck"][st][anim]) == spec["frames"], (st, anim)


def test_giraffe_neck_layers_in_dots():
    m = _man("giraffe")
    for st, (n, _s) in px.STAGE_GRID.items():
        meta = m["neck_stretch"][st]
        assert 0 < meta["cut"] < n and meta["tile"] >= 3 and meta["ext"] % meta["tile"] == 0
        with Image.open(PIXEL / "giraffe" / f"neck_{st}.png") as im:
            assert im.size == (4 * n, n)
        tile = _a(Image.open(PIXEL / "giraffe" / f"neck_tile_{st}.png"))
        assert tile.shape[:2] == (meta["tile"], n)
        assert set(np.unique(tile[..., 3])) <= {0, 255}
        solid = tile[..., 3] > 0
        assert (solid == solid[0]).all()  # 곧은 토막: 위아래로 이어진다


# ---- 도장·문맥
def test_eye_stamp_is_exact():
    """변환이 없을 때 눈은 stage_eye가 받은 자리에 찍힌다 — 찾아낼 필요가 없다."""
    grid = px.stage_grid(dp.CAT, "baby")
    with dp.stage("baby", dp.CAT.stage_tweak.get("baby")), px.pixel_mode(grid):
        with dp.view(scale=1.0, lift=0):
            (fns, _), = px.stage_eye("dot", 44, 54)
    st = fns[0]
    assert st.rows and st.x == pytest.approx(grid.to_dot_x(44))
    assert st.y == pytest.approx(grid.to_dot_y(54 + dp.STAGE_PROPS["baby"]["eye_dy"]))


def test_adult_eyes_are_open_not_half_closed():
    adult = px.render_pixel(dp.CAT, "adult", anims=["sit_front"])["sit_front"][0]
    dark = (_a(adult)[..., :3] == np.array(px.EYE_COL)).all(-1)
    assert dark.any(axis=1).sum() >= 2  # 위 눈꺼풀 한 줄이 아니라 또렷한 눈


def test_front_pose_is_mirror_symmetric():
    """앞모습 앉기: 머리 실루엣이 가운데선 대칭 (삐죽 털이 없는 어른)."""
    a = _a(px.render_pixel(dp.CAT, "adult", anims=["sit_front"])["sit_front"][0])
    n = a.shape[0]
    head = a[: n // 2]
    assert ((head[:, : n // 2, 3] > 0) == (head[:, n // 2:, 3] > 0)[:, ::-1]).mean() > 0.97


def test_patch_is_undone():
    names = [(mod, name, getattr(mod, name)) for mod in px._modules() for name in px._PATCH if hasattr(mod, name)]
    grid = px.stage_grid(dp.CAT, "baby")
    with px.pixel_mode(grid):
        assert dp.compose is not px._ORIG["compose"]
    for mod, name, val in names:
        assert getattr(mod, name) is val, (mod.__name__, name)


def test_pixel_mode_stays_out_of_the_way_of_unpatched():
    with px.pixel_mode(px.stage_grid(dp.CAT, "baby")):
        with px._unpatched():
            assert dp.compose is px._ORIG["compose"]
        assert dp.compose is px.compose


# ---- 동글 그림
@pytest.mark.parametrize("pid,stage,anim,i", [
    ("cat", "baby", "sleep", 0), ("cat", "adult", "roll", 0), ("rabbit", "teenager", "walk", 1), ("bird", "adult", "walk", 1),
    ("persian", "baby", "huff", 1), ("frog", "child", "bigjump", 1), ("pumpkin", "adult", "sit_front", 0), ("ghost", "kinder", "walk", 0),
    ("duck", "teenager", "hungry", 1), ("giraffe", "child", "neckstretch", 1), ("rudolph", "adult", "walk", 0),
    ("santa", "adult", "hungry", 1), ("snowman", "kinder", "heart", 1), ("dino_green", "teenager", "fire", 2),
])
def test_round_art_stays_byte_identical(pid, stage, anim, i):
    """픽셀 모드를 한 번 돌린 뒤에도 동글 그림은 커밋된 시트와 바이트까지 같다."""
    sp = dp.SPECIES[pid]
    px.render_pixel(sp, "kinder", anims=["walk"])
    with dp.stage(stage, sp.stage_tweak.get(stage)), dp.output(frame_px(pid, stage) // FRAME):
        ssp = dp.staged_species(sp)
        n = [r[1] for r in dp.rows_for(sp) if r[0] == anim][0]
        got = dp.frames(ssp, anim, n)[i]
    assert got.tobytes() == _shipped_frame(pid, stage, anim, i).tobytes()


def test_shipped_round_sheets_untouched():
    """픽셀 모드를 돌려도 동글 시트 파일은 그대로다 (크기는 pet.json — 그림만 그대로)."""
    assert _round_sheets() == ROUND_SHEETS_AT_START


def test_preview_group_makes_its_folder(tmp_path, monkeypatch):
    """tools/preview는 커밋되지 않는다 — 새 체크아웃에서도 미리보기가 저장된다."""
    monkeypatch.setattr(px, "_panel_species", lambda sp, by_stage: Image.new("RGBA", (4, 4)))
    name, pids = next(iter(px.GROUPS.items()))
    path = px.preview_group(name, dict.fromkeys(pids), tmp_path / "new" / "dir")
    assert path.is_file()
