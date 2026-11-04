"""선물 창·도감 창: 커맨드·권한 연결, 글자를 안전하게 넣는지, book.js를 가짜 DOM에서 돌려 본다 (브라우저 없이)."""

import json
import re
import shutil
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui"

HARNESS = r"""
const fs = require("fs");
class Node {
  constructor(tag) { this.tag = tag; this.children = []; this.className = ""; this.style = {}; this.text = ""; }
  set textContent(v) { this.text = String(v); this.children = []; }
  get textContent() { return this.text + this.children.map((c) => (typeof c === "string" ? c : c.textContent)).join(""); }
  append(...xs) { this.children.push(...xs); }
  set innerHTML(v) { throw new Error("innerHTML must not be used"); }
}
const byId = { sum: new Node("div"), bar: new Node("i"), sections: new Node("div") };
global.document = { createElement: (t) => new Node(t), getElementById: (id) => byId[id] };
global.window = {
  scrollY: 0, scrollTo() {}, addEventListener() {},
  __TAURI__: {
    core: { invoke: async (cmd) => { if (cmd !== "book_state") throw new Error(cmd); return VIEW; } },
    webviewWindow: { getCurrentWebviewWindow: () => ({ listen: async () => {} }) },
  },
};
const VIEW = JSON.parse(process.argv[3]);
require("vm").runInThisContext(fs.readFileSync(process.argv[2], "utf8"));
setTimeout(() => {
  const sections = byId.sections.children;
  const out = { sum: byId.sum.textContent, bar: byId.bar.style.width, n: sections.length,
    cells: sections[0].children[1].children.map((c) => ({ cls: c.className, src: c.children[0].src, title: c.children[c.children.length - 1].title, text: c.textContent })) };
  console.log(JSON.stringify(out));
}, 10);
"""


def test_commands_are_registered_and_windows_have_capabilities():
    main = (ROOT / "src-tauri" / "src" / "main.rs").read_text(encoding="utf-8")
    assert "gifts::gift_collect" in main and "gifts::book_state" in main
    caps = json.loads((ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8"))
    windows = {w for cap in caps["capabilities"] for w in cap["windows"]}
    assert "book" in windows and "prop-gift-*" in windows


def test_gift_window_is_prop_labelled_so_tray_hide_follows_it():
    rs = (ROOT / "src-tauri" / "src" / "gifts.rs").read_text(encoding="utf-8")
    assert 'const PREFIX: &str = "prop-gift-";' in rs
    assert "const SIZE: f64 = 40.0;" in rs


def test_pages_never_use_inner_html():
    for name in ("gift.html", "book.html", "book.js"):
        assert "innerHTML" not in (UI / name).read_text(encoding="utf-8"), name


def test_gift_page_has_no_idle_animation():
    html = (UI / "gift.html").read_text(encoding="utf-8")
    assert "infinite" not in html and "setInterval" not in html and "requestAnimationFrame" not in html


def test_book_text_is_korean():
    html = (UI / "book.html").read_text(encoding="utf-8")
    assert "선물 도감" in html and "주우세요" in html
    assert re.search(r"모은 선물", (UI / "book.js").read_text(encoding="utf-8"))


def test_menu_item_is_korean():
    assert '"🎁 선물 도감"' in (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")


@pytest.mark.skipif(shutil.which("node") is None, reason="node not installed")
def test_book_renders_colored_silhouette_and_friend_name(tmp_path):
    item = lambda id, name, count, shiny, frm: {"id": id, "name": name, "count": count, "shiny": shiny, "from": frm, "image": id}  # noqa: E731
    view = {
        "collected": 2, "total": 4, "shiny": 1,
        "sections": [{"key": "cat", "title": "고양이", "emoji": "🐱", "collected": 2, "items": [
            item("cat_yarn", "털실 뭉치", 2, 0, None),
            item("cat_bell", "방울 목걸이", 0, 1, "<b>달이</b>"),
            item("cat_mouse", "???", 0, 0, None),
        ]}],
    }
    script = tmp_path / "harness.js"
    script.write_text(HARNESS, encoding="utf-8")
    out = subprocess.run(["node", str(script), str(UI / "book.js"), json.dumps(view)], capture_output=True, text=True, timeout=30, check=True)
    r = json.loads(out.stdout)
    assert r["n"] == 1 and "2 / 4" in r["sum"] and "반짝이 1" in r["sum"] and r["bar"] == "50%"
    yarn, bell, mouse = r["cells"]
    assert "missing" not in yarn["cls"] and "×2" in yarn["text"] and yarn["src"] == "gifts/cat_yarn.png"
    assert "shiny" in bell["cls"] and "✨×1" in bell["text"]
    # 친구 이름은 그대로 글자로 (태그로 해석되지 않는다), "…님이 준 선물"
    assert "💌 <b>달이</b>" in bell["text"] and bell["title"] == "<b>달이</b>님이 준 선물"
    assert "missing" in mouse["cls"] and "???" in mouse["text"]
