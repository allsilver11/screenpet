//! 🕸️📦 물건도 거미줄에 걸림 (spec 5부 — object.rs Snagged): 보이는 거미집 안으로 날아오거나 떨어지거나 굴러온 물건은 무조건 걸려
//! "팅~" 출렁 뒤 매달려 살랑, 거미집이 사라지면 떨어지고, 창 모서리 거미집이 옮겨 가면 따라감, 잡아 끌면 빠지고 1초 동안은 다시 안 걸림.
//! 들려 있거나 동물이 든 것·회오리 속인 것·숨은 거미집은 안 걸림.
use super::*;
use crate::geom::Rect;
use crate::visit::ExitEdges;
use crate::web::{self, Corner, WebSpot};
use crate::world::World;

const DT: f64 = 0.016;
const NONE: ExitEdges = ExitEdges { left: false, right: false, up: false, down: false };

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0)
}

/// 화면 위 가운데 아래로 펼쳐진 거미집 (윗변 위 (x, y) 모서리, 반지름 r)
fn web_at(id: u64, x: f64, y: f64, r: f64) -> WebSpot {
    WebSpot { id, x, y, corner: Corner::TopLeft, size: r, hidden: false }
}

/// 물건 가운데 (그림 가운데 — 바닥에서 반 높이 위)
fn centre(o: &Object) -> (f64, f64) {
    (o.body.x, o.body.y - 0.5 * o.hit_box().1)
}

/// 한 틱: 거미집을 알리고 tick
fn tick(o: &mut Object, w: &World, webs: &[WebSpot]) {
    o.set_webs(webs);
    assert!(o.tick(DT, w, NONE, false).is_none());
}

/// 떨어지는 물건이 거미집에 걸리기까지 (최대 3초) — 걸렸으면 true
fn fall_into(o: &mut Object, w: &World, webs: &[WebSpot]) -> bool {
    for _ in 0..(3.0 / DT) as usize {
        tick(o, w, webs);
        if o.is_snagged() {
            return true;
        }
    }
    false
}

#[test]
fn a_falling_object_is_always_caught_by_a_visible_web_and_hangs_there() {
    let w = floor();
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    for (seed, kind) in ObjectKind::ALL.iter().copied().filter(|k| !k.is_giant()).enumerate() {
        let mut o = Object::new(kind, 800.0, 100.0, 1.0, seed as u64);
        assert!(fall_into(&mut o, &w, &webs), "{kind:?} 무조건 걸림");
        let (cx, cy) = centre(&o);
        assert!(web::in_web(cx, cy, 700.0, 300.0, Corner::TopLeft, 300.0 + o.hit_box().1), "{kind:?} 거미집 자리");
        // 그 자리에 매달림 — 떨어지지도 구르지도 않음 (2초 동안 아주 조금만 흔들림)
        let at = centre(&o);
        let mut far: f64 = 0.0;
        for _ in 0..(2.0 / DT) as usize {
            tick(&mut o, &w, &webs);
            assert!(o.is_snagged() && !o.is_asleep());
            far = far.max((o.body.x - at.0).hypot(centre(&o).1 - at.1));
        }
        assert!(far < o.hit_box().1, "{kind:?} 매달려 있음 {far}");
        assert!(o.spot(0).is_none(), "바닥 물건이 아님 (동물이 줍지 않음)");
    }
}

#[test]
fn it_twangs_along_the_entry_direction_then_settles_into_a_faint_sway() {
    let w = floor();
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    assert!(fall_into(&mut o, &w, &webs));
    let caught = centre(&o);
    let mut ys = Vec::new();
    let mut xs = Vec::new();
    for _ in 0..(2.5 / DT) as usize {
        tick(&mut o, &w, &webs);
        ys.push(centre(&o).1 - caught.1);
        xs.push(o.body.x - caught.0);
    }
    let spring = (web::SNAG_SPRING_SECS / DT) as usize;
    let max_down = ys[..spring].iter().copied().fold(f64::MIN, f64::max);
    let max_up = ys[..spring].iter().copied().fold(f64::MAX, f64::min);
    assert!(max_down > 5.0, "떨어지던 쪽(아래)으로 쭉 늘어남 {max_down}");
    assert!(max_up < -1.0, "되돌아오며 출렁 {max_up}");
    let first = ys.iter().position(|y| y.abs() > 1.0).unwrap();
    assert!(ys[first] > 0.0, "먼저 아래로");
    let late = &ys[spring + 5..];
    assert!(late.iter().all(|y| y.abs() < 1.5), "출렁은 멎음");
    assert!(xs[spring + 5..].iter().any(|x| x.abs() > 0.05), "그 뒤 살랑살랑");
    assert!(xs[spring + 5..].iter().all(|x| x.abs() < 0.1 * o.hit_box().1), "아주 살짝");
}

#[test]
fn a_thrown_object_and_a_rolling_one_are_caught_too() {
    let w = floor();
    // 던진 것: 오른쪽으로 날아가다가
    let webs = [web_at(3, 1200.0, 200.0, 350.0)];
    let mut o = Object::new(ObjectKind::Paper, 900.0, 400.0, 1.0, 2);
    o.launch(900.0, -200.0);
    assert!(fall_into(&mut o, &w, &webs), "던진 것");
    assert!(!o.in_flight(), "더는 날지 않음");
    // 굴러가는 것: 바닥 오른쪽 아래 구석의 거미집으로
    let floor_web = [WebSpot { id: 4, x: 1920.0, y: 1040.0, corner: Corner::BottomRight, size: 300.0, hidden: false }];
    let mut r = Object::new(ObjectKind::Can, 1500.0, 1040.0, 1.0, 3);
    for _ in 0..30 {
        tick(&mut r, &w, &[]);
    }
    assert!(r.nudge(1.0, 600.0), "툭 밀어 굴림");
    assert!(fall_into(&mut r, &w, &floor_web), "굴러가던 것");
}

#[test]
fn it_drops_when_the_web_goes_away_and_follows_a_web_that_moves() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    assert!(fall_into(&mut o, &w, &webs));
    for _ in 0..80 {
        tick(&mut o, &w, &webs);
    }
    // 창이 오른쪽 아래로 옮겨 가면 같이 감
    let at = centre(&o);
    let moved = [web_at(9, 900.0, 350.0, 300.0)];
    tick(&mut o, &w, &moved);
    let now = centre(&o);
    assert!((now.0 - at.0 - 200.0).abs() < 3.0 && (now.1 - at.1 - 50.0).abs() < 3.0, "{at:?} → {now:?}");
    // 다른 거미집만 남으면 (이 거미집이 사라짐) 그 자리에서 떨어짐
    let other = [web_at(5, 100.0, 100.0, 100.0)];
    tick(&mut o, &w, &other);
    assert!(!o.is_snagged());
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut o, &w, &other);
    }
    assert_eq!(o.body.y, 1040.0, "바닥까지 떨어짐");
}

#[test]
fn dragging_frees_it_and_it_is_not_caught_again_for_a_second() {
    let w = floor();
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    assert!(fall_into(&mut o, &w, &webs));
    let (x, y) = centre(&o);
    o.pointer_down(x, y);
    assert!(o.is_held() && !o.is_snagged());
    // 잡은 채 거미집 안에서 끌어도 안 걸림
    for _ in 0..10 {
        o.pointer_move(x + 5.0, y);
        tick(&mut o, &w, &webs);
        assert!(!o.is_snagged());
    }
    // 그 자리에서 놓아도 (클릭) 1초 동안은 다시 안 걸림 — 거미집 안에 붙들어 두어도
    o.pointer_up();
    for _ in 0..((web::SNAG_IMMUNE_SECS - 2.0 * DT) / DT) as usize {
        o.pointer_down(x, y);
        o.pointer_up();
        tick(&mut o, &w, &webs);
        assert!(!o.is_snagged(), "막 빠진 것");
    }
    // 그 뒤엔 다시 걸림
    let mut again = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 4);
    assert!(fall_into(&mut again, &w, &webs));
}

#[test]
fn hidden_webs_carried_and_whirled_objects_never_snag() {
    let w = floor();
    let hidden = [WebSpot { hidden: true, ..web_at(9, 700.0, 300.0, 300.0) }];
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    assert!(!fall_into(&mut o, &w, &hidden), "숨은 거미집");
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    let mut c = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 2);
    for _ in 0..60 {
        c.carry_at(800.0, 420.0);
        tick(&mut c, &w, &webs);
        assert!(c.is_carried() && !c.is_snagged(), "동물이 든 것");
    }
    let mut h = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 3);
    for _ in 0..30 {
        tick(&mut h, &w, &[]);
    }
    for _ in 0..30 {
        assert!(h.whirl_at(800.0, 420.0, 10.0));
        tick(&mut h, &w, &webs);
        assert!(!h.is_snagged(), "회오리 속");
    }
}

/// 🕸️📦 둥근 거미집 (Corner::Center — spec 6부)에도 물건이 걸림: 위에서 떨어지면 원 윗반쪽에서 (구석 사분원이면 못 걸리는 자리)
#[test]
fn a_falling_object_is_caught_by_a_round_web_too() {
    let w = floor();
    let webs = [WebSpot { id: 5, x: 800.0, y: 500.0, corner: Corner::Center, size: 200.0, hidden: false }];
    let kind = ObjectKind::ALL.iter().copied().find(|k| !k.is_giant()).unwrap();
    let mut o = Object::new(kind, 760.0, 100.0, 1.0, 1);
    assert!(fall_into(&mut o, &w, &webs), "걸림");
    let (cx, cy) = centre(&o);
    assert!(cy < 500.0 && cx < 800.0, "원 왼쪽 위에서 걸림 {cx},{cy}");
}

#[test]
fn it_drops_when_its_web_is_no_longer_shown() {
    // 사용자: 거미줄은 없어졌는데 걸려 있던 물건이 계속 공중에 떠 있음 — 거미집이 앞 창에 가려지거나 흐려져 사라지는 중(hidden)이면
    // 화면엔 거미집이 없으니 물건도 꼭 떨어져야 함
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    let webs = [web_at(9, 700.0, 300.0, 300.0)];
    assert!(fall_into(&mut o, &w, &webs));
    let hidden = [WebSpot { hidden: true, ..webs[0] }];
    tick(&mut o, &w, &hidden);
    assert!(!o.is_snagged());
    for _ in 0..(3.0 / DT) as usize {
        tick(&mut o, &w, &hidden);
    }
    assert_eq!(o.body.y, 1040.0, "바닥까지 떨어짐");
}
