"""농장 친구 🦆 — 뒤뚱뒤뚱 오리(duck). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_farm.py → ui/pets/duck/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_duck.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

오리: 동글 몸 + 큰 동글 머리 + 주황 납작 부리 + 물갈퀴 발 + 작은 날개(팔 대신 파닥). 걸음 = 좌우로 기우뚱 뒤뚱.
      단계: 아기 = 노란 솜털 병아리 오리 (정수리 솜털 세 가닥, 아주 짧은 날개) → 유아 노랑, 솜털 하나 → 어린이 연노랑, 머리 꼬불 깃
            → 청소년 하얀 오리, 꽁지 꼬불 깃 → 어른 하얀 오리 + 하늘색 세일러 스카프.
      (기린은 v0.7.0에서 빠짐)
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 입이 그릇 밥 더미 바로 위 (프레임 (64, 84)).
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, L, MOUTH_IN, T, TONGUE, WHITE, Part, Species, _ST, anger_fx,  # noqa: E402
                       arc, blush, brows, mix, question_fx, stage_eye, strand, sweat_fx, view,
                       xf, z_fx)
from draw_pets import _bubble, _fit, _stars, speed_fx  # noqa: E402
from sprite_spec import ROLL_CENTER, STAGES  # noqa: E402

FLOOR = 125.0

# =====================================================================
# 오리 🦆
# =====================================================================
DUCK_W, DUCK_Y, DUCK_LEMON = (255, 253, 247), (255, 226, 112), (255, 240, 176)
DUCK_LINE = (150, 112, 84)
BILL, BILL_DARK, BILL_LIGHT = (255, 160, 64), (226, 120, 46), (255, 200, 130)
SCARF, SCARF_DARK, SCARF_LIGHT = (126, 186, 236), (92, 150, 210), (210, 234, 252)
# 단계(lvl)별 — col: 몸 색 · fluff: 정수리 솜털 가닥 · curl: 머리(child)·꽁지(teen+) 꼬불 깃 · wing: 날개 크기 · scarf
DK_STAGE = {
    0: dict(col=DUCK_Y, fluff=3, curl=None, wing=0.72, scarf=False),
    1: dict(col=DUCK_Y, fluff=1, curl=None, wing=0.85, scarf=False),
    2: dict(col=DUCK_LEMON, fluff=0, curl="head", wing=0.95, scarf=False),
    3: dict(col=DUCK_W, fluff=0, curl="tail", wing=1.0, scarf=False),
    4: dict(col=DUCK_W, fluff=0, curl="tail", wing=1.05, scarf=True),
}
BREAD, BREAD_CRUST = (250, 214, 150), (214, 150, 84)


def _dk():
    return DK_STAGE[_ST["lvl"]]


def _wing_col(col):
    return mix(col, DUCK_LINE, 0.1)


def duck_bill(cx, cy, kind, side, k=1.0):
    """부리 Part들 + 입 ops. side: 옆모습 (cx, cy = 부리 뿌리 가운데) · 앞모습 (cx, cy = 부리 가운데)."""
    parts, ops = [], []
    gap = {"open": 4.0, "big": 6.5, "yawn": 7.5, "quack": 5.0}.get(kind, 0.0)
    if side:
        up = [E(cx - 2, cy - 5.5 * k, cx + 17 * k, cy + 2.5 * k)]
        parts.append(Part(up, BILL, [([E(cx + 2, cy - 4.5 * k, cx + 9 * k, cy - 2 * k)], BILL_LIGHT)], grow=0, outline=True))
        if gap:
            with xf(rot=gap * 3.2, pivot=(cx, cy)):
                lo = [E(cx - 1, cy - 1, cx + 13 * k, cy + 5 * k)]
                ops.append(([E(cx, cy - 1.5, cx + 11 * k, cy + 3 * k)], MOUTH_IN))
                parts.insert(0, Part(lo, BILL_DARK, grow=0, outline=True))
        else:
            ops.append(([L([(cx + 1, cy + 0.8), (cx + 13 * k, cy + 0.6)], 1.1)], BILL_DARK))
        ops.append(([C(cx + 6 * k, cy - 2.4 * k, 0.9)], BILL_DARK))  # 콧구멍
        return parts, ops
    w, h = 13 * k, 6.5 * k
    if gap:
        lo = [E(cx - w * 0.78, cy - 1 + gap * 0.4, cx + w * 0.78, cy + h + gap * 0.9)]
        parts.append(Part(lo, BILL_DARK, grow=0, outline=True))
        ops.append(([E(cx - w * 0.6, cy + 0.5, cx + w * 0.6, cy + h * 0.4 + gap)], MOUTH_IN))
        if kind in ("open", "big", "quack"):
            ops.append(([E(cx - w * 0.32, cy + gap * 0.5, cx + w * 0.32, cy + h * 0.3 + gap)], TONGUE))
        up = [E(cx - w, cy - h, cx + w, cy + h * 0.45)]
    else:
        up = [E(cx - w, cy - h, cx + w, cy + h)]
    parts.append(Part(up, BILL, [([E(cx - w * 0.55, cy - h * 0.8, cx - w * 0.05, cy - h * 0.3)], BILL_LIGHT)], grow=0, outline=True))
    if not gap:
        if kind == "frown":
            ops.append(([L(arc(cx, cy + h * 0.55, w * 0.42, 1.8, 200, 340, 8), 1.2)], BILL_DARK))
        elif kind == "chew":
            ops.append(([L([(cx - 6, cy + 1), (cx - 3, cy + 2.6), (cx, cy + 1), (cx + 3, cy + 2.6), (cx + 6, cy + 1)], 1.2)], BILL_DARK))
        elif kind == "pout":
            ops.append(([L(arc(cx + 1, cy + h * 0.6, 3.4, 1.6, 200, 340, 8), 1.2)], BILL_DARK))
        else:
            ops.append(([L(arc(cx, cy - 0.5, w * 0.62, 2.6, 20, 160, 10), 1.2)], BILL_DARK))
    ops.append(([C(cx - 3 * k, cy - h * 0.55, 0.8), C(cx + 3 * k, cy - h * 0.55, 0.8)], BILL_DARK))
    return parts, ops


def duck_foot(x, y, rot=0.0, side=True):
    """물갈퀴 발 하나 (발뒤꿈치 (x, y) 바닥) — 오리발 부채꼴."""
    with xf(rot=rot, pivot=(x, y)):
        if side:
            fan = [T((x - 3, y - 3.5), (x + 11, y - 1.5), (x + 11, y + 0.2), (x - 4, y + 0.2)), C(x - 2, y - 2, 2.6)]
        else:
            fan = [T((x - 6.5, y), (x, y - 6), (x + 6.5, y)), C(x - 5, y - 1.4, 1.9), C(x, y - 1.4, 1.9), C(x + 5, y - 1.4, 1.9)]
        toes = [L([(x + 1, y - 1.6), (x + 8, y - 0.9)], 0.8)] if side else [L([(x - 2.4, y - 1.2), (x - 1, y - 4)], 0.8),
                                                                          L([(x + 2.4, y - 1.2), (x + 1, y - 4)], 0.8)]
        return Part(fan, BILL, [(toes, BILL_DARK)], grow=0, outline=True)


def duck_wing(sh, ang, k, col, side):
    """날개 (어깨 sh에서 ang도 — 0 = 아래로 접힘, 90 = 옆으로 펼침). 끝에 깃털 세 갈래."""
    ln = 17 * k
    with xf(rot=-ang if side else ang * (1 if sh[0] > 64 else -1), pivot=sh):
        x, y = sh
        if side:
            body = [E(x - 15 * k, y - 6 * k, x + 4 * k, y + 9 * k), T((x - 15 * k, y), (x - 22 * k, y + 7 * k), (x - 10 * k, y + 8 * k))]
        else:
            s = 1 if x > 64 else -1
            body = [E(x - 5.5 * k, y - 3, x + 5.5 * k, y + ln), C(x + s * 1.5, y + ln - 1, 4.2 * k),
                    C(x - s * 2.6 * k, y + ln + 1.5 * k, 3.2 * k)]
        return Part(body, _wing_col(col), grow=0, outline=True)


def duck_fluff(cx, top, n):
    """정수리 솜털 (아기 세 가닥, 유아 한 가닥) · 어린이 꼬불 깃."""
    if n == 0:
        return []
    pts = [[(cx - 1, top + 4)] + arc(cx + 5, top - 5, 5.5, 6.5, 180, 300, 8)]
    if n >= 3:
        pts += [[(cx - 3, top + 4), (cx - 6, top - 3), (cx - 10, top - 5)], [(cx + 2, top + 4), (cx + 7, top - 2), (cx + 11, top - 1)]]
    return [strand(p, 4.4, 1.2) for p in pts]


def duck(sp, *, side=True, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, wings=None,
         feet=((0, 0), (0, 0)), head=(0.0, 0.0), head_rot=0.0, fx=(), held=(), props=(), ball=False, belly=False):
    """오리 한 장. side: 오른쪽을 보는 옆모습 · bottom: 몸 아래끝 (발 위) · wings: [(쪽, 각도)] (None = 접음) ·
    feet: (뒤/왼, 앞/오른) 발 (dx, dy) 또는 None · head: 머리 (dx, dy) · ball: 구르기 공 · belly: 발라당 (앞모습, 발이 위로)."""
    st = _dk()
    col = st["col"]
    hk = _ST["head"]
    bk = _ST["body"]
    # 서 있을 땐 몸을 조금 띄워 몸 아래로 다리가 보인다 (다리 윗끝은 몸 뒤)
    bottom = FLOOR - 8 if bottom is None else bottom
    parts, ops = [], []
    rx, ry = 29 * bk * _ST["torso"][0], 22 * bk * _ST["torso"][1]
    bcy = bottom - ry
    hr = (23.5 if side else 25.0) * hk
    if side:
        rx *= 0.9
    with xf(rot=rot, pivot=(cx, bottom)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
            if side:
                bx = cx - 4
                hx, hy = bx + 0.62 * rx + head[0], bcy - ry * 0.55 - hr * 0.7 + head[1]
                # 발 (몸 뒤)
                if feet and not ball:
                    for j, (fx_, fy_) in enumerate(feet):
                        x = bx - 9 + 16 * j + fx_
                        lift = min(0.0, fy_)
                        parts.append(Part([L([(x, bottom - 6), (x + fx_ * 0.2, FLOOR - 2 + fy_)], 3.2)], BILL, grow=0, outline=True))
                        parts.append(duck_foot(x, FLOOR - 0.5 + fy_, rot=-6 if lift < 0 else 0))
                tail = [T((bx - rx + 6, bcy - 4), (bx - rx - 9, bcy - ry * 0.75), (bx - rx + 2, bcy + 6))]
                spikes = []
                if st["curl"] == "tail":
                    tx, ty = bx - rx - 6, bcy - ry * 0.6
                    spikes.append(strand([(tx + 4, ty + 4)] + arc(tx - 1, ty - 4, 4, 4.5, 90, 330, 10), 3.6, 1.4))
                belly_m = [([E(bx - rx * 0.6, bcy + ry * 0.1, bx + rx * 0.9, bcy + ry * 1.3)], mix(col, WHITE, 0.45))]
                body = Part([E(bx - rx, bcy - ry, bx + rx, bcy + ry)] + tail, col, belly_m, grow=1, spikes=spikes)
                with xf(rot=head_rot, pivot=(hx - hr * 0.3, hy + hr * 0.8)):
                    neck = L([(bx + rx * 0.4, bcy - ry * 0.3), (hx - 2, hy + hr * 0.4)], hr * 0.9)
                    hspk = duck_fluff(hx - 2, hy - hr, st["fluff"])
                    if st["curl"] == "head":
                        hspk.append(strand([(hx - 3, hy - hr + 4), (hx - 2, hy - hr - 3)] + arc(hx + 2, hy - hr - 5, 4, 4, 180, 330, 8), 4.2, 1.6))
                    hpart = Part([C(hx, hy, hr), neck], col, grow=1, spikes=hspk)
                    dp.mark_head(hx - 2, hy - hr * 0.84, hr * 1.25)
                    # 몸은 다리 위에 (다리는 몸 뒤라 몸 아래로 발만 나온다)
                    parts.append(body)
                    parts.append(hpart)
                    bp, bo = duck_bill(hx + hr * 0.78, hy + hr * 0.18, mouth, True, k=hk ** 0.5)
                    ex, ey = hx + hr * 0.3, hy - hr * 0.16
                    face = list(stage_eye(eyes, ex, ey))
                    if brow:
                        face += brows(brow, ex, ey + _ST["eye_dy"], flip=True)
                    face += blush(hx + hr * 0.12, hy + hr * 0.42)
                    hops = face
                if st["scarf"]:
                    # 목에 두른다: 머리가 내려가거나(잘 때·엎드릴 때) 움직이면 그만큼 따라간다
                    sx_, sy_ = bx + rx * 0.45 + head[0] * 0.6, bcy - ry * 0.72 + head[1] * 0.75
                    parts.append(Part([L(arc(sx_ + 2, sy_ - 2, 10, 5, 30, 200, 10), 5.0),
                                       T((sx_ - 6, sy_ - 1), (sx_ - 14, sy_ + 9), (sx_ - 3, sy_ + 6))], SCARF,
                                      [([L([(sx_ - 11, sy_ + 6), (sx_ - 5, sy_ + 4)], 1.0)], SCARF_LIGHT)], grow=0, outline=True))
                for side_, a in (wings or [(1, 0)]):
                    parts.append(duck_wing((bx + rx * 0.2, bcy - ry * 0.2), a, st["wing"] * bk, col, True))
                with xf(rot=head_rot, pivot=(hx - hr * 0.3, hy + hr * 0.8)):
                    parts += bp
                ops += hops + bo
            else:
                fcx = cx
                hx, hy = fcx + head[0], bcy - ry * 0.6 - hr * 0.62 + head[1]
                late = []  # 발라당: 발은 머리·배 앞에 (위로 버둥)
                body_parts = [E(fcx - rx, bcy - ry, fcx + rx, bcy + ry)]
                belly_m = [([E(fcx - rx * 0.62, bcy - ry * 0.2, fcx + rx * 0.62, bcy + ry * 1.1)], mix(col, WHITE, 0.45))]
                body = Part(body_parts, col, belly_m, grow=1)
                if feet and not ball:
                    for j, (fx_, fy_) in enumerate(feet):
                        s = -1 if j == 0 else 1
                        x = fcx + s * 11 + fx_
                        if belly:
                            x, y = fcx + s * 12 + fx_, bcy + ry * 0.05 + fy_
                            late.append(Part([L([(fcx + s * 9, bcy + ry * 0.75), (x, y + 2)], 3.2)], BILL, grow=0, outline=True))
                            late.append(duck_foot(x, y, rot=180 + s * 25, side=False))
                        else:
                            parts.append(Part([L([(x, bottom - 5), (x, FLOOR - 2 + fy_)], 3.2)], BILL, grow=0, outline=True))
                            parts.append(duck_foot(x, FLOOR - 0.5 + fy_, rot=s * 8, side=False))
                # 몸은 다리 위에 (발라당 다리는 late로 몸 앞에)
                parts.append(body)
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    hspk = duck_fluff(hx - 1, hy - hr, st["fluff"])
                    if st["curl"] == "head":
                        hspk.append(strand([(hx - 1, hy - hr + 4), (hx, hy - hr - 3)] + arc(hx + 4, hy - hr - 5, 4, 4, 180, 330, 8), 4.2, 1.6))
                    parts.append(Part([C(hx, hy, hr)], col, grow=1, spikes=hspk))
                    dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
                    if st["scarf"]:
                        ky = hy + hr * 0.9
                        parts.append(Part([L(arc(hx, ky - 6, 15, 6, 25, 155, 12), 5.0),
                                           T((hx - 2, ky), (hx - 10, ky + 11), (hx + 1, ky + 6)), T((hx + 2, ky), (hx + 10, ky + 11), (hx - 1, ky + 6)),
                                           C(hx, ky, 3.2)], SCARF, [([C(hx - 1, ky - 1, 1.2)], SCARF_LIGHT)], grow=0, outline=True))
                    bp, bo = duck_bill(hx, hy + hr * 0.3, mouth, False, k=hk ** 0.5)
                    face = []
                    for ex, flip in ((hx - hr * 0.42, False), (hx + hr * 0.42, True)):
                        face += stage_eye(eyes, ex, hy - hr * 0.12, flip)
                        if brow:
                            face += brows(brow, ex, hy - hr * 0.12 + _ST["eye_dy"], flip=flip)
                    face += blush(hx - hr * 0.68, hy + hr * 0.36) + blush(hx + hr * 0.68, hy + hr * 0.36)
                parts += late
                if not ball:
                    for s, a in (wings or [(-1, 10), (1, 10)]):
                        parts.append(duck_wing((fcx + s * rx * 0.86, bcy - ry * 0.35), a, st["wing"] * bk, col, False))
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    parts += bp
                ops += face + bo
            parts += [h() for h in held]
    parts += [p() for p in props]
    return dp.compose(parts, ops, list(fx))


def bread_bubble_fx(cx, cy):
    """생각 풍선 안에 식빵 한 조각."""
    fx = _bubble(cx, cy)
    top = arc(cx, cy - 2.5, 7.2, 4.6, 180, 360, 10)
    slab = top + [(cx + 6.2, cy + 6.5), (cx - 6.2, cy + 6.5)]
    fx += [([T(*slab), L(slab + [slab[0]], 2.4)], BREAD_CRUST, 0), ([T(*[(x * 0.8 + cx * 0.2, y * 0.8 + (cy + 1) * 0.2) for x, y in slab])], BREAD, 0)]
    return fx


# =====================================================================
# 포즈 — 오리
# =====================================================================
def dk_walk(sp, i):
    """뒤뚱뒤뚱: 몸이 좌우(앞뒤)로 기우뚱, 발이 번갈아."""
    t = [-7, 0, 7, 0][i]
    up = [0, 1.5, 0, 1.5][i]
    feet = [((-3, -3), (3, 0)), ((0, -1), (0, 0)), ((3, 0), (-3, -3)), ((0, 0), (0, -1))][i]
    return duck(sp, bottom=FLOOR - 8 - up, rot=t, head_rot=-t * 0.4, feet=feet, wings=[(1, [10, 20, 10, 20][i])])


def dk_run(sp, i):
    up = [0, 3, 5, 2][i]
    feet = [((-5, -4), (5, 0)), ((-2, -2), (2, -1)), ((5, 0), (-5, -4)), ((2, -1), (-2, -2))][i]
    return duck(sp, bottom=FLOOR - 8 - up, rot=[-10, -4, -10, -4][i] - 4, eyes="happy", mouth="open", feet=feet,
                wings=[(1, [70, 110, 70, 110][i])], fx=speed_fx(2, (70, 88, 104), 12, [0, -3, 0, -3][i]))


def dk_roll(sp, i):
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        k = 33 / 29
        with xf(sx=k * 0.9, sy=k * 0.9, pivot=(cx, cy + 33)):
            return duck(sp, side=False, bottom=cy + 33, cx=cx, eyes="happy", mouth="open" if i else None, ball=True,
                        sq=(1.0, 0.92), head=(0, 18), feet=None)


def dk_idle(sp, i):
    return duck(sp, sq=(1.0, 1.0 + 0.02 * i), head=(0, -0.8 * i))


def dk_look(sp, i):
    return duck(sp, head_rot=[-10, 0, 12, 0][i], eyes="closed" if i == 3 else "dot")


def dk_lookup(sp, i):
    """고개를 젖혀 부리를 들고 위를 본다."""
    return duck(sp, head=(-1, -1), head_rot=dp.lookup_tilt(i), eyes="up", fx=dp.lookup_fx(i, 112, 14))


def dk_sleep(sp, i):
    zs = [z_fx(104, 58, 3.4), z_fx(115, 47, 2.6)] if i == 0 else [z_fx(106, 55, 3.6), z_fx(118, 43, 2.8)]
    return duck(sp, bottom=FLOOR - 0.5, sq=(1.08, 0.9 + 0.012 * i), eyes="closed", head=(-4, 12), head_rot=14, feet=None, fx=zs)


def dk_annoyed(sp, i):
    return duck(sp, bottom=FLOOR - 1, sq=(1.04, 0.94), eyes="squint", mouth="pout", feet=None, head=(0, 4),
                fx=[anger_fx(106, 34 - i, 7.0)])


def dk_hop(sp, i):
    if i == 0:
        return duck(sp, sq=(1.1, 0.86), eyes="tight", feet=((-2, 0), (2, 0)), head=(0, 4))
    if i == 1:
        return duck(sp, bottom=FLOOR - 18, sq=(0.93, 1.08), rot=-6, eyes="happy", mouth="open", feet=((-3, -14), (3, -15)),
                    wings=[(1, 120)])
    return duck(sp, bottom=FLOOR - 10, eyes="happy", feet=((-2, -8), (2, -8)), wings=[(1, 80)])


def dk_fall(sp, i):
    stars = _stars([(100, 22, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 20, 3.6), (104, 32, 3.0)])
    return duck(sp, bottom=FLOOR - 12, rot=[-12, 12][i], eyes="spiral", mouth="open", feet=((-4, -8), (4, -6)),
                wings=[(1, [150, 110][i])], fx=stars)


def dk_dragged(sp, i):
    s = [-1, 1][i]
    return duck(sp, bottom=FLOOR - 12, rot=5 * s, eyes="tight", mouth="open", feet=((-3 + 2 * s, -4), (3 + 2 * s, -3)),
                wings=[(1, 150)], fx=sweat_fx(106, 30))


def dk_land(sp, i):
    if i == 0:
        return duck(sp, sq=(1.12, 0.84), eyes="tight", wings=[(1, 80)])
    return duck(sp, sq=(1.02, 0.98), wings=[(1, 30)])


def dk_sit_front(sp, i):
    return duck(sp, side=False, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot")


def dk_wave(sp, i):
    return duck(sp, side=False, wings=[(-1, 10), (1, [140, 170, 140, 170][i])], rot=[-3, -1, -3, -1][i])


def dk_tilt(sp, i):
    return duck(sp, side=False, head_rot=dp._swing([8, 12, 12][i]), fx=[question_fx(18, 22 + [2, 0, -2][i])])


def dk_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return duck(sp, side=False, belly=True, bottom=FLOOR - 1, sq=(1.12, 0.8), rot=-6, eyes="happy", mouth="open",
                feet=((-2 + wig, -2), (2 - wig, -2 + wig)), wings=[(-1, 120 + 6 * wig), (1, 120 - 6 * wig)])


def dk_stretch(sp, i):
    if i == 0:
        return duck(sp, side=False, sq=(0.94, 1.08), eyes="tight", wings=[(-1, 170), (1, 170)], head=(0, -3))
    if i == 1:
        return duck(sp, side=False, eyes="tight", mouth="yawn", wings=[(-1, 100), (1, 100)], rot=-3)
    return duck(sp, side=False)


def dk_heart(sp, i):
    hs = dp.heart_burst(i)
    hy = FLOOR - 4 - 22 * _ST["body"] * 0.5
    held = [lambda: dp.held_heart(hy)]
    return duck(sp, side=False, eyes="happy", wings=[(-1, 60), (1, 60)], held=held, fx=hs)


def dk_tickle(sp, i):
    return duck(sp, side=False, rot=dp._swing([-7, 7, 0][i]), eyes="happy", mouth="quack" if i < 2 else "open",
                wings=[(-1, [60, 120, 90][i]), (1, [120, 60, 90][i])])


def dk_hungry(sp, i):
    return duck(sp, side=False, rot=-4, eyes="closed", mouth="frown", brow="sad", head=(0, [0, 1, 0][i] + 2),
                wings=[(-1, 15), (1, [30, 40, 30][i])], fx=bread_bubble_fx(106, 18))


def dk_eat(sp, i):
    """냠냠 (그릇 없음): 부리가 그릇 밥 더미 바로 위 (프레임 (64, 84))로 오도록 머리를 숙여 콕콕."""
    bite = i % 2 == 0
    mx, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    st_b = FLOOR - 4
    ry = 22 * _ST["body"] * _ST["torso"][1]
    hr = 25.0 * _ST["head"]  # duck() 앞모습 머리 반지름
    bcy = st_b - ry
    hy0 = bcy - ry * 0.6 - hr * 0.62
    want = my - hr * 0.3 - 6.5 * _ST["head"] ** 0.5 * 0.5
    fx = dp.eat_crumbs(i)
    return duck(sp, side=False, bottom=st_b, head=(0, want - hy0), eyes="happy" if bite else "closed", mouth="open" if bite else "chew",
                wings=[(-1, 40), (1, 40)], feet=None, fx=fx)


def dk_level_up(sp, i):
    stars = dp.level_up_stars(i)
    up = [150, 165, 175][i]
    return duck(sp, side=False, bottom=FLOOR - 8 - [0, 1, 3][i], eyes="happy", mouth="big", wings=[(-1, up), (1, up)], fx=stars)


DUCK_POSES = {"walk": dk_walk, "run": dk_run, "roll": dk_roll, "idle": dk_idle, "look": dk_look, "sleep": dk_sleep, "hop": dk_hop,
              "fall": dk_fall, "dragged": dk_dragged, "land": dk_land, "sit_front": dk_sit_front, "wave": dk_wave, "tilt": dk_tilt,
              "belly": dk_belly, "stretch": dk_stretch, "heart": dk_heart, "tickle": dk_tickle, "annoyed": dk_annoyed,
              "hungry": dk_hungry, "eat": dk_eat, "level_up": dk_level_up, "lookup": dk_lookup}
DUCK_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in DUCK_POSES.items()}


DUCK = Species(
    id="duck", name="오리", walk_speed=80, scale=0.94, base=DUCK_W, back=DUCK_W, belly=DUCK_W, nose=BILL, mouth="w",
    front_tail_color=DUCK_W, tail_color=DUCK_W, stage_size=dict.fromkeys(STAGES, 0.9), stripe=BILL, outline=DUCK_LINE,
    stage_tweak={"baby": dict(pale=0.0, eye=1.2, fit=0.9), "kinder": dict(pale=0.0, eye=1.12, fit=0.93),
                 "child": dict(pale=0.0, eye=1.05), "adult": dict(fit=0.94)},
    specials=["lookup"], poses=DUCK_POSES, **dp.NO_COMMON_PARTS,
)

def register():
    dp.SPECIES[DUCK.id] = DUCK


register()


def _in_view(sp):
    """draw_pets.frames와 같은 view (그림 좌표를 시트와 같게)."""
    return view(scale=sp.scale * _ST["fit"], lift=dp.LIFT + 1.7 * dp.OUTLINE * (_ST["line"] - 1))


def main():
    import draw_halloween as dh
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    out = root / "ui" / "pets" / DUCK.id
    by_stage = dp.at_frame(dp.write_species(DUCK, out))
    dh.preview_species(by_stage, pv / f"stages_{DUCK.id}.png")
    print("ok: duck")


if __name__ == "__main__":
    main()
