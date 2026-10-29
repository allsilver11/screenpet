"""카와이 스프라이트 생성기 — 4종 × 18동작 (+ 고양이 arch, 강아지 wag), 128px.

    python tools/draw_pets.py        → ui/pets/<id>/{sheet.png, pet.json} + tools/preview/all_{light,dark}.png

구성:
  1. 엔진      — 4배 슈퍼샘플 마스크, 변환 스택(xf), 도형(E/T/L), 녹이기(melt)로 한 덩어리 실루엣 + 굵은 외곽선
  2. 얼굴·효과 — 눈/입/볼터치, 하트·물음표·z·💢
  3. 종별 설정 — 색과 무늬(데이터). 앞모습/옆모습 머리·몸·꼬리·귀를 표준 좌표계로 적는다
  4. 부품      — 종별 설정을 현재 변환 아래에서 Part로 만든다
  5. 포즈      — 동작별 공통 뼈대. 부품을 옮기고 돌려 배치할 뿐 종별 분기는 없다
  6. 출력      — 시트·매니페스트·미리보기
"""

import json
import math
import sys
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sprite_spec import ANIMS, FRAME, OPTIONAL_ANIMS, ROLL_CENTER, ROLL_RADIUS, SPEECH, manifest  # noqa: E402

# =====================================================================
# 1. 엔진
# =====================================================================
K = 4  # 슈퍼샘플 배율
S = FRAME * K
LIFT = 2  # 발끝 외곽선이 프레임 맨 아래 1px 투명 여백 안에 들어오게 전체를 올린다

LINE = (132, 96, 92)
PINK = (250, 180, 180)
NOSE = (236, 136, 142)
DARK_NOSE = (96, 66, 60)
BLUSH = (252, 196, 194)
BLUSH_LINE = (238, 150, 150)
EYE = (84, 58, 56)
WHITE = (255, 255, 255)
MOUTH_IN = (196, 92, 100)
TONGUE = (248, 142, 152)
HEART = (246, 116, 140)
ZZZ = (112, 132, 196)
ANGER = (228, 76, 76)

_XF = [(1.0, 0.0, 0.0, 0.0, 1.0, 0.0)]  # 현재 변환 (x' = a x + b y + c, y' = d x + e y + f)
_VIEW = {"scale": 1.0, "lift": LIFT}  # 종 배율: 발 기준점 (64, 126)으로 축소


def _mul(m, n):
    a, b, c, d, e, f = m
    A, B, C, D, E_, F = n
    return (a * A + b * D, a * B + b * E_, a * C + b * F + c, d * A + e * D, d * B + e * E_, d * C + e * F + f)


@contextmanager
def xf(dx=0.0, dy=0.0, rot=0.0, sx=1.0, sy=1.0, pivot=(64, 126)):
    """국소 변환: pivot 기준으로 (sx, sy) 배율 → rot도(시계 방향) 회전 → (dx, dy) 이동."""
    px, py = pivot
    r = math.radians(rot)
    co, si = math.cos(r), math.sin(r)
    a, b, d, e = co * sx, -si * sy, si * sx, co * sy
    m = (a, b, px - a * px - b * py + dx, d, e, py - d * px - e * py + dy)
    _XF.append(_mul(_XF[-1], m))
    try:
        yield
    finally:
        _XF.pop()


@contextmanager
def view(scale=None, lift=None):
    old = dict(_VIEW)
    if scale is not None:
        _VIEW["scale"] = scale
    if lift is not None:
        _VIEW["lift"] = lift
    try:
        yield
    finally:
        _VIEW.update(old)


def _pt(x, y):
    a, b, c, d, e, f = _XF[-1]
    ux, uy = a * x + b * y + c, d * x + e * y + f
    s = _VIEW["scale"]
    return ((64 + (ux - 64) * s) * K, (126 + (uy - 126) * s - _VIEW["lift"]) * K)


def _wscale():
    a, b, _, d, e, _ = _XF[-1]
    return math.sqrt(abs(a * e - b * d)) * _VIEW["scale"] * K


def arc(cx, cy, rx, ry, a0, a1, n=14):
    """타원 호 위의 점들 (각도: 0=오른쪽, 90=아래)."""
    return [(cx + rx * math.cos(math.radians(a0 + (a1 - a0) * i / n)), cy + ry * math.sin(math.radians(a0 + (a1 - a0) * i / n)))
            for i in range(n + 1)]


def E(x0, y0, x1, y1):
    pts = [_pt(x, y) for x, y in arc((x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2, 0, 360, 72)[:-1]]
    return lambda d: d.polygon(pts, fill=255)


def C(cx, cy, r):
    return E(cx - r, cy - r, cx + r, cy + r)


def T(*pts):
    mp = [_pt(*p) for p in pts]
    return lambda d: d.polygon(mp, fill=255)


def L(pts, w):
    mp = [_pt(*p) for p in pts]
    wp = w * _wscale()

    def f(d):
        if len(mp) > 1:
            d.line(mp, fill=255, width=max(1, int(round(wp))), joint="curve")
        for x, y in (mp[0], mp[-1]):
            d.ellipse((x - wp / 2, y - wp / 2, x + wp / 2, y + wp / 2), fill=255)
    return f


def shape(desc):
    """종별 설정의 도형 기술 ('E', x0, y0, x1, y1) / ('T', p, ...) / ('L', [p, ...], w)을 현재 변환으로."""
    kind, *args = desc
    return {"E": E, "T": T, "L": L}[kind](*args)


def blank():
    return Image.new("L", (S, S), 0)


def shapes(fns):
    m = blank()
    d = ImageDraw.Draw(m)
    for f in fns:
        f(d)
    return m


def melt(m, r=2.4):
    """도형들을 말랑하게 한 덩어리로 녹인다."""
    return m.filter(ImageFilter.GaussianBlur(r * K)).point(lambda v: 255 if v >= 118 else 0)


def aa(m):
    return m.filter(ImageFilter.GaussianBlur(0.35 * K))


def grow(m, px):
    """정사각 팽창 (ImageFilter.MaxFilter(2r+1)과 같음) — 행·열로 나눠 numpy로 빠르게."""
    r = int(px * K)
    if r <= 0:
        return m
    a = np.asarray(m)
    for axis in (0, 1):
        p = np.pad(a, [(r, r) if ax == axis else (0, 0) for ax in (0, 1)])
        n = a.shape[axis]
        out = p.take(range(0, n), axis=axis)
        for k in range(1, 2 * r + 1):
            out = np.maximum(out, p.take(range(k, k + n), axis=axis))
        a = out
    return Image.fromarray(a)


def paint(img, mask, color, clip=None):
    if clip is not None:
        mask = ImageChops.multiply(mask, clip)
    img.paste(Image.new("RGBA", (S, S), color + (255,)), (0, 0), mask)


@dataclass
class Part:
    """실루엣 한 조각: 도형 + 바탕색 + 무늬. 앞에 그린 부품 위에 영역(grow)만큼 덧칠한다."""
    shapes: list
    base: tuple = None
    marks: list = field(default_factory=list)  # [(fns, color)] — 부품 영역 안으로 잘림
    grow: float = 2.0  # 녹인 틈(목·겨드랑이)까지 칠하려고 도형보다 넓게 칠하는 폭
    outline: bool = False  # 몸 앞에 겹친 조각(앞발·머리) — 자기 외곽선을 따로 그린다
    sharp: bool = False  # 녹이지 않음 (삐죽 털)


def compose(parts, ops=(), fx=(), clip=None):
    """parts(뒤→앞 순서)를 한 실루엣으로 녹여 외곽선을 두르고 칠한다. ops: 얼굴 등 위에 얹는 (fns, color)."""
    raw = [shapes(p.shapes) for p in parts]
    soft, sharp = blank(), blank()
    for p, m in zip(parts, raw):
        if p.sharp:
            sharp = ImageChops.lighter(sharp, m)
        else:
            soft = ImageChops.lighter(soft, m)
    sil = ImageChops.lighter(melt(soft), sharp)
    if clip is not None:
        sil = ImageChops.multiply(sil, clip)
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    paint(img, aa(grow(sil, 3.2)), LINE)
    paint(img, aa(sil), parts[0].base)
    for p, m in zip(parts, raw):
        if p.outline:
            m = melt(m, 1.0)
            paint(img, aa(grow(m, 1.3)), LINE, clip=sil)
            region = ImageChops.multiply(m, sil)
            if p.base:
                paint(img, aa(m), p.base, clip=sil)
        else:
            region = ImageChops.multiply(grow(m, p.grow) if p.grow else m, sil)
            if p.base:
                paint(img, region, p.base)
        for fns, color in p.marks:
            paint(img, aa(shapes(fns)), color, clip=region)
    for fns, color in ops:
        paint(img, aa(shapes(fns)), color)
    for fns, color, halo in fx:
        m = shapes(fns)
        if halo:
            paint(img, aa(grow(m, halo)), WHITE)
        paint(img, aa(m), color)
    out = img.resize((FRAME, FRAME), Image.LANCZOS)
    # LANCZOS 링잉이 남기는 거의 투명한 찌꺼기(알파 < 10)를 지워 1px 여백을 지킨다
    out.putalpha(out.getchannel("A").point(lambda v: 0 if v < 10 else v))
    return out


# =====================================================================
# 2. 얼굴·효과 (모두 현재 변환 아래에서 만든다 → 머리와 함께 움직인다)
# =====================================================================
def eye(kind, cx, cy, flip=False):
    if kind == "dot":
        return [([E(cx - 4, cy - 5, cx + 4, cy + 5)], EYE), ([E(cx - 2.2, cy - 3.5, cx, cy - 0.75)], WHITE)]
    if kind == "happy":  # ^
        return [([L(arc(cx, cy + 2.4, 4.6, 4.6, 200, 340), 2.0)], EYE)]
    if kind == "closed":  # ‿
        return [([L(arc(cx, cy - 1.5, 4.8, 3.6, 20, 160), 2.0)], EYE)]
    if kind == "round":
        return [([E(cx - 5.8, cy - 6.4, cx + 5.8, cy + 6.4)], LINE), ([E(cx - 4.5, cy - 5.1, cx + 4.5, cy + 5.1)], WHITE),
                ([E(cx - 2.4, cy - 2.4, cx + 2.4, cy + 2.4)], EYE)]
    if kind == "squint":
        return [([L([(cx - 4.6, cy + 1), (cx, cy + 0.2), (cx + 4.6, cy + 1)], 1.9)], EYE)]
    if kind == "tight":  # > <
        s = -1 if flip else 1
        return [([L([(cx - 4 * s, cy - 3.4), (cx + 3 * s, cy), (cx - 4 * s, cy + 3.4)], 1.9)], EYE)]
    raise ValueError(kind)


def blush(cx, cy):
    ops = [([E(cx - 7.5, cy - 5, cx + 7.5, cy + 5)], BLUSH)]
    for dx in (-3, 2):
        ops.append(([L([(cx + dx, cy + 2), (cx + dx + 2.4, cy - 2)], 1.2)], BLUSH_LINE))
    return ops


def open_mouth(cx, top, rx, ry, tongue=True):
    """D자 입 (위가 평평한 반타원)."""
    outer = arc(cx, top - 1, rx + 1.3, ry + 1.3, 0, 180, 16)
    inner = arc(cx, top, rx, ry, 0, 180, 16)
    ops = [([T(*outer)], LINE), ([T(*inner)], MOUTH_IN)]
    if tongue:
        ops.append(([E(cx - rx * 0.62, top + ry * 0.42, cx + rx * 0.62, top + ry * 1.0)], TONGUE))
    return ops


def heart_fx(cx, cy, r):
    return ([C(cx - r * 0.5, cy - r * 0.25, r * 0.56), C(cx + r * 0.5, cy - r * 0.25, r * 0.56),
             T((cx - r * 1.04, cy - 0.05 * r), (cx + r * 1.04, cy - 0.05 * r), (cx, cy + r * 0.95))], HEART, 1.4)


def question_fx(cx, cy):
    pts = arc(cx, cy - 4, 4.6, 4.4, 190, 395, 16) + [(cx, cy + 2.6)]
    return ([L(pts, 2.6), C(cx, cy + 7.2, 1.7)], LINE, 1.3)


def z_fx(cx, cy, s):
    return ([L([(cx - s, cy - s), (cx + s, cy - s), (cx - s, cy + s), (cx + s, cy + s)], 1.9)], ZZZ, 1.2)


def anger_fx(cx, cy, r=5.0):
    fns = []
    for q in (45, 135, 225, 315):
        a = math.radians(q)
        ox, oy = cx + math.cos(a) * r, cy + math.sin(a) * r
        fns.append(L(arc(ox, oy, r * 0.62, r * 0.62, q + 120, q + 240, 6), 1.9))
    return (fns, ANGER, 1.2)


# =====================================================================
# 3. 종별 설정 (시안 좌표 그대로)
#    앞모습 머리: 중심 (64, 50) · 몸 E(32,58,96,122) / 옆모습 머리: 중심 (88, 45) · 몸통 E(18,56,98,110)
# =====================================================================
@dataclass
class Ear:
    shapes: list
    pivot: tuple
    color: tuple
    marks: list = field(default_factory=list)


@dataclass
class Species:
    id: str
    name: str
    walk_speed: float
    scale: float
    base: tuple  # 몸 바탕
    back: tuple  # 등 색 (배 보이기·구르기에서 쓰임)
    belly: tuple  # 배 색
    nose: tuple
    mouth: str  # "w" | "dog" | "beaver"
    front_ears: list  # 머리 도형에 합쳐지는 귀
    front_head_marks: list  # [(descs, color)]
    front_body_marks: list
    front_tail: list
    front_tail_root: tuple
    front_tail_color: tuple
    side_ears: list  # [Ear]
    ears_over: bool  # 옆모습 귀가 머리 위에 덮이는지 (처진 귀)
    side_head_marks: list
    side_torso_marks: list
    side_tail: list  # 몸통 좌표의 꼬리 도형
    tail_root: tuple
    tail_color: tuple
    tail_marks: list = field(default_factory=list)
    wag_tail: list = field(default_factory=list)  # 앞모습 살랑 꼬리 (뿌리 기준)
    lie_ears: tuple = (-34, -30)  # 엎드렸을 때 귀를 뒤로 눕히는 각도
    specials: list = field(default_factory=list)


def _stripe(pts, w=2.8):
    return ("L", pts, w)


CAT_G, CAT_ST, CAT_W = (248, 190, 128), (234, 158, 96), (255, 250, 242)
_cat_tail_line = [(26, 78), (12, 64), (10, 46), (16, 36)]
CAT = Species(
    id="cat", name="고양이", walk_speed=90, scale=0.94, base=CAT_W, back=CAT_G, belly=CAT_W, nose=NOSE, mouth="w",
    front_ears=[("T", (20, 40), (24, 6), (52, 22)), ("T", (76, 22), (104, 6), (108, 40))],
    front_head_marks=[
        ([("E", -6, -20, 134, 44), ("E", 14, 20, 50, 52), ("E", 78, 20, 114, 52)], CAT_G),
        ([("T", (60, 50), (62, 16), (66, 16), (68, 50))], CAT_W),  # 이마 흰 줄
        ([("E", 79, 63, 91, 73)], CAT_G),  # 왼쪽 뺨 점
        ([("E", 30, 44, 58, 66), ("E", 70, 44, 98, 66)], CAT_W),
        ([("T", (28, 30), (29, 14), (44, 23)), ("T", (84, 23), (99, 14), (100, 30))], PINK),
        ([_stripe([(x, 19), (x + (x - 64) * 0.08, 28)]) for x in (50, 57, 71, 78)], CAT_ST),
    ],
    front_body_marks=[
        ([("E", 16, 68, 42, 112), ("E", 86, 68, 112, 112)], CAT_G),
        ([("E", 86, 98, 112, 122)], CAT_G),
        ([_stripe([(31, y), (37, y + 2)], 2.4) for y in (84, 95)] + [_stripe([(97, y), (91, y + 2)], 2.4) for y in (84, 95)], CAT_ST),
    ],
    front_tail=[("E", 90, 100, 108, 118)], front_tail_root=(94, 112), front_tail_color=CAT_G,
    side_ears=[Ear([("T", (58, 34), (64, 4), (84, 20))], (70, 26), CAT_G, [([("T", (65, 29), (66, 14), (77, 22))], PINK)]),
               Ear([("T", (94, 18), (112, 4), (118, 32))], (106, 24), CAT_G)],
    ears_over=False,
    side_head_marks=[
        ([("E", 40, -10, 120, 42), ("E", 44, 20, 76, 60)], CAT_G),
        ([("T", (110, 42), (115, 16), (128, 16), (128, 42))], CAT_W),
        ([_stripe([(x, 18), (x + 2, 27)]) for x in (62, 72, 82)], CAT_ST),
    ],
    side_torso_marks=[
        ([("E", 8, 46, 92, 80), ("E", 12, 70, 44, 104)], CAT_G),
        ([_stripe([(x, 58), (x + 2, 67)]) for x in (40, 52, 64)], CAT_ST),
    ],
    side_tail=[("L", _cat_tail_line, 10)], tail_root=(26, 78), tail_color=CAT_G,
    tail_marks=[([_stripe([(9, 58), (17, 58)]), _stripe([(10, 47), (18, 48)])], CAT_ST)],
    specials=["arch"],
)

DOG_W, DOG_G, DOG_EAR = (255, 247, 236), (222, 168, 112), (196, 140, 92)
DOG = Species(
    id="dog", name="강아지", walk_speed=105, scale=0.94, base=DOG_W, back=DOG_G, belly=DOG_W, nose=DARK_NOSE, mouth="dog",
    front_ears=[("E", 8, 30, 32, 76), ("E", 96, 30, 120, 76)],
    front_head_marks=[
        ([("E", 8, 30, 32, 76), ("E", 96, 30, 120, 76)], DOG_EAR),
        ([("E", 8, -6, 120, 44)], DOG_G),
        ([("E", 30, 46, 52, 60)], DOG_G),  # 눈가 점
        ([("L", [(31, 34), (28, 52), (31, 72)], 1.6), ("L", [(97, 34), (100, 52), (97, 72)], 1.6)], LINE),
    ],
    front_body_marks=[([("E", 16, 72, 40, 110), ("E", 88, 72, 112, 110)], DOG_G), ([("E", 90, 90, 112, 114)], DOG_G)],
    front_tail=[("E", 92, 92, 110, 112)], front_tail_root=(96, 108), front_tail_color=DOG_G,
    side_ears=[Ear([("E", 64, 22, 84, 58)], (74, 26), DOG_EAR)],
    ears_over=True,
    side_head_marks=[([("E", 40, -6, 120, 40)], DOG_G)],
    side_torso_marks=[([("E", 8, 48, 92, 80)], DOG_G)],
    side_tail=[("L", [(24, 70), (14, 56), (20, 44)], 11)], tail_root=(24, 70), tail_color=DOG_G,
    wag_tail=[("L", [(0, 0), (12, -10), (16, -24)], 10)],
    specials=["wag"],
)

RAB_W = (255, 252, 250)
RABBIT = Species(
    id="rabbit", name="토끼", walk_speed=120, scale=0.8, base=RAB_W, back=RAB_W, belly=RAB_W, nose=NOSE, mouth="w",
    front_ears=[("E", 30, -22, 50, 34), ("E", 78, -22, 98, 34)],
    front_head_marks=[([("E", 35, -14, 45, 28), ("E", 83, -14, 93, 28)], PINK)],
    front_body_marks=[],
    front_tail=[("E", 92, 100, 110, 118)], front_tail_root=(98, 110), front_tail_color=RAB_W,
    side_ears=[Ear([("E", 84, -18, 100, 30)], (92, 24), RAB_W),
               Ear([("E", 70, -21, 86, 30)], (78, 24), RAB_W, [([("E", 74, -13, 82, 24)], PINK)])],
    ears_over=False,
    side_head_marks=[],
    side_torso_marks=[],
    side_tail=[("E", 10, 64, 30, 84)], tail_root=(20, 74), tail_color=RAB_W,
    lie_ears=(-48, -42),
)

BEA_B, BEA_L, BEA_D, BEA_IN = (196, 140, 98), (240, 214, 180), (112, 76, 56), (160, 108, 74)
BEAVER = Species(
    id="beaver", name="비버", walk_speed=60, scale=0.94, base=BEA_B, back=BEA_B, belly=BEA_L, nose=DARK_NOSE, mouth="beaver",
    front_ears=[("E", 18, 18, 36, 36), ("E", 92, 18, 110, 36)],
    front_head_marks=[([("E", 22, 22, 32, 32), ("E", 96, 22, 106, 32)], BEA_IN), ([("E", 40, 54, 88, 82)], BEA_L)],
    front_body_marks=[([("E", 42, 76, 86, 122)], BEA_L)],
    front_tail=[("E", 86, 96, 118, 124)], front_tail_root=(96, 112), front_tail_color=BEA_D,
    side_ears=[Ear([("E", 72, 4, 90, 22)], (81, 18), BEA_B, [([("E", 76, 8, 86, 18)], BEA_IN)])],
    ears_over=False,
    side_head_marks=[([("E", 96, 40, 128, 74)], BEA_L)],
    side_torso_marks=[([("E", 30, 82, 92, 112)], BEA_L)],
    side_tail=[("E", 4, 84, 36, 118)], tail_root=(26, 96), tail_color=BEA_D,
    tail_marks=[([("L", [(12 + i * 8, 90), (15 + i * 8, 112)], 1.0) for i in range(3)], (90, 60, 44))],
)

SPECIES = {s.id: s for s in (CAT, DOG, RABBIT, BEAVER)}


# =====================================================================
# 4. 부품 — 종별 설정을 현재 변환 아래에서 Part로
# =====================================================================
def _marks(spec_marks):
    return [([shape(d) for d in descs], color) for descs, color in spec_marks]


# ---- 앞모습 ----
def front_head(sp, eyes="dot", mouth=None, tongue_dy=0, outline=False):
    """앞모습 머리(귀 포함) Part와 얼굴 ops. 머리 좌표: 중심 (64, 50)."""
    part = Part([shape(("E", 16, 16, 112, 84))] + [shape(d) for d in sp.front_ears], sp.base, _marks(sp.front_head_marks),
                grow=1, outline=outline)
    ops = eye(eyes, 44, 54) + eye(eyes, 84, 54, flip=True) + blush(33, 66) + blush(95, 66) + front_mouth(sp, mouth, tongue_dy)
    return part, ops


def front_mouth(sp, kind=None, tongue_dy=0):
    ops = []
    if sp.mouth == "dog":
        ops.append(([E(59, 56, 69, 63)], sp.nose))
        top = 66
    elif sp.mouth == "beaver":
        ops.append(([E(59, 57, 69, 63)], (110, 72, 62)))
        top = 65
    else:
        ops.append(([E(61.6, 59.4, 66.4, 62.6)], sp.nose))
        top = 64
    if kind is None:
        if sp.mouth == "dog":
            ops += [([L([(64, 63), (64, 66)], 1.3)], LINE), ([L([(59, 66), (62, 68.5), (64, 66), (66, 68.5), (69, 66)], 1.3)], LINE)]
        elif sp.mouth == "beaver":
            ops += [([L([(60, 64), (64, 66), (68, 64)], 1.3)], LINE),
                    ([T((60.5, 66), (67.5, 66), (67.5, 72), (60.5, 72))], WHITE),
                    ([L([(64, 66), (64, 72)], 0.9), L([(60.5, 72), (67.5, 72)], 0.9)], LINE)]
        else:
            ops.append(([L([(59.8, 64), (61.9, 65.8), (64, 64), (66.1, 65.8), (68.2, 64)], 1.3)], LINE))
        return ops
    rx, ry = {"open": (5.2, 6.2), "yawn": (6.4, 9.5), "tongue": (5.6, 5.0)}[kind]
    ops += open_mouth(64, top, rx, ry, tongue=kind != "tongue")
    if sp.mouth == "beaver":
        ops += [([T((60.6, top - 0.5), (67.4, top - 0.5), (67.4, top + 4.5), (60.6, top + 4.5))], WHITE),
                ([L([(64, top - 0.5), (64, top + 4.5)], 0.9)], LINE)]
    if kind == "tongue":
        t0 = top + 2 + tongue_dy
        ops += [([E(57.6, t0 - 1.3, 70.4, t0 + 13.3)], LINE), ([E(59, t0, 69, t0 + 12)], TONGUE),
                ([L([(64, t0 + 2), (64, t0 + 7)], 1.0)], (214, 104, 118))]
    return ops


def front_body(sp, leg_dy=(0, 0)):
    """앞모습 앉은 몸 + 앞발 두 개. 꼬리는 따로."""
    ld, rd = leg_dy
    fns = [shape(("E", 32, 58, 96, 122)), shape(("E", 36, 106 + ld, 60, 125 + ld)), shape(("E", 68, 106 + rd, 92, 125 + rd))]
    marks = _marks(sp.front_body_marks) + [([L([(64, 110), (64, 121)], 1.5)], LINE)]
    return Part(fns, sp.base, marks, grow=6)


def front_tail(sp, rot=0.0):
    with xf(rot=rot, pivot=sp.front_tail_root):
        return Part([shape(d) for d in sp.front_tail], sp.front_tail_color)


def front_arm(sp, side=1, rot=0.0, beans=True, outline=False):
    """들어 올린 앞발 (시안 wave 팔). side=1: 화면 오른쪽, -1: 왼쪽 (좌우 대칭). rot: 어깨 기준 회전."""
    mx = (lambda x: x) if side == 1 else (lambda x: 128 - x)
    with xf(rot=rot * side, pivot=(mx(92), 82)):
        part = Part([E(min(mx(90), mx(114)), 52, max(mx(90), mx(114)), 80)], sp.base, grow=1.5, outline=outline)
        ops = []
        if beans:
            for cx, cy, r in [(102, 66, 3.6), (97, 60.5, 1.6), (102, 59, 1.6), (107, 60.5, 1.6)]:
                ops.append(([E(mx(cx) - r, cy - r * 0.9, mx(cx) + r, cy + r * 0.9)], PINK))
    return part, ops


def front_paw(sp, cx, cy, w=15, h=13):
    """몸 앞에 겹친 작은 앞발 (자기 외곽선)."""
    return Part([E(cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2)], sp.base, outline=True)


# ---- 옆모습 (오른쪽을 본다) ----
def side_head(sp, eyes="dot", mouth=None, ear_rot=(0, 0), ear_dy=0, outline=False):
    """옆모습 머리 Parts(귀 포함, 그릴 순서)와 얼굴 ops. 머리 좌표: 중심 (88, 45)."""
    ears = []
    for i, ear in enumerate(sp.side_ears):
        with xf(dy=ear_dy, rot=ear_rot[min(i, len(ear_rot) - 1)], pivot=ear.pivot):
            ears.append(Part([shape(d) for d in ear.shapes], ear.color, _marks(ear.marks), grow=1, outline=outline and sp.ears_over))
    head = Part([shape(("E", 52, 12, 124, 78))], sp.base, _marks(sp.side_head_marks), grow=1, outline=outline)
    parts = [head] + ears if sp.ears_over else ears + [head]
    return parts, eye(eyes, 104, 46) + blush(99, 59) + side_mouth(sp, mouth)


def side_mouth(sp, kind=None):
    nose = [([E(118, 51, 125, 56)], sp.nose)]
    if kind is None:
        ops = nose + [([L([(121, 57), (119, 60), (115, 59)], 1.3)], LINE)]
        if sp.mouth == "beaver":
            ops.append(([T((114, 60), (119, 60), (119, 66), (114, 66))], WHITE))
        return ops
    if kind == "pout":
        return nose + [([L(arc(116.5, 63.5, 3.4, 2.6, 200, 340, 8), 1.5)], LINE)]
    if kind == "open":
        return nose + [([E(110.5, 57.5, 121, 68)], LINE), ([E(111.8, 58.8, 119.7, 66.7)], MOUTH_IN),
                       ([E(113, 63, 118.5, 66.4)], TONGUE)]
    if kind == "hiss":
        return [([E(104, 54, 123, 73)], LINE), ([E(105.6, 55.6, 121.4, 71.4)], MOUTH_IN), ([E(108, 64, 119, 71)], TONGUE),
                ([T((107.5, 56.5), (111.5, 56.5), (109.5, 62.5))], WHITE), ([T((114.5, 56.5), (118.5, 56.5), (116.5, 62.5))], WHITE),
                ([E(118, 49.5, 125, 54.5)], sp.nose)]
    raise ValueError(kind)


def side_torso(sp):
    return Part([shape(("E", 18, 56, 98, 110))], sp.base, _marks(sp.side_torso_marks), grow=6)


def side_tail(sp, rot=0.0, at=None, sx=1.0, sy=1.0, width=None):
    """꼬리를 뿌리 기준으로 돌리고 at(뿌리 위치, 프레임 좌표)으로 옮긴다."""
    rx, ry = sp.tail_root
    dx, dy = (at[0] - rx, at[1] - ry) if at else (0, 0)
    with xf(dx=dx, dy=dy, rot=rot, sx=sx, sy=sy, pivot=sp.tail_root):
        descs = sp.side_tail if width is None else [(k, pts, width) if k == "L" else (k, pts, *rest) for k, pts, *rest in sp.side_tail]
        return Part([shape(d) for d in descs], sp.tail_color, _marks(sp.tail_marks))


def legs(sp, specs, outline=False):
    """다리: (x, top, bottom, angle[, w]) — angle>0이면 발끝이 앞(오른쪽)으로."""
    fns = []
    for x, top, bottom, angle, *w in specs:
        w = w[0] if w else 16.5
        with xf(rot=-angle, pivot=(x, top + 4)):
            fns.append(E(x - w / 2, top, x + w / 2, bottom))
    return Part(fns, sp.base, outline=outline)


# =====================================================================
# 5. 포즈 — (species, frame) -> 128×128 RGBA. 같은 뼈대를 종별 설정으로 칠한다
# =====================================================================
WALK_LEGS = lambda sw: [(37 - sw, 96, 124, 0, 18), (54 + sw, 98, 125, 0, 16), (78 - sw, 98, 125, 0, 16), (92 + sw, 96, 124, 0, 16)]


def _side(sp, *, head=None, torso=None, tail=None, leg_specs=(), head_kw=None, whole=None, before=(), after=(), fx=(), ops=()):
    """옆모습 공통 조립: 다리 → 몸통 → 꼬리 → 머리 → 덧붙임. 인자는 각 부품의 xf 인자."""
    with xf(**(whole or {})):
        parts = list(before)
        if leg_specs:
            parts.append(legs(sp, leg_specs))
        with xf(**(torso or {})):
            parts.append(side_torso(sp))
        parts.append(side_tail(sp, **(tail or {})))
        with xf(**(head or {})):
            hp, face = side_head(sp, **(head_kw or {}))
        parts += hp
        parts += list(after)
    return compose(parts, face + list(ops), fx)


def walk(sp, i):
    sw, lift = [4, 0, -4, 0][i], [0, 1.5, 0, 1.5][i]
    return _side(sp, head=dict(dy=-lift), torso=dict(dy=-lift), tail=dict(at=(sp.tail_root[0], sp.tail_root[1] - lift)),
                 leg_specs=WALK_LEGS(sw))


def run(sp, i):
    """질주: 0 쭉 뻗음 · 1 모음 · 2 뻗음(반대 위상) · 3 모음. 몸을 앞으로 숙이고 귀는 뒤로."""
    k = 1 if i < 2 else -1
    if i % 2 == 0:
        lg = [(32, 92, 124, -56 - 4 * k, 15), (44, 94, 125, -44 + 4 * k, 15), (86, 92, 122, 52 + 4 * k, 15), (96, 90, 120, 62 - 4 * k, 15)]
        bob = -3
    else:
        lg = [(50, 94, 124, 30 + 3 * k, 15), (58, 96, 125, 20 - 3 * k, 15), (70, 94, 125, -22 + 3 * k, 15), (78, 92, 124, -32 - 3 * k, 15)]
        bob = 1
    return _side(sp, whole=dict(dx=1), head=dict(dx=-4, dy=3 + bob, rot=3, pivot=(82, 72)), torso=dict(dy=bob, sx=1.06, rot=5, pivot=(58, 83)),
                 tail=dict(at=(sp.tail_root[0] + 2, sp.tail_root[1] + bob + 2), rot=-12 + 6 * (i % 2)), leg_specs=lg,
                 head_kw=dict(ear_rot=(-28, -22)))


SIT_TORSO = dict(rot=-38, pivot=(58, 83), dx=-2, dy=7)
SIT_LEGS = [(44, 90, 122, 0, 34), (70, 80, 123, 2, 16), (84, 80, 123, -2, 16)]


def _sit_side(sp, head=None, breathe=0, head_kw=None):
    """옆모습 앉기: 엉덩이 내리고 앞발 세움, 꼬리는 바닥에서 앞으로 감음."""
    hd = dict(dx=-6, dy=-1 + breathe, pivot=(82, 72))
    hd.update(head or {})
    return _side(sp, head=hd, torso=dict(SIT_TORSO, sy=1 + 0.02 * breathe), tail=dict(at=(34, 112), rot=96, sy=0.8),
                 leg_specs=SIT_LEGS, head_kw=head_kw)


def idle(sp, i):
    return _sit_side(sp, breathe=i)


def look(sp, i):
    rot, dy = [(-12, -1), (0, 0), (12, 2), (0, 0)][i]
    return _sit_side(sp, head=dict(dy=-1 + dy, rot=rot), head_kw=dict(eyes="closed" if i == 3 else "dot"))


def _lying(sp, i, eyes, mouth, ear_rot, fx):
    """엎드림: 배를 바닥에 깔고 앞발 위에 턱. 머리는 말랑하게 눌린 모양(0.76)."""
    paws = [Part([E(86, 112, 112, 125.5)], sp.base, outline=True), Part([E(98, 112.5, 125, 125.5)], sp.base, outline=True)]
    return _side(sp, torso=dict(sx=1.04, sy=0.66 + 0.012 * i, pivot=(58, 110), dy=14.5, dx=-6),
                 tail=dict(at=(26, 113), rot=100, sy=0.8),
                 head=dict(sx=0.98, sy=0.76, pivot=(88, 78), dy=46 - 0.3 * i, dx=-2),
                 head_kw=dict(eyes=eyes, mouth=mouth, ear_rot=ear_rot, ear_dy=4),
                 leg_specs=[(32, 108, 125, 30, 22)], after=paws, fx=fx)


def sleep(sp, i):
    zs = [z_fx(106, 66, 3.4), z_fx(117, 55, 2.6)] if i == 0 else [z_fx(108, 63, 3.6), z_fx(120, 51, 2.8)]
    return _lying(sp, i, "closed", None, sp.lie_ears, zs)


def annoyed(sp, i):
    a, b = sp.lie_ears
    return _lying(sp, i, "squint", "pout", (a, b - 26) if i == 0 else (a, b - 12), [anger_fx(116, 74 - i, 7.5)])


def hop(sp, i):
    if i == 0:  # 웅크림
        return _side(sp, head=dict(dy=9, dx=-2), torso=dict(dy=8, sy=0.92, pivot=(58, 110)), tail=dict(at=(sp.tail_root[0], sp.tail_root[1] + 5)),
                     leg_specs=[(36, 104, 124, -12, 18), (54, 106, 125, -6), (76, 104, 125, 10), (90, 102, 124, 14)],
                     head_kw=dict(eyes="tight"))
    if i == 1:  # 쭉 뻗음: 뒷발 뒤로 차고 앞발 앞으로
        return _side(sp, whole=dict(rot=-6, pivot=(96, 40), dy=-3, dx=1), tail=dict(rot=0),
                     leg_specs=[(34, 96, 126, -52, 16), (48, 98, 126, -44, 15), (82, 96, 124, 42, 15), (94, 94, 122, 52, 16)],
                     head_kw=dict(eyes="happy", mouth="open"))
    return _side(sp, whole=dict(dy=-3, dx=2), tail=dict(rot=0),  # 다리 모음
                 leg_specs=[(40, 96, 116, 34, 18), (54, 98, 116, 22), (74, 98, 116, -22), (88, 96, 114, -34)],
                 head_kw=dict(eyes="happy"))


def fall(sp, i):
    w = 10 if i else -10
    return _side(sp, whole=dict(rot=-10, pivot=(64, 80), dy=2), tail=dict(rot=28 + w),
                 leg_specs=[(30, 90, 116, -96 - w, 15), (44, 98, 122, -62 + w, 15), (80, 98, 122, 58 - w, 15), (94, 90, 116, 96 + w, 15)],
                 head_kw=dict(eyes="round", mouth="open", ear_rot=(10, 10)))


def dragged(sp, i):
    """목덜미 잡혀 대롱대롱: 몸이 머리 아래로 늘어지고 다리는 축 처짐."""
    sway = [-1, 1][i]
    return _side(sp, whole=dict(rot=4 * sway, sx=0.84, sy=0.84, pivot=(74, 10), dx=2 * sway + 4, dy=2),
                 torso=dict(rot=80, sx=0.78, pivot=(58, 83), dx=10, dy=6),
                 tail=dict(at=(52, 104), rot=150 + 10 * sway, sy=0.72),
                 head=dict(dx=-12, dy=-8, pivot=(82, 72)),
                 leg_specs=[(60, 96, 118, 6 * sway, 14), (74, 96, 120, -6 * sway, 14), (58, 110, 126, 8 * sway, 15), (72, 112, 127, -8 * sway, 15)],
                 head_kw=dict(eyes="closed", ear_rot=(-6, -6)))


def land(sp, i):
    if i == 0:
        return _side(sp, whole=dict(sx=1.05, sy=0.84, pivot=(60, 126)), leg_specs=WALK_LEGS(2), head_kw=dict(eyes="tight"))
    return _side(sp, whole=dict(sx=1.02, sy=0.96, pivot=(60, 126)), leg_specs=WALK_LEGS(2))


def arch(sp, i):
    """하악질: 뒤집힌 U자로 높이 휜 등(아래로 빈 틈), 꼿꼿한 다리, 펑 부푼 꼬리, 등털 삐죽, 귀 젖힘, 송곳니, 동그란 눈."""
    puff = [0.55, 1.0, 1.0][i]
    cx, cy, rx, ry = 46, 98, 28, 46 + 12 * puff
    band = lambda w, dy=0: L(arc(cx, cy + dy, rx, ry, 180, 360, 24), w)
    with xf(dy=-1 if i == 2 else 0):
        stripes = [L([(cx + rx * math.cos(math.radians(a)) * 1.0, cy + ry * math.sin(math.radians(a)) - 10),
                      (cx + rx * math.cos(math.radians(a)) * 1.0, cy + ry * math.sin(math.radians(a)) - 2)], 2.8) for a in (235, 260, 285)]
        body = Part([band(26)], sp.base, [([band(20, -6)], sp.back), (stripes, CAT_ST)], grow=4)
        spikes = []
        for t in range(6):
            a = math.radians(208 + t * 22)
            ox, oy = (rx + 12) * math.cos(a), (ry + 12) * math.sin(a)
            bx, by = cx + ox, cy + oy
            nx, ny = math.cos(a) / (rx + 12), math.sin(a) / (ry + 12)
            nn = math.hypot(nx, ny)
            nx, ny = nx / nn, ny / nn
            ln = 2.5 + 4.5 * puff
            spikes.append(T((bx - ny * 3.6, by + nx * 3.6), (bx + nx * ln, by + ny * ln), (bx + ny * 3.6, by - nx * 3.6)))
        spike_part = Part(spikes, sp.back, sharp=True, grow=1)
        # 펑 부푼 꼬리: 줄기 둘레에 동그라미를 엇갈려 붙여 울퉁불퉁한 털 뭉치로
        r = 5 + 3.5 * puff
        stem = [(25, 66), (15, 50), (12, 34), (15, 20 + 4 * (1 - puff))]
        puffs = [C(x + 3 + (2 if k % 2 else -2), y, r) for k, (x, y) in enumerate([
            (22, 64), (16, 55), (11, 46), (9, 37), (10, 28), (12, 21 + 4 * (1 - puff))])]
        tail = Part(puffs + [L(stem, 1.6 * r)], sp.tail_color, [([L([(5, 46), (23, 44)], 3), L([(3, 32), (21, 31)], 3)], CAT_ST)], outline=True)
        leg_part = legs(sp, [(cx - rx - 2, 92, 124, -4, 16), (cx - rx + 10, 94, 125, 4, 15), (cx + rx - 6, 94, 125, -4, 15), (cx + rx + 4, 92, 124, 4, 16)])
        with xf(dx=0, dy=20, pivot=(82, 72)):
            hp, face = side_head(sp, eyes="round", mouth="hiss", ear_rot=(-34, -28), ear_dy=2)
    return compose([leg_part, spike_part, body, tail] + hp, face)


# ---- 앞모습 포즈 ----
def _front(sp, *, head=None, body=None, whole=None, tail_rot=0.0, eyes="dot", mouth=None, tongue_dy=0, leg_dy=(0, 0),
           arms=(), paws=(), held=None, tail_desc=None, fx=()):
    """앞모습 공통 조립: 몸 → 꼬리 → 머리 → 팔·앞발."""
    with xf(**(whole or {})):
        with xf(**(body or {})):
            parts = [front_body(sp, leg_dy)]
            if tail_desc is not None:
                rx, ry = sp.front_tail_root
                with xf(dx=rx, dy=ry, rot=tail_rot, pivot=(0, 0)):
                    parts.append(Part([shape(d) for d in tail_desc], sp.front_tail_color))
            else:
                parts.append(front_tail(sp, tail_rot))
        with xf(**(head or {})):
            hp, face = front_head(sp, eyes, mouth, tongue_dy)
        parts.append(hp)
        ops = list(face)
        with xf(**(body or {})):
            if held:
                parts.append(held())
            for side, rot in arms:
                ap, aops = front_arm(sp, side, rot)
                parts.append(ap)
                ops += aops
            for cx, cy in paws:
                parts.append(front_paw(sp, cx, cy))
    return compose(parts, ops, fx)


def sit_front(sp, i):
    return _front(sp, eyes="closed" if i == 3 else "dot", tail_rot=[0, -6, 0, 0][i], head=dict(dy=[0, 0.6, 0, 0][i]))


def wave(sp, i):
    return _front(sp, arms=[(1, [0, 26, 0, 26][i])], head=dict(rot=[-3, -1, -3, -1][i], pivot=(64, 84)))


def tilt(sp, i):
    deg = [8, 12, 12][i]
    return _front(sp, head=dict(rot=deg, pivot=(64, 84), dy=2), fx=[question_fx(18, 26 + [2, 0, -2][i])])


def belly(sp, i):
    """등을 대고 누워 배 보이기: 몸은 바닥에 눕고 네 발은 위로 꼬물, 얼굴은 정면."""
    wig = [0, 2, 0, -2][i]
    torso = Part([E(44, 84, 122, 124)], sp.back, [([E(46, 78, 120, 114)], sp.belly)], grow=6)
    up = []
    beans = []
    for k, (x, top) in enumerate([(62, 58), (76, 56), (98, 60), (112, 64)]):
        dy = wig if k % 2 == 0 else -wig
        up.append(E(x - 7, top + dy, x + 7, 100))
        beans.append(([E(x - 3.4, top + dy + 3, x + 3.4, top + dy + 8)], PINK))
    paws = Part(up, sp.base, grow=1.5)
    tail = Part([E(106, 108, 124, 124)], sp.front_tail_color)
    with xf(dx=-15, dy=42, rot=-10, pivot=(64, 50), sx=0.8, sy=0.8):
        hp, face = front_head(sp, "happy", "open", outline=True)
    return compose([torso, tail, paws, hp], face + beans)


def stretch(sp, i):
    if i == 0:  # 엎드려 기지개: 엉덩이 들고 앞발 쭉 (정면)
        rump = Part([E(36, 49, 92, 98)], sp.back, [], grow=6)
        tail = side_tail(sp, rot=40, at=(80, 58))  # 엉덩이 위로 바짝 세운 꼬리
        pl = Part([E(18, 104, 52, 125)], sp.base, outline=True)
        pr = Part([E(76, 104, 110, 125)], sp.base, outline=True)
        with xf(dy=34, pivot=(64, 84)):
            hp, face = front_head(sp, "tight", None, outline=True)
        return compose([rump, tail, hp, pl, pr], face)
    if i == 1:  # 하품
        return _front(sp, eyes="tight", mouth="yawn", head=dict(dy=-1, rot=-4, pivot=(64, 84)))
    return _front(sp)


def heart(sp, i):
    """양발로 가슴 앞에 하트를 꼭 쥐고, 작은 하트가 머리 옆으로 떠오른다."""
    held = lambda: Part([C(57.5, 90, 7.2), C(70.5, 90, 7.2), T((50.8, 92), (77.2, 92), (64, 105))], HEART, outline=True)
    hs = [heart_fx(111, 44 - 6 * i, 4.6 + 0.6 * i), heart_fx(17, 50 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(116, 22 - 3 * i, 2.6 + 0.6 * i))
    return _front(sp, eyes="happy", held=held, paws=[(48, 94), (80, 94)], fx=hs)


def tickle(sp, i):
    dx, rot = [(-2, -4), (2, 4), (0, 0)][i]
    return _front(sp, whole=dict(dx=dx, dy=-1, rot=rot, pivot=(64, 124)), eyes="happy", mouth="open",
                  arms=[(1, [-6, 18, 6][i]), (-1, [18, -6, 6][i])], leg_dy=[(-3, 0), (0, -3), (-2, -2)][i])


def wag(sp, i):
    rot = [-25, -5, 15, -5][i]
    return _front(sp, eyes="happy", mouth="tongue", tongue_dy=[0, 1, 0, 1][i], body=dict(dy=[0, -1, 0, -1][i]),
                  head=dict(dy=[0, -1, 0, -1][i]), tail_desc=sp.wag_tail, tail_rot=rot)


def roll(sp, i):
    """공처럼 만 몸: 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안. 꼬리가 아래를 감싸고 얼굴은 ^^."""
    cx, cy = ROLL_CENTER
    r = (ROLL_RADIUS - 4.4) * (0.86 if sp.scale < 0.9 else 1.0)
    with view(scale=1.0, lift=0):
        ball = Part([C(cx, cy, r)], sp.back, [([C(cx + r * 0.1, cy + r * 0.25, r * 0.8)], sp.belly)], grow=0)
        tail = Part([L(arc(cx, cy, r * 0.9, r * 0.9, 120, 215, 12), 13)], sp.tail_color, outline=True)
        paws = [Part([E(cx - 15, cy + 15, cx - 3, cy + 25)], sp.base, outline=True), Part([E(cx + 3, cy + 15, cx + 15, cy + 25)], sp.base, outline=True)]
        s = r / 34 * 0.6
        with xf(dx=cx - 64, dy=cy - 6 - 50, sx=s, sy=s, pivot=(64, 50)):
            hp, face = front_head(sp, "happy", None, outline=True)
        if i == 1:  # 두 번째 프레임은 볼이 더 발그레
            with xf(dx=cx - 64, dy=cy - 6 - 50, sx=s, sy=s, pivot=(64, 50)):
                face = face + [([E(22, 60, 44, 72), E(84, 60, 106, 72)], BLUSH)] + blush(33, 66)[1:] + blush(95, 66)[1:]
        clip = shapes([C(cx, cy, r)])
    return compose([ball, tail, hp] + paws, face, clip=clip)


POSES = {"walk": walk, "run": run, "roll": roll, "idle": idle, "look": look, "sleep": sleep, "hop": hop, "fall": fall,
         "dragged": dragged, "land": land, "sit_front": sit_front, "wave": wave, "tilt": tilt, "belly": belly,
         "stretch": stretch, "heart": heart, "tickle": tickle, "annoyed": annoyed, "arch": arch, "wag": wag}


def frames(sp, anim, n):
    with view(scale=sp.scale):
        return [POSES[anim](sp, i) for i in range(n)]


# =====================================================================
# 6. 출력
# =====================================================================
def rows_for(sp):
    return list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in sp.specials]


def render_species(sp):
    return {anim: frames(sp, anim, n) for anim, n, *_ in rows_for(sp)}


def write_species(sp, out_dir, rendered=None):
    rendered = rendered or render_species(sp)
    rows = rows_for(sp)
    cols = max(n for _, n, *_ in rows)
    sheet = Image.new("RGBA", (cols * FRAME, len(rows) * FRAME), (0, 0, 0, 0))
    for r, (anim, *_) in enumerate(rows):
        for c, im in enumerate(rendered[anim]):
            sheet.paste(im, (c * FRAME, r * FRAME))
    out_dir.mkdir(parents=True, exist_ok=True)
    sheet.save(out_dir / "sheet.png")
    counts = {anim: len(rendered[anim]) for anim, *_ in rows}
    m = manifest(sp.name, sp.walk_speed, counts, SPEECH[sp.id])
    (out_dir / "pet.json").write_text(json.dumps(m, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return rendered


def build(out_root):
    """모든 종의 sheet.png + pet.json을 out_root/<id>/에 쓴다. {id: {anim: [frames]}} 반환."""
    out_root = Path(out_root)
    return {pid: write_species(sp, out_root / pid) for pid, sp in SPECIES.items()}


def preview(all_frames, path, bg):
    from PIL import ImageFont
    names = [a for a, *_ in ANIMS] + [o[0] for o in OPTIONAL_ANIMS]
    cell, gap, label = FRAME + 4, 24, 70
    block = 4 * cell
    W = label + len(SPECIES) * (block + gap)
    H = 30 + len(names) * cell
    img = Image.new("RGBA", (W, H), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    fg = (60, 60, 60) if sum(bg) > 380 else (220, 220, 220)
    for si, pid in enumerate(SPECIES):
        d.text((label + si * (block + gap), 8), pid, fill=fg, font=font)
    for r, anim in enumerate(names):
        d.text((4, 30 + r * cell + FRAME // 2), anim, fill=fg, font=font)
        for si, pid in enumerate(SPECIES):
            for c, im in enumerate(all_frames[pid].get(anim, [])):
                img.alpha_composite(im, (label + si * (block + gap) + c * cell, 30 + r * cell))
    img.save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    all_frames = build(root / "ui" / "pets")
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    preview(all_frames, pv / "all_light.png", (255, 255, 255))
    preview(all_frames, pv / "all_dark.png", (40, 44, 52))
    print("ok:", ", ".join(SPECIES))


if __name__ == "__main__":
    main()
