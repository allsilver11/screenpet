"""성장 축하: 말풍선 "I grew up!!", 하트·별 터짐 (창 안, 한 번만, 하우스 안에선 집 폴짝)과 커진 동물의 크기표."""

import json
import re
from pathlib import Path

import pytest

import draw_pets as dp
from sprite_spec import SPEECH, STAGE_SCALE, STAGES
from conftest import needs_node, node_json

ROOT = Path(__file__).resolve().parent.parent
PETS = ROOT / "ui" / "pets"
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
CSS = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
HARNESS = ROOT / "tools" / "levelup_harness.js"


def manifests():
    for d in sorted(PETS.iterdir()):
        yield d.name, json.loads((d / "pet.json").read_text(encoding="utf-8"))


def harness(mode):
    return node_json(HARNESS, mode)


def test_every_pet_says_i_grew_up():
    for pid, m in manifests():
        assert m["speech"]["level_up"] == ["I grew up!!"], pid
    for pid, lines in SPEECH.items():
        assert lines["level_up"] == ["I grew up!!"], pid


def test_size_table_after_the_too_big_feedback():
    """피드백 "동물이 너무 크다": 작던 동물은 v0.3.10 이전 크기로, 큰 동물은 토끼쯤으로 — 기린의 단계별 배율도 없앴다. 산타와 공룡은 사용자 요청으로 그대로."""
    want = {"squirrel": 0.82, "dino_green": 1.2, "dino_yellow": 1.2, "dino_purple": 1.2, "santa": 1.65, "duck": 0.9, "ghost": 0.85, "pumpkin": 0.9, "frog": 0.85,
            "rabbit": 0.85, "cat": 0.91, "persian": 0.92, "giraffe": 1.06, "rudolph": 0.84, "snowman": 0.9, "bird": 0.8, "penguin": 0.85,
            "redpanda": 0.88}
    assert set(want) == set(dict(manifests()))
    for pid, v in want.items():
        m = dict(manifests())[pid]
        assert set(m["stage_size"].values()) == {v} and list(m["stage_size"]) == STAGES, pid


def test_bundled_manifests_match_the_generators():
    """pet.json을 시트 없이 고쳤으니, 다시 그려도 같은 크기·대사가 나오는지 (그림 코드가 원본이다)."""
    for pid, m in manifests():
        assert m["stage_size"] == dict(dp.SPECIES[pid].stage_size), pid
        assert m["speech"] == dp.SPEECH[pid], pid


@pytest.mark.parametrize("pid", ["cat", "giraffe", "rudolph", "snowman"])
def test_big_pets_shrank_to_about_the_rabbits_long_side(pid):
    from draw_house import sit_frame

    def long_side(p):
        m = dict(manifests())[p]
        box = sit_frame(p, "teenager").getchannel("A").point(lambda a: 255 if a > 128 else 0).getbbox()
        return max(box[2] - box[0], box[3] - box[1]) * m["stage_size"]["teenager"]

    assert abs(long_side(pid) / long_side("rabbit") - 1) < 0.08, pid


def burst_js(sides, runs=40):
    a, b = MAIN.index("// BURST-LAYOUT-START"), MAIN.index("// BURST-LAYOUT-END")
    return f"""{MAIN[a:b]}
    const out = {{}};
    for (const side of {json.dumps(sides)}) {{
      out[side] = [];
      for (let i = 0; i < {runs}; i++) out[side].push(burstLayout(side));
    }}
    console.log(JSON.stringify({{ count: BURST_COUNT, out }}));"""


@needs_node
def test_burst_stays_inside_the_window_at_every_stage_and_species():
    sides = sorted({round(128 * STAGE_SCALE[st] * m["stage_size"].get(st, 1.0), 2) for _, m in manifests() for st in STAGES})
    assert min(sides) < 60 and max(sides) > 200
    res = node_json("-e", burst_js(sides))
    assert 10 <= res["count"] <= 14
    for side, layouts in res["out"].items():
        side = float(side)
        w, h = max(128, side), 36 + side
        for lay in layouts:
            assert any(b["heart"] for b in lay["bits"]) and not all(b["heart"] for b in lay["bits"])
            for b in lay["bits"]:
                half = b["size"] * 0.6 * 1.2  # 글자 네모 절반 × 커지는 순간 배율
                assert abs(b["dx"]) + half <= w / 2 + 1e-6, (side, b)
                assert lay["cy"] + b["dy"] + half <= h + 1e-6 and lay["cy"] + b["dy"] - half >= -1e-6, (side, b)
                assert b["delay"] <= 100
            assert max(abs(b["dx"]) for b in lay["bits"]) > 0.3 * side, "밖으로 퍼진다"


def test_burst_is_one_short_css_animation():
    pop = re.search(r"\.burst-bit \{.*?\}", CSS, re.S).group(0)
    assert "pointer-events: none" in pop
    assert float(re.search(r"burst-pop ([\d.]+)s", pop).group(1)) <= 1.0
    assert "@keyframes burst-pop" in CSS
    fn = MAIN[MAIN.index("function levelUpBurst"):MAIN.index("// 첫 pet-render는")]
    assert "requestAnimationFrame" not in fn


@needs_node
@pytest.mark.parametrize("mode", ["main", "main-guest"])
def test_burst_fires_once_per_level_up_and_cleans_up(mode):
    r = harness(mode)
    assert r["beforeLevelUp"] == 0
    assert 10 <= r["onLevelUp"] <= 14
    assert set(r["glyphs"]) == {"♥", "★"}
    assert r["dragEcho"] == r["onLevelUp"], "눌렀다 놓아도 한 번의 성장엔 한 번"
    assert r["afterOneEnded"] == r["onLevelUp"] - 1
    assert r["afterFallback"] == 0, "창이 숨겨져 animationend가 안 와도 지운다"
    assert r["housed"] == 0, "하우스 안에선 터지지 않는다"
    assert r["hidden"] == 0, "숨은 창엔 쌓지 않는다"
    assert 10 <= r["nextLevelUp"] <= 14


@needs_node
def test_burst_leaves_the_nightcap_alone():
    r = harness("main")
    assert r["capBefore"] and r["capAfter"] == r["capBefore"] and not r["capHidden"]
    assert not {"burst-bit", ""} & set(r["petChildren"]), "조각은 #pet의 자식이 아니다"
    assert "nightcap" in r["petChildren"]


@needs_node
def test_house_hops_instead_of_bursting():
    r = harness("house")
    assert not r["hopOutside"], "밖에서 축하하면 집은 가만히"
    assert r["hopHoused"] and r["bits"] == 0
    assert not r["hopEnded"] and not r["hopSameCelebration"], "한 번만"
    assert r["bubble"] == "I grew up!!" and not r["bubbleHidden"]


@needs_node
def test_house_keeps_the_old_art_until_the_new_one_is_decoded():
    r = harness("house")
    assert r["waiting"] == 3, "시트 + 집 그림 + 문틀"
    assert set(r["artWhileLoading"]) == {r["artBefore"]} and "teenager" in r["artBefore"]
    assert "adult" in r["artAfter"] and r["houseWidth"] == f"{128 * 1.12}px"


def test_house_hop_is_small_and_short():
    hop = re.search(r"@keyframes house-hop \{.*?\n\}", CSS, re.S).group(0)
    assert max(int(v) for v in re.findall(r"translateY\(-(\d+)px\)", hop)) <= 12
    assert re.search(r"#house\.house-hop \{\s*animation: house-hop 0\.4s", CSS)
    # 창이 먼저 커져도 옛 집이 뜨지 않게 창 아래에 붙인다
    house = re.search(r"#house \{.*?\}", CSS, re.S).group(0)
    assert "bottom: 0" in house and "top:" not in house
