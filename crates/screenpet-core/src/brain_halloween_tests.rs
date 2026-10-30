//! 할로윈 동물 — 유령(날지만 쏘지 않는다)과 호박(늘 깡총 튀어 다닌다) brain 동작 테스트.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{sim_cfg, win, world};
use super::*;
use crate::flight;
use crate::hop;

const DT: f64 = 1.0 / 60.0;

// ---- 유령: locomotion fly, walk_speed 40, sting 없음 ----

fn ghost() -> PetConfig {
    // stage_size 0.85
    let size = 128.0 * 0.85;
    PetConfig { size, half_w: 0.3 * size, walk_speed: 40.0, flies: true, stings: false, ..sim_cfg() }
}

/// 유령 10분 × 4 — 늘 화면 안, 대개 둥둥 떠다니고, 잠은 내려앉았을 때만, 쏘기는 없다
#[test]
fn ghost_ten_minutes_floats_within_bounds_never_stings_and_sleeps_only_perched() {
    let w = world(vec![win(1, 300.0, 600.0, 1100.0, 900.0), win(2, 1300.0, 350.0, 1800.0, 700.0)]);
    let (mut flying, mut sleeping, mut total) = (0usize, 0usize, 0usize);
    for seed in 0..4 {
        let mut p = Pet::new(ghost(), 960.0, 128.0, seed);
        let b = flight::fly_bounds(&w.bounds, p.cfg.half_w, p.cfg.size * (1.0 + flight::TOP_SPACE));
        for tick in 0..(60 * 600) {
            p.update(DT, &w);
            let (x, y) = (p.body.x, p.body.y);
            assert!(x >= b.left - 1e-9 && x <= b.right + 1e-9, "seed {seed}: x {x} {:?}", p.state);
            assert!(y >= b.top - 1e-9 && y <= b.bottom + 1e-9, "seed {seed}: y {y} {:?}", p.state);
            assert!(!matches!(p.state, State::Sting { .. }), "seed {seed}: a ghost stung");
            assert!(p.take_sting().is_none());
            let anim = p.render().anim;
            match anim {
                Anim::Walk | Anim::Run | Anim::Roll => flying += 1,
                Anim::Sleep => {
                    assert!(p.standing.is_some(), "seed {seed}: sleeping in the air");
                    sleeping += 1;
                }
                Anim::Sting | Anim::Climb | Anim::Slide | Anim::Hop => panic!("seed {seed}: {anim:?}"),
                Anim::Fall => assert!(tick < 5, "seed {seed}: fell at {tick}"),
                _ => {}
            }
            total += 1;
        }
    }
    let f = flying as f64 / total as f64;
    assert!(f > 0.2, "flying {f:.3}");
    assert!(sleeping > 0, "never slept");
}

/// 간질이고·흔들고·계속 눌러도 유령은 쏘지 않는다 (sting 동작이 없다)
#[test]
fn pestered_ghost_never_stings() {
    let w = world(vec![]);
    let mut p = Pet::new(ghost(), 800.0, 300.0, 1);
    p.state = State::Idle { remaining: 100.0, look: false };
    for i in 0..(60 * 20) {
        let (x, y) = (p.body.x, p.body.y - 50.0);
        p.hover(x + if i % 2 == 0 { 0.0 } else { 40.0 }, y, true);
        if i % 30 == 0 {
            p.pointer_down(x, y);
        }
        p.update(DT, &w);
        if i % 30 == 1 {
            p.pointer_up(x, y);
        }
        assert!(!matches!(p.state, State::Sting { .. }), "tick {i}: {:?}", p.state);
        assert!(p.take_sting().is_none());
    }
}

/// 유령은 꿀벌보다 느리게 떠다닌다 — 날기 속도는 walk_speed 그대로
#[test]
fn ghost_floats_at_its_slow_walk_speed() {
    let w = world(vec![]);
    let mut p = Pet::new(ghost(), 300.0, 500.0, 1);
    p.state = State::Idle { remaining: 100.0, look: false };
    p.start_flight(flight::Goal::Point(1800.0, 500.0), Move::Walk);
    for _ in 0..60 {
        p.update(DT, &w);
    }
    let s = p.flight.unwrap().s;
    assert!((s - 40.0).abs() < 1.5, "{s}");
}

// ---- 호박: 걷는 동물, hop_share 1.0 — 걷기·달리기는 늘 통통 ----

fn pumpkin() -> PetConfig {
    // stage_size 0.9
    let size = 128.0 * 0.9;
    PetConfig { size, half_w: 0.3 * size, walk_speed: 70.0, hop_share: 1.0, personality: Personality::Playful, ..sim_cfg() }
}

/// 호박 10분 × 6: 걷기·달리기 시간 내내 통통(첫 틱 빼고), 튀기는 낮고 늘 디딤판 위, 큰 점프·벽 타기 없음
#[test]
fn pumpkin_bounces_through_all_of_its_walking_and_never_jumps_big_or_climbs() {
    let w = world(vec![win(1, 300.0, 750.0, 900.0, 1000.0), win(2, 1200.0, 500.0, 1700.0, 800.0)]);
    let size = pumpkin().size;
    let (mut walking_ticks, mut hopping_ticks, mut hop_frames) = (0usize, 0usize, 0usize);
    for seed in 0..6 {
        let mut p = Pet::new(pumpkin(), 960.0, 128.0, seed);
        for _ in 0..(60 * 600) {
            p.update(DT, &w);
            assert!(!matches!(p.state, State::Leap { .. }), "seed {seed}: no big jumps");
            let lift = p.body.y - size - p.window_pos().1;
            assert!((0.0..=hop::BUNNY_RUN_HOP_HEIGHT * size + 1e-9).contains(&lift), "seed {seed}: lift {lift}");
            let anim = p.render().anim;
            assert!(!matches!(anim, Anim::Climb | Anim::Slide | Anim::Sting | Anim::BigJump), "seed {seed}: {anim:?}");
            if let State::Move { kind: Move::Walk | Move::Run, .. } = p.state {
                let st = p.standing.expect("moving pumpkin stands on something");
                assert_eq!(w.anchor(st.owner).unwrap().1, p.body.y, "seed {seed}: feet on the surface");
                walking_ticks += 1;
                hopping_ticks += p.bunny_hopping() as usize;
                hop_frames += (anim == Anim::Hop) as usize;
            }
        }
    }
    let share = hopping_ticks as f64 / walking_ticks as f64;
    assert!(walking_ticks > 60 * 600, "walked {walking_ticks}");
    assert!(share > 0.99, "hop share {share:.4}");
    assert!(hop_frames > walking_ticks / 4, "hop frames {hop_frames} of {walking_ticks}");
}
