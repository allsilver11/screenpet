"""공룡 친구들 🦖 — 노란 공룡(dino_yellow) · 초록 공룡(dino_green) · 보라 공룡(dino_purple). draw_pets 엔진을 그대로 쓴다.

    python tools/draw_dino.py → ui/pets/dino_{yellow,green,purple}/{sheet_<stage>.png ×5, sheet.png, pet.json}
                                + tools/preview/stages_dino.png (세 색 × 다섯 단계, 2배 확대)
    (python tools/draw_pets.py도 셋을 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

말랑한 봉제 인형 티라노 (참고: 하늘색 몸 + 노란 등 가시 인형): 몸의 절반쯤 되는 큰 동글 머리와 불룩한 동글 주둥이,
작은 갈색 점 눈, 분홍 타원 볼터치, 아주 짧은 앞발, 통통한 동글 다리, 끝이 말랑하게 뾰족한 짧고 굵은 꼬리,
등을 따라 몸과 대비되는 색의 동글 세모 가시. 한 번 그리고 색만 바꾼다 (PALETTES):
  노랑 = PANTONE 13-0758 Dandelion (#FFD02E) + 하늘색 가시 · 초록 + 진초록 가시 · 보라(라벤더) + 분홍 가시.
단계: 아기 = 머리에 알껍데기 모자 + 콩알 가시 → 유아 → 어린이 → 청소년 → 어른 = 가장 큰 가시 (색과 상관없이 같다).
공룡만의 동작 (선택 동작, 옆모습): fire (흡 → 작은 불꽃 퐁 → 화르륵 + 연기 → 흐뭇) · roar (고개 들고 크앙 + 소리 물결) ·
stomp (제자리 쿵쾅쿵쾅 + 먼지 퐁퐁). 불꽃은 칸 안에 그린다 (몸을 조금 뒤로 물려 입 앞에 자리를 낸다).
걷기 = 쿵쿵 뒤뚱 (꼬리 살랑), 달리기 = 크게 통통 튀는 쿵쾅 달리기 (발밑 먼지). eat에는 밥그릇이 없다.
"""

import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, E, EYE, HEART, L, LINE, MOUTH_IN, T, TONGUE, WHITE, Part, Species, _ST, anger_fx, arc, blush,  # noqa: E402
                       brows, heart_fx, mix, open_mouth, question_fx, sparkle_fx, stage_eye, sweat_fx, view, xf, z_fx)
from draw_halloween import _bubble, _fit, _stars, speed_fx  # noqa: E402
from sprite_spec import FRAME, OPTIONAL_ANIMS, ROLL_CENTER, STAGES  # noqa: E402

FLOOR = 125.0

# =====================================================================
# 색 — 종마다 몸·가시·배·외곽선
# =====================================================================
DANDELION = (255, 208, 46)  # PANTONE 13-0758 Dandelion
PALETTES = {
    "dino_yellow": dict(body=DANDELION, spike=(118, 192, 240), belly=(255, 243, 200), line=(170, 112, 52)),
    "dino_green": dict(body=(142, 214, 118), spike=(64, 156, 88), belly=(232, 248, 210), line=(84, 122, 70)),
    "dino_purple": dict(body=(194, 166, 236), spike=(250, 156, 196), belly=(246, 238, 255), line=(118, 94, 152)),
}
SHELL, SHELL_SPOT = (255, 252, 240), (226, 216, 196)
FLAME, FLAME_MID, FLAME_CORE = (255, 132, 64), (255, 190, 72), (255, 244, 170)
SMOKE = (214, 214, 220)
DUST, DUST_LINE = (240, 230, 214), (196, 176, 150)
WAVE = mix(LINE, WHITE, 0.25)

# 단계(lvl)별 — spikes: 몸 가시 개수 · head_spikes: 머리 가시 · h: 가시 높이 · shell: 알껍데기 모자
DN_STAGE = {
    0: dict(spikes=3, head_spikes=0, h=7.0, shell=True),
    1: dict(spikes=3, head_spikes=1, h=8.6, shell=False),
    2: dict(spikes=4, head_spikes=1, h=10.0, shell=False),
    3: dict(spikes=4, head_spikes=2, h=11.4, shell=False),
    4: dict(spikes=5, head_spikes=2, h=13.6, shell=False),
}


def _dn():
    return DN_STAGE[_ST["lvl"]]


def _pal(sp):
    return PALETTES[sp.id]


def _shade(col):
    return mix(col, (90, 70, 90), 0.14)


# =====================================================================
# 부품
# =====================================================================
def spike(px, py, nx, ny, h, w):
    """동글 세모 가시: 밑변 가운데 (px, py) (몸 안으로 조금 묻힘), 바깥쪽 (nx, ny) 방향으로 h, 밑변 폭 w."""
    tx, ty = -ny, nx
    bx, by = px - nx * 2.5, py - ny * 2.5
    tip = (px + nx * h, py + ny * h)
    rr = w * 0.28
    cx, cy = tip[0] - nx * rr, tip[1] - ny * rr
    a0 = math.degrees(math.atan2(ty, tx))
    cap = arc(cx, cy, rr, rr, a0 - 180, a0, 6)
    return T((bx - tx * w / 2, by - ty * w / 2), *cap, (bx + tx * w / 2, by + ty * w / 2))


def ell_point(cx, cy, rx, ry, a):
    """타원 위 점과 바깥 법선 (각도 a: 0=오른쪽, 90=아래)."""
    r = math.radians(a)
    x, y = cx + rx * math.cos(r), cy + ry * math.sin(r)
    nx, ny = math.cos(r) / rx, math.sin(r) / ry
    n = math.hypot(nx, ny)
    return x, y, nx / n, ny / n


def egg_shell(cx, top, w, side=True):
    """아기 머리 위 알껍데기 모자: 동글 윗부분 + 지그재그 깨진 아래 가장자리, 점 두어 개."""
    dome = arc(cx, top + w * 0.42, w * 0.5, w * 0.55, 180, 360, 14)
    zig = []
    n = 6
    for k in range(n + 1):
        x = cx + w * 0.5 - w * k / n
        zig.append((x, top + w * 0.42 + (3.2 if k % 2 else 0)))
    spots = [([C(cx - w * 0.15, top + w * 0.08, 1.4), C(cx + w * 0.2, top + w * 0.22, 1.1)], SHELL_SPOT)]
    return Part([T(*(dome + zig))], SHELL, spots, grow=0, outline=True)


def side_mouth(kind, mx, my):
    """옆모습 입 (주둥이 아래 앞쪽, mx·my = 입꼬리 앞 끝)."""
    if kind in ("open", "big", "roar", "yawn", "fire"):
        rx, ry = {"open": (4.2, 3.6), "big": (6, 5.4), "roar": (7.2, 7.4), "yawn": (6, 6.6), "fire": (4.6, 4.2)}[kind]
        return open_mouth(mx - rx, my - 1.2, rx, ry, tongue=kind != "fire")
    if kind == "o":
        return [([C(mx - 2.6, my, 2.6)], LINE), ([C(mx - 2.6, my, 1.6)], MOUTH_IN)]
    if kind == "chew":
        return [([L([(mx - 10, my - 0.5), (mx - 7, my + 1.2), (mx - 4, my - 0.5), (mx - 1, my + 1.2)], 1.3)], LINE)]
    if kind in ("frown", "pout"):
        return [([L(arc(mx - 4, my + 2.2, 4, 2.2, 200, 340, 8), 1.4)], LINE)]
    return [([L([(mx - 11, my - 0.8), (mx - 4, my + 0.8), (mx + 0.5, my - 1.6)], 1.4)], LINE)]


def front_mouth(kind, cx, my):
    if kind in ("open", "big", "roar", "yawn"):
        rx, ry = {"open": (4.6, 4.2), "big": (6.6, 6.4), "roar": (7, 7.4), "yawn": (5.6, 6.8)}[kind]
        return open_mouth(cx, my - 1, rx, ry)
    if kind == "o":
        return [([C(cx, my + 1, 2.6)], LINE), ([C(cx, my + 1, 1.6)], MOUTH_IN)]
    if kind == "chew":
        return [([L([(cx - 6, my), (cx - 3, my + 1.6), (cx, my), (cx + 3, my + 1.6), (cx + 6, my)], 1.3)], LINE)]
    if kind in ("frown", "pout"):
        return [([L(arc(cx, my + 2.4, 3.6, 2.0, 200, 340, 8), 1.4)], LINE)]
    return [([L(arc(cx, my - 1.6, 4.2, 3.0, 25, 155, 10), 1.4)], LINE)]


def dino(sp, *, side=True, cx=64.0, bottom=None, sq=(1.0, 1.0), rot=0.0, eyes="dot", mouth=None, brow=None, head=(0.0, 0.0),
         head_rot=0.0, arm=20.0, arms=None, legs=((0, 0), (0, 0)), tail_rot=0.0, fx=(), held=(), belly_up=False, ball=False,
         look_up=False, puff=False):
    """공룡 한 장. side: 오른쪽을 보는 옆모습 · bottom: 몸 아래끝 · legs: (뒤/왼, 앞/오른) 발 (dx, 들어올림) 또는 None ·
    arm: 옆모습 앞발 각도 (0 = 아래, 90 = 앞으로) · arms: 앞모습 [(쪽, 각도)] · head: 머리 (dx, dy) · head_rot: 고개 (음수 = 턱 듦) ·
    belly_up: 발라당 (앞모습, 발이 위로) · ball: 구르기 공 · look_up: 눈동자를 위로 · puff: 볼 빵빵 (불 뿜기 전 흡)."""
    pal = _pal(sp)
    col, spk, bel = pal["body"], pal["spike"], pal["belly"]
    st = _dn()
    hk, bk, lk = _ST["head"], _ST["body"], _ST["leg"]
    leg_show = 7.0 * lk
    bottom = FLOOR - leg_show if bottom is None else bottom
    fl = FLOOR if bottom >= FLOOR - leg_show - 3.5 else bottom + leg_show  # 발바닥 (뛰어오르면 몸과 함께 뜬다)
    parts, ops = [], []
    rx, ry = 25 * bk * _ST["torso"][0], 23 * bk * _ST["torso"][1]
    bcy = bottom - ry
    hr = 21.0 * hk
    sh = st["h"] * (0.85 + 0.15 * hk)
    with xf(rot=rot, pivot=(cx, bottom)), xf(sx=sq[0], sy=sq[1], pivot=(cx, bottom)):
        if side:
            bx = cx - 8
            hx, hy = bx + rx * 0.52 + head[0], bcy - ry * 0.8 - hr * 0.42 + head[1]
            tl = 21 * (0.55 + 0.45 * _ST["tail"])
            root = (bx - rx * 0.7, bcy + ry * 0.25)
            body_sh = [E(bx - rx, bcy - ry, bx + rx, bcy + ry)]
            with xf(rot=tail_rot, pivot=root):
                tail_sh = [T((bx - rx * 0.55, bcy - ry * 0.4), (bx - rx - tl, bcy + ry * 0.55), (bx - rx * 0.3, bcy + ry * 0.92)),
                           C(bx - rx - tl + 2.2, bcy + ry * 0.52, 2.6)]
                tail_tip = (bx - rx - tl, bcy + ry * 0.55)
                tail_base = (bx - rx * 0.6, bcy - ry * 0.3)
            parts.append(Part(body_sh + tail_sh, col, grow=1))
            # 등 가시: 목 뒤 → 등 → 꼬리 위 (꼬리 쪽으로 작아진다)
            spikes = []
            n = st["spikes"]
            for k in range(n):
                t = k / max(1, n - 1)
                a = 250 - 62 * t
                px, py, nx, ny = ell_point(bx, bcy, rx, ry, a)
                spikes.append(spike(px, py, nx, ny, sh * (1 - 0.3 * t), sh * 1.45 * (1 - 0.25 * t)))
            for k in range(2):
                t = 0.35 + 0.35 * k
                px, py = tail_base[0] + (tail_tip[0] - tail_base[0]) * t, tail_base[1] + (tail_tip[1] - tail_base[1]) * t
                dx, dy = tail_tip[0] - tail_base[0], tail_tip[1] - tail_base[1]
                ln = math.hypot(dx, dy)
                spikes.append(spike(px, py, dy / ln, -dx / ln, sh * (0.62 - 0.18 * k), sh * (0.8 - 0.2 * k)))
            parts.append(Part([], spk, grow=0, outline=True, spikes=spikes))
            # 뒷다리 (몸 뒤, 조금 짙게)
            if legs and not ball:
                (bdx, blift), (fdx, flift) = legs
                lx = bx - rx * 0.35 + bdx
                parts.append(Part([E(lx - 9, bottom - 14 - blift, lx + 9, bottom + 2 - blift),
                                   E(lx - 8, fl - 8 - blift, lx + 11, fl - blift)], _shade(col), grow=0, outline=True))
            belly_m = [([E(bx - rx * 0.05, bcy - ry * 0.55, bx + rx * 1.15, bcy + ry * 1.1)], bel),
                       ([E(bx - rx * 0.7, bcy - ry * 0.95, bx + rx * 0.1, bcy - ry * 0.45)], mix(col, WHITE, 0.22))]
            parts.append(Part(body_sh + tail_sh, col, belly_m, grow=0))
            if legs and not ball:
                lx = bx + rx * 0.3 + fdx
                parts.append(Part([E(lx - 12, bcy - ry * 0.05 - flift * 0.4, lx + 9, bottom + 1 - flift * 0.6),
                                   E(lx - 8, fl - 8.5 - flift, lx + 12, fl - flift)], col,
                                  [([L([(lx + 6, fl - 4.5 - flift), (lx + 6.5, fl - 2 - flift)], 0.9),
                                     L([(lx + 9, fl - 4.5 - flift), (lx + 9.5, fl - 2 - flift)], 0.9)], _shade(col))],
                                  grow=0, outline=True))
            # 머리 (가시 → 머리 → 짧은 앞발은 머리 아래 몸 앞)
            hpiv = (hx - hr * 0.4, hy + hr * 0.75)
            with xf(rot=head_rot, pivot=hpiv):
                head_sh = [E(hx - hr, hy - hr * 0.9, hx + hr * 0.9, hy + hr * 0.82),
                           E(hx - hr * 0.1, hy - hr * 0.62, hx + hr * 1.3, hy + hr * 0.78)]
                hsp = []
                for k in range(st["head_spikes"]):
                    a = [238, 205][k]
                    px, py, nx, ny = ell_point(hx - hr * 0.05, hy - hr * 0.04, hr * 0.95, hr * 0.86, a)
                    hsp.append(spike(px, py, nx, ny, sh * (0.9 - 0.12 * k), sh * (1.05 - 0.1 * k)))
                if hsp:
                    parts.append(Part([], spk, grow=0, outline=True, spikes=hsp))
                hmarks = [([E(hx - hr * 0.7, hy - hr * 0.86, hx + hr * 0.3, hy - hr * 0.35)], mix(col, WHITE, 0.28)),
                          ([E(hx - hr * 0.2, hy + hr * 0.42, hx + hr * 1.2, hy + hr * 0.95)], mix(col, bel, 0.45))]
                hpart = Part(head_sh, col, hmarks, grow=1)
            parts.append(hpart)
            if arm is not None and not ball:
                shx, shy = bx + rx * 0.62, bcy - ry * 0.32
                with xf(rot=-arm, pivot=(shx, shy)):
                    parts.append(Part([L([(shx, shy), (shx + 1.5, shy + 9 * (0.8 + 0.2 * bk))], 6.6)], col,
                                      [([E(shx - 3, shy - 3, shx + 1, shy + 4)], mix(col, WHITE, 0.25))], grow=0, outline=True))
            with xf(rot=head_rot, pivot=hpiv):
                if st["shell"]:
                    with xf(rot=-12, pivot=(hx - hr * 0.1, hy - hr * 0.8)):
                        parts.append(egg_shell(hx - hr * 0.1, hy - hr * 1.32, hr * 1.25))
                ex, ey = hx + hr * 0.42, hy - hr * 0.18
                face = list(stage_eye(eyes, ex, ey - (1.8 * _ST["eye"] if look_up else 0)))
                if brow:
                    face += brows(brow, ex, ey + _ST["eye_dy"], flip=True)
                bl = blush(hx + hr * 0.12, hy + hr * 0.26)
                if puff:
                    bl = [([E(hx - hr * 0.2, hy + hr * 0.0, hx + hr * 0.5, hy + hr * 0.6)], mix(col, (255, 170, 170), 0.35))] + bl
                face += bl
                face += [([C(hx + hr * 1.13, hy - hr * 0.2, 1.1)], _shade(mix(col, LINE, 0.4)))]
                face += side_mouth(mouth, hx + hr * 1.18, hy + hr * 0.46)
            ops += face
        else:
            fcx = cx
            rxf = rx * _ST["fbody"][0]
            hx, hy = fcx + head[0], bcy - ry * 0.72 - hr * 0.5 + head[1]
            body_sh = [E(fcx - rxf, bcy - ry, fcx + rxf, bcy + ry)]
            tail_sh = [] if belly_up or ball else [T((fcx + rxf * 0.5, bcy + ry * 0.2), (fcx + rxf + 13, bcy + ry * 0.75),
                                                    (fcx + rxf * 0.4, bcy + ry * 0.9))]
            parts.append(Part(body_sh + tail_sh, col, grow=1))
            late = []
            if legs and not ball:
                for j, (ldx, lift) in enumerate(legs):
                    s = -1 if j == 0 else 1
                    lx = fcx + s * rxf * 0.55 + ldx
                    if belly_up:
                        ly = bcy - ry * 0.1 - lift
                        late.append(Part([E(lx - 9, ly - 8, lx + 9, ly + 8), L([(lx, ly + 4), (fcx + s * rxf * 0.45, bcy + ry * 0.6)], 12)], col,
                                         [([E(lx - 5, ly - 6, lx + 5, ly - 1)], mix(col, bel, 0.6))], grow=0, outline=True))
                    else:
                        late.append(Part([E(lx - 11, bottom - 10 - lift, lx + 11, fl - lift)], col,
                                         [([L([(lx - 3.5, fl - 3 - lift), (lx - 3.5, fl - 0.5 - lift)], 0.9),
                                            L([(lx + 3.5, fl - 3 - lift), (lx + 3.5, fl - 0.5 - lift)], 0.9)], _shade(col))],
                                         grow=0, outline=True))
            belly_m = [([E(fcx - rxf * 0.62, bcy - ry * 0.45, fcx + rxf * 0.62, bcy + ry * 1.0)], bel)]
            parts.append(Part(body_sh + tail_sh, col, belly_m, grow=0))
            parts += late
            with xf(rot=head_rot, pivot=(hx, hy + hr * 0.85)):
                hsp = []
                n = 1 + 2 * min(1, st["head_spikes"]) if not st["shell"] else 0
                for k in range(n):
                    a = 270 + (k - (n - 1) / 2) * 26
                    px, py, nx, ny = ell_point(hx, hy - hr * 0.04, hr * 1.12, hr * 0.86, a)
                    hsp.append(spike(px, py, nx, ny, sh * (1.0 if a == 270 else 0.78), sh * 1.1))
                if hsp:
                    parts.append(Part([], spk, grow=0, outline=True, spikes=hsp))
                head_sh = [E(hx - hr * 1.15, hy - hr * 0.9, hx + hr * 1.15, hy + hr * 0.84)]
                hmarks = [([E(hx - hr * 0.62, hy + hr * 0.08, hx + hr * 0.62, hy + hr * 0.84)], mix(col, bel, 0.45)),
                          ([E(hx - hr * 0.7, hy - hr * 0.86, hx + hr * 0.1, hy - hr * 0.42)], mix(col, WHITE, 0.28))]
                parts.append(Part(head_sh, col, hmarks, grow=1))
                if st["shell"]:
                    parts.append(egg_shell(hx + hr * 0.15, hy - hr * 1.38, hr * 1.25))
                face = []
                for ex, flip in ((hx - hr * 0.5, False), (hx + hr * 0.5, True)):
                    face += stage_eye(eyes, ex, hy - hr * 0.2 - (1.8 * _ST["eye"] if look_up else 0), flip)
                    if brow:
                        face += brows(brow, ex, hy - hr * 0.2 + _ST["eye_dy"], flip=flip)
                face += blush(hx - hr * 0.8, hy + hr * 0.18) + blush(hx + hr * 0.8, hy + hr * 0.18)
                face += [([C(hx - 3, hy + hr * 0.3, 1.1), C(hx + 3, hy + hr * 0.3, 1.1)], _shade(mix(col, LINE, 0.4)))]
                face += front_mouth(mouth, hx, hy + hr * 0.55)
            if not ball:
                for s, a in (arms or [(-1, 30), (1, 30)]):
                    out = min(1.0, max(0.0, (a - 60) / 90))  # 번쩍 든 팔은 몸 옆으로 (볼에 가리지 않게)
                    shx, shy = fcx + s * rxf * (0.62 + 0.3 * out), bcy - ry * (0.35 + 0.1 * out)
                    with xf(rot=-s * a, pivot=(shx, shy)):
                        parts.append(Part([L([(shx, shy), (shx + s * 2.5, shy + (6.5 + 4 * out) * (0.8 + 0.2 * bk))], 7.4)], col, grow=0, outline=True))
            ops += face
        parts += [h() for h in held]
    return dp.compose(parts, ops, list(fx))


# =====================================================================
# 효과
# =====================================================================
def dust_fx(x, y, s=1.0):
    """발밑 먼지 퐁퐁 (작은 구름 셋)."""
    blobs = [C(x - 4 * s, y - 2 * s, 3.2 * s), C(x + 1 * s, y - 4 * s, 3.8 * s), C(x + 5.5 * s, y - 1.6 * s, 2.8 * s)]
    return [(blobs, DUST_LINE, 1.0), ([C(x - 4 * s, y - 2 * s, 2.2 * s), C(x + 1 * s, y - 4 * s, 2.8 * s), C(x + 5.5 * s, y - 1.6 * s, 1.8 * s)], DUST, 0)]


def smoke_fx(x, y, s=1.0):
    """동글 연기 한 가닥 (위로 갈수록 작게)."""
    return [([C(x, y, 2.6 * s), C(x + 2.5 * s, y - 4.5 * s, 2.0 * s), C(x + 1 * s, y - 8.5 * s, 1.4 * s)], SMOKE, 0.9)]


def flame_fx(x, y, ln, r):
    """입 앞의 동글 만화 불꽃 (오른쪽으로): 바깥 주황 · 가운데 노랑 · 속 연노랑. (x, y) = 입, ln = 길이, r = 굵기."""
    def tongue(k, dx=0.0):
        pts = [(x + dx, y - r * 0.35 * k)] + arc(x + ln * 0.6 + dx * 0.5, y, r * k, r * k, 270, 450, 12) + [(x + dx, y + r * 0.35 * k)]
        tip = [(x + ln * 0.62 + dx * 0.5, y - r * k * 0.95), (x + ln * (0.98 if k == 1 else 0.82), y - r * k * 0.25),
               (x + ln * 0.62 + dx * 0.5, y + r * k * 0.2)]
        return [T(*pts), T(*tip), C(x + ln * 0.6 + dx * 0.5, y, r * k)]
    return [(tongue(1.0), FLAME, 1.2), (tongue(0.68, 3), FLAME_MID, 0), (tongue(0.36, 6), FLAME_CORE, 0)]


def wave_fx(cx, cy, n, r0=6.0):
    """크앙 소리 물결 ))) — 입 앞에서 퍼진다."""
    return [([L(arc(cx, cy, r0 + 5 * k, r0 + 5 * k, -38, 38, 8), 1.9)], WAVE, 1.0) for k in range(n)]


def impact_fx(cx, y):
    """쿵! 바닥 충격선 (양옆으로 짧은 선)."""
    return [([L([(cx - 26, y - 2), (cx - 33, y - 7)], 1.6), L([(cx - 24, y - 7), (cx - 28, y - 14)], 1.6),
              L([(cx + 26, y - 2), (cx + 33, y - 7)], 1.6), L([(cx + 24, y - 7), (cx + 28, y - 14)], 1.6)], WAVE, 1.0)]


# ---- 좋아하는 음식 생각 풍선 ----
MELON, MELON_RIND, MELON_PALE, SEED = (252, 108, 120), (110, 190, 92), (236, 252, 214), (70, 50, 52)
BROC, BROC_DARK, BROC_STEM = (112, 186, 92), (80, 150, 70), (186, 222, 150)
GRAPE, GRAPE_DARK, GRAPE_LEAF = (160, 110, 200), (124, 80, 166), (128, 196, 104)


def melon_bubble_fx(cx, cy):
    fx = _bubble(cx, cy)
    rind = arc(cx, cy - 3, 8.5, 9, 0, 180, 12)
    fx += [([T(*rind)], MELON_RIND, 0), ([T(*arc(cx, cy - 3, 7.0, 7.4, 0, 180, 12))], MELON_PALE, 0),
           ([T(*arc(cx, cy - 3, 6.0, 6.4, 0, 180, 12))], MELON, 0),
           ([C(cx - 2.5, cy + 0.2, 0.8), C(cx + 2.5, cy + 0.2, 0.8), C(cx, cy - 1.2, 0.8)], SEED, 0)]
    return fx


def broc_bubble_fx(cx, cy):
    fx = _bubble(cx, cy)
    fx += [([T((cx - 2.6, cy), (cx + 2.6, cy), (cx + 2, cy + 8), (cx - 2, cy + 8))], BROC_STEM, 0),
           ([C(cx - 4.2, cy - 1, 3.6), C(cx + 4.2, cy - 1, 3.6), C(cx, cy - 4.5, 4.2)], BROC_DARK, 0),
           ([C(cx - 4.2, cy - 1.5, 2.6), C(cx + 4.2, cy - 1.5, 2.6), C(cx, cy - 5, 3.0)], BROC, 0)]
    return fx


def grape_bubble_fx(cx, cy):
    fx = _bubble(cx, cy)
    pts = [(-3.6, -3.5), (0, -4), (3.6, -3.5), (-1.8, 0), (1.8, 0), (0, 3.6)]
    fx += [([E(cx + 1, cy - 9.5, cx + 7, cy - 6)], GRAPE_LEAF, 0),
           ([C(cx + x, cy + y, 2.7) for x, y in pts], GRAPE_DARK, 0), ([C(cx + x - 0.4, cy + y - 0.4, 2.0) for x, y in pts], GRAPE, 0)]
    return fx


FOOD_BUBBLE = {"dino_yellow": melon_bubble_fx, "dino_green": broc_bubble_fx, "dino_purple": grape_bubble_fx}


# =====================================================================
# 포즈
# =====================================================================
def dn_walk(sp, i):
    """쿵쿵 뒤뚱: 몸이 앞뒤로 기우뚱, 발 번갈아, 꼬리 살랑."""
    legs = [((-2, 5), (2, 0)), ((0, 1), (0, 0)), ((2, 0), (-2, 5)), ((0, 0), (0, 1))][i]
    return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - [0, 1.5, 0, 1.5][i], rot=[-3, 0, 3, 0][i], head_rot=[2, 0, -2, 0][i],
                legs=legs, tail_rot=[-8, 0, 8, 0][i], arm=[10, 20, 30, 20][i])


def dn_run(sp, i):
    """쿵쾅쿵쾅 달리기: 크게 통통 튀고 발이 닿는 순간 먼지 퐁."""
    up = [0, 7, 10, 4][i]
    legs = [((-4, 0), (4, 9)), ((-2, 6), (3, 4)), ((4, 9), (-4, 0)), ((3, 4), (-2, 6))][i]
    fx = speed_fx(2, (60, 78, 96), 11, [0, -3, 0, -3][i])
    if i in (0, 2):
        fx += dust_fx(32 if i == 0 else 40, FLOOR - 1, 1.0) + dust_fx(76, FLOOR - 1, 0.7)
    return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - up, sq=[(1.06, 0.93), (0.97, 1.04), (1.06, 0.93), (0.98, 1.02)][i],
                rot=-6, eyes="happy", mouth="open", legs=legs, tail_rot=[-12, 4, -12, 4][i], arm=60, fx=fx)


def dn_roll(sp, i):
    """동글 공으로 말려 데굴데굴 (앱이 돌린다): 등 가시가 공 둘레에."""
    pal = _pal(sp)
    cx, cy = ROLL_CENTER
    r = 31.0
    with view(scale=1.0, lift=0):
        spikes = []
        for k in range(6):
            a = 150 + 36 * k
            px, py, nx, ny = ell_point(cx, cy, r, r, a)
            spikes.append(spike(px, py, nx, ny, 7.5, 9))
        parts = [Part([C(cx, cy, r)], pal["body"], grow=1),
                 Part([], pal["spike"], grow=0, outline=True, spikes=spikes),
                 Part([C(cx, cy, r)], pal["body"], [([C(cx + 8, cy + 8, r * 0.62)], pal["belly"]),
                                                     ([E(cx - 20, cy - 24, cx - 2, cy - 12)], mix(pal["body"], WHITE, 0.28))], grow=0),
                 Part([E(cx + 10, cy + 18, cx + 26, cy + 30)], pal["body"], grow=0, outline=True),
                 Part([E(cx - 4, cy + 20, cx + 12, cy + 32)], pal["body"], grow=0, outline=True)]
        ops = stage_eye("happy", cx + 6, cy - 6) + stage_eye("happy", cx + 20, cy - 4, True)
        ops += blush(cx + 2, cy + 3) + blush(cx + 24, cy + 5)
        ops += front_mouth("open" if i else None, cx + 13, cy + 5)
        return dp.compose(parts, ops)


def dn_idle(sp, i):
    return dino(sp, sq=(1.0, 1.0 + 0.02 * i), head=(0, -0.8 * i), tail_rot=-3 * i)


def dn_look(sp, i):
    return dino(sp, head_rot=[-8, 0, 9, 0][i], eyes="closed" if i == 3 else "dot", tail_rot=[0, 4, 0, -4][i])


def dn_lookup(sp, i):
    """올려다보기: 턱을 들고 눈동자를 위로 (✧ 마지막 프레임)."""
    fx = sparkle_fx(112, 14, 4.6) if i == 2 else []
    return dino(sp, head_rot=[-12, -18, -18][i], head=(-1, -1 * min(i, 1)), look_up=True, fx=fx)


def dn_sleep(sp, i):
    zs = [z_fx(104, 58, 3.4), z_fx(115, 47, 2.6)] if i == 0 else [z_fx(106, 55, 3.6), z_fx(118, 43, 2.8)]
    return dino(sp, bottom=FLOOR - 1, sq=(1.1, 0.86 + 0.012 * i), eyes="closed", head=(3, 9), head_rot=12, legs=None, arm=70,
                tail_rot=-6, fx=zs)


def dn_annoyed(sp, i):
    return dino(sp, bottom=FLOOR - 1, sq=(1.05, 0.92), eyes="squint", mouth="pout", legs=None, head=(0, 3), arm=40,
                fx=[anger_fx(106, 30 - i, 7.0)] + smoke_fx(116, 44 - 2 * i, 0.8))


def dn_hop(sp, i):
    if i == 0:
        return dino(sp, sq=(1.1, 0.86), eyes="tight", head=(0, 3), arm=10)
    if i == 1:
        return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - 16, sq=(0.94, 1.07), rot=-6, eyes="happy", mouth="open",
                    legs=((-2, 4), (2, 6)), arm=90, tail_rot=10)
    return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - 9, eyes="happy", legs=((-2, 2), (2, 3)), arm=60, tail_rot=4)


def dn_fall(sp, i):
    stars = _stars([(100, 22, 3.6), (26, 30, 3.0)] if i == 0 else [(30, 20, 3.6), (104, 32, 3.0)])
    return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - 10, rot=[-10, 10][i], eyes="spiral", mouth="open", legs=((-3, 6), (3, 4)),
                arm=[150, 110][i], tail_rot=[14, -14][i], fx=stars)


def dn_dragged(sp, i):
    s = [-1, 1][i]
    return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - 10, rot=5 * s, eyes="tight", mouth="open", legs=((-3 + 2 * s, 4), (3 + 2 * s, 5)),
                arm=140, tail_rot=10 * s, fx=sweat_fx(106, 30))


def dn_land(sp, i):
    if i == 0:
        return dino(sp, sq=(1.12, 0.84), eyes="tight", arm=70, fx=dust_fx(30, FLOOR - 1, 0.8) + dust_fx(96, FLOOR - 1, 0.8))
    return dino(sp, sq=(1.02, 0.98), arm=30)


def dn_sit_front(sp, i):
    return dino(sp, side=False, sq=(1.0, 1.0 + [0, 0.01, 0.02, 0.01][i]), eyes="closed" if i == 3 else "dot")


def dn_wave(sp, i):
    return dino(sp, side=False, arms=[(-1, 15), (1, [130, 160, 130, 160][i])], rot=[-2, -1, -2, -1][i], mouth="open" if i % 2 else None)


def dn_tilt(sp, i):
    return dino(sp, side=False, head_rot=dp._swing([8, 12, 12][i]), fx=[question_fx(18, 22 + [2, 0, -2][i])])


def dn_belly(sp, i):
    wig = [0, 2, 0, -2][i]
    return dino(sp, side=False, belly_up=True, bottom=FLOOR - 1, sq=(1.12, 0.82), rot=-5, eyes="happy", mouth="open",
                legs=((-2 + wig, 2), (2 - wig, 2 + wig)), arms=[(-1, 120 + 6 * wig), (1, 120 - 6 * wig)])


def dn_stretch(sp, i):
    if i == 0:
        return dino(sp, side=False, sq=(0.95, 1.07), eyes="tight", arms=[(-1, 165), (1, 165)], head=(0, -3))
    if i == 1:
        return dino(sp, side=False, eyes="tight", mouth="yawn", arms=[(-1, 100), (1, 100)], rot=-3)
    return dino(sp, side=False)


def dn_heart(sp, i):
    hs = [heart_fx(112, 40 - 6 * i, 4.6 + 0.6 * i), heart_fx(16, 46 - 6 * i, 4 + 0.5 * i)]
    if i:
        hs.append(heart_fx(114, 18 - 3 * i, 2.6 + 0.6 * i))
    hy = FLOOR - 7 * _ST["leg"] - 23 * _ST["body"] * 0.7
    held = [lambda: Part([C(64 - 6, hy, 7), C(64 + 6, hy, 7), T((51, hy + 2), (77, hy + 2), (64, hy + 14))], HEART,
                         [([E(53, hy - 5, 57, hy - 1)], WHITE)], grow=0, outline=True)]
    return dino(sp, side=False, eyes="happy", arms=[(-1, 55), (1, 55)], held=held, fx=hs)


def dn_tickle(sp, i):
    return dino(sp, side=False, rot=dp._swing([-7, 7, 0][i]), eyes="happy", mouth="big" if i < 2 else "open",
                arms=[(-1, [60, 120, 90][i]), (1, [120, 60, 90][i])])


def dn_hungry(sp, i):
    return dino(sp, side=False, rot=-4, eyes="closed", mouth="frown", brow="sad", head=(0, [0, 1, 0][i] + 2),
                arms=[(-1, 10), (1, [25, 35, 25][i])], fx=FOOD_BUBBLE[sp.id](106, 18))


def dn_eat(sp, i):
    """냠냠 (그릇 없음): 입이 그릇 밥 더미 바로 위 (프레임 (64, 84))로 오도록 머리를 숙인다."""
    bite = i % 2 == 0
    mx, my = dp._frame_to_pre(64, 84 + [0, 1, 0, 1][i])
    bottom = FLOOR - 7 * _ST["leg"]
    ry = 23 * _ST["body"] * _ST["torso"][1]
    hr = 21.0 * _ST["head"]
    hy0 = bottom - ry - ry * 0.72 - hr * 0.5
    want = my - hr * 0.55
    with view(scale=1.0, lift=0):
        crumbs = [[(40, 84), (88, 80)], [(34, 76), (93, 72), (86, 86)], [(42, 82), (90, 78)], [(36, 74), (94, 70), (44, 86)]][i]
        fx = []
        for x, y in crumbs:
            fx += [([C(x, y, 2.4)], LINE, 0), ([C(x, y, 1.5)], (255, 236, 200), 0)]
    return dino(sp, side=False, head=(0, want - hy0), head_rot=[0, -2, 0, 2][i], eyes="happy" if bite else "closed",
                mouth="open" if bite else "chew", arms=[(-1, 40), (1, 40)], fx=fx)


def dn_level_up(sp, i):
    stars = _stars([[(14, 40, 5.5), (114, 34, 4.0), (16, 86, 3.6), (112, 84, 5.0)],
                    [(16, 30, 4.2), (112, 26, 6.0), (12, 74, 5.0), (116, 74, 3.6)],
                    [(14, 50, 6.0), (114, 44, 4.6), (20, 18, 3.8), (108, 14, 4.4)]][i])
    up = [150, 165, 175][i]
    return dino(sp, side=False, bottom=FLOOR - 7 * _ST["leg"] - [0, 1, 3][i], eyes="happy", mouth="big", arms=[(-1, up), (1, up)], fx=stars)


# ---- 공룡만의 동작 ----
FIRE_SHIFT = -7  # 불꽃 자리를 내려고 몸을 뒤로 물린다


def _mouth_frame(head_rot=0.0, head=(0.0, 0.0)):
    """옆모습 입의 (머리 좌표) 자리 — 불꽃·소리 물결을 입 앞에 놓는다."""
    hk, bk = _ST["head"], _ST["body"]
    bottom = FLOOR - 7 * _ST["leg"]
    rx, ry = 25 * bk * _ST["torso"][0], 23 * bk * _ST["torso"][1]
    bcy = bottom - ry
    hr = 21.0 * hk
    bx = 64 + FIRE_SHIFT - 8
    hx, hy = bx + rx * 0.52 + head[0], bcy - ry * 0.8 - hr * 0.42 + head[1]
    px, py = hx - hr * 0.4, hy + hr * 0.75
    mx, my = hx + hr * 1.25, hy + hr * 0.4
    r = math.radians(head_rot)
    return px + (mx - px) * math.cos(r) - (my - py) * math.sin(r), py + (mx - px) * math.sin(r) + (my - py) * math.cos(r)


def dn_fire(sp, i):
    """불 뿜기: 0 흡 (볼 빵빵, 고개 뒤로) → 1 작은 불꽃 퐁 → 2 화르륵 + 연기 → 3 흐뭇 (연기 한 가닥)."""
    head_rot = [-8, 2, 4, 0][i]
    mx, my = _mouth_frame(head_rot)
    fx = []
    if i == 1:
        fx = flame_fx(mx + 1, my, 15, 6.0)
    elif i == 2:
        fx = flame_fx(mx + 1, my, 21, 8.0) + smoke_fx(mx + 16, my - 12, 0.9)
    elif i == 3:
        fx = smoke_fx(mx + 2, my - 4, 0.8)
    look = [dict(eyes="tight", mouth="o", puff=True), dict(eyes="dot", mouth="fire"), dict(eyes="tight", mouth="fire"),
            dict(eyes="happy", mouth=None)][i]
    return dino(sp, cx=64 + FIRE_SHIFT, head_rot=head_rot, sq=[(0.98, 1.03), (1.02, 0.98), (1.03, 0.97), (1.0, 1.0)][i],
                arm=[40, 60, 70, 20][i], tail_rot=[-6, -2, 0, 0][i], fx=fx, **look)


def dn_roar(sp, i):
    """크앙!: 고개를 들고 입을 크게, 입 앞에 소리 물결이 퍼진다 (몸이 살짝 떨림)."""
    head_rot = [-14, -18, -16][i]
    mx, my = _mouth_frame(head_rot)
    return dino(sp, cx=64 + FIRE_SHIFT + [0, 1, -1][i], head_rot=head_rot, eyes="tight", mouth="roar", brow="angry" if i else None,
                arm=[80, 95, 85][i], tail_rot=[10, 14, 8][i], fx=wave_fx(mx + 2, my - 3, [2, 3, 3][i], [6, 6, 9][i]))


def dn_stomp(sp, i):
    """제자리 쿵쾅쿵쾅: 0 앞발 번쩍 → 1 쿵! (찌그러짐 + 먼지 + 충격선) → 2 뒷발 번쩍 → 3 쿵!"""
    if i in (0, 2):
        legs = ((0, 0), (2, 11)) if i == 0 else ((-2, 11), (0, 0))
        return dino(sp, bottom=FLOOR - 7 * _ST["leg"] - 3, rot=[-6, 0, 6, 0][i], eyes="tight", mouth="open", legs=legs, arm=70,
                    tail_rot=[-10, 0, 10, 0][i])
    fx = dust_fx(28, FLOOR - 1, 0.95) + dust_fx(98, FLOOR - 1, 0.95) + impact_fx(64, FLOOR - 1)
    return dino(sp, sq=(1.1, 0.88), eyes="happy", mouth="big", arm=30, tail_rot=[0, 6, 0, -6][i], fx=fx)


DINO_ANIMS = ("lookup", "fire", "roar", "stomp")  # 공룡의 선택 동작 (OPTIONAL_ANIMS에 있는 것만 시트에 넣는다)
DINO_POSES = {"walk": dn_walk, "run": dn_run, "roll": dn_roll, "idle": dn_idle, "look": dn_look, "sleep": dn_sleep, "hop": dn_hop,
              "fall": dn_fall, "dragged": dn_dragged, "land": dn_land, "sit_front": dn_sit_front, "wave": dn_wave, "tilt": dn_tilt,
              "belly": dn_belly, "stretch": dn_stretch, "heart": dn_heart, "tickle": dn_tickle, "annoyed": dn_annoyed,
              "hungry": dn_hungry, "eat": dn_eat, "level_up": dn_level_up, "lookup": dn_lookup, "fire": dn_fire,
              "roar": dn_roar, "stomp": dn_stomp}
DINO_POSES = {k: (v if k == "roll" else _fit(v)) for k, v in DINO_POSES.items()}

_COMMON = dict(front_ears=[], front_ear_pivots=(), front_ear_marks=[], front_head_marks=[], front_body_marks=[], front_tail=[],
               front_tail_root=(64, 112), side_ears=[], ears_over=False, side_head_marks=[], side_torso_marks=[], side_tail=[],
               tail_root=(28, 108))
# 작은 점 눈 (참고 인형처럼) — 아기·유아는 그래도 반짝이 눈
_TWEAK = {"baby": dict(pale=0.0, eye=1.05, fit=0.88), "kinder": dict(pale=0.0, eye=0.95, fit=0.92),
          "child": dict(pale=0.0, eye=0.9), "teenager": dict(eye=0.86), "adult": dict(eye=0.86, fit=0.93)}


def _specials():
    have = {a for a, *_ in OPTIONAL_ANIMS}
    return [a for a in DINO_ANIMS if a in have]


def _dino_species(pid, name):
    pal = PALETTES[pid]
    return Species(id=pid, name=name, walk_speed=72, scale=0.94, base=pal["body"], back=pal["body"], belly=pal["belly"],
                   nose=pal["line"], mouth="w", front_tail_color=pal["body"], tail_color=pal["body"],
                   stage_size=dict.fromkeys(STAGES, 1.0), stripe=pal["spike"], outline=pal["line"], stage_tweak=_TWEAK,
                   specials=_specials(), poses=DINO_POSES, **_COMMON)


DINO_YELLOW = _dino_species("dino_yellow", "노란 공룡")
DINO_GREEN = _dino_species("dino_green", "초록 공룡")
DINO_PURPLE = _dino_species("dino_purple", "보라 공룡")
DINOS = (DINO_YELLOW, DINO_GREEN, DINO_PURPLE)


def register():
    for sp in DINOS:
        dp.SPECIES[sp.id] = sp


register()


def preview_dinos(by_id, path, bg=(255, 255, 255)):
    """세 색 × 다섯 단계 × 대표 동작 (+ 공룡 동작) + 화면 표시 크기 + sit_front·walk 2배 확대."""
    from PIL import ImageDraw, ImageFont
    from sprite_spec import STAGE_SCALE
    names = [("walk", 0), ("walk", 2), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("tickle", 0),
             ("hungry", 0), ("belly", 0), ("roll", 0), ("dragged", 0)]
    names += [(a, k) for a, n in (("lookup", 3), ("fire", 4), ("roar", 3), ("stomp", 4)) if a in DINO_YELLOW.specials
              for k in (range(n) if a in ("fire", "stomp") else (n - 1,))]
    cell, label, row_h = FRAME + 4, 110, 2 * FRAME + 8
    x_disp = label + len(names) * cell + 16
    x_zoom = x_disp + cell + 16
    W = x_zoom + 2 * (2 * FRAME + 8)
    rows = [(pid, st) for pid in by_id for st in STAGES]
    H = 24 + len(rows) * row_h
    img = Image.new("RGBA", (W, H), bg + (255,))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    for c, (anim, k) in enumerate(names):
        d.text((label + c * cell, 6), f"{anim} {k}", fill=(60, 60, 60), font=font)
    d.text((x_disp, 6), "(display)", fill=(60, 60, 60), font=font)
    d.text((x_zoom, 6), "sit_front x2 / walk x2", fill=(60, 60, 60), font=font)
    for r, (pid, st) in enumerate(rows):
        by_stage = by_id[pid]
        y = 24 + r * row_h
        base = y + row_h - FRAME - 8
        d.text((4, base + FRAME // 2), f"{pid[5:]} {st}", fill=(60, 60, 60), font=font)
        for c, (anim, k) in enumerate(names):
            img.alpha_composite(by_stage[st][anim][k], (label + c * cell, base))
        s = STAGE_SCALE[st] / STAGE_SCALE["adult"]
        small = by_stage[st]["sit_front"][0].resize((round(FRAME * s), round(FRAME * s)), Image.LANCZOS)
        img.alpha_composite(small, (x_disp + (FRAME - small.width) // 2, base + FRAME - small.height))
        for z, (anim, k) in enumerate([("sit_front", 0), ("walk", 0)]):
            big = by_stage[st][anim][k].resize((2 * FRAME, 2 * FRAME), Image.LANCZOS)
            img.alpha_composite(big, (x_zoom + z * (2 * FRAME + 8), y))
    img.save(path)


def main():
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_id = {}
    for sp in DINOS:
        by_id[sp.id] = dp.write_species(sp, root / "ui" / "pets" / sp.id)
    preview_dinos(by_id, pv / "stages_dino.png")
    print("ok:", ", ".join(sp.id for sp in DINOS))


if __name__ == "__main__":
    main()
