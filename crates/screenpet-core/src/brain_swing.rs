//! 🎣 커서 매달리기 (흐물이, v0.6.3) — brain.rs의 자식 모듈.
//!
//! 쉬거나 걷는 흐물이의 머리 바로 위에 커서가 NOTICE_SECS(0.6초) 머물면 폴짝 뛰어 두 손으로 커서 끝을 잡는다.
//! 매달린 동안 몸은 커서 끝(손)을 따라가고, 커서가 움직이면 진자처럼 흔들린다 (render.swing — 웹뷰가 손 자리를 축으로 돌린다).
//! "Wheee~", 세게 흔들수록 신나서 하트와 "More!".
//!
//! 놓는 때: 커서를 아주 빠르게 휘두르면 (FLING_SPEED) 그 속도로 던져진다 (사람이 던진 것과 같다 — 친구가 연결된 끝을 넘으면 친구에게) ·
//! HANG_SECS가 지나면 · 클릭하면 · 커서가 전체 화면 앱 위나 화면 밖으로 가면 · 커서 장난을 끄거나 바빠지면 (숨김·잡힘·음식) ·
//! 자리 비움·집중이면. 쿨다운 COOLDOWN_SECS (3분 넘게).
//!
//! 클릭은 언제나 사용자 것: 뛰는 동안부터 매달린 동안 cursor_through가 참이고, 놓은 뒤에도 떨어져 커서에서 벗어날 때까지 통과시킨다.

use std::collections::VecDeque;

use super::*;

/// 머리 위에 커서가 이만큼 머물면 잡는다
pub const NOTICE_SECS: f64 = 0.6;
/// 머리 위: 옆으로 몸 이만큼 안, 머리 꼭대기 위로 몸 이만큼 안
pub const ABOVE_REACH: f64 = 0.45;
pub const ABOVE_RISE: f64 = 0.9;
/// 매달려 있는 시간
pub const HANG_SECS: (f64, f64) = (8.0, 20.0);
/// 놓은 뒤 다시 매달리기까지 (3분 넘게)
pub const COOLDOWN_SECS: (f64, f64) = (180.0, 300.0);
/// 앱을 켜고 처음 매달리기까지
pub const FIRST_READY_SECS: f64 = 45.0;
/// 커서 끝으로 뛰어오르는 시간과 호의 높이 (몸 배)
pub const JUMP_SECS: f64 = 0.35;
const JUMP_ARC: f64 = 0.3;
/// 뛰는 동안 커서가 몸 이만큼 넘게 달아나면 헛손질
const MISS_BODIES: f64 = 2.0;
/// 진자 길이 (손에서 몸 가운데까지 — 몸 배), 감쇠 (1/s), 그림이 기우는 최대 각도 (도)
pub const PENDULUM_LEN: f64 = 0.55;
const DAMPING: f64 = 0.4;
pub const MAX_SWING_DEG: f64 = 40.0;
/// 이보다 빠르게 커서를 휘두르면 그 속도로 던져진다 (논리 px/s)
pub const FLING_SPEED: f64 = 1500.0;
/// 던져질 때 커서 속도를 이어 받는 몫
const FLING_KEEP: f64 = 0.8;
/// 커서 속도를 재는 창 (초)
const VELOCITY_SECS: f64 = 0.08;
/// 이만큼 (rad/s) 빠르게 흔들리면 신난다 — 하트, 가끔 "더 더!"
pub const HAPPY_OMEGA: f64 = 2.2;
/// "Wheee~" · "More!" 간격
const TALK_SECS: (f64, f64) = (2.5, 4.5);

/// 매달리기 단계
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwingPhase {
    /// 커서 끝으로 뛰어오르는 중 (충돌 없이)
    Jump { from: (f64, f64), elapsed: f64 },
    /// 두 손으로 매달려 흔들흔들
    Hang,
}

#[derive(Debug, Clone)]
pub struct SwingMood {
    rng: fastrand::Rng,
    /// 이 시각(clock)부터 매달릴 수 있다
    pub(super) ready_at: f64,
    /// 커서가 머리 위에 머문 시간
    above_for: f64,
    /// 진자 각도 (rad, + = 몸이 커서 오른쪽으로) · 각속도
    theta: f64,
    omega: f64,
    /// 커서 자리 기록 (clock, x, y) · 지금 속도와 직전 속도 (물리 px/s)
    hist: VecDeque<(f64, f64, f64)>,
    vel: (f64, f64),
    prev_vel: (f64, f64),
    /// 뛰기 시작했을 때 커서 자리
    aim: (f64, f64),
    /// 직전 틱에 왼쪽 버튼이 눌려 있었다 (새로 누른 것만 클릭으로 본다)
    was_down: bool,
    talk_in: f64,
}

impl SwingMood {
    pub(super) fn new(seed: u64) -> Self {
        SwingMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x7377_696e),
            ready_at: FIRST_READY_SECS,
            above_for: 0.0,
            theta: 0.0,
            omega: 0.0,
            hist: VecDeque::new(),
            vel: (0.0, 0.0),
            prev_vel: (0.0, 0.0),
            aim: (0.0, 0.0),
            was_down: false,
            talk_in: 0.0,
        }
    }
}

impl Pet {
    /// 매 틱 (흐물이): 커서 속도와 머리 위에 머문 시간
    pub(super) fn track_swing_cursor(&mut self, dt: f64) {
        let (cx, cy) = self.cursor;
        let clock = self.clock;
        let above = self.cursor_above_head();
        let m = &mut self.swing;
        m.hist.push_back((clock, cx, cy));
        while m.hist.len() > 2 && m.hist.front().is_some_and(|&(t, _, _)| clock - t > VELOCITY_SECS) {
            m.hist.pop_front();
        }
        m.prev_vel = m.vel;
        if let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (m.hist.front(), m.hist.back()) {
            if t1 - t0 > 1e-6 {
                m.vel = ((x1 - x0) / (t1 - t0), (y1 - y0) / (t1 - t0));
            }
        }
        m.above_for = if above { m.above_for + dt } else { 0.0 };
    }

    /// 커서가 머리 바로 위에 있다
    fn cursor_above_head(&self) -> bool {
        let size = self.cfg.size;
        let (cx, cy) = self.cursor;
        let top = self.body.y - HEAD_TOP * size;
        (cx - self.body.x).abs() <= ABOVE_REACH * size && cy < top && top - cy <= ABOVE_RISE * size
    }

    /// 커서가 화면 밖이거나 전체 화면 앱 위에 있다
    fn cursor_gone(&self, world: &World) -> bool {
        let (cx, cy) = self.cursor;
        let b = world.bounds;
        !(b.left..b.right).contains(&cx) || !(b.top..b.bottom).contains(&cy) || crate::weather::fullscreen_over(&world.windows, &world.work_area_at(cx, cy))
    }

    /// 매달릴 수 없는 때 (커서 장난 꺼 둠·앱이 바쁨·자리 비움·집중·볼일)
    fn swing_blocked(&self, world: &World) -> bool {
        let (enabled, busy, _) = self.cursor_play_input();
        !enabled || busy || self.antics_interrupted() || self.cursor_gone(world)
    }

    /// 머리 위에 커서가 머물면 뛰어올라 잡는다. 시작했으면 true
    pub(super) fn maybe_swing(&mut self, world: &World) -> bool {
        let (_, _, down) = self.cursor_play_input();
        if self.swing.above_for < NOTICE_SECS || self.clock < self.swing.ready_at || down || !self.antics_free() || self.swing_blocked(world) {
            return false;
        }
        self.start_swing();
        true
    }

    fn start_swing(&mut self) {
        let hang = behavior::range(&mut self.swing.rng, HANG_SECS);
        let m = &mut self.swing;
        m.aim = self.cursor;
        m.theta = 0.0;
        m.omega = 0.0;
        m.was_down = false;
        m.above_for = 0.0;
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.standing = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.state = State::Swing { phase: SwingPhase::Jump { from: (self.body.x, self.body.y), elapsed: 0.0 }, remaining: hang };
    }

    /// 매달릴 자리 (몸 가운데 x, 발 y): hang_cursor 그림의 손 자리가 커서 끝에 온다
    pub(super) fn swing_spot(&self) -> (f64, f64) {
        let (cx, cy) = self.cursor;
        let (gx, gy) = self.cfg.grip.hang_cursor.at(0.0);
        (cx - gx * self.cfg.size, cy + (1.0 - gy) * self.cfg.size)
    }

    pub(super) fn update_swing(&mut self, dt: f64, world: &World, phase: SwingPhase, remaining: f64) {
        let (_, _, down) = self.cursor_play_input();
        let clicked = down && !self.swing.was_down;
        self.swing.was_down = down;
        if self.swing_blocked(world) {
            self.let_go_swing(None);
            return;
        }
        match phase {
            SwingPhase::Jump { from, elapsed } => {
                let elapsed = elapsed + dt;
                let t = (elapsed / JUMP_SECS).min(1.0);
                let (ax, ay) = self.swing_spot();
                let ease = 1.0 - (1.0 - t) * (1.0 - t);
                let arc = JUMP_ARC * self.cfg.size * (std::f64::consts::PI * t).sin();
                self.body.x = from.0 + (ax - from.0) * ease;
                self.body.y = from.1 + (ay - from.1) * ease - arc;
                if t < 1.0 {
                    self.state = State::Swing { phase: SwingPhase::Jump { from, elapsed }, remaining };
                    return;
                }
                let aim = self.swing.aim;
                if (self.cursor.0 - aim.0).hypot(self.cursor.1 - aim.1) > MISS_BODIES * self.cfg.size {
                    // 헛손질!
                    self.let_go_swing(Some(Speech::Ignore));
                    return;
                }
                self.speech = Some(Speech::Swing);
                self.swing.talk_in = behavior::range(&mut self.swing.rng, TALK_SECS);
                self.state = State::Swing { phase: SwingPhase::Hang, remaining };
            }
            SwingPhase::Hang => {
                let (x, y) = self.swing_spot();
                self.body.x = x;
                self.body.y = y;
                let (vx, vy) = self.swing.vel;
                if (vx.hypot(vy)) / self.cfg.dpi.max(0.01) >= FLING_SPEED {
                    self.fling_from_swing(vx, vy);
                    return;
                }
                if clicked {
                    self.let_go_swing(Some(Speech::Whee));
                    return;
                }
                self.step_pendulum(dt);
                let remaining = remaining - dt;
                if remaining <= 0.0 {
                    self.let_go_swing(Some(Speech::Whee));
                    return;
                }
                let happy = self.swing.omega.abs() >= HAPPY_OMEGA;
                if happy {
                    self.heart_timer = self.heart_timer.max(HEART_SECS * 0.5);
                }
                self.swing.talk_in -= dt;
                if self.swing.talk_in <= 0.0 {
                    self.swing.talk_in = behavior::range(&mut self.swing.rng, TALK_SECS);
                    self.speech = Some(if happy { Speech::More } else { Speech::Swing });
                }
                self.state = State::Swing { phase: SwingPhase::Hang, remaining };
            }
        }
    }

    /// 진자 한 틱: 축(커서 끝)이 가속하면 몸은 반대로 기운다. θ'' = −(g sinθ + aₓ cosθ − a_y sinθ) / L − c θ'
    fn step_pendulum(&mut self, dt: f64) {
        if dt <= 0.0 {
            return;
        }
        let len = PENDULUM_LEN * self.cfg.size;
        let m = &mut self.swing;
        let (ax, ay) = ((m.vel.0 - m.prev_vel.0) / dt, (m.vel.1 - m.prev_vel.1) / dt);
        // 커서 기록이 듬성듬성할 때 튀는 가속은 깎는다
        let cap = 40.0 * GRAVITY;
        let (ax, ay) = (ax.clamp(-cap, cap), ay.clamp(-cap, cap));
        let (s, c) = m.theta.sin_cos();
        let acc = -(GRAVITY * s + ax * c - ay * s) / len - DAMPING * m.omega;
        m.omega += acc * dt;
        m.theta += m.omega * dt;
        let max = MAX_SWING_DEG.to_radians();
        if m.theta.abs() > max {
            m.theta = m.theta.clamp(-max, max);
            m.omega *= -0.4;
        }
    }

    /// 지금 흔들리는 각도 (도, 웹뷰가 손을 축으로 시계 방향으로 돌린다 — 몸이 오른쪽이면 반시계라 −)
    pub(super) fn swing_degrees(&self) -> f64 {
        match self.state {
            State::Swing { phase: SwingPhase::Hang, .. } => -self.swing.theta.to_degrees(),
            _ => 0.0,
        }
    }

    pub(super) fn swing_anim(&self, phase: SwingPhase) -> Anim {
        match phase {
            SwingPhase::Jump { .. } => Anim::Hop,
            SwingPhase::Hang => Anim::HangCursor,
        }
    }

    fn end_swing_common(&mut self) {
        let cooldown = behavior::range(&mut self.swing.rng, COOLDOWN_SECS);
        self.swing.ready_at = self.clock + cooldown;
        self.swing.theta = 0.0;
        self.swing.omega = 0.0;
        self.standing = None;
        self.keep_click_through();
    }

    /// 손을 놓고 떨어진다 (흔들리던 쪽으로 살짝)
    fn let_go_swing(&mut self, speech: Option<Speech>) {
        if speech.is_some() {
            self.speech = speech;
        }
        let len = PENDULUM_LEN * self.cfg.size;
        let (s, c) = self.swing.theta.sin_cos();
        self.body.vx = self.swing.omega * len * c;
        self.body.vy = (-self.swing.omega * len * s).max(0.0);
        self.end_swing_common();
        self.thrown = false;
        self.state = State::Air { anim: Anim::Fall };
    }

    /// 커서를 세게 휘둘렀다: 그 속도로 던져진다 (사람이 던진 것과 같다)
    fn fling_from_swing(&mut self, vx: f64, vy: f64) {
        let (vx, vy) = crate::physics::throw_velocity(vx * FLING_KEEP, vy * FLING_KEEP, self.cfg.dpi, 0.0);
        self.speech = Some(Speech::Whee);
        self.end_swing_common();
        if vx.abs() > 50.0 {
            self.facing = vx.signum();
        }
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown = true;
        self.gentle_drop = false;
        self.thrown_down = visit::aimed_down(vx, vy, self.cfg.dpi);
        self.state = State::Air { anim: Anim::Fall };
    }
}
