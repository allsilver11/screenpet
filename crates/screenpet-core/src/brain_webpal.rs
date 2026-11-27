//! 🕷️🤝 거미인간과 친구 — 그네 태우기·구석 거미집으로 낚아채기 — brain.rs의 자식 모듈 (brain_web.rs 줄 놀이에 얹음).
//!
//! - 🌉🤝 그네 태우기: 그네를 시작할 때 (start_web_swing) 몸 RIDE_REACH배 안에 들 수 있는 동물 (친구 동물 포함)이 있으면 RIDE_CHANCE로
//!   그 동물을 붙잡고 같이 탐 — 태운 동물은 State::Lifted (brain_hoist.rs 들기 길을 빌림 — hoisting·hoist_point가 web_rider·rider_point를 봄)로
//!   거미인간 곁에 매달려 호를 따라가다 ("whee!"), 그네를 놓을 때 그 속도 그대로 날아가 내려섬. 잡아채이거나 끊기면 그냥 놓음.
//! - 🕸️🎣 낚아채기: 닿는 거리에 쉬는 다른 동물이 있으면 가끔 (SNATCH_RATE, 쉬는 시간 SNATCH_CALM — 몇 분) 가까운 구석에 거미집을 친 뒤
//!   다 짜자마자 그 동물에게 줄을 쏴 거미집으로 휙 끌어옴 ("gotcha!") — 끌려 날아가는 동안 (SNATCH_SURE_SECS) 거미집에 들어가면 무조건 걸려
//!   (brain_snare.rs) 버둥대다 찢고 빠져나옴. 그새 그 동물이 들 수 없게 됐으면 그만둠.
//! - 같은 화면 동물 사이의 일 (태우기·놓기·끌어오기)은 sync_pals (sync_lifts가 부름).
//!
//! 주사위는 자기 주사위(PalMood.rng)로 굴림 — 다른 줄 놀이의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::web_moves::{WebAim, WebPhase};
use super::*;
use crate::web::{self, WebSpin};

/// 🌉🤝 그네마다 곁의 동물을 태울 몫
pub const RIDE_CHANCE: f64 = 0.4;
/// 🌉🤝 태울 수 있는 거리 (몸 배)
pub const RIDE_REACH: f64 = 2.5;
/// 🌉🤝 태운 동물 발이 거미인간 발에서 뒤로 비켜난 만큼 (거미인간 몸 배)
const RIDE_BEHIND: f64 = 0.4;
/// 🌉🤝 떠오른 뒤 태운 동물 발이 거미인간 발보다 내려온 만큼 (태운 동물 몸 배)
const RIDE_HANG: f64 = 0.3;
/// 🕸️🎣 쉬는 다른 동물이 있을 때 1초마다 낚아채기를 할 몫
pub const SNATCH_RATE: f64 = 1.0 / 30.0;
/// 🕸️🎣 한 번 낚아채고 다시 하기까지 (초)
pub const SNATCH_CALM: (f64, f64) = (180.0, 360.0);
/// 🕸️🎣 앱을 켜고 처음 낚아채기까지 (초)
pub const SNATCH_FIRST_SECS: f64 = 90.0;
/// 🕸️🎣 거미집까지 날아가는 시간 (초)
pub const SNATCH_FLIGHT_SECS: f64 = 0.7;
/// 🕸️🎣 끌려 날아가는 동안 거미집에 들어가면 무조건 걸리는 시간 (초)
pub const SNATCH_SURE_SECS: f64 = 1.5;
/// 🕸️🎣 끌어올 자리: 거미집 모서리에서 반지름의 이만큼 안쪽 (대각선)
const SNATCH_DEPTH: f64 = 0.45;

/// 거미인간과 친구 놀이의 기억 (WebMood.pal)
#[derive(Debug, Clone)]
pub struct PalMood {
    rng: fastrand::Rng,
    /// 🌉🤝 태울 몫 (보통 RIDE_CHANCE — 테스트가 바꿈)
    pub(super) ride_odds: f64,
    /// 🌉🤝 이번 그네에 태운 동물 (social_key, 몸 크기)
    pub(super) rider: Option<(u64, f64)>,
    /// 🌉🤝 막 태운 동물 — sync_pals가 한 번 꺼내 be_lifted
    board_now: Option<u64>,
    /// 🌉🤝 그네 끝에 놓은 동물 (social_key, vx, vy) — sync_pals가 한 번 꺼냄
    drop_now: Option<(u64, f64, f64)>,
    /// 🕸️🎣 이 시각(clock)부터 낚아채기를 할 수 있음
    pub(super) snatch_ready_at: f64,
    /// 🕸️🎣 거미집을 다 짜면 낚아챌 동물 (social_key)
    pub(super) snatch: Option<u64>,
    /// 🕸️🎣 줄이 닿아 끌어온 동물 (social_key, vx, vy) — sync_pals가 한 번 꺼냄
    snatch_now: Option<(u64, f64, f64)>,
}

impl PalMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        PalMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x7061_6c73),
            ride_odds: RIDE_CHANCE,
            rider: None,
            board_now: None,
            drop_now: None,
            snatch_ready_at: SNATCH_FIRST_SECS,
            snatch: None,
            snatch_now: None,
        }
    }
}

impl Pet {
    /// 🌉🤝 그네를 시작할 때 (start_web_swing): 곁의 들 수 있는 동물 하나를 ride_odds로 태움
    pub(super) fn pick_rider(&mut self) {
        let reach = RIDE_REACH * self.cfg.size;
        let (x, y) = (self.body.x, self.body.y);
        let near: Vec<PetSpot> = self.web.pets.iter().filter(|s| s.key != self.social_key && s.liftable && s.owner.is_some() && (s.x - x).hypot(s.y - y) <= reach).copied().collect();
        if near.is_empty() || self.web.pal.rng.f64() >= self.web.pal.ride_odds {
            return;
        }
        let s = near[self.web.pal.rng.usize(..near.len())];
        self.web.pal.rider = Some((s.key, s.size));
        self.web.pal.board_now = Some(s.key);
    }

    /// 🌉🤝 지금 그네에 태우고 있는 동물 (줄을 쏘는 중·떠오르는 중·진자 — hoisting이 봄)
    pub(super) fn web_rider(&self) -> Option<u64> {
        match self.state {
            State::Web { phase: WebPhase::Shoot { aim: WebAim::Anchor(_), .. } | WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. } => self.web.pal.rider.map(|r| r.0),
            _ => None,
        }
    }

    /// 🌉🤝 태운 동물 발이 올 자리 (물리 px — hoist_point가 봄): 거미인간 발 조금 뒤, 떠오른 뒤엔 조금 아래에 매달림
    pub(super) fn rider_point(&self) -> Option<(f64, f64)> {
        let (_, size) = self.web.pal.rider.filter(|_| self.web_rider().is_some())?;
        let hang = if matches!(self.state, State::Web { phase: WebPhase::Shoot { .. }, .. }) { 0.0 } else { RIDE_HANG * size };
        Some((self.body.x - self.facing * RIDE_BEHIND * self.cfg.size, self.body.y + hang))
    }

    /// 🌉🤝 그네를 놓음 (vx, vy = 놓는 속도): 태운 동물도 같은 속도로 놓아 줌
    pub(super) fn drop_rider(&mut self, vx: f64, vy: f64) {
        if let Some((key, _)) = self.web.pal.rider.take() {
            self.web.pal.drop_now = Some((key, vx, vy));
        }
    }

    /// 🌉🤝 태운 동물을 놓쳤음 (잡아채였거나 사라짐 — lose_hoist가 부름): 잊고 그네는 계속
    pub(super) fn lose_rider(&mut self) {
        self.web.pal.rider = None;
        self.web.pal.board_now = None;
    }

    /// 🌉🤝 태워졌던 동물: 그네 끝에서 (vx, vy)로 놓여 날아감 ("whee!" — 사뿐 내려섬)
    fn leave_ride(&mut self, vx: f64, vy: f64) {
        self.standing = None;
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.thrown = false;
        self.gentle_drop = true;
        self.speech = Some(Speech::Whee);
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 🕸️🎣 닿는 거리에 쉬는 동물이 있으면 주사위: 구석 거미집으로 낚아채기
    pub(super) fn maybe_snatch(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.web.pal.snatch_ready_at {
            return false;
        }
        let pets = self.cocoon_targets(world);
        if pets.is_empty() || self.web.pal.rng.f64() >= SNATCH_RATE * dt {
            return false;
        }
        let k = self.web.pal.rng.usize(..pets.len());
        self.start_snatch(&pets[k], world)
    }

    /// 🕸️🎣 가까운 구석에 거미집을 치기 시작하고, 다 짜면 이 동물을 낚아챔 (주사위 없이). 칠 구석이 없으면 false
    pub(super) fn start_snatch(&mut self, spot: &PetSpot, world: &World) -> bool {
        let size = self.cfg.size;
        let spots = web::corner_spots(world, (self.body.x, self.body.y - 0.6 * size), web::WEB_REACH * size, size);
        if spots.is_empty() {
            return false;
        }
        let corner = spots[self.web.pal.rng.usize(..spots.len())];
        let corner = self.sized_web(corner, world);
        self.web.pal.snatch_ready_at = self.clock + behavior::range(&mut self.web.pal.rng, SNATCH_CALM);
        self.facing = if corner.x >= self.body.x { 1.0 } else { -1.0 };
        self.begin_shoot(WebAim::Corner(corner));
        self.web.pal.snatch = Some(spot.key);
        true
    }

    /// 🕸️🎣 거미집을 다 짰을 때 (spot): 낚아챌 동물이 아직 들 수 있으면 그쪽으로 줄을 쏨. 쐈으면 true
    pub(super) fn snatch_after_weave(&mut self, spot: WebSpin) -> bool {
        let Some(key) = self.web.pal.snatch.take() else { return false };
        let Some(s) = self.web.pets.iter().find(|s| s.key == key && s.liftable).copied() else { return false };
        let (ix, iy) = spot.corner.inward();
        let k = SNATCH_DEPTH * spot.size * std::f64::consts::FRAC_1_SQRT_2;
        let to = (spot.x + ix * k, spot.y + iy * k + 0.5 * s.size);
        self.facing = if s.x >= self.body.x { 1.0 } else { -1.0 };
        self.begin_shoot(WebAim::Snatch { key, at: (s.x, s.y - 0.5 * s.size), to });
        true
    }

    /// 🕸️🎣 낚는 줄이 닿음: 그 동물을 거미집(to = 발이 올 자리)으로 끌어옴 ("gotcha!")
    pub(super) fn snatch_hit(&mut self, key: u64, at: (f64, f64), to: (f64, f64)) {
        let from = self.web.pets.iter().find(|s| s.key == key).map_or(at, |s| (s.x, s.y));
        let (vx, vy) = web::yank_velocity(from, to, SNATCH_FLIGHT_SECS, GRAVITY);
        self.web.pal.snatch_now = Some((key, vx, vy));
        self.speech = Some(Speech::Gotcha);
        self.state = State::Web { phase: WebPhase::Pull { tip: at }, t: 0.0 };
    }
}

/// 🕷️🤝 앱이 매 틱 동물 update 뒤에 (sync_lifts가 맨 먼저 부름): 그네에 태우기·그네 끝에 놓기·거미집으로 끌어오기
pub(super) fn sync_pals<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    let find = |pets: &[T], key: u64| pets.iter().position(|p| p.as_ref().social_key == key);
    for i in 0..pets.len() {
        let me = pets[i].as_ref().social_key;
        if let Some(key) = pets[i].as_mut().web.pal.board_now.take() {
            let ok = find(pets, key).filter(|&j| j != i).is_some_and(|j| pets[j].as_mut().be_lifted(me));
            if ok {
                let j = find(pets, key).unwrap_or(i);
                pets[j].as_mut().speech = Some(Speech::Whee);
            } else {
                pets[i].as_mut().lose_rider();
            }
        }
        if let Some((key, vx, vy)) = pets[i].as_mut().web.pal.drop_now.take() {
            if let Some(j) = find(pets, key) {
                if matches!(pets[j].as_ref().state, State::Lifted { by } if by == me) {
                    pets[j].as_mut().leave_ride(vx, vy);
                }
            }
        }
        if let Some((key, vx, vy)) = pets[i].as_mut().web.pal.snatch_now.take() {
            if let Some(j) = find(pets, key).filter(|&j| j != i) {
                let p = pets[j].as_mut();
                if p.be_yanked(vx, vy) {
                    p.sure_snare(SNATCH_SURE_SECS);
                }
            }
        }
    }
}
