//! 🧙 마법사 펫 (해리포터·헤르미온느·볼드모트)과 👻 디멘터의 주문·책·가방·결투·패트로누스·서리 숫자와 주문 이름.
//! 동작은 brain_spell.rs (주문·둥둥 뜨기)·brain_study.rs (📖 책·🙋 손들기·👜 가방 청소)·brain_dark.rs (🐍 볼드모트 — 💫 기절·나기니·hehehe)·
//! brain_dementor.rs (👻 시무룩·🥶 서리·도망)·brain_duel.rs (⚔️ 결투·🦌 패트로누스), 빗자루는 broom_fly.rs·brain_broom.rs.
//!
//! - ✨ 주문 (pet.json `spells` + cast 그림): 한가할 때 초당 SPELL_RATE (🌱 power)로 SPELL_REACH 안의 대상에 지팡이를 휙 (CAST_SECS,
//!   CAST_HIT에 맞음). 다음 주문은 SPELL_CALM_SECS 뒤
//!   · Expelliarmus: 다른 펫이 든 물건을 떨어뜨림 · 아니면 바닥의 작은 물건을 휙 날림 (DISARM_VX·DISARM_VY)
//!   · Wingardium Leviosa: 작은 물건이나 작은 펫을 FLOAT_RISE만큼 둥둥 띄워 FLOAT_SECS 뒤 내려놓음 — 책벌레가 아니면 FIZZLE_CHANCE로 실패
//!   · Reparo: 바닥의 작은 물건 하나를 반짝 치움
//!   · 🪶 Leviosa로 앱 아이콘: ICON_LIFT_CHANCE로 발 앞 (ICON_AHEAD)에 작업표시줄 아이콘을 뽑아 띄움, ICON_WAIT_SECS 안에 안 오면 보통 물건
//!   · 💡 Lumos: 주문 주사위가 맞으면 LUMOS_CHANCE로 LUMOS_SECS 동안 지팡이 끝 불빛 (LUMOS_GLOW 반지름)
//!   · 🌑 Nox: 주문 주사위가 맞으면 NOX_CHANCE로 지팡이 끝 불이 꺼지며 작은 검은 연기 (NOX_PUFF 크기, NOX_HOLD_SECS 동안)
//! - 📖 책 읽기 (책벌레): 초당 READ_RATE로 READ_SECS 동안 앉아 읽음, 가끔 STUDY_LINE_RATE로 "fascinating..."
//! - 🙋 "I know!": 초당 KNOW_RATE로 KNOW_SECS 손 번쩍
//! - 👜 가방 청소: 같은 디딤판에 물건이 BAG_MIN개 (하나라도) 있으면 초당 BAG_RATE로 시작 — 가까운 것부터 하나씩 가방에 쏙 (BAG_STUFF_SECS, BAG_GULP_AT에 뿅).
//!   넣은 수마다 가방이 커짐 (bag_growth — BAG_FULL개에 다 큼), 가방이 BAG_DRAG_AT보다 크면 질질 끌며 느려짐 (BAG_DRAG_PACE).
//!   다 치우면 "Phew! All clean!" 하고 가방이 사라짐. 다음은 BAG_CALM_SECS 뒤
//! - ⚡ Stupefy (볼드모트): 초록 빛줄기 (CAST_HIT 조금 앞부터 휘두르기 끝까지 보임)에 맞은 펫이 💫 FAINT_SECS 동안 기절 (처음 FAINT_DROP_SECS는 쿵 착지 그림)
//! - 🐍 나기니: 초당 NAGINI_RATE로 "Serpensortia!" — NAGINI_SECS 동안 발밑을 NAGINI_SPEED로 NAGINI_RANGE 안에서 스르륵, NAGINI_FADE_SECS로 나타나고 사라짐
//! - 😈 "hehehe": 한가할 때 초당 CACKLE_RATE
//! - ⚔️ 결투: 해리 (Expelliarmus)와 볼드모트 (Stupefy)가 가로 DUEL_RANGE·세로 DUEL_RISE 안에서 둘 다 한가하면 마주 봄 → DUEL_AIM_SECS 뒤
//!   빨강·초록 빛줄기가 뻗어 가운데에서 맞부딪혀 duel_clash로 밀고 당김 → 약 15초 한 판 (합·저주·맞음·피하기·달아나기 숫자는 duel_bout.rs) →
//!   숨 고르고 DUEL_PART_SECS 동안 흩어짐. Expelliarmus에 맞으면 지팡이가 WAND_FLY_SECS 동안 날아감. 다음은 DUEL_CALM_SECS 뒤
//! - 🎒 가방에 숨기: 볼드모트와 결투가 시작될 때 헤르미온느 곁 (TUCK_REACH)에 해리가 있으면 TUCK_CHANCE로 "In you go!" — HIDE_SECS 동안 가방 (HIDE_BAG 크기)을 벌려 들고
//!   해리는 TUCK_SECS 동안 작아지며 쏙 → 그 뒤 결투. 끝나면 해리가 TUCK_POP으로 튀어나옴 ("Phew!"), 길어도 TUCK_MAX_SECS 안에
//! - 🦌 패트로누스 (pet.json `patronus` — 해리 "stag" 사슴 · 헤르미온느 "otter" 수달, PatronusAnimal): 디멘터가 PATRONUS_REACH 안에 오거나,
//!   PATRONUS_SEE 안에서 다른 펫을 시무룩하게 하면 "Expecto Patronum!" — 은빛 동물이 STAG_RUN_SECS 동안 디멘터에게 달려가
//!   (수달은 OTTER_BOB만큼 물결치듯 오르내리며 헤엄침 — otter_bob) 닿으면 디멘터가 FLEE_SECS 동안 도망. 다음은 PATRONUS_CALM_SECS 뒤
//!   (영혼을 빼앗는 디멘터에겐 기다리지 않음)
//! - 👻 디멘터: GLOOM_REACH 안의 펫은 GLOOM_SECS 시무룩 (펫마다 GLOOM_CALM_SECS에 한 번) + 가까이 있는 동안 배가 GLOOM_HUNGER씩 더 고픔.
//!   화면 구석이 디멘터의 FROST_REACH (작업 영역 대각선 몫) 안이면 서리가 FROST_GROW로 끼고, 멀어지거나 도망가면 FROST_FADE로 녹음.
//!   새로 날 때 STALK_CHANCE로 다른 펫 머리 위로 다가감
//! - 👻 영혼 빼앗기: GLOOM_REACH 안에서 만난 펫에게 DRAIN_CHANCE로 (펫마다 DRAIN_AGAIN_SECS에 한 번 굴림, 끝나면 DRAIN_GAP_SECS 쉼)
//!   머리 위 (DRAIN_HOVER)에 떠서 DRAIN_SECS 동안 은빛 영혼 줄기 (SOUL_WIDTH) — 펫은 SOUL_RISE만큼 들려 버둥 → DRAIN_FAINT_SECS 기절.
//!   패트로누스가 오는 중이면 끝내지 못함 (DRAIN_MAX_SECS가 지나면 펫이 그냥 풀려남)

use serde::Deserialize;

/// 🦌🦦 패트로누스 동물 (pet.json `patronus`: "stag"·"otter" — 옛 true는 사슴)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatronusAnimal {
    /// 🦌 은빛 사슴 (해리)
    Stag,
    /// 🦦 은빛 수달 (헤르미온느)
    Otter,
}

/// 🏰 호그와트 친구 셋 (🔥 모닥불·📖 공부 방해 — social_campfire.rs·social_pester.rs가 봄)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hogwarts {
    /// 🧙 해리포터 (pet.json `patronus`)
    Harry,
    /// 📚 헤르미온느 (pet.json `bookworm`)
    Hermione,
    /// 🧔 해그리드 (pet.json `gamekeeper`)
    Hagrid,
}

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
    /// ⚡ 기절 (볼드모트) — 초록 빛줄기에 맞은 펫이 💫 잠깐 쓰러짐 · 물건은 휙 날림
    Stupefy,
    /// 🐍 뱀 부르기 (볼드모트 dark_lord — 아무 때나 고르지 않음, brain_dark.rs가 부름)
    Serpensortia,
    /// 🦌 패트로누스 (해리 patronus — 아무 때나 고르지 않음, 디멘터가 오면 brain_duel.rs가 부름)
    Patronus,
    /// 💡 지팡이 끝에 불 밝히기 (주문을 아는 마법사 누구나 — pet.json에 적지 않아도 됨)
    Lumos,
    /// 🌑 지팡이 끝 불 끄기 ("Nox!" — 작은 검은 연기, 주문을 아는 마법사 누구나 — pet.json에 적지 않아도 됨)
    Nox,
}

impl Spell {
    /// 이 주문의 비트
    fn bit(self) -> u8 {
        match self {
            Spell::Expelliarmus => 1,
            Spell::Leviosa => 2,
            Spell::Reparo => 4,
            Spell::Stupefy => 8,
            Spell::Serpensortia => 16,
            Spell::Patronus => 32,
            Spell::Lumos => 64,
            Spell::Nox => 128,
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
/// 🪶 주문 때 작업표시줄 앱 아이콘을 뽑아 띄우는 몫 (만들 수 있을 때) · 발 앞 자리 (몸 크기 배) · 아이콘을 기다리는 시간 (초)
pub const ICON_LIFT_CHANCE: f64 = 0.3;
pub const ICON_AHEAD: f64 = 1.2;
pub const ICON_WAIT_SECS: f64 = 2.5;
/// 💡 주문 때 대상 없이 Lumos를 켜는 몫 · 불빛 시간 (초) · 밝아지는·흐려지는 시간 · 빛 번짐 반지름 (몸 크기 배)
pub const LUMOS_CHANCE: f64 = 0.25;
pub const LUMOS_SECS: (f64, f64) = (3.0, 5.0);
pub const LUMOS_RAMP_SECS: f64 = 0.25;
pub const LUMOS_FADE_SECS: f64 = 0.6;
pub const LUMOS_GLOW: f64 = 0.9;
/// 🌑 주문 때 대상 없이 Nox를 거는 몫 (LUMOS_CHANCE 다음 몫) · 휘두른 뒤 연기가 퍼지는 동안 들고 있는 시간 (초) · 꺼지는 불이 흐려지는 시간 (초) ·
/// 연기 크기 (몸 크기 배)
pub const NOX_CHANCE: f64 = 0.15;
pub const NOX_HOLD_SECS: f64 = 1.0;
pub const NOX_SNUFF_SECS: f64 = 0.3;
pub const NOX_PUFF: f64 = 0.45;

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
pub const BAG_FIRST_SECS: f64 = 20.0;
/// 👜 같은 디딤판에 이만큼 (하나라도) 있으면 치움
pub const BAG_MIN: usize = 1;
/// 👜 치울 게 있을 때 시작하는 빈도 (초당 — 🌱 power)
pub const BAG_RATE: f64 = 0.06;
/// 👜 다 치우고 다음까지 (초)
pub const BAG_CALM_SECS: (f64, f64) = (40.0, 90.0);
/// 👜 가방에 넣는 시간 (bag_stuff 2칸 4fps), 그 가운데 물건이 뿅
pub const BAG_STUFF_SECS: f64 = 0.7;
pub const BAG_GULP_AT: f64 = 0.35;
/// 👜 물건 하나에 다가가는 시간 상한 (초 — 다 큰 가방을 끌고 화면 끝에서 끝까지도 넉넉히) — 넘으면 그 물건은 건너뜀
pub const BAG_APPROACH_SECS: f64 = 90.0;
/// 👜 이만큼 넣으면 가방이 다 큼
pub const BAG_FULL: u32 = 4;
/// 👜 처음 가방 크기 (bag_growth 몫 — 0이면 안 보임)
pub const BAG_START: f64 = 0.25;
/// 👜 이보다 크면 질질 끌기 (bag_drag 그림)
pub const BAG_DRAG_AT: f64 = 0.5;
/// 👜 다 큰 가방을 끌 때 걷기 빠르기 배 (작을 땐 1 — 사이는 고르게)
pub const BAG_DRAG_PACE: f64 = 0.55;
/// 👜 "Phew! All clean!" 하고 가방을 내려놓는 시간 (초)
pub const BAG_DONE_SECS: f64 = 1.4;

/// ⚡ Stupefy 빛줄기가 보이는 때: CAST_HIT 이만큼 앞부터 휘두르기가 끝날 때까지 (초 — 사라진 뒤 흐려지기는 덮개 창이)
pub const BOLT_LEAD_SECS: f64 = 0.08;
/// ⚡ 빛줄기 굵기 (몸 크기 배)
pub const BOLT_WIDTH: f64 = 0.06;
/// ⚔️ 결투 빛줄기 굵기 (몸 크기 배)
pub const DUEL_WIDTH: f64 = 0.08;
/// 💫 기절해 쓰러져 있는 시간 (초)
pub const FAINT_SECS: (f64, f64) = (2.5, 4.0);
/// 💫 처음 쿵 (land 그림) 시간 — 그 뒤엔 누워 (sleep 그림) 머리 위에 💫
pub const FAINT_DROP_SECS: f64 = 0.35;

/// 🐍 처음 나기니를 부를 수 있게 되는 때 (clock 초)
pub const NAGINI_FIRST_SECS: f64 = 60.0;
/// 🐍 한가할 때 부르는 빈도 (초당 — 🌱 power)
pub const NAGINI_RATE: f64 = 0.012;
/// 🐍 기어 다니는 시간 (초)
pub const NAGINI_SECS: (f64, f64) = (12.0, 22.0);
/// 🐍 사라진 뒤 다음까지 (초)
pub const NAGINI_CALM_SECS: (f64, f64) = (90.0, 200.0);
/// 🐍 기는 빠르기 (볼드모트 몸 크기 배 / 초)
pub const NAGINI_SPEED: f64 = 0.45;
/// 🐍 부른 자리에서 가로로 이만큼 (몸 크기 배) 안을 오감
pub const NAGINI_RANGE: f64 = 2.6;
/// 🐍 길이 (몸 크기 배 — 덮개 창이 그림)
pub const NAGINI_LEN: f64 = 1.7;
/// 🐍 스르륵 나타나고 사라지는 시간 (초)
pub const NAGINI_FADE_SECS: f64 = 0.8;
/// 😈 "hehehe" 빈도 (초당)
pub const CACKLE_RATE: f64 = 0.012;

/// ⚔️ 처음 결투할 수 있게 되는 때 (clock 초 — 이 사이에서 하나)
pub const DUEL_FIRST_SECS: (f64, f64) = (8.0, 25.0);
/// ⚔️ 마주칠 거리: 가로 (몸 크기 배 — 큰 쪽) 이만큼 안, 이만큼 넘게 떨어져야 함 · 세로 이만큼 안
pub const DUEL_RANGE: f64 = 16.0;
pub const DUEL_MIN: f64 = 1.2;
pub const DUEL_RISE: f64 = 2.5;
/// ⚔️ 마주 보고 지팡이를 겨누는 시간 (그 뒤 빛줄기가 DUEL_REACH_SECS 동안 뻗어 나감)
pub const DUEL_AIM_SECS: f64 = 0.6;
pub const DUEL_REACH_SECS: f64 = 0.35;
/// ⚔️ 다음 결투까지 (초 — 자주 보이게 줄임, 2026-11-23)
pub const DUEL_CALM_SECS: (f64, f64) = (45.0, 90.0);
/// ⚔️ 한 판 뒤 서로 반대쪽으로 걸어가는 시간 (초)
pub const DUEL_PART_SECS: (f64, f64) = (1.5, 2.5);
/// ⚔️ Expelliarmus에 맞아 지팡이가 날아가 떨어지는 시간 (초) · 지팡이 길이 (몸 크기 배)
pub const WAND_FLY_SECS: f64 = 0.7;
pub const WAND_SIZE: f64 = 0.32;

/// 🔥 모닥불 크기 (불 피운 해그리드 몸 크기 배 — 장작 폭)
pub const CAMPFIRE_SIZE: f64 = 0.45;

/// 🎒 헤르미온느 곁 이만큼 (몸 크기 배 — 큰 쪽, 가로) 안의 해리를 가방에 숨김 (세로는 DUEL_RISE)
pub const TUCK_REACH: f64 = 12.0;
/// 🎒 셋이 모였을 때 결투가 시작되면 가방 숨기기로 가는 몫 (나머지는 평소 결투)
pub const TUCK_CHANCE: f64 = 0.7;
/// 🎒 결투 앞에 가방을 벌려 드는 시간 (초 — 그동안 해리가 쏙 들어감, 빛줄기는 그 뒤부터)
pub const HIDE_SECS: f64 = 1.4;
/// 🎒 해리가 작아지며 가방으로 들어가는 시간 (초)
pub const TUCK_SECS: f64 = 0.8;
/// 🎒 결투 동안 든 가방 크기 (render.bag — 0.25 … 1)
pub const HIDE_BAG: f64 = 0.3;
/// 🎒 가방에 있는 가장 긴 시간 (초 — 결투가 꼬여도 이만큼이면 나옴)
pub const TUCK_MAX_SECS: f64 = 30.0;
/// 🎒 튀어나오는 빠르기 (논리 px/s — dpi를 곱함, 옆·위)
pub const TUCK_POP_VX: f64 = 120.0;
pub const TUCK_POP_VY: f64 = -460.0;

/// 🦌 디멘터가 해리에게 이만큼 (해리 몸 크기 배) 가까우면 패트로누스
pub const PATRONUS_REACH: f64 = 5.0;
/// 🦌 다른 펫을 시무룩하게 하는 디멘터가 해리에게서 이만큼 (해리 몸 크기 배) 안이면 패트로누스 (같은 화면)
pub const PATRONUS_SEE: f64 = 14.0;
/// 🦌 다음 패트로누스까지 (초)
pub const PATRONUS_CALM_SECS: (f64, f64) = (25.0, 50.0);
/// 🦌 은빛 사슴이 디멘터에게 달려가는 시간 · 모두 (닿은 뒤 그대로 달려 나가며 흐려짐, 초)
pub const STAG_RUN_SECS: f64 = 1.3;
pub const STAG_SECS: f64 = 2.4;
/// 🦌 사슴 크기 (해리 몸 크기 배)
pub const STAG_SIZE: f64 = 1.2;

/// 👻 디멘터 둘레 이만큼 (디멘터 몸 크기 배) 안의 펫이 시무룩
pub const GLOOM_REACH: f64 = 2.6;
/// 👻 시무룩 시간 · 같은 펫이 다시 시무룩해지기까지 (초)
pub const GLOOM_SECS: f64 = 3.5;
pub const GLOOM_CALM_SECS: (f64, f64) = (12.0, 25.0);
/// 👻 가까이 있는 동안 더 줄어드는 포만감 (초당 — 보통 1/1800의 세 배쯤)
pub const GLOOM_HUNGER: f64 = 1.0 / 600.0;
/// 👻 패트로누스에 쫓겨 도망가 있는 시간 (초 — 그동안 시무룩·서리·다가가기 없음)
pub const FLEE_SECS: (f64, f64) = (35.0, 60.0);
/// 👻 새로 날 때 다른 펫 머리 위로 다가가는 몫
pub const STALK_CHANCE: f64 = 0.6;
/// 🥶 서리가 끼는 구석: 디멘터에서 작업 영역 대각선의 이 몫 안
pub const FROST_REACH: f64 = 0.38;
/// 🥶 서리가 끼는 빠르기 · 녹는 빠르기 (초당 — 0…1)
pub const FROST_GROW: f64 = 1.0 / 6.0;
/// 🧹 빗자루로 쓴 구석은 이만큼 (초) 다시 서리가 끼지 않음
pub const FROST_SWEPT_SECS: f64 = 60.0;
pub const FROST_FADE: f64 = 1.0 / 40.0;
/// 🥶 다 낀 서리 크기 (작업 영역 대각선 몫)
pub const FROST_SIZE: f64 = 0.09;
/// 🖐️ 펫을 들고 문질러 닦은 자리가 다시 뿌예지기까지 (초)
pub const FROST_REFOG_SECS: f64 = 40.0;
/// 🖐️ 서리 하나에 남기는 닦은 자리 수 상한 (넘으면 오래된 것부터 버림)
pub const FROST_MAX_WIPES: usize = 48;
/// 🖐️ 닦은 자리끼리 이만큼 (닦는 반지름 몫) 안이면 새로 안 남기고 그 자리를 다시 닦음
pub const FROST_WIPE_GAP: f64 = 0.4;

/// 👻 만난 펫의 영혼을 빼앗으려 하는 몫 · 같은 펫에게 다시 주사위를 굴리기까지 (초) · 한 번 빼앗고 쉬는 시간 (초)
pub const DRAIN_CHANCE: f64 = 0.9;
pub const DRAIN_AGAIN_SECS: (f64, f64) = (15.0, 30.0);
pub const DRAIN_GAP_SECS: (f64, f64) = (4.0, 8.0);
/// 👻 영혼 줄기 시간 · 그 뒤 기절 시간 · 펫이 그냥 풀려나는 가장 긴 시간 (초 — 패트로누스가 오는 동안 늘어남)
pub const DRAIN_SECS: f64 = 1.8;
pub const DRAIN_FAINT_SECS: f64 = 3.0;
pub const DRAIN_MAX_SECS: f64 = 8.0;
/// 👻 펫 머리 위로 뜨는 높이 (디멘터 몸 크기 배 — 디멘터 발끝) · 다가가는 빠르기 (초당 남은 거리 몫)
pub const DRAIN_HOVER: f64 = 0.15;
pub const DRAIN_GLIDE: f64 = 5.0;
/// 👻 영혼 줄기 굵기 · 펫이 들리는 높이 (펫 몸 크기 배) · 줄기가 짙어지는 시간 (초)
pub const SOUL_WIDTH: f64 = 0.3;
pub const SOUL_RISE: f64 = 0.12;
pub const SOUL_RAMP_SECS: f64 = 0.4;
/// 🦦 수달이 오르내리는 폭 (시전자 몸 크기 배) · 한 번 오르내리는 빠르기 (초당)
pub const OTTER_BOB: f64 = 0.35;
pub const OTTER_BOB_HZ: f64 = 1.6;

/// 🦦 수달 물결 (−1…1, 나오는 0.4초 동안 커짐) · 그때 몸 기울기 (라디안 — 오를 땐 머리를 들고 내려갈 땐 숙임)
pub fn otter_bob(t: f64) -> (f64, f64) {
    let w = std::f64::consts::TAU * OTTER_BOB_HZ;
    let k = (t / 0.4).clamp(0.0, 1.0);
    ((t * w).sin() * k, -0.45 * (t * w).cos() * k)
}

/// 👻 영혼 줄기 짙기 (0…1): SOUL_RAMP_SECS 동안 짙어지고 살짝 일렁임
pub fn soul_level(t: f64) -> f64 {
    ((t / SOUL_RAMP_SECS).clamp(0.0, 1.0) * (0.9 + 0.1 * (t * 7.0).sin())).clamp(0.0, 1.0)
}

/// ⚔️ 맞부딪히는 자리 (빨강 쪽 지팡이 끝에서 초록 쪽까지의 몫 0…1): 가운데에서 시작해 밀고 당기며 출렁 (t초째, len초 결투)
pub fn duel_clash(t: f64, len: f64) -> f64 {
    let start = DUEL_AIM_SECS + DUEL_REACH_SECS;
    if t <= start || len <= start {
        return 0.5;
    }
    let u = t - start;
    let k = (u / 0.8).min(1.0);
    let push = 0.2 * (u * 2.3).sin() + 0.09 * (u * 5.1 + 0.7).sin();
    (0.5 + push * k).clamp(0.2, 0.8)
}

/// ⚔️ 빛줄기가 뻗어 나간 몫 (0 = 지팡이 끝 … 1 = 맞부딪힘): DUEL_AIM_SECS 동안 겨누고 DUEL_REACH_SECS 동안 뻗음
pub fn duel_reach(t: f64) -> f64 {
    ((t - DUEL_AIM_SECS) / DUEL_REACH_SECS).clamp(0.0, 1.0)
}

/// 🐍 나기니가 보이는 몫 (0…1): 처음·끝 NAGINI_FADE_SECS 동안 스르륵
pub fn nagini_alpha(t: f64, len: f64) -> f64 {
    if !(0.0..len).contains(&t) {
        return 0.0;
    }
    (t / NAGINI_FADE_SECS).min((len - t) / NAGINI_FADE_SECS).clamp(0.0, 1.0)
}

/// 🦌 사슴 자리 몫 (0 = 지팡이 끝 … 1 = 디멘터, 넘으면 그 너머로): 처음엔 빠르게, 닿을 때 조금 느려짐 · 보이는 몫
pub fn stag_progress(t: f64) -> f64 {
    if t <= STAG_RUN_SECS {
        let u = t / STAG_RUN_SECS;
        1.0 - (1.0 - u) * (1.0 - u)
    } else {
        1.0 + (t - STAG_RUN_SECS) / STAG_RUN_SECS * 0.8
    }
}

/// 🦌 사슴이 보이는 몫 (0…1): 0.2초 동안 나타나고 마지막 0.7초 동안 흐려짐
pub fn stag_alpha(t: f64) -> f64 {
    if !(0.0..STAG_SECS).contains(&t) {
        return 0.0;
    }
    (t / 0.2).min((STAG_SECS - t) / 0.7).clamp(0.0, 1.0)
}

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
    fn dark_spells_parse_and_have_their_own_bits() {
        let parsed: Vec<Spell> = serde_json::from_str(r#"["stupefy","serpensortia","patronus"]"#).unwrap();
        assert_eq!(parsed, vec![Spell::Stupefy, Spell::Serpensortia, Spell::Patronus]);
        let s = SpellSet::of(&[Spell::Stupefy]);
        assert!(s.has(Spell::Stupefy) && !s.has(Spell::Expelliarmus) && !s.has(Spell::Patronus) && !s.has(Spell::Serpensortia));
    }

    #[test]
    fn duel_beams_reach_out_then_the_clash_point_pushes_and_pulls_around_the_middle() {
        assert_eq!(duel_reach(0.0), 0.0);
        assert_eq!(duel_reach(DUEL_AIM_SECS), 0.0);
        assert!(duel_reach(DUEL_AIM_SECS + DUEL_REACH_SECS * 0.5) > 0.4);
        assert_eq!(duel_reach(DUEL_AIM_SECS + DUEL_REACH_SECS + 0.01), 1.0);
        assert_eq!(duel_clash(0.3, 4.0), 0.5);
        let shares: Vec<f64> = (0..400).map(|i| duel_clash(1.0 + i as f64 * 0.01, 5.0)).collect();
        let (lo, hi) = shares.iter().fold((1.0f64, 0.0f64), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(lo >= 0.2 && hi <= 0.8, "{lo} {hi}");
        assert!(lo < 0.4 && hi > 0.6, "밀고 당김: {lo} {hi}");
        for w in shares.windows(2) {
            assert!((w[1] - w[0]).abs() < 0.03, "부드럽게");
        }
    }

    #[test]
    fn nagini_fades_in_and_out_and_the_stag_runs_then_fades() {
        assert_eq!(nagini_alpha(0.0, 10.0), 0.0);
        assert!((nagini_alpha(5.0, 10.0) - 1.0).abs() < 1e-9);
        assert!(nagini_alpha(9.8, 10.0) < 0.5 && nagini_alpha(10.0, 10.0) == 0.0);
        assert_eq!(stag_progress(0.0), 0.0);
        assert!((stag_progress(STAG_RUN_SECS) - 1.0).abs() < 1e-9);
        assert!(stag_progress(STAG_RUN_SECS * 0.5) > 0.5, "처음엔 빠르게");
        assert!(stag_progress(STAG_SECS) > 1.2, "닿은 뒤 그 너머로");
        assert!(stag_alpha(0.0) == 0.0 && (stag_alpha(1.0) - 1.0).abs() < 1e-9 && stag_alpha(STAG_SECS - 0.1) < 0.2);
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
