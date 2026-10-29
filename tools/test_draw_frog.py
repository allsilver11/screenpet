"""개구리 시트 검사: 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·뒷모습 앞발은 머리 뒤."""
import json

import pytest
from PIL import Image

import draw_frog
import draw_pets
from sprite_spec import ANIMS, FRAME, STAGES

GROUNDED = {"walk", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "eat", "level_up"}
# 밥그릇·밥알 색 (draw_pets의 옛 bowl_parts) — 흰 밥(거의 흰색)은 눈 반짝이와 겹쳐 빼고 본다
BOWL_COLORS = [(246, 196, 150), (252, 226, 196), (230, 222, 206)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    d = tmp_path_factory.mktemp("pets") / "frog"
    draw_pets.write_species(draw_frog.FROG, d)
    return d


def rows():
    return draw_pets.rows_for(draw_frog.FROG)


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def test_frog_is_registered_with_its_own_poses():
    assert draw_pets.SPECIES["frog"] is draw_frog.FROG
    names = [a for a, *_ in rows()]
    assert names == [a for a, *_ in ANIMS] + ["climb", "slide", "bigjump"]
    assert all(a in draw_frog.FROG_POSES for a in names)


def test_sheets_have_every_frame_inside_the_cell(out):
    for st in STAGES:
        sheet = Image.open(out / f"sheet_{st}.png")
        assert sheet.mode == "RGBA"
        assert sheet.size == (4 * FRAME, len(rows()) * FRAME), st
        for r, (anim, n, *_) in enumerate(rows()):
            for c in range(n):
                bbox = cell(sheet, r, c).getchannel("A").getbbox()
                assert bbox is not None, (st, anim, c)
                assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, (st, anim, c, bbox)
                if anim in GROUNDED:
                    assert bbox[3] >= 118, (st, anim, c, bbox)
            for c in range(n, 4):  # 빈 칸은 완전히 투명
                assert cell(sheet, r, c).getchannel("A").getbbox() is None, (st, anim, c)
    assert (out / "sheet.png").read_bytes() == (out / "sheet_teenager.png").read_bytes()


def test_manifest(out):
    m = json.loads((out / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "개구리" and m["locomotion"] == "hop" and m["personality"] == "playful"
    assert m["favorite_food"] == {"emoji": "🪰", "name": "파리"}
    assert "개굴!" in m["speech"]["chatter"] and "폴짝!" in m["speech"]["chatter"] and m["walk_speed"] == 90
    assert m["stages"] == STAGES and all(v == 0.85 for v in m["stage_size"].values())
    a = m["animations"]
    assert all(x in a for x, *_ in ANIMS)
    assert a["hop"] == {"row": 6, "frames": 3, "fps": 10, "loop": False}
    assert a["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6}
    assert a["slide"] == {"row": len(ANIMS) + 1, "frames": 2, "fps": 8}
    assert a["bigjump"] == {"row": len(ANIMS) + 2, "frames": 4, "fps": 8, "loop": False}


def test_eat_has_no_bowl_and_shows_a_tongue(out):
    sheet = Image.open(out / "sheet_teenager.png").convert("RGBA")
    row = [a for a, *_ in rows()].index("eat")
    tongue = bowlish = 0
    for c in range(4):
        px = cell(sheet, row, c).load()
        for y in range(FRAME):
            for x in range(FRAME):
                p = px[x, y]
                if p[3] < 200:
                    continue
                if any(sum(abs(p[k] - col[k]) for k in range(3)) <= 12 for col in BOWL_COLORS):
                    bowlish += 1
                if c == 1 and 78 <= y <= 96 and 54 <= x <= 80 and sum(abs(p[k] - draw_pets.TONGUE[k]) for k in range(3)) < 20:
                    tongue += 1
    assert bowlish < 20, bowlish  # 그릇이 있으면 수백 픽셀 (안티에일리어싱 몇 점만 허용)
    assert tongue > 15, tongue  # 혀 끝이 앞-아래 — 앱 소품 창의 밥 더미 위 (x 36–92, y ≈ 82)


def test_hop_lifts_off_and_bigjump_goes_higher(out):
    sheet = Image.open(out / "sheet_teenager.png")
    names = [a for a, *_ in rows()]
    hop = cell(sheet, names.index("hop"), 1).getchannel("A").getbbox()
    sit = cell(sheet, names.index("idle"), 0).getchannel("A").getbbox()
    big = cell(sheet, names.index("bigjump"), 2).getchannel("A").getbbox()
    assert hop[3] < sit[3] - 2, (hop, sit)  # 공중에 뜸
    assert big[3] < hop[3], (big, hop)


def test_baby_has_a_tadpole_tail_and_others_do_not():
    for st, has in (("baby", True), ("kinder", False), ("adult", False)):
        with draw_pets.stage(st, draw_frog.FROG.stage_tweak.get(st)):
            assert bool(draw_frog._tail_nub(draw_frog.FROG, (26, 100))) == has, st


def test_back_view_hands_are_behind_the_head(monkeypatch):
    orders = []
    compose = draw_pets.compose

    def spy(parts, *a, **k):
        orders.append(parts)
        return compose(parts, *a, **k)
    monkeypatch.setattr(draw_pets, "compose", spy)
    with draw_pets.stage("teenager"):
        draw_pets.frames(draw_frog.FROG, "climb", 1)
    parts = orders[0]
    head = next(i for i, p in enumerate(parts) if p.outline and len(p.shapes) == 3)  # 머리 + 눈 언덕 둘
    assert head >= 3  # 앞발 둘 + 몸 다음
