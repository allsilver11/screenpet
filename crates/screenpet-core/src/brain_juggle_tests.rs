//! 🤹 흐물이 (pet.json juggler) — 바닥의 물건·똥을 주워 머리 위로 들고 던지기 (반쯤 맞히기), 들고 돌아다니기, 친구에게 배달,
//! 거대한 것에 끙차 (들거나 털썩). brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::juggle_moves::{JugglePhase, Plan};
use super::tests::{place, world};
use super::*;
use crate::juggle::{self, JuggleEvent, ObjectSpot, Target};
use crate::object::{Object, ObjectKind};
use crate::test_util::{pet_cfg, DT};

fn juggler() -> PetConfig {
    PetConfig { half_w: 38.4, walk_speed: 110.0, personality: Personality::Playful, timid: true, energetic: true, rigging: true, juggler: true, ..pet_cfg() }
}

fn ground_y(w: &World) -> f64 {
    w.anchor(Owner::Ground(0)).unwrap().1
}

fn spot(id: u64, x: f64, w: &World) -> ObjectSpot {
    ObjectSpot { id, x, y: ground_y(w), owner: Owner::Ground(0), half_w: 14.4, giant: false, local: false }
}

fn giant(id: u64, x: f64, w: &World) -> ObjectSpot {
    ObjectSpot { half_w: 236.0, giant: true, ..spot(id, x, w) }
}

fn walking(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Move { kind: Move::Walk, remaining: 100.0 });
    p.facing = 1.0;
    p
}

fn juggling(p: &Pet) -> Option<JugglePhase> {
    match p.state {
        State::Juggle { phase, .. } => Some(phase),
        _ => None,
    }
}

/// 앱처럼 한 틱: 물건을 알아채고, 움직이고, 놓은 것을 꺼낸다
fn tick(p: &mut Pet, w: &World, spots: &[ObjectSpot], targets: &[Target]) -> Option<JuggleEvent> {
    p.notice_objects(spots, targets, w);
    p.update(DT, w);
    p.take_juggle()
}

/// 든 채로 이 단계에 둔다 (plan은 Hold가 끝난 뒤 할 일)
fn holding(p: &mut Pet, plan: Plan, w: &World) {
    let x = p.body.x;
    place(p, w, Owner::Ground(0), x, State::Juggle { phase: JugglePhase::Hold { secs: 0.1 }, t: 0.0 });
    p.juggle.carry = Some(7);
    p.juggle.plan = plan;
}

#[test]
fn juggle_animations_are_optional_side_views_except_the_front_present() {
    for (a, name) in [(Anim::Lift, "lift"), (Anim::Hold, "hold"), (Anim::Carry, "carry"), (Anim::Throw, "throw"), (Anim::Strain, "strain"), (Anim::Present, "present"), (Anim::Slump, "slump")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::OPTIONAL_ANIMS.contains(&name) && crate::manifest::JUGGLE_ANIMS.contains(&name));
        assert!(!a.is_back(), "{name}");
        assert_eq!(a.is_front(), a == Anim::Present, "{name}");
    }
}

#[test]
fn other_species_never_pick_up_and_their_dice_are_untouched() {
    let w = world(vec![]);
    let spots = [spot(1, 900.0, &w), spot(2, 700.0, &w)];
    let mut a = walking(pet_cfg(), 800.0, 9, &w);
    let mut b = walking(pet_cfg(), 800.0, 9, &w);
    for _ in 0..(60 * 120) {
        a.notice_objects(&spots, &[], &w);
        a.update(DT, &w);
        b.update(DT, &w);
        assert_eq!(a.state, b.state);
        assert_eq!(a.body.x, b.body.x);
        assert!(a.carried_object().is_none() && a.take_juggle().is_none());
    }
}

#[test]
fn a_juggler_goes_for_a_nearby_object_on_its_surface_fairly_often() {
    let w = world(vec![]);
    let mut went = 0;
    for seed in 0..600 {
        let mut p = walking(juggler(), 800.0, seed, &w);
        p.notice_objects(&[spot(3, 1000.0, &w)], &[], &w);
        if matches!(juggling(&p), Some(JugglePhase::Approach { id: 3, .. })) {
            went += 1;
        } else {
            // 같은 물건엔 다시 주사위를 굴리지 않는다
            p.notice_objects(&[spot(3, 1000.0, &w)], &[], &w);
            assert!(juggling(&p).is_none());
        }
    }
    let share = went as f64 / 600.0;
    assert!((share - juggle::PICK_CHANCE).abs() < 0.07, "{share}");
}

#[test]
fn far_objects_objects_on_other_surfaces_and_busy_jugglers_are_ignored() {
    let w = world(vec![super::tests::w1()]);
    for seed in 0..100 {
        let mut p = walking(juggler(), 200.0, seed, &w);
        let far = spot(1, 200.0 + 5.0 * 128.0, &w);
        let on_window = ObjectSpot { owner: Owner::Window(1), y: 400.0, ..spot(2, 520.0, &w) };
        p.notice_objects(&[far, on_window], &[], &w);
        assert!(juggling(&p).is_none());
        // 자는 중·잡힌 중엔 줍지 않는다
        p.state = State::Sleep { remaining: 10.0 };
        p.notice_objects(&[spot(3, 300.0, &w)], &[], &w);
        assert!(juggling(&p).is_none());
    }
}

#[test]
fn it_walks_over_crouches_lifts_the_object_above_its_head_and_throws_it() {
    let w = world(vec![]);
    let mut done = 0;
    for seed in 0..40 {
        let mut p = walking(juggler(), 800.0, seed, &w);
        p.juggle.rng = fastrand::Rng::with_seed(seed); // 가끔은 안 줍는다 — 주울 때까지 다른 물건으로
        let mut id = 10;
        while juggling(&p).is_none() && id < 60 {
            p.notice_objects(&[spot(id, 1000.0, &w)], &[], &w);
            id += 1;
        }
        let Some(JugglePhase::Approach { id, .. }) = juggling(&p) else { continue };
        let s = spot(id, 1000.0, &w);
        let mut lifted = false;
        let mut released = None;
        for _ in 0..(60 * 20) {
            // 들기 시작하면 바닥에서 없어진다 (앱의 물건이 든 상태가 된다)
            let spots: &[ObjectSpot] = if lifted { &[] } else { std::slice::from_ref(&s) };
            if let Some(JuggleEvent::Release { id: got, vx, vy }) = tick(&mut p, &w, spots, &[]) {
                assert_eq!(got, id);
                released = Some((vx, vy));
                break;
            }
            if let Some(carried) = p.carried_object() {
                assert_eq!(carried, id);
                if !lifted {
                    lifted = true;
                    assert!((p.body.x - s.x).abs() <= 0.3 * p.cfg.size, "물건 바로 옆에서 든다: {}", p.body.x);
                    assert_eq!(p.render().anim, Anim::Lift);
                }
                let (hx, hy) = p.carry_point().unwrap();
                if matches!(juggling(&p), Some(JugglePhase::Hold { .. } | JugglePhase::Wander { .. })) {
                    assert!(hy < p.body.y - 0.8 * p.cfg.size, "머리 위: {hy} vs feet {}", p.body.y);
                    assert!((hx - p.body.x).abs() < 0.3 * p.cfg.size);
                }
            }
        }
        let (_, vy) = released.expect("throws within 20 s");
        assert!(vy < 0.0, "위로 던진다");
        assert!(p.carried_object().is_none());
        for _ in 0..60 {
            tick(&mut p, &w, &[], &[]);
        }
        assert!(juggling(&p).is_none(), "던지고 나면 평소로: {:?}", p.state);
        done += 1;
    }
    assert!(done >= 30, "{done}");
}

#[test]
fn with_no_one_around_it_throws_up_in_either_direction() {
    let w = world(vec![]);
    let mut dirs = [0; 2];
    for seed in 0..200 {
        let mut p = walking(juggler(), 900.0, seed, &w);
        holding(&mut p, Plan::Throw, &w);
        let ev = (0..120).find_map(|_| tick(&mut p, &w, &[], &[]));
        let Some(JuggleEvent::Release { vx, vy, .. }) = ev else { panic!("no release") };
        assert!(vy <= -juggle::RANDOM_THROW_VY.0 + 1e-9 && vx.abs() >= juggle::RANDOM_THROW_VX.0 - 1e-9);
        dirs[usize::from(vx > 0.0)] += 1;
    }
    assert!(dirs[0] > 60 && dirs[1] > 60, "{dirs:?}");
}

/// 던진 물건(쓰레기 한 개)이 노린 동물의 몸 상자를 지나가는가 — 진짜 물건 물리로
fn hits(from: (f64, f64), vx: f64, vy: f64, t: Target, w: &World) -> bool {
    let mut o = Object::new(ObjectKind::Can, from.0, from.1, 1.0, 1);
    o.carry_at(from.0, from.1);
    o.launch(vx, vy);
    let half = 0.3 * t.size;
    for _ in 0..(60 * 3) {
        o.tick(DT, w, Default::default(), false);
        let (hw, h) = o.hit_box();
        let b = o.body;
        if b.x + hw > t.x - half && b.x - hw < t.x + half && b.y > t.y - 0.9 * t.size && b.y - h < t.y {
            return true;
        }
        if o.owner().is_some() {
            return false;
        }
    }
    false
}

#[test]
fn it_aims_at_the_nearest_pet_and_hits_it_about_half_the_time() {
    let w = world(vec![]);
    let gy = ground_y(&w);
    for (tx, want) in [(1250.0, 1.0), (550.0, -1.0)] {
        let target = Target { x: tx, y: gy, size: 116.0 };
        let far = Target { x: 1800.0, y: gy, size: 116.0 };
        let mut hit = 0;
        let n = 300;
        for seed in 0..n {
            let mut p = walking(juggler(), 900.0, seed, &w);
            p.facing = -want;
            holding(&mut p, Plan::Throw, &w);
            let mut from = None;
            let ev = (0..120).find_map(|_| {
                from = p.carry_point().or(from);
                tick(&mut p, &w, &[], &[far, target])
            });
            let Some(JuggleEvent::Release { vx, vy, .. }) = ev else { panic!("no release") };
            assert_eq!(vx.signum(), want, "노린 쪽으로");
            assert_eq!(p.facing, want, "노린 쪽을 본다");
            if hits(from.unwrap(), vx, vy, target, &w) {
                hit += 1;
            }
        }
        let share = hit as f64 / n as f64;
        assert!((0.35..=0.65).contains(&share), "hit share {share} toward {tx}");
    }
}

#[test]
fn prank_odds_need_a_new_friend_edge_it_can_walk_to_and_never_happen_for_giants() {
    let w = world(vec![]);
    let n = 2000;
    let (mut prank, mut wander, mut throw) = (0, 0, 0);
    for seed in 0..n {
        let mut p = walking(juggler(), 900.0, seed, &w);
        p.set_prank_edges(false, true);
        match p.pick_plan(false, &w) {
            Plan::Prank(side) => {
                assert_eq!(side, Side::Right);
                prank += 1;
            }
            Plan::Wander => wander += 1,
            Plan::Throw => throw += 1,
            Plan::Stuff => panic!("도로 넣기는 📌 장난감만"),
        }
        assert_eq!(p.pick_plan(true, &w), Plan::Throw, "거대한 것은 던지기만");
        p.set_prank_edges(false, false);
        assert!(!matches!(p.pick_plan(false, &w), Plan::Prank(_)));
    }
    let share = prank as f64 / n as f64;
    assert!((share - juggle::PRANK_CHANCE).abs() < 0.04, "prank {share}");
    let wander_share = wander as f64 / (wander + throw) as f64;
    assert!((wander_share - juggle::WANDER_CHANCE).abs() < 0.05, "wander {wander_share}");
    // 창 윗면에서는 화면 끝까지 걸어갈 수 없다 — 장난을 고르지 않는다
    let ww = world(vec![super::tests::w1()]);
    for seed in 0..200 {
        let mut p = Pet::new(juggler(), 0.0, 0.0, seed);
        place(&mut p, &ww, Owner::Window(1), 800.0, State::Idle { remaining: 5.0, look: false });
        p.set_prank_edges(true, true);
        assert!(!matches!(p.pick_plan(false, &ww), Plan::Prank(_)));
    }
}

#[test]
fn sometimes_it_carries_the_object_around_for_a_while_before_throwing() {
    let w = world(vec![]);
    for seed in 0..20 {
        let mut p = walking(juggler(), 1700.0, seed, &w);
        holding(&mut p, Plan::Wander, &w);
        let mut secs = 0.0;
        let mut turned = false;
        let start_facing = p.facing;
        let ev = (0..(60 * 15)).find_map(|_| {
            let ev = tick(&mut p, &w, &[], &[]);
            if ev.is_none() {
                secs += DT;
                assert!(p.carried_object().is_some(), "{:?}", p.state);
                assert!(p.standing.is_some(), "뛰어내리지 않는다");
                assert!(p.body.x <= 1920.0 - p.cfg.half_w + 0.01);
                if matches!(juggling(&p), Some(JugglePhase::Wander { .. })) {
                    assert_eq!(p.render().anim, Anim::Carry);
                    turned |= p.facing != start_facing;
                }
            }
            ev
        });
        assert!(matches!(ev, Some(JuggleEvent::Release { .. })));
        assert!((4.0..=10.0 + 1.0).contains(&secs), "carried {secs:.1}s");
        assert!(turned, "화면 끝에서 돌아선다");
    }
}

#[test]
fn delivery_prank_carries_the_object_off_through_the_friend_edge() {
    let w = world(vec![]);
    let mut p = walking(juggler(), 1500.0, 4, &w);
    p.set_exit_edges(false, true);
    holding(&mut p, Plan::Prank(Side::Right), &w);
    let mut exited = None;
    for _ in 0..(60 * 15) {
        assert!(tick(&mut p, &w, &[], &[]).is_none(), "배달 가는 길엔 던지지 않는다");
        if let Some(side) = p.take_exit() {
            exited = Some(side);
            break;
        }
        assert_eq!(p.carried_object(), Some(7), "{:?}", p.state);
        if matches!(p.state, State::Exit { .. }) {
            assert_eq!(p.render().anim, Anim::Carry);
        }
    }
    assert_eq!(exited, Some(Side::Right));
    assert!(p.is_away());
    assert_eq!(p.carried_object(), Some(7), "앱이 Visit에 싣는다");
    p.carry_handed_over();
    assert_eq!(p.carried_object(), None);
}

#[test]
fn a_bounced_prank_comes_home_holding_it_and_drops_it_there() {
    let w = world(vec![]);
    let mut p = walking(juggler(), 1500.0, 4, &w);
    p.set_exit_edges(false, true);
    holding(&mut p, Plan::Prank(Side::Right), &w);
    while p.take_exit().is_none() {
        tick(&mut p, &w, &[], &[]);
    }
    // 그새 아무도 없다 — 바로 다시 들어온다 (물건은 아직 손에)
    p.enter(&w, Side::Right);
    let ev = (0..(60 * 10)).find_map(|_| tick(&mut p, &w, &[], &[]));
    let Some(JuggleEvent::Release { id, vx, vy }) = ev else { panic!("drops it at home") };
    assert_eq!(id, 7);
    assert!(vy < 0.0 && vx.abs() <= juggle::FLICK_VX.1 + 1e-9);
}

#[test]
fn arriving_with_a_delivery_it_faces_front_holding_it_up_then_flicks_it_down() {
    let w = world(vec![]);
    let mut p = Pet::new_entering(juggler(), &w, Side::Left, 3);
    p.hold_object(42);
    let mut presented = 0.0;
    let mut ev = None;
    for _ in 0..(60 * 10) {
        ev = tick(&mut p, &w, &[], &[]);
        if ev.is_some() {
            break;
        }
        assert_eq!(p.carried_object(), Some(42), "{:?}", p.state);
        if matches!(p.state, State::Enter { .. }) {
            assert_eq!(p.render().anim, Anim::Carry, "들고 걸어 들어온다");
        }
        if matches!(juggling(&p), Some(JugglePhase::Present { .. })) {
            presented += DT;
            let r = p.render();
            assert_eq!(r.anim, Anim::Present);
            assert!(!r.flip, "앞모습");
            let (_, hy) = p.carry_point().unwrap();
            assert!(hy < p.body.y - 0.8 * p.cfg.size);
        }
    }
    let Some(JuggleEvent::Release { id, vx, vy }) = ev else { panic!("flicks it") };
    assert_eq!(id, 42);
    assert!(vy < 0.0 && vx.abs() <= juggle::FLICK_VX.1 + 1e-9, "살짝 떨군다");
    assert!((juggle::PRESENT_RELEASE - 0.05..=juggle::PRESENT_RELEASE + 0.05).contains(&presented), "{presented}");
    assert!(p.carried_object().is_none());
    for _ in 0..120 {
        tick(&mut p, &w, &[], &[]);
    }
    assert!(juggling(&p).is_none(), "그다음은 평소 손님처럼");
}

#[test]
fn giants_take_a_strain_then_sixty_percent_get_lifted_and_thrown_short_the_rest_end_in_a_slump() {
    let w = world(vec![]);
    let n = 300;
    let (mut lifted, mut slumped) = (0, 0);
    for seed in 0..n {
        let mut p = walking(juggler(), 600.0, seed, &w);
        let g = giant(5, 1000.0, &w);
        p.state = State::Juggle { phase: JugglePhase::Approach { id: 5, x: 1000.0 - 236.0 - 0.28 * 128.0, obj_x: 1000.0, giant: true }, t: 0.0 };
        let mut strained = 0.0;
        let mut outcome = None;
        let mut slump_secs = 0.0;
        for _ in 0..(60 * 15) {
            let carrying = p.carried_object().is_some();
            let spots: &[ObjectSpot] = if carrying { &[] } else { std::slice::from_ref(&g) };
            let ev = tick(&mut p, &w, spots, &[]);
            match juggling(&p) {
                Some(JugglePhase::Strain { .. }) => {
                    strained += DT;
                    assert_eq!(p.render().anim, Anim::Strain);
                    assert!(p.carried_object().is_none(), "힘주는 동안엔 아직 바닥에");
                    assert_eq!(p.facing, 1.0, "물건 쪽을 본다");
                }
                Some(JugglePhase::Slump) => {
                    slump_secs += DT;
                    assert_eq!(p.render().anim, Anim::Slump);
                    assert!(p.carried_object().is_none(), "물건은 그 자리 그대로");
                    outcome.get_or_insert(false);
                }
                _ => {}
            }
            if let Some(JuggleEvent::Release { vx, vy, .. }) = ev {
                assert!(vx.abs() <= juggle::GIANT_THROW_VX.1 + 1e-9 && vy < 0.0 && -vy <= juggle::GIANT_THROW_VY.1 + 1e-9, "가까이 툭");
                outcome = Some(true);
                break;
            }
            if outcome == Some(false) && juggling(&p).is_none() {
                assert!(matches!(p.state, State::Move { kind: Move::Walk, .. }), "일어나 걸어간다: {:?}", p.state);
                assert_eq!(p.facing, -1.0, "물건 반대쪽으로");
                break;
            }
        }
        assert!((juggle::STRAIN_SECS - 0.05..=juggle::STRAIN_SECS + 0.05).contains(&strained), "{strained}");
        match outcome {
            Some(true) => lifted += 1,
            Some(false) => {
                assert!(slump_secs >= juggle::SLUMP_SECS - 0.05);
                slumped += 1;
            }
            None => panic!("seed {seed}: no outcome"),
        }
    }
    let share = lifted as f64 / n as f64;
    assert!((share - juggle::GIANT_LIFT_CHANCE).abs() < 0.07, "lifted {share} ({lifted}/{slumped})");
}

#[test]
fn grabbing_the_pet_lets_go_of_the_object_and_a_snatched_object_ends_the_juggle() {
    let w = world(vec![]);
    let mut p = walking(juggler(), 900.0, 1, &w);
    holding(&mut p, Plan::Wander, &w);
    tick(&mut p, &w, &[], &[]);
    assert_eq!(p.carried_object(), Some(7));
    p.pointer_down(p.body.x, p.body.y - 50.0);
    assert_eq!(p.carried_object(), None, "잡히면 놓친다");
    p.notice_objects(&[], &[], &w);
    p.pointer_up(p.body.x, p.body.y - 50.0);
    for _ in 0..30 {
        assert!(tick(&mut p, &w, &[], &[]).is_none());
        assert_eq!(p.carried_object(), None);
    }
    // 사용자가 물건을 낚아챘다
    let mut p = walking(juggler(), 900.0, 2, &w);
    holding(&mut p, Plan::Wander, &w);
    tick(&mut p, &w, &[], &[]);
    p.lose_carry();
    assert_eq!(p.carried_object(), None);
    assert!(juggling(&p).is_none());
    assert_eq!(p.take_speech(), Some(Speech::Hmph));
}

#[test]
fn an_object_that_goes_away_while_walking_over_calls_it_off() {
    let w = world(vec![]);
    let mut p = walking(juggler(), 600.0, 1, &w);
    p.state = State::Juggle { phase: JugglePhase::Approach { id: 3, x: 980.0, obj_x: 1000.0, giant: false }, t: 0.0 };
    tick(&mut p, &w, &[spot(3, 1000.0, &w)], &[]);
    assert!(juggling(&p).is_some());
    tick(&mut p, &w, &[], &[]);
    assert!(juggling(&p).is_none());
    assert_eq!(p.carried_object(), None);
}

#[test]
fn the_bundled_heumul_juggles_with_grip_tables_and_no_one_else_does() {
    for (id, cfg) in crate::test_util::bundled() {
        assert_eq!(cfg.juggler, id == "heumul", "{id}");
        if id == "heumul" {
            for g in [cfg.grip.lift, cfg.grip.hold, cfg.grip.carry, cfg.grip.throw, cfg.grip.present] {
                assert!(g.n >= 2, "{id}: {g:?}");
            }
            // 든 손은 머리 위 (윗변 쪽), 쪼그린 손은 아래
            assert!(cfg.grip.hold.at(0.0).1 < 0.3, "{:?}", cfg.grip.hold);
            assert!(cfg.grip.lift.at(0.0).1 > 0.6, "{:?}", cfg.grip.lift);
        }
    }
}
