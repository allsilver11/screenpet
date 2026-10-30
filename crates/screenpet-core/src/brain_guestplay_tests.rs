//! 친구 동물과 놀아 주기 (brain_guestplay.rs) — 클릭하면 애교 또는 "잡아 봐!"

use super::guest_play::{pick_guest_click, steer_away, GuestClick, CATCH_ME_SECS, CORNER_COMMIT_SECS, DODGE_BODIES};
use super::tests::{place, sim_cfg, w1, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

fn guest(cfg: PetConfig, w: &World, owner: Owner, x: f64, seed: u64) -> Pet {
    let mut p = Pet::new(cfg, 0.0, 0.0, seed);
    place(&mut p, w, owner, x, idle());
    p.set_guest(Side::Left, 1000.0);
    p
}

/// 몸 가운데를 클릭 (커서가 올라와 있다가 누르고 뗀다)
fn click(p: &mut Pet) {
    let (x, y) = (p.body.x, p.body.y - 0.5 * p.cfg.size);
    p.hover(x, y, true);
    p.pointer_down(x, y);
    p.pointer_up(x, y);
}

/// 잡아 봐!가 나올 때까지 시드를 바꿔 가며 클릭한다
fn running_guest(cfg: PetConfig, w: &World, owner: Owner, x: f64) -> Pet {
    for seed in 1..200 {
        let mut p = guest(cfg.clone(), w, owner, x, seed);
        click(&mut p);
        if p.is_catch_me() {
            return p;
        }
    }
    panic!("never ran away");
}

#[test]
fn click_outcome_is_about_half_aegyo() {
    let mut rng = fastrand::Rng::with_seed(7);
    let n = 4000;
    let aegyo = (0..n).filter(|_| matches!(pick_guest_click(&mut rng, Personality::Friendly), GuestClick::Aegyo(_))).count();
    let share = aegyo as f64 / n as f64;
    assert!((0.45..0.55).contains(&share), "{share}");
    // 애교는 앞모습 재주 — 앉기(SitFront)는 애교가 아니다
    for _ in 0..200 {
        if let GuestClick::Aegyo(t) = pick_guest_click(&mut rng, Personality::Playful) {
            assert_ne!(t, Trick::SitFront);
        }
    }
}

#[test]
fn aloof_guest_does_less_aegyo() {
    let mut rng = fastrand::Rng::with_seed(11);
    let n = 4000;
    let aegyo = (0..n).filter(|_| matches!(pick_guest_click(&mut rng, Personality::Aloof), GuestClick::Aegyo(_))).count();
    let share = aegyo as f64 / n as f64;
    assert!((0.30..0.40).contains(&share), "{share}");
}

#[test]
fn steering_runs_away_from_a_near_cursor() {
    // 커서가 왼쪽 → 오른쪽으로, 오른쪽 → 왼쪽으로
    assert_eq!(steer_away(500.0, 450.0, -1.0, 0.0, 0.0, 1000.0, 400.0), (1.0, 0.0));
    assert_eq!(steer_away(500.0, 550.0, 1.0, 0.0, 0.0, 1000.0, 400.0), (-1.0, 0.0));
    // 멀면 하던 대로
    assert_eq!(steer_away(500.0, 50.0, -1.0, 0.0, 0.0, 1000.0, 400.0), (-1.0, 0.0));
    // 돌아선 직후엔 커서 옆을 빠져나간다
    assert_eq!(steer_away(500.0, 450.0, -1.0, 0.5, 0.0, 1000.0, 400.0), (-1.0, 0.5));
}

#[test]
fn cornered_runner_turns_and_commits() {
    // 오른쪽 끝, 커서는 왼쪽 가까이 → 돌아서서 한동안 버틴다
    let (dir, commit) = steer_away(1000.0, 950.0, 1.0, 0.0, 0.0, 1000.0, 400.0);
    assert_eq!((dir, commit), (-1.0, CORNER_COMMIT_SECS));
    // 버티는 동안엔 커서가 가까워도 그대로
    assert_eq!(steer_away(990.0, 960.0, dir, commit - 0.1, 0.0, 1000.0, 400.0).0, -1.0);
}

#[test]
fn clicking_a_guest_does_aegyo_or_runs_away_about_half_each() {
    let w = world(vec![]);
    let (mut aegyo, mut run) = (0, 0);
    for seed in 1..=200 {
        let mut p = guest(sim_cfg(), &w, Owner::Ground(0), 900.0, seed);
        click(&mut p);
        match p.state {
            State::Trick { kind, .. } => {
                assert_ne!(kind, Trick::SitFront);
                aegyo += 1;
            }
            State::CatchMe { .. } => run += 1,
            s => panic!("seed {seed}: {s:?}"),
        }
        assert!(matches!(p.take_speech(), Some(Speech::Tickle | Speech::Flee)), "seed {seed}");
    }
    assert!((70..=130).contains(&aegyo), "aegyo {aegyo}, run {run}");
}

#[test]
fn clicking_my_own_pet_never_runs_away() {
    let w = world(vec![]);
    for seed in 1..=100 {
        let mut p = Pet::new(sim_cfg(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 900.0, idle());
        click(&mut p);
        assert!(!p.is_catch_me(), "seed {seed}");
    }
}

#[test]
fn runner_flees_the_following_cursor_and_stays_on_its_window() {
    let w = world(vec![w1()]);
    let mut p = running_guest(sim_cfg(), &w, Owner::Window(1), 700.0);
    let start = p.body.x;
    let mut moved_right = 0.0_f64;
    // 커서가 왼쪽에서 몸 하나 뒤로 따라온다
    for _ in 0..60 {
        let (cx, cy) = (p.body.x - 1.2 * p.cfg.size, p.body.y - 50.0);
        p.hover(cx, cy, false);
        p.update(DT, &w);
        assert!(p.is_catch_me(), "{:?}", p.state);
        assert!(p.standing.is_some_and(|s| s.owner == Owner::Window(1)), "left its window: {:?}", p.state);
        moved_right = moved_right.max(p.body.x - start);
    }
    assert!(moved_right > 50.0, "did not run away: {moved_right}");
    assert_eq!(p.facing, 1.0);
}

#[test]
fn cornered_runner_never_jumps_off_and_slips_past() {
    let w = world(vec![w1()]);
    let mut p = running_guest(sim_cfg(), &w, Owner::Window(1), 1050.0);
    let (lo, hi) = (500.0 + p.cfg.half_w, 1100.0 - p.cfg.half_w);
    let mut min_x = f64::MAX;
    // 커서는 오른쪽 끝 근처 한 자리에 머문다 (몸 둘 왼쪽)
    let cx = hi - 2.0 * p.cfg.size;
    for _ in 0..240 {
        p.hover(cx, p.body.y - 50.0, false);
        p.update(DT, &w);
        if !p.is_catch_me() {
            break;
        }
        assert!(p.standing.is_some(), "fell: {:?}", p.state);
        assert!((lo - 0.01..=hi + 0.01).contains(&p.body.x), "{}", p.body.x);
        min_x = min_x.min(p.body.x);
    }
    assert!(min_x < cx, "never slipped past the cursor: {min_x}");
}

#[test]
fn run_ends_after_its_time_with_a_heart_or_a_look_back() {
    let w = world(vec![]);
    let (mut heart, mut look) = (0, 0);
    for seed in 1..=40 {
        let mut p = guest(sim_cfg(), &w, Owner::Ground(0), 900.0, seed);
        p.hover(800.0, 900.0, false);
        p.start_catch_me();
        let mut t = 0.0;
        while p.is_catch_me() {
            p.hover(800.0, 900.0, false);
            p.update(DT, &w);
            t += DT;
            assert!(t < CATCH_ME_SECS.1 + 0.1, "seed {seed} still running");
        }
        assert!(t >= CATCH_ME_SECS.0 - 0.1, "seed {seed} ended early: {t}");
        match p.state {
            State::Trick { kind: Trick::Heart, .. } => heart += 1,
            State::Idle { look: true, .. } => look += 1,
            s => panic!("seed {seed}: {s:?}"),
        }
        // 커서 쪽(왼쪽)을 돌아본다
        assert_eq!(p.facing, -1.0, "seed {seed}");
    }
    assert!(heart > 5 && look > 5, "heart {heart}, look {look}");
}

#[test]
fn close_cursor_makes_it_dodge_with_a_hop() {
    let w = world(vec![]);
    let mut p = running_guest(sim_cfg(), &w, Owner::Ground(0), 900.0);
    // 시작 폴짝이 끝날 때까지 커서는 멀리
    for _ in 0..90 {
        p.hover(100.0, 100.0, false);
        p.update(DT, &w);
    }
    assert_ne!(p.render().anim, Anim::Hop);
    let (x, y) = (p.body.x, p.body.y - 0.5 * p.cfg.size);
    p.hover(x + 0.5 * DODGE_BODIES * p.cfg.size, y, true);
    p.update(DT, &w);
    assert_eq!(p.render().anim, Anim::Hop, "{:?}", p.state);
    assert!(p.standing.is_some(), "a dodge stays on the surface");
}

#[test]
fn homesick_ends_the_run_and_blocks_new_ones() {
    let w = world(vec![]);
    let mut p = running_guest(sim_cfg(), &w, Owner::Ground(0), 900.0);
    p.guest = Some(crate::visit::Guest { home: Side::Left, homesick_in: 0.0 });
    p.update(DT, &w);
    assert!(!p.is_catch_me(), "{:?}", p.state);
    // 집 생각 중엔 클릭해도 애교만
    for seed in 1..=60 {
        let mut p = guest(sim_cfg(), &w, Owner::Ground(0), 900.0, seed);
        p.guest = Some(crate::visit::Guest { home: Side::Left, homesick_in: 0.0 });
        click(&mut p);
        assert!(matches!(p.state, State::Trick { .. }), "seed {seed}: {:?}", p.state);
    }
}

#[test]
fn grabbing_the_runner_ends_the_run() {
    let w = world(vec![]);
    let mut p = running_guest(sim_cfg(), &w, Owner::Ground(0), 900.0);
    p.update(DT, &w);
    let (x, y) = (p.body.x, p.body.y - 40.0);
    p.pointer_down(x, y);
    p.pointer_move(x + 40.0, y - 40.0);
    assert!(matches!(p.state, State::Dragged));
    p.pointer_up(x + 40.0, y - 40.0);
    assert!(!p.is_catch_me());
}

#[test]
fn flying_guest_flies_away_within_its_limits() {
    let w = world(vec![]);
    let cfg = PetConfig { flies: true, stings: true, walk_speed: 70.0, ..sim_cfg() };
    let mut p = None;
    for seed in 1..200 {
        let mut q = Pet::new(cfg.clone(), 0.0, 0.0, seed);
        q.body = Body { x: 900.0, y: 500.0, vx: 0.0, vy: 0.0 };
        q.state = idle();
        q.set_guest(Side::Left, 1000.0);
        click(&mut q);
        if q.is_catch_me() {
            p = Some(q);
            break;
        }
    }
    let mut p = p.expect("bee never ran");
    let b = p.fly_limits(&w);
    let start = p.body.x;
    for _ in 0..60 {
        let (cx, cy) = (p.body.x - p.cfg.size, p.body.y - 0.5 * p.cfg.size);
        p.hover(cx, cy, false);
        p.update(DT, &w);
        assert!(p.is_catch_me(), "{:?}", p.state);
        assert!(p.body.x >= b.left && p.body.x <= b.right && p.body.y >= b.top && p.body.y <= b.bottom);
    }
    assert!(p.body.x - start > 50.0, "bee did not fly away: {}", p.body.x - start);
}

#[test]
fn tickling_a_guest_giggles_like_my_own_pet() {
    let w = world(vec![]);
    let mut p = guest(sim_cfg(), &w, Owner::Ground(0), 900.0, 3);
    let y = p.body.y - 50.0;
    let mut tickled = false;
    for i in 0..60 {
        let x = 900.0 + if i % 2 == 0 { -20.0 } else { 20.0 };
        p.hover(x, y, true);
        p.update(DT, &w);
        tickled |= matches!(p.state, State::Tickled { .. });
    }
    assert!(tickled, "{:?}", p.state);
}
