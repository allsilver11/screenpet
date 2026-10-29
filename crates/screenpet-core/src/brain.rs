use std::collections::VecDeque;

use serde::Serialize;

use crate::behavior::{self, ClickReaction, Move, Rest, Special, Trick};
use crate::physics::{clamp_speed, step_air, Body};
use crate::surfaces::{EndKind, Owner};
use crate::world::World;

const TURN_PAUSE: f64 = 0.3;
const LAND_SECS: f64 = 0.25;
const HOP_VX: f64 = 180.0;
const HOP_VY: f64 = -650.0;
const HAPPY_VY: f64 = -500.0;
const HEART_SECS: f64 = 1.0;
const CLICK_SLOP: f64 = 5.0;
const VELOCITY_WINDOW: f64 = 0.1;
const TICKLE_WINDOW: f64 = 0.6;
const TICKLE_DIST: f64 = 250.0;
const TICKLE_HOLD: f64 = 1.5;
const TICKLE_WAKE_SECS: f64 = 5.0;
const CHATTER_SECS: (f64, f64) = (20.0, 40.0);
const FIRST_CHATTER_SECS: f64 = 25.0;
const CLICK_SPEECH_CHANCE: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Anim {
    Walk,
    Run,
    Roll,
    Idle,
    Look,
    Sleep,
    Hop,
    Fall,
    Dragged,
    Land,
    SitFront,
    Wave,
    Tilt,
    Belly,
    Stretch,
    Heart,
    Tickle,
    Annoyed,
    Arch,
    Wag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Speech {
    Chatter,
    Tickle,
    Grumpy,
    Hiss,
}

impl Anim {
    /// 앞모습 동작 — 좌우 반전하지 않는다.
    pub fn is_front(self) -> bool {
        matches!(self, Anim::SitFront | Anim::Wave | Anim::Tilt | Anim::Belly | Anim::Stretch | Anim::Heart | Anim::Tickle | Anim::Wag)
    }
}

impl From<Move> for Anim {
    fn from(m: Move) -> Self {
        match m {
            Move::Walk => Anim::Walk,
            Move::Run => Anim::Run,
            Move::Roll => Anim::Roll,
        }
    }
}

impl From<Special> for Anim {
    fn from(s: Special) -> Self {
        match s {
            Special::Arch => Anim::Arch,
            Special::Wag => Anim::Wag,
        }
    }
}

impl From<Trick> for Anim {
    fn from(t: Trick) -> Self {
        match t {
            Trick::SitFront => Anim::SitFront,
            Trick::Wave => Anim::Wave,
            Trick::Tilt => Anim::Tilt,
            Trick::Belly => Anim::Belly,
            Trick::Stretch => Anim::Stretch,
            Trick::Heart => Anim::Heart,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Move { kind: Move, remaining: f64 },
    Idle { remaining: f64, look: bool },
    Sleep { remaining: f64 },
    Turn { remaining: f64 },
    Land { remaining: f64 },
    Trick { kind: Trick, remaining: f64 },
    /// 간질이는 중 (앞모습 꼬물꼬물)
    Tickled { remaining: f64 },
    /// 자다가 간질여짐 — tickled_for가 TICKLE_WAKE_SECS를 넘으면 깸
    Annoyed { remaining: f64, tickled_for: f64 },
    /// Hop / Fall(낙하·던지기) / Happy 점프
    Air { anim: Anim },
    /// 마우스를 눌렀지만 아직 드래그 거리 미만
    Pressed { start_cx: f64, start_cy: f64, grounded: bool, was_sleeping: bool },
    Dragged,
    /// 종별 특별 동작
    Special { kind: Special, remaining: f64 },
}

/// 서 있는 디딤판과 그 기준점(anchor)으로부터의 x 오프셋.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Standing {
    pub owner: Owner,
    pub rel_x: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct PetConfig {
    /// 스프라이트 한 변 (물리 px)
    pub size: f64,
    /// 몸 폭의 절반 — 디딤판 끝 판정에 사용
    pub half_w: f64,
    /// px/s
    pub walk_speed: f64,
    /// 이 동물의 특별 동작 (없으면 None)
    pub special: Option<Special>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RenderState {
    pub anim: Anim,
    pub flip: bool,
    pub heart: bool,
}

pub struct Pet {
    pub cfg: PetConfig,
    pub body: Body,
    pub state: State,
    pub standing: Option<Standing>,
    /// +1 오른쪽, -1 왼쪽
    pub facing: f64,
    heart_timer: f64,
    grab_dx: f64,
    grab_dy: f64,
    /// (clock, cursor x, cursor y)
    history: VecDeque<(f64, f64, f64)>,
    clock: f64,
    hover_hist: VecDeque<(f64, f64, f64)>,
    tickling: bool,
    chatter_timer: f64,
    speech: Option<Speech>,
    rng: fastrand::Rng,
}

impl Pet {
    pub fn new(cfg: PetConfig, x: f64, y: f64, seed: u64) -> Self {
        Self {
            cfg,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            state: State::Air { anim: Anim::Fall },
            standing: None,
            facing: 1.0,
            heart_timer: 0.0,
            grab_dx: 0.0,
            grab_dy: 0.0,
            history: VecDeque::new(),
            clock: 0.0,
            hover_hist: VecDeque::new(),
            tickling: false,
            chatter_timer: FIRST_CHATTER_SECS,
            speech: None,
            rng: fastrand::Rng::with_seed(seed),
        }
    }

    pub fn update(&mut self, dt: f64, world: &World) {
        self.clock += dt;
        self.heart_timer = (self.heart_timer - dt).max(0.0);
        match self.state {
            State::Dragged => {}
            State::Pressed { grounded, .. } => {
                if grounded {
                    self.follow_anchor(world);
                }
            }
            State::Air { .. } => self.update_air(dt, world),
            _ => {
                self.react_to_tickle();
                self.update_chatter(dt);
                self.update_ground(dt, world);
            }
        }
    }

    pub fn render(&self) -> RenderState {
        let anim = match self.state {
            State::Move { kind, .. } => kind.into(),
            State::Trick { kind, .. } => kind.into(),
            State::Tickled { .. } => Anim::Tickle,
            State::Annoyed { .. } => Anim::Annoyed,
            State::Special { kind, .. } => kind.into(),
            State::Idle { look: true, .. } => Anim::Look,
            State::Idle { .. } | State::Turn { .. } => Anim::Idle,
            State::Sleep { .. } => Anim::Sleep,
            State::Land { .. } => Anim::Land,
            State::Air { anim } => anim,
            State::Pressed { grounded: true, .. } => Anim::Idle,
            State::Pressed { .. } | State::Dragged => Anim::Dragged,
        };
        RenderState { anim, flip: self.facing < 0.0 && !anim.is_front(), heart: self.heart_timer > 0.0 }
    }

    pub fn window_pos(&self) -> (f64, f64) {
        (self.body.x - self.cfg.size / 2.0, self.body.y - self.cfg.size)
    }

    pub fn is_held(&self) -> bool {
        matches!(self.state, State::Pressed { .. } | State::Dragged)
    }

    /// 매 틱 update 전에 호출 (버튼 안 누른 상태). inside = 커서가 스프라이트 사각형 안
    pub fn hover(&mut self, cx: f64, cy: f64, inside: bool) {
        if !inside || self.is_held() || matches!(self.state, State::Air { .. }) {
            self.hover_hist.clear();
            self.tickling = false;
            return;
        }
        self.hover_hist.push_back((self.clock, cx, cy));
        while self.hover_hist.front().is_some_and(|&(t, _, _)| self.clock - t > TICKLE_WINDOW) {
            self.hover_hist.pop_front();
        }
        let path: f64 = self
            .hover_hist
            .iter()
            .zip(self.hover_hist.iter().skip(1))
            .map(|(a, b)| (b.1 - a.1).hypot(b.2 - a.2))
            .sum();
        self.tickling = path >= TICKLE_DIST;
    }

    /// 이번 틱에 띄울 말풍선 (한 번 꺼내면 None)
    pub fn take_speech(&mut self) -> Option<Speech> {
        self.speech.take()
    }

    pub fn pointer_down(&mut self, cx: f64, cy: f64) {
        if self.is_held() {
            return;
        }
        let grounded = self.standing.is_some() && !matches!(self.state, State::Air { .. });
        let was_sleeping = matches!(self.state, State::Sleep { .. } | State::Annoyed { .. });
        self.grab_dx = self.body.x - cx;
        self.grab_dy = self.body.y - cy;
        self.history.clear();
        self.history.push_back((self.clock, cx, cy));
        if !grounded {
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = 0.0;
        }
        self.state = State::Pressed { start_cx: cx, start_cy: cy, grounded, was_sleeping };
    }

    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { start_cx, start_cy, .. } => {
                self.record(cx, cy);
                if (cx - start_cx).hypot(cy - start_cy) >= CLICK_SLOP {
                    self.standing = None;
                    self.state = State::Dragged;
                    self.drag_to(cx, cy);
                }
            }
            State::Dragged => {
                self.record(cx, cy);
                self.drag_to(cx, cy);
            }
            _ => {}
        }
    }

    pub fn pointer_up(&mut self, cx: f64, cy: f64) {
        match self.state {
            State::Pressed { grounded: true, was_sleeping: true, .. } => {
                self.state = self.idle_state();
            }
            State::Pressed { grounded: true, .. } => {
                match behavior::pick_click(&mut self.rng) {
                    ClickReaction::Hop => {
                        self.heart_timer = HEART_SECS;
                        self.standing = None;
                        self.body.vx = 0.0;
                        self.body.vy = HAPPY_VY;
                        self.state = State::Air { anim: Anim::Hop };
                    }
                    ClickReaction::Trick(kind) => {
                        self.state = State::Trick { kind, remaining: behavior::CLICK_TRICK_SECS };
                    }
                }
                if self.rng.f64() < CLICK_SPEECH_CHANCE {
                    self.speech = Some(Speech::Chatter);
                }
            }
            State::Pressed { .. } => self.start_fall(),
            State::Dragged => {
                self.record(cx, cy);
                let (vx, vy) = self.release_velocity();
                let (vx, vy) = clamp_speed(vx, vy);
                if vx.abs() > 50.0 {
                    self.facing = vx.signum();
                }
                self.standing = None;
                self.body.vx = vx;
                self.body.vy = vy;
                self.state = State::Air { anim: Anim::Fall };
            }
            _ => {}
        }
    }

    fn drag_to(&mut self, cx: f64, cy: f64) {
        self.body.x = cx + self.grab_dx;
        self.body.y = cy + self.grab_dy;
    }

    fn record(&mut self, cx: f64, cy: f64) {
        self.history.push_back((self.clock, cx, cy));
        while self.history.len() > 2 && self.clock - self.history[0].0 > VELOCITY_WINDOW {
            self.history.pop_front();
        }
    }

    fn release_velocity(&self) -> (f64, f64) {
        let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (self.history.front(), self.history.back()) else {
            return (0.0, 0.0);
        };
        let dt = t1 - t0;
        if dt < 1e-6 {
            return (0.0, 0.0);
        }
        ((x1 - x0) / dt, (y1 - y0) / dt)
    }

    /// 서 있는 디딤판 기준점을 따라 위치 갱신. 기준점이 사라졌으면 false.
    fn follow_anchor(&mut self, world: &World) -> bool {
        let Some(st) = self.standing else { return false };
        let Some((ax, ay)) = world.anchor(st.owner) else { return false };
        self.body.x = ax + st.rel_x;
        self.body.y = ay;
        true
    }

    fn react_to_tickle(&mut self) {
        if !self.tickling {
            return;
        }
        self.state = match self.state {
            State::Sleep { .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for: 0.0 },
            State::Annoyed { tickled_for, .. } => State::Annoyed { remaining: TICKLE_HOLD, tickled_for },
            State::Tickled { .. } => State::Tickled { remaining: TICKLE_HOLD },
            State::Special { .. } => return,
            _ => {
                if let Some(kind) = self.cfg.special {
                    if self.rng.f64() < kind.tickle_chance() {
                        self.start_special(kind);
                        return;
                    }
                }
                self.speech = Some(Speech::Tickle);
                State::Tickled { remaining: TICKLE_HOLD }
            }
        };
    }

    pub(crate) fn start_special(&mut self, kind: Special) {
        self.speech = Some(match kind {
            Special::Arch => Speech::Hiss,
            Special::Wag => Speech::Chatter,
        });
        self.state = State::Special { kind, remaining: behavior::range(&mut self.rng, kind.secs()) };
    }

    fn update_chatter(&mut self, dt: f64) {
        if matches!(self.state, State::Sleep { .. } | State::Annoyed { .. } | State::Tickled { .. }) {
            return;
        }
        self.chatter_timer -= dt;
        if self.chatter_timer <= 0.0 {
            if self.speech.is_some() {
                // 말풍선 자리가 비면 다음 틱에 다시 시도
                self.chatter_timer = 0.0;
                return;
            }
            self.speech = Some(Speech::Chatter);
            self.chatter_timer = behavior::range(&mut self.rng, CHATTER_SECS);
        }
    }

    fn update_ground(&mut self, dt: f64, world: &World) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.unwrap();
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        if piece.width() < 2.0 * self.cfg.half_w && !matches!(self.state, State::Land { .. }) {
            self.start_hop(self.facing);
            return;
        }

        match self.state {
            State::Move { kind, remaining } => {
                let speed = self.cfg.walk_speed * kind.speed_mult();
                let new_x = self.body.x + self.facing * speed * dt;
                let (hit, end) = if self.facing > 0.0 {
                    (new_x + self.cfg.half_w > piece.x_end, piece.end_kind)
                } else {
                    (new_x - self.cfg.half_w < piece.x_start, piece.start_kind)
                };
                if hit {
                    match (end, kind) {
                        (EndKind::Edge, Move::Walk) if self.rng.f64() < behavior::WALK_EDGE_HOP_CHANCE => {
                            self.start_hop(self.facing)
                        }
                        (EndKind::Edge, Move::Run) => {
                            self.start_hop(self.facing);
                            self.body.vx *= behavior::RUN_LEAP_VX_MULT;
                        }
                        (EndKind::Edge, Move::Roll) => {
                            self.standing = None;
                            self.body.vx = self.facing * speed;
                            self.body.vy = 0.0;
                            self.state = State::Air { anim: Anim::Roll };
                        }
                        _ => self.state = State::Turn { remaining: TURN_PAUSE },
                    }
                    return;
                }
                self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - self.body.x) });
                self.body.x = new_x;
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Move { kind, remaining }
                } else if self.rng.f64() < behavior::MOVE_END_TURN_CHANCE {
                    State::Turn { remaining: TURN_PAUSE }
                } else {
                    self.rest_state()
                };
            }
            State::Idle { remaining, look } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Idle { remaining, look }
                } else if self.rng.f64() < behavior::SLEEP_CHANCE {
                    State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
                } else {
                    self.move_state(true)
                };
            }
            State::Trick { kind, remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Trick { kind, remaining } } else { self.move_state(true) };
            }
            State::Tickled { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Tickled { remaining } } else { self.move_state(true) };
            }
            State::Annoyed { remaining, tickled_for } => {
                let tickled_for = if self.tickling { tickled_for + dt } else { tickled_for };
                if tickled_for >= TICKLE_WAKE_SECS {
                    self.speech = Some(Speech::Grumpy);
                    self.state = self.idle_state();
                } else {
                    let remaining = remaining - dt;
                    self.state = if remaining > 0.0 {
                        State::Annoyed { remaining, tickled_for }
                    } else {
                        State::Sleep { remaining: behavior::range(&mut self.rng, behavior::SLEEP_SECS) }
                    };
                }
            }
            State::Special { kind, remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Special { kind, remaining } } else { self.move_state(true) };
            }
            State::Sleep { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Sleep { remaining } } else { self.idle_state() };
            }
            State::Turn { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 {
                    State::Turn { remaining }
                } else {
                    self.facing = -self.facing;
                    self.move_state(false)
                };
            }
            State::Land { remaining } => {
                let remaining = remaining - dt;
                self.state = if remaining > 0.0 { State::Land { remaining } } else { self.rest_state() };
            }
            State::Air { .. } | State::Pressed { .. } | State::Dragged => {}
        }
    }

    fn update_air(&mut self, dt: f64, world: &World) {
        let prev_y = self.body.y;
        step_air(&mut self.body, dt, &world.bounds, self.cfg.half_w, self.cfg.size);
        let landing = world.landing(self.body.x, prev_y, self.body.y).copied().or_else(|| {
            // 이미 작업 영역 바닥보다 아래로 내려간 경우(작업표시줄 위에 놓음)엔 Ground를 바닥으로 취급
            if self.body.vy >= 0.0 { world.ground_below(self.body.x, self.body.y).copied() } else { None }
        });
        if let Some(s) = landing {
            let (ax, _) = world.anchor(s.owner).expect("surface owner has anchor");
            self.body = Body { x: self.body.x, y: s.y, vx: 0.0, vy: 0.0 };
            self.standing = Some(Standing { owner: s.owner, rel_x: self.body.x - ax });
            self.state = State::Land { remaining: LAND_SECS };
        } else if self.body.y > world.bounds.bottom + self.cfg.size * 4.0 {
            // 화면 밖으로 사라짐 → 가까운 위치 상단에서 다시 떨어뜨림
            let b = world.bounds;
            self.body = Body {
                x: self.body.x.clamp(b.left + self.cfg.half_w, b.right - self.cfg.half_w),
                y: b.top + self.cfg.size,
                vx: 0.0,
                vy: 0.0,
            };
        }
    }

    pub(crate) fn start_hop(&mut self, dir: f64) {
        self.standing = None;
        self.body.vx = dir * HOP_VX;
        self.body.vy = HOP_VY;
        self.state = State::Air { anim: Anim::Hop };
    }

    fn start_fall(&mut self) {
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Air { anim: Anim::Fall };
    }

    fn idle_state(&mut self) -> State {
        State::Idle { remaining: behavior::range(&mut self.rng, behavior::IDLE_SECS), look: self.rng.bool() }
    }

    /// 이동 시작. may_flip이면 절반 확률로 방향을 바꾼다 (Turn 직후엔 이미 바꿨으므로 false).
    fn move_state(&mut self, may_flip: bool) -> State {
        if may_flip && self.rng.bool() {
            self.facing = -self.facing;
        }
        let kind = behavior::pick_move(&mut self.rng);
        State::Move { kind, remaining: behavior::range(&mut self.rng, kind.secs()) }
    }

    /// 이동이 끝난 뒤: 옆모습 쉬기 또는 앞모습 애교
    fn rest_state(&mut self) -> State {
        if let Some(kind) = self.cfg.special {
            if self.rng.f64() < kind.rest_chance() {
                self.start_special(kind);
                return self.state;
            }
        }
        match behavior::pick_rest(&mut self.rng) {
            Rest::Idle => self.idle_state(),
            Rest::Trick(kind) => State::Trick { kind, remaining: behavior::range(&mut self.rng, behavior::TRICK_SECS) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::{Move, Trick};
    use crate::geom::{Rect, WindowInfo};

    const DT: f64 = 1.0 / 60.0;

    pub(super) fn cfg() -> PetConfig {
        PetConfig { size: 128.0, half_w: 38.0, walk_speed: 60.0, special: None }
    }

    pub(super) fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    /// windows는 앞→뒤 순서
    pub(super) fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    pub(super) fn w1() -> WindowInfo {
        win(1, 500.0, 400.0, 1100.0, 900.0)
    }

    pub(super) fn place(p: &mut Pet, w: &World, owner: Owner, x: f64, state: State) {
        let (ax, ay) = w.anchor(owner).unwrap();
        p.standing = Some(Standing { owner, rel_x: x - ax });
        p.body = Body { x, y: ay, vx: 0.0, vy: 0.0 };
        p.state = state;
    }

    pub(super) fn run(p: &mut Pet, w: &World, ticks: usize) {
        for _ in 0..ticks {
            p.update(DT, w);
        }
    }

    #[test]
    fn falls_and_lands_on_window_top() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
        assert_eq!(p.body.y, 400.0);
    }

    #[test]
    fn falls_to_ground_when_nothing_below() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 200.0, 200.0, 1);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!(p.body.y, 1040.0);
    }

    #[test]
    fn released_below_ground_line_lands_on_ground() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 800.0, 1090.0, 1);
        p.body.vy = 0.0;
        p.state = State::Air { anim: Anim::Fall };
        p.update(DT, &w);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
        assert_eq!(p.body.y, 1040.0);
        assert!(matches!(p.state, State::Land { .. }));
    }

    #[test]
    fn landing_shows_land_then_idle() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 390.0, 1);
        run(&mut p, &w, 10);
        assert!(matches!(p.state, State::Land { .. }));
        run(&mut p, &w, 30);
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn rides_window_when_it_moves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let moved = world(vec![win(1, 600.0, 450.0, 1200.0, 950.0)]);
        p.update(DT, &moved);
        assert_eq!((p.body.x, p.body.y), (900.0, 450.0));
    }

    #[test]
    fn falls_when_window_disappears() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &world(vec![]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!(p.standing.is_none());
    }

    #[test]
    fn falls_when_covered_by_front_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        let covered = world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]);
        p.update(DT, &covered);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn blocked_end_turns_around() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 840.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        let mut turned = false;
        for _ in 0..60 {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Air { .. }));
            if matches!(p.state, State::Turn { .. }) {
                turned = true;
                break;
            }
        }
        assert!(turned);
        run(&mut p, &w, 20);
        assert_eq!(p.facing, -1.0);
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn wall_end_always_turns() {
        let w = world(vec![]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Ground(0), 1920.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "seed {seed}");
        }
    }

    #[test]
    fn walking_to_a_window_edge_hops_about_80_percent() {
        let w = world(vec![w1()]);
        let (mut hops, mut turns) = (0, 0);
        for seed in 0..400 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Walk, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            match p.state {
                State::Air { anim: Anim::Hop } => hops += 1,
                State::Turn { .. } => turns += 1,
                s => panic!("unexpected {s:?}"),
            }
        }
        assert!((280..=360).contains(&hops), "hops={hops}");
        assert_eq!(hops + turns, 400);
    }

    #[test]
    fn hop_off_edge_leaves_the_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1061.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.start_hop(1.0);
        run(&mut p, &w, 120);
        assert_eq!(p.standing.unwrap().owner, Owner::Ground(0));
    }

    #[test]
    fn narrow_piece_makes_pet_hop_down() {
        let w = world(vec![win(2, 550.0, 300.0, 1300.0, 600.0), w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 525.0, State::Idle { remaining: 100.0, look: false });
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Hop });
    }

    #[test]
    fn expired_walk_becomes_idle_or_turn() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Walk, remaining: 0.001 });
        p.update(DT, &w);
        assert!(matches!(p.state, State::Idle { .. } | State::Turn { .. } | State::Trick { .. }));
    }

    #[test]
    fn expired_idle_becomes_sleep_or_walk() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 0.001, look: true });
        p.update(DT, &w);
        assert!(matches!(p.state, State::Sleep { .. } | State::Move { .. }));
    }

    #[test]
    fn render_maps_state_and_facing() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.state = State::Idle { remaining: 1.0, look: true };
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Look, flip: true, heart: false });
        p.state = State::Turn { remaining: 0.1 };
        assert_eq!(p.render().anim, Anim::Idle);
    }

    #[test]
    fn window_pos_is_top_left_of_sprite() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.body = Body { x: 800.0, y: 400.0, vx: 0.0, vy: 0.0 };
        assert_eq!(p.window_pos(), (736.0, 272.0));
    }

    #[test]
    fn running_moves_two_and_a_half_times_faster() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Move { kind: Move::Run, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert!((p.body.x - (900.0 + 60.0 * 2.5 * DT)).abs() < 1e-9);
    }

    #[test]
    fn running_off_a_window_edge_always_leaps_far() {
        let w = world(vec![w1()]);
        for seed in 0..50 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Run, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert_eq!(p.state, State::Air { anim: Anim::Hop }, "seed {seed}");
            assert!(p.body.vx > 300.0);
        }
    }

    #[test]
    fn rolling_off_a_window_edge_keeps_rolling_speed() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 1100.0 - 38.0 - 0.5, State::Move { kind: Move::Roll, remaining: 100.0 });
        p.facing = 1.0;
        p.update(DT, &w);
        assert_eq!(p.state, State::Air { anim: Anim::Roll });
        assert_eq!((p.body.vx, p.body.vy), (120.0, 0.0));
        assert!(p.standing.is_none());
    }

    #[test]
    fn blocked_end_turns_for_every_move_kind() {
        let w = world(vec![win(2, 900.0, 300.0, 1300.0, 600.0), w1()]);
        for kind in [Move::Walk, Move::Run, Move::Roll] {
            let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 900.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 });
            p.facing = 1.0;
            p.update(DT, &w);
            assert!(matches!(p.state, State::Turn { .. }), "{kind:?}");
        }
    }

    #[test]
    fn tricks_face_front_and_end_by_moving() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Trick { kind: Trick::Wave, remaining: 0.5 });
        p.facing = -1.0;
        assert_eq!(p.render(), RenderState { anim: Anim::Wave, flip: false, heart: false });
        run(&mut p, &w, 40);
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn covered_during_trick_falls() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Trick { kind: Trick::Belly, remaining: 3.0 });
        p.update(DT, &world(vec![win(2, 700.0, 300.0, 900.0, 600.0), w1()]));
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
    }

    #[test]
    fn side_animations_still_flip() {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        p.facing = -1.0;
        p.state = State::Move { kind: Move::Roll, remaining: 1.0 };
        assert_eq!(p.render(), RenderState { anim: Anim::Roll, flip: true, heart: false });
    }

    #[test]
    fn front_animations_are_the_tricks_and_tickle() {
        let all = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall, Anim::Dragged,
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Annoyed, Anim::Arch, Anim::Wag];
        let front: Vec<Anim> = all.into_iter().filter(|a| a.is_front()).collect();
        assert_eq!(front, vec![Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Wag]);
        assert_eq!(serde_json::to_string(&Anim::SitFront).unwrap(), "\"sit_front\"");
    }

    #[test]
    fn every_anim_name_is_a_required_manifest_animation() {
        let all = [Anim::Walk, Anim::Run, Anim::Roll, Anim::Idle, Anim::Look, Anim::Sleep, Anim::Hop, Anim::Fall, Anim::Dragged,
            Anim::Land, Anim::SitFront, Anim::Wave, Anim::Tilt, Anim::Belly, Anim::Stretch, Anim::Heart, Anim::Tickle, Anim::Annoyed, Anim::Arch, Anim::Wag];
        let names: Vec<String> = all.iter().map(|a| serde_json::to_string(a).unwrap().trim_matches('"').to_string()).collect();
        let mut expected: Vec<&str> = crate::manifest::REQUIRED_ANIMS.to_vec();
        expected.extend(crate::manifest::OPTIONAL_ANIMS);
        assert_eq!(names, expected);
    }

    /// 10분 시뮬레이션 — "계속 자기만 한다" 회귀 방지 (spec §6)
    #[test]
    fn ten_minutes_are_mostly_moving_and_rarely_sleeping() {
        let w = world(vec![]);
        let (mut moving, mut sleeping, mut total) = (0usize, 0usize, 0usize);
        for seed in 0..20 {
            let mut p = Pet::new(PetConfig { size: 128.0, half_w: 38.4, walk_speed: 90.0, special: None }, 960.0, 128.0, seed);
            for _ in 0..(60 * 600) {
                p.update(DT, &w);
                match p.render().anim {
                    Anim::Walk | Anim::Run | Anim::Roll => moving += 1,
                    Anim::Sleep => sleeping += 1,
                    _ => {}
                }
                total += 1;
            }
        }
        let (m, s) = (moving as f64 / total as f64, sleeping as f64 / total as f64);
        assert!((0.65..=0.75).contains(&m), "moving {m:.3}");
        assert!(s <= 0.10, "sleeping {s:.3}");
    }
}

#[cfg(test)]
mod input_tests {
    use super::tests::{cfg, place, run, w1, world};
    use super::*;
    use crate::behavior::Trick;

    const DT: f64 = 1.0 / 60.0;

    fn idle() -> State {
        State::Idle { remaining: 100.0, look: false }
    }

    #[test]
    fn click_on_grounded_pet_reacts_with_hop_or_trick() {
        let w = world(vec![w1()]);
        let mut seen = std::collections::HashSet::new();
        for seed in 0..80 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            match p.state {
                State::Air { anim: Anim::Hop } => {
                    assert!(p.render().heart);
                    run(&mut p, &w, 120);
                    assert_eq!(p.standing.unwrap().owner, Owner::Window(1));
                    seen.insert("hop");
                }
                State::Trick { kind, remaining } => {
                    assert_eq!(remaining, 2.0);
                    assert!(p.standing.is_some());
                    seen.insert(match kind {
                        Trick::Heart => "heart",
                        Trick::Wave => "wave",
                        Trick::Tilt => "tilt",
                        k => panic!("unexpected {k:?}"),
                    });
                }
                s => panic!("unexpected {s:?}"),
            }
        }
        assert_eq!(seen.len(), 4);
    }

    #[test]
    fn tiny_move_still_counts_as_click() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(803.0, 351.0);
        p.pointer_up(803.0, 351.0);
        assert!(matches!(p.state, State::Air { anim: Anim::Hop } | State::Trick { .. }));
    }

    #[test]
    fn click_on_sleeping_pet_wakes_it() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        p.pointer_down(800.0, 350.0);
        p.pointer_up(800.0, 350.0);
        assert!(matches!(p.state, State::Idle { .. }));
        assert!(!p.render().heart);
    }

    #[test]
    fn pressed_grounded_pet_keeps_riding_window() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.update(DT, &world(vec![super::tests::win(1, 600.0, 400.0, 1200.0, 900.0)]));
        assert_eq!(p.body.x, 900.0);
    }

    #[test]
    fn drag_moves_pet_with_cursor_keeping_grab_offset() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(810.0, 350.0);
        p.pointer_move(910.0, 250.0);
        assert_eq!(p.state, State::Dragged);
        assert!(p.standing.is_none());
        assert_eq!((p.body.x, p.body.y), (900.0, 300.0));
        assert_eq!(p.render().anim, Anim::Dragged);
    }

    #[test]
    fn release_throws_with_recent_cursor_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 300.0);
        for i in 1..=6 {
            p.update(DT, &w);
            p.pointer_move(800.0 + 20.0 * i as f64, 300.0);
        }
        p.pointer_up(920.0, 300.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert!((p.body.vx - 1200.0).abs() < 1.0, "vx={}", p.body.vx);
        assert!(p.body.vy.abs() < 1.0);
        assert_eq!(p.facing, 1.0);
    }

    #[test]
    fn click_while_airborne_drops_with_zero_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 200.0, 1);
        p.body.vx = 500.0;
        p.body.vy = 300.0;
        p.pointer_down(800.0, 150.0);
        p.update(DT, &w);
        assert_eq!(p.body.y, 200.0, "held pet must not move");
        p.pointer_up(800.0, 150.0);
        assert_eq!(p.state, State::Air { anim: Anim::Fall });
        assert_eq!((p.body.vx, p.body.vy), (0.0, 0.0));
        assert!(!p.render().heart);
    }

    #[test]
    fn second_pointer_down_while_held_is_ignored() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 350.0);
        p.pointer_move(900.0, 350.0);
        p.pointer_down(950.0, 350.0);
        assert_eq!(p.state, State::Dragged);
    }

    #[test]
    fn release_after_pause_throws_with_zero_velocity() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.pointer_down(800.0, 300.0);
        for i in 1..=6 {
            p.update(DT, &w);
            p.pointer_move(800.0 + 50.0 * i as f64, 300.0);
        }
        for _ in 0..12 {
            p.update(DT, &w);
            p.pointer_move(1100.0, 300.0);
        }
        p.pointer_up(1100.0, 300.0);
        assert!(p.body.vx.abs() < 1.0, "vx={}", p.body.vx);
        assert!(p.body.vy.abs() < 1.0, "vy={}", p.body.vy);
    }
}

#[cfg(test)]
mod tickle_tests {
    use super::tests::{cfg, place, w1, world};
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// 커서를 40px씩 좌우로 흔들며 n틱 간질인다
    fn tickle(p: &mut Pet, w: &World, ticks: usize) {
        for i in 0..ticks {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, w);
        }
    }

    fn idle() -> State {
        State::Idle { remaining: 100.0, look: false }
    }

    #[test]
    fn fast_hover_tickles_and_speaks_once() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tickle(&mut p, &w, 30);
        assert!(matches!(p.state, State::Tickled { .. }));
        assert_eq!(p.render().anim, Anim::Tickle);
        assert_eq!(p.take_speech(), Some(Speech::Tickle));
        tickle(&mut p, &w, 10);
        assert_eq!(p.take_speech(), None, "speech only when tickling starts");
    }

    #[test]
    fn slow_hover_does_not_tickle() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        for i in 0..60 {
            p.hover(800.0 + i as f64 * 2.0, 350.0, true);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Idle { .. }));
    }

    #[test]
    fn tickle_stops_1_5s_after_cursor_leaves() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        tickle(&mut p, &w, 30);
        for _ in 0..80 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Tickled { .. }), "still giggling before 1.5s");
        for _ in 0..20 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Move { .. }));
    }

    #[test]
    fn tickling_a_sleeper_annoys_then_it_sleeps_again() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        tickle(&mut p, &w, 120);
        assert!(matches!(p.state, State::Annoyed { .. }));
        assert_eq!(p.render().anim, Anim::Annoyed);
        assert_eq!(p.take_speech(), None);
        for _ in 0..100 {
            p.hover(0.0, 0.0, false);
            p.update(DT, &w);
        }
        assert!(matches!(p.state, State::Sleep { .. }));
    }

    #[test]
    fn five_seconds_of_tickling_wakes_a_sleeper() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Sleep { remaining: 30.0 });
        tickle(&mut p, &w, 280);
        assert!(matches!(p.state, State::Annoyed { .. }), "not yet at ~4.5s");
        // 깨어난 틱에서 멈춘다 (계속 간질이면 깬 뒤 바로 간질 반응으로 넘어가는 게 정상)
        let mut woke = false;
        for i in 0..40 {
            p.hover(800.0 + if i % 2 == 0 { 40.0 } else { 0.0 }, 350.0, true);
            p.update(DT, &w);
            if matches!(p.state, State::Idle { .. }) {
                woke = true;
                break;
            }
        }
        assert!(woke);
        assert_eq!(p.take_speech(), Some(Speech::Grumpy));
    }

    #[test]
    fn airborne_pet_ignores_tickling() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 100.0, 1);
        tickle(&mut p, &w, 10);
        assert!(matches!(p.state, State::Air { .. }));
    }

    #[test]
    fn chatters_every_20_to_40_seconds_but_not_while_sleeping() {
        let w = world(vec![]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 1000.0, look: false });
        let mut said = 0;
        for _ in 0..(60 * 90) {
            p.update(DT, &w);
            if p.take_speech() == Some(Speech::Chatter) {
                said += 1;
            }
        }
        assert!((2..=5).contains(&said), "said {said} times in 90s");
        place(&mut p, &w, Owner::Ground(0), 900.0, State::Sleep { remaining: 1000.0 });
        for _ in 0..(60 * 90) {
            p.update(DT, &w);
            assert_eq!(p.take_speech(), None);
        }
    }

    #[test]
    fn about_half_of_clicks_speak() {
        let w = world(vec![w1()]);
        let mut spoke = 0;
        for seed in 0..200 {
            let mut p = Pet::new(cfg(), 0.0, 0.0, seed);
            place(&mut p, &w, Owner::Window(1), 800.0, idle());
            p.pointer_down(800.0, 350.0);
            p.pointer_up(800.0, 350.0);
            if p.take_speech() == Some(Speech::Chatter) {
                spoke += 1;
            }
        }
        assert!((70..=130).contains(&spoke), "spoke {spoke}/200");
    }

    #[test]
    fn chatter_does_not_overwrite_a_pending_speech() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.chatter_timer = 0.001;
        p.speech = Some(Speech::Tickle);
        p.update(DT, &w);
        assert_eq!(p.take_speech(), Some(Speech::Tickle));
        p.update(DT, &w);
        assert_eq!(p.take_speech(), Some(Speech::Chatter));
    }

    #[test]
    fn chatter_waits_while_tickled() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Tickled { remaining: 1.0 });
        p.chatter_timer = 0.001;
        p.update(DT, &w);
        assert_eq!(p.take_speech(), None);
    }

    #[test]
    fn shaking_the_cursor_during_a_hop_does_not_tickle_on_landing() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(cfg(), 800.0, 300.0, 1);
        p.state = State::Air { anim: Anim::Fall };
        let mut airborne_ticks = 0;
        for i in 0..40 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            let was_air = matches!(p.state, State::Air { .. });
            p.update(DT, &w);
            if was_air {
                airborne_ticks += 1;
            }
            if matches!(p.state, State::Land { .. }) {
                assert!(airborne_ticks >= 10, "fall too short to build up a tickle: {airborne_ticks}");
                p.hover(800.0, 350.0, true);
                p.update(DT, &w);
                assert!(!matches!(p.state, State::Tickled { .. }), "tickled right after landing");
                return;
            }
            assert!(!matches!(p.state, State::Tickled { .. }));
        }
        panic!("never landed");
    }
}

#[cfg(test)]
mod special_tests {
    use super::tests::{cfg, place, w1, world};
    use super::*;
    use crate::behavior::Special;

    const DT: f64 = 1.0 / 60.0;

    fn with(special: Option<Special>) -> PetConfig {
        PetConfig { special, ..cfg() }
    }

    /// 착지 직후 쉬기 진입을 n번 시켜 특별 동작이 나온 비율
    fn rest_share(special: Option<Special>) -> f64 {
        let w = world(vec![w1()]);
        let n = 3000;
        let hits = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(with(special), 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Land { remaining: 0.001 });
                p.update(DT, &w);
                matches!(p.state, State::Special { .. })
            })
            .count();
        hits as f64 / n as f64
    }

    /// 깨어 있는 동물을 간질이기 시작했을 때 특별 동작이 나온 비율
    fn tickle_share(special: Option<Special>) -> f64 {
        let w = world(vec![w1()]);
        let n = 1000;
        let hits = (0..n)
            .filter(|&seed| {
                let mut p = Pet::new(with(special), 0.0, 0.0, seed);
                place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
                for i in 0..10 {
                    p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
                    p.update(DT, &w);
                }
                matches!(p.state, State::Special { .. })
            })
            .count();
        hits as f64 / n as f64
    }

    #[test]
    fn rest_specials_follow_species_chances() {
        let arch = rest_share(Some(Special::Arch));
        let wag = rest_share(Some(Special::Wag));
        assert!((0.05..=0.11).contains(&arch), "arch {arch}");
        assert!((0.09..=0.15).contains(&wag), "wag {wag}");
        assert_eq!(rest_share(None), 0.0);
    }

    #[test]
    fn tickling_sometimes_triggers_the_special() {
        let arch = tickle_share(Some(Special::Arch));
        let wag = tickle_share(Some(Special::Wag));
        assert!((0.19..=0.31).contains(&arch), "arch {arch}");
        assert!((0.24..=0.36).contains(&wag), "wag {wag}");
        assert_eq!(tickle_share(None), 0.0);
    }

    #[test]
    fn special_speech_facing_and_ending() {
        let w = world(vec![w1()]);
        for (kind, speech, anim, front) in [(Special::Arch, Speech::Hiss, Anim::Arch, false), (Special::Wag, Speech::Chatter, Anim::Wag, true)] {
            let mut p = Pet::new(with(Some(kind)), 0.0, 0.0, 1);
            place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
            p.start_special(kind);
            assert_eq!(p.take_speech(), Some(speech));
            p.facing = -1.0;
            assert_eq!(p.render().anim, anim);
            assert_eq!(p.render().flip, !front);
            for _ in 0..(60 * 5) {
                p.update(DT, &w);
            }
            assert!(!matches!(p.state, State::Special { .. }), "{kind:?} should end within its duration");
        }
    }

    #[test]
    fn tickling_does_not_interrupt_a_special() {
        let w = world(vec![w1()]);
        let mut p = Pet::new(with(Some(Special::Arch)), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
        p.start_special(Special::Arch);
        let State::Special { remaining: start, .. } = p.state else { panic!() };
        for i in 0..30 {
            p.hover(800.0 + if i % 2 == 0 { 0.0 } else { 40.0 }, 350.0, true);
            p.update(DT, &w);
        }
        let State::Special { kind: Special::Arch, remaining } = p.state else { panic!("{:?}", p.state) };
        assert!((start - remaining - 30.0 * DT).abs() < 1e-6);
    }
}
