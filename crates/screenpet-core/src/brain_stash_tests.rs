//! 🐿️ 다람쥐 볼 빵빵 (v0.6.3)

use super::stash_moves::StashPhase;
use super::tests::{place, world};
use super::*;
use crate::juggle::ObjectSpot;
use crate::play::{self, ObjectPlay};
use crate::test_util::{pet_cfg, DT};

const GROUND: f64 = 1040.0;

fn squirrel() -> PetConfig {
    PetConfig { stashes: true, half_w: 38.4, walk_speed: 70.0, personality: Personality::Playful, ..pet_cfg() }
}

fn resting(c: PetConfig, x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(c, x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 100.0, look: false });
    p.clock = 1000.0;
    p.play.stash_ready_at = 0.0;
    p
}

fn trash(id: u64, x: f64) -> ObjectSpot {
    ObjectSpot { id, x, y: GROUND, owner: Owner::Ground(0), half_w: 24.0, giant: false, local: false }
}

fn stashing(p: &Pet) -> bool {
    matches!(p.state, State::Stash { .. })
}

/// 앱처럼: 물건을 보여 주고, 움직이고, 볼에 넣은 것은 치움. 넣은 물건 id들과 들은 말
fn run(p: &mut Pet, w: &World, spots: &mut Vec<ObjectSpot>, secs: f64, mut each: impl FnMut(&Pet)) -> (Vec<u64>, Vec<Speech>) {
    let (mut gulped, mut said) = (Vec::new(), Vec::new());
    for _ in 0..(secs / DT) as usize {
        p.notice_play_objects(spots, w);
        p.update(DT, w);
        if let Some(ObjectPlay::Gulp { id }) = p.take_object_play() {
            gulped.push(id);
            spots.retain(|s| s.id != id);
        }
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        each(p);
    }
    (gulped, said)
}

#[test]
fn it_runs_over_stuffs_its_cheeks_grabs_a_neighbour_too_then_walks_off_and_buries_them() {
    let w = world(vec![]);
    let mut p = resting(squirrel(), 600.0, 1, &w);
    let mut spots = vec![trash(1, 800.0), trash(2, 900.0)];
    assert!(p.start_stash(&spots[0].clone(), &w));
    let (mut puffed_walk, mut puffed_sit, mut dug) = (false, false, false);
    let (gulped, said) = run(&mut p, &w, &mut spots, 20.0, |p| {
        let a = p.render().anim;
        puffed_walk |= a == Anim::CheeksWalk;
        puffed_sit |= a == Anim::CheeksIdle;
        dug |= a == Anim::Dig;
        if p.cheeks() > 0 && matches!(p.state, State::Stash { phase: StashPhase::Carry { .. }, .. }) {
            assert_eq!(a, Anim::CheeksWalk);
        }
    });
    assert_eq!(gulped, vec![1, 2], "가까운 것부터 둘 다");
    assert!(spots.is_empty());
    assert!(said.contains(&Speech::Nom));
    assert!(puffed_walk && dug, "볼 빵빵하게 걸어가 묻는다");
    let _ = puffed_sit;
    assert_eq!(p.cheeks(), 0, "묻고 나면 볼이 원래대로");
    assert!(!stashing(&p));
    assert!(p.play.stash_ready_at - p.clock >= play::STASH_CALM_SECS.0 - 25.0);
}

#[test]
fn at_most_three_per_trip() {
    let w = world(vec![]);
    let mut p = resting(squirrel(), 600.0, 2, &w);
    let mut spots: Vec<ObjectSpot> = (0..5).map(|k| trash(10 + k, 700.0 + 60.0 * k as f64)).collect();
    assert!(p.start_stash(&spots[0].clone(), &w));
    let (gulped, _) = run(&mut p, &w, &mut spots, 30.0, |p| assert!(p.cheeks() <= play::MAX_CHEEKS));
    assert_eq!(gulped.len(), play::MAX_CHEEKS as usize, "{gulped:?}");
}

#[test]
fn it_goes_for_a_thing_it_notices_now_and_then() {
    let w = world(vec![]);
    let n = 400;
    let mut went = 0;
    for seed in 0..n {
        let mut p = resting(squirrel(), 600.0, seed, &w);
        p.notice_play_objects(&[trash(1, 800.0)], &w);
        went += usize::from(stashing(&p));
    }
    let share = went as f64 / n as f64;
    assert!((share - play::STASH_CHANCE).abs() < 0.07, "{share}");
}

#[test]
fn never_guests_giants_toys_other_floors_or_while_away() {
    let w = world(vec![crate::brain::tests::w1()]);
    let tries = |p: &mut Pet, spot: ObjectSpot| -> bool {
        for k in 0..60 {
            p.notice_play_objects(&[ObjectSpot { id: 100 + k, ..spot }], &w);
            if stashing(p) {
                return true;
            }
        }
        false
    };
    assert!(tries(&mut resting(squirrel(), 300.0, 1, &w), trash(1, 400.0)));
    let mut guest = resting(squirrel(), 300.0, 1, &w);
    guest.set_guest(crate::visit::Side::Left, 1000.0);
    assert!(!tries(&mut guest, trash(1, 400.0)), "놀러 온 다람쥐는 남의 화면 물건을 가져가지 않는다");
    assert!(!tries(&mut resting(squirrel(), 300.0, 1, &w), ObjectSpot { giant: true, ..trash(1, 400.0) }));
    assert!(!tries(&mut resting(squirrel(), 300.0, 1, &w), ObjectSpot { local: true, ..trash(1, 400.0) }));
    assert!(!tries(&mut resting(squirrel(), 300.0, 1, &w), ObjectSpot { owner: Owner::Window(1), y: 400.0, ..trash(1, 700.0) }));
    assert!(!tries(&mut resting(pet_cfg(), 300.0, 1, &w), trash(1, 400.0)), "다람쥐만");
    let mut away = resting(squirrel(), 300.0, 1, &w);
    away.set_user_away(true);
    assert!(!tries(&mut away, trash(1, 400.0)));
}

#[test]
fn grabbed_with_full_cheeks_it_finishes_burying_once_free_again() {
    let w = world(vec![]);
    let mut p = resting(squirrel(), 600.0, 3, &w);
    let mut spots = vec![trash(1, 700.0)];
    assert!(p.start_stash(&spots[0].clone(), &w));
    run(&mut p, &w, &mut spots, 3.0, |_| {});
    assert_eq!(p.cheeks(), 1);
    // 사용자가 들었다가 놓았다
    p.pointer_down(p.body.x, p.body.y - 60.0);
    p.pointer_up(p.body.x, p.body.y - 60.0);
    let mut dug = false;
    run(&mut p, &w, &mut spots, 20.0, |p| dug |= p.render().anim == Anim::Dig);
    assert!(dug);
    assert_eq!(p.cheeks(), 0);
}
