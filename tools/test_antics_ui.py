"""흐물이 더 놀기 (v0.6.3) 웹뷰: 새 말풍선 대사 · 🎣 매달려 흔들기 (손 자리 축) · 🕺 반짝이 — 코어 Speech·RenderState와 이름이 같다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def test_every_new_speech_has_a_line():
    enum = BRAIN[BRAIN.index("pub enum Speech {"):]
    enum = enum[:enum.index("\n}")]
    variants = re.findall(r"^\s{4}([A-Z]\w*),", enum, re.M)
    for v in ("Swing", "More", "PutMeDown", "Found", "Peekaboo", "Dance", "RallyEnd"):
        assert v in variants
        assert re.search(rf"\b{snake(v)}: \[", MAIN), v


def test_swing_turns_around_the_hands_and_the_fancy_dance_sparkles():
    assert "render.swing" in MAIN and "hang_cursor" in MAIN and "gripOrigin" in MAIN
    assert "render.sparkle" in MAIN
    assert "pub swing: f64" in BRAIN and "pub sparkle: bool" in BRAIN
