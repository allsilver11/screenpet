"""디멘터 👻 — 사람 모양 치비가 아닌, 떠다니는 너덜너덜한 까만 후드 망토 펫(dementor): 얼굴 없이 캄캄한 후드 속, 소매 끝의 앙상한 회색 해골 손,
들쭉날쭉 찢어진 아랫단이 펄럭임 (발 없음 — 늘 바닥보다 조금 떠 있음). 나는 동물 (pet.json locomotion fly) — 벽 타기 없음.

    python tools/draw_dementor.py → ui/pets/dementor/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_dementor.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

생김새: 큰 둥근 후드 (뒤쪽 끝이 뾰족) + 아래로 퍼지는 망토 + 종 모양 소매 둘 + 뼈 손가락 셋. 얼굴은 그리지 않음 (후드 속은 까만 어둠 —
       표정은 💢·?·z·하트·반짝이 같은 효과로만). 단계 (DM): 아기 = 후드가 아주 크고 망토가 짧은 동글 꼬마 → 어른 = 망토가 길고 아랫단이 더 너덜너덜.
특기 (코어 brain_dementor.rs): 곁의 펫 시무룩·화면 구석 🥶 서리 (앱 덮개 창)·🦌 패트로누스에 도망 — 시트에 따로 그릴 동작은 없음.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import C, E, L, T, Part, Species, _ST, anger_fx, question_fx, sparkle_fx, view, xf, z_fx  # noqa: E402
from draw_spider import polar  # noqa: E402
from sprite_spec import ROLL_CENTER, ROLL_RADIUS, STAGES  # noqa: E402

CLOAK = (46, 46, 56)  # 망토
CLOAK_DARK = (30, 30, 38)  # 그늘·먼 쪽 소매
CLOAK_HI = (70, 70, 84)  # 주름 윤기
VOID = (8, 8, 12)  # 후드 속 어둠
VOID_RIM = (20, 20, 28)
BONE, BONE_DARK = (178, 182, 176), (128, 132, 126)  # 해골 손
OUT = (14, 14, 22)  # 외곽선
MIST = (196, 206, 226)  # 차가운 입김
ICE, ICE_HI = (176, 214, 246), (236, 246, 255)  # 🍧 빙수 생각 풍선
SYRUP = (232, 96, 120)
HOVER = 112.0  # 찢어진 아랫단 조각 끝 (바닥 125보다 떠 있음)

# 단계(lvl)별 — hr: 후드 반지름 · body: 목에서 아랫단까지 · hw: 아랫단 반폭 · sw: 어깨 반폭 · arm: 소매+손 길이 · teeth: 아랫단 조각 수 · rag: 조각 깊이
DM = {
    0: dict(hr=27.0, body=36.0, hw=25.0, sw=15.0, arm=15.0, teeth=4, rag=7.0),
    1: dict(hr=26.0, body=42.0, hw=27.0, sw=15.5, arm=17.0, teeth=5, rag=8.5),
    2: dict(hr=25.0, body=48.0, hw=29.0, sw=16.0, arm=19.0, teeth=5, rag=10.0),
    3: dict(hr=24.0, body=54.0, hw=31.0, sw=16.5, arm=21.0, teeth=6, rag=11.0),
    4: dict(hr=23.0, body=60.0, hw=33.0, sw=17.0, arm=23.0, teeth=7, rag=12.0),
}


def ds():
    """지금 단계의 디멘터 치수."""
    return DM[_ST["lvl"]]


def neck_y(dy=0.0):
    """망토 목 (후드 아래) y — 아랫단 조각 끝이 HOVER에 오게."""
    return HOVER - 1.1 * ds()["rag"] - ds()["body"] + dy


def ragged(xr, xl, y, phase, depth, flow=0.0):
    """오른쪽 xr에서 왼쪽 xl까지 들쭉날쭉 찢어진 아랫단 점들 (뾰족 조각이 아래로, 칸마다 phase로 펄럭, flow: 뒤(왼쪽)로 끌림)."""
    n = ds()["teeth"]
    pts = []
    for k in range(2 * n + 1):
        t = k / (2 * n)
        x = xr + (xl - xr) * t - flow * t * t
        if k % 2:
            yy = y + depth * (0.75 + 0.35 * math.sin(phase + k * 1.7))
            x += 1.5 * math.sin(phase * 1.3 + k)
        else:
            yy = y - depth * (0.35 + 0.15 * math.cos(phase + k))
        pts.append((x, yy - flow * 0.25 * t))
    return pts


def hand(wrist, a, open_=True, curl=0.0):
    """앙상한 해골 손 (손목에서 a도 쪽): 작은 손바닥 + 마디진 가는 손가락 셋 (curl: 안으로 굽힘)."""
    s = ds()
    palm = polar(wrist, a, 2.4)
    shapes_ = [C(palm[0], palm[1], 2.6)]
    marks = []
    spread = (-26, 0, 26) if open_ else (-10, 0, 10)
    for da in spread:
        k1 = polar(palm, a + da, s["arm"] * 0.3)
        k2 = polar(k1, a + da + curl, s["arm"] * 0.26)
        shapes_.append(L([palm, k1, k2], 1.8))
        marks.append(([C(k1[0], k1[1], 0.9)], BONE_DARK))
    return Part(shapes_, BONE, marks, outline=True, grow=0, sharp=True)


def sleeve(shoulder, a, ln, color, flap=0.0):
    """종 모양 소매 (어깨에서 a도 쪽으로 ln — 끝이 넓고 너덜너덜). 손목 자리도 돌려줌."""
    w0, w1 = 4.0, 7.5
    end = polar(shoulder, a, ln)
    nx, ny = math.cos(math.radians(a)), -math.sin(math.radians(a))
    p = [(shoulder[0] + nx * w0, shoulder[1] + ny * w0), (end[0] + nx * w1, end[1] + ny * w1)]
    tips = []
    for k, f in ((0.6, 1.0), (0.0, 0.6), (-0.6, 1.0)):
        tips.append(polar((end[0] + nx * w1 * k, end[1] + ny * w1 * k), a + flap * 6, 3.0 * f))
    p += tips
    p += [(end[0] - nx * w1, end[1] - ny * w1), (shoulder[0] - nx * w0, shoulder[1] - ny * w0)]
    wrist = polar(shoulder, a, ln * 0.92)
    return Part([T(*p)], color, [([L([polar(shoulder, a, ln * 0.3), polar(shoulder, a, ln * 0.85)], 1.0)], CLOAK_HI)], outline=True, grow=0), wrist


def hood(view_, cx, cy, rot=0.0):
    """후드 (가운데 cx, cy — 뒤쪽 끝이 뾰족, 속은 캄캄) + 머리 표시. 옆모습은 오른쪽을 봄."""
    r = ds()["hr"]
    with xf(rot=rot, pivot=(cx, cy + r * 0.8)):
        if view_ == "side":
            tip = T((cx - r * 0.55, cy - r * 0.8), (cx - r * 1.3, cy - r * 0.25), (cx - r * 0.85, cy + r * 0.2))
            shapes_ = [E(cx - r, cy - r, cx + r, cy + r * 0.95), tip]
            void = E(cx + r * 0.05, cy - r * 0.55, cx + r * 0.98, cy + r * 0.78)
            inner = E(cx + r * 0.25, cy - r * 0.38, cx + r * 0.9, cy + r * 0.62)
            fold = L(dp.arc(cx - r * 0.1, cy, r * 0.7, r * 0.8, 200, 260, 8), 1.2)
        else:
            tip = T((cx - r * 0.28, cy - r * 0.86), (cx, cy - r * 1.22), (cx + r * 0.28, cy - r * 0.86))
            shapes_ = [E(cx - r, cy - r, cx + r, cy + r * 0.95), tip]
            void = E(cx - r * 0.64, cy - r * 0.52, cx + r * 0.64, cy + r * 0.8)
            inner = E(cx - r * 0.48, cy - r * 0.34, cx + r * 0.48, cy + r * 0.66)
            fold = L(dp.arc(cx, cy, r * 0.82, r * 0.85, 205, 250, 8), 1.2)
        dp.mark_head(cx - (r * 0.15 if view_ == "side" else 0.0), cy - r + 2, r * 1.6)
        marks = [([fold], CLOAK_HI), ([void], VOID_RIM), ([inner], VOID)]
        return Part(shapes_, CLOAK, marks, outline=True, grow=0)


TOP_ROOM = 3.0  # 칸 맨 위에서 이만큼(128 칸 px)은 비움 — 큰 단계의 후드·번쩍 든 손이 칸 위를 넘어 잘리지 않게
SINK_STEP = 2.0  # 위가 닿으면 이만큼씩(그림 단위) 내려 다시 그림
SINK_MAX = 12  # 내려 그리기 최대 횟수


def dementor(view_="side", *, dy=0.0, **kw):
    """디멘터 한 장 (_dementor). 몸(효과 빼고)이 칸 맨 위 TOP_ROOM 안까지 닿으면 닿지 않을 때까지 조금씩 내려 다시 그림 —
    떠다니는 몸이라 아래(아랫단~바닥)에 틈이 있어서, 커진 단계·솟는 칸도 후드 꼭대기가 잘리지 않음."""
    u = dp.out_u()
    # 몸만 재 봄 (생각 풍선·z 같은 효과는 원래 칸 위쪽에 띄우는 것)
    body_only = {**kw, "fx": ()}
    for _ in range(SINK_MAX):
        bbox = _dementor(view_, dy=dy, **body_only).getchannel("A").getbbox()
        if bbox is None or bbox[1] >= TOP_ROOM * u:
            break
        dy += SINK_STEP
    return _dementor(view_, dy=dy, **kw)


def _dementor(view_="side", *, dy=0.0, rot=0.0, hood_rot=0.0, flow=0.0, phase=0.0, arms=None, curl=(0.0, 0.0), open_=(True, True),
             fx=(), held=(), whole=None, hem_up=0.0):
    """디멘터 한 장. view_: side (오른쪽을 봄) · front · dy: 위아래 (떠 있는 높이) · rot: 몸 기울기 (목 기준, 시계 방향) · hood_rot: 고개 ·
    flow: 아랫단이 뒤로 끌리는 정도 · phase: 아랫단 펄럭 · arms: [먼/왼, 가까운/오른] 소매 각도 (0 = 아래, 90 = 앞/오른쪽, 180 = 위) ·
    curl: 손가락 굽힘 · fx: 효과 · held: 몸과 손 사이 Part 함수 · whole: 통째 변환 · hem_up: 아랫단을 위로 걷어 올림 (떨어질 때)."""
    s = ds()
    side = view_ == "side"
    cx = 60.0 if side else 64.0
    ny = neck_y(dy)
    r = s["hr"]
    if arms is None:
        arms = [10.0, 22.0] if side else [-14.0, 14.0]
    with xf(**(whole or {})):
        with xf(rot=rot, pivot=(cx, ny)):
            hem_y = ny + s["body"] - hem_up
            hw = s["hw"] * (0.92 if side else 1.0)
            if side:
                back = cx - hw * 0.9 - flow * 0.4
                front = cx + hw * 0.75 - flow * 0.15
            else:
                back, front = cx - hw, cx + hw
            body_pts = [(cx - s["sw"], ny - 2), (cx + s["sw"], ny - 2), (front, hem_y - s["rag"] * 0.4)]
            body_pts += ragged(front, back, hem_y, phase, s["rag"], flow if side else 0.0)
            body_pts += [(back, hem_y - s["rag"] * 0.4)]
            folds = []
            for k in (-0.45, 0.0, 0.45) if not side else (-0.3, 0.25):
                x0 = cx + k * s["sw"]
                folds.append(L([(x0, ny + 6), (x0 + k * hw * 0.7 - (flow * 0.3 if side else 0), hem_y - s["rag"] * 0.3)], 1.3))
            body = Part([T(*body_pts)], CLOAK, [(folds, CLOAK_DARK)], outline=True, grow=0)
            sh = [(cx - s["sw"] * 0.75, ny + 4), (cx + s["sw"] * 0.75, ny + 4)] if not side else [(cx - 2, ny + 4), (cx + 3, ny + 5)]
            sleeves, hands = [], []
            for j in range(2):
                far = side and j == 0
                sv, wrist = sleeve(sh[j], arms[j], s["arm"] * 0.72, CLOAK_DARK if far else CLOAK, phase)
                sleeves.append(sv)
                hands.append(hand(wrist, arms[j], open_[j], curl[j] * (1 if (side or j == 1) else -1)))
        hd = hood(view_, cx + (2.0 if side else 0.0), ny - r * 0.8 + 3, rot + hood_rot)
        if side:
            parts = [sleeves[0], hands[0], body] + [h() for h in held] + [hd, sleeves[1], hands[1]]
        else:
            parts = [body, sleeves[0], sleeves[1]] + [h() for h in held] + [hd, hands[0], hands[1]]
    return dp.compose([Part([], CLOAK, grow=0)] + parts, [], list(fx))


def mist_fx(x, y, k=1.0):
    """차가운 입김 한 줄기 (후드 앞에서 흐르는 하얀 물결)."""
    return [([L([(x, y), (x + 4 * k, y - 2), (x + 8 * k, y), (x + 12 * k, y - 2)], 1.4)], MIST, 0)]


# =====================================================================
# 옆모습 (오른쪽을 봄)
# =====================================================================
def dm_walk(sp, i):
    """스르륵 떠서 가기: 몸을 앞으로 살짝 숙이고 아랫단이 뒤로 끌리며 펄럭, 두 손은 앞으로 뻗어 더듬음, 위아래로 출렁."""
    bob = [0, -1.5, -2.5, -1.0][i]
    return dementor(dy=bob, rot=8, flow=10 + 2 * (i % 2), phase=i * 1.6, arms=[62 + 4 * (i % 2), 74 - 4 * (i % 2)], curl=(20, 20))


def dm_run(sp, i):
    """빠르게 날기: 크게 숙이고 아랫단이 길게 뒤로, 두 손을 쭉 앞으로, 속도선."""
    return dementor(dy=[-2, -4, -2, -4][i], rot=18, flow=12, phase=i * 1.9, arms=[84, 92], curl=(8, 8), hood_rot=-6,
                    fx=dp.speed_fx(4, (64, 80, 96), 12, [0, -3, 0, -3][i]))


def dm_idle(sp, i):
    """제자리에 떠서 숨쉬기: 위아래로 살랑, 손은 아래로 늘어뜨림, 입김 한 줄기."""
    return dementor(dy=[0, -2][i], phase=i * 2.0, arms=[8, 18], curl=(30, 30), fx=mist_fx(88, 52 + [0, -2][i]))


def dm_look(sp, i):
    """두리번: 후드가 위 → 앞 → 아래 → 앞."""
    return dementor(hood_rot=[-12, 0, 12, 0][i], phase=i, arms=[8, 20], curl=(30, 30))


def dm_sleep(sp, i):
    """후드를 푹 숙이고 몸을 웅크려 둥실 잠: 소매로 몸을 감싸고 z."""
    return dementor(dy=[3, 4][i], rot=16, hood_rot=18, phase=i * 0.5, arms=[70, 60], curl=(60, 60), open_=(False, False),
                    fx=[z_fx(104, 30 - 3 * i, 4.0), z_fx(114, 18 - 3 * i, 2.6)])


def dm_annoyed(sp, i):
    """웅크리고 고개를 돌린 뾰로통 💢 (소매를 몸 앞에 모음)."""
    return dementor(dy=2, rot=-6, hood_rot=-10, phase=i, arms=[40, 46], curl=(70, 70), open_=(False, False),
                    fx=[anger_fx(100 + i, 26 - i, 5.0 + 0.6 * i)])


def dm_lookup(sp, i):
    """올려다보기: 후드를 뒤로 젖혀 위를 봄 (고개 들기 · 쭉 · ✧)."""
    return dementor(hood_rot=dp.lookup_tilt(i), arms=[8, 20], curl=(30, 30), phase=i, fx=dp.lookup_fx(i, 110, 14))


def dm_hop(sp, i):
    """쑥 솟았다 내려옴 (한 번): 웅크림 → 위로 길게 늘어나며 손 번쩍 → 내려옴."""
    dy = [4, -14, -6][i]
    return dementor(dy=dy, rot=[10, 0, 4][i], flow=[0, -6, 0][i], phase=i * 2, arms=[[30, 40], [150, 165], [60, 80]][i])


def dm_fall(sp, i):
    """떨어짐: 두 소매를 위로 허우적, 아랫단이 위로 펄럭."""
    return dementor(dy=-6, rot=[-6, 6][i], phase=i * 2.4, arms=[[150, 128], [128, 150]][i], hem_up=8, curl=(-20, -20))


def dm_dragged(sp, i):
    """후드 끝을 잡혀 대롱대롱: 소매가 축 늘어지고 살랑."""
    sway = [-6, 6][i]
    return dementor(dy=-4, arms=[2, 6], curl=(10, 10), phase=i, whole=dict(rot=sway * 0.5, pivot=(60, 14)), fx=dp.sweat_fx(100, 30))


def dm_land(sp, i):
    """내려앉음 (한 번): 납작 웅크렸다가 → 바로 둥실."""
    if i == 0:
        return dementor(whole=dict(sx=1.08, sy=0.9), dy=2, arms=[40, 50], curl=(30, 30), phase=1)
    return dementor(arms=[8, 18], curl=(30, 30))


def dm_roll(sp, i):
    """망토로 몸을 둘둘 만 공 (후드가 위, 손 하나): 회전 중심 ROLL_CENTER, 반지름 ROLL_RADIUS 원 안."""
    cx, cy = ROLL_CENTER
    with view(scale=1.0, lift=0):
        with xf(rot=[-10, 10][i], pivot=(cx, cy)):
            ball = Part([C(cx - 2, cy + 6, 26.0)], CLOAK, [([L(dp.arc(cx - 2, cy + 6, 18, 18, 30, 150, 10), 1.4)], CLOAK_DARK)], outline=True, grow=0)
            tips = Part([T((cx - 26, cy + 12), (cx - 34, cy + 24), (cx - 18, cy + 26)), T((cx + 4, cy + 30), (cx - 2, cy + 36), (cx + 12, cy + 32))],
                        CLOAK, outline=True, grow=0)
            r = 19.0
            hx, hy = cx + 6, cy - 12
            hd = Part([E(hx - r, hy - r, hx + r, hy + r), T((hx - r * 0.55, hy - r * 0.8), (hx - r * 1.3, hy - r * 0.25), (hx - r * 0.85, hy + r * 0.2))], CLOAK,
                      [([E(hx + r * 0.05, hy - r * 0.55, hx + r * 0.98, hy + r * 0.78)], VOID_RIM), ([E(hx + r * 0.25, hy - r * 0.38, hx + r * 0.9, hy + r * 0.62)], VOID)],
                      outline=True, grow=0)
            dp.mark_head(hx - 3, hy - r + 2, r * 1.6)
            hnd = Part([C(cx + 20, cy + 12, 2.6), L([(cx + 20, cy + 12), (cx + 25, cy + 8)], 1.6), L([(cx + 20, cy + 12), (cx + 26, cy + 13)], 1.6)],
                       BONE, outline=True, grow=0, sharp=True)
        clip = dp.shapes([C(cx, cy, ROLL_RADIUS - 1.25 * dp.OUTLINE * _ST["line"] - 1.2)])
    return dp.compose([tips, ball, hd, hnd], [], clip=clip)


# =====================================================================
# 앞모습
# =====================================================================
def front(**kw):
    """앞모습 디멘터."""
    return dementor("front", **kw)


def dm_sit_front(sp, i):
    """정면으로 둥실: 두 손을 몸 옆에 늘어뜨리고 살랑 (마지막 칸은 아래로 살짝)."""
    return front(dy=[0, -1, -2, -1][i], phase=i * 1.5, arms=[-10, 10], curl=(30, 30))


def dm_wave(sp, i):
    """한쪽 앙상한 손을 번쩍 들어 흔들흔들."""
    a = [140, 160, 140, 160][i]
    return front(phase=i, arms=[-12, a], curl=(30, [0, 14, 0, 14][i]))


def dm_tilt(sp, i):
    """후드를 갸웃 + ? (한 손은 후드 아래)."""
    return front(hood_rot=[8, 12, 12][i], arms=[-12, 150], curl=(30, 60), fx=[question_fx(104, 22 - i)])


def dm_belly(sp, i):
    """벌러덩: 뒤로 젖혀 두 소매를 위로 버둥, 아랫단이 위로 펄럭."""
    w = [0, 2, 0, -2][i]
    return front(dy=6, hem_up=10, phase=i * 2, arms=[-150 + 6 * w, 150 + 6 * w], whole=dict(sy=0.92), fx=sparkle_fx(100, 30, 4.0))


def dm_stretch(sp, i):
    """기지개 (한 번): 두 소매를 쭉 위로 → 옆으로 → 둥실."""
    if i == 0:
        return front(dy=-4, arms=[-165, 165], curl=(-10, -10))
    if i == 1:
        return front(arms=[-100, 100], curl=(0, 0), fx=mist_fx(70, 60))
    return front(arms=[-10, 10], curl=(30, 30))


def dm_heart(sp, i):
    """하트를 가슴께에 꼭 안음, 작은 하트가 떠오름."""
    hy = neck_y() + 14

    def held():
        """안은 하트."""
        return dp.held_heart(hy - 4)
    return front(arms=[-40, 40], curl=(-60, 60), held=[held], fx=dp.heart_burst(i))


def dm_tickle(sp, i):
    """간질간질: 몸을 비비 꼬며 소매로 몸을 감쌈, 반짝이."""
    return front(rot=[-8, 8, -3][i], arms=[-45, 45], curl=(-70, 70), open_=(False, False), phase=i * 2, fx=sparkle_fx(102, 28 + 2 * i, 3.5))


def ice_bubble_fx(cx, cy, r=12.0):
    """🍧 빙수 생각 풍선 (배고플 때): 하얀 그릇 위 얼음 산 + 분홍 시럽."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    fx += [([T((cx - 6, cy + 2), (cx + 6, cy + 2), (cx + 3.5, cy + 6.5), (cx - 3.5, cy + 6.5))], ICE, 1.0),
           ([E(cx - 6.5, cy - 5.5, cx + 6.5, cy + 3)], ICE_HI, 1.0), ([E(cx - 3, cy - 5, cx + 3, cy - 1)], SYRUP, 0)]
    return fx


def dm_hungry(sp, i):
    """배고파 축 처짐: 두 손을 몸 앞에 모으고 후드를 숙임, 빙수 생각 풍선."""
    return front(dy=[0, 1, 0][i], hood_rot=-4, arms=[-30, 30], curl=(-70, 70), fx=ice_bubble_fx(104 - 2 * i, 20, 12.0))


def dm_eat(sp, i):
    """냠냠 (그릇 없음): 앞으로 푹 숙여 후드 속 어둠을 밥 더미 바로 위에 — 숙였다 ↔ 조금 들었다, 부스러기."""
    return front(dy=14 + [0, 2, 0, 2][i], hood_rot=0, arms=[-50, 50], curl=(-40, 40), phase=i, fx=dp.eat_crumbs(i), whole=dict(sy=0.9, pivot=(64, 120)))


def dm_level_up(sp, i):
    """만세: 두 소매 번쩍, 반짝이 별 (가운데 칸은 쑥 솟음)."""
    return front(dy=-[0, 6, 2][i], arms=[-160, 160], curl=(-10, 10), fx=dp.level_up_stars(i))


DEMENTOR_POSES = {"walk": dm_walk, "run": dm_run, "roll": dm_roll, "idle": dm_idle, "look": dm_look, "sleep": dm_sleep, "hop": dm_hop,
                  "fall": dm_fall, "dragged": dm_dragged, "land": dm_land, "sit_front": dm_sit_front, "wave": dm_wave, "tilt": dm_tilt,
                  "belly": dm_belly, "stretch": dm_stretch, "heart": dm_heart, "tickle": dm_tickle, "annoyed": dm_annoyed,
                  "hungry": dm_hungry, "eat": dm_eat, "level_up": dm_level_up, "lookup": dm_lookup}

DEMENTOR = Species(
    id="dementor", name="디멘터", walk_speed=70, scale=1.0, base=CLOAK, back=CLOAK, belly=CLOAK, nose=OUT, mouth="w",
    front_tail_color=CLOAK, tail_color=CLOAK, **dp.NO_COMMON_PARTS,
    specials=["lookup"], stage_size=dict.fromkeys(STAGES, 0.95), stripe=CLOAK_DARK,
    poses={k: (v if k == "roll" else dp._fit(v)) for k, v in DEMENTOR_POSES.items()}, outline=OUT,
    # 어른은 조금 줄여 그림 (5%) — 커진 후드 꼭대기를 칸 안에 두려고 내려 그리면 아랫단이 바닥에 닿았음
    stage_tweak={st: dict(fit=0.95 if st == "adult" else 1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 디멘터를 등록함."""
    dp.SPECIES[DEMENTOR.id] = DEMENTOR


register()


def preview(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 모든 기본 동작 첫 칸 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("idle", 0), ("sit_front", 0), ("wave", 1), ("sleep", 0), ("annoyed", 0), ("hungry", 0), ("eat", 0),
             ("heart", 1), ("fall", 0), ("roll", 0), ("level_up", 1), ("tickle", 0), ("belly", 1), ("dragged", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "walk"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(DEMENTOR, root / "ui" / "pets" / DEMENTOR.id))
    preview(by_stage, pv / "stages_dementor.png")
    print("ok: dementor")


if __name__ == "__main__":
    main()
