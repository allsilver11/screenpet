//! ⚔️ 길고 치열한 결투 짜기 (0.7.2 — 약 BOUT_SECS초): 두 펫이 같은 판 (Bout)을 하나씩 들고 같은 시계로 따라 함 (brain_duel.rs).
//!
//! - 판 = 합 (Step)들 + 마지막 숨 고르기 (REST_SECS): 첫 합은 늘 ⚔️ 빛줄기 맞부딪힘 (Lock — 끝에 진 쪽이 맞음),
//!   그 뒤 저주 주고받기 (Shot — 맞음·피함), 셋째 합은 늘 🏃 달아나기 (Chase — 달아나던 쪽이 뒤돌아 쏨, 쫓던 쪽이 맞거나 피함)
//! - 맞으면 (Hit): KNOCK_SECS 동안 뒤로 튕겨 → HIT_FAINT_SECS (2초) 💫 기절·뻣뻣 → GETUP_SECS 동안 일어나 다시
//! - 피하면 (Dodge): 폴짝 (Hop — 낮게 오는 저주) / 몸 숙이기 (Duck — 높게 오는 저주)
//! - 🪄 저주 이름 (curse_of): 해리 (빨강) "Expelliarmus!" (지팡이가 날아감) · 헤르미온느 (금빛) "Petrificus Totalus!" (통나무처럼 뻣뻣하게 쿵) ·
//!   볼드모트 (초록) "Stupefy!"·"Avada Kedavra!"를 번갈아 (해롭지 않은 장난 — 튕겨 기절)
//! - act: 판·내 쪽·시각으로 지금 할 일 (Act)을 냄 — 순수 함수라 두 펫이 따로 불러도 늘 맞물림
//!
//! 주사위는 판을 짤 때만 (부른 쪽의 결투 주사위).

use crate::magic::BeamColor;

/// ⚔️ 결투 한 판 길이 (숨 고르기까지, 초 — 이 사이에서 하나, 합 길이에 따라 조금 넘을 수 있음)
pub const BOUT_SECS: (f64, f64) = (14.0, 16.0);
/// ⚔️ 빛줄기 맞부딪힘 길이 (겨누기·뻗기 포함 — wizard::DUEL_AIM_SECS·DUEL_REACH_SECS, 초)
pub const LOCK_SECS: (f64, f64) = (2.2, 2.8);
/// 🪄 저주를 쏘는 때 (지팡이 휙 — wizard::CAST_HIT와 같음) · 날아가는 시간 (초)
pub const SHOT_FIRE: f64 = 0.3;
pub const BOLT_FLY_SECS: f64 = 0.3;
/// 🪄 지팡이를 휘두르는 시간 (cast 그림, 초)
pub const SHOT_SECS: f64 = 0.9;
/// 💥 맞고 뒤로 튕기는 시간 · 💫 기절·뻣뻣 (2초) · 일어나는 시간 (초)
pub const KNOCK_SECS: f64 = 0.35;
pub const HIT_FAINT_SECS: f64 = 2.0;
pub const GETUP_SECS: f64 = 0.4;
/// 💥 튕겨 나가는 거리 · 높이 (몸 크기 배)
pub const KNOCK_DIST: f64 = 0.8;
pub const KNOCK_RISE: f64 = 0.35;
/// 🤸 피하기: 저주가 닿기 이만큼 앞부터 DODGE_SECS 동안 (초) · 폴짝 높이 (몸 크기 배)
pub const DODGE_LEAD: f64 = 0.2;
pub const DODGE_SECS: f64 = 0.55;
pub const HOP_RISE: f64 = 0.6;
/// 🏃 달아나는 시간 (초) · 뒤돌아서는 시간 · 달아나는·쫓는 빠르기 (몸 크기 배 / 초) · 쫓는 쪽이 멈추는 거리 (몸 크기 배)
pub const RUN_SECS: (f64, f64) = (1.6, 2.4);
pub const TURN_SECS: f64 = 0.25;
pub const RUN_SPEED: f64 = 1.7;
pub const CHASE_SPEED: f64 = 1.5;
pub const CHASE_GAP: f64 = 1.6;
/// 합 사이 잠깐 (초)
pub const PAUSE_SECS: f64 = 0.35;
/// 😮‍💨 끝에 둘 다 숨 고르는 시간 (초)
pub const REST_SECS: f64 = 2.2;
/// 저주가 맞는 몫 (아니면 피함) · 둘째 맞부딪힘이 나오는 몫 (판에 둘까지)
pub const HIT_CHANCE: f64 = 0.45;
pub const LOCK_CHANCE: f64 = 0.2;
const MAX_LOCKS: usize = 2;

/// 결투하는 쪽
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 밝은 쪽 (해리 빨강·헤르미온느 금빛)
    Light,
    /// 볼드모트 (초록)
    Dark,
}

impl Side {
    /// 상대 쪽
    pub fn other(self) -> Side {
        match self {
            Side::Light => Side::Dark,
            Side::Dark => Side::Light,
        }
    }
}

/// 🪄 결투 저주
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curse {
    /// 해리 — 빨강, 맞으면 지팡이가 날아가고 튕겨 기절
    Expelliarmus,
    /// 헤르미온느 — 금빛, 맞으면 뻣뻣하게 굳어 통나무처럼 쿵
    Petrificus,
    /// 볼드모트 — 초록, 튕겨 기절
    Stupefy,
    /// 볼드모트 — 초록, 튕겨 기절 (해롭지 않은 장난)
    AvadaKedavra,
}

impl Curse {
    /// 빛줄기 색
    pub fn color(self) -> BeamColor {
        match self {
            Curse::Expelliarmus => BeamColor::Red,
            Curse::Petrificus => BeamColor::Gold,
            Curse::Stupefy | Curse::AvadaKedavra => BeamColor::Green,
        }
    }

    /// 맞으면 뻣뻣하게 굳어 넘어짐 (기절 대신)
    pub fn stiff(self) -> bool {
        self == Curse::Petrificus
    }

    /// 맞으면 지팡이가 날아감
    pub fn disarms(self) -> bool {
        self == Curse::Expelliarmus
    }
}

/// 🪄 side 쪽이 n번째로 쏘는 저주: 밝은 쪽은 빛줄기 색대로 (빨강 Expelliarmus · 그 밖 Petrificus), 볼드모트는 Stupefy·Avada Kedavra 번갈아
pub fn curse_of(side: Side, light: BeamColor, n: u32) -> Curse {
    match side {
        Side::Light if light == BeamColor::Red => Curse::Expelliarmus,
        Side::Light => Curse::Petrificus,
        Side::Dark if n % 2 == 0 => Curse::Stupefy,
        Side::Dark => Curse::AvadaKedavra,
    }
}

/// 🤸 피하는 법
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dodge {
    /// 폴짝 (낮게 오는 저주 위로)
    Hop,
    /// 몸 숙이기 (높게 오는 저주 아래로)
    Duck,
}

/// 저주가 어떻게 됐나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Hit,
    Dodge(Dodge),
}

/// 합 하나의 종류
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Move {
    /// ⚔️ 빛줄기 맞부딪힘 len초 → 진 쪽 (loser)이 맞음
    Lock { len: f64, loser: Side },
    /// 🪄 by 쪽이 저주를 쏨
    Shot { by: Side, outcome: Outcome },
    /// 🏃 runner가 run초 달아나다 (상대는 쫓음) 뒤돌아 저주를 쏨
    Chase { runner: Side, run: f64, outcome: Outcome },
}

/// 합 하나: at초에 시작, curse = 이 합에서 쏘는 (이긴) 쪽의 저주
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub at: f64,
    pub mv: Move,
    pub curse: Curse,
}

impl Step {
    /// 저주를 쏘기 시작하는 때 (합 안의 초 — 맞부딪힘은 없음)
    pub fn shot_at(&self) -> Option<f64> {
        match self.mv {
            Move::Lock { .. } => None,
            Move::Shot { .. } => Some(0.0),
            Move::Chase { run, .. } => Some(run + TURN_SECS),
        }
    }

    /// 쏘는 (이긴) 쪽
    pub fn shooter(&self) -> Side {
        match self.mv {
            Move::Lock { loser, .. } => loser.other(),
            Move::Shot { by, .. } => by,
            Move::Chase { runner, .. } => runner,
        }
    }

    /// 어떻게 됐나 (맞부딪힘은 늘 맞음)
    pub fn outcome(&self) -> Outcome {
        match self.mv {
            Move::Lock { .. } => Outcome::Hit,
            Move::Shot { outcome, .. } | Move::Chase { outcome, .. } => outcome,
        }
    }

    /// 저주가 닿는 때 (합 안의 초): 맞부딪힘은 끝날 때, 저주는 쏜 뒤 SHOT_FIRE + BOLT_FLY_SECS
    pub fn impact_at(&self) -> f64 {
        match (self.mv, self.shot_at()) {
            (Move::Lock { len, .. }, _) => len,
            (_, Some(s)) => s + SHOT_FIRE + BOLT_FLY_SECS,
            _ => 0.0,
        }
    }

    /// 이 합 길이 (맞으면 일어날 때까지, 피하면 다 피할 때까지 + PAUSE_SECS)
    pub fn len(&self) -> f64 {
        let hit = self.impact_at();
        let after = match self.outcome() {
            Outcome::Hit => KNOCK_SECS + HIT_FAINT_SECS + GETUP_SECS,
            Outcome::Dodge(_) => (DODGE_SECS - DODGE_LEAD).max(SHOT_SECS - SHOT_FIRE - BOLT_FLY_SECS),
        };
        hit + after + PAUSE_SECS
    }
}

/// 지금 할 일 (act가 냄)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Act {
    /// ⚔️ 빛줄기 맞부딪힘 (u = 합 안의 초 — wizard::duel_reach·duel_clash)
    Beam { u: f64, len: f64 },
    /// 🪄 저주를 쏨 (u = 쏘기 시작한 뒤 초, dodge = 상대가 피할 것 — 빛줄기가 그 너머로)
    Shoot { curse: Curse, u: f64, dodge: Option<Dodge> },
    /// 지팡이를 들고 상대를 봄
    Ready,
    /// 🤸 피함 (u = 피하기 시작한 뒤 초)
    Dodge { kind: Dodge, u: f64 },
    /// 💥 맞음 (h = 닿은 뒤 초): KNOCK_SECS 튕김 → HIT_FAINT_SECS 💫 → GETUP_SECS 일어남
    Hit { curse: Curse, h: f64 },
    /// 🏃 달아남 (away) / 쫓음
    Run { away: bool },
    /// 😮‍💨 숨 고르기
    Rest,
    /// 다 끝남 (흩어짐)
    Done,
}

/// ⚔️ 결투 한 판 (두 펫이 같은 것을 하나씩)
#[derive(Debug, Clone, PartialEq)]
pub struct Bout {
    pub steps: Vec<Step>,
    /// 숨 고르기 시작 · 다 끝나는 때 (초)
    pub rest_at: f64,
    pub len: f64,
}

impl Bout {
    /// 한 판을 짬 (light = 밝은 쪽 빛줄기 색): 맞부딪힘 → 저주 → 달아나기 → … 약 BOUT_SECS (적어도 네 합), 피하기가 적어도 한 번
    pub fn plan(rng: &mut fastrand::Rng, light: BeamColor) -> Bout {
        let target = crate::behavior::range(rng, BOUT_SECS) - REST_SECS;
        let side = |rng: &mut fastrand::Rng| if rng.bool() { Side::Light } else { Side::Dark };
        let outcome = |rng: &mut fastrand::Rng| {
            if rng.f64() < HIT_CHANCE {
                Outcome::Hit
            } else if rng.bool() {
                Outcome::Dodge(Dodge::Hop)
            } else {
                Outcome::Dodge(Dodge::Duck)
            }
        };
        let mut moves: Vec<Move> = vec![Move::Lock { len: crate::behavior::range(rng, LOCK_SECS), loser: side(rng) }];
        let mut t = Step { at: 0.0, mv: moves[0], curse: Curse::Stupefy }.len();
        let mut last_by = side(rng);
        // 적어도 네 합 (맞부딪힘·저주·달아나기·저주)
        while t < target || moves.len() < 4 {
            let mv = if moves.len() == 2 {
                Move::Chase { runner: side(rng), run: crate::behavior::range(rng, RUN_SECS), outcome: outcome(rng) }
            } else if moves.len() > 2 && moves.iter().filter(|m| matches!(m, Move::Lock { .. })).count() < MAX_LOCKS && rng.f64() < LOCK_CHANCE {
                Move::Lock { len: crate::behavior::range(rng, LOCK_SECS), loser: side(rng) }
            } else {
                // 대개 번갈아 쏨 (가끔 같은 쪽이 몰아침)
                last_by = if rng.f64() < 0.75 { last_by.other() } else { last_by };
                Move::Shot { by: last_by, outcome: outcome(rng) }
            };
            // 맞으면 판이 너무 길어지는 저주는 피함 (BOUT_SECS 끝을 넘지 않게)
            let mut mv = mv;
            if t + (Step { at: 0.0, mv, curse: Curse::Stupefy }).len() > BOUT_SECS.1 - REST_SECS {
                match &mut mv {
                    Move::Shot { outcome, .. } | Move::Chase { outcome, .. } if *outcome == Outcome::Hit => *outcome = Outcome::Dodge(if rng.bool() { Dodge::Hop } else { Dodge::Duck }),
                    // 맞부딪힘은 늘 맞으니 피하는 저주로
                    Move::Lock { loser, .. } => mv = Move::Shot { by: loser.other(), outcome: Outcome::Dodge(Dodge::Hop) },
                    _ => {}
                }
            }
            t += Step { at: 0.0, mv, curse: Curse::Stupefy }.len();
            moves.push(mv);
        }
        // 피하기가 한 번도 없으면 마지막 저주를 피함
        if !moves.iter().any(|m| matches!(m, Move::Shot { outcome: Outcome::Dodge(_), .. } | Move::Chase { outcome: Outcome::Dodge(_), .. })) {
            if let Some(m) = moves.iter_mut().rev().find(|m| !matches!(m, Move::Lock { .. })) {
                match m {
                    Move::Shot { outcome, .. } | Move::Chase { outcome, .. } => *outcome = Outcome::Dodge(Dodge::Hop),
                    Move::Lock { .. } => {}
                }
            }
        }
        let (mut at, mut fired) = (0.0, [0u32; 2]);
        let mut steps = Vec::with_capacity(moves.len());
        for mv in moves {
            let mut s = Step { at, mv, curse: Curse::Stupefy };
            let by = s.shooter();
            let n = &mut fired[usize::from(by == Side::Dark)];
            s.curse = curse_of(by, light, *n);
            *n += 1;
            at += s.len();
            steps.push(s);
        }
        Bout { steps, rest_at: at, len: at + REST_SECS }
    }

    /// t초째의 합과 그 합 안의 초 (숨 고르기·끝이면 None)
    pub fn step_at(&self, t: f64) -> Option<(&Step, f64)> {
        self.steps.iter().rev().find(|s| t >= s.at).filter(|_| t < self.rest_at).map(|s| (s, t - s.at))
    }

    /// side 쪽이 t초째 할 일
    pub fn act(&self, side: Side, t: f64) -> Act {
        if t >= self.len {
            return Act::Done;
        }
        let Some((s, u)) = self.step_at(t) else { return Act::Rest };
        let hit = s.impact_at();
        let shooter = s.shooter() == side;
        if let Move::Lock { len, .. } = s.mv {
            if u < len {
                return Act::Beam { u, len };
            }
        }
        if shooter {
            return match s.shot_at() {
                Some(at) if u < at => Act::Run { away: true },
                Some(at) if u < at + SHOT_SECS => Act::Shoot { curse: s.curse, u: u - at, dodge: match s.outcome() {
                    Outcome::Dodge(d) => Some(d),
                    Outcome::Hit => None,
                } },
                _ => Act::Ready,
            };
        }
        if let Move::Chase { run, .. } = s.mv {
            if u < run {
                return Act::Run { away: false };
            }
        }
        match s.outcome() {
            Outcome::Hit if u >= hit && u < hit + KNOCK_SECS + HIT_FAINT_SECS + GETUP_SECS => Act::Hit { curse: s.curse, h: u - hit },
            Outcome::Dodge(kind) if u >= hit - DODGE_LEAD && u < hit - DODGE_LEAD + DODGE_SECS => Act::Dodge { kind, u: u - (hit - DODGE_LEAD) },
            _ => Act::Ready,
        }
    }
}

/// 💥 맞은 뒤 h초의 몫들: (튕긴 몫 0…1, 누운 몫 0…1 — 튕기며 쓰러지고 일어나며 서는)
pub fn hit_shape(h: f64) -> (f64, f64) {
    let smooth = |u: f64| {
        let u = u.clamp(0.0, 1.0);
        u * u * (3.0 - 2.0 * u)
    };
    let knock = smooth(h / KNOCK_SECS);
    let down = if h < KNOCK_SECS + HIT_FAINT_SECS { knock } else { 1.0 - smooth((h - KNOCK_SECS - HIT_FAINT_SECS) / GETUP_SECS) };
    (knock, down)
}

/// 💫 맞은 뒤 h초에 기절·뻣뻣해 누워 있음 (튕김 뒤 HIT_FAINT_SECS 동안)
pub fn hit_dazed(h: f64) -> bool {
    (KNOCK_SECS..KNOCK_SECS + HIT_FAINT_SECS).contains(&h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bouts() -> Vec<Bout> {
        (0..60).map(|seed| Bout::plan(&mut fastrand::Rng::with_seed(seed), if seed % 2 == 0 { BeamColor::Red } else { BeamColor::Gold })).collect()
    }

    #[test]
    fn each_wizard_has_their_own_curse_and_voldemort_alternates() {
        assert_eq!(curse_of(Side::Light, BeamColor::Red, 0), Curse::Expelliarmus);
        assert_eq!(curse_of(Side::Light, BeamColor::Red, 3), Curse::Expelliarmus);
        assert_eq!(curse_of(Side::Light, BeamColor::Gold, 0), Curse::Petrificus);
        let dark: Vec<Curse> = (0..4).map(|n| curse_of(Side::Dark, BeamColor::Red, n)).collect();
        assert_eq!(dark, vec![Curse::Stupefy, Curse::AvadaKedavra, Curse::Stupefy, Curse::AvadaKedavra]);
        assert_eq!(Curse::Expelliarmus.color(), BeamColor::Red);
        assert_eq!(Curse::Petrificus.color(), BeamColor::Gold);
        assert_eq!(Curse::AvadaKedavra.color(), BeamColor::Green);
        assert!(Curse::Petrificus.stiff() && !Curse::Stupefy.stiff());
        assert!(Curse::Expelliarmus.disarms() && !Curse::AvadaKedavra.disarms());
    }

    #[test]
    fn a_bout_lasts_about_fifteen_seconds_with_a_lock_a_chase_and_a_dodge() {
        for b in bouts() {
            assert!(b.len >= BOUT_SECS.0 && b.len <= BOUT_SECS.1 + 2.5, "{}", b.len);
            assert!((b.len - b.rest_at - REST_SECS).abs() < 1e-9);
            assert!(matches!(b.steps[0].mv, Move::Lock { .. }), "첫 합은 맞부딪힘");
            assert!(matches!(b.steps[2].mv, Move::Chase { .. }), "셋째 합은 달아나기");
            assert!(b.steps.len() >= 4, "여러 합: {}", b.steps.len());
            assert!(b.steps.iter().any(|s| matches!(s.outcome(), Outcome::Dodge(_))), "피하기");
            assert!(b.steps.iter().filter(|s| matches!(s.mv, Move::Lock { .. })).count() <= MAX_LOCKS);
            for w in b.steps.windows(2) {
                assert!((w[1].at - w[0].at - w[0].len()).abs() < 1e-9, "합이 이어짐");
            }
        }
    }

    #[test]
    fn voldemort_alternates_within_a_bout() {
        let b = bouts().into_iter().find(|b| b.steps.iter().filter(|s| s.shooter() == Side::Dark).count() >= 2).unwrap();
        let dark: Vec<Curse> = b.steps.iter().filter(|s| s.shooter() == Side::Dark).map(|s| s.curse).collect();
        assert_eq!(&dark[..2], &[Curse::Stupefy, Curse::AvadaKedavra]);
        let red = bouts().remove(0);
        assert!(red.steps.iter().filter(|s| s.shooter() == Side::Light).all(|s| s.curse == Curse::Expelliarmus));
    }

    #[test]
    fn a_hit_knocks_back_then_faints_two_seconds_then_gets_up() {
        let step = Step { at: 0.0, mv: Move::Shot { by: Side::Dark, outcome: Outcome::Hit }, curse: Curse::Stupefy };
        let b = Bout { steps: vec![step], rest_at: step.len(), len: step.len() + REST_SECS };
        let impact = SHOT_FIRE + BOLT_FLY_SECS;
        assert_eq!(b.act(Side::Dark, 0.1), Act::Shoot { curse: Curse::Stupefy, u: 0.1, dodge: None });
        assert_eq!(b.act(Side::Light, 0.1), Act::Ready);
        assert_eq!(b.act(Side::Dark, SHOT_SECS + 0.1), Act::Ready);
        let Act::Hit { curse, h } = b.act(Side::Light, impact + 0.1) else { panic!() };
        assert!(curse == Curse::Stupefy && (h - 0.1).abs() < 1e-9);
        // 💫 2초
        let dazed: f64 = (0..400).map(|i| i as f64 * 0.01).filter(|&h| hit_dazed(h)).count() as f64 * 0.01;
        assert!((dazed - HIT_FAINT_SECS).abs() < 0.02, "{dazed}");
        assert_eq!(hit_shape(0.0), (0.0, 0.0));
        assert_eq!(hit_shape(KNOCK_SECS + 1.0), (1.0, 1.0));
        assert!(hit_shape(KNOCK_SECS + HIT_FAINT_SECS + GETUP_SECS).1 < 1e-9, "일어남");
        let up = impact + KNOCK_SECS + HIT_FAINT_SECS + GETUP_SECS + 0.01;
        assert_eq!(b.act(Side::Light, up), Act::Ready);
        assert_eq!(b.act(Side::Light, b.rest_at + 0.1), Act::Rest);
        assert_eq!(b.act(Side::Dark, b.len), Act::Done);
    }

    #[test]
    fn a_chase_runs_away_turns_and_fires_while_the_other_chases_then_dodges() {
        let step = Step { at: 0.0, mv: Move::Chase { runner: Side::Light, run: 2.0, outcome: Outcome::Dodge(Dodge::Duck) }, curse: Curse::Petrificus };
        let b = Bout { steps: vec![step], rest_at: step.len(), len: step.len() + REST_SECS };
        assert_eq!(b.act(Side::Light, 1.0), Act::Run { away: true });
        assert_eq!(b.act(Side::Dark, 1.0), Act::Run { away: false });
        assert_eq!(b.act(Side::Dark, 2.1), Act::Ready, "멈춰 섬");
        assert!(matches!(b.act(Side::Light, 2.0 + TURN_SECS + 0.05), Act::Shoot { curse: Curse::Petrificus, dodge: Some(Dodge::Duck), .. }));
        let hit = step.impact_at();
        assert!(matches!(b.act(Side::Dark, hit), Act::Dodge { kind: Dodge::Duck, .. }));
        assert_eq!(b.act(Side::Dark, hit - DODGE_LEAD + DODGE_SECS + 0.01), Act::Ready);
    }

    #[test]
    fn the_lock_ends_with_the_loser_hit_by_the_winners_curse() {
        let step = Step { at: 0.0, mv: Move::Lock { len: 3.0, loser: Side::Dark }, curse: Curse::Expelliarmus };
        let b = Bout { steps: vec![step], rest_at: step.len(), len: step.len() + REST_SECS };
        assert_eq!(b.act(Side::Dark, 1.0), Act::Beam { u: 1.0, len: 3.0 });
        assert_eq!(b.act(Side::Light, 1.0), Act::Beam { u: 1.0, len: 3.0 });
        assert!(matches!(b.act(Side::Dark, 3.1), Act::Hit { curse: Curse::Expelliarmus, .. }));
        assert_eq!(b.act(Side::Light, 3.1), Act::Ready);
    }
}
