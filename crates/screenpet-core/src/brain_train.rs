//! 🦆 오리 기차 (v0.6.3, pet.json `train_leader` — 오리)의 Pet 동작 — brain.rs의 자식 모듈. 줄 세우기는 social_play.rs, 숫자는 play.rs.
//!
//! - 오리가 맨 아래 바닥에서 걷기 시작할 때 쉬는 때가 지났으면 TRAIN_CHANCE로 기차를 부르고 싶어 한다 (train_until).
//!   코디네이터가 같은 바닥의 한가한 친구(내 동물·손님, 최대 넷)를 찾으면 오리는 "Quack! Follow me!" 하고 TRAIN_SECS 동안 걷고 (State::Lead),
//!   친구들은 오리가 지나간 길을 따라 줄줄이 걷는다 (State::Follow — 자리는 코디네이터가 틱마다 set_follow_target으로).
//! - 끝나면 (시간이 다 됐거나 오리가 멈췄다) 다 같이 폴짝 하고 흩어진다.

use super::*;
use crate::play::{self};
use play_moves::BounceThen;

/// 따라가는 친구가 이만큼 (몸 크기 배)보다 뒤처졌으면 달린다
const FOLLOW_RUN_GAP: f64 = 0.35;

impl Pet {
    /// 걷기를 시작할 때 (move_state): 오리가 바닥에서 기차를 부르고 싶어 한다
    pub(super) fn maybe_want_train(&mut self) {
        if !self.cfg.leads_train || self.clock < self.play.train_ready_at || !matches!(self.standing, Some(Standing { owner: Owner::Ground(_), .. })) {
            return;
        }
        if self.play.rng.f64() < play::TRAIN_CHANCE {
            // 걷기 전 잠깐 앉는 박자가 있어도 그 뒤 걷기 시작할 때까지
            self.play.train_until = self.clock + 4.0;
        }
    }

    /// 🦆 기차를 부르고 싶은 오리가 지금 바닥에서 걷고 있다 (코디네이터가 한 번 꺼낸다)
    pub fn take_train_wish(&mut self) -> bool {
        if self.clock >= self.play.train_until || !matches!(self.state, State::Move { kind: Move::Walk, .. }) || !self.play_free() {
            return false;
        }
        self.play.train_until = f64::NEG_INFINITY;
        true
    }

    /// 🦆 따라갈 수 있다: 한가하고 바닥에 서 있다 (나는 동물·자는 동물·든 것이 있는 동물·볼일 있는 동물은 아니다)
    pub fn can_follow(&self) -> bool {
        self.play_free() && !self.cfg.leads_train && self.play.cheeks == 0
    }

    /// 🦆 기차 앞장: secs초 걷는다
    pub fn start_lead(&mut self, secs: f64) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.play.train_ready_at = self.clock + behavior::range(&mut self.play.rng, play::TRAIN_CALM_SECS);
        self.speech = Some(Speech::FollowMe);
        self.state = State::Lead { remaining: secs };
    }

    /// 🦆 기차를 따라가기 시작한다 (자리는 곧 set_follow_target으로 온다)
    pub fn start_follow(&mut self) {
        self.pacing.after_beat = None;
        self.clear_hover();
        // 자리가 오기 전엔 제자리 (FOLLOW_STALE_SECS 안에 오지 않으면 그만둔다)
        self.play.follow = Some((self.body.x, self.facing, self.clock));
        if self.speech.is_none() && self.play.rng.bool() {
            self.speech = Some(Speech::Choo);
        }
        self.state = State::Follow { remaining: play::TRAIN_SECS.1 + 5.0 };
    }

    /// 🦆 기차에서 갈 자리 (x)와 그 자리에서 볼 쪽
    pub fn set_follow_target(&mut self, x: f64, facing: f64) {
        self.play.follow = Some((x, facing, self.clock));
    }

    pub fn is_leading(&self) -> bool {
        matches!(self.state, State::Lead { .. })
    }

    pub fn is_following(&self) -> bool {
        matches!(self.state, State::Follow { .. })
    }

    /// 🦆 기차가 끝났다: 폴짝 하고 흩어진다
    pub fn end_train(&mut self) {
        if matches!(self.state, State::Lead { .. } | State::Follow { .. }) {
            self.play.follow = None;
            self.start_bounce(play::BOUNCE_RISE, BounceThen::Disperse);
        }
    }

    pub(super) fn update_lead(&mut self, dt: f64, world: &World, remaining: f64) {
        if self.romp_interrupted() {
            self.state = self.rest_beat();
            return;
        }
        let remaining = remaining - dt;
        if remaining <= 0.0 {
            self.end_train();
            return;
        }
        let Some(blocked) = self.juggle_walk(world, self.facing, self.walk_speed() * dt) else { return };
        if blocked {
            // 바닥 끝: 돌아선다 (친구들도 그 자리에서 돈다)
            self.facing = -self.facing;
        }
        self.state = State::Lead { remaining };
    }

    pub(super) fn update_follow(&mut self, dt: f64, world: &World, remaining: f64) {
        let stale = self.play.follow.map_or(true, |(_, _, at)| self.clock - at > play::FOLLOW_STALE_SECS);
        if self.romp_interrupted() || stale || remaining <= 0.0 {
            self.play.follow = None;
            self.state = self.rest_beat();
            return;
        }
        let remaining = remaining - dt;
        self.state = State::Follow { remaining };
        let Some((x, face, _)) = self.play.follow else { return };
        let dx = x - self.body.x;
        let run = self.cfg.walk_speed * Move::Run.speed_mult();
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
            if self.juggle_walk(world, dx.signum(), (run * dt).min(dx.abs())).is_none() {
                return;
            }
        } else {
            if !self.follow_anchor(world) {
                self.start_fall();
                return;
            }
            self.facing = face;
        }
        if self.speech.is_none() && self.play.rng.f64() < play::CHOO_RATE * dt {
            self.speech = Some(Speech::Choo);
        }
    }

    /// 따라가다 뒤처져 달리는 중
    pub(super) fn follow_running(&self) -> bool {
        self.play.follow.is_some_and(|(x, _, _)| (x - self.body.x).abs() > FOLLOW_RUN_GAP * self.cfg.size)
    }
}
