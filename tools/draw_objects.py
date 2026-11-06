"""물건 (쓰레기 장난감) — draw_pets.py 엔진으로 그린 128 칸 그림과 노란 포스트잇.

    python tools/draw_objects.py  → ui/objects/{can,paper,stone,glasses,computer,milk,shirt,umbrella,poop,pink,rainbow,pellets}.png (128×128)
                                    + ui/objects/postit.png (40×40)
                                    + tools/preview/objects.png (128px 고양이 옆에 놓아 본 모습, 포스트잇 붙인 줄 포함)

앱은 그림을 96 논리 px 칸에 맞춰 보이고, 쓰레기는 그 절반으로 그린다 (crates/screenpet-core/src/object.rs). 움직임은 그림을 더 그리지 않고
CSS로 돌린다 (ui/object.js) — 그래서 그림 한 장이면 된다.
바닥은 칸 아래 끝 BOTTOM에 앉는다. POSTIT_AT은 포스트잇을 붙일 자리 (ui/object.js POSTIT_AT과 같다).

    python tools/draw_objects.py --shapes  → 구르기에 쓰는 모양 표를 찍는다 (crates/screenpet-core/src/tumble.rs shape_of에 붙여 넣는다):
                                             무게중심(도는 중심), 볼록 껍질(어느 각도에서든 바닥에 닿는 점), 창에 필요한 반지름.
                                             그림을 고치면 다시 찍어야 한다 (tools/test_draw_objects.py가 어긋나면 알려 준다)
"""

import random
import sys
from math import ceil, hypot
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish  # noqa: E402
from draw_pets import LINE, WHITE, C, E, L, T, aa, mix, paint, shapes, view  # noqa: E402
from draw_props import new_canvas  # noqa: E402
from sprite_spec import FRAME  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "objects"
PREVIEW = ROOT / "tools" / "preview" / "objects.png"

BOTTOM = 126.5
POSTIT = 40
# 앱에서 포스트잇은 28/128 칸 폭 — 가운데에서 모서리까지
POSTIT_REACH = 20
# 구르기 모양 표의 볼록 껍질 꼭짓점 수
HULL_POINTS = 12

# crates/screenpet-core/src/object.rs ObjectKind::id와 같은 이름, 같은 순서
NAMES = ("can", "paper", "stone", "glasses", "computer", "milk", "shirt", "umbrella", "poop", "pink", "rainbow", "pellets")
# 포스트잇 가운데 자리 (128 칸 좌표) — 그림 위에 얹힌다. 똥(poop, pink, rainbow, pellets)엔 포스트잇을 붙이지 않는다
POSTIT_AT = {
    "can": (76, 80),
    "paper": (86, 104),
    "stone": (76, 86),
    "glasses": (40, 100),
    "computer": (92, 72),
    "milk": (74, 92),
    "shirt": (64, 100),
    "umbrella": (46, 96),
}

CAN_RED = (232, 96, 92)
METAL = (206, 212, 220)
NEWS = (216, 216, 210)
NEWS_LIGHT = (232, 232, 226)
NEWS_FOLD = (194, 194, 190)
NEWS_INK = (88, 90, 96)
NEWS_PHOTO = (132, 134, 140)
STONE = (146, 134, 120)
STONE_LIGHT = (172, 160, 144)
STONE_DARK = (104, 94, 84)
STONE_CRACK = (66, 58, 52)
FRAME_BROWN = (146, 100, 76)
LENS = (214, 236, 250)
PLASTIC = (222, 214, 196)
SCREEN = (34, 36, 42)
SPARK = (255, 214, 102)
MILK_BLUE = (138, 186, 232)
SHIRT = (106, 160, 232)
CLOTH = (150, 132, 214)
CLOTH_LIGHT = mix(CLOTH, WHITE, 0.35)
HANDLE = (176, 128, 92)
POOP = (138, 92, 58)
PINK_POOP = (240, 136, 176)
RAINBOW = ((164, 130, 226), (106, 160, 232), (124, 190, 96), (252, 214, 84), (244, 150, 60), (232, 84, 84))
PELLET = (112, 76, 50)
NOTE = (255, 232, 110)
NOTE_FOLD = (238, 200, 74)
NOTE_LINE = (196, 160, 70)


def draw_can(img):
    """찌그러진 캔: 허리가 꺾인 원통, 윗뚜껑과 따개."""
    body = [T((43, 66), (85, 66), (84, 88), (78, 97), (86, 110), (84, 122), (44, 122), (42, 106), (49, 94), (42, 84))]
    blob(img, body + [E(44, 116, 84, BOTTOM)], CAN_RED, r=1.6,
         marks=[([T((42, 76), (86, 76), (86, 84), (42, 84))], WHITE),
                ([L([(50, 98), (58, 94), (66, 99)], 1.4)], mix(CAN_RED, LINE, 0.4)),
                ([E(47, 70, 53, 115)], mix(CAN_RED, WHITE, 0.35))])
    blob(img, [E(43, 60, 85, 72)], METAL, r=0, line=0.9,
         marks=[([E(49, 63, 79, 70)], mix(METAL, LINE, 0.25)), ([E(60, 64, 70, 68)], LINE)])


def draw_paper(img):
    """구겨진 신문지: 덜 뭉쳐 일부 펼쳐진 연회색 장. 삐죽한 모서리, 접힌 음영, 제목 블록·사진 칸·글줄."""
    sheet = [T((16, 90), (30, 72), (24, 56), (48, 62), (64, 50), (84, 60), (108, 50), (100, 72), (116, 88), (104, 104), (112, 120),
               (84, BOTTOM), (62, 118), (38, BOTTOM), (18, 118), (28, 102))]
    left_ink = [L([(32, 82 + i * 6), (58, 80 + i * 6)], 1.7) for i in range(6)]
    right_ink = [L([(68, 88 + i * 6), (104, 91 + i * 6)], 1.7) for i in range(5)]
    blob(img, sheet, NEWS, r=0.5,
         marks=[([T((64, 50), (84, 60), (74, 88), (60, 86))], NEWS_FOLD),      # 접혀 그늘진 면
                ([T((60, 86), (74, 88), (84, BOTTOM), (62, 118))], NEWS_FOLD),
                ([T((30, 66), (58, 63), (58, 73), (30, 76))], NEWS_INK),       # 제목 블록
                ([T((68, 62), (94, 62), (94, 81), (69, 83))], NEWS_PHOTO),     # 사진 칸
                ([E(74, 66, 84, 76)], mix(NEWS_PHOTO, WHITE, 0.3)),
                (left_ink + right_ink, NEWS_INK),
                ([L([(64, 50), (60, 86), (62, 118)], 1.3), L([(16, 90), (60, 86)], 1.1), L([(60, 86), (116, 88)], 1.1)], mix(NEWS_FOLD, LINE, 0.5))])
    # 오른쪽 아래 귀퉁이가 접혀 올라왔다
    blob(img, [T((112, 120), (104, 104), (88, 112), (84, BOTTOM))], NEWS_LIGHT, r=0.3, line=0.8)


def draw_stone(img):
    """돌멩이: 울퉁불퉁한 회갈색 덩어리. 면이 나뉜 음영, 잔 반점, 작은 균열, 아래쪽 어두운 면."""
    outline = [(30, 92), (40, 70), (60, 60), (82, 64), (98, 80), (102, 102), (92, 120), (68, BOTTOM), (42, 123), (28, 108)]
    spots = random.Random(11)
    specks = [C(spots.uniform(30, 100), spots.uniform(62, 124), spots.uniform(0.7, 1.4)) for _ in range(42)]
    blob(img, [T(*outline)], STONE, r=2.0,
         marks=[([T((40, 70), (60, 60), (82, 64), (70, 80), (46, 86))], STONE_LIGHT),                  # 위쪽 밝은 면
                ([T((82, 64), (98, 80), (102, 102), (82, 96), (70, 80))], mix(STONE, STONE_DARK, 0.35)),  # 오른쪽 면
                ([T((28, 108), (46, 106), (66, 112), (88, 106), (102, 102), (92, 124), (68, 130), (40, 128))], STONE_DARK),  # 아래쪽 그림자 면
                (specks, mix(STONE, STONE_CRACK, 0.55)),
                ([L([(70, 80), (46, 86), (30, 92)], 1.0), L([(70, 80), (82, 96), (102, 102)], 1.0), L([(70, 80), (82, 64)], 0.9)], mix(STONE, LINE, 0.35)),
                ([L([(52, 90), (58, 98), (54, 104), (60, 112)], 1.2), L([(58, 98), (66, 100)], 1.0)], STONE_CRACK)])


def draw_glasses(img):
    """금 간 안경: 동그란 알 둘, 한쪽 알에 금, 다리 하나는 꺾였다."""
    y = BOTTOM - 20
    blob(img, [L([(19, y - 14), (10, y - 22), (8, y - 30)], 3.0), L([(109, y - 14), (118, y - 8), (122, y + 4)], 3.0)], FRAME_BROWN, r=0, line=0.7)
    for cx in (40, 88):
        blob(img, [C(cx, y, 19.5)], FRAME_BROWN, r=0, line=0.8)
        blob(img, [C(cx, y, 15)], LENS, r=0, line=0,
             marks=[([E(cx - 10, y - 11, cx - 2, y - 5)], WHITE)])
    blob(img, [L([(58, y - 4), (64, y - 8), (70, y - 4)], 3.2)], FRAME_BROWN, r=0, line=0.7)
    paint(img, aa(shapes([L([(80, y - 13), (88, y - 3), (82, y + 3), (92, y + 13)], 1.2), L([(88, y - 3), (98, y - 6)], 1.0),
                          L([(82, y + 3), (76, y + 7)], 1.0)])), LINE)


def draw_computer(img):
    """고장난 컴퓨터: 네모 모니터, 금 간 까만 화면, 튀는 불꽃."""
    blob(img, [T((48, 108), (80, 108), (84, BOTTOM - 7), (44, BOTTOM - 7)), E(36, BOTTOM - 10, 92, BOTTOM)], mix(PLASTIC, LINE, 0.15), r=0.8, line=0.9)
    blob(img, [T((18, 44), (110, 44), (110, 110), (18, 110))], PLASTIC, r=3.0)
    blob(img, [T((27, 52), (101, 52), (101, 98), (27, 98))], SCREEN, r=2.0, line=0.7)
    paint(img, aa(shapes([L([(40, 52), (52, 66), (46, 76), (60, 88), (56, 98)], 1.3), L([(52, 66), (72, 58), (84, 52)], 1.1),
                          L([(46, 76), (32, 82)], 1.0), L([(60, 88), (78, 82), (94, 90)], 1.1), L([(72, 58), (78, 70)], 0.9)])), mix(SCREEN, WHITE, 0.6))
    blob(img, [T((106, 28), (114, 38), (108, 38), (116, 50), (100, 36), (106, 36))], SPARK, r=0.2, line=0.7)


def smooth(points, n=6):
    """꺾은선을 부드러운 곡선으로 (Catmull-Rom)."""
    out = []
    last = len(points) - 1
    for i in range(last):
        p0, p1, p2, p3 = points[max(i - 1, 0)], points[i], points[i + 1], points[min(i + 2, last)]
        for k in range(n):
            t = k / n
            out.append(tuple(
                0.5 * (2 * p1[a] + (p2[a] - p0[a]) * t + (2 * p0[a] - 5 * p1[a] + 4 * p2[a] - p3[a]) * t ** 2
                       + (3 * p1[a] - p0[a] - 3 * p2[a] + p3[a]) * t ** 3)
                for a in (0, 1)))
    return out + [points[-1]]


def draw_milk(img):
    """빈 우유팩: 뾰족 지붕, 한쪽이 눌린 몸통, 파란 띠."""
    blob(img, [T((42, 80), (86, 80), (84, 100), (88, 112), (86, BOTTOM - 1), (42, BOTTOM - 1), (40, 104))], WHITE, r=1.4,
         marks=[([T((38, 108), (90, 108), (90, 120), (38, 120))], MILK_BLUE),
                ([L([(74, 84), (80, 100), (76, 112)], 1.2)], mix(WHITE, LINE, 0.3))])
    blob(img, [T((42, 80), (86, 80), (78, 62), (50, 62))], mix(WHITE, MILK_BLUE, 0.25), r=0.8, line=0.9)
    blob(img, [T((48, 54), (80, 54), (80, 63), (48, 63))], WHITE, r=0.6, line=0.9)


def draw_shirt(img):
    """누더기 티셔츠: 납작하게 구겨졌고, 밑단과 소매가 들쭉날쭉 뜯겼고, 구멍이 나고 덧댄 천이 붙었다."""
    y = BOTTOM
    rags = [(18, y - 46), (36, y - 62), (50, y - 63), (64, y - 53), (78, y - 65), (96, y - 60), (122, y - 40),
            (116, y - 34), (118, y - 27), (110, y - 30), (107, y - 24), (97, y - 35),
            (100, y - 22), (97, y - 16), (103, y - 8),
            (96, y - 2), (90, y - 11), (83, y - 1), (75, y - 8), (68, y), (60, y - 10), (52, y - 2), (45, y - 12), (38, y - 1), (30, y - 6),
            (33, y - 18), (29, y - 26), (33, y - 37),
            (26, y - 31), (22, y - 26), (19, y - 33), (13, y - 30), (15, y - 39)]
    fold = mix(SHIRT, LINE, 0.45)
    wrinkles = [L(pts, 1.5) for pts in (
        [(44, y - 48), (54, y - 38), (50, y - 28)],
        [(84, y - 50), (76, y - 40), (80, y - 32)],
        [(20, y - 42), (30, y - 48)],
        [(112, y - 42), (102, y - 48)],
    )]
    hole = mix(SHIRT, LINE, 0.8)
    stitches = [L(pts, 1.1) for pts in (
        [(75, y - 28), (72, y - 25)], [(79, y - 30), (79, y - 34)], [(90, y - 22), (94, y - 20)], [(85, y - 13), (85, y - 9)],
    )]
    blob(img, [T(*rags)], SHIRT, r=0.4,
         marks=[([L([(51, y - 62), (64, y - 50), (77, y - 63)], 2.6)], mix(SHIRT, WHITE, 0.55)),
                ([T((74, y - 30), (92, y - 28), (90, y - 12), (75, y - 14))], mix(SHIRT, WHITE, 0.6)),
                (stitches, LINE),
                (wrinkles, fold),
                # 구멍 (둥글면 입처럼 보인다)
                ([T((42, y - 28), (47, y - 25), (53, y - 29), (58, y - 23), (52, y - 19), (49, y - 22), (43, y - 17), (40, y - 23))], hole),
                ([T((62, y - 44), (66, y - 41), (70, y - 44), (68, y - 38), (63, y - 37))], hole)])


def draw_umbrella(img):
    """뒤집힌 고장 우산: 천이 바닥에 닿고 꺾인 대와 갈고리 손잡이가 위로, 천을 뚫고 나온 살."""
    blob(img, [L([(64, 98), (72, 74), (60, 58), (60, 44), (52, 38), (47, 46)], 3.6)], HANDLE, r=0, line=0.8)
    blob(img, [T((22, 86), (40, 108), (64, 118), (92, 110), (112, 84), (94, 92), (82, 78), (66, 90), (50, 76), (38, 92))], CLOTH, r=1.6,
         marks=[([L([(64, 118), (66, 90)], 1.6), L([(64, 118), (40, 92)], 1.4), L([(64, 118), (88, 92)], 1.4)], CLOTH_LIGHT)])
    blob(img, [L([(64, 118), (64, 124)], 3.0)], HANDLE, r=0, line=0.7)
    # 천을 뚫고 나온 살
    paint(img, aa(shapes([L([(92, 104), (108, 116), (114, 114)], 1.8)])), mix(METAL, LINE, 0.3))


def swirl(img, x0, x1, height, colour, bands=()):
    """전형적인 똥 (얼굴 없는 💩): 뚱뚱한 아랫단, 둥근 윗단, 오른쪽으로 휘어 올라간 꼭지."""
    top = BOTTOM - height
    w, mid = x1 - x0, (x0 + x1) / 2
    def p(fx, fy):
        return (mid + w * fx, top + height * fy)
    def lobe(half, y0, y1, shift=0.0):
        return E(mid + w * (shift - half), top + height * y0, mid + w * (shift + half), top + height * y1)
    shapes = [lobe(0.5, 0.52, 0.965), lobe(0.46, 0.66, 0.965), lobe(0.36, 0.22, 0.68),
              T(p(-0.19, 0.24), p(-0.06, 0.19), p(0.03, 0.13), p(0.085, 0.05), p(0.115, 0.0), p(0.145, 0.03), p(0.175, 0.12), p(0.19, 0.24), p(0.05, 0.34), p(-0.14, 0.34))]
    groove = mix(colour, LINE, 0.6)
    shine = mix(colour, WHITE, 0.3)
    line = max(1.4, w * 0.026)
    # 무지개: 위(꼭지)부터 아래까지 가로 띠
    stripes = [([T(p(-0.6, k / len(bands)), p(0.6, k / len(bands)), p(0.6, (k + 1) / len(bands)), p(-0.6, (k + 1) / len(bands)))], c)
               for k, c in enumerate(bands)]
    if bands:
        groove = mix(LINE, WHITE, 0.25)
    blob(img, shapes, colour, r=0.8,
         marks=stripes + [([L([p(-0.33, 0.56), p(-0.15, 0.65), p(0.15, 0.65), p(0.33, 0.56)], line),
                  L([p(-0.11, 0.29), p(0.02, 0.345), p(0.11, 0.31), p(0.175, 0.23)], line)], groove),
                ([E(*p(-0.38, 0.70), *p(-0.2, 0.79))], shine),
                ([E(*p(-0.24, 0.37), *p(-0.1, 0.45))], shine),
                ([E(*p(0.03, 0.14), *p(0.1, 0.21))], shine)])


def draw_poop(img):
    """강아지 등 큰 동물의 똥: 전형적인 모양의 갈색 똥."""
    swirl(img, 26, 102, 70, POOP)


def draw_pink(img):
    """핑크 똥: 똥과 같은 모양, 분홍빛."""
    swirl(img, 26, 102, 70, PINK_POOP)


def draw_rainbow(img):
    """가끔 나오는 무지개 똥: 똥과 같은 모양에 무지개 띠."""
    swirl(img, 26, 102, 70, RAINBOW[0], bands=RAINBOW)


def draw_pellets(img):
    """토끼 똥: 동글동글한 알갱이 여럿이 따로따로 흩어져 있다."""
    for cx, cy, r in ((27, 120, 6), (42, 110, 5.5), (56, 121, 5.5), (71, 111, 5.5), (86, 120.5, 6), (102, 112, 5.5)):
        blob(img, [C(cx, cy, r)], PELLET, r=0, line=0.9,
             marks=[([C(cx - r * 0.3, cy - r * 0.32, r * 0.3)], mix(PELLET, WHITE, 0.3))])


DRAW = {
    "can": draw_can,
    "paper": draw_paper,
    "stone": draw_stone,
    "glasses": draw_glasses,
    "computer": draw_computer,
    "milk": draw_milk,
    "shirt": draw_shirt,
    "umbrella": draw_umbrella,
    "poop": draw_poop,
    "pink": draw_pink,
    "rainbow": draw_rainbow,
    "pellets": draw_pellets,
}


def draw_object(name):
    img = new_canvas()
    with view(scale=1.0, lift=0):
        DRAW[name](img)
    return finish(img)


def draw_postit():
    """노란 포스트잇: 살짝 기운 네모, 접힌 귀퉁이, 글씨 줄 (봉투도 하트도 아니다)."""
    img = new_canvas()
    with view(scale=1.0, lift=0):
        blob(img, [T((48, 49), (80, 47), (82, 77), (72, 80), (50, 81))], NOTE, r=0.6, line=0.8,
             marks=[([T((72, 80), (82, 77), (72, 70))], NOTE_FOLD),
                    ([L([(54, 56), (75, 55)], 1.5), L([(54, 62), (76, 61)], 1.5), L([(54, 68), (68, 67.5)], 1.5)], NOTE_LINE)])
    box = ((FRAME - POSTIT) // 2, (FRAME - POSTIT) // 2, (FRAME + POSTIT) // 2, (FRAME + POSTIT) // 2)
    return finish(img).crop(box)


def build(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in NAMES:
        arts[name] = draw_object(name)
        arts[name].save(out_dir / f"{name}.png")
    arts["postit"] = draw_postit()
    arts["postit"].save(out_dir / "postit.png")
    return arts


def with_postit(art, name, postit):
    """앱과 같은 크기로 포스트잇을 붙여 본다 (앱은 28/128 칸 폭으로 보인다)."""
    out = art.copy()
    side = 28
    small = postit.resize((side, side), Image.LANCZOS)
    if name not in POSTIT_AT:
        return out
    x, y = POSTIT_AT[name]
    out.alpha_composite(small, (x - side // 2, y - side // 2))
    return out


def preview(arts, path=PREVIEW):
    """윗줄: 물건들, 아랫줄: 포스트잇을 붙인 모습 + 128px 고양이. 앱에서처럼 96px로 줄여 놓는다."""
    cell = 96
    n = len(NAMES)
    w = cell * n + FRAME
    h = cell * 2 + 32
    im = Image.new("RGBA", (w, h), (246, 242, 236, 255))
    for i, name in enumerate(NAMES):
        x = i * cell
        im.alpha_composite(arts[name].resize((cell, cell), Image.LANCZOS), (x, 0))
        im.alpha_composite(with_postit(arts[name], name, arts["postit"]).resize((cell, cell), Image.LANCZOS), (x, cell + 32))
    sheet = ROOT / "ui" / "pets" / "cat" / "sheet_teenager.png"
    if sheet.exists():
        cat = Image.open(sheet).convert("RGBA").crop((0, 0, FRAME, FRAME))
        im.alpha_composite(cat, (cell * n, h - FRAME))
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path)


def convex_hull(points):
    """볼록 껍질 (앤드류), 화면 좌표에서 시계 방향."""
    pts = sorted(set(points))

    def half(seq):
        out = []
        for p in seq:
            while len(out) >= 2 and (out[-1][0] - out[-2][0]) * (p[1] - out[-2][1]) - (out[-1][1] - out[-2][1]) * (p[0] - out[-2][0]) <= 0:
                out.pop()
            out.append(p)
        return out[:-1]

    return half(pts) + half(reversed(pts))


def simplify_hull(hull, keep=HULL_POINTS):
    """꼭짓점을 줄인다: 지워도 모양이 가장 덜 변하는 것부터 (삼각형 넓이가 작은 것)."""
    hull = list(hull)

    def area(i):
        a, b, c = hull[i - 1], hull[i], hull[(i + 1) % len(hull)]
        return abs((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]))

    while len(hull) > keep:
        del hull[min(range(len(hull)), key=area)]
    return hull


def shape_of(name, art):
    """구르기에 쓰는 모양: 무게중심 pivot, 볼록 껍질 hull, 포스트잇까지 담는 반지름 reach (모두 128 칸 좌표)."""
    a = np.asarray(art.getchannel("A"))
    solid = a >= 128
    ys, xs = np.nonzero(solid)
    pivot = (int(round(xs.mean() + 0.5)), int(round(ys.mean() + 0.5)))
    corners = []
    for y in np.unique(ys):
        row = xs[ys == y]
        corners += [(int(row.min()), int(y)), (int(row.max()) + 1, int(y)), (int(row.min()), int(y) + 1), (int(row.max()) + 1, int(y) + 1)]
    hull = simplify_hull(convex_hull(corners))
    ys, xs = np.nonzero(a > 0)
    far = float(np.hypot(xs + 0.5 - pivot[0], ys + 0.5 - pivot[1]).max()) + 1.0
    note = 0.0
    if name in POSTIT_AT:
        nx, ny = POSTIT_AT[name]
        note = hypot(nx - pivot[0], ny - pivot[1]) + POSTIT_REACH
    return {"pivot": pivot, "hull": hull, "reach": ceil(max(far, note))}


def shape_table():
    """tumble.rs shape_of의 몸 (이 글자 그대로 붙여 넣는다)"""
    lines = []
    for name in NAMES:
        s = shape_of(name, draw_object(name))
        hull = ", ".join(f"({x}.0, {y}.0)" for x, y in s["hull"])
        lines.append(f"        ObjectKind::{name.capitalize()} => Shape {{ pivot: ({s['pivot'][0]}.0, {s['pivot'][1]}.0), reach: {s['reach']}.0, hull: &[{hull}] }},")
    return "\n".join(lines)


def main():
    if "--shapes" in sys.argv:
        print(shape_table())
        return
    arts = build()
    preview(arts)
    print(f"wrote {len(arts)} images to {OUT_DIR} and {PREVIEW}")


if __name__ == "__main__":
    main()
