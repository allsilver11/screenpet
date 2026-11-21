"""짱구 🖍️ — 아주 굵은 까만 ㅅ자 눈썹, 까만 동그란 눈 + 하얀 점, 아래가 넓은 주먹밥 머리 (볼 통통, 이마 훤히), 뒤통수만 덮는 까만 머리, 빨간 반팔 티셔츠, 노란 반바지, 하얀 양말·운동화의 꼬마 펫(jjanggu).
몸·기본 동작은 draw_wizard (해리포터와 같은 사람 뼈대)를 쓰고, 로브 대신 티셔츠·반바지 (Look.skirt·sleeve·shorts·belt·socks·shoe)로 입힘.

    python tools/draw_jjanggu.py → ui/pets/jjanggu/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_jjanggu,climb_jjanggu,moves_jjanggu}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

특기 그림 (코어 brain_jjanggu.rs — JJANGGU_ANIMS): 🍑 butt_dance 몸을 숙여 엉덩이를 내밀고 좌우로 씰룩 (반바지 입은 채) ·
🦸 action_pose 허리에 손 → 팔 엇갈려 준비 → 손 뻗어 "Action beam!" 반짝 빔 · 😴 roll_lazy 바닥에 누워 뒹굴뒹굴 · lie 엎드려 다리 까딱 ·
💕 flirt 앞모습, 볼에 손을 대고 쑥스럽게 몸을 배배 (볼 빨갛게) · 🍪 snack 앉아서 초코비 상자에서 하나씩 쏙.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_wizard as wz  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, heart_fx, sparkle_fx, xf  # noqa: E402
from draw_spider import polar  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

SKIN, SKIN_SHADE = (255, 222, 194), (238, 188, 160)  # 살색 · 먼 쪽 팔다리
HAIR = (30, 26, 32)  # 짧은 까만 머리
HAIR_HI = (86, 84, 104)  # 머리 윤기
BROW = (22, 18, 24)  # 아주 굵은 까만 눈썹
SHIRT, SHIRT_FAR = (226, 50, 48), (186, 36, 40)  # 빨간 반팔 티셔츠
SHORTS, SHORTS_FAR = (250, 208, 58), (222, 176, 40)  # 노란 반바지
SOCKS = (252, 252, 252)  # 하얀 양말
SHOE, SHOE_FAR, SHOE_HI = (246, 246, 250), (214, 216, 228), (176, 180, 200)  # 하얀 운동화
BOX, BOX_BAND, BOX_STAR = (214, 64, 44), (255, 214, 80), (120, 60, 34)  # 🍪 초코비 상자 (빨강 + 노란 띠 + 갈색 별)
CHOCO = (120, 66, 36)  # 초코비 한 알
BEAM, BEAM_CORE, BEAM_EDGE = (255, 228, 96), (255, 255, 232), (240, 160, 50)  # 🦸 액션 빔
WIGGLE = (150, 150, 176)  # 🍑 씰룩 자국


def lobe(hx, hy, hrx, hry, cx, cy, rx, ry):
    """머리 반지름 배로 잡은 볼 덩이 타원 (가운데 (cx, cy), 반지름 (rx, ry))."""
    return E(hx + (cx - rx) * hrx, hy + (cy - ry) * hry, hx + (cx + rx) * hrx, hy + (cy + ry) * hry)


def jj_hair(view_, hx, hy, hrx, hry, q):
    """주먹밥 머리 + 짧은 까만 머리: 아래쪽 볼이 크게 부풀어 머리가 아래로 넓음 (앞·뒷모습은 양 볼, 옆모습은 앞으로 불룩한 볼).
    이마는 훤히 드러나고 머리카락은 정수리 위 (앞모습)·뒤통수 (옆모습)만 덮음. → (머리 도형에 더할 도형, 삐죽 조각, 머리 안 무늬)."""
    sky = hy - hry * 2.0
    if view_ == "side":
        cheek = [lobe(hx, hy, hrx, hry, 0.64, 0.5, 0.86, 0.62), lobe(hx, hy, hrx, hry, -0.26, 0.36, 0.74, 0.6),
                 lobe(hx, hy, hrx, hry, 0.2, 0.62, 0.78, 0.48)]
        z = [(0.16, -1.2), (0.06, -0.94), (-0.12, -0.74), (-0.3, -0.46), (-0.4, -0.16), (-0.46, 0.2), (-0.62, 0.46), (-1.6, 0.5)]
        edge = [(hx + a * hrx, hy + b * hry) for a, b in z]
        shine = [L(dp.arc(hx, hy, hrx * 0.8, hry * 0.8, 200, 236, 6), 2.0)]
        return cheek, [], [([T(*edge, (hx - 1.6 * hrx, sky))], HAIR), (shine, HAIR_HI)]
    k = 0.9 if view_ == "back" else 1.0  # 뒷모습은 볼이 조금 덜 보임
    cheeks = [lobe(hx, hy, hrx, hry, sd * 0.56 * k, 0.52, 0.86 * k, 0.6) for sd in (-1, 1)] + [lobe(hx, hy, hrx, hry, 0.0, 0.62, 0.84, 0.48)]
    if view_ == "back":
        shine = [L(dp.arc(hx, hy, hrx * 0.76, hry * 0.76, 218, 262, 8), 2.0)]
        z = [(1.6, 0.4), (0.96, 0.42), (0.6, 0.62), (0.3, 0.56), (0.0, 0.66), (-0.3, 0.56), (-0.6, 0.62), (-0.96, 0.42), (-1.6, 0.4)]
        edge = [(hx + a * hrx, hy + b * hry) for a, b in z]
        return cheeks, [], [([T(*edge, (hx - 1.6 * hrx, sky), (hx + 1.6 * hrx, sky))], HAIR), (shine, HAIR_HI)]
    z = [(1.6, -0.36), (0.98, -0.4), (0.78, -0.62), (0.42, -0.78), (0.0, -0.82), (-0.42, -0.78), (-0.78, -0.62), (-0.98, -0.4), (-1.6, -0.36)]
    edge = [(hx + a * hrx, hy + b * hry) for a, b in z]
    ears = [E(hx + sd * hrx - 3.2, hy - 0.08 * hry - 4.4, hx + sd * hrx + 3.2, hy - 0.08 * hry + 4.4) for sd in (-1, 1)]
    return cheeks + ears, [], [([T(*edge, (hx - 1.6 * hrx, sky), (hx + 1.6 * hrx, sky))], HAIR)]


def jj_eye(kind, cx, cy, flip, sx=1.0):
    """짱구 눈: 까만 동그라미 + 가운데 하얀 점 + 위에 가는 눈꺼풀 선 (뜬 눈 dot·up·round만 — 다른 표정은 None → 단계 눈)."""
    if kind not in ("dot", "up", "round"):
        return None
    hrx = wz.ws()["hrx"]
    r = 0.23 * hrx * (1.08 if kind == "round" else 1.0)
    dy = -0.25 * r if kind == "up" else 0.0
    with xf(sx=sx, pivot=(cx, cy)):
        return [([E(cx - r, cy - r * 1.06, cx + r, cy + r * 1.06)], BROW),
                ([C(cx, cy + dy, r * 0.36)], dp.WHITE),
                ([L(dp.arc(cx, cy + r * 0.5, r * 1.5, r * 2.0, 222, 318, 10), 0.75)], wz.OUT)]


def brow(pts, w):
    """굵은 ㅅ자 눈썹 하나 (바깥 끝 → 꼭대기 → 안쪽 끝)."""
    return L(pts, w)


def jj_extra(view_, hx, hy, hrx, hry, far_eye, near_eye):
    """아주 굵은 까만 ㅅ자 눈썹 (짱구 얼굴의 상징): 바깥 끝은 낮고 안쪽에서 솟았다가 살짝 내려옴 — 가운데는 또렷이 떨어짐.
    옆모습은 가까운 눈썹 + 먼 눈썹 끝."""
    w = 0.17 * hry
    if view_ == "front":
        out = []
        for (ex, ey), sd in ((far_eye, -1), (near_eye, 1)):
            out.append(brow([(ex + sd * 0.2 * hrx, ey - 0.34 * hry), (ex - sd * 0.03 * hrx, ey - 0.54 * hry),
                             (ex - sd * 0.16 * hrx, ey - 0.44 * hry)], w))
        return [(out, BROW)]
    if view_ == "side":
        (xf_, yf), (xn, yn) = far_eye, near_eye
        return [([brow([(xn - 0.17 * hrx, yn - 0.34 * hry), (xn + 0.02 * hrx, yn - 0.54 * hry), (xn + 0.13 * hrx, yn - 0.46 * hry)], w),
                  L([(xf_ - 0.02 * hrx, yf - 0.48 * hry), (xf_ + 0.09 * hrx, yf - 0.4 * hry)], w * 0.9)], BROW)]
    return []


def no_wand(hand, a, length):
    """지팡이 대신 아무것도 안 듦 (끝 자리 = 손)."""
    return None, hand


def chocobi_box(cx, cy, k=1.0, tilt=0.0):
    """🍪 초코비 상자 (빨강 상자 + 노란 띠 + 갈색 별) Part — 가운데 (cx, cy), 높이 약 14k."""
    w, h = 5.2 * k, 7.0 * k
    with xf(rot=tilt, pivot=(cx, cy)):
        body = T((cx - w, cy - h), (cx + w, cy - h), (cx + w, cy + h), (cx - w, cy + h))
        band = T((cx - w, cy - h * 0.15), (cx + w, cy - h * 0.15), (cx + w, cy + h * 0.38), (cx - w, cy + h * 0.38))
        star = T(*[(cx + (2.4 * k if j % 2 == 0 else 1.0 * k) * math.cos(math.radians(j * 36 - 90)),
                    cy + h * 0.12 + (2.4 * k if j % 2 == 0 else 1.0 * k) * math.sin(math.radians(j * 36 - 90))) for j in range(10)])
        lid = L([(cx - w, cy - h * 0.72), (cx + w, cy - h * 0.72)], 0.9 * k)
        return Part([body], BOX, [([band], BOX_BAND), ([star], BOX_STAR), ([lid], (150, 36, 30))], outline=True, grow=0)


def chocobi_bubble_fx(cx, cy, r=12.0):
    """🍪 초코비 생각 풍선 (배고플 때): 작은 초코비 상자."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    w, h = 4.6, 6.4
    fx += [([T((cx - w, cy - h), (cx + w, cy - h), (cx + w, cy + h), (cx - w, cy + h))], BOX, 1.0),
           ([T((cx - w, cy - 1), (cx + w, cy - 1), (cx + w, cy + 2.4), (cx - w, cy + 2.4))], BOX_BAND, 0),
           ([C(cx, cy + 0.7, 1.3)], BOX_STAR, 0)]
    return fx


JJ_LOOK = wz.Look(hair=jj_hair, hair_col=HAIR, skin=SKIN, skin_shade=SKIN_SHADE, extra=jj_extra, food_fx=chocobi_bubble_fx, prop=no_wand,
                  robe=SHIRT, robe_far=SHIRT_FAR, robe_in=SHIRT_FAR, jumper=SHIRT, pants=SHORTS, pants_far=SHORTS_FAR, scarf=False,
                  skirt=False, sleeve=0.55, shorts=0.6, belt=SHORTS, socks=SOCKS, shoe=SHOE, shoe_far=SHOE_FAR, shoe_hi=SHOE_HI,
                  dims=dict(hrx=0.98, hry=1.04, th=0.9, leg=0.78, arm=0.92, tw=0.98), cap_lift=0.05, eye_k=0.8, eye_draw=jj_eye, back_reach=8.0)


def s_():
    """지금 단계 치수."""
    return wz.ws()


# =====================================================================
# 🍑 엉덩이 춤 (옆모습) — 몸을 앞으로 숙이고 엉덩이를 뒤로 쑥, 좌우로 씰룩씰룩 "buri buri~"
# =====================================================================
BUTT = [(-2.0, 0.0, 4.0), (0.0, -2.0, -2.0), (2.0, 0.0, -6.0), (0.0, -2.0, 0.0)]  # 칸마다 엉덩이 (dx, dy, 기울기)


def jj_butt_dance(sp, i):
    """🍑 다리를 펴고 몸을 거의 가로로 숙여 (머리는 앞 아래, 얼굴은 이쪽을 보며 ^o^) 노란 반바지 엉덩이를 뒤로 쑥 —
    칸마다 엉덩이가 좌우·위아래로 씰룩, 뒤에 씰룩 자국. 손은 무릎께로 축."""
    s = s_()
    dx, dy, tilt = BUTT[i]
    rot = 80.0
    hip = (wz.SIDE_CX - 22 + dx * 0.5, wz.stand_hip_y() - 1 + dy * 0.5)
    legs = [wz.floor_at(wz.SIDE_CX - 26 + dx * 0.3), wz.floor_at(wz.SIDE_CX - 18 + dx * 0.3)]
    sh = wz.shoulders("side", hip, rot)
    arms = [wz.reach(sh[0], (sh[0][0] - 3 - dx, wz.FLOOR - 6), s["arm"], 0.95), wz.reach(sh[1], (sh[1][0] + 2 + dx, wz.FLOOR - 8), s["arm"], 0.95)]
    bx, by = hip[0] - 2 + dx, hip[1] - s["tw"] * 0.5 + dy
    rx, ry = s["tw"] * 0.85, s["tw"] * 0.72
    with xf(rot=tilt, pivot=(bx, by)):
        butt = Part([E(bx - rx, by - ry, bx + rx, by + ry)], SHORTS, [([L([(bx - rx * 0.1, by - ry * 0.95), (bx + rx * 0.05, by - ry * 0.25)], 1.3)], SHORTS_FAR)],
                    outline=True, grow=0)
    side = -1 if i in (0, 3) else 1
    fx = [([L(dp.arc(bx - rx - 2, by, 4 + k * 4, 6 + k * 4, 140 if side < 0 else 160, 200 if side < 0 else 220, 6), 1.5)], WIGGLE, 0) for k in (0, 1)]
    fx += [([L(dp.arc(bx, by - ry - 3, 5 + k * 4, 3 + k * 3, 200 if side < 0 else 220, 280 if side < 0 else 320, 6), 1.5)], WIGGLE, 0) for k in (0, 1)]
    return wz.wizard(hip=hip, rot=rot, head_rot=-rot + 18, legs=legs, leg_bend=[(1, 0), (1, 0)], arms=arms, arm_bend=[(-1, 0.3), (-1, 0.3)],
                     eyes="happy", mouth="big", fx=fx, after=[butt])


# =====================================================================
# 🦸 액션가면 흉내 — 허리에 손 (앞모습) → 팔 엇갈려 준비 → 손 뻗어 "Action beam!" 반짝 빔
# =====================================================================
def beam_fx(x0, y0, x1, k=1.0):
    """🦸 손끝 (x0, y0)에서 오른쪽 x1까지 뻗는 노란 빔 (하얀 속 + 반짝이)."""
    w = 6.0 * k
    fx = [([T((x0, y0 - w * 0.4), (x1, y0 - w), (x1, y0 + w), (x0, y0 + w * 0.4))], BEAM_EDGE, 0),
          ([T((x0, y0 - w * 0.25), (x1, y0 - w * 0.72), (x1, y0 + w * 0.72), (x0, y0 + w * 0.25))], BEAM, 0),
          ([L([(x0, y0), (x1, y0)], w * 0.5)], BEAM_CORE, 0)]
    fx += sparkle_fx(x0 + 2, y0, 4.5 * k)
    for t, dy, r in ((0.45, -9, 2.6), (0.75, 8, 2.2), (0.95, -6, 1.8)):
        fx += sparkle_fx(x0 + (x1 - x0) * t, y0 + dy * k, r * k)
    return fx


def jj_action_pose(sp, i):
    """🦸 (한 번, 마지막 칸 유지) 0 = 앞모습 두 주먹을 허리에 척 (팔꿈치 바깥), 씩 웃음 · 1 = 옆모습 가까운 팔을 세우고 먼 팔을 가로로 엇갈려 준비 ·
    2 = 가까운 팔을 앞으로 쭉, 편 손끝에서 노란 빔이 쭉 (반짝이)."""
    s = s_()
    if i == 0:
        top = wz.torso_top()
        y = top + s["th"] * 0.74
        hands = [(64 - s["tw"] * 0.95, y), (64 + s["tw"] * 0.95, y)]
        return wz.front(arms=hands, arm_bend=[(-1, -0.4), (1, -0.4)], eyes="dot", brow="angry", mouth="grin", head_rot=-3)
    legs = [wz.floor_at(wz.SIDE_CX - 9), wz.floor_at(wz.SIDE_CX + 8)]
    hip = wz.H(wz.SIDE_CX - 4, 1.0)
    rot = [0, -6, 6][i]
    sh = wz.shoulders("side", hip, rot)
    if i == 1:
        up = wz.reach(sh[0], (sh[0][0] - s["arm"] * 0.2, sh[0][1] - s["arm"] * 0.95), s["arm"], 0.95)
        across = wz.reach(sh[1], (sh[1][0] + s["arm"] * 0.75, sh[1][1] + s["arm"] * 0.15), s["arm"], 0.95)
        return wz.wizard(hip=hip, rot=rot, legs=legs, arms=[up, across], arm_bend=[(-1, 0.6), (0, 1)], eyes="dot",
                         brow="angry", mouth="open", fx=sparkle_fx(up[0] - 2, up[1] - 8, 3.0))
    hand = wz.reach(sh[1], (sh[1][0] + s["arm"], sh[1][1] + 1), s["arm"], 1.0)
    tip = (hand[0] + s["aw"] * 0.9, hand[1])
    return wz.wizard(hip=hip, rot=rot, legs=legs, arms=[("a", 20, 0.9), hand], arm_bend=[(1, 0.4), (0, 1)], open_hand=(False, True), eyes="dot",
                     brow="angry", mouth="big", fx=beam_fx(tip[0], tip[1], 126.0, 0.8 + 0.2 * _lvl() / 4))


def _lvl():
    """지금 단계 번호 (0 아기 … 4 어른)."""
    return dp._ST["lvl"]


# =====================================================================
# 😴 뒹굴뒹굴 · 엎드려 다리 까딱
# =====================================================================
def jj_roll_lazy(sp, i):
    """😴 바닥에 벌렁 누워 (머리는 왼쪽, 얼굴은 위) 몸을 좌우로 뒹굴뒹굴 — 칸마다 몸이 이쪽저쪽으로 기울고 팔다리가 축, 느긋한 눈 ‿."""
    s = s_()
    tilt = [-12, 0, 12, 0][i]
    hip = (wz.SIDE_CX + 12, wz.stand_hip_y())
    legs = [("a", 6 + 4 * (i % 2), 0.95), ("a", -8 - 4 * (i % 2), 0.92)]
    arms = [("a", 40, 0.95), ("a", -30 + tilt, 0.95)]
    lie_y = wz.FLOOR - s["hrx"] * 1.12 - 1
    whole = dict(rot=-90 + tilt * 0.5, pivot=hip, dy=lie_y - hip[1], dx=4)
    fx = [([L(dp.arc(30 + 4 * (i % 2), lie_y - 26, 6, 4, 200, 340, 6), 1.4)], WIGGLE, 0)] if i in (0, 2) else []
    return wz.wizard(hip=hip, legs=legs, arms=arms, arm_bend=[(-1, 0.4), (1, 0.4)], eyes="closed", mouth="smile", whole=whole, fx=fx)


def jj_lie(sp, i):
    """엎드려 턱을 괴고 (얼굴은 오른쪽 앞) 무릎을 굽혀 두 발을 번갈아 까딱까딱 — 느긋하게 빈둥."""
    s = s_()
    hip = (wz.SIDE_CX - 30, wz.FLOOR - s["lw"] * 0.6)
    rot = 78
    kick = [(0.2, 0.9), (0.9, 0.2), (0.55, 0.55)][i]
    legs = [polar(hip, -180 + 15 + 55 * (1 - k), s["leg"] * 0.98) for k in kick]
    sh = wz.shoulders("side", hip, rot)
    chin = (sh[1][0] + s["arm"] * 0.55, wz.FLOOR - s["aw"] * 0.6)
    arms = [wz.reach(sh[0], (chin[0] + 3, chin[1]), s["arm"], 1.0), wz.reach(sh[1], (chin[0] + 5, chin[1] - 1), s["arm"], 1.0)]
    return wz.wizard(hip=hip, rot=rot, head_rot=-62, head_d=(0, -2), legs=legs, leg_bend=[(-1, 1), (-1, 1)], far_limbs_front=False, arms=arms, arm_bend=[(1, 1), (1, 1)],
                     eyes="happy" if i == 2 else "dot", mouth="smile", look_=(1.0, 0.0),
                     fx=[([L(dp.arc(f[0], f[1], 6, 5, 200, 320, 6), 1.4)], WIGGLE, 0) for f in legs[:1] if i < 2] + [z_note(i)])


def z_note(i):
    """느긋한 ♪ 하나 (빈둥빈둥)."""
    x, y = 96 + 2 * i, 40 - 2 * i
    return ([C(x, y + 6, 2.4), L([(x + 2.2, y + 6), (x + 2.2, y - 2), (x + 6, y)], 1.4)], (120, 120, 150), 0)


# =====================================================================
# 💕 쑥스러운 작업 (앞모습) — 볼에 손을 대고 몸을 배배 꼼, 볼이 빨갛고 작은 하트
# =====================================================================
FLIRT_BLUSH = (255, 128, 140)


def jj_flirt(sp, i):
    """💕 앞모습: 한 손은 볼에 대고 다른 손은 뒷짐, 몸을 칸마다 이쪽저쪽 배배 꼬며 눈은 ^^, 볼 빨갛게 (보통보다 크게), 하트 퐁."""
    s = s_()
    top = wz.torso_top()
    hrx, hry = s["hrx"], s["hry"]
    rot = [-6, 0, 6][i]
    head_y = top + wz.OVERLAP - hry
    cheek = (64 + 0.62 * hrx, head_y + 0.42 * hry)
    arms = [(64 - s["tw"] * 0.4, top + s["th"] * 0.8), wz.reach(wz.shoulders("front", wz.F(), rot)[1], cheek, s["arm"], 1.0)]
    ops = [([E(64 + sd * 0.56 * hrx - 5.0, head_y + 0.44 * hry - 2.6, 64 + sd * 0.56 * hrx + 5.0, head_y + 0.44 * hry + 2.4)], FLIRT_BLUSH)
           for sd in (-1, 1)]
    return wz.front(rot=rot, head_rot=[-8, -4, 2][i], arms=arms, arm_bend=[(-1, 0.6), (1, 0.4)], arms_front=True, eyes="happy", mouth="smile",
                    open_hand=(False, True), ops=ops, fx=[heart_fx(108, 30 - 3 * i, 4.2 + 0.4 * i)] + ([heart_fx(118, 14 - 2 * i, 2.8)] if i else []))


# =====================================================================
# 🍪 초코비 (앞모습) — 바닥에 앉아 상자를 들고 하나씩 쏙
# =====================================================================
def jj_snack(sp, i):
    """🍪 앞모습으로 바닥에 털썩 앉아 먼 손 (왼쪽)에 초코비 상자, 가까운 손이 상자 → 입으로 한 알 쏙 → 오물오물 (4칸 반복)."""
    s = s_()
    hip = (64.0, wz.FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - s["leg"] * 0.7, wz.foot_y()), (64 + s["hip"] + s["leg"] * 0.7, wz.foot_y())]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    head_y = top + wz.OVERLAP - s["hry"]
    mouth = (64.0, head_y + 0.62 * s["hry"])
    box = (64 - s["tw"] * 0.55, top + s["th"] * 0.78)
    up = i in (1, 2)
    hand = (mouth[0] + 3, mouth[1] + 3) if up else (box[0] + 6, box[1] - 4)
    arms = [(box[0] - 2, box[1] + 5), hand]
    after = [chocobi_box(box[0], box[1], 1.15 + 0.15 * _lvl() / 4, -10)]
    fx = [([C(hand[0], hand[1] - 2, 2.2)], CHOCO, 0.8)] if i == 1 else []
    return wz.front(hip=hip, legs=legs, arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], arms_front=True, after=after,
                    eyes="happy" if i >= 2 else "dot", mouth="o" if i == 1 else ("chew" if i == 2 else "smile"), fx=fx)


def grounded(fn):
    """누운 그림 (뒹굴·엎드림)을 바닥에 붙임: 기울어 뜬 만큼 프레임을 통째로 내려 맨 아래가 바닥선 (FLOOR + 1)에 닿게."""
    def f(sp, i):
        from PIL import Image
        im = fn(sp, i)
        u = dp.out_u()
        y1 = im.getchannel("A").getbbox()[3]
        dy = int(round((wz.FLOOR + 1) * u)) - y1
        if dy <= 0:
            return im
        out = Image.new("RGBA", im.size, (0, 0, 0, 0))
        out.paste(im, (0, dy))
        dp.shift_head(0, dy / u)
        return out
    f.__name__ = fn.__name__
    f.__doc__ = fn.__doc__
    return f


JJANGGU_ANIMS = ["butt_dance", "action_pose", "roll_lazy", "lie", "flirt", "snack"]  # = Rust manifest::JJANGGU_ANIMS
JJ_POSES = wz.poses_for(JJ_LOOK, {"butt_dance": jj_butt_dance, "action_pose": jj_action_pose, "roll_lazy": grounded(jj_roll_lazy), "lie": grounded(jj_lie),
                                  "flirt": jj_flirt, "snack": jj_snack})
SIZE = 0.8  # 해리 (0.85)보다 조금 작은 다섯 살 꼬마

JJANGGU = Species(
    id="jjanggu", name="짱구", walk_speed=88, scale=1.0, base=SHIRT, back=SHIRT, belly=SHIRT, nose=wz.OUT, mouth="w",
    front_tail_color=SHIRT, tail_color=SHIRT, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + JJANGGU_ANIMS, stage_size=dict.fromkeys(STAGES, SIZE), stripe=SHORTS,
    poses=JJ_POSES, outline=wz.OUT,
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 짱구를 등록함."""
    dp.SPECIES[JJANGGU.id] = JJANGGU


register()


def preview(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hungry", 0), ("roll", 0),
             ("butt_dance", 0), ("action_pose", 2), ("roll_lazy", 0), ("lie", 0), ("flirt", 1), ("snack", 1), ("climb", 0), ("idle", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "butt_dance"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(JJANGGU, root / "ui" / "pets" / JJANGGU.id))
    preview(by_stage, pv / "stages_jjanggu.png")
    dp.preview_back(by_stage, pv / "climb_jjanggu.png")
    wz.preview_rows(by_stage, pv / "moves_jjanggu.png", JJANGGU_ANIMS + ["climb", "slide"])
    print("ok: jjanggu")


if __name__ == "__main__":
    main()
