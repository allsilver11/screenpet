"""박쥐인간 🦇 — 짙은 회색 슈트에 박쥐 귀 가면·검정 망토를 두른 사람 모양 꼬마 영웅 펫(bat). 뼈대·IK는 거미인간(draw_spider)과 같은 틀이고,
부품마다 자기 외곽선을 두른 두 발 꼬마 — 팔다리는 관절(어깨·팔꿈치·손 / 엉덩이·무릎·발)을 IK로 굽힌다.

    python tools/draw_bat.py → ui/pets/bat/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_bat,climb_bat,bike_bat}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 큰 머리 (키의 45% 남짓) + 작은 몸 + 짧은 팔다리 둘씩. 머리 위쪽은 뾰족한 두 귀가 달린 검정 가면 (하얀 눈 — 표정마다 모양이 바뀜),
        코 아래 입·턱은 살색으로 드러남. 몸: 짙은 회색 슈트, 검정 장갑·장화, 노란 벨트, 가슴에 노란 타원 + 까만 박쥐 로고,
        등 뒤로 끝이 박쥐 날개처럼 들쭉날쭉한 검정 망토.
단계 (BAT): 아기 = 동글 꼬마 (머리 아주 큼, 팔다리 뭉툭, 귀 작음) → 유아 → 어린이 → 청소년 → 어른 키 크고 어깨 넓음, 귀·망토가 김.
특기 동작 (코어 brain_bike.rs): ride 🏍️ 큰 바퀴 검정 오토바이 타고 달리기 (바퀴가 굴러감) · wheelie 앞바퀴를 들고 뒷바퀴로만 ·
      bike_jump 오토바이째 붕 뜀 · drift 급정거 드리프트 (뒷바퀴 먼지·한 발 바닥) · signal 🔦 바닥의 큰 조명을 켜 하늘로 박쥐 신호 (빛줄기는 앱의 덮개 창).
      lift·hold·carry·throw·strain·present·slump 💪 물건 들기 (흐물이 juggle 틀 — 손 자리는 mark_grip, 두 손이 머리 위로 받침: 팔은 머리 뒤로 가려짐).
      벽 타기 climb·slide = 뒷모습 (머리가 언제나 팔 위 — 팔이 머리를 덮지 않음).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, _ST, anger_fx, mix, question_fx, sparkle_fx, sweat_fx, view, xf, z_fx  # noqa: E402
from draw_spider import dust_fx, ik, polar, target  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FLOOR = 125.0  # 바닥 (장화 바닥)
SUIT = (126, 132, 158)  # 짙은 회색 슈트 (몸·팔·다리)
SUIT_FAR = (104, 110, 136)  # 먼 쪽 팔다리
BLACK = (40, 40, 54)  # 가면·장갑·장화
BLACK_FAR = (32, 32, 44)
CAPE = (44, 44, 60)  # 망토
CAPE_IN = (70, 62, 100)  # 망토 안감 (보랏빛, 펄럭일 때 살짝)
BELT = (250, 204, 64)  # 노란 벨트
BELT_DARK = (204, 150, 34)  # 벨트 주머니·버클 테
LOGO_BG = (252, 214, 84)  # 가슴 로고 노란 타원
LOGO_BAT = (30, 30, 42)  # 로고 속 까만 박쥐
EYE_W = (255, 255, 255)  # 가면의 하얀 눈
SKIN, SKIN_SHADE = (255, 222, 198), (236, 188, 164)  # 드러난 입·턱
MOUTH = (150, 64, 70)
BLUSH = (250, 156, 156)
OUT = (16, 14, 24)  # 외곽선
DUST, DUST_LINE = (236, 228, 214), (176, 160, 150)
# 🏍️ 오토바이
BIKE = (46, 48, 64)  # 차체
BIKE_HI = (92, 98, 124)  # 차체 줄 · 핸들
TYRE = (28, 28, 36)  # 굵은 타이어
TREAD = (70, 70, 88)  # 타이어 홈 (굴러가는 게 보이게)
RIM = (120, 126, 148)  # 바퀴 테
HUB = (200, 204, 218)  # 바퀴 가운데
SEAT = (26, 26, 34)
LAMP = (255, 228, 110)  # 전조등·신호 조명 렌즈
FLAME = (255, 150, 60)  # 배기 불꽃
SKID = (120, 120, 132)  # 바닥 미끄럼 자국
SPOT = (70, 72, 90)  # 🔦 신호 조명 통

# 단계(lvl)별 — hrx·hry: 머리 반지름 · tw: 어깨 반폭 · th: 몸 높이 · hip: 다리 뿌리 반간격 · leg: 다리 길이 · lw: 다리 굵기 ·
# arm: 팔 길이 · aw: 팔 굵기 · ear: 귀 높이 · eye: 눈 배율 · emb: 가슴 로고 배율 · cape: 망토 길이 몫 · bike: 오토바이 배율
BAT = {
    0: dict(hrx=27.0, hry=25.0, tw=13.5, th=21.0, hip=6.6, leg=9.5, lw=10.4, arm=13.0, aw=8.8, ear=12.0, eye=1.12, emb=0.8, cape=0.5, bike=0.84),
    1: dict(hrx=26.4, hry=24.2, tw=13.4, th=22.5, hip=6.8, leg=13.5, lw=9.8, arm=15.0, aw=8.3, ear=13.0, eye=1.07, emb=0.85, cape=0.6, bike=0.88),
    2: dict(hrx=25.8, hry=23.4, tw=13.6, th=24.5, hip=7.0, leg=18.0, lw=9.4, arm=17.5, aw=7.9, ear=14.5, eye=1.03, emb=0.92, cape=0.66, bike=0.92),
    3: dict(hrx=25.2, hry=22.8, tw=14.6, th=26.5, hip=7.2, leg=22.0, lw=9.2, arm=20.0, aw=7.6, ear=15.5, eye=1.0, emb=1.0, cape=0.7, bike=0.96),
    4: dict(hrx=24.6, hry=22.2, tw=16.4, th=29.5, hip=8.0, leg=26.0, lw=9.6, arm=23.0, aw=7.9, ear=16.5, eye=0.94, emb=1.1, cape=0.74, bike=1.0),
}
OVERLAP = 5.0  # 머리가 몸 윗부분을 덮는 깊이


def hs():
    """지금 단계의 꼬마 영웅 치수."""
    return BAT[_ST["lvl"]]


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


def bat_shape(cx, cy, w):
    """박쥐 실루엣 다각형 점들 (가운데 (cx, cy), 반폭 w): 두 귀 + 위로 솟은 날개 + 아래가 들쭉날쭉한 날개 끝 (로고·오토바이 무늬)."""
    top = [(0.07, -0.22), (0.15, -0.5), (0.23, -0.2), (0.52, -0.36), (1.0, -0.16)]
    bot = [(0.84, 0.12), (0.68, 0.02), (0.56, 0.3), (0.4, 0.1), (0.26, 0.36), (0.12, 0.18), (0.0, 0.46)]
    right = top + bot
    left = [(-x, y) for x, y in reversed(right[:-1])]
    return [(cx + x * w, cy + y * w) for x, y in right + left]


def logo_ops(ex, ey, k=1.0, sx=1.0):
    """가슴 로고: 노란 타원 + 까만 박쥐 (sx: 옆모습은 가로로 좁게)."""
    k *= hs()["emb"]
    with xf(sx=sx, pivot=(ex, ey)):
        oval = [E(ex - 7.4 * k, ey - 4.3 * k, ex + 7.4 * k, ey + 4.3 * k)]
        bat = [T(*bat_shape(ex, ey + 0.2 * k, 6.2 * k))]
    return [(oval, LOGO_BG), (bat, LOGO_BAT)]


def scallop_hem(right, left, n, depth, up):
    """망토 아랫단: right → left 사이를 n번 위로 오목하게 파인 박쥐 날개 끝 (up: 오목하게 들어가는 방향 단위 벡터)."""
    pts = []
    for j in range(n):
        a = (right[0] + (left[0] - right[0]) * j / n, right[1] + (left[1] - right[1]) * j / n)
        b = (right[0] + (left[0] - right[0]) * (j + 1) / n, right[1] + (left[1] - right[1]) * (j + 1) / n)
        for t in (0.0, 0.2, 0.4, 0.6, 0.8):
            bump = depth * 4 * t * (1 - t)
            pts.append((a[0] + (b[0] - a[0]) * t + up[0] * bump, a[1] + (b[1] - a[1]) * t + up[1] * bump))
    pts.append(left)
    return pts


def limb(root, end, ln, w, color, end_col, prefer, *, boot=False, toe=0.0, open_hand=False):
    """팔 또는 다리 한 짝 (자기 외곽선): 회색 마디 둘 + 끝은 검정 (장화: 정강이 아래 + 동글 장화 · 장갑: 손목 + 동글 주먹).
    toe: 장화 코 방향 (옆모습 +1, 앞모습 0) · open_hand: 손바닥을 펴 조금 큰 장갑."""
    j = ik(root, end, ln, prefer)
    shapes_ = [L([root, j, end], w)]
    seg = math.hypot(j[0] - end[0], j[1] - end[1]) or 1.0
    k = min(0.5, w * 0.95 / seg) if boot else min(0.4, w * 0.8 / seg)
    cuff = (end[0] + (j[0] - end[0]) * k, end[1] + (j[1] - end[1]) * k)
    blk = [L([cuff, end], w + 2.5)]
    if boot:
        x, y = end
        b = E(x - w * 0.62 + min(toe, 0) * w * 0.42, y - w * 0.5, x + w * 0.62 + max(toe, 0) * w * 0.42, y + w * 0.5)
        shapes_.append(b)
        blk.append(b)
    else:
        fist = C(end[0], end[1], w * (0.74 if open_hand else 0.66))
        shapes_.append(fist)
        blk.append(fist)
        if open_hand:  # 편 손바닥: 팔이 오는 반대쪽으로 손가락 셋
            a = math.degrees(math.atan2(end[0] - j[0], end[1] - j[1]))
            for da in (-32, 0, 32):
                tip = polar(end, a + da, w * 1.05)
                f = L([end, tip], w * 0.36)
                shapes_.append(f)
                blk.append(f)
    return Part(shapes_, color, [(blk, end_col)], outline=True, grow=0), j


# =====================================================================
# 가면 얼굴: 하얀 눈 (표정) · 입 · 볼
# =====================================================================
def eye_ops(kind, cx, cy, side, brow=None, sx=1.0):
    """가면의 하얀 눈 하나 (side: -1 왼쪽, +1 오른쪽 — 바깥이 살짝 올라간 아몬드). kind: dot · up · round · happy · closed · tight · flat · squint · spiral.
    brow: angry (안쪽이 눌린 눈꺼풀) · sad (바깥이 처진 눈꺼풀). 눈꺼풀은 가면색."""
    s = hs()
    k = s["eye"]
    rx, ry = 0.25 * s["hrx"] * k, 0.2 * s["hry"] * k
    tilt = -14 * side
    ops = []
    with xf(sx=sx, pivot=(cx, cy)):
        if kind in ("happy", "closed", "tight"):
            if kind == "happy":  # ^^
                pts = dp.arc(cx, cy + ry * 0.7, rx * 1.0, ry * 1.2, 200, 340, 12)
            elif kind == "closed":  # ‿
                pts = dp.arc(cx, cy - ry * 0.4, rx * 1.0, ry * 0.8, 25, 155, 12)
            else:  # > <
                ix = -side
                pts = [(cx - ix * rx * 0.8, cy - ry * 0.8), (cx + ix * rx * 0.7, cy), (cx - ix * rx * 0.8, cy + ry * 0.8)]
            return [([L(pts, 2.6)], EYE_W)]
        if kind == "round":
            rx, ry, tilt = rx * 1.1, ry * 1.35, 0
        if kind == "up":
            cy -= 1.4
        with xf(rot=tilt, pivot=(cx, cy)):
            ops.append(([E(cx - rx, cy - ry, cx + rx, cy + ry)], EYE_W))
            if kind == "spiral":
                pts = [(cx + 0.3 * a * math.cos(a), cy + 0.3 * a * math.sin(a)) for a in (j * 0.35 for j in range(1, 24))]
                ops.append(([L(pts, 1.1)], BLACK))
        lid = None
        o, i_ = cx + side * (rx + 2.5), cx - side * (rx + 2.5)
        if kind == "flat":
            lid = ((i_, cy - ry * 0.05), (o, cy - ry * 0.05))
        elif kind == "squint":
            lid = ((i_, cy - ry * 0.3), (o, cy - ry * 0.3))
        elif brow == "angry":
            lid = ((i_, cy + ry * 0.05), (o, cy - ry * 0.85))
        elif brow == "sad":
            lid = ((i_, cy - ry * 0.85), (o, cy + ry * 0.05))
        if lid:
            top = cy - ry - 4
            ops.append(([T(lid[0], lid[1], (lid[1][0], top), (lid[0][0], top))], BLACK))
        if kind == "squint":
            bot = cy + ry + 4
            b0, b1 = (i_, cy + ry * 0.4), (o, cy + ry * 0.4)
            ops.append(([T(b0, b1, (o, bot), (i_, bot))], BLACK))
    return ops


def mouth_ops(kind, cx, cy):
    """드러난 턱의 입: None 없음 · smile 빙긋 · open · big · grin · frown · pout · chew."""
    if kind is None:
        return []
    if kind == "smile":
        return [([L(dp.arc(cx, cy - 1.6, 3.2, 2.4, 25, 155, 10), 1.4)], MOUTH)]
    if kind == "open":
        return dp.open_mouth(cx, cy - 2.2, 3.0, 3.2)
    if kind == "big":
        return dp.open_mouth(cx, cy - 2.8, 4.0, 4.4)
    if kind == "grin":
        return [([T(*dp.arc(cx, cy - 2.0, 4.6, 3.6, 0, 180, 12))], MOUTH), ([L([(cx - 3.6, cy - 1.4), (cx + 3.6, cy - 1.4)], 1.4)], EYE_W)]
    if kind == "frown":
        return [([L(dp.arc(cx, cy + 1.8, 3.2, 2.0, 205, 335, 10), 1.4)], MOUTH)]
    if kind == "pout":
        return [([L(dp.arc(cx, cy + 1.2, 2.0, 1.4, 205, 335, 8), 1.3)], MOUTH)]
    if kind == "chew":
        return [([E(cx - 2.2, cy - 1.3, cx + 2.2, cy + 1.3)], MOUTH)]
    raise ValueError(kind)


def cowl_marks(view_, hx, hy):
    """머리 위쪽을 덮는 검정 가면 (머리 도형 안으로 잘림): 앞모습은 코끝까지 내려와 볼 옆은 턱 근처까지, 옆모습은 뒤통수 전체 +
    얼굴 쪽은 코 아래부터 살색 · 뒷모습은 전부 가면."""
    s = hs()
    hrx, hry = s["hrx"], s["hry"]
    sky = hy - hry - s["ear"] - 8
    if view_ == "back":
        shine = (74, 76, 98)
        return [([E(hx - hrx * 2, sky, hx + hrx * 2, hy + hry * 2)], BLACK), ([L(dp.arc(hx, hy, hrx * 0.78, hry * 0.74, 205, 300, 12), 2.2)], shine)]
    if view_ == "front":
        edge = [(hx + hrx * 1.2, hy + 0.62 * hry), (hx + 0.62 * hrx, hy + 0.36 * hry), (hx + 0.28 * hrx, hy + 0.2 * hry),
                (hx, hy + 0.3 * hry), (hx - 0.28 * hrx, hy + 0.2 * hry), (hx - 0.62 * hrx, hy + 0.36 * hry), (hx - hrx * 1.2, hy + 0.62 * hry)]
        return [([T(*edge, (hx - hrx * 1.2, sky), (hx + hrx * 1.2, sky))], BLACK)]
    edge = [(hx + hrx * 1.2, hy + 0.3 * hry), (hx + 0.95 * hrx, hy + 0.32 * hry), (hx + 0.62 * hrx, hy + 0.18 * hry),
            (hx + 0.22 * hrx, hy + 0.34 * hry), (hx - 0.12 * hrx, hy + 0.7 * hry), (hx - 0.3 * hrx, hy + hry * 1.2)]
    return [([T(*edge, (hx - hrx * 1.2, hy + hry * 1.2), (hx - hrx * 1.2, sky), (hx + hrx * 1.2, sky))], BLACK)]


def ear_shapes(view_, hx, hy):
    """가면의 뾰족한 두 귀 (머리 도형에 합침)."""
    s = hs()
    hrx, hry, eh = s["hrx"], s["hry"], s["ear"]
    out = []
    if view_ == "side":
        spots = [(-0.34, 0.92), (0.2, 1.0)]  # (가운데 x 몫, 크기 몫) — 먼 귀가 뒤
    else:
        spots = [(-0.52, 1.0), (0.52, 1.0)]
    for cx_k, kk in spots:
        cx = hx + cx_k * hrx
        half = 0.16 * hrx
        base_y = hy - hry * math.sqrt(max(0.0, 1 - (cx_k) ** 2)) + 4.0
        tip = (cx + (0.08 * hrx if cx_k > 0 else -0.08 * hrx) * (1 if view_ != "side" else 0) - (1.5 if view_ == "side" else 0), base_y - 4.0 - eh * kk)
        out.append(T((cx - half, base_y + 1.5), (cx + half, base_y + 1.5), tip))
    return out


def face_ops(view_, hx, hy, eyes, brow, mouth, look, blush_on):
    """얼굴 ops (하얀 눈 둘 + 볼 + 입) — 머리 변환 안에서 부름. 옆모습은 얼굴이 오른쪽으로 쏠리고 먼 눈은 좁게."""
    s = hs()
    hrx, hry = s["hrx"], s["hry"]
    lx, ly = look
    ops = []
    if view_ == "front":
        fy = hy - 0.06 * hry + ly
        for side in (-1, 1):
            ops += eye_ops(eyes, hx + side * 0.36 * hrx + lx, fy, side, brow)
        if blush_on:
            ops += [([E(hx + side * 0.5 * hrx - 3.0, hy + 0.56 * hry - 1.6, hx + side * 0.5 * hrx + 3.0, hy + 0.56 * hry + 1.4)], BLUSH)
                    for side in (-1, 1)]
        mx, my = hx + lx * 0.5, hy + 0.66 * hry
    else:
        fy = hy - 0.06 * hry + ly
        ops += eye_ops(eyes, hx + 0.8 * hrx + lx * 0.6, fy - 0.5, 1, brow, sx=0.5)
        ops += eye_ops(eyes, hx + 0.34 * hrx + lx, fy, -1, brow, sx=0.85)
        if blush_on:
            ops.append(([E(hx + 0.36 * hrx - 2.8, hy + 0.58 * hry - 1.6, hx + 0.36 * hrx + 2.8, hy + 0.58 * hry + 1.4)], BLUSH))
        mx, my = hx + 0.7 * hrx, hy + 0.64 * hry
    return ops + mouth_ops(mouth, mx, my)


# =====================================================================
# 망토
# =====================================================================
def cape_side(neck, length, a, width, flap=0.0):
    """옆모습 망토 (오른쪽을 봄): 목에서 a도만큼 뒤로 날리는 천 (0 = 똑바로 아래, 90 = 뒤로 수평), 끝은 박쥐 날개처럼 들쭉날쭉.
    flap: 끝단의 물결 (칸마다 바꿔 펄럭)."""
    r = math.radians(a)
    d = (-math.sin(r), math.cos(r))  # 늘어지는 쪽
    b = (-math.cos(r), -math.sin(r))  # 뒤쪽 (a = 0이면 왼쪽)

    def P(u, v):
        return (neck[0] + d[0] * u + b[0] * v, neck[1] + d[1] * u + b[1] * v)
    front_bot, back_bot = P(length, -3 - flap), P(length + flap * 0.6, width * 1.45)
    pts = [P(0, -3.5), P(length * 0.5, -3.5 - flap * 0.4), front_bot]
    pts += scallop_hem(front_bot, back_bot, 3, min(8.0, length * 0.24), (-d[0], -d[1]))
    pts += [P(length * 0.5, width * 1.05 + flap * 0.5), P(0, 6.5)]
    lining = [T(P(length * 0.15, -3.5), front_bot, P(length * 0.8, 1.5))]
    return Part([T(*pts)], CAPE, [(lining, CAPE_IN)] if a > 40 else [], outline=True, grow=0)


def cape_front(cx, top, hem_y, tw, sway=0.0):
    """앞·뒷모습 망토 (몸 뒤로 양옆이 보이는 천, 뒷모습은 등을 덮음): 어깨에서 아래로 넓어지고 끝단은 들쭉날쭉."""
    right, left = (cx + tw * 2.2 + sway, hem_y), (cx - tw * 2.2 + sway, hem_y)
    pts = [(cx - tw * 0.92, top), (cx + tw * 0.92, top)] + scallop_hem(right, left, 4, min(6.5, (hem_y - top) * 0.16), (0, -1))
    return Part([T(*pts)], CAPE, outline=True, grow=0)


# =====================================================================
# 꼬마 영웅 한 장
# =====================================================================
def hero_shoulders(view_, hip, rot):
    """hero가 쓰는 어깨 자리 [먼/왼, 가까운/오른] (프레임 좌표 — 오토바이 핸들을 손에 맞출 때)."""
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
    """똑바른 몸의 머리 꼭대기 y (귀 빼고)."""
    s = hs()
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    return top - 2 * s["hry"] + OVERLAP


def hero(*, view_="side", hip=None, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0), legs=None, arms=None, leg_bend=None, arm_bend=None,
         eyes="dot", brow=None, mouth="smile", look=(0.0, 0.0), blush_on=True, fx=(), ops=(), held=(), whole=None, grip=None,
         arms_front=False, arms_behind=False, far_limbs_front=False, cape=None, vehicle=(), open_hand=(False, False)):
    """꼬마 영웅. view_: side (오른쪽을 봄) · front · back · hip: 엉덩이 관절 (없으면 똑바로 선 자리) · rot: 몸 기울기 (엉덩이 기준, 시계 방향) ·
    head_rot·head_d: 고개 · legs·arms: [먼/왼, 가까운/오른] 끝 자리 (target 형식) · leg_bend·arm_bend: 무릎·팔꿈치 굽는 쪽 ·
    held: 몸과 머리 사이 Part 함수 · fx: 효과 목록 또는 손 자리를 받아 효과를 돌려주는 함수 · grip: mark_grip할 손 ("near"·"far"·"left"·"right") ·
    arms_front: 앞모습 팔을 머리 위에 · arms_behind: 옆모습 가까운 팔도 머리 뒤에 (머리 위로 든 물건 — 팔이 얼굴을 가로지르지 않게) ·
    far_limbs_front: 먼 팔도 몸 앞에 · cape: 망토 dict(a=날림 각도, flap=펄럭, len=길이 배) 또는 False ·
    vehicle: 먼 다리 뒤·가까운 다리 앞 사이에 그릴 Part들 (🏍️ 오토바이) · open_hand: 손바닥 편 장갑."""
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
            p, _ = limb(leg_roots[j], end, s["leg"], lw, SUIT_FAR if far else SUIT, BLACK_FAR if far else BLACK, leg_pref[j], boot=True,
                        toe=(1 if side_view else 0) if view_ != "back" else 0)
            leg_parts.append(p)
            end = target(sh_roots[j], arms[j], s["arm"], arm_sides[j])
            p, _ = limb(sh_roots[j], end, s["arm"], aw, SUIT_FAR if far else SUIT, BLACK_FAR if far else BLACK, arm_pref[j], open_hand=open_hand[j])
            arm_parts.append(p)
            hands.append(end)
        th = s["th"]
        # 몸통 (+ 노란 벨트·가슴 로고)
        with xf(rot=rot, pivot=(hx0, hy0)):
            tw = s["tw"] if not side_view else s["tw"] * 0.95
            if _ST["lvl"] == 0:
                tshape = [E(hx0 - tw * 1.04, top, hx0 + tw * 1.04, bottom + 1.5)]
            else:
                tshape = [E(hx0 - tw, top, hx0 + tw, top + th * 0.78), E(hx0 - tw * 0.84, top + th * 0.32, hx0 + tw * 0.84, bottom)]
            by0, by1 = bottom - th * 0.3, bottom - th * 0.12
            marks = [([T((hx0 - tw * 1.2, by0), (hx0 + tw * 1.2, by0 - 0.6), (hx0 + tw * 1.2, by1 - 0.6), (hx0 - tw * 1.2, by1))], BELT)]
            if view_ == "front":
                for dx in (-0.62, -0.34, 0.34, 0.62):
                    marks.append(([E(hx0 + tw * dx - 1.4, by0 + 0.6, hx0 + tw * dx + 1.4, by1 + 0.4)], BELT_DARK))
                marks.append(([E(hx0 - 2.4, by0 - 0.4, hx0 + 2.4, by1 + 0.4)], BELT_DARK))
                marks.append(([E(hx0 - 1.3, by0 + 0.8, hx0 + 1.3, by1 - 0.8)], BELT))
            elif view_ == "side":
                marks.append(([E(hx0 + tw * 0.5 - 1.4, by0 + 0.6, hx0 + tw * 0.5 + 1.4, by1 + 0.4)], BELT_DARK))
            if view_ == "side":
                marks += logo_ops(hx0 + tw * 0.4, top + th * 0.36, 0.95, sx=0.6)
            elif view_ == "front":
                marks += logo_ops(hx0, top + th * 0.36)
            torso = Part(tshape, SUIT, marks, outline=True, grow=0)
            neck = (hx0 + (1.5 if side_view else 0.0), top)
            if view_ == "side":
                cape_part = cape_side(neck, (th + s["leg"] * 0.9) * cape.get("len", 1.0) * (0.55 + s["cape"] * 0.6) / 0.95,
                                      cape.get("a", 8.0), tw * 1.1 + 2, cape.get("flap", 0.0)) if cape is not False else None
            else:
                hem = min(FLOOR - 4, hy0 + (s["leg"] * s["cape"] + 4 if view_ == "front" else s["lw"] * 0.2))
                cape_part = cape_front(hx0, top + 1.5, hem, tw, cape.get("sway", 0.0) if cape else 0.0) if cape is not False else None
        neck = P(neck)
        # 머리 (박쥐 귀 가면 + 드러난 턱)
        hrx, hry = s["hrx"], s["hry"]
        with xf(rot=rot + head_rot, pivot=neck):
            hx, hy = neck[0] + head_d[0] + (2.0 if side_view else 0.0), neck[1] - hry + OVERLAP + head_d[1]
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)] + ear_shapes(view_, hx, hy), SKIN, cowl_marks(view_, hx, hy), outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.7)
            face = [] if view_ == "back" else face_ops(view_, hx, hy, eyes, brow, mouth, look, blush_on)
        parts = [cape_part] if cape_part and view_ != "back" else []
        if side_view:
            parts += [arm_parts[0]] if not far_limbs_front else []
            parts += [leg_parts[0]] + [v() for v in vehicle] + [leg_parts[1], torso]
            parts += [arm_parts[1]] if arms_behind else []
            parts += [h() for h in held]
            parts += [head]
            parts += ([arm_parts[0]] if far_limbs_front else []) + ([] if arms_behind else [arm_parts[1]])
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


def bt_walk(sp, i):
    """총총 걷기: 두 다리를 번갈아 앞뒤로, 팔은 반대로 흔듦, 몸 통통, 망토 살랑."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    sw = [1, 0, -1, 0][i]
    arms = [("a", 22 * sw, 0.96), ("a", -22 * sw + 4, 0.96)]
    return hero(hip=hip, legs=legs, arms=arms, rot=2, cape=dict(a=[14, 18, 14, 18][i], flap=[0, 1.5, 0, -1.5][i]))


def bt_run(sp, i):
    """다다다 달리기: 앞으로 기울고 크게 성큼, 팔을 힘차게, 망토가 뒤로 휘날림, 속도선."""
    st = [1, 0, -1, 0][i]
    hip = H(dy=[-1.5, -3.5, -1.5, -3.5][i])
    legs = [("a", -40 * st, 0.95 if st else 0.75), ("a", 40 * st, 0.95 if st else 0.75)]
    if st == 0:
        legs = [("a", -25, 0.8), ("a", 35, 0.7)] if i == 1 else [("a", 35, 0.7), ("a", -25, 0.8)]
    arms = [("a", 55 * st + 5, 0.8), ("a", -45 * st + 5, 0.8)]
    return hero(hip=hip, legs=legs, arms=arms, rot=12, mouth="open", arm_bend=[(1, 0.3), (1, 0.3)],
                cape=dict(a=[58, 66, 58, 66][i], flap=[2, -2, 2, -2][i]), fx=dp.speed_fx(4, (70, 86, 102), 10, [0, -3, 0, -3][i]))


def bt_idle(sp, i):
    """제자리 숨쉬기: 가슴이 살짝 오르내림, 손은 허리께."""
    return hero(hip=H(dy=[0, 0.8][i]), arms=[("a", 12, 0.97), ("a", 8 + 3 * i, 0.97)], head_d=(0, [0, 0.6][i]))


def bt_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞 (고개만)."""
    rot, eyes, look = [(-8, "up", (0.5, -1.0)), (0, "dot", (1.2, 0)), (8, "dot", (0.6, 1.5)), (0, "closed", (0, 0))][i]
    return hero(head_rot=rot, eyes=eyes, look=look)


def sit_hip(x=SIDE_CX):
    """바닥에 앉은 엉덩이 관절."""
    return (x, FLOOR - hs()["lw"] * 0.75)


def bt_sleep(sp, i):
    """망토를 덮고 웅크려 쿨쿨: 바닥에 앉아 무릎을 끌어안고 고개를 무릎에 묻음, 눈 ‿, z."""
    s = hs()
    hip = sit_hip(SIDE_CX - 6)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 3, foot_y()), (knee_x + 5, foot_y())]
    arms = [(knee_x + 1, hip[1] - s["leg"] * 0.45 - 4), (knee_x + 3, hip[1] - s["leg"] * 0.45 - 2)]
    fx = [z_fx(106, 34 - 3 * i, 4.0), z_fx(116, 20 - 3 * i, 2.6)]
    return hero(hip=hip, rot=14 + i, head_rot=10, head_d=(2, 1 + 0.6 * i), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                arm_bend=[(0, 1), (0, 1)], eyes="closed", mouth=None, fx=fx, cape=dict(a=-12, len=0.75))


def bt_annoyed(sp, i):
    """털썩 앉아 팔짱 끼고 눈 반쯤 감은 뾰로통 💢."""
    s = hs()
    hip = sit_hip(SIDE_CX - 4)
    legs = [(hip[0] + s["leg"] + 4, foot_y()), (hip[0] + s["leg"] + 7, foot_y())]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms = [(hip[0] + 8, top + s["th"] * 0.45), (hip[0] + 9, top + s["th"] * 0.5)]
    return hero(hip=hip, rot=-6, legs=legs, arms=arms, arm_bend=[(1, 0.8), (1, 0.8)], eyes="flat", mouth="pout", head_rot=-4,
                fx=[anger_fx(102 + i, 30 - i, 5.0 + 0.6 * i)], cape=dict(a=4, len=0.7))


def bt_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 두 팔 번쩍 솟음 (다리 모음) → 다리 내림."""
    s = hs()
    if i == 0:
        hip = H(dy=s["leg"] * 0.32)
        return hero(hip=hip, rot=12, legs=[floor_at(SIDE_CX - 3), floor_at(SIDE_CX + 3)], arms=[("a", -30, 0.9), ("a", -25, 0.9)],
                    eyes="tight", mouth="open", cape=dict(a=20))
    hip = (SIDE_CX, [0, 84, 90][i] - (s["leg"] - 22) * 0.5)
    legs = [("a", -30, 0.7), ("a", 25, 0.62)] if i == 1 else [("a", -8, 0.92), ("a", 10, 0.9)]
    arms = [("a", 160, 0.95), ("a", 150, 0.95)] if i == 1 else [("a", 110, 0.95), ("a", 100, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="open", arm_bend=[(1, 0), (1, 0)], cape=dict(a=[0, -10, 25][i], flap=2))


def bt_fall(sp, i):
    """떨어짐: 팔다리를 허우적, 동그란 눈, 망토가 위로 펄럭."""
    w = [0, 1][i]
    hip = (SIDE_CX, 84.0)
    legs = [("a", -40 - 15 * w, 0.85), ("a", 40 + 15 * w, 0.75)]
    arms = [("a", 150 - 25 * w, 0.95), ("a", 125 + 25 * w, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="round", mouth="open", rot=[-6, 6][i], arm_bend=[(0, -1), (0, -1)],
                cape=dict(a=[150, 140][i], flap=[2, -2][i], len=0.8))


def bt_dragged(sp, i):
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


def bt_land(sp, i):
    """착지 (한 번): 무릎을 굽혀 쿵 → 바로 섬."""
    s = hs()
    if i == 0:
        return hero(hip=H(dy=s["leg"] * 0.3), rot=10, legs=[floor_at(SIDE_CX - 4), floor_at(SIDE_CX + 4)], arms=[("a", 60, 0.9), ("a", 50, 0.9)],
                    eyes="tight", mouth="open", cape=dict(a=40, flap=2))
    return hero(eyes="happy")


def bt_lookup(sp, i):
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


def bt_sit_front(sp, i):
    """정면 서기: 허리에 손 얹은 영웅 자세, 망토가 양옆으로 (마지막 장 깜빡)."""
    s = hs()
    wy = torso_top() + s["th"] * 0.72
    arms = [(64 - s["tw"] * 0.88, wy), (64 + s["tw"] * 0.88, wy)]
    return front(arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="closed" if i == 3 else "dot", head_d=(0, [0, 0.5, 0, 0][i]),
                 cape=dict(sway=[0, 0.6, 0, -0.6][i]))


def bt_wave(sp, i):
    """한 손을 번쩍 들어 흔들흔들 (다른 손은 허리)."""
    a = [118, 140, 118, 140][i]
    return front(arms=[("a", 18, 0.95), ("a", a, 1.0)], arm_bend=[(-1, 0.3), (1, 0)], eyes="happy" if i % 2 else "dot",
                 mouth="open", head_rot=[-3, -1, -3, -1][i], open_hand=(False, True))


def bt_tilt(sp, i):
    """고개 갸웃 + ? (한 손은 턱)."""
    top = torso_top()
    return front(head_rot=[8, 12, 12][i], arms=[("a", 15, 0.95), (64 + 6, top + 2)], arm_bend=[(-1, 0.3), (1, 0.6)], arms_front=True,
                 fx=[question_fx(106, 24 - i)], mouth="pout")


def bt_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 두 다리를 위로 버둥, 두 팔 번쩍, 깔깔."""
    s = hs()
    w = [0, 2, 0, -2][i]
    hip = (64.0, FLOOR - s["lw"] * 0.8)
    legs = [("a", 112 + 6 * w, 0.9), ("a", 112 - 6 * w, 0.9)]
    arms = [("a", 128 - 5 * w, 0.95), ("a", 128 + 5 * w, 0.95)]
    return front(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="big", cape=False)


def bt_stretch(sp, i):
    """기지개 (한 번): 두 팔 쭉 위로 → 하품하며 팔 벌림 → 서기."""
    if i == 0:
        return front(arms=[("a", 140, 1.0), ("a", 140, 1.0)], eyes="tight", hip=F(-1.5))
    if i == 1:
        return front(arms=[("a", 100, 1.0), ("a", 100, 1.0)], eyes="closed", mouth="big")
    return front(eyes="happy")


def bt_heart(sp, i):
    """하트를 가슴에 꼭 안고 눈 ^^, 작은 하트가 떠오름."""
    s = hs()
    hy = torso_top() + s["th"] * 0.45

    def held():
        """가슴에 안은 하트."""
        return dp.held_heart(hy - 4)
    return front(arms=[(58, hy + 3), (70, hy + 3)], arm_bend=[(-1, 0.6), (1, 0.6)], held=[held], eyes="happy", fx=dp.heart_burst(i))


def bt_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 배를 감싸고 깔깔 (> <)."""
    s = hs()
    y = torso_top() + s["th"] * 0.6
    return front(rot=[-7, 7, -3][i], arms=[(60, y), (68, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="tight", mouth="big")


def apple_bubble_fx(cx, cy, r=12.0):
    """🍎 사과 생각 풍선 (배고플 때)."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    red, leaf = (226, 64, 70), (96, 176, 84)
    fx += [([C(cx - 2.6, cy + 1.5, 5.2), C(cx + 2.6, cy + 1.5, 5.2)], red, 0), ([L([(cx, cy - 3.5), (cx + 0.8, cy - 7.5)], 1.4)], (110, 70, 50), 0),
           ([E(cx + 1, cy - 8, cx + 6, cy - 5)], leaf, 0), ([C(cx - 3.6, cy - 0.4, 1.4)], dp.WHITE, 0)]
    return fx


def bt_hungry(sp, i):
    """배고파 시무룩: 두 손으로 배를 감싸고 처진 눈꺼풀, 사과 생각 풍선."""
    s = hs()
    y = torso_top() + s["th"] * 0.68
    return front(hip=F([0, 1, 0][i]), arms=[(59, y), (69, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="dot", brow="sad", mouth="frown",
                 head_rot=-3, fx=apple_bubble_fx(104 - 2 * i, 20, 12.0))


EAT_MOUTH = 79.0  # 먹을 때 입 (프레임 y) — 그릇 밥 더미 바로 위


def bt_eat(sp, i):
    """냠냠 (그릇 없음): 무릎 꿇고 고개를 숙여 드러난 입을 밥 더미 바로 위에 — 앙 ↔ 오물오물, 부스러기."""
    s = hs()
    # 입 = 머리 중심 + 0.66 hry, 머리 중심 = 몸 위 - hry + OVERLAP, 몸 위 = 엉덩이 + 0.35 lw - th
    hip_y = EAT_MOUTH + [0, 1, 0, 1][i] - 0.66 * s["hry"] + s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]
    hip_y = min(hip_y, FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - 3, foot_y()), (64 + s["hip"] + 3, foot_y())]
    arms = [(44, 96), (84, 96)]
    return front(hip=(64.0, hip_y), legs=legs, arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="happy" if i % 2 else "dot",
                 mouth="big" if i % 2 == 0 else "chew", fx=dp.eat_crumbs(i), cape=dict(sway=0))


def bt_level_up(sp, i):
    """만세: 두 팔 번쩍, ^^, 반짝이 (가운데 칸은 살짝 뜀)."""
    up = [114, 124, 118][i]
    return front(hip=F(-[0, 3, 1][i]), legs=None if i != 1 else [("a", -6, 0.97), ("a", 6, 0.97)], arms=[("a", up, 1.0), ("a", up, 1.0)],
                 eyes="happy", mouth="big", fx=dp.level_up_stars(i))


# =====================================================================
# 구르기 (회전 원 안) — 망토에 싸인 공
# =====================================================================
def bt_roll(sp, i):
    """망토로 몸을 감싸고 무릎을 끌어안은 동그란 공: 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            hrx, hry = 21.0, 19.5
            hx, hy = cx + 5, cy - 8
            body = Part([C(cx - 4, cy + 7, 21.0)], CAPE, [([T(*scallop_hem((cx + 16, cy + 22), (cx - 24, cy + 22), 4, 4.5, (0, -1)),
                                                              (cx - 26, cy + 34), (cx + 18, cy + 34))], CAPE_IN)], outline=True, grow=0)
            knee = Part([E(cx + 2, cy + 6, cx + 28, cy + 24)], SUIT, [([E(cx + 2, cy + 6, cx + 12, cy + 18)], BELT)], outline=True, grow=0)
            boot = Part([E(cx - 2, cy + 18, cx + 16, cy + 29)], BLACK, outline=True, grow=0)
            glove = Part([C(cx + 22, cy + 15, 5.6)], BLACK, outline=True, grow=0)
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry), T((hx - 14, hy - 12), (hx - 4, hy - 16), (hx - 11, hy - 26)),
                         T((hx + 2, hy - 17), (hx + 12, hy - 13), (hx + 8, hy - 27))], SKIN,
                        [([T((hx + 30, hy + 4), (hx + 12, hy + 6), (hx + 2, hy + 3), (hx - 8, hy + 6), (hx - 30, hy + 10), (hx - 30, hy - 40),
                             (hx + 30, hy - 40))], BLACK)], outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.6)
            face = []
            for side, lx in ((-1, hx - 5), (1, hx + 10)):
                face += eye_ops("happy", lx, hy - 2, side, sx=0.8)
            face += mouth_ops("open" if i else "smile", hx + 4, hy + 12)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([body, knee, boot, head, glove], face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면 벽에 손발을 붙이고 엉금엉금, 망토가 등을 덮음 (머리는 언제나 팔 위)
# =====================================================================
CLIMB = [(1.0, 0.2), (0.6, 0.6), (0.2, 1.0), (0.6, 0.6)]  # (왼손, 오른손) 올린 몫 — 발은 엇갈려


def bat_back(reach_, *, dy=0.0, scratch=False, sweat=False, sway=0.0):
    """뒷모습: 두 손을 머리 옆·위로 벽에 짚고 (reach: 0 어깨 옆 … 1 머리 위), 두 다리는 무릎을 벌려 벽에 디딤. 망토가 등을 덮음."""
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


def bt_climb(sp, i):
    """뒷모습으로 기어오름: 왼손·오른손이 번갈아 위로, 발은 엇갈려 디딤, 몸 살짝 오르내림."""
    return bat_back(CLIMB[i], dy=[0, -1, 0, -1][i], sway=[-1, 0, 1, 0][i])


def bt_slide(sp, i):
    """뒷모습으로 주르륵: 두 손 쭉 위로, 긁힌 자국 3줄, 땀 한 방울."""
    return bat_back((1.0, 1.0), dy=[0, 2][i], scratch=True, sweat=True)


# =====================================================================
# 💪 물건 들기 (흐물이 juggle 틀) — 두 손이 머리 위로 물건 바닥을 받침 (팔은 머리 뒤로 가려짐), 손 자리 mark_grip
# =====================================================================
HAND_SPREAD = 3.0  # 머리 위 물건을 받칠 때 두 손이 머리 옆면보다 이만큼 바깥 (팔 길이가 짧은 꼬마라 손바닥을 머리 양옆 위쪽에 펴 듦)
STRAIN_HANDS_X = 100  # 손바닥이 거대한 것 옆면에 (코어 juggle 거대한 것 앞 간격 몸 크기 0.28)


def overhead(hip, rot=0.0, dx=2.0, dy=0.0):
    """머리 위 물건 바닥 자리 (두 귀 끝 바로 위)."""
    s = hs()
    return (hip[0] + 2.0 + dx, head_top_y(hip) - s["ear"] * 0.45 + dy)


def hold_up(g, side=True):
    """머리 위에 인 물건(바닥 g)을 받치는 두 손 [먼, 가까운]: 머리 양옆 위쪽에서 손바닥을 펴 듦 (팔은 머리 뒤로 가려짐)."""
    s = hs()
    w, y = s["hrx"] + HAND_SPREAD, g[1] + s["ear"] * 0.45 + s["hry"] * 0.5
    return [(g[0] - w * (0.92 if side else 1.0), y), (g[0] + w * (1.0 if side else 1.0), y)]


def bt_lift(sp, i):
    """쪼그려 앞에 놓인 물건을 두 손으로 잡고 → 가슴께로 → 머리 위로 번쩍 (한 번, 마지막 칸 = hold)."""
    s = hs()
    if i == 0:
        hip = H(SIDE_CX - 2, s["leg"] * 0.4)
        g = (88.0, 119.0)
        hands = [reach(sh, (g[0] - 3, g[1] - 2), s["arm"], 1.0) for sh in hero_shoulders("side", hip, 30)]
        dp.mark_grip(*g)
        return hero(hip=hip, rot=30, head_rot=-18, legs=[floor_at(SIDE_CX - 8), floor_at(SIDE_CX + 8)], arms=hands, arm_bend=[(0, 1), (0, 1)],
                    eyes="dot", brow="angry", mouth="grin", cape=dict(a=34))
    if i == 1:
        hip = H(SIDE_CX, s["leg"] * 0.15)
        sh = hero_shoulders("side", hip, 6)[1]
        g = (sh[0] + s["arm"] * 0.85, sh[1] - 2)
        dp.mark_grip(*g)
        return hero(hip=hip, rot=6, legs=[floor_at(SIDE_CX - 7), floor_at(SIDE_CX + 7)], arms=[(g[0] - 3, g[1] + 1), (g[0] + 1, g[1] + 1)],
                    arm_bend=[(0, 1), (0, 1)], eyes="dot", brow="angry", mouth="grin", cape=dict(a=22))
    hip = H(SIDE_CX, [-1.0, 0.0][i - 2])
    g = overhead(hip, dy=[-2, 0][i - 2])
    dp.mark_grip(*g)
    return hero(hip=hip, legs=[floor_at(SIDE_CX - 7), floor_at(SIDE_CX + 7)], arms=hold_up(g), arms_behind=True, arm_bend=[(-1, 0), (-1, 0)],
                eyes="happy" if i == 3 else "dot", mouth="open", open_hand=(True, True), cape=dict(a=12))


def bt_hold(sp, i):
    """머리 위에 든 채 버티고 섬: 숨 쉬듯 살짝 내려앉음, 여유로운 미소."""
    hip = H(SIDE_CX, [0.0, 0.8][i])
    g = overhead(hip)
    dp.mark_grip(*g)
    return hero(hip=hip, legs=[floor_at(SIDE_CX - 8), floor_at(SIDE_CX + 8)], arms=hold_up(g), arms_behind=True, arm_bend=[(-1, 0), (-1, 0)],
                eyes="dot", mouth="smile", open_hand=(True, True), cape=dict(a=10, flap=[0, 1][i]))


def bt_carry(sp, i):
    """머리 위에 인 채 성큼성큼 걷기: 다리는 걷기처럼, 두 손은 물건을 받친 채 통통, 망토 살랑."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    g = overhead(hip)
    dp.mark_grip(*g)
    return hero(hip=hip, legs=legs, arms=hold_up(g), arms_behind=True, arm_bend=[(-1, 0), (-1, 0)], eyes="dot", mouth="grin",
                open_hand=(True, True), cape=dict(a=[16, 20, 16, 20][i], flap=[0, 1.5, 0, -1.5][i]))


def bt_throw(sp, i):
    """젖혔다가 휙: 0 = 든 채 · 1 = 몸을 뒤로 젖혀 물건을 머리 뒤로 · 2 = 앞으로 확 내던지며 놓음 · 3 = 팔을 앞으로 뻗은 채 마무리 (한 번)."""
    s = hs()
    legs = [floor_at(SIDE_CX - 10), floor_at(SIDE_CX + 10)]
    hip = H(SIDE_CX, [0, 1.5, 2.5, 2.0][i])
    if i == 0:
        g = overhead(hip)
        dp.mark_grip(*g)
        return hero(hip=hip, legs=legs, arms=hold_up(g), arms_behind=True, arm_bend=[(-1, 0), (-1, 0)], open_hand=(True, True), mouth="grin",
                    cape=dict(a=10))
    if i == 1:
        rot = -12
        ht = head_top_y(hip)
        g = (hip[0] - s["hrx"] * 0.9, ht + 2)
        dp.mark_grip(*g)
        return hero(hip=hip, rot=rot, head_rot=-4, legs=legs, arms=[(g[0] - 4, g[1] + 2), (g[0] + 4, g[1] + 2)], arms_behind=True,
                    arm_bend=[(-1, 0), (-1, 0)], open_hand=(True, True), eyes="tight", mouth="grin", cape=dict(a=4))
    rot = [0, 0, 16, 12][i]
    sh = hero_shoulders("side", hip, rot)
    g = (sh[1][0] + s["arm"] * 0.92, sh[1][1] + [0, 0, 3, 8][i])
    fx = dp.speed_fx(4, (50, 62, 74), 12) if i == 2 else []
    dp.mark_grip(*g)
    return hero(hip=hip, rot=rot, head_rot=-rot * 0.4, legs=legs, arms=[(g[0] - 2, g[1] + 2), (g[0] + 1, g[1] + 1)], arm_bend=[(0, 1), (0, 1)],
                open_hand=(True, True), eyes="dot", brow="angry", mouth="big" if i == 2 else "grin", cape=dict(a=[0, 0, 40, 30][i], flap=2),
                fx=fx)


def bt_strain(sp, i):
    """흡! 거대한 것 앞에서 다리를 벌리고 버티며 두 손바닥으로 밀어 올림 — 영웅이라 금방 (살짝 덜덜, 자신만만)."""
    s = hs()
    sh_ = [0.0, 1.0, -0.6, 0.8][i]
    hip = (SIDE_CX + 10 + sh_ * 0.4, stand_hip_y() + s["leg"] * 0.3 + [0, 0.6, 0.2, 0.5][i])
    rot = 24
    shs = hero_shoulders("side", hip, rot)
    hands = [reach(shs[0], (STRAIN_HANDS_X + sh_, shs[0][1] - 4), s["arm"], 1.0), reach(shs[1], (STRAIN_HANDS_X + sh_, shs[1][1] + 6), s["arm"], 1.0)]
    legs = [floor_at(hip[0] - s["leg"] * 0.9 - 4), floor_at(hip[0] + 6)]
    return hero(hip=hip, rot=rot, head_rot=-20, legs=legs, leg_bend=[(1, -0.3), (1, -1)], arms=hands, arm_bend=[(0, 1), (0, 1)],
                open_hand=(True, True), eyes="dot", brow="angry", mouth="grin", cape=dict(a=38, flap=[1, -1, 1, -1][i]),
                fx=sweat_fx(30 + [0, 1, 0, 1][i], [30, 28, 31, 29][i]) if i % 2 else [])


def bt_present(sp, i):
    """짠! 배달 도착 (앞모습): 두 손으로 물건을 머리 위로 번쩍 든 채 섰다가 (흔들) → 마지막 칸에 휙 떨굼 — 두 팔이 앞아래로 (한 번)."""
    s = hs()
    if i < 3:
        hip = F([0, -1, 0][i])
        g = (64.0 + [0, 1.5, -1.5][i], head_top_y(hip) - s["ear"] * 0.45)
        dp.mark_grip(*g)
        return front(hip=hip, arms=hold_up(g, side=False), arm_bend=[(-1, 0), (1, 0)], open_hand=(True, True), eyes="happy" if i == 1 else "dot",
                     mouth="big", cape=dict(sway=[0, 0.8, -0.8][i]))
    hip = F(0.5)
    g = (64.0, head_top_y(hip) - s["ear"] * 0.45)
    dp.mark_grip(*g)
    return front(hip=hip, arms=[("a", 40, 1.0), ("a", 40, 1.0)], arm_bend=[(-1, 0), (1, 0)], open_hand=(True, True), eyes="happy", mouth="open")


def bt_slump(sp, i):
    """털썩 (너무 무거워): 바닥에 주저앉아 다리를 앞으로 쭉, 몸은 뒤로 기대 고개 푹, 팔은 바닥에 — 땀방울, 숨 고르기."""
    s = hs()
    hip = sit_hip(SIDE_CX - 6)
    legs = [(hip[0] + s["leg"] + 6, foot_y()), (hip[0] + s["leg"] + 9, foot_y())]
    arms = [(hip[0] - 10, FLOOR - s["aw"] * 0.6), (hip[0] - 6, FLOOR - s["aw"] * 0.6)]
    return hero(hip=hip, rot=-14, head_rot=[14, 18][i], legs=legs, arms=reach_all(hip, -14, arms), arm_bend=[(1, 0), (1, 0)],
                eyes="closed", mouth="open", fx=sweat_fx(98, [32, 34][i]), cape=dict(a=-20, len=0.7))


def reach_all(hip, rot, ends):
    """옆모습 두 팔 끝을 팔 길이 안으로."""
    return [reach(sh, e, hs()["arm"], 1.0) for sh, e in zip(hero_shoulders("side", hip, rot), ends)]


# =====================================================================
# 🏍️ 오토바이 (옆모습, 오른쪽으로 달림) — 아주 큰 앞뒤 바퀴, 낮은 차체
# =====================================================================
def bike_geom():
    """지금 단계 오토바이 자리: (k, R, 뒷바퀴 가운데, 앞바퀴 가운데, 안장, 발판, 포크 위)."""
    k = hs()["bike"]
    R = 21.0 * k
    rear, front_ = (64 - 34 * k, FLOOR - R), (64 + 34 * k, FLOOR - R)
    seat = (64 - 8 * k, FLOOR - 2 * R + 10 * k)
    peg = (64 + 4 * k, FLOOR - R + 1)
    fork = (front_[0] - 9 * k, FLOOR - 2 * R - 2 * k)
    return k, R, rear, front_, seat, peg, fork


def wheel(c, R, spin):
    """굵은 타이어 바퀴 하나 (spin: 굴러간 각도 — 바퀴살·타이어 홈이 함께 돎)."""
    marks = []
    for j in range(6):  # 타이어 홈 6개 (60°마다)
        a = math.radians(spin + 60 * j)
        p0 = (c[0] + math.sin(a) * R * 0.84, c[1] - math.cos(a) * R * 0.84)
        p1 = (c[0] + math.sin(a) * R * 1.05, c[1] - math.cos(a) * R * 1.05)
        marks.append(([L([p0, p1], 2.4)], TREAD))
    marks.append(([C(c[0], c[1], R * 0.62)], RIM))
    marks.append(([C(c[0], c[1], R * 0.5)], BIKE))
    for j in range(3):  # 바퀴살 6개 (지름 셋)
        a = math.radians(spin + 60 * j + 30)
        d = (math.sin(a) * R * 0.52, -math.cos(a) * R * 0.52)
        marks.append(([L([(c[0] - d[0], c[1] - d[1]), (c[0] + d[0], c[1] + d[1])], 1.8)], RIM))
    marks.append(([C(c[0], c[1], R * 0.2)], HUB))
    marks.append(([C(c[0], c[1], R * 0.08)], BIKE))
    return Part([C(c[0], c[1], R)], TYRE, marks, outline=True, grow=0)


def bike_parts(spin, bar, *, lamp=True, flame=0):
    """오토바이 Part 함수들 (뒤 → 앞): 뒷바퀴 · 앞바퀴 · 앞 포크 · 낮은 차체 (안장·노란 박쥐 무늬·전조등·배기관) · 핸들 (bar = 손잡이 자리).
    flame: 배기 불꽃 크기 (0이면 없음)."""
    k, R, rear, front_, seat, peg, fork = bike_geom()

    def rear_w():
        """뒷바퀴."""
        return wheel(rear, R, spin)

    def front_w():
        """앞바퀴."""
        return wheel(front_, R, spin + 17)

    def fork_p():
        """앞 포크 (앞바퀴 가운데 → 포크 위)."""
        return Part([L([front_, fork], 4.2 * k)], RIM, outline=True, grow=0)

    def body():
        """낮은 차체: 뒷바퀴 흙받이 → 안장 → 연료통 → 앞 덮개·전조등 → 배 아래."""
        ry = FLOOR - 2 * R
        pts = [(rear[0] - R * 0.95, rear[1] - R * 0.25), (rear[0] - R * 0.6, ry + 4), (seat[0] - 6 * k, seat[1] - 1), (seat[0] + 8 * k, seat[1]),
               (64 + 14 * k, ry - 5 * k), (fork[0] + 2 * k, ry - 3 * k), (front_[0] + R * 0.5, ry + 4 * k), (front_[0] + R * 0.25, front_[1] - R * 0.45),
               (front_[0] - R * 0.7, front_[1] - R * 0.2), (64 + 6 * k, FLOOR - R + 5 * k), (rear[0] + R * 0.7, rear[1] + 2 * k),
               (rear[0] + R * 0.35, rear[1] - R * 0.55)]
        marks = [([L([(seat[0] - 7 * k, seat[1] + 0.5), (seat[0] + 7 * k, seat[1] + 1)], 4.0 * k)], SEAT),
                 ([L([(rear[0] - R * 0.55, ry + 4), (64 + 12 * k, ry + 1 * k), (front_[0] + R * 0.1, ry + 2 * k)], 1.6)], BIKE_HI),
                 ([T(*bat_shape(64 + 10 * k, FLOOR - R - 4 * k, 6.5 * k))], BELT)]
        if lamp:
            lx, ly = front_[0] + R * 0.38, ry + 6 * k
            marks.append(([E(lx - 3.4 * k, ly - 2.6 * k, lx + 3.4 * k, ly + 2.6 * k)], LAMP))
        return Part([T(*pts)], BIKE, marks, outline=True, grow=0)

    def pipe():
        """배기관 (안장 아래 → 뒤로)."""
        return Part([L([(64 - 4 * k, FLOOR - R + 3 * k), (rear[0] - R * 0.2, FLOOR - R - 1 * k)], 3.6 * k)], RIM, outline=True, grow=0)

    def handle():
        """핸들: 포크 위에서 뒤로 뻗은 손잡이."""
        return Part([L([fork, bar], 3.4 * k), C(fork[0], fork[1], 2.4 * k)], BIKE_HI, outline=True, grow=0)
    parts = [rear_w, front_w, fork_p, pipe, body, handle]
    return parts


def flame_fx(n, k):
    """배기관 끝 불꽃 (n: 크기 몫)."""
    _, R, rear, _, _, _, _ = bike_geom()
    x, y = rear[0] - R * 0.2 - 2, FLOOR - R - 1 * k
    if n <= 0:
        return []
    return [([T((x, y - 2.4 * k), (x - 7 * n * k, y - 0.5), (x, y + 2.4 * k))], FLAME, 1.0), ([T((x, y - 1.2 * k), (x - 4 * n * k, y), (x, y + 1.2 * k))], LAMP, 0)]


def rider(spin, *, rot=14.0, hip_dy=0.0, stand=0.0, eyes="dot", brow="angry", mouth="grin", head_rot=-6.0, cape=None, whole=None, fx=(),
          near_foot=None, lamp=True, flame=0, look=(1.0, 0.0)):
    """오토바이에 앉은 박쥐인간 (옆모습): 안장에 엉덩이, 발은 발판, 두 손은 핸들 (손이 닿는 자리에 핸들을 맞춤).
    stand: 발판 위로 일어선 몫 (점프) · near_foot: 가까운 발 자리를 바꿈 (드리프트 — 바닥 짚기)."""
    s = hs()
    k, R, rear, front_, seat, peg, fork = bike_geom()
    hip = (seat[0] + 1, seat[1] - s["lw"] * 0.45 + hip_dy - stand * s["leg"] * 0.5)
    shs = hero_shoulders("side", hip, rot)
    bar = polar(shs[1], 58, s["arm"] * 0.9)
    legs = [reach((hip[0] - 2, hip[1]), (peg[0] - 2, peg[1]), s["leg"]), reach((hip[0] + 2, hip[1]), near_foot or peg, s["leg"])]
    arms = [(bar[0] - 2.0, bar[1] + 1.0), bar]
    return hero(hip=hip, rot=rot, head_rot=head_rot, legs=legs, leg_bend=[(1, -1), (1, -1)], arms=arms, arm_bend=[(0, 1), (0, 1)],
                eyes=eyes, brow=brow, mouth=mouth, look=look, cape=cape or dict(a=80), whole=whole, fx=fx,
                vehicle=bike_parts(spin, bar, lamp=lamp, flame=flame))


SPIN = 20.0  # 칸마다 굴러가는 각도 (6개 홈·바퀴살 = 60°마다 같은 모양 → 3칸이면 한 바퀴 무늬)


def bt_ride(sp, i):
    """🏍️ 부릉부릉 달리기 (3칸 반복): 바퀴가 굴러가고, 몸을 앞으로 숙여 핸들을 잡고, 망토가 뒤로 휘날림, 배기 불꽃·속도선."""
    k = hs()["bike"]
    fx = dp.speed_fx(2, (60, 74), 8, [0, -2, -1][i]) + flame_fx([1.0, 0.6, 0.85][i], k)
    return rider(SPIN * i, cape=dict(a=[80, 84, 78][i], flap=[1.5, -1.5, 0][i]), hip_dy=[0, -0.6, 0][i], fx=fx)


def bt_wheelie(sp, i):
    """🏍️ 윌리 (3칸 반복): 앞바퀴를 번쩍 들고 뒷바퀴로만 — 몸을 뒤로 젖혀 버티고 "yeah!" 신난 얼굴, 뒷바퀴 먼지."""
    k, R, rear, front_, seat, peg, fork = bike_geom()
    fx = dust_fx(rear[0] - R * 0.9, FLOOR - 1, 0.8 + 0.2 * (i % 2))
    return rider(SPIN * i, rot=10, head_rot=-8, eyes="happy", brow=None, mouth="big", cape=dict(a=[70, 74, 68][i], flap=[1.5, -1.5, 0][i], len=0.75),
                 whole=dict(rot=-[18, 20, 19][i], pivot=(rear[0], FLOOR), dx=8), fx=fx, flame=0.8)


def bt_bike_jump(sp, i):
    """🏍️ 붕! 오토바이째 점프 (2칸): 발판 위로 반쯤 일어서서, 앞이 들렸다가 → 수평으로, 망토가 위로 펄럭."""
    fx = dp.speed_fx(2, (90, 104), 8, -1)
    return rider(SPIN * i * 0.5, rot=[6, 10][i], stand=0.35, eyes=["round", "happy"][i], brow=None, mouth="big", head_rot=-8,
                 cape=dict(a=[110, 96][i], flap=[2, -2][i], len=0.75), whole=dict(rot=[-10, 4][i], pivot=(64, FLOOR), dx=[4, -2][i], dy=-6), fx=fx)


def bt_drift(sp, i):
    """🏍️ 끼익 드리프트 급정거 (2칸): 몸을 뒤로 젖히고 가까운 발로 앞 바닥을 짚음, 뒷바퀴 뒤로 먼지 구름·바닥 미끄럼 자국."""
    k, R, rear, front_, seat, peg, fork = bike_geom()
    s = hs()
    skid = [([L([(rear[0] - R * 1.1 - 18 * k, FLOOR - 0.8), (rear[0] - R * 0.2, FLOOR - 0.8)], 1.6)], SKID, 0)]
    dust = dust_fx(rear[0] - R * 0.6, FLOOR - 2, 1.0 + 0.25 * i) + dust_fx(rear[0] - R * 1.2, FLOOR - 8 - 3 * i, 0.7)
    foot = (peg[0] + s["leg"] * 0.6 + 4, foot_y())
    return rider(0, rot=-6, head_rot=-2, eyes="tight" if i == 0 else "dot", brow=None if i == 0 else "angry", mouth="grin",
                 cape=dict(a=[40, 30][i], flap=[2, -2][i]), whole=dict(rot=[-3, -2][i], pivot=(rear[0], FLOOR)), fx=skid + dust, near_foot=foot)


# ---- 🔦 박쥐 신호 조명 ----
SIGNAL_LENS = (96.0, 84.0)  # 조명 렌즈 가운데 (프레임 px) — 코어 bike::SIGNAL_FROM과 같은 자리


def spotlight(lit):
    """바닥에 선 큰 신호 조명: 받침 + 위를 향한 둥근 통 + 노란 렌즈 (lit: 켜짐)."""
    lx, ly = SIGNAL_LENS

    def part():
        """조명."""
        base = T((lx - 11, FLOOR), (lx + 11, FLOOR), (lx + 6, FLOOR - 9), (lx - 6, FLOOR - 9))
        leg = L([(lx, FLOOR - 8), (lx, ly + 18)], 4.0)
        drum = T((lx - 8, ly + 20), (lx + 8, ly + 20), (lx + 10, ly + 1), (lx - 10, ly + 1))
        rim = E(lx - 11, ly - 3, lx + 11, ly + 3.5)
        marks = [([L([(lx - 9, ly + 10), (lx + 9, ly + 10)], 1.6)], BIKE_HI), ([E(lx - 9, ly - 2, lx + 9, ly + 2.5)], LAMP if lit else (150, 150, 120)),
                 ([T(*bat_shape(lx, ly + 14, 5.0))], BELT)]
        return Part([base, leg, drum, rim], SPOT, marks, outline=True, grow=0)
    return part


def bt_signal(sp, i):
    """🔦 박쥐 신호 (한 번, 마지막 칸 유지): 0 = 조명 옆에 서서 손잡이를 잡고 위로 겨눔 · 1 = 탁 켜고 다른 주먹을 번쩍 ·
    2 = 켠 채 주먹 들고 고개 들어 하늘을 봄 (빛줄기·박쥐 무늬는 앱의 덮개 창이 그림)."""
    s = hs()
    hip = H(SIDE_CX - 8)
    lit = i > 0
    lx, ly = SIGNAL_LENS
    near = reach(hero_shoulders("side", hip, 0)[1], (lx - 9, ly + 12), s["arm"], 1.0)
    far = ("a", 70, 0.85) if i == 0 else ("a", 165, 0.98)
    glow = [([C(lx, ly - 2, 9 + 2 * i)], (255, 246, 196), 2.0)] if lit else []
    return hero(hip=hip, legs=[floor_at(hip[0] - 7), floor_at(hip[0] + 7)], arms=[far, near], arm_bend=[(1, 0), (0, 1)], 
                head_rot=[0, -10, -18][i], eyes=["dot", "dot", "up"][i], brow="angry" if i < 2 else None, mouth=["smile", "grin", "open"][i],
                look=(1.0, -0.6 * i), cape=dict(a=[10, 22, 26][i], flap=[0, 2, -2][i]), held=[spotlight(lit)], fx=glow)


BAT_POSES = {"walk": bt_walk, "run": bt_run, "roll": bt_roll, "idle": bt_idle, "look": bt_look, "sleep": bt_sleep, "hop": bt_hop,
             "fall": bt_fall, "dragged": bt_dragged, "land": bt_land, "sit_front": bt_sit_front, "wave": bt_wave, "tilt": bt_tilt,
             "belly": bt_belly, "stretch": bt_stretch, "heart": bt_heart, "tickle": bt_tickle, "annoyed": bt_annoyed,
             "hungry": bt_hungry, "eat": bt_eat, "level_up": bt_level_up, "climb": bt_climb, "slide": bt_slide, "lookup": bt_lookup,
             "lift": bt_lift, "hold": bt_hold, "carry": bt_carry, "throw": bt_throw, "strain": bt_strain, "present": bt_present, "slump": bt_slump,
             "ride": bt_ride, "wheelie": bt_wheelie, "bike_jump": bt_bike_jump, "drift": bt_drift, "signal": bt_signal}
BAT_POSES = {k: (v if k == "roll" else dp._fit(v)) for k, v in BAT_POSES.items()}  # roll은 회전 원 안 그대로
JUGGLE_ANIMS = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]  # = Rust manifest::JUGGLE_ANIMS
BIKE_ANIMS = ["ride", "wheelie", "bike_jump", "drift", "signal"]  # = Rust manifest::BIKE_ANIMS

BAT_HERO = Species(
    id="bat", name="박쥐인간", walk_speed=95, scale=1.0, base=SUIT, back=CAPE, belly=SUIT, nose=OUT, mouth="w",
    front_tail_color=CAPE, tail_color=CAPE, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + JUGGLE_ANIMS + BIKE_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=BELT, poses=BAT_POSES,
    outline=OUT,
    # 색은 파스텔 없이 또렷한 회색·검정·노랑 · 크기는 BAT가 정함 (fit 1)
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 박쥐인간을 등록함."""
    dp.SPECIES[BAT_HERO.id] = BAT_HERO


register()


def preview_bat(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hop", 1), ("fall", 0), ("roll", 0),
             ("ride", 0), ("wheelie", 1), ("bike_jump", 0), ("drift", 1), ("signal", 2), ("carry", 1), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "ride"), bg=bg)


def preview_all(by_stage, path, st="teenager", only=None, bg=(255, 255, 255)):
    """한 단계의 모든 동작·칸 (눈으로 확인용) — only: 이 동작들만."""
    from PIL import Image
    anims = {a: f for a, f in by_stage[st].items() if only is None or a in only}
    im = Image.new("RGBA", (4 * 132, len(anims) * 132), bg + (255,))
    for r, (a, frames) in enumerate(anims.items()):
        for c, f in enumerate(frames):
            im.alpha_composite(f, (c * 132, r * 132))
    im.save(path)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(BAT_HERO, root / "ui" / "pets" / BAT_HERO.id))
    preview_bat(by_stage, pv / "stages_bat.png")
    dp.preview_back(by_stage, pv / "climb_bat.png")
    if "--all" in sys.argv:
        for st in STAGES:
            preview_all(by_stage, pv / f"all_bat_{st}.png", st)
    print("ok: bat")


if __name__ == "__main__":
    main()
