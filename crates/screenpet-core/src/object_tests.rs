use super::*;
use crate::geom::{Rect, WindowInfo};
use crate::visit::{ExitEdges, Fling, Side};
use crate::surfaces::Owner;
use crate::world::World;

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
        assert!(k.id().chars().all(|c| c.is_ascii_lowercase()));
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
    let mut o = Object::new(ObjectKind::Sock, 800.0, 1040.0, 1.0, 7);
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
fn gusts_come_every_30_to_90_seconds_and_only_a_short_way() {
    for seed in 0..12 {
        let w = floor();
        let mut o = Object::new(ObjectKind::Can, 960.0, 1040.0, 1.0, seed);
        run(&mut o, &w, 0.5);
        let start = o.body.x;
        let mut t = 0.0;
        while o.is_asleep() {
            o.tick(DT, &w, NONE, true);
            t += DT;
            assert!(t <= GUST_EVERY_SECS.1 + 1.0, "no gust in time");
        }
        assert!(t >= GUST_EVERY_SECS.0 - 1.0, "gust too soon: {t}");
        // 그동안 혼자 움직였다가 다시 잠든다
        let mut t = 0.0;
        while !o.is_asleep() {
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
    let (sock_turned, _) = gust_of(ObjectKind::Sock);
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
        let mut o = Object::flown_in(ObjectKind::Sock, &w, side, Fling { height: 0.5, vx: 300.0, vy: -50.0 }, 1.0, 1);
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
fn roster_keeps_five_own_and_twenty_foreign_dropping_the_oldest() {
    let mut r = Roster::default();
    for id in 0..MAX_OWN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Own), None);
    }
    assert_eq!(r.own_count(), 5);
    assert_eq!(r.add(100, Origin::Own), Some(0));
    assert_eq!(r.add(101, Origin::Own), Some(1));
    assert_eq!(r.own_count(), 5);
    for id in 200..200 + MAX_FOREIGN_OBJECTS as u64 {
        assert_eq!(r.add(id, Origin::Foreign), None);
    }
    assert_eq!(r.add(300, Origin::Foreign), Some(200));
    // 받은 물건이 내 몫을 밀어내지 않는다
    assert_eq!(r.own_count(), 5);
    assert_eq!(r.foreign_count(), 20);
    // 치우거나 던져 보낸 만큼 자리가 난다
    assert!(r.remove(100));
    assert!(!r.remove(100));
    assert_eq!(r.add(102, Origin::Own), None);
    r.clear();
    assert_eq!((r.own_count(), r.foreign_count()), (0, 0));
}

#[test]
fn toss_budget_allows_ten_a_minute_per_link() {
    let mut b = TossBudget::default();
    for _ in 0..10 {
        assert!(b.can_spend(0.0));
        b.spend(0.0);
    }
    assert!(!b.can_spend(0.0));
    assert!(!b.can_spend(5.0));
    assert!(b.can_spend(6.0));
    b.spend(6.0);
    assert!(!b.can_spend(6.0));
    assert!(b.can_spend(66.0));
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
    while !o.is_asleep() {
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
    while !o.is_rolling() {
        o.tick(DT, &w, NONE, false);
        t += DT;
        assert!(t < 6.0, "never touched down");
    }
    let (x0, a0) = (o.body.x, o.angle());
    assert!(o.body.vx > 0.0 && o.spin() > 0.0, "vx {} spin {}", o.body.vx, o.spin());
    let mut last_v = o.body.vx;
    let mut t = 0.0;
    while o.body.vx > 0.0 {
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
            while !o.is_asleep() {
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
    while !o.is_asleep() {
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
    let (sock_far, sock_turn, _) = go(ObjectKind::Sock, 1);
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
    // 양말은 철퍼덕 — 캔보다 훨씬 덜 돈다
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
            assert!((py + s.drop_at(o.angle()) - o.body.y).abs() < 1e-9, "{kind:?}");
            assert!(o.body.y <= 1040.0, "{kind:?}");
            assert!((px - s.pivot_offset_x() - o.body.x).abs() < 1e-9, "{kind:?}");
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
    while !o.is_asleep() {
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
