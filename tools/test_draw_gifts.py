import re
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_gifts
from conftest import assert_folder_matches_shipped

ROOT = Path(__file__).resolve().parent.parent
CATALOG = (ROOT / "crates" / "screenpet-core" / "src" / "gift.rs").read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def built(tmp_path_factory):
    out = tmp_path_factory.mktemp("gifts")
    return out, draw_gifts.build(out)


def test_shipped_gifts_match_the_generator(built):
    """들어 있는 그림 == 생성기 출력 (한쪽만 고치면 다음 빌드에 달라진다)."""
    assert_folder_matches_shipped(built[0], ROOT / "ui" / "gifts")


def test_ids_match_the_core_catalog():
    ids = re.findall(r'item\("([a-z0-9_]+)", "', CATALOG)
    assert ids == draw_gifts.IDS


def test_every_list_has_six_items():
    assert len(draw_gifts.LINES) == 15  # 곰·꿀벌·강아지는 v0.5.5에서 빠졌다, 펭귄·레서판다·다람쥐가 왔다
    assert all(len(names) == 6 for names in draw_gifts.LINES.values())


@pytest.mark.parametrize("name", draw_gifts.IDS)
def test_generated_files(built, name):
    out, arts = built
    for suffix in ("", draw_gifts.SHINY):
        for d in (out, ROOT / "ui" / "gifts"):
            im = Image.open(d / f"{name}{suffix}.png")
            assert im.mode == "RGBA" and im.size == (draw_gifts.ICON, draw_gifts.ICON)
        assert arts[name + suffix].size == (draw_gifts.ICON, draw_gifts.ICON)


@pytest.mark.parametrize("name", draw_gifts.IDS)
def test_art_sits_on_the_bottom_edge_and_reads_at_small_size(built, name):
    _, arts = built
    alpha = arts[name].getchannel("A")
    left, top, right, bottom = alpha.getbbox()
    assert bottom == draw_gifts.ICON, "sits on the surface"
    assert right - left >= 20 and bottom - top >= 16, (left, top, right, bottom)
    assert abs((left + right) / 2 - draw_gifts.ICON / 2) <= 1, "centered"
    opaque = sum(1 for v in alpha.tobytes() if v > 200)
    assert opaque >= 150, "enough body for a silhouette in the book"


@pytest.mark.parametrize("name", draw_gifts.IDS)
def test_shiny_is_gold_with_sparkles(built, name):
    _, arts = built
    plain, gold = arts[name], arts[name + draw_gifts.SHINY]
    assert plain.tobytes() != gold.tobytes()
    # 별은 보통 그림의 바깥까지 번진다
    a0, a1 = plain.getchannel("A"), gold.getchannel("A")
    assert sum(a1.tobytes()) > sum(a0.tobytes())
    # 금빛: 빨강이 파랑보다 뚜렷이 많다 (불투명한 칸 평균)
    b = gold.convert("RGBA").tobytes()
    px = [b[k:k + 4] for k in range(0, len(b), 4) if b[k + 3] > 200]
    assert sum(p[0] - p[2] for p in px) / len(px) > 20
    # 별만 찍은 게 아니라 몸 전체가 물든다 (원래 금빛인 선물도)
    was, now = np.asarray(plain.convert("RGBA")).astype(int), np.asarray(gold.convert("RGBA")).astype(int)
    body = was[..., 3] > 200
    assert (np.abs(was - now).sum(axis=2) > 8)[body].mean() > 0.8


def test_silhouettes_differ_within_each_list(built):
    """도감의 실루엣으로 서로 구별된다."""
    _, arts = built
    for key, names in draw_gifts.LINES.items():
        masks = [arts[f"{key}_{n}"].getchannel("A").point(lambda v: 255 if v > 128 else 0).tobytes() for n in names]
        assert len(set(masks)) == len(masks), key
