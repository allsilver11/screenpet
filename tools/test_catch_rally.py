"""⚾ 캐치볼 랠리가 친구 사이를 오간다: 던지는 물건에 랠리를 싣고 (screenpet_net::Toss.rally), 받은 물건에 다시 붙인다
(Object::set_rally). 흐물이가 받아 되던질 때 하나 올리는 건 코어 몫 (brain_rally.rs)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OBJ = (ROOT / "src-tauri" / "src" / "objects.rs").read_text(encoding="utf-8")


def test_outgoing_tosses_carry_the_rally():
    away = OBJ[OBJ.index("fn toss_away(") :]
    away = away[: away.index("\n    }\n")]
    assert "rally: (l.object.rally() > 0).then(|| l.object.rally())" in away


def test_incoming_tosses_keep_the_senders_rally():
    assert "inbox: TossInbox<(AcceptedToss, Side, u32)>," in OBJ
    assert "self.inbox.push((accepted, side, toss.rally.unwrap_or(0)), &mut self.roster)" in OBJ
    admit = OBJ[OBJ.index("fn admit_next_toss(") :]
    admit = admit[: admit.index("\n    }\n")]
    assert "let Some((accepted, side, rally)) = self.inbox.pop()" in admit
    i = admit.index("Object::flown_in(")
    assert "object.set_rally(rally);" in admit[i : i + 400]
