"""숨겨 둔 SVG는 정말 숨겨짐 — 브라우저 기본 [hidden]{display:none}은 HTML 요소에만 걸리고 SVG 요소에는 안 걸림.

v0.7.3까지 👜 구슬 가방 SVG(#bag)에 숨김 규칙이 빠져 늘 보였고, 좌표 없는 가방 끈(둥근 끝)이 모든 펫 칸 왼쪽 위
(뒤집히면 오른쪽 위)에 점으로 찍혔음. 그래서 SVG를 쓰는 페이지의 CSS는 svg[hidden]을 모두 숨김.
"""
import re
from pathlib import Path

UI = Path(__file__).resolve().parent.parent / "ui"


def _css_for(html: str) -> str:
    out = []
    for href in re.findall(r'<link[^>]+href="([^"]+\.css)"', html):
        p = UI / href
        if p.exists():
            out.append(p.read_text(encoding="utf-8"))
    out += re.findall(r"<style[^>]*>(.*?)</style>", html, re.S)
    return "\n".join(out)


def test_every_page_with_a_hidden_svg_hides_it_in_css():
    pages = 0
    for page in sorted(UI.glob("*.html")):
        html = page.read_text(encoding="utf-8")
        if not re.search(r"<svg\b[^>]*\shidden\b", html):
            continue
        pages += 1
        css = _css_for(html)
        assert re.search(r"(^|[\s,}])svg\[hidden\]\s*[,{][^}]*display:\s*none", css, re.S), page.name
    assert pages >= 1


def test_the_bag_strap_has_no_stray_dot_when_the_bag_is_hidden():
    main = (UI / "main.js").read_text(encoding="utf-8")
    assert "bagEl.hidden = !(b > 0);" in main
