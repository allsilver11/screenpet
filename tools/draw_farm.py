"""농장 친구들 🦆🦒 — 뒤뚱뒤뚱 오리(duck)와 목을 쭉 늘이는 기린(giraffe). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_farm.py → ui/pets/{duck,giraffe}/{sheet_<stage>.png ×5, sheet.png, pet.json}
                                + ui/pets/giraffe/{neck_<stage>.png, neck_tile_<stage>.png} (목 늘이기 층)
                                + tools/preview/{stages_duck,stages_giraffe,neck_giraffe}.png
    (python tools/draw_pets.py도 둘을 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

오리: 동글 몸 + 큰 동글 머리 + 주황 납작 부리 + 물갈퀴 발 + 작은 날개(팔 대신 파닥). 걸음 = 좌우로 기우뚱 뒤뚱.
      단계: 아기 = 노란 솜털 병아리 오리 (정수리 솜털 세 가닥, 아주 짧은 날개) → 유아 노랑, 솜털 하나 → 어린이 연노랑, 머리 꼬불 깃
            → 청소년 하얀 오리, 꽁지 꼬불 깃 → 어른 하얀 오리 + 하늘색 세일러 스카프.
기린: 꼬마 기린 — 큰 머리, 적당히 긴 목, 작은 몸, 갈색 무늬, 뿔(오시콘) 끝 동글 혹, 크림 주둥이, 꼬리 끝 술.
      단계: 아기 = 짧은 목 + 털 뭉치 뿔 + 큰 무늬 몇 개 → 유아 목이 길어짐 → 어린이 뿔에 혹 + 갈기 → 청소년 무늬 가득
            → 어른 긴 속눈썹 + 작은 데이지 한 송이.
      구르지 않는다 (pet.json neckstretch → 코어가 구르기 대신 Special::NeckStretch): 멈춰 서서 목을 평소의 두 배로 쭉.
      늘어나는 목은 앱이 층으로 그린다 — neck_<stage>.png (가로 4칸: 몸 · 머리 "어?" · 머리 깜빡 · 머리 ✧ 흐뭇) 와
      neck_tile_<stage>.png (위아래로 이어지는 목 한 토막, 무늬 주기 = 높이). 자르는 높이 cut·늘이는 길이 ext는 pet.json neck_stretch.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 입이 그릇 밥 더미 바로 위 (프레임 (64, 84)).
"""

import json
import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (BLUSH, C, E, EYE, HEART, L, LINE, MOUTH_IN, SPEED, T, TONGUE, WHITE, Part, Species, _ST, anger_fx,  # noqa: E402
                       arc, blush, brows, heart_fx, mix, open_mouth, question_fx, sparkle_fx, stage_eye, strand, sweat_fx, view,
                       xf, z_fx)
from draw_halloween import _bubble, _fit, _stars, speed_fx  # noqa: E402
from sprite_spec import FRAME, ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

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
    0: dict(col=DUCK_Y, fluff=3, curl=None, wing=0.72, scarf=False, r=1.0),
    1: dict(col=DUCK_Y, fluff=1, curl=None, wing=0.85, scarf=False, r=1.0),
    2: dict(col=DUCK_LEMON, fluff=0, curl="head", wing=0.95, scarf=False, r=1.0),
    3: dict(col=DUCK_W, fluff=0, curl="tail", wing=1.0, scarf=False, r=1.0),
    4: dict(col=DUCK_W, fluff=0, curl="tail", wing=1.05, scarf=True, r=1.0),
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


def duck_fluff(cx, top, n, col):
    """정수리 솜털 (아기 세 가닥, 유아 한 가닥) · 어린이 꼬불 깃."""
    if n == 0:
        return []
    pts = [[(cx - 1, top + 4)] + arc(cx + 5, top - 5, 5.5, 6.5, 180, 300, 8)]
    if n >= 3:
        pts += [[(cx - 3, top + 4), (cx - 6, top - 3), (cx - 10, top - 5)], [(cx + 2, top + 4), (cx + 7, top - 2), (cx + 11, top - 1)]]
    return [strand(p, 4.4, 1.2) for p in pts]


def duck(sp, *, side=True, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, wings=None,
         feet=((0, 0), (0, 0)), head=(0.0, 0.0), head_rot=0.0, fx=(), held=(), props=(), ball=False, belly=False, tilt=0.0):
    """오리 한 장. side: 오른쪽을 보는 옆모습 · bottom: 몸 아래끝 (발 위) · wings: [(쪽, 각도)] (None = 접음) ·
    feet: (뒤/왼, 앞/오른) 발 (dx, dy) 또는 None · head: 머리 (dx, dy) · ball: 구르기 공 · belly: 발라당 (앞모습, 발이 위로)."""
    st = _dk()
    col = st["col"]
    hk = _ST["head"]
    bk = _ST["body"]
    bottom = FLOOR - 4 if bottom is None else bottom
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
                    hspk = duck_fluff(hx - 2, hy - hr, st["fluff"], col)
                    if st["curl"] == "head":
                        hspk.append(strand([(hx - 3, hy - hr + 4), (hx - 2, hy - hr - 3)] + arc(hx + 2, hy - hr - 5, 4, 4, 180, 330, 8), 4.2, 1.6))
                    hpart = Part([C(hx, hy, hr), neck], col, grow=1, spikes=hspk)
                    parts.insert(0, body) if not parts else parts.insert(0, body)
                    parts.append(hpart)
                    bp, bo = duck_bill(hx + hr * 0.78, hy + hr * 0.18, "quack" if mouth == "quack" else mouth, True, k=hk ** 0.5)
                    ex, ey = hx + hr * 0.3, hy - hr * 0.16
                    face = list(stage_eye(eyes, ex, ey))
                    if brow:
                        face += brows(brow, ex, ey + _ST["eye_dy"], flip=True)
                    face += blush(hx + hr * 0.12, hy + hr * 0.42)
                    hops = face
                if st["scarf"]:
                    sx_, sy_ = bx + rx * 0.45, bcy - ry * 0.72
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
                parts.append(Part(body_parts, col, belly_m, grow=1))
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
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    hspk = duck_fluff(hx - 1, hy - hr, st["fluff"], col)
                    if st["curl"] == "head":
                        hspk.append(strand([(hx - 1, hy - hr + 4), (hx, hy - hr - 3)] + arc(hx + 4, hy - hr - 5, 4, 4, 180, 330, 8), 4.2, 1.6))
                    parts.append(Part([C(hx, hy, hr)], col, grow=1, spikes=hspk))
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
# 기린 🦒
# =====================================================================
GIR, GIR_SPOT, GIR_CREAM, GIR_LINE = (255, 214, 120), (222, 146, 76), (255, 242, 216), (156, 104, 66)
HOOF, OSSI, MANE = (128, 90, 70), (176, 112, 70), (214, 134, 70)
DAISY, DAISY_C = (255, 255, 255), (255, 206, 84)
# 단계(lvl)별 — neck: 목 길이 배율 · ossi: 뿔 (0 털 뭉치 · 1 짧은 뿔 · 2 혹) · spots: 무늬 개수 단계 · mane · lash · daisy
GF_STAGE = {
    0: dict(neck=0.55, ossi=0, spots=0, mane=False, lash=False, daisy=False),
    1: dict(neck=0.7, ossi=1, spots=1, mane=False, lash=False, daisy=False),
    2: dict(neck=0.85, ossi=2, spots=2, mane=True, lash=False, daisy=False),
    3: dict(neck=1.0, ossi=2, spots=3, mane=True, lash=False, daisy=False),
    4: dict(neck=1.08, ossi=2, spots=4, mane=True, lash=True, daisy=True),
}
NECK_LEN = 36.0  # 청소년 목 길이 (어깨 → 머리 밑)
NECK_W = 13.0
LEAF_G, LEAF_V = (140, 204, 112), (100, 164, 84)


def _gf():
    return GF_STAGE[_ST["lvl"]]


def _spot(x, y, r, seed):
    """둥글게 일그러진 무늬 한 점."""
    pts = []
    for k in range(9):
        a = math.radians(k * 40 + seed * 17)
        rr = r * (0.82 + 0.22 * math.sin(seed * 3.1 + k * 1.7))
        pts.append((x + rr * math.cos(a), y + rr * 0.86 * math.sin(a)))
    return T(*pts)


# 몸 무늬 (몸 중심 기준 dx, dy, r, 나타나는 단계)
BODY_SPOTS = [(-12, -4, 6.5, 0), (8, -6, 6.0, 0), (-2, 8, 5.0, 1), (20, 4, 4.6, 2), (-22, 6, 4.4, 2), (-10, -13, 3.8, 3),
              (14, -14, 3.6, 3), (2, -1, 3.2, 4)]


def neck_spot_ys(y_top, y_bot, period, phase):
    """목 무늬 y들: phase에서 주기 period로 (위아래로 이어지게)."""
    k0 = math.floor((y_top - phase) / period) - 1
    out = []
    k = k0
    while True:
        y = phase + k * period
        if y > y_bot + period:
            break
        out.append((y, k))
        k += 1
    return out


def neck_marks(a, b, w, period=None, phase=0.0):
    """목 (a: 어깨 → b: 머리 밑) 위 무늬·갈기 marks. period: 세로 목일 때 주기 (목 늘이기 층과 이어지게)."""
    st = _gf()
    marks = []
    ln = math.hypot(b[0] - a[0], b[1] - a[1])
    ux, uy = (b[0] - a[0]) / ln, (b[1] - a[1]) / ln
    nx, ny = -uy, ux  # 오른쪽(앞) 법선
    spots = []
    if st["spots"] >= 1:
        if period:
            for y, k in neck_spot_ys(min(a[1], b[1]) - 20, max(a[1], b[1]) + 20, period, phase):
                s = 1 if k % 2 == 0 else -1
                spots.append(_spot(a[0] + s * w * 0.22, y, w * 0.26, k % 3))
        else:
            n = max(2, round(ln / 11))
            for j in range(n):
                t = (j + 0.6) / n
                s = 1 if j % 2 == 0 else -1
                spots.append(_spot(a[0] + ux * ln * t + nx * s * w * 0.22, a[1] + uy * ln * t + ny * s * w * 0.22, w * 0.26, j))
    if spots:
        marks.append((spots, GIR_SPOT))
    if st["mane"]:
        off = -w * 0.5
        marks.append(([L([(a[0] + nx * off - ux * 20, a[1] + ny * off - uy * 20), (b[0] + nx * off + ux * 20, b[1] + ny * off + uy * 20)], 3.4)], MANE))
    return marks


def gf_head(sp, hx, hy, *, side, eyes, mouth, brow, rot=0.0, munch=False):
    """기린 머리 Part들과 얼굴 ops (hx, hy = 머리 중심). 옆모습은 오른쪽을 본다."""
    st = _gf()
    hk = _ST["head"]
    parts, ops = [], []
    with xf(rot=rot, pivot=(hx, hy + 8)):
        with xf(sx=hk, sy=hk, pivot=(hx, hy)):
            # 뿔 (오시콘)
            horn = []
            hl = [6.0, 8.5, 10.0][st["ossi"]]
            bases = [(hx - 6, hy - 12), (hx + 3, hy - 13)] if side else [(hx - 8, hy - 13), (hx + 8, hy - 13)]
            tips = [(bx + (-2 if (side or bx < hx) else 2), by - hl) for bx, by in bases]
            for (bx, by), (tx, ty) in zip(bases, tips):
                horn.append(L([(bx, by), (tx, ty)], 4.6))
            knob = [C(tx, ty, 3.6 if st["ossi"] == 2 else 3.0) for tx, ty in tips]
            if st["ossi"] == 0:
                parts.append(Part(horn + knob, GIR, [(knob, OSSI)], grow=0, outline=True))
            else:
                parts.append(Part(horn + knob, GIR, [(knob, OSSI)], grow=0, outline=True))
            # 귀
            if side:
                with xf(rot=-24, pivot=(hx - 10, hy - 6)):
                    ear = [E(hx - 25, hy - 10, hx - 8, hy - 3)]
                    parts.append(Part(ear, GIR, [([E(hx - 22, hy - 8.5, hx - 12, hy - 5)], mix(GIR, BLUSH, 0.6))], grow=0, outline=True))
            else:
                for s in (-1, 1):
                    with xf(rot=s * -22, pivot=(hx + s * 14, hy - 7)):
                        ear = [E(hx + s * 14 - 9 + s * 7, hy - 10, hx + s * 14 + 9 + s * 7, hy - 3)]
                        parts.append(Part(ear, GIR, [([E(hx + s * 21 - 5, hy - 8.3, hx + s * 21 + 5, hy - 4.6)], mix(GIR, BLUSH, 0.6))],
                                          grow=0, outline=True))
            # 머리 + 주둥이
            if side:
                skull = [E(hx - 16, hy - 15, hx + 14, hy + 13), E(hx + 2, hy - 4, hx + 25, hy + 14)]
                muzzle = [([E(hx + 6, hy - 1, hx + 27, hy + 15)], GIR_CREAM)]
                spots = [([_spot(hx - 8, hy - 7, 3.6, 2)], GIR_SPOT)] if st["spots"] >= 3 else []
                parts.append(Part(skull, GIR, spots + muzzle, grow=1, outline=True))
                ex, ey = hx + 2, hy - 3
                face = list(stage_eye(eyes, ex, ey))
                if st["lash"] and eyes in ("dot", "closed", "happy"):
                    face.append(([L([(ex + 3.5, ey - 3.5), (ex + 6.5, ey - 6)], 1.3), L([(ex + 1, ey - 5), (ex + 2.4, ey - 8)], 1.3)], EYE))
                if brow:
                    face += brows(brow, ex, ey + _ST["eye_dy"], flip=True)
                face.append(([E(hx + 19, hy + 1.5, hx + 21.5, hy + 4)], mix(GIR_CREAM, GIR_LINE, 0.55)))
                face += gf_blush(hx + 4, hy + 8)
                face += gf_mouth(mouth, hx + 17, hy + 10, munch)
            else:
                skull = [E(hx - 18, hy - 16, hx + 18, hy + 12), E(hx - 13, hy - 2, hx + 13, hy + 17)]
                muzzle = [([E(hx - 12.5, hy + 1, hx + 12.5, hy + 18)], GIR_CREAM)]
                spots = [([_spot(hx, hy - 10, 3.6, 1)], GIR_SPOT)] if st["spots"] >= 3 else []
                parts.append(Part(skull, GIR, spots + muzzle, grow=1, outline=True))
                face = []
                for ex, flip in ((hx - 8.5, False), (hx + 8.5, True)):
                    face += stage_eye(eyes, ex, hy - 4, flip)
                    if st["lash"] and eyes in ("dot", "closed", "happy"):
                        s = 1 if flip else -1
                        face.append(([L([(ex + s * 3.6, hy - 7.5), (ex + s * 6.5, hy - 10)], 1.3)], EYE))
                    if brow:
                        face += brows(brow, ex, hy - 4 + _ST["eye_dy"], flip=flip)
                nost = mix(GIR_CREAM, GIR_LINE, 0.55)
                face.append(([E(hx - 5.5, hy + 5, hx - 3, hy + 7.4), E(hx + 3, hy + 5, hx + 5.5, hy + 7.4)], nost))
                face += gf_blush(hx - 13, hy + 6) + gf_blush(hx + 13, hy + 6)
                face += gf_mouth(mouth, hx, hy + 12, munch)
            if st["ossi"] == 0:  # 아기: 뿔 끝 털 뭉치
                parts.append(Part([], GIR, grow=0, sharp=True,
                                  spikes=[strand([(tx, ty + 1), (tx - 2, ty - 4), (tx - 4, ty - 6)], 3.0, 0.8) for tx, ty in tips]
                                  + [strand([(tx, ty + 1), (tx + 2, ty - 4), (tx + 4, ty - 5)], 3.0, 0.8) for tx, ty in tips]))
            if st["daisy"]:
                dx, dy = (hx - 12, hy - 15) if side else (hx + 13, hy - 17)
                pet = [C(dx + 3.6 * math.cos(math.radians(a)), dy + 3.6 * math.sin(math.radians(a)), 2.4) for a in range(0, 360, 60)]
                parts.append(Part(pet + [C(dx, dy, 2.0)], DAISY, [([C(dx, dy, 2.0)], DAISY_C)], grow=0, outline=True))
            ops += face
    return parts, ops


def gf_mouth(kind, mx, my, munch=False):
    if kind is None:
        return [([L(arc(mx, my - 1.6, 3.2, 2.2, 20, 160, 8), 1.3)], LINE)]
    if kind in ("frown", "pout", "chew", "wavy", "sip"):
        return dp.bee_mouth(kind, mx, my)
    rx, ry = {"open": (3.6, 4.0), "big": (5.0, 5.6), "yawn": (4.2, 6.0), "oh": (2.6, 3.0)}[kind]
    if kind == "oh":
        return [([E(mx - 2.6, my - 2.4, mx + 2.6, my + 3.0)], LINE), ([E(mx - 1.5, my - 1.3, mx + 1.5, my + 1.9)], MOUTH_IN)]
    return open_mouth(mx, my - 1, rx, ry, tongue=kind != "yawn")


def gf_leg(x0, y0, x1, y1, w, col=GIR, spot=False):
    """다리 하나 (엉덩이 → 발굽) + 발굽. 몸에 녹아 붙는다 (자기 외곽선 없음 — 고양이·루돌프 네 발 뼈대처럼):
    윗부분은 넓게 몸 안쪽(엉덩이·어깨)에서 시작해 발굽 쪽으로 가늘어지고, 그 위를 몸이 다시 덮는다 (giraffe body_cover)."""
    dx, dy = x1 - x0, y1 - y0
    ln = math.hypot(dx, dy) or 1.0
    top = (x0 - dx / ln * 7, y0 - dy / ln * 7)  # 몸 속으로 7px 더
    mid = (x0 + dx * 0.45, y0 + dy * 0.45)
    shank = strand([top, (x0, y0), mid, (x1, y1 - 2.5)], w * 1.75, w * 0.92)
    marks = [([E(x1 - w * 0.62, y1 - 4.4, x1 + w * 0.62, y1 + 1)], HOOF)]
    if spot:  # 엉덩이 무늬가 다리로 이어진다
        marks.insert(0, ([_spot(x0 + dx * 0.12, y0 + dy * 0.12, w * 0.5, 4)], GIR_SPOT))
    return Part([shank, E(x1 - w * 0.55, y1 - 4.5, x1 + w * 0.55, y1)], col, marks, grow=1)


def gf_blush(cx, cy):
    """기린 볼터치 — 작은 머리에 맞게 고양이·오리 볼터치 크기쯤으로 작고 연하게 (빗금 하나)."""
    bx, by = _ST["blush"]
    soft = mix(BLUSH, GIR, 0.35)
    return [([E(cx - 4.4 * bx, cy - 2.8 * by, cx + 4.4 * bx, cy + 2.8 * by)], soft),
            ([L([(cx - 0.6, cy + 1.2), (cx + 0.9, cy - 1.2)], 0.9)], mix(dp.BLUSH_LINE, soft, 0.4))]


def giraffe(sp, *, side=True, cx=64.0, bottom=None, rot=0.0, sq=(1.0, 1.0), eyes="dot", mouth=None, brow=None, neck_ang=18.0,
            neck_k=1.0, head=(0.0, 0.0), head_rot=0.0, legs=None, lie=False, tail=0.0, fx=(), props=(), held=(), front_legs=None,
            neck_period=None, neck_phase=0.0, parts_out=None, belly=False, munch=False):
    """기린 한 장. side: 오른쪽을 보는 옆모습. neck_ang: 목 기울기 (0 = 수직, + = 앞으로) · neck_k: 목 길이 배율 ·
    legs: 옆모습 다리 [(엉덩이 dx, 발굽 dx, 발굽 dy)] ×4 (뒤 먼 쪽, 뒤, 앞 먼 쪽, 앞) · lie: 엎드림 (다리 접음) ·
    front_legs: 앞모습 앞다리 [(발굽 dx, dy)] ×2 · parts_out: dict면 목 위치(어깨·머리 밑 점)를 적어 둔다 (목 늘이기 층)."""
    st = _gf()
    bk = _ST["body"]
    lk = _ST["leg"] ** 0.5
    parts, ops = [], []
    leg_len = 19 * lk
    if bottom is None:
        bottom = FLOOR - leg_len + 3 if not lie else FLOOR - 1
    rx, ry = 25 * bk * _ST["torso"][0], 15 * bk * _ST["torso"][1]
    bcy = bottom - ry
    nl = NECK_LEN * st["neck"] * neck_k
    with xf(rot=rot, pivot=(cx, FLOOR)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, FLOOR)):
            if side:
                bx = cx - 8
                hipy = bcy + ry * 0.45
                legs = legs or [(-17, -17, 0), (-10, -10, 0), (12, 12, 0), (19, 19, 0)]
                if not lie:
                    for j, (hdx, fdx, fdy) in enumerate(legs):
                        far = j in (0, 2)
                        parts.append(gf_leg(bx + hdx, hipy, bx + fdx, FLOOR + fdy, 7.0 * lk, mix(GIR, GIR_LINE, 0.16) if far else GIR,
                                            spot=not far and st["spots"] >= 2))
                else:
                    for x in (bx - 16, bx + 14):
                        parts.append(Part([E(x - 9, FLOOR - 6, x + 9, FLOOR), E(x - 7, FLOOR - 12, x + 7, FLOOR - 2)], GIR,
                                          [([E(x + 4, FLOOR - 5, x + 10, FLOOR)], HOOF)], grow=1))
                # 꼬리
                with xf(rot=tail, pivot=(bx - rx + 3, bcy - 4)):
                    tpts = [(bx - rx + 4, bcy - 5), (bx - rx - 4, bcy + 1), (bx - rx - 7, bcy + 10)]
                    parts.append(Part([strand(tpts, 5.6, 3.6), E(tpts[-1][0] - 3.4, tpts[-1][1] - 1, tpts[-1][0] + 3.4, tpts[-1][1] + 8)], GIR,
                                      [([E(tpts[-1][0] - 3.4, tpts[-1][1] - 1, tpts[-1][0] + 3.4, tpts[-1][1] + 8)], MANE)], grow=1))
                spots = [([_spot(bx + dx, bcy + dy, r * bk, j) for j, (dx, dy, r, lv) in enumerate(BODY_SPOTS) if lv <= st["spots"]], GIR_SPOT)]
                belly_m = [([E(bx - rx * 0.7, bcy + ry * 0.45, bx + rx * 0.8, bcy + ry * 1.4)], GIR_CREAM)]
                body_shape = [E(bx - rx, bcy - ry, bx + rx, bcy + ry)]
                parts.insert(0, Part(body_shape, GIR, grow=1))
                # 다리·꼬리 뿌리를 몸이 다시 덮는다 — 이음매에 선이 없고 무늬·배 색이 이어진다
                parts.append(Part(body_shape, GIR, belly_m + spots, grow=1))
                a = (bx + rx * 0.62, bcy - ry * 0.35)
                ang = math.radians(neck_ang)
                b = (a[0] + math.sin(ang) * nl, a[1] - math.cos(ang) * nl)
                if parts_out is not None:
                    parts_out.update(a=a, b=b)
                neck = Part([L([a, (b[0], b[1] - 4)], NECK_W * bk ** 0.5)], GIR,
                            neck_marks(a, b, NECK_W * bk ** 0.5, neck_period, neck_phase), grow=1)
                parts.insert(1, neck)
                hx, hy = b[0] + 4 + head[0], b[1] - 9 + head[1]
                hp, ho = gf_head(sp, hx, hy, side=True, eyes=eyes, mouth=mouth, brow=brow, rot=head_rot, munch=munch)
                parts += hp
                ops += ho
            else:
                bw = rx * 0.82
                if belly:
                    bcy = FLOOR - 12
                body_shape = [E(cx - bw, bcy - ry * 1.15, cx + bw, bcy + ry * 1.05)]
                body_marks = [([E(cx - bw * 0.55, bcy - ry * 0.4, cx + bw * 0.55, bcy + ry * 1.3)], GIR_CREAM),
                              ([_spot(cx + dx * 0.8, bcy + dy * 0.7, r * bk * 0.9, j) for j, (dx, dy, r, lv) in enumerate(BODY_SPOTS)
                                if lv <= st["spots"] and abs(dx) > 10], GIR_SPOT)]
                parts.append(Part(body_shape, GIR, grow=1))
                fl = front_legs or [(0, 0), (0, 0)]
                for j, (fdx, fdy) in enumerate(fl):
                    s = -1 if j == 0 else 1
                    x = cx + s * bw * 0.45
                    if belly:
                        parts.append(gf_leg(x, bcy - 2, x + s * 8 + fdx, bcy - 22 + fdy, 7.5 * lk))
                    else:
                        parts.append(gf_leg(x, bcy, x + fdx, FLOOR + fdy, 8.0 * lk))
                if belly:  # 뒷다리도 위로
                    for s in (-1, 1):
                        parts.insert(1, gf_leg(cx + s * bw * 0.7, bcy + 2, cx + s * (bw + 6), bcy - 14, 7.0 * lk, mix(GIR, GIR_LINE, 0.16)))
                # 다리 뿌리를 몸이 다시 덮는다 (이음매에 선 없이)
                parts.append(Part(body_shape, GIR, body_marks, grow=1))
                a = (cx, bcy - ry * 0.6)
                ang = math.radians(neck_ang)
                b = (a[0] + math.sin(ang) * nl * 0.62, a[1] - math.cos(ang) * nl * 0.62)
                neck = Part([L([a, b], NECK_W * 1.1 * bk ** 0.5)], GIR, neck_marks(a, b, NECK_W * 1.1), grow=1)
                parts.insert(1, neck)
                hx, hy = b[0] + head[0], b[1] - 12 + head[1]
                hp, ho = gf_head(sp, hx, hy, side=False, eyes=eyes, mouth=mouth, brow=brow, rot=head_rot, munch=munch)
                parts += hp
                ops += ho
            parts += [h() for h in held]
    parts += [p() for p in props]
    return dp.compose(parts, ops, list(fx))


def leaf_bubble_fx(cx, cy):
    """생각 풍선 안에 나뭇잎 하나."""
    fx = _bubble(cx, cy)
    leaf = arc(cx, cy, 7.5, 4.6, 0, 360, 18)
    leaf = [(x + (cx - x) * 0.0, y) for x, y in leaf]
    with xf(rot=-30, pivot=(cx, cy)):
        fx += [([T(*leaf), L(leaf, 2.4)], LINE, 0), ([T(*leaf)], LEAF_G, 0), ([L([(cx - 6, cy), (cx + 6, cy)], 1.0)], LEAF_V, 0)]
    return fx


# =====================================================================
# 포즈 — 오리
# =====================================================================
def dk_walk(sp, i):
    """뒤뚱뒤뚱: 몸이 좌우(앞뒤)로 기우뚱, 발이 번갈아."""
    t = [-7, 0, 7, 0][i]
    up = [0, 1.5, 0, 1.5][i]
    feet = [((-3, -3), (3, 0)), ((0, -1), (0, 0)), ((3, 0), (-3, -3)), ((0, 0), (0, -1))][i]
    return duck(sp, bottom=FLOOR - 4 - up, rot=t, head_rot=-t * 0.4, feet=feet, wings=[(1, [10, 20, 10, 20][i])])


def dk_run(sp, i):
    up = [0, 3, 5, 2][i]
    feet = [((-5, -4), (5, 0)), ((-2, -2), (2, -1)), ((5, 0), (-5, -4)), ((2, -1), (-2, -2))][i]
    return duck(sp, bottom=FLOOR - 4 - up, rot=[-10, -4, -10, -4][i] - 4, eyes="happy", mouth="open", feet=feet,
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
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    hy = FLOOR - 4 - 22 * _ST["body"] * 0.5
    held = [lambda: Part([C(64 - 6, hy, 7), C(64 + 6, hy, 7), T((51, hy + 2), (77, hy + 2), (64, hy + 14))], HEART,
                         [([E(53, hy - 5, 57, hy - 1)], WHITE)], grow=0, outline=True)]
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
    hr = 22 * _ST["head"]
    bcy = st_b - ry
    hy0 = bcy - ry * 0.6 - hr * 0.62
    want = my - hr * 0.3 - 6.5 * _ST["head"] ** 0.5 * 0.5
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    return duck(sp, side=False, head=(0, want - hy0), eyes="happy" if bite else "closed", mouth="open" if bite else "chew",
                wings=[(-1, 40), (1, 40)], feet=None, fx=fx)


def dk_level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    up = [150, 165, 175][i]
    return duck(sp, side=False, bottom=FLOOR - 4 - [0, 1, 3][i], eyes="happy", mouth="big", wings=[(-1, up), (1, up)], fx=stars)


DUCK_POSES = {"walk": dk_walk, "run": dk_run, "roll": dk_roll, "idle": dk_idle, "look": dk_look, "sleep": dk_sleep, "hop": dk_hop,
              "fall": dk_fall, "dragged": dk_dragged, "land": dk_land, "sit_front": dk_sit_front, "wave": dk_wave, "tilt": dk_tilt,
              "belly": dk_belly, "stretch": dk_stretch, "heart": dk_heart, "tickle": dk_tickle, "annoyed": dk_annoyed,
              "hungry": dk_hungry, "eat": dk_eat, "level_up": dk_level_up}
DUCK_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in DUCK_POSES.items()}


# =====================================================================
# 포즈 — 기린
# =====================================================================
def _trot(i, k=1.0):
    s = [1, 0, -1, 0][i] * k
    return [(-17, -17 - 4 * s, -2 * max(0, s)), (-10, -10 + 4 * s, -2 * max(0, -s)), (12, 12 + 4 * s, -2 * max(0, -s)),
            (19, 19 - 4 * s, -2 * max(0, s))]


def gf_walk(sp, i):
    return giraffe(sp, legs=_trot(i), bottom=None, neck_ang=[18, 20, 18, 20][i], head_rot=[-3, 0, 3, 0][i], tail=[8, 0, -8, 0][i])


def gf_run(sp, i):
    s = [1, 0.4, -1, -0.4][i]
    legs = [(-17, -24 * s - 10, -3), (-10, -24 * s - 4, -3 if s < 0 else 0), (12, 12 + 9 * s, -3), (19, 19 + 12 * s, 0)]
    return giraffe(sp, legs=legs, neck_ang=34, rot=-3, eyes="happy", mouth="open", tail=-20,
                   bottom=FLOOR - 19 * _ST["leg"] ** 0.5 + 3 - [0, 2, 3, 1][i], fx=speed_fx(2, (64, 82, 98), 12, [0, -3, 0, -3][i]))


def gf_stand(sp, **kw):
    """목을 곧게 세운 옆모습 (목 늘이기 · 구르기 대신)."""
    return giraffe(sp, neck_ang=kw.pop("neck_ang", 2.0), head_rot=kw.pop("head_rot", -10), **kw)


def gf_roll(sp, i):
    # 기린은 구르지 않는다 (pet.json에 spin 없음 — 코어가 구르기 대신 neckstretch). 예전 앱용으로 서 있는 모습
    return gf_stand(sp, eyes="dot" if i == 0 else "happy")


def gf_idle(sp, i):
    return giraffe(sp, sq=(1.0, 1.0 + 0.012 * i), head=(0, -0.8 * i), tail=[0, 6][i])


def gf_look(sp, i):
    return giraffe(sp, head_rot=[-12, 0, 12, 0][i], neck_ang=[14, 18, 22, 18][i], eyes="closed" if i == 3 else "dot")


def gf_sleep(sp, i):
    zs = [z_fx(106, 60, 3.4), z_fx(117, 49, 2.6)] if i == 0 else [z_fx(108, 57, 3.6), z_fx(120, 45, 2.8)]
    return giraffe(sp, lie=True, bottom=FLOOR - 4, neck_ang=72, neck_k=0.75, head=(2, 10 + 0.6 * i), head_rot=8, eyes="closed",
                   sq=(1.0, 1.0 + 0.01 * i), fx=zs)


def gf_annoyed(sp, i):
    return giraffe(sp, lie=True, bottom=FLOOR - 4, neck_ang=40, neck_k=0.8, eyes="squint", mouth="pout", head=(0, 4),
                   fx=[anger_fx(108, 34 - i, 7.0)])


def gf_hop(sp, i):
    lk = _ST["leg"] ** 0.5
    b0 = FLOOR - 19 * lk + 3
    if i == 0:
        return giraffe(sp, bottom=b0 + 4, legs=[(-17, -20, 0), (-10, -13, 0), (12, 15, 0), (19, 22, 0)], eyes="tight", neck_ang=26)
    if i == 1:
        return giraffe(sp, bottom=b0 - 14, legs=[(-17, -26, -12), (-10, -18, -12), (12, 20, -16), (19, 28, -16)], eyes="happy",
                       mouth="open", neck_ang=10, rot=-6, tail=-25)
    return giraffe(sp, bottom=b0 - 8, legs=[(-17, -20, -8), (-10, -12, -8), (12, 14, -8), (19, 22, -8)], eyes="happy", neck_ang=16)


def gf_fall(sp, i):
    stars = _stars([(100, 18, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 18, 3.6), (104, 30, 3.0)])
    lk = _ST["leg"] ** 0.5
    return giraffe(sp, bottom=FLOOR - 19 * lk - 6, rot=[-10, 10][i], eyes="spiral", mouth="oh", neck_ang=[30, 6][i], tail=-30,
                   legs=[(-17, -24, -8), (-10, -16, -10), (12, 18, -10), (19, 26, -8)], fx=stars)


def gf_dragged(sp, i):
    s = [-1, 1][i]
    lk = _ST["leg"] ** 0.5
    return giraffe(sp, bottom=FLOOR - 19 * lk - 4, rot=4 * s, eyes="tight", mouth="open", neck_ang=4,
                   legs=[(-17, -17 + 2 * s, -1), (-10, -10 + 2 * s, 0), (12, 12 + 2 * s, 0), (19, 19 + 2 * s, -1)], tail=10 * s,
                   fx=sweat_fx(108, 26))


def gf_land(sp, i):
    lk = _ST["leg"] ** 0.5
    if i == 0:
        return giraffe(sp, bottom=FLOOR - 19 * lk + 8, sq=(1.06, 0.94), legs=[(-17, -22, 0), (-10, -14, 0), (12, 16, 0), (19, 24, 0)],
                       eyes="tight", neck_ang=28)
    return giraffe(sp, neck_ang=20)


def gf_sit_front(sp, i):
    return giraffe(sp, side=False, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot",
                   head_rot=[0, 2, 0, -2][i])


def gf_wave(sp, i):
    lift = [-24, -30, -24, -30][i]
    return giraffe(sp, side=False, front_legs=[(0, 0), (10, lift)], head_rot=[-4, -2, -4, -2][i], mouth="open" if i % 2 else None)


def gf_tilt(sp, i):
    return giraffe(sp, side=False, head_rot=dp._swing([10, 15, 15][i]), neck_ang=dp._swing([6, 9, 9][i]),
                   fx=[question_fx(18, 22 + [2, 0, -2][i])])


def gf_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return giraffe(sp, side=False, belly=True, rot=-4, eyes="happy", mouth="open", front_legs=[(-wig, wig), (wig, -wig)],
                   neck_k=0.7, head=(0, 6))


def gf_stretch(sp, i):
    if i == 0:
        return giraffe(sp, side=False, sq=(0.96, 1.05), eyes="tight", neck_k=1.15, head=(0, -3))
    if i == 1:
        return giraffe(sp, side=False, eyes="tight", mouth="yawn", head_rot=-6, neck_k=1.1)
    return giraffe(sp, side=False)


def gf_heart(sp, i):
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    hy = FLOOR - 30
    held = [lambda: Part([C(64 - 6, hy, 7), C(64 + 6, hy, 7), T((51, hy + 2), (77, hy + 2), (64, hy + 14))], HEART,
                         [([E(53, hy - 5, 57, hy - 1)], WHITE)], grow=0, outline=True)]
    return giraffe(sp, side=False, eyes="happy", held=held, fx=hs, head_rot=[0, 3, -3][i])


def gf_tickle(sp, i):
    return giraffe(sp, side=False, rot=dp._swing([-6, 6, 0][i]), head_rot=dp._swing([-8, 8, 0][i]), eyes="happy", mouth="open",
                   front_legs=[(-2, [0, -6, 0][i]), (2, [-6, 0, 0][i])])


def gf_hungry(sp, i):
    return giraffe(sp, side=False, rot=-3, eyes="closed", mouth="frown", brow="sad", neck_ang=-8, head_rot=-10,
                   head=(0, [3, 4, 3][i]), fx=leaf_bubble_fx(106, 18))


def gf_eat(sp, i):
    """냠냠 (그릇 없음): 목을 숙여 입이 그릇 밥 더미 바로 위 (프레임 (64, 84))."""
    bite = i % 2 == 0
    mx, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    hk = _ST["head"]
    # 앞모습 입 = 머리 중심 + 12·hk → 머리 중심
    hy = my - 12 * hk
    lk = _ST["leg"] ** 0.5
    bottom = FLOOR - 19 * lk + 3
    ry = 15 * _ST["body"] * _ST["torso"][1]
    bcy = bottom - ry
    nl = NECK_LEN * _gf()["neck"] * 0.5
    b_y = bcy - ry * 0.6 - nl * 0.62
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    return giraffe(sp, side=False, neck_k=0.5, head=(0, hy - (b_y - 12)), eyes="happy" if bite else "closed",
                   mouth="open" if bite else "chew", front_legs=[(-3, 0), (3, 0)], fx=fx)


def gf_level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    return giraffe(sp, side=False, eyes="happy", mouth="big", neck_k=[1.0, 1.08, 1.14][i], front_legs=[(-4, -[4, 8, 10][i]), (4, -[4, 8, 10][i])],
                   fx=stars)


# ---- 목 늘이기 (neckstretch) ----
NECK_FACES = [("round", "oh", False), ("closed", None, False), ("happy", "open", True)]  # 어? · 깜빡 · ✧ 흐뭇
NECK_TILE = 16  # 목 토막 높이 = 무늬 주기 (프레임 px)
NECK_POSE_K = 1.25  # 목 늘이기 자세는 목을 조금 더 세워 늘인다 (아기도 자를 자리가 있게)


def _neck_pose(sp, face, period=None, phase=0.0, out=None):
    eyes, mouth, star = NECK_FACES[face]
    fx = []
    if star:
        fx = sparkle_fx(116, 18, 4.2) + sparkle_fx(20, 30, 3.0)
    return gf_stand(sp, eyes=eyes, mouth=mouth, neck_period=period, neck_phase=phase, parts_out=out, fx=fx, neck_ang=0.0,
                    neck_k=_neck_k())


def _view_k():
    return dp._VIEW["scale"], dp._VIEW["lift"]


def _to_frame_y(y):
    s, lift = _view_k()
    return 126 + (y - 126) * s - lift


def _neck_k():
    """목 늘이기 자세의 목 길이 배율 — 목이 짧은 아기·유아도 머리와 어깨 사이에 곧은 목이 보이게."""
    return NECK_POSE_K * max(1.0, 0.85 / _gf()["neck"])


def neck_geometry(sp):
    """(현재 단계·view 안에서) 목 늘이기 좌표: 어깨 a, 머리 밑 b (pre), 자르는 높이 cut (프레임 정수 px — 곧은 목만 지나는 줄),
    무늬 주기 (pre), 늘이는 길이 ext (지금 목 길이를 토막 높이 배수로)."""
    out = {}
    im = _neck_pose(sp, 0, out=out)
    a, b = out["a"], out["b"]
    s, lift = _view_k()
    alpha = im.getchannel("A").load()
    y0, y1 = round(_to_frame_y(b[1])), round(_to_frame_y(a[1]))

    def width(y):
        xs = [x for x in range(FRAME) if alpha[x, y] > 128]
        return (max(xs) - min(xs)) if xs else 999

    ws = {y: width(y) for y in range(max(0, y0), min(FRAME, y1 + 1))}
    neck_w = min(ws.values())
    pure = [y for y, w in ws.items() if w <= neck_w + 1]
    # 가장 아래(어깨 쪽) 곧은 목 줄들 — 어깨가 벌어지기 2px 위에서 자른다
    bottom = max(pure)
    top = bottom
    while top - 1 in pure:
        top -= 1
    cut = max(top + 1, bottom - 2)
    cut_pre = 126 + (cut + lift - 126) / s
    period = NECK_TILE / s
    ext = max(NECK_TILE, round((a[1] - b[1]) * s / NECK_TILE) * NECK_TILE)
    return dict(a=a, b=b, cut=cut, cut_pre=cut_pre, period=period, ext=ext, run=(top, bottom))


def gf_neckstretch(sp, i):
    """시트의 neckstretch 행 (앱이 층을 못 그릴 때도 보이는 모습): 목을 곧게 세우고 어? → 깜빡 → ✧."""
    g = neck_geometry(sp)
    return _neck_pose(sp, i, g["period"], g["cut_pre"])


def neck_layers(sp):
    """현재 단계(view 포함)의 목 늘이기 층 → (층 그림 4칸 가로 512×128, 목 토막 128×NECK_TILE, 메타)."""
    g = neck_geometry(sp)
    cut = g["cut"]
    strip = Image.new("RGBA", (4 * FRAME, FRAME), (0, 0, 0, 0))
    full0 = _neck_pose(sp, 0, g["period"], g["cut_pre"])
    strip.paste(full0.crop((0, cut, FRAME, FRAME)), (0, cut))
    for j in range(3):
        im = _neck_pose(sp, j, g["period"], g["cut_pre"])
        strip.paste(im.crop((0, 0, FRAME, cut)), ((j + 1) * FRAME, 0))
    # 목 토막: 목만 아주 길게 그려 자르는 높이 바로 위 NECK_TILE px (무늬 주기와 같아 위아래로 이어진다)
    a, b = g["a"], g["b"]
    w = NECK_W * _ST["body"] ** 0.5
    col = Part([L([(a[0], a[1] + 200), (a[0], a[1] - 400)], w)], GIR,
               neck_marks((a[0], a[1] + 200), (a[0], a[1] - 400), w, g["period"], g["cut_pre"]), grow=1)
    tall = dp.compose([col])
    tile = tall.crop((0, cut - NECK_TILE, FRAME, cut))
    meta = {"cut": cut, "tile": NECK_TILE, "ext": g["ext"]}
    return strip, tile, meta


GIRAFFE_POSES = {"walk": gf_walk, "run": gf_run, "roll": gf_roll, "idle": gf_idle, "look": gf_look, "sleep": gf_sleep, "hop": gf_hop,
                 "fall": gf_fall, "dragged": gf_dragged, "land": gf_land, "sit_front": gf_sit_front, "wave": gf_wave,
                 "tilt": gf_tilt, "belly": gf_belly, "stretch": gf_stretch, "heart": gf_heart, "tickle": gf_tickle,
                 "annoyed": gf_annoyed, "hungry": gf_hungry, "eat": gf_eat, "level_up": gf_level_up,
                 "neckstretch": gf_neckstretch}
# 목 늘이기 행은 층과 같은 좌표여야 해서 밀지 않는다
GIRAFFE_POSES = {k: (v if k == "neckstretch" else _fit(v)) for k, v in GIRAFFE_POSES.items()}

_COMMON = dict(front_ears=[], front_ear_pivots=(), front_ear_marks=[], front_head_marks=[], front_body_marks=[], front_tail=[],
               front_tail_root=(64, 112), side_ears=[], ears_over=False, side_head_marks=[], side_torso_marks=[], side_tail=[],
               tail_root=(28, 108))

DUCK = Species(
    id="duck", name="오리", walk_speed=80, scale=0.94, base=DUCK_W, back=DUCK_W, belly=DUCK_W, nose=BILL, mouth="w",
    front_tail_color=DUCK_W, tail_color=DUCK_W, stage_size=dict.fromkeys(STAGES, 0.9), stripe=BILL, outline=DUCK_LINE,
    stage_tweak={"baby": dict(pale=0.0, eye=1.2, fit=0.9), "kinder": dict(pale=0.0, eye=1.12, fit=0.93),
                 "child": dict(pale=0.0, eye=1.05), "adult": dict(fit=0.94)},
    specials=[], poses=DUCK_POSES, **_COMMON,
)

GIRAFFE = Species(
    id="giraffe", name="기린", walk_speed=75, scale=0.94, base=GIR, back=GIR, belly=GIR_CREAM, nose=GIR_LINE, mouth="w",
    front_tail_color=GIR, tail_color=GIR, stage_size={"baby": 1.0, "kinder": 1.0, "child": 1.1, "teenager": 1.1, "adult": 1.1},
    stripe=GIR_SPOT, outline=GIR_LINE,
    stage_tweak={"baby": dict(pale=0.0, eye=1.2, fit=0.9), "kinder": dict(pale=0.0, eye=1.1, fit=0.93),
                 "child": dict(pale=0.0), "adult": dict(fit=0.94)},
    specials=["neckstretch"], poses=GIRAFFE_POSES, **_COMMON,
)


def register():
    dp.SPECIES[DUCK.id] = DUCK
    dp.SPECIES[GIRAFFE.id] = GIRAFFE


register()


def _in_view(sp):
    """draw_pets.frames와 같은 view (층을 시트와 같은 좌표로)."""
    return view(scale=sp.scale * _ST["fit"], lift=dp.LIFT + 1.7 * dp.OUTLINE * (_ST["line"] - 1))


def write_neck_layers(out_dir):
    """기린 목 늘이기 층: neck_<stage>.png · neck_tile_<stage>.png + {단계: 메타}."""
    meta = {}
    for st in STAGES:
        with dp.stage(st, GIRAFFE.stage_tweak.get(st)):
            ssp = dp.staged_species(GIRAFFE)
            old = dp._INK["line"]
            dp._INK["line"] = GIRAFFE.outline
            try:
                with _in_view(ssp):
                    strip, tile, m = neck_layers(ssp)
            finally:
                dp._INK["line"] = old
        strip.save(out_dir / f"neck_{st}.png")
        tile.save(out_dir / f"neck_tile_{st}.png")
        meta[st] = m
    return meta


def write_farm(sp, out_dir):
    """시트·pet.json (+ 기린: 목 늘이기 층과 pet.json neck_stretch, roll은 돌리지 않음)."""
    out_dir = Path(out_dir)
    by_stage = dp.write_species(sp, out_dir)
    if sp is GIRAFFE:
        meta = write_neck_layers(out_dir)
        pj = out_dir / "pet.json"
        m = json.loads(pj.read_text(encoding="utf-8"))
        m["animations"]["roll"].pop("spin", None)
        m["neck_stretch"] = meta
        pj.write_text(json.dumps(m, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return by_stage


def preview_species(by_stage, path, bg=(255, 255, 255)):
    from draw_halloween import preview_species as pv
    pv(by_stage, path, bg)


def preview_neck(out_dir, path):
    """목 늘이기 미리보기: 단계마다 평소 → 늘이는 중 → 다 늘임 (✧) — 앱처럼 층을 겹쳐 2배로."""
    meta = json.loads((Path(out_dir) / "pet.json").read_text(encoding="utf-8"))["neck_stretch"]
    steps = [0.0, 0.5, 1.0, 1.0]
    heads = [0, 0, 1, 2]
    top = 70
    cw, ch = FRAME, FRAME + top
    img = Image.new("RGBA", (len(STAGES) * len(steps) * (cw + 4) + 8, ch + 8), (236, 240, 246, 255))
    for si, st in enumerate(STAGES):
        strip = Image.open(Path(out_dir) / f"neck_{st}.png")
        tile = Image.open(Path(out_dir) / f"neck_tile_{st}.png")
        m = meta[st]
        for k, (t, h) in enumerate(zip(steps, heads)):
            ext = round(m["ext"] * t)
            cell = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
            cell.alpha_composite(strip.crop((0, 0, FRAME, FRAME)), (0, top))
            y = top + m["cut"]
            filled = 0
            while filled < ext:
                hgt = min(m["tile"], ext - filled)
                piece = tile.crop((0, 0, FRAME, hgt))  # 머리 쪽(위)에 붙어 함께 올라간다
                cell.alpha_composite(piece, (0, y - ext + filled))
                filled += hgt
            cell.alpha_composite(strip.crop(((h + 1) * FRAME, 0, (h + 2) * FRAME, FRAME)), (0, top - ext))
            img.alpha_composite(cell, (4 + (si * len(steps) + k) * (cw + 4), 4))
    img.resize((img.width * 2, img.height * 2), Image.LANCZOS).save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    for sp in (DUCK, GIRAFFE):
        out = root / "ui" / "pets" / sp.id
        by_stage = write_farm(sp, out)
        preview_species(by_stage, pv / f"stages_{sp.id}.png")
    preview_neck(root / "ui" / "pets" / "giraffe", pv / "neck_giraffe.png")
    print("ok: duck, giraffe")


if __name__ == "__main__":
    main()
