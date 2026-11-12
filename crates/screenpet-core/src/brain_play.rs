//! 다른 동물들도 더 놀기 (v0.6.3) — brain.rs의 자식 모듈. 숫자는 play.rs.
//! 하는 중인 놀이를 이어 감: 🐱 툭툭 (brain_knock.rs) · 🐿️ 볼 빵빵 (brain_stash.rs) · 🦆 기차 (brain_train.rs) ·
//! 🙌 하이파이브 (brain_highfive.rs) · 깜짝 폴짝 (여기 — 🦖 큰 쿵·기차 끝). 🦖 큰 쿵 자체는 brain_quake.rs.
//!
//! 주사위는 놀이 주사위(PlayMood.rng)로 굴림 — 평소 동작(Pet::rng)과 다른 동물의 주사위 순서는 그대로.

use super::*;
use super::highfive_moves::HighFivePhase;
use crate::juggle::ObjectSpot;
use crate::play::{self, ObjectPlay};

/// 앱을 켜고 처음 놀이를 하기까지 (초)
const FIRST_READY_SECS: f64 = 60.0;

/// 깜짝 폴짝이 끝난 뒤
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BounceThen {
    /// 잠깐 앉았다가 평소대로
    Rest,
    /// 🦆 기차 끝: 아무 쪽으로 흩어져 걸음
    Disperse,
}

/// 놀이의 기억
#[derive(Debug, Clone)]
pub struct PlayMood {
    /// 놀이 전용 난수
    pub(super) rng: fastrand::Rng,
    /// 이 시각(clock)부터 다시 할 수 있음
    pub(super) knock_ready_at: f64,
    /// 다람쥐 숨기기를 다시 할 수 있는 시각
    pub(super) stash_ready_at: f64,
    /// 오리 기차를 다시 할 수 있는 시각
    pub(super) train_ready_at: f64,
    /// 쿵쾅을 다시 할 수 있는 시각
    pub(super) quake_ready_at: f64,
    /// 주사위를 굴린 물건과 다시 굴릴 수 있는 시각
    pub(super) seen: Vec<(u64, f64)>,
    /// 바닥의 물건들 (notice_play_objects가 채움)
    pub(super) spots: Vec<ObjectSpot>,
    /// 앱에 알릴 물건 일 (take_object_play로 한 번)
    pub(super) event: Option<ObjectPlay>,
    /// 🦆 기차를 부르고 싶음 — 이 시각까지 걷기 시작하면 코디네이터가 친구를 모음
    pub(super) train_until: f64,
    /// 🦆 따라갈 자리 (x, 그 자리에서 볼 쪽, 알려 준 시각)
    pub(super) follow: Option<(f64, f64, f64)>,
    /// 🐿️ 볼에 든 물건 수
    pub(super) cheeks: u8,
    /// 🦖 막 시작한 쿵쾅이 큰 쿵임
    pub(super) big_stomp: bool,
    /// 🙌 뛸 때 "High five!" 하는 쪽
    pub(super) speak_five: bool,
}

impl PlayMood {
    /// 시드로 난수를 따로 둠. 모든 놀이는 FIRST_READY_SECS 뒤부터
    pub(super) fn new(seed: u64) -> Self {
        PlayMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x706c_6179),
            knock_ready_at: FIRST_READY_SECS,
            stash_ready_at: FIRST_READY_SECS,
            train_ready_at: FIRST_READY_SECS,
            quake_ready_at: FIRST_READY_SECS,
            seen: Vec::new(),
            spots: Vec::new(),
            event: None,
            train_until: f64::NEG_INFINITY,
            follow: None,
            cheeks: 0,
            big_stomp: false,
            speak_five: false,
        }
    }
}

impl Pet {
    /// 앱이 물건에 할 일 (한 번만): 🐱 툭 밀기 · 🐿️ 뿅 치우기 · 🦖 톡 튀기기
    pub fn take_object_play(&mut self) -> Option<ObjectPlay> {
        self.play.event.take()
    }

    /// 매 틱 update 전에: 바닥의 물건 (🐱 고양이·🐿️ 다람쥐만 일이 있음). 숨겨 둔 동안엔 앱이 빈 목록을 줌
    pub fn notice_play_objects(&mut self, spots: &[ObjectSpot], world: &World) {
        if !self.cfg.knocks && !self.cfg.stashes {
            return;
        }
        self.play.spots.clear();
        self.play.spots.extend_from_slice(spots);
        if self.cfg.knocks {
            self.notice_knock(world);
        }
        if self.cfg.stashes {
            self.notice_stash(world);
        }
    }

    /// 새 놀이를 시작할 수 있음: 디딤판 위에서 스스로 걷거나 쉬는 중, 볼일·잡힘·조용한 때·떠나는 길·든 것 없음
    pub(super) fn play_free(&self) -> bool {
        matches!(self.state, State::Move { .. } | State::Idle { .. } | State::Turn { .. })
            && self.standing.is_some()
            && !self.cfg.flies
            && self.inside.is_none()
            && !self.housed
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.thrown
            && !self.tickling
            && self.hover_since.is_none()
            && !self.rider_on
            && self.juggle.carry.is_none()
    }

    /// 하던 놀이를 그만둬야 함 (사용자 볼일·자리 비움·집중·하우스·창 안·떠나는 길)
    pub(super) fn romp_interrupted(&self) -> bool {
        self.housed || self.has_errand() || self.presence_quiet() || self.inside.is_some() || self.is_heading_out() || self.tickling
    }

    /// 처음 보는 물건마다 한 번 주사위: 이 물건을 이미 봤으면 false, 아니면 적어 두고 true
    pub(super) fn first_look(&mut self, id: u64) -> bool {
        let clock = self.clock;
        if self.play.seen.iter().any(|&(s, until)| s == id && clock < until) {
            return false;
        }
        self.play.seen.retain(|&(_, until)| clock < until);
        self.play.seen.push((id, clock + play::SAME_OBJECT_SECS));
        true
    }

    /// update에서 (흐물이 놀이 뒤). 이번 틱을 여기서 처리했으면 true
    pub(super) fn update_romp(&mut self, dt: f64, world: &World) -> bool {
        match self.state {
            State::Knock { phase, t } => self.update_knock(dt, world, phase, t),
            State::Stash { phase, t } => self.update_stash(dt, world, phase, t),
            State::Lead { remaining } => self.update_lead(dt, world, remaining),
            State::Follow { remaining } => self.update_follow(dt, world, remaining),
            State::HighFive { phase, t } => self.update_high_five(dt, world, phase, t),
            State::Bounce { t, rise, then } => self.update_bounce(dt, world, t, rise, then),
            _ => return self.resume_stash(),
        }
        true
    }

    /// 제자리에서 폴짝 (rise = 몸 크기 배, 그림만 뜸 — 디딤판 위에 그대로 있음)
    pub(super) fn start_bounce(&mut self, rise: f64, then: BounceThen) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.state = State::Bounce { t: 0.0, rise, then };
    }

    /// 제자리 통통 튀기 한 틱. 끝나면 then을 함
    fn update_bounce(&mut self, dt: f64, world: &World, t: f64, rise: f64, then: BounceThen) {
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let t = t + dt;
        if t < play::BOUNCE_SECS {
            self.state = State::Bounce { t, rise, then };
            return;
        }
        self.state = match then {
            BounceThen::Rest => self.rest_beat(),
            BounceThen::Disperse => {
                if self.play.rng.bool() {
                    self.facing = -self.facing;
                }
                State::Move { kind: Move::Walk, remaining: behavior::range(&mut self.play.rng, play::DISPERSE_SECS) }
            }
        };
    }

    /// 놀이 중 그림이 떠 있는 높이 (물리 px) — 하이파이브·깜짝 폴짝
    pub(super) fn play_lift(&self) -> f64 {
        match self.state {
            State::Bounce { t, rise, .. } => play::hop_height(t, play::BOUNCE_SECS, rise * self.cfg.size),
            State::HighFive { phase: HighFivePhase::Jump, t } => play::hop_height(t, play::FIVE_JUMP_SECS, play::FIVE_RISE * self.cfg.size),
            _ => 0.0,
        }
    }

    /// 놀이 그림
    pub(super) fn play_anim(&self) -> Option<Anim> {
        Some(match self.state {
            State::Knock { phase, .. } => self.knock_anim(phase),
            State::Stash { phase, .. } => self.stash_anim(phase),
            State::Lead { .. } => Anim::Walk,
            State::Follow { .. } => {
                if self.follow_running() {
                    Anim::Run
                } else {
                    Anim::Walk
                }
            }
            State::HighFive { phase, .. } => match phase {
                HighFivePhase::Close { .. } => Anim::Walk,
                HighFivePhase::Ready => Anim::Idle,
                HighFivePhase::Jump => Anim::Hop,
                HighFivePhase::Land => Anim::Land,
            },
            State::Bounce { t, .. } => {
                if t < play::BOUNCE_SECS * 0.75 {
                    Anim::Hop
                } else {
                    Anim::Land
                }
            }
            _ => return None,
        })
    }

    /// 🐿️ 볼이 빵빵한 동안의 걷기·쉬기 그림
    pub(super) fn cheeks_anim(&self, anim: Anim) -> Anim {
        if self.play.cheeks == 0 || !self.cfg.stashes {
            return anim;
        }
        match anim {
            Anim::Walk | Anim::Run => Anim::CheeksWalk,
            Anim::Idle | Anim::Look | Anim::LookUp => Anim::CheeksIdle,
            a => a,
        }
    }
}

#[cfg(test)]
impl Pet {
    /// 테스트: 지금 걷고 있으면 기차를 부르고 싶음 (주사위·쉬는 때 없이)
    pub(crate) fn want_train_now(&mut self) {
        self.play.train_until = self.clock + 4.0;
    }

    /// 테스트: 다음 쿵쾅은 큰 쿵임 / 쉬는 때가 남았음
    pub(crate) fn set_quake_ready(&mut self, ready: bool) {
        self.play.quake_ready_at = if ready { f64::NEG_INFINITY } else { f64::INFINITY };
    }

    /// 테스트: 기차 쉬는 때
    pub(crate) fn set_train_ready(&mut self, at: f64) {
        self.play.train_ready_at = at;
    }

    /// 테스트: 오리 기차를 다시 할 수 있는 시각
    pub(crate) fn train_ready_at(&self) -> f64 {
        self.play.train_ready_at
    }

    /// 테스트: 동물 시계
    pub(crate) fn play_clock(&self) -> f64 {
        self.clock
    }

    /// 테스트: 막 시작한 쿵쾅이 큰 쿵이 되게 고름 (force_stomp 뒤에)
    pub(crate) fn begin_stomp_now(&mut self) {
        self.begin_stomp();
    }
}
