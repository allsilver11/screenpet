//! 🦇 박쥐인간 (brain_bike.rs): 🏍️ 자주 오토바이 타기·빠르기·윌리·디딤판 끝 점프/드리프트·부딪친 동물 살짝 튕김·
//! 🔦 박쥐 신호·오토바이로 친구 화면 오가기·💪 거대한 것도 늘 번쩍

use super::bike_moves::{BatSignal, SIGNAL_SECS};
use super::hoist_moves::{notice_pets, sync_lifts};
use super::juggle_moves::JugglePhase;
use super::swing_tests::resting;
use super::tests::{place, world};
use super::*;
use crate::bike;
use crate::geom::{Rect, WindowInfo};
use crate::juggle::{JuggleEvent, ObjectSpot};
use crate::test_util::{bundled, pet_cfg, LoopLimit, DT};

/// 번들 pet.json 그대로의 박쥐인간 (청소년)
fn bat() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "bat").expect("ui/pets/bat").1
}

/// 바닥(x)에 쉬는 박쥐인간 — 박쥐 신호·연령대 몸짓은 꺼 둠
fn calm_bat(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(bat(), x, seed, w);
    p.hush_stage();
    p.bike.signal_ready_at = f64::INFINITY;
    p
}

/// 오른쪽으로 오토바이를 타고 달리는 박쥐인간 (이번 토막은 반드시 탐)
fn riding_bat(x: f64, seed: u64, w: &World, owner: Owner) -> Pet {
    let mut p = calm_bat(x, seed, w);
    place(&mut p, w, owner, x, State::Move { kind: Move::Walk, remaining: 100.0 });
    p.facing = 1.0;
    p.bike.force_ride = true;
    p
}

#[test]
fn bike_animations_and_lines_have_their_sheet_names() {
    for (a, name) in [(Anim::Ride, "ride"), (Anim::Wheelie, "wheelie"), (Anim::BikeJump, "bike_jump"), (Anim::Drift, "drift"), (Anim::Signal, "signal")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(!a.is_front() && !a.is_back());
        assert!(crate::manifest::BIKE_ANIMS.contains(&name));
    }
    for (s, name) in [(Speech::Vroom, "vroom"), (Speech::Wheelie, "wheelie"), (Speech::BikeTrick, "bike_trick"), (Speech::BatSignal, "bat_signal"), (Speech::HeroLift, "hero_lift")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
    assert!(bat().rides_bike && bat().juggler && !pet_cfg().rides_bike);
}

/// 걷기·달리기 토막의 절반쯤은 오토바이 (토막 안에서는 그대로) — 다른 동물은 안 탐
#[test]
fn about_half_of_the_move_stretches_are_on_the_bike() {
    let w = world(vec![]);
    let n = 300;
    let mut rides = 0;
    for seed in 0..n {
        let mut p = calm_bat(900.0, seed, &w);
        p.state = State::Move { kind: if seed % 2 == 0 { Move::Walk } else { Move::Run }, remaining: 3.0 };
        p.update(DT, &w);
        let first = p.riding_now();
        if first {
            rides += 1;
            assert!(matches!(p.render().anim, Anim::Ride | Anim::Wheelie), "{seed}: {:?}", p.render().anim);
        }
        for _ in 0..30 {
            p.update(DT, &w);
            if matches!(p.state, State::Move { .. }) {
                assert_eq!(p.riding_now(), first, "토막 안에서는 그대로 ({seed})");
            }
        }
    }
    let share = rides as f64 / n as f64;
    assert!((bike::RIDE_SHARE - 0.1..=bike::RIDE_SHARE + 0.1).contains(&share), "{share}");
    assert!((0.4..=0.6).contains(&bike::RIDE_SHARE));
    let mut other = resting(pet_cfg(), 900.0, 1, &w);
    other.state = State::Move { kind: Move::Run, remaining: 3.0 };
    for _ in 0..60 {
        other.update(DT, &w);
        assert!(!other.riding_now() && other.render().anim != Anim::Ride);
    }
}

/// 오토바이는 달리기보다도 빠름
#[test]
fn the_bike_is_faster_than_running() {
    let w = world(vec![]);
    let mut p = riding_bat(300.0, 3, &w, Owner::Ground(0));
    let x0 = p.body.x;
    for _ in 0..30 {
        p.update(DT, &w);
    }
    let speed = (p.body.x - x0) / (30.0 * DT);
    let run = p.cfg.walk_speed * Move::Run.speed_mult();
    assert!(speed > run * 1.1, "{speed} vs run {run}");
    assert!((speed - p.bike_speed()).abs() < p.bike_speed() * 0.05, "{speed} vs {}", p.bike_speed());
}

/// 타는 동안 꽤 자주 윌리 ("yeah!") — 뒷바퀴로만 한동안
#[test]
fn it_pops_wheelies_fairly_often_while_riding() {
    let w = world(vec![]);
    let (mut ride, mut wheelie, mut said, mut stretches) = (0, 0, false, 0);
    for seed in 0..6 {
        let mut p = riding_bat(200.0, seed, &w, Owner::Ground(0));
        p.state = State::Move { kind: Move::Walk, remaining: 8.0 };
        let mut in_wheelie = false;
        for _ in 0..(60 * 8) {
            p.update(DT, &w);
            if !matches!(p.state, State::Move { .. }) {
                break;
            }
            match p.render().anim {
                Anim::Ride => ride += 1,
                Anim::Wheelie => {
                    wheelie += 1;
                    if !in_wheelie {
                        stretches += 1;
                    }
                }
                a => panic!("{a:?}"),
            }
            in_wheelie = p.render().anim == Anim::Wheelie;
            said |= p.speech == Some(Speech::Wheelie);
        }
    }
    assert!(said, "윌리 때 yeah!");
    let share = wheelie as f64 / (ride + wheelie) as f64;
    assert!((0.1..=0.5).contains(&share), "{share}");
    assert!(stretches >= 3, "{stretches}");
    assert!(wheelie as f64 / stretches as f64 * DT >= bike::WHEELIE_SECS.0 * 0.9, "한동안 이어짐");
}

/// 창 위를 오토바이로 달리다 끝에 닿으면: 붕 뛰어내리거나 (bike_jump) 끼익 멈춤 (drift) — 둘 다 나옴
#[test]
fn at_a_platform_edge_it_jumps_off_or_skids_to_a_stop() {
    let w = world(vec![WindowInfo { id: 1, rect: Rect::new(600.0, 500.0, 1100.0, 900.0) }]);
    let (mut jumps, mut stops) = (0, 0);
    for seed in 0..40 {
        let mut p = riding_bat(1000.0, seed, &w, Owner::Window(1));
        let mut guard = LoopLimit::new();
        while matches!(p.state, State::Move { .. }) {
            guard.step();
            p.update(DT, &w);
        }
        match p.state {
            State::Air { .. } => {
                assert_eq!(p.render().anim, Anim::BikeJump, "{seed}");
                assert!(p.body.vx > p.cfg.walk_speed, "앞으로 붕 {seed}");
                jumps += 1;
                // 내려서면 점프 그림은 끝
                let mut guard = LoopLimit::new();
                while matches!(p.state, State::Air { .. }) {
                    guard.step();
                    p.update(DT, &w);
                }
                assert_ne!(p.render().anim, Anim::BikeJump);
            }
            _ => {
                assert_eq!(p.render().anim, Anim::Drift, "{seed}: {:?}", p.state);
                assert!(p.standing.is_some() && p.body.x < 1100.0, "창 위에 멈춤");
                stops += 1;
            }
        }
    }
    assert!(jumps >= 10 && stops >= 6, "{jumps} {stops}");
}

/// 달리다 앞의 동물에 부딪치면 그 동물이 살짝 튕김 (한 번 — 금방 또 튕기지 않음)
#[test]
fn bumping_another_pet_bounces_it_a_little() {
    let w = world(vec![]);
    let mut pets = vec![riding_bat(600.0, 4, &w, Owner::Ground(0)), resting(pet_cfg(), 760.0, 9, &w)];
    pets[1].social_key = 77;
    pets[0].social_key = 11;
    let mut bounced = 0;
    let mut was_air = false;
    for _ in 0..(60 * 3) {
        notice_pets(&mut pets);
        for p in pets.iter_mut() {
            p.update(DT, &w);
        }
        sync_lifts(&mut pets);
        let air = matches!(pets[1].state, State::Air { .. });
        if air && !was_air {
            bounced += 1;
            assert!(pets[1].body.vx > 0.0 && pets[1].body.vx <= bike::BUMP_VX * pets[1].cfg.dpi + 1e-9, "앞으로 살짝 {}", pets[1].body.vx);
            assert!(pets[1].body.vy < 0.0, "통 튀어 오름");
        }
        was_air = air;
    }
    assert_eq!(bounced, 1);
}

/// 🔦 박쥐 신호: 서서 조명을 켜고 (signal) 앱에 빛줄기 자리를 한 번 알림 — 몇 초 뒤 끄고 평소대로
#[test]
fn the_bat_signal_shines_once_for_a_few_seconds() {
    let w = world(vec![]);
    let mut p = calm_bat(900.0, 2, &w);
    assert!(p.start_signal());
    assert_eq!(p.render().anim, Anim::Signal);
    assert_eq!(p.speech, Some(Speech::BatSignal));
    let s: BatSignal = p.take_bat_signal().expect("빛줄기");
    assert!(p.take_bat_signal().is_none(), "한 번만");
    assert!((s.x - p.body.x).abs() < p.cfg.size * 0.5 && s.y < p.body.y - 0.2 * p.cfg.size && s.y > p.body.y - p.cfg.size, "{s:?}");
    assert!((s.secs - SIGNAL_SECS).abs() < 1e-9 && s.size == p.cfg.size);
    let mut t = 0.0;
    while matches!(p.state, State::Signal { .. }) {
        assert_eq!(p.render().anim, Anim::Signal);
        p.update(DT, &w);
        t += DT;
        assert!(t < SIGNAL_SECS + 1.0);
    }
    assert!(t >= SIGNAL_SECS - 0.1, "{t}");
}

/// 🔦 저절로는 몇 분에 한 번쯤 — 다른 동물은 안 함
#[test]
fn the_signal_comes_every_few_minutes_on_its_own() {
    let w = world(vec![]);
    let mut p = resting(bat(), 900.0, 5, &w);
    p.hush_stage();
    p.bike.signal_ready_at = 0.0;
    let mut n = 0;
    for _ in 0..(60 * 60 * 30) {
        p.update(DT, &w);
        n += p.take_bat_signal().is_some() as u32;
    }
    assert!((3..=14).contains(&n), "30분에 {n}번");
    let mut other = resting(pet_cfg(), 900.0, 5, &w);
    other.bike.signal_ready_at = 0.0;
    for _ in 0..(60 * 60 * 3) {
        other.update(DT, &w);
        assert!(other.take_bat_signal().is_none());
    }
}

/// 🚪 친구가 연결된 끝으로 오토바이 타고 부릉 나가고, 친구 화면에서도 오토바이 타고 들어와 계속 달림
#[test]
fn it_rides_out_to_a_friend_and_rides_in() {
    let w = world(vec![]);
    let mut out = None;
    for seed in 0..200 {
        let mut p = calm_bat(900.0, seed, &w);
        let x = w.bounds.right - p.cfg.half_w - 0.5;
        place(&mut p, &w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
        p.facing = 1.0;
        p.set_exit_edges(false, true);
        p.update(DT, &w);
        if matches!(p.state, State::Exit { .. }) {
            out = Some(p);
            break;
        }
    }
    let mut p = out.expect("200 시드 동안 안 나감");
    assert_eq!(p.render().anim, Anim::Ride);
    assert_eq!(p.speech, Some(Speech::Vroom));
    let x0 = p.body.x;
    p.update(DT, &w);
    assert!((p.body.x - x0 - p.bike_speed() * DT).abs() < 1e-6, "오토바이 빠르기로");
    let mut guard = LoopLimit::new();
    while p.take_exit().is_none() {
        guard.step();
        p.update(DT, &w);
    }
    let mut g = Pet::new_entering(bat(), &w, Side::Left, 3);
    assert!(matches!(g.state, State::Enter { .. }));
    assert_eq!(g.render().anim, Anim::Ride);
    let mut guard = LoopLimit::new();
    while matches!(g.state, State::Enter { .. }) {
        guard.step();
        g.update(DT, &w);
    }
    assert!(matches!(g.state, State::Move { .. }) && g.riding_now(), "들어와서도 계속 달림 {:?}", g.state);
    // 물건을 들고 나갈 땐 (💪 배달) 오토바이 대신 들고 달림
    let mut c = calm_bat(900.0, 1, &w);
    c.juggle.carry = Some(4);
    c.state = State::Exit { remaining: 1.0, side: Side::Right };
    assert_ne!(c.render().anim, Anim::Ride);
}

/// 💪 거대한 것도 늘 번쩍 (털썩 없음) — 힘주기는 짧게, "easy!"
#[test]
fn giants_are_always_lifted_after_a_short_heave() {
    let w = world(vec![]);
    let gy = w.anchor(Owner::Ground(0)).unwrap().1;
    let g = ObjectSpot { id: 5, x: 1000.0, y: gy, owner: Owner::Ground(0), half_w: 236.0, giant: true, local: false };
    for seed in 0..40 {
        let mut p = riding_bat(600.0, seed, &w, Owner::Ground(0));
        p.state = State::Juggle { phase: JugglePhase::Approach { id: 5, x: 1000.0 - 236.0 - 0.28 * p.cfg.size, obj_x: 1000.0, giant: true }, t: 0.0 };
        let (mut strained, mut lifted, mut said) = (0.0, false, false);
        for _ in 0..(60 * 20) {
            let spots: &[ObjectSpot] = if p.carried_object().is_some() { &[] } else { std::slice::from_ref(&g) };
            p.notice_objects(spots, &[], &w);
            p.update(DT, &w);
            let ev = p.take_juggle();
            match p.state {
                State::Juggle { phase: JugglePhase::Strain { .. }, .. } => strained += DT,
                State::Juggle { phase: JugglePhase::Slump, .. } => panic!("털썩 없음 {seed}"),
                State::Juggle { phase: JugglePhase::Lift, .. } => lifted = true,
                _ => {}
            }
            said |= p.speech == Some(Speech::HeroLift);
            if matches!(ev, Some(JuggleEvent::Release { .. })) || (lifted && !matches!(p.state, State::Juggle { .. })) {
                break;
            }
        }
        assert!(lifted && said, "{seed}");
        assert!(strained <= bike::HERO_STRAIN_SECS + 0.05, "{strained}");
    }
}
