"""박쥐인간 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 💪 들기 일곱 + 🏍️ 오토바이 다섯)·모든 칸 여백·바닥선·pet.json(bike_rider, juggler, 사과, grip)·
사람 모양 꼬마 영웅 (짙은 회색 슈트, 노란 벨트·가슴 로고, 하얀 눈의 검정 가면 + 드러난 살색 턱, 앞모습 두 다리, 옆모습은 오른쪽)·
오토바이 (큰 앞뒤 바퀴가 바닥에, 윌리는 뒷바퀴만)·뒷모습은 머리가 팔 위·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_bat as db
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "bat"
JUGGLE = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]  # = Rust manifest::JUGGLE_ANIMS
BIKE = ["ride", "wheelie", "bike_jump", "drift", "signal"]  # = Rust manifest::BIKE_ANIMS
STANDING = ("walk", "idle", "look", "sit_front", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "sleep", "annoyed",
            "lift", "hold", "carry", "throw", "strain", "present", "slump", "ride", "wheelie", "drift", "signal")


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """박쥐인간만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"bat": db.BAT_HERO})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "bat" / f"sheet_{st}.png"), names(db.BAT_HERO).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_juggle_and_the_five_bike_rows_at_the_end():
    assert draw_pets.SPECIES["bat"] is db.BAT_HERO
    assert [o[0] for o in OPTIONAL_ANIMS[-25:-20]] == BIKE == db.BIKE_ANIMS  # 그 뒤는 🦸 슈퍼인간 날기 넷 · 🧙 마법사 일곱 · 🧔 해그리드 여덟 · 🛡️ 무쇠인간 하나
    assert db.JUGGLE_ANIMS == JUGGLE
    assert names(db.BAT_HERO) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + JUGGLE + ["peek"] + BIKE  # 🫣 숨바꼭질 (흐물이 장난)


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "bat", db.BAT_HERO, STAGES, grounded=STANDING)
    assert_roll_fits_circle(out / "bat", db.BAT_HERO, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "bat" / f"sheet_{st}.png", db.BAT_HERO)


def test_manifest_is_a_playful_bike_rider_that_lifts_and_loves_apples(out):
    m = json.loads((out / "bat" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "박쥐인간"
    assert m["favorite_food"] == {"emoji": "🍎", "name": "사과"}
    assert m["personality"] == "playful" and m["bike_rider"] is True and m["juggler"] is True and m["pranks"] == ["hoist", "hide"]
    assert "web_slinger" not in m and "tint" not in m and "taskbar_play" not in m
    assert m["animations"]["signal"]["loop"] is False and m["animations"]["ride"]["frames"] == 3
    for key in ("vroom", "wheelie", "bike_trick", "bat_signal", "hero_lift", "chatter"):
        assert m["speech"][key] and all(len(s) <= 16 for s in m["speech"][key]), key
    assert "I'm the night" in m["speech"]["bat_signal"] and "to the bike!" in m["speech"]["vroom"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)
    for st in STAGES:
        assert set(m["grip"][st]) == {"lift", "hold", "carry", "throw", "present"}, st


@pytest.mark.parametrize("st", STAGES)
def test_dark_suit_yellow_belt_and_bat_logo_with_a_black_cowl(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, db.SUIT, 30).sum() > 80, st
    assert near(im, db.BELT, 40).sum() > 8, st
    assert near(im, db.LOGO_BG, 40)[60:118, 44:84].sum() > 5, st  # 가슴 로고 노란 타원
    assert near(im, db.BLACK, 20).sum() > 600, st  # 가면·망토·장갑·장화
    assert near(im, db.SKIN, 30).sum() > 120, st  # 드러난 입·턱
    whites = np.nonzero(near(im, db.EYE_W, 30))[1]
    assert (whites < 64).sum() > 15 and (whites > 64).sum() > 15, st  # 하얀 눈 둘


@pytest.mark.parametrize("st", STAGES)
def test_two_pointed_ears_stick_up_from_the_cowl(out, st):
    """앞모습 머리 꼭대기에서 위로 솟은 뾰족한 귀 둘 (가운데는 비어 있음)."""
    a = np.asarray(frame(out, st, "sit_front", 0).getchannel("A")) > 128
    ys = np.nonzero(a.any(axis=1))[0]
    top = ys.min()
    row = a[top + 1]
    runs = int(np.sum(row[1:] & ~row[:-1]))
    assert runs == 2, (st, runs)
    xs = np.nonzero(row)[0]
    assert xs.min() < 60 and xs.max() > 68 and not row[62:66].any(), st


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    """옆모습은 오른쪽을 본다: 드러난 살색 턱이 몸 가운데보다 오른쪽."""
    for anim in ("walk", "run", "ride", "carry", "signal"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        body_x = np.nonzero(a)[1].mean()
        skin_x = np.nonzero(near(im, db.SKIN, 30))[1].mean()
        assert skin_x > body_x + 2, (st, anim, skin_x, body_x)


@pytest.mark.parametrize("st", STAGES)
def test_a_person_stands_on_two_legs_in_front_view(out, st):
    for anim in ("sit_front", "wave"):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        runs = [int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(116, 125)]
        assert max(runs) == 2, (st, anim, runs)


@pytest.mark.parametrize("st", STAGES)
def test_the_bike_has_two_big_wheels_on_the_floor_and_the_wheelie_lifts_the_front(out, st):
    for i in range(3):
        tyre = near(frame(out, st, "ride", i), db.TYRE, 18)[100:126]
        xs = np.nonzero(tyre)[1]
        assert (xs < 50).sum() > 40 and (xs > 78).sum() > 40, (st, i)  # 뒷바퀴·앞바퀴
        a = np.asarray(frame(out, st, "ride", i).getchannel("A")) > 128
        assert a[:, :40].any() and a[:, 90:].any(), st  # 넓은 오토바이
    for i in range(3):
        tyre = near(frame(out, st, "wheelie", i), db.TYRE, 18)
        low = np.nonzero(tyre[108:126])[1]
        assert (low > 84).sum() < 5, (st, i)  # 앞바퀴는 바닥에서 떠 있음
        assert (low < 60).sum() > 20, (st, i)  # 뒷바퀴만 바닥에
    rot = [np.asarray(frame(out, st, "ride", i).convert("RGBA")) for i in range(3)]
    assert not np.array_equal(rot[0], rot[1]) and not np.array_equal(rot[1], rot[2]), "바퀴가 굴러감"


@pytest.mark.parametrize("st", STAGES)
def test_the_signal_lamp_lights_up(out, st):
    glow = (255, 246, 196)  # 켜진 렌즈 둘레의 빛 (draw_bat bt_signal)
    lit = near(frame(out, st, "signal", 2), glow, 40)[50:110, 76:124].sum()
    off = near(frame(out, st, "signal", 0), glow, 40)[50:110, 76:124].sum()
    assert lit > 60 and off < lit / 4, (st, lit, off)


def test_the_build_reproduces_the_committed_bat(out):
    assert_folder_matches_shipped(out / "bat", SHIPPED)


def test_back_view_head_covers_the_arms(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨 (예전 다른 펫들에서도 고쳤던 실수)."""
    seen = []
    real = db.dp.compose
    monkeypatch.setattr(db.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        db.bat_back(r)
        parts = seen.pop()
        assert parts[-1].base == db.SKIN, "머리가 맨 나중 (팔보다 위)"
        assert all(p.base != db.SKIN for p in parts[:-1])
    hip = (64.0, 92.0)
    db.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=True)
    forced = seen.pop()
    db.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=False)
    behind = seen.pop()
    order = lambda parts: [(p.base, p.outline, len(p.shapes)) for p in parts]  # noqa: E731
    assert order(forced) == order(behind), "뒷모습에선 arms_front를 무시하고 팔을 머리 뒤에 (머리를 맨 나중에)"
    assert forced[-1].base == db.SKIN
