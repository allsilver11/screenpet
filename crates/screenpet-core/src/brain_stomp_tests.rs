//! 공룡 🦖 — 불 뿜기·크앙·쿵쾅 (Special::Fire/Roar/Stomp)과 쿵쾅에 흔들리고 데굴 넘어지기 (brain_stomp.rs).
//! brain.rs의 자식 모듈 (비공개 필드에 접근). 누가 흔들리는지(같은 바닥·거리·나는 동물)는 social_stomp_tests.rs

use super::stomp_moves::{STOMP_CALM_SECS, TUMBLE_SECS, WOBBLE_SECS};
use super::tests::{cfg, place, sim_cfg, w1, world};
use super::*;

const DT: f64 = 1.0 / 60.0;

fn dino() -> PetConfig {
    PetConfig { special: Some(Special::Fire), ..sim_cfg() }
}

fn resting() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// 공룡 동작을 시작하면 셋 중 하나 — 동작마다 말풍선이 다르고, 쿵쾅만 친구들에게 알린다 (한 번)
#[test]
fn a_dino_special_is_fire_roar_or_stomp_with_its_own_speech() {
    let w = world(vec![]);
    let mut seen = Vec::new();
    for seed in 0..60 {
        let mut p = Pet::new(dino(), 0.0, 0.0, seed);
        place(&mut p, &w, Owner::Ground(0), 800.0, resting());
        p.start_special(Special::Fire);
        let State::Special { kind, remaining } = p.state else { panic!("{:?}", p.state) };
        let (lo, hi) = kind.secs();
        assert!(lo <= remaining && remaining <= hi);
        let (anim, speech) = match kind {
            Special::Fire => (Anim::Fire, Speech::Fire),
            Special::Roar => (Anim::Roar, Speech::Roar),
            Special::Stomp => (Anim::Stomp, Speech::Stomp),
            other => panic!("{other:?}"),
        };
        assert_eq!(p.render().anim, anim);
        assert!(!anim.is_front() && !anim.is_back(), "side view — flips with facing");
        assert_eq!(p.take_speech(), Some(speech));
        assert_eq!(p.take_stomp(), kind == Special::Stomp);
        assert!(!p.take_stomp());
        if !seen.contains(&kind) {
            seen.push(kind);
        }
    }
    assert_eq!(seen.len(), 3, "{seen:?}");
}

/// 다른 동물의 특별 동작은 그대로 (고르기 없음, 쿵쾅 알림 없음)
#[test]
fn other_specials_are_not_reshuffled() {
    let mut p = Pet::new(PetConfig { special: Some(Special::Wag), ..cfg() }, 0.0, 0.0, 3);
    p.start_special(Special::Wag);
    assert!(matches!(p.state, State::Special { kind: Special::Wag, .. }));
    assert!(!p.take_stomp());
}

/// 10분 × 3 — 쉬다가 불·크앙·쿵쾅이 모두 나온다
#[test]
fn a_dino_breathes_fire_roars_and_stomps_while_resting() {
    let w = world(vec![w1()]);
    let mut counts = [0usize; 3];
    for seed in 0..3 {
        let mut p = Pet::new(dino(), 960.0, 128.0, seed);
        let mut last = None;
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            let a = p.render().anim;
            if Some(a) != last {
                match a {
                    Anim::Fire => counts[0] += 1,
                    Anim::Roar => counts[1] += 1,
                    Anim::Stomp => counts[2] += 1,
                    _ => {}
                }
            }
            last = Some(a);
            p.take_stomp();
        }
    }
    assert!(counts.iter().all(|&c| c >= 3), "{counts:?}");
}

/// 간질이면 가끔 (30%) 불을 퐁 뿜거나 크앙 (쿵쾅은 드물게)
#[test]
fn tickling_a_dino_sometimes_makes_it_puff_fire_or_roar() {
    let w = world(vec![]);
    let (mut special, mut fire_or_roar) = (0usize, 0usize);
    let n = 2000;
    for seed in 0..n {
        let mut p = Pet::new(dino(), 0.0, 0.0, seed as u64);
        place(&mut p, &w, Owner::Ground(0), 800.0, resting());
        p.tickling = true;
        p.react_to_tickle();
        if let State::Special { kind, .. } = p.state {
            special += 1;
            if kind != Special::Stomp {
                fire_or_roar += 1;
            }
        }
    }
    let share = special as f64 / n as f64;
    assert!((share - 0.30).abs() < 0.04, "{share}");
    assert!(fire_or_roar as f64 / special as f64 > 0.75);
}

/// 데굴: 공룡 반대쪽으로 구르며 미끄러지다 창 윗면 끝에서 멈추고 (떨어지지 않는다), 착지 자세로 일어선다
#[test]
fn a_tumble_rolls_away_and_stops_at_the_ledge() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    let edge = 1100.0 - p.cfg.half_w;
    place(&mut p, &w, Owner::Window(1), edge - 6.0, resting());
    assert!(p.feel_stomp(edge - 80.0, true));
    assert!(matches!(p.state, State::Tumble { dir, .. } if dir == 1.0), "{:?}", p.state);
    assert_eq!(p.take_speech(), Some(Speech::Whoa));
    let r = p.render();
    assert!(r.wobble && r.anim == Anim::Roll);
    let mut ended = false;
    for _ in 0..((TUMBLE_SECS.1 + 0.5) / DT) as usize {
        p.update(DT, &w);
        assert!(p.body.x <= edge + 1e-6, "{}", p.body.x);
        assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "{:?}", p.state);
        if !matches!(p.state, State::Tumble { .. }) {
            ended = true;
            break;
        }
    }
    assert!(ended);
    assert!(matches!(p.state, State::Land { .. }), "{:?}", p.state);
    assert!((p.body.x - edge).abs() < 1e-6, "slid to the edge and stopped: {}", p.body.x);
    assert!(!p.render().wobble);
}

/// 이미 끝을 넘어 서 있으면 그 자리에서 데굴 — 더 나가지 않는다. 가운데서는 몸 반 개쯤 미끄러진다
#[test]
fn a_tumble_never_pushes_a_pet_off_a_ledge() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    let x = 1100.0 - p.cfg.half_w + 10.0;
    place(&mut p, &w, Owner::Window(1), x, resting());
    assert!(p.feel_stomp(900.0, true));
    for _ in 0..90 {
        p.update(DT, &w);
    }
    assert!(p.body.x <= x + 1e-6 && p.standing.is_some());

    let mut q = Pet::new(cfg(), 0.0, 0.0, 2);
    place(&mut q, &w, Owner::Window(1), 800.0, resting());
    assert!(q.feel_stomp(850.0, true));
    for _ in 0..90 {
        q.update(DT, &w);
    }
    let moved = 800.0 - q.body.x;
    assert!(moved > 20.0 && moved < 110.0, "{moved}");
}

/// 흔들림만 (데굴 아님): 하던 걸 계속하고 WOBBLE_SECS 뒤 멈춘다. STOMP_CALM_SECS 동안은 다시 흔들리지 않는다
#[test]
fn a_wobble_is_short_and_there_is_a_cooldown() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, resting());
    assert!(p.feel_stomp(700.0, false));
    assert!(matches!(p.state, State::Idle { .. }) && p.wobbling());
    for _ in 0..((WOBBLE_SECS + 0.05) / DT) as usize {
        p.update(DT, &w);
    }
    assert!(!p.wobbling());
    place(&mut p, &w, Owner::Ground(0), 800.0, resting());
    assert!(!p.feel_stomp(700.0, true));
    for _ in 0..(STOMP_CALM_SECS / DT) as usize {
        p.update(DT, &w);
        if !matches!(p.state, State::Idle { .. }) {
            place(&mut p, &w, Owner::Ground(0), 800.0, resting());
        }
    }
    assert!(p.feel_stomp(700.0, false));
}

/// 자던 동물은 투덜("...") 깼다가 다시 잠든다
#[test]
fn a_sleeping_pet_wakes_grumpy_then_dozes_off() {
    let w = world(vec![]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, State::Sleep { remaining: 30.0 });
    assert!(p.feel_stomp(700.0, true));
    assert!(matches!(p.state, State::Annoyed { .. }));
    assert_eq!(p.take_speech(), Some(Speech::Ignore));
    for _ in 0..(2.0 / DT) as usize {
        p.update(DT, &w);
    }
    assert!(matches!(p.state, State::Sleep { .. }), "{:?}", p.state);
}

/// 들려 있거나 먹는 중·하우스 안·떠나 있음·공중이면 그대로
#[test]
fn busy_pets_do_not_feel_the_stomp() {
    let w = world(vec![]);
    for state in [State::Dragged, State::Pressed { start_cx: 0.0, start_cy: 0.0, grounded: true, was_sleeping: false },
        State::Eating { remaining: 3.0, favorite: false, level_up: false }, State::Away, State::Air { anim: Anim::Fall },
        State::Climb { target_y: 300.0, hang: 0.0 }] {
        let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
        place(&mut p, &w, Owner::Ground(0), 800.0, state);
        assert!(!p.feel_stomp(700.0, true), "{state:?}");
        assert_eq!(p.state, state);
        assert!(!p.render().wobble);
    }
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, resting());
    p.housed = true;
    assert!(!p.feel_stomp(700.0, true));
}

/// 나는 동물은 떠서도 내려앉아서도 흔들리기만 한다
#[test]
fn flyers_only_wobble() {
    let w = world(vec![]);
    let mut p = Pet::new(PetConfig { flies: true, ..cfg() }, 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 800.0, resting());
    assert!(p.feel_stomp(700.0, true));
    assert!(matches!(p.state, State::Idle { .. }) && p.wobbling());
    let mut q = Pet::new(PetConfig { flies: true, ..cfg() }, 800.0, 500.0, 2);
    q.standing = None;
    q.state = resting();
    assert!(q.feel_stomp(700.0, true));
    assert!(matches!(q.state, State::Idle { .. }) && q.wobbling());
}
