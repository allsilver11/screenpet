"""흐물이 시트 검사: 모든 동작·투명 여백·pet.json(겁쟁이·기운 넘침·창 옆면 매달리기, 막대사탕)·얼굴 없음·
곧지 않은 고무호스 팔다리·만세 줄행랑(panic)·창 옆면을 잡은 손(hang·rope)·아기는 머리가 크고 어른은 길쭉·
도트는 1칸 선·저장소 그림과 같음."""
import json
import math
from pathlib import Path

import numpy as np
import pytest

import draw_heumul as hm
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "heumul"
STANDING = ("walk", "idle", "look", "annoyed", "lookup", "stretch", "heart", "tickle", "sit_front", "wave", "tilt",
            "hungry", "eat", "sleep", "belly")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"heumul": hm.HEUMUL})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    return cell(sheet_128(out / "heumul" / f"sheet_{st}.png"), names(hm.HEUMUL).index(anim), i)


def alpha(im):
    return np.asarray(im.getchannel("A")) > 128


def near(im, col, tol=30):
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_panic_hang_and_rope():
    assert draw_pets.SPECIES["heumul"] is hm.HEUMUL
    assert [o[0] for o in OPTIONAL_ANIMS[-3:]] == ["panic", "hang", "rope"]  # = Rust manifest::OPTIONAL_ANIMS 맨 끝
    assert names(hm.HEUMUL) == [a for a, *_ in ANIMS] + ["lookup", "panic", "hang", "rope"]


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "heumul", hm.HEUMUL, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "heumul", hm.HEUMUL, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "heumul" / f"sheet_{st}.png", hm.HEUMUL)


def test_manifest_is_a_timid_energetic_rigger_that_loves_lollipops(out):
    m = json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "흐물이"
    assert m["favorite_food"] == {"emoji": "🍭", "name": "막대사탕"}
    assert m["personality"] == "playful"
    assert (m["timid"], m["energetic"], m["rigging"]) == (True, True, True)
    assert m["hop_share"] == 0.5 and m["walk_speed"] >= 100
    assert m["animations"]["panic"] == {"row": len(ANIMS) + 1, "frames": 4, "fps": 12}
    assert "front" not in m["animations"]["hang"] and "front" not in m["animations"]["rope"]
    assert "spin" in m["animations"]["roll"]


def test_the_build_reproduces_the_committed_heumul(out):
    names_ = sorted(p.name for p in SHIPPED.iterdir())
    assert names_ == sorted(p.name for p in (out / "heumul").iterdir())
    for name in names_:
        assert (out / "heumul" / name).read_bytes() == (SHIPPED / name).read_bytes(), name


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim", ["idle", "sit_front", "walk", "panic"])
def test_faceless_ink_lines_and_a_cream_head_only(out, st, anim):
    """얼굴이 없다: 칠해진 색은 잉크와 크림 머리, 그 사이 번짐뿐 (무채색에 가깝다) — 눈·입·볼 색이 없다."""
    im = frame(out, st, anim, 0)
    a = np.asarray(im.convert("RGBA")).astype(int)
    solid = a[..., 3] > 250
    ink, head = near(im, hm.INK, 40), near(im, hm.HEAD, 30)
    assert ink.sum() > 150 and head.sum() > 150, (st, anim)
    if anim != "panic":  # 땀방울·속도선 빼고
        chroma = a[..., :3].max(axis=2) - a[..., :3].min(axis=2)
        assert (solid & (chroma > 30)).sum() == 0, (st, anim, int((solid & (chroma > 30)).sum()))
    for col in (draw_pets.EYE, draw_pets.BLUSH, draw_pets.MOUTH_IN):
        assert near(im, col, 20).sum() == 0, (st, anim, col)


def bend(pts):
    """곡선이 곧은 줄에서 벗어난 정도 (현 길이 배)."""
    (x0, y0), (x1, y1) = pts[0], pts[-1]
    ln = math.hypot(x1 - x0, y1 - y0) or 1.0
    return max(abs((x1 - x0) * (y0 - y) - (x0 - x) * (y1 - y0)) / ln for x, y in pts) / ln


def lines_of(anim, i, st="teenager"):
    """그 칸에 그린 선들 (몸·팔다리 — 손발 점 빼고)."""
    seen = []
    orig = hm.Fig.render

    def spy(self, *a, **k):
        seen.extend(d for kind, d in self.items if kind == "line")
        return orig(self, *a, **k)

    with pytest.MonkeyPatch.context() as mp, draw_pets.stage(st, hm.HEUMUL.stage_tweak[st]):
        mp.setattr(hm.Fig, "render", spy)
        hm.RAW_POSES[anim](hm.HEUMUL, i)
    return seen


@pytest.mark.parametrize("anim", ["walk", "run", "idle", "panic", "hang", "tickle"])
def test_rubber_hose_limbs_are_curved_and_bend_differently_every_frame(anim):
    """팔다리·몸은 곧은 막대가 아니다 (대부분 눈에 띄게 휘고), 칸마다 휘는 모양이 다르다."""
    n = dict((a, k) for a, k, *_ in draw_pets.rows_for(hm.HEUMUL))[anim]
    shapes = []
    for i in range(n):
        ls = lines_of(anim, i)
        bent = [bend(p) for p in ls if len(p) > 2]
        assert sum(b > 0.06 for b in bent) >= len(bent) * 0.6, (anim, i, [round(b, 2) for b in bent])
        shapes.append([round(bend(p), 3) for p in ls])
    assert len({tuple(s) for s in shapes}) >= min(n, 2), anim


def head_box(im):
    m = near(im, hm.HEAD, 30)
    ys, xs = np.nonzero(m)
    return xs.min(), ys.min(), xs.max(), ys.max()


@pytest.mark.parametrize("st", STAGES)
def test_panic_throws_both_arms_up_above_the_head(out, st):
    """만세: 머리 꼭대기 쪽 높이에 잉크가 머리 양옆에 (두 팔 끝 — 손 점 없이 선으로만, 사용자 요청)."""
    for i in range(4):
        im = frame(out, st, "panic", i)
        x0, y0, x1, y1 = head_box(im)
        ink = near(im, hm.INK, 40)
        above = ink[: y0 + (y1 - y0) // 3]
        assert above[:, : (x0 + x1) // 2].sum() > 3 and above[:, (x0 + x1) // 2:].sum() > 3, (st, i)


def test_baby_has_a_bigger_head_and_the_adult_is_lankier(out):
    def ratio(st):
        im = frame(out, st, "idle", 0)
        x0, y0, x1, y1 = head_box(im)
        ys = np.nonzero(alpha(im))[0]
        return (y1 - y0) / (ys.max() - ys.min())
    r = [ratio(st) for st in STAGES]
    assert r == sorted(r, reverse=True) and r[0] > r[-1] + 0.08, [round(x, 2) for x in r]


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim", ["hang", "rope"])
def test_a_hand_grips_the_window_side_on_the_right(out, st, anim):
    """창 옆면 (몸 가운데 + 몸 반 폭 0.3 = 프레임 x 102)에 손이 닿아 있고, 몸은 그 왼쪽에 매달린다."""
    edge = 64 + 0.3 * 128
    for i in range(4):
        im = frame(out, st, anim, i)
        ink = near(im, hm.INK, 40)
        xs = np.nonzero(ink.any(axis=0))[0]
        assert abs(xs.max() - edge) <= 5, (st, anim, i, xs.max())
        x0, _, x1, _ = head_box(im)
        assert x1 < edge, (st, anim, i)


def test_pixel_limbs_are_one_dot_lines():
    """도트: 팔다리는 1칸 선 (외곽선을 두른 3칸 띠가 아니다) — 다리 높이의 가로줄에 잉크 덩어리가 3칸보다 넓지 않다."""
    import draw_pixel as px
    for st in ("child", "adult"):
        im = px.render_pixel(hm.HEUMUL, st, anims=("idle",))["idle"][0]
        a = np.asarray(im.convert("RGBA")).astype(int)
        ink = (a[..., 3] > 0) & (a[..., :3].sum(axis=2) < 300)
        ys = np.nonzero(ink.any(axis=1))[0]
        for y in range(ys.max() - 4, ys.max() - 1):
            runs, run = [], 0
            for v in ink[y]:
                run = run + 1 if v else 0
                runs.append(run)
            assert max(runs) <= 2, (st, y, ink[y].astype(int).tolist())


def test_limbs_end_as_plain_lines_without_hand_or_foot_dots():
    """사용자: 팔다리 끝의 손발 점은 빼고 선으로만 — 그리기 목록에 손 점·발끝이 남아도 그리지 않는다."""
    src = (Path(__file__).resolve().parent / "draw_heumul.py").read_text(encoding="utf-8")
    render = src[src.index("    def render(self"):src.index("def stand(")]
    assert 'kind == "dot"' not in render and 'kind == "foot"' not in render
