//! 위/아래 친구 화면 (v0.3.6): 친구 화면이 내 화면 위나 아래에 붙어 있으면
//! - 높이 뛰어 나가기: 이동을 정할 때 가끔(visit::vertical_exit_chance) 웅크렸다가 "boing!" 하고 화면 위로 솟구쳐 나간다
//!   (바닥이나 창 윗면에서, 머리 위가 비었을 때만 — 개구리는 더 자주). 나는 동물은 그냥 위로 날아 나간다.
//! - 바닥 밑으로 쏙: 가장 아래 바닥(작업표시줄 위)에서 폴짝 뛰었다가 "whee!" 하고 바닥을 뚫고 아래로 빠진다. 나는 동물은 아래로 날아 나간다.
//! - 던지기: 위 끝으로 세게 던지거나 아래 바닥으로 내리꽂으면 듀얼 모니터처럼 넘어간다 (Fling.height = 폭 비율 0 왼쪽 … 1 오른쪽).
//! - 들어오기: 위에서 오면(위 끝으로 들어옴) 위에서 떨어져 내려서고, 아래에서 오면 바닥 밑에서 "boing!" 튀어 올라 내려선다.
//!   나는 동물은 위/아래에서 날아 들어온다.
//!
//! 상태는 State::Exit / State::Enter 그대로 (side가 Up/Down). brain.rs의 자식 모듈.

use super::*;
use crate::visit::Fling;

impl Pet {
    /// 친구가 연결된 위/아래 끝을 정한다 (set_exit_edges는 왼쪽/오른쪽)
    pub fn set_vertical_edges(&mut self, up: bool, down: bool) {
        self.exit_edges = ExitEdges { up, down, ..self.exit_edges };
    }

    /// move_state 처음: 위/아래로 나갈지 정한다 (나가면 그 상태). 창 안·집에 가는 길·보내는 중·먹이·조작 중엔 안 한다
    pub(super) fn vertical_exit_state(&mut self, world: &World) -> Option<State> {
        if self.control.is_some() || self.inside.is_some() || self.home_trip.is_some() || self.send.is_some() || self.food.is_some() || self.housed {
            return None;
        }
        if !self.cfg.flies && !matches!(self.standing, Some(Standing { owner: Owner::Ground(_) | Owner::Window(_), .. })) {
            return None;
        }
        for side in [Side::Up, Side::Down] {
            let chance = visit::vertical_exit_chance(side, self.exit_edges, self.guest, self.cfg.hops);
            if chance <= 0.0 || !self.can_go_vertical(world, side) {
                continue;
            }
            if chance >= 1.0 || self.rng.f64() < chance {
                return Some(self.vertical_exit(side));
            }
        }
        None
    }

    /// 지금 자리에서 side(위/아래) 끝으로 나갈 수 있는지: 그 줄이 가장 바깥 끝에 닿아야 하고,
    /// 걷는 동물은 위로면 머리 위(SKY_HEADROOM_SIZES)에 창이 없어야, 아래로면 가장 아래 바닥에 서 있어야 한다
    pub(super) fn can_go_vertical(&self, world: &World, side: Side) -> bool {
        let (x, y) = (self.body.x, self.body.y);
        if !visit::outer_edge_at(world, side, x) {
            return false;
        }
        if self.cfg.flies {
            return true;
        }
        match side {
            Side::Up => {
                let (hw, top) = (self.cfg.half_w, y - visit::SKY_HEADROOM_SIZES * self.cfg.size);
                !world.windows.iter().any(|w| w.rect.left < x + hw && x - hw < w.rect.right && w.rect.top < y - 1.0 && w.rect.bottom > top)
            }
            Side::Down => {
                matches!(self.standing, Some(Standing { owner: Owner::Ground(_), .. })) && (y - world.bounds.bottom).abs() <= 2.0
            }
            Side::Left | Side::Right => false,
        }
    }

    /// 위/아래로 나가기 시작 — 걷는 동물은 웅크리기부터, 나는 동물은 바로 날아간다
    pub(super) fn vertical_exit(&mut self, side: Side) -> State {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.flight = None;
        if self.cfg.flies {
            self.standing = None;
            self.body.vx = 0.0;
            self.body.vy = 0.0;
            State::Exit { remaining: visit::VERTICAL_MAX_SECS, side }
        } else {
            State::Exit { remaining: visit::VERTICAL_CROUCH_SECS, side }
        }
    }

    /// 위/아래 나가기·들어오기 한 틱 (State::Exit / State::Enter의 side가 Up/Down)
    pub(super) fn update_vertical(&mut self, dt: f64, world: &World) {
        match self.state {
            State::Exit { remaining, side } if self.cfg.flies => self.update_fly_vexit(dt, world, remaining, side),
            State::Exit { remaining, side } => self.update_vexit(dt, world, remaining, side),
            State::Enter { remaining, side } if self.cfg.flies => self.update_fly_venter(dt, world, remaining, side),
            State::Enter { remaining, side } => self.update_venter(dt, world, remaining, side),
            _ => {}
        }
    }

    /// 나는 동물이 위/아래로 날아 나가거나 들어오는 속도 (px/s) — 화면 높이를 몇 초 안에 지나간다
    fn vertical_fly_speed(&self) -> f64 {
        self.fly_speed(Move::Run).max(self.scaled(450.0))
    }

    /// 걷는 동물: 웅크림(서 있는 동안) → 위로 솟구치거나(Up) 폴짝 뒤 바닥을 뚫고(Down) → 몸이 다 빠져나가면 사라진다
    fn update_vexit(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        let b = world.bounds;
        if self.standing.is_some() {
            if !self.follow_anchor(world) {
                self.standing = None;
            }
            let remaining = remaining - dt;
            if remaining > 0.0 && self.standing.is_some() {
                self.state = State::Exit { remaining, side };
                return;
            }
            // 뛴다: 위로는 화면 위 끝을 넉넉히 넘을 만큼, 아래로는 살짝 폴짝
            self.standing = None;
            let size = self.cfg.size;
            let rise = match side {
                Side::Up => (self.body.y - b.top + size).max(size) * 1.2,
                _ => visit::DIVE_HOP_SIZES * size,
            };
            self.body.vx = 0.0;
            self.body.vy = -(2.0 * GRAVITY * rise).sqrt();
            self.speech = Some(if side == Side::Up { Speech::Boing } else { Speech::Whee });
            self.state = State::Exit { remaining: visit::VERTICAL_MAX_SECS, side };
            return;
        }
        self.body.vy += GRAVITY * dt;
        self.body.y += self.body.vy * dt;
        let remaining = remaining - dt;
        if remaining <= 0.0 || visit::fully_outside_v(&b, side, self.body.y, self.cfg.size) {
            self.vanish(side);
        } else {
            self.state = State::Exit { remaining, side };
        }
    }

    fn update_fly_vexit(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.standing = None;
        let dir = if side == Side::Up { -1.0 } else { 1.0 };
        self.body.y += dir * self.vertical_fly_speed() * dt;
        let remaining = remaining - dt;
        if remaining <= 0.0 || visit::fully_outside_v(&world.bounds, side, self.body.y, self.cfg.size) {
            self.vanish(side);
        } else {
            self.state = State::Exit { remaining, side };
        }
    }

    /// 위/아래 끝(side) 바로 바깥에 두고 들어오게 한다 (enter가 부른다). frac = 그 끝 어디쯤 (폭 비율, 없으면 아무 데나).
    /// 위에서: 몸이 막 화면 위에 있는 자리에서 떨어진다. 아래에서: 바닥 밑에서 "boing!" 튀어 오른다
    pub(super) fn enter_vertical(&mut self, world: &World, side: Side, frac: Option<f64>) {
        let b = world.bounds;
        let size = self.cfg.size;
        let x = visit::vertical_entry_x(world, side, frac, self.cfg.half_w, &mut self.rng);
        let y = if side == Side::Up { b.top } else { b.bottom + size };
        let vy = if side == Side::Down && !self.cfg.flies {
            self.speech = Some(Speech::Boing);
            self.pop_up_vy()
        } else {
            0.0
        };
        self.body = Body { x, y, vx: 0.0, vy };
        self.standing = None;
        self.flight = None;
        self.hover_at = None;
        self.state = State::Enter { remaining: visit::VERTICAL_MAX_SECS, side };
    }

    /// 아래에서 튀어 오르는 처음 속도 (바닥 밑 몸 하나 + POP_UP_SIZES만큼 솟는다)
    fn pop_up_vy(&self) -> f64 {
        -(2.0 * GRAVITY * (1.0 + visit::POP_UP_SIZES) * self.cfg.size).sqrt()
    }

    /// 걷는 동물이 위/아래에서 들어오는 중: 화면 끝을 막지 않고 날아가다가 몸이 다 들어오면 평소 공중(떨어져 내려서기)으로
    fn update_venter(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        let b = world.bounds;
        let g = self.air_gravity();
        self.body.vy += g * dt;
        self.body.y += self.body.vy * dt;
        let hw = self.cfg.half_w;
        self.body.x = (self.body.x + self.body.vx * dt).clamp(b.left + hw, (b.right - hw).max(b.left + hw));
        let remaining = remaining - dt;
        let inside = visit::fully_inside_v(&b, side, self.body.y, self.cfg.size);
        if !inside && remaining > 0.0 {
            self.state = State::Enter { remaining, side };
            return;
        }
        if !inside {
            self.body.y = if side == Side::Up { b.top + self.cfg.size } else { b.bottom };
        }
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 나는 동물이 위/아래에서 날아 들어오는 중: 날 수 있는 높이 안으로 들어오면 평소 날기로
    fn update_fly_venter(&mut self, dt: f64, world: &World, remaining: f64, side: Side) {
        self.standing = None;
        let dir = if side == Side::Up { 1.0 } else { -1.0 };
        self.body.y += dir * self.vertical_fly_speed() * dt;
        let lim = self.fly_limits(world);
        let inside = if side == Side::Up { self.body.y >= lim.top } else { self.body.y <= lim.bottom };
        let remaining = remaining - dt;
        if inside || remaining <= 0.0 {
            self.body.y = self.body.y.clamp(lim.top, lim.bottom.max(lim.top));
            self.state = State::Move { kind: Move::Walk, remaining: crate::flight::MAX_FLIGHT_SECS };
        } else {
            self.state = State::Enter { remaining, side };
        }
    }

    /// 위/아래로 나가거나 들어오는 그림
    pub(super) fn vertical_anim(&self) -> Anim {
        let (exiting, side) = match self.state {
            State::Exit { side, .. } => (true, side),
            State::Enter { side, .. } => (false, side),
            _ => return Anim::Fall,
        };
        if self.cfg.flies {
            return Anim::Walk;
        }
        match (exiting, side, self.standing.is_some()) {
            (true, Side::Up, _) if self.cfg.hops => Anim::BigJump,
            // 웅크림
            (true, _, true) => Anim::Land,
            _ => Anim::Fall,
        }
    }

    /// 던진 비행의 다음 발 높이(next_y)가 top 위로(올라가는 중) 또는 아래 바닥 bottom 밑으로(세게 내리꽂는 중) 넘어가면:
    /// 그 끝으로 넘어갈 수 있으면 넘어가고 true
    pub(super) fn fling_out_vertical(&mut self, world: &World, next_y: f64, top: f64, bottom: f64) -> bool {
        let vy = self.body.vy;
        let min_down = if self.cfg.flies { visit::FLY_FLING_DOWN_MIN_SPEED } else { visit::FLING_DOWN_MIN_SPEED };
        let side = if vy < 0.0 && next_y < top {
            Side::Up
        } else if vy >= min_down * self.cfg.dpi && self.body.y <= bottom && next_y >= bottom {
            Side::Down
        } else {
            return false;
        };
        if !self.thrown || self.home_trip.is_some() || self.inside.is_some() || !visit::outer_edge_at(world, side, self.body.x) || self.exit_odds(side) <= 0.0 {
            return false;
        }
        let dpi = self.cfg.dpi.max(0.01);
        let fling = Fling { height: visit::edge_fraction(&world.bounds, self.body.x), vx: self.body.vx / dpi, vy: vy / dpi }.sanitized();
        self.thrown = false;
        self.vanish(side);
        self.fling = fling;
        true
    }

    /// 다른 화면에서 위/아래로 던져져 들어온다 (enter_flung이 부른다): 그 끝의 폭 비율 자리에서, 가로 속도를 이어 받고
    /// 위에서면 내려오는 속도, 아래에서면 적어도 튀어 오를 만큼의 속도로. 나는 동물은 평소처럼 날아 들어온다
    pub(super) fn enter_flung_vertical(&mut self, world: &World, side: Side, fling: Fling) {
        self.enter(world, side);
        let Some(f) = fling.sanitized() else { return };
        if self.cfg.flies {
            return;
        }
        self.enter_vertical(world, side, Some(f.height));
        let dpi = self.cfg.dpi;
        self.body.vx = f.vx * dpi;
        self.body.vy = if side == Side::Up { (f.vy * dpi).max(0.0) } else { (f.vy * dpi).min(self.pop_up_vy()) };
        if self.body.vx.abs() > 1.0 {
            self.facing = self.body.vx.signum();
        }
        self.thrown = true;
    }
}
