//! 🤹 물건 들어 던지기 (pet.json `juggler` — 흐물이)의 Pet 동작 — brain.rs의 자식 모듈 (비공개 필드에 접근). 숫자와 까닭은 juggle.rs.
//!
//! - `notice_objects`: 앱이 update 전에 바닥의 물건 자리와 같은 화면의 다른 동물 자리를 넘긴다. 처음 본 물건마다 한 번 주사위.
//! - 들고 있는 물건(`carried_object`)은 앱이 틱마다 손 자리(`carry_point`)로 옮긴다. 잡히거나 다른 일에 끼어들리면 손에서 놓친다
//!   (앱은 아무도 들고 있지 않은 물건을 떨어뜨린다). 사용자가 물건을 낚아채면 앱이 `lose_carry`를 부른다.
//! - 놓을 때는 `take_juggle`로 Release (속도) — 앱이 그 속도로 던진다.
//! - 배달 장난: 들고 친구 쪽 끝으로 걸어 나간다 (Exit 중에도 들고 있다). 앱이 Visit에 실어 보내면 `carry_handed_over`.
//!   친구 화면에 도착한 손님·못 들어가 돌아온 내 동물은 앱이 손에 쥐여 주고(`hold_object`) 들어와 서면 앞을 보고 휙 떨군다.

use super::*;
use crate::juggle::{self, JuggleEvent, ObjectSpot, Target};

/// 거대한 것 앞에 설 때 몸 가운데에서 그 옆면까지 (몸 크기 배) — 끙차 그림(strain)의 손바닥이 여기 닿는다 (tools/draw_heumul.py STRAIN_HANDS_X)
const GIANT_GAP: f64 = 0.28;

/// 들고 던지기 단계 (State::Juggle — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JugglePhase {
    /// 물건 옆(x)으로 다가간다. obj_x = 물건 가운데
    Approach { id: u64, x: f64, obj_x: f64, giant: bool },
    /// 🪨💩 거대한 것에 "끙차" — lifts면 끝내 들어 올린다 (시작할 때 정한다)
    Strain { id: u64, lifts: bool },
    /// 쪼그려 앉았다가 머리 위로 번쩍
    Lift,
    /// 든 채 잠깐 서 있다
    Hold { secs: f64 },
    /// 머리 위에 인 채 돌아다닌다
    Wander { secs: f64 },
    /// 배달 장난: 친구 쪽 끝으로 들고 간다
    CarryOff { side: Side },
    /// 젖혔다가 휙 (released = 이미 놓았다)
    Throw { released: bool },
    /// 앞을 보고 번쩍 든 채 섰다가 휙 떨군다 (배달 도착)
    Present { released: bool },
    /// 너무 무거워 털썩 주저앉음
    Slump,
}

/// 들어 올린 뒤 할 일 (줍는 순간 정한다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Plan {
    Throw,
    Wander,
    Prank(Side),
}

/// 들고 던지기의 기억
#[derive(Debug, Clone)]
pub struct JuggleMood {
    /// 이 동물만 굴리는 주사위 (다른 동물의 주사위 순서는 그대로)
    pub(super) rng: fastrand::Rng,
    /// 손에 든 물건 (앱의 물건 id)
    pub(super) carry: Option<u64>,
    /// 든 것이 거대한 것
    pub(super) giant: bool,
    /// 들고 들어와 서면 앞을 보고 떨군다 (배달 도착·돌아옴)
    pub(super) present_pending: bool,
    pub(super) plan: Plan,
    /// 이 시각(clock)까지는 새 물건을 줍지 않는다
    pub(super) calm_until: f64,
    /// 주사위를 굴린 물건과 다시 굴릴 수 있는 시각
    pub(super) seen: Vec<(u64, f64)>,
    /// 같은 화면의 다른 동물 (notice_objects가 채운다)
    pub(super) targets: Vec<Target>,
    /// 배달 장난을 해도 되는 끝 (새 버전 친구가 연결됨 — 앱이 정한다)
    pub(super) prank: (bool, bool),
    pub(super) event: Option<JuggleEvent>,
}

impl JuggleMood {
    pub(super) fn new(seed: u64) -> Self {
        JuggleMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x6a75_6767),
            carry: None,
            giant: false,
            present_pending: false,
            plan: Plan::Throw,
            calm_until: 0.0,
            seen: Vec::new(),
            targets: Vec::new(),
            prank: (false, false),
            event: None,
        }
    }
}

impl Pet {
    /// 배달 장난을 해도 되는 끝 (매 틱 — 내 동물만, 그 끝에 새 버전 친구가 연결돼 있고 놀러 갈 수 있을 때)
    pub fn set_prank_edges(&mut self, left: bool, right: bool) {
        self.juggle.prank = (left, right);
    }

    /// 손에 든 물건 (앱이 틱마다 손 자리로 옮긴다). 손에서 놓쳤으면 None
    pub fn carried_object(&self) -> Option<u64> {
        self.juggle.carry.filter(|_| self.carry_ok())
    }

    /// 앱에 알릴 일 (한 번만)
    pub fn take_juggle(&mut self) -> Option<JuggleEvent> {
        self.juggle.event.take()
    }

    /// 사용자가 손에 든 물건을 낚아챘거나 물건이 사라졌다: "hmph!" 하고 하던 일을 그만둔다
    pub fn lose_carry(&mut self) {
        if self.juggle.carry.take().is_none() {
            return;
        }
        self.juggle.present_pending = false;
        if matches!(self.state, State::Juggle { .. }) {
            self.speech = Some(Speech::Hmph);
            self.end_juggle();
        }
    }

    /// 들고 나간 물건을 친구에게 실어 보냈다 (이 화면에서는 끝)
    pub fn carry_handed_over(&mut self) {
        self.juggle.carry = None;
        self.juggle.present_pending = false;
    }

    /// 친구 화면에 들고 온 물건 (또는 못 들어가 들고 돌아온 것): 들어와 서면 앞을 보고 번쩍 든 채 섰다가 휙 떨군다
    pub fn hold_object(&mut self, id: u64) {
        self.juggle.carry = Some(id);
        self.juggle.giant = false;
        self.juggle.present_pending = true;
        self.juggle.calm_until = self.clock + behavior::range(&mut self.juggle.rng, juggle::CALM_SECS);
    }

    /// 손에 든 물건 바닥이 올 자리 (물리 px) — 손이 그 동작의 그 칸에 있는 곳
    pub fn carry_point(&self) -> Option<(f64, f64)> {
        self.carried_object()?;
        let g = &self.cfg.grip;
        let (gx, gy) = match self.state {
            State::Juggle { phase, t } => match phase {
                JugglePhase::Lift => g.lift.at(t),
                JugglePhase::Hold { .. } => g.hold.at(t),
                JugglePhase::Wander { .. } | JugglePhase::CarryOff { .. } => g.carry.at(t),
                JugglePhase::Throw { .. } => g.throw.at(t),
                JugglePhase::Present { .. } => g.present.at(t),
                _ => g.hold.at(0.0),
            },
            State::Move { .. } | State::Exit { .. } | State::Enter { .. } => g.carry.at(self.clock),
            _ => g.hold.at(0.0),
        };
        let front = self.render().anim.is_front();
        let gx = if self.facing < 0.0 && !front { -gx } else { gx };
        let (_, top) = self.window_pos();
        Some((self.body.x + gx * self.cfg.size, top + gy * self.cfg.size))
    }

    /// 손에 들고 있을 수 있는 때
    fn carry_ok(&self) -> bool {
        match self.state {
            State::Juggle { phase, .. } => matches!(
                phase,
                JugglePhase::Lift
                    | JugglePhase::Hold { .. }
                    | JugglePhase::Wander { .. }
                    | JugglePhase::CarryOff { .. }
                    | JugglePhase::Throw { released: false }
                    | JugglePhase::Present { released: false }
            ),
            State::Exit { side, .. } | State::Enter { side, .. } => !visit::is_vertical(side),
            State::Away => true,
            State::Move { .. } | State::Idle { .. } | State::Turn { .. } => self.juggle.present_pending && self.standing.is_some(),
            _ => false,
        }
    }

    /// 새 물건을 주우러 갈 수 있는 때: 스스로 걷거나 쉬는 중, 볼일 없음
    fn may_pick_up(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Walk | Move::Run, .. } | State::Idle { .. } | State::Turn { .. })
            && self.standing.is_some()
            && !self.cfg.flies
            && self.inside.is_none()
            && !self.housed
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.tickling
            && self.hover_since.is_none()
            && !self.thrown
            && self.juggle.carry.is_none()
            && self.clock >= self.juggle.calm_until
    }

    /// 매 틱 update 전에: 바닥의 물건과 같은 화면의 다른 동물 (주우러 가는 동물만 일이 있다)
    pub fn notice_objects(&mut self, spots: &[ObjectSpot], targets: &[Target], world: &World) {
        if !self.cfg.juggler {
            return;
        }
        self.juggle.targets.clear();
        self.juggle.targets.extend_from_slice(targets);
        if self.juggle.carry.is_some() && !self.carry_ok() {
            self.juggle.carry = None;
            self.juggle.present_pending = false;
        }
        if self.juggle.carry.is_some() && self.juggle.present_pending && matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. }) {
            self.start_present();
            return;
        }
        if let State::Juggle { phase: JugglePhase::Approach { id, giant, .. }, t } = self.state {
            let spot = spots.iter().find(|s| s.id == id && self.standing.is_some_and(|st| st.owner == s.owner)).copied();
            match spot.and_then(|s| self.approach_x(&s, world).map(|x| (x, s))) {
                Some((x, s)) => self.state = State::Juggle { phase: JugglePhase::Approach { id, x, obj_x: s.x, giant }, t },
                // 누가 치웠거나 던졌다
                None => self.end_juggle(),
            }
            return;
        }
        if !self.may_pick_up() {
            return;
        }
        let Some(st) = self.standing else { return };
        let (x, y, clock, reach) = (self.body.x, self.body.y, self.clock, juggle::NOTICE_REACH * self.cfg.size);
        let seen = &self.juggle.seen;
        let near = |s: &&ObjectSpot| {
            s.owner == st.owner && (s.y - y).abs() <= juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach && !seen.iter().any(|&(id, until)| id == s.id && clock < until)
        };
        let Some(spot) = spots.iter().filter(near).min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).copied() else { return };
        self.juggle.seen.retain(|&(_, until)| clock < until);
        self.juggle.seen.push((spot.id, clock + juggle::SAME_OBJECT_SECS));
        if self.juggle.rng.f64() >= juggle::PICK_CHANCE {
            return;
        }
        let Some(to) = self.approach_x(&spot, world) else { return };
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.state = State::Juggle { phase: JugglePhase::Approach { id: spot.id, x: to, obj_x: spot.x, giant: spot.giant }, t: 0.0 };
    }

    /// 물건을 들 자리: 작은 것은 손이 닿는 바로 앞, 거대한 것은 몸을 붙인 옆. 같은 조각 위에 설 수 없으면 None
    fn approach_x(&self, spot: &ObjectSpot, world: &World) -> Option<f64> {
        let st = self.standing?;
        let piece = world.piece_at(st.owner, self.body.x)?;
        if !(piece.x_start..=piece.x_end).contains(&spot.x) {
            return None;
        }
        let dir = if spot.x >= self.body.x { 1.0 } else { -1.0 };
        let gap = if spot.giant { spot.half_w + GIANT_GAP * self.cfg.size } else { 0.2 * self.cfg.size };
        let want = spot.x - dir * gap;
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        if lo > hi {
            return None;
        }
        let x = want.clamp(lo, hi);
        ((x - want).abs() <= 0.3 * self.cfg.size).then_some(x)
    }

    /// 디딤판을 따라 dir 쪽으로 speed만큼. 발밑이 사라졌으면 None (떨어진다), 조각 끝에 막혔으면 Some(true)
    fn juggle_walk(&mut self, world: &World, dir: f64, step: f64) -> Option<bool> {
        if !self.follow_anchor(world) {
            self.start_fall();
            return None;
        }
        let st = self.standing?;
        let x = self.body.x;
        let Some(piece) = world.piece_at(st.owner, x).copied() else {
            self.start_fall();
            return None;
        };
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        let (lo, hi) = (lo.min(x), hi.max(x));
        let want = x + dir * step;
        let new_x = want.clamp(lo, hi);
        self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
        self.body.x = new_x;
        Some(new_x != want)
    }

    fn end_juggle(&mut self) {
        self.juggle.carry = None;
        self.juggle.calm_until = self.clock + behavior::range(&mut self.juggle.rng, juggle::CALM_SECS);
        self.state = self.rest_beat();
    }

    /// 들어 올린 뒤 할 일: 거대한 것은 던지기만, 아니면 배달 장난 (갈 수 있는 끝이 있을 때) / 들고 돌아다니기 / 던지기
    pub(super) fn pick_plan(&mut self, giant: bool, world: &World) -> Plan {
        if giant {
            return Plan::Throw;
        }
        let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
        let reach = |side: Side, dir: f64| piece.is_some_and(|p| visit::screen_edge(world, &p, dir) == Some(side));
        let mut sides = Vec::new();
        if self.juggle.prank.0 && reach(Side::Left, -1.0) {
            sides.push(Side::Left);
        }
        if self.juggle.prank.1 && reach(Side::Right, 1.0) {
            sides.push(Side::Right);
        }
        if !sides.is_empty() && self.juggle.rng.f64() < juggle::PRANK_CHANCE {
            return Plan::Prank(sides[self.juggle.rng.usize(..sides.len())]);
        }
        if self.juggle.rng.f64() < juggle::WANDER_CHANCE {
            Plan::Wander
        } else {
            Plan::Throw
        }
    }

    fn start_lift(&mut self, id: u64, giant: bool, world: &World) {
        self.juggle.carry = Some(id);
        self.juggle.giant = giant;
        self.juggle.plan = self.pick_plan(giant, world);
        if !giant {
            self.speech = Some(Speech::Hup);
        }
        self.state = State::Juggle { phase: JugglePhase::Lift, t: 0.0 };
    }

    fn start_throw(&mut self) {
        // 노릴 동물이 있으면 그쪽을 본다
        let from = self.carry_point().unwrap_or((self.body.x, self.body.y - self.cfg.size));
        if !self.juggle.giant {
            if let Some(t) = juggle::nearest_target(from, self.cfg.size, &self.juggle.targets) {
                self.facing = if t.x >= from.0 { 1.0 } else { -1.0 };
            }
        }
        self.state = State::Juggle { phase: JugglePhase::Throw { released: false }, t: 0.0 };
    }

    fn start_present(&mut self) {
        self.juggle.present_pending = false;
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Tada);
        self.state = State::Juggle { phase: JugglePhase::Present { released: false }, t: 0.0 };
    }

    fn release(&mut self, vx: f64, vy: f64) {
        if let Some(id) = self.juggle.carry.take() {
            self.juggle.event = Some(JuggleEvent::Release { id, vx, vy });
        }
    }

    /// 들고 던지기 한 틱
    pub(super) fn update_juggle(&mut self, dt: f64, world: &World, phase: JugglePhase, t: f64) {
        let t = t + dt;
        let holding = matches!(phase, JugglePhase::Lift | JugglePhase::Hold { .. } | JugglePhase::Wander { .. } | JugglePhase::CarryOff { .. })
            || matches!(phase, JugglePhase::Throw { released: false } | JugglePhase::Present { released: false });
        if holding && self.juggle.carry.is_none() {
            // 손에서 놓쳤다 (눌렸다 놓였거나 앱이 물건을 못 찾았다)
            self.end_juggle();
            return;
        }
        let next = |phase| State::Juggle { phase, t };
        match phase {
            JugglePhase::Approach { id, x, obj_x, giant } => {
                if t > juggle::APPROACH_SECS {
                    self.end_juggle();
                    return;
                }
                let dx = x - self.body.x;
                let step = self.cfg.walk_speed * Move::Run.speed_mult() * dt;
                if dx.abs() > step {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, self.facing, step).is_none() {
                        return;
                    }
                    if (x - self.body.x).abs() > 1.0 {
                        self.state = next(phase);
                        return;
                    }
                } else if self.juggle_walk(world, dx.signum(), dx.abs()).is_none() {
                    return;
                }
                // 닿았다: 물건 쪽을 본다
                if (obj_x - self.body.x).abs() > 1.0 {
                    self.facing = (obj_x - self.body.x).signum();
                }
                if giant {
                    let lifts = self.juggle.rng.f64() < juggle::GIANT_LIFT_CHANCE;
                    self.speech = Some(Speech::Strain);
                    self.state = State::Juggle { phase: JugglePhase::Strain { id, lifts }, t: 0.0 };
                } else {
                    self.start_lift(id, false, world);
                }
            }
            JugglePhase::Strain { id, lifts } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < juggle::STRAIN_SECS {
                    self.state = next(phase);
                } else if lifts {
                    self.start_lift(id, true, world);
                } else {
                    // 너무 무겁다: 털썩
                    self.speech = Some(Speech::Phew);
                    self.state = State::Juggle { phase: JugglePhase::Slump, t: 0.0 };
                }
            }
            JugglePhase::Slump => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < juggle::SLUMP_SECS {
                    self.state = next(phase);
                    return;
                }
                // 일어나 반대쪽으로 걸어간다
                self.juggle.calm_until = self.clock + behavior::range(&mut self.juggle.rng, juggle::CALM_SECS);
                self.facing = -self.facing;
                self.state = State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.rng, Move::Walk.secs()) };
            }
            JugglePhase::Lift => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                self.state = if t < juggle::LIFT_SECS {
                    next(phase)
                } else {
                    State::Juggle { phase: JugglePhase::Hold { secs: behavior::range(&mut self.juggle.rng, juggle::HOLD_SECS) }, t: 0.0 }
                };
            }
            JugglePhase::Hold { secs } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if t < secs {
                    self.state = next(phase);
                    return;
                }
                match self.juggle.plan {
                    Plan::Throw => self.start_throw(),
                    Plan::Wander => {
                        let secs = behavior::range(&mut self.juggle.rng, juggle::WANDER_SECS);
                        self.state = State::Juggle { phase: JugglePhase::Wander { secs }, t: 0.0 };
                    }
                    Plan::Prank(side) => self.state = State::Juggle { phase: JugglePhase::CarryOff { side }, t: 0.0 },
                }
            }
            JugglePhase::Wander { secs } => {
                let step = self.walk_speed() * juggle::WANDER_PACE * dt;
                let Some(blocked) = self.juggle_walk(world, self.facing, step) else { return };
                if blocked {
                    // 조각 끝: 돌아선다 (뛰어내리지 않는다 — 들고 있으니까)
                    self.facing = -self.facing;
                }
                if t < secs {
                    self.state = next(phase);
                } else {
                    self.start_throw();
                }
            }
            JugglePhase::CarryOff { side } => {
                let dir = visit::dir(side);
                self.facing = dir;
                let Some(blocked) = self.juggle_walk(world, dir, self.walk_speed() * dt) else { return };
                if !blocked {
                    self.state = next(phase);
                    return;
                }
                let open = match side {
                    Side::Left => self.exit_edges.left,
                    _ => self.exit_edges.right,
                };
                let piece = self.standing.and_then(|st| world.piece_at(st.owner, self.body.x)).copied();
                if open && piece.is_some_and(|p| visit::screen_edge(world, &p, dir) == Some(side)) {
                    // 들고 나간다 — 못 들어가 돌아오면 집에 떨군다
                    self.juggle.present_pending = true;
                    self.start_exit(side);
                } else {
                    self.start_throw();
                }
            }
            JugglePhase::Throw { released } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if !released && t >= juggle::THROW_RELEASE {
                    let from = self.carry_point().unwrap_or((self.body.x, self.body.y - self.cfg.size));
                    let (vx, vy, facing) = juggle::throw_velocity(&mut self.juggle.rng, from, self.facing, self.cfg.size, self.cfg.dpi, self.juggle.giant, &self.juggle.targets);
                    self.facing = facing;
                    if !self.juggle.giant {
                        self.speech = Some(Speech::Yeet);
                    }
                    self.release(vx, vy);
                    self.state = State::Juggle { phase: JugglePhase::Throw { released: true }, t };
                } else if t >= juggle::THROW_SECS {
                    self.end_juggle();
                } else {
                    self.state = next(phase);
                }
            }
            JugglePhase::Present { released } => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                if !released && t >= juggle::PRESENT_RELEASE {
                    let (vx, vy) = juggle::flick_velocity(&mut self.juggle.rng, self.facing, self.cfg.dpi);
                    self.release(vx, vy);
                    self.state = State::Juggle { phase: JugglePhase::Present { released: true }, t };
                } else if t >= juggle::PRESENT_SECS {
                    self.end_juggle();
                } else {
                    self.state = next(phase);
                }
            }
        }
    }

    /// 들고 던지기 그림
    pub(super) fn juggle_anim(&self, phase: JugglePhase) -> Anim {
        match phase {
            JugglePhase::Approach { .. } => Anim::Run,
            JugglePhase::Strain { .. } => Anim::Strain,
            JugglePhase::Lift => Anim::Lift,
            JugglePhase::Hold { .. } => Anim::Hold,
            JugglePhase::Wander { .. } | JugglePhase::CarryOff { .. } => Anim::Carry,
            JugglePhase::Throw { .. } => Anim::Throw,
            JugglePhase::Present { .. } => Anim::Present,
            JugglePhase::Slump => Anim::Slump,
        }
    }

    /// 물건을 든 채 걸어 나가거나 들어오는 동안·도착해 서기 전엔 머리 위에 인 그림
    pub(super) fn carrying_anim(&self, anim: Anim) -> Anim {
        if self.carried_object().is_none() || matches!(self.state, State::Juggle { .. }) {
            return anim;
        }
        match anim {
            Anim::Walk | Anim::Run => Anim::Carry,
            Anim::Idle | Anim::Look | Anim::LookUp => Anim::Hold,
            a => a,
        }
    }
}
