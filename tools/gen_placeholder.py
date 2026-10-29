"""임시 픽셀 동물 4종 시트 생성. 사용법: python tools/gen_placeholder.py"""

import json
from pathlib import Path

from PIL import Image, ImageDraw

from sprite_spec import ANIMS, FRAME, manifest

OUT = Path(__file__).resolve().parent.parent / "ui" / "pets"
OUTLINE = (40, 30, 30, 255)
WHITE = (255, 255, 255, 255)

SPECIES = {
    "cat": dict(name="고양이", speed=60, body=(245, 166, 35, 255), dark=(190, 110, 20, 255), ears="pointy", tail="long"),
    "dog": dict(name="강아지", speed=70, body=(222, 184, 135, 255), dark=(139, 90, 43, 255), ears="floppy", tail="short"),
    "rabbit": dict(name="토끼", speed=80, body=(245, 245, 245, 255), dark=(255, 170, 190, 255), ears="long", tail="puff"),
    "beaver": dict(name="비버", speed=40, body=(150, 95, 55, 255), dark=(90, 55, 30, 255), ears="round", tail="flat"),
}


def pose(anim, i):
    p = dict(dy=0, squash=0, legs=0, eyes="open", head_dx=0, dangle=False, z=False)
    if anim == "walk":
        p.update(legs=[0, 1, 0, -1][i], dy=[0, 1, 0, 1][i])
    elif anim == "idle":
        p.update(dy=i)
    elif anim == "look":
        p.update(head_dx=[0, 3, 0, -3][i])
    elif anim == "sleep":
        p.update(eyes="closed", squash=3, z=i == 1)
    elif anim == "hop":
        p.update(dy=[2, 8, 5][i], legs=[0, 1, 1][i])
    elif anim == "fall":
        p.update(eyes="wide", legs=[1, -1][i])
    elif anim == "dragged":
        p.update(eyes="wide", dangle=True, head_dx=[-1, 1][i])
    elif anim == "land":
        p.update(squash=[5, 2][i])
    return p


def draw_frame(sp, anim, i):
    p = pose(anim, i)
    img = Image.new("RGBA", (FRAME, FRAME), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    body, dark = sp["body"], sp["dark"]
    base = FRAME - 2 - p["dy"]
    leg_len = 10 if p["dangle"] else 6
    top = 38 - p["dy"] + p["squash"]

    for k, lx in enumerate((18, 26, 34, 42)):
        off = p["legs"] * (2 if k % 2 == 0 else -2)
        d.rectangle([lx + off, base - leg_len, lx + off + 4, base], fill=dark, outline=OUTLINE)

    ty = top + 6
    if sp["tail"] == "long":
        d.line([(14, ty + 4), (6, ty - 6), (8, ty - 12)], fill=body, width=4)
    elif sp["tail"] == "short":
        d.ellipse([8, ty - 2, 16, ty + 6], fill=body, outline=OUTLINE)
    elif sp["tail"] == "puff":
        d.ellipse([7, ty, 17, ty + 10], fill=WHITE, outline=OUTLINE)
    elif sp["tail"] == "flat":
        d.ellipse([2, ty + 4, 18, ty + 12], fill=dark, outline=OUTLINE)

    d.ellipse([12 - p["squash"], top, 48 + p["squash"], base - leg_len + 4], fill=body, outline=OUTLINE)

    hx, hy, r = 46 + p["head_dx"], top - 2, 11
    if sp["ears"] == "pointy":
        d.polygon([(hx - 9, hy - 6), (hx - 6, hy - 17), (hx - 1, hy - 9)], fill=body, outline=OUTLINE)
        d.polygon([(hx + 1, hy - 9), (hx + 6, hy - 17), (hx + 9, hy - 6)], fill=body, outline=OUTLINE)
    elif sp["ears"] == "long":
        d.ellipse([hx - 7, hy - 26, hx - 2, hy - 6], fill=body, outline=OUTLINE)
        d.ellipse([hx + 2, hy - 26, hx + 7, hy - 6], fill=body, outline=OUTLINE)
    elif sp["ears"] == "round":
        d.ellipse([hx - 11, hy - 12, hx - 4, hy - 5], fill=dark, outline=OUTLINE)
        d.ellipse([hx + 4, hy - 12, hx + 11, hy - 5], fill=dark, outline=OUTLINE)

    d.ellipse([hx - r, hy - r, hx + r, hy + r], fill=body, outline=OUTLINE)

    if sp["ears"] == "floppy":
        d.ellipse([hx - 13, hy - 6, hx - 6, hy + 8], fill=dark, outline=OUTLINE)
        d.ellipse([hx + 6, hy - 6, hx + 13, hy + 8], fill=dark, outline=OUTLINE)

    ex, ey = hx + 4, hy - 1
    if p["eyes"] == "open":
        d.rectangle([ex, ey, ex + 1, ey + 2], fill=OUTLINE)
    elif p["eyes"] == "closed":
        d.line([(ex - 1, ey + 1), (ex + 2, ey + 1)], fill=OUTLINE)
    else:
        d.ellipse([ex - 1, ey - 1, ex + 3, ey + 3], fill=WHITE, outline=OUTLINE)
    d.rectangle([hx + 9, hy + 2, hx + 10, hy + 3], fill=(230, 90, 110, 255))
    if p["z"]:
        d.text((54, 0), "z", fill=OUTLINE)
    return img


def main():
    cols = max(n for _, n, _, _ in ANIMS)
    for pid, sp in SPECIES.items():
        sheet = Image.new("RGBA", (FRAME * cols, FRAME * len(ANIMS)), (0, 0, 0, 0))
        for row, (anim, n, _, _) in enumerate(ANIMS):
            for i in range(n):
                sheet.paste(draw_frame(sp, anim, i), (i * FRAME, row * FRAME))
        out = OUT / pid
        out.mkdir(parents=True, exist_ok=True)
        sheet.save(out / "sheet.png")
        counts = {a: n for a, n, _, _ in ANIMS}
        (out / "pet.json").write_text(
            json.dumps(manifest(sp["name"], sp["speed"], counts), ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print("wrote", out)


if __name__ == "__main__":
    main()
