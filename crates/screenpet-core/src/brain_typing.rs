//! ⌨️ 타자 응원 (흐물이, v0.6.3) — brain.rs의 자식 모듈.
//!
//! 앱이 매 틱 set_typing_rate(초당 키 수)를 알려 준다 (센서는 앱 — 어떤 키인지는 보지도 남기지도 않는다).
//! 초당 TYPING_KPS(4)타 넘게 TYPING_HOLD_SECS(2초) 이어지고 흐물이가 화면에서 쉬거나 걷고 있으면 화면 쪽(앞)을 보고 응원한다
//! (cheer_type — 주먹 번쩍 / 허공 타자), 가끔 "Cheer up!" / "You got this!". 빠른 타자가 이어지는 동안 + AFTER_SECS(2초).
//! 하루 종일은 아니게: BUDGET_WINDOW_SECS(3분)마다 모두 BUDGET_SECS(20초)까지만.

use super::*;

pub const TYPING_KPS: f64 = 4.0;
pub const TYPING_HOLD_SECS: f64 = 2.0;
pub const AFTER_SECS: f64 = 2.0;
pub const BUDGET_SECS: f64 = 20.0;
pub const BUDGET_WINDOW_SECS: f64 = 180.0;
/// "Cheer up!" 간격
const FIGHTING_SECS: (f64, f64) = (4.0, 7.0);

#[derive(Debug, Clone, Default)]
pub struct TypingMood {
    /// 앱이 알려 준 초당 키 수
    rate: f64,
    /// 빠른 타자가 이어진 시간
    fast_for: f64,
    /// 이번 3분 창이 시작된 시각과 그 안에서 응원한 시간
    window_start: Option<f64>,
    used: f64,
    talk_in: f64,
    rng_state: u64,
}

impl Pet {
    /// ⌨️ 초당 키 수 (앱이 매 틱 update 전에 — 흐물이만 쓴다). 음수·NaN은 0
    pub fn set_typing_rate(&mut self, keys_per_sec: f64) {
        self.typing.rate = if keys_per_sec.is_finite() { keys_per_sec.max(0.0) } else { 0.0 };
    }

    pub(super) fn track_typing(&mut self, dt: f64) {
        let t = &mut self.typing;
        t.fast_for = if t.rate >= TYPING_KPS { t.fast_for + dt } else { 0.0 };
    }

    /// 이번 3분 창에 응원할 시간이 남았다 (창이 지났으면 새로 연다)
    fn typing_budget_left(&mut self) -> bool {
        let clock = self.clock;
        let t = &mut self.typing;
        if t.window_start.is_none_or(|s| clock - s >= BUDGET_WINDOW_SECS) {
            t.window_start = Some(clock);
            t.used = 0.0;
        }
        t.used < BUDGET_SECS
    }

    pub(super) fn maybe_type_cheer(&mut self) -> bool {
        if self.typing.fast_for < TYPING_HOLD_SECS || !self.antics_free() || !self.typing_budget_left() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::Fighting);
        self.typing.talk_in = self.next_fighting();
        self.state = State::TypeCheer { remaining: AFTER_SECS };
        true
    }

    fn next_fighting(&mut self) -> f64 {
        let t = &mut self.typing;
        t.rng_state = t.rng_state.wrapping_add(1);
        let mut rng = fastrand::Rng::with_seed(t.rng_state ^ 0x7479_7065);
        behavior::range(&mut rng, FIGHTING_SECS)
    }

    pub(super) fn update_type_cheer(&mut self, dt: f64, world: &World, remaining: f64) {
        if self.antics_interrupted() {
            self.state = self.idle_state();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        self.typing.used += dt;
        let remaining = if self.typing.rate >= TYPING_KPS { AFTER_SECS } else { remaining - dt };
        if remaining <= 0.0 || self.typing.used >= BUDGET_SECS {
            self.state = self.rest_beat();
            return;
        }
        self.typing.talk_in -= dt;
        if self.typing.talk_in <= 0.0 {
            self.typing.talk_in = self.next_fighting();
            self.speech = Some(Speech::Fighting);
        }
        self.state = State::TypeCheer { remaining };
    }
}
