"""거미인간 🕷️ — 동글 털뭉치 거미 펫(spider). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓰고 뼈대만 거미 것.

    python tools/draw_spider.py → ui/pets/spider/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_spider,climb_spider,web_spider}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 머리·몸이 한 덩어리인 동그란 공 + 짧고 동글한 다리 8개 (옆모습: 가까운 다리 4 + 몸 뒤 먼 다리 4, 앞모습: 양쪽 4개씩).
        빨강·파랑 영웅 옷: 윗머리·앞가슴은 빨강 + 정수리에서 퍼지는 얇은 짙은 거미줄 선, 옆구리·다리는 파랑.
        앞가슴에 직접 그린 작은 까만 거미 무늬 (둥근 배 + 머리 + 다리 8가닥). 큰 반짝 눈 둘 (가면 렌즈 없음), 볼터치.
단계: 아기 = 거의 동그란 공, 줄 선 없음, 다리 짧은 혹, 정수리 삐죽 털 → 유아 줄 선 조금 → 어린이 → 청소년 기준 → 어른 조금 큰 몸·긴 다리·느긋한 눈.
특기 동작 (코어 brain_web.rs): shoot 앞다리를 쭉 뻗어 줄 쏘기 · web_swing 한 손을 들어 줄에 매달림 · hero_land 영웅 착지 ·
      web_pull 두 손으로 줄을 잡고 끌어당기기 — 줄 끝 손 자리는 mark_grip (pet.json grip). 벽 타기 climb·slide (뒷모습).
eat에는 밥그릇이 없다 (앱의 소품 창이 띄운다): 앞모습으로 몸을 숙여 입을 밥 더미 바로 위 (프레임 y ≈ 79)에 대고 냠냠.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, L, LINE, T, WHITE, Part, Species, _ST, anger_fx, arc, blush, brows, heart_fx, mix, open_mouth,  # noqa: E402
                       question_fx, sparkle_fx, stage_eye, sweat_fx, view, xf, z_fx)
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FLOOR = 125.0  # 바닥 (발 패드 아래 끝)
RED = (226, 66, 74)  # 영웅 옷 빨강 (윗머리·앞가슴)
RED_WEB = (138, 30, 46)  # 빨강 위의 얇은 거미줄 선
BLUE = (66, 112, 210)  # 옆구리·다리 파랑
BLUE_PAD = (44, 78, 164)  # 발끝 패드
EMBLEM = (30, 26, 38)  # 가슴의 까만 거미 무늬
OUT = (70, 38, 66)  # 외곽선 (짙은 자줏빛 남색)
THREAD = (236, 240, 248)  # 손끝에서 나오는 줄 (그림 안의 짧은 줄·퐁)
DUST, DUST_LINE = (236, 228, 214), (176, 160, 150)

# 단계(lvl)별 — rx·ry: 몸 반지름 · leg: 다리 길이 배 · lw: 다리 굵기 배 · webs: 정수리 거미줄 살 수 · rings: 고리 수 · emb: 가슴 무늬 크기
SPI = {
    0: dict(rx=31.0, ry=30.0, leg=0.68, lw=0.95, webs=0, rings=0, emb=0.85, tuft=True),
    1: dict(rx=33.0, ry=31.0, leg=0.78, lw=1.0, webs=4, rings=1, emb=0.9, tuft=False),
    2: dict(rx=34.5, ry=31.0, leg=0.86, lw=1.0, webs=6, rings=1, emb=0.95, tuft=False),
    3: dict(rx=35.0, ry=30.5, leg=1.0, lw=1.0, webs=6, rings=2, emb=1.0, tuft=False),
    4: dict(rx=36.5, ry=30.0, leg=1.12, lw=1.0, webs=8, rings=2, emb=1.05, tuft=False),
}
LEG_W = 4.8  # 다리 굵기 (청소년)
PAD_R = 3.1  # 발끝 패드 반지름


def spi():
    """지금 단계의 거미 치수."""
    return SPI[_ST["lvl"]]


def body_cy(clear=None, ry=None):
    """서 있을 때 몸 가운데 y: 다리 길이만큼 바닥에서 뜬다 (아기는 거의 바닥에 붙음)."""
    s = spi()
    ry = s["ry"] if ry is None else ry
    clear = 4.0 + 9.0 * s["leg"] if clear is None else clear
    return FLOOR - ry - clear


def on_ellipse(cx, cy, rx, ry, deg, k=0.84):
    """몸 타원 안쪽(k배) 위의 점 (각도: 0 = 오른쪽(앞), 90 = 아래)."""
    r = math.radians(deg)
    return (cx + rx * k * math.cos(r), cy + ry * k * math.sin(r))


def knee_of(root, foot, bend, out):
    """다리 무릎: 뿌리·발 가운데에서 바깥(out = ±1) 위로 bend(현 길이 배)만큼 솟음 — 거미 다리처럼 무릎이 위로."""
    mx, my = (root[0] + foot[0]) / 2, (root[1] + foot[1]) / 2
    dx, dy = foot[0] - root[0], foot[1] - root[1]
    ln = math.hypot(dx, dy) or 1.0
    n1 = (dy / ln, -dx / ln)
    n2 = (-n1[0], -n1[1])
    score = lambda n: n[0] * out * 0.6 - n[1]
    nx, ny = max((n1, n2), key=score)
    return (mx + nx * bend * ln, my + ny * bend * ln)


def leg_part(root, foot, out, *, bend=0.28, outline=True, knee=None, w=None, pad=None, fist=False):
    """다리 한 짝 (파랑, 두 마디 + 동글 발끝 패드). knee를 주면 그 무릎으로. fist: 줄을 쏘고 잡는 손 (패드 조금 크게)."""
    s = spi()
    w = (w or LEG_W) * s["lw"]
    pr = (pad or PAD_R) * (1.18 if fist else 1.0)
    k = knee or knee_of(root, foot, bend, out)
    pf = [C(foot[0], foot[1], pr)]
    return Part([L([root, k, foot], w)] + pf, BLUE, [(pf, BLUE_PAD)], grow=0, outline=outline)


def floor_foot(x, lift=0.0):
    """바닥에 닿는 발 (패드 아래 끝 = FLOOR - lift)."""
    return (x, FLOOR - PAD_R - lift)


def scaled_foot(root, foot):
    """설계 좌표(청소년)의 발을 단계 다리 길이로 (뿌리 기준)."""
    k = spi()["leg"]
    return (root[0] + (foot[0] - root[0]) * k, root[1] + (foot[1] - root[1]) * k)


def emblem_ops(ex, ey, k=1.0, sx=1.0):
    """가슴의 까만 거미 무늬 (직접 그린 단순한 거미): 둥근 배 + 작은 머리 + 다리 8가닥 (양쪽 4개, 무릎이 위로 굽음)."""
    k *= spi()["emb"]
    with xf(sx=sx, pivot=(ex, ey)):
        fns = [E(ex - 3.0 * k, ey - 0.6 * k, ex + 3.0 * k, ey + 5.6 * k), C(ex, ey - 2.6 * k, 2.1 * k)]
        for side in (-1, 1):
            for j, (ky, fy) in enumerate([(-5.4, -2.2), (-3.4, 0.6), (-0.8, 3.6), (1.4, 6.6)]):
                rx0, ry0 = ex + side * 1.6 * k, ey + (-1.6 + j * 1.4) * k
                kx, kyy = ex + side * (5.2 + 0.4 * j) * k, ey + ky * k
                fx_, fy_ = ex + side * (7.4 + 0.2 * j) * k, ey + fy * k
                fns.append(L([(rx0, ry0), (kx, kyy), (fx_, fy_)], 1.05 * k))
    return [(fns, EMBLEM)]


def web_marks(wx, wy, reach, a0=18, a1=162, webs=None, rings=None):
    """정수리 (wx, wy)에서 아래로 부채꼴로 퍼지는 얇은 거미줄 선 (살 + 고리) — 빨강 위에만 (단계: 아기는 없음)."""
    s = spi()
    webs = s["webs"] if webs is None else webs
    rings = s["rings"] if rings is None else rings
    if not webs:
        return []
    fns = []
    angs = [a0 + (a1 - a0) * j / (webs - 1) for j in range(webs)]
    for a in angs:
        r = math.radians(a)
        fns.append(L([(wx, wy), (wx + reach * math.cos(r), wy + reach * math.sin(r))], 0.85))
    for q in range(1, rings + 1):
        rr = reach * q / (rings + 0.6)
        pts = []
        for a in angs:
            r = math.radians(a)
            pts.append((wx + rr * math.cos(r), wy + rr * math.sin(r)))
        # 살 사이를 살짝 처진 줄로 잇는다 (거미집 느낌)
        sag = []
        for p, q2 in zip(pts, pts[1:]):
            m = ((p[0] + q2[0]) / 2, (p[1] + q2[1]) / 2)
            dx, dy = m[0] - wx, m[1] - wy
            d = math.hypot(dx, dy) or 1.0
            sag += [p, (m[0] - dx / d * 1.4, m[1] - dy / d * 1.4)]
        sag.append(pts[-1])
        fns.append(L(sag, 0.8))
    return [(fns, RED_WEB)]


def tuft_part(cx, top):
    """아기 정수리 삐죽 털 두 가닥 (빨강)."""
    if not spi()["tuft"]:
        return []
    return [Part([], RED, spikes=dp.tuft(cx, top), grow=0, sharp=True)]


def mouth_ops(kind, cx, cy, w=4.6):
    """입: None 작은 웃음 · open 벌린 입 · grin 씩 웃음 · frown · pout · chew (볼 빵빵 오물)."""
    if kind is None:
        return [([L(arc(cx, cy - 1.6, w, 3.0, 20, 160, 12), 1.5)], LINE)]
    if kind == "open":
        return open_mouth(cx, cy - 1.0, 3.6, 3.8)
    if kind == "big":
        return open_mouth(cx, cy - 1.4, 5.0, 5.0)
    if kind == "grin":
        return [([L(arc(cx, cy - 2.4, w + 1.2, 3.6, 10, 170, 14), 1.6)], LINE), ([L([(cx + w + 0.6, cy - 2.6), (cx + w + 2.0, cy - 3.6)], 1.3)], LINE)]
    if kind == "frown":
        return [([L(arc(cx, cy + 2.2, w * 0.8, 2.4, 200, 340, 10), 1.5)], LINE)]
    if kind == "pout":
        return [([L(arc(cx, cy + 1.6, 2.6, 1.8, 200, 340, 8), 1.5)], LINE)]
    if kind == "chew":
        return [([E(cx - 2.6, cy - 1.6, cx + 2.6, cy + 1.6)], LINE)]
    raise ValueError(kind)


# =====================================================================
# 옆모습 (오른쪽을 본다)
# =====================================================================
NEAR_A = (30, 64, 108, 146)  # 가까운 다리 뿌리 각도 (앞 → 뒤)
FAR_A = (20, 52, 96, 132)  # 몸 뒤 먼 다리
STAND_NEAR = (40, 17, -13, -37)  # 서 있는 발 x (몸 가운데에서) — 앞 → 뒤
STAND_FAR = (49, 28, 2, -26)
SIDE_CX = 62.0


def stand_feet(cx, offs, shift=(0, 0, 0, 0), lift=(0, 0, 0, 0)):
    """서 있는 발 넷: 몸 가운데 cx에서 offs (단계 다리 길이로 벌림) + 걸음 shift, 든 높이 lift."""
    k = 0.55 + 0.45 * spi()["leg"]
    return [floor_foot(cx + o * k + d, h) for o, d, h in zip(offs, shift, lift)]


def spider_side(sp, *, cx=SIDE_CX, cy=None, near=None, far=None, arm=None, far_arm=None, body=None, whole=None, eyes="dot", mouth=None,
                brow=None, fx=(), ops=(), sq=(1.0, 1.0), look=(0.0, 0.0), grip=None, far_eye=True, bend=0.36, emblem=False, arm_a=None):
    """옆모습 거미. near/far: 발 넷 (앞 → 뒤, 프레임 좌표 — 없으면 서 있는 자리) · arm: 가까운 앞다리 대신 [무릎, 손] (손 = 줄 끝) ·
    far_arm: 먼 앞다리 대신 [무릎, 손] · body: 몸만의 xf (다리 뿌리도 같이 움직임) · whole: 전체 xf · sq: 몸 (가로, 세로) 찌그러짐 ·
    look: 눈동자를 미는 정도 · grip: mark_grip할 점."""
    s = spi()
    rx, ry = s["rx"] * sq[0], s["ry"] * sq[1]
    cy = body_cy(ry=s["ry"]) if cy is None else cy
    bk = dict(body or {})
    with xf(**(whole or {})):
        bm = dp._xf_matrix(**bk) if bk else (1, 0, 0, 0, 1, 0)
        rootp = lambda a: dp._apply(bm, on_ellipse(cx, cy, rx, ry, a))
        near = near or stand_feet(cx, STAND_NEAR)
        far = far or stand_feet(cx, STAND_FAR)
        parts = []
        # 몸 뒤 먼 다리 (외곽선은 실루엣이 두름)
        for j, (a, foot) in enumerate(zip(FAR_A, far)):
            if j == 0 and far_arm:
                parts.append(leg_part(rootp(a + 14), far_arm[-1], 1, knee=far_arm[0], outline=False, fist=True))
                continue
            parts.append(leg_part(rootp(a), foot, 1 if foot[0] >= cx else -1, bend=bend, outline=False))
        with xf(**bk):
            red = E(cx - rx * 0.72, cy - ry * 1.36, cx + rx * 1.32, cy + ry * 0.86)
            marks = [([red], RED)] + web_marks(cx + rx * 0.1, cy - ry * 0.97, ry * 0.62, 12, 150)
            parts.append(Part([E(cx - rx, cy - ry, cx + rx, cy + ry)], BLUE, marks, grow=2))
            parts += tuft_part(cx + rx * 0.05, cy - ry + 1)
            dp.mark_head(cx + rx * 0.05, cy - ry + 2, rx * 1.15, tilt=0)
            ex, ey = cx + rx * 0.3 + look[0], cy - ry * 0.12 + look[1]
            face = blush(cx + rx * 0.16, cy + ry * 0.26, 0.8)
            if far_eye:
                with xf(sx=0.66, pivot=(cx + rx * 0.76, ey)):
                    face += stage_eye(eyes, cx + rx * 0.76 + look[0] * 0.6, ey - 1.0, flip=True)
            face += stage_eye(eyes, ex, ey)
            if brow:
                face += brows(brow, ex, ey + _ST["eye_dy"])
            face += mouth_ops(mouth, cx + rx * 0.58, cy + ry * 0.26, 3.6)
            if emblem:
                face += emblem_ops(cx + rx * 0.2, cy + ry * 0.5, 0.95, sx=0.7)
        legs = []
        for j, (a, foot) in enumerate(zip(NEAR_A, near)):
            if j == 0 and arm:
                legs.append(leg_part(rootp(a + 12 if arm_a is None else arm_a), arm[-1], 1, knee=arm[0], fist=True))
                continue
            legs.append(leg_part(rootp(a), foot, 1 if foot[0] >= cx else -1, bend=bend))
        parts += legs
        if grip:
            dp.mark_grip(*grip)
    return dp.compose(parts, face + list(ops), fx)


WALK_SHIFT = [(4, -4, 4, -4), (0, 0, 0, 0), (-4, 4, -4, 4), (0, 0, 0, 0)]
WALK_LIFT = [(0, 3, 0, 3), (0, 0, 0, 0), (3, 0, 3, 0), (0, 0, 0, 0)]


def sp_walk(sp, i):
    """총총 걷기: 다리 두 무리가 번갈아 앞뒤로 (한 무리씩 살짝 듦), 몸이 통통."""
    bob = [0, -1.5, 0, -1.5][i]
    cy = body_cy() + bob
    sh, lf = WALK_SHIFT[i], WALK_LIFT[i]
    near = stand_feet(SIDE_CX, STAND_NEAR, sh, lf)
    far = stand_feet(SIDE_CX, STAND_FAR, tuple(-d for d in sh), lf[::-1])
    return spider_side(sp, cy=cy, near=near, far=far)


def sp_run(sp, i):
    """다다다 달리기: 앞으로 살짝 기울고 다리를 쭉 뻗음 ↔ 모음, 속도선."""
    reach = [7, 0, -7, 0][i]
    cy = body_cy() - [2, 0, 2, 0][i]
    near = stand_feet(SIDE_CX, STAND_NEAR, (reach + 3, -reach, reach, -reach - 3), (0, 4, 0, 4) if i % 2 == 0 else (4, 0, 4, 0))
    far = stand_feet(SIDE_CX, STAND_FAR, (-reach, reach, -reach, reach), (4, 0, 4, 0) if i % 2 == 0 else (0, 4, 0, 4))
    return spider_side(sp, cy=cy, near=near, far=far, body=dict(rot=6, pivot=(SIDE_CX, cy)), mouth="open",
                       fx=dp.speed_fx(6, (70, 84, 98), 12, [0, -3, 0, -3][i]))


def sp_idle(sp, i):
    """제자리 숨쉬기: 몸이 살짝 부풀었다 가라앉음."""
    k = [1.0, 0.975][i]
    cy = body_cy()
    return spider_side(sp, cy=cy + (1 - k) * spi()["ry"], sq=(1.0 + (1 - k) * 0.6, k))


def sp_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞."""
    eyes, look = [("up", (0.6, -1.0)), ("dot", (1.5, 0)), ("dot", (0.8, 2.6)), ("dot", (0, 0))][i]
    return spider_side(sp, eyes=eyes, look=look)


def sp_sleep(sp, i):
    """납작 엎드려 쿨쿨: 다리를 몸 밑에 접고, 눈 감음, z."""
    s = spi()
    k = [1.0, 0.97][i]
    ry = s["ry"] * 0.84 * k
    cy = FLOOR - 2 - ry - 1.5
    near = [floor_foot(SIDE_CX + o) for o in (24, 9, -8, -22)]
    far = [floor_foot(SIDE_CX + o) for o in (30, 16, -1, -15)]
    fx = [z_fx(104, 34 - 3 * i, 4.0), z_fx(114, 20 - 3 * i, 2.6)]
    return spider_side(sp, cy=cy, sq=(1.06, 0.84 * k), near=near, far=far, eyes="closed", mouth="pout", fx=fx, bend=0.12)


def sp_annoyed(sp, i):
    """엎드린 채 실눈 뾰로통 💢."""
    s = spi()
    ry = s["ry"] * 0.86
    cy = FLOOR - 2 - ry - 1.5
    near = [floor_foot(SIDE_CX + o) for o in (24, 9, -8, -22)]
    far = [floor_foot(SIDE_CX + o) for o in (30, 16, -1, -15)]
    return spider_side(sp, cy=cy, sq=(1.05, 0.86), near=near, far=far, eyes="flat", mouth="pout", bend=0.12,
                       fx=[anger_fx(100 + i, 30 - i, 5.0 + 0.6 * i)])


def air_feet(cx, cy, spread, drop, tuck=0.0):
    """공중 다리: 가까운·먼 발 넷씩 (몸 아래로 늘어뜨리거나 벌림)."""
    s = spi()
    base_n = [(cx + 30, cy + 34), (cx + 13, cy + 40), (cx - 9, cy + 40), (cx - 28, cy + 34)]
    base_f = [(cx + 38, cy + 30), (cx + 22, cy + 37), (cx + 2, cy + 38), (cx - 18, cy + 33)]

    def mv(p):
        dx = (p[0] - cx) * spread
        return scaled_foot((cx + (p[0] - cx) * 0.6, cy + s["ry"] * 0.7), (cx + dx, p[1] + drop - tuck))
    return [mv(p) for p in base_n], [mv(p) for p in base_f]


def sp_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 쭉 뻗어 솟음 (다리 늘어뜨림) → 다리 모음."""
    s = spi()
    if i == 0:
        cy = body_cy() + 5
        near = stand_feet(SIDE_CX, STAND_NEAR, (3, 2, -2, -3))
        far = stand_feet(SIDE_CX, STAND_FAR, (3, 2, -2, -3))
        return spider_side(sp, cy=cy, sq=(1.08, 0.9), near=near, far=far, eyes="tight", mouth="open", bend=0.4)
    cy = [0, 66, 68][i]
    near, far = air_feet(SIDE_CX, cy, [1.0, 1.15, 0.8][i], [0, 10, -4][i])
    return spider_side(sp, cy=cy, sq=(0.96, 1.05) if i == 1 else (1.0, 1.0), near=near, far=far, eyes="happy", mouth="open", bend=0.22)


def sp_fall(sp, i):
    """떨어짐: 다리 여덟을 활짝 벌리고 동그란 눈."""
    cy = 64
    wig = [0, 3][i]
    near = [(SIDE_CX + 44, cy + 6 - wig), (SIDE_CX + 26, cy + 34 + wig), (SIDE_CX - 22, cy + 34 - wig), (SIDE_CX - 44, cy + 8 + wig)]
    far = [(SIDE_CX + 50, cy - 6 + wig), (SIDE_CX + 36, cy + 24 - wig), (SIDE_CX - 10, cy + 38 + wig), (SIDE_CX - 46, cy - 4 - wig)]
    return spider_side(sp, cy=cy, near=near, far=far, eyes="round", mouth="open", bend=0.25)


def sp_dragged(sp, i):
    """목덜미 잡혀 대롱대롱: 다리가 축 늘어지고 살랑."""
    sway = [-6, 6][i]
    cy = 52
    near = [(SIDE_CX + 22 + sway * 0.4, cy + 50), (SIDE_CX + 9 + sway * 0.6, cy + 56), (SIDE_CX - 8 + sway * 0.6, cy + 56), (SIDE_CX - 22 + sway * 0.4, cy + 50)]
    far = [(SIDE_CX + 28 + sway * 0.4, cy + 46), (SIDE_CX + 15 + sway * 0.6, cy + 53), (SIDE_CX + 1 + sway * 0.6, cy + 54), (SIDE_CX - 14 + sway * 0.4, cy + 49)]
    k = spi()["leg"]
    near = [(SIDE_CX + (x - SIDE_CX), cy + (y - cy) * (0.55 + 0.45 * k)) for x, y in near]
    far = [(SIDE_CX + (x - SIDE_CX), cy + (y - cy) * (0.55 + 0.45 * k)) for x, y in far]
    return spider_side(sp, cy=cy, near=near, far=far, eyes="round", mouth="pout", bend=0.12, whole=dict(rot=sway * 0.5, pivot=(SIDE_CX, 20)))


def sp_land(sp, i):
    """착지 (한 번): 찌그러짐 → 복귀."""
    if i == 0:
        cy = body_cy() + 6
        near = stand_feet(SIDE_CX, STAND_NEAR, (5, 2, -2, -5))
        far = stand_feet(SIDE_CX, STAND_FAR, (5, 2, -2, -5))
        return spider_side(sp, cy=cy, sq=(1.12, 0.86), near=near, far=far, eyes="tight", mouth="open", bend=0.42)
    return spider_side(sp, eyes="happy")


def sp_lookup(sp, i):
    """올려다보기: 몸을 뒤로 젖혀 눈이 위로."""
    rot = [-6, -10, -10][i]
    cy = body_cy()
    fx = [sparkle_fx(100, 16, 4.0)] if i == 2 else []
    return spider_side(sp, eyes="up", body=dict(rot=rot, pivot=(SIDE_CX, cy + 20)), fx=[f for g in fx for f in g])


# ---- 🕷️ 특기 동작 ----
def _arm_tip(cx, cy, reach, up):
    """앞다리 손 자리 (몸 가운데에서 앞으로 reach, 위로 up)."""
    return (cx + reach, cy - up)


def thwip_fx(x, y, k=1.0):
    """손끝에서 퐁 터지는 줄 덩어리 (하양) + 짧은 선."""
    return [([C(x + 2.5 * k, y, 3.2 * k), C(x + 6 * k, y - 2.4 * k, 2.2 * k), C(x + 6 * k, y + 2.4 * k, 2.2 * k)], THREAD, 1.0),
            ([L([(x + 9 * k, y - 4.5 * k), (x + 12 * k, y - 6.5 * k)], 1.2), L([(x + 10 * k, y + 0.2), (x + 13.5 * k, y + 0.2)], 1.2),
              L([(x + 9 * k, y + 4.5 * k), (x + 12 * k, y + 6.5 * k)], 1.2)], (120, 130, 160), 0)]


def sp_shoot(sp, i):
    """줄 쏘기 (한 번, 마지막 칸 유지): 앞다리를 당겼다가 → 앞으로 쭉 뻗어 손끝에서 줄이 퐁 → 뻗은 채."""
    s = spi()
    cy = body_cy()
    cx = SIDE_CX - 4
    rx = s["rx"]
    if i == 0:
        hand = (cx + rx * 0.78, cy + s["ry"] * 0.36)
        knee = (cx + rx + 3, cy + s["ry"] * 0.62)
        fx = []
    else:
        hand = (cx + rx + 22 + 2 * s["leg"], cy + s["ry"] * 0.3)
        knee = (cx + rx + 7, cy + s["ry"] * 0.42)
        fx = thwip_fx(hand[0] + 1.5, hand[1], 1.0 if i == 1 else 0.7)
    near = stand_feet(cx, STAND_NEAR, (2, 0, 0, -3))
    return spider_side(sp, cx=cx, cy=cy, near=near, far=stand_feet(cx, STAND_FAR), arm=[knee, hand], eyes="dot", mouth="grin" if i else "open",
                       brow="angry" if i == 0 else None, fx=fx, grip=hand, body=dict(rot=-4 if i else 3, pivot=(cx, cy + 20)))


def sp_web_swing(sp, i):
    """줄 그네: 한 손을 머리 위로 쭉 들어 줄을 잡고, 몸은 아래로 대롱 — 다리를 모아 뒤로 살랑."""
    s = spi()
    cy = 76.0
    cx = SIDE_CX - 2
    hand = (cx + 6, 8.0)
    knee = (cx + 12, cy - s["ry"] - 12)
    wag = [0, 4][i]
    near = [(cx + 20, cy + 36), (cx + 4, cy + 40 + wag * 0.3), (cx - 16, cy + 37 - wag), (cx - 32, cy + 28 - wag)]
    far = [(cx + 26, cy + 32), (cx + 12, cy + 38), (cx - 6, cy + 39 - wag), (cx - 24, cy + 33 - wag)]
    k = 0.6 + 0.4 * s["leg"]
    near = [(x, cy + (y - cy) * k) for x, y in near]
    far = [(x, cy + (y - cy) * k) for x, y in far]
    return spider_side(sp, cx=cx, cy=cy, near=near, far=far, arm=[knee, hand], arm_a=-70, eyes="happy", mouth="big", grip=hand, bend=0.2,
                       fx=dp.speed_fx(10, (60, 74), 10, [0, -3][i]))


def dust_fx(x, y, s=1.0):
    """먼지 퐁퐁 (작은 구름 셋)."""
    blobs = [C(x - 4 * s, y - 2 * s, 3.2 * s), C(x + 1 * s, y - 4 * s, 3.8 * s), C(x + 5.5 * s, y - 1.6 * s, 2.8 * s)]
    inner = [C(x - 4 * s, y - 2 * s, 2.2 * s), C(x + 1 * s, y - 4 * s, 2.8 * s), C(x + 5.5 * s, y - 1.6 * s, 1.8 * s)]
    return [(blobs, DUST_LINE, 1.0), (inner, DUST, 0)]


def sp_hero_land(sp, i):
    """영웅 착지 (한 번): 쪼그려 앞다리 하나(주먹)를 바닥에 쿵 짚고, 뒤 다리 하나를 뒤로 쭉 뻗음 — 먼지 → 버팀 → 반짝 씩."""
    s = spi()
    cx = SIDE_CX + 2
    ry = s["ry"] * 0.9
    cy = FLOOR - ry - 3.0 - 3.0 * s["leg"]
    rx = s["rx"]
    fist = floor_foot(cx + rx + 10)
    knee = (cx + rx + 5, cy + ry * 0.55)
    k = 0.55 + 0.45 * s["leg"]
    back = floor_foot(cx - rx - 20 * k)
    near = [fist, floor_foot(cx + 8 * k), floor_foot(cx - 12 * k), back]
    far = [floor_foot(cx + 30 * k), floor_foot(cx + 18 * k), floor_foot(cx - 2 * k), floor_foot(cx - 18 * k)]
    fx = []
    if i == 0:
        fx = dust_fx(fist[0] + 4, FLOOR - 1, 1.1) + dust_fx(back[0] - 2, FLOOR - 1, 0.9)
    elif i == 1:
        fx = dust_fx(fist[0] + 6, FLOOR - 3, 0.7)
    else:
        fx = sparkle_fx(cx + rx + 6, cy - ry - 2, 4.0)
    return spider_side(sp, cx=cx, cy=cy, sq=(1.06, 0.9), near=near, far=far, arm=[knee, fist], arm_a=62, eyes="dot", brow="angry",
                       mouth="grin", fx=fx, body=dict(rot=8, pivot=(cx, FLOOR)), bend=0.3)


def sp_web_pull(sp, i):
    """줄 끌어당기기: 두 앞다리로 앞의 줄을 잡고 몸을 뒤로 젖혀 영차 (손이 당겨졌다 다시 뻗음)."""
    s = spi()
    cx = SIDE_CX - 6
    cy = body_cy()
    pull = [0, 5, 9, 5][i]
    rx = s["rx"]
    hand = (cx + rx + 20 - pull, cy + s["ry"] * 0.2 + pull * 0.3)
    knee = (cx + rx + 4 - pull * 0.5, cy + s["ry"] * 0.5)
    hand2 = (hand[0] + 5, hand[1] + 3)
    knee2 = (knee[0] + 5, knee[1] + 2)
    near = stand_feet(cx, STAND_NEAR, (6, 2, -3, -6))
    far = stand_feet(cx, STAND_FAR, (6, 2, -3, -6))
    fx = sweat_fx(cx - rx * 0.2, cy - s["ry"] - 4) if i == 2 else []
    return spider_side(sp, cx=cx, cy=cy, near=near, far=far, arm=[knee, hand], far_arm=[knee2, hand2], eyes="tight" if i == 2 else "dot",
                       mouth="grin", body=dict(rot=-8 - pull * 0.4, pivot=(cx, FLOOR)), grip=hand, fx=fx)


# =====================================================================
# 앞모습
# =====================================================================
FRONT_A = (-10, 14, 38, 62)  # 오른쪽 다리 뿌리 각도 (위 → 아래). 왼쪽은 거울
FRONT_FEET = (60, 47, 34, 21)  # 오른쪽 발 x (가운데에서)


def front_feet(lift=(0, 0, 0, 0), spread=1.0):
    """서 있는 오른쪽 발 넷 (위 다리부터). 왼쪽은 거울."""
    k = min(0.9, 0.62 + 0.38 * spi()["leg"]) * spread
    return [floor_foot(64 + o * k, h) for o, h in zip(FRONT_FEET, lift)]


def _mirror(p):
    return (128 - p[0], p[1])


def spider_front(sp, *, cy=None, right=None, left=None, arms=(None, None), whole=None, body=None, eyes="dot", mouth=None, brow=None,
                 fx=(), ops=(), sq=(1.0, 1.0), held=(), face_dy=0.0, bend=0.2, look=(0, 0)):
    """앞모습 거미. right/left: 발 넷 (위 다리부터, 프레임 좌표 — left는 거울로 적지 않고 화면 좌표 그대로; 없으면 서 있는 자리) ·
    arms: (왼쪽, 오른쪽) 맨 위 다리를 팔로 [무릎, 손] · held: 몸과 팔 사이 Part 함수 · face_dy: 얼굴만 위아래로."""
    s = spi()
    rx, ry = s["rx"] * sq[0], s["ry"] * sq[1]
    cy = body_cy(ry=s["ry"]) if cy is None else cy
    cx = 64.0
    with xf(**(whole or {})):
        bm = dp._xf_matrix(**(body or {})) if body else (1, 0, 0, 0, 1, 0)
        right = right or front_feet()
        left = left or [_mirror(p) for p in right]
        parts = []
        with xf(**(body or {})):
            red_cap = E(cx - rx - 8, cy - ry - 8, cx + rx + 8, cy - ry * 0.02)
            red_chest = E(cx - rx * 0.6, cy - ry * 0.9, cx + rx * 0.6, cy + ry + 8)
            marks = [([red_cap, red_chest], RED)] + web_marks(cx, cy - ry * 0.98, ry * 0.58)
            body_p = Part([E(cx - rx, cy - ry, cx + rx, cy + ry)], BLUE, marks, grow=2)
            dp.mark_head(cx, cy - ry + 2, rx * 1.2)
            fdy = face_dy
            ex = rx * 0.38
            ey = cy - ry * 0.12 + fdy
            f_ops = blush(cx - rx * 0.6, cy + ry * 0.2 + fdy, 0.85) + blush(cx + rx * 0.6, cy + ry * 0.2 + fdy, 0.85)
            f_ops += stage_eye(eyes, cx - ex + look[0], ey + look[1]) + stage_eye(eyes, cx + ex + look[0], ey + look[1], flip=True)
            if brow:
                f_ops += brows(brow, cx - ex, ey + _ST["eye_dy"]) + brows(brow, cx + ex, ey + _ST["eye_dy"], flip=True)
            f_ops += mouth_ops(mouth, cx, cy + ry * 0.24 + fdy)
            f_ops += emblem_ops(cx, cy + ry * 0.58 + fdy * 0.3, 1.05)
        rootp = lambda a, side: dp._apply(bm, on_ellipse(cx, cy, rx * side, ry, a))
        for side, feet, arm in ((1, right, arms[1]), (-1, left, arms[0])):
            for j, (a, foot) in enumerate(zip(FRONT_A, feet)):
                if j == 0 and arm:
                    continue
                parts.append(leg_part(rootp(a, side), foot, side, bend=bend, outline=False))
        parts.append(body_p)
        parts += tuft_part(cx, cy - ry + 1)
        parts += [h() for h in held]
        for side, arm in ((1, arms[1]), (-1, arms[0])):
            if arm:
                parts.append(leg_part(rootp(FRONT_A[0] - 6, side), arm[-1], side, knee=arm[0], fist=True))
    return dp.compose(parts, f_ops + list(ops), fx)


def sp_sit_front(sp, i):
    """정면 앉기 (마지막 장 깜빡)."""
    return spider_front(sp, eyes="closed" if i == 3 else "dot", face_dy=[0, 0.6, 0, 0][i])


def sp_wave(sp, i):
    """오른쪽 맨 위 다리를 번쩍 들어 흔들흔들."""
    cy = body_cy()
    hand = [(110, cy - 40), (100, cy - 46), (112, cy - 38), (102, cy - 46)][i]
    knee = (104, cy - 14)
    return spider_front(sp, arms=(None, [knee, hand]), eyes="happy" if i % 2 else "dot", mouth="open",
                        body=dict(rot=[-2, 0, -2, 0][i], pivot=(64, FLOOR)))


def sp_tilt(sp, i):
    """고개(몸) 갸웃 + ?."""
    deg = [8, 12, 12][i]
    cy = body_cy()
    return spider_front(sp, body=dict(rot=deg, pivot=(64, cy + 20)), fx=[question_fx(106, 26 - i)], mouth="pout")


def sp_belly(sp, i):
    """발라당 누워 배 보이기: 몸이 낮게 납작, 다리 여덟이 위로 꼬물꼬물."""
    s = spi()
    ry = s["ry"] * 0.82
    cy = FLOOR - 2 - ry - 2
    w = [0, 3, 0, -3][i]
    k = 0.6 + 0.4 * s["leg"]
    right = [(64 + 44 * k + w, cy - 34 * k), (64 + 40 * k - w, cy - 22 * k), (64 + 34 * k + w, cy - 34 * k - 4), (64 + 18 * k - w, cy - 36 * k)]
    left = [(64 - 44 * k - w, cy - 34 * k), (64 - 40 * k + w, cy - 22 * k), (64 - 34 * k - w, cy - 34 * k - 4), (64 - 18 * k + w, cy - 36 * k)]
    return spider_front(sp, cy=cy, sq=(1.08, 0.82), right=right, left=left, eyes="happy", mouth="open", bend=0.2)


def sp_stretch(sp, i):
    """기지개 (한 번): 앞으로 엎드려 앞다리 쭉 → 하품 → 앉기."""
    if i == 2:
        return spider_front(sp, eyes="happy")
    cy = body_cy() + 6
    k = 0.6 + 0.4 * spi()["leg"]
    arms = ([(20, cy - 18), (12, cy - 34 * k)], [(108, cy - 18), (116, cy - 34 * k)])
    return spider_front(sp, cy=cy, sq=(1.06, 0.9), arms=arms, eyes="closed", mouth="big" if i == 1 else "open")


def sp_heart(sp, i):
    """하트를 안고 볼 발그레, 작은 하트가 떠오름."""
    cy = body_cy()
    hy = cy + 10

    def held():
        return dp.held_heart(hy)
    arms = ([(36, cy + 4), (54, hy + 4)], [(92, cy + 4), (74, hy + 4)])
    return spider_front(sp, arms=arms, held=[held], eyes="happy", mouth="open", fx=dp.heart_burst(i))


def sp_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 깔깔 (> <)."""
    rot = [-7, 7, -4][i]
    cy = body_cy()
    k = 0.6 + 0.4 * spi()["leg"]
    arms = ([(24, cy - 10), (18, cy - 26 * k + i)], [(104, cy - 10), (110, cy - 26 * k - i)])
    return spider_front(sp, arms=arms, eyes="tight", mouth="big", body=dict(rot=rot, pivot=(64, FLOOR)))


def sp_hungry(sp, i):
    """배고파 시무룩: 처진 눈썹, 작은 입, 파리 생각 풍선."""
    import draw_frog
    dy = [0, 1.5, 0][i]
    cy = body_cy() + 2
    fx = draw_frog.fly_bubble_fx(102 - 2 * i, 22, 12.0)
    return spider_front(sp, cy=cy, sq=(1.02, 0.96), eyes="dot", brow="sad", mouth="frown", face_dy=dy, fx=fx)


EAT_MOUTH = 79.0  # 먹을 때 입 (프레임 y) — 그릇 밥 더미 바로 위


def sp_eat(sp, i):
    """냠냠 (그릇 없음): 몸을 앞으로 숙여 입을 밥 더미 바로 위에 — 입 벌림 ↔ 볼 빵빵, 부스러기."""
    s = spi()
    cy = body_cy() - 2
    ry = s["ry"]
    face_dy = EAT_MOUTH - (cy + ry * 0.24)
    mouth = ["open", "chew", "open", "chew"][i]
    fx = [f for x, y in dp.EAT_CRUMBS[i] for f in [([C(x, y, 1.6)], (255, 236, 200), 0.8)]]
    return spider_front(sp, cy=cy, eyes="happy" if i % 2 else "dot", mouth=mouth, face_dy=face_dy, look=(0, 2), fx=fx,
                        whole=dict(sy=1.0, dy=[0, 1, 0, 1][i]))


def sp_level_up(sp, i):
    """만세: 맨 위 두 다리를 번쩍, ^^, 반짝이."""
    cy = body_cy() - [0, 3, 1][i]
    arms = ([(28, cy - 22), (20, cy - 46)], [(100, cy - 22), (108, cy - 46)])
    return spider_front(sp, cy=cy, arms=arms, eyes="happy", mouth="big", fx=dp.level_up_stars(i))


# =====================================================================
# 구르기 (회전 원 안)
# =====================================================================
def sp_roll(sp, i):
    """동그랗게 말린 거미 공 (다리는 배 밑으로 쏙): 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    r = 29.0
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            red = E(cx - r - 6, cy - r - 6, cx + r + 6, cy + r * 0.2)
            ball = Part([C(cx, cy, r)], BLUE, [([red, E(cx - r * 0.55, cy - r, cx + r * 0.55, cy + r + 4)], RED)] +
                        web_marks(cx, cy - r * 0.98, r * 0.55), grow=0)
            nubs = []
            for a in (40, 62, 84, 106, 128, 150):
                p = (cx + (r + 2) * math.cos(math.radians(a)), cy + (r + 2) * math.sin(math.radians(a)))
                nubs.append(Part([C(p[0], p[1], 4.2)], BLUE, [([C(p[0], p[1], 2.4)], BLUE_PAD)], grow=0, outline=True))
            dp.mark_head(cx, cy - r + 2, r * 1.1)
            face = blush(cx - 17, cy + 6, 0.8) + blush(cx + 17, cy + 6, 0.8)
            face += stage_eye("happy", cx - 11, cy - 4) + stage_eye("happy", cx + 11, cy - 4, flip=True)
            face += mouth_ops("open" if i else None, cx, cy + 8)
            face += emblem_ops(cx, cy + 16, 0.8)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([ball] + nubs, face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면을 기어오름. 얼굴은 안 보이고, 빨강 등 + 거미줄 선, 다리 여덟이 벽에 쩍
# =====================================================================
BACK_UP = [(44, 26), (30, 46), (28, 76), (40, 100)]  # 왼쪽 다리 발 (위 → 아래). 오른쪽은 거울


def spider_back(sp, *, reach=(1.0, 0.0), dy=0.0, sy=1.0, scratch=False, sweat=False):
    """뒷모습: 둥근 등 (빨강 위 + 거미줄, 파랑 아래) 뒤로 다리 여덟이 벽에 쩍 (reach: 왼쪽·오른쪽 위 다리 올림 몫)."""
    s = spi()
    cx, cy = 64.0, 70.0 + dy
    rx, ry = s["rx"], s["ry"] * sy
    k = 0.6 + 0.4 * s["leg"]
    parts = []
    for side, up in ((-1, reach[0]), (1, reach[1])):
        for j, (fx_, fy_) in enumerate(BACK_UP):
            a = [-60, -20, 20, 60][j]
            root = on_ellipse(cx, cy, rx * side, ry, a if side > 0 else a, 0.7)
            root = (cx + (root[0] - cx), root[1])
            foot = (cx + side * (64 - fx_) * k, cy + (fy_ - 70) * k - (10 * up if j < 2 else -4 * up))
            parts.append(leg_part(root, foot, side, bend=0.3, outline=False))
    red = E(cx - rx - 6, cy - ry - 6, cx + rx + 6, cy + ry * 0.35)
    marks = [([red], RED)] + web_marks(cx, cy - ry * 0.98, ry * 1.05, 20, 160)
    parts.append(Part([E(cx - rx, cy - ry, cx + rx, cy + ry)], BLUE, marks, grow=2))
    parts += tuft_part(cx, cy - ry + 1)
    dp.mark_head(cx, cy - ry + 2, rx * 1.15)
    fx = []
    if scratch:
        for side in (-1, 1):
            x0 = cx + side * (64 - BACK_UP[0][0]) * k
            fx.append(([L([(x0 + d, cy - 30 * k - 8), (x0 + d, cy - 30 * k + 4)], 1.2) for d in (-3, 0, 3)], (150, 150, 170), 0))
    if sweat:
        fx += sweat_fx(cx + rx * 0.8, cy - ry * 0.7)
    return dp.compose(parts, [], fx)


CLIMB_REACH = [(1.0, 0.0), (0.5, 0.5), (0.0, 1.0), (0.5, 0.5)]


def sp_climb(sp, i):
    """뒷모습으로 기어오름: 왼쪽·오른쪽 위 다리가 번갈아 위로, 몸 살짝 늘어남."""
    return spider_back(sp, reach=CLIMB_REACH[i], sy=[1.0, 1.03, 1.0, 1.03][i], dy=[0, -1, 0, -1][i])


def sp_slide(sp, i):
    """뒷모습으로 주르륵: 다리를 위로 쭉, 긁힌 자국 3줄, 땀 한 방울."""
    return spider_back(sp, reach=(1.0, 1.0), sy=0.96, dy=[0, 2][i], scratch=True, sweat=True)


SPIDER_POSES = {"walk": sp_walk, "run": sp_run, "roll": sp_roll, "idle": sp_idle, "look": sp_look, "sleep": sp_sleep, "hop": sp_hop,
                "fall": sp_fall, "dragged": sp_dragged, "land": sp_land, "sit_front": sp_sit_front, "wave": sp_wave, "tilt": sp_tilt,
                "belly": sp_belly, "stretch": sp_stretch, "heart": sp_heart, "tickle": sp_tickle, "annoyed": sp_annoyed,
                "hungry": sp_hungry, "eat": sp_eat, "level_up": sp_level_up, "climb": sp_climb, "slide": sp_slide, "lookup": sp_lookup,
                "shoot": sp_shoot, "web_swing": sp_web_swing, "hero_land": sp_hero_land, "web_pull": sp_web_pull}
SPIDER_POSES = {k: (v if k == "roll" else dp._fit(v)) for k, v in SPIDER_POSES.items()}  # roll은 회전 원 안 그대로
WEB_ANIMS = ["shoot", "web_swing", "hero_land", "web_pull"]  # = Rust manifest::WEB_ANIMS

SPIDER = Species(
    id="spider", name="거미인간", walk_speed=95, scale=1.0, base=BLUE, back=RED, belly=RED, nose=OUT, mouth="w",
    front_tail_color=BLUE, tail_color=BLUE, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + WEB_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=RED_WEB, poses=SPIDER_POSES, outline=OUT,
    # 모든 단계 반짝이 큰 눈 (어른만 느긋한 눈) · 색은 조금만 파스텔 (빨강·파랑 옷이 또렷하게) · 크기는 SPI가 정함 (fit 1)
    stage_tweak={"baby": dict(fit=1.0, pale=0.12, eye=1.6), "kinder": dict(fit=1.0, pale=0.06, eye=1.45),
                 "child": dict(fit=1.0, pale=0.0, eye=1.32, sparkle=True, smile=False), "teenager": dict(fit=1.0, eye=1.22, sparkle=True),
                 "adult": dict(fit=1.0, eye=1.1, sparkle=False, calm=True)},
)


def register():
    """draw_pets.SPECIES에 거미인간을 등록함."""
    dp.SPECIES[SPIDER.id] = SPIDER


register()


def preview_spider(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hop", 1), ("fall", 0), ("roll", 0),
             ("shoot", 2), ("web_swing", 0), ("hero_land", 0), ("web_pull", 2), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "hero_land"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(SPIDER, root / "ui" / "pets" / SPIDER.id))
    preview_spider(by_stage, pv / "stages_spider.png")
    dp.preview_back(by_stage, pv / "climb_spider.png")
    print("ok: spider")


if __name__ == "__main__":
    main()
