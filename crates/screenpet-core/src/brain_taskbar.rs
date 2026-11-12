//! 📌 작업표시줄(Dock)에서 앱 아이콘 장난감 끌어내기 (pet.json `taskbar_play` — 흐물이)의 Pet 동작 — brain.rs의 자식 모듈. 숫자와 까닭은 taskbar.rs.
//!
//! - `notice_taskbar`: 앱이 매 틱 (notice_objects 뒤에) 바닥의 물건 자리와 장난감 사정(Toys)을 넘김
//!   화면 아래 바닥을 걷거나 쉬다가 굴릴 때가 되면 한 번 주사위 → 쪼그려 끙차 (JugglePhase::Yank) + JuggleEvent::Pull.
//! - 앱이 장난감 창을 만들어 `hold_toy`로 쥐여 주면 번쩍 들어 올림 — 그 뒤는 들고 던지기와 같음 (brain_juggle.rs, 배달은 빼고).
//! - 손을 떠난 장난감이 내려앉으면 조금 뒤 가서 주워 바닥을 조금 걷다가 도로 밀어 넣음 (JugglePhase::Stuff + JuggleEvent::Stow → 앱이 뿅).
//! - 앱이 장난감이 없어졌다고 하면 (치움·저절로 사라짐) 잊음

use super::juggle_moves::{JugglePhase, Plan};
use super::*;
use crate::juggle::{self, JuggleEvent, ObjectSpot};
use crate::taskbar::{self, Toys};

/// 작업표시줄 장난감의 기억
#[derive(Debug, Clone)]
pub struct ToyMood {
    /// 이 동물만 굴리는 주사위 (다른 동물과 들고 던지기 주사위 순서는 그대로)
    pub(super) rng: fastrand::Rng,
    /// 다음에 끌어내기 주사위를 굴릴 시각 (clock). 처음 볼 때 정함
    pub(super) next_at: Option<f64>,
    /// 내가 끌어내 화면에 있는 장난감 (물건 id)
    pub(super) toy: Option<u64>,
    /// 도로 넣으러 갈 시각 — 장난감이 손을 떠나 놓인 것을 본 뒤에 정함
    pub(super) return_at: Option<f64>,
    /// 도로 넣으려고 다가가는 중
    pub(super) returning: bool,
}

impl ToyMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        ToyMood { rng: fastrand::Rng::with_seed(seed ^ 0x7461_736b), next_at: None, toy: None, return_at: None, returning: false }
    }

    /// 장난감 기억을 지움
    fn forget(&mut self) {
        self.toy = None;
        self.return_at = None;
        self.returning = false;
    }
}

impl Pet {
    /// 장난감을 기다리며 끙차 당기는 중 (앱: 장난감을 다 만들었을 때 아직 이러면 쥐여 줌)
    pub fn wants_toy(&self) -> bool {
        self.cfg.taskbar_play && matches!(self.state, State::Juggle { phase: JugglePhase::Yank, .. }) && self.juggle.carry.is_none()
    }

    /// 앱이 만든 장난감(id)을 쥐여 줌. 이제 안 기다리면 false (앱은 그 장난감을 만들지 않음)
    pub fn hold_toy(&mut self, id: u64) -> bool {
        if !self.wants_toy() {
            return false;
        }
        self.juggle.carry = Some(id);
        self.juggle.giant = false;
        self.juggle.local = true;
        self.toy.toy = Some(id);
        self.toy.return_at = None;
        self.toy.returning = false;
        true
    }

    /// 화면 아래 바닥(작업표시줄 쪽)에 서 있음 — 작업표시줄이 옆·위에 있거나 숨어 있어도 바닥에서 함
    fn on_bottom_ground(&self) -> bool {
        self.standing.is_some_and(|s| matches!(s.owner, Owner::Ground(_)))
    }

    /// 매 틱 notice_objects 뒤에: 끌어낼 때인지, 도로 넣으러 갈 때인지 봄 (taskbar_play인 동물만 일이 있음)
    pub fn notice_taskbar(&mut self, spots: &[ObjectSpot], toys: Toys, world: &World) {
        if !self.cfg.taskbar_play {
            return;
        }
        if self.toy.toy.is_some_and(|id| toys.live != Some(id)) {
            // 사용자가 치웠거나 저절로 사라졌다
            self.toy.forget();
        }
        let next_at = match self.toy.next_at {
            Some(at) => at,
            None => {
                let at = self.clock + behavior::range(&mut self.toy.rng, taskbar::PULL_EVERY_SECS);
                self.toy.next_at = Some(at);
                at
            }
        };
        if let Some(id) = self.toy.toy {
            // 한 번에 하나: 내 장난감이 나와 있는 동안엔 새로 끌어내지 않는다
            self.plan_return(id, spots, world);
            return;
        }
        if !toys.ready || toys.live.is_some() || self.clock < next_at || !self.may_pick_up() || !self.on_bottom_ground() {
            return;
        }
        self.toy.next_at = Some(self.clock + behavior::range(&mut self.toy.rng, taskbar::PULL_EVERY_SECS));
        if self.toy.rng.f64() >= taskbar::PULL_CHANCE {
            return;
        }
        self.start_yank();
    }

    /// 손을 떠난 장난감이 놓인 것을 보면 도로 넣으러 갈 시각을 정하고, 때가 되면 (같은 바닥에서 닿을 수 있으면) 다가감
    fn plan_return(&mut self, id: u64, spots: &[ObjectSpot], world: &World) {
        if self.juggle.carry == Some(id) {
            return;
        }
        if self.toy.returning && !matches!(self.state, State::Juggle { phase: JugglePhase::Approach { .. }, .. }) {
            // 다가가다 그만뒀다 (못 닿음·누가 치움)
            self.toy.returning = false;
        }
        let spot = spots.iter().find(|s| s.id == id).copied();
        let Some(at) = self.toy.return_at else {
            if spot.is_some() {
                let quick = self.toy.rng.f64() < taskbar::QUICK_RETURN_CHANCE;
                let secs = if quick { taskbar::QUICK_RETURN_SECS } else { taskbar::RETURN_SECS };
                self.toy.return_at = Some(self.clock + behavior::range(&mut self.toy.rng, secs));
            }
            return;
        };
        if self.clock < at || self.toy.returning || !self.free_hands() || !self.on_bottom_ground() {
            return;
        }
        let (owner, y) = (self.standing.map(|s| s.owner), self.body.y);
        let reachable = spot.filter(|s| Some(s.owner) == owner && (s.y - y).abs() <= juggle::SAME_LEVEL_PX).and_then(|s| self.approach_x(&s, world).map(|x| (s, x)));
        match reachable {
            Some((s, x)) => {
                self.toy.returning = true;
                self.start_approach(&s, x);
            }
            // 다른 디딤판 위·날아가는 중: 조금 뒤에 다시 본다 (끝내 못 가면 장난감이 저절로 사라진다)
            None => self.toy.return_at = Some(self.clock + taskbar::RETRY_SECS),
        }
    }

    /// 작업표시줄 아이콘을 끌어내기 시작함
    fn start_yank(&mut self) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.juggle.giant = false;
        self.juggle.local = true;
        self.speech = Some(Speech::Strain);
        self.juggle.event = Some(JuggleEvent::Pull);
        self.state = State::Juggle { phase: JugglePhase::Yank, t: 0.0 };
    }

    /// 끙차 한 틱 (t = 이 단계에 들어선 뒤): 장난감이 왔고 충분히 당겼으면 쑥 빠져 번쩍, 끝내 안 오면 그만둠
    pub(super) fn update_yank(&mut self, world: &World, t: f64) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        if self.juggle.carry.is_some() && t >= taskbar::YANK_MIN_SECS {
            self.speech = Some(Speech::Pop);
            self.juggle.plan = if self.toy.rng.f64() < taskbar::PUT_BACK_CHANCE { Plan::Stuff } else { self.toy_play_plan() };
            self.state = State::Juggle { phase: JugglePhase::Lift, t: 0.0 };
        } else if self.juggle.carry.is_none() && t >= taskbar::YANK_GIVE_UP_SECS {
            self.end_juggle();
        } else {
            self.state = State::Juggle { phase: JugglePhase::Yank, t };
        }
    }

    /// 갖고 놀기: 들고 돌아다니다 던지거나 바로 던짐 (친구에게 배달은 하지 않음)
    fn toy_play_plan(&mut self) -> Plan {
        if self.toy.rng.f64() < taskbar::TOY_WANDER_CHANCE {
            Plan::Wander
        } else {
            Plan::Throw
        }
    }

    /// 내 장난감을 주워 들었음 (start_lift): 도로 넣을 때가 됐으면 넣으러, 아니면 또 갖고 놂. 내 장난감이 아니면 None
    pub(super) fn toy_pickup_plan(&mut self, id: u64) -> Option<Plan> {
        if self.toy.toy != Some(id) {
            return None;
        }
        let due = self.toy.returning || self.toy.return_at.is_some_and(|at| self.clock >= at);
        self.toy.returning = false;
        // 다시 놓이면 그때부터 잰다
        self.toy.return_at = None;
        Some(if due && self.on_bottom_ground() { Plan::Stuff } else { self.toy_play_plan() })
    }

    /// 도로 밀어 넣기 시작 (바닥이 아니면 그냥 던짐)
    pub(super) fn start_stuff(&mut self) {
        if !self.on_bottom_ground() {
            self.start_throw();
            return;
        }
        self.speech = Some(Speech::Stow);
        self.state = State::Juggle { phase: JugglePhase::Stuff { released: false }, t: 0.0 };
    }

    /// 밀어 넣기 한 틱: STUFF_RELEASE에 손을 떠나 바닥 속으로 (앱이 뿅 치움)
    pub(super) fn update_stuff(&mut self, world: &World, released: bool, t: f64) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        if !released && t >= taskbar::STUFF_RELEASE {
            if let Some(id) = self.juggle.carry.take() {
                self.juggle.event = Some(JuggleEvent::Stow { id });
                if self.toy.toy == Some(id) {
                    self.toy.forget();
                }
            }
            self.state = State::Juggle { phase: JugglePhase::Stuff { released: true }, t };
        } else if t >= taskbar::STUFF_SECS {
            self.end_juggle();
        } else {
            self.state = State::Juggle { phase: JugglePhase::Stuff { released }, t };
        }
    }
}
