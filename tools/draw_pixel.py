"""픽셀(네모네모) 스파이크 — 완성된 PNG를 변환하지 않고, 그림 코드에서 바로 도트를 찍는다 (지금은 고양이만).

draw_pets.py는 한 줄도 고치지 않는다. pixel_mode() 안에서만 compose·stage_eye·blush·z_fx·heart_fx… 를 도트 버전으로
바꿔 끼우고, 나오면 되돌린다 (이 문맥이 곧 플래그). 포즈·뼈대·종별 설정은 동글 그림과 같은 코드를 그대로 쓰므로:
  - 눈·입·볼·효과는 "찾아내는" 게 아니라 코드가 넘겨준 자리(현재 변환 적용)에 도장(Stamp)으로 찍는다.
  - 색은 칸 평균이 아니라 부품 순서대로 설정 색을 그대로 칠하므로 탁해지지 않는다.
  - 외곽선은 채운 칸 바깥 1칸. 가는 줄무늬·발가락은 칸에 맞춘 1칸 선, 작은 점(젤리)은 1칸 점으로 남긴다.

    python tools/draw_pixel.py   → tools/preview/pixel_cat.png  (실제 표시 크기 + 확대, 밝은·어두운 바탕, pixelize c와 비교)

roll은 픽셀 모드에 없다. 앱·시트는 건드리지 않는다.
"""

import math
import sys
import time
from contextlib import contextmanager
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image, ImageChops, ImageDraw, ImageFont

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402

K = dp.K
NO_ANIM = {"roll"}
# 단계 → (캔버스 칸 수, 화면 배율). 표시 크기 = 칸 × 배율
STAGE_GRID = {"baby": (24, 3), "kinder": (28, 3), "child": (32, 3), "teenager": (32, 4), "adult": (36, 4)}

EYE_COL = (58, 38, 42)
SHINE = (255, 255, 255)
IRIS = (138, 96, 92)
BLUSH_COL = (250, 158, 164)
Z_COL = (112, 144, 214)
ANGER_COL = (228, 76, 76)
STAR_COL, STAR_EDGE = (255, 222, 92), (236, 158, 64)
SMALL_PART = 18  # 칸 수가 이보다 적은 겹친 조각(앞발): 테두리를 옅게, 발가락 선 없이
INK_DARK = 0.70  # 외곽선: 설정 외곽선 색을 이만큼 어둡게 (도트는 1칸뿐이라 더 진해야 또렷하다)


@dataclass
class Grid:
    """128 프레임 → n×n 칸. c: 칸 한 변(슈퍼샘플 픽셀, 정수 → 칸 경계가 정확하고 좌우 대칭)."""
    n: int
    c: int
    x0: int  # 칸 (0,0)의 슈퍼샘플 좌표
    y0: int
    half_px: float = 0.0
    top_px: float = 0.0

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


_PX = {"grid": None, "collect": None}  # pixel_mode 상태
_ORIG = {name: getattr(dp, name) for name in ("compose", "stage_eye", "blush", "open_mouth", "front_mouth", "side_mouth", "brows",
                                              "z_fx", "heart_fx", "question_fx", "anger_fx", "sparkle_fx", "thought_fx", "E", "L", "WHISKERS", "back_arm")}
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


def _mirror_rows(rows):
    return [r[::-1] for r in rows]


def _wide():
    return _PX["grid"].n >= 32


def _eye_rows(kind, flip=False):
    n = _PX["grid"].n
    wide = _wide()
    if kind in ("dot", "up"):
        if _ST_get("sparkle"):
            return ["o##", "###", "##i"] if n < 28 else ["o##", "###", "###", "#i#"]
        if _ST_get("calm") and kind == "dot":
            return ["####", ".o#.", ".##."]
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
    if kind == "round":
        return ["###", "#o#", "###"]
    if kind == "spiral":
        return ["###", "#.#", "##."]
    if kind == "big":
        return ["o##", "###", "###", "##i"]
    raise ValueError(kind)


def _ST_get(key):
    return dp._ST.get(key)


def stage_eye(kind, cx, cy, flip=False):
    cy += dp._ST["eye_dy"]
    if kind == "up":
        cy -= 2.0
    return [([_stamp(cx, cy, _eye_rows(kind, flip), EYE_PAL)], None)]


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
}


def front_mouth(sp, kind=None, tongue_dy=0):
    key = "smile" if kind is None and dp._ST["smile"] else kind
    rows = FRONT_MOUTHS[key][0 if _wide() else 1]
    ny = 57 if sp.mouth == "dog" else 59.5
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
    st.y -= 3.0 + (1.0 if dp._ST["sparkle"] else 0.0)  # 눈 위 한 칸 띄워서 (칸으로 재야 감은 눈과 붙어 ◇가 되지 않는다)
    return [([st], None)]


Z_BIG = ["###", "..#", ".#.", "###"]
Z_SMALL = ["##", ".#", "##"]


def z_fx(cx, cy, s):
    return ([_stamp(cx, cy, Z_BIG if s >= 3.0 else Z_SMALL, {"#": Z_COL}, where="out", alts=(Z_SMALL,))], None, 0)


HEARTS = [  # 칸으로 잰 반지름 상한 — 작은 격자에서 하트가 머리만 해지지 않게
    (0.75, ["#.#", ".#."]),
    (1.25, ["#.#", "###", ".#."]),
    (9.00, ["##.##", "#o###", "#####", ".###.", "..#.."]),
]


def heart_fx(cx, cy, r):
    rows = next(rw for lim, rw in HEARTS if r / _PX["grid"].cell <= lim)
    return ([_stamp(cx, cy, rows, {"#": dp.HEART, "o": SHINE}, where="out", alts=(HEARTS[0][1],), optional=True)], None, 0)


def question_fx(cx, cy):
    rows = ["###", "#.#", "..#", ".#.", "...", ".#."]
    return ([_stamp(cx, cy, rows, {"#": dp.LINE}, where="out", halo=SHINE)], None, 0)


def anger_fx(cx, cy, r=5.0):
    small = ["#.#", ".#.", "#.#"]
    rows = [".#.#.", "##.##", ".....", "##.##", ".#.#."] if r / _PX["grid"].cell >= 1.4 else small
    return ([_stamp(cx, cy, rows, {"#": ANGER_COL}, where="out", alts=(small,))], None, 0)


def sparkle_fx(cx, cy, r):
    small = [".e.", "eye", ".e."]
    rows = ["..e..", "..y..", "eyyye", "..y..", "..e.."] if r / _PX["grid"].cell >= 1.1 else small
    return [([_stamp(cx, cy, rows, {"y": STAR_COL, "e": STAR_EDGE}, where="out", alts=(small,), optional=True)], None, 0)]


BUBBLE_WIDE = ["..lllllll..",
               ".lfffffffl.",
               "lffffwffffl",
               "lfffwwwfffl",
               "lffwwwwwffl",
               "lfwwkkkwwfl",
               ".lfffffffl.",
               "..lllllll..",
               ".l.........",
               "l.........."]
BUBBLE_NARROW = [".lllll.",
                 "lffwffl",
                 "lfwwwfl",
                 "lwkkkwl",
                 ".lllll.",
                 "l......"]


def thought_fx(cx, cy, r=12.5):
    rows = BUBBLE_WIDE if _wide() else BUBBLE_NARROW
    pal = {"l": dp.LINE, "w": SHINE, "f": (206, 222, 240), "k": (52, 62, 58)}
    return [([_stamp(cx, cy + 3, rows, pal, where="out")], None, 0)]


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
def compose(parts, ops=(), fx=(), clip=None):
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
    if _PX["collect"] is not None:
        box = ImageChops.lighter(soft, sharp).getbbox()
        if box:
            _PX["collect"].append(box)
        return Image.new("RGBA", (1, 1))
    grid = _PX["grid"]
    n = grid.n
    sil = (grid.down(soft) >= 128) | (grid.down(sharp) >= 90)
    sil = _clean(sil)
    img = np.zeros((n, n, 4), np.uint8)
    base = parts[0].base
    img[sil] = base + (255,)
    outer = _dilate(sil) & ~sil
    img[outer] = _ink(base) + (255,)
    sil_full = ImageChops.lighter(soft, sharp)
    for p, m, k in zip(parts, raw, spk):
        if p.outline:
            mm = dp.melt(m, 1.0)
            if k is not None:
                mm = ImageChops.lighter(mm, k)
            region = (grid.down(ImageChops.multiply(mm, sil_full)) >= 128) & sil
            ring = _dilate(region) & ~region & _erode(sil)  # 바깥 외곽선과 붙은 칸은 두 겹이 되니 뺀다
            small = region.sum() < SMALL_PART
            if small and region.any():  # 작은 앞발은 둘레를 다 두르면 동그라미가 된다 → 아래쪽만
                ring &= np.arange(n)[:, None] > np.nonzero(region)[0].mean()
            img[ring] = dp.mix(_ink(p.base or base), p.base or base, 0.55 if small else 0.25) + (255,)
        else:
            small = False
            if k is not None:
                m = ImageChops.lighter(m, k)
            grown = dp.grow(m, p.grow) if p.grow else m
            region = (grid.down(ImageChops.multiply(grown, sil_full)) >= 128) & sil
        if p.base:
            img[region] = p.base + (255,)
        for fns, color in p.marks:
            if small and color == dp._INK["line"]:  # 작은 앞발의 발가락 선은 칸이 모자라 얼룩이 된다
                continue
            img[_fns_dots(fns, grid) & region] = tuple(color) + (255,)
    occ = sil | outer
    face = np.zeros((n, n), bool)
    for fns, color in ops:
        for f in fns:
            if isinstance(f, Stamp):
                _place_in(f, img, sil, n, face) if f.where == "in" else _place_out(f, img, occ, n)
        rest = [f for f in fns if not isinstance(f, Stamp)]
        if rest:
            d = _fns_dots(rest, grid)
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


# ---------------------------------------------------------------- 문맥·격자·렌더
_PATCH = dict(compose=compose, stage_eye=stage_eye, blush=blush, open_mouth=open_mouth, front_mouth=front_mouth, side_mouth=side_mouth,
              brows=brows, z_fx=z_fx, heart_fx=heart_fx, question_fx=question_fx, anger_fx=anger_fx, sparkle_fx=sparkle_fx,
              thought_fx=thought_fx, back_arm=back_arm, E=_Ellipse, L=_Line, WHISKERS=[[(21, 61), (11, 59)], [(21, 67), (11, 69)]])


def _modules():
    names = ("draw_frog", "draw_halloween", "draw_farm", "draw_christmas", "draw_dino", "draw_snowman")
    return [dp] + [sys.modules[m] for m in names if m in sys.modules]


@contextmanager
def pixel_mode(grid, collect=None):
    """이 문맥 안에서만 그림 코드가 도트를 찍는다 (from draw_pets import …로 받은 다른 모듈의 이름까지). 나오면 원래대로."""
    saved = []
    for mod in _modules():
        for name, new in _PATCH.items():
            cur = getattr(mod, name, None)
            if cur is _ORIG[name]:
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


def _anims(sp):
    return [(a, n) for a, n, *_ in dp.rows_for(sp) if a not in NO_ANIM]


_GRIDS = {}


def stage_grid(sp, stage):
    """모든 프레임의 실루엣 경계에 맞춘 격자 (단계마다 고정 — 프레임마다 바꾸면 크기가 튄다)."""
    key = (sp.id, stage)
    if key in _GRIDS:
        return _GRIDS[key]
    n = STAGE_GRID[stage][0]
    boxes = []
    dummy = Grid(n, 12, 0, 0)
    with dp.stage(stage, sp.stage_tweak.get(stage)), pixel_mode(dummy, collect=boxes):
        ssp = dp.staged_species(sp)
        for anim, cnt in _anims(sp):
            dp.frames(ssp, anim, cnt)
    x0, y0 = min(b[0] for b in boxes), min(b[1] for b in boxes)
    x1, y1 = max(b[2] for b in boxes), max(b[3] for b in boxes)
    half = max(dp.S / 2 - x0, x1 - dp.S / 2)
    c = math.ceil(max(half / (n / 2 - 1), (y1 - y0) / (n - 2)))
    _GRIDS[key] = Grid(n, c, dp.S // 2 - c * n // 2, y1 - c * (n - 1), half / K, y0 / K)
    return _GRIDS[key]


def render_pixel(sp, stage, anims=None):
    """{동작: [n×n RGBA 프레임]}."""
    grid = stage_grid(sp, stage)
    out = {}
    with dp.stage(stage, sp.stage_tweak.get(stage)), pixel_mode(grid):
        ssp = dp.staged_species(sp)
        for anim, cnt in _anims(sp):
            if anims is None or anim in anims:
                out[anim] = dp.frames(ssp, anim, cnt)
    return out


# ---------------------------------------------------------------- 미리보기
LIGHT, DARK = (240, 240, 244), (44, 46, 58)
KEY_FRAMES = [("walk", 0), ("walk", 1), ("sit_front", 0), ("sleep", 0), ("hungry", 1), ("heart", 2), ("tilt", 1), ("climb", 0), ("climb", 2),
              ("slide", 1), ("lookup", 2), ("wave", 1), ("eat", 0), ("level_up", 2), ("huff", 1), ("arch", 2)]


def _blit(canvas, im, x, y, scale):
    """밑선 맞춰 정수 배율로 붙인다."""
    canvas.alpha_composite(im.resize((im.width * scale, im.height * scale), Image.NEAREST), (x, y))


def _round_real(stage, by_round, anim, i):
    k = dp.STAGE_SCALE[stage]
    im = by_round[anim][i]
    return im.resize((round(dp.FRAME * k), round(dp.FRAME * k)), Image.LANCZOS)


def _panel_compare(rendered, rounds, c_frames):
    """실제 표시 크기: 줄 = 단계 × (동글 / 직접 / c), 왼쪽 밝은 바탕 · 오른쪽 어두운 바탕."""
    cell, label, pad = 150, 92, 12
    width = label + (cell * len(KEY_FRAMES)) * 2 + pad
    blocks = len(dp.STAGES) * (3 * cell + pad) + 40
    img = Image.new("RGBA", (width, blocks), (255, 255, 255, 255))
    d = ImageDraw.Draw(img)
    font = ImageFont.load_default()
    d.rectangle((label, 18, label + cell * len(KEY_FRAMES), blocks), fill=LIGHT + (255,))
    d.rectangle((label + cell * len(KEY_FRAMES), 18, width, blocks), fill=DARK + (255,))
    d.text((4, 2), "REAL SIZE  round (LANCZOS, current app scale) / direct (draw_pixel) / method c (pixelize.py)", fill=(40, 40, 40), font=font)
    for side in (0, 1):
        for c, (anim, i) in enumerate(KEY_FRAMES):
            d.text((label + side * cell * len(KEY_FRAMES) + c * cell + 4, 20), f"{anim} {i}", fill=(120, 120, 130) if side else (70, 70, 80), font=font)
    for r, st in enumerate(dp.STAGES):
        n, sc = STAGE_GRID[st]
        y0 = 36 + r * (3 * cell + pad)
        for line, (name, get) in enumerate((("round", lambda a, i: _round_real(st, rounds[st], a, i)),
                                            ("direct", lambda a, i: rendered[st][a][i]),
                                            ("c", lambda a, i: c_frames[st][(a, i)]))):
            d.text((4, y0 + line * cell + 40), f"{st} {n}x{sc}\n={n * sc}px\n{name}" if name != "round" else f"{st}\nround {round(128 * dp.STAGE_SCALE[st])}px",
                   fill=(40, 40, 40), font=font)
            for side in (0, 1):
                for c, (anim, i) in enumerate(KEY_FRAMES):
                    im = get(anim, i)
                    s = 1 if name == "round" else sc
                    x = label + side * cell * len(KEY_FRAMES) + c * cell + (cell - im.width * s) // 2
                    _blit(img, im, x, y0 + line * cell + cell - im.height * s - 2, s)
    return img


def _panel_all(rendered, enlarged):
    """모든 동작·프레임. enlarged: 칸 크기 일정하게 확대(밝은 바탕), 아니면 실제 크기(밝은 → 어두운)."""
    font = ImageFont.load_default()
    cols = 20
    tile = 216 if enlarged else 150
    total = sum(len(v) for v in rendered["baby"].values())
    rows_n = math.ceil(total / cols)
    strips = 1 if enlarged else 2
    H = (rows_n * (tile + 14) * strips + 20) * len(dp.STAGES)
    img = Image.new("RGBA", (cols * tile + 4, H), (255, 255, 255, 255))
    d = ImageDraw.Draw(img)
    y = 0
    for st in dp.STAGES:
        n, sc = STAGE_GRID[st]
        s = (tile - 4) // n if enlarged else sc
        d.text((4, y + 4), f"{st}  {n} dots x{sc} = {n * sc}px  " + (f"enlarged x{s}" if enlarged else "REAL SIZE"), fill=(40, 40, 40), font=font)
        y += 20
        for bg in ((LIGHT,) if enlarged else (LIGHT, DARK)):
            d.rectangle((0, y, img.width, y + rows_n * (tile + 14)), fill=bg + (255,))
            k = 0
            fg = (60, 60, 70) if bg == LIGHT else (190, 190, 200)
            for anim, ims in rendered[st].items():
                for i, im in enumerate(ims):
                    cx, cy = (k % cols) * tile, y + (k // cols) * (tile + 14)
                    d.text((cx + 3, cy), f"{anim} {i}", fill=fg, font=font)
                    _blit(img, im, cx + (tile - im.width * s) // 2, cy + 14 + tile - im.height * s - 2, s)
                    k += 1
            y += rows_n * (tile + 14)
    return img


def make_preview(path):
    import pixelize
    t0 = time.perf_counter()
    rendered = {st: render_pixel(dp.CAT, st) for st in dp.STAGES}
    t_direct = time.perf_counter() - t0
    rounds = {st: dp.render_species(dp.CAT, st) for st in dp.STAGES}
    t0 = time.perf_counter()
    c_frames = {st: {(a, i): Image.fromarray(pixelize.approach_c("cat", st, a, i, STAGE_GRID[st][0])) for a, i in KEY_FRAMES} for st in dp.STAGES}
    t_c = (time.perf_counter() - t0) / (len(dp.STAGES) * len(KEY_FRAMES))
    frames_n = sum(len(v) for r in rendered.values() for v in r.values())
    panels = [_panel_compare(rendered, rounds, c_frames), _panel_all(rendered, False), _panel_all(rendered, True)]
    W = max(p.width for p in panels)
    H = sum(p.height for p in panels) + 20 * len(panels)
    img = Image.new("RGBA", (W, H), (255, 255, 255, 255))
    y = 0
    for p in panels:
        img.alpha_composite(p, (0, y))
        y += p.height + 20
    img.save(path)
    print(f"direct: {frames_n} frames in {t_direct:.1f}s ({t_direct / frames_n * 1000:.0f} ms/frame, incl. fit pass) · c: {t_c * 1000:.0f} ms/frame · {path}")
    for st in dp.STAGES:
        g = stage_grid(dp.CAT, st)
        print(st, f"n={g.n} cell={g.cell:.2f}px half={g.half_px:.1f} top={g.top_px:.1f}")
    return panels


if __name__ == "__main__":
    make_preview(Path(__file__).resolve().parent / "preview" / "pixel_cat.png")
