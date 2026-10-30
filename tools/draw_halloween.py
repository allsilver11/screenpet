"""할로윈 친구들 🎃👻 — 눈에 불이 들어오는 호박(pumpkin)과 우유빛 유령(ghost). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을
그대로 쓰고, 빛(glow)은 따로 겹치는 층으로 그린다: 몸 안쪽으로 잘린 부드러운 후광 층 → 얼굴 층(조각한 눈·입, 반짝이).

    python tools/draw_halloween.py  → ui/pets/{pumpkin,ghost}/{sheet_<stage>.png ×5, sheet.png, pet.json}
                                      + tools/preview/stages_{pumpkin,ghost}.png
    (python tools/draw_pets.py도 둘을 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

호박: 반질반질 주황 몸(골 세 줄), 꼬불 꼭지, 짤막한 발·팔. 조각한 세모 눈과 들쭉날쭉 웃는 입이 안에서 노랗게 빛난다
      (프레임마다 살짝 일렁임, 잘 때는 잉걸불처럼 어둑). 걸음 = 통통 튀는 공 (pet.json hop_share 1.0 — 걷기는 늘 깡총).
      단계: 아기 = 잎 달린 미니 호박 → 유아 잎 + 짧은 꼭지 → 어린이 꼭지 덩굴손 → 청소년 골 깊게 + 큰 덩굴 + 잎 → 어른 작은 마녀 모자.
유령: 반투명 우유빛 몸(알파 ≈ 0.88), 배 속의 따뜻한 주황 빛, 반짝이 두 개 큰 눈 + 볼터치 + 작은 입, 짤막한 팔, 물결치는 치맛단.
      둥실 난다 (locomotion fly, 꿀벌보다 느리게). 내려앉는 동작: sleep · annoyed · land.
      떠 있을 때는 치마(흰 천 하단)가 몸 높이의 15%만큼 더 길게 늘어진다 (쉬는 동작은 짧게).
      단계: 아기 = 짧은 이불 덩어리 → 치마가 길어짐 → 어린이 머리 꼭지 → 청소년 → 어른 리본.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 입이 그릇 밥 더미 바로 위 (프레임 y ≈ 84).
"""

import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageFilter

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (BLUSH, C, E, HEART, K, L, LINE, S, SPEED, T, WHITE, Part, Species, _ST, aa, anger_fx,  # noqa: E402
                       arc, bee_mouth, brows, grow, heart_fx, mix, question_fx, shapes, sparkle_fx, stage_eye, strand, sweat_fx,
                       view, xf, z_fx)
from sprite_spec import FRAME, ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

# ---- 색 ----
PUMP, PUMP_RIDGE, PUMP_LINE, PUMP_FOOT = (250, 156, 62), (222, 108, 38), (150, 80, 48), (238, 128, 48)
STEM, STEM_DARK, LEAF, LEAF_VEIN = (150, 142, 78), (112, 104, 58), (150, 202, 108), (110, 166, 84)
HAT, HAT_LIGHT, HAT_BAND = (138, 104, 188), (172, 144, 216), (255, 200, 92)
GLOW_Y, GLOW_CORE, CARVE, HALO, EMBER, INNER = (255, 226, 92), (255, 252, 218), (128, 58, 30), (255, 178, 60), (236, 116, 50), (255, 212, 120)
GHOST, GHOST_LINE, GHOST_SHADE = (253, 250, 247), (156, 136, 152), (238, 232, 240)
WARM, WARM_CORE = (255, 178, 96), (255, 226, 160)
BOW, BOW_LIGHT = (248, 146, 176), (255, 196, 212)
CANDY, CANDY_STRIPE, CANDY_WRAP = (255, 166, 72), (255, 246, 232), (186, 150, 230)
LOLLY, LOLLY_SWIRL = (250, 132, 170), (255, 240, 246)
CHEEK, CHEEK_LINE = (255, 136, 120), (232, 96, 80)

FLOOR = 125.0
FLICK = [1.0, 0.8, 0.94, 0.86]  # 호박 불빛 일렁임 (프레임마다)
GHOST_ALPHA = 0.88  # 유령 몸 안쪽 투명도

# 단계(lvl)별 호박 — rx, ry: 몸 반지름 · ridge: 골 진하기 · stem: 꼭지 크기 · leaf · curl: 덩굴손 · hat: 마녀 모자
PK_STAGE = {
    0: dict(rx=33, ry=29, ridge=0.35, stem=0.75, leaf=True, curl=0.0, hat=False),
    1: dict(rx=36, ry=30, ridge=0.55, stem=0.85, leaf=True, curl=0.0, hat=False),
    2: dict(rx=39, ry=31.5, ridge=0.75, stem=1.0, leaf=False, curl=1.0, hat=False),
    3: dict(rx=42, ry=33, ridge=0.95, stem=1.12, leaf=True, curl=1.25, hat=False),
    4: dict(rx=43, ry=33.5, ridge=1.0, stem=1.0, leaf=False, curl=1.0, hat=True),
}
# 단계(lvl)별 유령 — r: 머리 반지름 · L: 치마 길이 (머리 중심 → 단) · n: 단 물결 수 · arm: 팔 크기 · curl: 머리 꼭지 · bow: 리본
GH_STAGE = {
    0: dict(r=30, L=26, n=3, arm=0.7, curl=False, bow=False),
    1: dict(r=30, L=32, n=3, arm=0.82, curl=False, bow=False),
    2: dict(r=29.5, L=38, n=4, arm=0.92, curl=True, bow=False),
    3: dict(r=29, L=43, n=4, arm=1.0, curl=True, bow=False),
    4: dict(r=29, L=47, n=5, arm=1.05, curl=False, bow=True),
}
GH_EYE = 1.5  # 유령 눈은 크게 (반짝이 두 개 큰 눈)
SKIRT_LONG = 0.15  # 떠 있을 때 치마를 몸 높이(2.3r + L)의 15%만큼 더 길게 늘어뜨린다 (머리·얼굴 크기는 그대로)


def _skirt(st):
    """떠 있는 유령의 치마 길이 (머리 중심 → 단)."""
    return st["L"] + SKIRT_LONG * (2.3 * st["r"] + st["L"])


def _pk():
    return PK_STAGE[_ST["lvl"]]


def _gh():
    return GH_STAGE[_ST["lvl"]]


def _fs():
    return _ST["head"] ** 0.6


def _bez(p0, mid, p2, n=12):
    """p0 → mid(가운데를 지남) → p2 2차 곡선."""
    cx_, cy_ = 2 * mid[0] - (p0[0] + p2[0]) / 2, 2 * mid[1] - (p0[1] + p2[1]) / 2
    return [((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * cx_ + t * t * p2[0], (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * cy_ + t * t * p2[1])
            for t in (k / n for k in range(n + 1))]


# =====================================================================
# 겹치는 층: (fns, color, alpha, grow_px, blur_px)
# =====================================================================
def _layer(items):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    for fns, color, alpha, g, b in items:
        m = shapes(fns)
        if g:
            m = grow(m, g)
        m = m.filter(ImageFilter.GaussianBlur(b * K)) if b else aa(m)
        if alpha < 1:
            m = m.point(lambda v, a=alpha: round(v * a))
        lay = Image.new("RGBA", (S, S), color + (0,))
        lay.putalpha(m)
        img.alpha_composite(lay)
    return img.resize((FRAME, FRAME), Image.LANCZOS)


def _ops(ops, alpha=1.0, recolor=None):
    """draw_pets 얼굴 ops [(fns, color)] → 층 항목."""
    return [(fns, (recolor or {}).get(color, color), alpha, 0, 0) for fns, color in ops]


def _finish(parts, glow=(), face=(), fx=(), translucent=None, circle=None):
    """몸(compose) → (반투명) → 몸 안쪽으로 잘린 빛 층 → 얼굴·효과 층. circle: (cx, cy, r) 밖은 지운다 (roll)."""
    out = dp.compose(parts)
    a_body = out.getchannel("A")
    if translucent:
        a = np.asarray(a_body).astype(np.float32)
        inner = np.asarray(a_body.filter(ImageFilter.MinFilter(5))) >= 250
        a[inner] *= translucent
        out.putalpha(Image.fromarray(np.round(a).astype(np.uint8)))
    if glow:
        g = _layer(glow)
        g.putalpha(ImageChops.multiply(g.getchannel("A"), a_body.filter(ImageFilter.MinFilter(3))))
        out.alpha_composite(g)
    items = list(face)
    for fns, color, halo in fx:
        if halo:
            items.append((fns, WHITE, 1.0, halo, 0))
        items.append((fns, color, 1.0, 0, 0))
    if items:
        out.alpha_composite(_layer(items))
    if circle:
        cx, cy, r = circle
        m = Image.new("L", (S, S), 0)
        from PIL import ImageDraw
        ImageDraw.Draw(m).ellipse(((cx - r) * K, (cy - r) * K, (cx + r) * K, (cy + r) * K), fill=255)
        out.putalpha(ImageChops.multiply(out.getchannel("A"), m.resize((FRAME, FRAME), Image.LANCZOS)))
    out.putalpha(out.getchannel("A").point(lambda v: 0 if v < 10 else v))
    return out


def _arm(sp, sh, side, a, ln, w):
    """짤막한 팔 (자기 외곽선). a: 아래(0)에서 바깥쪽으로 벌린 각도 | (x, y) 손끝."""
    if isinstance(a, (int, float)):
        r = math.radians(a)
        tip = (sh[0] + side * math.sin(r) * ln, sh[1] + math.cos(r) * ln)
    else:
        tip = a
    return Part([L([sh, tip], w)], sp.base, outline=True, grow=0)


def _heart_items(hx, hy, k=1.0):
    """품에 안은 하트 — 빛 층 위에 얹는 얼굴 층 항목 (배 속 빛에 묻히지 않게)."""
    fns = [C(hx - 6.5 * k, hy, 7.2 * k), C(hx + 6.5 * k, hy, 7.2 * k), T((hx - 13.2 * k, hy + 2 * k), (hx + 13.2 * k, hy + 2 * k), (hx, hy + 15 * k))]
    return [(fns, LINE, 1.0, 1.2, 0), (fns, HEART, 1.0, 0, 0), ([E(hx - 10.5 * k, hy - 4.5 * k, hx - 6.5 * k, hy - 1 * k)], WHITE, 1.0, 0, 0)]


# =====================================================================
# 호박 🎃
# =====================================================================
def pk_eye_fns(kind, cx, cy, flip=False):
    """조각한 눈 모양 (fns, 반짝이 fns). dot = 둥근 세모 · up = 위로 올려다보는 세모 (lookup)."""
    if kind == "up":
        return pk_eye_fns("dot", cx, cy - 1.8, flip)
    if kind in ("dot", "big"):
        s = 1.22 if kind == "big" else 1.0
        tri = [(cx, cy - 6.4 * s), (cx - 6.6 * s, cy + 4.4 * s), (cx + 6.6 * s, cy + 4.4 * s)]
        fns = [T(*tri), L(tri + [tri[0]], 3.0 * s)]
        return fns, [C(cx - 2.2 * s, cy + 0.4 * s, 1.7 * s), C(cx + 2.4 * s, cy + 3.0 * s, 0.9 * s)]
    if kind == "round":
        return [C(cx, cy, 6.4)], [C(cx - 2.2, cy - 2.0, 1.8), C(cx + 2.2, cy + 2.4, 0.9)]
    if kind == "happy":
        return [L(arc(cx, cy + 3.4, 5.4, 5.4, 205, 335), 3.4)], []
    if kind == "closed":
        return [L(arc(cx, cy - 1.8, 5.6, 4.2, 25, 155), 3.4)], []
    if kind == "tight":
        s = -1 if flip else 1
        return [L([(cx - 4 * s, cy - 3.8), (cx + 3.2 * s, cy), (cx - 4 * s, cy + 3.8)], 3.0)], []
    if kind == "squint":
        return [L([(cx - 5.2, cy + 1), (cx, cy + 0.2), (cx + 5.2, cy + 1)], 3.0)], []
    if kind == "spiral":
        pts = [(cx + 0.6 * t * math.cos(t), cy + 0.6 * t * math.sin(t)) for t in (k * 0.35 for k in range(1, 30))]
        return [L(pts, 2.2)], []
    raise ValueError(kind)


def pk_mouth_fns(kind, mx, my):
    """조각한 입. None = 이 하나씩 들쭉날쭉한 웃는 입 (귀엽게 짧게)."""
    if kind is None or kind == "big":
        w, d = (13.0, 9.0) if kind is None else (14.5, 12.0)
        top = lambda u: my - 2.6 + 2.6 * (1 - u * u)
        bot = lambda u: my - 2.6 + d * (1 - u * u) ** 0.8
        us = [k / 10 - 1 for k in range(21)]
        up = [(mx + u * w, top(u)) for u in us]
        dn = [(mx + u * w, bot(u)) for u in reversed(us)]
        if kind is None:  # 위 이 하나 (왼쪽), 아래 이 하나 (오른쪽)
            t0, t1 = -0.42, -0.14
            up = [p for p in up if not (t0 * w < p[0] - mx < t1 * w)]
            k = next(j for j, p in enumerate(up) if p[0] - mx >= t1 * w)
            up[k:k] = [(mx + t0 * w, top(t0) + 3.2), (mx + t1 * w, top(t1) + 3.2)]
            b0, b1 = 0.16, 0.44
            dn = [p for p in dn if not (b0 * w < p[0] - mx < b1 * w)]
            k = next(j for j, p in enumerate(dn) if p[0] - mx <= b0 * w)
            dn[k:k] = [(mx + b1 * w, bot(b1) - 3.2), (mx + b0 * w, bot(b0) - 3.2)]
        return [T(*(up + dn))]
    if kind == "open":
        return [E(mx - 4.6, my - 3, mx + 4.6, my + 6)]
    if kind == "yawn":
        return [E(mx - 4.6, my - 3, mx + 4.6, my + 9)]
    if kind == "sip":
        return [E(mx - 3, my - 1.5, mx + 3, my + 3.5)]
    if kind == "frown":
        return [L(arc(mx, my + 4, 5.2, 3.4, 205, 335, 10), 3.0)]
    if kind == "pout":
        return [L(arc(mx + 1, my + 3.4, 3.4, 2.4, 205, 335, 8), 3.0)]
    if kind == "chew":
        return [L([(mx - 6, my), (mx - 3, my + 2.4), (mx, my), (mx + 3, my + 2.4), (mx + 6, my)], 2.8)]
    if kind == "wavy":
        return [L([(mx - 6, my + 1), (mx - 3, my - 1), (mx, my + 1), (mx + 3, my - 1), (mx + 6, my + 1)], 2.6)]
    raise ValueError(kind)


def _carve(make, center, lit=1.0):
    """조각해서 빛나는 구멍 → (빛 층 항목, 얼굴 층 항목). make(): 현재 변환 아래 fns. lit < 0.5 = 잉걸불."""
    fns = make()
    glow = [(fns, HALO, min(1.0, 1.0 * lit), 3.4, 3.2)]
    if lit < 0.5:
        return glow, [(fns, CARVE, 1.0, 1.1, 0), (fns, EMBER, 1.0, 0, 0)]
    with xf(sx=0.5, sy=0.5, pivot=center):
        core = make()
    return glow, [(fns, CARVE, 1.0, 1.1, 0), (fns, GLOW_Y, 1.0, 0, 0), (core, GLOW_CORE, 0.9 * lit, 0, 0.5)]


def pk_face(fx_, fy, eyes, mouth, brow, lit, side):
    """호박 얼굴 (빛 층, 얼굴 층). side: 3/4 옆모습 (먼 눈은 좁게)."""
    fs = _fs()
    glow, face = [], []
    with xf(sx=fs, sy=fs, pivot=(fx_, fy)):
        es = _ST["eye"]
        sp_x = 12.5 if not side else 11.0
        for j, (ex, flip) in enumerate(((fx_ - sp_x, False), (fx_ + sp_x, True))):
            ey = fy - 5 + _ST["eye_dy"] * 0.5
            k = es if eyes in ("dot", "big", "round", "up") else es ** 0.6
            kx = k * (0.84 if side and j == 0 else 1.0)
            with xf(sx=kx, sy=k, pivot=(ex, ey)):
                ek = eyes if not (lit < 0.5 and eyes == "dot") else "closed"
                g, f = _carve(lambda: pk_eye_fns(ek, ex, ey, flip)[0], (ex, ey), lit)
                glow += g
                face += f
                gl = pk_eye_fns(ek, ex, ey, flip)[1]
                if gl and lit >= 0.5:
                    face.append((gl, WHITE, 1.0, 0, 0))
            if brow:
                face += _ops(brows(brow, ex, ey - 1 + _ST["eye_dy"] * 0.5, flip=flip), recolor={dp.EYE: CARVE})
        bx, by = _ST["blush"]
        for cx_ in (fx_ - 21, fx_ + 21):
            if side and cx_ < fx_:
                cx_ += 3
            glow.append(([E(cx_ - 6 * bx, fy + 4 - 3.8 * by, cx_ + 6 * bx, fy + 4 + 3.8 * by)], CHEEK, 0.55, 0, 0.4))
        mx, my = fx_ + (1 if side else 0), fy + 9
        g, f = _carve(lambda: pk_mouth_fns(mouth, mx, my), (mx, my + 2), lit)
        glow += g
        face += f
    return glow, face


def pk_stem_parts(sp, base, rot=0.0, trim=False):
    """꼭지 (꼬불) + 잎 + 덩굴손 · 어른은 작은 마녀 모자. base: 몸 윗선 가운데."""
    st = _pk()
    bx, by = base
    out = []
    if st["hat"] and not trim:
        with xf(rot=-12 + rot * 0.4, pivot=(bx, by + 2)):
            brim = E(bx - 20, by - 3, bx + 20, by + 5)
            cone = T((bx - 11.5, by + 1), (bx + 11.5, by + 1), (bx + 6, by - 16), (bx + 9, by - 22), (bx + 15, by - 24), (bx + 2.5, by - 21))
            out.append(Part([brim, cone], HAT, [([T((bx - 11, by - 3), (bx + 11, by - 3), (bx + 10, by + 0.5), (bx - 10, by + 0.5))], HAT_BAND),
                                                ([L([(bx - 5, by - 7), (bx - 1, by - 16)], 1.6)], HAT_LIGHT)], outline=True, grow=0))
            out.append(Part([C(bx + 15.5, by - 24, 2.6)], HAT_BAND, outline=True, grow=0))
        return out
    s = st["stem"] * (0.7 if trim else 1.0)
    with xf(rot=rot, pivot=(bx, by + 3)):
        pts = _bez((bx, by + 4), (bx + 0.5 * s, by - 6 * s), (bx + 6 * s, by - 12 * s))
        out.append(Part([strand(pts, 9 * s, 5.4 * s), C(bx + 6.2 * s, by - 12 * s, 2.8 * s)], STEM,
                        [([L([(bx - 1.2 * s, by), (bx - 0.6 * s, by - 6 * s)], 1.2)], STEM_DARK)], outline=True, grow=0))
        if st["leaf"] and not trim:
            with xf(rot=-28, pivot=(bx + 2, by + 1)):
                lf = [E(bx + 2, by - 5.5, bx + 19, by + 3.5)]
                out.append(Part(lf, LEAF, [([L([(bx + 3, by - 1), (bx + 16, by - 1)], 1.1)], LEAF_VEIN)], outline=True, grow=0))
        c = st["curl"]
        if c and not trim:
            p0 = (bx - 3, by + 1)
            pts = _bez(p0, (bx - 8 * c, by - 3 * c), (bx - 13 * c, by - 2 * c), 8) + \
                arc(bx - 13 * c, by - 6.5 * c, 4.5 * c, 4.5 * c, 90, 400, 14)
            out.append(Part([L(pts, 2.0)], LEAF_VEIN, outline=False, grow=0, sharp=True))
    return out


def pumpkin(sp, *, side=False, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None,
            arms=None, feet=((0, 0), (0, 0)), stem=0.0, lit=1.0, fx=(), held=(), props=(), trim=False, size=1.0, circle=None, top=()):
    """호박 한 장. bottom: 몸 아래끝 y (발은 그 아래 6px까지) · sq: 아래끝 기준 찌그러짐 · rot: 몸 중심 기준 기울기 ·
    arms: [(side, 각도 | 손끝)] (None = 기본 양팔) · feet: (왼, 오) 발 위치 오프셋 또는 None · stem: 꼭지 흔들림(도) ·
    lit: 불빛 세기 (< 0.5 잉걸불) · trim: 공 모양 (팔·발·모자·잎 없음)."""
    st = _pk()
    rx, ry = st["rx"] * size, st["ry"] * size
    bottom = FLOOR - 6 if bottom is None else bottom
    cy = bottom - ry
    fx_ = cx + (0.24 * rx if side else 0.0)
    fy = cy + 0.02 * ry
    parts, glow, face = [], [], []
    with xf(rot=rot, pivot=(cx, cy)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
            lobes = [E(cx - 0.56 * rx, cy - ry, cx + 0.56 * rx, cy + ry),
                     E(cx - rx, cy - 0.9 * ry, cx + 0.12 * rx, cy + 0.94 * ry), E(cx - 0.12 * rx, cy - 0.9 * ry, cx + rx, cy + 0.94 * ry)]
            rc = mix(sp.base, PUMP_RIDGE, st["ridge"])
            ridges = []
            ks = (-0.46, 0.46) + ((-0.82, 0.82) if _ST["lvl"] >= 3 else ())
            for k in ks:
                kk = k if not side else (k * 0.85 + (0.18 if k > 0 else 0.2))
                mid = (cx + kk * rx, cy)
                top_x = cx + kk * 0.5 * rx
                ridges.append(L(_bez((top_x, cy - 0.9 * ry), mid, (top_x, cy + 0.92 * ry)), 1.7 if abs(k) < 0.6 else 1.3))
            marks = [([E(cx - 1.15 * rx, cy + 0.5 * ry, cx + 1.15 * rx, cy + 1.9 * ry)], mix(sp.base, PUMP_RIDGE, 0.35)),
                     (ridges, rc),
                     ([E(cx - 0.66 * rx, cy - 0.7 * ry, cx - 0.36 * rx, cy - 0.44 * ry)], mix(sp.base, WHITE, 0.55)),
                     ([C(cx - 0.3 * rx, cy - 0.66 * ry, 1.6)], mix(sp.base, WHITE, 0.7))]
            parts.append(Part(lobes, sp.base, marks, grow=1))
            # 안에서 비치는 따뜻한 빛 (얼굴 둘레)
            glow.append(([E(fx_ - 0.7 * rx, fy - 0.55 * ry, fx_ + 0.7 * rx, fy + 0.75 * ry)], INNER, 0.32 * lit, 0, 7))
            g, f = pk_face(fx_, fy, eyes, mouth, brow, lit, side)
            glow += g
            face += f
            if feet and not trim:
                xs = (cx - 0.24 * rx, cx + 0.3 * rx) if side else (cx - 0.42 * rx, cx + 0.42 * rx)
                for x, (dx, dy) in zip(xs, feet):
                    if dy > 4:  # 까치발: 몸에서 발까지 짧은 다리
                        parts.append(Part([L([(x, bottom - 6), (x + dx, bottom + dy)], 6.5)], PUMP_FOOT, outline=True, grow=0))
                    parts.append(Part([E(x + dx - 7.5, bottom - 4 + dy, x + dx + 7.5, bottom + 6 + dy)], PUMP_FOOT, outline=True, grow=0))
            if arms is None:
                arms = [(1, 30)] if side else [(-1, 34), (1, 34)]
            if not trim:
                for s_, a in arms:
                    if side:
                        sh = (cx + 0.8 * rx, cy + 0.42 * ry)
                    else:
                        sh = (cx + s_ * 0.86 * rx, cy + 0.22 * ry)
                    parts.append(_arm(sp, sh, s_, a, 8.5, 8.0))
            parts += [h() for h in held]
            parts += pk_stem_parts(sp, (cx + (0.1 * rx if side else 0.0), cy - ry + 3), stem, trim)
    parts += [p() for p in props]
    return _finish(parts, glow, face + list(top), fx, circle=circle)


# =====================================================================
# 유령 👻
# =====================================================================
def gh_face(fcx, fcy, eyes, mouth, brow, side):
    """→ (볼터치: 몸 안으로 잘리는 층, 눈·입 층)."""
    fs = _fs()
    ops = []
    with xf(sx=fs, sy=fs, pivot=(fcx, fcy)):
        sp_x = 11.5 if not side else 10.0
        bd = 1.5 if _ST["sparkle"] else 0
        cheeks = dp.blush(fcx - 19 + (4 if side else 0), fcy + 10 + bd) + dp.blush(fcx + 19, fcy + 10 + bd)
        for j, (ex, flip) in enumerate(((fcx - sp_x, False), (fcx + sp_x, True))):
            k = GH_EYE * (0.86 if side and j == 0 else 1.0)
            with xf(sx=k, sy=GH_EYE, pivot=(ex, fcy)):
                ops += stage_eye(eyes, ex, fcy, flip)
            if brow:
                ops += brows(brow, ex, fcy - 2 + _ST["eye_dy"], flip=flip)
        ops += bee_mouth(mouth, fcx + (1 if side else 0), fcy + 10)
    return _ops(cheeks), _ops(ops)


def ghost(sp, *, side=False, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None,
          arms=None, phase=0, rest=False, trail=1.0, lit=1.0, fx=(), held=(), props=(), trim=False, size=1.0, circle=None,
          face_dy=0.0, top=(), short=False):
    """유령 한 장. bottom: 치맛단 아래끝 y (기본: 떠 있음 112 · rest = 바닥에 내려앉음) · phase: 단 물결 ·
    trail: 옆모습 치마 뒤로 날림 · arms: [(side, 각도 | 손끝)]."""
    st = _gh()
    r = st["r"] * size
    # 쉬는 동작·공 모양은 짧게 · short: 원래 길이 (eat — 입이 그릇 위에 고정이라 긴 치마는 칸 밖으로 나간다, 단은 그릇이 가림)
    Ln = size * (st["L"] * 0.82 if rest else st["L"] if (trim or short) else _skirt(st))
    br = r * 0.3
    if bottom is None:
        bottom = FLOOR - 0.5 if rest else 112.0
    hem = bottom - br
    cy = hem - Ln
    parts, glow = [], []
    with xf(rot=rot, pivot=(cx, cy + Ln * 0.4)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
            n = st["n"]
            amp = 0.6 if rest else 2.2
            if side:
                back = r * (1.25 + 0.25 * trail)
                lx, rxx = cx - back, cx + r * 0.95
                tail_up = 6 * trail
                skirt = T((cx - r * 0.98, cy), (lx + br, hem - tail_up - br * 0.6), (rxx - br * 0.4, hem - br * 0.3), (cx + r, cy))
                bumps = []
                for j in range(n + 1):
                    t = j / n
                    x = lx + br + (rxx - lx - 1.6 * br) * t
                    y = hem - tail_up * (1 - t) ** 1.6 - br * 0.25 + amp * math.sin(phase * math.pi / 2 + j * 2.1)
                    bumps.append(C(x, y, br * (0.92 if j else 0.8)))
            else:
                flare = r * 1.12
                skirt = T((cx - r, cy), (cx - flare, hem - br * 0.3), (cx + flare, hem - br * 0.3), (cx + r, cy))
                bumps = []
                for j in range(n):
                    t = j / (n - 1)
                    x = cx - flare + br + (2 * flare - 2 * br) * t
                    y = hem - br * 0.25 + amp * math.sin(phase * math.pi / 2 + j * 2.1)
                    bumps.append(C(x, y, br))
            head = C(cx + (3 if side else 0), cy, r)
            shade = [([E(cx - r * 1.4, hem - br * 0.2, cx + r * 1.4, hem + br * 2.4)], GHOST_SHADE)]
            spikes = []
            if st["curl"] and not trim:
                tx, ty = cx + (5 if side else 0), cy - r
                spikes = [strand([(tx - 2, ty + 5)] + arc(tx + 4, ty - 3.5, 5, 5.5, 180, 330, 10), 6.0, 2.2)]
            parts.append(Part([head, skirt] + bumps, sp.base, shade, grow=1, spikes=spikes))
            # 배 속 따뜻한 빛
            gx, gy = cx - (4 if side else 0), cy + Ln * 0.62
            glow += [([E(gx - r * 0.72, gy - r * 0.6, gx + r * 0.72, gy + r * 0.62)], WARM, 0.8 * lit, 0, 5.5),
                     ([E(gx - r * 0.36, gy - r * 0.3, gx + r * 0.36, gy + r * 0.32)], WARM_CORE, 0.75 * lit, 0, 3.5)]
            fcx, fcy = cx + (0.36 * r if side else 0), cy + 2 + face_dy
            cheeks, face = gh_face(fcx, fcy, eyes, mouth, brow, side)
            glow += cheeks
            if arms is None:
                arms = [(1, 62)] if side else [(-1, 68), (1, 68)]
            if not trim:
                ak = st["arm"]
                for s_, a in arms:
                    if side:
                        sh = (cx + 0.86 * r, cy + 0.85 * r)
                    else:
                        sh = (cx + s_ * 0.94 * r, cy + 0.82 * r)
                    parts.append(_arm(sp, sh, s_, a, 9 * ak, 9.5 * ak))
            parts += [h() for h in held]
            if st["bow"] and not trim:
                bx, by = cx + 0.55 * r + (3 if side else 0), cy - 0.8 * r
                with xf(rot=24, pivot=(bx, by)):
                    parts.append(Part([T((bx, by), (bx - 10, by - 6), (bx - 10, by + 6)), T((bx, by), (bx + 10, by - 6), (bx + 10, by + 6)),
                                       C(bx, by, 3.4)], BOW, [([E(bx - 8.5, by - 3.5, bx - 4.5, by - 0.5)], BOW_LIGHT)], outline=True, grow=0,
                                      sharp=True))
    parts += [p() for p in props]
    return _finish(parts, glow, face + list(top), fx, translucent=GHOST_ALPHA, circle=circle)


# =====================================================================
# 효과
# =====================================================================
def _bubble(cx, cy, r=12.5):
    fx = []
    for x, y, rr in [(cx - r - 7, cy + r + 8, 2.0), (cx - r - 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], LINE, 1.0), ([C(x, y, rr)], WHITE, 0)]
    return fx


def candy_bubble_fx(cx, cy):
    """생각 풍선 안에 포장 사탕."""
    fx = _bubble(cx, cy)
    wr = [T((cx - 4, cy), (cx - 10, cy - 4.5), (cx - 10, cy + 4.5)), T((cx + 4, cy), (cx + 10, cy - 4.5), (cx + 10, cy + 4.5))]
    fx += [(wr, LINE, 0.9), (wr, CANDY_WRAP, 0), ([C(cx, cy, 6.2)], LINE, 0), ([C(cx, cy, 5.0)], CANDY, 0),
           ([L([(cx - 3, cy + 3), (cx + 3, cy - 3)], 1.4)], CANDY_STRIPE, 0)]
    return fx


def lolly_bubble_fx(cx, cy):
    """생각 풍선 안에 막대사탕."""
    fx = _bubble(cx, cy)
    spiral = [(cx + 0.55 * t * math.cos(t), cy - 2 + 0.55 * t * math.sin(t)) for t in (k * 0.4 for k in range(2, 22))]
    fx += [([L([(cx, cy + 2), (cx, cy + 10)], 3.2)], LINE, 0), ([L([(cx, cy + 2), (cx, cy + 10)], 1.6)], WHITE, 0),
           ([C(cx, cy - 2, 6.6)], LINE, 0), ([C(cx, cy - 2, 5.4)], LOLLY, 0), ([L(spiral, 1.2)], LOLLY_SWIRL, 0)]
    return fx


def speed_fx(x0, ys, ln, shift=0.0):
    return [([L([(x0 + shift, y), (x0 + ln + shift, y)], 1.8)], SPEED, 0) for y in ys]


def _stars(items):
    return [f for x, y, r in items for f in sparkle_fx(x, y, r)]


def _swing(v):
    return dp._swing(v)


# =====================================================================
# 포즈 — 호박(통통 튀는 공)과 유령(둥실)이 같은 이름 아래 각자 몸짓
# =====================================================================
def _is_ghost(sp):
    return sp.id == "ghost"


def walk(sp, i):
    if _is_ghost(sp):
        return ghost(sp, side=True, bottom=112 - [0, 1.5, 3, 1.5][i], rot=5, phase=i, arms=[(1, [40, 52, 64, 52][i])])
    # 통통: 0 바닥에 찌그러짐 → 1 솟음 (길쭉) → 2 꼭대기 (동글, ^^) → 3 내려옴
    up = [0, 5, 8, 4][i]
    sq = [(1.08, 0.9), (0.95, 1.06), (1.0, 1.0), (0.98, 1.03)][i]
    feet = ((0, 0), (0, 0)) if i == 0 else ((-2, 1), (2, -1))
    return pumpkin(sp, side=True, bottom=FLOOR - 6 - up, sq=sq, rot=[0, -4, -2, 2][i], stem=[8, -8, -12, 2][i],
                   eyes="happy" if i == 2 else "dot", feet=feet, arms=[(1, [20, 60, 80, 40][i])], lit=FLICK[i])


def run(sp, i):
    if _is_ghost(sp):
        return ghost(sp, side=True, bottom=110 - [0, 1, 0, 1][i], rot=12, phase=i, trail=1.6, mouth="open" if i % 2 else None,
                     arms=[(1, 100)], fx=speed_fx(2, (52, 70, 88), 14, [0, -3, 0, -3][i]))
    up = [0, 7, 11, 5][i]
    sq = [(1.1, 0.88), (0.93, 1.08), (0.98, 1.02), (0.96, 1.05)][i]
    return pumpkin(sp, side=True, bottom=FLOOR - 6 - up, sq=sq, rot=[2, -8, -6, -2][i], stem=[10, -10, -16, 0][i],
                   eyes="happy", mouth="open", feet=((-3, 0), (3, 0)) if i == 0 else ((-4, 2), (4, -2)), arms=[(1, 95)],
                   lit=FLICK[i], fx=speed_fx(2, (74, 92, 108), 12, [0, -3, 0, -3][i]))


def roll(sp, i):
    """공처럼 데굴 (웹뷰가 (64, 88) 기준으로 돌린다 — 원 안에만)."""
    cx, cy = ROLL_CENTER
    circle = (cx, cy, ROLL_RADIUS - 0.6)
    with view(scale=1.0, lift=0):
        if _is_ghost(sp):
            st = _gh()
            k = 31 / (st["r"] + st["L"] * 0.5)
            return ghost(sp, bottom=cy + 30, size=k * 1.25, sq=(1.0, 0.8), rot=[-6, 6][i], eyes="happy", mouth="open" if i else None,
                         arms=[], trim=True, phase=i, circle=circle)
        st = _pk()
        k = 33 / st["rx"]
        return pumpkin(sp, bottom=cy + 33 * st["ry"] / st["rx"] - 2, size=k, rot=[-6, 6][i], eyes="happy",
                       mouth="open" if i else None, trim=True, feet=None, arms=[], stem=[-8, 8][i], circle=circle)


def idle(sp, i):
    if _is_ghost(sp):
        return ghost(sp, side=True, bottom=112 - 1.5 * i, phase=i * 2, arms=[(1, 50 + 6 * i)])
    return pumpkin(sp, side=True, sq=(1.0, 1.0 + 0.02 * i), stem=[0, 4][i], lit=FLICK[i * 2])


def look(sp, i):
    if _is_ghost(sp):
        return ghost(sp, side=True, rot=[-9, 0, 9, 0][i], phase=i, eyes="closed" if i == 3 else "dot")
    return pumpkin(sp, side=True, rot=[-8, 0, 8, 0][i], stem=[-6, 0, 6, 0][i], eyes="closed" if i == 3 else "dot", lit=FLICK[i])


def lookup(sp, i):
    """올려다보기: 몸째 살짝 젖히고 눈은 위로 (유령은 떠서)."""
    if _is_ghost(sp):
        return ghost(sp, side=True, rot=dp.lookup_tilt(i, 0.55), phase=i, eyes="up", fx=dp.lookup_fx(i, 112, 14))
    return pumpkin(sp, side=True, rot=dp.lookup_tilt(i, 0.5), stem=-4, eyes="up", lit=FLICK[i], fx=dp.lookup_fx(i, 112, 16))


def sleep(sp, i):
    if _is_ghost(sp):
        zs = [z_fx(104, 66, 3.4), z_fx(115, 55, 2.6)] if i == 0 else [z_fx(106, 63, 3.6), z_fx(118, 51, 2.8)]
        return ghost(sp, side=True, rest=True, sq=(1.08, 0.86 + 0.012 * i), eyes="closed", arms=[(1, 20)], lit=0.7, fx=zs, phase=0)
    zs = [z_fx(108, 62, 3.4), z_fx(119, 51, 2.6)] if i == 0 else [z_fx(110, 59, 3.6), z_fx(121, 47, 2.8)]
    return pumpkin(sp, side=True, sq=(1.06, 0.92 + 0.012 * i), eyes="closed", feet=((2, -2), (-2, -2)), arms=[(1, 10)],
                   lit=0.3 + 0.06 * i, fx=zs)


def annoyed(sp, i):
    if _is_ghost(sp):
        return ghost(sp, side=True, rest=True, sq=(1.04, 0.92), eyes="squint", mouth="pout", arms=[(1, 15)],
                     fx=[anger_fx(104, 40 - i, 7.0)])
    return pumpkin(sp, side=True, sq=(1.04, 0.95), eyes="squint", mouth="pout", arms=[(1, 15)], lit=FLICK[i],
                   fx=[anger_fx(106, 44 - i, 7.0)])


def hop(sp, i):
    """한 번 통! 0 웅크림 → 1 쭉 솟음 → 2 공중에서 ^^ (코어가 깡총마다 다시 튼다)."""
    if _is_ghost(sp):
        if i == 0:
            return ghost(sp, side=True, bottom=116, sq=(1.08, 0.88), eyes="tight", phase=0)
        if i == 1:
            lines = [([L([(x, 110), (x, 122)], 1.8)], SPEED, 0) for x in (46, 60, 74)]
            return ghost(sp, side=True, bottom=114, sq=(0.97, 1.02), eyes="happy", mouth="open", arms=[(1, 120)], phase=1, fx=lines)
        return ghost(sp, side=True, bottom=109, eyes="happy", mouth="open", arms=[(1, 90)], phase=2)
    if i == 0:
        return pumpkin(sp, side=True, sq=(1.1, 0.86), eyes="tight", stem=10, feet=((-2, 0), (2, 0)), arms=[(1, 15)])
    if i == 1:
        return pumpkin(sp, side=True, bottom=FLOOR - 20, sq=(0.92, 1.1), rot=-6, eyes="happy", mouth="open", stem=-14,
                       feet=((-3, 3), (3, 1)), arms=[(1, 120)], lit=FLICK[1])
    return pumpkin(sp, side=True, bottom=FLOOR - 10, sq=(1.02, 0.98), rot=3, eyes="happy", stem=6, feet=((-2, 1), (2, 0)),
                   arms=[(1, 70)], lit=FLICK[2])


def fall(sp, i):
    stars = _stars([(100, 22, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 20, 3.6), (104, 32, 3.0)])
    if _is_ghost(sp):
        return ghost(sp, side=True, bottom=110, rot=[-14, 14][i], eyes="spiral", mouth="wavy", arms=[(1, [150, 120][i])],
                     phase=i * 2, fx=stars)
    return pumpkin(sp, side=True, bottom=FLOOR - 12, rot=[-12, 12][i], eyes="spiral", mouth="wavy", stem=[-16, 16][i],
                   feet=((-4, 2), (4, -3)) if i == 0 else ((-4, -3), (4, 2)), arms=[(1, [150, 110][i])], fx=stars)


def dragged(sp, i):
    """잡혀서 대롱대롱: 호박은 꼭지를 잡혀서 · 유령은 머리 꼭대기를 잡혀서 — 발·치마가 아래로 축, 땀."""
    s = [-1, 1][i]
    if _is_ghost(sp):
        return ghost(sp, side=True, bottom=122, sq=(0.94, 1.05), rot=4 * s, eyes="tight", mouth="open", arms=[(1, 160)], phase=i,
                     trail=0.4, fx=sweat_fx(100, 34))
    return pumpkin(sp, side=True, bottom=FLOOR - 14, rot=5 * s, eyes="tight", mouth="open", stem=-4 * s,
                   feet=((-3 + 2 * s, 4), (3 + 2 * s, 5)), arms=[(1, 150)], fx=sweat_fx(104, 40))


def land(sp, i):
    if _is_ghost(sp):
        if i == 0:
            return ghost(sp, side=True, rest=True, sq=(1.12, 0.84), eyes="tight", arms=[(1, 100)])
        return ghost(sp, side=True, rest=True, arms=[(1, 50)], phase=1)
    if i == 0:
        return pumpkin(sp, side=True, sq=(1.12, 0.84), eyes="tight", stem=-12, arms=[(1, 80)])
    return pumpkin(sp, side=True, sq=(1.02, 0.98), stem=6, lit=FLICK[1])


# ---- 앞모습 ----
def _front(sp, **kw):
    return ghost(sp, **kw) if _is_ghost(sp) else pumpkin(sp, **kw)


def _kw(sp, i, **kw):
    """종별 기본: 유령은 단 물결 phase, 호박은 불빛 일렁임 lit."""
    if _is_ghost(sp):
        kw.setdefault("phase", i)
        kw.pop("stem", None)
        kw.pop("feet", None)
    else:
        kw.setdefault("lit", FLICK[i % 4])
        kw.pop("phase", None)
    return kw


def _bob(sp, d):
    """떠 있는 유령은 아래끝을 d만큼 들고, 호박은 바닥 그대로."""
    return {"bottom": 112 - d} if _is_ghost(sp) else {}


def sit_front(sp, i):
    return _front(sp, **_kw(sp, i, **_bob(sp, [0, 1, 2, 1][i]), sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]),
                            eyes="closed" if i == 3 else "dot", stem=[0, 3, 0, -3][i]))


def wave(sp, i):
    up = [140, 165, 140, 165][i] if _is_ghost(sp) else [150, 175, 150, 175][i]
    return _front(sp, **_kw(sp, i, arms=[(-1, 30), (1, up)], rot=[-3, -1, -3, -1][i], stem=[-4, 4, -4, 4][i]))


def tilt(sp, i):
    return _front(sp, **_kw(sp, i, **_bob(sp, 3), rot=_swing([8, 12, 12][i]), stem=6, fx=[question_fx(18, 22 + [2, 0, -2][i])]))


def belly(sp, i):
    """발라당: 호박은 납작 주저앉아 짧은 발을 양옆 위로 버둥, 유령은 바닥에 납작 엎드려 팔을 하늘로 꼬물. 얼굴 ^o^."""
    wig = [0, 2, 0, -2][i]
    if _is_ghost(sp):
        # 바닥에 납작 엎드려 (치마를 깔고) 팔을 하늘로 꼬물꼬물 ^o^
        return ghost(sp, rest=True, sq=(1.16, 0.8), rot=[-4, 0, 4, 0][i], eyes="happy", mouth="open",
                     arms=[(-1, 150 + 6 * wig), (1, 150 - 6 * wig)], phase=i)
    rx = _pk()["rx"]
    return pumpkin(sp, sq=(1.1, 0.86), rot=-8, eyes="happy", mouth="open", feet=((-0.8 * rx, -26 + wig), (0.8 * rx, -28 - wig)),
                   arms=[(-1, 140 + 8 * wig), (1, 140 - 8 * wig)], stem=-20 + 3 * wig, lit=FLICK[i])


def stretch(sp, i):
    if i == 0:
        return _front(sp, **_kw(sp, i, **_bob(sp, -3), sq=(0.96, 1.03) if _is_ghost(sp) else (0.94, 1.08), eyes="tight", arms=[(-1, 170), (1, 170)], stem=-6))
    if i == 1:
        return _front(sp, **_kw(sp, i, eyes="tight", mouth="yawn", arms=[(-1, 70), (1, 70)], rot=-3, stem=4))
    return _front(sp, **_kw(sp, i))


def heart(sp, i):
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    if _is_ghost(sp):
        st = _gh()
        hy = 112 - st["r"] * 0.3 - _skirt(st) + min(st["r"] * 0.95, st["L"] - 9)  # 아기는 치마가 짧아 조금 위로
        return ghost(sp, eyes="happy", phase=i, top=_heart_items(64, hy, 0.9),
                     arms=[(-1, (56, hy + 6)), (1, (72, hy + 6))], fx=hs)
    st = _pk()
    hy = FLOOR - 6 - st["ry"] + st["ry"] * 0.55 + 3
    return pumpkin(sp, eyes="happy", lit=FLICK[i], top=_heart_items(64, hy, 0.9),
                   arms=[(-1, (55, hy + 6)), (1, (73, hy + 6))], fx=hs)


def tickle(sp, i):
    return _front(sp, **_kw(sp, i, rot=_swing([-7, 7, 0][i]), eyes="happy", mouth="open",
                            arms=[(-1, [60, 110, 85][i]), (1, [110, 60, 85][i])], stem=[-10, 10, 0][i]))


def hungry(sp, i):
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    if _is_ghost(sp):
        st = _gh()
        cy = 114 - st["r"] * 0.3 - _skirt(st)
        return ghost(sp, bottom=114, rot=-4, eyes="closed", mouth="frown", brow="sad", phase=i,
                     arms=[(-1, 25), (1, (62 + dx, cy + _skirt(st) * 0.5 + dy))], fx=lolly_bubble_fx(106, 18))
    st = _pk()
    cy = FLOOR - 6 - st["ry"]
    return pumpkin(sp, rot=-4, eyes="closed", mouth="frown", brow="sad", lit=0.75, stem=-10,
                   arms=[(-1, 20), (1, (70 + dx, cy + st["ry"] * 0.5 + dy))], fx=candy_bubble_fx(106, 18))


def eat(sp, i):
    """냠냠 (그릇 없음): 입이 그릇 밥 더미 바로 위 (프레임 (64, 84))에 오도록 몸을 낮추고/올리고, 앙 ↔ 오물오물."""
    bite = i % 2 == 0
    fs = _fs()
    mx, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    eyes, mouth = ("happy", "open") if bite else ("closed", "chew")
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    paws = [dp._frame_to_pre(44, 98), dp._frame_to_pre(84, 98)]  # 그릇 가장자리를 두 손으로
    if _is_ghost(sp):
        st = _gh()
        cy = my - 10 * fs - 2  # 입 = 얼굴 중심 + 10·fs
        bottom = cy + st["L"] + st["r"] * 0.3
        return ghost(sp, bottom=bottom, short=True, eyes=eyes, mouth=mouth, phase=i, rot=[0, -2, 0, 2][i],
                     arms=[(-1, paws[0]), (1, paws[1])], fx=fx)
    st = _pk()
    fy = my - 9 * fs
    bottom = fy - 0.02 * st["ry"] + st["ry"]
    return pumpkin(sp, bottom=bottom, eyes=eyes, mouth=mouth, rot=[0, -2, 0, 2][i], stem=[6, -6, 6, -6][i], lit=FLICK[i],
                   feet=((0, FLOOR - 6 - bottom), (0, FLOOR - 6 - bottom)), arms=[(-1, paws[0]), (1, paws[1])], fx=fx)


def level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    up = [150, 165, 175][i]
    kw = dict(eyes="happy", mouth="big", arms=[(-1, up), (1, up)], fx=stars)
    if _is_ghost(sp):
        return ghost(sp, bottom=112 - 2 * i, phase=i, **kw)
    return pumpkin(sp, bottom=FLOOR - 6 - [0, 1, 3][i], stem=[-8, 8, -4][i], lit=[1.0, 0.9, 1.0][i],
                   feet=((0, 0), (0, 0)) if i < 2 else ((-1, 1), (1, 1)), **kw)


def _fit(fn):
    """다 그린 프레임이 1px 투명 여백을 넘으면 그만큼 안쪽으로 민다."""
    def f(sp, i):
        im = fn(sp, i)
        x0, y0, x1, y1 = im.getchannel("A").getbbox()
        dx = (1 - x0) if x0 < 1 else min(0, (FRAME - 1) - x1)
        dy = (1 - y0) if y0 < 1 else min(0, (FRAME - 1) - y1)
        if not (dx or dy):
            return im
        out = Image.new("RGBA", im.size, (0, 0, 0, 0))
        out.paste(im, (dx, dy))
        return out
    f.__name__ = fn.__name__
    return f


POSES = {"walk": walk, "run": run, "roll": roll, "idle": idle, "look": look, "sleep": sleep, "hop": hop, "fall": fall,
         "dragged": dragged, "land": land, "sit_front": sit_front, "wave": wave, "tilt": tilt, "belly": belly,
         "stretch": stretch, "heart": heart, "tickle": tickle, "annoyed": annoyed, "hungry": hungry, "eat": eat,
         "level_up": level_up, "lookup": lookup}
POSES = {k: (v if k == "roll" else _fit(v)) for k, v in POSES.items()}  # roll은 회전 원 안 그대로

_COMMON = dict(front_ears=[], front_ear_pivots=(), front_ear_marks=[], front_head_marks=[], front_body_marks=[], front_tail=[],
               front_tail_root=(64, 112), side_ears=[], ears_over=False, side_head_marks=[], side_torso_marks=[], side_tail=[],
               tail_root=(28, 108), specials=["lookup"], poses=POSES)

PUMPKIN = Species(
    id="pumpkin", name="호박", walk_speed=70, scale=0.94, base=PUMP, back=PUMP, belly=PUMP, nose=PUMP_RIDGE, mouth="w",
    front_tail_color=PUMP, tail_color=PUMP, stage_size=dict.fromkeys(STAGES, 0.9), stripe=PUMP_RIDGE, outline=PUMP_LINE,
    # 모든 단계 동글 · 아기는 눈이 더 크고 색이 살짝 연함
    stage_tweak={"baby": dict(pale=0.16, eye=1.14, fit=0.9), "kinder": dict(pale=0.08, eye=1.08, fit=0.94),
                 "child": dict(eye=1.04, fit=0.96), "teenager": dict(eye=1.0), "adult": dict(eye=0.98, calm=False, fit=0.95)},
    **_COMMON,
)

GHOST_SP = Species(
    id="ghost", name="유령", walk_speed=40, scale=0.94, base=GHOST, back=GHOST, belly=GHOST, nose=BLUSH, mouth="w",
    front_tail_color=GHOST, tail_color=GHOST, stage_size=dict.fromkeys(STAGES, 0.85), stripe=GHOST_SHADE, outline=GHOST_LINE,
    # 모든 단계 반짝이 두 개 큰 눈 (어른도) — 아기는 더 크게
    stage_tweak={"baby": dict(pale=0.0, eye=1.32, fit=0.92), "kinder": dict(pale=0.0, eye=1.22, fit=0.95),
                 "child": dict(sparkle=True, smile=False, eye=1.16), "teenager": dict(sparkle=True, eye=1.12),
                 "adult": dict(sparkle=True, calm=False, eye=1.08, fit=0.95)},
    **_COMMON,
)


def register():
    dp.SPECIES[PUMPKIN.id] = PUMPKIN
    dp.SPECIES[GHOST_SP.id] = GHOST_SP


register()


def preview_species(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 화면 표시 크기 + sit_front·walk 2배 확대 (귀여움 확인용). 아래 띠: 어두운 바탕 (빛·반투명 확인)."""
    from PIL import ImageDraw, ImageFont
    from sprite_spec import STAGE_SCALE
    names = [("walk", 0), ("walk", 2), ("sit_front", 0), ("wave", 1), ("eat", 0), ("eat", 1), ("sleep", 0), ("hop", 1),
             ("tickle", 0), ("hungry", 0), ("belly", 0), ("roll", 0), ("dragged", 0)]
    cell, label, row_h = FRAME + 4, 70, 2 * FRAME + 8
    x_disp = label + len(names) * cell + 24
    x_zoom = x_disp + cell + 24
    W = x_zoom + 2 * (2 * FRAME + 8)
    H = 24 + 2 * len(STAGES) * row_h
    img = Image.new("RGBA", (W, H), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    for c, (anim, k) in enumerate(names):
        d.text((label + c * cell, 6), f"{anim} {k}", fill=(60, 60, 60), font=font)
    d.text((x_disp, 6), "(display)", fill=(60, 60, 60), font=font)
    d.text((x_zoom, 6), "sit_front x2 / walk x2", fill=(60, 60, 60), font=font)
    for band, bgc in enumerate([bg, (40, 44, 52)]):
        y0 = 24 + band * len(STAGES) * row_h
        if band:
            img.paste(Image.new("RGBA", (W, len(STAGES) * row_h), bgc + (255,)), (0, y0))
        for r, st in enumerate(STAGES):
            y = y0 + r * row_h
            base = y + row_h - FRAME - 8
            d.text((4, base + FRAME // 2), st, fill=(60, 60, 60) if not band else (220, 220, 220), font=font)
            for c, (anim, k) in enumerate(names):
                img.alpha_composite(by_stage[st][anim][k], (label + c * cell, base))
            s = STAGE_SCALE[st] / STAGE_SCALE["adult"]
            small = by_stage[st]["sit_front"][0].resize((round(FRAME * s), round(FRAME * s)), Image.LANCZOS)
            img.alpha_composite(small, (x_disp + (FRAME - small.width) // 2, base + FRAME - small.height))
            img.alpha_composite(by_stage[st]["sit_front"][0].resize((2 * FRAME, 2 * FRAME), Image.LANCZOS), (x_zoom, y))
            img.alpha_composite(by_stage[st]["walk"][0].resize((2 * FRAME, 2 * FRAME), Image.LANCZOS), (x_zoom + 2 * FRAME + 8, y))
    img.save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    for sp in (PUMPKIN, GHOST_SP):
        by_stage = dp.write_species(sp, root / "ui" / "pets" / sp.id)
        preview_species(by_stage, pv / f"stages_{sp.id}.png")
    print("ok: pumpkin, ghost")


if __name__ == "__main__":
    main()
