//! 🦸 슈퍼인간 (brain_soar.rs): ✈️ 걷기 대신 바닥 조금 위를 낮게 날기 (높이·빠르기 천천히 바뀜, 걷기는 가끔)·🔄 공중제비·
//! 디딤판 끝에서 멀리 활공해 사뿐 착지 (콰당 없음)·친구 화면을 날아서 오가기·💪 거대한 것도 한 손으로 번쩍·🌱 연령대 세기

use super::juggle_moves::JugglePhase;
use super::swing_tests::resting;
use super::tests::{place, world};
use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::juggle::{self, JuggleEvent, ObjectSpot};
use crate::soar;
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};

/// 번들 pet.json 그대로의 슈퍼인간 (청소년)
fn sup() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "super").expect("ui/pets/super").1
}

/// 바닥(x)에 쉬는 슈퍼인간 — 연령대 몸짓은 꺼 둠
fn calm_super(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(c, x, seed, w);
    p.hush_stage();
    p
}

/// 오른쪽으로 나는 슈퍼인간 (이번 토막은 반드시 낢)
fn flying_super(x: f64, seed: u64, w: &World, owner: Owner, secs: f64) -> Pet {
    let mut p = calm_super(sup(), x, seed, w);
    place(&mut p, w, owner, x, State::Move { kind: Move::Walk, remaining: secs });
    p.facing = 1.0;
    p.soar.force_fly = true;
    p
}

/// 창 아래끝과 발 사이 (창이 바닥보다 떠 있는 높이, 물리 px)
fn gap(p: &Pet) -> f64 {
    p.body.y - (p.window_pos().1 + p.cfg.size)
}

#[test]
fn flight_animations_and_lines_have_their_sheet_names() {
    for (a, name) in [(Anim::Fly, "fly"), (Anim::FlyFast, "fly_fast"), (Anim::FlyFlip, "fly_flip"), (Anim::Hover, "hover")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(!a.is_front() && !a.is_back());
        assert!(crate::manifest::SOAR_ANIMS.contains(&name));
    }
    for (s, name) in [(Speech::TakeOff, "take_off"), (Speech::Rescue, "rescue"), (Speech::AirFlip, "air_flip"), (Speech::SoftLand, "soft_land")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    assert!(sup().soars && sup().juggler && !sup().flies && !sup().rides_bike && !pet_cfg().soars);
}

/// 걷기·달리기 토막의 대부분은 날기 (걷기는 가끔 — 15% 안팎), 토막 안에서는 그대로 — 다른 동물은 안 낢
#[test]
fn most_move_stretches_are_flown_and_walking_is_rare() {
    let w = world(vec![]);
    let n = 300;
    let mut flown = 0;
    for seed in 0..n {
        let mut p = calm_super(sup(), 900.0, seed, &w);
        p.state = State::Move { kind: if seed % 2 == 0 { Move::Walk } else { Move::Run }, remaining: 3.0 };
        p.update(DT, &w);
        let first = p.soaring_now();
        if first {
            flown += 1;
            assert!(matches!(p.render().anim, Anim::Fly | Anim::FlyFast | Anim::FlyFlip), "{seed}: {:?}", p.render().anim);
        }
        for _ in 0..30 {
            p.update(DT, &w);
            if matches!(p.state, State::Move { .. }) {
                assert_eq!(p.soaring_now(), first, "토막 안에서는 그대로 ({seed})");
            }
        }
    }
    let share = flown as f64 / n as f64;
    assert!((soar::FLY_SHARE - 0.08..=soar::FLY_SHARE + 0.08).contains(&share), "{share}");
    assert!((0.78..=0.92).contains(&soar::FLY_SHARE), "걷기는 가끔만");
    let mut other = resting(pet_cfg(), 900.0, 1, &w);
    other.state = State::Move { kind: Move::Run, remaining: 3.0 };
    for _ in 0..60 {
        other.update(DT, &w);
        assert!(!other.soaring_now() && other.soar_lift() == 0.0 && other.render().anim != Anim::Fly);
    }
}

/// ✈️ 바닥(땅·창 윗면) 조금 위 (몸 크기 0.3~0.8)를 떠서 날고, 디딤판은 그대로 따라감 — 서서히 떠오름
#[test]
fn it_flies_a_little_above_its_floor_and_follows_it() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 500.0, 1700.0, 900.0) }]);
    for (owner, x) in [(Owner::Ground(0), 200.0), (Owner::Window(1), 400.0)] {
        for seed in 0..10 {
            let mut p = flying_super(x, seed, &w, owner, 6.0);
            let floor_y = p.body.y;
            let mut prev: Option<f64> = Some(0.0);
            for k in 0..(60 * 3) {
                p.update(DT, &w);
                if !matches!(p.state, State::Move { .. }) {
                    break;
                }
                assert_eq!(p.standing.map(|s| s.owner), Some(owner), "디딤판을 따라감");
                assert!((p.body.y - floor_y).abs() < 1e-6, "발 자리는 바닥 그대로");
                if p.render().anim == Anim::FlyFlip {
                    prev = None;
                } else {
                    if let Some(pv) = prev {
                        assert!(gap(&p) - pv <= soar::LIFT_RATE * p.cfg.size * DT + 1e-6, "서서히 떠오름 {k}");
                    }
                    prev = Some(gap(&p));
                }
                if k > 60 && p.render().anim != Anim::FlyFlip {
                    let h = gap(&p) / p.cfg.size;
                    assert!((soar::LIFT.0 - 1e-6..=soar::LIFT.1 + 1e-6).contains(&h), "{owner:?} {seed}: {h}");
                }
            }
        }
    }
    assert!(soar::LIFT.0 >= 0.3 && soar::LIFT.1 <= 0.8);
}

/// 빠르기가 천천히 바뀜: 느긋한 비행 ↔ 빠른 돌진 (빠르면 fly_fast 그림), 한 틱에 확 바뀌지 않음
#[test]
fn speed_drifts_between_a_slow_cruise_and_fast_dashes() {
    let w = world(vec![]);
    let (mut slow, mut fast, mut fast_anim) = (0, 0, 0);
    for seed in 0..6 {
        let mut p = flying_super(100.0, seed, &w, Owner::Ground(0), 1000.0);
        let walk = p.cfg.walk_speed;
        let mut prev: Option<f64> = None;
        for _ in 0..(60 * 40) {
            let x0 = p.body.x;
            p.update(DT, &w);
            if !matches!(p.state, State::Move { .. }) {
                p.state = State::Move { kind: Move::Walk, remaining: 1000.0 };
                p.facing = -p.facing;
                p.soar.force_fly = true;
                prev = None;
                continue;
            }
            let v = (p.body.x - x0).abs() / DT;
            if v < 1.0 {
                prev = None;
                continue;
            }
            if let Some(pv) = prev {
                assert!((v - pv).abs() < 0.08 * walk, "한 틱에 확 바뀌지 않음 {pv} → {v}");
            }
            prev = Some(v);
            if v < 2.0 * walk {
                slow += 1;
            }
            if v > 3.0 * walk {
                fast += 1;
            }
            fast_anim += (p.render().anim == Anim::FlyFast) as u32;
        }
    }
    assert!(slow > 600 && fast > 600, "느긋 {slow} · 돌진 {fast}");
    assert!(fast_anim > 300, "빠를 땐 fly_fast 그림 {fast_anim}");
}

/// 🔄 날다가 가끔 공중제비 한 바퀴 (fly_flip, 몸이 살짝 더 솟음) — 분에 몇 번
#[test]
fn it_somersaults_now_and_then_while_flying() {
    let w = world(vec![]);
    let mut p = flying_super(100.0, 7, &w, Owner::Ground(0), 1e9);
    let (mut flips, mut was, mut said, mut higher) = (0, false, false, false);
    let mins = 6.0;
    for _ in 0..(60.0 * 60.0 * mins) as usize {
        let before = gap(&p);
        p.update(DT, &w);
        if !matches!(p.state, State::Move { .. }) {
            p.state = State::Move { kind: Move::Walk, remaining: 1e9 };
            p.facing = -p.facing;
            p.soar.force_fly = true;
        }
        let now = p.render().anim == Anim::FlyFlip;
        if now && !was {
            flips += 1;
        }
        if now {
            higher |= gap(&p) > before + 0.1;
        }
        was = now;
        said |= p.take_speech() == Some(Speech::AirFlip);
    }
    let per_min = flips as f64 / mins;
    assert!((1.0..=10.0).contains(&per_min), "{per_min}/분");
    assert!(said && higher);
}

/// 창 높이에서 오른쪽 끝을 떠나면 (날다가든 걷다가든) 추락하지 않고 멀리 활공해 내려앉음 — 보통 동물이 뛰어내린 거리의 두 배 넘게
#[test]
fn leaving_a_ledge_it_glides_down_far_and_lands_softly() {
    let edge = 700.0;
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 400.0, edge, 900.0) }]);
    // 보통 동물: 창 끝을 달려 뛰어내림
    let mut normal = Pet::new(pet_cfg(), 0.0, 0.0, 3);
    let nx = edge - normal.cfg.half_w - 0.5;
    place(&mut normal, &w, Owner::Window(1), nx, State::Move { kind: Move::Run, remaining: 100.0 });
    normal.facing = 1.0;
    let mut guard = LoopLimit::new();
    while normal.standing.is_some_and(|s| s.owner == Owner::Window(1)) || !matches!(normal.state, State::Land { .. } | State::Idle { .. }) {
        guard.step();
        normal.update(DT, &w);
    }
    let normal_dist = normal.body.x - edge;
    assert!(normal_dist > 0.0);
    let (mut glides, mut turns) = (0, 0);
    for seed in 0..30 {
        let walking = seed % 3 == 0;
        let mut p = flying_super(edge - 200.0, seed, &w, Owner::Window(1), 100.0);
        if walking {
            p.soar.force_fly = false;
            p.soar.walk_next = true;
        }
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Move { .. }) {
            guard.step();
            p.update(DT, &w);
        }
        if p.standing.is_some() {
            assert!(matches!(p.state, State::Turn { .. }), "{seed}: {:?}", p.state);
            turns += 1;
            continue;
        }
        glides += 1;
        assert!(matches!(p.state, State::Air { .. }));
        let mut guard = LoopLimit::new();
        let mut thud = false;
        while matches!(p.state, State::Air { .. }) {
            guard.step();
            assert!(matches!(p.render().anim, Anim::Fly | Anim::FlyFast), "추락 그림 대신 날기 {:?}", p.render().anim);
            assert!(p.body.vy <= soar::GLIDE_MAX_VY * p.cfg.size + 1e-6, "천천히 내려감 {}", p.body.vy);
            p.update(DT, &w);
            thud |= p.render().thud;
        }
        assert!(matches!(p.state, State::Land { .. }) && !thud && p.speech != Some(Speech::Oof), "사뿐 {seed}");
        assert_eq!(p.render().anim, Anim::Hover, "사뿐 착지 그림");
        let dist = p.body.x - edge;
        assert!(dist > 2.0 * normal_dist, "{seed}: {dist} vs 보통 {normal_dist}");
    }
    assert!(glides >= 12 && turns >= 3, "{glides} {turns}");
}

/// 디딤판이 사라져도 (창이 닫힘) 떨어지지 않고 활공
#[test]
fn a_vanishing_floor_turns_into_a_glide() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(300.0, 400.0, 1100.0, 900.0) }]);
    let mut p = calm_super(sup(), 600.0, 1, &w);
    place(&mut p, &w, Owner::Window(1), 600.0, State::Idle { remaining: 10.0, look: false });
    let empty = world(vec![]);
    p.update(DT, &empty);
    assert!(matches!(p.state, State::Air { .. }));
    assert!(matches!(p.render().anim, Anim::Fly | Anim::FlyFast));
    assert!(p.body.vx.abs() > p.cfg.walk_speed);
}

/// 높이서 떨어져도 (던져져도) 콰당 없이 사뿐 (hover)
#[test]
fn landings_are_always_soft() {
    let w = world(vec![]);
    let mut p = calm_super(sup(), 900.0, 2, &w);
    p.standing = None;
    p.body = Body { x: 900.0, y: 100.0, vx: 0.0, vy: 0.0 };
    p.state = State::Air { anim: Anim::Fall };
    p.thrown = true;
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Air { .. }) {
        guard.step();
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Land { .. }) && !p.render().thud);
    assert_eq!(p.render().anim, Anim::Hover);
}

/// 날던 토막이 끝나면 똑바로 서서 (hover) 천천히 내려앉음 — 그 뒤엔 바닥에 섬
#[test]
fn after_a_flight_it_settles_down_upright() {
    let w = world(vec![]);
    let mut p = flying_super(300.0, 4, &w, Owner::Ground(0), 2.0);
    let mut guard = LoopLimit::new();
    while matches!(p.state, State::Move { .. }) {
        guard.step();
        p.update(DT, &w);
    }
    let start = gap(&p);
    assert!(start > 0.2 * p.cfg.size, "{start}");
    let mut hovered = false;
    for _ in 0..(60 * 3) {
        let before = gap(&p);
        p.update(DT, &w);
        if !matches!(p.state, State::Idle { .. } | State::Turn { .. } | State::Land { .. }) {
            break;
        }
        assert!(before - gap(&p) <= soar::LIFT_RATE * p.cfg.size * DT + 1e-6, "천천히");
        if gap(&p) > soar::HOVER_MIN * p.cfg.size {
            hovered |= p.render().anim == Anim::Hover;
        }
    }
    assert!(hovered);
}

/// 🚪 친구가 연결된 끝으로 날아서 나가고 ("to the rescue!"), 친구 화면에도 날아 들어와 계속 낢. 위·아래 끝은 똑바로 떠서 (hover)
#[test]
fn it_flies_out_to_a_friend_and_flies_in() {
    let w = world(vec![]);
    let mut out = None;
    for seed in 0..200 {
        let mut p = flying_super(900.0, seed, &w, Owner::Ground(0), 100.0);
        let x = w.bounds.right - p.cfg.half_w - 0.5;
        place(&mut p, &w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.soar.force_fly = true;
        p.set_exit_edges(false, true);
        p.update(DT, &w);
        if matches!(p.state, State::Exit { .. }) {
            out = Some(p);
            break;
        }
    }
    let mut p = out.expect("200 시드 동안 안 나감");
    assert_eq!(p.render().anim, Anim::Fly);
    assert_eq!(p.speech, Some(Speech::Rescue));
    let x0 = p.body.x;
    p.update(DT, &w);
    assert!(p.body.x - x0 > p.cfg.walk_speed * 1.2 * DT, "날아서 빠르게");
    let mut guard = LoopLimit::new();
    let mut top = 0.0f64;
    while p.take_exit().is_none() {
        guard.step();
        p.update(DT, &w);
        top = top.max(gap(&p));
    }
    assert!(top > 0.2 * p.cfg.size, "떠서 나감 {top}");
    let mut g = Pet::new_entering(sup(), &w, Side::Left, 3);
    assert!(matches!(g.state, State::Enter { .. }));
    assert_eq!(g.render().anim, Anim::Fly);
    assert!(gap(&g) > 0.2 * g.cfg.size, "떠서 들어옴");
    let mut guard = LoopLimit::new();
    while matches!(g.state, State::Enter { .. }) {
        guard.step();
        g.update(DT, &w);
    }
    assert!(matches!(g.state, State::Move { .. }) && g.soaring_now(), "들어와서도 계속 낢 {:?}", g.state);
    let mut v = calm_super(sup(), 900.0, 1, &w);
    v.state = State::Exit { remaining: 1.0, side: Side::Up };
    assert_eq!(v.render().anim, Anim::Hover);
}

/// 💪 거대한 것도 늘 번쩍 (털썩 없음, 짧게 "easy!") — 들고 다닐 땐 날아서 (떠서, 걷기보다 빠르게)
#[test]
fn giants_are_lifted_one_handed_and_carried_in_flight() {
    let w = world(vec![]);
    let gy = w.anchor(Owner::Ground(0)).unwrap().1;
    let g = ObjectSpot { id: 5, x: 1000.0, y: gy, owner: Owner::Ground(0), half_w: 236.0, giant: true, local: false };
    let (mut carried_aloft, mut moved) = (0, 0.0);
    for seed in 0..40 {
        let mut p = flying_super(600.0, seed, &w, Owner::Ground(0), 1.0);
        p.state = State::Juggle { phase: JugglePhase::Approach { id: 5, x: 1000.0 - 236.0 - 0.28 * p.cfg.size, obj_x: 1000.0, giant: true }, t: 0.0 };
        let (mut strained, mut lifted, mut said) = (0.0, false, false);
        for _ in 0..(60 * 20) {
            let spots: &[ObjectSpot] = if p.carried_object().is_some() { &[] } else { std::slice::from_ref(&g) };
            p.notice_objects(spots, &[], &w);
            let x0 = p.body.x;
            p.update(DT, &w);
            let ev = p.take_juggle();
            match p.state {
                State::Juggle { phase: JugglePhase::Strain { .. }, .. } => strained += DT,
                State::Juggle { phase: JugglePhase::Slump, .. } => panic!("털썩 없음 {seed}"),
                State::Juggle { phase: JugglePhase::Lift, .. } => lifted = true,
                State::Juggle { phase: JugglePhase::Wander { .. }, t } if t > 1.0 => {
                    assert!(gap(&p) > 0.2 * p.cfg.size, "날아서 옮김");
                    moved += (p.body.x - x0).abs();
                    carried_aloft += 1;
                }
                _ => {}
            }
            said |= p.speech == Some(Speech::HeroLift);
            if matches!(ev, Some(JuggleEvent::Release { .. })) || (lifted && !matches!(p.state, State::Juggle { .. })) {
                break;
            }
        }
        assert!(lifted && said, "{seed}");
        assert!(strained <= crate::bike::HERO_STRAIN_SECS + 0.05, "{strained}");
    }
    assert!(carried_aloft > 30);
    let pace = moved / (carried_aloft as f64 * DT);
    let walk = sup().walk_speed * juggle::WANDER_PACE;
    assert!(pace > 1.3 * walk, "들고 걷기보다 빠르게 {pace} vs {walk}");
}

/// 🌱 연령대 세기: 아기는 느리게·덜 뒤집고, 어른은 빠르게 (soft_power·power)
#[test]
fn babies_fly_slower_than_adults() {
    let w = world(vec![]);
    let speed = |stage: crate::care::Stage| {
        let mut p = flying_super(100.0, 5, &w, Owner::Ground(0), 1000.0);
        p.cfg.stage = stage;
        p.soar_speed()
    };
    assert!(speed(crate::care::Stage::Baby) < speed(crate::care::Stage::Teenager));
    assert!(speed(crate::care::Stage::Adult) > speed(crate::care::Stage::Teenager));
}

/// 🐞 0.7.0: 창 옆면을 타다 앞 창에 가려져 떨어지면 활공이 그 창 쪽으로 날아가 다시 붙고 → 또 떨어지고를 매 틱 되풀이해
/// 화면 위쪽에 멈춘 채 버벅였음. 벽·창 옆면에서 떨어질 때는 활공하지 않고 그냥 떨어지고, 활공 중엔 벽에 붙지 않음
#[test]
fn falling_off_a_covered_window_side_does_not_loop_back_onto_the_wall() {
    // 창 1 (뒤) 왼쪽 옆면을 타는 중, 그 옆면 높이를 창 2 (앞)가 덮고 있음
    let w = world(vec![
        WindowInfo { id: 2, rect: Rect::new(650.0, 100.0, 900.0, 400.0) },
        WindowInfo { id: 1, rect: Rect::new(800.0, 150.0, 1400.0, 900.0) },
    ]);
    let mut p = Pet::new(sup(), 800.0 - sup().half_w, 300.0, 3);
    p.state = State::WallClimb { window: 1, side: Side::Left, rel_y: 150.0 };
    p.facing = 1.0;
    let (mut sticks, mut was_wall) = (0, true);
    let y0 = p.body.y;
    for _ in 0..(60 * 3) {
        p.update(DT, &w);
        let wall = matches!(p.state, State::WallClimb { .. } | State::Climb { .. });
        sticks += usize::from(wall && !was_wall);
        was_wall = wall;
    }
    assert!(sticks <= 1, "벽에 다시 붙었다 떨어지기를 되풀이함: {sticks}번");
    assert!(p.body.y > y0 + 100.0, "아래로 내려감: {} → {}", y0, p.body.y);
}

/// 활공은 서 있던 디딤판 끝에서만 — 벽에서 떨어지거나 사다리·탑 등에서 떨어지면 평소처럼 떨어짐 (사뿐 착지는 그대로)
#[test]
fn a_glide_only_starts_from_standing_on_a_platform() {
    let w = world(vec![]);
    let mut p = Pet::new(sup(), 1920.0 - sup().half_w, 500.0, 3);
    p.state = State::Climb { target_y: 300.0, hang: 1.0, over: false };
    p.start_fall();
    assert!(!p.soar.gliding, "벽에서 떨어질 땐 활공하지 않음");
}

/// 🐞 0.7.0 재현 찾기: 창 여럿 (위쪽 창 포함)인 화면에서 슈퍼인간을 오래 돌려, 화면 위쪽에서 상태가 마구 바뀌며 멈춰 있는 구간이 없음
#[test]
fn super_never_gets_stuck_jittering_near_the_top_of_the_screen() {
    let w = world(vec![
        WindowInfo { id: 1, rect: Rect::new(100.0, 20.0, 900.0, 500.0) },
        WindowInfo { id: 2, rect: Rect::new(700.0, 0.0, 1500.0, 300.0) },
        WindowInfo { id: 3, rect: Rect::new(1200.0, 400.0, 1900.0, 1000.0) },
    ]);
    for seed in 0..12 {
        let mut p = Pet::new(sup(), 400.0, 0.0, seed);
        let (mut changes, mut last, mut top_secs) = (0usize, String::new(), 0.0);
        let mut hist: std::collections::VecDeque<String> = Default::default();
        let mut worst = 0usize;
        for tick in 0..(60 * 240) {
            p.update(DT, &w);
            let name = format!("{:?}", p.state).split([' ', '{']).next().unwrap_or("").to_string();
            let near_top = p.body.y - p.soar_lift() < 120.0 + p.cfg.size;
            if near_top {
                top_secs += DT;
                if name != last {
                    changes += 1;
                }
            } else {
                top_secs = 0.0;
                changes = 0;
            }
            if name != last {
                hist.push_back(format!("{:?} y={:.0} x={:.0} vx={:.0} vy={:.0} st={:?}", p.state, p.body.y, p.body.x, p.body.vx, p.body.vy, p.standing.map(|s| s.owner)));
                if hist.len() > 8 {
                    hist.pop_front();
                }
            }
            last = name;
            if top_secs > 3.0 {
                worst = worst.max(changes);
                assert!(changes < 60, "seed {seed} tick {tick}: 화면 위쪽 3초에 상태가 {changes}번 바뀜 — {:?} y={} lift={}\n{:#?}", p.state, p.body.y, p.soar_lift(), hist);
            }
        }
        let _ = worst;
    }
}
