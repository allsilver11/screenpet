"""창 스크립트를 html이 읽는 순서 그대로 가짜 DOM에서 돌린다 (tools/window_harness.js): 첫 로드 실패, 프레임당 스타일 쓰기, 글 쓰는 창의 키."""

import json
from pathlib import Path

import pytest
from conftest import needs_node, run_node

ROOT = Path(__file__).resolve().parent.parent
HARNESS = ROOT / "tools" / "window_harness.js"
pytestmark = needs_node


def run(*args):
    out = run_node(HARNESS, *args, check=False, timeout=60)
    assert out.returncode == 0, out.stderr
    return json.loads(out.stdout)


@pytest.mark.parametrize("page", sorted(p.name for p in (ROOT / "ui").glob("*.html")))
def test_every_window_loads_its_scripts_without_a_clash(page):
    run("load", page)


@pytest.mark.parametrize("scenario", ["firstLoadFails", "firstLoadFailsHouse"])
def test_a_failed_first_load_does_not_stop_the_draw_loop(scenario):
    r = run(scenario)
    assert r["running"], "그리기 고리가 시작되지 않았다"
    assert r["drew"], "다음 동물 정보로 다시 그리지 못했다"


def test_idle_frames_write_no_styles_but_frames_and_spin_still_do():
    r = run("drawWrites")
    assert r["idle"] == [0, 0, 0]
    assert r["advanced"] >= 2, "칸이 넘어가면 쓴다"
    # 구르기는 프레임마다 돈다
    assert r["spinWrites"] == 10 and r["spinDistinct"] == 10, r["spin"]
    assert "rotate(" in r["spin"]


@pytest.mark.parametrize("pet", ["penguin", "snowman"])
def test_pixel_belly_slide_plays_its_roll_row_without_spinning(pet):
    r = run("pixelRoll", pet)
    assert r["hasRoll"] and r["row"] == r["rollRow"], r
    assert not r["rotates"], r


@pytest.mark.parametrize("pet", ["cat", "santa", "giraffe"])
def test_other_pixel_pets_still_have_no_roll_and_never_spin(pet):
    r = run("pixelRoll", pet)
    assert not r["hasRoll"] and r["row"] == r["idleRow"], r
    assert not r["rotates"], r


def test_house_door_writes_only_when_the_frame_changes():
    assert run("drawWritesHouse")["idle"] == 0


def test_frame_index_never_goes_negative():
    r = run("frameIndex")
    assert r == {"loop": 0, "once": 0, "later": 0}


@pytest.mark.parametrize(
    "page,submit,close",
    [("letter.html", "letter_submit", "letter_close"), ("postit.html", "postit_submit", "postit_close"), ("name.html", "name_submit", "name_close"), ("code.html", "code_submit", "code_close"), ("link.html", "link_submit", "link_close")],
)
def test_form_keys(page, submit, close):
    r = run("formKeys", page, submit, close)
    assert r["onButton"] == 0, "버튼 위 Enter는 그 버튼만 누른다"
    assert r["composing"] == 0, "한글 조합 중 Enter는 글자 확정"
    assert r["onInput"] == 1
    assert r["closed"] == 1
    assert r["f5"] and r["ctrlR"] and r["menu"]
    assert not r["typing"], "글자 치기는 막지 않는다"


def test_link_window_asks_whether_it_is_my_own_pc_before_linking():
    r = run("linkConfirm")
    c = r["confirm"]
    assert c["title"] == "본인의 PC가 맞습니까?"
    assert c["ask"] == "스크린펫의 모든 데이터가 공유됩니다."
    assert "민수" in c["who"], "저쪽이 누구인지 보인다"
    assert (c["ok"], c["cancel"]) == ("확인", "취소")
    assert not c["okDisabled"] and not c["input"] and not c["code"]
    assert r["yes"] == [{"yes": True}]
    w = r["waiting"]
    assert w["title"] == "본인의 PC가 맞습니까?" and w["note"] == "저쪽 컴퓨터에서도 확인을 눌러 주세요"
    assert w["okDisabled"] and w["cancel"] == "취소"
    # 기다리는 동안 확인은 다시 가지 않고, 취소는 간다
    assert r["after"] == [{"yes": True}, {"yes": False}]
    assert r["closed"] == 1, "Esc는 창 닫기 (백엔드가 취소한다)"
    x = r["cancelled"]
    assert x["note"] == "연결을 취소했어요" and x["ok"] is None and x["ask"] is None and x["cancel"] == "닫기"


def test_book_blocks_refresh_and_has_no_close_key():
    r = run("bookKeys")
    assert r["f5"] and r["menu"] and not r["escape"]


def test_renewing_a_code_does_not_copy_the_old_one():
    r = run("renewCopy")
    assert r["renewed"] == 1
    assert r["copyDisabled"] and r["whilePending"] == []
    assert not r["copyDisabledAfter"] and r["copied"] == ["NEW1-2345"]


def test_bubble_text_lives_in_a_clippable_span():
    assert run("bubbleText") == {"tag": "span", "text": "off to 아주아주아주긴이름's~ brb!", "hidden": False}


# ---- CSS: 브라우저 없이는 규칙이 있는지만 본다 (보이는 모습은 실제 앱에서) ----
def css_rule(text, selector):
    start = text.index(selector + " {")
    return text[start : text.index("}", start)]


def test_pixel_hide_can_fade_past_the_filled_entrance_animation():
    # 나타나기 애니메이션의 opacity: 1이 forwards로 남아 있으면 .hide의 흐려짐이 막힌다
    gift = (ROOT / "ui" / "gift.html").read_text(encoding="utf-8")
    prop = (ROOT / "ui" / "prop.html").read_text(encoding="utf-8")
    assert "animation: none" in css_rule(gift, "body.pixel.hide #gift")
    assert "animation: none" in css_rule(prop, "body.pixel.hide #prop")


def test_gift_hover_keeps_the_drop_so_it_does_not_replay():
    # 애니메이션 목록에서 drop이 빠졌다 돌아오면 처음부터 다시 돈다 — 목록에 남겨 둔다
    gift = (ROOT / "ui" / "gift.html").read_text(encoding="utf-8")
    assert "drop 420ms ease-out both" in css_rule(gift, "#gift:hover")
    assert "px-drop 300ms step-end both" in css_rule(gift, "body.pixel #gift:hover")


def test_long_names_end_in_an_ellipsis_inside_the_window():
    css = (ROOT / "ui" / "style.css").read_text(encoding="utf-8")
    assert "max-width: calc(100vw - 8px)" in css_rule(css, "#bubble")
    assert "text-overflow: ellipsis" in css_rule(css, "#bubble > span")
    tag = css_rule(css, "#nametag")
    assert "max-width: calc(100vw - 8px)" in tag and "text-overflow: ellipsis" in tag and "overflow: hidden" in tag
    # 꼬리(::after)가 말풍선 밖에 있어 말풍선 자체는 자르지 않는다
    assert "overflow" not in css_rule(css, "#bubble")


# ---- 지운 대체 경로가 다시 필요해지지 않게: 웹뷰가 믿는 그림 자료 ----
def test_pet_data_the_windows_rely_on():
    for d in (ROOT / "ui" / "pets_pixel").iterdir():
        sizes = json.loads((d / "pet.json").read_text(encoding="utf-8"))["frame_sizes"]
        assert set(sizes) >= {"baby", "kinder", "child", "teenager", "adult"}, d.name


def test_pet_glow_gets_room_below_only_while_it_shines():
    """루돌프 코·호박 빛은 몸 둘레로 번진다: 빛나는 동안만 창 아래를 늘려 아래 번짐이 잘리지 않게 (늘 비워 두면 작업 표시줄 클릭을 막는다)."""
    slots = (ROOT / "src-tauri" / "src" / "slots.rs").read_text(encoding="utf-8")
    loop = (ROOT / "src-tauri" / "src" / "game_loop.rs").read_text(encoding="utf-8")
    js = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
    assert "pub const GLOW_SPACE: f64" in slots and "pub fn start_glow(" in slots and '"pet-pad"' in slots
    assert ".start_glow();" in loop[loop.index("Input::PointerDown { label }") :][:800]
    # 바닥에 선 채로 빛날 땐 늘리지 않는다 (아래가 작업 표시줄) — 들어 올렸을 때만
    sync = slots[slots.index("fn sync_glow") :]
    assert "self.pet.is_held()" in sync[: sync.index("\n    }\n")]
    assert 'listen("pet-pad"' in js and "stageEl.style.bottom" in js
