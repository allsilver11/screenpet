"""수면 모자·집중 고리: 모든 동물 × 단계 × 동작 × 칸에서 머리에 딱 맞고, 몸과 한 덩어리로 움직이는지 (진짜 시트 + 실제 main.js 계산)."""

import json
import math
import re
from pathlib import Path

import numpy as np
import pytest
from PIL import Image, ImageDraw

import draw_pets  # 먼저: draw_halloween을 먼저 부르면 draw_pets 끝의 순환 import가 빈 모듈을 만나 깨진다
import draw_halloween  # noqa: I001
import draw_snowman
from conftest import needs_node, node_json
import preview_nightcap as pn
import preview_scarf as ps
from sprite_spec import FRAME

ROOT = Path(__file__).resolve().parent.parent
PETS = ROOT / "ui" / "pets"
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
HTML = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")
HARNESS = ROOT / "tools" / "nightcap_harness.js"

# 그림에 쓰는 눈 색 (draw_pets EYE). 눈사람은 석탄, 호박은 조각한 눈 (테두리·안쪽 불빛)
EYE_COLORS = {"snowman": [draw_snowman.COAL], "pumpkin": [draw_halloween.CARVE, draw_halloween.GLOW_Y]}
DEFAULT_EYE_COLORS = [draw_pets.EYE]
# 슬픈 눈썹도 눈과 같은 색이라 눈 색으로는 가려졌는지 가릴 수 없다 — 모자가 눈썹만 살짝 덮는 동작 (머리 자리는 앉은 동작과 같은 식이다)
BROW_ANIMS = {"hungry"}
BRIM_X = [8, 11, 14, 16.5, 19, 22, 25]


def cells():
    for pid in pn.SPECIES:
        man = pn.manifest(pid)
        for st in pn.STAGES:
            for anim, spec in man["animations"].items():
                for f in range(spec["frames"]):
                    yield pid, man, st, anim, spec, f


def check_fit(cell, mat, eyes=True, pid=None):
    """문제 목록. cell: 칸 RGBA 배열 · mat: 모자 svg 점 → 칸 좌표 · eyes: 눈도 본다 · pid: 그 동물의 눈 색으로."""
    problems = []
    alpha = cell[..., 3]
    # 1. 뜨지 않았다: 모자 입구 띠 아래 가장자리가 몸 위에 얹힌다
    hits = 0
    for x in BRIM_X:
        # 입구 아래 곡선 (viewBox): y = 25 → 가운데 22.5 → 25
        t = (x - 3) / 27
        lip = (1 - t) ** 2 * 25 + 2 * (1 - t) * t * 20 + t ** 2 * 25
        px, py = mat(x, lip)
        ix, iy = int(round(px)), int(round(py))
        if 0 <= ix < FRAME and 0 <= iy < FRAME and alpha[iy, ix] > 128:
            hits += 1
    if hits < 5:
        problems.append(f"floating:{hits}/{len(BRIM_X)}")
    if not eyes:
        return problems
    # 2. 눈을 덮지 않는다
    mask = Image.new("L", (FRAME, FRAME), 0)
    d = ImageDraw.Draw(mask)
    for _, pts, *_ in pn.CAP:
        d.polygon([mat(*p) for p in pts], fill=255)
    cover = np.asarray(mask) > 0
    rgb = cell[..., :3].astype(np.int16)
    eye = np.zeros(cover.shape, bool)
    for c in EYE_COLORS.get(pid, DEFAULT_EYE_COLORS):
        eye |= (np.abs(rgb - np.array(c)).sum(axis=2) < 14) & (alpha > 200)
    if eye.sum() >= 3:
        hidden = (eye & cover).sum() / eye.sum()
        # 낱개 픽셀 한두 개는 잡음이다 (또렷한 2배 칸을 128로 줄이면 눈 끝·속눈썹 픽셀이 몇 개만 눈 색으로 남아 비율이 크게 튄다)
        if hidden > 0.1 and (eye & cover).sum() >= 3:
            problems.append(f"eyes:{hidden:.2f}")
    return problems


_SHEETS = {}


def cell_of(pid, st, row, f):
    key = (pid, st)
    if key not in _SHEETS:
        _SHEETS.clear()
        _SHEETS[key] = np.asarray(pn.layer_128(pid, st, f"sheet_{st}.png"))
    return _SHEETS[key][row * FRAME:(row + 1) * FRAME, f * FRAME:(f + 1) * FRAME]


# ---- 표 ----


def test_every_cell_has_head_data():
    for pid, man, st, anim, spec, f in cells():
        entry = man["head"][st][anim][f]
        assert len(entry) == 4 and entry[2] > 4, (pid, st, anim, f, entry)


def test_cap_sits_on_the_head_in_every_cell():
    bad = []
    for pid, man, st, anim, spec, f in cells():
        mat = pn.new_cap_matrix_cell(man["head"][st][anim][f])
        bad += [(pid, st, anim, f, p) for p in check_fit(cell_of(pid, st, spec["row"], f), mat, anim not in BROW_ANIMS and "neck" not in man, pid)]  # neck 표가 있는 동물(산타)은 모자 대신 목도리 (ui/main.js capOn) — 눈을 덮을 모자가 없다
    assert not bad, f"{len(bad)}: {bad[:40]}"


def test_cap_does_not_jump_between_frames():
    """칸 사이 움직임은 머리의 실제 움직임 안에서만."""
    for pid in pn.SPECIES:
        man = pn.manifest(pid)
        for st in pn.STAGES:
            for anim, rows in man["head"][st].items():
                for a, b in zip(rows, rows[1:]):
                    assert math.hypot(a[0] - b[0], a[1] - b[1]) <= 46, (pid, st, anim, a, b)
                    assert max(a[2], b[2]) / min(a[2], b[2]) <= 1.35, (pid, st, anim, a, b)


def test_cap_and_ring_stay_inside_the_window():
    """창 = 칸 + 위 36px 말풍선 자리 (기린 목 늘이기는 STRETCH_SPACE 52 × 배율을 더 받는다)."""
    for pid in pn.SPECIES:
        man = pn.manifest(pid)
        for st in pn.STAGES:
            side = 128 * pn.STAGE_SCALE[st] * man["stage_size"].get(st, 1.0)
            k = side / 128
            win = 128 * max(1, side / 128)
            for anim, rows in man["head"][st].items():
                neck = anim == "neckstretch"
                rise = man["neck_stretch"][st]["ext"] * k if neck else 0
                room = 36 + (52 * k if neck else 0)
                for e in rows:
                    m = pn.new_cap_matrix_cell(e)
                    pts = [m(*p) for shape in pn.CAP for p in shape[1]]
                    assert min(p[1] for p in pts) * k - rise + room >= 0, (pid, st, anim, e)
                    left = (win - side) / 2
                    assert left + min(p[0] for p in pts) * k >= 0 and left + max(p[0] for p in pts) * k <= win, (pid, st, anim, e)
                    assert (e[1] - 0.6 * e[2]) * k - 22 - rise + room >= 0, ("고리", pid, st, anim, e)


def test_head_data_matches_the_generator():
    """표는 그림 코드에서 나온다 — 고치고 다시 만들지 않으면 여기서 걸린다 (한 종 한 단계만, 느리므로)."""
    import draw_pets as dp

    with dp.stage("adult", dp.SPECIES["dino_green"].stage_tweak.get("adult")):
        sp = dp.staged_species(dp.SPECIES["dino_green"])
        dp.frames(sp, "sleep", 2)
    assert dp._HEAD_LOG["sleep"] == pn.manifest("dino_green")["head"]["adult"]["sleep"]


# ---- 실제 main.js 계산 ----


def run_layout(entries, k, cap_on, flip):
    a = MAIN.index("// HEADGEAR-LAYOUT-START")
    b = MAIN.index("// HEADGEAR-LAYOUT-END")
    js = f"{MAIN[a:b]}\nconsole.log(JSON.stringify({json.dumps(entries)}.map((e) => headgearLayout(e, {k}, {json.dumps(cap_on)}, {json.dumps(flip)}))));"
    return node_json("-e", js)


def parse_cap(css):
    tx, ty, rot, sx, sy, bx, by = (float(v) for v in re.match(
        r"translate\((.+)px, (.+)px\) rotate\((.+)deg\) scale\((.+), (.+)\) translate\((.+)px, (.+)px\)", css).groups())
    c, s = math.cos(math.radians(rot)), math.sin(math.radians(rot))

    def m(px, py):
        qx, qy = (px + bx) * sx, (py + by) * sy
        return (c * qx - s * qy + tx, s * qx + c * qy + ty)

    return m


@needs_node
def test_python_preview_matches_main_js():
    entries = [[64, 20, 50, 0], [80.5, 31.2, 42.1, -17.3], [30, 60, 25.5, 90]]
    for entry, out in zip(entries, run_layout(entries, 1, True, False)):
        js_m, py_m = parse_cap(out["cap"]), pn.new_cap_matrix_cell(entry)
        for p in [(3, 21), (30, 25), (29, 15), (16.5, 20.5)]:
            assert js_m(*p) == pytest.approx(py_m(*p), abs=0.02)


@needs_node
def test_real_js_layout_fits_a_real_dino():
    man = pn.manifest("dino_green")
    for st in pn.STAGES:
        entries = man["head"][st]["sleep"]
        for f, out in enumerate(run_layout(entries, 1, True, False)):
            assert not check_fit(cell_of("dino_green", st, man["animations"]["sleep"]["row"], f), parse_cap(out["cap"])), (st, f)


@needs_node
def test_ring_floats_above_the_head_and_above_the_cap():
    entry = [64, 30, 50, 0]
    off, on = run_layout([entry], 1, False, False)[0], run_layout([entry], 1, True, False)[0]
    y = lambda css: float(re.match(r"translate\(.+px, (.+)px\)", css).group(1))  # noqa: E731
    assert y(off["ring"]) + 20 <= 30
    assert y(on["ring"]) < y(off["ring"])
    assert "scaleX(-1)" in run_layout([entry], 1, True, True)[0]["ring"], "반전된 몸 안에서 진행 방향 유지"


def test_cap_and_ring_are_children_of_the_body():
    """위치를 따로 계산하지 않는다 — #pet의 자식이라 몸의 변환을 그대로 받는다."""
    pet = HTML[HTML.index('<div id="pet">'):HTML.index('<div id="envelope"')]
    assert '<svg id="nightcap"' in pet and '<svg id="focusring"' in pet
    fn = MAIN[MAIN.index("function placeHeadgear"):MAIN.index("// 눈사람의 작은 눈구름")]
    assert "clientWidth" not in fn and "innerWidth" not in fn


# ---- 움직임 시나리오: common.js + main.js를 가짜 DOM에서 그대로 ----


def harness(pid, st="teenager"):
    return node_json(HARNESS, pid, st)


@needs_node
@pytest.mark.parametrize("pid", ["dino_green", "cat", "bird", "snowman"])
def test_cap_moves_as_one_with_the_body(pid):
    r = harness(pid)
    base = r["base"]
    assert base["capParent"] == "pet" and base["ringParent"] == "pet"
    assert not base["capHidden"]
    # 매달림·덜덜·말풍선·편지로 창이 바뀌어도: 몸만 변환을 받고 모자는 칸 안 같은 자리 → 몸과 같이 움직인다
    for name in ("hangRight", "hangLeft", "wobble", "bubbleWindow", "noteWindow", "base2"):
        assert r[name]["cap"] == base["cap"], name
        assert r[name]["ring"] == base["ring"], name
        assert r[name]["capParent"] == "pet", name
    # 매달리면 몸은 커서 끝(머리 꼭대기)을 축으로 흔들린다 — 옆으로 옮기지 않는다
    cells = pn.manifest(pid)["head"]["teenager"]["dragged"]
    gx, gy = pn.hang_grip_cell(cells)
    for name in ("hangRight", "hangLeft"):
        assert "rotate" in r[name]["pet"] and "translateX" not in r[name]["pet"], name
        assert r[name]["petOrigin"] == f"{gx / 128 * 100:.2f}% {gy / 128 * 100:.2f}%", name
    # 반전은 몸이 받는다 — 모자 자리는 그대로, 고리만 진행 방향을 되돌린다
    assert r["flip"]["cap"] == base["cap"]
    assert "scaleX(-1)" in r["flip"]["pet"]
    assert "scaleX(-1)" in r["flip"]["ring"]
    # 구르기: 회전은 몸이 받고 모자는 칸 안에서 머리 자리에 있다 (숨지 않는다)
    assert not r["roll"]["capHidden"] and r["roll"]["capParent"] == "pet"
    # 없는 동작은 idle로 그리므로 idle 자리
    assert r["unknownAnim"]["cap"] == base["cap"], (r["unknownAnim"]["cap"], base["cap"])


@needs_node
def test_giraffe_cap_rides_the_head_layer_while_the_neck_stretches():
    r = harness("giraffe")
    assert r["base"]["capParent"] == "pet"
    assert r["neck"]["capParent"] == "nk-head" and r["neck"]["ringParent"] == "nk-head"
    assert r["neckMid"]["capParent"] == "nk-head"
    assert r["afterNeck"]["capParent"] == "pet", "돌아온다"


@needs_node
def test_cap_poofs_away_once_in_the_morning():
    r = harness("cat")
    assert r["startOff"]["capHidden"] and r["startOff"]["sparks"] == 0, "처음부터 꺼져 있으면 효과 없이"
    assert "cap-poof" in r["morning"]["capClasses"] and not r["morning"]["capHidden"]
    assert r["morning"]["sparks"] - r["afterNeck"]["sparks"] == 5
    assert r["morningDone"]["capHidden"] and not r["morningDone"]["capClasses"]
    assert r["hiddenWindow"]["capHidden"] and not r["hiddenWindow"]["capClasses"], "창이 숨어 있으면 바로"


def test_click_through_turns_on_before_the_window_moves_over_the_cursor():
    slots = (ROOT / "src-tauri" / "src" / "slots.rs").read_text(encoding="utf-8")
    place = slots[slots.index("pub fn place(&mut self"):]
    assert place.index("self.sync_click_through()") < place.index("self.move_window(")


# ---- 산타의 밤 목도리: 모자를 이미 썼으니 수면 모자 대신 파란 목도리 (#pet 자식 오버레이, pet.json neck) ----


def scarf_mask(view, entry):
    d_size = FRAME * 2
    return np.asarray(ps.draw_scarf(view, ps.scarf_matrix(entry), 2, d_size).getchannel("A").resize((FRAME, FRAME), Image.BILINEAR)) > 100


def santa_cells():
    man = pn.manifest("santa")
    for st in pn.STAGES:
        for anim, spec in man["animations"].items():
            for f in range(spec["frames"]):
                yield man, st, anim, spec, f


def test_only_santa_has_neck_data_and_every_santa_cell_has_it():
    for pid in pn.SPECIES:
        assert ("neck" in pn.manifest(pid)) == (pid == "santa"), pid
    for man, st, anim, spec, f in santa_cells():
        assert len(man["neck"][st][anim][f]) == 4, (st, anim, f)


def test_no_extra_santa_sheets():
    assert not list((PETS / "santa").glob("*night*"))


def test_scarf_sits_on_the_body_and_leaves_the_face_alone():
    bad = []
    for man, st, anim, spec, f in santa_cells():
        entry = man["neck"][st][anim][f]
        mask = scarf_mask(ps.scarf_view(anim, f), entry)
        cell = cell_of("santa", st, spec["row"], f)
        alpha = cell[..., 3] > 128
        # 목도리 기준점 둘레가 몸 위다
        x, y = int(round(entry[0])), int(round(entry[1]))
        if not alpha[max(0, y - 2):y + 5, max(0, x - 8):x + 9].any():
            bad.append(("floating", st, anim, f))
        # 칸(창)을 벗어나지 않는다
        ys, xs = np.nonzero(mask)
        if xs.min() < 1 or xs.max() > FRAME - 2 or ys.max() > FRAME - 2:
            bad.append(("outside", st, anim, f, int(xs.min()), int(xs.max()), int(ys.max())))
        # 얼굴을 덮지 않는다: 눈 줄 (모자 입구 표시 아래 0.65 × 머리 반지름, 입구 너비 = 머리 반지름 × 1.9) 둘레. 뒷모습은 얼굴이 없다
        hx, hy, hw, _ = man["head"][st][anim][f]
        hr = hw / 1.9
        if ps.scarf_view(anim, f) != "back":
            ey = int(hy + 0.65 * hr)
            # 앞모습 얼굴은 가운데, 옆모습 얼굴은 앞쪽(오른쪽) — 뒤로 올라간 목도리 끝이 볼 뒤에 닿는 건 괜찮다
            x0, x1 = (int(hx - 0.6 * hr), int(hx + 0.6 * hr)) if ps.scarf_view(anim, f) == "front" else (int(hx), int(hx + 0.7 * hr))
            if mask[ey - 4:ey + 5, x0:x1].any():
                bad.append(("face", st, anim, f))
    assert not bad, f"{len(bad)}: {bad[:30]}"


def test_scarf_hides_the_green_sled_scarf():
    """썰매: 원래 그려진 초록 목도리가 파란 목도리 밖으로 삐져나오지 않는다 (장갑도 같은 색이라 몸 뒤쪽만 본다)."""
    for man, st, anim, spec, f in santa_cells():
        if anim != "roll":
            continue
        cell = cell_of("santa", st, spec["row"], f)
        green = (np.abs(cell[..., :3].astype(np.int16) - np.array((92, 168, 112))).sum(axis=2) < 40) & (cell[..., 3] > 128)
        green[:, 50:] = False
        mask = scarf_mask(ps.scarf_view(anim, f), man["neck"][st][anim][f])
        assert green.sum() > 20, (st, f)
        assert (green & ~mask).sum() <= max(1, 0.03 * green.sum()), (st, f, int((green & ~mask).sum()), int(green.sum()))


def test_scarf_is_the_same_blue_as_the_nightcap_with_white_fringe():
    cap = re.search(r'<svg id="nightcap".*?</svg>', HTML, re.S).group(0)
    scarf = re.search(r'<svg id="scarf".*?</svg>', HTML, re.S).group(0)
    for color in ("#6f86c9", "#3f4f86", "#fff7e8"):
        assert color in cap and color in scarf
    pet = HTML[HTML.index('<div id="pet">'):HTML.index('<div id="envelope"')]
    assert '<svg id="scarf"' in pet
    assert set(ps.GROUPS) == {"front", "side", "back", "sled0", "sled1"}


@needs_node
def test_python_scarf_preview_matches_main_js():
    a, b = MAIN.index("// HEADGEAR-LAYOUT-START"), MAIN.index("// HEADGEAR-LAYOUT-END")
    entry = [80.5, 96.2, 52.1, -7.5]
    js = f"{MAIN[a:b]}\nconsole.log(JSON.stringify({{ css: scarfLayout({json.dumps(entry)}, 1), views: [['roll', 3], ['climb', 0], ['slide', 1], ['sit_front', 0], ['sleep', 0]].map(([n, f]) => scarfView(n, f)) }}));"
    out = node_json("-e", js)
    tx, ty, rot, s, ax, ay = (float(v) for v in re.match(r"translate\((.+)px, (.+)px\) rotate\((.+)deg\) scale\((.+)\) translate\((.+)px, (.+)px\)", out["css"]).groups())
    c, sn = math.cos(math.radians(rot)), math.sin(math.radians(rot))
    m = ps.scarf_matrix(entry)
    for p in [(0, 0), (26, 8), (51, 2)]:
        qx, qy = (p[0] + ax) * s, (p[1] + ay) * s
        assert (c * qx - sn * qy + tx, sn * qx + c * qy + ty) == pytest.approx(m(*p), abs=0.05)
    assert out["views"] == ["sled1", "back", "back", "front", "side"]
    assert [ps.scarf_view(n, f) for n, f in [("roll", 3), ("climb", 0), ("slide", 1), ("sit_front", 0), ("sleep", 0)]] == out["views"]


@needs_node
def test_santa_wears_the_scarf_not_a_second_hat():
    r = harness("santa")
    assert r["startOff"]["scarfHidden"] and r["startOff"]["sparks"] == 0
    base = r["base"]
    assert base["capHidden"], "수면 모자는 산타에게 안 나온다"
    assert not base["scarfHidden"] and base["scarfParent"] == "pet"
    # 집중 고리는 그대로
    assert base["ringParent"] == "pet" and base["ring"].startswith("translate(")
    # 몸과 한 덩어리: 몸만 변환을 받고, 칸 안 자리는 그대로
    for name in ("hangRight", "hangLeft", "wobble", "bubbleWindow", "noteWindow", "flip"):
        assert r[name]["scarf"] == base["scarf"] and r[name]["scarfParent"] == "pet", name
    assert "scaleX(-1)" in r["flip"]["pet"] and "scaleX(-1)" not in r["flip"]["scarf"]
    assert r["sleep"]["scarfView"] == "side" and r["sitFront"]["scarfView"] == "front"
    assert r["climb"]["scarfView"] == "back" and r["sled"]["scarfView"] == "sled0"
    # 아침 8시 30분: 목도리가 뿅
    assert "cap-poof" in r["morning"]["scarfClasses"] and not r["morning"]["scarfHidden"] and not r["morning"]["capClasses"]
    assert r["morning"]["sparks"] - r["afterNeck"]["sparks"] == 5
    assert r["morningDone"]["scarfHidden"] and not r["morningDone"]["scarfClasses"] and r["morningDone"]["capHidden"]
    assert r["hiddenWindow"]["scarfHidden"], "창이 숨어 있으면 효과 없이"


@needs_node
def test_round_body_ignores_a_scale_change():
    r = harness("cat")
    for name in ("resizeStart", "resizeWider", "resizeDpr", "resizeDprAgain"):
        assert r[name]["width"] == "128px" and r[name]["left"] == "calc(50% - 64px)", name
        assert r[name]["loads"] == 0, name


@needs_node
def test_other_pets_keep_the_nightcap_and_no_scarf():
    r = harness("cat")
    assert not r["base"]["capHidden"] and r["base"]["scarfHidden"]


def test_contact_sheet_makes_its_folder(tmp_path, monkeypatch):
    """tools/preview는 커밋되지 않는다 — 새 체크아웃에서도 미리보기가 저장된다."""
    monkeypatch.setattr(pn, "page", lambda pid: Image.new("RGBA", (4, 4)))
    out = tmp_path / "new" / "dir" / "nightcap.png"
    pn.contact(out)
    assert out.is_file()


def test_preview_has_no_old_head_finder():
    """고치기 전 계산(before 모드)과 그 scipy 의존은 없어졌다 — CI에 없는 패키지를 부르지 않는다."""
    src = Path(pn.__file__).read_text(encoding="utf-8")
    assert "scipy" not in src and "old_head_top" not in src
