//! 공룡 쿵쾅에 흔들리기 (v0.3.6) — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 공룡이 제자리 쿵쾅쿵쾅(Special::Stomp)을 시작하면 take_stomp가 한 번 true — social.rs가 같은 화면의 동물 중
//! 같은 바닥(같은 디딤판, 같은 높이) 몸 STOMP_BODIES개 안의 친구에게 feel_stomp를 부른다 (나는 동물은 직선 거리).
//!
//! - 흔들림: 스프라이트가 WOBBLE_SECS 덜덜 (RenderState.wobble) + "whoa!".
//! - 데굴 (코디네이터가 TUMBLE_CHANCE로 정함): 디딤판 위 걷는 동물만 — 구르기 그림으로 공룡 반대쪽으로 조금 미끄러졌다가
//!   (디딤판 끝에선 멈춘다 — 이미 끝을 넘어 있으면 더 나가지 않는다) 착지 자세로 일어선다.
//! - 나는 동물(꿀벌·유령)은 떠서 흔들리기만, 자던 동물은 투덜대며("...") 깼다가 다시 잠든다.
//! - 들려 있거나·먹는 중·하우스 안·떠나 있음·공중·벽 타기 등은 그대로. 한 번 흔들리면 STOMP_CALM_SECS 동안은 다시 흔들리지 않는다.

use super::*;

/// 흔들리는 시간 (앱의 덜덜 떨기 길이)
pub const WOBBLE_SECS: f64 = 0.4;
/// 데굴 넘어져 있는 시간
pub const TUMBLE_SECS: (f64, f64) = (0.8, 1.2);
/// 데굴 넘어지며 미끄러지는 처음 속도 (크기 128 기준 px/s, 점점 느려진다)
pub const TUMBLE_SPEED: f64 = 110.0;
/// 한 번 흔들린 뒤 다시 흔들리기까지 (초)
pub const STOMP_CALM_SECS: f64 = 10.0;
/// 자다 깨어 투덜대는 시간 (그 뒤 다시 잠든다)
const WAKE_GRUMPY_SECS: f64 = 1.5;

impl Pet {
    /// 공룡이 막 쿵쾅을 시작했으면 true (한 번만)
    pub fn take_stomp(&mut self) -> bool {
        std::mem::take(&mut self.stomp_out)
    }

    /// 지금 쿵쾅에 흔들리는 중
    pub fn wobbling(&self) -> bool {
        self.wobble_timer > 0.0
    }

    /// 쿵쾅을 느낄 수 있는 상태: 화면 위에 나와 있고, 들리거나 먹거나 공중·벽에 있지 않고, 막 흔들리지 않았음
    fn stomp_feels(&self) -> bool {
        if self.clock < self.stomp_calm_until
            || self.housed
            || self.is_held()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.home_trip.is_some()
        {
            return false;
        }
        if self.cfg.flies && self.standing.is_none() {
            // 떠서 날고 있으면 (손님 놀이·인사 포함) 흔들리기만 — 쏘거나 떠나는 중은 그대로
            return !matches!(self.state, State::Sting { .. } | State::Exit { .. } | State::Enter { .. } | State::Farewell { .. } | State::Away);
        }
        self.standing.is_some()
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

    /// 쿵쾅을 느꼈다 (from_x: 공룡 자리). tumble이면 (걷는 동물이 디딤판 위에 있을 때) 데굴 넘어진다. 반응했으면 true
    pub fn feel_stomp(&mut self, from_x: f64, tumble: bool) -> bool {
        if !self.stomp_feels() {
            return false;
        }
        self.stomp_calm_until = self.clock + STOMP_CALM_SECS;
        if matches!(self.state, State::Sleep { .. } | State::Annoyed { .. }) {
            // 쿵! 에 깨어 투덜 — 그러고는 다시 잠든다
            self.speech = Some(Speech::Ignore);
            self.state = State::Annoyed { remaining: WAKE_GRUMPY_SECS, tickled_for: 0.0 };
            self.wobble_timer = WOBBLE_SECS;
            return true;
        }
        self.wobble_timer = WOBBLE_SECS;
        if self.speech.is_none() {
            self.speech = Some(Speech::Whoa);
        }
        // 나는 동물(꿀벌·유령)은 내려앉아 있어도 흔들리기만
        let flying = self.cfg.flies;
        // 창 안에서 노는 중엔 창 안 길을 따라가야 해서 흔들리기만, 공룡끼리도 서로 흔들리기만 (같이 데굴 구르면 끝이 없다)
        if tumble && !flying && self.inside.is_none() && !matches!(self.state, State::Special { .. }) {
            let dx = self.body.x - from_x;
            let dir = if dx.abs() > 1.0 { dx.signum() } else { -self.facing };
            self.pacing.after_beat = None;
            self.clear_hover();
            self.state = State::Tumble { remaining: behavior::range(&mut self.rng, TUMBLE_SECS), dir };
        }
        true
    }

    /// 데굴: 디딤판을 따라가며 dir 쪽으로 점점 느리게 미끄러진다 — 조각 끝에서 멈춘다. 끝나면 착지 자세로 일어선다
    pub(super) fn update_tumble(&mut self, dt: f64, world: &World, remaining: f64, dir: f64) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let st = self.standing.expect("follow_anchor keeps standing");
        let x = self.body.x;
        if let Some(piece) = world.piece_at(st.owner, x).copied() {
            let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
            // 이미 끝을 넘어 서 있으면 그 자리보다 더 나가지 않는다
            let (lo, hi) = (lo.min(x), hi.max(x));
            let fade = (remaining / TUMBLE_SECS.1).clamp(0.0, 1.0);
            let speed = TUMBLE_SPEED * self.cfg.size / 128.0 * fade;
            let new_x = (x + dir * speed * dt).clamp(lo, hi);
            self.standing = Some(Standing { owner: st.owner, rel_x: st.rel_x + (new_x - x) });
            self.body.x = new_x;
        }
        let remaining = remaining - dt;
        self.state = if remaining > 0.0 { State::Tumble { remaining, dir } } else { State::Land { remaining: LAND_SECS } };
    }
}

#[cfg(test)]
impl Pet {
    /// 테스트: 지금 쿵쾅을 시작한다 (공룡 동작 고르기 없이)
    pub(crate) fn force_stomp(&mut self) {
        self.stomp_out = true;
        self.state = State::Special { kind: Special::Stomp, remaining: 2.0 };
    }
}
