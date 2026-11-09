"""픽셀(네모네모) 그림 — 완성된 PNG를 변환하지 않고, 그림 코드에서 바로 도트를 찍는다 (모든 동물, 다섯 단계, 도는 roll 빼고 모든 동작).

그림 파일(draw_pets 등)은 한 줄도 고치지 않는다. pixel_mode() 안에서만 compose·stage_eye·blush·z_fx·heart_fx… 를 도트 버전으로
바꿔 끼우고, 나오면 되돌린다 (이 문맥이 곧 플래그). 포즈·뼈대·종별 설정은 동글 그림과 같은 코드를 그대로 쓰므로:
  - 눈·입·볼·효과는 "찾아내는" 게 아니라 코드가 넘겨준 자리(현재 변환 적용)에 도장(Stamp)으로 찍는다.
  - 색은 칸 평균이 아니라 부품 순서대로 설정 색을 그대로 칠하므로 탁해지지 않는다.
  - 외곽선은 채운 칸 바깥 1칸. 가는 줄무늬·발가락은 칸에 맞춘 1칸 선, 작은 점(젤리)은 1칸 점으로 남긴다.
  - 반투명·번짐이 없다: 유령은 불투명 우유빛, 호박 불빛은 노란 칸, 루돌프 코는 빨간 칸 + 반짝 칸.
  - 프레임 하나의 크기로 칸 크기를 정하고, 높이 뛴 프레임은 창을 정수 칸만큼 옮긴다 (_fit_shift).

    python tools/draw_pixel.py [묶음…] [--no-preview]
      → ui/pets_pixel/<id>/{sheet_<stage>.png ×5, pet.json} (기린은 neck_<stage>.png, neck_tile_<stage>.png 도)
      → tools/preview/pixel_<묶음>.png (실제 표시 크기 + 확대, 밝은·어두운 바탕). 묶음: GROUPS

단계마다 칸 수 × 화면 배율 = 표시 크기 (STAGE_GRID). 동글 그림은 바이트까지 그대로다 (test_draw_pixel.py).
"""

import json
import math
import os
import sys
import time
from contextlib import contextmanager
from dataclasses import dataclass, replace
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_christmas as dc  # noqa: E402
import draw_dino as dd  # noqa: E402
import draw_farm as dfarm  # noqa: E402
import draw_frog as df  # noqa: E402
import draw_halloween as dh  # noqa: E402
import draw_snowman as ds  # noqa: E402
import draw_bird as dbird  # noqa: E402
import draw_penguin as dpeng  # noqa: E402
import draw_squirrel as dsq  # noqa: E402
import draw_redpanda as dred  # noqa: E402

K = dp.K
OUT_ROOT = Path(__file__).resolve().parent.parent / "ui" / "pets_pixel"
NO_ANIM = {"roll"}
# 도는 구르기는 도트로 돌리면 무너져서 픽셀에서 빠진다. 돌지 않는 배 미끄럼(sprite_spec NO_SPIN 중 펭귄·눈사람)만 roll 줄을 그대로 그린다
# — 픽셀 pet.json에는 spin이 없어 앱도 돌리지 않는다. 산타 썰매·기린 목 늘이기는 NO_SPIN이어도 픽셀에서는 아직 없다
PIXEL_ROLL = {"penguin", "snowman"}
assert PIXEL_ROLL <= dp.NO_SPIN


def dropped_anims(sp):
    """이 동물의 픽셀 시트에서 빠지는 동작."""
    return NO_ANIM - {"roll"} if sp.id in PIXEL_ROLL else NO_ANIM
# 단계 → (캔버스 칸 수, 화면 배율). 표시 크기 = 칸 × 배율
STAGE_GRID = {"baby": (24, 3), "kinder": (28, 3), "child": (32, 3), "teenager": (32, 3), "adult": (36, 3)}
# 종마다 다른 칸 수 (칸 크기는 그대로라 점이 고르다): 공룡은 픽셀에서만 조금 작다 — 60/72/84/84/96 px. 앱은 pet.json frame_sizes를 따른다
_DINO_DOTS = {"baby": 20, "kinder": 24, "child": 28, "teenager": 28, "adult": 32}
SPECIES_DOTS = {"dino_yellow": _DINO_DOTS, "dino_green": _DINO_DOTS, "dino_purple": _DINO_DOTS}


def stage_dots(pid, stage):
    """그 동물·단계 시트 한 칸의 칸 수."""
    return SPECIES_DOTS.get(pid, {}).get(stage, STAGE_GRID[stage][0])


EYE_COL = (58, 38, 42)
SHINE = (255, 255, 255)
IRIS = (138, 96, 92)
BLUSH_COL = (250, 158, 164)
Z_COL = (112, 144, 214)
ANGER_COL = (228, 76, 76)
STAR_COL, STAR_EDGE = (255, 222, 92), (236, 158, 64)
SMALL_PART = 18  # 칸 수가 이보다 적은 겹친 조각(앞발): 테두리를 옅게, 발가락 선 없이
PALE_BASE = 700  # 이보다 밝은 (R+G+B) 조각의 고리는 옅게
INK_DARK = 0.70  # 외곽선: 설정 외곽선 색을 이만큼 어둡게 (도트는 1칸뿐이라 더 진해야 또렷하다)


@dataclass
class Grid:
    """128 프레임 → n×n 칸. c: 칸 한 변(슈퍼샘플 픽셀, 정수 → 칸 경계가 정확하고 좌우 대칭)."""
    n: int
    c: int
    x0: int  # 칸 (0,0)의 슈퍼샘플 좌표
    y0: int

    @property
    def cell(self):
        return self.c / K

    @property
    def ox(self):
        return self.x0 / K

    @property
    def oy(self):
        return self.y0 / K

    def to_dot_x(self, x):
        return (x - self.ox) / self.cell

    def to_dot_y(self, y):
        return (y - self.oy) / self.cell

    def down(self, m):
        """슈퍼샘플 마스크 → 칸마다 덮인 비율 (0~255)."""
        box = (self.x0, self.y0, self.x0 + self.c * self.n, self.y0 + self.c * self.n)
        return np.asarray(m.crop(box).reduce(self.c))


_PX = {"grid": None, "collect": None, "last_dx": 0}  # pixel_mode 상태
_OWNERS = {  # 바꿔 끼우는 이름 → 원래 있는 모듈
    dp: ("compose", "stage_eye", "blush", "open_mouth", "front_mouth", "side_mouth", "brows", "z_fx", "heart_fx", "question_fx", "anger_fx",
         "sparkle_fx", "thought_fx", "E", "L", "WHISKERS", "back_arm", "front_paw", "back_tail"),
    dh: ("_finish", "pk_face", "_heart_items", "candy_bubble_fx", "lolly_bubble_fx"),
    dc: ("_nose_glow", "_apple_bubble", "milk_bubble_fx", "_beard_fns", "_mitt_arm"),
    df: ("fly_bubble_fx",),
    dfarm: ("bread_bubble_fx", "leaf_bubble_fx", "duck_bill", "GF_STAGE", "neck_geometry"),
    dd: ("melon_bubble_fx", "broc_bubble_fx", "grape_bubble_fx", "FOOD_BUBBLE", "wave_fx", "dust_fx", "impact_fx"),
    ds: ("ice_bubble_fx",),
    dbird: ("seeds_bubble_fx",),
    dpeng: ("shrimp_bubble_fx",),
    dsq: ("acorn_bubble_fx",),
    dred: ("bamboo_bubble_fx",),
}
_ORIG = {name: getattr(mod, name) for mod, names in _OWNERS.items() for name in names}
_ORIG_E, _ORIG_L = _ORIG["E"], _ORIG["L"]


# ---------------------------------------------------------------- 도장
@dataclass
class Stamp:
    """코드가 아는 자리 (x, y: 칸 좌표의 중심)에 찍는 도트 그림. rows: 문자열 줄, pal: 문자 → 색, '.'은 비움.
    where: 'in' 몸 안에만 (안쪽으로 밀어 넣음) · 'out' 몸 밖 (비는 자리를 찾음)."""
    x: float
    y: float
    rows: list
    pal: dict
    where: str = "in"
    halo: tuple = None
    anchor_top: bool = False
    alts: tuple = ()  # 자리가 없을 때 대신 시도할 더 작은 그림
    optional: bool = False  # 그래도 자리가 없으면 찍지 않는다 (몸 위에 겹치는 것보다 낫다)

    def __call__(self, d):  # compose가 아닌 곳(shapes)에 흘러가도 아무것도 그리지 않는다
        return None


def _stamp(x, y, rows, pal, **kw):
    fx, fy = dp._pt(x, y)
    grid = _PX["grid"]
    return Stamp(grid.to_dot_x(fx / K), grid.to_dot_y(fy / K), rows, pal, **kw)


EYE_PAL = {"#": EYE_COL, "o": SHINE, "i": IRIS, "l": dp.LINE, "w": SHINE}


DARK_EYE_PAL = dict(EYE_PAL, **{"#": (30, 18, 20)})  # 짙은 외곽선 동물: 눈이 털에 묻히지 않게 더 까맣게
DARK_INK = 190


def _mirror_rows(rows):
    return [r[::-1] for r in rows]


def _wide():
    return _PX["grid"].n >= 32


def _eye_rows(kind, flip=False):
    n = _PX["grid"].n
    wide = _wide()
    if kind == "dot" and _ST_get("sleepy"):  # 반쯤 감은 졸린 눈 (페르시안): 넓은 눈꺼풀 한 줄 아래 반쪽 눈
        if _ST_get("sparkle"):
            return ["####", "o##.", ".#i."] if n >= 28 else ["###", "o#i"]
        return ["####", ".o#."] if wide else ["###", ".#."]
    if kind in ("dot", "up"):
        if _ST_get("sparkle"):
            return ["o##", "###", "##i"] if n < 28 else ["o##", "###", "###", "#i#"]
        return ["o#", "##", "##"] if wide else ["o#", "##"]
    if kind == "closed":
        return ["#..#", ".##."] if wide else ["#.#", ".#."]
    if kind == "happy":
        return [".##.", "#..#"] if wide else [".#.", "#.#"]
    if kind == "squint":
        return ["####"] if wide else ["###"]
    if kind == "flat":
        return ["####", ".##."] if wide else ["###", ".#."]
    if kind == "tight":
        rows = ["#..", ".##", "#.."]
        return _mirror_rows(rows) if flip else rows
    if kind == "round":  # 놀란 눈 (떨어질 때): 동그랗게 뜨되 반짝이를 남긴다 — 뻥 뚫린 고리는 무섭다
        return ["o##", "###", "##i"] if _PX["grid"].n >= 28 else ["o#", "##"]
    if kind == "spiral":  # 어질어질: 소용돌이는 칸에서 네모 구멍이 된다 → X 눈
        return ["#.#", ".#.", "#.#"]
    if kind == "big":
        return ["o##", "###", "###", "##i"]
    raise ValueError(kind)


def _ST_get(key):
    return dp._ST.get(key)


def stage_eye(kind, cx, cy, flip=False):
    cy += dp._ST["eye_dy"]
    if kind == "up":
        cy -= 2.0
    pal = DARK_EYE_PAL if sum(dp._INK["line"]) < DARK_INK else EYE_PAL
    return [([_stamp(cx, cy, _eye_rows(kind, flip), pal)], None)]


def blush(cx, cy):
    n = _PX["grid"].n
    rows = ["bbb"] if n >= 28 else ["bb"]
    return [([_stamp(cx, cy, rows, {"b": BLUSH_COL})], None)]


MOUTH_PAL = {"l": dp.LINE, "r": dp.MOUTH_IN, "t": dp.TONGUE, "w": SHINE}


def open_mouth(cx, top, rx, ry, tongue=True):
    rows = ["llll", "lrrl", "lttl" if tongue else "lrrl", ".ll."] if _wide() else ["lrrl", ".tt."]
    return [([_stamp(cx, top + ry * 0.5, rows, MOUTH_PAL)], None)]


FRONT_MOUTHS = {  # 넓은 격자 (32칸 이상), 좁은 격자 — 첫 줄이 코
    None: (["..nn..", "l.ll.l", ".l..l."], ["nn", "ll"]),
    "smile": ([".nn.", "lrrl", ".ll."], ["nn", "rr"]),
    "frown": ([".nn.", "....", ".ll.", "l..l"], ["nn", "..", "ll"]),
    "pout": ([".nn.", "....", "..ll", ".l.."], ["nn", ".l", "l."]),
    "chew": (["..nn..", "l.ll.l", ".l..l."], ["nn", "ll"]),
    "open": ([".nn.", "llll", "lrrl", "lttl", ".ll."], [".nn.", "lrrl", ".tt."]),
    "yawn": ([".nn.", ".ll.", "lrrl", "lrrl", "lttl", ".ll."], [".nn.", "lrrl", "lrrl", ".tt."]),
    "big": ([".nn.", "llll", "lrrl", "lrrl", "lttl", ".ll."], [".nn.", "lrrl", "lrrl", ".tt."]),
    "tongue": ([".nn.", ".ll.", ".tt.", ".tt."], ["nn", "tt"]),
    "flat": ([".nn.", "....", ".ll."], ["nn", "..", "ll"]),  # 무표정 일자 입 (페르시안)
}


def front_mouth(sp, kind=None, tongue_dy=0):
    key = "smile" if kind is None and dp._ST["smile"] else kind
    if key is None and sp.mouth == "flat":
        key = "flat"
    rows = FRONT_MOUTHS[key][0 if _wide() else 1]
    ny = {"dog": 57, "flat": 57.5}.get(sp.mouth, 59.5)
    return [([_stamp(64, ny, rows, dict(MOUTH_PAL, n=sp.nose), anchor_top=True)], None)]


SIDE_MOUTHS = {  # 넓은 격자, 좁은 격자
    None: (["..n", ".l."], ["n", "l"]),
    "pout": (["..n", "...", ".ll"], [".n", "ll"]),
    "open": (["..n", ".rl", ".ll"], ["..n", ".rr"]),
    "fang": (["..n", "wrw", ".ll"], ["..n", "wrw"]),
    "hiss": (["..n", "lll", "wrw", ".ll"], ["..n", "wrw"]),
}


def side_mouth(sp, kind=None):
    a, b, _c, d, e, _f = dp._XF[-1]
    if kind is None and math.hypot(b, e) < 0.85:  # 납작하게 엎드린 머리: 입 자리가 없다 → 코만
        return [([_stamp(121.5, 53, ["n"], {"n": sp.nose})], None)]
    rows = SIDE_MOUTHS[kind][0 if _wide() else 1]
    return [([_stamp(119.5, 51, rows, dict(MOUTH_PAL, n=sp.nose), anchor_top=True)], None)]


def brows(kind, cx, cy, flip=False):
    rows = {"sad": ["..#", "##."], "angry": ["##.", "..#"]}[kind]
    if not _wide():
        rows = {"sad": [".#", "#."], "angry": ["#.", ".#"]}[kind]
    st = _stamp(cx, cy, _mirror_rows(rows) if flip else rows, EYE_PAL)
    if dp._INK["line"] == dfarm.GIR_LINE:  # 기린: 머리가 작아 눈썹은 뿔·귀와 엉킨다
        return []
    st.y -= (4.0 if _wide() else 3.0) + (1.0 if dp._ST["sparkle"] else 0.0)  # 눈 위 한 칸 띄워서 (칸으로 재야 감은 눈과 붙어 ◇가 되지 않는다)
    return [([st], None)]


def _fx_stamp(cx, cy, rows, pal, **kw):
    """몸 밖에 찍는 효과 한 장 (compose의 fx 항목)."""
    return ([_stamp(cx, cy, rows, pal, where="out", **kw)], None, 0)


Z_BIG = ["###", "..#", ".#.", "###"]
Z_SMALL = ["##", ".#", "##"]


def z_fx(cx, cy, s):
    return _fx_stamp(cx, cy, Z_BIG if s >= 3.0 else Z_SMALL, {"#": Z_COL}, alts=(Z_SMALL,))


HEARTS = [  # 칸으로 잰 반지름 상한 — 작은 격자에서 하트가 머리만 해지지 않게
    (0.75, ["#.#", ".#."]),
    (1.25, ["#.#", "###", ".#."]),
    (9.00, ["##.##", "#o###", "#####", ".###.", "..#.."]),
]


def heart_fx(cx, cy, r):
    rows = next(rw for lim, rw in HEARTS if r / _PX["grid"].cell <= lim)
    return _fx_stamp(cx, cy, rows, {"#": dp.HEART, "o": SHINE}, alts=(HEARTS[0][1],), optional=True)


def question_fx(cx, cy):
    rows = ["###", "#.#", "..#", ".#.", "...", ".#."]
    return _fx_stamp(cx, cy, rows, {"#": dp.LINE}, halo=SHINE)


def anger_fx(cx, cy, r=5.0):
    small = ["#.#", ".#.", "#.#"]
    rows = [".#.#.", "##.##", ".....", "##.##", ".#.#."] if r / _PX["grid"].cell >= 1.4 else small
    return _fx_stamp(cx, cy, rows, {"#": ANGER_COL}, alts=(small,))


def sparkle_fx(cx, cy, r):
    small = [".e.", "eye", ".e."]
    rows = ["..e..", "..y..", "eyyye", "..y..", "..e.."] if r / _PX["grid"].cell >= 1.1 else small
    return [_fx_stamp(cx, cy, rows, {"y": STAR_COL, "e": STAR_EDGE}, alts=(small,), optional=True)]


def back_arm(sp, side, reach, scratch=None):
    """긁힌 자국 세 줄은 도트에서 붙어 검은 덩어리가 된다 → 두 줄 도장."""
    part, ops = _ORIG["back_arm"](sp, side, reach, None)
    if scratch is not None:
        ox, py = dp._back_paw_at(reach)
        k = dp._ST["head"] * dp.BACK_HEAD
        with dp.xf(dy=-4, sx=k, sy=k, pivot=(64, 84)):
            st = _stamp(64 + ox * side, py - 16 - scratch * 2, ["#.#", "#.#"], {"#": dp.mix(dp.LINE, SHINE, 0.2)}, where="out", optional=True)
        ops = ops + [([st], None)]
    return part, ops


def front_paw(sp, cx, cy, w=15, h=13, toes=False):
    """몸 앞 작은 앞발: 고리·발가락 선은 칸이 모자라 얼룩이 된다 → 등 색 덩어리 하나 (배 색과 구별되게)."""
    col = sp.back if sp.back and sp.back != sp.belly else dp.PINK if sum(sp.base) >= PALE_BASE else dp.mix(sp.base, dp.LINE, 0.25)
    return dp.Part([dp.E(cx - w * 0.62, cy - h * 0.6, cx + w * 0.62, cy + h * 0.6)], col, grow=0)  # 조금 크게: 작으면 칸에서 + 모양이 된다


def back_tail(sp, rot):
    """뒷모습 꼬리: 줄무늬·긴 고리가 엉덩이에서 엉킨다 → 무늬 없이 짧고 통통한 꼬리."""
    if sp.side_tail and sp.side_tail[0][0] == "L":
        root = (64, 112)
        tl, tw = dp._ST["tail"], dp._ST["tail_w"]
        with dp.xf(rot=rot * 1.4, sx=tl, sy=tl, pivot=root):
            return dp.Part([dp.L([root, (68, 103), (70, 92)], 12 * tw / tl)], sp.tail_color, outline=True)
    return _ORIG["back_tail"](sp, rot)


# ---------------------------------------------------------------- 칸에 맞춘 선·점
class _Line:
    """L()과 같게 그리되 (굵을 때), 칸보다 가늘면 중심선을 1칸 선으로 낼 수 있게 점들을 기억한다."""

    def __init__(self, pts, w):
        self.pts = [dp._pt(*p) for p in pts]
        self.wp = w * dp._wscale()
        self.draw = _ORIG_L(pts, w)

    def __call__(self, d):
        self.draw(d)

    def dots(self, grid):
        if self.wp > 1.5 * grid.c:
            return None
        pts = [((x - grid.x0) / grid.c, (y - grid.y0) / grid.c) for x, y in self.pts]
        out = set()
        for (ax, ay), (bx, by) in zip(pts, pts[1:] or pts):
            steps = max(1, int(math.ceil(max(abs(bx - ax), abs(by - ay)) * 2)))
            for k in range(steps + 1):
                t = k / steps
                out.add((math.floor(ay + (by - ay) * t), math.floor(ax + (bx - ax) * t)))
        return out


class _Ellipse:
    """E()와 같게 그리되, 칸보다 작으면 사라지지 않고 1칸 점이 된다."""

    def __init__(self, x0, y0, x1, y1):
        pts = [dp._pt(*p) for p in dp.arc((x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2, (y1 - y0) / 2, 0, 360, 16)]
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        self.box = (min(xs), min(ys), max(xs), max(ys))
        self.draw = _ORIG_E(x0, y0, x1, y1)

    def __call__(self, d):
        self.draw(d)

    def dots(self, grid):
        x0, y0, x1, y1 = self.box
        if max(x1 - x0, y1 - y0) >= 1.6 * grid.c:
            return None
        return {(math.floor(((y0 + y1) / 2 - grid.y0) / grid.c), math.floor(((x0 + x1) / 2 - grid.x0) / grid.c))}


def _fns_dots(fns, grid, thresh=115):
    """도형 목록 → 칸 마스크. 가는 선·작은 점은 1칸으로, 나머지는 덮인 비율로."""
    n = grid.n
    out = np.zeros((n, n), bool)
    big = []
    for f in fns:
        ds = f.dots(grid) if hasattr(f, "dots") else None
        if ds is None:
            big.append(f)
            continue
        for y, x in ds:
            if 0 <= y < n and 0 <= x < n:
                out[y, x] = True
    if big:
        out |= grid.down(dp.shapes(big)) >= thresh
    return out


# ---------------------------------------------------------------- 칸 다루기
def _dilate(m):
    p = np.pad(m, 1)
    return p[1:-1, 1:-1] | p[:-2, 1:-1] | p[2:, 1:-1] | p[1:-1, :-2] | p[1:-1, 2:]


def _nb4(m):
    p = np.pad(m, 1).astype(int)
    return p[:-2, 1:-1] + p[2:, 1:-1] + p[1:-1, :-2] + p[1:-1, 2:]


def _erode(m):
    p = np.pad(m, 1)
    return p[1:-1, 1:-1] & p[:-2, 1:-1] & p[2:, 1:-1] & p[1:-1, :-2] & p[1:-1, 2:]


def _clean(m):
    """외톨이 칸을 지우고 1칸 구멍을 메운다."""
    nb = _nb4(m)
    return (m & (nb >= 1)) | (~m & (nb == 4))


def _ink(base):
    line = dp.mix(dp._INK["line"], base, dp._ST["soft"])
    return tuple(int(c * INK_DARK) for c in line)


def _cells(st, left, top):
    for j, row in enumerate(st.rows):
        for i, ch in enumerate(row):
            if ch != ".":
                yield top + j, left + i, ch


def _origin(st):
    w, h = len(st.rows[0]), len(st.rows)
    left = math.floor(st.x - w / 2 + 0.5)
    top = math.floor(st.y) if st.anchor_top else math.floor(st.y - h / 2 + 0.5)
    return left, top, w, h


def _place_in(st, img, fill, n, face):
    """몸 안 도장: 칸이 몸 밖으로 나가면 안쪽으로 민다 (외곽선을 파먹지 않는다). 먼저 찍은 눈·입(face)은 덮지 않는다."""
    left, top, w, h = _origin(st)
    for _ in range(4):
        hit = [(y, x) for y, x, _c in _cells(st, left, top) if not (0 <= y < n and 0 <= x < n and fill[y, x])]
        if not hit:
            break
        mx = np.mean([x for _y, x in hit]) - (left + w / 2 - 0.5)
        my = np.mean([y for y, _x in hit]) - (top + h / 2 - 0.5)
        left -= int(np.sign(mx)) if abs(mx) > 0.2 else 0
        top -= int(np.sign(my)) if abs(my) > 0.2 else 0
    for y, x, ch in _cells(st, left, top):
        if 0 <= y < n and 0 <= x < n and fill[y, x] and (not face[y, x] or st.rows[0][0] == "b"):
            img[y, x] = st.pal[ch] + (255,)
            face[y, x] = st.rows[0][0] != "b"
    return left, top


def _free_spot(st, occ, n):
    """코드가 준 자리에서 가장 가까운, 몸과 한 칸 떨어진 빈자리 (위쪽 우선). 없으면 None."""
    left, top, w, h = _origin(st)
    near = _dilate(occ)
    best = None
    for rad in range(0, 8):
        for dy in range(-rad, rad + 1):
            for dx in range(-rad, rad + 1):
                if max(abs(dx), abs(dy)) != rad:
                    continue
                lx, ty = left + dx, top + dy
                if lx < 0 or ty < 0 or lx + w > n or ty + h > n:
                    continue
                if any(near[y, x] for y, x, _c in _cells(st, lx, ty)):
                    continue
                score = (abs(dx) + abs(dy) + (0.5 if dy > 0 else 0), dy)
                if best is None or score < best[0]:
                    best = (score, lx, ty)
        if best:
            return best[1], best[2]
    return None


def _place_out(st, img, occ, n):
    """몸 밖 효과: 빈자리를 찾고, 없으면 더 작은 그림으로, 그래도 없으면 optional은 생략 · 나머지는 캔버스 안으로 밀어 겹쳐 찍는다."""
    first = st.rows
    spot = _free_spot(st, occ, n)
    for alt in st.alts:
        if spot:
            break
        st.rows = alt
        spot = _free_spot(st, occ, n)
    if not spot:
        if st.optional:
            return
        st.rows = first
        left, top, w, h = _origin(st)
        spot = (min(max(left, 0), n - w), min(max(top, 0), n - h))
    cells = list(_cells(st, *spot))
    if st.halo:
        mine = {(y, x) for y, x, _c in cells}
        for y, x, _c in cells:
            for ny, nx in ((y - 1, x), (y + 1, x), (y, x - 1), (y, x + 1)):
                if (ny, nx) not in mine and 0 <= ny < n and 0 <= nx < n and not occ[ny, nx]:
                    img[ny, nx] = st.halo + (255,)
                    occ[ny, nx] = True
    for y, x, ch in cells:
        if 0 <= y < n and 0 <= x < n:
            img[y, x] = st.pal[ch] + (255,)
            occ[y, x] = True


# ---------------------------------------------------------------- 도트 compose
def _fit_shift(grid, box):
    """프레임 하나가 격자 밖으로 나가면 (높이 뛴 깡총·앞으로 뻗은 다리) 정수 칸만큼 창을 옮긴다: (옮긴 격자, dx, dy칸). 1칸 외곽선 자리를 남긴다."""
    n, c = grid.n, grid.c
    def need(lo, hi, origin):
        a, b = (lo - origin) / c, (hi - origin) / c  # 칸 단위 경계
        if a < 1:
            return math.ceil(1 - a)
        if b > n - 1:
            return -math.ceil(b - (n - 1))
        return 0
    dx, dy = need(box[0], box[2], grid.x0), need(box[1], box[3], grid.y0)
    return replace(grid, x0=grid.x0 - dx * c, y0=grid.y0 - dy * c), dx, dy


def _shift_stamps(lists, dx, dy):
    seen = set()
    for items in lists:
        for fns, *_ in items:
            for f in fns:
                if isinstance(f, Stamp) and id(f) not in seen:
                    seen.add(id(f))
                    f.x += dx
                    f.y += dy


HELD_HEART = {0: [".ll.ll.", "lrrlrrl", "lrwrrrl", "lrrrrrl", ".lrrrl.", "..lrl.."], 1: [".l.l.", "lrlrl", "lrrrl", ".lrl.", "..l.."], 2: ["r.r", "rrr", ".r."]}


def compose(parts, ops=(), fx=(), clip=None, noshift=False):
    raw = [dp.shapes(p.shapes) for p in parts]
    spk = [dp.shapes(p.spikes) if p.spikes else None for p in parts]
    soft, sharp = dp.blank(), dp.blank()
    for p, m, k in zip(parts, raw, spk):
        if p.sharp:
            sharp = ImageChops.lighter(sharp, m)
        else:
            soft = ImageChops.lighter(soft, m)
        if k is not None:
            sharp = ImageChops.lighter(sharp, k)
    soft = dp.melt(soft)
    boxes = [b for b in [ImageChops.lighter(soft, sharp).getbbox()] if b]
    box = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
    if _PX["collect"] is not None:
        _PX["collect"].append(box)
        return Image.new("RGBA", (1, 1))
    grid = _PX["grid"]
    if not noshift:
        grid, dx, dy = _fit_shift(grid, box)
        if dx or dy:
            _shift_stamps((ops, fx, *[p.marks for p in parts]), dx, dy)
            dp.shift_head(dx * grid.cell, dy * grid.cell)
        _PX["last_dx"] = dx
    n = grid.n
    sil = (grid.down(soft) >= 128) | (grid.down(sharp) >= 90)
    sil = _clean(sil)
    img = np.zeros((n, n, 4), np.uint8)
    base = parts[0].base
    img[sil] = base + (255,)
    outer = _dilate(sil) & ~sil
    img[outer] = _ink(base) + (255,)
    sil_full = ImageChops.lighter(soft, sharp)
    late = []
    for p, m, k in zip(parts, raw, spk):
        if p.base and sum(abs(a - b) for a, b in zip(p.base, dp.HEART)) < 40:  # 품에 안은 하트: 칸이 모자라 세모가 된다 → 하트 도장 (자기 칸 가운데)
            ys, xs = np.nonzero(grid.down(m) >= 128)
            if len(xs):
                st = Stamp(xs.mean() + 0.5, ys.mean() + 0.5, HELD_HEART[_ix()], {"l": dp.mix(dp.HEART, dp.LINE, 0.7), "r": dp.HEART, "w": SHINE})
                late.append(([st], None))
            continue
        if p.outline:
            mm = dp.melt(m, 1.0)
            if k is not None:
                mm = ImageChops.lighter(mm, k)
            region = (grid.down(ImageChops.multiply(mm, sil_full)) >= 128) & sil
            ring = _dilate(region) & ~region & _erode(sil)  # 바깥 외곽선과 붙은 칸은 두 겹이 되니 뺀다
            small = region.sum() < SMALL_PART
            if small and region.any():  # 작은 앞발은 둘레를 다 두르면 동그라미가 된다 → 아래쪽만
                ring &= np.arange(n)[:, None] > np.nonzero(region)[0].mean()
            pb = p.base or base
            pale = sum(pb) >= PALE_BASE  # 흰 조각(수염·날개·털): 진한 고리는 도트에서 지저분하다 → 옅게
            img[ring] = dp.mix(_ink(pb), pb, 0.72 if pale else 0.55 if small else 0.25) + (255,)
        else:
            small = False
            if k is not None:
                m = ImageChops.lighter(m, k)
            grown = dp.grow(m, p.grow) if p.grow else m
            region = (grid.down(ImageChops.multiply(grown, sil_full)) >= 128) & sil
        if p.base:
            img[region] = p.base + (255,)
        for fns, color, *_ in p.marks:
            if small and color == dp._INK["line"]:  # 작은 앞발의 발가락 선은 칸이 모자라 얼룩이 된다
                continue
            stamps = [f for f in fns if isinstance(f, Stamp)]  # 몸 무늬로 들어온 도장(얼굴 등) → 위에 찍는 층으로
            if stamps:
                late.append((stamps, color))
                continue
            img[_fns_dots(fns, grid) & region] = tuple(color) + (255,)
    occ = sil | outer
    face = np.zeros((n, n), bool)
    for fns, color, *rest_ in late + list(ops):
        opts = rest_[0] if rest_ else {}
        for f in fns:
            if isinstance(f, Stamp):
                _place_in(f, img, sil, n, face) if f.where == "in" else _place_out(f, img, occ, n)
        rest = [f for f in fns if not isinstance(f, Stamp)]
        if rest:
            d = _fns_dots(rest, grid)
            if opts.get("clip"):
                d &= sil
            if opts.get("dilate"):
                d = _dilate(d) & (sil | d)
            img[d] = tuple(color) + (255,)
            occ |= d
    for fns, color, _halo in fx:
        for f in fns:
            if isinstance(f, Stamp):
                _place_out(f, img, occ, n)
        rest = [f for f in fns if not isinstance(f, Stamp)]
        if rest:
            d = _fns_dots(rest, grid)
            img[d] = tuple(color) + (255,)
            occ |= d
    return Image.fromarray(img)


# ---------------------------------------------------------------- 산타: 얼굴 칸을 남긴다
def _beard_fns(fx_, hy, hr, k):
    """복슬 수염이 얼굴을 덮으면 눈이 묻힌다 → 코 아래부터 시작하는 둥근 수염 (아래끝은 동글 수염과 같다 — 목도리 자리)."""
    top = hy + 0.36 * hr
    bot = hy + (0.84 * hr if k < 0.65 else 0.2 * hr + 0.86 * hr * k)
    half = hr * (0.56 if k < 0.65 else 0.86)
    return [dp.E(fx_ - half, top, fx_ + half, bot)]


def _mitt_arm(sh, side, a, ln, w, lvl):
    """소매 + 장갑 (소맷부리·반짝이는 칸이 모자라 얼룩이 된다)."""
    p = dh._arm(dc._SUITSP, sh, side, a, ln, w)
    if isinstance(a, (int, float)):
        r = math.radians(a)
        t = (sh[0] + side * math.sin(r) * ln, sh[1] + math.cos(r) * ln)
    else:
        t = a
    return [p, dp.Part([dp.C(t[0], t[1], w * 0.62)], dc.MITT if lvl else dc.SKIN, grow=0)]


# ---------------------------------------------------------------- 오리·기린·공룡
def duck_bill(cx, cy, mouth, side, k=1.0):
    """앞모습 부리가 눈까지 올라오면 눈이 부리에 붙어 안 보인다 → 조금 작고 낮게."""
    return _ORIG["duck_bill"](cx, cy + (0 if side else 4), mouth, side, k * (1.0 if side else 0.84))


GF_STAGE = {lvl: dict(st, lash=False, daisy=False) for lvl, st in dfarm.GF_STAGE.items()}  # 속눈썹·데이지는 칸에서 얼룩
GF_STAGE[0].update(neck=0.8, ossi=1, spots=1)  # 아기: 목·뿔·무늬가 있어야 24칸에서 기린이다
GF_STAGE[1].update(neck=0.85)


WAVE_ROWS = {2: ["#.#.", ".#.#", ".#.#", "#.#."], 3: ["#.#.#.", ".#.#.#", ".#.#.#", "#.#.#."]}


def wave_fx(cx, cy, n, r0=6.0):
    """크앙 소리 물결 ))): 칸에 맞춘 도장 (24칸은 한 줄)."""
    rows = WAVE_ROWS[min(max(n, 2), 3)] if _wide() else ["#.", ".#", ".#", "#."]
    return [([_stamp(cx + r0 + 6, cy, rows, {"#": dd.WAVE}, where="out", optional=True)], None, 0)]


def dust_fx(x, y, s=1.0):
    """발밑 먼지: 작은 구름 도장."""
    rows = [".d.", "ddd"] if s >= 0.8 and _wide() else ["dd"]
    return [([_stamp(x, y - 2, rows, {"d": dp.mix(dd.DUST, dd.DUST_LINE, 0.55)}, where="out", optional=True)], None, 0)]


def impact_fx(cx, y):
    return []


# ---------------------------------------------------------------- 층 (할로윈·산타 _finish): 반투명·번짐 없이
def _finish(parts, glow=(), face=(), fx=(), translucent=None, circle=None):
    """몸 + 얼굴. 번지는 빛(blur 큰 층)은 그리지 않고, 옅은 층은 몸 색과 미리 섞은 불투명 색으로 칠한다. 유령은 불투명한 우유빛."""
    base = parts[0].base
    ops = []
    for items, clip in ((glow, True), (face, False)):
        for fns, color, alpha, g, blur in items:
            if blur >= 2 and (not translucent or color != dh.WARM):
                continue
            if color == dc.CHEEK_RED:  # 산타 볼·코: 칸에 맞춘 도장
                x0, y0, x1, y1 = fns[0].box
                g_ = _PX["grid"]
                st = Stamp(((x0 + x1) / 2 - g_.x0) / g_.c, ((y0 + y1) / 2 - g_.y0) / g_.c, ["bb"], {"b": BLUSH_COL})
                ops.append(([st], None))
                continue
            if color == SANTA_NOSE:
                x0, y0, x1, y1 = fns[0].box
                g_ = _PX["grid"]
                st = Stamp(((x0 + x1) / 2 - g_.x0) / g_.c, ((y0 + y1) / 2 - g_.y0) / g_.c, ["nn"] if _wide() else ["n"], {"n": SANTA_NOSE})
                ops.append(([st], None))
                continue
            if color in (SANTA_NOSE_RIM, ) or (color == SHINE and all(isinstance(f, _Ellipse) and f.dots(_PX["grid"]) for f in fns)):
                continue
            col = dp.mix(base, color, GHOST_GLOW if blur >= 2 else alpha) if color else color
            ops.append((fns, col, dict(clip=clip, dilate=1 if g >= 1 and _wide() else 0)))
    return compose(parts, ops, fx)


SANTA_NOSE, SANTA_NOSE_RIM = (250, 166, 156), (150, 96, 90)
GHOST_GLOW = 0.2  # 유령 배 속 빛: 몸 색 쪽으로 많이 섞은 불투명 색 한 겹 (두 겹이면 계란 프라이가 된다)


CARVE_PAL = {"c": dh.CARVE, "y": dh.GLOW_Y, "w": dh.GLOW_CORE, "e": dh.EMBER}
PK_EYES = {  # 36·32칸, 28칸, 24칸 — 조각한 세모 눈 (짙은 테 + 노란 불빛)
    "dot": (["..c..", ".cyc.", "cywyc", "ccccc"], ["..c..", ".cyc.", "ccccc"], [".c.", "cyc", "ccc"]),
    "up": (["..c..", ".cwc.", "cyyyc", "ccccc"], ["..c..", ".cwc.", "ccccc"], [".c.", "cwc", "ccc"]),
    "big": (["..c..", ".cyc.", "cywyc", "cyyyc", "ccccc"], ["..c..", ".cyc.", "cywyc", "ccccc"], [".c.", "cyc", "ccc"]),
    "round": ([".ccc.", "cyyyc", "cywyc", ".ccc."], [".c.", "cwc", ".c."], [".c.", "cwc", ".c."]),
    "happy": ([".ccc.", "c...c"], [".c.", "c.c"], [".c.", "c.c"]),
    "closed": (["c...c", ".ccc."], ["c.c", ".c."], ["c.c", ".c."]),
    "tight": (["c..", ".cc", "c.."], ["c..", ".cc", "c.."], ["c.", ".c", "c."]),
    "squint": (["cccc"], ["ccc"], ["ccc"]),
    "spiral": (["c.c", ".c.", "c.c"], ["c.c", ".c.", "c.c"], ["c.c", ".c.", "c.c"]),
}
PK_MOUTHS = {  # 넓은 격자, 28칸, 24칸
    None: (["ccccccc", "cyycyyc", ".cyyyc.", "..ccc.."], ["ccccc", "cycyc", ".ccc."], ["ccccc", "cycyc", ".ccc."]),
    "big": (["ccccccc", "cycyycc", "cyyyyyc", ".cyyyc.", "..ccc.."], ["ccccc", "cyyyc", "cyyyc", ".ccc."], ["ccccc", "cyyyc", ".ccc."]),
    "open": ([".ccc.", "cyyyc", ".ccc."], [".c.", "cyc", ".c."], [".c.", "cyc", ".c."]),
    "yawn": ([".ccc.", "cyyyc", "cyyyc", ".ccc."], [".c.", "cyc", "cyc", ".c."], [".c.", "cyc", ".c."]),
    "sip": (["cc", "cc"], ["cc"], ["cc"]),
    "frown": ([".ccc.", "c...c"], [".c.", "c.c"], [".c.", "c.c"]),
    "pout": ([".cc.", "c..c"], ["cc"], ["cc"]),
    "chew": (["c.c.c", ".c.c."], ["c.c", ".c."], ["c.c", ".c."]),
    "wavy": (["c.c.c", ".c.c."], ["c.c", ".c."], ["c.c", ".c."]),
}


def _ix():
    n = _PX["grid"].n
    return 0 if n >= 32 else 1 if n >= 28 else 2


def pk_face(fx_, fy, eyes, mouth, brow, lit, side):
    """호박 얼굴을 도장으로: 조각한 세모 눈 + 들쭉날쭉 입 (번지는 불빛 없이 노란 칸이 빛난다)."""
    fs = dh._fs()
    pal = CARVE_PAL if lit >= 0.5 else dict(CARVE_PAL, y=dh.EMBER, w=dh.EMBER)
    face, glow = [], []
    with dp.xf(sx=fs, sy=fs, pivot=(fx_, fy)):
        sp_x = 12.5 if not side else 11.0
        for ex, flip in ((fx_ - sp_x, False), (fx_ + sp_x, True)):
            ey = fy - 5 + dp._ST["eye_dy"] * 0.5
            ek = "closed" if lit < 0.5 and eyes == "dot" else eyes
            rows = PK_EYES[ek][_ix()]
            face.append(([_stamp(ex, ey, _mirror_rows(rows) if flip else rows, pal)], None, 1.0, 0, 0))
            if brow:
                face += dh._ops(brows(brow, ex, ey - 1 + dp._ST["eye_dy"] * 0.5, flip=flip))
        for cx_ in (fx_ - 21, fx_ + 21):
            face.append(([_stamp(cx_ + (3 if side and cx_ < fx_ else 0), fy + 4, ["bb"] if _ix() else ["bbb"], {"b": (255, 128, 104)})], None, 1.0, 0, 0))
        rows = PK_MOUTHS[mouth][_ix()]
        face.append(([_stamp(fx_ + (1 if side else 0), fy + 9, rows, pal, anchor_top=True)], None, 1.0, 0, 0))
    return glow, face


def _heart_items(hx, hy, k=1.0):
    """품에 안은 하트."""
    rows = {0: [".ll.ll.", "lrrlrrl", "lrwrrrl", "lrrrrrl", ".lrrrl.", "..lrl..", "...l..."],
            1: [".l.l.", "lrlrl", "lrrrl", ".lrl.", "..l.."], 2: ["r.r", "rrr", ".r."]}[_ix()]
    return [([_stamp(hx, hy + 6 * k, rows, {"l": dp.mix(dp.HEART, dp.LINE, 0.7), "r": dp.HEART, "w": SHINE})], None, 1.0, 0, 0)]


# ---------------------------------------------------------------- 생각 풍선 (음식 그림은 종마다 도장)
BUBBLE_LINE, BUBBLE_FILL = dp.LINE, (224, 236, 250)
ICON_PALS = dict(k=(52, 62, 58), w=(244, 246, 250), g=(118, 186, 96), G=(84, 148, 72), r=(236, 84, 84), R=(250, 140, 150), p=(246, 150, 186),
                 y=(255, 206, 84), v=(170, 130, 224), o=(255, 166, 72), b=(206, 158, 96), t=(248, 220, 170), s=(140, 200, 240), P=(156, 104, 204),
                 n=(150, 104, 62), B=(120, 170, 230), m=(150, 156, 170))
ICONS = {  # 넓은 격자, 좁은 격자
    "rice": (["..w..", ".www.", "wwwww", "kkkkk"], [".m.", "mwm", "kkk"]),
    "fly": (["ss.ss", ".kkk.", "kkkkk", ".k.k."], ["s.s", ".k.", "k.k"]),
    "candy": (["v.ooo.v", "vvooovv", "v.ooo.v"], ["vov"]),
    "lolly": ([".ppp.", "ppwpp", ".ppp.", "..n..", "..n.."], ["ppp", "ppp", ".n."]),
    "apple": (["..g..", ".rrr.", "rwrrr", "rrrrr", ".r.r."], [".g.", "rrr", "rrr"]),
    "milk": (["sss..", "www.t", "www.b", "www.."], ["ss", "mm", "mm"]),
    "bread": ([".bbb.", "bttbb", "btttb", "bbbbb"], ["bbb", "btb", "bbb"]),
    "leaf": (["..ggg", ".gGgg", "gGgg.", "G...."], [".gg", "gGg", "G.."]),
    "melon": (["rrkrr", ".rrr.", "ggggg"], ["rkr", "ggg"]),
    "broc": ([".ggg.", "gGgGg", ".ggg.", "..n.."], ["ggg", ".n."]),
    "grape": (["..g..", "PP.PP", ".PPP.", "..P.."], ["P.P", ".P."]),
    "ice": ([".www.", "wwwww", "BBBBB", ".BBB."], ["sss", "BBB"]),
    "shrimp": ([".oo..", "o..o.", "o.oo.", ".oo.."], ["oo.", "o.o", ".o."]),
    "seeds": (["..t..", ".tbt.", "t.t.t", ".tbt."], [".t.", "tbt"]),
    "acorn": (["..n..", "nnnnn", ".ooo.", ".ooo.", "..o.."], [".n.", "nnn", ".o."]),
    "bamboo": (["g..yG", ".gyG.", "..yg.", ".yG.g"], [".yg", "gy.", "y.."]),
}


def _bubble_rows(icon):
    w = len(icon[0])
    rows = ["." + "l" * (w + 2) + "."] + ["l" + "f" + "".join(c if c != "." else "f" for c in r) + "f" + "l" for r in icon]
    rows += ["." + "l" * (w + 2) + ".", "l" + "." * (w + 3)]
    return rows


def _food_bubble(name, cx, cy):
    wide, narrow = ICONS[name]
    pal = dict(ICON_PALS, l=BUBBLE_LINE, f=BUBBLE_FILL)
    if _PX["grid"].n >= 32:
        rows, alts = _bubble_rows(wide), (_bubble_rows(narrow), narrow)
    elif _PX["grid"].n >= 28:
        rows, alts = _bubble_rows(narrow), (narrow,)
    else:  # 24칸: 머리 옆에 풍선 자리가 없다 — 작은 그림만 (얼굴을 가리지 않게, 자리 없으면 생략)
        rows, alts = narrow, ()
    return [([_stamp(cx, cy + 3, rows, pal, where="out", alts=alts, optional=True)], None, 0)]


def thought_fx(cx, cy, r=12.5):
    return _food_bubble("rice", cx, cy)


def _bubble_fn(name):
    return lambda cx, cy, r=12.5: _food_bubble(name, cx, cy)


# ---------------------------------------------------------------- 루돌프 코: 후광 없이 딱딱한 빨간 코 + 반짝 칸
NOSE_ROWS = {0: [".dddd.", "drwrrd", "drrrrd", ".dddd."], 1: [".dd.", "dwrd", "drrd", ".dd."], 2: ["rr", "rr"]}
NOSE_SIDE = {0: [".ddd", "drwr", "drrr", ".ddd"], 1: [".dd", "dwr", ".dd"], 2: ["rr", "rr"]}
NOSE_TWINKLE = {"s": (255, 244, 170), "e": (250, 200, 92)}


def _nose_glow(im, i, lit=1.0):
    a = np.array(im)
    if a.shape[0] != _PX["grid"].n:
        return im
    core = (np.abs(a[..., :3].astype(int) - np.array(dc.NOSE_KEY)).sum(axis=2) < 30) & (a[..., 3] > 0)
    if not core.any():
        return im
    ys, xs = np.nonzero(core)
    n = a.shape[0]
    side = xs.mean() > n * 0.62
    rows = (NOSE_SIDE if side else NOSE_ROWS)[_ix()]
    pal = {"d": dc.NOSE_DEEP, "r": dc.NOSE_RED, "w": dc.NOSE_SHINE}
    h, w = len(rows), len(rows[0])
    top = int(round(ys.mean() - h / 2 + 0.5))
    if side:  # 옆모습: 코끝을 주둥이 끝에 맞춘다
        left = min(int(xs.max()) + 1, n - 1) - w + 1
    else:
        left = int(round(xs.mean() - w / 2 + 0.5))
    for j, row in enumerate(rows):
        for k, ch in enumerate(row):
            y, x = top + j, left + k
            if ch != "." and 0 <= y < n and 0 <= x < n:
                a[y, x] = pal[ch] + (255,)
    if i % 2 == 0 and lit >= 0.9 and _ix() < 2:  # 반짝 ✦ 한 칸 (코 위 빈자리에만)
        y, x = max(top - 2, 0), min(left + w + 1, n - 1)
        if a[y, x, 3] == 0:
            a[y, x] = NOSE_TWINKLE["s"] + (255,)
    return Image.fromarray(a)


# ---------------------------------------------------------------- 문맥·격자·렌더
_PATCH = dict(compose=compose, stage_eye=stage_eye, blush=blush, open_mouth=open_mouth, front_mouth=front_mouth, side_mouth=side_mouth,
              brows=brows, z_fx=z_fx, heart_fx=heart_fx, question_fx=question_fx, anger_fx=anger_fx, sparkle_fx=sparkle_fx,
              thought_fx=thought_fx, back_arm=back_arm, E=_Ellipse, L=_Line, WHISKERS=[[(21, 61), (11, 59)], [(21, 67), (11, 69)]],
              front_paw=front_paw, back_tail=back_tail, _finish=_finish, pk_face=pk_face, _heart_items=_heart_items,
              _nose_glow=_nose_glow, candy_bubble_fx=_bubble_fn("candy"), lolly_bubble_fx=_bubble_fn("lolly"),
              _apple_bubble=_bubble_fn("apple"), milk_bubble_fx=_bubble_fn("milk"), fly_bubble_fx=_bubble_fn("fly"), bread_bubble_fx=_bubble_fn("bread"),
              leaf_bubble_fx=_bubble_fn("leaf"), melon_bubble_fx=_bubble_fn("melon"), broc_bubble_fx=_bubble_fn("broc"), grape_bubble_fx=_bubble_fn("grape"),
              ice_bubble_fx=_bubble_fn("ice"), _beard_fns=_beard_fns, _mitt_arm=_mitt_arm, duck_bill=duck_bill, GF_STAGE=GF_STAGE,
              wave_fx=wave_fx, dust_fx=dust_fx, impact_fx=impact_fx, seeds_bubble_fx=_bubble_fn("seeds"), shrimp_bubble_fx=_bubble_fn("shrimp"), bamboo_bubble_fx=_bubble_fn("bamboo"),
              FOOD_BUBBLE={"dino_yellow": _bubble_fn("melon"), "dino_green": _bubble_fn("broc"), "dino_purple": _bubble_fn("grape")})


def _modules():
    return [dp, dc, dd, dfarm, df, dh, ds, dbird, dpeng, dred, dsq]


@contextmanager
def _unpatched():
    """픽셀 문맥 안에서 잠깐 동글 그림 코드로 (기린 목 길이 재기는 128칸 그림에서 잰다)."""
    saved = []
    for mod in _modules():
        for name, new in _PATCH.items():
            if getattr(mod, name, None) is new:
                saved.append((mod, name, new))
                setattr(mod, name, _ORIG[name])
    try:
        yield
    finally:
        for mod, name, new in saved:
            setattr(mod, name, new)


def neck_geometry(sp):
    """동글 그림으로 잰 목 자리 → 도트 격자에 맞춘다: 자르는 줄 · 토막 높이 · 늘이는 길이가 모두 정수 칸, 무늬 주기 = 토막 높이."""
    with _unpatched():
        g = dict(_ORIG["neck_geometry"](sp))
    grid = _PX["grid"]
    s_, lift = dfarm._view_k()
    cut = int(round((g["cut"] - grid.oy) / grid.cell))
    tile = max(3, round(dfarm.NECK_TILE / grid.cell))
    g.update(cut_dot=cut, tile_dot=tile, ext_dot=max(tile, round(g["ext"] / grid.cell / tile) * tile),
             period=tile * grid.cell / s_, cut_pre=126 + (cut * grid.cell + grid.oy + lift - 126) / s_)
    return g


def neck_layers_pixel(sp, stage):
    """기린 목 늘이기 층 (도트): (층 4칸 가로 4n×n, 목 토막 n×tile, 메타 {cut, tile, ext} — 칸 단위)."""
    grid = stage_grid(sp, stage)
    n = grid.n
    with _stage_ctx(sp, stage), pixel_mode(grid):
        ssp = pixel_species(dp.staged_species(sp))
        with dp.view(scale=ssp.scale * dp._ST["fit"], lift=dp.LIFT + 1.7 * dp.OUTLINE * (dp._ST["line"] - 1)):
            old_ink = dp._INK["line"]
            dp._INK["line"] = ssp.outline or dp.LINE
            try:
                g = neck_geometry(ssp)
                cut = g["cut_dot"]
                strip = Image.new("RGBA", (4 * n, n), (0, 0, 0, 0))
                strip.paste(dfarm._neck_pose(ssp, 0, g["period"], g["cut_pre"]).crop((0, cut, n, n)), (0, cut))
                for j in range(3):
                    im = dfarm._neck_pose(ssp, j, g["period"], g["cut_pre"])
                    strip.paste(im.crop((0, 0, n, cut)), ((j + 1) * n, 0))
                a = g["a"]
                w = dfarm.NECK_W * dp._ST["body"] ** 0.5
                col = dp.Part([dp.L([(a[0], a[1] + 200), (a[0], a[1] - 400)], w)], dfarm.GIR,
                              dfarm.neck_marks((a[0], a[1] + 200), (a[0], a[1] - 400), w, g["period"], g["cut_pre"]), grow=1)
                dx = _PX["last_dx"]
                tall = dp.compose([col], noshift=True)
                tile = Image.new("RGBA", (n, g["tile_dot"]), (0, 0, 0, 0))
                tile.paste(tall.crop((0, cut - g["tile_dot"], n, cut)), (dx, 0))
            finally:
                dp._INK["line"] = old_ink
    return strip, tile, {"cut": cut, "tile": g["tile_dot"], "ext": g["ext_dot"]}


@contextmanager
def pixel_mode(grid, collect=None):
    """이 문맥 안에서만 그림 코드가 도트를 찍는다 (from draw_pets import …로 받은 다른 모듈의 이름까지). 나오면 원래대로."""
    saved = []
    for mod in _modules():
        for name, new in _PATCH.items():
            cur = getattr(mod, name, None)
            if cur is not None and cur is _ORIG[name]:
                saved.append((mod, name, cur))
                setattr(mod, name, new)
    old = dict(_PX)
    _PX.update(grid=grid, collect=collect)
    try:
        yield
    finally:
        _PX.update(old)
        for mod, name, cur in saved:
            setattr(mod, name, cur)


_PATCH["neck_geometry"] = neck_geometry
_PATCH["acorn_bubble_fx"] = _bubble_fn("acorn")  # 다람쥐 🌰


def _anims(sp):
    return [(a, n) for a, n, *_ in dp.rows_for(sp) if a not in dropped_anims(sp)]


def _unfit(fn):
    """_fit 감싸개를 벗긴다: 128칸에 맞춰 프레임을 미는 코드 — 도트는 격자가 이미 몸 전체에 맞춰져 있다."""
    while "_fit.<locals>" in getattr(fn, "__qualname__", ""):
        fn = fn.__closure__[fn.__code__.co_freevars.index("fn")].cell_contents
    return fn


def pixel_species(sp):
    """그림 코드에 넘길 종 설정: 포즈의 _fit을 벗겼다."""
    return replace(sp, poses={k: _unfit(v) for k, v in sp.poses.items()}) if sp.poses else sp


# 24칸에서 작은 귀·꼬리는 칸이 모자라 사라진다 → 어릴수록 귀를 동글 그림보다 크게 · 어른도 눈은 활짝 (반쯤 감은 눈은 도트에서 뚱해 보인다)
PIXEL_TWEAK = {"baby": dict(ear=1.0, tail=0.62, pale=0.2), "kinder": dict(ear=1.05, tail=0.8, pale=0.12), "child": dict(ear=1.05), "teenager": dict(ear=1.05),
               "adult": dict(calm=False, ear=1.05)}


SPECIES_TWEAK = {}  # {종: {단계: 덮어쓰기}} — 도트에서만 다르게 할 종별 단계 특징


def _stage_ctx(sp, stage):
    tw = dict(PIXEL_TWEAK[stage])
    for key, val in (sp.stage_tweak.get(stage) or {}).items():
        tw[key] = val if key != "ear" else max(val, tw.get("ear", 0))
    tw.update(SPECIES_TWEAK.get(sp.id, {}).get(stage, {}))
    tw.update(calm=False, chest_fluff=False)
    return dp.stage(stage, tw)


_GRIDS = {}
_GRID_CACHE = os.environ.get("PIXEL_GRID_CACHE")  # 개발용: 격자를 파일에 기억해 반복을 빠르게 (그림 윤곽을 바꾸면 지운다)


def _load_cache():
    if _GRID_CACHE and Path(_GRID_CACHE).exists():
        return json.loads(Path(_GRID_CACHE).read_text())
    return {}


def stage_grid(sp, stage):
    """모든 프레임의 실루엣 경계에 맞춘 격자 (단계마다 고정 — 프레임마다 바꾸면 크기가 튄다)."""
    key = (sp.id, stage)
    if key in _GRIDS:
        return _GRIDS[key]
    cache = _load_cache()
    ck = f"{sp.id}/{stage}"
    if ck in cache:
        _GRIDS[key] = Grid(**cache[ck])
        return _GRIDS[key]
    n = stage_dots(sp.id, stage)
    boxes = []
    dummy = Grid(n, 12, 0, 0)
    with _stage_ctx(sp, stage), pixel_mode(dummy, collect=boxes):
        ssp = pixel_species(dp.staged_species(sp))
        for anim, cnt in _anims(sp):
            dp.frames(ssp, anim, cnt)
    # 칸 크기는 프레임 하나하나의 크기로 정한다 (높이 뛴 프레임은 _fit_shift가 창을 옮긴다) — 깡총 때문에 몸이 작아지지 않게
    w = max(b[2] - b[0] for b in boxes)
    h = max(b[3] - b[1] for b in boxes)
    c = max(math.ceil(max(w, h) / (n - 2)), 8)
    y1 = max(b[3] for b in boxes)
    _GRIDS[key] = Grid(n, c, dp.S // 2 - c * n // 2, y1 - c * (n - 1))
    if _GRID_CACHE:
        cache[ck] = dict(n=_GRIDS[key].n, c=c, x0=_GRIDS[key].x0, y0=_GRIDS[key].y0)
        Path(_GRID_CACHE).write_text(json.dumps(cache))
    return _GRIDS[key]


def render_pixel(sp, stage, anims=None):
    """{동작: [n×n RGBA 프레임]}. 머리·목 표시는 dp._HEAD_LOG·_NECK_LOG에 (128칸 좌표) 남는다."""
    grid = stage_grid(sp, stage)
    out = {}
    with _stage_ctx(sp, stage), pixel_mode(grid):
        ssp = pixel_species(dp.staged_species(sp))
        for anim, cnt in _anims(sp):
            if anims is None or anim in anims:
                out[anim] = dp.frames(ssp, anim, cnt)
    return out


# ---------------------------------------------------------------- 시트·pet.json (ui/pets_pixel/<id>/)
def _to_dots(entry, grid):
    """[x, y, w, r] (128 프레임 좌표) → 그 단계의 칸 좌표."""
    x, y, w, r = entry
    return [round(grid.to_dot_x(x), 2), round(grid.to_dot_y(y), 2), round(w / grid.cell, 2), r]


def render_with_marks(sp, stage):
    """(프레임, head 표, neck 표 — 칸 좌표)."""
    dp._HEAD_LOG.clear()
    dp._NECK_LOG.clear()
    rendered = render_pixel(sp, stage)
    grid = stage_grid(sp, stage)
    head = {a: [_to_dots(e, grid) for e in v] for a, v in dp._HEAD_LOG.items()}
    neck = {a: [_to_dots(e, grid) for e in v] for a, v in dp._NECK_LOG.items()}
    return rendered, head, neck


def pixel_rows(sp):
    """(동작, 프레임 수, fps, 반복, 앞모습) — roll만 빠지고 (PIXEL_ROLL은 남는다) 순서는 동글 시트와 같다. 줄 번호는 이어 붙인다."""
    return [r for r in dp.rows_for(sp) if r[0] not in dropped_anims(sp)]


def write_sheet(sp, rendered, path):
    dp.sheet_image(pixel_rows(sp), rendered).save(path, optimize=True)


def manifest_for(sp, heads, necks, neck_stretch=None):
    """픽셀 pet.json: 그림 정보만 (속도·성격·크기·집·대사는 앱이 동글 pet.json에서 읽는다). head·neck·neck_stretch는 그 단계의 칸 좌표."""
    animations = {}
    for row, (anim, cnt, fps, loop, front) in enumerate(pixel_rows(sp)):
        spec = {"row": row, "frames": cnt, "fps": fps}
        if not loop:
            spec["loop"] = False
        if front:
            spec["front"] = True
        animations[anim] = spec
    m = {"name": sp.name, "frame_sizes": {st: stage_dots(sp.id, st) for st in dp.STAGES}, "animations": animations, "head": heads}
    if any(necks.values()):
        m["neck"] = necks
    if neck_stretch:
        m["neck_stretch"] = neck_stretch
    return m


def write_species(sp, out_root=OUT_ROOT):
    """ui/pets_pixel/<id>/{sheet_<stage>.png ×5, pet.json} (기린은 neck_<stage>.png, neck_tile_<stage>.png도). {stage: {anim: [frames]}} 반환."""
    out = Path(out_root) / sp.id
    out.mkdir(parents=True, exist_ok=True)
    by_stage, heads, necks, stretch = {}, {}, {}, {}
    for st in dp.STAGES:
        by_stage[st], heads[st], necks[st] = render_with_marks(sp, st)
        write_sheet(sp, by_stage[st], out / f"sheet_{st}.png")
        if "neckstretch" in by_stage[st]:
            strip, tile, stretch[st] = neck_layers_pixel(sp, st)
            strip.save(out / f"neck_{st}.png", optimize=True)
            tile.save(out / f"neck_tile_{st}.png", optimize=True)
    m = manifest_for(sp, heads, necks, stretch)
    (out / "pet.json").write_text(dp._manifest_text(m), encoding="utf-8")
    return by_stage


# ---------------------------------------------------------------- 미리보기 (tools/preview/pixel_<묶음>.png)
LIGHT, DARK = (240, 240, 244), (44, 46, 58)
GROUPS = {"animals": ["cat", "rabbit", "persian"], "frog": ["frog"], "halloween": ["pumpkin", "ghost"],
          "farm": ["duck", "giraffe"], "christmas": ["rudolph", "santa", "snowman"], "dinos": ["dino_yellow", "dino_green", "dino_purple"],
          "birds": ["bird"], "penguin": ["penguin"], "redpanda": ["redpanda"]}
GROUPS["squirrel"] = ["squirrel"]
KEY_FRAMES = [("walk", 0), ("run", 1), ("sit_front", 0), ("sleep", 0), ("hungry", 1), ("heart", 2), ("tilt", 1), ("fall", 0), ("wave", 1),
              ("eat", 0), ("level_up", 2), ("lookup", 2)]
EXTRA_KEYS = [("climb", 0), ("climb", 2), ("slide", 1), ("bigjump", 1), ("neckstretch", 2), ("arch", 2), ("huff", 1),
              ("fire", 2), ("roar", 1), ("stomp", 1)]
KEY_COUNT = 16
PREVIEW_LIMIT = 8000


def _keys_for(rendered):
    have = lambda k: k[0] in rendered and k[1] < len(rendered[k[0]])
    keys = [k for k in KEY_FRAMES if have(k)]
    return (keys + [k for k in EXTRA_KEYS if have(k)])[:KEY_COUNT]


def _blit(canvas, im, x, y, scale):
    canvas.alpha_composite(im.resize((im.width * scale, im.height * scale), Image.NEAREST), (x, y))


def _panel_species(sp, by_stage):
    """한 종: 실제 표시 크기 (밝은·어두운 바탕 나란히) + 확대 (단계마다 한 줄)."""
    font = ImageFont.load_default()
    real, big, label = 150, 216, 84
    keys = {st: _keys_for(by_stage[st]) for st in dp.STAGES}
    heights = [stage_dots(sp.id, st) * STAGE_GRID[st][1] + 6 for st in dp.STAGES]
    W = label + 2 * real * KEY_COUNT
    H = 18 + sum(heights) + 16 + len(dp.STAGES) * (big + 4)
    img = Image.new("RGBA", (W, H), (255, 255, 255, 255))
    d = ImageDraw.Draw(img)
    d.text((4, 3), f"{sp.id}  REAL SIZE (light, dark), then enlarged", fill=(30, 30, 30), font=font)
    d.rectangle((label, 18, label + real * KEY_COUNT - 1, 18 + sum(heights)), fill=LIGHT + (255,))
    d.rectangle((label + real * KEY_COUNT, 18, W, 18 + sum(heights)), fill=DARK + (255,))
    y = 18
    for st, h in zip(dp.STAGES, heights):
        n, sc = stage_dots(sp.id, st), STAGE_GRID[st][1]
        d.text((4, y + 4), f"{st}\n{n}x{sc}={n * sc}px", fill=(50, 50, 50), font=font)
        for side in (0, 1):
            for c, (anim, i) in enumerate(keys[st]):
                x = label + side * real * KEY_COUNT + c * real + (real - n * sc) // 2
                _blit(img, by_stage[st][anim][i], x, y + 3, sc)
        y += h
    y += 16
    for st in dp.STAGES:
        n = stage_dots(sp.id, st)
        z = (big - 8) // n
        d.rectangle((label, y, label + big * len(keys[st]) - 1, y + big), fill=LIGHT + (255,))
        d.text((4, y + 4), f"{st}\nx{z}", fill=(50, 50, 50), font=font)
        for c, (anim, i) in enumerate(keys[st]):
            d.text((label + c * big + 2, y + 1), f"{anim} {i}", fill=(120, 120, 130), font=font)
            _blit(img, by_stage[st][anim][i], label + c * big + (big - n * z) // 2, y + big - n * z - 2, z)
        y += big + 4
    return img


def preview_group(name, by_species, out_dir):
    panels = [_panel_species(dp.SPECIES[pid], by_species[pid]) for pid in GROUPS[name]]
    W, H = max(p.width for p in panels), sum(p.height + 12 for p in panels)
    assert max(W, H) < PREVIEW_LIMIT, (name, W, H)
    img = Image.new("RGB", (W, H), (255, 255, 255))
    y = 0
    for p in panels:
        img.paste(p, (0, y), p)
        y += p.height + 12
    path = Path(out_dir) / f"pixel_{name}.png"
    path.parent.mkdir(parents=True, exist_ok=True)
    img.save(path, optimize=True)
    return path


def main(argv):
    """python tools/draw_pixel.py [묶음 …] [--no-preview] : 시트·pet.json + 미리보기."""
    preview = "--no-preview" not in argv
    argv = [a for a in argv if a != "--no-preview"]
    names = argv or list(GROUPS)
    pv = Path(__file__).resolve().parent / "preview"
    for name in names:
        by_species = {}
        for pid in GROUPS[name]:
            t0 = time.perf_counter()
            by_species[pid] = write_species(dp.SPECIES[pid])
            print(f"{pid}: {time.perf_counter() - t0:.1f}s", flush=True)
        if preview:
            print(preview_group(name, by_species, pv), flush=True)
        by_species.clear()


if __name__ == "__main__":
    main(sys.argv[1:])
