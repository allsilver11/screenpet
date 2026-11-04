"""오리·기린 시트 검사: 모든 동작·투명 여백·pet.json·eat에 밥그릇 없음·기린 목 늘이기 층 (구르지 않음)."""
import json
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_farm
import draw_pets
from sprite_spec import ANIMS, FRAME, NO_SPIN, STAGES, frame_px, sheet_128

ROOT = Path(__file__).resolve().parent.parent
BOWL_COLORS = [(246, 196, 150), (252, 226, 196), (230, 222, 206)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다 — 기린 층과 neck_stretch도 여기서 나와야 한다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"duck": draw_farm.DUCK, "giraffe": draw_farm.GIRAFFE})
        draw_pets.build(root)
    return root


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def names(sp):
    return [a for a, *_ in draw_pets.rows_for(sp)]


def test_registered_with_their_own_poses():
    assert draw_pets.SPECIES["duck"] is draw_farm.DUCK and draw_pets.SPECIES["giraffe"] is draw_farm.GIRAFFE
    assert names(draw_farm.DUCK) == [a for a, *_ in ANIMS] + ["lookup"]
    assert names(draw_farm.GIRAFFE) == [a for a, *_ in ANIMS] + ["neckstretch", "lookup"]
    for sp in (draw_farm.DUCK, draw_farm.GIRAFFE):
        assert all(a in sp.poses for a in names(sp))


@pytest.mark.parametrize("pid", ["duck", "giraffe"])
def test_every_frame_is_drawn_inside_the_cell(out, pid):
    sp = draw_pets.SPECIES[pid]
    rows = draw_pets.rows_for(sp)
    for st in STAGES:
        sheet = sheet_128(out / pid / f"sheet_{st}.png")
        assert sheet.size == (4 * FRAME, len(rows) * FRAME)
        for r, (anim, n, *_) in enumerate(rows):
            for c in range(n):
                box = cell(sheet, r, c).getchannel("A").getbbox()
                assert box, (st, anim, c)
                if anim in ("roll", "neckstretch"):
                    continue
                x0, y0, x1, y1 = box
                assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, box)
    assert not (out / pid / "sheet.png").exists()


def test_manifests(out):
    duck = json.loads((out / "duck" / "pet.json").read_text(encoding="utf-8"))
    gir = json.loads((out / "giraffe" / "pet.json").read_text(encoding="utf-8"))
    assert (duck["name"], gir["name"]) == ("오리", "기린")
    assert duck["favorite_food"] == {"emoji": "🍞", "name": "빵"}
    assert gir["favorite_food"] == {"emoji": "🍃", "name": "나뭇잎"}
    assert (duck["personality"], gir["personality"]) == ("playful", "friendly")
    assert duck["speech"]["chatter"] == ["quack!", "♪"] and gir["speech"]["chatter"] == ["hmm~", "♪"]
    # 보통 집 · 걷는다
    assert "house" not in duck and "house" not in gir and "locomotion" not in duck and "locomotion" not in gir
    assert set(gir["stage_size"].values()) == {1.06}  # "동물이 너무 크다"는 피드백: 단계별 배율 없앰, 토끼쯤으로
    # 기린은 구르지 않는다: neckstretch 동작 (코어 Special::NeckStretch), roll은 돌지 않는다
    assert gir["animations"]["neckstretch"] == {"row": 21, "frames": 3, "fps": 2}
    assert "spin" not in gir["animations"]["roll"] and "spin" in duck["animations"]["roll"]
    assert "neckstretch" not in duck["animations"]
    assert "neck_stretch" in gir and "neck_stretch" not in duck
    assert "giraffe" in NO_SPIN


def test_the_build_reproduces_the_committed_giraffe(out):
    """draw_pets 빌드 결과가 저장소의 기린과 바이트까지 같다 (빌드가 neck_stretch를 지우고 spin을 되살리던 버그)."""
    shipped = ROOT / "ui" / "pets" / "giraffe"
    names = sorted(p.name for p in shipped.iterdir())
    assert names == sorted(p.name for p in (out / "giraffe").iterdir())
    for name in names:
        assert (out / "giraffe" / name).read_bytes() == (shipped / name).read_bytes(), name


@pytest.mark.parametrize("pid", ["duck", "giraffe"])
def test_eat_has_no_bowl(out, pid):
    """밥그릇은 앱의 소품 창이 띄운다 — 시트 eat에는 그릇 색이 없다."""
    row = names(draw_pets.SPECIES[pid]).index("eat")
    for st in STAGES:
        sheet = sheet_128(out / pid / f"sheet_{st}.png").convert("RGBA")
        for c in range(4):
            a = np.asarray(cell(sheet, row, c)).astype(int)
            # 하얀 오리의 날개 그늘은 회색 그릇 색과 비슷해 뺀다
            for col in (BOWL_COLORS[:2] if pid == "duck" else BOWL_COLORS):
                near = (np.abs(a[..., :3] - col).sum(axis=2) < 12) & (a[..., 3] > 200)
                assert near.sum() < 25, (st, c, col, near.sum())


def test_neck_stretch_layers_fit_the_sheet(out):
    """층 (몸 · 머리 셋)을 늘이지 않고 겹치면 시트의 neckstretch 프레임과 같고, 목 토막은 위아래로 이어진다."""
    meta = json.loads((out / "giraffe" / "pet.json").read_text(encoding="utf-8"))["neck_stretch"]
    assert set(meta) == set(STAGES)
    row = names(draw_farm.GIRAFFE).index("neckstretch")
    for st in STAGES:
        u = frame_px("giraffe", st) // FRAME
        assert all(v % u == 0 for v in meta[st].values()), (st, meta[st])  # 2배 칸은 128 칸 값의 정확히 2배 (같은 자리에서 자른다)
        m = {k: v // u for k, v in meta[st].items()}
        assert m["tile"] == draw_farm.NECK_TILE and m["ext"] % m["tile"] == 0
        assert 40 < m["cut"] < 110, m
        # 목을 두 배쯤 (지금 목 길이를 토막 배수로) — 앱은 창을 위로 키워 담는다 (src-tauri slots STRETCH_SPACE)
        assert 16 <= m["ext"] <= 56, m
        strip = Image.open(out / "giraffe" / f"neck_{st}.png").reduce(u)
        tile = Image.open(out / "giraffe" / f"neck_tile_{st}.png").reduce(u)
        assert strip.size == (4 * FRAME, FRAME) and tile.size == (FRAME, m["tile"])
        sheet = sheet_128(out / "giraffe" / f"sheet_{st}.png")
        for j in range(3):
            comp = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
            comp.paste(strip.crop((0, m["cut"], FRAME, FRAME)), (0, m["cut"]))
            comp.paste(strip.crop(((j + 1) * FRAME, 0, (j + 2) * FRAME, m["cut"])), (0, 0))
            want = np.asarray(cell(sheet, row, j)).astype(int)[m["cut"] - 1 if j else 0:]
            got = np.asarray(comp).astype(int)[m["cut"] - 1 if j else 0:]
            # 머리 셋 모두 같은 몸에 붙는다 (✧ 반짝이는 머리 층에만 있어 위쪽은 j = 0만 통째로 본다)
            assert np.abs(want - got).max() <= 2, (st, j)
        # 몸 층은 자르는 줄 아래만, 머리 층은 위만
        assert strip.crop((0, 0, FRAME, m["cut"])).getchannel("A").getbbox() is None
        for j in range(1, 4):
            assert strip.crop((j * FRAME, m["cut"], (j + 1) * FRAME, FRAME)).getchannel("A").getbbox() is None
        # 토막은 곧은 목 한 줄기: 매 줄 같은 폭, 맨 위 줄과 맨 아래 줄 다음(= 몸 층의 자르는 줄)이 이어진다
        ta = np.asarray(tile.getchannel("A")) > 128
        widths = {int(r.sum()) for r in ta}
        assert len(widths) <= 2 and min(widths) > 6, widths
        body_row = np.asarray(strip.getchannel("A"))[m["cut"], :FRAME] > 128
        assert np.array_equal(np.nonzero(ta[0])[0][[0, -1]], np.nonzero(body_row)[0][[0, -1]]) or \
            np.abs(np.nonzero(ta[0])[0][[0, -1]] - np.nonzero(body_row)[0][[0, -1]]).max() <= 1, st


@pytest.mark.parametrize("st", ["kinder", "teenager", "adult"])
def test_neck_preview_shows_the_same_giraffe_on_2x_cells(tmp_path, st):
    """preview_neck는 층을 128 칸으로 줄여 겹친다: 첫 칸(평소)은 시트의 neckstretch 첫 프레임과 같아야 한다."""
    d = ROOT / "ui" / "pets" / "giraffe"
    out = tmp_path / "neck.png"
    draw_farm.preview_neck(d, out)
    bg = (236, 240, 246, 255)
    x, y = 4 + STAGES.index(st) * 4 * (FRAME + 4), 4 + 70
    got = Image.open(out).reduce(2).convert("RGB").crop((x, y, x + FRAME, y + FRAME))
    frame = cell(sheet_128(d / f"sheet_{st}.png"), names(draw_farm.GIRAFFE).index("neckstretch"), 0)
    want = Image.alpha_composite(Image.new("RGBA", frame.size, bg), frame).convert("RGB")
    assert np.abs(np.asarray(got).astype(int) - np.asarray(want).astype(int)).mean() < 2.0


@pytest.mark.parametrize("st", STAGES)
def test_giraffe_eats_right_above_the_bowl(st):
    """입이 그릇 밥 더미 바로 위, 프레임 (64, 84) — 목이 기울면 입이 옆으로 비껴 간다."""
    sp = draw_farm.GIRAFFE
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        im = draw_pets.frames(draw_pets.staged_species(sp), "eat", 4)[0]
    a = np.asarray(im.convert("RGBA")).astype(int)
    near = lambda col: np.abs(a[..., :3] - col).sum(axis=2) < 12  # noqa: E731
    mouth = (near(draw_pets.MOUTH_IN) | near(draw_pets.TONGUE)) & (a[..., 3] > 200)  # 작은 입은 혀가 다 채운다
    ys, xs = np.nonzero(mouth)
    assert abs(xs.mean() - 64) <= 1.0, (st, xs.mean())
    assert abs(ys.mean() - 84) <= 3.0, (st, ys.mean())


@pytest.mark.parametrize("st", STAGES)
def test_duck_eat_puts_the_bill_on_the_bowl_with_the_real_head_radius(st, monkeypatch):
    """dk_eat의 부리 자리 계산은 duck()의 앞모습 머리 반지름(25)과 같아야 한다."""
    sp = draw_farm.DUCK
    seen = []
    real = draw_farm.duck_bill
    monkeypatch.setattr(draw_farm, "duck_bill", lambda cx, cy, *a, **k: seen.append((cy, k["k"])) or real(cx, cy, *a, **k))
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        with draw_farm._in_view(draw_pets.staged_species(sp)):
            my = draw_pets._frame_to_pre(64, 84)[1]
            draw_farm.dk_eat(sp, 0)
    cy, k = seen[0]
    assert abs(cy + 6.5 * k * 0.5 - my) < 0.01, (st, cy, k, my)
