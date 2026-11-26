"""💤 밤새 잠금 → 풀면 펫이 안 보이던 버그 (Windows, 2026-11-26): WebView2가 긴 잠금·절전·모니터 끊김 뒤 투명 창 그림을 잃는다
(창은 "보임"이라 1초마다의 다시 보이기 검사(overlay::is_shown)로는 못 잡음). 코어 wake.rs가 잤다 깼음·오래 비웠다 돌아옴·모니터 바뀜을 알려 주면
게임 루프가 떠 있는 창을 모두 새로 고친다 (웹뷰 다시 읽기 + 다시 보이기 + 잠시 뒤 모습 다시 보내기). 트레이 🔄 새로고침도 같은 길."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
SLOTS = (SRC / "slots.rs").read_text(encoding="utf-8")
CORE = (ROOT / "crates" / "screenpet-core" / "src")


def _fn(src, head):
    body = src[src.index(head):]
    return body[: body.index("\n}\n")]


def test_core_watches_sleep_long_away_and_monitor_changes():
    wake = (CORE / "wake.rs").read_text(encoding="utf-8")
    assert "pub mod wake;" in (CORE / "lib.rs").read_text(encoding="utf-8")
    for reason in ("Slept", "Back", "Display"):
        assert reason in wake
    assert "SLEEP_GAP_SECS" in wake and "DISPLAY_SETTLE_SECS" in wake and "AWAY_AFTER_SECS" in wake


def test_the_game_loop_refreshes_after_waking():
    sample = LOOP[LOOP.index("if let Some((secs, idle)) = presence.take_sample() {"):]
    sample = sample[: sample.index("// 손님도 이 화면 주인이")]
    assert "wake.observe(wall, idle, &slots[OWN].world.work_areas)" in sample
    assert "refresh_windows(" in sample and "resend_at = Some(now + RESEND_AFTER);" in sample
    assert "resend_all(" in sample


def test_refresh_reloads_and_reshows_every_floating_window():
    refresh = _fn(LOOP, "fn refresh_windows(")
    assert "s.reload_view();" in refresh
    assert "hw.reload()" in refresh and "house::show_in_corner(app)" in refresh and "windows_hidden(app)" in refresh
    assert '"prop-"' in refresh and '"hand-"' in refresh
    for part in ("objects.reload_windows()", "vehicles.reload_windows()", "weather.reload_windows()"):
        assert part in refresh, part
    resend = _fn(LOOP, "fn resend_all(")
    for part in ("s.forget_sent();", "objects.resend(app);", "vehicles.resend();", "weather.resend();"):
        assert part in resend, part


def test_a_reloaded_pet_window_is_hidden_then_shown_quietly_in_place():
    reload = SLOTS[SLOTS.index("pub fn reload_view(&mut self)"):]
    reload = reload[: reload.index("\n    }\n")]
    # 숨김·보임은 overlay를 거침 (포커스를 뺏지 않음) — 다음 place가 화면 위에 있어야 할 때만 다시 보임
    assert "self.window.reload()" in reload and "crate::overlay::hide_quietly(&self.window);" in reload
    assert "self.need_show = true;" in reload and "self.forget_sent();" in reload and "set_note_open(false)" in reload
    assert ".show()" not in reload
