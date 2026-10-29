from pathlib import Path

from PIL import Image

import draw_pets
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, ROLL_CENTER, ROLL_RADIUS

STANDING = {"walk", "run", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "eat", "level_up", "huff"}
SPECIES_IDS = ("cat", "dog", "rabbit", "bear")
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
            # 필수 21행 + 그 종의 선택 동작 행 (고양이 arch·huff, 강아지 wag)
            rows = list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in draw_pets.SPECIES[pid].specials]
            assert sheet.height == FRAME * len(rows)
            for row, (anim, n, *_) in enumerate(rows):
                for col in range(n):
                    cell = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))
                    bbox = cell.getchannel("A").getbbox()
                    assert bbox is not None, f"{pid} {st} {anim} {col} empty"
                    assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, \
                        f"{pid} {st} {anim} {col} clipped {bbox}"
                    if anim in STANDING:
                        assert bbox[3] >= 118, f"{pid} {st} {anim} {col} floats (bottom {bbox[3]})"


def test_only_cat_has_arch_and_huff_and_only_dog_has_wag(tmp_path):
    import json
    draw_pets.build(tmp_path)
    for pid in SPECIES_IDS:
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert ("arch" in m["animations"]) == (pid == "cat"), pid
        assert ("huff" in m["animations"]) == (pid == "cat"), pid
        assert ("wag" in m["animations"]) == (pid == "dog"), pid
        assert ("personality" in m) == (pid == "cat"), pid
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
    assert sheet.height == FRAME * (len(ANIMS) + 2)


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
