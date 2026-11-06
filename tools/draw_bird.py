"""새 🐦 — 쌩쌩 빠른 꼬마 제비(bird). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_bird.py → ui/pets/bird/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_bird.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

제비: 동글 머리 + 작은 몸 + 길고 뾰족한 날개 + 두 갈래로 갈라진 긴 꽁지깃. 등·머리는 파란 남색, 얼굴 아래·배는 크림,
      턱(멱)과 이마는 빨강, 작고 까만 부리, 앉으면 조그만 발. 이름은 "새" (사용자가 정한 이름 그대로).
      단계: 아기 = 회색빛 솜털 아기 새, 부리 끝 노란 입가, 정수리 솜털 세 가닥, 꽁지는 뭉툭 → 유아 짧은 갈래 꽁지, 솜털 하나
            → 어린이 빨간 멱·이마 → 청소년 긴 꽁지깃 → 어른 더 긴 꽁지깃 + 이마에 비행 고글 (가장 빠른 새).
날기 (pet.json locomotion fly + swift): walk = 파닥파닥 날기 (날개 위 → 가운데 → 아래 → 가운데), run = 날개를 뒤로 접고 쌩 (속도선),
      둘 다 바닥에서 떠 있다. 앉은 동작(idle·look·sleep·annoyed·lookup·land, 앞모습 동작)은 창 윗면·바닥에 내려앉아 발로 선다
      (코어가 떠서 쉴 땐 idle 대신 walk를 보인다 — brain.rs render).
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 앞모습으로 고개를 숙여 부리 끝을 그릇 밥 더미(프레임 (64, 84))에 콕 → 고개 들고 오물.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (BLUSH, C, E, L, T, WHITE, Part, Species, _ST, arc, mix, strand, view, xf)  # noqa: E402
from draw_pets import _fit  # noqa: E402
from sprite_spec import ROLL_CENTER, STAGES  # noqa: E402

FLOOR = 125.0

BIRD_LINE = (84, 88, 138)  # 외곽선 (짙은 남색 — 파란 등에 갈색 선은 탁하다)
NAVY = {0: (178, 190, 222), 1: (146, 164, 220), 2: (126, 150, 214), 3: (112, 138, 206), 4: (102, 128, 200)}  # 등·머리
CREAM = (255, 251, 242)
THROAT = (240, 128, 106)  # 멱·이마 빨강
BEAK = (88, 74, 86)
GAPE = (255, 214, 96)  # 아기 새 노란 입가
FEET = (236, 168, 152)
GOGGLE, GOGGLE_LENS, GOGGLE_SHINE = (196, 136, 86), (178, 226, 248), (255, 255, 255)
SEED, SEED_DARK = (236, 202, 128), (176, 132, 84)
SPEED = dp.SPEED

# 단계(lvl)별 — fluff: 정수리 솜털 · tail: 꽁지깃 길이 (0 = 뭉툭) · red: 빨간 멱·이마 · gape: 노란 입가 · goggles · wing: 날개 길이
BD_STAGE = {
    0: dict(fluff=3, tail=0.12, fork=0.35, red=False, gape=True, goggles=False, wing=0.8),
    1: dict(fluff=1, tail=0.4, fork=0.6, red=False, gape=True, goggles=False, wing=0.9),
    2: dict(fluff=0, tail=0.65, fork=0.85, red=True, gape=False, goggles=False, wing=0.96),
    3: dict(fluff=0, tail=0.9, fork=1.0, red=True, gape=False, goggles=False, wing=1.0),
    4: dict(fluff=0, tail=1.05, fork=1.0, red=True, gape=False, goggles=True, wing=1.05),
}


def _bd():
    return BD_STAGE[_ST["lvl"]]


def body_color():
    return NAVY[_ST["lvl"]]


def wing_color(stage=None):
    """날개 색 (몸보다 조금 짙다). stage: 단계 이름 (시험용)."""
    lvl = _ST["lvl"] if stage is None else STAGES.index(stage)
    return mix(NAVY[lvl], BIRD_LINE, 0.22)


def _pts(root, local, rot):
    """뿌리(root)에서 local 점들을 rot도 돌려 놓는다 (y가 아래로 +, rot +는 시계 방향)."""
    a = math.radians(rot)
    ca, sa = math.cos(a), math.sin(a)
    return [(root[0] + x * ca - y * sa, root[1] + x * sa + y * ca) for x, y in local]


def wing_shape(root, ln, rot, side=True, s=1):
    """길고 뾰족한 날개 (뿌리 root에서 뒤쪽(-x)으로 ln). side: 옆모습 (rot 0 = 등에 접힘, - = 위로 듦, + = 아래로).
    앞모습(side=False)은 아래로 뻗은 날개 (rot 0 = 몸 옆에 접힘, s쪽으로 벌어짐)."""
    if side:
        local = [(5, -3.5), (-ln * 0.2, -6.5), (-ln * 0.55, -5.5), (-ln, -0.5), (-ln * 0.7, 3.2), (-ln * 0.62, 1.8),
                 (-ln * 0.48, 5.5), (-ln * 0.4, 3.8), (-ln * 0.22, 7), (4, 5)]
        return T(*_pts(root, local, -rot))
    local = [(-3.5 * s, -4), (-6.5 * s, ln * 0.25), (-5 * s, ln * 0.6), (0.5 * s, ln), (3 * s, ln * 0.7), (1.8 * s, ln * 0.62),
             (5 * s, ln * 0.45), (4 * s, ln * 0.36), (6 * s, ln * 0.18), (4 * s, -4)]
    return T(*_pts(root, local, -rot * s))


def wing_part(root, ln, rot, side=True, s=1):
    col = wing_color()
    tip = mix(col, BIRD_LINE, 0.3)
    shp = wing_shape(root, ln, rot, side, s)
    # 날개 끝 깃 (짙은 끝)
    if side:
        tips = [L(_pts(root, [(-ln * 0.55, 0.0), (-ln * 0.98, -0.2)], -rot), 3.0)]
    else:
        tips = [L(_pts(root, [(0, ln * 0.55), (0.3 * s, ln * 0.97)], -rot * s), 3.0)]
    return Part([shp], col, [(tips, tip)], grow=0, outline=True)


def tail_parts(root, ln, fork, rot=0.0, front=False, k=1.0):
    """두 갈래 꽁지깃: 뿌리 root에서 뒤쪽(-x)으로 ln (옆모습). fork: 갈래 벌어짐. front: 앞모습 — 몸 아래 양옆으로 갈래."""
    col = mix(body_color(), BIRD_LINE, 0.12)
    if ln <= 0:
        return []
    if front:
        x, y = root
        tips = [strand([(x - 2, y - 4), (x - 5 - 5 * fork * k, y + ln * 0.6), (x - 7 - 9 * fork * k, y + ln)], 7 * k, 1.4),
                strand([(x + 2, y - 4), (x + 5 + 5 * fork * k, y + ln * 0.6), (x + 7 + 9 * fork * k, y + ln)], 7 * k, 1.4)]
        return [Part(tips, col, grow=0, outline=True)]
    base = [T(*_pts(root, [(4, -6), (-ln * 0.3, -5), (-ln * 0.3, 5), (4, 6)], rot))]
    up = strand(_pts(root, [(-2, -2), (-ln * 0.55, -3 - 4 * fork), (-ln, -5 - 10 * fork)], rot), 7.5, 1.3)
    lo = strand(_pts(root, [(-2, 2), (-ln * 0.55, 2 + 2 * fork), (-ln * 0.92, 2 + 7 * fork)], rot), 7.0, 1.3)
    return [Part(base + [up, lo], col, grow=0, outline=True)]


def fluff(cx, top, n):
    """정수리 솜털 (아기 세 가닥, 유아 한 가닥)."""
    if n == 0:
        return []
    pts = [[(cx - 1, top + 4)] + arc(cx + 5, top - 4, 5, 6, 180, 300, 8)]
    if n >= 3:
        pts += [[(cx - 3, top + 4), (cx - 6, top - 3), (cx - 10, top - 4)], [(cx + 2, top + 4), (cx + 7, top - 2), (cx + 11, top)]]
    return [strand(p, 4.0, 1.1) for p in pts]


def beak(bx, by, kind, side, k=1.0, s=1):
    """부리 Part들 + 입 ops. side: 옆모습 (bx, by = 부리 뿌리, s = 바라보는 쪽) · 앞모습 (bx, by = 부리 뿌리 가운데, 아래로 콕)."""
    parts, ops = [], []
    gap = {"open": 3.0, "big": 4.5, "yawn": 5.0, "chirp": 3.5}.get(kind, 0.0)
    gape = _bd()["gape"]
    if side:
        ln = 9.5 * k
        if gap:
            up = [T((bx, by - 3.2 * k), (bx + s * ln, by - 0.6), (bx, by + 0.6))]
            lo = [T((bx, by + 0.6), (bx + s * ln * 0.85, by + gap), (bx, by + 3.2 * k + gap * 0.4))]
            ops.append(([T((bx, by), (bx + s * ln * 0.6, by + gap * 0.6), (bx, by + gap * 0.6))], dp.MOUTH_IN))
            parts.append(Part(lo, BEAK, grow=0, outline=True))
        else:
            up = [T((bx, by - 3.2 * k), (bx + s * ln, by + 0.4), (bx, by + 3.2 * k))]
        parts.append(Part(up, BEAK, [([E(bx + s * 1.5, by - 2.2 * k, bx + s * 4.5, by - 0.8 * k)], mix(BEAK, WHITE, 0.35))], grow=0, outline=True))
        if gape:
            parts.append(Part([C(bx - s * 0.5, by + 1.4 * k, 2.4 * k)], GAPE, grow=0, outline=True))
        return parts, ops
    w, ln = 4.2 * k, 7.5 * k
    if gap:
        ops.append(([E(bx - w * 0.8, by + 1, bx + w * 0.8, by + 2 + gap)], dp.MOUTH_IN))
        lo = [T((bx - w * 0.75, by + gap * 0.6 + 1), (bx + w * 0.75, by + gap * 0.6 + 1), (bx, by + ln * 0.7 + gap))]
        parts.append(Part(lo, BEAK, grow=0, outline=True))
        up = [T((bx - w, by), (bx + w, by), (bx, by + ln * 0.62))]
    else:
        up = [T((bx - w, by), (bx + w, by), (bx, by + ln))]
    parts.append(Part(up, BEAK, [([E(bx - w * 0.5, by + 0.6, bx - w * 0.05, by + 2.6)], mix(BEAK, WHITE, 0.35))], grow=0, outline=True))
    if gape:
        parts.insert(0, Part([C(bx - w - 0.6, by + 1.2, 2.3 * k), C(bx + w + 0.6, by + 1.2, 2.3 * k)], GAPE, grow=0, outline=True))
    if kind == "chew":
        ops.append(([L([(bx - 2.5, by + ln + 2.5), (bx, by + ln + 1.2), (bx + 2.5, by + ln + 2.5)], 1.1)], BEAK))
    return parts, ops


def goggles(hx, hy, hr, side):
    """어른: 이마에 올린 비행 고글 (밤색 끈 + 하늘색 렌즈)."""
    if side:
        strap = L(arc(hx, hy - hr * 0.1, hr * 0.98, hr * 0.98, 205, 300, 10), 3.0)
        lx, ly = hx + hr * 0.5, hy - hr * 0.62
        lens = [C(lx, ly, 4.6)]
        return [Part([strap] + lens, GOGGLE, [([C(lx + 0.6, ly, 3.0)], GOGGLE_LENS), ([C(lx - 0.6, ly - 1.2, 1.0)], GOGGLE_SHINE)],
                     grow=0, outline=True)]
    y = hy - hr * 0.58
    strap = L(arc(hx, hy + hr * 0.05, hr * 1.0, hr * 0.66, 200, 340, 12), 3.0)
    lenses = [C(hx - 6.5, y, 5.0), C(hx + 6.5, y, 5.0)]
    marks = [([C(hx - 6.5, y, 3.3), C(hx + 6.5, y, 3.3)], GOGGLE_LENS), ([C(hx - 7.6, y - 1.3, 1.1), C(hx + 5.4, y - 1.3, 1.1)], GOGGLE_SHINE)]
    return [Part([strap] + lenses, GOGGLE, marks, grow=0, outline=True)]


def bird(sp, *, side=True, c=None, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None,
         wings=None, feet="perch", head=(0.0, 0.0), head_rot=0.0, tail_rot=0.0, fx=(), held=(), props=(), ball=False, belly=False):
    """새 한 장. side: 오른쪽을 보는 옆모습 · c: 몸 가운데 (날 때) 또는 bottom: 몸 아래끝 (앉을 때, 발 위) ·
    wings: 옆모습 [(앞/뒤 날개, 각도)] (None = 등에 접음) · 앞모습 ((왼쪽 각도), (오른쪽 각도)) ·
    feet: "perch" (바닥에 섬) | "tuck" (배에 붙임) | "up" (발라당) | None · head: 머리 (dx, dy) · ball: 구르기 공 (머리를 묻은 털 공)."""
    st = _bd()
    col = body_color()
    hk, bk = _ST["head"], _ST["body"]
    parts, ops, late = [], [], []
    rx, ry = 22 * bk * _ST["torso"][0], 16 * bk * _ST["torso"][1]
    hr = (19.5 if side else 21.0) * hk
    belly_col = CREAM
    if c is None:
        bottom = FLOOR - 5 if bottom is None else bottom
        if not side:
            ry *= 1.2
        c = (cx - (4 if side else 0), bottom - ry)
    bx, by = c
    pivot = (bx, by + ry)
    with xf(rot=rot, pivot=pivot):
        with xf(sx=sq[0], sy=sq[1], pivot=pivot):
            if side:
                hx, hy = bx + rx * 0.62 + head[0], by - ry * 0.7 - hr * 0.5 + head[1]
                # 먼 쪽 날개 (몸 뒤) — 날 때만 보인다
                sh = (bx + rx * 0.15, by - ry * 0.55)
                ln = 40 * st["wing"] * bk
                for which, a in (wings or []):
                    if which == "far":
                        parts.append(wing_part((sh[0] + 3, sh[1] - 2), ln * 0.9, a))
                # 다리 (몸 뒤)
                if feet == "perch" and not ball:
                    for j in (0, 1):
                        x = bx + 2 + 7 * j
                        parts.append(Part([L([(x, by + ry - 4), (x + 1, FLOOR - 1.5)], 2.4)], FEET, grow=0, outline=True))
                        parts.append(Part([E(x - 2, FLOOR - 3.2, x + 5.5, FLOOR - 0.3)], FEET, grow=0, outline=True))
                parts += tail_parts((bx - rx * 0.8, by - ry * 0.1), 40 * st["tail"] * bk, st["fork"], rot=tail_rot)
                belly_m = [([E(bx - rx * 0.55, by - ry * 0.05, bx + rx * 1.05, by + ry * 1.25)], belly_col)]
                body = Part([E(bx - rx, by - ry, bx + rx, by + ry)], col, belly_m, grow=1)
                if feet == "tuck" and not ball:
                    parts.append(Part([E(bx - 1, by + ry - 3, bx + 8, by + ry + 2.5)], FEET, grow=0, outline=True))
                parts.append(body)
                with xf(rot=head_rot, pivot=(hx - hr * 0.2, hy + hr * 0.8)):
                    face_m = [([E(hx - hr * 0.05, hy - hr * 0.1, hx + hr * 1.2, hy + hr * 1.3)], belly_col)]
                    if st["red"]:
                        face_m += [([E(hx + hr * 0.35, hy + hr * 0.38, hx + hr * 1.15, hy + hr * 1.1)], THROAT),
                                   ([E(hx + hr * 0.62, hy - hr * 0.62, hx + hr * 1.1, hy - hr * 0.18)], THROAT)]
                    hpart = Part([C(hx, hy, hr)], col, face_m, grow=1, spikes=fluff(hx - 3, hy - hr, st["fluff"]))
                    dp.mark_head(hx - 1, hy - hr * 0.84, hr * 1.25)
                    parts.append(hpart)
                    bp, bo = beak(hx + hr * 0.9, hy + hr * 0.12, mouth, True, k=hk ** 0.5)
                    ex, ey = hx + hr * 0.38, hy - hr * 0.08
                    face = list(dp.stage_eye(eyes, ex, ey))
                    if brow:
                        face += dp.brows(brow, ex, ey + _ST["eye_dy"], flip=True)
                    face += dp.blush(hx + hr * 0.12, hy + hr * 0.5)
                    if st["goggles"]:
                        late_head = goggles(hx, hy, hr, True)
                    else:
                        late_head = []
                # 가까운 쪽 날개 (몸 앞)
                near = [a for w, a in (wings or [("near", 0)]) if w == "near"]
                for a in near:
                    parts.append(wing_part(sh, ln, a))
                with xf(rot=head_rot, pivot=(hx - hr * 0.2, hy + hr * 0.8)):
                    parts += bp + late_head
                ops += face + bo
            else:
                fcx = bx
                hx, hy = fcx + head[0], by - ry * 0.55 - hr * 0.62 + head[1]
                ln = 34 * st["wing"] * bk
                if not ball:
                    parts += tail_parts((fcx, by + ry * 0.55), 24 * max(st["tail"], 0.3) * bk, st["fork"], front=True, k=bk)
                if feet == "perch" and not ball and not belly:
                    for s in (-1, 1):
                        x = fcx + s * 7
                        parts.append(Part([L([(x, by + ry - 6), (x, FLOOR - 2)], 2.4)], FEET, grow=0, outline=True))
                        parts.append(Part([E(x - 4, FLOOR - 3.4, x + 4, FLOOR - 0.3)], FEET, grow=0, outline=True))
                belly_m = [([E(fcx - rx * 0.66, by - ry * 0.45, fcx + rx * 0.66, by + ry * 1.05)], belly_col)]
                parts.append(Part([E(fcx - rx, by - ry, fcx + rx, by + ry)], col, belly_m, grow=1))
                if belly:  # 발라당: 발이 위로 버둥 (몸 앞)
                    for s in (-1, 1):
                        x, y = fcx + s * 8, by - ry * 0.2
                        late.append(Part([L([(fcx + s * 6, by + ry * 0.3), (x, y)], 2.4)], FEET, grow=0, outline=True))
                        late.append(Part([C(x, y - 1, 2.8)], FEET, grow=0, outline=True))
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    face_m = [([E(hx - hr * 0.8, hy - hr * 0.05, hx + hr * 0.8, hy + hr * 1.2)], belly_col)]
                    if st["red"]:
                        face_m += [([E(hx - hr * 0.42, hy + hr * 0.45, hx + hr * 0.42, hy + hr * 1.05)], THROAT),
                                   ([E(hx - hr * 0.28, hy - hr * 0.7, hx + hr * 0.28, hy - hr * 0.32)], THROAT)]
                    parts.append(Part([C(hx, hy, hr)], col, face_m, grow=1, spikes=fluff(hx - 1, hy - hr, st["fluff"])))
                    dp.mark_head(hx, hy - hr * 0.84, hr * 1.25)
                    bp, bo = beak(hx, hy + hr * 0.22, mouth, False, k=hk ** 0.5)
                    face = []
                    for ex, flip in ((hx - hr * 0.42, False), (hx + hr * 0.42, True)):
                        face += dp.stage_eye(eyes, ex, hy + hr * 0.02, flip)
                        if brow:
                            face += dp.brows(brow, ex, hy + hr * 0.02 + _ST["eye_dy"], flip=flip)
                    face += dp.blush(hx - hr * 0.66, hy + hr * 0.42) + dp.blush(hx + hr * 0.66, hy + hr * 0.42)
                    late_head = goggles(hx, hy, hr, False) if st["goggles"] else []
                parts += late
                if not ball:
                    wl, wr = wings or (8, 8)
                    for s, a in ((-1, wl), (1, wr)):
                        parts.append(wing_part((fcx + s * rx * 0.82, by - ry * 0.45), ln, a, side=False, s=s))
                with xf(rot=head_rot, pivot=(hx, hy + hr * 0.9)):
                    parts += bp + late_head
                ops += face + bo
            parts += [h() for h in held]
    parts += [p() for p in props]
    return dp.compose(parts, ops, list(fx))


def seeds_bubble_fx(cx, cy):
    """생각 풍선 안에 모이 씨앗 세 알."""
    fx = dp._bubble(cx, cy)
    for x, y, r in ((cx - 4.5, cy + 2, -30), (cx + 4, cy + 3, 25), (cx - 0.5, cy - 4, 5)):
        with xf(rot=r, pivot=(x, y)):
            fx += [([E(x - 2.6, y - 4.2, x + 2.6, y + 4.2)], SEED_DARK, 0), ([E(x - 1.7, y - 3.2, x + 1.7, y + 3.2)], SEED, 0)]
    return fx


# =====================================================================
# 포즈
# =====================================================================
FLY_C = (60.0, 72.0)  # 날 때 몸 가운데 (몸 아래로 바닥까지 틈이 보인다)
FLAP = [-84, -30, 40, -30]  # 옆모습 날갯짓: 위 → 가운데 → 아래 → 가운데


def bd_walk(sp, i):
    """파닥파닥 날기: 날개 위 → 아래, 몸은 날갯짓에 맞춰 살짝 오르내리고 발은 배에 붙인다."""
    a = FLAP[i]
    up = [0, 1.5, 3, 1.5][i]
    return bird(sp, c=(FLY_C[0], FLY_C[1] - up), rot=-4, wings=[("far", a - 14), ("near", a)], feet="tuck", tail_rot=4)


def bd_run(sp, i):
    """쌩 날기: 몸을 앞으로 기울이고 날개를 뒤로 쫙 접은 화살 모양, 뒤로 속도선."""
    a = [10, -8, 10, 4][i]
    return bird(sp, c=(66, FLY_C[1] + [0, -1, 0, 1][i]), rot=8, wings=[("far", a - 10), ("near", a)], feet="tuck", eyes="happy",
                mouth="open" if i % 2 else None, tail_rot=-2, fx=dp.speed_fx(2, (60, 74, 88), 13, [0, -3, 0, -3][i]))


def bd_roll(sp, i):
    """공중제비 공: 날개로 몸을 감싼 동글 털 공 (앱이 돌린다)."""
    cx, cy = ROLL_CENTER
    k = 0.88 if _ST["head"] > 1.1 else 1.0  # 큰 머리(아기·유아)도 회전 원 안에
    with view(scale=1.0, lift=0), xf(sx=k, sy=k, pivot=(cx, cy)):
        return bird(sp, side=False, c=(cx, cy + 4), sq=(1.0, 0.95), eyes="happy", mouth="open" if i else None, ball=True,
                    head=(0, 16), feet=None)


def bd_idle(sp, i):
    return bird(sp, sq=(1.0, 1.0 + 0.02 * i), head=(0, -0.8 * i), tail_rot=-14)


def bd_look(sp, i):
    return bird(sp, head_rot=[-10, 0, 12, 0][i], eyes="closed" if i == 3 else "dot", tail_rot=-14)


def bd_lookup(sp, i):
    """고개를 젖혀 부리를 들고 위를 본다."""
    return bird(sp, head=(-1, -1), head_rot=dp.lookup_tilt(i), eyes="up", tail_rot=-14, fx=dp.lookup_fx(i, 112, 14))


def bd_sleep(sp, i):
    """앉아서 동글 부풀어 쿨쿨: 머리를 어깨에 묻고 z."""
    zs = [dp.z_fx(102, 64, 3.4), dp.z_fx(113, 53, 2.6)] if i == 0 else [dp.z_fx(104, 61, 3.6), dp.z_fx(116, 49, 2.8)]
    return bird(sp, bottom=FLOOR - 2, sq=(1.1, 0.92 + 0.012 * i), eyes="closed", head=(-3, 9), head_rot=10, feet=None,
                tail_rot=-8, fx=zs)


def bd_annoyed(sp, i):
    return bird(sp, bottom=FLOOR - 3, sq=(1.06, 0.94), eyes="squint", mouth="frown", head=(0, 3), tail_rot=-20,
                fx=[dp.anger_fx(104, 38 - i, 7.0)])


def bd_hop(sp, i):
    """폴짝 날아오르기: 웅크림 → 날개 위로 솟구침 → 떠서 ^^."""
    if i == 0:
        return bird(sp, sq=(1.1, 0.86), eyes="tight", head=(0, 3), tail_rot=-10)
    if i == 1:
        lines = [([L([(x, 110), (x, 122)], 1.8)], SPEED, 0) for x in (46, 58, 70)]
        return bird(sp, c=(60, 70), sq=(0.94, 1.06), rot=-10, eyes="happy", mouth="open", wings=[("far", -76), ("near", -62)],
                    feet="tuck", fx=lines)
    return bird(sp, c=(60, 66), eyes="happy", mouth="open", wings=[("far", 20), ("near", 34)], feet="tuck")


def bd_fall(sp, i):
    stars = dp._stars([(100, 22, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 20, 3.6), (104, 32, 3.0)])
    return bird(sp, c=(62, 76), rot=[-22, 22][i], eyes="spiral", mouth="open", wings=[("far", [-70, 30][i]), ("near", [30, -70][i])],
                feet="tuck", fx=stars)


def bd_dragged(sp, i):
    """잡혀서 버둥: 날개 파닥, > < 눈, 땀."""
    s = [-1, 1][i]
    return bird(sp, c=(62, 70), rot=5 * s, eyes="tight", mouth="open", wings=[("far", [-70, 20][i]), ("near", [-56, 34][i])],
                feet="tuck", fx=dp.sweat_fx(104, 34))


def bd_land(sp, i):
    """내려앉기: 날개를 펴 브레이크 → 접으며 앉기."""
    if i == 0:
        return bird(sp, sq=(1.1, 0.86), eyes="tight", wings=[("far", -70), ("near", -56)], tail_rot=-24)
    return bird(sp, sq=(1.02, 0.98), wings=[("near", -14)], tail_rot=-16)


def bd_sit_front(sp, i):
    return bird(sp, side=False, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot")


def bd_wave(sp, i):
    return bird(sp, side=False, wings=(8, [120, 150, 120, 150][i]), rot=[-3, -1, -3, -1][i], mouth="chirp" if i % 2 else None)


def bd_tilt(sp, i):
    return bird(sp, side=False, head_rot=dp._swing([8, 12, 12][i]), fx=[dp.question_fx(18, 22 + [2, 0, -2][i])])


def bd_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return bird(sp, side=False, belly=True, bottom=FLOOR - 1, sq=(1.12, 0.8), rot=-6, eyes="happy", mouth="open", feet=None,
                wings=(110 + 6 * wig, 110 - 6 * wig))


def bd_stretch(sp, i):
    if i == 0:
        return bird(sp, side=False, sq=(0.94, 1.08), eyes="tight", wings=(160, 160), head=(0, -3))
    if i == 1:
        return bird(sp, side=False, eyes="tight", mouth="yawn", wings=(100, 100), rot=-3)
    return bird(sp, side=False)


def bd_heart(sp, i):
    hs = dp.heart_burst(i)
    hy = FLOOR - 5 - 22 * _ST["body"] * 0.55
    held = [lambda: dp.held_heart(hy)]
    return bird(sp, side=False, eyes="happy", wings=(50, 50), held=held, fx=hs)


def bd_tickle(sp, i):
    return bird(sp, side=False, rot=dp._swing([-7, 7, 0][i]), eyes="happy", mouth="chirp" if i < 2 else "open",
                wings=([60, 120, 90][i], [120, 60, 90][i]))


def bd_hungry(sp, i):
    return bird(sp, side=False, rot=-4, eyes="closed", mouth=None, brow="sad", head=(0, [0, 1, 0][i] + 2),
                wings=(10, [26, 36, 26][i]), fx=seeds_bubble_fx(106, 18))


def bd_eat(sp, i):
    """콕콕 (그릇 없음): 0·2 = 고개를 푹 숙여 부리 끝이 그릇 밥 더미 (프레임 (64, 84))에 콕, 1·3 = 고개를 쏙 들고 ^^ (씨앗 부스러기가 튄다)."""
    peck = i % 2 == 0
    k = _ST["head"] ** 0.5
    mx, my = dp._frame_to_pre(64, 86)
    st_b = FLOOR - 5
    ry = 16 * _ST["body"] * _ST["torso"][1] * 1.2
    hr = 21.0 * _ST["head"]  # bird() 앞모습 머리 반지름
    by = st_b - ry
    hy0 = by - ry * 0.55 - hr * 0.62
    want = my - hr * 0.22 - 7.5 * k  # 부리 끝 = 머리 가운데 + 0.22 hr + 부리 길이
    fx = dp.eat_crumbs(i, SEED)
    return bird(sp, side=False, bottom=st_b, head=(0, want - hy0 if peck else -16 * _ST["head"]), eyes="closed" if peck else "happy", mouth=None,
                wings=(30, 30), sq=(1.0, 0.97 if peck else 1.0), fx=fx)


def bd_level_up(sp, i):
    stars = dp.level_up_stars(i)
    up = [150, 165, 175][i]
    return bird(sp, side=False, bottom=FLOOR - 5 - [0, 1, 3][i], eyes="happy", mouth="big", wings=(up, up), fx=stars)


BIRD_POSES = {"walk": bd_walk, "run": bd_run, "roll": bd_roll, "idle": bd_idle, "look": bd_look, "sleep": bd_sleep, "hop": bd_hop,
              "fall": bd_fall, "dragged": bd_dragged, "land": bd_land, "sit_front": bd_sit_front, "wave": bd_wave, "tilt": bd_tilt,
              "belly": bd_belly, "stretch": bd_stretch, "heart": bd_heart, "tickle": bd_tickle, "annoyed": bd_annoyed,
              "hungry": bd_hungry, "eat": bd_eat, "level_up": bd_level_up, "lookup": bd_lookup}
BIRD_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in BIRD_POSES.items()}


BIRD = Species(
    id="bird", name="새", walk_speed=110, scale=0.94, base=NAVY[3], back=NAVY[3], belly=CREAM, nose=BEAK, mouth="w",
    front_tail_color=NAVY[3], tail_color=NAVY[3], stage_size=dict.fromkeys(STAGES, 0.8), stripe=THROAT, outline=BIRD_LINE,
    stage_tweak={"baby": dict(pale=0.0, eye=1.2, fit=0.9), "kinder": dict(pale=0.0, eye=1.12, fit=0.93),
                 "child": dict(pale=0.0, eye=1.05), "adult": dict(fit=0.94)},
    specials=["lookup"], poses=BIRD_POSES, **dp.NO_COMMON_PARTS,
)


def register():
    dp.SPECIES[BIRD.id] = BIRD


register()


def main():
    import draw_halloween as dh
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(BIRD, root / "ui" / "pets" / BIRD.id))
    dh.preview_species(by_stage, pv / "stages_bird.png")
    print("ok: bird")


if __name__ == "__main__":
    main()
