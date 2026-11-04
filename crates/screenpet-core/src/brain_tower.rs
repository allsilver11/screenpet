//! 친구 머리 위에 서기 (탑 쌓기) — brain.rs의 자식 모듈 (비공개 필드에 접근). 누가 어디에 서고 언제 끊기는지는 social_tower.rs.
//!
//! - 머리 위에 있는 동안 State::OnHead: 스스로 움직이지 않고, 코디네이터가 매 틱 아래 친구 머리로 옮겨 준다 (Social::carry). 똥·인사·커서 놀이 같은 일은 하지 않는다.
//! - 맨 위 동물만 제 시간(RIDE_SECS)이 되면 폴짝 내려온다. 아래 동물은 시간이 지나도 위 친구가 내려갈 때까지 기다리지만, RIDE_MAX_SECS가 되면 무조건 내려온다.
//! - 먹이·집·집 생각·보내기·자리 비움·집중이 오면 그 자리에서 떨어지고, 조작 모드에서 키를 누르면 폴짝 내려온다. 잡으면 평소처럼 들린다.
//! - 떨어지거나 내려오면 평소 공중 물리로 진짜 디딤판에 내려선다. 탑에서 떨어지는 동안엔 다른 머리에 다시 서지 않는다 (no_perch).
//! - 아래쪽 동물: 걷고 쉬고 자던 그대로다. 말풍선과 덜덜 한 번 (자는 동물은 그대로 잔다).

use super::*;

/// 머리 위에 서 있는 시간 (초) — 맨 위 동물이 이만큼 있다가 내려온다
pub const RIDE_SECS: (f64, f64) = (4.0, 8.0);
/// 어떤 동물도 이보다 오래 머리 위에 있지 않는다 (초). 5층 탑이 위에서부터 다 내려오는 시간보다 길다
pub const RIDE_MAX_SECS: f64 = 30.0;
/// 코디네이터가 이만큼 알려 주지 않으면 (사라졌다) 스스로 떨어진다 (초)
const RIDE_LOST_SECS: f64 = 0.25;
/// 뛰는 동물이 친구 너머로 더 가는 거리: 달리는 속도로 이만큼 더 간 셈이다 (초)
const RUN_CARRY_SECS: f64 = 0.5;
/// 내려올 때 화면 끝이 이만큼(몸 배) 가까우면 안쪽으로 뛴다
const HOP_EDGE_BODIES: f64 = 1.5;

impl Pet {
    /// 친구 머리 위에 서 있다
    pub fn is_on_head(&self) -> bool {
        matches!(self.state, State::OnHead { .. })
    }

    /// 머리 위에 친구가 서 있다
    pub fn has_rider(&self) -> bool {
        self.rider_on
    }

    pub fn set_has_rider(&mut self, on: bool) {
        self.rider_on = on;
    }

    /// 머리 꼭대기 자리 (발 기준, 위에 설 동물의 발이 닿는 곳)
    pub fn head_top(&self) -> (f64, f64) {
        (self.body.x, self.body.y - meet_moves::HEAD_RISE * self.cfg.size - self.hop_lift())
    }

    /// 지금 발밑 모니터 작업 영역의 위 끝 (탑 높이 제한)
    pub fn ceiling(&self) -> f64 {
        self.ceiling
    }

    /// 달리는 중이면 뛰어 내릴 때 더 가는 거리
    pub fn run_carry(&self) -> f64 {
        if self.is_running() { self.cfg.walk_speed * Move::Run.speed_mult() * RUN_CARRY_SECS } else { 0.0 }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Run, .. })
    }

    /// 머리 위에 친구를 태우고 있을 수 있는 때: 디딤판 위에서 평소 하던 일을 하거나, 자기도 머리 위에 있다
    pub fn can_carry(&self) -> bool {
        if self.cfg.flies
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
        match self.state {
            State::OnHead { .. } => true,
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
            | State::Flee { .. } => self.standing.is_some(),
            _ => false,
        }
    }

    /// 떨어지는 중이라 머리 위에 내려설 수 있다 (폴짝 뛰는 중·탑에서 떨어지는 중·아래 친구를 노린 던지기는 아니다)
    /// 사용자가 던지지 않고 그냥 놓아 떨어지는 중
    pub fn is_gentle_drop(&self) -> bool {
        self.gentle_drop && self.thrown
    }

    pub fn can_perch(&self) -> bool {
        matches!(self.state, State::Air { anim: Anim::Fall })
            && !self.cfg.flies
            && !self.no_perch
            && !self.thrown_down
            && !self.housed
            && self.inside.is_none()
            && self.home_trip.is_none()
            && self.send.is_none()
            && !self.farewell_pending
    }

    /// 머리 위에 선다 (자리는 코디네이터가 옮겨 준다)
    pub fn begin_ride(&mut self) {
        let remaining = behavior::range(&mut self.rng, RIDE_SECS);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.meet = None;
        self.thrown = false;
        self.thrown_down = false;
        self.drop_check = false;
        self.standing = None;
        self.flight = None;
        self.hover_at = None;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.ride_seen = self.clock;
        self.state = State::OnHead { remaining, age: 0.0 };
    }

    /// 머리에 올라왔다 (이번 틱 자리)
    pub fn ride_at(&mut self, x: f64, y: f64) {
        self.body.x = x;
        self.body.y = y;
        self.ride_seen = self.clock;
    }

    /// 코디네이터: 아직 탑에 있다
    pub fn keep_riding(&mut self) {
        self.ride_seen = self.clock;
    }

    /// 친구가 머리 위에 올라왔다: 깨어 있으면 짧은 반응 (자는 동물은 그대로 잔다)
    pub fn feel_rider(&mut self) {
        self.rider_on = true;
        if !matches!(self.state, State::Sleep { .. }) {
            self.wobble_timer = stomp_moves::WOBBLE_SECS;
            if self.speech.is_none() {
                self.speech = Some(Speech::Oof);
            }
        }
    }

    /// 뛰어올라 섰다
    pub fn say_on_top(&mut self) {
        self.speech = Some(Speech::Whee);
    }

    /// 탑에서 떨어진다: 평소 낙하로 진짜 디딤판까지 (머리 위라면 이 동물은 잡혔거나 먼저 떠나지 않았다)
    pub fn fall_off(&mut self) {
        if self.is_on_head() {
            self.start_fall();
            self.no_perch = true;
        }
    }

    /// 폴짝 내려온다 (화면 끝이 가까우면 안쪽으로)
    fn hop_off(&mut self, world: &World) {
        let b = world.bounds;
        let edge = HOP_EDGE_BODIES * self.cfg.size;
        let dir = if self.body.x - b.left < edge {
            1.0
        } else if b.right - self.body.x < edge {
            -1.0
        } else {
            self.facing
        };
        self.facing = dir;
        self.no_perch = true;
        self.start_hop(dir);
    }

    fn ride_must_end(&self) -> bool {
        self.cfg.flies
            || self.housed
            || self.inside.is_some()
            || self.food.is_some()
            || self.meal_pending.is_some()
            || self.home_trip.is_some()
            || self.return_home.is_some()
            || self.hand.is_some()
            || self.send.is_some()
            || self.farewell_pending
            || self.is_heading_out()
            || self.presence_quiet()
    }

    /// 머리 위의 한 틱: 끝날 때가 아니면 그대로 (자리는 코디네이터가)
    pub(super) fn update_on_head(&mut self, dt: f64, world: &World, remaining: f64, age: f64) {
        let (remaining, age) = (remaining - dt, age + dt);
        if self.ride_must_end() || self.clock - self.ride_seen > RIDE_LOST_SECS {
            self.fall_off();
        } else if age >= RIDE_MAX_SECS || self.control_pressed() || (remaining <= 0.0 && !self.rider_on) {
            self.hop_off(world);
        } else {
            self.state = State::OnHead { remaining, age };
        }
    }

    /// 머리로 뛰어오르는 중이다 (웅크리기 포함)
    pub fn is_climbing(&self) -> bool {
        matches!(self.state, State::Meet { phase: meet_moves::MeetPhase::Crouch | meet_moves::MeetPhase::ToHead, .. }) && self.meet.is_some_and(|m| m.is_climb())
    }

    /// 올라가는 머리가 움직였다: 그쪽으로 길을 고친다
    pub fn aim_climb(&mut self, head: (f64, f64)) {
        let Some(plan) = self.meet.as_mut().filter(|m| m.is_climb()) else { return };
        plan.set_head(head);
        if let State::Meet { phase: meet_moves::MeetPhase::ToHead, remaining } = self.state {
            self.body.vx = (head.0 - self.body.x) / remaining.max(1e-3);
        }
    }
}

#[cfg(test)]
impl Pet {
    pub(crate) fn expire_ride(&mut self) {
        if let State::OnHead { age, .. } = self.state {
            self.state = State::OnHead { remaining: 0.0, age };
        }
    }

    pub(crate) fn ride_clock(&self) -> Option<(f64, f64)> {
        match self.state {
            State::OnHead { remaining, age } => Some((remaining, age)),
            _ => None,
        }
    }

    pub(crate) fn offer_food_test(&mut self) {
        self.hand = Some((self.body.x, self.body.y));
    }
}
