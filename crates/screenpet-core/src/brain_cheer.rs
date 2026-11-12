//! 🔄 새 버전이 나왔음 (앱이 업데이트 확인에서 처음 찾았을 때 부름) — brain.rs의 자식 모듈.
//!
//! "new version!!" 하고 10초쯤 신나게 이리저리 뛰어다님: 1~2초마다 (또는 벽·끝에서) 방향을 바꾸며 달리기(Move::Run) —
//! 깡총 동물은 깡총깡총, 나는 동물은 이리저리 남. 끝나면 평소대로 돌아감
//! - 손님은 하지 않음 (cheer가 false).
//! - 지금 할 수 없으면 (자리 비움·집중·하우스 안·들림·사용자 볼일·떠나고 들어오는 길·공중 등) 미뤄 두었다가 할 수 있게 되면 함
//!   보통의 낮잠은 깨워도 됨 (자리 비움 잠은 자리 비움이라 미룸).
//! - 하는 도중에 들리거나 볼일이 생기면 그만둠. 친구 화면으로 걸어 나가지 않음

use super::*;

/// 한 번 달리는 길이 (초) — 끝나면 돌아서 반대로
const LEG_SECS: (f64, f64) = (1.0, 2.0);
/// 달리기 상태의 남은 시간: 끝은 이쪽(leg)이 정함
const RUN_HOLD_SECS: f64 = 100.0;
/// 나는 동물이 한 번에 날아가는 위아래 폭 (몸 배)
const FLY_DY: f64 = 0.8;

/// 새 버전 응원의 기억: 미뤄 둔 응원, 하는 중인 응원
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct CheerMood {
    /// 아직 시작하지 못한 응원 (초)
    pending: Option<f64>,
    /// 하는 중: 남은 시간, 이번 달리기의 남은 시간
    active: Option<(f64, f64)>,
}

impl Pet {
    /// 🔄 새 버전! — 내 동물만 (손님이면 false). 지금 못 하면 할 수 있을 때 함. 이미 하는 중이거나 기다리는 중이면 그대로
    pub fn cheer(&mut self, secs: f64) -> bool {
        if self.guest.is_some() {
            return false;
        }
        if self.cheer.active.is_none() && self.cheer.pending.is_none() {
            self.cheer.pending = Some(secs);
        }
        true
    }

    /// 새 버전 응원 중 (기다리는 중은 아님)
    pub fn is_cheering(&self) -> bool {
        self.cheer.active.is_some()
    }

    /// 지금 응원하며 뛰어다니는 중임
    pub(super) fn cheering(&self) -> bool {
        self.cheer.active.is_some()
    }

    /// 지금은 응원할 수 없음 (하던 것은 그만, 기다리던 것은 계속 기다림)
    fn cheer_blocked(&self) -> bool {
        self.guest.is_some()
            || self.is_user_away()
            || self.focusing()
            || self.housed
            || self.is_held()
            || self.has_errand()
            || self.in_transit()
            || self.celebrating()
            || self.rider_on
            || matches!(self.state, State::Away | State::OnHead { .. })
    }

    /// 새로 달리기를 시작해도 되는 상태 (벽에서 돌아서기·뛰어내린 뒤 착지 같은 건 끝나기를 기다림)
    fn cheer_free(&self) -> bool {
        let settled = self.cfg.flies || self.standing.is_some();
        settled
            && matches!(
                self.state,
                State::Idle { .. }
                    | State::Move { .. }
                    | State::Sleep { .. }
                    | State::Trick { .. }
                    | State::Special { .. }
                    | State::Huff { .. }
                    | State::Hungry { .. }
                    | State::Annoyed { .. }
                    | State::Tickled { .. }
            )
    }

    /// 응원 달리기(Move::Run) 중임
    fn is_cheer_run(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Run, .. })
    }

    /// update에서 (자리 비움 전): 응원을 시작하고, 달리기를 이어 가고, 끝냄. 움직임 자체는 평소 길(update_ground·update_flyer)이 함
    pub(super) fn update_cheer(&mut self, dt: f64, world: &World) {
        if let Some(secs) = self.cheer.pending {
            if self.guest.is_some() {
                self.cheer.pending = None;
            } else if !self.cheer_blocked() && self.cheer_free() {
                self.cheer.pending = None;
                self.speech = Some(Speech::NewVersion);
                self.cheer.active = Some((secs, 0.0));
                self.cheer_leg(world, false);
            }
            return;
        }
        let Some((remaining, leg)) = self.cheer.active else { return };
        if self.cheer_blocked() {
            self.cheer.active = None;
            return;
        }
        let remaining = remaining - dt;
        if remaining <= 0.0 {
            self.end_cheer();
            return;
        }
        let leg = leg - dt;
        self.cheer.active = Some((remaining, leg));
        if self.is_cheer_run() {
            // 나는 동물은 다 날아가면 쉬기로 바뀌어 아래에서 새로 난다. 걷는 동물은 시간이 되면 돌아선다
            if leg <= 0.0 && !self.cfg.flies {
                self.cheer_leg(world, true);
            }
        } else if self.cheer_free() {
            // 벽에서 돌아섰거나 뛰어내려 착지한 뒤: 그 방향 그대로 다시 달린다 (나는 동물은 다 날아왔으니 돌아서)
            self.cheer_leg(world, self.cfg.flies);
        }
    }

    /// 한 번 달리기 (turn = 반대로 돌아서)
    fn cheer_leg(&mut self, world: &World, turn: bool) {
        if turn {
            self.facing = -self.facing;
        }
        let secs = behavior::range(&mut self.rng, LEG_SECS);
        self.pacing.after_beat = None;
        self.gaze_up = false;
        self.clear_hover();
        if let Some((remaining, _)) = self.cheer.active {
            self.cheer.active = Some((remaining, secs));
        }
        if self.cfg.flies {
            let b = self.fly_limits(world);
            let reach = self.cfg.walk_speed * crate::flight::speed_mult(Move::Run) * secs;
            // 그쪽으로 넉넉히 갈 곳이 없으면 반대로
            let room = if self.facing > 0.0 { b.right - self.body.x } else { self.body.x - b.left };
            if room < reach * 0.5 {
                self.facing = -self.facing;
            }
            let dy = (self.rng.f64() * 2.0 - 1.0) * FLY_DY * self.cfg.size;
            let (x, y) = crate::flight::clamp(&b, self.body.x + self.facing * reach, self.body.y + dy);
            self.flight = None;
            self.start_flight(crate::flight::Goal::Point(x, y), Move::Run);
            return;
        }
        self.state = State::Move { kind: Move::Run, remaining: RUN_HOLD_SECS };
    }

    /// 응원을 끝냄. 달리던 중이면 평소 자세로 돌아감
    fn end_cheer(&mut self) {
        self.cheer.active = None;
        if self.is_cheer_run() {
            self.flight = None;
            self.state = self.idle_state();
        }
    }
}
