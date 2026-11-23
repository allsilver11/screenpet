"""💪 영웅이 거대한 것을 들고 친구 화면으로 (v0.7.1). 동작은 코어 (brain_juggle.rs·brain_web.rs)가 하고 여기선 앱이 잇는지 소스로 봄.
- 🦇 박쥐인간·🦸 슈퍼인간 (머리 위로)·🕷️ 거미인간 (줄로 끌고)이 든 거대한 것만 Carry에 실음 (look "giant" — 새 필드 없음)
- 거대한 것을 받는 새 버전 친구 (GIANT_CARRY_MIN_VERSION)에게만 — 옛 버전 친구뿐이면 거대한 것은 두고 감 (이 화면 끝에 떨어짐)
- 받는 앱은 제 화면 높이로 크기를 다시 정해 (spawn_carried → scale_at) 그 펫 손에 hold_giant로 쥐여 줌
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src-tauri" / "src"
OBJ = (SRC / "objects.rs").read_text(encoding="utf-8")
NET = (SRC / "network.rs").read_text(encoding="utf-8")
LOOP = (SRC / "game_loop.rs").read_text(encoding="utf-8")
HELP = (ROOT / "ui" / "help.html").read_text(encoding="utf-8")


def fn_body(src: str, name: str) -> str:
    """fn 이름부터 다음 doc comment까지 (대충 그 함수 하나)"""
    start = src.index(f"fn {name}(")
    end = src.find("\n    /// ", start)
    return src[start:end if end != -1 else len(src)]


def test_only_a_hero_carried_giant_goes_on_the_wire():
    body = fn_body(OBJ, "carry_wire")
    assert "giants: bool" in body
    assert "kind.is_giant() && !giants" in body
    # 이름·생김새는 거대한 것을 던질 때 (toss_away)와 같음 — 옛 버전이 몰라도 보통 Carry로 읽힘
    assert "kind.wire_id()" in body and "kind.look()" in body
    assert "objects.carry_wire(id, slots[i].pet.carries_giants())" in LOOP


def test_giants_go_only_to_friends_that_take_them():
    assert "GIANT_CARRY_MIN_VERSION" in NET
    takes = fn_body(NET, "takes_giant_carry")
    assert "GIANT_CARRY_MIN_VERSION" in takes
    exited = fn_body(NET, "own_exited")
    assert "is_some_and(is_giant_carry)" in exited
    assert "me.takes_giant_carry(id)" in exited and "takes(self, &friend)" in exited
    # 내 동물만 거대한 배달 끝을 앎 (손님은 배달하지 않음)
    refresh = NET[NET.index("own.pet.set_prank_edges("):]
    assert "own.pet.set_giant_edges(" in refresh
    assert "s.pet.set_giant_edges(false, false)" in refresh


def test_a_received_giant_is_resized_here_and_held_as_a_giant():
    spawn = fn_body(OBJ, "spawn_carried")
    # 거대한 것은 보낸 크기 대신 이 화면 높이에 맞춤 (scale은 똥만 씀)
    assert "scale.filter(|_| kind.is_poop())" in spawn and "self.scale_at(app, kind, base, at.0, at.1)" in spawn
    arrivals = LOOP[LOOP.index("for c in network.take_carry_arrivals()"):]
    arrivals = arrivals[:arrivals.index("Input::ShowHelp")]
    assert "c.kind.is_giant()" in arrivals
    assert "s.pet.hold_giant(id, half)" in arrivals and "objects.half_w_of(id)" in arrivals
    assert "s.pet.hold_object(id)" in arrivals


def test_help_tells_about_heroes_taking_giants_to_friends():
    assert "거대한 돌·똥을 들고 친구 화면" in HELP
    assert "예전 버전" in HELP[HELP.index("거대한 돌·똥을 들고 친구 화면"):][:400]
