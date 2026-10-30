//! 트레이 "놀러 보내기" (phase 3): 내 동물이 그 친구가 연결된 화면 끝(side)으로 곧장 가서 걸어 나간다.
//! 손님도 이 화면 주인이 "<친구>에게 보내기"로 보내면 같은 길로 그 끝으로 간다 ("bye~!").
//! 가는 길은 집이 그리운 손님과 같다 (`is_heading_out` — 기어오르기·창 안 놀기·큰 점프 없이 그쪽으로, 창 모서리에선 뛰어내림).
//! SEND_GIVE_UP_SECS 안에 못 나가면 그 자리에서 그쪽으로 걸어 나간다. brain.rs의 자식 모듈.

use super::*;

/// 이만큼(초) 안에 그 끝으로 못 나가면 그 자리에서 그쪽으로 걸어 나간다
pub const SEND_GIVE_UP_SECS: f64 = 12.0;
/// 보낸 순간 그쪽으로 달려가는 시간 (그 뒤로는 평소처럼 고르되 늘 그쪽으로)
const SEND_DASH_SECS: (f64, f64) = (3.0, 5.0);

/// 친구 화면으로 보내는 중: 나갈 끝과 남은 시간
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SendTrip {
    pub side: Side,
    pub remaining: f64,
}

impl Pet {
    /// side 끝(친구가 연결된 끝)으로 보낸다 — 내 동물, 또는 다음 친구에게 보내는 손님 ("bye~!").
    /// 하우스 안·들림·떠나는 중이면 false
    pub fn send_to(&mut self, side: Side) -> bool {
        if self.housed || self.home_trip.is_some() || self.is_held() || self.in_transit() {
            return false;
        }
        self.send = Some(SendTrip { side, remaining: SEND_GIVE_UP_SECS });
        if self.guest.is_some() {
            self.speech = Some(Speech::Bye);
        }
        // 창 안에 있었으면 곧 나온다 (update_inside가 is_heading_out을 본다)
        true
    }

    /// 친구가 데려가고 싶어 할 때 갈 수 있음: 내 동물이고, 하우스 안·집에 가는 중·먹는 중(먹으러 나옴 포함)·성장 축하 중·
    /// 들림·떠나는 중이 아니고, 이미 보내는 중도 아님
    pub fn is_free_to_go(&self) -> bool {
        self.guest.is_none()
            && !self.housed
            && self.home_trip.is_none()
            && self.return_home.is_none()
            && !self.is_eating()
            && !self.celebrating()
            && !self.is_held()
            && !self.in_transit()
            && self.send.is_none()
    }

    /// 보내기를 그만둔다 (그 친구가 끊겼을 때 등)
    pub fn cancel_send(&mut self) {
        self.send = None;
    }

    /// 지금 보내는 중인 끝
    pub fn sending(&self) -> Option<Side> {
        self.send.map(|s| s.side)
    }

    /// 곧장 가는 끝: 보냈으면 그 끝, 집이 그리운 손님이면 집 쪽
    pub(super) fn heading(&self) -> Option<Side> {
        match (self.send, self.guest) {
            (Some(s), _) => Some(s.side),
            (None, Some(g)) if g.is_homesick() => Some(g.home),
            _ => None,
        }
    }

    /// 화면 끝 edge에 닿았을 때 걸어 나갈 확률: 보냈으면 그 끝에서만 (반드시), 아니면 visit 규칙
    pub(super) fn exit_odds(&mut self, edge: Side) -> f64 {
        match self.send {
            Some(s) if s.side == edge => 1.0,
            Some(_) => 0.0,
            None => visit::exit_chance(edge, self.exit_edges, self.guest),
        }
    }

    /// 매 틱 (주 동작 전): 끝났으면 지우고, 쉬는 중이면 그쪽으로 움직이게 하고, 시간이 다 되면 그 자리에서 걸어 나간다
    pub(super) fn tick_send(&mut self, dt: f64, world: &World) {
        let Some(mut trip) = self.send else { return };
        // 나갔거나(떠나는 중) 하우스로 가게 됐으면 끝
        if self.leaving() || self.housed || self.home_trip.is_some() {
            self.send = None;
            return;
        }
        trip.remaining -= dt;
        self.send = Some(trip);
        let grounded = self.standing.is_some() || self.cfg.flies;
        let calm = matches!(
            self.state,
            State::Idle { .. } | State::Sleep { .. } | State::Hungry { .. } | State::Trick { .. } | State::Huff { .. } | State::Special { .. }
        );
        if !grounded || self.is_held() || self.inside.is_some() {
            return;
        }
        let free = calm || matches!(self.state, State::Move { .. } | State::Turn { .. } | State::Land { .. });
        if trip.remaining <= 0.0 && free {
            // 길이 없다 — 그 자리에서 그쪽으로 걸어 나간다
            self.flight = None;
            self.facing = visit::dir(trip.side);
            self.start_exit(trip.side);
            return;
        }
        // 쉬거나 자는 중(박자 사이 아님)이면 바로 그쪽으로 움직인다
        if calm && self.pacing.after_beat.is_none() {
            self.facing = visit::dir(trip.side);
            if self.cfg.flies {
                self.state = self.move_state(false, world);
            } else {
                let remaining = behavior::range(&mut self.rng, SEND_DASH_SECS);
                self.state = State::Move { kind: Move::Run, remaining };
            }
        }
    }
}
