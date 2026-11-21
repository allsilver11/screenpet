//! 🖍️ 짱구 (pet.json `rascal` + JJANGGU_ANIMS — PetConfig::rascal) — brain.rs의 자식 모듈. 💕 헤르미온느에게 작업 거는 건 social_flirt.rs.
//!
//! - 한가할 때 가끔 (ACT_RATE × 🌱 power) 하나를 고름:
//!   🍑 엉덩이 춤 (ButtDance — "buri buri~", 제자리 butt_dance) · 🦸 액션가면 흉내 (Action — "Action beam!", action_pose 한 번) ·
//!   🍪 초코비 (Snack — "Chocobi!", 앉아서 하나씩 쏙) · 😴 뒹굴뒹굴 (Lazy — "So lazy~", 앞 몫 LAZY_ROLL은 roll_lazy, 나머지는 엎드려 lie)
//! - 🐶 흰둥이: 가끔 (SHIRO_RATE) "Shiro!" 하고 작은 하얀 강아지가 옆에 나타나 SHIRO_SECS 동안 짱구 뒤를 졸졸 따라다니다 사라짐
//!   (MagicFx::Shiro — 덮개 창이 그림, 마우스는 통과). 하던 일과 상관없이 따라다님
//!
//! 주사위는 자기 주사위(JjangguMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::magic::MagicFx;

/// 처음 장난을 할 수 있게 되는 때 (clock 초)
pub const FIRST_SECS: f64 = 20.0;
/// 한가할 때 장난을 시작하는 빈도 (초당 — 🌱 power를 곱함)
pub const ACT_RATE: f64 = 0.025;
/// 다음 장난까지 (초)
pub const CALM_SECS: (f64, f64) = (20.0, 55.0);
/// 고르는 몫: 🍑 엉덩이 춤 · 🦸 액션 빔 · 🍪 초코비 · 😴 뒹굴 (나머지)
pub const BUTT_SHARE: f64 = 0.32;
pub const ACTION_SHARE: f64 = 0.25;
pub const SNACK_SHARE: f64 = 0.2;

/// 🍑 엉덩이 춤 시간 (초)
pub const BUTT_SECS: (f64, f64) = (2.5, 4.5);
/// 🦸 액션 포즈 (action_pose 3칸 3fps + 빔 유지, 초)
pub const ACTION_SECS: f64 = 2.2;
/// 🍪 초코비 먹는 시간 (초)
pub const SNACK_SECS: (f64, f64) = (4.0, 7.0);
/// 😴 빈둥대는 시간 (초) · 그 가운데 앞 몫은 뒹굴뒹굴, 나머지는 엎드려 다리 까딱
pub const LAZY_SECS: (f64, f64) = (6.0, 12.0);
pub const LAZY_ROLL: f64 = 0.5;

/// 🐶 흰둥이: 처음 올 수 있는 때 (clock 초) · 오는 빈도 (초당) · 머무는 시간 (초) · 다녀간 뒤 다시 오기까지 (초)
pub const SHIRO_FIRST: f64 = 60.0;
pub const SHIRO_RATE: f64 = 0.01;
pub const SHIRO_SECS: (f64, f64) = (12.0, 22.0);
pub const SHIRO_CALM: (f64, f64) = (90.0, 240.0);
/// 🐶 짱구 뒤 간격 (몸 크기 배) · 따라가는 빠르기 (짱구 걷기 배) · 크기 (몸 크기 배)
pub const SHIRO_GAP: f64 = 0.55;
pub const SHIRO_PACE: f64 = 1.6;
pub const SHIRO_SIZE: f64 = 0.28;

/// 짱구가 하는 장난 (State::Jjanggu — t는 시작한 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JjangguAct {
    /// 🍑 len초 엉덩이 춤
    ButtDance { len: f64 },
    /// 🦸 "Action beam!" 한 번
    Action,
    /// 🍪 len초 초코비
    Snack { len: f64 },
    /// 😴 len초 뒹굴뒹굴 → 엎드려 까딱
    Lazy { len: f64 },
}

impl JjangguAct {
    /// 이 장난의 길이 (초)
    fn len(self) -> f64 {
        match self {
            JjangguAct::ButtDance { len } | JjangguAct::Snack { len } | JjangguAct::Lazy { len } => len,
            JjangguAct::Action => ACTION_SECS,
        }
    }
}

/// 🐶 지금 와 있는 흰둥이
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shiro {
    /// 발 가운데 x (물리 px)
    pub x: f64,
    /// 온 뒤 흐른 시간 · 머무는 시간 (초)
    pub t: f64,
    pub len: f64,
    /// 보는 쪽 (±1)
    pub dir: f64,
}

/// 짱구의 기억
#[derive(Debug, Clone)]
pub struct JjangguMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 새 장난을 할 수 있음
    pub(super) ready_at: f64,
    /// 이 시각(clock)부터 흰둥이가 올 수 있음
    pub(super) shiro_ready_at: f64,
    /// 🐶 와 있는 흰둥이
    shiro: Option<Shiro>,
}

impl JjangguMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        JjangguMood { rng: fastrand::Rng::with_seed(seed ^ 0x6a6a_6775), ready_at: FIRST_SECS, shiro_ready_at: SHIRO_FIRST, shiro: None }
    }
}

impl Pet {
    /// 🖍️ 짱구임 (💕 헤르미온느에게 작업 — social_flirt.rs가 봄)
    pub fn is_rascal(&self) -> bool {
        self.cfg.rascal
    }

    /// 장난별 그림
    pub(super) fn jjanggu_anim(&self, act: JjangguAct, t: f64) -> Anim {
        match act {
            JjangguAct::ButtDance { .. } => Anim::ButtDance,
            JjangguAct::Action => Anim::ActionPose,
            JjangguAct::Snack { .. } => Anim::Snack,
            JjangguAct::Lazy { len } if t < len * LAZY_ROLL => Anim::RollLazy,
            JjangguAct::Lazy { .. } => Anim::Lie,
        }
    }

    /// 매 틱 (해그리드 뒤): 흰둥이를 돌보고, 장난 중이면 그 한 틱, 아니면 가끔 새 장난. 처리했으면 true
    pub(super) fn update_jjanggu(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.rascal {
            return false;
        }
        self.tick_shiro(dt);
        if let State::Jjanggu { act, t } = self.state {
            self.tick_jjanggu(world, act, t + dt);
            return true;
        }
        if !self.play_free() || self.romp_interrupted() || self.is_user_away() || self.clock < self.jjanggu.ready_at {
            return false;
        }
        if self.jjanggu.rng.f64() >= ACT_RATE * self.power() * dt {
            return false;
        }
        let rng = &mut self.jjanggu.rng;
        let roll = rng.f64();
        let act = if roll < BUTT_SHARE {
            JjangguAct::ButtDance { len: behavior::range(rng, BUTT_SECS) }
        } else if roll < BUTT_SHARE + ACTION_SHARE {
            JjangguAct::Action
        } else if roll < BUTT_SHARE + ACTION_SHARE + SNACK_SHARE {
            JjangguAct::Snack { len: behavior::range(rng, SNACK_SECS) }
        } else {
            JjangguAct::Lazy { len: behavior::range(rng, LAZY_SECS) }
        };
        let started = self.start_jjanggu(act);
        if !started {
            self.jjanggu.ready_at = self.clock + 5.0;
        }
        started
    }

    /// 장난을 시작함 (서 있을 때만): 말하고, 쉬는 박자를 지우고, 다음 장난 때를 정함
    pub(crate) fn start_jjanggu(&mut self, act: JjangguAct) -> bool {
        if !self.cfg.rascal || self.standing.is_none() {
            return false;
        }
        self.speech = Some(match act {
            JjangguAct::ButtDance { .. } => Speech::BuriBuri,
            JjangguAct::Action => Speech::ActionBeam,
            JjangguAct::Snack { .. } => Speech::Chocobi,
            JjangguAct::Lazy { .. } => Speech::Lazy,
        });
        self.pacing.after_beat = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.jjanggu.ready_at = self.clock + behavior::range(&mut self.jjanggu.rng, CALM_SECS);
        self.state = State::Jjanggu { act, t: 0.0 };
        true
    }

    /// 장난 한 틱 (t = 이번 틱까지 흐른 시간): 제자리, 길이가 다 되면 쉬는 박자로
    fn tick_jjanggu(&mut self, world: &World, act: JjangguAct, t: f64) {
        if self.romp_interrupted() || self.is_user_away() {
            self.state = self.rest_beat();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        self.state = if t < act.len() { State::Jjanggu { act, t } } else { self.rest_beat() };
    }

    /// 🐶 흰둥이를 부름 (len초 머묾): 짱구 뒤쪽 옆에 나타남, "Shiro!"
    pub(crate) fn call_shiro(&mut self, len: f64) {
        let x = self.body.x - self.facing * SHIRO_GAP * self.cfg.size;
        self.jjanggu.shiro = Some(Shiro { x, t: 0.0, len, dir: self.facing });
        self.speech = Some(Speech::Shiro);
    }

    /// 🐶 흰둥이 한 틱: 와 있으면 짱구 뒤쪽 자리로 졸졸 (다 머물면 사라짐), 없으면 가끔 옴
    fn tick_shiro(&mut self, dt: f64) {
        if let Some(mut s) = self.jjanggu.shiro {
            s.t += dt;
            if s.t >= s.len {
                self.jjanggu.shiro = None;
                self.jjanggu.shiro_ready_at = self.clock + behavior::range(&mut self.jjanggu.rng, SHIRO_CALM);
                return;
            }
            let goal = self.body.x - self.facing * SHIRO_GAP * self.cfg.size;
            let dx = goal - s.x;
            let step = SHIRO_PACE * self.cfg.walk_speed * dt;
            if dx.abs() > 1.0 {
                s.dir = dx.signum();
            } else {
                s.dir = self.facing;
            }
            s.x += dx.clamp(-step, step);
            self.jjanggu.shiro = Some(s);
            return;
        }
        if self.clock < self.jjanggu.shiro_ready_at || self.standing.is_none() || self.is_user_away() || !self.play_free() {
            return;
        }
        if self.jjanggu.rng.f64() < SHIRO_RATE * dt {
            let len = behavior::range(&mut self.jjanggu.rng, SHIRO_SECS);
            self.call_shiro(len);
        }
    }

    /// 🐶 덮개 창에 그릴 흰둥이 (짱구 발 높이)
    pub(super) fn jjanggu_fx(&self, out: &mut Vec<MagicFx>) {
        if let Some(s) = self.jjanggu.shiro.filter(|_| !self.is_away()) {
            out.push(MagicFx::Shiro { x: s.x, y: self.body.y, size: SHIRO_SIZE * self.cfg.size, t: s.t, len: s.len, dir: s.dir });
        }
    }
}
