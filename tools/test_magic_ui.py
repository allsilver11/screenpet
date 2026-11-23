"""🐍 볼드모트·👻 디멘터·⚔️ 결투·🦌 패트로누스의 웹뷰 쪽: ✨ 마법 효과 (ui/magic.js — 무지개 구름·낙서·거미집·박쥐 신호·스니치와 같은 모니터 덮개 창
trail.html의 스니치 아래 캔버스), 새 말풍선 키의 기본 대사와 도움말 줄. 앱 쪽 연결 (trail.rs·game_loop.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "magic.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "trail.rs").read_text(encoding="utf-8")
LOOP = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
MAIN = (UI / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
MAGIC_RS = (ROOT / "crates" / "screenpet-core" / "src" / "magic.rs").read_text(encoding="utf-8")


def test_magic_has_the_canvas_just_under_the_snitch():
    assert '<canvas id="magic">' in HTML and '<script src="magic.js"></script>' in HTML
    assert HTML.index('id="trail"') < HTML.index('id="magic"') < HTML.index('id="snitch"')
    assert '"trail-magic"' in RS and '"trail-magic"' in JS
    assert "setInterval" not in JS and "innerHTML" not in JS


def test_the_webview_knows_every_effect_kind_of_the_core():
    kinds = re.findall(r"^    (\w+) \{", MAGIC_RS[MAGIC_RS.index("pub enum MagicFx") :], re.M)
    snake = [re.sub(r"(?<!^)([A-Z])", r"_\1", k).lower() for k in kinds]
    assert 'const MG_KINDS = [' in JS and set(snake) == set(re.findall(r'"(\w+)"', JS[JS.index("const MG_KINDS") :].split("\n")[0]))
    colors = MAGIC_RS[MAGIC_RS.index("pub enum BeamColor") :]
    for c in re.findall(r"^    (\w+),", colors[: colors.index("\n}")], re.M):
        assert f'"{c.lower()}"' in JS or c == "Red", c


def test_rust_side_sends_magic_from_every_pet_and_keeps_the_window_open_and_chill_reaches_care():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🖍️ 동물마다 새 낙서")]
    assert "self.magic(app, now, slots);" in tick
    send = RS[RS.index("fn magic(") : RS.index("/// 🧹 모든 물건 다 치우기")]
    assert "magic_fx(" in send and "LINE_EVERY" in send and "f.anchor()" in send and "f.mapped(" in send
    house = RS[RS.index("fn housekeep(") :]
    assert "magic_done" in house[: house.index("self.tear_down();")]
    assert "take_chill()" in LOOP and "care.chill(chill)" in LOOP


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "magic_harness.js")


def test_listens_to_one_event_and_idles_until_magic(run):
    assert run["listeners"] == ["trail-magic"] and run["idle"] == 0


def test_bad_effects_are_dropped_and_capped(run):
    assert run["filtered"] == 1 and run["capped"] == 24


def test_frost_alone_draws_once_without_animation_frames(run):
    assert run["frost_drawn"] and run["frost_rafs"] == 0


def test_moving_magic_animates_then_a_clashed_duel_pops_and_a_bolt_fades(run):
    assert run["moving_frames"] == 30 and run["ellipses"] >= 30 * 3, "사슴 몸·머리·뱀 머리"
    assert run["pops"] == 1 and run["ghosts"] == 1
    assert 10 <= run["end_frames"] <= 60 and run["rafs_end"] == 0 and run["pops_end"] == 0 and run["ghosts_end"] == 0
    assert run["no_pop"] == 0, "아직 뻗던 결투는 펑 없음"


def test_hagrid_egg_and_baby_dragon_animate_and_are_checked(run):
    """🥚 용알·🐉 아기 용: 이상한 값은 버리고, 있는 동안 프레임을 돌리며 그림 (깨어난 용은 끝에 불 퐁 — 그라데이션)."""
    assert run["hagrid_filtered"] == 2
    assert run["hagrid_frames"] == 20 and run["hagrid_ellipses"] >= 20 * 4 and run["hagrid_flame"] > 0


def test_campfire_flickers_and_is_checked(run):
    """🔥 모닥불: 이상한 값은 버리고, 있는 동안 프레임을 돌리며 불빛 (그라데이션)·돌 (타원)·불꽃 (곡선)·불티 (원)을 그림 — 꺼진 불 (level 0)은 장작만."""
    assert run["fire_filtered"] == 1
    assert run["fire_frames"] == 20 and run["fire_glow"] >= 20 and run["fire_curves"] >= 20 * 9 and run["fire_arcs"] >= 20 * 7
    assert run["out_glow"] == 0 and run["out_rects"] >= 2


def test_dark_speech_defaults_and_help_lines():
    """🐍👻 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), 도움말에 볼드모트·디멘터 한 줄씩 (해요체)."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("Stupefy", "stupefy"), ("Dazed", "dazed"), ("Nagini", "nagini"), ("Hehehe", "hehehe"), ("DuelCall", "duel_call"),
                   ("Patronus", "patronus"), ("Gloom", "gloom"), ("Repelled", "repelled")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Expecto Patronum!"' in extra and 'voldemort: ["' in MAIN and 'dementor: ["' in MAIN
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    v = next(li for li in help_html.split("<li>") if li.startswith("🐍 볼드모트"))
    assert "나기니" in v and "기절" in v and "결투" in v and "헤르미온느" in v and v.rstrip().endswith("요.</li>")
    d = next(li for li in help_html.split("<li>") if li.startswith("👻 디멘터"))
    assert "서리" in d and "패트로누스" in d and d.rstrip().endswith("요.</li>")


def test_wizard_group_events_speech_tuck_bag_and_help():
    """🎒 가방 숨기기·🔥 모닥불·📖 공부 방해: 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름),
    가방 속으로 쏙 (render.tuck — 발밑을 축으로 작아지며 흐려짐), 결투 (cast) 동안 가방은 바닥에, 도움말 줄 (해요체)과 마법사 집."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("InYouGo", "in_you_go"), ("PoppedOut", "popped_out"), ("Reunion", "reunion"), ("LightFire", "light_fire"),
                   ("Fireside", "fireside"), ("HaHa", "ha_ha"), ("Douse", "douse"), ("Pester", "pester"), ("Shh", "shh"), ("OhAlright", "oh_alright")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Shh! I\'m studying!"' in extra and '"In you go!"' in extra and '"Ha ha!"' in extra
    assert re.search(r"^    pub tuck: f64,", BRAIN, re.M)
    assert "render.tuck" in MAIN and '"50% 100%"' in MAIN and 'writePet("opacity"' in MAIN
    assert 'anim === "cast" ? null : bagHand(anim, f)' in MAIN
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    for start, words in (("🎒", ("In you go!", "Phew!", "결투")), ("🔥", ("모닥불", "Ha ha!", "흩어져요")), ("📖 헤르미온느가", ("Shh! I'm studying!", "술래잡기", "달리기 시합"))):
        li = next(li for li in help_html.split("<li>") if li.startswith(start))
        assert all(w in li for w in words) and li.rstrip().endswith("요.</li>"), start
    assert "호그와트 성" in help_html and "나무집" in help_html


def test_wand_and_lumos_glow_animate_and_are_checked(run):
    """🪄 Expelliarmus에 날아간 지팡이 (빙글 — rotate)·💡 루모스 불빛 (그라데이션): 이상한 값은 버리고, 있는 동안 프레임을 돌리며 그림."""
    assert run["wizard_filtered"] == 2
    assert run["wizard_frames"] == 10 and run["wizard_glow"] >= 10 and run["wizard_turns"] >= 10


def test_nox_puff_animates_and_is_checked(run):
    """🌑 녹스 연기 (puff — 피어오르는 검은 연기 덩이): 이상한 값은 버리고, 있는 동안 프레임을 돌리며 그림."""
    assert run["puff_filtered"] == 1
    assert run["puff_frames"] == 10 and run["puff_gradients"] >= 10 * 3 and run["puff_arcs"] >= 10 * 3


def test_long_duel_and_lumos_speech_topple_and_help():
    """⚔️ 긴 결투·💡 루모스: 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), Petrificus에 통나무처럼 눕기
    (render.topple — 가운데를 축으로 돌려 바닥 쪽으로), 앱이 이 화면의 Lumos를 🌗 일꾼에게 넘기고 ⚙️ 설정 체크 항목이 있음, 도움말 (해요체)."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("Petrificus", "petrificus"), ("AvadaKedavra", "avada_kedavra"), ("Petrified", "petrified"), ("Dodged", "dodged"),
                   ("DuelFlee", "duel_flee"), ("DuelChase", "duel_chase"), ("DuelRest", "duel_rest"), ("Lumos", "lumos"), ("Nox", "nox")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Petrificus Totalus!"' in extra and '"Avada Kedavra!"' in extra and '"Lumos!"' in extra and '"Nox!"' in extra
    assert re.search(r"^    pub topple: f64,", BRAIN, re.M) and "render.topple" in MAIN and "rotate(${topple" in MAIN
    assert "take_lumos()" in LOOP and "take_nox()" in LOOP and "l.nox();" in LOOP and "lumos::spawn(" in LOOP and "ToggleLumosLight" in LOOP and "set_icon_lift_ok(" in LOOP
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert "🌗 루모스·녹스로 화면 밝기 바꾸기" in tray and "&lumos_light, &web_ledges, &autostart]" in tray
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    for start, words in (("⚔️ <b>결투</b>", ("Expelliarmus!", "Petrificus Totalus!", "Avada Kedavra!", "2초", "catch me!", "흩어져요")),
                         ("💡 \"Lumos!\"", ("🌗 루모스·녹스로 화면 밝기 바꾸기", "라이트 모드")),
                         ("🌑 \"Nox!\"", ("🌗 루모스·녹스로 화면 밝기 바꾸기", "다크 모드", "연기")),
                         ("<b>⚙️ 설정 ▸ 🌗 루모스·녹스로 화면 밝기 바꾸기</b>", ("꺼져 있어요", "되돌려요", "1분", "Nox!", "다크 모드"))):
        li = next(li for li in help_html.split("<li>") if li.startswith(start))
        assert all(w in li for w in words) and li.rstrip().endswith("요.</li>"), start
    assert "앱 아이콘 하나를 쏙 뽑아" in help_html


def test_patronus_otter_and_soul_stream_animate_and_are_checked(run):
    """🦦 헤르미온느의 은빛 수달 (몸·꼬리·기울기)·👻 디멘터가 빼앗는 영혼 줄기 (일렁이는 가닥·빨려 올라가는 빛): 이상한 값은 버리고,
    있는 동안 프레임을 돌리며 그림 — 짙기 0인 줄기는 그리지 않음."""
    assert run["otter_soul_filtered"] == 2
    assert run["otter_soul_frames"] == 10 and run["otter_soul_ellipses"] >= 10 * 3 and run["otter_soul_turns"] >= 10
    assert run["otter_soul_curves"] >= 10 * 4 and run["otter_soul_gradients"] >= 10 * 2
    assert run["soul_dim_curves"] == 0


def test_soul_drain_speech_and_help():
    """👻 영혼 빼앗기: 새 말풍선 키마다 기본 영어 대사 (ui/main.js EXTRA_SPEECH — 코어 Speech와 같은 이름), 도움말 디멘터 줄에 영혼·기절·헤르미온느 수달."""
    enum = BRAIN[BRAIN.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = MAIN[MAIN.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key in (("SoulDrain", "soul_drain"), ("SoulTaken", "soul_taken"), ("SoulSaved", "soul_saved")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"my soul...!"' in extra and '"phew!"' in extra
    help_html = (UI / "help.html").read_text(encoding="utf-8")
    d = next(li for li in help_html.split("<li>") if li.startswith("👻 디멘터"))
    assert "영혼" in d and "기절" in d and "수달" in d and d.rstrip().endswith("요.</li>")
