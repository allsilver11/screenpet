"""흐물이 🤸 — 선으로 그린 얼굴 없는 꼬마 사람(heumul). draw_pets 엔진(슈퍼샘플 마스크·외곽선)을 그대로 쓴다.

    python tools/draw_heumul.py → ui/pets/heumul/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_heumul.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

흐물이: 나뭇가지처럼 가는 선으로 된 꼬마 사람. 동그란 머리(크림색, 눈·코·입 없음) + 한 줄 몸 + 팔다리 넷 + 동글 손발.
        몸과 팔다리는 곧은 막대가 아니라 고무호스처럼 휘는 곡선 (3차 베지어) — 칸마다 휘는 쪽·정도가 달라 흐물흐물 말랑하게 움직인다.
        선은 모두 같은 짙은 잉크색, 같은 굵기 (동글: 선 + 실루엣 외곽선 / 도트: 1칸 선 — HM_PIXEL).
단계: 아기 = 머리가 크고 팔다리가 짧다 (정수리에 꼬불 머리카락 한 가닥) → 유아 → 어린이 → 청소년 → 어른은 키가 크고 팔다리가 길쭉.
놀람 (panic, 옆모습 4칸 12fps 반복 — 코어 State::Panic): 친구 동물과 마주치면 반쯤은 깜짝 놀라 등을 돌리고
      두 팔을 머리 위로 번쩍 (만세) 든 채 다리를 마구 휘저으며 달아난다 — 땀방울과 속도선.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다): 앞모습으로 허리를 굽혀 머리를 그릇 밥 더미(프레임 (64, 86))에 콕 → 들고 냠.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_halloween as dh  # noqa: E402
from draw_pets import C, L, Part, Species, _ST, _fit  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

FLOOR = 125.0
INK = (58, 54, 76)  # 선 (짙은 남보라 잉크)
HEAD = (255, 249, 238)  # 머리 속 (크림)
W = 0.7  # 동글 선 굵기 (실루엣 외곽선이 양쪽에 더해진다)
HEAD_RING = 1.9  # 머리 테 (안쪽 잉크) — 실루엣 외곽선과 합쳐 팔다리 선 굵기쯤
# 도트 그림 (draw_pixel이 {"on": True}로 바꿔 끼운다): 팔다리는 실루엣이 아니라 1칸 선으로 찍는다
HM_PIXEL = {"on": False}
PX_INK = tuple(int(c * 0.7) for c in INK)  # draw_pixel._ink와 같은 진하기 (머리 외곽선과 같은 색)

# 단계(lvl)별 — hr: 머리 반지름 · spine: 몸 길이 · leg·arm: 팔다리 길이 · curl: 정수리 꼬불 머리카락
HM_STAGE = {
    0: dict(hr=16.5, spine=17.0, leg=20.0, arm=20.0, curl=True),
    1: dict(hr=16.0, spine=21.0, leg=25.0, arm=24.0, curl=True),
    2: dict(hr=15.5, spine=24.0, leg=29.0, arm=27.0, curl=False),
    3: dict(hr=15.0, spine=27.0, leg=33.0, arm=30.0, curl=False),
    4: dict(hr=14.5, spine=30.0, leg=39.0, arm=34.0, curl=False),
}


def hm():
    return HM_STAGE[_ST["lvl"]]


# ---------------------------------------------------------------- 곡선
def lerp(a, b, t):
    return (a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t)


def cubic(p0, c1, c2, p1, n=16):
    out = []
    for i in range(n + 1):
        t = i / n
        u = 1 - t
        a, b, c, d = u ** 3, 3 * u * u * t, 3 * u * t * t, t ** 3
        out.append((a * p0[0] + b * c1[0] + c * c2[0] + d * p1[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p1[1]))
    return out


def hose(p0, p1, b1, b2=None):
    """p0 → p1 고무호스 곡선. b1·b2: 1/3·2/3 자리를 옆으로 미는 정도 (현 길이 배, + = 진행 방향의 왼쪽 —
    아래로 뻗은 팔다리면 앞(+x)). b2가 없으면 b1 (활 모양), 부호가 다르면 S자로 꿈틀."""
    b2 = b1 if b2 is None else b2
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    ln = math.hypot(dx, dy) or 1.0
    nx, ny = dy / ln, -dx / ln
    c1, c2 = lerp(p0, p1, 1 / 3), lerp(p0, p1, 2 / 3)
    return cubic(p0, (c1[0] + nx * b1 * ln, c1[1] + ny * b1 * ln), (c2[0] + nx * b2 * ln, c2[1] + ny * b2 * ln), p1)


def polar(p, ang, ln):
    """p에서 ang(도: 0 = 아래, 90 = 앞(+x), -90 = 뒤, 180 = 위)으로 ln."""
    a = math.radians(ang)
    return (p[0] + ln * math.sin(a), p[1] + ln * math.cos(a))


# ---------------------------------------------------------------- 한 장
class Fig:
    """뒤 → 앞 순서로 쌓는 선·손발·머리. 동글은 잉크 부품(실루엣 + 외곽선), 도트는 1칸 선."""

    def __init__(self):
        self.items = []
        self.head_at = None

    def line(self, pts):
        self.items.append(("line", pts))

    def limb(self, p0, p1, b1, b2=None, end=None):
        """팔다리 한 짝: 곡선 + 끝에 손(end="hand") / 발(end=±1: 발끝 방향)."""
        pts = hose(p0, p1, b1, b2)
        self.items.append(("line", pts))
        if end == "hand":
            self.items.append(("dot", (p1, 2.1)))
        elif end is not None:
            self.foot(p1, end)
        return pts

    def foot(self, p, toward):
        """동글 발 (발끝이 toward 쪽으로 조금) — 옆모습은 ±1, 앞모습은 ±0.5."""
        self.items.append(("foot", (p, (p[0] + 4.2 * toward, p[1]))))

    def head(self, cx, cy, tilt=0.0, mark=True):
        r = hm()["hr"]
        self.items.append(("head", (cx, cy, r)))
        if hm()["curl"]:
            top = polar((cx, cy), 180 + tilt, r - 1)
            a = 180 + tilt
            p1 = polar(top, a - 18, 5.5)
            p2 = polar(p1, a - 80, 4.5)
            p3 = polar(p2, a - 170, 3.0)
            self.items.append(("line", cubic(top, p1, p2, p3, 10)))
        if mark:
            dp.mark_head(cx, cy - r * 0.84, r * 1.25, tilt)
        self.head_at = (cx, cy, r)

    def render(self, fx=(), held=()):
        pixel = HM_PIXEL["on"]
        parts = [Part([], INK, grow=0, sharp=True)]
        ops = []
        for kind, data in self.items:
            if kind == "head":
                cx, cy, r = data
                marks = [] if pixel else [([C(cx, cy, r - HEAD_RING)], HEAD)]
                parts.append(Part([C(cx, cy, r)], INK if not pixel else HEAD, marks, grow=0, sharp=True))
                if pixel:
                    ops.append(([C(cx, cy, r - 0.6)], HEAD))  # 뒤에 그린 팔다리를 머리가 가린다
            elif kind == "line":
                if pixel:
                    parts.append(Part([L(data, 0.3)], INK, grow=0, sharp=True))  # 칸 맞춤(테두리 상자)에만
                    ops.append(([L(data, W)], PX_INK))
                else:
                    parts.append(Part([L(data, W)], INK, grow=0, sharp=True))
            elif kind == "dot":
                (x, y), r = data
                if pixel:
                    parts.append(Part([L([(x, y), (x, y)], 0.3)], INK, grow=0, sharp=True))
                    ops.append(([L([(x, y), (x, y)], W)], PX_INK))
                else:
                    parts.append(Part([C(x, y, r)], INK, grow=0, sharp=True))
            elif kind == "foot":
                a, b = data
                if pixel:
                    parts.append(Part([L([a, b], 0.3)], INK, grow=0, sharp=True))
                    ops.append(([L([a, b], W)], PX_INK))
                else:
                    parts.append(Part([L([a, b], 2.6)], INK, grow=0, sharp=True))
        parts += [h() for h in held]
        return dp.compose(parts, ops, list(fx))


def stand(f, *, hip, lean=0.0, sbend=0.0, sbend2=None, head_tilt=0.0, arms=(), legs=(), side=True, front_arms=None, head_mark=True):
    """몸 한 줄 + 머리 + 팔다리. hip: 엉덩이 · lean: 몸 기울기 (도, + = 앞) · sbend·sbend2: 몸 휨 (S자 가능).
    arms: [팔] 뒤 → 앞 (옆모습: 먼 팔, 가까운 팔 · 앞모습: 왼팔, 오른팔) — 팔 하나 = (손 각도 | 손 자리, 길이 배, 나가는 각도, 꿈틀).
    legs: [(발 자리, 휨1, 휨2)] 뒤 → 앞. front_arms: 머리보다 앞에 그릴 팔 번호들 (기본: 옆모습은 가까운 팔).
    돌려준다: (목, 어깨, 머리 가운데)."""
    s = hm()
    neck = polar(hip, 180 - lean, s["spine"])
    spine = hose(hip, neck, sbend, sbend2)
    tx, ty = spine[-1][0] - spine[-3][0], spine[-1][1] - spine[-3][1]
    tl = math.hypot(tx, ty) or 1.0
    hr = s["hr"]
    head = (neck[0] + tx / tl * (hr + 0.5), neck[1] + ty / tl * (hr + 0.5))
    shoulder = spine[int(len(spine) * (0.72 if side else 0.6))]
    fa = set(front_arms if front_arms is not None else ([len(arms) - 1] if side else []))
    toes = (1, 1) if side else (-0.5, 0.5)
    for k, leg in enumerate(legs):
        if k == 0 and len(legs) > 1:
            _leg(f, hip, leg, toes[0])
    for k, arm in enumerate(arms):
        if k not in fa:
            _arm(f, shoulder, arm)
    f.line(spine)
    for k, leg in enumerate(legs):
        if not (k == 0 and len(legs) > 1):
            _leg(f, hip, leg, toes[1])
    f.head(head[0], head[1], head_tilt, mark=head_mark)
    for k, arm in enumerate(arms):
        if k in fa:
            _arm(f, shoulder, arm)
    return neck, shoulder, head


def arm_pts(shoulder, arm):
    """고무호스 팔: 어깨에서 out 각도로 먼저 뻗었다가 (팔꿈치 없이 둥글게) 손으로 휘어 간다. wob: 손 쪽 반을 옆으로 꿈틀.
    out이 없으면: 든 팔은 옆으로 (머리를 피해), 내린 팔은 손보다 조금 바깥으로 불룩."""
    tip, reach, *rest = arm
    out = rest[0] if rest else None
    wob = rest[1] if len(rest) > 1 else 0.0
    ln = hm()["arm"] * reach
    if isinstance(tip, tuple):
        p1 = tip
        ang = math.degrees(math.atan2(p1[0] - shoulder[0], p1[1] - shoulder[1]))
        ln = max(math.dist(shoulder, p1), 4.0)
    else:
        ang = tip
        p1 = polar(shoulder, ang, ln)
    if out is None:
        sg = 1 if ang >= 0 else -1
        out = sg * 82 if abs(ang) > 100 else ang + sg * 26
    c1 = polar(shoulder, out, 0.5 * max(ln, 0.75 * hm()["arm"]))
    m = lerp(c1, p1, 0.5)
    dx, dy = p1[0] - c1[0], p1[1] - c1[1]
    d = math.hypot(dx, dy) or 1.0
    c2 = (m[0] + dy / d * wob * ln, m[1] - dx / d * wob * ln)
    return cubic(shoulder, c1, c2, p1), p1


def _arm(f, shoulder, arm):
    pts, p1 = arm_pts(shoulder, arm)
    f.line(pts)
    f.items.append(("dot", (p1, 2.1)))


def _leg(f, hip, leg, toward):
    foot, b1, *rest = leg
    f.limb(hip, foot, b1, rest[0] if rest else None, end=toward)


def ground_hip(x, bend=0.92, up=0.0):
    """발이 바닥에 닿을 엉덩이 높이: 다리 길이 × bend (휜 만큼 짧다)."""
    return (x, FLOOR - 2.0 - hm()["leg"] * bend - up)


def floor_at(x, lift=0.0):
    return (x, FLOOR - 2.0 - lift)


def leg_k():
    return hm()["leg"] / 33.0


def arm_k():
    return hm()["arm"] / 28.0


def wig(i, a=1.0):
    """칸마다 번갈아 꿈틀 (+a, -a, …)."""
    return a if i % 2 == 0 else -a


# =====================================================================
# 포즈 — 옆모습은 오른쪽을 본다 (웹뷰가 왼쪽이면 뒤집는다)
# =====================================================================
WALK_FEET = [((-12, 0), (11, 0)), ((-7, 0), (6, 6)), ((11, 0), (-12, 0)), ((6, 6), (-7, 0))]


def hm_walk(sp, i):
    """통통 걷기: 발이 번갈아 앞으로, 무릎은 말랑하게 휘고 (칸마다 휘는 정도가 다르다), 팔은 다리와 반대로 흔들흔들, 몸이 위아래로 통통."""
    f = Fig()
    k = leg_k()
    up = [0, 3, 0, 3][i]
    hip = ground_hip(62, 0.92, up)
    (bx, bl), (fx_, fl) = WALK_FEET[i]
    legs = [(floor_at(hip[0] + bx * k, bl), [0.1, 0.22, 0.2, 0.32][i], [-0.05, 0.12, 0.08, 0.2][i]),
            (floor_at(hip[0] + fx_ * k, fl), [0.2, 0.32, 0.1, 0.22][i], [0.08, 0.2, -0.05, 0.12][i])]
    arms = [([40, 8, -38, 8][i], 0.95, None, wig(i, 0.12)), ([-38, 8, 40, 8][i], 0.95, None, wig(i, -0.12))]
    if i % 2:
        arms = [(-22, 0.95, -48, 0.15), (20, 0.95, 46, -0.15)]
    stand(f, hip=hip, lean=[5, 2, 5, 2][i], sbend=[0.1, -0.06, 0.1, -0.06][i], sbend2=[0.0, 0.06, 0.0, 0.06][i],
          head_tilt=[4, -3, 4, -3][i], arms=arms, legs=legs)
    return f.render()


RUN_FEET = [((-17, 8), (15, 0)), ((-10, 2), (9, 6)), ((15, 0), (-17, 8)), ((9, 6), (-10, 2))]


def hm_run(sp, i):
    """다다다 달리기: 앞으로 쑥 기울고, 다리는 크게 휘저어 바퀴처럼, 팔은 흐느적 크게 앞뒤로, 뒤에 속도선."""
    f = Fig()
    k = leg_k()
    up = [1, 6, 1, 6][i]
    hip = ground_hip(60, 0.84, up)
    (bx, bl), (fx_, fl) = RUN_FEET[i]
    legs = [(floor_at(hip[0] + bx * k, bl * k), [-0.25, 0.35, 0.3, 0.38][i], [0.1, 0.3, 0.28, 0.3][i]),
            (floor_at(hip[0] + fx_ * k, fl * k), [0.3, 0.38, -0.25, 0.35][i], [0.28, 0.3, 0.1, 0.3][i])]
    if i == 0:
        legs[0] = ((hip[0] - 17 * k, hip[1] + 18 * k), -0.4, 0.25)
    if i == 2:
        legs[1] = ((hip[0] - 17 * k, hip[1] + 18 * k), -0.4, 0.25)
    arms = [([95, -55, -70, -55][i], 1.0, None, wig(i, 0.25)), ([-70, 85, 95, 85][i], 1.0, None, wig(i, -0.25))]
    stand(f, hip=hip, lean=18, sbend=[0.14, 0.04, 0.14, 0.04][i], sbend2=-0.04, head_tilt=8, arms=arms, legs=legs)
    return f.render(fx=dp.speed_fx(4, (70, 84, 98), 13, [0, -3, 0, -3][i]))


def hm_roll(sp, i):
    """데굴: 무릎을 끌어안고 동글 웅크린 공 (앱이 돌린다 — 회전 원 안)."""
    f = Fig()
    hr = hm()["hr"]
    hx, hy = 73, 77
    neck = (hx - hr * 0.55, hy + hr * 0.85)
    hip = (47, 103)
    foot = (84, 109)
    f.limb(hip, (foot[0] - 6, foot[1] + 2), [-0.55, -0.6][i], [-0.3, -0.2][i], end=1)
    f.line(hose(hip, neck, [-0.45, -0.4][i]))
    f.limb(hip, foot, [-0.5, -0.45][i], [-0.15, -0.3][i], end=1)
    f.head(hx, hy, 30)
    sh = lerp(hip, neck, 0.78)
    f.line(hose(sh, (88, 98), [0.25, 0.35][i]))
    f.items.append(("dot", ((88, 98), 2.1)))
    return f.render()


def hm_idle(sp, i):
    """서서 쉬기: 숨 쉬듯 살짝 내려앉고, 늘어진 팔이 말랑하게 흔들."""
    f = Fig()
    hip = ground_hip(62, [0.93, 0.89][i])
    legs = [(floor_at(54), [0.1, 0.16][i], [0.02, 0.08][i]), (floor_at(71), [0.12, 0.18][i], [0.03, 0.1][i])]
    arms = [(-34, 0.95, None, [0.08, -0.08][i]), (32, 0.95, None, [-0.08, 0.1][i])]
    stand(f, hip=hip, lean=[1, -1][i], sbend=[0.05, 0.09][i], sbend2=[-0.03, 0.0][i], head_tilt=[0, 3][i], arms=arms, legs=legs)
    return f.render()


def hm_look(sp, i):
    """두리번: 머리를 이쪽저쪽 갸웃 (몸이 따라 휜다), 마지막엔 살짝 웅크림."""
    f = Fig()
    hip = ground_hip(62, [0.93, 0.93, 0.93, 0.88][i])
    legs = [(floor_at(54), 0.12), (floor_at(71), 0.14)]
    arms = [(-32, 0.95, None, 0.1), ([32, 32, 70, 32][i], 0.95, None, -0.1)]
    stand(f, hip=hip, lean=[-10, 0, 12, 2][i], sbend=[-0.18, 0.04, 0.2, 0.06][i], head_tilt=[-14, 0, 16, 0][i], arms=arms, legs=legs)
    return f.render()


def hm_lookup(sp, i):
    """올려다보기: 몸을 뒤로 젖혀 머리를 들고, 한 팔로 하늘을 가리킨다 (마지막 칸 ✧)."""
    f = Fig()
    hip = ground_hip(60, 0.93)
    legs = [(floor_at(54), 0.12), (floor_at(68), 0.14)]
    arms = [(-30, 0.95, None, 0.1), ([115, 128, 132][i], 1.2, 95, [0.15, -0.1, 0.12][i])]
    stand(f, hip=hip, lean=[-6, -12, -12][i], sbend=[0.1, 0.2, 0.2][i], head_tilt=[-10, -18, -18][i], arms=arms, legs=legs, front_arms=[])
    return f.render(fx=dp.lookup_fx(i, 112, 16))


def hm_sleep(sp, i):
    """옆으로 누워 새우잠: 머리는 바닥에, 몸은 뒤로 길게, 무릎은 접어 동글 (z)."""
    f = Fig()
    s = hm()
    hr = s["hr"]
    hx, hy = 94, FLOOR - 2 - hr + [0, 0.6][i]
    neck = (hx - hr - 0.5, FLOOR - 5)
    hip = (neck[0] - s["spine"], FLOOR - 7 + [0, -0.8][i])
    kl = s["leg"] * 0.55
    f.limb(hip, (hip[0] - kl, FLOOR - 3), -0.75, -0.4, end=-1)
    f.line(hose(neck, hip, [0.12, 0.15][i]))
    f.limb(hip, (hip[0] - kl * 0.8, FLOOR - 4), -0.9, -0.55, end=-1)
    f.head(hx, hy, 70)
    sh = lerp(hip, neck, 0.75)
    f.line(hose(sh, (hx - hr * 0.9, FLOOR - 3), [0.35, 0.4][i]))
    f.items.append(("dot", ((hx - hr * 0.9, FLOOR - 3), 2.1)))
    zs = [dp.z_fx(106, 66, 3.4), dp.z_fx(116, 54, 2.6)] if i == 0 else [dp.z_fx(108, 63, 3.6), dp.z_fx(119, 50, 2.8)]
    return f.render(fx=zs)


def hm_hop(sp, i):
    """폴짝 (용수철처럼): 깊게 웅크림 → 쭉 늘어나며 두 팔 번쩍 (만세) → 공중에서 무릎 접기."""
    f = Fig()
    k = leg_k()
    if i == 0:
        hip = ground_hip(62, 0.55)
        legs = [(floor_at(53), 0.6, 0.45), (floor_at(70), 0.62, 0.5)]
        arms = [(-70, 0.9, None, 0.15), (-50, 0.9, None, -0.15)]
        stand(f, hip=hip, lean=16, sbend=0.2, head_tilt=6, arms=arms, legs=legs)
        return f.render()
    if i == 1:
        hip = ground_hip(62, 1.0, 16)
        legs = [((58, hip[1] + 34 * k), -0.12, 0.08), ((66, hip[1] + 33 * k), 0.1, -0.12)]
        arms = [(-150, 1.2, -88, 0.12), (150, 1.2, 88, -0.12)]
        stand(f, hip=hip, lean=-2, sbend=-0.08, sbend2=0.08, arms=arms, legs=legs)
        return f.render()
    hip = ground_hip(62, 0.75, 12)
    legs = [((55, hip[1] + 18 * k), 0.7, 0.4), ((70, hip[1] + 20 * k), 0.65, 0.35)]
    arms = [(-120, 1.0, None, 0.2), (115, 1.0, None, -0.2)]
    stand(f, hip=hip, lean=6, sbend=0.1, head_tilt=4, arms=arms, legs=legs)
    return f.render()


def hm_fall(sp, i):
    """떨어지며 허우적: 팔다리가 제각각 꼬불꼬불, 별."""
    f = Fig()
    k = leg_k()
    hip = ground_hip(64, 1.0, 10)
    s = wig(i)
    legs = [((hip[0] - 15 * k, hip[1] + 26 * k), 0.4 * s, -0.4 * s), ((hip[0] + 16 * k, hip[1] + 24 * k), -0.45 * s, 0.35 * s)]
    arms = [([-130, -100][i], 1.0, -75, 0.3 * s), ([100, 135][i], 1.0, 75, -0.3 * s)]
    stand(f, hip=hip, lean=[-12, 12][i], sbend=0.2 * s, sbend2=-0.15 * s, head_tilt=[-12, 12][i], arms=arms, legs=legs)
    stars = dp._stars([(106, 22, 3.6), (22, 30, 3.0)] if i == 0 else [(24, 20, 3.6), (108, 32, 3.0)])
    return f.render(fx=stars)


def hm_dragged(sp, i):
    """잡혀서 대롱대롱: 머리를 잡혀 팔다리가 국수처럼 축 늘어져 흔들흔들, 땀."""
    f = Fig()
    k = leg_k()
    s = [-1, 1][i]
    hip = ground_hip(64 + 2 * s, 1.0, 8)
    legs = [((hip[0] - 7 + 3 * s, hip[1] + 32 * k), 0.15 * s, -0.15 * s), ((hip[0] + 7 + 4 * s, hip[1] + 31 * k), -0.12 * s, 0.16 * s)]
    arms = [(-28 + 8 * s, 1.0, None, 0.2 * s), (28 + 8 * s, 1.0, None, -0.2 * s)]
    stand(f, hip=hip, lean=-3 * s, sbend=0.12 * s, sbend2=-0.12 * s, head_tilt=4 * s, arms=arms, legs=legs, side=False)
    return f.render(fx=dp.sweat_fx(100, 30))


def hm_land(sp, i):
    f = Fig()
    if i == 0:
        hip = ground_hip(62, 0.5)
        legs = [(floor_at(51), 0.62, 0.5), (floor_at(72), 0.64, 0.5)]
        arms = [(-100, 0.95, -75, 0.25), (100, 0.95, 75, -0.25)]
        stand(f, hip=hip, lean=8, sbend=0.2, head_tilt=4, arms=arms, legs=legs)
        return f.render()
    hip = ground_hip(62, 0.86)
    legs = [(floor_at(54), 0.2, 0.1), (floor_at(70), 0.22, 0.12)]
    arms = [(-45, 0.95, None, 0.1), (42, 0.95, None, -0.1)]
    stand(f, hip=hip, lean=2, sbend=0.08, arms=arms, legs=legs)
    return f.render()


# ---- 앞모습 ----
def _front_sit(f, *, arms, lean=0.0, sbend=0.0, sbend2=None, head_tilt=0.0, knees=1.0, front_arms=None):
    """바닥에 털썩 앉은 앞모습: 엉덩이가 바닥, 무릎을 양옆으로 세운 다리 (발은 바닥)."""
    hip = (64, FLOOR - 5)
    kk = leg_k()
    for s in (-1, 1):
        f.limb(hip, (64 + s * 25 * kk, FLOOR - 2), s * 0.95 * knees, s * 0.55 * knees, end=0.5 * s)
    return stand(f, hip=hip, lean=lean, sbend=sbend, sbend2=sbend2, head_tilt=head_tilt, arms=arms, side=False, front_arms=front_arms)


def knee_hands(b):
    """앉은 앞모습: 두 손을 무릎 위에."""
    kk = leg_k()
    return [((64 - 21 * kk, FLOOR - 13 * kk - 5), 1, -100, 0.1 + b), ((64 + 21 * kk, FLOOR - 13 * kk - 5), 1, 100, -0.1 - b)]


def hm_sit_front(sp, i):
    """털썩 앉아 앞을 본다: 두 손은 무릎 위, 숨 쉬듯 몸이 살짝 휜다."""
    f = Fig()
    b = [0, 0.04, 0.08, 0.04][i]
    _front_sit(f, arms=knee_hands(b), sbend=b, head_tilt=[0, 1, 2, 1][i])
    return f.render()


def hm_wave(sp, i):
    """안녕~: 한 팔을 머리 옆으로 번쩍 들어 국수처럼 흔든다 (칸마다 반대로 휜다)."""
    f = Fig()
    arms = [knee_hands(0)[0], ([118, 140, 118, 140][i], 1.25, 95, wig(i, 0.35))]
    _front_sit(f, arms=arms, lean=[-4, -2, -4, -2][i], sbend=[0.06, -0.04, 0.06, -0.04][i], head_tilt=[-5, -2, -5, -2][i])
    return f.render()


def hm_tilt(sp, i):
    f = Fig()
    t = dp._swing([10, 16, 16][i])
    _front_sit(f, arms=knee_hands(0.05), lean=t * 0.4, sbend=-0.08, head_tilt=t)
    return f.render(fx=[dp.question_fx(20, 26 + [2, 0, -2][i])])


def hm_belly(sp, i):
    """발라당: 등을 대고 누워 팔다리를 하늘로 버둥버둥 (꼬불꼬불)."""
    f = Fig()
    s = hm()
    w = wig(i)
    hr = s["hr"]
    hx, hy = 64 + s["spine"] * 0.5 + hr * 0.5, FLOOR - 2 - hr
    hip = (64 - s["spine"] * 0.5, FLOOR - 6)
    neck = (hx - hr - 0.5, FLOOR - 7)
    ka, kl = s["arm"], s["leg"]
    for j, (dx, b) in enumerate(((-20, 0.2), (-3, -0.2))):
        f.limb(hip, (hip[0] + dx, hip[1] - kl * 0.78), b * w, -b * w, end=-0.5 if j == 0 else 0.5)
    f.line(hose(hip, neck, 0.12 * w))
    f.head(hx, hy, 80)
    sh = lerp(hip, neck, 0.72)
    for dx, b in ((2, 0.22), (17, -0.22)):
        f.limb(sh, (sh[0] + dx, sh[1] - ka * 0.8), b * w, -b * w * 0.8, end="hand")
    return f.render()


def hm_stretch(sp, i):
    """기지개: 일어서서 두 팔을 머리 위로 쭉 — 몸이 늘어난다 (가운데 칸은 옆으로 휘청)."""
    f = Fig()
    if i == 2:
        hip = ground_hip(64, 0.93)
        legs = [(floor_at(55), -0.08), (floor_at(73), 0.08)]
        stand(f, hip=hip, arms=[(-30, 0.95), (30, 0.95)], legs=legs, side=False)
        return f.render()
    hip = ground_hip(64, 1.0, 1)
    legs = [(floor_at(57), -0.04), (floor_at(71), 0.04)]
    arms = [([-145, -135][i], 1.3, -98, [0.05, 0.2][i]), ([145, 155][i], 1.3, 98, [-0.05, 0.15][i])]
    stand(f, hip=hip, lean=[0, -10][i], sbend=[0.0, -0.2][i], head_tilt=[0, -10][i], arms=arms, legs=legs, side=False)
    return f.render()


def hm_heart(sp, i):
    """하트를 품에 꼭: 두 팔로 가슴 앞 하트를 감싸고, 하트가 퐁퐁."""
    f = Fig()
    hip = ground_hip(64, 0.93)
    legs = [(floor_at(55), -0.08), (floor_at(73), 0.08)]
    hy = hip[1] - hm()["spine"] * 0.45
    arms = [((56, hy + 4), 1, -110, 0.1), ((72, hy + 4), 1, 110, -0.1)]
    stand(f, hip=hip, sbend=[0, 0.05, 0][i], head_tilt=[0, 5, 0][i], arms=arms, legs=legs, side=False, front_arms=[0, 1])
    return f.render(fx=dp.heart_burst(i), held=[lambda: dp.held_heart(hy)])


def hm_tickle(sp, i):
    """간질간질: 온몸이 꼬불꼬불 S자로 꿈틀, 팔다리가 제멋대로."""
    f = Fig()
    s = [1, -1, 0.9][i]
    hip = ground_hip(64 + 2 * s, 0.9)
    legs = [(floor_at(55), 0.3 * s, -0.3 * s), (floor_at(73), -0.3 * s, 0.3 * s)]
    arms = [([-130, -60, -100][i], 1.0, -85, 0.35 * s), ([60, 130, 100][i], 1.0, 85, -0.35 * s)]
    stand(f, hip=hip, lean=dp._swing(8 * s), sbend=0.25 * s, sbend2=-0.25 * s, head_tilt=dp._swing(10 * s), arms=arms, legs=legs,
          side=False)
    return f.render()


def hm_annoyed(sp, i):
    """뾰로통: 두 손을 허리에 (팔꿈치는 밖으로), 고개를 홱, 발을 동동."""
    f = Fig()
    hip = ground_hip(62, 0.93)
    legs = [(floor_at(55), 0.08), (floor_at(70, [0, 4][i]), [0.12, 0.3][i])]
    arms = [((hip[0] - 3, hip[1] - 2), 1, -100, 0.0), ((hip[0] + 4, hip[1] - 2), 1, 100, 0.0)]
    stand(f, hip=hip, lean=-6, sbend=-0.1, head_tilt=-12, arms=arms, legs=legs)
    return f.render(fx=[dp.anger_fx(102, 34 - i, 7.0)])


def hm_hungry(sp, i):
    """배고파 축 처짐: 앉아서 고개를 푹, 팔은 바닥까지 축, 생각 풍선에 막대사탕."""
    f = Fig()
    d = [0, 1, 0][i]
    kk = leg_k()
    arms = [((64 - 13 * kk, FLOOR - 5), 1, -40, 0.1 + 0.03 * d), ((64 + 13 * kk, FLOOR - 5), 1, 40, -0.1 - 0.03 * d)]  # 손이 바닥에 축
    _front_sit(f, arms=arms, lean=-6, sbend=0.25 + 0.02 * d, sbend2=0.1, head_tilt=-8 - 2 * d, knees=0.8)
    return f.render(fx=dh.lolly_bubble_fx(106, 18))


def hm_eat(sp, i):
    """콕콕 (그릇 없음): 0·2 = 쪼그려 앉아 머리를 그릇 밥 더미(프레임 (64, 86))에 콕, 1·3 = 몸을 쏙 펴고 냠."""
    f = Fig()
    s = hm()
    mx, my = dp._frame_to_pre(64, 86)
    if i % 2 == 0:
        hy = my - s["hr"] + 2
        hip = (64, FLOOR - 9)
        for sg in (-1, 1):
            f.limb(hip, (64 + sg * 22 * leg_k(), FLOOR - 2), sg * 1.0, sg * 0.6, end=0.5 * sg)
        f.line(hose(hip, (64, hy + s["hr"]), 0.15))
        sh = (64, hip[1] - 4)
        for sg in (-1, 1):
            f.line(hose(sh, (64 + sg * 26 * arm_k(), FLOOR - 3), -sg * 0.3))
            f.items.append(("dot", ((64 + sg * 26 * arm_k(), FLOOR - 3), 2.1)))
        f.head(64, hy, 0)
        return f.render(fx=dp.eat_crumbs(i, (255, 196, 210)))
    hip = ground_hip(64, 0.92)
    legs = [(floor_at(54), -0.12), (floor_at(74), 0.12)]
    arms = [(-40, 0.95, None, 0.12), (40, 0.95, None, -0.12)]
    stand(f, hip=hip, sbend=[0, 0.06, 0, -0.06][i], head_tilt=[0, 5, 0, -5][i], arms=arms, legs=legs, side=False)
    return f.render(fx=dp.eat_crumbs(i, (255, 196, 210)))


def hm_level_up(sp, i):
    """쑥쑥 컸다!: 폴짝 뛰며 두 팔을 하늘로 번쩍, 별이 반짝."""
    f = Fig()
    up = [0, 4, 8][i]
    k = leg_k()
    hip = ground_hip(64, [0.9, 0.97, 1.0][i], up)
    legs = [(floor_at(56), -0.1), (floor_at(72), 0.1)]
    if i == 2:
        legs = [((56, hip[1] + 30 * k), -0.2, 0.15), ((72, hip[1] + 30 * k), 0.2, -0.15)]
    arms = [([-130, -138, -142][i], 1.3, -98, wig(i, 0.15)), ([130, 138, 142][i], 1.3, 98, wig(i, -0.15))]
    stand(f, hip=hip, sbend=[0.05, 0, -0.04][i], arms=arms, legs=legs, side=False)
    return f.render(fx=dp.level_up_stars(i))


PANIC_FEET = [((-15, 9), (16, 0)), ((-9, 3), (10, 6)), ((16, 0), (-15, 9)), ((10, 6), (-9, 3))]


def hm_panic(sp, i):
    """으악! 등을 돌려 줄행랑: 앞으로 잔뜩 기울어 다리를 마구 휘젓고, 두 팔은 머리 위로 번쩍 (만세) — 흐물흐물 흔들, 땀방울·속도선."""
    f = Fig()
    k = leg_k()
    up = [2, 7, 2, 7][i]
    hip = ground_hip(62, 0.82, up)
    (bx, bl), (fx_, fl) = PANIC_FEET[i]
    legs = [(floor_at(hip[0] + bx * k, bl * k), [-0.3, -0.15, 0.35, 0.45][i], [0.2, 0.2, 0.3, 0.3][i]),
            (floor_at(hip[0] + fx_ * k, fl * k), [0.35, 0.45, -0.3, -0.15][i], [0.3, 0.3, 0.2, 0.2][i])]
    if i == 0:
        legs[0] = ((hip[0] - 16 * k, hip[1] + 18 * k), -0.4, 0.3)
    if i == 2:
        legs[1] = ((hip[0] - 16 * k, hip[1] + 18 * k), -0.4, 0.3)
    w = wig(i)
    s = hm()
    r = 1.35 * max(1.0, 1.75 * s["hr"] / s["arm"])  # 큰 머리(아기)도 손이 머리 위로
    arms = [(-155 + 6 * w, r, -100, 0.2 * w), (128 - 6 * w, r, 100, -0.22 * w)]
    stand(f, hip=hip, lean=16, sbend=0.1, sbend2=-0.04, head_tilt=10, arms=arms, legs=legs, front_arms=[])
    fx = dp.speed_fx(2, (74, 88, 102), 12, [0, -3, 0, -3][i]) + dp.sweat_fx([26, 24, 26, 24][i], [46, 42, 46, 42][i])
    return f.render(fx=fx)


# ---- 창 옆면 매달리기 (코어 State::Rig) — 창은 오른쪽: 옆면 x = 몸 가운데 + 몸 반 폭 (코어 HALF_WIDTH_RATIO 0.3) ----
def edge_x():
    return dp._frame_to_pre(64 + 0.3 * dp.FRAME, 0)[0] - 1.5


GRIP_Y = 26.0
HANG_LEAN = -20


def _hang_hip(grip):
    """잡은 손이 어깨 오른쪽 위에 오는 엉덩이 자리 (몸은 창 반대쪽으로 비스듬히 — 머리가 손 아래 왼쪽)."""
    s = hm()
    up = polar((0, 0), 180 - HANG_LEAN, s["spine"] * 0.72)  # 엉덩이 → 어깨
    reach = s["arm"] * 1.05
    return (grip[0] - reach * 0.45 - up[0], grip[1] + reach * 0.9 - up[1])


def hm_hang(sp, i):
    """돛대에 매달린 뱃사람처럼: 한 손으로 창 옆면을 꽉 잡고 몸은 비스듬히, 한 발은 옆면을 딛고,
    다른 팔은 허공에서 흐물흐물 손 흔들기 (칸마다 반대로 꿈틀), 다른 다리는 대롱대롱."""
    f = Fig()
    s = hm()
    ex = edge_x()
    grip = (ex, GRIP_Y + [0, 1, 0, 1][i])
    hip = _hang_hip(grip)
    w = wig(i)
    legs = [((hip[0] - 4 + 4 * w, hip[1] + s["leg"] * 0.95), 0.18 * w, -0.22 * w),
            ((ex - 5, hip[1] + s["leg"] * 0.6), -0.5, -0.25)]
    arms = [([-100, -135, -85, -140][i], 1.3, -80, [0.35, -0.3, 0.3, -0.35][i]), (grip, 1, 165, 0.06)]
    stand(f, hip=hip, lean=HANG_LEAN, sbend=-0.12, sbend2=0.06 * w, head_tilt=-18, arms=arms, legs=legs, front_arms=[1])
    return f.render()


def hm_rope(sp, i):
    """밧줄 타듯 손을 번갈아 창 옆면을 오르내린다: 두 손이 옆면을 위아래로 바꿔 잡고, 두 발은 옆면을 번갈아 딛는다."""
    f = Fig()
    s = hm()
    ex = edge_x()
    d = s["arm"] * 0.5
    hand = [(0, d), (d * 0.35, d * 0.65), (d, 0), (d * 0.65, d * 0.35)][i]
    hip = _hang_hip((ex, GRIP_Y + d * 0.5))
    fy = [(0.45, 0.8), (0.58, 0.68), (0.8, 0.45), (0.68, 0.58)][i]
    legs = [((ex - 5, hip[1] + s["leg"] * fy[0]), -0.55, -0.25), ((ex - 5, hip[1] + s["leg"] * fy[1]), -0.5, -0.2)]
    arms = [((ex, GRIP_Y + hand[0]), 1, 160, 0.08), ((ex, GRIP_Y + hand[1]), 1, 165, -0.08)]
    stand(f, hip=hip, lean=HANG_LEAN + 6, sbend=[-0.12, -0.06, -0.12, -0.06][i], head_tilt=-12, arms=arms, legs=legs, front_arms=[0, 1])
    return f.render()


HEUMUL_POSES = {"walk": hm_walk, "run": hm_run, "roll": hm_roll, "idle": hm_idle, "look": hm_look, "sleep": hm_sleep, "hop": hm_hop,
                "fall": hm_fall, "dragged": hm_dragged, "land": hm_land, "sit_front": hm_sit_front, "wave": hm_wave, "tilt": hm_tilt,
                "belly": hm_belly, "stretch": hm_stretch, "heart": hm_heart, "tickle": hm_tickle, "annoyed": hm_annoyed,
                "hungry": hm_hungry, "eat": hm_eat, "level_up": hm_level_up, "lookup": hm_lookup, "panic": hm_panic,
                "hang": hm_hang, "rope": hm_rope}
RAW_POSES = dict(HEUMUL_POSES)
HEUMUL_POSES = {k: _fit(v) for k, v in HEUMUL_POSES.items()}

_FLAT = dict(pale=0.0, soft=0.0, fit=1.0, line=1.0)
HEUMUL = Species(
    id="heumul", name="흐물이", walk_speed=110, scale=1.0, base=INK, back=INK, belly=INK, nose=INK, mouth="w",
    front_tail_color=INK, tail_color=INK, stage_size=dict.fromkeys(STAGES, 0.9), stripe=INK, outline=INK,
    stage_tweak={st: dict(_FLAT) for st in STAGES},
    specials=["lookup", "panic", "hang", "rope"], poses=HEUMUL_POSES, **dp.NO_COMMON_PARTS,
)


def register():
    dp.SPECIES[HEUMUL.id] = HEUMUL


register()


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(HEUMUL, root / "ui" / "pets" / HEUMUL.id))
    dh.preview_species(by_stage, pv / "stages_heumul.png")
    print("ok: heumul")


if __name__ == "__main__":
    main()
