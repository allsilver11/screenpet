"""앱 커맨드 권한: 창이 부르는 커맨드와 이벤트 듣기가 그 창의 capability에 모두 있고, build.rs 목록과 generate_handler가 같다.
권한이 빠지면 앱은 컴파일되고 런타임에 조용히 거절당하므로 여기서 잡는다."""
import fnmatch
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
CAPS = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))["capabilities"]

# 창 라벨(또는 그 꼴의 한 예) → 그 창의 페이지. 새 창을 만들면 여기에 적는다
PAGES = {
    "pet": "index.html",
    "guest-1": "index.html",
    "house": "house.html",
    "pair": "pair.html",
    "code": "code.html",
    "name": "name.html",
    "link": "link.html",
    "size": "size.html",
    "sleigh": "sleigh.html",
    "letter": "letter.html",
    "ladder": "ladder.html",
    "weather-1": "weather.html",
    "prop-gift-1": "gift.html",
    "book": "book.html",
    "timeline": "timeline.html",
    "object-1": "object.html",
    "postit": "postit.html",
    "dodge": "dodge.html",
}
# 음식 고르기는 펫·하우스 창에서만 그린다 (ladder·code 창도 common.js를 읽지만 부르지 않는다)
ONLY_DEFINED_THERE = {"feed": {"pet", "guest-1", "house"}}


def page_source(page):
    html = (UI / page).read_text(encoding="utf-8")
    parts = [html]
    for src in re.findall(r'<script src="([^"]+\.js)"', html):
        parts.append((UI / src).read_text(encoding="utf-8"))
    return "\n".join(parts)


def permissions(label):
    out = set()
    for cap in CAPS:
        if any(fnmatch.fnmatchcase(label, w) for w in cap["windows"]):
            out |= set(cap["permissions"])
    return out


def test_every_window_may_call_what_its_page_calls():
    for label, page in PAGES.items():
        src = page_source(page)
        allowed = permissions(label)
        for cmd in set(re.findall(r'invoke\("([a-z_]+)"', src)):
            if label not in ONLY_DEFINED_THERE.get(cmd, {label}):
                continue
            assert "allow-" + cmd.replace("_", "-") in allowed, f"{label} ({page}) calls {cmd}"
        if "listen(" in src:
            assert {"core:event:allow-listen", "core:event:allow-unlisten"} <= allowed, f"{label} listens"


def test_pages_use_no_tauri_api_beyond_commands_and_listening():
    """창을 다루는 JS API나 이벤트 보내기를 쓰기 시작하면 권한도 같이 늘려야 한다"""
    for page in set(PAGES.values()):
        src = page_source(page)
        apis = set(re.findall(r"__TAURI__\.([a-zA-Z]+)", src))
        assert apis <= {"core", "event", "webviewWindow"}, f"{page}: {apis}"
        assert "data-tauri-drag-region" not in src, page
        assert not re.search(r"\bemit(To)?\(", src), f"{page} emits"
        assert not re.search(r"\bhere\.(?!listen\b)[a-zA-Z]+\(", src), f"{page} uses a window API"


def test_no_window_keeps_the_broad_default_permission():
    for cap in CAPS:
        assert "core:default" not in cap["permissions"], cap["identifier"]


def test_build_manifest_and_handler_list_the_same_commands():
    build = (ROOT / "src-tauri" / "build.rs").read_text(encoding="utf-8")
    block = re.search(r"commands\(&\[(.*?)\]\)", build, re.S).group(1)
    manifest = set(re.findall(r'"([a-z_]+)"', block))
    main = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
    handler = re.search(r"generate_handler!\[(.*?)\]", main, re.S).group(1)
    handled = {name.split("::")[-1].strip() for name in handler.split(",") if name.strip()}
    assert manifest == handled, manifest ^ handled
    granted = {p[len("allow-"):].replace("-", "_") for cap in CAPS for p in cap["permissions"] if p.startswith("allow-")}
    assert granted == handled, granted ^ handled
