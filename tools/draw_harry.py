"""해리포터 🧙 — 둥근 까만 안경, 이마의 번개 흉터, 덥수룩 몽글몽글 까만 머리, 검은 로브 + 그리핀도르 목도리, 밝은 나무 지팡이의 꼬마 마법사 펫(harry).
몸·동작은 draw_wizard (볼드모트도 나중에 같은 몸)를 쓰고, 여기서는 생김새(Look)와 등록만.

    python tools/draw_harry.py → ui/pets/harry/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_harry,climb_harry,wizard_harry}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

특기 (코어): 🧹 broom 빗자루 날기 (pet.json broom_rider) · ✨ cast 주문 (spells: expelliarmus·leviosa) · 🟡 골든 스니치 쫓기 (seeker — 빗자루 그림으로).
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_wizard as wz  # noqa: E402
from draw_pets import C, E, L, T, Species  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

HAIR = (34, 30, 38)  # 까만 머리
HAIR_HI = (84, 84, 110)  # 머리 윤기
GLASSES = (28, 26, 34)  # 안경테
GLINT = (255, 255, 255)  # 렌즈 반짝
SCAR = (204, 70, 70)  # 번개 흉터
CANDY, CANDY_HI = (246, 112, 150), (255, 196, 214)  # 🍬 사탕 생각 풍선


def spikes_at(hx, hy, hrx, hry, angles, k=1.3):
    """머리 둘레 angles(도, 0 = 오른쪽, -90 = 위)에서 바깥으로 살짝 솟은 짧은 머리 끝 (작은 삼각 — 덥수룩한 느낌만)."""
    out = []
    for a, kk in angles:
        r = math.radians(a)
        b0 = (hx + hrx * 0.9 * math.cos(r - 0.17), hy + hry * 0.9 * math.sin(r - 0.17))
        b1 = (hx + hrx * 0.9 * math.cos(r + 0.17), hy + hry * 0.9 * math.sin(r + 0.17))
        tip = (hx + hrx * k * kk * math.cos(r + 0.1), hy + hry * k * kk * math.sin(r + 0.1))
        out.append(T(b0, tip, b1))
    return out


def tufts_at(hx, hy, hrx, hry, angles, d=1.04, r=0.25):
    """머리 둘레 angles(도)에 동그란 머리 뭉치 (녹이면 몽글몽글 부드러운 실루엣) — (각도, 크기 배율)."""
    return [C(hx + hrx * d * math.cos(math.radians(a)), hy + hry * d * math.sin(math.radians(a)), hrx * r * kk) for a, kk in angles]


def harry_hair(view_, hx, hy, hrx, hry, q):
    """덥수룩한 까만 머리: 정수리 볼륨 + 둘레에 몽글몽글 둥근 머리 뭉치 + 짧은 삐침 몇 가닥 + 앞머리 끝선이 부드럽게 들쭉날쭉
    (이마 가운데 오른쪽에 흉터 자리를 비움). → (머리 도형에 더할 도형, 삐죽 조각, 머리 안 무늬)."""
    sky = hy - hry * 1.6 - q - 8
    shine = [L(dp.arc(hx, hy, hrx * 0.74, hry * 0.74, 215, 262, 8), 2.0)]
    if view_ == "back":
        tf = tufts_at(hx, hy, hrx, hry, [(-165, 0.95), (-135, 1.05), (-105, 1.0), (-75, 1.05), (-45, 1.0), (-15, 0.95)])
        sp = spikes_at(hx, hy, hrx, hry, [(-170, 1.0), (-120, 1.04), (-90, 1.06), (-58, 1.04), (-10, 1.0)], 1.2)
        marks = [([T((hx - hrx * 2, sky), (hx + hrx * 2, sky), (hx + hrx * 2, hy + 0.55 * hry), (hx - hrx * 2, hy + 0.55 * hry))], HAIR), (shine, HAIR_HI)]
        nape = [T((hx - 0.7 * hrx, hy + 0.5 * hry), (hx - 0.45 * hrx, hy + 0.9 * hry), (hx - 0.2 * hrx, hy + 0.62 * hry),
                  (hx + 0.05 * hrx, hy + 0.92 * hry), (hx + 0.3 * hrx, hy + 0.62 * hry), (hx + 0.5 * hrx, hy + 0.88 * hry), (hx + 0.72 * hrx, hy + 0.5 * hry))]
        return [E(hx - 0.95 * hrx, hy - hry - q, hx + 0.95 * hrx, hy)] + tf, sp, marks + [(nape, HAIR)]
    if view_ == "front":
        tf = tufts_at(hx, hy, hrx, hry, [(-160, 0.9), (-130, 1.0), (-100, 1.05), (-72, 1.0), (-44, 1.0), (-18, 0.9)])
        sp = spikes_at(hx, hy, hrx, hry, [(-168, 1.0), (-116, 1.05), (-86, 1.06), (-56, 1.05), (-12, 1.0)], 1.2)
        z = [(1.2, 0.12), (0.95, -0.04), (0.8, -0.26), (0.62, -0.2), (0.46, -0.4), (0.3, -0.34), (0.16, -0.52), (-0.04, -0.38),
             (-0.22, -0.5), (-0.42, -0.32), (-0.6, -0.42), (-0.78, -0.2), (-0.95, -0.04), (-1.2, 0.12)]
        edge = [(hx + a * hrx, hy + b * hry) for a, b in z]
        ears = [E(hx + sd * hrx - 3.2, hy + 0.04 * hry - 4.6, hx + sd * hrx + 3.2, hy + 0.04 * hry + 4.6) for sd in (-1, 1)]
        return [E(hx - 0.95 * hrx, hy - hry - q, hx + 0.95 * hrx, hy - 0.2 * hry)] + ears + tf, sp, \
            [([T(*edge, (hx - hrx * 1.3, sky), (hx + hrx * 1.3, sky))], HAIR), (shine, HAIR_HI)]
    # 옆모습 (오른쪽을 봄): 앞머리는 이마 위에서 몽글, 뒤통수·목덜미까지 머리
    tf = tufts_at(hx, hy, hrx, hry, [(-175, 1.0), (-148, 1.05), (-120, 1.05), (-92, 1.0), (-64, 0.95), (172, 0.9)])
    sp = spikes_at(hx, hy, hrx, hry, [(-180, 1.0), (-134, 1.05), (-106, 1.07), (-78, 1.05), (-40, 1.02)], 1.2)
    z = [(1.2, -0.4), (0.95, -0.46), (0.8, -0.32), (0.62, -0.46), (0.46, -0.34), (0.3, -0.46), (0.14, -0.24), (0.02, 0.06),
         (-0.06, 0.3), (-0.3, 0.46), (-0.48, 0.66), (-0.62, 0.6), (-0.82, 0.74), (-1.2, 0.72)]
    edge = [(hx + a * hrx, hy + b * hry) for a, b in z]
    return [E(hx - 0.2 * hrx, hy - hry - q, hx + 0.9 * hrx, hy - 0.4 * hry)] + tf, sp, \
        [([T(*edge, (hx - hrx * 1.3, sky), (hx + hrx * 1.3, sky))], HAIR), (shine, HAIR_HI)]


def ring(cx, cy, rx, ry, w=1.5):
    """안경테 하나 (타원 고리)."""
    return L(dp.arc(cx, cy, rx, ry, 0, 360, 24), w)


def bolt(x, y, h):
    """번개 흉터 ⚡ (위 (x, y)에서 아래로 h)."""
    return L([(x + h * 0.18, y), (x - h * 0.12, y + h * 0.5), (x + h * 0.14, y + h * 0.5), (x - h * 0.16, y + h)], 1.25)


def harry_extra(view_, hx, hy, hrx, hry, far_eye, near_eye):
    """둥근 까만 안경 (앞모습 두 렌즈 + 다리, 옆모습 가까운 렌즈 + 귀까지 다리 + 먼 렌즈 반쪽) · 이마의 번개 흉터."""
    r = 0.25 * hrx
    ops = []
    if view_ == "front":
        (x0, y0), (x1, y1) = far_eye, near_eye
        ops.append(([ring(x0, y0, r, r * 0.95), ring(x1, y1, r, r * 0.95), L([(x0 + r, y0 - 0.5), (x1 - r, y1 - 0.5)], 1.3),
                     L([(x0 - r, y0 - 1), (hx - hrx * 0.97, y0 - 2)], 1.2), L([(x1 + r, y1 - 1), (hx + hrx * 0.97, y1 - 2)], 1.2)], GLASSES))
        ops.append(([L(dp.arc(x0, y0, r * 0.62, r * 0.62, 200, 250, 4), 1.0), L(dp.arc(x1, y1, r * 0.62, r * 0.62, 200, 250, 4), 1.0)], GLINT))
        ops.append(([bolt(hx + 0.08 * hrx, hy - 0.54 * hry, 0.26 * hry)], SCAR))
    elif view_ == "side":
        (xf_, yf), (xn, yn) = far_eye, near_eye
        ops.append(([ring(xn, yn, r * 0.9, r * 0.95), ring(xf_, yf, r * 0.42, r * 0.9, 1.3), L([(xn + r * 0.9, yn - 0.6), (xf_ - r * 0.42, yf - 0.6)], 1.2),
                     L([(xn - r * 0.9, yn - 1.0), (hx - 0.1 * hrx, hy + 0.02 * hry)], 1.3)], GLASSES))
        ops.append(([L(dp.arc(xn, yn, r * 0.56, r * 0.56, 200, 250, 4), 1.0)], GLINT))
        ops.append(([bolt(hx + 0.58 * hrx, hy - 0.42 * hry, 0.22 * hry)], SCAR))
    return ops


def candy_bubble_fx(cx, cy, r=12.0):
    """🍬 사탕 생각 풍선 (배고플 때): 분홍 사탕 + 양쪽 비튼 포장."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    wrap = [T((cx - 4, cy), (cx - 9.5, cy - 4.5), (cx - 9.5, cy + 4.5)), T((cx + 4, cy), (cx + 9.5, cy - 4.5), (cx + 9.5, cy + 4.5))]
    fx += [(wrap, CANDY_HI, 1.0), ([E(cx - 5, cy - 4, cx + 5, cy + 4)], CANDY, 1.0), ([L([(cx - 2.5, cy - 1.5), (cx + 1, cy - 2.6)], 1.2)], dp.WHITE, 0)]
    return fx


HARRY_LOOK = wz.Look(hair=harry_hair, hair_col=HAIR, extra=harry_extra, food_fx=candy_bubble_fx, wand=(186, 134, 82), wand_tip=(238, 212, 170),
                     cap_lift=0.15)
HARRY_POSES = wz.poses_for(HARRY_LOOK)

HARRY = Species(
    id="harry", name="해리포터", walk_speed=92, scale=1.0, base=wz.ROBE, back=wz.ROBE, belly=wz.ROBE, nose=wz.OUT, mouth="w",
    front_tail_color=wz.SCARF, tail_color=wz.SCARF, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + wz.BROOM_ANIMS + wz.SPELL_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=wz.SCARF_GOLD,
    poses=HARRY_POSES, outline=wz.OUT,
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 해리포터를 등록함."""
    dp.SPECIES[HARRY.id] = HARRY


register()


def preview(by_stage, path, bg=(255, 255, 255)):
    """5단계 × 대표 동작 + 특기 동작 (밝은·어두운 바탕)."""
    names = [("walk", 0), ("run", 1), ("sit_front", 0), ("wave", 1), ("eat", 0), ("sleep", 0), ("hungry", 0), ("roll", 0),
             ("broom", 0), ("broom", 1), ("cast", 0), ("cast", 1), ("cast", 2), ("climb", 0), ("slide", 0), ("idle", 0)]
    rows = [(st, by_stage, st) for st in STAGES]
    dp.preview_poses([(bg, rows), ((40, 44, 52), rows)], names, path, zoom=("sit_front", "broom"), bg=bg)


def main():
    """시트·pet.json과 미리보기를 만듦."""
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(HARRY, root / "ui" / "pets" / HARRY.id))
    preview(by_stage, pv / "stages_harry.png")
    dp.preview_back(by_stage, pv / "climb_harry.png")
    wz.preview_rows(by_stage, pv / "wizard_harry.png", wz.BROOM_ANIMS + wz.SPELL_ANIMS + ["climb", "slide"])
    print("ok: harry")


if __name__ == "__main__":
    main()
