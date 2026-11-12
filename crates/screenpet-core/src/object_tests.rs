use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::visit::{ExitEdges, Fling, Side};
use crate::surfaces::Owner;
use crate::world::World;
use crate::test_util::LoopLimit;

const DT: f64 = 0.016;

fn world() -> World {
    let w = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
    World::new(vec![w], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0)
}

fn floor() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0)
}

const NONE: ExitEdges = ExitEdges { left: false, right: false, up: false, down: false };
const ALL: ExitEdges = ExitEdges { left: true, right: true, up: true, down: true };

fn run(o: &mut Object, w: &World, secs: f64) -> Option<Exit> {
    let mut t = 0.0;
    while t < secs {
        if let Some(e) = o.tick(DT, w, NONE, false) {
            return Some(e);
        }
        t += DT;
    }
    None
}

/// 커서를 (vx, vy) 물리 px/s로 끌다가 놓는다
fn fling(o: &mut Object, w: &World, from: (f64, f64), v: (f64, f64)) {
    o.pointer_down(from.0, from.1);
    for i in 1..=8 {
        let t = i as f64 * DT;
        o.tick(DT, w, NONE, false);
        o.pointer_move(from.0 + v.0 * t, from.1 + v.1 * t);
    }
    o.pointer_up();
}

fn resting_on(o: &Object, owner: Owner) -> bool {
    o.is_asleep() && o.owner() == Some(owner)
}

#[test]
fn kinds_round_trip_and_unknown_ids_are_rejected() {
    assert!(ObjectKind::ALL.len() >= 8);
    for k in ObjectKind::ALL {
        assert_eq!(ObjectKind::from_id(k.id()), Some(k));
        assert!(!k.label().is_empty());
        // 창 라벨·파일 이름에 그대로 쓰는 글자만
        assert!(k.id().chars().all(|c| c.is_ascii_lowercase() || c == '_'));
    }
    // "banana": 없앤 종류 — 옛 버전 친구가 아직 보낼 수 있다
    for bad in ["", "CAN", "../can", "cat", "can\n", "<img>", "banana"] {
        assert_eq!(ObjectKind::from_id(bad), None, "{bad:?}");
    }
}

#[test]
fn a_dropped_object_falls_lands_and_sleeps() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 200.0, 1.0, 1);
    assert!(!o.is_asleep());
    assert!(run(&mut o, &w, 3.0).is_none());
    assert!(resting_on(&o, Owner::Ground(0)));
    assert_eq!((o.body.y, o.body.vx, o.body.vy), (1040.0, 0.0, 0.0));
    assert_eq!(o.angle(), 0.0);
}

#[test]
fn an_object_never_moves_by_itself_between_gusts() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Shirt, 800.0, 1040.0, 1.0, 7);
    run(&mut o, &w, 1.0);
    assert!(o.is_asleep());
    let at = o.body;
    // 바람은 가장 빠른 때(30초)가 되어서야 분다
    let mut t = 0.0;
    while t < GUST_EVERY_SECS.0 - 2.0 {
        assert!(o.tick(DT, &w, NONE, true).is_none());
        assert_eq!(o.body, at);
        assert!(o.is_asleep());
        t += DT;
    }
}

#[test]
fn no_wind_while_the_user_is_away() {
    assert!(gusts_ok(false, false));
    assert!(!gusts_ok(true, false), "hidden");
    assert!(!gusts_ok(false, true), "away");
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 960.0, 1040.0, 1.0, 3);
    run(&mut o, &w, 0.5);
    let start = (o.body.x, o.body.y);
    let mut t = 0.0;
    while t < 3600.0 {
        o.tick(DT, &w, NONE, gusts_ok(false, true));
        t += DT;
        assert!(o.is_asleep());
    }
    assert_eq!((o.body.x, o.body.y), start);
}

#[test]
fn gusts_come_every_30_to_90_seconds_and_only_a_short_way() {
    for seed in 0..12 {
        let w = floor();
        let mut o = Object::new(ObjectKind::Can, 960.0, 1040.0, 1.0, seed);
        run(&mut o, &w, 0.5);
        let start = o.body.x;
        let mut t = 0.0;
        let mut limit = LoopLimit::new();
        while o.is_asleep() {
            limit.step();
            o.tick(DT, &w, NONE, true);
            t += DT;
            assert!(t <= GUST_EVERY_SECS.1 + 1.0, "no gust in time");
        }
        assert!(t >= GUST_EVERY_SECS.0 - 1.0, "gust too soon: {t}");
        // 그동안 혼자 움직였다가 다시 잠든다
        let mut t = 0.0;
        let mut limit = LoopLimit::new();
        while !o.is_asleep() {
            limit.step();
            o.tick(DT, &w, NONE, false);
            t += DT;
            assert!(t < 5.0, "gust never ended");
        }
        let moved = (o.body.x - start).abs();
        assert!(moved > 10.0 && moved <= 200.0, "moved {moved}");
    }
}

#[test]
fn a_gust_is_the_same_roll_light_things_turn_over_heavy_things_barely_budge() {
    let gust_of = |kind| {
        let w = floor();
        let mut o = Object::new(kind, 960.0, 1040.0, 1.0, 3);
        run(&mut o, &w, 0.5);
        o.start_gust(1.0);
        let start = o.body.x;
        let mut turned = 0.0f64;
        for _ in 0..600 {
            let before = o.angle();
            o.tick(DT, &w, NONE, false);
            if !o.is_asleep() {
                turned += (o.angle() - before).abs();
            }
        }
        assert!(o.is_asleep(), "{kind:?}");
        (turned, (o.body.x - start).abs())
    };
    let (can_turned, can_went) = gust_of(ObjectKind::Can);
    assert!(can_turned > 90.0 && can_went > 40.0, "can {can_turned} {can_went}");
    let (computer_turned, computer_went) = gust_of(ObjectKind::Computer);
    assert!(computer_turned < 60.0 && computer_went <= 30.0, "computer {computer_turned} {computer_went}");
    let (stone_turned, stone_went) = gust_of(ObjectKind::Stone);
    assert!(stone_turned < can_turned && stone_went <= 30.0, "stone {stone_turned} {stone_went}");
    let (sock_turned, _) = gust_of(ObjectKind::Shirt);
    assert!(sock_turned < can_turned, "sock {sock_turned} can {can_turned}");
}

#[test]
fn a_gust_never_carries_an_object_across_a_friend_edge() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 1920.0 - 30.0, 1040.0, 1.0, 5);
    run(&mut o, &w, 0.5);
    o.start_gust(1.0);
    for _ in 0..300 {
        assert!(o.tick(DT, &w, ALL, false).is_none());
    }
    assert!(o.body.x <= 1920.0 - o.half_w());
    assert!(o.is_asleep());
}

#[test]
fn hidden_means_no_gust() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 960.0, 1040.0, 1.0, 5);
    run(&mut o, &w, 0.5);
    let at = o.body;
    let mut t = 0.0;
    while t < GUST_EVERY_SECS.1 + 5.0 {
        o.tick(0.25, &w, NONE, false);
        t += 0.25;
    }
    assert_eq!(o.body, at);
}

#[test]
fn a_gust_can_roll_an_object_off_a_window_edge() {
    let w = world();
    let mut o = Object::new(ObjectKind::Can, 1095.0, 400.0, 1.0, 5);
    run(&mut o, &w, 0.5);
    assert!(resting_on(&o, Owner::Window(1)));
    o.start_gust(1.0);
    run(&mut o, &w, 4.0);
    assert!(resting_on(&o, Owner::Ground(0)), "{:?} at {:?}", o.owner(), o.body);
}

#[test]
fn it_lands_on_a_window_rides_it_and_falls_when_it_goes_away() {
    let mut w = world();
    let mut o = Object::new(ObjectKind::Paper, 800.0, 200.0, 1.0, 1);
    run(&mut o, &w, 2.0);
    assert!(resting_on(&o, Owner::Window(1)));
    assert_eq!(o.body.y, 400.0);
    // 창을 옮기면 그대로 따라가고, 잠든 채로다
    w.patch_windows(&[(1, Rect::new(600.0, 300.0, 1200.0, 800.0))]);
    assert!(o.reseat(&w));
    assert_eq!((o.body.x, o.body.y), (900.0, 300.0));
    assert!(o.is_asleep());
    // 그대로면 할 일 없음
    assert!(!o.reseat(&w));
    // 창이 사라지면 떨어져 바닥에
    let gone = floor();
    assert!(o.reseat(&gone));
    assert!(!o.is_asleep());
    run(&mut o, &gone, 3.0);
    assert!(resting_on(&o, Owner::Ground(0)));
}

#[test]
fn a_click_without_movement_is_a_click_and_stays_put() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    run(&mut o, &w, 0.5);
    o.pointer_down(800.0, 1000.0);
    assert!(o.is_held());
    o.tick(DT, &w, NONE, false);
    o.pointer_move(801.0, 1000.0);
    assert_eq!(o.pointer_up(), Release::Click);
    run(&mut o, &w, 0.5);
    assert!(o.is_asleep());
    assert!((o.body.x - 801.0).abs() < 2.0);
}

#[test]
fn a_long_press_or_a_drag_is_not_a_click() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    run(&mut o, &w, 0.5);
    o.pointer_down(800.0, 1000.0);
    o.tick(DT, &w, NONE, false);
    o.pointer_move(900.0, 800.0);
    assert_eq!(o.pointer_up(), Release::Throw);
    run(&mut o, &w, 3.0);
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    run(&mut o, &w, 0.5);
    o.pointer_down(800.0, 1000.0);
    run(&mut o, &w, 1.0);
    assert_eq!(o.pointer_up(), Release::Throw);
}

#[test]
fn a_drag_keeps_the_grab_point_and_throws_like_a_pet() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    run(&mut o, &w, 0.5);
    o.pointer_down(810.0, 1000.0);
    o.tick(DT, &w, NONE, false);
    o.pointer_move(860.0, 900.0);
    assert_eq!((o.body.x, o.body.y), (850.0, 940.0));
    o.pointer_up();
    assert!(!o.is_held());

    // 같은 몸짓이면 화면 배율이 달라도 같은 거리로 날아간다
    let distance = |dpi: f64| {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 8000.0 * dpi, 1040.0 * dpi)], 96.0 * dpi);
        let mut o = Object::new(ObjectKind::Can, 100.0 * dpi, 800.0 * dpi, dpi, 1);
        fling(&mut o, &w, (100.0 * dpi, 760.0 * dpi), (1000.0 * dpi, -300.0 * dpi));
        let start = o.body.x;
        run(&mut o, &w, 6.0);
        assert!(o.is_asleep());
        (o.body.x - start) / dpi
    };
    let (a, b) = (distance(1.0), distance(2.0));
    assert!(a > 300.0 && (a - b).abs() / a < 0.08, "{a} vs {b}");
}

#[test]
fn it_bounces_lightly_when_it_lands_hard() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 100.0, 1.0, 1);
    let mut bounced = false;
    let mut was_down = false;
    for _ in 0..400 {
        o.tick(DT, &w, NONE, false);
        was_down |= o.body.y >= 1039.0;
        bounced |= was_down && o.body.vy < 0.0;
    }
    assert!(bounced);
    assert!(o.is_asleep());
}

#[test]
fn a_throw_crosses_only_into_a_linked_edge_and_carries_its_motion() {
    let w = floor();
    for (dir, side, edges, closed) in [
        (1.0, Side::Right, ExitEdges { right: true, ..NONE }, ExitEdges { left: true, ..NONE }),
        (-1.0, Side::Left, ExitEdges { left: true, ..NONE }, ExitEdges { right: true, ..NONE }),
    ] {
        let x = if dir > 0.0 { 1800.0 } else { 120.0 };
        // 연결된 끝: 넘어간다
        let mut o = Object::new(ObjectKind::Can, x, 600.0, 1.0, 1);
        fling(&mut o, &w, (x, 560.0), (dir * 1800.0, -100.0));
        let mut exit = None;
        for _ in 0..200 {
            exit = o.tick(DT, &w, edges, false);
            if exit.is_some() {
                break;
            }
        }
        let e = exit.expect("crossed");
        assert_eq!(e.side, side);
        assert!(e.fling.vx * dir > 0.0 && e.fling.height > 0.0 && e.fling.height < 1.0);
        // 연결 안 된 끝: 튕겨서 화면 안에 남는다
        let mut o = Object::new(ObjectKind::Can, x, 600.0, 1.0, 1);
        fling(&mut o, &w, (x, 560.0), (dir * 1800.0, -100.0));
        for _ in 0..400 {
            assert!(o.tick(DT, &w, closed, false).is_none());
        }
        assert!(o.body.x > 0.0 && o.body.x < 1920.0);
    }
}

#[test]
fn a_throw_crosses_up_and_hard_down_when_linked() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 900.0, 300.0, 1.0, 1);
    fling(&mut o, &w, (900.0, 260.0), (0.0, -2500.0));
    let up = ExitEdges { up: true, ..NONE };
    let mut exit = None;
    for _ in 0..200 {
        exit = o.tick(DT, &w, up, false);
        if exit.is_some() {
            break;
        }
    }
    let e = exit.expect("crossed up");
    assert_eq!(e.side, Side::Up);
    assert!((e.fling.height - 900.0 / 1920.0).abs() < 0.01);

    // 아래: 세게 내리꽂아야 넘어간다 (살짝 떨어뜨리면 바닥에 앉는다)
    let down = ExitEdges { down: true, ..NONE };
    let mut o = Object::new(ObjectKind::Can, 900.0, 500.0, 1.0, 1);
    fling(&mut o, &w, (900.0, 460.0), (0.0, 2800.0));
    let mut exit = None;
    for _ in 0..100 {
        exit = o.tick(DT, &w, down, false);
        if exit.is_some() {
            break;
        }
    }
    assert_eq!(exit.expect("crossed down").side, Side::Down);
    let mut o = Object::new(ObjectKind::Can, 900.0, 800.0, 1.0, 1);
    for _ in 0..200 {
        assert!(o.tick(DT, &w, down, false).is_none());
    }
    assert!(o.is_asleep());
}

#[test]
fn just_dropping_or_a_landed_object_never_crosses() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 1880.0, 900.0, 1.0, 1);
    for _ in 0..300 {
        assert!(o.tick(DT, &w, ALL, false).is_none());
    }
    assert!(o.is_asleep());
}

#[test]
fn it_flies_in_from_each_edge_keeping_height_and_speed() {
    let w = floor();
    let f = Fling { height: 0.25, vx: 500.0, vy: -100.0 };
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Left, f, 1.0, 1);
    assert!(o.body.x < 100.0 && o.body.vx > 0.0);
    assert!((o.body.y - 260.0).abs() <= 1.0);
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Right, f, 2.0, 1);
    assert!(o.body.x > 1800.0 && o.body.vx == -1000.0 && o.body.vy == -200.0);
    // 위에서: 그 폭 비율 자리에서 내려온다, 아래에서: 튀어 오른다
    let f = Fling { height: 0.5, vx: 20.0, vy: -900.0 };
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Up, f, 1.0, 1);
    assert!((o.body.x - 960.0).abs() < 1.0 && o.body.vy >= 0.0);
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Down, Fling { height: 0.5, vx: 0.0, vy: 2000.0 }, 1.0, 1);
    assert!(o.body.vy < 0.0 && o.body.y <= 1040.0);
    // 어디서 왔든 결국 이 화면에 내려앉아 잠든다
    for side in [Side::Left, Side::Right, Side::Up, Side::Down] {
        let mut o = Object::flown_in(ObjectKind::Shirt, &w, side, Fling { height: 0.5, vx: 300.0, vy: -50.0 }, 1.0, 1);
        run(&mut o, &w, 8.0);
        assert!(o.is_asleep(), "{side:?}");
        assert!(o.body.x >= 0.0 && o.body.x <= 1920.0);
    }
}

#[test]
fn accept_toss_validates_everything_a_peer_sends() {
    // 모르는 종류는 버린다
    assert!(accept_toss("cat", None, None).is_none());
    assert!(accept_toss("../../x", None, Some("hi")).is_none());
    // 없앤 종류: 옛 버전 친구가 던져도 조용히 버린다
    assert!(accept_toss("banana", None, Some("hi")).is_none());
    // 이상한 fling은 안전한 기본값으로, 너무 빠르면 잘린다
    let t = accept_toss("can", Some(Fling { height: f64::NAN, vx: 1.0, vy: 1.0 }), None).unwrap();
    assert!(t.fling.height.is_finite() && t.fling.vx.is_finite());
    let t = accept_toss("can", Some(Fling { height: 9.0, vx: 1e9, vy: -1e9 }), None).unwrap();
    assert_eq!(t.fling.height, 1.0);
    assert!(t.fling.vx.hypot(t.fling.vy) <= crate::visit::MAX_FLING_SPEED + 1.0);
    assert!(accept_toss("can", None, None).is_some());
    // 글자는 편지처럼 다듬는다: 보이지 않는 글자 없음, 80자까지, 공백뿐이면 포스트잇 없음
    let t = accept_toss("can", None, Some("  a\u{202e}b\u{200b}\nc  ")).unwrap();
    assert_eq!(t.note.as_deref(), Some("abc"));
    let long = "가".repeat(500);
    assert_eq!(accept_toss("can", None, Some(&long)).unwrap().note.unwrap().chars().count(), 80);
    assert_eq!(accept_toss("can", None, Some(" \u{200b} ")).unwrap().note, None);
}

#[test]
fn a_received_post_it_shows_once_for_ten_seconds_then_is_gone_forever() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    o.receive_note(Some("안녕".into()));
    assert_eq!(o.postit(), Postit::Unread);
    assert!(o.has_sticker());
    // 눌러 읽으면 글자를 한 번 내주고, 스티커는 떨어진다
    assert_eq!(o.read_note().as_deref(), Some("안녕"));
    assert_eq!(o.postit(), Postit::Reading);
    assert!(!o.has_sticker());
    assert_eq!(o.read_note(), None, "can't read twice");
    // 읽는 중엔 던져 넘겨도 글자가 따라가지 않는다
    assert_eq!(o.note_to_send(), None);
    // 10초가 다 되어야 끝난다 (딱 한 번 알린다)
    let mut t = 0.0;
    let mut ended = 0;
    while t < NOTE_READ_SECS - 0.1 {
        o.tick(DT, &w, NONE, false);
        ended += o.take_note_ended() as i32;
        t += DT;
    }
    assert_eq!(ended, 0);
    for _ in 0..30 {
        o.tick(DT, &w, NONE, false);
        ended += o.take_note_ended() as i32;
    }
    assert_eq!(ended, 1);
    assert_eq!(o.postit(), Postit::None);
    assert_eq!(o.read_note(), None);
    assert_eq!(o.note_to_send(), None);
    // 그 뒤 다시 잠든다
    run(&mut o, &w, 1.0);
    assert!(o.is_asleep());
}

#[test]
fn a_post_it_i_wrote_is_not_read_by_my_own_click_and_flies_with_the_throw() {
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    assert!(o.attach_note(" 받아라! \u{202e}"));
    assert_eq!(o.postit(), Postit::Mine);
    assert!(o.has_sticker());
    assert_eq!(o.read_note(), None);
    assert_eq!(o.postit(), Postit::Mine);
    assert_eq!(o.note_to_send().as_deref(), Some("받아라!"));
    // 떼면 없다
    assert!(o.peel_note());
    assert_eq!(o.postit(), Postit::None);
    assert!(!o.peel_note());
    // 빈 글자는 안 붙는다
    assert!(!o.attach_note(" \u{200b} "));
    assert_eq!(o.postit(), Postit::None);
}

#[test]
fn a_post_it_from_a_friend_cant_be_replaced_or_peeled_but_can_travel_on_unread() {
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    o.receive_note(Some("비밀".into()));
    assert!(!o.attach_note("덮어쓰기"));
    assert!(!o.peel_note());
    assert_eq!(o.note_to_send().as_deref(), Some("비밀"));
    // 포스트잇 없는 받은 물건엔 내가 붙일 수 있다
    let mut o = Object::new(ObjectKind::Can, 800.0, 1040.0, 1.0, 1);
    o.receive_note(None);
    assert_eq!(o.postit(), Postit::None);
    assert!(o.attach_note("다음 사람에게"));
}

#[test]
fn roster_keeps_ten_own_and_twenty_foreign_dropping_the_oldest() {
    let mut r = Roster::default();
    for id in 0..MAX_OWN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Own), None);
    }
    assert_eq!(r.own_count(), 10);
    assert_eq!(r.add(100, Origin::Own), Some(0));
    assert_eq!(r.add(101, Origin::Own), Some(1));
    assert_eq!(r.own_count(), 10);
    for id in 200..200 + MAX_FOREIGN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Foreign), None);
    }
    assert_eq!(r.add(300, Origin::Foreign), Some(200));
    // 받은 물건이 내 몫을 밀어내지 않는다
    assert_eq!(r.own_count(), 10);
    assert_eq!(r.foreign_count(), 20);
    // 치우거나 던져 보낸 만큼 자리가 난다
    assert!(r.remove(100));
    assert!(!r.remove(100));
    assert_eq!(r.add(102, Origin::Own), None);
    r.clear();
    assert_eq!((r.own_count(), r.foreign_count()), (0, 0));
}

#[test]
fn toss_budget_allows_fifty_at_once_then_fifty_a_minute() {
    let mut b = TossBudget::default();
    for _ in 0..50 {
        assert!(b.can_spend(0.0));
        b.spend(0.0);
    }
    assert!(!b.can_spend(0.0));
    assert!(!b.can_spend(1.0));
    assert!(b.can_spend(1.2));
    b.spend(1.2);
    assert!(!b.can_spend(1.2));
    assert!(b.can_spend(61.2));
}

#[test]
fn thrown_objects_wait_in_an_inbox_under_the_same_cap() {
    let mut r = Roster::default();
    let mut q: TossInbox<u32> = TossInbox::default();
    // 비어 있으면 아무것도 치우지 않고 20개까지 기다린다
    for i in 0..MAX_FOREIGN_OBJECTS as u32 {
        assert_eq!(q.push(i, &mut r), None);
    }
    assert_eq!(q.len(), MAX_FOREIGN_OBJECTS);
    // 넘치면 가장 오래 기다린 것이 빠진다
    assert_eq!(q.push(100, &mut r), None);
    assert_eq!(q.len(), MAX_FOREIGN_OBJECTS);
    assert_eq!(q.pop(), Some(1));
    q.clear();
    assert_eq!(q.pop(), None);
}

#[test]
fn queued_and_live_foreign_objects_share_the_twenty_and_the_live_ones_go_first() {
    let mut r = Roster::default();
    let mut q: TossInbox<u32> = TossInbox::default();
    for id in 0..18u64 {
        assert_eq!(r.add(id, Origin::Foreign), None);
    }
    r.add(500, Origin::Own);
    assert_eq!(q.push(0, &mut r), None);
    assert_eq!(q.push(1, &mut r), None);
    // 18 + 2 = 20 — 다음부터 가장 오래된 받은 물건이 비켜 준다 (내 물건은 그대로)
    assert_eq!(q.push(2, &mut r), Some(0));
    assert_eq!(r.foreign_count(), 17);
    assert_eq!(q.len(), 3);
    assert_eq!(r.own_count(), 1);
    // 줄이 다 창이 되어도 넘치지 않는다
    while q.pop().is_some() {
        assert_eq!(r.add(1000 + q.len() as u64, Origin::Foreign), None);
    }
    assert_eq!(r.foreign_count(), 20);
}

// ---- 구르기 ----

/// 던져서 멈출 때까지 지켜본다: (간 거리, 돈 각도 합, 멈춘 때까지 초)
fn tumble(o: &mut Object, w: &World) -> (f64, f64, f64) {
    let start = o.body.x;
    let (mut turned, mut t) = (0.0f64, 0.0f64);
    let mut limit = LoopLimit::new();
    while !o.is_asleep() {
        limit.step();
        let before = o.angle();
        o.tick(DT, w, NONE, false);
        // 잠들 때 각도를 한 바퀴 안으로 접으므로 그때는 세지 않는다
        if !o.is_asleep() {
            turned += (o.angle() - before).abs();
        }
        t += DT;
        assert!(t < 12.0, "{:?} never settled", o.kind);
    }
    ((o.body.x - start).abs(), turned, t)
}

fn thrown(kind: ObjectKind, w: &World, x: f64, v: (f64, f64), seed: u64) -> Object {
    let mut o = Object::new(kind, x, 800.0, 1.0, seed);
    fling(&mut o, w, (x, 760.0), v);
    o
}

fn wide() -> World {
    World::new(vec![], vec![Rect::new(0.0, 0.0, 12000.0, 1040.0)], 96.0)
}

#[test]
fn a_throw_spins_in_the_direction_it_goes_while_flying() {
    let w = wide();
    let mut right = thrown(ObjectKind::Can, &w, 3000.0, (1500.0, -300.0), 1);
    let mut left = thrown(ObjectKind::Can, &w, 3000.0, (-1500.0, -300.0), 1);
    assert!(right.spin() > 0.0 && left.spin() < 0.0, "{} {}", right.spin(), left.spin());
    let (a0, b0) = (right.angle(), left.angle());
    for _ in 0..10 {
        right.tick(DT, &w, NONE, false);
        left.tick(DT, &w, NONE, false);
    }
    assert!(right.angle() - a0 > 30.0, "{}", right.angle());
    assert!(left.angle() - b0 < -30.0, "{}", left.angle());
}

#[test]
fn harder_throws_spin_faster_and_turn_more() {
    let w = wide();
    let mut soft = thrown(ObjectKind::Can, &w, 3000.0, (500.0, -100.0), 1);
    let mut hard = thrown(ObjectKind::Can, &w, 3000.0, (1400.0, -100.0), 1);
    assert!(hard.spin() > soft.spin() && soft.spin() > 0.0, "{} {}", hard.spin(), soft.spin());
    let (_, soft_turned, _) = tumble(&mut soft, &w);
    let (_, hard_turned, _) = tumble(&mut hard, &w);
    assert!(hard_turned > soft_turned, "{hard_turned} {soft_turned}");
}

#[test]
fn after_landing_it_keeps_rolling_forward_then_friction_stops_it() {
    let w = wide();
    let mut o = thrown(ObjectKind::Can, &w, 3000.0, (1400.0, 0.0), 1);
    // 바닥에 처음 닿아 구르기 시작할 때까지 날아간다
    let mut t = 0.0;
    let mut limit = LoopLimit::new();
    while !o.is_rolling() {
        limit.step();
        o.tick(DT, &w, NONE, false);
        t += DT;
        assert!(t < 6.0, "never touched down");
    }
    let (x0, a0) = (o.body.x, o.angle());
    assert!(o.body.vx > 0.0 && o.spin() > 0.0, "vx {} spin {}", o.body.vx, o.spin());
    let mut last_v = o.body.vx;
    let mut t = 0.0;
    let mut limit = LoopLimit::new();
    while o.body.vx > 0.0 {
        limit.step();
        o.tick(DT, &w, NONE, false);
        assert!(o.body.vx <= last_v, "friction only slows it");
        last_v = o.body.vx;
        t += DT;
        assert!(t < 10.0, "friction never stopped it");
    }
    assert!(o.body.x - x0 > 150.0, "rolled {}", o.body.x - x0);
    assert!(o.angle() - a0 > 180.0, "turned {}", o.angle() - a0);
    tumble(&mut o, &w);
    assert_eq!(o.spin(), 0.0);
    assert_eq!(o.body.vx, 0.0);
}

#[test]
fn every_kind_settles_at_a_natural_angle_and_goes_back_to_sleep() {
    let w = wide();
    let mut seen_tipped = false;
    for kind in ObjectKind::ALL {
        for seed in 0..8 {
            for v in [(1500.0, -500.0), (-900.0, -100.0), (300.0, -900.0)] {
                let mut o = thrown(kind, &w, 6000.0, v, seed);
                tumble(&mut o, &w);
                assert!(o.is_asleep());
                assert_eq!((o.spin(), o.body.vx, o.body.vy), (0.0, 0.0, 0.0), "{kind:?}");
                assert_eq!(o.body.y, 1040.0, "{kind:?}");
                let a = o.angle();
                assert!((-180.0..180.0).contains(&a), "{kind:?} {a}");
                seen_tipped |= a.abs() > 5.0;
                // 잠든 뒤엔 아무것도 변하지 않는다 (창도 웹뷰도 안 건드린다)
                let p = o.pivot();
                for _ in 0..200 {
                    assert!(o.tick(DT, &w, NONE, false).is_none());
                    assert!(o.is_asleep());
                }
                assert_eq!((o.angle(), o.pivot()), (a, p));
            }
        }
    }
    assert!(seen_tipped, "something should end up lying down");
}

#[test]
fn a_gentle_drop_only_wobbles_a_little_and_comes_back() {
    let w = floor();
    for kind in ObjectKind::ALL {
        // 살며시: 거의 안 움직이고 놓거나, 천천히 끌다 놓거나, 그냥 떨어뜨리거나
        let mut clicked = Object::new(kind, 800.0, 700.0, 1.0, 1);
        clicked.pointer_down(800.0, 660.0);
        clicked.tick(DT, &w, NONE, false);
        clicked.pointer_up();
        let mut slow = Object::new(kind, 800.0, 700.0, 1.0, 1);
        fling(&mut slow, &w, (800.0, 660.0), (120.0, 40.0));
        let mut fell = Object::new(kind, 800.0, 100.0, 1.0, 1);
        for o in [&mut clicked, &mut slow, &mut fell] {
            assert_eq!(o.spin(), 0.0, "{kind:?}");
            let mut most = 0.0f64;
            let mut t = 0.0;
            let mut limit = LoopLimit::new();
            while !o.is_asleep() {
                limit.step();
                o.tick(DT, &w, NONE, false);
                most = most.max(o.angle().abs());
                t += DT;
                assert!(t < 5.0);
            }
            assert!(most <= 4.0, "{kind:?} wobbled {most}");
            assert_eq!(o.angle(), 0.0, "{kind:?}");
        }
    }
}

#[test]
fn a_landing_wobbles_at_least_visibly() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Milk, 800.0, 900.0, 1.0, 1);
    let mut most = 0.0f64;
    let mut limit = LoopLimit::new();
    while !o.is_asleep() {
        limit.step();
        o.tick(DT, &w, NONE, false);
        most = most.max(o.angle().abs());
    }
    assert!(most > 0.1, "{most}");
}

#[test]
fn each_kind_rolls_its_own_way() {
    let w = wide();
    // 같은 몸짓으로 던져 보면
    let go = |kind, seed| tumble(&mut thrown(kind, &w, 2000.0, (1400.0, -200.0), seed), &w);
    let (can_far, can_turn, _) = go(ObjectKind::Can, 1);
    let (paper_far, paper_turn, _) = go(ObjectKind::Paper, 1);
    let (computer_far, computer_turn, _) = go(ObjectKind::Computer, 1);
    let (sock_far, sock_turn, _) = go(ObjectKind::Shirt, 1);
    let (stone_far, stone_turn, _) = go(ObjectKind::Stone, 1);
    // 캔이 가장 멀리 구른다
    for (name, far) in [("paper", paper_far), ("computer", computer_far), ("sock", sock_far), ("stone", stone_far)] {
        assert!(can_far > far, "can {can_far} vs {name} {far}");
    }
    // 돌멩이는 무겁지만 동그래서 데굴 구른다 — 컴퓨터보다 많이, 캔보다 적게
    assert!(stone_turn > computer_turn && stone_turn < can_turn, "stone {stone_turn}");
    // 컴퓨터는 거의 안 구른다
    assert!(computer_turn < 120.0, "computer turned {computer_turn}");
    assert!(computer_turn * 4.0 < can_turn, "{computer_turn} vs {can_turn}");
    // 티셔츠는 철퍼덕 — 캔보다 훨씬 덜 돈다
    assert!(sock_turn * 2.0 < can_turn, "sock {sock_turn} can {can_turn}");
    assert!(paper_turn > sock_turn, "paper {paper_turn} sock {sock_turn}");
}

#[test]
fn crumpled_paper_is_irregular_a_can_is_not() {
    let w = wide();
    let run_of = |kind, seed| {
        let mut o = thrown(kind, &w, 3000.0, (1400.0, -200.0), seed);
        tumble(&mut o, &w);
        (o.body.x, o.angle())
    };
    let papers: Vec<_> = (0..6).map(|s| run_of(ObjectKind::Paper, s)).collect();
    assert!(papers.iter().any(|p| (p.0 - papers[0].0).abs() > 1.0 || (p.1 - papers[0].1).abs() > 1.0), "{papers:?}");
    let cans: Vec<_> = (0..3).map(|s| run_of(ObjectKind::Can, s)).collect();
    assert!(cans.iter().all(|c| *c == cans[0]), "{cans:?}");
}

#[test]
fn the_sprite_keeps_touching_the_surface_at_every_angle() {
    let w = wide();
    for kind in ObjectKind::ALL {
        let s = crate::tumble::shape_of(kind);
        let mut o = thrown(kind, &w, 3000.0, (1400.0, -200.0), 2);
        for _ in 0..600 {
            o.tick(DT, &w, NONE, false);
            let (px, py) = o.pivot();
            // 축 아래로 가장 낮은 점 = 몸 바닥 — 땅에선 표면에 닿고, 표면 아래로 내려가지 않는다
            assert!((py + s.drop_at(o.angle()) * o.scale() - o.body.y).abs() < 1e-9, "{kind:?}");
            assert!(o.body.y <= 1040.0, "{kind:?}");
            assert!((px - s.pivot_offset_x() * o.scale() - o.body.x).abs() < 1e-9, "{kind:?}");
        }
    }
}

#[test]
fn it_is_the_same_tumble_on_any_screen_scale() {
    let turned = |dpi: f64| {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 12000.0 * dpi, 1040.0 * dpi)], 96.0 * dpi);
        let mut o = Object::new(ObjectKind::Can, 2000.0 * dpi, 800.0 * dpi, dpi, 1);
        fling(&mut o, &w, (2000.0 * dpi, 760.0 * dpi), (1200.0 * dpi, -300.0 * dpi));
        tumble(&mut o, &w).1
    };
    let (a, b) = (turned(1.0), turned(2.0));
    assert!(a > 100.0 && (a - b).abs() / a < 0.12, "{a} vs {b}");
}

#[test]
fn rolling_off_a_window_keeps_turning_and_falls_naturally() {
    let w = world();
    let mut o = Object::new(ObjectKind::Can, 800.0, 400.0, 1.0, 5);
    run(&mut o, &w, 0.5);
    assert!(resting_on(&o, Owner::Window(1)));
    // 창 위로 오른쪽으로 던진다
    o.pointer_down(800.0, 390.0);
    for i in 1..=8 {
        o.tick(DT, &w, NONE, false);
        o.pointer_move(800.0 + 800.0 * i as f64 * DT, 390.0);
    }
    o.pointer_up();
    let mut fell_spinning = false;
    let mut t = 0.0;
    let mut limit = LoopLimit::new();
    while !o.is_asleep() {
        limit.step();
        let was_on_window = o.owner() == Some(Owner::Window(1));
        o.tick(DT, &w, NONE, false);
        fell_spinning |= was_on_window && o.owner().is_none() && o.spin() != 0.0;
        t += DT;
        assert!(t < 12.0);
    }
    assert!(fell_spinning);
    assert!(resting_on(&o, Owner::Ground(0)), "{:?}", o.owner());
}

#[test]
fn it_stops_at_the_screen_edge_and_a_wall_turns_the_spin_around() {
    let w = floor();
    // 땅에서 구르다 화면 끝: 넘어가지 않고 멈춘다
    let mut o = Object::new(ObjectKind::Can, 1880.0, 1040.0, 1.0, 1);
    run(&mut o, &w, 0.5);
    o.start_gust(1.0);
    tumble(&mut o, &w);
    assert!(o.body.x <= 1920.0 - o.half_w());
    // 날다가 벽: 튕겨 나오며 회전도 반대로
    let mut o = thrown(ObjectKind::Can, &w, 1800.0, (1500.0, -600.0), 1);
    assert!(o.spin() > 0.0);
    let mut t = 0.0;
    while o.body.vx > 0.0 && t < 2.0 {
        o.tick(DT, &w, NONE, false);
        t += DT;
    }
    assert!(o.body.vx < 0.0 && o.spin() < 0.0, "vx {} spin {}", o.body.vx, o.spin());
}

#[test]
fn grabbing_stops_the_spin_and_keeps_the_angle_it_is_at() {
    let w = wide();
    let mut o = thrown(ObjectKind::Can, &w, 3000.0, (1400.0, -300.0), 1);
    for _ in 0..15 {
        o.tick(DT, &w, NONE, false);
    }
    let a = o.angle();
    assert!(a != 0.0);
    o.pointer_down(o.body.x, o.body.y - 40.0);
    assert_eq!(o.spin(), 0.0);
    for _ in 0..10 {
        o.tick(DT, &w, NONE, false);
    }
    assert_eq!(o.angle(), a);
}

#[test]
fn caught_mid_spin_and_let_go_gently_it_lies_down_instead_of_balancing_on_a_corner() {
    let w = wide();
    for kind in ObjectKind::ALL {
        let s = crate::tumble::shape_of(kind);
        let mut o = thrown(kind, &w, 3000.0, (1400.0, -600.0), 1);
        for _ in 0..7 {
            o.tick(DT, &w, NONE, false);
        }
        let (x, y) = (o.body.x, o.body.y);
        o.pointer_down(x, y);
        o.tick(0.6, &w, NONE, false);
        assert_eq!(o.pointer_up(), Release::Throw);
        assert_eq!(o.spin(), 0.0, "{kind:?}");
        tumble(&mut o, &w);
        let a = o.angle();
        assert!(s.drop_at(a + 0.2) >= s.drop_at(a) && s.drop_at(a - 0.2) >= s.drop_at(a), "{kind:?} balancing at {a}");
    }
}

#[test]
fn an_object_from_a_friend_arrives_tumbling_from_its_speed() {
    let w = floor();
    let fast = Fling { height: 0.3, vx: 1200.0, vy: -100.0 };
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Left, fast, 1.0, 1);
    assert!(o.spin() > 0.0, "{}", o.spin());
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Right, fast, 1.0, 1);
    assert!(o.spin() < 0.0, "{}", o.spin());
    let slow = Fling { height: 0.3, vx: 60.0, vy: 30.0 };
    assert_eq!(Object::flown_in(ObjectKind::Can, &w, Side::Left, slow, 1.0, 1).spin(), 0.0);
    let up = Fling { height: 0.5, vx: 0.0, vy: 1000.0 };
    let mut o = Object::flown_in(ObjectKind::Paper, &w, Side::Up, up, 1.0, 1);
    assert!(o.spin() != 0.0);
    tumble(&mut o, &w);
}

// ---- 아래로 던지기: 동물과 같은 규칙 ----

fn crossed(o: &mut Object, w: &World, edges: ExitEdges) -> Option<Side> {
    for _ in 0..600 {
        if let Some(e) = o.tick(DT, w, edges, false) {
            return Some(e.side);
        }
    }
    None
}

#[test]
fn thrown_down_it_crosses_even_if_slow_at_the_floor_on_every_screen() {
    let down = ExitEdges { down: true, ..NONE };
    for dpi in [1.0, 1.5, 2.0] {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0 * dpi, 1040.0 * dpi)], 96.0 * dpi);
        let throw = |v: (f64, f64)| {
            let mut o = Object::new(ObjectKind::Can, 500.0 * dpi, 780.0 * dpi, dpi, 1);
            fling(&mut o, &w, (500.0 * dpi, 760.0 * dpi), (v.0 * dpi, v.1 * dpi));
            o
        };
        // 커서 1000 px/s: 바닥에서 900쯤 (옛 기준 1400 미만)
        assert_eq!(crossed(&mut throw((0.0, 1000.0)), &w, down), Some(Side::Down), "dpi {dpi}");
        // 옆으로, 위로, 살살 던지면 바닥에 앉는다
        for v in [(1500.0, 0.0), (0.0, -600.0), (0.0, 200.0)] {
            assert_eq!(crossed(&mut throw(v), &w, down), None, "dpi {dpi} {v:?}");
        }
    }
}

#[test]
fn thrown_down_with_the_body_already_over_the_taskbar_it_still_crosses() {
    let w = floor();
    let down = ExitEdges { down: true, ..NONE };
    let mut o = Object::new(ObjectKind::Can, 500.0, 980.0, 1.0, 1);
    fling(&mut o, &w, (500.0, 960.0), (0.0, 1000.0));
    assert!(o.body.y > 1040.0, "{}", o.body.y);
    assert_eq!(crossed(&mut o, &w, down), Some(Side::Down));
}

#[test]
fn it_crosses_at_the_floor_under_it_not_the_longest_monitors() {
    let down = ExitEdges { down: true, ..NONE };
    let up = ExitEdges { up: true, ..NONE };
    // 작업표시줄 있는 모니터 + 없는 모니터
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 300.0, 3840.0, 1080.0)], 96.0);
    let mut o = Object::new(ObjectKind::Can, 500.0, 780.0, 1.0, 1);
    fling(&mut o, &w, (500.0, 760.0), (0.0, 1000.0));
    assert_eq!(crossed(&mut o, &w, down), Some(Side::Down));
    // 위가 낮은 모니터도 그 위 끝으로
    let mut o = Object::new(ObjectKind::Can, 2500.0, 900.0, 1.0, 1);
    fling(&mut o, &w, (2500.0, 880.0), (0.0, -2500.0));
    assert_eq!(crossed(&mut o, &w, up), Some(Side::Up));
    // 쌓인 모니터의 위쪽 바닥은 끝이 아니다
    let w = World::new(vec![], vec![Rect::new(0.0, -1080.0, 1920.0, -40.0), Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0);
    let mut o = Object::new(ObjectKind::Can, 500.0, -300.0, 1.0, 1);
    fling(&mut o, &w, (500.0, -320.0), (0.0, 1000.0));
    assert_eq!(crossed(&mut o, &w, down), None);
    assert!(o.is_asleep());
    assert_eq!(o.body.y, -40.0);
}

#[test]
fn thrown_down_it_falls_past_window_tops_to_the_friend_below() {
    let w = world();
    let down = ExitEdges { down: true, ..NONE };
    let mut o = Object::new(ObjectKind::Can, 800.0, 200.0, 1.0, 1);
    fling(&mut o, &w, (800.0, 180.0), (0.0, 1000.0));
    assert_eq!(crossed(&mut o, &w, down), Some(Side::Down));
    // 아래 친구가 없으면 창 위에 앉는다
    let mut o = Object::new(ObjectKind::Can, 800.0, 200.0, 1.0, 1);
    fling(&mut o, &w, (800.0, 180.0), (0.0, 1000.0));
    assert_eq!(crossed(&mut o, &w, NONE), None);
    assert!(o.is_asleep() && o.body.y == 400.0, "{:?}", o.body);
}

// ---- 💩 똥 ----

const POOPS: [ObjectKind; 4] = [ObjectKind::Poop, ObjectKind::Pink, ObjectKind::Rainbow, ObjectKind::Pellets];

fn poop_on_floor(kind: ObjectKind) -> (Object, World) {
    let w = floor();
    let mut o = Object::new(kind, 900.0, 1000.0, 1.0, 3);
    run(&mut o, &w, 3.0);
    assert!(o.is_asleep(), "{kind:?}");
    (o, w)
}

#[test]
fn poops_are_foreign_validated_but_not_offered_in_the_menu() {
    assert_eq!(ObjectKind::TRASH.len() + POOPS.len() + ObjectKind::GIANTS.len(), ObjectKind::ALL.len());
    for k in POOPS {
        assert!(k.is_poop());
        assert!(!ObjectKind::TRASH.contains(&k), "{k:?}");
        assert_eq!(accept_toss(k.id(), None, None).map(|a| a.kind), Some(k));
    }
    for k in ObjectKind::TRASH {
        assert!(!k.is_poop(), "{k:?}");
    }
    // 위아래 틀린 이름은 모르는 종류다
    for bad in ["Poop", "poop ", "poops", "💩", "pellet"] {
        assert!(accept_toss(bad, None, None).is_none(), "{bad:?}");
    }
}

#[test]
fn poops_land_flat_and_the_wind_does_not_move_them() {
    for k in POOPS {
        let (mut o, w) = poop_on_floor(k);
        assert_eq!(o.angle(), 0.0, "{k:?}");
        assert_eq!(o.body.y, 1040.0);
        let x = o.body.x;
        o.start_gust(1.0);
        assert!(o.is_asleep(), "{k:?}");
        // 바람을 불게 두어도 그대로
        let mut t = 0.0;
        while t < GUST_EVERY_SECS.1 + 1.0 {
            o.tick(0.05, &w, NONE, true);
            t += 0.05;
        }
        assert_eq!(o.body.x, x, "{k:?}");
        assert!(o.is_asleep());
    }
}

#[test]
fn poops_take_no_post_its_even_from_a_friend() {
    for k in POOPS {
        let mut o = Object::new(k, 900.0, 1000.0, 1.0, 3);
        assert_eq!(o.postit(), Postit::Never);
        assert!(!o.attach_note("hello"));
        o.receive_note(Some("hi".into()));
        assert_eq!(o.postit(), Postit::Never);
        assert_eq!(o.read_note(), None);
        assert_eq!(o.note_to_send(), None);
        assert!(!o.has_sticker());
        let a = accept_toss(k.id(), None, Some("message")).unwrap();
        assert_eq!(a.note, None);
    }
    // 쓰레기는 그대로
    assert_eq!(accept_toss("can", None, Some("message")).unwrap().note.as_deref(), Some("message"));
}

#[test]
fn only_poops_fade_after_thirty_minutes_and_not_while_held() {
    let (mut o, w) = poop_on_floor(ObjectKind::Poop);
    let mut stone = Object::new(ObjectKind::Stone, 800.0, 1040.0, 1.0, 3);
    let mut t = 0.0;
    while t < crate::poop::LIFE_SECS - 10.0 {
        o.tick(0.05, &w, NONE, false);
        stone.tick(0.05, &w, NONE, false);
        t += 0.05;
    }
    assert!(!o.expired(), "still here near the end");
    let (x, y) = (o.body.x, o.body.y);
    o.pointer_down(x, y);
    o.tick(20.0, &w, NONE, false);
    stone.tick(20.0, &w, NONE, false);
    assert!(!o.expired(), "not out of the hand");
    o.pointer_up();
    assert!(o.expired());
    assert!(!stone.expired());
    assert!(crate::poop::LIFE_SECS >= 25.0 * 60.0 && crate::poop::LIFE_SECS <= 35.0 * 60.0);
}

#[test]
fn only_a_settled_poop_reports_its_spot() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Poop, 900.0, 800.0, 1.0, 3);
    assert_eq!(o.poop_spot(7), None, "still falling");
    run(&mut o, &w, 3.0);
    let s = o.poop_spot(7).unwrap();
    assert_eq!((s.id, s.x, s.y, s.owner), (7, 900.0, 1040.0, Owner::Ground(0)));
    o.pointer_down(900.0, 1000.0);
    assert_eq!(o.poop_spot(7), None, "held");
    let mut can = Object::new(ObjectKind::Can, 900.0, 1040.0, 1.0, 3);
    run(&mut can, &w, 3.0);
    assert_eq!(can.poop_spot(7), None, "not a poop");
}

#[test]
fn the_roster_counts_poops_on_their_own_and_they_never_push_out_trash() {
    let mut r = Roster::default();
    for id in 0..MAX_OWN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Own), None);
    }
    for id in 100..100 + MAX_FOREIGN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Foreign), None);
    }
    for id in 200..200 + MAX_POOPS as u64 {
        assert_eq!(r.add(id, Origin::Poop), None);
    }
    assert_eq!(r.poop_count(), MAX_POOPS);
    // 넘치면 가장 오래된 똥만
    assert_eq!(r.add(300, Origin::Poop), Some(200));
    assert_eq!((r.poop_count(), r.own_count(), r.foreign_count()), (MAX_POOPS, MAX_OWN_OBJECTS, MAX_FOREIGN_OBJECTS));
    assert!(r.remove(201));
    assert_eq!(r.poop_count(), MAX_POOPS - 1);
    r.clear();
    assert_eq!(r.poop_count(), 0);
}

#[test]
fn the_broom_sweeps_a_poop_along_and_lets_it_settle_again() {
    let (mut o, w) = poop_on_floor(ObjectKind::Poop);
    assert!(!o.sweep(0.0, 0.0), "the broom has to move");
    assert!(o.sweep(600.0, 0.0));
    assert!(o.body.vx > 0.0 && o.body.vy < 0.0, "{:?}", o.body);
    assert!(!o.sweep(600.0, 0.0), "already flying");
    run(&mut o, &w, 4.0);
    assert!(o.is_asleep());
    assert!(o.body.x > 900.0 + 30.0, "{}", o.body.x);
    assert_eq!(o.body.y, 1040.0);
    assert!(o.sweep(-60.0, 0.0), "a slow stroke still moves it");
    assert!(o.body.vx <= -SWEEP_MIN_VX);
    // 쓰레기는 쓸리지 않고, 든 것도 쓸리지 않는다
    let mut can = Object::new(ObjectKind::Can, 900.0, 1040.0, 1.0, 3);
    run(&mut can, &w, 3.0);
    assert!(!can.sweep(600.0, 0.0));
    let (mut held, _) = poop_on_floor(ObjectKind::Poop);
    held.pointer_down(900.0, 1000.0);
    assert!(!held.sweep(600.0, 0.0));
}

#[test]
fn a_poop_swept_at_a_linked_edge_flies_across_and_at_a_closed_edge_stays() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Poop, 1880.0, 1000.0, 1.0, 3);
    run(&mut o, &w, 3.0);
    assert!(o.sweep(900.0, 0.0));
    let mut exit = None;
    for _ in 0..200 {
        exit = o.tick(DT, &w, ALL, false);
        if exit.is_some() {
            break;
        }
    }
    let exit = exit.expect("crossed");
    assert_eq!(exit.side, Side::Right);
    assert!(exit.fling.vx > 0.0);
    // 받는 쪽: 코어의 평소 통로로 들어와 내려앉는다
    let a = accept_toss(ObjectKind::Poop.id(), Some(exit.fling), None).unwrap();
    let mut got = Object::flown_in(a.kind, &w, Side::Left, a.fling, 1.0, 9);
    assert_eq!(got.postit(), Postit::Never);
    run(&mut got, &w, 4.0);
    assert!(got.is_asleep());
    // 친구가 없는 끝에선 튕겨 돌아와 그 화면에 남는다
    let mut o = Object::new(ObjectKind::Poop, 1880.0, 1000.0, 1.0, 3);
    run(&mut o, &w, 3.0);
    assert!(o.sweep(900.0, 0.0));
    assert!(run(&mut o, &w, 4.0).is_none());
    assert_eq!(o.body.y, 1040.0);
}

#[test]
fn a_smaller_drawing_still_sits_on_the_floor() {
    let w = World::new(Vec::new(), vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0);
    let (mut big, mut small) = (Object::new(ObjectKind::Poop, 800.0, 900.0, 1.0, 1), Object::new(ObjectKind::Poop, 800.0, 900.0, 1.0, 1));
    small.set_scale(0.25);
    for _ in 0..600 {
        big.tick(DT, &w, NONE, false);
        small.tick(DT, &w, NONE, false);
    }
    assert_eq!((big.body.y, big.angle), (small.body.y, small.angle), "same place on the floor");
    let drop = crate::tumble::shape_of(ObjectKind::Poop).drop_at(big.angle);
    assert!((big.body.y - big.pivot().1 - drop).abs() < 1e-6);
    // 축이 그만큼 바닥에 가까워야 그림 아래쪽이 바닥에 닿는다
    assert!((small.body.y - small.pivot().1 - drop * 0.25).abs() < 1e-6);
}

#[test]
fn volley_sends_ten_at_once_on_a_full_budget() {
    let mut b = TossBudget::default();
    let mut v = Volley::default();
    v.request();
    assert_eq!(v.release(&mut b, 0.0), 10);
    assert!(v.is_done());
    // 손으로 던지는 것과 같은 예산을 쓴다: 남은 건 40개
    for _ in 0..40 {
        assert!(b.can_spend(0.0));
        b.spend(0.0);
    }
    assert!(!b.can_spend(0.0), "the volley spends the same budget as a hand throw");
}

#[test]
fn volley_over_the_budget_waits_for_its_turn() {
    let mut b = TossBudget::default();
    for _ in 0..47 {
        b.spend(0.0);
    }
    let mut v = Volley::default();
    v.request();
    assert_eq!(v.release(&mut b, 0.0), 3);
    assert_eq!(v.release(&mut b, 1.0), 0);
    assert_eq!(v.release(&mut b, 1.2), 1);
    assert!(!v.is_done());
    assert_eq!(v.release(&mut b, 1000.0), 6);
    assert!(v.is_done());
}

#[test]
fn up_to_five_volleys_pile_up_and_no_more() {
    let mut b = TossBudget::default();
    let mut v = Volley::default();
    for _ in 0..50 {
        v.request();
    }
    assert_eq!(v.release(&mut b, 0.0), 50, "five volleys go out at once on a full budget");
    assert!(v.is_done());
    for _ in 0..50 {
        v.request();
    }
    assert_eq!(v.release(&mut b, 1000.0), 50);
}

#[test]
fn volley_pieces_are_random_trash_and_never_poop() {
    let mut rng = fastrand::Rng::with_seed(7);
    let mut kinds = std::collections::HashSet::new();
    for _ in 0..200 {
        let (kind, fling) = volley_piece(&mut rng);
        assert!(ObjectKind::TRASH.contains(&kind) && !kind.is_poop());
        assert_eq!(fling.sanitized(), Some(fling), "the friend's app would have changed it");
        assert!(fling.vx.abs() >= 100.0, "it must clear the edge it enters from");
        kinds.insert(kind.id());
    }
    assert_eq!(kinds.len(), ObjectKind::TRASH.len());
}

/// 던져서 바닥에 처음 닿은 뒤 다시 떠오른 가장 큰 높이
fn rebound(kind: ObjectKind, w: &World) -> f64 {
    let floor = w.bounds.bottom;
    let mut o = thrown(kind, w, 300.0, (900.0, 600.0), 5);
    let mut limit = LoopLimit::new();
    while !o.is_rolling() && o.body.y < floor {
        limit.step();
        o.tick(DT, w, NONE, false);
    }
    let mut top = 0.0f64;
    while !o.is_asleep() {
        limit.step();
        o.tick(DT, w, NONE, false);
        top = top.max(floor - o.body.y);
    }
    top
}

#[test]
fn a_thrown_poop_splats_then_rolls_heavily() {
    let w = floor();
    let (milk_went, _, _) = tumble(&mut thrown(ObjectKind::Milk, &w, 300.0, (900.0, -200.0), 5), &w);
    for k in POOPS {
        assert!(rebound(k, &w) < 12.0, "{k:?} bounced {}", rebound(k, &w));
        let (went, turned, _) = tumble(&mut thrown(k, &w, 300.0, (900.0, -200.0), 5), &w);
        assert!(went < milk_went * 0.75, "{k:?} went {went}, milk {milk_went}");
        // 미끄러지기만 하지 않고 눈에 띄게 굴러 넘어간다
        assert!(turned >= 180.0, "{k:?} turned {turned}");
    }
}

#[test]
fn trash_is_half_size_and_poop_keeps_its_own_size() {
    for k in ObjectKind::TRASH {
        let o = Object::new(k, 500.0, 500.0, 2.0, 1);
        assert_eq!(o.scale(), 0.5, "{k:?}");
        assert_eq!(o.half_w(), OBJECT_LOGICAL_SIZE * 2.0 * 0.5 * HALF_WIDTH_RATIO, "{k:?}");
        assert_eq!(o.hit_box().0, o.half_w(), "{k:?}");
    }
    for k in POOPS {
        assert_eq!(Object::new(k, 500.0, 500.0, 1.0, 1).scale(), 1.0, "{k:?}");
    }
}

#[test]
fn half_size_trash_stops_at_the_wall_where_its_body_ends() {
    let w = floor();
    let mut o = thrown(ObjectKind::Stone, &w, 300.0, (-1400.0, 0.0), 1);
    let mut closest = f64::MAX;
    let mut limit = LoopLimit::new();
    while !o.is_asleep() {
        limit.step();
        o.tick(DT, &w, NONE, false);
        closest = closest.min(o.body.x);
    }
    assert!((closest - o.half_w()).abs() < 1.0, "{closest} vs {}", o.half_w());
}

#[test]
fn a_window_fits_the_drawn_size_but_never_goes_below_the_smallest_known_good_one() {
    for k in ObjectKind::ALL {
        let s = crate::tumble::shape_of(k);
        assert_eq!(s.window_side_at(1.0), s.window_side(), "{k:?}");
        for scale in [0.2, 0.5, 0.9] {
            let side = s.window_side_at(scale);
            assert!(side >= s.window_side().min(56.0), "{k:?}");
            assert!(side <= s.window_side_at(1.0), "{k:?}");
            assert!(s.reach * (96.0 / 128.0) * scale < side / 2.0, "{k:?}");
        }
    }
    let can = crate::tumble::shape_of(ObjectKind::Paper);
    assert!(can.window_side_at(0.5) < can.window_side_at(1.0));
}

#[test]
fn the_shirt_still_travels_as_sock_so_older_friends_can_catch_it() {
    assert_eq!(ObjectKind::Shirt.wire_id(), "sock");
    assert_eq!(ObjectKind::Shirt.id(), "shirt");
    assert_eq!(ObjectKind::from_id("sock"), Some(ObjectKind::Shirt));
    assert_eq!(ObjectKind::from_id("shirt"), Some(ObjectKind::Shirt));
    for k in ObjectKind::ALL {
        assert_eq!(ObjectKind::wire_kind(k.wire_id(), k.look()), Some(k));
        if k != ObjectKind::Shirt && k.look().is_none() {
            assert_eq!(k.wire_id(), k.id());
        }
    }
    assert!(!ObjectKind::Shirt.label().contains("양말"));
}

#[test]
fn a_new_umbrella_falls_and_rests_flipped_with_the_handle_up() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Umbrella, 800.0, 700.0, 1.0, 3);
    assert_eq!(o.angle(), 0.0);
    let mut limit = LoopLimit::new();
    while !o.is_asleep() {
        limit.step();
        o.tick(DT, &w, NONE, false);
    }
    assert!(o.angle().abs() < 60.0, "rests at {}", o.angle());
}

#[test]
fn rainbow_poop_travels_as_poop_and_the_green_one_is_gone() {
    assert!(ObjectKind::Rainbow.is_poop());
    assert_eq!(ObjectKind::Rainbow.id(), "rainbow");
    assert_eq!(ObjectKind::Rainbow.wire_id(), "poop", "older friends get a plain poop");
    assert_eq!(ObjectKind::from_id("rainbow"), Some(ObjectKind::Rainbow));
    // 옛 친구가 보낸 초록 똥은 그냥 똥이 된다
    assert_eq!(ObjectKind::from_id("dropping"), Some(ObjectKind::Poop));
    assert!(ObjectKind::ALL.iter().all(|k| k.id() != "dropping"));
    assert_eq!(ObjectKind::wire_kind(ObjectKind::Rainbow.wire_id(), Some("rainbow")), Some(ObjectKind::Rainbow));
    assert_eq!(ObjectKind::wire_kind("poop", None), Some(ObjectKind::Poop));
    // 똥이 아닌 것에 붙은 색 표시, 모르는 색은 무시한다
    assert_eq!(ObjectKind::wire_kind("can", Some("rainbow")), Some(ObjectKind::Can));
    assert_eq!(ObjectKind::wire_kind("poop", Some("plaid")), Some(ObjectKind::Poop));
}

#[test]
fn pink_poop_travels_as_poop_with_its_colour() {
    assert!(ObjectKind::Pink.is_poop());
    assert_eq!(ObjectKind::Pink.id(), "pink");
    assert_eq!(ObjectKind::Pink.wire_id(), "poop", "older friends get a plain poop");
    assert_eq!(ObjectKind::Pink.look(), Some("pink"));
    assert_eq!(ObjectKind::Rainbow.look(), Some("rainbow"));
    assert_eq!(ObjectKind::Poop.look(), None);
    for k in ObjectKind::ALL {
        assert_eq!(ObjectKind::wire_kind(k.wire_id(), k.look()), Some(k), "{k:?}");
    }
}

// ---- 🪨💩 거대한 돌·똥 ----

/// 1920×1040 작업 영역의 바닥
fn giant(kind: ObjectKind, x: f64, y: f64) -> Object {
    let mut o = Object::new(kind, x, y, 1.0, 7);
    o.set_scale(kind.scale_on(1.0, 1040.0));
    o
}

#[test]
fn giants_follow_the_trash_in_the_menu_but_never_fly_in_a_volley() {
    assert_eq!(ObjectKind::GIANTS, [ObjectKind::GiantStone, ObjectKind::GiantPoop, ObjectKind::GiantPink, ObjectKind::GiantRainbow]);
    // 메뉴엔 거대한 돌·똥 둘만 (똥은 꺼낼 때 갈색·핑크·무지개 중 하나 — menu_pick)
    let menu: Vec<ObjectKind> = ObjectKind::MENU.to_vec();
    let want: Vec<ObjectKind> = ObjectKind::TRASH.iter().chain([ObjectKind::GiantStone, ObjectKind::GiantPoop].iter()).copied().collect();
    assert_eq!(menu, want);
    assert_eq!(ObjectKind::GiantStone.label(), "🪨 거대한 돌");
    assert_eq!(ObjectKind::GiantPoop.label(), "💩 거대한 똥");
    for k in ObjectKind::GIANTS {
        assert!(k.is_giant() && !k.is_poop(), "{k:?}");
        assert_eq!(ObjectKind::from_id(k.id()), Some(k));
    }
    assert!(ObjectKind::ALL.iter().filter(|k| k.is_giant()).count() == 4);
    let mut rng = fastrand::Rng::with_seed(4);
    for _ in 0..500 {
        assert!(!volley_piece(&mut rng).0.is_giant());
    }
}

#[test]
fn a_giant_is_fifteen_baby_cats_but_at_most_80_percent_of_the_screen_height() {
    // 아기 고양이 한 변 = 128 × 아기 배율 0.45 × 고양이 stage_size 0.91 = 52.416 논리 px
    let baby_cat = crate::brain::PET_LOGICAL_SIZE * crate::care::Stage::Baby.scale() * BABY_CAT_STAGE_SIZE;
    assert!((baby_cat - 52.416).abs() < 1e-9, "{baby_cat}");
    assert!((giant_side() - 15.0 * baby_cat).abs() < 1e-9);
    // 1080p 화면 (작업 영역 1040): 786 논리 px < 832 (80%)
    assert!((giant_scale(1040.0) * OBJECT_LOGICAL_SIZE - 786.24).abs() < 1e-6);
    // 낮은 화면은 80%로 줄인다
    assert!((giant_scale(700.0) * OBJECT_LOGICAL_SIZE - 560.0).abs() < 1e-6);
    for h in [300.0, 500.0, 768.0, 1040.0, 1400.0, 2100.0] {
        assert!(giant_scale(h) * OBJECT_LOGICAL_SIZE <= 0.8 * h + 1e-9, "{h}");
    }
    // 모르는 높이면 그냥 15배
    for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
        assert!((giant_scale(bad) * OBJECT_LOGICAL_SIZE - giant_side()).abs() < 1e-9, "{bad}");
    }
    // 크기 조절 창의 쓰레기 배율은 거대한 물건에 듣지 않는다 (똥은 싼 동물을 따른다)
    assert_eq!(ObjectKind::GiantStone.scale_on(1.5, 1040.0), giant_scale(1040.0));
    assert_eq!(ObjectKind::GiantPoop.scale_on(0.5, 700.0), giant_scale(700.0));
    assert_eq!(ObjectKind::Stone.scale_on(1.5, 1040.0), 0.75);
    assert_eq!(ObjectKind::Poop.scale_on(1.5, 1040.0), 1.0);
    // 처음 만들 때(화면을 알기 전)는 15배
    assert!((Object::new(ObjectKind::GiantStone, 0.0, 0.0, 1.0, 1).scale() * OBJECT_LOGICAL_SIZE - giant_side()).abs() < 1e-9);
}

#[test]
fn giants_are_too_heavy_for_the_wind_and_take_no_post_it() {
    let w = floor();
    for k in ObjectKind::GIANTS {
        let mut o = giant(k, 900.0, 1000.0);
        run(&mut o, &w, 4.0);
        assert!(o.is_asleep(), "{k:?}");
        assert_eq!(o.body.y, 1040.0);
        let x = o.body.x;
        o.start_gust(1.0);
        assert!(o.is_asleep(), "{k:?} moved by the wind");
        // 바람 시계가 몇 번 돌아도 그대로
        for _ in 0..(200.0 / DT) as usize {
            o.tick(DT, &w, NONE, true);
        }
        assert_eq!(o.body.x, x, "{k:?}");
        assert_eq!(o.postit(), Postit::Never, "{k:?}");
        assert!(!o.attach_note("hi"));
        // 쓰레기처럼 치울 때까지 남는다 (똥처럼 마르지 않는다)
        for _ in 0..((crate::poop::LIFE_SECS + 60.0) / 1.0) as usize {
            o.tick(1.0, &w, NONE, false);
        }
        assert!(!o.expired(), "{k:?}");
        assert!(o.poop_spot(1).is_none(), "{k:?}: pets don't sniff at it");
        // 받아도 포스트잇은 버린다
        assert_eq!(accept_toss(k.id(), None, Some("hello")).unwrap().note, None);
    }
}

#[test]
fn the_broom_sweeps_the_giant_poop_but_not_the_giant_stone() {
    let w = wide();
    let mut poop = giant(ObjectKind::GiantPoop, 3000.0, 1000.0);
    run(&mut poop, &w, 4.0);
    assert!(poop.sweep(600.0, 0.0));
    let mut stone = giant(ObjectKind::GiantStone, 3000.0, 1000.0);
    run(&mut stone, &w, 4.0);
    assert!(!stone.sweep(600.0, 0.0));
}

#[test]
fn a_thrown_giant_tumbles_lands_on_the_floor_inside_the_screen_and_sleeps() {
    let w = floor();
    for k in ObjectKind::GIANTS {
        for v in [(1500.0, -500.0), (-900.0, -100.0), (300.0, -900.0)] {
            let mut o = giant(k, 960.0, 800.0);
            fling(&mut o, &w, (960.0, 760.0), v);
            tumble(&mut o, &w);
            assert!(o.is_asleep(), "{k:?}");
            assert_eq!(o.body.y, 1040.0, "{k:?}");
            assert!(o.body.x - o.half_w() >= -1e-6 && o.body.x + o.half_w() <= 1920.0 + 1e-6, "{k:?} {}", o.body.x);
        }
        // 창 위에도 앉는다
        let ww = World::new(vec![WindowInfo { id: 1, rect: Rect::new(200.0, 600.0, 1700.0, 1000.0) }], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 96.0);
        let mut o = giant(k, 950.0, 100.0);
        run(&mut o, &ww, 5.0);
        assert!(resting_on(&o, Owner::Window(1)), "{k:?} {:?}", o.owner());
    }
}

#[test]
fn a_giant_travels_as_a_plain_stone_or_poop_with_a_giant_look() {
    assert_eq!(ObjectKind::GiantStone.id(), "giant_stone");
    assert_eq!(ObjectKind::GiantPoop.id(), "giant_poop");
    // 옛 버전 친구는 이름만 보고 보통 돌·똥으로 받는다
    assert_eq!((ObjectKind::GiantStone.wire_id(), ObjectKind::GiantStone.look()), ("stone", Some("giant")));
    assert_eq!((ObjectKind::GiantPoop.wire_id(), ObjectKind::GiantPoop.look()), ("poop", Some("giant")));
    assert_eq!(ObjectKind::from_id("stone"), Some(ObjectKind::Stone));
    assert_eq!(ObjectKind::from_id("poop"), Some(ObjectKind::Poop));
    // 새 버전 친구는 거대하게 받는다
    assert_eq!(ObjectKind::wire_kind("stone", Some("giant")), Some(ObjectKind::GiantStone));
    assert_eq!(ObjectKind::wire_kind("poop", Some("giant")), Some(ObjectKind::GiantPoop));
    // 다른 것에 붙은 giant 표시는 무시한다
    assert_eq!(ObjectKind::wire_kind("can", Some("giant")), Some(ObjectKind::Can));
    assert_eq!(ObjectKind::wire_kind("stone", Some("pink")), Some(ObjectKind::Stone));
}

#[test]
fn a_giant_window_grows_with_its_drawing() {
    for k in ObjectKind::GIANTS {
        let s = crate::tumble::shape_of(k);
        let scale = giant_scale(1040.0);
        let side = s.window_side_at(scale);
        assert!(s.reach * (96.0 / 128.0) * scale < side / 2.0, "{k:?}");
        assert!(side > s.window_side() * 5.0, "{k:?} {side}");
        assert!(s.window_side_at(scale * 0.5) < side);
    }
}

// ---- 거대한 물건: 투명한 곳의 클릭은 아래로 지나간다 ----

#[test]
fn a_shape_contains_points_inside_its_hull_only() {
    let s = crate::tumble::shape_of(ObjectKind::GiantStone);
    assert!(s.contains(s.pivot.0, s.pivot.1));
    // 칸 모서리는 비어 있다
    for (x, y) in [(1.0, 1.0), (127.0, 1.0), (1.0, 127.0), (127.0, 127.0), (64.0, 2.0)] {
        assert!(!s.contains(x, y), "{x} {y}");
    }
    for &(x, y) in s.hull {
        // 꼭짓점 살짝 안쪽은 안, 바깥쪽은 밖
        let (dx, dy) = (x - s.pivot.0, y - s.pivot.1);
        assert!(s.contains(x - dx * 0.05, y - dy * 0.05), "{x} {y}");
        assert!(!s.contains(x + dx * 0.08, y + dy * 0.08), "{x} {y}");
    }
}

/// 그림 칸 좌표 (cx, cy)가 지금 화면 어디에 그려지나 (웹뷰처럼 축을 중심으로 시계 방향으로 돌린다)
fn on_screen(o: &Object, (cx, cy): (f64, f64)) -> (f64, f64) {
    let s = crate::tumble::shape_of(o.kind);
    let k = (96.0 / 128.0) * o.scale() * o.dpi();
    let (dx, dy) = ((cx - s.pivot.0) * k, (cy - s.pivot.1) * k);
    let (sin, cos) = o.angle().to_radians().sin_cos();
    let (px, py) = o.pivot();
    (px + dx * cos - dy * sin, py + dx * sin + dy * cos)
}

#[test]
fn a_flying_giant_lets_every_click_through_so_its_window_does_not_flicker() {
    // 사용자: 거대한 돌·똥이 친구 화면에 나타났다 깜빡 사라졌다 다시 나타난다 — Windows에서 클릭 통과를 바꿀 때마다
    // 창 스타일(layered)이 바뀌고 창 전체를 다시 그린다. 날아가는 동안은 바꾸지 않는다 (모양이 커서 밑을 계속 지나간다)
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    for k in ObjectKind::GIANTS {
        let mut o = Object::flown_in(k, &w, Side::Right, Fling { height: 0.3, vx: 600.0, vy: -300.0 }, 1.0, 1);
        o.set_scale(k.scale_on(1.0, 1040.0));
        o.tick(DT, &w, NONE, false);
        let (px, py) = o.pivot();
        assert!(o.passes_clicks_at(px, py), "{k:?} in the air");
        // 내려앉으면 다시 그림 위 클릭은 잡힌다
        run(&mut o, &w, 8.0);
        let (px, py) = o.pivot();
        assert!(!o.passes_clicks_at(px, py), "{k:?} on the ground");
    }
}

#[test]
fn only_a_few_giants_stay_on_screen_at_once() {
    // 친구 여럿이 거대한 것을 던지면 화면만 한 창이 쌓여 앱이 느려진다: MAX_GIANTS를 넘으면 가장 오래된 것이 뿅
    assert_eq!(MAX_GIANTS, 3);
    assert_eq!(giant_to_drop(&[]), None);
    assert_eq!(giant_to_drop(&[7, 9]), None);
    assert_eq!(giant_to_drop(&[7, 9, 11]), Some(7), "a 4th is coming: the oldest goes");
}

#[test]
fn clicks_pass_through_a_giant_only_where_nothing_is_drawn() {
    let w = floor();
    for k in ObjectKind::GIANTS {
        let mut o = giant(k, 900.0, 1000.0);
        o.set_dpi(1.0);
        run(&mut o, &w, 4.0);
        assert!(o.is_asleep());
        let s = crate::tumble::shape_of(k);
        // 무게중심(그림 한가운데)은 잡히고, 칸 위 구석(투명)은 지나간다
        let (px, py) = o.pivot();
        assert!(!o.passes_clicks_at(px, py), "{k:?}");
        let (x, y) = on_screen(&o, (4.0, 4.0));
        assert!(o.passes_clicks_at(x, y), "{k:?}");
        // 가장 오른쪽 끝: 4칸 안쪽은 잡히고 4칸 바깥은 지나간다
        let &(rx, ry) = s.hull.iter().max_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
        let (x, y) = on_screen(&o, (rx + 4.0, ry));
        assert!(o.passes_clicks_at(x, y), "{k:?}");
        let (x, y) = on_screen(&o, (rx - 4.0, ry));
        assert!(!o.passes_clicks_at(x, y), "{k:?}");
        // 들고 있는 동안엔 절대 지나가지 않는다 (놓칠 수 있으니)
        o.pointer_down(px, py);
        let (x, y) = on_screen(&o, (4.0, 4.0));
        assert!(!o.passes_clicks_at(x, y), "{k:?}");
    }
}

#[test]
fn the_click_through_follows_the_turned_drawing() {
    let w = floor();
    let mut o = giant(ObjectKind::GiantStone, 900.0, 300.0);
    o.set_dpi(1.5);
    // 던져서 돌고 있는 중에도 그린 자리 그대로
    fling(&mut o, &w, (900.0, 260.0), (1800.0, -900.0));
    let s = crate::tumble::shape_of(o.kind);
    let mut checked = 0;
    for _ in 0..80 {
        o.tick(DT, &w, NONE, false);
        if o.angle().abs() < 10.0 {
            continue;
        }
        for &(x, y) in s.hull {
            let (dx, dy) = (x - s.pivot.0, y - s.pivot.1);
            let inside = on_screen(&o, (x - dx * 0.1, y - dy * 0.1));
            let outside = on_screen(&o, (x + dx * 0.15, y + dy * 0.15));
            assert!(!o.passes_clicks_at(inside.0, inside.1), "angle {}", o.angle());
            assert!(o.passes_clicks_at(outside.0, outside.1), "angle {}", o.angle());
        }
        checked += 1;
    }
    assert!(checked > 5, "it should have turned");
}

#[test]
fn small_objects_never_pass_clicks_through() {
    let w = floor();
    for k in ObjectKind::ALL.into_iter().filter(|k| !k.is_giant()) {
        let mut o = Object::new(k, 900.0, 1000.0, 1.0, 3);
        run(&mut o, &w, 4.0);
        let (x, y) = on_screen(&o, (2.0, 2.0));
        assert!(!o.passes_clicks_at(x, y), "{k:?}");
    }
}

// ---- 🤹 동물이 들고 던지기 (brain_juggle.rs) ----

#[test]
fn a_resting_object_is_a_spot_to_pick_up_but_not_while_held_flying_or_carried() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 300.0, 900.0, 1.0, 1);
    assert_eq!(o.spot(4), None, "still falling");
    run(&mut o, &w, 3.0);
    let s = o.spot(4).expect("lying on the floor");
    assert_eq!((s.id, s.x, s.y, s.owner, s.giant), (4, o.body.x, 1040.0, Owner::Ground(0), false));
    assert_eq!(s.half_w, o.half_w());
    let mut g = Object::new(ObjectKind::GiantStone, 900.0, 1000.0, 1.0, 1);
    run(&mut g, &w, 3.0);
    assert!(g.spot(5).unwrap().giant);
    o.carry_at(500.0, 700.0);
    assert!(o.is_carried() && !o.is_asleep() && !o.in_flight());
    assert_eq!(o.spot(4), None);
    o.pointer_down(500.0, 690.0);
    assert_eq!(o.spot(4), None, "held");
}

#[test]
fn a_carried_object_rides_upright_in_the_hands_without_physics_and_never_dries_up() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Poop, 300.0, 900.0, 1.0, 1);
    run(&mut o, &w, 3.0);
    o.carry_at(600.0, 700.0);
    let (_, h) = o.hit_box();
    assert_eq!(o.body.x, 600.0);
    assert!(o.body.y > 700.0 && o.body.y - 700.0 <= 0.2 * h + 1e-9, "두 손이 아랫부분을 감싼다: {}", o.body.y);
    assert_eq!(o.angle(), 0.0);
    let at = o.body;
    run(&mut o, &w, crate::poop::LIFE_SECS + 5.0);
    assert_eq!(o.body.x, at.x);
    assert_eq!(o.body.y, at.y, "들고 있는 동안엔 떨어지지 않는다");
    assert!(!o.expired(), "들고 있는 동안엔 말라 사라지지 않는다");
    // 거대한 것도 손이 바닥 근처를 받친다 (몇 px만 아래)
    let mut g = Object::new(ObjectKind::GiantPoop, 900.0, 1000.0, 1.0, 1);
    g.carry_at(900.0, 600.0);
    assert!(g.body.y - 600.0 <= 10.0 + 1e-9);
}

#[test]
fn a_launched_object_flies_like_a_thrown_one_and_crosses_a_friend_edge() {
    let w = floor();
    let mut o = Object::new(ObjectKind::Can, 1800.0, 900.0, 1.0, 1);
    o.carry_at(1800.0, 800.0);
    o.launch(900.0, -500.0);
    assert!(o.in_flight() && !o.is_carried());
    assert!(o.spin() != 0.0, "던진 것처럼 돈다");
    let mut exit = None;
    for _ in 0..200 {
        if let Some(e) = o.tick(DT, &w, ALL, false) {
            exit = Some(e);
            break;
        }
    }
    assert_eq!(exit.map(|e| e.side), Some(Side::Right));
    // 놓친 것은 그냥 떨어진다 (던진 것이 아니라 화면 끝을 넘지 않는다)
    let mut o = Object::new(ObjectKind::Can, 400.0, 900.0, 1.0, 1);
    o.carry_at(400.0, 800.0);
    o.let_fall();
    assert!(!o.is_carried() && !o.in_flight());
    run(&mut o, &w, 3.0);
    assert_eq!(o.owner(), Some(Owner::Ground(0)));
    // 사용자가 잡고 있는 물건은 동물 손으로 가지 않는다
    let mut o = Object::new(ObjectKind::Can, 400.0, 900.0, 1.0, 1);
    o.pointer_down(400.0, 890.0);
    o.carry_at(100.0, 100.0);
    o.launch(100.0, -100.0);
    assert!(o.is_held() && o.body.x == 400.0);
}

#[test]
fn a_delivered_object_is_checked_like_a_toss_and_never_a_giant() {
    assert_eq!(accept_carry("can", None), Some(ObjectKind::Can));
    assert_eq!(accept_carry("sock", None), Some(ObjectKind::Shirt));
    assert_eq!(accept_carry("poop", Some("rainbow")), Some(ObjectKind::Rainbow));
    assert_eq!(accept_carry("poop", Some("weird")), Some(ObjectKind::Poop));
    for (kind, look) in [("stone", Some("giant")), ("poop", Some("giant")), ("giant_stone", None), ("giant_poop", None), ("cat", None), ("", None), ("../x", None)] {
        assert_eq!(accept_carry(kind, look), None, "{kind} {look:?}");
    }
}

#[test]
fn about_one_in_ten_of_a_friends_downward_throws_drops_on_to_the_friend_below() {
    // 사용자: 친구가 던진 물건이 바닥을 향하면 10%쯤은 아래 친구 화면까지 (나머지는 착지)
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let down = ExitEdges { down: true, ..NONE };
    let dives = |from: Side, vy: f64| {
        (0..400u64)
            .filter(|&seed| {
                let mut o = Object::flown_in(ObjectKind::Can, &w, from, Fling { height: 0.3, vx: 200.0, vy }, 1.0, seed);
                crossed(&mut o, &w, down) == Some(Side::Down)
            })
            .count()
    };
    for from in [Side::Right, Side::Up] {
        let n = dives(from, 600.0);
        assert!((20..=60).contains(&n), "{n} from {from:?}");
    }
    assert_eq!(dives(Side::Right, -500.0), 0);
}

// ---- 📌 작업표시줄에서 꺼낸 앱 아이콘 장난감 (taskbar.rs) ----

#[test]
fn app_icon_toys_are_not_on_the_wire_the_menu_or_the_volley_list() {
    let toy = ObjectKind::AppIcon;
    assert!(toy.is_toy() && !toy.is_poop() && !toy.is_giant() && !toy.is_sweepable() && !toy.takes_postit());
    assert!(!ObjectKind::ALL.contains(&toy) && !ObjectKind::TRASH.contains(&toy) && !ObjectKind::MENU.contains(&toy));
    assert!(ObjectKind::ALL.iter().all(|k| !k.is_toy()));
    // 친구가 그 이름을 보내도 받지 않는다 (던지기·배달 모두)
    for look in [None, Some("giant"), Some("pink")] {
        assert_eq!(ObjectKind::wire_kind(toy.id(), look), None);
        assert_eq!(accept_carry(toy.id(), look), None);
    }
    assert_eq!(ObjectKind::from_id(toy.id()), None);
    assert_eq!(accept_toss(toy.id(), None, None), None);
    // 크기 조절 창을 따르지 않는다
    assert_eq!(toy.scale_on(2.0, 1000.0), toy.base_scale());
    assert_eq!(Object::new(toy, 0.0, 0.0, 1.0, 1).postit(), Postit::Never);
}

#[test]
fn a_thrown_app_icon_toy_never_leaves_through_a_friend_edge_and_bounces_back() {
    let w = floor();
    for (vx, vy) in [(-3000.0, -400.0), (3000.0, -400.0), (0.0, -4000.0), (200.0, 3000.0)] {
        let mut o = Object::new(ObjectKind::AppIcon, 960.0, 600.0, 1.0, 3);
        o.launch(vx, vy);
        let mut t = 0.0;
        while t < 8.0 {
            assert!(o.tick(DT, &w, ALL, false).is_none(), "({vx}, {vy}) 친구에게 날아가지 않는다");
            assert!(o.body.x >= 0.0 && o.body.x <= 1920.0);
            t += DT;
        }
        assert!(o.owner().is_some(), "화면 안 바닥에 내려앉는다");
    }
    // 같은 던지기라도 보통 물건은 넘어간다 (연결된 끝)
    let mut can = Object::new(ObjectKind::Can, 960.0, 600.0, 1.0, 3);
    can.launch(-3000.0, -400.0);
    assert!(run_edges(&mut can, &w, ALL, 3.0).is_some());
}

fn run_edges(o: &mut Object, w: &World, edges: ExitEdges, secs: f64) -> Option<Exit> {
    let mut t = 0.0;
    while t < secs {
        if let Some(e) = o.tick(DT, w, edges, false) {
            return Some(e);
        }
        t += DT;
    }
    None
}

#[test]
fn app_icon_toys_fade_by_themselves_after_a_few_minutes_but_not_in_a_hand() {
    let w = floor();
    let mut o = Object::new(ObjectKind::AppIcon, 960.0, 1040.0, 1.0, 1);
    let mut can = Object::new(ObjectKind::Can, 960.0, 1040.0, 1.0, 1);
    let mut t = 0.0;
    while t < crate::taskbar::TOY_LIFE_SECS - 1.0 {
        o.tick(1.0, &w, NONE, false);
        can.tick(1.0, &w, NONE, false);
        t += 1.0;
    }
    assert!(!o.expired());
    o.carry_at(900.0, 800.0);
    o.tick(5.0, &w, NONE, false);
    assert!(!o.expired(), "흐물이가 들고 있는 동안은 기다린다");
    o.let_fall();
    assert!(o.expired());
    assert!(!can.expired(), "쓰레기는 저절로 사라지지 않는다");
    // 바닥에 놓이면 흐물이가 알아챈다 — 이 컴퓨터 것이라는 표시와 함께
    let mut toy = Object::new(ObjectKind::AppIcon, 960.0, 900.0, 1.0, 1);
    run(&mut toy, &w, 3.0);
    assert!(toy.spot(4).is_some_and(|s| s.local && !s.giant));
    let mut can = Object::new(ObjectKind::Can, 960.0, 900.0, 1.0, 1);
    run(&mut can, &w, 3.0);
    assert!(can.spot(5).is_some_and(|s| !s.local));
}

#[test]
fn only_one_toy_is_kept_and_it_is_not_counted_as_trash() {
    let mut r = Roster::default();
    assert_eq!(r.add(1, Origin::Toy), None);
    assert_eq!(r.add(2, Origin::Toy), Some(1), "한 번에 하나");
    assert_eq!(r.own_count(), 0);
    assert!(r.remove(2));
    assert!(!r.remove(2));
}

#[test]
fn clicks_pass_through_the_clear_parts_of_a_toy_window_to_the_taskbar() {
    let w = floor();
    let mut toy = Object::new(ObjectKind::AppIcon, 960.0, 900.0, 1.0, 1);
    run(&mut toy, &w, 3.0);
    assert!(toy.is_asleep());
    let (px, py) = toy.pivot();
    assert!(!toy.passes_clicks_at(px, py), "아이콘 위는 잡힌다");
    // 바닥 바로 밑 (작업표시줄 자리) — 창은 아직 덮고 있지만 클릭은 지나간다
    assert!(toy.passes_clicks_at(px, toy.body.y + 4.0));
    let mut can = Object::new(ObjectKind::Can, 960.0, 900.0, 1.0, 1);
    run(&mut can, &w, 3.0);
    assert!(!can.passes_clicks_at(can.pivot().0, can.body.y + 4.0), "작은 쓰레기는 그대로");
}

#[test]
fn a_giant_from_a_friend_never_bounces_straight_on_to_the_next_screen() {
    // 사용자: 친구가 거대한 똥을 받았는데 떨어지지 않고 화면 정중앙에서 깜빡깜빡만 한다 — 위아래로 이어진 두 화면 사이를
    // 0.1초마다 오갔다 (아래에서 몸 2.5배를 튀어 오르면 거대한 것은 화면 위를 넘는다. 옆에서 위로 던져져 오면 머리가 이미 위 끝이다)
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let all = ExitEdges { up: true, down: true, left: true, right: true };
    for k in ObjectKind::GIANTS {
        for from in [Side::Down, Side::Left, Side::Right, Side::Up] {
            for (vx, vy) in [(0.0, 0.0), (300.0, 800.0), (100.0, -900.0), (600.0, -1500.0)] {
                for seed in 0..5 {
                    let mut o = Object::flown_in(k, &w, from, Fling { height: 0.5, vx, vy }, 1.0, seed);
                    o.set_scale(k.scale_on(1.0, 1040.0));
                    let mut left = None;
                    for _ in 0..600 {
                        if let Some(e) = o.tick(DT, &w, all, false) {
                            left = Some(e.side);
                            break;
                        }
                    }
                    assert!(left.is_none() || left == Some(Side::Down), "{k:?} from {from:?} v=({vx},{vy}) left by {left:?}");
                }
            }
        }
    }
}

#[test]
fn small_things_from_below_still_pop_up_as_before() {
    let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
    let o = Object::flown_in(ObjectKind::Can, &w, Side::Down, Fling { height: 0.5, vx: 0.0, vy: 0.0 }, 1.0, 1);
    let g = THROW_GRAVITY_LOGICAL;
    let size = OBJECT_LOGICAL_SIZE * o.scale();
    assert!((o.body.vy + (2.0 * g * 2.5 * size).sqrt()).abs() < 1e-6, "{}", o.body.vy);
}

#[test]
fn a_giant_poop_from_the_menu_comes_out_brown_pink_or_rainbow() {
    // 사용자: 거대 똥 꺼낼 때 랜덤하게 무지개똥/핑크똥/갈색똥
    use ObjectKind::*;
    assert_eq!(GiantPoop.menu_pick(0.1), GiantPoop);
    assert_eq!(GiantPoop.menu_pick(0.5), GiantPink);
    assert_eq!(GiantPoop.menu_pick(0.9), GiantRainbow);
    assert_eq!(GiantStone.menu_pick(0.9), GiantStone, "others come out as they are");
    assert_eq!(Can.menu_pick(0.5), Can);
    for k in [GiantPink, GiantRainbow] {
        assert!(k.is_giant() && k.is_sweepable() && !k.takes_postit(), "{k:?}");
        assert!(ObjectKind::ALL.contains(&k) && !ObjectKind::MENU.contains(&k), "{k:?}");
        assert_eq!(ObjectKind::from_id(k.id()), Some(k));
        // 친구에게: 똥으로 보내고 look으로 색과 거대함을 알린다 (옛 버전은 그냥 똥)
        assert_eq!(k.wire_id(), "poop");
        assert_eq!(ObjectKind::wire_kind(k.wire_id(), k.look()), Some(k));
    }
    assert_eq!((GiantPink.id(), GiantRainbow.id()), ("giant_pink", "giant_rainbow"));
    assert_eq!(ObjectKind::wire_kind("poop", Some("giant")), Some(GiantPoop));
}
