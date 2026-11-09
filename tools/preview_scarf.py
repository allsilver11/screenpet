"""산타 밤 목도리 미리보기 — 진짜 시트 위에 ui/index.html의 진짜 목도리 SVG를 앱과 같은 계산(main.js scarfLayout)으로 얹어 본다.

    python tools/preview_scarf.py [--out tools/preview/santa_scarf.png]

위 줄부터 대표 동작 × 5단계, 앱에서 그린 크기의 3배. test_nightcap.py도 같은 계산을 쓴다.
"""

import argparse
import json
import math
import re
from pathlib import Path

from PIL import Image, ImageDraw

from preview_nightcap import path_points
from sprite_spec import FRAME, STAGES, frame_px

ROOT = Path(__file__).resolve().parent.parent
FRONT_ANIMS = {"sit_front", "wave", "tilt", "belly", "stretch", "heart", "tickle", "hungry", "eat", "level_up", "huff"}
SHOWCASE = [("sleep", 0), ("sleep", 1), ("idle", 0), ("walk", 0), ("walk", 2), ("sit_front", 0), ("wave", 0), ("eat", 1), ("heart", 0), ("hungry", 1), ("tilt", 1),
            ("belly", 0), ("level_up", 2), ("lookup", 2), ("hop", 1), ("fall", 0), ("dragged", 0), ("climb", 0), ("climb", 2), ("slide", 0), ("roll", 0), ("roll", 1)]

# main.js scarfLayout과 같은 값 (test_nightcap.py가 실제 JS 결과와 대조한다)
SCARF_ANCHOR_X, SCARF_ANCHOR_Y, SCARF_W = 26.0, 8.0, 52.0


def manifest():
    return json.loads((ROOT / "ui" / "pets" / "santa" / "pet.json").read_text(encoding="utf-8"))


def scarf_groups():
    """{보기: [(점들, 채움, 선, 굵기)]} — ui/index.html <svg id="scarf"> 안의 <g data-v="보기">"""
    html = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
    svg = re.search(r'<svg id="scarf".*?</svg>', html, re.S).group(0)
    out = {}
    for g in re.finditer(r'<g data-v="(\w+)">(.*?)</g>', svg, re.S):
        out[g.group(1)] = [(path_points(m.group(1)), m.group(2), m.group(3), float(m.group(4)))
                           for m in re.finditer(r'<path d="([^"]+)" fill="([^"]+)" stroke="([^"]+)" stroke-width="([\d.]+)"', g.group(2))]
    return out


GROUPS = scarf_groups()


def scarf_view(anim, f):
    """main.js scarfView와 같다."""
    if anim == "roll":
        return f"sled{f % 2}"
    if anim in ("climb", "slide"):
        return "back"
    return "front" if anim in FRONT_ANIMS else "side"


def scarf_matrix(entry):
    """main.js scarfLayout과 같은 식: svg 점 → 칸 좌표."""
    x, y, w, r = entry
    s = w / SCARF_W
    rad = math.radians(r)
    c, sn = math.cos(rad), math.sin(rad)

    def m(px, py):
        qx, qy = (px - SCARF_ANCHOR_X) * s, (py - SCARF_ANCHOR_Y) * s
        return (c * qx - sn * qy + x, sn * qx + c * qy + y)

    return m


def draw_scarf(view, mat, zoom, size, ss=4):
    layer = Image.new("RGBA", (size * ss, size * ss), (0, 0, 0, 0))
    d = ImageDraw.Draw(layer)
    ax, ay = mat(0, 0)
    bx, by = mat(1, 0)
    unit = math.hypot(bx - ax, by - ay) * zoom * ss
    for pts, fill, stroke, sw in GROUPS[view]:
        poly = [tuple(v * zoom * ss for v in mat(*p)) for p in pts]
        if fill != "none":
            d.polygon(poly, fill=fill)
        if sw:
            d.line(poly + [poly[0]], fill=stroke, width=max(1, round(sw * unit)), joint="curve")
    return layer.resize((size, size), Image.LANCZOS)


def sheet_row(sheet_cache, man, st, anim):
    if st not in sheet_cache:
        sheet = Image.open(ROOT / "ui" / "pets" / "santa" / f"sheet_{st}.png").convert("RGBA")
        px = frame_px("santa", st)
        sheet_cache[st] = sheet.resize((sheet.width * FRAME // px, sheet.height * FRAME // px), Image.LANCZOS) if px != FRAME else sheet
    return sheet_cache[st], man["animations"][anim]["row"]


def tile(cache, man, st, anim, f, px):
    sheet, row = sheet_row(cache, man, st, anim)
    big = FRAME * 3
    cell = sheet.crop((f * FRAME, row * FRAME, (f + 1) * FRAME, (row + 1) * FRAME)).resize((big, big), Image.LANCZOS)
    entry = [v * FRAME / frame_px("santa", st) for v in man["neck"][st][anim][f][:3]] + man["neck"][st][anim][f][3:]
    cell.alpha_composite(draw_scarf(scarf_view(anim, f), scarf_matrix(entry), 3, big))
    return cell.resize((px, px), Image.LANCZOS)


def build(out, px=150, per_row=11):
    """대표 칸 × 5단계. 위아래로 단계, 줄마다 per_row칸 (칸 안 크기는 그림 좌표 그대로, 단계 비교용)."""
    man = manifest()
    cache = {}
    rows = len(STAGES) * math.ceil(len(SHOWCASE) / per_row)
    img = Image.new("RGB", (per_row * (px + 4) + 4, rows * (px + 4) + 4), (226, 232, 238))
    y = 0
    for st in STAGES:
        for c, (anim, f) in enumerate(SHOWCASE):
            t = tile(cache, man, st, anim, f, px)
            img.paste(t, (4 + c % per_row * (px + 4), 4 + (y + c // per_row) * (px + 4)), t)
        y += math.ceil(len(SHOWCASE) / per_row)
    out.parent.mkdir(exist_ok=True)
    img.save(out)
    return out


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "tools" / "preview" / "santa_scarf.png")
    print(build(ap.parse_args().out))
