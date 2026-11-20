"""오리 시트 검사: 모든 동작·투명 여백·pet.json·eat에 밥그릇 없음 (기린은 v0.7.0에서 빠짐)."""
import json
from pathlib import Path

import numpy as np
import pytest
from PIL import Image

import draw_farm
import draw_pets
from conftest import anim_names as names, assert_eat_has_no_bowl, assert_frames_inside_cell, assert_roll_fits_circle
from sprite_spec import ANIMS, NO_SPIN, STAGES

ROOT = Path(__file__).resolve().parent.parent


@pytest.fixture(scope="module")
def out(tmp_path_factory):
    """문서의 명령(python tools/draw_pets.py)이 부르는 build로 만든다."""
    root = tmp_path_factory.mktemp("pets")
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(draw_pets, "SPECIES", {"duck": draw_farm.DUCK})
        draw_pets.build(root)
    return root


def test_registered_with_its_own_poses():
    assert draw_pets.SPECIES["duck"] is draw_farm.DUCK and "giraffe" not in draw_pets.SPECIES
    assert names(draw_farm.DUCK) == [a for a, *_ in ANIMS] + ["lookup"]
    assert all(a in draw_farm.DUCK.poses for a in names(draw_farm.DUCK))


def test_every_frame_is_drawn_inside_the_cell(out):
    assert_frames_inside_cell(out / "duck", draw_pets.SPECIES["duck"], STAGES, free=("roll",))


def test_duck_roll_ball_fits_rotation_circle(out):
    # 지금 어른 오리는 43.4px까지 나온다 (반지름 38): 공룡과 같다 (test_draw_dino.py). 더 나빠지지 않게만 막는다
    assert_roll_fits_circle(out / "duck", draw_pets.SPECIES["duck"], STAGES, slack=6)


def test_manifest(out):
    duck = json.loads((out / "duck" / "pet.json").read_text(encoding="utf-8"))
    assert duck["name"] == "오리"
    assert duck["favorite_food"] == {"emoji": "🍞", "name": "빵"}
    assert duck["personality"] == "playful"
    assert duck["speech"]["chatter"] == ["quack!", "♪"]
    # 보통 집 · 걷는다 · 구른다
    assert "house" not in duck and "locomotion" not in duck
    assert "spin" in duck["animations"]["roll"] and "duck" not in NO_SPIN
    assert "neckstretch" not in duck["animations"] and "neck_stretch" not in duck


def test_the_build_reproduces_the_committed_duck(out):
    """draw_pets 빌드 결과가 저장소의 오리와 바이트까지 같다 (기린을 빼도 오리 그림은 그대로)."""
    shipped = ROOT / "ui" / "pets" / "duck"
    files = sorted(p.name for p in shipped.iterdir())
    assert files == sorted(p.name for p in (out / "duck").iterdir())
    for name in files:
        assert (out / "duck" / name).read_bytes() == (shipped / name).read_bytes(), name


def test_eat_has_no_bowl(out):
    for st in STAGES:
        assert_eat_has_no_bowl(out / "duck" / f"sheet_{st}.png", draw_pets.SPECIES["duck"])


@pytest.mark.parametrize("st", STAGES)
def test_duck_eat_puts_the_bill_on_the_bowl_with_the_real_head_radius(st, monkeypatch):
    """dk_eat의 부리 자리 계산은 duck()의 앞모습 머리 반지름(25)과 같아야 한다."""
    sp = draw_farm.DUCK
    seen = []
    real = draw_farm.duck_bill
    monkeypatch.setattr(draw_farm, "duck_bill", lambda cx, cy, *a, **k: seen.append((cy, k["k"])) or real(cx, cy, *a, **k))
    with draw_pets.stage(st, sp.stage_tweak.get(st)):
        with draw_farm._in_view(draw_pets.staged_species(sp)):
            my = draw_pets._frame_to_pre(64, 84)[1]
            draw_farm.dk_eat(sp, 0)
    cy, k = seen[0]
    assert abs(cy + 6.5 * k * 0.5 - my) < 0.01, (st, cy, k, my)


@pytest.mark.parametrize("anim", ["walk", "idle", "sit_front"])
@pytest.mark.parametrize("stage", ["baby", "adult"])
def test_duck_legs_go_behind_the_body(monkeypatch, anim, stage):
    """다리는 몸 뒤: 몸(가장 큰 조각)을 다리·발보다 나중에 칠해 몸 아래로 발만 나온다 (위에 칠하면 몸에 꽂힌 막대처럼 보인다)."""
    seen = []
    monkeypatch.setattr(draw_pets, "compose", lambda parts, ops, fx: seen.append(list(parts)) or Image.new("RGBA", (8, 8), (0, 0, 0, 255)))
    with draw_pets.stage(stage):
        draw_farm.DUCK_POSES[anim](draw_farm.DUCK, 0)
    parts = seen[0]
    # 몸 = 처음 칠하는 몸 색 조각, 머리 = 그다음 (부리도 주황이라 머리 앞 것만 다리로 센다)
    body, head = [k for k, p in enumerate(parts) if p.grow == 1 and p.base != draw_farm.BILL][:2]
    legs = [k for k, p in enumerate(parts[:head]) if p.base == draw_farm.BILL]
    assert legs and body > max(legs), (anim, stage, legs, body)

def test_duck_scarf_follows_the_neck_when_the_head_drops(out):
    """어른 오리 목도리: 잘 때 머리가 내려가면 목도리도 따라 내려간다 (제자리에 남으면 얼굴에 걸친다)."""
    meta = json.loads((out / "duck" / "pet.json").read_text(encoding="utf-8"))
    fs = meta["frame_sizes"].get("adult", meta["frame_size"])
    sheet = np.asarray(Image.open(out / "duck" / "sheet_adult.png").convert("RGBA")).astype(int)

    def scarf_y(anim):
        r = meta["animations"][anim]["row"]
        f = sheet[r * fs:(r + 1) * fs, :fs]
        m = (np.abs(f[..., :3] - np.array(draw_farm.SCARF)).sum(axis=2) < 30) & (f[..., 3] > 200)
        # 머리를 묻어 목이 거의 안 보이면 목도리도 거의 가린다 (그것도 괜찮다)
        return m.nonzero()[0].mean() if m.sum() > 50 else None

    asleep, awake = scarf_y("sleep"), scarf_y("idle")
    assert awake is not None
    assert asleep is None or asleep - awake >= fs * 0.05
