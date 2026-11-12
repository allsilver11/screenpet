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
            "hungry", "eat", "sleep", "belly", "lift", "hold", "carry", "throw", "strain", "present", "slump", "yank", "stuff")
JUGGLE = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]
TASKBAR = ["yank", "stuff"]


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


def test_registered_with_panic_hang_rope_the_juggle_rows_and_the_taskbar_rows():
    assert draw_pets.SPECIES["heumul"] is hm.HEUMUL
    assert [o[0] for o in OPTIONAL_ANIMS[-12:-9]] == ["panic", "hang", "rope"]
    assert [o[0] for o in OPTIONAL_ANIMS[-9:-2]] == JUGGLE  # = Rust manifest::JUGGLE_ANIMS
    assert [o[0] for o in OPTIONAL_ANIMS[-2:]] == TASKBAR  # = Rust manifest::OPTIONAL_ANIMS 맨 끝 (TASKBAR_ANIMS)
    assert names(hm.HEUMUL) == [a for a, *_ in ANIMS] + ["lookup", "panic", "hang", "rope"] + JUGGLE + TASKBAR


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
    assert (m["timid"], m["energetic"], m["rigging"], m["juggler"], m["taskbar_play"]) == (True, True, True, True, True)
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


def test_juggle_rows_play_like_the_core_expects(out):
    """코어 juggle.rs: lift 4칸 8fps 한 번 (LIFT_SECS 0.6), throw 4칸 10fps 한 번 (셋째 칸 = THROW_RELEASE 0.2초에 놓는다),
    present 앞모습 4칸 3fps 한 번 (넷째 칸 = PRESENT_RELEASE 1초에 떨군다)."""
    a = json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))["animations"]
    assert (a["lift"]["frames"], a["lift"]["fps"], a["lift"].get("loop")) == (4, 8, False)
    assert (a["throw"]["frames"], a["throw"]["fps"], a["throw"].get("loop")) == (4, 10, False)
    assert (a["present"]["frames"], a["present"]["fps"], a["present"].get("loop"), a["present"].get("front")) == (4, 3, False, True)
    for anim in ("hold", "carry", "strain", "slump"):
        assert "loop" not in a[anim] and "front" not in a[anim], anim


@pytest.mark.parametrize("st", STAGES)
def test_grip_table_puts_the_held_object_above_the_head(out, st):
    """pet.json grip (칸 px): 든 동작에서 물건 바닥(손)은 머리 꼭대기 바로 위, 몸 가운데 언저리. 쪼그린 첫 칸은 바닥 근처 앞쪽."""
    m = json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))
    g = m["grip"][st]
    fs = m.get("frame_sizes", {}).get(st, m["frame_size"])
    assert sorted(g) == ["carry", "hold", "lift", "present", "stuff", "throw", "yank"]
    for anim, cells in g.items():
        assert len(cells) == m["animations"][anim]["frames"], anim
    for anim, idx in (("hold", (0, 1)), ("carry", (0, 1, 2, 3)), ("lift", (3,)), ("throw", (0,)), ("present", (0, 1, 2))):
        for i in idx:
            x, y = (v * 128 / fs for v in g[anim][i])
            im = frame(out, st, anim, i)
            _, y0, _, _ = head_box(im)
            assert y <= y0 + 3 and y >= y0 - 10, (st, anim, i, y, y0)
            assert abs(x - 64) < 14, (st, anim, i, x)
    x, y = (v * 128 / fs for v in g["lift"][0])
    assert y > 110 and x > 80, (st, x, y)


@pytest.mark.parametrize("st", STAGES)
def test_strain_pushes_forward_and_slump_sits_on_the_floor(out, st):
    """끙차: 손이 몸 앞 거대한 것 옆면 (프레임 x ≈ STRAIN_HANDS_X)까지 · 털썩: 머리가 서 있을 때보다 한참 낮다."""
    for i in range(4):
        ink = near(frame(out, st, "strain", i), hm.INK, 40)
        xs = np.nonzero(ink.any(axis=0))[0]
        assert abs(xs.max() - hm.STRAIN_HANDS_X) <= 6, (st, i, xs.max())
    stand_low = head_box(frame(out, st, "idle", 0))[3]
    for i in range(2):
        low = head_box(frame(out, st, "slump", i))[3]  # 머리 아래 끝 (땀방울의 흰 반짝임은 위에 있다)
        assert low > stand_low + 12, (st, i, low, stand_low)


def test_taskbar_rows_play_like_the_core_expects(out):
    """코어 taskbar.rs: yank 4칸 6fps 반복 (YANK_MIN_SECS 1초 남짓 한 바퀴), stuff 4칸 6fps 한 번 (넷째 칸 = STUFF_RELEASE 0.5초에 쏙)."""
    a = json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))["animations"]
    assert (a["yank"]["frames"], a["yank"]["fps"], a["yank"].get("loop"), a["yank"].get("front")) == (4, 6, None, None)
    assert (a["stuff"]["frames"], a["stuff"]["fps"], a["stuff"].get("loop"), a["stuff"].get("front")) == (4, 6, False, None)


@pytest.mark.parametrize("st", STAGES)
def test_yank_and_stuff_grip_the_floor_in_front(out, st):
    """끌어내기 칸은 모두, 밀어 넣기는 셋째·넷째 칸에서 두 손이 몸 앞 바닥께 (프레임 YANK_HANDS) — 장난감이 바닥에서 나오고 바닥으로 들어간다.
    밀어 넣기 첫 칸은 머리 위 (hold처럼)."""
    m = json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))
    g = m["grip"][st]
    fs = m.get("frame_sizes", {}).get(st, m["frame_size"])
    for anim, idx in (("yank", (0, 1, 2, 3)), ("stuff", (2, 3))):
        for i in idx:
            x, y = (v * 128 / fs for v in g[anim][i])
            assert abs(x - hm.YANK_HANDS[0]) < 6 and abs(y - hm.YANK_HANDS[1]) < 7 and y < 121, (st, anim, i, x, y)
    x, y = (v * 128 / fs for v in g["stuff"][0])
    _, y0, _, _ = head_box(frame(out, st, "stuff", 0))
    assert y <= y0 + 3 and abs(x - 64) < 14, (st, x, y, y0)


# ---- 🎨 몸 색 (사용자: 어두운 바탕화면에서 짙은 잉크 선이 안 보인다) — 앱이 ink_<stage>.png 자리만 고른 색으로 덮는다 (ui/main.js tint)
def ink_frame(out, st, anim, i, root="pets"):
    d = out / "heumul" if root == "pets" else out
    return cell(sheet_128(d / f"ink_{st}.png"), names(hm.HEUMUL).index(anim), i)


def test_manifest_says_the_body_can_be_tinted_and_only_heumul_does(out):
    assert json.loads((out / "heumul" / "pet.json").read_text(encoding="utf-8"))["tint"] is True
    for d in ("pets", "pets_pixel"):
        for p in sorted((ROOT / "ui" / d).iterdir()):
            inks = sorted(f.name for f in p.glob("ink_*.png"))
            if p.name == "heumul":
                assert inks == sorted(f"ink_{st}.png" for st in STAGES), (d, inks)
            else:
                assert inks == [], (d, p.name)
                if d == "pets":
                    assert "tint" not in json.loads((p / "pet.json").read_text(encoding="utf-8")), p.name


@pytest.mark.parametrize("st", STAGES)
def test_ink_mask_has_the_sheet_layout(out, st):
    from PIL import Image
    with Image.open(out / "heumul" / f"sheet_{st}.png") as s, Image.open(out / "heumul" / f"ink_{st}.png") as m:
        assert m.size == s.size and "A" in m.getbands(), (st, m.mode)


@pytest.mark.parametrize("st", STAGES)
@pytest.mark.parametrize("anim,i", [("idle", 0), ("walk", 1), ("panic", 0), ("heart", 1), ("hang", 2), ("sleep", 0)])
def test_ink_mask_covers_the_ink_lines_but_not_the_head_or_the_effects(out, st, anim, i):
    """잉크 선(몸·팔다리·머리 테)은 마스크가 꽉, 크림 머리 속·효과(땀방울·하트·Z)는 0, 그림이 없는 곳은 0, 그림보다 짙지 않다."""
    im, mk = frame(out, st, anim, i), ink_frame(out, st, anim, i)
    m = np.asarray(mk.getchannel("A")).astype(int)
    a = np.asarray(im.getchannel("A")).astype(int)
    ink = near(im, hm.INK, 12)
    assert ink.sum() > 100 and (m[ink] >= a[ink] - 12).mean() > 0.93, (st, anim, i)
    head = near(im, hm.HEAD, 12)  # 머리 테 바로 안쪽은 잉크가 조금 번져 있다
    assert head.sum() > 100 and (m[head] <= 12).all() and (m[head] == 0).mean() > 0.7, (st, anim, i)
    assert (m <= a + 1).all() and (m[a == 0] == 0).all()
    rgb = np.asarray(im.convert("RGBA")).astype(int)[..., :3]
    chroma = rgb.max(axis=2) - rgb.min(axis=2)
    # 땀방울·하트·속도선… (잉크·크림은 무채색에 가깝다). 품에 안은 하트의 테두리는 몸 실루엣 외곽선이라 잉크 — 잉크 옆 2px은 뺀다
    near_ink = np.zeros_like(ink)
    ys, xs = np.nonzero(ink)
    for dy in range(-2, 3):
        for dx in range(-2, 3):
            near_ink[np.clip(ys + dy, 0, ink.shape[0] - 1), np.clip(xs + dx, 0, ink.shape[1] - 1)] = True
    fx = (a > 250) & (chroma > 40) & ~near_ink
    assert anim not in ("panic", "heart", "sleep") or fx.sum() > 10, (st, anim, i)
    assert not fx.any() or ((m[fx] <= 64).all() and (m[fx] == 0).mean() > 0.9), (st, anim, i, int((m[fx] > 0).sum()))


def test_pixel_ink_mask_is_hard_dots_on_the_ink_only():
    """도트: 마스크도 반투명 없이 0 / 255, 잉크 칸(선·머리 고리)만 — 크림 머리와 하트는 0."""
    import draw_pixel as px
    for st in ("baby", "adult"):
        frames = px.render_pixel(hm.HEUMUL, st, anims=("idle", "heart"))
        masks = px.render_ink_masks(hm.HEUMUL, st, anims=("idle", "heart"))
        for anim in ("idle", "heart"):
            for im, mk in zip(frames[anim], masks[anim]):
                a = np.asarray(im.convert("RGBA")).astype(int)
                m = np.asarray(mk.getchannel("A")).astype(int)
                assert set(np.unique(m)) <= {0, 255}
                dark = (a[..., 3] > 0) & (a[..., :3].sum(axis=2) < 300)
                assert dark.sum() > 20 and (m[dark] == 255).all(), (st, anim)
                assert (m[(a[..., 3] > 0) & ~dark] == 0).all(), (st, anim)
                assert (m[a[..., 3] == 0] == 0).all()


def test_shipped_pixel_ink_masks_match_the_sheets():
    from PIL import Image
    d = ROOT / "ui" / "pets_pixel" / "heumul"
    for st in STAGES:
        with Image.open(d / f"sheet_{st}.png") as s, Image.open(d / f"ink_{st}.png") as m:
            assert m.size == s.size, st
            sa = np.asarray(s.convert("RGBA")).astype(int)
            ma = np.asarray(m.convert("RGBA"))[..., 3]
            dark = (sa[..., 3] > 0) & (sa[..., :3].sum(axis=2) < 300)
            assert (ma[dark] == 255).all() and (ma[~dark] == 0).all(), st
