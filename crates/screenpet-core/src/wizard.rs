//! 🧙 마법사 펫 (해리포터·헤르미온느 — 나중에 볼드모트도)의 주문·책·가방 숫자와 주문 이름. 동작은 brain_spell.rs (주문·둥둥 뜨기)·
//! brain_study.rs (📖 책·🙋 손들기·👜 가방 청소), 빗자루는 broom_fly.rs·brain_broom.rs.
//!
//! - ✨ 주문 (pet.json `spells` + cast 그림): 한가할 때 초당 SPELL_RATE (🌱 power)로 SPELL_REACH 안의 대상에 지팡이를 휙 (CAST_SECS,
//!   CAST_HIT에 맞음). 다음 주문은 SPELL_CALM_SECS 뒤
//!   · Expelliarmus: 다른 펫이 든 물건을 떨어뜨림 · 아니면 바닥의 작은 물건을 휙 날림 (DISARM_VX·DISARM_VY)
//!   · Wingardium Leviosa: 작은 물건이나 작은 펫을 FLOAT_RISE만큼 둥둥 띄워 FLOAT_SECS 뒤 내려놓음 — 책벌레가 아니면 FIZZLE_CHANCE로 실패
//!   · Reparo: 바닥의 작은 물건 하나를 반짝 치움
//! - 📖 책 읽기 (책벌레): 초당 READ_RATE로 READ_SECS 동안 앉아 읽음, 가끔 STUDY_LINE_RATE로 "fascinating..."
//! - 🙋 "I know!": 초당 KNOW_RATE로 KNOW_SECS 손 번쩍
//! - 👜 가방 청소: 같은 디딤판에 물건이 BAG_MIN개 넘게 있으면 초당 BAG_RATE로 시작 — 가까운 것부터 하나씩 가방에 쏙 (BAG_STUFF_SECS, BAG_GULP_AT에 뿅).
//!   넣은 수마다 가방이 커짐 (bag_growth — BAG_FULL개에 다 큼), 가방이 BAG_DRAG_AT보다 크면 질질 끌며 느려짐 (BAG_DRAG_PACE).
//!   다 치우면 "Phew! All clean!" 하고 가방이 사라짐. 다음은 BAG_CALM_SECS 뒤

use serde::Deserialize;

/// ✨ 주문 이름 (pet.json `spells`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spell {
    /// 무장 해제 — 든 물건을 떨어뜨리거나 바닥의 물건을 휙 날림
    Expelliarmus,
    /// 둥둥 띄우기 — 물건·작은 펫
    Leviosa,
    /// 고치기 — 쓰레기 하나를 반짝 치움
    Reparo,
}

impl Spell {
    /// 이 주문의 비트
    fn bit(self) -> u8 {
        match self {
            Spell::Expelliarmus => 1,
            Spell::Leviosa => 2,
            Spell::Reparo => 4,
        }
    }
}

/// 아는 주문 모음 (PetConfig가 Copy라 비트로)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpellSet(u8);

impl SpellSet {
    /// 주문 목록으로 만듦
    pub fn of(spells: &[Spell]) -> SpellSet {
        SpellSet(spells.iter().fold(0, |b, s| b | s.bit()))
    }

    /// 이 주문을 앎
    pub fn has(self, spell: Spell) -> bool {
        self.0 & spell.bit() != 0
    }

    /// 아는 주문이 없음
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// 처음 주문을 쓸 수 있게 되는 때 (clock 초)
pub const SPELL_FIRST_SECS: f64 = 45.0;
/// 한가할 때 주문을 시도하는 빈도 (초당 — 🌱 power를 곱함)
pub const SPELL_RATE: f64 = 0.03;
/// 다음 주문까지 (초)
pub const SPELL_CALM_SECS: (f64, f64) = (40.0, 100.0);
/// 주문이 닿는 거리 (몸 크기 배 — 가로·세로 모두)
pub const SPELL_REACH: f64 = 5.0;
/// 지팡이를 휙 하는 시간 (cast 3칸 8fps + 끝에 잠깐)
pub const CAST_SECS: f64 = 0.9;
/// 주문이 맞는 때 (cast 셋째 칸)
pub const CAST_HIT: f64 = 0.3;
/// Expelliarmus로 바닥의 물건이 날아가는 빠르기 (논리 px/s — dpi를 곱함, 위가 -)
pub const DISARM_VX: f64 = 420.0;
pub const DISARM_VY: f64 = -380.0;
/// Leviosa로 뜨는 높이 (몸 크기 배 — 물건은 시전자 크기, 펫은 그 펫 크기)
pub const FLOAT_RISE: f64 = 0.9;
/// 떠 있는 시간 (오르기·내리기 포함, 초)
pub const FLOAT_SECS: (f64, f64) = (3.5, 5.5);
/// 떠오르고 내려오는 시간 (초)
pub const FLOAT_EASE_SECS: f64 = 0.9;
/// 띄울 수 있는 펫: 시전자 크기의 이 배까지
pub const FLOAT_PET_MAX: f64 = 1.15;
/// 책벌레가 아니면 Leviosa가 실패하는 몫 ("Leviosaaa?")
pub const FIZZLE_CHANCE: f64 = 0.35;
/// 쓸 수 있는 주문 중 펫을 노리는 몫 (가까이 펫이 있을 때)
pub const PET_TARGET_CHANCE: f64 = 0.45;

/// 📖 처음 책을 읽을 수 있게 되는 때 (clock 초)
pub const READ_FIRST_SECS: f64 = 20.0;
/// 📖 한가할 때 책을 펴는 빈도 (초당 — 꽤 자주)
pub const READ_RATE: f64 = 0.02;
/// 📖 읽는 시간 (초)
pub const READ_SECS: (f64, f64) = (14.0, 32.0);
/// 📖 읽다가 "fascinating..." 하는 빈도 (초당)
pub const STUDY_LINE_RATE: f64 = 0.04;
/// 📖 다 읽고 다시 펴기까지 (초)
pub const READ_CALM_SECS: (f64, f64) = (25.0, 70.0);
/// 🙋 "I know!" 빈도 (초당 — 🌱 power)
pub const KNOW_RATE: f64 = 0.006;
/// 🙋 손 번쩍 시간 (초)
pub const KNOW_SECS: f64 = 2.2;

/// 👜 처음 가방 청소를 할 수 있게 되는 때 (clock 초)
pub const BAG_FIRST_SECS: f64 = 50.0;
/// 👜 같은 디딤판에 이만큼 넘게 있으면 치움
pub const BAG_MIN: usize = 2;
/// 👜 치울 게 있을 때 시작하는 빈도 (초당 — 🌱 power)
pub const BAG_RATE: f64 = 0.02;
/// 👜 다 치우고 다음까지 (초)
pub const BAG_CALM_SECS: (f64, f64) = (90.0, 200.0);
/// 👜 가방에 넣는 시간 (bag_stuff 2칸 4fps), 그 가운데 물건이 뿅
pub const BAG_STUFF_SECS: f64 = 0.7;
pub const BAG_GULP_AT: f64 = 0.35;
/// 👜 물건 하나에 다가가는 시간 상한 (초 — 다 큰 가방을 끌고 화면 끝에서 끝까지도 넉넉히) — 넘으면 그 물건은 건너뜀
pub const BAG_APPROACH_SECS: f64 = 90.0;
/// 👜 이만큼 넣으면 가방이 다 큼
pub const BAG_FULL: u32 = 6;
/// 👜 처음 가방 크기 (bag_growth 몫 — 0이면 안 보임)
pub const BAG_START: f64 = 0.15;
/// 👜 이보다 크면 질질 끌기 (bag_drag 그림)
pub const BAG_DRAG_AT: f64 = 0.5;
/// 👜 다 큰 가방을 끌 때 걷기 빠르기 배 (작을 땐 1 — 사이는 고르게)
pub const BAG_DRAG_PACE: f64 = 0.55;
/// 👜 "Phew! All clean!" 하고 가방을 내려놓는 시간 (초)
pub const BAG_DONE_SECS: f64 = 1.4;

/// 👜 가방 크기 (0 = 없음 … 1 = 다 큼): 처음 BAG_START에서 넣은 수(n)만큼 커져 BAG_FULL개에 1
pub fn bag_growth(n: u32) -> f64 {
    BAG_START + (1.0 - BAG_START) * (f64::from(n) / f64::from(BAG_FULL)).min(1.0)
}

/// 👜 가방 크기에 따른 걷기 빠르기 배 (작으면 1, 다 크면 BAG_DRAG_PACE)
pub fn bag_pace(growth: f64) -> f64 {
    let k = ((growth - BAG_START) / (1.0 - BAG_START)).clamp(0.0, 1.0);
    1.0 + (BAG_DRAG_PACE - 1.0) * k
}

/// ✨ Leviosa로 뜬 높이 (rise 만큼, len초 동안): FLOAT_EASE_SECS 동안 부드럽게 올랐다가, 살랑 떠 있다가, 마지막에 부드럽게 내려옴
pub fn float_height(t: f64, len: f64, rise: f64) -> f64 {
    if !(0.0..len).contains(&t) || len <= 0.0 {
        return 0.0;
    }
    let ease = FLOAT_EASE_SECS.min(len / 2.0);
    let smooth = |u: f64| u * u * (3.0 - 2.0 * u);
    let k = if t < ease {
        smooth(t / ease)
    } else if t > len - ease {
        smooth((len - t) / ease)
    } else {
        1.0
    };
    // 떠 있는 동안 살랑 (높이의 6%)
    let bob = 1.0 + 0.06 * (t * 3.0).sin() * k;
    rise * k * bob
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spell_sets_know_only_their_spells() {
        let s = SpellSet::of(&[Spell::Expelliarmus, Spell::Leviosa]);
        assert!(s.has(Spell::Expelliarmus) && s.has(Spell::Leviosa) && !s.has(Spell::Reparo));
        assert!(SpellSet::default().is_empty() && !s.is_empty());
        let parsed: Vec<Spell> = serde_json::from_str(r#"["leviosa","reparo"]"#).unwrap();
        assert_eq!(parsed, vec![Spell::Leviosa, Spell::Reparo]);
    }

    #[test]
    fn the_bag_grows_with_each_item_and_slows_her_down() {
        assert!((bag_growth(0) - BAG_START).abs() < 1e-9);
        assert!(bag_growth(1) > bag_growth(0) && bag_growth(3) > bag_growth(1));
        assert!((bag_growth(BAG_FULL) - 1.0).abs() < 1e-9 && (bag_growth(BAG_FULL + 5) - 1.0).abs() < 1e-9);
        assert!((bag_pace(BAG_START) - 1.0).abs() < 1e-9);
        assert!((bag_pace(1.0) - BAG_DRAG_PACE).abs() < 1e-9);
        assert!(bag_pace(0.5) < 1.0 && bag_pace(0.5) > BAG_DRAG_PACE);
    }

    #[test]
    fn levitation_rises_gently_hovers_and_comes_down() {
        let (len, rise) = (4.0, 100.0);
        assert_eq!(float_height(0.0, len, rise), 0.0);
        assert!(float_height(0.2, len, rise) < float_height(0.6, len, rise));
        assert!((float_height(2.0, len, rise) - rise).abs() <= rise * 0.07);
        assert!(float_height(3.8, len, rise) < float_height(3.3, len, rise));
        assert_eq!(float_height(len, len, rise), 0.0);
        assert_eq!(float_height(-1.0, len, rise), 0.0);
    }
}
