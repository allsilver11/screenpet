"""크리스마스 시트 검사 (루돌프·산타): 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·루돌프 코가 빨갛게 빛남·
뿔이 단계마다 자람·산타는 썰매를 타고 roll에 spin이 없음."""
import json

import numpy as np
import pytest
from PIL import Image

import draw_christmas
import draw_pets
from sprite_spec import ANIMS, FRAME, STAGES

GROUNDED = {"walk", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "level_up", "eat"}
BOWL_COLORS = [(246, 160, 170), (250, 205, 210)]


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in (draw_christmas.RUDOLPH, draw_christmas.SANTA):
        draw_pets.write_species(sp, root / sp.id)
    return root


def names(pid):
    return [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]


def cell(sheet, row, col):
    return sheet.crop((col * FRAME, row * FRAME, (col + 1) * FRAME, (row + 1) * FRAME))


def frame(out, pid, anim, col=0, st="teenager"):
    return cell(Image.open(out / pid / f"sheet_{st}.png").convert("RGBA"), names(pid).index(anim), col)


def px(im):
    return np.asarray(im.convert("RGBA")).astype(int)


def count(im, fn):
    a = px(im)
    return int(fn(a[..., 0], a[..., 1], a[..., 2], a[..., 3]).sum())


def red_nose(im):
    """밝은 빨강 코 (불투명)."""
    return count(im, lambda r, g, b, a: (a > 200) & (r > 200) & (g < 80) & (b < 90))


def halo(im):
    """코 둘레 반투명 빨간 후광 (몸 밖)."""
    return count(im, lambda r, g, b, a: (a > 8) & (a < 200) & (r > 200) & (g < 140))


def test_both_are_registered_with_every_required_anim():
    for pid, poses in (("rudolph", draw_christmas.RUDOLPH_POSES), ("santa", draw_christmas.SANTA_POSES)):
        sp = draw_pets.SPECIES[pid]
        assert sp.poses is poses
        assert names(pid) == [a for a, *_ in ANIMS]
        assert all(a in poses for a in names(pid))


@pytest.mark.parametrize("pid", ["rudolph", "santa"])
def test_sheets_have_every_frame_inside_the_cell(out, pid):
    rows = draw_pets.rows_for(draw_pets.SPECIES[pid])
    for st in STAGES:
        sheet = Image.open(out / pid / f"sheet_{st}.png")
        assert sheet.mode == "RGBA" and sheet.size == (4 * FRAME, len(rows) * FRAME), st
        for r, (anim, n, *_) in enumerate(rows):
            for c in range(n):
                bbox = cell(sheet, r, c).getchannel("A").getbbox()
                assert bbox is not None, (st, anim, c)
                assert bbox[0] >= 1 and bbox[1] >= 1 and bbox[2] <= FRAME - 1 and bbox[3] <= FRAME - 1, (st, anim, c, bbox)
                if anim in GROUNDED:
                    assert bbox[3] >= 116, (st, anim, c, bbox)
            for c in range(n, 4):
                assert cell(sheet, r, c).getchannel("A").getbbox() is None, (st, anim, c)
    assert (out / pid / "sheet.png").read_bytes() == (out / pid / "sheet_teenager.png").read_bytes()


def test_manifests(out):
    ru = json.loads((out / "rudolph" / "pet.json").read_text(encoding="utf-8"))
    assert ru["name"] == "루돌프" and ru["personality"] == "friendly" and "locomotion" not in ru and ru["hop_share"] == 0.3
    assert ru["favorite_food"] == {"emoji": "🍎", "name": "사과"} and ru["house"] == "xmas"
    assert ru["speech"]["chatter"] == ["jingle~", "♪"] and ru["speech"]["tickle"] == ["hehe!"] and ru["speech"]["grumpy"] == ["hmph!"]
    assert ru["animations"]["roll"]["spin"] > 0  # 루돌프는 그대로 데굴데굴
    sa = json.loads((out / "santa" / "pet.json").read_text(encoding="utf-8"))
    assert sa["name"] == "산타" and sa["personality"] == "friendly" and "locomotion" not in sa and "hop_share" not in sa
    assert sa["favorite_food"] == {"emoji": "🥛", "name": "우유"} and sa["house"] == "xmas"
    assert sa["speech"]["chatter"] == ["ho ho ho!", "merry~"] and sa["speech"]["tickle"] == ["ho ho!"]
    for m in (ru, sa):
        assert m["speech"]["hungry"] and m["speech"]["full"] and m["speech"]["level_up"]
        assert set(m["animations"]) == {a for a, *_ in ANIMS} and m["stages"] == STAGES
        assert m["animations"]["eat"] == {"row": 19, "frames": 4, "fps": 6, "front": True}


def test_santa_rides_a_sled_instead_of_rolling(out):
    """산타 roll = 썰매 (옆모습, 바닥에 붙어 미끄러짐) — pet.json에 spin이 없어 앱이 돌리지 않는다."""
    sa = json.loads((out / "santa" / "pet.json").read_text(encoding="utf-8"))
    assert "spin" not in sa["animations"]["roll"]
    sled_red = lambda r, g, b, a: (a > 200) & (np.abs(r - draw_christmas.SLED[0]) + np.abs(g - draw_christmas.SLED[1]) + np.abs(b - draw_christmas.SLED[2]) < 30)
    runner = lambda r, g, b, a: (a > 200) & (np.abs(r - draw_christmas.RUNNER[0]) + np.abs(g - draw_christmas.RUNNER[1]) + np.abs(b - draw_christmas.RUNNER[2]) < 30)
    for st in STAGES:
        frames = [frame(out, "santa", "roll", c, st) for c in range(2)]
        for im in frames:
            bbox = im.getchannel("A").getbbox()
            assert bbox[3] >= 118, (st, bbox)  # 바닥에 붙어 있다
            assert bbox[2] - bbox[0] >= 80, (st, bbox)  # 길쭉한 썰매
            low = im.crop((0, 100, FRAME, FRAME))
            assert count(low, sled_red) >= 150 and count(low, runner) >= 20, st
        assert frames[0].tobytes() != frames[1].tobytes()  # 방울·목도리·눈가루가 움직인다


@pytest.mark.parametrize("pid", ["rudolph", "santa"])
def test_eat_has_no_bowl(out, pid):
    for st in ("baby", "teenager", "adult"):
        for c in range(4):
            im = frame(out, pid, "eat", c, st)
            bowlish = count(im, lambda r, g, b, a: (a > 200) & np.logical_or.reduce(
                [np.abs(r - col[0]) + np.abs(g - col[1]) + np.abs(b - col[2]) <= 12 for col in BOWL_COLORS]))
            assert bowlish < 60, (st, c, bowlish)  # 그릇이면 수백 px — 루돌프 코 후광·산타 볼터치가 몇 px 섞인다


def test_santa_mouth_sits_over_the_food(out):
    for st in ("baby", "teenager", "adult"):
        im = frame(out, "santa", "eat", 0, st).crop((50, 70, 79, 97))
        mouth = count(im, lambda r, g, b, a: (a > 200) & (np.abs(r - draw_pets.MOUTH_IN[0]) + np.abs(g - draw_pets.MOUTH_IN[1])
                                                          + np.abs(b - draw_pets.MOUTH_IN[2]) < 40))
        assert mouth >= 6, (st, mouth)


def test_rudolph_nose_glows_red_and_twinkles(out):
    for st in STAGES:
        for anim, n in (("walk", 4), ("sit_front", 4), ("idle", 2), ("eat", 4)):
            for c in range(n):
                im = frame(out, "rudolph", anim, c, st)
                assert red_nose(im) >= 60, (st, anim, c)  # 코가 루돌프의 포인트 — 크게
        side = frame(out, "rudolph", "walk", 0, st)
        assert halo(side) >= 20, st  # 옆모습 코끝 밖으로 빨간 빛
    # 프레임마다 반짝반짝: 같은 그림이라도 프레임 번호마다 코 빛 세기가 다르다
    base = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    base.paste(Image.new("RGBA", (12, 10), draw_christmas.NOSE_KEY + (255,)), (58, 60))
    glows = [draw_christmas._nose_glow(base, i).tobytes() for i in range(4)]
    assert len(set(glows)) == 4
    assert halo(frame(out, "rudolph", "sleep", 0)) < halo(frame(out, "rudolph", "idle", 0))  # 잘 때는 은은하게


def test_rudolph_antlers_grow_with_age(out):
    antler = lambda r, g, b, a: (a > 200) & (np.abs(r - draw_christmas.ANTLER[0]) + np.abs(g - draw_christmas.ANTLER[1])
                                             + np.abs(b - draw_christmas.ANTLER[2]) < 60)
    sizes = [count(frame(out, "rudolph", "sit_front", 0, st), antler) for st in STAGES]
    assert sizes[0] >= 20 and all(b > a for a, b in zip(sizes, sizes[1:])), sizes


def test_rudolph_apple_bubble_is_not_mistaken_for_the_nose(out):
    """배고픔 생각 풍선 속 사과는 코 빛내기에 걸리지 않는다 (코만 동글게 — 큰 빨간 덩어리 없음)."""
    for st in STAGES:
        for c in range(3):
            im = frame(out, "rudolph", "hungry", c, st)
            assert red_nose(im) < 400, (st, c, red_nose(im))  # 큰 코 하나만큼
            assert red_nose(im.crop((0, 0, FRAME, 40))) < 30, (st, c)  # 머리 위 풍선 쪽엔 빨간 코가 없다
