"""🕷️ 거미인간의 줄과 모서리 거미집 — 🌈 무지개 구름·🖍️ 낙서와 같은 모니터 덮개 창 (trail.html)의 가운데 캔버스 (ui/web.js).
새 창 없음 (마우스 통과·포커스 안 뺏음), 손님 거미인간 것도, 창이 움직이면 그 거미집만, 🧹 다 치우기·한 시간마다 전부 지움.
앱 쪽 연결 (trail.rs · game_loop.rs · objects.rs)은 이 컴퓨터에서 빌드할 수 없어 소스로 본다."""

import re
from pathlib import Path

import pytest
from conftest import node_json

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
JS = (UI / "web.js").read_text(encoding="utf-8")
HTML = (UI / "trail.html").read_text(encoding="utf-8")
SRC = ROOT / "src-tauri" / "src"
RS = (SRC / "trail.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
OBJ = (SRC / "objects.rs").read_text(encoding="utf-8")
CORE_WEB = (ROOT / "crates" / "screenpet-core" / "src" / "web.rs").read_text(encoding="utf-8")


def test_webs_share_the_trail_window_between_doodles_and_clouds():
    assert '<canvas id="web">' in HTML and '<script src="web.js"></script>' in HTML
    assert HTML.index('id="doodle"') < HTML.index('id="web"') < HTML.index('id="trail"')
    # 다른 창 종류를 만들지 않는다 (덮개 창은 overlay::screen_cover 하나 — 포커스를 뺏지 않는 마우스 통과 창)
    assert RS.count("screen_cover(") == 1
    for ev in ("trail-webs", "trail-lines", "trail-web-drop", "trail-web-clear", "trail-web-move"):
        assert f'"{ev}"' in RS and f'"{ev}"' in JS, ev


def test_scripts_in_one_page_do_not_clash():
    """trail.js·doodle.js·web.js는 한 페이지의 전역을 같이 쓴다 — 최상위 이름이 겹치면 페이지가 멈춘다."""
    names = {}
    for f in ("trail.js", "doodle.js", "web.js"):
        for n in re.findall(r"^(?:const|let|function) (\w+)", (UI / f).read_text(encoding="utf-8"), re.M):
            assert n not in names, (n, f, names.get(n))
            names[n] = f


def test_timing_matches_the_core():
    nums = {k: float(v) for k, v in re.findall(r"const (WEB_LIFE|WEB_FADE|WEB_DROP|MAX_WEBS) = ([\d.]+);", JS)}
    core = {k: float(v) for k, v in re.findall(r"pub const (WEB_LIFE_SECS|MAX_WEBS): \w+ = ([\d.]+);", CORE_WEB)}
    assert nums["WEB_LIFE"] == core["WEB_LIFE_SECS"] and nums["MAX_WEBS"] == core["MAX_WEBS"]
    # 앱은 거미집이 다 흐려질 때까지 창을 둔다 (WEB_LIFE_SECS + WEB_FADE)
    assert f"WEB_LIFE_SECS as u64 + {int(nums['WEB_FADE'])}" in RS
    assert "setInterval" not in JS and "innerHTML" not in JS


def test_rust_side_takes_webs_and_lines_from_every_pet_and_keeps_the_window_alive():
    tick = RS[RS.index("pub fn tick(") : RS.index("/// 🕷️ 동물마다 새 거미집")]
    assert "let _ = s.pet.take_web();" in tick, "숨김·자리 비움 동안 친 거미집은 버림"
    assert "self.spider_webs(app, now, slots);" in tick and "self.web_lines(app, now, slots);" in tick
    taking = RS[RS.index("fn spider_webs(") : RS.index("fn place_web(")]
    assert "for i in 0..slots.len()" in taking and "take_web()" in taking and "s.on_screen()" in taking and "overlay_paused(" in taking
    lines = RS[RS.index("fn web_lines(") : RS.index("pub fn clear_webs(")]
    assert "web_line()" in lines and "LINE_EVERY" in lines and "want == o.lines" in lines
    house = RS[RS.index("fn housekeep(") : RS.index("/// 덮개 창을 모두 닫음")]
    assert "webs_done" in house and "lines_done" in house and "follow_window_webs" in house
    assert "AUTO_CLEAR" in house and "self.clear_webs(app)" in house


def test_clear_all_also_clears_webs_and_yanked_objects_fly():
    clear = LOOP[LOOP.index("if matches!(input, ObjectInput::ClearAll)") :][:300]
    assert "weather.clear_piles();" in clear and "trail.clear_webs(&app);" in clear
    yank = OBJ[OBJ.index("ObjectPlay::Yank") :][:300]
    assert "l.object.spot(l.id).is_some()" in yank and "l.object.launch(vx, vy)" in yank


def test_window_webs_follow_their_window():
    follow = RS[RS.index("fn follow_window_webs(") : RS.index("/// 그 자리 모니터의 창")]
    assert "own.world.windows" in follow and "web_on_window(" in follow
    assert '"trail-web-drop"' in follow and '"trail-web-move"' in follow and "WebAt::Gone" in follow
    assert "pub fn web_on_window(" in CORE_WEB


def test_pets_learn_the_webs_and_torn_webs_are_dropped():
    """🕸️ 거미집에 걸리기 (코어 brain_snare.rs): 게임 루프가 거미집 자리를 모든 동물 뇌에 알려 주고, 찢고 나온 거미집은 지운다."""
    spots = RS[RS.index("pub fn web_spots(") : RS.index("pub fn tear_web(")]
    assert "WebSpot { id: t.id, x: t.x, y: t.y, corner: t.corner, size: t.size, hidden: t.hidden || paused }" in spots
    tear = RS[RS.index("pub fn tear_web(") : RS.index("pub fn sweep_webs(")]
    assert "o.webs.retain(|(_, w)| w.id != id)" in tear, "아직 안 보낸 거미집도 버림"
    assert "self.webs.remove(i)" in tear and '"trail-web-drop", [t.id]' in tear
    notice = LOOP.index("hoist_moves::notice_pets(&mut slots);")
    tell = LOOP.index("let web_spots = trail.web_spots();")
    update = LOOP.index("s.pet.update(dt, &s.world);")
    assert notice < tell < update and "s.pet.set_webs(&web_spots);" in LOOP[tell:update]
    lifts = LOOP.index("hoist_moves::sync_lifts(&mut slots)")
    torn = LOOP.index("s.pet.take_torn_web()")
    assert lifts < torn < LOOP.index("trail.tick(&app, now, dt") and "trail.tear_web(&app, id);" in LOOP[torn:][:200]
    assert "pub struct WebSpot" in CORE_WEB and "pub fn in_web(" in CORE_WEB



def test_objects_learn_the_webs_every_tick_to_get_snagged():
    """🕸️📦 물건도 거미줄에 걸림 (spec 5부 — 코어 object.rs Snagged): 게임 루프가 펫에게 준 거미집 자리를 물건들에도 주고,
    물건은 틱마다 그 목록으로 걸리고 (사라진 거미집이면 떨어짐) 매달림."""
    tell = LOOP.index("let web_spots = trail.web_spots();")
    give = LOOP.index("objects.set_webs(&web_spots);")
    tick = LOOP.index("objects.tick(&app, &slots[OWN].world")
    assert tell < give < tick
    assert "pub fn set_webs(&mut self, webs: &[WebSpot])" in OBJ and "webs: Vec<WebSpot>," in OBJ
    loop_ = OBJ[OBJ.index("for (i, l) in self.live.iter_mut().enumerate()") :][:600]
    assert loop_.index("l.object.set_webs(&self.webs);") < loop_.index("l.object.tick(")
    assert "use screenpet_core::web::WebSpot;" in OBJ


def test_spider_speech_defaults_cover_the_hero_lines_and_the_multiverse():
    """🦸 영웅 착지 대사 10개 안팎 · 🕷️🕷️🕷️ 멀티버스 모임 말 (코어 Speech::Multiverse / SeeYa) — pet.json에 없을 때의 기본 말 (ui/main.js EXTRA_SPEECH)."""
    main = (UI / "main.js").read_text(encoding="utf-8")
    extra = main[main.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    hero = re.search(r"hero_land: \[(.*?)\]", extra).group(1)
    assert hero.count('"') // 2 >= 8, hero
    assert re.search(r'multiverse: \[[^\]]*"multiverse\?!"', extra) and re.search(r'see_ya: \[[^\]]*"see ya!"', extra)

# ---- ui/web.js를 가짜 캔버스 위에서 (tools/web_harness.js) ----


@pytest.fixture(scope="module")
def run():
    return node_json(ROOT / "tools" / "web_harness.js")


def test_listens_only_to_its_five_events_and_idles_until_one_arrives(run):
    assert run["listeners"] == ["trail-lines", "trail-web-clear", "trail-web-drop", "trail-web-move", "trail-webs"]
    assert run["idle"] == 0


def test_webs_are_capped_and_checked(run):
    assert run["capped"] == 12 and run["filtered"] == 1


def test_frames_only_while_growing_fading_or_with_a_line(run):
    assert 30 <= run["grow_frames"] <= 50, "피어나는 0.6초"
    assert run["hold_rafs"] == 0 and run["hold_timers"] == 1, "가만히 있는 동안은 타이머 하나"
    assert 200 <= run["fade_frames"] * 16 / 4 <= 700 and run["alive_end"] == 0
    assert run["line_strokes"] >= 20 and run["line_rafs"] == 1, "줄이 있는 동안 매 프레임 (그늘 + 실)"
    assert run["line_rafs_after"] == 0 and run["bad_line_rafs"] == 0


def test_drop_removes_one_web_and_clear_removes_all(run):
    assert run["after_drop"] == 1 and run["after_clear"] == 0


def test_a_window_web_moves_with_its_window_and_hides_while_covered(run):
    assert run["moved"] == [500, 120, False] and run["move_rafs"] == 1, "옮기면 바로 다시 그림"
    assert run["hidden"] == [500, 120, True] and run["hidden_strokes"] < run["shown_strokes"], "가려지면 그리지 않음"
    assert run["untouched"] == [0, 0, False], "이상한 값은 무시"


def test_a_woven_web_appears_piece_by_piece_shot_by_shot(run):
    woven = [m for m in run["woven"] if m]
    assert woven[0] < woven[-1] and woven[-1] == max(woven) >= 2 * (4 + 3), "처음엔 몇 조각, 다 짜면 살 + 고리 전부 × (그늘 + 실)"
    assert all(b >= a for a, b in zip(woven, woven[1:])), "조각은 늘기만"
    assert len(set(woven)) >= 5, "한 번에 휙이 아니라 여러 번에 걸쳐"
    assert run["bad_weave"] == [[0, 1], [0, 1]], "이상한 값이면 예전처럼 한 번에 핌"


def test_every_web_has_its_own_irregular_shape_inside_its_radius(run):
    shapes = run["shapes"]
    assert shapes == run["shapes_again"], "같은 거미집은 늘 같은 모양"
    assert all(4 <= n <= 7 and 3 <= m <= 5 and 0.6 < far <= 1.0 + 1e-9 for n, m, far, _ in shapes)
    assert len({(n, m) for n, m, _, _ in shapes}) >= 3, "살·고리 수가 제각각"
    even = [[1.5707963 * k / (n - 1) for k in range(n)] for n, _, _, _ in shapes]
    assert any(max(abs(a - b) for a, b in zip(angs, ev)) > 0.05 for (_, _, _, angs), ev in zip(shapes, even)), "고르게 나눈 각도가 아님 (비정형)"


# ---- 🕸️⭕ 둥근 거미집 (spec 6부 — corner "center") ----


def test_round_webs_are_accepted_with_their_own_irregular_shape(run):
    assert run["orb_alive"] == 8, "center도 받아들임"
    orbs = run["orbs"]
    assert orbs == run["orbs_again"], "같은 거미집은 늘 같은 모양 (id로 정함)"
    assert all(7 <= n <= 9 and 3 <= m <= 5 and 0.7 < far <= 1.0 + 1e-9 for n, m, far, _, _, _ in orbs), orbs
    assert len({(n, m) for n, m, _, _, _, _ in orbs}) >= 3, "살·고리 수가 제각각"
    for n, _, _, angs, rings, _ in orbs:
        a = sorted(angs)
        gaps = [b - x for x, b in zip(a, a[1:])] + [a[0] + 6.2831853 - a[-1]]
        assert max(gaps) < 1.6, ("사방으로", gaps)
        assert max(gaps) / min(gaps) > 1.15, ("고르게 나눈 각도가 아님 (비정형)", gaps)
        for pts in rings:
            assert max(pts) - min(pts) > 0.01, "고리 점마다 흔들림 (반듯한 다각형 아님)"


def test_a_round_web_is_woven_spokes_then_rings_in_both_styles(run):
    for key in ("orb_woven", "orb_woven_pixel"):
        woven = [m for m in run[key] if m]
        pieces = run["orb_pieces" if key == "orb_woven" else "orb_pieces_pixel"]
        assert woven[0] < woven[-1] and woven[-1] == max(woven) == 2 * pieces, (key, woven[-1], pieces)
        assert all(b >= a for a, b in zip(woven, woven[1:])), "조각은 늘기만"
        assert len(set(woven)) >= 5, "한 번에 휙이 아니라 여러 번에 걸쳐"


def test_window_round_webs_follow_their_window_by_offset():
    """🕸️⭕ 창 위 둥근 거미집: 처음에 창 왼쪽 위 기준 자리를 재 두고 (web::window_offset) 그 자리로 따라감 (web::orb_on_window)."""
    follow = RS[RS.index("fn follow_window_webs(") : RS.index("/// 그 자리 모니터의 창")]
    assert "orb_on_window(&own.world.windows, window, t.offset, t.size)" in follow and "t.corner.is_orb()" in follow
    place = RS[RS.index("fn place_web(") : RS.index("fn web_lines(")]
    assert "window_offset(windows, w)" in place and "offset," in place
    assert "Corner::Center => (w.x, w.y)," in RS, "둥근 거미집 모니터는 가운데로 봄"
    assert "pub fn orb_on_window(" in CORE_WEB and "pub fn window_offset(" in CORE_WEB and "Center," in CORE_WEB


def test_giants_can_be_yanked_whirled_and_tossed_by_the_app():
    """💪 거미인간이 거대한 것도 끌어오고 휘둘러 던짐 (spec 7부): 앱의 Yank·Release·carry_at이 거대한 것을 막지 않고,
    끝을 넘은 거대한 것은 look "giant"로 친구에게 날아감 (사라지지 않음 — 넘길 친구가 있을 때만 끝이 열림)."""
    yank = OBJ[OBJ.index("Some(ObjectPlay::Yank") :][:300]
    assert "is_giant" not in yank and "l.object.launch(vx, vy)" in yank
    release = OBJ[OBJ.index("Some(JuggleEvent::Release") :][:300]
    assert "is_giant" not in release and "l.object.launch(vx, vy)" in release
    carry = OBJ[OBJ.index("let Some(id) = pet.carried_object()") :][:300]
    assert "is_giant" not in carry and "l.object.carry_at(x, y)" in carry
    toss = OBJ[OBJ.index("fn toss_away(") :][:2000]
    assert "is_giant" not in toss and "look: l.object.kind.look()" in toss
    pull = (ROOT / "crates" / "screenpet-core" / "src" / "brain_web.rs").read_text(encoding="utf-8")
    targets = pull[pull.index("fn pull_targets(") :][:900]
    assert "!s.giant" not in targets


def test_musoe_and_spider_pair_speech_defaults():
    """🛡️🕷️ 무쇠인간·거미인간 (코어 social_mentor.rs — Speech::Cheer·Shucks·KidChat·UhHuh·NotNow): pet.json에 없을 때의 기본 말 (ui/main.js EXTRA_SPEECH)."""
    main = (UI / "main.js").read_text(encoding="utf-8")
    brain = (UI.parent / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
    enum = brain[brain.index("pub enum Speech {") :]
    enum = enum[: enum.index("\n}")]
    extra = main[main.index("const EXTRA_SPEECH = {") :]
    extra = extra[: extra.index("\n};")]
    for v, key, line in (("Cheer", "cheer", "You got this, kid!"), ("Shucks", "shucks", "aw, shucks..."), ("KidChat", "kid_chat", "Stopped a bike thief!"),
                         ("UhHuh", "uh_huh", "Uh-huh."), ("NotNow", "not_now", "Not now, kid.")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf'\b{key}: \[[^\]]*"{re.escape(line)}"', extra), key
