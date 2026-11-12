//! ⚾ 캐치볼 (흐물이, v0.6.3) — brain.rs의 자식 모듈. 숫자는 juggle.rs (MAX_RALLY·CATCH_SECS·CHASE_SECS·CATCH_REACH).
//!
//! 친구 화면 끝에서 물건이 날아 들어오면 (Object::flown_in — 앱이 틱마다 notice_incoming으로 넘긴다) 바닥에서 손이 빈 흐물이는
//! 그것이 머리 위 높이로 내려올 자리를 셈해 달려가 (JugglePhase::Chase) 머리 위로 받는다 (Catch — JuggleEvent::Catch로 앱이 손에 쥐여 준다).
//! 몇 번째인지 외친다 ("3!" — take_speech_text). 그리고 날아온 끝으로 되던진다 (Plan::Lob — 그 끝 가까이 달려가 모니터 너머로):
//! 놓을 때 take_rally가 (id, 주고받은 횟수 + 1)을 한 번 돌려준다 — 앱은 그 물건에 set_rally로 적어 친구에게 보내는 Toss에 싣는다.
//! MAX_RALLY(20)에 닿으면 "끝!" 하고 되던지지 않는다 (그 자리에서 휙). 거대한 것은 받지 않는다. 늦어서 못 받으면 그냥 그만둔다.

use super::*;
use crate::juggle::{self, Incoming, JuggleEvent};

#[derive(Debug, Clone, Default)]
pub struct RallyMood {
    /// 받으러 가는 물건의 지금 자리 (notice_incoming이 채운다)
    target: Option<Incoming>,
    /// 받았고 되던질 물건과 그때 실을 횟수
    throwing: Option<(u64, u32)>,
    /// 방금 되던졌다 (take_rally로 한 번)
    out: Option<(u64, u32)>,
}

impl Pet {
    /// ⚾ 친구 화면에서 날아 들어오는 물건들 (매 틱 update 전에 — 받는 흐물이만 일이 있다)
    pub fn notice_incoming(&mut self, incoming: &[Incoming], world: &World) {
        if !self.cfg.antics || !self.cfg.juggler {
            return;
        }
        if let State::Juggle { phase: JugglePhase::Chase { id, .. }, t } = self.state {
            self.rally.target = incoming.iter().find(|o| o.id == id).copied();
            if let Some(x) = self.rally.target.and_then(|o| self.catch_x(&o, world)) {
                self.state = State::Juggle { phase: JugglePhase::Chase { id, x }, t };
            }
            return;
        }
        if !self.free_hands() {
            return;
        }
        let pick = incoming.iter().filter(|o| !o.giant).filter_map(|o| self.catch_x(o, world).map(|x| (*o, x))).min_by(|a, b| (a.1 - self.body.x).abs().total_cmp(&(b.1 - self.body.x).abs()));
        let Some((o, x)) = pick else { return };
        self.rally.target = Some(o);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.state = State::Juggle { phase: JugglePhase::Chase { id: o.id, x }, t: 0.0 };
    }

    /// 머리 위에서 받을 몸 가운데 x — 내려오는 길에 머리 위 높이를 지나는 곳이 같은 바닥 조각 위이고 그때까지 달려갈 수 있을 때만
    fn catch_x(&self, o: &Incoming, world: &World) -> Option<f64> {
        let st = self.standing?;
        let piece = world.piece_at(st.owner, self.body.x)?;
        let hands_y = self.body.y - self.catch_height();
        if o.y > hands_y {
            return None;
        }
        let (x, t) = juggle::descent_x(o.x, o.y, o.vx, o.vy, hands_y, self.cfg.dpi)?;
        let (lo, hi) = (piece.x_start + self.cfg.half_w, piece.x_end - self.cfg.half_w);
        if !(lo..=hi).contains(&x) {
            return None;
        }
        let run = self.cfg.walk_speed * Move::Run.speed_mult() * t + juggle::CATCH_REACH * self.cfg.size;
        ((x - self.body.x).abs() <= run).then_some(x)
    }

    /// 발에서 받는 손(머리 위)까지 높이
    fn catch_height(&self) -> f64 {
        let (_, gy) = self.cfg.grip.catch.at(0.0);
        (1.0 - gy) * self.cfg.size
    }

    pub(super) fn update_chase(&mut self, dt: f64, world: &World, id: u64, x: f64, t: f64) {
        let Some(o) = self.rally.target.filter(|o| o.id == id) else {
            self.end_juggle();
            return;
        };
        if t > juggle::CHASE_SECS || self.antics_interrupted() {
            self.end_juggle();
            return;
        }
        let dx = x - self.body.x;
        let step = self.cfg.walk_speed * Move::Run.speed_mult() * dt;
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
            if self.juggle_walk(world, dx.signum(), step.min(dx.abs())).is_none() {
                return;
            }
        } else if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let hands = (self.body.x, self.body.y - self.catch_height());
        if (o.x - hands.0).hypot(o.y - hands.1) <= juggle::CATCH_REACH * self.cfg.size {
            self.catch_object(o);
            return;
        }
        if o.y > self.body.y - 0.3 * self.cfg.size {
            // 놓쳤다 (벌써 머리 아래로)
            self.end_juggle();
            return;
        }
        self.state = State::Juggle { phase: JugglePhase::Chase { id, x }, t };
    }

    fn catch_object(&mut self, o: Incoming) {
        let n = o.rally.saturating_add(1);
        self.rally.target = None;
        self.juggle.carry = Some(o.id);
        self.juggle.giant = false;
        self.juggle.local = false;
        self.juggle.event = Some(JuggleEvent::Catch { id: o.id });
        if n >= juggle::MAX_RALLY {
            self.speech = Some(Speech::RallyEnd);
            self.juggle.plan = Plan::Throw;
            self.rally.throwing = None;
        } else {
            self.speech_text = Some(format!("{n}!"));
            self.juggle.plan = Plan::Lob(o.from);
            self.rally.throwing = Some((o.id, n));
        }
        self.facing = visit::dir(o.from);
        self.state = State::Juggle { phase: JugglePhase::Catch, t: 0.0 };
    }

    /// 손에서 놓았다 (brain_juggle.rs release): 되던지는 것이었으면 횟수를 실어 보낸다
    pub(super) fn rally_released(&mut self, id: u64) {
        match self.rally.throwing.take() {
            Some((got, n)) if got == id => self.rally.out = Some((id, n)),
            _ => {}
        }
    }

    /// ⚾ 방금 되던진 물건과 실을 주고받은 횟수 (한 번) — 앱이 Object::set_rally로 적는다
    pub fn take_rally(&mut self) -> Option<(u64, u32)> {
        self.rally.out.take()
    }
}

use super::juggle_moves::{JugglePhase, Plan};
