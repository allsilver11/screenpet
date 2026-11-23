"""🧳 포트키의 앱·웹뷰 쪽 (코어 brain_portkey.rs·portkey.rs): 놓인 물건 자리와 짝 열쇠를 펫에게 알리고, 친구 쪽은 새 버전에게만 Visit.portkey로 물어보고
(물건은 싣지 않음), 곧바로 돌려보내지면 내 화면의 짝 물건으로 (없으면 제자리), 받는 쪽은 같은 물건이 있을 때만 그 옆에 나타나게 함.
앱은 이 컴퓨터에서 빌드할 수 없어 소스로 본다 (cargo check는 따로)."""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"
OBJECTS = (SRC / "objects.rs").read_text(encoding="utf-8")
NETWORK = (SRC / "network.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
MAIN = (ROOT / "ui" / "main.js").read_text(encoding="utf-8")
BRAIN = (ROOT / "crates" / "screenpet-core" / "src" / "brain.rs").read_text(encoding="utf-8")


def body(src, start, end):
    """src에서 start부터 end 앞까지"""
    i = src.index(start)
    return src[i : src.index(end, i + len(start))]


def test_every_still_object_is_a_portkey_and_toys_use_only_the_icon_picture():
    spots = body(OBJECTS, "pub fn portkey_spots(", "\n    }\n")
    assert "l.object.spot(l.id)?" in spots, "바닥·창 위에 가만히 놓인 것만"
    assert "look: l.portkey_look?" in spots and "PortkeyKey::of(l.object.kind)" in spots, "장난감은 그린 아이콘이 있을 때만"
    assert "title" not in spots and "name" not in spots, "앱 이름·창 제목은 쓰지 않음"
    assert "objects.portkey_spots(&mut portkey_spots);" in LOOP
    assert re.search(r"if all_hidden \{\s*portkey_spots\.clear\(\);", LOOP), "모두 숨겨 둔 동안엔 없음"
    assert "network.set_portkey_spots(&portkey_spots);" in LOOP


def test_only_own_pets_ask_new_friends_and_portkeys_carry_nothing():
    feed = body(LOOP, "if s.pet.cfg.portkey {", "\n                }\n")
    assert "s.is_own() &&" in feed and "portkey_friend_ok" in feed, "손님은 이 화면 안에서만"
    friends = body(NETWORK, "fn portkey_friends", "\n    }\n")
    assert "PORTKEY_MIN_VERSION" in friends and "self.is_open(id)" in friends, "옛 친구에겐 친구 쪽 포트키를 안 씀"
    away = body(NETWORK, "pub fn portkey_away(", "\n    }\n")
    assert "None, None, None, None, None, None, Some(screenpet_net::Portkey { kind, look })" in away, "물건·쪽지·탈것은 싣지 않음"
    assert "self.portkey_out.insert(visit_id" in away
    asks = body(LOOP, "for (i, key) in portkey_asks {", "\n        }\n")
    assert "|| !network.portkey_away(" in asks and "portkey_return(world)" in asks, "보낼 친구가 없으면 내 화면의 짝 · 제자리"


def test_a_quick_refusal_comes_back_by_portkey_and_the_host_checks_the_object():
    leave = body(NETWORK, "let portkey_sent = self.portkey_out.remove(&visit_id);", "return;\n                }")
    assert "!p.acked" in leave and "portkey_return(world)" in leave and "self.away.remove(&uid)" in leave
    host = body(NETWORK, "let portkey_at = match &portkey {", "None => None,")
    assert "self.portkey_arrival(from, w)" in host and "self.reply_later(link, Visit::leave(visit_id))" in host, "같은 물건이 없으면 (늦춰) 돌려보냄"
    arrival = body(NETWORK, "fn portkey_arrival(", "\n    }\n")
    assert "PortkeyKey::from_wire(" in arrival and "pick_arrival(&self.portkey_spots" in arrival, "아는 값만 · 받는 쪽 물건 자리"
    assert "pet.portkey_arrive(&spot, world, nth);" in NETWORK
    # 바쁠 때 (집중·하우스) 돌려보내기가 포트키 검사보다 먼저
    visit = body(NETWORK, "fn on_visit_message(", "fn welcome_guest(")
    assert visit.index("own_refuses(slots)") < visit.index("let portkey_at")


def test_webview_twirls_and_has_portkey_lines_and_wizards_carry_the_flag():
    assert re.search(r"^    pub twirl: f64,", BRAIN, re.M)
    assert "render.twirl" in MAIN and "scaleX(${Math.cos(" in MAIN
    extra = body(MAIN, "const EXTRA_SPEECH = {", "\n};")
    enum = body(BRAIN, "pub enum Speech {", "\n}")
    for v, key in (("Portkey", "portkey"), ("PortkeyLand", "portkey_land"), ("PortkeyHuh", "portkey_huh")):
        assert re.search(rf"^\s{{4}}{v},", enum, re.M), v
        assert re.search(rf"\b{key}: \[\"", extra), key
    assert '"Portkey!"' in extra and '"huh?"' in extra
    for pid in ("harry", "hermione", "voldemort", "cat", "hagrid"):
        m = json.loads((ROOT / "ui" / "pets" / pid / "pet.json").read_text(encoding="utf-8"))
        assert m.get("portkey", False) is (pid in ("harry", "hermione", "voldemort")), pid
    help_html = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")
    li = next(li for li in help_html.split("<li>") if li.startswith("🧳 <b>포트키</b>"))
    assert all(w in li for w in ("Portkey!", "똑같은 물건", "huh?", "제자리", "같이")) and li.rstrip().endswith("요.</li>")


def test_icon_toys_are_not_a_presence_oracle():
    """🔒 친구가 아이콘 해시를 맞혀 보며 내 화면의 장난감을 알아내지 못하게: 그 친구와 주고받은 아이콘만, 친구마다 PORTKEY_ASK_GAP에 한 번, 답은 늘 늦춰서."""
    arrival = body(NETWORK, "fn portkey_arrival(", "\n    }\n")
    assert "self.icon_shared_with(from, key.look)" in arrival and "is_toy()" in arrival
    assert "PORTKEY_ASK_GAP" in arrival and "self.portkey_seen" in arrival
    assert "PORTKEY_JOIN_BURST" in arrival, "같이 잡은 마법사 몇은 함께 받음"
    assert "fn reply_later(" in NETWORK and "portkey_replies" in NETWORK
    later = body(NETWORK, "fn reply_later(", "\n    }\n")
    assert "PORTKEY_REPLY_SECS" in later
    # 주고받은 아이콘을 친구마다 적음: 던질 때·받을 때·들고 갈 때·들고 올 때
    assert "network.note_icon_shared(&friend," in OBJECTS
    assert "network.note_wire_icon(&from," in LOOP
    assert NETWORK.count("self.note_wire_icon(") >= 2


def test_the_host_acks_and_silence_brings_the_wizard_back():
    ok = body(NETWORK, "// 🧳 화면 끝이 아니라 같은 물건 옆에 빙글 나타남", "\n                        }\n")
    assert "self.portkey_in.push(" in ok and "Visit::PortkeyOk { visit_id }" in ok, "받았을 때만 셈 · 받았다고 답함"
    arrival = body(NETWORK, "fn portkey_arrival(", "\n    }\n")
    assert "self.portkey_in.push(" not in arrival
    assert "Visit::PortkeyOk { visit_id } =>" in NETWORK
    tick = body(NETWORK, "pub fn tick_portkeys(", "\n    }\n")
    assert "ANSWER_SECS" in tick and "Visit::ComeHome" in tick and "portkey_return(world)" in tick
    assert "network.tick_portkeys(" in LOOP
    leave = body(NETWORK, "let portkey_sent = self.portkey_out.remove(&visit_id);", "let reply")
    assert "!p.acked" in leave
    assert "&& portkey_sent.is_none()" in body(NETWORK, "let portkey_sent = self.portkey_out.remove(&visit_id);", "let forwarded"), "포트키는 꽉 찼다고 보지 않음"


def test_no_friend_portkey_while_hidden_or_focusing_and_toy_looks_are_cached():
    asks = body(LOOP, "for (i, key) in portkey_asks {", "\n        }\n")
    assert "all_hidden" in asks and "is_focusing()" in asks
    spots = body(OBJECTS, "pub fn portkey_spots(", "\n    }\n")
    assert "l.portkey_look" in spots and "PortkeyKey::toy(" not in spots, "아이콘 해시는 매 틱 다시 세지 않음"
    assert "portkey_look" in body(OBJECTS, "fn set_icon(", "\n    }\n")


def test_portkey_log_events_are_catalogued():
    doc = (ROOT / "docs" / "architecture" / "logging.md").read_text(encoding="utf-8")
    for ev in ("portkey_out", "portkey_back", "portkey_timeout", "portkey_in"):
        assert f"`{ev}`" in doc, ev
