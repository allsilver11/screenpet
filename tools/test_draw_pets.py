import json
from pathlib import Path

import pytest
from PIL import Image

import draw_pets
from conftest import assert_eat_has_no_bowl, assert_feet_only_peek, record_draw_order
from sprite_spec import ANIMS, FRAME, HIRES_TEENAGER, NO_SPIN, OPTIONAL_ANIMS, ROLL_CENTER, ROLL_RADIUS, STAGES as STAGE_NAMES, frame_px, sheet_128

STANDING = {"walk", "run", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "eat", "level_up", "huff", "lookup"}
SPECIES_IDS = ("cat", "rabbit")
GONE = ("bear", "bee", "dog")  # v0.5.5에서 빠진 동물


def grounded(pid, anim):
    return anim in STANDING
# 전체 검사는 대표 단계만 (5단계 모두는 너무 오래 걸린다) — 다른 단계는 test_stage_sheets_and_manifest가 파일 존재를 본다
CHECK_STAGES = ("baby", "teenager", "adult")


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    """한 파일이 한 번만, 이 파일이 보는 종만 그린다 (한 종에 1분쯤 걸린다, 나머지는 test_draw_<종>.py가 그린다). 테스트는 읽기만 한다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {pid: draw_pets.SPECIES[pid] for pid in SPECIES_IDS})
        draw_pets.build(root)
    return root


def test_sleep_is_lying_flat(built):
    row = [a for a, *_ in ANIMS].index("sleep")
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            cell = sheet_128(built / pid / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
            # z 글자는 제외하고 몸만: 불투명한 큰 덩어리의 위쪽이 낮아야 한다
            alpha = cell.getchannel("A").point(lambda v: 255 if v > 200 else 0)
            body_top = min(y for y in range(FRAME) if sum(alpha.getpixel((x, y)) > 0 for x in range(FRAME)) >= 20)
            assert body_top >= 70, f"{pid} {st} sleep body top {body_top} — 엎드린 자세가 아님"


def test_every_species_every_animation(built):
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            sheet = sheet_128(built / pid / f"sheet_{st}.png")
            # 필수 21행 + 그 종의 선택 동작 행 (고양이 arch·huff·climb·slide)
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


def test_bear_bee_and_dog_are_gone():
    """곰·꿀벌·강아지는 v0.5.5에서 빠졌다: 그리지 않고, 대사·성격·좋아하는 음식·움직임·집 표에도 없고, 시트도 들어 있지 않다.
    그 동물만 쓰던 동작(wag 살랑, sting 쏘기)도 없다."""
    import sprite_spec
    root = Path(__file__).resolve().parent.parent / "ui"
    for pid in GONE:
        assert pid not in draw_pets.SPECIES, pid
        for table in (sprite_spec.SPEECH, sprite_spec.PERSONALITY, sprite_spec.FAVORITE_FOOD, sprite_spec.LOCOMOTION, sprite_spec.HOUSE):
            assert pid not in table, pid
        assert not (root / "pets" / pid).exists() and not (root / "pets_pixel" / pid).exists(), pid
    assert not {"wag", "sting"} & {a for a, *_ in OPTIONAL_ANIMS}
    for sp in draw_pets.SPECIES.values():
        assert not {"wag", "sting"} & set(sp.specials), sp.id


def test_only_cat_has_arch_and_huff(built):
    import json
    for pid in SPECIES_IDS:
        m = json.loads((built / pid / "pet.json").read_text(encoding="utf-8"))
        assert ("arch" in m["animations"]) == (pid == "cat"), pid
        assert ("huff" in m["animations"]) == (pid == "cat"), pid
        assert m.get("personality") == {"cat": "aloof"}.get(pid), pid
    m = json.loads((built / "cat" / "pet.json").read_text(encoding="utf-8"))
    assert m["animations"]["arch"]["row"] == len(ANIMS)
    assert m["animations"]["huff"] == {"row": len(ANIMS) + 1, "frames": 3, "fps": 4, "front": True}
    assert m["speech"]["hiss"] == ["hiss!"]
    assert m["speech"]["ignore"] == ["..."]
    assert m["personality"] == "aloof"
    sheet = sheet_128(built / "cat" / "sheet_teenager.png")
    assert sheet.height == FRAME * (len(ANIMS) + 5)  # arch, huff, climb, slide, lookup


def test_roll_ball_fits_rotation_circle(built):
    cx, cy = ROLL_CENTER
    row = [a for a, *_ in ANIMS].index("roll")
    for pid in SPECIES_IDS:
        for st in CHECK_STAGES:
            cell = sheet_128(built / pid / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
            alpha = cell.getchannel("A")
            for y in range(FRAME):
                for x in range(FRAME):
                    if alpha.getpixel((x, y)) > 40:
                        assert (x - cx) ** 2 + (y - cy) ** 2 <= (ROLL_RADIUS + 1) ** 2, f"{pid} {st} roll pixel ({x},{y}) outside circle"


def test_stage_sheets_and_manifest(built):
    import json
    from sprite_spec import STAGES
    for pid in SPECIES_IDS:
        for st in STAGES:
            assert (built / pid / f"sheet_{st}.png").is_file(), (pid, st)
        assert not (built / pid / "sheet.png").exists()
        m = json.loads((built / pid / "pet.json").read_text(encoding="utf-8"))
        assert m["stages"] == STAGES
        assert m["favorite_food"]["emoji"]
        for key in ("hungry", "full", "level_up"):
            assert m["speech"][key], (pid, key)
        assert list(m["stage_size"]) == STAGES, pid
        assert m.get("hop_share") == (0.5 if pid == "rabbit" else None), pid
    size = {pid: json.loads((built / pid / "pet.json").read_text(encoding="utf-8"))["stage_size"] for pid in SPECIES_IDS}
    want = {"rabbit": 0.85, "cat": 0.91}
    for pid, v in want.items():
        assert all(x == v for x in size[pid].values()), pid


def test_rabbit_manifest_keeps_hop_share():
    """토끼만 걷는 시간의 절반쯤 깡총거린다 (pet.json hop_share) — 다시 그려도 칸이 남고, 들어 있는 pet.json과 같다."""
    import json
    for pid in SPECIES_IDS + ("frog",):
        m = draw_pets.species_fields(pid, {})
        assert m.get("hop_share") == (0.5 if pid == "rabbit" else None), pid
    bundled = json.loads((Path(__file__).resolve().parent.parent / "ui/pets/rabbit/pet.json").read_text(encoding="utf-8"))
    assert bundled["hop_share"] == 0.5 and "locomotion" not in bundled


def test_manifest_house_kind():
    """호박·유령은 귀신의 집 (pet.json house) — 다시 그려도 칸이 남고, 들어 있는 pet.json과 같다. 보통 집은 적지 않는다."""
    import json
    want = {"pumpkin": "haunted", "ghost": "haunted", "rudolph": "xmas", "santa": "xmas", "snowman": "xmas"}
    pets = Path(__file__).resolve().parent.parent / "ui" / "pets"
    for d in sorted(p for p in pets.iterdir() if (p / "pet.json").is_file()):
        assert draw_pets.species_fields(d.name, {}).get("house") == want.get(d.name), d.name
        bundled = json.loads((d / "pet.json").read_text(encoding="utf-8"))
        assert bundled.get("house") == want.get(d.name), d.name


def test_baby_head_is_relatively_bigger_than_adult(built):
    row = [a for a, *_ in ANIMS].index("sit_front")
    def head_ratio(st):
        cell = sheet_128(built / "cat" / f"sheet_{st}.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
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


def test_only_cat_has_climb_and_slide(built):
    import json
    for pid in SPECIES_IDS:
        m = json.loads((built / pid / "pet.json").read_text(encoding="utf-8"))
        for anim in ("climb", "slide"):
            assert (anim in m["animations"]) == (pid == "cat"), (pid, anim)
    cat = json.loads((built / "cat" / "pet.json").read_text(encoding="utf-8"))["animations"]
    assert cat["climb"] == {"row": len(ANIMS) + 2, "frames": 4, "fps": 6}  # 뒷모습: front 아님 (앱은 반전하지 않음 — Task 4b)
    assert cat["slide"] == {"row": len(ANIMS) + 3, "frames": 2, "fps": 8}


def test_climb_and_slide_fit_cell_in_every_stage(built):
    from sprite_spec import STAGES
    for pid in ("cat",):
        names = [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]
        for st in STAGES:
            sheet = sheet_128(built / pid / f"sheet_{st}.png")
            for anim, n in (("climb", 4), ("slide", 2)):
                row = names.index(anim)
                for col in range(n):
                    bbox = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME)).getchannel("A").getbbox()
                    assert bbox is not None, (pid, st, anim, col)
                    assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, (pid, st, anim, col, bbox)


def test_baby_eyes_are_relatively_bigger_than_adult(built):
    """귀여운 아기: sit_front 첫 장의 눈동자 픽셀 수 / 머리 폭² 이 아기 > 어른 (모든 종)."""
    row = [a for a, *_ in ANIMS].index("sit_front")

    def eye_ratio(pid, st):
        # 128 칸 시트와 같은 필터(LANCZOS)로 줄여 잰다 — 얇은 눈의 픽셀 수는 필터에 민감하다
        sheet = Image.open(built / pid / f"sheet_{st}.png").convert("RGBA")
        cell = sheet.resize((4 * FRAME, sheet.height * 4 * FRAME // sheet.width), Image.LANCZOS).crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        px = cell.load()
        er, eg, eb = draw_pets.EYE
        dark = sum(1 for y in range(FRAME) for x in range(FRAME)
                   if px[x, y][3] > 200 and abs(px[x, y][0] - er) + abs(px[x, y][1] - eg) + abs(px[x, y][2] - eb) < 30)
        a = cell.getchannel("A").point(lambda v: 255 if v > 128 else 0)
        head_w = max(sum(1 for x in range(FRAME) if a.getpixel((x, y))) for y in range(FRAME // 2))
        return dark / head_w ** 2

    for pid in SPECIES_IDS:
        assert eye_ratio(pid, "baby") > 1.5 * eye_ratio(pid, "adult"), pid


def test_back_view_paws_are_drawn_behind_the_head(monkeypatch):
    """뒷모습 climb·slide: 앞발은 벽 쪽 — 머리·몸보다 먼저(뒤에) 그려서 발끝만 머리 옆·위로 빼꼼 보인다."""
    from sprite_spec import STAGES
    orders = record_draw_order(monkeypatch, [(draw_pets, "back_arm", "arm"), (draw_pets, "back_head", "head")])
    for pid in ("cat",):
        sp = draw_pets.SPECIES[pid]
        for st in (STAGES[0], STAGES[-1]):
            with draw_pets.stage(st, sp.stage_tweak.get(st)):
                draw_pets.frames(draw_pets.staged_species(sp), "climb", 1)
                draw_pets.frames(draw_pets.staged_species(sp), "slide", 1)
    assert len(orders) == 4
    for order in orders:
        assert order.count("arm") == 2 and order.index("head") > max(i for i, t in enumerate(order) if t == "arm"), order


def test_back_view_feet_are_drawn_behind_the_body(monkeypatch):
    """뒷모습은 벽을 보고 있다: 다리·발은 몸보다 먼저(뒤에) 그려서 엉덩이를 가리지 않는다. 꼬리는 몸 위."""
    from sprite_spec import STAGES
    orders = record_draw_order(monkeypatch, [(draw_pets, "back_leg", "leg"), (draw_pets, "back_body", "body"), (draw_pets, "back_tail", "tail")])
    for pid in ("cat",):
        sp = draw_pets.SPECIES[pid]
        for st in STAGES:
            with draw_pets.stage(st, sp.stage_tweak.get(st)):
                draw_pets.frames(draw_pets.staged_species(sp), "climb", 4)
                draw_pets.frames(draw_pets.staged_species(sp), "slide", 2)
    assert len(orders) == len(STAGES) * 6
    for order in orders:
        assert order.count("leg") == 2 and max(i for i, t in enumerate(order) if t == "leg") < order.index("body") < order.index("tail"), order


def _near(px, c, tol=40):
    return abs(px[0] - c[0]) + abs(px[1] - c[1]) + abs(px[2] - c[2]) < tol and px[3] > 128


def test_back_view_shows_no_soles_on_the_rump():
    """시각 의도: 엉덩이 위에 발바닥 젤리가 없다 (아래쪽 절반에 분홍 픽셀이 거의 없다)."""
    from sprite_spec import STAGES
    for pid in ("cat",):
        sp = draw_pets.SPECIES[pid]
        for st in STAGES:
            with draw_pets.stage(st, sp.stage_tweak.get(st)):
                got = draw_pets.frames(draw_pets.staged_species(sp), "climb", 4) + draw_pets.frames(draw_pets.staged_species(sp), "slide", 2)
            for n, im in enumerate(got):
                pink = sum(_near(im.getpixel((x, y)), draw_pets.PINK) for y in range(80, FRAME) for x in range(FRAME))
                assert pink <= 8, (pid, st, n, pink)  # 무늬 경계의 안티에일리어싱 몇 점은 허용


def test_climb_feet_only_peek_and_take_turns(monkeypatch):
    from sprite_spec import STAGES
    for pid in ("cat",):
        assert_feet_only_peek(monkeypatch, draw_pets.SPECIES[pid], draw_pets, "back_leg", lambda s, *a, **k: draw_pets.Part([], s.base), STAGES)


def _eat_cell(tmp_path, pid, st, col):
    names = [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]
    row = names.index("eat")
    return sheet_128(tmp_path / pid / f"sheet_{st}.png").convert("RGBA").crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def test_eat_mouth_meets_the_real_bowl(built):
    """그릇은 앱의 소품 창 (프레임 x 36–92, 밥 더미 위 y ≈ 82): 먹는 입(벌린 입 속 색)이 그 바로 위 가운데에 온다 — 모든 단계."""
    from sprite_spec import STAGES
    for pid in SPECIES_IDS:
        for st in STAGES:
            px = _eat_cell(built, pid, st, 0).load()
            mr, mg, mb = draw_pets.MOUTH_IN
            mouth = [(x, y) for y in range(FRAME) for x in range(FRAME)
                     if px[x, y][3] > 200 and abs(px[x, y][0] - mr) + abs(px[x, y][1] - mg) + abs(px[x, y][2] - mb) < 30]
            assert mouth, (pid, st)
            ys, xs = [y for _, y in mouth], [x for x, _ in mouth]
            assert 72 <= min(ys) and max(ys) <= 90, (pid, st, min(ys), max(ys))
            assert 54 <= min(xs) and max(xs) <= 74, (pid, st, min(xs), max(xs))


def test_eat_draws_no_bowl(built):
    """그릇은 앱이 따로 띄운다 — 먹는 프레임에 초록 줄기·잎이나 그릇 테두리 색이 없다.
    개구리는 몸이 초록이라 여기서 빼고 test_draw_frog.py(eat에 밥그릇 없음)가 따로 본다."""
    assert "frog" in draw_pets.SPECIES
    for pid in (p for p in SPECIES_IDS if p != "frog"):
        for st in CHECK_STAGES:
            for col in range(4):
                px = _eat_cell(built, pid, st, col).load()
                green = sum(1 for y in range(FRAME) for x in range(FRAME)
                            if px[x, y][3] > 200 and px[x, y][1] > px[x, y][0] + 30 and px[x, y][1] > px[x, y][2] + 30)
                assert green == 0, (pid, st, col, green)
            assert_eat_has_no_bowl(built / pid / f"sheet_{st}.png", draw_pets.SPECIES[pid], most=25)


def test_every_species_looks_up():
    """올려다보기(lookup): 모든 동물 (새 동물도 specials에 "lookup"과 자기 lookup 포즈를 더한다), 옆모습 3장, 눈을 위로 — 앉기(idle)·쳐다보기(look)와 다른 그림.
    마지막 장에만 ✧ (프레임 위쪽 오른편에 노란 별)."""
    from sprite_spec import manifest
    names = [o[0] for o in OPTIONAL_ANIMS]
    assert names[:7] == ["arch", "huff", "climb", "slide", "bigjump", "neckstretch", "lookup"]  # = Rust manifest::OPTIONAL_ANIMS
    assert OPTIONAL_ANIMS[6][1:] == (3, 3, True, False)
    for pid, sp in draw_pets.SPECIES.items():
        assert "lookup" in sp.specials, pid
        rows = draw_pets.rows_for(sp)
        assert [a for a, *_ in rows if a in names[:7]][-1] == "lookup", pid  # 지금까지의 선택 동작 중 맨 뒤 (뒤에 더한 동작은 그 뒤)
        fr = draw_pets.render_species(sp, "teenager")
        assert len(fr["lookup"]) == 3, pid
        for im in fr["lookup"]:
            assert im.tobytes() != fr["idle"][0].tobytes() and im.tobytes() != fr["look"][0].tobytes(), pid
        m = manifest(sp.name, sp.walk_speed, {a: len(fr[a]) for a, *_ in rows})
        assert m["animations"]["lookup"] == {"row": [a for a, *_ in rows].index("lookup"), "frames": 3, "fps": 3}, pid
        assert fr["lookup"][2].tobytes() != fr["lookup"][1].tobytes(), pid  # ✧


def test_up_eye_sits_higher_than_dot_eye():
    for st in ("baby", "teenager", "adult"):
        with draw_pets.stage(st):
            tops = []
            for kind in ("dot", "up"):
                fns = [f for fs, *_ in draw_pets.stage_eye(kind, 64, 64) for f in fs]
                tops.append(draw_pets.shapes(fns).getbbox()[1])
        assert tops[1] < tops[0], (st, tops)


# ---- 2배 칸 (sprite_spec.frame_px): 저장소의 진짜 그림을 본다 (그리지 않아 빠르다)
def _shipped():
    root = Path(__file__).resolve().parent.parent / "ui" / "pets"
    return [(d.name, json.loads((d / "pet.json").read_text(encoding="utf-8")), d) for d in sorted(root.iterdir())]


def test_hires_stages_follow_the_on_screen_size():
    """청소년 때 이미 128보다 크게 보이는 동물(stage_size >= 1)만 청소년도 2배, 어른은 모두 2배."""
    shipped = _shipped()
    assert len(shipped) == 18  # v0.5.5: 곰·꿀벌·강아지 −3, 펭귄 +1, 레서판다 +1, 다람쥐 +1
    assert {pid for pid, m, _ in shipped if m["stage_size"]["teenager"] >= 1} == HIRES_TEENAGER
    for pid, m, _ in shipped:
        assert m["frame_size"] == FRAME
        assert m["frame_sizes"] == {st: frame_px(pid, st) for st in STAGE_NAMES if frame_px(pid, st) != FRAME}
        assert m["frame_sizes"]["adult"] == 2 * FRAME and ("teenager" in m["frame_sizes"]) == (pid in HIRES_TEENAGER)


def test_only_rolling_pets_have_a_roll_spin():
    for pid, m, _ in _shipped():
        assert ("spin" in m["animations"]["roll"]) == (pid not in NO_SPIN), pid


def test_every_layer_has_the_cell_size_the_manifest_says():
    for pid, m, d in _shipped():
        rows = 1 + max(a["row"] for a in m["animations"].values())
        for st in STAGE_NAMES:
            px = m.get("frame_sizes", {}).get(st, FRAME)
            with Image.open(d / f"sheet_{st}.png") as im:
                assert im.size == (4 * px, rows * px), (pid, st, im.size)
            for key in ("head", "neck"):
                for cells in m.get(key, {}).get(st, {}).values():
                    assert all(-px * 0.2 < c[0] < px * 1.2 and -px * 0.2 < c[1] < px * 1.2 and c[2] < px for c in cells), (pid, st, key)
            assert ("neck_stretch" in m) == (pid == "giraffe"), pid
            if pid == "giraffe":
                meta = m["neck_stretch"][st]
                with Image.open(d / f"neck_{st}.png") as strip, Image.open(d / f"neck_tile_{st}.png") as tile:
                    assert strip.size == (4 * px, px) and tile.size == (px, meta["tile"]), (pid, st)
                assert meta["tile"] == 16 * px // FRAME and 16 * px // FRAME <= meta["ext"] <= 56 * px // FRAME and meta["cut"] < px, (pid, st, meta)
        assert not (d / "sheet.png").exists(), pid  # 앱은 단계 시트만 읽는다 (sheet.png는 sheet_teenager.png와 같은 바이트였다)


def test_preview_poses_draws_every_band(tmp_path):
    """개발용 표: 줄 수만큼 높이, zoom 수만큼 너비 — 밝은·어두운 띠 둘 다 그려진다."""
    blank = Image.new("RGBA", (FRAME, FRAME), (255, 0, 0, 255))
    by_stage = {st: {"walk": [blank], "sit_front": [blank]} for st in STAGE_NAMES}
    rows = [(st, by_stage, st) for st in STAGE_NAMES]
    out = tmp_path / "p.png"
    draw_pets.preview_poses([((255, 255, 255), rows), ((40, 44, 52), rows)], [("walk", 0)], out, zoom=("sit_front", "walk"))
    im = Image.open(out)
    row_h = 2 * FRAME + 8
    assert im.height == 24 + 10 * row_h
    assert im.getpixel((2, 24 + 9 * row_h))[:3] == (40, 44, 52)
    assert im.getpixel((im.width - 10, 24 + row_h // 2))[:3] == (255, 0, 0)



# ---- 🐱 페르시안 고양이: 한눈에 고양이와 다르다 — 납작 네모 얼굴 · 낮고 작은 귀 · 졸린 반눈 · 복슬 갈기·꼬리 · 느릿느릿
def _one(pid, anim, st="teenager"):
    """한 동물·한 단계의 첫 장만 (128 칸) — 시트 전체를 그리지 않아 빠르다."""
    sp = draw_pets.SPECIES[pid]
    with draw_pets.stage(st, sp.stage_tweak.get(st)), draw_pets.output(1):
        return draw_pets.frames(draw_pets.staged_species(sp), anim, 1)[0]


def _widths(im):
    a = im.getchannel("A").point(lambda v: 255 if v > 128 else 0)
    return [sum(1 for x in range(FRAME) if a.getpixel((x, y))) for y in range(FRAME)]


def _area(pts):
    return abs(sum(x0 * y1 - x1 * y0 for (x0, y0), (x1, y1) in zip(pts, pts[1:] + pts[:1]))) / 2


def test_persian_is_a_lazy_aloof_cat():
    sp = draw_pets.SPECIES["persian"]
    assert sp.name == "페르시안 고양이" and sp.walk_speed < draw_pets.CAT.walk_speed * 0.7
    assert draw_pets.PERSONALITY["persian"] == "aloof" and "huff" in sp.specials  # 새침한 고양이는 클릭에 흥! (Rust: Aloof → Huff)
    assert draw_pets.FAVORITE_FOOD["persian"]["emoji"] != draw_pets.FAVORITE_FOOD["cat"]["emoji"]
    assert sp.mouth == "flat"  # 납작 얼굴: 눈 사이 높은 작은 코 + 무표정 일자 입


def test_persian_ears_are_much_smaller_than_the_cats():
    ear = lambda sp: max(_area(list(d[1:])) for d in sp.front_ears if d[0] == "T")
    assert ear(draw_pets.SPECIES["persian"]) < 0.45 * ear(draw_pets.CAT)


@pytest.mark.parametrize("st", ["baby", "teenager", "adult"])
def test_persian_face_is_wide_and_square_with_a_ruff(st):
    """앉은 앞모습: 머리(위쪽) 실루엣이 고양이보다 넓고 (볼 털·갈기), 꼭대기 바로 아래가 이미 넓다 (낮은 작은 귀 = 네모난 윗머리 —
    고양이는 뾰족 귀 두 개만 있는 줄). 아기는 정수리 삐죽 털이 꼭대기라 폭만 본다."""
    def head(pid):
        w = _widths(_one(pid, "sit_front", st))
        top = next(y for y, v in enumerate(w) if v)
        widest = max(w[top:top + 48])
        return widest, w[top + 6] / widest
    (pw, pt), (cw, ct) = head("persian"), head("cat")
    assert pw > cw * 1.06, (st, pw, cw)
    if st != "baby":
        assert pt > ct + 0.2, (st, pt, ct)


@pytest.mark.parametrize("anim", ["idle", "sit_front"])
def test_persian_resting_eyes_are_half_closed(anim):
    """쉴 때 눈: 눈 색 덩어리의 높이가 고양이보다 한참 낮다 (반쯤 감은 졸린 눈)."""
    def eye_h(pid):
        px = _one(pid, anim).load()
        er, eg, eb = draw_pets.EYE
        ys = [y for y in range(FRAME // 2 + 16) for x in range(FRAME)
              if px[x, y][3] > 200 and abs(px[x, y][0] - er) + abs(px[x, y][1] - eg) + abs(px[x, y][2] - eb) < 30
              and (x < 64 if anim == "sit_front" else True)]
        return max(ys) - min(ys) + 1
    assert eye_h("persian") < 0.7 * eye_h("cat"), (eye_h("persian"), eye_h("cat"))


def test_persian_tail_is_bushier_than_the_cats():
    w = lambda sp: max(d[2] for d in sp.side_tail if d[0] == "L")
    assert w(draw_pets.SPECIES["persian"]) > 1.4 * w(draw_pets.CAT)
