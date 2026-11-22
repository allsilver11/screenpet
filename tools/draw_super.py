"""슈퍼인간 🦸 — 파란 슈트에 빨간 망토를 두른 사람 모양 꼬마 영웅 펫(super). 뼈대·IK는 박쥐인간(draw_bat)·거미인간(draw_spider)과 같은 틀이고,
부품마다 자기 외곽선을 두른 두 발 꼬마 — 팔다리는 관절(어깨·팔꿈치·손 / 엉덩이·무릎·발)을 IK로 굽힌다.

    python tools/draw_super.py → ui/pets/super/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_super,climb_super,fly_super}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 큰 머리 (키의 45% 남짓) + 작은 몸 + 짧은 팔다리 둘씩. 가면 없는 살색 얼굴 (단계 눈 — 아기 반짝 큰 눈 … 어른 느긋한 눈),
        까만 머리 (옆가르마 앞머리) + 이마에 동그랗게 말린 컬 한 가닥. 몸: 파란 슈트 (소매 끝까지 — 손은 맨손), 빨간 장화,
        빨간 팬츠 + 노란 벨트 줄, 가슴에 노란 바탕·빨간 테의 방패 로고 + 빨간 S 모양, 등 뒤로 빨간 망토 (끝단이 부드럽게 물결).
단계 (SUPER): 아기 = 동글 꼬마 (머리 아주 큼, 팔다리 뭉툭, 망토 짧음) → 유아 → 어린이 → 청소년 → 어른 키 크고 어깨 넓음, 망토가 김.
특기 동작 (코어 brain_soar.rs): fly ✈️ 몸을 앞으로 눕혀 가까운 주먹을 앞으로 내밀고 다리는 모아 뒤로, 망토가 뒤로 휘날림 (3칸 펄럭) ·
      fly_fast 두 주먹을 앞으로 쭉, 몸이 더 곧게, 속도선 · fly_flip 공중제비 한 바퀴 (웅크린 몸이 4칸에 걸쳐 돎) ·
      hover 똑바로 선 채 떠서 사뿐 (발끝이 아래, 망토 살랑 — 이륙·착지·위아래 친구 화면).
      lift·hold·carry·throw·strain·present·slump 💪 물건 들기 (흐물이 juggle 틀 — 손 자리는 mark_grip): 한 손을 머리 위로 쭉 뻗어 번쩍 (다른 손은 허리).
      벽 타기 climb·slide = 뒷모습 (머리가 언제나 팔 위 — 팔이 머리를 덮지 않음).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, _ST, anger_fx, question_fx, sparkle_fx, sweat_fx, view, xf, z_fx  # noqa: E402
from draw_spider import ik, polar, target  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FLOOR = 125.0  # 바닥 (장화 바닥)
SUIT = (62, 116, 222)  # 파란 슈트 (몸·팔·다리)
SUIT_FAR = (46, 92, 186)  # 먼 쪽 팔다리
BOOT = (222, 50, 58)  # 빨간 장화
BOOT_FAR = (182, 36, 48)
CAPE = (226, 52, 60)  # 빨간 망토
CAPE_IN = (168, 28, 44)  # 망토 안감 (조금 어둡게, 펄럭일 때)
BRIEF = (214, 44, 54)  # 빨간 팬츠
BELT = (252, 206, 64)  # 노란 벨트 줄
LOGO_BG = (253, 214, 70)  # 가슴 방패 노란 바탕
LOGO_RED = (214, 36, 48)  # 방패 테·S
HAIR = (34, 32, 46)  # 까만 머리 (+ 이마의 컬)
HAIR_HI = (78, 82, 120)  # 머리 윤기
SKIN, SKIN_SHADE = (255, 222, 198), (236, 188, 164)  # 얼굴·손
MOUTH = (150, 64, 70)
BLUSH = (250, 156, 156)
OUT = (24, 22, 40)  # 외곽선
DUST, DUST_LINE = (236, 228, 214), (176, 160, 150)
WIND = (180, 204, 240)  # 바람 자국·속도선
MILK, MILK_GLASS = (255, 255, 255), (196, 220, 240)

# 단계(lvl)별 — hrx·hry: 머리 반지름 · tw: 어깨 반폭 · th: 몸 높이 · hip: 다리 뿌리 반간격 · leg: 다리 길이 · lw: 다리 굵기 ·
# arm: 팔 길이 · aw: 팔 굵기 · ear: 앞머리 볼륨 높이 · emb: 가슴 로고 배율 · cape: 망토 길이 몫
SUPER = {
    0: dict(hrx=27.0, hry=25.0, tw=13.5, th=21.0, hip=6.6, leg=9.5, lw=10.4, arm=13.0, aw=8.8, ear=3.0, emb=0.8, cape=0.55),
    1: dict(hrx=26.4, hry=24.2, tw=13.4, th=22.5, hip=6.8, leg=13.5, lw=9.8, arm=15.0, aw=8.3, ear=3.4, emb=0.85, cape=0.62),
    2: dict(hrx=25.8, hry=23.4, tw=13.6, th=24.5, hip=7.0, leg=18.0, lw=9.4, arm=17.5, aw=7.9, ear=3.8, emb=0.92, cape=0.68),
    3: dict(hrx=25.2, hry=22.8, tw=14.6, th=26.5, hip=7.2, leg=22.0, lw=9.2, arm=20.0, aw=7.6, ear=4.2, emb=1.0, cape=0.72),
    4: dict(hrx=24.6, hry=22.2, tw=16.4, th=29.5, hip=8.0, leg=26.0, lw=9.6, arm=23.0, aw=7.9, ear=4.6, emb=1.1, cape=0.76),
}
OVERLAP = 5.0  # 머리가 몸 윗부분을 덮는 깊이


def hs():
    """지금 단계의 꼬마 영웅 치수."""
    return SUPER[_ST["lvl"]]


def foot_y():
    """바닥에 선 발 관절 y (장화 바닥 = FLOOR)."""
    return FLOOR - hs()["lw"] * 0.5


def stand_hip_y():
    """똑바로 섰을 때 엉덩이 관절 y."""
    return foot_y() - hs()["leg"]


def reach(root, pt, ln, k=0.97):
    """root에서 pt 쪽으로 팔다리 길이(ln × k)를 넘지 않는 끝 자리 (짧은 아기 팔다리가 늘어나지 않게)."""
    dx, dy = pt[0] - root[0], pt[1] - root[1]
    d = math.hypot(dx, dy) or 1e-6
    m = min(1.0, ln * k / d)
    return (root[0] + dx * m, root[1] + dy * m)


def shield_pts(cx, cy, w):
    """방패 로고 다각형 (가운데 (cx, cy), 반폭 w): 윗변이 평평하고 양 어깨가 각진 뒤 아래 한 점으로 모이는 오각 방패."""
    return [(cx - w, cy - 0.62 * w), (cx + w, cy - 0.62 * w), (cx + 1.22 * w, cy - 0.2 * w), (cx, cy + 0.92 * w), (cx - 1.22 * w, cy - 0.2 * w)]


def s_curve(cx, cy, w):
    """방패 속 S 모양 (위 오른쪽에서 시작해 왼쪽으로 감았다가 오른쪽 아래로 돌아 왼쪽 끝)."""
    top = dp.arc(cx - 0.05 * w, cy - 0.2 * w, 0.42 * w, 0.2 * w, -20, -270, 8)
    bot = dp.arc(cx + 0.02 * w, cy + 0.2 * w, 0.4 * w, 0.2 * w, -90, 160, 8)
    return top + bot


def logo_ops(ex, ey, k=1.0, sx=1.0):
    """가슴 로고: 빨간 테 방패 + 노란 바탕 + 빨간 S (sx: 옆모습은 가로로 좁게)."""
    k *= hs()["emb"]
    w = 6.4 * k
    with xf(sx=sx, pivot=(ex, ey)):
        rim = [T(*shield_pts(ex, ey, w + 1.3))]
        bg = [T(*shield_pts(ex, ey, w - 0.2))]
        s = [L(s_curve(ex, ey - 0.6 * k, w), max(1.1, 1.5 * k))]
    return [(rim, LOGO_RED), (bg, LOGO_BG), (s, LOGO_RED)]


def wave_hem(right, left, n, depth, up):
    """망토 아랫단: right → left 사이를 n번 부드럽게 출렁이는 물결 (up: 들어가는 방향 단위 벡터)."""
    pts = []
    for j in range(n * 6 + 1):
        t = j / (n * 6)
        bump = depth * 0.5 * (1 - math.cos(2 * math.pi * n * t))
        pts.append((right[0] + (left[0] - right[0]) * t + up[0] * bump, right[1] + (left[1] - right[1]) * t + up[1] * bump))
    return pts


def limb(root, end, ln, w, color, end_col, prefer, *, boot=False, toe=0.0, open_hand=False, point=False):
    """팔 또는 다리 한 짝 (자기 외곽선): 파란 마디 둘 + 끝은 빨간 장화 (정강이 아래 + 동글 장화) 또는 살색 맨손 (동글 주먹).
    toe: 장화 코 방향 (옆모습 +1, 앞모습 0) · open_hand: 손바닥을 펴 손가락 셋 · point: 발끝을 아래로 (날 때 — 장화가 길쭉)."""
    j = ik(root, end, ln, prefer)
    shapes_ = [L([root, j, end], w)]
    marks = []
    if boot:
        seg = math.hypot(j[0] - end[0], j[1] - end[1]) or 1.0
        k = min(0.55, w * 1.05 / seg)
        cuff = (end[0] + (j[0] - end[0]) * k, end[1] + (j[1] - end[1]) * k)
        red = [L([cuff, end], w + 2.0)]
        x, y = end
        if point:
            dx, dy = end[0] - j[0], end[1] - j[1]
            d = math.hypot(dx, dy) or 1.0
            tip = (x + dx / d * w * 0.7, y + dy / d * w * 0.7)
            b = L([end, tip], w * 1.05)
        else:
            b = E(x - w * 0.62 + min(toe, 0) * w * 0.42, y - w * 0.5, x + w * 0.62 + max(toe, 0) * w * 0.42, y + w * 0.5)
        shapes_.append(b)
        red.append(b)
        marks.append((red, end_col))
        # 장화 윗단 (살짝 밝은 테)
        marks.append(([L([(cuff[0] - (end[1] - cuff[1]) * 0.0, cuff[1]), cuff], w + 2.2)], end_col))
    else:
        fist = C(end[0], end[1], w * (0.66 if open_hand else 0.6))
        shapes_.append(fist)
        hand = [fist]
        if open_hand:  # 편 손바닥: 팔이 오는 반대쪽으로 손가락 셋
            a = math.degrees(math.atan2(end[0] - j[0], end[1] - j[1]))
            for da in (-32, 0, 32):
                tip = polar(end, a + da, w * 0.95)
                f = L([end, tip], w * 0.34)
                shapes_.append(f)
                hand.append(f)
        marks.append((hand, end_col))
    return Part(shapes_, color, marks, outline=True, grow=0), j


# =====================================================================
# 얼굴: 단계 눈 · 눈썹 · 입 · 볼 / 까만 머리 + 이마 컬
# =====================================================================
def eye_ops(kind, cx, cy, flip, brow=None, sx=1.0):
    """눈 하나 (draw_pets 단계 눈 — 아기 반짝 큰 눈 … 어른 느긋한 눈) + 눈썹 (angry · sad). sx: 옆모습 먼 눈은 가로로 좁게."""
    with xf(sx=sx * 0.82, sy=0.82, pivot=(cx, cy)):
        ops = dp.stage_eye(kind, cx, cy, flip)
        if brow:
            ops += dp.brows(brow, cx, cy + 2.0, flip)
    return ops


def mouth_ops(kind, cx, cy):
    """입: None 없음 · smile 빙긋 · open · big · grin · frown · pout · chew."""
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
    raise ValueError(kind)


def curl_pts(x, y, r):
    """이마의 컬 한 가닥: (x, y)에서 아래로 내려와 동그랗게 한 번 말림 (반지름 r)."""
    return [(x + r * 0.5, y - r * 0.5), (x, y)] + dp.arc(x - r * 0.2, y + r, r, r, 290, 290 - 250, 9)


def hair_marks(view_, hx, hy):
    """머리 위쪽을 덮는 까만 머리 (머리 도형 안으로 잘림): 앞모습은 옆가르마 앞머리가 이마 위에서 비스듬히, 옆모습은 뒤통수 전체 +
    이마 앞까지 · 뒷모습은 목덜미 위까지 전부 머리."""
    s = hs()
    hrx, hry = s["hrx"], s["hry"]
    sky = hy - hry - s["ear"] - 8
    shine = [L(dp.arc(hx, hy, hrx * 0.74, hry * 0.74, 215, 262, 8), 2.0)]
    if view_ == "back":
        return [([T(*dp.arc(hx, hy + 0.42 * hry, hrx * 1.3, 0.22 * hry, 0, 180, 12)[::-1], (hx - hrx * 1.3, sky), (hx + hrx * 1.3, sky))], HAIR),
                ([E(hx - hrx * 2, sky, hx + hrx * 2, hy + 0.2 * hry)], HAIR), (shine, HAIR_HI)]
    if view_ == "front":
        # 앞머리 끝선: 왼쪽 가르마에서 오른쪽으로 비스듬히 내려오는 물결 + 양옆 귀 위 구레나룻
        edge = [(hx + hrx * 1.2, hy + 0.1 * hry), (hx + 0.86 * hrx, hy - 0.08 * hry), (hx + 0.62 * hrx, hy - 0.34 * hry),
                (hx + 0.3 * hrx, hy - 0.4 * hry), (hx + 0.02 * hrx, hy - 0.5 * hry), (hx - 0.3 * hrx, hy - 0.56 * hry),
                (hx - 0.6 * hrx, hy - 0.4 * hry), (hx - 0.86 * hrx, hy - 0.1 * hry), (hx - hrx * 1.2, hy + 0.1 * hry)]
        return [([T(*edge, (hx - hrx * 1.2, sky), (hx + hrx * 1.2, sky))], HAIR), (shine, HAIR_HI),
                ([L(curl_pts(hx + 0.06 * hrx, hy - 0.5 * hry, 0.11 * hrx + 0.6), 1.9)], HAIR)]
    # 옆모습 (오른쪽을 봄): 얼굴 쪽 앞머리 끝선은 이마 위 · 귀 앞까지 구레나룻, 뒤통수는 목덜미까지
    edge = [(hx + hrx * 1.2, hy - 0.44 * hry), (hx + 0.78 * hrx, hy - 0.46 * hry), (hx + 0.5 * hrx, hy - 0.56 * hry),
            (hx + 0.22 * hrx, hy - 0.44 * hry), (hx + 0.02 * hrx, hy - 0.1 * hry), (hx - 0.04 * hrx, hy + 0.3 * hry),
            (hx - 0.3 * hrx, hy + 0.44 * hry), (hx - 0.62 * hrx, hy + 0.64 * hry), (hx - hrx * 1.2, hy + 0.7 * hry)]
    return [([T(*edge, (hx - hrx * 1.2, sky), (hx + hrx * 1.2, sky))], HAIR), (shine, HAIR_HI),
            ([L(curl_pts(hx + 0.66 * hrx, hy - 0.5 * hry, 0.11 * hrx + 0.6), 1.9)], HAIR)]


def hair_shapes(view_, hx, hy):
    """머리 도형에 더하는 앞머리 볼륨 (정수리 앞쪽 살짝 솟음) + 귀 (앞·뒷모습 양옆, 옆모습은 머리 안의 무늬)."""
    s = hs()
    hrx, hry, q = s["hrx"], s["hry"], s["ear"]
    out = []
    if view_ == "side":
        out.append(E(hx - 0.1 * hrx, hy - hry - q, hx + 0.9 * hrx, hy - hry + 0.5 * hry))
    else:
        out.append(E(hx - 0.7 * hrx, hy - hry - q, hx + 0.5 * hrx, hy - hry + 0.5 * hry))
        for side in (-1, 1):
            out.append(E(hx + side * hrx - 3.2, hy + 0.02 * hry - 4.6, hx + side * hrx + 3.2, hy + 0.02 * hry + 4.6))
    return out


def face_ops(view_, hx, hy, eyes, brow, mouth, look, blush_on):
    """얼굴 ops (눈 둘 + 볼 + 입, 옆모습은 귀 무늬도) — 머리 변환 안에서 부름. 옆모습은 얼굴이 오른쪽으로 쏠리고 먼 눈은 좁게."""
    s = hs()
    hrx, hry = s["hrx"], s["hry"]
    lx, ly = look
    ops = []
    if view_ == "front":
        fy = hy + 0.02 * hry + ly
        for side in (-1, 1):
            ops += eye_ops(eyes, hx + side * 0.36 * hrx + lx, fy, side > 0, brow)
        if blush_on:
            ops += [([E(hx + side * 0.56 * hrx - 3.2, hy + 0.44 * hry - 1.6, hx + side * 0.56 * hrx + 3.2, hy + 0.44 * hry + 1.4)], BLUSH)
                    for side in (-1, 1)]
        mx, my = hx + lx * 0.5, hy + 0.62 * hry
    else:
        fy = hy + 0.02 * hry + ly
        ear = (hx - 0.12 * hrx, hy + 0.14 * hry)
        ops.append(([E(ear[0] - 3.4, ear[1] - 4.8, ear[0] + 3.4, ear[1] + 4.8)], SKIN_SHADE))
        ops.append(([E(ear[0] - 2.0, ear[1] - 3.2, ear[0] + 2.0, ear[1] + 3.2)], SKIN))
        ops += eye_ops(eyes, hx + 0.82 * hrx + lx * 0.6, fy - 0.5, True, brow, sx=0.5)
        ops += eye_ops(eyes, hx + 0.4 * hrx + lx, fy, False, brow, sx=0.85)
        if blush_on:
            ops.append(([E(hx + 0.42 * hrx - 2.8, hy + 0.46 * hry - 1.6, hx + 0.42 * hrx + 2.8, hy + 0.46 * hry + 1.4)], BLUSH))
        mx, my = hx + 0.72 * hrx, hy + 0.62 * hry
    return ops + mouth_ops(mouth, mx, my)


# =====================================================================
# 망토
# =====================================================================
def cape_side(neck, length, a, width, flap=0.0):
    """옆모습 망토 (오른쪽을 봄): 목에서 a도만큼 뒤로 날리는 빨간 천 (0 = 똑바로 아래, 90 = 뒤로 수평), 끝단은 부드러운 물결.
    flap: 끝단의 물결 (칸마다 바꿔 펄럭)."""
    r = math.radians(a)
    d = (-math.sin(r), math.cos(r))  # 늘어지는 쪽
    b = (-math.cos(r), -math.sin(r))  # 뒤쪽 (a = 0이면 왼쪽)

    def P(u, v):
        return (neck[0] + d[0] * u + b[0] * v, neck[1] + d[1] * u + b[1] * v)
    front_bot, back_bot = P(length, -3 - flap), P(length + flap * 0.8, width * 1.45)
    pts = [P(0, -3.5), P(length * 0.5, -3.5 - flap * 0.5), front_bot]
    pts += wave_hem(front_bot, back_bot, 2, min(5.0, length * 0.14) + abs(flap) * 0.6, (-d[0], -d[1]))
    pts += [P(length * 0.55, width * 1.1 + flap * 0.7), P(0, 6.5)]
    lining = [T(P(length * 0.15, -3.5), front_bot, P(length * 0.8, 1.5))]
    return Part([T(*pts)], CAPE, [(lining, CAPE_IN)] if a > 40 else [], outline=True, grow=0)


def cape_front(cx, top, hem_y, tw, sway=0.0):
    """앞·뒷모습 망토 (몸 뒤로 양옆이 보이는 빨간 천, 뒷모습은 등을 덮음): 어깨에서 아래로 넓어지고 끝단은 부드러운 물결."""
    right, left = (cx + tw * 2.0 + sway, hem_y), (cx - tw * 2.0 + sway, hem_y)
    pts = [(cx - tw * 0.92, top), (cx + tw * 0.92, top)] + wave_hem(right, left, 3, min(3.5, (hem_y - top) * 0.1), (0, -1))
    return Part([T(*pts)], CAPE, outline=True, grow=0)


# =====================================================================
# 꼬마 영웅 한 장
# =====================================================================
def hero_shoulders(view_, hip, rot):
    """hero가 쓰는 어깨 자리 [먼/왼, 가까운/오른] (프레임 좌표)."""
    s = hs()
    hx0, hy0 = hip
    top = hy0 + s["lw"] * 0.35 - s["th"]
    bm = dp._xf_matrix(rot=rot, pivot=(hx0, hy0))
    if view_ == "side":
        pts = [(hx0 - 1.5, top + s["aw"] * 0.75), (hx0 + 1.0, top + s["aw"] * 0.8)]
    else:
        pts = [(hx0 - (s["tw"] - s["aw"] * 0.45), top + s["aw"] * 0.6), (hx0 + (s["tw"] - s["aw"] * 0.45), top + s["aw"] * 0.6)]
    return [dp._apply(bm, p) for p in pts]


def head_top_y(hip, rot=0.0):
    """똑바른 몸의 머리 꼭대기 y (앞머리 볼륨 빼고)."""
    s = hs()
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    return top - 2 * s["hry"] + OVERLAP


def hero(*, view_="side", hip=None, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0), legs=None, arms=None, leg_bend=None, arm_bend=None,
         eyes="dot", brow=None, mouth="smile", look=(0.0, 0.0), blush_on=True, fx=(), ops=(), held=(), whole=None, grip=None,
         arms_front=False, arms_behind=False, far_limbs_front=False, near_arm_back=False, cape=None, open_hand=(False, False), point=False):
    """꼬마 영웅. view_: side (오른쪽을 봄) · front · back · hip: 엉덩이 관절 (없으면 똑바로 선 자리) · rot: 몸 기울기 (엉덩이 기준, 시계 방향) ·
    head_rot·head_d: 고개 · legs·arms: [먼/왼, 가까운/오른] 끝 자리 (target 형식) · leg_bend·arm_bend: 무릎·팔꿈치 굽는 쪽 ·
    held: 몸과 머리 사이 Part 함수 · fx: 효과 목록 또는 손 자리를 받아 효과를 돌려주는 함수 · grip: mark_grip할 손 ("near"·"far"·"left"·"right") ·
    arms_front: 앞모습 팔을 머리 위에 · arms_behind: 옆모습 가까운 팔도 머리 뒤에 · far_limbs_front: 먼 팔도 몸 앞에 ·
    near_arm_back: 옆모습 가까운 팔을 몸 뒤에 (먼 팔처럼) · cape: 망토 dict(a=날림 각도, flap=펄럭, len=길이 배) 또는 False ·
    open_hand: 손바닥 편 손 · point: 발끝을 아래로 (날 때)."""
    s = hs()
    side_view = view_ == "side"
    hx0, hy0 = hip or ((60.0 if side_view else 64.0), stand_hip_y())
    lw, aw = s["lw"], s["aw"]
    bottom = hy0 + lw * 0.35
    top = bottom - s["th"]
    cape = {} if cape is None else cape
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
        sh_roots = hero_shoulders(view_, (hx0, hy0), rot)
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
            p, _ = limb(leg_roots[j], end, s["leg"], lw, SUIT_FAR if far else SUIT, BOOT_FAR if far else BOOT, leg_pref[j], boot=True,
                        toe=(1 if side_view else 0) if view_ != "back" else 0, point=point)
            leg_parts.append(p)
            end = target(sh_roots[j], arms[j], s["arm"], arm_sides[j])
            p, _ = limb(sh_roots[j], end, s["arm"], aw, SUIT_FAR if far else SUIT, SKIN_SHADE if far else SKIN, arm_pref[j], open_hand=open_hand[j])
            arm_parts.append(p)
            hands.append(end)
        th = s["th"]
        # 몸통 (+ 빨간 팬츠·노란 벨트 줄·가슴 방패 로고)
        with xf(rot=rot, pivot=(hx0, hy0)):
            tw = s["tw"] if not side_view else s["tw"] * 0.95
            if _ST["lvl"] == 0:
                tshape = [E(hx0 - tw * 1.04, top, hx0 + tw * 1.04, bottom + 1.5)]
            else:
                tshape = [E(hx0 - tw, top, hx0 + tw, top + th * 0.78), E(hx0 - tw * 0.84, top + th * 0.32, hx0 + tw * 0.84, bottom)]
            by0, by1 = bottom - th * 0.32, bottom - th * 0.22
            marks = [([T((hx0 - tw * 1.2, by1), (hx0 + tw * 1.2, by1 - 0.4), (hx0 + tw * 1.2, bottom + 4), (hx0 - tw * 1.2, bottom + 4))], BRIEF),
                     ([T((hx0 - tw * 1.2, by0), (hx0 + tw * 1.2, by0 - 0.4), (hx0 + tw * 1.2, by1 - 0.4), (hx0 - tw * 1.2, by1))], BELT)]
            if view_ == "front":
                marks.append(([E(hx0 - 2.6, by0 - 0.6, hx0 + 2.6, by1 + 0.6)], LOGO_BG))
            if view_ == "side":
                marks += logo_ops(hx0 + tw * 0.42, top + th * 0.34, 0.95, sx=0.6)
            elif view_ == "front":
                marks += logo_ops(hx0, top + th * 0.34)
            torso = Part(tshape, SUIT, marks, outline=True, grow=0)
            neck = (hx0 + (1.5 if side_view else 0.0), top)
            if view_ == "side":
                cape_part = cape_side(neck, (th + s["leg"] * 0.9) * cape.get("len", 1.0) * (0.55 + s["cape"] * 0.6) / 0.95,
                                      cape.get("a", 8.0), tw * 1.1 + 2, cape.get("flap", 0.0)) if cape is not False else None
            else:
                hem = min(FLOOR - 4, hy0 + (s["leg"] * s["cape"] + 4 if view_ == "front" else s["lw"] * 0.2))
                cape_part = cape_front(hx0, top + 1.5, hem, tw, cape.get("sway", 0.0) if cape else 0.0) if cape is not False else None
        neck = P(neck)
        # 머리 (살색 얼굴 + 까만 머리 + 이마 컬)
        hrx, hry = s["hrx"], s["hry"]
        with xf(rot=rot + head_rot, pivot=neck):
            hx, hy = neck[0] + head_d[0] + (2.0 if side_view else 0.0), neck[1] - hry + OVERLAP + head_d[1]
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)] + hair_shapes(view_, hx, hy), SKIN, hair_marks(view_, hx, hy), outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.7)
            face = [] if view_ == "back" else face_ops(view_, hx, hy, eyes, brow, mouth, look, blush_on)
        parts = [cape_part] if cape_part and view_ != "back" else []
        if side_view:
            parts += [arm_parts[0]] if not far_limbs_front else []
            parts += [arm_parts[1]] if near_arm_back else []
            parts += [leg_parts[0], leg_parts[1], torso]
            parts += [arm_parts[1]] if arms_behind and not near_arm_back else []
            parts += [h() for h in held]
            parts += [head]
            parts += ([arm_parts[0]] if far_limbs_front else []) + ([] if arms_behind or near_arm_back else [arm_parts[1]])
        else:
            # 뒷모습은 머리가 팔을 가림 (팔을 머리 위에 그리지 않음 — 벽 타기에서 팔이 머리를 덮던 실수)
            if view_ == "back":
                arms_front = False
            parts += leg_parts + [torso]
            if view_ == "back" and cape_part:
                parts.append(cape_part)
            if not arms_front:
                parts += arm_parts
            parts += [h() for h in held]
            parts += [head]
            if arms_front:
                parts += arm_parts
        if grip:
            idx = {"far": 0, "near": 1, "left": 0, "right": 1}[grip]
            dp.mark_grip(*hands[idx])
    if callable(fx):
        fx = fx(hands)
    return dp.compose(parts, face + list(ops), fx)


SIDE_CX = 60.0


def H(x=SIDE_CX, dy=0.0):
    """엉덩이 관절 자리 (x, 선 높이 + dy)."""
    return (x, stand_hip_y() + dy)


def floor_at(x, lift=0.0):
    """바닥에 닿은 발 관절 (장화 바닥 = FLOOR - lift)."""
    return (x, foot_y() - lift)


# =====================================================================
# 옆모습 (오른쪽을 본다)
# =====================================================================
WALK = [(-7, 7, 0, 3), (0, 0, 2.5, 0), (7, -7, 3, 0), (0, 0, 0, 2.5)]  # (먼 발 dx, 가까운 발 dx, 먼 발 듦, 가까운 발 듦)


def su_walk(sp, i):
    """씩씩하게 걷기: 두 다리를 번갈아 앞뒤로, 팔은 반대로 흔듦, 몸 통통, 망토 살랑."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    sw = [1, 0, -1, 0][i]
    arms = [("a", 22 * sw, 0.96), ("a", -22 * sw + 4, 0.96)]
    return hero(hip=hip, legs=legs, arms=arms, rot=2, cape=dict(a=[14, 18, 14, 18][i], flap=[0, 1.5, 0, -1.5][i]))


def su_run(sp, i):
    """다다다 달리기: 앞으로 기울고 크게 성큼, 팔을 힘차게, 망토가 뒤로 휘날림, 속도선."""
    st = [1, 0, -1, 0][i]
    hip = H(dy=[-1.5, -3.5, -1.5, -3.5][i])
    legs = [("a", -40 * st, 0.95 if st else 0.75), ("a", 40 * st, 0.95 if st else 0.75)]
    if st == 0:
        legs = [("a", -25, 0.8), ("a", 35, 0.7)] if i == 1 else [("a", 35, 0.7), ("a", -25, 0.8)]
    arms = [("a", 55 * st + 5, 0.8), ("a", -45 * st + 5, 0.8)]
    return hero(hip=hip, legs=legs, arms=arms, rot=12, mouth="open", arm_bend=[(1, 0.3), (1, 0.3)],
                cape=dict(a=[58, 66, 58, 66][i], flap=[2, -2, 2, -2][i]), fx=dp.speed_fx(4, (70, 86, 102), 10, [0, -3, 0, -3][i]))


def su_idle(sp, i):
    """제자리 숨쉬기: 가슴이 살짝 오르내림, 손은 허리께, 망토 살랑."""
    return hero(hip=H(dy=[0, 0.8][i]), arms=[("a", 12, 0.97), ("a", 8 + 3 * i, 0.97)], head_d=(0, [0, 0.6][i]), cape=dict(a=8 + 3 * i, flap=i))


def su_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞 (고개만)."""
    rot, eyes, look = [(-8, "up", (0.5, -1.0)), (0, "dot", (1.2, 0)), (8, "dot", (0.6, 1.5)), (0, "closed", (0, 0))][i]
    return hero(head_rot=rot, eyes=eyes, look=look)


def sit_hip(x=SIDE_CX):
    """바닥에 앉은 엉덩이 관절."""
    return (x, FLOOR - hs()["lw"] * 0.75)


def su_sleep(sp, i):
    """망토를 덮고 웅크려 쿨쿨: 바닥에 앉아 무릎을 끌어안고 고개를 무릎에 묻음, 눈 ‿, z."""
    s = hs()
    hip = sit_hip(SIDE_CX - 6)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 3, foot_y()), (knee_x + 5, foot_y())]
    arms = [(knee_x + 1, hip[1] - s["leg"] * 0.45 - 4), (knee_x + 3, hip[1] - s["leg"] * 0.45 - 2)]
    fx = [z_fx(106, 34 - 3 * i, 4.0), z_fx(116, 20 - 3 * i, 2.6)]
    return hero(hip=hip, rot=14 + i, head_rot=10, head_d=(2, 1 + 0.6 * i), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                arm_bend=[(0, 1), (0, 1)], eyes="closed", mouth=None, fx=fx, cape=dict(a=-12, len=0.75))


def su_annoyed(sp, i):
    """털썩 앉아 팔짱 끼고 눈 반쯤 감은 뾰로통 💢."""
    s = hs()
    hip = sit_hip(SIDE_CX - 4)
    legs = [(hip[0] + s["leg"] + 4, foot_y()), (hip[0] + s["leg"] + 7, foot_y())]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms = [(hip[0] + 8, top + s["th"] * 0.45), (hip[0] + 9, top + s["th"] * 0.5)]
    return hero(hip=hip, rot=-6, legs=legs, arms=arms, arm_bend=[(1, 0.8), (1, 0.8)], eyes="flat", mouth="pout", head_rot=-4,
                fx=[anger_fx(102 + i, 30 - i, 5.0 + 0.6 * i)], cape=dict(a=4, len=0.7))


def su_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 주먹 하나를 위로 번쩍 솟음 (다리 모음) → 다리 내림."""
    s = hs()
    if i == 0:
        hip = H(dy=s["leg"] * 0.32)
        return hero(hip=hip, rot=12, legs=[floor_at(SIDE_CX - 3), floor_at(SIDE_CX + 3)], arms=[("a", -30, 0.9), ("a", -25, 0.9)],
                    eyes="tight", mouth="open", cape=dict(a=20))
    hip = (SIDE_CX, [0, 84, 90][i] - (s["leg"] - 22) * 0.5)
    legs = [("a", -30, 0.7), ("a", 25, 0.62)] if i == 1 else [("a", -8, 0.92), ("a", 10, 0.9)]
    arms = [("a", 20, 0.9), ("a", 165, 0.98)] if i == 1 else [("a", 30, 0.9), ("a", 120, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="open", arm_bend=[(1, 0), (1, 0)], cape=dict(a=[0, -10, 25][i], flap=2),
                arms_behind=i == 1)


def su_fall(sp, i):
    """떨어짐 (던져졌을 때만 — 스스로는 날아서 내려감): 팔다리를 허우적, 동그란 눈, 망토가 위로 펄럭."""
    w = [0, 1][i]
    hip = (SIDE_CX, 84.0)
    legs = [("a", -40 - 15 * w, 0.85), ("a", 40 + 15 * w, 0.75)]
    arms = [("a", 150 - 25 * w, 0.95), ("a", 125 + 25 * w, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="round", mouth="open", rot=[-6, 6][i], arm_bend=[(0, -1), (0, -1)],
                cape=dict(a=[150, 140][i], flap=[2, -2][i], len=0.8))


def su_dragged(sp, i):
    """목덜미가 잡혀 대롱대롱: 팔다리가 축 늘어지고 살랑."""
    s = hs()
    sway = [-6, 6][i]
    top_y = 22.0 + s["ear"]
    hip_y = top_y + 2 * s["hry"] - OVERLAP + s["th"] - s["lw"] * 0.35 - 2
    hip = (SIDE_CX + 2, hip_y)
    legs = [("a", -6, 0.97), ("a", 8, 0.95)]
    arms = [("a", 12, 0.95), ("a", 18, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="round", mouth="pout", whole=dict(rot=sway * 0.5, pivot=(SIDE_CX, 14)),
                fx=sweat_fx(104, 30), cape=dict(a=-4, len=0.8))


def su_land(sp, i):
    """착지 (한 번): 무릎을 살짝 굽혀 사뿐 → 바로 섬 (쿵 없이 — 슈퍼인간은 늘 사뿐)."""
    s = hs()
    if i == 0:
        return hero(hip=H(dy=s["leg"] * 0.18), rot=6, legs=[floor_at(SIDE_CX - 4), floor_at(SIDE_CX + 4)], arms=[("a", 40, 0.9), ("a", 34, 0.9)],
                    eyes="dot", mouth="smile", cape=dict(a=30, flap=2))
    return hero(eyes="happy", cape=dict(a=12, flap=1))


def su_lookup(sp, i):
    """올려다보기: 고개를 젖혀 눈이 위로, 반짝."""
    return hero(head_rot=dp.lookup_tilt(i), eyes="up", look=(0.6, -1.0), fx=dp.lookup_fx(i, 110, 14))


# =====================================================================
# 앞모습
# =====================================================================
def front(**kw):
    """앞모습 꼬마 영웅."""
    return hero(view_="front", **kw)


def F(dy=0.0):
    """앞모습 엉덩이 관절."""
    return (64.0, stand_hip_y() + dy)


def torso_top(hip_y=None):
    """몸 위쪽 y."""
    s = hs()
    return (stand_hip_y() if hip_y is None else hip_y) + s["lw"] * 0.35 - s["th"]


def su_sit_front(sp, i):
    """정면 서기: 허리에 두 주먹을 얹은 영웅 자세, 망토가 양옆으로 (마지막 장 깜빡)."""
    s = hs()
    wy = torso_top() + s["th"] * 0.72
    arms = [(64 - s["tw"] * 0.92, wy), (64 + s["tw"] * 0.92, wy)]
    return front(arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="closed" if i == 3 else "dot", head_d=(0, [0, 0.5, 0, 0][i]),
                 cape=dict(sway=[0, 0.6, 0, -0.6][i]))


def su_wave(sp, i):
    """한 손을 번쩍 들어 흔들흔들 (다른 손은 허리)."""
    a = [118, 140, 118, 140][i]
    return front(arms=[("a", 18, 0.95), ("a", a, 1.0)], arm_bend=[(-1, 0.3), (1, 0)], eyes="happy" if i % 2 else "dot",
                 mouth="open", head_rot=[-3, -1, -3, -1][i], open_hand=(False, True))


def su_tilt(sp, i):
    """고개 갸웃 + ? (한 손은 턱)."""
    top = torso_top()
    return front(head_rot=[8, 12, 12][i], arms=[("a", 15, 0.95), (64 + 6, top + 2)], arm_bend=[(-1, 0.3), (1, 0.6)], arms_front=True,
                 fx=[question_fx(106, 24 - i)], mouth="pout")


def su_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 두 다리를 위로 버둥, 두 팔 번쩍, 깔깔."""
    s = hs()
    w = [0, 2, 0, -2][i]
    hip = (64.0, FLOOR - s["lw"] * 0.8)
    legs = [("a", 112 + 6 * w, 0.9), ("a", 112 - 6 * w, 0.9)]
    arms = [("a", 128 - 5 * w, 0.95), ("a", 128 + 5 * w, 0.95)]
    return front(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="big", cape=False)


def su_stretch(sp, i):
    """기지개 (한 번): 두 팔 쭉 위로 → 하품하며 팔 벌림 → 서기."""
    if i == 0:
        return front(arms=[("a", 140, 1.0), ("a", 140, 1.0)], eyes="tight", hip=F(-1.5))
    if i == 1:
        return front(arms=[("a", 100, 1.0), ("a", 100, 1.0)], eyes="closed", mouth="big")
    return front(eyes="happy")


def su_heart(sp, i):
    """하트를 가슴에 꼭 안고 눈 ^^, 작은 하트가 떠오름."""
    s = hs()
    hy = torso_top() + s["th"] * 0.45

    def held():
        """가슴에 안은 하트."""
        return dp.held_heart(hy - 4)
    return front(arms=[(58, hy + 3), (70, hy + 3)], arm_bend=[(-1, 0.6), (1, 0.6)], held=[held], eyes="happy", fx=dp.heart_burst(i))


def su_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 배를 감싸고 깔깔 (> <)."""
    s = hs()
    y = torso_top() + s["th"] * 0.6
    return front(rot=[-7, 7, -3][i], arms=[(60, y), (68, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="tight", mouth="big")


def milk_bubble_fx(cx, cy, r=12.0):
    """🥛 우유 생각 풍선 (배고플 때): 하얀 우유가 찬 컵."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    cup = T((cx - 5.5, cy - 7), (cx + 5.5, cy - 7), (cx + 4.2, cy + 7.5), (cx - 4.2, cy + 7.5))
    fx += [([cup], MILK_GLASS, 1.0), ([T((cx - 4.9, cy - 3), (cx + 4.9, cy - 3), (cx + 4.0, cy + 6.5), (cx - 4.0, cy + 6.5))], MILK, 0)]
    return fx


def su_hungry(sp, i):
    """배고파 시무룩: 두 손으로 배를 감싸고 처진 눈썹, 우유 생각 풍선."""
    s = hs()
    y = torso_top() + s["th"] * 0.68
    return front(hip=F([0, 1, 0][i]), arms=[(59, y), (69, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="dot", brow="sad", mouth="frown",
                 head_rot=-3, fx=milk_bubble_fx(104 - 2 * i, 20, 12.0))


EAT_MOUTH = 79.0  # 먹을 때 입 (프레임 y) — 그릇 밥 더미 바로 위


def su_eat(sp, i):
    """냠냠 (그릇 없음): 무릎 꿇고 고개를 숙여 입을 밥 더미 바로 위에 — 앙 ↔ 오물오물, 부스러기."""
    s = hs()
    # 입 = 머리 중심 + 0.62 hry, 머리 중심 = 몸 위 - hry + OVERLAP, 몸 위 = 엉덩이 + 0.35 lw - th
    hip_y = EAT_MOUTH + [0, 1, 0, 1][i] - 0.62 * s["hry"] + s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]
    hip_y = min(hip_y, FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - 3, foot_y()), (64 + s["hip"] + 3, foot_y())]
    arms = [(44, 96), (84, 96)]
    return front(hip=(64.0, hip_y), legs=legs, arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="happy" if i % 2 else "dot",
                 mouth="big" if i % 2 == 0 else "chew", fx=dp.eat_crumbs(i), cape=dict(sway=0))


def su_level_up(sp, i):
    """만세: 두 팔 번쩍, ^^, 반짝이 (가운데 칸은 살짝 뜀)."""
    up = [114, 124, 118][i]
    return front(hip=F(-[0, 3, 1][i]), legs=None if i != 1 else [("a", -6, 0.97), ("a", 6, 0.97)], arms=[("a", up, 1.0), ("a", up, 1.0)],
                 eyes="happy", mouth="big", fx=dp.level_up_stars(i))


# =====================================================================
# 구르기 (회전 원 안) — 빨간 망토에 싸인 공
# =====================================================================
def su_roll(sp, i):
    """빨간 망토로 몸을 감싸고 무릎을 끌어안은 동그란 공: 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            hrx, hry = 21.0, 19.5
            hx, hy = cx + 5, cy - 8
            body = Part([C(cx - 4, cy + 7, 21.0)], CAPE, [([T(*wave_hem((cx + 16, cy + 22), (cx - 24, cy + 22), 2, 3.0, (0, -1)),
                                                              (cx - 26, cy + 34), (cx + 18, cy + 34))], CAPE_IN)], outline=True, grow=0)
            knee = Part([E(cx + 2, cy + 6, cx + 28, cy + 24)], SUIT, outline=True, grow=0)
            boot = Part([E(cx - 2, cy + 18, cx + 16, cy + 29)], BOOT, outline=True, grow=0)
            hand = Part([C(cx + 22, cy + 15, 5.0)], SKIN, outline=True, grow=0)
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)], SKIN,
                        [([T((hx + 30, hy - 8), (hx + 14, hy - 9), (hx, hy - 12), (hx - 10, hy - 4), (hx - 30, hy + 10), (hx - 30, hy - 40),
                             (hx + 30, hy - 40))], HAIR), ([L(curl_pts(hx + 8, hy - 11, 2.6), 1.8)], HAIR)], outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.6)
            face = []
            for flip, lx in ((False, hx - 4), (True, hx + 10)):
                with xf(sx=0.7, sy=0.7, pivot=(lx, hy + 1)):
                    face += dp.eye("happy", lx, hy + 1, flip)
            face += mouth_ops("open" if i else "smile", hx + 4, hy + 12)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([body, knee, boot, head, hand], face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면 벽에 손발을 붙이고 엉금엉금, 망토가 등을 덮음 (머리는 언제나 팔 위)
# =====================================================================
CLIMB = [(1.0, 0.2), (0.6, 0.6), (0.2, 1.0), (0.6, 0.6)]  # (왼손, 오른손) 올린 몫 — 발은 엇갈려


def super_back(reach_, *, dy=0.0, scratch=False, sweat=False, sway=0.0):
    """뒷모습: 두 손을 머리 옆·위로 벽에 짚고 (reach: 0 어깨 옆 … 1 머리 위), 두 다리는 무릎을 벌려 벽에 디딤. 빨간 망토가 등을 덮음."""
    s = hs()
    hip = (64.0, 92.0 - (s["leg"] - 22) * 0.3 + dy)
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms, legs = [], []
    for side, r in zip((-1, 1), reach_):
        arms.append((64 + side * (s["hrx"] + 9 - 2 * r), top - s["arm"] * (0.05 + 0.65 * r)))
        lift = 1.0 - r
        legs.append((64 + side * (s["hip"] + s["leg"] * 0.62), hip[1] + s["leg"] * (0.75 - 0.45 * lift)))
    fx = []
    if scratch:
        for x, y in arms:
            fx.append(([L([(x + d, y - 12), (x + d * 1.2, y - 4)], 1.2) for d in (-3, 0, 3)], (150, 150, 170), 0))
    if sweat:
        fx += sweat_fx(64 + s["hrx"] + 6, top - s["hry"] * 1.2)
    return hero(view_="back", hip=hip, legs=legs, leg_bend=[(-1, -1), (1, -1)], arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], fx=fx,
                cape=dict(sway=sway))


def su_climb(sp, i):
    """뒷모습으로 기어오름: 왼손·오른손이 번갈아 위로, 발은 엇갈려 디딤, 몸 살짝 오르내림."""
    return super_back(CLIMB[i], dy=[0, -1, 0, -1][i], sway=[-1, 0, 1, 0][i])


def su_slide(sp, i):
    """뒷모습으로 주르륵: 두 손 쭉 위로, 긁힌 자국 3줄, 땀 한 방울."""
    return super_back((1.0, 1.0), dy=[0, 2][i], scratch=True, sweat=True)


# =====================================================================
# ✈️ 날기 (옆모습, 오른쪽으로) — 몸을 앞으로 눕혀 주먹을 앞으로, 다리는 모아 뒤로, 망토가 뒤로 휘날림
# =====================================================================
FLY_HIP = (44.0, 88.0)  # 나는 몸의 엉덩이 관절 (몸은 오른쪽 위로 뻗음 — 코어가 창을 바닥보다 띄움)


def wind_fx(x, ys, ln, shift=0.0):
    """뒤로 흐르는 바람 자국 (하늘색 가는 줄)."""
    return [([L([(x + shift, y), (x + ln + shift, y)], 1.6)], WIND, 0) for y in ys]


def fly_head(hip, rot, head_rot):
    """나는 몸의 (목, 머리 가운데) 프레임 자리 — hero와 같은 계산 (옆모습)."""
    s = hs()
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    neck = dp._apply(dp._xf_matrix(rot=rot, pivot=hip), (hip[0] + 1.5, top))
    hc = dp._apply(dp._xf_matrix(rot=rot + head_rot, pivot=neck), (neck[0] + 2.0, neck[1] - s["hry"] + OVERLAP))
    return neck, hc


def flyer(i, *, fast=False, fx=(), eyes="dot", brow="angry", mouth="smile"):
    """나는 슈퍼인간 한 장 (i: 망토 펄럭 칸): 몸을 거의 수평으로 눕히고 (머리가 오른쪽 앞) 고개를 들어 앞을 봄,
    가까운 주먹을 턱 아래로 쭉 앞으로 (빠를 땐 두 주먹 다), 먼 팔은 몸 옆에 붙여 뒤로, 다리는 모아 뒤로 쭉 (발끝 뒤로),
    망토는 등 위로 뒤로 휘날림."""
    s = hs()
    rot = 84.0 if fast else 78.0
    head_rot = -72.0 if fast else -66.0
    bob = [0.0, -0.8, 0.4][i % 3] * (0.4 if fast else 1.0)
    hip = (FLY_HIP[0] - (s["leg"] - 22) * 0.35, FLY_HIP[1] + bob)
    _, hc = fly_head(hip, rot, head_rot)
    sh = hero_shoulders("side", hip, rot)
    # 주먹: 턱 아래 앞쪽 (머리 아래끝 바로 밑, 얼굴 앞쪽으로) — 팔 길이 안으로
    fist = (hc[0] + s["hrx"] * 0.62, hc[1] + s["hry"] * 0.95 + s["aw"] * 0.25)
    near = reach(sh[1], fist, s["arm"], 1.0)
    if fast:
        far = reach(sh[0], (fist[0] - 3, fist[1] - 2), s["arm"], 1.0)
    else:
        far = ("a", -rot + 8, 0.92)  # 몸 옆에 붙여 뒤로
    lift = [0, 2, -1][i % 3]
    legs = [("a", -rot - 2 + lift, 0.98), ("a", -rot + 3 - lift, 0.98)]
    cape = dict(a=[20, 26, 15][i % 3] + (2 if fast else 0), flap=[3.0, -3.0, 0.8][i % 3], len=(1.3 if fast else 1.2) * min(1.4, (22.0 / s["leg"]) ** 0.5))
    return hero(hip=hip, rot=rot, head_rot=head_rot, legs=legs, leg_bend=[(1, -1), (1, -1)], arms=[far, near],
                arm_bend=[(0, 1), (0, 1)], far_limbs_front=fast, eyes=eyes, brow=brow, mouth=mouth, look=(1.2, -0.4), cape=cape,
                point=True, fx=fx)


def su_fly(sp, i):
    """✈️ 날기 (3칸 반복): 몸을 눕혀 가까운 주먹을 앞으로, 다리는 모아 뒤로, 망토 펄럭 (칸마다 끝단이 출렁), 바람 자국."""
    return flyer(i, fx=wind_fx(4, (80 + [0, 2, -1][i], 94 + [0, -1, 2][i]), 10, [0, -3, -1][i]))


def su_fly_fast(sp, i):
    """✈️💨 빨리 날기 (3칸 반복): 두 주먹을 앞으로 쭉, 몸이 더 곧게, 망토가 길게 뒤로, 속도선 여러 줄."""
    fx = wind_fx(2, (74, 84 + [0, 1, -1][i], 96, 104 + [0, -1, 1][i]), 14, [0, -4, -2][i])
    return flyer(i, fast=True, fx=fx, mouth="grin")


FLIP = [0, 90, 180, 270]  # 공중제비 칸마다 도는 각도 (시계 방향 = 오른쪽으로 날며 앞으로 돎)


def su_fly_flip(sp, i):
    """🔄 공중제비 (4칸 반복 — 코어가 한 바퀴 시간만큼): 무릎을 끌어안듯 웅크린 몸이 칸마다 90°씩 앞으로 돎, 망토가 둘레로 휘감김."""
    s = hs()
    cx, cy = 64.0, 72.0 - (s["hry"] - 22.8) * 0.8
    hip = (cx - 2, cy + s["th"] * 0.25)
    knee = hip[0] + s["leg"] * 0.55 + 3
    legs = [(knee + 1, hip[1] + s["leg"] * 0.25), (knee + 3, hip[1] + s["leg"] * 0.2)]
    arms = [(knee + 1, hip[1] - s["leg"] * 0.3 - 3), (knee + 4, hip[1] - s["leg"] * 0.3 - 1)]
    return hero(hip=hip, rot=18, head_rot=8, legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms, arm_bend=[(0, 1), (0, 1)],
                eyes=["happy", "tight", "happy", "dot"][i], mouth=["big", "open", "big", "grin"][i], cape=dict(a=70, flap=[2, -2, 2, -2][i], len=0.8),
                whole=dict(rot=FLIP[i], pivot=(cx, cy - 6)), fx=wind_fx(6, (70, 96), 8, [0, -2, 0, -2][i]))


def su_hover(sp, i):
    """🫧 둥실 (3칸 반복): 똑바로 선 채 떠 있음 — 발끝이 아래 (바닥에서 살짝 뜸), 두 주먹은 허리, 망토 살랑 (이륙·사뿐 착지·위아래 화면)."""
    s = hs()
    hip = H(SIDE_CX, -9.0 - [0, 1.0, 0.4][i])
    legs = [("a", -6, 0.98), ("a", 6, 0.96)]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    wy = top + s["th"] * 0.72
    arms = reach_all(hip, 0, [(hip[0] - 4, wy), (hip[0] + s["tw"] * 0.7, wy)])
    return hero(hip=hip, legs=legs, leg_bend=[(1, -0.2), (1, -0.2)], arms=arms, arm_bend=[(1, 0.2), (1, 0.2)], point=True,
                eyes="dot", mouth="smile", cape=dict(a=[16, 22, 18][i], flap=[1.5, -1.5, 0.5][i]))


# =====================================================================
# 💪 물건 들기 (흐물이 juggle 틀) — 한 손을 머리 위로 쭉 뻗어 번쩍 (다른 손은 허리), 손 자리 mark_grip
# =====================================================================
STRAIN_HANDS_X = 100  # 손바닥이 거대한 것 옆면에 (코어 juggle 거대한 것 앞 간격 몸 크기 0.28)


def one_hand_up(hip, rot=0.0):
    """한 손 번쩍의 (손 자리, 물건 바닥 자리): 물건은 머리 위 (앞머리 바로 위), 가까운 손바닥은 머리 앞쪽 위 옆면에서 그 물건을 받침
    (팔은 머리 뒤로 가려짐 — 얼굴을 가로지르지 않음)."""
    s = hs()
    _, hc = fly_head(hip, rot, 0.0)
    g = (hc[0] + 3.0, hc[1] - s["hry"] - s["ear"] * 0.5 - 1.0)
    hand = (hc[0] + s["hrx"] * 0.86 + 2.0, hc[1] - s["hry"] * 0.62)
    return hand, g


def lift_pose(hip, *, rot=0.0, legs=None, eyes="dot", mouth="grin", cape=None, far=None, head_rot=0.0):
    """한 손 번쩍: 물건을 머리 위에 얹고 가까운 손바닥 하나로 받침 (손 자리 grip = 물건 바닥), 먼 손은 허리에 주먹 · 여유로운 얼굴."""
    s = hs()
    hand, g = one_hand_up(hip, rot)
    dp.mark_grip(*g)
    if far is None:
        top = hip[1] + s["lw"] * 0.35 - s["th"]
        far = reach(hero_shoulders("side", hip, rot)[0], (hip[0] - 3, top + s["th"] * 0.7), s["arm"], 1.0)
    return hero(hip=hip, rot=rot, head_rot=head_rot, legs=legs or [floor_at(hip[0] - 7), floor_at(hip[0] + 7)], arms=[far, hand],
                arm_bend=[(1, 0.4), (-1, 0)], arms_behind=True, open_hand=(False, True), eyes=eyes, mouth=mouth, cape=cape or dict(a=12))


def su_lift(sp, i):
    """쪼그려 앞에 놓인 물건 바닥을 한 손으로 잡고 → 가슴께로 → 머리 위로 한 손 번쩍 (한 번, 마지막 칸 = hold)."""
    s = hs()
    if i == 0:
        hip = H(SIDE_CX - 2, s["leg"] * 0.4)
        g = (88.0, 119.0)
        hand = reach(hero_shoulders("side", hip, 30)[1], (g[0] - 2, g[1] - 2), s["arm"], 1.0)
        dp.mark_grip(*g)
        return hero(hip=hip, rot=30, head_rot=-18, legs=[floor_at(SIDE_CX - 8), floor_at(SIDE_CX + 8)], arms=[("a", 20, 0.9), hand],
                    arm_bend=[(0, 1), (0, 1)], open_hand=(False, True), eyes="dot", mouth="smile", cape=dict(a=34))
    if i == 1:
        hip = H(SIDE_CX, s["leg"] * 0.1)
        sh = hero_shoulders("side", hip, 4)[1]
        g = (sh[0] + s["arm"] * 0.85, sh[1] + 2)
        dp.mark_grip(*g)
        return hero(hip=hip, rot=4, legs=[floor_at(SIDE_CX - 7), floor_at(SIDE_CX + 7)], arms=[("a", 25, 0.9), (g[0], g[1] + 2)],
                    arm_bend=[(1, 0.4), (0, 1)], open_hand=(False, True), eyes="dot", mouth="grin", cape=dict(a=22))
    return lift_pose(H(SIDE_CX, [-1.0, 0.0][i - 2]), eyes="happy" if i == 3 else "dot", mouth="open")


def su_hold(sp, i):
    """한 손으로 머리 위에 든 채 버티고 섬 (가볍다는 듯 여유로운 미소), 숨 쉬듯 살짝."""
    return lift_pose(H(SIDE_CX, [0.0, 0.8][i]), mouth="smile", cape=dict(a=10, flap=[0, 1][i]))


def su_carry(sp, i):
    """한 손으로 머리 위에 든 채 성큼성큼 (날며 옮길 땐 코어가 창을 띄움): 다리는 걷기처럼, 망토 살랑."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    return lift_pose(hip, legs=legs, cape=dict(a=[16, 20, 16, 20][i], flap=[0, 1.5, 0, -1.5][i]))


def su_throw(sp, i):
    """한 손으로 젖혔다가 휙: 0 = 든 채 · 1 = 몸을 뒤로 젖혀 팔을 머리 뒤로 · 2 = 앞으로 확 내던지며 놓음 · 3 = 팔을 앞으로 뻗은 채 마무리 (한 번)."""
    s = hs()
    legs = [floor_at(SIDE_CX - 10), floor_at(SIDE_CX + 10)]
    hip = H(SIDE_CX, [0, 1.5, 2.5, 2.0][i])
    if i == 0:
        return lift_pose(hip, legs=legs)
    if i == 1:
        rot = -12
        sh = hero_shoulders("side", hip, rot)[1]
        g = (sh[0] - s["arm"] * 0.55, sh[1] - s["arm"] * 0.8)
        dp.mark_grip(*g)
        return hero(hip=hip, rot=rot, head_rot=-4, legs=legs, arms=[("a", 40, 0.9), (g[0], g[1] + 2)], arms_behind=True,
                    arm_bend=[(1, 0.4), (-1, 0)], open_hand=(False, True), eyes="tight", mouth="grin", cape=dict(a=4))
    rot = [0, 0, 16, 12][i]
    sh = hero_shoulders("side", hip, rot)
    g = (sh[1][0] + s["arm"] * 0.92, sh[1][1] + [0, 0, 3, 8][i])
    fx = dp.speed_fx(4, (50, 62, 74), 12) if i == 2 else []
    dp.mark_grip(*g)
    return hero(hip=hip, rot=rot, head_rot=-rot * 0.4, legs=legs, arms=[("a", -20, 0.9), (g[0] - 1, g[1] + 1)], arm_bend=[(0, 1), (0, 1)],
                open_hand=(False, True), eyes="dot", brow="angry", mouth="big" if i == 2 else "grin", cape=dict(a=[0, 0, 40, 30][i], flap=2),
                fx=fx)


def su_strain(sp, i):
    """흡! 거대한 것 앞에서 한 손바닥을 옆면 아래에 대고 들어 올리는 참 — 영웅이라 금방 (여유, 반짝)."""
    s = hs()
    sh_ = [0.0, 0.6, -0.4, 0.5][i]
    hip = (SIDE_CX + 8 + sh_ * 0.4, stand_hip_y() + s["leg"] * 0.25 + [0, 0.4, 0.2, 0.3][i])
    rot = 16
    shs = hero_shoulders("side", hip, rot)
    hand = reach(shs[1], (STRAIN_HANDS_X + sh_, shs[1][1] + 4), s["arm"], 1.0)
    legs = [floor_at(hip[0] - s["leg"] * 0.8 - 4), floor_at(hip[0] + 6)]
    return hero(hip=hip, rot=rot, head_rot=-14, legs=legs, leg_bend=[(1, -0.3), (1, -1)], arms=[("a", 30, 0.9), hand], arm_bend=[(1, 0.4), (0, 1)],
                open_hand=(False, True), eyes="dot", brow="angry", mouth="grin", cape=dict(a=30, flap=[1, -1, 1, -1][i]),
                fx=sparkle_fx(34, 30, 3.0 + [0, 1, 0, 1][i]) if i % 2 else [])


def su_present(sp, i):
    """짠! 배달 도착 (앞모습): 한 손으로 물건을 머리 위로 번쩍 든 채 (다른 손은 허리, 흔들) → 마지막 칸에 휙 떨굼 — 팔이 앞아래로 (한 번)."""
    s = hs()
    wy = torso_top() + s["th"] * 0.72
    if i < 3:
        hip = F([0, -1, 0][i])
        hy = torso_top(hip[1]) - s["hry"] + OVERLAP
        hand = (64.0 + s["hrx"] + 2.0, hy - s["hry"] * 0.55 + [0, -1, 1][i])
        g = (64.0 + [0, 1.5, -1.5][i], head_top_y(hip) - s["ear"] * 0.5 - 1.0)
        dp.mark_grip(*g)
        return front(hip=hip, arms=[(64 - s["tw"] * 0.92, wy), hand], arm_bend=[(-1, -0.2), (1, 0)], open_hand=(False, True),
                     eyes="happy" if i == 1 else "dot", mouth="big", cape=dict(sway=[0, 0.8, -0.8][i]))
    hip = F(0.5)
    g = (64.0, head_top_y(hip) - s["ear"] * 0.5 - 1.0)
    dp.mark_grip(*g)
    return front(hip=hip, arms=[(64 - s["tw"] * 0.92, wy), ("a", 40, 1.0)], arm_bend=[(-1, -0.2), (1, 0)], open_hand=(False, True), eyes="happy",
                 mouth="open")


def su_slump(sp, i):
    """털썩 (쉴 때만 — 영웅은 무거워서 주저앉지 않지만 juggle 틀이 요구): 바닥에 앉아 다리를 앞으로 쭉, 몸은 뒤로 기대 후유."""
    s = hs()
    hip = sit_hip(SIDE_CX - 6)
    legs = [(hip[0] + s["leg"] + 6, foot_y()), (hip[0] + s["leg"] + 9, foot_y())]
    arms = [(hip[0] - 10, FLOOR - s["aw"] * 0.6), (hip[0] - 6, FLOOR - s["aw"] * 0.6)]
    return hero(hip=hip, rot=-14, head_rot=[14, 18][i], legs=legs, arms=reach_all(hip, -14, arms), arm_bend=[(1, 0), (1, 0)],
                eyes="closed", mouth="open", fx=sweat_fx(98, [32, 34][i]), cape=dict(a=-20, len=0.7))


def reach_all(hip, rot, ends):
    """옆모습 두 팔 끝을 팔 길이 안으로."""
    return [reach(sh, e, hs()["arm"], 1.0) for sh, e in zip(hero_shoulders("side", hip, rot), ends)]


SUPER_POSES = {"walk": su_walk, "run": su_run, "roll": su_roll, "idle": su_idle, "look": su_look, "sleep": su_sleep, "hop": su_hop,
               "fall": su_fall, "dragged": su_dragged, "land": su_land, "sit_front": su_sit_front, "wave": su_wave, "tilt": su_tilt,
               "belly": su_belly, "stretch": su_stretch, "heart": su_heart, "tickle": su_tickle, "annoyed": su_annoyed,
               "hungry": su_hungry, "eat": su_eat, "level_up": su_level_up, "climb": su_climb, "slide": su_slide, "lookup": su_lookup,
               "lift": su_lift, "hold": su_hold, "carry": su_carry, "throw": su_throw, "strain": su_strain, "present": su_present, "slump": su_slump,
               "fly": su_fly, "fly_fast": su_fly_fast, "fly_flip": su_fly_flip, "hover": su_hover}
SUPER_POSES = {k: (v if k == "roll" else dp._fit(v)) for k, v in SUPER_POSES.items()}  # roll은 회전 원 안 그대로
SUPER_POSES["peek"] = dp.peek_from(SUPER_POSES["look"])  # 🫣 창 뒤 숨바꼭질 (흐물이 장난 — 두리번 칸을 뒤집어 옆면에서 자름)
JUGGLE_ANIMS = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]  # = Rust manifest::JUGGLE_ANIMS
SOAR_ANIMS = ["fly", "fly_fast", "fly_flip", "hover"]  # = Rust manifest::SOAR_ANIMS

SUPER_HERO = Species(
    id="super", name="슈퍼인간", walk_speed=95, scale=1.0, base=SUIT, back=CAPE, belly=SUIT, nose=OUT, mouth="w",
    front_tail_color=CAPE, tail_color=CAPE, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + JUGGLE_ANIMS + ["peek"] + SOAR_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=BELT, poses=SUPER_POSES,
    outline=OUT,
    # 색은 파스텔 없이 또렷한 파랑·빨강·노랑 · 크기는 SUPER가 정함 (fit 1)
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 슈퍼인간을 등록함."""
    dp.SPECIES[SUPER_HERO.id] = SUPER_HERO


register()


def preview_super(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hop", 1), ("roll", 0),
             ("fly", 0), ("fly", 1), ("fly_fast", 0), ("fly_flip", 1), ("hover", 0), ("hold", 0), ("carry", 1), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "fly"), bg=bg)


def preview_rows(by_stage, path, anims, bg=(255, 255, 255)):
    """동작들 (줄마다 한 동작의 모든 칸) × 5단계 (열 띠) — 새 그림 확인용."""
    from PIL import Image
    cell = 132
    im = Image.new("RGBA", (len(STAGES) * 4 * cell + (len(STAGES) - 1) * 12, len(anims) * cell), bg + (255,))
    for c, st in enumerate(STAGES):
        for r, a in enumerate(anims):
            for k, f in enumerate(by_stage[st][a]):
                im.alpha_composite(f, (c * (4 * cell + 12) + k * cell, r * cell))
    im.save(path)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(SUPER_HERO, root / "ui" / "pets" / SUPER_HERO.id))
    preview_super(by_stage, pv / "stages_super.png")
    dp.preview_back(by_stage, pv / "climb_super.png")
    preview_rows(by_stage, pv / "fly_super.png", SOAR_ANIMS + JUGGLE_ANIMS)
    print("ok: super")


if __name__ == "__main__":
    main()
