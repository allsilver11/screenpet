//! 📚 헤르미온느 (brain_study.rs): 📖 꽤 자주 앉아서 책 읽기 · 🙋 "I know!" · 👜 구슬 가방으로 바닥 청소
//! (가까운 것부터 하나씩 쏙 — 가방이 커져 질질 끌다가 "Phew! All clean!" 하고 가방이 사라짐, 장난감은 안 주움)

use super::study_moves::{BagPhase, StudyAct};
use super::swing_tests::resting;
use super::tests::world;
use super::*;
use crate::juggle::ObjectSpot;
use crate::play::ObjectPlay;
use crate::test_util::{pet_cfg, DT};
use crate::wizard::{self as wz, Spell, SpellSet};

const GROUND: f64 = 1040.0;

/// 헤르미온느 (테스트 설정): 책벌레 + 가방 청소 + Leviosa·Reparo
fn hermione() -> PetConfig {
    PetConfig { bookworm: true, bag_cleaner: true, rides_broom: true, spells: SpellSet::of(&[Spell::Leviosa, Spell::Reparo]), ..pet_cfg() }
}

/// 바닥(x)에 쉬는 헤르미온느 — 빗자루·주문·책·가방은 테스트가 시작
fn calm(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = resting(hermione(), x, seed, w);
    p.hush_stage();
    p.broom.ready_at = f64::INFINITY;
    p.spell.ready_at = f64::INFINITY;
    p.study.read_ready_at = f64::INFINITY;
    p.study.bag_ready_at = f64::INFINITY;
    p
}

/// 바닥의 물건 자리
fn thing(id: u64, x: f64, giant: bool, local: bool) -> ObjectSpot {
    ObjectSpot { id, x, y: GROUND, owner: Owner::Ground(0), half_w: if giant { 300.0 } else { 24.0 }, giant, local }
}

/// 앱처럼: 물건을 보여 주고, 움직이고, 가방에 넣은 것은 치움. (넣은 id들, 들은 말)
fn run(p: &mut Pet, w: &World, spots: &mut Vec<ObjectSpot>, secs: f64, mut each: impl FnMut(&Pet)) -> (Vec<u64>, Vec<Speech>) {
    let (mut gone, mut said) = (Vec::new(), Vec::new());
    for _ in 0..(secs / DT) as usize {
        p.notice_play_objects(spots, w);
        p.update(DT, w);
        if let Some(ObjectPlay::Vanish { id }) = p.take_object_play() {
            gone.push(id);
            spots.retain(|s| s.id != id);
        }
        if let Some(s) = p.take_speech() {
            said.push(s);
        }
        each(p);
    }
    (gone, said)
}

#[test]
fn study_animations_and_lines_have_their_sheet_names() {
    for (a, name) in [(Anim::Read, "read"), (Anim::RaiseHand, "raise_hand"), (Anim::BagWalk, "bag_walk"), (Anim::BagDrag, "bag_drag"), (Anim::BagStuff, "bag_stuff")] {
        assert_eq!(serde_json::to_string(&a).unwrap(), format!("\"{name}\""));
        assert!(crate::manifest::BOOK_ANIMS.contains(&name) || crate::manifest::BAG_ANIMS.contains(&name));
    }
    assert!(Anim::RaiseHand.is_front() && !Anim::Read.is_front());
    for (s, name) in [(Speech::IKnow, "i_know"), (Speech::Study, "study"), (Speech::TidyUp, "tidy_up"), (Speech::AllClean, "all_clean")] {
        assert_eq!(serde_json::to_string(&s).unwrap(), format!("\"{name}\""));
    }
}

/// 📖 앉아서 책을 읽다가 (다른 펫도 앎 — PetSpot.reading) 다 읽으면 평소대로
#[test]
fn she_sits_and_reads_then_gets_up() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    assert!(p.start_reading());
    assert_eq!(p.render().anim, Anim::Read);
    assert!(p.is_reading() && p.pet_spot().reading);
    let mut spots = Vec::new();
    run(&mut p, &w, &mut spots, wz::READ_SECS.1 + 1.0, |_| {});
    assert!(!p.is_reading());
    assert!(p.study.read_ready_at > p.clock, "조금 쉬었다 다시");
}

/// 📖 꽤 자주 — 15분 중 넉넉히 읽음. 책벌레가 아니면 안 읽음
#[test]
fn she_reads_fairly_often() {
    let w = world(vec![]);
    let mut p = calm(900.0, 2, &w);
    p.study.read_ready_at = 0.0;
    let mut other = resting(pet_cfg(), 900.0, 2, &w);
    let (mut reading, n) = (0, 60 * 60 * 15);
    for _ in 0..n {
        p.update(DT, &w);
        other.update(DT, &w);
        reading += p.is_reading() as u32;
        assert!(!other.is_reading());
    }
    let share = reading as f64 / n as f64;
    assert!(share > 0.15 && share < 0.7, "{share}");
}

/// 🙋 손 번쩍 "I know!" (앞모습) 뒤 평소대로
#[test]
fn she_raises_her_hand_i_know() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    assert!(p.start_know());
    assert_eq!((p.render().anim, p.speech), (Anim::RaiseHand, Some(Speech::IKnow)));
    let mut spots = Vec::new();
    run(&mut p, &w, &mut spots, wz::KNOW_SECS + 0.2, |_| {});
    assert_ne!(p.render().anim, Anim::RaiseHand);
}

/// 👜 가까운 것부터 하나씩 가방에 쏙 — 거대한 것도, 넣을수록 가방이 커져 질질 끌고 느려짐, 다 치우면 "Phew! All clean!" 하고 가방이 사라짐.
/// 📌 장난감 아이콘은 안 주움
#[test]
fn the_bag_swallows_everything_on_the_floor_one_by_one_and_grows() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    let mut spots = vec![
        thing(1, 1050.0, false, false),
        thing(2, 700.0, false, false),
        thing(3, 1200.0, false, false),
        thing(4, 400.0, false, false),
        thing(5, 1700.0, true, false),
        thing(6, 1350.0, false, false),
        thing(7, 1500.0, false, false),
        thing(8, 600.0, false, true),
    ];
    p.notice_play_objects(&spots, &w);
    assert!(p.start_bag(&w));
    assert_eq!(p.speech, Some(Speech::TidyUp));
    let (mut sizes, mut dragged, mut stuffed, mut slow_steps) = (Vec::new(), false, false, Vec::new());
    let mut last_x = p.body.x;
    let (gone, said) = run(&mut p, &w, &mut spots, 120.0, |p| {
        let r = p.render();
        if r.bag > 0.0 && sizes.last() != Some(&r.bag) {
            sizes.push(r.bag);
        }
        dragged |= r.anim == Anim::BagDrag;
        stuffed |= r.anim == Anim::BagStuff;
        if matches!(r.anim, Anim::BagWalk | Anim::BagDrag) {
            slow_steps.push(((p.body.x - last_x).abs() / DT, r.bag));
        }
        last_x = p.body.x;
    });
    let mut sorted = gone.clone();
    sorted.sort();
    assert_eq!(sorted, vec![1, 2, 3, 4, 5, 6, 7], "장난감(8) 말고 다 — 거대한 것도");
    assert_eq!(gone[0], 1, "가까운 것부터");
    assert!(sizes.windows(2).all(|w| w[1] > w[0]) && sizes.len() >= 6, "커짐 {sizes:?}");
    assert!((sizes[0] - wz::BAG_START).abs() < 1e-9 && (sizes.last().unwrap() - 1.0).abs() < 1e-9);
    assert!(dragged && stuffed);
    assert!(said.contains(&Speech::AllClean));
    assert_eq!(p.render().bag, 0.0, "가방이 사라짐");
    assert!(!p.study.bag_on && p.study.bag_items == 0);
    // 다 큰 가방은 느림
    let fast = slow_steps.iter().filter(|(_, b)| *b < 0.3).map(|(v, _)| *v).fold(0.0, f64::max);
    let slow = slow_steps.iter().filter(|(_, b)| *b >= 1.0).map(|(v, _)| *v).fold(0.0, f64::max);
    assert!(slow > 0.0 && slow < fast * 0.8, "{fast} {slow}");
}

/// 👜 물건이 BAG_MIN개 넘게 있을 때만 스스로 시작 (가끔) — 끼어들리면 가방은 그대로 두었다가 마저 치움
#[test]
fn she_tidies_up_on_her_own_and_finishes_after_an_interruption() {
    let w = world(vec![]);
    let mut p = calm(900.0, 3, &w);
    p.study.bag_ready_at = 0.0;
    let mut few = vec![thing(1, 1000.0, false, false)];
    let (gone, _) = run(&mut p, &w, &mut few, 300.0, |_| {});
    assert!(gone.is_empty(), "하나뿐이면 안 함");
    // 손님은 친구 화면을 치우지 않음
    let mut guest = calm(900.0, 3, &w);
    guest.study.bag_ready_at = 0.0;
    guest.set_guest(Side::Left, 1e9);
    let mut many = vec![thing(1, 1000.0, false, false), thing(2, 1100.0, false, false), thing(3, 1200.0, false, false)];
    let (gone, _) = run(&mut guest, &w, &mut many, 300.0, |_| {});
    assert!(gone.is_empty(), "손님은 안 치움");
    let mut spots = vec![thing(1, 1000.0, false, false), thing(2, 1100.0, false, false), thing(3, 1200.0, false, false)];
    let mut started = false;
    let mut interrupted = false;
    let mut gone_all = Vec::new();
    for _ in 0..(600.0 / DT) as usize {
        p.notice_play_objects(&spots, &w);
        p.update(DT, &w);
        if let Some(ObjectPlay::Vanish { id }) = p.take_object_play() {
            gone_all.push(id);
            spots.retain(|s| s.id != id);
            if !interrupted {
                // 하나 넣은 뒤 잡혔다 놓임
                interrupted = true;
                p.pointer_down(p.body.x, p.body.y - 40.0);
                p.update(DT, &w);
                p.pointer_up(p.body.x, p.body.y - 40.0);
                assert_eq!(p.render().bag, 0.0, "잡힌 동안은 가방이 안 보임");
                assert_eq!(p.study.bag_items, 1);
            }
        }
        started |= matches!(p.state, State::Study { act: StudyAct::Bag(_), .. });
        if spots.is_empty() && !p.study.bag_on {
            break;
        }
    }
    assert!(started && interrupted);
    assert_eq!(gone_all.len(), 3, "마저 치움 {gone_all:?}");
}

/// 👜 가방 단계별 그림: 작으면 들고 걷기, 커지면 끌기
#[test]
fn a_small_bag_is_carried_and_a_big_one_dragged() {
    let w = world(vec![]);
    let mut p = calm(900.0, 1, &w);
    p.study.bag_on = true;
    p.study.bag_items = 0;
    p.state = State::Study { act: StudyAct::Bag(BagPhase::Approach { id: 1, x: 1200.0, obj_x: 1220.0 }), t: 0.0 };
    assert_eq!(p.render().anim, Anim::BagWalk);
    p.study.bag_items = wz::BAG_FULL;
    assert_eq!(p.render().anim, Anim::BagDrag);
    assert!((p.render().bag - 1.0).abs() < 1e-9);
}
