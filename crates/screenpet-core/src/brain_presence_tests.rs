//! 곁에 있기와 하루 리듬 (presence.rs, brain_presence.rs) — 가짜 시계와 유휴 시간으로 끝까지 돌려 본다.

use super::tests::{place, sim_cfg, w1, world};
use super::*;
use crate::care::Care;
use crate::geom::Rect;
use crate::presence::{DailyMarks, FocusLength, LocalMoment, Rhythm, RhythmInput, AWAY_AFTER_SECS};

const DT: f64 = 1.0 / 60.0;
const AFTERNOON: (u32, u32) = (15, 0);

fn resting() -> State {
    State::Idle { remaining: 100.0, look: false }
}

fn on_ground(c: PetConfig, x: f64) -> (Pet, World) {
    let w = world(vec![]);
    let mut p = Pet::new(c, 0.0, 0.0, 7);
    place(&mut p, &w, Owner::Ground(0), x, resting());
    (p, w)
}

fn flyer() -> PetConfig {
    PetConfig { flies: true, ..sim_cfg() }
}

/// 가짜 세상: 시각·유휴 시간을 테스트가 정한다. 1초에 한 번 Rhythm::tick, 그 사이 60 틱
struct Sim {
    rhythm: Rhythm,
    care: Care,
    marks: DailyMarks,
    secs: f64,
    day: i32,
    time: (u32, u32),
    idle: Option<f64>,
    visible: bool,
    said: Vec<Speech>,
    house_waves: u32,
}

impl Sim {
    fn new() -> Self {
        // 오늘 아침 인사는 이미 했다 (따로 시험하는 테스트만 지운다)
        Self {
            rhythm: Rhythm::default(),
            care: Care::default(),
            marks: DailyMarks { morning: Some(1), lunch: Some(1) },
            secs: 0.0,
            day: 1,
            time: AFTERNOON,
            idle: Some(0.0),
            visible: true,
            said: Vec::new(),
            house_waves: 0,
        }
    }

    fn input(&self) -> RhythmInput {
        RhythmInput { secs: self.secs, moment: LocalMoment::new(self.day, self.time.0, self.time.1), idle: self.idle, visible: self.visible }
    }

    fn second(&mut self, p: &mut Pet, w: &World) {
        let input = self.input();
        let out = self.rhythm.tick(&input, p, w, &mut self.care, &mut self.marks);
        self.house_waves += out.house_wave as u32;
        for _ in 0..60 {
            p.update(DT, w);
            if let Some(s) = p.take_speech() {
                self.said.push(s);
            }
        }
        self.secs += 1.0;
        if let Some(i) = self.idle.as_mut() {
            *i += 1.0;
        }
    }

    fn run(&mut self, p: &mut Pet, w: &World, secs: u32) {
        for _ in 0..secs {
            self.second(p, w);
        }
    }

    fn has_said(&self, s: Speech) -> bool {
        self.said.contains(&s)
    }

    fn count(&self, s: Speech) -> usize {
        self.said.iter().filter(|&&x| x == s).count()
    }
}

/// 10분 넘게 입력이 없으면 잠든다
fn sim_away(sim: &mut Sim, p: &mut Pet, w: &World) {
    sim.idle = Some(AWAY_AFTER_SECS);
    sim.run(p, w, 3);
}

// ---- 자리 비움 / 돌아옴 ----

#[test]
fn after_ten_idle_minutes_the_pet_sleeps_and_stays_asleep() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.idle = Some(AWAY_AFTER_SECS - 5.0);
    sim.run(&mut p, &w, 3);
    assert!(!matches!(p.state, State::Sleep { .. }), "not yet: {:?}", p.state);
    sim.run(&mut p, &w, 4);
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    for _ in 0..600 {
        sim.second(&mut p, &w);
        assert!(matches!(p.state, State::Sleep { .. }), "stays asleep: {:?}", p.state);
    }
    assert_eq!(p.render().anim, Anim::Sleep);
    assert!(sim.said.is_empty(), "sleeps quietly: {:?}", sim.said);
}

#[test]
fn a_walking_pet_finishes_on_the_spot_and_a_napping_pet_stays_napping() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.state = State::Move { kind: Move::Walk, remaining: 100.0 };
    let mut sim = Sim::new();
    sim_away(&mut sim, &mut p, &w);
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    let (mut q, w) = on_ground(sim_cfg(), 800.0);
    q.state = State::Sleep { remaining: 3.0 };
    let mut sim = Sim::new();
    sim_away(&mut sim, &mut q, &w);
    sim.run(&mut q, &w, 120);
    assert!(matches!(q.state, State::Sleep { .. }), "{:?}", q.state);
}

#[test]
fn no_cursor_play_or_friends_while_the_user_is_away() {
    let (mut p, _w) = on_ground(sim_cfg(), 800.0);
    p.set_user_away(true);
    assert!(!p.social_free());
    p.set_user_away(false);
    assert!(p.social_free());
}

#[test]
fn on_return_the_pet_runs_to_the_cursor_and_bounces_happily() {
    let (mut p, w) = on_ground(sim_cfg(), 200.0);
    let mut sim = Sim::new();
    sim_away(&mut sim, &mut p, &w);
    p.hover(1500.0, 500.0, false);
    sim.idle = Some(0.0);
    sim.second(&mut p, &w);
    assert!(sim.has_said(Speech::WelcomeBack) || matches!(p.state, State::PresenceTrip), "{:?}", p.state);
    // 달리는 동안의 모습
    let (mut q, _) = on_ground(sim_cfg(), 200.0);
    q.hover(1500.0, 500.0, false);
    assert!(q.welcome_back(&w));
    assert_eq!(q.state, State::PresenceTrip);
    assert_eq!(q.render().anim, Anim::Run);
    assert!(!q.render().flip, "faces the cursor");
    // 끝까지
    sim.run(&mut p, &w, 6);
    assert_eq!(sim.count(Speech::WelcomeBack), 1, "{:?}", sim.said);
    assert!((p.body.x - 1500.0).abs() < 1.5 * p.cfg.size, "near the cursor: {}", p.body.x);
    assert_eq!(p.body.y, 1040.0, "back on the floor");
}

#[test]
fn the_welcome_is_two_bounces_with_a_heart_each() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.hover(800.0, 900.0, false);
    assert!(p.welcome_back(&w), "already near: bounces right there");
    assert_eq!(p.take_speech(), Some(Speech::WelcomeBack));
    let (mut hearts, mut hops, mut was_heart, mut was_air) = (0, 0, false, false);
    assert!(p.render().heart);
    for _ in 0..300 {
        let r = p.render();
        if r.heart && !was_heart {
            hearts += 1;
        }
        let air = matches!(p.state, State::Air { .. });
        if air && !was_air {
            hops += 1;
        }
        was_heart = r.heart;
        was_air = air;
        p.update(DT, &w);
    }
    assert_eq!(hops, 2);
    assert_eq!(hearts, 2);
    assert!(p.standing.is_some());
    assert_eq!(p.take_speech(), None, "says it once");
}

#[test]
fn the_pet_leaps_to_another_monitor_when_the_cursor_is_there() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 0.0, 3840.0, 1040.0)], 128.0);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 400.0, resting());
    p.hover(3000.0, 500.0, false);
    assert!(p.welcome_back(&w));
    assert_eq!(p.state, State::PresenceTrip);
    assert_eq!(p.render().anim, Anim::Hop);
    let mut top = p.body.y;
    let mut t = 0.0;
    while matches!(p.state, State::PresenceTrip) && t < 3.0 {
        p.update(DT, &w);
        top = top.min(p.body.y);
        t += DT;
    }
    assert!(t < 3.0);
    assert!(top < 1040.0 - 100.0, "an arc: {top}");
    // 도착하자마자 통통 — 내려서면
    for _ in 0..120 {
        p.update(DT, &w);
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(1)));
    assert!((p.body.x - 3000.0).abs() < 1.5 * p.cfg.size, "{}", p.body.x);
}

#[test]
fn a_pet_on_a_window_comes_down_to_the_cursor_floor() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Window(1), 800.0, resting());
    p.hover(1700.0, 900.0, false);
    assert!(p.welcome_back(&w));
    for _ in 0..300 {
        p.update(DT, &w);
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
}

#[test]
fn a_flyer_glides_to_the_cursor_and_hovers_with_a_heart() {
    let w = world(vec![]);
    let mut p = Pet::new(flyer(), 200.0, 500.0, 3);
    p.state = State::Sleep { remaining: 100.0 };
    p.hover(1500.0, 400.0, false);
    assert!(p.welcome_back(&w));
    assert_eq!(p.state, State::PresenceTrip);
    assert_eq!(p.render().anim, Anim::Walk);
    let mut said = false;
    for _ in 0..240 {
        p.update(DT, &w);
        said |= p.take_speech() == Some(Speech::WelcomeBack);
        if said {
            break;
        }
    }
    assert!(said);
    assert!(p.render().heart);
    assert!((p.body.x - 1500.0).abs() < 1.5 * p.cfg.size);
    assert!(p.standing.is_none(), "stays in the air");
}

#[test]
fn a_housed_pet_greets_from_the_house() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.enter_house();
    let mut sim = Sim::new();
    sim_away(&mut sim, &mut p, &w);
    assert!(p.is_housed());
    sim.idle = Some(0.0);
    sim.second(&mut p, &w);
    assert_eq!(sim.house_waves, 1);
    assert!(sim.has_said(Speech::WelcomeBack));
    assert!(p.is_housed());
}

#[test]
fn control_mode_and_held_pets_are_left_alone() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.set_control(true);
    let mut sim = Sim::new();
    sim_away(&mut sim, &mut p, &w);
    assert!(!matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    sim.idle = Some(0.0);
    p.hover(1500.0, 500.0, false);
    sim.second(&mut p, &w);
    assert!(!sim.has_said(Speech::WelcomeBack));
    let (mut q, w) = on_ground(sim_cfg(), 800.0);
    q.pointer_down(800.0, 1000.0);
    assert!(!q.welcome_back(&w));
}

#[test]
fn being_picked_up_stops_the_run() {
    let (mut p, w) = on_ground(sim_cfg(), 200.0);
    p.hover(1500.0, 500.0, false);
    assert!(p.welcome_back(&w));
    p.update(DT, &w);
    p.pointer_down(p.body.x, p.body.y - 50.0);
    p.pointer_move(p.body.x + 100.0, p.body.y - 50.0);
    assert!(p.is_held());
    p.update(DT, &w);
    let (cx, cy) = (p.body.x, p.body.y);
    p.pointer_up(cx, cy);
    for _ in 0..300 {
        p.update(DT, &w);
        assert_ne!(p.state, State::PresenceTrip);
    }
    assert!(p.standing.is_some());
}

// ---- 졸린 시간 ----

/// 동물이 secs초 동안 잔 시간의 몫과 말
fn sleep_share(time: (u32, u32), secs: u32) -> (f64, Sim) {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    p.state = State::Idle { remaining: 0.1, look: false };
    let mut sim = Sim::new();
    sim.time = time;
    let mut asleep = 0;
    for _ in 0..secs {
        sim.idle = Some(0.0);
        sim.second(&mut p, &w);
        asleep += matches!(p.state, State::Sleep { .. }) as u32;
    }
    (asleep as f64 / secs as f64, sim)
}

#[test]
fn after_midnight_the_pet_sleeps_much_more() {
    let (night, _) = sleep_share((2, 0), 1800);
    let (day, _) = sleep_share(AFTERNOON, 1800);
    assert!(night > 0.5, "{night}");
    assert!(night > day * 2.0, "{night} vs {day}");
}

#[test]
fn after_midnight_the_pet_yawns_and_nags_only_at_night() {
    let (_, night) = sleep_share((2, 0), 1800);
    assert!(night.has_said(Speech::Yawn), "{:?}", night.said);
    assert!(night.has_said(Speech::GoToBed), "{:?}", night.said);
    assert!(night.count(Speech::GoToBed) <= 8, "occasionally: {}", night.count(Speech::GoToBed));
    let (_, day) = sleep_share(AFTERNOON, 1800);
    assert!(!day.has_said(Speech::Yawn));
    assert!(!day.has_said(Speech::GoToBed));
}

#[test]
fn the_nightcap_is_on_only_in_the_sleepy_hours() {
    let mut sim = Sim::new();
    sim.time = (5, 59);
    assert!(sim.rhythm.overlay(&sim.input()).nightcap);
    sim.time = (6, 0);
    assert!(!sim.rhythm.overlay(&sim.input()).nightcap);
}

// ---- 아침 ----

#[test]
fn good_morning_is_a_big_stretch_once_a_day() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks::default();
    sim.time = (8, 59);
    sim.second(&mut p, &w);
    assert!(!sim.has_said(Speech::GoodMorning));
    sim.time = (9, 0);
    let input = sim.input();
    let out = sim.rhythm.tick(&input, &mut p, &w, &mut sim.care, &mut sim.marks);
    assert!(out.marks_changed, "saved");
    assert_eq!(p.take_speech(), Some(Speech::GoodMorning));
    assert_eq!(p.render().anim, Anim::Stretch);
    assert_eq!(sim.marks.morning, Some(1));
    sim.run(&mut p, &w, 30);
    assert_eq!(sim.count(Speech::GoodMorning), 0, "not again today");
    sim.day = 2;
    sim.run(&mut p, &w, 8);
    assert_eq!(sim.count(Speech::GoodMorning), 1, "tomorrow");
}

#[test]
fn good_morning_waits_until_the_pet_is_seen() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks::default();
    sim.time = (9, 30);
    sim.visible = false;
    sim.run(&mut p, &w, 3);
    assert!(!sim.has_said(Speech::GoodMorning), "hidden");
    assert_eq!(sim.marks.morning, None);
    sim.visible = true;
    sim_away(&mut sim, &mut p, &w);
    assert!(!sim.has_said(Speech::GoodMorning), "user away");
    assert_eq!(sim.marks.morning, None);
    // 돌아오면 먼저 "you're back!!", 그 다음 아침 인사
    p.hover(800.0, 900.0, false);
    sim.idle = Some(0.0);
    sim.run(&mut p, &w, 8);
    let (a, b) = (sim.said.iter().position(|&s| s == Speech::WelcomeBack), sim.said.iter().position(|&s| s == Speech::GoodMorning));
    assert!(a.is_some() && b.is_some() && a < b, "{:?}", sim.said);
}

#[test]
fn good_morning_waits_for_the_pet_to_land() {
    let (mut p, _) = on_ground(sim_cfg(), 800.0);
    p.state = State::Air { anim: Anim::Fall };
    assert!(!p.good_morning());
    assert_eq!(p.take_speech(), None);
}

// ---- 점심 ----

#[test]
fn lunch_time_makes_a_full_pet_hungry_once_a_day() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks { morning: Some(1), lunch: None };
    assert!(!sim.care.is_hungry());
    sim.time = (11, 29);
    sim.second(&mut p, &w);
    assert!(!sim.care.is_hungry());
    sim.time = (11, 30);
    sim.second(&mut p, &w);
    assert!(sim.has_said(Speech::LunchTime));
    assert!(sim.care.is_hungry());
    assert!(p.hungry);
    assert_eq!(sim.marks.lunch, Some(1));
    // 밥을 먹이면 오늘은 다시 조르지 않는다
    sim.care.fullness = 1.0;
    p.set_hungry(false);
    sim.run(&mut p, &w, 20);
    assert_eq!(sim.count(Speech::LunchTime), 1);
    assert!(!sim.care.is_hungry());
}

#[test]
fn lunch_time_is_said_when_already_hungry_and_keeps_it_hungry() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks { morning: Some(1), lunch: None };
    sim.care.fullness = 0.0;
    sim.time = (12, 0);
    sim.second(&mut p, &w);
    assert!(sim.has_said(Speech::LunchTime));
    assert_eq!(sim.care.fullness, 0.0, "not refilled");
}

#[test]
fn outside_the_window_there_is_no_lunch() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks { morning: Some(1), lunch: None };
    sim.time = (13, 0);
    sim.run(&mut p, &w, 5);
    assert!(!sim.has_said(Speech::LunchTime));
    assert!(!sim.care.is_hungry());
}

// ---- 집중 친구 ----

fn start_focus(sim: &mut Sim, p: &mut Pet, length: FocusLength) {
    sim.rhythm.start_focus(length, sim.secs, p);
}

/// 쉬기 동안에도 사용자는 앞에 있다
fn keep_busy(sim: &mut Sim, p: &mut Pet, w: &World, secs: u32) {
    for _ in 0..secs {
        sim.idle = Some(0.0);
        sim.second(p, w);
    }
}

#[test]
fn focusing_pets_go_to_the_bottom_right_corner_and_sit_quietly() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Short);
    assert_eq!(p.take_speech(), Some(Speech::Focus));
    keep_busy(&mut sim, &mut p, &w, 8);
    let corner = 1920.0 - 0.75 * p.cfg.size;
    assert!((p.body.x - corner).abs() <= 4.0, "{} vs {corner}", p.body.x);
    assert_eq!(p.body.y, 1040.0);
    sim.said.clear();
    for _ in 0..300 {
        // 커서로 간질여도 조용하다
        p.hover(p.body.x + (sim.secs as i64 % 2) as f64 * 40.0, p.body.y - 50.0, true);
        sim.idle = Some(0.0);
        sim.second(&mut p, &w);
        assert!((p.body.x - corner).abs() <= 4.0, "stays: {}", p.body.x);
        assert_eq!(p.render().anim, Anim::Idle, "{:?}", p.state);
    }
    assert!(sim.said.is_empty(), "{:?}", sim.said);
    assert!(!p.social_free());
}

#[test]
fn the_ring_shows_progress_only_while_focusing() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    assert_eq!(sim.rhythm.overlay(&sim.input()).ring, None);
    start_focus(&mut sim, &mut p, FocusLength::Short);
    assert_eq!(sim.rhythm.overlay(&sim.input()).ring, Some(0));
    sim.secs += 12.5 * 60.0;
    assert_eq!(sim.rhythm.overlay(&sim.input()).ring, Some(50));
    sim.secs += 13.0 * 60.0;
    sim.second(&mut p, &w);
    assert_eq!(sim.rhythm.overlay(&sim.input()).ring, None, "break");
}

#[test]
fn when_focus_ends_the_pet_stretches_and_plays_and_then_goes_back() {
    let (mut p, w) = on_ground(sim_cfg(), 1800.0);
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Short);
    p.take_speech();
    keep_busy(&mut sim, &mut p, &w, 5);
    sim.said.clear();
    sim.secs = 25.0 * 60.0;
    let input = sim.input();
    sim.rhythm.tick(&input, &mut p, &w, &mut sim.care, &mut sim.marks);
    assert_eq!(p.take_speech(), Some(Speech::BreakTime));
    assert_eq!(p.render().anim, Anim::Stretch);
    assert!(p.render().heart);
    // 쉬는 동안엔 평소대로 논다
    let corner_x = p.body.x;
    let mut moved = false;
    for _ in 0..240 {
        sim.idle = Some(0.0);
        sim.second(&mut p, &w);
        moved |= (p.body.x - corner_x).abs() > 10.0;
    }
    assert!(moved, "plays normally during the break");
    assert!(p.social_free() || !matches!(p.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. }));
    // 다시 집중
    sim.said.clear();
    sim.secs = 30.0 * 60.0;
    keep_busy(&mut sim, &mut p, &w, 10);
    assert_eq!(sim.count(Speech::Focus), 1, "{:?}", sim.said);
    assert!((p.body.x - (1920.0 - 0.75 * p.cfg.size)).abs() <= 4.0, "{}", p.body.x);
}

#[test]
fn stopping_focus_lets_the_pet_wander_again() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Long);
    keep_busy(&mut sim, &mut p, &w, 8);
    let x = p.body.x;
    sim.rhythm.stop_focus(&mut p);
    assert!(sim.rhythm.focus().is_none());
    assert_eq!(sim.rhythm.overlay(&sim.input()).ring, None);
    let mut moved = false;
    for _ in 0..240 {
        sim.idle = Some(0.0);
        sim.second(&mut p, &w);
        moved |= (p.body.x - x).abs() > 10.0;
    }
    assert!(moved);
}

#[test]
fn a_flyer_perches_in_the_corner_while_focusing() {
    let w = world(vec![]);
    let mut p = Pet::new(flyer(), 300.0, 500.0, 3);
    p.state = resting();
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Short);
    keep_busy(&mut sim, &mut p, &w, 8);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    let x = p.body.x;
    keep_busy(&mut sim, &mut p, &w, 120);
    assert_eq!(p.body.x, x);
    assert_eq!(p.body.y, 1040.0);
    // 끝나면 다시 날아오른다
    sim.rhythm.stop_focus(&mut p);
    let mut flew = false;
    for _ in 0..240 {
        sim.idle = Some(0.0);
        sim.second(&mut p, &w);
        flew |= p.standing.is_none();
    }
    assert!(flew);
}

#[test]
fn a_dragged_pet_is_dropped_and_returns_to_the_corner() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Short);
    keep_busy(&mut sim, &mut p, &w, 8);
    p.pointer_down(p.body.x, p.body.y - 50.0);
    p.pointer_move(700.0, 300.0);
    assert!(p.is_held());
    p.update(DT, &w);
    assert!(p.is_held(), "the user comes first");
    p.pointer_up(700.0, 300.0);
    keep_busy(&mut sim, &mut p, &w, 10);
    assert!((p.body.x - (1920.0 - 0.75 * p.cfg.size)).abs() <= 4.0, "{}", p.body.x);
}

#[test]
fn control_mode_beats_focus_and_the_pet_returns_afterwards() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    p.set_control(true);
    start_focus(&mut sim, &mut p, FocusLength::Short);
    keep_busy(&mut sim, &mut p, &w, 5);
    assert!((p.body.x - 300.0).abs() < 1.0, "stays under the keys: {}", p.body.x);
    p.set_control(false);
    keep_busy(&mut sim, &mut p, &w, 10);
    assert!((p.body.x - (1920.0 - 0.75 * p.cfg.size)).abs() <= 4.0, "{}", p.body.x);
}

#[test]
fn food_comes_first_while_focusing() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    start_focus(&mut sim, &mut p, FocusLength::Short);
    keep_busy(&mut sim, &mut p, &w, 8);
    assert!(p.feed_result(FeedResult::Ate { leveled_up: false }, Food::Snack));
    let mut ate = false;
    for _ in 0..1200 {
        p.update(DT, &w);
        ate |= matches!(p.state, State::Eating { .. });
    }
    assert!(ate);
    assert!((p.body.x - (1920.0 - 0.75 * p.cfg.size)).abs() <= 4.0, "back in the corner: {}", p.body.x);
}

#[test]
fn focus_keeps_morning_and_lunch_and_the_idle_welcome_waiting() {
    let (mut p, w) = on_ground(sim_cfg(), 300.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks::default();
    sim.time = (11, 45);
    start_focus(&mut sim, &mut p, FocusLength::Short);
    p.take_speech();
    keep_busy(&mut sim, &mut p, &w, 5);
    assert!(sim.said.is_empty(), "{:?}", sim.said);
    assert_eq!(sim.marks, DailyMarks::default());
    // 10분 넘게 입력이 없으면 잠만 자고, 돌아오면 깨지만 뛰쳐나와 맞이하지는 않는다 (집중 중이니까)
    sim.idle = Some(AWAY_AFTER_SECS);
    sim.run(&mut p, &w, 5);
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    sim.idle = Some(0.0);
    sim.second(&mut p, &w);
    assert!(!matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
    assert!(!sim.has_said(Speech::WelcomeBack));
    // 쉬는 시간이 되면 밀린 인사
    sim.secs = 25.0 * 60.0;
    keep_busy(&mut sim, &mut p, &w, 20);
    assert!(sim.has_said(Speech::GoodMorning), "{:?}", sim.said);
    assert!(sim.has_said(Speech::LunchTime), "{:?}", sim.said);
}

#[test]
fn nothing_changes_for_guests() {
    let w = world(vec![]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 3);
    place(&mut p, &w, Owner::Ground(0), 800.0, resting());
    p.set_guest(crate::visit::Side::Left, 1000.0);
    assert!(!p.welcome_back(&w));
    assert!(!p.good_morning());
    assert!(!p.lunch_time());
}

// ---- 좁은 화면 ----

#[test]
fn welcome_back_on_a_work_area_narrower_than_the_pet_does_not_panic() {
    let c = sim_cfg();
    let narrow = Rect::new(0.0, 0.0, c.half_w, 1040.0);
    let w = World::new(vec![], vec![narrow], c.size);
    let mut p = Pet::new(c, 0.0, 1040.0, 7);
    p.state = resting();
    p.hover(c.half_w / 2.0, 500.0, false);
    let _ = p.welcome_back(&w);
}

// ---- 자리 비움 알려 주기 ----

#[test]
fn the_rhythm_tells_whether_the_user_is_away() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    assert!(!sim.rhythm.is_away());
    sim_away(&mut sim, &mut p, &w);
    assert!(sim.rhythm.is_away());
    sim.idle = Some(0.0);
    sim.second(&mut p, &w);
    assert!(!sim.rhythm.is_away());
}

// ---- 아침은 정오까지 ----

#[test]
fn past_noon_the_morning_greeting_is_skipped_for_the_day() {
    let (mut p, w) = on_ground(sim_cfg(), 800.0);
    let mut sim = Sim::new();
    sim.marks = DailyMarks { morning: None, lunch: Some(1) };
    sim.time = (12, 0);
    sim.run(&mut p, &w, 3);
    assert_eq!(sim.count(Speech::GoodMorning), 0);
    sim.time = (23, 0);
    sim.run(&mut p, &w, 3);
    assert_eq!(sim.count(Speech::GoodMorning), 0, "does not greet at 23:00");
    assert_eq!(sim.marks.morning, None);
    sim.day = 2;
    sim.marks.lunch = Some(2);
    sim.time = (11, 59);
    sim.run(&mut p, &w, 8);
    assert_eq!(sim.count(Speech::GoodMorning), 1, "the next morning");
}
