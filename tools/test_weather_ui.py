"""계절 날씨 창의 약속 — 사용자는 CPU에 민감하다: 마우스 통과, 낮은 fps, 적은 알갱이, 권한 등록."""

import json
import re
import shutil
from pathlib import Path

import pytest
from conftest import run_node

ROOT = Path(__file__).resolve().parent.parent
JS = (ROOT / "ui" / "weather.js").read_text(encoding="utf-8")
RS = (ROOT / "src-tauri" / "src" / "weather.rs").read_text(encoding="utf-8")


def const(name):
    return int(re.search(rf"const {name} = (\d+);", JS).group(1))


def test_frame_rate_and_particle_budget():
    assert const("FPS") <= 30
    assert const("MAX_PARTICLES") <= 80
    assert const("MIN_PARTICLES") >= 30
    counts = [int(c) for c in re.findall(r"count: (\d+)", JS)]
    assert len(counts) == 9 and all(30 <= c <= 80 for c in counts)
    # 날씨마다 지난번과 같은 예산 (비 70개가 가장 많다)
    assert max(counts) <= 70


def test_the_overlay_never_takes_a_click():
    assert RS.count("set_click_through(&window, true)") == 1  # 눈덩이·눈사람 창 (날씨 창은 overlay::screen_cover)
    assert RS.count("overlay::finish(") == 1
    overlay = (ROOT / "src-tauri" / "src" / "overlay.rs").read_text(encoding="utf-8")
    cover = overlay[overlay.index("pub fn screen_cover("):]
    assert "set_click_through(&window, true)" in cover[: cover.index("\n}\n")]
    assert 'overlay::screen_cover(app, &label, "weather.html"' in RS
    assert ".focused(false)" in (ROOT / "src-tauri" / "src" / "overlay.rs").read_text(encoding="utf-8")
    assert "pointer-events: none" in (ROOT / "ui" / "weather.html").read_text(encoding="utf-8")


def test_nothing_ticks_while_paused():
    # 멈추면 타이머를 끊고, 다음 장면이 올 때까지 다시 걸지 않는다
    stop = JS[JS.index("function stop()"):JS.index("function hash(")]
    assert "clearTimeout(timer)" in stop and "running = false" in stop
    assert "if (!running) return;" in JS
    assert "setInterval" not in JS


def test_the_overlay_may_listen_for_scenes():
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text())
    assert "weather-*" in {w for cap in caps["capabilities"] for w in cap["windows"]}


def test_preview_is_not_in_the_menu():
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert "날씨 효과" in tray
    assert "SCREENPET_WEATHER" not in tray and "preview" not in tray


def test_the_menu_is_open_all_year():
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert '"🌨️ 날씨 효과", true,' in tray
    assert "set_enabled" not in RS
    assert "on_offer" not in RS


def test_the_app_follows_the_clock_and_shuts_the_overlay_between_showers():
    # 달력·실제 날씨는 1분에 한 번 (실제 날씨가 없거나 오래면 core가 weather_at으로) — 맑을 때 None이면 tick이 창을 모두 닫는다 (tear_down)
    assert "real_weather::weather_now(" in RS and "num_seconds_from_midnight()" in RS
    assert "const SEASON_EVERY: Duration = Duration::from_secs(60);" in RS
    tick = RS[RS.index("pub fn tick("):RS.index("/// 날씨가 켜진 동안 5 Hz")]
    assert "self.tear_down(slots)" in tick and "self.next_scene = now + IDLE_EVERY" in tick


def test_real_weather_comes_from_the_relay_and_falls_back_to_the_calendar():
    # ☀️ 앱은 날씨 서비스를 부르지 않는다: 중계 서버가 준 값(NetEvent::Weather)을 core가 그림으로 바꾼다
    assert "real_weather::weather_now(self.source, self.reading," in RS
    assert "ureq" not in RS and "open-meteo" not in RS.lower()
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    assert "NetEvent::Weather { code, at }" in game and "weather.set_reading(Some(Reading { code, at }))" in game
    net = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
    assert "net.want_weather(place.id())" in net
    # 새 값·설정이 오면 1분을 기다리지 않고 바로 다시 본다
    for fn in ("pub fn set_source(", "pub fn set_reading("):
        body = RS[RS.index(fn):]
        body = body[: body.index("\n    }\n")]
        assert "self.next_season = Instant::now()" in body, fn


def test_pets_look_up_at_whatever_the_core_says():
    assert "kind.looks_up()" in RS


def test_help_tells_the_whole_year():
    help_text = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
    line = next(l for l in help_text.splitlines() if "날씨 효과" in l)
    for word in ["벚꽃잎", "소나기", "낙엽", "눈", "비"]:
        assert word in line
    real = next(l for l in help_text.splitlines() if "실제 날씨" in l)
    for word in ["용인", "📍 날씨 지역", "📅 계절 날씨", "3시간"]:
        assert word in real, word


# ---- ui/weather.js를 가짜 캔버스로 돌려 보기 (tools/weather_harness.js) — 호출 수만, 실제 렌더링 비용은 아니다 ----
@pytest.fixture(scope="module", params=["light_snow", "snow", "heavy_snow", "rain", "shower", "petals", "leaves", "green_leaves", "typhoon"])
def run(request):
    if not shutil.which("node"):
        pytest.skip("node not installed")
    out = run_node(ROOT / "tools" / "weather_harness.js", request.param, timeout=60)
    return json.loads(out.stdout)


def test_each_frame_stays_within_budget(run):
    # 지우기 수 = 알갱이 수 (화면 전체를 지우지 않는다). 그리기는 아직 화면 위에 있는 알갱이만큼 더 적을 수 있다
    assert 30 <= run["maxClears"] <= 80
    assert run["maxDraws"] <= run["maxClears"]
    assert run["minDelay"] >= 1000 / 30


def test_particles_stop_at_the_first_top(run):
    # 화면 전체 폭의 윗면이 있으면 아무것도 그 아래로 지나가지 않는다 (옆으로 흘러도 2~3 px 안)
    assert run["deepest"] <= run["lineY"] + 4, run["deepest"]


def test_piles_are_not_repainted_without_a_change(run):
    assert run["repaintedPiles"] == 0


def test_pausing_stops_all_work_and_resuming_restarts(run):
    assert run["pendingAfterPause"] == 0 and run["timersAfterRaf"] == 0
    assert run["drawsWhilePaused"] == 0
    assert run["drawsAfterResume"] > 0


def test_pause_and_resume_never_stack_a_second_frame_loop(run):
    assert run["loopsAfterPauseRace"] == 1


def test_piles_come_back_after_a_resize(run):
    wet = run["kind"] in ("rain", "shower", "typhoon")
    assert run["pilesAfterResize"] == (0 if wet else 1)
    assert run["pilesAfterPausedResize"] == 0


def test_a_flake_or_drop_stays_the_same_and_petals_and_leaves_tumble(run):
    if run["kind"] in ("petals", "leaves", "green_leaves"):
        assert run["tumbleSprites"] >= 4, "turns as it falls"
        assert run["tumbleColours"] == 1, "a leaf keeps its colour while it tumbles"
    else:
        assert run["tumbleSprites"] == 1


def test_leaves_are_warm_red_orange_and_yellow(run):
    if run["kind"] != "leaves":
        pytest.skip("leaves only")
    assert len(run["colours"]) == 3
    for colour in run["colours"]:
        r, g, b = (int(colour[i:i + 2], 16) for i in (1, 3, 5))
        assert r > g > b, colour
    greens = sorted(int(c[3:5], 16) for c in run["colours"])
    assert greens[2] - greens[0] > 80, "from red through to yellow"


def test_petal_and_leaf_piles_are_made_of_their_own_sprites(run):
    if run["kind"] in ("petals", "leaves", "green_leaves"):
        assert run["pileImages"] > 0
    else:
        assert run["pileImages"] == 0


def test_a_petal_or_leaf_pile_keeps_its_look_while_rain_melts_it(run):
    if run["kind"] in ("petals", "leaves", "green_leaves"):
        assert run["meltingPileImages"] > 0
    else:
        assert run["meltingPileImages"] == 0


def test_green_leaves_are_fresh_bright_greens(run):
    if run["kind"] != "green_leaves":
        pytest.skip("green leaves only")
    assert len(run["colours"]) == 3
    for colour in run["colours"]:
        r, g, b = (int(colour[i:i + 2], 16) for i in (1, 3, 5))
        assert g > r and g > b and g >= 0xb0, colour


def test_green_leaves_flutter_livelier_than_autumn_leaves():
    def spec(kind):
        line = next(l for l in JS.splitlines() if l.strip().startswith(f"{kind}:"))
        return {k: float(v) for k, v in re.findall(r"(sway|flutter): ([\d.]+)", line)}
    green, autumn = spec("green_leaves"), spec("leaves")
    assert green["sway"] > autumn["sway"]
    assert green["flutter"] > autumn.get("flutter", 1.6)
    assert "spec.flutter" in JS


def kind_spec(kind):
    """KINDS 표의 한 줄 → {count, box, sway, ...}와 fall·wind 범위."""
    line = next(l for l in JS.splitlines() if l.strip().startswith(f"{kind}:"))
    spec = {k: float(v) for k, v in re.findall(r"(\w+): (-?[\d.]+)[,\s}]", line)}
    for k, lo, hi in re.findall(r"(\w+): \[(-?[\d.]+), (-?[\d.]+)\]", line):
        spec[k] = (float(lo), float(hi))
    return spec


def test_heavy_snow_is_big_plentiful_flakes_drifting_down_slowly():
    heavy, snow = kind_spec("heavy_snow"), kind_spec("snow")
    assert heavy["count"] > snow["count"], "펑펑"
    assert heavy["box"] > snow["box"], "큰 눈송이"
    assert heavy["fall"][1] < snow["fall"][1], "천천히"
    assert heavy["sway"] >= snow["sway"], "살랑살랑 흩날림"
    px_box = dict(re.findall(r"(\w+): (\d+)", JS[JS.index("const PX_BOX"):].split("\n")[0]))
    assert int(px_box["heavy_snow"]) > int(px_box["snow"]), "픽셀도 큼"
    assert "PX_HEAVY_FLAKES" in JS


def test_heavy_snow_draws_bigger_flakes_than_snow_and_piles_like_snow(run):
    if run["kind"] != "heavy_snow":
        pytest.skip("heavy snow only")
    snow = json.loads(run_node(ROOT / "tools" / "weather_harness.js", "snow", timeout=60).stdout)
    assert run["spriteBox"] > snow["spriteBox"]
    assert run["particleCount"] > snow["particleCount"]
    assert run["flurryBox"] > snow["flurryBox"], "쓸려 와도 큰 눈송이"
    assert run["pileImages"] == 0 and run["pileFills"] > 0, "눈 더미 그대로"


def test_only_rain_showers_and_typhoons_splash(run):
    assert run["hasSplash"] == (run["kind"] in ("rain", "shower", "typhoon"))


def test_a_typhoon_draws_its_tornado_only_while_the_scene_has_one(run):
    if run["kind"] != "typhoon":
        assert run["funnelRings"] == 0 and run["funnelAfterGone"] == 0
        return
    assert run["funnelRings"] >= 24, "구름 뭉치들 + 바닥 먼지"
    assert run["funnelAfterGone"] == 0, "회오리가 가면 지움"


def test_the_whole_tornado_fits_in_the_box_cleared_next_frame(run):
    """지울 칸보다 삐져나온 그림은 다음 프레임에 안 지워져 회색 줄로 남음 (돌풍이 지나간 자리 잔상, v0.6.8).
    하네스가 칸을 셀 수 없는 그리기(호를 긋기·채우기 등)도 밖으로 친다."""
    if run["kind"] != "typhoon":
        return
    assert run["funnelOutside"] == 0


@pytest.fixture(scope="module")
def pixel_typhoon():
    if not shutil.which("node"):
        pytest.skip("node not installed")
    return json.loads(run_node(ROOT / "tools" / "weather_harness.js", "typhoon", "pixel", timeout=60).stdout)


@pytest.fixture(scope="module")
def typhoon():
    if not shutil.which("node"):
        pytest.skip("node not installed")
    return json.loads(run_node(ROOT / "tools" / "weather_harness.js", "typhoon", timeout=60).stdout)


@pytest.mark.parametrize("style", ["typhoon", "pixel_typhoon"])
def test_the_tornado_is_a_swirl_of_see_through_storm_cloud_puffs(style, request):
    """동그라미 줄 대신: 반투명 먹구름 뭉치(미리 그린 그림)들이 축을 돌며, 위가 넓고 바닥으로 좁아짐."""
    r = request.getfixturevalue(style)
    assert r["funnelRings"] >= 24
    assert r["funnelSpriteCount"] >= 3, "뭉치 그림이 여러 가지"
    lo, hi = r["funnelAlphas"]
    assert 0.2 <= lo and hi < 1, "반투명"
    # 아래 1/3엔 바닥 흙먼지도 들어 있음
    assert r["funnelTopWidth"] > 1.6 * r["funnelBottomWidth"], (r["funnelTopWidth"], r["funnelBottomWidth"])
    assert r["funnelMoved"] > 0.4, "제자리에서도 빙글빙글 돎 (픽셀은 칸에 맞춰 덜 자주 움직임)"
    assert r["funnelOutside"] == 0


@pytest.mark.parametrize("style", ["typhoon", "pixel_typhoon"])
def test_the_tornado_stays_within_the_reach_the_core_uses_for_straddling(style, request):
    """core typhoon.rs straddles()의 reach = FUNNEL_R.1 + 30 — 그보다 넓으면 옆 모니터에 걸칠 때 잘림."""
    r = request.getfixturevalue(style)
    core = (ROOT / "crates" / "screenpet-core" / "src" / "typhoon.rs").read_text(encoding="utf-8")
    top_r = float(re.search(r"const FUNNEL_R: \(f64, f64\) = \([\d.]+, ([\d.]+)\);", core).group(1))
    assert re.search(rf"const FUNNEL_R = \[\d+, {int(top_r)}\];", JS)
    assert "(FUNNEL_R.1 + 30.0) * scale" in core
    assert 0 < r["funnelReach"] <= top_r + 30, r["funnelReach"]


def test_smooth_puffs_are_soft_gradients_and_pixel_puffs_are_stepped_cells(typhoon, pixel_typhoon):
    assert "createRadialGradient" in typhoon["funnelSpriteOps"]
    assert pixel_typhoon["funnelSpriteOps"] and set(pixel_typhoon["funnelSpriteOps"]) <= {"fillRect", "set:fillStyle"}, pixel_typhoon["funnelSpriteOps"]
    assert pixel_typhoon["funnelOnGrid"], "픽셀: 칸에 맞춰, 늘리지 않고 찍음"


def test_the_tornado_puffs_are_drawn_once_and_reused():
    """뭉치 그림은 처음 한 번만 만들고 (프레임마다 흐림 필터·새 캔버스 없음)."""
    body = JS[JS.index("function drawFunnel("):]
    body = body[: body.index("\n}\n")]
    assert "createElement" not in body and "filter" not in body and "createRadialGradient" not in body


def test_typhoon_debris_is_green_leaves_flying_with_the_wind(run):
    if run["kind"] != "typhoon":
        assert run["debrisColours"] == []
        return
    assert len(run["debrisColours"]) == 3
    # 바람이 왼쪽(−)이면 왼쪽으로, 돌풍이 바뀌면 오른쪽으로 — 옆으로 세게
    assert run["flowNow"] < -5 and run["flowFlipped"] > 5, (run["flowNow"], run["flowFlipped"])


def test_typhoon_is_wired_from_core_to_overlay_and_pets():
    core = (ROOT / "crates" / "screenpet-core" / "src" / "weather.rs").read_text(encoding="utf-8")
    assert 'Weather::Typhoon => "typhoon"' in core
    # 바람·회오리는 태풍 장면에만, 회오리는 그 모니터 창에만
    assert "kind.is_stormy()" in RS and "t.area() == o.wa" in RS
    storm = RS[RS.index("pub fn storm("):]
    storm = storm[: storm.index("\n    }\n")]
    for call in ("s.pet.storm_free()", "s.pet.whirl_to(x, y)", "s.pet.whirl_release(vx, vy, cross)", "s.pet.feel_gust(gust)", "objects.storm_apply(w)", "objects.gust(gust)"):
        assert call in storm, call
    # 자리 비움·모두 숨김·태풍이 아니면 회오리를 보냄 (도는 것은 코어가 그 자리에서 떨어뜨림)
    assert "self.storm = None;" in storm and "!away && !crate::windows_hidden(app)" in storm
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    assert game.index("weather.storm(&app, now,") < game.index("objects.tick(&app, &slots[OWN].world")
    # 새 창을 만들지 않음 — 깔때기는 날씨 덮개 창에 그림
    assert RS.count("overlay::finish(") == 1


def test_flurries_are_capped_and_fade_away(run):
    assert 0 < run["flurryPeak"] <= 120
    assert run["flurryLeft"] == 0


def test_the_broom_is_in_the_menu_and_flurries_reach_the_overlay():
    tray = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
    assert '"🧹 빗자루"' in tray and "Input::ToggleBroom" in tray
    assert '"weather-flurry"' in RS and 'listen("weather-flurry"' in JS


def test_the_broom_pushes_every_loose_object_not_just_poop():
    """🧹 쓰레기·똥·장난감·눈덩이·거대한 것 모두 밀림 — 거르기는 코어 Object::sweep (들린 것·읽는 포스트잇 등)."""
    game = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    objects = (ROOT / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")
    assert "objects.sweep_objects(b);" in game and "sweep_poops" not in game
    body = objects[objects.index("pub fn sweep_objects("):]
    body = body[: body.index("\n    }\n")]
    assert "is_poop" not in body and "filter" not in body and "l.object.sweep(vx, vy)" in body
    help_text = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
    line = next(l for l in help_text.splitlines() if "<b>빗자루</b>" in l)
    for word in ["쓰레기", "눈덩이", "거대한"]:
        assert word in line, word
