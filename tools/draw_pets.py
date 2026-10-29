"""카와이 스프라이트 생성기 — 4종 × 21동작 (+ 고양이 arch·huff, 강아지 wag, 고양이·곰 뒷모습 climb·slide) × 성장 5단계, 128px.

    python tools/draw_pets.py  → ui/pets/<id>/{sheet_<stage>.png ×5, sheet.png (= teenager), pet.json}
                                 + tools/preview/{all_light,all_dark,stages_<id>,climb_<id>}.png

구성:
  1. 엔진      — 4배 슈퍼샘플 마스크, 변환 스택(xf), 도형(E/T/L), 녹이기(melt)로 한 덩어리 실루엣 + 굵은 외곽선
  2. 얼굴·효과 — 눈/입/볼터치, 하트·물음표·z·💢
  3. 종별 설정 — 색과 무늬(데이터). 앞모습/옆모습 머리·몸·꼬리·귀를 표준 좌표계로 적는다
  4. 부품      — 종별 설정을 현재 변환 아래에서 Part로 만든다
  5. 포즈      — 동작별 공통 뼈대. 부품을 옮기고 돌려 배치할 뿐 종별 분기는 없다.
                 성장 단계(STAGE_PROPS)는 비율·눈·귀·꼬리·색·무늬 단계로 모든 포즈에 똑같이 적용된다 (_side/_front/_back/부품)
  6. 출력      — 시트·매니페스트·미리보기
"""

import json
import math
import shutil
import sys
from contextlib import contextmanager
from dataclasses import dataclass, field, fields, replace
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sprite_spec import (ANIMS, FAVORITE_FOOD, FRAME, OPTIONAL_ANIMS, PERSONALITY, ROLL_CENTER, ROLL_RADIUS, SPEECH, STAGE_SCALE,  # noqa: E402
                         STAGES, manifest)

# =====================================================================
# 1. 엔진
# =====================================================================
K = 4  # 슈퍼샘플 배율
S = FRAME * K
LIFT = 2  # 발끝 외곽선이 프레임 맨 아래 1px 투명 여백 안에 들어오게 전체를 올린다
OUTLINE = 2.3  # 실루엣 외곽선 굵기 (128 프레임, teenager) — 슈퍼샘플에서 정수로 내림되어 약 2.25px
PART_OUTLINE = 1.0  # 몸 앞에 겹친 조각(앞발·머리)의 자기 외곽선

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


def _xf_matrix(dx=0.0, dy=0.0, rot=0.0, sx=1.0, sy=1.0, pivot=(64, 126)):
    px, py = pivot
    r = math.radians(rot)
    co, si = math.cos(r), math.sin(r)
    a, b, d, e = co * sx, -si * sy, si * sx, co * sy
    return (a, b, px - a * px - b * py + dx, d, e, py - d * px - e * py + dy)


def _apply(m, p):
    a, b, c, d, e, f = m
    return (a * p[0] + b * p[1] + c, d * p[0] + e * p[1] + f)


def _inv(m):
    a, b, c, d, e, f = m
    det = a * e - b * d
    ia, ib, id_, ie = e / det, -b / det, -d / det, a / det
    return (ia, ib, -(ia * c + ib * f), id_, ie, -(id_ * c + ie * f))


@contextmanager
def xf(dx=0.0, dy=0.0, rot=0.0, sx=1.0, sy=1.0, pivot=(64, 126)):
    """국소 변환: pivot 기준으로 (sx, sy) 배율 → rot도(시계 방향) 회전 → (dx, dy) 이동."""
    _XF.append(_mul(_XF[-1], _xf_matrix(dx, dy, rot, sx, sy, pivot)))
    try:
        yield
    finally:
        _XF.pop()


# ---- 성장 단계별 디자인 ----
# 비율 — head: 머리 배율 · body: 몸 배율 · leg: 다리 길이 · fit: 프레임에 들어오게 전체를 줄이는 배율 (종 배율에 곱함)
#        torso: 옆모습 몸통 (가로, 세로) 모양 · fbody: 앞모습 몸 (가로, 세로) 모양 (아기 = 동글 찹쌀떡, 어른 = 길쭉)
# 선   — line: 외곽선 굵기 배율 (모든 단계 거의 같게) · soft: 외곽선 색을 몸 색 쪽으로 섞는 비율 (어릴수록 부드럽게)
# 색   — pale: 몸 색을 크림색 쪽으로 섞는 비율 (어릴수록 파스텔)
# 무늬 — lvl: 무늬 단계 (0 아기 … 4 어른). 종별 무늬는 (도형, 색, 최소 lvl | (최소, 최대))로 단계마다 늘어난다
# 얼굴 — eye: 눈 크기 · eye_dy: 눈 높이 · sparkle: 반짝이 두 개 큰 눈 · calm: 반쯤 감은 느긋한 눈 · smile: 작게 벌린 웃는 입
#        blush: 볼터치 (가로, 세로) 배율 · ear: 귀 크기 · tuft: 정수리 삐죽 털
# 몸   — tail: 꼬리 길이 · tail_w: 꼬리 굵기 · cheek_fluff: 볼 지그재그 털
# 종 특징 (Species.stage_tweak로 켠다) — whiskers: 수염 · snout: 주둥이 길이 · chest_fluff: 가슴 털
STAGE_PROPS = {
    "baby": dict(head=1.25, body=0.80, leg=0.45, line=1.1, soft=0.22, fit=0.84, torso=(0.9, 1.14), fbody=(1.12, 1.0),
                 pale=0.35, lvl=0, eye=1.5, eye_dy=2.5, sparkle=True, calm=False, smile=False, blush=(1.25, 1.45), ear=0.66,
                 tuft=True, tail=0.5, tail_w=1.15, cheek_fluff=False),
    "kinder": dict(head=1.15, body=0.88, leg=0.66, line=1.05, soft=0.14, fit=0.91, torso=(0.94, 1.08), fbody=(1.07, 1.0),
                   pale=0.20, lvl=1, eye=1.3, eye_dy=1.5, sparkle=True, calm=False, smile=False, blush=(1.15, 1.3), ear=0.8,
                   tuft=False, tail=0.72, tail_w=1.05, cheek_fluff=False),
    "child": dict(head=1.07, body=0.94, leg=0.86, line=1.0, soft=0.0, fit=0.96, torso=(0.98, 1.02), fbody=(1.0, 1.0),
                  pale=0.08, lvl=2, eye=1.12, eye_dy=0.5, sparkle=False, calm=False, smile=True, blush=(1.05, 1.1), ear=0.92,
                  tuft=False, tail=0.88, tail_w=1.0, cheek_fluff=False),
    "teenager": dict(head=1.0, body=1.0, leg=1.0, line=1.0, soft=0.0, fit=1.0, torso=(1.0, 1.0), fbody=(1.0, 1.0),
                     pale=0.0, lvl=3, eye=1.0, eye_dy=0.0, sparkle=False, calm=False, smile=False, blush=(1.0, 1.0), ear=1.0,
                     tuft=False, tail=1.0, tail_w=1.0, cheek_fluff=False),
    "adult": dict(head=0.93, body=1.06, leg=1.12, line=1.0, soft=0.0, fit=0.93, torso=(1.06, 0.97), fbody=(1.0, 1.0),
                  pale=0.0, lvl=4, eye=0.92, eye_dy=0.5, sparkle=False, calm=True, smile=False, blush=(0.95, 0.9), ear=1.0,
                  tuft=False, tail=1.0, tail_w=1.22, cheek_fluff=True),
}
STAGE_FLAGS = dict(whiskers=False, snout=0.0, chest_fluff=False)  # 종 특징 기본값 (Species.stage_tweak로 덮어씀)
_ST = dict(STAGE_PROPS["teenager"], **STAGE_FLAGS)


@contextmanager
def stage(name, tweak=None):
    old = dict(_ST)
    _ST.update(STAGE_FLAGS)
    _ST.update(STAGE_PROPS[name])
    _ST.update(tweak or {})
    try:
        yield
    finally:
        _ST.clear()
        _ST.update(old)


CREAM = (255, 250, 242)


def mix(c, d, t):
    return tuple(round(a + (b - a) * t) for a, b in zip(c, d))


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
    spikes: list = field(default_factory=list)  # 녹이지 않는 삐죽 조각 (정수리 털·볼 털) — 부품 영역에 포함


def compose(parts, ops=(), fx=(), clip=None):
    """parts(뒤→앞 순서)를 한 실루엣으로 녹여 외곽선을 두르고 칠한다. ops: 얼굴 등 위에 얹는 (fns, color)."""
    raw = [shapes(p.shapes) for p in parts]
    spk = [shapes(p.spikes) if p.spikes else None for p in parts]
    soft, sharp = blank(), blank()
    for p, m, k in zip(parts, raw, spk):
        if p.sharp:
            sharp = ImageChops.lighter(sharp, m)
        else:
            soft = ImageChops.lighter(soft, m)
        if k is not None:
            sharp = ImageChops.lighter(sharp, k)
    sil = ImageChops.lighter(melt(soft), sharp)
    if clip is not None:
        sil = ImageChops.multiply(sil, clip)
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    line = mix(LINE, parts[0].base, _ST["soft"])  # 어린 단계는 외곽선을 몸 색 쪽으로 부드럽게
    paint(img, aa(grow(sil, OUTLINE * _ST["line"])), line)
    paint(img, aa(sil), parts[0].base)
    for p, m, k in zip(parts, raw, spk):
        if p.outline:
            m = melt(m, 1.0)
            if k is not None:
                m = ImageChops.lighter(m, k)
            paint(img, aa(grow(m, PART_OUTLINE * _ST["line"])), mix(LINE, p.base or parts[0].base, _ST["soft"]), clip=sil)
            region = ImageChops.multiply(m, sil)
            if p.base:
                paint(img, aa(m), p.base, clip=sil)
        else:
            if k is not None:
                m = ImageChops.lighter(m, k)
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
    if kind == "flat":  # 반쯤 감은 시큰둥한 눈 (평평한 눈꺼풀 + 아래 반달 눈동자)
        return [([T(*arc(cx, cy - 0.6, 4.6, 4.4, 0, 180, 12))], EYE), ([L([(cx - 5.6, cy - 0.8), (cx + 5.6, cy - 0.8)], 2.0)], EYE)]
    if kind == "big":  # 큰 눈동자 + 반짝이 두 개
        return [([E(cx - 5, cy - 6, cx + 5, cy + 6)], EYE), ([E(cx - 3, cy - 4.4, cx - 0.2, cy - 1.2)], WHITE),
                ([C(cx + 2, cy + 2.6, 1.0)], WHITE)]
    raise ValueError(kind)


EYE_LIGHT = (138, 96, 92)  # 반짝 눈의 아래쪽 밝은 홍채


def stage_eye(kind, cx, cy, flip=False):
    """단계 디자인을 입힌 눈: 기본 눈("dot")은 아기·유아 = 반짝이 두 개 큰 눈, 어른 = 반쯤 감은 느긋한 눈.
    다른 표정 눈(^^, ‿ …)은 단계 눈 크기에 맞춰 조금 키우거나 줄인다."""
    es = _ST["eye"]
    cy += _ST["eye_dy"]
    if kind == "dot" and _ST["sparkle"]:
        rx, ry = 4 * es, 5 * es
        return [([E(cx - rx, cy - ry, cx + rx, cy + ry)], EYE),
                ([E(cx - rx * 0.74, cy + ry * 0.08, cx + rx * 0.74, cy + ry * 0.88)], EYE_LIGHT),
                ([E(cx - rx * 0.8, cy - ry * 0.8, cx - rx * 0.02, cy - ry * 0.12)], WHITE),
                ([C(cx + rx * 0.4, cy + ry * 0.42, rx * 0.22)], WHITE)]
    if kind == "dot" and _ST["calm"]:
        rx, ry = 4 * es, 5 * es
        return [([T(*arc(cx, cy, rx, ry, -12, 192, 16))], EYE), ([C(cx - rx * 0.36, cy + ry * 0.2, rx * 0.3)], WHITE),
                ([L(arc(cx, cy + 0.6, rx + 1.3, 2.4, 180, 360, 10), 1.9)], EYE)]
    k = es if kind == "dot" else es ** 0.6
    with xf(sx=k, sy=k, pivot=(cx, cy)):
        return eye(kind, cx, cy, flip)


def strand(pts, w0, w1):
    """끝이 가늘어지는 털 한 가닥 (다각형)."""
    left, right = [], []
    n = len(pts)
    for k, (x, y) in enumerate(pts):
        a, b = pts[max(k - 1, 0)], pts[min(k + 1, n - 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        ln = math.hypot(dx, dy) or 1.0
        w = (w0 + (w1 - w0) * k / (n - 1)) / 2
        left.append((x - dy / ln * w, y + dx / ln * w))
        right.append((x + dy / ln * w, y - dx / ln * w))
    return T(*(left + right[::-1]))


def tuft(cx, top):
    """정수리에 삐죽 솟은 아기 털 두 가닥 (오른쪽은 동그랗게 말림). top: 머리 윗선."""
    return [strand([(cx - 1, top + 4)] + arc(cx + 6, top - 6, 6, 7, 180, 300, 8), 4.6, 1.2),
            strand([(cx - 3, top + 4), (cx - 5.5, top - 3), (cx - 9, top - 6.5)], 3.8, 1.0)]


def brows(kind, cx, cy, flip=False):
    """눈 위 짧은 사선 눈썹. sad: 안쪽(코 쪽) 끝이 올라감 · angry: 안쪽 끝이 내려감. flip: 화면 오른쪽 눈."""
    s = -1 if flip else 1  # 안쪽 방향 (왼쪽 눈은 +x가 안쪽)
    up = {"sad": -2.6, "angry": 2.4}[kind]
    return [([L([(cx - 4.6 * s, cy - 10), (cx + 4.4 * s, cy - 10 + up)], 1.8)], EYE)]


def sparkle_fx(cx, cy, r):
    """노란 4각 별 (주황 테두리)."""
    def star(R):
        return T(*[(cx + (R if k % 2 == 0 else R * 0.36) * math.cos(math.radians(k * 45 - 90)),
                    cy + (R if k % 2 == 0 else R * 0.36) * math.sin(math.radians(k * 45 - 90))) for k in range(8)])
    return [([star(r + 1.8)], (236, 158, 64), 1.0), ([star(r)], (255, 222, 92), 0)]


def thought_fx(cx, cy, r=12.5):
    """생각 풍선 (작은 원 2개 + 큰 원) 안에 🍙: 흰 삼각 밥 + 검은 김 띠."""
    fx = []
    for x, y, rr in [(cx - r - 7, cy + r + 8, 2.0), (cx - r - 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], LINE, 1.0), ([C(x, y, rr)], WHITE, 0)]
    tri = [(cx, cy - 7.5), (cx - 8, cy + 6.5), (cx + 8, cy + 6.5)]
    fx += [([L(tri + [tri[0]], 4.4)], LINE, 0), ([T(*tri), L(tri + [tri[0]], 1.8)], WHITE, 0),
           ([T((cx - 3.6, cy + 1.2), (cx + 3.6, cy + 1.2), (cx + 3.6, cy + 7.6), (cx - 3.6, cy + 7.6))], (52, 62, 58), 0)]
    return fx


def blush(cx, cy):
    bx, by = _ST["blush"]
    ops = [([E(cx - 7.5 * bx, cy - 5 * by, cx + 7.5 * bx, cy + 5 * by)], BLUSH)]
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
    """무늬 목록 항목: (도형들, 색) 또는 (도형들, 색, 단계) — 단계는 최소 lvl(int) 또는 (최소, 최대)."""
    id: str
    name: str
    walk_speed: float
    scale: float
    base: tuple  # 몸 바탕
    back: tuple  # 등 색 (배 보이기·구르기·뒷모습에서 쓰임)
    belly: tuple  # 배 색
    nose: tuple
    mouth: str  # "w" (고양이·토끼 ω) | "dog" (큰 코 + 갈라진 입: 강아지·곰)
    front_ears: list  # 머리 도형에 합쳐지는 귀 (왼쪽, 오른쪽)
    front_ear_pivots: tuple  # 귀 크기를 바꿀 때 기준점 (귀 뿌리) — 왼쪽, 오른쪽
    front_ear_marks: list  # 귀 안쪽 무늬 (귀와 함께 크기가 바뀐다; x < 64면 왼쪽 귀)
    front_head_marks: list  # [(descs, color[, 단계])]
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
    stage_size: dict = field(default_factory=lambda: dict.fromkeys(STAGES, 1.0))  # pet.json stage_size: 공통 단계 배율에 곱하는 종 배율
    stage_tweak: dict = field(default_factory=dict)  # {단계: STAGE_PROPS 덮어쓰기} — 종별 단계 특징 (수염·긴 주둥이 …)
    stripe: tuple = None  # 줄무늬 색 (arch 꼬리 등)
    back_head_marks: list = field(default_factory=list)  # 뒷모습 (climb·slide) 머리·몸 무늬 — 바탕은 back 색
    back_body_marks: list = field(default_factory=list)


def _stripe(pts, w=2.8):
    return ("L", pts, w)


def _mirror(pts):
    return [(128 - x, y) for x, y in pts]


CAT_G, CAT_ST, CAT_W = (248, 190, 128), (234, 158, 96), (255, 250, 242)
_cat_tail_line = [(26, 78), (12, 64), (10, 46), (16, 36)]
_cat_lobe_stripes = [[(19, 35), (27, 37.5)], [(18, 43), (25, 44.5)]]
CAT = Species(
    id="cat", name="고양이", walk_speed=90, scale=0.94, base=CAT_W, back=CAT_G, belly=CAT_W, nose=NOSE, mouth="w",
    front_ears=[("T", (20, 40), (24, 6), (52, 22)), ("T", (76, 22), (104, 6), (108, 40))],
    front_ear_pivots=((38, 32), (90, 32)),
    front_ear_marks=[([("T", (28, 30), (29, 14), (44, 23)), ("T", (84, 23), (99, 14), (100, 30))], PINK)],
    front_head_marks=[
        ([("E", -6, -20, 134, 44), ("E", 14, 20, 50, 52), ("E", 78, 20, 114, 52)], CAT_G),
        ([("T", (60, 50), (62, 16), (66, 16), (68, 50))], CAT_W, 1),  # 이마 흰 줄 (유아부터)
        ([("E", 79, 63, 91, 73)], CAT_G, 1),  # 왼쪽 뺨 점 (유아부터)
        ([("E", 30, 44, 58, 66), ("E", 70, 44, 98, 66)], CAT_W),
        ([_stripe([(64, 18), (64, 28)])], CAT_ST, (0, 0)),  # 아기: 가운데 줄 하나
        ([_stripe([(x, 19), (x + (x - 64) * 0.08, 28)]) for x in (56, 72)], CAT_ST, (1, 1)),
        ([_stripe([(x, 19), (x + (x - 64) * 0.08, 28)]) for x in (50, 57, 71, 78)], CAT_ST, 2),
        ([_stripe(p, 2.4) for p in _cat_lobe_stripes] + [_stripe(_mirror(p), 2.4) for p in _cat_lobe_stripes], CAT_ST, 4),
    ],
    front_body_marks=[
        ([("E", 16, 68, 42, 112), ("E", 86, 68, 112, 112)], CAT_G, 1),
        ([("E", 86, 98, 112, 122)], CAT_G, 1),
        ([_stripe([(31, 84), (37, 86)], 2.4), _stripe([(97, 84), (91, 86)], 2.4)], CAT_ST, 2),
        ([_stripe([(31, 95), (37, 97)], 2.4), _stripe([(97, 95), (91, 97)], 2.4)], CAT_ST, 3),
        ([_stripe([(32, 74), (37, 76)], 2.2), _stripe([(96, 74), (91, 76)], 2.2)], CAT_ST, 4),
    ],
    front_tail=[("E", 90, 100, 108, 118)], front_tail_root=(94, 112), front_tail_color=CAT_G,
    side_ears=[Ear([("T", (58, 34), (64, 4), (84, 20))], (70, 26), CAT_G, [([("T", (65, 29), (66, 14), (77, 22))], PINK)]),
               Ear([("T", (94, 18), (112, 4), (118, 32))], (106, 24), CAT_G)],
    ears_over=False,
    side_head_marks=[
        ([("E", 40, -10, 120, 42), ("E", 44, 20, 76, 60)], CAT_G),
        ([("T", (110, 42), (115, 16), (128, 16), (128, 42))], CAT_W, 1),
        ([_stripe([(72, 18), (74, 27)])], CAT_ST, (0, 0)),
        ([_stripe([(x, 18), (x + 2, 27)]) for x in (66, 78)], CAT_ST, (1, 1)),
        ([_stripe([(x, 18), (x + 2, 27)]) for x in (62, 72, 82)], CAT_ST, 2),
    ],
    side_torso_marks=[
        ([("E", 8, 46, 92, 80), ("E", 12, 70, 44, 104)], CAT_G, 1),
        ([_stripe([(x, 58), (x + 2, 67)]) for x in (46, 58)], CAT_ST, (2, 2)),
        ([_stripe([(x, 58), (x + 2, 67)]) for x in (40, 52, 64)], CAT_ST, 3),
        ([_stripe([(76, 58), (78, 66)])], CAT_ST, 4),
    ],
    side_tail=[("L", _cat_tail_line, 10)], tail_root=(26, 78), tail_color=CAT_G,
    tail_marks=[([_stripe([(9, 58), (17, 58)]), _stripe([(10, 47), (18, 48)])], CAT_ST, 2)],
    specials=["arch", "huff", "climb", "slide"],
    stage_tweak={"adult": dict(whiskers=True)},
    stripe=CAT_ST,
    back_head_marks=[
        ([_stripe([(64, 17), (64, 30)], 3.0)], CAT_ST, (0, 0)),
        ([_stripe([(x, 17), (x + (x - 64) * 0.1, 30)], 3.0) for x in (57, 71)], CAT_ST, (1, 1)),
        ([_stripe([(x, 17), (x + (x - 64) * 0.1, 30)], 3.0) for x in (52, 64, 76)], CAT_ST, 2),
        ([("E", 34, 64, 94, 92)], CAT_W, 1),  # 뒷목 아래 흰 털
    ],
    back_body_marks=[
        ([("E", 22, 40, 106, 108)], CAT_G, 1),  # 등 (아기는 하얀 몸)
        ([_stripe(arc(64, 84 + dy, 22 - abs(dy) * 0.3, 6, 205, 335, 8), 3.0) for dy in (-10, 2)], CAT_ST, (2, 2)),
        ([_stripe(arc(64, 80 + dy, 22, 6, 205, 335, 8), 3.0) for dy in (-10, 0, 10)], CAT_ST, 3),
    ],
)

DOG_W, DOG_G, DOG_EAR = (255, 247, 236), (222, 168, 112), (196, 140, 92)
DOG = Species(
    id="dog", name="강아지", walk_speed=105, scale=0.94, base=DOG_W, back=DOG_G, belly=DOG_W, nose=DARK_NOSE, mouth="dog",
    front_ears=[("E", 8, 30, 32, 76), ("E", 96, 30, 120, 76)],
    front_ear_pivots=((22, 32), (106, 32)),
    front_ear_marks=[
        ([("E", 8, 30, 32, 76), ("E", 96, 30, 120, 76)], DOG_EAR),
        ([("L", [(31, 34), (28, 52), (31, 72)], 1.6), ("L", [(97, 34), (100, 52), (97, 72)], 1.6)], LINE, 2),
    ],
    front_head_marks=[
        ([("E", 8, -6, 120, 44)], DOG_G),
        ([("E", 30, 46, 52, 60)], DOG_G, 2),  # 눈가 점 (어린이부터)
    ],
    front_body_marks=[([("E", 16, 72, 40, 110), ("E", 88, 72, 112, 110)], DOG_G, 1), ([("E", 90, 90, 112, 114)], DOG_G, 1)],
    front_tail=[("E", 92, 92, 110, 112)], front_tail_root=(96, 108), front_tail_color=DOG_G,
    side_ears=[Ear([("E", 64, 22, 84, 58)], (74, 26), DOG_EAR)],
    ears_over=True,
    side_head_marks=[([("E", 40, -6, 120, 40)], DOG_G)],
    side_torso_marks=[([("E", 8, 48, 92, 80)], DOG_G, 1), ([("E", 60, 50, 74, 60)], DOG_EAR, 4)],
    side_tail=[("L", [(24, 70), (14, 56), (20, 44)], 11)], tail_root=(24, 70), tail_color=DOG_G,
    wag_tail=[("L", [(0, 0), (12, -10), (16, -24)], 10)],
    specials=["wag"],
    stage_tweak={"adult": dict(snout=3.0)},
)

RAB_W = (255, 252, 250)
RABBIT = Species(
    id="rabbit", name="토끼", walk_speed=120, scale=0.8, base=RAB_W, back=RAB_W, belly=RAB_W, nose=NOSE, mouth="w",
    front_ears=[("E", 30, -22, 50, 34), ("E", 78, -22, 98, 34)],
    front_ear_pivots=((40, 28), (88, 28)),
    front_ear_marks=[([("E", 35, -14, 45, 28), ("E", 83, -14, 93, 28)], PINK)],
    front_head_marks=[],
    front_body_marks=[],
    front_tail=[("E", 92, 100, 110, 118)], front_tail_root=(98, 110), front_tail_color=RAB_W,
    side_ears=[Ear([("E", 84, -18, 100, 30)], (92, 24), RAB_W),
               Ear([("E", 70, -21, 86, 30)], (78, 24), RAB_W, [([("E", 74, -13, 82, 24)], PINK)])],
    ears_over=False,
    side_head_marks=[],
    side_torso_marks=[],
    side_tail=[("E", 10, 64, 30, 84)], tail_root=(20, 74), tail_color=RAB_W,
    lie_ears=(-48, -42),
    stage_size=dict.fromkeys(STAGES, 0.85),
    stage_tweak={"baby": dict(ear=0.8), "kinder": dict(ear=0.88), "adult": dict(ear=1.12, chest_fluff=True)},
)

BEAR_B, BEAR_L, BEAR_IN, BEAR_NOSE = (190, 132, 90), (242, 214, 176), (136, 90, 62), (92, 60, 48)
BEAR_SOFT = mix(BEAR_L, BEAR_B, 0.3)  # 아기: 주둥이·배 대비를 줄인 색
BEAR = Species(
    id="bear", name="곰", walk_speed=60, scale=0.94, base=BEAR_B, back=BEAR_B, belly=BEAR_L, nose=BEAR_NOSE, mouth="dog",
    front_ears=[("E", 14, 10, 42, 38), ("E", 86, 10, 114, 38)],
    front_ear_pivots=((34, 32), (94, 32)),
    front_ear_marks=[([("E", 20, 16, 36, 32), ("E", 92, 16, 108, 32)], BEAR_IN)],
    front_head_marks=[([("E", 44, 52, 84, 78)], BEAR_SOFT, (0, 0)), ([("E", 44, 52, 84, 78)], BEAR_L, (1, 3)),
                      ([("E", 40, 50, 88, 81)], BEAR_L, 4)],
    front_body_marks=[([("E", 44, 80, 84, 120)], BEAR_SOFT, (0, 0)), ([("E", 44, 80, 84, 120)], BEAR_L, 1)],
    front_tail=[("E", 92, 104, 110, 120)], front_tail_root=(98, 112), front_tail_color=BEAR_B,
    side_ears=[Ear([("E", 69, 5, 91, 27)], (80, 20), BEAR_B, [([("E", 74, 10, 86, 22)], BEAR_IN)])],
    ears_over=False,
    side_head_marks=[([("E", 100, 40, 132, 70)], BEAR_SOFT, (0, 0)), ([("E", 100, 40, 132, 70)], BEAR_L, (1, 3)),
                     ([("E", 97, 38, 134, 73)], BEAR_L, 4)],
    side_torso_marks=[([("E", 28, 90, 94, 120)], BEAR_SOFT, (0, 0)), ([("E", 28, 90, 94, 120)], BEAR_L, 1)],
    side_tail=[("E", 6, 62, 26, 82)], tail_root=(18, 74), tail_color=BEAR_B, lie_ears=(-56, -50),
    specials=["climb", "slide"],
    stage_size={"baby": 1.0, "kinder": 1.0, "child": 1.1, "teenager": 1.2, "adult": 1.3},
    stage_tweak={"adult": dict(fbody=(1.06, 1.0), torso=(1.06, 1.0), fit=0.91)},
)

SPECIES = {s.id: s for s in (CAT, DOG, RABBIT, BEAR)}


# =====================================================================
# 4. 부품 — 종별 설정을 현재 변환 아래에서 Part로
# =====================================================================
def _shown(lvl):
    lo, hi = (lvl, 9) if isinstance(lvl, int) else lvl
    return lo <= _ST["lvl"] <= hi


def _marks(spec_marks):
    """종별 무늬 → [(fns, color)]. 세 번째 값(단계)이 있으면 현재 단계 lvl에서 보이는 것만."""
    return [([shape(d) for d in m[0]], m[1]) for m in spec_marks if len(m) < 3 or _shown(m[2])]


def _desc_x(desc):
    v = desc[1]
    while isinstance(v, (tuple, list)):
        v = v[0]
    return v


def _front_ears(sp):
    """단계 귀 크기로 바꾼 앞모습 귀 도형과 귀 무늬 (귀 뿌리 기준)."""
    k = _ST["ear"]
    shp, marks = [], []
    for side, pv in enumerate(sp.front_ear_pivots):
        mine = (lambda d: _desc_x(d) < 64) if side == 0 else (lambda d: _desc_x(d) >= 64)
        with xf(sx=k, sy=k, pivot=pv):
            shp += [shape(d) for d in sp.front_ears if mine(d)]
            for m in sp.front_ear_marks:
                if len(m) < 3 or _shown(m[2]):
                    marks.append(([shape(d) for d in m[0] if mine(d)], m[1]))
    return shp, marks


CHEEK_FLUFF = [((21, 55), (11.5, 60), (20, 63)), ((23, 62), (14, 68.5), (25, 70))]
WHISKERS = [[(27, 61), (9, 58)], [(27, 64.5), (8, 65.5)], [(28, 68), (10, 72.5)]]


# ---- 앞모습 ----
def front_head(sp, eyes="dot", mouth=None, tongue_dy=0, outline=False, cheeks=False, brow=None, face=(0, 0)):
    """앞모습 머리(귀 포함) Part와 얼굴 ops. 머리 좌표: 중심 (64, 50). 단계 머리 배율은 턱 (64, 84) 기준.
    cheeks: 볼 빵빵 (True 양쪽 · "left"/"right" 한쪽) · brow: 눈썹 ("sad" | "angry") · face: 얼굴만 옮김 (고개 돌림 흉내)."""
    with xf(sx=_ST["head"], sy=_ST["head"], pivot=(64, 84)):
        lc, rc = cheeks in (True, "left"), cheeks in (True, "right")
        puff = ([shape(("E", 10, 50, 40, 82))] if lc else []) + ([shape(("E", 88, 50, 118, 82))] if rc else [])
        snout = [shape(("E", 44, 56, 84, 84 + 2 * _ST["snout"]))] if _ST["snout"] else []
        ear_fns, ear_marks = _front_ears(sp)
        spikes = tuft(64, 16) if _ST["tuft"] else []
        if _ST["cheek_fluff"]:
            spikes += [T(*t) for t in CHEEK_FLUFF if not lc] + [T(*_mirror(t)) for t in CHEEK_FLUFF if not rc]
        part = Part([shape(("E", 16, 16, 112, 84))] + puff + snout + ear_fns, sp.base,
                    ear_marks + _marks(sp.front_head_marks), grow=1, outline=outline, spikes=spikes)
        lb, rb = 5 if lc else 0, 5 if rc else 0
        bd = 2 if _ST["sparkle"] else 0  # 큰 눈 아래로 볼터치를 조금 내림
        with xf(dx=face[0], dy=face[1]):
            ops = blush(33 - lb - bd * 0.5, 66 + bd + lb * 0.4) + blush(95 + rb + bd * 0.5, 66 + bd + rb * 0.4)
            if _ST["whiskers"]:
                ops += [([L(w, 0.9) for w in WHISKERS] + [L(_mirror(w), 0.9) for w in WHISKERS], LINE)]
            ops += stage_eye(eyes, 44, 54) + stage_eye(eyes, 84, 54, flip=True)
            with xf(dy=_ST["snout"]):
                ops += front_mouth(sp, mouth, tongue_dy)
            if brow:
                ops += brows(brow, 44, 54 + _ST["eye_dy"]) + brows(brow, 84, 54 + _ST["eye_dy"], flip=True)
    return part, ops


def front_mouth(sp, kind=None, tongue_dy=0):
    ops = []
    if sp.mouth == "dog":
        ops.append(([E(59, 56, 69, 63)], sp.nose))
        top = 66
    else:
        ops.append(([E(61.6, 59.4, 66.4, 62.6)], sp.nose))
        top = 64
    if kind is None and _ST["smile"]:  # 어린이: 작게 벌린 웃는 입
        return ops + open_mouth(64, top + 0.4, 3.2, 3.4)
    if kind is None:
        if sp.mouth == "dog":
            ops += [([L([(64, 63), (64, 66)], 1.3)], LINE), ([L([(59, 66), (62, 68.5), (64, 66), (66, 68.5), (69, 66)], 1.3)], LINE)]
        else:
            ops.append(([L([(59.8, 64), (61.9, 65.8), (64, 64), (66.1, 65.8), (68.2, 64)], 1.3)], LINE))
        return ops
    if kind == "frown":  # 작게 뒤집힌 ‿
        return ops + [([L(arc(64, top + 3.4, 3.4, 2.4, 200, 340, 8), 1.4)], LINE)]
    if kind == "pout":  # 뾰로통 작은 입 (한쪽으로 삐죽)
        return ops + [([L(arc(65.5, top + 3.6, 2.8, 2.0, 200, 340, 8), 1.4)], LINE)]
    if kind == "chew":  # 다문 입 오물오물 (작은 ω)
        return ops + [([L([(60.6, top + 0.6), (62.3, top + 2.2), (64, top + 0.8), (65.7, top + 2.2), (67.4, top + 0.6)], 1.3)], LINE)]
    rx, ry = {"open": (5.2, 6.2), "yawn": (6.4, 9.5), "tongue": (5.6, 5.0), "big": (7.0, 8.0)}[kind]
    ops += open_mouth(64, top, rx, ry, tongue=kind != "tongue")
    if kind == "tongue":
        t0 = top + 2 + tongue_dy
        ops += [([E(57.6, t0 - 1.3, 70.4, t0 + 13.3)], LINE), ([E(59, t0, 69, t0 + 12)], TONGUE),
                ([L([(64, t0 + 2), (64, t0 + 7)], 1.0)], (214, 104, 118))]
    return ops


def front_body(sp, leg_dy=(0, 0)):
    """앞모습 앉은 몸 + 앞발 두 개. 꼬리는 따로."""
    ld, rd = leg_dy
    ft = 125 - 19 * _ST["leg"] ** 0.7  # 발 높이: 단계 다리 길이
    fns = [shape(("E", 32, 58, 96, 122)), shape(("E", 36, ft + ld, 60, 125 + ld)), shape(("E", 68, ft + rd, 92, 125 + rd))]
    marks = _marks(sp.front_body_marks) + [([L([(64, 110), (64, 121)], 1.5)], LINE)]
    if _ST["chest_fluff"]:  # 어른 토끼: 턱 아래 복슬 가슴 털 (부드러운 지그재그)
        marks.append(([L([(52 + 4 * k, 89 + (k % 2) * 3.5) for k in range(7)], 1.2)], mix(LINE, sp.base, 0.6)))
    return Part(fns, sp.base, marks, grow=6)


def front_tail(sp, rot=0.0):
    k = _ST["tail"] ** 0.5 * _ST["tail_w"] ** 0.5
    with xf(rot=rot, sx=k, sy=k, pivot=sp.front_tail_root):
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


def raised_arm(sp, side=1, rot=0.0):
    """만세 팔: 어깨에서 머리 옆으로 번쩍 (자기 외곽선, 젤리 발바닥이 앞을 본다). rot: 바깥쪽으로 벌림."""
    mx = (lambda x: x) if side == 1 else (lambda x: 128 - x)
    with xf(rot=rot * side, pivot=(mx(94), 88)):
        part = Part([E(min(mx(97), mx(117)), 44, max(mx(97), mx(117)), 94)], sp.base, outline=True)
        ops = [([E(mx(cx) - r, cy - r * 0.9, mx(cx) + r, cy + r * 0.9)], PINK)
               for cx, cy, r in [(107, 57, 3.6), (102, 51, 1.6), (107, 49.5, 1.6), (112, 51, 1.6)]]
    return part, ops


def front_paw(sp, cx, cy, w=15, h=13, toes=False):
    """몸 앞에 겹친 작은 앞발 (자기 외곽선). toes: 발가락 줄 두 개."""
    marks = [([L([(cx + dx, cy + h / 2 - 3.6), (cx + dx, cy + h / 2)], 1.2) for dx in (-2.4, 2.4)], LINE)] if toes else []
    return Part([E(cx - w / 2, cy - h / 2, cx + w / 2, cy + h / 2)], sp.base, marks, outline=True)


# ---- 옆모습 (오른쪽을 본다) ----
SIDE_NECK = (80, 74)  # 옆모습 머리가 몸에 붙는 점 (머리 좌표) — 단계 머리 배율의 기준


def side_head(sp, eyes="dot", mouth=None, ear_rot=(0, 0), ear_dy=0, outline=False, brow=None):
    """옆모습 머리 Parts(귀 포함, 그릴 순서)와 얼굴 ops. 머리 좌표: 중심 (88, 45)."""
    with xf(sx=_ST["head"], sy=_ST["head"], pivot=SIDE_NECK):
        ears = []
        k = _ST["ear"]
        for i, ear in enumerate(sp.side_ears):
            with xf(dy=ear_dy, rot=ear_rot[min(i, len(ear_rot) - 1)], sx=k, sy=k, pivot=ear.pivot):
                ears.append(Part([shape(d) for d in ear.shapes], ear.color, _marks(ear.marks), grow=1, outline=outline and sp.ears_over))
        sn = _ST["snout"]
        snout = [shape(("E", 96, 38, 124 + 2 * sn, 68))] if sn else []
        head = Part([shape(("E", 52, 12, 124, 78))] + snout, sp.base, _marks(sp.side_head_marks), grow=1, outline=outline,
                    spikes=tuft(90, 12) if _ST["tuft"] else [])
        parts = [head] + ears if sp.ears_over else ears + [head]
        bd = 2 if _ST["sparkle"] else 0
        ops = blush(99 - bd, 59 + bd) + stage_eye(eyes, 104, 46)
        with xf(dx=sn * 1.6):
            ops += side_mouth(sp, mouth)
        if brow:
            ops += brows(brow, 104, 46 + _ST["eye_dy"])
    return parts, ops


def side_mouth(sp, kind=None):
    nose = [([E(118, 51, 125, 56)], sp.nose)]
    if kind is None:
        return nose + [([L([(121, 57), (119, 60), (115, 59)], 1.3)], LINE)]
    if kind == "pout":
        return nose + [([L(arc(116.5, 63.5, 3.4, 2.6, 200, 340, 8), 1.5)], LINE)]
    if kind == "open":
        return nose + [([E(110.5, 57.5, 121, 68)], LINE), ([E(111.8, 58.8, 119.7, 66.7)], MOUTH_IN),
                       ([E(113, 63, 118.5, 66.4)], TONGUE)]
    if kind == "fang":  # 작게 벌린 입 + 아기 송곳니 두 개
        return nose + [([E(108.6, 57, 121.4, 69)], LINE), ([E(110, 58.4, 120, 67.6)], MOUTH_IN),
                       ([E(111.5, 63.4, 118.5, 67.4)], TONGUE),
                       ([T((110.8, 58.4), (114.0, 58.4), (112.4, 62.6))], WHITE), ([T((116.0, 58.4), (119.2, 58.4), (117.6, 62.6))], WHITE)]
    if kind == "hiss":
        return [([E(104, 54, 123, 73)], LINE), ([E(105.6, 55.6, 121.4, 71.4)], MOUTH_IN), ([E(108, 64, 119, 71)], TONGUE),
                ([T((107.5, 56.5), (111.5, 56.5), (109.5, 62.5))], WHITE), ([T((114.5, 56.5), (118.5, 56.5), (116.5, 62.5))], WHITE),
                ([E(118, 49.5, 125, 54.5)], sp.nose)]
    raise ValueError(kind)


def side_torso(sp):
    torso = Part([shape(("E", 18, 56, 98, 110))], sp.base, _marks(sp.side_torso_marks), grow=6)
    if not _ST["chest_fluff"]:
        return torso
    fluff = [T((92, 66), (101, 70), (94, 74)), T((95, 73), (103, 78), (96, 82)), T((95, 81), (102, 87), (93, 89))]
    return [torso, Part(fluff, sp.base, sharp=True, grow=1)]


def side_tail(sp, rot=0.0, at=None, sx=1.0, sy=1.0, width=None):
    """꼬리를 뿌리 기준으로 돌리고 at(뿌리 위치, 프레임 좌표)으로 옮긴다."""
    rx, ry = sp.tail_root
    dx, dy = (at[0] - rx, at[1] - ry) if at else (0, 0)
    tl, tw = _ST["tail"], _ST["tail_w"]
    with xf(dx=dx, dy=dy, rot=rot, sx=sx, sy=sy, pivot=sp.tail_root):
        descs = sp.side_tail if width is None else [(k, pts, width) if k == "L" else (k, pts, *rest) for k, pts, *rest in sp.side_tail]
        if any(k == "L" for k, *_ in descs):  # 줄 꼬리: 길이 tl, 굵기 tw (짧아져도 굵기는 그대로)
            k = tl
            descs = [(kd, pts, w * tw / tl) if kd == "L" else (kd, pts, w, *rest) for kd, pts, w, *rest in descs]
        else:  # 동그란 꼬리
            k = (tl * tw) ** 0.5
        with xf(sx=k, sy=k, pivot=sp.tail_root):
            return Part([shape(d) for d in descs], sp.tail_color, _marks(sp.tail_marks))


def legs(sp, specs, outline=False, move=None):
    """다리: (x, top, bottom, angle[, w]) — angle>0이면 발끝이 앞(오른쪽)으로.
    move: 다리 뿌리(어깨·엉덩이)를 단계 비율로 옮기는 함수 — 발끝은 그 자리에 두고 다리를 다시 잇는다."""
    fns = []
    for x, top, bottom, angle, *w in specs:
        w = w[0] if w else 16.5
        piv = (x, top + 4)
        if move is None:
            with xf(rot=-angle, pivot=piv):
                fns.append(E(x - w / 2, top, x + w / 2, bottom))
            continue
        foot = _apply(_xf_matrix(rot=-angle, pivot=piv), (x, bottom))
        p2 = move(piv)
        ln = math.hypot(foot[0] - p2[0], foot[1] - p2[1])
        th = math.degrees(math.atan2(foot[0] - p2[0], foot[1] - p2[1]))
        w *= 0.5 + 0.5 * _ST["body"]
        with xf(rot=-th, pivot=p2):
            fns.append(E(p2[0] - w / 2, p2[1] - 4, p2[0] + w / 2, p2[1] + ln))
    return Part(fns, sp.base, outline=outline)


# =====================================================================
# 5. 포즈 — (species, frame) -> 128×128 RGBA. 같은 뼈대를 종별 설정으로 칠한다
# =====================================================================
WALK_LEGS = lambda sw: [(37 - sw, 96, 124, 0, 18), (54 + sw, 98, 125, 0, 16), (78 - sw, 98, 125, 0, 16), (92 + sw, 96, 124, 0, 16)]


SIDE_BODY_PIVOT = (58, 110)  # 몸통 배 아래 가운데 (몸통 좌표) — 단계 몸 배율의 기준
LEG_SHOW = 16  # 몸통 아래로 보이는 다리 길이 (teenager) — 짧은 다리만큼 몸이 내려앉는다


def _side(sp, *, head=None, torso=None, tail=None, leg_specs=(), head_kw=None, whole=None, before=(), after=(), fx=(), ops=(),
          body_fn=None, tail_fn=None, tail_root=None, support=True):
    """옆모습 공통 조립: 다리 → 몸통 → 꼬리 → 머리 → 덧붙임. 인자는 각 부품의 xf 인자.
    단계 비율: 몸통을 SIDE_BODY_PIVOT 기준 body배로 줄이고 짧은 다리만큼 내려앉힌 뒤, 몸통에 붙은 점(목·꼬리 뿌리·다리 뿌리)을
    같은 변환으로 옮긴다. body_fn/tail_fn: 몸통·꼬리 Part(들)를 직접 만드는 함수 (몸통 좌표 / tail_root 기준)."""
    b, drop = _ST["body"], (1 - _ST["leg"]) * LEG_SHOW if support else 0.0  # support: 다리로 서 있음 (엎드림은 아님)
    ax, ay = _ST["torso"] if support else (1.0, 1.0)  # 몸통 모양 (아기 동글 · 어른 길쭉)
    staged = b != 1 or drop != 0 or (ax, ay) != (1.0, 1.0)
    mt = _xf_matrix(**(torso or {}))
    mti = _inv(mt)
    stage_m = _xf_matrix(dy=drop, sx=b * ax, sy=b * ay, pivot=SIDE_BODY_PIVOT)

    def move(p):  # 몸통에 붙은 점(전체 좌표) → 단계 비율 위치
        return _apply(mt, _apply(stage_m, _apply(mti, p))) if staged else p

    def delta(p):
        q = move(p)
        return q[0] - p[0], q[1] - p[1]

    with xf(**(whole or {})):
        parts = list(before)
        if leg_specs:
            parts.append(legs(sp, leg_specs, move=move if staged else None))
        with xf(**(torso or {})):
            with xf(dy=drop, sx=b * ax, sy=b * ay, pivot=SIDE_BODY_PIVOT):
                body = body_fn() if body_fn else side_torso(sp)
                parts += body if isinstance(body, list) else [body]
        if tail_fn:
            tdx, tdy = delta(tail_root)
            with xf(dx=tdx, dy=tdy, sx=b, sy=b, pivot=tail_root):
                parts.append(tail_fn())
        else:
            tk = dict(tail or {})
            if staged:
                at0 = tk.get("at") or sp.tail_root
                at1 = move(at0)
                tk["at"] = (at1[0], min(at1[1], max(at0[1], 108)))  # 바닥에 놓인 꼬리는 바닥 아래로 내려가지 않게
                tk["sx"], tk["sy"] = tk.get("sx", 1.0) * b, tk.get("sy", 1.0) * b
            parts.append(side_tail(sp, **tk))
        hdx, hdy = delta(_apply(_xf_matrix(**(head or {})), SIDE_NECK))
        if not support:
            hdy = min(hdy, 0.0)  # 엎드린 머리는 앞발 위에 그대로 (바닥을 뚫지 않게)
        with xf(dx=hdx, dy=hdy):
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
    return _sit_side(sp, head=dict(dy=-1 + dy, rot=_swing(rot)), head_kw=dict(eyes="closed" if i == 3 else "dot"))


def _lying(sp, i, eyes, mouth, ear_rot, fx):
    """엎드림: 배를 바닥에 깔고 앞발 위에 턱. 머리는 말랑하게 눌린 모양(0.76)."""
    paws = [Part([E(86, 112, 112, 125.5)], sp.base, outline=True), Part([E(98, 112.5, 125, 125.5)], sp.base, outline=True)]
    return _side(sp, torso=dict(sx=1.04, sy=0.66 + 0.012 * i, pivot=(58, 110), dy=14.5, dx=-6),
                 tail=dict(at=(26, 113), rot=100, sy=0.8),
                 head=dict(sx=0.98, sy=0.76 / _ST["head"] ** 0.8, pivot=(88, 78), dy=46 - 0.3 * i, dx=-2),  # 큰 머리도 납작하게 엎드림
                 head_kw=dict(eyes=eyes, mouth=mouth, ear_rot=ear_rot, ear_dy=4),
                 leg_specs=[(32, 108, 125, 30, 22)], after=paws, fx=fx, support=False)


def sleep(sp, i):
    u = (_ST["head"] - 1) * 48  # 큰 머리(아기)일수록 z를 위로
    zs = [z_fx(106, 66 - u, 3.4), z_fx(117, 55 - u, 2.6)] if i == 0 else [z_fx(108, 63 - u, 3.6), z_fx(120, 51 - u, 2.8)]
    return _lying(sp, i, "closed", None, sp.lie_ears, zs)


def annoyed(sp, i):
    a, b = sp.lie_ears
    u = (_ST["head"] - 1) * 40
    return _lying(sp, i, "squint", "pout", (a, b - 26) if i == 0 else (a, b - 12), [anger_fx(116, 74 - i - u, 7.5)])


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
    """하악질 (무서운 척하는 아기 고양이): 둥글게 솟은 등(완만한 혹), 곧지만 짧은 다리를 살짝 벌림, 위로 선 복슬 꼬리,
    등에 작은 털 몇 가닥, 귀는 살짝 뒤로, 큰 눈 + 화난 눈썹, 작게 벌린 입에 송곳니 두 개."""
    puff = [0.55, 1.0, 1.0][i]
    cx, cy, rx, ry = 56, 104, 31, 22 + 6 * puff
    band = lambda w, dy=0: L(arc(cx, cy + dy, rx, ry, 180, 360, 24), w)
    top = lambda a, out=0.0: (cx + rx * math.cos(math.radians(a)), cy + ry * math.sin(math.radians(a)) - out)

    def body():
        stripes = [L([(x, y - 7), (x + 1, y + 2)], 2.8) for x, y in (top(a) for a in (236, 258, 280))]
        torso = Part([band(28)], sp.base, [([band(20, -6)], sp.back), (stripes, sp.stripe)], grow=4)
        tufts = []
        for a in (246, 266, 286):
            bx, by = top(a, 12)
            ln = 4 + 3 * puff
            tufts.append(T((bx - 3.4, by + 1), (bx + 1.5, by - ln), (bx + 3.4, by + 1)))
        return [torso, Part(tufts, sp.back, sharp=True, grow=1)]

    root = (28, 94)

    def tail():  # 위로 선 복슬 꼬리: 줄기 양옆에 동그라미를 엇갈려 붙여 둥근 지그재그 털 가장자리
        r = 3.6 + 2.2 * puff
        stem = [(28, 94), (18, 84), (13, 70), (12, 56), (16, 44 + 4 * (1 - puff))]
        pts = [stem[0]]
        for (x0, y0), (x1, y1) in zip(stem, stem[1:]):
            pts += [(x0 + (x1 - x0) * t / 3, y0 + (y1 - y0) * t / 3) for t in (1, 2, 3)]
        fluff = []
        for k, (x, y) in enumerate(pts[2:]):
            side = 1 if k % 2 else -1
            fluff.append(C(x + side * r * 0.75, y, r))
        fluff.append(C(pts[-1][0], pts[-1][1] - r * 0.5, r * 1.05))
        return Part(fluff + [L(pts, 1.5 * r)], sp.tail_color, [([L([(0, 74), (24, 72)], 3), L([(0, 60), (22, 59)], 3)], sp.stripe)] if _shown(2) else [],
                    outline=True)

    leg_specs = [(cx - rx - 2, 96, 125, -7, 16), (cx - rx + 10, 98, 125, -2, 15), (cx + rx - 10, 98, 125, 2, 15), (cx + rx + 2, 96, 125, 7, 16)]
    return _side(sp, whole=dict(dy=-1 if i == 2 else 0), body_fn=body, tail_fn=tail, tail_root=root, leg_specs=leg_specs,
                 head=dict(dx=2, dy=5, pivot=(82, 72)),
                 head_kw=dict(eyes="big", mouth="fang", brow="angry", ear_rot=(-16, -12)))


# ---- 앞모습 포즈 ----
FRONT_BODY_PIVOT = (64, 124)  # 앞모습 몸 바닥 가운데 — 단계 몸 배율의 기준


def _front(sp, *, head=None, body=None, whole=None, tail_rot=0.0, eyes="dot", mouth=None, tongue_dy=0, leg_dy=(0, 0),
           arms=(), paws=(), held=None, tail_desc=None, fx=(), head_kw=None, front_props=(), arm_fn=front_arm, toes=False):
    """앞모습 공통 조립: 몸 → 꼬리 → 머리 → 팔·앞발 → 소품.
    단계 비율: 몸·팔·앞발은 FRONT_BODY_PIVOT 기준 body배, 머리는 줄어든 몸 위(턱 기준)로 내려앉는다.
    front_props: 맨 앞에 놓는 소품 Part를 만드는 함수 (프레임 좌표, 단계 배율 없음)."""
    b = _ST["body"]
    fx_, fy_ = _ST["fbody"]
    stage_body = dict(sx=b * fx_, sy=b * fy_, pivot=FRONT_BODY_PIVOT)
    with xf(**(whole or {})):
        with xf(**(body or {})):
            with xf(**stage_body):
                parts = [front_body(sp, leg_dy)]
                if tail_desc is not None:
                    rx, ry = sp.front_tail_root
                    with xf(dx=rx, dy=ry, rot=tail_rot, pivot=(0, 0)):
                        parts.append(Part([shape(d) for d in tail_desc], sp.front_tail_color))
                else:
                    parts.append(front_tail(sp, tail_rot))
        with xf(dy=(1 - b) * (FRONT_BODY_PIVOT[1] - 84)):
            with xf(**(head or {})):
                hp, face = front_head(sp, eyes, mouth, tongue_dy, **(head_kw or {}))
        parts.append(hp)
        ops = list(face)
        with xf(**(body or {})):
            with xf(**stage_body):
                if held:
                    parts.append(held())
                for side, rot in arms:
                    ap, aops = arm_fn(sp, side, rot)
                    parts.append(ap)
                    ops += aops
                for cx, cy in paws:
                    parts.append(front_paw(sp, cx, cy, toes=toes))
        parts += [f() for f in front_props]
    return compose(parts, ops, fx)


def sit_front(sp, i):
    return _front(sp, eyes="closed" if i == 3 else "dot", tail_rot=[0, -6, 0, 0][i], head=dict(dy=[0, 0.6, 0, 0][i]))


def wave(sp, i):
    return _front(sp, arms=[(1, [0, 26, 0, 26][i])], head=dict(rot=[-3, -1, -3, -1][i], pivot=(64, 84)))


def _swing(v):
    """머리를 돌리는 각도·흔드는 폭: 큰 머리(아기)는 덜 돌려 프레임 안에 둔다."""
    return v / max(1.0, _ST["head"]) ** 4


def tilt(sp, i):
    deg = _swing([8, 12, 12][i])
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
    return _front(sp, whole=dict(dx=_swing(dx), dy=-1, rot=_swing(rot), pivot=(64, 124)), eyes="happy", mouth="open",
                  arms=[(1, [-6, 18, 6][i]), (-1, [18, -6, 6][i])], leg_dy=[(-3, 0), (0, -3), (-2, -2)][i])


def wag(sp, i):
    rot = [-25, -5, 15, -5][i]
    return _front(sp, eyes="happy", mouth="tongue", tongue_dy=[0, 1, 0, 1][i], body=dict(dy=[0, -1, 0, -1][i]),
                  head=dict(dy=[0, -1, 0, -1][i]), tail_desc=sp.wag_tail, tail_rot=rot)


def roll(sp, i):
    """몸을 동글게 만 털뭉치: 옆으로 조금 납작한 달걀꼴 (가로:세로 ≈ 1.15:1) 위로 머리·귀가 볼록, 아래로 앞발,
    옆으로 꼬리가 삐죽 나온다. 회전 중심 (64, 88) — 모든 픽셀이 반지름 ROLL_RADIUS 원 안 (안 보이는 원으로 잘라 둔다)."""
    cx, cy = ROLL_CENTER
    k = 0.9 if sp.scale < 0.9 else 1.0  # 귀가 긴 토끼는 조금 작게
    rx, ry = 33 * k, 23 * k
    with view(scale=1.0, lift=0):
        ball = Part([E(cx - rx, cy - ry + 2, cx + rx, cy + ry + 2)], sp.back,
                    [([E(cx - rx * 0.7, cy - ry * 0.1, cx + rx * 0.8, cy + ry + 1)], sp.belly)], grow=0)
        tail = Part([L(arc(cx - 1, cy + 3, rx + 1, ry + 1, 112, 196, 12), 10.5 * k)], sp.tail_color, outline=True)
        paws = [Part([E(cx - 15, cy + ry - 6, cx - 2, cy + ry + 5)], sp.base, outline=True),
                Part([E(cx + 3, cy + ry - 7, cx + 16, cy + ry + 4)], sp.base, outline=True)]
        s = 0.55 * k / _ST["head"]  # 공 안의 머리는 단계 머리 배율 없이 (귀가 회전 원 밖으로 나가지 않게)
        head_at = dict(dx=cx + 4 - 64, dy=cy - 5 - 50, sx=s, sy=s, pivot=(64, 50))
        with xf(rot=[-6, 6][i], pivot=(cx, cy - 6)):
            with xf(**head_at):
                hp, face = front_head(sp, "happy", None, outline=True)
                if i == 1:  # 두 번째 프레임은 볼이 더 발그레
                    face = face + [([E(22, 60, 44, 72), E(84, 60, 106, 72)], BLUSH)] + blush(33, 66)[1:] + blush(95, 66)[1:]
        clip = shapes([C(cx, cy, ROLL_RADIUS - 1.25 * OUTLINE * _ST["line"] - 1.2)])
    return compose([ball, tail, hp] + paws, face, clip=clip)


def hungry(sp, i):
    """배고파 시무룩: 처진 눈썹 + ︶ 눈, 작게 뒤집힌 입, 한 앞발로 배를 문지르고 머리 위에 🍙 생각 풍선."""
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    return _front(sp, eyes="closed", mouth="frown", head=dict(dy=2, rot=-4, pivot=(64, 84)), head_kw=dict(brow="sad"),
                  tail_rot=6, paws=[(56 + dx, 100 + dy)], toes=True, fx=thought_fx(108, 17))


BOWL, BOWL_LIGHT, RICE, RICE_GRAIN = (246, 196, 150), (252, 226, 196), (255, 255, 252), (230, 222, 206)


def bowl_parts():
    """앞에 놓인 밥그릇 (낮은 반원, 갈색 외곽선) + 소복한 밥."""
    rice = Part([E(41, 92, 87, 112)], RICE, [([E(52 + 9 * k - 1.6, 97 + (k % 2) * 5 - 1, 52 + 9 * k + 1.6, 97 + (k % 2) * 5 + 1) for k in range(4)], RICE_GRAIN)],
                outline=True)
    bowl = Part([T(*arc(64, 103, 27, 21, 0, 180, 20))], BOWL, [([L([(40, 110), (88, 110)], 2.6)], BOWL_LIGHT)], outline=True)
    return [rice, bowl]


def eat(sp, i):
    """밥그릇 위로 고개 숙여 냠냠: 머리 3px 위아래, 입 벌림(앙)/다묾(볼 빵빵), 그릇에서 밥알 부스러기."""
    bite = i % 2 == 0
    crumbs = [[(36, 92), (93, 88)], [(31, 84), (97, 80), (89, 92)], [(38, 90), (92, 86)], [(33, 81), (96, 78), (40, 92)]][i]
    fx = []
    for x, y in crumbs:
        fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], RICE, 0)]
    paws = [(36, 101), (92, 101)]
    return _front(sp, eyes="happy" if bite else "closed", mouth="open" if bite else "chew", head_kw=dict(cheeks=not bite),
                  head=dict(dy=8 if bite else 5), fx=fx,
                  front_props=[lambda: bowl_parts()[0], lambda: bowl_parts()[1]] + [lambda p=p: front_paw(sp, *p) for p in paws])


def level_up(sp, i):
    """레벨 업 만세: 양팔 번쩍, 눈 ^^, 활짝 웃음, 노란 반짝이 (프레임마다 자리·크기 변화), 프레임 2는 1px 점프."""
    lift = [12, 17, 20][i]
    stars = [[(18, 40, 5.5), (110, 34, 4.0), (22, 86, 3.6), (106, 84, 5.0)],
             [(20, 30, 4.2), (106, 26, 6.0), (18, 74, 5.0), (110, 74, 3.6)],
             [(18, 50, 6.0), (110, 44, 4.6), (24, 22, 3.8), (104, 16, 4.4)]][i]
    fx = [f for x, y, r in stars for f in sparkle_fx(x, y, r)]
    return _front(sp, whole=dict(dy=-1 if i == 2 else 0), eyes="happy", mouth="big", arms=[(1, lift), (-1, lift)],
                  arm_fn=raised_arm, fx=fx)


def huff(sp, i):
    """흥! (고양이 전용): 시큰둥 반눈, 한쪽 볼 빵빵, 뾰로통 입, 💢, 0 정면 → 1·2 고개를 돌려 턱을 들고 코는 하늘로, 꼬리 탁탁."""
    head = [dict(), dict(rot=-9, dy=-1, pivot=(64, 84)), dict(rot=-12, dy=-2, pivot=(64, 84))][i]
    face = [(0, 0), (6, -2), (7, -3)][i]
    u = (_ST["head"] - 1) * 40
    return _front(sp, eyes="flat", mouth="pout", head=head, head_kw=dict(cheeks="right", face=face), tail_rot=[0, -16, 12][i],
                  fx=[anger_fx(24, 24 - u + [0, -1, -2][i], 6.0)])


# ---- 뒷모습 포즈 (고양이·곰): 화면에 매달려 기어오르기 / 미끄러지기. 얼굴은 안 보인다 ----
SWEAT, SWEAT_LINE = (176, 220, 248), (104, 156, 204)
BACK_HEAD = 0.8  # 뒷모습 머리는 조금 작게 (몸과 위로 뻗은 앞발이 보이게)


def back_head(sp):
    """뒤통수 (귀 포함, 귀 안쪽은 안 보임). 바탕은 등 색, 고양이는 줄무늬와 뒷목 흰 털."""
    k = _ST["head"] * BACK_HEAD
    with xf(dy=-4, sx=k, sy=k, pivot=(64, 84)):
        ear_fns, _ = _front_ears(sp)
        return Part([shape(("E", 16, 16, 112, 84))] + ear_fns, sp.back, _marks(sp.back_head_marks), grow=1, outline=True,
                    spikes=tuft(64, 16) if _ST["tuft"] else [])


def back_arm(sp, side, reach, scratch=None):
    """화면을 짚은 앞발 (자기 외곽선). reach: 0 낮게 옆으로 짚음 … 1 머리 옆 위로 쭉. scratch: 발 위 긁힌 자국 위상."""
    mx = (lambda x: x) if side == 1 else (lambda x: 128 - x)
    ln = 34 + 24 * reach
    sh = (mx(84), 84)
    ops = []
    with xf(rot=(24 + 38 * (1 - reach)) * side, pivot=sh):
        part = Part([E(sh[0] - 7.5, sh[1] - ln, sh[0] + 7.5, sh[1] + 6)], sp.back,
                    [([E(sh[0] - 8, sh[1] - ln - 1, sh[0] + 8, sh[1] - ln + 11)], sp.base)], outline=True)
        if scratch is not None:
            for k, dx in enumerate((-4.2, 0, 4.2)):
                y0 = sh[1] - ln - 3.5 - (k % 2) * 1.5 - scratch * 2
                ops.append(([L([(sh[0] + dx, y0), (sh[0] + dx, y0 - 7)], 1.3)], LINE))
    return part, ops


def back_leg(sp, side, lift):
    """뒷다리 (발바닥 젤리가 보인다). lift: 들어 올린 높이."""
    cx = 64 + 17 * side
    top, bot = 104 - lift, 125 - lift
    part = Part([E(cx - 11, top, cx + 11, bot)], sp.base, outline=True)
    beans = [([E(cx - 4.4, bot - 9, cx + 4.4, bot - 3)], PINK)]
    beans += [([C(cx + dx, bot - 12.5, 1.7)], PINK) for dx in (-4.4, 0, 4.4)]
    return part, beans


def back_tail(sp, rot):
    """뒷모습 꼬리: 줄 꼬리(고양이)는 엉덩이에서 위로 세워 살랑, 동그란 꼬리(곰)는 엉덩이에 동글."""
    root = (64, 112)
    tl, tw = _ST["tail"], _ST["tail_w"]
    if sp.side_tail[0][0] == "L":
        with xf(rot=rot, sx=tl, sy=tl, pivot=root):
            marks = [([L([(54, 96), (80, 96)], 3), L([(52, 84), (80, 84)], 3)], sp.stripe)] if sp.stripe and _shown(2) else []
            return Part([L([root, (69, 102), (72, 90), (69, 78)], 10 * tw / tl)], sp.tail_color, marks, outline=True)
    k = (tl * tw) ** 0.5
    with xf(dx=rot * 0.15, sx=k, sy=k, pivot=root):
        return Part([C(64, 106, 8)], sp.tail_color, outline=True)


def _back(sp, reach, lift, tail_rot, body_sy=1.0, dy=0.0, scratch=None, sweat=None):
    b = _ST["body"]
    fx_, fy_ = _ST["fbody"]
    stage_body = dict(sx=b * fx_, sy=b * fy_, pivot=FRONT_BODY_PIVOT)
    ops = []
    with xf(dy=dy):
        with xf(sy=body_sy, pivot=FRONT_BODY_PIVOT):
            with xf(**stage_body):
                body = Part([E(33, 50, 95, 122)], sp.base, _marks(sp.back_body_marks), grow=6)
            with xf(dy=(1 - b) * (FRONT_BODY_PIVOT[1] - 84)):
                head = back_head(sp)
                if sweat is not None:  # 💦 뒤통수 옆 땀 한 방울
                    with xf(dy=-4, sx=_ST["head"] * BACK_HEAD, sy=_ST["head"] * BACK_HEAD, pivot=(64, 84)):
                        x, y = 94, 30 + sweat
                        drop = lambda g: [C(x, y, 3.4 + g), T((x - 3.1 - g, y - 1), (x, y - 8.5 - 1.6 * g), (x + 3.1 + g, y - 1))]
                        ops += [(drop(1.3), SWEAT_LINE), (drop(0), SWEAT), ([C(x - 1.1, y - 0.6, 1.0)], WHITE)]
            with xf(**stage_body):
                tail = back_tail(sp, tail_rot)
                legs_ = [back_leg(sp, s, l) for s, l in zip((-1, 1), lift)]
                arms = [back_arm(sp, s, r, scratch) for s, r in zip((-1, 1), reach)]
        parts = [body, head, tail] + [p for p, _ in legs_] + [p for p, _ in arms]
        ops = [o for _, os_ in legs_ for o in os_] + [o for _, os_ in arms for o in os_] + ops
    return compose(parts, ops)


def climb(sp, i):
    """뒷모습으로 화면을 기어오름: 앞발 번갈아 위로 (왼↑오↓ → 오↑왼↓), 몸 살짝 늘어남, 꼬리 좌우 살랑."""
    reach = [(1.0, 0.25), (0.62, 0.62), (0.25, 1.0), (0.62, 0.62)][i]
    lift = [(0, 7), (3, 3), (7, 0), (3, 3)][i]
    return _back(sp, reach, lift, tail_rot=[-16, 0, 16, 0][i], body_sy=[1.05, 1.0, 1.05, 1.0][i], dy=[-1, 0, -1, 0][i])


def slide(sp, i):
    """주르륵 미끄러짐: 두 앞발 쭉 위로 + 발 위 긁힌 자국, 몸 살짝 찌그러짐, 땀 한 방울, 꼬리는 위로."""
    return _back(sp, (1.0, 1.0), (0, 0), tail_rot=[-3, 3][i], body_sy=0.93, dy=[0, 1][i], scratch=i, sweat=i)


POSES = {"walk": walk, "run": run, "roll": roll, "idle": idle, "look": look, "sleep": sleep, "hop": hop, "fall": fall,
         "dragged": dragged, "land": land, "sit_front": sit_front, "wave": wave, "tilt": tilt, "belly": belly,
         "stretch": stretch, "heart": heart, "tickle": tickle, "annoyed": annoyed, "arch": arch, "wag": wag,
         "hungry": hungry, "eat": eat, "level_up": level_up, "huff": huff,
         "climb": climb, "slide": slide}


def frames(sp, anim, n):
    with view(scale=sp.scale * _ST["fit"], lift=LIFT + 1.7 * OUTLINE * (_ST["line"] - 1)):  # 굵은 외곽선만큼 더 올린다
        return [POSES[anim](sp, i) for i in range(n)]


# =====================================================================
# 6. 출력
# =====================================================================
def rows_for(sp):
    return list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in sp.specials]


_RENDERED = {}  # (종, 단계) → {동작: [프레임]} — 한 프로세스에서 여러 번 build해도 한 번만 그린다


_KEEP = {LINE, EYE, DARK_NOSE, BEAR_NOSE}  # 파스텔로 바꾸지 않는 색 (외곽선·눈·코)


def _pale(v, t):
    """종별 설정 안의 모든 색(정수 3개 튜플)을 크림색 쪽으로 t만큼 섞는다."""
    if isinstance(v, tuple) and len(v) == 3 and all(isinstance(c, int) for c in v):
        return v if v in _KEEP else mix(v, CREAM, t)
    if isinstance(v, (list, tuple)):
        return type(v)(_pale(x, t) for x in v)
    if isinstance(v, Ear):
        return Ear(v.shapes, v.pivot, _pale(v.color, t), _pale(v.marks, t))
    return v


def staged_species(sp):
    """현재 단계의 파스텔 색을 입힌 종 설정."""
    t = _ST["pale"]
    if not t:
        return sp
    return replace(sp, **{f.name: _pale(getattr(sp, f.name), t) for f in fields(sp) if f.name not in ("stage_size", "stage_tweak")})


def render_species(sp, stage_name="teenager"):
    key = (sp.id, stage_name)
    if key not in _RENDERED:
        with stage(stage_name, sp.stage_tweak.get(stage_name)):
            ssp = staged_species(sp)
            _RENDERED[key] = {anim: frames(ssp, anim, n) for anim, n, *_ in rows_for(sp)}
    return _RENDERED[key]


def write_sheet(sp, rendered, path):
    rows = rows_for(sp)
    cols = max(n for _, n, *_ in rows)
    sheet = Image.new("RGBA", (cols * FRAME, len(rows) * FRAME), (0, 0, 0, 0))
    for r, (anim, *_) in enumerate(rows):
        for c, im in enumerate(rendered[anim]):
            sheet.paste(im, (c * FRAME, r * FRAME))
    sheet.save(path)


def write_species(sp, out_dir):
    """sheet_<stage>.png 5장 + sheet.png(= sheet_teenager.png 바이트 그대로) + pet.json. {stage: {anim: [frames]}} 반환."""
    out_dir.mkdir(parents=True, exist_ok=True)
    by_stage = {}
    for st in STAGES:
        by_stage[st] = render_species(sp, st)
        write_sheet(sp, by_stage[st], out_dir / f"sheet_{st}.png")
    shutil.copyfile(out_dir / "sheet_teenager.png", out_dir / "sheet.png")
    counts = {anim: len(by_stage["teenager"][anim]) for anim, *_ in rows_for(sp)}
    m = manifest(sp.name, sp.walk_speed, counts, SPEECH[sp.id], FAVORITE_FOOD[sp.id], STAGES, sp.stage_size)
    if sp.id in PERSONALITY:
        m["personality"] = PERSONALITY[sp.id]
    (out_dir / "pet.json").write_text(json.dumps(m, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return by_stage


def build(out_root):
    """모든 종의 시트·pet.json을 out_root/<id>/에 쓴다. {id: {stage: {anim: [frames]}}} 반환."""
    out_root = Path(out_root)
    return {pid: write_species(sp, out_root / pid) for pid, sp in SPECIES.items()}


STAGE_PREVIEW_ANIMS = ["walk", "sit_front", "wave", "eat", "sleep"]


def preview_stages(by_stage, sp, path, bg=(255, 255, 255)):
    """한 동물의 5단계 × 대표 동작 (첫 프레임) + 오른쪽에 화면 표시 크기(STAGE_SCALE)로 줄인 sit_front."""
    from PIL import ImageFont
    names = STAGE_PREVIEW_ANIMS + (["climb"] if "climb" in sp.specials else [])
    cell, label = FRAME + 4, 70
    W = label + len(names) * cell + 24 + cell
    H = 24 + len(STAGES) * cell
    img = Image.new("RGBA", (W, H), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    for c, anim in enumerate(names + ["(display)"]):
        d.text((label + c * cell + (24 if c == len(names) else 0), 6), anim, fill=(60, 60, 60), font=font)
    for r, st in enumerate(STAGES):
        y = 24 + r * cell
        d.text((4, y + FRAME // 2), st, fill=(60, 60, 60), font=font)
        for c, anim in enumerate(names):
            img.alpha_composite(by_stage[st][anim][0], (label + c * cell, y))
        k = STAGE_SCALE[st] / STAGE_SCALE["adult"]
        small = by_stage[st]["sit_front"][0].resize((round(FRAME * k), round(FRAME * k)), Image.LANCZOS)
        img.alpha_composite(small, (label + len(names) * cell + 24 + (FRAME - small.width) // 2, y + FRAME - small.height))
    img.save(path)


def preview_back(by_stage, path, bg=(255, 255, 255)):
    """뒷모습 climb·slide 모든 프레임 × 5단계."""
    from PIL import ImageFont
    cols = [("climb", c) for c in range(4)] + [("slide", c) for c in range(2)]
    cell, label = FRAME + 4, 70
    img = Image.new("RGBA", (label + len(cols) * cell, 24 + len(STAGES) * cell), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    for c, (anim, k) in enumerate(cols):
        d.text((label + c * cell, 6), f"{anim} {k}", fill=(60, 60, 60), font=font)
    for r, st in enumerate(STAGES):
        d.text((4, 24 + r * cell + FRAME // 2), st, fill=(60, 60, 60), font=font)
        for c, (anim, k) in enumerate(cols):
            img.alpha_composite(by_stage[st][anim][k], (label + c * cell, 24 + r * cell))
    img.save(path)


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
            for c, im in enumerate(all_frames[pid]["teenager"].get(anim, [])):
                img.alpha_composite(im, (label + si * (block + gap) + c * cell, 30 + r * cell))
    img.save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    all_frames = build(root / "ui" / "pets")
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    preview(all_frames, pv / "all_light.png", (255, 255, 255))
    preview(all_frames, pv / "all_dark.png", (40, 44, 52))
    for pid, sp in SPECIES.items():
        preview_stages(all_frames[pid], sp, pv / f"stages_{pid}.png")
        if "climb" in sp.specials:
            preview_back(all_frames[pid], pv / f"climb_{pid}.png")
    print("ok:", ", ".join(SPECIES))


if __name__ == "__main__":
    main()
