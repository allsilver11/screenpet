"""개구리 🐸 — 폴짝폴짝 뛰는 동물 (pet.json "locomotion": "hop"). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓰고
뼈대만 개구리 것: 머리와 몸이 한 덩어리, 머리 위로 볼록 솟은 두 눈 언덕, 넓은 웃는 입, 밝은 배, 접힌 뒷다리, 발끝 동그란 빨판.

    python tools/draw_frog.py  → ui/pets/frog/{sheet_<stage>.png ×5, sheet.png, pet.json} + tools/preview/{stages_frog,climb_frog}.png
    (python tools/draw_pets.py도 개구리를 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

단계: 아기 = 동글 올챙이 개구리 (엉덩이에 꼬리 꼬투리, 아주 짧은 다리, 점 없음) → 유아 꼬리 없어지고 점 하나 → 어린이 점 셋, 다리 자람, 머리에 새싹 잎
      → 청소년 기준 (작은 연잎 모자) → 어른 긴 뒷다리, 점 많이, 턱 아래 울음주머니 줄.
동작: 필수 21 + climb (뒷모습으로 벽을 폴짝폴짝: 웅크림 → 밀기 → 솟음 → 쩍 붙기) · slide · bigjump (깊이 웅크림 → 쭉 뻗어 날아오름 → 공중 → 착지 준비).
eat에는 밥그릇이 없다: 혀를 앞-아래(앱이 그릇/먹이 소품을 놓는 자리)로 날름 뻗는다.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (BLUSH, C, E, EYE, HEART, L, LINE, MOUTH_IN, SPEED, T, TONGUE, WHITE, Part, Species, _ST, _shown,  # noqa: E402
                       anger_fx, arc, blush, brows, heart_fx, mix, open_mouth, question_fx, sparkle_fx, stage_eye, strand, sweat_fx,
                       view, xf, z_fx)
from sprite_spec import FRAME, ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FROG_G, FROG_SPOT, FROG_BELLY, FROG_LINE = (152, 212, 116), (104, 176, 90), (244, 248, 206), (76, 116, 70)
FLY_BODY, FLY_WING = (92, 96, 112), (226, 240, 252)
# 단계(lvl)별 — leg: 다리 길이 · lw: 다리 굵기 · tail: 올챙이 꼬리 꼬투리 크기 · bump: 눈 언덕 배율 · pouch: 울음주머니 줄
FROG_STAGE = {
    0: dict(leg=0.55, lw=1.15, tail=1.0, bump=1.12, pouch=False),
    1: dict(leg=0.7, lw=1.08, tail=0.0, bump=1.08, pouch=False),
    2: dict(leg=0.86, lw=1.0, tail=0.0, bump=1.03, pouch=False),
    3: dict(leg=1.0, lw=1.0, tail=0.0, bump=1.0, pouch=False),
    4: dict(leg=1.12, lw=1.02, tail=0.0, bump=0.98, pouch=True),
}
EYE_K = 1.45  # 개구리 눈은 다른 동물보다 크게 (반짝이 두 개 큰 눈)


def _fs():
    return FROG_STAGE[_ST["lvl"]]


def _pad(sp):
    return mix(sp.belly, sp.base, 0.35)


def _eye(kind, cx, cy, flip=False):
    with xf(sx=EYE_K, sy=EYE_K, pivot=(cx, cy)):
        return stage_eye(kind, cx, cy, flip)


def frog_mouth(kind, cx, cy, w=11.0):
    """개구리 입 (cx, cy = 입 가운데). None: 넓은 웃는 곡선 (어린이는 작게 벌린 웃음)."""
    if kind is None and _ST["smile"]:
        return [([L(arc(cx, cy - 2, w, 3.4, 10, 170, 14), 1.5)], LINE)] + open_mouth(cx, cy + 0.6, 3.6, 3.8)
    if kind is None:
        return [([L(arc(cx, cy - 2, w, 4.2, 10, 170, 14), 1.6)], LINE)]
    if kind == "frown":
        return [([L(arc(cx, cy + 3, w * 0.55, 2.6, 200, 340, 10), 1.5)], LINE)]
    if kind == "pout":
        return [([L(arc(cx + 1.5, cy + 3, 3.0, 2.0, 200, 340, 8), 1.5)], LINE)]
    if kind == "chew":
        return [([L([(cx - 5, cy), (cx - 2.5, cy + 1.8), (cx, cy), (cx + 2.5, cy + 1.8), (cx + 5, cy)], 1.4)], LINE)]
    if kind == "wide":  # 혀를 내미는 넓은 입 (안쪽만, 혀는 따로)
        return [([T(*arc(cx, cy - 1, w * 0.62 + 1.3, 6.3, 0, 180, 16))], LINE), ([T(*arc(cx, cy, w * 0.62, 5, 0, 180, 16))], MOUTH_IN)]
    rx, ry = {"open": (5.4, 5.6), "big": (8.0, 8.0), "yawn": (6.6, 9.0)}[kind]
    return open_mouth(cx, cy - 1, rx, ry, tongue=kind != "yawn")


FLOOR = 125.0  # 바닥 (몸 좌표) — 발끝 빨판이 프레임 아래 1px 여백을 넘지 않게


def _limb(chain, w, pads, sp, outline=True, pad_r=3.2, floor=None):
    """다리 하나: 굵기가 줄어드는 마디 선(chain) + 끝 빨판들 (pads: [(x, y)]). floor: 이 y 아래로 내려가지 않게 (바닥 위 발)."""
    if floor is not None:
        chain = [(x, min(y, floor - w[min(k, len(w)) - 1] / 2)) if k else (x, y) for k, (x, y) in enumerate(chain)]
        pads = [(x, min(y, floor - pad_r)) for x, y in pads]
    fns = []
    for k, (a, b) in enumerate(zip(chain, chain[1:])):
        fns.append(L([a, b], w[min(k, len(w) - 1)]))
    pf = [C(x, y, pad_r) for x, y in pads]
    return Part(fns + pf, sp.base, [(pf, _pad(sp))] if pads else [], grow=0, outline=outline)


def _stage_chain(chain, root_to=None, ground=None):
    """다리 마디를 단계 다리 길이로: 뿌리 기준으로 줄이고, ground면 가장 낮은 점을 원래 높이에 맞춰 내린다."""
    k = _fs()["leg"]
    r = chain[0]
    nr = root_to or r
    out = [(nr[0] + (x - r[0]) * k, nr[1] + (y - r[1]) * k) for x, y in chain]
    if ground:
        dy = max(y for _, y in chain) - max(y for _, y in out)
        out = [(x, y + dy) for x, y in out]
    return out


def _pads_at(end, prev, n=3, spread=26, d=3.6):
    """마디 끝에서 부채꼴로 벌어진 발가락 빨판 위치."""
    ang = math.atan2(end[1] - prev[1], end[0] - prev[0])
    out = []
    for j in range(n):
        a = ang + math.radians((j - (n - 1) / 2) * spread)
        out.append((end[0] + math.cos(a) * d, end[1] + math.sin(a) * d))
    return out


def _tail_nub(sp, root, direction=(-1, -0.3)):
    """아기 올챙이 꼬리 꼬투리 (끝이 살짝 말린 짧은 꼬리)."""
    t = _fs()["tail"]
    if not t:
        return []
    dx, dy = direction
    pts = [root, (root[0] + dx * 11 * t, root[1] + dy * 11 * t), (root[0] + dx * 18 * t, root[1] + dy * 18 * t - 5 * t)]
    return [Part([strand(pts, 11 * t, 3.0)], sp.base, [([C(pts[-1][0], pts[-1][1], 3.5 * t)], mix(sp.base, sp.belly, 0.6))],
                 grow=0, outline=True)]


LILY, LILY_VEIN = (118, 186, 104), (92, 156, 84)


def _hat(cx, top, rot=0.0):
    """단계 장식 (작게): 어린이 = 새싹 잎 하나 · 청소년 = 작은 연잎 모자. 다른 단계는 없음."""
    lv = _ST["lvl"]
    if lv not in (2, 3):
        return []
    with xf(rot=rot, pivot=(cx, top + 4)):
        if lv == 2:
            stem = L([(cx, top + 3), (cx + 1, top - 4)], 2.2)
            leaf = [T(*([(cx + 1, top - 4)] + arc(cx + 7, top - 7, 7, 4, 200, 380, 10))),
                    T(*([(cx + 1, top - 3)] + arc(cx - 4, top - 6, 5, 3, -20, 160, 10)))]
            return [Part([stem] + leaf, LILY, [([L([(cx + 2, top - 4.5), (cx + 11, top - 8)], 1.0)], LILY_VEIN)], grow=0, outline=True)]
        pad = T(*arc(cx, top, 14, 5.5, 18, 342, 24), (cx, top))
        veins = [L([(cx, top), (cx + 12 * math.cos(math.radians(a)), top + 4.6 * math.sin(math.radians(a)))], 0.9) for a in (60, 120, 180, 240, 300)]
        return [Part([pad], LILY, [(veins, LILY_VEIN)], grow=0, outline=True)]


def _spots(sp, items):
    """[(x, y, r, lvl)] → 등 점무늬 marks."""
    return [([C(x, y, r)], sp.stripe) for x, y, r, lv in items if _shown(lv)]


# =====================================================================
# 옆모습 (오른쪽을 본다). 몸 좌표: 바닥 y 125
# =====================================================================
SIDE_SPOTS = [(44, 86, 5.5, 1), (60, 74, 4.4, 2), (28, 100, 4.2, 2), (74, 64, 3.4, 3), (52, 100, 3.8, 3), (34, 82, 3.2, 4), (66, 90, 3.0, 4)]
SIT_HIND = [(36, 108), (60, 102), (36, 121), (62, 123)]
SIT_ARM = [(96, 108), (100, 123)]
SIDE_PIVOT = (60, 124)


def frog_side(sp, *, whole=None, body=None, hind=SIT_HIND, arm=SIT_ARM, far_arm=True, ground=True, eyes="dot", mouth=None,
              brow=None, fx=(), ops=(), tongue=None, hind_w=(15, 11, 7), legs_front=True):
    """옆모습 개구리. whole: 전체 xf · body: 몸·머리만 xf (다리 뿌리는 같이 움직임) · hind/arm: 마디 점 (몸 좌표, 뿌리 먼저).
    ground: 발이 바닥에 닿는 자세 (단계 다리 길이를 바꿔도 발은 바닥에)."""
    b = _ST["body"]
    ax, ay = _ST["torso"]
    hs = _ST["head"]
    fs = _fs()
    bk = dict(body or {})
    lw = fs["lw"]
    with xf(**(whole or {})):
        stage_m = dp._xf_matrix(sx=b * ax, sy=b * ay, pivot=SIDE_PIVOT)
        body_m = dp._xf_matrix(**bk) if bk else (1, 0, 0, 0, 1, 0)

        def root(p):
            return dp._apply(body_m, dp._apply(stage_m, p))
        parts = []
        if far_arm and arm:
            ch = _stage_chain([(arm[0][0] - 9, arm[0][1] + 2)] + [(x - 7, y) for x, y in arm[1:]],
                              root((arm[0][0] - 9, arm[0][1] + 2)), ground)
            parts.append(_limb(ch, [8 * lw], _pads_at(ch[-1], ch[-2], 2, 40, 2.6), sp, pad_r=2.6, floor=FLOOR if ground else None))
        with xf(**bk):
            with xf(sx=b * ax, sy=b * ay, pivot=SIDE_PIVOT):
                with xf(sx=hs, sy=hs, pivot=(84, 108)):
                    bump = fs["bump"]
                    with xf(sx=bump, sy=bump, pivot=(80, 62)):
                        far = C(76, 54, 12)
                        near_c = (92, 56)
                    head = [E(48, 52, 118, 114), far]
                    marks = [([E(54, 92, 124, 144)], sp.belly)]
                    if fs["pouch"]:
                        marks.append(([L(arc(104, 100, 12, 5, 30, 150, 10), 1.2)], mix(LINE, sp.belly, 0.55)))
                body_p = Part([E(16, 74, 100, 125)] + head, sp.base, _spots(sp, SIDE_SPOTS) + marks, grow=2)
                parts.append(body_p)
                parts += _tail_nub(sp, (26, 100), (-1, -0.7))
                with xf(sx=hs, sy=hs, pivot=(84, 108)):
                    with xf(sx=fs["bump"], sy=fs["bump"], pivot=(80, 62)):
                        parts.append(Part([C(*near_c, 15)], sp.base, grow=0, outline=True))
                        face = _eye(eyes, 94, 56) + (brows(brow, 94, 54 + _ST["eye_dy"]) if brow else [])
                    parts += _hat(66, 50, -6)
                    face = blush(92, 78) + face
                    face += frog_mouth(mouth, 109, 88, 9)
                    if tongue:
                        face += tongue
        limbs = []
        if hind:
            ch = _stage_chain(hind, root(hind[0]), ground)
            w = [x * lw for x in hind_w]
            limbs.append(_limb(ch, w, _pads_at(ch[-1], ch[-2], 2, 36, 3.0), sp, pad_r=3.0, floor=FLOOR if ground else None))
        if arm:
            ch = _stage_chain(arm, root(arm[0]), ground)
            limbs.append(_limb(ch, [10 * lw], _pads_at(ch[-1], ch[-2], 2, 50, 3.0), sp, pad_r=2.9, floor=FLOOR if ground else None))
        parts = parts + limbs if legs_front else limbs + parts
    return dp.compose(parts, face + list(ops), fx)


def frog_walk(sp, i):
    """폴짝 걸음 (작게): 0 웅크림 · 1 앞으로 살짝 뜸 · 2 착지 · 3 숨 고르기."""
    k = [0, 1, 0.3, 0][i]
    sq = [(1.04, 0.95), (0.97, 1.04), (1.03, 0.97), (1.0, 1.0)][i]
    hind = [SIT_HIND, [(40, 104), (52, 106), (28, 120), (20, 126)], [(40, 106), (62, 102), (36, 121), (64, 123)], SIT_HIND][i]
    arm = [SIT_ARM, [(92, 98), (102, 106), (110, 112)], SIT_ARM, SIT_ARM][i]
    return frog_side(sp, whole=dict(dy=-4 * k, rot=-5 * k, pivot=(64, 110)), body=dict(sx=sq[0], sy=sq[1], pivot=SIDE_PIVOT),
                     hind=hind, arm=arm, ground=k == 0, eyes="happy" if i == 1 else "dot")


def frog_run(sp, i):
    """빠른 폴짝 이어 뛰기: 몸을 앞으로 눕히고 뒷다리 쭉 ↔ 모음, 속도선."""
    stretch = i % 2 == 0
    if stretch:
        hind = [(40, 104), (30, 110), (14, 118), (4, 122)]
        arm = [(92, 98), (104, 104), (114, 108)]
    else:
        hind = [(40, 106), (64, 100), (42, 118), (68, 120)]
        arm = [(92, 98), (98, 110), (104, 118)]
    fx = [([L([(4 + s, y), (18 + s, y)], 1.8)], SPEED, 0) for y, s in ((70, 0), (86, 3), (102, 0))] if stretch else []
    return frog_side(sp, whole=dict(dy=-5 if stretch else -1, rot=-12 if stretch else -2, pivot=(64, 110), dx=2), hind=hind, arm=arm,
                     ground=False, eyes="happy" if stretch else "dot", mouth="open" if stretch else None, fx=fx)


def frog_idle(sp, i):
    return frog_side(sp, body=dict(sy=1 + 0.02 * i, pivot=SIDE_PIVOT))


def frog_look(sp, i):
    rot = dp._swing([-10, 0, 10, 0][i])
    return frog_side(sp, body=dict(rot=rot, pivot=(70, 110)), eyes="closed" if i == 3 else "dot")


def frog_lookup(sp, i):
    """앉은 채 몸(머리)째 뒤로 젖혀 위를 본다."""
    return frog_side(sp, body=dict(rot=dp.lookup_tilt(i, 0.6), pivot=(70, 110)), eyes="up", fx=dp.lookup_fx(i, 114, 16))


def _lying(sp, i, eyes, mouth, fx):
    hind = [(40, 112), (66, 110), (40, 123), (72, 124)]
    return frog_side(sp, body=dict(sx=1.08, sy=0.72 + 0.012 * i, pivot=(60, 125)), hind=hind,
                     arm=[(94, 112), (106, 120), (116, 124)], eyes=eyes, mouth=mouth, fx=fx)


def frog_sleep(sp, i):
    zs = [z_fx(108, 62, 3.4), z_fx(119, 51, 2.6)] if i == 0 else [z_fx(110, 59, 3.6), z_fx(121, 47, 2.8)]
    return _lying(sp, i, "closed", None, zs)


def frog_annoyed(sp, i):
    return _lying(sp, i, "squint", "pout", [anger_fx(110, 60 - i, 7.0)])


def frog_hop(sp, i):
    """한 번 폴짝 (낮게): 0 웅크림 → 1 쭉 뻗고 공중 (발은 뒤로 대롱) → 2 다리 모으며 착지 ^^ (한 번, 코어가 폴짝마다 다시 튼다)."""
    if i == 0:
        return frog_side(sp, body=dict(sx=1.06, sy=0.9, pivot=SIDE_PIVOT), hind=[(40, 108), (68, 104), (38, 122), (66, 124)],
                         eyes="tight")
    if i == 1:
        return frog_side(sp, whole=dict(rot=-12, pivot=(64, 110), dy=-10), body=dict(sx=0.96, sy=1.06, pivot=SIDE_PIVOT),
                         hind=[(40, 104), (28, 110), (14, 114), (3, 113)], arm=[(92, 98), (104, 102), (114, 104)], ground=False,
                         eyes="happy", mouth="open")
    return frog_side(sp, whole=dict(rot=4, pivot=(70, 110), dy=-2), body=dict(sx=1.03, sy=0.96, pivot=SIDE_PIVOT),
                     hind=[(40, 106), (60, 106), (38, 120), (64, 122)], arm=[(92, 98), (100, 110), (106, 120)], ground=False,
                     eyes="happy")


def frog_bigjump(sp, i):
    """크게 뛰기 (창 위로): 0 깊이 웅크림 (힘 모음 💦) → 1 쭉 뻗어 솟구침 (아래 속도선) → 2 공중에서 활짝 → 3 착지 준비 (한 번)."""
    if i == 0:
        return frog_side(sp, body=dict(sx=1.12, sy=0.8, pivot=SIDE_PIVOT), hind=[(40, 110), (70, 108), (36, 123), (66, 124)],
                         arm=[(92, 104), (100, 114), (102, 123)], eyes="tight", mouth="pout", fx=sweat_fx(110, 46))
    if i == 1:
        lines = [([L([(x, 104), (x - 8, 120)], 1.8)], SPEED, 0) for x in (34, 46, 58)]
        return frog_side(sp, whole=dict(rot=-34, pivot=(66, 90), dy=-8, dx=6), body=dict(sx=0.9, sy=1.12, pivot=SIDE_PIVOT),
                         hind=[(40, 102), (30, 112), (20, 122), (10, 130)], arm=[(92, 96), (104, 92), (116, 88)], ground=False,
                         eyes="happy", mouth="big", fx=lines)
    if i == 2:
        stars = [f for x, y, r in ((22, 30, 4.0), (110, 22, 3.4)) for f in sparkle_fx(x, y, r)]
        return frog_side(sp, whole=dict(rot=-6, pivot=(64, 90), dy=-10), hind=[(40, 104), (20, 104), (6, 112), (4, 102)],
                         arm=[(92, 98), (108, 96), (118, 90)], ground=False, eyes="happy", mouth="big", fx=stars)
    return frog_side(sp, whole=dict(rot=10, pivot=(64, 100), dy=-6), body=dict(sx=1.02, sy=0.98, pivot=SIDE_PIVOT),
                     hind=[(40, 106), (54, 112), (40, 122), (60, 124)], arm=[(92, 100), (104, 110), (112, 118)], ground=False,
                     eyes="round", mouth="open")


def frog_fall(sp, i):
    w = 6 if i else -6
    return frog_side(sp, whole=dict(rot=-8, pivot=(64, 90), dy=-4), hind=[(40, 104), (24, 100 + w), (10, 106 - w), (4, 98 - w)],
                     arm=[(92, 96), (106, 86 + w), (116, 80 + w)], far_arm=False, ground=False, eyes="round", mouth="open")


def frog_dragged(sp, i):
    """뒷덜미 잡혀 대롱대롱: 몸이 위에서 늘어지고 긴 뒷다리가 축 처짐."""
    s = [-1, 1][i]
    return frog_side(sp, whole=dict(rot=4 * s, sx=0.84, sy=0.84, pivot=(70, 12), dx=4 + 2 * s, dy=-14),
                     body=dict(sy=1.08, pivot=SIDE_PIVOT),
                     hind=[(44, 110), (42, 122), (40 + 3 * s, 136), (38 + 5 * s, 146)], hind_w=(12, 9, 6),
                     arm=[(90, 102), (92, 114), (93 - 2 * s, 124)], ground=False, eyes="closed", mouth="frown")


def frog_land(sp, i):
    if i == 0:
        return frog_side(sp, body=dict(sx=1.08, sy=0.84, pivot=SIDE_PIVOT), hind=[(40, 110), (66, 106), (38, 122), (68, 124)],
                         eyes="tight")
    return frog_side(sp, body=dict(sx=1.02, sy=0.97, pivot=SIDE_PIVOT))


# =====================================================================
# 앞모습. 몸 좌표: 바닥 y 125, 가운데 x 64
# =====================================================================
FRONT_PIVOT = (64, 124)
FRONT_SPOTS_HEAD = [(64, 40, 4.2, 2), (52, 34, 3.0, 4), (76, 34, 3.0, 4)]
FRONT_SPOTS_BODY = [(28, 96, 4.4, 1), (100, 98, 4.0, 1), (32, 110, 3.2, 3), (96, 112, 3.2, 3)]
F_HIND = [(40, 112), (20, 104), (22, 121), (10, 124)]
F_ARM = [(48, 108), (47, 121)]


def _mx(pts):
    return [(128 - x, y) for x, y in pts]


def frog_front(sp, *, whole=None, head=None, body=None, arms=(F_ARM, F_ARM), hind=(F_HIND, F_HIND), eyes="dot", mouth=None,
               brow=None, fx=(), ops=(), held=(), props=(), tongue=(), cheeks=False, ground=True, face=(0, 0)):
    """앞모습 개구리. arms/hind: (왼쪽, 오른쪽) 마디 점 — 오른쪽은 화면 좌우 반전해서 적는다 (같은 점을 넣으면 대칭).
    None이면 그 다리를 그리지 않는다. held: 몸과 팔 사이 Part 함수 · props: 맨 앞 소품 Part 함수 · tongue: 얼굴 위 ops."""
    b = _ST["body"]
    fx_, fy_ = _ST["fbody"]
    hs = _ST["head"]
    fs = _fs()
    lw = fs["lw"]
    st = dict(sx=b * fx_, sy=b * fy_, pivot=FRONT_PIVOT)
    stage_m = dp._xf_matrix(**st)
    with xf(**(whole or {})):
        body_m = dp._xf_matrix(**(body or {})) if body else (1, 0, 0, 0, 1, 0)

        def root(p):
            return dp._apply(body_m, dp._apply(stage_m, p))
        parts = []
        for side, ch in enumerate(hind):
            if ch is None:
                continue
            ch = _mx(ch) if side else ch
            c2 = _stage_chain(ch, root(ch[0]), ground)
            parts.append(_limb(c2, [18 * lw, 12 * lw, 8 * lw], _pads_at(c2[-1], c2[-2], 3, 34, 3.4), sp, pad_r=3.0, floor=FLOOR if ground else None))
        with xf(**(body or {})):
            with xf(**st):
                parts.append(Part([E(22, 64, 106, 125)], sp.base, _spots(sp, FRONT_SPOTS_BODY) + [([E(38, 76, 90, 132)], sp.belly)], grow=4))
            hy = (1 - b * fy_) * (FRONT_PIVOT[1] - 96)
            with xf(dy=hy):
                with xf(**(head or {})):
                    with xf(sx=hs, sy=hs, pivot=(64, 96)):
                        bump = fs["bump"]
                        with xf(sx=bump, sy=bump, pivot=(64, 60)):
                            bumps = [C(38, 36, 16), C(90, 36, 16)]
                        puff = []
                        if cheeks:
                            puff = [E(12, 56, 40, 86), E(88, 56, 116, 86)]
                        marks = [([E(34, 72, 94, 112)], sp.belly)] + _spots(sp, FRONT_SPOTS_HEAD)
                        if fs["pouch"]:
                            marks.append(([L(arc(64, 88, 14, 4, 20, 160, 10), 1.2)], mix(LINE, sp.belly, 0.55)))
                        parts.append(Part([E(14, 32, 114, 98)] + bumps + puff, sp.base, marks, grow=1))
                        parts += _hat(64, 33)
                        with xf(dx=face[0], dy=face[1]):
                            with xf(sx=bump, sy=bump, pivot=(64, 60)):
                                f_ops = _eye(eyes, 38, 36) + _eye(eyes, 90, 36, flip=True)
                                if brow:
                                    f_ops += brows(brow, 38, 34 + _ST["eye_dy"]) + brows(brow, 90, 34 + _ST["eye_dy"], flip=True)
                            bd = 2 if _ST["sparkle"] else 0
                            f_ops = blush(30 - bd, 66 + bd) + blush(98 + bd, 66 + bd) + f_ops
                            f_ops += frog_mouth(mouth, 64, 72, 13)
                            f_ops += list(tongue)
        parts += [h() for h in held]
        for side, ch in enumerate(arms):
            if ch is None:
                continue
            ch = _mx(ch) if side else ch
            c2 = _stage_chain(ch, root(ch[0]), ground)
            parts.append(_limb(c2, [12.5 * lw], _pads_at(c2[-1], c2[-2], 3, 40, 3.8), sp, pad_r=3.1, floor=FLOOR if ground else None))
        parts += [p() for p in props]
    return dp.compose(parts, f_ops + list(ops), fx)


RAISED = [(46, 84), (34, 70), (28, 52)]  # 앞발 번쩍 (왼쪽 기준)


def frog_sit_front(sp, i):
    return frog_front(sp, eyes="closed" if i == 3 else "dot", head=dict(dy=[0, 0.6, 0, 0][i]))


def frog_wave(sp, i):
    up = [(46, 84), (30, 70), (24, 54)] if i % 2 == 0 else [(46, 84), (26, 74), (14, 62)]
    return frog_front(sp, arms=(F_ARM, up), head=dict(rot=[-3, -1, -3, -1][i], pivot=(64, 96)), ground=True)


def frog_tilt(sp, i):
    deg = dp._swing([8, 12, 12][i])
    return frog_front(sp, head=dict(rot=deg, pivot=(64, 96), dy=2), fx=[question_fx(116, 20 + [2, 0, -2][i])])


def frog_belly(sp, i):
    """발라당: 등을 대고 누워 밝은 배를 보이고 네 발 꼬물, 얼굴은 정면 ^o^."""
    wig = [0, 2, 0, -2][i]
    fs = _fs()
    lw = fs["lw"]
    k = fs["leg"]
    body = Part([E(34, 86, 120, 125)], sp.base, [([E(42, 80, 114, 116)], sp.belly)] + _spots(sp, [(40, 116, 3.6, 1), (114, 116, 3.4, 2)]), grow=4)
    limbs = []
    # 앞발 둘은 짧게 위로 꼬물, 뒷다리 둘은 무릎을 굽혀 발바닥 빨판이 하늘로
    chains = [[(62, 96), (58 - wig, 74 + wig)], [(76, 94), (78 + wig, 72 - wig)],
              [(94, 100), (92, 78 - wig), (102, 70 - wig)], [(110, 102), (116, 80 + wig), (124, 76 + wig)]]
    for j, ch in enumerate(chains):
        ch = [ch[0]] + [(ch[0][0] + (x - ch[0][0]) * (0.65 + 0.35 * k), ch[0][1] + (y - ch[0][1]) * (0.65 + 0.35 * k)) for x, y in ch[1:]]
        w = [10 * lw] if j < 2 else [14 * lw, 9 * lw]
        limbs.append(_limb(ch, w, _pads_at(ch[-1], ch[-2], 3, 38, 3.4), sp, pad_r=3.0))
    with xf(dx=-18, dy=40, rot=-8, pivot=(64, 70), sx=0.72, sy=0.72):
        hs = _ST["head"]
        with xf(sx=hs, sy=hs, pivot=(64, 96)):
            head = Part([E(14, 32, 114, 98), C(38, 36, 16), C(90, 36, 16)], sp.base, [([E(34, 72, 94, 112)], sp.belly)], grow=1, outline=True)
            face = blush(30, 66) + blush(98, 66) + _eye("happy", 38, 36) + _eye("happy", 90, 36, flip=True) + frog_mouth("open", 64, 72)
    return dp.compose([body] + limbs + [head], face)


def frog_stretch(sp, i):
    if i == 0:  # 엎드려 앞발 쭉 내밀고 엉덩이 들기
        return frog_front(sp, body=dict(sy=0.82, pivot=FRONT_PIVOT), head=dict(dy=14, pivot=(64, 96)), eyes="tight",
                          arms=([(46, 104), (30, 116), (14, 122)], [(46, 104), (30, 116), (14, 122)]))
    if i == 1:
        return frog_front(sp, eyes="tight", mouth="yawn", head=dict(dy=-1, rot=-4, pivot=(64, 96)))
    return frog_front(sp)


def frog_heart(sp, i):
    held = lambda: Part([C(57.5, 98, 7.2), C(70.5, 98, 7.2), T((50.8, 100), (77.2, 100), (64, 113))], HEART,
                        [([E(53.5, 93.5, 57.5, 97)], WHITE)], outline=True)
    hs = [heart_fx(116, 44 - 6 * i, 4.6 + 0.6 * i), heart_fx(12, 50 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(118, 20 - 3 * i, 2.6 + 0.6 * i))
    hug = [(48, 90), (50, 98), (56, 104)]
    return frog_front(sp, eyes="happy", held=[held], arms=(hug, hug), fx=hs)


def frog_tickle(sp, i):
    dx, rot = [(-2, -4), (2, 4), (0, 0)][i]
    a = [[(46, 86), (30, 80), (22, 70)], [(46, 88), (36, 96), (30, 104)], [(46, 86), (32, 88), (26, 80)]]
    return frog_front(sp, whole=dict(dx=dp._swing(dx), dy=-1, rot=dp._swing(rot), pivot=(64, 124)), eyes="happy", mouth="open",
                      arms=(a[i], a[(i + 1) % 3]))


FLY_SPOT = (66, 86)  # eat: 혀 끝이 닿는 앞-아래 — 앱 소품 창의 그릇 밥 더미 위 (x 36–92, y ≈ 82)


def fly_fx(cx, cy, k=1.0):
    """귀여운 파리 한 마리 (생각 풍선용): 동글 몸 + 반투명 느낌 흰 날개 둘 + 반짝 눈."""
    wing = lambda s: E(cx + s * 1 - 5 * k + s * 4 * k, cy - 9 * k, cx + s * 1 + 5 * k + s * 4 * k, cy - 1 * k)
    return [([wing(-1), wing(1)], (146, 178, 212), 0.9), ([E(cx - 5 * k - 3.6 * k, cy - 8 * k, cx - 3.6 * k + 3.4 * k, cy - 2 * k),
                                                       E(cx + 3.6 * k - 3.4 * k, cy - 8 * k, cx + 3.6 * k + 5 * k, cy - 2 * k)], FLY_WING, 0),
            ([C(cx, cy + 1 * k, 5.4 * k)], LINE, 0), ([C(cx, cy + 1 * k, 4.2 * k)], FLY_BODY, 0),
            ([C(cx - 1.8 * k, cy - 0.2 * k, 1.3 * k), C(cx + 1.8 * k, cy - 0.2 * k, 1.3 * k)], WHITE, 0)]


def fly_bubble_fx(cx, cy, r=12.5):
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], LINE, 1.0), ([C(x, y, rr)], WHITE, 0)]
    return fx + fly_fx(cx, cy + 1, 1.05)


def frog_hungry(sp, i):
    """배고파 시무룩: 처진 눈썹 ︶ 눈, 작은 입, 한 앞발로 배 문지름, 파리 생각 풍선."""
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    rub = [(46, 90), (54 + dx, 100 + dy), (60 + dx, 106 + dy)]
    return frog_front(sp, eyes="closed", mouth="frown", brow="sad", head=dict(dy=2, rot=-4, pivot=(64, 96)),
                      arms=(F_ARM, _mx(rub)), fx=fly_bubble_fx(22, 19))


def _tongue_ops(start, end, w=5.0):
    """분홍 혀: 입에서 end까지 휘어진 띠 + 끝의 동그란 혀끝 (짙은 테두리)."""
    (x0, y0), (x1, y1) = start, end
    mx, my = (x0 + x1) / 2 + 6, (y0 + y1) / 2 - 2
    pts = [((1 - t) ** 2 * x0 + 2 * (1 - t) * t * mx + t * t * x1, (1 - t) ** 2 * y0 + 2 * (1 - t) * t * my + t * t * y1)
           for t in (k / 12 for k in range(13))]
    tip = C(x1, y1, w * 0.95)
    return [([L(pts, w + 2.4), C(x1, y1, w * 0.95 + 1.3)], LINE), ([L(pts, w), tip], TONGUE),
            ([L(pts[2:-2], 1.0)], mix(TONGUE, MOUTH_IN, 0.5)), ([C(x1 - 1.4, y1 - 1.6, 1.3)], WHITE)]


def frog_eat(sp, i):
    """냠! 혀 날름 (그릇 없음 — 앱이 앞-아래에 먹이 소품을 놓는다): 0 먹이를 보고 입 벌림 → 1 혀를 앞-아래로 쭉 →
    2 혀를 감아 들이며 볼 빵빵 → 3 오물오물 ^^."""
    hy = (1 - _ST["body"] * _ST["fbody"][1]) * (FRONT_PIVOT[1] - 96)
    hs = _ST["head"]
    mouth = (64, 96 + (72 - 96) * hs + hy + 4)
    if i == 0:
        return frog_front(sp, eyes="dot", mouth="open", head=dict(dy=3, pivot=(64, 96)))
    if i == 1:
        return frog_front(sp, eyes="tight", mouth="wide", head=dict(dy=4, pivot=(64, 96)),
                          ops=_tongue_ops((mouth[0], mouth[1] + 4), FLY_SPOT))
    if i == 2:
        return frog_front(sp, eyes="happy", mouth="chew", cheeks=True, head=dict(dy=2, pivot=(64, 96)))
    return frog_front(sp, eyes="closed", mouth="chew", cheeks="right", head=dict(dy=1, rot=-3, pivot=(64, 96)),
                      fx=[([C(108, 50, 2.0)], BLUSH, 0.8), ([C(114, 42, 1.4)], BLUSH, 0.8)])


def frog_level_up(sp, i):
    lift = [0, 1, 2][i]
    stars = [[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
             [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
             [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i]
    fx = [f for x, y, r in stars for f in sparkle_fx(x, y, r)]
    up = [(46, 84), (32, 68 - 2 * lift), (26, 50 - 4 * lift)]
    return frog_front(sp, whole=dict(dy=-1 if i == 2 else 0), eyes="happy", mouth="big", arms=(up, up), fx=fx)


def frog_roll(sp, i):
    """동글 말린 개구리 공 (발은 배 밑으로 쏙): 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        with xf(rot=[-6, 6][i], pivot=(cx, cy)):
            ball = Part([E(cx - 32, cy - 22, cx + 32, cy + 30), C(cx - 15, cy - 20, 11), C(cx + 15, cy - 20, 11)], sp.base,
                        [([E(cx - 22, cy + 4, cx + 22, cy + 36)], sp.belly)] + _spots(sp, [(cx - 26, cy + 4, 3.4, 1), (cx + 26, cy + 6, 3.2, 2)]),
                        grow=0)
            feet = [_limb([(cx + s * 12, cy + 22), (cx + s * 18, cy + 26)], [8], _pads_at((cx + s * 18, cy + 26), (cx + s * 12, cy + 22), 3, 40, 3.0),
                          sp, pad_r=2.6) for s in (-1, 1)]
            k = 0.62
            face = blush(cx - 20, cy + 2) + blush(cx + 20, cy + 2)
            with xf(sx=k, sy=k, pivot=(cx, cy - 20)):
                face += _eye("happy", cx - 24, cy - 20) + _eye("happy", cx + 24, cy - 20, flip=True)
            face += frog_mouth("open" if i else None, cx, cy + 4, 9)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([ball] + feet, face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 벽에 붙어 폴짝폴짝 기어오르기. 얼굴은 안 보인다. 앞발은 몸·머리 뒤 (빨판만 빼꼼),
# 뒷다리는 양옆으로 쩍 벌리고 발바닥 빨판이 보인다. 좌우 대칭 (앱은 반전하지 않음)
# =====================================================================
BACK_SPOTS = [(64, 70, 5.0, 1), (48, 90, 4.2, 2), (80, 92, 4.2, 2), (64, 104, 3.6, 3), (44, 64, 3.2, 3), (84, 64, 3.2, 3),
              (56, 50, 2.8, 4), (72, 50, 2.8, 4)]


def frog_back(sp, *, hands, knees, feet, sy=1.0, dy=0.0, scratch=None, sweat=None, fx=()):
    """hands: 왼쪽 앞발 끝 (x, y) · knees/feet: 왼쪽 무릎·발 끝 — 오른쪽은 좌우 대칭."""
    b = _ST["body"]
    fx_, fy_ = _ST["fbody"]
    hs = _ST["head"]
    fs = _fs()
    lw = fs["lw"]
    k = fs["leg"]
    ops = []
    with xf(dy=dy):
        with xf(sy=sy, pivot=FRONT_PIVOT):
            st = dict(sx=b * fx_, sy=b * fy_, pivot=FRONT_PIVOT)
            hy = (1 - b * fy_) * (FRONT_PIVOT[1] - 96)
            arms = []
            for s in (-1, 1):
                sh = (64 + 22 * s, 78 + hy)
                hx, hy_ = hands
                tip = (64 + (hx - 64) * -s, hy_ + hy)
                tip = (sh[0] + (tip[0] - sh[0]) * (0.6 + 0.25 * k) * hs, sh[1] + (tip[1] - sh[1]) * (0.7 + 0.2 * k) * hs ** 0.5)
                arms.append(_limb([sh, tip], [11 * lw], _pads_at(tip, sh, 3, 34, 4.0), sp, pad_r=3.4))
                if scratch is not None:
                    for j, ddx in enumerate((-4.5, 0, 4.5)):
                        y0 = tip[1] - 10 - (j % 2) * 1.5 - scratch * 2
                        ops.append(([L([(tip[0] + ddx * 1.1, y0), (tip[0] + ddx * 1.3, y0 - 8)], 1.6)], LINE))
            with xf(**st):
                body = Part([E(30, 58, 98, 124)], sp.base, _spots(sp, BACK_SPOTS), grow=4)
                parts_tail = _tail_nub(sp, (64, 116), (0, 1)) if fs["tail"] else []
            with xf(dy=hy):
                with xf(sx=hs * BACK_HEAD, sy=hs * BACK_HEAD, pivot=(64, 92)):
                    bump = fs["bump"]
                    with xf(sx=bump, sy=bump, pivot=(64, 60)):
                        bumps = [C(40, 34, 15), C(88, 34, 15)]
                        shine = [([E(33, 25, 41, 31), E(81, 25, 89, 31)], mix(sp.base, WHITE, 0.45))]
                    head = Part([E(16, 30, 112, 96)] + bumps, sp.base, _spots(sp, [(64, 44, 3.6, 2)]) + shine, grow=1, outline=True)
                    hat = _hat(64, 31)
            legs = []
            for s in (-1, 1):
                hip = dp._apply(dp._xf_matrix(**st), (64 + 16 * s, 112))
                kn = (64 + (knees[0] - 64) * -s, knees[1])
                ft = (64 + (feet[0] - 64) * -s, feet[1])
                ch = [hip, (hip[0] + (kn[0] - hip[0]) * k, hip[1] + (kn[1] - hip[1]) * k),
                      (hip[0] + (ft[0] - hip[0]) * k, hip[1] + (ft[1] - hip[1]) * k)]
                pads = _pads_at(ch[-1], ch[-2], 3, 36, 4.0)
                p = _limb(ch, [14 * lw, 10 * lw], pads, sp, pad_r=3.4, floor=FLOOR)
                legs.append(p)
            if sweat is not None:
                with xf(dy=hy):
                    ops += [(f[0], f[1]) for f in sweat_fx(94, 44 + sweat)]
    parts = arms + [body, head] + hat + parts_tail + legs
    return dp.compose(parts, ops, fx)


# 폴짝 기어오르기 한 주기: 0 웅크려 붙음 → 1 다리로 밀기 → 2 위로 솟음 (손 쭉) → 3 쩍 벌려 붙기
BACK_HEAD = 0.8  # 뒷모습 머리는 조금 작게 (등 무늬와 빼꼼 나온 앞발이 보이게)
CLIMB_KEYS = [
    dict(hands=(16, 50), knees=(16, 94), feet=(30, 108), sy=0.94, dy=2),
    dict(hands=(18, 30), knees=(26, 108), feet=(34, 124), sy=1.04, dy=-1),
    dict(hands=(28, 8), knees=(40, 116), feet=(46, 126), sy=1.08, dy=-3),
    dict(hands=(8, 36), knees=(8, 102), feet=(18, 122), sy=0.97, dy=0),
]


def frog_climb(sp, i):
    return frog_back(sp, **CLIMB_KEYS[i])


def frog_slide(sp, i):
    return frog_back(sp, hands=(22, 10), knees=(14, 106), feet=(24, 124), sy=0.94, dy=[0, 1][i], scratch=i, sweat=i)


def _fit(fn):
    """다 그린 프레임이 1px 투명 여백을 넘으면 (빨판·뻗은 다리 끝이 1~4px) 그만큼 안쪽으로 민다."""
    from PIL import Image

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
    f.__doc__ = fn.__doc__
    return f


FROG_POSES = {"walk": frog_walk, "run": frog_run, "roll": frog_roll, "idle": frog_idle, "look": frog_look, "sleep": frog_sleep,
              "hop": frog_hop, "fall": frog_fall, "dragged": frog_dragged, "land": frog_land, "sit_front": frog_sit_front,
              "wave": frog_wave, "tilt": frog_tilt, "belly": frog_belly, "stretch": frog_stretch, "heart": frog_heart,
              "tickle": frog_tickle, "annoyed": frog_annoyed, "hungry": frog_hungry, "eat": frog_eat, "level_up": frog_level_up,
              "climb": frog_climb, "slide": frog_slide, "bigjump": frog_bigjump, "lookup": frog_lookup}
FROG_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in FROG_POSES.items()}  # roll은 회전 원 안 그대로

FROG = Species(
    id="frog", name="개구리", walk_speed=90, scale=0.94, base=FROG_G, back=FROG_G, belly=FROG_BELLY, nose=FROG_SPOT, mouth="w",
    front_ears=[], front_ear_pivots=(), front_ear_marks=[], front_head_marks=[], front_body_marks=[],
    front_tail=[], front_tail_root=(64, 112), front_tail_color=FROG_G, side_ears=[], ears_over=False,
    side_head_marks=[], side_torso_marks=[], side_tail=[], tail_root=(28, 108), tail_color=FROG_G,
    specials=["climb", "slide", "bigjump", "lookup"], stage_size=dict.fromkeys(STAGES, 0.85), stripe=FROG_SPOT, poses=FROG_POSES,
    outline=FROG_LINE,
    # 동글 찹쌀떡 몸 (가로가 더 넓다) · 모든 단계 반짝이 두 개 큰 눈 (어른도 — 귀여움 우선, 크기만 작게) · 아기는 눈이 더 크다
    stage_tweak={"baby": dict(torso=(1.14, 0.94), fbody=(1.14, 0.94), eye=1.6),
                 "kinder": dict(torso=(1.1, 0.96), fbody=(1.1, 0.96), eye=1.42),
                 "child": dict(torso=(1.06, 0.98), fbody=(1.06, 0.98), sparkle=True, smile=False, eye=1.28),
                 "teenager": dict(torso=(1.04, 1.0), fbody=(1.04, 1.0), sparkle=True, eye=1.16),
                 "adult": dict(torso=(1.06, 0.98), fbody=(1.04, 0.98), fit=0.9, sparkle=True, calm=False, eye=1.04)},
)


def register():
    dp.SPECIES[FROG.id] = FROG


register()


def main():
    root = Path(__file__).resolve().parent.parent
    by_stage = dp.write_species(FROG, root / "ui" / "pets" / "frog")
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    preview_frog(by_stage, pv / "stages_frog.png")
    dp.preview_back(by_stage, pv / "climb_frog.png")
    print("ok: frog")


def preview_frog(by_stage, path, bg=(255, 255, 255)):
    """5단계 × walk·sit_front·wave·eat(혀)·sleep·hop·bigjump·climb + 화면 표시 크기 + sit_front 2배 확대 (귀여움 확인용)."""
    from PIL import Image, ImageDraw, ImageFont
    from sprite_spec import STAGE_SCALE
    names = [("walk", 0), ("sit_front", 0), ("wave", 0), ("eat", 1), ("sleep", 0), ("hop", 1), ("bigjump", 1), ("climb", 0)]
    cell, label, row_h = FRAME + 4, 70, 2 * FRAME + 8
    x_disp = label + len(names) * cell + 24
    x_zoom = x_disp + cell + 24
    W, H = x_zoom + 2 * FRAME + 8, 24 + len(STAGES) * row_h
    img = Image.new("RGBA", (W, H), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    for c, (anim, k) in enumerate(names):
        d.text((label + c * cell, 6), f"{anim} {k}", fill=(60, 60, 60), font=font)
    d.text((x_disp, 6), "(display)", fill=(60, 60, 60), font=font)
    d.text((x_zoom, 6), "sit_front x2", fill=(60, 60, 60), font=font)
    for r, st in enumerate(STAGES):
        y = 24 + r * row_h
        base = y + row_h - FRAME - 8
        d.text((4, base + FRAME // 2), st, fill=(60, 60, 60), font=font)
        for c, (anim, k) in enumerate(names):
            img.alpha_composite(by_stage[st][anim][k], (label + c * cell, base))
        s = STAGE_SCALE[st] / STAGE_SCALE["adult"]
        small = by_stage[st]["sit_front"][0].resize((round(FRAME * s), round(FRAME * s)), Image.LANCZOS)
        img.alpha_composite(small, (x_disp + (FRAME - small.width) // 2, base + FRAME - small.height))
        img.alpha_composite(by_stage[st]["sit_front"][0].resize((2 * FRAME, 2 * FRAME), Image.LANCZOS), (x_zoom, y))
    img.save(path)


if __name__ == "__main__":
    main()
