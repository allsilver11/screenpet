//! 👻 디멘터 (pet.json `dementor` + locomotion fly — PetConfig::dementor) + 곁에서 시무룩 (누구나) — brain.rs의 자식 모듈. 숫자는 wizard.rs.
//!
//! - 날기는 나는 동물 그대로 (brain_fly.rs). 새로 날 때 STALK_CHANCE로 같은 화면의 다른 펫 머리 위로 스르륵 다가감 (stalk_target)
//! - 👻 시무룩: sync_gloom (앱이 매 틱 — sync_lifts가 부름)이 디멘터 GLOOM_REACH 안의 펫에게 알려 줌 (DementorMood.near).
//!   그 펫은 가까이 있는 동안 배가 더 고프고 (take_chill → 앱이 포만감에서 뺌), 들 수 있는 때면 GLOOM_SECS 동안 시무룩 (State::Gloom — annoyed 그림, "brrr...")
//! - 🥶 서리: 디멘터가 작업 영역 구석의 FROST_REACH 안에 있으면 그 구석에 서리가 끼고, 멀어지거나 도망가면 천천히 녹음 (dementor_fx → MagicFx::Frost).
//!   🧹 빗자루로 쓸면 바로 녹고 FROST_SWEPT_SECS 동안 다시 안 낌 (sweep_frost)
//! - 🦌 패트로누스에 쫓김 (be_repelled — brain_duel.rs가 부름): "hsss!" 하고 사슴 반대쪽 화면 위 구석으로 빠르게 날아감.
//!   FLEE_SECS 동안 시무룩·서리·다가가기 없음
//!
//! 주사위는 자기 주사위(DementorMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::flight::{self, Goal, Style};
use crate::magic::MagicFx;
use crate::wizard as wz;

/// 🥶 화면 구석 하나의 서리 (물리 px)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrostPatch {
    /// 구석 자리
    pub x: f64,
    pub y: f64,
    /// 화면 안쪽 부호 (±1, ±1)
    pub inward: (f64, f64),
    /// 다 낀 크기
    pub size: f64,
    /// 낀 몫 (0…1)
    pub level: f64,
}

/// 디멘터 (와 그 곁의 펫)의 기억
#[derive(Debug, Clone)]
pub struct DementorMood {
    rng: fastrand::Rng,
    /// 디멘터 곁에 있음 (sync_gloom이 매 틱 정함 — 디멘터 자신은 늘 false)
    pub(super) near: bool,
    /// 이 시각(clock)부터 다시 시무룩해질 수 있음
    pub(super) gloom_ready_at: f64,
    /// 아직 앱이 꺼내 가지 않은 더 고파진 배 (포만감 몫)
    chill: f64,
    /// 🥶 구석의 서리들 (디멘터만)
    pub(super) frost: Vec<FrostPatch>,
    /// 🧹 빗자루로 쓴 구석 (x, y)과 다시 낄 수 있는 시각 (clock)
    swept: Vec<(f64, f64, f64)>,
    /// 🦌 이 시각(clock)까지 도망 중 (디멘터만)
    pub(super) flee_until: f64,
    /// 🦌 쫓긴 쪽 x — 아직 날아가기 시작하지 않음
    flee_from: Option<f64>,
}

impl DementorMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        DementorMood { rng: fastrand::Rng::with_seed(seed ^ 0x6465_6d65), near: false, gloom_ready_at: 0.0, chill: 0.0, frost: Vec::new(), swept: Vec::new(), flee_until: f64::NEG_INFINITY, flee_from: None }
    }
}

impl Pet {
    /// 👻 디멘터가 🦌 패트로누스에 쫓겨 도망 중
    pub fn dementor_fleeing(&self) -> bool {
        self.cfg.dementor && self.clock < self.dementor.flee_until
    }

    /// 👻 지금 곁의 펫을 시무룩하게 하는 디멘터 (도망 중·친구 화면·하우스가 아님)
    pub fn dementor_active(&self) -> bool {
        self.cfg.dementor && !self.dementor_fleeing() && !self.is_away() && !self.housed
    }

    /// 👻 디멘터 곁에서 시무룩한 중
    pub fn is_gloomy(&self) -> bool {
        matches!(self.state, State::Gloom { .. })
    }

    /// 👻 디멘터 곁에서 더 고파진 배 (포만감 몫 — 앱이 매 틱 꺼내 care에서 뺌)
    pub fn take_chill(&mut self) -> f64 {
        std::mem::take(&mut self.dementor.chill)
    }

    /// 🥶 지금 낀 서리들
    pub fn frost(&self) -> &[FrostPatch] {
        &self.dementor.frost
    }

    /// 🦌 패트로누스에 맞음 (디멘터만, 도망 중이 아닐 때): from_x 반대쪽으로 날아가 FLEE_SECS 동안 얌전. 쫓겼으면 true
    pub fn be_repelled(&mut self, from_x: f64) -> bool {
        if !self.cfg.dementor || self.dementor_fleeing() {
            return false;
        }
        self.dementor.flee_until = self.clock + behavior::range(&mut self.dementor.rng, wz::FLEE_SECS);
        self.dementor.flee_from = Some(from_x);
        self.speech = Some(Speech::Repelled);
        true
    }

    /// 매 틱 (상태와 상관없이): 곁이면 배가 더 고파지고, 디멘터는 쫓겼으면 날아가기 시작하고 구석의 서리가 끼거나 녹음
    pub(super) fn dementor_tick(&mut self, dt: f64, world: &World) {
        if self.dementor.near {
            self.dementor.chill += wz::GLOOM_HUNGER * dt;
        }
        if !self.cfg.dementor {
            return;
        }
        if let Some(from_x) = self.dementor.flee_from {
            if !self.is_held() && !matches!(self.state, State::Away | State::Exit { .. } | State::Enter { .. } | State::Farewell { .. }) && !self.housed {
                self.dementor.flee_from = None;
                self.flee_flight(world, from_x);
            }
        }
        self.tick_frost(dt, world);
    }

    /// 🦌 from_x 반대쪽 화면 위 구석으로 빠르게 날아감
    fn flee_flight(&mut self, world: &World, from_x: f64) {
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let dir = if (self.body.x - from_x).abs() > 1.0 { (self.body.x - from_x).signum() } else { self.facing };
        let size = self.cfg.size;
        let tx = if dir > 0.0 { wa.right - 1.2 * size } else { wa.left + 1.2 * size };
        let to = flight::clamp(&self.fly_limits(world), tx, wa.top + 1.4 * size);
        if self.standing.is_some() {
            self.take_off();
        }
        self.pacing.after_beat = None;
        self.start_flight_styled(Goal::Point(to.0, to.1), Move::Run, Style::Gentle);
    }

    /// 🥶 가까운 구석은 서리가 끼고 (가까울수록 빠르게), 나머지는 녹음 — 다 녹은 것은 버림
    fn tick_frost(&mut self, dt: f64, world: &World) {
        let active = self.dementor_active();
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let diag = (wa.right - wa.left).hypot(wa.bottom - wa.top);
        let reach = wz::FROST_REACH * diag;
        let (cx, cy) = (self.body.x, self.body.y - 0.5 * self.cfg.size);
        let mut grown = Vec::new();
        if active {
            for (x, y, inward) in [(wa.left, wa.top, (1.0, 1.0)), (wa.right, wa.top, (-1.0, 1.0)), (wa.left, wa.bottom, (1.0, -1.0)), (wa.right, wa.bottom, (-1.0, -1.0))] {
                let d = (x - cx).hypot(y - cy);
                let swept = self.dementor.swept.iter().any(|&(sx, sy, until)| (sx - x).abs() < 1.0 && (sy - y).abs() < 1.0 && self.clock < until);
                if d > reach || swept {
                    continue;
                }
                let grow = wz::FROST_GROW * dt * (1.0 - 0.5 * d / reach);
                match self.dementor.frost.iter_mut().find(|f| (f.x - x).abs() < 1.0 && (f.y - y).abs() < 1.0) {
                    Some(f) => {
                        f.level = (f.level + grow).min(1.0);
                        f.size = wz::FROST_SIZE * diag;
                    }
                    None => self.dementor.frost.push(FrostPatch { x, y, inward, size: wz::FROST_SIZE * diag, level: grow }),
                }
                grown.push((x, y));
            }
        }
        for f in self.dementor.frost.iter_mut() {
            if !grown.iter().any(|&(x, y)| (f.x - x).abs() < 1.0 && (f.y - y).abs() < 1.0) {
                f.level -= wz::FROST_FADE * dt;
            }
        }
        self.dementor.frost.retain(|f| f.level > 0.0);
        let clock = self.clock;
        self.dementor.swept.retain(|s| clock < s.2);
    }

    /// 🧹 빗자루가 이번 틱에 지나간 구석의 서리를 녹임 (그 구석은 FROST_SWEPT_SECS 동안 다시 안 낌). 녹였으면 true
    pub fn sweep_frost(&mut self, broom: &crate::broom::Broom, dpi: f64) -> bool {
        let until = self.clock + wz::FROST_SWEPT_SECS;
        let mut melted = false;
        let swept = &mut self.dementor.swept;
        self.dementor.frost.retain(|f| {
            let corner = match f.inward {
                (x, y) if x > 0.0 && y > 0.0 => crate::web::Corner::TopLeft,
                (_, y) if y > 0.0 => crate::web::Corner::TopRight,
                (x, _) if x > 0.0 => crate::web::Corner::BottomLeft,
                _ => crate::web::Corner::BottomRight,
            };
            if !broom.sweeps_web(f.x, f.y, corner, f.size * f.level.max(0.3), dpi) {
                return true;
            }
            swept.push((f.x, f.y, until));
            melted = true;
            false
        });
        melted
    }

    /// 👻 새 날기의 목표: 가끔 (STALK_CHANCE) 같은 화면의 다른 펫 머리 위 (디멘터만, 도망 중·창 안이 아닐 때)
    pub(super) fn stalk_target(&mut self, world: &World) -> Option<(f64, f64)> {
        if !self.dementor_active() || self.inside.is_some() || self.dementor.rng.f64() >= wz::STALK_CHANCE {
            return None;
        }
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let pets: Vec<PetSpot> = self.hoist.spots.iter().filter(|s| world.work_area_at(s.x, s.y - 1.0) == wa).copied().collect();
        if pets.is_empty() {
            return None;
        }
        let s = pets[self.dementor.rng.usize(..pets.len())];
        let size = self.cfg.size;
        let x = s.x + (self.dementor.rng.f64() * 2.0 - 1.0) * 0.6 * size;
        let y = s.y - s.size - 0.3 * size;
        Some(flight::clamp(&self.fly_limits(world), x, y))
    }

    /// 👻 시무룩 한 틱 (누구나): 시무룩한 중이면 디딤판을 따라가다 끝나면 쉼, 아니면 디멘터 곁이고 들 수 있는 때면 시무룩해짐. 처리했으면 true
    pub(super) fn update_gloom(&mut self, dt: f64, world: &World) -> bool {
        if let State::Gloom { remaining } = self.state {
            if !self.follow_anchor(world) {
                self.start_fall();
                return true;
            }
            let remaining = remaining - dt;
            self.state = if remaining > 0.0 { State::Gloom { remaining } } else { self.rest_beat() };
            return true;
        }
        if !self.dementor.near || self.clock < self.dementor.gloom_ready_at || !self.lift_ok() || self.cfg.flies {
            return false;
        }
        self.dementor.gloom_ready_at = self.clock + behavior::range(&mut self.dementor.rng, wz::GLOOM_CALM_SECS);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Gloom);
        self.state = State::Gloom { remaining: wz::GLOOM_SECS };
        true
    }

    /// 🥶 덮개 창에 그릴 서리 (낀 몫은 1/50 단위로 — 다 끼었거나 천천히 녹는 동안 앱이 매 틱 보내지 않게)
    pub(super) fn dementor_fx(&self, out: &mut Vec<MagicFx>) {
        for f in self.dementor.frost.iter() {
            out.push(MagicFx::Frost { x: f.x, y: f.y, size: f.size.round(), inward: f.inward, level: (f.level * 50.0).ceil() / 50.0 });
        }
    }
}

/// 👻 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 시무룩하게 하는 디멘터 GLOOM_REACH 안의 동물에게 곁이라고 알림
pub(super) fn sync_gloom<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    let dementors: Vec<(u64, f64, f64, f64)> = pets
        .iter()
        .map(|p| p.as_ref())
        .filter(|p| p.dementor_active())
        .map(|p| (p.social_key, p.body.x, p.body.y - 0.5 * p.cfg.size, p.cfg.size))
        .collect();
    for p in pets.iter_mut() {
        let p = p.as_mut();
        let (x, y) = (p.body.x, p.body.y - 0.5 * p.cfg.size);
        p.dementor.near = !p.cfg.dementor
            && !p.is_away()
            && dementors.iter().any(|&(key, dx, dy, size)| key != p.social_key && (x - dx).hypot(y - dy) <= wz::GLOOM_REACH * size);
    }
}
