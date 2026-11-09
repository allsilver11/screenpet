"""크리스마스 시트 검사 (루돌프·산타): 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·루돌프 코가 빨갛게 빛남·
뿔이 단계마다 자람·산타는 썰매를 타고 roll에 spin이 없음·산타는 고양이처럼 벽을 탄다 (뒷모습)."""
import json
from dataclasses import replace

import numpy as np
import pytest
from PIL import Image

import draw_christmas
import draw_pets
from conftest import assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, FRAME, STAGES, sheet_128

GROUNDED = {"walk", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "level_up", "eat"}


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in (draw_christmas.RUDOLPH, draw_christmas.SANTA):
        draw_pets.write_species(sp, root / sp.id)
    return root


def names(pid):
    return [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]


def frame(out, pid, anim, col=0, st="teenager"):
    return cell(sheet_128(out / pid / f"sheet_{st}.png").convert("RGBA"), names(pid).index(anim), col)


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
        assert names(pid) == [a for a, *_ in ANIMS] + {"rudolph": [], "santa": ["climb", "slide"]}[pid] + ["lookup"]
        assert all(a in poses for a in names(pid))


@pytest.mark.parametrize("pid", ["rudolph", "santa"])
def test_sheets_have_every_frame_inside_the_cell(out, pid):
    # 루돌프의 구르기는 도는 공: 칸 안이고 회전 원 안이다 (test_rudolph_roll_ball_fits_rotation_circle)
    assert_frames_inside_cell(out / pid, draw_pets.SPECIES[pid], STAGES, grounded=GROUNDED, ground_y=116)


def test_rudolph_roll_ball_fits_rotation_circle(out):
    assert_roll_fits_circle(out / "rudolph", draw_pets.SPECIES["rudolph"], STAGES)


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
    assert set(ru["animations"]) == {a for a, *_ in ANIMS} | {"lookup"}
    assert set(sa["animations"]) == {a for a, *_ in ANIMS} | {"climb", "slide", "lookup"}
    assert sa["animations"]["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6} and sa["animations"]["slide"] == {"row": len(ANIMS) + 1, "frames": 2, "fps": 8}
    for m in (ru, sa):
        assert m["speech"]["hungry"] and m["speech"]["full"] and m["speech"]["level_up"] and m["stages"] == STAGES
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
        assert_eat_has_no_bowl(out / pid / f"sheet_{st}.png", draw_pets.SPECIES[pid], most=60)  # 그릇이면 수백 px — 루돌프 코 후광·산타 볼터치가 몇 px 섞인다


def near(col, tol=24):
    return lambda r, g, b, a: (a > 200) & (np.abs(r - col[0]) + np.abs(g - col[1]) + np.abs(b - col[2]) < tol)


def patches(im, col, tol):
    """그 색의 2×2 덩어리 수 — 빨강·흰색 경계의 안티에일리어싱 낱개 픽셀은 세지 않고, 그려진 코·볼·눈만 잡는다."""
    a = px(im)
    h = (a[..., 3] > 200) & (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol)
    return int((h[:-1, :-1] & h[1:, :-1] & h[:-1, 1:] & h[1:, 1:]).sum())


def back_frames(out, st):
    return [("climb", c, frame(out, "santa", "climb", c, st)) for c in range(4)] + [("slide", c, frame(out, "santa", "slide", c, st)) for c in range(2)]


def torso_box(st, frame_img):
    """뒷모습 몸통 칸 (모자 테 바로 아래 ~ 몸 아래끝, 가운데 몸 폭): 프레임 좌표. 그림의 _layout을 현재 단계 배율로 옮기고 불투명 아래끝에 맞춘다."""
    sp = draw_christmas.SANTA
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        s = sp.scale * draw_pets._ST["fit"]
        ss = draw_christmas._sa()
        cy_b, hy = draw_christmas._layout(ss, draw_christmas.SA_FLOOR - 1)
        to_y = lambda y: 126 + (y - 126) * s
        half = ss["brx"] * s * 0.8
        top, body_bottom = to_y(hy + ss["hr"] * 0.08), to_y(cy_b + ss["bry"] * 0.72)
    return round(64 - half), round(top), round(64 + half), round(body_bottom)


def test_santa_climbs_with_his_back_to_us(out):
    """뒷모습은 뒷모습으로 읽혀야 한다: 어느 단계에도 살색·얼굴 없음, 벨트는 어린이부터 등 쪽 띠로, 버클 없음, 자루는 어른만."""
    for st in STAGES:
        for anim, c, im in back_frames(out, st):
            assert count(im, near(draw_christmas.SUIT)) >= 400, (st, anim, c)
            assert count(im, near(draw_christmas.SKIN, 14)) == 0, (st, anim, c)  # 살색이 어디에도 없다
            assert patches(im, draw_christmas.CHEEK_RED, 14) == 0, (st, anim, c)  # 볼터치가 없다
            assert patches(im, (250, 166, 156), 16) == 0, (st, anim, c)  # 코가 없다
            assert patches(im.crop((0, 0, FRAME, 66)), draw_pets.EYE, 12) == 0, (st, anim, c)  # 눈이 없다
            gold = im.crop((0, 95, FRAME, FRAME)) if st == "adult" else im  # 어른의 선물 상자 리본은 위쪽, 버클 자리는 아래쪽
            assert count(gold, near(draw_christmas.BUCKLE, 30)) == 0, (st, anim, c)
            sack = count(im, near(draw_christmas.SACK, 16))
            assert (sack >= 200) == (st == "adult"), (st, anim, c, sack)
            belt = count(im, near(draw_christmas.BELT, 16))
            assert (belt >= 60) == (st in ("child", "teenager", "adult")), (st, anim, c, belt)


def test_santa_back_torso_is_red_not_a_big_white_oval(out):
    """모자 아래 몸통은 빨간 등판 — 수염·머리칼이 차지하는 흰 부분은 없다 (얼굴 없는 앞모습처럼 보이던 큰 흰 타원 금지)."""
    for st in STAGES:
        for anim, c, im in back_frames(out, st):
            box = im.crop(torso_box(st, im))
            white = count(box, near(draw_christmas.FUR, 16)) + count(box, near(draw_christmas.FUR_SHADE, 16))
            red = count(box, near(draw_christmas.SUIT))
            area = box.width * box.height
            assert white <= 0.03 * area, (st, anim, c, white, area)
            if st != "adult":  # 어른은 자루가 등을 덮는다
                assert red >= 0.6 * area, (st, anim, c, red, area)


def test_santa_back_hair_is_a_thin_line_and_beard_tips_barely_show(out):
    """모자 테 아래 머리 자리: 가운데 세로줄에서 흰색은 얇은 한 줄(몇 px)뿐, 그 아래는 곧장 빨강."""
    for st in STAGES:
        im = frame(out, "santa", "climb", 1, st)
        sp = draw_christmas.SANTA
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            s = sp.scale * draw_pets._ST["fit"]
            ss = draw_christmas._sa()
            _, hy = draw_christmas._layout(ss, draw_christmas.SA_FLOOR - 1)
            y0 = round(126 + (hy - ss["hr"] * 0.02 - 126) * s)
            hr = ss["hr"] * s
        below = im.crop((58, y0, 70, y0 + round(hr * 0.5)))
        assert count(below, near(draw_christmas.FUR, 16)) <= 2 * below.width, (st, count(below, near(draw_christmas.FUR, 16)))
        if st != "adult":  # 어른은 자루 위 선물 상자가 이 자리까지 올라온다
            assert count(below, near(draw_christmas.SUIT)) >= 0.5 * below.width * below.height, st


def test_santa_mitts_and_boots_take_turns_on_the_climb(out):
    """장갑은 머리 옆·위로 번갈아, 장화는 번갈아 빼꼼 — 왼쪽과 오른쪽이 프레임마다 바뀐다."""
    for st in STAGES:
        frames = [frame(out, "santa", "climb", c, st) for c in range(4)]
        assert len({f.tobytes() for f in frames}) >= 3, st
        boot = near(draw_christmas.BOOT, 20)
        low_left = [count(f.crop((0, 90, 64, FRAME)), boot) for f in frames]
        low_right = [count(f.crop((64, 90, FRAME, FRAME)), boot) for f in frames]
        assert low_left[2] > low_left[0] and low_right[0] > low_right[2], (st, low_left, low_right)


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


# ---------------------------------------------------------------- 루돌프 등 무늬 (아기 사슴 흰 점) — 크면서 없어지지 않는다
SIDE_BACK_ANIMS = ("walk", "idle", "run", "sleep")


def _spotless(sp):
    """등 흰 점만 뺀 루돌프 (다른 무늬는 그대로) — 두 그림의 차이가 곧 등 무늬."""
    spots = [m for m in sp.side_torso_marks if len(m[0]) >= 6]
    assert len(spots) == 1, "등 흰 점 무늬 한 묶음"
    return replace(sp, side_torso_marks=[m for m in sp.side_torso_marks if m is not spots[0]])


def _back_spot_pixels(a, b):
    """두 프레임에서 다른 불투명 픽셀 중 점 쪽(b보다 밝은 크림색)이 몸 위쪽(등)에 있는 수."""
    pa, pb = px(a), px(b)
    diff = (pa[..., 3] > 200) & (np.abs(pa[..., :3] - pb[..., :3]).sum(axis=2) > 40) & (pa[..., :3].sum(axis=2) > pb[..., :3].sum(axis=2))
    ys = np.nonzero(pa[..., 3] > 200)[0]
    if not len(ys):
        return 0
    top, bottom = ys.min(), ys.max()
    upper = np.zeros_like(diff)
    upper[: top + int((bottom - top) * 0.75)] = True  # 배(크림)는 아래 — 점은 등·옆구리 위쪽
    return int((diff & upper).sum())


def _round_side_frames(sp, st, anim):
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        return draw_pets.frames(draw_pets.staged_species(sp), anim, 1)[0]


@pytest.mark.parametrize("st", STAGES)
def test_rudolph_keeps_back_spots_at_every_stage_round(st):
    sp = draw_christmas.RUDOLPH
    plain = _spotless(sp)
    counts = {anim: _back_spot_pixels(_round_side_frames(sp, st, anim), _round_side_frames(plain, st, anim)) for anim in SIDE_BACK_ANIMS}
    assert min(counts.values()) >= 10 and sum(counts.values()) >= 100, (st, counts)  # 엎드려 자는 옆모습은 등이 덜 보인다


@pytest.mark.parametrize("st", STAGES)
def test_rudolph_keeps_back_spots_at_every_stage_pixel(st):
    import draw_pixel
    sp = draw_christmas.RUDOLPH
    plain = _spotless(sp)
    anims = set(SIDE_BACK_ANIMS) - set(draw_pixel.NO_ANIM)
    with_spots, without = draw_pixel.render_pixel(sp, st, anims), draw_pixel.render_pixel(plain, st, anims)
    counts = {anim: _back_spot_pixels(with_spots[anim][0], without[anim][0]) for anim in anims}
    assert sum(counts.values()) >= 3 and counts["walk"] >= 1, (st, counts)  # 도트 하나 = 1px (시트 칸 크기 그대로) · 아기 24칸은 점이 몇 개뿐
