"""🚗 탈것 그림 — draw_pets.py 엔진 (녹인 도형 + 외곽선)으로 그린 옆모습 시트.

    python tools/draw_vehicles.py [--preview 경로.png]
        → ui/vehicles/{car,heli,scooter,truck}.png  (가로 FRAMES칸 — 한 칸 = 탈것 창 크기의 2배 px)
        + 미리보기 (기본 tools/preview/vehicles.png): 앱 크기로 줄인 탈것마다 청소년 고양이를 좌석에 앉혀 본 모습

오른쪽을 보고 선다 (앞이 오른쪽 — 왼쪽으로 갈 때 웹뷰가 뒤집음). 칸은 탈것 창 그대로: 그림 둘레에 WINDOW_PAD 여유,
바퀴 바닥은 칸 아래에서 4 논리 px 위 (crates/screenpet-core/src/vehicle.rs window_logical·Spec과 같은 숫자 — 테스트가 맞춰 봄).
칸마다 바퀴 살이 돌고 (자동차·트럭·킥보드) 프로펠러가 돎 (헬리콥터) — 웹뷰는 달리거나 시동이 걸렸을 때만 칸을 넘김.
펫은 그림에 없음: 펫 창이 좌석에 앉고, 문·짐칸 벽 아래 다리 몫은 펫 웹뷰가 잘라 냄 (render.seat) — 그래서 좌석 둘레는 위가 트여 있음.
"""

import math
import sys
from contextlib import contextmanager
from pathlib import Path

from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets  # noqa: E402
from draw_house import blob  # noqa: E402
from draw_pets import LINE, WHITE, E, L, T, aa, drop_faint, mix, paint, shapes, view  # noqa: E402
from sprite_spec import FRAME  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "ui" / "vehicles"
PREVIEW = ROOT / "tools" / "preview" / "vehicles.png"

# crates/screenpet-core/src/vehicle.rs VehicleKind::id와 같은 이름, 같은 순서
NAMES = ("car", "heli", "scooter", "truck")
# 그림 크기 (논리 px: 너비, 높이) — vehicle.rs Spec w·h
SIZE = {"car": (176, 100), "heli": (208, 150), "scooter": (104, 118), "truck": (236, 104)}
# 좌석 (논리 px, 가운데 바닥 기준: 가로 자리, 문 윗선 높이, 발 가장 낮은 높이) — vehicle.rs Spec seat_x·cover·seat_min
SEAT = {"car": (-14, 56, 22), "heli": (20, 50, 22), "scooter": (-8, 0, 15), "truck": (58, 58, 24)}
# 운전대·손잡이 가로 자리 (논리 px — 운전석 펫 앞 끝이 여기 닿음, 작은 펫은 앞으로 당겨 앉음) — vehicle.rs Spec wheel_x
WHEEL = {"car": 8, "heli": 0, "scooter": 30, "truck": 70}
# 그 x에서 운전대·손잡이가 그려진 높이 (논리 px: 아래, 위 — 시험용)
STEER_Y = {"car": (62, 72), "scooter": (104, 108), "truck": (62, 72)}
# 🛻 짐칸 좌석 가로 자리와 벽 윗선 — vehicle.rs Spec bed_x·bed_cover
BED = {"truck": ((-30, -88), 44)}
# 창 여유 (논리 px) — vehicle.rs WINDOW_PAD
PAD = 24
# 바퀴 바닥 아래 그림자 자리 (논리 px)
UNDER = 4
# 칸 수 (바퀴·프로펠러가 도는 세 장)
FRAMES = 3
# 내보내는 배율 (논리 px당 px — 2배 화면에서도 또렷하게)
OUT = 2
# 슈퍼샘플 배율 (draw_pets.K 대신 — 큰 그림이라 더 촘촘히)
HIRES_K = 8
# 펫 다리가 가려지는 몫 — vehicle.rs LEG_SHARE
LEG_SHARE = 0.32
# 운전석 펫 앞 끝이 몸 가운데에서 나온 몫 — vehicle.rs DRIVER_REACH
DRIVER_REACH = 0.22

TIRE = (78, 72, 82)
HUB = (226, 224, 230)
METAL = (196, 202, 212)
GLASS = (200, 232, 248)
HEADLIGHT = (255, 232, 140)
TAIL = (240, 96, 96)
SEAT_BROWN = (176, 120, 96)
CAR = (246, 124, 116)
TRUCK = (104, 172, 226)
SCOOTER = (120, 206, 176)
HELI = (252, 204, 88)
HELI_STRIPE = (244, 146, 74)


def window(name):
    """창 크기 (논리 px)"""
    w, h = SIZE[name]
    return w + 2 * PAD, h + PAD + UNDER


@contextmanager
def hires(k=HIRES_K):
    """엔진의 슈퍼샘플 배율을 잠깐 키움 (큰 그림을 촘촘히)"""
    old = draw_pets.K, draw_pets.S
    draw_pets.K, draw_pets.S = k, FRAME * k
    try:
        yield
    finally:
        draw_pets.K, draw_pets.S = old


class Frame:
    """논리 px (가운데 바닥 기준, 위로 +) → 128 칸 좌표. 창이 칸 아래쪽 가운데에 들어가게 맞춤"""

    def __init__(self, name):
        self.name = name
        ww, wh = window(name)
        self.f = 124.0 / max(ww, wh)
        self.base = 128.0 - UNDER * self.f

    def p(self, x, y):
        return (64.0 + x * self.f, self.base - y * self.f)

    def E(self, x0, y0, x1, y1):
        (a, b), (c, d) = self.p(x0, y1), self.p(x1, y0)
        return E(a, b, c, d)

    def C(self, x, y, r):
        return self.E(x - r, y - r, x + r, y + r)

    def T(self, *pts):
        return T(*[self.p(x, y) for x, y in pts])

    def L(self, pts, w):
        return L([self.p(x, y) for x, y in pts], w * self.f)

    def crop(self, img):
        """칸 그림에서 창 자리만 잘라 OUT배 px로"""
        ww, wh = window(self.name)
        k = draw_pets.K
        left = (64.0 - ww / 2 * self.f) * k
        top = (128.0 - wh * self.f) * k
        box = tuple(round(v) for v in (left, top, left + ww * self.f * k, 128.0 * k))
        out = img.crop(box).resize((ww * OUT, wh * OUT), Image.LANCZOS)
        drop_faint(out)
        return out


def canvas():
    """빈 슈퍼샘플 칸"""
    return Image.new("RGBA", (draw_pets.S, draw_pets.S), (0, 0, 0, 0))


def rot(cx, cy, r, deg):
    """(cx, cy)에서 반지름 r, 각도 deg (도)의 점"""
    a = math.radians(deg)
    return (cx + r * math.cos(a), cy + r * math.sin(a))


def wheel(img, fr, x, y, r, spin, rim=None):
    """바퀴: 짙은 타이어, 밝은 휠, 살 셋 (spin도 돌림), 가운데 못"""
    rim = rim or HUB
    spokes = [fr.L([(x, y), rot(x, y, r * 0.5, spin + 120 * i)], max(1.6, r * 0.13)) for i in range(3)]
    blob(img, [fr.C(x, y, r)], TIRE, r=0, line=0.8,
         marks=[([fr.C(x, y, r * 0.58)], rim),
                (spokes, mix(rim, LINE, 0.35)),
                ([fr.C(x, y, r * 0.16)], mix(rim, LINE, 0.5)),
                ([fr.L([rot(x, y, r * 0.8, 200), rot(x, y, r * 0.8, 240)], max(1.2, r * 0.1))], mix(TIRE, WHITE, 0.25))])


def lamp(img, fr, x, y, r, color):
    """전조등·미등: 동그란 불과 반짝"""
    blob(img, [fr.C(x, y, r)], color, r=0, line=0.6, marks=[([fr.C(x - r * 0.3, y + r * 0.3, r * 0.35)], WHITE)])


def smile(img, fr, x, y, w):
    """앞 범퍼의 작은 미소 (동글한 펫들과 어울리게)"""
    paint(img, aa(shapes([fr.L([(x, y + w * 0.5), (x + w * 0.18, y), (x + w * 0.1, y - w * 0.4)], 1.6)])), LINE)


def draw_car(img, fr, n):
    """🚗 동글한 오픈카: 산호색 몸통, 앞 유리, 운전대, 좌석 등받이, 바퀴 둘 (n칸째 살 돌림)"""
    spin = n * 40
    # 좌석 등받이 (펫 뒤로 보임)
    blob(img, [fr.T((-50, 50), (-40, 86), (-30, 86), (-28, 50))], SEAT_BROWN, r=1.4, line=0.7,
         marks=[([fr.L([(-41, 80), (-38, 56)], 2.0)], mix(SEAT_BROWN, WHITE, 0.3))])
    # 앞 유리 테두리와 유리
    blob(img, [fr.T((26, 54), (36, 88), (44, 88), (40, 54))], METAL, r=0.6, line=0.7,
         marks=[([fr.T((32, 58), (38, 82), (40, 82), (38, 58))], GLASS)])
    # 운전대
    blob(img, [fr.L([(12, 64), (24, 52)], 2.6)], mix(TIRE, WHITE, 0.2), r=0, line=0.5)
    blob(img, [fr.E(6, 60, 16, 74)], mix(TIRE, WHITE, 0.15), r=0, line=0.5)
    # 몸통: 둥근 뒤, 낮은 문, 앞으로 비스듬한 보닛
    body = [(-84, 18), (-88, 40), (-80, 58), (-50, 58), (-30, 56), (10, 56), (30, 54), (60, 50), (82, 42), (88, 28), (84, 14), (-80, 12)]
    blob(img, [fr.T(*body)], CAR, r=2.0,
         marks=[([fr.L([(-80, 52), (-30, 51), (30, 49), (66, 44)], 2.6)], mix(CAR, WHITE, 0.45)),
                ([fr.L([(-18, 22), (-18, 54)], 1.2), fr.L([(30, 22), (30, 52)], 1.0)], mix(CAR, LINE, 0.35)),
                ([fr.L([(-12, 40), (-4, 40)], 2.2)], mix(CAR, LINE, 0.45)),
                ([fr.T((-86, 12), (86, 12), (86, 20), (-86, 20))], mix(CAR, LINE, 0.18))])
    lamp(img, fr, 80, 34, 6, HEADLIGHT)
    lamp(img, fr, -84, 40, 4.5, TAIL)
    smile(img, fr, 86, 20, 10)
    wheel(img, fr, -54, 18, 18, spin)
    wheel(img, fr, 54, 18, 18, spin)


def draw_truck(img, fr, n):
    """🛻 동글한 픽업 트럭: 하늘색, 지붕 없는 운전석 (앞 유리·운전대), 뒤에 짐칸 (벽 위가 트임), 바퀴 둘"""
    spin = n * 40
    # 운전석 등받이와 짐칸 앞벽 (뒷창)
    blob(img, [fr.T((18, 40), (18, 84), (32, 84), (32, 40))], mix(TRUCK, LINE, 0.15), r=1.2, line=0.8,
         marks=[([fr.T((21, 62), (21, 78), (29, 78), (29, 62))], GLASS)])
    # 앞 유리
    blob(img, [fr.T((86, 52), (94, 92), (102, 92), (100, 52))], METAL, r=0.6, line=0.7,
         marks=[([fr.T((92, 56), (97, 86), (99, 86), (97, 56))], GLASS)])
    # 운전대
    blob(img, [fr.L([(74, 64), (86, 52)], 2.6)], mix(TIRE, WHITE, 0.2), r=0, line=0.5)
    blob(img, [fr.E(68, 60, 78, 74)], mix(TIRE, WHITE, 0.15), r=0, line=0.5)
    # 짐칸 바닥 안쪽 (벽 위로 살짝 보이는 그늘)
    blob(img, [fr.T((-114, 38), (16, 38), (16, 46), (-114, 46))], mix(TRUCK, LINE, 0.45), r=0.4, line=0.6)
    # 몸통: 짐칸 벽 (낮음) + 운전석 문 (높음) + 보닛
    body = [(-116, 14), (-118, 44), (-110, 46), (14, 46), (18, 40), (34, 40), (36, 58), (84, 58), (108, 52), (118, 40), (120, 24), (114, 12), (-112, 12)]
    blob(img, [fr.T(*body)], TRUCK, r=1.8,
         marks=[([fr.L([(-112, 41), (12, 41)], 2.4), fr.L([(40, 53), (84, 53), (106, 47)], 2.4)], mix(TRUCK, WHITE, 0.45)),
                ([fr.L([(26, 16), (26, 40)], 1.4), fr.L([(84, 18), (84, 56)], 1.0)], mix(TRUCK, LINE, 0.35)),
                ([fr.L([(50, 40), (58, 40)], 2.2)], mix(TRUCK, LINE, 0.45)),
                ([fr.T((-118, 12), (120, 12), (120, 20), (-118, 20))], mix(TRUCK, LINE, 0.18)),
                ([fr.L([(-104, 30), (0, 30)], 1.0)], mix(TRUCK, LINE, 0.25))])
    lamp(img, fr, 112, 32, 6, HEADLIGHT)
    lamp(img, fr, -114, 36, 4.5, TAIL)
    smile(img, fr, 118, 18, 10)
    wheel(img, fr, -72, 19, 19, spin)
    wheel(img, fr, 72, 19, 19, spin)


def draw_scooter(img, fr, n):
    """🛴 킥보드: 민트색 발판, 비스듬한 손잡이 대, T자 손잡이와 따릉 종, 작은 바퀴 둘"""
    spin = n * 40
    # 손잡이 대 (앞바퀴에서 위로)
    blob(img, [fr.L([(36, 12), (44, 104)], 5.0)], METAL, r=0, line=0.8,
         marks=[([fr.L([(38, 20), (44, 96)], 1.6)], mix(METAL, WHITE, 0.5))])
    # 손잡이 (T)와 고무 손잡이
    blob(img, [fr.L([(30, 106), (58, 106)], 4.6)], METAL, r=0, line=0.8)
    blob(img, [fr.L([(28, 106), (34, 106)], 6.0), fr.L([(54, 106), (60, 106)], 6.0)], SCOOTER, r=0, line=0.7)
    # 따릉 종
    blob(img, [fr.C(48, 112, 4.2)], (252, 214, 84), r=0, line=0.6, marks=[([fr.C(47, 113, 1.4)], WHITE)])
    # 발판
    blob(img, [fr.T((-46, 12), (32, 12), (36, 18), (-42, 22))], SCOOTER, r=1.6,
         marks=[([fr.L([(-40, 19), (28, 17)], 1.8)], mix(SCOOTER, WHITE, 0.45)),
                ([fr.L([(-30, 14), (20, 14)], 1.2)], mix(SCOOTER, LINE, 0.3))])
    # 뒷바퀴 흙받이
    blob(img, [fr.L([(-54, 14), (-48, 24), (-36, 24)], 2.4)], mix(SCOOTER, LINE, 0.2), r=0, line=0.6)
    wheel(img, fr, -40, 10, 10, spin, rim=mix(SCOOTER, WHITE, 0.4))
    wheel(img, fr, 38, 10, 10, spin, rim=mix(SCOOTER, WHITE, 0.4))


def draw_heli(img, fr, n):
    """🚁 동글한 헬리콥터: 노란 몸통, 앞이 트인 유리 조종석, 꼬리와 꼬리 프로펠러, 썰매 다리, 위 프로펠러 (n칸째 돎)"""
    # 썰매 다리
    blob(img, [fr.L([(-48, 4), (52, 4), (60, 10)], 3.2), fr.L([(-24, 4), (-18, 22)], 2.6), fr.L([(30, 4), (26, 22)], 2.6)], METAL, r=0, line=0.8)
    # 꼬리 대와 꼬리 날개
    blob(img, [fr.T((-40, 52), (-98, 70), (-100, 80), (-36, 78))], HELI, r=1.2,
         marks=[([fr.L([(-44, 66), (-96, 74)], 2.0)], HELI_STRIPE)])
    blob(img, [fr.T((-104, 66), (-96, 64), (-90, 96), (-98, 98))], HELI_STRIPE, r=1.0, line=0.8)
    # 꼬리 프로펠러 (돎)
    tail = [fr.L([rot(-100, 82, 12, n * 60 + 120 * i), rot(-100, 82, 12, n * 60 + 120 * i + 180)], 2.2) for i in range(2)]
    blob(img, tail, mix(METAL, LINE, 0.2), r=0, line=0.5)
    blob(img, [fr.C(-100, 82, 3)], mix(METAL, LINE, 0.4), r=0, line=0.5)
    # 프로펠러 대
    blob(img, [fr.L([(14, 96), (14, 132)], 4.0)], mix(METAL, LINE, 0.25), r=0, line=0.7)
    # 몸통: 아래는 노란 통, 위 앞은 트인 유리 조종석 (펫이 앉음)
    blob(img, [fr.E(-44, 16, 72, 104)], HELI, r=1.6,
         marks=[([fr.T((-6, 52), (80, 52), (80, 108), (-6, 108))], GLASS),
                ([fr.E(30, 74, 46, 96)], WHITE),
                ([fr.L([(-40, 40), (66, 40)], 3.0)], HELI_STRIPE),
                ([fr.L([(-6, 52), (70, 52)], 1.6)], mix(HELI, LINE, 0.4)),
                ([fr.E(-30, 66, -14, 82)], GLASS)])
    lamp(img, fr, 68, 32, 4.5, HEADLIGHT)
    # 위 프로펠러: 칸마다 날개가 돌아 길이가 바뀜 (옆에서 보면)
    reach = (100, 56, 16)[n % 3]
    blade = mix(METAL, LINE, 0.35)
    blob(img, [fr.E(14 - reach * 0.92, 130, 14 + reach, 138)], blade, r=0, line=0.6,
         marks=[([fr.L([(14 - reach * 0.8, 135), (14 + reach * 0.85, 135)], 1.2)], mix(blade, WHITE, 0.4))])
    blob(img, [fr.E(6, 128, 22, 140)], mix(METAL, LINE, 0.15), r=0, line=0.7)


DRAW = {"car": draw_car, "heli": draw_heli, "scooter": draw_scooter, "truck": draw_truck}


def draw_frame(name, n):
    """탈것 name의 n번째 칸 (창 크기 × OUT px)"""
    fr = Frame(name)
    # 경계 번짐을 내보내는 px 기준으로 (draw_pets.aa — 칸 하나가 OUT/f px로 나감)
    with hires(), view(scale=1.0, lift=0), draw_pets.output(OUT / fr.f):
        img = canvas()
        DRAW[name](img, fr, n)
        return fr.crop(img)


def draw_sheet(name):
    """가로로 FRAMES칸 이은 시트"""
    frames = [draw_frame(name, n) for n in range(FRAMES)]
    w, h = frames[0].size
    sheet = Image.new("RGBA", (w * FRAMES, h), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        sheet.alpha_composite(f, (i * w, 0))
    return sheet


def build(out_dir=OUT_DIR):
    """시트를 모두 그려 저장함"""
    out_dir = Path(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    arts = {}
    for name in NAMES:
        arts[name] = draw_sheet(name)
        arts[name].save(out_dir / f"{name}.png", optimize=True)
    return arts


def pet_frame(row, col=0, pid="cat", stage="teenager"):
    """번들 시트에서 동물 한 칸 (128 px — 큰 칸이면 줄임)"""
    import json
    folder = ROOT / "ui" / "pets" / pid
    meta = json.loads((folder / "pet.json").read_text(encoding="utf-8"))
    px = meta.get("frame_sizes", {}).get(stage, meta.get("frame_size", FRAME))
    sheet = Image.open(folder / f"sheet_{stage}.png").convert("RGBA")
    cell = sheet.crop((col * px, row * px, (col + 1) * px, (row + 1) * px))
    return cell.resize((FRAME, FRAME), Image.LANCZOS) if px != FRAME else cell, meta["animations"]


def seated(name, art, pet_size=128):
    """미리보기: 앱 크기 (논리 px)로 줄인 탈것 첫 칸에 고양이를 좌석마다 앉혀 봄 (다리는 문 아래로 잘림)"""
    ww, wh = window(name)
    frame = art.crop((0, 0, ww * OUT, wh * OUT)).resize((ww, wh), Image.LANCZOS)
    out = Image.new("RGBA", (ww, wh + pet_size), (0, 0, 0, 0))
    ground = wh + pet_size - UNDER
    _, anims = pet_frame(0)
    sx0, cover0, low0 = SEAT[name]
    seats = [((max(sx0, WHEEL[name] - DRIVER_REACH * pet_size), cover0, low0), anims["idle"]["row"])]
    if name in BED:
        xs, cover = BED[name]
        seats += [((x, cover, SEAT[name][2] - 2), anims["sit_front"]["row"]) for x in xs]
    pets = []
    for (sx, cover, low), row in seats:
        pet, _ = pet_frame(row)
        pet = pet.resize((pet_size, pet_size), Image.LANCZOS)
        feet = low if cover <= 0 else min(cover, max(0, cover - LEG_SHARE * pet_size))
        hidden = 0 if cover <= 0 else cover - feet
        if hidden > 0:
            keep = pet_size - round(hidden)
            pet = pet.crop((0, 0, pet_size, keep))
        pets.append((pet, (round(ww / 2 + sx - pet_size / 2), round(ground - feet - pet_size))))
    out.alpha_composite(frame, (0, pet_size))
    # 펫 창은 탈것 창 위에 뜸 (가려지는 다리는 잘림)
    for pet, at in pets:
        out.alpha_composite(pet, at)
    return out


def preview(arts, path=PREVIEW):
    """윗줄: 탈것 세 칸 (앱 크기), 아랫줄: 고양이를 태운 모습"""
    cols = []
    for name in NAMES:
        ww, wh = window(name)
        strip = arts[name].resize((ww * FRAMES, wh), Image.LANCZOS)
        ride = seated(name, arts[name])
        col = Image.new("RGBA", (max(strip.width, ride.width), strip.height + ride.height + 16), (0, 0, 0, 0))
        col.alpha_composite(strip, (0, 0))
        col.alpha_composite(ride, (0, strip.height + 16))
        cols.append(col)
    w = sum(c.width for c in cols) + 24 * (len(cols) + 1)
    h = max(c.height for c in cols) + 48
    im = Image.new("RGBA", (w, h), (246, 242, 236, 255))
    x = 24
    for c in cols:
        im.alpha_composite(c, (x, 24))
        x += c.width + 24
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    im.save(path)
    return im


def main():
    arts = build()
    path = PREVIEW
    if "--preview" in sys.argv:
        path = Path(sys.argv[sys.argv.index("--preview") + 1])
    preview(arts, path)
    print(f"wrote {len(arts)} sheets to {OUT_DIR} and {path}")


if __name__ == "__main__":
    main()
