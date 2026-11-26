"""🌗 루모스·녹스로 테마를 바꾸거나 되돌릴 때 모든 모니터의 작업표시줄이 같이 바뀜 (2026-11-26 "되돌아올 때 한 모니터만 돌아오고
다른 모니터는 한참 나중에 돌아와"): 모든 창에 차례로 보내는 알림(멈춘 창마다 기다림)보다 먼저, 주 작업표시줄(Shell_TrayWnd)과
보조 모니터 작업표시줄(Shell_SecondaryTrayWnd) 모두에 직접 알림."""
from pathlib import Path

WIN = (Path(__file__).resolve().parent.parent / "crates" / "screenpet-core" / "src" / "platform" / "win32.rs").read_text(encoding="utf-8")


def test_every_taskbar_is_told_before_the_broadcast():
    body = WIN[WIN.index("fn write(&mut self, theme: crate::lumos::Theme) -> bool {"):]
    body = body[: body.index("\n    }\n")]
    assert "notify_taskbars" in body
    assert body.index("notify_taskbars(") < body.index("SendMessageTimeoutW(HWND_BROADCAST")
    fn = WIN[WIN.index("fn notify_taskbars("):]
    fn = fn[: fn.index("\n}\n")]
    assert "Shell_TrayWnd" in fn and "Shell_SecondaryTrayWnd" in fn and "FindWindowExW" in fn
