"""🌱 연령대별 특기 (2026-11-20 spec 2부) 웹뷰·도움말: 새 말풍선 대사(짧은 영어) · 연령대 말투 표 (종 말에 더해짐, pet.json stage_speech로 덮음) ·
레벨업 뒤 새 특기 한 줄 · 도움말의 연령대 표 — 코어 Speech·stage.rs와 이름이 같다."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
STAGE = (ROOT / "crates" / "screenpet-core" / "src" / "stage.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
STAGES = ["baby", "kinder", "child", "teenager", "adult"]

LINES = {"Plop": "oops!", "Waah": "waah", "Ooh": "ooh?", "Flip": "yeah!", "Sulk": "ugh", "GoodKid": "good kid", "TagAlong": "wait for me!"}


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def test_every_stage_speech_has_its_short_english_line():
    enum = BRAIN[BRAIN.index("pub enum Speech {"):]
    enum = enum[:enum.index("\n}")]
    variants = re.findall(r"^\s{4}([A-Z]\w*),", enum, re.M)
    for v, line in LINES.items():
        assert v in variants, v
        m = re.search(rf"\b{snake(v)}: \[([^\]]*)\]", MAIN)
        assert m and f'"{line}"' in m.group(1), v
        assert all(len(s) <= 16 and s.isascii() for s in re.findall(r'"([^"]*)"', m.group(1))), v


def test_stage_lines_are_added_under_species_lines_and_pet_json_can_override():
    table = MAIN[MAIN.index("const STAGE_SPEECH = {"):]
    table = table[:table.index("\n};")]
    for s in STAGES:
        assert re.search(rf"^\s+{s}: \{{ chatter: \[", table, re.M), s
    assert '"ba!"' in table and '"mama?"' in table and '"whatever"' in table and '"hm hm"' in table
    body = MAIN[MAIN.index("function stageSpeech"):]
    body = body[:body.index("\n}\n")]
    assert "...(out[key] ?? [])" in body, "같은 키는 이어 붙임 (종 말이 사라지지 않음)"
    assert "m.stage_speech?.[stageName]" in body
    assert "stageSpeech({ cursor_play:" in MAIN and "manifest, stage.stage)" in MAIN


def test_level_up_announces_one_short_signature_trait_per_stage():
    body = STAGE[STAGE.index("pub fn signature_line"):]
    lines = re.findall(r'=> "([^"]*)"', body[:body.index("\n}\n")])
    assert len(lines) == 5 and all(l.startswith("new: ") and len(l) <= 24 for l in lines), lines
    assert "new: 🤸 flips!" in lines


def test_help_has_the_stage_table_saying_every_age_can_but_its_own_age_does_most():
    sec = HELP[HELP.index("🍚 밥 주고 키우기"):]
    sec = sec[:sec.index("</section>")]
    assert '<table class="stages">' in sec
    rows = re.findall(r"<tr><th>([^<]+)</th>", sec)
    assert rows == ["👶 아기", "🧒 유치원", "🧑 어린이", "🧑‍🎤 청소년", "🧓 어른"], rows
    assert "모든 연령대가 다 해요" in sec and "그 연령대가 훨씬 자주" in sec
    for word in ("엉덩방아", "ooh?", "깡총깡총", "공중제비", "쓰다듬어"):
        assert word in sec, word
