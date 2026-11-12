//! 🫣 숨바꼭질 (흐물이, v0.6.3) — brain.rs의 자식 모듈.
//!
//! 바닥에서 쉬거나 걷던 흐물이가 가끔 (HIDE_RATE, 쿨다운 COOLDOWN_SECS) 가까운 창의 옆면 뒤로 달려가 숨는다 — 머리만 쏙 (peek).
//! 진짜로 창 뒤로 가지는 않는다 (펫 창은 늘 맨 앞): 몸 가운데를 옆면 안쪽 HIDE_INSET에 두고, 그림이 옆면 안쪽(창 쪽)은 그리지 않아
//! 창에 가린 것처럼 보인다. 그래서 숨을 수 있는 곳은 그 창이 몸을 위아래로 다 덮고, 그 자리에서 맨 앞 창인 옆면뿐이다
//! (같은 바닥 조각 위, HIDE_REACH 안).
//!
//! 커서가 머리에서 몸 FOUND_BODIES배 안으로 오면 "Found me!" 하고 커서 반대쪽으로 줄행랑 (panic). 아무도 못 찾으면
//! HIDE_SECS 뒤 "Peekaboo~" 하고 걸어 나온다. 창이 움직이면 따라가고, 사라지거나 몸을 더 덮지 않으면 그냥 나온다.

use super::*;

/// 바닥에서 쉬거나 걷는 동안 숨으러 갈 빈도 (초당 확률 — 숨을 곳이 있을 때)
pub const HIDE_RATE: f64 = 1.0 / 60.0;
/// 한 번 숨은 뒤 다시 숨기까지
pub const COOLDOWN_SECS: (f64, f64) = (240.0, 420.0);
const FIRST_READY_SECS: f64 = 90.0;
/// 몸 가운데가 옆면 안쪽으로 이만큼 (몸 배) — 그림 peek은 이 옆면 오른쪽을 그리지 않는다 (tools/draw_heumul.py HIDE_INSET)
pub const HIDE_INSET: f64 = 0.15;
/// 숨으러 갈 수 있는 거리 (몸 배)
pub const HIDE_REACH: f64 = 6.0;
/// 숨어 있는 시간 (못 찾으면 나온다)
pub const HIDE_SECS: (f64, f64) = (20.0, 40.0);
/// 커서가 머리에서 몸 이만큼 안이면 들켰다
pub const FOUND_BODIES: f64 = 2.0;
/// 숨을 곳까지 달려가다 이만큼 못 닿으면 그만둔다
const APPROACH_SECS: f64 = 8.0;
/// 나올 때 걷는 시간
const COME_OUT_SECS: (f64, f64) = (1.5, 3.0);

/// 숨바꼭질 단계 — window의 side 옆면 (Left = 창 왼쪽 옆면: 창이 몸 오른쪽에 있다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HidePhase {
    /// 옆면 뒤 자리로 달려간다 (remaining = 남은 제한 시간)
    Approach { window: u64, side: Side },
    /// 머리만 내밀고 숨어 있다 (remaining = 못 찾으면 나올 때까지)
    Peek { window: u64, side: Side },
}

#[derive(Debug, Clone)]
pub struct HideMood {
    rng: fastrand::Rng,
    pub(super) ready_at: f64,
}

impl HideMood {
    pub(super) fn new(seed: u64) -> Self {
        HideMood { rng: fastrand::Rng::with_seed(seed ^ 0x6869_6465), ready_at: FIRST_READY_SECS }
    }
}

/// 창 안쪽 방향 (+1 = 오른쪽)
fn inward(side: Side) -> f64 {
    if side == Side::Left {
        1.0
    } else {
        -1.0
    }
}

impl Pet {
    /// 이 창 옆면 뒤에 숨을 자리 (몸 가운데 x) — 그 창이 몸을 다 덮고 그 자리 맨 앞이며, 지금 바닥 조각 위일 때만
    fn hide_x(&self, world: &World, window: u64, side: Side) -> Option<f64> {
        let st = self.standing?;
        if st.owner == Owner::Window(window) {
            return None;
        }
        let w = world.window(window)?;
        let r = w.rect;
        let size = self.cfg.size;
        let edge = if side == Side::Left { r.left } else { r.right };
        let x = edge + inward(side) * HIDE_INSET * size;
        let y = self.body.y;
        let piece = world.piece_at(st.owner, self.body.x)?;
        let covers = r.top <= y - 0.9 * size && r.bottom >= y - 0.05 * size && r.left < x && x < r.right;
        let front = world.front_window_at(x, y - 0.45 * size).map(|f| f.id) == Some(window);
        // 머리가 내미는 쪽은 화면 안이고 같은 바닥 위
        let peek_x = edge - inward(side) * 0.3 * size;
        let room = (world.bounds.left..world.bounds.right).contains(&peek_x) && (piece.x_start..=piece.x_end).contains(&peek_x);
        (covers && front && room && (piece.x_start..=piece.x_end).contains(&x)).then_some(x)
    }

    /// 가장 가까운 숨을 곳 (창, 옆면, 몸 가운데 x)
    fn hide_spot(&self, world: &World) -> Option<(u64, Side, f64)> {
        let reach = HIDE_REACH * self.cfg.size;
        world
            .windows
            .iter()
            .flat_map(|w| [(w.id, Side::Left), (w.id, Side::Right)])
            .filter_map(|(id, side)| self.hide_x(world, id, side).map(|x| (id, side, x)))
            .filter(|&(_, _, x)| (x - self.body.x).abs() <= reach)
            .min_by(|a, b| (a.2 - self.body.x).abs().total_cmp(&(b.2 - self.body.x).abs()))
    }

    #[cfg(test)]
    pub(super) fn hide_spot_for_test(&self, world: &World) -> Option<(u64, Side)> {
        self.hide_spot(world).map(|(id, side, _)| (id, side))
    }

    pub(super) fn maybe_hide(&mut self, dt: f64, world: &World) -> bool {
        if self.clock < self.hide.ready_at || !self.antics_free() || self.hide.rng.f64() >= HIDE_RATE * dt {
            return false;
        }
        self.start_hide(world)
    }

    /// 가까운 옆면 뒤로 숨으러 간다 (주사위 없이). 숨을 곳이 없으면 false
    pub(super) fn start_hide(&mut self, world: &World) -> bool {
        let Some((window, side, _)) = self.hide_spot(world) else { return false };
        self.hide.ready_at = self.clock + behavior::range(&mut self.hide.rng, COOLDOWN_SECS);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.state = State::Hide { phase: HidePhase::Approach { window, side }, remaining: APPROACH_SECS };
        true
    }

    pub(super) fn update_hide(&mut self, dt: f64, world: &World, phase: HidePhase, remaining: f64) {
        if self.antics_interrupted() {
            self.state = self.idle_state();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let remaining = remaining - dt;
        match phase {
            HidePhase::Approach { window, side } => {
                let Some(x) = self.hide_x(world, window, side) else {
                    self.state = self.rest_beat();
                    return;
                };
                if remaining <= 0.0 {
                    self.state = self.rest_beat();
                    return;
                }
                let dx = x - self.body.x;
                let step = self.cfg.walk_speed * Move::Run.speed_mult() * dt;
                if dx.abs() > step {
                    self.facing = dx.signum();
                    if self.juggle_walk(world, self.facing, step).is_some() {
                        self.state = State::Hide { phase, remaining };
                    }
                    return;
                }
                if self.juggle_walk(world, dx.signum(), dx.abs()).is_none() {
                    return;
                }
                // 숨었다: 창 반대쪽(바깥)을 보고 머리만 쏙
                self.facing = inward(side);
                let secs = behavior::range(&mut self.hide.rng, HIDE_SECS);
                self.state = State::Hide { phase: HidePhase::Peek { window, side }, remaining: secs };
            }
            HidePhase::Peek { window, side } => {
                let Some(x) = self.hide_x(world, window, side) else {
                    self.come_out(side);
                    return;
                };
                if (x - self.body.x).abs() > 0.5 && self.juggle_walk(world, (x - self.body.x).signum(), (x - self.body.x).abs()).is_none() {
                    return;
                }
                if self.cursor_finds_me(side) {
                    let dir = if self.cursor.0 > self.body.x { -1.0 } else { 1.0 };
                    self.facing = dir;
                    self.speech = Some(Speech::Found);
                    self.state = State::Panic { remaining: behavior::range(&mut self.hide.rng, heumul_moves::PANIC_SECS), dir };
                    return;
                }
                if remaining <= 0.0 {
                    self.speech = Some(Speech::Peekaboo);
                    self.come_out(side);
                    return;
                }
                self.state = State::Hide { phase, remaining };
            }
        }
    }

    /// 내민 머리 가까이 커서가 왔다
    fn cursor_finds_me(&self, side: Side) -> bool {
        let size = self.cfg.size;
        let head = (self.body.x - inward(side) * (HIDE_INSET + 0.15) * size, self.body.y - 0.65 * size);
        (self.cursor.0 - head.0).hypot(self.cursor.1 - head.1) <= FOUND_BODIES * size
    }

    /// 창 바깥쪽으로 걸어 나온다
    fn come_out(&mut self, side: Side) {
        self.facing = -inward(side);
        let secs = behavior::range(&mut self.hide.rng, COME_OUT_SECS);
        self.state = State::Move { kind: Move::Walk, remaining: secs };
    }

    pub(super) fn hide_anim(&self, phase: HidePhase) -> Anim {
        match phase {
            HidePhase::Approach { .. } => Anim::Run,
            HidePhase::Peek { .. } => Anim::Peek,
        }
    }
}
