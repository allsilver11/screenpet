//! 창 안에서 놀기 (spec 2B §9) — brain 동작 테스트. brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, run, sim_cfg, win, world};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::inside::{Mode, Stay};

const DT: f64 = 1.0 / 60.0;

/// 창 1: 500..1100 × 400..900 → 안쪽 바닥 y 896, x 504..1096
fn w1() -> WindowInfo {
    win(1, 500.0, 400.0, 1100.0, 900.0)
}

fn climber() -> PetConfig {
    PetConfig { can_climb: true, ..cfg() }
}

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 창 id 안쪽 바닥 x에 세운다 (머무는 시간 leave_in)
fn place_inside(p: &mut Pet, w: &World, id: u64, x: f64, state: State, leave_in: f64) {
    place(p, w, Owner::Inside(id), x, state);
    p.inside = Some(Stay { window: id, mode: Mode::In, leave_in, climb_out: false });
}

/// 몸을 잡아 (x, y)로 옮긴 뒤 잠깐 멈췄다 놓는다 (던지는 속도 0)
fn drop_at(p: &mut Pet, w: &World, x: f64, y: f64) {
    let (gx, gy) = (p.body.x, p.body.y - 40.0);
    p.pointer_down(gx, gy);
    p.pointer_move(x, y - 40.0);
    assert_eq!(p.state, State::Dragged);
    run(p, w, 30);
    p.pointer_up(x, y - 40.0);
}

/// 내려설 때까지 (최대 ticks)
fn run_until_standing(p: &mut Pet, w: &World, ticks: usize) {
    for _ in 0..ticks {
        p.update(DT, w);
        if p.standing.is_some() && !p.off_surface() {
            return;
        }
    }
}

#[test]
fn dropped_inside_a_window_lands_on_its_inner_floor() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 800.0, 700.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Inside(1)));
    assert_eq!((p.body.x, p.body.y), (800.0, 896.0));
    assert_eq!(p.inside_window(), Some(1));
    // 그대로 머문다
    run(&mut p, &w, 600);
    assert_eq!(p.inside_window(), Some(1));
    assert!(p.standing.is_none() || p.standing.unwrap().owner == Owner::Inside(1));
}

#[test]
fn dropped_on_top_or_across_the_border_stays_outside() {
    let w = world(vec![w1()]);
    // 윗면 위에서 놓음 → 윗면
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 800.0, 300.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!(p.inside_window(), None);
    // 옆 테두리에 걸침 → 밖 (바닥까지 떨어진다)
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 520.0, 700.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert_eq!(p.inside_window(), None);
}

#[test]
fn thrown_inside_bounces_off_inner_walls_and_ceiling() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.standing = None;
    p.body = Body { x: 800.0, y: 700.0, vx: 2500.0, vy: -1500.0 };
    p.state = State::Air { anim: Anim::Fall };
    for _ in 0..300 {
        p.update(DT, &w);
        assert!(p.body.x + 38.0 <= 1096.0 + 1e-9 && p.body.x - 38.0 >= 504.0 - 1e-9, "x {}", p.body.x);
        assert!(p.body.y - 128.0 >= 400.0 - 1e-9, "y {}", p.body.y);
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Inside(1)));
}

#[test]
fn walking_turns_at_the_inner_walls() {
    let w = world(vec![w1()]);
    for kind in [Move::Walk, Move::Run, Move::Roll] {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place_inside(&mut p, &w, 1, 1096.0 - 38.0 - 0.5, State::Move { kind, remaining: 100.0 }, 1000.0);
        p.facing = 1.0;
        p.update(DT, &w);
        assert!(matches!(p.state, State::Turn { .. }), "{kind:?} {:?}", p.state);
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place_inside(&mut p, &w, 1, 504.0 + 38.0 + 0.5, State::Move { kind, remaining: 100.0 }, 1000.0);
        p.facing = -1.0;
        p.update(DT, &w);
        assert!(matches!(p.state, State::Turn { .. }), "{kind:?} {:?}", p.state);
    }
}

#[test]
fn rides_inside_when_the_window_moves() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    let moved = world(vec![win(1, 600.0, 350.0, 1200.0, 850.0)]);
    p.update(DT, &moved);
    assert_eq!((p.body.x, p.body.y), (900.0, 846.0));
    assert_eq!(p.inside_window(), Some(1));
}

#[test]
fn closed_window_drops_the_pet_outside_to_the_ground() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    let gone = world(vec![]);
    run_until_standing(&mut p, &gone, 300);
    assert_eq!(p.inside_window(), None);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert_eq!(p.body.y, 1040.0);
    // 공중에 있을 때 닫혀도 밖으로 떨어진다
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.standing = None;
    p.body.y = 700.0;
    p.state = State::Air { anim: Anim::Fall };
    run_until_standing(&mut p, &gone, 300);
    assert_eq!((p.inside_window(), p.standing.map(|s| s.owner)), (None, Some(Owner::Ground(0))));
}

#[test]
fn covered_spot_pops_the_pet_out_onto_the_top() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    // 아래쪽만 가리는 앞 창 — 윗면은 그대로
    let covered = world(vec![win(2, 750.0, 850.0, 850.0, 1000.0), w1()]);
    p.update(DT, &covered);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    let mut above_top = false;
    for _ in 0..600 {
        p.update(DT, &covered);
        above_top |= p.body.y < 400.0;
        if p.standing.is_some() && !p.off_surface() {
            break;
        }
    }
    assert!(above_top);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!((p.body.x, p.body.y), (800.0, 400.0));
    assert_eq!(p.inside_window(), None);
}

#[test]
fn covered_spot_with_covered_top_falls_outside() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    let covered = world(vec![win(2, 750.0, 300.0, 850.0, 1000.0), w1()]);
    run_until_standing(&mut p, &covered, 600);
    assert_eq!(p.inside_window(), None);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
}

/// 창 윗면에서 쉬기를 정할 때 창 안으로 뛰어드는 비율
fn hop_in_share(w: &World, owner: Owner, x: f64, cfg: PetConfig) -> f64 {
    let mut p = Pet::new(cfg, 0.0, 0.0, 3);
    let n = 20_000;
    let mut hops = 0;
    for _ in 0..n {
        place(&mut p, w, owner, x, idle());
        p.inside = None;
        let s = p.rest_state(w);
        if p.inside.is_some() {
            assert!(matches!(s, State::Air { anim: Anim::Hop }));
            assert_eq!(p.inside.unwrap().mode, Mode::HopIn);
            hops += 1;
        }
    }
    hops as f64 / n as f64
}

#[test]
fn resting_on_a_window_top_hops_in_about_5_percent() {
    let w = world(vec![w1()]);
    let share = hop_in_share(&w, Owner::Window(1), 800.0, cfg());
    assert!((0.04..=0.06).contains(&share), "{share}");
    assert_eq!(hop_in_share(&w, Owner::Ground(0), 200.0, cfg()), 0.0);
}

#[test]
fn hop_in_arcs_through_the_top_and_lands_on_the_inner_floor() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 3);
    for _ in 0..1000 {
        place(&mut p, &w, Owner::Window(1), 800.0, idle());
        p.inside = None;
        p.state = p.rest_state(&w);
        if p.inside.is_some() {
            break;
        }
    }
    assert!(p.inside.is_some());
    let mut went_up = false;
    for _ in 0..300 {
        p.update(DT, &w);
        went_up |= p.body.y < 400.0;
        if p.standing.is_some() && !p.off_surface() {
            break;
        }
    }
    assert!(went_up);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Inside(1)));
    assert_eq!(p.body.y, 896.0);
    assert_eq!(p.inside.unwrap().mode, Mode::In);
    let left = p.inside.unwrap().leave_in;
    assert!((50.0..=180.0).contains(&left), "{left}");
}

#[test]
fn too_small_windows_cannot_be_entered() {
    // 높이 140 < 1.2 × 128
    let short = win(1, 500.0, 400.0, 1100.0, 540.0);
    let w = world(vec![short]);
    assert_eq!(hop_in_share(&w, Owner::Window(1), 800.0, cfg()), 0.0);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 800.0, 535.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.inside_window(), None);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    // 몸 폭보다 좁은 창
    let narrow = win(1, 500.0, 400.0, 580.0, 900.0);
    let w = world(vec![narrow]);
    let mut p = Pet::new(PetConfig { half_w: 38.0, ..cfg() }, 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 540.0, 800.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.inside_window(), None);
}

#[test]
fn leaves_after_its_stay_onto_the_top() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 0.05);
    let mut above_top = false;
    for _ in 0..600 {
        p.update(DT, &w);
        above_top |= p.body.y < 400.0;
        if p.inside.is_none() && p.standing.is_some() && !p.off_surface() {
            break;
        }
    }
    assert!(above_top);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!(p.body.y, 400.0);
}

#[test]
fn stay_runs_out_only_after_one_to_three_minutes() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(sim_cfg(), 0.0, 0.0, 9);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 800.0, 700.0);
    let mut inside_ticks = 0usize;
    for _ in 0..(60 * 200) {
        p.update(DT, &w);
        if p.inside_window().is_some() {
            inside_ticks += 1;
        } else if inside_ticks > 0 {
            break;
        }
    }
    let secs = inside_ticks as f64 * DT;
    assert!((60.0..=185.0).contains(&secs), "{secs}");
}

#[test]
fn sleeping_pet_waits_to_leave_until_it_wakes() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, State::Sleep { remaining: 2.0 }, 0.0);
    run(&mut p, &w, 60);
    assert!(matches!(p.state, State::Sleep { .. }));
    assert_eq!(p.inside_window(), Some(1));
}

#[test]
fn climbers_sometimes_climb_an_inner_wall_out() {
    let w = world(vec![w1()]);
    let (mut climbs, n) = (0, 400);
    for seed in 0..n {
        let mut p = Pet::new(climber(), 0.0, 0.0, seed);
        place_inside(&mut p, &w, 1, 800.0, idle(), 0.0);
        p.update(DT, &w);
        climbs += p.inside.is_some_and(|s| s.climb_out) as usize;
    }
    let share = climbs as f64 / n as f64;
    assert!((0.4..=0.6).contains(&share), "{share}");
    // 벽을 못 타는 동물은 늘 뛰어나간다
    let mut p = Pet::new(cfg(), 0.0, 0.0, 0);
    place_inside(&mut p, &w, 1, 800.0, idle(), 0.0);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { .. }));
}

#[test]
fn climbing_out_goes_up_the_inner_wall_and_onto_the_top() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 1000.0, State::Move { kind: Move::Walk, remaining: 100.0 }, -1.0);
    p.inside.as_mut().unwrap().climb_out = true;
    p.facing = 1.0;
    let mut climbed = false;
    for _ in 0..(60 * 20) {
        p.update(DT, &w);
        if let State::InnerClimb { .. } = p.state {
            climbed = true;
            assert_eq!(p.render().anim, Anim::Climb);
            assert_eq!(p.body.x, 1096.0 - 38.0);
        }
        if climbed && p.standing.is_some() {
            break;
        }
    }
    assert!(climbed);
    assert_eq!(p.inside_window(), None);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!((p.body.x, p.body.y), (1096.0 - 38.0, 400.0));
    // 창이 움직이면 따라간다
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 1000.0, idle(), 1000.0);
    p.standing = None;
    p.state = State::InnerClimb { window: 1, rel_x: 558.0, rel_y: 300.0 };
    let moved = world(vec![win(1, 600.0, 400.0, 1200.0, 900.0)]);
    p.update(DT, &moved);
    assert_eq!(p.body.x, 1158.0);
}

#[test]
fn no_screen_climbing_wall_sticking_or_exits_while_inside() {
    let w = world(vec![w1()]);
    for seed in 0..6 {
        let mut p = Pet::new(PetConfig { can_climb: true, ..sim_cfg() }, 0.0, 0.0, seed);
        p.set_exit_edges(true, true);
        place_inside(&mut p, &w, 1, 800.0, idle(), 1.0e9);
        for _ in 0..(60 * 180) {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Climb { .. } | State::Slide | State::WallClimb { .. } | State::Exit { .. }), "{:?}", p.state);
            assert_eq!(p.inside_window(), Some(1));
        }
    }
}

#[test]
fn dragging_out_leaves_the_window() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    drop_at(&mut p, &w, 200.0, 700.0);
    assert_eq!(p.inside_window(), None);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    // 다른 창 안으로 옮기기
    let w = world(vec![w1(), win(2, 1200.0, 300.0, 1800.0, 800.0)]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    drop_at(&mut p, &w, 1500.0, 600.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Inside(2)));
}

#[test]
fn go_home_from_inside_hops_out_first() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.go_home(Owner::Ground(0), 1800.0);
    let mut left_window = false;
    for _ in 0..(60 * 30) {
        p.update(DT, &w);
        left_window |= p.inside.is_none() && !p.is_housed();
        if p.is_housed() {
            break;
        }
    }
    assert!(left_window);
    assert!(p.is_housed());
    assert_eq!(p.inside_window(), None);
}

#[test]
fn go_home_while_hopping_in_lands_then_hops_out() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.standing = None;
    p.body.y = 600.0;
    p.state = State::Air { anim: Anim::Hop };
    p.inside.as_mut().unwrap().mode = Mode::HopIn;
    p.go_home(Owner::Ground(0), 1800.0);
    for _ in 0..(60 * 30) {
        p.update(DT, &w);
        if p.is_housed() {
            break;
        }
    }
    assert!(p.is_housed());
}

#[test]
fn entering_the_house_or_leaving_the_screen_forgets_the_window() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.enter_house();
    assert_eq!(p.inside_window(), None);
    p.leave_house(&w, 1800.0, Owner::Ground(0));
    assert_eq!(p.inside_window(), None);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.enter(&w, crate::world::Side::Left);
    assert_eq!(p.inside_window(), None);
}

/// 창이 많은 화면 10분 — 패닉 없이, 늘 화면 안에서, 창 안에서도 논다. 창이 가끔 움직이고 닫힌다
#[test]
fn window_rich_ten_minutes_smoke() {
    let screen = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)];
    let base = vec![
        win(1, 100.0, 500.0, 700.0, 1000.0),
        win(2, 600.0, 300.0, 1300.0, 900.0),
        win(3, 1200.0, 600.0, 1900.0, 1040.0),
        win(4, 400.0, 700.0, 520.0, 800.0),
        win(5, 0.0, 0.0, 1920.0, 1040.0),
    ];
    let worlds: Vec<World> = (0..6)
        .map(|k| {
            let mut ws: Vec<WindowInfo> = base.clone();
            match k {
                1 => ws[1].rect = Rect::new(650.0, 280.0, 1350.0, 880.0),
                2 => {
                    ws.remove(0);
                }
                3 => ws.swap(0, 1),
                4 => ws[2].rect = Rect::new(1150.0, 620.0, 1850.0, 1040.0),
                _ => {}
            }
            World::new(ws, screen.clone(), 128.0)
        })
        .collect();
    let (mut inside_ticks, mut inner_climb) = (0usize, 0usize);
    for seed in 0..8 {
        let cfg = PetConfig { can_climb: seed % 2 == 0, special: Some(Special::Wag), ..sim_cfg() };
        let mut p = Pet::new(cfg, 960.0, 128.0, seed);
        let mut rng = fastrand::Rng::with_seed(seed + 100);
        for t in 0..(60 * 600) {
            let w = &worlds[(t / (60 * 45)) % worlds.len()];
            if t % (60 * 100) == 60 * 50 {
                // 가끔 아무 데나 집어서 놓는다
                let (x, y) = (rng.f64() * 1700.0 + 100.0, rng.f64() * 800.0 + 200.0);
                if !p.off_surface() {
                    drop_at(&mut p, w, x, y);
                }
            }
            p.update(DT, w);
            inside_ticks += p.inside_window().is_some() as usize;
            inner_climb += matches!(p.state, State::InnerClimb { .. }) as usize;
            let b = w.bounds;
            assert!(p.body.x >= b.left && p.body.x <= b.right, "seed {seed} t {t}: x {} {:?}", p.body.x, p.state);
            assert!(p.body.y >= b.top + 128.0 - 1e-9 && p.body.y <= b.bottom + 1e-9, "seed {seed} t {t}: y {} {:?}", p.body.y, p.state);
            if let Some(id) = p.inside_window() {
                if p.standing.is_some() && !p.off_surface() {
                    assert_eq!(p.standing.unwrap().owner, Owner::Inside(id), "seed {seed} t {t}");
                }
            } else if let Some(st) = p.standing {
                assert!(!matches!(st.owner, Owner::Inside(_)), "seed {seed} t {t}");
            }
        }
    }
    assert!(inside_ticks > 60 * 60, "inside {inside_ticks}");
    assert!(inner_climb > 0, "inner climb {inner_climb}");
}

#[test]
fn homesick_guest_hops_out_to_head_home() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    p.set_guest(crate::world::Side::Left, 0.0);
    place_inside(&mut p, &w, 1, 800.0, idle(), 1000.0);
    p.update(DT, &w);
    assert!(matches!(p.state, State::Air { anim: Anim::Hop }));
    assert_eq!(p.inside.unwrap().mode, Mode::HopOut);
}
