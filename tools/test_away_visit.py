"""친구 화면에 간 내 동물 확인과 "펫 데리고 오기" 대답 (v0.6.8 고침). 판단은 screenpet_core::away가 하고 여기선 앱이 그걸 쓰는지 본다.
- 친구 앱이 다시 켜지면 링크가 몇 초 만에 다시 붙어 끊김 소식이 없음 → 가 있는 동안 묻고, 없다고 하면 바로 집으로
- 내 동물이 놀러 가 있으면 친구의 "busy~"를 보이는 곳(손님)에, 아무도 없으면 돌아올 때 띄움
- 그 친구 펫이 이미 와 있으면 "데리고 오기" 대신 "이미 와 있어요"
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NET = (ROOT / "src-tauri" / "src" / "network.rs").read_text(encoding="utf-8")
TRAY = (ROOT / "src-tauri" / "src" / "tray.rs").read_text(encoding="utf-8")
CORE = (ROOT / "crates" / "screenpet-core" / "src" / "lib.rs").read_text(encoding="utf-8")


def fn_body(src: str, name: str) -> str:
    """fn 이름부터 다음 doc comment까지 (대충 그 함수 하나)"""
    start = src.index(f"fn {name}(")
    end = src.find("\n    /// ", start)
    return src[start:end if end != -1 else len(src)]


def test_core_has_the_away_rules():
    assert "pub mod away;" in CORE
    assert "use screenpet_core::away;" in NET


def test_my_away_pet_asks_its_host_now_and_then():
    tick = fn_body(NET, "tick")
    assert "away::probe_due(" in tick
    assert "net.ask(&a.friend, &away::where_ask(a.visit_id))" in tick
    # 예전처럼 보낸 뒤 32분만 기다리지 않음: 답하던 친구가 조용해지면 일찍 부름
    assert "away::too_quiet(" in tick
    assert ">= visit::AWAY_SILENCE_LIMIT_SECS" not in NET


def test_the_host_answers_whether_the_guest_is_still_there():
    ask = fn_body(NET, "on_ask")
    assert "away::parse_where(ask)" in ask
    assert "away::answer(visit_id, present)" in ask
    # 📱 내 다른 컴퓨터의 펫을 친구에게 데려다준 중이면 그 펫은 아직 있는 것
    assert "self.carried.get(&visit_id)" in ask


def test_a_lost_visit_brings_my_pet_home_right_away_with_a_line():
    ask = fn_body(NET, "on_ask")
    assert "away::parse_news(ask)" in ask
    news = fn_body(NET, "on_away_news")
    assert "a.friend == from" in news and "a.visit_id" in news
    assert "self.come_back(&mut slots[i], None, None)" in news
    assert "away::LOST_LINE" in news


def test_a_refusal_is_shown_where_someone_can_see_it():
    assert 'emit_to(OWN_LABEL, "pet-say-text", format!("{name}: busy~"))' not in NET
    say = fn_body(NET, "say_answer")
    assert "away::speaker(" in say
    assert "self.held_line = Some(" in say
    # 돌아오면 미뤄 둔 한마디를 함
    assert "self.held_line.take()" in fn_body(NET, "tick")


def test_bringing_a_pet_that_is_already_here_says_so():
    assert "pub visiting: bool," in TRAY
    assert "펫 이미 와 있어요" in TRAY
    assert "visiting: self.guest_owners.iter().any(" in NET
    bring = NET[NET.index("NetInput::BringPet(id) => {"):]
    bring = bring[: bring.index("NetInput::BringDevicePet =>")]
    assert "away::ALREADY_HERE_LINE" in bring
