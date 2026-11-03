"""수면 모자 미리보기 — 진짜 시트 위에 ui/index.html의 진짜 모자 SVG를 앱과 같은 계산으로 얹어 본다 (브라우저 없이).

    python tools/preview_nightcap.py after  [--out tools/preview/nightcap_after.png] [--pages DIR]
    python tools/preview_nightcap.py before [--out tools/preview/nightcap_before.png] [--pages DIR]

before = 고치기 전 main.js의 머리 찾기(가장 큰 덩어리의 맨 위 줄)를 그대로 옮긴 것 — 버그를 다시 보려고 남겨 둔다.
after  = pet.json head 표 + ui/main.js capPlacement 계산 (test_nightcap.py도 같은 계산을 쓴다).
--pages: 동물마다 큰 그림(5단계 × 대표 동작 + 청소년 모든 동작)을 따로 저장한다.
"""

import argparse
import json
import math
import re
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
PETS = ROOT / "ui" / "pets"
FRAME = 128
STAGES = ["baby", "kinder", "child", "teenager", "adult"]
STAGE_SCALE = {"baby": 0.45, "kinder": 0.60, "child": 0.78, "teenager": 1.00, "adult": 1.15}
SPECIES = ["cat", "dog", "rabbit", "bear", "bee", "frog", "pumpkin", "ghost", "duck", "giraffe", "rudolph", "santa", "snowman",
           "dino_yellow", "dino_green", "dino_purple"]
FRONT_ANIMS = {"sit_front", "wave", "tilt", "belly", "stretch", "heart", "tickle", "wag", "hungry", "eat", "level_up", "huff", "sting"}
# 대표 칸 (동작, 프레임) — 모자가 가장 자주 보이는 자세부터
SHOWCASE = [("sleep", 0), ("sleep", 1), ("idle", 0), ("walk", 1), ("sit_front", 0), ("tilt", 1), ("look", 1), ("lookup", 2), ("hop", 1),
            ("annoyed", 0), ("eat", 1), ("fall", 0), ("climb", 1), ("level_up", 2)]

CAP_BOX = (34, 26)


# ---- 모자 그림: ui/index.html의 SVG 경로를 그대로 읽어 다각형으로 ----
def _bezier(p0, p1, p2, p3=None, n=16):
    out = []
    for i in range(1, n + 1):
        t = i / n
        if p3 is None:
            out.append(((1 - t) ** 2 * p0[0] + 2 * (1 - t) * t * p1[0] + t ** 2 * p2[0], (1 - t) ** 2 * p0[1] + 2 * (1 - t) * t * p1[1] + t ** 2 * p2[1]))
        else:
            out.append(tuple((1 - t) ** 3 * p0[j] + 3 * (1 - t) ** 2 * t * p1[j] + 3 * (1 - t) * t ** 2 * p2[j] + t ** 3 * p3[j] for j in (0, 1)))
    return out


def path_points(d):
    tok = re.findall(r"[MCQLZ]|-?[\d.]+", d)
    pts, i, cur = [], 0, None
    while i < len(tok):
        c = tok[i]
        if c == "Z":
            i += 1
            continue
        if c in "ML":
            cur = (float(tok[i + 1]), float(tok[i + 2]))
            pts.append(cur)
            i += 3
        elif c == "Q":
            ctrl = (float(tok[i + 1]), float(tok[i + 2]))
            end = (float(tok[i + 3]), float(tok[i + 4]))
            pts += _bezier(cur, ctrl, end)
            cur = end
            i += 5
        elif c == "C":
            vals = [float(v) for v in tok[i + 1:i + 7]]
            pts += _bezier(cur, (vals[0], vals[1]), (vals[2], vals[3]), (vals[4], vals[5]))
            cur = (vals[4], vals[5])
            i += 7
        else:  # 숫자만 이어진 C/Q 반복은 이 그림에 없다
            raise ValueError(f"path 토큰 {c}")
    return pts


def cap_shapes():
    """[(점들, 채움, 선, 선 굵기)] + 원 (cx, cy, r, 채움, 선, 굵기) — 그리는 순서대로."""
    html = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
    svg = re.search(r'<svg id="nightcap".*?</svg>', html, re.S).group(0)
    out = []
    for m in re.finditer(r'<path d="([^"]+)" fill="([^"]+)" stroke="([^"]+)" stroke-width="([\d.]+)"', svg):
        out.append(("poly", path_points(m.group(1)), m.group(2), m.group(3), float(m.group(4))))
    for m in re.finditer(r'<circle cx="([\d.]+)" cy="([\d.]+)" r="([\d.]+)" fill="([^"]+)" stroke="([^"]+)" stroke-width="([\d.]+)"', svg):
        cx, cy, r = float(m.group(1)), float(m.group(2)), float(m.group(3))
        out.append(("circle", [(cx + r * math.cos(a / 24 * math.tau), cy + r * math.sin(a / 24 * math.tau)) for a in range(24)], m.group(4), m.group(5), float(m.group(6))))
    return out


CAP = cap_shapes()


def draw_cap(mat, canvas_size, zoom, ss=4):
    """mat(px, py) → 그림 좌표. zoom 배로 그린 RGBA 층."""
    w, h = canvas_size
    layer = Image.new("RGBA", (w * ss, h * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    # 칸 배율은 첫 벡터의 길이로 잰다
    ax, ay = mat(0, 0)
    bx, by = mat(1, 0)
    unit = math.hypot(bx - ax, by - ay) * ss
    for _, pts, fill, stroke, sw in CAP:
        poly = [tuple(v * ss for v in mat(*p)) for p in pts]
        d.polygon(poly, fill=fill)
        d.line(poly + [poly[0]], fill=stroke, width=max(1, round(sw * unit)), joint="curve")
    return layer.resize((w, h), Image.LANCZOS)


# ---- 고치기 전 계산 (main.js headTop + placeHeadgear를 그대로 옮김) ----
def old_head_top(cell_alpha, front):
    from scipy import ndimage
    size = cell_alpha.shape[0]
    lab, n = ndimage.label(cell_alpha > 128)
    if n == 0:
        return (size / 2, size * 0.15)
    counts = np.bincount(lab.ravel())[1:]
    ys, xs = np.nonzero(lab == int(np.argmax(counts)) + 1)
    lo, hi = xs.min(), xs.max()
    start = lo if front else hi - (hi - lo) * 0.45
    keep = xs >= start
    xs, ys = xs[keep], ys[keep]
    first = ys.min()
    band = xs[ys < first + size * 0.08]
    return (float(band.sum()) / band.size, int(first))


def old_cap_matrix(cell_alpha, anim, stage, flip, side, stage_w):
    """스테이지 좌표(왼쪽 위 0,0 — 높이 36 + side)로 옮기는 함수를 돌려준다. CSS: left 0, bottom 0, origin 50% 100%."""
    k = side / FRAME
    hx, hy = old_head_top(cell_alpha, anim in FRONT_ANIMS)
    x = (FRAME - hx if flip else hx) * k + (stage_w - side) / 2
    y = side - hy * k
    tilt = 12 if flip else -12
    size = math.sqrt(STAGE_SCALE[stage])
    tx, ty = x - 15, -y + 8 * size
    rad = math.radians(tilt)
    c, s = math.cos(rad), math.sin(rad)
    stage_h = 36 + side
    ox, oy = CAP_BOX[0] / 2, CAP_BOX[1]

    def m(px, py):
        qx, qy = px - ox, py - oy
        if not flip:
            qx = -qx
        rx, ry = c * qx - s * qy, s * qx + c * qy
        rx, ry = rx * size, ry * size
        return (rx + ox + tx, ry + stage_h - CAP_BOX[1] + oy + ty)

    return m


# ---- 고친 뒤 계산: pet.json head 표 → ui/main.js capPlacement ----
def manifest(pid):
    return json.loads((PETS / pid / "pet.json").read_text(encoding="utf-8"))


CAP_BRIM_W = 27.0  # 모자 입구 너비 (viewBox 3..30) — main.js CAP_BRIM_W
CAP_BRIM_Y = 20.5  # 모자 입구 가운데 (viewBox) — 이 점이 표의 (x, y)에 온다
CAP_BRIM_X = 16.5
CAP_TILT = -8.0  # 끝이 뒤로 처진다 (칸은 오른쪽을 본다)


def new_cap_matrix_cell(entry):
    """main.js headgearLayout의 cap과 같은 식: svg 점 → 칸 좌표. test_nightcap.py가 실제 JS 결과와 대조한다."""
    x, y, w, r = entry[:4]
    sc = w / CAP_BRIM_W
    rad = math.radians(r + CAP_TILT)
    c, s = math.cos(rad), math.sin(rad)

    def m(px, py):
        qx, qy = -(px - CAP_BRIM_X) * sc, (py - CAP_BRIM_Y) * sc
        return (c * qx - s * qy + x, s * qx + c * qy + y)

    return m


def hang_grip_cell(cells):
    """커서 끝이 닿는 칸 좌표 — 코어 PetManifest::hang_grip · main.js hangOrigin과 같은 계산."""
    n = len(cells)
    mean = [sum(c[i] for c in cells) / n for i in (0, 1, 2)]
    return mean[0], max(0.0, mean[1] - 0.15 * mean[2])


def head_entry(man, stage, anim, f):
    return man["head"][stage][anim][f]


# ---- 그리기 ----
def compose_cell(sheet, spec, f, pid, man, stage, flip, zoom, mode):
    """한 칸: 진짜 표시 크기 × zoom. 위에 말풍선 자리 없이 칸 + 위 여백만."""
    side = 128 * STAGE_SCALE[stage] * man["stage_size"].get(stage, 1.0)
    cell = sheet.crop((f * FRAME, spec["row"] * FRAME, (f + 1) * FRAME, (spec["row"] + 1) * FRAME))
    stage_w = 128 * max(1.0, side / 128)
    W = round(stage_w * zoom)
    pad_top = 36
    H = round((side + pad_top) * zoom)
    out = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    big = cell.resize((round(side * zoom), round(side * zoom)), Image.LANCZOS)
    if flip:
        big = big.transpose(Image.FLIP_LEFT_RIGHT)
    left = (stage_w - side) / 2
    out.alpha_composite(big, (round(left * zoom), H - big.height))
    if mode == "before":
        alpha = np.asarray(cell.getchannel("A"))
        m0 = old_cap_matrix(alpha, spec["_anim"], stage, flip, side, stage_w)
        m = lambda px, py: tuple(v * zoom for v in m0(px, py))  # noqa: E731
    else:
        entry = head_entry(man, stage, spec["_anim"], f)
        mc = new_cap_matrix_cell(entry)
        k = side / FRAME

        def m(px, py):
            cx, cy = mc(px, py)
            if flip:
                cx = FRAME - cx
            return ((left + cx * k) * zoom, (pad_top + cy * k) * zoom)

    out.alpha_composite(draw_cap(m, (W, H), zoom))
    return out


def label_font():
    return ImageFont.load_default()


def page(pid, mode, zoom=1):
    """동물 한 장: 5단계 × 대표 칸 (왼쪽을 볼 때도 한 칸)."""
    man = manifest(pid)
    cols = [(a, f, False) for a, f in SHOWCASE if a in man["animations"] and f < man["animations"][a]["frames"]]
    cols += [("sleep", 0, True), ("idle", 0, True)]
    cell_w, cell_h = 150 * zoom, 186 * zoom
    img = Image.new("RGBA", (64 + len(cols) * cell_w, 16 + len(STAGES) * cell_h), (236, 240, 246, 255))
    d = ImageDraw.Draw(img)
    for c, (a, f, flip) in enumerate(cols):
        d.text((64 + c * cell_w + 2, 2), f"{a} {f}{' <' if flip else ''}", fill=(60, 60, 60), font=label_font())
    for r, st in enumerate(STAGES):
        sheet = Image.open(PETS / pid / f"sheet_{st}.png").convert("RGBA")
        d.text((2, 16 + r * cell_h + cell_h // 2), f"{pid}\n{st}", fill=(60, 60, 60), font=label_font())
        for c, (a, f, flip) in enumerate(cols):
            spec = dict(man["animations"][a], _anim=a)
            tile = compose_cell(sheet, spec, f, pid, man, st, flip, zoom, mode)
            img.alpha_composite(tile, (64 + c * cell_w + (cell_w - tile.width) // 2, 16 + r * cell_h + cell_h - tile.height))
    return img


def all_anims_page(pid, stage, mode, zoom=1):
    """한 단계의 모든 동작 모든 칸."""
    man = manifest(pid)
    sheet = Image.open(PETS / pid / f"sheet_{stage}.png").convert("RGBA")
    names = list(man["animations"])
    cell_w, cell_h = 150 * zoom, 186 * zoom
    cols = 12
    cells = [(a, f) for a in names for f in range(man["animations"][a]["frames"])]
    rows = math.ceil(len(cells) / cols)
    img = Image.new("RGBA", (cols * cell_w, rows * (cell_h + 14)), (236, 240, 246, 255))
    d = ImageDraw.Draw(img)
    for i, (a, f) in enumerate(cells):
        x, y = (i % cols) * cell_w, (i // cols) * (cell_h + 14)
        d.text((x + 2, y), f"{a} {f}", fill=(60, 60, 60), font=label_font())
        tile = compose_cell(sheet, dict(man["animations"][a], _anim=a), f, pid, man, stage, False, zoom, mode)
        img.alpha_composite(tile, (x + (cell_w - tile.width) // 2, y + 14 + cell_h - tile.height))
    return img


def _pet_tile(pid, stage, anim, f, zoom, man=None, sheet=None, head_entry_override=None):
    """#pet 한 칸 (스프라이트 + 자식 모자)을 칸 크기 × zoom으로 — 몸의 CSS 변환은 이 그림 전체에 걸린다 (앱의 부모-자식과 같다)."""
    man = man or manifest(pid)
    sheet = sheet or Image.open(PETS / pid / f"sheet_{stage}.png").convert("RGBA")
    row = man["animations"][anim]["row"]
    cell = sheet.crop((f * FRAME, row * FRAME, (f + 1) * FRAME, (row + 1) * FRAME)).resize((FRAME * zoom, FRAME * zoom), Image.LANCZOS)
    pad = 60 * zoom
    out = Image.new("RGBA", (FRAME * zoom + 2 * pad, FRAME * zoom + 2 * pad), (0, 0, 0, 0))
    out.alpha_composite(cell, (pad, pad))
    mc = new_cap_matrix_cell(head_entry(man, stage, anim, f))
    m = lambda px, py: tuple(v * zoom + pad for v in mc(px, py))  # noqa: E731
    out.alpha_composite(draw_cap(m, out.size, zoom))
    return out, pad


def _css(tile, pad, zoom, origin, ops):
    """ops: [("translate", x, y) | ("rotate", deg) | ("flip",) | ("scale", k)]를 CSS 순서대로 origin(칸 px) 기준으로 건다."""
    a, b, c, d, e, f = 1.0, 0.0, 0.0, 0.0, 1.0, 0.0  # 그림 → 화면

    def mul(m):
        nonlocal a, b, c, d, e, f
        a2, b2, c2, d2, e2, f2 = m
        a, b, c, d, e, f = a * a2 + b * d2, a * b2 + b * e2, a * c2 + b * f2 + c, d * a2 + e * d2, d * b2 + e * e2, d * c2 + e * f2 + f

    ox, oy = origin[0] * zoom + pad, origin[1] * zoom + pad
    mul((1, 0, ox, 0, 1, oy))
    for op in ops:
        if op[0] == "translate":
            mul((1, 0, op[1] * zoom, 0, 1, op[2] * zoom))
        elif op[0] == "rotate":
            r = math.radians(op[1])
            mul((math.cos(r), -math.sin(r), 0, math.sin(r), math.cos(r), 0))
        elif op[0] == "flip":
            mul((-1, 0, 0, 0, 1, 0))
        else:
            mul((op[1], 0, 0, 0, op[1], 0))
    mul((1, 0, -ox, 0, 1, -oy))
    det = a * e - b * d
    inv = (e / det, -b / det, (b * f - e * c) / det, -d / det, a / det, (d * c - a * f) / det)
    return tile.transform(tile.size, Image.AFFINE, inv, Image.BICUBIC)


ROLL_ORIGIN = (64, 88)  # style.css transform-origin 50% 68.75%


def _neck_tile(pid, stage, f, ext_part, zoom):
    """기린 목 늘이기: 몸 층 + 목 토막 + 머리 층(모자 자식 포함)을 앱과 같은 계산으로."""
    man = manifest(pid)
    meta = man["neck_stretch"][stage]
    layer = Image.open(PETS / pid / f"neck_{stage}.png").convert("RGBA")
    tile_img = Image.open(PETS / pid / f"neck_tile_{stage}.png").convert("RGBA")
    pad = 60 * zoom
    side = FRAME * zoom
    out = Image.new("RGBA", (side + 2 * pad, side + 2 * pad), (0, 0, 0, 0))
    body = layer.crop((0, 0, FRAME, FRAME)).resize((side, side), Image.LANCZOS)
    out.alpha_composite(body, (pad, pad))
    ext = round(meta["ext"] * ext_part)
    for y in range(0, ext, meta["tile"]):
        piece = tile_img.resize((side, meta["tile"] * zoom), Image.LANCZOS)
        out.alpha_composite(piece, (pad, pad + (meta["cut"] - ext + y) * zoom))
    head = layer.crop(((f + 1) * FRAME, 0, (f + 2) * FRAME, FRAME)).resize((side, side), Image.LANCZOS)
    out.alpha_composite(head, (pad, pad - ext * zoom))
    mc = new_cap_matrix_cell(head_entry(man, stage, "neckstretch", f))
    m = lambda px, py: (mc(px, py)[0] * zoom + pad, (mc(px, py)[1] - ext) * zoom + pad)  # noqa: E731
    out.alpha_composite(draw_cap(m, out.size, zoom))
    return out


CURSOR = [(0, 0), (0, 16), (4, 12), (7, 18), (9, 17), (6, 11), (11, 11)]


def hang_page(path, zoom=1):
    """커서 끝에 매달린 모습 (모든 동물 × 5단계, 몸은 커서 끝을 축으로 ±6도, 모자는 자식이라 함께): 빨간 십자 칸 = 커서 끝."""
    cell_w, cell_h = 230, 210
    img = Image.new("RGBA", (70 + len(SPECIES) * cell_w, 16 + 5 * cell_h), (236, 240, 246, 255))
    d = ImageDraw.Draw(img)
    for c, pid in enumerate(SPECIES):
        man = manifest(pid)
        d.text((70 + c * cell_w + 2, 2), pid, fill=(60, 60, 60), font=label_font())
        for r, st in enumerate(STAGES):
            if c == 0:
                d.text((2, 16 + r * cell_h + 90), st, fill=(60, 60, 60), font=label_font())
            k = STAGE_SCALE[st] * man["stage_size"].get(st, 1.0)
            tile, pad = _pet_tile(pid, st, "dragged", (r + c) % man["animations"]["dragged"]["frames"], 1)
            gx, gy = hang_grip_cell(man["head"][st]["dragged"])
            swung = _css(tile, pad, 1, (gx, gy), [("rotate", -6 if (r + c) % 2 else 6)])
            small = swung.resize((round(swung.width * k), round(swung.height * k)), Image.LANCZOS)
            # 커서 끝 = 칸 안 (gx, gy)
            tip_x, tip_y = 70 + c * cell_w + 100, 16 + r * cell_h + 12
            img.alpha_composite(small, (round(tip_x - (gx + pad) * k), round(tip_y - (gy + pad) * k)))
            d.polygon([(tip_x + x, tip_y + y) for x, y in CURSOR], fill=(20, 20, 20), outline=(255, 255, 255))
    img.convert("RGB").save(path, optimize=True)


def scenarios(path, zoom=1):
    """몸과 한 덩어리로 움직이는지: 매달림 · 구르기 회전 · 덜덜 · 반전 · 쏘기 · 뿅 · 목 늘이기 (모자 자식에 몸 변환을 그대로 건 그림)."""
    pets = [("cat", "teenager"), ("dino_green", "adult"), ("santa", "child"), ("bee", "teenager"), ("frog", "teenager"), ("giraffe", "teenager")]
    cols = ["hang >", "hang <", "roll 0", "roll 90", "roll 180", "roll 270", "wobble", "flip", "poof", "sting", "neck 50%", "neck 100%"]
    cell = (FRAME + 120) * zoom
    img = Image.new("RGBA", (70 + len(cols) * cell, 16 + len(pets) * cell), (236, 240, 246, 255))
    d = ImageDraw.Draw(img)
    for c, name in enumerate(cols):
        d.text((70 + c * cell + 4, 2), name, fill=(60, 60, 60), font=label_font())
    for r, (pid, st) in enumerate(pets):
        d.text((2, 16 + r * cell + cell // 2), f"{pid}\n{st}", fill=(60, 60, 60), font=label_font())
        man = manifest(pid)
        base, pad = _pet_tile(pid, st, "idle", 0, zoom)
        roll, _ = _pet_tile(pid, st, "roll", 0, zoom)
        spin = "spin" in man["animations"]["roll"]
        sheets = {
            "hang >": _css(base, pad, zoom, hang_grip_cell(man["head"][st]["dragged"]), [("rotate", 6)]),
            "hang <": _css(base, pad, zoom, hang_grip_cell(man["head"][st]["dragged"]), [("rotate", -6)]),
            "wobble": _css(base, pad, zoom, ROLL_ORIGIN, [("translate", 2.5, 1.2)]),
            "flip": _css(base, pad, zoom, (64, 64), [("flip",)]),
            "poof": _css(base, pad, zoom, (64, 64), [("scale", 0.55)]),
        }
        for deg in (0, 90, 180, 270):
            sheets[f"roll {deg}"] = _css(roll, pad, zoom, ROLL_ORIGIN, [("rotate", deg if spin else 0)])
        if "sting" in man["animations"]:
            t, _ = _pet_tile(pid, st, "sting", 2, zoom)
            sheets["sting"] = _css(t, pad, zoom, ROLL_ORIGIN, [("scale", 1.2)])
        if "neck_stretch" in man:
            sheets["neck 50%"] = _neck_tile(pid, st, 0, 0.5, zoom)
            sheets["neck 100%"] = _neck_tile(pid, st, 2, 1.0, zoom)
        for c, name in enumerate(cols):
            if name in sheets:
                img.alpha_composite(sheets[name], (70 + c * cell, 16 + r * cell))
    img.convert("RGB").save(path, optimize=True)


def contact(mode, path):
    """동물 × 단계 전부 (대표 칸) — 세로로 길다."""
    pages = [page(pid, mode) for pid in SPECIES]
    w = max(p.width for p in pages)
    img = Image.new("RGBA", (w, sum(p.height for p in pages)), (236, 240, 246, 255))
    y = 0
    for p in pages:
        img.alpha_composite(p, (0, y))
        y += p.height
    img.convert("RGB").save(path, optimize=True)
    return pages


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["before", "after"])
    ap.add_argument("--out")
    ap.add_argument("--pages", help="동물마다 따로 저장할 폴더")
    a = ap.parse_args()
    out = Path(a.out or ROOT / "tools" / "preview" / f"nightcap_{a.mode}.png")
    pages = contact(a.mode, out)
    if a.mode == "after":
        scenarios(out.with_name("nightcap_moves.png"))
        hang_page(out.with_name("nightcap_hang.png"))
    if a.pages:
        d = Path(a.pages)
        d.mkdir(parents=True, exist_ok=True)
        for pid, p in zip(SPECIES, pages):
            p.convert("RGB").save(d / f"{a.mode}_{pid}.png")
            all_anims_page(pid, "teenager", a.mode).convert("RGB").save(d / f"{a.mode}_{pid}_all.png")
    print("ok", out)


if __name__ == "__main__":
    sys.exit(main())
