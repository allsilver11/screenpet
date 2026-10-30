"""공룡 셋 시트 검사: 모든 동작·투명 여백·pet.json (대사·성격·좋아하는 음식·선택 동작 순서)·eat에 밥그릇 없음·색 (같은 그림, 다른 팔레트)."""
import json

import numpy as np
import pytest
from PIL import Image

import draw_dino
import draw_pets
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, SPEECH, STAGES

IDS = ["dino_yellow", "dino_green", "dino_purple"]
BOWL_COLORS = [(246, 196, 150), (252, 226, 196), (230, 222, 206)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in draw_dino.DINOS:
        draw_pets.write_species(sp, root / sp.id)
    return root


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def names(sp):
    return [a for a, *_ in draw_pets.rows_for(sp)]


def test_registered_with_dino_anims_after_every_other_optional_anim():
    optional = [a for a, *_ in OPTIONAL_ANIMS]
    # 공룡 동작은 선택 동작 목록 맨 끝 (Rust manifest::OPTIONAL_ANIMS와 같은 순서)
    assert optional[-3:] == ["fire", "roar", "stomp"]
    for pid in IDS:
        sp = draw_pets.SPECIES[pid]
        assert sp in draw_dino.DINOS
        extra = ["lookup"] if "lookup" in optional else []
        assert names(sp) == [a for a, *_ in ANIMS] + extra + ["fire", "roar", "stomp"]
        assert all(a in sp.poses for a in names(sp))


@pytest.mark.parametrize("pid", IDS)
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
                if anim == "roll":
                    continue
                x0, y0, x1, y1 = box
                assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, anim, c, box)
    assert (out / pid / "sheet.png").read_bytes() == (out / pid / "sheet_teenager.png").read_bytes()


def test_manifests(out):
    ms = {pid: json.loads((out / pid / "pet.json").read_text(encoding="utf-8")) for pid in IDS}
    assert [ms[p]["name"] for p in IDS] == ["노란 공룡", "초록 공룡", "보라 공룡"]
    assert [ms[p]["favorite_food"] for p in IDS] == [{"emoji": "🍉", "name": "수박"}, {"emoji": "🥦", "name": "브로콜리"},
                                                     {"emoji": "🍇", "name": "포도"}]
    assert [ms[p]["personality"] for p in IDS] == ["playful", "friendly", "friendly"]
    for pid, m in ms.items():
        sp = m["speech"]
        assert sp["chatter"] == ["rawr!", "♪"] and sp["tickle"] == ["hehe!"] and sp["grumpy"] == ["grrr~"]
        assert sp["roar"] == ["RAWR!", "grrr~"] and sp["fire"] == ["fwoosh~"] and sp["stomp"] == ["stomp!"]
        assert sp["hungry"] == SPEECH["cat"]["hungry"]
        # 보통 집 · 걷는다 · 기어오르지 않는다 · 크기 1.0
        assert not {"house", "locomotion", "hop_share"} & set(m)
        assert not {"climb", "slide", "arch", "wag"} & set(m["animations"])
        assert set(m["stage_size"].values()) == {1.0}
        a = m["animations"]
        base = len(ANIMS) + (1 if "lookup" in a else 0)
        assert a["fire"] == {"row": base, "frames": 4, "fps": 4, "loop": False}
        assert a["roar"] == {"row": base + 1, "frames": 3, "fps": 4}
        assert a["stomp"] == {"row": base + 2, "frames": 4, "fps": 6}
        assert "spin" in a["roll"]


@pytest.mark.parametrize("pid", IDS)
def test_eat_has_no_bowl(out, pid):
    """밥그릇은 앱의 소품 창이 띄운다 — 시트 eat에는 그릇 색이 없다."""
    row = names(draw_pets.SPECIES[pid]).index("eat")
    for st in STAGES:
        sheet = Image.open(out / pid / f"sheet_{st}.png").convert("RGBA")
        for c in range(4):
            a = np.asarray(cell(sheet, row, c)).astype(int)
            for col in BOWL_COLORS:
                near = (np.abs(a[..., :3] - col).sum(axis=2) < 12) & (a[..., 3] > 200)
                assert near.sum() < 25, (st, c, col, near.sum())


def _count(im, col, tol=24):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return int(((np.abs(a[..., :3] - col).sum(axis=2) < tol) & (a[..., 3] > 200)).sum())


def test_palettes_body_spikes_and_dandelion(out):
    """같은 그림을 색만 바꿔 그린다: 몸 색이 넓게, 대비되는 가시 색도 보인다. 노랑은 Dandelion #FFD02E."""
    assert draw_dino.PALETTES["dino_yellow"]["body"] == (0xFF, 0xD0, 0x2E)
    walk = names(draw_dino.DINO_YELLOW).index("walk")
    alphas = []
    for pid in IDS:
        pal = draw_dino.PALETTES[pid]
        f = cell(Image.open(out / pid / "sheet_teenager.png"), walk, 0)
        assert _count(f, pal["body"]) > 1500, pid
        assert _count(f, pal["spike"]) > 60, pid
        alphas.append(np.asarray(f.getchannel("A")) > 128)
    # 모양은 셋이 같다
    assert (alphas[0] != alphas[1]).sum() < 40 and (alphas[0] != alphas[2]).sum() < 40


def test_baby_wears_an_egg_shell_and_adult_has_the_biggest_spikes(out):
    sit = names(draw_dino.DINO_GREEN).index("sit_front")
    baby = cell(Image.open(out / "dino_green" / "sheet_baby.png"), sit, 0)
    teen = cell(Image.open(out / "dino_green" / "sheet_teenager.png"), sit, 0)
    assert _count(baby, draw_dino.SHELL, 10) > 80 and _count(teen, draw_dino.SHELL, 10) < 10
    walk = names(draw_dino.DINO_GREEN).index("walk")
    spike = draw_dino.PALETTES["dino_green"]["spike"]
    counts = [_count(cell(Image.open(out / "dino_green" / f"sheet_{st}.png"), walk, 0), spike) for st in STAGES]
    assert counts[-1] == max(counts) and counts[0] < counts[2] < counts[-1], counts


def test_fire_puffs_a_flame_inside_the_cell(out):
    """불꽃은 퐁·화르륵 프레임에만 (흡·흐뭇엔 없다), 입 앞 (몸 오른쪽)에."""
    row = names(draw_dino.DINO_PURPLE).index("fire")
    for st in STAGES:
        sheet = Image.open(out / "dino_purple" / f"sheet_{st}.png")
        fl = [_count(cell(sheet, row, c), draw_dino.FLAME, 30) for c in range(4)]
        assert fl[0] == 0 and fl[3] == 0 and fl[1] > 15 and fl[2] > fl[1], (st, fl)
        a = np.asarray(cell(sheet, row, 2).convert("RGBA")).astype(int)
        xs = np.nonzero((np.abs(a[..., :3] - draw_dino.FLAME).sum(axis=2) < 30) & (a[..., 3] > 200))[1]
        assert xs.min() > 64, st
