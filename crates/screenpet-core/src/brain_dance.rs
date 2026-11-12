//! 🕺 연타 춤 (흐물이, v0.6.3) — brain.rs의 자식 모듈.
//!
//! 흐물이를 CLICK_WINDOW(1.5초) 안에 CLICKS(4)번 넘게 클릭하면 (끌기는 세지 않음) 춤을 춤 (dance — 앞모습).
//! 춤추는 동안 더 클릭하면 더 신나게: 8번째부터 dance2, 12번째부터 반짝이까지 (render.sparkle).
//! 마지막 클릭 뒤 DANCE_SECS(3초) + 단계마다 1.5초 (최대 MAX_SECS 6초) 동안 춤
//! 처음 세 번의 클릭은 평소 반응 그대로 (폴짝·애교 — 폴짝 중의 클릭도 셈: 내려서면 춤).
//! 자는 흐물이를 깨우는 클릭, 배고플 때의 클릭(음식 고르기)은 그대로임

use std::collections::VecDeque;

use super::*;

/// 이만큼 클릭하면 춤 (CLICK_WINDOW초 안에)
pub const CLICKS: usize = 4;
/// 클릭을 세는 시간 창 (초)
pub const CLICK_WINDOW: f64 = 1.5;
/// 마지막 클릭 뒤 추는 시간 (단계마다 STEP_SECS 더, 최대 MAX_SECS)
pub const DANCE_SECS: f64 = 3.0;
/// 춤 단계마다 더 추는 시간 (초)
pub const STEP_SECS: f64 = 1.5;
/// 춤을 추는 최대 시간 (초)
pub const MAX_SECS: f64 = 6.0;
/// 춤 단계가 오르는 클릭 수 (한 번 춤에서 모두 센 것): 8 → dance2, 12 → 반짝이
pub const LEVEL_CLICKS: [usize; 2] = [8, 12];

/// 연타 춤의 기억: 최근 클릭 시각, 받은 클릭 수, 단계
#[derive(Debug, Clone, Default)]
pub struct DanceMood {
    clicks: VecDeque<f64>,
    /// 이번 춤에서 받은 클릭 수 · 단계 (1..=3)
    count: usize,
    /// 지금 춤 단계 (0 = 추지 않음)
    pub(super) level: u32,
    /// 공중에서 네 번째 클릭을 받았음: 이 시각까지 내려서면 춤
    pending_until: f64,
}

impl Pet {
    /// pointer_up에서 클릭(끌기 아님) 하나 — 춤이 시작되거나 이어지면 true (평소 클릭 반응은 하지 않음)
    pub(super) fn dance_click(&mut self, grounded: bool) -> bool {
        if !self.cfg.antics {
            return false;
        }
        let clock = self.clock;
        let m = &mut self.dance;
        m.clicks.push_back(clock);
        while m.clicks.front().is_some_and(|&t| clock - t > CLICK_WINDOW) {
            m.clicks.pop_front();
        }
        let dancing = matches!(self.before_press, Some((State::Dance { .. }, _)));
        if dancing && grounded {
            m.count += 1;
            self.start_dance_level();
            return true;
        }
        if m.clicks.len() < CLICKS {
            return false;
        }
        m.count = m.clicks.len();
        if grounded {
            self.start_dance_level();
            self.speech = Some(Speech::Dance);
            true
        } else {
            m.pending_until = clock + CLICK_WINDOW;
            false
        }
    }

    /// 받은 클릭 수로 단계를 정하고 춤을 (다시) 시작함
    fn start_dance_level(&mut self) {
        let m = &mut self.dance;
        m.level = 1 + LEVEL_CLICKS.iter().filter(|&&n| m.count >= n).count() as u32;
        let secs = (DANCE_SECS + STEP_SECS * f64::from(m.level - 1)).min(MAX_SECS);
        self.pacing.after_beat = None;
        self.before_press = None;
        self.state = State::Dance { remaining: secs };
    }

    /// 공중에서 춤 클릭을 받았으면 내려서는 대로 (update_antics)
    pub(super) fn maybe_dance_now(&mut self) -> bool {
        if self.dance.pending_until <= self.clock || self.standing.is_none() || !matches!(self.state, State::Land { .. } | State::Idle { .. } | State::Move { .. } | State::Turn { .. }) {
            return false;
        }
        self.dance.pending_until = 0.0;
        self.start_dance_level();
        self.speech = Some(Speech::Dance);
        true
    }

    /// 춤 한 틱: 방해받으면 그만, 시간이 다 되면 끝냄
    pub(super) fn update_dance(&mut self, dt: f64, world: &World, remaining: f64) {
        if self.antics_interrupted() {
            self.end_dance();
            return;
        }
        if !self.follow_anchor(world) {
            self.dance.level = 0;
            self.start_fall();
            return;
        }
        let remaining = remaining - dt;
        if remaining <= 0.0 {
            self.end_dance();
        } else {
            self.state = State::Dance { remaining };
        }
    }

    /// 춤을 끝내고 클릭 기억을 지움
    fn end_dance(&mut self) {
        self.dance.level = 0;
        self.dance.count = 0;
        self.dance.clicks.clear();
        self.state = self.rest_beat();
    }

    /// 춤 그림: 2단계부터 dance2
    pub(super) fn dance_anim(&self) -> Anim {
        if self.dance.level >= 2 {
            Anim::Dance2
        } else {
            Anim::Dance
        }
    }

    /// 가장 신나는 춤 — 웹뷰가 반짝이를 뿌림
    pub(super) fn dance_sparkle(&self) -> bool {
        matches!(self.state, State::Dance { .. }) && self.dance.level >= 3
    }
}
