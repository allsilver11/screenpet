"""먹이 소품 — draw_pets.py 엔진(슈퍼샘플 마스크·melt·외곽선·paint)으로 그린 작은 그림, draw_house.py와 같은 말랑한 외곽선.

    python tools/draw_props.py  → ui/effects/{bowl_food,bowl_water,cookie}.png
                                  + ui/effects/hand_{meal,cookie,fish,carrot,fly,candy,lollipop,apple,milk,bread,leaf,watermelon,broccoli,grapes,shaved_ice,seeds,special}.png (직접 주기, 48×48)
                                  + tools/preview/hand_food.png
                                  + tools/preview/props.png (128px 고양이 옆에 놓아 본 모습)
                                  + ui/effects/{snowball,snowman}.png (계절 날씨) + tools/preview/snowman.png

앱(src-tauri/src/prop.rs)은 동물이 먹을 때 몸 앞 바닥에 WIDTH × HEIGHT(논리 px, 몸 배율 1 기준) 투명 창을 띄우고
그림을 아래 가운데에 붙여 보인다 (ui/prop.html). 쿠키는 입가에 뜬다.
그림은 모두 같은 상자(WIDTH × HEIGHT) — 바닥은 아래 끝 1px 투명 여백 위에 앉는다.
"""

import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish, paw  # noqa: E402
from draw_pets import LINE, S, WHITE, C, E, L, T, aa, arc, mix, paint, shapes, view, xf  # noqa: E402
from sprite_spec import FRAME  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "effects"
PREVIEW = ROOT / "tools" / "preview" / "props.png"

# 소품 상자 (src-tauri/src/prop.rs WIDTH, HEIGHT와 같다)
WIDTH, HEIGHT = 56, 48
# 128 프레임 안의 상자: 가운데 x = 64, 아래 = 128
BOX = ((FRAME - WIDTH) // 2, FRAME - HEIGHT, (FRAME + WIDTH) // 2, FRAME)
BOTTOM = 126.5  # 그림 바닥 (프레임 좌표)

# ---- 색 ----
BOWL_PINK = (246, 160, 170)
BOWL_PINK_LIGHT = mix(BOWL_PINK, WHITE, 0.45)
BOWL_BLUE = (138, 186, 232)
BOWL_BLUE_LIGHT = mix(BOWL_BLUE, WHITE, 0.45)
KIBBLE = (204, 140, 88)
RICE = (255, 252, 244)
WATER = (188, 226, 250)
WATER_DEEP = (150, 204, 240)
COOKIE = (232, 184, 116)
COOKIE_DARK = mix(COOKIE, LINE, 0.3)
CHIP = (110, 70, 58)
LEAF = (150, 204, 124)

NAMES = ("bowl_food", "bowl_water", "cookie")


def new_canvas():
    return Image.new("RGBA", (S, S), (0, 0, 0, 0))


def bowl_body(img, color, light):
    """둥근 그릇 몸통 + 테 (위에서 비스듬히 본 모습). 안쪽(테 타원)은 내용물이 덮는다."""
    cx = 64
    blob(img, [T((cx - 21.5, 106), (cx + 21.5, 106), (cx + 18, BOTTOM - 3), (cx - 18, BOTTOM - 3)),
               E(cx - 19, BOTTOM - 8, cx + 19, BOTTOM)], color, r=1.2,
         marks=[(paw(cx, 118.5, 0.85), light),
                ([L([(cx - 19, 109.5), (cx + 19, 109.5)], 1.2)], light)])
    blob(img, [E(cx - 23.5, 100, cx + 23.5, 112)], light, r=0, line=0.9)


def front_lip(img, color):
    """그릇 테의 앞쪽 가장자리 (내용물 아래쪽을 덮는다)."""
    pts = arc(64, 106, 22, 5, 10, 170, 20)
    blob(img, [L(pts, 2.6)], color, r=0, line=0.7)


def draw_bowl_food(img):
    bowl_body(img, BOWL_PINK, BOWL_PINK_LIGHT)
    # 수북한 밥 + 사료 알갱이
    mound = blob(img, [E(43, 95, 85, 109), E(48, 89, 80, 104), E(54, 85, 74, 98)], RICE, r=1.6, line=0.8)
    kibbles = [(52, 96), (60, 92), (68, 95), (76, 99), (57, 101), (66, 101), (49, 102), (79, 103), (63, 88)]
    for x, y in kibbles:
        blob(img, [E(x - 3.2, y - 2.4, x + 3.2, y + 2.4)], KIBBLE, r=0, line=0.5,
             marks=[([C(x - 0.9, y - 0.8, 0.9)], mix(KIBBLE, WHITE, 0.4))])
    paint(img, aa(shapes([E(55, 88, 62, 92)])), WHITE, clip=mound)
    front_lip(img, BOWL_PINK_LIGHT)


def draw_bowl_water(img):
    bowl_body(img, BOWL_BLUE, BOWL_BLUE_LIGHT)
    water = blob(img, [E(43.5, 101.5, 84.5, 110.5)], WATER, r=0, line=0.6)
    paint(img, aa(shapes([E(47, 105, 81, 110.5)])), WATER_DEEP, clip=water)
    # 반짝이는 물결
    paint(img, aa(shapes([L([(49, 104), (56, 103.4)], 1.2), L([(66, 103.2), (71, 103.6)], 1.0)])), WHITE, clip=water)
    # 물방울 하나
    blob(img, [C(80, 92, 3.0), T((77.6, 91), (82.4, 91), (80, 86))], WATER, r=0.3, line=0.6,
         marks=[([C(79, 91.5, 0.9)], WHITE)])
    front_lip(img, BOWL_BLUE_LIGHT)


def draw_cookie(img):
    cx, cy, r = 64, BOTTOM - 13, 12
    blob(img, [C(cx, cy, r)], COOKIE, r=0.6,
         marks=[([E(cx - r + 2, cy + 2, cx + r - 2, cy + r + 1)], COOKIE_DARK),
                ([C(cx - 4, cy - 4, 2.0), C(cx + 5, cy - 2, 1.8), C(cx - 1, cy + 4, 2.0), C(cx + 4, cy + 6, 1.4),
                  C(cx - 7, cy + 3, 1.4)], CHIP),
                ([C(cx - 5, cy - 6.5, 1.5)], mix(COOKIE, WHITE, 0.5))])
    # 부스러기
    blob(img, [C(cx + 15, BOTTOM - 3, 1.6), C(cx - 15, BOTTOM - 2.5, 1.3)], COOKIE, r=0, line=0.5)


# ---- 직접 주기: 커서 끝에 붙는 음식 (HAND × HAND, 가운데 (64, 64)) ----
HAND = 48
HAND_BOX = ((FRAME - HAND) // 2, (FRAME - HAND) // 2, (FRAME + HAND) // 2, (FRAME + HAND) // 2)
# crates/screenpet-core/src/food.rs hand_icon과 같은 이름
NORI = (70, 96, 82)
FISH = (150, 186, 214)
FISH_BELLY = mix(FISH, WHITE, 0.6)
CARROT = (246, 150, 70)
CARROT_DARK = mix(CARROT, LINE, 0.3)
FLY = (92, 98, 122)
FLY_EYE = (60, 42, 48)
WING = (214, 236, 250)
CANDY = (255, 160, 70)
CANDY_WRAP = (190, 154, 234)
LOLLY = (250, 130, 168)
STICK = (255, 250, 242)


def tiny_face(img, cx, cy, s=1.0):
    """점 눈 둘 + 볼터치 — 음식도 말랑하게."""
    paint(img, aa(shapes([C(cx - 4.5 * s, cy, 1.3 * s), C(cx + 4.5 * s, cy, 1.3 * s)])), (84, 58, 56))
    paint(img, aa(shapes([E(cx - 8 * s, cy + 1.8 * s, cx - 4.5 * s, cy + 4 * s), E(cx + 4.5 * s, cy + 1.8 * s, cx + 8 * s, cy + 4 * s)])),
          (250, 180, 180))


def hand_meal(img):
    rice = blob(img, [T((64, 45), (85, 81), (43, 81))], RICE, r=4.5, line=1.0)
    paint(img, aa(shapes([T((55, 69), (73, 69), (73, 86), (55, 86))])), NORI, clip=rice)
    paint(img, aa(shapes([L([(55, 69.5), (73, 69.5)], 1.0)])), mix(NORI, WHITE, 0.25), clip=rice)
    tiny_face(img, 64, 60, 0.9)


def hand_cookie(img):
    cx, cy, r = 64, 64, 18
    blob(img, [C(cx, cy, r)], COOKIE, r=0.6,
         marks=[([E(cx - r + 3, cy + 4, cx + r - 3, cy + r + 1)], COOKIE_DARK),
                ([C(cx - 7, cy - 6, 2.6), C(cx + 7, cy - 3, 2.4), C(cx - 1, cy + 6, 2.6), C(cx + 7, cy + 9, 2.0),
                  C(cx - 10, cy + 4, 2.0), C(cx + 1, cy - 11, 2.0)], CHIP),
                ([C(cx - 8, cy - 11, 2.2)], mix(COOKIE, WHITE, 0.5))])


def hand_fish(img):
    blob(img, [T((58, 57), (65, 50), (67, 58))], mix(FISH, LINE, 0.2), r=0.6, line=0.8)
    blob(img, [E(45, 55, 75, 74), T((71, 64.5), (84, 54.5), (84, 74.5))], FISH, r=1.4,
         marks=[([E(48, 66, 72, 76)], FISH_BELLY),
                ([L([(77.5, 58), (81, 64.5), (77.5, 71)], 1.0)], mix(FISH, LINE, 0.25)),
                ([E(58, 56, 65, 60)], mix(FISH, WHITE, 0.35))])
    paint(img, aa(shapes([C(52.5, 63, 1.9)])), (84, 58, 56))
    paint(img, aa(shapes([C(52, 62.5, 0.6)])), WHITE)
    paint(img, aa(shapes([E(47.5, 66.5, 51.5, 69)])), (250, 180, 180))


def hand_carrot(img):
    blob(img, [L([(61, 53), (57, 47)], 3.0), L([(64, 52), (65, 45.5)], 3.2), L([(67, 53), (72, 47)], 3.0)], LEAF, r=0.6, line=0.85)
    blob(img, [T((53, 55), (75, 55), (65, 84)), E(53, 51, 75, 61)], CARROT, r=1.6,
         marks=[([L([(57, 63), (62, 63)], 1.2), L([(66, 70), (70, 70)], 1.2), L([(59, 76), (63, 76)], 1.2)], CARROT_DARK),
                ([E(56, 53, 63, 58)], mix(CARROT, WHITE, 0.4))])


def hand_fly(img):
    """개구리가 좋아하는 파리 — 무섭지 않게: 동글동글 몸, 반짝이는 큰 눈, 비스듬히 선 투명한 날개 둘, 작은 웃음."""
    # 날개 (몸 뒤에서 위로 비스듬히) — 연한 하늘색 캡슐에 반짝이
    for sgn in (-1, 1):
        blob(img, [L([(64 + 5 * sgn, 63), (64 + 14.5 * sgn, 55)], 8.5)], WING, r=0.4, line=0.6,
             marks=[([L([(64 + 11.5 * sgn, 55), (64 + 14.5 * sgn, 53.6)], 1.8)], WHITE),
                    ([L([(64 + 7.5 * sgn, 61.5), (64 + 13.5 * sgn, 57.5)], 0.6)], mix(WING, LINE, 0.3))])
    # 작은 다리 셋씩
    paint(img, aa(shapes([L([(58 + 6 * i, 78), (56 + 8 * i, 83)], 1.6) for i in range(3)])), FLY)
    # 동그란 몸 (배는 살짝 밝게)
    blob(img, [E(52, 57, 76, 80)], FLY, r=0.6, line=0.75,
         marks=[([E(54, 71, 74, 82)], mix(FLY, WHITE, 0.2)), ([E(55, 58, 61, 61.5)], mix(FLY, WHITE, 0.4))])
    # 커다란 눈 둘 (반짝이 두 점)
    for ex in (58.8, 69.2):
        blob(img, [C(ex, 65.5, 4.8)], WHITE, r=0, line=0.6,
             marks=[([C(ex + 0.4, 66.2, 3.3)], FLY_EYE), ([C(ex - 0.7, 64.6, 1.25)], WHITE), ([C(ex + 1.5, 67.8, 0.6)], WHITE)])
    # 작은 웃음 + 볼터치
    paint(img, aa(shapes([L(arc(64, 72.2, 2.0, 1.6, 20, 160), 0.9)])), WHITE)
    paint(img, aa(shapes([E(53, 72, 56.5, 74), E(71.5, 72, 75, 74)])), (250, 180, 180))

def hand_candy(img):
    """호박이 좋아하는 포장 사탕 — 동글 주황 사탕 + 양옆 보라 포장 리본 (주름 줄), 흰 줄무늬, 작은 얼굴."""
    for sgn in (-1, 1):
        x0 = 64 + 9 * sgn
        blob(img, [T((x0, 64), (64 + 20 * sgn, 54.5), (64 + 17 * sgn, 64), (64 + 20 * sgn, 73.5))], CANDY_WRAP, r=0.8, line=0.8,
             marks=[([L([(64 + 13 * sgn, 63), (64 + 18 * sgn, 58)], 1.0), L([(64 + 13 * sgn, 65), (64 + 18 * sgn, 70)], 1.0)],
                     mix(CANDY_WRAP, LINE, 0.3))])
    blob(img, [C(64, 64, 12)], CANDY, r=0, line=0.9,
         marks=[([L([(56, 70), (70, 56)], 2.6), L([(61, 75), (75, 61)], 2.2)], mix(CANDY, WHITE, 0.6)),
                ([E(56, 55, 62, 59)], mix(CANDY, WHITE, 0.75))])
    tiny_face(img, 64, 65, 0.85)


def hand_lollipop(img):
    """유령이 좋아하는 막대사탕 — 분홍·흰 소용돌이 동그라미 + 하얀 막대 + 작은 리본."""
    blob(img, [L([(64, 66), (64, 81.5)], 3.6)], STICK, r=0, line=0.8)
    spiral = [(64 + 0.62 * t * math.cos(t), 58 + 0.62 * t * math.sin(t)) for t in (k * 0.3 for k in range(3, 59))]
    blob(img, [C(64, 58, 12)], LOLLY, r=0, line=0.9,
         marks=[([L(spiral, 2.0)], mix(LOLLY, WHITE, 0.8)), ([E(55, 50, 60, 53.5)], WHITE)])
    blob(img, [T((64, 72), (57, 68.5), (57, 75.5)), T((64, 72), (71, 68.5), (71, 75.5)), C(64, 72, 2.2)], CANDY_WRAP, r=0, line=0.7)


APPLE, APPLE_STEM, APPLE_LEAF = (240, 78, 82), (128, 90, 62), (128, 196, 104)
GLASS, MILK = (214, 234, 248), (255, 255, 252)


def hand_apple(img):
    """루돌프가 좋아하는 빨간 사과 — 동글 하트꼴 사과 + 짧은 꼭지 + 잎 하나, 반짝이, 작은 얼굴."""
    blob(img, [L([(64, 53), (65.5, 46)], 2.6)], APPLE_STEM, r=0, line=0.8)
    blob(img, [E(66, 44, 79, 51)], APPLE_LEAF, r=0.3, line=0.8, marks=[([L([(68, 47.8), (76.5, 47)], 0.8)], mix(APPLE_LEAF, LINE, 0.3))])
    blob(img, [C(57.5, 66, 12), C(70.5, 66, 12), E(51, 64, 77, 83)], APPLE, r=1.2, line=0.9,
         marks=[([E(48, 72, 80, 87)], mix(APPLE, LINE, 0.18)), ([E(52, 57, 57.5, 63)], mix(APPLE, WHITE, 0.7))])
    tiny_face(img, 64, 69, 0.9)


def hand_milk(img):
    """산타가 좋아하는 우유 한 잔 + 기대 놓은 초코칩 쿠키."""
    glass = [T((47, 44), (69, 44), (67, 84), (49, 84))]
    blob(img, glass, GLASS, r=0.3, line=0.9,
         marks=[([T((47.6, 52), (68.4, 52), (67, 84), (49, 84))], MILK), ([E(48, 50, 68, 54.5)], mix(MILK, GLASS, 0.5)),
                ([L([(51.5, 57), (52.5, 79)], 1.8)], WHITE)])
    tiny_face(img, 58, 67, 0.8)
    blob(img, [C(74, 76, 8.5)], COOKIE, r=0.3, line=0.9,
         marks=[([C(71, 73, 1.7), C(77, 75, 1.5), C(73.5, 80, 1.6)], CHIP), ([C(70, 71.5, 1.3)], mix(COOKIE, WHITE, 0.5))])


def draw_hand(name):
    """손에 든 음식 하나를 HAND × HAND RGBA로."""
    img = new_canvas()
    with view(scale=1.0, lift=0):
        HAND_DRAW[name](img)
    return finish(img).crop(HAND_BOX)


def build_hand(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in HAND_NAMES:
        art = draw_hand(name)
        art.save(out_dir / f"{name}.png")
        arts[name] = art
    return arts


BREAD, BREAD_CRUST, BREAD_IN = (250, 214, 150), (214, 150, 84), (255, 244, 214)
LEAF_DARK = mix(LEAF, LINE, 0.25)
TWIG = (176, 128, 92)


def hand_bread(img):
    """오리가 좋아하는 식빵 한 조각 — 동글 머리 식빵, 노릇한 테두리, 폭신한 속, 작은 얼굴."""
    top = arc(64, 58, 16, 10, 180, 360, 16)
    slab = top + [(78, 83), (50, 83)]
    blob(img, [T(*slab), C(50.5, 60, 5), C(77.5, 60, 5)], BREAD_CRUST, r=1.6, line=0.9,
         marks=[([T(*[(64 + (x - 64) * 0.8, 69 + (y - 69) * 0.8) for x, y in slab])], BREAD_IN),
                ([E(54, 53, 61, 57)], mix(BREAD_CRUST, WHITE, 0.45))])
    tiny_face(img, 64, 70, 0.95)


def hand_leaf(img):
    """기린이 좋아하는 나뭇잎 — 작은 가지에 동글 잎 셋 (가운데 잎에 얼굴)."""
    blob(img, [L([(59, 81), (63, 72), (64, 60)], 2.8)], TWIG, r=0, line=0.8)
    for cx, cy, rot in ((54, 70, -40), (74, 68, 40)):
        pts = [(cx + 8 * math.cos(math.radians(a + rot)),
                cy + 8 * math.sin(math.radians(a + rot)) * 0.62) for a in range(0, 360, 20)]
        blob(img, [T(*pts)], LEAF, r=0.6, line=0.85,
             marks=[([L([(cx - 6 * math.cos(math.radians(rot)), cy - 6 * math.sin(math.radians(rot)) * 0.62),
                         (cx + 6 * math.cos(math.radians(rot)), cy + 6 * math.sin(math.radians(rot)) * 0.62)], 1.0)], LEAF_DARK)])
    blob(img, [E(53, 47, 75, 68), T((60, 49), (64, 43), (68, 49))], LEAF, r=1.0, line=0.9,
         marks=[([L([(64, 47), (64, 66)], 1.1)], LEAF_DARK), ([E(56.5, 50.5, 61, 55)], mix(LEAF, WHITE, 0.5))])
    tiny_face(img, 64, 58, 0.85)


MELON, MELON_PALE, MELON_RIND, MELON_RIND_DARK, SEED = (250, 104, 118), (240, 252, 214), (112, 192, 94), (78, 150, 74), (70, 50, 52)
BROC, BROC_DARK, BROC_STEM = (118, 190, 96), (84, 152, 74), (190, 224, 152)
GRAPE, GRAPE_DARK, GRAPE_LIGHT = (164, 112, 204), (126, 82, 168), (214, 186, 238)


def hand_watermelon(img):
    """노란 공룡이 좋아하는 수박 한 조각 — 부채꼴 빨간 속(뾰족한 쪽이 위), 연두 속껍질, 초록 줄무늬 껍질, 까만 씨, 작은 얼굴."""
    cx, cy = 64, 50
    fan = lambda r: [(cx, cy)] + arc(cx, cy, r, r, 47, 133, 16)
    blob(img, [T(*fan(27))], MELON_RIND, r=1.2, line=0.9,
         marks=[([T(*fan(24))], MELON_PALE), ([T(*fan(22))], MELON),
                ([L(arc(cx, cy, 25.8, 25.8, a, a + 11, 4), 1.4) for a in (52, 72, 97, 117)], MELON_RIND_DARK),
                ([E(57, 70, 59, 73.5), E(69, 70, 71, 73.5), E(63, 67, 65, 70.5)], SEED)])
    tiny_face(img, 64, 62, 0.75)


def hand_broccoli(img):
    """초록 공룡이 좋아하는 브로콜리 — 동글 송이 셋 + 연두 줄기, 작은 얼굴."""
    with xf(sx=0.84, sy=0.84, pivot=(64, 64)):
        blob(img, [T((59, 66), (69, 66), (68, 85), (60, 85))], BROC_STEM, r=1.0, line=0.9,
             marks=[([L([(64, 70), (64, 82)], 1.0)], mix(BROC_STEM, LINE, 0.25))])
        blob(img, [C(53.5, 61, 9), C(74.5, 61, 9), C(64, 53, 10.5), E(52, 56, 76, 70)], BROC, r=1.4, line=0.9,
             marks=[([C(51, 64, 2.2), C(77, 64, 2.2), C(58, 48, 2.0), C(71, 49, 2.3), C(64, 66, 2.0)], BROC_DARK),
                    ([E(57, 46, 63, 51)], mix(BROC, WHITE, 0.45))])
        tiny_face(img, 64, 59, 0.85)


def hand_grapes(img):
    """보라 공룡이 좋아하는 포도 한 송이 — 동글 알 여러 개 + 꼭지와 잎, 큰 알에 작은 얼굴."""
    with xf(sx=0.78, sy=0.78, pivot=(64, 63)):
        blob(img, [L([(64, 50), (66, 43)], 2.4)], (150, 112, 82), r=0, line=0.8)
        blob(img, [E(66, 41, 79, 48)], (128, 196, 104), r=0.3, line=0.8)
        for x, y in ((55, 56), (73, 56), (50, 66), (78, 66), (57, 76), (71, 76), (64, 85)):
            blob(img, [C(x, y, 6.2)], GRAPE, r=0, line=0.8, marks=[([C(x - 2, y - 2, 1.5)], GRAPE_LIGHT)])
        blob(img, [C(64, 64, 9)], GRAPE, r=0, line=0.85, marks=[([C(64, 68, 7)], mix(GRAPE, GRAPE_DARK, 0.4)), ([C(60, 59.5, 2.0)], GRAPE_LIGHT)])
        tiny_face(img, 64, 65, 0.8)


ICE, ICE_SHADE, ICE_SYRUP, ICE_CUP = (246, 251, 255), (214, 228, 246), (250, 140, 170), (252, 208, 116)


def hand_shaved_ice(img):
    """눈사람이 좋아하는 빙수 🍧 — 소복한 얼음 산 + 분홍 시럽 + 노란 컵."""
    with xf(dy=-2.5, sx=0.92, sy=0.92, pivot=(64, 66)):
        blob(img, [C(64, 58, 13), E(47, 56, 81, 72)], ICE, r=1.0, line=0.9,
             marks=[([E(44, 64, 84, 76)], ICE_SHADE), ([C(64, 50, 8.5), E(55, 50, 72, 61)], ICE_SYRUP), ([E(58, 47, 63, 51)], WHITE)])
        blob(img, [T((48, 70), (80, 70), (73, 87), (55, 87))], ICE_CUP, r=0.6, line=0.9,
             marks=[([E(44, 82, 84, 92)], mix(ICE_CUP, LINE, 0.16)), ([L([(48, 72), (80, 72)], 2.0)], mix(ICE_CUP, WHITE, 0.5))])
        tiny_face(img, 64, 77, 0.8)


GRAIN, GRAIN_DARK = (240, 206, 120), (196, 150, 78)
SUNSEED, SUNSEED_STRIPE = (120, 110, 118), (236, 232, 226)


def hand_seeds(img):
    """새가 좋아하는 모이 🌾 — 이삭 하나 (낟알 여러 개) 앞에 해바라기씨 둘, 가운데 통통한 씨앗에 작은 얼굴."""
    with xf(dx=-1, dy=-1, sx=0.82, sy=0.82, pivot=(64, 66)):
        _seeds(img)


def _seeds(img):
    blob(img, [L([(70, 86), (72, 66), (78, 46)], 2.2)], GRAIN_DARK, r=0, line=0.8)
    for k, (x, y) in enumerate([(78, 46), (74, 50), (81, 52), (76, 56), (83, 58), (73, 61)]):
        with xf(rot=-20 + 12 * (k % 2), pivot=(x, y)):
            blob(img, [E(x - 3, y - 4.6, x + 3, y + 4.6)], GRAIN, r=0, line=0.75)
    for x, y, rot in ((50, 74, -35), (78, 78, 30)):
        with xf(rot=rot, pivot=(x, y)):
            blob(img, [E(x - 4.2, y - 7.5, x + 4.2, y + 7.5)], SUNSEED, r=0, line=0.8,
                 marks=[([L([(x, y - 6), (x, y + 6)], 1.4)], SUNSEED_STRIPE)])
    blob(img, [E(54, 58, 72, 84), T((58, 62), (63, 52), (68, 62))], GRAIN, r=1.2, line=0.9,
         marks=[([E(56, 76, 72, 88)], mix(GRAIN, LINE, 0.18)), ([E(57, 61, 62, 67)], mix(GRAIN, WHITE, 0.55))])
    tiny_face(img, 63, 71, 0.8)


SHRIMP, SHRIMP_DARK, SHRIMP_LIGHT = (255, 146, 112), (226, 98, 82), (255, 206, 186)


def hand_shrimp(img):
    """펭귄이 좋아하는 새우 🦐 — C자로 꼬부린 통통한 몸 (마디 줄) + 꼬리 부채 + 더듬이, 머리에 작은 얼굴."""
    with xf(dx=2, dy=2, sx=0.7, sy=0.7, pivot=(64, 66)):
        _shrimp(img)


def _shrimp(img):
    body = arc(62, 66, 15, 14, -160, 100, 18)  # 머리(왼쪽 위) → 오른쪽으로 휘어 → 꼬리(아래)
    tail = body[-1]
    blob(img, [L([(tail[0] - 1, tail[1] - 2), (tail[0] - 10, tail[1] + 2)], 2.0), T((tail[0], tail[1]), (tail[0] - 14, tail[1] - 2), (tail[0] - 9, tail[1] + 9))],
         SHRIMP_DARK, r=0.6, line=0.8)
    blob(img, [L([(52, 56), (40, 40)], 1.4), L([(55, 54), (50, 36)], 1.4)], SHRIMP_DARK, r=0, line=0.6)
    fns = [C(x, y, 10.5 - 6.5 * k / (len(body) - 1)) for k, (x, y) in enumerate(body)]
    m = blob(img, fns, SHRIMP, r=1.4, line=0.9, marks=[([E(50, 70, 82, 86)], mix(SHRIMP, LINE, 0.12)), ([E(56, 50, 66, 56)], SHRIMP_LIGHT)])
    for k in (6, 9, 12, 15):
        x, y = body[k]
        dx, dy = x - 62, y - 66
        n = math.hypot(dx, dy) or 1
        paint(img, aa(shapes([L([(x - dx / n * 7, y - dy / n * 7), (x + dx / n * 7, y + dy / n * 7)], 1.2)])), SHRIMP_DARK, clip=m)
    tiny_face(img, 54, 62, 0.8)


BAMBOO_STALK, BAMBOO_NODE, BAMBOO_LEAF, BAMBOO_LEAF_DARK = (186, 214, 110), (130, 168, 74), (124, 196, 98), (78, 146, 72)


def hand_bamboo(img):
    """레서판다가 좋아하는 대나무잎 🎋 — 비스듬한 대나무 한 마디 (마디 띠) 끝에 길쭉한 잎 셋이 부채처럼, 줄기에 작은 얼굴."""
    with xf(dx=2, dy=-1, sx=0.58, sy=0.58, pivot=(64, 64)):
        for tip in ((40, 42), (60, 32), (86, 42)):
            base = (66, 54)
            mx, my = (tip[0] + base[0]) / 2, (tip[1] + base[1]) / 2
            nx, ny = -(tip[1] - base[1]) * 0.24, (tip[0] - base[0]) * 0.24
            blob(img, [T(base, (mx + nx, my + ny), tip, (mx - nx, my - ny))], BAMBOO_LEAF, r=0.8, line=0.8,
                 marks=[([L([base, tip], 1.0)], BAMBOO_LEAF_DARK)])
        m = blob(img, [L([(56, 90), (68, 52)], 13)], BAMBOO_STALK, r=0.4, line=0.9,
                 marks=[([L([(62, 90), (73, 54)], 3.0)], mix(BAMBOO_STALK, LINE, 0.1)), ([L([(52, 84), (60, 58)], 2.2)], mix(BAMBOO_STALK, WHITE, 0.5))])
        paint(img, aa(shapes([L([(52, 63), (70, 69)], 2.0), L([(48, 83), (64, 88)], 2.0)])), BAMBOO_NODE, clip=m)
        tiny_face(img, 61, 75, 0.8)


ACORN, ACORN_DARK, ACORN_LIGHT = (220, 156, 88), (180, 116, 62), (248, 210, 160)
ACORN_CAP, ACORN_CAP_DARK = (150, 100, 62), (112, 72, 48)


def hand_acorn(img):
    """다람쥐가 좋아하는 도토리 🌰 — 통통한 알 + 비늘 무늬 깍정이 모자 + 꼭지, 알에 작은 얼굴."""
    with xf(dx=0, dy=1, sx=0.64, sy=0.64, pivot=(64, 66)):
        blob(img, [E(48, 56, 80, 88), T((52, 78), (76, 78), (64, 96))], ACORN, r=1.4, line=0.9,
             marks=[([E(52, 62, 60, 76)], ACORN_LIGHT), ([E(46, 82, 82, 100)], ACORN_DARK)])
        m = blob(img, [E(44, 44, 84, 66), L([(64, 46), (68, 34)], 3.6)], ACORN_CAP, r=0.8, line=0.9)
        paint(img, aa(shapes([L(p, 1.1) for p in ([(46, 52), (82, 52)], [(46, 58), (82, 58)], [(54, 44), (58, 66)], [(64, 44), (64, 66)],
                                                  [(74, 44), (70, 66)])])), ACORN_CAP_DARK, clip=m)
        tiny_face(img, 64, 76, 0.85)


BENTO, BENTO_RIM, BENTO_IN = (222, 92, 86), (176, 62, 66), (255, 244, 224)
UME, EGG, GREEN, SAUSAGE = (232, 80, 96), (255, 214, 96), (122, 190, 104), (246, 138, 138)


def hand_special(img):
    """🍱 특식 — 빨간 도시락: 왼쪽 주먹밥 칸(매실 하나), 오른쪽 달걀말이·브로콜리·문어 소시지."""
    with xf(sx=0.92, sy=0.92, pivot=(64, 65)):
        blob(img, [T((43, 47), (85, 47), (85, 83), (43, 83))], BENTO, r=3.5, line=1.0,
             marks=[([E(36, 78, 92, 92)], BENTO_RIM), ([L([(46, 50), (82, 50)], 1.6)], mix(BENTO, WHITE, 0.45))])
        # 칸: 밥 / 반찬
        blob(img, [T((47, 53), (65, 53), (65, 79), (47, 79))], RICE, r=1.2, line=0.8)
        paint(img, aa(shapes([C(56, 60, 3.2)])), UME)
        paint(img, aa(shapes([C(55, 59, 0.9)])), WHITE)
        tiny_face(img, 56, 70, 0.62)
        blob(img, [T((68, 53), (81, 53), (81, 79), (68, 79))], BENTO_IN, r=1.2, line=0.8)
        blob(img, [T((70, 55), (79, 55), (79, 62), (70, 62))], EGG, r=0.8, line=0.6, marks=[([L([(70, 58.5), (79, 58.5)], 0.8)], mix(EGG, CARROT, 0.5))])
        blob(img, [C(72, 67, 2.6), C(76.5, 66, 2.6), C(75, 69, 2.6)], GREEN, r=0.4, line=0.6)
        blob(img, [T((70, 72), (79, 72), (79, 77), (70, 77))], SAUSAGE, r=1.0, line=0.6, marks=[([L([(73, 72), (73, 77)], 0.7), L([(76, 72), (76, 77)], 0.7)], mix(SAUSAGE, LINE, 0.3))])


def hand_broom(img):
    """🧹 쓰는 끝이 왼쪽 아래 (커서 자리)."""
    STRAW = (240, 196, 92)
    paint(img, aa(shapes([L([(82, 45), (61, 67)], 5.0)])), LINE)
    paint(img, aa(shapes([L([(82, 45), (61, 67)], 3.0)])), (170, 110, 72))
    blob(img, [T((56, 60), (68, 71), (60, 84), (45, 79))], STRAW, r=1.5, line=1.0,
         marks=[([L([(56, 71), (51, 79)], 1.2), L([(61, 74), (57, 82)], 1.2)], mix(STRAW, LINE, 0.35)),
                ([L([(56, 62), (66, 71)], 3.0)], (214, 80, 70))])


# ---- 계절 날씨: 눈덩이(굴리는 중) · 눈사람 (바닥 가운데 아래에 앉는다, src-tauri/src/weather.rs) ----
SNOWMAN_W, SNOWMAN_H = 64, 80
SNOWBALL = 40
SNOW = (252, 253, 255)
SNOW_SHADE = (214, 228, 244)
SCARF, SCARF_DARK = (232, 92, 96), (196, 66, 76)
COAL = (70, 62, 72)


def seasonal_box(w, h):
    return ((FRAME - w) // 2, FRAME - h, (FRAME + w) // 2, FRAME)


def draw_snowball(img):
    """굴리는 눈덩이 — 동글, 아래쪽 푸른 그늘, 반짝이 하나."""
    cx, cy, r = 64, BOTTOM - 15, 15
    blob(img, [C(cx, cy, r)], SNOW, r=0.4, line=0.9,
         marks=[([E(cx - r + 3, cy + 3, cx + r - 3, cy + r + 2)], SNOW_SHADE),
                ([C(cx - 7, cy + 6, 1.4), C(cx + 6, cy - 1, 1.2), C(cx + 2, cy + 9, 1.1)], SNOW_SHADE)])


def draw_snowman(img):
    """작은 눈사람 — 두 덩이 몸, 나뭇가지 팔, 빨간 털모자와 목도리, 당근 코, 볼터치."""
    # 팔 (몸 뒤)
    blob(img, [L([(52, 92), (37, 80)], 2.6), L([(41, 84), (37, 89)], 2.0)], TWIG, r=0, line=0.75)
    blob(img, [L([(76, 92), (91, 80)], 2.6), L([(87, 84), (91, 89)], 2.0)], TWIG, r=0, line=0.75)
    base_y, base_r = BOTTOM - 20, 20
    blob(img, [C(64, base_y, base_r), C(64, 79, 15.5)], SNOW, r=1.0, line=0.95,
         marks=[([E(47, base_y + 5, 81, base_y + base_r + 2)], SNOW_SHADE),
                ([C(64, 99, 1.9), C(64, 108, 1.9)], COAL)])
    # 목도리 + 늘어진 자락
    blob(img, [L([(52.5, 91), (75.5, 91)], 5.4)], SCARF, r=0.4, line=0.75)
    blob(img, [T((68, 92), (75, 92), (76, 103), (69, 103))], SCARF, r=0.5, line=0.75,
         marks=[([L([(70, 99.5), (75, 99.5)], 1.0)], SCARF_DARK)])
    # 얼굴
    paint(img, aa(shapes([C(58.5, 80, 2.1), C(69.5, 80, 2.1)])), COAL)
    paint(img, aa(shapes([C(57.9, 79.3, 0.7), C(68.9, 79.3, 0.7)])), WHITE)
    paint(img, aa(shapes([E(52.5, 83, 56.5, 85.4), E(71.5, 83, 75.5, 85.4)])), (250, 180, 180))
    blob(img, [T((62.5, 82.2), (62.5, 86.2), (74, 84.6))], CARROT, r=0.4, line=0.6)
    # 털모자 + 방울
    blob(img, [E(52, 54, 76, 71)], SCARF, r=0.6, line=0.85,
         marks=[([E(55.5, 56, 62, 60)], mix(SCARF, WHITE, 0.4))])
    blob(img, [L([(51.5, 69), (76.5, 69)], 4.4)], mix(SCARF, WHITE, 0.72), r=0.3, line=0.75)
    blob(img, [C(64, 52.5, 3.6)], SNOW, r=0, line=0.75)


SEASONAL = {"snowball": (draw_snowball, SNOWBALL, SNOWBALL), "snowman": (draw_snowman, SNOWMAN_W, SNOWMAN_H)}


def draw_seasonal(name):
    fn, w, h = SEASONAL[name]
    img = new_canvas()
    with view(scale=1.0, lift=0):
        fn(img)
    return finish(img).crop(seasonal_box(w, h))


def build_seasonal(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in SEASONAL:
        arts[name] = draw_seasonal(name)
        arts[name].save(out_dir / f"{name}.png")
    return arts


def preview_seasonal(arts):
    """눈 위의 128px 고양이 옆에 놓아 본다 (밝은 바탕·어두운 바탕, 2배)."""
    sheet = ROOT / "ui" / "pets" / "cat" / "sheet_teenager.png"
    pad = 8
    w = FRAME + sum(a.width + pad for a in arts.values()) + pad
    im = Image.new("RGBA", (w, 2 * FRAME), (246, 242, 236, 255))
    im.paste((58, 66, 88, 255), (0, FRAME, w, 2 * FRAME))
    for row in (0, FRAME):
        if sheet.is_file():
            im.alpha_composite(Image.open(sheet).convert("RGBA").crop((0, 0, FRAME, FRAME)), (0, row))
        x = FRAME
        for art in arts.values():
            im.alpha_composite(art, (x, row + FRAME - art.height))
            x += art.width + pad
    PREVIEW.parent.mkdir(parents=True, exist_ok=True)
    im.resize((im.width * 2, im.height * 2), Image.NEAREST).save(PREVIEW.with_name("snowman.png"))


# 손에 든 음식 (앱 메뉴 순서와 같다)
HAND_DRAW = {"hand_meal": hand_meal, "hand_cookie": hand_cookie, "hand_fish": hand_fish,
             "hand_carrot": hand_carrot, "hand_fly": hand_fly,
             "hand_candy": hand_candy, "hand_lollipop": hand_lollipop,
             "hand_acorn": hand_acorn,  # 다람쥐 🌰
             "hand_apple": hand_apple, "hand_milk": hand_milk,  # 크리스마스: 루돌프 🍎 · 산타 🥛
             "hand_bread": hand_bread, "hand_leaf": hand_leaf,  # 농장: 오리 🍞 · 기린 🍃
             "hand_watermelon": hand_watermelon, "hand_broccoli": hand_broccoli, "hand_grapes": hand_grapes,  # 공룡: 노랑 🍉 · 초록 🥦 · 보라 🍇
             "hand_shaved_ice": hand_shaved_ice,  # 눈사람 🍧
             "hand_seeds": hand_seeds,  # 새 🌾
             "hand_shrimp": hand_shrimp,  # 펭귄 🦐
             "hand_bamboo": hand_bamboo,  # 레서판다 🎋
             "hand_special": hand_special,  # 특식 🍱
             "hand_broom": hand_broom}  # 🧹 빗자루 (음식은 아니지만 커서를 따라다닌다)
HAND_NAMES = tuple(HAND_DRAW)


DRAW = {"bowl_food": draw_bowl_food, "bowl_water": draw_bowl_water, "cookie": draw_cookie}


def draw(name):
    """소품 하나를 WIDTH × HEIGHT RGBA로."""
    img = new_canvas()
    with view(scale=1.0, lift=0):
        DRAW[name](img)
    return finish(img).crop(BOX)


def build(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in NAMES:
        art = draw(name)
        art.save(out_dir / f"{name}.png")
        arts[name] = art
    return arts


def preview(arts):
    """teenager 고양이 eat 프레임 옆에 소품을 놓아 본다 (밝은 바탕, 2배)."""
    sheet = ROOT / "ui" / "pets" / "cat" / "sheet_teenager.png"
    pad = 8
    w = FRAME + len(arts) * (WIDTH + pad) + pad
    im = Image.new("RGBA", (w, FRAME), (246, 242, 236, 255))
    if sheet.is_file():
        im.alpha_composite(Image.open(sheet).convert("RGBA").crop((0, 0, FRAME, FRAME)), (0, 0))
    x = FRAME
    for art in arts.values():
        im.alpha_composite(art, (x, FRAME - HEIGHT))
        x += WIDTH + pad
    PREVIEW.parent.mkdir(parents=True, exist_ok=True)
    im.resize((im.width * 2, im.height * 2), Image.NEAREST).save(PREVIEW)


def preview_hand(arts):
    """손에 든 음식들 (밝은 바탕, 3배)."""
    pad = 6
    im = Image.new("RGBA", (len(arts) * (HAND + pad) + pad, HAND + 2 * pad), (246, 242, 236, 255))
    for i, art in enumerate(arts.values()):
        im.alpha_composite(art, (pad + i * (HAND + pad), pad))
    im.resize((im.width * 3, im.height * 3), Image.NEAREST).save(PREVIEW.with_name("hand_food.png"))


def main():
    arts = build()
    preview(arts)
    preview_hand(build_hand())
    preview_seasonal(build_seasonal())
    print("ok: props")


if __name__ == "__main__":
    main()
