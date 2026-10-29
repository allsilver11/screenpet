"""카와이 펫 집 — draw_pets.py 엔진(슈퍼샘플 마스크·melt·외곽선·paint)으로 그린 128px 아늑한 오두막.

    python tools/draw_house.py  → ui/house/{house.png, house_front.png, house.json} + tools/preview/house.png

앱의 겹치기 순서: house.png → 문 사각형(house.json "door")으로 잘라낸 펫 → house_front.png
house_front.png는 펫 앞에 와야 하는 부분(문틀·아치 위 벽 모서리·문턱)만 담고, 문 안쪽은 투명하다.
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageChops

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_pets import (CREAM, LINE, OUTLINE, PINK, S, WHITE, C, E, L, T, aa, blank, grow, melt, mix, paint, shapes,  # noqa: E402
                       view)
from sprite_spec import FRAME  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "house"
PREVIEW = ROOT / "tools" / "preview" / "house.png"

# ---- 색 ----
WALL = (255, 238, 212)
PLANK = mix(WALL, LINE, 0.18)
ROOF = (204, 108, 92)
ROOF_DARK = mix(ROOF, LINE, 0.35)
ROOF_LIGHT = mix(ROOF, WHITE, 0.25)
CHIMNEY = (222, 150, 124)
WOOD = (214, 160, 112)
WOOD_DARK = mix(WOOD, LINE, 0.35)
WOOD_LIGHT = mix(WOOD, WHITE, 0.3)
INSIDE = (92, 64, 62)
INSIDE_FLOOR = (118, 84, 76)
GLASS = (206, 230, 244)
CURTAIN = PINK
SMOKE = (246, 246, 250)
GRASS = (150, 204, 124)
GRASS_DARK = mix(GRASS, LINE, 0.3)

# ---- 문 (128 프레임 좌표). 위는 반원 아치, 아래는 집 바닥 ----
DOOR = dict(x=30, y=60, w=52, h=62)
FLOOR = DOOR["y"] + DOOR["h"]  # 122


def arch(x0, y0, x1, y1):
    """아치 모양 (위 반원 + 아래 사각)."""
    r = (x1 - x0) / 2
    return [E(x0, y0, x1, y0 + 2 * r), T((x0, y0 + r), (x1, y0 + r), (x1, y1), (x0, y1))]


def rrect(x0, y0, x1, y1, r):
    """둥근 사각형."""
    return [E(x0, y0, x0 + 2 * r, y0 + 2 * r), E(x1 - 2 * r, y0, x1, y0 + 2 * r), E(x0, y1 - 2 * r, x0 + 2 * r, y1),
            E(x1 - 2 * r, y1 - 2 * r, x1, y1), T((x0 + r, y0), (x1 - r, y0), (x1 - r, y1), (x0 + r, y1)),
            T((x0, y0 + r), (x1, y0 + r), (x1, y1 - r), (x0, y1 - r))]


def blob(img, fns, color, r=2.4, line=1.0, line_color=LINE, marks=()):
    """도형들을 녹여(r) 외곽선을 두르고 칠한다. marks는 이 조각 안으로 잘린다. 슈퍼샘플 마스크를 돌려준다."""
    m = shapes(fns)
    if r:
        m = melt(m, r)
    if line:
        paint(img, aa(grow(m, OUTLINE * line)), line_color)
    paint(img, aa(m), color)
    for mfns, mcolor in marks:
        paint(img, aa(shapes(mfns)), mcolor, clip=m)
    return m


def finish(img):
    """draw_pets.compose와 같은 마무리: 128로 줄이고 거의 투명한 찌꺼기를 지운다."""
    out = img.resize((FRAME, FRAME), Image.LANCZOS)
    out.putalpha(out.getchannel("A").point(lambda v: 0 if v < 10 else v))
    return out


def paw(cx, cy, s=1.0):
    """발바닥 도장: 큰 패드 + 발가락 셋."""
    return [E(cx - 3.2 * s, cy - 0.6 * s, cx + 3.2 * s, cy + 3.6 * s),
            C(cx - 3.6 * s, cy - 2.6 * s, 1.4 * s), C(cx, cy - 4.0 * s, 1.5 * s), C(cx + 3.6 * s, cy - 2.6 * s, 1.4 * s)]


def tuft(cx, base, h=8, lean=0):
    """풀 한 포기: 끝이 둥근 잎 세 가닥."""
    return [L([(cx, base), (cx - 3.5 + lean, base - h * 0.7)], 2.6), L([(cx, base), (cx + lean, base - h)], 2.8),
            L([(cx, base), (cx + 3.5 + lean, base - h * 0.75)], 2.6), E(cx - 4.5, base - 3, cx + 4.5, base + 1.5)]


def draw():
    """(house 슈퍼샘플 캔버스, 문 안쪽 마스크, 문턱 마스크)."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOOR
    x0, y0, x1, y1 = d["x"], d["y"], d["x"] + d["w"], FLOOR
    with view(scale=1.0, lift=0):
        # 굴뚝 (지붕 뒤) + 연기 한 모금
        blob(img, [T((95, 17), (106, 17), (106, 40), (95, 40))], CHIMNEY, r=1.0,
             marks=[([L([(95, 24), (106, 24)], 1.0), L([(100.5, 17), (100.5, 24)], 1.0)], mix(CHIMNEY, LINE, 0.3))])
        blob(img, rrect(92.5, 12, 108.5, 18.5, 2.5), mix(CHIMNEY, LINE, 0.15), r=0.8)
        blob(img, [C(101, 7.5, 3.2), C(107.5, 5.6, 2.4), C(96, 8.2, 2.0)], SMOKE, r=0.8, line=0.75,
             line_color=mix(LINE, SMOKE, 0.45))

        # 벽 (가로 판자 무늬)
        planks = [L([(10, y), (118, y)], 1.0) for y in (66, 78, 90, 102, 114)]
        blob(img, rrect(12, 40, 116, 124, 7), WALL, r=1.5, marks=[(planks, PLANK)])

        # 지붕 — 둥근 박공 + 비늘 기와
        roof = [T((4, 50), (44, 12), (84, 12), (124, 50), (124, 56), (4, 56)), E(34, 5, 94, 36)]
        tiles = []
        for row, (y, xs) in enumerate([(51, range(10, 124, 12)), (40, range(22, 112, 12)), (29, range(34, 100, 12)),
                                       (18, range(46, 88, 12))]):
            tiles += [E(x - 6 + (6 if row % 2 else 0), y - 5, x + 6 + (6 if row % 2 else 0), y + 5) for x in xs]
        blob(img, roof, ROOF, r=4.0,
             marks=[([L([(8, 55.5), (120, 55.5)], 3.0)], ROOF_DARK), (tiles, mix(ROOF, LINE, 0.22)),
                    ([L([(40, 15), (56, 9)], 2.2)], ROOF_LIGHT)])
        # 기와 윗선만 남기도록 다시 바탕색으로 살짝 덮는다
        with view(scale=1.0, lift=1.2):
            paint(img, aa(shapes(tiles)), ROOF, clip=melt(shapes(roof), 4.0))

        # 열린 문짝 (왼쪽 경첩, 바깥으로 젖혀져 비스듬히 보인다)
        blob(img, [T((15, 66), (26, 71), (26, 121), (15, 123))], WOOD, r=0.8,
             marks=[([L([(20.5, 72), (20.5, 119)], 1.0)], WOOD_DARK)])
        blob(img, [C(23.5, 97, 1.5)], (236, 196, 112), r=0, line=0.5)

        # 문틀 (아치 테두리) → 열린 문 안쪽 (어두운 집 안 + 은은한 바닥)
        blob(img, arch(x0 - 4.5, y0 - 4.5, x1 + 4.5, y1 + 1), WOOD, r=0,
             marks=[([L([(x0 - 2.2, y0 + d["w"] / 2), (x0 - 2.2, y1)], 1.0),
                      L([(x1 + 2.2, y0 + d["w"] / 2), (x1 + 2.2, y1)], 1.0)], WOOD_LIGHT)])
        opening = blob(img, arch(x0, y0, x1, y1), INSIDE, r=0, line=0.7,
                       marks=[([E(x0 - 6, y1 - 8, x1 + 6, y1 + 8)], INSIDE_FLOOR),
                              ([E(x0 + 6, y0 + 3, x1 - 6, y0 + 22)], mix(INSIDE, (0, 0, 0), 0.15))])

        # 문 위 발바닥 간판 (지붕 처마에 매달림)
        blob(img, [L([(50, 50), (52, 45)], 1.0), L([(62, 50), (60, 45)], 1.0)], WOOD_DARK, r=0, line=0)
        blob(img, rrect(45, 48.5, 67, 58.5, 3.5), WOOD_LIGHT, r=0.6, line=0.85)
        paint(img, aa(shapes(paw(56, 54.2, 0.95))), (236, 136, 142))

        # 동그란 창문: 나무 테 → 유리 + 커튼 + 십자 창살
        blob(img, [C(101, 82, 12)], WOOD, r=0, line=0.9)
        blob(img, [C(101, 82, 8.6)], GLASS, r=0, line=0.6,
             marks=[([L([(101, 73), (101, 91)], 1.4), L([(92, 82), (110, 82)], 1.4)], WOOD),
                    ([T((92, 72), (99, 72), (95, 82), (91, 93)), E(90, 72, 97, 92)], CURTAIN),
                    ([T((103, 72), (110, 72), (111, 93), (107, 82)), E(105, 72, 112, 92)], CURTAIN),
                    ([L([(93.5, 75), (96, 88)], 0.8), L([(108.5, 75), (106, 88)], 0.8)], mix(CURTAIN, LINE, 0.25)),
                    ([L([(103, 76), (106, 75)], 1.0)], WHITE)])
        blob(img, rrect(88, 94, 114, 98.5, 2), WOOD, r=0, line=0.75)  # 창턱

        # 문턱 (펫 앞에 온다)
        sill = blob(img, rrect(x0 - 7, y1 - 2.5, x1 + 7, y1 + 3.5, 2.8), WOOD_LIGHT, r=0, line=0.9,
                    marks=[([L([(x0 - 4, y1 - 1), (x1 + 4, y1 - 1)], 1.0)], WHITE)])

        # 풀 포기 (바닥 가장자리)
        grass = [tuft(9, 125, 9, -1), tuft(17, 126, 7), tuft(110, 126, 8), tuft(119, 125, 9, 1), tuft(95, 126, 6.5)]
        blob(img, [f for t in grass for f in t], GRASS, r=0.6, line=0.85,
             marks=[([L([(9, 124), (8, 119)], 0.8), L([(119, 124), (120, 119)], 0.8)], GRASS_DARK)])
        sill_full = grow(sill, OUTLINE * 0.9)
    return img, opening, sill_full


def front_layer(img, opening, sill):
    """펫 앞에 올 부분만: 문 주변(문틀·아치 위 벽 모서리)에서 문 안쪽을 뺀 것 + 문턱."""
    d = DOOR
    pad = 8
    region = blank()
    with view(scale=1.0, lift=0):
        region = shapes([T((d["x"] - pad, d["y"] - pad), (d["x"] + d["w"] + pad, d["y"] - pad),
                           (d["x"] + d["w"] + pad, FLOOR + 6), (d["x"] - pad, FLOOR + 6))])
    keep = ImageChops.lighter(ImageChops.subtract(region, opening), aa(sill))
    out = img.copy()
    out.putalpha(ImageChops.multiply(img.getchannel("A"), keep))
    return out


def pet_in_door(pet, door=DOOR):
    """앱과 같은 방식: 펫 프레임을 문 너비에 맞춰 줄이고, 발이 문 바닥에 오게 놓은 뒤 문 사각형으로 자른 128 레이어."""
    bb = pet.getchannel("A").getbbox()
    s = (door["w"] - 2) / (bb[2] - bb[0])
    size = round(FRAME * s)
    small = pet.resize((size, size), Image.LANCZOS)
    cx = door["x"] + door["w"] / 2
    left = round(cx - size / 2)
    top = round(door["y"] + door["h"] + 6 - bb[3] * s)
    layer = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    layer.paste(small, (left, top), small)
    clip = Image.new("L", (FRAME, FRAME), 0)
    clip.paste(255, (door["x"], door["y"], door["x"] + door["w"], door["y"] + door["h"]))
    layer.putalpha(ImageChops.multiply(layer.getchannel("A"), clip))
    return layer


def build(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    img, opening, sill = draw()
    house = finish(img)
    front = finish(front_layer(img, opening, sill))
    house.save(out_dir / "house.png")
    front.save(out_dir / "house_front.png")
    (out_dir / "house.json").write_text(json.dumps({"door": DOOR}, indent=2) + "\n")
    return house, front


def preview(house, front, path=PREVIEW):
    sheet = Image.open(ROOT / "ui" / "pets" / "cat" / "sheet_teenager.png")
    pet = sheet.crop((0, 10 * FRAME, FRAME, 11 * FRAME))  # sit_front 첫 프레임
    with_pet = house.copy()
    with_pet.alpha_composite(pet_in_door(pet))
    with_pet.alpha_composite(front)
    tiles = [house, front, with_pet]
    big = 2
    out = Image.new("RGBA", (len(tiles) * (FRAME * big + 16) + 16, FRAME * big * 2 + 48), (0, 0, 0, 0))
    for row, bg in enumerate([(255, 255, 255), (40, 44, 52)]):
        for i, t in enumerate(tiles):
            cell = Image.new("RGBA", (FRAME, FRAME), bg + (255,))
            cell.alpha_composite(t)
            out.paste(cell.resize((FRAME * big, FRAME * big), Image.NEAREST),
                      (16 + i * (FRAME * big + 16), 16 + row * (FRAME * big + 16)))
    bg = Image.new("RGBA", out.size, (200, 200, 200, 255))
    bg.alpha_composite(out)
    path.parent.mkdir(exist_ok=True)
    bg.convert("RGB").save(path)


def main():
    house, front = build()
    preview(house, front)
    print("ok: house")


if __name__ == "__main__":
    main()
