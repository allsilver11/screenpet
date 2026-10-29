import json

from PIL import Image

from pack_sheet import pack
from sprite_spec import ANIMS, FRAME, roll_spin


def make_frames(d, size=(300, 200)):
    for anim, n, *_ in ANIMS:
        for i in range(n):
            img = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
            img.paste((200, 100, 50, 255), (100, 200, 100 + size[0], 200 + size[1]))
            img.save(d / f"{anim}_{i}.png")


def test_pack_builds_sheet_and_manifest(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    pack(frames, out, "테스트", 55)

    sheet = Image.open(out / "sheet.png")
    max_frames = max(n for _, n, *_ in ANIMS)
    assert sheet.size == (FRAME * max_frames, FRAME * len(ANIMS))

    m = json.loads((out / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "테스트"
    assert m["walk_speed"] == 55
    assert m["animations"]["walk"] == {"row": 0, "frames": 4, "fps": 8}
    assert m["animations"]["hop"]["loop"] is False
    assert m["animations"]["wave"]["front"] is True
    assert m["animations"]["roll"]["spin"] == roll_spin(55)


def test_frames_are_bottom_aligned_and_fit(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    pack(frames, out, "x", 60)
    first = Image.open(out / "sheet.png").crop((0, 0, FRAME, FRAME))
    bbox = first.getchannel("A").getbbox()
    assert bbox[3] == FRAME  # 발이 프레임 바닥에 닿음
    assert bbox[2] - bbox[0] <= FRAME


def test_missing_frame_raises(tmp_path):
    frames = tmp_path / "frames"
    frames.mkdir()
    try:
        pack(frames, tmp_path / "out", "x", 60)
    except FileNotFoundError as e:
        assert "walk_0.png" in str(e)
    else:
        raise AssertionError("expected FileNotFoundError")


def test_blank_frame_raises_value_error_naming_file(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    Image.new("RGBA", (512, 512), (0, 0, 0, 0)).save(frames / "walk_1.png")
    try:
        pack(frames, out, "x", 60)
    except ValueError as e:
        assert "walk_1.png" in str(e)
    else:
        raise AssertionError("expected ValueError")


def test_motion_within_animation_is_preserved(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    # walk 프레임 두 장: 같은 캔버스에서 동물이 x로 50px 이동
    for i, x0 in enumerate((100, 150)):
        img = Image.new("RGBA", (512, 512), (0, 0, 0, 0))
        img.paste((200, 100, 50, 255), (x0, 200, x0 + 300, 400))
        img.save(frames / f"walk_{i}.png")
    pack(frames, out, "x", 60)

    sheet = Image.open(out / "sheet.png")
    lefts = []
    for col in range(2):
        cell = sheet.crop((col * FRAME, 0, col * FRAME + FRAME, FRAME))
        lefts.append(cell.getchannel("A").getbbox()[0])
    # 공통 크기: 합집합 박스 350px → 배율 FRAME/350, 이동량 50px × 배율만큼 차이
    expected = round(50 * FRAME / 350)
    assert abs((lefts[1] - lefts[0]) - expected) <= 1, lefts


def test_numbering_gap_raises_value_error(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    (frames / "walk_1.png").unlink()  # walk_0, walk_2, walk_3 — 1번 빠짐
    try:
        pack(frames, out, "x", 60)
    except ValueError as e:
        assert "walk_1.png" in str(e)
    else:
        raise AssertionError("expected ValueError")


def test_mixed_canvas_sizes_in_animation_raise(tmp_path):
    frames, out = tmp_path / "frames", tmp_path / "out"
    frames.mkdir()
    make_frames(frames)
    Image.new("RGBA", (256, 256), (200, 100, 50, 255)).save(frames / "walk_1.png")
    try:
        pack(frames, out, "x", 60)
    except ValueError as e:
        assert "walk" in str(e)
    else:
        raise AssertionError("expected ValueError")
