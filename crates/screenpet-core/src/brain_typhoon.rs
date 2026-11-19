//! 🌀 태풍에 휘둘리기 (v0.6.8) — brain.rs의 자식 모듈 (비공개 필드에 접근). 바람·회오리 계산은 typhoon.rs, 부르는 곳은 앱(src-tauri/src/weather.rs).
//!
//! - 돌풍 (feel_gust): 걷는 동물은 버팀("hold on!" + 덜덜) / 데굴(Tumble, "whoa!") / 발이 떨어져 붕("wheee!" — 던진 게 아니라 이 화면 안).
//!   나는 동물은 떠밀림. 자는 동물은 덜덜만 (계속 잠)
//! - 회오리 (whirl_to → whirl_release): 매달린 그림(Dragged)으로 앱이 틱마다 정해 주는 자리에 붙들림 (물리 없음).
//!   내던질 때 cross면 던진 비행 — 친구 쪽 끝을 넘으면 손으로 던진 것과 같은 길 (brain_fling.rs → 앱의 own_exited / guest_left)
//! - 앱이 WHIRL_LOST_SECS 동안 자리를 주지 않으면 (태풍이 끝남 등) 그 자리에서 떨어짐 — 잃어버리지 않음. 잡히면 바로 놓임
//! - 하우스 안·들림·창 안·떠나는/들어오는 길·보내는 중·먹는 중·조작 모드·집중·자리 비움·머리 위·물건/동물을 든 흐물이는 그대로

use super::*;
use crate::typhoon::{self, Gust, Stagger};

/// 앱이 이만큼(초) 회오리 자리를 주지 않으면 떨어짐
const WHIRL_LOST_SECS: f64 = 0.3;
/// 한 번 휘청인 뒤 다음 돌풍까지 (초)
const GUST_CALM_SECS: f64 = 1.2;
/// 나는 동물이 떠밀리는 빠르기 (크기 128 기준 논리 px/s, 세기 1일 때 더하는 몫)
const FLY_PUSH: (f64, f64) = (180.0, 320.0);
/// 날아갈 때 위로 더 (blow_velocity의 배)
const BLOWN_LIFT: f64 = 1.6;
/// 회오리에 빨려 들 때 외치는 말
const WHIRL_SHOUTS: [&str; 3] = ["whoa!", "wheee!", "hold on!"];

/// 🌀 태풍의 기억
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StormMood {
    /// 회오리 속이면 마지막으로 자리를 받은 시각 (clock)
    whirl_seen: Option<f64>,
    /// 이 시각(clock)까지는 돌풍에 다시 휘청이지 않음
    calm_until: f64,
}

impl Pet {
    /// 돌풍·회오리가 건드릴 수 있음 (회오리 속이면 놓아줄 일이 생기기 전까지 참)
    pub fn storm_free(&self) -> bool {
        let busy = self.housed
            || self.is_held()
            || self.inside.is_some()
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.send.is_some()
            || self.farewell_pending
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.control.is_some()
            || self.presence_quiet()
            || self.sleeps_for_user()
            || self.in_transit()
            || self.is_on_head()
            || self.celebrating()
            || self.carried_object().is_some()
            || self.hoisting().is_some();
        if busy {
            return false;
        }
        if self.storm.whirl_seen.is_some() {
            return true;
        }
        match self.state {
            State::Move { .. }
            | State::Idle { .. }
            | State::Turn { .. }
            | State::Land { .. }
            | State::Trick { .. }
            | State::Tickled { .. }
            | State::Hungry { .. }
            | State::Huff { .. }
            | State::Special { .. }
            | State::Greet { .. }
            | State::Chase { .. }
            | State::Flee { .. }
            | State::CatchMe { .. }
            | State::Tumble { .. } => self.standing.is_some() || self.cfg.flies,
            // 돌풍에 날아가는 중에도 빨려 듦 (던진 비행은 그대로 둠)
            State::Air { .. } => !self.thrown,
            _ => false,
        }
    }

    /// 🌀 회오리 속에서 도는 중
    pub fn is_whirled(&self) -> bool {
        self.storm.whirl_seen.is_some()
    }

    /// 돌풍이 막 일었음: 버티거나 데굴 구르거나 날아감 (나는 동물은 떠밀림, 자는 동물은 덜덜만). 반응했으면 true
    pub fn feel_gust(&mut self, gust: Gust) -> bool {
        if self.clock < self.storm.calm_until || self.is_whirled() {
            return false;
        }
        let asleep = matches!(self.state, State::Sleep { .. }) && self.standing.is_some() && !self.housed && self.inside.is_none() && !self.is_held();
        if asleep {
            self.storm.calm_until = self.clock + GUST_CALM_SECS;
            self.wobble_timer = stomp_moves::WOBBLE_SECS;
            return true;
        }
        if !self.storm_free() || matches!(self.state, State::Air { .. }) {
            return false;
        }
        self.storm.calm_until = self.clock + GUST_CALM_SECS;
        self.wobble_timer = stomp_moves::WOBBLE_SECS;
        let dir = gust.dir.signum();
        if self.cfg.flies {
            self.pacing.after_beat = None;
            self.clear_hover();
            self.flight = None;
            self.hover_at = None;
            self.standing = None;
            self.thrown = false;
            self.body.vx = dir * self.scaled(FLY_PUSH.0 + FLY_PUSH.1 * gust.strength) * self.cfg.dpi;
            self.body.vy = -40.0 * self.cfg.dpi;
            self.state = State::Air { anim: Anim::Fall };
            self.speech = Some(Speech::Whoa);
            return true;
        }
        match typhoon::gust_reaction(gust.strength, self.rng.f64()) {
            Stagger::Lean => {
                // 바람 쪽을 보고 버팀
                self.facing = dir;
                self.speech_text = Some("hold on!".to_string());
            }
            Stagger::Tumble => {
                self.start_tumble(dir);
                self.speech = Some(Speech::Whoa);
            }
            Stagger::Blown => {
                let (vx, vy) = typhoon::blow_velocity(dir, gust.strength, self.cfg.dpi, self.rng.f64());
                self.pacing.after_beat = None;
                self.clear_hover();
                self.standing = None;
                self.body.vx = vx;
                self.body.vy = vy * BLOWN_LIFT;
                self.facing = dir;
                self.thrown = false;
                self.gentle_drop = false;
                self.no_perch = true;
                self.state = State::Air { anim: Anim::Fall };
                self.speech_text = Some("wheee!".to_string());
            }
        }
        true
    }

    /// 🌀 회오리가 (x, 발 y)에 붙들어 돌림 (틱마다). 처음이면 한마디. 건드릴 수 없으면 false (회오리가 놓아줌)
    pub fn whirl_to(&mut self, x: f64, y: f64) -> bool {
        if !self.storm_free() {
            // 도는 중에 다른 일이 생겼음 (보내기 등): 매달린 그림 그대로 두지 않고 그 자리에서 떨어짐
            if self.storm.whirl_seen.take().is_some() && self.state == (State::Air { anim: Anim::Dragged }) {
                self.state = State::Air { anim: Anim::Fall };
            }
            return false;
        }
        if self.storm.whirl_seen.is_none() {
            self.speech_text = Some(WHIRL_SHOUTS[self.rng.usize(..WHIRL_SHOUTS.len())].to_string());
            self.wobble_timer = stomp_moves::WOBBLE_SECS;
            self.pacing.after_beat = None;
            self.clear_hover();
            self.flight = None;
            self.hover_at = None;
            self.play_target = None;
        }
        self.storm.whirl_seen = Some(self.clock);
        self.standing = None;
        self.thrown = false;
        self.body = Body { x, y, vx: 0.0, vy: 0.0 };
        self.state = State::Air { anim: Anim::Dragged };
        true
    }

    /// 🌀 회오리가 (vx, vy, 물리 px/s)로 내던짐. cross면 던진 비행 — 친구 쪽 끝을 넘어감 (fling_out)
    pub fn whirl_release(&mut self, vx: f64, vy: f64, cross: bool) {
        if self.storm.whirl_seen.take().is_none() {
            return;
        }
        self.standing = None;
        self.body.vx = vx;
        self.body.vy = vy;
        if vx.abs() > 1.0 {
            self.facing = vx.signum();
        }
        self.state = State::Air { anim: Anim::Fall };
        self.thrown = cross;
        self.thrown_down = false;
        self.gentle_drop = false;
        self.no_perch = true;
        self.speech_text = Some("wheee!".to_string());
    }

    /// 🌀 회오리가 내던져 넘길 수 있는 친구 쪽 끝 (왼쪽·오른쪽만): 내 동물은 친구가 연결된 가까운 끝, 손님은 집 쪽만
    pub fn whirl_cross_side(&self, world: &World) -> Option<Side> {
        if self.send.is_some() {
            return None;
        }
        if let Some(g) = self.guest {
            return matches!(g.home, Side::Left | Side::Right).then_some(g.home);
        }
        let b = world.bounds;
        let near_left = self.body.x - b.left <= b.right - self.body.x;
        match (self.exit_edges.left, self.exit_edges.right) {
            (true, true) => Some(if near_left { Side::Left } else { Side::Right }),
            (true, false) => Some(Side::Left),
            (false, true) => Some(Side::Right),
            (false, false) => None,
        }
    }

    /// 매 틱 (주 동작 전): 회오리 속이면 물리 없이 그 자리 (true). 잡혔거나 자리를 오래 못 받았으면 놓임 (떨어짐)
    pub(super) fn update_whirl(&mut self) -> bool {
        let Some(seen) = self.storm.whirl_seen else { return false };
        if self.housed || !matches!(self.state, State::Air { anim: Anim::Dragged }) {
            self.storm.whirl_seen = None;
            return false;
        }
        if self.clock - seen > WHIRL_LOST_SECS {
            self.storm.whirl_seen = None;
            self.start_fall();
            return false;
        }
        true
    }
}
