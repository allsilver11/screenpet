"""픽셀 스타일의 세계 그림 — 동물 시트(tools/draw_pixel.py)가 아닌 것 전부. 변환이 아니라 칸을 직접 찍는다.

    python tools/draw_pixel_world.py   → ui/house_pixel/  {house,haunted,xmas}_<stage>{.png,_front.png,.json} + xmas_smoke.png
                                         ui/effects_pixel/ 소품·손 음식·눈덩이(4)·눈사람·눈구름·모자·목도리·효과
                                         ui/gifts_pixel/   선물 전부 + _shiny
                                         ui/objects_pixel/ 물건 8 + 똥 3 + 포스트잇
                                         tools/preview/pixel_world_*.png

한 칸 크기 (어디서나 화면 3px — 동물·세계 그림이 고르다):
  - 동물: 전 종 같은 표시 크기 72/84/96/96/108 px (칸 3 — 코어 style.rs, 동글 stage_size는 곱하지 않는다). 공룡만 칸 수가 적어 60/72/84/84/96 px (pet.json frame_sizes).
  - 집: 그 단계 동물과 같은 칸. 문은 그 단계 시트 한 칸 전체(frame n칸)가 들어가는 넓이 (n+2)×(n+1)라서
    어느 동물이든 줄이지 않고 그대로 앉는다 — 그래서 집은 동글보다 크다 (114/126/144/144/162 px, 동글 70/87/105/128/143).
  - 밥그릇·쿠키: 그 단계 칸. 손 음식·선물·물건·눈사람: 같은 칸 3 (동글도 크기가 고정이다).
"""

import hashlib
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pixel_world_icons as icons  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
PREVIEW = ROOT / "tools" / "preview"

# 단계 → (시트 한 칸 수, 칸 크기) — tools/draw_pixel.py STAGE_GRID, 코어 style::pixel_grid와 같다
STAGE_GRID = {"baby": (24, 3), "kinder": (28, 3), "child": (32, 3), "teenager": (32, 3), "adult": (36, 3)}
# 집 그림 한 변 (칸)
HOUSE_DOTS = {"baby": 38, "kinder": 42, "child": 48, "teenager": 48, "adult": 54}
KINDS = ("house", "haunted", "xmas")
# 그 집에 사는 동물 (문 크기 확인용)
RESIDENTS = {"haunted": ("pumpkin", "ghost"), "xmas": ("rudolph", "santa", "snowman"),
             "house": ("cat", "persian", "rabbit", "frog", "duck", "giraffe", "dino_yellow", "dino_green", "dino_purple", "bird", "penguin", "redpanda")}
RESIDENTS["house"] += ("squirrel",)
GIFT_DOTS = 12
HAND_DOTS = 12
PROP_DOTS = (14, 12)
SNOWMAN_DOTS = (16, 20)
SNOWBALL_DOTS = (10, 10)
CLOUD_DOTS = (14, 9)
OBJECT_DOTS = 24
POSTIT_DOTS = 6
# 모자·목도리 띠 (한 칸 = 머리 너비 하나): ui/main.js와 같다
CAP_CELL = (40, 30)
CAP_ANCHOR = (20, 24)
CAP_WIDTHS = range(4, 27)
SCARF_CELL = (48, 24)
SCARF_ANCHOR = (24, 6)
SCARF_WIDTHS = range(6, 31)
SCARF_VIEWS = ("front", "side", "back")
SMOKE_DOTS = 8

INK = (88, 62, 60)
PAL = {
    "r": (232, 84, 84), "R": (176, 48, 60), "p": (250, 176, 192), "P": (236, 110, 150),
    "o": (244, 150, 60), "O": (200, 104, 40), "y": (252, 214, 84), "Y": (232, 170, 48),
    "g": (124, 190, 96), "G": (72, 140, 84), "l": (184, 226, 144),
    "b": (106, 160, 232), "B": (64, 100, 176), "c": (190, 226, 248),
    "v": (164, 130, 226), "V": (104, 76, 160),
    "n": (170, 110, 72), "N": (112, 70, 50), "t": (222, 176, 124), "T": (244, 220, 176),
    "w": (255, 255, 255), "W": (250, 246, 236), "s": (176, 176, 188), "S": (112, 112, 126),
    "k": (52, 44, 52), "f": (255, 214, 196), "m": (255, 240, 200),
    "e": (138, 92, 58), "d": (128, 122, 66), "D": (96, 90, 50),
}


def mix(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


# ---------------------------------------------------------------- 엔진
class Dots:
    """칸 캔버스. 좌표는 칸, 끝점 포함. 그림은 안티에일리어싱 없이 불투명 칸만."""

    def __init__(self, w, h=None):
        self.w, self.h = w, h if h is not None else w
        self.a = np.zeros((self.h, self.w, 4), np.uint8)

    def px(self, x, y, col):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.a[y, x] = (*col, 255)

    def rect(self, x0, y0, x1, y1, col):
        x0, y0, x1, y1 = max(x0, 0), max(y0, 0), min(x1, self.w - 1), min(y1, self.h - 1)
        if x0 <= x1 and y0 <= y1:
            self.a[y0:y1 + 1, x0:x1 + 1] = (*col, 255)

    def clear(self, x0, y0, x1, y1):
        self.a[max(y0, 0):y1 + 1, max(x0, 0):x1 + 1] = 0

    def disc(self, cx, cy, r, col):
        for y in range(math.floor(cy - r), math.ceil(cy + r) + 1):
            for x in range(math.floor(cx - r), math.ceil(cx + r) + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r + 0.3:
                    self.px(x, y, col)

    def stamp(self, rows, x, y, pal=None):
        pal = pal or PAL
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch != ".":
                    self.px(x + i, y + j, pal[ch])

    def outlined(self, ink=INK):
        """빈칸 쪽으로 1칸 외곽선을 두른 새 캔버스."""
        out = Dots(self.w, self.h)
        solid = self.a[..., 3] > 0
        near = np.zeros_like(solid)
        near[1:] |= solid[:-1]
        near[:-1] |= solid[1:]
        near[:, 1:] |= solid[:, :-1]
        near[:, :-1] |= solid[:, 1:]
        out.a[near & ~solid] = (*ink, 255)
        out.a[solid] = self.a[solid]
        return out

    def over(self, other, x=0, y=0):
        """other를 (x, y)에 얹는다 (불투명 칸만)."""
        h, w = other.a.shape[:2]
        for j in range(h):
            for i in range(w):
                if other.a[j, i, 3] and 0 <= x + i < self.w and 0 <= y + j < self.h:
                    self.a[y + j, x + i] = other.a[j, i]
        return self

    def add(self, layer, ink=INK):
        return self.over(layer.outlined(ink))

    def image(self):
        return Image.fromarray(self.a, "RGBA")


def icon(text, w, h=None, bottom=False, ink=INK, pal=None):
    """표 → (w, h)칸 캔버스 가운데 (bottom이면 아래에 붙여) + 외곽선."""
    h = h or w
    rows = icons.art(text)
    aw = max(len(r) for r in rows)
    x = (w - aw) // 2
    y = h - len(rows) - 1 if bottom else (h - len(rows)) // 2
    c = Dots(w, h)
    c.stamp(rows, x, y, pal)
    return c.outlined(ink) if ink else c


def save(c, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    img = c.image() if isinstance(c, Dots) else c
    img.save(path, optimize=True)


# ---------------------------------------------------------------- 집 공통
WALL, PLANK = (255, 238, 212), (226, 200, 170)
WOOD, WOOD_LIGHT, WOOD_DARK = (214, 160, 112), (240, 200, 156), (150, 104, 76)
INSIDE, FLOOR_IN, SHADE_IN = (92, 64, 62), (118, 84, 76), (74, 50, 50)
GRASS, GRASS_DARK = (150, 204, 124), (104, 164, 96)
GLASS = (206, 230, 244)
CURTAIN = (250, 176, 192)
BRICK, MORTAR = (206, 92, 80), (236, 178, 152)
SNOW, SNOW_SHADE = (255, 255, 255), (204, 224, 244)
H_WALL, H_PLANK, H_ROOF, H_ROOF_DARK = (184, 168, 222), (156, 138, 198), (118, 98, 166), (90, 72, 136)
H_WOOD, H_WOOD_LIGHT = (176, 128, 104), (210, 170, 146)
H_INSIDE, H_FLOOR, H_SHADE = (66, 52, 84), (92, 72, 102), (50, 40, 66)
GLOW, GLOW_CORE = (255, 186, 84), (255, 234, 150)
NIGHT = (104, 112, 160)
STRAW, STRAW_DARK, STRAW_LIGHT = (240, 198, 112), (196, 150, 76), (250, 224, 160)


def door_of(stage, x):
    """문 사각형 (칸): 그 단계 시트 한 칸이 양옆 1칸(외곽선)·위 1칸을 남기고 들어간다. 바닥 = 집 바닥에서 2칸 위."""
    n = STAGE_GRID[stage][0]
    h = n + 1
    return dict(x=x, y=HOUSE_DOTS[stage] - 2 - h, w=n + 2, h=h)


def inside(back, d, col=INSIDE, floor=FLOOR_IN, shade=SHADE_IN):
    """어두운 집 안 — 위쪽은 더 어둡고 바닥은 밝다."""
    x0, y0, x1, y1 = d["x"], d["y"], d["x"] + d["w"] - 1, d["y"] + d["h"] - 1
    back.rect(x0, y0, x1, y1, col)
    back.rect(x0, y0, x1, y0 + d["h"] // 5, shade)
    back.rect(x0, y1 - 1, x1, y1, floor)


def door_front(front, d, frame=WOOD, light=WOOD_LIGHT, sill=WOOD_LIGHT, corner=(3, 2, 1), ink=INK):
    """문틀(양옆 2칸, 위 2칸, 안쪽 위 모서리는 계단) + 문턱 — 동물 앞에 온다."""
    x0, y0, x1, y1 = d["x"], d["y"], d["x"] + d["w"] - 1, d["y"] + d["h"] - 1
    f = Dots(front.w, front.h)
    f.rect(x0 - 2, y0 - 2, x0 - 1, y1, frame)
    f.rect(x1 + 1, y0 - 2, x1 + 2, y1, frame)
    f.rect(x0 - 2, y0 - 2, x1 + 2, y0 - 1, frame)
    # 아치처럼 보이는 위 모서리 (안쪽으로 3·2·1칸)
    for i, n in enumerate(corner):
        f.rect(x0, y0 + i, x0 + n - 1, y0 + i, frame)
        f.rect(x1 - n + 1, y0 + i, x1, y0 + i, frame)
    f.clear(x0 - 2, y0 - 2, x0 - 2, y0 - 2)
    f.clear(x1 + 2, y0 - 2, x1 + 2, y0 - 2)
    f.rect(x0 - 1, y0 + 2, x0 - 1, y1, light)
    f.rect(x1 + 1, y0 + 2, x1 + 1, y1, light)
    f.rect(x0 + 1, y0 - 2, x1 - 1, y0 - 2, light)
    f.rect(x0 - 3, y1 + 1, x1 + 3, y1 + 2, sill)
    f.rect(x0 - 2, y1 + 1, x1 + 2, y1 + 1, mix(sill, (255, 255, 255), 0.5))
    # 외곽선은 문 안쪽 1칸까지 (문 크기가 그만큼 여유 있다), 그러나 바닥은 문턱 아래까지 트지 않는다
    out = f.outlined(ink)
    out.clear(x0 + 1, y1 + 3, x1 - 1, front.h)  # 바닥 아래 외곽선 대신 잔디·땅
    front.over(out)


def gable(c, cx, top, half, eave, col, dark, light=None, step=3):
    """계단 박공 지붕 + 기와 무늬. half = 처마 반너비."""
    L = Dots(c.w, c.h)
    n = eave - top
    for i in range(n + 1):
        h = 3 + round((half - 3) * min(1.0, (i / n) ** 0.85))
        L.rect(cx - h, top + i, cx + h, top + i, col)
    for row, y in enumerate(range(eave - 2, top, -step)):
        h = 3 + round((half - 3) * min(1.0, ((y - top) / n) ** 0.85))
        for x in range(cx - h + (row % 2) * 2, cx + h, 4):
            L.rect(x, y, x + 1, y, dark)
    L.rect(cx - half, eave, cx + half, eave, dark)
    if light:
        L.rect(cx - 5, top + 1, cx - 1, top + 1, light)
    c.add(L)


def planks(c, x0, y0, x1, y1, col=WALL, line=PLANK, step=5):
    L = Dots(c.w, c.h)
    L.rect(x0, y0, x1, y1, col)
    for y in range(y0 + step, y1, step):
        L.rect(x0, y, x1, y, line)
    c.add(L)


def window(c, x, y, w, h, frame=WOOD, glass=GLASS, curtain=CURTAIN):
    """네모 창: 테 1칸 + 유리 + 커튼 양옆 + 가로 창살."""
    L = Dots(c.w, c.h)
    L.rect(x, y, x + w - 1, y + h - 1, frame)
    L.rect(x + 1, y + 1, x + w - 2, y + h - 2, glass)
    if curtain:
        L.px(x + 1, y + 1, curtain)
        L.px(x + w - 2, y + 1, curtain)
        L.px(x + 1, y + 2, curtain)
        L.px(x + w - 2, y + 2, curtain)
    L.rect(x + 1, y + h // 2, x + w - 2, y + h // 2, frame)
    c.add(L)


def chimney(c, x, top, bottom, col=(222, 150, 124), w=5):
    L = Dots(c.w, c.h)
    L.rect(x, top + 1, x + w - 1, bottom, col)
    L.rect(x - 1, top, x + w, top + 1, mix(col, INK, 0.2))
    c.add(L)


TUFT = ["g.g", "ggg"]


def ground(front, xs, col=None):
    pal = dict(PAL, g=col or GRASS)
    H = front.h
    L = Dots(front.w, H)
    for x in xs:
        L.stamp(TUFT, x, H - 2, pal)
    front.over(L.outlined())


# ---------------------------------------------------------------- 집 (house)
def draw_house(stage):
    H = HOUSE_DOTS[stage]
    back, front = Dots(H), Dots(H)
    if stage == "baby":  # 아기 버섯집
        d = door_of(stage, 5)
        L = Dots(H)
        L.rect(3, 9, H - 4, H - 2, (255, 244, 226))
        back.add(L)
        C = Dots(H)
        for i in range(13):
            h = 8 + round((H / 2 - 9) * min(1, (i / 9) ** 0.6))
            C.rect(H // 2 - h, 1 + i, H // 2 + h - 1, 1 + i, (248, 170, 186))
        for sx, sy, sw in ((5, 7, 3), (15, 3, 5), (28, 5, 3), (33, 10, 2), (1, 11, 2)):
            C.rect(sx, sy, sx + sw - 1, sy + 1, (255, 255, 255))
        C.rect(1, 13, H - 2, 13, mix((248, 170, 186), INK, 0.3))
        back.add(C)
        inside(back, d)
        door_front(front, d, frame=(172, 222, 204), light=(214, 240, 228), sill=(214, 240, 228))
        ground(front, (0, H - 3))
    elif stage == "kinder":  # 파란 지붕 오두막
        d = door_of(stage, 3)
        planks(back, 2, 10, H - 3, H - 2)
        gable(back, H // 2, 0, H // 2 - 1, 11, (132, 170, 214), mix((132, 170, 214), INK, 0.35), (190, 214, 240))
        inside(back, d)
        window(back, H - 7, 18, 5, 6)
        door_front(front, d)
        ground(front, (H - 5,))
    elif stage == "child":  # 빨간 지붕 + 깃발
        d = door_of(stage, 3)
        L = Dots(H)
        L.rect(H // 2, 0, H // 2, 5, WOOD_DARK)
        L.stamp(["yyyy", "yyyyy", "yyyy"], H // 2 + 1, 0, PAL)
        planks(back, 2, 10, H - 3, H - 2)
        gable(back, H // 2, 3, H // 2 - 1, 13, (204, 108, 92), mix((204, 108, 92), INK, 0.35), (230, 160, 146))
        back.over(L)
        inside(back, d)
        window(back, H - 8, 21, 6, 7)
        door_front(front, d)
        ground(front, (H - 5,))
    elif stage == "teenager":  # 굴뚝 + 발바닥 간판 + 창
        d = door_of(stage, 3)
        chimney(back, H - 12, 0, 10)
        planks(back, 2, 10, H - 3, H - 2)
        gable(back, H // 2, 2, H // 2 - 1, 13, (204, 108, 92), mix((204, 108, 92), INK, 0.35), (230, 160, 146))
        inside(back, d)
        window(back, H - 8, 20, 6, 7)
        back.stamp([".k.k.", "k.k.k", ".kkk."], d["x"] + d["w"] // 2 - 2, 6, {"k": (244, 220, 176)})
        back.stamp(["....f", "f.nnn", "fnnnn"], H - 7, 40, {"f": CURTAIN, "n": GRASS})
        door_front(front, d)
        ground(front, (H - 5,))
    else:  # 어른: 초록 지붕, 굴뚝, 꽃 상자 창
        d = door_of(stage, 3)
        chimney(back, H - 13, 0, 12)
        planks(back, 2, 12, H - 3, H - 2)
        gable(back, H // 2, 2, H // 2 - 1, 15, (124, 170, 128), mix((124, 170, 128), INK, 0.35), (170, 206, 170))
        inside(back, d)
        window(back, H - 9, 22, 7, 8)
        fl = Dots(H)
        fl.rect(H - 10, 31, H - 2, 33, (196, 128, 96))
        fl.stamp(["p.y.p.v.", ], H - 10, 30, PAL)
        back.add(fl)
        back.stamp(["..f..", ".fff.", "..g..", "..g.."], H - 8, 42, {"f": (166, 150, 230), "g": GRASS})
        door_front(front, d)
        ground(front, (H - 4,))
    return back, front, d, None


# ---------------------------------------------------------------- 귀신의 집
JACK = [".....g.", "..oooGoo", ".oOooOooo", ".oyOoOyoo", ".oooOoooo", ".oyoyoyoo", "..ooyyoo."]
BAT = ["V.......V", "VV.V.V.VV", "VVVVVVVVV", ".VVwVwVV.", "...VVV..."]
MINI_GHOST = [".www.", "wkwkw", "wwwww", "wwwww", "w.w.w"]


def glow_window(c, cx, cy, r):
    L = Dots(c.w, c.h)
    L.disc(cx, cy, r, H_WOOD)
    L.disc(cx, cy, r - 1, GLOW)
    L.disc(cx, cy, max(0.4, r - 2), GLOW_CORE)
    L.rect(cx, cy - r + 1, cx, cy + r - 1, H_WOOD)
    L.rect(cx - r + 1, cy, cx + r - 1, cy, H_WOOD)
    c.add(L)


def bent_roof(c, cx, top, half, eave, lean):
    """휜 뾰족 지붕: 끝이 lean칸 기울고 말린다."""
    L = Dots(c.w, c.h)
    n = eave - top
    for i in range(n + 1):
        t = i / n
        h = 1 + round((half - 1) * t ** 0.9)
        shift = round(lean * (1 - t) ** 1.5)
        L.rect(cx + shift - h, top + i, cx + shift + h, top + i, H_ROOF)
    for row, y in enumerate(range(eave - 2, top + 3, -3)):
        t = (y - top) / n
        h = 1 + round((half - 1) * t ** 0.9)
        shift = round(lean * (1 - t) ** 1.5)
        for x in range(cx + shift - h + (row % 2) * 2, cx + shift + h, 4):
            L.rect(x, y, x + 1, y, H_ROOF_DARK)
    L.rect(cx - half, eave, cx + half, eave, H_ROOF_DARK)
    # 말린 끝 + 방울
    L.rect(cx + lean, top, cx + lean + 2, top, H_ROOF)
    L.rect(cx + lean + 2, top, cx + lean + 2, top + 1, H_ROOF)
    L.px(cx + lean + 3, top + 2, GLOW_CORE)
    c.add(L)


def draw_haunted(stage):
    H = HOUSE_DOTS[stage]
    back, front = Dots(H), Dots(H)
    d = door_of(stage, 3 if stage != "baby" else 5)
    eave = d["y"] - 3 if stage != "baby" else d["y"] - 1
    # 기운 벽: 위로 갈수록 오른쪽으로 1칸씩
    L = Dots(H)
    for y in range(eave, H - 1):
        lean = 0 if y > eave + (H - eave) * 0.66 else (1 if y > eave + (H - eave) * 0.33 else 2)
        L.rect(2 + lean // 2, y, H - 3 + lean // 2, y, H_WALL)
    for y in range(eave + 5, H - 2, 5):
        L.rect(2, y, H - 2, y, H_PLANK)
    if stage in ("teenager", "adult", "child", "kinder"):  # 기운 굴뚝
        c2 = Dots(H)
        c2.rect(H - 13, 2, H - 9, eave, (164, 134, 160))
        c2.rect(H - 13, 1, H - 8, 2, (130, 104, 132))
        back.add(c2)
    back.add(L)
    bent_roof(back, H // 2 - 1, 2 if stage != "baby" else 1, H // 2 - 2, eave, 4)
    glow_window(back, H // 2 - 1, max(4, eave - 4), 3 if eave > 10 else 2)
    inside(back, d, H_INSIDE, H_FLOOR, H_SHADE)
    spare = H - (d["x"] + d["w"] + 2)
    if spare >= 6:
        G = Dots(H)
        wx, wy = H - spare, eave + 6
        ww, wh = spare - 1, 8
        G.rect(wx, wy, wx + ww - 1, wy + wh - 1, H_WOOD)
        G.rect(wx + 1, wy + 1, wx + ww - 2, wy + wh - 2, NIGHT)
        if ww >= 7:
            G.stamp(MINI_GHOST, wx + 1, wy + 2)
        G.px(wx + ww - 2, wy + 1, (244, 242, 252))
        G.px(wx + ww - 3, wy + 2, (244, 242, 252))
        back.add(G)
    door_front(front, d, frame=H_WOOD, light=H_WOOD_LIGHT, sill=H_WOOD_LIGHT)
    if stage != "baby":
        back.stamp(BAT, 1, 1)
    jack = Dots(H)
    if stage == "baby":  # 문 옆 자리가 좁다: 꼬마 호박
        jack.stamp(["..g", "ooo", "yoy", "ooo"], H - 4, H - 5)
    else:
        jack.stamp(JACK, H - 10, H - 8)
    front.over(jack.outlined())
    ground(front, (0,), (146, 190, 136))
    return back, front, d, None


# ---------------------------------------------------------------- 크리스마스 벽돌집
WREATH = ["..GGG..", ".GgrgG.", "GgG.GgG", "Gg...gG", "GgG.GgG", ".GgrgG.", "..GrG.."]
MINI_SNOWMAN = [".kkk.", ".kkk.", ".WWW.", "WkWkW", "WWoWW", ".RRR.", "WWWWW", "WWkWW", "WWWWW"]


def bricks(L, x0, y0, x1, y1):
    L.rect(x0, y0, x1, y1, BRICK)
    for row, y in enumerate(range(y0 + 2, y1 + 1, 3)):
        L.rect(x0, y, x1, y, MORTAR)
        for x in range(x0 + (row % 2) * 3, x1, 6):
            L.rect(x, y - 2, x, y - 1, MORTAR)


def draw_xmas(stage):
    H = HOUSE_DOTS[stage]
    back, front = Dots(H), Dots(H)
    d = door_of(stage, 3 if stage != "baby" else 5)
    eave = d["y"] - 2
    # 굴뚝 (오른쪽, 지붕 뒤) — 연기는 앱이 피운다
    cx0 = H - 11
    ch = Dots(H)
    bricks(ch, cx0, 2, cx0 + 5, eave)
    ch.rect(cx0 - 1, 1, cx0 + 6, 2, SNOW)
    back.add(ch)
    smoke = {"x": cx0 + 3, "y": 1}
    W = Dots(H)
    bricks(W, 2, eave, H - 3, H - 2)
    back.add(W)
    # 눈 덮인 지붕
    R = Dots(H)
    n = eave - 3
    half = H // 2 - 1
    for i in range(n + 1):
        h = 4 + round((half - 4) * (i / n) ** 0.85)
        R.rect(H // 2 - h, 3 + i, H // 2 + h - 1, 3 + i, SNOW)
    # 처마 아래로 늘어진 눈 (삐죽삐죽)
    for k, x in enumerate(range(1, H - 1, 5)):
        R.rect(x, eave + 1, x + 2, eave + 1 + (k % 3), SNOW)
    R.rect(H // 2 - half // 2, eave - 1, H // 2 + half // 2, eave - 1, SNOW_SHADE)
    back.add(R)
    inside(back, d, (86, 60, 60), (120, 84, 72), (66, 46, 48))
    spare = H - (d["x"] + d["w"] + 2)
    if spare >= 6:
        G = Dots(H)
        wx, wy, ww, wh = H - spare, eave + 7, spare - 1, 8
        G.rect(wx, wy, wx + ww - 1, wy + wh - 1, (120, 76, 60))
        G.rect(wx + 1, wy + 1, wx + ww - 2, wy + wh - 2, GLOW)
        G.rect(wx + 2, wy + 2, wx + ww - 3, wy + 3, GLOW_CORE)
        G.rect(wx + 1, wy + wh // 2, wx + ww - 2, wy + wh // 2, (120, 76, 60))
        back.add(G)
    # 리스 (지붕 눈 위)
    if eave >= 10:
        w = Dots(H)
        w.stamp(WREATH, d["x"] + d["w"] // 2 - 3, max(2, eave - 8))
        back.over(w)
    door_front(front, d, frame=(150, 96, 76), light=(196, 142, 114), sill=(230, 220, 214))
    # 앞마당 눈 더미 + 작은 눈사람
    S = Dots(H)
    S.rect(0, H - 2, 3, H - 1, SNOW)
    S.rect(1, H - 3, 2, H - 3, SNOW)
    S.rect(H - 6, H - 2, H - 1, H - 1, SNOW)
    S.rect(H - 5, H - 3, H - 2, H - 3, SNOW)
    if stage in ("teenager", "adult", "child"):
        S.stamp(MINI_SNOWMAN, H - 6, H - 12)
    front.over(S.outlined(mix(INK, SNOW_SHADE, 0.2)))
    return back, front, d, smoke


DRAW = {"house": draw_house, "haunted": draw_haunted, "xmas": draw_xmas}


def smoke_strip():
    """연기 네 장면 (옆으로 이어 붙임): 작은 모금 → 커짐 → 듬성듬성 사라짐 (흐림 없이 칸을 뺀다)."""
    D, L = (120, 120, 130), (168, 168, 178)
    frames = []
    a = Dots(SMOKE_DOTS)
    a.rect(3, 4, 4, 5, L)
    frames.append(a)
    b = Dots(SMOKE_DOTS)
    b.rect(2, 3, 5, 6, D)
    b.rect(3, 2, 4, 7, D)
    b.rect(3, 3, 4, 4, L)
    frames.append(b)
    c = Dots(SMOKE_DOTS)
    c.rect(1, 2, 6, 6, D)
    c.rect(2, 1, 5, 7, D)
    c.rect(2, 2, 4, 4, L)
    frames.append(c)
    e = Dots(SMOKE_DOTS)
    for y in range(1, 8):
        for x in range(1, 7):
            if (x + y) % 2 == 0:
                e.px(x, y, D)
    frames.append(e)
    strip = Dots(SMOKE_DOTS * 4, SMOKE_DOTS)
    for i, f in enumerate(frames):
        strip.over(f, i * SMOKE_DOTS, 0)
    return strip


# ---------------------------------------------------------------- 모자·목도리·효과
CAP_BLUE, CAP_DARK, CAP_LINE, CREAM = (111, 134, 201), (84, 104, 170), (63, 79, 134), (255, 247, 232)
SCARF_LIGHT = (142, 162, 221)


def nightcap_cell(w):
    """모자 하나: 입구 가운데가 CAP_ANCHOR, 입구 너비 w. 시트는 오른쪽을 보니 꼬리는 왼쪽(머리 뒤)으로."""
    c = Dots(*CAP_CELL)
    ax, ay = CAP_ANCHOR
    h = max(4, round(w * 0.62))
    x0 = ax - w // 2
    top_shift = 0
    for i in range(h):
        t = i / h
        half = max(1, round((w / 2) * (1 - t) ** 0.8))
        top_shift = -round(t * t * w * 0.5)
        c.rect(ax + top_shift - half, ay - 2 - i, ax + top_shift + half - 1, ay - 2 - i, CAP_BLUE)
        c.px(ax + top_shift + half - 1, ay - 2 - i, CAP_DARK)
    r = 1 if w < 12 else 2
    c.disc(ax + top_shift - 1, ay - 1 - h - r + 1, r, CREAM)
    c.rect(x0 - 1, ay - 1, x0 + w, ay + (0 if w < 10 else 1), CREAM)
    return c.outlined(CAP_LINE)


def scarf_cell(w, view):
    c = Dots(*SCARF_CELL)
    ax, ay = SCARF_ANCHOR
    x0, x1 = ax - w // 2, ax - w // 2 + w - 1
    band = max(3, round(w / 5))
    c.rect(x0, ay - band // 2, x1, ay - band // 2 + band - 1, CAP_BLUE)
    c.rect(x0, ay - band // 2 + band - 1, x1, ay - band // 2 + band - 1, SCARF_LIGHT)
    tw, th = max(3, round(w / 5)), max(4, round(w / 3))
    tx = {"front": x1 - tw - max(1, w // 8), "side": x0, "back": ax + max(1, w // 8)}[view]
    ty = ay - band // 2 + band
    c.rect(tx, ty, tx + tw - 1, ty + th - 1, CAP_BLUE)
    c.rect(tx, ty + th, tx + tw - 1, ty + th, CREAM)
    for x in range(tx, tx + tw, 2):
        c.px(x, ty + th + 1, CREAM)
    if view == "front":  # 매듭
        c.rect(tx - 1, ty - 2, tx + tw, ty, SCARF_LIGHT)
    return c.outlined(CAP_LINE)


def build_overlays():
    out = UI / "effects_pixel"
    strip = Dots(CAP_CELL[0] * len(CAP_WIDTHS), CAP_CELL[1])
    for i, w in enumerate(CAP_WIDTHS):
        strip.over(nightcap_cell(w), i * CAP_CELL[0], 0)
    save(strip, out / "nightcap.png")
    grid = Dots(SCARF_CELL[0] * len(SCARF_WIDTHS), SCARF_CELL[1] * len(SCARF_VIEWS))
    for j, v in enumerate(SCARF_VIEWS):
        for i, w in enumerate(SCARF_WIDTHS):
            grid.over(scarf_cell(w, v), i * SCARF_CELL[0], j * SCARF_CELL[1])
    save(grid, out / "scarf.png")
    for name, text in icons.FX.items():
        rows = icons.art(text)
        c = Dots(max(len(r) for r in rows), len(rows))
        c.stamp(rows, 0, 0)
        save(c, out / f"{name}.png")


# ---------------------------------------------------------------- 내보내기
SPARK_S = [".w.", "wyw", ".w."]


def shiny(base):
    """반짝이: 금빛으로 물들이고 네 갈래 반짝이 셋 (둥근 선물과 같은 뜻)."""
    a = base.a.copy()
    solid = a[..., 3] > 0
    lum = (a[..., 0] * 0.3 + a[..., 1] * 0.59 + a[..., 2] * 0.11)[..., None] / 255
    gold = np.array((255, 208, 92), float)
    dark = np.array((150, 96, 40), float)
    tinted = dark + (gold - dark) * np.clip(lum * 1.25, 0, 1)
    rgb = a[..., :3] * 0.3 + tinted * 0.7
    a[..., :3] = np.where(solid[..., None], rgb, a[..., :3]).astype(np.uint8)
    out = Dots(base.w, base.h)
    out.a = a
    out.stamp(SPARK_S, 0, 0)
    out.stamp(SPARK_S, base.w - 3, 1)
    out.stamp(SPARK_S, 1, base.h - 3)
    return out


def build_gifts():
    out = UI / "gifts_pixel"
    for name, text in icons.GIFTS.items():
        c = icon(text, GIFT_DOTS)
        save(c, out / f"{name}.png")
        save(shiny(c), out / f"{name}_shiny.png")


def build_effects():
    out = UI / "effects_pixel"
    for name, text in icons.HAND.items():
        c = icon(text, HAND_DOTS)
        if name in icons.HAND_LINES:
            c.stamp(icons.art(icons.HAND_LINES[name]), 0, 0)
        save(c, out / f"hand_{name}.png")
    for name, text in icons.PROPS.items():
        save(icon(text, PROP_DOTS[0], PROP_DOTS[1], bottom=True), out / f"{name}.png")
    # 눈덩이: 네 크기 (같은 칸 크기로 자란다), snowball.png가 다 자란 것
    for i, text in enumerate(icons.SNOWBALLS):
        c = icon(text, SNOWBALL_DOTS[0], SNOWBALL_DOTS[1], bottom=True, ink=(120, 150, 190))
        save(c, out / (f"snowball_{i}.png" if i < 3 else "snowball.png"))
    save(icon(icons.SNOWMAN, *SNOWMAN_DOTS, bottom=True), out / "snowman.png")
    save(icon(icons.SNOW_CLOUD, *CLOUD_DOTS, ink=(150, 170, 200)), out / "snow_cloud.png")


def build_objects():
    out = UI / "objects_pixel"
    for name, text in icons.OBJECTS.items():
        c = Dots(OBJECT_DOTS)
        c.stamp(icons.art(text), 0, 0)
        save(c.outlined(), out / f"{name}.png")
    save(icon(icons.POSTIT, POSTIT_DOTS, ink=(190, 150, 60)), out / "postit.png")


def build_houses():
    out = UI / "house_pixel"
    out.mkdir(parents=True, exist_ok=True)
    for kind in KINDS:
        for st in STAGES:
            back, front, d, smoke = DRAW[kind](st)
            save(back, out / f"{kind}_{st}.png")
            save(front, out / f"{kind}_{st}_front.png")
            meta = {"dots": HOUSE_DOTS[st], "dot": STAGE_GRID[st][1], "door": d}
            if smoke:
                meta["smoke"] = smoke
            (out / f"{kind}_{st}.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    save(smoke_strip(), out / "xmas_smoke.png")


# ---------------------------------------------------------------- 미리보기
LIGHT, DARK = (244, 240, 232, 255), (52, 58, 82, 255)


def scaled(img, k):
    return img.resize((img.width * k, img.height * k), Image.NEAREST)


def pet_cell(pid, stage, anim):
    """픽셀 시트의 그 동작 첫 칸 → (pet.json, 칸 수, 그림)."""
    p = UI / "pets_pixel" / pid
    m = json.loads((p / "pet.json").read_text(encoding="utf-8"))
    n = m["frame_sizes"][stage]
    row = m["animations"][anim]["row"]
    return m, n, Image.open(p / f"sheet_{stage}.png").convert("RGBA").crop((0, row * n, n, (row + 1) * n))


def door_pet(kind, stage):
    """문간에 앉힐 동물 (시트가 있으면) — sit_front 첫 칸."""
    for pid in RESIDENTS[kind]:
        p = UI / "pets_pixel" / pid
        if (p / "pet.json").is_file() and (p / f"sheet_{stage}.png").is_file():
            return pet_cell(pid, stage, "sit_front")[2]
    return None


def house_with_pet(kind, stage, pet=None):
    """집 + 문간에 앉은 동물 (앱과 같은 자리: 문 가운데, 발은 문 바닥). pet이 없으면 그 집의 첫 주민."""
    out = UI / "house_pixel"
    img = Image.open(out / f"{kind}_{stage}.png").convert("RGBA")
    d = json.loads((out / f"{kind}_{stage}.json").read_text(encoding="utf-8"))["door"]
    pet = pet or door_pet(kind, stage)
    if pet:
        n = pet.width
        img.alpha_composite(pet, (d["x"] + (d["w"] - n) // 2, d["y"] + d["h"] - n))
    img.alpha_composite(Image.open(out / f"{kind}_{stage}_front.png").convert("RGBA"))
    return img


def grid_preview(items, cols, cell, path, bgs=(LIGHT, DARK), pad=8):
    rows = math.ceil(len(items) / cols)
    W, H = cols * (cell[0] + pad) + pad, rows * (cell[1] + pad) + pad
    sheet = Image.new("RGBA", (W * len(bgs), H), (0, 0, 0, 0))
    for b, bg in enumerate(bgs):
        sheet.paste(Image.new("RGBA", (W, H), bg), (b * W, 0))
        for i, im in enumerate(items):
            x = b * W + pad + (i % cols) * (cell[0] + pad) + (cell[0] - im.width) // 2
            y = pad + (i // cols) * (cell[1] + pad) + cell[1] - im.height
            sheet.alpha_composite(im, (x, y))
    PREVIEW.mkdir(parents=True, exist_ok=True)
    sheet.save(path, optimize=True)


def build_previews():
    tiles = [scaled(house_with_pet(k, s), STAGE_GRID[s][1]) for k in KINDS for s in STAGES]
    grid_preview(tiles, 5, (216, 216), PREVIEW / "pixel_world_houses.png")
    gifts = sorted((UI / "gifts_pixel").glob("*.png"))
    plain = [p for p in gifts if not p.stem.endswith("_shiny")]
    grid_preview([scaled(Image.open(p).convert("RGBA"), 4) for p in plain], 12, (48, 48), PREVIEW / "pixel_world_gifts.png", pad=6)
    grid_preview([scaled(Image.open(p).convert("RGBA"), 8) for p in plain], 12, (96, 96), PREVIEW / "pixel_world_gifts_big.png", bgs=(LIGHT,), pad=6)
    shiny_list = [p for p in gifts if p.stem.endswith("_shiny")]
    grid_preview([scaled(Image.open(p).convert("RGBA"), 4) for p in shiny_list[:24]], 12, (48, 48), PREVIEW / "pixel_world_shiny.png", pad=6)
    e = UI / "effects_pixel"
    props = [scaled(Image.open(e / f"{n}.png").convert("RGBA"), 4) for n in
             ("bowl_food", "bowl_water", "cookie", "snowball_0", "snowball_1", "snowball_2", "snowball", "snowman", "snow_cloud")]
    grid_preview(props, 6, (128, 128), PREVIEW / "pixel_world_props.png")
    hand = sorted(e.glob("hand_*.png"))
    grid_preview([scaled(Image.open(p).convert("RGBA"), 8) for p in hand], 10, (96, 96), PREVIEW / "pixel_world_hand.png")
    objs = sorted((UI / "objects_pixel").glob("*.png"))
    grid_preview([scaled(Image.open(p).convert("RGBA"), 4) for p in objs], 9, (96, 96), PREVIEW / "pixel_world_objects.png")
    cap = Image.open(e / "nightcap.png").convert("RGBA")
    caps = [scaled(cap.crop((i * CAP_CELL[0], 0, (i + 1) * CAP_CELL[0], CAP_CELL[1])), 4) for i in (0, 4, 8, 12, 16, 20)]
    sc = Image.open(e / "scarf.png").convert("RGBA")
    scarves = [scaled(sc.crop((i * SCARF_CELL[0], j * SCARF_CELL[1], (i + 1) * SCARF_CELL[0], (j + 1) * SCARF_CELL[1])), 3)
               for j in range(3) for i in (4, 10, 16)]
    fx = [scaled(Image.open(p).convert("RGBA"), 8) for p in sorted(e.glob("fx_*.png"))]
    smoke = scaled(Image.open(UI / "house_pixel" / "xmas_smoke.png").convert("RGBA"), 6)
    grid_preview(caps + scarves + fx + [smoke], 7, (192, 144), PREVIEW / "pixel_world_overlays.png")
    weather_preview()
    doors_preview()


def worn(pid, stage, anim):
    """시트 칸 + 모자(또는 산타 목도리)를 앱과 같은 자리에 (칸 단위)."""
    m, n, frame = pet_cell(pid, stage, anim)
    pad = 12
    img = Image.new("RGBA", (n + 2 * pad, n + 2 * pad))
    img.alpha_composite(frame, (pad, pad))
    if "neck" in m:
        x, y, wd, _ = m["neck"][stage][anim][0]
        col = min(max(round(wd), SCARF_WIDTHS[0]), SCARF_WIDTHS[-1]) - SCARF_WIDTHS[0]
        v = 0 if anim in ("sit_front", "eat", "wave") else 1
        sheet = Image.open(UI / "effects_pixel" / "scarf.png").convert("RGBA")
        cell = sheet.crop((col * SCARF_CELL[0], v * SCARF_CELL[1], (col + 1) * SCARF_CELL[0], (v + 1) * SCARF_CELL[1]))
        at = (round(x) - SCARF_ANCHOR[0] + pad, round(y) - SCARF_ANCHOR[1] + pad)
    else:
        x, y, wd, _ = m["head"][stage][anim][0]
        col = min(max(round(wd), CAP_WIDTHS[0]), CAP_WIDTHS[-1]) - CAP_WIDTHS[0]
        sheet = Image.open(UI / "effects_pixel" / "nightcap.png").convert("RGBA")
        cell = sheet.crop((col * CAP_CELL[0], 0, (col + 1) * CAP_CELL[0], CAP_CELL[1]))
        at = (round(x) - CAP_ANCHOR[0] + pad, round(y) - CAP_ANCHOR[1] + pad)
    layer = Image.new("RGBA", img.size)
    layer.paste(cell, at)
    img.alpha_composite(layer)
    return img


def doors_preview():
    """모든 입주자 × 작은·큰 단계 (앉은 모습), 모자·목도리를 쓴 모습."""
    tiles = [scaled(house_with_pet(k, s, pet_cell(pid, s, "sit_front")[2]), STAGE_GRID[s][1]) for k in KINDS for pid in RESIDENTS[k] for s in ("baby", "teenager")]
    grid_preview(tiles, 8, (192, 192), PREVIEW / "pixel_world_doors.png")
    wear = [scaled(worn(pid, s, a), STAGE_GRID[s][1]) for pid in ("cat", "frog", "rabbit", "giraffe", "santa") for s in ("baby", "teenager") for a in ("idle",)]
    grid_preview(wear, 5, (224, 224), PREVIEW / "pixel_world_wear.png")


def weather_preview():
    """ui/weather.js의 칸 표를 그대로 읽어 실제 크기(칸 4 px)로 — 눈송이·빗방울·꽃잎·낙엽 + 계단 눈 더미."""
    import ast
    import re
    js = (UI / "weather.js").read_text(encoding="utf-8")
    table = lambda name: ast.literal_eval(re.search(rf"const {name} = (\[.*?\]);", js).group(1))  # noqa: E731
    cols = {k: tuple(int(v[i:i + 2], 16) for i in (1, 3, 5)) for k, v in re.findall(r'(\w): "(#[0-9a-f]{6})"', js)}
    cols.update(L=cols["o"], F=cols["O"])
    w = Dots(60, 26)
    x = 1
    for rows in table("PX_FLAKES") + [table("PX_DROP"), table("PX_SPLASH"), table("PX_PETAL"), table("PX_LEAF")]:
        w.stamp(rows, x, 2, cols)
        x += len(rows[0]) + 2
    # 더미: 칸 기둥
    heights = [1, 1, 2, 2, 3, 4, 4, 5, 5, 6, 6, 6, 5, 5, 4, 3, 3, 2, 2, 2, 3, 4, 5, 5, 6, 7, 7, 6, 5, 4, 3, 2, 1, 1]
    for i, h in enumerate(heights):
        w.rect(i + 2, 25 - h + 1, i + 2, 25, cols["w"])
        w.px(i + 2, 25 - h + 1, cols["c"])
    grid_preview([scaled(w.image(), 4)], 1, (240, 104), PREVIEW / "pixel_world_weather.png")


def main():
    build_houses()
    build_gifts()
    build_effects()
    build_objects()
    build_overlays()
    build_previews()
    size = sum(p.stat().st_size for d in ("house_pixel", "effects_pixel", "gifts_pixel", "objects_pixel") for p in (UI / d).glob("*"))
    print(f"pixel world: {size / 1024:.0f} KB")


def round_digest():
    """동글 그림 해시 (byte-identical 확인, tools/round_art_hashes.json)."""
    out = {}
    for d in ("effects", "gifts", "objects", "house", "pets"):
        for p in sorted((UI / d).rglob("*")):
            if p.is_file():
                out[p.relative_to(UI).as_posix()] = hashlib.sha256(p.read_bytes()).hexdigest()[:16]
    return out


if __name__ == "__main__":
    if "--round-digest" in sys.argv:
        (ROOT / "tools" / "round_art_hashes.json").write_text(json.dumps(round_digest(), indent=0, sort_keys=True) + "\n", encoding="utf-8")
    else:
        main()
