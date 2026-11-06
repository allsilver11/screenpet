//! 두 동물이 만났을 때 (같은 디딤판, 서로 쪽으로 걷거나 한쪽이 다른 쪽 앞에 걸어·달려 옴) — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 누가 무엇을 할지와 뛰어 내릴 자리는 social.rs가 정하고, 여기서는 그대로 움직인다:
//!
//! - 밟기: 한 마리가 친구 머리로 뛰어 밟고 조금 튀어 친구 너머에 내린다. 밟힌 친구는 잠깐 납작 (Land → Annoyed, "oof!").
//! - 높이 튀기: 같지만 머리를 밟고 훨씬 높이 솟았다가 너머에 내린다.
//! - 지나치기: 둘 다 폴짝 — 한 마리는 높이, 한 마리는 낮게, 꼭대기가 같은 때에 오게 해서 닿지 않고 엇갈린다.
//! - 올라서기: 친구 머리(탑이면 맨 위)로 뛰어올라 그대로 선다 — 그 뒤는 brain_tower.rs.
//! - 밟고 폴짝 되튀기: 머리를 밟고 돌아서 온 쪽으로 작게 폴짝 내린 뒤 그쪽으로 간다.
//! - 한쪽만 다가가도 (친구가 서 있거나 앞서 가도) 다가간 쪽이 밟는다. 내려앉은 나는 동물도 밟힐 수 있다 (나는 동물은 뛰지 않는다).
//! - 달리던 동물은 웅크림이 짧고, 조금 더 멀리서 뛰고, 더 멀리 내린다 (Pet::run_carry).
//!
//! 뛰는 길은 큰 점프와 같은 포물선(hop::leap_arc)과 중력이고, 그림은 Land·Hop·Fall·Look·Annoyed 그대로다.
//! 내릴 때 디딤판이 그대로면 그 위에, 아니면 그 자리에서 떨어져 진짜 디딤판에 내린다 (land_leap). 내린 뒤엔 가던 쪽으로 계속 걷는다.
//! 손님도 이 화면 주인의 코디네이터가 움직인다 — 통신할 것이 없다.

use super::*;
use crate::hop;

/// 뛰기 전 웅크리는 시간
pub const CROUCH_SECS: f64 = 0.18;
/// 내려서는 자세로 있는 시간
const LANDING_SECS: f64 = 0.2;
/// 밟히기를 기다리는 시간의 상한 (뛴 친구가 사라져도 스스로 끝낸다)
const BRACE_MAX_SECS: f64 = 2.0;
/// 밟힌 친구: Land (납작) 뒤 Annoyed
pub(super) const SQUASH_SECS: f64 = 1.0;
const SQUASH_GRUMPY_FROM: f64 = 0.75;
/// 머리 꼭대기 높이 (발에서 몸 크기의 이 비율 위, brain.rs HEAD_TOP과 같다)
pub const HEAD_RISE: f64 = HEAD_TOP;
/// 머리로 뛸 때, 밟고 튈 때, 높이 튈 때 머리보다 더 솟는 높이 (몸 크기 배)
const TO_HEAD_RISE: f64 = 0.35;
const BOUNCE_RISE: f64 = 0.5;
const HIGH_BOUNCE_RISE: f64 = 2.4;

/// 고른 만남
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetKind {
    /// 머리를 밟고 친구 너머에 내린다
    Stomp,
    /// 머리를 밟고 훨씬 높이 튀어 너머에 내린다
    HighBounce,
    /// 둘 다 폴짝, 닿지 않고 지나친다
    Pass,
    /// 친구 머리 위로 뛰어올라 선다 (brain_tower.rs) — 친구는 하던 일을 계속한다
    Climb,
    /// 머리를 밟고 폴짝 되튀어 온 쪽에 내려선 뒤 돌아서 간다
    BounceBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetPhase {
    Crouch,
    /// 친구 머리로 가는 포물선
    ToHead,
    /// 내려설 자리로 가는 포물선
    Final,
    /// 내려서는 자세
    Landing,
    /// 친구가 머리로 뛰어오는 동안 쳐다본다
    Brace,
    /// 밟혔다
    Squash,
}

impl MeetPhase {
    pub fn airborne(self) -> bool {
        matches!(self, MeetPhase::ToHead | MeetPhase::Final)
    }
}

/// 뛰는 쪽의 길
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeetPlan {
    kind: MeetKind,
    /// 내려설 디딤판과 그 기준점에서의 x
    owner: Owner,
    rel_x: f64,
    land: (f64, f64),
    /// 밟을 머리 꼭대기 (지나치기면 None)
    head: Option<(f64, f64)>,
    /// 다음 포물선이 더 솟는 높이 (px)
    rise: f64,
}

impl MeetPlan {
    pub(super) fn is_climb(&self) -> bool {
        self.kind == MeetKind::Climb
    }

    pub(super) fn set_head(&mut self, head: (f64, f64)) {
        self.head = Some(head);
    }
}

#[cfg(test)]
impl Pet {
    /// 이 동물이 뛰는 만남의 종류 (뛰는 쪽만)
    pub(crate) fn meet_kind(&self) -> Option<MeetKind> {
        self.meet.map(|m| m.kind)
    }
}

impl Pet {
    /// 스스로 걷거나 달리는 걷는 동물 (마주침을 볼 수 있다)
    pub fn walks_on_surface(&self) -> bool {
        matches!(self.state, State::Move { kind: Move::Walk | Move::Run, .. }) && !self.cfg.flies && self.standing.is_some()
    }

    /// 지금 걷는 디딤판 조각 (왼쪽, 오른쪽)
    pub fn walk_span(&self) -> Option<(f64, f64)> {
        self.walk_span
    }

    /// 밟거나 뛰어넘거나 밟히는 중
    pub fn is_meeting(&self) -> bool {
        matches!(self.state, State::Meet { .. })
    }

    /// 방금 친구 머리를 밟았으면 true (한 번만)
    pub fn take_head_bump(&mut self) -> bool {
        std::mem::take(&mut self.head_bump)
    }

    fn begin_meet(&mut self, plan: Option<MeetPlan>, phase: MeetPhase, remaining: f64) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.poop.stepping_off();
        self.meet = plan;
        self.state = State::Meet { phase, remaining };
    }

    /// 뛰는 쪽: land_x(같은 디딤판)로 간다. head가 있으면 먼저 그 머리를 밟는다. crouch초 웅크린 뒤 뛴다
    pub fn start_vault(&mut self, kind: MeetKind, head: Option<(f64, f64)>, land_x: f64, rise: f64, crouch: f64) {
        let Some(st) = self.standing else { return };
        let rel_x = land_x - (self.body.x - st.rel_x);
        let rise = if head.is_some() { TO_HEAD_RISE * self.cfg.size } else { rise };
        self.begin_meet(Some(MeetPlan { kind, owner: st.owner, rel_x, land: (land_x, self.body.y), head, rise }), MeetPhase::Crouch, crouch);
        if kind == MeetKind::Pass && rise > self.cfg.size / 2.0 {
            self.speech = Some(Speech::Whee);
        }
    }

    /// 밟힐 쪽: 뛰어오는 친구를 쳐다보며 기다린다
    pub fn start_brace(&mut self) {
        self.begin_meet(None, MeetPhase::Brace, BRACE_MAX_SECS);
    }

    /// 머리를 밟혔다: 납작하게 덜덜 + "oof!" (기다리던 중일 때만)
    pub fn feel_head_stomp(&mut self) {
        if matches!(self.state, State::Meet { phase: MeetPhase::Brace, .. }) {
            self.speech = Some(Speech::Oof);
            self.wobble_timer = stomp_moves::WOBBLE_SECS;
            self.state = State::Meet { phase: MeetPhase::Squash, remaining: SQUASH_SECS };
        }
    }

    /// 뛴 친구가 오지 않게 됐다: 기다리던 쪽은 가던 길을 간다
    pub fn end_brace(&mut self) {
        if matches!(self.state, State::Meet { phase: MeetPhase::Brace, .. }) {
            self.carry_on();
        }
    }

    /// 가던 쪽으로 다시 걷는다 (내려앉아 밟힌 나는 동물은 그대로 앉아 있다가 평소대로)
    fn carry_on(&mut self) {
        self.meet = None;
        self.pacing.after_beat = None;
        self.state = if self.cfg.flies { self.idle_state() } else { State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.rng, Move::Walk.secs()) } };
    }

    /// 내려앉은 나는 동물이 밟히기를 기다리거나 밟힌 중 (나는 동물이 하는 만남은 이것뿐)
    pub(super) fn perched_meet(&self) -> bool {
        self.cfg.flies && self.standing.is_some() && matches!(self.state, State::Meet { phase: MeetPhase::Brace | MeetPhase::Squash, .. })
    }

    /// 나는 동물로 바뀌었으면 걷는 동물의 동작은 끝낸다 (나는 동물의 길에는 이 상태들이 없다 — 내려앉아 밟히는 것만 빼고)
    pub(super) fn drop_walking_moves(&mut self) {
        if self.cfg.flies && !self.perched_meet() && matches!(self.state, State::Meet { .. } | State::Slip { .. } | State::Leap { .. } | State::Tumble { .. }) {
            self.meet = None;
            self.state = self.idle_state();
        }
    }

    /// 포물선 하나를 잡아 떠난다
    fn take_arc(&mut self, world: &World, to: (f64, f64), rise: f64, phase: MeetPhase) {
        let top = world.bounds.top + self.cfg.size;
        let arc = hop::leap_arc((self.body.x, self.body.y), to, rise, top);
        self.standing = None;
        self.body.vx = arc.vx;
        self.body.vy = arc.vy;
        self.state = State::Meet { phase, remaining: arc.secs };
    }

    pub(super) fn update_meet(&mut self, dt: f64, world: &World, phase: MeetPhase, remaining: f64) {
        match phase {
            MeetPhase::Crouch | MeetPhase::Brace if self.social_interrupted() => self.carry_on(),
            MeetPhase::Crouch => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                let remaining = remaining - dt;
                let Some(plan) = self.meet.filter(|_| remaining <= 0.0) else {
                    self.state = if self.meet.is_some() { State::Meet { phase, remaining } } else { self.idle_state() };
                    return;
                };
                let (to, next) = match plan.head {
                    Some(head) => (head, MeetPhase::ToHead),
                    None => (plan.land, MeetPhase::Final),
                };
                self.take_arc(world, to, plan.rise, next);
            }
            MeetPhase::ToHead | MeetPhase::Final => self.fly_arc(dt, world, phase, remaining),
            MeetPhase::Landing | MeetPhase::Brace | MeetPhase::Squash => {
                if !self.follow_anchor(world) {
                    self.start_fall();
                    return;
                }
                let remaining = remaining - dt;
                if remaining > 0.0 {
                    self.state = State::Meet { phase, remaining };
                } else {
                    self.carry_on();
                }
            }
        }
    }

    /// 포물선 위 한 틱. 끝에 닿으면 머리를 밟고 튀거나 내려선다
    fn fly_arc(&mut self, dt: f64, world: &World, phase: MeetPhase, remaining: f64) {
        self.body.arc_step(dt.min(remaining));
        let remaining = remaining - dt;
        if remaining > 0.0 {
            self.state = State::Meet { phase, remaining };
            return;
        }
        let Some(plan) = self.meet else {
            self.start_fall();
            return;
        };
        match (phase, plan.head) {
            (MeetPhase::ToHead, Some(head)) if plan.kind == MeetKind::Climb => {
                self.body.x = head.0;
                self.body.y = head.1;
                self.head_bump = true;
                self.begin_ride();
            }
            (MeetPhase::ToHead, Some(head)) => {
                self.body.x = head.0;
                self.body.y = head.1;
                self.head_bump = true;
                let (speech, rise) = match plan.kind {
                    MeetKind::HighBounce => (Speech::Whee, HIGH_BOUNCE_RISE),
                    _ => (Speech::Boing, BOUNCE_RISE),
                };
                self.speech = Some(speech);
                // 되튀기: 머리 위에서 돌아서 온 쪽으로 폴짝 (내린 뒤 그쪽으로 간다)
                if plan.kind == MeetKind::BounceBack {
                    self.facing = (plan.land.0 - self.body.x).signum();
                }
                self.take_arc(world, plan.land, rise * self.cfg.size, MeetPhase::Final);
            }
            _ => {
                self.land_leap(world, plan.owner, plan.rel_x);
                if matches!(self.state, State::Land { .. }) {
                    self.state = State::Meet { phase: MeetPhase::Landing, remaining: LANDING_SECS };
                }
            }
        }
    }

    pub(super) fn meet_anim(&self, phase: MeetPhase, remaining: f64) -> Anim {
        match phase {
            MeetPhase::Crouch | MeetPhase::Landing => Anim::Land,
            MeetPhase::ToHead | MeetPhase::Final => {
                if self.body.vy < 0.0 {
                    Anim::Hop
                } else {
                    Anim::Fall
                }
            }
            MeetPhase::Brace => Anim::Look,
            MeetPhase::Squash if remaining > SQUASH_GRUMPY_FROM => Anim::Land,
            MeetPhase::Squash => Anim::Annoyed,
        }
    }
}
