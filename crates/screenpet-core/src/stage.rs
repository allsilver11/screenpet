//! 🌱 연령대별 특기 (2026-11-20 spec 2부)의 숫자 — 펫 쪽 동작은 brain_stage.rs, 펫끼리(🧓 쓰다듬기 · 👶 졸졸)는 social_romp.rs.
//!
//! - 사용자 결정: 특징 행동은 모든 연령대가 다 함. 그 연령대에서 훨씬 자주 — 연령대 배 (stage_weight: 제 연령대 ×1.0, 한 단계 차이 ×0.35, 두 단계 이상 ×0.15)
//! - 행동마다 "제 연령대"(StageTrait::home)와 기본 확률(제 연령대 기준)이 있고, 실제 확률 = 기본 × 연령대 배
//! - 종별 특기의 세기: stage_power (아기 0.6 → 청소년 1.0 → 어른 1.2) — 거미집 크기·줄 수, 쿵 범위, 공룡·고양이·다람쥐·흐물이·펭귄 빈도 등.
//!   빠르기·높이는 덜 변하게 soft_power (청소년은 둘 다 1 — 예전 그대로)
//! - 펫끼리 놀이 성향: social_bias = 바닥 + 늘임 × 연령대 배 (투닥투닥은 청소년, 찌르기·시합은 어린이, 낮잠·수다는 아기·어른 …)
//! - 레벨업 뒤 새 연령대의 대표 특기 한 줄 (signature_line)

use crate::care::Stage;
use crate::social::RompKind;

/// 연령대 차이별 배 (같음 · 한 단계 · 두 단계 이상)
pub const STAGE_WEIGHTS: [f64; 3] = [1.0, 0.35, 0.15];

/// own 연령대의 펫이 home 연령대의 특징 행동을 하는 배 (STAGE_WEIGHTS 표)
pub fn stage_weight(own: Stage, home: Stage) -> f64 {
    let gap = own.step().abs_diff(home.step()) as usize;
    STAGE_WEIGHTS[gap.min(STAGE_WEIGHTS.len() - 1)]
}

/// 종별 특기의 세기 (빈도·크기·횟수에 곱함) — 청소년 1.0이 예전 그대로
pub fn stage_power(stage: Stage) -> f64 {
    match stage {
        Stage::Baby => 0.6,
        Stage::Kinder => 0.75,
        Stage::Child => 0.9,
        Stage::Teenager => 1.0,
        Stage::Adult => 1.2,
    }
}

/// 빠르기·높이처럼 덜 변해야 하는 세기 (stage_power의 절반만 움직임 — 아기 0.8, 어른 1.1)
pub fn soft_power(stage: Stage) -> f64 {
    0.5 + 0.5 * stage_power(stage)
}

/// 공통 연령대 특징 행동
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageTrait {
    /// 👶 아장아장: 걸음이 느리고 가끔 휘청 (wobble)
    Toddle,
    /// 👶 콩 넘어져 엉덩방아 (sit_front + "oops!") — 걷다가
    Plop,
    /// 👶 더 자주 졸고 오래 잠
    Sleepy,
    /// 👶 큰 펫 뒤를 졸졸 (social_romp.rs RompKind::Tag)
    Tagalong,
    /// 👶 혼자 오래 있으면 "waah"
    Waah,
    /// 🧒 위를 쳐다보며 "ooh?"
    Ooh,
    /// 🧒 제자리 폴짝폴짝 신남
    HappyHop,
    /// 🧒 커서 놀이 ↑
    CursorFan,
    /// 🧑 걷다가 깡총깡총
    HopSkip,
    /// 🧑 술래잡기 ↑
    TagFan,
    /// 🧑‍🎤 점프하며 공중제비 ("yeah!")
    Flip,
    /// 🧑‍🎤 시무룩 (annoyed "ugh")
    Sulk,
    /// 🧑‍🎤 전력 질주 (걷기 대신 달리기)
    Sprint,
    /// 🧓 어린 펫 쓰다듬기 (social_romp.rs RompKind::Pat)
    Pat,
    /// 🧓 여유롭게 기지개
    Stretch,
}

impl StageTrait {
    /// 모든 특징 행동
    pub const ALL: [StageTrait; 15] = [
        StageTrait::Toddle,
        StageTrait::Plop,
        StageTrait::Sleepy,
        StageTrait::Tagalong,
        StageTrait::Waah,
        StageTrait::Ooh,
        StageTrait::HappyHop,
        StageTrait::CursorFan,
        StageTrait::HopSkip,
        StageTrait::TagFan,
        StageTrait::Flip,
        StageTrait::Sulk,
        StageTrait::Sprint,
        StageTrait::Pat,
        StageTrait::Stretch,
    ];

    /// 이 행동이 가장 잦은 연령대
    pub fn home(self) -> Stage {
        use StageTrait::*;
        match self {
            Toddle | Plop | Sleepy | Tagalong | Waah => Stage::Baby,
            Ooh | HappyHop | CursorFan => Stage::Kinder,
            HopSkip | TagFan => Stage::Child,
            Flip | Sulk | Sprint => Stage::Teenager,
            Pat | Stretch => Stage::Adult,
        }
    }

    /// 제 연령대에서의 기본 확률·세기. 단위는 행동마다: 쉬는 박자당 확률 (Ooh·HappyHop·Flip·Sulk·Stretch·Waah),
    /// 걷는 동안 1초당 (Toddle 휘청·Plop), 걷기 토막당 (HopSkip·Sprint), 늘어나는 몫 (Sleepy·CursorFan·TagFan — 1 + 이 값 × 배),
    /// 펫끼리 놀이 몫 (Tagalong·Pat — social_bias)
    pub fn base(self) -> f64 {
        use StageTrait::*;
        match self {
            Toddle => 0.15,
            Plop => 0.03,
            Sleepy => 0.4,
            Tagalong => 1.0,
            Waah => 0.15,
            Ooh => 0.12,
            HappyHop => 0.10,
            CursorFan => 0.8,
            HopSkip => 0.35,
            TagFan => 0.6,
            Flip => 0.10,
            Sulk => 0.05,
            Sprint => 0.2,
            Pat => 1.0,
            Stretch => 0.10,
        }
    }

    /// stage 펫에게 이 행동의 실제 확률·세기 (기본 × 연령대 배)
    pub fn rate(self, stage: Stage) -> f64 {
        self.base() * stage_weight(stage, self.home())
    }
}

/// 👶 아장아장: 걷기 빠르기가 이 몫만큼 (× 연령대 배) 느려짐
pub const TODDLE_SLOW: f64 = 0.3;
/// 👶 휘청 한 번 (초)
pub const TODDLE_WOBBLE_SECS: f64 = 0.4;
/// 👶 오래 자는 몫 (잠 시간 × (1 + 이 값 × 연령대 배))
pub const SLEEPY_LONGER: f64 = 0.25;
/// 👶 혼자 이만큼 (초) 있으면 가끔 "waah" — 한 번 울면 다시 셈. 다른 연령대는 이 시간 ÷ 연령대 배만큼 (더 오래) 혼자여야 함
pub const WAAH_ALONE_SECS: f64 = 300.0;

/// stage 펫이 "waah" 할 만큼 혼자였던 시간 (초)
pub fn lonely_secs(stage: Stage) -> f64 {
    WAAH_ALONE_SECS / stage_weight(stage, Stage::Baby)
}

/// 👶 엉덩방아: 휘청 넘어지는 시간 · 전체 (초)
pub const PLOP_TRIP_SECS: f64 = 0.35;
/// 👶 엉덩방아 전체 (초)
pub const PLOP_SECS: f64 = 2.0;
/// 👶 "waah" 하며 앉아 있는 시간 (초)
pub const WAAH_SECS: f64 = 2.5;
/// 🧒 "ooh?" 하며 올려다보는 시간 (초)
pub const OOH_SECS: (f64, f64) = (1.5, 2.5);
/// 🧒 신나서 폴짝 한 번 (초), 폴짝 수, 높이 (몸 크기 배)
pub const HAPPY_HOP_SECS: f64 = 0.45;
/// 🧒 신나서 폴짝 수
pub const HAPPY_HOPS: u32 = 2;
/// 🧒 신나서 폴짝 높이 (몸 크기 배)
pub const HAPPY_HOP_RISE: f64 = 0.22;
/// 🧑‍🎤 공중제비 전체 (초) · 높이 (몸 크기 배)
pub const FLIP_SECS: f64 = 0.9;
/// 🧑‍🎤 공중제비 높이 (몸 크기 배)
pub const FLIP_RISE: f64 = 0.8;
/// 🧑‍🎤 공중제비에서 구르는 구간 (전체 시간의 몫: 시작, 끝)
pub const FLIP_ROLL: (f64, f64) = (0.15, 0.8);
/// 🧑‍🎤 시무룩 시간 (초)
pub const SULK_SECS: (f64, f64) = (2.0, 3.0);
/// 🧓 기지개 시간 (초)
pub const STRETCH_SECS: (f64, f64) = (2.5, 3.5);

/// 펫끼리 놀이 성향의 바닥·늘임: social_bias = 바닥 + 늘임 × 연령대 배 (제 연령대 1.4, 한 단계 0.75, 먼 연령대 0.55)
pub const BIAS_FLOOR: f64 = 0.4;
/// 펫끼리 놀이 성향의 늘임
pub const BIAS_GAIN: f64 = 1.0;
/// 👶🧓 아기·어른은 투닥투닥·밀치기를 덜 함 (싸움 없음·말리기 — 금지가 아니라 배)
pub const CALM_FIGHT_MULT: f64 = 0.6;

/// 펫끼리 놀이 종류마다 가장 좋아하는 연령대들
fn romp_homes(kind: RompKind) -> &'static [Stage] {
    match kind {
        RompKind::Scuffle | RompKind::Shove => &[Stage::Teenager],
        RompKind::Prank | RompKind::Race => &[Stage::Child],
        RompKind::Nap | RompKind::Chat => &[Stage::Baby, Stage::Adult],
        RompKind::Tag => &[Stage::Baby],
        RompKind::Pat => &[Stage::Adult],
    }
}

/// stage 펫이 이 펫끼리 놀이를 얼마나 좋아하나 (몫에 곱함)
pub fn social_bias(stage: Stage, kind: RompKind) -> f64 {
    let w = romp_homes(kind).iter().map(|&home| stage_weight(stage, home)).fold(0.0, f64::max);
    let calm = matches!(kind, RompKind::Scuffle | RompKind::Shove) && matches!(stage, Stage::Baby | Stage::Adult);
    (BIAS_FLOOR + BIAS_GAIN * w) * if calm { CALM_FIGHT_MULT } else { 1.0 }
}

/// 레벨업 뒤 말풍선 한 줄 — 새 연령대의 대표 특기 (짧은 영어)
pub fn signature_line(stage: Stage) -> &'static str {
    match stage {
        Stage::Baby => "new: 🍼 toddles!",
        Stage::Kinder => "new: 🐇 happy hops!",
        Stage::Child => "new: 😈 pranks!",
        Stage::Teenager => "new: 🤸 flips!",
        Stage::Adult => "new: 🤗 head pats!",
    }
}
