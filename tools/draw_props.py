"""먹이 소품 — draw_pets.py 엔진(슈퍼샘플 마스크·melt·외곽선·paint)으로 그린 작은 그림, draw_house.py와 같은 말랑한 외곽선.

    python tools/draw_props.py  → ui/effects/{bowl_food,bowl_water,cookie,flower}.png
                                  + tools/preview/props.png (128px 고양이 옆에 놓아 본 모습)

앱(src-tauri/src/prop.rs)은 동물이 먹을 때 몸 앞 바닥에 WIDTH × HEIGHT(논리 px, 몸 배율 1 기준) 투명 창을 띄우고
그림을 아래 가운데에 붙여 보인다 (ui/prop.html). 쿠키는 입가에, 꽃은 꿀벌이 내려앉는 자리에 핀다.
그림은 모두 같은 상자(WIDTH × HEIGHT) — 바닥은 아래 끝 1px 투명 여백 위에 앉는다.
"""

import math
import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish, paw, tuft  # noqa: E402
from draw_pets import LINE, S, WHITE, C, E, L, T, aa, arc, mix, paint, shapes, view  # noqa: E402
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
KIBBLE_DARK = mix(KIBBLE, LINE, 0.4)
RICE = (255, 252, 244)
WATER = (188, 226, 250)
WATER_DEEP = (150, 204, 240)
COOKIE = (232, 184, 116)
COOKIE_DARK = mix(COOKIE, LINE, 0.3)
CHIP = (110, 70, 58)
PETAL = (248, 150, 176)
PETAL_LIGHT = mix(PETAL, WHITE, 0.4)
CENTER = (255, 214, 102)
LEAF = (150, 204, 124)

NAMES = ("bowl_food", "bowl_water", "cookie", "flower")


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


def draw_flower(img):
    cx, top = 64, 99
    blob(img, tuft(cx - 9, BOTTOM - 0.5, 9, -1) + tuft(cx + 9, BOTTOM - 0.5, 8, 1), LEAF, r=0.6, line=0.8)
    blob(img, [L([(cx, top + 6), (cx + 0.5, BOTTOM - 1)], 2.0)], LEAF, r=0, line=0.7)
    blob(img, [E(cx + 1, 112, cx + 11, 117.5), E(cx - 10, 108, cx - 1, 113.5)], LEAF, r=0.4, line=0.7)
    r = 11
    petals = [C(cx + r * 0.62 * math.cos(math.radians(-90 + 72 * i)), top + r * 0.62 * math.sin(math.radians(-90 + 72 * i)),
                r * 0.5) for i in range(5)]
    blob(img, petals, PETAL, r=0.3, line=0.8,
         marks=[([C(cx + r * 0.62 * math.cos(math.radians(-90 + 72 * i)) - 1.2,
                    top + r * 0.62 * math.sin(math.radians(-90 + 72 * i)) - 1.2, 1.8) for i in range(5)], PETAL_LIGHT)])
    blob(img, [C(cx, top, r * 0.36)], CENTER, r=0, line=0.6, marks=[([C(cx - 1.3, top - 1.3, 1.2)], WHITE)])


DRAW = {"bowl_food": draw_bowl_food, "bowl_water": draw_bowl_water, "cookie": draw_cookie, "flower": draw_flower}


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


def main():
    arts = build()
    preview(arts)
    print("ok: props")


if __name__ == "__main__":
    main()
