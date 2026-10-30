//! 창을 따라 움직이기 — 게임 루프가 매 틱 기대고 있는 창 경계만 고쳐 넣을 때 (World::patch_windows) 동물이 창에 붙어 있는지.
//! brain.rs의 자식 모듈 (비공개 필드에 접근).

use super::tests::{cfg, place, w1, win, world};
use super::*;
use crate::geom::Rect;
use crate::inside::{Mode, Stay};

const DT: f64 = 1.0 / 60.0;

fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

#[test]
fn ridden_window_is_the_window_the_pet_depends_on() {
    let w = world(vec![w1()]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Ground(0), 200.0, idle());
    assert_eq!(p.ridden_window(), None);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    assert_eq!(p.ridden_window(), Some(1));
    place(&mut p, &w, Owner::Inside(1), 800.0, idle());
    p.inside = Some(Stay { window: 1, mode: Mode::In, leave_in: 100.0, climb_out: false, hops: 0 });
    assert_eq!(p.ridden_window(), Some(1));
    // 창 옆면·안쪽 벽에 매달려 있어도 (디딤판 없음)
    p.standing = None;
    p.inside = None;
    p.state = State::WallClimb { window: 1, side: Side::Left, rel_y: 50.0 };
    assert_eq!(p.ridden_window(), Some(1));
    p.state = State::InnerClimb { window: 1, rel_x: 4.0, rel_y: 300.0 };
    assert_eq!(p.ridden_window(), Some(1));
    p.state = State::Air { anim: Anim::Fall };
    assert_eq!(p.ridden_window(), None);
}

/// 창이 매 틱 10 px씩 움직여도 (끌어 옮기는 중) 창 위에 선 동물은 같은 자리에 붙어 따라간다 — 한 틱도 늦지 않는다
#[test]
fn pet_on_a_moving_window_stays_glued_every_tick() {
    let mut w = world(vec![w1(), win(2, 1300.0, 600.0, 1700.0, 900.0)]);
    let mut p = Pet::new(cfg(), 0.0, 0.0, 1);
    place(&mut p, &w, Owner::Window(1), 800.0, idle());
    let rel_x = 800.0 - 500.0;
    let mut rect = w1().rect;
    for tick in 0..60 {
        rect = Rect::new(rect.left + 10.0, rect.top - 2.0, rect.right + 10.0, rect.bottom - 2.0);
        if let Some(id) = p.ridden_window() {
            assert!(w.patch_windows(&[(id, rect)]));
        }
        p.update(DT, &w);
        assert_eq!(p.standing.map(|s| (s.owner, s.rel_x)), Some((Owner::Window(1), rel_x)), "tick {tick}");
        assert_eq!((p.body.x, p.body.y), (rect.left + rel_x, rect.top), "tick {tick}");
    }
}
