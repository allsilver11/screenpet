"""마법사 🧙 공통 몸 — 검은 로브에 그리핀도르 목도리를 두르고 지팡이를 든 사람 모양 꼬마 (해리포터·헤르미온느, 나중에 볼드모트).
뼈대·IK는 슈퍼인간(draw_super)·박쥐인간·거미인간과 같은 틀이고, 부품마다 자기 외곽선을 두른 두 발 꼬마 — 팔다리는 관절을 IK로 굽힌다.
종마다 다른 것 (머리 모양·안경·흉터·피부·지팡이 색·배고플 때 생각하는 음식)은 Look으로 받는다 (draw_harry.py·draw_hermione.py).

생김새: 큰 머리 (키의 45% 남짓) + 작은 몸 + 짧은 팔다리 둘씩. 살색 얼굴 (draw_pets 단계 눈).
        몸: 검은 로브 (소매 끝까지 — 손은 맨손, 아랫단이 정강이까지 퍼지고 펄럭일 땐 진홍 안감), 앞섶 사이로 회색 스웨터,
        진홍 바탕에 금색 줄무늬 그리핀도르 목도리 (목에 감고 한쪽 끝이 늘어지거나 뒤로 휘날림), 회색 바지, 까만 구두, 나무 지팡이.
단계 (WIZ): 아기 = 동글 꼬마 (머리 아주 큼, 팔다리 뭉툭, 로브 짧음) → 어른 키 크고 로브가 김.
특기 동작: broom 🧹 빗자루에 걸터앉아 앞으로 숙이고 두 손으로 자루를 잡고 날기 (로브·목도리 펄럭 3칸) ·
      cast ✨ 지팡이를 어깨 뒤로 들었다가 휙 → 앞으로 쭉, 끝에 반짝 별 (지팡이 끝 = mark_grip).
      벽 타기 climb·slide = 뒷모습 (머리가 언제나 팔 위 — 팔이 머리를 덮지 않음).
"""

import math
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, _ST, anger_fx, question_fx, sparkle_fx, sweat_fx, view, xf, z_fx  # noqa: E402
from draw_spider import ik, polar, target  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS  # noqa: E402

FLOOR = 125.0  # 바닥 (구두 바닥)
ROBE = (56, 56, 72)  # 검은 로브 (몸·소매)
ROBE_FAR = (42, 42, 56)  # 먼 쪽 소매
ROBE_IN = (146, 30, 42)  # 로브 안감 (진홍 — 펄럭일 때 보임)
JUMPER = (124, 126, 140)  # 앞섶 사이 회색 스웨터
PANTS, PANTS_FAR = (88, 90, 104), (70, 72, 86)  # 회색 바지
SHOE, SHOE_FAR = (40, 36, 44), (30, 28, 34)  # 까만 구두
SHOE_HI = (104, 100, 116)  # 구두 윤기
SCARF = (162, 26, 40)  # 그리핀도르 목도리 진홍
SCARF_GOLD = (238, 186, 52)  # 목도리 금색 줄
SCARF_FAR = (128, 18, 32)  # 목도리 그늘 (뒤로 감긴 쪽)
MOUTH = (150, 64, 70)
BLUSH = (250, 156, 156)
OUT = (24, 22, 40)  # 외곽선
WIND = (186, 204, 236)  # 바람 자국
SPARK, SPARK_EDGE = (255, 246, 176), (240, 178, 60)  # 지팡이 끝 반짝
BROOM_WOOD, BROOM_WOOD_HI = (150, 96, 54), (196, 140, 88)  # 빗자루 자루
BRISTLE, BRISTLE_LINE = (222, 176, 92), (176, 128, 56)  # 빗자루 솔
BROOM_BAND = (120, 74, 40)  # 솔을 묶은 띠

# 단계(lvl)별 — hrx·hry: 머리 반지름 · tw: 어깨 반폭 · th: 몸 높이 · hip: 다리 뿌리 반간격 · leg: 다리 길이 · lw: 다리 굵기 ·
# arm: 팔 길이 · aw: 팔 굵기 · ear: 정수리 머리숱 높이 · robe: 로브 아랫단 길이 (다리 길이 몫) · wand: 지팡이 길이 · sw: 목도리 폭
WIZ = {
    0: dict(hrx=27.0, hry=25.0, tw=13.5, th=21.0, hip=6.6, leg=9.5, lw=10.4, arm=13.0, aw=8.8, ear=3.0, robe=0.62, wand=12.0, sw=6.0),
    1: dict(hrx=26.4, hry=24.2, tw=13.4, th=22.5, hip=6.8, leg=13.5, lw=9.8, arm=15.0, aw=8.3, ear=3.4, robe=0.66, wand=13.5, sw=6.2),
    2: dict(hrx=25.8, hry=23.4, tw=13.6, th=24.5, hip=7.0, leg=18.0, lw=9.4, arm=17.5, aw=7.9, ear=3.8, robe=0.7, wand=15.0, sw=6.4),
    3: dict(hrx=25.2, hry=22.8, tw=14.6, th=26.5, hip=7.2, leg=22.0, lw=9.2, arm=20.0, aw=7.6, ear=4.2, robe=0.72, wand=16.5, sw=6.8),
    4: dict(hrx=24.6, hry=22.2, tw=16.4, th=29.5, hip=8.0, leg=26.0, lw=9.6, arm=23.0, aw=7.9, ear=4.6, robe=0.74, wand=18.0, sw=7.2),
}
OVERLAP = 5.0  # 머리가 몸 윗부분을 덮는 깊이
SIDE_CX = 60.0  # 옆모습 엉덩이 x


@dataclass
class Look:
    """종마다 다른 생김새. hair(view_, hx, hy, hrx, hry, q) → (머리 도형에 더할 도형, 삐죽 조각, 머리 안 무늬) ·
    extra(view_, hx, hy, hrx, hry) → 얼굴 위 ops (안경·흉터) · food_fx(cx, cy, r) → 배고플 때 생각 풍선 fx."""
    hair: Callable
    hair_col: tuple
    skin: tuple = (255, 224, 202)
    skin_shade: tuple = (236, 190, 166)
    extra: Callable = None
    food_fx: Callable = None
    wand: tuple = (178, 128, 78)  # 지팡이 나무
    wand_tip: tuple = (236, 210, 168)
    side_ear: bool = True  # 옆모습에 귀가 보임 (긴 머리는 가림)
    cap_lift: float = 0.0  # 수면 모자를 머리숱 위로 올리는 몫 (머리 반지름 배)
    eye_k: float = 0.82  # 눈 크기 배
    back_reach: float = 0.0  # 뒷모습 벽 짚은 손을 더 바깥으로 (부푼 머리에 가리지 않게)
    extra_parts: dict = field(default_factory=dict)


_LOOK = {}  # 지금 그리는 종의 Look (poses_for가 칸마다 바꿈)


def look():
    """지금 그리는 종의 생김새."""
    return _LOOK["cur"]


def ws():
    """지금 단계의 꼬마 마법사 치수."""
    return WIZ[_ST["lvl"]]


def foot_y():
    """바닥에 선 발 관절 y (구두 바닥 = FLOOR)."""
    return FLOOR - ws()["lw"] * 0.5


def stand_hip_y():
    """똑바로 섰을 때 엉덩이 관절 y."""
    return foot_y() - ws()["leg"]


def reach(root, pt, ln, k=0.97):
    """root에서 pt 쪽으로 팔다리 길이(ln × k)를 넘지 않는 끝 자리 (짧은 아기 팔다리가 늘어나지 않게)."""
    dx, dy = pt[0] - root[0], pt[1] - root[1]
    d = math.hypot(dx, dy) or 1e-6
    m = min(1.0, ln * k / d)
    return (root[0] + dx * m, root[1] + dy * m)


def wave_line(a, b, n, depth, up):
    """a → b 사이를 n번 부드럽게 출렁이는 물결 (up: 들어가는 방향 단위 벡터)."""
    pts = []
    for j in range(n * 6 + 1):
        t = j / (n * 6)
        bump = depth * 0.5 * (1 - math.cos(2 * math.pi * n * t))
        pts.append((a[0] + (b[0] - a[0]) * t + up[0] * bump, a[1] + (b[1] - a[1]) * t + up[1] * bump))
    return pts


def limb(root, end, ln, w, color, end_col, prefer, *, shoe=False, toe=0.0, open_hand=False, point=False):
    """팔 또는 다리 한 짝 (자기 외곽선): 마디 둘 + 끝은 까만 구두 (옆모습은 코가 앞으로) 또는 살색 맨손 (동글 주먹).
    open_hand: 손바닥을 펴 손가락 셋 · point: 발끝을 아래로 (빗자루에 앉아 다리가 늘어질 때)."""
    j = ik(root, end, ln, prefer)
    shapes_ = [L([root, j, end], w)]
    marks = []
    if shoe:
        x, y = end
        if point:
            dx, dy = end[0] - j[0], end[1] - j[1]
            d = math.hypot(dx, dy) or 1.0
            tip = (x + dx / d * w * 0.55 + toe * w * 0.3, y + dy / d * w * 0.55)
            b = L([end, tip], w * 0.95)
        else:
            b = E(x - w * 0.6 + min(toe, 0) * w * 0.45, y - w * 0.42, x + w * 0.6 + max(toe, 0) * w * 0.45, y + w * 0.5)
        shapes_.append(b)
        marks.append(([b], end_col))
        if not point:
            gx = x + toe * w * 0.35
            marks.append(([L(dp.arc(gx, y - w * 0.05, w * 0.32, w * 0.2, 200, 260, 5), 1.2)], SHOE_HI))
    else:
        fist = C(end[0], end[1], w * (0.66 if open_hand else 0.6))
        shapes_.append(fist)
        hand = [fist]
        if open_hand:
            a = math.degrees(math.atan2(end[0] - j[0], end[1] - j[1]))
            for da in (-32, 0, 32):
                f = L([end, polar(end, a + da, w * 0.95)], w * 0.34)
                shapes_.append(f)
                hand.append(f)
        marks.append((hand, end_col))
    return Part(shapes_, color, marks, outline=True, grow=0), j


# =====================================================================
# 얼굴: 단계 눈 · 눈썹 · 입 · 볼 (머리 모양·안경·흉터는 Look)
# =====================================================================
def eye_ops(kind, cx, cy, flip, brow=None, sx=1.0):
    """눈 하나 (draw_pets 단계 눈) + 눈썹 (angry · sad). sx: 옆모습 먼 눈은 가로로 좁게."""
    k = look().eye_k
    with xf(sx=sx * k, sy=k, pivot=(cx, cy)):
        ops = dp.stage_eye(kind, cx, cy, flip)
        if brow:
            ops += dp.brows(brow, cx, cy + 2.0, flip)
    return ops


def mouth_ops(kind, cx, cy):
    """입: None 없음 · smile 빙긋 · open · big · grin · frown · pout · chew · o (동그랗게 오)."""
    if kind is None:
        return []
    if kind == "smile":
        return [([L(dp.arc(cx, cy - 1.6, 3.2, 2.4, 25, 155, 10), 1.4)], MOUTH)]
    if kind == "open":
        return dp.open_mouth(cx, cy - 2.2, 3.0, 3.2)
    if kind == "big":
        return dp.open_mouth(cx, cy - 2.8, 4.0, 4.4)
    if kind == "grin":
        return [([T(*dp.arc(cx, cy - 2.0, 4.6, 3.6, 0, 180, 12))], MOUTH), ([L([(cx - 3.6, cy - 1.4), (cx + 3.6, cy - 1.4)], 1.4)], (255, 255, 255))]
    if kind == "frown":
        return [([L(dp.arc(cx, cy + 1.8, 3.2, 2.0, 205, 335, 10), 1.4)], MOUTH)]
    if kind == "pout":
        return [([L(dp.arc(cx, cy + 1.2, 2.0, 1.4, 205, 335, 8), 1.3)], MOUTH)]
    if kind == "chew":
        return [([E(cx - 2.2, cy - 1.3, cx + 2.2, cy + 1.3)], MOUTH)]
    if kind == "o":
        return [([E(cx - 1.8, cy - 2.4, cx + 1.8, cy + 1.2)], MOUTH)]
    raise ValueError(kind)


def eye_spots(view_, hx, hy, look_=(0.0, 0.0)):
    """눈 가운데 자리 [먼/왼, 가까운/오른] (안경 테가 이 자리를 두름)."""
    s = ws()
    hrx, hry = s["hrx"], s["hry"]
    lx, ly = look_
    fy = hy + 0.02 * hry + ly
    if view_ == "front":
        return [(hx - 0.36 * hrx + lx, fy), (hx + 0.36 * hrx + lx, fy)]
    return [(hx + 0.82 * hrx + lx * 0.6, fy - 0.5), (hx + 0.4 * hrx + lx, fy)]


def face_ops(view_, hx, hy, eyes, brow, mouth, look_, blush_on):
    """얼굴 ops (눈 둘 + 볼 + 입, 옆모습은 귀도) — 머리 변환 안에서 부름. 옆모습은 얼굴이 오른쪽으로 쏠리고 먼 눈은 좁게.
    Look.extra (안경·흉터)를 맨 위에 얹음."""
    s = ws()
    lk = look()
    hrx, hry = s["hrx"], s["hry"]
    lx, _ = look_
    ops = []
    (ex0, ey0), (ex1, ey1) = eye_spots(view_, hx, hy, look_)
    if view_ == "front":
        ops += eye_ops(eyes, ex0, ey0, False, brow)
        ops += eye_ops(eyes, ex1, ey1, True, brow)
        if blush_on:
            ops += [([E(hx + side * 0.56 * hrx - 3.2, hy + 0.44 * hry - 1.6, hx + side * 0.56 * hrx + 3.2, hy + 0.44 * hry + 1.4)], BLUSH)
                    for side in (-1, 1)]
        mx, my = hx + lx * 0.5, hy + 0.62 * hry
    else:
        if lk.side_ear:
            ear = (hx - 0.12 * hrx, hy + 0.14 * hry)
            ops.append(([E(ear[0] - 3.4, ear[1] - 4.8, ear[0] + 3.4, ear[1] + 4.8)], lk.skin_shade))
            ops.append(([E(ear[0] - 2.0, ear[1] - 3.2, ear[0] + 2.0, ear[1] + 3.2)], lk.skin))
        ops += eye_ops(eyes, ex0, ey0, True, brow, sx=0.5)
        ops += eye_ops(eyes, ex1, ey1, False, brow, sx=0.85)
        if blush_on:
            ops.append(([E(hx + 0.42 * hrx - 2.8, hy + 0.46 * hry - 1.6, hx + 0.42 * hrx + 2.8, hy + 0.46 * hry + 1.4)], BLUSH))
        mx, my = hx + 0.72 * hrx, hy + 0.62 * hry
    ops += mouth_ops(mouth, mx, my)
    if lk.extra:
        ops += lk.extra(view_, hx, hy, hrx, hry, (ex0, ey0), (ex1, ey1))
    return ops


# =====================================================================
# 그리핀도르 목도리 · 로브 · 지팡이
# =====================================================================
def strip_pts(root, a, length, w, flap=0.0, n=8, taper=1.0):
    """root에서 a도 (0 = 아래, 90 = 오른쪽, -90 = 왼쪽) 쪽으로 length만큼 늘어지는 폭 w 띠의 가운데 선 점들 (flap: 물결)."""
    pts = []
    for j in range(n + 1):
        t = j / n
        p = polar(root, a, length * t)
        off = flap * math.sin(t * math.pi * 1.6) * t
        nx, ny = math.cos(math.radians(a)), -math.sin(math.radians(a))  # 가운데 선에 수직
        pts.append((p[0] + nx * off, p[1] + ny * off))
    return pts


def scarf_tail(root, a, length, w, flap=0.0, color=SCARF):
    """목도리 늘어진 끝 (진홍 바탕 + 금색 줄 셋 + 끝 술): a도 쪽으로 length만큼, 칸마다 flap으로 펄럭."""
    c = strip_pts(root, a, length, w, flap)
    left, right = [], []
    for j in range(len(c)):
        p0 = c[max(0, j - 1)]
        p1 = c[min(len(c) - 1, j + 1)]
        dx, dy = p1[0] - p0[0], p1[1] - p0[1]
        d = math.hypot(dx, dy) or 1.0
        nx, ny = -dy / d, dx / d
        left.append((c[j][0] + nx * w / 2, c[j][1] + ny * w / 2))
        right.append((c[j][0] - nx * w / 2, c[j][1] - ny * w / 2))
    marks = []
    n = len(c) - 1
    for t0 in (0.28, 0.52, 0.76):
        j0, j1 = int(t0 * n), int(t0 * n) + 1
        marks.append(([T(left[j0], left[j1], right[j1], right[j0])], SCARF_GOLD))
    end = c[-1]
    fringe = []
    dx, dy = c[-1][0] - c[-2][0], c[-1][1] - c[-2][1]
    d = math.hypot(dx, dy) or 1.0
    for k in (-0.34, 0.0, 0.34):
        fx_ = end[0] + (-dy / d) * w * k
        fy_ = end[1] + (dx / d) * w * k
        fringe.append(L([(fx_, fy_), (fx_ + dx / d * 3.2, fy_ + dy / d * 3.2)], 1.4))
    return Part([T(*(left + right[::-1]))], color, marks + [(fringe, SCARF_GOLD)], outline=True, grow=0, spikes=fringe)


def scarf_band(view_, cx, top, tw):
    """목에 감은 목도리 (몸 윗부분 앞에 두툼한 띠 + 금색 줄)."""
    s = ws()
    h = s["sw"]
    if view_ == "side":
        x0, x1 = cx - tw * 0.72, cx + tw * 0.86
    else:
        x0, x1 = cx - tw * 0.92, cx + tw * 0.92
    y0, y1 = top - h * 0.2, top + h * 0.95
    band = E(x0, y0, x1, y1)
    marks = []
    for k in (-0.5, 0.0, 0.5) if view_ != "side" else (-0.35, 0.3):
        gx = (x0 + x1) / 2 + k * (x1 - x0) * 0.5
        marks.append(([L([(gx - 0.8, y0 - 1), (gx + 0.8, y1 + 1)], 1.6)], SCARF_GOLD))
    return Part([band], SCARF, marks, outline=True, grow=0)


def wand_part(hand, a, length):
    """지팡이 (손에서 a도 쪽으로 length — 녹이지 않는 가는 막대, 끝은 밝게). 끝 자리도 돌려줌."""
    lk = look()
    tip = polar(hand, a, length)
    mid = polar(hand, a, length * 0.72)
    return Part([L([hand, tip], 2.3)], lk.wand, [([L([mid, tip], 2.3)], lk.wand_tip)], grow=0, sharp=True), tip


# =====================================================================
# 꼬마 마법사 한 장
# =====================================================================
def shoulders(view_, hip, rot):
    """어깨 자리 [먼/왼, 가까운/오른] (프레임 좌표)."""
    s = ws()
    hx0, hy0 = hip
    top = hy0 + s["lw"] * 0.35 - s["th"]
    bm = dp._xf_matrix(rot=rot, pivot=(hx0, hy0))
    if view_ == "side":
        pts = [(hx0 - 1.5, top + s["aw"] * 0.75), (hx0 + 1.0, top + s["aw"] * 0.8)]
    else:
        pts = [(hx0 - (s["tw"] - s["aw"] * 0.45), top + s["aw"] * 0.6), (hx0 + (s["tw"] - s["aw"] * 0.45), top + s["aw"] * 0.6)]
    return [dp._apply(bm, p) for p in pts]


def head_center(view_, hip, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0)):
    """머리 가운데 (프레임 좌표) — wizard와 같은 계산."""
    s = ws()
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    side = view_ == "side"
    neck = dp._apply(dp._xf_matrix(rot=rot, pivot=hip), (hip[0] + (1.5 if side else 0.0), top))
    c = (neck[0] + head_d[0] + (2.0 if side else 0.0), neck[1] - s["hry"] + OVERLAP + head_d[1])
    return dp._apply(dp._xf_matrix(rot=rot + head_rot, pivot=neck), c)


def robe_side(hx0, waist, bottom, tw, robe):
    """옆모습 로브 자락 (허리에서 아래로, 펄럭이면 뒤로 휘날림 — 안감 진홍): robe = dict(a=뒤로 날림 각도, flap, len=길이 배)."""
    s = ws()
    a, flap = robe.get("a", 6.0), robe.get("flap", 0.0)
    ln = s["leg"] * s["robe"] * robe.get("len", 1.0) + (bottom - waist)
    fw, bw = (hx0 + tw * 0.86, waist), (hx0 - tw * 0.92, waist)
    fh = polar(fw, -a * 0.55 + 4, ln * 0.98)
    bh = polar(bw, -a - 6, ln * 1.04 + flap * 0.6)
    if robe.get("floor") is not None:  # 앉았을 때: 바닥 아래로 내려가지 않게
        fl = robe["floor"]
        fh, bh = (fh[0], min(fh[1], fl)), (bh[0], min(bh[1], fl))
    hem = wave_line(fh, bh, 2, 2.2 + abs(flap) * 0.5, (0, -1))
    pts = [fw] + hem + [bw]
    marks = []
    if a > 25:  # 휘날리면 안감이 보임
        marks.append(([T(bh, polar(bw, -a - 2, ln * 0.55), (hem[len(hem) // 2][0], hem[len(hem) // 2][1] - 2))], ROBE_IN))
    return pts, marks


def wizard(*, view_="side", hip=None, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0), legs=None, arms=None, leg_bend=None, arm_bend=None,
           eyes="dot", brow=None, mouth="smile", look_=(0.0, 0.0), blush_on=True, fx=(), ops=(), held=(), whole=None, grip=None,
           arms_front=False, arms_behind=False, far_limbs_front=False, near_arm_back=False, robe=None, scarf=None, open_hand=(False, False),
           point=False, wand=None, mid=(), behind=()):
    """꼬마 마법사. view_: side (오른쪽을 봄) · front · back · hip: 엉덩이 관절 · rot: 몸 기울기 (엉덩이 기준, 시계 방향) ·
    head_rot·head_d: 고개 · legs·arms: [먼/왼, 가까운/오른] 끝 자리 (target 형식) · leg_bend·arm_bend: 무릎·팔꿈치 굽는 쪽 ·
    held: 몸과 머리 사이 Part 함수 · fx: 효과 목록 또는 손 자리를 받아 효과를 돌려주는 함수 · grip: mark_grip할 손 ("near"·"far"·"left"·"right") ·
    arms_front: 앞모습 팔을 머리 위에 · arms_behind: 옆모습 가까운 팔도 머리 뒤에 · far_limbs_front: 먼 팔도 몸 앞에 ·
    near_arm_back: 옆모습 가까운 팔을 몸 뒤에 · robe: 로브 dict(a=뒤로 날림, flap, len, sway, floor) · scarf: 목도리 끝 dict(a=각도, flap, len) 또는 False ·
    wand: 지팡이 dict(hand=0/1, a=각도, k=길이 배, grip=끝에 mark_grip) · mid: 먼 다리 뒤·가까운 다리 앞 사이 Part (빗자루) ·
    behind: 맨 뒤 Part 함수 · point: 발끝을 아래로."""
    s = ws()
    lk = look()
    side_view = view_ == "side"
    hx0, hy0 = hip or ((SIDE_CX if side_view else 64.0), stand_hip_y())
    lw, aw = s["lw"], s["aw"]
    bottom = hy0 + lw * 0.35
    top = bottom - s["th"]
    robe = {} if robe is None else robe
    scarf = {} if scarf is None else scarf
    with xf(**(whole or {})):
        bm = dp._xf_matrix(rot=rot, pivot=(hx0, hy0))
        P = lambda p: dp._apply(bm, p)  # noqa: E731
        if side_view:
            leg_roots = [P((hx0 - 2.0, hy0)), P((hx0 + 2.0, hy0))]
            leg_sides, arm_sides = (1, 1), (1, 1)
            leg_pref = leg_bend or [(1, -0.2), (1, -0.2)]
            arm_pref = arm_bend or [(-1, 0.4), (-1, 0.4)]
        else:
            leg_roots = [P((hx0 - s["hip"], hy0)), P((hx0 + s["hip"], hy0))]
            leg_sides, arm_sides = (-1, 1), (-1, 1)
            leg_pref = leg_bend or [(-1, 0), (1, 0)]
            arm_pref = arm_bend or [(-1, 0.3), (1, 0.3)]
        sh_roots = shoulders(view_, (hx0, hy0), rot)
        if legs is None:
            fy = foot_y()
            legs = [(leg_roots[0][0] - 2, fy), (leg_roots[1][0] + 2, fy)] if side_view else \
                [(leg_roots[0][0] - 2.6, fy), (leg_roots[1][0] + 2.6, fy)]
        if arms is None:
            arms = [("a", 14, 0.98), ("a", 10, 0.98)] if side_view else [("a", 14, 0.98), ("a", 14, 0.98)]
        leg_parts, arm_parts, hands = [], [], []
        for j in range(2):
            far = side_view and j == 0
            end = target(leg_roots[j], legs[j], s["leg"], leg_sides[j])
            p, _ = limb(leg_roots[j], end, s["leg"], lw, PANTS_FAR if far else PANTS, SHOE_FAR if far else SHOE, leg_pref[j], shoe=True,
                        toe=(1 if side_view else 0) if view_ != "back" else 0, point=point)
            leg_parts.append(p)
            end = target(sh_roots[j], arms[j], s["arm"], arm_sides[j])
            p, _ = limb(sh_roots[j], end, s["arm"], aw, ROBE_FAR if far else ROBE, lk.skin_shade if far else lk.skin, arm_pref[j],
                        open_hand=open_hand[j])
            arm_parts.append(p)
            hands.append(end)
        wand_p = None
        if wand:
            hi = wand.get("hand", 1)
            wand_p, tip = wand_part(hands[hi], wand.get("a", 150), s["wand"] * wand.get("k", 1.0))
            if wand.get("grip"):
                dp.mark_grip(*tip)
        th = s["th"]
        # 몸통 (로브 + 앞섶 스웨터 + 로브 자락)
        with xf(rot=rot, pivot=(hx0, hy0)):
            tw = s["tw"] if not side_view else s["tw"] * 0.95
            waist = bottom - th * 0.3
            if _ST["lvl"] == 0:
                tshape = [E(hx0 - tw * 1.04, top, hx0 + tw * 1.04, bottom + 1.5)]
            else:
                tshape = [E(hx0 - tw, top, hx0 + tw, top + th * 0.78), E(hx0 - tw * 0.84, top + th * 0.32, hx0 + tw * 0.84, bottom)]
            marks = []
            if side_view:
                pts, rmarks = robe_side(hx0, waist, bottom, tw, robe)
                tshape.append(T(*pts))
                marks += rmarks
                # 앞섶 (가슴 앞쪽 가장자리에 회색 스웨터 한 줄)
                marks.append(([L([(hx0 + tw * 0.86, top + 3), (hx0 + tw * 0.9, waist)], 2.6)], JUMPER))
            else:
                ln = s["leg"] * s["robe"] * robe.get("len", 1.0) + (bottom - waist)
                hem_y = waist + ln
                if robe.get("floor") is not None:
                    hem_y = min(hem_y, robe["floor"])
                sway = robe.get("sway", 0.0)
                spread = tw * 0.3 + s["leg"] * 0.12
                r_, l_ = (hx0 + tw * 0.92 + spread + sway, hem_y), (hx0 - tw * 0.92 - spread + sway, hem_y)
                hem = wave_line(r_, l_, 3, min(2.6, ln * 0.12), (0, -1))
                tshape.append(T((hx0 - tw * 0.84, waist), (hx0 + tw * 0.84, waist), *hem))
                if view_ == "front":
                    # 앞섶이 열려 회색 스웨터가 보임 (목도리 아래에서 허리까지 좁아짐) + 아랫단까지 여밈 선
                    marks.append(([T((hx0 - tw * 0.36, top), (hx0 + tw * 0.36, top), (hx0 + 1.4, waist + 1), (hx0 - 1.4, waist + 1))], JUMPER))
                    marks.append(([L([(hx0, waist), (hx0 + sway * 0.6, hem_y + 1)], 1.3)], OUT))
                else:
                    marks.append(([L([(hx0, top + 6), (hx0 + sway * 0.6, hem_y)], 1.1)], ROBE_FAR))
            torso = Part(tshape, ROBE, marks, outline=True, grow=0)
            neck = (hx0 + (1.5 if side_view else 0.0), top)
            band = scarf_band(view_, neck[0], top + 1.0, tw)
            tail = None
            if scarf is not False:
                sl = (th * 0.85 + s["leg"] * 0.25) * scarf.get("len", 1.0)
                if side_view:
                    a = scarf.get("a", -12.0)
                    root = (neck[0] - tw * 0.4, top + s["sw"] * 0.4) if a < -25 else (neck[0] + tw * 0.25, top + s["sw"] * 0.5)
                    tail = scarf_tail(root, a, sl, s["sw"] * 0.9, scarf.get("flap", 0.0), SCARF if a > -25 else SCARF)
                elif view_ == "front":
                    tail = scarf_tail((neck[0] - tw * 0.4, top + s["sw"] * 0.5), scarf.get("a", -4.0), sl * 0.9, s["sw"] * 1.1,
                                      scarf.get("flap", 0.0))
                else:
                    tail = scarf_tail((neck[0] - tw * 0.3, top + s["sw"] * 0.5), scarf.get("a", -4.0), sl * 0.8, s["sw"] * 0.9,
                                      scarf.get("flap", 0.0))
        neck = P(neck)
        # 머리 (살색 얼굴 + Look의 머리 모양)
        hrx, hry = s["hrx"], s["hry"]
        with xf(rot=rot + head_rot, pivot=neck):
            hx, hy = neck[0] + head_d[0] + (2.0 if side_view else 0.0), neck[1] - hry + OVERLAP + head_d[1]
            hshapes, spikes, hmarks = lk.hair(view_, hx, hy, hrx, hry, s["ear"])
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)] + hshapes, lk.skin, hmarks, outline=True, grow=0, spikes=spikes)
            dp.mark_head(hx, hy - hry + 2 - lk.cap_lift * hry, hrx * 1.7)
            face = [] if view_ == "back" else face_ops(view_, hx, hy, eyes, brow, mouth, look_, blush_on)
        parts = [b() for b in behind]
        tail_behind = tail is not None and (view_ == "back" or (side_view and scarf.get("a", -12.0) < -25))
        if side_view:
            parts += [tail] if tail_behind else []
            parts += [arm_parts[0]] if not far_limbs_front else []
            parts += [wand_p] if wand_p and wand.get("hand", 1) == 0 and not far_limbs_front else []
            parts += [arm_parts[1]] if near_arm_back else []
            parts += [leg_parts[0]] + [m() for m in mid] + [leg_parts[1], torso, band]
            parts += [tail] if tail is not None and not tail_behind else []
            if arms_behind and not near_arm_back:
                parts += [arm_parts[1]] + ([wand_p] if wand_p and wand.get("hand", 1) == 1 else [])
            parts += [h() for h in held]
            parts += [head]
            parts += ([arm_parts[0]] if far_limbs_front else []) + ([wand_p] if wand_p and wand.get("hand", 1) == 0 and far_limbs_front else [])
            if not (arms_behind or near_arm_back):
                parts += [arm_parts[1]] + ([wand_p] if wand_p and wand.get("hand", 1) == 1 else [])
            elif near_arm_back and wand_p and wand.get("hand", 1) == 1:
                parts.insert(parts.index(arm_parts[1]) + 1, wand_p)
        else:
            # 뒷모습은 머리가 팔을 가림 (팔을 머리 위에 그리지 않음 — 벽 타기에서 팔이 머리를 덮던 실수)
            if view_ == "back":
                arms_front = False
            parts += leg_parts + [torso]
            parts += [tail] if tail is not None and view_ == "back" else []
            parts += [band]
            parts += [tail] if tail is not None and view_ == "front" else []
            if not arms_front:
                parts += arm_parts + ([wand_p] if wand_p else [])
            parts += [h() for h in held]
            parts += [head]
            if arms_front:
                parts += arm_parts + ([wand_p] if wand_p else [])
        if grip:
            idx = {"far": 0, "near": 1, "left": 0, "right": 1}[grip]
            dp.mark_grip(*hands[idx])
    if callable(fx):
        fx = fx(hands)
    # 실루엣 바탕·외곽선 색은 맨 앞 부품을 따름: 뒤로 휘날린 목도리가 맨 앞이어도 로브 색으로 (빈 바탕 조각)
    return dp.compose([Part([], ROBE, grow=0)] + parts, face + list(ops), fx)


def H(x=SIDE_CX, dy=0.0):
    """엉덩이 관절 자리 (x, 선 높이 + dy)."""
    return (x, stand_hip_y() + dy)


def floor_at(x, lift=0.0):
    """바닥에 닿은 발 관절 (구두 바닥 = FLOOR - lift)."""
    return (x, foot_y() - lift)


def torso_top(hip_y=None):
    """몸 위쪽 y."""
    s = ws()
    return (stand_hip_y() if hip_y is None else hip_y) + s["lw"] * 0.35 - s["th"]


def reach_all(hip, rot, ends, view_="side"):
    """두 팔 끝을 팔 길이 안으로."""
    return [reach(sh, e, ws()["arm"], 1.0) for sh, e in zip(shoulders(view_, hip, rot), ends)]


def wand_down(a=40):
    """평소 지팡이: 가까운 손에 쥐고 끝이 앞아래 (a도 — 0 = 아래, 90 = 앞)."""
    return dict(hand=1, a=a, k=0.9)


# =====================================================================
# 옆모습 (오른쪽을 본다)
# =====================================================================
WALK = [(-7, 7, 0, 3), (0, 0, 2.5, 0), (7, -7, 3, 0), (0, 0, 0, 2.5)]  # (먼 발 dx, 가까운 발 dx, 먼 발 듦, 가까운 발 듦)


def walk_legs(i):
    """걷기 i칸의 두 발 자리와 엉덩이."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * ws()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    return hip, [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]


def wz_walk(sp, i):
    """총총 걷기: 두 다리를 번갈아 앞뒤로, 팔은 반대로 흔듦 (가까운 손에 지팡이), 로브 자락·목도리 끝 살랑."""
    hip, legs = walk_legs(i)
    sw = [1, 0, -1, 0][i]
    arms = [("a", 22 * sw, 0.96), ("a", -22 * sw + 4, 0.96)]
    return wizard(hip=hip, legs=legs, arms=arms, rot=2, robe=dict(a=[8, 12, 8, 12][i], flap=[0, 1.2, 0, -1.2][i]),
                  scarf=dict(a=[-8, -14, -8, -14][i], flap=[0, 1, 0, -1][i]), wand=dict(hand=1, a=48 - 16 * sw, k=0.85))


def wz_run(sp, i):
    """다다다 달리기: 앞으로 기울고 크게 성큼, 로브가 뒤로 휘날려 진홍 안감, 목도리 끝이 뒤로, 속도선."""
    st = [1, 0, -1, 0][i]
    hip = H(dy=[-1.5, -3.5, -1.5, -3.5][i])
    legs = [("a", -40 * st, 0.95 if st else 0.75), ("a", 40 * st, 0.95 if st else 0.75)]
    if st == 0:
        legs = [("a", -25, 0.8), ("a", 35, 0.7)] if i == 1 else [("a", 35, 0.7), ("a", -25, 0.8)]
    arms = [("a", 55 * st + 5, 0.8), ("a", -45 * st + 5, 0.8)]
    return wizard(hip=hip, legs=legs, arms=arms, rot=12, mouth="open", arm_bend=[(1, 0.3), (1, 0.3)],
                  robe=dict(a=[40, 48, 40, 48][i], flap=[1.5, -1.5, 1.5, -1.5][i]), scarf=dict(a=-70, flap=[2, -2, 2, -2][i], len=0.9),
                  fx=dp.speed_fx(4, (70, 86, 102), 10, [0, -3, 0, -3][i]))


def wz_idle(sp, i):
    """제자리 숨쉬기: 가슴이 살짝 오르내림, 지팡이는 가까운 손에 아래로."""
    return wizard(hip=H(dy=[0, 0.8][i]), arms=[("a", 12, 0.97), ("a", 8 + 3 * i, 0.97)], head_d=(0, [0, 0.6][i]),
                  robe=dict(a=4 + 2 * i, flap=i * 0.6), scarf=dict(a=-6 - 2 * i), wand=wand_down(36 + 4 * i))


def wz_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞 (고개만)."""
    rot, eyes, lk = [(-8, "up", (0.5, -1.0)), (0, "dot", (1.2, 0)), (8, "dot", (0.6, 1.5)), (0, "closed", (0, 0))][i]
    return wizard(head_rot=rot, eyes=eyes, look_=lk, wand=wand_down())


def sit_hip(x=SIDE_CX):
    """바닥에 앉은 엉덩이 관절."""
    return (x, FLOOR - ws()["lw"] * 0.75)


def wz_sleep(sp, i):
    """로브를 덮고 웅크려 쿨쿨: 바닥에 앉아 무릎을 끌어안고 고개를 무릎에 묻음, 눈 ‿, z."""
    s = ws()
    hip = sit_hip(SIDE_CX - 6)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 3, foot_y()), (knee_x + 5, foot_y())]
    arms = [(knee_x + 1, hip[1] - s["leg"] * 0.45 - 4), (knee_x + 3, hip[1] - s["leg"] * 0.45 - 2)]
    fx = [z_fx(106, 34 - 3 * i, 4.0), z_fx(116, 20 - 3 * i, 2.6)]
    return wizard(hip=hip, rot=14 + i, head_rot=10, head_d=(2, 1 + 0.6 * i), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                  arm_bend=[(0, 1), (0, 1)], eyes="closed", mouth=None, fx=fx, robe=dict(a=-30, len=0.5, floor=FLOOR - 1), scarf=dict(a=8, len=0.6))


def wz_annoyed(sp, i):
    """털썩 앉아 팔짱 끼고 눈 반쯤 감은 뾰로통 💢."""
    s = ws()
    hip = sit_hip(SIDE_CX - 4)
    legs = [(hip[0] + s["leg"] + 4, foot_y()), (hip[0] + s["leg"] + 7, foot_y())]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms = [(hip[0] + 8, top + s["th"] * 0.45), (hip[0] + 9, top + s["th"] * 0.5)]
    return wizard(hip=hip, rot=-6, legs=legs, arms=arms, arm_bend=[(1, 0.8), (1, 0.8)], eyes="flat", mouth="pout", head_rot=-4,
                  fx=[anger_fx(102 + i, 30 - i, 5.0 + 0.6 * i)], robe=dict(a=-40, len=0.4, floor=FLOOR - 1), scarf=dict(a=6, len=0.6))


def wz_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 지팡이 든 손을 위로 번쩍 솟음 (다리 모음) → 다리 내림."""
    s = ws()
    if i == 0:
        hip = H(dy=s["leg"] * 0.32)
        return wizard(hip=hip, rot=12, legs=[floor_at(SIDE_CX - 3), floor_at(SIDE_CX + 3)], arms=[("a", -30, 0.9), ("a", -25, 0.9)],
                      eyes="tight", mouth="open", robe=dict(a=10, len=0.8), scarf=dict(a=-20))
    hip = (SIDE_CX, [0, 84, 90][i] - (s["leg"] - 22) * 0.5)
    legs = [("a", -30, 0.7), ("a", 25, 0.62)] if i == 1 else [("a", -8, 0.92), ("a", 10, 0.9)]
    arms = [("a", 20, 0.9), ("a", 165, 0.98)] if i == 1 else [("a", 30, 0.9), ("a", 95, 0.95)]
    return wizard(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="open", arm_bend=[(1, 0), (1, 0)], arms_behind=i == 1,
                  robe=dict(a=[0, -14, 20][i], flap=1.5), scarf=dict(a=[0, 20, -30][i], flap=2), wand=dict(hand=1, a=[0, 172, 100][i], k=0.85))


def wz_fall(sp, i):
    """떨어짐: 팔다리를 허우적, 동그란 눈, 로브·목도리가 위로 펄럭."""
    w = [0, 1][i]
    hip = (SIDE_CX, 84.0)
    legs = [("a", -40 - 15 * w, 0.85), ("a", 40 + 15 * w, 0.75)]
    arms = [("a", 150 - 25 * w, 0.95), ("a", 125 + 25 * w, 0.95)]
    return wizard(hip=hip, legs=legs, arms=arms, eyes="round", mouth="open", rot=[-6, 6][i], arm_bend=[(0, -1), (0, -1)], arms_behind=True,
                  robe=dict(a=[60, 54][i], flap=[2, -2][i], len=0.8), scarf=dict(a=[160, 150][i], flap=[2, -2][i], len=0.8))


def wz_dragged(sp, i):
    """목덜미가 잡혀 대롱대롱: 팔다리가 축 늘어지고 살랑."""
    s = ws()
    sway = [-6, 6][i]
    top_y = 22.0 + s["ear"] + look().cap_lift * s["hry"]
    hip_y = top_y + 2 * s["hry"] - OVERLAP + s["th"] - s["lw"] * 0.35 - 2
    hip = (SIDE_CX + 2, hip_y)
    legs = [("a", -6, 0.97), ("a", 8, 0.95)]
    arms = [("a", 12, 0.95), ("a", 18, 0.95)]
    return wizard(hip=hip, legs=legs, arms=arms, eyes="round", mouth="pout", whole=dict(rot=sway * 0.5, pivot=(SIDE_CX, 14)),
                  fx=sweat_fx(104, 30), robe=dict(a=-4, len=0.9), scarf=dict(a=4, len=0.8), wand=wand_down(10))


def wz_land(sp, i):
    """착지 (한 번): 무릎을 굽혀 쿵 → 바로 섬."""
    s = ws()
    if i == 0:
        return wizard(hip=H(dy=s["leg"] * 0.25), rot=8, legs=[floor_at(SIDE_CX - 5), floor_at(SIDE_CX + 5)], arms=[("a", 50, 0.9), ("a", 44, 0.9)],
                      eyes="tight", mouth="open", robe=dict(a=-10, len=0.8, floor=FLOOR - 1), scarf=dict(a=30, flap=2))
    return wizard(eyes="happy", robe=dict(a=6, flap=1), scarf=dict(a=-6), wand=wand_down())


def wz_lookup(sp, i):
    """올려다보기: 고개를 젖혀 눈이 위로, 반짝."""
    return wizard(head_rot=dp.lookup_tilt(i), eyes="up", look_=(0.6, -1.0), fx=dp.lookup_fx(i, 110, 14), wand=wand_down())


# =====================================================================
# 앞모습
# =====================================================================
def front(**kw):
    """앞모습 꼬마 마법사."""
    return wizard(view_="front", **kw)


def F(dy=0.0):
    """앞모습 엉덩이 관절."""
    return (64.0, stand_hip_y() + dy)


def wz_sit_front(sp, i):
    """정면 서기: 지팡이를 오른손에 아래로 쥐고 다른 손은 옆에 (마지막 장 깜빡)."""
    return front(arms=[("a", 12, 0.97), ("a", 16, 0.97)], eyes="closed" if i == 3 else "dot", head_d=(0, [0, 0.5, 0, 0][i]),
                 robe=dict(sway=[0, 0.5, 0, -0.5][i]), scarf=dict(flap=[0, 0.6, 0, -0.6][i]), wand=dict(hand=1, a=24, k=0.85))


def wz_wave(sp, i):
    """한 손을 번쩍 들어 흔들흔들 (다른 손에 지팡이)."""
    a = [118, 140, 118, 140][i]
    return front(arms=[("a", 16, 0.95), ("a", a, 1.0)], arm_bend=[(-1, 0.3), (1, 0)], eyes="happy" if i % 2 else "dot",
                 mouth="open", head_rot=[-3, -1, -3, -1][i], open_hand=(False, True), wand=dict(hand=0, a=-30, k=0.85), arms_front=True)


def wz_tilt(sp, i):
    """고개 갸웃 + ? (한 손은 턱)."""
    top = torso_top()
    return front(head_rot=[8, 12, 12][i], arms=[("a", 15, 0.95), (64 + 6, top + 2)], arm_bend=[(-1, 0.3), (1, 0.6)], arms_front=True,
                 fx=[question_fx(106, 24 - i)], mouth="pout")


def wz_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 두 다리를 위로 버둥, 두 팔 번쩍, 깔깔."""
    s = ws()
    w = [0, 2, 0, -2][i]
    hip = (64.0, FLOOR - s["lw"] * 0.8)
    legs = [("a", 112 + 6 * w, 0.9), ("a", 112 - 6 * w, 0.9)]
    arms = [("a", 128 - 5 * w, 0.95), ("a", 128 + 5 * w, 0.95)]
    return front(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="big", robe=dict(len=0.3, floor=FLOOR - 1), scarf=dict(a=-60, len=0.6))


def wz_stretch(sp, i):
    """기지개 (한 번): 두 팔 쭉 위로 → 하품하며 팔 벌림 → 서기."""
    if i == 0:
        return front(arms=[("a", 140, 1.0), ("a", 140, 1.0)], eyes="tight", hip=F(-1.5), arms_front=True)
    if i == 1:
        return front(arms=[("a", 100, 1.0), ("a", 100, 1.0)], eyes="closed", mouth="big")
    return front(eyes="happy", wand=dict(hand=1, a=24, k=0.85))


def wz_heart(sp, i):
    """하트를 가슴에 꼭 안고 눈 ^^, 작은 하트가 떠오름."""
    s = ws()
    hy = torso_top() + s["th"] * 0.45

    def held():
        """가슴에 안은 하트."""
        return dp.held_heart(hy - 4)
    return front(arms=[(58, hy + 3), (70, hy + 3)], arm_bend=[(-1, 0.6), (1, 0.6)], held=[held], eyes="happy", fx=dp.heart_burst(i), scarf=False)


def wz_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 배를 감싸고 깔깔 (> <)."""
    s = ws()
    y = torso_top() + s["th"] * 0.6
    return front(rot=[-7, 7, -3][i], arms=[(60, y), (68, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="tight", mouth="big")


def wz_hungry(sp, i):
    """배고파 시무룩: 두 손으로 배를 감싸고 처진 눈썹, 좋아하는 음식 생각 풍선 (Look.food_fx)."""
    s = ws()
    y = torso_top() + s["th"] * 0.68
    fx = look().food_fx(104 - 2 * i, 20, 12.0) if look().food_fx else []
    return front(hip=F([0, 1, 0][i]), arms=[(59, y), (69, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="dot", brow="sad", mouth="frown",
                 head_rot=-3, fx=fx)


EAT_MOUTH = 79.0  # 먹을 때 입 (프레임 y) — 그릇 밥 더미 바로 위


def wz_eat(sp, i):
    """냠냠 (그릇 없음): 무릎 꿇고 고개를 숙여 입을 밥 더미 바로 위에 — 앙 ↔ 오물오물, 부스러기."""
    s = ws()
    hip_y = EAT_MOUTH + [0, 1, 0, 1][i] - 0.62 * s["hry"] + s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]
    hip_y = min(hip_y, FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - 3, foot_y()), (64 + s["hip"] + 3, foot_y())]
    arms = [(44, 96), (84, 96)]
    return front(hip=(64.0, hip_y), legs=legs, arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="happy" if i % 2 else "dot",
                 mouth="big" if i % 2 == 0 else "chew", fx=dp.eat_crumbs(i), robe=dict(len=0.6, floor=FLOOR - 1), scarf=False)


def wz_level_up(sp, i):
    """만세: 두 팔 번쩍 (지팡이 끝에서 반짝), ^^, 반짝이 (가운데 칸은 살짝 뜀)."""
    up = [114, 124, 118][i]
    return front(hip=F(-[0, 3, 1][i]), legs=None if i != 1 else [("a", -6, 0.97), ("a", 6, 0.97)], arms=[("a", up, 1.0), ("a", up, 1.0)],
                 eyes="happy", mouth="big", fx=dp.level_up_stars(i), arms_front=True, wand=dict(hand=1, a=150, k=0.8))


# =====================================================================
# 구르기 (회전 원 안) — 로브에 싸인 공
# =====================================================================
def wz_roll(sp, i):
    """검은 로브로 몸을 감싸고 무릎을 끌어안은 동그란 공 (목도리 한 바퀴): 회전 중심 ROLL_CENTER, 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    lk = look()
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            hrx, hry = 20.0, 18.5
            hx, hy = cx + 5, cy - 8
            body = Part([C(cx - 4, cy + 7, 21.0)], ROBE, [([T(*wave_line((cx + 16, cy + 22), (cx - 24, cy + 22), 2, 3.0, (0, -1)),
                                                              (cx - 26, cy + 34), (cx + 18, cy + 34))], ROBE_IN)], outline=True, grow=0)
            knee = Part([E(cx + 2, cy + 6, cx + 28, cy + 24)], PANTS, outline=True, grow=0)
            shoe = Part([E(cx - 2, cy + 19, cx + 16, cy + 29)], SHOE, outline=True, grow=0)
            hand = Part([C(cx + 22, cy + 15, 5.0)], lk.skin, outline=True, grow=0)
            band = Part([E(cx - 8, cy + 6, cx + 20, cy + 14)], SCARF, [([L([(cx + 2, cy + 4), (cx + 3, cy + 16)], 1.8)], SCARF_GOLD)], outline=True, grow=0)
            hs_, sp_, hm = lk.hair("side", hx, hy, hrx, hry, 3.0)
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)] + hs_, lk.skin, hm, outline=True, grow=0, spikes=sp_)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.6)
            face = []
            for flip, lx in ((False, hx - 2), (True, hx + 11)):
                with xf(sx=0.7, sy=0.7, pivot=(lx, hy + 1)):
                    face += dp.eye("happy", lx, hy + 1, flip)
            face += mouth_ops("open" if i else "smile", hx + 6, hy + 12)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([body, knee, shoe, band, head, hand], face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면 벽에 손발을 붙이고 엉금엉금, 로브가 등을 덮음 (머리는 언제나 팔 위)
# =====================================================================
CLIMB = [(1.0, 0.2), (0.6, 0.6), (0.2, 1.0), (0.6, 0.6)]  # (왼손, 오른손) 올린 몫


def wizard_back(reach_, *, dy=0.0, scratch=False, sweat=False, sway=0.0):
    """뒷모습: 두 손을 머리 옆·위로 벽에 짚고 (reach: 0 어깨 옆 … 1 머리 위), 두 다리는 무릎을 벌려 벽에 디딤. 로브가 등을 덮음."""
    s = ws()
    hip = (64.0, 92.0 - (s["leg"] - 22) * 0.3 + dy)
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms, legs = [], []
    for side, r in zip((-1, 1), reach_):
        arms.append((64 + side * (s["hrx"] + 9 + look().back_reach - 2 * r), top - s["arm"] * (0.05 + 0.65 * r)))
        lift = 1.0 - r
        legs.append((64 + side * (s["hip"] + s["leg"] * 0.62), hip[1] + s["leg"] * (0.75 - 0.45 * lift)))
    fx = []
    if scratch:
        for x, y in arms:
            fx.append(([L([(x + d, y - 12), (x + d * 1.2, y - 4)], 1.2) for d in (-3, 0, 3)], (150, 150, 170), 0))
    if sweat:
        fx += sweat_fx(64 + s["hrx"] + 6, top - s["hry"] * 1.2)
    return wizard(view_="back", hip=hip, legs=legs, leg_bend=[(-1, -1), (1, -1)], arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], fx=fx,
                  robe=dict(sway=sway, len=0.8), scarf=dict(a=-4 + sway * 3, len=0.8))


def wz_climb(sp, i):
    """뒷모습으로 기어오름: 왼손·오른손이 번갈아 위로, 발은 엇갈려 디딤, 몸 살짝 오르내림."""
    return wizard_back(CLIMB[i], dy=[0, -1, 0, -1][i], sway=[-1, 0, 1, 0][i])


def wz_slide(sp, i):
    """뒷모습으로 주르륵: 두 손 쭉 위로, 긁힌 자국 3줄, 땀 한 방울."""
    return wizard_back((1.0, 1.0), dy=[0, 2][i], scratch=True, sweat=True)


# =====================================================================
# 🧹 빗자루 (옆모습, 오른쪽으로) — 자루에 걸터앉아 앞으로 숙이고 두 손으로 자루를 잡음, 로브·목도리가 뒤로 휘날림
# =====================================================================
BROOM_Y = 100.0  # 빗자루 자루 높이 (청소년 — 단계마다 다리 길이만큼 조금씩)


def broom_y():
    """이 단계의 빗자루 자루 높이 (짧은 다리는 덜 내려옴)."""
    return BROOM_Y + (ws()["leg"] - 22) * 0.3


def broom_part(y, x0=6.0, x1=118.0, bob=0.0):
    """빗자루: 나무 자루 (x0+22 → x1) + 뒤쪽 솔 다발 (부채꼴, 묶은 띠). y = 자루 높이."""
    stick = L([(x0 + 18, y + 1.2 + bob * 0.3), (x1, y - 1.6)], 3.6)
    tip = C(x1, y - 1.6, 2.3)
    bx = x0 + 22
    bristle = T((bx, y - 2.6), (bx, y + 3.2), (x0 + 2, y + 9 + bob), (x0 - 2, y + 4 + bob * 0.6), (x0 - 1, y - 3), (x0 + 3, y - 6 - bob * 0.4))
    lines = [L([(bx - 1, y + k * 0.6), (x0 + 1, y + k * 1.6 + bob * 0.5)], 0.9) for k in (-3, -1, 1, 3)]
    band = L([(bx + 1, y - 3.4), (bx + 1, y + 3.8)], 3.0)
    return Part([stick, tip, bristle, band], BROOM_WOOD,
                [([bristle], BRISTLE), (lines, BRISTLE_LINE), ([band], BROOM_BAND), ([L([(x0 + 40, y - 0.4), (x1 - 4, y - 2.4)], 1.0)], BROOM_WOOD_HI)],
                outline=True, grow=0)


def wz_broom(sp, i):
    """🧹 빗자루 날기 (3칸 반복): 자루에 걸터앉아 몸을 앞으로 숙이고 두 손으로 앞쪽 자루를 꼭 잡음, 다리는 굽혀 아래로 (발끝 뒤로),
    로브 자락과 목도리 끝이 뒤로 휘날리며 칸마다 펄럭, 바람 자국 · 살짝 위아래로 출렁."""
    s = ws()
    bob = [0.0, -1.2, 0.6][i]
    y = broom_y() + bob
    hip = (52.0, y - s["lw"] * 0.55)
    rot = 22.0
    sh = shoulders("side", hip, rot)
    grip_x = 96.0 + (s["arm"] - 20) * 0.6
    hands = [reach(sh[0], (grip_x - 6, y - 1.8), s["arm"], 1.0), reach(sh[1], (grip_x, y - 2.4), s["arm"], 1.0)]
    knee = s["leg"] * 0.62
    legs = [(hip[0] + knee * 0.7 - 2, y + s["leg"] * 0.62), (hip[0] + knee * 0.8, y + s["leg"] * 0.66)]
    flap = [2.5, -2.5, 0.8][i]
    return wizard(hip=hip, rot=rot, head_rot=-16, legs=legs, leg_bend=[(1, -0.6), (1, -0.6)], arms=hands, arm_bend=[(0, 1), (0, 1)],
                  eyes="dot", brow=None, mouth="grin", look_=(1.2, -0.3), point=True,
                  robe=dict(a=[62, 70, 56][i], flap=flap, len=0.85), scarf=dict(a=-84 + [0, 6, -4][i], flap=flap * 1.2, len=1.15),
                  mid=[lambda: broom_part(y, bob=bob)],
                  fx=[([L([(4 + sh_, yy), (16 + sh_, yy)], 1.6)], WIND, 0) for yy, sh_ in ((y - 22, [0, -3, -1][i]), (y + 14, [-2, 0, -3][i]))])


# =====================================================================
# ✨ 지팡이 휙 (옆모습) — 어깨 뒤로 들었다가 휙 앞으로, 끝에서 반짝
# =====================================================================
def arc_fx(c, r, a0, a1):
    """휘두른 자국 (반짝 노란 호)."""
    return [([L(dp.arc(c[0], c[1], r, r, a0, a1, 10), 1.6)], SPARK_EDGE, 0.8)]


def wz_cast(sp, i):
    """✨ 주문 (한 번, 마지막 칸 유지): 0 = 지팡이 든 손을 몸 뒤로 젖혀 끝이 뒤 위 · 1 = 가슴 앞으로 휙 (휘두른 자국, 끝이 앞 위) ·
    2 = 팔을 앞으로 쭉, 지팡이 끝에 반짝 별. 지팡이 끝 = mark_grip (주문이 나가는 자리)."""
    s = ws()
    legs = [floor_at(SIDE_CX - 8), floor_at(SIDE_CX + 8)]
    hip = H(SIDE_CX - 2, [0.5, 0.0, 0.8][i])
    rot = [-8, 4, 8][i]
    sh = shoulders("side", hip, rot)
    if i == 0:
        hand = reach(sh[1], (sh[1][0] - s["arm"] * 0.75, sh[1][1] + s["arm"] * 0.45), s["arm"], 0.95)
        return wizard(hip=hip, rot=rot, legs=legs, arms=[("a", 25, 0.9), hand], near_arm_back=True, arm_bend=[(1, 0.4), (1, 0.2)],
                      eyes="dot", brow="angry", mouth="smile", robe=dict(a=6), scarf=dict(a=-10), wand=dict(hand=1, a=-140, k=1.0, grip=True))
    if i == 1:
        hand = reach(sh[1], (sh[1][0] + s["arm"] * 0.95, sh[1][1] + 1), s["arm"], 1.0)
        tip = polar(hand, 128, s["wand"])
        return wizard(hip=hip, rot=rot, legs=legs, arms=[("a", 10, 0.9), hand], arm_bend=[(1, 0.4), (0, 1)], eyes="dot", brow="angry",
                      mouth="open", robe=dict(a=14, flap=1), scarf=dict(a=-24, flap=1.5), wand=dict(hand=1, a=128, k=1.0, grip=True),
                      fx=[([C(*tip, 1.8)], SPARK, 0.8), ([C(tip[0] - 5, tip[1] - 3, 1.1)], SPARK, 0.6), ([C(tip[0] - 9, tip[1] - 2, 0.8)], SPARK, 0.5)])
    hand = reach(sh[1], (sh[1][0] + s["arm"], sh[1][1] + 3 + max(0.0, 18 - s["leg"]) * 0.8), s["arm"], 1.0)
    tip = polar(hand, 102, s["wand"])
    return wizard(hip=hip, rot=rot, legs=legs, arms=[("a", 5, 0.9), hand], arm_bend=[(1, 0.4), (0, 1)], eyes="happy", mouth="open",
                  robe=dict(a=18, flap=1.5), scarf=dict(a=-30, flap=2), wand=dict(hand=1, a=102, k=1.0, grip=True),
                  fx=sparkle_fx(tip[0] + 2, tip[1] - 1, 5.5) + [([C(tip[0] + 9, tip[1] - 8, 1.4)], SPARK, 0.8), ([C(tip[0] + 8, tip[1] + 6, 1.1)], SPARK, 0.8)])


# =====================================================================
# 등록 도우미
# =====================================================================
WIZARD_POSES = {"walk": wz_walk, "run": wz_run, "roll": wz_roll, "idle": wz_idle, "look": wz_look, "sleep": wz_sleep, "hop": wz_hop,
                "fall": wz_fall, "dragged": wz_dragged, "land": wz_land, "sit_front": wz_sit_front, "wave": wz_wave, "tilt": wz_tilt,
                "belly": wz_belly, "stretch": wz_stretch, "heart": wz_heart, "tickle": wz_tickle, "annoyed": wz_annoyed,
                "hungry": wz_hungry, "eat": wz_eat, "level_up": wz_level_up, "climb": wz_climb, "slide": wz_slide, "lookup": wz_lookup,
                "broom": wz_broom, "cast": wz_cast}
BROOM_ANIMS = ["broom"]  # = Rust manifest::BROOM_ANIMS
SPELL_ANIMS = ["cast"]  # = Rust manifest::SPELL_ANIMS


def poses_for(look_, extra=None):
    """이 Look으로 그리는 포즈 표 (공통 + extra): 칸마다 지금 Look을 바꿔 둠. roll은 회전 원 안 그대로, 나머지는 _fit."""
    table = dict(WIZARD_POSES)
    table.update(extra or {})

    def bind(fn):
        """fn을 이 Look으로 부르는 포즈."""
        def f(sp, i):
            _LOOK["cur"] = look_
            return fn(sp, i)
        f.__name__ = fn.__name__
        f.__doc__ = fn.__doc__
        return f
    return {k: (bind(v) if k == "roll" else dp._fit(bind(v))) for k, v in table.items()}


def preview_rows(by_stage, path, anims, bg=(255, 255, 255), cols=4):
    """동작들 (줄마다 한 동작의 모든 칸) × 5단계 (열 띠) — 새 그림 확인용."""
    from PIL import Image
    from sprite_spec import STAGES
    cell = 132
    im = Image.new("RGBA", (len(STAGES) * cols * cell + (len(STAGES) - 1) * 12, len(anims) * cell), bg + (255,))
    for c, st in enumerate(STAGES):
        for r, a in enumerate(anims):
            for k, f in enumerate(by_stage[st][a]):
                im.alpha_composite(f, (c * (cols * cell + 12) + k * cell, r * cell))
    im.save(path)
