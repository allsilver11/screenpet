"""선물 아이콘 — draw_pets.py 엔진(슈퍼샘플 마스크·melt·외곽선·paint)과 draw_house.py의 말랑한 외곽선으로 그린 작은 그림.

    python tools/draw_gifts.py  → ui/gifts/<id>.png, ui/gifts/<id>_shiny.png (40×40, 아래 가운데에 앉는다)
                                  + tools/preview/gifts.png (동물별 한 줄: 보통 6개 + 반짝이 6개, 밝은 바탕·어두운 바탕)

id 목록은 crates/screenpet-core/src/gift.rs의 CATALOG와 같아야 한다 (양쪽 테스트가 확인한다).
앱(src-tauri/src/gifts.rs)은 바닥에 40×40(논리 px) 창을 띄우고 ui/gift.html이 이 그림을 보인다. 도감(ui/book.html)도 같은 그림을 쓴다.
반짝이는 보통 그림을 금빛으로 물들이고 별을 찍어 만든다 (다시 그리지 않는다).
"""

import math
import sys
from pathlib import Path

from PIL import Image, ImageDraw

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish  # noqa: E402
from draw_pets import LINE, S, WHITE, C, E, L, T, aa, mix, paint, shapes, view  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "gifts"
PREVIEW = ROOT / "tools" / "preview" / "gifts.png"

ICON = 40
SHINY = "_shiny"
LINES = {
    "cat": ("fishbone", "yarn", "feather", "mouse", "bell", "catnip"),
    "squirrel": ("acorn", "pinecone", "walnut", "ginkgo", "fluff", "mushroom"),
    "rabbit": ("carrot", "clover", "lettuce", "radish", "daisy", "strawberry"),
    "frog": ("lilypad", "dragonfly", "fly", "lotus", "tadpole", "pebble"),
    "pumpkin": ("candycorn", "lantern", "seed", "vine", "lollipop", "hat"),
    "ghost": ("candle", "key", "web", "bat", "tombstone", "moon"),
    "duck": ("bread", "pebble", "rubber", "feather", "shell", "reed"),
    "bird": ("gourdseed", "feather", "twig", "berry", "egg", "dandelion"),
    "penguin": ("pebble", "ice", "fish", "snowflake", "down", "shell"),
    "redpanda": ("bamboo", "berry", "stick", "maple", "fluff", "flower"),
    "giraffe": ("acacia", "pod", "mango", "cloud", "nest", "scarf"),
    "rudolph": ("bell", "bulb", "snowflake", "star", "moss", "ribbon"),
    "santa": ("giftbox", "candycane", "mitten", "cookie", "stocking", "coal"),
    "snowman": ("icicle", "carrot", "twig", "globe", "bucket", "snowball"),
    "dino": ("fossil", "eggshell", "fern", "amber", "tooth", "footprint"),
}
IDS = [f"{key}_{name}" for key, names in LINES.items() for name in names]

# ---- 색 ----
GREEN = (146, 204, 120)
GREEN_DARK = (104, 168, 92)
GREEN_LIGHT = mix(GREEN, WHITE, 0.45)
RED = (232, 92, 88)
RED_DARK = (188, 62, 66)
RED_LIGHT = mix(RED, WHITE, 0.45)
PINK = (246, 160, 176)
PINK_LIGHT = mix(PINK, WHITE, 0.5)
ORANGE = (246, 156, 72)
ORANGE_DARK = (214, 118, 48)
GOLD = (250, 204, 84)
GOLD_DARK = (214, 160, 54)
GOLD_LIGHT = mix(GOLD, WHITE, 0.55)
BROWN = (170, 118, 78)
BROWN_DARK = (128, 84, 58)
BROWN_LIGHT = mix(BROWN, WHITE, 0.35)
TAN = (226, 192, 142)
BONE = (248, 240, 222)
CREAM = (250, 236, 200)
GRAY = (176, 176, 186)
GRAY_DARK = (128, 128, 142)
GRAY_LIGHT = mix(GRAY, WHITE, 0.45)
BLUE = (138, 190, 236)
BLUE_LIGHT = mix(BLUE, WHITE, 0.5)
PURPLE = (150, 112, 196)
PURPLE_DARK = (104, 76, 152)
INK = (70, 54, 56)


def bl(img, fns, color, r=0.7, line=0.7, marks=()):
    return blob(img, fns, color, r=r, line=line, marks=marks)


def dots(img, pts, color, r=1.0, clip=None):
    paint(img, aa(shapes([C(x, y, r) for x, y in pts])), color, clip=clip)


def strokes(img, lines, color, w=1.0, clip=None):
    paint(img, aa(shapes([L(p, w) for p in lines])), color, clip=clip)


def ring(cx, cy, dist, r, n, start=-90.0):
    return [C(cx + dist * math.cos(math.radians(start + 360 / n * i)), cy + dist * math.sin(math.radians(start + 360 / n * i)), r) for i in range(n)]


def polar(cx, cy, r, deg):
    return (cx + r * math.cos(math.radians(deg)), cy + r * math.sin(math.radians(deg)))


def star_pts(cx, cy, r, inner=0.45, n=5, start=-90.0):
    return [polar(cx, cy, r if i % 2 == 0 else r * inner, start + 180 / n * i) for i in range(2 * n)]


def hexagon(cx, cy, r):
    return T(*[polar(cx, cy, r, 60 * i) for i in range(6)])


def curve(p0, p1, p2, n=12):
    """2차 베지어 점들."""
    out = []
    for i in range(n + 1):
        t = i / n
        a, b, c = (1 - t) ** 2, 2 * t * (1 - t), t**2
        out.append((a * p0[0] + b * p1[0] + c * p2[0], a * p0[1] + b * p1[1] + c * p2[1]))
    return out


def tiny_face(img, cx, cy, s=1.0):
    dots(img, [(cx - 3.4 * s, cy), (cx + 3.4 * s, cy)], INK, 1.05 * s)
    strokes(img, [curve((cx - 1.2 * s, cy + 1.2 * s), (cx, cy + 2.4 * s), (cx + 1.2 * s, cy + 1.2 * s), 6)], INK, 0.65 * s)


def leaf(img, a, b, w=5.0, color=GREEN):
    bl(img, [L([a, b], w)], color, r=0.3, line=0.55, marks=[([L([a, b], 0.7)], mix(color, LINE, 0.3))])


def feather(img, base, tip, color, light):
    (x0, y0), (x1, y1) = base, tip
    mx, my = x0 + (x1 - x0) * 0.58, y0 + (y1 - y0) * 0.58
    bx, by = x0 + (x1 - x0) * 0.22, y0 + (y1 - y0) * 0.22
    bl(img, [L([(bx, by), (mx, my)], 12), L([(mx, my), (x1, y1)], 7)], color, r=1.2, marks=[([L([(bx - 3, by + 1), (mx - 3, my + 1)], 3)], light)])
    dark = mix(color, LINE, 0.55)
    strokes(img, [[(x0, y0), (x1, y1)]], dark, 1.0)
    # 깃가지 몇 가닥
    ang = math.degrees(math.atan2(y1 - y0, x1 - x0))
    barbs = []
    for t in (0.35, 0.5, 0.65, 0.8):
        p = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
        barbs += [[p, polar(p[0], p[1], 5.0, ang + 45)], [p, polar(p[0], p[1], 5.0, ang - 45)]]
    strokes(img, barbs, mix(color, LINE, 0.28), 0.7)


# ---- 고양이 ----
def cat_fishbone(i):
    bl(i, [T((46, 57), (55, 64), (46, 71))], BONE, r=0.4)
    bl(i, [L([(50, 64), (72, 64)], 2.8)] + [L([(x, 56 + (x - 54) * 0.2), (x, 72 - (x - 54) * 0.2)], 2.3) for x in (55, 61, 67)], BONE, r=0.2)
    bl(i, [E(68, 55, 83, 73)], BONE, r=0.3, marks=[([E(70, 60, 76, 70)], mix(BONE, LINE, 0.15))])
    dots(i, [(76.5, 62)], INK, 1.4)


def cat_yarn(i):
    bl(i, [L(curve((64, 74), (56, 84), (47, 80)), 2.0)], PINK, r=0)
    m = bl(i, [C(65, 65, 15)], PINK, r=0)
    dark = mix(PINK, LINE, 0.35)
    strokes(i, [curve((52, 58), (66, 62), (74, 80)), curve((51, 65), (64, 68), (68, 80)), curve((57, 51), (70, 55), (79, 72)),
                curve((70, 51), (60, 62), (52, 72)), curve((76, 56), (66, 68), (58, 78))], dark, 1.1, clip=m)


def cat_feather(i):
    feather(i, (53, 83), (76, 47), BLUE, BLUE_LIGHT)


def cat_mouse(i):
    bl(i, [L(curve((48, 76), (44, 62), (52, 58)), 2.0)], PINK, r=0)
    bl(i, [C(68, 59, 5.2)], GRAY, r=0, marks=[([C(68, 59, 2.8)], PINK_LIGHT)])
    bl(i, [E(48, 58, 82, 80), T((68, 62), (84, 74), (70, 80))], GRAY, r=1.4, marks=[([E(54, 72, 76, 84)], GRAY_LIGHT)])
    bl(i, [C(75, 62, 4.4)], GRAY, r=0, line=0.5, marks=[([C(75, 62, 2.3)], PINK_LIGHT)])
    dots(i, [(76, 70)], INK, 1.3)
    dots(i, [(83, 75)], RED, 1.5)


def cat_bell(i):
    bl(i, [L(curve((46, 50), (64, 66), (82, 50)), 5.0)], RED, r=0, marks=[([L(curve((46, 50), (64, 64), (82, 50)), 1.0)], RED_LIGHT)])
    bl(i, [C(64, 70, 11.5)], GOLD, r=0, marks=[([E(55, 60, 66, 68)], GOLD_LIGHT), ([E(54, 74, 74, 84)], GOLD_DARK)])
    strokes(i, [[(54, 70), (74, 70)]], GOLD_DARK, 1.1)
    bl(i, [C(64, 74.5, 2.2), L([(64, 75), (64, 80)], 1.6)], INK, r=0, line=0)


def cat_catnip(i):
    bl(i, [L(curve((64, 84), (62, 68), (65, 52)), 2.2)], GREEN_DARK, r=0)
    for a, b, w in (((63, 78), (49, 70), 7.5), ((63, 77), (79, 69), 7.5), ((63, 66), (51, 59), 6.5), ((64, 65), (77, 58), 6.5)):
        leaf(i, a, b, w)
    bl(i, ring(65, 50, 3.0, 2.6, 5), (186, 150, 226), r=0.2, line=0.55)


# ---- 강아지 ----
def rabbit_carrot(i):
    for b in ((58, 47), (64, 44.5), (70, 47)):
        bl(i, [L([(64, 56), b], 3.2)], GREEN, r=0.3, line=0.6)
    m = bl(i, [T((53, 58), (75, 58), (65, 85)), E(53, 53, 75, 63)], ORANGE, r=1.4, marks=[([E(56, 55, 62, 60)], mix(ORANGE, WHITE, 0.45))])
    strokes(i, [[(57, 66), (62, 66)], [(66, 72), (71, 72)], [(59, 78), (63, 78)]], ORANGE_DARK, 1.1, clip=m)


def rabbit_clover(i):
    bl(i, [L(curve((64, 66), (63, 76), (70, 85)), 2.2)], GREEN_DARK, r=0)
    for dx, dy in ((-1, -1), (1, -1), (-1, 1), (1, 1)):
        cx, cy = 64 + 8.6 * dx, 62 + 8.6 * dy
        bl(i, [C(cx + 3.3 * dx, cy - 2.5 * dx * dy, 5.8), C(cx - 2.5 * dx * dy, cy + 3.3 * dy, 5.8), C(cx - 2 * dx, cy - 2 * dy, 3.8)], GREEN, r=0.3, line=0.6,
           marks=[([C(cx, cy, 2.2)], GREEN_LIGHT)])


def rabbit_lettuce(i):
    bl(i, ring(64, 66, 12, 7.5, 9) + [C(64, 66, 12)], GREEN, r=0.7, marks=[([C(64, 66, 10)], GREEN_LIGHT)])
    strokes(i, [[(64, 80), (64, 58)], [(64, 72), (55, 62)], [(64, 72), (73, 62)], [(64, 66), (59, 58)], [(64, 66), (69, 58)]], GREEN_DARK, 1.0)


def rabbit_radish(i):
    for b in ((56, 47), (64, 45), (72, 47)):
        leaf(i, (64, 60), b, 5.5)
    m = bl(i, [E(52, 57, 76, 78), T((55, 74), (73, 74), (64, 87))], WHITE, r=1.0, marks=[([E(50, 54, 78, 66)], (240, 120, 160))])
    strokes(i, [[(57, 73), (62, 73)], [(66, 78), (70, 78)]], GRAY_LIGHT, 0.9, clip=m)


def rabbit_daisy(i):
    bl(i, [L(curve((64, 72), (62, 78), (64, 86)), 2.2)], GREEN_DARK, r=0)
    leaf(i, (64, 81), (75, 76), 5)
    bl(i, ring(64, 61, 10.5, 4.6, 9), WHITE, r=0.1, line=0.6)
    bl(i, [C(64, 61, 6.2)], GOLD, r=0, line=0.5, marks=[([C(62.4, 59.4, 2)], GOLD_LIGHT)])


def rabbit_strawberry(i):
    m = bl(i, [E(50, 54, 78, 72), T((51, 64), (77, 64), (64, 85))], RED, r=1.8, marks=[([E(53, 56, 60, 62)], RED_LIGHT)])
    dots(i, [(57, 66), (64, 63), (71, 66), (60, 73), (68, 73), (64, 79)], CREAM, 1.0, clip=m)
    bl(i, [T(*star_pts(64, 53, 10, 0.4, 5, -90))], GREEN, r=0.2, line=0.6)


# ---- 곰 ----
def frog_lilypad(i):
    pad = T(*[polar(64, 70, 18 if abs(((a + 90) % 360) - 0) > 22 and abs(((a + 90) % 360) - 360) > 22 else 3, a) for a in range(0, 360, 12)])
    m = bl(i, [pad], GREEN, r=0.4, marks=[([E(44, 76, 84, 90)], GREEN_DARK)])
    strokes(i, [[(64, 70), polar(64, 70, 16, a)] for a in (-30, 10, 50, 90, 130, 170, 210)], GREEN_LIGHT, 0.9, clip=m)
    bl(i, ring(64, 52, 3.4, 3, 5), PINK, r=0.1, line=0.5)
    dots(i, [(64, 52)], GOLD, 1.5)


def frog_dragonfly(i):
    wing = (206, 232, 248)
    for x1, y1 in ((46, 53), (82, 53), (46, 67), (82, 67)):
        bl(i, [L([(64, 61), (x1, y1)], 6.0)], wing, r=0.2, line=0.5, marks=[([L([(64, 60), (x1, y1 - 1)], 1.0)], WHITE)])
    m = bl(i, [L([(64, 58), (64, 86)], 4.0)], (74, 168, 186), r=0.3, line=0.6)
    strokes(i, [[(60, 70), (68, 70)], [(60, 75), (68, 75)], [(60, 80), (68, 80)]], mix((74, 168, 186), LINE, 0.5), 1.0, clip=m)
    bl(i, [C(64, 54, 5.2)], (74, 168, 186), r=0, line=0.6)
    dots(i, [(61.4, 53), (66.6, 53)], INK, 1.5)
    dots(i, [(61, 52.4), (66.2, 52.4)], WHITE, 0.5)


def frog_fly(i):
    wing = (214, 236, 250)
    for sgn in (-1, 1):
        bl(i, [L([(64 + 5 * sgn, 62), (64 + 15 * sgn, 53)], 9)], wing, r=0.3, line=0.55, marks=[([L([(64 + 12 * sgn, 53), (64 + 15 * sgn, 52)], 2)], WHITE)])
    bl(i, [E(52, 58, 76, 84)], (90, 84, 102), r=0.2, marks=[([E(55, 74, 73, 84)], (116, 110, 128))])
    for sgn in (-1, 1):
        bl(i, [C(64 + 5.4 * sgn, 65, 5)], WHITE, r=0, line=0.4)
        dots(i, [(64 + 5.8 * sgn, 65.4)], INK, 2.6)
        dots(i, [(64 + 5 * sgn, 64.4)], WHITE, 0.8)
    strokes(i, [curve((61, 75), (64, 78), (67, 75), 6)], WHITE, 0.9)


def frog_lotus(i):
    def petal(a, length, w, color):
        tip = polar(64, 78, length, a)
        l = polar(64, 78, length * 0.45, a - w)
        r = polar(64, 78, length * 0.45, a + w)
        bl(i, [T((64, 80), l, tip, r)], color, r=0.3, line=0.55)

    leaf(i, (47, 83), (81, 83), 6)
    for a in (-160, -20, -135, -45):
        petal(a, 22, 24, mix(PINK, WHITE, 0.15))
    for a in (-112, -68):
        petal(a, 27, 18, PINK)
    petal(-90, 30, 16, PINK_LIGHT)
    dots(i, [(64, 74)], GOLD, 2.4)


def frog_tadpole(i):
    dark = (86, 92, 112)
    bl(i, [L(curve((64, 66), (76, 56), (82, 66)) + curve((82, 66), (84, 72), (80, 76)), 2.6)], dark, r=0.1, line=0.5)
    bl(i, [E(45, 58, 71, 82)], dark, r=0.2, marks=[([E(48, 72, 68, 86)], mix(dark, WHITE, 0.3))])
    dots(i, [(53, 66)], WHITE, 3.0)
    dots(i, [(52.6, 66.6)], INK, 1.8)
    strokes(i, [curve((49, 74), (52, 76), (55, 74), 6)], WHITE, 0.9)


def frog_pebble(i):
    moss = bl(i, [E(47, 58, 81, 84)], GRAY, r=0.5, marks=[([E(52, 76, 78, 88)], GRAY_DARK), ([E(56, 62, 66, 66)], GRAY_LIGHT)])
    paint(i, aa(shapes([E(46, 54, 66, 68), C(70, 58, 8), C(58, 70, 5.5), C(78, 64, 4)])), GREEN, clip=moss)
    dots(i, [(54, 61), (63, 63), (70, 58), (58, 69)], GREEN_LIGHT, 1.3, clip=moss)


# ---- 호박 ----
def pumpkin_candycorn(i):
    m = bl(i, [T((64, 47), (82, 83), (46, 83))], WHITE, r=2.8)
    paint(i, aa(shapes([T((40, 73), (88, 73), (88, 92), (40, 92))])), GOLD, clip=m)
    paint(i, aa(shapes([T((40, 61), (88, 61), (88, 73), (40, 73))])), ORANGE, clip=m)
    strokes(i, [[(58, 53), (54.5, 60)]], WHITE, 1.4, clip=m)


def pumpkin_lantern(i):
    strokes(i, [curve((55, 56), (64, 40), (73, 56))], INK, 1.3)
    bl(i, [E(47, 54, 81, 84), E(55, 53, 73, 85)], ORANGE, r=0.6, marks=[([E(49, 76, 79, 88)], ORANGE_DARK)])
    strokes(i, [curve((57, 54), (50, 69), (57, 84)), curve((71, 54), (78, 69), (71, 84))], ORANGE_DARK, 1.0)
    bl(i, [E(58, 50, 70, 57)], (98, 84, 92), r=0, line=0.5)
    glow = (255, 232, 130)
    bl(i, [T((56, 64), (61, 64), (58.5, 59)), T((67, 64), (72, 64), (69.5, 59))], glow, r=0, line=0)
    bl(i, [T((55, 71), (73, 71), (70, 77), (58, 77))], glow, r=0, line=0)


def pumpkin_seed(i):
    for (x, y, a) in ((56, 67, -14), (72, 71, 16)):
        tip = polar(x, y, 16, -90 + a)
        bot = polar(x, y, 13, 90 + a)
        bl(i, [L([bot, polar(x, y, 2, -90 + a)], 14), T(polar(x, y, 7, a), polar(x, y, 7, 180 + a), tip)], CREAM, r=1.1, marks=[([L([polar(x - 3, y, 2, -90 + a), polar(x - 3, y, 9, 90 + a)], 3)], WHITE)])
        strokes(i, [[polar(x, y, 8, -90 + a), polar(x, y, 13, -90 + a)]], TAN, 0.9)


def pumpkin_vine(i):
    pts = curve((46, 82), (58, 80), (65, 66)) + [polar(72, 60, 9 - t * 0.8, 150 + t * 42) for t in range(9)]
    bl(i, [L(pts, 2.6)], GREEN_DARK, r=0.1, line=0.55)
    leaf(i, (54, 79), (50, 62), 8)
    leaf(i, (62, 72), (79, 83), 7)


def pumpkin_lollipop(i):
    bl(i, [L([(64, 66), (64, 87)], 3.0)], BONE, r=0)
    bl(i, [C(64, 60, 14)], ORANGE, r=0)
    swirl = [polar(64, 60, 1 + t * 0.48, -90 + t * 28) for t in range(26)]
    strokes(i, [swirl], (156, 108, 206), 2.6)
    dots(i, [(58, 53)], WHITE, 1.7)


def pumpkin_hat(i):
    hat = PURPLE
    bl(i, [T((62, 44), (80, 80), (48, 80)), L([(62, 45), (70, 44)], 3)], hat, r=1.0, marks=[([T((62, 44), (66, 44), (58, 80), (52, 80))], mix(hat, WHITE, 0.25))])
    bl(i, [E(44, 73, 84, 86)], PURPLE_DARK, r=0)
    bl(i, [T((50.5, 67), (75.5, 67), (79, 76), (47, 76))], GOLD, r=0.2, line=0.5)
    bl(i, [T((60, 68), (68, 68), (68, 75.5), (60, 75.5))], GOLD_DARK, r=0, line=0.5, marks=[([T((62, 70), (66, 70), (66, 73.5), (62, 73.5))], hat)])


# ---- 유령 ----
def ghost_candle(i):
    bl(i, [E(46, 77, 82, 87)], GOLD_DARK, r=0)
    bl(i, [T((56, 56), (72, 56), (72, 82), (56, 82))], BONE, r=0.7, marks=[([T((67, 56), (72, 56), (72, 82), (67, 82))], mix(BONE, LINE, 0.15))])
    bl(i, [E(55, 53, 73, 60), L([(59, 58), (59, 67)], 3.4)], WHITE, r=0.4, line=0.5)
    bl(i, [C(64, 48.5, 4.4), T((60, 47), (68, 47), (64, 39.5))], (255, 190, 90), r=0.2, line=0.5, marks=[([C(64, 49.5, 2)], (255, 244, 180))])


def ghost_key(i):
    brass = (216, 176, 96)
    bl(i, [L([(58, 62), (80, 84)], 4.2), L([(73, 76), (68, 82)], 3.6), L([(78, 81), (73, 87)], 3.6)], brass, r=0.1)
    bl(i, [C(54, 57, 9.2), C(54, 57, 9.2)], brass, r=0, marks=[([E(48, 50, 56, 54)], mix(brass, WHITE, 0.5))])
    bl(i, [C(54, 57, 4.2)], (84, 72, 88), r=0, line=0.5)


def ghost_web(i):
    silver = (214, 218, 232)
    cx, cy, n = 62, 64, 8
    spokes = [polar(cx, cy, 20, 22.5 + 45 * k) for k in range(n)]
    web = [[(cx, cy), p] for p in spokes]
    for r in (6.5, 12, 18):
        web += [curve(polar(cx, cy, r, 22.5 + 45 * k), polar(cx, cy, r * 0.8, 45 + 45 * k), polar(cx, cy, r, 67.5 + 45 * k), 6) for k in range(n)]
    bl(i, [L(p, 1.4) for p in web], silver, r=0, line=0.4)
    bl(i, [C(74, 76, 4.2), C(74, 71, 2.8)], INK, r=0, line=0)
    dots(i, [(73, 70.2), (75.2, 70.2)], WHITE, 0.6)
    strokes(i, [[(74, 76), (68, 79)], [(74, 76), (80, 79)], [(74, 75), (68.5, 73)], [(74, 75), (79.5, 73)]], INK, 0.8)


def ghost_bat(i):
    wing = PURPLE_DARK
    for sgn in (-1, 1):
        x = lambda d: 64 + d * sgn  # noqa: E731
        bl(i, [T((x(4), 60), (x(22), 54), (x(19), 70), (x(14), 66), (x(11), 74), (x(6), 70), (x(4), 76))], wing, r=0.3, line=0.65)
    bl(i, [T((54, 56), (56, 46), (61, 54)), T((74, 56), (72, 46), (67, 54))], wing, r=0.2, line=0.6)
    bl(i, [E(53, 52, 75, 78)], wing, r=0.2, marks=[([E(57, 66, 71, 80)], PURPLE)])
    dots(i, [(59.5, 60), (68.5, 60)], WHITE, 2.4)
    dots(i, [(59.5, 60.5), (68.5, 60.5)], INK, 1.4)
    bl(i, [T((60, 66), (63, 66), (61.5, 69.5)), T((65, 66), (68, 66), (66.5, 69.5))], WHITE, r=0, line=0)


def ghost_tombstone(i):
    bl(i, [T((51, 60), (77, 60), (77, 82), (51, 82)), E(51, 46, 77, 72)], GRAY, r=0.6, marks=[([T((72, 46), (78, 46), (78, 82), (72, 82))], GRAY_DARK), ([E(55, 50, 62, 55)], GRAY_LIGHT)])
    strokes(i, [[(64, 55), (64, 73)], [(58, 61), (70, 61)]], GRAY_DARK, 2.4)
    bl(i, [E(45, 79, 83, 88)] + [L([(x, 84), (x + dx, 75)], 2.8) for x, dx in ((48, -1), (53, 1), (75, -1), (80, 1))], GREEN, r=0.3, line=0.6)


def ghost_moon(i):
    outer = [polar(62, 64, 18, a) for a in range(60, 301, 12)]
    inner = [polar(72, 62, 13.5, a) for a in range(250, 90, -12)]
    bl(i, [T(*(outer + inner))], GOLD, r=0.6, marks=[([E(44, 66, 60, 86)], GOLD_DARK), ([E(49, 53, 54, 62)], GOLD_LIGHT)])
    bl(i, [T(*star_pts(76, 64, 6.6))], GOLD_LIGHT, r=0.1, line=0.5)
    dots(i, [(51, 66)], INK, 1.1)
    strokes(i, [curve((50, 72), (53, 74), (56, 72.5), 6)], INK, 0.8)


# ---- 오리 ----
def duck_bread(i):
    crust = (214, 158, 92)
    bl(i, [T((52, 62), (82, 62), (82, 84), (52, 84)), C(58, 58, 9), C(76, 58, 9)], crust, r=1.2)
    bl(i, [T((55, 63), (79, 63), (79, 81), (55, 81)), C(60, 60, 6.5), C(74, 60, 6.5)], CREAM, r=1.0, line=0)
    dots(i, [(60, 68), (68, 73), (73, 65), (63, 76)], mix(CREAM, LINE, 0.25), 0.9)
    bl(i, [C(47, 81, 2.6), C(45, 74, 1.7)], crust, r=0, line=0.5)


def duck_pebble(i):
    bl(i, [E(47, 68, 81, 86)], GRAY, r=0.2, marks=[([E(52, 80, 76, 90)], GRAY_DARK), ([E(53, 71, 62, 75)], GRAY_LIGHT)])
    warm = (206, 186, 168)
    bl(i, [E(52, 53, 74, 71)], warm, r=0.2, marks=[([E(55, 56, 62, 60)], mix(warm, WHITE, 0.6))])
    dots(i, [(68, 62), (60, 66)], mix(warm, LINE, 0.35), 0.8)


def duck_rubber(i):
    yellow = (252, 216, 92)
    bl(i, [E(47, 64, 83, 86), T((78, 72), (87, 62), (82, 80))], yellow, r=1.0, marks=[([E(52, 80, 78, 90)], mix(yellow, ORANGE, 0.4))])
    bl(i, [C(58, 56, 11)], yellow, r=0, marks=[([E(50, 49, 57, 53)], WHITE)])
    bl(i, [E(42, 56, 52, 63)], ORANGE, r=0, line=0.6)
    dots(i, [(55, 54)], INK, 1.5)
    bl(i, [E(64, 69, 78, 78)], mix(yellow, ORANGE, 0.3), r=0, line=0.4)


def duck_feather(i):
    feather(i, (76, 84), (53, 46), WHITE, (232, 232, 240))


def duck_shell(i):
    shell = (248, 190, 176)
    fan = [(64, 84)] + [polar(64, 84, 34, a) for a in range(-150, -29, 10)]
    m = bl(i, [T(*fan)], shell, r=1.4, marks=[([T((64, 84), (52, 88), (76, 88))], mix(shell, LINE, 0.2))])
    strokes(i, [[(64, 84), polar(64, 84, 34, a)] for a in (-135, -115, -90, -65, -45)], mix(shell, LINE, 0.3), 1.0, clip=m)
    bl(i, [E(58, 80, 70, 87)], mix(shell, WHITE, 0.3), r=0, line=0.5)


def duck_reed(i):
    bl(i, [L(curve((58, 86), (57, 70), (52, 58)), 2.0), L(curve((68, 86), (68, 72), (74, 54)), 2.0)], GREEN_DARK, r=0)
    leaf(i, (63, 86), (49, 72), 4.5)
    leaf(i, (63, 86), (82, 72), 4.5)
    for (a, b) in (((53, 62), (50.5, 50)), ((73, 58), (76, 45))):
        bl(i, [L([a, b], 7)], BROWN, r=0.1, line=0.6, marks=[([L([(a[0] - 2, a[1]), (b[0] - 2, b[1])], 1.4)], BROWN_LIGHT)])


# ---- 새 (제비) ----
SWALLOW_BLUE = (112, 138, 206)


def bird_gourdseed(i):
    """흥부네 제비가 물어다 준 박씨: 통통한 물방울 씨앗 + 가운데 줄 + 반짝."""
    seed = (236, 214, 168)
    m = bl(i, [E(51, 52, 77, 86), T((56, 58), (64, 44), (72, 58))], seed, r=1.2,
           marks=[([E(54, 76, 74, 90)], mix(seed, LINE, 0.18)), ([E(56, 55, 62, 63)], mix(seed, WHITE, 0.6))])
    strokes(i, [curve((64, 50), (62, 68), (64, 84))], mix(seed, LINE, 0.4), 1.1, clip=m)
    tiny_face(i, 64, 72, 0.9)


def bird_feather(i):
    feather(i, (76, 86), (52, 46), SWALLOW_BLUE, mix(SWALLOW_BLUE, WHITE, 0.45))


def bird_twig(i):
    """둥지 짓는 지푸라기 한 다발 (가운데를 풀잎으로 묶음)."""
    straw = (238, 204, 120)
    for a, b in (((46, 82), (80, 50)), ((50, 86), (84, 58)), ((44, 72), (76, 46)), ((54, 88), (82, 66))):
        bl(i, [L([a, b], 3.2)], straw, r=0, line=0.55, marks=[([L([a, b], 0.8)], mix(straw, LINE, 0.25))])
    bl(i, [L([(58, 74), (70, 64)], 4.5)], GREEN, r=0.2, line=0.55)


def bird_berry(i):
    """빨간 열매 두 알 + 잎 하나."""
    bl(i, [L(curve((64, 50), (62, 58), (56, 66)), 1.6), L(curve((64, 50), (68, 60), (73, 68)), 1.6)], BROWN, r=0)
    leaf(i, (64, 50), (78, 44), 5.0)
    for x, y, r in ((55, 74, 9), (73, 76, 8.5)):
        bl(i, [C(x, y, r)], RED, r=0, marks=[([C(x - 3, y - 3, 2.2)], RED_LIGHT), ([E(x - r, y + r * 0.4, x + r, y + r * 1.2)], RED_DARK)])


def bird_egg(i):
    """하늘색 작은 알 (얼룩 몇 점)."""
    egg = (176, 220, 240)
    bl(i, [E(51, 50, 77, 87)], egg, r=0.2,
       marks=[([E(53, 78, 75, 92)], mix(egg, LINE, 0.14)), ([E(55, 54, 62, 62)], WHITE)])
    dots(i, [(68, 60), (60, 70), (71, 74), (64, 80)], mix(egg, LINE, 0.35), 1.1)


def bird_dandelion(i):
    """바람 타는 민들레 홀씨 (동글 솜털 공 + 줄기)."""
    bl(i, [L(curve((64, 88), (62, 76), (64, 64)), 2.0)], GREEN_DARK, r=0)
    puff = (250, 250, 246)
    bl(i, ring(64, 58, 10, 4.2, 10) + [C(64, 58, 10)], puff, r=0.6, marks=[([C(64, 58, 4)], mix(puff, LINE, 0.12))])
    strokes(i, [[(64, 58), polar(64, 58, 12, a)] for a in range(-90, 270, 36)], mix(puff, LINE, 0.22), 0.6)
    dots(i, [(64, 58)], GOLD_DARK, 1.8)


# ---- 다람쥐 ----
SQ_NUT, SQ_CAP = (214, 150, 86), (150, 100, 62)


def squirrel_acorn(i):
    """반질반질 도토리 한 알 (깍정이 모자 + 비늘 무늬 + 꼭지, 작은 얼굴)."""
    bl(i, [E(51, 58, 77, 82), T((53, 72), (75, 72), (64, 88))], SQ_NUT, r=1.2,
       marks=[([E(55, 63, 60, 74)], mix(SQ_NUT, WHITE, 0.45)), ([E(50, 78, 78, 92)], mix(SQ_NUT, LINE, 0.18))])
    m = bl(i, [E(48, 50, 80, 67), L([(64, 52), (67, 43)], 3.0)], SQ_CAP, r=0.7)
    strokes(i, [[(50, 56), (78, 56)], [(50, 61), (78, 61)], [(56, 50), (60, 67)], [(64, 50), (64, 67)], [(72, 50), (68, 67)]],
            mix(SQ_CAP, LINE, 0.35), 0.9, clip=m)
    tiny_face(i, 64, 75, 0.85)


def squirrel_pinecone(i):
    """솔방울 (겹겹 비늘, 꼭대기에 솔잎 하나)."""
    bl(i, [E(51, 50, 77, 86)], BROWN, r=0.8)
    for row, (y, xs) in enumerate(((54, (64,)), (60, (58, 70)), (66, (53.5, 64, 74.5)), (72, (58, 70)), (78, (64,)))):
        for x in xs:
            bl(i, [E(x - 5.2, y - 3.5, x + 5.2, y + 4)], BROWN_LIGHT if row % 2 else mix(BROWN, TAN, 0.5), r=0, line=0.45)
    leaf(i, (64, 50), (71, 43), 3.6, GREEN_DARK)


def squirrel_walnut(i):
    """동글 호두 (가운데 이음 줄 + 꼬불꼬불 주름)."""
    nut = (206, 164, 112)
    m = bl(i, [E(46, 54, 82, 88)], nut, r=1.0, marks=[([E(48, 76, 80, 92)], mix(nut, LINE, 0.22)), ([E(52, 59, 60, 66)], mix(nut, WHITE, 0.45))])
    dark = mix(nut, LINE, 0.45)
    strokes(i, [[(64, 54), (64, 88)]], dark, 1.4, clip=m)
    strokes(i, [curve((52, 66), (57, 70), (53, 76), 8), curve((76, 66), (71, 71), (75, 78), 8), curve((58, 60), (60, 64), (58, 68), 6),
                curve((70, 60), (68, 65), (71, 69), 6), curve((57, 80), (60, 83), (58, 86), 6), curve((71, 80), (68, 83), (70, 86), 6)],
            dark, 0.9, clip=m)


def squirrel_ginkgo(i):
    """노란 은행잎 한 장 (부채꼴 + 가운데 홈 + 잎맥 + 잎자루)."""
    gold = (250, 210, 90)
    bl(i, [L(curve((64, 74), (63, 82), (66, 89), 6), 2.0)], (206, 160, 60), r=0, line=0.5)
    fan = T(*([(64, 76)] + [polar(64, 76, 24, a) for a in range(-160, -19, 10)]))
    m = bl(i, [fan, C(64, 74, 4)], gold, r=1.4, marks=[([E(44, 52, 84, 66)], mix(gold, WHITE, 0.35))])
    strokes(i, [[(64, 76), polar(64, 76, 22, a)] for a in range(-150, -29, 20)], mix(gold, LINE, 0.25), 0.7, clip=m)
    strokes(i, [[(64, 52), (64, 60)]], mix(gold, LINE, 0.45), 1.3)  # 가운데 갈라진 홈


def squirrel_fluff(i):
    """다람쥐 꼬리 솜털 한 다발 (위로 솟아 끝이 말린 복슬 깃털 꼬리 모양 + 밝은 털 줄 + 삐죽 털)."""
    fur = (204, 104, 60)
    pts = [(58, 88, 4), (55, 80, 6), (54, 71, 8), (56, 62, 9.5), (61, 54, 10), (68, 50, 9), (75, 51, 7), (78, 56, 5)]
    chain = []
    for (x0, y0, r0), (x1, y1, r1) in zip(pts, pts[1:]):
        chain += [(x0 + (x1 - x0) * t / 4, y0 + (y1 - y0) * t / 4, r0 + (r1 - r0) * t / 4) for t in range(4)]
    spikes = [T(polar(x, y, r - 1, a - 14), polar(x, y, r + 4, a), polar(x, y, r - 1, a + 14))
              for (x, y, r), a in zip(chain[4::3], (180, 190, 215, 245, 270, 290, 310))]
    m = bl(i, [C(x, y, r) for x, y, r in chain] + spikes, fur, r=0.6, marks=[([E(44, 70, 64, 92)], mix(fur, LINE, 0.15))])
    strokes(i, [[(x + r * 0.35, y + r * 0.1) for x, y, r in chain[3:-6]]], (240, 162, 108), 1.6, clip=m)


def squirrel_mushroom(i):
    """빨간 점박이 꼬마 버섯 (크림 기둥 + 흰 점 갓, 작은 얼굴)."""
    bl(i, [T((58, 66), (70, 66), (73, 86), (55, 86))], CREAM, r=1.4, marks=[([L([(69, 68), (71, 86)], 3)], mix(CREAM, LINE, 0.15))])
    bl(i, [E(46, 46, 82, 76)], RED, r=0.6, marks=[([E(48, 66, 80, 80)], RED_DARK), ([C(57, 55, 4.2), C(71, 53, 3.4), C(64, 63, 3.4), C(77, 62, 2.4), C(50, 63, 2.2)], BONE)])
    tiny_face(i, 64, 78, 0.75)


# ---- 펭귄 ----
PEN_ICE, PEN_ICE_DARK = (206, 236, 250), (140, 194, 228)


def penguin_pebble(i):
    """펭귄이 마음에 드는 친구에게 건네는 반들반들 조약돌 (하트 반짝)."""
    stone = (150, 156, 176)
    bl(i, [E(46, 58, 82, 86)], stone, r=0.3, marks=[([E(50, 76, 80, 92)], mix(stone, LINE, 0.25)), ([E(52, 61, 64, 67)], mix(stone, WHITE, 0.55))])
    heart = [C(62, 71, 3.2), C(68, 71, 3.2), T((59, 72), (71, 72), (65, 78.5))]
    paint(i, aa(shapes(heart)), (250, 170, 186))


def penguin_ice(i):
    """네모난 얼음 조각 (반짝 모서리, 작은 얼굴)."""
    bl(i, [T((48, 60), (70, 52), (82, 62), (80, 84), (56, 88), (46, 78))], PEN_ICE, r=1.6,
       marks=[([T((48, 60), (70, 52), (82, 62), (60, 68))], WHITE), ([T((60, 68), (82, 62), (80, 84), (62, 88))], mix(PEN_ICE, PEN_ICE_DARK, 0.5))])
    strokes(i, [[(56, 62), (64, 59)]], WHITE, 1.4)
    tiny_face(i, 60, 76, 0.85)


def penguin_fish(i):
    """은빛 멸치 한 마리 (통통한 몸 + 꼬리 + 동글 눈)."""
    silver = (176, 206, 226)
    bl(i, [E(48, 60, 78, 76), T((76, 68), (88, 58), (88, 78))], silver, r=0.8,
       marks=[([E(48, 68, 78, 80)], mix(silver, WHITE, 0.55)), ([L([(52, 63), (72, 62)], 1.6)], (110, 140, 188))])
    dots(i, [(55, 66)], INK, 1.6)
    dots(i, [(54.5, 65.3)], WHITE, 0.5)


def penguin_snowflake(i):
    """동글한 눈송이 (가운데 동그라미 + 여섯 갈래 + 끝 방울)."""
    snow = (232, 244, 255)
    arms = []
    for k in range(6):
        a = 60 * k - 90
        tip = polar(64, 66, 18, a)
        arms += [L([(64, 66), tip], 3.2), C(tip[0], tip[1], 3.0)]
        base = polar(64, 66, 10, a)
        arms += [L([base, polar(base[0], base[1], 5, a - 50)], 2.2), L([base, polar(base[0], base[1], 5, a + 50)], 2.2)]
    bl(i, arms + [C(64, 66, 7)], snow, r=0.2, line=0.6, marks=[([C(64, 66, 5)], PEN_ICE)])
    tiny_face(i, 64, 66, 0.6)


def penguin_down(i):
    """아기 펭귄의 회색 솜털 한 뭉치 (보송한 공 + 삐죽 털)."""
    down = (196, 200, 214)
    fluff = ring(64, 70, 13, 6.5, 9) + [C(64, 70, 14)]
    bl(i, fluff, down, r=1.0, marks=[([E(50, 76, 80, 90)], mix(down, LINE, 0.15)), ([E(54, 58, 64, 66)], mix(down, WHITE, 0.6))])
    strokes(i, [curve((62, 56), (60, 50), (66, 46), 6), curve((66, 56), (70, 50), (74, 51), 6)], mix(down, LINE, 0.35), 1.1)


def penguin_shell(i):
    """동글 소라 껍데기: 통통한 몸 + 뾰족 꼭지 + 나선 줄 + 분홍 입구."""
    shell = (250, 214, 190)
    m = bl(i, [C(60, 72, 15), T((52, 62), (82, 44), (74, 70))], shell, r=1.6,
           marks=[([E(44, 78, 78, 92)], mix(shell, LINE, 0.2)), ([E(50, 62, 58, 68)], mix(shell, WHITE, 0.6))])
    spiral = [polar(64, 66, 3 + t * 0.9, -40 + t * 26) for t in range(20)]
    strokes(i, [spiral, curve((66, 56), (74, 52), (80, 46))], mix(shell, LINE, 0.32), 1.1, clip=m)
    bl(i, [E(48, 72, 60, 86)], (252, 168, 172), r=0, line=0.5)


# ---- 레서판다 ----
RP_RUST, RP_RING = (214, 104, 58), (150, 64, 42)


def redpanda_bamboo(i):
    """길쭉한 대나무잎 한 장 (가운데 잎맥, 짧은 잎자루)."""
    a, b = (48, 86), (80, 48)
    mx, my = 64, 67
    bl(i, [L([(45, 89), (50, 84)], 2.2)], GREEN_DARK, r=0, line=0.5)
    bl(i, [T(a, (mx - 8, my - 9), b, (mx + 8, my + 7))], GREEN, r=1.0,
       marks=[([T(a, (mx + 8, my + 7), b)], mix(GREEN, LINE, 0.12)), ([E(55, 66, 61, 71)], GREEN_LIGHT)])
    strokes(i, [[a, b]], GREEN_DARK, 1.0)


def redpanda_berry(i):
    """빨간 산딸기 (작은 알이 오글오글) + 초록 꼭지."""
    bl(i, ring(64, 70, 8, 6.2, 7) + [C(64, 70, 9)], RED, r=0.4,
       marks=[([E(50, 74, 78, 88)], RED_DARK), ([C(59, 64, 2.2)], RED_LIGHT)])
    dots(i, [(64, 70), (58, 72), (70, 72), (64, 78), (70, 64)], RED_DARK, 1.0)
    for a in (-140, -90, -40):
        leaf(i, (64, 59), polar(64, 59, 8, a), 3.6)


def redpanda_stick(i):
    """오르기 좋은 작은 나뭇가지 (곁가지 하나에 새싹)."""
    bl(i, [L([(46, 84), (82, 50)], 5.0), L([(64, 67), (68, 56)], 3.2)], BROWN, r=0.3,
       marks=[([L([(48, 84), (82, 52)], 1.4)], BROWN_DARK)])
    leaf(i, (68, 56), (74, 48), 4.6)


def redpanda_maple(i):
    """빨간 단풍잎 (다섯 갈래 별 모양 + 잎자루)."""
    bl(i, [L([(64, 74), (64, 90)], 2.2)], BROWN_DARK, r=0, line=0.5)
    bl(i, [T(*star_pts(64, 64, 18, 0.5))], RED, r=1.0,
       marks=[([T(*star_pts(64, 66, 9, 0.5))], RED_DARK), ([E(56, 54, 61, 59)], RED_LIGHT)])
    strokes(i, [[(64, 74), polar(64, 64, 15, a)] for a in (-90, -18, -162)], RED_DARK, 0.8)


def redpanda_fluff(i):
    """레서판다 꼬리 털뭉치 (주황 복슬 공에 짙은 고리 줄 둘)."""
    m = bl(i, ring(64, 68, 12, 6.5, 9) + [C(64, 68, 13)], RP_RUST, r=1.0,
           marks=[([E(48, 76, 80, 90)], mix(RP_RUST, LINE, 0.15)), ([E(54, 56, 62, 62)], mix(RP_RUST, WHITE, 0.5))])
    strokes(i, [curve((50, 62), (64, 58), (78, 64)), curve((50, 74), (64, 70), (78, 76))], RP_RING, 3.2, clip=m)


def redpanda_flower(i):
    """분홍 들꽃 (다섯 잎 + 노란 가운데 + 잎 하나)."""
    bl(i, [L(curve((64, 90), (60, 80), (64, 70)), 2.0)], GREEN_DARK, r=0)
    leaf(i, (62, 84), (52, 78), 4.6)
    bl(i, ring(64, 62, 8, 6.4, 5) + [C(64, 62, 7)], PINK, r=0.4, marks=[([C(60, 58, 2.6)], PINK_LIGHT)])
    bl(i, [C(64, 62, 4.6)], GOLD, r=0, line=0.5)
    tiny_face(i, 64, 62.5, 0.5)


# ---- 기린 ----
def giraffe_acacia(i):
    stem = [(50, 85), (63, 66), (76, 47)]
    bl(i, [L(stem, 2.2)], BROWN, r=0)
    for t in (0.2, 0.38, 0.56, 0.74, 0.9):
        x, y = 50 + 26 * t, 85 - 38 * t
        for sgn in (-1, 1):
            leaf(i, (x, y), (x + 8.6 * sgn + 4, y + 6 * sgn + 3 * -1), 4.6)
    leaf(i, (76, 47), (80, 41), 4.6)


def giraffe_pod(i):
    pod = (172, 196, 96)
    bl(i, [L(curve((47, 56), (60, 86), (82, 72)), 11)], pod, r=0.2)
    bl(i, [L([(47, 56), (45, 49)], 2.2)], GREEN_DARK, r=0, line=0.5)
    pts = curve((50, 60), (60, 84), (79, 72), 8)[1:-1:2]
    dots(i, pts, mix(pod, WHITE, 0.45), 3.2)
    strokes(i, [curve((46.5, 60), (60, 82), (80, 68))], mix(pod, LINE, 0.25), 0.7)


def giraffe_mango(i):
    m = bl(i, [E(47, 54, 83, 85), C(58, 66, 11)], (250, 184, 76), r=1.2)
    paint(i, aa(shapes([E(64, 48, 90, 74)])), (240, 100, 80), clip=m)
    paint(i, aa(shapes([E(52, 58, 60, 66)])), GOLD_LIGHT, clip=m)
    bl(i, [L([(66, 54), (64, 47)], 2.2)], BROWN, r=0, line=0.5)
    leaf(i, (65, 50), (79, 45), 6.5)


def giraffe_cloud(i):
    bl(i, [C(53, 72, 9), C(64, 62, 12), C(76, 70, 10), E(46, 68, 84, 84)], WHITE, r=1.0, marks=[([E(46, 77, 84, 88)], BLUE_LIGHT)])
    tiny_face(i, 64, 70, 1.05)
    dots(i, [(55, 74), (73, 74)], PINK_LIGHT, 1.8)


def giraffe_nest(i):
    egg = (172, 216, 236)
    for x, y in ((57, 62), (71, 62)):
        bl(i, [E(x - 7, y - 10, x + 7, y + 11)], egg, r=0, line=0.6, marks=[([E(x - 4, y - 7, x, y - 3)], WHITE), ([C(x + 3, y + 1, 1), C(x - 2, y + 4, 0.9)], mix(egg, LINE, 0.35))])
    m = bl(i, [E(44, 66, 84, 87)], BROWN, r=0.3)
    strokes(i, [curve((45, 72), (64, 84), (83, 72)), curve((46, 78), (58, 72), (72, 86)), curve((82, 78), (66, 72), (54, 86)), [(48, 70), (58, 76)], [(80, 70), (70, 77)]], BROWN_LIGHT, 1.2, clip=m)
    strokes(i, [[(45, 68), (40, 64)], [(83, 69), (88, 66)]], BROWN, 1.3)


def giraffe_scarf(i):
    pts = curve((74, 46), (52, 50), (58, 64)) + curve((58, 64), (74, 74), (52, 84))
    m = bl(i, [L(pts, 9.5)], ORANGE, r=0.2)
    for t in (2, 5, 8, 11, 14, 17, 20, 23):
        x, y = pts[t]
        strokes(i, [[(x - 7, y - 6), (x + 7, y + 6)]], CREAM, 2.4, clip=m)
    for dx, end in ((-4, (72, 40)), (0, (77, 41)), (4, (82, 43))):
        strokes(i, [[(75 + dx, 47), end]], ORANGE_DARK, 1.4)


# ---- 루돌프 ----
def rudolph_bell(i):
    bl(i, [C(64, 80, 3.6)], GOLD_DARK, r=0, line=0.6)
    bl(i, [E(51, 50, 77, 72), T((53, 62), (75, 62), (82, 80), (46, 80))], GOLD, r=1.2, marks=[([E(54, 53, 61, 62)], GOLD_LIGHT), ([E(44, 74, 84, 86)], GOLD_DARK)])
    bow = RED
    bl(i, [T((64, 50), (51, 44), (51, 56)), T((64, 50), (77, 44), (77, 56))], bow, r=0.3, line=0.6)
    bl(i, [C(64, 50, 3)], RED_DARK, r=0, line=0.5)


def rudolph_bulb(i):
    bl(i, ring(64, 62, 0, 18, 1), mix(RED, WHITE, 0.72), r=0, line=0)
    bl(i, [C(64, 62, 13), T((54, 69), (74, 69), (68, 82), (60, 82))], RED, r=1.0, marks=[([E(56, 53, 62, 60)], WHITE), ([E(64, 64, 80, 84)], RED_DARK)])
    m = bl(i, [T((58, 78), (70, 78), (70, 87), (58, 87))], GREEN_DARK, r=0.3, line=0.6)
    strokes(i, [[(56, 82), (72, 82)]], GREEN, 1.0, clip=m)


def rudolph_snowflake(i):
    ice = (176, 222, 246)
    arms = []
    for k in range(6):
        a = 60 * k - 90
        tip = polar(64, 65, 19, a)
        arms.append(L([(64, 65), tip], 2.6))
        for d, w in ((10, 6), (15, 4.5)):
            base = polar(64, 65, d, a)
            arms += [L([base, polar(base[0], base[1], w, a - 55)], 1.9), L([base, polar(base[0], base[1], w, a + 55)], 1.9)]
    bl(i, arms, ice, r=0, line=0.55)
    bl(i, [hexagon(64, 65, 4)], WHITE, r=0, line=0.4)


def rudolph_star(i):
    bl(i, [T(*star_pts(64, 67, 21, 0.5))], GOLD, r=1.2, marks=[([E(43, 74, 85, 92)], GOLD_DARK), ([E(57, 53, 63, 60)], GOLD_LIGHT)])
    tiny_face(i, 64, 67, 1.1)


def rudolph_moss(i):
    moss = (196, 214, 170)
    bl(i, [E(46, 72, 82, 87), C(54, 70, 8), C(66, 64, 10), C(76, 72, 7)], moss, r=0.8, marks=[([E(46, 80, 82, 90)], mix(moss, LINE, 0.25))])
    pts = [(52, 66), (58, 72), (64, 58), (70, 66), (63, 70), (76, 70), (72, 76), (55, 78), (66, 78)]
    dots(i, pts, mix(moss, WHITE, 0.6), 1.5)
    bl(i, [C(82, 84, 2.4)], RED, r=0, line=0.45)


def rudolph_ribbon(i):
    bl(i, [T((60, 68), (48, 86), (55, 84), (59, 88), (66, 70)), T((68, 68), (80, 86), (73, 84), (69, 88), (62, 70))], RED_DARK, r=0.3, line=0.6)
    bl(i, [T((64, 62), (46, 49), (44, 72)), T((64, 62), (82, 49), (84, 72))], RED, r=1.1, marks=[([E(48, 53, 55, 60), E(73, 53, 80, 60)], RED_LIGHT)])
    bl(i, [E(58.5, 56, 69.5, 68)], RED_DARK, r=0, line=0.6, marks=[([E(61, 58, 65, 61)], RED)])


# ---- 산타 ----
def santa_giftbox(i):
    bl(i, [T((49, 62), (79, 62), (79, 86), (49, 86))], RED, r=0.4, marks=[([T((59, 62), (69, 62), (69, 86), (59, 86))], GOLD), ([T((49, 80), (79, 80), (79, 88), (49, 88))], RED_DARK)])
    bl(i, [T((46, 55), (82, 55), (82, 66), (46, 66))], RED_LIGHT, r=0.4, line=0.7, marks=[([T((59, 55), (69, 55), (69, 66), (59, 66))], GOLD_LIGHT)])
    bl(i, [T((64, 55), (51, 44), (50, 56)), T((64, 55), (77, 44), (78, 56))], GOLD, r=0.5, line=0.6)
    bl(i, [C(64, 53, 3)], GOLD_DARK, r=0, line=0.5)


def santa_candycane(i):
    hook = [polar(58, 55, 9, a) for a in range(180, 359, 15)]
    pts = hook + [(67, 56), (67, 86)]
    m = bl(i, [L(pts, 8.5)], WHITE, r=0.2)
    stripes = []
    for p in pts[::2]:
        stripes.append(L([(p[0] - 7, p[1] + 5), (p[0] + 7, p[1] - 5)], 3.2))
    for y in (64, 71, 78, 85):
        stripes.append(L([(60, y + 4), (74, y - 4)], 3.2))
    paint(i, aa(shapes(stripes)), RED, clip=m)


def santa_mitten(i):
    bl(i, [E(44, 56, 58, 72)], RED, r=0, marks=[([E(46, 58, 51, 62)], RED_LIGHT)])
    bl(i, [E(52, 46, 80, 80), T((55, 62), (77, 62), (77, 80), (55, 80))], RED, r=1.2, marks=[([E(56, 49, 64, 55)], RED_LIGHT), ([E(70, 50, 84, 80)], RED_DARK)])
    bl(i, [E(51, 75, 81, 88)], WHITE, r=0.2, marks=[([E(51, 82, 81, 92)], GRAY_LIGHT)])


def santa_cookie(i):
    ginger = (206, 142, 88)
    bl(i, [C(64, 52, 9), T((57, 58), (71, 58), (71, 74), (57, 74)), L([(58, 64), (47, 69)], 7), L([(70, 64), (81, 69)], 7), L([(60, 74), (54, 84)], 8), L([(68, 74), (74, 84)], 8)], ginger, r=0.8)
    tiny_face(i, 64, 51, 0.9)
    dots(i, [(64, 62), (64, 68)], RED, 1.4)
    strokes(i, [[(49, 67), (52, 69)], [(79, 67), (76, 69)], [(52.5, 80), (56, 82)], [(75.5, 80), (72, 82)]], WHITE, 1.0)


def santa_stocking(i):
    m = bl(i, [L([(60, 52), (60, 74)], 15), L([(60, 74), (75, 79)], 14)], RED, r=1.0, marks=[([E(70, 70, 86, 90)], GREEN), ([E(49, 76, 60, 88)], GREEN)])
    strokes(i, [[(53, 64), (67, 64)], [(53, 70), (67, 70)]], RED_LIGHT, 1.2, clip=m)
    bl(i, [E(49, 45, 71, 58)], WHITE, r=0.4, marks=[([E(49, 53, 71, 60)], GRAY_LIGHT)])


def santa_coal(i):
    coal = (72, 70, 82)
    bl(i, [T((50, 58), (64, 50), (78, 56), (83, 73), (72, 85), (52, 85), (45, 72))], coal, r=1.0, marks=[([T((64, 50), (78, 56), (68, 66), (56, 62))], (112, 110, 124)), ([T((56, 62), (68, 66), (64, 80), (48, 72))], (92, 90, 104))])
    dots(i, [(61, 56)], WHITE, 1.3)
    tiny = mix(ORANGE, WHITE, 0.2)
    dots(i, [(73, 76)], tiny, 1.0)


# ---- 눈사람 ----
SNOW_BLUE = (206, 226, 248)
SNOW_BLUE_DARK = (150, 184, 226)


def snowman_icicle(i):
    bl(i, [T((48, 48), (80, 48), (68, 90), (64, 94), (60, 90))], SNOW_BLUE, r=0.5, marks=[([T((66, 48), (82, 48), (68, 92))], SNOW_BLUE_DARK), ([L([(56, 52), (61, 76)], 2.4)], WHITE)])
    bl(i, [E(42, 42, 86, 54)], WHITE, r=0.3, line=0.7, marks=[([E(42, 50, 86, 58)], GRAY_LIGHT)])


def snowman_carrot(i):
    bl(i, [T((50, 56), (58, 48), (84, 82), (82, 85))], ORANGE, r=0.9, marks=[([T((54, 62), (62, 54), (64, 58), (56, 66))], ORANGE_DARK)])
    strokes(i, [[(60, 62), (64, 60)], [(66, 72), (70, 69)]], ORANGE_DARK, 1.3)
    bl(i, [L([(53, 52), (44, 44)], 3.4), L([(53, 52), (53, 41)], 3.4), L([(53, 52), (43, 53)], 3.4)], GREEN, r=0, line=0.7)
    tiny_face(i, 62, 64, 0.65)


def snowman_twig(i):
    bark = (150, 106, 76)
    bl(i, [L([(45, 85), (64, 64), (82, 44)], 4.6), L([(64, 64), (50, 52)], 3.6), L([(72, 55), (84, 62)], 3.6), L([(55, 74), (68, 82)], 3.2)], bark, r=0, line=0.8)
    strokes(i, [[(50, 79), (54, 74)], [(68, 59), (72, 55)]], mix(bark, WHITE, 0.3), 1.0)


def snowman_globe(i):
    bl(i, [T((48, 80), (80, 80), (84, 90), (44, 90))], RED, r=0.5, marks=[([E(44, 86, 84, 94)], RED_DARK)])
    bl(i, [C(64, 62, 21)], SNOW_BLUE, r=0, line=0.7, marks=[([E(40, 73, 88, 92)], WHITE), ([E(49, 45, 57, 52)], WHITE)])
    bl(i, [C(64, 73, 7), C(64, 63, 5.5)], WHITE, r=0.2, line=0.5)
    dots(i, [(62, 62), (66, 62)], (52, 52, 66), 0.7)
    dots(i, [(50, 58), (76, 55), (72, 68), (54, 68), (66, 48)], WHITE, 1.1)


def snowman_bucket(i):
    bl(i, [T((46, 82), (82, 82), (75, 50), (53, 50))], SNOW_BLUE_DARK, r=0.7, marks=[([T((68, 82), (84, 82), (76, 50), (66, 50))], mix(SNOW_BLUE_DARK, LINE, 0.2)), ([L([(55, 56), (53, 76)], 2.6)], WHITE)])
    bl(i, [E(43, 78, 85, 88)], mix(SNOW_BLUE_DARK, LINE, 0.25), r=0, line=0.6)
    bl(i, [E(52, 45, 76, 54)], WHITE, r=0.3, line=0.6)
    strokes(i, [[(53, 52), (48, 42), (64, 36), (80, 42), (75, 52)]], GRAY, 1.5)


def snowman_snowball(i):
    bl(i, [C(64, 67, 21)], WHITE, r=0, marks=[([E(44, 68, 88, 94)], SNOW_BLUE)])
    tiny_face(i, 64, 66, 1.1)
    dots(i, [(44, 46), (84, 50), (40, 58)], SNOW_BLUE_DARK, 1.2)


# ---- 공룡 ----
def dino_fossil(i):
    m = bl(i, [E(45, 52, 83, 87)], TAN, r=0.4, marks=[([E(50, 78, 80, 92)], mix(TAN, LINE, 0.25))])
    shell = [polar(64, 69, 1.2 + t * 0.55, t * 28) for t in range(26)]
    strokes(i, [shell], BROWN_DARK, 2.0, clip=m)
    for t in range(6, 26, 3):
        a = t * 28
        strokes(i, [[polar(64, 69, 1.2 + t * 0.55 - 2.0, a), polar(64, 69, 1.2 + t * 0.55 + 2.0, a)]], BROWN, 0.8, clip=m)
    dots(i, [(50, 62), (78, 77), (72, 56)], mix(TAN, WHITE, 0.5), 1.1, clip=m)


def dino_eggshell(i):
    egg = (250, 244, 224)
    top = [(46, 64), (52, 58), (56, 66), (62, 56), (68, 66), (73, 57), (77, 64), (82, 60)]
    bottom = [polar(64, 66, 20, a) for a in range(10, 171, 16)]
    # 깨진 위쪽 지그재그 + 둥근 아래쪽 (오른쪽 → 아래 → 왼쪽으로 돈다)
    pts = [(p[0], min(p[1], 66)) for p in top] + bottom
    bl(i, [T(*pts)], egg, r=0.1, marks=[([E(46, 76, 82, 90)], mix(egg, LINE, 0.15))])
    dots(i, [(54, 73), (64, 78), (74, 72), (60, 70)], (150, 196, 130), 1.6)
    bl(i, [T((70, 47), (76, 43), (79, 49))], egg, r=0.1, line=0.55)


def dino_fern(i):
    stem = curve((54, 87), (58, 62), (80, 47), 10)
    bl(i, [L(stem, 2.2)], GREEN_DARK, r=0)
    for k in range(1, 10):
        x, y = stem[k]
        size = 12.5 - k * 0.85
        leaf(i, (x, y), (x - size * 0.95, y - size * 0.42), 3.8, GREEN)
        leaf(i, (x, y), (x + size * 0.35, y + size * 0.9), 3.8, GREEN_DARK if k % 2 else GREEN)


def dino_amber(i):
    amber = (240, 158, 56)
    gem = [(58, 46), (74, 50), (83, 66), (74, 84), (54, 86), (45, 70), (48, 54)]
    bl(i, [T(*gem)], amber, r=1.0, marks=[([T((58, 46), (74, 50), (64, 62), (50, 56))], mix(amber, WHITE, 0.45)), ([E(50, 76, 84, 92)], ORANGE_DARK)])
    strokes(i, [[(59, 72), (55, 69)], [(59, 74), (55, 76)], [(69, 72), (73, 69)], [(69, 74), (73, 76)]], INK, 0.8)
    bl(i, [E(58, 68, 70, 78)], BROWN_DARK, r=0, line=0)
    dots(i, [(61, 71)], WHITE, 0.6)


def dino_tooth(i):
    tooth = (250, 246, 228)
    pts = curve((53, 52), (50, 72), (70, 89), 10) + curve((70, 89), (76, 68), (75, 52), 10)
    bl(i, [T(*pts), E(52, 44, 66, 60), E(62, 44, 76, 60)], tooth, r=0.8, marks=[([L(curve((73, 50), (72, 68), (70, 88)), 4)], mix(tooth, LINE, 0.2)), ([L(curve((57, 50), (56, 62), (60, 72)), 2)], WHITE)])


def dino_footprint(i):
    stone = (196, 188, 172)
    bl(i, [E(44, 50, 84, 87)], stone, r=0.4, marks=[([E(49, 78, 80, 92)], mix(stone, LINE, 0.25)), ([E(50, 53, 60, 57)], mix(stone, WHITE, 0.5))])
    print_ = mix(stone, LINE, 0.55)
    paint(i, aa(shapes([E(56, 66, 72, 81), L([(60, 70), (53, 57)], 4.2), L([(64, 68), (64, 54)], 4.6), L([(68, 70), (75, 57)], 4.2)])), print_)


DRAW = {name: globals()[name] for name in IDS}


def draw(name):
    """선물 하나를 ICON × ICON RGBA로 (아래 가운데 정렬)."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    with view(scale=1.0, lift=0):
        DRAW[name](img)
    art = finish(img)
    box = art.getchannel("A").getbbox()
    piece = art.crop(box)
    if piece.width > ICON or piece.height > ICON:
        piece.thumbnail((ICON, ICON), Image.LANCZOS)
    out = Image.new("RGBA", (ICON, ICON), (0, 0, 0, 0))
    out.alpha_composite(piece, ((ICON - piece.width) // 2, ICON - piece.height))
    return out


def sparkle(draw_, cx, cy, r):
    """네 갈래 별 (4배 슈퍼샘플 좌표)."""
    pts = [polar(cx, cy, r if k % 2 == 0 else r * 0.26, 45 * k - 90) for k in range(8)]
    draw_.polygon(pts, fill=(255, 255, 240, 255), outline=(214, 150, 40, 255))


def shiny(art):
    """보통 그림을 금빛으로 물들이고 별 셋을 찍는다."""
    rgba = art.convert("RGBA")
    px = rgba.load()
    for y in range(rgba.height):
        for x in range(rgba.width):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            lum = (0.3 * r + 0.59 * g + 0.11 * b) / 255
            gold = mix((186, 120, 28), (255, 232, 130), lum) if lum < 0.8 else mix((255, 232, 130), (255, 252, 226), (lum - 0.8) / 0.2)
            t = 0.62
            px[x, y] = (round(r * (1 - t) + gold[0] * t), round(g * (1 - t) + gold[1] * t), round(b * (1 - t) + gold[2] * t), a)
    k = 4
    stars = Image.new("RGBA", (ICON * k, ICON * k), (0, 0, 0, 0))
    d = ImageDraw.Draw(stars)
    for cx, cy, r in ((33, 8, 7.5), (6, 15, 5.5), (35, 26, 4.5)):
        sparkle(d, cx * k, cy * k, r * k)
    rgba.alpha_composite(stars.resize((ICON, ICON), Image.LANCZOS))
    return rgba


def build(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in IDS:
        art = draw(name)
        art.save(out_dir / f"{name}.png", optimize=True)
        gold = shiny(art)
        gold.save(out_dir / f"{name}{SHINY}.png", optimize=True)
        arts[name] = art
        arts[name + SHINY] = gold
    return arts


def preview(arts):
    """동물별 한 줄: 보통 6개 + 반짝이 6개. 왼쪽은 밝은 바탕, 오른쪽은 어두운 바탕 (2배)."""
    pad = 4
    cols = 6
    cell = ICON + pad
    w = cols * cell * 2 + pad
    h = len(LINES) * cell + pad
    half = cols * cell
    im = Image.new("RGBA", (w, h), (246, 242, 236, 255))
    im.paste((58, 66, 88, 255), (half + pad // 2, 0, w, h))
    for row, (key, names) in enumerate(LINES.items()):
        for col, name in enumerate(names):
            y = pad + row * cell
            im.alpha_composite(arts[f"{key}_{name}"], (pad + col * cell, y))
            im.alpha_composite(arts[f"{key}_{name}{SHINY}"], (half + pad + col * cell, y))
    PREVIEW.parent.mkdir(parents=True, exist_ok=True)
    im.resize((im.width * 2, im.height * 2), Image.NEAREST).save(PREVIEW)


def main():
    preview(build())
    print(f"ok: {len(IDS)} gifts")


if __name__ == "__main__":
    main()
