"""💤 밤새 잠금 → 풀면 펫이 안 보이던 버그 (Windows, 2026-11-26): WebView2가 긴 잠금·절전·모니터 끊김 뒤 투명 창 그림을 잃는다.
잠금 풀기는 OS 알림(WTS)으로만, 깸은 조금 기다렸다가, 모니터 바뀜은 드물게. 다시 읽은 페이지는 준비되면 모습을 다시 받고 모아 둔 말풍선을 받는다
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


def test_core_watches_unlock_sleep_and_monitor_changes_but_not_idle():
    wake = (CORE / "wake.rs").read_text(encoding="utf-8")
    assert "pub mod wake;" in (CORE / "lib.rs").read_text(encoding="utf-8")
    for reason in ("Slept", "Unlocked", "Display"):
        assert reason in wake
    for k in ("SLEEP_GAP_SECS", "UNLOCK_DELAY_SECS", "SLEEP_SETTLE_SECS", "DISPLAY_SETTLE_SECS", "DISPLAY_MIN_GAP_SECS", "DEDUPE_SECS"):
        assert k in wake, k
    # 그냥 오래 비운 것(유휴 시간)으로는 새로 고치지 않음 — 다시 읽으면 웹뷰가 들고 있던 것이 사라질 수 있음
    assert "idle" not in wake[wake.index("pub fn observe("):].split("\n")[0]
    assert "AWAY_AFTER_SECS" not in wake


def test_windows_tells_us_about_lock_unlock_and_resume():
    win = (CORE / "platform" / "win32.rs").read_text(encoding="utf-8")
    watch = win[win.index("mod session_watch {"):]
    assert "WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)" in watch
    for msg in ("WTS_SESSION_LOCK", "WTS_SESSION_UNLOCK", "WM_POWERBROADCAST", "PBT_APMRESUMEAUTOMATIC"):
        assert msg in watch, msg
    assert "fn session_events(&mut self) -> crate::wake::SessionEvents" in win
    cargo = (ROOT / "crates" / "screenpet-core" / "Cargo.toml").read_text(encoding="utf-8")
    assert '"Win32_System_RemoteDesktop"' in cargo


def test_the_game_loop_refreshes_after_waking():
    sample = LOOP[LOOP.index("if let Some((secs, idle)) = presence.take_sample() {"):]
    sample = sample[: sample.index("// 손님도 이 화면 주인이")]
    assert "wake.observe(secs, wall, events, &monitors_now(&app))" in sample and "source.session_events()" in sample
    assert "refresh_windows(" in sample and "crate::pages::tick(&app, now);" in sample
    # 모니터는 테두리 전체·배율 (작업 영역이 아님 — 메뉴 막대·작업 표시줄 숨김으로 새로 고치지 않음)
    mon = _fn(LOOP, "fn monitors_now(")
    assert "m.scale_factor()" in mon and "work_area" not in mon


def test_refresh_reloads_and_reshows_every_floating_window():
    refresh = _fn(LOOP, "fn refresh_windows(")
    assert "s.reload_view();" in refresh
    assert "hw.reload()" in refresh and "house::show_in_corner(app)" in refresh and "windows_hidden(app)" in refresh
    assert "crate::pages::begin(house::LABEL);" in refresh
    assert '"prop-"' in refresh and '"hand-"' in refresh
    for part in ("objects.reload_windows()", "vehicles.reload_windows()", "weather.reload_windows()", "trail.reload_windows()", "sleigh.reload_windows()"):
        assert part in refresh, part
    # 눈사람(prop-snow-)은 prop- 창으로 한 번만 읽음
    weather = (SRC / "weather.rs").read_text(encoding="utf-8")
    reload = weather[weather.index("pub fn reload_windows(&mut self)"):]
    assert "snowman" not in reload[: reload.index("\n    }\n")]
    resend = _fn(LOOP, "fn resend_to(")
    for part in ("forget_sent();", "objects.resend(app, label);", "vehicles.resend(label);", "weather.resend(label);", "trail.resend(label, Instant::now());", "sleigh.resend();"):
        assert part in resend, part


def test_pages_pull_their_state_when_they_are_ready():
    """다시 읽은 페이지가 1.5초 뒤 한 번 보낸 모습을 놓치던 것: 페이지가 듣기를 마치면 page_ready → 그 창의 모습을 다시 보냄"""
    pages = (SRC / "pages.rs").read_text(encoding="utf-8")
    assert "pub fn page_ready(" in pages and "Input::PageReady(label)" in pages
    assert "RESEND_AFTER" not in LOOP and "resend_at" not in LOOP
    assert "Input::PageReady(label) =>" in LOOP
    ready = (ROOT / "ui" / "ready.js").read_text(encoding="utf-8")
    assert 'invoke("page_ready")' in ready and "Promise.allSettled(window.spListening)" in ready
    for page in ("index", "house", "object", "vehicle", "weather", "trail", "sleigh"):
        html = (ROOT / "ui" / f"{page}.html").read_text(encoding="utf-8")
        assert '<script src="ready.js"></script>' in html, page
    for js in ("main", "house", "common", "object", "vehicle", "sleigh", "weather", "trail", "web", "doodle", "batsignal", "magic", "snitch"):
        text = (ROOT / "ui" / f"{js}.js").read_text(encoding="utf-8")
        assert "window.spOn ||" in text, js
    caps = (ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8")
    assert '"allow-page-ready"' in caps
    # 포스트잇도 다시 보냄
    objects = (SRC / "objects.rs").read_text(encoding="utf-8")
    resend = objects[objects.index("pub fn resend(&mut self, app: &AppHandle, label: &str)"):]
    assert '"object-sticker"' in resend[: resend.index("\n    }\n")]


def test_one_off_notices_wait_for_a_reloaded_page():
    """다시 읽는 동안 보낸 말풍선·💌 답장·손 흔들기가 사라지던 것: 페이지가 준비될 때까지 모았다가 보냄"""
    for name in ("game_loop.rs", "network.rs", "presence.rs", "update.rs", "ladder.rs", "dodge.rs"):
        text = (SRC / name).read_text(encoding="utf-8")
        for ev in ('"pet-say"', '"pet-say-text"', '"house-wave"', '"food-picker"', '"pet-mail"'):
            for line in text.splitlines():
                if ev in line and "emit_to(" in line:
                    raise AssertionError(f"{name}: {line.strip()}")
    pages = (SRC / "pages.rs").read_text(encoding="utf-8")
    assert "READY_WAIT" in pages and "pub fn tick(" in pages


def test_an_unread_reply_survives_a_reload():
    """💌 답장은 한 번 pet-mail로만 가서 다시 읽은 페이지에선 사라졌음: slot_info의 mail로 다시 받고, 읽으면 지움"""
    assert "pub mail: Option<crate::network::PetMail>," in SLOTS
    net = (SRC / "network.rs").read_text(encoding="utf-8")
    assert "info.mail = Some(mail.clone());" in net
    note = LOOP[LOOP.index("Input::NoteSpace { label, open } =>"):]
    assert "info.mail = None;" in note[: note.index("Input::Net(")]
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    assert "if (stage.mail) takeMail(stage.mail);" in js
    assert "SlotInfo { mail, ..info.clone() }" in SLOTS


def test_a_reloaded_pet_window_is_hidden_then_shown_quietly_in_place():
    reload = SLOTS[SLOTS.index("pub fn reload_view(&mut self)"):]
    reload = reload[: reload.index("\n    }\n")]
    # 숨김·보임은 overlay를 거침 (포커스를 뺏지 않음) — 다음 place가 화면 위에 있어야 할 때만 다시 보임
    assert "crate::pages::begin(&self.label);" in reload and "self.window.reload()" in reload and "crate::overlay::hide_quietly(&self.window);" in reload
    assert "self.need_show = true;" in reload and "self.forget_sent();" in reload and "set_note_open(false)" in reload
    assert ".show()" not in reload
