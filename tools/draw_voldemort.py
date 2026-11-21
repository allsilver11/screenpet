"""볼드모트 🐍 — 창백한 대머리, 코 없이 납작한 얼굴에 콧구멍 점 둘, 웃지 않는 일자 입·비웃음, 붉은 눈, 까만 로브 (초록 안감), 하얀 뼈 지팡이의 꼬마 마법사 펫(voldemort).
몸·동작은 draw_wizard (해리·헤르미온느와 같은 몸 — 목도리 없음)를 쓰고, 여기서는 생김새(Look)와 등록만.

    python tools/draw_voldemort.py → ui/pets/voldemort/{sheet_<stage>.png ×5, pet.json} + tools/preview/{stages_voldemort,climb_voldemort,wizard_voldemort}.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

특기 (코어): 🧹 broom 빗자루 날기 (pet.json broom_rider) · ✨ cast ⚡ Stupefy (spells: stupefy — 초록 빛줄기는 앱 덮개 창) ·
🐍 나기니·😈 hehehe (dark_lord — 뱀은 앱 덮개 창이 그림) · ⚔️ 해리·헤르미온느와 결투.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
import draw_wizard as wz  # noqa: E402
from draw_pets import C, E, L, T, Species  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

SKIN, SKIN_SHADE = (236, 238, 228), (198, 204, 194)  # 창백한 살 (살짝 푸르스름)
SCALP_HI = (252, 253, 248)  # 대머리 윤기
SLIT = (92, 96, 88)  # 납작한 콧구멍 두 줄
VEIN = (206, 212, 204)  # 관자놀이 그늘
EYE_RED, EYE_RED_HI = (150, 16, 30), (236, 58, 58)  # 붉은 눈 (눈동자, 아래쪽 밝은 홍채)
ROBE, ROBE_FAR = (32, 30, 38), (22, 20, 28)  # 까만 로브
ROBE_IN = (36, 96, 60)  # 초록 안감 (슬리데린)
JUMPER = (46, 44, 54)  # 앞섶 사이도 까맣게
PANTS, PANTS_FAR = (40, 38, 48), (30, 28, 36)
WAND, WAND_TIP = (230, 224, 206), (255, 255, 246)  # 하얀 뼈 지팡이
FISH, FISH_DARK, FISH_FIN = (138, 178, 212), (78, 116, 156), (196, 222, 240)  # 🐟 생선 생각 풍선


def bald(view_, hx, hy, hrx, hry, q):
    """대머리: 머리카락 없이 정수리에 윤기 한 줄 · 관자놀이 그늘. → (더할 도형 없음, 삐죽 없음, 머리 안 무늬)."""
    shine = [L(dp.arc(hx, hy, hrx * 0.72, hry * 0.72, 205, 250, 8), 2.2)]
    marks = [(shine, SCALP_HI)]
    if view_ == "front":
        marks.append(([L(dp.arc(hx, hy, hrx * 0.9, hry * 0.9, 150, 175, 5), 1.4), L(dp.arc(hx, hy, hrx * 0.9, hry * 0.9, 5, 30, 5), 1.4)], VEIN))
    elif view_ == "side":
        marks.append(([L(dp.arc(hx, hy, hrx * 0.86, hry * 0.86, 160, 190, 5), 1.4)], VEIN))
    return [], [], marks


def snake_face(view_, hx, hy, hrx, hry, far_eye, near_eye):
    """코 없는 납작한 얼굴: 콧등 없이 작은 콧구멍 점 둘 (앞모습은 눈 사이 아래 좌우로, 옆모습은 얼굴 앞쪽에 하나)."""
    (x0, y0), (x1, y1) = far_eye, near_eye
    if view_ == "front":
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2 + 0.3 * hry
        return [([E(cx - 3.2, cy - 0.7, cx - 1.2, cy + 0.9), E(cx + 1.2, cy - 0.7, cx + 3.2, cy + 0.9)], SLIT)]
    if view_ == "side":
        nx, ny = x0 + 0.04 * hrx, y1 + 0.3 * hry
        return [([E(nx - 1.0, ny - 0.7, nx + 1.0, ny + 0.9)], SLIT)]
    return []


def fish_bubble_fx(cx, cy, r=12.0):
    """🐟 생선 생각 풍선 (배고플 때): 파란 생선 한 마리 (꼬리·지느러미·눈)."""
    fx = []
    for x, y, rr in [(cx + r + 7, cy + r + 8, 2.0), (cx + r + 1, cy + r + 1, 3.2), (cx, cy, r)]:
        fx += [([C(x, y, rr + 1.5)], dp.LINE, 1.0), ([C(x, y, rr)], dp.WHITE, 0)]
    tail = T((cx - 4, cy), (cx - 9.5, cy - 4.5), (cx - 9.5, cy + 4.5))
    fx += [([tail], FISH_DARK, 1.0), ([E(cx - 6, cy - 3.8, cx + 7, cy + 3.8)], FISH, 1.0), ([T((cx - 1, cy - 3.4), (cx + 2, cy - 6.5), (cx + 3.5, cy - 3.2))], FISH_FIN, 0.8),
           ([C(cx + 4, cy - 0.8, 1.0)], dp.LINE, 0)]
    return fx


VOLDEMORT_LOOK = wz.Look(hair=bald, hair_col=SKIN, skin=SKIN, skin_shade=SKIN_SHADE, extra=snake_face, food_fx=fish_bubble_fx, wand=WAND, wand_tip=WAND_TIP,
                         robe=ROBE, robe_far=ROBE_FAR, robe_in=ROBE_IN, jumper=JUMPER, pants=PANTS, pants_far=PANTS_FAR, scarf=False, blush=None, side_ear=False,
                         eyes=(EYE_RED, EYE_RED_HI),
                         eye_swap={"happy": "dot", "tight": "flat"},  # 웃는 ^^·질끈 눈 → 붉은 눈·가늘게 뜬 눈
                         mouth_swap={"smile": "flat", "chew": "flat", "open": "sneer", "big": "sneer", "grin": "sneer"})  # 웃지 않음: 일자 입·비웃음
VOLDEMORT_POSES = wz.poses_for(VOLDEMORT_LOOK)

VOLDEMORT = Species(
    id="voldemort", name="볼드모트", walk_speed=84, scale=1.0, base=ROBE, back=ROBE, belly=ROBE, nose=wz.OUT, mouth="w",
    front_tail_color=ROBE, tail_color=ROBE, **dp.NO_COMMON_PARTS,
    specials=["climb", "slide", "lookup"] + wz.BROOM_ANIMS + wz.SPELL_ANIMS, stage_size=dict.fromkeys(STAGES, 0.85), stripe=ROBE_IN,
    poses=VOLDEMORT_POSES, outline=wz.OUT,
    stage_tweak={st: dict(fit=1.0, pale=0.0) for st in STAGES},
)


def register():
    """draw_pets.SPECIES에 볼드모트를 등록함."""
    dp.SPECIES[VOLDEMORT.id] = VOLDEMORT


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
    by_stage = dp.at_frame(dp.write_species(VOLDEMORT, root / "ui" / "pets" / VOLDEMORT.id))
    preview(by_stage, pv / "stages_voldemort.png")
    dp.preview_back(by_stage, pv / "climb_voldemort.png")
    wz.preview_rows(by_stage, pv / "wizard_voldemort.png", wz.BROOM_ANIMS + wz.SPELL_ANIMS + ["climb", "slide"])
    print("ok: voldemort")


if __name__ == "__main__":
    main()
