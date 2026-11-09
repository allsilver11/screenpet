"""레서판다 🎋 — 잘 놀라는 장난꾸러기 레서판다(redpanda). draw_pets 공통 뼈대(네 발 동물)에 종별 설정을 입히고,
깜짝 자세(surprise)·들어 올린 모습(dragged)·배고픔(hungry)만 따로 그린다.

    python tools/draw_redpanda.py → ui/pets/redpanda/{sheet_<stage>.png ×5, pet.json} + tools/preview/stages_redpanda.png
    (python tools/draw_pets.py도 함께 만든다 — draw_pets 맨 끝에서 이 모듈을 불러 SPECIES에 등록)

레서판다: 녹슨 주황빨강 털 + 흰 얼굴 무늬 (볼·주둥이·눈썹 점·귀 테두리) + 눈에서 내려오는 적갈색 눈물 줄 +
          짙은 적갈색 다리·배·팔 + 짙은 고리 무늬가 있는 길고 복슬한 꼬리 (상징). 이름은 "레서판다".
          바탕(base)이 짙은 다리 색이고, 머리·몸·등은 무늬로 주황빨강을 덮는다 (다리·발·팔은 저절로 짙은 색).
단계: 아기 = 크림빛으로 바랜 복슬 아기 (고리 둘, 정수리 털) → 유아 고리 셋·눈물 줄 → 어린이부터 고리 넷과 짙은 꼬리 끝
      → 어른 볼 털 + 귀 옆에 대나무잎 핀.
깜짝 (surprise, 앞모습 한 번): 놀라면 뒷발로 벌떡 서서 두 팔을 머리 위로 번쩍 — 0 움찔(팔 반쯤) · 1 쭉 (!) · 2 쭉 (!!).
      클릭·살짝 부딪힘에 (코어 Special::Surprise). 들어 올리면(dragged)도 두 팔 번쩍, 다리는 대롱대롱.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import draw_pets as dp  # noqa: E402
from draw_pets import (C, CREAM, E, L, T, WHITE, Ear, Part, Species, _ST, arc, mix, shape, xf)  # noqa: E402
from draw_pets import FRONT_BODY_PIVOT, STAGE_PROPS, _fit  # noqa: E402
from sprite_spec import STAGES  # noqa: E402

RUST = (212, 98, 52)  # 몸·머리 녹슨 주황빨강
TAIL = (226, 124, 70)  # 꼬리 (몸보다 조금 밝은 주황) — 고리가 또렷하게
RING = (146, 60, 40)  # 꼬리 고리 (짙은 적갈색)
LIMB = (96, 50, 44)  # 다리·배·팔 (짙은 적갈색) — 공통 뼈대의 바탕색
FACE_W = (255, 250, 244)  # 흰 얼굴 무늬
TEAR = (178, 76, 46)  # 눈물 줄
EAR_IN = (112, 58, 46)  # 귀 안쪽
NOSE = (70, 44, 42)
RP_LINE = (104, 54, 44)  # 외곽선 (짙은 적갈색)
LEAF, LEAF_DARK = (128, 198, 98), (80, 148, 74)  # 어른 대나무잎 핀 · 생각 풍선 대나무
BAMBOO, BAMBOO_DARK = (176, 206, 104), (122, 160, 70)
SURPRISE_INK = (232, 76, 76)  # 깜짝 "!"

_COLORS = {"rust": RUST, "tail": TAIL, "ring": RING, "limb": LIMB, "white": FACE_W}
_TWEAK = {"baby": dict(ear=0.86), "kinder": dict(ear=0.92), "adult": dict(fit=0.9)}


def fur(name, stage):
    """그 단계 시트에 칠해지는 색 (시험용): 어린 단계는 크림색 쪽으로 바랜다 (draw_pets _pale)."""
    pale = {**STAGE_PROPS[stage], **_TWEAK.get(stage, {})}["pale"]
    return mix(_COLORS[name], CREAM, pale) if pale else _COLORS[name]


def _mirror_t(desc):
    return ("T", *[(128 - x, y) for x, y in desc[1:]])


def _rings(line, w, ts, thick=4.4):
    """줄 꼬리(line, 굵기 w)를 가로지르는 고리 띠 — ts: 줄 위의 자리 (0 뿌리 … 1 끝)."""
    seg = list(zip(line, line[1:]))
    lens = [math.dist(a, b) for a, b in seg]
    total = sum(lens)
    out = []
    for t in ts:
        d = t * total
        for (a, b), ln in zip(seg, lens):
            if d <= ln:
                u = d / ln
                x, y = a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u
                nx, ny = -(b[1] - a[1]) / ln, (b[0] - a[0]) / ln
                h = w * 0.75
                out.append(("L", [(x - nx * h, y - ny * h), (x + nx * h, y + ny * h)], thick))
                break
            d -= ln
    return out


def _tip(line, w):
    """꼬리 끝 짙은 털 (줄 끝을 덮는 동그라미)."""
    x, y = line[-1]
    return ("E", x - w * 0.62, y - w * 0.62, x + w * 0.62, y + w * 0.62)


def _staged_rings(line, w, root_shift=0.0):
    """단계별 고리: 아기 둘 · 유아 셋 · 어린이부터 넷 + 짙은 끝."""
    k = root_shift
    return [(_rings(line, w, [0.4 + k, 0.72 + k * 0.5]), RING, (0, 0)),
            (_rings(line, w, [0.3 + k, 0.55 + k * 0.5, 0.8]), RING, (1, 1)),
            (_rings(line, w, [0.24 + k, 0.44 + k * 0.5, 0.64, 0.84]) + [_tip(line, w)], RING, 2)]


# ---- 머리 무늬 ----
_EAR_L = ("T", (14, 42), (17, 0), (52, 18))
_EAR_L_IN = ("T", (22, 34), (22.5, 11), (42, 21))
_LEAF_FRONT = [("T", (86, 32), (95, 18), (108, 9), (101, 24)), ("T", (86, 32), (100, 25), (114, 25), (101, 32))]
_LEAF_SIDE = [("T", (64, 32), (73, 18), (86, 9), (79, 24)), ("T", (64, 32), (78, 25), (92, 25), (79, 32))]

_side_tail = [(26, 80), (14, 77), (8, 65), (8, 50), (14, 38)]
_front_tail = [(84, 116), (101, 114), (112, 103), (115, 88), (112, 76)]

RED_PANDA = Species(
    id="redpanda", name="레서판다", walk_speed=80, scale=0.9, base=LIMB, back=RUST, belly=LIMB, nose=NOSE, mouth="w",
    stage_size=dict.fromkeys(STAGES, 0.88),
    front_ears=[_EAR_L, _mirror_t(_EAR_L)],
    front_ear_pivots=((30, 32), (98, 32)),
    front_ear_marks=[([_EAR_L, _mirror_t(_EAR_L)], FACE_W), ([_EAR_L_IN, _mirror_t(_EAR_L_IN)], EAR_IN)],
    front_head_marks=[
        ([("E", 14, 14, 114, 86)], RUST),
        ([("E", 6, 56, 46, 86), ("E", 82, 56, 122, 86)], FACE_W),  # 흰 볼 (어른 볼 털까지)
        ([("E", 45, 54, 83, 82)], FACE_W),  # 흰 주둥이
        ([("E", 35, 32, 50, 42), ("E", 78, 32, 93, 42)], FACE_W),  # 흰 눈썹 점
        ([("L", [(49, 62), (52, 74)], 3.6), ("L", [(79, 62), (76, 74)], 3.6)], TEAR, 1),  # 눈물 줄 (유아부터)
        (_LEAF_FRONT, LEAF, 4), ([("L", [(86, 32), (104, 14)], 1.3), ("L", [(86, 32), (110, 27)], 1.3)], LEAF_DARK, 4),  # 어른 대나무잎 핀
    ],
    front_body_marks=[
        ([("E", 22, 48, 106, 106)], RUST),
        ([("E", 47, 80, 81, 124)], LIMB),  # 짙은 가슴·배
    ],
    front_tail=[("L", _front_tail, 15)], front_tail_root=(94, 112), front_tail_color=TAIL,
    front_tail_marks=_staged_rings(_front_tail, 15),
    side_ears=[Ear([("T", (54, 36), (57, 0), (86, 18))], (70, 26), FACE_W, [([("T", (60, 30), (60.5, 9), (77, 18))], EAR_IN)]),
               Ear([("T", (92, 20), (110, 0), (122, 30))], (106, 24), FACE_W, [([("T", (98, 19), (109, 7), (115, 26))], EAR_IN)])],
    ears_over=False,
    side_head_marks=[
        ([("E", 44, 6, 132, 84)], RUST),
        ([("E", 86, 54, 114, 78)], FACE_W),  # 볼
        ([("E", 104, 46, 134, 74)], FACE_W),  # 주둥이
        ([("E", 97, 30, 110, 39)], FACE_W),  # 눈썹 점
        ([("L", [(103, 53), (106, 63)], 3.4)], TEAR, 1),
        (_LEAF_SIDE, LEAF, 4), ([("L", [(64, 32), (82, 14)], 1.3), ("L", [(64, 32), (88, 27)], 1.3)], LEAF_DARK, 4),
    ],
    side_torso_marks=[([("E", 4, 34, 112, 96)], RUST)],
    side_tail=[("L", _side_tail, 17)], tail_root=(26, 80), tail_color=TAIL,
    tail_marks=_staged_rings(_side_tail, 17, 0.04),
    lie_ears=(-30, -26),
    specials=["climb", "slide", "lookup", "surprise"],
    stage_tweak=_TWEAK,
    stripe=RING,
    back_body_marks=[([("E", 24, 40, 104, 112)], RUST)],
    outline=RP_LINE,
)


# =====================================================================
# 포즈 — 공통 뼈대 + 깜짝 · 들어 올림 · 배고픔
# =====================================================================
def bamboo_bubble_fx(cx, cy):
    """생각 풍선 안에 대나무 한 마디 + 잎 두 장."""
    fx = dp._bubble(cx, cy)
    stem = [(cx - 5, cy + 8), (cx + 4, cy - 8)]
    fx += [([L(stem, 4.6)], BAMBOO_DARK, 0), ([L(stem, 2.8)], BAMBOO, 0),
           ([L([(cx - 2.6, cy + 1.6), (cx + 1.0, cy + 3.4)], 1.2)], BAMBOO_DARK, 0)]
    for tip, base in (((cx + 9, cy - 3), (cx + 1, cy - 2)), ((cx - 8, cy - 6), (cx - 1, cy - 1))):
        mx, my = (tip[0] + base[0]) / 2, (tip[1] + base[1]) / 2
        nx, ny = -(tip[1] - base[1]) * 0.28, (tip[0] - base[0]) * 0.28
        leaf = T(base, (mx + nx, my + ny), tip, (mx - nx, my - ny))
        fx += [([leaf], LEAF, 0), ([L([base, tip], 0.8)], LEAF_DARK, 0)]
    return fx


def hungry(sp, i):
    """배고파 시무룩 (공통 hungry와 같은 자세) + 머리 위 생각 풍선에 대나무."""
    dx, dy = [(-2, 0), (0, 2), (2, 0)][i]
    return dp._front(sp, eyes="closed", mouth="frown", head=dict(dy=2, rot=-4, pivot=(64, 84)), head_kw=dict(brow="sad"),
                     tail_rot=6, paws=[(56 + dx, 100 + dy)], toes=True, fx=bamboo_bubble_fx(108, 17))


def up_arm(sp, side, reach, spread):
    """머리 위로 번쩍 든 팔 한 짝 (머리 좌표 — 머리와 함께 움직인다): 턱 아래 어깨 → 귀 옆 바깥으로 손.
    reach: 0 귀 아래 옆으로 … 1 귀 높이 위로 · spread: 바깥으로 벌림. 머리 뒤에 그려서 머리 밖으로 나온 팔과 손만 보인다."""
    s, h = side, _ST["head"]
    sx, sy = 64 + 24 * s, 96
    hx = 64 + s * (42 * h + 9 + 5 * spread)
    hy = 84 - (50 + 22 * reach) * h
    elbow = (64 + s * (44 * h + 8 + 3 * spread), (sy + hy) / 2 + 8)
    arm = L([(sx, sy), elbow, (hx, hy)], 13)
    paw = E(hx - 8, hy - 7.5, hx + 8, hy + 7.5)
    toes = [C(hx + d, hy - 6.2, 2.6) for d in (-4.6, 0, 4.6)]
    beans = ([E(hx - 3.2, hy - 1.2, hx + 3.2, hy + 4)], dp.PINK)
    return Part([arm, paw] + toes, sp.base, [beans], grow=0, outline=True)


def bang_fx(i, top):
    """깜짝 "!" (한 장 0: 작게 하나, 1: 하나, 2: 둘)."""
    out = []
    marks = [[(64, top - 3, 0.7)], [(64, top - 6, 1.0)], [(56, top - 5, 1.0), (72, top - 7, 1.0)]][i]
    for x, y, k in marks:
        out.append(([L([(x, y - 11 * k), (x, y - 3.5 * k)], 3.0 * k), C(x, y, 1.7 * k)], SURPRISE_INK, 1.4))
    return out


def _stand(sp, *, stretch, reach, spread, eyes, mouth, legs_dy=(0, 0), whole=None, tail_rot=0.0, fx_fn=None, arms_front=False):
    """뒷발로 선 앞모습: 몸을 위로 stretch만큼 늘이고 (발은 바닥), 머리는 늘어난 몸 위에, 두 팔은 머리 뒤로 번쩍.
    fx_fn: 머리 꼭대기 y(프레임 그림 좌표)를 받아 효과를 돌려준다."""
    b = _ST["body"]
    fx_, fy_ = _ST["fbody"]
    stage_body = dict(sx=b * fx_, sy=b * fy_, pivot=FRONT_BODY_PIVOT)
    stand = dict(sy=1 + stretch, pivot=FRONT_BODY_PIVOT)
    rise = stretch * (FRONT_BODY_PIVOT[1] - 60) * b * fy_
    parts, ops = [], []
    with xf(**(whole or {})):
        with xf(**stand):
            with xf(**stage_body):
                body = dp.front_body(sp, legs_dy)
                tail = dp.front_tail(sp, tail_rot)
        with xf(dy=(1 - b) * (FRONT_BODY_PIVOT[1] - 84) - rise):
            arms = [up_arm(sp, s, reach, spread) for s in (-1, 1)]
            hp, face = dp.front_head(sp, eyes, mouth)
            head_top = dp._apply(dp._XF[-1], (64, 84 - 68 * _ST["head"]))[1]
        parts = [tail, body] + ([] if arms_front else arms) + [hp] + (arms if arms_front else [])
        ops = list(face)
    return dp.compose(parts, ops, fx_fn(head_top) if fx_fn else [])


_SMALL = {"baby": 1.0, "kinder": 0.96, "child": 0.92, "teenager": 0.9, "adult": 0.93}  # 서면 커지니 조금 줄여 칸 안에 (lvl 순)


def _small(**kw):
    k = _SMALL[STAGES[_ST["lvl"]]]
    return dict(sx=k, sy=k, pivot=(64, 126), **kw)


SURPRISE = [dict(stretch=0.12, reach=0.2, spread=0.6, eyes="round", mouth="open", whole=dict(dy=-1)),
            dict(stretch=0.24, reach=1.0, spread=0.0, eyes="round", mouth="open", whole=dict()),
            dict(stretch=0.24, reach=1.0, spread=0.8, eyes="round", mouth="open", whole=dict(rot=1.5))]


def surprise(sp, i):
    """깜짝! 뒷발로 벌떡 서서 두 팔을 머리 위로 번쩍 (동그란 눈, 작게 벌린 입, 꼬리는 바닥에 받침, "!")."""
    kw = dict(SURPRISE[i])
    kw["whole"] = _small(**kw["whole"])
    return _stand(sp, tail_rot=[-6, -10, -12][i], fx_fn=lambda top: bang_fx(i, top), **kw)


def dragged(sp, i):
    """들어 올려져 대롱대롱: 앞모습으로 두 팔 번쩍, 짧은 다리는 아래로 축 (흔들흔들), 동그란 눈."""
    s = [-1, 1][i]
    return _stand(sp, stretch=0.16, reach=0.95, spread=0.5 + 0.3 * s, eyes="round", mouth="open", legs_dy=(4 + 2 * s, 4 - 2 * s),
                  whole=_small(rot=3 * s), tail_rot=24 + 8 * s)


RED_PANDA.poses = {k: _fit(v) for k, v in {"surprise": surprise, "dragged": dragged, "hungry": hungry}.items()}


def register():
    dp.SPECIES[RED_PANDA.id] = RED_PANDA


register()


def main():
    import draw_halloween as dh
    root = Path(__file__).resolve().parent.parent
    pv = root / "tools" / "preview"
    pv.mkdir(exist_ok=True)
    by_stage = dp.at_frame(dp.write_species(RED_PANDA, root / "ui" / "pets" / RED_PANDA.id))
    dh.preview_species(by_stage, pv / "stages_redpanda.png")
    dp.preview_back(by_stage, pv / "climb_redpanda.png")
    print("ok: redpanda")


if __name__ == "__main__":
    main()
