import math
import re
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_objects as d
from conftest import assert_folder_matches_shipped

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("objects")
    return out, d.build(out)


def test_names_match_the_core_kinds():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "object.rs").read_text(encoding="utf-8")
    assert tuple(re.findall(r'ObjectKind::\w+ => "([a-z_]+)",', rs)) == d.NAMES


def test_postit_spots_match_the_webview():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    table = dict((k, (int(x), int(y))) for k, x, y in re.findall(r"(\w+): \[(\d+), (\d+)\]", js))
    assert table == d.POSTIT_AT


@pytest.mark.parametrize("name", d.NAMES)
def test_sprite_fits_the_cell_and_sits_on_the_bottom(built, name):
    out, arts = built
    im = arts[name]
    px = d.art_px(name)
    assert im.size == (px, px) and im.mode == "RGBA"
    assert Image.open(out / f"{name}.png").size == (px, px)
    a = d.cell128(im).getchannel("A")
    for corner in ((0, 0), (127, 0)):
        assert a.getpixel(corner) == 0, corner
    left, top, right, bottom = a.getbbox()
    # 64~96 언저리 크기, 바닥에 앉는다
    assert 48 <= right - left <= 124, (left, right)
    assert 124 <= bottom <= 128, bottom
    assert top > 0


@pytest.mark.parametrize("name", sorted(d.POSTIT_AT))
def test_postit_is_stuck_on_the_object(built, name):
    _, arts = built
    x, y = d.POSTIT_AT[name]
    assert arts[name].getchannel("A").getpixel((x, y)) > 0, "post-it would float in the air"


def test_postit_is_yellow_not_an_envelope_or_heart(built):
    _, arts = built
    im = arts["postit"]
    assert im.size == (d.POSTIT, d.POSTIT)
    r, g, b, a = im.getpixel((d.POSTIT // 2 - 6, d.POSTIT // 2 + 8))
    assert a == 255 and r > 230 and g > 200 and b < 150


def test_shipped_objects_match_the_generator(built):
    """들어 있는 그림 == 생성기 출력 (이름 전부: 없는 파일도 여기서 걸린다)."""
    out, arts = built
    assert set(arts) == set(d.NAMES) | {"postit"}
    assert_folder_matches_shipped(out, ROOT / "ui" / "objects")


def test_note_text_reaches_the_page_only_as_text():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    assert "note.textContent = text" in js
    for risky in ("innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval("):
        assert risky not in js


def np_alpha(im):
    import numpy as np

    return np.asarray(im.getchannel("A"))


def core_shapes():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "tumble.rs").read_text(encoding="utf-8")
    table = {}
    for kind, px, py, reach, hull in re.findall(
        r"ObjectKind::(\w+) => Shape \{ pivot: \(([\d.]+), ([\d.]+)\), reach: ([\d.]+), hull: &\[(.*?)\] \}", rs
    ):
        pts = [(float(x), float(y)) for x, y in re.findall(r"\(([\d.]+), ([\d.]+)\)", hull)]
        table[re.sub(r"(?<!^)([A-Z])", r"_\1", kind).lower()] = {"pivot": (float(px), float(py)), "reach": float(reach), "hull": pts}
    return table


def test_core_shape_table_is_the_one_printed_from_the_art():
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "tumble.rs").read_text(encoding="utf-8")
    assert d.shape_table() in rs, "그림이 바뀌었다: python tools/draw_objects.py --shapes 로 다시 찍을 것"


@pytest.mark.parametrize("name", d.NAMES)
@pytest.mark.parametrize("deg", range(0, 360, 30))
def test_turning_sprite_touches_where_the_core_thinks_and_is_not_clipped(built, name, deg):
    """코어의 볼록 껍질이 말하는 가장 낮은 점 = 실제로 돌린 그림의 가장 낮은 점. 그림은 reach 반지름 안에 있다."""
    _, arts = built
    s = core_shapes()[name]
    (px, py), reach = s["pivot"], s["reach"]
    rad = math.radians(deg)
    lowest = max((x - px) * math.sin(rad) + (y - py) * math.cos(rad) for x, y in s["hull"])
    a = np_alpha(d.cell128(arts[name]))
    ys, xs = a.nonzero()
    xs, ys = xs + 0.5 - px, ys + 0.5 - py
    assert (xs ** 2 + ys ** 2).max() <= reach ** 2
    solid_y, solid_x = (a >= 128).nonzero()
    drawn = ((solid_x + 0.5 - px) * math.sin(rad) + (solid_y + 0.5 - py) * math.cos(rad)).max()
    assert abs(drawn - lowest) <= 3.0, (drawn, lowest)


@pytest.mark.parametrize("name", sorted(d.POSTIT_AT))
def test_postit_stays_inside_the_turning_room(name):
    s = core_shapes()[name]
    x, y = d.POSTIT_AT[name]
    assert math.hypot(x - s["pivot"][0], y - s["pivot"][1]) + d.POSTIT_REACH <= s["reach"]


def test_shirt_has_sleeves_wider_than_its_hem_and_a_dipped_neck(built):
    _, arts = built
    a = np.asarray(arts["shirt"].getchannel("A")) >= 128
    ys, xs = a.nonzero()
    h = ys.max() - ys.min() + 1
    widths = a.sum(axis=1)[ys.min(): ys.max() + 1]
    assert widths[: h // 2].max() > widths[-h // 4:].max() * 1.3, widths.tolist()
    tops = [np.flatnonzero(a[:, x])[0] if a[:, x].any() else 128 for x in range(128)]
    mid = (xs.min() + xs.max()) // 2
    assert tops[mid] > min(tops[mid - 20: mid]) + 3 and tops[mid] > min(tops[mid + 1: mid + 21]) + 3, tops[mid - 20: mid + 21]


def _opaque(im):
    a = np.asarray(im.convert("RGBA")).reshape(-1, 4)
    return a[a[:, 3] > 200][:, :3].astype(int)


def _share(px, test):
    return float(np.mean(test(px)))


def test_newspaper_is_grey_paper_with_ink(built):
    _, arts = built
    px = _opaque(arts["paper"])
    grey = np.abs(px.max(axis=1) - px.min(axis=1)) < 24
    assert _share(px, lambda p: grey & (p.mean(axis=1) > 180)) > 0.35, "mostly light grey sheet"
    assert _share(px, lambda p: grey & (p.mean(axis=1) < 130)) > 0.08, "ink: headline, photo, lines"
    assert _share(px, lambda p: p.mean(axis=1) > 240) < 0.02, "not the old white wad"


def test_stone_is_a_grey_brown_rock_not_white(built):
    _, arts = built
    px = _opaque(arts["stone"])
    assert _share(px, lambda p: p.mean(axis=1) > 200) < 0.01, "white"
    # 회갈색: 빨강 ≥ 초록 ≥ 파랑, 채도는 낮게
    r, g, b = px[:, 0].mean(), px[:, 1].mean(), px[:, 2].mean()
    assert r > g > b and r - b < 50, (r, g, b)
    # 아래쪽이 위쪽보다 어둡다 (그림자 면)
    arr = np.asarray(arts["stone"])
    lit = lambda rows: arr[rows][arr[rows][:, :, 3] > 200][:, :3].mean()
    assert lit(slice(66, 84)) > lit(slice(108, 122)) + 20


def test_objects_are_just_objects_no_faces():
    """던지기용 물건이다 — 눈·입·볼터치를 그리지 않는다. tiny_face·×× 눈이 돌아오면 여기서 걸린다."""
    src = (ROOT / "tools" / "draw_objects.py").read_text(encoding="utf-8")
    for face in ("tiny_face", "x_eyes", "eye", "mouth", "cheek"):
        assert face not in src, face


@pytest.mark.parametrize("name", d.NAMES)
def test_no_face_colours_in_the_art(built, name):
    """tiny_face의 눈(84,58,56)·볼터치(250,180,180) 색이 그림에 없다."""
    _, arts = built
    px = _opaque(arts[name])
    for colour in ((84, 58, 56), (250, 180, 180)):
        assert (np.abs(px - colour).sum(axis=1) < 6).sum() == 0, colour


def test_broken_computer_screen_is_black_glass_with_no_white_marks(built):
    _, arts = built
    screen = np.asarray(arts["computer"])[56:95, 31:97, :3].astype(int)
    assert screen.mean() < 70
    assert (screen.min(axis=2) > 200).sum() == 0, "x eyes or a mouth"


POOPS = ("poop", "pink", "rainbow", "pellets")


def test_only_trash_takes_post_its():
    assert set(d.POSTIT_AT) == set(d.NAMES) - set(POOPS) - set(d.GIANTS)


@pytest.mark.parametrize("name", ["poop", "pink", "rainbow"])
def test_poop_is_a_swirl_stacked_narrower_toward_a_tip(built, name):
    """둘둘 쌓인 똥: 바닥이 가장 넓고 위로 갈수록 좁아지며 꼭지로 끝난다."""
    _, arts = built
    a = np.asarray(arts[name].getchannel("A")) >= 128
    ys, xs = a.nonzero()
    w, h = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
    assert h >= w * 0.75, (w, h)
    widths = a.sum(axis=1)[ys.min(): ys.max() + 1]
    assert widths.argmax() >= h * 2 // 3, widths.tolist()
    assert widths[: h // 6].max() < w * 0.45, widths.tolist()


def _lumps(mask):
    """이어진 덩어리 수 (4방향)."""
    seen = np.zeros_like(mask)
    count = 0
    for start in zip(*mask.nonzero()):
        if seen[start]:
            continue
        count += 1
        stack = [start]
        while stack:
            y, x = stack.pop()
            if not (0 <= y < mask.shape[0] and 0 <= x < mask.shape[1]) or seen[y, x] or not mask[y, x]:
                continue
            seen[y, x] = True
            stack += [(y + 1, x), (y - 1, x), (y, x + 1), (y, x - 1)]
    return count


def test_rabbit_pellets_are_several_separate_little_balls(built):
    _, arts = built
    # 외곽선 안쪽의 갈색만 보면 알갱이 하나하나가 떨어져 있다
    px = np.asarray(arts["pellets"]).astype(int)
    fill = (px[..., 3] > 200) & (px[..., 0] > 95)
    assert _lumps(fill) >= 5


def test_poop_is_brown_and_the_rainbow_one_has_every_colour(built):
    _, arts = built
    big = _opaque(arts["poop"]).mean(axis=0)
    assert big[0] > big[1] > big[2], big
    px = _opaque(arts["rainbow"])
    hues = {"red": (220, 80, 80), "orange": (240, 150, 60), "yellow": (250, 214, 84), "green": (110, 190, 100), "blue": (100, 150, 230), "violet": (160, 120, 220)}
    for name, c in hues.items():
        near = (np.abs(px - np.array(c)).sum(axis=1) < 70).sum()
        assert near > 40, (name, near)
    area = lambda n: (np.asarray(arts[n].getchannel("A")) >= 128).sum()
    assert abs(area("rainbow") - area("poop")) < area("poop") * 0.1, "same swirl, other colours"


def test_the_green_poop_is_gone():
    assert "dropping" not in d.NAMES
    assert not (ROOT / "ui" / "objects" / "dropping.png").exists()
    assert not (ROOT / "ui" / "objects_pixel" / "dropping.png").exists()


@pytest.mark.parametrize("name", ["poop", "pink", "rainbow"])
def test_poop_has_the_classic_outline_not_an_oval(built, name):
    """전형적인 똥: 단 사이 옆선이 안으로 꺾이고, 꼭지는 오른쪽으로 휜다."""
    _, arts = built
    a = np.asarray(arts[name].getchannel("A")) >= 128
    ys, xs = a.nonzero()
    w = xs.max() - xs.min() + 1
    # 타원은 왼쪽 옆선이 볼록해 한 번 꺾이면 안으로 돌아오지 않는다: 위에서 내려오며 왼쪽 끝의 이동폭이 다시 커지면 단이 나뉜 것
    lefts = [np.flatnonzero(row)[0] for row in a[ys.min(): ys.max() + 1]]
    steps = [lefts[i] - lefts[i + 2] for i in range(len(lefts) - 2)]
    drop = int(np.argmin(steps[len(steps) // 4: len(steps) * 3 // 4])) + len(steps) // 4
    assert max(steps[drop:]) > steps[drop], steps
    tip = np.flatnonzero(a[ys.min()]).mean()
    assert tip > (xs.min() + xs.max()) / 2 + w * 0.05, (tip, xs.min(), xs.max())
    # 바닥이 캔버스에 납작하게 잘리지 않는다
    assert a[-1].sum() < w * 0.3, a[-1].sum()


def test_umbrella_lies_flipped_with_the_handle_up(built):
    """뒤집힌 우산: 손잡이가 위로, 천이 아래로 간다."""
    _, arts = built
    px = np.asarray(arts["umbrella"].convert("RGBA")).astype(int)
    def rows(colour):
        return ((np.abs(px[..., :3] - colour).sum(axis=2) < 30) & (px[..., 3] > 200)).nonzero()[0]
    handle, cloth = rows(d.HANDLE), rows(d.CLOTH)
    assert handle.mean() < cloth.mean() - 15, (handle.mean(), cloth.mean())
    a = px[..., 3] >= 128
    ys = a.any(axis=1).nonzero()[0]
    widths = a.sum(axis=1)
    assert widths.argmax() > (ys.min() + ys.max()) / 2, widths.tolist()


def test_shirt_is_tattered_not_a_neat_tee(built):
    """누더기: 밑단이 들쭉날쭉하고, 양쪽 소매가 다르게 뜯겼다."""
    _, arts = built
    a = np.asarray(arts["shirt"].getchannel("A")) >= 128
    xs = a.any(axis=0).nonzero()[0]
    bottoms = np.array([np.flatnonzero(a[:, x])[-1] for x in xs])
    hem = bottoms[len(bottoms) // 4: len(bottoms) * 3 // 4]
    turns = int(((hem[1:-1] > hem[:-2]) & (hem[1:-1] >= hem[2:])).sum())
    assert turns >= 3 and hem.max() - hem.min() >= 7, hem.tolist()
    ys = a.any(axis=1).nonzero()[0]
    mid = (xs.min() + xs.max()) // 2
    left, right = a[ys.min():, :mid + 1], a[ys.min():, mid + 1:][:, ::-1]
    n = min(left.shape[1], right.shape[1])
    assert (left[:, -n:] != right[:, -n:]).sum() > a.sum() * 0.05, "the two sides should tear differently"


def test_pink_poop_is_pink_and_the_same_swirl(built):
    _, arts = built
    r, g, b = _opaque(arts["pink"]).mean(axis=0)
    assert r > g + 40 and b > g, (r, g, b)
    area = lambda n: (np.asarray(arts[n].getchannel("A")) >= 128).sum()
    assert abs(area("pink") - area("poop")) < area("poop") * 0.05


# ---- 🪨💩 거대한 돌·똥: 화면 높이의 80%까지 커지므로 1024로 그린다 ----


def test_giants_are_named_like_the_core():
    assert d.GIANTS == {"giant_stone": "stone", "giant_poop": "poop"}
    assert all(name in d.NAMES for name in d.GIANTS)
    rs = (ROOT / "crates" / "screenpet-core" / "src" / "object.rs").read_text(encoding="utf-8")
    assert '"🪨 거대한 돌"' in rs and '"💩 거대한 똥"' in rs


def test_the_baby_cat_size_in_the_core_is_the_cat_pet_json():
    import json

    rs = (ROOT / "crates" / "screenpet-core" / "src" / "object.rs").read_text(encoding="utf-8")
    core = float(re.search(r"pub const BABY_CAT_STAGE_SIZE: f64 = ([\d.]+);", rs).group(1))
    pet = json.loads((ROOT / "ui" / "pets" / "cat" / "pet.json").read_text(encoding="utf-8"))
    assert core == pet["stage_size"]["baby"]


def _partial(im):
    a = np.asarray(im.getchannel("A")).astype(int)
    return int(((a > 10) & (a < 245)).sum()), int((a >= 245).sum())


@pytest.mark.parametrize("name", sorted(d.GIANTS))
def test_giants_are_drawn_sharp_at_1024_not_upscaled(built, name):
    """1024 칸에 직접 그렸다: 가장자리 번짐이 128 그림을 키운 것보다 훨씬 얇다."""
    _, arts = built
    big = arts[name]
    assert big.size == (1024, 1024)
    blurry = arts[d.GIANTS[name]].resize((1024, 1024), Image.LANCZOS)
    (edge, solid), (edge_up, solid_up) = _partial(big), _partial(blurry)
    # 넓이로 맞춘 가장자리 띠 두께
    assert edge / solid ** 0.5 < 0.35 * edge_up / solid_up ** 0.5, (edge, solid, edge_up, solid_up)


@pytest.mark.parametrize("name", sorted(d.GIANTS))
def test_giants_look_like_their_small_selves_only_bigger_in_the_cell(built, name):
    _, arts = built
    small, big = arts[d.GIANTS[name]], d.cell128(arts[name])
    sb, bb = small.getchannel("A").getbbox(), big.getchannel("A").getbbox()
    grow = (bb[2] - bb[0]) / (sb[2] - sb[0])
    assert 1.12 <= grow <= 1.3, (sb, bb)
    assert bb[3] >= 124 and bb[1] > 0, bb
    want, got = _opaque(small).mean(axis=0), _opaque(arts[name]).mean(axis=0)
    assert np.abs(want - got).max() < 25, (want, got)


def test_giant_poop_is_a_brown_swirl(built):
    _, arts = built
    r, g, b = _opaque(arts["giant_poop"]).mean(axis=0)
    assert r > g > b, (r, g, b)
    a = np.asarray(d.cell128(arts["giant_poop"]).getchannel("A")) >= 128
    ys, xs = a.nonzero()
    widths = a.sum(axis=1)[ys.min(): ys.max() + 1]
    h = ys.max() - ys.min() + 1
    assert widths.argmax() >= h * 2 // 3 and widths[: h // 6].max() < (xs.max() - xs.min() + 1) * 0.45


def test_the_webview_draws_giants_from_the_big_art_and_knows_their_pixel_dots():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    dots = dict((k, int(v)) for k, v in re.findall(r"(giant_\w+): (\d+)", js))
    assert dots == {name: 20 for name in d.GIANTS}
    for name in d.GIANTS:
        assert Image.open(ROOT / "ui" / "objects_pixel" / f"{name}.png").size == (dots[name], dots[name])


def test_giant_windows_start_click_through_and_are_capped():
    """거대한 돌·똥 깜빡임·렉 (v0.6.3): 창을 들일 때 클릭 통과를 미리 켜고 (화면 밖에서), 화면엔 MAX_GIANTS개까지"""
    src = (Path(__file__).resolve().parent.parent / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")
    admit = src[src.index("fn admit(&mut self"):]
    admit = admit[: admit.index("\n    fn ", 10)]
    assert "giant_to_drop(" in admit
    assert "set_ignore_cursor_events(true)" in admit
    assert "click_through," in admit and "click_through: false" not in admit
