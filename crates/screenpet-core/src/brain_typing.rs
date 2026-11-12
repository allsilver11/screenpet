//! ⌨️ 타자 응원 (흐물이, v0.6.3 · v0.6.4 키보드) — brain.rs의 자식 모듈.
//!
//! 앱이 매 틱 set_typing_rate(초당 키 수)와 typing_keys(이번 틱에 새로 눌린 수)를 알려 준다
//! (센서는 앱 — 어떤 키인지는 보지도 남기지도 않는다, 넘어오는 건 개수뿐).
//! 초당 TYPING_KPS(4)타 넘게 TYPING_HOLD_SECS(2초) 이어지고 흐물이가 화면에서 쉬거나 걷고 있으면 화면 쪽(앞)을 보고
//! 작은 키보드 앞에 앉아 같이 친다 (type — 사용자가 키를 누를 때마다 손이 번갈아 톡, 멈추면 손도 쉰다: render.tap).
//! 가끔 (YAY_EVERY_SECS마다 YAY_SECS) 두 팔 번쩍 만세 (cheer_type), 가끔 (4~6초) 타자 대사 — TYPING_LINES 차례로, 아주 빠르면 SPEEDY_LINE.
//! 빠른 타자가 이어지는 동안 + AFTER_SECS(2초). 하루 종일은 아니게: BUDGET_WINDOW_SECS(3분)마다 모두 BUDGET_SECS(20초)까지만.

use super::*;

pub const TYPING_KPS: f64 = 4.0;
pub const TYPING_HOLD_SECS: f64 = 2.0;
pub const AFTER_SECS: f64 = 2.0;
pub const BUDGET_SECS: f64 = 20.0;
pub const BUDGET_WINDOW_SECS: f64 = 180.0;
/// 타자 대사 간격
const TALK_SECS: (f64, f64) = (4.0, 6.0);
/// 타자 대사 (차례로) · 아주 빠를 때 (초당 SPEEDY_KPS타 이상)
pub const TYPING_LINES: [&str; 4] = ["Type type!", "So fast!", "Keep going! ⌨️", "Tap tap tap!"];
pub const SPEEDY_LINE: &str = "🔥 Speedy!";
pub const SPEEDY_KPS: f64 = 8.0;
/// 키 한 번에 손 하나가 톡 (이만큼 눌린 그림) — 그림이 보여 줄 수 있는 만큼만: 초당 MAX_TAPS_PER_SEC번까지
pub const TAP_SECS: f64 = 0.09;
pub const MAX_TAPS_PER_SEC: f64 = 12.0;
/// 밀린 키는 이만큼만 기억한다 (손을 뗐는데 한참 더 치지 않게)
const MAX_QUEUED_TAPS: u32 = 2;
/// 가끔 두 팔 번쩍 만세
pub const YAY_EVERY_SECS: f64 = 6.0;
pub const YAY_SECS: f64 = 0.6;

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
    /// 다음 대사 차례 (TYPING_LINES)
    line: usize,
    /// 아직 치지 않은 키 수 (개수뿐, MAX_QUEUED_TAPS까지)
    queued: u32,
    /// 지금 톡 하는 손 (-1 화면 왼쪽 · +1 오른쪽)과 남은 시간, 마지막 톡 뒤 지난 시간
    hand: i8,
    tap_left: f64,
    since_tap: f64,
    /// 다음 만세까지 · 만세 남은 시간
    yay_in: f64,
    yay_left: f64,
}

impl Pet {
    /// ⌨️ 초당 키 수 (앱이 매 틱 update 전에 — 흐물이만 쓴다). 음수·NaN은 0
    pub fn set_typing_rate(&mut self, keys_per_sec: f64) {
        self.typing.rate = if keys_per_sec.is_finite() { keys_per_sec.max(0.0) } else { 0.0 };
    }

    /// ⌨️ 이번 틱에 새로 눌린 키 수 (앱이 매 틱 update 전에 — 개수뿐). 응원하는 동안만 손이 따라 친다
    pub fn typing_keys(&mut self, presses: u32) {
        let t = &mut self.typing;
        t.queued = t.queued.saturating_add(presses).min(MAX_QUEUED_TAPS);
    }

    pub(super) fn track_typing(&mut self, dt: f64) {
        let cheering = matches!(self.state, State::TypeCheer { .. });
        let t = &mut self.typing;
        t.fast_for = if t.rate >= TYPING_KPS { t.fast_for + dt } else { 0.0 };
        if !cheering {
            // 응원 밖에서 누른 키는 모아 두지 않는다
            t.queued = 0;
            t.tap_left = 0.0;
            t.yay_left = 0.0;
        }
    }

    /// 응원 중의 그림: 키보드 앞 타자 (type) / 가끔 만세 (cheer_type)
    pub(super) fn type_cheer_anim(&self) -> Anim {
        if self.typing.yay_left > 0.0 {
            Anim::CheerType
        } else {
            Anim::Type
        }
    }

    /// 지금 톡 하는 손 (render.tap: 0 = 두 손 쉼, -1 화면 왼쪽 손, +1 오른쪽 손)
    pub(super) fn typing_tap(&self) -> i8 {
        let t = &self.typing;
        if matches!(self.state, State::TypeCheer { .. }) && t.yay_left <= 0.0 && t.tap_left > 0.0 {
            t.hand
        } else {
            0
        }
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
        let t = &mut self.typing;
        t.queued = 0;
        t.tap_left = 0.0;
        t.since_tap = f64::INFINITY;
        t.yay_left = 0.0;
        t.yay_in = YAY_EVERY_SECS;
        self.say_typing_line();
        self.state = State::TypeCheer { remaining: AFTER_SECS };
        true
    }

    /// 타자 대사 하나 (차례로, 아주 빠르면 🔥) — 다음 대사는 4~6초 뒤
    fn say_typing_line(&mut self) {
        let t = &mut self.typing;
        let line = if t.rate >= SPEEDY_KPS {
            SPEEDY_LINE
        } else {
            t.line += 1;
            TYPING_LINES[(t.line - 1) % TYPING_LINES.len()]
        };
        t.rng_state = t.rng_state.wrapping_add(1);
        let mut rng = fastrand::Rng::with_seed(t.rng_state ^ 0x7479_7065);
        t.talk_in = behavior::range(&mut rng, TALK_SECS);
        self.speech_text = Some(line.to_string());
    }

    /// 키 하나에 톡 하나 (손을 번갈아), 초당 MAX_TAPS_PER_SEC번까지 · 가끔 만세
    fn step_typing_hands(&mut self, dt: f64) {
        let t = &mut self.typing;
        t.tap_left -= dt;
        t.since_tap += dt;
        if t.yay_left > 0.0 {
            t.yay_left -= dt;
            t.queued = 0;
            return;
        }
        t.yay_in -= dt;
        if t.yay_in <= 0.0 {
            t.yay_in = YAY_EVERY_SECS;
            t.yay_left = YAY_SECS;
            t.tap_left = 0.0;
            t.queued = 0;
            return;
        }
        if t.queued > 0 && t.since_tap + 1e-9 >= 1.0 / MAX_TAPS_PER_SEC {
            t.queued -= 1;
            t.hand = if t.hand < 0 { 1 } else { -1 };
            t.tap_left = TAP_SECS;
            t.since_tap = 0.0;
        }
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
        self.step_typing_hands(dt);
        self.typing.talk_in -= dt;
        if self.typing.talk_in <= 0.0 {
            self.say_typing_line();
        }
        self.state = State::TypeCheer { remaining };
    }
}
