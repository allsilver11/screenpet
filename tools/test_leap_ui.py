"""큰 점프 그림 대체의 약속 — bigjump 없는 걷는 동물은 웹뷰가 hop 줄로 그린다: 웅크림을 버티다가 뜨는 순간 공중 프레임."""

import json
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
COMMON = (ROOT / "ui" / "common.js").read_text(encoding="utf-8")
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
HOP_RS = (ROOT / "crates" / "screenpet-core" / "src" / "hop.rs").read_text(encoding="utf-8")


def test_crouch_time_matches_the_core():
    secs = float(re.search(r"BIG_JUMP_CROUCH_SECS: f64 = ([\d.]+);", HOP_RS).group(1))
    assert int(re.search(r"const LEAP_CROUCH_MS = (\d+);", COMMON).group(1)) == round(secs * 1000)


def test_main_uses_the_fallback_only_without_bigjump_art():
    assert 'render.anim === "bigjump" && !manifest.animations.bigjump' in MAIN
    assert "hopLeapFrame(spec, elapsed)" in MAIN


@pytest.mark.skipif(not shutil.which("node"), reason="node 없음")
def test_hop_frames_crouch_then_fly_and_hold():
    fn = COMMON[COMMON.index("const LEAP_CROUCH_MS"):COMMON.index("// 말풍선(`pet-say`)")]
    spec = {"row": 6, "frames": 3, "fps": 10, "loop": False}
    times = [0, 250, 499, 500, 599, 600, 700, 5000]
    js = f"{fn}\nconsole.log(JSON.stringify({times}.map((t) => hopLeapFrame({json.dumps(spec)}, t))));"
    out = subprocess.run(["node", "-e", js], capture_output=True, text=True, check=True).stdout
    assert json.loads(out) == [0, 0, 0, 1, 1, 2, 2, 2]


def test_every_walker_has_a_hop_row_to_fall_back_on():
    for d in (ROOT / "ui" / "pets").iterdir():
        m = json.loads((d / "pet.json").read_text(encoding="utf-8"))
        if m.get("locomotion") == "fly":
            continue
        a = m["animations"]
        assert "bigjump" in a or a["hop"]["frames"] >= 3, d.name
