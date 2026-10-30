//! 조작 모드 (v0.3.6): 트레이 "🎮 조작 모드"를 켜면 내 동물이 저절로 돌아다니지 않고 키보드를 따른다.
//! - ← / → 걷기 (Shift = 달리기), 놓으면 멈춰 마지막 방향을 보고 쉰다.
//! - Space (걷는 동물은 ↑도) = 진짜 점프 (몸 JUMP_BODIES배 높이, 개구리는 더 높이) — 창 윗면에도 내려선다. 공중에서 ←/→로 조금 방향을 튼다.
//! - ↓ 누르는 동안 앞모습으로 앉기. 창 윗면에서 ↓ 두 번 빠르게 = 창에서 뛰어내리기.
//! - 벽을 타는 동물(고양이·곰·개구리)은 화면 끝 벽 쪽 + ↑ = 기어오르기, ↑로 계속 오르고 놓으면 매달려 있고, ↓ = 미끄러져 내려온다.
//! - 나는 동물(꿀벌·유령)은 방향키로 위아래 좌우 (중력 없음, Shift = 빠르게).
//! - 친구가 연결된 화면 끝으로 몰고 가면 평소 규칙대로 걸어(날아) 나간다.
//!
//! 먹이·직접 주기·누르기·하우스·"집에 와"·손님의 집 그리움·보내기는 조작보다 먼저다 — 그동안은 평소 동작을 그대로 쓰고,
//! 끝나면 다시 조작을 따른다. 키는 읽기만 한다 (다른 앱에도 그대로 간다 — 앱 쪽). 친구 화면에 가 있는 내 동물에겐
//! 앱이 키를 친구 링크로 보내고, 그 화면에서는 remote_keys로 받는다 (1초 소식이 없으면 키를 다 놓은 것으로 본다).
//! 키 없이 CONTROL_IDLE_SECS가 지나면 저절로 끝난다. brain.rs의 자식 모듈.

use super::*;

/// 키 없이 이만큼 지나면 조작 모드가 끝난다 (초)
pub const CONTROL_IDLE_SECS: f64 = 180.0;
/// 친구 링크로 온 키: 이만큼 새 소식이 없으면 다 놓은 것으로 본다 (초)
pub const REMOTE_STALE_SECS: f64 = 1.0;
/// 점프 높이 (몸 크기 배 — 크기·화면 배율이 이미 들어 있다)
pub const JUMP_BODIES: f64 = 2.2;
/// 개구리(폴짝 동물)는 이만큼 더 높이 뛴다
pub const FROG_JUMP_MULT: f64 = 1.5;
/// 공중에서 방향 틀기 가속 (걷기 속도 배 / 초)
const AIR_STEER: f64 = 4.0;
/// ↓ 두 번을 "빠르게"로 보는 간격 (초)
pub const DOUBLE_TAP_SECS: f64 = 0.35;
/// ↓로 앉은 자세는 놓은 뒤 이만큼만 더 앉아 있다 (초)
const SIT_HOLD_SECS: f64 = 0.15;
/// 화면 끝 벽에 "닿았다"로 보는 거리 (물리 px)
const WALL_REACH: f64 = 2.0;
/// 조작 중 쉬기 (끝나지 않게 매 틱 다시 채운다)
const CONTROL_IDLE: State = State::Idle { remaining: 1.0, look: false };

/// 조작 키 (방향키·Space·Shift) — 앱이 키보드에서 읽거나 친구 링크로 받는다
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlInput {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    /// Space
    pub jump: bool,
    /// Shift
    pub run: bool,
}

impl ControlInput {
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const UP: u8 = 4;
    const DOWN: u8 = 8;
    const JUMP: u8 = 16;
    const RUN: u8 = 32;

    /// 무엇이든 눌림
    pub fn any(&self) -> bool {
        self.bits() != 0
    }

    /// 친구 링크로 보낼 비트 (←1 →2 ↑4 ↓8 Space16 Shift32)
    pub fn bits(&self) -> u8 {
        let b = |on: bool, bit: u8| if on { bit } else { 0 };
        b(self.left, Self::LEFT) | b(self.right, Self::RIGHT) | b(self.up, Self::UP) | b(self.down, Self::DOWN) | b(self.jump, Self::JUMP) | b(self.run, Self::RUN)
    }

    /// 받은 비트 → 키 (모르는 비트는 무시)
    pub fn from_bits(bits: u8) -> Self {
        let on = |bit: u8| bits & bit != 0;
        ControlInput { left: on(Self::LEFT), right: on(Self::RIGHT), up: on(Self::UP), down: on(Self::DOWN), jump: on(Self::JUMP), run: on(Self::RUN) }
    }

    /// 좌우 방향 (+1 오른쪽, -1 왼쪽, 0 = 없음·둘 다)
    fn dir(&self) -> f64 {
        (self.right as i8 - self.left as i8) as f64
    }

    /// 위아래 방향 (+1 아래, -1 위)
    fn vertical(&self) -> f64 {
        (self.down as i8 - self.up as i8) as f64
    }
}

/// 조작 모드 상태
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Control {
    input: ControlInput,
    /// 지난 틱에 처리한 키 (막 누른 키 찾기)
    prev: ControlInput,
    /// 친구 링크로 키가 온다 (이 화면의 손님)
    remote: bool,
    /// 마지막으로 키 소식이 온 뒤 (remote)
    stale: f64,
    /// 마지막으로 키가 눌려 있던 뒤
    idle: f64,
    /// 마지막으로 ↓를 막 누른 시각 (clock)
    last_down: f64,
}

impl Control {
    fn new(remote: bool) -> Self {
        Control { input: ControlInput::default(), prev: ControlInput::default(), remote, stale: 0.0, idle: 0.0, last_down: f64::NEG_INFINITY }
    }

    fn feed(&mut self, input: ControlInput) {
        self.input = input;
        if input.any() {
            self.idle = 0.0;
        }
    }
}

impl Pet {
    /// 조작 모드 켜기 / 끄기 (트레이, Esc). 끄면 하던 쉬기·앉기는 평소 쉬기로, 벽에 매달려 있었으면 미끄러져 내려온다
    pub fn set_control(&mut self, on: bool) {
        if on {
            if self.control.is_none() {
                self.control = Some(Control::new(false));
            }
            self.control_ended = false;
        } else {
            self.release_control();
        }
    }

    /// 조작 모드 중
    pub fn is_controlled(&self) -> bool {
        self.control.is_some()
    }

    /// 내 동물의 키 (게임 루프가 조작 모드 동안 매 틱 — 친구 화면에 가 있어도). 키 없이 CONTROL_IDLE_SECS가 지나면 끝난다
    pub fn control_keys(&mut self, input: ControlInput, dt: f64) {
        let Some(c) = self.control.as_mut() else { return };
        c.feed(input);
        c.stale = 0.0;
        if !input.any() {
            c.idle += dt;
        }
        if c.idle >= CONTROL_IDLE_SECS {
            self.release_control();
            self.control_ended = true;
        }
    }

    /// 친구 링크로 온 키 (이 화면의 손님 — 앱이 그 주인 링크에서 온 것만 넘긴다). on = false면 조작이 끝난다
    pub fn remote_keys(&mut self, on: bool, input: ControlInput) {
        if !on {
            if self.control.is_some_and(|c| c.remote) {
                self.release_control();
            }
            return;
        }
        let c = self.control.get_or_insert_with(|| Control::new(true));
        c.remote = true;
        c.stale = 0.0;
        c.feed(input);
    }

    /// 키 없이 시간이 다 돼 조작 모드가 저절로 끝났으면 true를 한 번
    pub fn take_control_ended(&mut self) -> bool {
        std::mem::take(&mut self.control_ended)
    }

    fn release_control(&mut self) {
        if self.control.take().is_none() {
            return;
        }
        match self.state {
            State::Climb { .. } => self.state = State::Slide,
            State::Idle { .. } | State::Move { .. } | State::Trick { kind: Trick::SitFront, .. } => {
                self.pacing.after_beat = None;
                self.state = self.idle_state();
            }
            _ => {}
        }
        if self.cfg.flies && self.standing.is_none() {
            self.flight = None;
            self.hover_at = Some((self.body.x, self.body.y));
        }
    }

    /// 친구 링크로 온 키의 시간: 1초 소식이 없으면 다 놓은 것으로, 키 없이 CONTROL_IDLE_SECS면 끝 (내 동물은 control_keys가 센다)
    fn tick_remote_control(&mut self, dt: f64) {
        let Some(c) = self.control.as_mut().filter(|c| c.remote) else { return };
        c.stale += dt;
        if c.stale > REMOTE_STALE_SECS {
            c.input = ControlInput::default();
        }
        if !c.input.any() {
            c.idle += dt;
        }
        if c.idle >= CONTROL_IDLE_SECS {
            self.release_control();
        }
    }

    /// 지금 키를 따를 수 있음: 먹이·손길·하우스·떠나기·집 그리움·창 안이 아니고, 쉬기·걷기·공중·벽 타기 중
    fn controllable(&self) -> bool {
        let busy = self.housed
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.hand.is_some()
            || self.farewell_pending
            || self.inside.is_some()
            || self.is_heading_out();
        if busy {
            return false;
        }
        match self.state {
            State::Idle { .. }
            | State::Move { .. }
            | State::Turn { .. }
            | State::Land { .. }
            | State::Sleep { .. }
            | State::Trick { .. }
            | State::Huff { .. }
            | State::Tickled { .. }
            | State::Special { .. }
            | State::Annoyed { .. }
            | State::Hungry { .. } => true,
            // 던져진 나는 동물은 멈출 때까지 떠밀려 간다
            State::Air { .. } => !self.cfg.flies,
            State::Climb { .. } | State::Slide => !self.cfg.flies,
            _ => false,
        }
    }

    /// 조작 모드의 한 틱 (update에서, 평소 동작 전에). 키를 따랐으면 true — 아니면 평소 동작
    pub(super) fn update_control(&mut self, dt: f64, world: &World) -> bool {
        self.tick_remote_control(dt);
        let Some(c) = self.control else { return false };
        if !self.controllable() {
            if let Some(c) = self.control.as_mut() {
                c.prev = c.input;
            }
            return false;
        }
        let input = c.input;
        let pressed_down = input.down && !c.prev.down;
        let double_down = pressed_down && self.clock - c.last_down <= DOUBLE_TAP_SECS;
        if let Some(c) = self.control.as_mut() {
            c.prev = input;
            if pressed_down {
                c.last_down = if double_down { f64::NEG_INFINITY } else { self.clock };
            }
        }
        self.pacing.after_beat = None;
        self.before_press = None;
        if self.cfg.flies {
            self.control_fly(dt, world, input);
            return true;
        }
        match self.state {
            State::Air { .. } => {
                self.steer_air(dt, input.dir());
                self.update_air(dt, world);
            }
            State::Climb { target_y, .. } => self.control_climb(dt, input, target_y),
            State::Slide => self.update_slide(dt, world),
            _ => self.control_ground(dt, world, input, double_down),
        }
        true
    }

    /// 걷기·달리기 속도 (폴짝 동물은 박자에 맞춰)
    fn control_speed(&mut self, kind: Move, dt: f64) -> f64 {
        self.cfg.walk_speed * kind.speed_mult() * self.hop_pace(kind, dt)
    }

    /// 디딤판 위
    fn control_ground(&mut self, dt: f64, world: &World, input: ControlInput, double_down: bool) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.expect("follow_anchor needs standing");
        let Some(piece) = world.piece_at(st.owner, self.body.x).copied() else {
            self.start_fall();
            return;
        };
        let dir = input.dir();
        if dir != 0.0 {
            self.facing = dir;
        }
        if self.cfg.can_climb && input.up && dir != 0.0 && self.at_wall(&piece, dir) {
            self.start_control_climb(world);
            return;
        }
        if input.jump || input.up {
            self.control_jump(dir, input.run);
            return;
        }
        if double_down && matches!(st.owner, Owner::Window(_)) {
            // 창 윗면 바로 아래로 내려 다시 내려서지 않게 하고 떨어진다
            self.standing = None;
            self.body = Body { x: self.body.x, y: self.body.y + 1.0, vx: 0.0, vy: 0.0 };
            self.state = State::Air { anim: Anim::Fall };
            return;
        }
        if dir != 0.0 {
            self.control_walk(dt, world, st, &piece, dir, if input.run { Move::Run } else { Move::Walk });
            return;
        }
        if input.down {
            self.state = State::Trick { kind: Trick::SitFront, remaining: SIT_HOLD_SECS };
            return;
        }
        // 키 없음: 간질이기·클릭 반응은 다 하고 나서 쉰다. 저절로 걷거나 자지 않는다
        self.react_to_tickle();
        self.update_chatter(dt);
        self.state = match self.state {
            State::Trick { kind, remaining } => countdown(remaining, dt, |remaining| State::Trick { kind, remaining }),
            State::Huff { remaining } => countdown(remaining, dt, |remaining| State::Huff { remaining }),
            State::Tickled { remaining } => countdown(remaining, dt, |remaining| State::Tickled { remaining }),
            State::Special { kind, remaining } => countdown(remaining, dt, |remaining| State::Special { kind, remaining }),
            State::Annoyed { remaining, tickled_for } => countdown(remaining, dt, |remaining| State::Annoyed { remaining, tickled_for }),
            State::Hungry { remaining } => countdown(remaining, dt, |remaining| State::Hungry { remaining }),
            State::Land { remaining } => countdown(remaining, dt, |remaining| State::Land { remaining }),
            State::Sting { .. } => self.state,
            State::Idle { look, .. } => State::Idle { remaining: 1.0, look },
            _ => CONTROL_IDLE,
        };
    }

    /// 몸 앞쪽이 dir 쪽 화면 끝 벽에 닿아 있음
    fn at_wall(&self, piece: &Surface, dir: f64) -> bool {
        let (edge_x, end) = if dir > 0.0 { (piece.x_end, piece.end_kind) } else { (piece.x_start, piece.start_kind) };
        end == EndKind::Wall && (edge_x - (self.body.x + dir * self.cfg.half_w)) * dir <= WALL_REACH
    }

    /// dir 쪽으로 걷거나 달린다. 창 모서리에선 걸어 떨어지고, 화면 끝 벽에선 (친구가 있으면 나가고) 멈춘다
    fn control_walk(&mut self, dt: f64, world: &World, st: Standing, piece: &Surface, dir: f64, kind: Move) {
        self.state = State::Move { kind, remaining: 1.0 };
        let speed = self.control_speed(kind, dt);
        let new_x = self.body.x + dir * speed * dt;
        let (edge_x, end) = if dir > 0.0 { (piece.x_end, piece.end_kind) } else { (piece.x_start, piece.start_kind) };
        let beyond = |x: f64| (x - edge_x) * dir > 0.0;
        let step_to = |p: &mut Pet, x: f64| {
            p.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (x - p.body.x) });
            p.body.x = x;
        };
        if !beyond(new_x + dir * self.cfg.half_w) {
            step_to(self, new_x);
            return;
        }
        match end {
            EndKind::Seam { to } => {
                if !self.at_seam(world, to, kind, speed, new_x, beyond(new_x)) && matches!(self.state, State::Turn { .. }) {
                    // 너무 높은 옆 모니터 — 돌아서지 않고 그 자리에 선다
                    self.state = CONTROL_IDLE;
                }
            }
            EndKind::Edge => {
                // 발 중심이 모서리를 넘을 때까지 걷다가 떨어진다
                step_to(self, new_x);
                if beyond(new_x) {
                    self.standing = None;
                    self.body.vx = dir * speed;
                    self.body.vy = 0.0;
                    self.state = State::Air { anim: Anim::Fall };
                }
            }
            EndKind::Wall | EndKind::Blocked => {
                // 친구가 연결된 화면 끝이면 평소처럼 걸어 나간다
                if end == EndKind::Wall {
                    if let Some(side) = visit::screen_edge(world, piece, dir) {
                        if self.exit_odds(side) > 0.0 {
                            self.start_exit(side);
                            return;
                        }
                    }
                }
                let stop_x = edge_x - dir * self.cfg.half_w;
                if (stop_x - self.body.x) * dir > 0.0 {
                    step_to(self, stop_x);
                }
                self.state = CONTROL_IDLE;
            }
        }
    }

    /// 진짜 점프: 몸 JUMP_BODIES배 높이 (개구리는 FROG_JUMP_MULT배 더), 누르고 있던 쪽으로 걷기·달리기 속도
    fn control_jump(&mut self, dir: f64, run: bool) {
        let mult = if self.cfg.hops { FROG_JUMP_MULT } else { 1.0 };
        let height = JUMP_BODIES * self.cfg.size * mult;
        let kind = if run { Move::Run } else { Move::Walk };
        self.standing = None;
        self.body.vx = dir * self.cfg.walk_speed * kind.speed_mult();
        self.body.vy = -(2.0 * GRAVITY * height).sqrt();
        self.state = State::Air { anim: Anim::Hop };
    }

    /// 공중에서 ←/→로 조금 방향을 튼다 (달리기 속도까지만 — 이미 더 빠르면 늦추기만)
    fn steer_air(&mut self, dt: f64, dir: f64) {
        if dir == 0.0 {
            return;
        }
        let cap = (self.cfg.walk_speed * Move::Run.speed_mult()).max(self.body.vx.abs());
        let vx = self.body.vx + dir * AIR_STEER * self.cfg.walk_speed * dt;
        self.body.vx = vx.clamp(-cap, cap);
    }

    /// 화면 끝 벽을 기어오르기 시작한다 (목표 = 그 화면 맨 위)
    fn start_control_climb(&mut self, world: &World) {
        let wa = world.work_area_at(self.body.x, self.body.y);
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Climb { target_y: wa.top + self.cfg.size, hang: behavior::CLIMB_HANG_SECS };
    }

    /// 벽 타기: ↑ 오르기, 키 없음 매달리기, ↓ 미끄러져 내려오기, Space 벽에서 뛰어내리기
    fn control_climb(&mut self, dt: f64, input: ControlInput, target_y: f64) {
        if input.down {
            self.state = State::Slide;
        } else if input.jump {
            self.start_hop(-self.facing);
        } else if input.up && self.body.y > target_y {
            self.body.y = (self.body.y - self.scaled(behavior::CLIMB_SPEED) * self.climb_pace(dt) * dt).max(target_y);
        }
    }

    /// 나는 동물: 방향키로 위아래 좌우 (Shift = 빠르게). 놓으면 그 자리에 떠 있다. 친구가 연결된 끝으로 가면 날아 나간다
    fn control_fly(&mut self, dt: f64, world: &World, input: ControlInput) {
        self.flight = None;
        let (dx, dy) = (input.dir(), input.vertical());
        if dx == 0.0 && dy == 0.0 {
            if self.standing.is_some() && !self.follow_anchor(world) {
                self.take_off();
            }
            if self.standing.is_none() {
                self.hover_in_place(world);
            }
            self.react_to_tickle();
            if matches!(self.state, State::Sting { .. }) {
                return;
            }
            self.update_chatter(dt);
            self.state = match self.state {
                State::Trick { kind, remaining } => countdown(remaining, dt, |remaining| State::Trick { kind, remaining }),
                State::Huff { remaining } => countdown(remaining, dt, |remaining| State::Huff { remaining }),
                State::Tickled { remaining } => countdown(remaining, dt, |remaining| State::Tickled { remaining }),
                State::Special { kind, remaining } => countdown(remaining, dt, |remaining| State::Special { kind, remaining }),
                State::Hungry { remaining } => countdown(remaining, dt, |remaining| State::Hungry { remaining }),
                State::Idle { look, .. } => State::Idle { remaining: 1.0, look },
                _ => CONTROL_IDLE,
            };
            return;
        }
        if self.standing.is_some() {
            self.take_off();
        }
        self.hover_at = None;
        let kind = if input.run { Move::Run } else { Move::Walk };
        let speed = self.fly_speed(kind);
        let len = dx.hypot(dy);
        let b = self.fly_limits(world);
        let (x, y) = (self.body.x + dx / len * speed * dt, self.body.y + dy / len * speed * dt);
        if dx != 0.0 {
            self.facing = dx;
        }
        if x < b.left || x > b.right {
            let side = if x < b.left { Side::Left } else { Side::Right };
            if self.exit_odds(side) > 0.0 {
                self.start_exit(side);
                return;
            }
        }
        self.body = Body { x: x.clamp(b.left, b.right), y: y.clamp(b.top, b.bottom), vx: 0.0, vy: 0.0 };
        self.state = State::Move { kind, remaining: 1.0 };
    }
}

/// 남은 시간을 흘려 아직이면 그 상태, 다 됐으면 조작 중 쉬기
fn countdown(remaining: f64, dt: f64, make: impl FnOnce(f64) -> State) -> State {
    let remaining = remaining - dt;
    if remaining > 0.0 {
        make(remaining)
    } else {
        CONTROL_IDLE
    }
}
