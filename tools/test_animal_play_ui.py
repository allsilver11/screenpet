"""다른 동물들도 더 놀기 (v0.6.3) · 펫끼리 놀기 더 (2026-11-20) 웹뷰: 새 말풍선 대사(짧고 귀여운 영어) · 🙌 하이파이브 짝! 반짝이 · 도움말 — 코어 Speech·RenderState와 이름이 같다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")

LINES = {"Tap": "Tap… tap…", "Oops": "Oops~", "FollowMe": "Quack! Follow me!", "Choo": "Choo choo!", "BigStomp": "STOMP!",
         "Nom": "Nom nom~", "HighFive": "High five!"}


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def test_every_new_speech_has_its_english_line():
    enum = BRAIN[BRAIN.index("pub enum Speech {"):]
    enum = enum[:enum.index("\n}")]
    variants = re.findall(r"^\s{4}([A-Z]\w*),", enum, re.M)
    for v, line in LINES.items():
        assert v in variants, v
        m = re.search(rf"\b{snake(v)}: \[([^\]]*)\]", MAIN)
        assert m and f'"{line}"' in m.group(1), v
    assert '"Yay!"' in re.search(r"\bchoo: \[([^\]]*)\]", MAIN).group(1)
    # 큰 쿵에 놀란 친구는 예전 쿵쾅처럼 "whoa!"
    assert re.search(r"\bwhoa: \[", MAIN)


def test_high_five_clap_bursts_sparks_in_front_once():
    assert "pub clap: bool" in BRAIN
    assert re.search(r"e\.payload\.clap && !render\.clap\) clapBurst\(e\.payload\.flip\)", MAIN)
    body = MAIN[MAIN.index("function clapBurst"):]
    body = body[:body.index("\n}\n")]
    assert "spark(" in body and "pixelBit(" in body and "flip ? -1 : 1" in body


# 펫끼리 놀기 더 (2026-11-20, 코어 social_romp.rs): 새 놀이의 말풍선 키마다 기본 영어 대사가 있고, 도움말에 한 줄씩
ROMP_LINES = {"Scuffle": "mine!", "Hehe": "hehe", "Hey": "hey!", "HeyBack": "you hey!", "Ready": "ready…", "Go": "go!",
              "IWin": "I win!", "NoFair": "no fair!", "ChatA": "nice day", "ChatB": "same!"}


def test_every_romp_speech_has_its_english_line():
    enum = BRAIN[BRAIN.index("pub enum Speech {"):]
    enum = enum[:enum.index("\n}")]
    variants = re.findall(r"^\s{4}([A-Z]\w*),", enum, re.M)
    for v, line in ROMP_LINES.items():
        assert v in variants, v
        m = re.search(rf"\b{snake(v)}: \[([^\]]*)\]", MAIN)
        assert m and f'"{line}"' in m.group(1), v


def test_help_lists_each_new_pet_to_pet_play():
    help_html = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
    for name in ("투닥투닥", "몰래 찌르기", "밀치기", "같이 낮잠", "달리기 시합", "수다"):
        assert f"<b>{name}</b>" in help_html, name
