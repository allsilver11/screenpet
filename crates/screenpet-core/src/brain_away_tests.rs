//! 자리를 비운 동안엔 잠만 잔다 (brain_presence.rs) — 가짜 시계로 두 시간을 돌려 본다.

use super::tests::{place, sim_cfg, world};
use super::*;
use crate::brain::control_moves::ControlInput;
use crate::care::Care;
use crate::geom::{Rect, WindowInfo};
use crate::presence::{DailyMarks, FocusLength, FocusPhase, LocalMoment, Rhythm, RhythmInput, AWAY_AFTER_SECS};
use crate::brain::social_moves::GreetKind;
use crate::brain::snow_moves::SnowPhase;
use crate::social::Social;
use crate::visit::HOMESICK_HARD_CAP_SECS;

const DT: f64 = 1.0 / 60.0;
const TWO_HOURS: u32 = 2 * 60 * 60;

fn walker() -> PetConfig {
    PetConfig { can_climb: true, looks_up: true, snow_cloud: true, ..sim_cfg() }
}

fn flyer() -> PetConfig {
    PetConfig { flies: true, looks_up: true, ..sim_cfg() }
}

fn frog() -> PetConfig {
    PetConfig { hops: true, special: None, ..sim_cfg() }
}

fn dino() -> PetConfig {
    PetConfig { special: Some(Special::Stomp), ..sim_cfg() }
}

fn window(id: u64, top: f64) -> WindowInfo {
    WindowInfo { id, rect: Rect::new(400.0, top, 1400.0, top + 300.0) }
}

/// 유혹을 켠 동물: 배고프고, 커서가 코앞에 멈춰 있고, 눈이 쌓였다
fn tempted(c: PetConfig, w: &World) -> Pet {
    let mut p = Pet::new(c, 0.0, 0.0, 11);
    place(&mut p, w, Owner::Ground(0), 800.0, State::Move { kind: Move::Walk, remaining: 100.0 });
    if p.cfg.flies {
        p.standing = None;
        p.body.y -= 300.0;
    }
    p.set_hungry(true);
    p.set_snow_ready(true);
    p.cursor_input(false, false, true);
    p
}

/// 1초에 한 번 Rhythm::tick, 그 사이 60 틱 — 내 동물의 하루
struct Sim {
    rhythm: Rhythm,
    care: Care,
    marks: DailyMarks,
    secs: f64,
    /// 시작 시각 (분)
    start: u32,
    idle: f64,
    said: Vec<Speech>,
}

impl Sim {
    fn at(hour: u32, minute: u32) -> Self {
        Self { rhythm: Rhythm::default(), care: Care::default(), marks: DailyMarks::default(), secs: 0.0, start: hour * 60 + minute, idle: 0.0, said: Vec::new() }
    }

    fn input(&self) -> RhythmInput {
        let minute = self.start + (self.secs / 60.0) as u32;
        RhythmInput { secs: self.secs, moment: LocalMoment::new(1, minute / 60, minute % 60), idle: Some(self.idle), visible: true }
    }

    fn tick_rhythm(&mut self, p: &mut Pet, w: &World) {
        let input = self.input();
        self.rhythm.tick(&input, p, w, &mut self.care, &mut self.marks);
    }

    /// 1초. each는 틱마다 (세상을 흔들거나 살펴본다)
    fn second(&mut self, p: &mut Pet, w: &World, mut each: impl FnMut(&Pet)) {
        self.tick_rhythm(p, w);
        for _ in 0..60 {
            let (cx, cy) = (p.body.x + 90.0, p.body.y - 200.0);
            p.hover(cx, cy, false);
            p.update(DT, w);
            if let Some(s) = p.take_speech() {
                self.said.push(s);
            }
            each(p);
        }
        self.secs += 1.0;
        self.idle += 1.0;
    }

    fn run(&mut self, p: &mut Pet, w: &World, secs: u32) {
        for _ in 0..secs {
            self.second(p, w, |_| {});
        }
    }

    /// 자리를 비운다: 다음 Rhythm::tick이 알아챈다 (하던 일은 몇 틱 안에 잠으로)
    fn walk_away(&mut self, p: &mut Pet, w: &World) {
        self.idle = AWAY_AFTER_SECS;
        self.second(p, w, |_| {});
        assert!(self.rhythm.is_away());
        self.said.clear();
    }
}

fn is_asleep(p: &Pet) -> bool {
    matches!(p.state, State::Sleep { .. })
}

/// 두 시간 내내 잠만 — 자세도 자리도 말도 그대로
fn sleeps_two_hours(name: &str, c: PetConfig, start: (u32, u32)) {
    let w = world(vec![]);
    let mut sim = Sim::at(start.0, start.1);
    let mut p = tempted(c, &w);
    sim.run(&mut p, &w, 20);
    sim.walk_away(&mut p, &w);
    // 친구가 있는 화면 끝도 나가는 길이 되지 않는다
    p.set_exit_edges(true, true);
    p.set_vertical_edges(true, true);
    let marks = sim.marks;
    // 공중이었으면 내려선 뒤
    for _ in 0..10 {
        if !is_asleep(&p) {
            sim.second(&mut p, &w, |_| {});
        }
    }
    assert!(is_asleep(&p), "{name}: asleep, was {:?}", p.state);
    let (x, y) = (p.body.x, p.body.y);
    let flies = p.cfg.flies;
    for _ in 0..TWO_HOURS {
        sim.second(&mut p, &w, |p| {
            assert!(is_asleep(p), "{name}: only sleep, was {:?}", p.state);
            let r = p.render();
            assert_eq!(r.anim, Anim::Sleep, "{name}");
            assert!(!r.cloud, "{name}: no snow cloud");
            assert_eq!(p.body.x, x, "{name}: stays put");
            // 떠서 자는 동물의 살랑임은 그대로 (그 자리에서)
            assert!(flies || p.body.y == y, "{name}: stays put");
            assert!((p.body.y - y).abs() < p.cfg.size, "{name}: stays put");
        });
    }
    assert_eq!(sim.said, vec![], "{name}: not a word");
    assert_eq!(sim.marks, marks, "{name}: daily lines wait for the user");
}

#[test]
fn two_hours_away_is_only_sleep_through_lunch() {
    for (name, c) in [("walker", walker()), ("flyer", flyer()), ("frog", frog()), ("dino", dino())] {
        sleeps_two_hours(name, c, (11, 0));
    }
}

#[test]
fn two_hours_away_is_only_sleep_through_bedtime_and_morning() {
    for (name, c) in [("walker", walker()), ("flyer", flyer()), ("frog", frog()), ("dino", dino())] {
        sleeps_two_hours(name, c, (7, 30));
    }
}

#[test]
fn a_perched_flyer_sleeps_on_its_perch_and_a_flying_one_dozes_where_it_is() {
    let w = world(vec![]);
    let mut sim = Sim::at(15, 0);
    let mut perched = Pet::new(flyer(), 0.0, 0.0, 3);
    place(&mut perched, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    sim.walk_away(&mut perched, &w);
    assert!(is_asleep(&perched));
    assert!(perched.standing.is_some(), "stays on the perch");

    let mut sim = Sim::at(15, 0);
    let mut flying = tempted(flyer(), &w);
    sim.run(&mut flying, &w, 5);
    let from = (flying.body.x, flying.body.y);
    sim.walk_away(&mut flying, &w);
    assert!(is_asleep(&flying));
    assert!(flying.flight.is_none(), "no long flight to find a perch");
    let moved = (flying.body.x - from.0).hypot(flying.body.y - from.1);
    assert!(moved < 2.0 * flying.cfg.size, "dozes about where it was: {moved}");
}

#[test]
fn whatever_it_was_doing_stops_for_sleep() {
    let w = world(vec![]);
    let states = [
        State::Trick { kind: Trick::Heart, remaining: 50.0 },
        State::Special { kind: Special::Stomp, remaining: 50.0 },
        State::Hungry { remaining: 50.0 },
        State::Huff { remaining: 50.0 },
        State::Tickled { remaining: 50.0 },
        State::Annoyed { remaining: 50.0, tickled_for: 0.0 },
        State::Turn { remaining: 50.0 },
        State::Land { remaining: 50.0 },
        State::Glare { remaining: 50.0 },
        State::Tumble { remaining: 50.0, dir: 1.0 },
        State::Climb { target_y: 100.0, hang: 1.0 },
        State::Slide,
        State::CursorHang { remaining: 50.0 },
        State::GoHome,
        State::Snowman { phase: SnowPhase::Roll, remaining: 50.0, dir: 1.0 },
    ];
    for state in states {
        for c in [walker(), flyer()] {
            let mut sim = Sim::at(15, 0);
            let mut p = tempted(c, &w);
            p.state = state;
            sim.idle = AWAY_AFTER_SECS;
            sim.tick_rhythm(&mut p, &w);
            // 벽이나 커서에 매달려 있었으면 떨어져 내린 뒤에
            for _ in 0..(10 * 60) {
                p.update(DT, &w);
                if is_asleep(&p) {
                    break;
                }
            }
            assert!(is_asleep(&p), "from {state:?}: {:?}", p.state);
            assert!(p.take_speech().is_none(), "no words from {state:?}");
            assert!(p.snowball_at().is_none());
        }
    }
}

#[test]
fn it_falls_when_dropped_and_goes_straight_back_to_sleep_on_landing() {
    let w = world(vec![]);
    let mut sim = Sim::at(15, 0);
    let mut p = tempted(walker(), &w);
    sim.walk_away(&mut p, &w);
    p.standing = None;
    p.body.y = 200.0;
    p.state = State::Air { anim: Anim::Fall };
    let floor = w.work_areas[0].bottom;
    let mut land_ticks = 0;
    for _ in 0..(10 * 60) {
        p.update(DT, &w);
        if matches!(p.state, State::Land { .. }) {
            land_ticks += 1;
        }
    }
    assert_eq!(p.body.y, floor);
    assert!(is_asleep(&p), "{:?}", p.state);
    assert!(land_ticks <= 1, "no pause after landing: {land_ticks}");
    assert!(p.take_speech().is_none());
}

#[test]
fn it_rides_a_moving_window_asleep_and_falls_asleep_again_when_the_window_closes() {
    let top = |y| world(vec![window(1, y)]);
    let w = top(500.0);
    let mut sim = Sim::at(15, 0);
    let mut p = Pet::new(walker(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 100.0, look: false });
    sim.walk_away(&mut p, &w);
    assert!(is_asleep(&p));

    // 창이 움직인다 (흔들어도 놀라지 않는다)
    for i in 0..600 {
        let y = 500.0 + if i % 8 < 4 { 60.0 } else { -60.0 };
        let moved = top(y);
        p.update(DT, &moved);
        assert_eq!(p.body.y, y, "rides the window");
        assert!(is_asleep(&p), "{:?}", p.state);
    }

    // 창이 닫힌다: 말없이 떨어져 그대로 잔다
    let gone = world(vec![]);
    p.update(DT, &gone);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    for _ in 0..(10 * 60) {
        p.update(DT, &gone);
    }
    assert!(is_asleep(&p), "{:?}", p.state);
    assert!(p.standing.is_some_and(|s| matches!(s.owner, Owner::Ground(_))));
    assert!(p.take_speech().is_none(), "no eek, no glare");
}

#[test]
fn coming_back_wakes_it_right_away_with_the_welcome() {
    for c in [walker(), flyer(), frog(), dino()] {
        let w = world(vec![]);
        let mut sim = Sim::at(15, 0);
        let mut p = tempted(c, &w);
        sim.walk_away(&mut p, &w);
        sim.run(&mut p, &w, 600);
        assert!(is_asleep(&p));
        p.cursor = (300.0, 900.0);
        sim.idle = 0.0;
        sim.tick_rhythm(&mut p, &w);
        assert!(!sim.rhythm.is_away());
        assert!(!is_asleep(&p), "awake on the same tick: {:?}", p.state);
        sim.run(&mut p, &w, 5);
        assert!(sim.said.contains(&Speech::WelcomeBack), "{:?}", sim.said);
    }
}

#[test]
fn coming_back_wakes_it_even_when_it_cannot_run_to_the_welcome() {
    // 맞이할 수 없는 때(집중)에도 잠은 바로 깬다
    let w = world(vec![]);
    let mut p = Pet::new(walker(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(is_asleep(&p));
    p.set_focus(true);
    p.set_user_away(false);
    assert!(!is_asleep(&p), "{:?}", p.state);
}

#[test]
fn a_click_as_the_first_input_is_not_a_sleeper_click_once_it_knows_you_are_back() {
    let w = world(vec![]);
    let mut p = Pet::new(walker(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    p.set_user_away(true);
    p.update(DT, &w);
    p.pointer_down(800.0, 900.0);
    p.set_user_away(false);
    p.pointer_up(800.0, 900.0);
    assert!(!matches!(p.state, State::Sleep { .. } | State::Annoyed { .. }), "{:?}", p.state);
}

// ---- 하우스 ----

#[test]
fn a_housed_pet_stays_quiet_and_is_welcomed_when_you_are_back() {
    let w = world(vec![]);
    let mut sim = Sim::at(11, 0);
    let mut p = tempted(walker(), &w);
    p.enter_house();
    sim.walk_away(&mut p, &w);
    sim.run(&mut p, &w, 3 * 60 * 60);
    assert!(p.is_housed());
    assert_eq!(sim.said, vec![], "not a word from the house");
    sim.idle = 0.0;
    sim.second(&mut p, &w, |_| {});
    assert!(sim.said.contains(&Speech::WelcomeBack), "{:?}", sim.said);
}

// ---- 친구 ----

/// 이 화면의 손님: 주인과 별개로 set_user_away만 받는다 (앱이 매 틱 넘겨 준다)
fn guest_on_screen(c: PetConfig, w: &World, homesick_after: f64) -> Pet {
    let mut p = tempted(c, w);
    p.set_guest(Side::Left, homesick_after);
    p
}

fn run_ticks(p: &mut Pet, w: &World, secs: f64, mut each: impl FnMut(&mut Pet)) {
    for _ in 0..(secs * 60.0) as u32 {
        p.update(DT, w);
        each(p);
    }
}

#[test]
fn guests_on_my_screen_sleep_too_and_wake_with_me() {
    for c in [walker(), flyer(), frog(), dino()] {
        let w = world(vec![]);
        let mut g = guest_on_screen(c, &w, 1e9);
        g.set_user_away(true);
        g.update(DT, &w);
        assert!(is_asleep(&g), "{:?}", g.state);
        run_ticks(&mut g, &w, 3600.0, |g| {
            assert!(is_asleep(g), "{:?}", g.state);
            assert!(g.take_speech().is_none());
        });
        g.set_user_away(false);
        assert!(!is_asleep(&g), "wakes at once: {:?}", g.state);
    }
}

#[test]
fn a_guest_that_arrives_while_i_am_away_walks_in_and_goes_to_sleep() {
    let w = world(vec![]);
    let mut g = Pet::new_entering(walker(), &w, Side::Left, 9);
    g.set_guest(Side::Left, 1e9);
    g.set_user_away(true);
    assert!(matches!(g.state, State::Enter { .. }));
    run_ticks(&mut g, &w, 30.0, |_| {});
    assert!(is_asleep(&g), "{:?}", g.state);
}

#[test]
fn a_sleeping_guest_ignores_soft_homesickness_but_the_hard_cap_sends_it_home() {
    let w = world(vec![]);
    let mut g = guest_on_screen(walker(), &w, 60.0);
    g.set_user_away(true);
    g.update(DT, &w);
    let x = g.body.x;
    let mut gone = None;
    let mut t = 0.0;
    while gone.is_none() && t < 60.0 + HOMESICK_HARD_CAP_SECS + 30.0 {
        g.update(DT, &w);
        t += DT;
        gone = g.take_exit();
        if t < 60.0 + HOMESICK_HARD_CAP_SECS - 1.0 {
            assert!(is_asleep(&g), "at {t}: {:?}", g.state);
            assert_eq!(g.body.x, x);
        }
    }
    assert_eq!(gone, Some(Side::Left), "goes home at the hard cap (after {t} s)");
    assert!(t >= 60.0 + HOMESICK_HARD_CAP_SECS - 1.0);
}

#[test]
fn the_owner_can_call_a_sleeping_guest_home() {
    let w = world(vec![]);
    let mut g = guest_on_screen(walker(), &w, 1e9);
    g.set_user_away(true);
    run_ticks(&mut g, &w, 5.0, |_| {});
    g.go_home_now();
    assert!(matches!(g.state, State::Farewell { .. }), "{:?}", g.state);
    let mut gone = None;
    run_ticks(&mut g, &w, 10.0, |g| gone = gone.or(g.take_exit()));
    assert_eq!(gone, Some(Side::Left));
}

#[test]
fn a_guest_its_owner_is_steering_obeys_and_sleeps_when_the_keys_stop() {
    let w = world(vec![]);
    let mut g = guest_on_screen(walker(), &w, 1e9);
    g.set_user_away(true);
    run_ticks(&mut g, &w, 2.0, |_| {});
    assert!(is_asleep(&g));
    let keys = ControlInput { right: true, ..Default::default() };
    let x = g.body.x;
    for _ in 0..120 {
        g.remote_keys(true, keys);
        g.update(DT, &w);
    }
    assert!(g.body.x > x, "the owner is at the keys, so it walks");
    g.remote_keys(false, ControlInput::default());
    run_ticks(&mut g, &w, 10.0, |_| {});
    assert!(is_asleep(&g), "{:?}", g.state);
}

#[test]
fn a_friend_can_still_take_the_sleeping_pet_and_it_sleeps_again_when_it_comes_home() {
    let w = world(vec![]);
    let mut sim = Sim::at(15, 0);
    let mut p = tempted(walker(), &w);
    sim.walk_away(&mut p, &w);
    sim.run(&mut p, &w, 60);
    assert!(is_asleep(&p));

    // "펫 데리고 오기": 자리 비움은 거절 이유가 아니다
    assert!(p.is_free_to_go(), "away alone does not refuse an invite");
    assert!(p.send_to(Side::Right, false));
    let mut gone = None;
    let mut waved = false;
    let mut said_bye = false;
    run_ticks(&mut p, &w, 5.0, |p| {
        waved |= matches!(p.state, State::Farewell { .. });
        said_bye |= p.take_speech() == Some(Speech::Bye);
        gone = gone.or(p.take_exit());
    });
    assert!(waved && said_bye, "wakes up, waves and poofs");
    assert_eq!(gone, Some(Side::Right));
    assert!(p.is_away(), "{:?}", p.state);

    // 사용자가 아직 없는데 돌아왔다: 걸어 들어와 바로 잔다
    p.enter(&w, Side::Right);
    run_ticks(&mut p, &w, 30.0, |_| {});
    assert!(is_asleep(&p), "{:?}", p.state);
    assert!(p.is_free_to_go());
}

#[test]
fn the_usual_reasons_still_refuse_an_invite_while_away() {
    let w = world(vec![]);
    let mut p = tempted(walker(), &w);
    p.set_user_away(true);
    p.update(DT, &w);
    assert!(p.is_free_to_go());
    p.enter_house();
    assert!(!p.is_free_to_go(), "in the house");

    let mut p = tempted(walker(), &w);
    p.set_user_away(true);
    p.update(DT, &w);
    p.pointer_down(800.0, 900.0);
    assert!(!p.is_free_to_go(), "held");

    let mut p = tempted(walker(), &w);
    p.set_user_away(true);
    p.state = State::Eating { remaining: 3.0, favorite: false, level_up: false };
    assert!(!p.is_free_to_go(), "eating");
}

// ---- 어울리기·쿵쾅·선물 ----

#[test]
fn sleepers_side_by_side_do_not_greet_or_get_stomped_awake() {
    let w = world(vec![]);
    let mut own = Pet::new(walker(), 0.0, 0.0, 1);
    place(&mut own, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    let mut guest = Pet::new(dino(), 0.0, 0.0, 2);
    place(&mut guest, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
    guest.set_guest(Side::Left, 1e9);
    let mut pets = vec![own, guest];
    let mut social = Social::new(4);
    for p in &mut pets {
        p.set_user_away(true);
    }
    for tick in 0..(300 * 60) {
        social.tick(DT, &mut pets);
        if tick == 60 {
            // 주인이 조종하는 공룡이 옆에서 쿵쾅해도 깨지 않는다
            pets[1].force_stomp();
        }
        for p in &mut pets {
            p.update(DT, &w);
        }
        assert!(is_asleep(&pets[0]), "{:?}", pets[0].state);
        assert!(!pets[0].wobbling());
        assert!(pets[0].take_speech().is_none());
        assert_eq!(social.active(), 0);
    }
}

#[test]
fn a_game_in_progress_ends_when_i_leave() {
    let w = world(vec![]);
    let mut a = Pet::new(walker(), 0.0, 0.0, 1);
    place(&mut a, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    let mut b = Pet::new(frog(), 0.0, 0.0, 2);
    place(&mut b, &w, Owner::Ground(0), 900.0, State::Idle { remaining: 100.0, look: false });
    a.start_greet(900.0, GreetKind::Hello, 5.0);
    b.start_greet(800.0, GreetKind::Hello, 5.0);
    let mut pets = vec![a, b];
    let mut social = Social::new(4);
    for p in &mut pets {
        p.set_user_away(true);
    }
    for _ in 0..60 {
        social.tick(DT, &mut pets);
        for p in &mut pets {
            p.update(DT, &w);
        }
    }
    assert!(is_asleep(&pets[0]) && is_asleep(&pets[1]), "{:?} {:?}", pets[0].state, pets[1].state);
    assert_eq!(social.active(), 0);
}

#[test]
fn no_gifts_while_away() {
    let w = world(vec![]);
    // 깨어 있는 자세여도 (손 흔들러 깬 동물 등) 선물은 없다
    let mut p = tempted(walker(), &w);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    assert!(crate::gift::drop_spot(&p, &w).is_some());
    p.set_user_away(true);
    assert!(crate::gift::drop_spot(&p, &w).is_none());
    p.set_user_away(false);
    assert!(crate::gift::drop_spot(&p, &w).is_some());
}

// ---- 집중 친구 ----

/// 집중 중에 자리를 비운 동물: 타이머만 조용히 돈다. 자리는 그대로
fn away_during_focus(c: PetConfig, w: &World) -> (Sim, Pet) {
    let mut sim = Sim::at(15, 0);
    let mut p = Pet::new(c, 0.0, 0.0, 5);
    place(&mut p, w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    sim.rhythm.start_focus(FocusLength::Short, sim.secs, &mut p);
    sim.walk_away(&mut p, w);
    p.take_speech();
    (sim, p)
}

fn phase(sim: &Sim) -> FocusPhase {
    sim.rhythm.focus().expect("the timer keeps running").phase
}

#[test]
fn away_the_focus_buddy_just_sleeps_where_it_is_through_every_segment() {
    let w = world(vec![]);
    for c in [walker(), flyer(), frog(), dino()] {
        let (mut sim, mut p) = away_during_focus(c, &w);
        let spot = (p.body.x, p.body.y);
        // 집중 25분 → 쉬기 5분 → 다시 집중 (타이머는 계속 돈다)
        for (secs, want) in [(25 * 60 - 10, FocusPhase::Focus), (20, FocusPhase::Break), (5 * 60, FocusPhase::Focus)] {
            for _ in 0..secs {
                sim.second(&mut p, &w, |p| assert!(is_asleep(p), "{:?}", p.state));
            }
            assert_eq!(phase(&sim), want);
            assert_eq!((p.body.x, p.body.y), spot, "stays where it sleeps");
        }
        assert_eq!(sim.said, vec![], "no focus! and no break time!");
        assert_eq!(p.heart_timer, 0.0, "no break hearts");
    }
}

#[test]
fn back_during_a_focus_segment_the_pet_wakes_and_goes_to_its_corner() {
    let w = world(vec![]);
    let (mut sim, mut p) = away_during_focus(walker(), &w);
    let spot = p.body.x;
    // 쉬기를 지나 두 번째 집중 시간에 돌아온다
    sim.run(&mut p, &w, 25 * 60 + 5 * 60 + 30);
    assert_eq!(phase(&sim), FocusPhase::Focus);
    assert!(is_asleep(&p) && p.body.x == spot);
    sim.idle = 0.0;
    sim.run(&mut p, &w, 60);
    assert!(!is_asleep(&p), "{:?}", p.state);
    assert!(p.is_focusing());
    assert!(p.body.x > spot, "went to the corner");
    assert!(!sim.said.contains(&Speech::WelcomeBack) && !sim.said.contains(&Speech::BreakTime), "{:?}", sim.said);
}

#[test]
fn back_during_a_break_the_pet_wakes_and_welcomes_you_and_focus_resumes_on_time() {
    let w = world(vec![]);
    let (mut sim, mut p) = away_during_focus(walker(), &w);
    sim.run(&mut p, &w, 25 * 60 + 30);
    assert_eq!(phase(&sim), FocusPhase::Break);
    assert!(is_asleep(&p));
    sim.idle = 0.0;
    sim.run(&mut p, &w, 10);
    assert!(!is_asleep(&p), "{:?}", p.state);
    assert!(!p.is_focusing());
    assert!(sim.said.contains(&Speech::WelcomeBack), "{:?}", sim.said);
    assert!(!sim.said.contains(&Speech::BreakTime), "the break started without it: {:?}", sim.said);
    // 쉬기가 끝나면 사용자가 있으니 다시 "focus!" 하고 구석으로
    sim.idle = 0.0;
    sim.run(&mut p, &w, 5 * 60);
    assert_eq!(phase(&sim), FocusPhase::Focus);
    assert!(sim.said.contains(&Speech::Focus), "{:?}", sim.said);
    assert!(p.is_focusing());
}

#[test]
fn a_user_who_stays_still_hears_the_segment_changes_only_while_present() {
    // 자리에 있는 동안(유휴 10분 미만)엔 그대로 "break time!" 기지개
    let w = world(vec![]);
    let mut sim = Sim::at(15, 0);
    let mut p = Pet::new(walker(), 0.0, 0.0, 5);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Idle { remaining: 100.0, look: false });
    sim.rhythm.start_focus(FocusLength::Short, sim.secs, &mut p);
    for _ in 0..25 * 60 + 2 {
        sim.idle = 0.0;
        sim.second(&mut p, &w, |_| {});
    }
    assert!(sim.said.contains(&Speech::BreakTime), "{:?}", sim.said);
}

// ---- 돌보기 ----

#[test]
fn being_away_only_makes_it_hungry_never_smaller() {
    let mut care = Care { stage: crate::care::Stage::Baby, stamina: 3.0, fullness: 1.0, locked: false };
    care.tick(24.0 * 3600.0);
    assert_eq!(care.fullness, 0.0);
    assert_eq!(care.stage, crate::care::Stage::Baby);
    assert_eq!(care.stamina, 3.0);
}
