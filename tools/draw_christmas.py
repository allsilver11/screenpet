"""크리스마스 친구들 🦌🎅 — 코가 빨갛게 빛나는 루돌프(rudolph)와 꼬마 산타(santa). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을
그대로 쓴다.

    python tools/draw_christmas.py → ui/pets/{rudolph,santa}/{sheet_<stage>.png ×5, sheet.png, pet.json}
                                     + tools/preview/stages_{rudolph,santa}.png
    (python tools/draw_pets.py도 둘을 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

루돌프: 네 발 동물 공통 뼈대(draw_pets POSES)에 사슴 귀·뿔·크림 주둥이를 입힌다. 뿔은 단계마다 자란다 (아기 = 동글 뿔 싹 →
        유아 짧은 Y → 어린이 가지 하나 → 청소년 가지 둘 → 어른 가지 셋). 아기·유아는 등에 아기 사슴 흰 점, 어린이부터 초록 리본 목걸이 +
        금방울. 빨간 코는 그린 뒤에 찾아서 (코 색 NOSE_KEY) 조금 키우고, 반짝 하이라이트 + 부드러운 빨간 후광을 덧칠한다
        (프레임마다 세기가 살짝 달라 반짝반짝, 잘 때는 은은하게). 걸음 = 가볍게 통통 (pet.json hop_share 0.3).
산타: 큰 머리 꼬마 산타 — 빨간 옷·모자(흰 방울), 복슬 흰 수염, 발그레 볼, 동글 배, 검은 허리띠 + 금 버클, 작은 장화.
      단계: 아기 = 모자 + 작은 수염 뭉치 (몸은 동글 빨간 콩) → 유아 장갑 → 어린이 허리띠 → 청소년 콧수염 + 버클 → 어른 큰 배 + 선물 자루.
      roll(구르기) 대신 썰매를 타고 미끄러진다 (옆모습, 방울·목도리 펄럭, 썰매 날 뒤로 눈가루) — pet.json roll에 spin이 없어 앱이 돌리지 않는다.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다).
"""

import math
import sys
from dataclasses import replace
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, LINE, PINK, T, WHITE, Ear, L, Species, _ST, arc, mix, view, xf)  # noqa: E402
from sprite_spec import FRAME, STAGES  # noqa: E402

# =====================================================================
# 루돌프 🦌
# =====================================================================
DEER, DEER_CREAM, DEER_LINE = (184, 122, 80), (255, 238, 212), (110, 70, 48)
ANTLER, ANTLER_LINE = (236, 204, 150), (150, 110, 70)
NOSE_KEY = (226, 30, 52)  # 그린 뒤 코를 찾는 색 (다른 곳에 쓰지 않는다)
NOSE_RED, NOSE_DEEP, NOSE_SHINE = (236, 40, 56), (196, 26, 46), (255, 214, 214)
GLOW = (255, 72, 72)
RIBBON, RIBBON_DARK, BELL, BELL_LIGHT = (84, 168, 104), (60, 128, 80), (250, 200, 72), (255, 236, 160)
TWINKLE = [1.0, 0.78, 0.94, 0.84]
NOSE_GROW = 1.65  # 그린 코(엔진의 작은 코)보다 이만큼 크게 — 코가 루돌프의 포인트  # 코 후광 세기 (프레임마다)


def _antler(lvl):
    """왼쪽 뿔 ('L', 점들, 굵기) 목록 — 앞모습 머리 좌표 (머리 중심 (64, 50), 윗선 y ≈ 17)."""
    return {
        0: [("L", [(45, 22), (43, 12)], 7.5)],
        1: [("L", [(45, 22), (42, 7)], 6.5), ("L", [(43, 13), (35, 9)], 5.0)],
        2: [("L", [(45, 22), (42, 6), (37, -2)], 6.2), ("L", [(43, 11), (33, 7)], 5.0)],
        3: [("L", [(45, 22), (42, 5), (36, -6)], 6.2), ("L", [(43, 12), (32, 7)], 5.0), ("L", [(41, 1), (46, -7)], 4.6)],
        4: [("L", [(46, 22), (42, 5), (33, -8)], 7.4), ("L", [(43, 13), (30, 9)], 6.0), ("L", [(41, 2), (48, -8)], 5.8),
            ("L", [(37, -3), (27, -5)], 5.4)],
    }[lvl]


def _mirror_desc(d):
    kind, *args = d
    if kind == "L":
        return ("L", [(128 - x, y) for x, y in args[0]], args[1])
    if kind == "T":
        return ("T", *[(128 - x, y) for x, y in args])
    x0, y0, x1, y1 = args
    return ("E", 128 - x1, y0, 128 - x0, y1)


def _side_antler(lvl):
    """옆모습 뿔 하나 (머리 좌표 (88, 45) 중심, 오른쪽을 본다): 줄기는 뒤로 휘고 가지는 앞으로 — 녹이기에 메워지지 않게 사이를 넓게."""
    return {
        0: [("L", [(84, 20), (83, 10)], 7.5)],
        1: [("L", [(84, 22), (81, 6)], 6.5), ("L", [(82, 13), (91, 8)], 5.0)],
        2: [("L", [(85, 22), (81, 5), (74, -3)], 6.2), ("L", [(82, 12), (94, 6)], 5.0)],
        3: [("L", [(85, 22), (80, 5), (70, -3)], 6.2), ("L", [(82, 13), (96, 7)], 5.0), ("L", [(78, 2), (85, -8)], 4.6)],
        4: [("L", [(85, 22), (80, 5), (67, -4)], 6.4), ("L", [(82, 14), (97, 8)], 5.0), ("L", [(78, 2), (87, -8)], 4.8),
            ("L", [(71, -1), (69, -11)], 4.4)],
    }[lvl]


def _deer_parts(lvl, t):
    """단계별 귀·뿔 설정 (파스텔 t)."""
    lear = [("T", (34, 30), (18, 21), (5, 22), (2, 28), (12, 37), (30, 42))]
    lear_in = [("T", (28, 31), (16, 25), (8, 26), (12, 33), (27, 37))]
    ant = _antler(lvl)
    front_ears = lear + ant + [_mirror_desc(d) for d in lear + ant]
    cover = [("L", pts, w + 1.2) for _, pts, w in ant]
    marks = [(lear_in + [_mirror_desc(d) for d in lear_in], PINK),
             (cover + [_mirror_desc(d) for d in cover], ANTLER)]
    sear = Ear([("T", (76, 22), (60, 12), (48, 12), (46, 18), (56, 27), (72, 31))], (70, 24), DEER,
               [([("T", (71, 22), (58, 15), (51, 16), (56, 23), (69, 27))], PINK)])
    near = Ear(_side_antler(lvl), (88, 18), ANTLER)
    return dict(front_ears=front_ears, front_ear_marks=dp._pale(marks, t), side_ears=dp._pale([sear, near], t))


def _nose_glow(im, i, lit=1.0):
    """코(NOSE_KEY 픽셀)를 찾아 1px 키우고, 동글 빨간 코 + 하이라이트 + 부드러운 후광을 덧칠한다."""
    a = np.asarray(im).astype(np.int16)
    d = np.abs(a[..., :3] - np.array(NOSE_KEY)).sum(axis=2)
    core = (d < 30) & (a[..., 3] > 160)
    if core.sum() < 3:
        return im
    ys, xs = np.nonzero(core)
    if xs.max() > FRAME - 18:  # 옆모습 코가 칸 끝에 붙어 있으면 후광이 잘리지 않게 몸 전체를 왼쪽으로 조금 민다
        x0 = im.getchannel("A").getbbox()[0]
        dx = min(11, xs.max() - (FRAME - 18), x0 - 1)
        if dx > 0:
            sh = Image.new("RGBA", im.size, (0, 0, 0, 0))
            sh.paste(im, (-dx, 0))
            im = sh
            a = np.asarray(im).astype(np.int16)
            d = np.abs(a[..., :3] - np.array(NOSE_KEY)).sum(axis=2)
            core = (d < 30) & (a[..., 3] > 160)
    m = Image.fromarray((core * 255).astype(np.uint8))
    # 동글 코: 찾은 코 둘레를 1.5px씩 넓힌 타원 (작은 옆모습 코도 동그랗게)
    ys, xs = np.nonzero(core)
    # 루돌프의 자랑: 찾은 코보다 1.65배 큰 동글 코 (입을 덜 가리게 조금 위로)
    w = max(xs.max() - xs.min() + 4.0, 9.0) * NOSE_GROW
    h = max(ys.max() - ys.min() + 4.0, 8.0, 0.8 * w / NOSE_GROW) * NOSE_GROW
    h = max(h, 0.84 * w)
    ncx, ncy = (xs.min() + xs.max() + 1) / 2, (ys.min() + ys.max() + 1) / 2 - 0.18 * h
    bx0, bx1, by0, by1 = ncx - w / 2, ncx + w / 2, ncy - h / 2, ncy + h / 2
    from PIL import ImageDraw
    hi = Image.new("L", (FRAME * 4, FRAME * 4), 0)
    ImageDraw.Draw(hi).ellipse((bx0 * 4, by0 * 4, bx1 * 4, by1 * 4), fill=255)
    big = hi.resize(im.size, Image.LANCZOS)
    out = im.copy()
    tw = TWINKLE[i % 4] * lit
    # 후광 (몸 위·밖으로 번진다, 칸 1px 여백 안으로 자른다)
    halo = big.filter(ImageFilter.MaxFilter(3)).filter(ImageFilter.GaussianBlur(4.2))
    ha = np.asarray(halo).astype(np.float32) * min(1.0, 1.25 * tw)
    ha[:1, :] = ha[-1:, :] = 0
    ha[:, :1] = ha[:, -1:] = 0
    glow = Image.new("RGBA", im.size, GLOW + (0,))
    glow.putalpha(Image.fromarray(np.clip(ha, 0, 255).astype(np.uint8)))
    under = Image.new("RGBA", im.size, (0, 0, 0, 0))
    under.alpha_composite(glow)
    under.alpha_composite(out)  # 몸 밖에는 후광이 비치고
    tint = glow.copy()
    tint.putalpha(Image.fromarray((np.clip(ha, 0, 255) * 0.42).astype(np.uint8)))
    under.alpha_composite(tint)  # 몸 위에는 살짝 붉게 물든다
    out = under
    # 코 본체: 테두리 짙은 빨강 → 안쪽 밝은 빨강 → 하이라이트
    ys, xs = np.nonzero(np.asarray(big) > 127)
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    rim = Image.new("RGBA", im.size, NOSE_DEEP + (255,))
    out.paste(rim, (0, 0), big.filter(ImageFilter.GaussianBlur(0.4)))
    inner = big.filter(ImageFilter.MinFilter(3)) if (x1 - x0) >= 7 else m
    out.paste(Image.new("RGBA", im.size, NOSE_RED + (255,)), (0, 0), inner.filter(ImageFilter.GaussianBlur(0.4)))
    w, h = x1 - x0, y1 - y0
    sh = Image.new("L", im.size, 0)
    from PIL import ImageDraw
    dd = ImageDraw.Draw(sh)
    sx, sy = x0 + w * 0.22, y0 + h * 0.18
    dd.ellipse((sx, sy, sx + max(2.0, w * 0.32), sy + max(1.6, h * 0.3)), fill=round(255 * min(1.0, 0.75 + 0.25 * tw)))
    out.paste(Image.new("RGBA", im.size, NOSE_SHINE + (255,)), (0, 0), sh.filter(ImageFilter.GaussianBlur(0.3)))
    if i % 2 == 0 and lit >= 0.9:  # 반짝 ✦ (코 오른쪽 위)
        r = 2.2 if i == 0 else 1.6
        cx, cy = min(FRAME - 4, x1 + 2), max(3, y0 - 2)
        st = Image.new("L", im.size, 0)
        sd = ImageDraw.Draw(st)
        sd.polygon([(cx, cy - r * 1.6), (cx + r * 0.45, cy - r * 0.45), (cx + r * 1.6, cy), (cx + r * 0.45, cy + r * 0.45),
                    (cx, cy + r * 1.6), (cx - r * 0.45, cy + r * 0.45), (cx - r * 1.6, cy), (cx - r * 0.45, cy - r * 0.45)], fill=230)
        out.alpha_composite(Image.composite(Image.new("RGBA", im.size, (255, 250, 230, 255)), Image.new("RGBA", im.size, (0, 0, 0, 0)), st))
    # 큰 코의 후광이 칸 끝까지 번지면 1px 투명 여백을 지킨다 (몸 전체를 밀지 않게)
    a = np.asarray(out).copy()
    a[:, -1, 3] = 0
    a[0, :, 3] = 0
    return Image.fromarray(a)


def _apple_bubble(cx, cy, r=12.5):
    """생각 풍선 안에 빨간 사과 🍎."""
    fx = []
    for x, y, rr in [(cx - r - 7, cy + r + 8, 2.0), (cx - r - 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], LINE, 1.0), ([C(x, y, rr)], WHITE, 0)]
    body = [C(cx - 3, cy + 1.5, 5.6), C(cx + 3, cy + 1.5, 5.6)]
    fx += [(body, (140, 50, 56), 1.0), ([C(cx - 3, cy + 1.5, 4.4), C(cx + 3, cy + 1.5, 4.4)], (244, 96, 92), 0),
           ([L([(cx, cy - 3), (cx + 1, cy - 8)], 1.6)], (120, 84, 54), 0),
           ([E(cx + 1, cy - 9, cx + 7, cy - 5.5)], (110, 184, 92), 0), ([C(cx - 4.4, cy - 0.5, 1.3)], WHITE, 0)]
    return fx


def _rudolph_hungry(sp, i):
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    return dp._front(sp, eyes="closed", mouth="frown", head=dict(dy=2, rot=-4, pivot=(64, 84)), head_kw=dict(brow="sad"),
                     tail_rot=6, paws=[(56 + dx, 100 + dy)], toes=True, fx=_apple_bubble(108, 17))


def _fit(fn):
    import draw_halloween
    return draw_halloween._fit(fn)


def _rudolph_pose(anim):
    base = _rudolph_hungry if anim == "hungry" else dp.POSES[anim]

    def f(sp, i):
        ssp = replace(sp, nose=NOSE_KEY, **_deer_parts(_ST["lvl"], _ST["pale"]))
        im = base(ssp, i)
        lit = 0.55 if anim == "sleep" else 1.0
        return _nose_glow(im, i, lit)
    f.__name__ = f"rudolph_{anim}"
    return f if anim == "roll" else _fit(f)  # 넓은 귀·뿔이 1px 여백을 넘으면 안쪽으로 (roll은 회전 원 안 그대로)


def _rudolph_poses():
    from sprite_spec import ANIMS
    return {a: _rudolph_pose(a) for a in [a for a, *_ in ANIMS] + ["lookup"]}


RUDOLPH_POSES = _rudolph_poses()

_ribbon_front = [("L", [(40, 80), (52, 86), (64, 88), (76, 86), (88, 80)], 6.0)]
_bell_front = [("E", 58.5, 86, 69.5, 97)]
RUDOLPH = Species(
    id="rudolph", name="루돌프", walk_speed=95, scale=0.94, base=DEER, back=DEER, belly=DEER_CREAM, nose=NOSE_KEY, mouth="dog",
    front_ears=[], front_ear_pivots=((24, 34), (104, 34)), front_ear_marks=[],
    front_head_marks=[([("E", 42, 50, 86, 82)], DEER_CREAM)],
    front_body_marks=[([("E", 44, 80, 84, 121)], DEER_CREAM), (_ribbon_front, RIBBON, 2),
                      ([("L", [(58, 84), (64, 86), (70, 84)], 2.0)], RIBBON_DARK, 2),
                      (_bell_front, BELL, 2), ([("E", 60.5, 87.5, 64.5, 91)], BELL_LIGHT, 2),
                      ([("L", [(60, 93.5), (68, 93.5)], 1.2)], ANTLER_LINE, 2)],
    front_tail=[("E", 92, 104, 108, 120)], front_tail_root=(98, 112), front_tail_color=DEER_CREAM,
    side_ears=[], ears_over=False,
    side_head_marks=[([("E", 98, 38, 134, 72)], DEER_CREAM)],
    side_torso_marks=[([("E", 26, 90, 94, 122)], DEER_CREAM),
                      ([("E", 38, 62, 45, 68), ("E", 52, 59, 59, 65), ("E", 66, 63, 73, 69), ("E", 46, 71, 52, 76),
                        ("E", 60, 72, 66, 77), ("E", 30, 68, 36, 73)], DEER_CREAM, (0, 1)),
                      ([("L", [(84, 56), (88, 68), (93, 82)], 6.5)], RIBBON, 2),
                      ([("E", 88, 80, 98, 90)], BELL, 2), ([("E", 89.5, 81.5, 93, 84.5)], BELL_LIGHT, 2)],
    side_tail=[("E", 8, 58, 28, 76)], tail_root=(20, 70), tail_color=DEER_CREAM,
    lie_ears=(-20, -16), outline=DEER_LINE, stripe=DEER, poses=RUDOLPH_POSES, specials=["lookup"],
    stage_size=dict.fromkeys(STAGES, 0.9),
    # 귀·뿔 크기는 _deer_parts가 단계마다 직접 정한다 (공통 ear 배율 1.0) · 모든 단계 반짝이 두 개 큰 눈, 어른도
    stage_tweak={"baby": dict(ear=1.0, pale=0.22, tuft=False), "kinder": dict(ear=1.0, pale=0.12),
                 "child": dict(ear=1.0, sparkle=True, smile=False, eye=1.18), "teenager": dict(ear=1.0, sparkle=True, eye=1.1),
                 "adult": dict(ear=1.0, sparkle=True, calm=False, eye=1.04, cheek_fluff=False)},
)


# =====================================================================
# 산타 🎅 — 할로윈 호박처럼 동글 몸 하나로 그리는 꼬마 (draw_halloween의 층 쌓기 _finish를 그대로 쓴다)
# =====================================================================
import draw_halloween as dh  # noqa: E402
from draw_halloween import _arm, _finish, _heart_items, _stars, _swing, speed_fx  # noqa: E402
from draw_pets import (anger_fx, bee_mouth, brows, heart_fx, question_fx, stage_eye, strand, sweat_fx, z_fx)  # noqa: E402

SUIT, SUIT_SHADE, SUIT_LINE = (232, 72, 72), (204, 52, 60), (128, 52, 56)
FUR, FUR_SHADE = (255, 255, 255), (232, 236, 244)
SKIN, CHEEK_RED = (255, 224, 200), (255, 150, 150)
BELT, BUCKLE, BUCKLE_IN = (66, 52, 56), (252, 204, 84), (66, 52, 56)
BOOT, BOOT_LIGHT = (72, 58, 62), (120, 102, 106)
MITT, MITT_LIGHT = (92, 168, 112), (140, 204, 150)
SACK, SACK_SHADE, SACK_TIE = (204, 156, 104), (176, 128, 84), (232, 72, 72)
GIFT, GIFT_RIBBON = (92, 168, 112), (252, 204, 84)
SLED, SLED_LIGHT, RUNNER = (226, 70, 70), (246, 128, 112), (196, 150, 92)
SNOW, SNOW_LINE = (255, 255, 255), (176, 196, 222)
SCARF, SCARF_STRIPE = (92, 168, 112), (255, 255, 255)
MILK_GLASS, MILK = (220, 236, 248), (255, 255, 255)
SA_FLOOR = 125.0

# 단계(lvl)별 — hr: 머리 반지름 · brx, bry: 몸 반지름 · beard: 수염 크기 · stache: 콧수염 · belt · buckle · mitt: 초록 장갑 · sack: 선물 자루
SA_STAGE = {
    0: dict(hr=29, brx=19, bry=16, beard=0.5, stache=False, belt=False, buckle=False, mitt=False, sack=False),
    1: dict(hr=29, brx=21, bry=17.5, beard=0.7, stache=False, belt=False, buckle=False, mitt=True, sack=False),
    2: dict(hr=28.5, brx=23, bry=19, beard=0.84, stache=False, belt=True, buckle=False, mitt=True, sack=False),
    3: dict(hr=28, brx=25, bry=20.5, beard=0.96, stache=True, belt=True, buckle=True, mitt=True, sack=False),
    4: dict(hr=28, brx=27.5, bry=22.5, beard=1.04, stache=True, belt=True, buckle=True, mitt=True, sack=True),
}


def _sa():
    return SA_STAGE[_ST["lvl"]]


def _layout(st, bottom, size=1.0, head_dy=0.0):
    """(몸 중심 y, 머리 중심 y) — 짧고 동글한 얼굴이 동글 배 위에 폭 얹힌다 (수염이 가슴을 덮는다)."""
    hr, bry = st["hr"] * size, st["bry"] * size
    cy_b = bottom - 8 * size - bry
    return cy_b, cy_b - bry * 0.7 - hr * 0.42 + head_dy


def mouth_dy(st, size=1.0):
    """머리 중심에서 입까지 (콧수염이 있으면 그 아래)."""
    return st["hr"] * size * (0.5 if st["stache"] else 0.42)


def _beard_fns(fx_, hy, hr, k):
    """복슬 흰 수염: 볼·턱을 감싸는 구름 고리 (양옆은 모자 테 바로 아래까지) + 가운데 채움. 아기(k < 0.65)는 턱 아래 작은 뭉치."""
    if k < 0.65:
        return [C(fx_ + u * hr, hy + 0.52 * hr, 0.24 * hr) for u in (-0.42, -0.14, 0.14, 0.42)] + \
            [E(fx_ - 0.5 * hr, hy + 0.4 * hr, fx_ + 0.5 * hr, hy + 0.84 * hr)]
    rx, ry = 0.9 * hr, 0.72 * hr * k
    fns = [E(fx_ - 0.84 * hr, hy + 0.22 * hr, fx_ + 0.84 * hr, hy + 0.2 * hr + 0.86 * hr * k)]
    for a in range(-20, 201, 20):
        r = math.radians(a)
        fns.append(C(fx_ + rx * math.cos(r), hy + 0.22 * hr + ry * math.sin(r), hr * (0.22 + 0.1 * k)))
    return fns


def _hat_parts(hx, hy, hr, side, pom, lvl):
    """크고 말랑한 빨간 모자 (끝이 한쪽으로 축 늘어짐) + 눈썹까지 푹 눌러쓴 복슬 흰 털 테 + 흰 방울. pom: 늘어진 끝 흔들림 (도)."""
    out = []
    flop = -1 if side else 1  # 옆모습은 뒤(왼쪽)로 늘어진다
    dome = E(hx - hr * 1.0, hy - hr * 1.3, hx + hr * 1.0, hy - hr * 0.22)
    root = (hx + flop * hr * 0.3, hy - hr * 1.12)
    with xf(rot=pom * flop, pivot=root):
        tip = (hx + flop * hr * 1.16, hy - hr * 0.5)
        tail = strand(dh._bez(root, (hx + flop * hr * 0.95, hy - hr * 1.32), tip, 10), hr * 0.66, 6.0)
        out.append(dp.Part([dome, tail], SUIT, [([E(hx - hr * 0.62, hy - hr * 1.18, hx - hr * 0.22, hy - hr * 0.92)], mix(SUIT, WHITE, 0.3))],
                           outline=True, grow=0))
        out.append(dp.Part([C(tip[0] + flop * 1.5, tip[1] + 2.5, hr * (0.27 if lvl else 0.3))], FUR,
                           [([C(tip[0] + flop * 3.5, tip[1] + 5, hr * 0.12)], FUR_SHADE)], outline=True, grow=0))
    brim = [E(hx - hr * 1.08, hy - hr * 0.6, hx + hr * 1.08, hy - hr * 0.18)]
    brim += [C(hx + hr * u, hy - hr * 0.39 + (1.0 if k % 2 else -0.8), hr * 0.23) for k, u in enumerate((-0.86, -0.43, 0.0, 0.43, 0.86))]
    out.append(dp.Part(brim, FUR, [([E(hx - hr * 1.0, hy - hr * 0.3, hx + hr * 1.0, hy - hr * 0.14)], FUR_SHADE)], outline=True, grow=0))
    return out


def _mitt_arm(sh, side, a, ln, w, lvl):
    """빨간 소매 + 흰 털 소맷부리 + 초록 장갑 (아기는 맨손)."""
    p = _arm(_SUITSP, sh, side, a, ln, w)
    if isinstance(a, (int, float)):
        r = math.radians(a)
        t = (sh[0] + side * math.sin(r) * ln, sh[1] + math.cos(r) * ln)
    else:
        t = a
    hand = dp.Part([C(t[0], t[1], w * 0.62)], MITT if lvl else SKIN,
                   [([C(t[0] - w * 0.18, t[1] - w * 0.2, w * 0.2)], MITT_LIGHT if lvl else WHITE)], outline=True, grow=0)
    ux, uy = t[0] - sh[0], t[1] - sh[1]
    n = math.hypot(ux, uy) or 1
    cuff = dp.Part([C(t[0] - ux / n * w * 0.55, t[1] - uy / n * w * 0.55, w * 0.58)], FUR, outline=True, grow=0)
    return [p, cuff, hand]


class _SuitSp:  # _arm은 sp.base 색만 쓴다
    base = SUIT


_SUITSP = _SuitSp()


def _sack_parts(cx, cy_b, brx, bry, side):
    """어른: 등에 멘 선물 자루 (입구로 초록 선물 상자가 빼꼼)."""
    if side:
        sx0, sy0, sx1, sy1 = cx - brx - 20, cy_b - bry - 14, cx - brx + 10, cy_b + bry * 0.7
    else:
        sx0, sy0, sx1, sy1 = cx - brx - 16, cy_b - bry - 22, cx - brx + 14, cy_b + 2
    mx = (sx0 + sx1) / 2
    gift = dp.Part([T((mx - 6, sy0 - 4), (mx + 8, sy0 - 6), (mx + 9, sy0 + 6), (mx - 5, sy0 + 8))], GIFT,
                   [([L([(mx + 1, sy0 - 5), (mx + 2, sy0 + 7)], 2.2)], GIFT_RIBBON)], outline=True, grow=0)
    bag = dp.Part([E(sx0, sy0, sx1, sy1)], SACK, [([E(sx0 + 2, (sy0 + sy1) / 2, sx1 + 4, sy1 + 6)], SACK_SHADE),
                                                   ([L([(sx0 + 6, sy0 + 5), (sx1 - 6, sy0 + 4)], 2.6)], SACK_TIE)], outline=True, grow=0)
    return [gift, bag]


def _flat(makers):
    out = []
    for f in makers:
        r = f()
        out += r if isinstance(r, list) else [r]
    return out


def santa(sp, *, side=False, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, arms=None,
          feet=((0, 0), (0, 0)), pom=0.0, fx=(), held=(), props=(), back=(), top=(), head_dy=0.0, head_rot=0.0, size=1.0,
          sack=True, trim=False):
    """산타 한 장. bottom: 장화 아래끝 y · sq: 아래끝 기준 찌그러짐 · rot: 몸 중심 기준 기울기 · arms: [(side, 각도 | 손끝)] ·
    feet: (왼, 오) 장화 오프셋 또는 None · pom: 모자 끝 흔들림 · head_dy/head_rot: 고개 · back: 맨 뒤 부품 · props: 맨 앞 부품."""
    st = _sa()
    lvl = _ST["lvl"]
    hr, brx, bry = st["hr"] * size, st["brx"] * size, st["bry"] * size
    bottom = SA_FLOOR - 1 if bottom is None else bottom
    cy_b, hy = _layout(st, bottom, size, head_dy)
    hx = cx + (3 if side else 0)
    fx_ = hx + (0.3 * hr if side else 0.0)
    parts, glow, face = _flat(back), [], []
    with xf(rot=rot, pivot=(cx, cy_b)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
            if st["sack"] and sack and not trim:
                parts += _sack_parts(cx, cy_b, brx, bry, side)
            if feet and not trim:
                xs = (cx - 0.2 * brx, cx + 0.35 * brx) if side else (cx - 0.48 * brx, cx + 0.48 * brx)
                for x, (dx, dy) in zip(xs, feet):
                    x0 = x + dx - (5 if side else 7.5) * size
                    x1 = x + dx + (10 if side else 7.5) * size
                    parts.append(dp.Part([E(x0, bottom - 12 * size + dy, x1, bottom + dy)], BOOT,
                                         [([E(x0 + 2, bottom - 8 * size + dy, x0 + 7, bottom - 5 * size + dy)], BOOT_LIGHT)], outline=True, grow=0))
            # 몸 (빨간 옷 + 흰 털 단 + 허리띠·버클)
            ox = cx + (0.42 * brx if side else 0)
            marks = [([E(cx - brx * 1.3, cy_b + bry * 0.6, cx + brx * 1.3, cy_b + bry * 1.7)], FUR),
                     ([E(cx - brx * 1.3, cy_b + bry * 1.05, cx + brx * 1.3, cy_b + bry * 1.7)], FUR_SHADE),
                     ([E(cx - brx * 0.7, cy_b - bry * 0.7, cx - brx * 0.3, cy_b - bry * 0.35)], mix(SUIT, WHITE, 0.3))]
            if lvl >= 1:
                marks.append(([L([(ox, cy_b - bry), (ox, cy_b + bry * 0.7)], 4.6 * size)], FUR))
            if st["belt"]:
                marks.append(([T((cx - brx * 1.1, cy_b + bry * 0.02), (cx + brx * 1.1, cy_b + bry * 0.02),
                                 (cx + brx * 1.1, cy_b + bry * 0.36), (cx - brx * 1.1, cy_b + bry * 0.36))], BELT))
            if st["buckle"]:
                bw, bh = 5.2 * size, bry * 0.26
                yb = cy_b + bry * 0.19
                marks += [([T((ox - bw, yb - bh), (ox + bw, yb - bh), (ox + bw, yb + bh), (ox - bw, yb + bh))], BUCKLE),
                          ([T((ox - bw * 0.45, yb - bh * 0.4), (ox + bw * 0.45, yb - bh * 0.4), (ox + bw * 0.45, yb + bh * 0.4),
                              (ox - bw * 0.45, yb + bh * 0.4))], BUCKLE_IN)]
            parts.append(dp.Part([E(cx - brx, cy_b - bry, cx + brx, cy_b + bry)], SUIT, marks, grow=1))
            if arms is None:
                arms = [(1, 28)] if side else [(-1, 30), (1, 30)]
            arm_parts = []
            if not trim:
                for s_, a in arms:
                    sh = (cx + 0.45 * brx, cy_b - 0.35 * bry) if side else (cx + s_ * 0.82 * brx, cy_b - 0.42 * bry)
                    arm_parts += _mitt_arm(sh, s_, a, 9.5 * size, 8.5 * size, lvl)
            parts += [h() for h in held]
            # 머리: 가로로 넓은 짧은 얼굴 — 모자 테와 수염 사이로 빼꼼
            with xf(rot=head_rot, pivot=(hx, hy + hr * 0.6)):
                parts.append(dp.Part([E(hx - hr, hy - hr * 0.7, hx + hr, hy + hr * 0.62)], SKIN, outline=True, grow=0))
                k = st["beard"] * size
                parts.append(dp.Part(_beard_fns(fx_, hy, hr, k), FUR,
                                     [([E(fx_ - 0.7 * hr, hy + 0.55 * hr * k + 0.3 * hr, fx_ + 0.7 * hr, hy + 0.95 * hr * k + 0.3 * hr)], FUR_SHADE)],
                                     outline=True, grow=0))
                if st["stache"]:
                    parts.append(dp.Part([E(fx_ - 0.38 * hr, hy + 0.2 * hr, fx_ - 0.01 * hr, hy + 0.42 * hr),
                                          E(fx_ + 0.01 * hr, hy + 0.2 * hr, fx_ + 0.38 * hr, hy + 0.42 * hr)], FUR, outline=True, grow=0))
                parts += _hat_parts(hx, hy, hr, side, pom, lvl)
                # 얼굴 층: 동글 볼 · 작은 점 눈 · 작은 코 · 작은 입
                ey = hy + 0.03 * hr
                sp_x = 0.34 * hr if not side else 0.3 * hr
                for j, ex in enumerate((fx_ - sp_x, fx_ + sp_x)):
                    kx = 0.86 if side and j == 0 else 1.0
                    with xf(sx=kx, pivot=(ex, ey)):
                        face += dh._ops(stage_eye(eyes, ex, ey))
                    if brow:
                        face += dh._ops(brows(brow, ex, ey - 1 + _ST["eye_dy"], flip=j == 1))
                bx_, by_ = _ST["blush"]
                for cxx in (fx_ - 0.6 * hr + (0.12 * hr if side else 0), fx_ + 0.6 * hr):
                    glow.append(([E(cxx - 0.17 * hr * bx_, hy + 0.2 * hr - 0.12 * hr * by_, cxx + 0.17 * hr * bx_, hy + 0.2 * hr + 0.12 * hr * by_)],
                                 CHEEK_RED, 0.75, 0, 0.6))
                face += dh._ops(bee_mouth(mouth, fx_ + (1 if side else 0), hy + mouth_dy(st, size)))
                nx, ny = fx_ + (1.5 if side else 0), hy + 0.2 * hr
                face.append(([C(nx, ny, 0.11 * hr + 0.8)], (150, 96, 90), 1.0, 0, 0))
                face.append(([C(nx, ny, 0.11 * hr)], (250, 166, 156), 1.0, 0, 0))
                face.append(([C(nx - 0.04 * hr, ny - 0.04 * hr, 0.035 * hr)], WHITE, 1.0, 0, 0))
            parts += arm_parts
    parts += _flat(props)
    return _finish(parts, glow, face + list(top), fx)


# ---- 효과 ----
def milk_bubble_fx(cx, cy):
    """생각 풍선 안에 우유 한 잔 + 쿠키."""
    fx = dh._bubble(cx, cy)
    glass = T((cx - 6.5, cy - 7), (cx + 3.5, cy - 7), (cx + 2.5, cy + 7), (cx - 5.5, cy + 7))
    fx += [([glass], LINE, 0.9), ([glass], MILK_GLASS, 0),
           ([T((cx - 6, cy - 3.5), (cx + 3, cy - 3.5), (cx + 2.4, cy + 6.2), (cx - 4.9, cy + 6.2))], MILK, 0),
           ([C(cx + 6, cy + 3.5, 4.6)], LINE, 0), ([C(cx + 6, cy + 3.5, 3.6)], (226, 170, 104), 0),
           ([C(cx + 5, cy + 2.5, 0.9), C(cx + 7.4, cy + 4.6, 0.9)], (120, 76, 52), 0)]
    return fx


def snow_puffs(items):
    out = []
    for x, y, r in items:
        out += [([C(x, y, r)], SNOW_LINE, 0), ([C(x, y, r - 1.1)], SNOW, 0)]
    return out


# ---- 포즈 (호박 포즈를 산타 몸짓으로) ----
def sa_walk(sp, i):
    """뒤뚱뒤뚱: 몸을 좌우로 기울이며 장화를 번갈아."""
    up = [0, 2, 0, 2][i]
    feet = [((-3, 0), (3, -3)), ((0, 0), (0, 0)), ((3, -3), (-3, 0)), ((0, 0), (0, 0))][i]
    return santa(sp, side=True, bottom=SA_FLOOR - 1 - up, rot=[-5, 0, 5, 0][i], feet=feet, pom=[6, -4, -8, 2][i],
                 arms=[(1, [10, 30, 50, 30][i])])


def sa_run(sp, i):
    up = [0, 4, 6, 3][i]
    feet = ((-6, 0), (6, -4)) if i % 2 == 0 else ((5, -4), (-5, 0))
    return santa(sp, side=True, bottom=SA_FLOOR - 1 - up, rot=[4, 8, 6, 8][i], feet=feet, pom=[14, 22, 18, 22][i], eyes="happy",
                 mouth="open", arms=[(1, [60, 90, 60, 90][i])], fx=speed_fx(4, (70, 88, 104), 12, [0, -3, 0, -3][i]))


def sa_roll(sp, i):
    """구르기 대신 썰매 🛷: 옆모습으로 썰매에 앉아 미끄러진다 (앱이 돌리지 않는다 — pet.json roll에 spin 없음).
    모자 방울·초록 목도리가 뒤로 펄럭, 썰매 날 뒤로 눈가루."""
    st = _sa()
    base = SA_FLOOR - 1
    run_y = base - 1.5
    seat = base - 13
    s0, s1 = 22, 104

    def sled():
        rails = L([(s0 - 2, run_y)] + [(x, run_y) for x in (40, 70)] + dh._bez((90, run_y), (102, run_y - 1), (107, run_y - 10), 6)[1:], 3.0)
        curl = L(arc(103, run_y - 11, 4, 4, 0, -200, 10), 3.0)
        struts = [L([(x, run_y), (x, seat + 6)], 3.0) for x in (36, 82)]
        runner = dp.Part([rails, curl] + struts, RUNNER, outline=True, grow=0)
        box = T((s0, seat - 3), (s1 - 6, seat - 3), (s1, seat - 9), (s1 + 2, seat - 3), (s1 - 2, seat + 6), (s0 + 3, seat + 7))
        body = dp.Part([box], SLED, [([L([(s0 + 4, seat - 1), (s1 - 6, seat - 1)], 1.8)], SLED_LIGHT),
                                     ([L([(s0 + 8, seat + 3), (s1 - 10, seat + 3)], 1.4)], mix(SLED, BELT, 0.25))], outline=True, grow=0)
        return [runner, body]

    bottom = seat + 6 + 4 * st["bry"] / 20

    def scarf():
        _, hy = _layout(st, bottom)
        wave = [0, 3][i]
        pts = [(54, hy + st["hr"] * 0.82), (42, hy + st["hr"] * 0.7 - wave), (32, hy + st["hr"] * 0.85 + wave), (24, hy + st["hr"] * 0.7)]
        return dp.Part([strand(pts, 8, 6)], SCARF, [([L([(p[0], p[1] - 3), (p[0], p[1] + 3)], 1.6) for p in pts[1:3]], SCARF_STRIPE)],
                       outline=True, grow=0)

    puffs = snow_puffs([[(14, base - 4, 4.2), (6, base - 10, 3.0), (20, base - 12, 2.4)],
                        [(10, base - 6, 3.6), (18, base - 3, 2.8), (5, base - 14, 2.6)]][i])
    return santa(sp, side=True, bottom=bottom, cx=60, feet=None, sack=False, eyes="happy", mouth="open",
                 pom=[-26, -18][i], rot=[-2, 0][i], arms=[(1, 70)], back=[scarf], props=[sled],
                 fx=puffs + speed_fx(2, (base - 30, base - 22), 10, [0, -3][i]))


def sa_idle(sp, i):
    return santa(sp, side=True, sq=(1.0, 1.0 + 0.02 * i), pom=[0, 4][i])


def sa_look(sp, i):
    return santa(sp, side=True, head_rot=[-8, 0, 8, 0][i], pom=[-6, 0, 6, 0][i], eyes="closed" if i == 3 else "dot")


def sa_sleep(sp, i):
    """앉아서 꾸벅꾸벅 (모자 끝이 앞으로 늘어짐)."""
    zs = [z_fx(104, 50, 3.4), z_fx(115, 39, 2.6)] if i == 0 else [z_fx(106, 47, 3.6), z_fx(118, 35, 2.8)]
    return santa(sp, side=True, sq=(1.06, 0.92 + 0.012 * i), eyes="closed", head_dy=3, head_rot=8, feet=((2, 0), (-2, 0)),
                 arms=[(1, 8)], pom=-20, fx=zs)


def sa_annoyed(sp, i):
    return santa(sp, side=True, sq=(1.04, 0.95), eyes="squint", mouth="pout", arms=[(1, 15)], fx=[anger_fx(106, 34 - i, 7.0)])


def sa_hop(sp, i):
    if i == 0:
        return santa(sp, side=True, sq=(1.1, 0.86), eyes="tight", pom=10, arms=[(1, 15)])
    if i == 1:
        return santa(sp, side=True, bottom=SA_FLOOR - 16, sq=(0.94, 1.08), rot=-6, eyes="happy", mouth="open", pom=-16,
                     feet=((-3, 3), (3, 1)), arms=[(1, 130)])
    return santa(sp, side=True, bottom=SA_FLOOR - 9, rot=3, eyes="happy", pom=8, feet=((-2, 1), (2, 0)), arms=[(1, 70)])


def sa_fall(sp, i):
    stars = _stars([(100, 18, 3.6), (26, 26, 3.0)] if i == 0 else [(30, 16, 3.6), (104, 28, 3.0)])
    return santa(sp, side=True, bottom=SA_FLOOR - 10, rot=[-12, 12][i], eyes="spiral", mouth="wavy", pom=[-20, 20][i],
                 feet=((-4, 2), (4, -3)) if i == 0 else ((-4, -3), (4, 2)), arms=[(1, [150, 110][i])], fx=stars)


def sa_dragged(sp, i):
    s = [-1, 1][i]
    return santa(sp, side=True, bottom=SA_FLOOR - 8, rot=5 * s, eyes="tight", mouth="open", pom=-30,
                 feet=((-3 + 2 * s, 4), (3 + 2 * s, 5)), arms=[(1, 150)], fx=sweat_fx(104, 34))


def sa_land(sp, i):
    if i == 0:
        return santa(sp, side=True, sq=(1.12, 0.84), eyes="tight", pom=-14, arms=[(1, 80)])
    return santa(sp, side=True, sq=(1.02, 0.98), pom=6)


def sa_sit_front(sp, i):
    return santa(sp, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot", pom=[0, 3, 0, -3][i])


def sa_wave(sp, i):
    return santa(sp, arms=[(-1, 30), (1, [150, 175, 150, 175][i])], head_rot=[-3, -1, -3, -1][i], pom=[-4, 4, -4, 4][i])


def sa_tilt(sp, i):
    return santa(sp, head_rot=_swing([8, 12, 12][i]), pom=6, fx=[question_fx(18, 22 + [2, 0, -2][i])])


def sa_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 장화를 위로 버둥, 양팔 번쩍, 껄껄 ^o^."""
    wig = [0, 2, 0, -2][i]
    brx = _sa()["brx"]
    return santa(sp, sq=(1.08, 0.9), rot=-6, eyes="happy", mouth="open", feet=((-0.75 * brx, -22 + wig), (0.75 * brx, -24 - wig)),
                 arms=[(-1, 140 + 8 * wig), (1, 140 - 8 * wig)], pom=-16 + 3 * wig, head_dy=2)


def sa_stretch(sp, i):
    if i == 0:
        return santa(sp, sq=(0.95, 1.06), eyes="tight", arms=[(-1, 170), (1, 170)], pom=-6)
    if i == 1:
        return santa(sp, eyes="tight", mouth="yawn", arms=[(-1, 70), (1, 70)], head_rot=-3, pom=4)
    return santa(sp)


def sa_heart(sp, i):
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    st = _sa()
    cy_b, _ = _layout(st, SA_FLOOR - 1)
    hy = cy_b - st["bry"] * 0.25
    return santa(sp, eyes="happy", top=_heart_items(64, hy, 0.9), arms=[(-1, (55, hy + 6)), (1, (73, hy + 6))], fx=hs)


def sa_tickle(sp, i):
    return santa(sp, rot=_swing([-7, 7, 0][i]), eyes="happy", mouth="open", arms=[(-1, [60, 110, 85][i]), (1, [110, 60, 85][i])],
                 pom=[-10, 10, 0][i])


def sa_hungry(sp, i):
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    st = _sa()
    cy_b = SA_FLOOR - 6 - st["bry"]
    return santa(sp, rot=-3, eyes="closed", mouth="frown", brow="sad", pom=-10, head_rot=-4,
                 arms=[(-1, 20), (1, (68 + dx, cy_b + st["bry"] * 0.3 + dy))], fx=milk_bubble_fx(106, 16))


def sa_eat(sp, i):
    """냠냠 (그릇 없음): 입이 그릇 밥 더미 바로 위 (프레임 (64, 84))에 오도록 고개를 숙이고, 앙 ↔ 오물오물."""
    bite = i % 2 == 0
    st = _sa()
    _, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    _, hy0 = _layout(st, SA_FLOOR - 1)
    head_dy = max(-6.0, min(18.0, my - (hy0 + mouth_dy(st))))
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    paws = [dp._frame_to_pre(42, 98), dp._frame_to_pre(86, 98)]
    return santa(sp, head_dy=head_dy, eyes="happy" if bite else "closed", mouth="open" if bite else "chew", head_rot=[0, -2, 0, 2][i],
                 pom=[6, -6, 6, -6][i], arms=[(-1, paws[0]), (1, paws[1])], fx=fx)


def sa_level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    up = [150, 165, 175][i]
    return santa(sp, bottom=SA_FLOOR - 1 - [0, 1, 3][i], eyes="happy", mouth="big", arms=[(-1, up), (1, up)], pom=[-8, 8, -4][i],
                 feet=((0, 0), (0, 0)) if i < 2 else ((-1, 1), (1, 1)), fx=stars)


def sa_lookup(sp, i):
    """고개를 젖혀 위를 본다 (모자 끝은 뒤로 늘어짐)."""
    return santa(sp, side=True, head_rot=dp.lookup_tilt(i), pom=-6, eyes="up", fx=dp.lookup_fx(i, 112, 14))


SANTA_POSES = {"walk": sa_walk, "run": sa_run, "roll": sa_roll, "idle": sa_idle, "look": sa_look, "sleep": sa_sleep, "hop": sa_hop,
               "fall": sa_fall, "dragged": sa_dragged, "land": sa_land, "sit_front": sa_sit_front, "wave": sa_wave, "tilt": sa_tilt,
               "belly": sa_belly, "stretch": sa_stretch, "heart": sa_heart, "tickle": sa_tickle, "annoyed": sa_annoyed,
               "hungry": sa_hungry, "eat": sa_eat, "level_up": sa_level_up, "lookup": sa_lookup}
SANTA_POSES = {k: dh._fit(v) for k, v in SANTA_POSES.items()}

SANTA = Species(
    id="santa", name="산타", walk_speed=70, scale=0.94, base=SUIT, back=SUIT, belly=FUR, nose=SKIN, mouth="w",
    front_tail_color=SUIT, tail_color=SUIT, stage_size=dict.fromkeys(STAGES, 1.65), stripe=SUIT_SHADE, outline=SUIT_LINE,
    # 작은 점 눈 (반짝이 큰 눈·반쯤 감은 눈 없이) — 아기만 조금 크게
    stage_tweak={"baby": dict(pale=0.0, sparkle=False, eye=1.0, fit=0.9), "kinder": dict(pale=0.0, sparkle=False, eye=0.95, fit=0.93),
                 "child": dict(smile=False, eye=0.92, fit=0.95), "teenager": dict(eye=0.9, fit=0.97),
                 "adult": dict(calm=False, eye=0.88, fit=0.93)},
    **{k: v for k, v in dh._COMMON.items() if k != "poses"}, poses=SANTA_POSES,
)


def register():
    dp.SPECIES[RUDOLPH.id] = RUDOLPH
    dp.SPECIES[SANTA.id] = SANTA


register()


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    for sp in (RUDOLPH, SANTA):
        by_stage = dp.write_species(sp, root / "ui" / "pets" / sp.id)
        dh.preview_species(by_stage, pv / f"stages_{sp.id}.png")
    print("ok: rudolph, santa")


if __name__ == "__main__":
    main()
