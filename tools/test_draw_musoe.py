"""무쇠인간 시트 검사: 줄 순서 (필수 21 + climb·slide·lookup + 🚀 날기 넷 + 🪖 suit_up)·모든 칸 여백·바닥선 (나는 칸은 바닥과 틈)·
pet.json(super_flier, 🔋 배터리, 짧은 영어 대사)·갑옷 꼬마 영웅 (빨간 갑옷·금색 판, 둥근 빨간 투구에 금테 하늘색 렌즈 눈, 가슴 하트 빛,
앞모습 두 다리, 옆모습은 오른쪽)·제트팩 날기 (가로로 누운 몸, 투구는 앞, 하얀 빛 꼬리는 등 뒤 분사구에서 뒤로 — 발바닥 불꽃 없음)·
투구 장착 (맨얼굴 → 반쪽 투구 둘이 양옆에서 → 렌즈 켜짐)·뒷모습은 머리가 팔 위·등에 제트팩·저장소 그림과 같음."""
import json
from pathlib import Path

import numpy as np
import pytest

import draw_pets
import draw_musoe as dm
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_folder_matches_shipped, assert_frames_inside_cell, assert_roll_fits_circle, cell
from sprite_spec import ANIMS, OPTIONAL_ANIMS, STAGES, sheet_128

ROOT = Path(__file__).resolve().parent.parent
SHIPPED = ROOT / "ui" / "pets" / "musoe"
SOAR = ["fly", "fly_fast", "fly_flip", "hover"]  # = Rust manifest::SOAR_ANIMS (🦸 슈퍼인간과 같은 날기 뇌)
STANDING = ("walk", "idle", "look", "wave", "tilt", "heart", "tickle", "hungry", "eat", "level_up", "annoyed", "stretch", "suit_up",
            "hold", "carry", "present", "hug")
FLYING = ("fly", "fly_fast", "fly_flip")
JUGGLE = ["lift", "hold", "carry", "throw", "strain", "present", "slump"]  # = Rust manifest::JUGGLE_ANIMS (💪 영웅 들기)


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """무쇠인간만 임시 폴더에 그림 (한 번)."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"musoe": dm.MUSOE})
        draw_pets.build(root)
    return root


def frame(out, st, anim, i):
    """그 단계·동작·칸 (128 칸)."""
    return cell(sheet_128(out / "musoe" / f"sheet_{st}.png"), names(dm.MUSOE).index(anim), i)


def near(im, col, tol=40):
    """이 색에 가까운 불투명 픽셀 마스크."""
    a = np.asarray(im.convert("RGBA")).astype(int)
    return (np.abs(a[..., :3] - np.array(col)).sum(axis=2) < tol) & (a[..., 3] > 200)


def test_registered_with_climb_flight_rows_and_suit_up_at_the_end():
    assert draw_pets.SPECIES["musoe"] is dm.MUSOE
    assert [o[0] for o in OPTIONAL_ANIMS[-1:]] == ["suit_up"] == dm.SUIT_ANIMS  # 맨 끝에 이어 붙인다
    assert dm.SOAR_ANIMS == SOAR
    assert dm.JUGGLE_ANIMS == JUGGLE
    # 💪 들기 · 🫣 숨바꼭질 (흐물이 장난) · 🤗 거미인간 포옹 (hug — 해그리드 줄 자리)도 OPTIONAL_ANIMS 순서대로
    assert names(dm.MUSOE) == [a for a, *_ in ANIMS] + ["climb", "slide", "lookup"] + JUGGLE + ["peek"] + SOAR + ["hug", "suit_up"]
    assert OPTIONAL_ANIMS[-1][1:] == (4, 4, False, True)  # 4칸 한 번 (마지막 칸 = 렌즈 켜짐 유지), 앞모습


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "musoe", dm.MUSOE, STAGES, grounded=STANDING, hover=FLYING)
    assert_roll_fits_circle(out / "musoe", dm.MUSOE, STAGES, slack=2)
    for st in STAGES:
        assert_eat_has_no_bowl(out / "musoe" / f"sheet_{st}.png", dm.MUSOE)


def test_manifest_is_a_jetpack_flier_that_loves_batteries(out):
    m = json.loads((out / "musoe" / "pet.json").read_text(encoding="utf-8"))
    assert m["name"] == "무쇠인간"
    assert m["favorite_food"] == {"emoji": "🔋", "name": "배터리"}
    assert m["personality"] == "playful" and m["super_flier"] is True
    assert m["juggler"] is True and m["pranks"] == ["hoist", "hide"]
    assert set(m["grip"]["teenager"]) == {"lift", "hold", "carry", "throw", "present"}
    assert m["animations"]["hug"]["frames"] == 2 and m["animations"]["peek"]["frames"] == 4
    assert "web_slinger" not in m and "bike_rider" not in m and "tint" not in m and "locomotion" not in m
    assert m["animations"]["fly"]["frames"] == 3 and m["animations"]["hover"]["frames"] == 3
    assert m["animations"]["suit_up"]["frames"] == 4 and m["animations"]["suit_up"]["loop"] is False
    assert {"You got this, kid!", "I believe in you!", "Proud of you!"} <= set(m["speech"]["cheer"])
    assert "Uh-huh." in m["speech"]["uh_huh"] and "Not now, kid." in m["speech"]["not_now"]
    for key in ("take_off", "rescue", "air_flip", "soft_land", "suit_up", "chatter", "tickle", "grumpy", "hero_lift", "cheer", "uh_huh", "not_now"):
        assert m["speech"][key] and all(len(s) <= 20 for s in m["speech"][key]), key
    assert {"Beep boop!", "Charging...", "Heart power!"} <= set(m["speech"]["chatter"])
    assert "Suit up!" in m["speech"]["suit_up"]
    assert m["stage_size"] == dict.fromkeys(STAGES, 0.85)


@pytest.mark.parametrize("st", STAGES)
def test_red_armor_gold_plates_round_helmet_lens_eyes_and_a_glowing_heart(out, st):
    im = frame(out, st, "sit_front", 0)
    assert near(im, dm.RED, 40).sum() > 400, st  # 빨간 갑옷 + 투구
    assert near(im, dm.GOLD, 40).sum() > 120, st  # 금색 판 (어깨·무릎·허리·투구 줄)
    assert near(im, dm.LENS, 30)[:96].sum() > 12, st  # 하늘색 렌즈 눈 (앉은 투구)
    assert near(im, dm.SKIN, 12).sum() < 5, st  # 투구를 쓴 얼굴은 안 보임
    heart = near(im, (255, 255, 255), 20)[64:110, 44:84]
    assert heart.sum() > 8, st  # 가슴의 하얀 하트 빛
    for anim in ("walk", "wave", "climb"):
        g = frame(out, st, anim, 0)
        assert near(g, dm.GOLD, 40).sum() > 40, (st, anim)


@pytest.mark.parametrize("st", STAGES)
def test_side_views_face_right(out, st):
    """옆모습은 오른쪽을 본다: 렌즈 눈이 몸 가운데보다 오른쪽, 제트팩은 왼쪽 (등)."""
    for anim in ("walk", "run", "hover", "fly"):
        im = frame(out, st, anim, 0)
        a = np.asarray(im.getchannel("A")) > 128
        body_x = np.nonzero(a)[1].mean()
        lens_x = np.nonzero(near(im, dm.LENS, 30))[1].mean()
        pack_x = np.nonzero(near(im, dm.PACK, 30))[1].mean()
        assert lens_x > body_x + 2 and pack_x < lens_x - 10, (st, anim, lens_x, body_x, pack_x)


@pytest.mark.parametrize("st", STAGES)
def test_a_person_stands_on_two_legs_in_front_view(out, st):
    for anim in ("wave", "level_up"):
        a = np.asarray(frame(out, st, anim, 0).getchannel("A")) > 128
        runs = [int(np.sum(a[y][1:] & ~a[y][:-1])) for y in range(118, 125)]
        assert max(runs) == 2, (st, anim, runs)


@pytest.mark.parametrize("st", STAGES)
def test_jetpack_flight_lies_flat_with_a_long_white_trail_from_the_back(out, st):
    for anim in ("fly", "fly_fast"):
        for i in range(3):
            im = frame(out, st, anim, i)
            a = np.asarray(im.getchannel("A")) > 128
            ys, xs = np.nonzero(a)
            assert xs.max() - xs.min() > 1.3 * (ys.max() - ys.min()), (st, anim, i)  # 가로로 누운 몸 + 빛 꼬리
            assert ys.max() <= 116, (st, anim, i)  # 바닥에서 떠 있음
            lens = np.nonzero(near(im, dm.LENS, 30))
            trail = np.nonzero(near(im, dm.TRAIL, 12))
            assert len(trail[1]) > 40, (st, anim, i)
            assert trail[1].min() < 16 and trail[1].mean() < lens[1].mean() - 30, (st, anim, i)  # 긴 꼬리가 뒤(왼쪽) 끝까지
            pack_y = np.nonzero(near(im, dm.PACK, 30))[0].mean()
            boot_y = np.nonzero(near(im, dm.BOOT, 12))[0].mean()
            assert pack_y < boot_y, (st, anim, i)  # 제트팩은 등 (누운 몸 위쪽)
        frames = [np.asarray(frame(out, st, anim, i).convert("RGBA")) for i in range(3)]
        assert not np.array_equal(frames[0], frames[1]) and not np.array_equal(frames[1], frames[2]), "빛 꼬리가 일렁"


@pytest.mark.parametrize("st", STAGES)
def test_no_light_from_palms_or_soles(out, st):
    """손바닥 빛·발바닥 분사 없음: 빛은 등의 분사구에서만 — 서 있는 칸 발 아래와 손 둘레엔 하얀 빛이 없다."""
    for anim in ("walk", "idle", "wave", "level_up", "hover"):
        im = frame(out, st, anim, 0)
        assert near(im, dm.TRAIL, 12)[118:].sum() == 0, (st, anim)
    im = frame(out, st, "hover", 0)
    flame = np.nonzero(near(im, dm.TRAIL, 12))
    boots = np.nonzero(near(im, dm.BOOT, 12))
    assert len(flame[1]) > 6 and flame[1].mean() < np.median(boots[1]) - 6, st  # 둥실: 불꽃은 등 뒤 (장화보다 왼쪽)


@pytest.mark.parametrize("st", STAGES)
def test_the_flip_turns_round_and_hover_floats_upright(out, st):
    tops = []
    for i in range(4):
        im = frame(out, st, "fly_flip", i)
        lens_y = np.nonzero(near(im, dm.LENS, 30))[0].mean() if near(im, dm.LENS, 30).any() else 64
        boot_y = np.nonzero(near(im, dm.BOOT, 12))[0].mean() if near(im, dm.BOOT, 12).any() else 64
        tops.append(lens_y < boot_y)
    assert tops[0] and not tops[2], (st, tops)  # 반 바퀴 뒤엔 머리가 아래
    for i in range(3):
        im = frame(out, st, "hover", i)
        lens_y = np.nonzero(near(im, dm.LENS, 30))[0].mean()
        boot_y = np.nonzero(near(im, dm.BOOT, 12))[0].mean()
        assert lens_y < boot_y - 30, (st, i)  # 똑바로 섬
        assert np.nonzero(np.asarray(im.getchannel("A")) > 128)[0].max() < 125, (st, i)  # 바닥에서 살짝 뜸


@pytest.mark.parametrize("st", STAGES)
def test_suit_up_bare_face_then_two_shells_close_then_lenses_light(out, st):
    f = [frame(out, st, "suit_up", i) for i in range(4)]
    assert near(f[0], dm.SKIN, 30).sum() > 150 and near(f[0], dm.HAIR, 30).sum() > 40, st  # 맨얼굴 + 갈색 머리
    assert near(f[0], dm.LENS, 30).sum() == 0, st
    # 반쪽 투구 둘이 양옆에서 다가옴: 첫 칸엔 얼굴 양옆에 빨간 껍데기가 따로
    shells = near(f[0], dm.RED, 40)[:60]
    xs = np.nonzero(shells)[1]
    assert (xs < 40).sum() > 20 and (xs > 88).sum() > 20, st
    assert near(f[1], dm.SKIN, 30).sum() < near(f[0], dm.SKIN, 30).sum(), st  # 닫히는 중
    assert near(f[2], dm.SKIN, 12).sum() < 5 and near(f[2], dm.LENS, 30).sum() == 0, st  # 닫힘, 렌즈는 아직 꺼짐
    assert near(f[3], dm.LENS, 30).sum() > 12, st  # 렌즈 켜짐


@pytest.mark.parametrize("st", STAGES)
def test_heart_flash_charge_blink_and_arm_block(out, st):
    def white(im):
        return int(near(im, (255, 255, 255), 20)[50:115, 30:98].sum())
    flash = [frame(out, st, "stretch", i) for i in range(3)]
    assert white(flash[1]) > white(flash[2]) + 20, st  # 💗 하트 번쩍 (빛줄기)
    charge = [white(frame(out, st, "sit_front", i)) for i in range(4)]
    assert max(charge) > min(charge) + 8, (st, charge)  # 🔋 충전 중 하트 깜빡
    for anim, n in (("tickle", 3), ("annoyed", 2)):
        for i in range(n):
            im = frame(out, st, anim, i)
            fists_y = np.nonzero(near(im, dm.FIST, 30))[0]
            lens_y = np.nonzero(near(im, dm.LENS, 30))[0]
            assert len(fists_y) > 10 and fists_y.min() < lens_y.max() + 14, (st, anim, i)  # 🛡️ 팔뚝을 엇갈려 투구 앞을 가림


def test_the_build_reproduces_the_committed_musoe(out):
    assert_folder_matches_shipped(out / "musoe", SHIPPED)


def test_back_view_head_covers_the_arms_and_the_jetpack_shows(monkeypatch):
    """뒷모습 (벽 타기·미끄러지기): 머리가 팔 위에 그려짐 — 팔이 머리를 덮으면 안 됨 (거듭된 실수). 제트팩은 등에 보임."""
    seen = []
    real = dm.dp.compose
    monkeypatch.setattr(dm.dp, "compose", lambda parts, *a, **k: (seen.append(list(parts)), real(parts, *a, **k))[1])
    for r in ((1.0, 1.0), (1.0, 0.2), (0.2, 1.0)):
        dm.musoe_back(r)
        parts = seen.pop()
        assert parts[-1].base == dm.RED and parts[-1].marks, "투구가 맨 나중 (팔보다 위)"
        bases = [p.base for p in parts]
        assert dm.PACK in bases and bases.index(dm.PACK) < len(parts) - 1
    hip = (64.0, 92.0)
    dm.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=True)
    forced = seen.pop()
    dm.hero(view_="back", hip=hip, arms=[(50.0, 20.0), (78.0, 20.0)], arms_front=False)
    behind = seen.pop()
    order = lambda parts: [(p.base, p.outline, len(p.shapes)) for p in parts]  # noqa: E731
    assert order(forced) == order(behind), "뒷모습에선 arms_front를 무시하고 팔을 머리 뒤에 (머리를 맨 나중에)"


@pytest.mark.parametrize("st", STAGES)
def test_climb_shows_the_jetpack_under_the_helmet(out, st):
    for anim, n in (("climb", 4), ("slide", 2)):
        for i in range(n):
            im = frame(out, st, anim, i)
            assert near(im, dm.PACK, 30).sum() > 35, (st, anim, i)
            assert near(im, dm.LENS, 12).sum() == 0, (st, anim, i)  # 뒤통수 — 렌즈 없음 (땀방울 하늘색은 빼고)
            pack_y = np.nonzero(near(im, dm.PACK, 30))[0].mean()
            helmet_y = np.nonzero(near(im, dm.RED, 40)[:70])[0].mean()
            assert helmet_y < pack_y, (st, anim, i)


def test_core_and_ui_know_musoe():
    food = (ROOT / "crates" / "screenpet-core" / "src" / "food.rs").read_text(encoding="utf-8")
    assert '"musoe" => "hand_battery"' in food
    main = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    extra = main[main.index("const EXTRA_SPEECH = {"):]
    assert "suit_up:" in extra[: extra.index("};")]
    lines = main[main.index("const CURSOR_LINES = {"):]
    assert "musoe:" in lines[: lines.index("};")]


@pytest.mark.parametrize("st", ["baby", "teenager", "adult"])
def test_lifts_overhead_with_one_fist_and_the_object_sits_on_the_helmet(out, st):
    """💪 들기 (🦸 슈퍼인간처럼 한 손 번쩍): 든 물건 바닥 (grip)은 투구 위, 렌즈는 그대로 보임 — 팔이 투구 앞을 가로지르지 않음."""
    m = json.loads((out / "musoe" / "pet.json").read_text(encoding="utf-8"))
    px = m.get("frame_sizes", {}).get(st, m["frame_size"])
    for anim in ("hold", "carry"):
        for k, (gx, gy) in enumerate(m["grip"][st][anim]):
            im = frame(out, st, anim, k)
            red = np.nonzero(near(im, dm.RED, 40))
            top = red[0].min()
            assert gy / px * 128 <= top + 8, (st, anim, k, gy, top)
            assert near(im, dm.LENS, 30).sum() > 8, (st, anim, k)


@pytest.mark.parametrize("st", STAGES)
def test_hug_wraps_both_arms_forward_with_hearts(out, st):
    """🤗 꼭 안기 (옆모습): 두 팔 (빨강·금색)이 몸 앞 오른쪽으로 쭉 — 친구 몸 자리까지, 하트가 뜸."""
    for i in range(2):
        im = frame(out, st, "hug", i)
        body = near(im, dm.RED, 40) | near(im, dm.RED_FAR, 40) | near(im, dm.FIST, 30)
        xs = np.nonzero(body.any(axis=0))[0]
        assert xs.max() >= 82, (st, i, xs.max())
        assert near(im, draw_pets.HEART, 40).sum() > 20, (st, i)


@pytest.mark.parametrize("st", ["baby", "teenager", "adult"])
def test_peek_shows_only_the_part_outside_the_window_side(out, st):
    """🫣 숨바꼭질: 창 옆면 (x = 64 − 0.15 × 128) 오른쪽은 비어 있고 (창 뒤), 왼쪽에 투구와 렌즈가 빼꼼."""
    edge = int(64 - 0.15 * 128)
    for i in range(4):
        im = np.asarray(frame(out, st, "peek", i).convert("RGBA"))
        assert im[:, edge + 1:, 3].max() == 0, (st, i)
        assert near(frame(out, st, "peek", i), dm.RED, 40).sum() > 40, (st, i)
        if i < 3:  # 마지막 칸은 두리번처럼 렌즈 깜빡 (꺼짐)
            assert near(frame(out, st, "peek", i), dm.LENS, 30).sum() > 5, (st, i)
