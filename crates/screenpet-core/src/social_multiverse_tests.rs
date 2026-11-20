//! 🕷️🕷️🕷️ 멀티버스 모임 (spec 4부 — social_multiverse.rs): 같은 화면에 거미인간이 둘 이상 한가하면 곧 모여 서로 놀라고 ("multiverse?!"),
//! 껴안고 뱅글뱅글 (하트·반짝이) 돌다가 "see ya!" 하며 흩어짐. 같은 무리는 몇 분 동안 다시 안 함. 혼자거나 다른 동물은 안 낌.

use super::*;
use crate::brain::{Pet, PetConfig, Speech, Standing, State};
use crate::geom::Rect;
use crate::test_util::{bundled, DT};
use crate::world::World;

const GROUND: f64 = 1040.0;

/// 번들 pet.json 그대로의 거미인간
fn spider() -> PetConfig {
    bundled().into_iter().find(|(id, _)| id == "spider").expect("ui/pets/spider").1
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0)
}

/// 바닥 x에 쉬는 펫 — 혼자 하는 놀이(줄 놀이·연령대 특기)는 꺼 둠
fn idle_at(w: &World, c: PetConfig, x: f64, seed: u64) -> Pet {
    let owner = Owner::Ground(0);
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.hush_stage();
    p.hush_web_play();
    p.body.x = x;
    p.body.y = ay;
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = State::Idle { remaining: 1e6, look: false };
    p.update(0.0, w);
    p
}

/// 다른 어울림 (인사·만남·하이파이브·펫끼리 놀이)은 끈 코디네이터
fn quiet(seed: u64) -> Social {
    let mut s = Social::new(seed);
    s.meet_chance = 0.0;
    s.one_way_chance = 0.0;
    s.set_high_five_chance(0.0);
    s.set_romp_rate(0.0);
    s.hold_greets(1e9);
    s
}

/// 앱처럼 한 틱
fn step(s: &mut Social, pets: &mut [Pet], w: &World) {
    s.tick(DT, pets);
    for p in pets.iter_mut() {
        p.update(DT, w);
        // 계속 쉬게 둠 (스스로 걸어가 버리지 않게)
        if matches!(p.state, State::Idle { .. } | State::Move { .. } | State::Turn { .. }) && !p.in_romp() {
            p.state = State::Idle { remaining: 1e6, look: false };
        }
    }
}

#[derive(Default)]
struct Seen {
    lines: Vec<(usize, Speech)>,
    hugged: bool,
    hearts: usize,
    sparkle: bool,
    lifted: bool,
    swapped: bool,
    ended_at: Option<f64>,
}

/// secs초 돌리며 본 것
fn watch(s: &mut Social, pets: &mut [Pet], w: &World, secs: f64) -> Seen {
    let mut seen = Seen::default();
    let n = pets.len();
    let order = |pets: &[Pet]| {
        let mut ix: Vec<usize> = (0..n).collect();
        ix.sort_by(|&a, &b| pets[a].body.x.total_cmp(&pets[b].body.x));
        ix
    };
    let mut was_order = order(pets);
    let mut started = false;
    for k in 0..(secs / DT) as usize {
        step(s, pets, w);
        for (i, p) in pets.iter_mut().enumerate() {
            if let Some(sp) = p.take_speech() {
                seen.lines.push((i, sp));
            }
            let r = p.render();
            seen.hearts += usize::from(r.heart);
            seen.sparkle |= r.sparkle && p.in_romp();
            seen.lifted |= p.in_romp() && p.window_pos().1 < p.body.y - p.cfg.size - 1.0;
        }
        let xs: Vec<f64> = pets.iter().map(|p| p.body.x).collect();
        let spread = xs.iter().copied().fold(f64::MIN, f64::max) - xs.iter().copied().fold(f64::MAX, f64::min);
        let all_in = pets.iter().all(|p| p.in_romp());
        started |= all_in;
        if all_in && spread < 1.2 * spider().size {
            seen.hugged = true;
            let now = order(pets);
            seen.swapped |= now != was_order;
            was_order = now;
        }
        if started && seen.ended_at.is_none() && pets.iter().all(|p| !p.in_romp()) {
            seen.ended_at = Some(k as f64 * DT);
        }
    }
    seen
}

#[test]
fn three_spiders_gather_gasp_hug_spin_and_scatter() {
    let w = floor();
    let mut pets = vec![idle_at(&w, spider(), 500.0, 1), idle_at(&w, spider(), 900.0, 2), idle_at(&w, spider(), 1400.0, 3)];
    let mut s = quiet(1);
    let seen = watch(&mut s, &mut pets, &w, 30.0);
    assert_eq!(s.multiverses_started(), 1, "한 번 모임");
    let gasps: Vec<usize> = seen.lines.iter().filter(|l| l.1 == Speech::Multiverse).map(|l| l.0).collect();
    assert!(gasps.len() >= 3, "번갈아 놀람 {:?}", seen.lines);
    assert!(gasps.windows(2).all(|g| g[0] != g[1]), "번갈아 {gasps:?}");
    assert!(seen.hugged && seen.swapped, "껴안고 가운데를 돎 (자리가 서로 바뀜)");
    assert!(seen.sparkle && seen.hearts > 0 && seen.lifted, "반짝·하트·들썩");
    let byes: Vec<usize> = seen.lines.iter().filter(|l| l.1 == Speech::SeeYa).map(|l| l.0).collect();
    let mut who = byes.clone();
    who.sort();
    who.dedup();
    assert_eq!(who, vec![0, 1, 2], "모두 see ya");
    let ended = seen.ended_at.expect("끝남");
    assert!(ended < 25.0, "{ended}");
    // 흩어졌음
    let xs: Vec<f64> = pets.iter().map(|p| p.body.x).collect();
    let spread = xs.iter().copied().fold(f64::MIN, f64::max) - xs.iter().copied().fold(f64::MAX, f64::min);
    assert!(spread > 2.0 * spider().size, "{xs:?}");
    // 같은 무리는 몇 분 동안 다시 안 함
    watch(&mut s, &mut pets, &w, 120.0);
    assert_eq!(s.multiverses_started(), 1, "쿨다운");
}

#[test]
fn guests_count_too_and_more_than_three_all_join() {
    let w = floor();
    let mut pets: Vec<Pet> = (0..4).map(|k| idle_at(&w, spider(), 400.0 + 350.0 * k as f64, 10 + k)).collect();
    pets[2].set_guest(crate::visit::Side::Left, 1e6);
    let mut s = quiet(2);
    let mut joined = [false; 4];
    for _ in 0..(10.0 / DT) as usize {
        step(&mut s, &mut pets, &w);
        for (j, p) in pets.iter().enumerate() {
            joined[j] |= p.in_romp();
        }
    }
    assert_eq!(joined, [true; 4], "손님도, 넷 다");
}

#[test]
fn two_spiders_pair_up_hug_and_spin() {
    let w = floor();
    let mut pets = vec![idle_at(&w, spider(), 600.0, 5), idle_at(&w, spider(), 1300.0, 6)];
    let mut s = quiet(4);
    let seen = watch(&mut s, &mut pets, &w, 30.0);
    assert_eq!(s.multiverses_started(), 1, "둘이어도 모임");
    let gasps: Vec<usize> = seen.lines.iter().filter(|l| l.1 == Speech::Multiverse).map(|l| l.0).collect();
    assert!(gasps.len() >= 2 && gasps.windows(2).all(|g| g[0] != g[1]), "번갈아 {gasps:?}");
    assert!(seen.hugged && seen.swapped && seen.sparkle && seen.hearts > 0, "짝꿍 포옹 뱅글");
    let mut byes: Vec<usize> = seen.lines.iter().filter(|l| l.1 == Speech::SeeYa).map(|l| l.0).collect();
    byes.sort();
    byes.dedup();
    assert_eq!(byes, vec![0, 1]);
    assert!(seen.ended_at.is_some());
}

#[test]
fn a_lone_spider_among_other_pets_never_gathers() {
    let w = floor();
    let cat = crate::test_util::pet_cfg();
    let mut pets = vec![idle_at(&w, spider(), 500.0, 1), idle_at(&w, cat.clone(), 900.0, 2), idle_at(&w, cat, 1300.0, 3)];
    let mut s = quiet(3);
    watch(&mut s, &mut pets, &w, 30.0);
    assert_eq!(s.multiverses_started(), 0);
}
