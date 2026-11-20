"""🦇 박쥐인간의 🔦 박쥐 신호 — 🌈 무지개 구름·🖍️ 낙서·🕷️ 거미집과 같은 모니터 덮개 창 (trail.html)의 맨 아래 캔버스 (ui/batsignal.js).
새 창 없음 (마우스 통과·포커스 안 뺏음), 비추는 동안만 그림. 앱 쪽 연결 (trail.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "batsignal.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
CORE = (ROOT / "crates" / "screenpet-core" / "src" / "bike.rs").read_text(encoding="utf-8")


def test_the_signal_has_the_bottom_canvas_of_the_trail_window():
    assert '<canvas id="signal">' in HTML and '<script src="batsignal.js"></script>' in HTML
    assert HTML.index('id="signal"') < HTML.index('id="doodle"') < HTML.index('id="web"')
    assert RS.count("screen_cover(") == 1
    assert '"trail-signal"' in RS and '"trail-signal"' in JS


def test_scripts_in_one_page_do_not_clash():
    """trail.js·doodle.js·web.js·batsignal.js는 한 페이지의 전역을 같이 쓴다 — 최상위 이름이 겹치면 페이지가 멈춘다."""
    names = {}
    for f in ("trail.js", "doodle.js", "web.js", "batsignal.js"):
        for n in re.findall(r"^(?:const|let|function) (\w+)", (UI / f).read_text(encoding="utf-8"), re.M):
            assert n not in names, (n, f, names.get(n))
            names[n] = f


def test_rust_side_takes_signals_from_every_pet_and_keeps_the_window_open_while_shining():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🦇 동물마다 새 박쥐 신호")]
    assert "self.bat_signals(app, now, slots);" in tick and "let _ = s.pet.take_bat_signal();" in tick
    place = RS[RS.index("fn place_signal(") : RS.index("/// 🕷️ 동물마다 새 거미집")]
    assert "self.signals_until" in place and "b.secs" in place
    house = RS[RS.index("fn housekeep(") :]
    assert "signals_done" in house[: house.index("self.tear_down();")]
    # 렌즈 자리는 그림과 같은 점
    lens = re.search(r"SIGNAL_LENS = \(([\d.]+), ([\d.]+)\)", (ROOT / "tools" / "draw_bat.py").read_text(encoding="utf-8"))
    assert f"SIGNAL_FROM: (f64, f64) = ({lens.group(1)} / 128.0, {lens.group(2)} / 128.0)" in CORE
    assert "setInterval" not in JS and "innerHTML" not in JS


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "batsignal_harness.js")


def test_listens_to_one_event_and_idles_until_a_signal(run):
    assert run["listeners"] == ["trail-signal"] and run["idle"] == 0


def test_bad_signals_are_dropped_and_capped(run):
    assert run["filtered"] == 1 and run["capped"] == 4


def test_frames_only_while_shining(run):
    secs = float(re.search(r"pub const SIGNAL_SECS: f64 = ([\d.]+);", CORE).group(1))
    assert (secs + 0.8) * 60 * 0.9 <= run["frames"] <= (secs + 0.8) * 62.5 + 2
    assert run["fills"] >= run["frames"] * 2, "빛줄기 + 빛 동그라미 + 박쥐"
    assert run["rafs_after"] == 0 and run["alive_end"] == 0


def test_bat_speech_defaults_and_help_line():
    """🦇 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), 도움말에 박쥐인간 한 줄 (해요체)."""
    main = (UI / "main.js").read_text(encoding="utf-8")
    brain = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
    enum = brain[brain.index("pub enum Speech {"):]
    enum = enum[: enum.index("\n}")]
    extra = main[main.index("const EXTRA_SPEECH = {"):]
    extra = extra[: extra.index("\n};")]
    for v, key in (("Vroom", "vroom"), ("Wheelie", "wheelie"), ("BikeTrick", "bike_trick"), ("BatSignal", "bat_signal"), ("HeroLift", "hero_lift")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert "\"I'm the night\"" in extra and 'bat: ["to the bike!"' in main
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    bat = next(li for li in help_html.split("<li>") if li.startswith("🦇 박쥐인간"))
    assert "오토바이" in bat and "박쥐 무늬" in bat and "거대한" in bat and bat.rstrip().endswith("요.</li>")
