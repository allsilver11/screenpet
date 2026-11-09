//! 날아온 것에 부딪히기 — brain.rs의 자식 모듈 (비공개 필드에 접근). 판정과 쿨다운은 social.rs, 숫자와 되튀는 계산은 collide.rs.
//!
//! - 던진 동물: 옆으로 부딪히면 "ting!" 하고 맞은 쪽 반대로 되튀고, 머리에 떨어지면 "boing!" 하고 튀어 오른다. 던진 비행은 이어지지만 (느려져서 화면 끝을 못 넘을 수 있다) 아래 친구를 노린 것은 잊는다.
//! - 맞은 동물: 빠르면 데굴 (brain_stomp.rs Tumble), 느리면 움찔 + "ow!", 머리를 맞으면 납작 (brain_meet.rs Squash), 나는 동물은 떠밀려 갔다가 멈춘 자리에 뜬다.
//! - 잘 놀라는 동물(레서판다)은 느리게 맞으면 움찔 대신 깜짝 벌떡 (Special::Surprise, "!!").
//! - 똥에 맞으면 반응은 같고 말만 "ew!".
//! - 조작 모드: 걷는 동물은 빠르면 데굴만 (납작은 하지 않는다), 나는 동물은 덜덜만 — 잠깐 넘어지는 것 말고 키를 뺏지 않는다.
//! - 들려 있거나, 집·창 안, 떠나거나 들어오는 길, 먹는 중, 자리 비움·집중, 공중·벽, 뛰어넘는 중은 맞지 않고 그대로 지나간다.

use super::meet_moves::MeetPhase;
use super::*;
use crate::collide::{self, Face, Hit, HIT_CALM_SECS};

/// 나는 동물이 떠밀리는 속도 (부딪힌 것의 속도 배)
const FLY_PUSH: f64 = 0.6;
/// 머리 위에서 맞아 떨어질 때 밀리는 속도 (부딪힌 것의 속도 배)
const KNOCK_OFF: f64 = 0.3;

impl Pet {
    /// 손에서 던져져 날아가는 중 (창 안에서 노는 중은 빼고): 다른 동물에 부딪힐 수 있다
    pub fn is_flung(&self) -> bool {
        self.thrown && matches!(self.state, State::Air { .. }) && self.inside.is_none() && !self.housed
    }

    /// 날아온 것에 맞을 수 있는 때 (아니면 그대로 지나간다)
    pub fn hit_feels(&self) -> bool {
        if self.clock < self.hit_calm_until
            || self.housed
            || self.inside.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.send.is_some()
            || self.farewell_pending
            || self.is_heading_out()
            || self.presence_quiet()
        {
            return false;
        }
        self.is_on_head()
            || (self.standing.is_some() || self.cfg.flies)
            && matches!(
                self.state,
                State::Move { .. }
                    | State::Idle { .. }
                    | State::Turn { .. }
                    | State::Land { .. }
                    | State::Trick { .. }
                    | State::Tickled { .. }
                    | State::Hungry { .. }
                    | State::Huff { .. }
                    | State::Special { .. }
                    | State::Sleep { .. }
                    | State::Annoyed { .. }
                    | State::Greet { .. }
                    | State::Chase { .. }
                    | State::Flee { .. }
                    | State::CatchMe { .. }
            )
    }

    /// 던진 내가 부딪혔다: 되튄다 (away = 맞은 몸에서 멀어지는 쪽)
    pub fn bounce_off(&mut self, face: Face, away: f64) {
        let (vx, vy) = collide::rebound(face, self.body.vx, self.body.vy, away, self.cfg.dpi);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown_down = false;
        self.speech = Some(if face == Face::Top { Speech::Boing } else { Speech::Ting });
    }

    /// 날아온 것에 맞았다
    pub fn feel_hit(&mut self, hit: Hit) {
        self.hit_calm_until = self.clock + HIT_CALM_SECS;
        self.wobble_timer = stomp_moves::WOBBLE_SECS;
        let controlled = self.control.is_some();
        let speech = if self.is_on_head() {
            // 머리 위의 친구: 옆에서 세게 맞으면 떨어지고 (위 층도 같이), 아니면 덜덜만
            if hit.fast && hit.face != Face::Top {
                self.fall_off();
                self.body.vx = hit.vx * KNOCK_OFF;
                Speech::Whoa
            } else if hit.face == Face::Top {
                Speech::Oof
            } else {
                Speech::Ow
            }
        } else if !hit.fast && matches!(self.state, State::Sleep { .. } | State::Annoyed { .. }) {
            self.state = State::Annoyed { remaining: stomp_moves::WAKE_GRUMPY_SECS, tickled_for: 0.0 };
            Speech::Ow
        } else if self.cfg.flies {
            if !controlled {
                self.push_flier(hit);
            }
            if hit.fast { Speech::Whoa } else { Speech::Ow }
        } else if hit.face == Face::Top && !controlled {
            self.squash();
            Speech::Oof
        } else if hit.fast {
            self.start_tumble(hit.dir);
            Speech::Whoa
        } else if !controlled && self.startle() {
            // 잘 놀라는 동물 (레서판다): 살짝 부딪혀도 깜짝 벌떡
            Speech::Surprise
        } else {
            Speech::Ow
        };
        self.speech = Some(if hit.poop { Speech::Ew } else { speech });
    }

    /// 떠밀려 간다: 던져진 나는 동물처럼 느려지다 멈춘 자리에 뜬다 (던진 비행이 아니라서 화면 끝을 넘거나 다른 동물을 치지 않는다)
    fn push_flier(&mut self, hit: Hit) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.flight = None;
        self.hover_at = None;
        self.standing = None;
        self.body.vx = hit.vx * FLY_PUSH;
        self.body.vy = hit.vy * FLY_PUSH;
        self.thrown = false;
        self.state = State::Air { anim: Anim::Fall };
    }

    fn squash(&mut self) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.meet = None;
        self.state = State::Meet { phase: MeetPhase::Squash, remaining: meet_moves::SQUASH_SECS };
    }
}

#[cfg(test)]
impl Pet {
    /// 테스트: 지금 자리에서 (vx, vy)로 던져져 날아간다
    pub(crate) fn force_throw(&mut self, vx: f64, vy: f64) {
        self.standing = None;
        self.body.vx = vx;
        self.body.vy = vy;
        self.state = State::Air { anim: Anim::Fall };
        self.thrown = true;
        self.thrown_down = false;
        self.gentle_drop = false;
    }

    pub(crate) fn force_throw_down(&mut self, vx: f64, vy: f64) {
        self.force_throw(vx, vy);
        self.thrown_down = true;
    }

    pub(crate) fn exit_edges_test(&mut self, edges: ExitEdges) {
        self.exit_edges = edges;
    }

    pub(crate) fn aims_down(&self) -> bool {
        self.thrown_down
    }
}
