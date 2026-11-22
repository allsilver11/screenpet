"""해그리드 🧔 — 덩치 큰 (다른 펫의 약 1.4배 — stage_size) 사람 모양 꼬마: 덥수룩한 짙은 갈색 머리·수염 (얼굴 대부분을 덮고 다정한 눈만 빼꼼),
긴 갈색 두더지 가죽 코트, 분홍 우산 (지팡이 대신). 몸·공통 동작은 draw_wizard (Look.dims로 넓은 어깨·굵은 팔다리·작은 머리,
Look.prop 우산)를 쓰고, 여기서는 생김새·해그리드만의 동작·등록만.

    python tools/draw_hagrid.py → ui/pets/hagrid/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_hagrid,climb_hagrid,wizard_hagrid}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

특기 (코어 brain_hagrid.rs·brain_broom.rs):
  🐉 dragon — 빗자루 대신 날개 달린 초록·구릿빛 용 등에 앉아 날기 (날개 퍼덕 3칸 — 해그리드는 작게, 용이 칸을 채움)
  🏍️ sidecar — 사이드카 달린 오토바이 타기 (바퀴 3칸 — 사이드카 자리 = grip, 다른 펫이 앉음) · sidecar_fly — 오토바이째 붕 날기 (2칸)
  🤗 hug — 다른 펫을 가슴에 안고 서기 (손 = grip) · hug_walk — 안고 걷기 · pat — 허리 숙여 쓰담쓰담 (하트)
  🐉 egg — 앉아 두 손에 용알을 품음 (알·아기 용은 앱 덮개 창 — 손 자리 grip) · offer — 코트에서 🪨 락 케이크를 꺼내 바닥에 내려놓음 (한 번)
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_wizard as wz  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, heart_fx, sparkle_fx  # noqa: E402
from draw_spider import polar  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

HAIR = (78, 50, 34)  # 짙은 갈색 머리·수염 (한 덩어리, 같은 색)
HAIR_DARK = (56, 34, 22)  # 곱슬 결
HAIR_HI = (120, 84, 58)  # 윤기
NOSE, NOSE_HI = (236, 172, 148), (255, 214, 196)  # 둥근 코
COAT, COAT_FAR = (124, 90, 60), (98, 70, 46)  # 갈색 두더지 가죽 코트
COAT_IN = (88, 62, 40)  # 코트 안감
SHIRT = (168, 116, 74)  # 앞섶 사이 셔츠
PANTS, PANTS_FAR = (82, 66, 54), (64, 52, 42)  # 바지
UMBRELLA, UMBRELLA_DARK = (244, 128, 176), (204, 84, 138)  # 분홍 우산
SHAFT = (110, 80, 60)  # 우산대·손잡이
DRAGON, DRAGON_DARK = (98, 156, 92), (66, 118, 64)  # 🐉 초록 용
DRAGON_BELLY = (214, 168, 96)  # 구릿빛 배
WING, WING_BONE = (150, 198, 120), (180, 132, 70)  # 날개 막·뼈 (구릿빛)
HORN = (236, 214, 160)
EYE_GOLD = (250, 206, 70)
BIKE, BIKE_HI = (64, 92, 120), (120, 150, 180)  # 🏍️ 하늘색 오토바이
TUB, TUB_HI = (70, 104, 136), (128, 160, 190)  # 사이드카
TYRE, RIM, HUB = (34, 32, 40), (150, 156, 170), (214, 218, 228)
LAMP = (255, 228, 110)
PUFF = (232, 236, 244)  # 배기·구름 퐁
CAKE, CAKE_DARK, CAKE_DOT = (176, 150, 116), (132, 108, 80), (96, 70, 52)  # 🪨 락 케이크 (돌같이 울퉁불퉁한 빵 + 건포도)
WARM = (255, 176, 96)  # 알을 데우는 따뜻한 김


# =====================================================================
# 머리·수염·코·우산 (Look)
# =====================================================================
def tufts_at(hx, hy, hrx, hry, angles, d=1.02, r=0.3):
    """머리 둘레 angles(도)에 동그란 머리 뭉치 — (각도, 크기 배율)."""
    return [C(hx + hrx * d * math.cos(math.radians(a)), hy + hry * d * math.sin(math.radians(a)), hrx * r * kk) for a, kk in angles]


def curls(hx, hy, hrx, hry, spots):
    """곱슬 결 (작은 호들) — (가로 몫, 세로 몫, 시작 각)."""
    return [L(dp.arc(hx + a * hrx, hy + b * hry, 0.12 * hrx, 0.1 * hry, a0, a0 + 200, 6), 1.1) for a, b, a0 in spots]


def hagrid_hair(view_, hx, hy, hrx, hry, q):
    """덥수룩한 짙은 갈색 머리·수염 한 덩어리: 정수리에서 옆머리·구레나룻을 타고 턱 아래 가슴까지 끊김 없이 흘러내리는 한 실루엣
    (같은 색·같은 곱슬 결, 머리와 수염 사이 선 없음) — 얼굴 (이마 아래 눈·코·볼)만 가운데 구멍으로 빼꼼. 뒷모습은 머리 덩어리만.
    → (머리 도형에 더할 도형, 삐죽 조각, 머리 안 무늬)."""
    sky = hy - hry * 1.8 - q - 10
    shine = [L(dp.arc(hx, hy, hrx * 0.8, hry * 0.8, 215, 255, 8), 2.0)]
    if view_ == "back":
        tf = tufts_at(hx, hy, hrx, hry, [(a, 1.0 + 0.08 * ((a // 30) % 2)) for a in range(-190, 20, 26)] + [(40, 0.95), (140, 0.95), (75, 1.0), (105, 1.0)])
        marks = [([T((hx - hrx * 2, sky), (hx + hrx * 2, sky), (hx + hrx * 2, hy + 2 * hry), (hx - hrx * 2, hy + 2 * hry))], HAIR), (shine, HAIR_HI),
                 (curls(hx, hy, hrx, hry, [(-0.4, -0.5, 200), (0.3, -0.3, 20), (-0.1, 0.3, 120), (0.5, 0.5, 300), (-0.6, 0.6, 60)]), HAIR_DARK)]
        return [E(hx - 1.02 * hrx, hy - hry - q, hx + 1.02 * hrx, hy + 0.9 * hry)] + tf, [], marks
    P = lambda a, b: (hx + a * hrx, hy + b * hry)  # noqa: E731
    if view_ == "front":
        tf = tufts_at(hx, hy, hrx, hry, [(-200, 1.05), (-172, 1.1), (-145, 1.05), (-118, 1.0), (-90, 1.05), (-62, 1.0), (-35, 1.05), (-8, 1.1), (20, 1.05)])
        # 얼굴 구멍 테두리 (위 = 들쭉날쭉 앞머리, 양옆 = 볼을 감싸 내려오는 옆머리·구레나룻)
        z = [(1.3, 0.5), (0.9, 0.36), (0.8, 0.1), (0.8, -0.14), (0.66, -0.3), (0.48, -0.22), (0.3, -0.34), (0.1, -0.26), (-0.1, -0.36), (-0.3, -0.26),
             (-0.48, -0.34), (-0.66, -0.24), (-0.8, -0.14), (-0.8, 0.1), (-0.9, 0.36), (-1.3, 0.5)]
        # 수염: 옆머리에서 이어져 볼 아래를 들쭉날쭉 덮고 가슴까지
        top = [(-1.04, -0.08), (-0.84, 0.3), (-0.55, 0.38), (-0.28, 0.3), (0.0, 0.36), (0.28, 0.3), (0.55, 0.38), (0.84, 0.3), (1.04, -0.08)]
        low = [(1.1, 0.5), (0.95, 1.1), (0.5, 1.5), (0.0, 1.62), (-0.5, 1.5), (-0.95, 1.1), (-1.1, 0.5)]
        pf = [(-1.0, 0.55, 0.3), (1.0, 0.55, 0.3), (-0.78, 1.08, 0.3), (0.78, 1.08, 0.3), (-0.4, 1.42, 0.3), (0.4, 1.42, 0.3), (0.0, 1.55, 0.3)]
        hair_shape = E(hx - 1.0 * hrx, hy - hry - q, hx + 1.0 * hrx, hy - 0.1 * hry)
        cl = [(-0.5, -0.62, 200), (0.2, -0.75, 10), (0.6, -0.5, 300), (-1.0, -0.1, 90), (1.0, -0.05, 250), (-0.95, 0.4, 70), (0.95, 0.45, 110),
              (-0.7, 0.8, 120), (0.65, 0.85, 60), (-0.3, 1.15, 160), (0.3, 1.2, 20), (0.0, 0.95, 90)]
        stache = [L(dp.arc(*P(-0.24, 0.36), 0.24 * hrx, 0.12 * hry, 20, 180, 6), 1.1), L(dp.arc(*P(0.24, 0.36), 0.24 * hrx, 0.12 * hry, 0, 160, 6), 1.1)]
    else:
        # 옆모습 (오른쪽을 봄): 앞머리는 이마 위에서 들쭉날쭉, 뒤통수·목덜미를 덮고 귀 자리를 지나 그대로 수염으로
        tf = tufts_at(hx, hy, hrx, hry, [(-190, 1.08), (-160, 1.1), (-130, 1.08), (-100, 1.05), (-72, 1.0), (-46, 0.95), (160, 1.05), (132, 1.0)])
        tf += [C(hx - 0.7 * hrx, hy + 0.95 * hry, 0.34 * hrx), C(hx - 0.25 * hrx, hy + 1.0 * hry, 0.28 * hrx)]
        z = [(1.25, -0.42), (0.98, -0.36), (0.8, -0.5), (0.62, -0.34), (0.44, -0.48), (0.26, -0.28), (0.12, -0.36), (0.04, 0.05), (-0.02, 0.4),
             (-0.15, 0.8), (-0.4, 1.3), (-1.4, 1.3)]
        top = [(-0.2, 0.0), (0.05, 0.3), (0.32, 0.4), (0.58, 0.32), (0.84, 0.38), (1.1, 0.28)]
        low = [(1.26, 0.7), (1.08, 1.22), (0.62, 1.56), (0.1, 1.46), (-0.22, 1.0), (-0.28, 0.4)]
        pf = [(1.12, 0.72, 0.3), (0.9, 1.2, 0.3), (0.5, 1.46, 0.3), (0.08, 1.3, 0.28), (-0.15, 0.8, 0.26)]
        hair_shape = E(hx - 1.05 * hrx, hy - hry - q, hx + 0.9 * hrx, hy + 0.9 * hry)
        cl = [(-0.5, -0.5, 200), (0.2, -0.7, 20), (-0.8, 0.2, 100), (-0.4, 0.9, 300), (-0.1, 0.3, 60), (0.3, 0.8, 120), (0.8, 0.95, 60),
              (0.45, 1.2, 160), (0.0, 0.6, 90)]
        stache = [L(dp.arc(*P(0.84, 0.4), 0.3 * hrx, 0.12 * hry, 10, 170, 6), 1.1)]
    edge = [P(a, b) for a, b in z]
    beard = bushy(top + low, pf, hx, hy, hrx, hry)
    mass = [T(*edge, (hx - hrx * 1.6, sky), (hx + hrx * 1.6, sky))] + beard
    marks = [(mass, HAIR), (shine, HAIR_HI), (curls(hx, hy, hrx, hry, cl) + stache, HAIR_DARK)]
    return [hair_shape] + tf + beard, [], marks


def bushy(pts, puffs_, hx, hy, hrx, hry):
    """수염 덩어리: 테두리 다각형 + 아래쪽 동그란 뭉치 (녹이면 몽글몽글 부스스) — 점은 (가로 몫, 세로 몫), 뭉치는 (가로, 세로, 반지름 몫)."""
    P = lambda a, b: (hx + a * hrx, hy + b * hry)  # noqa: E731
    return [T(*[P(a, b) for a, b in pts])] + [C(*P(a, b), r * hrx) for a, b, r in puffs_]


def hagrid_face(view_, hx, hy, hrx, hry, far_eye, near_eye):
    """둥근 코 (콧수염 위, 빛 한 점) — 앞모습은 가운데, 옆모습은 얼굴 앞쪽 끝에 볼록."""
    if view_ == "front":
        cx, cy, rx, ry = hx, hy + 0.22 * hry, 0.17 * hrx, 0.13 * hry
    elif view_ == "side":
        cx, cy, rx, ry = hx + 0.98 * hrx, hy + 0.2 * hry, 0.16 * hrx, 0.14 * hry
    else:
        return []
    return [([E(cx - rx - 1.0, cy - ry - 1.0, cx + rx + 1.0, cy + ry + 1.0)], wz.OUT), ([E(cx - rx, cy - ry, cx + rx, cy + ry)], NOSE),
            ([C(cx - rx * 0.35, cy - ry * 0.35, rx * 0.3)], NOSE_HI)]


def umbrella(hand, a, length):
    """분홍 우산 (접힌 채): 손에서 a도 쪽으로 — 손 뒤로 굽은 손잡이, 가는 우산대, 가운데가 불룩한 분홍 천 (접힌 주름), 끝 쇠촉. 끝 자리도 돌려줌."""
    ln = length * 1.9
    tip = polar(hand, a, ln)
    root = polar(hand, a, ln * 0.3)
    mid = polar(hand, a, ln * 0.55)
    end = polar(hand, a, ln * 0.9)
    w = 3.0 + ln * 0.06
    nx, ny = math.cos(math.radians(a)), -math.sin(math.radians(a))  # 우산대에 수직
    canopy = T(root, (mid[0] + nx * w, mid[1] + ny * w), end, (mid[0] - nx * w, mid[1] - ny * w))
    hook_pts = [hand, polar(hand, a + 180, 3.0)]
    hb = hook_pts[-1]
    hook_pts += [polar(hb, a + 180 + 60, 2.6), polar(polar(hb, a + 180 + 60, 2.6), a + 90, 2.6)]
    shaft = L([hand, tip], 1.6)
    hook = L(hook_pts, 1.8)
    folds = [L([root, (mid[0] + nx * w * 0.5, mid[1] + ny * w * 0.5), end], 0.9), L([root, (mid[0] - nx * w * 0.4, mid[1] - ny * w * 0.4), end], 0.9)]
    return Part([canopy, shaft, hook], UMBRELLA, [([shaft, hook], SHAFT), (folds, UMBRELLA_DARK), ([L([end, tip], 1.6)], RIM)],
                outline=True, grow=0), tip


def cake_bubble_fx(cx, cy, r=12.0):
    """🧁 생각 풍선 (배고플 때): 울퉁불퉁 락 케이크 (건포도 점)."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    return fx + cake_fx(cx, cy + 1, 6.5)


def cake_fx(cx, cy, r):
    """🪨 락 케이크 한 덩이 (돌같이 울퉁불퉁한 동그란 빵 + 건포도 점) — fx 목록."""
    bumps = [C(cx - r * 0.5, cy - r * 0.15, r * 0.6), C(cx + r * 0.45, cy - r * 0.2, r * 0.62), C(cx, cy - r * 0.5, r * 0.55), C(cx, cy + r * 0.1, r * 0.75)]
    dots = [C(cx - r * 0.4, cy - r * 0.2, r * 0.13), C(cx + r * 0.35, cy - r * 0.35, r * 0.13), C(cx + r * 0.1, cy + r * 0.25, r * 0.13)]
    return [(bumps, CAKE, 1.2), ([L(dp.arc(cx, cy, r * 0.8, r * 0.5, 20, 160, 8), 1.0)], CAKE_DARK, 0), (dots, CAKE_DOT, 0)]


HAGRID_LOOK = wz.Look(hair=hagrid_hair, hair_col=HAIR, extra=hagrid_face, food_fx=cake_bubble_fx, side_ear=False, cap_lift=0.35, eye_k=0.78,
                      back_reach=6.0, robe=COAT, robe_far=COAT_FAR, robe_in=COAT_IN, jumper=SHIRT, pants=PANTS, pants_far=PANTS_FAR, scarf=False,
                      blush=None, prop=umbrella,
                      dims=dict(hrx=0.94, hry=0.94, tw=1.32, th=1.12, aw=1.25, lw=1.18, hip=1.2, arm=1.06, robe=1.15))


# =====================================================================
# 🐉 용 타기 (옆모습, 오른쪽으로) — 날개 퍼덕 3칸, 해그리드는 작게 (RIDE_K) 용 등에 앉음
# =====================================================================
RIDE_K = 0.6  # 탈것에 앉은 해그리드 배율 (용·오토바이가 칸을 채움)
WING_TIPS = [(20.0, 14.0), (5.0, 50.0), (22.0, 98.0)]  # 날개 끝: 위 · 가운데 · 아래


def wing(root, tip, back, col=WING, k=1.0):
    """박쥐 날개: 어깨(root) → 끝(tip) 뼈, 뒤 (back)로 물결치는 막 (뼈 셋)."""
    pts = [root, tip]
    n = 3
    for j in range(1, n + 1):
        t = j / (n + 1)
        p = (tip[0] + (back[0] - tip[0]) * t, tip[1] + (back[1] - tip[1]) * t)
        q = (p[0] + (root[0] - p[0]) * 0.18, p[1] + (root[1] - p[1]) * 0.18)
        pts += [q, p] if j < n else [q]
    pts.append(back)
    bones = [L([root, tip], 2.0 * k)] + [L([root, (tip[0] + (back[0] - tip[0]) * j / (n + 1), tip[1] + (back[1] - tip[1]) * j / (n + 1))], 1.2 * k)
                                          for j in range(1, n + 1)]
    return Part([T(*pts), L([root, tip], 3.0 * k)], col, [(bones, WING_BONE)], outline=True, grow=0)


def dragon_parts(i):
    """용 Part 함수들: (맨 뒤 = 먼 날개·꼬리, 해그리드 먼 다리와 가까운 다리 사이 = 몸·목·머리·가까운 날개). i = 날개 칸."""
    bob = [0.0, 1.5, 2.5][i]
    cx, cy = 60.0, 92.0 + bob
    tx, ty = WING_TIPS[i]

    def far_wing():
        """먼 쪽 날개 (조금 어둡게)."""
        return wing((70, cy - 10), (tx + 6, ty + bob - 4), (cx - 24, cy - 6), (128, 176, 100))

    def tail():
        """꼬리: 몸 뒤에서 아래로 휘었다 올라가는 굵은 꼬리 + 끝 세모."""
        pts = [(cx - 24, cy + 2), (cx - 40, cy + 8), (cx - 52, cy + 4 - bob), (cx - 56, cy - 4 - bob)]
        tipp = pts[-1]
        return Part([L(pts, 7.0), T((tipp[0] - 6, tipp[1] - 2), (tipp[0] + 3, tipp[1] - 8), (tipp[0] + 3, tipp[1] + 3))], DRAGON,
                    [([L(pts[:2], 2.0)], DRAGON_DARK)], outline=True, grow=0)

    def body():
        """몸 (구릿빛 배) · 접은 다리 둘 · 목 · 머리 (주둥이·뿔·금빛 눈·콧구멍) · 등 가시."""
        head = (106.0, 58.0 + bob * 0.5)
        neck = [(cx + 20, cy - 6), (cx + 34, cy - 18), (head[0] - 8, head[1] + 4)]
        shp = [E(cx - 30, cy - 14, cx + 30, cy + 14), L(neck, 13.0), E(head[0] - 12, head[1] - 9, head[0] + 10, head[1] + 9),
               E(head[0] + 2, head[1] - 4, head[0] + 18, head[1] + 7),
               L([(cx + 10, cy + 8), (cx + 16, cy + 18), (cx + 6, cy + 20)], 6.0), L([(cx - 14, cy + 8), (cx - 8, cy + 18), (cx - 18, cy + 20)], 6.0)]
        horns = [T((head[0] - 8, head[1] - 6), (head[0] - 18, head[1] - 16), (head[0] - 4, head[1] - 9)),
                 T((head[0] - 2, head[1] - 8), (head[0] - 8, head[1] - 19), (head[0] + 3, head[1] - 9))]
        spines = [T((cx - 22 + 9 * j, cy - 12 + 0.6 * j), (cx - 18 + 9 * j, cy - 19), (cx - 14 + 9 * j, cy - 12)) for j in range(0, 2)]
        marks = [([E(cx - 26, cy - 2, cx + 26, cy + 14), L([(cx + 20, cy), (cx + 33, cy - 12), (head[0] - 8, head[1] + 9)], 6.0)], DRAGON_BELLY),
                 ([L([(cx - 18 + 7 * j, cy + 2), (cx - 16 + 7 * j, cy + 12)], 1.0) for j in range(6)], (184, 136, 72)),
                 ([C(head[0] + 1, head[1] - 2, 2.4)], EYE_GOLD), ([C(head[0] + 1.6, head[1] - 2, 1.2)], wz.OUT),
                 ([C(head[0] + 15, head[1] + 0.5, 1.0)], DRAGON_DARK), ([L(dp.arc(head[0] + 8, head[1] + 4, 7, 2, 10, 120, 6), 1.1)], DRAGON_DARK)]
        return Part(shp + horns + spines, DRAGON, marks + [(horns, HORN)], outline=True, grow=0, spikes=horns + spines)

    def near_wing():
        """가까운 쪽 날개 (몸 앞, 해그리드 뒤)."""
        return wing((76, cy - 8), (tx + 16, ty + bob + 4), (cx - 18, cy - 2))

    return [far_wing, tail], [body, near_wing]


def ride_frame(hip, *, legs, arms, rot, behind=(), mid=(), after=(), fx=(), robe=None, grip_at=None):
    """탈것에 앉은 작은 해그리드 (RIDE_K배, 엉덩이 hip 기준 — 탈것 Part는 칸 좌표로 미리 만들어 둠): 다리·팔 자리는 해그리드 크기 좌표 (hip에서).
    behind: 맨 뒤 · mid: 먼 다리와 가까운 다리 사이 · after: 맨 앞 Part · grip_at: 이 칸 자리에 mark_grip (사이드카 자리)."""
    lx = lambda dx, dy: (hip[0] + dx, hip[1] + dy)  # noqa: E731
    if grip_at:
        dp.mark_grip(*grip_at)
    return wz.wizard(hip=hip, rot=rot, head_rot=-6, legs=[lx(*legs[0]), lx(*legs[1])], leg_bend=[(1, -0.6), (1, -0.6)],
                     arms=[lx(*arms[0]), lx(*arms[1])], arm_bend=[(0, 1), (0, 1)], eyes="happy", mouth="big", look_=(1.0, -0.2),
                     robe=robe or dict(a=60, len=0.8), whole=dict(sx=RIDE_K, sy=RIDE_K, pivot=hip), behind=[(lambda p=p: p) for p in behind],
                     mid=[(lambda p=p: p) for p in mid], after=list(after), fx=list(fx), point=True)


def hg_dragon(sp, i):
    """🐉 용 타고 날기 (3칸 반복): 초록 용이 날개를 위 · 가운데 · 아래로 퍼덕이고, 해그리드는 등에 걸터앉아 앞으로 숙여 목의 뿔을 잡음 —
    코트가 뒤로 휘날리고 신난 얼굴, 바람 자국."""
    s = wz.ws()
    back, mid = dragon_parts(i)
    hip = (56.0, 80.0 + [0.0, 1.5, 2.5][i])
    flap = [2.0, -2.0, 0.8][i]
    fx = [([L([(4 + d, y), (18 + d, y)], 1.6)], wz.WIND, 0) for y, d in ((30, [0, -3, -1][i]), (64, [-2, 0, -3][i]))]
    with dp.xf(sx=0.92, pivot=(62.0, 100.0)):  # 머리부터 꼬리까지 칸 안에 (1px 여백)
        back, mid = [f() for f in back], [f() for f in mid]
    return ride_frame(hip, legs=[(s["leg"] * 0.5, s["leg"] * 0.7), (s["leg"] * 0.6, s["leg"] * 0.75)],
                      arms=[(s["arm"] * 1.3, -s["arm"] * 0.25), (s["arm"] * 1.4, -s["arm"] * 0.15)], rot=14, behind=back,
                      mid=mid, fx=fx,
                      robe=dict(a=[62, 70, 56][i], flap=flap, len=0.85))


# =====================================================================
# 🏍️ 사이드카 오토바이 (옆모습, 오른쪽으로) — 해그리드는 작게, 가까운 쪽 사이드카에 다른 펫이 앉음 (grip)
# =====================================================================
REAR, FRONT_W, R = (28.0, wz.FLOOR - 13.0), (70.0, wz.FLOOR - 13.0), 13.0
SIDE_W, SIDE_R = (104.0, wz.FLOOR - 9.0), 9.0
SEAT_AT = (96.0, wz.FLOOR - 24.0)  # 사이드카 가운데 (통 윗테 = 이보다 10 위 — grip: 앉은 펫은 그 아래로 가려짐)


def wheel(c, r, spin):
    """굵은 타이어 바퀴 하나 (spin: 굴러간 각도 — 바퀴살이 함께 돎)."""
    marks = [([C(c[0], c[1], r * 0.62)], RIM), ([C(c[0], c[1], r * 0.48)], BIKE)]
    for j in range(3):
        a = math.radians(spin + 60 * j + 30)
        d = (math.sin(a) * r * 0.5, -math.cos(a) * r * 0.5)
        marks.append(([L([(c[0] - d[0], c[1] - d[1]), (c[0] + d[0], c[1] + d[1])], 1.5)], RIM))
    marks.append(([C(c[0], c[1], r * 0.2)], HUB))
    return Part([C(c[0], c[1], r)], TYRE, marks, outline=True, grow=0)


def bike_parts(spin, lift=0.0):
    """오토바이 Part 함수들: (맨 뒤 = 뒷바퀴·앞바퀴·차체, 해그리드 사이 = 없음, 맨 앞 = 사이드카 통·바퀴). lift: 바퀴를 접어 올린 몫 (날 때)."""
    def wheels():
        """두 바퀴."""
        return wheel((REAR[0], REAR[1] - lift), R, spin)

    def wheel2():
        """앞바퀴."""
        return wheel((FRONT_W[0], FRONT_W[1] - lift), R, spin + 17)

    def body():
        """차체: 흙받이 → 안장 → 연료통 → 전조등, 핸들, 배기관."""
        y = wz.FLOOR - 2 * R - lift
        pts = [(REAR[0] - R * 0.9, REAR[1] - R * 0.3 - lift), (REAR[0] - R * 0.4, y + 3), (44, y - 1), (58, y - 3), (FRONT_W[0] + 4, y - 2),
               (FRONT_W[0] + R * 0.6, y + 6), (FRONT_W[0] - 2, FRONT_W[1] - R * 0.3 - lift), (48, wz.FLOOR - R + 4 - lift),
               (REAR[0] + R * 0.6, REAR[1] - lift)]
        marks = [([L([(36, y + 1), (50, y + 1)], 4.0)], (40, 36, 44)), ([L([(REAR[0] - R * 0.5, y + 5), (FRONT_W[0], y + 2)], 1.4)], BIKE_HI),
                 ([E(FRONT_W[0] + R * 0.25, y + 2, FRONT_W[0] + R * 0.75, y + 7)], LAMP)]
        bar = [(FRONT_W[0] - 6, y - 1), (FRONT_W[0] - 10, y - 12)]
        pipe = L([(42, wz.FLOOR - R + 2 - lift), (REAR[0] - R * 0.6, wz.FLOOR - R - 2 - lift)], 3.2)
        fork = L([(FRONT_W[0], FRONT_W[1] - lift), (FRONT_W[0] - 6, y - 1)], 3.4)
        return Part([T(*pts), L(bar, 3.0), pipe, fork],
                    BIKE, marks + [([pipe], RIM)], outline=True, grow=0)

    def tub():
        """사이드카: 앞이 둥근 통 (윗테 줄·리벳) + 작은 바퀴."""
        y0 = SEAT_AT[1] - 10 - lift
        pts = [(SEAT_AT[0] - 20, y0), (SEAT_AT[0] + 14, y0 - 2), (SEAT_AT[0] + 24, y0 + 6), (SEAT_AT[0] + 22, wz.FLOOR - 10 - lift),
               (SEAT_AT[0] - 18, wz.FLOOR - 10 - lift), (SEAT_AT[0] - 24, y0 + 6)]
        marks = [([L([(SEAT_AT[0] - 21, y0 + 1.5), (SEAT_AT[0] + 16, y0 - 0.5)], 2.2)], TUB_HI)]
        marks += [([C(SEAT_AT[0] - 12 + 9 * j, y0 + 8, 1.0) for j in range(4)], TUB_HI)]
        return Part([T(*pts)], TUB, marks, outline=True, grow=0)

    def side_wheel():
        """사이드카 바퀴."""
        return wheel((SIDE_W[0], SIDE_W[1] - lift), SIDE_R, spin + 40)

    return [wheels, wheel2, body], [side_wheel, tub]


def puff_fx(x, y, k):
    """배기 연기 퐁 (흰 동그라미 셋)."""
    return [([C(x, y, 3.2 * k), C(x - 5 * k, y - 2 * k, 2.4 * k), C(x - 9 * k, y + 1, 1.8 * k)], PUFF, 1.0)]


def hg_sidecar(sp, i):
    """🏍️ 사이드카 오토바이 (3칸 반복): 바퀴가 굴러가고 해그리드가 두 손으로 핸들을 잡고 신남, 코트가 뒤로 펄럭, 배기 연기 퐁 · 속도선.
    사이드카 자리 = grip (다른 펫이 앉음)."""
    fx = puff_fx(10 - 2 * i, wz.FLOOR - 16 - i, 0.8 + 0.15 * i) + dp.speed_fx(2, (52, 70), 8, [0, -2, -1][i])
    return _ride_bike(i, 0.0, 40.0 * i, 0.0, fx)


def hg_sidecar_fly(sp, i):
    """🏍️ 오토바이째 붕 날기 (2칸 반복): 앞이 살짝 들리고 바퀴 아래로 구름 퐁, 코트가 위로 펄럭 — 사이드카 자리 = grip."""
    fx = puff_fx(30 + 6 * i, wz.FLOOR - 3, 1.0) + puff_fx(86 - 4 * i, wz.FLOOR - 1, 0.9) + dp.speed_fx(2, (40, 60), 8, -i)
    return _ride_bike(i, 4.0, 20.0 * i, -8.0, fx)


def _ride_bike(i, lift, spin, rot, fx):
    """오토바이 한 칸: 뒤 (바퀴·차체) → 작은 해그리드 (안장 위, 두 손으로 핸들) → 사이드카 (통·바퀴), rot도 기울임 (뒷바퀴 기준)."""
    s = wz.ws()
    back, front_ = bike_parts(spin, lift)
    hip = (44.0, wz.FLOOR - 2 * R - 4 - lift + [0.0, -0.6, 0.0][i])
    bar = (FRONT_W[0] - 10, wz.FLOOR - 2 * R - 12 - lift)
    hand = ((bar[0] - hip[0]) / RIDE_K, (bar[1] - hip[1]) / RIDE_K)
    with dp.xf(rot=rot, pivot=(REAR[0], wz.FLOOR)):
        return ride_frame(hip, legs=[(s["leg"] * 0.6, s["leg"] * 0.8), (s["leg"] * 0.75, s["leg"] * 0.85)], arms=[(hand[0] - 3, hand[1]), hand],
                          rot=10, behind=[f() for f in back], after=[f() for f in front_], fx=fx,
                          robe=dict(a=[58, 64, 54][i % 3], flap=[1.5, -1.5, 0.6][i % 3], len=0.8), grip_at=(SEAT_AT[0], SEAT_AT[1] - 9 - lift))


# =====================================================================
# 🤗 안기·쓰담 (옆모습) — 두 팔로 가슴 앞에 펫을 안음 (손 = grip, 펫은 앱이 그 자리에)
# =====================================================================
def hug_arms(hip, rot=4.0):
    """가슴 앞에 모은 두 손 (안은 펫의 발 자리)."""
    s = wz.ws()
    sh = wz.shoulders("side", hip, rot)
    return [wz.reach(sh[0], (sh[0][0] + s["arm"] * 0.95, sh[0][1] + s["arm"] * 0.55), s["arm"], 1.0),
            wz.reach(sh[1], (sh[1][0] + s["arm"] * 1.0, sh[1][1] + s["arm"] * 0.6), s["arm"], 1.0)]


def hg_hug(sp, i):
    """🤗 안고 서기 (2칸 반복): 두 팔을 가슴 앞에 모아 펫을 꼭 안음 (가까운 손 = grip), 눈 ^^, 살랑 숨쉬기 · 작은 하트."""
    hip = wz.H(dy=[0, 0.8][i])
    fx = [heart_fx(100, 24 - 2 * i, 3.6)] if i else []
    return wz.wizard(hip=hip, rot=4, head_rot=8, arms=hug_arms(hip), arm_bend=[(0, 1), (0, 1)], far_limbs_front=False, eyes="happy", mouth="smile",
                     robe=dict(a=6, flap=0.6 * i), grip="near", fx=fx)


def hg_hug_walk(sp, i):
    """🤗 안고 걷기 (4칸 반복): 총총 걸으며 두 팔은 가슴 앞에 펫을 안은 채 (가까운 손 = grip), 코트 자락 살랑."""
    hip, legs = wz.walk_legs(i)
    return wz.wizard(hip=hip, legs=legs, rot=3, head_rot=6, arms=hug_arms(hip, 3), arm_bend=[(0, 1), (0, 1)], eyes="happy", mouth="smile",
                     robe=dict(a=[8, 12, 8, 12][i], flap=[0, 1.2, 0, -1.2][i]), grip="near")


def hg_pat(sp, i):
    """🖐️ 쓰담쓰담 (3칸 반복): 허리를 숙여 가까운 손바닥을 펴고 앞 아래 (작은 펫 머리 높이)를 톡톡, 먼 손엔 우산, 하트가 퐁."""
    s = wz.ws()
    hip = wz.H(wz.SIDE_CX - 8, s["leg"] * 0.1)
    rot = 26.0
    sh = wz.shoulders("side", hip, rot)
    hand = wz.reach(sh[1], (sh[1][0] + s["arm"] * 1.2, sh[1][1] + s["arm"] * [0.25, 0.45, 0.3][i]), s["arm"], 1.0)
    fx = [heart_fx(hand[0] + 12, 26 - 3 * i, 3.4 + i * 0.6)]
    return wz.wizard(hip=hip, rot=rot, head_rot=10, legs=[wz.floor_at(hip[0] - 6), wz.floor_at(hip[0] + 8)], arms=[("a", 20, 0.95), hand],
                     arm_bend=[(1, 0.4), (0, 1)], open_hand=(False, True), eyes="happy", mouth="smile", robe=dict(a=12), fx=fx,
                     wand=dict(hand=0, a=30, k=0.8))


# =====================================================================
# 🐉 용알 품기 (옆모습) — 바닥에 앉아 두 손을 오므려 무릎 위에 알 (알·아기 용은 앱 덮개 창 — 손 = grip)
# =====================================================================
def hg_egg(sp, i):
    """🥚 알 품기 (4칸 반복, 천천히): 바닥에 앉아 무릎 위에 두 손을 오므려 (grip — 앱이 알을 그림) 들여다보며 살랑 흔들어 줌, 따뜻한 김."""
    s = wz.ws()
    hip = wz.sit_hip(wz.SIDE_CX - 12)
    knee_x = hip[0] + s["leg"] * 0.55 + 4
    legs = [(knee_x + 4, wz.foot_y()), (knee_x + 7, wz.foot_y())]
    sway = [0.0, 1.5, 0.0, -1.5][i]
    rot = -4 + sway
    sh = wz.shoulders("side", hip, rot)
    cup = (knee_x + 14 + sway, hip[1] - s["leg"] * 0.3 - 2)
    arms = [wz.reach(sh[0], (cup[0] - 4, cup[1] + 2), s["arm"], 1.0), wz.reach(sh[1], (cup[0] + 3, cup[1] + 3), s["arm"], 1.0)]
    fx = [([L(dp.arc(cup[0] + 8 + dx, cup[1] - 22 - 2 * ((i + j) % 2), 2.0, 3.0, 90, 450, 10)[:6], 1.2)], WARM, 0) for j, dx in enumerate((-5, 3))]
    return wz.wizard(hip=hip, rot=rot, head_rot=6, head_d=(2, 1.5), legs=legs, leg_bend=[(0.4, -1), (0.4, -1)], arms=arms, arm_bend=[(0, 1), (0, 1)],
                     eyes="happy" if i % 2 else "dot", look_=(0.8, 1.6), mouth="smile", robe=dict(a=-36, len=0.4, floor=wz.FLOOR - 1), grip="near",
                     fx=fx)


# =====================================================================
# 🪨 락 케이크 (옆모습, 한 번) — 코트 안주머니에서 꺼내 → 내밀고 → 쪼그려 바닥에 내려놓음 (그 자리에 앱이 돌 물건을 둠)
# =====================================================================
def hg_offer(sp, i):
    """🪨 "Rock cake?" (한 번, 마지막 칸 유지): 0 = 가까운 손을 코트 안주머니에 넣어 뒤적 · 1 = 락 케이크를 손바닥에 얹어 앞으로 내밂 ·
    2 = 쪼그려 앞 바닥에 내려놓고 손을 뗌 (케이크는 앱의 돌 물건이 그 자리에 나옴 — 그림엔 없음)."""
    s = wz.ws()
    if i == 0:
        hip = wz.H()
        sh = wz.shoulders("side", hip, 2)
        hand = wz.reach(sh[1], (sh[1][0] + 4, sh[1][1] + s["th"] * 0.35), s["arm"], 1.0)
        return wz.wizard(hip=hip, rot=2, head_rot=12, arms=[("a", 14, 0.95), hand], arm_bend=[(1, 0.4), (0, 1)], eyes="dot", look_=(0.4, 1.6),
                         mouth="o", robe=dict(a=6))
    if i == 1:
        hip = wz.H()
        sh = wz.shoulders("side", hip, 2)
        hand = wz.reach(sh[1], (sh[1][0] + s["arm"] * 1.05, sh[1][1] + s["arm"] * 0.2), s["arm"], 1.0)
        return wz.wizard(hip=hip, rot=2, head_rot=-2, arms=[("a", 14, 0.95), hand], arm_bend=[(1, 0.4), (0, 1)], open_hand=(False, True),
                         eyes="happy", mouth="big", robe=dict(a=6), fx=cake_fx(hand[0] + 1, hand[1] - 5, 5.5))
    hip = wz.H(wz.SIDE_CX - 10, s["leg"] * 0.4)
    rot = 26.0
    sh = wz.shoulders("side", hip, rot)
    hand = wz.reach(sh[1], (sh[1][0] + s["arm"] * 0.8, wz.FLOOR - 6), s["arm"], 1.0)
    return wz.wizard(hip=hip, rot=rot, head_rot=-4, legs=[wz.floor_at(hip[0] - 6), wz.floor_at(hip[0] + 10)], leg_bend=[(1, -0.4), (1, -0.4)],
                     arms=[("a", 30, 0.9), hand], arm_bend=[(1, 0.4), (0, 1)], open_hand=(False, True), eyes="happy", mouth="smile",
                     robe=dict(a=10, floor=wz.FLOOR - 1), fx=sparkle_fx(hand[0] + 10, hand[1] - 12, 3.0))


HAGRID_ANIMS = ["dragon", "sidecar", "sidecar_fly", "hug", "hug_walk", "pat", "egg", "offer"]  # = Rust manifest::HAGRID_ANIMS
HAGRID_POSES = wz.poses_for(HAGRID_LOOK, {"dragon": hg_dragon, "sidecar": hg_sidecar, "sidecar_fly": hg_sidecar_fly, "hug": hg_hug,
                                          "hug_walk": hg_hug_walk, "pat": hg_pat, "egg": hg_egg, "offer": hg_offer})
SIZE = 1.2  # 다른 마법사 (0.85)의 약 1.4배

HAGRID = Species(
    id="hagrid", name="해그리드", walk_speed=76, scale=1.0, base=COAT, back=COAT, belly=COAT, nose=wz.OUT, mouth="w",
    front_tail_color=COAT, tail_color=COAT, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + wz.PRANK_ANIMS + HAGRID_ANIMS, stage_size=dict.fromkeys(STAGES, SIZE), stripe=COAT_IN,
    poses=HAGRID_POSES, outline=wz.OUT,
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 해그리드를 등록함."""
    dp.SPECIES[HAGRID.id] = HAGRID


register()


def preview(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hungry", 0), ("roll", 0),
             ("dragon", 0), ("sidecar", 0), ("hug", 0), ("pat", 1), ("egg", 0), ("offer", 1), ("climb", 0), ("idle", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "dragon"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(HAGRID, root / "ui" / "pets" / HAGRID.id))
    preview(by_stage, pv / "stages_hagrid.png")
    dp.preview_back(by_stage, pv / "climb_hagrid.png")
    wz.preview_rows(by_stage, pv / "wizard_hagrid.png", HAGRID_ANIMS + ["climb", "slide"])
    print("ok: hagrid")


if __name__ == "__main__":
    main()
