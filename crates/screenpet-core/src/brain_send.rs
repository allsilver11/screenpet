//! 트레이 "놀러 보내기" / "편지 들려 보내기" / 친구의 "펫 데리고 오기" (phase 3): 보낸 동물은 화면 끝으로 뛰어가지 않고
//! 그 자리에 멈춰 이쪽을 보고 손 흔든 뒤("bye~!", 편지를 들었으면 "💌!") 뿅 사라진다 — 사라지는 쪽(take_exit)은 그 친구가 연결된 끝.
//! 손님도 이 화면 주인이 "<친구>에게 보내기"로 보내면 같은 길로 사라진다.
//! 들려 있거나 공중·창 안·먹는 중이면 그게 끝난 뒤에 손 흔든다. 던지기와 저절로 걸어 나가기는 그대로 (brain.rs). brain.rs의 자식 모듈.

use super::*;

/// 손 흔드는 시간 (초)
pub const SEND_WAVE_SECS: f64 = 1.2;
/// 그 뒤 "뿅" (웹뷰가 작아지며 흐려지고 반짝이 몇 개 — RenderState.poof) 시간 (초)
pub const SEND_POOF_SECS: f64 = 0.4;

/// 친구 화면으로 보내는 중: 사라질 끝, 편지를 들었는지
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SendTrip {
    pub side: Side,
    pub letter: bool,
    /// 손 흔들 때 말풍선 없이 (앱이 이미 다른 말풍선을 띄웠다 — "펫 데리고 오기"의 "off to <친구>'s~ brb!")
    pub quiet: bool,
}

impl Pet {
    /// side 끝(친구가 연결된 끝)으로 보낸다 — 내 동물, 또는 다음 친구에게 보내는 손님. letter = 편지를 들려 보냄 ("💌!").
    /// 하우스 안·들림·떠나는 중(손 흔드는 중 포함)이면 false
    pub fn send_to(&mut self, side: Side, letter: bool) -> bool {
        if self.housed || self.home_trip.is_some() || self.is_held() || self.in_transit() {
            return false;
        }
        self.send = Some(SendTrip { side, letter, quiet: false });
        true
    }

    /// 보낸 동물이 손 흔들 때 "bye~!"를 하지 않는다 (앱이 띄운 말풍선을 덮지 않게)
    pub fn hush_send(&mut self) {
        if let Some(trip) = self.send.as_mut() {
            trip.quiet = true;
        }
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

    /// 보내기를 그만둔다 (그 친구가 끊겼을 때 등). 손 흔들던 중이면 그만 흔들고 그 자리에 남는다
    pub fn cancel_send(&mut self) {
        if self.send.take().is_some() && matches!(self.state, State::Farewell { .. }) {
            self.state = State::Idle { remaining: behavior::range(&mut self.rng, behavior::IDLE_SECS), look: false };
        }
    }

    /// 지금 보내는 중인 끝
    pub fn sending(&self) -> Option<Side> {
        self.send.map(|s| s.side)
    }

    /// 보내져 손 흔드는 중 (손님의 "집에 와" 손 흔들기는 아니다)
    pub(super) fn waving_goodbye(&self) -> bool {
        self.send.is_some() && matches!(self.state, State::Farewell { .. })
    }

    /// 손 흔들기 마지막 SEND_POOF_SECS: 웹뷰가 "뿅" 하고 사라지는 모습을 그린다.
    /// 화면을 떠난 뒤(Away)에도 참 — 창이 숨기 전에 사라진 몸이 번쩍 다시 보이지 않게. 그 밖의 모든 상태(취소돼 남음, 돌아옴)는 거짓 → 웹뷰가 바로 다시 보인다
    pub(super) fn poofing(&self) -> bool {
        match self.state {
            State::Farewell { remaining } => self.send.is_some() && remaining <= SEND_POOF_SECS,
            State::Away => true,
            _ => false,
        }
    }

    /// 곧장 가는 끝: 보냈으면 그 끝, 집이 그리운 손님이면 집 쪽 (보낸 동물이 손 흔들기를 기다리는 동안 — 창에서 나오기 등)
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

    /// 매 틱 (주 동작 전): 끝났으면 지우고, 손 흔들 수 있게 되면(내려서 있고, 들리거나 창 안·먹는 중이 아님) 그 자리에서 손 흔든다
    pub(super) fn tick_send(&mut self, _dt: f64, _world: &World) {
        let Some(trip) = self.send else { return };
        // 손 흔드는 중 — update_farewell이 끝나면 trip.side로 사라진다
        if matches!(self.state, State::Farewell { .. }) {
            return;
        }
        // 나갔거나(떠나는 중) 하우스로 가게 됐으면 끝
        if self.leaving() || self.housed || self.home_trip.is_some() {
            self.send = None;
            return;
        }
        let grounded = self.standing.is_some() || self.cfg.flies;
        let busy = matches!(
            self.state,
            State::Eating { .. } | State::LevelUp { .. } | State::Enter { .. } | State::Leap { .. } | State::Sting { .. } | State::ToFood { .. } | State::WantFood { .. }
        );
        if !grounded || self.is_held() || self.inside.is_some() || busy {
            return;
        }
        self.flight = None;
        self.pacing.after_beat = None;
        self.before_press = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        if !trip.quiet {
            self.speech = Some(if trip.letter { Speech::Letter } else { Speech::Bye });
        }
        self.state = State::Farewell { remaining: SEND_WAVE_SECS + SEND_POOF_SECS };
    }
}
