from pathlib import Path

from PIL import Image

import draw_pets
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, ROLL_CENTER, ROLL_RADIUS

STANDING = {"walk", "run", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "eat", "level_up", "huff"}
SPECIES_IDS = ("cat", "dog", "rabbit", "bear", "bee")
# 꿀벌은 날아다녀서 떠 있는 동작이 많다 — 바닥에 내려앉는 동작만 발이 닿는지 본다
BEE_GROUNDED = {"sleep", "annoyed", "land"}  # eat: 꽃(앱 소품) 위에 낮게 떠서 마신다


def grounded(pid, anim):
    return anim in (BEE_GROUNDED if pid == "bee" else STANDING)
# 전체 검사는 대표 단계만 (5단계 모두는 너무 오래 걸린다) — 다른 단계는 test_stage_sheets_and_manifest가 파일 존재를 본다
CHECK_STAGES = ("baby", "teenager", "adult")


def test_sleep_is_lying_flat(tmp_path):
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sleep")
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            cell = Image.open(tmp_path / pid / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
            # z 글자는 제외하고 몸만: 불투명한 큰 덩어리의 위쪽이 낮아야 한다
            alpha = cell.getchannel("A").point(lambda v: 255 if v > 200 else 0)
            body_top = min(y for y in range(FRAME) if sum(alpha.getpixel((x, y)) > 0 for x in range(FRAME)) >= 20)
            assert body_top >= 70, f"{pid} {st} sleep body top {body_top} — 엎드린 자세가 아님"


def test_every_species_every_animation(tmp_path):
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            sheet = Image.open(tmp_path / pid / f"sheet_{st}.png")
            # 필수 21행 + 그 종의 선택 동작 행 (고양이 arch·huff·climb·slide, 강아지 wag, 곰 climb·slide)
            rows = list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in draw_pets.SPECIES[pid].specials]
            assert sheet.height == FRAME * len(rows)
            for row, (anim, n, *_) in enumerate(rows):
                for col in range(n):
                    cell = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))
                    bbox = cell.getchannel("A").getbbox()
                    assert bbox is not None, f"{pid} {st} {anim} {col} empty"
                    assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, \
                        f"{pid} {st} {anim} {col} clipped {bbox}"
                    if grounded(pid, anim):
                        assert bbox[3] >= 118, f"{pid} {st} {anim} {col} floats (bottom {bbox[3]})"


def test_only_cat_has_arch_and_huff_and_only_dog_has_wag(tmp_path):
    import json
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert ("arch" in m["animations"]) == (pid == "cat"), pid
        assert ("huff" in m["animations"]) == (pid == "cat"), pid
        assert ("wag" in m["animations"]) == (pid == "dog"), pid
        assert m.get("personality") == {"cat": "aloof", "bee": "friendly"}.get(pid), pid
    m = json.loads((tmp_path / "dog" / "pet.json").read_text(encoding="utf-8"))
    assert m["animations"]["wag"]["row"] == len(ANIMS)
    assert m["animations"]["wag"]["front"] is True
    m = json.loads((tmp_path / "cat" / "pet.json").read_text(encoding="utf-8"))
    assert m["animations"]["arch"]["row"] == len(ANIMS)
    assert m["animations"]["huff"] == {"row": len(ANIMS) + 1, "frames": 3, "fps": 4, "front": True}
    assert m["speech"]["hiss"] == ["hiss!"]
    assert m["speech"]["ignore"] == ["..."]
    assert m["personality"] == "aloof"
    sheet = Image.open(tmp_path / "cat" / "sheet.png")
    assert sheet.height == FRAME * (len(ANIMS) + 4)  # arch, huff, climb, slide


def test_roll_ball_fits_rotation_circle(tmp_path):
    draw_pets.build(tmp_path)
    cx, cy = ROLL_CENTER
    row = [a for a, *_ in ANIMS].index("roll")
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            cell = Image.open(tmp_path / pid / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
            alpha = cell.getchannel("A")
            for y in range(FRAME):
                for x in range(FRAME):
                    if alpha.getpixel((x, y)) > 40:
                        assert (x - cx) ** 2 + (y - cy) ** 2 <= (ROLL_RADIUS + 1) ** 2, f"{pid} {st} roll pixel ({x},{y}) outside circle"


def test_stage_sheets_and_manifest(tmp_path):
    import json
    from sprite_spec import STAGES
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        for st in STAGES:
            assert (tmp_path / pid / f"sheet_{st}.png").is_file(), (pid, st)
        assert (tmp_path / pid / "sheet.png").read_bytes() == (tmp_path / pid / "sheet_teenager.png").read_bytes()
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert m["stages"] == STAGES
        assert m["favorite_food"]["emoji"]
        for key in ("hungry", "full", "level_up"):
            assert m["speech"][key], (pid, key)
        assert list(m["stage_size"]) == STAGES, pid
    size = {pid: json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))["stage_size"] for pid in SPECIES_IDS}
    assert size["bear"]["baby"] == size["bear"]["kinder"] == 1.0
    assert all(size["bear"][st] > 1.0 for st in ("child", "teenager", "adult"))
    assert all(v == 0.85 for v in size["rabbit"].values())
    assert all(v == 1.0 for pid in ("cat", "dog") for v in size[pid].values())
    assert all(v == 0.8 for v in size["bee"].values())


def test_baby_head_is_relatively_bigger_than_adult(tmp_path):
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sit_front")
    def head_ratio(st):
        cell = Image.open(tmp_path / "cat" / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        a = cell.getchannel("A").point(lambda v: 255 if v > 128 else 0)
        widths = [sum(1 for x in range(FRAME) if a.getpixel((x, y))) for y in range(FRAME)]
        top = next(y for y, w in enumerate(widths) if w)
        bottom = max(y for y, w in enumerate(widths) if w)
        mid = top + (bottom - top) // 2
        # 위 절반(머리) 폭 / 아래 30%(몸·발) 폭 — 카와이 비율은 모든 단계에서 머리가 키의 절반을 넘어서
        # '아래 절반'에는 머리 아랫부분이 들어가므로 몸통 쪽만 잰다 (brief의 아래 절반에서 바꿈)
        low = bottom - (bottom - top) * 3 // 10
        return max(widths[top:mid]) / max(1, max(widths[low:bottom + 1]))
    assert head_ratio("baby") > head_ratio("teenager") > head_ratio("adult")


def test_only_cat_and_bear_have_climb_and_slide(tmp_path):
    import json
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        for anim in ("climb", "slide"):
            assert (anim in m["animations"]) == (pid in ("cat", "bear")), (pid, anim)
    cat = json.loads((tmp_path / "cat" / "pet.json").read_text(encoding="utf-8"))["animations"]
    bear = json.loads((tmp_path / "bear" / "pet.json").read_text(encoding="utf-8"))["animations"]
    assert cat["climb"] == {"row": len(ANIMS) + 2, "frames": 4, "fps": 6}  # 뒷모습: front 아님 (앱은 반전하지 않음 — Task 4b)
    assert cat["slide"] == {"row": len(ANIMS) + 3, "frames": 2, "fps": 8}
    assert bear["climb"]["row"] == len(ANIMS) and bear["slide"]["row"] == len(ANIMS) + 1


def test_climb_and_slide_fit_cell_in_every_stage(tmp_path):
    from sprite_spec import STAGES
    draw_pets.build(tmp_path)
    for pid in ("cat", "bear"):
        names = [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]
        for st in STAGES:
            sheet = Image.open(tmp_path / pid / f"sheet_{st}.png")
            for anim, n in (("climb", 4), ("slide", 2)):
                row = names.index(anim)
                for col in range(n):
                    bbox = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME)).getchannel("A").getbbox()
                    assert bbox is not None, (pid, st, anim, col)
                    assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, (pid, st, anim, col, bbox)


def test_baby_eyes_are_relatively_bigger_than_adult(tmp_path):
    """귀여운 아기: sit_front 첫 장의 눈동자 픽셀 수 / 머리 폭² 이 아기 > 어른 (모든 종)."""
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sit_front")

    def eye_ratio(pid, st):
        cell = Image.open(tmp_path / pid / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME)).convert("RGBA")
        px = cell.load()
        er, eg, eb = draw_pets.EYE
        dark = sum(1 for y in range(FRAME) for x in range(FRAME)
                   if px[x, y][3] > 200 and abs(px[x, y][0] - er) + abs(px[x, y][1] - eg) + abs(px[x, y][2] - eb) < 30)
        a = cell.getchannel("A").point(lambda v: 255 if v > 128 else 0)
        head_w = max(sum(1 for x in range(FRAME) if a.getpixel((x, y))) for y in range(FRAME // 2))
        return dark / head_w ** 2

    for pid in SPECIES_IDS:
        assert eye_ratio(pid, "baby") > 1.5 * eye_ratio(pid, "adult"), pid


def test_bee_flies_and_only_bee_has_sting(tmp_path):
    import json
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert ("sting" in m["animations"]) == (pid == "bee"), pid
        assert m.get("locomotion") == ("fly" if pid == "bee" else None), pid
    m = json.loads((tmp_path / "bee" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "꿀벌" and m["walk_speed"] == 50
    assert m["animations"]["sting"] == {"row": len(ANIMS), "frames": 3, "fps": 8, "loop": False, "front": True}
    assert all(a in m["animations"] for a, *_ in ANIMS)
    assert m["speech"]["chatter"] == ["bzz~", "♪"] and "sting" not in m["speech"]
    assert m["favorite_food"] == {"emoji": "🌸", "name": "꽃꿀"}
    sheet = Image.open(tmp_path / "bee" / "sheet.png")
    assert sheet.height == FRAME * (len(ANIMS) + 1)


def test_bee_hovers_above_the_ground_when_flying(tmp_path):
    """떠 있는 꿀벌: walk·idle·sit_front는 바닥(y 127)에서 떨어져 있다 (날고 있는 게 보인다)."""
    draw_pets.build(tmp_path)
    names = [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES["bee"])]
    sheet = Image.open(tmp_path / "bee" / "sheet_teenager.png")
    for anim in ("walk", "idle", "sit_front"):
        row = names.index(anim)
        bbox = sheet.crop((0, row * FRAME, FRAME, (row + 1) * FRAME)).getchannel("A").getbbox()
        assert bbox[3] <= 116, (anim, bbox)


def test_bee_wings_are_translucent(tmp_path):
    """날개는 반투명 층: walk 첫 장에 반투명(알파 60~200) 픽셀이 많다."""
    draw_pets.build(tmp_path)
    cell = Image.open(tmp_path / "bee" / "sheet_teenager.png").crop((0, 0, FRAME, FRAME))
    semi = sum(cell.getchannel("A").histogram()[60:201])
    assert semi > 150, semi


def test_sting_effect_is_a_centered_stinger_with_cracks():
    img = draw_pets.sting_effect()
    assert img.size == (FRAME, FRAME) and img.mode == "RGBA"
    a = img.getchannel("A")
    for p in ((0, 0), (FRAME - 1, 0), (0, FRAME - 1), (FRAME - 1, FRAME - 1)):
        assert a.getpixel(p) == 0
    bbox = a.getbbox()
    assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, bbox
    # 가운데 근처에 짙은 침 끝
    px = img.load()
    dark = [(x, y) for y in range(54, 70) for x in range(50, 68) if px[x, y][3] > 200 and sum(px[x, y][:3]) < 330]
    assert len(dark) > 15, len(dark)
    # 가운데에서 먼 곳(반지름 30 밖)까지 뻗은 금
    far = sum(1 for y in range(FRAME) for x in range(FRAME) if px[x, y][3] > 100 and (x - 64) ** 2 + (y - 64) ** 2 > 30 ** 2)
    assert far > 60, far


def test_back_view_paws_are_drawn_behind_the_head(monkeypatch):
    """뒷모습 climb·slide: 앞발은 벽 쪽 — 머리·몸보다 먼저(뒤에) 그려서 발끝만 머리 옆·위로 빼꼼 보인다."""
    from sprite_spec import STAGES
    tags, orders = {}, []
    arm, head, compose = draw_pets.back_arm, draw_pets.back_head, draw_pets.compose

    def tagged(kind, fn):
        def f(*a, **k):
            r = fn(*a, **k)
            tags[id(r[0] if kind == "arm" else r)] = kind
            return r
        return f
    monkeypatch.setattr(draw_pets, "back_arm", tagged("arm", arm))
    monkeypatch.setattr(draw_pets, "back_head", tagged("head", head))
    def spy(parts, *a, **k):
        orders.append([tags.get(id(p)) for p in parts])
        tags.clear()  # id는 다음 프레임에서 다시 쓰일 수 있다
        return compose(parts, *a, **k)
    monkeypatch.setattr(draw_pets, "compose", spy)
    for pid in ("cat", "bear"):
        sp = draw_pets.SPECIES[pid]
        for st in (STAGES[0], STAGES[-1]):
            with draw_pets.stage(st, sp.stage_tweak.get(st)):
                draw_pets.frames(draw_pets.staged_species(sp), "climb", 1)
                draw_pets.frames(draw_pets.staged_species(sp), "slide", 1)
    assert len(orders) == 8
    for order in orders:
        assert order.count("arm") == 2 and order.index("head") > max(i for i, t in enumerate(order) if t == "arm"), order


def test_bear_is_a_deeper_brown_with_bigger_ears():
    assert sum(draw_pets.BEAR.base) < sum((190, 132, 90))
    assert draw_pets.BEAR.outline is not None and sum(draw_pets.BEAR.outline) < sum(draw_pets.BEAR.base)
    for st in ("baby", "teenager", "adult"):
        assert draw_pets.BEAR.stage_tweak[st]["ear"] > draw_pets.STAGE_PROPS[st]["ear"] * 1.1


def _eat_cell(tmp_path, pid, st, col):
    names = [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]
    row = names.index("eat")
    return Image.open(tmp_path / pid / f"sheet_{st}.png").convert("RGBA").crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def test_eat_mouth_meets_the_real_bowl(tmp_path):
    """그릇은 앱의 소품 창 (프레임 x 36–92, 밥 더미 위 y ≈ 82): 먹는 입(벌린 입 속 색)이 그 바로 위 가운데에 온다 — 모든 단계."""
    from sprite_spec import STAGES
    draw_pets.build(tmp_path)
    for pid in ("cat", "dog", "rabbit", "bear"):
        for st in STAGES:
            px = _eat_cell(tmp_path, pid, st, 0).load()
            mr, mg, mb = draw_pets.MOUTH_IN
            mouth = [(x, y) for y in range(FRAME) for x in range(FRAME)
                     if px[x, y][3] > 200 and abs(px[x, y][0] - mr) + abs(px[x, y][1] - mg) + abs(px[x, y][2] - mb) < 30]
            assert mouth, (pid, st)
            ys, xs = [y for _, y in mouth], [x for x, _ in mouth]
            assert 72 <= min(ys) and max(ys) <= 90, (pid, st, min(ys), max(ys))
            assert 54 <= min(xs) and max(xs) <= 74, (pid, st, min(xs), max(xs))


def test_eat_draws_no_bowl_or_flower(tmp_path):
    """그릇·꽃은 앱이 따로 띄운다 — 먹는 프레임에 초록 줄기·잎이나 그릇 테두리 색이 없다.
    개구리는 몸이 초록이라 여기서 빼고 test_draw_frog.py(eat에 밥그릇 없음)가 따로 본다."""
    draw_pets.build(tmp_path)
    assert "frog" in draw_pets.SPECIES
    for pid in (p for p in SPECIES_IDS if p != "frog"):
        for st in CHECK_STAGES:
            for col in range(4):
                px = _eat_cell(tmp_path, pid, st, col).load()
                green = sum(1 for y in range(FRAME) for x in range(FRAME)
                            if px[x, y][3] > 200 and px[x, y][1] > px[x, y][0] + 30 and px[x, y][1] > px[x, y][2] + 30)
                assert green == 0, (pid, st, col, green)


def test_bee_sips_with_a_thick_straw_into_the_flower(tmp_path):
    """꿀벌 eat: 입에서 꽃술(64, 100) 쪽으로 내려가는 꿀색 빨대가 아기 크기에서도 굵게 보인다."""
    draw_pets.build(tmp_path)
    for st in ("baby", "teenager"):
        px = _eat_cell(tmp_path, "bee", st, 0).load()
        straw = [(x, y) for y in range(84, 99) for x in range(58, 72)
                 if px[x, y][3] > 200 and px[x, y][0] > 200 and 120 <= px[x, y][1] <= 165 and px[x, y][2] < 90]
        assert len(straw) >= 18, (st, len(straw))
        rows = {y for _, y in straw}
        assert max(rows) - min(rows) >= 8, (st, sorted(rows))
