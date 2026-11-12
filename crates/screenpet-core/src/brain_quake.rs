//! 🦖 공룡 큰 쿵! (v0.6.3) — brain.rs의 자식 모듈. 숫자는 play.rs, 예전의 쿵쾅 흔들기는 brain_stomp.rs.
//!
//! 공룡이 쿵쾅(Special::Stomp)을 시작할 때 쉬는 때(QUAKE_CALM_SECS)가 지났으면 큰 쿵임: "STOMP!" —
//! 같은 바닥(같은 디딤판, 같은 높이) 몸 QUAKE_BODIES개 안의 물건이 톡 튀어 오르고 (ObjectPlay::Quake → 앱이 Object::pop),
//! 친구들은 깜짝 폴짝 ("Whoa!" — social.rs가 feel_big_stomp를 부름). 자던 친구는 투덜대며 깼다가 다시 잠듦
//! 사이의 쿵쾅은 예전처럼 흔들기만. 자리 비움·집중 중엔 큰 쿵을 하지 않음

use super::*;
use crate::play::{self, ObjectPlay};

impl Pet {
    /// 쿵쾅을 시작함: 큰 쿵이면 앱과 친구들에게 알림
    pub(super) fn begin_stomp(&mut self) {
        self.play.big_stomp = false;
        let Some(st) = self.standing else { return };
        if self.clock < self.play.quake_ready_at || self.presence_quiet() || self.is_user_away() {
            return;
        }
        self.play.big_stomp = true;
        self.play.quake_ready_at = self.clock + behavior::range(&mut self.play.rng, play::QUAKE_CALM_SECS);
        self.speech = Some(Speech::BigStomp);
        self.play.event = Some(ObjectPlay::Quake { x: self.body.x, y: self.body.y, owner: st.owner, reach: play::QUAKE_BODIES * self.cfg.size });
    }

    /// 막 시작한 쿵쾅이 큰 쿵임 (social.rs가 take_stomp 뒤에 봄)
    pub fn stomp_is_big(&self) -> bool {
        self.play.big_stomp
    }

    /// 큰 쿵을 느꼈음 (from_x: 공룡 자리): 걷는 동물은 깜짝 폴짝, 나는 동물은 흔들리기만, 자던 동물은 투덜대며 깼다가 다시 잠듦. 반응했으면 true
    pub fn feel_big_stomp(&mut self, from_x: f64) -> bool {
        if matches!(self.state, State::Sleep { .. } | State::Annoyed { .. }) || self.cfg.flies || self.inside.is_some() {
            // 예전 쿵쾅과 같다 (데굴은 없다)
            return self.feel_stomp(from_x, false);
        }
        if !self.stomp_feels() {
            return false;
        }
        self.stomp_calm_until = self.clock + stomp_moves::STOMP_CALM_SECS;
        self.wobble_timer = stomp_moves::WOBBLE_SECS;
        self.speech = Some(Speech::Whoa);
        self.start_bounce(play::QUAKE_HOP_RISE, play_moves::BounceThen::Rest);
        true
    }
}
