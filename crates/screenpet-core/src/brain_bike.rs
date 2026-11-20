//! 🦇 박쥐인간 오토바이·박쥐 신호 (pet.json `bike_rider` + BIKE_ANIMS — PetConfig::rides_bike) — brain.rs의 자식 모듈. 숫자는 bike.rs.
//!
//! - 🏍️ 걷기·달리기 토막마다 RIDE_SHARE로 오토바이 (bike_pace — 토막 안에서는 그대로): 빠르게 달리고 (ride), 가끔 윌리 (wheelie, "yeah!").
//!   물건을 들고 있으면 안 탐. 토막이 끝나면 가끔 끼익 드리프트로 멈춤 (drift).
//! - 디딤판 끝 (bike_edge): 붕 뛰어내리거나 (State::Air + bike_jump 그림) 드리프트로 멈춰 돌아섬.
//! - 앞의 동물에 부딪치면 그 동물이 살짝 튕김 (web.yank_pet → sync_lifts가 be_yanked — 거미인간 끌어오기와 같은 길).
//! - 🚪 친구 화면으로 나가고 들어올 때 오토바이를 탐 (bike_transit — Exit/Enter 그림 ride, 오토바이 빠르기). 들어오면 계속 달림 (force_ride).
//! - 🔦 박쥐 신호 (State::Signal): 몇 분에 한 번 서서 바닥의 조명을 켬 — take_bat_signal로 앱에 한 번 (덮개 창이 빛줄기·박쥐 무늬를 그림).
//! - 💪 거대한 것도 늘 번쩍 (brain_juggle.rs가 rides_bike를 봄 — 짧게 힘주고 "easy!").
//!
//! 주사위는 자기 주사위(BikeMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::bike;
pub use crate::bike::SIGNAL_SECS;

/// 🔦 앱에 알리는 박쥐 신호 한 번 (물리 px): 조명 렌즈 자리 (x, y)에서 위로 secs초 동안 — size = 몸 크기
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatSignal {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub secs: f64,
}

/// 오토바이·박쥐 신호의 기억
#[derive(Debug, Clone)]
pub struct BikeMood {
    rng: fastrand::Rng,
    /// 이번 이동 토막은 오토바이
    riding: bool,
    /// 마지막으로 본 이동 틱 (clock, 이동 종류 번호, 남은 시간) — 새 토막인지 볼 때
    last: Option<(f64, u8, f64)>,
    /// 다음 이동 토막은 반드시 탐 (친구 화면에서 타고 들어옴 — 테스트도 씀)
    pub(super) force_ride: bool,
    /// 이 시각(clock)까지 윌리
    wheelie_until: f64,
    /// 오토바이째 뛰어내리는 중 (공중 그림 bike_jump)
    pub(super) jumping: bool,
    /// 이 시각(clock)까지 드리프트로 멈춰 있음
    drift_until: f64,
    /// 🔦 이 시각(clock)부터 박쥐 신호를 켤 수 있음
    pub(super) signal_ready_at: f64,
    /// 🔦 새 신호 (take_bat_signal로 한 번)
    signal: Option<BatSignal>,
    /// 부딪쳐 튕긴 동물과 다시 튕길 수 있는 시각
    bumped: Vec<(u64, f64)>,
}

impl BikeMood {
    /// 시드로 난수를 따로 둠. 박쥐 신호는 SIGNAL_FIRST_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        BikeMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x6269_6b65),
            riding: false,
            last: None,
            force_ride: false,
            wheelie_until: 0.0,
            jumping: false,
            drift_until: 0.0,
            signal_ready_at: bike::SIGNAL_FIRST_SECS,
            signal: None,
            bumped: Vec::new(),
        }
    }
}

impl Pet {
    /// 오토바이 빠르기 (px/s)
    pub(super) fn bike_speed(&self) -> f64 {
        self.cfg.walk_speed * bike::BIKE_SPEED
    }

    /// 지금 오토바이를 타고 달리는 중 (이번 틱의 이동 토막이 오토바이, 또는 막 타고 들어와 다음 토막을 탈 참)
    pub(super) fn riding_now(&self) -> bool {
        self.cfg.rides_bike
            && matches!(self.state, State::Move { .. })
            && (self.bike.riding && self.bike.last.is_some_and(|(c, ..)| c == self.clock) || self.bike.force_ride)
    }

    /// 달리는 그림: 윌리 중이면 wheelie, 아니면 ride
    pub(super) fn ride_anim(&self) -> Anim {
        if self.clock < self.bike.wheelie_until {
            Anim::Wheelie
        } else {
            Anim::Ride
        }
    }

    /// 드리프트로 멈춰 있는 중 (쉬기·돌아서기 그림 대신 drift)
    pub(super) fn drifting(&self) -> bool {
        self.cfg.rides_bike && self.clock < self.bike.drift_until && self.standing.is_some()
    }

    /// 친구 화면으로 오토바이 타고 나가거나 들어오는 중 (옆 끝만, 손에 든 것이 없을 때)
    pub(super) fn bike_transit(&self) -> bool {
        self.cfg.rides_bike
            && matches!(self.state, State::Exit { side, .. } | State::Enter { side, .. } if !visit::is_vertical(side))
            && self.carried_object().is_none()
            && self.hoisting().is_none()
    }

    /// update_ground 이동 한 틱: 이번 토막이 오토바이면 그 빠르기 (None이면 평소 걸음). 새 토막이면 탈지 정하고, 타는 동안 윌리·부딪치기
    pub(super) fn bike_pace(&mut self, kind: Move, dt: f64) -> Option<f64> {
        if !self.cfg.rides_bike || kind == Move::Roll || self.cfg.hops || self.carried_object().is_some() {
            self.bike.riding = false;
            return None;
        }
        let remaining = match self.state {
            State::Move { remaining, .. } => remaining,
            _ => 0.0,
        };
        let clock = self.clock;
        let same = self.bike.last.is_some_and(|(c, k, r)| c < clock && clock - c <= dt * 1.5 && k == kind as u8 && remaining <= r);
        if !same {
            let b = &mut self.bike;
            b.riding = b.force_ride || b.rng.f64() < bike::RIDE_SHARE;
            b.force_ride = false;
            b.wheelie_until = 0.0;
            if b.riding && b.rng.f64() < bike::VROOM_CHANCE {
                self.speech = Some(Speech::Vroom);
            }
        }
        self.bike.last = Some((clock, kind as u8, remaining));
        if !self.bike.riding {
            return None;
        }
        if clock >= self.bike.wheelie_until && self.bike.rng.f64() < bike::WHEELIE_RATE * dt {
            self.bike.wheelie_until = clock + behavior::range(&mut self.bike.rng, bike::WHEELIE_SECS);
            self.speech = Some(Speech::Wheelie);
        }
        self.bump_ahead();
        Some(self.bike_speed())
    }

    /// 달리는 앞 (같은 바닥, 몸 반 폭 + 그 동물 크기 BUMP_REACH 안)의 동물을 살짝 튕김 (sync_lifts가 be_yanked)
    fn bump_ahead(&mut self) {
        let Some(st) = self.standing else { return };
        let clock = self.clock;
        self.bike.bumped.retain(|&(_, until)| clock < until);
        let (x, y, f, half) = (self.body.x, self.body.y, self.facing, self.cfg.half_w);
        let bumped = &self.bike.bumped;
        let hit = self.web.pets.iter().find(|s| {
            let dx = (s.x - x) * f;
            s.liftable
                && s.owner == Some(st.owner)
                && (s.y - y).abs() <= bike::BUMP_LEVEL_PX
                && dx > 0.0
                && dx <= half + bike::BUMP_REACH * s.size
                && !bumped.iter().any(|&(k, _)| k == s.key)
        });
        if let Some(s) = hit {
            let (key, dpi) = (s.key, self.cfg.dpi);
            self.web.yank_pet = Some((key, f * bike::BUMP_VX * dpi, -bike::BUMP_VY * dpi));
            self.bike.bumped.push((key, clock + bike::BUMP_COOLDOWN_SECS));
        }
    }

    /// 오토바이로 달리다 디딤판 끝 (Edge)에 닿음: 붕 뛰어내리거나 드리프트로 멈춰 돌아섬. 오토바이가 아니면 false (평소대로)
    pub(super) fn bike_edge(&mut self) -> bool {
        if !self.riding_now() {
            return false;
        }
        self.bike.riding = false;
        self.bike.wheelie_until = 0.0;
        if self.bike.rng.f64() < bike::EDGE_JUMP_CHANCE {
            let dir = self.facing;
            self.start_hop(dir);
            self.body.vx = dir * self.body.vx.abs().max(self.bike_speed() * bike::EDGE_JUMP_VX);
            self.bike.jumping = true;
        } else {
            self.pacing.after_beat = None;
            self.bike.drift_until = self.clock + bike::DRIFT_SECS;
            self.state = State::Turn { remaining: bike::DRIFT_SECS };
        }
        self.speech = Some(Speech::BikeTrick);
        true
    }

    /// 이동 토막이 끝남: 오토바이였으면 가끔 끼익 드리프트로 멈춤
    pub(super) fn bike_stop(&mut self) {
        if self.riding_now() && self.bike.rng.f64() < bike::STOP_DRIFT_CHANCE {
            self.bike.drift_until = self.clock + bike::DRIFT_SECS;
            self.speech = Some(Speech::BikeTrick);
        }
        self.bike.riding = false;
    }

    /// 친구 화면으로 나가기 시작함: 오토바이에 올라 "vroom!"
    pub(super) fn bike_exit(&mut self) {
        if self.bike_transit() {
            self.speech = Some(Speech::Vroom);
        }
    }

    /// 친구 화면에서 들어옴: 들어온 뒤 첫 토막도 오토바이
    pub(super) fn bike_enter(&mut self) {
        if self.cfg.rides_bike {
            self.bike.force_ride = true;
        }
    }

    /// 🔦 박쥐 신호를 켬 (서 있을 때만). 켰으면 true
    pub(super) fn start_signal(&mut self) -> bool {
        if !self.cfg.rides_bike || self.standing.is_none() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.bike.signal_ready_at = self.clock + behavior::range(&mut self.bike.rng, bike::SIGNAL_COOLDOWN_SECS);
        let (left, top) = self.window_pos();
        let size = self.cfg.size;
        let fx = if self.facing >= 0.0 { bike::SIGNAL_FROM.0 } else { 1.0 - bike::SIGNAL_FROM.0 };
        self.bike.signal = Some(BatSignal { x: left + fx * size, y: top + bike::SIGNAL_FROM.1 * size, size, secs: SIGNAL_SECS });
        self.speech = Some(Speech::BatSignal);
        self.state = State::Signal { remaining: SIGNAL_SECS };
        true
    }

    /// 🔦 새 박쥐 신호를 한 번 돌려줌 (앱이 덮개 창에 빛줄기를 그림)
    pub fn take_bat_signal(&mut self) -> Option<BatSignal> {
        self.bike.signal.take()
    }

    /// 매 틱 (줄 놀이 앞): 박쥐 신호 중이면 그 한 틱, 아니면 가끔 켬. 처리했으면 true
    pub(super) fn update_bike(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.rides_bike {
            return false;
        }
        if !matches!(self.state, State::Air { .. }) {
            self.bike.jumping = false;
        }
        if let State::Signal { remaining } = self.state {
            if !self.follow_anchor(world) {
                self.start_fall();
                return true;
            }
            let remaining = remaining - dt;
            self.state = if remaining > 0.0 { State::Signal { remaining } } else { self.rest_beat() };
            return true;
        }
        if self.clock < self.bike.signal_ready_at || !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        if self.bike.rng.f64() >= bike::SIGNAL_RATE * dt {
            return false;
        }
        self.start_signal()
    }
}
