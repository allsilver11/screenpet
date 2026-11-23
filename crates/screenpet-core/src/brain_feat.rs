//! 🎮 조작 모드 특기 (Z 키) — brain.rs의 자식 모듈. 조작 모드 한 틱 (brain_control.rs update_control)이 맨 먼저 update_feat를 부름.
//! 원래 있던 놀이의 그림·숫자를 빌려 씀 (soar.rs·broom_fly.rs·bike.rs·web.rs).
//!
//! - 🛡️ 날기 (무쇠인간·슈퍼인간 — cfg.soars): Z = 날아오름 (몸 TAKE_OFF_RISE만큼 떠서 hover). 방향키로 위아래 좌우 (중력 없음, 걷기 FEAT_FLY_SPEED배),
//!   Shift = FEAT_BOOST배 (fly_fast). Z 한 번 더 = 활공으로 사뿐 내려옴, ↓로 디딤판(창 윗면·바닥)에 닿으면 그 자리에 내려앉음.
//! - 🧹 빗자루 (해리·헤르미온느·볼드모트 — cfg.rides_broom): 날기와 같고 broom 그림·빗자루 빠르기 (broom_fly::BROOM_SPEED). Z = 빗자루째 천천히 내려옴.
//! - 🦇 오토바이 (박쥐인간 — cfg.rides_bike, 🧔 해그리드 — 사이드카, cfg.gamekeeper): Z = 오르기/내리기. 타는 동안 ←/→가 걷기 대신
//!   bike::BIKE_SPEED배로 달리고 (Shift FEAT_BOOST배), 그림은 ride (해그리드 sidecar)·서 있으면 drift·공중이면 bike_jump (sidecar_fly).
//!   창 끝에서 그대로 몰고 떨어지고, 점프·친구 끝 나가기는 평소 조작 그대로. 벽 타기는 안 함.
//! - 🕷️ 웹스윙 (거미인간 — cfg.slings_webs): Z = 보는 쪽 (안 되면 반대쪽) 위 창 아랫변·화면 윗변에 줄을 붙여 (web::swing_anchor) 당겨 오른 뒤 (Zip) 그네.
//!   저절로 놓지 않음. 가는 쪽 방향키 = 펌프 (더 높이 — SWING_MAX_THETA까지), 반대쪽 = 브레이크. Space·↓ = 놓고 그 속도로 날아감.
//!   공중 (놓고 날아가는 중·그네 중)에서 Z = 앞 위로 새 줄 (web::grab_anchor — 지금 자리·속도 그대로 이어 감). 화면 밖으로는 안 흔들림.
//! - 그 밖의 펫은 Z가 아무것도 안 함.
//! - 볼일 (먹이·잡기·하우스 등)이 끼어들거나 조작 모드가 끝나면 접음 (drop_feat): 날던 중이면 활공·빗자루째 사뿐, 줄에 매달렸으면 놓음.
//! - 친구 화면에 간 내 펫도 됨 — Z는 친구 링크 키 비트 64 (예전 판 친구는 모르는 비트라 무시함, 프로토콜은 그대로).

use super::control_moves::ControlInput;
use super::web_moves::WebPhase;
use super::*;
use crate::{bike, broom_fly as bf, web};

/// 🛡️ 날기 빠르기 (걷기 속도 배 — 🌱 soft_power)
pub const FEAT_FLY_SPEED: f64 = 3.0;
/// Shift를 누르면 이만큼 더 빠르게 (날기·빗자루·오토바이)
pub const FEAT_BOOST: f64 = 1.6;
/// 날아오를 때 몸이 뜨는 높이 (몸 크기 배)
pub const TAKE_OFF_RISE: f64 = 0.35;
/// 🕷️ 펌프 각가속도 (rad/s²)
pub const PUMP_ACCEL: f64 = 1.6;
/// 🕷️ 그네가 수직에서 기울 수 있는 가장 큰 각 (rad)
pub const SWING_MAX_THETA: f64 = 1.3;

/// Z 특기 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Feat {
    /// 🛡️ 자유 비행
    Fly,
    /// 🧹 빗자루 비행
    Broom,
    /// 🦇 오토바이 (🧔 사이드카)
    Bike,
    /// 🕷️ 웹스윙
    Swing,
}

impl Pet {
    /// 이 펫의 Z 특기 (없으면 None — 원래 나는 동물도 없음)
    pub(super) fn feat_kind(&self) -> Option<Feat> {
        let c = &self.cfg;
        if c.flies {
            None
        } else if c.soars {
            Some(Feat::Fly)
        } else if c.slings_webs {
            Some(Feat::Swing)
        } else if c.rides_bike || c.gamekeeper {
            Some(Feat::Bike)
        } else if c.rides_broom {
            Some(Feat::Broom)
        } else {
            None
        }
    }

    /// 지금 켜 둔 특기 (조작 모드가 아니면 None)
    pub(super) fn feat_on(&self) -> Option<Feat> {
        self.control.and_then(|c| c.feat)
    }

    /// 켜 둔 특기를 바꿈 (조작 모드가 아니면 아무것도 안 함)
    fn set_feat(&mut self, feat: Option<Feat>) {
        if let Some(c) = self.control.as_mut() {
            c.feat = feat;
        }
    }

    /// 🦇 오토바이에 타고 있음
    pub(super) fn on_bike(&self) -> bool {
        self.feat_on() == Some(Feat::Bike)
    }

    /// 🦇 오토바이 빠르기 (px/s — boost = Shift). 안 타고 있으면 None
    pub(super) fn bike_feat_speed(&self, boost: bool) -> Option<f64> {
        let mult = if boost { FEAT_BOOST } else { 1.0 };
        self.on_bike().then(|| self.cfg.walk_speed * bike::BIKE_SPEED * self.soft_power() * mult)
    }

    /// 🦇 오토바이 그림 (render가 맨 먼저 봄): 달리면 ride, 공중이면 bike_jump, 서 있으면 drift (해그리드는 sidecar·sidecar_fly). 아니면 None
    pub(super) fn feat_anim(&self) -> Option<Anim> {
        if !self.on_bike() {
            return None;
        }
        let sidecar = !self.cfg.rides_bike;
        Some(match self.state {
            State::Move { .. } if sidecar => Anim::Sidecar,
            State::Move { .. } => Anim::Ride,
            State::Air { .. } if sidecar => Anim::SidecarFly,
            State::Air { .. } => Anim::BikeJump,
            State::Idle { .. } | State::Turn { .. } | State::Land { .. } | State::Trick { kind: Trick::SitFront, .. } if sidecar => Anim::Sidecar,
            State::Idle { .. } | State::Turn { .. } | State::Land { .. } | State::Trick { kind: Trick::SitFront, .. } => Anim::Drift,
            _ => return None,
        })
    }

    /// 조작 모드 한 틱의 맨 앞 (update_control): Z로 특기를 켜고 끄고, 날기·그네 중이면 그 한 틱. 이번 틱을 다 처리했으면 true
    /// (오토바이는 켜 두기만 하고 false — 평소 조작이 빠르기·그림만 바꿔 씀)
    pub(super) fn update_feat(&mut self, dt: f64, world: &World, input: ControlInput, prev: ControlInput) -> bool {
        let pressed = input.special && !prev.special;
        match self.feat_on() {
            Some(kind @ (Feat::Fly | Feat::Broom)) => {
                if pressed {
                    self.drop_feat();
                    return true;
                }
                if self.standing.is_some() || !matches!(self.state, State::Air { .. }) {
                    // 다른 길로 내려섬 (탈것에 올라탐 등)
                    self.set_feat(None);
                    return false;
                }
                self.feat_fly(dt, world, input, kind);
                true
            }
            Some(Feat::Swing) => {
                if !matches!(self.state, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. }) {
                    self.set_feat(None);
                    return false;
                }
                self.feat_swing(dt, world, input, prev, pressed);
                true
            }
            Some(Feat::Bike) => {
                if pressed {
                    self.set_feat(None);
                }
                false
            }
            None if pressed => self.start_feat(world),
            None => false,
        }
    }

    /// Z를 막 누름: 이 펫의 특기를 켬. 벽에 매달려 있을 땐 안 함. 이번 틱을 다 처리했으면 true
    fn start_feat(&mut self, world: &World) -> bool {
        let Some(kind) = self.feat_kind() else { return false };
        if matches!(self.state, State::Climb { .. } | State::Slide | State::WallClimb { .. }) {
            return false;
        }
        match kind {
            Feat::Fly | Feat::Broom => {
                if self.standing.take().is_some() {
                    self.body.y -= TAKE_OFF_RISE * self.cfg.size;
                }
                let b = self.fly_limits(world);
                self.body = Body { x: self.body.x.clamp(b.left, b.right), y: self.body.y.clamp(b.top, b.bottom), vx: 0.0, vy: 0.0 };
                self.soar.gliding = false;
                self.broom.descending = false;
                self.speech = Some(if kind == Feat::Fly { Speech::TakeOff } else { Speech::BroomUp });
                self.set_feat(Some(kind));
                self.state = State::Air { anim: self.feat_fly_anim(kind, false, false) };
                true
            }
            Feat::Bike => {
                self.speech = Some(Speech::Vroom);
                self.set_feat(Some(Feat::Bike));
                false
            }
            Feat::Swing => self.feat_shoot(world),
        }
    }

    /// 날기·빗자루 그림: 빗자루는 늘 broom (🐉 용), 날기는 떠 있으면 hover · 날면 fly · 빠르면 fly_fast
    fn feat_fly_anim(&self, kind: Feat, moving: bool, fast: bool) -> Anim {
        match kind {
            Feat::Broom => self.mount_anim(),
            _ if !moving => Anim::Hover,
            _ if fast => Anim::FlyFast,
            _ => Anim::Fly,
        }
    }

    /// 🛡️🧹 비행 한 틱: 방향키로 위아래 좌우 (화면 안), 키가 없으면 그 자리에 뜸. ↓로 디딤판에 닿으면 내려앉고 특기를 끔
    fn feat_fly(&mut self, dt: f64, world: &World, input: ControlInput, kind: Feat) {
        let (dx, dy) = (input.dir(), input.vertical());
        let base = if kind == Feat::Broom { bf::BROOM_SPEED } else { FEAT_FLY_SPEED };
        let speed = self.cfg.walk_speed * base * self.soft_power() * if input.run { FEAT_BOOST } else { 1.0 };
        let moving = dx != 0.0 || dy != 0.0;
        let (mut x, mut y) = (self.body.x, self.body.y);
        if moving {
            let len = dx.hypot(dy);
            x += dx / len * speed * dt;
            y += dy / len * speed * dt;
        }
        if dx != 0.0 {
            self.facing = dx;
        }
        let b = self.fly_limits(world);
        let prev_y = self.body.y;
        let (x, y) = (x.clamp(b.left, b.right), y.clamp(b.top, b.bottom));
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        if input.down {
            // 바닥에 딱 붙어 있어도 닿은 것으로 (0.5px 더 봄)
            if let Some(s) = world.fall_landing(x, prev_y, y + 0.5, 0.0, false) {
                self.feat_land(world, s, kind);
                return;
            }
        }
        self.state = State::Air { anim: self.feat_fly_anim(kind, moving, input.run) };
    }

    /// 🛡️🧹 디딤판 s에 사뿐 내려앉고 특기를 끔 ("touchdown!" / "smooth landing")
    fn feat_land(&mut self, world: &World, s: Surface, kind: Feat) {
        let Some((ax, _)) = world.anchor(s.owner) else { return };
        self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
        self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - ax });
        self.set_feat(None);
        let secs = if kind == Feat::Broom {
            self.broom.landing = true;
            self.speech = Some(Speech::BroomLand);
            bf::LAND_SECS
        } else {
            self.speech = Some(Speech::SoftLand);
            crate::soar::SOFT_LAND_SECS
        };
        self.state = State::Land { remaining: secs };
    }

    /// 특기를 접음 (Z 다시·볼일·조작 끝): 날던 중이면 활공 (슈퍼인간)·빗자루째 천천히 내려옴, 줄에 매달렸으면 지금 속도로 놓음
    pub(super) fn drop_feat(&mut self) {
        let Some(kind) = self.feat_on() else { return };
        self.set_feat(None);
        match (kind, self.state) {
            (Feat::Fly | Feat::Broom, State::Air { .. }) if self.standing.is_none() => {
                self.body.vx = 0.0;
                self.body.vy = 0.0;
                if kind == Feat::Fly {
                    self.soar.gliding = true;
                } else {
                    self.broom.descending = true;
                }
                self.state = State::Air { anim: Anim::Fall };
            }
            (Feat::Swing, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. }) => {
                // 그네 중엔 body.vx·vy가 늘 지금 줄 끝 속도 (Zip은 0)
                self.standing = None;
                self.thrown = false;
                self.gentle_drop = false;
                self.state = State::Air { anim: Anim::Fall };
            }
            _ => {}
        }
    }

    /// 🕷️ 줄을 쏨: 서 있으면 보는 쪽 (안 되면 반대쪽) 위에 붙여 당겨 오름 (Zip), 공중이면 가는 쪽 위에 붙여 지금 자리·속도 그대로 그네.
    /// 닻이 없으면 false
    fn feat_shoot(&mut self, world: &World) -> bool {
        let (gx, drop) = self.swing_hand_offset();
        let size = self.cfg.size;
        let floor = world.work_area_at(self.body.x, self.body.y - 1.0).bottom;
        if self.standing.is_some() {
            for facing in [self.facing, -self.facing] {
                let hand = (self.body.x + facing * gx, self.body.y - drop);
                if let Some(anchor) = web::swing_anchor(world, hand, facing, size, drop, floor) {
                    self.facing = facing;
                    self.standing = None;
                    self.web.chain_left = 0;
                    self.body.vx = 0.0;
                    self.body.vy = 0.0;
                    self.speech = Some(Speech::Thwip);
                    self.set_feat(Some(Feat::Swing));
                    self.state = State::Web { phase: WebPhase::Zip { anchor, from: (self.body.x, self.body.y) }, t: 0.0 };
                    return true;
                }
            }
            return false;
        }
        if !matches!(self.state, State::Air { .. } | State::Web { phase: WebPhase::Swing { .. }, .. }) {
            return false;
        }
        let dir = if self.body.vx.abs() > 1.0 { self.body.vx.signum() } else { self.facing };
        let hand = (self.body.x + dir * gx, self.body.y - drop);
        let Some(anchor) = web::grab_anchor(world, hand, dir, size, drop, floor) else { return false };
        let theta = -anchor.angle;
        let omega = web::rope_omega(anchor.len, theta, self.body.vx, self.body.vy, dir);
        self.facing = dir;
        self.web.chain_left = 0;
        self.speech = Some(Speech::Thwip);
        self.set_feat(Some(Feat::Swing));
        self.state = State::Web { phase: WebPhase::Swing { anchor, theta, omega }, t: 0.0 };
        true
    }

    /// 🕷️ 그네 한 틱: 당겨 오르는 중이면 step_zip, 그네면 진자 + 펌프·브레이크. Space·↓ = 놓음, Z = 새 줄 (공중 잇기)
    fn feat_swing(&mut self, dt: f64, world: &World, input: ControlInput, prev: ControlInput, pressed: bool) {
        let State::Web { phase, t } = self.state else { return };
        let t = t + dt;
        if (input.jump && !prev.jump) || (input.down && !prev.down) {
            self.drop_feat();
            return;
        }
        match phase {
            WebPhase::Zip { anchor, from } => self.step_zip(anchor, from, t),
            WebPhase::Swing { anchor, theta, omega } => {
                if pressed && self.feat_shoot(world) {
                    return;
                }
                self.feat_pendulum(dt, world, input, anchor, theta, omega, t);
            }
            _ => self.set_feat(None),
        }
    }

    /// 🕷️ 진자 한 걸음: 가는 쪽 방향키 = 펌프 (빨라짐), 반대쪽 = 브레이크 (0을 넘지 않음). SWING_MAX_THETA·화면 끝에서 멈춰 되돌아옴
    #[allow(clippy::too_many_arguments)]
    fn feat_pendulum(&mut self, dt: f64, world: &World, input: ControlInput, anchor: web::Anchor, theta: f64, omega: f64, t: f64) {
        // ω > 0 = 보는 쪽으로 감
        let going = (self.facing * omega).signum();
        let d = input.dir();
        let omega = if d != 0.0 && omega != 0.0 && d == going {
            omega + omega.signum() * PUMP_ACCEL * dt
        } else if d != 0.0 && omega != 0.0 {
            let slowed = omega - omega.signum() * PUMP_ACCEL * dt;
            if slowed * omega < 0.0 { 0.0 } else { slowed }
        } else {
            omega
        };
        let (mut th, mut om) = web::pendulum_step(theta, omega, anchor.len, GRAVITY, dt);
        if th.abs() > SWING_MAX_THETA {
            th = th.clamp(-SWING_MAX_THETA, SWING_MAX_THETA);
            om = 0.0;
        }
        let (gx, drop) = self.swing_hand_offset();
        let facing = self.facing;
        let feet = |th: f64| {
            let hand = web::pendulum_hand((anchor.x, anchor.y), anchor.len, th, facing);
            (hand.0 - facing * gx, hand.1 + drop)
        };
        let (half_w, size, b) = (self.cfg.half_w, self.cfg.size, world.bounds);
        let on_screen = |(x, y): (f64, f64)| x - half_w >= b.left && x + half_w <= b.right && y <= b.bottom && y - size >= b.top;
        let mut at = feet(th);
        if !on_screen(at) {
            th = theta;
            om = 0.0;
            at = feet(th);
        }
        let (vx, vy) = web::release_velocity(anchor.len, th, om, facing);
        self.body = Body { x: at.0, y: at.1, vx, vy };
        self.state = State::Web { phase: WebPhase::Swing { anchor, theta: th, omega: om }, t };
    }
}
