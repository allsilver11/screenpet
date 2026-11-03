//! 곁에 있기와 하루 리듬 (v0.3.7) — 자리 비움·돌아옴, 지역 시각의 하루 일과, ⏱️ 집중 친구.
//!
//! 시계와 유휴 시간은 앱이 넣어 준다 (RhythmInput) — 여기엔 OS도 달력 라이브러리도 없다. 앱은 1초에 한 번 tick을 부른다.
//! 정해진 일은 Pet에 시킨다 (동작 자체는 brain_presence.rs). 내 동물에게만 쓴다 — 손님은 그 주인의 하루를 산다.

use serde::{Deserialize, Serialize};

use crate::brain::Pet;
use crate::care::Care;
use crate::world::World;

/// 입력이 이만큼(초) 없으면 자리를 비운 것으로 본다 — 내 동물이 잠든다
pub const AWAY_AFTER_SECS: f64 = 600.0;
/// 자리를 비운 뒤 유휴 시간이 이보다 짧아지면 돌아온 것이다 (1초에 한 번 보므로 넉넉히)
pub const BACK_BELOW_SECS: f64 = 3.0;
/// 00:00부터 이 분까지 졸린 시간
const SLEEPY_UNTIL: u32 = 6 * 60;
/// 그날의 첫 "good morning!" [시작, 끝) 분 — 정오가 지나면 그날 인사는 건너뛴다
const MORNING: (u32, u32) = (9 * 60, 12 * 60);
/// 점심시간 [시작, 끝) 분
const LUNCH: (u32, u32) = (11 * 60 + 30, 13 * 60);

/// 지역 시각 한 순간: day는 날짜마다 다른 번호 (앱은 서기 1년부터 센 날수), minute은 자정부터 분
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalMoment {
    pub day: i32,
    pub minute: u32,
}

impl LocalMoment {
    pub fn new(day: i32, hour: u32, minute: u32) -> Self {
        Self { day, minute: hour * 60 + minute }
    }

    /// 00:00–06:00
    pub fn is_sleepy(self) -> bool {
        self.minute < SLEEPY_UNTIL
    }
}

/// 오늘의 인사를 이미 했는지 — 설정에 남겨 앱을 다시 켜도 하루 한 번이다
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyMarks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub morning: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lunch: Option<i32>,
}

impl DailyMarks {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn morning_due(&self, m: LocalMoment) -> bool {
        (MORNING.0..MORNING.1).contains(&m.minute) && self.morning != Some(m.day)
    }

    pub fn lunch_due(&self, m: LocalMoment) -> bool {
        (LUNCH.0..LUNCH.1).contains(&m.minute) && self.lunch != Some(m.day)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleChange {
    Away,
    Back,
}

/// 유휴 시간 → 자리 비움 / 돌아옴 (바뀔 때 한 번씩)
#[derive(Debug, Clone, Copy, Default)]
pub struct IdleWatch {
    away: bool,
}

impl IdleWatch {
    pub fn is_away(&self) -> bool {
        self.away
    }

    /// idle = 마지막 입력 뒤 초. None = 이 OS에서는 알 수 없다 (자리를 비웠다고 보지 않는다)
    pub fn observe(&mut self, idle: Option<f64>) -> Option<IdleChange> {
        let idle = idle?;
        if !self.away && idle >= AWAY_AFTER_SECS {
            self.away = true;
            Some(IdleChange::Away)
        } else if self.away && idle < BACK_BELOW_SECS {
            self.away = false;
            Some(IdleChange::Back)
        } else {
            None
        }
    }
}

/// 집중 친구 길이 (메뉴에서 고른다, 설정에 남는 건 이것뿐)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusLength {
    /// 25분 집중 · 5분 쉬기
    #[default]
    Short,
    /// 50분 집중 · 10분 쉬기
    Long,
}

impl FocusLength {
    pub fn is_short(&self) -> bool {
        *self == Self::Short
    }

    pub fn focus_secs(self) -> f64 {
        match self {
            Self::Short => 25.0 * 60.0,
            Self::Long => 50.0 * 60.0,
        }
    }

    pub fn break_secs(self) -> f64 {
        match self {
            Self::Short => 5.0 * 60.0,
            Self::Long => 10.0 * 60.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPhase {
    Focus,
    Break,
}

/// 집중 ↔ 쉬기 타이머. now = 단조 증가하는 초
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pomodoro {
    pub length: FocusLength,
    pub phase: FocusPhase,
    started: f64,
}

impl Pomodoro {
    pub fn new(length: FocusLength, now: f64) -> Self {
        Self { length, phase: FocusPhase::Focus, started: now }
    }

    /// 다음 국면으로 넘어갔으면 그 국면. 컴퓨터가 오래 잠들었다 깨도 한 번만 넘어간다 (밀린 만큼 다다다 울리지 않게)
    pub fn tick(&mut self, now: f64) -> Option<FocusPhase> {
        if now - self.started < self.phase_secs() {
            return None;
        }
        self.started = now;
        self.phase = match self.phase {
            FocusPhase::Focus => FocusPhase::Break,
            FocusPhase::Break => FocusPhase::Focus,
        };
        Some(self.phase)
    }

    fn phase_secs(&self) -> f64 {
        match self.phase {
            FocusPhase::Focus => self.length.focus_secs(),
            FocusPhase::Break => self.length.break_secs(),
        }
    }

    /// 집중 중이면 진행률 0..=100 (쉬는 동안은 고리가 없다)
    pub fn ring(&self, now: f64) -> Option<u8> {
        (self.phase == FocusPhase::Focus).then(|| ((now - self.started) / self.phase_secs() * 100.0).clamp(0.0, 100.0) as u8)
    }
}

/// 앱이 1초에 한 번 넣어 주는 바깥 세상
#[derive(Debug, Clone, Copy)]
pub struct RhythmInput {
    /// 단조 증가하는 초 (집중 타이머)
    pub secs: f64,
    /// 지역 시각
    pub moment: LocalMoment,
    /// 마지막 입력 뒤 초 (WindowSource::idle_seconds)
    pub idle: Option<f64>,
    /// 사용자가 "모두 숨기기"로 감춰 두지 않았다
    pub visible: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RhythmOutput {
    /// DailyMarks가 바뀌었다 — 설정에 저장한다
    pub marks_changed: bool,
    /// 하우스 안에서 돌아온 사용자를 맞았다 — 하우스 창이 손을 흔든다
    pub house_wave: bool,
}

/// 웹뷰가 동물 위에 그리는 것 (`pet-presence`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Overlay {
    /// 졸린 시간의 수면 모자
    pub nightcap: bool,
    /// 집중 진행 고리 0..=100
    pub ring: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Rhythm {
    idle: IdleWatch,
    focus: Option<Pomodoro>,
}

impl Rhythm {
    /// 사용자가 자리를 비웠다
    pub fn is_away(&self) -> bool {
        self.idle.is_away()
    }

    pub fn focus(&self) -> Option<&Pomodoro> {
        self.focus.as_ref()
    }

    /// 집중을 (다시) 시작한다 — "focus!" 하고 구석으로 간다
    pub fn start_focus(&mut self, length: FocusLength, secs: f64, pet: &mut Pet) {
        self.focus = Some(Pomodoro::new(length, secs));
        pet.set_focus(true);
        pet.say_focus();
    }

    pub fn stop_focus(&mut self, pet: &mut Pet) {
        self.focus = None;
        pet.set_focus(false);
    }

    pub fn overlay(&self, input: &RhythmInput) -> Overlay {
        Overlay { nightcap: input.moment.is_sleepy(), ring: self.focus.and_then(|f| f.ring(input.secs)) }
    }

    pub fn tick(&mut self, input: &RhythmInput, pet: &mut Pet, world: &World, care: &mut Care, marks: &mut DailyMarks) -> RhythmOutput {
        let mut out = RhythmOutput::default();
        pet.set_sleepy(input.moment.is_sleepy());
        match self.idle.observe(input.idle) {
            Some(IdleChange::Away) => pet.set_user_away(true),
            Some(IdleChange::Back) => {
                pet.set_user_away(false);
                out.house_wave = pet.welcome_back(world) && pet.is_housed();
            }
            None => {}
        }
        if let Some(pomodoro) = self.focus.as_mut() {
            match pomodoro.tick(input.secs) {
                // 자리를 비운 동안엔 타이머만 조용히 돈다: 말도 기지개도 없이 잠만 잔다
                Some(FocusPhase::Break) => {
                    pet.set_focus(false);
                    if !self.idle.is_away() {
                        pet.focus_break();
                    }
                }
                Some(FocusPhase::Focus) => {
                    pet.set_focus(true);
                    if !self.idle.is_away() {
                        pet.say_focus();
                    }
                }
                None => {}
            }
        }
        let focusing = self.focus.is_some_and(|f| f.phase == FocusPhase::Focus);
        // 사용자가 보고 있을 때만, 집중을 방해하지 않게. 지금 못 하면 (공중·먹는 중…) 다음 초에 다시
        if input.visible && !self.idle.is_away() && !focusing {
            let m = input.moment;
            if marks.morning_due(m) && pet.good_morning() {
                marks.morning = Some(m.day);
                out.marks_changed = true;
            } else if marks.lunch_due(m) && pet.lunch_time() {
                care.lunch_hunger();
                pet.set_hungry(care.is_hungry());
                marks.lunch = Some(m.day);
                out.marks_changed = true;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: f64 = 60.0;

    #[test]
    fn ten_idle_minutes_mean_away_and_the_next_input_means_back() {
        let mut w = IdleWatch::default();
        assert_eq!(w.observe(Some(0.0)), None);
        assert_eq!(w.observe(Some(AWAY_AFTER_SECS - 1.0)), None);
        assert!(!w.is_away());
        assert_eq!(w.observe(Some(AWAY_AFTER_SECS)), Some(IdleChange::Away));
        assert!(w.is_away());
        assert_eq!(w.observe(Some(AWAY_AFTER_SECS + 300.0)), None, "only once");
        assert_eq!(w.observe(Some(BACK_BELOW_SECS - 0.5)), Some(IdleChange::Back));
        assert!(!w.is_away());
        assert_eq!(w.observe(Some(0.1)), None, "only once");
    }

    #[test]
    fn an_os_that_cannot_tell_is_never_away() {
        let mut w = IdleWatch::default();
        for _ in 0..3 {
            assert_eq!(w.observe(None), None);
        }
        assert!(!w.is_away());
    }

    #[test]
    fn sleepy_hours_are_midnight_to_six() {
        assert!(LocalMoment::new(1, 0, 0).is_sleepy());
        assert!(LocalMoment::new(1, 5, 59).is_sleepy());
        assert!(!LocalMoment::new(1, 6, 0).is_sleepy());
        assert!(!LocalMoment::new(1, 23, 59).is_sleepy());
    }

    #[test]
    fn morning_is_due_from_nine_until_noon_once_a_day() {
        let mut marks = DailyMarks::default();
        assert!(!marks.morning_due(LocalMoment::new(10, 8, 59)));
        assert!(marks.morning_due(LocalMoment::new(10, 9, 0)));
        assert!(marks.morning_due(LocalMoment::new(10, 11, 59)), "the first time seen that morning");
        assert!(!marks.morning_due(LocalMoment::new(10, 12, 0)), "past noon the greeting is skipped");
        assert!(!marks.morning_due(LocalMoment::new(10, 23, 0)));
        marks.morning = Some(10);
        assert!(!marks.morning_due(LocalMoment::new(10, 9, 1)));
        assert!(!marks.morning_due(LocalMoment::new(11, 8, 59)));
        assert!(marks.morning_due(LocalMoment::new(11, 9, 0)));
    }

    #[test]
    fn lunch_is_due_from_half_past_eleven_until_one_once_a_day() {
        let mut marks = DailyMarks::default();
        assert!(!marks.lunch_due(LocalMoment::new(10, 11, 29)));
        assert!(marks.lunch_due(LocalMoment::new(10, 11, 30)));
        assert!(marks.lunch_due(LocalMoment::new(10, 12, 59)));
        assert!(!marks.lunch_due(LocalMoment::new(10, 13, 0)));
        marks.lunch = Some(10);
        assert!(!marks.lunch_due(LocalMoment::new(10, 12, 0)));
        assert!(marks.lunch_due(LocalMoment::new(11, 12, 0)));
    }

    #[test]
    fn a_pomodoro_alternates_focus_and_break() {
        let mut p = Pomodoro::new(FocusLength::Short, 100.0);
        assert_eq!(p.phase, FocusPhase::Focus);
        assert_eq!(p.tick(100.0 + 25.0 * MIN - 1.0), None);
        assert_eq!(p.tick(100.0 + 25.0 * MIN), Some(FocusPhase::Break));
        assert_eq!(p.tick(100.0 + 30.0 * MIN - 1.0), None);
        assert_eq!(p.tick(100.0 + 30.0 * MIN), Some(FocusPhase::Focus));
        assert_eq!(p.tick(100.0 + 30.0 * MIN + 1.0), None);
    }

    #[test]
    fn the_long_pomodoro_is_fifty_and_ten() {
        let mut p = Pomodoro::new(FocusLength::Long, 0.0);
        assert_eq!(p.tick(49.0 * MIN), None);
        assert_eq!(p.tick(50.0 * MIN), Some(FocusPhase::Break));
        assert_eq!(p.tick(59.0 * MIN), None);
        assert_eq!(p.tick(60.0 * MIN), Some(FocusPhase::Focus));
    }

    #[test]
    fn a_long_gap_moves_only_one_phase_on() {
        let mut p = Pomodoro::new(FocusLength::Short, 0.0);
        assert_eq!(p.tick(600.0 * MIN), Some(FocusPhase::Break));
        assert_eq!(p.tick(600.0 * MIN + 1.0), None, "the break starts now, nothing piles up");
    }

    #[test]
    fn the_ring_fills_during_focus_only() {
        let mut p = Pomodoro::new(FocusLength::Short, 10.0);
        assert_eq!(p.ring(10.0), Some(0));
        assert_eq!(p.ring(10.0 + 12.5 * MIN), Some(50));
        assert_eq!(p.ring(10.0 + 25.0 * MIN), Some(100));
        p.tick(10.0 + 25.0 * MIN);
        assert_eq!(p.ring(10.0 + 26.0 * MIN), None);
    }

    #[test]
    fn focus_length_is_short_by_default_and_snake_case() {
        assert_eq!(FocusLength::default(), FocusLength::Short);
        assert_eq!(serde_json::to_string(&FocusLength::Long).unwrap(), "\"long\"");
    }
}
