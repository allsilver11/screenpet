"""펭귄 🐧 — 뒤뚱뒤뚱 걷고 배로 쭉 미끄러지는 꼬마 펭귄(penguin). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_penguin.py → ui/pets/penguin/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_penguin.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

펭귄: 동글 몸통에 머리가 목 없이 붙은 콩 모양 + 짧은 지느러미 날개(팔) + 주황 작은 부리 + 주황 납작 발 + 짧은 꽁지.
      등·머리는 짙은 남색 검정, 배와 얼굴(하트 모양 가면)은 흰색. 이름은 "펭귄".
      단계: 아기 = 회색 솜털 병아리 (정수리 솜털 세 가닥, 몸 옆 솜털 삐죽, 검은 모자 + 흰 얼굴) → 유아 회색, 솜털 하나
            → 어린이 검은 등·흰 배, 정수리에 회색 솜털 한 가닥 남음 → 청소년 매끈 → 어른 빨간 나비넥타이.
걷기 = 뒤뚱뒤뚱 (몸이 좌우로 기우뚱, 발이 번갈아, 지느러미를 살짝 벌려 균형). 구르기(roll) 대신 배로 쭉 미끄러진다 —
      pet.json roll에 spin이 없다 (눈사람처럼 앱이 돌리지 않는다): 엎드려 머리만 들고, 지느러미는 뒤로, 발은 동동, 앞으로 얼음 가루.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 앞모습으로 고개를 숙여 부리 끝을 그릇 밥 더미(프레임 (64, 84))에 콕 → 고개 들고 냠.
"""

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, L, T, WHITE, Part, Species, _ST, arc, mix, strand, view, xf)  # noqa: E402
from draw_pets import _fit  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

FLOOR = 125.0

PEN_LINE = (40, 42, 60)  # 외곽선 (짙은 남색 — 검은 등보다 더 짙게)
BACK = {2: (74, 78, 104), 3: (68, 72, 98), 4: (64, 68, 94)}  # 등·머리 (어린이부터)
CHICK_GREY = (186, 190, 204)  # 아기·유아 솜털 몸
CHICK_GREY_K = (168, 173, 190)  # 유아 (조금 짙은 회색)
CHICK_CAP = (92, 96, 120)  # 아기·유아 검은 모자
WHITE_BELLY = (255, 254, 250)
BEAK, BEAK_DARK, BEAK_LIGHT = (255, 158, 66), (226, 118, 46), (255, 204, 140)
FEET = (255, 170, 84)
BOWTIE, BOWTIE_DARK, BOWTIE_LIGHT = (232, 76, 92), (186, 50, 70), (255, 160, 170)
ICE, ICE_DARK = (214, 238, 252), (150, 196, 230)
# 동글 안경 — LENS는 렌즈에만 쓰는 전용 색 (앱의 "안경 번쩍" SVG 필터가 이 색만 골라 하얗게 번쩍인다: ui/index.html #lens-flash)
LENS = (196, 222, 250)
GLASS_FRAME = (66, 44, 50)  # 가는 짙은 코코아색 테 (남색 모자와 섞여도 등 색이 되지 않게 따뜻한 빛)
GLASS_ARM = (156, 122, 120)  # 짙은 머리 위를 지나는 다리 (어두운 모자 위에서도 보이게 조금 밝게)
# 안경 모양 — r: 렌즈 반지름 / 머리 반지름 · dx: 렌즈 가운데 간격 / 머리 반지름 · arms: 앞모습 다리 · glint: 반짝 "streak" 사선 | "dot" 한 칸
# (도트 그림은 draw_pixel이 이 표를 바꿔 끼운다: 칸이 적어 렌즈를 조금 키우고 반짝은 한 칸 점)
#   alpha_key: 동글 그림은 렌즈 둥근 자리(눈·반짝 포함)의 알파를 LENS_ALPHA로 표시한다 — 앱의 "안경 번쩍" 필터(ui/index.html #lens-flash)가
#   이 알파만 골라 하얗게 덮는다 (축소·보간돼도 렌즈 안쪽은 그대로 254). 도트 그림은 반투명이 없어야 해서 끄고, 렌즈 색 그대로 고른다 (#lens-flash-px)
GLASS = dict(r=0.43, dx=0.46, arms=True, glint="streak", alpha_key=True)
LENS_ALPHA = 254
_LENS_SHAPES = []  # 지금 그리는 프레임의 렌즈 원들 (그린 변환 그대로) — lens_key가 알파 표시에 쓴다
SHRIMP, SHRIMP_DARK = (255, 140, 110), (222, 96, 80)

# 단계(lvl)별 — chick: 회색 솜털 몸 · fluff: 정수리 솜털 가닥 · fuzz: 몸 옆 솜털 삐죽 · flip: 지느러미 길이 · tie: 나비넥타이
#               chub: 동글 공 몸 (가로, 세로) 배율 — 공통 torso(아기 길쭉)를 쓰지 않고 모든 단계 포동포동 (머리 큰 아기는 몸을 더 넓게)
PG_STAGE = {
    0: dict(chick=True, fluff=3, fuzz=True, flip=0.78, tie=False, chub=(1.3, 1.0)),
    1: dict(chick=True, fluff=1, fuzz=False, flip=0.86, tie=False, chub=(1.2, 1.02)),  # 등 솜털 세모는 아기만 (사용자: 등이 뾰족뾰족)
    2: dict(chick=False, fluff=1, fuzz=False, flip=0.94, tie=False, chub=(1.17, 0.97)),
    3: dict(chick=False, fluff=0, fuzz=False, flip=1.0, tie=False, chub=(1.0, 1.0)),
    4: dict(chick=False, fluff=0, fuzz=False, flip=1.04, tie=True, chub=(0.98, 0.97)),
}


def _pg():
    return PG_STAGE[_ST["lvl"]]


def body_dims(side=True):
    """포동포동 몸 (가로 반지름, 세로 반지름, 머리 반지름): 키보다 넓은 동글 공 몸에 머리가 목 없이 파묻힌다."""
    cx_, cy_ = _pg()["chub"]
    bk = _ST["body"]
    return (27.0 if side else 28.5) * bk * cx_, 21.5 * bk * cy_, (16.5 if side else 18.0) * _ST["head"]


def front_head_y(by, ry, hr):
    """앞모습 머리 가운데 높이 (몸 위쪽에 반쯤 파묻힘)."""
    return by - ry * 0.58 - hr * 0.34


def back_color(stage=None):
    """등(몸) 색. stage: 단계 이름 (시험용, 없으면 지금 단계). 아기·유아는 회색 솜털."""
    lvl = _ST["lvl"] if stage is None else STAGES.index(stage)
    if lvl == 0:
        return CHICK_GREY
    if lvl == 1:
        return CHICK_GREY_K
    return BACK[lvl]


def cap_color():
    """머리 (아기·유아는 검은 모자, 그 뒤로는 등과 같은 색)."""
    return CHICK_CAP if _pg()["chick"] else back_color()


def belly_color(stage=None):
    """배: 아기는 연회색 솜털 배, 유아는 더 흰 배, 어린이부터 새하얀 배. stage: 단계 이름 (시험용, 없으면 지금 단계)."""
    lvl = _ST["lvl"] if stage is None else STAGES.index(stage)
    return {0: mix(CHICK_GREY, WHITE, 0.55), 1: mix(CHICK_GREY_K, WHITE, 0.7)}.get(lvl, WHITE_BELLY)


def flipper_color():
    return mix(back_color(), PEN_LINE, 0.12)


def _dir(a, side, s=1):
    """지느러미 방향 단위 벡터. 옆모습: a 0 = 아래, 90 = 뒤(-x)로 수평, 150 = 뒤로 위. 앞모습: s쪽 바깥으로 (0 아래, 90 옆, 170 위)."""
    r = math.radians(a)
    if side:
        return (-math.sin(r), math.cos(r))
    return (s * math.sin(r), math.cos(r))


def flipper(root, ln, a, side=True, s=1, k=1.0):
    """짧은 지느러미 날개 한 짝 (뿌리 root에서 ln만큼, 끝이 동글게 가늘어짐)."""
    dx, dy = _dir(a, side, s)
    # 살짝 휜 지느러미: 가운데를 몸 바깥쪽으로 조금
    nx, ny = (-dy, dx) if side else (dy * s, -dx * s)
    bend = 1.6 * k
    pts = [root, (root[0] + dx * ln * 0.5 + nx * bend, root[1] + dy * ln * 0.5 + ny * bend), (root[0] + dx * ln, root[1] + dy * ln)]
    col = flipper_color()
    tip = (root[0] + dx * ln * 0.92, root[1] + dy * ln * 0.92)
    return Part([strand(pts, 10.5 * k, 5.0 * k), C(tip[0], tip[1], 2.6 * k)], col, grow=0, outline=True)


def side_foot(x, lift=0.0, k=1.0, y=None):
    """옆모습 납작 발 (뒤꿈치 x, 앞으로 뾰족). y: 발 아래끝 (없으면 바닥)."""
    y = (FLOOR - 0.5 if y is None else y) + lift
    return Part([E(x - 4 * k, y - 5.2 * k, x + 11.5 * k, y)], FEET, [([E(x - 1 * k, y - 4.4 * k, x + 6 * k, y - 2.8 * k)], mix(FEET, WHITE, 0.4))],
                grow=0, outline=True)


def front_foot(x, s, lift=0.0, k=1.0, y=None):
    """앞모습 발 (바깥쪽으로 살짝 벌림). y: 발 아래끝 (없으면 바닥)."""
    y = (FLOOR - 0.5 if y is None else y) + lift
    with xf(rot=s * 14, pivot=(x, y)):
        return Part([E(x - 7.5 * k, y - 6 * k, x + 7.5 * k, y)], FEET, [([L([(x - 1.8 * k, y - 1.2), (x - 2.4 * k, y - 4.2 * k)], 0.8),
                                                                         L([(x + 1.8 * k, y - 1.2), (x + 2.4 * k, y - 4.2 * k)], 0.8)], mix(FEET, PEN_LINE, 0.3))],
                    grow=0, outline=True)


def fluff(cx, top, n):
    """정수리 솜털 (아기 세 가닥, 유아·어린이 한 가닥)."""
    if n == 0:
        return []
    pts = [[(cx - 1, top + 4)] + arc(cx + 5, top - 4, 5, 6, 180, 300, 8)]
    if n >= 3:
        pts += [[(cx - 3, top + 4), (cx - 6, top - 3), (cx - 10, top - 4)], [(cx + 2, top + 4), (cx + 7, top - 2), (cx + 11, top)]]
    return [strand(p, 4.2, 1.1) for p in pts]


def fuzz(cx, cy, rx, ry, angles):
    """몸 가장자리 솜털 삐죽 (작은 세모들) — angles: 도 (0 = 오른쪽, 90 = 아래)."""
    out = []
    for a in angles:
        r = math.radians(a)
        x, y = cx + rx * math.cos(r), cy + ry * math.sin(r)
        ox, oy = math.cos(r), math.sin(r)
        px_, py_ = -oy, ox
        out.append(T((x - px_ * 3.2 - ox * 2, y - py_ * 3.2 - oy * 2), (x + ox * 4.2, y + oy * 4.2), (x + px_ * 3.2 - ox * 2, y + py_ * 3.2 - oy * 2)))
    return out


def lens_glint(cx, cy, lr):
    """렌즈 왼쪽 위의 하얀 사선 반짝 (눈보다 먼저 그려 눈을 가리지 않는다)."""
    if GLASS["glint"] == "dot":
        return ([C(cx - lr * 0.55, cy - lr * 0.55, 0.4)], WHITE)
    return ([L([(cx - lr * 0.7, cy - lr * 0.02), (cx - lr * 0.06, cy - lr * 0.66)], max(1.1, lr * 0.2))], WHITE)


def glasses_front(hx, ey, hr):
    """앞모습 큰 동글 안경: (머리 Part에 얹을 렌즈 marks, 눈 앞에 그릴 반짝 ops, 눈 뒤에 그릴 테·다리 ops)."""
    lr, d = hr * GLASS["r"], hr * GLASS["dx"]
    xs = (hx - d, hx + d)
    discs = [C(x, ey, lr) for x in xs]
    _LENS_SHAPES.extend(discs)
    marks = [(discs, LENS)]
    glint = [lens_glint(x, ey, lr) for x in xs]
    w = max(1.2, hr * 0.085)
    rims = [L(arc(x, ey, lr, lr, 0, 360, 28), w) for x in xs]
    bridge = L(arc(hx, ey - lr * 0.1, hr * 0.08, lr * 0.3, 200, 340, 6), w)
    arms = [L([(hx + s * (d + lr), ey - lr * 0.12), (hx + s * hr * 0.97, ey - lr * 0.4)], w) for s in (-1, 1)] if GLASS["arms"] else []
    return marks, glint, [(rims + [bridge] + arms, GLASS_FRAME)]


def glasses_side(ex, ey, hx, hr):
    """옆모습 안경: 렌즈 하나 + 머리 뒤로 넘어가는 다리. (렌즈 marks, 반짝 ops, 테 ops)."""
    lr = hr * GLASS["r"] * 1.08
    disc = C(ex, ey, lr)
    _LENS_SHAPES.append(disc)
    marks = [([disc], LENS)]
    w = max(1.2, hr * 0.085)
    arm = L([(ex - lr * 0.96, ey - lr * 0.2), (hx - hr * 0.1, ey - lr * 0.5), (hx - hr * 0.62, ey - lr * 0.2)], w)
    return marks, [lens_glint(ex, ey, lr)], [([arm], GLASS_ARM), ([L(arc(ex, ey, lr, lr, 0, 360, 28), w)], GLASS_FRAME)]


def beak(bx, by, kind, side, k=1.0):
    """부리 Part들 + 입 ops. 옆모습: (bx, by) = 부리 뿌리 (오른쪽으로 짧고 뾰족) · 앞모습: 부리 뿌리 가운데 (아래로 콕)."""
    parts, ops = [], []
    gap = {"open": 3.0, "big": 4.5, "yawn": 5.0, "chirp": 3.2}.get(kind, 0.0)
    if side:
        ln = 10.0 * k
        if gap:
            up = [T((bx, by - 3.0 * k), (bx + ln, by - 0.4), (bx, by + 0.8))]
            lo = [T((bx, by + 0.8), (bx + ln * 0.8, by + gap), (bx, by + 3.0 * k + gap * 0.4))]
            ops.append(([T((bx, by), (bx + ln * 0.55, by + gap * 0.6), (bx, by + gap * 0.6))], dp.MOUTH_IN))
            parts.append(Part(lo, BEAK_DARK, grow=0, outline=True))
        else:
            up = [T((bx, by - 3.0 * k), (bx + ln, by + 0.6), (bx, by + 3.4 * k))]
        parts.append(Part(up, BEAK, [([E(bx + 1, by - 2.0 * k, bx + 4.5, by - 0.6 * k)], BEAK_LIGHT)], grow=0, outline=True))
        return parts, ops
    w, ln = 4.6 * k, 7.0 * k
    if gap:
        ops.append(([E(bx - w * 0.8, by + 1, bx + w * 0.8, by + 2 + gap)], dp.MOUTH_IN))
        lo = [T((bx - w * 0.75, by + gap * 0.6 + 1), (bx + w * 0.75, by + gap * 0.6 + 1), (bx, by + ln * 0.7 + gap))]
        parts.append(Part(lo, BEAK_DARK, grow=0, outline=True))
        up = [T((bx - w, by), (bx + w, by), (bx, by + ln * 0.62))]
    else:
        up = [T((bx - w, by), (bx + w, by), (bx, by + ln))]
    parts.append(Part(up, BEAK, [([E(bx - w * 0.55, by + 0.5, bx - w * 0.05, by + 2.4)], BEAK_LIGHT)], grow=0, outline=True))
    if kind == "chew":
        ops.append(([L([(bx - 2.5, by + ln + 2.5), (bx, by + ln + 1.2), (bx + 2.5, by + ln + 2.5)], 1.1)], PEN_LINE))
    return parts, ops


def bow_tie(x, y, side, k=1.0):
    """어른: 목(턱 아래)에 빨간 나비넥타이. 옆모습은 앞쪽에 반만 보이는 작은 리본."""
    if side:
        wings = [T((x, y), (x + 6.5 * k, y - 4.5 * k), (x + 6.5 * k, y + 4.5 * k)), T((x, y), (x - 4 * k, y - 4 * k), (x - 4 * k, y + 4 * k))]
        return Part(wings + [C(x + 0.5, y, 2.4 * k)], BOWTIE, [([C(x + 4.5 * k, y - 1.5 * k, 1.1 * k)], BOWTIE_LIGHT)], grow=0, outline=True)
    wings = [T((x, y), (x - 9 * k, y - 5.5 * k), (x - 9 * k, y + 5.5 * k)), T((x, y), (x + 9 * k, y - 5.5 * k), (x + 9 * k, y + 5.5 * k))]
    marks = [([C(x - 6 * k, y - 2 * k, 1.3 * k), C(x + 6 * k, y - 2 * k, 1.3 * k)], BOWTIE_LIGHT),
             ([C(x, y, 2.9 * k)], BOWTIE_DARK)]
    return Part(wings + [C(x, y, 3.0 * k)], BOWTIE, marks, grow=0, outline=True)


def side_head(hx, hy, hr, eyes, mouth, brow, head_rot=0.0):
    """옆모습 머리 (오른쪽을 본다): 검은 모자 + 앞쪽 흰 얼굴 + 부리 + 눈·볼. (머리 Part, 부리 Part들, 얼굴 ops) — 부리는 몸 앞 부품 뒤에 얹는다."""
    st = _pg()
    with xf(rot=head_rot, pivot=(hx - hr * 0.1, hy + hr * 0.85)):
        face_m = [([E(hx - hr * 0.3, hy - hr * 0.3, hx + hr * 1.3, hy + hr * 1.5)], WHITE_BELLY)] if st["chick"] else \
            [([E(hx - hr * 0.02, hy - hr * 0.42, hx + hr * 1.2, hy + hr * 1.3)], belly_color())]
        ex, ey = hx + hr * 0.44, hy - hr * 0.02
        lens_m, glint, rims = glasses_side(ex, ey + _ST["eye_dy"], hx, hr)
        hpart = Part([C(hx, hy, hr)], cap_color(), face_m + lens_m, grow=1, spikes=fluff(hx - 3, hy - hr, st["fluff"]))
        dp.mark_head(hx - 1, hy - hr * 0.84, hr * 1.25)
        bp, bo = beak(hx + hr * 0.92, hy + hr * 0.2, mouth, True, k=_ST["head"] ** 0.5)
        face = rims + glint + list(dp.stage_eye(eyes, ex, ey))
        if brow:
            face += dp.brows(brow, ex, ey + _ST["eye_dy"], flip=True)
        face += dp.blush(hx + hr * 0.26, hy + hr * 0.64)  # 볼은 렌즈 아래
    return hpart, bp, face + bo


def penguin(sp, *, side=True, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, flips=None,
            feet=((0.0, 0.0), (0.0, 0.0)), head=(0.0, 0.0), head_rot=0.0, fx=(), held=(), props=(), belly=False, rock=0.0):
    """펭귄 한 장. side: 오른쪽을 보는 옆모습 · bottom: 몸 아래끝 (발 위, 없으면 바닥에 섬) · rot: 발밑을 축으로 기우뚱 ·
    rock: 기우뚱 축 높이 (몸 세로 반지름 배수, 0 = 발밑 — 동글 공 몸은 가운데를 축으로 흔들어야 위아래가 엇갈려 뒤뚱이 보인다) ·
    flips: 옆모습 [(near|far, 각도)] (None = 가까운 쪽을 옆구리에 붙임) · 앞모습 (왼쪽 각도, 오른쪽 각도) ·
    feet: 옆모습 (뒤, 앞) / 앞모습 (왼, 오른) 발 (dx, 들기) 또는 None · head: 머리 (dx, dy) · belly: 발라당 (앞모습, 발이 위로)."""
    st = _pg()
    col = back_color()
    cap = cap_color()
    bcol = belly_color()
    hk, bk = _ST["head"], _ST["body"]
    fk = bk ** 0.5
    parts, ops, late = [], [], []
    rx, ry, hr = body_dims(side)
    bottom = FLOOR - 3 if bottom is None else bottom
    bx = cx - (2 if side else 0)
    by = bottom - ry
    pivot = (bx, bottom)
    ln = 16 * st["flip"] * bk  # 동글 몸에 짧고 작은 지느러미
    with xf(rot=rot, pivot=(bx, bottom - ry * rock)):
        with xf(sx=sq[0], sy=sq[1], pivot=pivot):
            if side:
                hx, hy = bx + rx * 0.26 + head[0], by - ry * 0.62 - hr * 0.28 + head[1]
                sh = (bx - rx * 0.02, by - ry * 0.3)
                for which, a in (flips or []):
                    if which == "far":
                        parts.append(flipper((sh[0] + 4, sh[1] - 1), ln * 0.92, a, k=fk))
                if feet:
                    for j, (dx, lift) in enumerate(feet):  # 동글 몸 아래로 짧은 발이 쏙
                        parts.append(side_foot(bx - rx * 0.36 + rx * 0.62 * j + dx, lift, k=fk * 1.12, y=bottom + 3.0))
                tail = T((bx - rx * 0.7, by + ry * 0.4), (bx - rx - 6 * bk, by + ry * 0.96), (bx - rx * 0.3, by + ry * 0.92))
                # 볼록 배: 몸 앞쪽 아래로 불룩 튀어나온 공 (무게중심이 아래)
                bulge = E(bx - rx * 0.3, by - ry * 0.5, bx + rx * 1.1, by + ry)
                belly_m = [([E(bx - rx * 0.42, by - ry * 0.6, bx + rx * 1.3, by + ry * 1.12)], bcol)]
                spikes = fuzz(bx, by, rx, ry, (200, 165, 135)) if st["fuzz"] else []
                parts.append(Part([E(bx - rx, by - ry, bx + rx, by + ry), bulge, tail], col, belly_m, grow=1, spikes=spikes))
                hpart, bp, face = side_head(hx, hy, hr, eyes, mouth, brow, head_rot)
                parts.append(hpart)
                if st["tie"]:
                    tx, ty = hx + hr * 0.66 + head[0] * 0.2, hy + hr * 0.98 + head[1] * 0.2
                    parts.append(bow_tie(tx, ty, True, k=bk))
                near = [a for w, a in (flips or [("near", 8)]) if w == "near"]
                for a in near:
                    parts.append(flipper(sh, ln, a, k=fk))
                parts += bp
                ops += face
            else:
                fcx = bx
                hx, hy = fcx + head[0], front_head_y(by, ry, hr) + head[1]
                if feet and not belly:
                    for s, (dx, lift) in zip((-1, 1), feet):
                        parts.append(front_foot(fcx + s * 11 * bk + dx, s, lift, k=fk, y=bottom + 2.5))
                # 아래가 더 넓은 동글 공 몸 (무게중심이 아래) + 몸 너비 대부분을 차지하는 동그란 흰 배
                hips = E(fcx - rx * 1.04, by - ry * 0.2, fcx + rx * 1.04, by + ry)
                belly_m = [([E(fcx - rx * 0.8, by - ry * 0.56, fcx + rx * 0.8, by + ry * 1.06)], bcol)]
                spikes = fuzz(fcx, by, rx, ry, (205, 160, 20, -25)) if st["fuzz"] else []
                parts.append(Part([E(fcx - rx, by - ry, fcx + rx, by + ry), hips], col, belly_m, grow=1, spikes=spikes))
                if belly and feet:  # 발라당: 발이 위로 버둥 (몸 앞)
                    for s, (dx, lift) in zip((-1, 1), feet):
                        late.append(front_foot(fcx + s * 11 * bk + dx, s, lift, k=fk, y=by - ry * 0.05))
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    face_col = WHITE_BELLY if st["chick"] else bcol
                    m = 0.62 if st["chick"] else 0.5  # 아기·유아는 흰 얼굴이 더 넓다
                    face_m = [([C(hx - hr * 0.38, hy + hr * 0.08, hr * m), C(hx + hr * 0.38, hy + hr * 0.08, hr * m),
                                E(hx - hr * 0.7, hy + hr * 0.05, hx + hr * 0.7, hy + hr * 1.15)], face_col)]
                    lens_m, glint, rims = glasses_front(hx, hy + hr * 0.06 + _ST["eye_dy"], hr)
                    parts.append(Part([C(hx, hy, hr)], cap, face_m + lens_m, grow=1, spikes=fluff(hx - 1, hy - hr, st["fluff"])))
                    dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
                    bp, bo = beak(hx, hy + hr * 0.3, mouth, False, k=hk ** 0.5)
                    face = rims + glint
                    for ex, flip in ((hx - hr * 0.4, False), (hx + hr * 0.4, True)):
                        face += dp.stage_eye(eyes, ex, hy + hr * 0.06, flip)
                        if brow:
                            face += dp.brows(brow, ex, hy + hr * 0.06 + _ST["eye_dy"], flip=flip)
                    face += dp.blush(hx - hr * 0.68, hy + hr * 0.64) + dp.blush(hx + hr * 0.68, hy + hr * 0.64)  # 볼은 렌즈 아래
                parts += late
                fl, fr = flips or (12, 12)
                for s, a in ((-1, fl), (1, fr)):
                    parts.append(flipper((fcx + s * rx * 0.9, by - ry * 0.34), ln, a, side=False, s=s, k=fk))
                if st["tie"]:
                    parts.append(bow_tie(hx, hy + hr * 0.98, False, k=bk))
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    parts += bp
                ops += face + bo
            parts += [h() for h in held]
    parts += [p() for p in props]
    return dp.compose(parts, ops, list(fx))


def shrimp_bubble_fx(cx, cy):
    """생각 풍선 안에 C자로 꼬부린 통통한 새우 한 마리 (마디 줄 + 꼬리 부채 + 까만 눈)."""
    fx = dp._bubble(cx, cy)
    body = arc(cx + 0.5, cy - 0.5, 5.6, 5.0, -150, 110, 12)  # 머리(왼쪽 위) → 아래로 휘어 꼬리(왼쪽 아래)
    tail = T((body[-1][0] + 0.5, body[-1][1] - 0.5), (body[-1][0] - 5.5, body[-1][1] + 1.5), (body[-1][0] - 1.5, body[-1][1] + 4.5))
    fx += [([strand(body, 6.4, 3.4), tail], SHRIMP_DARK, 0), ([strand(body, 4.6, 1.8)], SHRIMP, 0)]
    fx += [([L([body[k], (body[k][0] + (body[k][0] - cx) * 0.25, body[k][1] + (body[k][1] - cy) * 0.25)], 0.6)], SHRIMP_DARK, 0) for k in (4, 7, 10)]
    hx, hy = body[0]
    fx += [([C(hx + 1.2, hy + 0.6, 0.9)], PEN_LINE, 0), ([L([(hx, hy), (hx + 5, hy - 4.5)], 0.6)], SHRIMP_DARK, 0)]
    return fx


def ice_puffs(items):
    """배 미끄럼 앞으로 튀는 얼음 가루 (프레임 좌표 아님 — 지금 변환)."""
    return [fx for x, y, r in items for fx in (([C(x, y, r + 1.2)], ICE_DARK, 0), ([C(x, y, r)], ICE, 0))]


# =====================================================================
# 포즈
# =====================================================================
WADDLE = [-16, 0, 20, 0]  # 뒤뚱: 기우뚱 각도 (동글 몸 가운데가 축이라 크게 기울여야 뒤뚱이 보인다)


def pg_walk(sp, i):
    """뒤뚱뒤뚱: 몸이 좌우로 기우뚱 (발밑이 축), 기운 쪽 반대 발을 살짝 들고, 지느러미를 벌려 균형."""
    t = WADDLE[i]
    up = [0, 1.2, 0, 1.2][i]
    feet = [((0, 0), (0, -3)), ((0, 0), (0, 0)), ((0, -3), (0, 0)), ((0, 0), (0, 0))][i]
    return penguin(sp, bottom=FLOOR - 3 - up, rot=t, rock=1.3, head_rot=-t * 0.3, feet=feet, flips=[("near", [30, 20, 30, 20][i])])


def pg_run(sp, i):
    """총총 달리기: 앞으로 기울이고 지느러미를 뒤로 쫙, 빠른 발, 속도선."""
    up = [0, 2, 3, 1][i]
    feet = [((-3, -3), (3, 0)), ((-1, -1), (1, -1)), ((3, 0), (-3, -3)), ((1, -1), (-1, -1))][i]
    return penguin(sp, bottom=FLOOR - 3 - up, rot=[6, 10, 6, 10][i], eyes="happy", mouth="open" if i % 2 else None, feet=feet,
                   flips=[("far", [70, 90, 70, 90][i] - 8), ("near", [70, 90, 70, 90][i])], fx=dp.speed_fx(2, (72, 88, 104), 12, [0, -3, 0, -3][i]))


def slide(sp, i):
    """배 미끄럼 한 장: 몸을 바닥에 길게 눕히고 (흰 배가 바닥 쪽, 검은 등이 위), 머리는 앞에서 쏙 들어 활짝,
    지느러미는 뒤로 쭉, 발은 뒤에서 동동, 앞으로 얼음 가루."""
    st = _pg()
    col, bcol = back_color(), belly_color()
    hk, bk = _ST["head"], _ST["body"]
    fk = bk ** 0.5
    cx_, cy_ = st["chub"]
    lx, ly = 26 * bk * cx_, 18.5 * bk * cy_  # 누운 동글 몸 (길이, 두께) — 통통한 배가 바닥에 볼록
    hr = 16.5 * hk
    bx, by = 56 - (hk - 1) * 10, FLOOR - 1.5 - ly
    parts = []
    # 뒤로 뻗은 발 (몸 뒤)
    for j, dy in enumerate([(-1.5, 1.5), (1.5, -1.5)][i]):
        x, y = bx - lx * 0.82 - 3 * j, by + ly * 0.35 + 3 * j + dy
        parts.append(Part([E(x - 11 * fk, y - 2.6 * fk, x + 2, y + 2.6 * fk)], FEET, grow=0, outline=True))
    tail = T((bx - lx * 0.8, by - ly * 0.5), (bx - lx - 7 * bk, by - ly * 0.95), (bx - lx * 0.75, by + ly * 0.1))
    belly_m = [([E(bx - lx * 1.1, by - ly * 0.1, bx + lx * 1.1, by + ly * 1.6)], bcol)]
    spikes = fuzz(bx, by, lx, ly, (-120, -90, -60)) if st["fuzz"] else []
    parts.append(Part([E(bx - lx, by - ly, bx + lx, by + ly), tail], col, belly_m, grow=1, spikes=spikes))
    hx, hy = bx + lx * 0.78, by - ly * 0.4 - hr * 0.2
    hpart, bp, face = side_head(hx, hy, hr, "happy", "open", None, -6)
    parts.append(hpart)
    if st["tie"]:
        parts.append(bow_tie(hx + hr * 0.5, hy + hr * 1.0, True, k=bk))
    parts.append(flipper((bx + lx * 0.15, by - ly * 0.4), 16 * st["flip"] * bk, [104, 96][i], k=fk))
    parts += bp
    puffs = ice_puffs([[(118, FLOOR - 3, 3.4), (111, FLOOR - 1.5, 2.4), (122, FLOOR - 10, 2.2)],
                       [(120, FLOOR - 4, 3.0), (113, FLOOR - 1.5, 2.8), (116, FLOOR - 12, 2.4)]][i])
    return dp.compose(parts, face, puffs)


def pg_roll(sp, i):
    """구르기 대신 배로 쭉 미끄러짐 (앱이 돌리지 않는다 — pet.json roll에 spin 없음)."""
    return slide(sp, i)


def pg_idle(sp, i):
    return penguin(sp, sq=(1.0, 1.0 + 0.02 * i), head=(0, -0.8 * i), flips=[("near", 8 + 4 * i)])


def pg_look(sp, i):
    return penguin(sp, head_rot=[-10, 0, 12, 0][i], eyes="closed" if i == 3 else "dot")


def pg_lookup(sp, i):
    """고개를 젖혀 부리를 들고 위를 본다."""
    return penguin(sp, head=(-1, -1), head_rot=dp.lookup_tilt(i), eyes="up", fx=dp.lookup_fx(i, 112, 14))


def pg_sleep(sp, i):
    """배를 깔고 동글 웅크려 쿨쿨: 고개를 푹 숙이고 z."""
    zs = [dp.z_fx(102, 60, 3.4), dp.z_fx(113, 49, 2.6)] if i == 0 else [dp.z_fx(104, 57, 3.6), dp.z_fx(116, 45, 2.8)]
    return penguin(sp, bottom=FLOOR - 1, sq=(1.1, 0.9 + 0.012 * i), eyes="closed", head=(-2, 7), head_rot=12, feet=None,
                   flips=[("near", 4)], fx=zs)


def pg_annoyed(sp, i):
    return penguin(sp, bottom=FLOOR - 2, sq=(1.05, 0.95), eyes="squint", mouth="frown", head=(0, 2), flips=[("near", 40)],
                   fx=[dp.anger_fx(104, 36 - i, 7.0)])


def pg_hop(sp, i):
    """폴짝: 웅크림 → 지느러미 번쩍 솟구침 → 떠서 ^^."""
    if i == 0:
        return penguin(sp, sq=(1.1, 0.86), eyes="tight", head=(0, 3), flips=[("near", 30)])
    if i == 1:
        return penguin(sp, bottom=FLOOR - 18, sq=(0.93, 1.08), rot=-6, eyes="happy", mouth="open", feet=((-2, 0), (2, -2)),
                       flips=[("far", 140), ("near", 150)])
    return penguin(sp, bottom=FLOOR - 10, eyes="happy", feet=((0, -1), (0, 0)), flips=[("far", 90), ("near", 100)])


def pg_fall(sp, i):
    stars = dp._stars([(100, 22, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 20, 3.6), (104, 32, 3.0)])
    return penguin(sp, bottom=FLOOR - 10, rot=[-14, 14][i], eyes="spiral", mouth="open", feet=((-2, -2), (2, 0)),
                   flips=[("far", [150, 60][i]), ("near", [60, 150][i])], fx=stars)


def pg_dragged(sp, i):
    """잡혀서 버둥: 지느러미 파닥, > < 눈, 땀."""
    s = [-1, 1][i]
    return penguin(sp, bottom=FLOOR - 10, rot=5 * s, eyes="tight", mouth="open", feet=((-2 + 2 * s, -2), (2 + 2 * s, 0)),
                   flips=[("far", [140, 90][i]), ("near", [100, 150][i])], fx=dp.sweat_fx(104, 32))


def pg_land(sp, i):
    if i == 0:
        return penguin(sp, sq=(1.12, 0.84), eyes="tight", flips=[("far", 90), ("near", 100)])
    return penguin(sp, sq=(1.02, 0.98), flips=[("near", 30)])


def pg_sit_front(sp, i):
    return penguin(sp, side=False, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot")


def pg_wave(sp, i):
    return penguin(sp, side=False, flips=(12, [130, 160, 130, 160][i]), rot=[-3, -1, -3, -1][i], mouth="chirp" if i % 2 else None)


def pg_tilt(sp, i):
    return penguin(sp, side=False, head_rot=dp._swing([8, 12, 12][i]), fx=[dp.question_fx(18, 22 + [2, 0, -2][i])])


def pg_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return penguin(sp, side=False, belly=True, bottom=FLOOR - 1, sq=(1.12, 0.8), rot=-6, eyes="happy", mouth="open",
                   feet=((-1 + wig, -2), (1 - wig, -2 + wig)), flips=(110 + 6 * wig, 110 - 6 * wig))


def pg_stretch(sp, i):
    if i == 0:
        return penguin(sp, side=False, sq=(0.94, 1.08), eyes="tight", flips=(165, 165), head=(0, -3))
    if i == 1:
        return penguin(sp, side=False, eyes="tight", mouth="yawn", flips=(100, 100), rot=-3)
    return penguin(sp, side=False)


def pg_heart(sp, i):
    hs = dp.heart_burst(i)
    hy = FLOOR - 3 - body_dims(side=False)[1] * 0.8
    held = [lambda: dp.held_heart(hy)]
    return penguin(sp, side=False, eyes="happy", flips=(55, 55), held=held, fx=hs)


def pg_tickle(sp, i):
    return penguin(sp, side=False, rot=dp._swing([-7, 7, 0][i]), eyes="happy", mouth="chirp" if i < 2 else "open",
                   flips=([60, 130, 95][i], [130, 60, 95][i]))


def pg_hungry(sp, i):
    return penguin(sp, side=False, rot=-4, eyes="closed", mouth=None, brow="sad", head=(0, [0, 1, 0][i] + 2),
                   flips=(10, [24, 34, 24][i]), fx=shrimp_bubble_fx(106, 18))


def pg_eat(sp, i):
    """콕콕 (그릇 없음): 0·2 = 고개를 숙여 부리 끝이 그릇 밥 더미 (프레임 (64, 84))에 콕, 1·3 = 그보다 고개를 쏙 들고 ^^
    (포동포동 몸이라 머리가 낮다 — 든 고개도 콕 자리에서 잰다)."""
    peck = i % 2 == 0
    k = _ST["head"] ** 0.5
    mx, my = dp._frame_to_pre(64, 86)
    st_b = FLOOR - 3
    _, ry, hr = body_dims(side=False)  # penguin() 앞모습과 같은 몸·머리
    hy0 = front_head_y(st_b - ry, ry, hr)
    want = my - hr * 0.3 - 7.0 * k  # 부리 끝 = 머리 가운데 + 0.3 hr + 부리 길이
    fx = dp.eat_crumbs(i, SHRIMP)
    return penguin(sp, side=False, bottom=st_b, head=(0, want - hy0 - (0 if peck else 12 * _ST["head"])), eyes="closed" if peck else "happy",
                   mouth=None if peck else "chew", flips=(34, 34), sq=(1.0, 0.97 if peck else 1.0), fx=fx)


def pg_level_up(sp, i):
    stars = dp.level_up_stars(i)
    up = [150, 165, 175][i]
    return penguin(sp, side=False, bottom=FLOOR - 3 - [0, 1, 3][i], eyes="happy", mouth="big", flips=(up, up), fx=stars)


PENGUIN_POSES = {"walk": pg_walk, "run": pg_run, "roll": pg_roll, "idle": pg_idle, "look": pg_look, "sleep": pg_sleep, "hop": pg_hop,
                 "fall": pg_fall, "dragged": pg_dragged, "land": pg_land, "sit_front": pg_sit_front, "wave": pg_wave, "tilt": pg_tilt,
                 "belly": pg_belly, "stretch": pg_stretch, "heart": pg_heart, "tickle": pg_tickle, "annoyed": pg_annoyed,
                 "hungry": pg_hungry, "eat": pg_eat, "level_up": pg_level_up, "lookup": pg_lookup}
def lens_key(im, discs):
    """렌즈 둥근 자리 (그린 렌즈 원 그대로 — 안의 눈·반짝까지)의 알파를 LENS_ALPHA로. 눈으로는 254와 255가 같다.
    외곽선 부드러운 가장자리에 우연히 생긴 254는 253으로 비켜 렌즈만 254가 되게 한다."""
    a = np.array(im.convert("RGBA"))
    a[a[..., 3] == LENS_ALPHA, 3] = LENS_ALPHA - 1
    if discs:
        m = dp.shapes(discs).resize(im.size, Image.BILINEAR)
        a[(np.asarray(m) >= 128) & (a[..., 3] == 255), 3] = LENS_ALPHA
    return Image.fromarray(a)


def _lens_keyed(fn):
    def f(sp, i):
        _LENS_SHAPES.clear()
        im = fn(sp, i)
        return lens_key(im, list(_LENS_SHAPES)) if GLASS["alpha_key"] else im
    f.__name__, f.__doc__ = fn.__name__, fn.__doc__
    return f


RAW_POSES = dict(PENGUIN_POSES)
PENGUIN_POSES = {k: _fit(_lens_keyed(v)) for k, v in PENGUIN_POSES.items()}


PENGUIN = Species(
    id="penguin", name="펭귄", walk_speed=65, scale=0.94, base=BACK[3], back=BACK[3], belly=WHITE_BELLY, nose=BEAK, mouth="w",
    front_tail_color=BACK[3], tail_color=BACK[3], stage_size=dict(dict.fromkeys(STAGES, 0.85), baby=0.935), stripe=BOWTIE, outline=PEN_LINE,
    # 외곽선은 모든 단계 같은 짙은 남색 (soft 0 — 첫 부품이 주황 발이라 섞으면 갈색이 된다)
    stage_tweak={"baby": dict(pale=0.0, soft=0.0, eye=1.2, fit=0.9), "kinder": dict(pale=0.0, soft=0.0, eye=1.12, fit=0.93),
                 "child": dict(pale=0.0, eye=1.05), "adult": dict(fit=0.94)},
    specials=["lookup"], poses=PENGUIN_POSES, **dp.NO_COMMON_PARTS,
)


def register():
    dp.SPECIES[PENGUIN.id] = PENGUIN


register()


def main():
    import draw_halloween as dh
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(PENGUIN, root / "ui" / "pets" / PENGUIN.id))
    dh.preview_species(by_stage, pv / "stages_penguin.png")
    print("ok: penguin")


if __name__ == "__main__":
    main()
