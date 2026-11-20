//! 🌱 연령대별 특기 (2026-11-20 spec 2부) — brain.rs의 자식 모듈. 숫자·표는 stage.rs, 펫끼리(👶 졸졸 · 🧓 쓰다듬기)는 social_romp.rs.
//!
//! - 모든 연령대가 모든 특징 행동을 하지만 제 연령대가 훨씬 자주 (StageTrait::rate = 기본 × stage_weight)
//! - 쉬는 박자 뒤 (stage_rest): 🧒 "ooh?" 올려다보기 · 🧒 신나서 폴짝폴짝 · 🧑‍🎤 공중제비 "yeah!" · 🧑‍🎤 시무룩 "ugh" · 🧓 기지개 ·
//!   👶 혼자 오래 있으면 "waah"
//! - 걷는 동안 (stage_walk_tick·toddle_pace·hop_pace): 👶 느린 걸음·가끔 휘청 · 👶 콩 엉덩방아 "oops!" · 🧑 깡총깡총 (토끼 걸음) ·
//!   🧑‍🎤 걷기 대신 전력 질주 (move_state)
//! - 👶 더 자주 졸고 오래 잠 (sleepy_mult·sleep_secs) · 🧒 커서 놀이 ↑ (cursor_fan) · 🧑 술래잡기 ↑ (tag_fan, social.rs)
//! - 종별 특기의 세기 (stage_power): 거미집·쿵 범위·특기 빈도 — 각 brain_*.rs가 power()·soft_power()를 곱함
//! - 레벨업 축하(LevelUp)가 끝나면 새 연령대의 대표 특기 한 줄 ("new: 🤸 flips!" — 글자 그대로, take_speech_text)
//!
//! 주사위는 연령대 주사위(StageMood.rng)로 굴림 — 평소 동작의 주사위 순서는 그대로.

use super::*;
use crate::care::Stage;
use crate::stage::{self as st, StageTrait};

/// 연령대 몸짓 종류 (State::StageAct)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageAct {
    /// 👶 휘청 넘어져 엉덩방아 (fall → sit_front, "oops!")
    Plop,
    /// 👶 혼자 오래 있어 앉아서 "waah" (sit_front, 덜덜)
    Waah,
    /// 🧒 제자리 폴짝폴짝 (hop/land, 반짝)
    HappyHop,
    /// 🧑‍🎤 높이 뛰어 공중제비 (hop → roll → land, "yeah!")
    Flip,
    /// 🧑‍🎤 시무룩 (annoyed, "ugh")
    Sulk,
}

/// 연령대 특기의 기억
#[derive(Debug, Clone)]
pub struct StageMood {
    /// 연령대 전용 난수
    pub(super) rng: fastrand::Rng,
    /// 곁에 다른 펫이 없던 시간 (초) — 👶 "waah"
    pub(super) alone: f64,
    /// 마지막으로 알던 연령대 — 레벨업 뒤 커졌으면 새 특기를 알림
    pub(super) known: Stage,
    /// 행동별로 한 번 센 수 (StageTrait::ALL 순서 — 테스트가 봄)
    pub(super) counts: [u32; StageTrait::ALL.len()],
    /// 테스트: 연령대 특기를 끔 (예전 동작만 재는 테스트 — hush_stage)
    pub(super) hushed: bool,
}

impl StageMood {
    /// 시드로 난수를 따로 둠. 지금 연령대를 알던 연령대로
    pub(super) fn new(seed: u64, stage: Stage) -> Self {
        StageMood { rng: fastrand::Rng::with_seed(seed ^ 0x7374_6167), alone: 0.0, known: stage, counts: [0; StageTrait::ALL.len()], hushed: false }
    }
}

impl Pet {
    /// 지금 연령대
    pub fn stage(&self) -> Stage {
        self.cfg.stage
    }

    /// 동물 종류 id (set_cursor_species가 알려 줌 — 모르면 빈 문자열)
    pub fn species(&self) -> &str {
        &self.species
    }

    /// 종별 특기의 세기 (stage_power — 청소년 1)
    pub(crate) fn power(&self) -> f64 {
        st::stage_power(self.cfg.stage)
    }

    /// 빠르기·높이에 곱하는 덜 변하는 세기 (soft_power — 청소년 1)
    pub(super) fn soft_power(&self) -> f64 {
        st::soft_power(self.cfg.stage)
    }

    /// 쉴 때 이 특별 동작을 할 확률 배: 🦖 공룡 불·크앙·쿵쾅은 연령대 세기, 나머지는 1
    pub(super) fn special_power(&self, kind: Special) -> f64 {
        if kind.is_dino() {
            self.power()
        } else {
            1.0
        }
    }

    /// 이 특징 행동의 지금 확률·세기 (기본 × 연령대 배 — 테스트로 끄면 0)
    pub(super) fn stage_rate(&self, t: StageTrait) -> f64 {
        if self.stage.hushed {
            0.0
        } else {
            t.rate(self.cfg.stage)
        }
    }

    /// home 연령대 행동의 연령대 배 (테스트로 끄면 0)
    fn weight_of(&self, home: Stage) -> f64 {
        if self.stage.hushed {
            0.0
        } else {
            st::stage_weight(self.cfg.stage, home)
        }
    }

    /// 테스트: 연령대 특기를 끔 — 예전 동작의 분포·주사위 순서를 재는 테스트용
    #[cfg(test)]
    pub(crate) fn hush_stage(&mut self) {
        self.stage.hushed = true;
    }

    /// 특징 행동을 한 번 셈
    pub(super) fn count_stage(&mut self, t: StageTrait) {
        if let Some(i) = StageTrait::ALL.iter().position(|&x| x == t) {
            self.stage.counts[i] = self.stage.counts[i].saturating_add(1);
        }
    }

    /// 특징 행동을 지금까지 한 수
    pub fn stage_count(&self, t: StageTrait) -> u32 {
        StageTrait::ALL.iter().position(|&x| x == t).map_or(0, |i| self.stage.counts[i])
    }

    /// 매 틱: 혼자 있는 시간을 셈, 레벨업 중이 아니면 알던 연령대를 지금 것으로 (메뉴로 바꾼 단계는 알리지 않음)
    pub(super) fn tick_stage_mood(&mut self, dt: f64) {
        self.stage.alone += dt;
        let celebrating = matches!(self.state, State::LevelUp { .. } | State::Eating { level_up: true, .. })
            || self.queued_food.is_some_and(|(_, level_up)| level_up)
            || matches!(self.before_press, Some((State::LevelUp { .. } | State::Eating { level_up: true, .. }, _)));
        if !celebrating {
            self.stage.known = self.cfg.stage;
        }
    }

    /// 곁에 다른 펫이 있음 (social_romp.rs가 매 틱 알려 줌) — 👶 혼자 시계를 되돌림
    pub fn note_company(&mut self) {
        self.stage.alone = 0.0;
    }

    /// 레벨업 축하가 끝남: 알던 연령대보다 커졌으면 새 연령대의 대표 특기 한 줄
    pub(super) fn announce_stage(&mut self) {
        if self.cfg.stage.step() > self.stage.known.step() {
            self.speech_text = Some(st::signature_line(self.cfg.stage).to_string());
        }
        self.stage.known = self.cfg.stage;
    }

    /// 연령대 행동을 시작해도 되는 때: 걷는 동물이 디딤판 위, 볼일·조작·창 안·집 가는 길·조용한 때 없음
    fn stage_free(&self) -> bool {
        !self.cfg.flies
            && self.standing.is_some()
            && self.inside.is_none()
            && !self.housed
            && self.home_trip.is_none()
            && self.control.is_none()
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && !self.tickling
            && self.hover_since.is_none()
            && !self.rider_on
            && self.juggle.carry.is_none()
    }

    /// rest_state: 쉬는 박자 뒤 연령대 특징 행동 주사위 하나 (안 걸리면 None — 평소대로)
    pub(super) fn stage_rest(&mut self) -> Option<State> {
        if !self.stage_free() {
            return None;
        }
        let lonely = self.stage.alone >= st::lonely_secs(self.cfg.stage);
        let options = [
            (StageTrait::Ooh, self.stage_rate(StageTrait::Ooh)),
            (StageTrait::HappyHop, self.stage_rate(StageTrait::HappyHop)),
            (StageTrait::Flip, self.stage_rate(StageTrait::Flip)),
            (StageTrait::Sulk, self.stage_rate(StageTrait::Sulk)),
            (StageTrait::Stretch, self.stage_rate(StageTrait::Stretch)),
            (StageTrait::Waah, if lonely { self.stage_rate(StageTrait::Waah) } else { 0.0 }),
        ];
        let mut roll = self.stage.rng.f64();
        let picked = options.iter().find(|&&(_, p)| {
            roll -= p;
            roll < 0.0
        })?;
        let t = picked.0;
        self.count_stage(t);
        self.pacing.after_beat = None;
        Some(match t {
            StageTrait::Ooh => {
                self.gaze_up = true;
                self.speech = Some(Speech::Ooh);
                State::Idle { remaining: behavior::range(&mut self.stage.rng, st::OOH_SECS), look: true }
            }
            StageTrait::HappyHop => self.stage_act(StageAct::HappyHop, st::HAPPY_HOP_SECS * f64::from(st::HAPPY_HOPS)),
            StageTrait::Flip => self.stage_act(StageAct::Flip, st::FLIP_SECS),
            StageTrait::Sulk => {
                self.speech = Some(Speech::Sulk);
                let len = behavior::range(&mut self.stage.rng, st::SULK_SECS);
                self.stage_act(StageAct::Sulk, len)
            }
            StageTrait::Stretch => State::Trick { kind: Trick::Stretch, remaining: behavior::range(&mut self.stage.rng, st::STRETCH_SECS) },
            _ => {
                self.stage.alone = 0.0;
                self.speech = Some(Speech::Waah);
                self.stage_act(StageAct::Waah, st::WAAH_SECS)
            }
        })
    }

    /// 연령대 몸짓 상태 (처음부터)
    pub(super) fn stage_act(&mut self, act: StageAct, len: f64) -> State {
        self.clear_hover();
        State::StageAct { act, t: 0.0, len }
    }

    /// 걷는 동안 한 틱 (update_ground): 👶 가끔 휘청, 가끔 콩 엉덩방아 — 엉덩방아를 시작했으면 true
    pub(super) fn stage_walk_tick(&mut self, kind: Move, dt: f64) -> bool {
        if kind != Move::Walk || !self.stage_free() || self.stage.rng.f64() >= self.stage_rate(StageTrait::Toddle) * dt {
            return self.maybe_plop(kind, dt);
        }
        self.count_stage(StageTrait::Toddle);
        self.wobble_timer = self.wobble_timer.max(st::TODDLE_WOBBLE_SECS);
        self.maybe_plop(kind, dt)
    }

    /// 👶 걷다가 콩 넘어져 엉덩방아 주사위 — 시작했으면 true
    fn maybe_plop(&mut self, kind: Move, dt: f64) -> bool {
        if kind == Move::Roll || !self.stage_free() || self.stage.rng.f64() >= self.stage_rate(StageTrait::Plop) * dt {
            return false;
        }
        self.count_stage(StageTrait::Plop);
        self.pacing.after_beat = None;
        self.state = self.stage_act(StageAct::Plop, st::PLOP_SECS);
        true
    }

    /// 👶 아장아장: 걷기 빠르기 배 (달리기·구르기는 그대로)
    pub(super) fn toddle_pace(&self, kind: Move) -> f64 {
        if kind == Move::Walk {
            1.0 - st::TODDLE_SLOW * self.weight_of(StageTrait::Toddle.home())
        } else {
            1.0
        }
    }

    /// 🧑‍🎤 걷기를 고른 참에 가끔 전력 질주로 바꿈 (연령대 주사위)
    pub(super) fn stage_sprint(&mut self, kind: Move) -> Move {
        if kind != Move::Walk || !self.stage_free() || self.stage.rng.f64() >= self.stage_rate(StageTrait::Sprint) {
            return kind;
        }
        self.count_stage(StageTrait::Sprint);
        Move::Run
    }

    /// 🧑 깡총깡총: 토끼 걸음이 아닌 동물이 걷기·달리기 토막을 깡총으로 할 몫 (hop_pace)
    pub(super) fn hop_skip_share(&self) -> f64 {
        if self.cfg.hops || self.cfg.hop_share > 0.0 || self.cfg.flies {
            0.0
        } else {
            self.stage_rate(StageTrait::HopSkip)
        }
    }

    /// 👶 졸음 배 (잘 확률에 곱함)
    pub(super) fn sleepy_mult(&self) -> f64 {
        1.0 + self.stage_rate(StageTrait::Sleepy)
    }

    /// 👶 잠 시간 배
    pub(super) fn sleep_longer(&self) -> f64 {
        1.0 + st::SLEEPY_LONGER * self.weight_of(StageTrait::Sleepy.home())
    }

    /// 🧒 커서 놀이 확률 배
    pub(super) fn cursor_fan(&self) -> f64 {
        1.0 + self.stage_rate(StageTrait::CursorFan)
    }

    /// 🧑 술래잡기 확률 배 (social.rs — 더 놀고 싶은 쪽을 따름)
    pub fn tag_fan(&self) -> f64 {
        1.0 + self.stage_rate(StageTrait::TagFan)
    }

    /// 연령대 몸짓 한 틱 (tick_activity): 때맞춰 말하고, 끝나면 쉼
    pub(super) fn tick_stage_act(&mut self, dt: f64, act: StageAct, t: f64, len: f64) {
        let next = t + dt;
        let crossed = |at: f64| t < at && next >= at;
        match act {
            StageAct::Plop if crossed(st::PLOP_TRIP_SECS) => self.speech = Some(Speech::Plop),
            StageAct::Flip if crossed(st::FLIP_ROLL.1 * st::FLIP_SECS) => self.speech = Some(Speech::Flip),
            _ => {}
        }
        self.state = if next < len { State::StageAct { act, t: next, len } } else { self.idle_state() };
    }

    /// 연령대 몸짓의 그림
    pub(super) fn stage_anim(&self, act: StageAct, t: f64, len: f64) -> Anim {
        match act {
            StageAct::Plop if t < st::PLOP_TRIP_SECS => Anim::Fall,
            StageAct::Plop | StageAct::Waah => Anim::SitFront,
            StageAct::Sulk => Anim::Annoyed,
            StageAct::HappyHop => {
                let u = t % st::HAPPY_HOP_SECS;
                if u < st::HAPPY_HOP_SECS * 0.75 {
                    Anim::Hop
                } else {
                    Anim::Land
                }
            }
            StageAct::Flip => {
                let u = t / len.max(1e-6);
                if u < st::FLIP_ROLL.0 {
                    Anim::Hop
                } else if u < st::FLIP_ROLL.1 {
                    // 구르기 그림이 없는 픽셀 동물·구르지 않는 기린은 그냥 폴짝
                    if self.cfg.no_roll || self.cfg.special.is_some_and(Special::replaces_roll) {
                        Anim::Hop
                    } else {
                        Anim::Roll
                    }
                } else {
                    Anim::Land
                }
            }
        }
    }

    /// 연령대 몸짓으로 그림이 뜬 높이 (물리 px) — 폴짝·공중제비
    pub(super) fn stage_lift(&self) -> f64 {
        match self.state {
            State::StageAct { act: StageAct::HappyHop, t, .. } => {
                crate::play::hop_height(t % st::HAPPY_HOP_SECS, st::HAPPY_HOP_SECS * 0.75, st::HAPPY_HOP_RISE * self.cfg.size)
            }
            State::StageAct { act: StageAct::Flip, t, len } => crate::play::hop_height(t, len * st::FLIP_ROLL.1, st::FLIP_RISE * self.cfg.size),
            _ => 0.0,
        }
    }

    /// 덜덜 (👶 넘어지는 순간·"waah")
    pub(super) fn stage_wobble(&self) -> bool {
        match self.state {
            State::StageAct { act: StageAct::Plop, t, .. } => t < st::PLOP_TRIP_SECS,
            State::StageAct { act: StageAct::Waah, .. } => true,
            _ => false,
        }
    }

    /// 반짝이 (🧒 신나서 폴짝)
    pub(super) fn stage_sparkle(&self) -> bool {
        matches!(self.state, State::StageAct { act: StageAct::HappyHop, .. })
    }
}
