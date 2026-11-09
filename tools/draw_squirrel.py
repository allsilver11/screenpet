"""다람쥐 🐿️ — 크고 복슬한 꼬리를 말아 올린 날쌘 꼬마 다람쥐(squirrel). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_squirrel.py → ui/pets/squirrel/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_squirrel.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

다람쥐: 큰 동글 머리 + 작은 콩 몸 + 짧은 네 다리 (뒷다리는 통통한 허벅지와 긴 발), 몸 뒤로 S자로 솟아 끝이 말린 커다란 복슬 꼬리.
        털은 따뜻한 적갈색, 꼬리는 조금 더 붉고 안쪽에 밝은 털 줄, 배·주둥이·볼은 크림색, 귀 끝에 짙은 털 술. 이름은 "다람쥐".
        단계: 아기 = 연한 털, 작은 꼬리 (꼬리 테두리 솜털이 더 삐죽), 귀 술 없이 정수리 솜털 → 유아 작은 귀 술 → 어린이 →
              청소년 큰 꼬리 → 어른 가장 큰 깃털 꼬리 + 머리에 도토리 깍정이 모자.
걷기 = 총총 (몸이 위아래로 통통, 다리가 번갈아, 꼬리 끝이 살랑). 달리기 = 깡충 뛰는 질주 (모았다 쭉 뻗기, 꼬리가 뒤로 흩날림).
벽 타기 = 뒷모습 (고양이처럼 pet.json climb·slide): 앞발이 머리 옆·위로 번갈아 빼꼼, 꼬리는 몸 옆으로 솟아 살랑.
구르기 = 꼬리로 몸을 감싼 동글 공 (앱이 돌린다). 잠 = 엎드려 꼬리를 이불처럼 덮는다.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 0 = 몸을 숙여 두 앞발로 그릇(프레임 (64, 88))에서 한 알 집기 →
    1·2·3 = 앉아서 두 앞발로 입 앞에 쥐고 오물오물 (볼 빵빵, 앞니, 부스러기).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, L, T, WHITE, Part, Species, _ST, arc, mix, strand, xf)  # noqa: E402
from draw_pets import _fit  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

FLOOR = 125.0

SQ_LINE = (116, 66, 50)  # 외곽선 (짙은 밤색)
FUR = (214, 120, 66)  # 몸·머리 (따뜻한 적갈색, 청소년 기준)
TAIL = (200, 96, 54)  # 꼬리 (조금 더 붉고 짙게)
TAIL_LIGHT = (240, 160, 104)  # 꼬리 안쪽 밝은 털 줄
CREAM = (255, 243, 222)  # 배·주둥이·볼
TUFT = (176, 90, 56)  # 귀 끝 털 술
EAR_IN = (246, 178, 160)
PAW = (244, 198, 166)  # 앞발 끝 (연한 살구색)
NOSE = (176, 92, 88)
TOOTH = (255, 255, 252)
MORSEL, MORSEL_DARK, MORSEL_LIGHT = (226, 176, 104), (182, 128, 72), (246, 214, 160)  # 두 앞발에 쥔 한 알
CAP, CAP_DARK = (164, 112, 70), (124, 80, 52)  # 어른 도토리 깍정이 모자
CAP_STEM = (110, 76, 50)
ACORN, ACORN_DARK = (214, 150, 84), (176, 112, 60)  # 생각 풍선 속 도토리

PALE = {0: 0.24, 1: 0.13, 2: 0.05, 3: 0.0, 4: 0.0}  # 어릴수록 털이 연하다 (크림 쪽으로)

# 단계(lvl)별 — tail: 꼬리 크기 · fluff: 꼬리 테두리 솜털 삐죽 길이 · tuft: 귀 끝 털 술 길이 (0 = 없음) · fuzz: 정수리 솜털 · hat: 도토리 모자
SQ_STAGE = {
    0: dict(tail=0.8, fluff=1.35, tuft=0.0, fuzz=True, hat=False),
    1: dict(tail=0.86, fluff=1.15, tuft=0.6, fuzz=False, hat=False),
    2: dict(tail=0.93, fluff=1.0, tuft=0.8, fuzz=False, hat=False),
    3: dict(tail=1.0, fluff=1.0, tuft=1.0, fuzz=False, hat=False),
    4: dict(tail=1.14, fluff=1.0, tuft=1.1, fuzz=False, hat=True),
}


def _sq():
    return SQ_STAGE[_ST["lvl"]]


def _lvl(stage):
    return _ST["lvl"] if stage is None else STAGES.index(stage)


def fur_color(stage=None):
    """몸 털 색. stage: 단계 이름 (시험용, 없으면 지금 단계). 어릴수록 연하다."""
    return mix(FUR, CREAM, PALE[_lvl(stage)])


def tail_color(stage=None):
    return mix(TAIL, CREAM, PALE[_lvl(stage)])


def paw_color(stage=None):
    return PAW


def far_color():
    """먼 쪽 다리 (조금 어둡게)."""
    return mix(fur_color(), SQ_LINE, 0.16)


# =====================================================================
# 꼬리
# =====================================================================
# 꼬리 뼈대 (뿌리 (0, 0) 기준, 청소년 크기: x, y, 굵기 반지름). ccw: 반시계로 말림 (바깥 솜털이 반대쪽)
TAILS = {
    # 옆모습: 몸 뒤로 S자로 솟아 끝이 앞(머리 쪽)으로 말림
    "up": ([(0, 0, 7.5), (-7, -7, 11), (-13, -18, 14.5), (-16, -31, 16.5), (-14, -45, 17.5), (-7, -57, 16.5), (3, -63, 13.5), (12, -62, 10),
            (17, -56, 7)], False),
    # 달리기·뛰기: 뒤로 흩날리다 끝만 위로
    "flow": ([(0, 0, 7), (-8, -4, 10), (-17, -10, 12), (-25, -18, 13.5), (-29, -29, 13.5), (-29, -40, 12), (-24, -48, 9.5), (-18, -50, 7)], False),
    # 앞모습: 몸 왼쪽 뒤에서 솟아 머리 옆에서 바깥으로 말림
    "front": ([(0, 0, 8), (-8, -7, 10.5), (-15, -18, 13), (-19, -32, 14.5), (-19, -46, 14.5), (-14, -57, 12.5), (-6, -62, 9.5), (0, -59, 6.5)], False),
    # 뒷모습: 몸 오른쪽으로 솟아 안쪽(왼쪽)으로 말림
    "back": ([(0, 0, 7.5), (8, -5, 10), (15, -14, 12), (19, -26, 13), (19, -39, 12.5), (15, -49, 10.5), (9, -53, 7.5), (5, -50, 5)], True),
}


def _chain(pts, n=5):
    """뼈대 점 사이를 촘촘히 (부드러운 굵기 변화)."""
    out = []
    for (x0, y0, r0), (x1, y1, r1) in zip(pts, pts[1:]):
        for k in range(n):
            t = k / n
            out.append((x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r0 + (r1 - r0) * t))
    out.append(pts[-1])
    return out


def tail_part(root, kind="up", rot=0.0, k=None, mirror=False, puff=1.0):
    """복슬 꼬리 한 덩어리: 굵기가 변하는 동그라미 사슬 + 바깥쪽 솜털 삐죽 + 안쪽 밝은 털 줄.
    root: 뿌리 · rot: 뿌리 기준 회전 (도, 시계) · k: 크기 (없으면 단계 꼬리 크기) · mirror: 좌우 뒤집기 · puff: 굵기 배율."""
    st = _sq()
    k = st["tail"] * _ST["body"] ** 0.3 if k is None else k
    pts, ccw = TAILS[kind]
    if mirror:
        pts, ccw = [(-x, y, r) for x, y, r in pts], not ccw
    rx, ry = root
    ch = [(rx + x * k, ry + y * k, r * k * puff) for x, y, r in _chain(pts)]
    n = len(ch)
    fl = st["fluff"]
    with xf(rot=rot, pivot=root):
        shapes_ = [C(x, y, r) for x, y, r in ch]
        spikes, light = [], []
        for j in range(4, n - 3, 3):
            x, y, r = ch[j]
            a, b = ch[j - 1], ch[j + 1]
            dx, dy = b[0] - a[0], b[1] - a[1]
            ln = math.hypot(dx, dy) or 1.0
            ox, oy = (dy / ln, -dx / ln) if not ccw else (-dy / ln, dx / ln)  # 바깥 (말리는 반대쪽)
            px_, py_ = -oy, ox
            tip, w = 4.8 * k * fl, 3.4 * k
            bx_, by_ = x + ox * (r - 1.5), y + oy * (r - 1.5)
            spikes.append(T((bx_ - px_ * w, by_ - py_ * w), (bx_ + ox * tip + px_ * w * 0.5, by_ + oy * tip + py_ * w * 0.5),
                            (bx_ + px_ * w, by_ + py_ * w)))
        for j in range(3, n - 6):
            x, y, r = ch[j]
            a, b = ch[j - 1], ch[j + 1]
            dx, dy = b[0] - a[0], b[1] - a[1]
            ln = math.hypot(dx, dy) or 1.0
            ox, oy = (dy / ln, -dx / ln) if not ccw else (-dy / ln, dx / ln)
            light.append((x - ox * r * 0.34, y - oy * r * 0.34))
        marks = [([L(light, 2.4 * k)], TAIL_LIGHT)] if len(light) > 1 else []
        return Part(shapes_, tail_color(), marks, grow=1, outline=True, spikes=spikes)


def plume(ch, ccw, k, fl, col=None, stripe=True):
    """동그라미 사슬(ch: [(x, y, r)], 현재 변환 안)로 복슬 덩어리 Part: 바깥쪽 솜털 삐죽 + 안쪽 밝은 털 줄."""
    n = len(ch)
    spikes, light = [], []

    def normal(j):
        a, b = ch[max(j - 1, 0)], ch[min(j + 1, n - 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1.0
        return (dy / ln, -dx / ln) if not ccw else (-dy / ln, dx / ln)  # 바깥 (말리는 반대쪽)

    for j in range(3, n - 2, 2):
        x, y, r = ch[j]
        ox, oy = normal(j)
        px_, py_ = -oy, ox
        tip, w = 3.8 * k * fl, 3.0 * k
        bx_, by_ = x + ox * (r - 1.5), y + oy * (r - 1.5)
        spikes.append(T((bx_ - px_ * w, by_ - py_ * w), (bx_ + ox * tip + px_ * w * 0.5, by_ + oy * tip + py_ * w * 0.5),
                        (bx_ + px_ * w, by_ + py_ * w)))
    if stripe:
        for j in range(3, n - 6):
            x, y, r = ch[j]
            ox, oy = normal(j)
            light.append((x - ox * r * 0.34, y - oy * r * 0.34))
    marks = [([L(light, 2.4 * k)], TAIL_LIGHT)] if len(light) > 1 else []
    return Part([C(x, y, r) for x, y, r in ch], col or tail_color(), marks, grow=1, outline=True, spikes=spikes)


# =====================================================================
# 부품
# =====================================================================
def ear(ex, ey, k, rot=0.0, tuft=0.0, inner=True, col=None):
    """귀 하나: 뿌리 (ex, ey) 위로 동글 뾰족, 끝에 짙은 털 술 (tuft 길이 배율, 0 = 없음)."""
    with xf(rot=rot, pivot=(ex, ey + 4 * k)):
        shp = [T((ex - 6.6 * k, ey + 5 * k), (ex - 0.4 * k, ey - 11 * k), (ex + 6.6 * k, ey + 5 * k)), C(ex, ey + 1.5 * k, 5.8 * k)]
        marks = [([T((ex - 3.4 * k, ey + 4 * k), (ex - 0.4 * k, ey - 6 * k), (ex + 3.4 * k, ey + 4 * k))], EAR_IN)] if inner else []
        spikes = []
        if tuft:
            tx, ty = ex - 0.4 * k, ey - 9.5 * k
            spikes = [strand([(tx - 2.0 * k, ty + 3 * k), (tx - 1.6 * k, ty - 4 * tuft * k), (tx + 0.4 * k, ty - 8.5 * tuft * k)], 5.4 * k, 0.8),
                      strand([(tx + 1.4 * k, ty + 3 * k), (tx + 2.2 * k, ty - 3 * tuft * k), (tx + 3.4 * k, ty - 5.4 * tuft * k)], 3.4 * k, 0.8)]
            marks.append(([C(tx, ty - 4 * tuft * k, 5 * k * max(tuft, 0.6) + 0.5)], TUFT))
        return Part(shp, col or fur_color(), marks, grow=0, outline=True, spikes=spikes)


def acorn_cap(x, y, hr, tilt):
    """어른 모자: 도토리 깍정이 (반구 + 비늘 무늬 + 꼭지). (x, y) = 머리에 닿는 아랫변 가운데."""
    w, h = hr * 0.38, hr * 0.3
    with xf(rot=tilt, pivot=(x, y)):
        dome = T(*arc(x, y + 1, w, h, 180, 360, 16))
        rim = E(x - w - 0.8, y - 2.2, x + w + 0.8, y + 2.6)
        stem = L([(x, y - h + 0.5), (x + 1.2, y - h - 3.8), (x + 3.2, y - h - 5.4)], 2.4)
        step = 2 * w / 3.5
        hatch = [L([(x - w + 2 + step * j, y - h * 0.9), (x - w + step * j - 2, y + 1)], 0.8) for j in range(1, 4)]
        hatch += [L([(x - w + step * j - 2, y - h * 0.9), (x - w + 2 + step * j, y + 1)], 0.8) for j in range(1, 4)]
        return Part([dome, rim, stem], CAP, [(hatch, CAP_DARK), ([stem], CAP_STEM), ([E(x - w * 0.55, y - h * 0.72, x - w * 0.1, y - h * 0.42)], mix(CAP, WHITE, 0.3))],
                    grow=0, outline=True)


def morsel(x, y, k=1.0):
    """두 앞발에 쥔 한 알 (작은 도토리 알 — 모자 없이)."""
    return Part([E(x - 4.6 * k, y - 5.6 * k, x + 4.6 * k, y + 5.0 * k), T((x - 2 * k, y + 4 * k), (x + 2 * k, y + 4 * k), (x, y + 7 * k))], MORSEL,
                [([E(x - 3.2 * k, y - 4.4 * k, x - 0.6 * k, y - 0.6 * k)], MORSEL_LIGHT), ([E(x - 4.6 * k, y - 6.2 * k, x + 4.6 * k, y - 3.4 * k)], MORSEL_DARK)],
                grow=0, outline=True)


def _side_mouth(nx, ny, kind, k):
    ops = []
    if kind in ("open", "big", "yawn", "chirp"):
        g = {"open": 3.2, "big": 4.4, "yawn": 5.0, "chirp": 2.6}[kind] * k
        ops.append(([E(nx - 5.5 * k, ny + 2.6 * k, nx - 0.4 * k, ny + 2.6 * k + g)], dp.MOUTH_IN))
        ops.append(([E(nx - 4.4 * k, ny + 2.6 * k + g * 0.5, nx - 1.4 * k, ny + 2.6 * k + g)], dp.TONGUE))
    elif kind == "frown":
        ops.append(([L(arc(nx - 3 * k, ny + 5.4 * k, 2.6 * k, 1.8 * k, 200, 340, 8), 1.2)], SQ_LINE))
    else:
        ops.append(([L([(nx - 0.8 * k, ny + 1.6 * k), (nx - 2.2 * k, ny + 4.2 * k), (nx - 4.6 * k, ny + 4.4 * k)], 1.1)], SQ_LINE))
        if kind == "chew":
            ops.append(([T((nx - 3.2 * k, ny + 4.4 * k), (nx - 1.4 * k, ny + 4.4 * k), (nx - 1.6 * k, ny + 7 * k), (nx - 3.0 * k, ny + 7 * k))], TOOTH))
    return ops


def side_head(hx, hy, hr, eyes, mouth, brow, head_rot=0.0, cheeks=False):
    """옆모습 머리 (오른쪽을 본다): 동글 머리 + 짧은 주둥이 + 크림 볼·턱 + 귀 둘 (털 술) + 눈·코·볼. (Part들, 얼굴 ops)."""
    st = _sq()
    k = hr / 21
    parts = []
    with xf(rot=head_rot, pivot=(hx - hr * 0.2, hy + hr * 0.8)):
        parts.append(ear(hx + hr * 0.12, hy - hr * 0.8, k * 0.86, rot=16, tuft=st["tuft"] * 0.9, inner=False, col=far_color()))
        parts.append(ear(hx - hr * 0.3, hy - hr * 0.76, k, rot=-14, tuft=st["tuft"]))
        shp = [C(hx, hy, hr), C(hx + hr * 0.72, hy + hr * 0.26, hr * 0.46)]
        if cheeks:
            shp.append(C(hx + hr * 0.42, hy + hr * 0.5, hr * 0.42))
        marks = [([E(hx - hr * 0.08, hy + hr * 0.1, hx + hr * 1.34, hy + hr * 1.22)], CREAM)]
        parts.append(Part(shp, fur_color(), marks, grow=1, outline=True, spikes=dp.tuft(hx - 3, hy - hr + 1) if st["fuzz"] else []))
        dp.mark_head(hx - 1, hy - hr * 0.84, hr * 1.25)
        if st["hat"]:
            parts.append(acorn_cap(hx + hr * 0.12, hy - hr * 0.9, hr, 10))
        ex, ey = hx + hr * 0.4, hy - hr * 0.06
        face = list(dp.stage_eye(eyes, ex, ey))
        if brow:
            face += dp.brows(brow, ex, ey + _ST["eye_dy"], flip=True)
        face += dp.blush(hx + hr * 0.2, hy + hr * 0.5)
        nx, ny = hx + hr * 1.13, hy + hr * 0.16
        face += [([E(nx - 2.6 * k, ny - 1.8 * k, nx + 1.4 * k, ny + 1.8 * k)], NOSE)]
        face += _side_mouth(nx, ny, mouth, k ** 0.5)
    return parts, face


def front_leg(x, top, bottom, dx, lift, k, col):
    """옆모습 앞다리 (짧은 막대 + 동글 발)."""
    fx_, fy = x + dx, bottom - lift
    return Part([L([(x, top), (fx_, fy - 3 * k)], 6.4 * k), E(fx_ - 3.6 * k, fy - 4.6 * k, fx_ + 5.0 * k, fy)], col, grow=0, outline=True)


def hind_leg(x, y, bottom, dx, lift, k, col, thigh=True):
    """옆모습 뒷다리: 통통한 허벅지 + 길쭉한 발."""
    fx_, fy = x + dx, bottom - lift
    shp = [E(fx_ - 4 * k, fy - 4.8 * k, fx_ + 11 * k, fy)]
    if thigh:
        shp = [C(x, y, 9.6 * k), L([(x, y), (fx_ + 1, fy - 3 * k)], 6 * k)] + shp
    return Part(shp, col, grow=0, outline=True)


def side(*, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, legs=None, head=(0.0, 0.0), head_rot=0.0,
         tail="up", tail_rot=0.0, tail_puff=1.0, crouch=0.0, cheeks=False, fx=(), held=()):
    """옆모습 다람쥐 (오른쪽을 본다). bottom: 발바닥 (없으면 바닥) · legs: [(dx, 들기)] 뒤-먼 · 앞-먼 · 뒤-가까운 · 앞-가까운 ·
    crouch: 몸을 낮춤 · tail: TAILS 이름 · tail_rot: 꼬리 뿌리 기준 회전."""
    bk, hk = _ST["body"], _ST["head"]
    fk = bk ** 0.5
    rx, ry = 19 * bk * _ST["torso"][0], 13.5 * bk * _ST["torso"][1]
    lg = 4 + 5 * _ST["leg"]
    bottom = FLOOR - 1.5 if bottom is None else bottom
    bx = cx - 6
    by = bottom - lg - ry + crouch
    hr = 21 * hk
    hx, hy = bx + rx * 0.9 + (hk - 1) * 18 + head[0], by - ry * 0.55 - hr * 0.5 + head[1]
    legs = legs or [(0, 0)] * 4
    parts, ops = [], []
    pivot = (bx, bottom)
    far = far_color()
    with xf(rot=rot, pivot=pivot):
        with xf(sx=sq[0], sy=sq[1], pivot=pivot):
            parts.append(tail_part((bx - rx * (0.78 + (hk - 1) * 0.8), by + ry * 0.05), tail, tail_rot, puff=tail_puff))
            parts.append(hind_leg(bx - rx * 0.35, by + ry * 0.3, bottom, legs[0][0] + 4, legs[0][1], fk, far, thigh=False))
            parts.append(front_leg(bx + rx * 0.6, by + ry * 0.2, bottom, legs[1][0] + 3, legs[1][1], fk, far))
            belly = [([E(bx - rx * 0.1, by - ry * 0.35, bx + rx * 1.3, by + ry * 1.6)], CREAM)]
            parts.append(Part([E(bx - rx, by - ry, bx + rx, by + ry)], fur_color(), belly, grow=1, outline=True))
            parts.append(hind_leg(bx - rx * 0.42, by + ry * 0.28, bottom, legs[2][0], legs[2][1], fk, fur_color()))
            parts.append(front_leg(bx + rx * 0.5, by + ry * 0.2, bottom, legs[3][0], legs[3][1], fk, fur_color()))
            hp, face = side_head(hx, hy, hr, eyes, mouth, brow, head_rot, cheeks)
            parts += hp
            ops += face
            parts += [h() for h in held]
    return dp.compose(parts, ops, list(fx))


def _front_mouth(hx, my, kind, k):
    ops = []
    if kind in ("open", "big", "yawn", "chirp"):
        w, g = {"open": (3.4, 4.4), "big": (4.6, 6.0), "yawn": (4.0, 7.0), "chirp": (2.4, 3.2)}[kind]
        ops.append(([E(hx - w * k, my, hx + w * k, my + g * k)], dp.MOUTH_IN))
        ops.append(([E(hx - w * 0.6 * k, my + g * 0.5 * k, hx + w * 0.6 * k, my + g * k)], dp.TONGUE))
        if kind == "big":
            ops.append(([T((hx - 2.2 * k, my), (hx + 2.2 * k, my), (hx + 2.0 * k, my + 2.4 * k), (hx - 2.0 * k, my + 2.4 * k))], TOOTH))
        return ops
    if kind == "frown":
        return [([L(arc(hx, my + 3.6 * k, 3.4 * k, 2.4 * k, 200, 340, 10), 1.3)], SQ_LINE)]
    ops.append(([L(arc(hx - 2.4 * k, my, 2.4 * k, 2.2 * k, 0, 180, 8), 1.2), L(arc(hx + 2.4 * k, my, 2.4 * k, 2.2 * k, 0, 180, 8), 1.2)], SQ_LINE))
    if kind == "chew":  # 오물오물: ω 아래로 앞니 두 개
        ops.append(([T((hx - 2.3 * k, my + 1.6 * k), (hx + 2.3 * k, my + 1.6 * k), (hx + 2.2 * k, my + 5.0 * k), (hx - 2.2 * k, my + 5.0 * k))], SQ_LINE))
        ops.append(([T((hx - 1.7 * k, my + 2.0 * k), (hx - 0.3 * k, my + 2.0 * k), (hx - 0.3 * k, my + 4.4 * k), (hx - 1.7 * k, my + 4.4 * k)),
                     T((hx + 0.3 * k, my + 2.0 * k), (hx + 1.7 * k, my + 2.0 * k), (hx + 1.7 * k, my + 4.4 * k), (hx + 0.3 * k, my + 4.4 * k))], TOOTH))
    return ops


def front_head(hx, hy, hr, eyes, mouth, brow, head_rot=0.0, cheeks=False):
    """앞모습 머리: 동글 머리 + 위로 쫑긋 두 귀 (털 술) + 크림 주둥이·볼 + 눈·코·입·볼터치. (Part들, 얼굴 ops)."""
    st = _sq()
    k = hr / 23
    parts = []
    with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
        for s in (-1, 1):
            parts.append(ear(hx + s * hr * 0.6, hy - hr * 0.72, k, rot=s * 16, tuft=st["tuft"]))
        shp = [C(hx, hy, hr)]
        if cheeks:  # 볼 빵빵
            shp += [C(hx + s * hr * 0.6, hy + hr * 0.44, hr * 0.36) for s in (-1, 1)]
        marks = [([E(hx - hr * 0.58, hy + hr * 0.06, hx + hr * 0.58, hy + hr * 1.04)] +
                  ([C(hx + s * hr * 0.6, hy + hr * 0.5, hr * 0.3) for s in (-1, 1)] if cheeks else []), CREAM)]
        parts.append(Part(shp, fur_color(), marks, grow=1, outline=True, spikes=dp.tuft(hx - 1, hy - hr + 1) if st["fuzz"] else []))
        dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
        if st["hat"]:
            parts.append(acorn_cap(hx + hr * 0.18, hy - hr * 0.9, hr, 14))
        face = []
        for ex, flip in ((hx - hr * 0.38, False), (hx + hr * 0.38, True)):
            face += dp.stage_eye(eyes, ex, hy + hr * 0.02, flip)
            if brow:
                face += dp.brows(brow, ex, hy + hr * 0.02 + _ST["eye_dy"], flip=flip)
        face += dp.blush(hx - hr * 0.64, hy + hr * 0.44) + dp.blush(hx + hr * 0.64, hy + hr * 0.44)
        ny = hy + hr * 0.3
        face += [([E(hx - 2.8 * k, ny - 1.8 * k, hx + 2.8 * k, ny + 2.0 * k)], NOSE)]
        face += _front_mouth(hx, ny + 2.0 * k, mouth, k ** 0.5)
    return parts, face


def front(*, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, arms=("chest", "chest"),
          feet=((0.0, 0.0), (0.0, 0.0)), head=(0.0, 0.0), head_rot=0.0, tail_rot=0.0, cheeks=False, belly=False, fx=(), held=(), hold=None):
    """앞모습 다람쥐 (앉음). arms: 왼·오 앞발 — "chest" 가슴 앞에 모음 · 각도 (0 아래, 90 옆, 170 위) · (x, y) 그 자리 ·
    feet: (dx, 들기) · belly: 발라당 (발이 위로) · hold: 두 앞발 사이에 든 한 알 (x, y) · held: 가슴 앞에 든 것 (하트)."""
    bk, hk = _ST["body"], _ST["head"]
    fk = bk ** 0.5
    rx, ry = 18 * bk * _ST["fbody"][0], 18.5 * bk * _ST["fbody"][1]
    bottom = FLOOR - 2.5 if bottom is None else bottom
    by = bottom - ry
    hr = 23 * hk
    hx, hy = cx + head[0], by - ry * 0.62 - hr * 0.55 + head[1]
    parts, ops = [], []
    pivot = (cx, bottom)
    col = fur_color()
    with xf(rot=rot, pivot=pivot):
        with xf(sx=sq[0], sy=sq[1], pivot=pivot):
            parts.append(tail_part((cx - rx * 0.5, by + ry * 0.55), "front", tail_rot))
            feet_parts = []
            for s, (dx, lift) in zip((-1, 1), feet or []):
                x = cx + s * 8.5 * bk + dx
                y = (by - ry * 0.2 if belly else bottom + 1.5) - lift
                toes = [L([(x + d * fk, y - 1.0), (x + d * fk, y - 3.4 * fk)], 0.9) for d in (-2, 2)]
                feet_parts.append(Part([E(x - 6.5 * fk, y - 6 * fk, x + 6.5 * fk, y)], col, [(toes, mix(col, SQ_LINE, 0.4))], grow=0, outline=True))
            if not belly:
                parts += feet_parts
            parts.append(Part([E(cx - rx, by - ry, cx + rx, by + ry)], col,
                              [([E(cx - rx * 0.72, by - ry * 0.8, cx + rx * 0.72, by + ry * 1.04)], CREAM)], grow=1, outline=True))
            if belly:
                parts += feet_parts
            hp, face = front_head(hx, hy, hr, eyes, mouth, brow, head_rot, cheeks)
            parts += hp
            ops += face
            if hold is not None:
                parts.append(morsel(*hold, k=fk))
            parts += [h() for h in held]
            for s, a in zip((-1, 1), arms):
                sh = (cx + s * rx * 0.6, by - ry * 0.6)
                if a == "chest":
                    tip = (cx + s * 4.6 * fk, by - ry * 0.32)
                elif isinstance(a, tuple):
                    tip = a if s < 0 else a
                else:
                    r = math.radians(a)
                    ln = 13 * bk
                    tip = (sh[0] + s * math.sin(r) * ln, sh[1] + math.cos(r) * ln)
                pr = 4.4 * fk
                parts.append(Part([strand([sh, ((sh[0] + tip[0]) / 2, (sh[1] + tip[1]) / 2), tip], 7.6 * fk, 6.0 * fk), C(tip[0], tip[1], pr)], col,
                                  [([C(tip[0], tip[1], pr)], PAW)], grow=0, outline=True))
    return dp.compose(parts, ops, list(fx))


# ---- 뒷모습 (벽 타기, 고양이처럼) — 앞발 끝은 머리 옆·위로 빼꼼, 뒷발은 몸 뒤에서 빼꼼, 꼬리가 몸 옆으로 솟아 살랑 ----
SQ_BACK_FOOT = {"side": (1.0, 0.55), "low": (0.5, 1.02), "tuck": (0.3, 0.3)}  # 몸 (rx, ry) 기준 발 자리


def back(reach, feet, *, sq=(1.0, 1.0), dy=0.0, tail_rot=0.0, scratch=None, sweat=None):
    """뒷모습. reach: (왼, 오) 0 머리 옆 낮게 … 1 머리 위로 쭉 · feet: (왼, 오) SQ_BACK_FOOT 키."""
    st = _sq()
    bk, hk = _ST["body"], _ST["head"]
    fk = bk ** 0.5
    rx, ry = 18 * bk * _ST["fbody"][0], 19 * bk * _ST["fbody"][1]
    cx = 56.0
    bottom = FLOOR - 2 + dy
    by = bottom - ry
    hr = 22 * hk
    hx, hy = cx, by - ry * 0.62 - hr * 0.5
    col = fur_color()
    parts, ops = [], []
    with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
        for s, at in zip((-1, 1), feet):  # 뒷발은 몸 뒤 — 몸 밖으로 나온 만큼만
            ox, oy = SQ_BACK_FOOT[at]
            x, y = cx + s * rx * ox, by + ry * oy
            parts.append(Part([E(x - 6.5 * fk, y - 5.5 * fk, x + 6.5 * fk, y + 5.5 * fk)], col, grow=0, outline=True))
        for s, r in zip((-1, 1), reach):  # 앞발은 머리 뒤에 숨고 발끝만 빼꼼
            tx = cx + s * hr * (1.02 - 0.18 * r)
            ty = hy - hr * (0.2 + 0.95 * r)
            pr = 5.4 * fk
            parts.append(Part([L([(cx + s * hr * 0.55, hy + hr * 0.55), (tx, ty)], 7.5 * fk), C(tx, ty, pr)], col, [([C(tx, ty, pr)], PAW)],
                              grow=0, outline=True))
            if scratch is not None:
                for j, d in enumerate((-3, 0, 3)):
                    y0 = ty - 7 - (j % 2) * 1.5 - scratch * 2
                    ops.append(([L([(tx + d * 1.1, y0), (tx + d * 1.3, y0 - 6)], 1.4)], SQ_LINE))
        parts.append(Part([E(cx - rx, by - ry, cx + rx, by + ry)], col, [([E(cx - rx * 0.5, by - ry * 1.1, cx + rx * 0.5, by - ry * 0.5)], mix(col, SQ_LINE, 0.08))],
                          grow=1, outline=True))
        for s in (-1, 1):
            parts.append(ear(hx + s * hr * 0.6, hy - hr * 0.72, hr / 23, rot=s * 16, tuft=st["tuft"], inner=False))
        parts.append(Part([C(hx, hy, hr)], col, grow=1, outline=True, spikes=dp.tuft(hx - 1, hy - hr + 1) if st["fuzz"] else []))
        dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
        if st["hat"]:
            parts.append(acorn_cap(hx - hr * 0.18, hy - hr * 0.9, hr, -14))
        parts.append(tail_part((cx + rx * 0.72, by + ry * 0.6), "back", tail_rot))
    fx = dp.sweat_fx(hx - hr - 8, hy - 4 + sweat) if sweat is not None else []
    return dp.compose(parts, ops, fx)


def acorn_bubble_fx(cx, cy):
    """생각 풍선 안에 도토리 한 알 (깍정이 모자 + 꼭지 + 반짝)."""
    fx = dp._bubble(cx, cy)
    nut = [E(cx - 5.2, cy - 3.6, cx + 5.2, cy + 7.0), T((cx - 2, cy + 6), (cx + 2, cy + 6), (cx, cy + 8.6))]
    fx += [(nut, ACORN, 0), ([E(cx - 3.6, cy - 1.6, cx - 1.2, cy + 2.6)], mix(ACORN, WHITE, 0.45), 0)]
    fx += [([E(cx - 6.4, cy - 6.6, cx + 6.4, cy - 0.6)], ACORN_DARK, 0), ([L([(cx, cy - 6.4), (cx + 1.4, cy - 9.4)], 1.6)], CAP_STEM, 0)]
    return fx


# =====================================================================
# 포즈
# =====================================================================
WALK_LEGS = [[(-3, 0), (3, 1.6), (3, 1.6), (-3, 0)], [(0, 0), (0, 0), (0, 0), (0, 0)],
             [(3, 1.6), (-3, 0), (-3, 0), (3, 1.6)], [(0, 0), (0, 0), (0, 0), (0, 0)]]


def sq_walk(sp, i):
    """총총: 몸이 통통 튀고 다리가 번갈아, 꼬리 끝이 살랑."""
    return side(bottom=FLOOR - 1.5 - [0, 1.5, 0, 1.5][i], legs=WALK_LEGS[i], tail_rot=[-3, 0, 3, 0][i], head=(0, [0, -0.6, 0, -0.6][i]))


def sq_run(sp, i):
    """깡충 질주: 쭉 뻗기 → 떠서 모으기 → 모아 디딤 → 떠서. 꼬리는 뒤로 흩날림, 속도선."""
    legs = [[(-9, 1), (9, 2), (-8, 0), (10, 1)], [(-4, 3), (4, 3), (-3, 3), (5, 3)], [(7, 0), (-5, 0), (8, 0), (-6, 0)], [(2, 3), (-2, 3), (3, 3), (-1, 3)]][i]
    return side(cx=68, bottom=FLOOR - 1.5 - [0, 4, 0, 3][i], rot=[-5, 0, 5, 0][i], legs=legs, eyes="happy", mouth="open" if i % 2 else None,
                tail="flow", tail_rot=[0, -6, 6, -3][i], fx=dp.speed_fx(4, (104, 112, 120), 10, [0, -3, 0, -3][i]))


def sq_idle(sp, i):
    return side(sq=(1.0, 1.0 + 0.02 * i), head=(0, -0.8 * i), tail_rot=[0, -3][i])


def sq_look(sp, i):
    return side(head_rot=[-10, 0, 12, 0][i], eyes="closed" if i == 3 else "dot", tail_rot=[0, 2, 4, 2][i])


def sq_lookup(sp, i):
    """고개를 젖혀 위를 본다 (호기심 ✧)."""
    return side(head=(-1, -1), head_rot=dp.lookup_tilt(i), eyes="up", tail_rot=[0, 3, 5][i], fx=dp.lookup_fx(i, 114, 14))


def sq_sleep(sp, i):
    """엎드려 동글 웅크리고 꼬리를 이불처럼 덮고 쿨쿨."""
    bk, hk = _ST["body"], _ST["head"]
    rx, ry = 21 * bk, 11 * bk * (1 + 0.015 * i)
    bottom = FLOOR - 1
    bx, by = 54.0, bottom - ry * 0.95
    hr = 19.5 * hk
    hx, hy = bx + rx * 1.05, bottom - hr * 0.8
    col = fur_color()
    parts = [Part([E(bx - rx * 1.2, by - ry, bx + rx * 1.1, by + ry * 0.95)], col, grow=1, outline=True)]
    k = _sq()["tail"] * bk ** 0.3
    ch = [(bx - rx * 1.0 + x * k, by + ry * 0.4 + y * k, r * k) for x, y, r in
          _chain([(0, 0, 8), (-5, -12, 11), (1, -23, 13), (13, -28, 13.5), (26, -27, 12.5), (37, -21, 10), (43, -13, 7)])]
    parts.append(plume(ch, False, k, _sq()["fluff"]))
    parts.append(Part([E(hx - 5, bottom - 6, hx + 11, bottom)], col, [([E(hx - 2, bottom - 5, hx + 11, bottom)], PAW)], grow=0, outline=True))
    with xf(sy=0.9, pivot=(hx, bottom)):
        hp, face = side_head(hx, hy, hr, "closed", None, None, 10)
    parts += hp
    u = (hk - 1) * 40
    zs = [dp.z_fx(104, 62 - u, 3.4), dp.z_fx(115, 51 - u, 2.6)] if i == 0 else [dp.z_fx(106, 59 - u, 3.6), dp.z_fx(118, 47 - u, 2.8)]
    return dp.compose(parts, face, zs)


def sq_roll(sp, i):
    """꼬리로 몸을 감싼 동글 공 (앱이 돌린다): 웅크린 몸 + 꼬리가 등을 휘감고, 앞에 잠든 얼굴과 귀가 빼꼼."""
    cx, cy = 64.0, 88.0
    k = _ST["body"] ** 0.3
    col = fur_color()
    with xf(rot=[0, 40][i], pivot=(cx, cy)):
        parts = [Part([C(cx, cy, 23 * k)], col, [([E(cx - 4 * k, cy + 2 * k, cx + 20 * k, cy + 22 * k)], CREAM)], grow=1, outline=True)]
        ch = [(cx + 17 * k * math.cos(math.radians(a)), cy + 17 * k * math.sin(math.radians(a)), r * k) for a, r in
              zip(range(100, 341, 12), [7, 8, 9, 10, 10.5, 11, 11, 11, 11, 11, 10.5, 10, 9.5, 9, 8.5, 8, 7.5, 7, 6.5, 6, 5.5])]
        parts.append(plume(ch, False, k * 0.8, 0.7))
        hx, hy, hr = cx + 8 * k, cy + 6 * k, 13 * k
        parts.append(ear(hx + 2 * k, hy - hr * 0.8, k * 0.62, rot=10, tuft=_sq()["tuft"] * 0.6))
        parts.append(Part([C(hx, hy, hr)], col, [([E(hx - hr * 0.6, hy + hr * 0.1, hx + hr * 0.9, hy + hr * 1.1)], CREAM)], grow=1, outline=True))
        dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
        face = []
        for ex in (hx - hr * 0.34, hx + hr * 0.38):
            face += dp.stage_eye("closed", ex, hy - 0.5)
        face += [([C(hx + 1, hy + hr * 0.36, 1.8)], NOSE)]
        return dp.compose(parts, face)


def sq_annoyed(sp, i):
    """흥! 꼬리를 부풀리고 웅크림."""
    return side(crouch=2, sq=(1.04, 0.96), eyes="squint", mouth="frown", head=(0, 2), tail_puff=1.12, tail_rot=[-4, 4][i],
                fx=[dp.anger_fx(106, 34 - i, 7.0)])


def sq_hop(sp, i):
    """깡충: 웅크림 → 쭉 뻗어 솟구침 (꼬리 흩날림) → 다리 모으고 떠서 ^^."""
    if i == 0:
        return side(crouch=4, sq=(1.06, 0.92), eyes="tight", head=(0, 3), legs=[(2, 0), (-2, 0), (3, 0), (-2, 0)])
    if i == 1:
        return side(bottom=FLOOR - 16, rot=-10, eyes="happy", mouth="open", legs=[(-9, -2), (8, 2), (-10, -2), (9, 2)], tail="flow", tail_rot=8)
    return side(bottom=FLOOR - 8, eyes="happy", legs=[(4, 2), (-3, 2), (5, 2), (-3, 2)], tail_rot=-6)


def sq_fall(sp, i):
    stars = dp._stars([(104, 22, 3.6), (24, 70, 3.0)] if i == 0 else [(28, 64, 3.6), (108, 32, 3.0)])
    return side(bottom=FLOOR - 8, rot=[-12, 12][i], eyes="spiral", mouth="open", legs=[(-3, 4), (4, 6), (-4, 5), (5, 7)], tail_rot=[-14, 10][i],
                fx=stars)


def sq_dragged(sp, i):
    """잡혀서 버둥: 다리 동동, > < 눈, 땀, 꼬리 축."""
    s = [-1, 1][i]
    return side(bottom=FLOOR - 8, rot=5 * s, eyes="tight", mouth="open", legs=[(-2 + 2 * s, 3), (2 - 2 * s, 5), (-1 + 2 * s, 4), (1 - 2 * s, 6)],
                tail_rot=20 + 6 * s, fx=dp.sweat_fx(108, 32))


def sq_land(sp, i):
    if i == 0:
        return side(sq=(1.1, 0.86), eyes="tight", legs=[(-2, 0), (2, 0), (-3, 0), (3, 0)], tail_rot=6)
    return side(sq=(1.02, 0.98), tail_rot=-3)


def sq_sit_front(sp, i):
    return front(sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot", tail_rot=[0, -2, -4, -2][i])


def sq_wave(sp, i):
    return front(arms=("chest", [130, 160, 130, 160][i]), rot=[-2, -1, -2, -1][i], mouth="chirp" if i % 2 else None, tail_rot=[0, 3, 0, 3][i])


def sq_tilt(sp, i):
    return front(head_rot=dp._swing([8, 12, 12][i]), tail_rot=[0, 2, 2][i], fx=[dp.question_fx(18, 22 + [2, 0, -2][i])])


def sq_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return front(belly=True, bottom=FLOOR - 1, sq=(1.1, 0.84), rot=-6, eyes="happy", mouth="open", feet=((-1 + wig, 0), (1 - wig, wig)),
                 arms=(110 + 6 * wig, 110 - 6 * wig), tail_rot=-8 + 3 * wig)


def sq_stretch(sp, i):
    if i == 0:
        return front(sq=(0.94, 1.08), eyes="tight", arms=(165, 165), head=(0, -3))
    if i == 1:
        return front(eyes="tight", mouth="yawn", arms=(100, 100), rot=-3)
    return front()


def sq_heart(sp, i):
    hs = dp.heart_burst(i)
    by = FLOOR - 2.5 - 18.5 * _ST["body"] * _ST["fbody"][1]
    held = [lambda: dp.held_heart(by - 4)]
    return front(eyes="happy", held=held, fx=hs, tail_rot=[0, -3, -5][i])


def sq_tickle(sp, i):
    return front(rot=dp._swing([-7, 7, 0][i]), eyes="happy", mouth="chirp" if i < 2 else "open", arms=([60, 130, 95][i], [130, 60, 95][i]),
                 tail_rot=[-6, 6, 0][i])


def sq_hungry(sp, i):
    return front(rot=-4, eyes="closed", brow="sad", head=(0, [0, 1, 0][i] + 2), arms=("chest", "chest"), tail_rot=-6,
                 fx=acorn_bubble_fx(106, 18))


def morsel_fx(x, y, k=1.0, paws=None):
    """두 앞발에 쥔 한 알 (모자 없는 도토리 알) — 얼굴 위에 오도록 효과 층으로 그린다. paws: 알 양옆을 감싼 앞발 끝 [(x, y)]."""
    nut = lambda g: [E(x - 4.8 * k - g, y - 5.6 * k - g, x + 4.8 * k + g, y + 5.2 * k + g), T((x - 2.2 * k - g, y + 4 * k), (x + 2.2 * k + g, y + 4 * k), (x, y + 7.4 * k + g))]
    fx = [(nut(1.1), SQ_LINE, 0), (nut(0), MORSEL, 0), ([E(x - 3.2 * k, y - 4.2 * k, x - 0.8 * k, y - 0.4 * k)], MORSEL_LIGHT, 0),
          ([E(x - 4.8 * k, y - 5.6 * k, x + 4.8 * k, y - 3.4 * k)], MORSEL_DARK, 0)]
    for px_, py_ in paws or []:
        fx += [([C(px_, py_, 4.4 * k + 1.1)], SQ_LINE, 0), ([C(px_, py_, 4.4 * k)], PAW, 0)]
    return fx


def sq_eat(sp, i):
    """0 = 고개를 숙이고 두 앞발을 그릇 밥 더미에 넣어 한 알 집기 · 1·2·3 = 고개를 들고 두 앞발로 입 아래에 쥐고 오물오물
    (볼 빵빵, 앞니, 부스러기). 알과 앞발 끝은 얼굴 위에 보이도록 맨 위 층."""
    bk = _ST["body"]
    fk = bk ** 0.5
    hk = _ST["head"]
    hr = 23 * hk
    k = 1.15 * fk
    ry = 18.5 * bk * _ST["fbody"][1]
    by = FLOOR - 2.5 - ry
    up = -8.0
    hy = by - ry * 0.62 - hr * 0.55 + up
    my = hy + hr * 0.3 + 2.0 * (hr / 23) + 9.5 * fk  # 앞니 바로 아래
    if i == 0:  # 그릇 속: 쥐고 있을 때보다 한 뼘 아래 (큰 머리 아기도)
        up_f = 126 + (my - 126) * dp._VIEW["scale"] - dp._VIEW["lift"]
        gx, gy = dp._frame_to_pre(64, min(112, max(101, up_f + 11)))
        paws = [(gx - 7.6 * k, gy), (gx + 7.6 * k, gy)]
        return front(sq=(1.02, 0.96), head=(0, 6), eyes="happy", mouth="open", arms=tuple(paws), tail_rot=6,
                     fx=dp.eat_crumbs(0, MORSEL_LIGHT) + morsel_fx(gx, gy, k, paws))
    wob = [0, 0, 1.0, -1.0][i]
    paws = [(64 - 7.6 * k + wob, my + 1.5), (64 + 7.6 * k + wob, my + 1.5)]
    return front(head=(0, up), head_rot=[0, 0, -3, 3][i], eyes="closed", mouth="chew", cheeks=True, arms=tuple(paws),
                 tail_rot=[0, 0, -3, 3][i], fx=dp.eat_crumbs(i, MORSEL_LIGHT) + morsel_fx(64 + wob, my, k, paws))


def sq_level_up(sp, i):
    stars = dp.level_up_stars(i)
    up = [150, 165, 175][i]
    return front(bottom=FLOOR - 2.5 - [0, 1, 3][i], eyes="happy", mouth="big", arms=(up, up), tail_rot=[-4, 0, 4][i], fx=stars)


def sq_climb(sp, i):
    """화면을 기어오름 (뒷모습): 앞발 번갈아 위로, 뒷발은 앞발과 엇갈려 빼꼼, 꼬리 살랑."""
    reach, feet = dp.CLIMB_REACH[i], dp.CLIMB_FEET[i]
    return back(reach, feet, sq=(1.0, [1.04, 1.0, 1.04, 1.0][i]), dy=[-1, 0, -1, 0][i], tail_rot=[-10, 0, 10, 0][i])


def sq_slide(sp, i):
    """주르륵: 두 앞발 쭉 위로 + 긁힌 자국, 살짝 찌그러짐, 땀 한 방울, 꼬리가 위로 펄럭."""
    return back((1.0, 1.0), ("low", "low"), sq=(1.03, 0.94), dy=[0, 1][i], tail_rot=[-20, -14][i], scratch=i, sweat=i)


SQUIRREL_POSES = {"walk": sq_walk, "run": sq_run, "roll": sq_roll, "idle": sq_idle, "look": sq_look, "sleep": sq_sleep, "hop": sq_hop,
                  "fall": sq_fall, "dragged": sq_dragged, "land": sq_land, "sit_front": sq_sit_front, "wave": sq_wave, "tilt": sq_tilt,
                  "belly": sq_belly, "stretch": sq_stretch, "heart": sq_heart, "tickle": sq_tickle, "annoyed": sq_annoyed,
                  "hungry": sq_hungry, "eat": sq_eat, "level_up": sq_level_up, "lookup": sq_lookup, "climb": sq_climb, "slide": sq_slide}
RAW_POSES = dict(SQUIRREL_POSES)
SQUIRREL_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in SQUIRREL_POSES.items()}


SQUIRREL = Species(
    id="squirrel", name="다람쥐", walk_speed=100, scale=0.94, base=FUR, back=FUR, belly=CREAM, nose=NOSE, mouth="w",
    front_tail_color=TAIL, tail_color=TAIL, stage_size=dict.fromkeys(STAGES, 0.82), stripe=TAIL_LIGHT, outline=SQ_LINE,
    # 털 색은 단계 표(fur_color)가 정한다 — 공통 파스텔은 끈다. 외곽선도 모든 단계 같은 짙은 밤색
    stage_tweak={"baby": dict(pale=0.0, soft=0.0, fit=0.88), "kinder": dict(pale=0.0, soft=0.0, fit=0.92),
                 "child": dict(pale=0.0), "adult": dict(fit=0.93)},
    specials=["climb", "slide", "lookup"], poses=SQUIRREL_POSES, **dp.NO_COMMON_PARTS,
)


def register():
    dp.SPECIES[SQUIRREL.id] = SQUIRREL


register()


def main():
    import draw_halloween as dh
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(SQUIRREL, root / "ui" / "pets" / SQUIRREL.id))
    dh.preview_species(by_stage, pv / "stages_squirrel.png")
    print("ok: squirrel")


if __name__ == "__main__":
    main()
