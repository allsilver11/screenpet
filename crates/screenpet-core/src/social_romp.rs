//! 펫끼리 놀기 더 (2026-11-20 spec 1부) 의 짝·차례 맞추기 — social.rs의 자식 모듈 (Social의 비공개 필드에 접근). 펫 쪽 몸짓은 brain_romp.rs.
//!
//! 같은 디딤판 조각, 몸 몇 개 거리 안의 두 펫이 할 수 있는 놀이들의 몫(종류 기본 몫 × 성격 × 펫의 social_bias)을 더해
//! 1초에 ROMP_RATE × 몫의 합으로 하나를 시작함. 짝마다 ROMP_COOLDOWN, 펫마다 한 번에 하나 (engaged).
//! 나는 동물·조작·하우스·먹기·잡힘·집 생각 등 바쁜 펫은 끼지 않음 (Pet::romp_free — 기존 놀이와 같은 조건).
//!
//! - 🥊 투닥투닥 (Scuffle): 마주 보고 달려가 부딪힌 뒤 1.5~3초 한 덩어리로 데굴데굴 (가운데를 두고 빙글 — roll/annoyed 번갈아, 덜덜)
//!   "hey!" "mine!" "grr". 끝나면 튕겨 나가 진 쪽은 콰당 💫 "oof", 이긴 쪽은 손 흔들며 "I win!" (새침하면 흥 "hmph!")
//! - 😈 몰래 찌르기 (Prank): 쉬거나 자는 친구 뒤로 살금살금 → 갸웃 콕 → 친구는 화들짝 폴짝 "hey!", 장난꾼은 "hehe" 하며 한발 달아남 →
//!   친구가 몇 초 쫓아감 (기존 술래 놀이, 역할 반대). 자던 친구가 그 사이 깨면 그만둠
//! - 🫸 밀치기 (Shove): 친구에게 다가가 툭 "hey!" → 친구는 몸 1개쯤 미끄러지며 "oof". SHOVE_BACK_CHANCE로 되밀기 "you hey!", 최대 SHOVE_MAX번
//! - 😴 같이 낮잠 (Nap): 자는 친구 옆에 다가가 하트 한 번, 같이 잠 (평소 잠). 먼저 자던 친구가 깨면 1~3초 뒤 기지개 켜며 같이 깸
//! - 🏁 달리기 시합 (Race): 쉬던 둘이 나란히 서서 "ready…" "go!" → 먼 쪽 끝으로 (최대 RACE_MAX_BODIES) 달림, 빠르기 × 0.9~1.1 주사위.
//!   먼저 닿은 쪽 손 흔들기 + 반짝 "I win!", 진 쪽 annoyed "no fair!"
//! - 💬 수다 (Chat): 가까이 쉬는 둘이 마주 보고 말풍선 2~4번 번갈아 (chat_a ↔ chat_b), 가끔 끝에 하트
//! - 🌱 연령대 (spec 2부, stage.rs): 👶 졸졸 (Tag) — 어린 펫이 걷는 큰(나이 많은) 펫 뒤를 "wait for me!" 하며 몇 초 따라가고 하트.
//!   🧓 쓰다듬기 (Pat) — 나이 많은 펫이 쉬는 어린 펫에게 다가가 손을 흔들어 쓰다듬고 "good kid", 어린 펫은 하트
//!
//! 주사위는 따로 (다른 놀이의 주사위 순서는 그대로).
//! 같은 조각 판정 (same_piece)·자리 맞추기 (fit)는 🕷️🕷️🕷️ 멀티버스 모임 (social_multiverse.rs)도 씀.

use super::*;
use crate::brain::romp_moves::Pose;
use crate::brain::Anim;
use crate::brain::Speech;
use crate::behavior::Move;

/// 펫끼리 놀이 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RompKind {
    /// 🥊 투닥투닥
    Scuffle,
    /// 😈 몰래 찌르기
    Prank,
    /// 🫸 밀치기
    Shove,
    /// 😴 같이 낮잠
    Nap,
    /// 🏁 달리기 시합
    Race,
    /// 💬 수다
    Chat,
    /// 👶 졸졸 따라가기 (어린 쪽이 걸음, spec 2부)
    Tag,
    /// 🧓 쓰다듬기 (나이 많은 쪽이 걸음, spec 2부)
    Pat,
}

impl RompKind {
    /// 모든 종류
    pub const ALL: [RompKind; 8] = [RompKind::Scuffle, RompKind::Prank, RompKind::Shove, RompKind::Nap, RompKind::Race, RompKind::Chat, RompKind::Tag, RompKind::Pat];

    /// 종류의 기본 몫 (보통 성격 기준)
    fn base(self) -> f64 {
        match self {
            RompKind::Scuffle => 0.8,
            RompKind::Prank => 1.0,
            RompKind::Shove => 0.6,
            RompKind::Nap => 1.2,
            RompKind::Race => 1.0,
            RompKind::Chat => 1.2,
            RompKind::Tag => 1.0,
            RompKind::Pat => 1.0,
        }
    }

    /// 성격 배: 새침·장난꾸러기는 투닥투닥·밀치기, 장난꾸러기는 찌르기·시합, 다정하면 낮잠·수다
    fn temper(self, p: Personality) -> f64 {
        use Personality::*;
        match (self, p) {
            (RompKind::Scuffle, Friendly) => 0.6,
            (RompKind::Scuffle, _) => 1.4,
            (RompKind::Prank, Friendly) => 0.6,
            (RompKind::Prank, Playful) => 1.8,
            (RompKind::Prank, Aloof) => 0.8,
            (RompKind::Shove, Friendly) => 0.6,
            (RompKind::Shove, Playful) => 1.2,
            (RompKind::Shove, Aloof) => 1.4,
            (RompKind::Nap, Friendly) => 1.6,
            (RompKind::Nap, Playful) => 0.6,
            (RompKind::Nap, Aloof) => 0.3,
            (RompKind::Race, Friendly) => 1.0,
            (RompKind::Race, Playful) => 1.5,
            (RompKind::Race, Aloof) => 0.5,
            (RompKind::Chat, Friendly) => 1.4,
            (RompKind::Chat, Playful) => 1.0,
            (RompKind::Chat, Aloof) => 0.5,
            (RompKind::Tag, Aloof) => 0.6,
            (RompKind::Tag, _) => 1.2,
            (RompKind::Pat, Friendly) => 1.4,
            (RompKind::Pat, Playful) => 1.0,
            (RompKind::Pat, Aloof) => 0.6,
        }
    }

    /// 둘이 같이 하는 놀이 (먼저 거는 쪽이 따로 없음)
    fn mutual(self) -> bool {
        matches!(self, RompKind::Scuffle | RompKind::Race | RompKind::Chat)
    }
}

/// 몫 1일 때 1초에 놀이를 시작할 확률 — 할 수 있는 놀이 몫의 합을 곱함
pub const ROMP_RATE: f64 = 0.015;
/// 놀이를 걸 수 있는 거리 (두 몸 크기 평균 배, 같은 디딤판 조각)
pub const ROMP_BODIES: f64 = 4.0;
/// 몰래 찌르기를 걸 수 있는 거리 (살금살금은 느려서 더 가까이)
pub const PRANK_BODIES: f64 = 2.5;
/// 수다를 걸 수 있는 거리
pub const CHAT_BODIES: f64 = 3.0;
/// 같은 짝이 다시 펫끼리 놀이를 하기까지 (초)
pub const ROMP_COOLDOWN: (f64, f64) = (40.0, 80.0);

/// 🥊 다가가는 빠르기 (걷기 속도 배), 다가가는 시간 상한
const SCUFFLE_CLOSE_PACE: f64 = 1.6;
/// 🥊 다가가는 시간 상한 (초)
const SCUFFLE_CLOSE_SECS: f64 = 2.5;
/// 🥊 데굴데굴 시간 (초)
pub const SCUFFLE_SECS: (f64, f64) = (1.5, 3.0);
/// 🥊 덩어리가 도는 빠르기 (rad/s)
const SCUFFLE_SPIN: f64 = std::f64::consts::TAU * 1.2;
/// 🥊 덩어리가 들썩이는 높이 (몸 크기 배)
const SCUFFLE_LIFT: f64 = 0.15;
/// 🥊 그림을 바꾸는 틈 (초)
const SCUFFLE_FLIP_SECS: f64 = 0.2;
/// 🥊 "mine!" "grr" 사이 (초)
const SCUFFLE_LINE_SECS: f64 = 0.8;
/// 🥊 진 쪽이 튕겨 나가는 거리 (몸 크기 배), 시간 (초)
const SCUFFLE_FLING_BODIES: f64 = 0.8;
/// 🥊 진 쪽이 튕겨 나가는 시간 (초)
const SCUFFLE_FLING_SECS: f64 = 0.4;
/// 🥊 끝 — 털썩·뻐김 시간 (초)
const SCUFFLE_APART_SECS: f64 = 1.6;

/// 😈 살금살금 빠르기 (걷기 속도 배 — 평소 걷기 0.7보다 한참 느림)
pub const SNEAK_PACE: f64 = 0.33;
/// 😈 살금살금 시간 상한 (초)
const SNEAK_SECS: f64 = 15.0;
/// 😈 콕 (갸웃) 시간 (초)
const POKE_SECS: f64 = 0.5;
/// 😈 화들짝 (폴짝) 시간 (초), 폴짝 높이 (몸 크기 배)
const STARTLE_SECS: f64 = 0.8;
/// 😈 화들짝 폴짝 시간 (초)
const STARTLE_HOP_SECS: f64 = 0.5;
/// 😈 화들짝 폴짝 높이 (몸 크기 배)
const STARTLE_RISE: f64 = 0.25;
/// 😈 찔린 쪽이 쫓아가는 시간 (초)
pub const PRANK_CHASE_SECS: (f64, f64) = (3.0, 5.0);

/// 🫸 밀치기를 걸 수 있는 거리 (두 몸 크기 평균 배)
pub const SHOVE_BODIES: f64 = 3.0;
/// 🫸 다가가는 빠르기 (걷기 속도 배 — 성큼성큼)
const SHOVE_CLOSE_PACE: f64 = 1.2;
/// 🫸 다가가는 시간 상한 (초)
const SHOVE_CLOSE_SECS: f64 = 6.0;
/// 🫸 밀린 쪽이 미끄러지는 거리 (몸 크기 배)
pub const SHOVE_SLIDE_BODIES: f64 = 1.0;
/// 🫸 미끄러지는 시간 (초)
const SHOVE_SLIDE_SECS: f64 = 0.5;
/// 🫸 한 번 밀기 전체 (미끄러짐 + 털고 일어남, 초)
const SHOVE_SECS: f64 = 1.1;
/// 🫸 툭 내미는 시간 (초)
const SHOVE_LUNGE_SECS: f64 = 0.25;
/// 🫸 되밀 확률
pub const SHOVE_BACK_CHANCE: f64 = 0.3;
/// 🫸 주고받는 밀기 상한 (처음 밀기 포함)
pub const SHOVE_MAX: u8 = 3;

/// 😴 다가가는 시간 상한 (초)
const NAP_CLOSE_SECS: f64 = 10.0;
/// 😴 붙어서 하트 (초)
const SNUGGLE_SECS: f64 = 0.8;
/// 😴 친구가 깬 뒤 같이 깨기까지 (초)
pub const NAP_WAKE_SECS: (f64, f64) = (1.0, 3.0);

/// 🏁 달릴 거리 상한·하한 (몸 크기 배)
pub const RACE_MAX_BODIES: f64 = 8.0;
/// 🏁 이보다 짧게밖에 못 달리면 시합 안 함 (몸 크기 배)
pub const RACE_MIN_BODIES: f64 = 3.0;
/// 🏁 출발선에 서는 시간 상한 (초)
const RACE_LINE_SECS: f64 = 4.0;
/// 🏁 "ready…" 에서 "go!" 까지 (초)
const RACE_READY_SECS: f64 = 1.2;
/// 🏁 달리기 빠르기 주사위 (달리기 속도 배)
pub const RACE_PACE: (f64, f64) = (0.9, 1.1);
/// 🏁 달리는 시간 상한 (초)
const RACE_RUN_SECS: f64 = 15.0;
/// 🏁 끝 — 뻐김·투덜 시간 (초)
const RACE_FINISH_SECS: f64 = 1.8;

/// 💬 주고받는 말 수
pub const CHAT_LINES: (u8, u8) = (2, 4);
/// 💬 말 사이 (초)
const CHAT_LINE_SECS: f64 = 1.6;
/// 💬 끝에 하트를 띄울 확률
const CHAT_HEART_CHANCE: f64 = 0.35;

/// 👶 따라가는 시간 (초)
pub const TAG_SECS: (f64, f64) = (4.0, 8.0);
/// 👶 큰 펫이 걸어가는 거리 상한 (몸 크기 배)
const TAG_WALK_BODIES: f64 = 6.0;
/// 👶 큰 펫 뒤 간격 (두 몸 크기 평균 배)
const TAG_GAP: f64 = 0.9;
/// 👶 큰 펫 걷기 빠르기 (걷기 속도 배 — 평소 걷기) · 따라가는 쪽 (조금 빨라 따라잡음)
const TAG_LEAD_PACE: f64 = 0.7;
/// 👶 따라가는 쪽 빠르기 (걷기 속도 배)
const TAG_FOLLOW_PACE: f64 = 0.85;
/// 🧓 다가가는 시간 상한 (초)
const PAT_CLOSE_SECS: f64 = 8.0;
/// 🧓 쓰다듬는 시간 (초)
pub const PAT_SECS: f64 = 1.8;

/// 놀이 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Step {
    /// 다가감 (투닥투닥·밀치기·낮잠)
    Close,
    /// 🥊 데굴데굴
    Brawl,
    /// 🥊 튕겨 나가 털썩·뻐김
    Apart,
    /// 😈 살금살금
    Sneak,
    /// 😈 콕
    Poke,
    /// 😈 화들짝
    Startle,
    /// 🫸 툭 — 미끄러짐
    Push,
    /// 😴 붙어서 하트
    Snuggle,
    /// 😴 같이 잠
    Nap,
    /// 😴 친구가 깼음 — 곧 깸
    Wake,
    /// 🏁 출발선으로
    Line,
    /// 🏁 ready…
    Ready,
    /// 🏁 달림
    Run,
    /// 🏁 결과
    Finish,
    /// 💬 수다
    Talk,
    /// 👶 졸졸 따라감
    Follow,
    /// 🧓 쓰다듬음
    Pet,
}

/// 하는 중인 놀이 하나
#[derive(Debug, Clone, Copy)]
struct Romp {
    kind: RompKind,
    /// 건 쪽 (장난꾼·미는 쪽·낮잠 가는 쪽) / 상대
    a: u64,
    b: u64,
    step: Step,
    /// 이 단계에 든 뒤 흐른 시간
    t: f64,
    /// 이 단계 길이 (주사위)
    len: f64,
    /// 센 것: 말 수·밀기 수
    n: u8,
    /// 정해 둔 수: 수다 말 수
    m: u8,
    /// 자리: 🥊 가운데 → 튕겨 나갈 자리, 😈 찌를 자리, 🫸 미끄러질 자리, 😴 누울 자리, 🏁 결승선
    x: f64,
    /// 🏁 출발선
    from: f64,
    /// 쪽: 🥊 a가 선 쪽 (가운데에서), 🫸 미는 쪽, 🏁 달리는 쪽
    dir: f64,
    /// 🏁 빠르기 (a, b — 걷기 속도 배)
    pace: (f64, f64),
    /// a가 이겼음
    a_wins: bool,
    /// 지금 몸짓을 받는 쪽 (State::Romp여야 함 — 아니면 놀이가 깨진 것)
    a_in: bool,
    b_in: bool,
}

/// 코디네이터의 기억
pub(super) struct SocialRomp {
    romps: Vec<Romp>,
    cooldown: HashMap<(u64, u64), f64>,
    rng: fastrand::Rng,
    /// 1초 확률 배 (테스트에서 바꿈)
    rate: f64,
    /// 테스트: 이 놀이만
    only: Option<RompKind>,
    /// 되밀 확률 (테스트에서 바꿈)
    shove_back: f64,
    /// 지금까지 시작한 놀이 수
    started: usize,
    /// 짝마다 할 수 있는 놀이를 모으는 자리 (틱마다 새로 할당하지 않게 다시 씀)
    options: Vec<(RompKind, usize, usize, f64)>,
}

impl SocialRomp {
    /// seed로 주사위를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SocialRomp {
            romps: Vec::new(),
            cooldown: HashMap::new(),
            rng: fastrand::Rng::with_seed(seed ^ 0x726f_6d70),
            rate: ROMP_RATE,
            only: None,
            shove_back: SHOVE_BACK_CHANCE,
            started: 0,
            options: Vec::new(),
        }
    }
}

/// 같은 디딤판 조각 위 (같은 높이)
pub(super) fn same_piece(a: &Snap, b: &Snap) -> bool {
    a.owner.is_some() && a.owner == b.owner && (a.y - b.y).abs() <= SAME_LEVEL_PX && a.ground.is_some() && a.ground == b.ground
}

/// s가 설 수 있게 x를 조각 안으로
pub(super) fn fit(s: &Snap, x: f64) -> f64 {
    match s.ground {
        Some((lo, hi)) if hi - lo > 2.0 * s.half_w => x.clamp(lo + s.half_w, hi - s.half_w),
        _ => s.x,
    }
}

/// x가 s가 설 수 있는 자리임
fn fits(s: &Snap, x: f64) -> bool {
    (fit(s, x) - x).abs() < 0.5
}

/// 두 몸 크기 평균
fn body(a: &Snap, b: &Snap) -> f64 {
    (a.size + b.size) / 2.0
}

/// from에서 to 쪽 (±1, 같으면 fallback)
fn toward(from: f64, to: f64, fallback: f64) -> f64 {
    if (to - from).abs() > 0.5 {
        (to - from).signum()
    } else {
        fallback
    }
}

/// 🏁 a·b가 서 있는 조각에서 달릴 쪽과 결승선, 출발선 (자리가 모자라면 None)
pub(super) fn race_course(a: &Snap, b: &Snap) -> Option<(f64, f64, f64)> {
    let (lo, hi) = a.ground?;
    let half = a.half_w.max(b.half_w);
    let from = (a.x + b.x) / 2.0;
    let (right, left) = (hi - half - from, from - (lo + half));
    let (dir, room) = if right >= left { (1.0, right) } else { (-1.0, left) };
    let size = body(a, b);
    (room >= RACE_MIN_BODIES * size).then(|| (dir, from + dir * room.min(RACE_MAX_BODIES * size), from))
}

impl Social {
    /// 짝 (init이 거는 쪽)에게 이 놀이의 몫: 기본 몫 × 성격 배 × 펫의 social_bias (같이 하는 놀이는 더 하고 싶은 쪽)
    fn kind_weight<T: AsRef<Pet>>(&self, pets: &[T], kind: RompKind, init: usize, other: usize) -> f64 {
        let (pi, po) = (self.snap[init].personality, self.snap[other].personality);
        let (bi, bo) = (pets[init].as_ref().social_bias(kind), pets[other].as_ref().social_bias(kind));
        if kind.mutual() {
            kind.base() * (kind.temper(pi) * bi).max(kind.temper(po) * bo)
        } else {
            kind.base() * kind.temper(pi) * bi
        }
    }

    /// 테스트: 지금 자리에서 init이 other에게 거는 이 놀이의 몫
    #[cfg(test)]
    pub(crate) fn romp_weight<T: AsRef<Pet>>(&self, pets: &[T], kind: RompKind, init: usize, other: usize) -> f64 {
        let snap: Vec<Snap> = pets.iter().map(|p| Snap::of(p.as_ref())).collect();
        let me = Social { snap, ..Social::new(0) };
        me.kind_weight(pets, kind, init, other)
    }

    /// i가 j에게 걸 수 있는 놀이들을 out에 (같이 하는 놀이는 i < j 쪽에서만)
    fn romp_options<T: AsRef<Pet>>(&self, pets: &[T], i: usize, j: usize, out: &mut Vec<(RompKind, usize, usize, f64)>) {
        let (a, b) = (self.snap[i], self.snap[j]);
        let (d, size) = ((a.x - b.x).abs(), body(&a, &b));
        let mut add = |kind: RompKind| {
            if self.romp.only.is_none_or(|k| k == kind) {
                let w = self.kind_weight(pets, kind, i, j);
                if w > 0.0 {
                    out.push((kind, i, j, w));
                }
            }
        };
        if !a.romp_free {
            return;
        }
        let both = b.romp_free && d <= ROMP_BODIES * size;
        if both && i < j {
            add(RompKind::Scuffle);
            // 시합·수다는 한쪽이라도 쉬고 있으면 (걷던 쪽이 멈춰 섬)
            let calm = a.resting || b.resting;
            if calm && race_course(&a, &b).is_some() {
                add(RompKind::Race);
            }
            if calm && d <= CHAT_BODIES * size {
                add(RompKind::Chat);
            }
        }
        if both && d <= SHOVE_BODIES * size {
            add(RompKind::Shove);
        }
        if ((b.romp_free && b.resting) || b.sleeping) && d <= PRANK_BODIES * size {
            add(RompKind::Prank);
        }
        if b.sleeping && d <= ROMP_BODIES * size {
            add(RompKind::Nap);
        }
        // 🌱 연령대: 👶 어린 쪽이 걷는 큰 펫을 졸졸 · 🧓 나이 많은 쪽이 쉬는 어린 펫을 쓰다듬음
        if b.romp_free && d <= ROMP_BODIES * size {
            if a.stage.step() < b.stage.step() && !b.resting {
                add(RompKind::Tag);
            }
            if a.stage.step() > b.stage.step() && b.resting {
                add(RompKind::Pat);
            }
        }
    }

    /// 새 펫끼리 놀이: 같은 조각의 한가한 짝마다 할 수 있는 놀이 몫을 더해 굴리고, 걸리면 몫대로 하나를 시작함
    pub(super) fn find_romps<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let n = self.snap.len();
        let mut options = std::mem::take(&mut self.romp.options);
        for i in 0..n {
            for j in (i + 1)..n {
                let (a, b) = (self.snap[i], self.snap[j]);
                // 👶 곁에 친구가 있음 — 혼자 오래 있어 "waah" 하는 시계를 되돌림 (brain_stage.rs)
                if same_piece(&a, &b) && (a.x - b.x).abs() <= ROMP_BODIES * body(&a, &b) {
                    pets[i].as_mut().note_company();
                    pets[j].as_mut().note_company();
                }
                if a.engaged || b.engaged || !same_piece(&a, &b) || cooling(&self.romp.cooldown, &pair(a.key, b.key), self.clock) {
                    continue;
                }
                options.clear();
                self.romp_options(pets, i, j, &mut options);
                self.romp_options(pets, j, i, &mut options);
                let total: f64 = options.iter().map(|o| o.3).sum();
                if total <= 0.0 || self.romp.rng.f64() >= self.romp.rate * total * dt {
                    continue;
                }
                let mut roll = self.romp.rng.f64() * total;
                let pick = options.iter().copied().find(|o| {
                    roll -= o.3;
                    roll < 0.0
                });
                let (kind, init, other, _) = pick.unwrap_or(options[options.len() - 1]);
                if self.start_romp(pets, kind, init, other) {
                    self.romp.started += 1;
                    self.snap[i].engaged = true;
                    self.snap[j].engaged = true;
                }
            }
        }
        self.romp.options = options;
    }

    /// 놀이를 시작함 (init이 건 쪽). 자리가 없으면 false (📖 공부 방해 뒤 놀이도 이걸로 — social_pester.rs)
    pub(super) fn start_romp<T: AsRef<Pet> + AsMut<Pet>>(&mut self, pets: &mut [T], kind: RompKind, ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        let mut r = Romp { kind, a: a.key, b: b.key, step: Step::Close, t: 0.0, len: 0.0, n: 0, m: 0, x: 0.0, from: 0.0, dir: 1.0, pace: (1.0, 1.0), a_wins: true, a_in: true, b_in: true };
        match kind {
            RompKind::Scuffle => {
                r.x = (a.x + b.x) / 2.0;
                r.dir = toward(b.x, a.x, -1.0);
                let touch = 0.5 * (a.half_w + b.half_w);
                pets[ia].as_mut().start_romp(Pose::go(Anim::Run, fit(&a, r.x + r.dir * touch / 2.0), SCUFFLE_CLOSE_PACE).facing(-r.dir));
                pets[ib].as_mut().start_romp(Pose::go(Anim::Run, fit(&b, r.x - r.dir * touch / 2.0), SCUFFLE_CLOSE_PACE).facing(r.dir));
                pets[ia].as_mut().say_now(Speech::Scuffle);
                r.n = 1;
            }
            RompKind::Prank => {
                let touch = 0.85 * (a.half_w + b.half_w);
                let behind = b.x - b.facing * touch;
                r.x = if fits(&a, behind) { behind } else { b.x + b.facing * touch };
                if !fits(&a, r.x) {
                    return false;
                }
                r.step = Step::Sneak;
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, r.x, SNEAK_PACE));
                if b.sleeping {
                    r.b_in = false;
                } else {
                    pets[ib].as_mut().start_romp(Pose::hold(Anim::Idle).facing(b.facing));
                }
            }
            RompKind::Shove => {
                r.dir = toward(a.x, b.x, a.facing);
                pets[ib].as_mut().start_romp(Pose::hold(Anim::Idle).facing(b.facing));
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, a.x, SHOVE_CLOSE_PACE));
            }
            RompKind::Nap => {
                let touch = a.half_w + b.half_w;
                let side = toward(b.x, a.x, 1.0);
                r.x = if fits(&a, b.x + side * touch) { b.x + side * touch } else { b.x - side * touch };
                if !fits(&a, r.x) {
                    return false;
                }
                r.b_in = false;
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, r.x, 0.7));
            }
            RompKind::Race => {
                let Some((dir, finish, from)) = race_course(&a, &b) else { return false };
                (r.dir, r.x, r.from) = (dir, finish, from);
                r.step = Step::Line;
                let (sa, sb) = self.race_slots(&r, &a, &b);
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, sa, 1.0));
                pets[ib].as_mut().start_romp(Pose::go(Anim::Walk, sb, 1.0));
            }
            RompKind::Chat => {
                r.step = Step::Talk;
                r.m = self.romp.rng.u8(CHAT_LINES.0..=CHAT_LINES.1);
                pets[ia].as_mut().start_romp(Pose::hold(Anim::Idle).facing(toward(a.x, b.x, a.facing)));
                pets[ib].as_mut().start_romp(Pose::hold(Anim::Idle).facing(toward(b.x, a.x, b.facing)));
            }
            RompKind::Tag => {
                // 큰 펫(b)이 가던 쪽으로 걸어가고 어린 펫(a)이 그 뒤를 따라감 — 갈 자리가 모자라면 안 함
                r.dir = b.facing;
                r.x = fit(&b, b.x + r.dir * TAG_WALK_BODIES * b.size);
                if (r.x - b.x).abs() < b.size {
                    return false;
                }
                r.step = Step::Follow;
                r.len = behavior::range(&mut self.romp.rng, TAG_SECS);
                pets[ib].as_mut().start_romp(Pose::go(Anim::Walk, r.x, TAG_LEAD_PACE));
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, fit(&a, b.x - r.dir * TAG_GAP * body(&a, &b)), TAG_FOLLOW_PACE));
                pets[ia].as_mut().say_now(Speech::TagAlong);
            }
            RompKind::Pat => {
                // 나이 많은 펫(a)이 어린 펫(b) 옆으로 걸어감
                let touch = 0.9 * (a.half_w + b.half_w);
                let side = toward(b.x, a.x, 1.0);
                r.x = if fits(&a, b.x + side * touch) { b.x + side * touch } else { b.x - side * touch };
                if !fits(&a, r.x) {
                    return false;
                }
                pets[ib].as_mut().start_romp(Pose::hold(Anim::Idle).facing(toward(b.x, a.x, b.facing)));
                pets[ia].as_mut().start_romp(Pose::go(Anim::Walk, r.x, 0.7));
            }
        }
        self.romp.romps.push(r);
        true
    }

    /// 🏁 출발선의 두 자리 (나란히 — 원래 왼쪽에 있던 쪽이 왼쪽)
    fn race_slots(&self, r: &Romp, a: &Snap, b: &Snap) -> (f64, f64) {
        let off = 0.2 * body(a, b);
        let side = toward(b.x, a.x, -1.0);
        (fit(a, r.from + side * off), fit(b, r.from - side * off))
    }

    /// 하는 중인 놀이를 한 틱 진행함: 몸짓을 받던 쪽이 놀이를 벗어났으면 (잡힘·볼일 등) 남은 쪽을 풀어 줌
    pub(super) fn step_romps<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, pets: &mut [T]) {
        let mut k = 0;
        while k < self.romp.romps.len() {
            let mut r = self.romp.romps[k];
            let (ia, ib) = (self.index(r.a), self.index(r.b));
            let live = |i: Option<usize>, inside: bool| i.filter(|&i| !inside || pets[i].as_ref().in_romp());
            let (Some(ia), Some(ib)) = (live(ia, r.a_in), live(ib, r.b_in)) else {
                self.end_romp_at(k, pets);
                continue;
            };
            self.snap[ia].engaged = true;
            self.snap[ib].engaged = true;
            r.t += dt;
            let going = match r.kind {
                RompKind::Scuffle => self.step_scuffle(&mut r, pets, ia, ib),
                RompKind::Prank => self.step_prank(&mut r, pets, ia, ib),
                RompKind::Shove => self.step_shove(&mut r, pets, ia, ib),
                RompKind::Nap => self.step_nap(&mut r, pets, ia, ib),
                RompKind::Race => self.step_race(&mut r, pets, ia, ib),
                RompKind::Chat => self.step_chat(&mut r, pets, ia, ib),
                RompKind::Tag => self.step_tag(&mut r, pets, ia, ib),
                RompKind::Pat => self.step_pat(&mut r, pets, ia, ib),
            };
            self.romp.romps[k] = r;
            if going {
                k += 1;
            } else {
                self.end_romp_at(k, pets);
            }
        }
    }

    /// 놀이 k를 끝냄: 아직 몸짓을 받는 쪽은 쉬게 하고, 짝에 쿨다운 (내려서자마자 인사하지도 않음)
    fn end_romp_at<T: AsRef<Pet> + AsMut<Pet>>(&mut self, k: usize, pets: &mut [T]) {
        let r = self.romp.romps.swap_remove(k);
        for key in [r.a, r.b] {
            if let Some(i) = self.index(key) {
                pets[i].as_mut().end_romp();
                self.snap[i].engaged = true;
            }
        }
        let now = self.clock;
        let key = pair(r.a, r.b);
        let wait = behavior::range(&mut self.romp.rng, ROMP_COOLDOWN);
        cool(&mut self.romp.cooldown, key, now, wait);
        if !self.links.iter().any(|l| pair(l.a, l.b) == key) {
            let greet = behavior::range(&mut self.romp.rng, GREET_COOLDOWN);
            self.cooldown.insert(key, now + greet);
        }
    }

    /// 다음 단계로 (시간을 0부터)
    fn next(r: &mut Romp, step: Step) {
        r.step = step;
        r.t = 0.0;
    }

    /// 🥊 투닥투닥 한 틱. 끝나면 false
    fn step_scuffle<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        let touch = 0.5 * (a.half_w + b.half_w);
        match r.step {
            Step::Close => {
                let (xa, xb) = (fit(&a, r.x + r.dir * touch / 2.0), fit(&b, r.x - r.dir * touch / 2.0));
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Run, xa, SCUFFLE_CLOSE_PACE).facing(-r.dir));
                pets[ib].as_mut().set_romp_pose(Pose::go(Anim::Run, xb, SCUFFLE_CLOSE_PACE).facing(r.dir));
                if ((a.x - xa).abs() < 2.0 && (b.x - xb).abs() < 2.0) || r.t >= SCUFFLE_CLOSE_SECS {
                    Self::next(r, Step::Brawl);
                    r.len = behavior::range(&mut self.romp.rng, SCUFFLE_SECS);
                }
            }
            Step::Brawl => {
                let th = r.t * SCUFFLE_SPIN;
                let (xa, xb) = (fit(&a, r.x + r.dir * touch * th.cos()), fit(&b, r.x - r.dir * touch * th.cos()));
                let flip = ((r.t / SCUFFLE_FLIP_SECS) as u32).is_multiple_of(2);
                let (anim_a, anim_b) = if flip { (Anim::Roll, Anim::Annoyed) } else { (Anim::Annoyed, Anim::Roll) };
                let ball = |anim, x, other: f64, lift: f64| Pose { lift: SCUFFLE_LIFT * lift, wobble: true, ..Pose::go(anim, x, f64::INFINITY).facing(toward(x, other, 1.0)) };
                pets[ia].as_mut().set_romp_pose(ball(anim_a, xa, xb, th.sin().abs()));
                pets[ib].as_mut().set_romp_pose(ball(anim_b, xb, xa, th.cos().abs()));
                if r.t >= SCUFFLE_LINE_SECS * f64::from(r.n) {
                    let who = if r.n % 2 == 0 { ia } else { ib };
                    pets[who].as_mut().say_now(Speech::Scuffle);
                    r.n += 1;
                }
                if r.t >= r.len {
                    Self::next(r, Step::Apart);
                    r.a_wins = self.romp.rng.bool();
                    let (l, w) = if r.a_wins { (b, a) } else { (a, b) };
                    let out = toward(r.x, l.x, if r.a_wins { -r.dir } else { r.dir });
                    r.x = fit(&l, l.x + out * SCUFFLE_FLING_BODIES * l.size);
                    r.n = 0;
                    let iw = if r.a_wins { ia } else { ib };
                    if w.personality == Personality::Aloof {
                        pets[iw].as_mut().say_now(Speech::Hmph);
                    } else {
                        pets[iw].as_mut().say_now(Speech::IWin);
                    }
                }
            }
            Step::Apart => {
                let (iw, il) = if r.a_wins { (ia, ib) } else { (ib, ia) };
                let (w, l) = (self.snap[iw], self.snap[il]);
                let gloat = if w.personality == Personality::Aloof { Anim::Huff } else { Anim::Wave };
                pets[iw].as_mut().set_romp_pose(Pose::hold(gloat));
                let back = toward(r.x, l.x, 1.0);
                if r.t < SCUFFLE_FLING_SECS {
                    let pace = SCUFFLE_FLING_BODIES * l.size / SCUFFLE_FLING_SECS / pets[il].as_ref().cfg.walk_speed.max(1.0);
                    pets[il].as_mut().set_romp_pose(Pose::go(Anim::Roll, r.x, pace).facing(back));
                } else {
                    if r.n == 0 {
                        pets[il].as_mut().say_now(Speech::Oof);
                        r.n = 1;
                    }
                    pets[il].as_mut().set_romp_pose(Pose { thud: true, ..Pose::hold(Anim::Land).facing(back) });
                }
                if r.t >= SCUFFLE_APART_SECS {
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    /// 😈 몰래 찌르기 한 틱. 끝나면 false (찔린 쪽이 쫓기 시작했거나 그만둠)
    fn step_prank<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        if !r.b_in && !pets[ib].as_ref().is_sleeping() {
            // 자던 친구가 먼저 깼다
            return false;
        }
        let at_b = toward(a.x, b.x, a.facing);
        match r.step {
            Step::Sneak => {
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Walk, r.x, SNEAK_PACE));
                if r.b_in {
                    pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(b.facing));
                }
                if (a.x - r.x).abs() < 2.0 {
                    Self::next(r, Step::Poke);
                } else if r.t >= SNEAK_SECS {
                    return false;
                }
            }
            Step::Poke => {
                pets[ia].as_mut().set_romp_pose(Pose::hold(Anim::Tilt).facing(at_b));
                if r.b_in {
                    pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(b.facing));
                }
                if r.t >= POKE_SECS {
                    Self::next(r, Step::Startle);
                    if !r.b_in {
                        pets[ib].as_mut().start_romp(Pose::hold(Anim::Hop));
                        r.b_in = true;
                    }
                    pets[ib].as_mut().say_now(Speech::Hey);
                    pets[ia].as_mut().say_now(Speech::Hehe);
                }
            }
            Step::Startle => {
                let hop = crate::play::hop_height(r.t, STARTLE_HOP_SECS, STARTLE_RISE);
                let anim = if r.t < STARTLE_HOP_SECS { Anim::Hop } else { Anim::Annoyed };
                pets[ib].as_mut().set_romp_pose(Pose { lift: hop, ..Pose::hold(anim).facing(-at_b) });
                // 장난꾼은 키득대며 한발 먼저 달아남 (바로 잡히지 않게)
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Run, fit(&a, a.x - at_b * a.size), Move::Run.speed_mult()));
                if r.t >= STARTLE_SECS {
                    // 찔린 쪽이 술래 — 장난꾼은 도망 (기존 술래 놀이로 넘김)
                    let secs = behavior::range(&mut self.romp.rng, PRANK_CHASE_SECS);
                    pets[ib].as_mut().start_chase(a.at(), secs);
                    pets[ia].as_mut().start_flee(b.at(), secs);
                    self.links.push(Link { a: a.key, b: b.key, phase: Phase::Chase { chaser: b.key, remaining: secs }, played: true, swapped: true });
                    // 이번 틱의 술래 놀이 차례(step_links)가 새 역할을 보게
                    for i in [ia, ib] {
                        self.snap[i] = Snap { engaged: true, ..Snap::of(pets[i].as_ref()) };
                    }
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    /// 🫸 밀치기 한 틱. 끝나면 false
    fn step_shove<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        let touch = 0.8 * (a.half_w + b.half_w);
        match r.step {
            Step::Close => {
                let contact = fit(&a, b.x - r.dir * touch);
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Walk, contact, SHOVE_CLOSE_PACE));
                pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(b.facing));
                if (a.x - contact).abs() < 2.0 {
                    Self::next(r, Step::Push);
                    pets[ia].as_mut().say_now(if r.n == 0 { Speech::Hey } else { Speech::HeyBack });
                    pets[ib].as_mut().say_now(Speech::Oof);
                    r.x = fit(&b, b.x + r.dir * SHOVE_SLIDE_BODIES * b.size);
                    r.from = a.x;
                } else if r.t >= SHOVE_CLOSE_SECS {
                    return false;
                }
            }
            Step::Push => {
                let lunge = if r.t < SHOVE_LUNGE_SECS { Pose::go(Anim::Walk, fit(&a, r.from + r.dir * 0.1 * a.size), 1.5) } else { Pose::hold(Anim::Idle) };
                pets[ia].as_mut().set_romp_pose(lunge.facing(r.dir));
                let slide = if r.t < SHOVE_SLIDE_SECS {
                    let pace = SHOVE_SLIDE_BODIES * b.size / SHOVE_SLIDE_SECS / pets[ib].as_ref().cfg.walk_speed.max(1.0);
                    Pose::go(Anim::Fall, r.x, pace)
                } else {
                    Pose::hold(Anim::Land)
                };
                pets[ib].as_mut().set_romp_pose(slide.facing(-r.dir));
                if r.t >= SHOVE_SECS {
                    r.n += 1;
                    if r.n >= SHOVE_MAX || self.romp.rng.f64() >= self.romp.shove_back {
                        return false;
                    }
                    // 되밀기: 밀린 쪽이 미는 쪽
                    (r.a, r.b) = (r.b, r.a);
                    r.dir = -r.dir;
                    Self::next(r, Step::Close);
                }
            }
            _ => return false,
        }
        true
    }

    /// 😴 같이 낮잠 한 틱. 끝나면 false
    fn step_nap<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        let sleeper_up = !pets[ib].as_ref().is_sleeping();
        match r.step {
            Step::Close => {
                if sleeper_up {
                    return false;
                }
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Walk, r.x, 0.7));
                if (a.x - r.x).abs() < 2.0 {
                    Self::next(r, Step::Snuggle);
                    pets[ia].as_mut().romp_heart();
                } else if r.t >= NAP_CLOSE_SECS {
                    return false;
                }
            }
            Step::Snuggle => {
                if sleeper_up {
                    return false;
                }
                pets[ia].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(toward(a.x, b.x, a.facing)));
                if r.t >= SNUGGLE_SECS {
                    pets[ia].as_mut().start_nap();
                    r.a_in = false;
                    Self::next(r, Step::Nap);
                }
            }
            Step::Nap => {
                if !pets[ia].as_ref().is_sleeping() {
                    return false;
                }
                if sleeper_up {
                    Self::next(r, Step::Wake);
                    r.len = behavior::range(&mut self.romp.rng, NAP_WAKE_SECS);
                }
            }
            Step::Wake => {
                if !pets[ia].as_ref().is_sleeping() {
                    return false;
                }
                if r.t >= r.len {
                    pets[ia].as_mut().wake_from_nap();
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    /// 🏁 달리기 시합 한 틱. 끝나면 false
    fn step_race<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        match r.step {
            Step::Line => {
                let (sa, sb) = self.race_slots(r, &a, &b);
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Walk, sa, 1.0));
                pets[ib].as_mut().set_romp_pose(Pose::go(Anim::Walk, sb, 1.0));
                if ((a.x - sa).abs() < 2.0 && (b.x - sb).abs() < 2.0) || r.t >= RACE_LINE_SECS {
                    Self::next(r, Step::Ready);
                    pets[ia].as_mut().say_now(Speech::Ready);
                }
            }
            Step::Ready => {
                pets[ia].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(r.dir));
                pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(r.dir));
                if r.t >= RACE_READY_SECS {
                    Self::next(r, Step::Run);
                    pets[ib].as_mut().say_now(Speech::Go);
                    let run = Move::Run.speed_mult();
                    r.pace = (behavior::range(&mut self.romp.rng, RACE_PACE) * run, behavior::range(&mut self.romp.rng, RACE_PACE) * run);
                }
            }
            Step::Run => {
                let (fa, fb) = (fit(&a, r.x), fit(&b, r.x));
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Run, fa, r.pace.0).facing(r.dir));
                pets[ib].as_mut().set_romp_pose(Pose::go(Anim::Run, fb, r.pace.1).facing(r.dir));
                let (da, db) = ((a.x - fa).abs(), (b.x - fb).abs());
                if da < 1.0 || db < 1.0 || r.t >= RACE_RUN_SECS {
                    r.a_wins = da < db || (da == db && self.romp.rng.bool());
                    Self::next(r, Step::Finish);
                    let (iw, il) = if r.a_wins { (ia, ib) } else { (ib, ia) };
                    pets[iw].as_mut().say_now(Speech::IWin);
                    pets[il].as_mut().say_now(Speech::NoFair);
                }
            }
            Step::Finish => {
                let (iw, il) = if r.a_wins { (ia, ib) } else { (ib, ia) };
                pets[iw].as_mut().set_romp_pose(Pose { sparkle: true, ..Pose::hold(Anim::Wave) });
                pets[il].as_mut().set_romp_pose(Pose::hold(Anim::Annoyed));
                if r.t >= RACE_FINISH_SECS {
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    /// 💬 수다 한 틱. 끝나면 false
    fn step_chat<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        // 지금 말하는 쪽은 갸웃, 듣는 쪽은 쳐다봄
        let speaking_a = r.n > 0 && (r.n - 1) % 2 == 0;
        let (anim_a, anim_b) = match (r.n, speaking_a) {
            (0, _) => (Anim::Idle, Anim::Idle),
            (_, true) => (Anim::Tilt, Anim::Idle),
            (_, false) => (Anim::Idle, Anim::Tilt),
        };
        pets[ia].as_mut().set_romp_pose(Pose::hold(anim_a).facing(toward(a.x, b.x, a.facing)));
        pets[ib].as_mut().set_romp_pose(Pose::hold(anim_b).facing(toward(b.x, a.x, b.facing)));
        let due = 0.3 + CHAT_LINE_SECS * f64::from(r.n);
        if r.n < r.m && r.t >= due {
            if r.n % 2 == 0 {
                pets[ia].as_mut().say_now(Speech::ChatA);
            } else {
                pets[ib].as_mut().say_now(Speech::ChatB);
            }
            r.n += 1;
        } else if r.n >= r.m && r.t >= due {
            if self.romp.rng.f64() < CHAT_HEART_CHANCE {
                pets[ia].as_mut().romp_heart();
                pets[ib].as_mut().romp_heart();
            }
            return false;
        }
        true
    }

    /// 👶 졸졸 한 틱: 큰 펫은 걸어가고 어린 펫은 그 뒤 간격을 지킴. 시간이 다 되거나 큰 펫이 닿으면 하트 하고 끝 (false)
    fn step_tag<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        let behind = fit(&a, b.x - r.dir * TAG_GAP * body(&a, &b));
        pets[ib].as_mut().set_romp_pose(Pose::go(Anim::Walk, r.x, TAG_LEAD_PACE));
        let follow = if (a.x - behind).abs() > 1.0 { Pose::go(Anim::Walk, behind, TAG_FOLLOW_PACE) } else { Pose::hold(Anim::Idle).facing(r.dir) };
        pets[ia].as_mut().set_romp_pose(follow);
        if r.t >= r.len || (b.x - r.x).abs() < 2.0 {
            pets[ia].as_mut().romp_heart();
            return false;
        }
        true
    }

    /// 🧓 쓰다듬기 한 틱: 다가가서 손 흔들어 쓰다듬고 "good kid", 어린 펫은 하트. 끝나면 false
    fn step_pat<T: AsRef<Pet> + AsMut<Pet>>(&mut self, r: &mut Romp, pets: &mut [T], ia: usize, ib: usize) -> bool {
        let (a, b) = (self.snap[ia], self.snap[ib]);
        match r.step {
            Step::Close => {
                pets[ia].as_mut().set_romp_pose(Pose::go(Anim::Walk, r.x, 0.7));
                pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Idle).facing(toward(b.x, a.x, b.facing)));
                if (a.x - r.x).abs() < 2.0 {
                    Self::next(r, Step::Pet);
                    pets[ia].as_mut().say_now(Speech::GoodKid);
                    pets[ib].as_mut().romp_heart();
                } else if r.t >= PAT_CLOSE_SECS {
                    return false;
                }
            }
            Step::Pet => {
                pets[ia].as_mut().set_romp_pose(Pose::hold(Anim::Wave).facing(toward(a.x, b.x, a.facing)));
                pets[ib].as_mut().set_romp_pose(Pose::hold(Anim::Heart));
                if r.t >= PAT_SECS {
                    return false;
                }
            }
            _ => return false,
        }
        true
    }

    /// 테스트: 펫끼리 놀이 확률 배를 바꿈
    #[cfg(test)]
    pub(crate) fn set_romp_rate(&mut self, rate: f64) {
        self.romp.rate = rate;
    }

    /// 테스트: 이 놀이만 함
    #[cfg(test)]
    pub(crate) fn set_romp_only(&mut self, kind: Option<RompKind>) {
        self.romp.only = kind;
    }

    /// 테스트: 되밀 확률
    #[cfg(test)]
    pub(crate) fn set_shove_back(&mut self, p: f64) {
        self.romp.shove_back = p;
    }

    /// 테스트: 하는 중인 펫끼리 놀이 수
    #[cfg(test)]
    pub(crate) fn romps_active(&self) -> usize {
        self.romp.romps.len()
    }

    /// 테스트: 지금까지 시작한 펫끼리 놀이 수
    #[cfg(test)]
    pub(crate) fn romps_started(&self) -> usize {
        self.romp.started
    }

    /// 테스트: 인사·술래 놀이 시작을 끔
    #[cfg(test)]
    pub(crate) fn hold_greets(&mut self, _secs: f64) {
        self.greets_off = true;
    }
}
