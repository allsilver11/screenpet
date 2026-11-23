//! 🎮 조작 모드 특기 (Z — brain_feat.rs): 🛡️ 날기 · 🕷️ 웹스윙 · 🧹 빗자루 · 🦇 오토바이. brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::control_moves::ControlInput;
use super::feat_moves::{Feat, FEAT_BOOST, FEAT_FLY_SPEED};
use super::swing_tests::resting;
use super::tests::{place, win, world};
use super::web_moves::WebPhase;
use super::*;
use crate::test_util::{bundled, DT};

/// 번들 pet.json 그대로의 설정 (청소년)
fn pet(id: &str) -> PetConfig {
    bundled().into_iter().find(|(i, _)| i == id).unwrap_or_else(|| panic!("ui/pets/{id}")).1
}

/// 키 묶음
fn keys(f: impl FnOnce(&mut ControlInput)) -> ControlInput {
    let mut k = ControlInput::default();
    f(&mut k);
    k
}

const NONE: ControlInput = ControlInput { left: false, right: false, up: false, down: false, jump: false, run: false, special: false };
const Z: ControlInput = ControlInput { special: true, ..NONE };

/// 바닥(x)에 쉬며 조작 모드가 켜진 펫 (연령대 몸짓·저절로 놀기는 꺼 둠)
fn controlled(id: &str, x: f64, w: &World) -> Pet {
    let mut p = resting(pet(id), x, 7, w);
    p.hush_stage();
    p.bike.signal_ready_at = f64::INFINITY;
    p.set_control(true);
    p
}

/// 내 동물: 키를 누른 채 secs초 (게임 루프처럼 매 틱 키를 먼저 알려 줌)
fn hold(p: &mut Pet, w: &World, k: ControlInput, secs: f64) {
    let n = ((secs / DT).round() as usize).max(1);
    for _ in 0..n {
        p.control_keys(k, DT);
        p.update(DT, w);
        assert!(p.body.x.is_finite() && p.body.y.is_finite(), "NaN body: {:?}", p.body);
    }
}

/// Z를 한 번 눌렀다 뗌
fn tap(p: &mut Pet, w: &World, k: ControlInput) {
    hold(p, w, k, DT);
    hold(p, w, NONE, DT);
}

/// 내려설 때까지 키 없이 (최대 secs초) — 내려섰으면 true
fn settle(p: &mut Pet, w: &World, secs: f64) -> bool {
    for _ in 0..(secs / DT) as usize {
        hold(p, w, NONE, DT);
        if p.standing.is_some() && !matches!(p.state, State::Air { .. }) {
            return true;
        }
    }
    false
}

#[test]
fn z_is_bit_64_on_the_friend_wire() {
    assert_eq!(Z.bits(), 64);
    assert_eq!(ControlInput::from_bits(64), Z);
    assert!(Z.any());
}

#[test]
fn each_hero_gets_its_own_special() {
    for (id, feat) in [
        ("musoe", Some(Feat::Fly)),
        ("super", Some(Feat::Fly)),
        ("spider", Some(Feat::Swing)),
        ("harry", Some(Feat::Broom)),
        ("hermione", Some(Feat::Broom)),
        ("voldemort", Some(Feat::Broom)),
        ("bat", Some(Feat::Bike)),
        ("hagrid", Some(Feat::Bike)),
        ("cat", None),
        ("dementor", None),
        ("heumul", None),
    ] {
        let w = world(vec![]);
        let p = Pet::new(pet(id), 500.0, 0.0, 1);
        assert_eq!(p.feat_kind(), feat, "{id}");
        let _ = w;
    }
}

#[test]
fn z_does_nothing_for_a_pet_without_a_special() {
    let w = world(vec![]);
    let mut p = controlled("cat", 700.0, &w);
    hold(&mut p, &w, Z, 1.0);
    assert!(p.standing.is_some());
    assert_eq!(p.body.x, 700.0);
    assert_eq!(p.feat_on(), None);
}

#[test]
fn musoe_z_takes_off_and_the_arrows_fly_four_ways_with_shift_faster() {
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    let y0 = p.body.y;
    tap(&mut p, &w, Z);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
    assert!(p.standing.is_none(), "took off");
    assert_eq!(p.render().anim, Anim::Hover);
    // 키가 없으면 그 자리에 떠 있음 (떨어지지 않음)
    let (x, y) = (p.body.x, p.body.y);
    hold(&mut p, &w, NONE, 2.0);
    assert_eq!((p.body.x, p.body.y), (x, y));
    assert!(y < y0, "rose a little on take-off");
    let speed = p.cfg.walk_speed * FEAT_FLY_SPEED * p.soft_power();
    // ↑
    hold(&mut p, &w, keys(|k| k.up = true), 1.0);
    assert!((y - p.body.y - speed).abs() < 3.0, "flew up one second: {}", y - p.body.y);
    assert_eq!(p.render().anim, Anim::Fly);
    // →
    let (x, y) = (p.body.x, p.body.y);
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!((p.body.x - x - speed).abs() < 3.0);
    assert_eq!(p.body.y, y);
    assert_eq!(p.facing, 1.0);
    // ← + Shift = 빠르게
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| { k.left = true; k.run = true; }), 0.5);
    assert!((x - p.body.x - speed * FEAT_BOOST * 0.5).abs() < 3.0, "boosted: {}", x - p.body.x);
    assert_eq!(p.facing, -1.0);
    assert_eq!(p.render().anim, Anim::FlyFast);
    assert!(p.render().flip);
    // ↓ (허공 — 바닥까지 멀면 그냥 내려감)
    let y = p.body.y;
    hold(&mut p, &w, keys(|k| k.down = true), 0.3);
    assert!(p.body.y > y);
    assert!(p.standing.is_none());
}

#[test]
fn flight_never_leaves_the_screen() {
    let w = world(vec![]);
    let mut p = controlled("super", 900.0, &w);
    tap(&mut p, &w, Z);
    hold(&mut p, &w, keys(|k| { k.up = true; k.right = true; k.run = true; }), 15.0);
    let b = w.bounds;
    assert!(p.body.x + p.cfg.half_w <= b.right + 0.5 && p.body.y - p.cfg.size >= b.top - 0.5, "{:?}", p.body);
    hold(&mut p, &w, keys(|k| { k.left = true; k.run = true; }), 30.0);
    assert!(p.body.x - p.cfg.half_w >= b.left - 0.5);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
}

#[test]
fn musoe_z_again_glides_down_softly_and_lands() {
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    tap(&mut p, &w, Z);
    hold(&mut p, &w, keys(|k| k.up = true), 1.5);
    tap(&mut p, &w, Z);
    assert_eq!(p.feat_on(), None);
    assert!(p.soar.gliding, "glides instead of dropping like a stone");
    assert!(settle(&mut p, &w, 20.0), "landed");
    assert_eq!(p.thud, 0.0, "soft landing, no thud");
    assert!(p.is_controlled());
    // 내려선 뒤 다시 걸음
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!(p.body.x > x + 30.0);
    assert_eq!(p.render().anim, Anim::Walk);
}

#[test]
fn holding_down_onto_a_window_top_lands_on_it() {
    let w = world(vec![win(1, 600.0, 600.0, 1300.0, 1000.0)]);
    let mut p = controlled("musoe", 900.0, &w);
    tap(&mut p, &w, Z);
    // 창 윗면 위로 올라감 (창 안을 지나 날아도 걸리지 않음)
    hold(&mut p, &w, keys(|k| k.up = true), 3.0);
    assert!(p.body.y < 560.0, "{}", p.body.y);
    assert!(p.standing.is_none());
    hold(&mut p, &w, keys(|k| k.down = true), 3.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Window(1)), "landed on the window top: {:?}", p.state);
    assert_eq!(p.body.y, 600.0);
    assert_eq!(p.feat_on(), None);
}

#[test]
fn down_at_the_floor_lands_on_it() {
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    tap(&mut p, &w, Z);
    hold(&mut p, &w, keys(|k| k.down = true), 2.0);
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert_eq!(p.feat_on(), None);
}

#[test]
fn turning_control_off_mid_flight_glides_down_instead_of_hanging_in_the_air() {
    let w = world(vec![]);
    let mut p = controlled("musoe", 900.0, &w);
    tap(&mut p, &w, Z);
    hold(&mut p, &w, keys(|k| k.up = true), 1.5);
    p.set_control(false);
    let mut landed = false;
    for _ in 0..(20.0 / DT) as usize {
        p.update(DT, &w);
        if p.standing.is_some() {
            landed = true;
            break;
        }
    }
    assert!(landed, "came down: {:?}", p.state);
    assert_eq!(p.thud, 0.0);
}

#[test]
fn a_broom_rider_mounts_flies_and_dismounts_on_z() {
    for id in ["harry", "hermione", "voldemort"] {
        let w = world(vec![]);
        let mut p = controlled(id, 900.0, &w);
        tap(&mut p, &w, Z);
        assert_eq!(p.feat_on(), Some(Feat::Broom), "{id}");
        assert_eq!(p.render().anim, Anim::Broom, "{id}");
        let (x, y) = (p.body.x, p.body.y);
        hold(&mut p, &w, NONE, 1.0);
        assert_eq!((p.body.x, p.body.y), (x, y), "{id}: hovers on the broom");
        let speed = p.cfg.walk_speed * crate::broom_fly::BROOM_SPEED * p.soft_power();
        hold(&mut p, &w, keys(|k| { k.up = true; k.left = true; }), 1.0);
        let d = (x - p.body.x).hypot(y - p.body.y);
        assert!((d - speed).abs() < 3.0, "{id}: diagonal at broom speed {d} vs {speed}");
        assert!(p.body.x < x && p.body.y < y);
        assert_eq!(p.render().anim, Anim::Broom);
        tap(&mut p, &w, Z);
        assert_eq!(p.feat_on(), None);
        assert_eq!(p.render().anim, Anim::Broom, "{id}: comes down still on the broom");
        assert!(settle(&mut p, &w, 20.0), "{id}: landed");
        assert_eq!(p.thud, 0.0);
    }
}

#[test]
fn the_bat_rides_its_bike_fast_with_a_shift_boost_and_gets_off_on_z() {
    let w = world(vec![]);
    let mut p = controlled("bat", 500.0, &w);
    tap(&mut p, &w, Z);
    assert_eq!(p.feat_on(), Some(Feat::Bike));
    assert!(p.standing.is_some());
    assert_eq!(p.render().anim, Anim::Drift, "waits on the bike");
    let speed = p.cfg.walk_speed * crate::bike::BIKE_SPEED * p.soft_power();
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!((p.body.x - x - speed).abs() < 4.0, "rode one second: {} vs {speed}", p.body.x - x);
    assert_eq!(p.render().anim, Anim::Ride);
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| { k.right = true; k.run = true; }), 0.5);
    assert!((p.body.x - x - speed * FEAT_BOOST * 0.5).abs() < 4.0, "boost");
    tap(&mut p, &w, Z);
    assert_eq!(p.feat_on(), None);
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| k.left = true), 1.0);
    let walk = p.cfg.walk_speed * Move::Walk.speed_mult();
    assert!((x - p.body.x - walk).abs() < 4.0, "walks again: {} vs {walk}", x - p.body.x);
    assert_eq!(p.render().anim, Anim::Walk);
}

#[test]
fn the_bike_drives_off_a_window_edge_and_lands() {
    let w = world(vec![win(1, 300.0, 600.0, 900.0, 1000.0)]);
    let mut p = controlled("bat", 800.0, &w);
    place(&mut p, &w, Owner::Window(1), 800.0, State::Idle { remaining: 1.0, look: false });
    tap(&mut p, &w, Z);
    let mut flew = false;
    for _ in 0..(3.0 / DT) as usize {
        hold(&mut p, &w, keys(|k| k.right = true), DT);
        if matches!(p.state, State::Air { .. }) {
            flew = true;
            assert_eq!(p.render().anim, Anim::BikeJump);
        }
    }
    assert!(flew, "drove off the edge");
    assert_eq!(p.standing.map(|s| s.owner), Some(Owner::Ground(0)));
    assert_eq!(p.feat_on(), Some(Feat::Bike), "still on the bike");
}

#[test]
fn hagrid_rides_his_sidecar_bike() {
    let w = world(vec![]);
    let mut p = controlled("hagrid", 500.0, &w);
    tap(&mut p, &w, Z);
    assert_eq!(p.feat_on(), Some(Feat::Bike));
    assert_eq!(p.render().anim, Anim::Sidecar);
    let x = p.body.x;
    hold(&mut p, &w, keys(|k| k.right = true), 1.0);
    assert!(p.body.x - x > p.cfg.walk_speed * 2.0);
    assert_eq!(p.render().anim, Anim::Sidecar);
    hold(&mut p, &w, keys(|k| k.jump = true), DT);
    assert!(matches!(p.state, State::Air { .. }));
    assert_eq!(p.render().anim, Anim::SidecarFly);
}

/// 거미인간이 줄에 매달려 흔들리는 중
fn swinging(p: &Pet) -> bool {
    matches!(p.state, State::Web { phase: WebPhase::Zip { .. } | WebPhase::Swing { .. }, .. })
}

#[test]
fn the_spider_shoots_a_web_on_z_swings_and_lets_go_on_space() {
    let w = world(vec![]);
    let mut p = controlled("spider", 600.0, &w);
    p.facing = 1.0;
    tap(&mut p, &w, Z);
    assert!(swinging(&p), "{:?}", p.state);
    assert_eq!(p.feat_on(), Some(Feat::Swing));
    assert!(p.web_line().is_some(), "the line shows");
    // 키가 없어도 계속 흔들림 (저절로 놓지 않음)
    let mut xs = Vec::new();
    for _ in 0..(8.0 / DT) as usize {
        hold(&mut p, &w, NONE, DT);
        assert!(swinging(&p), "keeps swinging until let go: {:?}", p.state);
        assert!(p.standing.is_none());
        xs.push(p.body.x);
    }
    let (lo, hi) = xs.iter().fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
    assert!(hi - lo > 100.0, "swung to and fro: {lo}..{hi}");
    // Space = 놓고 날아감
    hold(&mut p, &w, keys(|k| k.jump = true), DT);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    assert!(p.web_line().is_none());
    assert_eq!(p.feat_on(), None);
    assert!(settle(&mut p, &w, 10.0), "landed");
    assert!(p.is_controlled());
}

#[test]
fn pumping_the_arrows_swings_higher_but_never_off_screen() {
    let w = world(vec![]);
    let mut p = controlled("spider", 960.0, &w);
    p.facing = 1.0;
    tap(&mut p, &w, Z);
    hold(&mut p, &w, NONE, 1.0);
    let span = |p: &mut Pet, k: ControlInput| {
        let mut ys = (f64::MAX, f64::MIN);
        for _ in 0..(6.0 / DT) as usize {
            // 가는 쪽으로 펌프 (진행 방향 키)
            let k = if k.left || k.right { if p.body.vx >= 0.0 { keys(|k| k.right = true) } else { keys(|k| k.left = true) } } else { k };
            hold(p, &w, k, DT);
            ys = (ys.0.min(p.body.y), ys.1.max(p.body.y));
            let b = w.bounds;
            assert!(p.body.x >= b.left && p.body.x <= b.right && p.body.y - p.cfg.size >= b.top - 1.0 && p.body.y <= b.bottom, "{:?}", p.body);
        }
        ys.1 - ys.0
    };
    let calm = span(&mut p, NONE);
    let pumped = span(&mut p, keys(|k| k.right = true));
    assert!(pumped > calm + 20.0, "pumping swings higher: {calm} → {pumped}");
    assert!(swinging(&p));
}

#[test]
fn z_mid_air_shoots_a_new_line_and_keeps_swinging() {
    let w = world(vec![]);
    let mut p = controlled("spider", 400.0, &w);
    p.facing = 1.0;
    tap(&mut p, &w, Z);
    // 앞으로 가장 빠를 때 (맨 아래 언저리) 놓음
    let mut guard = 0;
    while !(p.body.vx > 0.0 && matches!(p.state, State::Web { phase: WebPhase::Swing { theta, .. }, .. } if theta > 0.2)) {
        hold(&mut p, &w, NONE, DT);
        guard += 1;
        assert!(guard < 2000);
    }
    hold(&mut p, &w, keys(|k| k.jump = true), DT);
    assert!(matches!(p.state, State::Air { .. }));
    hold(&mut p, &w, NONE, 0.1);
    tap(&mut p, &w, Z);
    assert!(swinging(&p), "re-shot mid-air: {:?} {:?}", p.state, p.body);
    assert_eq!(p.feat_on(), Some(Feat::Swing));
}

#[test]
fn turning_control_off_mid_swing_lets_go() {
    let w = world(vec![]);
    let mut p = controlled("spider", 600.0, &w);
    p.facing = 1.0;
    tap(&mut p, &w, Z);
    hold(&mut p, &w, NONE, 1.0);
    p.set_control(false);
    assert!(matches!(p.state, State::Air { .. }), "{:?}", p.state);
    let mut landed = false;
    for _ in 0..(10.0 / DT) as usize {
        p.update(DT, &w);
        if p.standing.is_some() {
            landed = true;
            break;
        }
    }
    assert!(landed);
}

#[test]
fn my_pet_on_a_friends_screen_uses_its_special_from_forwarded_keys() {
    let w = world(vec![]);
    let mut p = resting(pet("musoe"), 900.0, 7, &w);
    p.hush_stage();
    p.remote_keys(true, ControlInput::from_bits(Z.bits()));
    p.update(DT, &w);
    p.remote_keys(true, NONE);
    p.update(DT, &w);
    assert_eq!(p.feat_on(), Some(Feat::Fly));
    assert!(p.standing.is_none());
    let y = p.body.y;
    for _ in 0..30 {
        p.remote_keys(true, keys(|k| k.up = true));
        p.update(DT, &w);
    }
    assert!(p.body.y < y - 20.0, "flew up on forwarded keys");
    // 주인이 조작을 끝내면 사뿐 내려옴
    p.remote_keys(false, NONE);
    assert!(p.soar.gliding);
}

