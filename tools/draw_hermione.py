"""헤르미온느 📚 — 라면처럼 꼬불꼬불한 갈색 곱슬 긴 머리, 검은 로브 + 그리핀도르 목도리, 짙은 나무 지팡이의 꼬마 마법사 펫(hermione).
몸·공통 동작은 draw_wizard를 쓰고, 여기서는 생김새(Look)·헤르미온느만의 동작·등록만.

    python tools/draw_hermione.py → ui/pets/hermione/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_hermione,climb_hermione,wizard_hermione}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

특기 (코어): 🧹 broom 빗자루 날기 · ✨ cast 주문 (spells: leviosa·reparo) · 📖 read 바닥에 앉아 책 읽기 (책장 넘김) ·
      🙋 raise_hand 앞모습 손 번쩍 "I know!" · 👜 bag_walk·bag_drag·bag_stuff 끝없는 구슬 가방으로 치우기
      (가방은 앱이 손 자리 grip에 그림 — 작으면 손에 매달리고 커지면 몸 뒤 바닥에 놓여 끈으로 끌림. 시트엔 가방이 없음).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_wizard as wz  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, sparkle_fx, sweat_fx  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

HAIR = (128, 82, 46)  # 갈색 머리
HAIR_DARK = (98, 60, 32)  # 머리 물결 결
HAIR_HI = (172, 122, 78)  # 머리 윤기
APPLE, APPLE_HI, LEAF = (226, 58, 58), (255, 150, 140), (98, 170, 72)  # 🍎 사과 생각 풍선
COVER, COVER_DARK = (150, 44, 52), (110, 30, 38)  # 책 표지
PAGE, PAGE_LINE = (255, 250, 238), (188, 178, 160)  # 책장·글줄


def puffs(pts, r):
    """부스스한 머리 덩어리: 점마다 동그라미 (녹이면 몽글몽글 물결치는 실루엣)."""
    return [C(x, y, rr * r) for x, y, rr in pts]


def squiggle(a, b, n, amp, w=1.1, ph=0.0):
    """a → b 사이를 꼬불꼬불 (사인 물결 n번) 지나는 라면 머리 가닥."""
    (x0, y0), (x1, y1) = a, b
    ln = math.hypot(x1 - x0, y1 - y0) or 1.0
    nx, ny = -(y1 - y0) / ln, (x1 - x0) / ln
    k = max(8, int(n * 6))
    pts = []
    for j in range(k + 1):
        t = j / k
        o = amp * math.sin(t * n * 2 * math.pi + ph)
        pts.append((x0 + (x1 - x0) * t + nx * o, y0 + (y1 - y0) * t + ny * o))
    return L(pts, w)


def curl(x, y, r, a0=30, w=1.0):
    """동그랗게 말린 곱슬 한 올 (C자 고리)."""
    return L(dp.arc(x, y, r, r, a0, a0 + 270, 9), w)


def hermione_hair(view_, hx, hy, hrx, hry, q):
    """라면처럼 꼬불꼬불한 갈색 곱슬 긴 머리: 머리 둘레를 작은 동그라미로 촘촘히 부풀려 (몽글몽글 곱슬 실루엣) 어깨 아래까지,
    안에는 꼬불꼬불 가닥·말린 고리 (뒷모습은 등을 덮되 벽 짚은 두 손은 보임), 얼굴은 가운데 가르마 앞머리가 감쌈 (눈은 가리지 않음).
    → (머리 도형에 더할 도형, 삐죽 조각, 머리 안 무늬)."""
    R = hrx
    big = [T((hx - hrx * 3, hy - hry * 3), (hx + hrx * 3, hy - hry * 3), (hx + hrx * 3, hy + hry * 3), (hx - hrx * 3, hy + hry * 3))]
    shine = [L(dp.arc(hx, hy - 0.05 * hry, hrx * 0.86, hry * 0.86, 214, 252, 8), 1.8)]
    amp = 0.06 * hrx

    def ring_(a0, a1, step, rx, ry, dy=0.0):
        """머리 둘레 a0..a1도에 몽글몽글 (크고 작은 동그라미 번갈아) 곱슬 머리숱."""
        return [(hx + math.cos(math.radians(a)) * hrx * rx, hy + math.sin(math.radians(a)) * hry * ry + dy * hry, 0.28 if k % 2 else 0.22)
                for k, a in enumerate(range(a0, a1 + 1, step))]

    def P(a, b):
        """머리 크기 비율 좌표 → 그림 좌표."""
        return (hx + a * hrx, hy + b * hry)
    if view_ == "back":
        pts = ring_(-174, -6, 12, 1.04, 1.04)
        pts += [(hx + sd * hrx * 0.92, hy + k * hry, 0.29 if j % 2 else 0.2) for sd in (-1, 1) for j, k in enumerate((0.1, 0.42, 0.74, 1.0))]
        pts += [(hx + dx * hrx, hy + 1.2 * hry, 0.25 if j % 2 else 0.29) for j, dx in enumerate((-0.72, -0.42, -0.14, 0.14, 0.42, 0.72))]
        waves = [squiggle(P(dx, -0.7), P(dx + 0.04, 1.15), 3.5, amp, 1.2, dx * 5) for dx in (-0.62, -0.3, 0.02, 0.34, 0.64)]
        curls = [curl(*P(a, b), 0.1 * hrx, a0) for a, b, a0 in ((-0.5, -0.55, 200), (0.2, -0.6, 20), (-0.75, 0.6, 300), (0.78, 0.5, 120))]
        return puffs(pts, R), [], [(big, HAIR), (waves + curls, HAIR_DARK), (shine, HAIR_HI)]
    if view_ == "front":
        pts = ring_(-186, 6, 12, 1.06, 1.04, -0.08)
        pts += [(hx + sd * hrx * (1.08 + 0.04 * (j % 2)), hy + k * hry, 0.29 if j % 2 else 0.2) for sd in (-1, 1)
                for j, k in enumerate((0.1, 0.38, 0.66, 0.94))]
        pts += [(hx + sd * hrx * dx, hy + 1.2 * hry, 0.24) for sd in (-1, 1) for dx in (0.84, 1.06)]
        face = [E(hx - 0.84 * hrx, hy - 0.62 * hry, hx + 0.84 * hrx, hy + 1.0 * hry)]
        fringe = [T(P(-1.2, -1.6), P(1.2, -1.6), P(1.0, 0.1), P(0.88, -0.08), P(0.78, -0.22), P(0.66, -0.3), P(0.52, -0.46), P(0.38, -0.46),
                    P(0.22, -0.56), P(0.12, -0.56), P(0.02, -0.66), P(-0.1, -0.56), P(-0.24, -0.54), P(-0.38, -0.44), P(-0.54, -0.44),
                    P(-0.66, -0.3), P(-0.8, -0.2), P(-0.9, -0.06), P(-1.0, 0.1))]
        waves = [squiggle(P(sd * 1.0, -0.2), P(sd * 1.02, 1.15), 3.0, amp, 1.2, sd) for sd in (-1, 1)]
        waves += [squiggle(P(sd * 0.5, -0.98), P(sd * 0.82, -0.5), 1.5, amp * 0.8, 1.0) for sd in (-1, 1)]
        waves += [squiggle(P(sd * 0.22, -1.02), P(sd * 0.3, -0.62), 1.2, amp * 0.7, 1.0) for sd in (-1, 1)]
        curls = [curl(*P(sd * 1.1, b), 0.09 * hrx, a0) for sd in (-1, 1) for b, a0 in ((0.25, 90 + sd * 60), (0.85, 270 - sd * 60))]
        curls += [curl(*P(sd * 0.68, -0.86), 0.08 * hrx, 200 if sd < 0 else -20) for sd in (-1, 1)]
        return puffs(pts, R), [], [(big, HAIR), (face, wz.look().skin), (fringe, HAIR), (waves + curls, HAIR_DARK), (shine, HAIR_HI)]
    # 옆모습 (오른쪽을 봄): 앞쪽은 이마를 덮는 앞머리, 뒤로는 크게 부풀어 등까지
    pts = ring_(-204, -54, 12, 1.04, 1.04, -0.05)
    pts += [(hx - hrx * (1.02 + 0.06 * (j % 2)), hy + k * hry, 0.3 if j % 2 else 0.21) for j, k in enumerate((-0.05, 0.28, 0.6, 0.92))]
    pts += [(hx + dx * hrx, hy + 1.2 * hry, 0.25 if j % 2 else 0.28) for j, dx in enumerate((-0.92, -0.62, -0.32))]
    face = [E(hx - 0.02 * hrx, hy - 0.5 * hry, hx + 1.1 * hrx, hy + 1.05 * hry)]
    fringe = [T(P(-1.3, -1.6), P(1.3, -1.6), P(1.12, -0.4), P(0.98, -0.5), P(0.86, -0.46), P(0.72, -0.58), P(0.56, -0.52), P(0.42, -0.6),
                P(0.3, -0.48), P(0.16, -0.42), P(0.08, -0.26), P(-0.02, 0.2), P(0.02, 0.7), P(-0.3, 1.5), P(-1.3, 1.5))]
    waves = [squiggle(P(-dx, -0.5), P(-dx - 0.06, 1.15), 3.0, amp, 1.2, dx * 4) for dx in (0.3, 0.62, 0.92)]
    waves += [squiggle(P(0.05, -0.95), P(0.5, -0.66), 1.5, amp * 0.8, 1.0)]
    curls = [curl(*P(a, b), 0.09 * hrx, a0) for a, b, a0 in ((-0.4, -0.7, 200), (-1.0, 0.45, 300), (-0.5, 1.0, 120), (0.3, -0.84, 20))]
    return puffs(pts, R), [], [(big, HAIR), (face, wz.look().skin), (fringe, HAIR), (waves + curls, HAIR_DARK), (shine, HAIR_HI)]


def apple_bubble_fx(cx, cy, r=12.0):
    """🍎 사과 생각 풍선 (배고플 때): 빨간 사과 + 꼭지·잎."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    fx += [([C(cx - 2.4, cy + 1.5, 5.4), C(cx + 2.4, cy + 1.5, 5.4)], APPLE, 1.0), ([L([(cx - 4, cy - 1), (cx - 2.6, cy - 2.6)], 1.3)], APPLE_HI, 0),
           ([L([(cx, cy - 3.5), (cx + 0.8, cy - 7.5)], 1.4)], (110, 70, 40), 0), ([E(cx + 1, cy - 8, cx + 6, cy - 5)], LEAF, 0.6)]
    return fx


HERMIONE_LOOK = wz.Look(hair=hermione_hair, hair_col=HAIR, food_fx=apple_bubble_fx, wand=(132, 86, 52), wand_tip=(196, 150, 104), side_ear=False,
                        cap_lift=0.3, back_reach=5.0)


# =====================================================================
# 📖 책 읽기 (옆모습) — 바닥에 앉아 무릎을 세우고 그 위에 펼친 책, 고개 숙여 읽음, 책장 넘김
# =====================================================================
def book_fx(cx, cy, w, turn=None):
    """두 손으로 받쳐 든 펼친 책 (보는 쪽으로 기울인 V — 하얀 두 쪽 + 빨간 표지 테 + 글줄, 짙은 외곽선): 몸·머리 위에 얹는 fx.
    turn: 넘어가는 책장 (0 → 1 진행) 또는 None."""
    h = w * 0.78
    spine = (cx, cy + h * 0.32)
    top = (cx, cy - h * 0.5)
    lt, lb = (cx - w, cy - h * 0.62), (cx - w * 0.96, cy + h * 0.22)
    rt, rb = (cx + w, cy - h * 0.62), (cx + w * 0.96, cy + h * 0.22)
    out_pts = [(lt[0] - 1.6, lt[1] - 1.4), top, (rt[0] + 1.6, rt[1] - 1.4), (rb[0] + 1.8, rb[1] + 2.6), (spine[0], spine[1] + 3.0), (lb[0] - 1.8, lb[1] + 2.6)]
    fx = [([T(*out_pts)], COVER, 0), ([L(out_pts + [out_pts[0]], 1.4)], wz.OUT, 0), ([T(lt, top, spine, lb), T(top, rt, rb, spine)], PAGE, 0)]
    lines = []
    for k in (0.3, 0.5, 0.7):
        for a, b in ((lt, lb), (rt, rb)):
            y0 = a[1] + (b[1] - a[1]) * k
            y1 = top[1] + (spine[1] - top[1]) * k
            lines.append(L([(a[0] + (cx - a[0]) * 0.15, y0 + 0.6), (cx + (a[0] - cx) * 0.18, y1 + 0.4)], 0.9))
    fx.append((lines, PAGE_LINE, 0))
    fx.append(([L([top, spine], 1.0)], PAGE_LINE, 0))
    if turn is not None:  # 넘어가는 한 장: 오른쪽에서 일어서서 왼쪽으로
        a = math.radians(180 * turn)
        dx, dy = math.cos(a) * w * 0.95, -math.sin(a) * w * 0.55
        flip = [top, (cx + dx, top[1] + dy - h * 0.1), (cx + dx * 0.96, spine[1] - h * 0.1 + dy), spine]
        fx += [([T(*flip)], PAGE, 0), ([L(flip + [flip[0]], 1.0)], PAGE_LINE, 0)]
    return fx


def hand_fx(x, y, r):
    """책을 받친 손 (외곽선 두른 살색 동그라미 — 책 위에 손가락 끝이 보이게)."""
    return [([C(x, y, r + 1.0)], wz.OUT, 0), ([C(x, y, r)], wz.look().skin, 0)]


def he_read(sp, i):
    """📖 책 읽기 (4칸 반복, 천천히): 바닥에 앉아 무릎을 세우고 펼친 책을 두 손으로 턱 앞에 받쳐 듦, 고개를 숙여 책을 봄 —
    0·1 = 읽기 (눈이 글줄을 따라감) · 2 = 오른쪽 책장을 넘김 · 3 = 넘어간 책장이 내려앉음."""
    s = wz.ws()
    hip = wz.sit_hip(wz.SIDE_CX - 14)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 4, wz.foot_y()), (knee_x + 7, wz.foot_y())]
    hc = wz.head_center("side", hip, -4, 14, (2, 1.5))
    bw = 8.5 + s["arm"] * 0.22
    bx = max(hc[0] + s["hrx"] * 0.92 + bw * 0.25, knee_x + 4)
    by = min(hc[1] + s["hry"] * 0.9, wz.FLOOR - s["lw"] - bw)
    turn = [None, None, 0.45, 0.85][i]
    arms = wz.reach_all(hip, -4, [(bx - bw * 0.7, by + 3), (bx + bw * 0.55, by + 4)])
    fx = book_fx(bx, by, bw, turn) + hand_fx(arms[1][0], arms[1][1], s["aw"] * 0.55)
    return wz.wizard(hip=hip, rot=-4, head_rot=14, head_d=(2, 1.5), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                     arm_bend=[(0, 1), (0, 1)], eyes="dot", look_=([0.6, 1.4, 1.0, 0.4][i], 1.6), mouth="smile" if i != 2 else "o",
                     robe=dict(a=-36, len=0.4, floor=wz.FLOOR - 1), scarf=dict(a=8, len=0.6), fx=fx)


# =====================================================================
# 🙋 "I know!" (앞모습) — 한 손을 쭉 위로 번쩍, 발끝으로 서서 폴짝폴짝
# =====================================================================
def he_raise_hand(sp, i):
    """🙋 손들기 (2칸 반복): 오른손을 머리 위로 쭉 (손바닥 펴고), 왼손은 허리 앞에 지팡이, 발끝으로 서서 통통 — 눈 반짝 입 벌림."""
    s = wz.ws()
    hip = wz.F(-[2.5, 4.5][i])
    k = 1.3 + (20 - s["arm"]) / 20  # 신나서 쭉 뻗은 팔 (짧은 아기 팔은 더 늘여 얼굴을 가리지 않게)
    return wz.front(hip=hip, arms=[("a", 18, 0.95), ("a", [140, 146][i], k)], arm_bend=[(-1, 0.3), (1, 0)], open_hand=(False, True),
                    arms_front=True, point=True, eyes="happy" if i else "dot", mouth="big", legs=[("a", -4, 1.0), ("a", 4, 1.0)],
                    wand=dict(hand=0, a=-28, k=0.8), fx=sparkle_fx(98, 22 - 2 * i, 3.0 + i))


# =====================================================================
# 👜 구슬 가방 (가방은 앱이 그림 — 손 자리 grip): 들고 걷기 · 질질 끌기 · 쏙 넣기
# =====================================================================
def he_bag_walk(sp, i):
    """👜 가방 들고 걷기 (4칸 반복): 총총 걸으며 가까운 손을 몸 옆 아래로 내려 가방 끈을 쥠 (손 자리 grip — 작은 가방이 매달림),
    먼 손엔 지팡이를 흔듦."""
    hip, legs = wz.walk_legs(i)
    sw = [1, 0, -1, 0][i]
    arms = [("a", 22 * sw, 0.96), ("a", 10 - 4 * sw, 0.95)]
    return wz.wizard(hip=hip, legs=legs, arms=arms, rot=2, robe=dict(a=[8, 12, 8, 12][i], flap=[0, 1.2, 0, -1.2][i]),
                     scarf=dict(a=[-8, -14, -8, -14][i], flap=[0, 1, 0, -1][i]), grip="near", mouth="smile")


def he_bag_drag(sp, i):
    """👜 커진 가방 질질 끌기 (4칸 반복, 느릿느릿): 몸을 앞으로 숙이고 성큼 버티며 걸음, 가까운 팔을 몸 뒤 아래로 쭉 뻗어 끈을 당김
    (손 자리 grip = 몸 뒤 — 가방은 그 뒤 바닥에), 먼 팔은 앞으로 흔듦, 끙 (땀)."""
    s = wz.ws()
    fdx, ndx, fl, nl = wz.WALK[i]
    k = 0.6 + 0.4 * s["leg"] / 22
    x0 = wz.SIDE_CX + 12
    hip = (x0, wz.stand_hip_y() + [1.0, 0.0, 1.0, 0.0][i] + s["leg"] * 0.08)
    rot = 20.0
    legs = [wz.floor_at(x0 - 4 + fdx * k, fl), wz.floor_at(x0 + 2 + ndx * k, nl)]
    sh = wz.shoulders("side", hip, rot)
    hand = wz.reach(sh[1], (sh[1][0] - s["arm"] * 0.95, sh[1][1] + s["arm"] * 0.45), s["arm"], 1.0)
    fx = sweat_fx(hip[0] + s["hrx"] + 14, wz.torso_top(hip[1]) - s["hry"] * 1.4) if i % 2 else []
    return wz.wizard(hip=hip, rot=rot, head_rot=-12, legs=legs, arms=[("a", [40, 20, 0, 20][i], 0.9), hand], arm_bend=[(1, 0.3), (1, -0.3)],
                     eyes="tight" if i % 2 else "dot", brow="angry", mouth="grin", robe=dict(a=[20, 26, 20, 26][i], flap=[1, -1, 1, -1][i]),
                     scarf=dict(a=-40, flap=[1, -1, 1, -1][i]), grip="near", fx=fx)


def he_bag_stuff(sp, i):
    """👜 가방에 쏙 (한 번): 허리를 굽혀 가까운 손으로 몸 앞 아래에 가방을 벌려 듦 (손 자리 grip) → 쏙 넣는 순간 작은 반짝."""
    s = wz.ws()
    hip = wz.H(wz.SIDE_CX - 6, s["leg"] * 0.12)
    rot = [30.0, 36.0][i]
    sh = wz.shoulders("side", hip, rot)
    hand = wz.reach(sh[1], (sh[1][0] + s["arm"] * 0.9, sh[1][1] + s["arm"] * [0.45, 0.6][i]), s["arm"], 1.0)
    fx = sparkle_fx(hand[0] + 6, hand[1] - 9, 3.2) if i else []
    return wz.wizard(hip=hip, rot=rot, head_rot=-10, legs=[wz.floor_at(hip[0] - 8), wz.floor_at(hip[0] + 7)], arms=[("a", -30, 0.9), hand],
                     arm_bend=[(1, 0.4), (0, 1)], eyes="happy" if i else "dot", mouth="open" if i else "smile", robe=dict(a=10),
                     scarf=dict(a=10, len=0.8), grip="near", fx=fx)


BOOK_ANIMS = ["read", "raise_hand"]  # = Rust manifest::BOOK_ANIMS
BAG_ANIMS = ["bag_walk", "bag_drag", "bag_stuff"]  # = Rust manifest::BAG_ANIMS
HERMIONE_POSES = wz.poses_for(HERMIONE_LOOK, {"read": he_read, "raise_hand": he_raise_hand, "bag_walk": he_bag_walk, "bag_drag": he_bag_drag,
                                              "bag_stuff": he_bag_stuff})

HERMIONE = Species(
    id="hermione", name="헤르미온느", walk_speed=88, scale=1.0, base=wz.ROBE, back=wz.ROBE, belly=wz.ROBE, nose=wz.OUT, mouth="w",
    front_tail_color=wz.SCARF, tail_color=wz.SCARF, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + wz.PRANK_ANIMS + wz.BROOM_ANIMS + wz.SPELL_ANIMS + BOOK_ANIMS + BAG_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85),
    stripe=wz.SCARF_GOLD, poses=HERMIONE_POSES, outline=wz.OUT,
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 헤르미온느를 등록함."""
    dp.SPECIES[HERMIONE.id] = HERMIONE


register()


def preview(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hungry", 0), ("roll", 0), ("broom", 0), ("cast", 2),
             ("read", 0), ("read", 2), ("raise_hand", 0), ("bag_walk", 0), ("bag_drag", 1), ("bag_stuff", 1), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "read"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(HERMIONE, root / "ui" / "pets" / HERMIONE.id))
    preview(by_stage, pv / "stages_hermione.png")
    dp.preview_back(by_stage, pv / "climb_hermione.png")
    wz.preview_rows(by_stage, pv / "wizard_hermione.png", wz.BROOM_ANIMS + wz.SPELL_ANIMS + BOOK_ANIMS + BAG_ANIMS + ["climb", "slide"])
    print("ok: hermione")


if __name__ == "__main__":
    main()
