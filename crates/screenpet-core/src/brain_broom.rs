//! 🧹 빗자루 비행 (pet.json `broom_rider` + broom 그림 — PetConfig::rides_broom, 해리·헤르미온느, 나중에 볼드모트도) — brain.rs의 자식 모듈.
//! 숫자·길·스니치는 broom_fly.rs.
//!
//! - 한가할 때 가끔 빗자루에 올라 (Mount) 화면의 허공 점 몇 개를 낮은 호·높은 호로 이어 날고 (Leg — 디딤판을 따라가지 않음),
//!   마지막 호로 아무 디딤판 (창 윗면·바닥)에 사뿐 내려앉음 (Land + broom 그림). 내려앉을 디딤판이 비행 중 옮겨 가면 따라감.
//! - 친구가 연결된 옆 끝이 있으면 마지막 호를 그 끝 너머로 날아 나가 친구 화면으로 감 (take_exit — 걸어 나간 것과 같음).
//!   걸어서 나가고 들어올 때 (State::Exit·Enter, 옆 끝)도 빗자루를 타고 창을 조금 띄움 (broom_transit·broom_lift).
//! - 🟡 골든 스니치 (seeker — 해리): 가끔 화면 위쪽에 스니치가 나타나 (Spot — 올려다보고 "the Snitch!") 빗자루로 쫓음 (Chase).
//!   스니치는 broom_fly::Snitch가 몰고, 앱은 snitch_spot을 덮개 창에 그림. 잡으면 "Got it!" (Caught — 손에 쥔 채 잠깐 떠 있음) 뒤 내려앉음.
//! - 끊기면 (볼일·자리 비움·집중·하우스 — 잡기는 평소처럼 Pressed) 빗자루째 천천히 내려와 (descending — 낮은 중력) 사뿐.
//! - 훅: on_broom (빗자루 위) — 나중에 결투·다른 펫이 봄.
//!
//! 주사위는 자기 주사위(BroomMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::broom_fly::{self as bf, Snitch};

/// 호가 닿는 곳
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Dest {
    /// 허공의 점 (발 자리, 물리 px)
    Air((f64, f64)),
    /// 디딤판 (기준점에서 rel_x) — 닿으면 내려앉음
    Land { owner: Owner, rel_x: f64 },
    /// 화면 끝(side) 너머 (발 높이 y) — 다 나가면 친구 화면으로
    Out { side: Side, y: f64 },
}

/// 호 하나 (2차 베지어: from → ctrl → to, secs초)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leg {
    pub from: (f64, f64),
    pub ctrl: (f64, f64),
    pub to: Dest,
    pub secs: f64,
}

/// 빗자루 비행 단계 (State::Broom — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BroomPhase {
    /// 서서 빗자루에 오름 (legs = 날아갈 허공 점 수)
    Mount { legs: u8 },
    /// 호를 따라 낢 (left = 이 호 뒤 남은 허공 점 수)
    Leg { leg: Leg, left: u8 },
    /// 🟡 나타난 스니치를 서서 올려다봄
    Spot,
    /// 🟡 스니치를 쫓음
    Chase,
    /// 🟡 잡아서 쥔 채 떠 있음
    Caught,
}

/// 🟡 앱이 덮개 창에 그릴 스니치 (물리 px — 공 가운데, size = 공 지름, caught = 손에 잡힘)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnitchSpot {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub caught: bool,
}

/// 빗자루 비행의 기억
#[derive(Debug, Clone)]
pub struct BroomMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 빗자루를 탈 수 있음
    pub(super) ready_at: f64,
    /// 🟡 이 시각(clock)부터 스니치가 나올 수 있음
    pub(super) snitch_ready_at: f64,
    /// 🟡 지금 쫓는 스니치
    snitch: Option<Snitch>,
    /// 🟡 쫓는 빠르기 (물리 px/s)
    vel: (f64, f64),
    /// 끊긴 비행이 빗자루째 내려오는 중 (State::Air — 낮은 중력, broom 그림)
    pub(super) descending: bool,
    /// 빗자루로 내려앉는 중 (State::Land — broom 그림)
    pub(super) landing: bool,
    /// 친구 화면을 빗자루로 오갈 때 창을 띄운 높이 (물리 px — 천천히 오르내림)
    lift: f64,
}

impl BroomMood {
    /// 시드로 난수를 따로 둠. 빗자루는 BROOM_FIRST_SECS, 스니치는 SNITCH_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        BroomMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x6272_6f6f),
            ready_at: bf::BROOM_FIRST_SECS,
            snitch_ready_at: bf::SNITCH_FIRST_SECS,
            snitch: None,
            vel: (0.0, 0.0),
            descending: false,
            landing: false,
            lift: 0.0,
        }
    }
}

impl Pet {
    /// 🧹 빗자루 위 (오르는 중·나는 중·스니치 쫓기 — 다른 펫·나중의 결투가 봄)
    pub fn on_broom(&self) -> bool {
        matches!(self.state, State::Broom { phase: BroomPhase::Leg { .. } | BroomPhase::Chase | BroomPhase::Caught | BroomPhase::Mount { .. }, .. })
            || self.broom_transit()
    }

    /// 🧹 허공에 떠서 나는 중 (디딤판 없음)
    pub(super) fn broom_airborne(&self) -> bool {
        matches!(self.state, State::Broom { phase: BroomPhase::Leg { .. } | BroomPhase::Chase | BroomPhase::Caught, .. })
    }

    /// 비행 단계별 그림: 올려다보기 (스니치를 봄) 말고는 빗자루
    pub(super) fn broom_anim(&self, phase: BroomPhase) -> Anim {
        match phase {
            BroomPhase::Spot if self.cfg.looks_up => Anim::LookUp,
            BroomPhase::Spot => Anim::Look,
            _ => self.mount_anim(),
        }
    }

    /// 친구 화면으로 빗자루 타고 나가거나 들어오는 중 (옆 끝만, 손에 든 것이 없을 때)
    pub(super) fn broom_transit(&self) -> bool {
        self.cfg.rides_broom
            && matches!(self.state, State::Exit { side, .. } | State::Enter { side, .. } if !visit::is_vertical(side))
            && self.carried_object().is_none()
            && self.hoisting().is_none()
    }

    /// 친구 화면을 빗자루로 오가는 빠르기 (px/s)
    pub(super) fn broom_transit_speed(&self) -> f64 {
        self.cfg.walk_speed * bf::TRANSIT_SPEED * self.soft_power()
    }

    /// 창을 바닥보다 띄우는 높이 (물리 px — 빗자루로 오갈 때)
    pub(super) fn broom_lift(&self) -> f64 {
        if self.cfg.rides_broom {
            self.broom.lift
        } else {
            0.0
        }
    }

    /// 친구 화면으로 나가기 시작함: 빗자루에 올라 "off I go!"
    pub(super) fn broom_exit(&mut self) {
        if self.broom_transit() {
            self.speech = Some(Speech::BroomAway);
        }
    }

    /// 친구 화면에서 들어옴: 빗자루째 떠서 들어옴
    pub(super) fn broom_enter(&mut self) {
        if self.cfg.rides_broom {
            self.broom.lift = bf::TRANSIT_LIFT * self.cfg.size;
        }
    }

    /// 매 틱 맨 앞: 내려오기·내려앉기 표시를 정리하고, 오가기 높이를 천천히 옮기고, 비행이 아니면 스니치를 치움
    pub(super) fn broom_tick(&mut self, dt: f64) {
        if !self.cfg.rides_broom {
            return;
        }
        if !matches!(self.state, State::Air { .. }) {
            self.broom.descending = false;
        }
        if !matches!(self.state, State::Land { .. }) {
            self.broom.landing = false;
        }
        if !matches!(self.state, State::Broom { .. }) {
            self.broom.snitch = None;
        }
        let goal = if self.broom_transit() && self.standing.is_some() { bf::TRANSIT_LIFT * self.cfg.size } else { 0.0 };
        self.broom.lift = crate::physics::step_toward(self.broom.lift, goal, self.cfg.size * 1.5 * dt);
    }

    /// 🟡 앱이 덮개 창에 그릴 스니치 (쫓는 중이면 그 자리, 잡았으면 손 자리)
    pub fn snitch_spot(&self) -> Option<SnitchSpot> {
        let size = bf::SNITCH_SIZE * self.cfg.size;
        match self.state {
            State::Broom { phase: BroomPhase::Spot | BroomPhase::Chase, .. } => self.broom.snitch.as_ref().map(|s| SnitchSpot { x: s.pos.0, y: s.pos.1, size, caught: false }),
            State::Broom { phase: BroomPhase::Caught, .. } => {
                let (x, y) = self.broom_hand();
                Some(SnitchSpot { x, y, size, caught: true })
            }
            _ => None,
        }
    }

    /// 빗자루를 쥔 앞손 언저리 (물리 px — 스니치를 잡는 자리)
    fn broom_hand(&self) -> (f64, f64) {
        (self.body.x + self.facing * 0.32 * self.cfg.size, self.body.y - 0.5 * self.cfg.size)
    }

    /// 매 틱 (주문·책 앞): 나는 중이면 그 한 틱, 아니면 가끔 빗자루에 오르거나 🟡 스니치가 나옴. 처리했으면 true
    pub(super) fn update_broom(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.rides_broom {
            return false;
        }
        if let State::Broom { phase, t } = self.state {
            self.tick_broom(dt, world, phase, t + dt);
            return true;
        }
        if !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        if self.cfg.seeker && self.clock >= self.broom.snitch_ready_at && self.broom.rng.f64() < bf::SNITCH_RATE * self.power() * dt {
            return self.start_snitch(world);
        }
        if self.clock >= self.broom.ready_at && self.broom.rng.f64() < bf::BROOM_RATE * self.power() * dt {
            return self.start_broom_flight();
        }
        false
    }

    /// 빗자루에 오름 (서 있을 때만): 허공 점 수를 정하고 가끔 "up we go!"
    pub(super) fn start_broom_flight(&mut self) -> bool {
        if !self.cfg.rides_broom || self.standing.is_none() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        let b = &mut self.broom;
        b.ready_at = self.clock + behavior::range(&mut b.rng, bf::BROOM_CALM_SECS);
        let legs = b.rng.u8(bf::LEGS.0..=bf::LEGS.1);
        if b.rng.f64() < bf::UP_LINE_CHANCE {
            self.speech = Some(Speech::BroomUp);
        }
        self.state = State::Broom { phase: BroomPhase::Mount { legs }, t: 0.0 };
        true
    }

    /// 🟡 스니치가 나타남 (서 있을 때만): 화면 위쪽 어딘가에 — 올려다보고 "the Snitch!"
    pub(super) fn start_snitch(&mut self, world: &World) -> bool {
        if !self.cfg.rides_broom || self.standing.is_none() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        let size = self.cfg.size;
        let b = &mut self.broom;
        b.snitch_ready_at = self.clock + behavior::range(&mut b.rng, bf::SNITCH_CALM_SECS);
        b.ready_at = b.ready_at.max(self.clock + bf::BROOM_CALM_SECS.0);
        let (lo, hi) = (wa.left + 2.0 * size, (wa.right - 2.0 * size).max(wa.left + 2.0 * size));
        let x = lo + b.rng.f64() * (hi - lo);
        let y = wa.top + 1.2 * size + b.rng.f64() * ((wa.bottom - wa.top) * 0.45 - 1.2 * size).max(0.0);
        b.snitch = Some(Snitch::new((x, y), b.rng.u64(..)));
        self.facing = if x >= self.body.x { 1.0 } else { -1.0 };
        self.speech = Some(Speech::Snitch);
        self.state = State::Broom { phase: BroomPhase::Spot, t: 0.0 };
        true
    }

    /// 비행이 끊겨야 함 (볼일·자리 비움·집중·하우스·창 안)
    fn broom_interrupted(&self) -> bool {
        self.housed || self.has_errand() || self.presence_quiet() || self.is_user_away() || self.inside.is_some()
    }

    /// 비행을 그만둠: 서 있었으면 그 자리에서 쉬고, 날던 중이면 빗자루째 천천히 내려옴
    pub(super) fn broom_drop(&mut self) {
        self.broom.snitch = None;
        if self.standing.is_some() {
            self.state = self.idle_state();
            return;
        }
        self.body.vx = self.broom.vel.0.clamp(-self.cfg.walk_speed * 3.0, self.cfg.walk_speed * 3.0);
        self.body.vy = 0.0;
        self.broom.descending = true;
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 끊긴 비행의 내려오는 중력 (px/s²) — 내려오는 중이 아니면 None
    pub(super) fn descent_gravity(&self) -> Option<f64> {
        (self.cfg.rides_broom && self.broom.descending && !self.thrown).then_some(GRAVITY * bf::DESCENT_GRAVITY)
    }

    /// update_air에서 한 걸음 뒤: 빗자루째 내려오는 빠르기를 DESCENT_MAX_VY까지로
    pub(super) fn cap_descent(&mut self) {
        if self.descent_gravity().is_some() {
            self.body.vy = self.body.vy.min(bf::DESCENT_MAX_VY * self.cfg.size);
        }
    }

    /// land_or_recover에서 내려섬: 빗자루째 내려오던 중이면 사뿐 (콰당 없음 — 내려앉기 시간)
    pub(super) fn broom_landed(&mut self) -> Option<f64> {
        if !self.cfg.rides_broom || !self.broom.descending {
            return None;
        }
        self.broom.descending = false;
        self.broom.landing = true;
        Some(bf::LAND_SECS)
    }

    /// 비행 한 틱 (t = 이번 틱까지 흐른 시간)
    fn tick_broom(&mut self, dt: f64, world: &World, phase: BroomPhase, t: f64) {
        if self.broom_interrupted() {
            self.broom_drop();
            return;
        }
        match phase {
            BroomPhase::Mount { legs } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < bf::MOUNT_SECS {
                    self.state = State::Broom { phase, t };
                } else {
                    self.next_leg(world, legs);
                }
            }
            BroomPhase::Spot => {
                if !self.follow_anchor(world) {
                    self.broom.snitch = None;
                    self.start_fall();
                    return;
                }
                self.tick_snitch(dt, world);
                if t < bf::SPOT_SECS {
                    self.state = State::Broom { phase, t };
                } else {
                    self.standing = None;
                    self.broom.vel = (0.0, -0.5 * bf::CHASE_SPEED * self.cfg.size);
                    self.state = State::Broom { phase: BroomPhase::Chase, t: 0.0 };
                }
            }
            BroomPhase::Leg { leg, left } => self.fly_leg(dt, world, leg, left, t),
            BroomPhase::Chase => self.chase_snitch(dt, world, t),
            BroomPhase::Caught => {
                if t < bf::CAUGHT_SECS {
                    self.state = State::Broom { phase, t };
                } else {
                    self.broom.snitch = None;
                    self.next_leg(world, 0);
                }
            }
        }
    }

    /// 🟡 스니치 한 틱 (쫓는 손 쪽을 피함)
    fn tick_snitch(&mut self, dt: f64, world: &World) {
        let hand = self.broom_hand();
        let size = self.cfg.size;
        if let Some(s) = self.broom.snitch.as_mut() {
            let wa = world.work_area_at(s.pos.0, s.pos.1);
            s.tick(dt, wa, size, hand);
        }
    }

    /// 🟡 쫓기 한 틱: 스니치가 손에 오도록 몸을 몰고, 손에 닿으면 "Got it!", 너무 오래면 놓치고 내려앉음
    fn chase_snitch(&mut self, dt: f64, world: &World, t: f64) {
        self.tick_snitch(dt, world);
        let Some(pos) = self.broom.snitch.as_ref().map(|s| s.pos) else {
            self.next_leg(world, 0);
            return;
        };
        if t >= bf::GIVE_UP_SECS {
            self.broom.snitch = None;
            self.next_leg(world, 0);
            return;
        }
        let size = self.cfg.size;
        let hand = self.broom_hand();
        if (pos.0 - hand.0).hypot(pos.1 - hand.1) <= bf::CATCH_REACH * size {
            self.broom.vel = (0.0, 0.0);
            self.speech = Some(Speech::GotIt);
            self.state = State::Broom { phase: BroomPhase::Caught, t: 0.0 };
            return;
        }
        // 손이 스니치에 오는 몸 자리
        let want_at = (pos.0 - (hand.0 - self.body.x), pos.1 - (hand.1 - self.body.y));
        let (dx, dy) = (want_at.0 - self.body.x, want_at.1 - self.body.y);
        let d = dx.hypot(dy).max(1e-6);
        let speed = bf::CHASE_SPEED * size * self.soft_power();
        let want = (dx / d * speed.min(d / dt.max(1e-6)), dy / d * speed.min(d / dt.max(1e-6)));
        let k = (bf::CHASE_TURN * dt).min(1.0);
        self.broom.vel = (self.broom.vel.0 + (want.0 - self.broom.vel.0) * k, self.broom.vel.1 + (want.1 - self.broom.vel.1) * k);
        let wa = world.work_area_at(self.body.x, self.body.y - 1.0);
        self.body.x = (self.body.x + self.broom.vel.0 * dt).clamp(wa.left + self.cfg.half_w, (wa.right - self.cfg.half_w).max(wa.left + self.cfg.half_w));
        self.body.y = (self.body.y + self.broom.vel.1 * dt).clamp(wa.top + size, wa.bottom.max(wa.top + size));
        if self.broom.vel.0.abs() > 0.2 * size {
            self.facing = self.broom.vel.0.signum();
        }
        self.state = State::Broom { phase: BroomPhase::Chase, t };
    }

    /// 호가 닿는 자리 (물리 px) — 디딤판이 사라졌으면 None
    fn dest_point(&self, to: Dest, world: &World) -> Option<(f64, f64)> {
        match to {
            Dest::Air(p) => Some(p),
            Dest::Out { side, y } => Some((self.out_x(side, world), y)),
            Dest::Land { owner, rel_x } => {
                let (ax, _) = world.anchor(owner)?;
                let x = ax + rel_x;
                world.piece_at(owner, x).map(|s| (x, s.y))
            }
        }
    }

    /// side 끝 너머로 날아 나갈 x (물리 px)
    fn out_x(&self, side: Side, world: &World) -> f64 {
        let past = bf::OUT_PAST * self.cfg.size + self.cfg.half_w;
        match side {
            Side::Left => world.bounds.left - past,
            _ => world.bounds.right + past,
        }
    }

    /// 호 하나를 따라 한 틱: 닿으면 다음 호·내려앉기, 끝 너머로 다 나가면 친구 화면으로
    fn fly_leg(&mut self, dt: f64, world: &World, leg: Leg, left: u8, t: f64) {
        let Some(to) = self.dest_point(leg.to, world) else {
            // 내려앉으려던 디딤판이 사라짐: 지금 자리에서 새 곳으로
            self.next_leg(world, 0);
            return;
        };
        let u = (t / leg.secs.max(1e-6)).min(1.0);
        let p = bf::bezier(leg.from, leg.ctrl, to, bf::ease(u));
        let dx = p.0 - self.body.x;
        if dx.abs() > 0.3 {
            self.facing = dx.signum();
        }
        self.broom.vel = ((p.0 - self.body.x) / dt.max(1e-6), (p.1 - self.body.y) / dt.max(1e-6));
        self.standing = None;
        self.body.x = p.0;
        self.body.y = p.1;
        if let Dest::Out { side, .. } = leg.to {
            if visit::fully_outside(&world.bounds, side, self.body.x, self.cfg.half_w) || u >= 1.0 {
                self.vanish(side);
                return;
            }
        }
        if u < 1.0 {
            self.state = State::Broom { phase: BroomPhase::Leg { leg, left }, t };
            return;
        }
        match leg.to {
            Dest::Air(_) => self.next_leg(world, left),
            Dest::Land { owner, rel_x } => {
                self.standing = Some(Standing { owner, rel_x });
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                self.broom.vel = (0.0, 0.0);
                self.broom.landing = true;
                if self.speech.is_none() && self.broom.rng.f64() < bf::LAND_LINE_CHANCE {
                    self.speech = Some(Speech::BroomLand);
                }
                self.state = State::Land { remaining: bf::LAND_SECS };
            }
            Dest::Out { side, .. } => self.vanish(side),
        }
    }

    /// 다음 호를 정함: left > 0이면 허공 점으로, 아니면 친구 화면 끝 너머 (가끔) 또는 내려앉을 디딤판으로. 갈 곳이 없으면 빗자루째 내려옴
    fn next_leg(&mut self, world: &World, left: u8) {
        let from = (self.body.x, self.body.y);
        let wa = world.work_area_at(from.0, from.1 - 1.0);
        let size = self.cfg.size;
        let to = if left > 0 {
            Some(Dest::Air(self.air_point(wa, from)))
        } else {
            self.out_dest(wa).or_else(|| self.land_dest(world, wa))
        };
        let Some(to) = to else {
            self.standing = None;
            self.broom_drop();
            return;
        };
        let Some(end) = self.dest_point(to, world) else {
            self.broom_drop();
            return;
        };
        let b = &mut self.broom;
        let low = b.rng.f64() < bf::LOW_ARC_CHANCE;
        let lift = behavior::range(&mut b.rng, bf::LOW_ARC) * size;
        let high = behavior::range(&mut b.rng, bf::HIGH_ARC) * (wa.bottom - wa.top);
        let ctrl = bf::arc_ctrl(from, end, wa, bf::AIR_MARGIN * size, low.then_some(lift), high);
        let speed = self.cfg.walk_speed * bf::BROOM_SPEED * self.soft_power();
        let secs = (bf::bezier_len(from, ctrl, end) / speed.max(1.0)).max(bf::MIN_LEG_SECS);
        self.standing = None;
        self.state = State::Broom { phase: BroomPhase::Leg { leg: Leg { from, ctrl, to, secs }, left: left.saturating_sub(1) }, t: 0.0 };
    }

    /// 허공 점 하나 (작업 영역 안 — 지금 자리에서 가로로 몸 2개 넘게 떨어진 곳을 고름)
    fn air_point(&mut self, wa: Rect, from: (f64, f64)) -> (f64, f64) {
        let size = self.cfg.size;
        let (lo_x, hi_x) = (wa.left + size, (wa.right - size).max(wa.left + size));
        let (lo_y, hi_y) = (wa.top + bf::AIR_MARGIN * size, (wa.bottom - 0.6 * size).max(wa.top + bf::AIR_MARGIN * size));
        let b = &mut self.broom;
        let mut p = (lo_x + b.rng.f64() * (hi_x - lo_x), lo_y + b.rng.f64() * (hi_y - lo_y));
        for _ in 0..4 {
            if (p.0 - from.0).abs() >= 2.0 * size {
                break;
            }
            p.0 = lo_x + b.rng.f64() * (hi_x - lo_x);
        }
        p
    }

    /// 친구가 연결된 옆 끝 너머로 날아 나갈지 (걸어 나갈 확률의 반 — 집이 그리운 손님·보내기는 늘)
    fn out_dest(&mut self, wa: Rect) -> Option<Dest> {
        for side in [Side::Left, Side::Right] {
            let chance = self.exit_odds(side);
            let chance = if chance >= 1.0 { 1.0 } else { chance * 0.5 };
            if chance > 0.0 && self.broom.rng.f64() < chance {
                let size = self.cfg.size;
                let y = wa.top + bf::AIR_MARGIN * size + self.broom.rng.f64() * ((wa.bottom - wa.top) * 0.5).max(0.0);
                return Some(Dest::Out { side, y: y.min(wa.bottom - size) });
            }
        }
        None
    }

    /// 내려앉을 디딤판 (이 작업 영역의 창 윗면·바닥 중 넉넉한 것 하나, 그 위 아무 자리)
    fn land_dest(&mut self, world: &World, wa: Rect) -> Option<Dest> {
        let size = self.cfg.size;
        let half = self.cfg.half_w;
        let fits: Vec<Surface> = world
            .surfaces
            .iter()
            // 🌐 웹 페이지 디딤돌은 0.5초마다 바뀔 수 있어 빗자루 착지 자리로는 쓰지 않음
            .filter(|s| !matches!(s.owner, Owner::Inside(_)))
            .filter(|s| s.width() >= bf::LAND_MIN_WIDTH * size && s.y - size >= wa.top && s.y <= wa.bottom + 1.0)
            .filter(|s| (wa.left..wa.right).contains(&((s.x_start + s.x_end) / 2.0)))
            .copied()
            .collect();
        if fits.is_empty() {
            return None;
        }
        let s = fits[self.broom.rng.usize(..fits.len())];
        let (lo, hi) = (s.x_start + half, s.x_end - half);
        let x = lo + self.broom.rng.f64() * (hi - lo).max(0.0);
        let (ax, _) = world.anchor(s.owner)?;
        Some(Dest::Land { owner: s.owner, rel_x: x - ax })
    }
}
