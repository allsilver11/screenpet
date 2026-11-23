"""바들바들 떨기 없음 (v0.7.4): 펫 몸은 render.wobble(쿵쾅에 흔들림·들림·아기 넘어짐 등)로 떨지 않음 — 나이 상관없이.
공룡이 제 발로 쿵쾅 밟는 그림(stomp)만 덜덜 남김"""

import pathlib
import re

MAIN = (pathlib.Path(__file__).resolve().parent.parent / "ui" / "main.js").read_text(encoding="utf-8")


def test_the_body_never_jitters_on_wobble():
    body = re.search(r"function jitter\(f, now\) \{(.*?)\n\}", MAIN, re.S).group(1)
    assert "render.wobble" not in body
    assert 'render.anim === "stomp"' in body
