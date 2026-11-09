"""🛷 썰매 세리머니: 루돌프와 산타가 만나면 썰매로 합쳐 화면을 한 바퀴 돈다 (규칙은 crates/screenpet-core/src/sleigh.rs)."""
import re
from pathlib import Path

import numpy as np
from PIL import Image

import draw_sleigh

ROOT = Path(__file__).resolve().parent.parent
APP = ROOT / "src-tauri" / "src"


def test_the_sleigh_art_is_shipped_as_generated(tmp_path):
    out = draw_sleigh.build(tmp_path)
    for name in ("sleigh.png", "sleigh_pixel.png"):
        assert (tmp_path / name).is_file()
        a = np.asarray(Image.open(tmp_path / name).convert("RGBA"))
        b = np.asarray(Image.open(ROOT / "ui" / "sleigh" / name).convert("RGBA"))
        assert (a == b).all(), name
    assert set(out) == {"sleigh", "sleigh_pixel"}


def test_the_sleigh_is_red_with_gold_and_sits_on_the_bottom():
    for name in ("sleigh.png", "sleigh_pixel.png"):
        im = Image.open(ROOT / "ui" / "sleigh" / name).convert("RGBA")
        px = np.asarray(im).astype(int)
        solid = px[..., 3] > 200
        rgb = px[..., :3][solid]
        red = ((rgb[:, 0] > 150) & (rgb[:, 1] < 90) & (rgb[:, 2] < 100)).sum()
        gold = ((rgb[:, 0] > 200) & (rgb[:, 1] > 150) & (rgb[:, 2] < 110)).sum()
        assert red > solid.sum() * 0.3 and gold > solid.sum() * 0.05, (name, red, gold, solid.sum())
        ys = solid.any(axis=1).nonzero()[0]
        assert ys.max() >= im.height - 3, "runners touch the bottom (the window's pivot)"


def test_the_window_rides_quietly_and_shows_both_pets():
    rs = (APP / "sleigh.rs").read_text(encoding="utf-8")
    assert "overlay::finish_quiet(" in rs and "move_quietly(" in rs
    assert ".set_position(" not in rs and ".show()" not in rs
    js = (ROOT / "ui" / "sleigh.js").read_text(encoding="utf-8")
    html = (ROOT / "ui" / "sleigh.html").read_text(encoding="utf-8")
    assert "loadPetSheet(" in js and "frameIndex(" in js
    assert 'src="common.js"' in html and 'src="sleigh.js"' in html
    assert "innerHTML" not in js
    caps = (ROOT / "src-tauri" / "capabilities" / "default.json").read_text(encoding="utf-8")
    assert '"sleigh"' in caps


def test_the_game_loop_freezes_and_parks_the_riders():
    gl = (APP / "game_loop.rs").read_text(encoding="utf-8")
    assert "sleigh.tick(" in gl
    loop = gl[gl.index("s.pet.notice_poops(") - 400 : gl.index("s.pet.notice_poops(")]
    assert "!sleigh.is_riding(&s.label)" in loop, "riders don't walk while flying"
    place = gl[gl.index("s.place(hidden);") - 300 : gl.index("s.place(hidden);")]
    assert "sleigh.is_riding(&s.label)" in place, "rider windows stay parked"
    slots = (APP / "slots.rs").read_text(encoding="utf-8")
    assert "pub fn park(" in slots


def test_help_and_notes_tell_about_it():
    assert "🛷" in (ROOT / "ui" / "help.html").read_text(encoding="utf-8")


def test_the_sleigh_is_lifted_off_the_window_bottom_so_a_tilt_does_not_clip():
    rs = (APP / "sleigh.rs").read_text(encoding="utf-8")
    js = (ROOT / "ui" / "sleigh.js").read_text(encoding="utf-8")
    assert "sleigh::window_box(unit)" in rs and "(h - self.below) * dpi" in rs
    assert "bottom: `${setup.below || 0}px`" in js
    # 그림 상자 크기는 코어와 같다
    core = (ROOT / "crates" / "screenpet-core" / "src" / "sleigh.rs").read_text(encoding="utf-8")
    assert "pub const RIDE_BOX: (f64, f64) = (2.5, 1.25);" in core
    assert "const W = u * 2.5;" in js and "const H = u * 1.25;" in js
