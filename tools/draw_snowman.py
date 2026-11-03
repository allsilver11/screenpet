"""눈사람 ⛄ — 동글동글 눈덩이 친구 (snowman). draw_pets 엔진(슈퍼샘플 마스크·녹이기·외곽선)을 그대로 쓴다.

    python tools/draw_snowman.py → ui/pets/snowman/{sheet_<stage>.png ×5, sheet.png, pet.json}
                                   + ui/effects/snow_cloud.png + tools/preview/stages_snowman.png, climb_snowman.png, snow_cloud.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

바탕에 묻히지 않게 연한 파랑 그늘 + 파랑 회색 외곽선. 점 석탄 눈, 볼터치, 작은 당근 코, 활짝 웃는 입, 나뭇가지 팔, 석탄 단추, 작은 눈덩이 발.
단계: 아기·유아·어린이 = 두 덩이 / 청소년·어른 = 세 덩이 (사용자 지정).
      아기 맨몸 → 유아 빨간 목도리 + 단추 하나 → 어린이 작은 양동이 모자 + 단추 둘 → 청소년 세 덩이, 단추 셋 → 어른 긴 줄무늬 목도리 + 호랑가시 잎.
움직임: 작게 통통 (pet.json hop_share 0.5, 눈덩이가 찌그러졌다 늘어난다). roll(구르기) 대신 배로 쭉 미끄러진다 — spin 없음.
벽 타기: 뒷모습 (나뭇가지 손끝만 머리 옆·위로 빼꼼, 발은 몸 뒤에서 번갈아 빼꼼). 위를 보며 행복한 lookup — 구름이 오면 앱이 튼다.
eat에는 밥그릇이 없다 (앱의 소품 창이 그릇을 띄운다).
"""

import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_halloween as dh  # noqa: E402
from draw_halloween import _finish, _heart_items, _stars, _swing, speed_fx  # noqa: E402
from draw_pets import (C, E, L, LINE, Part, Species, T, WHITE, _ST, anger_fx, bee_mouth, brows, heart_fx, mix, question_fx,  # noqa: E402
                       stage_eye, sweat_fx, view, xf, z_fx)
from sprite_spec import FRAME, STAGES, STAGE_SCALE  # noqa: E402

SNOW, SNOW_SHADE, SNOW_LINE = (253, 254, 255), (222, 233, 247), (124, 146, 184)
COAL, COAL_SHINE = (52, 52, 66), (150, 156, 176)
CARROT, CARROT_DARK, CARROT_LINE = (250, 152, 72), (226, 118, 52), (176, 94, 52)
TWIG, TWIG_LINE = (150, 106, 76), (96, 68, 52)
CHEEK = (255, 170, 184)
SCARF, SCARF_DARK, SCARF_STRIPE = (240, 92, 96), (204, 66, 78), (255, 232, 214)
PAIL, PAIL_DARK, PAIL_LIGHT, PAIL_RIM = (146, 190, 236), (112, 158, 214), (204, 228, 250), (98, 140, 198)
HOLLY, HOLLY_BERRY = (92, 168, 112), (232, 72, 72)
ICE, ICE_SYRUP, ICE_CUP = (236, 246, 255), (250, 140, 170), (255, 214, 120)
CLOUD, CLOUD_SHADE, CLOUD_LINE = (240, 245, 253), (212, 224, 242), (124, 146, 184)
SN_FLOOR = 125.0

# 단계(lvl)별 — balls: 아래에서 위로 몸 덩이 (rx, ry) · head: 머리 (rx, ry) · sink: 덩이끼리 겹치는 폭 · scarf: 0 없음, 1 목도리, 2 긴 줄무늬 ·
#               hat: 0 없음, 1 양동이 모자, 2 + 호랑가시 잎 · buttons: 석탄 단추 수 · arm: 나뭇가지 길이
SN_STAGE = {
    0: dict(balls=[(25, 21)], head=(30, 27), sink=9, scarf=0, hat=0, buttons=0, arm=15),
    1: dict(balls=[(28, 24)], head=(29.5, 26), sink=9, scarf=1, hat=0, buttons=1, arm=18),
    2: dict(balls=[(31, 27)], head=(28.5, 25), sink=9, scarf=1, hat=1, buttons=2, arm=20),
    3: dict(balls=[(29, 23), (22.5, 17.5)], head=(25.5, 22), sink=7, scarf=1, hat=1, buttons=3, arm=21),
    4: dict(balls=[(32, 25), (24.5, 19)], head=(25.5, 22), sink=7, scarf=2, hat=2, buttons=3, arm=23),
}


def _sn():
    return SN_STAGE[_ST["lvl"]]


def _ball(cx, cy, rx, ry):
    """눈덩이 하나: 흰 공 + 아래 오른쪽 연한 파랑 그늘 초승달 + 왼쪽 위 은은한 윤."""
    marks = [([E(cx - rx, cy - ry, cx + rx, cy + ry)], SNOW_SHADE),
             ([E(cx - rx * 1.14, cy - ry * 1.16, cx + rx * 0.86, cy + ry * 0.8)], SNOW)]
    return Part([E(cx - rx, cy - ry, cx + rx, cy + ry)], SNOW, marks, outline=True, grow=0)


def _centers(st, bottom, stack=1.0, sway=0.0, head_dy=0.0):
    """[(몸 덩이 cx 오프셋, cy, rx, ry)…], 머리 (dx, cy) — stack < 1이면 덩이들이 서로 파묻힌다, sway: 위로 갈수록 옆으로 쏠림."""
    balls, y, out = st["balls"], bottom - 3, []  # 작은 발이 몸 아래로 3px 나온다
    for k, (rx, ry) in enumerate(balls):
        cy = y - ry if k == 0 else out[-1][1] - (out[-1][3] + ry - st["sink"]) * stack
        out.append((sway * k, cy, rx, ry))
    hrx, hry = st["head"]
    hy = out[-1][1] - (out[-1][3] + hry - st["sink"]) * stack + head_dy
    return out, (sway * len(balls), hy)


def _twig(sh, side, a, ln, wiggle=0.0):
    """나뭇가지 팔: 어깨 sh에서 a(아래 0 ~ 옆 90 ~ 위 180) 쪽으로 + 끝에 잔가지 둘. a가 (x, y)면 손끝."""
    if isinstance(a, (int, float)):
        r = math.radians(a + wiggle)
        tip = (sh[0] + side * math.sin(r) * ln, sh[1] + math.cos(r) * ln)
    else:
        tip = a
    ux, uy = tip[0] - sh[0], tip[1] - sh[1]
    n = math.hypot(ux, uy) or 1.0
    ux, uy = ux / n, uy / n
    px, py = -uy, ux
    k = min(1.0, n / 16)
    # 끝에서 조금 안쪽에서 양쪽으로 벌어지는 잔가지 — 녹아 메워지지 않게 넓게
    fx0, fy0 = tip[0] - ux * 7 * k, tip[1] - uy * 7 * k
    f1 = (fx0 + ux * 5 * k + px * 8.5 * k, fy0 + uy * 5 * k + py * 8.5 * k)
    f2 = (fx0 + ux * 5 * k - px * 8.5 * k, fy0 + uy * 5 * k - py * 8.5 * k)
    return Part([L([sh, tip], 3.0), L([(fx0, fy0), f1], 2.4), L([(fx0, fy0), f2], 2.4)], TWIG, outline=True, grow=0, sharp=True)


def _mark_snowman(st, hx, hy, hrx, hry, side):
    """눈사람의 수면 모자 자리: 양동이가 있으면 양동이째 덮어 쓴다 (_pail과 같은 자리·기울기), 없으면 머리 위에."""
    if st["hat"]:
        dp.mark_head(hx + (hrx * 0.1 if side else hrx * 0.14), hy - hry * 0.84, hrx * 1.55, tilt=-8)
    else:
        dp.mark_head(hx, hy - hry * 0.84, hrx * 1.2)


def _pail(hx, hy, hrx, hry, rot=0.0, side=False, holly=False):
    """작은 양동이 모자 (머리 위에 살짝 기울여) — 어른은 호랑가시 잎 + 빨간 열매."""
    w = hrx * 0.6
    h = hry * 0.68
    top = hy - hry * 0.82
    x = hx + (hrx * 0.1 if side else hrx * 0.14)
    out = []
    with xf(rot=rot - 8, pivot=(x, top)):
        body = T((x - w, top + 2), (x + w, top + 2), (x + w * 0.74, top - h), (x - w * 0.74, top - h))
        marks = [([T((x + w * 0.3, top + 2), (x + w + 1, top + 2), (x + w * 0.8, top - h), (x + w * 0.2, top - h))], PAIL_DARK),
                 ([L([(x - w * 0.5, top - 1), (x - w * 0.42, top - h + 2)], 2.2)], PAIL_LIGHT)]
        out.append(Part([body], PAIL, marks, outline=True, grow=0))
        out.append(Part([L([(x - w - 1, top + 2), (x + w + 1, top + 2)], 4.4)], PAIL_RIM, outline=True, grow=0))
        if holly:
            lx, ly = x - w * 0.45, top - h * 0.45
            out.append(Part([E(lx - 6.5, ly - 2.4, lx + 0.5, ly + 2.4), E(lx - 0.5, ly - 2.4, lx + 6.5, ly + 2.4)], HOLLY, outline=True, grow=0))
            out.append(Part([C(lx, ly, 2.2)], HOLLY_BERRY, [([C(lx - 0.6, ly - 0.7, 0.7)], WHITE)], outline=True, grow=0))
    return out


def _scarf(cx, cy, rx, st, side, wave=0.0, back_view=False):
    """목에 두른 빨간 목도리 + 늘어진 끝. wave: 끝 펄럭임 (옆모습은 뒤로 날린다)."""
    w = 7.5 if st["scarf"] == 2 else 6.5
    band = L([(cx - rx * 0.86, cy - 1), (cx - rx * 0.4, cy + 2.4), (cx + rx * 0.4, cy + 2.4), (cx + rx * 0.86, cy - 1)], w)
    marks = [([L([(cx - rx * 0.8, cy + 2.6), (cx + rx * 0.8, cy + 2.6)], 1.6)], SCARF_DARK)]
    if st["scarf"] == 2:
        marks += [([L([(cx + d, cy - 4), (cx + d, cy + 7)], 2.2)], SCARF_STRIPE) for d in (-rx * 0.55, -rx * 0.18, rx * 0.18, rx * 0.55)]
    ln = 13.0 if st["scarf"] == 1 else 19.0
    if side:
        tx, ty = cx - rx * 0.72, cy + 2
        pts = [(tx, ty), (tx - ln * 0.42, ty + ln * 0.28 + wave), (tx - ln * 0.8, ty + ln * 0.62 - wave)]
    elif back_view:
        tx, ty = cx - rx * 0.28, cy + 3
        pts = [(tx, ty), (tx - 1.5 + wave, ty + ln * 0.55), (tx - 2.5 - wave, ty + ln)]
    else:
        tx, ty = cx + rx * 0.42, cy + 3
        pts = [(tx, ty), (tx + 1.5 + wave, ty + ln * 0.55), (tx + 2.5 - wave, ty + ln)]
    tail_marks = [([L([(x - 4.5, y), (x + 4.5, y)], 2.0)], SCARF_STRIPE) for x, y in pts[1:]] if st["scarf"] == 2 else []
    tail = Part([L(pts, w - 0.5)], SCARF, tail_marks, outline=True, grow=0)
    return [tail, Part([band], SCARF, marks, outline=True, grow=0)]


def _carrot(nx, ny, side, k=1.0, rot=0.0):
    """작은 당근 코 (얼굴 층 항목): 옆모습은 앞으로 뾰족, 앞모습은 살짝 옆으로."""
    ln, hw = (12.5 * k, 4.1 * k) if side else (9.5 * k, 3.9 * k)
    drop = 1.2 if side else 2.4
    with xf(rot=rot, pivot=(nx, ny)):
        tri = T((nx - 1, ny - hw), (nx - 1, ny + hw), (nx + ln, ny + drop))
        line = [L([(nx - 1, ny - hw), (nx + ln, ny + drop), (nx - 1, ny + hw), (nx - 1, ny - hw)], 1.5)]
        stripes = [L([(nx + 3 * k, ny - hw * 0.5), (nx + 3.6 * k, ny + hw * 0.1)], 1.0)]
        return [(line, CARROT_LINE, 1.0, 0, 0), ([tri], CARROT, 1.0, 0, 0), (stripes, CARROT_DARK, 1.0, 0, 0)]


def snowman(sp, *, side=False, bottom=None, cx=64.0, sq=(1.0, 1.0), rot=0.0, stack=1.0, sway=0.0, eyes="dot", mouth=None, brow=None, arms=None,
            feet=((0, 0), (0, 0)), fx=(), back=(), props=(), top=(), head_dy=0.0, head_rot=0.0, wave=0.0, spots=None):
    """눈사람 한 장. bottom: 발 아래끝 y · sq: 아래끝 기준 찌그러짐 · rot: 몸 기울기 · stack: 덩이 사이 (작을수록 파묻힘) · sway: 위로 갈수록 옆으로 ·
    arms: [(side, 각도 | 손끝)] · feet: (왼, 오) 발 오프셋 또는 None · head_dy/head_rot: 고개 · wave: 목도리 펄럭 ·
    spots: (몸 덩이 [(cx, cy)…], 머리 (cx, cy)) — 눕혀서 그릴 때 자리를 직접 준다."""
    st = _sn()
    lvl = _ST["lvl"]
    bottom = SN_FLOOR if bottom is None else bottom
    balls, (hdx, hy) = _centers(st, bottom, stack, sway, head_dy)
    if spots:
        balls = [(p[0] - cx, p[1], rx, ry) for p, (_, _, rx, ry) in zip(spots[0], balls)]
        hdx, hy = spots[1][0] - cx, spots[1][1]
    hrx, hry = st["head"]
    hx = cx + hdx
    fxx = hx + (0.34 * hrx if side else 0.0)
    parts, glow, face = _flat(back), [], []
    base_x, base_y, brx, bry = cx + balls[0][0], balls[0][1], balls[0][2], balls[0][3]
    upper_x, upper_y, urx, ury = cx + balls[-1][0], balls[-1][1], balls[-1][2], balls[-1][3]
    with xf(rot=rot, pivot=(cx, bottom)):
        with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
            arm_at = (upper_x, upper_y - ury * 0.05)
            if arms is None:
                arms = [(1, 62)] if side else [(-1, 66), (1, 66)]
            near = []
            for s_, a in arms:
                if side:
                    sh = (arm_at[0] + 0.3 * urx, arm_at[1] + 1)
                else:
                    sh = (arm_at[0] + s_ * 0.8 * urx, arm_at[1])
                tw = _twig(sh, s_, a, st["arm"])
                if side:
                    near.append(tw)
                else:
                    parts.append(tw)
            if side and len(near) > 1:
                parts.append(near.pop(0))
            if feet:
                xs = (base_x - 0.22 * brx, base_x + 0.34 * brx) if side else (base_x - 0.5 * brx, base_x + 0.5 * brx)
                fr = max(6.2, 0.26 * brx)
                for x, (dx, dy) in zip(xs, feet):
                    parts.append(Part([E(x + dx - fr * 1.14, base_y + bry - fr * 0.76 + dy, x + dx + fr * 1.14, base_y + bry + 3 + dy)], SNOW,
                                      [([E(x + dx - fr, base_y + bry + dy, x + dx + fr, base_y + bry + 4 + dy)], SNOW_SHADE)], outline=True, grow=0))
            for dx, cy, rx, ry in balls:
                parts.append(_ball(cx + dx, cy, rx, ry))
            # 석탄 단추 (맨 윗덩이 → 아랫덩이)
            ox = upper_x + (0.34 * urx if side else 0.0)
            btn = []
            if st["buttons"] >= 1:
                btn.append((ox, upper_y + ury * (0.05 if st["buttons"] == 1 or len(balls) > 1 else -0.15)))
            if st["buttons"] >= 2:
                if len(balls) > 1:
                    btn.append((base_x + (0.34 * brx if side else 0.0), base_y - bry * 0.32))
                else:
                    btn.append((ox, upper_y + ury * 0.42))
            if st["buttons"] >= 3:
                btn.append((base_x + (0.36 * brx if side else 0.0), base_y + bry * 0.2))
            for bx_, by_ in btn:
                face.append(([C(bx_, by_, 3.1)], COAL, 1.0, 0, 0))
                face.append(([C(bx_ - 0.9, by_ - 0.9, 0.9)], COAL_SHINE, 1.0, 0, 0))
            if st["scarf"]:
                parts += _scarf(hx + (0.05 * hrx if side else 0.0), hy + hry * 0.9, hrx * 0.86, st, side, wave)
            parts += near
            # 머리
            with xf(rot=head_rot, pivot=(hx, hy + hry * 0.8)):
                parts.append(_ball(hx, hy, hrx, hry))
                _mark_snowman(st, hx, hy, hrx, hry, side)
                if st["hat"]:
                    parts += _pail(hx, hy, hrx, hry, side=side, holly=st["hat"] >= 2)
                ey = hy - 0.1 * hry
                sp_x = 0.36 * hrx if not side else 0.32 * hrx
                for j, ex in enumerate((fxx - sp_x, fxx + sp_x)):
                    kx = 0.84 if side and j == 0 else 1.0
                    with xf(sx=kx, pivot=(ex, ey)):
                        face += dh._ops(stage_eye(eyes, ex, ey, flip=j == 1), recolor={dp.EYE: COAL})
                    if brow:
                        face += dh._ops(brows(brow, ex, ey - 1 + _ST["eye_dy"], flip=j == 1))
                bx_, by_ = _ST["blush"]
                for cxx in (fxx - 0.66 * hrx + (0.14 * hrx if side else 0), fxx + 0.62 * hrx):
                    glow.append(([E(cxx - 0.17 * hrx * bx_, hy + 0.3 * hry - 0.12 * hry * by_, cxx + 0.17 * hrx * bx_, hy + 0.3 * hry + 0.12 * hry * by_)],
                                 CHEEK, 0.8, 0, 0.6))
                face += dh._ops(_mouth(mouth, fxx + (1 if side else 0), hy + mouth_dy(st)))
                face += _carrot(fxx + (1.5 if side else -0.5), hy + 0.14 * hry, side, 0.92 + 0.04 * (4 - lvl))
    parts += _flat(props)
    return _finish(parts, glow, face + list(top), fx)


def mouth_dy(st):
    return st["head"][1] * 0.56


def _mouth(kind, mx, my):
    """바보같이 환한 웃음: 기본은 넓게 벌린 입 곡선 + 입꼬리."""
    if kind is None:
        from draw_pets import arc
        return [([L(arc(mx, my - 3.0, 6.6, 4.2, 18, 162, 12), 1.7)], LINE)]
    return bee_mouth(kind, mx, my)


def _flat(makers):
    out = []
    for f in makers:
        r = f()
        out += r if isinstance(r, list) else [r]
    return out


def snow_puffs(items):
    out = []
    for x, y, r in items:
        out += [([C(x, y, r)], SNOW_LINE, 0), ([C(x, y, r - 1.1)], WHITE, 0)]
    return out


def ice_bubble_fx(cx, cy):
    """생각 풍선 안에 빙수 한 컵."""
    fx = dh._bubble(cx, cy)
    cup = T((cx - 7, cy + 1), (cx + 7, cy + 1), (cx + 4.6, cy + 8), (cx - 4.6, cy + 8))
    fx += [([C(cx, cy - 1.5, 6.8), E(cx - 8, cy - 2, cx + 8, cy + 2.6)], SNOW_LINE, 0.6), ([C(cx, cy - 1.5, 5.8), E(cx - 7, cy - 1.5, cx + 7, cy + 2)], ICE, 0),
           ([E(cx - 3.8, cy - 6.6, cx + 3.4, cy - 2.2)], ICE_SYRUP, 0), ([cup], LINE, 0.6), ([cup], ICE_CUP, 0)]
    return fx


# ---- 포즈 ----
def sn_walk(sp, i):
    """뒤뚱뒤뚱: 위 덩이가 좌우로 쏠리고 작은 발을 번갈아."""
    up = [0, 1.5, 0, 1.5][i]
    feet = [((-3, 0), (3, -3)), ((0, 0), (0, 0)), ((3, -3), (-3, 0)), ((0, 0), (0, 0))][i]
    return snowman(sp, side=True, bottom=SN_FLOOR - up, rot=[-3, 0, 3, 0][i], sway=[-2.2, 0, 2.2, 0][i], feet=feet, arms=[(1, [48, 66, 84, 66][i])],
                   head_rot=[-2, 0, 2, 0][i], sq=(1.0, [1.0, 1.02, 1.0, 1.02][i]), wave=[1, -1, 1, -1][i])


def sn_run(sp, i):
    up = [0, 3, 5, 2][i]
    feet = ((-6, 0), (6, -3)) if i % 2 == 0 else ((5, -3), (-5, 0))
    return snowman(sp, side=True, bottom=SN_FLOOR - up, rot=[4, 7, 5, 7][i], sway=[1.5, 3, 2, 3][i], feet=feet, eyes="happy", mouth="open",
                   arms=[(1, [110, 80, 110, 80][i])], wave=[2, -2, 2, -2][i], fx=speed_fx(4, (72, 90, 106), 11, [0, -3, 0, -3][i]))


def sn_roll(sp, i):
    """구르기 대신 배로 쭉 미끄러짐 🐧: 눈덩이들이 옆으로 눕고 머리만 들고 활짝 — 앞으로 눈가루, 목도리 펄럭 (앱이 돌리지 않는다 — pet.json roll에 spin 없음)."""
    st = _sn()
    base = SN_FLOOR - 1
    n = len(st["balls"])
    hrx, hry = st["head"]
    lift = (4, 13, 20)[st["scarf"]]  # 목도리가 바닥에 끌리지 않게 머리를 든다
    if n == 1:
        rx, ry = st["balls"][0]
        x0 = 9 + rx
        spots = ([(x0, base - ry)], (x0 + rx * 0.72 + hrx * 0.8, base - hry - lift))
    else:
        (r0x, r0y), (r1x, r1y) = st["balls"]
        x0 = 2 + r0x
        x1 = x0 + r0x * 0.8 + r1x * 0.42
        spots = ([(x0, base - r0y), (x1, base - r1y)], (x1 + r1x * 0.35 + hrx * 0.68, base - hry - lift))
    x0, y0 = spots[0][0]
    tail = lambda: _twig((x0 + 4, y0 - st["balls"][0][1] * 0.55), -1, [122, 108][i], st["arm"] * 1.5)  # 뒤로 쭉 뻗은 나뭇가지 팔
    puffs = snow_puffs([[(118, base - 3, 4.0), (111, base - 1, 3.0), (121, base - 11, 2.6)],
                        [(121, base - 4, 3.6), (114, base - 1, 2.8), (116, base - 13, 2.8)]][i])
    return snowman(sp, side=True, bottom=SN_FLOOR, spots=spots, feet=None, eyes="happy", mouth="open", arms=[(1, 118)], back=[tail], head_rot=[-6, -4][i], wave=[3, -3][i],
                   fx=puffs)


def sn_idle(sp, i):
    return snowman(sp, side=True, sq=(1.0, 1.0 + 0.02 * i), arms=[(1, 62 + 5 * i)])


def sn_look(sp, i):
    return snowman(sp, side=True, head_rot=[-8, 0, 8, 0][i], eyes="closed" if i == 3 else "dot", arms=[(1, [60, 66, 72, 66][i])])


def sn_lookup(sp, i):
    """위를 보며 활짝 (구름이 오면) — 나뭇가지 팔을 번쩍."""
    return snowman(sp, side=True, head_rot=dp.lookup_tilt(i), eyes="up" if i < 2 else "happy", mouth="open", arms=[(1, [110, 135, 150][i])],
                   fx=dp.lookup_fx(i, 112, 14))


def sn_sleep(sp, i):
    zs = [z_fx(104, 52, 3.4), z_fx(115, 41, 2.6)] if i == 0 else [z_fx(106, 49, 3.6), z_fx(118, 37, 2.8)]
    return snowman(sp, side=True, sq=(1.05, 0.93 + 0.012 * i), stack=0.92, eyes="closed", head_dy=2, head_rot=7, feet=((1, 0), (-1, 0)), arms=[(1, 18)], fx=zs)


def sn_annoyed(sp, i):
    return snowman(sp, side=True, sq=(1.03, 0.96), eyes="squint", mouth="pout", arms=[(1, 15)], fx=[anger_fx(106, 34 - i, 7.0)])


def sn_hop(sp, i):
    """통! 눈덩이가 납작 → 쭉 → 사뿐."""
    if i == 0:
        return snowman(sp, side=True, sq=(1.13, 0.82), stack=0.9, eyes="tight", arms=[(1, 30)], feet=((-2, 0), (2, 0)))
    if i == 1:
        return snowman(sp, side=True, bottom=SN_FLOOR - 3, sq=(0.94, 1.02), stack=1.03, rot=-4, eyes="happy", mouth="open", feet=((-3, 3), (3, 1)),
                       arms=[(1, 130)], wave=3)
    return snowman(sp, side=True, bottom=SN_FLOOR - 6, rot=2, eyes="happy", feet=((-2, 1), (2, 0)), arms=[(1, 80)], wave=-2)


def sn_fall(sp, i):
    stars = _stars([(100, 18, 3.6), (26, 26, 3.0)] if i == 0 else [(30, 16, 3.6), (104, 28, 3.0)])
    return snowman(sp, side=True, bottom=SN_FLOOR - 7, rot=[-10, 10][i], sway=[-3, 3][i], eyes="spiral", mouth="wavy", feet=((-4, 2), (4, -3)) if i == 0 else ((-4, -3), (4, 2)),
                   arms=[(1, [150, 110][i])], wave=[3, -3][i], fx=stars)


def sn_dragged(sp, i):
    s = [-1, 1][i]
    return snowman(sp, side=True, bottom=SN_FLOOR - 6, rot=4 * s, sway=2 * s, stack=1.03, eyes="tight", mouth="open", feet=((-3 + 2 * s, 4), (3 + 2 * s, 5)),
                   arms=[(1, 150)], fx=sweat_fx(104, 34))


def sn_land(sp, i):
    if i == 0:
        return snowman(sp, side=True, sq=(1.14, 0.8), stack=0.88, eyes="tight", arms=[(1, 90)])
    return snowman(sp, side=True, sq=(1.03, 0.97))


def sn_sit_front(sp, i):
    return snowman(sp, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot", arms=[(-1, 66 + 3 * [0, 1, 2, 1][i]), (1, 66 + 3 * [0, 1, 2, 1][i])])


def sn_wave(sp, i):
    return snowman(sp, arms=[(-1, 50), (1, [135, 165, 135, 165][i])], head_rot=[-3, -1, -3, -1][i], eyes="happy" if i % 2 else "dot", mouth="open" if i % 2 else None)


def sn_tilt(sp, i):
    return snowman(sp, head_rot=_swing([8, 12, 12][i]), sway=[1, 2, 2][i], arms=[(-1, 60), (1, 100)], fx=[question_fx(18, 22 + [2, 0, -2][i])])


def sn_belly(sp, i):
    """벌러덩: 뒤로 엉덩방아 찧고 작은 발을 위로 버둥, 나뭇가지 팔 번쩍, 껄껄."""
    wig = [0, 2, 0, -2][i]
    brx = _sn()["balls"][0][0]
    return snowman(sp, sq=(1.1, 0.86), stack=0.86, rot=-6, eyes="happy", mouth="big", feet=((-0.5 * brx, -24 + wig), (0.5 * brx, -26 - wig)),
                   arms=[(-1, 140 + 8 * wig), (1, 140 - 8 * wig)], head_dy=2)


def sn_stretch(sp, i):
    if i == 0:
        return snowman(sp, sq=(0.95, 1.02), stack=1.03, eyes="tight", arms=[(-1, 165), (1, 165)])
    if i == 1:
        return snowman(sp, eyes="tight", mouth="yawn", arms=[(-1, 100), (1, 100)], head_rot=-3)
    return snowman(sp)


def sn_heart(sp, i):
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    st = _sn()
    balls, _ = _centers(st, SN_FLOOR)
    hy = balls[-1][1] - 3
    return snowman(sp, eyes="happy", top=_heart_items(64, hy, 0.8), arms=[(-1, (53, hy + 6)), (1, (75, hy + 6))], fx=hs)


def sn_tickle(sp, i):
    return snowman(sp, bottom=SN_FLOOR - 3, rot=_swing([-6, 6, 0][i]), sway=[-3, 3, 0][i], eyes="happy", mouth="big", arms=[(-1, [70, 120, 95][i]), (1, [120, 70, 95][i])])


def sn_hungry(sp, i):
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    st = _sn()
    balls, _ = _centers(st, SN_FLOOR)
    return snowman(sp, rot=-3, eyes="closed", mouth="frown", brow="sad", head_rot=-4, arms=[(-1, 25), (1, (68 + dx, balls[-1][1] + dy))], fx=ice_bubble_fx(106, 16))


def sn_eat(sp, i):
    """냠냠 (그릇 없음): 덩이들이 쏙 파묻혀 입이 그릇 밥 더미 바로 위 (프레임 (64, 84))로 내려온다. 앙 ↔ 오물오물."""
    bite = i % 2 == 0
    st = _sn()
    _, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    stack = 0.72
    _, (_, hy0) = _centers(st, SN_FLOOR, stack)
    head_dy = max(-6.0, my - (hy0 + mouth_dy(st)))
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    paws = [dp._frame_to_pre(40, 96), dp._frame_to_pre(88, 96)]
    return snowman(sp, stack=stack, head_dy=head_dy, eyes="happy" if bite else "closed", mouth="open" if bite else "chew", head_rot=[0, -2, 0, 2][i],
                   sq=(1.04, 1.0), arms=[(-1, paws[0]), (1, paws[1])], fx=fx)


def sn_level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    up = [150, 165, 175][i]
    return snowman(sp, bottom=SN_FLOOR - [0, 1, 3][i], eyes="happy", mouth="big", arms=[(-1, up), (1, up)], feet=((0, 0), (0, 0)) if i < 2 else ((-1, 1), (1, 1)), fx=stars)


# ---- 뒷모습 (벽 타기) — 다리는 몸 뒤, 나뭇가지 손끝만 머리 옆·위로 빼꼼 ----
BACK_FOOT = {"side": (0.92, -0.1), "low": (0.42, 0.72), "tuck": (0.3, -0.25)}  # 몸 덩이 (rx, ry) 기준 발 자리: 옆으로 디딤 · 아래로 빼꼼 · 완전히 숨음


def snowman_back(reach, feet, *, sq=(1.0, 1.0), dy=0.0, scratch=None, sweat=None, wave=0.0):
    """뒷모습. reach: (왼, 오) 0 머리 옆 낮게 … 1 머리 위로 쭉 · feet: (왼, 오) BACK_FOOT 키."""
    st = _sn()
    bottom = SN_FLOOR + dy
    balls, (_, hy) = _centers(st, bottom)
    cx = 64.0
    hrx, hry = st["head"]
    parts, ops = [], []
    with xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
        base_y, brx, bry = balls[0][1], balls[0][2], balls[0][3]
        fr = max(6.2, 0.26 * brx)
        for s_, at in zip((-1, 1), feet):  # 몸보다 먼저 — 몸 밖으로 나온 만큼만 보인다
            ox, oy = BACK_FOOT[at]
            x, y = cx + s_ * brx * ox, base_y + bry * oy
            parts.append(Part([E(x - fr * 1.12, y - fr, x + fr * 1.12, y + fr)], SNOW, [([E(x - fr, y + fr * 0.2, x + fr, y + fr + 2)], SNOW_SHADE)], outline=True, grow=0))
        for s_, r in zip((-1, 1), reach):  # 나뭇가지 손끝 (머리 뒤에 숨는다)
            tx = cx + s_ * (hrx * (1.26 - 0.62 * r))
            ty = hy + hry * (0.5 - 1.75 * r)
            parts.append(_twig((cx + s_ * hrx * 0.6, hy + hry * 0.45), s_, (tx, ty), st["arm"]))
            if scratch is not None:
                for j, d in enumerate((-4, 0, 4)):
                    y0 = ty - 8 - (j % 2) * 1.5 - scratch * 2
                    ops.append(([L([(tx + d * 1.1, y0), (tx + d * 1.3, y0 - 7)], 1.5)], SNOW_LINE, 1.0, 0, 0))
        for _, cy, rx, ry in balls:
            parts.append(_ball(cx, cy, rx, ry))
        parts.append(_ball(cx, hy, hrx, hry))
        _mark_snowman(st, cx, hy, hrx, hry, False)
        if st["scarf"]:
            parts += _scarf(cx, hy + hry * 0.9, hrx * 0.86, st, False, wave, back_view=True)
        if st["hat"]:
            parts += _pail(cx, hy, hrx, hry)
    fx = sweat_fx(cx + hrx + 10, hy - 2 + sweat) if sweat is not None else []
    return _finish(parts, (), ops, fx)


def sn_climb(sp, i):
    """화면을 기어오름: 나뭇가지 손끝 번갈아 위로, 발은 손과 엇갈려 빼꼼 → 옆으로 → 숨음, 덩이 살짝 늘어남."""
    reach = [(1.0, 0.25), (0.62, 0.62), (0.25, 1.0), (0.62, 0.62)][i]
    feet = [("tuck", "side"), ("low", "tuck"), ("side", "tuck"), ("tuck", "low")][i]
    return snowman_back(reach, feet, sq=(1.0, [1.04, 1.0, 1.04, 1.0][i]), dy=[-1, 0, -1, 0][i], wave=[1, 0, -1, 0][i])


def sn_slide(sp, i):
    """주르륵: 두 손 쭉 위로 + 긁힌 자국, 살짝 찌그러짐, 땀 한 방울, 발은 아래로 빼꼼."""
    return snowman_back((1.0, 1.0), ("low", "low"), sq=(1.03, 0.94), dy=[0, 1][i], scratch=i, sweat=i, wave=[2, -2][i])


SNOWMAN_POSES = {"walk": sn_walk, "run": sn_run, "roll": sn_roll, "idle": sn_idle, "look": sn_look, "sleep": sn_sleep, "hop": sn_hop, "fall": sn_fall,
                 "dragged": sn_dragged, "land": sn_land, "sit_front": sn_sit_front, "wave": sn_wave, "tilt": sn_tilt, "belly": sn_belly, "stretch": sn_stretch,
                 "heart": sn_heart, "tickle": sn_tickle, "annoyed": sn_annoyed, "hungry": sn_hungry, "eat": sn_eat, "level_up": sn_level_up, "lookup": sn_lookup,
                 "climb": sn_climb, "slide": sn_slide}
RAW_POSES = dict(SNOWMAN_POSES)  # 테스트: 자르기 전에 칸을 넘지 않는지
SNOWMAN_POSES = {k: dh._fit(v) for k, v in SNOWMAN_POSES.items()}

SNOWMAN = Species(
    id="snowman", name="눈사람", walk_speed=70, scale=0.94, base=SNOW, back=SNOW, belly=SNOW, nose=CARROT, mouth="w",
    front_tail_color=SNOW, tail_color=SNOW, stage_size=dict.fromkeys(STAGES, 1.0), stripe=SNOW_SHADE, outline=SNOW_LINE,
    # 작은 석탄 점 눈 (모든 단계 동그랗게 — 반쯤 감은 눈 없이) · 어릴수록 크게
    stage_tweak={"baby": dict(pale=0.0, sparkle=False, soft=0.0, eye=1.2, eye_dy=0.0, fit=0.98, smile=False), "kinder": dict(pale=0.0, sparkle=False, soft=0.0, eye=1.12, eye_dy=0.0, fit=0.98),
                 "child": dict(smile=False, soft=0.0, eye=1.04, eye_dy=0.0, fit=0.96), "teenager": dict(eye=0.96, fit=0.94),
                 "adult": dict(calm=False, eye=0.96, eye_dy=0.0, fit=0.9)},
    **{k: v for k, v in dh._COMMON.items() if k not in ("poses", "specials")}, specials=["climb", "slide", "lookup"], poses=SNOWMAN_POSES,
)


def register():
    dp.SPECIES[SNOWMAN.id] = SNOWMAN


register()


# ---- 눈구름 (앱이 render.cloud 동안 머리 위에 띄운다 — ui/style.css #snowcloud) ----
CLOUD_W, CLOUD_H = 56, 37


def draw_cloud():
    """작고 포근한 눈구름 — 졸린 듯 웃는 얼굴."""
    img = Image.new("RGBA", (dp.S, dp.S), (0, 0, 0, 0))
    with view(scale=1.0, lift=0):
        from draw_props import blob, tiny_face
        fns = [C(46, 67, 13), C(64, 59, 16), C(83, 66, 13), E(36, 62, 92, 80)]
        blob(img, fns, CLOUD, r=1.5, line=1.0, line_color=CLOUD_LINE, marks=[([E(32, 72, 96, 90)], CLOUD_SHADE), ([E(53, 46, 65, 52)], WHITE)])
        tiny_face(img, 64, 66, 1.3)
    # 그림 좌표 (27, 37)~(101, 86)을 슈퍼샘플에서 바로 줄인다
    k = dp.K
    out = img.crop((27 * k, 37 * k, 101 * k, 86 * k)).resize((CLOUD_W, CLOUD_H), Image.LANCZOS)
    out.putalpha(out.getchannel("A").point(lambda v: 0 if v < 10 else v))
    return out


# ---- 미리보기 ----
def preview_cloud(by_stage, cloud, path):
    """단계마다 sit_front·idle + 머리 위 구름과 눈송이 (앱과 같은 배치·크기), 밝은·어두운 바탕."""
    cols = len(STAGES)
    cell = 170
    img = Image.new("RGBA", (cols * cell, 2 * (cell + 36)), (246, 248, 252, 255))
    img.paste((46, 50, 60, 255), (0, cell + 36, cols * cell, 2 * (cell + 36)))
    for band in range(2):
        for c, st in enumerate(STAGES):
            scale = STAGE_SCALE[st] * SNOWMAN.stage_size[st]
            side = round(FRAME * scale)
            fr = by_stage[st]["lookup"][1].resize((side, side), Image.LANCZOS)
            x, y = c * cell + (cell - side) // 2, band * (cell + 36) + cell + 36 - side - 2
            img.alpha_composite(fr, (x, y))
            a = fr.getchannel("A").point(lambda v: 255 if v > 128 else 0)
            head_top = a.getbbox()[1]
            k = math.sqrt(scale)
            w, h = round(CLOUD_W * k), round(CLOUD_H * k)
            cl = cloud.resize((w, h), Image.LANCZOS)
            gap = round(14 * k)
            cy = max(band * (cell + 36), y + head_top - gap - h)
            img.alpha_composite(cl, (c * cell + (cell - w) // 2, cy))
            for j, (dx, t) in enumerate([(-0.3, 0.25), (-0.1, 0.7), (0.12, 0.4), (0.3, 0.85)]):
                fy = cy + h + t * gap
                fxx = c * cell + cell / 2 + dx * w
                d = Image.new("RGBA", img.size, (0, 0, 0, 0))
                from PIL import ImageDraw
                ImageDraw.Draw(d).ellipse((fxx - 2.5, fy - 2.5, fxx + 2.5, fy + 2.5), fill=(255, 255, 255, 255), outline=(150, 170, 204, 255))
                img.alpha_composite(d)
    img.save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.write_species(SNOWMAN, root / "ui" / "pets" / SNOWMAN.id)
    dh.preview_species(by_stage, pv / "stages_snowman.png")
    dp.preview_back(by_stage, pv / "climb_snowman.png")
    cloud = draw_cloud()
    cloud.save(root / "ui" / "effects" / "snow_cloud.png")
    preview_cloud(by_stage, cloud, pv / "snow_cloud.png")
    print("ok: snowman")


if __name__ == "__main__":
    main()
