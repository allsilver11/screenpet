"""🔎 "엥 왜 이래?" 웹뷰 정적 검사 — 화면에 엉뚱한 것이 보이거나 그려야 할 것이 안 그려지는 실수를 HTML·CSS·JS·pet.json만 읽어 찾는다.

- hidden인 요소 (HTML의 hidden 속성, 또는 JS가 `.hidden =`으로 끄고 켜는 요소)가 CSS에서도 정말 숨는다:
  SVG 요소는 브라우저 기본 [hidden] 숨김이 안 걸리고 (v0.7.3 👜 가방 끈이 칸 구석에 점으로 찍힘), HTML 요소도 작성자 CSS의 display가
  기본 숨김을 덮어쓰면 보인다 → 그 요소에 맞는 [hidden] { display: none } 규칙이 더 세거나 같아야 함
- 기본으로 보이는 자리에 크기 없는 그림 (좌표 없는 선·d 없는 path — 둥근 끝이면 점으로 찍힘)이 없다 (숨은 조상 안이면 괜찮음)
- 코어가 낼 수 있는 그림 (brain.rs Anim serde 이름)은 어느 동물의 pet.json에든 있거나 main.js가 대신 그리고, pet.json의 그림 이름은 모두 코어가 아는 이름
- 코어가 낼 수 있는 대사 (Speech)는 어느 동물의 대사표나 기본 대사에 있고, 대사표의 이름은 모두 코어가 아는 이름
"""
import json
import re
from html.parser import HTMLParser
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")
MAIN = (UI / "main.js").read_text(encoding="utf-8")
COMMON = (UI / "common.js").read_text(encoding="utf-8")
PETS = {p.parent.name: json.loads(p.read_text(encoding="utf-8")) for p in sorted((UI / "pets").glob("*/pet.json"))}
# main.js가 따로 대신 그리는 그림 (큰 점프 그림이 없으면 hop 줄 — draw()의 hopLeap)
DRAWN_INSTEAD = {"bigjump": "hop"}
VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"}


class Element:
    """HTML 요소 하나: 태그, 속성, 조상, SVG 안인지"""

    def __init__(self, tag, attrs, parent):
        self.tag = tag
        self.attrs = dict(attrs)
        self.parent = parent
        self.svg = tag == "svg" or (parent is not None and parent.svg)

    @property
    def id(self):
        return self.attrs.get("id")

    @property
    def classes(self):
        return set((self.attrs.get("class") or "").split())

    def ancestors(self):
        p = self.parent
        while p is not None:
            yield p
            p = p.parent

    def __repr__(self):
        return f"<{self.tag}{' #' + self.id if self.id else ''}{' .' + '.'.join(sorted(self.classes)) if self.classes else ''}>"


class Collect(HTMLParser):
    """요소 목록, 안쪽 <style> 글, <link> 스타일시트, <script src>"""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.elements, self.stack, self.styles, self.sheets, self.scripts = [], [], [], [], []
        self._in_style = False

    def handle_starttag(self, tag, attrs):
        el = Element(tag, attrs, self.stack[-1] if self.stack else None)
        self.elements.append(el)
        a = el.attrs
        if tag == "link" and a.get("rel") == "stylesheet":
            self.sheets.append(a["href"])
        if tag == "script" and a.get("src"):
            self.scripts.append(a["src"])
        self._in_style = tag == "style"
        if tag not in VOID:
            self.stack.append(el)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if tag not in VOID:
            self.stack.pop()
        self._in_style = False

    def handle_endtag(self, tag):
        self._in_style = False
        for i in range(len(self.stack) - 1, -1, -1):
            if self.stack[i].tag == tag:
                del self.stack[i:]
                break

    def handle_data(self, data):
        if self._in_style:
            self.styles.append(data)


def css_rules(text):
    """(순서, 선택자, 선언 dict) — @media 안 규칙도 (중괄호 없는 가장 안쪽 블록만)"""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    out = []
    for m in re.finditer(r"([^{}]+)\{([^{}]*)\}", text):
        sel = m.group(1).strip()
        if sel.startswith("@") or re.fullmatch(r"(from|to|[\d.]+%)(\s*,\s*(from|to|[\d.]+%))*", sel):
            continue
        decls = {}
        for d in m.group(2).split(";"):
            if ":" in d:
                k, v = d.split(":", 1)
                decls[k.strip().lower()] = v.strip().lower()
        for s in sel.split(","):
            out.append((len(out), s.strip(), decls))
    return out


def last_compound(sel):
    """선택자의 마지막 단순 선택자 묶음 (`#stage > #pet.x[hidden]` → `#pet.x[hidden]`)"""
    return re.split(r"\s*[>+~]\s*|\s+", sel.strip())[-1]


def matches(compound, el):
    """마지막 묶음이 이 요소에 맞음 (조상 조건은 보지 않음 — 넉넉하게). [hidden]·:not([hidden])은 따로 봄"""
    c = re.sub(r":not\(\[hidden\]\)", "", compound)
    c = re.sub(r"\[hidden\]", "", c)
    c = re.sub(r"::?[\w-]+(\([^)]*\))?", "", c)  # 다른 가상 클래스·요소는 무시
    tag = re.match(r"^[a-zA-Z][\w-]*|^\*", c)
    if tag and tag.group(0) not in ("*", el.tag):
        return False
    for i in re.findall(r"#([\w-]+)", c):
        if el.id != i:
            return False
    for k in re.findall(r"\.([\w-]+)", c):
        if k not in el.classes:
            return False
    for attr in re.findall(r"\[([\w-]+)(?:[~|^$*]?=[^\]]*)?\]", c):
        if attr not in el.attrs:
            return False
    return bool(c) or "[hidden]" in compound


def specificity(sel):
    ids = len(re.findall(r"#[\w-]+", sel))
    cls = len(re.findall(r"\.[\w-]+|\[[^\]]+\]|:(?!:)[\w-]+", sel)) - len(re.findall(r":not\(", sel))
    tags = len(re.findall(r"(?:^|[\s>+~(])([a-zA-Z][\w-]*)", sel))
    return (ids, cls, tags)


def js_hidden_ids(scripts):
    """JS가 `.hidden =`으로 끄고 켜는 요소 id (getElementById·querySelector("#id")로 잡은 변수)"""
    ids = set()
    for src in scripts:
        path = UI / src
        if not path.is_file():
            continue
        js = path.read_text(encoding="utf-8")
        names = {}
        for m in re.finditer(r"(?:const|let|var)\s+(\w+)\s*=\s*document\.(?:getElementById\(\s*[\"']([\w-]+)[\"']|querySelector\(\s*[\"']#([\w-]+)[\"'])", js):
            names[m.group(1)] = m.group(2) or m.group(3)
        for var, el_id in names.items():
            if re.search(rf"\b{re.escape(var)}\.hidden\s*=", js):
                ids.add(el_id)
        for m in re.finditer(r"document\.getElementById\(\s*[\"']([\w-]+)[\"']\s*\)\.hidden\s*=", js):
            ids.add(m.group(1))
    return ids


def pages():
    return sorted(p.name for p in UI.glob("*.html"))


def load(page):
    c = Collect()
    c.feed((UI / page).read_text(encoding="utf-8"))
    css = "\n".join([(UI / s).read_text(encoding="utf-8") for s in c.sheets if (UI / s).is_file()] + c.styles)
    return c, css_rules(css)


def shown_display(rules, el, hidden):
    """hidden 상태의 el에 마지막으로 이기는 display 값과 그 규칙 (없으면 None — 브라우저 기본)"""
    best = None
    for order, sel, decls in rules:
        if "display" not in decls:
            continue
        comp = last_compound(sel)
        if not matches(comp, el):
            continue
        if ":not([hidden])" in comp and hidden:
            continue
        if "[hidden]" in comp.replace(":not([hidden])", "") and not hidden:
            continue
        key = ("!important" in decls["display"], specificity(sel), order)
        if best is None or key >= best[0]:
            best = (key, decls["display"].replace("!important", "").strip(), sel)
    return best


@pytest.mark.parametrize("page", pages())
def test_hidden_elements_are_really_hidden(page):
    """hidden 속성이 있거나 JS가 hidden으로 끄는 요소는 CSS를 다 따져도 display: none (SVG는 브라우저 기본 숨김이 없음)"""
    c, rules = load(page)
    toggled = js_hidden_ids(c.scripts)
    bad = []
    for el in c.elements:
        if "hidden" not in el.attrs and el.id not in toggled:
            continue
        win = shown_display(rules, el, hidden=True)
        if win is None:
            if el.svg:
                bad.append(f"{el}: SVG [hidden] has no CSS rule — Chromium draws it anyway")
            continue
        if win[1] != "none":
            bad.append(f"{el}: hidden but `{win[2]}` gives display: {win[1]}")
    assert not bad, f"{page}:\n" + "\n".join(bad)


def hidden_by_default(el, rules):
    """처음 화면에 안 보이는 요소 (자신이나 조상이 hidden이고 정말 숨음, 또는 CSS가 display: none)"""
    for e in [el, *el.ancestors()]:
        win = shown_display(rules, e, hidden="hidden" in e.attrs)
        if win is not None and win[1] == "none":
            return True
        if win is None and "hidden" in e.attrs and not e.svg:
            return True
    return False


@pytest.mark.parametrize("page", pages())
def test_no_zero_size_strokes_show_by_default(page):
    """좌표 없는 선·d 없는 path·r 없는 원 (JS가 나중에 채움)은 처음엔 숨은 조상 안에 — 둥근 끝 선은 0,0에 점으로 찍힘"""
    c, rules = load(page)
    bad = []
    for el in c.elements:
        if not el.svg:
            continue
        a = el.attrs
        empty = (el.tag == "line" and not all(k in a for k in ("x1", "y1", "x2", "y2"))) or (el.tag == "path" and not a.get("d")) or \
                (el.tag in ("circle", "ellipse") and not any(k in a for k in ("r", "rx")))
        if empty and not hidden_by_default(el, rules):
            bad.append(repr(el))
    assert not bad, f"{page}: empty shapes visible at load: {bad}"


def test_the_bag_rule_is_caught():
    """검사가 v0.7.3 버그를 잡는지: svg[hidden] 규칙을 빼면 #bag이 걸림"""
    c, rules = load("index.html")
    rules = [r for r in rules if r[1] != "svg[hidden]"]
    bag = next(e for e in c.elements if e.id == "bag")
    assert shown_display(rules, bag, hidden=True) is None and bag.svg


def enum_names(name):
    """brain.rs enum의 serde 이름들 (snake_case, rename 반영)"""
    body = BRAIN[BRAIN.index(f"pub enum {name} {{"):]
    body = body[:body.index("\n}")]
    out, rename = [], None
    for line in body.splitlines():
        m = re.search(r'#\[serde\(rename = "(\w+)"\)\]', line)
        if m:
            rename = m.group(1)
            continue
        m = re.match(r"\s{4}([A-Z]\w*)\b", line)
        if m:
            out.append(rename or re.sub(r"(?<!^)(?=[A-Z])", "_", m.group(1)).lower())
            rename = None
    return out


def test_every_core_anim_is_drawable_and_every_sheet_row_is_a_core_anim():
    """코어 그림 이름이 어느 pet.json에든 있거나 main.js가 대신 그림 (없으면 웹뷰가 조용히 idle로 — "갑자기 다른 그림").
    pet.json 쪽 이름이 코어에 없으면 (오타) 그 줄은 영영 안 그려짐"""
    anims = enum_names("Anim")
    assert "walk" in anims and "bigjump" in anims and "hang_cursor" in anims
    have = set().union(*(m["animations"] for m in PETS.values()))
    assert [a for a in anims if a not in have and DRAWN_INSTEAD.get(a) not in have] == []
    for pid, m in PETS.items():
        assert [a for a in m["animations"] if a not in anims] == [], pid
    for a, b in DRAWN_INSTEAD.items():
        assert f'render.anim === "{a}"' in MAIN and f"manifest.animations.{b}" in MAIN


def js_object_keys(src, decl):
    body = src[src.index(decl) + len(decl):]
    body = body[:body.index("\n};")]
    return set(re.findall(r'^\s+"?([a-z_0-9]+)"?:', body, re.M))


def test_every_core_speech_has_a_line_and_every_line_is_a_core_speech():
    """코어 대사 이름마다 어느 동물의 대사표·main.js·common.js 기본 대사에 줄이 있음 (없으면 말풍선이 안 뜸). 대사표 이름 오타도 없음"""
    speech = enum_names("Speech")
    assert "chatter" in speech and "level_up" in speech
    stages = {"baby", "kinder", "child", "teenager", "adult"}
    defaults = js_object_keys(MAIN, "const EXTRA_SPEECH = {") | js_object_keys(COMMON, "const RHYTHM_SPEECH = {") | {"cursor_play"}
    staged = js_object_keys(MAIN, "const STAGE_SPEECH = {") - stages
    tables = set()
    for m in PETS.values():
        tables |= set(m.get("speech", {}))
        for t in m.get("stage_speech", {}).values():
            tables |= set(t)
    assert [s for s in speech if s not in defaults | staged | tables] == []
    assert sorted((defaults | staged | tables) - set(speech)) == []
