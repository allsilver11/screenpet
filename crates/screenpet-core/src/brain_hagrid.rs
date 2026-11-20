//! 🧔 해그리드 (pet.json `gamekeeper` + HAGRID_ANIMS — PetConfig::gamekeeper) — brain.rs의 자식 모듈. 🐉 용 타기는 빗자루 비행 (brain_broom.rs)이
//! 그림만 dragon으로 바꿔 태움 (PetConfig::dragon — mount_anim).
//!
//! - 🏍️ 사이드카 오토바이 (Bike): 가끔 BIKE_SECS 동안 디딤판을 따라 걷기의 BIKE_SPEED배로 달림 (끝에 막히면 돌아섬), 달리는 동안 초당 FLY_HOP_RATE로
//!   오토바이째 붕 날아오름 (FLY_HOP_SECS 동안 FLY_HOP_RISE — 창만 뜸, sidecar_fly 그림). 타기 전 같은 바닥 HOP_IN_REACH 안에 작은 펫
//!   (내 몸의 SIDECAR_MAX배까지)이 있으면 HOP_IN_CHANCE로 "Hop in!" — 그 펫은 사이드카에 앉아 (State::Lifted — 앞모습, 아래 SIDECAR_COVER는 통에 가려짐)
//!   함께 다니다가 끝나면 내려섬. 들기·내려놓기는 흐물이 들기와 같은 길 (hoist.lifted_now·put_now → sync_lifts)
//! - 🤗 안기 (Hug): 같은 바닥 HUG_REACH 안의 다른 펫 (내 몸의 HUG_MAX배까지)에게 다가가 (Approach) 번쩍 안고 "C'mere, you!" (Pick) →
//!   HUG_WALK_SECS 동안 안고 걷다가 (Carry) 살며시 내려놓고 (Set) 🖐️ 쓰담쓰담 (Pat — 그 펫 머리 위에 하트, sync_hagrid). 안긴 펫은 버둥대지 않음
//! - 🖐️ 쓰담 (Pat): 가끔 가까운 펫에게 다가가 쓰다듬기만
//! - 🥚 용알 (Egg → Hatch): 가끔 앉아 EGG_SECS 동안 알을 품음 (알은 덮개 창 — CRACK_AT부터 금이 감) → 쩍! 아기 용이 나와 HATCH_SECS 동안 둘레를
//!   파닥파닥 날다가 작은 불을 퐁 뿜고 사라짐 (MagicFx::Egg·Hatchling)
//! - 🪨 락 케이크 (Offer): 가끔 "Rock cake?" 하고 코트에서 꺼내 발 앞 바닥에 내려놓음 — 앱이 돌 물건을 만듦 (ObjectPlay::Spawn, 물건 수 제한은 앱이 지킴).
//!   손님 해그리드는 안 함 (친구 화면에 물건을 늘리지 않게)
//! - 훅: is_hagrid (다른 펫·나중의 모닥불·하우스가 봄)
//!
//! 주사위는 자기 주사위(HagridMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::hoist_moves::PetSpot;
use super::*;
use crate::juggle::GripAnim;
use crate::magic::MagicFx;
use crate::object::ObjectKind;
use crate::play::ObjectPlay;

/// 처음 해그리드 놀이를 할 수 있게 되는 때 (clock 초)
pub const FIRST_SECS: f64 = 25.0;
/// 한가할 때 무언가 하는 빈도 (초당 — 🌱 power를 곱함). 고르는 몫은 아래 *_SHARE
pub const ACT_RATE: f64 = 0.02;
/// 다음 놀이까지 (초)
pub const CALM_SECS: (f64, f64) = (25.0, 70.0);
/// 고르는 몫: 🏍️ 오토바이 · 🤗 안기 (가까이 펫이 있을 때) · 🖐️ 쓰담 · 🥚 알 · 🪨 락 케이크 (나머지)
pub const BIKE_SHARE: f64 = 0.3;
pub const HUG_SHARE: f64 = 0.25;
pub const PAT_SHARE: f64 = 0.15;
pub const EGG_SHARE: f64 = 0.15;

/// 🏍️ 타는 시간 (초) · 빠르기 (걷기 배 — 🌱 soft_power)
pub const BIKE_SECS: (f64, f64) = (8.0, 16.0);
pub const BIKE_SPEED: f64 = 2.6;
/// 🏍️ 달리는 동안 붕 날아오르는 빈도 (초당) · 나는 시간 (초) · 높이 (몸 크기 배)
pub const FLY_HOP_RATE: f64 = 0.22;
pub const FLY_HOP_SECS: f64 = 1.6;
pub const FLY_HOP_RISE: f64 = 1.1;
/// 🏍️ 사이드카에 태울 펫: 같은 바닥 가로 이만큼 (내 몸 크기 배) 안, 내 몸의 이 배까지, 이 몫으로 "Hop in!"
pub const HOP_IN_REACH: f64 = 3.0;
pub const SIDECAR_MAX: f64 = 0.85;
pub const HOP_IN_CHANCE: f64 = 0.7;
/// 🏍️ 사이드카에 앉은 펫의 그림 아래 몫이 통에 가려짐 (render.seat)
pub const SIDECAR_COVER: f64 = 0.3;

/// 🤗 안을 펫: 같은 바닥 가로 이만큼 (내 몸 크기 배) 안, 내 몸의 이 배까지
pub const HUG_REACH: f64 = 4.0;
pub const HUG_MAX: f64 = 0.9;
/// 🤗 다가가다 이만큼 못 닿으면 그만둠 (초)
pub const APPROACH_SECS: f64 = 6.0;
/// 🤗 번쩍 안기 · 안고 걷기 · 내려놓기 (그 가운데 HUG_RELEASE에 놓음) 시간 (초)
pub const HUG_PICK_SECS: f64 = 0.8;
pub const HUG_WALK_SECS: (f64, f64) = (2.5, 5.0);
pub const HUG_SET_SECS: f64 = 0.8;
pub const HUG_RELEASE: f64 = 0.5;
/// 🤗 안고 걷는 빠르기 (걷기 배)
pub const HUG_PACE: f64 = 0.6;
/// 🖐️ 쓰담쓰담 시간 (초) · 쓰다듬긴 펫 하트 시간 (초)
pub const PAT_SECS: f64 = 1.8;
pub const PAT_HEART_SECS: f64 = 1.6;

/// 🥚 알 품는 시간 (초) · 금이 가기 시작하는 몫 · 아기 용이 날다 사라지기까지 (초)
pub const EGG_SECS: (f64, f64) = (8.0, 14.0);
pub const CRACK_AT: f64 = 0.55;
pub const HATCH_SECS: f64 = 5.0;
/// 🥚 알 크기 · 🐉 아기 용 크기 (몸 크기 배 — 덮개 창이 그림)
pub const EGG_SIZE: f64 = 0.16;
pub const HATCHLING_SIZE: f64 = 0.3;

/// 🪨 꺼내 놓는 시간 (offer 3칸 3fps + 잠깐) · 바닥에 놓는 때 (셋째 칸) · 발 앞 거리 (몸 크기 배)
pub const OFFER_SECS: f64 = 1.6;
pub const OFFER_PLACE: f64 = 0.7;
pub const OFFER_AHEAD: f64 = 0.45;

/// 🥚 알의 금 (0 = 매끈 … 1 = 다 갈라짐): 품은 몫 u (0…1)가 CRACK_AT를 넘으면서 차오름
pub fn egg_crack(u: f64) -> f64 {
    ((u - CRACK_AT) / (1.0 - CRACK_AT)).clamp(0.0, 1.0)
}

/// 다가간 뒤 할 일
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// 🤗 번쩍 안기
    Hug,
    /// 🖐️ 쓰담만
    Pat,
}

/// 해그리드가 하는 일 (State::Hagrid — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HagridAct {
    /// 🏍️ 사이드카 오토바이 len초 (passenger = 사이드카의 펫, hop_at = 붕 날기 시작 t — 없으면 음수)
    Bike { len: f64, passenger: Option<u64>, hop_at: f64 },
    /// 그 펫 옆 x로 걸어감
    Approach { key: u64, x: f64, then: Then },
    /// 🤗 번쩍 안음
    Pick { key: u64 },
    /// 🤗 안고 secs초 걸음
    Carry { key: u64, secs: f64 },
    /// 🤗 살며시 내려놓음 (released = 이미 놓음)
    Set { key: u64, released: bool },
    /// 🖐️ 쓰담쓰담
    Pat { key: u64 },
    /// 🥚 len초 알 품기
    Egg { len: f64 },
    /// 🐉 아기 용이 깨어나 날다 사라짐
    Hatch,
    /// 🪨 락 케이크 꺼내 놓기 (placed = 이미 놓음)
    Offer { placed: bool },
}

/// 해그리드의 기억
#[derive(Debug, Clone)]
pub struct HagridMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 새 놀이를 할 수 있음
    pub(super) ready_at: f64,
    /// 🖐️ 방금 쓰다듬은 펫 (sync_hagrid가 한 번 꺼내 하트)
    pat_now: Option<u64>,
}

impl HagridMood {
    /// 시드로 난수를 따로 둠. 첫 놀이는 FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        HagridMood { rng: fastrand::Rng::with_seed(seed ^ 0x6861_6772), ready_at: FIRST_SECS, pat_now: None }
    }
}

impl Pet {
    /// 🧔 해그리드임 (나중의 모닥불·하우스·다른 펫이 봄)
    pub fn is_hagrid(&self) -> bool {
        self.cfg.gamekeeper
    }

    /// 🐉 빗자루 비행 그림: 해그리드는 용, 아니면 빗자루
    pub(super) fn mount_anim(&self) -> Anim {
        if self.cfg.dragon {
            Anim::Dragon
        } else {
            Anim::Broom
        }
    }

    /// 하는 일별 그림
    pub(super) fn hagrid_anim(&self, act: HagridAct, t: f64) -> Anim {
        match act {
            HagridAct::Bike { hop_at, .. } if hop_at >= 0.0 && t - hop_at < FLY_HOP_SECS => Anim::SidecarFly,
            HagridAct::Bike { .. } => Anim::Sidecar,
            HagridAct::Approach { .. } => Anim::Walk,
            HagridAct::Pick { .. } | HagridAct::Set { .. } => Anim::Hug,
            HagridAct::Carry { .. } => Anim::HugWalk,
            HagridAct::Pat { .. } => Anim::Pat,
            HagridAct::Egg { .. } => Anim::Egg,
            HagridAct::Hatch if t < 0.6 => Anim::Egg,
            HagridAct::Hatch => Anim::Heart,
            HagridAct::Offer { .. } => Anim::Offer,
        }
    }

    /// 🏍️ 오토바이째 붕 떠오른 높이 (물리 px — 창만 뜸)
    pub(super) fn hagrid_lift(&self) -> f64 {
        match self.state {
            State::Hagrid { act: HagridAct::Bike { hop_at, .. }, t } if hop_at >= 0.0 => {
                crate::play::hop_height(t - hop_at, FLY_HOP_SECS, FLY_HOP_RISE * self.cfg.size)
            }
            _ => 0.0,
        }
    }

    /// 🏍️🤗 지금 사이드카에 태웠거나 안고 있는 펫 (hoisting이 봄)
    pub(super) fn hagrid_holding(&self) -> Option<u64> {
        match self.state {
            State::Hagrid { act: HagridAct::Bike { passenger, .. }, .. } => passenger,
            State::Hagrid { act: HagridAct::Pick { key } | HagridAct::Carry { key, .. } | HagridAct::Set { key, released: false }, .. } => Some(key),
            _ => None,
        }
    }

    /// 그림 손 자리 g (t초째)의 물리 px (오른쪽을 볼 때의 표 — 왼쪽이면 뒤집음)
    fn grip_point(&self, g: GripAnim, t: f64) -> (f64, f64) {
        let (gx, gy) = g.at(t);
        let gx = if self.facing < 0.0 { -gx } else { gx };
        let (_, top) = self.window_pos();
        (self.body.x + gx * self.cfg.size, top + gy * self.cfg.size)
    }

    /// 🏍️🤗 태운·안은 펫의 발 자리 (hoist_point가 봄)
    pub(super) fn hagrid_hold_point(&self) -> Option<(f64, f64)> {
        self.hagrid_holding()?;
        let State::Hagrid { act, t } = self.state else { return None };
        let g = &self.cfg.grip;
        let anim = self.hagrid_anim(act, t);
        let grip = match anim {
            Anim::SidecarFly => g.sidecar_fly,
            Anim::Sidecar => g.sidecar,
            Anim::HugWalk => g.hug_walk,
            _ => g.hug,
        };
        Some(self.grip_point(grip, t))
    }

    /// 태운·안은 펫이 앉는 모양 (sync_lifts가 들린 펫에게): 사이드카면 아래 몫이 가려짐, 안기면 0
    pub(super) fn hagrid_seat(&self) -> f64 {
        match self.state {
            State::Hagrid { act: HagridAct::Bike { .. }, .. } => SIDECAR_COVER,
            _ => 0.0,
        }
    }

    /// 태운·안은 펫을 놓쳤음 (사용자가 낚아챘거나 사라짐): 오토바이는 혼자 계속, 안기는 그만둠
    pub(super) fn hagrid_lost(&mut self) {
        match self.state {
            State::Hagrid { act: HagridAct::Bike { len, hop_at, .. }, t } => {
                self.state = State::Hagrid { act: HagridAct::Bike { len, passenger: None, hop_at }, t };
            }
            State::Hagrid { .. } => self.end_hagrid(),
            _ => {}
        }
    }

    /// 매 틱 (주문·책 뒤): 하는 중이면 그 한 틱, 아니면 가끔 오토바이·안기·쓰담·알·락 케이크. 처리했으면 true
    pub(super) fn update_hagrid(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.gamekeeper {
            return false;
        }
        if let State::Hagrid { act, t } = self.state {
            self.tick_hagrid(dt, world, act, t + dt);
            return true;
        }
        if !self.play_free() || self.romp_interrupted() || self.is_user_away() || self.clock < self.hagrid.ready_at {
            return false;
        }
        if self.hagrid.rng.f64() >= ACT_RATE * self.power() * dt {
            return false;
        }
        let roll = self.hagrid.rng.f64();
        let started = if roll < BIKE_SHARE {
            self.start_bike()
        } else if roll < BIKE_SHARE + HUG_SHARE {
            self.start_cuddle(world, Then::Hug)
        } else if roll < BIKE_SHARE + HUG_SHARE + PAT_SHARE {
            self.start_cuddle(world, Then::Pat)
        } else if roll < BIKE_SHARE + HUG_SHARE + PAT_SHARE + EGG_SHARE {
            self.start_egg()
        } else {
            self.start_offer()
        };
        if !started {
            // 할 수 없던 것 (곁에 펫이 없음 등) — 조금 뒤 다시
            self.hagrid.ready_at = self.clock + 5.0;
        }
        started
    }

    /// 새 놀이를 시작함: 쉬는 박자를 지우고 다음 놀이 때를 정함
    fn begin_hagrid(&mut self, act: HagridAct) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.hagrid.ready_at = self.clock + behavior::range(&mut self.hagrid.rng, CALM_SECS);
        self.state = State::Hagrid { act, t: 0.0 };
    }

    /// 놀이를 끝내고 쉬는 박자로
    fn end_hagrid(&mut self) {
        self.state = self.rest_beat();
    }

    /// 같은 바닥의 들 수 있는 가까운 펫 (가로 reach, 크기 max 배까지 — 내 몸 크기 배)
    fn nearby_pet(&self, reach: f64, max: f64) -> Option<PetSpot> {
        let st = self.standing?;
        let (x, y, size) = (self.body.x, self.body.y, self.cfg.size);
        self.hoist
            .spots
            .iter()
            .filter(|s| s.liftable && s.owner == Some(st.owner) && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX)
            .filter(|s| (s.x - x).abs() <= reach * size && s.size <= max * size)
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .copied()
    }

    /// 🏍️ 오토바이에 오름 (서 있을 때만): 가까운 작은 펫이 있으면 "Hop in!" 하고 사이드카에 태움
    pub(crate) fn start_bike(&mut self) -> bool {
        if !self.cfg.gamekeeper || self.standing.is_none() {
            return false;
        }
        let len = behavior::range(&mut self.hagrid.rng, BIKE_SECS);
        let mut passenger = None;
        if let Some(spot) = self.nearby_pet(HOP_IN_REACH, SIDECAR_MAX) {
            if self.hagrid.rng.f64() < HOP_IN_CHANCE {
                passenger = Some(spot.key);
                self.hoist.lifted_now = Some(spot.key);
            }
        }
        self.speech = Some(if passenger.is_some() { Speech::HopIn } else { Speech::Vroom });
        self.begin_hagrid(HagridAct::Bike { len, passenger, hop_at: -1.0 });
        true
    }

    /// 🤗🖐️ 가까운 펫에게 다가가기 시작함 (그 뒤 안기 또는 쓰담). 곁에 펫이 없으면 false
    pub(crate) fn start_cuddle(&mut self, world: &World, then: Then) -> bool {
        if !self.cfg.gamekeeper {
            return false;
        }
        let max = if then == Then::Hug { HUG_MAX } else { f64::INFINITY };
        let Some(spot) = self.nearby_pet(HUG_REACH, max) else { return false };
        let Some(x) = self.hoist_x(&spot, world) else { return false };
        self.begin_hagrid(HagridAct::Approach { key: spot.key, x, then });
        true
    }

    /// 🥚 그 자리에 앉아 알을 품음 (서 있을 때만)
    pub(crate) fn start_egg(&mut self) -> bool {
        if !self.cfg.gamekeeper || self.standing.is_none() {
            return false;
        }
        let len = behavior::range(&mut self.hagrid.rng, EGG_SECS);
        self.begin_hagrid(HagridAct::Egg { len });
        true
    }

    /// 🪨 "Rock cake?" (서 있을 때만, 손님은 안 함)
    pub(crate) fn start_offer(&mut self) -> bool {
        if !self.cfg.gamekeeper || self.standing.is_none() || self.guest.is_some() {
            return false;
        }
        self.speech = Some(Speech::RockCake);
        self.begin_hagrid(HagridAct::Offer { placed: false });
        true
    }

    /// 해그리드 놀이 한 틱 (t = 이번 틱까지 흐른 시간)
    fn tick_hagrid(&mut self, dt: f64, world: &World, act: HagridAct, t: f64) {
        if self.romp_interrupted() || self.is_user_away() {
            self.end_hagrid();
            return;
        }
        let next = |act| State::Hagrid { act, t };
        match act {
            HagridAct::Bike { len, passenger, mut hop_at } => {
                // 사이드카의 펫이 그새 사라졌으면 (sync_lifts가 lose_hoist) passenger는 이미 None
                let step = self.cfg.walk_speed * BIKE_SPEED * self.soft_power() * dt;
                let Some(blocked) = self.juggle_walk(world, self.facing, step) else { return };
                if blocked {
                    self.facing = -self.facing;
                }
                if hop_at >= 0.0 && t - hop_at >= FLY_HOP_SECS {
                    hop_at = -1.0;
                }
                if hop_at < 0.0 && t + FLY_HOP_SECS < len && self.hagrid.rng.f64() < FLY_HOP_RATE * dt {
                    hop_at = t;
                    if self.speech.is_none() {
                        self.speech = Some(Speech::Whee);
                    }
                }
                if t < len || hop_at >= 0.0 {
                    self.state = next(HagridAct::Bike { len, passenger, hop_at });
                    return;
                }
                if let Some(key) = passenger {
                    self.hoist.put_now = Some(key);
                }
                self.end_hagrid();
            }
            HagridAct::Approach { key, x, then } => {
                let Some(spot) = self.hoist.spots.iter().find(|s| s.key == key).copied().filter(|s| s.liftable) else {
                    self.end_hagrid();
                    return;
                };
                if t > APPROACH_SECS {
                    self.end_hagrid();
                    return;
                }
                let x = self.hoist_x(&spot, world).unwrap_or(x);
                let dx = x - self.body.x;
                let step = self.cfg.walk_speed * dt;
                if dx.abs() > step {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, self.facing, step).is_some() {
                        self.state = next(HagridAct::Approach { key, x, then });
                    }
                    return;
                }
                if self.juggle_walk(world, dx.signum(), dx.abs()).is_none() {
                    return;
                }
                if (spot.x - self.body.x).abs() > 1.0 {
                    self.facing = (spot.x - self.body.x).signum();
                }
                match then {
                    Then::Hug => {
                        self.speech = Some(Speech::Cuddle);
                        self.hoist.lifted_now = Some(key);
                        self.state = State::Hagrid { act: HagridAct::Pick { key }, t: 0.0 };
                    }
                    Then::Pat => self.start_pat(key),
                }
            }
            HagridAct::Pick { key } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                self.state = if t < HUG_PICK_SECS {
                    next(act)
                } else {
                    State::Hagrid { act: HagridAct::Carry { key, secs: behavior::range(&mut self.hagrid.rng, HUG_WALK_SECS) }, t: 0.0 }
                };
            }
            HagridAct::Carry { key, secs } => {
                let step = self.cfg.walk_speed * HUG_PACE * dt;
                let Some(blocked) = self.juggle_walk(world, self.facing, step) else { return };
                if blocked {
                    self.facing = -self.facing;
                }
                self.state = if t < secs { next(act) } else { State::Hagrid { act: HagridAct::Set { key, released: false }, t: 0.0 } };
            }
            HagridAct::Set { key, released } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if !released && t >= HUG_RELEASE {
                    self.hoist.put_now = Some(key);
                    self.state = next(HagridAct::Set { key, released: true });
                } else if t >= HUG_SET_SECS {
                    self.start_pat(key);
                } else {
                    self.state = next(act);
                }
            }
            HagridAct::Pat { .. } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < PAT_SECS {
                    self.state = next(act);
                } else {
                    self.end_hagrid();
                }
            }
            HagridAct::Egg { len } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < len {
                    self.state = next(act);
                } else {
                    self.speech = Some(Speech::Hatch);
                    self.state = State::Hagrid { act: HagridAct::Hatch, t: 0.0 };
                }
            }
            HagridAct::Hatch => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < HATCH_SECS {
                    self.state = next(act);
                } else {
                    self.end_hagrid();
                }
            }
            HagridAct::Offer { placed } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if !placed && t >= OFFER_PLACE {
                    let x = self.body.x + self.facing * OFFER_AHEAD * self.cfg.size;
                    self.play.event = Some(ObjectPlay::Spawn { kind: ObjectKind::Stone, x, y: self.body.y - 0.15 * self.cfg.size });
                    self.state = next(HagridAct::Offer { placed: true });
                } else if t >= OFFER_SECS {
                    self.end_hagrid();
                } else {
                    self.state = next(act);
                }
            }
        }
    }

    /// 🖐️ 쓰담쓰담 시작: "Good lad!" — 그 펫에게 하트 (sync_hagrid)
    fn start_pat(&mut self, key: u64) {
        self.speech = Some(Speech::Pat);
        self.hagrid.pat_now = Some(key);
        self.state = State::Hagrid { act: HagridAct::Pat { key }, t: 0.0 };
    }

    /// 🖐️ 해그리드가 쓰다듬음: 하트가 퐁 (하던 일은 그대로)
    pub fn be_patted(&mut self) {
        self.heart_timer = self.heart_timer.max(PAT_HEART_SECS);
    }

    /// 🥚🐉 덮개 창에 그릴 알·아기 용 (알을 품은 손 자리)
    pub(super) fn hagrid_fx(&self, out: &mut Vec<MagicFx>) {
        let State::Hagrid { act, t } = self.state else { return };
        let size = self.cfg.size;
        match act {
            HagridAct::Egg { len } => {
                let (x, y) = self.grip_point(self.cfg.grip.egg, t);
                out.push(MagicFx::Egg { x, y, size: EGG_SIZE * size, crack: egg_crack(t / len.max(1e-6)) });
            }
            HagridAct::Hatch => {
                let (x, y) = self.grip_point(self.cfg.grip.egg, 0.0);
                out.push(MagicFx::Hatchling { x, y, size: HATCHLING_SIZE * size, t, len: HATCH_SECS, dir: self.facing });
            }
            _ => {}
        }
    }
}

/// 🖐️ 앱이 매 틱 동물 update 뒤에 (sync_lifts가 부름): 해그리드가 쓰다듬은 펫에게 하트
pub(super) fn sync_hagrid<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    for i in 0..pets.len() {
        let Some(key) = pets[i].as_mut().hagrid.pat_now.take() else { continue };
        if let Some(j) = pets.iter().position(|p| p.as_ref().social_key == key) {
            pets[j].as_mut().be_patted();
        }
    }
}
