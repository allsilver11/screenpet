"""웹뷰 CSP: 정책이 막을 것을 페이지가 쓰지 않는가. 진짜 웹뷰 없이 보는 정적 검사다 — 실제로 차단되는지는 앱을 띄워야 안다."""

import json
import re
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
PAGES = sorted(UI.glob("*.html"))
SCRIPTS = sorted(UI.glob("*.js"))
BY_NAME = [p.name for p in PAGES]


@pytest.mark.parametrize("page", PAGES, ids=BY_NAME)
def test_pages_have_no_inline_scripts_or_handlers(page):
    html = page.read_text(encoding="utf-8")
    for attrs in re.findall(r"<script\b([^>]*)>", html):
        src = re.search(r'src="([^"]+)"', attrs)
        assert src, "script는 파일로 (script-src 'self')"
        assert (UI / src.group(1)).is_file(), src.group(1)
    assert not re.search(r"<[a-z][^>]*\son[a-z]+\s*=", html, re.I), "on… 속성은 CSP에 막힌다"
    assert "javascript:" not in html


@pytest.mark.parametrize("script", SCRIPTS, ids=[p.name for p in SCRIPTS])
def test_scripts_do_not_need_eval(script):
    js = script.read_text(encoding="utf-8")
    assert not re.search(r"\beval\(|new Function\b|document\.write|setTimeout\(\s*[\"'`]|setInterval\(\s*[\"'`]", js)
    assert not re.search(r"""\.on(error|load|click)\s*=\s*["'`]""", js)


def test_nothing_is_loaded_from_outside_the_app():
    # img-src·connect-src는 앱 안 파일만이다: 원격 주소·data:·blob:을 쓰면 정책도 함께 고친다
    for f in [*PAGES, *SCRIPTS, UI / "style.css"]:
        text = f.read_text(encoding="utf-8").replace("http://www.w3.org/2000/svg", "")
        assert not re.search(r"https?://|\bdata:|\bblob:|\basset:|convertFileSrc|createObjectURL|toDataURL", text), f.name


# 고칠 땐 앱을 띄워 창마다 확인한다 — 잘못되면 창이 빈 채로 뜨고 어떤 테스트도 잡지 못한다
POLICY = {
    "default-src": "'self'",
    "script-src": "'self'",
    # 글자 style 속성·<style> (창마다 있다). Tauri가 style-src에 nonce를 넣으면 'unsafe-inline'이 무시되므로 그것만 끈다
    "style-src": "'self' 'unsafe-inline'",
    "img-src": "'self'",
    # ipc: / http://ipc.localhost = invoke (macOS·Windows), 'self' = pet.json fetch
    "connect-src": "'self' ipc: http://ipc.localhost",
    "object-src": "'none'",
    "base-uri": "'none'",
}


def test_policy_is_the_one_reviewed():
    conf = json.loads((ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8"))
    security = conf["app"]["security"]
    assert security["csp"] == POLICY
    assert security["dangerousDisableAssetCspModification"] == ["style-src"]
    assert "'unsafe-eval'" not in json.dumps(security)
