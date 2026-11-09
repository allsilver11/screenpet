"""픽셀 세계 그림: 파일이 다 있고, 칸 그림이고 (안티에일리어싱 없음), 문에 동물이 들어가고, 동글 그림은 그대로다."""

import ast
import json
import re
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_pixel_world as w
import pixel_world_icons as icons
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"


def alpha_is_hard(path):
    a = np.asarray(Image.open(path).convert("RGBA"))[..., 3]
    return set(np.unique(a).tolist()) <= {0, 255}


def test_round_art_is_byte_identical():
    want = json.loads((ROOT / "tools" / "round_art_hashes.json").read_text(encoding="utf-8"))
    assert w.round_digest() == want


def test_stage_grid_matches_the_pet_sheets():
    src = (ROOT / "tools" / "draw_pixel.py").read_text(encoding="utf-8")
    table = re.search(r"STAGE_GRID = (\{.*?\})", src).group(1)
    assert ast.literal_eval(table) == w.STAGE_GRID


@pytest.mark.parametrize("kind", w.KINDS)
@pytest.mark.parametrize("stage", w.STAGES)
def test_house_files_are_hard_dots_of_the_right_size(kind, stage):
    d = UI / "house_pixel"
    meta = json.loads((d / f"{kind}_{stage}.json").read_text(encoding="utf-8"))
    n = w.HOUSE_DOTS[stage]
    assert meta["dots"] == n and meta["dot"] == w.STAGE_GRID[stage][1]
    for name in (f"{kind}_{stage}.png", f"{kind}_{stage}_front.png"):
        assert Image.open(d / name).size == (n, n)
        assert alpha_is_hard(d / name)


@pytest.mark.parametrize("kind", w.KINDS)
@pytest.mark.parametrize("stage", w.STAGES)
def test_door_is_dark_inside_and_open_in_front(kind, stage):
    d = UI / "house_pixel"
    door = json.loads((d / f"{kind}_{stage}.json").read_text(encoding="utf-8"))["door"]
    back = np.asarray(Image.open(d / f"{kind}_{stage}.png").convert("RGBA"))
    front = np.asarray(Image.open(d / f"{kind}_{stage}_front.png").convert("RGBA"))
    cx, cy = door["x"] + door["w"] // 2, door["y"] + door["h"] // 2
    assert back[cy, cx, 3] == 255 and back[cy, cx, :3].sum() < 330
    x0, x1 = door["x"] + 1, door["x"] + door["w"] - 1
    # 맨 아랫줄은 문턱 외곽선이 발끝을 살짝 덮는다 (동글의 문턱과 같다)
    y0, y1 = door["y"] + 7, door["y"] + door["h"] - 1
    assert front[y0:y1, x0:x1, 3].max() == 0, "문 안쪽은 앞 그림이 비어 있어야 한다"
    # 앞 레이어: 문틀과 문턱이 있다
    assert front[door["y"] + door["h"], door["x"] + door["w"] // 2, 3] == 255
    assert door["x"] - 2 >= 0 and door["x"] + door["w"] + 1 < w.HOUSE_DOTS[stage]


@pytest.mark.parametrize("stage", w.STAGES)
def test_door_fits_a_whole_sheet_frame_at_the_pets_own_dot(stage):
    """문은 시트 한 칸 전체 + 외곽선만큼 — 어떤 동물이든 줄이지 않고 앉는다."""
    n, dot = w.STAGE_GRID[stage]
    assert dot == 3, "동물·집·세계 그림이 같은 칸"
    for kind in w.KINDS:
        door = w.DRAW[kind](stage)[2]
        assert door["w"] >= n + 2 and door["h"] >= n + 1
    # 몸이 동글 문간 동물보다 작지 않다 (동글 32/43/56/71/82 px)
    assert n * dot >= {"baby": 32, "kinder": 43, "child": 56, "teenager": 71, "adult": 82}[stage]
    # 집 한 변: 114/126/144/144/162 px ("동물이 너무 크다"는 피드백으로 teenager·adult 칸을 4 → 3으로)
    assert w.HOUSE_DOTS[stage] * dot == {"baby": 114, "kinder": 126, "child": 144, "teenager": 144, "adult": 162}[stage]


@pytest.mark.parametrize("kind", w.KINDS)
@pytest.mark.parametrize("stage", w.STAGES)
def test_every_resident_sheet_on_disk_fits_the_door(kind, stage):
    """있는 시트로 실제로 재 본다: 앉은 모습(sit_front)이 문 위 계단 모서리·양옆 외곽선에 걸리지 않는다."""
    door = w.DRAW[kind](stage)[2]
    for pid in w.RESIDENTS[kind]:
        p = UI / "pets_pixel" / pid
        sheet = p / f"sheet_{stage}.png"
        m = json.loads((p / "pet.json").read_text(encoding="utf-8"))
        n = m["frame_sizes"][stage]
        assert n <= w.STAGE_GRID[stage][0], "공룡만 칸 수가 더 적다"
        s = m["animations"]["sit_front"]
        im = Image.open(sheet).convert("RGBA")
        for f in range(s["frames"]):
            cell = np.asarray(im.crop((f * n, s["row"] * n, (f + 1) * n, (s["row"] + 1) * n)))[..., 3] > 0
            left = (door["w"] - n) // 2
            ys, xs = np.nonzero(cell)
            assert xs.min() + left >= 1 and xs.max() + left <= door["w"] - 2, (pid, stage)
            assert ys.max() < n


def test_xmas_smoke_sits_on_the_chimney_and_steps_through_four_scenes():
    d = UI / "house_pixel"
    for st in w.STAGES:
        meta = json.loads((d / f"xmas_{st}.json").read_text(encoding="utf-8"))
        back = np.asarray(Image.open(d / f"xmas_{st}.png").convert("RGBA"))
        x, y = meta["smoke"]["x"], meta["smoke"]["y"]
        assert back[y + 1, x, 3] == 255
    strip = Image.open(d / "xmas_smoke.png")
    assert strip.size == (w.SMOKE_DOTS * 4, w.SMOKE_DOTS)
    assert alpha_is_hard(d / "xmas_smoke.png")


def round_names(folder):
    return sorted(p.stem for p in (UI / folder).glob("*.png"))


def test_every_round_gift_has_a_pixel_gift_and_shiny():
    names = round_names("gifts")
    assert len(names) == 180  # v0.5.5: 곰·꿀벌·강아지 선물 −36, 펭귄 +12, 레서판다 +12, 다람쥐 +12
    assert round_names("gifts_pixel") == names
    for n in names:
        p = UI / "gifts_pixel" / f"{n}.png"
        assert Image.open(p).size == (w.GIFT_DOTS, w.GIFT_DOTS)
        assert alpha_is_hard(p)


def test_gift_art_leaves_room_for_the_outline():
    for table, side in ((icons.GIFTS, 10), (icons.HAND, 10)):
        for name, text in table.items():
            rows = icons.art(text)
            assert len(rows) <= side and max(map(len, rows)) <= side, name


def test_hand_table_draws_exactly_the_round_hand_foods():
    assert {f"hand_{n}" for n in icons.HAND} == {n for n in round_names("effects") if n.startswith("hand_")}


def test_hand_lines_are_thin_dots_drawn_after_the_outline():
    """가는 선(새우 더듬이)은 외곽선 뒤에 1칸으로 — 외곽선을 두르면 3칸 덩어리가 되어 뿔처럼 보인다."""
    assert set(icons.HAND_LINES) <= set(icons.HAND)
    for name, text in icons.HAND_LINES.items():
        rows = icons.art(text)
        assert len(rows) <= w.HAND_DOTS and max(map(len, rows)) <= w.HAND_DOTS, name
        assert set("".join(rows)) <= set(w.PAL) | {"."}, name


def test_pixel_shrimp_has_feelers_reaching_the_top_corner():
    a = np.asarray(Image.open(UI / "effects_pixel" / "hand_shrimp.png").convert("RGBA"))
    feeler = (a[..., :3] == w.PAL["R"]).all(axis=-1) & (a[..., 3] > 0)
    ys, xs = np.nonzero(feeler)
    assert len(ys) >= 4 and ys.min() == 0, "더듬이가 칸 맨 위까지"
    assert (a[..., 3] > 0).sum() >= 90, "칸을 채운다"


@pytest.mark.parametrize("name", ["squirrel_ginkgo", "squirrel_ginkgo_shiny"])
def test_ginkgo_is_a_tilted_fan_on_a_stem_not_a_heart(name):
    a = np.asarray(Image.open(UI / "gifts_pixel" / f"{name}.png").convert("RGBA"))
    solid = a[..., 3] > 0
    assert (solid != solid[:, ::-1]).sum() >= 20, "하트처럼 좌우 대칭이 아니다 (기울어진 부채)"
    rows = icons.art(icons.GIFTS["squirrel_ginkgo"])
    assert sum(r.count("d") for r in rows) >= 3, "잎자루"


def test_shiny_differs_from_plain_and_has_white_sparkle_dots():
    a = np.asarray(Image.open(UI / "gifts_pixel" / "cat_yarn.png").convert("RGBA"))
    b = np.asarray(Image.open(UI / "gifts_pixel" / "cat_yarn_shiny.png").convert("RGBA"))
    assert (a != b).any()
    assert (b[..., :3] == 255).all(axis=-1).sum() >= 3


def test_every_round_effect_has_a_pixel_effect():
    have = set(round_names("effects_pixel"))
    missing = [n for n in round_names("effects") if n not in have]
    assert not missing
    for p in (UI / "effects_pixel").glob("*.png"):
        assert alpha_is_hard(p), p.name
    for n in ("nightcap", "scarf", "fx_heart", "fx_star", "fx_spark", "fx_spark2", "fx_envelope", "fx_pad", "snowball_0", "snowball_1", "snowball_2"):
        assert (UI / "effects_pixel" / f"{n}.png").is_file(), n


@pytest.mark.parametrize("name,size", [("bowl_food", w.PROP_DOTS), ("cookie", w.PROP_DOTS), ("hand_special", (12, 12)),
                                       ("snowman", w.SNOWMAN_DOTS), ("snowball", w.SNOWBALL_DOTS), ("snow_cloud", w.CLOUD_DOTS)])
def test_effect_sizes_are_the_round_window_at_four_px_a_dot(name, size):
    assert Image.open(UI / "effects_pixel" / f"{name}.png").size == tuple(size)


def _outlined_icons():
    yield from (("gift " + n, t, w.GIFT_DOTS, w.GIFT_DOTS, False) for n, t in icons.GIFTS.items())
    yield from (("hand " + n, t, w.HAND_DOTS, w.HAND_DOTS, False) for n, t in icons.HAND.items())
    yield from ((n, t, *w.PROP_DOTS, True) for n, t in icons.PROPS.items())
    yield from ((f"snowball {i}", t, *w.SNOWBALL_DOTS, True) for i, t in enumerate(icons.SNOWBALLS))
    yield "snowman", icons.SNOWMAN, *w.SNOWMAN_DOTS, True
    yield "snow_cloud", icons.SNOW_CLOUD, *w.CLOUD_DOTS, False
    yield "postit", icons.POSTIT, w.POSTIT_DOTS, w.POSTIT_DOTS, False


@pytest.mark.parametrize("name,text,cw,ch,bottom", list(_outlined_icons()), ids=lambda v: v if isinstance(v, str) and len(v) < 20 else "")
def test_every_outlined_icon_leaves_room_for_its_outline(name, text, cw, ch, bottom):
    """표가 캔버스에 꽉 차면 윗줄이 잘리거나(눈사람 모자) 위 외곽선만 빠진다 (꽃·눈구름)."""
    rows = icons.art(text)
    aw = max(len(r) for r in rows)
    x = (cw - aw) // 2
    y = ch - len(rows) - 1 if bottom else (ch - len(rows)) // 2
    assert x >= 1 and x + aw <= cw - 1 and y >= 1 and y + len(rows) <= ch - 1, (x, y, aw, len(rows))


@pytest.mark.parametrize("name,ink", [("snowman", w.INK), ("snow_cloud", (150, 170, 200))])
def test_the_shipped_icon_has_a_closed_top_outline(name, ink):
    a = np.asarray(Image.open(UI / "effects_pixel" / f"{name}.png").convert("RGBA"))
    ys, xs = np.nonzero(a[..., 3])
    top = a[ys.min(), xs[ys == ys.min()]]
    assert (top[:, :3] == ink).all(), top


def test_objects_sit_where_the_tumble_shape_says():
    """코어 tumble.rs의 볼록 껍질(128칸)과 칸 그림의 네모가 몇 칸 안에서 맞는다 — 구를 때 바닥에 닿는 자리가 같다."""
    src = (ROOT / "crates" / "screenpet-core" / "src" / "tumble.rs").read_text(encoding="utf-8")
    for kind in ("can", "paper", "stone", "glasses", "computer", "milk", "shirt", "umbrella", "poop", "pink", "rainbow", "pellets"):
        title = kind.capitalize()
        hull_txt = re.search(rf"ObjectKind::{title} => Shape .*?hull: &\[(.*?)\] \}}", src).group(1)
        pts = [(float(a), float(b)) for a, b in re.findall(r"\(([\d.]+), ([\d.]+)\)", hull_txt)]
        a = np.asarray(Image.open(UI / "objects_pixel" / f"{kind}.png").convert("RGBA"))
        assert a.shape[:2] == (w.OBJECT_DOTS, w.OBJECT_DOTS)
        ys, xs = np.nonzero(a[..., 3])
        k = 128 / w.OBJECT_DOTS
        x0, x1 = min(p[0] for p in pts), max(p[0] for p in pts)
        y0, y1 = min(p[1] for p in pts), max(p[1] for p in pts)
        assert abs(xs.min() * k - x0) <= 14 and abs((xs.max() + 1) * k - x1) <= 14, kind
        assert abs(ys.min() * k - y0) <= 16 and abs((ys.max() + 1) * k - y1) <= 8, kind
    assert (UI / "objects_pixel" / "postit.png").is_file()


def node_run(marker_file, start, end, tail):
    src = (UI / marker_file).read_text(encoding="utf-8")
    body = src[src.index(start): src.index(end)]
    return node_json("-e", body + "\n" + tail)


def test_pixel_headgear_stays_on_whole_dots_and_quarter_turns():
    r = node_run("main.js", "// PIXEL-HEADGEAR-START", "// PIXEL-HEADGEAR-END", """
      const a = pixelWearLayout([12.4, 7.6, 14.2, 8], true, false, 4);
      const b = pixelWearLayout([12.4, 7.6, 14.2, 80], false, true, 3);
      console.log(JSON.stringify({a, b, cells: ringCells().length, first: ringCells()[0]}));
    """)
    assert r["a"]["cap"] == "translate(-32px, -64px) rotate(0deg)"
    assert r["a"]["capCell"] == 14 - 4
    assert r["b"]["cap"].endswith("rotate(90deg)")
    assert r["b"]["ring"].endswith("scaleX(-1)")
    for v in re.findall(r"(-?[\d.]+)px", r["a"]["ring"]):
        assert float(v) % 4 == 0
    assert r["cells"] > 20
    assert r["first"]["x"] == 5  # 맨 위 가운데에서 시작한다


def test_object_pose_snaps_to_quarters_and_keeps_the_object_on_the_floor():
    r = node_run("object.js", "// OBJECT-POSE-START", "// OBJECT-POSE-END", """
      const cells = [];
      for (let x = 0; x < 20; x++) for (let y = 0; y < 8; y++) cells.push([x + 0.5, y + 0.5]);
      console.log(JSON.stringify([0, 40, 46, 89, 135, 359].map((a) => pixelPose(cells, [10, 4], a))));
    """)
    assert [p["quarter"] for p in r] == [0, 0, 90, 90, 180, 360]
    assert r[0]["drop"] == pytest.approx(0)
    # 네모 그림을 40도로 세우면 축이 그만큼 떠 있으니 그림을 내려 바닥에 닿게 한다
    assert r[1]["drop"] > 0


def test_ui_wires_every_pixel_folder_and_keeps_round_paths():
    house = (UI / "house.js").read_text(encoding="utf-8")
    assert "house_pixel" in house and '"house"' in house
    assert "gifts_pixel" in (UI / "gift.js").read_text(encoding="utf-8")
    assert "gifts_pixel" in (UI / "book.js").read_text(encoding="utf-8")
    assert "objects_pixel" in (UI / "object.js").read_text(encoding="utf-8")
    assert "effects_pixel" in (UI / "prop.js").read_text(encoding="utf-8")
    assert "effects_pixel" in (UI / "style.css").read_text(encoding="utf-8")
    assert "art-style" in (UI / "weather.js").read_text(encoding="utf-8")
    for f in ("prop.html", "gift.html", "object.html"):
        assert "image-rendering: pixelated" in (UI / f).read_text(encoding="utf-8"), f


def test_pixel_css_has_no_blur():
    css = (UI / "style.css").read_text(encoding="utf-8")
    pixel_part = css[css.index("/* ---- 픽셀 세계"):]
    assert "text-shadow: 0" not in pixel_part
    for blur in re.findall(r"drop-shadow\(([^()]*(?:\([^()]*\)[^()]*)*)\)", pixel_part):
        assert re.search(r"\s0\s+(var|#)", blur), f"blur must be 0: {blur}"


def main_in_pixel_mode(pid, stage="teenager"):
    tools = ROOT / "tools"
    return node_json(tools / "nightcap_harness.js", pid, stage, "pixel")


@pytest.mark.parametrize("pid", ["cat", "frog"])
def test_main_window_wears_the_pixel_cap_on_whole_dots(pid):
    r = main_in_pixel_mode(pid)
    assert r["base"]["capHidden"] and not r["base"]["capPxHidden"], "SVG 대신 칸 모자"
    assert r["base"]["ringHidden"], "SVG 고리 대신 칸 고리"
    assert not r["base"]["ringPxHidden"] and r["base"]["ringPxFilled"] > 0
    for name in ("base", "sleep", "climb", "flip", "hop"):
        angle = int(re.search(r"rotate\((-?\d+)deg\)", r[name]["capPx"]).group(1))
        assert angle % 90 == 0, name
    assert r["morning"]["capPxHidden"] is False and r["morning"]["pxBits"] > 0, "뿅은 칸 조각으로"
    assert r["morningDone"]["capPxHidden"] is True


def test_santa_wears_the_pixel_scarf():
    r = main_in_pixel_mode("santa")
    assert r["base"]["capPxHidden"] and not r["base"]["scarfPxHidden"]
    assert r["base"]["scarfHidden"], "SVG 목도리는 숨는다"
    rows = {r[n]["scarfPxPos"].split()[1] for n in ("base", "sitFront", "climb")}
    assert len(rows) == 3, "옆·앞·뒤 모양이 다르다"


def test_house_window_loads_pixel_art_and_seats_the_pet_at_full_size():
    r = node_json(ROOT / "tools" / "levelup_harness.js", "house-pixel")
    assert "house_pixel/house_adult.png" in r["artAfter"]
    assert r["houseWidth"] == "162px" and r["doorWidth"] == "114px"  # 54칸·38칸 × 3
    assert r["doorPetWidth"] == "108px", "어른 고양이 그대로 (36칸 × 3)"
    assert r["doorPetLeft"] == "3px"
    assert r["dot"] == "3px" and r["pixelHouse"] is True


def dropOf_flat():
    """20×8 네모, 축 (10, 4), 0도: 축에서 가장 낮은 칸 가운데까지 (칸)."""
    return 3.5


def test_house_stays_hidden_until_its_art_is_set():
    """픽셀로 시작해도 동글 집이 먼저 비치지 않는다 (house.html 기본 그림은 house/)."""
    html = (UI / "house.html").read_text(encoding="utf-8")
    assert '<body class="art-pending">' in html
    assert re.search(r"body\.art-pending #house \{\s*visibility: hidden;", (UI / "style.css").read_text(encoding="utf-8"))
    for mode in ("house", "house-pixel"):
        r = node_json(ROOT / "tools" / "levelup_harness.js", mode)
        assert r["pendingWhileDecoding"] and not r["artWhilePending"], mode
        assert not r["pendingAfterArt"], mode
    # 그림을 끝내 못 읽어도 집은 나타난다
    assert re.search(r"setTimeout\(showHouse, ART_WAIT_MS\)", (UI / "house.js").read_text(encoding="utf-8"))


def test_pixel_body_is_measured_again_on_a_new_monitor_scale():
    """pet-stage가 웹뷰 배율보다 먼저 와도 resize 때 칸을 다시 잰다."""
    r = main_in_pixel_mode("cat")
    assert r["resizeStart"]["width"] == "96px" and r["resizeStart"]["loads"] == 0
    # 배율이 그대로면 left만 다시 맞춘다
    assert r["resizeWider"]["width"] == "96px" and r["resizeWider"]["left"] == "52px" and r["resizeWider"]["loads"] == 0
    # 110%: 칸 3.3 물리 px → 3 물리 px (32칸)
    changed = r["resizeDpr"]
    assert changed["loads"] > 0
    side = 32 * 3 / 1.1
    assert float(changed["width"].removesuffix("px")) == pytest.approx(side)
    assert float(changed["bg"].split("px")[0]) == pytest.approx(512 * side / 32)
    assert r["resizeDprAgain"]["loads"] == 0, "한 번만"


def test_style_switch_recreates_props_and_skips_the_checked_style():
    """src-tauri는 여기서 컴파일하지 않으니 순서만 읽는다."""
    src = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    arm = src[src.index("Input::SetStyle(style) =>"):src.index("Input::Feed(food) =>")]
    assert arm.index("sync_style") < arm.index("if style == config.style") < arm.index("continue;") < arm.index("own.restyle")
    # 칸 크기는 새 스타일이 정해진 뒤에 읽는다
    assert arm.index("own.restyle") < arm.index("props.restyle")
    assert "own.pixel_dot()" in arm and "weather.restyle" in arm and "show_hand" in arm
    assert arm.index("art::set") < arm.index("show_hand") and arm.index("art::set") < arm.index("weather.restyle")


def test_pixel_bits_stay_hidden_during_their_delay():
    css = (UI / "style.css").read_text(encoding="utf-8")
    bit = re.search(r"\.px-bit \{.*?\n\}", css, re.S).group(0)
    assert "opacity: 0;" in bit, "delay 동안은 기본값(숨김)"
    assert " both" not in bit and bit.count("forwards") == 2
    assert css.count("@keyframes house-hop-pixel") == 1


def test_object_layout_scales_the_pivot_with_the_dots():
    r = node_run("object.js", "// OBJECT-POSE-START", "// OBJECT-POSE-END", """
      const cells = [];
      for (let x = 0; x < 20; x++) for (let y = 0; y < 8; y++) cells.push([x + 0.5, y + 0.5]);
      console.log(JSON.stringify({
        at: [1, 1.1, 1.2, 1.5, 2].map((dpr) => ({ dpr, ...pixelMetrics(dpr, [48, 72]) })),
        same: pixelPose(cells, [10, 4], 40),
        scaled: pixelPose(cells, [10, 4], 40, 1.1),
        flat: pixelPose(cells, [10, 4], 0, 1.1),
        tall: dropOf(cells, [10, 4], 40),
      }));
    """)
    for m in r["at"]:
        dpr = m["dpr"]
        assert m["side"] == pytest.approx(24 * m["dot"])
        assert (m["dot"] * dpr) == pytest.approx(round(m["dot"] * dpr)), "칸이 물리 px의 정수배"
        # 축은 그림 안 같은 자리에 (물리 px로 맞추느라 반 물리 px까지 어긋난다)
        assert abs(m["pivotX"] - 48 * m["side"] / 96) * dpr <= 0.5 + 1e-9, dpr
        assert abs(m["pivotY"] - 72 * m["side"] / 96) * dpr <= 0.5 + 1e-9, dpr
    # 칸 3 논리 px: 그림 한 변 72 — 100%·200%는 칸이 정확히 3
    assert r["at"][0]["dot"] == 3 and r["at"][0]["side"] == 72 and r["at"][0]["pivotX"] == 36 and r["at"][4]["dot"] == 3
    # 물리는 칸 4 논리 px로 바닥을 잡는다 (96 px 몸) — 그림 칸이 더 작으면 그만큼 내려 닿게 한다
    assert r["scaled"]["drop"] == pytest.approx(r["tall"] * 1.1 - dropOf_flat(), abs=1e-6)
    assert r["same"]["drop"] == pytest.approx(r["tall"] - dropOf_flat(), abs=1e-6)
    assert r["flat"]["drop"] == pytest.approx(dropOf_flat() * 0.1, abs=1e-6)


# ---- 픽셀 표에 대한 빠른 맞춤 검사 (test_nightcap.py는 동글 전부를 돌려 무겁다): 실제 시트 위에서 모자·목도리 자리를 잰다
def _pixel_ids():
    return sorted(p.parent.name for p in (UI / "pets_pixel").glob("*/pet.json"))


def _frame(sheet, spec, f, n):
    return np.asarray(sheet.crop((f * n, spec["row"] * n, (f + 1) * n, (spec["row"] + 1) * n)))[..., 3] > 0


def _worst_fit(table_name):
    """표의 칸마다: 기준점 2칸 안에 몸이 있고, 입구 줄이 머리와 절반 이상 겹치고, 너비가 띠 안에 있다. 어긋난 칸 목록."""
    bad = []
    for pid in _pixel_ids():
        m = json.loads((UI / "pets_pixel" / pid / "pet.json").read_text(encoding="utf-8"))
        widths = w.CAP_WIDTHS if table_name == "head" else w.SCARF_WIDTHS
        for stage, table in (m.get(table_name) or {}).items():
            n = m["frame_sizes"][stage]
            sheet = Image.open(UI / "pets_pixel" / pid / f"sheet_{stage}.png").convert("RGBA")
            for anim, cells in table.items():
                spec = m["animations"].get(anim)
                if spec is None:
                    bad.append((pid, stage, anim, "표에만 있는 동작"))
                    continue
                for f, (x, y, width, r) in enumerate(cells[: spec["frames"]]):
                    body = _frame(sheet, spec, f, n)
                    if not widths.start - 1 <= width <= widths.stop:
                        bad.append((pid, stage, anim, f, "width", width))
                    px, py = round(x), round(y)
                    near = body[max(py - 2, 0):py + 3, max(px - 2, 0):px + 3]
                    if not near.any():
                        bad.append((pid, stage, anim, f, "floating", x, y))
                        continue
                    if abs(r) < 45:  # 서 있는 머리: 입구 줄이 머리에 얹힌다
                        lo, hi = max(round(x - width / 2), 0), min(round(x + width / 2), n)
                        rows = body[max(py - 1, 0):min(py + 3, n), lo:hi]
                        if hi > lo and rows.any(axis=0).mean() < 0.5:
                            bad.append((pid, stage, anim, f, "off head", x, y, width))
    return bad


def test_every_species_has_a_pixel_sheet_now():
    assert len(_pixel_ids()) == 18  # v0.5.5: 곰·꿀벌·강아지 −3, 펭귄 +1, 레서판다 +1, 다람쥐 +1


def test_nightcap_table_fits_every_pixel_frame():
    bad = _worst_fit("head")
    assert not bad, f"{len(bad)}칸: {bad[:12]}"


def test_santa_scarf_table_fits_every_pixel_frame():
    assert (UI / "pets_pixel" / "santa" / "pet.json").is_file()
    bad = _worst_fit("neck")
    assert not bad, f"{len(bad)}칸: {bad[:12]}"


def test_pet_display_size_ignores_stage_size_everywhere():
    style = (ROOT / "crates" / "screenpet-core" / "src" / "style.rs").read_text(encoding="utf-8")
    assert "pub fn dot_logical(stage: Stage) -> u32" in style
    assert "stage_size" not in (UI / "house.js").read_text(encoding="utf-8")
    assert "manifest.size_mult" not in re.search(r"pub fn pixel_dot.*?\n    \}", (ROOT / "src-tauri" / "src" / "slots.rs").read_text(encoding="utf-8"), re.S).group(0)
