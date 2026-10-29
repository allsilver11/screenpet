from pathlib import Path

from PIL import Image

import draw_pets
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, ROLL_CENTER, ROLL_RADIUS

STANDING = {"walk", "run", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed"}


def test_sleep_is_lying_flat(tmp_path):
    draw_pets.build(tmp_path)
    row = [a for a, *_ in ANIMS].index("sleep")
    for pid in ("cat", "dog", "rabbit", "beaver"):
        cell = Image.open(tmp_path / pid / "sheet.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        # z 글자는 제외하고 몸만: 불투명한 큰 덩어리의 위쪽이 낮아야 한다
        alpha = cell.getchannel("A").point(lambda v: 255 if v > 200 else 0)
        body_top = min(y for y in range(FRAME) if sum(alpha.getpixel((x, y)) > 0 for x in range(FRAME)) >= 20)
        assert body_top >= 70, f"{pid} sleep body top {body_top} — 엎드린 자세가 아님"


def test_every_species_every_animation(tmp_path):
    draw_pets.build(tmp_path)
    for pid in ("cat", "dog", "rabbit", "beaver"):
        sheet = Image.open(tmp_path / pid / "sheet.png")
        # 필수 18행 + 그 종의 선택 동작 행 (고양이 arch, 강아지 wag)
        rows = list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in draw_pets.SPECIES[pid].specials]
        assert sheet.height == FRAME * len(rows)
        for row, (anim, n, *_) in enumerate(rows):
            for col in range(n):
                cell = sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))
                bbox = cell.getchannel("A").getbbox()
                assert bbox is not None, f"{pid} {anim} {col} empty"
                assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, f"{pid} {anim} {col} clipped {bbox}"
                if anim in STANDING:
                    assert bbox[3] >= 118, f"{pid} {anim} {col} floats (bottom {bbox[3]})"


def test_only_cat_has_arch_and_only_dog_has_wag(tmp_path):
    import json
    draw_pets.build(tmp_path)
    for pid in ("cat", "dog", "rabbit", "beaver"):
        m = json.loads((tmp_path / pid / "pet.json").read_text(encoding="utf-8"))
        assert ("arch" in m["animations"]) == (pid == "cat"), pid
        assert ("wag" in m["animations"]) == (pid == "dog"), pid
    m = json.loads((tmp_path / "dog" / "pet.json").read_text(encoding="utf-8"))
    assert m["animations"]["wag"]["row"] == len(ANIMS)
    assert m["animations"]["wag"]["front"] is True
    m = json.loads((tmp_path / "cat" / "pet.json").read_text(encoding="utf-8"))
    assert m["animations"]["arch"]["row"] == len(ANIMS)
    assert m["speech"]["hiss"] == ["hiss!"]
    sheet = Image.open(tmp_path / "cat" / "sheet.png")
    assert sheet.height == FRAME * (len(ANIMS) + 1)


def test_roll_ball_fits_rotation_circle(tmp_path):
    draw_pets.build(tmp_path)
    cx, cy = ROLL_CENTER
    row = [a for a, *_ in ANIMS].index("roll")
    for pid in ("cat", "dog", "rabbit", "beaver"):
        cell = Image.open(tmp_path / pid / "sheet.png").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))
        alpha = cell.getchannel("A")
        for y in range(FRAME):
            for x in range(FRAME):
                if alpha.getpixel((x, y)) > 40:
                    assert (x - cx) ** 2 + (y - cy) ** 2 <= (ROLL_RADIUS + 1) ** 2, f"{pid} roll pixel ({x},{y}) outside circle"
