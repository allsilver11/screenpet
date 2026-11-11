"""콰당! 💥 (render.thud, crates/screenpet-core/src/brain.rs land_or_recover): 높이서 떨어지거나 세게 던져져 내려서면
몸이 아래를 축으로 납작 찌그러졌다 튕기고, 머리 위에 💫이 돌고, 말풍선 "oof!". 새 그림 없이 효과로만 (모든 동물·단계·스타일)."""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
JS = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
CSS = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
HTML = (ROOT / "ui" / "index.html").read_text(encoding="utf-8")


def test_the_pet_squashes_from_its_feet_while_thudding():
    assert 'if (e.payload.thud) petEl.classList.add("thud");' in JS
    assert 'else petEl.classList.remove("thud");' in JS
    rule = CSS[CSS.index("#pet.thud {"):]
    rule = rule[: rule.index("}")]
    # 몸 transform(좌우 반전·흔들기)은 그리기 루프가 쓰므로, 찌그러짐은 따로 쓰는 scale 속성으로 — 발밑이 축
    assert "animation: thud-squash" in rule and "transform-origin: 50% 100%" in rule
    kf = CSS[CSS.index("@keyframes thud-squash"):]
    assert "scale:" in kf[: kf.index("\n}\n")]


def test_dizzy_stars_circle_over_the_head():
    assert 'id="dizzy"' in HTML
    assert 'dizzyEl.hidden = !e.payload.thud;' in JS
    assert "#dizzy" in CSS and "@keyframes dizzy-spin" in CSS


def test_it_says_oof():
    assert 'oof: ["oof!", "ouch!"' in JS
