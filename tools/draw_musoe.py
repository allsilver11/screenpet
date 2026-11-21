"""무쇠인간 🛡️ — 빨간 갑옷에 금색 판을 덧댄 사람 모양 꼬마 영웅 펫(musoe, 우리만의 캐릭터). 뼈대·IK는 슈퍼인간(draw_super)·
거미인간(draw_spider)과 같은 틀이고, 부품마다 자기 외곽선을 두른 두 발 꼬마 — 팔다리는 관절을 IK로 굽힌다.

    python tools/draw_musoe.py → ui/pets/musoe/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_musoe,climb_musoe,moves_musoe}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 큰 둥근 통 투구 (빨강, 정수리에서 이마까지 짧은 금색 줄, 작은 안테나 끝에 금색 구슬) — 위로 치켜 올라간 렌즈 눈 둘
        (금테 + 하늘색 유리 + 하얀 반짝), 입 자리엔 작은 숨구멍 점 한 줄. 몸은 빨간 갑옷 (어깨 넓고 허리 좁음): 둥근 금색 어깨 받침,
        윗팔·허벅지 금색 판, 금색 무릎 관절, 금색 허리판, 각진 패널 선 (가슴 V·정강이 줄), 하얀 금속 반사 줄.
        가슴엔 금테 하트 창이 하얗게 빛나고 짧은 빛줄기 (기분 좋으면 더 밝게, 충전 중엔 깜빡). 등엔 작은 금색 날개 둘·분사구 둘의 제트팩.
        만들지 않는 것 (다른 영웅을 닮지 않게): 손바닥 빛 쏘기·발바닥 분사·내려 닫는 얼굴 가리개·동그란/세모 가슴 빛·가는 틈 눈.
단계 (DIMS): 아기 = 동글 꼬마 (투구 아주 큼, 팔다리 뭉툭) → 유아 → 어린이 → 청소년 → 어른 키 크고 어깨 넓음.
특기 동작: fly·fly_fast 🚀 제트팩 날기 (몸을 눕혀 등 뒤 분사구에서 하얀 빛 꼬리가 뒤로 길게 — 코어 brain_soar.rs, 슈퍼인간과 같은 뇌) ·
      fly_flip 공중제비 · hover 똑바로 떠서 둥실 (분사구 불꽃은 등 뒤 아래) · suit_up 🪖 투구 장착 (맨얼굴 갈색 머리 꼬마 → 빨간 반쪽 투구
      둘이 양옆에서 캡슐처럼 미끄러져 닫힘 → 렌즈 켜짐) · stretch 💗 하트 번쩍 · sit_front 🔋 앉아서 충전 (하트 깜빡, 안테나 불똥) ·
      tickle·annoyed 🛡️ 팔뚝 엇갈려 막기 · wave 👋 삐삐 손 흔들기 · hungry 🔋 배터리 생각 풍선.
      벽 타기 climb·slide = 뒷모습 (머리가 언제나 팔 위 — 팔이 머리를 덮지 않음, 등에 제트팩).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, _ST, anger_fx, question_fx, sparkle_fx, sweat_fx, view, xf, z_fx  # noqa: E402
from draw_spider import ik, target  # noqa: E402
from draw_super import eye_ops, mouth_ops, reach  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FLOOR = 125.0  # 바닥 (장화 바닥)
RED = (206, 40, 42)  # 빨간 갑옷·투구
RED_FAR = (168, 30, 38)  # 먼 쪽 팔다리
PANEL = (150, 22, 30)  # 각진 패널 선·숨구멍 점
SHINE = (255, 240, 234)  # 하얀 금속 반사 줄
GOLD = (238, 188, 62)  # 금색 판 (어깨·윗팔·허벅지·무릎·허리·투구 줄·안테나 구슬)
GOLD_FAR = (204, 152, 46)
LENS = (176, 236, 255)  # 하늘색 렌즈 유리
LENS_OFF = (70, 92, 120)  # 꺼진 렌즈 (잠·눈 깜빡·막 닫힌 투구)
WHITE = (255, 255, 255)  # 렌즈 반짝·하트 빛
HEART_DIM = (250, 176, 186)  # 충전 중 꺼진 하트
RAY = (255, 248, 222)  # 하트 둘레 짧은 빛줄기
FIST = (172, 30, 36)  # 장갑 주먹
BOOT = (120, 20, 32)  # 장화
PACK = (122, 130, 150)  # 강철 제트팩
PACK_D = (84, 90, 108)  # 분사구·이음새
STICK = (96, 100, 116)  # 안테나 막대
TRAIL = (250, 250, 238)  # 분사구 하얀 빛 꼬리 (속 — 따뜻한 흰빛, 렌즈 반짝과 다름)
TRAIL_EDGE = (222, 230, 244)  # 빛 꼬리 가장자리 (옅은 은빛)
SPARK = (255, 220, 80)  # 안테나 불똥·충전 번개
SKIN, SKIN_SHADE = (255, 222, 198), (236, 188, 164)  # 투구 벗은 맨얼굴 (suit_up)
HAIR = (122, 78, 46)  # 갈색 머리
BLUSH = (250, 156, 156)
OUT = (46, 26, 28)  # 외곽선
DUST, DUST_LINE = (236, 228, 214), (176, 160, 150)
LOW = (236, 84, 76)  # 배터리 생각 풍선의 빨간 마지막 칸

# 단계(lvl)별 (DIMS) — hrx·hry: 투구 반지름 · tw: 어깨 반폭 · th: 몸 높이 · hip: 다리 뿌리 반간격 · leg: 다리 길이 · lw: 다리 굵기 ·
# arm: 팔 길이 · aw: 팔 굵기 · emb: 가슴 하트 배율 · ant: 안테나 높이
DIMS = {
    0: dict(hrx=26.5, hry=25.0, tw=14.0, th=21.0, hip=6.6, leg=9.5, lw=10.6, arm=13.0, aw=9.2, emb=0.8, ant=5.0),
    1: dict(hrx=26.0, hry=24.2, tw=14.0, th=22.5, hip=6.8, leg=13.5, lw=10.0, arm=15.0, aw=8.7, emb=0.85, ant=5.5),
    2: dict(hrx=25.4, hry=23.4, tw=14.4, th=24.5, hip=7.0, leg=18.0, lw=9.6, arm=17.5, aw=8.3, emb=0.92, ant=6.0),
    3: dict(hrx=24.8, hry=22.8, tw=15.6, th=26.5, hip=7.2, leg=22.0, lw=9.4, arm=20.0, aw=8.0, emb=1.0, ant=6.5),
    4: dict(hrx=24.2, hry=22.2, tw=17.6, th=29.5, hip=8.0, leg=26.0, lw=9.8, arm=23.0, aw=8.3, emb=1.1, ant=7.0),
}
OVERLAP = 5.0  # 투구가 몸 윗부분을 덮는 깊이
HEART_R = 7.0  # 가슴 하트 창 크기 (청소년 — 펫 크기에서도 또렷하게)
HEART_Y = 0.55  # 하트 가운데 높이 (몸 위에서 몸 높이 몫 — 투구 턱 아래로 다 보이게)


def heart_dy():
    """몸 위에서 가슴 하트 가운데까지 (아기는 큰 투구 턱에 가리지 않게 조금 더 아래)."""
    return hs()["th"] * HEART_Y + (1.5 if _ST["lvl"] == 0 else 0.0)


def hs():
    """지금 단계의 꼬마 영웅 치수."""
    return DIMS[_ST["lvl"]]


def foot_y():
    """바닥에 선 발 관절 y (장화 바닥 = FLOOR)."""
    return FLOOR - hs()["lw"] * 0.5


def stand_hip_y():
    """똑바로 섰을 때 엉덩이 관절 y."""
    return foot_y() - hs()["leg"]


def lerp(a, b, t):
    """a → b 사이 t 몫 자리."""
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def unit_normal(a, b):
    """a → b 방향의 왼쪽 수직 단위 벡터."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    d = math.hypot(dx, dy) or 1e-6
    return (-dy / d, dx / d)


# =====================================================================
# 가슴 하트 · 렌즈 눈 · 투구
# =====================================================================
def heart_pts(cx, cy, r, n=36):
    """하트 꼴 점들 (가운데 (cx, cy), 크기 r)."""
    pts = []
    for k in range(n):
        t = k / n * 2 * math.pi
        pts.append((cx + 16 * math.sin(t) ** 3 * r / 16,
                    cy - (13 * math.cos(t) - 5 * math.cos(2 * t) - 2 * math.cos(3 * t) - math.cos(4 * t)) * r / 16))
    return pts


def heart_ops(cx, cy, r, glow="on", sx=1.0):
    """가슴 하트 창 (금테 + 속): on 하얀 빛 + 짧은 빛줄기 · bright 더 큰 하얀 빛 · dim 꺼진 분홍 (충전 중 깜빡). sx: 옆모습은 가로로 좁게."""
    out = []
    with xf(sx=sx, pivot=(cx, cy)):
        if glow in ("on", "bright"):
            rays = []
            for k in range(8):
                g = math.radians(k * 45 + 22)
                rays.append(L([(cx + math.cos(g) * r * 1.15, cy + math.sin(g) * r * 1.05), (cx + math.cos(g) * r * 1.6, cy + math.sin(g) * r * 1.5)], 1.2))
            out.append((rays, RAY))
        pts = heart_pts(cx, cy, r)
        out.append(([T(*pts)], GOLD))
        k = 0.82 if glow == "bright" else 0.76
        out.append(([T(*[(cx + (x - cx) * k, cy + 0.3 + (y - cy) * k) for x, y in pts])], HEART_DIM if glow == "dim" else WHITE))
    return out


def heart_flash_fx(cx, cy, r, ln=1.0):
    """💗 하트 번쩍: 가슴 밖까지 뻗는 하얀 빛줄기 여덟 + 반짝이 둘 (몸 위에 그림)."""
    fx = []
    for k in range(8):
        g = math.radians(k * 45 + 22)
        a = (cx + math.cos(g) * r * 1.5, cy + math.sin(g) * r * 1.4)
        b = (cx + math.cos(g) * r * (2.2 + 1.4 * ln), cy + math.sin(g) * r * (2.1 + 1.3 * ln))
        fx.append(([L([a, b], 2.0)], WHITE, 0.9))
    return fx + sparkle_fx(cx - r * 3.4, cy - r * 2.0, 3.0) + sparkle_fx(cx + r * 3.4, cy - r * 1.6, 2.4)


def lens_pts(lx, ly, r, s):
    """렌즈 눈 하나의 점들: 바깥쪽 (s = +1 오른쪽, -1 왼쪽)이 더 넓고 위로 치켜 올라간 아몬드 (가는 틈 눈이 아님)."""
    pts = []
    for k in range(24):
        t = k / 24 * 2 * math.pi
        cx_, cy_ = math.cos(t), math.sin(t)
        rx = r * 0.3 * (1 + 0.35 * max(0.0, cx_))
        ry = r * 0.21 * (1 - 0.4 * max(0.0, cx_))
        pts.append((lx + s * cx_ * rx, ly + cy_ * ry - 0.5 * max(0.0, cx_) * r * 0.18))
    return pts


def lens_ops(lx, ly, r, s, lens="on", sx=1.0):
    """렌즈 눈 (금테 + 하늘색 유리 + 하얀 반짝). lens: on · bright (반짝이 큼) · off (꺼진 유리, 반짝 없음)."""
    with xf(sx=sx, pivot=(lx, ly)):
        pts = lens_pts(lx, ly, r, s)
        ops = [([T(*pts)], GOLD), ([T(*[(lx + (x - lx) * 0.74, ly + (y - ly) * 0.74) for x, y in pts])], LENS_OFF if lens == "off" else LENS)]
        if lens != "off":
            g = 1.35 if lens == "bright" else 1.0
            gx, gy = lx - s * r * 0.06, ly - r * 0.07
            ops.append(([E(gx - r * 0.07 * g, gy - r * 0.045 * g, gx + r * 0.07 * g, gy + r * 0.045 * g)], WHITE))
    return ops


def helmet_marks(view_, hx, hy, lens="on", dots="dots"):
    """투구 무늬 (투구 안으로 잘림): 금속 반사 · 아래 패널 선 · 금색 줄 (정수리 → 이마, 뒷모습은 뒤통수로) · 렌즈 눈 둘 · 숨구멍 점.
    dots: dots 작은 점 한 줄 · o 가운데 동그랗게 (냠냠·놀람) · None 없음."""
    s = hs()
    rx, ry = s["hrx"], s["hry"]
    w = rx * 0.09
    out = []
    if view_ == "side":
        top = dp.arc(hx, hy, rx * 0.86, ry * 0.86, 262, 318, 10)
        out.append(([L(top, w * 2.0)], GOLD))
        out.append(([L(dp.arc(hx, hy, rx * 0.8, ry * 0.8, 190, 228, 8), 1.8)], SHINE))
        out.append(([L(dp.arc(hx - rx * 0.05, hy + ry * 0.05, rx * 0.95, ry * 0.86, 40, 150, 12), 1.0)], PANEL))
        out += lens_ops(hx + rx * 0.9, hy - ry * 0.06, rx, 1, lens, sx=0.42)
        out += lens_ops(hx + rx * 0.44, hy - ry * 0.08, rx, -1, lens, sx=0.85)
        if dots == "dots":
            out.append(([C(hx + rx * (0.56 + 0.13 * k), hy + ry * 0.55 - k * 0.4, 1.1) for k in range(3)], PANEL))
        elif dots == "o":
            out.append(([E(hx + rx * 0.66 - 2.2, hy + ry * 0.5 - 2.0, hx + rx * 0.66 + 2.2, hy + ry * 0.5 + 2.0)], PANEL))
        return out
    if view_ == "back":
        out.append(([T((hx - w, hy - ry - 2), (hx + w, hy - ry - 2), (hx + w, hy + ry * 0.35), (hx - w, hy + ry * 0.35))], GOLD))
        out.append(([L(dp.arc(hx, hy, rx * 0.8, ry * 0.8, 290, 330, 8), 1.8)], SHINE))
        out.append(([L(dp.arc(hx, hy + ry * 0.05, rx * 0.95, ry * 0.86, 30, 150, 14), 1.0)], PANEL))
        return out
    out.append(([T((hx - w, hy - ry - 2), (hx + w, hy - ry - 2), (hx + w * 0.8, hy - ry * 0.42), (hx - w * 0.8, hy - ry * 0.42))], GOLD))
    out.append(([L(dp.arc(hx, hy, rx * 0.8, ry * 0.8, 202, 242, 8), 1.8)], SHINE))
    out.append(([L(dp.arc(hx, hy + ry * 0.05, rx * 0.95, ry * 0.86, 32, 148, 14), 1.0)], PANEL))
    for side in (-1, 1):
        out += lens_ops(hx + side * rx * 0.42, hy - ry * 0.06, rx, side, lens)
    if dots == "dots":
        out.append(([C(hx + k * rx * 0.15, hy + ry * 0.56 + abs(k) * -0.35, 1.15) for k in range(-2, 3)], PANEL))
    elif dots == "o":
        out.append(([E(hx - 2.6, hy + ry * 0.5 - 2.4, hx + 2.6, hy + ry * 0.5 + 2.4)], PANEL))
    return out


def antenna_part(view_, hx, hy, spark=False):
    """작은 안테나 (투구 뒤에 꽂힌 막대 + 금색 구슬). spark: 구슬이 하얗게 반짝 (삐삐·충전)."""
    s = hs()
    rx, ry = s["hrx"], s["hry"]
    if view_ == "side":
        base, tip = (hx - rx * 0.12, hy - ry * 0.9), (hx - rx * 0.26, hy - ry - s["ant"])
    else:
        k = -1 if view_ == "back" else 1
        base, tip = (hx + k * rx * 0.3, hy - ry * 0.9), (hx + k * rx * 0.42, hy - ry - s["ant"])
    ball = C(tip[0], tip[1], 2.6)
    marks = [([ball], GOLD)]
    if spark:
        marks.append(([C(tip[0] - 0.5, tip[1] - 0.5, 1.3)], WHITE))
    return Part([L([base, tip], 1.5), ball], STICK, marks, outline=True, grow=0), tip


def spark_fx(tip, i):
    """⚡ 안테나 불똥 (충전·삐삐): 구슬 둘레의 작은 번개 셋 — 칸마다 자리가 바뀜."""
    fx = []
    for k in range(3):
        a = math.radians(k * 120 + i * 40 - 60)
        p0 = (tip[0] + math.cos(a) * 4.5, tip[1] + math.sin(a) * 4.5)
        p1 = (tip[0] + math.cos(a + 0.4) * 7.5, tip[1] + math.sin(a + 0.4) * 7.5)
        p2 = (tip[0] + math.cos(a - 0.1) * 10.0, tip[1] + math.sin(a - 0.1) * 10.0)
        fx.append(([L([p0, p1, p2], 1.4)], SPARK, 0.8))
    return fx


def beep_fx(tip, i):
    """👋 삐삐: 안테나 구슬 옆으로 퍼지는 작은 소리 물결 둘 (칸마다 커졌다 작아짐)."""
    k = [1.0, 1.4, 1.0, 1.4][i % 4]
    fx = []
    for j, r in enumerate((4.5, 8.0)):
        rr = r * k
        fx.append(([L(dp.arc(tip[0] + 1, tip[1], rr, rr, -50, 40, 8), 1.4)], dp.mix(OUT, WHITE, 0.35 + 0.2 * j), 0))
    return fx


# =====================================================================
# 팔다리 · 몸 · 제트팩
# =====================================================================
def limb(root, end, ln, w, color, gold, prefer, *, boot=False, toe=0.0, point=False):
    """팔 또는 다리 한 짝 (자기 외곽선): 빨간 마디 둘 + 윗마디 금색 판 + 끝은 장화 (금색 무릎 관절·정강이 패널 선) 또는 장갑 주먹.
    toe: 장화 코 방향 (옆모습 +1, 앞모습 0) · point: 발끝을 아래로 (날 때 — 장화가 길쭉)."""
    j = ik(root, end, ln, prefer)
    shapes_ = [L([root, j, end], w)]
    marks = [([L([lerp(root, j, 0.22), lerp(root, j, 0.8)], w * 0.8)], gold)]
    seg = math.hypot(j[0] - end[0], j[1] - end[1]) or 1.0
    n = unit_normal(j, end)
    if boot:
        marks.append(([C(j[0], j[1], w * 0.42)], gold))
        mid = lerp(j, end, 0.4)
        marks.append(([L([(mid[0] - n[0] * w * 0.5, mid[1] - n[1] * w * 0.5), (mid[0] + n[0] * w * 0.5, mid[1] + n[1] * w * 0.5)], 0.9)], PANEL))
        k = min(0.5, w * 0.95 / seg)
        cuff = lerp(end, j, k)
        x, y = end
        if point:
            dx, dy = end[0] - j[0], end[1] - j[1]
            d = math.hypot(dx, dy) or 1.0
            b = L([end, (x + dx / d * w * 0.7, y + dy / d * w * 0.7)], w * 1.05)
        else:
            x0 = x - w * 0.64 + min(toe, 0) * w * 0.45
            x1 = x + w * 0.64 + max(toe, 0) * w * 0.45
            b = T((x0 + 1.2, y - w * 0.52), (x1 - 1.2, y - w * 0.52), (x1, y + w * 0.5), (x0, y + w * 0.5))
        shapes_.append(b)
        marks.append(([L([cuff, end], w + 1.6), b], BOOT))
    else:
        fist = C(end[0], end[1], w * 0.62)
        shapes_.append(fist)
        k = min(0.36, w * 0.7 / seg)
        marks.append(([fist, L([lerp(end, j, k), end], w + 0.8)], FIST))
    marks.append(([L([(p[0] + n[0] * w * 0.22, p[1] + n[1] * w * 0.22) for p in (lerp(j, end, 0.12), lerp(j, end, 0.5))], 1.0)], SHINE))
    return Part(shapes_, color, marks, outline=True, grow=0), j


def hero_shoulders(view_, hip, rot):
    """hero가 쓰는 어깨 자리 [먼/왼, 가까운/오른] (프레임 좌표)."""
    s = hs()
    hx0, hy0 = hip
    top = hy0 + s["lw"] * 0.35 - s["th"]
    bm = dp._xf_matrix(rot=rot, pivot=(hx0, hy0))
    if view_ == "side":
        pts = [(hx0 - 1.5, top + s["aw"] * 0.75), (hx0 + 1.0, top + s["aw"] * 0.8)]
    else:
        pts = [(hx0 - (s["tw"] * 1.05 - s["aw"] * 0.45), top + s["aw"] * 0.6), (hx0 + (s["tw"] * 1.05 - s["aw"] * 0.45), top + s["aw"] * 0.6)]
    return [dp._apply(bm, p) for p in pts]


def torso_part(view_, hx0, top, bottom, glow):
    """갑옷 몸통 (어깨 넓고 허리 좁음) + 금색 허리판 · 패널 선 · 반사 줄 · 가슴 하트 (뒷모습은 등 패널)."""
    s = hs()
    th, tw = s["th"], s["tw"]
    if view_ == "side":
        ws = tw * 0.8
        shape = [E(hx0 - ws, top, hx0 + ws, top + th * 0.7),
                 T((hx0 - ws * 0.86, top + th * 0.4), (hx0 + ws * 0.92, top + th * 0.4), (hx0 + ws * 0.66, bottom), (hx0 - ws * 0.62, bottom))]
        marks = [([T((hx0 - ws * 1.2, bottom - th * 0.28), (hx0 + ws * 1.2, bottom - th * 0.3), (hx0 + ws * 1.2, bottom + 4), (hx0 - ws * 1.2, bottom + 4))], GOLD),
                 ([L([(hx0 + ws * 0.15, top + 3), (hx0 + ws * 0.62, top + th * 0.58)], 1.0)], PANEL),
                 ([L([(hx0 - ws * 0.55, top + 4), (hx0 - ws * 0.45, top + th * 0.45)], 1.3)], SHINE)]
        marks += heart_ops(hx0 + ws * 0.55, top + heart_dy(), HEART_R * s["emb"], glow, sx=0.58)
        return Part(shape, RED, marks, outline=True, grow=0)
    if _ST["lvl"] == 0:
        shape = [E(hx0 - tw * 1.08, top, hx0 + tw * 1.08, bottom + 1.5)]
    else:
        shape = [E(hx0 - tw * 1.12, top, hx0 + tw * 1.12, top + th * 0.55),
                 T((hx0 - tw * 1.1, top + th * 0.25), (hx0 + tw * 1.1, top + th * 0.25), (hx0 + tw * 0.64, bottom), (hx0 - tw * 0.64, bottom))]
    plate = T((hx0 - tw * 0.8, bottom - th * 0.2), (hx0 + tw * 0.8, bottom - th * 0.2), (hx0 + tw * 0.7, bottom + 4), (hx0 - tw * 0.7, bottom + 4))
    marks = [([plate], GOLD), ([L([(hx0 - tw * 0.8, bottom - th * 0.2), (hx0 + tw * 0.8, bottom - th * 0.2)], 0.9)], dp.mix(GOLD, OUT, 0.35))]
    if view_ == "back":
        marks += [([L([(hx0, top + 3), (hx0, bottom - th * 0.3)], 1.0)], PANEL),
                  ([L([(hx0 + tw * 0.75, top + 4), (hx0 + tw * 0.55, top + th * 0.4)], 1.3)], SHINE)]
        return Part(shape, RED, marks, outline=True, grow=0)
    marks += [([L([(hx0 - tw * 0.92, top + 3), (hx0 - tw * 0.28, top + th * 0.62)], 1.0),
                L([(hx0 + tw * 0.92, top + 3), (hx0 + tw * 0.28, top + th * 0.62)], 1.0)], PANEL),
              ([L([(hx0 - tw * 0.98, top + 5), (hx0 - tw * 0.74, top + th * 0.36)], 1.3)], SHINE)]
    marks += heart_ops(hx0, top + heart_dy(), HEART_R * s["emb"], glow)
    return Part(shape, RED, marks, outline=True, grow=0)


def pack_geom(hx0, top):
    """옆모습 제트팩 자리 (몸통 좌표, 오른쪽을 봄 — 등은 왼쪽): (왼, 오른, 위, 아래, 분사구 끝 가운데)."""
    s = hs()
    ws = s["tw"] * 0.8
    pw = s["tw"] * 0.55 + 2.5
    x0, x1 = hx0 - ws - pw + 2.5, hx0 - ws * 0.35
    y0, y1 = top + 1.0, top + s["th"] * 0.78
    return x0, x1, y0, y1, (x0 + pw * 0.42, y1 + 4.5)


def pack_parts(view_, hx0, top):
    """🚀 제트팩 부품 (뒤 → 앞): 옆모습은 등 뒤 강철 통 + 뒤로 치솟은 작은 금색 날개 + 아래 분사구 · 뒷모습은 등 가운데 통 +
    양옆 금색 날개 둘 + 아래 분사구 둘 · 앞모습은 어깨 뒤로 날개 끝만 빼꼼."""
    s = hs()
    tw = s["tw"]
    if view_ == "front":
        wings = []
        for side in (-1, 1):
            wings.append(T((hx0 + side * tw * 0.7, top + 3), (hx0 + side * tw * 1.7, top - 6), (hx0 + side * tw * 1.6, top + 1),
                           (hx0 + side * tw * 0.9, top + 10)))
        return [Part(wings, GOLD_FAR, outline=True, grow=0)]
    if view_ == "back":
        x0, x1 = hx0 - tw * 0.62, hx0 + tw * 0.62
        y0, y1 = top + 2.0, top + s["th"] * 0.8
        wings = [T((hx0 + side * tw * 0.4, top + 5), (hx0 + side * tw * 1.65, top - 4), (hx0 + side * tw * 1.5, top + 3.5),
                   (hx0 + side * tw * 0.5, top + 12)) for side in (-1, 1)]
        nozzles = [T((hx0 + side * tw * 0.32 - 3.2, y1 - 1), (hx0 + side * tw * 0.32 + 3.2, y1 - 1), (hx0 + side * tw * 0.32 + 2.6, y1 + 4.5),
                     (hx0 + side * tw * 0.32 - 2.6, y1 + 4.5)) for side in (-1, 1)]
        box = [E(x0, y0, x1, y0 + 9), T((x0, y0 + 4.5), (x1, y0 + 4.5), (x1, y1 - 3), (x0, y1 - 3)), E(x0, y1 - 7, x1, y1)]
        marks = [([L([(hx0, y0 + 3), (hx0, y1 - 2)], 1.0)], PACK_D), ([C(x0 + 3.2, y0 + 4.5, 1.3), C(x1 - 3.2, y0 + 4.5, 1.3)], GOLD),
                 ([L([(x0 + 2.2, y0 + 6), (x0 + 2.2, y1 - 5)], 1.2)], dp.mix(PACK, WHITE, 0.5))]
        return [Part(wings, GOLD, outline=True, grow=0), Part(nozzles, PACK_D, outline=True, grow=0), Part(box, PACK, marks, outline=True, grow=0)]
    x0, x1, y0, y1, (nx, ny) = pack_geom(hx0, top)
    pw = x1 - x0
    th = s["th"]
    wing = T((x0 + pw * 0.5, y0 + 3), (x0 - pw * 0.95, y0 + th * 0.2), (x0 - pw * 0.6, y0 + th * 0.32), (x0 + pw * 0.4, y0 + th * 0.3))
    nozzle = T((nx - 3.4, y1 - 1), (nx + 3.4, y1 - 1), (nx + 2.7, ny), (nx - 2.7, ny))
    box = [E(x0, y0, x1, y0 + 8), T((x0, y0 + 4), (x1, y0 + 4), (x1, y1 - 3), (x0, y1 - 3)), E(x0, y1 - 6, x1, y1)]
    marks = [([L([(x0 + pw * 0.55, y0 + 3), (x0 + pw * 0.55, y1 - 2)], 1.0)], PACK_D), ([C(x0 + pw * 0.28, y0 + 4.5, 1.3)], GOLD),
             ([L([(x0 + 1.8, y0 + 6), (x0 + 1.8, y1 - 5)], 1.2)], dp.mix(PACK, WHITE, 0.5))]
    return [Part([wing], GOLD, outline=True, grow=0), Part([nozzle], PACK_D, outline=True, grow=0), Part(box, PACK, marks, outline=True, grow=0)]


def trail_fx(p, d, length, phase, width=3.6):
    """분사구 끝 p에서 d 방향으로 길게 흐르는 하얀 빛 꼬리 (끝으로 갈수록 가늘고 칸마다 살짝 일렁) + 떨어져 나간 빛 방울."""
    n = (-d[1], d[0])
    edge, core = [], []
    steps = 14
    for j in range(steps + 1):
        t = j / steps
        wob = math.sin(t * 7.0 + phase * 2.1) * 0.9 * t
        wdt = width * (1 - 0.82 * t) + 0.6
        c = (p[0] + d[0] * length * t + n[0] * wob, p[1] + d[1] * length * t + n[1] * wob)
        edge.append((c, wdt + 1.4))
        core.append((c, wdt * 0.62))
    def band(pts):
        """가운데 줄 + 폭 → 다각형."""
        left = [(c[0] + n[0] * w, c[1] + n[1] * w) for c, w in pts]
        right = [(c[0] - n[0] * w, c[1] - n[1] * w) for c, w in pts]
        return T(*(left + right[::-1]))
    fx = [([band(edge), C(p[0], p[1], width + 1.6)], TRAIL_EDGE, 0), ([band(core), C(p[0], p[1], width * 0.75)], TRAIL, 0)]
    for k, t in enumerate((0.55, 0.8)):
        q = (p[0] + d[0] * length * (t + 0.05 * (phase % 2)) + n[0] * (4.5 if k else -4.0), p[1] + d[1] * length * (t + 0.05 * (phase % 2)) + n[1] * (4.5 if k else -4.0))
        fx += [([C(q[0], q[1], 1.9)], TRAIL_EDGE, 0), ([C(q[0], q[1], 1.1)], TRAIL, 0)]
    return fx


def flame_fx(p, d, length, phase):
    """분사구 끝의 짧은 하얀 불꽃 (둥실 · 공중제비 — 등 뒤 분사구에서만, 발바닥은 아님)."""
    n = (-d[1], d[0])
    ln = length * [1.0, 0.8, 0.92][phase % 3]
    tip = (p[0] + d[0] * ln, p[1] + d[1] * ln)
    w = 3.4
    outer = T((p[0] + n[0] * (w + 1.2), p[1] + n[1] * (w + 1.2)), (tip[0] + d[0] * 1.5, tip[1] + d[1] * 1.5), (p[0] - n[0] * (w + 1.2), p[1] - n[1] * (w + 1.2)))
    inner = T((p[0] + n[0] * w * 0.6, p[1] + n[1] * w * 0.6), tip, (p[0] - n[0] * w * 0.6, p[1] - n[1] * w * 0.6))
    return [([outer], TRAIL_EDGE, 0), ([inner], TRAIL, 0)]


# =====================================================================
# 꼬마 영웅 한 장
# =====================================================================
def hero(*, view_="side", hip=None, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0), legs=None, arms=None, leg_bend=None, arm_bend=None,
         lens="on", dots="dots", heart="on", fx=(), ops=(), whole=None, arms_front=False, arms_behind=False, far_limbs_front=False,
         near_arm_back=False, point=False, jet=None, head=None, spark=False, spark_fx_i=None, beep_i=None):
    """꼬마 영웅. view_: side (오른쪽을 봄) · front · back · hip: 엉덩이 관절 (없으면 똑바로 선 자리) · rot: 몸 기울기 (엉덩이 기준, 시계 방향) ·
    head_rot·head_d: 고개 · legs·arms: [먼/왼, 가까운/오른] 끝 자리 (target 형식) · leg_bend·arm_bend: 무릎·팔꿈치 굽는 쪽 ·
    lens·dots: 렌즈 눈·숨구멍 · heart: 가슴 하트 (on · bright · dim) · fx: 효과 목록 또는 손 자리를 받아 효과를 돌려주는 함수 ·
    arms_front: 앞모습 팔을 투구 위에 (팔뚝 막기) · arms_behind: 옆모습 가까운 팔도 투구 뒤에 · far_limbs_front: 먼 팔도 몸 앞에 ·
    near_arm_back: 옆모습 가까운 팔을 몸 뒤에 · point: 발끝을 아래로 (날 때) · jet: 분사구 빛 (("trail", 칸, 길이 몫) · ("flame", 칸, 길이)) ·
    head: 투구 대신 그릴 머리 부품 함수 (hx, hy) → [Part] (suit_up) · spark: 안테나 구슬 반짝 · spark_fx_i·beep_i: 안테나 불똥·삐삐 물결 칸."""
    s = hs()
    side_view = view_ == "side"
    hx0, hy0 = hip or ((60.0 if side_view else 64.0), stand_hip_y())
    lw, aw = s["lw"], s["aw"]
    bottom = hy0 + lw * 0.35
    top = bottom - s["th"]
    extra_fx = []
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
            arms = [("a", 14, 0.98), ("a", 10, 0.98)] if side_view else [("a", 16, 0.98), ("a", 16, 0.98)]
        leg_parts, arm_parts, pads, hands = [], [], [], []
        for j in range(2):
            far = side_view and j == 0
            end = target(leg_roots[j], legs[j], s["leg"], leg_sides[j])
            p, _ = limb(leg_roots[j], end, s["leg"], lw, RED_FAR if far else RED, GOLD_FAR if far else GOLD, leg_pref[j], boot=True,
                        toe=(1 if side_view else 0) if view_ != "back" else 0, point=point)
            leg_parts.append(p)
            end = target(sh_roots[j], arms[j], s["arm"], arm_sides[j])
            p, _ = limb(sh_roots[j], end, s["arm"], aw, RED_FAR if far else RED, GOLD_FAR if far else GOLD, arm_pref[j])
            arm_parts.append(p)
            hands.append(end)
            r = aw * (0.78 if side_view else 0.86)
            pads.append(Part([E(sh_roots[j][0] - r * 1.08, sh_roots[j][1] - r * 0.95, sh_roots[j][0] + r * 1.08, sh_roots[j][1] + r * 0.85)],
                             GOLD_FAR if far else GOLD, [([L(dp.arc(sh_roots[j][0], sh_roots[j][1], r * 0.7, r * 0.6, 205, 250, 6), 1.1)], SHINE)],
                             outline=True, grow=0))
        # 몸통 + 제트팩 (몸 기울기 안에서)
        with xf(rot=rot, pivot=(hx0, hy0)):
            torso = torso_part(view_, hx0, top, bottom, heart)
            packs = pack_parts(view_, hx0, top)
            neck = (hx0 + (1.5 if side_view else 0.0), top)
        if jet and side_view:
            *_, noz = pack_geom(hx0, top)
            p0 = P(noz)
            p1 = P((noz[0], noz[1] + 1.0))
            d = (p1[0] - p0[0], p1[1] - p0[1])
            kind, phase, amount = jet
            if kind == "trail":
                d = (d[0] * 0.92 - 0.12, d[1] * 0.92 - 0.06)
                dn = math.hypot(*d) or 1.0
                d = (d[0] / dn, d[1] / dn)
                length = max(10.0, (p0[0] - 3.0) / max(0.3, -d[0])) * amount
                extra_fx += trail_fx(p0, d, length, phase)
                far_p = (p0[0] + 1.5, p0[1] - 2.8)
                extra_fx += trail_fx(far_p, d, length * 0.9, phase + 1, width=2.4)
            else:
                extra_fx += flame_fx(p0, d, amount, phase)
        neck = P(neck)
        # 머리 (투구 + 안테나, 또는 suit_up의 맨얼굴·반쪽 투구)
        rx, ry = s["hrx"], s["hry"]
        with xf(rot=rot + head_rot, pivot=neck):
            hx, hy = neck[0] + head_d[0] + (2.0 if side_view else 0.0), neck[1] - ry + OVERLAP + head_d[1]
            dp.mark_head(hx, hy - ry + 2, rx * 1.7)
            if head is not None:
                head_parts = head(hx, hy)
                tip = None
            else:
                ant, tip = antenna_part(view_, hx, hy, spark)
                head_parts = [ant, Part([E(hx - rx, hy - ry, hx + rx, hy + ry)], RED, helmet_marks(view_, hx, hy, lens, dots), outline=True, grow=0)]
            if tip is not None and spark_fx_i is not None:
                extra_fx += spark_fx(tip, spark_fx_i)
            if tip is not None and beep_i is not None:
                extra_fx += beep_fx(tip, beep_i)
        # 맨 앞의 Part 바탕색이 실루엣 틈(겨드랑이·가랑이)을 채움 — 짙은 빨강으로
        parts = [Part([], RED_FAR)]
        if side_view:
            parts += packs
            parts += [arm_parts[0], pads[0]] if not far_limbs_front else []
            parts += [arm_parts[1], pads[1]] if near_arm_back else []
            parts += [leg_parts[0], leg_parts[1], torso]
            parts += [arm_parts[1], pads[1]] if arms_behind and not near_arm_back else []
            parts += head_parts
            parts += ([arm_parts[0], pads[0]] if far_limbs_front else []) + ([] if arms_behind or near_arm_back else [arm_parts[1], pads[1]])
        else:
            # 뒷모습은 투구가 팔을 가림 (팔을 투구 위에 그리지 않음 — 벽 타기에서 팔이 머리를 덮던 거듭된 실수)
            if view_ == "back":
                arms_front = False
            if view_ == "front":
                parts += packs
            parts += leg_parts + [torso]
            if view_ == "back":
                parts += packs
            if not arms_front:
                parts += arm_parts + pads
            else:
                parts += pads
            parts += head_parts
            if arms_front:
                parts += arm_parts
    if callable(fx):
        fx = fx(hands)
    return dp.compose(parts, list(ops), list(fx) + extra_fx)


SIDE_CX = 60.0


def H(x=SIDE_CX, dy=0.0):
    """엉덩이 관절 자리 (x, 선 높이 + dy)."""
    return (x, stand_hip_y() + dy)


def floor_at(x, lift=0.0):
    """바닥에 닿은 발 관절 (장화 바닥 = FLOOR - lift)."""
    return (x, foot_y() - lift)


def head_center(hip, rot=0.0, head_rot=0.0, view_="side", head_d=(0.0, 0.0)):
    """hero와 같은 계산의 (목, 투구 가운데) 프레임 자리."""
    s = hs()
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    side = view_ == "side"
    neck = dp._apply(dp._xf_matrix(rot=rot, pivot=hip), (hip[0] + (1.5 if side else 0.0), top))
    hc = dp._apply(dp._xf_matrix(rot=rot + head_rot, pivot=neck),
                   (neck[0] + head_d[0] + (2.0 if side else 0.0), neck[1] - s["hry"] + OVERLAP + head_d[1]))
    return neck, hc


def reach_all(hip, rot, ends, view_="side"):
    """두 팔 끝을 팔 길이 안으로."""
    return [reach(sh, e, hs()["arm"], 1.0) for sh, e in zip(hero_shoulders(view_, hip, rot), ends)]


# =====================================================================
# 옆모습 (오른쪽을 본다)
# =====================================================================
WALK = [(-7, 7, 0, 3), (0, 0, 2.5, 0), (7, -7, 3, 0), (0, 0, 0, 2.5)]  # (먼 발 dx, 가까운 발 dx, 먼 발 듦, 가까운 발 듦)


def mu_walk(sp, i):
    """철컹철컹 씩씩하게 걷기: 두 다리를 번갈아 앞뒤로, 팔은 반대로 흔듦, 몸 통통."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    hip = H(dy=[0, -1.2, 0, -1.2][i])
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    sw = [1, 0, -1, 0][i]
    return hero(hip=hip, legs=legs, arms=[("a", 22 * sw, 0.96), ("a", -22 * sw + 4, 0.96)], rot=2)


def mu_run(sp, i):
    """다다다 달리기: 앞으로 기울고 크게 성큼, 팔을 힘차게, 속도선 (분사구는 꺼둠 — 달리기는 다리로)."""
    st = [1, 0, -1, 0][i]
    hip = H(dy=[-1.5, -3.5, -1.5, -3.5][i])
    legs = [("a", -40 * st, 0.95 if st else 0.75), ("a", 40 * st, 0.95 if st else 0.75)]
    if st == 0:
        legs = [("a", -25, 0.8), ("a", 35, 0.7)] if i == 1 else [("a", 35, 0.7), ("a", -25, 0.8)]
    arms = [("a", 55 * st + 5, 0.8), ("a", -45 * st + 5, 0.8)]
    return hero(hip=hip, legs=legs, arms=arms, rot=12, dots="o", arm_bend=[(1, 0.3), (1, 0.3)],
                fx=dp.speed_fx(4, (70, 86, 102), 10, [0, -3, 0, -3][i]))


def mu_idle(sp, i):
    """제자리 숨쉬기: 가슴이 살짝 오르내림, 주먹은 허리께."""
    return hero(hip=H(dy=[0, 0.8][i]), arms=[("a", 12, 0.97), ("a", 8 + 3 * i, 0.97)], head_d=(0, [0, 0.6][i]))


def mu_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞 (투구만, 마지막엔 렌즈 깜빡)."""
    rot, lens = [(-8, "on"), (0, "bright"), (8, "on"), (0, "off")][i]
    return hero(head_rot=rot, lens=lens)


def sit_hip(x=SIDE_CX):
    """바닥에 앉은 엉덩이 관절."""
    return (x, FLOOR - hs()["lw"] * 0.75)


def mu_sleep(sp, i):
    """웅크려 쿨쿨 (절전): 바닥에 앉아 무릎을 끌어안고 투구를 무릎에 묻음, 렌즈 꺼짐, 하트 흐릿, z."""
    s = hs()
    hip = sit_hip(SIDE_CX - 2)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 3, foot_y()), (knee_x + 5, foot_y())]
    arms = [(knee_x + 1, hip[1] - s["leg"] * 0.45 - 4), (knee_x + 3, hip[1] - s["leg"] * 0.45 - 2)]
    fx = [z_fx(108, 30 - 3 * i, 4.0), z_fx(118, 16 - 3 * i, 2.6)]
    return hero(hip=hip, rot=14 + i, head_rot=10, head_d=(2, 1 + 0.6 * i), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                arm_bend=[(0, 1), (0, 1)], lens="off", dots=None, heart="dim", fx=fx)


def mu_annoyed(sp, i):
    """🛡️ 팔뚝 막기 (옆모습 — 다른 펫이 찌르거나 투닥댈 때): 무릎을 굽혀 투구를 숙이고 두 팔뚝을 엇갈려 얼굴 앞을 가림, 💢."""
    s = hs()
    hip = H(SIDE_CX - 4, s["leg"] * 0.22 + [0, 0.6][i])
    rot, head_rot = 6, 12
    _, hc = head_center(hip, rot, head_rot, head_d=(-1, 3))
    rx, ry = s["hrx"], s["hry"]
    ends = [(hc[0] + rx * 0.95, hc[1] - ry * 0.05 + [0, 0.5][i]), (hc[0] + rx * 1.0, hc[1] + ry * 0.2)]
    arms = reach_all(hip, rot, ends)
    legs = [floor_at(hip[0] - 7), floor_at(hip[0] + 8)]
    return hero(hip=hip, rot=rot, head_rot=head_rot, head_d=(-1, 3), legs=legs, leg_bend=[(1, -0.3), (1, -0.3)], arms=arms,
                arm_bend=[(1, 1), (1, 1)], far_limbs_front=True, lens="on", dots=None,
                fx=[anger_fx(108 + i, 24 - i, 5.0 + 0.6 * i)])


def mu_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 주먹 하나를 위로 번쩍 솟음 (다리 모음) → 다리 내림."""
    s = hs()
    if i == 0:
        hip = H(dy=s["leg"] * 0.32)
        return hero(hip=hip, rot=12, legs=[floor_at(SIDE_CX - 3), floor_at(SIDE_CX + 3)], arms=[("a", -30, 0.9), ("a", -25, 0.9)], dots="o")
    hip = (SIDE_CX, [0, 84, 90][i] - (s["leg"] - 22) * 0.5)
    legs = [("a", -30, 0.7), ("a", 25, 0.62)] if i == 1 else [("a", -8, 0.92), ("a", 10, 0.9)]
    arms = [("a", 20, 0.9), ("a", 165, 0.98)] if i == 1 else [("a", 30, 0.9), ("a", 120, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, lens="bright", dots="o", arm_bend=[(1, 0), (1, 0)], arms_behind=i == 1)


def mu_fall(sp, i):
    """떨어짐 (던져졌을 때만 — 스스로는 제트팩으로 내려감): 팔다리를 허우적, 숨구멍 동그랗게."""
    w = [0, 1][i]
    hip = (SIDE_CX, 84.0)
    legs = [("a", -40 - 15 * w, 0.85), ("a", 40 + 15 * w, 0.75)]
    arms = [("a", 150 - 25 * w, 0.95), ("a", 125 + 25 * w, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, dots="o", rot=[-6, 6][i], arm_bend=[(0, -1), (0, -1)])


def mu_dragged(sp, i):
    """뒷덜미가 잡혀 대롱대롱: 팔다리가 축 늘어지고 살랑, 땀."""
    s = hs()
    sway = [-6, 6][i]
    top_y = 20.0 + s["ant"]
    hip_y = top_y + 2 * s["hry"] - OVERLAP + s["th"] - s["lw"] * 0.35 - 2
    hip = (SIDE_CX + 2, hip_y)
    return hero(hip=hip, legs=[("a", -6, 0.97), ("a", 8, 0.95)], arms=[("a", 12, 0.95), ("a", 18, 0.95)], dots="o",
                whole=dict(rot=sway * 0.5, pivot=(SIDE_CX, 14)), fx=sweat_fx(106, 30))


def mu_land(sp, i):
    """착지 (한 번): 무릎을 살짝 굽혀 사뿐 → 바로 섬."""
    s = hs()
    if i == 0:
        return hero(hip=H(dy=s["leg"] * 0.18), rot=6, legs=[floor_at(SIDE_CX - 4), floor_at(SIDE_CX + 4)], arms=[("a", 40, 0.9), ("a", 34, 0.9)])
    return hero(lens="bright")


def mu_lookup(sp, i):
    """올려다보기: 투구를 젖혀 렌즈가 위로, 반짝."""
    return hero(head_rot=dp.lookup_tilt(i), lens="bright", fx=dp.lookup_fx(i, 110, 14))


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


def heart_center(hip_y=None):
    """앞모습 가슴 하트 가운데 (프레임 좌표)."""
    return (64.0, torso_top(hip_y) + heart_dy())


CHARGE = ["on", "dim", "bright", "dim"]  # 🔋 충전 중 하트 깜빡 (칸마다)


def mu_sit_front(sp, i):
    """🔋 앉아서 충전: 바닥에 털썩 앉아 무릎을 양옆으로, 두 주먹은 무릎 위 — 하트가 깜빡, 안테나 구슬에 불똥 (마지막 칸 렌즈 깜빡)."""
    s = hs()
    hip_y = FLOOR - s["lw"] * 0.8 + [0, 0.5, 0, 0.5][i]
    legs = [(64 - s["hip"] - s["leg"] * 0.75 - 3, foot_y()), (64 + s["hip"] + s["leg"] * 0.75 + 3, foot_y())]
    knee_y = hip_y - s["leg"] * 0.3
    arms = reach_all((64.0, hip_y), 0, [(64 - s["tw"] - 3, knee_y), (64 + s["tw"] + 3, knee_y)], "front")
    bolt = [] if i % 2 else [([T((104, 52), (98, 60), (102, 60), (99, 68), (107, 58), (103, 58))], SPARK, 1.0)]
    return front(hip=(64.0, hip_y), legs=legs, leg_bend=[(-1, -1), (1, -1)], arms=arms, arm_bend=[(-1, 0.2), (1, 0.2)],
                 heart=CHARGE[i], lens="off" if i == 3 else "on", spark=i % 2 == 0, spark_fx_i=i, fx=bolt)


def mu_wave(sp, i):
    """👋 삐삐 손 흔들기: 한 주먹을 번쩍 들어 흔들흔들 (다른 주먹은 허리), 안테나 구슬이 깜빡이며 소리 물결."""
    a = [118, 140, 118, 140][i]
    return front(arms=[("a", 18, 0.95), ("a", a, 1.0)], arm_bend=[(-1, 0.3), (1, 0)], lens="bright" if i % 2 else "on",
                 head_rot=[-3, -1, -3, -1][i], spark=i % 2 == 1, beep_i=i)


def mu_tilt(sp, i):
    """투구 갸웃 + ? (한 주먹은 턱)."""
    top = torso_top()
    return front(head_rot=[8, 12, 12][i], arms=[("a", 15, 0.95), (64 + 6, top + 2)], arm_bend=[(-1, 0.3), (1, 0.6)],
                 fx=[question_fx(108, 22 - i)], dots=None)


def mu_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 두 다리를 위로 버둥, 두 팔 번쩍, 하트 반짝."""
    s = hs()
    w = [0, 2, 0, -2][i]
    hip = (64.0, FLOOR - s["lw"] * 0.8)
    legs = [("a", 112 + 6 * w, 0.9), ("a", 112 - 6 * w, 0.9)]
    arms = [("a", 128 - 5 * w, 0.95), ("a", 128 + 5 * w, 0.95)]
    return front(hip=hip, legs=legs, arms=arms, lens="bright", dots="o", heart="bright")


def mu_stretch(sp, i):
    """💗 하트 번쩍 기지개 (한 번): 두 팔 쭉 위로 → 가슴을 펴고 두 팔을 활짝, 하트가 번쩍 (빛줄기가 가슴 밖까지) → 서기."""
    if i == 0:
        return front(arms=[("a", 140, 1.0), ("a", 140, 1.0)], hip=F(-1.5), heart="on")
    if i == 1:
        hc = heart_center()
        return front(arms=[("a", 100, 1.0), ("a", 100, 1.0)], lens="bright", dots="o", heart="bright",
                     fx=heart_flash_fx(hc[0], hc[1], HEART_R * hs()["emb"]))
    return front(heart="on")


def mu_heart(sp, i):
    """하트 뿜뿜: 두 주먹을 가슴 하트 양옆에 모으고 하트가 밝게, 작은 하트가 떠오름."""
    hc = heart_center()
    arms = reach_all(F(), 0, [(hc[0] - 9, hc[1] + 3), (hc[0] + 9, hc[1] + 3)], "front")
    return front(arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], lens="bright", heart="bright", fx=dp.heart_burst(i))


def mu_tickle(sp, i):
    """🛡️ 간질간질 막기: 몸을 비비 꼬며 두 팔뚝을 X로 엇갈려 투구 앞을 가림 (렌즈가 팔 사이로 빼꼼)."""
    s = hs()
    hip = F(s["leg"] * 0.15)
    rot = [-6, 6, -2][i]
    _, hc = head_center(hip, rot, 0, "front", (0, 3))
    ends = [(hc[0] + s["hrx"] * 0.42, hc[1] + s["hry"] * 0.05), (hc[0] - s["hrx"] * 0.42, hc[1] - s["hry"] * 0.02)]
    arms = reach_all(hip, rot, ends, "front")
    return front(hip=hip, rot=rot, head_d=(0, 3), arms=arms, arm_bend=[(-1, 1), (1, 1)], arms_front=True, dots=None)


def battery_bubble_fx(cx, cy, r=12.0):
    """🔋 배터리 생각 풍선 (배고플 때): 빨간 마지막 칸 하나만 남은 건전지."""
    fx = []
    for x, y, rr in [(cx - r - 7, cy + r + 8, 2.0), (cx - r - 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    body = T((cx - 4.8, cy - 6.5), (cx + 4.8, cy - 6.5), (cx + 4.8, cy + 7.5), (cx - 4.8, cy + 7.5))
    fx += [([T((cx - 2, cy - 8.5), (cx + 2, cy - 8.5), (cx + 2, cy - 6), (cx - 2, cy - 6))], GOLD, 0.8), ([body], PACK, 1.0),
           ([T((cx - 3.4, cy + 2.5), (cx + 3.4, cy + 2.5), (cx + 3.4, cy + 6), (cx - 3.4, cy + 6))], LOW, 0)]
    return fx


def mu_hungry(sp, i):
    """배고파 (배터리 부족): 두 주먹으로 배를 감싸고 투구를 떨굼, 하트 흐릿, 배터리 생각 풍선."""
    s = hs()
    y = torso_top() + s["th"] * 0.68
    return front(hip=F([0, 1, 0][i]), arms=[(59, y), (69, y)], arm_bend=[(-1, 0.5), (1, 0.5)], heart="dim", dots=None,
                 head_rot=-3, head_d=(0, 1.5), fx=battery_bubble_fx(28 + 2 * i, 20, 12.0))


EAT_MOUTH = 79.0  # 먹을 때 숨구멍 (프레임 y) — 그릇 밥 더미 바로 위


def mu_eat(sp, i):
    """냠냠 충전 (그릇 없음): 무릎 꿇고 투구를 숙여 숨구멍을 밥 더미 바로 위에 — 앙 ↔ 오물오물, 부스러기, 하트가 차오름."""
    s = hs()
    hip_y = EAT_MOUTH + [0, 1, 0, 1][i] - 0.56 * s["hry"] + s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]
    hip_y = min(hip_y, FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - 3, foot_y()), (64 + s["hip"] + 3, foot_y())]
    return front(hip=(64.0, hip_y), legs=legs, arms=[(44, 96), (84, 96)], arm_bend=[(-1, -0.2), (1, -0.2)],
                 lens="bright" if i % 2 else "on", dots="o" if i % 2 == 0 else "dots", heart="on" if i < 2 else "bright", fx=dp.eat_crumbs(i))


def mu_level_up(sp, i):
    """만세: 두 팔 번쩍, 하트 번쩍, 반짝이 (가운데 칸은 살짝 뜀)."""
    up = [114, 124, 118][i]
    return front(hip=F(-[0, 3, 1][i]), legs=None if i != 1 else [("a", -6, 0.97), ("a", 6, 0.97)], arms=[("a", up, 1.0), ("a", up, 1.0)],
                 lens="bright", dots="o", heart="bright", fx=dp.level_up_stars(i))


# =====================================================================
# 🪖 투구 장착 (앞모습) — 맨얼굴 → 반쪽 투구 둘이 양옆에서 캡슐처럼 → 착 → 렌즈 켜짐
# =====================================================================
SUIT_GAP = [1.0, 0.45, 0.0, 0.0]  # 칸마다 반쪽 투구가 떨어진 몫 (투구 반지름 배)


def kid_face_parts(hx, hy):
    """투구 벗은 꼬마 얼굴 (갈색 머리, 점 눈, 빙긋) — 투구보다 작은 동그란 머리."""
    s = hs()
    rx, ry = s["hrx"] * 0.8, s["hry"] * 0.84
    hair = [T(*dp.arc(hx, hy - ry * 0.05, rx * 1.3, ry * 0.42, 180, 360, 14), (hx + rx * 1.3, hy - ry * 2), (hx - rx * 1.3, hy - ry * 2)),
            L([(hx + rx * 0.15, hy - ry * 0.55), (hx + rx * 0.45, hy - ry * 0.25)], 2.2)]
    marks = [(hair, HAIR), ([L(dp.arc(hx, hy, rx * 0.78, ry * 0.78, 215, 250, 6), 1.6)], dp.mix(HAIR, WHITE, 0.35))]
    face = Part([E(hx - rx, hy - ry, hx + rx, hy + ry), E(hx - rx - 2.6, hy - 3.5, hx - rx + 2.6, hy + 3.5),
                 E(hx + rx - 2.6, hy - 3.5, hx + rx + 2.6, hy + 3.5)], SKIN, marks, outline=True, grow=0)
    return face, rx, ry


def shells_parts(hx, hy, gap, lit):
    """빨간 반쪽 투구 둘 (왼쪽·오른쪽, 사이가 gap × 반지름만큼 벌어짐): 닫히면 (gap 0) 온전한 투구 + 렌즈 (lit이면 켜짐)."""
    s = hs()
    rx, ry = s["hrx"], s["hry"]
    if gap <= 0:
        ant, _ = antenna_part("front", hx, hy, spark=lit)
        return [ant, Part([E(hx - rx, hy - ry, hx + rx, hy + ry)], RED, helmet_marks("front", hx, hy, "bright" if lit else "off", "dots"),
                          outline=True, grow=0)]
    out = []
    off = gap * rx * 1.05
    for side in (-1, 1):
        cx = hx + side * off
        half = dp.arc(cx, hy, rx, ry, 90, 270, 18) if side < 0 else dp.arc(cx, hy, rx, ry, -90, 90, 18)
        rim = [L([(cx, hy - ry + 1.5), (cx, hy + ry - 1.5)], 1.4)]
        marks = [(rim, PANEL), ([L(dp.arc(cx, hy, rx * 0.8, ry * 0.8, 205 if side < 0 else 300, 240 if side < 0 else 330, 6), 1.6)], SHINE),
                 ([T((cx - side * 0.5, hy - ry - 2), (cx + side * rx * 0.09, hy - ry - 2), (cx + side * rx * 0.07, hy - ry * 0.42),
                     (cx - side * 0.5, hy - ry * 0.42))], GOLD)]
        marks += lens_ops(cx + side * rx * 0.42, hy - ry * 0.06, rx, side, "off")
        out.append(Part([T(*half)], RED, marks, outline=True, grow=0))
    return out


def mu_suit_up(sp, i):
    """🪖 투구 장착 (한 번, 마지막 칸 유지): 0 맨얼굴 갈색 머리 꼬마 (반쪽 투구 둘이 양옆에 떠 있음) · 1 반쪽 투구가 미끄러져 다가옴 ·
    2 착 닫힘 (렌즈는 아직 꺼짐) · 3 렌즈가 켜지고 반짝."""
    gap = SUIT_GAP[i]

    def head(hx, hy):
        """이 칸의 머리 부품 (맨얼굴 뒤 → 반쪽 투구 앞)."""
        if gap <= 0:
            return shells_parts(hx, hy, 0, i == 3)
        face, _, _ = kid_face_parts(hx, hy + 2)
        return [face] + shells_parts(hx, hy, gap, False)

    s = hs()
    _, hc = head_center(F(), 0, 0, "front")
    ops = []
    if gap > 0:
        fy = hc[1] + 2
        for side in (-1, 1):
            ops += eye_ops("dot", hc[0] + side * 0.3 * s["hrx"], fy + 1, side > 0)
        ops += mouth_ops("grin" if i == 0 else "smile", hc[0], fy + 0.5 * s["hry"])
    fx = sparkle_fx(hc[0] + s["hrx"] + 6, hc[1] - s["hry"] * 0.6, 3.4) + sparkle_fx(hc[0] - s["hrx"] - 6, hc[1] - s["hry"] * 0.2, 2.6) if i == 3 else []
    arms = [("a", 20, 0.95), ("a", 20, 0.95)] if i < 2 else [("a", 30, 0.95), ("a", 30, 0.95)]
    return front(arms=arms, head=head, ops=ops, heart="bright" if i == 3 else "on", fx=fx)


# =====================================================================
# 구르기 (회전 원 안) — 갑옷 공
# =====================================================================
def mu_roll(sp, i):
    """무릎을 끌어안은 동그란 갑옷 공 (등에 제트팩): 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            rx, ry = 21.0, 19.5
            hx, hy = cx + 5, cy - 8
            pack = Part([E(cx - 30, cy - 4, cx - 12, cy + 24)], PACK, [([L([(cx - 21, cy - 2), (cx - 21, cy + 22)], 1.0)], PACK_D)], outline=True, grow=0)
            body = Part([C(cx - 4, cy + 7, 20.0)], RED, [([T((cx - 20, cy + 16), (cx + 12, cy + 16), (cx + 8, cy + 30), (cx - 16, cy + 30))], GOLD)],
                        outline=True, grow=0)
            knee = Part([E(cx + 2, cy + 6, cx + 28, cy + 24)], RED, [([C(cx + 22, cy + 12, 3.4)], GOLD), ([L([(cx + 6, cy + 10), (cx + 18, cy + 9)], 3.6)], GOLD)],
                        outline=True, grow=0)
            boot = Part([E(cx - 2, cy + 18, cx + 16, cy + 29)], BOOT, outline=True, grow=0)
            hand = Part([C(cx + 22, cy + 17, 5.0)], FIST, outline=True, grow=0)
            k = 21.0 / hs()["hrx"]
            with xf(sx=k, sy=k, pivot=(hx, hy)):
                head = Part([E(hx - hs()["hrx"], hy - hs()["hry"], hx + hs()["hrx"], hy + hs()["hry"])], RED,
                            helmet_marks("side", hx, hy, "bright" if i else "on", "o" if i else "dots"), outline=True, grow=0)
            dp.mark_head(hx, hy - ry + 2, rx * 1.6)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([Part([], RED_FAR), pack, body, knee, boot, head, hand], [], clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면 벽에 손발을 붙이고 엉금엉금, 등에 제트팩 (투구는 언제나 팔 위)
# =====================================================================
CLIMB = [(1.0, 0.2), (0.6, 0.6), (0.2, 1.0), (0.6, 0.6)]  # (왼손, 오른손) 올린 몫 — 발은 엇갈려


def musoe_back(reach_, *, dy=0.0, scratch=False, sweat=False):
    """뒷모습: 두 주먹을 투구 옆·위로 벽에 짚고 (reach: 0 어깨 옆 … 1 투구 위), 두 다리는 무릎을 벌려 벽에 디딤. 등엔 제트팩."""
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
    return hero(view_="back", hip=hip, legs=legs, leg_bend=[(-1, -1), (1, -1)], arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], fx=fx)


def mu_climb(sp, i):
    """뒷모습으로 기어오름: 왼주먹·오른주먹이 번갈아 위로, 발은 엇갈려 디딤, 몸 살짝 오르내림."""
    return musoe_back(CLIMB[i], dy=[0, -1, 0, -1][i])


def mu_slide(sp, i):
    """뒷모습으로 주르륵: 두 주먹 쭉 위로, 긁힌 자국 3줄, 땀 한 방울."""
    return musoe_back((1.0, 1.0), dy=[0, 2][i], scratch=True, sweat=True)


# =====================================================================
# 🚀 제트팩 날기 (옆모습, 오른쪽으로) — 몸을 앞으로 눕혀 등의 분사구에서 하얀 빛 꼬리가 뒤로 길게
# =====================================================================
FLY_HIP = (46.0, 90.0)  # 나는 몸의 엉덩이 관절 (몸은 오른쪽으로 뻗음 — 코어가 창을 바닥보다 띄움)


def flyer(i, *, fast=False, fx=()):
    """나는 무쇠인간 한 장 (i: 빛 꼬리 일렁 칸): 몸을 거의 수평으로 눕히고 (투구가 오른쪽 앞) 고개를 들어 앞을 봄,
    느긋할 땐 가까운 주먹을 앞으로 (먼 팔은 몸 옆), 빠를 땐 두 팔을 몸 옆에 붙여 뒤로 (로켓처럼), 다리는 모아 뒤로 쭉,
    등의 제트팩 분사구 둘에서 하얀 빛 꼬리가 뒤로 프레임 끝까지."""
    s = hs()
    rot = 84.0 if fast else 78.0
    head_rot = -72.0 if fast else -66.0
    bob = [0.0, -0.8, 0.4][i % 3] * (0.4 if fast else 1.0)
    hip = (FLY_HIP[0] - (s["leg"] - 22) * 0.2, FLY_HIP[1] + bob)
    head_d = (9.0, 1.0)  # 큰 투구를 앞으로 내밀어 등의 제트팩이 보이게
    _, hc = head_center(hip, rot, head_rot, head_d=head_d)
    sh = hero_shoulders("side", hip, rot)
    if fast:
        arms = [("a", -rot + 8, 0.92), ("a", -rot + 14, 0.92)]
    else:
        fist = (hc[0] + s["hrx"] * 0.62, hc[1] + s["hry"] * 0.95 + s["aw"] * 0.25)
        arms = [("a", -rot + 8, 0.92), reach(sh[1], fist, s["arm"], 1.0)]
    lift = [0, 2, -1][i % 3]
    legs = [("a", -rot - 2 + lift, 0.98), ("a", -rot + 3 - lift, 0.98)]
    return hero(hip=hip, rot=rot, head_rot=head_rot, head_d=head_d, legs=legs, leg_bend=[(1, -1), (1, -1)], arms=arms, arm_bend=[(0, 1), (0, 1)],
                lens="bright" if fast else "on", point=True, jet=("trail", i, 1.0), fx=fx)


def mu_fly(sp, i):
    """🚀 날기 (3칸 반복): 몸을 눕혀 가까운 주먹을 앞으로, 다리는 모아 뒤로, 등 뒤 하얀 빛 꼬리가 칸마다 일렁."""
    return flyer(i)


def mu_fly_fast(sp, i):
    """🚀💨 빨리 날기 (3칸 반복): 두 팔을 몸 옆에 붙여 로켓처럼 곧게, 빛 꼬리 + 속도선."""
    return flyer(i, fast=True, fx=dp.speed_fx(4, (58, 64 + [0, 1, -1][i]), 10, [0, -4, -2][i]))


FLIP = [0, 90, 180, 270]  # 공중제비 칸마다 도는 각도 (시계 방향 = 오른쪽으로 날며 앞으로 돎)


def mu_fly_flip(sp, i):
    """🔄 공중제비 (4칸 반복 — 코어가 한 바퀴 시간만큼): 무릎을 끌어안듯 웅크린 몸이 칸마다 90°씩 앞으로 돎, 등 분사구에 짧은 불꽃."""
    s = hs()
    cx, cy = 64.0, 65.0 - (s["hry"] - 22.8) * 0.8
    hip = (cx - 2, cy + s["th"] * 0.25)
    knee = hip[0] + s["leg"] * 0.55 + 3
    legs = [(knee + 1, hip[1] + s["leg"] * 0.25), (knee + 3, hip[1] + s["leg"] * 0.2)]
    arms = [(knee + 1, hip[1] - s["leg"] * 0.3 - 3), (knee + 4, hip[1] - s["leg"] * 0.3 - 1)]
    return hero(hip=hip, rot=18, head_rot=8, legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms, arm_bend=[(0, 1), (0, 1)],
                lens=["bright", "on", "bright", "on"][i], dots="o", jet=("flame", i, 7.0), whole=dict(rot=FLIP[i], pivot=(cx, cy - 6)))


def mu_hover(sp, i):
    """🫧 둥실 (3칸 반복): 똑바로 선 채 떠 있음 — 발끝이 아래 (바닥에서 살짝 뜸), 두 주먹은 허리, 등 분사구에서 아래로 짧은 하얀 불꽃."""
    s = hs()
    hip = H(SIDE_CX + 4, -9.0 - [0, 1.0, 0.4][i])
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    wy = top + s["th"] * 0.72
    arms = reach_all(hip, 0, [(hip[0] - 4, wy), (hip[0] + s["tw"] * 0.7, wy)])
    return hero(hip=hip, legs=[("a", -6, 0.98), ("a", 6, 0.96)], leg_bend=[(1, -0.2), (1, -0.2)], arms=arms, arm_bend=[(1, 0.2), (1, 0.2)],
                point=True, jet=("flame", i, 9.0))


MUSOE_POSES = {"walk": mu_walk, "run": mu_run, "roll": mu_roll, "idle": mu_idle, "look": mu_look, "sleep": mu_sleep, "hop": mu_hop,
               "fall": mu_fall, "dragged": mu_dragged, "land": mu_land, "sit_front": mu_sit_front, "wave": mu_wave, "tilt": mu_tilt,
               "belly": mu_belly, "stretch": mu_stretch, "heart": mu_heart, "tickle": mu_tickle, "annoyed": mu_annoyed,
               "hungry": mu_hungry, "eat": mu_eat, "level_up": mu_level_up, "climb": mu_climb, "slide": mu_slide, "lookup": mu_lookup,
               "fly": mu_fly, "fly_fast": mu_fly_fast, "fly_flip": mu_fly_flip, "hover": mu_hover, "suit_up": mu_suit_up}
MUSOE_POSES = {k: (v if k == "roll" else dp._fit(v)) for k, v in MUSOE_POSES.items()}  # roll은 회전 원 안 그대로
SOAR_ANIMS = ["fly", "fly_fast", "fly_flip", "hover"]  # = Rust manifest::SOAR_ANIMS (🦸 슈퍼인간과 같은 날기 뇌)
SUIT_ANIMS = ["suit_up"]  # = Rust manifest::SUIT_ANIMS (behavior::Special::SuitUp)

MUSOE = Species(
    id="musoe", name="무쇠인간", walk_speed=92, scale=1.0, base=RED, back=RED, belly=RED, nose=OUT, mouth="w",
    front_tail_color=RED, tail_color=RED, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + SOAR_ANIMS + SUIT_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=GOLD, poses=MUSOE_POSES,
    outline=OUT,
    # 색은 파스텔 없이 또렷한 빨강·금색 · 크기는 DIMS가 정함 (fit 1)
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 무쇠인간을 등록함."""
    dp.SPECIES[MUSOE.id] = MUSOE


register()


def preview_musoe(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hop", 1), ("roll", 0),
             ("fly", 0), ("fly_fast", 0), ("fly_flip", 1), ("hover", 0), ("suit_up", 0), ("suit_up", 3), ("stretch", 1), ("tickle", 0),
             ("annoyed", 0), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "fly"), bg=bg)


def preview_rows(by_stage, path, anims, stages=STAGES, bg=(255, 255, 255)):
    """동작들 (줄마다 한 동작의 모든 칸) × 단계 (열 띠) — 새 그림 확인용."""
    from PIL import Image
    cell = 132
    im = Image.new("RGBA", (len(stages) * 4 * cell + (len(stages) - 1) * 12, len(anims) * cell), bg + (255,))
    for c, st in enumerate(stages):
        for r, a in enumerate(anims):
            for k, f in enumerate(by_stage[st][a]):
                im.alpha_composite(f, (c * (4 * cell + 12) + k * cell, r * cell))
    im.save(path)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(MUSOE, root / "ui" / "pets" / MUSOE.id))
    preview_musoe(by_stage, pv / "stages_musoe.png")
    dp.preview_back(by_stage, pv / "climb_musoe.png")
    preview_rows(by_stage, pv / "moves_musoe.png", SOAR_ANIMS + SUIT_ANIMS + ["sit_front", "stretch", "wave", "tickle", "annoyed", "hungry"],
                 stages=["baby", "teenager", "adult"])
    print("ok: musoe")


if __name__ == "__main__":
    main()
