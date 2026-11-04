"""개구리 시트 검사: 크기·투명 여백·모든 동작·pet.json·eat에 밥그릇 없음·뒷모습 앞발은 머리 뒤."""
import json

import numpy as np
import pytest
from PIL import ImageChops

import draw_frog
import draw_pets
from conftest import assert_eat_has_no_bowl, assert_feet_only_peek, assert_frames_inside_cell, assert_roll_fits_circle, cell, record_draw_order, spy_compose
from sprite_spec import ANIMS, FRAME, STAGES, sheet_128

GROUNDED = {"walk", "idle", "look", "sleep", "land", "sit_front", "wave", "tilt", "stretch", "heart", "tickle", "annoyed",
            "hungry", "eat", "level_up"}


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    d = tmp_path_factory.mktemp("pets") / "frog"
    draw_pets.write_species(draw_frog.FROG, d)
    return d


def rows():
    return draw_pets.rows_for(draw_frog.FROG)


def test_frog_is_registered_with_its_own_poses():
    assert draw_pets.SPECIES["frog"] is draw_frog.FROG
    names = [a for a, *_ in rows()]
    assert names == [a for a, *_ in ANIMS] + ["climb", "slide", "bigjump", "lookup"]
    assert all(a in draw_frog.FROG_POSES for a in names)


def test_sheets_have_every_frame_inside_the_cell(out):
    assert_frames_inside_cell(out, draw_frog.FROG, STAGES, grounded=GROUNDED)


def test_roll_ball_fits_rotation_circle(out):
    assert_roll_fits_circle(out, draw_frog.FROG, STAGES)


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
    assert_eat_has_no_bowl(out / "sheet_teenager.png", draw_frog.FROG, most=20)  # 그릇이 있으면 수백 픽셀 (안티에일리어싱 몇 점만 허용)
    tongue = 0
    for c in range(4):
        px = cell(sheet, row, c).load()
        for y in range(FRAME):
            for x in range(FRAME):
                p = px[x, y]
                if p[3] < 200:
                    continue
                if c == 1 and 78 <= y <= 96 and 54 <= x <= 80 and sum(abs(p[k] - draw_pets.TONGUE[k]) for k in range(3)) < 20:
                    tongue += 1
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
    spy_compose(monkeypatch, orders.append)
    with draw_pets.stage("teenager"):
        draw_pets.frames(draw_frog.FROG, "climb", 1)
    parts = orders[0]
    head = next(i for i, p in enumerate(parts) if p.outline and len(p.shapes) == 3)  # 머리 + 눈 언덕 둘
    assert head >= 3  # 앞발 둘 + 몸 다음


def test_back_view_feet_are_behind_the_body(monkeypatch):
    """뒷모습: 뒷발·몸·머리 순서 — 발은 몸 뒤에 숨어 엉덩이를 가리지 않는다."""
    orders = record_draw_order(monkeypatch, [(draw_frog, "frog_back_foot", "leg"), (draw_frog, "frog_back_body", "body")])
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
    assert_feet_only_peek(monkeypatch, draw_frog.FROG, draw_frog, "frog_back_foot", lambda sp, *a, **k: draw_pets.Part([], sp.base), STAGES, staged=False)


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


def test_hungry_has_one_arm_on_each_side(monkeypatch):
    """한 앞발로 배를 문지른다 — 두 팔이 다 왼쪽으로 몰리면 안 된다 (arms[1]은 frog_front가 좌우로 뒤집는다)."""
    sp = draw_frog.FROG
    tips = []
    real = draw_frog._limb
    monkeypatch.setattr(draw_frog, "_limb", lambda ch, *a, **k: (tips.append(ch[-1]) if k.get("pad_r") == 3.1 else None) or real(ch, *a, **k))
    with draw_pets.stage("teenager"):
        draw_frog.frog_hungry(sp, 1)
    assert len(tips) == 2 and tips[0][0] < 64 < tips[1][0], tips


def test_right_cheek_puff_changes_only_the_right_side():
    """cheeks="right"는 오른쪽 볼만 (True는 양쪽)."""
    sp = draw_frog.FROG
    with draw_pets.stage("teenager"):
        plain = draw_frog.frog_front(sp)
        right = draw_frog.frog_front(sp, cheeks="right")
        both = draw_frog.frog_front(sp, cheeks=True)
    box = ImageChops.difference(plain.getchannel("A"), right.getchannel("A")).getbbox()
    assert box and box[0] >= 64, box
    assert ImageChops.difference(plain.getchannel("A"), both.getchannel("A")).getbbox()[0] < 64


@pytest.mark.parametrize("st", STAGES)
def test_tongue_tip_lands_on_the_fly_spot(st):
    """FLY_SPOT은 프레임 좌표 — 단계마다 view 배율이 달라도 혀 끝이 그 자리에 온다."""
    sp = draw_frog.FROG
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        im = draw_pets.frames(draw_pets.staged_species(sp), "eat", 4)[1]
        k = sp.scale * draw_pets._ST["fit"]
    a = np.asarray(im.convert("RGBA")).astype(int)
    pink = (np.abs(a[..., :3] - draw_pets.TONGUE).sum(axis=2) < 12) & (a[..., 3] > 200)
    bottom = np.nonzero(pink)[0].max() + 1
    want = draw_frog.FLY_SPOT[1] + 5.0 * 0.95 * k  # 혀끝 원의 아래 끝
    assert abs(bottom - want) <= 2.0, (st, bottom, want)  # 안티에일리어싱·_fit 1px씩
