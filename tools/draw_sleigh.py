"""🛷 썰매 세리머니의 썰매 그림.

    python tools/draw_sleigh.py  → ui/sleigh/sleigh.png (동글, 128칸) + ui/sleigh/sleigh_pixel.png (픽셀, 32×16칸)

오른쪽을 보고 선다 (앞이 오른쪽, 앞코가 동그랗게 말렸다). 썰매 날의 바닥이 그림 바닥에 닿는다 — 앱은 그림 아래 가운데를
날아가는 자리에 맞춘다 (src-tauri/src/sleigh.rs). 루돌프·산타는 그림에 없다: 웹뷰가 동물 시트에서 그려 얹는다 (ui/sleigh.js).
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from draw_house import blob, finish  # noqa: E402
from draw_pets import LINE, WHITE, E, L, T, mix, view  # noqa: E402
from draw_props import new_canvas  # noqa: E402
import draw_pixel_world as px  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "sleigh"

RED = (214, 52, 60)
RED_DARK = (150, 30, 44)
GOLD = (246, 196, 72)
GOLD_DARK = (196, 140, 40)

PIXEL = """
    RR........................yy..
    RRr......................yrry.
    RRr......................yrrRy
    RRr.....................yrrrRy
    RRrrrrrrrrrrrrrrrrrrrrrrrrrRy.
    RRrryyyyyyyyyyyyyyyyyyyyrrRy..
    RRrrrrrrrrrrrrrrrrrrrrrrrRRy..
    .RRrrrrrrrrrrrrrrrrrrrrrRRy...
    ..RRRRRRRRRRRRRRRRRRRRRRRy....
    ....Y.............Y...........
    ....Y.............Y.......yy..
    yyyyyyyyyyyyyyyyyyyyyyyyyy.y..
    .YYYYYYYYYYYYYYYYYYYYYYYYyy...
"""


def draw_round():
    img = new_canvas()
    with view(scale=1.0, lift=0):
        # 썰매 날: 앞에서 위로 말려 올라간다
        blob(img, [L([(6, 124), (100, 124), (114, 119), (120, 110), (114, 104), (108, 109)], 3.4),
                   L([(30, 124), (30, 106)], 3.0), L([(84, 124), (84, 106)], 3.0)], GOLD, r=0, line=0.9,
             marks=[([L([(8, 126), (100, 126)], 1.2)], GOLD_DARK)])
        # 몸통: 뒤가 높은 등받이, 가운데가 낮고, 앞코가 말렸다
        body = [(6, 46), (18, 42), (24, 66), (64, 72), (88, 70), (98, 56), (106, 46), (116, 46), (120, 56), (114, 66),
                (106, 92), (96, 106), (22, 106), (8, 94)]
        blob(img, [T(*body)], RED, r=1.4,
             marks=[([L([(12, 76), (60, 82), (92, 80), (110, 58)], 2.6)], GOLD),
                    ([E(40, 86, 48, 94), E(64, 87, 72, 95)], GOLD),
                    ([T((6, 46), (18, 42), (22, 60), (10, 64))], RED_DARK),
                    ([L([(110, 48), (116, 52), (114, 58)], 1.6)], mix(RED, WHITE, 0.4)),
                    ([L([(16, 100), (92, 100)], 1.4)], mix(RED, LINE, 0.4))])
    return finish(img)


def draw_pixel():
    return px.icon(PIXEL, 32, 16, bottom=True).image()


def build(out_dir=OUT_DIR):
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {"sleigh": draw_round(), "sleigh_pixel": draw_pixel()}
    for name, im in arts.items():
        im.save(out_dir / f"{name}.png", optimize=True)
    return arts


if __name__ == "__main__":
    build()
    print(f"wrote {OUT_DIR}/sleigh.png, sleigh_pixel.png")
