"""물건 (쓰레기 장난감) — draw_pets.py 엔진으로 그린 128 칸 그림과 노란 포스트잇.

    python tools/draw_objects.py  → ui/objects/{can,paper,glasses,computer,banana,milk,sock,umbrella}.png (128×128)
                                    + ui/objects/postit.png (40×40)
                                    + tools/preview/objects.png (128px 고양이 옆에 놓아 본 모습, 포스트잇 붙인 줄 포함)

앱은 그림을 96 논리 px로 줄여 보인다 (crates/screenpet-core/src/object.rs OBJECT_LOGICAL_SIZE). 움직임은 그림을 더 그리지 않고
CSS로 돌리거나(구르기·흔들기) 뒤집는다 (ui/object.js) — 그래서 그림 한 장이면 된다.
바닥은 칸 아래 끝 BOTTOM에 앉는다. POSTIT_AT은 포스트잇을 붙일 자리 (ui/object.js POSTIT_AT과 같다).
"""

import sys
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish  # noqa: E402
from draw_pets import LINE, WHITE, C, E, L, T, aa, mix, paint, shapes, view  # noqa: E402
from draw_props import new_canvas, tiny_face  # noqa: E402
from sprite_spec import FRAME  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "objects"
PREVIEW = ROOT / "tools" / "preview" / "objects.png"

BOTTOM = 126.5
POSTIT = 40

# crates/screenpet-core/src/object.rs ObjectKind::id와 같은 이름, 같은 순서
NAMES = ("can", "paper", "glasses", "computer", "banana", "milk", "sock", "umbrella")
# 포스트잇 가운데 자리 (128 칸 좌표) — 그림 위에 얹힌다
POSTIT_AT = {
    "can": (76, 80),
    "paper": (78, 84),
    "glasses": (40, 100),
    "computer": (92, 72),
    "banana": (64, 100),
    "milk": (74, 92),
    "sock": (58, 76),
    "umbrella": (44, 66),
}

CAN_RED = (232, 96, 92)
METAL = (206, 212, 220)
PAPER = (250, 246, 234)
CREASE = mix(PAPER, LINE, 0.35)
FRAME_BROWN = (146, 100, 76)
LENS = (214, 236, 250)
PLASTIC = (222, 214, 196)
SCREEN = (70, 78, 96)
SPARK = (255, 214, 102)
BANANA = (252, 216, 92)
BANANA_DARK = (196, 150, 66)
BANANA_IN = (255, 244, 196)
MILK_BLUE = (138, 186, 232)
SOCK = (246, 160, 170)
SOCK_STRIPE = (255, 240, 226)
CLOTH = (150, 132, 214)
CLOTH_LIGHT = mix(CLOTH, WHITE, 0.35)
HANDLE = (176, 128, 92)
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
    tiny_face(img, 62, 106, 0.85)


def draw_paper(img):
    """구긴 종이: 각진 뭉치, 구김 선."""
    ball = [T((38, 86), (46, 68), (62, 62), (78, 66), (90, 80), (92, 100), (84, 118), (68, BOTTOM), (48, 122), (36, 106))]
    blob(img, ball, PAPER, r=1.2,
         marks=[([L([(46, 68), (56, 84), (40, 94)], 1.3), L([(56, 84), (72, 78), (78, 66)], 1.3), L([(72, 78), (90, 92)], 1.2),
                  L([(56, 84), (60, 104), (48, 122)], 1.3), L([(60, 104), (84, 112)], 1.2)], CREASE),
                ([T((62, 86), (72, 80), (82, 94), (70, 100))], mix(PAPER, CREASE, 0.35))])
    tiny_face(img, 64, 100, 0.8)


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
    """고장난 컴퓨터: 네모 모니터, 금 간 화면, ×× 눈, 튀는 불꽃."""
    blob(img, [T((48, 108), (80, 108), (84, BOTTOM - 7), (44, BOTTOM - 7)), E(36, BOTTOM - 10, 92, BOTTOM)], mix(PLASTIC, LINE, 0.15), r=0.8, line=0.9)
    blob(img, [T((18, 44), (110, 44), (110, 110), (18, 110))], PLASTIC, r=3.0,
         marks=[([E(92, 100, 102, 106)], (232, 120, 110))])
    blob(img, [T((27, 52), (101, 52), (101, 98), (27, 98))], SCREEN, r=2.0, line=0.7)
    x_eyes = [L([(46 + dx, 66), (54 + dx, 74)], 2.0) for dx in (0, 28)] + [L([(54 + dx, 66), (46 + dx, 74)], 2.0) for dx in (0, 28)]
    paint(img, aa(shapes(x_eyes)), WHITE)
    paint(img, aa(shapes([L([(56, 88), (64, 83), (72, 88)], 1.8)])), WHITE)
    paint(img, aa(shapes([L([(30, 56), (42, 62), (38, 74), (48, 84)], 1.2), L([(42, 62), (56, 56)], 1.0)])), mix(SCREEN, WHITE, 0.6))
    blob(img, [T((106, 28), (114, 38), (108, 38), (116, 50), (100, 36), (106, 36))], SPARK, r=0.2, line=0.7)


def draw_banana(img):
    """바나나 껍질: 꼭지에서 세 갈래로 축 늘어진 껍질."""
    y = BOTTOM
    blob(img, [L([(64, y - 34), (40, y - 26), (24, y - 12), (20, y - 3)], 12.0)], BANANA, r=1.0,
         marks=[([L([(62, y - 30), (42, y - 22), (28, y - 9)], 4.0)], BANANA_IN)])
    blob(img, [L([(64, y - 34), (88, y - 26), (104, y - 12), (108, y - 3)], 12.0)], BANANA, r=1.0,
         marks=[([L([(66, y - 30), (86, y - 22), (100, y - 9)], 4.0)], BANANA_IN)])
    blob(img, [L([(64, y - 40), (65, y - 22), (64, y - 6)], 15.0)], BANANA, r=1.2,
         marks=[([E(52, y - 7, 76, y + 1)], BANANA_DARK), ([E(58, y - 26, 62, y - 12)], BANANA_IN)])
    blob(img, [L([(64, y - 40), (65, y - 50)], 7.0)], BANANA_DARK, r=0, line=0.8)
    tiny_face(img, 64, y - 25, 0.75)


def draw_milk(img):
    """빈 우유팩: 뾰족 지붕, 한쪽이 눌린 몸통, 파란 띠."""
    blob(img, [T((42, 80), (86, 80), (84, 100), (88, 112), (86, BOTTOM - 1), (42, BOTTOM - 1), (40, 104))], WHITE, r=1.4,
         marks=[([T((38, 108), (90, 108), (90, 120), (38, 120))], MILK_BLUE),
                ([L([(74, 84), (80, 100), (76, 112)], 1.2)], mix(WHITE, LINE, 0.3))])
    blob(img, [T((42, 80), (86, 80), (78, 62), (50, 62))], mix(WHITE, MILK_BLUE, 0.25), r=0.8, line=0.9)
    blob(img, [T((48, 54), (80, 54), (80, 63), (48, 63))], WHITE, r=0.6, line=0.9)
    tiny_face(img, 60, 94, 0.85)


def draw_sock(img):
    """찢어진 양말: ㄴ자, 줄무늬 목, 뒤꿈치에 구멍."""
    blob(img, [L([(52, 58), (52, 104), (66, 116), (98, 116)], 24.0)], SOCK, r=1.8,
         marks=[([T((36, 50), (70, 50), (70, 62), (36, 62)), T((36, 68), (70, 68), (70, 76), (36, 76))], SOCK_STRIPE),
                ([E(90, 104, 112, 128)], mix(SOCK, WHITE, 0.45))])
    blob(img, [E(46, 104, 60, 116)], mix(LINE, SOCK, 0.2), r=0, line=0,
         marks=[([E(48, 110, 58, 116)], mix(SOCK, LINE, 0.7))])
    paint(img, aa(shapes([L([(44, 103), (48, 100)], 1.1), L([(61, 104), (65, 100)], 1.1), L([(60, 117), (65, 121)], 1.1)])), LINE)
    tiny_face(img, 76, 114, 0.7)


def draw_umbrella(img):
    """부러진 우산: 뒤집힌 듯 꺾인 천, 굽은 살, 갈고리 손잡이."""
    blob(img, [L([(64, 62), (70, 90), (66, 112), (60, 120), (52, 118), (51, 110)], 3.6)], HANDLE, r=0, line=0.8)
    blob(img, [T((22, 74), (40, 52), (64, 42), (92, 50), (112, 76), (94, 68), (82, 82), (66, 70), (50, 84), (38, 68))], CLOTH, r=1.6,
         marks=[([L([(64, 42), (66, 70)], 1.6), L([(64, 42), (40, 68)], 1.4), L([(64, 42), (88, 68)], 1.4)], CLOTH_LIGHT)])
    blob(img, [L([(64, 42), (64, 34)], 3.0)], HANDLE, r=0, line=0.7)
    # 천을 뚫고 나온 살
    paint(img, aa(shapes([L([(92, 56), (108, 44), (114, 46)], 1.8)])), mix(METAL, LINE, 0.3))
    tiny_face(img, 64, 58, 0.75)


DRAW = {
    "can": draw_can,
    "paper": draw_paper,
    "glasses": draw_glasses,
    "computer": draw_computer,
    "banana": draw_banana,
    "milk": draw_milk,
    "sock": draw_sock,
    "umbrella": draw_umbrella,
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


def main():
    arts = build()
    preview(arts)
    print(f"wrote {len(arts)} images to {OUT_DIR} and {PREVIEW}")


if __name__ == "__main__":
    main()
