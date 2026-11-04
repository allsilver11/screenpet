"""개구리 시트 검사: 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·뒷모습 앞발은 머리 뒤."""
import json

import pytest
from PIL import Image

import draw_frog
import draw_pets
from sprite_spec import ANIMS, FRAME, STAGES, sheet_128

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
    assert names == [a for a, *_ in ANIMS] + ["climb", "slide", "bigjump", "lookup"]
    assert all(a in draw_frog.FROG_POSES for a in names)


def test_sheets_have_every_frame_inside_the_cell(out):
    for st in STAGES:
        sheet = sheet_128(out / f"sheet_{st}.png")
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
    assert "ribbit!" in m["speech"]["chatter"] and "hop hop!" in m["speech"]["chatter"] and m["walk_speed"] == 90
    assert m["stages"] == STAGES and all(v == 0.85 for v in m["stage_size"].values())
    a = m["animations"]
    assert all(x in a for x, *_ in ANIMS)
    assert a["hop"] == {"row": 6, "frames": 3, "fps": 10, "loop": False}
    assert a["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6}
    assert a["slide"] == {"row": len(ANIMS) + 1, "frames": 2, "fps": 8}
    assert a["bigjump"] == {"row": len(ANIMS) + 2, "frames": 4, "fps": 8, "loop": False}


def test_eat_has_no_bowl_and_shows_a_tongue(out):
    sheet = sheet_128(out / "sheet_teenager.png").convert("RGBA")
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
    sheet = sheet_128(out / "sheet_teenager.png")
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


def test_back_view_feet_are_behind_the_body(monkeypatch):
    """뒷모습: 뒷발·몸·머리 순서 — 발은 몸 뒤에 숨어 엉덩이를 가리지 않는다."""
    tags, orders = {}, []
    compose = draw_pets.compose

    def tagged(kind, fn):
        def f(*a, **k):
            r = fn(*a, **k)
            tags[id(r)] = kind
            return r
        return f
    monkeypatch.setattr(draw_frog, "frog_back_foot", tagged("leg", draw_frog.frog_back_foot))
    monkeypatch.setattr(draw_frog, "frog_back_body", tagged("body", draw_frog.frog_back_body))

    def spy(parts, *a, **k):
        orders.append([tags.get(id(p)) for p in parts])
        tags.clear()
        return compose(parts, *a, **k)
    monkeypatch.setattr(draw_pets, "compose", spy)
    for st in STAGES:
        with draw_pets.stage(st, draw_frog.FROG.stage_tweak.get(st)):
            draw_pets.frames(draw_frog.FROG, "climb", 4)
            draw_pets.frames(draw_frog.FROG, "slide", 2)
    assert len(orders) == 6 * len(STAGES)
    for order in orders:
        assert order.count("leg") == 2 and max(i for i, t in enumerate(order) if t == "leg") < order.index("body"), order


def test_back_view_shows_no_suction_pads_on_the_rump():
    """시각 의도: 발바닥 빨판은 벽을 향한다 — 아래쪽 절반에 빨판 색 픽셀이 거의 없다."""
    pad = draw_frog._pad(draw_frog.FROG)
    for st in STAGES:
        with draw_pets.stage(st, draw_frog.FROG.stage_tweak.get(st)):
            got = draw_pets.frames(draw_frog.FROG, "climb", 4) + draw_pets.frames(draw_frog.FROG, "slide", 2)
        for n, im in enumerate(got):
            count = 0
            for y in range(80, FRAME):
                for x in range(FRAME):
                    p = im.getpixel((x, y))
                    count += p[3] > 128 and abs(p[0] - pad[0]) + abs(p[1] - pad[1]) + abs(p[2] - pad[2]) < 24
            assert count <= 8, (st, n, count)  # 아기 꼬리 끝 몇 점은 허용


def test_climb_feet_only_peek_and_take_turns(monkeypatch):
    for st in STAGES:
        with draw_pets.stage(st, draw_frog.FROG.stage_tweak.get(st)):
            full = draw_pets.frames(draw_frog.FROG, "climb", 4)
            with monkeypatch.context() as m:
                m.setattr(draw_frog, "frog_back_foot", lambda sp, *a, **k: draw_pets.Part([], sp.base))
                bare = draw_pets.frames(draw_frog.FROG, "climb", 4)
        seen = []
        for f, b in zip(full, bare):
            left = right = 0
            for y in range(FRAME):
                for x in range(FRAME):
                    if sum(abs(p - q) for p, q in zip(f.getpixel((x, y)), b.getpixel((x, y)))) > 60:
                        if x < FRAME // 2:
                            left += 1
                        else:
                            right += 1
            seen.append((left, right))
        areas = [sum(f.getchannel("A").histogram()[129:]) for f in full]
        for n, ((l, r), area) in enumerate(zip(seen, areas)):
            assert 0 < l + r < 0.09 * area, (st, n, l, r, area)
        assert seen[0][1] > seen[0][0] and seen[2][0] > seen[2][1], (st, seen)


def test_back_view_is_not_cut_by_the_cell_or_nudged_between_frames():
    """뒷모습 원본이 칸 끝에 닿으면 손끝·꼬리가 잘리고 _fit이 그 프레임만 밀어 튄다."""
    sp = draw_frog.FROG
    for st in STAGES:
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            with draw_pets.view(scale=sp.scale * draw_pets._ST["fit"], lift=draw_pets.LIFT + 1.7 * draw_pets.OUTLINE * (draw_pets._ST["line"] - 1)):
                raw = [draw_frog.frog_climb(sp, i) for i in range(4)] + [draw_frog.frog_slide(sp, i) for i in range(2)]
        for n, im in enumerate(raw):
            x0, y0, x1, y1 = im.getchannel("A").getbbox()
            assert x0 >= 1 and y0 >= 1 and x1 <= FRAME - 1 and y1 <= FRAME - 1, (st, n, (x0, y0, x1, y1))
