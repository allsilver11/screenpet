"""카와이 펫 집 — draw_pets.py 엔진(슈퍼샘플 마스크·melt·외곽선·paint)으로 그린 128px 집, 단계마다 하나씩 + 꿀벌 벌통.

    python tools/draw_house.py  → ui/house/{house,hive}_<stage>{.png,_front.png,.json}
                                  + ui/house/{house.png, house_front.png, house.json} (teenager 집 별칭)
                                  + tools/preview/house.png (teenager 집), tools/preview/houses.png (모든 단계 × 집·벌통)

앱의 겹치기 순서: <art>.png → 문 사각형(<art>.json "door")으로 잘라낸 펫 → <art>_front.png
<art>_front.png는 펫 앞에 와야 하는 부분(문틀·아치 위 벽 모서리·문턱)만 담고, 문 안쪽은 투명하다.

크기: 그림은 모두 128×128에 그리고, 앱은 단계마다 128 × HOUSE_SCALE[stage]로 줄여 보인다.
문간 펫은 화면에서 DOOR_PET × 몸 배율(STAGE_SCALE) 크기 — 문 사각형은 단계마다 그 펫이 teenager 집과 같은 비율로
문을 채우도록 정했다 (문 너비 ≈ 52 × STAGE_SCALE / HOUSE_SCALE).
"""

import json
import math
import sys
from pathlib import Path

from PIL import Image, ImageChops

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_pets import (EYE, LINE, OUTLINE, PINK, S, WHITE, C, E, L, T, aa, blank, grow, melt, mix, paint,  # noqa: E402
                       shapes, view)
from sprite_spec import FRAME, STAGE_SCALE, STAGES  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "house"
PREVIEW = ROOT / "tools" / "preview" / "house.png"
PREVIEW_ALL = ROOT / "tools" / "preview" / "houses.png"

# 단계별 하우스 창 배율 (src-tauri/src/house.rs house_scale, ui/house.js HOUSE_SCALE와 같다)
HOUSE_SCALE = {"baby": 0.55, "kinder": 0.68, "child": 0.82, "teenager": 1.0, "adult": 1.12}
# 문간 펫 스프라이트 한 변 (화면 px, 몸 배율 1) — ui/house.js DOOR_PET와 같다 (teenager 문 52 → (52 - 2) / 0.7)
DOOR_PET = (52 - 2) / 0.7
# 문간 펫 발 숨김: 문 바닥 아래로 SILL_HIDE + 스프라이트 × FEET_PAD (ui/house.js와 같다)
SILL_HIDE = 6
FEET_PAD = 0.1
KINDS = ("house", "hive")

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
# 아기 버섯집
CAP = (248, 170, 186)
CAP_DARK = mix(CAP, LINE, 0.3)
STEM = (255, 244, 226)
MINT = (172, 222, 204)
# 꼬마 오두막
ROOF_BLUE = (132, 170, 214)
# 어른 집
ROOF_GREEN = (124, 170, 128)
FLAG = (246, 196, 92)
FLOWERS = [(246, 128, 150), (250, 206, 96), (166, 150, 230)]
# 벌통
STRAW = (240, 198, 112)
STRAW_DARK = mix(STRAW, LINE, 0.32)
STRAW_LIGHT = mix(STRAW, WHITE, 0.35)
HONEY = (247, 178, 52)
HONEY_DARK = mix(HONEY, LINE, 0.3)
HONEY_LIGHT = mix(HONEY, WHITE, 0.45)
HIVE_INSIDE = (102, 68, 46)

FLOOR = 122  # 모든 문의 바닥 (= 집 바닥)


def _door(cx, w, h):
    return dict(x=round(cx - w / 2), y=FLOOR - h, w=w, h=h)


# 문 사각형 (128 프레임 좌표). 위는 반원 아치, 아래는 집 바닥.
# 너비·높이 = teenager(52×62) × STAGE_SCALE / HOUSE_SCALE (펫이 문을 같은 비율로 채운다)
DOOR_SIZE = {"baby": (42, 51), "kinder": (46, 55), "child": (50, 59), "teenager": (52, 62), "adult": (54, 64)}
# 꿀벌은 프레임 안에서 몸이 고양이보다 좁고(sit_front 몸 폭 66–80 vs 94–102) 몸 배율도 0.8이라, 같은 비율로 채우려면
# 벌통 입구가 집 문의 (꿀벌 몸 폭 × 0.8 / 고양이 몸 폭)배 — 단계별 0.52 / 0.58 / 0.62 / 0.67 / 0.65 — 작은 둥근 구멍이 된다
HIVE_FILL = {"baby": 0.52, "kinder": 0.58, "child": 0.62, "teenager": 0.67, "adult": 0.65}
HIVE_DOOR_SIZE = {s: (round(w * HIVE_FILL[s]), round(w * HIVE_FILL[s] * 62 / 52)) for s, (w, _) in DOOR_SIZE.items()}
DOORS = {
    "house": {"baby": _door(64, *DOOR_SIZE["baby"]), "kinder": _door(49, *DOOR_SIZE["kinder"]),
              "child": _door(55, *DOOR_SIZE["child"]), "teenager": dict(x=30, y=60, w=52, h=62),
              "adult": _door(46, *DOOR_SIZE["adult"])},
    "hive": {s: _door(64, *HIVE_DOOR_SIZE[s]) for s in STAGES},
}
DOOR = DOORS["house"]["teenager"]


def arch(x0, y0, x1, y1):
    """아치 모양 (위 반원 + 아래 사각)."""
    r = (x1 - x0) / 2
    return [E(x0, y0, x1, y0 + 2 * r), T((x0, y0 + r), (x1, y0 + r), (x1, y1), (x0, y1))]


def rrect(x0, y0, x1, y1, r):
    """둥근 사각형."""
    return [E(x0, y0, x0 + 2 * r, y0 + 2 * r), E(x1 - 2 * r, y0, x1, y0 + 2 * r), E(x0, y1 - 2 * r, x0 + 2 * r, y1),
            E(x1 - 2 * r, y1 - 2 * r, x1, y1), T((x0 + r, y0), (x1 - r, y0), (x1 - r, y1), (x0 + r, y1)),
            T((x0, y0 + r), (x1, y0 + r), (x1, y1 - r), (x0, y1 - r))]


def hexagon(cx, cy, r):
    """뾰족한 쪽이 위아래인 육각형."""
    return T(*[(cx + r * math.cos(math.radians(30 + 60 * i)), cy + r * math.sin(math.radians(30 + 60 * i)))
               for i in range(6)])


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


def flower(img, cx, cy, r, color, stem=0):
    """작은 꽃: (줄기) + 꽃잎 다섯 + 노란 가운데."""
    if stem:
        blob(img, [L([(cx, cy), (cx + 0.5, cy + stem)], 1.6)], GRASS, r=0, line=0.6)
        blob(img, [E(cx + 0.5, cy + stem * 0.55, cx + 6, cy + stem * 0.55 + 3.5)], GRASS, r=0, line=0.6)
    petals = [C(cx + r * 0.62 * math.cos(math.radians(-90 + 72 * i)), cy + r * 0.62 * math.sin(math.radians(-90 + 72 * i)),
                r * 0.48) for i in range(5)]
    blob(img, petals, color, r=0.3, line=0.7)
    paint(img, aa(shapes([C(cx, cy, r * 0.34)])), (255, 222, 120))


def grass_row(img, tufts):
    blob(img, [f for t in tufts for f in t], GRASS, r=0.6, line=0.85)


def doorway(img, d, frame=WOOD, frame_light=WOOD_LIGHT, inside=INSIDE, floor=INSIDE_FLOOR, frame_pad=4.5):
    """문틀 (아치 테두리) → 열린 문 안쪽 (어두운 집 안 + 은은한 바닥). 문 안쪽 마스크를 돌려준다."""
    x0, y0, x1, y1 = d["x"], d["y"], d["x"] + d["w"], FLOOR
    blob(img, arch(x0 - frame_pad, y0 - frame_pad, x1 + frame_pad, y1 + 1), frame, r=0,
         marks=[([L([(x0 - 2.2, y0 + d["w"] / 2), (x0 - 2.2, y1)], 1.0),
                  L([(x1 + 2.2, y0 + d["w"] / 2), (x1 + 2.2, y1)], 1.0)], frame_light)])
    return blob(img, arch(x0, y0, x1, y1), inside, r=0, line=0.7,
                marks=[([E(x0 - 6, y1 - 8, x1 + 6, y1 + 8)], floor),
                       ([E(x0 + 6, y0 + 3, x1 - 6, y0 + 22)], mix(inside, (0, 0, 0), 0.15))])


def door_sill(img, d, color=WOOD_LIGHT):
    """문턱 (펫 앞에 온다) — 외곽선까지 넓힌 마스크를 돌려준다."""
    x0, x1, y1 = d["x"], d["x"] + d["w"], FLOOR
    sill = blob(img, rrect(x0 - 7, y1 - 2.5, x1 + 7, y1 + 3.5, 2.8), color, r=0, line=0.9,
                marks=[([L([(x0 - 4, y1 - 1), (x1 + 4, y1 - 1)], 1.0)], WHITE)])
    return grow(sill, OUTLINE * 0.9)


def door_leaf(img, d, color=WOOD):
    """열린 문짝 (왼쪽 경첩, 바깥으로 젖혀져 비스듬히 보인다)."""
    x0, y0 = d["x"], d["y"]
    top = y0 + d["w"] * 0.12
    blob(img, [T((x0 - 15, top), (x0 - 4, top + 5), (x0 - 4, FLOOR - 1), (x0 - 15, FLOOR + 1))], color, r=0.8,
         marks=[([L([(x0 - 9.5, top + 6), (x0 - 9.5, FLOOR - 3)], 1.0)], mix(color, LINE, 0.35))])
    blob(img, [C(x0 - 6.5, (top + FLOOR) / 2 + 2, 1.5)], (236, 196, 112), r=0, line=0.5)


def round_window(img, cx, cy, r, curtain=CURTAIN, sill=True):
    """동그란 창문: 나무 테 → 유리 + 커튼 + 십자 창살 (+ 창턱)."""
    k = r / 12
    g = 8.6 * k
    blob(img, [C(cx, cy, r)], WOOD, r=0, line=0.9)
    blob(img, [C(cx, cy, g)], GLASS, r=0, line=0.6,
         marks=[([L([(cx, cy - g), (cx, cy + g)], 1.4), L([(cx - g, cy), (cx + g, cy)], 1.4)], WOOD),
                ([T((cx - 9 * k, cy - 10 * k), (cx - 2 * k, cy - 10 * k), (cx - 6 * k, cy), (cx - 10 * k, cy + 11 * k)),
                  E(cx - 11 * k, cy - 10 * k, cx - 4 * k, cy + 10 * k)], curtain),
                ([T((cx + 2 * k, cy - 10 * k), (cx + 9 * k, cy - 10 * k), (cx + 10 * k, cy + 11 * k), (cx + 6 * k, cy)),
                  E(cx + 4 * k, cy - 10 * k, cx + 11 * k, cy + 10 * k)], curtain),
                ([L([(cx + 2 * k, cy - 6 * k), (cx + 5 * k, cy - 7 * k)], 1.0)], WHITE)])
    if sill:
        blob(img, rrect(cx - 13 * k, cy + 12 * k, cx + 13 * k, cy + 16.5 * k, 2), WOOD, r=0, line=0.75)


def gable_roof(img, x0, x1, eave, top, color, trim=None):
    """둥근 박공 지붕 + 비늘 기와 (teenager 지붕과 같은 꼴, 크기만 다르다). 지붕 마스크를 돌려준다."""
    cx = (x0 + x1) / 2
    half = (x1 - x0) / 2
    roof = [T((x0, eave - 6), (cx - half * 0.33, top), (cx + half * 0.33, top), (x1, eave - 6), (x1, eave), (x0, eave)),
            E(cx - half * 0.5, top - 7, cx + half * 0.5, top + 24)]
    tiles = []
    for row, y in enumerate(range(eave - 5, top + 2, -11)):
        off = 6 if row % 2 else 0
        tiles += [E(x - 6 + off, y - 5, x + 6 + off, y + 5) for x in range(int(x0) + 6, int(x1), 12)]
    m = blob(img, roof, color, r=4.0,
             marks=[([L([(x0 + 4, eave - 0.5), (x1 - 4, eave - 0.5)], 3.0)], trim or mix(color, LINE, 0.35)),
                    (tiles, mix(color, LINE, 0.22)),
                    ([L([(cx - half * 0.38, top + 3), (cx - half * 0.12, top - 3)], 2.2)], mix(color, WHITE, 0.25))])
    with view(scale=1.0, lift=1.2):
        paint(img, aa(shapes(tiles)), color, clip=melt(shapes(roof), 4.0))
    return m


def plank_wall(img, x0, y0, x1, color=WALL, step=12):
    planks = [L([(x0 - 2, y), (x1 + 2, y)], 1.0) for y in range(int(y0) + 14, 124, step)]
    blob(img, rrect(x0, y0, x1, 124, 7), color, r=1.5, marks=[(planks, mix(color, LINE, 0.18))])


def chimney(img, x0, x1, top, bottom, puffs=1):
    blob(img, [T((x0, top + 5), (x1, top + 5), (x1, bottom), (x0, bottom))], CHIMNEY, r=1.0,
         marks=[([L([(x0, top + 12), (x1, top + 12)], 1.0), L([((x0 + x1) / 2, top + 5), ((x0 + x1) / 2, top + 12)], 1.0)],
                 mix(CHIMNEY, LINE, 0.3))])
    blob(img, rrect(x0 - 2.5, top, x1 + 2.5, top + 6.5, 2.5), mix(CHIMNEY, LINE, 0.15), r=0.8)
    cx = (x0 + x1) / 2
    clouds = [C(cx, top - 4.5, 3.2), C(cx + 6.5, top - 6.4, 2.4), C(cx - 5, top - 3.8, 2.0)]
    if puffs > 1:
        clouds += [C(cx + 3, top - 12, 3.4), C(cx + 9.5, top - 14, 2.6), C(cx - 2, top - 11, 2.2)]
    blob(img, clouds, SMOKE, r=0.8, line=0.75, line_color=mix(LINE, SMOKE, 0.45))


# =====================================================================
# 집 — 단계별
# =====================================================================
def draw_house_teenager():
    """지금의 집 (teenager). 그림은 바꾸지 않는다."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["house"]["teenager"]
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


def draw_house_baby():
    """아기 버섯집: 동글동글한 버섯 갓 지붕 + 커다란 둥근 문, 파스텔."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["house"]["baby"]
    with view(scale=1.0, lift=0):
        # 통통한 기둥 벽 (아래가 살짝 넓다)
        dots = [C(30, 92, 2.2), C(100, 100, 2.6), C(96, 82, 1.6), C(32, 110, 1.8)]
        blob(img, rrect(19, 52, 109, 124, 16), STEM, r=2.0,
             marks=[(dots, mix(STEM, CAP, 0.45))])
        # 버섯 갓: 큰 반타원, 아래 가장자리는 살짝 안으로 말린 띠
        cap = shapes([E(5, 6, 123, 104)])
        cap = ImageChops.multiply(cap, shapes([T((0, 0), (128, 0), (128, 64), (0, 64))]))
        cap = melt(ImageChops.lighter(cap, shapes(rrect(8, 54, 120, 68, 7))), 2.0)
        paint(img, aa(grow(cap, OUTLINE)), LINE)
        paint(img, aa(cap), CAP)
        spots = [E(22, 30, 38, 42), E(52, 15, 74, 29), E(86, 26, 102, 38), E(38, 47, 48, 55), E(98, 46, 108, 54),
                 E(66, 38, 78, 47), E(11, 50, 19, 57)]
        paint(img, aa(shapes(spots)), WHITE, clip=cap)
        paint(img, aa(shapes([L([(10, 63.5), (118, 63.5)], 3.2)])), CAP_DARK, clip=cap)
        paint(img, aa(shapes([L([(30, 22), (44, 13)], 2.4)])), mix(CAP, WHITE, 0.45), clip=cap)

        opening = doorway(img, d, frame=MINT, frame_light=mix(MINT, WHITE, 0.4))
        # 문 위 작은 하트 (갓 띠에 매달림)
        blob(img, [L([(64, 66), (64, 62)], 0.9)], WOOD_DARK, r=0, line=0)
        heart = [C(61.6, 64.6, 2.6), C(66.4, 64.6, 2.6), T((59.2, 65.5), (68.8, 65.5), (64, 70.5))]
        blob(img, heart, (246, 128, 150), r=0.4, line=0.7)

        sill = door_sill(img, d, color=mix(MINT, WHITE, 0.5))
        grass_row(img, [tuft(12, 125, 7, -1), tuft(19, 126, 5.5), tuft(112, 126, 6.5), tuft(118, 125, 7.5, 1)])
        flower(img, 103, 113, 5.0, FLOWERS[0], stem=8)
    return img, opening, sill


def draw_house_kinder():
    """꼬마 오두막: 작은 판자 오두막, 파란 지붕, 동그란 창문 하나."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["house"]["kinder"]
    with view(scale=1.0, lift=0):
        plank_wall(img, 16, 50, 112)
        gable_roof(img, 6, 122, 63, 20, ROOF_BLUE)
        door_leaf(img, d)
        opening = doorway(img, d)
        round_window(img, 95, 86, 11)
        sill = door_sill(img, d)
        grass_row(img, [tuft(10, 125, 8, -1), tuft(116, 125, 8, 1), tuft(107, 126, 6)])
    return img, opening, sill


def draw_house_child():
    """어린이 집: 지금 집과 비슷하지만 조금 단순하게 (굴뚝·간판 없음) + 지붕 위 작은 깃발."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["house"]["child"]
    with view(scale=1.0, lift=0):
        # 깃발 (지붕 뒤에서 솟은 깃대)
        blob(img, [L([(64, 26), (64, 5)], 1.6)], WOOD_DARK, r=0, line=0.6)
        blob(img, [T((65, 5.5), (82, 10), (65, 15.5))], FLAG, r=0.6, line=0.8,
             marks=[([C(70, 10.5, 1.8)], WHITE)])
        plank_wall(img, 12, 44, 116)
        gable_roof(img, 4, 124, 59, 17, ROOF)
        door_leaf(img, d)
        opening = doorway(img, d)
        round_window(img, 102, 84, 11.5)
        sill = door_sill(img, d)
        grass_row(img, [tuft(9, 125, 8, -1), tuft(118, 125, 8, 1), tuft(110, 126, 6.5)])
    return img, opening, sill


def draw_house_adult():
    """어른 집: 더 넓은 집, 꽃 상자 창문, 굴뚝 연기 두 모금."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["house"]["adult"]
    with view(scale=1.0, lift=0):
        chimney(img, 98, 110, 22, 44, puffs=2)
        plank_wall(img, 7, 40, 121)
        gable_roof(img, 4, 124, 54, 16, ROOF_GREEN)
        door_leaf(img, d)
        opening = doorway(img, d)
        # 발바닥 간판
        cx = d["x"] + d["w"] / 2
        blob(img, [L([(cx - 6, 48), (cx - 4, 44)], 1.0), L([(cx + 6, 48), (cx + 4, 44)], 1.0)], WOOD_DARK, r=0, line=0)
        blob(img, rrect(cx - 11, 46.5, cx + 11, 56.5, 3.5), WOOD_LIGHT, r=0.6, line=0.85)
        paint(img, aa(shapes(paw(cx, 52.2, 0.95))), (236, 136, 142))
        # 네모 창문 + 꽃 상자
        wx0, wy0, wx1, wy1 = 84, 64, 114, 90
        blob(img, rrect(wx0 - 3.5, wy0 - 3.5, wx1 + 3.5, wy1 + 3, 3), WOOD, r=0, line=0.9)
        blob(img, rrect(wx0, wy0, wx1, wy1, 2), GLASS, r=0, line=0.6,
             marks=[([L([((wx0 + wx1) / 2, wy0), ((wx0 + wx1) / 2, wy1)], 1.4), L([(wx0, 77), (wx1, 77)], 1.4)], WOOD),
                    ([T((wx0, wy0), (wx0 + 9, wy0), (wx0 + 4, 80), (wx0, wy1))], CURTAIN),
                    ([T((wx1 - 9, wy0), (wx1, wy0), (wx1, wy1), (wx1 - 4, 80))], CURTAIN),
                    ([L([(wx0 + 12, 69), (wx0 + 16, 68)], 1.0)], WHITE)])
        for i, fx in enumerate(range(wx0 + 1, wx1 + 2, 6)):
            flower(img, fx, 91 - (i % 2) * 2, 3.6, FLOWERS[i % 3])
        blob(img, rrect(wx0 - 5, 92, wx1 + 5, 101, 2.5), (196, 128, 96), r=0, line=0.85,
             marks=[([L([(wx0 - 2, 96.5), (wx1 + 2, 96.5)], 1.0)], mix((196, 128, 96), LINE, 0.3))])
        sill = door_sill(img, d)
        grass_row(img, [tuft(9, 125, 8, -1), tuft(119, 125, 8, 1), tuft(84, 126, 6.5), tuft(96, 126, 7)])
        flower(img, 108, 114, 4.4, FLOWERS[2], stem=7)
    return img, opening, sill


# =====================================================================
# 벌통 — 짚을 둥글게 감아 올린 옛날 벌통(skep)
# =====================================================================
# 단계별: (돔 반너비, 돔 꼭대기 y, 띠 수, 벌집 칸 수, 꽃 수)
HIVE = {"baby": (50, 18, 4, 3, 1), "kinder": (54, 14, 5, 4, 1), "child": (56, 12, 5, 5, 1),
        "teenager": (58, 9, 6, 6, 2), "adult": (59, 6, 7, 7, 2)}


def draw_hive(stage):
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = DOORS["hive"][stage]
    half, top, coils, cells, flowers = HIVE[stage]
    cx, base = 64, 118
    with view(scale=1.0, lift=0):
        # 받침 나무판
        blob(img, rrect(cx - half - 1, 114, cx + half + 1, 124, 3), WOOD, r=0, line=0.9,
             marks=[([L([(cx - half + 2, 117), (cx + half - 2, 117)], 1.0)], WOOD_LIGHT)])
        # 짚 띠: 아래부터 위로 좁아진다 (종 모양). 위 띠부터 그려 아래 띠가 앞에 온다
        h = (base - top) / (coils + 0.6)
        bands = []
        for i in range(coils):
            y1 = base - i * h
            y0 = y1 - h - 1.5
            t = (i + 0.5) / (coils + 0.6)
            w = half * (1 - t ** 2.4) ** 0.55
            bands.append((y0, y1, w))
        dome = blank()
        knob_y = base - coils * h
        blob(img, [E(cx - h * 0.9, knob_y - h * 0.7, cx + h * 0.9, knob_y + h * 0.6)], STRAW, r=0.5, line=0.9,
             marks=[([L([(cx - h * 0.3, knob_y - h * 0.3), (cx + h * 0.1, knob_y - h * 0.45)], 1.2)], STRAW_LIGHT)])
        for y0, y1, w in reversed(bands):
            r = (y1 - y0) / 2
            band = rrect(cx - w, y0, cx + w, y1, r)
            straws = [L([(x, y0 + 2), (x + 3, y1 - 2)], 1.0) for x in range(int(cx - w) + 6, int(cx + w) - 3, 7)]
            m = blob(img, band, STRAW, r=0.4, line=0.9,
                     marks=[(straws, STRAW_DARK), ([L([(cx - w * 0.6, y0 + 2.6), (cx - w * 0.2, y0 + 2.2)], 1.3)],
                                                   STRAW_LIGHT)])
            dome = ImageChops.lighter(dome, m)
        # 벌집 무늬 (오른쪽 옆구리)
        hx, hy, hr = cx + half * 0.45, top + (base - top) * 0.24, 5.2
        spots = [(0, 0), (1, 0), (0.5, 1), (-0.5, 1), (1.5, 1), (0, 2), (1, 2)][:cells]
        hexes = [hexagon(hx + sx * hr * 1.73, hy + sy * hr * 1.5, hr) for sx, sy in spots]
        hm = shapes(hexes)
        paint(img, aa(grow(hm, 1.0)), HONEY_DARK, clip=dome)
        inner = shapes([hexagon(hx + sx * hr * 1.73, hy + sy * hr * 1.5, hr - 1.3) for sx, sy in spots])
        paint(img, aa(inner), HONEY, clip=dome)
        paint(img, aa(shapes([C(hx + sx * hr * 1.73 - 1.4, hy + sy * hr * 1.5 - 1.6, 1.1) for sx, sy in spots])),
              HONEY_LIGHT, clip=dome)
        # 꿀 방울: 벌집 아래 끝에서 똑
        low = max(sy for _, sy in spots)
        dx = hx + min(sx for sx, sy in spots if sy == low) * hr * 1.73
        dy = hy + low * hr * 1.5 + hr * 0.6
        blob(img, [L([(dx, dy - 2), (dx, dy + 5)], 2.6), C(dx, dy + 6.5, 2.9)], HONEY, r=0.5, line=0.8,
             marks=[([C(dx - 0.9, dy + 5.6, 0.9)], HONEY_LIGHT)])
        # 입구: 짚 테두리 아치 → 어두운 안쪽
        opening = doorway(img, d, frame=STRAW_DARK, frame_light=STRAW, inside=HIVE_INSIDE,
                          floor=mix(HIVE_INSIDE, HONEY, 0.25), frame_pad=4.0)
        sill = door_sill(img, d, color=WOOD_LIGHT)
        # 작은 꽃 (받침 옆)
        flower(img, 14, 104, 5.0, FLOWERS[0], stem=12)
        if flowers > 1:
            flower(img, 116, 107, 4.4, FLOWERS[2], stem=9)
        grass_row(img, [tuft(11, 125, 7, -1), tuft(21, 126, 5.5), tuft(117, 125, 7, 1)])
    return img, opening, sill


HOUSE_DRAW = {"baby": draw_house_baby, "kinder": draw_house_kinder, "child": draw_house_child,
              "teenager": draw_house_teenager, "adult": draw_house_adult}


def draw(kind="house", stage="teenager"):
    """(슈퍼샘플 캔버스, 문 안쪽 마스크, 문턱 마스크)."""
    return HOUSE_DRAW[stage]() if kind == "house" else draw_hive(stage)


def front_layer(img, opening, sill, door=DOOR):
    """펫 앞에 올 부분만: 문 주변(문틀·아치 위 벽 모서리)에서 문 안쪽을 뺀 것 + 문턱."""
    d = door
    pad = 8
    with view(scale=1.0, lift=0):
        region = shapes([T((d["x"] - pad, d["y"] - pad), (d["x"] + d["w"] + pad, d["y"] - pad),
                           (d["x"] + d["w"] + pad, FLOOR + 6), (d["x"] - pad, FLOOR + 6))])
    keep = ImageChops.lighter(ImageChops.subtract(region, opening), aa(sill))
    out = img.copy()
    out.putalpha(ImageChops.multiply(img.getchannel("A"), keep))
    return out


def door_pet_size(stage, body=1.0):
    """문간 펫 스프라이트 한 변 (128 그림 좌표) — 앱: 화면 DOOR_PET × 몸 배율, 그림은 HOUSE_SCALE배로 보인다."""
    return DOOR_PET * STAGE_SCALE[stage] * body / HOUSE_SCALE[stage]


def pet_in_door(pet, door=DOOR, size=DOOR_PET):
    """앱과 같은 방식: 펫 프레임을 size(그림 좌표)로 줄이고, 문 가운데·발이 문턱 뒤에 오게 놓은 뒤 문 사각형으로 자른 128 레이어."""
    size = round(size)
    small = pet.resize((size, size), Image.LANCZOS)
    cx = door["x"] + door["w"] / 2
    left = round(cx - size / 2)
    top = round(door["y"] + door["h"] + SILL_HIDE + size * FEET_PAD - size)
    layer = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    layer.paste(small, (left, top), small)
    clip = Image.new("L", (FRAME, FRAME), 0)
    clip.paste(255, (door["x"], door["y"], door["x"] + door["w"], door["y"] + door["h"]))
    layer.putalpha(ImageChops.multiply(layer.getchannel("A"), clip))
    return layer


def build(out_dir=OUT_DIR):
    """모든 단계의 집·벌통을 그려 저장한다. {(kind, stage): (그림, 앞 레이어)}를 돌려준다."""
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for kind in KINDS:
        for stage in STAGES:
            door = DOORS[kind][stage]
            img, opening, sill = draw(kind, stage)
            art = finish(img)
            front = finish(front_layer(img, opening, sill, door))
            name = f"{kind}_{stage}"
            art.save(out_dir / f"{name}.png")
            front.save(out_dir / f"{name}_front.png")
            (out_dir / f"{name}.json").write_text(json.dumps({"door": door}, indent=2) + "\n")
            arts[kind, stage] = (art, front)
    # 예전 이름 (teenager 집) — 호환용 별칭
    art, front = arts["house", "teenager"]
    art.save(out_dir / "house.png")
    front.save(out_dir / "house_front.png")
    (out_dir / "house.json").write_text(json.dumps({"door": DOOR}, indent=2) + "\n")
    return arts


def body_scale(pet_id, stage):
    """pet.json stage_size (없으면 1)."""
    f = ROOT / "ui" / "pets" / pet_id / "pet.json"
    return json.loads(f.read_text()).get("stage_size", {}).get(stage, 1.0) if f.is_file() else 1.0


def sit_frame(pet_id, stage):
    """펫 시트의 sit_front 첫 프레임 (없으면 None)."""
    d = ROOT / "ui" / "pets" / pet_id
    sheet = d / f"sheet_{stage}.png"
    if not sheet.is_file():
        return None
    row = json.loads((d / "pet.json").read_text())["animations"]["sit_front"]["row"]
    return Image.open(sheet).convert("RGBA").crop((0, row * FRAME, FRAME, (row + 1) * FRAME))


def bee_placeholder():
    """ui/pets/bee가 아직 없을 때 미리보기용 꿀벌 (앞모습 대충)."""
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    with view(scale=1.0, lift=0):
        blob(img, [E(14, 30, 52, 74), E(76, 30, 114, 74)], (226, 240, 252), r=0.5, line=0.8)
        blob(img, [E(30, 62, 98, 126)], (252, 212, 84), r=0.5,
             marks=[([L([(28, 88), (100, 88)], 7), L([(32, 108), (96, 108)], 7)], (96, 70, 60))])
        blob(img, [L([(52, 22), (44, 8)], 2), L([(76, 22), (84, 8)], 2), C(44, 8, 4), C(84, 8, 4)], (96, 70, 60),
             r=0, line=0.5)
        blob(img, [E(26, 14, 102, 78)], (252, 212, 84), r=0.5)
        paint(img, aa(shapes([C(50, 46, 4.2), C(78, 46, 4.2)])), EYE)
        paint(img, aa(shapes([C(48.6, 44.4, 1.4), C(76.6, 44.4, 1.4)])), WHITE)
        paint(img, aa(shapes([E(36, 54, 46, 60), E(82, 54, 92, 60)])), (252, 170, 160))
        paint(img, aa(shapes([L([(59, 56), (64, 59), (69, 56)], 1.6)])), LINE)
    return finish(img)


def preview(arts, path=PREVIEW):
    """teenager 집 (그림 / 앞 레이어 / 고양이와 함께) — 예전 미리보기와 같다."""
    house, front = arts["house", "teenager"]
    with_pet = house.copy()
    with_pet.alpha_composite(pet_in_door(sit_frame("cat", "teenager"), DOOR, door_pet_size("teenager")))
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


def preview_all(arts, path=PREVIEW_ALL):
    """단계마다 앱에서 보이는 크기(128 × HOUSE_SCALE, 2배 확대)로: 집 + 고양이, 벌통 + 꿀벌. 아래 줄은 어두운 배경."""
    big = 2
    bee = {s: sit_frame("bee", s) for s in STAGES}
    col_w = round(FRAME * max(HOUSE_SCALE.values()) * big) + 12
    width = 16 + len(STAGES) * col_w
    height = 16 + 2 * 2 * (round(FRAME * max(HOUSE_SCALE.values()) * big) + 12)
    out = Image.new("RGBA", (width, height), (200, 200, 200, 255))
    cell_h = round(FRAME * max(HOUSE_SCALE.values()) * big) + 12
    for c, stage in enumerate(STAGES):
        side = round(FRAME * HOUSE_SCALE[stage] * big)
        pets = {"house": (sit_frame("cat", stage), body_scale("cat", stage)),
                "hive": (bee[stage], body_scale("bee", stage)) if bee[stage] else (bee_placeholder(), 0.8)}
        for k, kind in enumerate(KINDS):
            art, front = arts[kind, stage]
            comp = art.copy()
            pet, body = pets[kind]
            comp.alpha_composite(pet_in_door(pet, DOORS[kind][stage], door_pet_size(stage, body)))
            comp.alpha_composite(front)
            comp = comp.resize((side, side), Image.LANCZOS)
            for b, bg in enumerate([(255, 255, 255), (40, 44, 52)]):
                cell = Image.new("RGBA", (side, side), bg + (255,))
                cell.alpha_composite(comp)
                x = 16 + c * col_w + (col_w - 12 - side) // 2
                y = 16 + (b * 2 + k) * cell_h + (cell_h - 12 - side)
                out.paste(cell, (x, y))
    path.parent.mkdir(exist_ok=True)
    out.convert("RGB").save(path)


def main():
    arts = build()
    preview(arts)
    preview_all(arts)
    print("ok: houses + hives")


if __name__ == "__main__":
    main()
