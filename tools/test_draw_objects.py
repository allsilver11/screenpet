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
    assert tuple(re.findall(r'ObjectKind::\w+ => "([a-z]+)",', rs)) == d.NAMES


def test_postit_spots_match_the_webview():
    js = (ROOT / "ui" / "object.js").read_text(encoding="utf-8")
    table = dict((k, (int(x), int(y))) for k, x, y in re.findall(r"(\w+): \[(\d+), (\d+)\]", js))
    assert table == d.POSTIT_AT


@pytest.mark.parametrize("name", d.NAMES)
def test_sprite_fits_the_cell_and_sits_on_the_bottom(built, name):
    out, arts = built
    im = arts[name]
    assert im.size == (128, 128) and im.mode == "RGBA"
    assert Image.open(out / f"{name}.png").size == (128, 128)
    a = im.getchannel("A")
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
        table[kind.lower()] = {"pivot": (float(px), float(py)), "reach": float(reach), "hull": pts}
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
    a = np_alpha(arts[name])
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


def test_sock_is_a_flat_limp_heap_not_an_upright_l(built):
    _, arts = built
    left, top, right, bottom = arts["sock"].getchannel("A").getbbox()
    assert right - left > (bottom - top) * 2.5, (left, top, right, bottom)


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


POOPS = ("poop", "dropping", "pellets")


def test_only_trash_takes_post_its():
    assert set(d.POSTIT_AT) == set(d.NAMES) - set(POOPS)


@pytest.mark.parametrize("name,min_ratio", [("poop", 2.2), ("dropping", 2.2)])
def test_poop_is_a_long_lying_oval_not_a_tall_swirl(built, name, min_ratio):
    """똬리(위로 뾰족한 더미)가 아니다: 납작하게 누운 타원이고 가운데 줄이 가장 넓다."""
    _, arts = built
    a = np.asarray(arts[name].getchannel("A")) >= 128
    ys, xs = a.nonzero()
    w, h = xs.max() - xs.min() + 1, ys.max() - ys.min() + 1
    assert w >= h * min_ratio, (w, h)
    widths = a.sum(axis=1)[ys.min(): ys.max() + 1]
    assert widths.argmax() in range(h // 4, h - h // 4 + 1), widths.tolist()
    top = a[ys.min() + h // 4].sum()
    assert top >= w * 0.6, "the top is round and wide, not a peak"


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


def test_poop_is_brown_and_the_small_kind_is_smaller_and_olive(built):
    _, arts = built
    big, small = _opaque(arts["poop"]).mean(axis=0), _opaque(arts["dropping"]).mean(axis=0)
    assert big[0] > big[1] > big[2], big
    assert small[1] > big[1] * 0.9 and small[0] - small[1] < big[0] - big[1], (small, big)
    area = lambda n: (np.asarray(arts[n].getchannel("A")) >= 128).sum()
    assert area("dropping") < area("poop") * 0.7
