//! 📌 흐물이 (pet.json taskbar_play) — 작업표시줄에서 앱 아이콘 장난감 끌어내기·갖고 놀기·도로 밀어 넣기. brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::juggle_moves::{JugglePhase, Plan};
use super::tests::{place, win, world};
use super::*;
use crate::juggle::{JuggleEvent, ObjectSpot};
use crate::taskbar::{self, Toys};
use crate::test_util::{pet_cfg, DT};

fn juggler() -> PetConfig {
    PetConfig { half_w: 38.4, walk_speed: 110.0, personality: Personality::Playful, timid: true, energetic: true, rigging: true, juggler: true, ..pet_cfg() }
}

fn heumul() -> PetConfig {
    PetConfig { taskbar_play: true, ..juggler() }
}

fn ground_y(w: &World) -> f64 {
    w.anchor(Owner::Ground(0)).unwrap().1
}

fn toy_spot(id: u64, x: f64, w: &World) -> ObjectSpot {
    ObjectSpot { id, x, y: ground_y(w), owner: Owner::Ground(0), half_w: 14.4, giant: false, local: true }
}

fn walking(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
    p.facing = 1.0;
    p
}

fn phase(p: &Pet) -> Option<JugglePhase> {
    match p.state {
        State::Juggle { phase, .. } => Some(phase),
        _ => None,
    }
}

const READY: Toys = Toys { ready: true, live: None };

/// 굴릴 때가 됐다 (테스트: 바로 주사위를 굴린다)
fn due(p: &mut Pet) {
    p.toy.next_at = Some(p.clock);
}

/// 앱 흉내: Pull이면 (deliver면) 조금 뒤 장난감을 만들어 쥐여 주고, 놓으면 몇 뼘 옆 바닥에 내려앉고, Stow면 치운다
struct FakeApp {
    deliver: bool,
    live: Option<u64>,
    next: u64,
    pending: Option<f64>,
    spot: Option<ObjectSpot>,
    pulls: Vec<f64>,
    stows: usize,
    landed_at: Option<f64>,
    return_delays: Vec<f64>,
    exits: usize,
}

impl FakeApp {
    fn new(deliver: bool) -> Self {
        FakeApp { deliver, live: None, next: 100, pending: None, spot: None, pulls: Vec::new(), stows: 0, landed_at: None, return_delays: Vec::new(), exits: 0 }
    }

    fn step(&mut self, p: &mut Pet, w: &World) {
        let spots: Vec<ObjectSpot> = self.spot.into_iter().collect();
        let was_returning = p.toy.returning;
        p.notice_objects(&spots, &[], w);
        p.notice_taskbar(&spots, Toys { ready: self.live.is_none() && self.pending.is_none(), live: self.live }, w);
        if p.toy.returning && !was_returning {
            if let Some(at) = self.landed_at {
                self.return_delays.push(p.clock - at);
            }
        }
        p.update(DT, w);
        if matches!(p.state, State::Exit { .. }) && p.carried_object().is_some() {
            self.exits += 1;
        }
        match p.take_juggle() {
            Some(JuggleEvent::Pull) => {
                self.pulls.push(p.clock);
                if self.deliver {
                    self.pending = Some(p.clock + 0.3);
                }
            }
            Some(JuggleEvent::Release { id, .. }) => {
                assert_eq!(Some(id), self.live);
                let x = (p.body.x + 250.0 * p.facing).clamp(100.0, 1820.0);
                self.spot = Some(toy_spot(id, x, w));
                self.landed_at = Some(p.clock);
            }
            Some(JuggleEvent::Stow { id }) => {
                assert_eq!(Some(id), self.live, "도로 넣는 건 화면의 그 장난감");
                assert!(p.standing.is_some_and(|s| matches!(s.owner, Owner::Ground(_))), "바닥에서 밀어 넣는다");
                self.live = None;
                self.spot = None;
                self.stows += 1;
            }
            None => {}
        }
        if self.pending.is_some_and(|at| p.clock >= at) {
            self.pending = None;
            if p.wants_toy() {
                let id = self.next;
                self.next += 1;
                assert!(p.hold_toy(id));
                self.live = Some(id);
            }
        }
        if self.live.is_some() && p.carried_object() == self.live {
            self.spot = None;
        }
        // 들고 있던 것을 놓쳤다: 그 자리 바닥에 떨어진다
        if let (Some(id), None, None) = (self.live, self.spot, p.carried_object()) {
            if !matches!(phase(p), Some(JugglePhase::Yank)) {
                let x = p.body.x.clamp(100.0, 1820.0);
                self.spot = Some(toy_spot(id, x, w));
                self.landed_at = Some(p.clock);
            }
        }
    }
}

#[test]
fn taskbar_animations_are_optional_side_views() {
    for (a, name) in [(Anim::Yank, "yank"), (Anim::Stuff, "stuff")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::OPTIONAL_ANIMS.contains(&name) && crate::manifest::TASKBAR_ANIMS.contains(&name));
        assert!(!a.is_back() && !a.is_front(), "{name}");
    }
}

#[test]
fn other_species_plain_jugglers_and_unready_heumul_never_pull_and_their_dice_are_untouched() {
    let w = world(vec![]);
    for (with, without, toys) in [(pet_cfg(), pet_cfg(), READY), (juggler(), juggler(), READY), (heumul(), juggler(), Toys { ready: false, live: None })] {
        let mut a = walking(with, 800.0, 9, &w);
        let mut b = walking(without, 800.0, 9, &w);
        for _ in 0..(60 * 600) {
            a.notice_objects(&[], &[], &w);
            a.notice_taskbar(&[], toys, &w);
            a.update(DT, &w);
            b.notice_objects(&[], &[], &w);
            b.update(DT, &w);
            assert_eq!(a.state, b.state);
            assert_eq!(a.body.x, b.body.x);
            assert!(a.take_juggle().is_none() && !a.wants_toy());
        }
    }
}

#[test]
fn it_pulls_now_and_then_but_only_on_the_bottom_ground() {
    let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
    let mut pulled = 0;
    for seed in 0..400 {
        let mut p = walking(heumul(), 800.0, seed, &w);
        due(&mut p);
        p.notice_taskbar(&[], READY, &w);
        if matches!(phase(&p), Some(JugglePhase::Yank)) {
            pulled += 1;
            assert_eq!(p.take_juggle(), Some(JuggleEvent::Pull));
            assert!(p.wants_toy());
            assert_eq!(p.render().anim, Anim::Yank);
        } else {
            assert!(p.take_juggle().is_none());
        }
        // 굴린 뒤엔 한동안 다시 굴리지 않는다
        let next = p.toy.next_at.unwrap();
        assert!(next - p.clock >= taskbar::PULL_EVERY_SECS.0 && next - p.clock <= taskbar::PULL_EVERY_SECS.1);
        // 창 윗면에서는 끌어내지 않는다 (작업표시줄은 화면 아래에 있다)
        let mut q = Pet::new(heumul(), 800.0, 0.0, seed);
        place(&mut q, &w, Owner::Window(1), 800.0, State::Move { kind: Move::Walk, remaining: 100.0 });
        due(&mut q);
        q.notice_taskbar(&[], READY, &w);
        assert!(phase(&q).is_none() && q.take_juggle().is_none());
        // 자거나 잡혀 있으면 굴리지도 않는다
        let mut r = walking(heumul(), 800.0, seed, &w);
        r.state = State::Sleep { remaining: 10.0 };
        due(&mut r);
        r.notice_taskbar(&[], READY, &w);
        assert!(phase(&r).is_none() && r.toy.next_at == Some(r.clock));
    }
    let share = pulled as f64 / 400.0;
    assert!((share - taskbar::PULL_CHANCE).abs() < 0.08, "{share}");
}

#[test]
fn over_two_hours_it_pulls_a_modest_number_of_times() {
    let w = world(vec![]);
    let mut p = walking(heumul(), 800.0, 3, &w);
    let mut app = FakeApp::new(false);
    for _ in 0..(60 * 60 * 120) {
        app.step(&mut p, &w);
    }
    let gaps: Vec<f64> = app.pulls.windows(2).map(|w| w[1] - w[0]).collect();
    assert!((6..=30).contains(&app.pulls.len()), "{:?}", app.pulls);
    assert!(gaps.iter().all(|&g| g >= taskbar::PULL_EVERY_SECS.0), "{gaps:?}");
    assert!(app.pulls[0] >= taskbar::PULL_EVERY_SECS.0, "켜자마자 하지 않는다");
}

#[test]
fn one_toy_at_a_time() {
    let w = world(vec![]);
    for seed in 0..100 {
        for toys in [Toys { ready: false, live: None }, Toys { ready: true, live: Some(5) }] {
            let mut p = walking(heumul(), 800.0, seed, &w);
            due(&mut p);
            p.notice_taskbar(&[], toys, &w);
            assert!(phase(&p).is_none() && p.take_juggle().is_none(), "{toys:?}");
        }
        // 내 장난감이 화면에 있는 동안엔 앱이 된다고 해도 새로 끌어내지 않는다
        let mut p = walking(heumul(), 800.0, seed, &w);
        p.toy.toy = Some(5);
        due(&mut p);
        p.notice_taskbar(&[], Toys { ready: true, live: Some(5) }, &w);
        assert!(!matches!(phase(&p), Some(JugglePhase::Yank)));
    }
}

#[test]
fn a_pulled_toy_pops_up_into_its_hands_and_is_never_carried_off_to_friends() {
    let w = world(vec![]);
    let mut played = 0;
    for seed in 0..60 {
        let mut p = walking(heumul(), 800.0, seed, &w);
        p.toy.rng = fastrand::Rng::with_seed(seed);
        // 친구가 양쪽 끝에 다 연결돼 있다
        p.set_exit_edges(true, true);
        p.set_prank_edges(true, true);
        let mut app = FakeApp::new(true);
        let mut tries = 0;
        while app.pulls.is_empty() && tries < 50 {
            due(&mut p);
            app.step(&mut p, &w);
            tries += 1;
        }
        assert_eq!(app.pulls.len(), 1, "{seed}");
        let pulled_at = app.pulls[0];
        let mut popped = None;
        for _ in 0..(60 * 30) {
            app.step(&mut p, &w);
            let ph = phase(&p);
            assert!(!matches!(ph, Some(JugglePhase::CarryOff { .. })), "친구에게 들고 가지 않는다");
            if popped.is_none() && app.live.is_some() && matches!(ph, Some(JugglePhase::Lift)) {
                popped = Some(p.clock - pulled_at);
                assert_eq!(p.carried_object(), app.live);
                let (hx, hy) = p.carry_point().unwrap();
                assert!((hx - p.body.x).abs() < 0.5 * p.cfg.size && hy < p.body.y + 1.0, "손에 든다");
            }
        }
        assert_eq!(app.exits, 0, "{seed}: 화면 밖으로 들고 나가지 않는다");
        let popped = popped.expect("번쩍 들어 올린다");
        assert!(popped >= taskbar::YANK_MIN_SECS - DT && popped < taskbar::YANK_MIN_SECS + 0.1, "{popped}");
        played += 1;
    }
    assert_eq!(played, 60);
}

#[test]
fn without_a_toy_it_gives_up_and_a_late_toy_is_refused() {
    let w = world(vec![]);
    let mut p = walking(heumul(), 800.0, 1, &w);
    p.toy.rng = fastrand::Rng::with_seed(0);
    let mut seed = 0;
    while !matches!(phase(&p), Some(JugglePhase::Yank)) {
        p.toy.rng = fastrand::Rng::with_seed(seed);
        seed += 1;
        due(&mut p);
        p.notice_taskbar(&[], READY, &w);
    }
    let mut t = 0.0;
    while matches!(phase(&p), Some(JugglePhase::Yank)) {
        p.notice_taskbar(&[], Toys { ready: false, live: None }, &w);
        p.update(DT, &w);
        t += DT;
        assert!(t < taskbar::YANK_GIVE_UP_SECS + 0.1);
    }
    assert!(t >= taskbar::YANK_GIVE_UP_SECS - DT);
    assert!(!p.wants_toy() && !p.hold_toy(42));
    assert!(p.carried_object().is_none() && p.toy.toy.is_none());
}

#[test]
fn it_goes_back_for_the_toy_and_pushes_it_back_in() {
    let w = world(vec![]);
    let (mut stowed, mut quick, mut slow, mut saw_stuff) = (0, 0, 0, 0);
    for seed in 0..40 {
        let mut p = walking(heumul(), 800.0, seed, &w);
        p.toy.rng = fastrand::Rng::with_seed(seed * 7 + 1);
        let mut app = FakeApp::new(true);
        let mut stuffing = false;
        for _ in 0..(60 * 60 * 6) {
            if app.pulls.is_empty() {
                due(&mut p);
            }
            app.step(&mut p, &w);
            if matches!(phase(&p), Some(JugglePhase::Stuff { .. })) {
                stuffing = true;
                assert_eq!(p.render().anim, Anim::Stuff);
            }
            if app.stows > 0 {
                break;
            }
        }
        if app.stows == 0 {
            continue;
        }
        stowed += 1;
        saw_stuff += usize::from(stuffing);
        assert!(p.toy.toy.is_none() && p.carried_object().is_none(), "도로 넣으면 잊는다");
        for d in &app.return_delays {
            assert!(*d >= taskbar::QUICK_RETURN_SECS.0 - 0.1 && *d <= taskbar::RETURN_SECS.1 + taskbar::RETRY_SECS * 3.0, "{seed}: {d}");
            if *d <= taskbar::QUICK_RETURN_SECS.1 + 0.1 {
                quick += 1;
            } else if *d >= taskbar::RETURN_SECS.0 - 0.1 {
                slow += 1;
            }
        }
        // 다 넣었으면 언젠가 또 끌어낼 수 있다
        let mut again = false;
        for _ in 0..(60 * 60 * 60) {
            app.step(&mut p, &w);
            if app.pulls.len() > 1 {
                again = true;
                break;
            }
        }
        assert!(again, "{seed}");
    }
    assert!(stowed >= 36, "{stowed}");
    assert_eq!(saw_stuff, stowed);
    assert!(slow > 0 && quick > 0, "{slow} {quick}");
}

#[test]
fn a_toy_it_cannot_reach_waits_and_a_gone_toy_is_forgotten() {
    let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
    let mut p = walking(heumul(), 200.0, 1, &w);
    p.toy.toy = Some(5);
    p.toy.return_at = Some(0.0);
    let on_window = ObjectSpot { owner: Owner::Window(1), y: 400.0, ..toy_spot(5, 800.0, &w) };
    for _ in 0..(60 * 10) {
        p.notice_taskbar(&[on_window], Toys { ready: false, live: Some(5) }, &w);
        assert!(!matches!(phase(&p), Some(JugglePhase::Approach { .. })), "창 위의 장난감엔 갈 수 없다");
        p.update(DT, &w);
    }
    assert_eq!(p.toy.toy, Some(5));
    // 앱이 치웠다 (사용자가 치움·저절로 사라짐): 잊는다
    p.notice_taskbar(&[], Toys { ready: true, live: None }, &w);
    assert_eq!(p.toy.toy, None);
}

#[test]
fn a_juggler_never_takes_a_local_toy_to_a_friend() {
    let w = world(vec![]);
    for seed in 0..300 {
        let mut p = walking(heumul(), 800.0, seed, &w);
        p.set_prank_edges(true, true);
        p.juggle.rng = fastrand::Rng::with_seed(seed);
        p.juggle.local = true;
        assert!(!matches!(p.pick_plan(false, &w), Plan::Prank(_)));
    }
}

#[test]
fn the_toy_never_dips_below_the_bottom_ground_onto_the_taskbar() {
    use crate::object::{Object, ObjectKind};
    let w = world(vec![]);
    let floor = ground_y(&w);
    // 손이 바닥 바로 앞에 있는 그림 (끌어내기·밀어 넣기 칸)
    let low = crate::juggle::GripAnim { pts: [(0.3, 0.95); 4], n: 4, fps: 6.0, looping: true };
    let cfg = PetConfig { grip: crate::juggle::GripTable { yank: low, lift: low, stuff: low, ..Default::default() }, ..heumul() };
    let mut p = walking(cfg, 800.0, 1, &w);
    p.state = State::Juggle { phase: JugglePhase::Yank, t: 0.0 };
    assert!(p.hold_toy(9));
    let mut toy = Object::new(ObjectKind::AppIcon, 0.0, 0.0, 1.0, 1);
    for _ in 0..(60 * 3) {
        p.update(DT, &w);
        let Some((x, y)) = p.carry_point() else { break };
        toy.carry_at(x, y);
        assert!(toy.body.y <= floor + 1e-9, "{} > {floor}", toy.body.y);
    }
}
