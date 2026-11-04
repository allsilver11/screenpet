"""할로윈 시트 검사 (호박·유령): 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·호박 눈이 노랗게 빛남·유령 몸이 반투명."""
import json
import math

import pytest

import draw_pets  # 먼저: draw_halloween을 먼저 부르면 draw_pets 끝의 순환 import가 빈 모듈을 만나 깨진다
import draw_halloween  # noqa: I001
from conftest import assert_eat_has_no_bowl, assert_feet_only_peek, assert_frames_inside_cell, cell, record_draw_order
from sprite_spec import ANIMS, FRAME, ROLL_CENTER, ROLL_RADIUS, STAGES, sheet_128

PUMPKIN_GROUNDED = {"idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
                    "hungry", "level_up", "belly"}
GHOST_GROUNDED = {"sleep", "annoyed", "land", "belly"}  # 유령은 떠다니고 이 동작만 바닥에 내려앉는다
GHOST_HOVER = {"walk", "run", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "level_up"}


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    for sp in (draw_halloween.PUMPKIN, draw_halloween.GHOST_SP):
        draw_pets.write_species(sp, root / sp.id)
    return root


def names(pid):
    return [a for a, *_ in draw_pets.rows_for(draw_pets.SPECIES[pid])]


def frame(out, pid, anim, col=0, st="teenager"):
    return cell(sheet_128(out / pid / f"sheet_{st}.png").convert("RGBA"), names(pid).index(anim), col)


def pixels(im):
    """RGBA 픽셀 목록 (Image.getdata는 Pillow 14에서 사라진다)."""
    b = im.convert("RGBA").tobytes()
    return [tuple(b[k:k + 4]) for k in range(0, len(b), 4)]


def glow_pixels(im):
    """밝은 노랑 불빛 (조각한 눈·입 안쪽)."""
    return sum(1 for p in pixels(im) if p[3] > 200 and p[0] > 245 and p[1] > 210 and p[2] < 150)


def test_both_are_registered_with_every_required_anim():
    for pid, extra in (("pumpkin", ["climb", "slide", "lookup"]), ("ghost", ["lookup"])):  # 호박은 벽을 탄다, 유령은 올려다보기뿐 (침·기어오르기 없음)
        sp = draw_pets.SPECIES[pid]
        assert sp.poses is draw_halloween.POSES
        assert names(pid) == [a for a, *_ in ANIMS] + extra
        assert all(a in draw_halloween.POSES for a in names(pid))


@pytest.mark.parametrize("pid", ["pumpkin", "ghost"])
def test_sheets_have_every_frame_inside_the_cell(out, pid):
    grounded = PUMPKIN_GROUNDED if pid == "pumpkin" else GHOST_GROUNDED
    hover = GHOST_HOVER if pid == "ghost" else ()
    assert_frames_inside_cell(out / pid, draw_pets.SPECIES[pid], STAGES, grounded=grounded, hover=hover)


def test_manifests(out):
    pk = json.loads((out / "pumpkin" / "pet.json").read_text(encoding="utf-8"))
    assert pk["name"] == "호박" and pk["personality"] == "playful" and "locomotion" not in pk and pk["hop_share"] == 1.0
    assert pk["favorite_food"] == {"emoji": "🍬", "name": "사탕"}
    assert pk["speech"]["chatter"] == ["trick or treat!", "boo~", "♪"] and pk["speech"]["grumpy"] == ["hmph!"]
    assert pk["speech"]["hungry"] and pk["speech"]["full"] and pk["speech"]["level_up"]
    assert all(v == 0.9 for v in pk["stage_size"].values()) and pk["stages"] == STAGES
    gh = json.loads((out / "ghost" / "pet.json").read_text(encoding="utf-8"))
    assert gh["name"] == "유령" and gh["personality"] == "friendly" and gh["locomotion"] == "fly" and "hop_share" not in gh
    assert gh["favorite_food"] == {"emoji": "🍭", "name": "막대사탕"} and gh["walk_speed"] == 40
    assert gh["speech"]["chatter"] == ["boo!", "ooo~"] and gh["speech"]["tickle"] == ["hehe!"]
    assert all(v == 0.85 for v in gh["stage_size"].values())
    assert set(gh["animations"]) == {a for a, *_ in ANIMS} | {"lookup"}  # sting·climb 없음
    assert set(pk["animations"]) == {a for a, *_ in ANIMS} | {"climb", "slide", "lookup"}  # 호박은 벽을 탄다
    assert pk["animations"]["climb"] == {"row": len(ANIMS), "frames": 4, "fps": 6} and pk["animations"]["slide"] == {"row": len(ANIMS) + 1, "frames": 2, "fps": 8}
    for m in (pk, gh):
        assert m["animations"]["eat"] == {"row": 19, "frames": 4, "fps": 6, "front": True}


@pytest.mark.parametrize("pid", ["pumpkin", "ghost"])
def test_eat_has_no_bowl_and_mouth_sits_over_the_food(out, pid):
    for st in ("baby", "teenager", "adult"):
        assert_eat_has_no_bowl(out / pid / f"sheet_{st}.png", draw_pets.SPECIES[pid], most=20)
        # 입(앙 벌린 입)이 그릇 밥 더미 위 (프레임 x 50–78, y 70–96)
        im = frame(out, pid, "eat", 0, st)
        px = im.load()
        dark = {"pumpkin": lambda p: p[0] > 245 and p[1] > 205 and p[2] < 190,  # 빛나는 입
                "ghost": lambda p: abs(p[0] - draw_pets.MOUTH_IN[0]) + abs(p[1] - draw_pets.MOUTH_IN[1]) + abs(p[2] - draw_pets.MOUTH_IN[2]) < 40}[pid]
        mouth = [(x, y) for y in range(70, 97) for x in range(50, 79) if px[x, y][3] > 200 and dark(px[x, y])]
        assert len(mouth) >= 6, (pid, st, len(mouth))


def test_pumpkin_eyes_glow_and_dim_to_embers_when_asleep(out):
    for st in STAGES:
        for anim, n in (("walk", 4), ("idle", 2), ("sit_front", 3)):
            for c in range(n):
                assert glow_pixels(frame(out, "pumpkin", anim, c, st)) >= 25, (st, anim, c)
        for c in range(2):
            assert glow_pixels(frame(out, "pumpkin", "sleep", c, st)) <= 4, (st, c)  # 잉걸불 (어둑한 주황)


def test_pumpkin_glow_flickers_between_frames(out):
    a, b = frame(out, "pumpkin", "idle", 0), frame(out, "pumpkin", "idle", 1)
    halo = lambda im: sum(p[0] + p[1] for p in pixels(im) if p[3] > 200)
    assert halo(a) != halo(b)


def test_ghost_body_is_translucent_but_eyes_are_solid(out):
    for st in STAGES:
        im = frame(out, "ghost", "sit_front", 0, st)
        a = im.getchannel("A")
        x0, y0, x1, y1 = a.getbbox()
        # 머리 꼭대기 바로 아래, 가운데 꼭지를 피한 왼쪽 (외곽선 안쪽 몸)
        top = next(y for y in range(y0, y1) if a.getpixel((54, y)) >= 250)
        inner = [a.getpixel((54 + dx, y)) for y in range(top + 6, top + 11) for dx in (-3, 0, 3)]
        assert all(150 <= v < 255 for v in inner), (st, inner)
        dark = [p for p in pixels(im) if p[3] > 0 and p[0] < 110 and p[1] < 90 and p[2] < 90]
        assert sum(1 for p in dark if p[3] == 255) >= 40, st  # 눈은 또렷 (반투명 몸 위에 불투명)


def test_ghost_belly_glows_warm(out):
    im = frame(out, "ghost", "sit_front", 0)
    warm = sum(1 for p in pixels(im) if p[3] > 150 and p[0] > 240 and 170 <= p[1] <= 225 and p[2] < 170)
    assert warm >= 60, warm


def test_hop_lifts_off(out):
    for pid in ("pumpkin", "ghost"):
        up = min(frame(out, pid, "hop", c).getchannel("A").getbbox()[3] for c in (1, 2))  # 공중 (유령 1은 아래 속도선)
        rest = frame(out, pid, "hop", 0).getchannel("A").getbbox()
        assert up < rest[3] - 4, (pid, up, rest)


def test_roll_ball_fits_rotation_circle(out):
    cx, cy = ROLL_CENTER
    for pid in ("pumpkin", "ghost"):
        for st in ("baby", "adult"):
            for c in range(2):
                im = frame(out, pid, "roll", c, st)
                a = im.load()
                for y in range(FRAME):
                    for x in range(FRAME):
                        if a[x, y][3] > 0:
                            assert math.hypot(x + 0.5 - cx, y + 0.5 - cy) <= ROLL_RADIUS + 0.5, (pid, st, c, x, y)


def test_floating_ghost_sheet_hangs_longer_but_head_keeps_its_size():
    """떠 있을 때 흰 천 하단이 몸 높이의 12–18%만큼 더 길다 (머리 반지름은 그대로)."""
    for lvl, st in draw_halloween.GH_STAGE.items():
        height = 2.3 * st["r"] + st["L"]
        extra = draw_halloween._skirt(st) - st["L"]
        assert 0.12 * height <= extra <= 0.18 * height, (lvl, extra, height)
    assert [st["r"] for st in draw_halloween.GH_STAGE.values()] == [30, 30, 29.5, 29, 29]


# ---- 호박 뒷모습 (벽 타기) ----
def _near(im, col, tol):
    return sum(1 for p in pixels(im) if p[3] > 200 and sum(abs(p[k] - col[k]) for k in range(3)) < tol)


def _back(out, st):
    return [frame(out, "pumpkin", "climb", c, st) for c in range(4)] + [frame(out, "pumpkin", "slide", c, st) for c in range(2)]


def test_pumpkin_back_has_ribs_and_stem_but_no_glowing_face(out):
    for st in STAGES:
        for n, im in enumerate(_back(out, st)):
            assert glow_pixels(im) == 0, (st, n)  # 불빛 없음 (앞모습은 ≥ 25)
            assert _near(im, draw_halloween.GLOW_CORE, 30) == 0 and _near(im, draw_halloween.CARVE, 12) == 0, (st, n)  # 조각한 구멍 없음
            assert _near(im, draw_halloween.PUMP_RIDGE, 60) >= 60, (st, n)  # 골이 보인다
            if st == "adult":  # 어른은 꼭지 자리에 마녀 모자
                assert _near(im, draw_halloween.HAT, 24) >= 40, (st, n)
            else:
                assert _near(im, draw_halloween.STEM, 30) >= 12, (st, n)


def test_pumpkin_back_limbs_are_behind_the_body(monkeypatch):
    orders = record_draw_order(monkeypatch, [(draw_halloween, "pk_back_foot", "foot"), (draw_halloween, "pk_back_arm", "arm")])
    sp = draw_halloween.PUMPKIN
    for st in STAGES:
        with draw_pets.stage(st, sp.stage_tweak.get(st)):
            draw_pets.frames(draw_pets.staged_species(sp), "climb", 4)
            draw_pets.frames(draw_pets.staged_species(sp), "slide", 2)
    assert len(orders) == 6 * len(STAGES)
    for o in orders:
        assert o[:4].count("foot") == 2 and o[:4].count("arm") == 2 and "foot" not in o[4:] and "arm" not in o[4:], o  # 몸·꼭지는 그 뒤


def test_pumpkin_feet_only_peek_and_take_turns(monkeypatch):
    assert_feet_only_peek(monkeypatch, draw_halloween.PUMPKIN, draw_halloween, "pk_back_foot", lambda *a, **k: draw_pets.Part([], draw_halloween.PUMP_FOOT), STAGES)
