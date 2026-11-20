"""거미인간 🕷️ — 빨강·파랑 쫄쫄이를 입은 사람 모양 꼬마 영웅 펫(spider). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓰고,
뼈대는 산타(draw_christmas)처럼 부품마다 자기 외곽선을 두른 두 발 꼬마 — 팔다리는 관절(어깨·팔꿈치·손 / 엉덩이·무릎·발)을 IK로 굽힌다.

    python tools/draw_spider.py → ui/pets/spider/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_spider,climb_spider}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 큰 머리 (키의 45% 남짓) + 작은 몸 + 짧은 팔다리 둘씩. 머리는 통째로 빨간 가면 + 얇은 짙은 거미줄 선 + 까만 테 큰 하얀 렌즈 둘
        (표정마다 렌즈 모양이 바뀜: ^^ 웃음 · 동그란 놀람 · ‿ 졸림 · 눈꺼풀 내린 뾰로통 · > < 질끈). 입은 없고, 먹을 때만 가면 아래를 걷어 올림.
        몸: 가슴·어깨는 빨강 V (거미줄 선), 옆구리·팔·다리는 파랑, 장갑·장화는 빨강, 가슴에 직접 그린 까만 거미 무늬.
단계 (HERO): 아기 = 동글 우주복 꼬마 (머리 아주 큼, 팔다리 뭉툭, 줄 선 없음) → 유아 가면 줄 선 → 어린이 몸에도 줄 선 → 청소년 → 어른 키 크고 어깨 넓음.
특기 동작 (코어 brain_web.rs): shoot 팔을 쭉 뻗어 손목을 까딱 (손가락 둘 펴고) 줄 쏘기 · web_swing 한 팔을 머리 위로 들어 줄에 매달림 ·
      hero_land 영웅 착지 (쪼그려 앞 주먹을 바닥에, 뒤 다리 쭉, 다른 팔은 뒤로) · web_pull 두 손으로 줄을 잡고 영차 ·
      web_whirl 🌀 버티고 서서 가까운 팔을 머리 위로 들어 줄 끝의 물건을 빙빙 ·
      hang_down 🙃 웅크린 채 거꾸로 뒤집혀 위로 뻗은 줄을 두 손·두 발로 꼭 잡고 살랑 (머리가 아래) — 줄 끝 손 자리는 mark_grip (pet.json grip).
      벽 타기 climb·slide = 뒷모습으로 손발을 벽에 붙이고 엉금엉금 (거미인간 벽 타기).
eat에는 밥그릇이 없다 (앱의 소품 창이 띄운다): 무릎 꿇고 가면 아래를 걷어 올려 입을 밥 더미 바로 위 (프레임 y ≈ 79)에 대고 냠냠.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, _ST, anger_fx, mix, question_fx, sparkle_fx, sweat_fx, view, xf, z_fx  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

FLOOR = 125.0  # 바닥 (장화 바닥)
RED = (226, 66, 74)  # 쫄쫄이 빨강 (가면·가슴·장갑·장화)
RED_FAR = (196, 50, 64)  # 먼 쪽 장갑·장화 (조금 어둡게)
RED_WEB = (138, 30, 46)  # 빨강 위의 얇은 거미줄 선
BLUE = (66, 112, 210)  # 옆구리·팔·다리 파랑
BLUE_FAR = (50, 86, 172)  # 먼 쪽 팔다리
EMBLEM = (30, 26, 38)  # 가슴의 까만 거미 무늬
LENS = (255, 255, 255)  # 가면 눈 렌즈
LENS_RIM = (28, 24, 36)  # 렌즈 까만 테
MASK_BLUSH = (250, 136, 150)  # 가면 위 볼터치
MOUTH_LINE = (132, 30, 46)  # 가면 너머 비치는 입 (짙은 빨강)
SKIN, SKIN_LINE = (255, 222, 198), (196, 120, 110)  # 걷어 올린 가면 아래 턱
OUT = (52, 32, 62)  # 외곽선 (짙은 남보라)
THREAD = (236, 240, 248)  # 손목에서 퐁 터지는 줄
DUST, DUST_LINE = (236, 228, 214), (176, 160, 150)

# 단계(lvl)별 — hrx·hry: 머리 반지름 · tw: 어깨 반폭 · th: 몸 높이 · hip: 다리 뿌리 반간격 · leg: 다리 길이 · lw: 다리 굵기 ·
# arm: 팔 길이 · aw: 팔 굵기 · webs·rings: 가면 거미줄 살·고리 · body_web: 몸에도 줄 선 · lens: 렌즈 배율 · emb: 가슴 무늬 배율
HERO = {
    0: dict(hrx=27.0, hry=25.0, tw=13.5, th=21.0, hip=6.6, leg=9.5, lw=10.4, arm=13.0, aw=8.8, webs=0, rings=0, body_web=False, lens=1.12, emb=0.8),
    1: dict(hrx=26.4, hry=24.2, tw=13.4, th=22.5, hip=6.8, leg=13.5, lw=9.8, arm=15.0, aw=8.3, webs=10, rings=1, body_web=False, lens=1.07, emb=0.85),
    2: dict(hrx=25.8, hry=23.4, tw=13.6, th=24.5, hip=7.0, leg=18.0, lw=9.4, arm=17.5, aw=7.9, webs=12, rings=2, body_web=True, lens=1.03, emb=0.92),
    3: dict(hrx=25.2, hry=22.8, tw=14.6, th=26.5, hip=7.2, leg=22.0, lw=9.2, arm=20.0, aw=7.6, webs=14, rings=2, body_web=True, lens=1.0, emb=1.0),
    4: dict(hrx=24.6, hry=22.2, tw=16.4, th=29.5, hip=8.0, leg=26.0, lw=9.6, arm=23.0, aw=7.9, webs=14, rings=3, body_web=True, lens=0.94, emb=1.1),
}
OVERLAP = 5.0  # 머리가 몸 윗부분을 덮는 깊이


def hs():
    """지금 단계의 꼬마 영웅 치수."""
    return HERO[_ST["lvl"]]


def foot_y():
    """바닥에 선 발 관절 y (장화 바닥 = FLOOR)."""
    return FLOOR - hs()["lw"] * 0.5


def stand_hip_y():
    """똑바로 섰을 때 엉덩이 관절 y."""
    return foot_y() - hs()["leg"]


def polar(p, deg, d):
    """p에서 각도 deg (0 = 아래, 90 = 오른쪽, 180 = 위)로 d만큼 간 점."""
    r = math.radians(deg)
    return (p[0] + math.sin(r) * d, p[1] + math.cos(r) * d)


def ik(a, b, ln, prefer):
    """a → b 팔다리 (마디 둘, 길이 합 ln)의 가운데 관절: 짧으면 prefer 쪽으로 굽힘, 길면 쭉 편다."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    d = math.hypot(dx, dy) or 1e-6
    mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
    if d >= ln * 0.995:
        return (mx, my)
    h = math.sqrt(max(0.0, (ln / 2) ** 2 - (d / 2) ** 2))
    n = (-dy / d, dx / d)
    if n[0] * prefer[0] + n[1] * prefer[1] < 0:
        n = (-n[0], -n[1])
    return (mx + n[0] * h, my + n[1] * h)


def target(root, spec, ln, side):
    """팔다리 끝 자리: 숫자 = 각도로 쭉 · ("a", 각도, 몫) = 그 각도로 몫만큼 (굽힘) · (x, y) = 그 점. side: 바깥 방향 (각도의 가로 부호)."""
    if isinstance(spec, (int, float)):
        r = math.radians(spec)
        return (root[0] + side * math.sin(r) * ln, root[1] + math.cos(r) * ln)
    if spec[0] == "a":
        r = math.radians(spec[1])
        return (root[0] + side * math.sin(r) * ln * spec[2], root[1] + math.cos(r) * ln * spec[2])
    return spec


def limb(root, end, ln, w, color, end_col, prefer, *, boot=False, toe=0.0, fingers=None):
    """팔 또는 다리 한 짝 (자기 외곽선): 파랑 마디 둘 + 끝은 빨강 (장화: 정강이 아래 + 동글 장화 · 장갑: 손목 + 동글 주먹).
    toe: 장화 코 방향 (옆모습 +1/-1, 앞모습 0) · fingers: 손가락 둘 (거미줄 쏘는 손 — 펴는 방향 각도)."""
    j = ik(root, end, ln, prefer)
    shapes_ = [L([root, j, end], w)]
    seg = math.hypot(j[0] - end[0], j[1] - end[1]) or 1.0
    k = min(0.5, w * 0.95 / seg) if boot else min(0.36, w * 0.75 / seg)  # 정강이·팔뚝 끝에서 이 몫만큼이 장화·장갑 (짧은 팔다리도 파랑이 보이게)
    cuff = (end[0] + (j[0] - end[0]) * k, end[1] + (j[1] - end[1]) * k)
    red = [L([cuff, end], w + 2.5)]
    if boot:
        x, y = end
        b = E(x - w * 0.62 + min(toe, 0) * w * 0.42, y - w * 0.5, x + w * 0.62 + max(toe, 0) * w * 0.42, y + w * 0.5)
        shapes_.append(b)
        red.append(b)
    else:
        fist = C(end[0], end[1], w * 0.66)
        shapes_.append(fist)
        red.append(fist)
        if fingers is not None:
            for da in (-28, 28):
                tip = polar(end, fingers + da, w * 1.05)
                f = L([end, tip], w * 0.34)
                shapes_.append(f)
                red.append(f)
    return Part(shapes_, color, [(red, end_col)], outline=True, grow=0), j


def emblem_ops(ex, ey, k=1.0, sx=1.0):
    """가슴의 까만 거미 무늬 (직접 그린 단순한 거미): 둥근 배 + 작은 머리 + 다리 8가닥 (양쪽 4개, 무릎이 위로 굽음)."""
    k *= hs()["emb"]
    with xf(sx=sx, pivot=(ex, ey)):
        fns = [E(ex - 1.9 * k, ey - 0.4 * k, ex + 1.9 * k, ey + 4.4 * k), C(ex, ey - 1.8 * k, 1.5 * k)]
        for side in (-1, 1):
            for j, (ky, fy) in enumerate([(-4.4, -1.6), (-2.6, 0.6), (-0.4, 3.2), (1.6, 5.8)]):
                rx0, ry0 = ex + side * 1.0 * k, ey + (-1.2 + j * 1.1) * k
                kx, kyy = ex + side * (3.8 + 0.3 * j) * k, ey + ky * k
                fx_, fy_ = ex + side * (5.4 + 0.2 * j) * k, ey + fy * k
                fns.append(L([(rx0, ry0), (kx, kyy), (fx_, fy_)], 0.95 * k))
    return [(fns, EMBLEM)]


def web_lines(wx, wy, reach, a0, a1, spokes, rings, w=0.8):
    """(wx, wy)에서 a0 → a1도 (0 = 오른쪽, 90 = 아래) 부채꼴로 퍼지는 얇은 거미줄 선 (살 + 살짝 처진 고리)."""
    if not spokes:
        return []
    full = (a1 - a0) % 360 == 0
    n = spokes if full else spokes - 1
    angs = [a0 + (a1 - a0) * j / max(1, n) for j in range(spokes)]
    fns = [L([(wx, wy), (wx + reach * math.cos(math.radians(a)), wy + reach * math.sin(math.radians(a)))], w) for a in angs]
    for q in range(1, rings + 1):
        rr = reach * q / (rings + 0.7)
        pts = [(wx + rr * math.cos(math.radians(a)), wy + rr * math.sin(math.radians(a))) for a in angs + ([angs[0]] if full else [])]
        sag = []
        for p, q2 in zip(pts, pts[1:]):
            m = ((p[0] + q2[0]) / 2, (p[1] + q2[1]) / 2)
            dx, dy = m[0] - wx, m[1] - wy
            d = math.hypot(dx, dy) or 1.0
            sag += [p, (m[0] - dx / d * rr * 0.12, m[1] - dy / d * rr * 0.12)]
        sag.append(pts[-1])
        fns.append(L(sag, w))
    return [(fns, RED_WEB)]


# =====================================================================
# 가면 얼굴: 렌즈 (표정) · 볼터치 · 비치는 입
# =====================================================================
def lens_ops(kind, cx, cy, side, brow=None, sx=1.0):
    """렌즈 하나 (side: -1 왼쪽, +1 오른쪽 — 바깥 위로 기울어진 큰 물방울 렌즈). kind: dot · up · round · happy · closed · tight · flat · squint · spiral.
    brow: angry (안쪽이 눌린 눈꺼풀) · sad (바깥이 처진 눈꺼풀)."""
    s = hs()
    k = s["lens"]
    rx, ry = 0.3 * s["hrx"] * k, 0.4 * s["hry"] * k
    t = 1.7  # 까만 테
    tilt = 26 * side
    ops = []
    with xf(sx=sx, pivot=(cx, cy)):
        if kind in ("happy", "closed", "tight"):
            if kind == "happy":  # ^^
                pts = dp.arc(cx, cy + ry * 0.5, rx * 1.05, ry * 0.95, 200, 340, 12)
            elif kind == "closed":  # ‿ (졸음)
                pts = dp.arc(cx, cy - ry * 0.35, rx * 1.05, ry * 0.6, 25, 155, 12)
            else:  # > <
                ix = -side
                pts = [(cx - ix * rx * 0.8, cy - ry * 0.62), (cx + ix * rx * 0.7, cy), (cx - ix * rx * 0.8, cy + ry * 0.62)]
            return [([L(pts, 2.7 + 2 * t)], LENS_RIM), ([L(pts, 2.7)], LENS)]
        if kind == "round":
            rx, ry, tilt = rx * 1.12, ry * 1.08, tilt * 0.4
        if kind == "up":
            cy -= 1.4
        with xf(rot=tilt, pivot=(cx, cy)):
            ops += [([E(cx - rx - t, cy - ry - t, cx + rx + t, cy + ry + t)], LENS_RIM), ([E(cx - rx, cy - ry, cx + rx, cy + ry)], LENS)]
        if kind == "spiral":
            pts = [(cx + 0.42 * a * math.cos(a), cy + 0.42 * a * math.sin(a)) for a in (j * 0.35 for j in range(1, 30))]
            ops.append(([L(pts, 1.3)], LENS_RIM))
        lid = None
        o, i_ = cx + side * (rx + t + 1.5), cx - side * (rx + t + 1.5)  # 바깥·안쪽 끝 x
        if kind == "flat":
            lid = ((i_, cy - ry * 0.05), (o, cy - ry * 0.05))
        elif kind == "squint":
            lid = ((i_, cy - ry * 0.2), (o, cy - ry * 0.2))
        elif brow == "angry":
            lid = ((i_, cy - ry * 0.05), (o, cy - ry * 0.9))
        elif brow == "sad":
            lid = ((i_, cy - ry * 0.85), (o, cy - ry * 0.15))
        if lid:
            top = cy - ry - t - 3
            ops += [([T(lid[0], lid[1], (lid[1][0], top), (lid[0][0], top))], RED), ([L([lid[0], lid[1]], t * 1.6)], LENS_RIM)]
        if kind == "squint":
            bot = cy + ry + t + 3
            b0, b1 = (i_, cy + ry * 0.45), (o, cy + ry * 0.45)
            ops += [([T(b0, b1, (o, bot), (i_, bot))], RED), ([L([b0, b1], t * 1.6)], LENS_RIM)]
    return ops


def mouth_ops(kind, cx, cy):
    """가면 너머로 살짝 비치는 입 (짙은 빨강): None 없음 · open · big · grin · frown · pout · chew."""
    if kind is None:
        return []
    if kind == "open":
        return [([E(cx - 2.6, cy - 2.0, cx + 2.6, cy + 2.4)], MOUTH_LINE)]
    if kind == "big":
        return [([E(cx - 3.6, cy - 2.6, cx + 3.6, cy + 3.6)], MOUTH_LINE)]
    if kind == "grin":
        return [([L(dp.arc(cx, cy - 2.2, 4.4, 3.2, 20, 160, 10), 1.5)], MOUTH_LINE)]
    if kind == "frown":
        return [([L(dp.arc(cx, cy + 2.0, 3.4, 2.2, 205, 335, 10), 1.5)], MOUTH_LINE)]
    if kind == "pout":
        return [([L(dp.arc(cx, cy + 1.4, 2.2, 1.5, 205, 335, 8), 1.4)], MOUTH_LINE)]
    if kind == "chew":
        return [([E(cx - 2.2, cy - 1.3, cx + 2.2, cy + 1.3)], MOUTH_LINE)]
    raise ValueError(kind)


def mask_face(view_, hx, hy, eyes, brow, mouth, look, blush_on, mask_up):
    """가면 얼굴 ops (렌즈 둘 + 볼터치 + 입) — 머리 변환 안에서 부름. 옆모습은 얼굴이 오른쪽으로 쏠리고 먼 렌즈는 좁게."""
    s = hs()
    hrx, hry = s["hrx"], s["hry"]
    ops = []
    lx, ly = look
    if view_ == "front":
        fy = hy + 0.02 * hry + ly
        for side in (-1, 1):
            ops += lens_ops(eyes, hx + side * 0.42 * hrx + lx, fy, side, brow)
        if blush_on:
            ops += [([E(hx + side * 0.62 * hrx - 3.6, hy + 0.5 * hry - 2, hx + side * 0.62 * hrx + 3.6, hy + 0.5 * hry + 1.6)], MASK_BLUSH)
                    for side in (-1, 1)]
        mx, my = hx + lx * 0.5, hy + 0.64 * hry
    else:
        fy = hy + 0.02 * hry + ly
        ops += lens_ops(eyes, hx + 0.72 * hrx + lx * 0.6, fy - 0.5, 1, brow, sx=0.5)
        ops += lens_ops(eyes, hx + 0.22 * hrx + lx, fy, -1, brow, sx=0.9)
        if blush_on:
            ops.append(([E(hx + 0.18 * hrx - 3.4, hy + 0.5 * hry - 2, hx + 0.18 * hrx + 3.4, hy + 0.5 * hry + 1.6)], MASK_BLUSH))
        mx, my = hx + 0.62 * hrx, hy + 0.62 * hry
    if mask_up:  # 가면 아래를 걷어 올린 턱 + 입 (먹을 때)
        return ops
    return ops + mouth_ops(mouth, mx, my)


# =====================================================================
# 꼬마 영웅 한 장
# =====================================================================
def hero(*, view_="side", hip=None, rot=0.0, head_rot=0.0, head_d=(0.0, 0.0), legs=None, arms=None, leg_bend=None, arm_bend=None,
         eyes="dot", brow=None, mouth=None, look=(0.0, 0.0), blush_on=True, fx=(), ops=(), held=(), whole=None, grip=None,
         mask_up=None, arms_front=False, fingers=(None, None), far_limbs_front=False, grip_red=False):
    """꼬마 영웅. view_: side (오른쪽을 봄) · front · back · hip: 엉덩이 관절 (없으면 똑바로 선 자리) · rot: 몸 기울기 (엉덩이 기준, 시계 방향) ·
    head_rot·head_d: 고개 · legs·arms: [먼/왼, 가까운/오른] 끝 자리 (target 형식) · leg_bend·arm_bend: 무릎·팔꿈치 굽는 쪽 ·
    held: 몸과 팔 사이 Part 함수 · fx: 효과 목록 또는 손 자리 [먼/왼, 가까운/오른]를 받아 효과를 돌려주는 함수 · grip: mark_grip할 손 ("near"·"far"·"left"·"right") · mask_up: 걷어 올린 턱의 입 모양 ·
    arms_front: 팔을 머리 위에 그림 · fingers: 손가락 편 방향 (줄 쏘는 손) · far_limbs_front: 먼 팔다리도 몸 앞에 (뒷모습 아님) ·
    grip_red: 먼 손이라도 줄 잡은 장갑은 밝은 빨강."""
    s = hs()
    side_view = view_ == "side"
    hx0, hy0 = hip or ((60.0 if side_view else 64.0), stand_hip_y())
    lw, aw = s["lw"], s["aw"]
    bottom = hy0 + lw * 0.35
    top = bottom - s["th"]
    with xf(**(whole or {})):
        bm = dp._xf_matrix(rot=rot, pivot=(hx0, hy0))
        P = lambda p: dp._apply(bm, p)
        if side_view:
            leg_roots = [P((hx0 - 2.0, hy0)), P((hx0 + 2.0, hy0))]
            sh_roots = [P((hx0 - 1.5, top + aw * 0.75)), P((hx0 + 1.0, top + aw * 0.8))]
            leg_sides, arm_sides = (1, 1), (1, 1)
            leg_pref = leg_bend or [(1, -0.2), (1, -0.2)]
            arm_pref = arm_bend or [(-1, 0.4), (-1, 0.4)]
        else:
            leg_roots = [P((hx0 - s["hip"], hy0)), P((hx0 + s["hip"], hy0))]
            sh_roots = [P((hx0 - (s["tw"] - aw * 0.45), top + aw * 0.6)), P((hx0 + (s["tw"] - aw * 0.45), top + aw * 0.6))]
            leg_sides, arm_sides = (-1, 1), (-1, 1)
            leg_pref = leg_bend or [(-1, 0), (1, 0)]
            arm_pref = arm_bend or [(-1, 0.3), (1, 0.3)]
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
            p, _ = limb(leg_roots[j], end, s["leg"], lw, BLUE_FAR if far else BLUE, RED_FAR if far else RED, leg_pref[j], boot=True,
                        toe=(1 if side_view else 0) if view_ != "back" else 0)
            leg_parts.append(p)
            end = target(sh_roots[j], arms[j], s["arm"], arm_sides[j])
            glove = RED_FAR if far and not grip_red else RED
            p, _ = limb(sh_roots[j], end, s["arm"], aw, BLUE_FAR if far else BLUE, glove, arm_pref[j], fingers=fingers[j])
            arm_parts.append(p)
            hands.append(end)
        # 몸통 (+ 빨강 V·거미줄·가슴 무늬)
        with xf(rot=rot, pivot=(hx0, hy0)):
            tw = s["tw"] if not side_view else s["tw"] * 0.95
            if _ST["lvl"] == 0:
                tshape = [E(hx0 - tw * 1.04, top, hx0 + tw * 1.04, bottom + 1.5)]
            else:
                tshape = [E(hx0 - tw, top, hx0 + tw, top + s["th"] * 0.78), E(hx0 - tw * 0.84, top + s["th"] * 0.32, hx0 + tw * 0.84, bottom)]
            th = s["th"]
            if view_ == "side":
                red = [E(hx0 - tw * 0.7, top - th * 0.45, hx0 + tw * 2.1, top + th * 0.82)]
                ex, ey, esx = hx0 + tw * 0.42, top + th * 0.36, 0.62
            else:
                red = [T((hx0 - tw * 1.3, top - 4), (hx0 + tw * 1.3, top - 4), (hx0 + tw * 1.3, top + th * 0.1), (hx0 + tw * 0.4, top + th * 0.6),
                         (hx0, top + th * 0.86), (hx0 - tw * 0.4, top + th * 0.6), (hx0 - tw * 1.3, top + th * 0.1))]
                ex, ey, esx = hx0, top + th * 0.36, 1.0
            marks = [(red, RED)]
            if s["body_web"]:
                if view_ == "side":
                    marks += web_lines(ex, ey, th * 0.5, 180, 330, 5, 1, 0.75)
                else:
                    marks += web_lines(ex, ey, th * 0.62, 190, 350, 7, 1, 0.75) + web_lines(ex, ey, th * 0.5, 75, 105, 2, 0, 0.75)
            emb = emblem_ops(ex, ey, 1.0, sx=esx) if view_ != "back" else emblem_ops(ex, ey + 1, 1.25)
            torso = Part(tshape, BLUE, marks + emb, outline=True, grow=0)
            neck = (hx0 + (1.5 if side_view else 0.0), top)
        neck = P(neck)
        # 머리 (빨간 가면)
        hrx, hry = s["hrx"], s["hry"]
        with xf(rot=rot + head_rot, pivot=neck):
            hx, hy = neck[0] + head_d[0] + (2.0 if side_view else 0.0), neck[1] - hry + OVERLAP + head_d[1]
            if view_ == "back":
                wmarks = web_lines(hx, hy - hry * 0.35, hrx * 1.5, 0, 360, max(8, s["webs"]), s["rings"] + 1)
            else:
                wc = (hx + (0.44 * hrx if side_view else 0.0), hy + 0.1 * hry)
                wmarks = web_lines(wc[0], wc[1], hrx * 1.5, 0, 360, s["webs"], s["rings"])
            hmarks = list(wmarks)
            if mask_up is not None and view_ != "back":
                chin_x = hx + (0.5 * hrx if side_view else 0.0)
                hmarks.append(([E(chin_x - hrx * 0.72, hy + 0.42 * hry, chin_x + hrx * 0.72, hy + 1.3 * hry)], SKIN))
                hmarks.append(([L(dp.arc(chin_x, hy + 0.48 * hry, hrx * 0.72, 2.2, 180, 360, 10), 2.4)], mix(RED, RED_WEB, 0.3)))
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)], RED, hmarks, outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.7)
            face = [] if view_ == "back" else mask_face(view_, hx, hy, eyes, brow, mouth, look, blush_on, mask_up is not None)
            if mask_up is not None and view_ != "back":
                mx = hx + (0.5 * hrx if side_view else 0.0)
                my = hy + 0.72 * hry
                if mask_up == "open":
                    face += dp.open_mouth(mx, my - 2.6, 3.4, 3.4)
                else:
                    face += [([E(mx - 2.6, my - 1.2, mx + 2.6, my + 1.4)], SKIN_LINE)]
        parts = []
        if side_view:
            parts += [arm_parts[0]] if not far_limbs_front else []
            parts += [leg_parts[0], leg_parts[1], torso]
            parts += [h() for h in held]
            parts += [head]
            parts += ([arm_parts[0]] if far_limbs_front else []) + [arm_parts[1]]
        else:
            # 뒷모습은 머리가 팔을 가림 (팔을 머리 위에 그리지 않음 — 벽 타기에서 팔이 머리를 덮던 실수)
            if view_ == "back":
                arms_front = False
            parts += leg_parts + [torso]
            if not arms_front:
                parts += arm_parts
            parts += [h() for h in held]
            parts += [head]
            if arms_front:
                parts += arm_parts
        if grip:
            idx = {"far": 0, "near": 1, "left": 0, "right": 1}[grip]
            dp.mark_grip(*hands[idx])
    if callable(fx):  # 손 자리를 알아야 하는 효과 (줄 퐁)
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


def sp_walk(sp, i):
    """총총 걷기: 두 다리를 번갈아 앞뒤로, 팔은 반대로 흔듦, 몸 통통."""
    fdx, ndx, fl, nl = WALK[i]
    k = 0.55 + 0.45 * hs()["leg"] / 22
    bob = [0, -1.2, 0, -1.2][i]
    hip = H(dy=bob)
    legs = [floor_at(SIDE_CX - 2 + fdx * k, fl), floor_at(SIDE_CX + 2 + ndx * k, nl)]
    sw = [1, 0, -1, 0][i]
    arms = [("a", 22 * sw, 0.96), ("a", -22 * sw + 4, 0.96)]
    return hero(hip=hip, legs=legs, arms=arms, rot=2)


def sp_run(sp, i):
    """다다다 달리기: 앞으로 기울고 크게 성큼, 팔을 힘차게 앞뒤로, 속도선."""
    st = [1, 0, -1, 0][i]
    hip = H(dy=[-1.5, -3.5, -1.5, -3.5][i])
    legs = [("a", -40 * st, 0.95 if st else 0.75), ("a", 40 * st, 0.95 if st else 0.75)]
    if st == 0:
        legs = [("a", -25, 0.8), ("a", 35, 0.7)] if i == 1 else [("a", 35, 0.7), ("a", -25, 0.8)]
    arms = [("a", 55 * st + 5, 0.8), ("a", -45 * st + 5, 0.8)]
    return hero(hip=hip, legs=legs, arms=arms, rot=12, mouth="open", arm_bend=[(1, 0.3), (1, 0.3)],
                fx=dp.speed_fx(4, (70, 86, 102), 12, [0, -3, 0, -3][i]))


def sp_idle(sp, i):
    """제자리 숨쉬기: 가슴이 살짝 오르내림, 손은 허리께."""
    return hero(hip=H(dy=[0, 0.8][i]), arms=[("a", 12, 0.97), ("a", 8 + 3 * i, 0.97)], head_d=(0, [0, 0.6][i]))


def sp_look(sp, i):
    """두리번: 위 → 앞 → 아래 → 앞 (고개만)."""
    rot, eyes, look = [(-8, "up", (0.5, -1.0)), (0, "dot", (1.2, 0)), (8, "dot", (0.6, 1.5)), (0, "closed", (0, 0))][i]
    return hero(head_rot=rot, eyes=eyes, look=look)


def sit_hip(x=SIDE_CX):
    """바닥에 앉은 엉덩이 관절."""
    return (x, FLOOR - hs()["lw"] * 0.75)


def sp_sleep(sp, i):
    """웅크려 쿨쿨: 바닥에 앉아 무릎을 끌어안고 고개를 무릎에 묻음, 렌즈 ‿, z."""
    s = hs()
    hip = sit_hip(SIDE_CX - 6)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 3, foot_y()), (knee_x + 5, foot_y())]
    arms = [(knee_x + 1, hip[1] - s["leg"] * 0.45 - 4), (knee_x + 3, hip[1] - s["leg"] * 0.45 - 2)]
    fx = [z_fx(106, 34 - 3 * i, 4.0), z_fx(116, 20 - 3 * i, 2.6)]
    return hero(hip=hip, rot=14 + i, head_rot=10, head_d=(2, 1 + 0.6 * i), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms,
                arm_bend=[(0, 1), (0, 1)], eyes="closed", fx=fx, blush_on=True)


def sp_annoyed(sp, i):
    """털썩 앉아 팔짱 끼고 렌즈 반쯤 감은 뾰로통 💢."""
    s = hs()
    hip = sit_hip(SIDE_CX - 4)
    legs = [(hip[0] + s["leg"] + 4, foot_y()), (hip[0] + s["leg"] + 7, foot_y())]
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms = [(hip[0] + 8, top + s["th"] * 0.45), (hip[0] + 9, top + s["th"] * 0.5)]
    return hero(hip=hip, rot=-6, legs=legs, arms=arms, arm_bend=[(1, 0.8), (1, 0.8)], eyes="flat", mouth="pout", head_rot=-4,
                fx=[anger_fx(102 + i, 30 - i, 5.0 + 0.6 * i)])


def sp_hop(sp, i):
    """폴짝 (한 번): 웅크림 → 두 팔 번쩍 솟음 (다리 모음) → 다리 내림."""
    s = hs()
    if i == 0:
        hip = H(dy=s["leg"] * 0.32)
        return hero(hip=hip, rot=12, legs=[floor_at(SIDE_CX - 3), floor_at(SIDE_CX + 3)], arms=[("a", -30, 0.9), ("a", -25, 0.9)],
                    eyes="tight", mouth="open")
    hip = (SIDE_CX, [0, 84, 90][i] - (s["leg"] - 22) * 0.5)
    legs = [("a", -30, 0.7), ("a", 25, 0.62)] if i == 1 else [("a", -8, 0.92), ("a", 10, 0.9)]
    arms = [("a", 160, 0.95), ("a", 150, 0.95)] if i == 1 else [("a", 110, 0.95), ("a", 100, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="open", arm_bend=[(1, 0), (1, 0)])


def sp_fall(sp, i):
    """떨어짐: 팔다리를 허우적, 동그란 렌즈."""
    w = [0, 1][i]
    hip = (SIDE_CX, 84.0)
    legs = [("a", -40 - 15 * w, 0.85), ("a", 40 + 15 * w, 0.75)]
    arms = [("a", 150 - 25 * w, 0.95), ("a", 125 + 25 * w, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="round", mouth="open", rot=[-6, 6][i], arm_bend=[(0, -1), (0, -1)])


def sp_dragged(sp, i):
    """목덜미가 잡혀 대롱대롱: 팔다리가 축 늘어지고 살랑."""
    s = hs()
    sway = [-6, 6][i]
    top_y = 22.0
    hip_y = top_y + 2 * s["hry"] - OVERLAP + s["th"] - s["lw"] * 0.35 - 2
    hip = (SIDE_CX + 2, hip_y)
    legs = [("a", -6, 0.97), ("a", 8, 0.95)]
    arms = [("a", 12, 0.95), ("a", 18, 0.95)]
    return hero(hip=hip, legs=legs, arms=arms, eyes="round", mouth="pout", whole=dict(rot=sway * 0.5, pivot=(SIDE_CX, 14)),
                fx=sweat_fx(104, 30))


def sp_land(sp, i):
    """착지 (한 번): 무릎을 굽혀 쿵 → 바로 섬."""
    s = hs()
    if i == 0:
        return hero(hip=H(dy=s["leg"] * 0.3), rot=10, legs=[floor_at(SIDE_CX - 4), floor_at(SIDE_CX + 4)], arms=[("a", 60, 0.9), ("a", 50, 0.9)],
                    eyes="tight", mouth="open")
    return hero(eyes="happy")


def sp_lookup(sp, i):
    """올려다보기: 고개를 젖혀 렌즈가 위로, 반짝."""
    return hero(head_rot=dp.lookup_tilt(i), eyes="up", look=(0.6, -1.0), fx=dp.lookup_fx(i, 110, 14))


# ---- 🕷️ 특기 동작 ----
def thwip_fx(x, y, k=1.0):
    """손목에서 퐁 터지는 줄 덩어리 (하양) + 짧은 선."""
    return [([C(x + 2.5 * k, y, 3.0 * k), C(x + 5.6 * k, y - 2.2 * k, 2.0 * k), C(x + 5.6 * k, y + 2.2 * k, 2.0 * k)], THREAD, 1.0),
            ([L([(x + 9 * k, y - 4.5 * k), (x + 12 * k, y - 6.5 * k)], 1.2), L([(x + 10 * k, y), (x + 13.5 * k, y)], 1.2),
              L([(x + 9 * k, y + 4.5 * k), (x + 12 * k, y + 6.5 * k)], 1.2)], (120, 130, 160), 0)]


def sp_shoot(sp, i):
    """줄 쏘기 (한 번, 마지막 칸 유지): 팔을 당겨 겨눔 → 몸을 살짝 젖히며 팔을 쭉 뻗어 손목 까딱 (손가락 둘) + 줄 퐁 → 뻗은 채."""
    s = hs()
    cx = SIDE_CX - 6
    hip = H(cx)
    legs = [floor_at(cx - 8), floor_at(cx + 8)]
    if i == 0:
        near, fing, fx, rot = ("a", 70, 0.55), None, (), 4
    else:
        near, fing, rot = ("a", 88, 1.3), 120, -8
        k = 1.0 if i == 1 else 0.7
        fx = lambda hands: thwip_fx(hands[1][0] + s["aw"] * 0.6, hands[1][1] - 2, k)  # noqa: E731
    return hero(hip=hip, rot=rot, head_rot=-rot * 0.5, legs=legs, leg_bend=[(1, -0.2), (1, -0.2)], arms=[("a", -25, 0.9), near],
                arm_bend=[(-1, 0.4), (0, 1)], fingers=(None, fing), brow="angry", mouth=None if i == 0 else "grin", grip="near", fx=fx)


def sp_web_swing(sp, i):
    """줄 그네: 가까운 팔을 머리 위로 쭉 들어 줄을 잡고, 먼 팔은 앞으로, 다리는 모아 뒤로 ↔ 앞으로 흔듦."""
    s = hs()
    cx = SIDE_CX - 4
    top_hand = 18.0
    hip_y = top_hand + 2 * s["hry"] - OVERLAP + s["th"] - s["lw"] * 0.35 + 2
    hip = (cx, hip_y)
    hand = (cx + 4, top_hand)
    legs = [("a", -35, 0.72), ("a", -20, 0.78)] if i == 0 else [("a", 30, 0.72), ("a", 45, 0.78)]
    return hero(hip=hip, rot=[6, -4][i], head_rot=[-14, -8][i], head_d=(-3, 0), legs=legs, leg_bend=[(1, 0), (1, 0)],
                arms=[hand, ("a", 70, 0.9)], arm_bend=[(1, 0), (1, 0.3)], eyes="happy", mouth="big", grip="far", grip_red=True,
                fx=dp.speed_fx(6, (70, 86), 10, [0, -3][i]))


def dust_fx(x, y, s=1.0):
    """먼지 퐁퐁 (작은 구름 셋)."""
    blobs = [C(x - 4 * s, y - 2 * s, 3.2 * s), C(x + 1 * s, y - 4 * s, 3.8 * s), C(x + 5.5 * s, y - 1.6 * s, 2.8 * s)]
    inner = [C(x - 4 * s, y - 2 * s, 2.2 * s), C(x + 1 * s, y - 4 * s, 2.8 * s), C(x + 5.5 * s, y - 1.6 * s, 1.8 * s)]
    return [(blobs, DUST_LINE, 1.0), (inner, DUST, 0)]


def sp_hero_land(sp, i):
    """영웅 착지 (한 번): 쪼그려 앞 주먹을 바닥에 쿵, 뒤 다리를 뒤로 쭉, 다른 팔은 뒤로 활짝 — 먼지 → 버팀 → 반짝."""
    s = hs()
    cx = SIDE_CX + 2
    hip = (cx, FLOOR - s["lw"] * 0.5 - s["leg"] * 0.62)
    fist = (cx + s["arm"] + 14, FLOOR - s["aw"] * 0.66)
    back = floor_at(cx - s["leg"] - 8)
    near_foot = floor_at(cx + s["leg"] * 0.55)
    fx = []
    if i == 0:
        fx = dust_fx(fist[0] + 4, FLOOR - 1, 1.1) + dust_fx(back[0] - 2, FLOOR - 1, 0.9)
    elif i == 1:
        fx = dust_fx(fist[0] + 6, FLOOR - 3, 0.7)
    else:
        fx = sparkle_fx(cx + 34, 30, 4.0)
    return hero(hip=hip, rot=28, head_rot=-24, head_d=(2, 0), legs=[back, near_foot], leg_bend=[(0, -1), (1, -1)],
                arms=[("a", -130, 0.95), fist], arm_bend=[(0, -1), (-1, 0.6)], eyes="dot", brow="angry", fx=fx)


def sp_web_pull(sp, i):
    """줄 끌어당기기: 두 손으로 앞의 줄을 잡고 몸을 뒤로 젖혀 영차 (손이 당겨졌다 다시 뻗음)."""
    s = hs()
    cx = SIDE_CX - 8
    pull = [0, 4, 7, 4][i]
    hip = H(cx, s["leg"] * 0.2)
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    hy = top + s["th"] * 0.5
    near = (cx + s["arm"] + 14 - pull, hy - 2 + pull * 0.2)
    far = (near[0] - 5, near[1] + 1.5)
    legs = [floor_at(cx - 6), floor_at(cx + 14)]
    fx = sweat_fx(cx - 10, top - 2 * s["hry"]) if i == 2 else []
    return hero(hip=hip, rot=-12 - pull * 0.8, legs=legs, leg_bend=[(1, -0.2), (1, -0.2)], arms=[far, near], arm_bend=[(0, 1), (0, 1)],
                far_limbs_front=True, eyes="tight" if i == 2 else "dot", brow=None if i == 2 else "angry", mouth="grin", grip="near", fx=fx)


WHIRL_HAND = [(3.0, 0.0), (0.0, -2.0), (-3.0, 0.0)]  # 🌀 칸마다 머리 위 손의 작은 원 (손목으로 돌림)


def whirl_fx(x, y, i, k=1.0):
    """🌀 손목 둘레에서 빙빙 도는 바람 자국 (회색 납작한 호 둘 — 칸마다 돎, 장갑보다 위로 솟지 않게). 줄 자체는 앱이 그림."""
    a = 120 * i
    grey = (150, 160, 190)
    return [([L(dp.arc(x, y - 1 * k, 13 * k, 3.5 * k, a + 200, a + 290, 8), 1.3), L(dp.arc(x, y - 1 * k, 13 * k, 3.5 * k, a + 20, a + 110, 8), 1.3)],
             grey, 0)]


def sp_web_whirl(sp, i):
    """🌀 줄 휘두르기: 다리를 벌려 버티고 서서 가까운 팔을 머리 뒤쪽 위로 쭉 들어 (얼굴은 가리지 않게) 손목으로 줄을 빙빙
    (손이 작은 원을 그리고 손 위에 바람 자국), 먼 팔은 주먹 쥐고 앞으로, 신난 얼굴."""
    s = hs()
    cx = SIDE_CX - 2
    hip = H(cx, s["leg"] * 0.32)  # 무릎을 굽혀 버팀
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    head_top = top - 2 * s["hry"] + OVERLAP
    dx, dy = WHIRL_HAND[i]
    hand = (cx - 1.0 * s["hrx"] + dx, head_top - 2 + dy)
    legs = [floor_at(cx - 10), floor_at(cx + 11)]
    return hero(hip=hip, rot=[-5, -3, -5][i], head_rot=-6, legs=legs, leg_bend=[(1, -0.2), (1, -0.2)],
                arms=[("a", 75, 0.8), hand], arm_bend=[(1, 0.4), (-1, 0)], eyes="happy", mouth="big", grip="near",
                fx=lambda hands: whirl_fx(hands[1][0], hands[1][1], i))


HANG_FLIP = dict(sy=-1, pivot=(64.0, 64.0))  # 🙃 거꾸로: 똑바로 웅크린 그림을 위아래로 뒤집음 (머리가 아래, 손발이 위)
HANG_SWAY = [(0.0, 0.0), (1.2, 2.5), (-1.0, -2.0)]  # 🙃 칸마다 (몸 기울기 더함, 손발 아래 몸 흔들림 dx) — 살랑살랑 숨쉬기


def sp_hang_down(sp, i):
    """🙃 거꾸로 매달리기: 쪼그려 앉듯 무릎·팔꿈치를 굽혀 몸을 웅크린 채 위아래로 뒤집혀, 위로 뻗은 줄 한 가닥을 두 발과 두 손으로 모두 꼭 붙잡음.
    똑바로 쪼그린 그림 (몸을 앞으로 숙이고 발 앞의 줄을 두 손이 발 바로 위에서 잡음)을 그린 뒤 HANG_FLIP으로 뒤집음 —
    뒤집으면 발이 맨 위 (줄이 붙는 점 = mark_grip), 그 아래 두 손, 굽힌 무릎·팔꿈치, 몸, 맨 아래 머리. 줄은 손발 사이만 그리고 위쪽은 앱이 닻까지 그림."""
    s = hs()
    lean, dx = HANG_SWAY[i]
    cx = SIDE_CX - 8
    hip_y = 4.0 + 2 * s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]  # 뒤집으면 머리 아래 끝이 바닥께
    hip = (cx, hip_y)
    lx = cx + s["tw"] * 0.9 + s["leg"] * 0.25 + dx  # 줄 x (몸 앞)
    far_foot = (lx - s["lw"] * 0.1, hip_y + s["leg"] * 0.55 + s["lw"] * 0.2)
    near_foot = (lx + s["lw"] * 0.1, far_foot[1] + s["lw"] * 0.85)  # 뒤집으면 맨 위 (밝은 빨강 장화)
    near_hand = (lx, hip_y + s["aw"] * 0.1)
    far_hand = (lx, near_hand[1] - s["aw"] * 1.3)
    grip = (lx, near_foot[1] + s["lw"] * 0.3)  # 뒤집으면 맨 위 장화 안쪽 — 줄이 붙는 점
    with xf(**HANG_FLIP):
        fx = [([L([grip, (lx, far_hand[1] - s["aw"] * 0.5)], 1.7)], OUT, 0), ([L([grip, (lx, far_hand[1] - s["aw"] * 0.5)], 0.9)], THREAD, 0)]
        dp.mark_grip(*grip)
    return hero(hip=hip, rot=6 + lean, head_rot=-8, legs=[far_foot, near_foot], leg_bend=[(1, -0.2), (1, -0.2)],
                arms=[far_hand, near_hand], arm_bend=[(1, 0.5), (1, 0.5)], grip_red=True,
                eyes="happy" if i == 1 else "dot", mouth="grin", whole=HANG_FLIP, fx=fx)


# =====================================================================
# 앞모습
# =====================================================================
def front(**kw):
    """앞모습 꼬마 영웅."""
    return hero(view_="front", **kw)


def F(dy=0.0):
    """앞모습 엉덩이 관절."""
    return (64.0, stand_hip_y() + dy)


def sp_sit_front(sp, i):
    """정면 서기: 허리에 손 얹은 영웅 자세 (마지막 장 깜빡)."""
    s = hs()
    top = stand_hip_y() + s["lw"] * 0.35 - s["th"]
    wy = top + s["th"] * 0.72
    arms = [(64 - s["tw"] * 0.88, wy), (64 + s["tw"] * 0.88, wy)]
    return front(arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="closed" if i == 3 else "dot", head_d=(0, [0, 0.5, 0, 0][i]))


def sp_wave(sp, i):
    """한 손을 번쩍 들어 흔들흔들 (다른 손은 허리)."""
    a = [118, 140, 118, 140][i]
    return front(arms=[("a", 18, 0.95), ("a", a, 1.0)], arm_bend=[(-1, 0.3), (1, 0)], eyes="happy" if i % 2 else "dot",
                 mouth="open", head_rot=[-3, -1, -3, -1][i])


def sp_tilt(sp, i):
    """고개 갸웃 + ? (한 손은 턱)."""
    s = hs()
    top = stand_hip_y() + s["lw"] * 0.35 - s["th"]
    return front(head_rot=[8, 12, 12][i], arms=[("a", 15, 0.95), (64 + 6, top + 2)], arm_bend=[(-1, 0.3), (1, 0.6)], arms_front=True,
                 fx=[question_fx(106, 24 - i)], mouth="pout")


def sp_belly(sp, i):
    """벌러덩: 엉덩방아 찧고 두 다리를 위로 버둥, 두 팔 번쩍, 깔깔."""
    s = hs()
    w = [0, 2, 0, -2][i]
    hip = (64.0, FLOOR - s["lw"] * 0.8)
    legs = [("a", 112 + 6 * w, 0.9), ("a", 112 - 6 * w, 0.9)]
    arms = [("a", 128 - 5 * w, 0.95), ("a", 128 + 5 * w, 0.95)]
    return front(hip=hip, legs=legs, arms=arms, eyes="happy", mouth="big")


def sp_stretch(sp, i):
    """기지개 (한 번): 두 팔 쭉 위로 → 하품하며 팔 벌림 → 서기."""
    if i == 0:
        return front(arms=[("a", 140, 1.0), ("a", 140, 1.0)], eyes="tight", hip=F(-1.5))
    if i == 1:
        return front(arms=[("a", 100, 1.0), ("a", 100, 1.0)], eyes="closed", mouth="big")
    return front(eyes="happy")


def sp_heart(sp, i):
    """하트를 가슴에 꼭 안고 렌즈 ^^, 작은 하트가 떠오름."""
    s = hs()
    top = stand_hip_y() + s["lw"] * 0.35 - s["th"]
    hy = top + s["th"] * 0.45

    def held():
        """가슴에 안은 하트."""
        return dp.held_heart(hy - 4)
    return front(arms=[(58, hy + 3), (70, hy + 3)], arm_bend=[(-1, 0.6), (1, 0.6)], held=[held], eyes="happy", fx=dp.heart_burst(i))


def sp_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 배를 감싸고 깔깔 (> <)."""
    s = hs()
    top = stand_hip_y() + s["lw"] * 0.35 - s["th"]
    y = top + s["th"] * 0.6
    return front(rot=[-7, 7, -3][i], arms=[(60, y), (68, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="tight", mouth="big")


def sp_hungry(sp, i):
    """배고파 시무룩: 두 손으로 배를 감싸고 처진 눈꺼풀, 파리 생각 풍선."""
    import draw_frog
    s = hs()
    top = stand_hip_y() + s["lw"] * 0.35 - s["th"]
    y = top + s["th"] * 0.68
    fx = draw_frog.fly_bubble_fx(104 - 2 * i, 20, 12.0)
    return front(hip=F([0, 1, 0][i]), arms=[(59, y), (69, y)], arm_bend=[(-1, 0.5), (1, 0.5)], eyes="dot", brow="sad", mouth="frown",
                 head_rot=-3, fx=fx)


EAT_MOUTH = 79.0  # 먹을 때 입 (프레임 y) — 그릇 밥 더미 바로 위


def sp_eat(sp, i):
    """냠냠 (그릇 없음): 무릎 꿇고 가면 아래를 걷어 올려 입을 밥 더미 바로 위에 — 앙 ↔ 오물오물, 부스러기."""
    s = hs()
    # 입 = 머리 중심 + 0.72 hry, 머리 중심 = 몸 위 - hry + OVERLAP, 몸 위 = 엉덩이 + 0.35 lw - th
    hip_y = EAT_MOUTH + [0, 1, 0, 1][i] - 0.72 * s["hry"] + s["hry"] - OVERLAP + s["th"] - 0.35 * s["lw"]
    hip_y = min(hip_y, FLOOR - s["lw"] * 0.8)
    legs = [(64 - s["hip"] - 3, foot_y()), (64 + s["hip"] + 3, foot_y())]
    arms = [(44, 96), (84, 96)]
    return front(hip=(64.0, hip_y), legs=legs, arms=arms, arm_bend=[(-1, -0.2), (1, -0.2)], eyes="happy" if i % 2 else "dot",
                 mask_up="open" if i % 2 == 0 else "chew", fx=dp.eat_crumbs(i))


def sp_level_up(sp, i):
    """만세: 두 팔 번쩍, ^^, 반짝이 (가운데 칸은 살짝 뜀)."""
    up = [114, 124, 118][i]
    return front(hip=F(-[0, 3, 1][i]), legs=None if i != 1 else [("a", -6, 0.97), ("a", 6, 0.97)], arms=[("a", up, 1.0), ("a", up, 1.0)],
                 eyes="happy", mouth="big", fx=dp.level_up_stars(i))


# =====================================================================
# 구르기 (회전 원 안) — 공처럼 웅크림
# =====================================================================
def sp_roll(sp, i):
    """무릎을 끌어안고 동그랗게 웅크린 공: 회전 중심 (64, 88), 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    s = hs()
    with view(scale=1.0, lift=0):
        with xf(rot=[-8, 8][i], pivot=(cx, cy)):
            hrx, hry = 21.0, 19.5
            hx, hy = cx + 5, cy - 8
            body = Part([C(cx - 6, cy + 8, 19.0)], BLUE, [([E(cx - 26, cy - 14, cx + 6, cy + 6)], RED)] + emblem_ops(cx - 12, cy - 1, 0.8),
                        outline=True, grow=0)
            knee = Part([E(cx + 2, cy + 6, cx + 30, cy + 26), E(cx - 4, cy + 18, cx + 14, cy + 30)], BLUE,
                        [([E(cx - 6, cy + 18, cx + 14, cy + 31)], RED)], outline=True, grow=0)
            head = Part([E(hx - hrx, hy - hry, hx + hrx, hy + hry)], RED,
                        web_lines(hx + 4, hy + 2, hrx * 1.5, 0, 360, s["webs"], min(1, s["rings"])), outline=True, grow=0)
            glove = Part([C(cx + 22, cy + 16, 5.6)], RED, outline=True, grow=0)
            dp.mark_head(hx, hy - hry + 2, hrx * 1.6)
            face = []
            for side, lx in ((-1, hx - 4), (1, hx + 11)):
                face += lens_ops("happy", lx, hy + 1, side, sx=0.8)
            face += mouth_ops("open" if i else None, hx + 8, hy + 13)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([body, knee, head, glove], face, clip=clip)


# =====================================================================
# 뒷모습 (climb·slide) — 화면 벽에 손발을 붙이고 엉금엉금
# =====================================================================
CLIMB = [(1.0, 0.2), (0.6, 0.6), (0.2, 1.0), (0.6, 0.6)]  # (왼손, 오른손) 올린 몫 — 발은 엇갈려


def spider_back(reach, *, dy=0.0, scratch=False, sweat=False):
    """뒷모습: 두 손을 머리 옆·위로 벽에 짚고 (reach: 0 어깨 옆 … 1 머리 위), 두 다리는 개구리처럼 무릎을 벌려 벽에 디딤."""
    s = hs()
    hip = (64.0, 92.0 - (s["leg"] - 22) * 0.3 + dy)
    top = hip[1] + s["lw"] * 0.35 - s["th"]
    arms, legs = [], []
    for side, r in zip((-1, 1), reach):
        arms.append((64 + side * (s["hrx"] + 4 - 3 * r), top - s["arm"] * (0.15 + 0.75 * r)))
        lift = 1.0 - r  # 손이 내려간 쪽 발이 올라옴 (엇갈림)
        legs.append((64 + side * (s["hip"] + s["leg"] * 0.62), hip[1] + s["leg"] * (0.75 - 0.45 * lift)))
    fx = []
    if scratch:
        for x, y in arms:
            fx.append(([L([(x + d, y - 12), (x + d * 1.2, y - 4)], 1.2) for d in (-3, 0, 3)], (150, 150, 170), 0))
    if sweat:
        fx += sweat_fx(64 + s["hrx"] + 6, top - s["hry"] * 1.2)
    return hero(view_="back", hip=hip, legs=legs, leg_bend=[(-1, -1), (1, -1)], arms=arms, arm_bend=[(-1, 0.6), (1, 0.6)], fx=fx)


def sp_climb(sp, i):
    """뒷모습으로 기어오름: 왼손·오른손이 번갈아 위로, 발은 엇갈려 디딤, 몸 살짝 오르내림."""
    return spider_back(CLIMB[i], dy=[0, -1, 0, -1][i])


def sp_slide(sp, i):
    """뒷모습으로 주르륵: 두 손 쭉 위로, 긁힌 자국 3줄, 땀 한 방울."""
    return spider_back((1.0, 1.0), dy=[0, 2][i], scratch=True, sweat=True)


SPIDER_POSES = {"walk": sp_walk, "run": sp_run, "roll": sp_roll, "idle": sp_idle, "look": sp_look, "sleep": sp_sleep, "hop": sp_hop,
                "fall": sp_fall, "dragged": sp_dragged, "land": sp_land, "sit_front": sp_sit_front, "wave": sp_wave, "tilt": sp_tilt,
                "belly": sp_belly, "stretch": sp_stretch, "heart": sp_heart, "tickle": sp_tickle, "annoyed": sp_annoyed,
                "hungry": sp_hungry, "eat": sp_eat, "level_up": sp_level_up, "climb": sp_climb, "slide": sp_slide, "lookup": sp_lookup,
                "shoot": sp_shoot, "web_swing": sp_web_swing, "hero_land": sp_hero_land, "web_pull": sp_web_pull,
                "web_whirl": sp_web_whirl, "hang_down": sp_hang_down}
SPIDER_POSES = {k: (v if k == "roll" else dp._fit(v)) for k, v in SPIDER_POSES.items()}  # roll은 회전 원 안 그대로
WEB_ANIMS = ["shoot", "web_swing", "hero_land", "web_pull", "web_whirl", "hang_down"]  # = Rust manifest::WEB_ANIMS

SPIDER = Species(
    id="spider", name="거미인간", walk_speed=95, scale=1.0, base=BLUE, back=RED, belly=RED, nose=OUT, mouth="w",
    front_tail_color=BLUE, tail_color=BLUE, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + WEB_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=RED_WEB, poses=SPIDER_POSES, outline=OUT,
    # 색은 파스텔 없이 또렷한 빨강·파랑 · 크기는 HERO가 정함 (fit 1)
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 거미인간을 등록함."""
    dp.SPECIES[SPIDER.id] = SPIDER


register()


def preview_spider(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hop", 1), ("fall", 0), ("roll", 0),
             ("shoot", 2), ("web_swing", 0), ("hero_land", 1), ("web_pull", 2), ("web_whirl", 1), ("hang_down", 0), ("climb", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "hero_land"), bg=bg)


def preview_all(by_stage, path, st="teenager"):
    """한 단계의 모든 동작·칸 (눈으로 확인용)."""
    from PIL import Image
    anims = by_stage[st]
    im = Image.new("RGBA", (4 * 132, len(anims) * 132), (255, 255, 255, 255))
    for r, (a, frames) in enumerate(anims.items()):
        for c, f in enumerate(frames):
            im.alpha_composite(f, (c * 132, r * 132))
    im.save(path)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(SPIDER, root / "ui" / "pets" / SPIDER.id))
    preview_spider(by_stage, pv / "stages_spider.png")
    dp.preview_back(by_stage, pv / "climb_spider.png")
    if "--all" in sys.argv:
        for st in ("baby", "teenager", "adult"):
            preview_all(by_stage, pv / f"all_spider_{st}.png", st)
    print("ok: spider")


if __name__ == "__main__":
    main()
