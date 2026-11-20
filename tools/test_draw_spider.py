"""거미인간 시트 검사: 줄 순서 (필수 21 + climb·slide + 특기 다섯)·모든 칸 여백·바닥선·pet.json(web_slinger, 파리, grip)·
사람 모양 꼬마 영웅 (빨강·파랑 쫄쫄이, 가슴의 까만 거미 무늬, 까만 테 하얀 렌즈, 앞모습 두 다리, 옆모습은 오른쪽)·
줄 끝 손 자리 = 빨간 장갑·영웅 착지 (장갑으로 바닥 짚고 다리 쭉)·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_spider as ds
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "spider"
WEB = ["shoot", "web_swing", "hero_land", "web_pull", "web_whirl"]  # = Rust manifest::WEB_ANIMS
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed", "shoot",
            "hero_land", "web_pull", "web_whirl")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """거미인간만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"spider": ds.SPIDER})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "spider" / f"sheet_{st}.png"), names(ds.SPIDER).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_and_the_five_web_rows_at_the_end():
    assert draw_pets.SPECIES["spider"] is ds.SPIDER
    assert [o[0] for o in OPTIONAL_ANIMS[-5:]] == WEB == ds.WEB_ANIMS
    assert names(ds.SPIDER) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + WEB


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "spider", ds.SPIDER, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "spider", ds.SPIDER, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "spider" / f"sheet_{st}.png", ds.SPIDER)


def test_manifest_is_a_playful_web_slinger_that_loves_flies(out):
    m = json.loads((out / "spider" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "거미인간"
    assert m["favorite_food"] == {"emoji": "🪰", "name": "파리"}
    assert m["personality"] == "playful" and m["web_slinger"] is True
    assert "juggler" not in m and "tint" not in m
    assert m["animations"]["shoot"] == {"row": len(ANIMS) + 3, "frames": 3, "fps": 10, "loop": False}
    assert m["animations"]["hero_land"]["loop"] is False and "front" not in m["animations"]["web_swing"]
    for key in ("thwip", "web", "web_swing", "hero_land", "gotcha", "yeet"):
        assert m["speech"][key] and all(len(s) <= 16 for s in m["speech"][key]), key
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)


@pytest.mark.parametrize("st", STAGES)
def test_grip_marks_the_red_glove_at_the_end_of_the_line(out, st):
    """줄 끝 손 자리 = 빨간 장갑: 쏠 때는 몸 앞, 그네는 머리 위로 든 손, 끌어올 때는 몸 앞, 🌀 휘두를 때는 머리 위 (pet.json grip — 코어 GripTable)."""
    m = json.loads((out / "spider" / "pet.json").read_text(encoding="utf-8"))
    px = m.get("frame_sizes", {}).get(st, 128) / 128
    g = {a: [[x / px, y / px] for x, y in cells] for a, cells in m["grip"][st].items()}
    head = {a: [c[1] / px for c in m["head"][st][a]] for a in ("web_swing", "web_whirl")}
    head_x = [c[0] / px for c in m["head"][st]["shoot"]]
    assert set(g) == {"shoot", "web_swing", "web_pull", "web_whirl"}
    assert [len(g[a]) for a in ("shoot", "web_swing", "web_pull", "web_whirl")] == [3, 2, 4, 3]
    assert all(x > 68 and x > hx + 12 for (x, _), hx in zip(g["shoot"][1:], head_x[1:])), (g["shoot"], head_x)
    assert all(y < 34 and y < hy + 4 for (_, y), hy in zip(g["web_swing"], head["web_swing"])), (g["web_swing"], head)
    assert all(x > 70 for x, _ in g["web_pull"]), g["web_pull"]
    # 🌀 휘두르는 손: 머리 꼭대기보다 위, 손목으로 작은 원 (칸마다 자리가 바뀜)
    assert all(y < hy for (_, y), hy in zip(g["web_whirl"], head["web_whirl"])), (g["web_whirl"], head)
    assert len({(round(x), round(y)) for x, y in g["web_whirl"]}) == 3, g["web_whirl"]
    for anim, cells in g.items():
        for i, (x, y) in enumerate(cells):
            im = frame(out, st, anim, i)
            x0, y0 = int(round(x)), int(round(y))
            win = near(im, ds.RED)[max(0, y0 - 4):y0 + 5, max(0, x0 - 4):x0 + 5]
            assert win.sum() >= 4, (st, anim, i, (x, y), int(win.sum()))


@pytest.mark.parametrize("st", STAGES)
def test_red_and_blue_suit_with_a_black_spider_on_the_chest(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, ds.RED).sum() > 600 and near(im, ds.BLUE).sum() > 100, st
    chest = near(im, ds.EMBLEM, 30)[70:118, 48:80]
    assert chest.sum() > 20, (st, int(chest.sum()))
    side = frame(out, st, "walk", 0)
    assert near(side, ds.RED).sum() > 500 and near(side, ds.BLUE).sum() > 90, st


@pytest.mark.parametrize("st", STAGES)
def test_big_white_mask_lenses_outlined_in_black(out, st):
    """가면의 큰 하얀 눈 렌즈 둘 (까만 테): 앞모습 머리 높이에 하양이 넉넉하고, 왼쪽·오른쪽에 하나씩."""
    im = frame(out, st, "sit_front", 0)
    white = near(im, ds.LENS, 30)
    ys, xs = np.nonzero(white)
    assert len(xs) > 120, (st, len(xs))
    assert (xs < 64).sum() > 50 and (xs > 64).sum() > 50, st
    assert near(im, ds.LENS_RIM, 40).sum() > 60, st


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    """옆모습은 오른쪽을 본다: 렌즈(하양)가 몸 가운데보다 오른쪽."""
    for anim in ("walk", "run", "shoot", "web_whirl"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        body_x = np.nonzero(a)[1].mean()
        lens_x = np.nonzero(near(im, ds.LENS, 30))[1].mean()
        assert lens_x > body_x + 3, (st, anim, lens_x, body_x)


@pytest.mark.parametrize("st", STAGES)
def test_a_person_stands_on_two_legs_in_front_view(out, st):
    """앞모습: 바닥께에 발(장화)이 둘 — 거미 다리 여덟이 아니다."""
    for anim in ("sit_front", "wave"):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        runs = [int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(112, 125)]
        assert max(runs) == 2, (st, anim, runs)


@pytest.mark.parametrize("st", STAGES)
def test_hero_landing_plants_a_glove_on_the_floor_and_stretches_a_leg_back(out, st):
    im = frame(out, st, "hero_land", 1)
    a = np.asarray(im.getchannel("A")) > 128
    ys, xs = np.nonzero(a)
    assert xs.min() < 40, (st, xs.min())  # 뒤로 쭉 뻗은 다리
    glove = near(im, ds.RED)[108:125, 78:]
    assert glove.sum() > 12, (st, int(glove.sum()))  # 앞 바닥을 짚은 빨간 장갑
    assert ys.min() > 34, st  # 쪼그려 낮다


def test_the_build_reproduces_the_committed_spider(out):
    assert_folder_matches_shipped(out / "spider", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨 (예전 다른 펫들에서도 고쳤던 실수)."""
    seen = []
    real = ds.dp.compose
    monkeypatch.setattr(ds.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    ds.spider_back((1.0, 1.0))
    with_front = seen.pop()
    ds.hs()
    hip = (64.0, 92.0)
    ds.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=True)
    forced = seen.pop()
    ds.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=False)
    behind = seen.pop()
    order = lambda parts: [(p.base, p.outline, len(p.shapes)) for p in parts]  # noqa: E731
    assert order(forced) == order(behind), "뒷모습에선 arms_front를 무시하고 팔을 머리 뒤에 (머리를 맨 나중에)"
    # 벽 타기 그림도 머리가 마지막 (팔보다 위) — 맨 나중 조각이 머리 (같은 바탕색)
    assert order(with_front)[-1][0] == order(behind)[-1][0], order(with_front)
