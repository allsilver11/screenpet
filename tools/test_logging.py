"""구조화 로그 검사: 모든 `log!` 호출이 정한 수준·영역·사건·키만 쓰고, 개인정보(편지·건의함 글·이름·코드·키·주소 전체·경로)를
넘기지 않으며, 사건마다 docs/architecture/logging.md 목록에 있는지 봄. 릴레이에는 옛 eprintln! 로그가 남지 않아야 함."""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RELAY = ROOT / "crates" / "screenpet-relay" / "src"
NET = ROOT / "crates" / "screenpet-net" / "src"
APP = ROOT / "src-tauri" / "src"
DOC = (ROOT / "docs" / "architecture" / "logging.md").read_text(encoding="utf-8")

LEVELS = {"Info", "Warn", "Error"}
RELAY_AREAS = {"conn", "hub", "presence", "route", "mail", "board", "weather", "dl", "admin", "store"}
APP_AREAS = {"net", "relay", "update", "visit", "toss"}
# 쓸 수 있는 키 (값은 아래 규칙을 따름)
KEYS = {
    "conn", "peer", "ip", "v", "took_ms", "err", "n", "bytes",
    "reason", "ch", "to", "from", "friend", "owner", "via", "old_conn", "old_ip", "new", "friends", "online", "channels",
    "up_s", "what", "op", "id", "page", "changed", "cmd", "ok", "place", "wmo", "file", "head", "limit", "bind", "relay_pub",
    "proxies", "weather", "port", "status", "version", "visit", "side", "with_note", "with_reply", "carried", "kind",
}
# 이 키들의 값은 앞 8자로 줄여야 함
SHORT_KEYS = {"peer", "to", "from", "friend", "owner", "via"}
# 이 키들의 값은 가려야 함
IP_KEYS = {"ip", "old_ip"}
# 값 식에 나오면 안 되는 낱말 (short(…)·mask_ip(…) 안과 `…note.is_some()`은 뺀 뒤 봄)
FORBIDDEN = {
    "name", "names", "line", "lines", "code", "key", "secret", "sealed", "note", "text", "path", "pixels", "icon", "typed",
    "hex", "display", "body", "content", "letter", "nickname", "password", "seed",
}
# 이름이 `name`인 변수지만 고정 모양 파일 이름인 곳 (릴레이 다운로드·편지함 파일, 업데이트 설치 파일)
FILE_NAME_OK = {
    ("dl", "not_found", "name"),
    ("dl", "download_busy", "name"),
    ("dl", "meta", "name"),
    ("dl", "download_start", "name"),
    ("dl", "download_done", "name"),
    ("dl", "download_cut", "name"),
    ("mail", "load_fail", "name"),
    ("update", "read_fail", "release.asset.name.as_str()"),
}


def _split_top(s):
    """괄호·따옴표 밖의 쉼표로 나눔"""
    parts, depth, cur, quote = [], 0, "", False
    i = 0
    while i < len(s):
        c = s[i]
        if quote:
            cur += c
            if c == "\\":
                cur += s[i + 1]
                i += 1
            elif c == '"':
                quote = False
        elif c == '"':
            quote = True
            cur += c
        elif c in "([{":
            depth += 1
            cur += c
        elif c in ")]}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            parts.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        parts.append(cur.strip())
    return parts


def _calls(path):
    """파일의 `log!(…)` 호출들: (줄 번호, 인자 글)"""
    text = path.read_text(encoding="utf-8")
    out = []
    for m in re.finditer(r"\blog!\(", text):
        start = text.rfind("\n", 0, m.start()) + 1
        if text[start : m.start()].lstrip().startswith("//"):
            continue
        i, depth = m.end(), 1
        while depth:
            c = text[i]
            if c == '"':
                i += 1
                while text[i] != '"':
                    i += 2 if text[i] == "\\" else 1
            elif c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
            i += 1
        out.append((text.count("\n", 0, m.start()) + 1, " ".join(text[m.end() : i - 1].split())))
    return out


def _all_calls():
    """(파일, 줄, 수준, 영역, 사건, [(키, 값)], 앱 쪽인지)"""
    found = []
    for base, app in ((RELAY, False), (NET, True), (APP, True)):
        for f in sorted(base.rglob("*.rs")):
            for line, args in _calls(f):
                parts = _split_top(args)
                level, area, event = parts[0], parts[1].strip('"'), parts[2].strip('"')
                fields = []
                for p in parts[3:]:
                    k, v = p.split("=>", 1)
                    fields.append((k.strip().strip('"'), v.strip()))
                found.append((f.relative_to(ROOT), line, level, area, event, fields, app))
    return found


CALLS = _all_calls()


def _strip_safe(expr):
    """가린 값(short·mask_ip 안)과 `note.is_some()`을 빼고 남은 식"""
    expr = re.sub(r"[\w.]*note\.is_some\(\)", "X", expr)
    for fn in ("short_id", "short", "mask_ip"):
        while True:
            m = re.search(r"\b" + fn + r"\(", expr)
            if not m:
                break
            i, depth = m.end(), 1
            while depth:
                depth += {"(": 1, ")": -1}.get(expr[i], 0)
                i += 1
            expr = expr[: m.start()] + "SAFE" + expr[i:]
    return expr


def test_there_are_log_calls_on_both_sides():
    assert sum(1 for c in CALLS if not c[6]) >= 40
    assert sum(1 for c in CALLS if c[6]) >= 30


def test_levels_areas_events_and_keys_are_fixed():
    for f, line, level, area, event, fields, app in CALLS:
        where = f"{f}:{line}"
        assert level in LEVELS, where
        assert area in (APP_AREAS if app else RELAY_AREAS), f"{where}: area {area}"
        assert re.fullmatch(r"[a-z][a-z0-9_]*", event), f"{where}: event {event}"
        for k, _ in fields:
            assert k in KEYS, f"{where}: key {k}"


def test_no_private_values_reach_a_log_line():
    for f, line, level, area, event, fields, app in CALLS:
        where = f"{f}:{line} {area}/{event}"
        for k, v in fields:
            if k in SHORT_KEYS:
                assert "short" in v, f"{where}: {k} must be cut to 8 characters: {v}"
            if k in IP_KEYS:
                assert "mask_ip(" in v, f"{where}: {k} must be masked: {v}"
            if k == "file" and (area, event, v) in FILE_NAME_OK:
                continue
            assert ".display()" not in v, f"{where}: no file paths: {v}"
            words = set(re.findall(r"[a-z_]+", _strip_safe(v).lower()))
            bad = words & FORBIDDEN
            assert not bad, f"{where}: {k} => {v} uses {sorted(bad)}"


def test_the_guard_catches_bad_calls():
    """검사가 실제로 잡는지 (가짜 호출로)"""
    for v in ('l.name', 'letter.body', 'format!("{}", secret)', 'cfg.code', 'path.display()', 'hex'):
        assert set(re.findall(r"[a-z_]+", _strip_safe(v).lower())) & FORBIDDEN or ".display()" in v, v
    assert not set(re.findall(r"[a-z_]+", _strip_safe("short(key)").lower())) & FORBIDDEN
    assert not set(re.findall(r"[a-z_]+", _strip_safe("u8::from(toss.note.is_some())").lower())) & FORBIDDEN
    parts = _split_top('Info, "hub", "login", "conn" => Cid(conn), "peer" => short(key), "x" => f(a, b)')
    assert parts[3:] == ['"conn" => Cid(conn)', '"peer" => short(key)', '"x" => f(a, b)']


def _catalogue():
    """문서 표의 (영역, 사건) 목록: `| 영역 | 사건 | 수준 | 필드 | 뜻 |` 줄"""
    rows = set()
    for row in DOC.splitlines():
        m = re.match(r"\|\s*`?([a-z]+)`?\s*\|\s*`([a-z0-9_]+)`\s*\|", row)
        if m:
            rows.add((m.group(1), m.group(2)))
    return rows


def test_every_event_is_in_the_catalogue_and_back():
    used = {(c[3], c[4]) for c in CALLS}
    documented = _catalogue()
    assert used - documented == set(), f"undocumented: {sorted(used - documented)}"
    assert documented - used == set(), f"documented but never logged: {sorted(documented - used)}"


def test_relay_has_no_old_style_eprintln_logs():
    for f in sorted(RELAY.rglob("*.rs")):
        for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            if "eprintln!" not in line:
                continue
            # 싱크 자신과, 관리 명령줄이 터미널에 쓰는 답·사용법만
            assert f.name in ("monitor.rs", "main.rs"), f"{f}:{n}: {line.strip()}"
            assert "screenpet-relay:" not in line and "listening on" not in line, f"{f}:{n}: {line.strip()}"


def test_app_network_update_and_toss_logs_are_structured():
    update = (APP / "update.rs").read_text(encoding="utf-8")
    assert "eprintln!" not in update
    objects = (APP / "objects.rs").read_text(encoding="utf-8")
    assert all("failed to create" in l for l in objects.splitlines() if "eprintln!" in l)
    network = (APP / "network.rs").read_text(encoding="utf-8")
    assert "fn net_log" not in network and "append_network_log(&dir, r.line)" in network
    # 내 키 앞 8자를 peer=로: 릴레이 로그와 맞추는 열쇠
    assert '"peer" => short(&self.own_peer)' in network
    net_relay = (NET / "relay.rs").read_text(encoding="utf-8")
    assert "eprintln!" not in net_relay and 'Info, "relay", "login"' in net_relay


def test_docs_explain_monitoring():
    for part in (
        "journalctl --user -u screenpet-relay -f",
        "--since \"10 min ago\"",
        "grep 'conn=c42'",
        "admin status",
        "admin recent",
        "+09:00",
        "network.log",
    ):
        assert part in DOC, part
    readme = (ROOT / "docs" / "architecture" / "README.md").read_text(encoding="utf-8")
    assert "[logging.md](logging.md)" in readme and "(예정)" not in readme
