//! 실제처럼 돌린 마주침 (social.rs + brain_meet.rs): 크기·종이 다른 두 동물(하나는 손님)이 스스로 걷고·쉬고·돌아서며
//! 한 바닥(또는 창 위)을 오갈 때, 서로 쪽으로 걷다 만나는 번마다 머리 밟고 넘기가 얼마나 보이는지.

use super::*;
use crate::brain::{Pet, PetConfig, Standing, State};
use crate::geom::{Rect, WindowInfo};
use crate::test_util::DT;
use crate::visit::Side;
use crate::world::World;

const GROUND: f64 = 1040.0;
const KINDS: [MeetKind; 4] = [MeetKind::Stomp, MeetKind::HighBounce, MeetKind::Pass, MeetKind::Climb];

/// 번들 pet.json 그대로 (게임이 쓰는 설정, 청소년, 배율 1)
fn species(id: &str) -> PetConfig {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets").join(id).join("pet.json");
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let m = crate::manifest::parse_manifest(&json).unwrap_or_else(|e| panic!("{id}: {e:?}"));
    PetConfig::from_manifest(&m, 1.0, m.size_mult(crate::care::Stage::Teenager), crate::care::Stage::Teenager.name())
}

/// owner 위 x에 막 내려선 동물 — 그다음은 스스로 정한다
fn settle(w: &World, c: PetConfig, owner: Owner, x: f64, seed: u64) -> Pet {
    let (ax, ay) = w.anchor(owner).unwrap();
    let mut p = Pet::new(c, x, ay, seed);
    p.standing = Some(Standing { owner, rel_x: x - ax });
    p.state = State::Idle { remaining: 0.5, look: false };
    // 화면 끝 너머에 이어진 친구가 없다 (손님도 집 생각이 들 때까지는 이 화면에서 논다)
    p.set_exit_edges(false, false);
    p
}

#[derive(Debug, Default, Clone, Copy)]
struct Tally {
    /// 둘이 서로 쪽으로 걷다 만난 번 (head_on이 참이 된 마주침)
    head_ons: usize,
    /// 실제로 고른 만남 (Stomp, HighBounce, Pass, Climb)
    kinds: [usize; 4],
}

/// 두 동물(b는 아직 집 생각이 없는 손님)을 secs초 동안 앱처럼 돌린다: 틱마다 Social::tick 뒤 각자 update
fn simulate(w: &World, owner: Owner, a: &str, b: &str, (lo, hi): (f64, f64), secs: f64, seed: u64) -> Tally {
    // 손님은 오른쪽 친구 화면에서 왔다. 집 쪽 끝에서 돌아가면 곧 다시 놀러 온다 (새 방문 = 새 동물)
    let guest = |n: u64| {
        let mut g = settle(w, species(b), owner, lo + 0.7 * (hi - lo), seed + 7 + n);
        g.set_guest(Side::Right, 1e9);
        g
    };
    let mut visits = 0;
    let mut pets = vec![settle(w, species(a), owner, lo + 0.3 * (hi - lo), seed), guest(visits)];
    let mut s = Social::new(seed);
    let mut t = Tally::default();
    let (mut facing_off, mut was_meeting) = (false, false);
    for _ in 0..(secs / DT) as usize {
        let (sa, sb) = (Snap::of(&pets[0]), Snap::of(&pets[1]));
        // 한 번의 마주침: head_on이 처음 참이 된 때부터 둘이 만나는 거리 밖으로 나갈 때까지
        if head_on(&sa, &sb) && !facing_off {
            facing_off = true;
            t.head_ons += 1;
        } else if !near(&sa, &sb, MEET_BODIES) {
            facing_off = false;
        }
        let meeting = pets.iter().any(Pet::is_meeting);
        if meeting && !was_meeting {
            if let Some(k) = pets.iter().find_map(Pet::meet_kind) {
                t.kinds[KINDS.iter().position(|x| *x == k).unwrap()] += 1;
            }
        }
        was_meeting = meeting;
        s.tick(DT, &mut pets);
        for p in pets.iter_mut() {
            p.update(DT, w);
        }
        if pets[1].is_away() {
            visits += 1;
            pets[1] = guest(visits);
        }
    }
    t
}

#[test]
fn walking_into_each_other_often_ends_in_a_head_stomp_over() {
    let floor = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let window = WindowInfo { id: 1, rect: Rect::new(400.0, 500.0, 1300.0, 1000.0) };
    let desk = World::new(vec![window], vec![Rect::new(0.0, 0.0, 1920.0, GROUND)], 128.0);
    let runs = [
        (&floor, Owner::Ground(0), "cat", "dog", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "rabbit", "bear", (0.0, 1920.0)),
        (&floor, Owner::Ground(0), "dino_green", "duck", (0.0, 1920.0)),
        (&desk, Owner::Window(1), "frog", "persian", (400.0, 1300.0)),
    ];
    let mut all = Tally::default();
    for (i, &(w, owner, a, b, span)) in runs.iter().enumerate() {
        let t = simulate(w, owner, a, b, span, 3600.0, 11 + i as u64);
        eprintln!("{a}+{b}: {t:?}");
        all.head_ons += t.head_ons;
        for k in 0..4 {
            all.kinds[k] += t.kinds[k];
        }
    }
    let stomps = all.kinds[0];
    let share = stomps as f64 / all.head_ons as f64;
    eprintln!("all: {all:?} stomps per head-on {share:.2}");
    assert!(all.head_ons > 60, "they do walk into each other: {all:?}");
    // 서로 쪽으로 걷다 만나면 세 번에 한 번쯤은 머리를 밟고 넘는다
    assert!(share > 0.25, "stomps per head-on {share:.2}: {all:?}");
    assert!(all.kinds[1..].iter().all(|&k| k < stomps), "the stomp is the usual meet: {all:?}");
}
