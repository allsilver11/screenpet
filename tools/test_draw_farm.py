"""오리·기린 시트 검사: 모든 동작·투명 여백·pet.json·eat에 밥그릇 없음·기린 목 늘이기 층 (구르지 않음)."""
import json

import numpy as np
import pytest
from PIL import Image

import draw_farm
import draw_pets
from sprite_spec import ANIMS, FRAME, STAGES

BOWL_COLORS = [(246, 196, 150), (252, 226, 196), (230, 222, 206)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in (draw_farm.DUCK, draw_farm.GIRAFFE):
        draw_farm.write_farm(sp, root / sp.id)
    return root


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def names(sp):
    return [a for a, *_ in draw_pets.rows_for(sp)]


def test_registered_with_their_own_poses():
    assert draw_pets.SPECIES["duck"] is draw_farm.DUCK and draw_pets.SPECIES["giraffe"] is draw_farm.GIRAFFE
    assert names(draw_farm.DUCK) == [a for a, *_ in ANIMS]
    assert names(draw_farm.GIRAFFE) == [a for a, *_ in ANIMS] + ["neckstretch"]
    for sp in (draw_farm.DUCK, draw_farm.GIRAFFE):
        assert all(a in sp.poses for a in names(sp))


@pytest.mark.parametrize("pid", ["duck", "giraffe"])
def test_every_frame_is_drawn_inside_the_cell(out, pid):
    sp = draw_pets.SPECIES[pid]
    rows = draw_pets.rows_for(sp)
    for st in STAGES:
        sheet = Image.open(out / pid / f"sheet_{st}.png")
        assert sheet.size == (4 * FRAME, len(rows) * FRAME)
        for r, (anim, n, *_) in enumerate(rows):
            for c in range(n):
                box = cell(sheet, r, c).getchannel("A").getbbox()
                assert box, (st, anim, c)
                if anim in ("roll", "neckstretch"):
                    continue
                x0, y0, x1, y1 = box
                assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, box)
    assert (out / pid / "sheet.png").read_bytes() == (out / pid / "sheet_teenager.png").read_bytes()


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
    assert gir["stage_size"]["teenager"] == 1.1 and gir["stage_size"]["baby"] == 1.0
    # 기린은 구르지 않는다: neckstretch 동작 (코어 Special::NeckStretch), roll은 돌지 않는다
    assert gir["animations"]["neckstretch"] == {"row": 21, "frames": 3, "fps": 2}
    assert "spin" not in gir["animations"]["roll"] and "spin" in duck["animations"]["roll"]
    assert "neckstretch" not in duck["animations"]


@pytest.mark.parametrize("pid", ["duck", "giraffe"])
def test_eat_has_no_bowl(out, pid):
    """밥그릇은 앱의 소품 창이 띄운다 — 시트 eat에는 그릇 색이 없다."""
    row = names(draw_pets.SPECIES[pid]).index("eat")
    for st in STAGES:
        sheet = Image.open(out / pid / f"sheet_{st}.png").convert("RGBA")
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
        m = meta[st]
        assert m["tile"] == draw_farm.NECK_TILE and m["ext"] % m["tile"] == 0
        assert 40 < m["cut"] < 110, m
        # 목을 두 배쯤 (지금 목 길이를 토막 배수로) — 앱은 창을 위로 키워 담는다 (src-tauri slots STRETCH_SPACE)
        assert 16 <= m["ext"] <= 56, m
        strip = Image.open(out / "giraffe" / f"neck_{st}.png")
        tile = Image.open(out / "giraffe" / f"neck_tile_{st}.png")
        assert strip.size == (4 * FRAME, FRAME) and tile.size == (FRAME, m["tile"])
        sheet = Image.open(out / "giraffe" / f"sheet_{st}.png")
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
