import json

from PIL import Image

from pack_sheet import pack
from sprite_spec import ANIMS, FRAME


def make_frames(d, size=(300, 200)):
    for anim, n, _, _ in ANIMS:
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
    max_frames = max(n for _, n, _, _ in ANIMS)
    assert sheet.size == (FRAME * max_frames, FRAME * len(ANIMS))

    m = json.loads((out / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "테스트"
    assert m["walk_speed"] == 55
    assert m["animations"]["walk"] == {"row": 0, "frames": 4, "fps": 8}
    assert m["animations"]["hop"]["loop"] is False


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
