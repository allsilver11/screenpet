"""📦 물건 창은 게임 루프가 만들지 않는다 (렉 고침).

창 만들기(WebviewWindowBuilder::build)는 메인 스레드가 웹뷰를 다 띄울 때까지 부른 스레드를 막는다 (Windows WebView2 0.1–0.3초).
친구가 "쓰레기 10개 던지기"를 하면 받는 쪽 게임 루프가 열 번 멈춰 1–3초 뚝뚝 끊겼다. 그래서 물건은 바로 들여 움직이고
창은 일꾼 스레드(object-windows)가 하나씩 만들어 돌려보내면 다음 틱에 붙인다. 앱은 이 컴퓨터에서 빌드할 수 없어 소스로 본다.
"""
import re
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "src-tauri" / "src"
OBJ = (SRC / "objects.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")


def _fn(name, src=OBJ):
    """이름의 함수 몸 (다음 같은 깊이의 닫는 괄호까지)"""
    start = src.index(f"fn {name}(")
    indent = src.rfind("\n", 0, start) + 1
    pad = src[indent:start].replace("pub ", "").replace("async ", "")
    end = src.index(f"\n{pad}}}\n", start)
    return src[start:end]


def test_build_window_is_called_only_by_the_worker():
    calls = [m.start() for m in re.finditer(r"\bbuild_window\(", OBJ)]
    # 정의 하나 + 일꾼(make_window) 하나뿐
    assert len(calls) == 2, calls
    assert "fn build_window(" in OBJ
    assert "build_window(" in _fn("make_window")
    # 일꾼 스레드에서만 make_window를 부름
    assert OBJ.count("make_window(") == 2
    assert "make_window(&app, order)" in _fn("start_window_worker")
    assert 'name("object-windows".into())' in _fn("start_window_worker")


def test_game_loop_paths_never_build_or_wait_for_object_windows():
    for name in ("spawn_own", "spawn_snowball", "spawn_carried", "deliver_toy", "admit_next_toss", "admit_poop", "admit", "tick", "attach_built"):
        body = _fn(name)
        assert "build_window(" not in body, name
        assert ".build()" not in body and "finish_quiet(" not in body, name
        # 창 배율은 창에 묻지 않고 (메인 스레드 왕복) 내 동물 배율로 시작
        assert "window.scale_factor()" not in body, name
    for name in ("spawn_own", "spawn_snowball", "spawn_carried", "deliver_toy", "admit_next_toss", "admit_poop"):
        body = _fn(name)
        assert "self.dpi_hint" in body, name
        assert "self.admit(app, id, object," in body, name
    # 결과는 기다리지 않고 온 것만 (try_recv)
    attach = _fn("attach_built")
    assert "try_recv()" in attach and ".recv()" not in attach
    assert "self.windows = None;" in attach


def test_admit_orders_a_window_and_starts_without_one():
    admit = _fn("admit")
    assert "self.order_window(app, order)" in admit
    assert "window: None," in admit and "hwnd: None," in admit
    assert "label_of(id)" in admit
    assert "window: Option<WebviewWindow>," in OBJ


def test_incoming_queue_waits_for_the_worker_one_at_a_time():
    body = _fn("admit_next_toss")
    guard = body.index("w.waiting > 0")
    assert guard < body.index("self.poop_queue.pop_front()") < body.index("self.inbox.pop()")
    # 주문을 넣으면 세고, 돌아오면 뺌
    assert "worker.waiting += 1;" in _fn("order_window")
    assert "worker.waiting.saturating_sub(1)" in _fn("attach_built")


def test_tick_attaches_arrived_windows_before_anything_else():
    tick = _fn("tick")
    assert tick.index("self.attach_built(app);") < tick.index("self.admit_next_toss(app, base);")
    assert tick.index("self.attach_built(app);") < tick.index("if self.live.is_empty()")


def test_a_window_for_an_object_that_is_already_gone_is_destroyed():
    attach = _fn("attach_built")
    gone = attach[attach.index("let Some(i) = self.live.iter().position(|l| l.id == built.id) else {") :]
    gone = gone[: gone.index("continue;")]
    assert "w.destroy()" in gone
    # 창을 못 만들었으면 물건도 치움 (예전처럼 생기지 않은 것과 같게)
    assert "None => self.drop_at(i)" in attach


def test_a_pending_object_without_a_window_is_handled_everywhere():
    # 창이 없으면 옮기기·크기·클릭 통과·쪽지 펼치기는 건너뜀
    for name in ("place", "set_scale", "sync_click_through", "set_note_open"):
        assert "let Some(window) = &self.window else { return };" in _fn(name), name
    # 치우기: 뿅·스르륵은 창이 있을 때만, 넘기기·던지기는 있으면 닫음
    for name in ("poof", "fade"):
        assert "if let Some(w) = l.window {" in _fn(name), name
    assert "if let Some(w) = &l.window {" in _fn("hand_over")
    assert "if let Some(w) = &l.window {" in _fn("tick")
    assert "l.window.destroy()" not in OBJ
    assert "self.fading.start(l.window," not in OBJ
    # 배율 다시 읽기는 창이 있는 것만
    assert "l.window.as_ref().and_then(|w| w.scale_factor().ok())" in _fn("refresh")
    # 모습은 창이 올 때까지 보내지 않음 (움직임·부딪힘·걸림은 그대로)
    tick = _fn("tick")
    loop = tick[tick.index("for (i, l) in self.live.iter_mut().enumerate()") :]
    assert loop.index("l.object.tick(") < loop.index("if l.window.is_none() {") < loop.index('"object-pose"')


def test_attach_restores_click_through_size_and_pose_while_off_screen():
    attach = _fn("attach")
    assert "set_click_through(&window, true)" in attach
    assert "crate::windows_hidden(app)" in attach and "hide_quietly(&window)" in attach
    assert "resize_quietly(&window" in attach and '"object-scale"' in attach
    assert "self.window = Some(window);" in attach and "self.last_pos = None;" in attach
    assert "f64::NAN" in attach


def test_worker_death_drops_windowless_objects():
    assert "fn drop_windowless(" in OBJ
    assert "self.drop_windowless();" in _fn("attach_built")
    assert "self.drop_windowless();" in _fn("order_window")


def test_game_loop_gives_the_dpi_hint():
    assert "pub fn set_dpi_hint(&mut self, dpi: f64)" in OBJ
    assert LOOP.count("objects.set_dpi_hint(slots[OWN].dpi);") == 2
    i = LOOP.index("objects.refresh(&windows, &work_areas, now);")
    assert "objects.set_dpi_hint(slots[OWN].dpi);" in LOOP[i - 120 : i]
