//! 🙆 다른 동물 번쩍 들기 (흐물이, v0.6.3) — brain.rs의 자식 모듈. 같은 화면 동물들 사이의 일은 notice_pets·sync_lifts (앱이 틱마다).
//!
//! - 같은 바닥에서 쉬거나 걷는 다른 동물 (내 동물·손님 모두)을 처음 알아채면 LIFT_CHANCE로 다가가 쪼그려 머리 위로 번쩍 (lift → hold).
//!   들린 동물은 State::Lifted — 잡힌 그림(dragged)으로 버둥버둥 (render.wobble) "놔줘!". 흐물이는 조금 들고 다니다가
//!   살며시 내려놓는다 (stuff 그림 — 던지지 않는다, 들린 동물은 사뿐 떨어져 선다).
//! - 친구에게 데려가기: 앱이 set_lift_edges로 알려 준 끝(그 끝 친구가 새 버전이고, 들린 동물이 이 화면 주인의 동물일 때만 — 손님은 절대
//!   남의 동물을 데려가지 않는다)까지 같은 바닥으로 갈 수 있으면 CARRY_CHANCE로 그 끝으로 달려가 들고 나간다. 다 나가면 sync_lifts가
//!   들린 동물도 같은 끝으로 나간 것으로 한다 (그 동물의 take_exit = 같은 끝 — 앱이 그 동물의 방문을 같은 친구에게 보낸다).
//! - 들지 않는 동물: 사용자가 잡고 있거나, 하우스 안, 먹는 중, 공중, 날고 있는 새·유령, 무언가 들고 있거나 들고 있는 흐물이,
//!   떠나고 들어오는 길, 볼일이 있는 동물. 몸이 BIG_RATIO(2)배 넘게 큰 동물은 끙차 하다가 털썩 (strain → slump).
//! - 쿨다운 COOLDOWN_SECS, 같은 동물은 SAME_PET_SECS 동안 다시 주사위를 굴리지 않는다.

use super::*;
use crate::juggle;

/// 처음 알아챈 동물마다 들러 갈 확률
pub const LIFT_CHANCE: f64 = 0.2;
/// 알아채는 거리 (몸 배, 같은 바닥)
pub const NOTICE_REACH: f64 = 4.0;
/// 같은 동물은 이만큼 다시 주사위를 굴리지 않는다
pub const SAME_PET_SECS: f64 = 60.0;
/// 한 번 들고(또는 털썩) 나면 다시 들기까지
pub const COOLDOWN_SECS: (f64, f64) = (120.0, 240.0);
const FIRST_READY_SECS: f64 = 30.0;
/// 이보다 크면 (내 몸의 배) 끙차 하다가 털썩
pub const BIG_RATIO: f64 = 2.0;
/// 들고 다니는 시간 (내려놓기 전)
pub const WANDER_SECS: (f64, f64) = (2.0, 5.0);
/// 친구에게 데려갈 확률 (데려갈 수 있을 때)
pub const CARRY_CHANCE: f64 = 0.5;
/// 다가가다 이만큼 못 닿으면 그만둔다
const APPROACH_SECS: f64 = 6.0;
/// 내려놓기: 쪼그려 내려놓고 (PUT_RELEASE초에 놓는다) PUT_SECS에 끝 (그림 stuff 4칸 6fps)
pub const PUT_SECS: f64 = 0.7;
pub const PUT_RELEASE: f64 = 0.5;
/// 흐물이가 이만큼 오래 옮겨 주지 않으면 들린 동물은 그냥 떨어진다
const LIFT_STALE_SECS: f64 = 0.5;
/// 들린 동물이 "놔줘!" 하는 간격
const PLEAD_SECS: f64 = 2.5;

/// 같은 화면의 다른 동물 (notice_pets가 채운다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetSpot {
    pub key: u64,
    /// 발 가운데 (물리 px)
    pub x: f64,
    pub y: f64,
    /// 서 있는 디딤판 (없으면 공중)
    pub owner: Option<Owner>,
    pub size: f64,
    /// 지금 들 수 있다 (lift_ok)
    pub liftable: bool,
    /// 손님 (다른 사람의 동물 — 친구에게 데려가지 않는다)
    pub guest: bool,
}

/// 들어 올린 뒤 할 일
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoistPlan {
    /// 들고 조금 다니다가 근처에 살며시 내려놓는다
    PutDown,
    /// 그 끝으로 달려가 친구에게 데려간다
    Carry(Side),
}

/// 들기 단계 (State::Hoist — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoistPhase {
    /// 그 동물 옆(x)으로 다가간다
    Approach { key: u64, x: f64 },
    /// 너무 큰 동물에 끙차
    Strain { key: u64 },
    /// 털썩
    Slump,
    /// 쪼그려 번쩍
    Lift { key: u64 },
    /// 들고 서 있다가 다닌다
    Wander { key: u64, secs: f64 },
    /// 친구 쪽 끝으로 달려간다
    CarryOff { key: u64, side: Side },
    /// 살며시 내려놓는다 (released = 이미 놓았다)
    PutDown { key: u64, released: bool },
}

#[derive(Debug, Clone)]
pub struct HoistMood {
    rng: fastrand::Rng,
    pub(super) ready_at: f64,
    seen: Vec<(u64, f64)>,
    spots: Vec<PetSpot>,
    plan: HoistPlan,
    /// 데려갈 수 있는 끝 (앱이 정한다 — 손님 흐물이만)
    edges: (bool, bool),
    /// 들고 화면 끝으로 나가는 중인 동물
    out_with: Option<u64>,
    /// 들고 나갔다 (동물, 끝) — sync_lifts가 한 번 꺼낸다
    carried_out: Option<(u64, Side)>,
    /// 방금 들어 올렸다 / 내려놓았다 — sync_lifts가 한 번 꺼낸다
    lifted_now: Option<u64>,
    put_now: Option<u64>,
    /// 들린 동물: 마지막으로 옮겨진 시각, 다음 "놔줘!"
    lifted_seen: f64,
    plead_in: f64,
}

impl HoistMood {
    pub(super) fn new(seed: u64) -> Self {
        HoistMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x686f_6973),
            ready_at: FIRST_READY_SECS,
            seen: Vec::new(),
            spots: Vec::new(),
            plan: HoistPlan::PutDown,
            edges: (false, false),
            out_with: None,
            carried_out: None,
            lifted_now: None,
            put_now: None,
            lifted_seen: 0.0,
            plead_in: 0.0,
        }
    }
}

impl Pet {
    /// 🙆 이 동물을 지금 들 수 있다 (다른 흐물이가 볼 때)
    pub fn lift_ok(&self) -> bool {
        matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. } | State::Sleep { .. })
            && self.standing.is_some()
            && !self.housed
            && !self.is_eating()
            && self.inside.is_none()
            && !self.has_errand()
            && !self.is_heading_out()
            && !self.rider_on
            && self.juggle.carry.is_none()
            && self.hoisting().is_none()
    }

    /// 🙆 다른 흐물이에게 보이는 이 동물
    pub fn pet_spot(&self) -> PetSpot {
        PetSpot { key: self.social_key, x: self.body.x, y: self.body.y, owner: self.standing.map(|s| s.owner), size: self.cfg.size, liftable: self.lift_ok(), guest: self.guest.is_some() }
    }

    /// 🙆 데려갈 수 있는 끝 (앱이 매 틱 — 손님 흐물이에게만, 그 끝의 친구가 새 버전이고 이 화면 주인의 동물이 놀러 갈 수 있을 때)
    pub fn set_lift_edges(&mut self, left: bool, right: bool) {
        self.hoist.edges = (left, right);
    }

    /// 지금 들고 있는 동물 (social_key)
    pub fn hoisting(&self) -> Option<u64> {
        match self.state {
            State::Hoist { phase: HoistPhase::Lift { key } | HoistPhase::Wander { key, .. } | HoistPhase::CarryOff { key, .. } | HoistPhase::PutDown { key, released: false }, .. } => Some(key),
            State::Exit { .. } | State::Away => self.hoist.out_with,
            _ => None,
        }
    }

    /// 들린 동물의 발이 올 자리 (물리 px) — 흐물이 손 자리 (그림의 grip)
    pub fn hoist_point(&self) -> Option<(f64, f64)> {
        self.hoisting()?;
        let g = &self.cfg.grip;
        let (gx, gy) = match self.state {
            State::Hoist { phase, t } => match phase {
                HoistPhase::Lift { .. } => g.lift.at(t),
                HoistPhase::Wander { .. } => g.carry.at(t),
                HoistPhase::CarryOff { .. } => g.carry_run.at(t),
                HoistPhase::PutDown { .. } => g.stuff.at(t + 0.2),
                _ => g.hold.at(0.0),
            },
            _ => g.carry_run.at(self.clock),
        };
        let front = self.render().anim.is_front();
        let gx = if self.facing < 0.0 && !front { -gx } else { gx };
        let (_, top) = self.window_pos();
        Some((self.body.x + gx * self.cfg.size, top + gy * self.cfg.size))
    }

    /// 들린 동물: 흐물이(by)에게 번쩍 들렸다
    pub fn be_lifted(&mut self, by: u64) -> bool {
        if !self.lift_ok() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.speech = Some(Speech::PutMeDown);
        self.hoist.lifted_seen = self.clock;
        self.hoist.plead_in = PLEAD_SECS;
        self.state = State::Lifted { by };
        true
    }

    /// 들린 동물을 흐물이 손 자리로 (sync_lifts가 매 틱)
    pub fn lifted_to(&mut self, x: f64, y: f64) {
        if matches!(self.state, State::Lifted { .. }) {
            self.body.x = x;
            self.body.y = y;
            self.hoist.lifted_seen = self.clock;
        }
    }

    /// 들린 동물을 살며시 내려놓았다: 사뿐 떨어져 선다
    pub fn put_down(&mut self) {
        if !matches!(self.state, State::Lifted { .. }) {
            return;
        }
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.gentle_drop = true;
        self.thrown = false;
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 들린 채 흐물이와 함께 화면 끝(side)으로 나갔다 — take_exit가 같은 끝을 돌려준다
    pub fn carried_out(&mut self, side: Side) {
        if matches!(self.state, State::Lifted { .. }) {
            self.vanish(side);
        }
    }

    /// 들려 있다
    pub fn is_lifted(&self) -> bool {
        matches!(self.state, State::Lifted { .. })
    }

    pub(super) fn update_lifted(&mut self, dt: f64, _world: &World, _by: u64) {
        self.wobble_timer = self.wobble_timer.max(2.0 * dt);
        if self.clock - self.hoist.lifted_seen > LIFT_STALE_SECS {
            self.put_down();
            return;
        }
        self.hoist.plead_in -= dt;
        if self.hoist.plead_in <= 0.0 {
            self.hoist.plead_in = PLEAD_SECS;
            self.speech = Some(Speech::PutMeDown);
        }
    }

    /// 앱이 매 틱 (notice_pets): 같은 화면의 다른 동물
    pub fn notice_pets(&mut self, spots: &[PetSpot]) {
        self.hoist.spots.clear();
        self.hoist.spots.extend(spots.iter().filter(|s| s.key != self.social_key).copied());
    }

    /// 손이 비었고 쉬는 때가 지났으면 처음 본 가까운 동물에 주사위
    pub(super) fn maybe_hoist(&mut self, world: &World) -> bool {
        if !self.cfg.juggler || self.clock < self.hoist.ready_at || !self.free_hands() || !self.antics_free() {
            return false;
        }
        let Some(st) = self.standing else { return false };
        let (x, y, clock, reach) = (self.body.x, self.body.y, self.clock, NOTICE_REACH * self.cfg.size);
        let seen = &self.hoist.seen;
        let pick = self
            .hoist
            .spots
            .iter()
            .filter(|s| s.liftable && s.owner == Some(st.owner) && (s.y - y).abs() <= juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach)
            .filter(|s| !seen.iter().any(|&(k, until)| k == s.key && clock < until))
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .copied();
        let Some(spot) = pick else { return false };
        self.hoist.seen.retain(|&(_, until)| clock < until);
        self.hoist.seen.push((spot.key, clock + SAME_PET_SECS));
        if self.hoist.rng.f64() >= LIFT_CHANCE {
            return false;
        }
        self.start_hoist(&spot, world)
    }

    /// 그 동물 옆으로 다가가기 시작한다 (주사위 없이). 같은 바닥 조각 위에 설 자리가 없으면 false
    pub fn start_hoist(&mut self, spot: &PetSpot, world: &World) -> bool {
        let Some(to) = self.hoist_x(spot, world) else { return false };
        self.hoist.ready_at = self.clock + behavior::range(&mut self.hoist.rng, COOLDOWN_SECS);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.state = State::Hoist { phase: HoistPhase::Approach { key: spot.key, x: to }, t: 0.0 };
        true
    }

    /// 들 자리: 그 동물 바로 옆 (큰 동물은 몸을 붙인 옆)
    fn hoist_x(&self, spot: &PetSpot, world: &World) -> Option<f64> {
        let st = self.standing?;
        let piece = world.piece_at(st.owner, self.body.x)?;
        if !(piece.x_start..=piece.x_end).contains(&spot.x) {
            return None;
        }
        let dir = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        let gap = 0.25 * self.cfg.size + 0.3 * spot.size;
        let want = spot.x - dir * gap;
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        if lo > hi {
            return None;
        }
        let x = want.clamp(lo, hi);
        ((x - want).abs() <= 0.3 * self.cfg.size).then_some(x)
    }

    fn end_hoist(&mut self) {
        self.hoist.out_with = None;
        self.state = self.rest_beat();
    }

    /// 들고 있던 동물을 놓쳤다 (사용자가 낚아챘거나 사라졌다): "hmph!"
    pub fn lose_hoist(&mut self) {
        if self.hoisting().is_none() {
            return;
        }
        self.hoist.out_with = None;
        if matches!(self.state, State::Hoist { .. }) {
            self.speech = Some(Speech::Hmph);
            self.state = self.rest_beat();
        }
    }

    /// 들어 올린 뒤 할 일: 데려갈 수 있는 끝까지 같은 바닥이고 손님이 아닌 동물이면 CARRY_CHANCE로 데려간다
    fn hoist_plan(&mut self, key: u64, world: &World) -> HoistPlan {
        let own = self.hoist.spots.iter().find(|s| s.key == key).is_some_and(|s| !s.guest);
        let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
        let reach = |side: Side, dir: f64| piece.is_some_and(|p| visit::screen_edge(world, &p, dir) == Some(side));
        let mut sides = Vec::new();
        if self.hoist.edges.0 && reach(Side::Left, -1.0) {
            sides.push(Side::Left);
        }
        if self.hoist.edges.1 && reach(Side::Right, 1.0) {
            sides.push(Side::Right);
        }
        if own && !sides.is_empty() && self.hoist.rng.f64() < CARRY_CHANCE {
            return HoistPlan::Carry(sides[self.hoist.rng.usize(..sides.len())]);
        }
        HoistPlan::PutDown
    }

    pub(super) fn update_hoist(&mut self, dt: f64, world: &World, phase: HoistPhase, t: f64) {
        let t = t + dt;
        if self.antics_interrupted() {
            self.lose_hoist_quietly();
            return;
        }
        let next = |phase| State::Hoist { phase, t };
        match phase {
            HoistPhase::Approach { key, x } => {
                // 그새 들 수 없게 됐거나 사라졌다
                let Some(spot) = self.hoist.spots.iter().find(|s| s.key == key).copied().filter(|s| s.liftable) else {
                    self.end_hoist();
                    return;
                };
                if t > APPROACH_SECS {
                    self.end_hoist();
                    return;
                }
                let x = self.hoist_x(&spot, world).unwrap_or(x);
                let dx = x - self.body.x;
                let step = self.cfg.walk_speed * Move::Run.speed_mult() * dt;
                if dx.abs() > step {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, self.facing, step).is_some() {
                        self.state = State::Hoist { phase: HoistPhase::Approach { key, x }, t };
                    }
                    return;
                }
                if self.juggle_walk(world, dx.signum(), dx.abs()).is_none() {
                    return;
                }
                if (spot.x - self.body.x).abs() > 1.0 {
                    self.facing = (spot.x - self.body.x).signum();
                }
                if spot.size > BIG_RATIO * self.cfg.size {
                    self.speech = Some(Speech::Strain);
                    self.state = State::Hoist { phase: HoistPhase::Strain { key }, t: 0.0 };
                } else {
                    self.speech = Some(Speech::Hup);
                    self.hoist.lifted_now = Some(key);
                    self.state = State::Hoist { phase: HoistPhase::Lift { key }, t: 0.0 };
                }
            }
            HoistPhase::Strain { .. } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < juggle::STRAIN_SECS {
                    self.state = next(phase);
                } else {
                    self.speech = Some(Speech::Phew);
                    self.state = State::Hoist { phase: HoistPhase::Slump, t: 0.0 };
                }
            }
            HoistPhase::Slump => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < juggle::SLUMP_SECS {
                    self.state = next(phase);
                    return;
                }
                self.facing = -self.facing;
                self.state = State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.hoist.rng, Move::Walk.secs()) };
            }
            HoistPhase::Lift { key } => {
                if !self.follow_anchor(world) {
                    self.lose_hoist_quietly();
                    self.start_fall();
                    return;
                }
                if t < juggle::LIFT_SECS {
                    self.state = next(phase);
                    return;
                }
                self.hoist.plan = self.hoist_plan(key, world);
                self.state = match self.hoist.plan {
                    HoistPlan::Carry(side) => State::Hoist { phase: HoistPhase::CarryOff { key, side }, t: 0.0 },
                    HoistPlan::PutDown => State::Hoist { phase: HoistPhase::Wander { key, secs: behavior::range(&mut self.hoist.rng, WANDER_SECS) }, t: 0.0 },
                };
            }
            HoistPhase::Wander { key, secs } => {
                let step = self.walk_speed() * juggle::WANDER_PACE * dt;
                let Some(blocked) = self.juggle_walk(world, self.facing, step) else {
                    self.lose_hoist_quietly();
                    return;
                };
                if blocked {
                    self.facing = -self.facing;
                }
                self.state = if t < secs { next(phase) } else { State::Hoist { phase: HoistPhase::PutDown { key, released: false }, t: 0.0 } };
            }
            HoistPhase::CarryOff { key, side } => {
                let dir = visit::dir(side);
                self.facing = dir;
                let Some(blocked) = self.juggle_walk(world, dir, self.carry_run_speed() * dt) else {
                    self.lose_hoist_quietly();
                    return;
                };
                if !blocked {
                    self.state = next(phase);
                    return;
                }
                let open = match side {
                    Side::Left => self.exit_edges.left && self.hoist.edges.0,
                    _ => self.exit_edges.right && self.hoist.edges.1,
                };
                let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
                if open && piece.is_some_and(|p| visit::screen_edge(world, &p, dir) == Some(side)) {
                    // 들고 나간다 — 다 나가면 들린 동물도 같은 끝으로 (sync_lifts)
                    self.hoist.out_with = Some(key);
                    self.start_exit(side);
                } else {
                    // 그새 친구가 떠났다: 끝에 살며시 내려놓는다
                    self.speech = Some(Speech::Tada);
                    self.state = State::Hoist { phase: HoistPhase::PutDown { key, released: false }, t: 0.0 };
                }
            }
            HoistPhase::PutDown { key, released } => {
                if !self.follow_anchor(world) {
                    self.lose_hoist_quietly();
                    self.start_fall();
                    return;
                }
                if !released && t >= PUT_RELEASE {
                    self.hoist.put_now = Some(key);
                    self.state = State::Hoist { phase: HoistPhase::PutDown { key, released: true }, t };
                } else if t >= PUT_SECS {
                    self.end_hoist();
                } else {
                    self.state = next(phase);
                }
            }
        }
    }

    /// 하던 들기를 그만둔다 (들린 동물은 sync_lifts가 내려놓는다)
    fn lose_hoist_quietly(&mut self) {
        self.hoist.out_with = None;
        if matches!(self.state, State::Hoist { .. }) {
            self.state = self.rest_beat();
        }
    }

    /// 화면 끝으로 다 나갔다 (vanish): 들고 나간 동물을 sync_lifts에 알린다
    pub(super) fn hoist_vanished(&mut self, side: Side) {
        if let Some(key) = self.hoist.out_with.take() {
            self.hoist.carried_out = Some((key, side));
        }
    }

    pub(super) fn hoist_anim(&self, phase: HoistPhase) -> Anim {
        match phase {
            HoistPhase::Approach { .. } => Anim::Run,
            HoistPhase::Strain { .. } => Anim::Strain,
            HoistPhase::Slump => Anim::Slump,
            HoistPhase::Lift { .. } => Anim::Lift,
            HoistPhase::Wander { .. } => Anim::Carry,
            HoistPhase::CarryOff { .. } => self.carry_run_anim(),
            HoistPhase::PutDown { .. } => Anim::Stuff,
        }
    }
}

/// 같은 화면 동물들의 일 — 앱이 매 틱 동물 update 전에: 흐물이들에게 다른 동물을 보여 준다
pub fn notice_pets<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) {
    if !pets.iter().any(|p| p.as_ref().cfg.antics && p.as_ref().cfg.juggler) {
        return;
    }
    let spots: Vec<PetSpot> = pets.iter().filter(|p| !p.as_ref().is_away()).map(|p| p.as_ref().pet_spot()).collect();
    for p in pets.iter_mut() {
        let p = p.as_mut();
        if p.cfg.antics && p.cfg.juggler {
            p.notice_pets(&spots);
        }
    }
}

/// 들고 나간 동물: (흐물이 index, 들린 동물 index, 끝)
pub type CarriedOut = (usize, usize, Side);

/// 앱이 매 틱 동물 update 뒤에: 들어 올리기·옮기기·내려놓기·놓침·함께 나가기. 함께 나간 짝을 돌려준다
/// (들린 동물의 take_exit도 같은 끝 — 앱은 그 동물이 흐물이와 같은 친구에게 가게 한다)
pub fn sync_lifts<T: AsRef<Pet> + AsMut<Pet>>(pets: &mut [T]) -> Vec<CarriedOut> {
    let mut out = Vec::new();
    let find = |pets: &[T], key: u64| pets.iter().position(|p| p.as_ref().social_key == key);
    for i in 0..pets.len() {
        let lifter_key = pets[i].as_ref().social_key;
        // 방금 들어 올렸다
        if let Some(key) = pets[i].as_mut().hoist.lifted_now.take() {
            let ok = find(pets, key).is_some_and(|j| pets[j].as_mut().be_lifted(lifter_key));
            if !ok {
                pets[i].as_mut().lose_hoist();
            }
        }
        // 내려놓았다
        if let Some(key) = pets[i].as_mut().hoist.put_now.take() {
            if let Some(j) = find(pets, key) {
                pets[j].as_mut().put_down();
            }
        }
        // 함께 나갔다
        if let Some((key, side)) = pets[i].as_mut().hoist.carried_out.take() {
            if let Some(j) = find(pets, key) {
                if matches!(pets[j].as_ref().state, State::Lifted { by } if by == lifter_key) {
                    pets[j].as_mut().carried_out(side);
                    out.push((i, j, side));
                }
            }
        }
        // 들고 있는 동안 손 자리로 옮긴다 — 들린 동물이 그새 잡히거나 사라졌으면 놓친다
        if let (Some(key), Some(at)) = (pets[i].as_ref().hoisting(), pets[i].as_ref().hoist_point()) {
            match find(pets, key) {
                Some(j) if matches!(pets[j].as_ref().state, State::Lifted { by } if by == lifter_key) => pets[j].as_mut().lifted_to(at.0, at.1),
                _ => pets[i].as_mut().lose_hoist(),
            }
        }
    }
    // 흐물이가 더는 들고 있지 않은 들린 동물은 내려놓는다
    let holding: Vec<(u64, u64)> = pets.iter().filter_map(|p| p.as_ref().hoisting().map(|k| (p.as_ref().social_key, k))).collect();
    for p in pets.iter_mut() {
        let p = p.as_mut();
        if let State::Lifted { by } = p.state {
            if !holding.contains(&(by, p.social_key)) {
                p.put_down();
            }
        }
    }
    out
}
