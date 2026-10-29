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
    p.inside = Some(Stay { window: id, mode: Mode::In, leave_in, climb_out: false, hops: 0 });
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

/// 크기 size인 동물 (반 폭·걷기 속도도 크기에 비례)
fn sized(size: f64, can_climb: bool) -> PetConfig {
    let k = size / 128.0;
    PetConfig { size, half_w: 0.3 * size, walk_speed: 90.0 * k, can_climb, ..sim_cfg() }
}

/// 창이 많은 화면 10분 (아기 0.45배·보통·큰 1.5배) — 패닉 없이, 늘 화면 안에서, 창 안에서도 논다.
/// 창이 가끔 움직이고 닫히고 순서가 바뀐다
#[test]
fn window_rich_ten_minutes_smoke() {
    let screen = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)];
    let base = vec![
        win(1, 100.0, 500.0, 700.0, 1000.0),
        win(2, 600.0, 300.0, 1300.0, 900.0),
        win(3, 1200.0, 600.0, 1900.0, 1040.0),
        win(4, 400.0, 700.0, 520.0, 800.0),
        win(6, 1400.0, 90.0, 1800.0, 1040.0),
        win(5, 0.0, 0.0, 1920.0, 1040.0),
    ];
    // 안쪽 벽 타기는 창이 자주 바뀌면 드물어서 모든 크기를 합쳐 센다
    let mut inner_climb = 0usize;
    for size in [0.45 * 128.0, 128.0, 1.5 * 128.0] {
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
                World::new(ws, screen.clone(), size)
            })
            .collect();
        let mut inside_ticks = 0usize;
        for seed in 0..8 {
            let cfg = PetConfig { special: Some(Special::Wag), ..sized(size, seed % 2 == 0) };
            let mut p = Pet::new(cfg, 960.0, size, seed);
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
                let at = format!("size {size} seed {seed} t {t}: ({}, {}) {:?}", p.body.x, p.body.y, p.state);
                assert!(p.body.x >= b.left && p.body.x <= b.right, "{at}");
                assert!(p.body.y >= b.top + size - 1e-9 && p.body.y <= b.bottom + 1e-9, "{at}");
                if let Some(id) = p.inside_window() {
                    if p.standing.is_some() && !p.off_surface() {
                        assert_eq!(p.standing.unwrap().owner, Owner::Inside(id), "{at}");
                    }
                } else if let Some(st) = p.standing {
                    assert!(!matches!(st.owner, Owner::Inside(_)), "{at}");
                }
            }
        }
        assert!(inside_ticks > 60 * 60, "size {size}: inside {inside_ticks}");
    }
    assert!(inner_climb > 0, "inner climb {inner_climb}");
}

/// 아기(0.45배)가 꼭대기가 화면 위 가까이 있는 키 큰 창 안에서 나가기 시작 — secs초 안에 윗면에 서는지
fn baby_leaves_tall_window(can_climb: bool, seed: u64, dt: f64, secs: f64) -> bool {
    let size = 0.45 * 128.0;
    let w = World::new(vec![win(1, 500.0, size + 2.0, 1100.0, 1040.0)], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], size);
    assert!(w.surfaces.iter().any(|s| s.owner == Owner::Window(1)));
    let mut p = Pet::new(sized(size, can_climb), 0.0, 0.0, seed);
    place_inside(&mut p, &w, 1, 800.0, idle(), 0.01);
    for _ in 0..(secs / dt) as usize {
        p.update(dt, &w);
        if p.inside.is_none() && p.standing.is_some() && !p.off_surface() {
            return p.standing.unwrap().owner == Owner::Window(1);
        }
    }
    false
}

#[test]
fn baby_in_a_tall_window_hops_clear_of_the_top() {
    // 연속 공식 그대로면 반암시적 적분이 v·dt/2를 잃어 윗면에 못 닿고 영원히 다시 뛴다
    assert!(baby_leaves_tall_window(false, 1, DT, 3.0));
    assert!(baby_leaves_tall_window(false, 2, 1.0 / 30.0, 3.0));
    // 벽 타는 아기도 (뛰거나 기어올라) 나간다
    for seed in 0..4 {
        assert!(baby_leaves_tall_window(true, seed, DT, 60.0), "seed {seed}");
    }
}

#[test]
fn after_two_failed_hops_the_leap_ignores_the_ceiling() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 800.0, idle(), 0.01);
    p.inside.as_mut().unwrap().hops = 2;
    // 아주 큰 dt 한 번이어도 반드시 윗면에 선다
    p.update(DT, &w);
    assert_eq!(p.inside.unwrap().mode, Mode::LeapOut);
    let mut steps = 0;
    while !(p.inside.is_none() && p.standing.is_some() && !p.off_surface()) && steps < 100 {
        p.update(0.1, &w);
        steps += 1;
    }
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)));
    assert_eq!(p.body.y, 400.0);
}

#[test]
fn maximized_windows_cannot_be_entered() {
    // 작업 영역의 90% 이상을 덮는 창 (뒤에 깔린 최대화 창) — 빈 바탕에 놓아도 안으로 들어가지 않는다
    let w = world(vec![win(5, 0.0, 0.0, 1920.0, 1040.0)]);
    assert!(!w.surfaces.iter().any(|s| s.owner == Owner::Inside(5)));
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    drop_at(&mut p, &w, 800.0, 700.0);
    run_until_standing(&mut p, &w, 300);
    assert_eq!((p.inside_window(), p.standing.map(|s| s.owner)), (None, Some(Owner::Ground(0))));
    let near_full = world(vec![win(5, 20.0, 20.0, 1900.0, 1030.0)]);
    assert!(!near_full.surfaces.iter().any(|s| s.owner == Owner::Inside(5)));
    let big = world(vec![win(5, 100.0, 100.0, 1800.0, 1000.0)]);
    assert!(big.surfaces.iter().any(|s| s.owner == Owner::Inside(5)));
}

#[test]
fn climb_out_give_up_counts_from_when_it_was_chosen() {
    let w = world(vec![w1()]);
    for seed in 0..50 {
        let mut p = Pet::new(climber(), 0.0, 0.0, seed);
        // 오래 자고 일어나 이미 한참 지남
        place_inside(&mut p, &w, 1, 800.0, idle(), -100.0);
        p.update(DT, &w);
        if p.inside.is_some_and(|s| s.climb_out) {
            run(&mut p, &w, 30);
            assert!(matches!(p.state, State::Move { .. }), "seed {seed}: {:?}", p.state);
            assert!(p.inside.is_some_and(|s| s.climb_out));
            return;
        }
    }
    panic!("never chose to climb out");
}

#[test]
fn interrupted_inner_climb_does_not_snap_down() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 1058.0, idle(), 1000.0);
    p.standing = None;
    // 머리가 윗면 위로 나와 있음 (발은 윗면 아래 40)
    p.state = State::InnerClimb { window: 1, rel_x: 558.0, rel_y: 40.0 };
    p.update(DT, &w);
    let y0 = p.body.y;
    p.pointer_down(p.body.x, p.body.y - 40.0);
    p.pointer_up(p.body.x, p.body.y - 40.0);
    p.update(DT, &w);
    assert!((p.body.y - y0).abs() < 5.0, "{y0} -> {}", p.body.y);
    run_until_standing(&mut p, &w, 300);
    assert!(p.standing.is_some());
}

#[test]
fn no_hop_in_from_the_very_edge_of_a_top() {
    let w = world(vec![w1()]);
    // 모서리에서 반 폭 + 4 안쪽이면 뛰어들지 않는다
    assert_eq!(hop_in_share(&w, Owner::Window(1), 510.0, cfg()), 0.0);
    assert_eq!(hop_in_share(&w, Owner::Window(1), 1090.0, cfg()), 0.0);
    assert!(hop_in_share(&w, Owner::Window(1), 545.0, cfg()) > 0.0);
}

#[test]
fn climb_out_needs_climbing_at_the_moment_it_starts() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    place_inside(&mut p, &w, 1, 1050.0, State::Move { kind: Move::Walk, remaining: 100.0 }, -1.0);
    p.inside.as_mut().unwrap().climb_out = true;
    p.facing = 1.0;
    p.set_can_climb(false);
    for _ in 0..120 {
        p.update(DT, &w);
        assert!(!matches!(p.state, State::InnerClimb { .. }));
    }
    assert!(p.inside.is_none_or(|s| s.mode == Mode::HopOut));
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


/// (바깥) 꼭대기가 화면 위에 붙은 창의 옆면을 기어오르다 머리가 화면 위로 나가기 전에 떨어진다
#[test]
fn wall_climb_never_pushes_the_head_off_screen() {
    let w = world(vec![win(6, 1400.0, 90.0, 1800.0, 1040.0)]);
    let mut p = Pet::new(climber(), 0.0, 0.0, 1);
    p.standing = None;
    p.body = Body { x: 1400.0 - 38.0, y: 700.0, vx: 0.0, vy: 0.0 };
    p.state = State::WallClimb { window: 6, side: crate::world::Side::Left, rel_y: 610.0 };
    let mut fell = false;
    for _ in 0..(60 * 15) {
        p.update(DT, &w);
        assert!(p.body.y - 128.0 >= -1e-9, "{} {:?}", p.body.y, p.state);
        fell |= matches!(p.state, State::Air { .. });
    }
    assert!(fell);
}
