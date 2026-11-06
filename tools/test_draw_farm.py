"""오리·기린 시트 검사: 모든 동작·투명 여백·pet.json·eat에 밥그릇 없음·기린 목 늘이기 층 (구르지 않음)."""
import json
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_farm
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, FRAME, NO_SPIN, STAGES, frame_px, sheet_128

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다 — 기린 층과 neck_stretch도 여기서 나와야 한다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"duck": draw_farm.DUCK, "giraffe": draw_farm.GIRAFFE})
        draw_pets.build(root)
    return root


def test_registered_with_their_own_poses():
    assert draw_pets.SPECIES["duck"] is draw_farm.DUCK and draw_pets.SPECIES["giraffe"] is draw_farm.GIRAFFE
    assert names(draw_farm.DUCK) == [a for a, *_ in ANIMS] + ["lookup"]
    assert names(draw_farm.GIRAFFE) == [a for a, *_ in ANIMS] + ["neckstretch", "lookup"]
    for sp in (draw_farm.DUCK, draw_farm.GIRAFFE):
        assert all(a in sp.poses for a in names(sp))


@pytest.mark.parametrize("pid", ["duck", "giraffe"])
def test_every_frame_is_drawn_inside_the_cell(out, pid):
    assert_frames_inside_cell(out / pid, draw_pets.SPECIES[pid], STAGES, free=("roll", "neckstretch"))


def test_duck_roll_ball_fits_rotation_circle(out):
    # 지금 어른 오리는 43.4px까지 나온다 (반지름 38): 공룡과 같다 (test_draw_dino.py). 더 나빠지지 않게만 막는다
    assert_roll_fits_circle(out / "duck", draw_pets.SPECIES["duck"], STAGES, slack=6)


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
    for st in STAGES:
        assert_eat_has_no_bowl(out / pid / f"sheet_{st}.png", draw_pets.SPECIES[pid])


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


@pytest.mark.parametrize("anim", ["walk", "idle", "sit_front"])
@pytest.mark.parametrize("stage", ["baby", "adult"])
def test_duck_legs_go_behind_the_body(monkeypatch, anim, stage):
    """다리는 몸 뒤: 몸(가장 큰 조각)을 다리·발보다 나중에 칠해 몸 아래로 발만 나온다 (위에 칠하면 몸에 꽂힌 막대처럼 보인다)."""
    seen = []
    monkeypatch.setattr(draw_pets, "compose", lambda parts, ops, fx: seen.append(list(parts)) or Image.new("RGBA", (8, 8), (0, 0, 0, 255)))
    with draw_pets.stage(stage):
        draw_farm.DUCK_POSES[anim](draw_farm.DUCK, 0)
    parts = seen[0]
    # 몸 = 처음 칠하는 몸 색 조각, 머리 = 그다음 (부리도 주황이라 머리 앞 것만 다리로 센다)
    body, head = [k for k, p in enumerate(parts) if p.grow == 1 and p.base != draw_farm.BILL][:2]
    legs = [k for k, p in enumerate(parts[:head]) if p.base == draw_farm.BILL]
    assert legs and body > max(legs), (anim, stage, legs, body)

def test_duck_scarf_follows_the_neck_when_the_head_drops(out):
    """어른 오리 목도리: 잘 때 머리가 내려가면 목도리도 따라 내려간다 (제자리에 남으면 얼굴에 걸친다)."""
    meta = json.loads((out / "duck" / "pet.json").read_text(encoding="utf-8"))
    fs = meta["frame_sizes"].get("adult", meta["frame_size"])
    sheet = np.asarray(Image.open(out / "duck" / "sheet_adult.png").convert("RGBA")).astype(int)

    def scarf_y(anim):
        r = meta["animations"][anim]["row"]
        f = sheet[r * fs:(r + 1) * fs, :fs]
        m = (np.abs(f[..., :3] - np.array(draw_farm.SCARF)).sum(axis=2) < 30) & (f[..., 3] > 200)
        # 머리를 묻어 목이 거의 안 보이면 목도리도 거의 가린다 (그것도 괜찮다)
        return m.nonzero()[0].mean() if m.sum() > 50 else None

    asleep, awake = scarf_y("sleep"), scarf_y("idle")
    assert awake is not None
    assert asleep is None or asleep - awake >= fs * 0.05
