//! 🚗 탈것 (vehicle.rs Garage + brain_vehicle.rs): 꺼내기·떨어져 서기, 걸어가 올라타기, 달리기, 내리기,
//! 급정거·쿵, 헬리콥터 뜨고 내려앉기, 트럭 짐칸, 펫을 탈것 위에 떨구기, 탈것 잡기·던지기, 친구 화면으로 몰고 나가기

use super::tests::{place, win, world};
use super::*;
use crate::test_util::{pet_cfg, DT};
use crate::vehicle::{self, Garage, VehicleEvent, VehicleKind, VehicleSpot};

/// 바닥(x)에 한참 쉬는 평범한 동물 (말풍선·연령대 몸짓 끔)
fn resting_pet(x: f64, seed: u64, w: &World) -> Pet {
    let mut p = Pet::new(pet_cfg(), x, 0.0, seed);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 1000.0, look: false });
    p.clock = 1000.0;
    p.hush_stage();
    p
}

/// 바닥(x)에 놓인 탈것 하나가 있는 차고 (다 떨어져 섬)
fn garage_with(kind: VehicleKind, x: f64, w: &World) -> (Garage, u64) {
    let mut g = Garage::new(7);
    let id = g.spawn(kind, x, 1040.0, 1.0).expect("room");
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..120 {
        g.tick(DT, w, &mut none);
    }
    (g, id)
}

/// 차고와 동물들을 n틱 함께 돌림 (게임 루프처럼: 자리 알림 → 동물 update → 차고 tick)
fn run_all(g: &mut Garage, pets: &mut Vec<Pet>, w: &World, n: usize) -> Vec<VehicleEvent> {
    let mut events = Vec::new();
    let mut spots: Vec<VehicleSpot> = Vec::new();
    for _ in 0..n {
        g.spots(&mut spots);
        for p in pets.iter_mut() {
            p.notice_vehicles(&spots, w);
            p.update(DT, w);
        }
        g.tick(DT, w, pets);
        events.extend(g.take_events());
    }
    events
}

#[test]
fn kinds_have_ids_labels_and_seats() {
    assert_eq!(VehicleKind::ALL.len(), 4);
    for k in VehicleKind::ALL {
        assert_eq!(VehicleKind::from_id(k.id()), Some(k));
        assert!(!k.label().is_empty());
    }
    assert_eq!(VehicleKind::from_id("tank"), None);
    assert!(VehicleKind::Heli.flies());
    assert!(!VehicleKind::Car.flies());
    assert_eq!(VehicleKind::Truck.bed_seats(), 2);
    assert_eq!(VehicleKind::Car.bed_seats(), 0);
    // 킥보드는 작고 서서 탐
    assert!(VehicleKind::Scooter.spec().w < VehicleKind::Car.spec().w);
    assert!(VehicleKind::Scooter.spec().standing);
}

#[test]
fn at_most_four_vehicles_on_a_screen() {
    let mut g = Garage::new(1);
    for i in 0..vehicle::MAX_VEHICLES {
        assert!(g.spawn(VehicleKind::Car, 300.0 + i as f64 * 250.0, 500.0, 1.0).is_some());
    }
    assert!(g.spawn(VehicleKind::Car, 100.0, 500.0, 1.0).is_none());
    let id = g.vehicles()[0].id();
    assert!(g.remove(id));
    assert!(g.spawn(VehicleKind::Heli, 100.0, 500.0, 1.0).is_some());
    g.clear();
    assert!(g.vehicles().is_empty());
}

#[test]
fn a_spawned_vehicle_falls_and_parks_on_the_ground() {
    let w = world(vec![]);
    let mut g = Garage::new(1);
    let id = g.spawn(VehicleKind::Car, 800.0, 200.0, 1.0).unwrap();
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..240 {
        g.tick(DT, &w, &mut none);
    }
    let v = g.get(id).unwrap();
    assert_eq!(v.y(), 1040.0);
    assert!(v.is_parked());
}

#[test]
fn a_parked_vehicle_rides_a_moving_window() {
    let mut w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
    let mut g = Garage::new(1);
    let id = g.spawn(VehicleKind::Car, 800.0, 300.0, 1.0).unwrap();
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..120 {
        g.tick(DT, &w, &mut none);
    }
    assert_eq!(g.get(id).unwrap().y(), 400.0);
    w.patch_windows(&[(1, crate::geom::Rect::new(600.0, 300.0, 1200.0, 800.0))]);
    g.tick(DT, &w, &mut none);
    let v = g.get(id).unwrap();
    assert_eq!((v.x(), v.y()), (900.0, 300.0));
}

#[test]
fn a_free_pet_walks_to_a_nearby_vehicle_and_drives_off() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut p = resting_pet(700.0, 3, &w);
    p.vehicle.eager = true;
    let mut pets = vec![p];
    run_all(&mut g, &mut pets, &w, 1);
    assert!(matches!(pets[0].state, State::ToVehicle { id: v, .. } if v == id), "{:?}", pets[0].state);
    run_all(&mut g, &mut pets, &w, 400);
    assert_eq!(pets[0].riding(), Some((id, 0)));
    assert_eq!(g.get(id).unwrap().driver(), Some(pets[0].social_key()));
    // 운전 중: 동물은 좌석에 붙어 탈것과 함께 움직임
    let x0 = g.get(id).unwrap().x();
    run_all(&mut g, &mut pets, &w, 120);
    let v = g.get(id).unwrap();
    assert!((v.x() - x0).abs() > 50.0, "drove {} → {}", x0, v.x());
    assert!((pets[0].body.x - v.seat_point(0, pets[0].cfg.size).0).abs() < 1.0);
    assert!(pets[0].body.y < v.y());
    assert!(v.pose().moving);
}

#[test]
fn riders_render_at_the_seat_with_their_legs_behind_the_door() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w)];
    assert!(g.board(id, &mut pets[0]));
    run_all(&mut g, &mut pets, &w, 2);
    let r = pets[0].render();
    assert_eq!(r.anim, Anim::Idle);
    assert!(r.seat > 0.1 && r.seat < 0.6, "{}", r.seat);
    // 내리면 다시 온몸이 보임
    pets[0].leave_vehicle(1.0);
    assert_eq!(pets[0].render().seat, 0.0);
}

#[test]
fn flyers_and_busy_pets_do_not_go_for_vehicles() {
    let w = world(vec![]);
    let (g, _) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut spots = Vec::new();
    g.spots(&mut spots);
    let mut bee = Pet::new(crate::test_util::flyer(), 800.0, 600.0, 2);
    bee.vehicle.eager = true;
    bee.notice_vehicles(&spots, &w);
    assert!(!matches!(bee.state, State::ToVehicle { .. }));
    let mut p = resting_pet(800.0, 2, &w);
    p.vehicle.eager = true;
    p.pointer_down(800.0, 1000.0);
    p.notice_vehicles(&spots, &w);
    assert!(!matches!(p.state, State::ToVehicle { .. }));
}

#[test]
fn the_ride_ends_and_the_driver_hops_out() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w)];
    assert!(g.board(id, &mut pets[0]));
    g.end_ride_soon(id);
    run_all(&mut g, &mut pets, &w, 240);
    assert_eq!(pets[0].riding(), None);
    assert!(g.get(id).unwrap().driver().is_none());
    assert!(g.get(id).unwrap().is_parked());
    // 내린 뒤엔 한동안 다시 타러 가지 않음
    let mut spots = Vec::new();
    g.spots(&mut spots);
    run_all(&mut g, &mut pets, &w, 120);
    assert!(!matches!(pets[0].state, State::ToVehicle { .. } | State::Ride { .. }));
}

/// 오른쪽으로 달리는 차 (운전 펫 = pets[0])와 그 앞 x에 선 동물 (pets[1])
fn car_toward_walker(crash: bool, walker_x: f64) -> (Garage, u64, Vec<Pet>, World) {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 400.0, &w);
    g.force_crash(Some(crash));
    let mut driver = resting_pet(400.0, 3, &w);
    driver.facing = 1.0;
    let walker = resting_pet(walker_x, 4, &w);
    assert!(g.board(id, &mut driver));
    g.face(id, 1.0);
    (g, id, vec![driver, walker], w)
}

#[test]
fn meeting_a_pet_screeches_to_a_stop_and_the_walker_jumps() {
    let (mut g, id, mut pets, w) = car_toward_walker(false, 900.0);
    let mut said_whoa = false;
    let mut events = Vec::new();
    for _ in 0..300 {
        events.extend(run_all(&mut g, &mut pets, &w, 1));
        said_whoa |= pets[1].speech == Some(Speech::Whoa);
        let _ = pets[1].take_speech();
    }
    assert!(events.iter().any(|e| matches!(e, VehicleEvent::Screech { .. })), "{events:?}");
    assert!(said_whoa);
    // 멈췄고 부딪치지 않았음 (운전 펫은 아직 타고 있음)
    assert_eq!(pets[0].riding(), Some((id, 0)));
    assert!(!events.iter().any(|e| matches!(e, VehicleEvent::Crash { .. })));
    let v = g.get(id).unwrap();
    assert!(v.x() + v.half_len() < 900.0 + 10.0, "stopped at {}", v.x());
}

#[test]
fn sometimes_it_cannot_stop_and_both_bounce_off() {
    let (mut g, id, mut pets, w) = car_toward_walker(true, 900.0);
    let mut events = Vec::new();
    for _ in 0..400 {
        events.extend(run_all(&mut g, &mut pets, &w, 1));
        if !events.is_empty() {
            break;
        }
    }
    assert!(events.iter().any(|e| matches!(e, VehicleEvent::Crash { .. })), "{events:?}");
    // 운전 펫은 튕겨 내리고, 걷던 펫은 튕겨 날아감
    assert_eq!(pets[0].riding(), None);
    assert!(matches!(pets[0].state, State::Air { .. }));
    assert!(matches!(pets[1].state, State::Air { .. }));
    assert!(pets[1].body.vx > 0.0);
    assert!(g.get(id).unwrap().driver().is_none());
}

#[test]
fn a_helicopter_takes_off_flies_and_lands_again() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Heli, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w)];
    assert!(g.board(id, &mut pets[0]));
    run_all(&mut g, &mut pets, &w, 240);
    let v = g.get(id).unwrap();
    assert!(v.y() < 1040.0 - 200.0, "flying at {}", v.y());
    assert!(v.pose().rotor);
    assert_eq!(pets[0].riding(), Some((id, 0)));
    g.end_ride_soon(id);
    run_all(&mut g, &mut pets, &w, 900);
    let v = g.get(id).unwrap();
    assert_eq!(v.y(), 1040.0);
    assert!(v.is_parked());
    assert_eq!(pets[0].riding(), None);
}

#[test]
fn a_helicopter_rises_over_pets_instead_of_meeting_them() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Heli, 400.0, &w);
    let mut pets = vec![resting_pet(400.0, 3, &w), resting_pet(800.0, 4, &w)];
    assert!(g.board(id, &mut pets[0]));
    let events = run_all(&mut g, &mut pets, &w, 600);
    assert!(!events.iter().any(|e| matches!(e, VehicleEvent::Screech { .. } | VehicleEvent::Crash { .. })));
    assert_eq!(pets[1].riding(), None);
    assert!(matches!(pets[1].state, State::Idle { .. }));
}

#[test]
fn a_truck_waits_for_friends_to_hop_into_the_bed() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Truck, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w), resting_pet(700.0, 4, &w), resting_pet(1100.0, 5, &w)];
    assert!(g.board(id, &mut pets[0]));
    assert_eq!(pets[0].speech, Some(Speech::HopIn));
    run_all(&mut g, &mut pets, &w, 400);
    assert_eq!(pets[1].riding().map(|r| r.0), Some(id));
    assert_eq!(pets[2].riding().map(|r| r.0), Some(id));
    let seats: Vec<u8> = pets[1..].iter().map(|p| p.riding().unwrap().1).collect();
    assert!(seats.contains(&1) && seats.contains(&2));
    // 짐칸은 앞모습으로 앉음
    assert_eq!(pets[1].render().anim, Anim::SitFront);
}

#[test]
fn a_sudden_stop_can_throw_bed_riders_forward() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Truck, 400.0, &w);
    g.force_crash(Some(false));
    g.force_bed_fling(true);
    let mut pets = vec![resting_pet(400.0, 3, &w), resting_pet(400.0, 4, &w), resting_pet(1000.0, 5, &w)];
    assert!(g.board(id, &mut pets[0]));
    assert!(g.board(id, &mut pets[1]));
    assert_eq!(pets[1].riding(), Some((id, 1)));
    g.face(id, 1.0);
    let mut events = Vec::new();
    for _ in 0..400 {
        events.extend(run_all(&mut g, &mut pets, &w, 1));
        if pets[1].riding().is_none() {
            break;
        }
    }
    assert!(events.iter().any(|e| matches!(e, VehicleEvent::Screech { .. })));
    assert_eq!(pets[1].riding(), None);
    assert!(pets[1].body.vx > 0.0, "thrown forward");
}

#[test]
fn dropping_a_pet_on_a_vehicle_seats_it() {
    let w = world(vec![]);
    let (mut g, car) = garage_with(VehicleKind::Car, 600.0, &w);
    let truck = g.spawn(VehicleKind::Truck, 1300.0, 1040.0, 1.0).unwrap();
    let mut none: Vec<Pet> = Vec::new();
    g.tick(DT, &w, &mut none);
    // 끌어다 차 위에 놓음
    let mut a = resting_pet(200.0, 3, &w);
    a.pointer_down(200.0, 1000.0);
    a.pointer_move(400.0, 900.0);
    a.pointer_move(600.0, 990.0);
    a.pointer_up(600.0, 990.0);
    assert!(g.offer_drop(&mut a));
    assert_eq!(a.riding(), Some((car, 0)));
    // 운전석이 찬 차에는 못 탐
    let mut b = resting_pet(200.0, 4, &w);
    b.body.x = 600.0;
    b.body.y = 990.0;
    b.state = State::Air { anim: Anim::Fall };
    b.standing = None;
    assert!(!g.offer_drop(&mut b));
    // 운전석이 찬 트럭이면 짐칸으로
    let mut c = resting_pet(1300.0, 5, &w);
    assert!(g.board(truck, &mut c));
    b.body.x = 1260.0;
    b.body.y = 1000.0;
    assert!(g.offer_drop(&mut b));
    assert_eq!(b.riding(), Some((truck, 1)));
    // 탈것 밖에 놓으면 아무 일 없음
    let mut d = resting_pet(200.0, 6, &w);
    d.state = State::Air { anim: Anim::Fall };
    d.standing = None;
    d.body.y = 700.0;
    assert!(!g.offer_drop(&mut d));
}

#[test]
fn grabbing_a_ridden_vehicle_throws_the_riders_off_and_it_can_be_thrown() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w)];
    assert!(g.board(id, &mut pets[0]));
    run_all(&mut g, &mut pets, &w, 30);
    let (x, y) = (g.get(id).unwrap().x(), g.get(id).unwrap().y() - 20.0);
    assert!(g.pointer_down(id, x, y, &mut pets));
    assert!(g.is_held());
    assert_eq!(pets[0].riding(), None);
    assert!(matches!(pets[0].state, State::Air { .. }));
    for i in 1..=6 {
        g.pointer_move(x + 40.0 * i as f64, y - 30.0 * i as f64);
        g.tick(DT, &w, &mut pets);
    }
    g.pointer_up();
    assert!(!g.is_held());
    g.tick(DT, &w, &mut pets);
    let v = g.get(id).unwrap();
    assert!(!v.is_parked());
    run_all(&mut g, &mut pets, &w, 300);
    assert!(g.get(id).unwrap().is_parked());
}

#[test]
fn grabbing_the_driver_stops_the_vehicle() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, &w);
    let mut pets = vec![resting_pet(900.0, 3, &w)];
    assert!(g.board(id, &mut pets[0]));
    run_all(&mut g, &mut pets, &w, 60);
    let (px, py) = (pets[0].body.x, pets[0].body.y - 20.0);
    pets[0].pointer_down(px, py);
    pets[0].pointer_move(px + 30.0, py - 30.0);
    g.tick(DT, &w, &mut pets);
    assert!(g.get(id).unwrap().driver().is_none());
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..120 {
        g.tick(DT, &w, &mut none);
    }
    assert!(g.get(id).unwrap().is_parked());
}

#[test]
fn a_car_turns_around_at_a_window_edge_or_drives_off_it() {
    let w = world(vec![win(1, 500.0, 600.0, 1100.0, 900.0)]);
    let mut g = Garage::new(1);
    let id = g.spawn(VehicleKind::Car, 800.0, 500.0, 1.0).unwrap();
    let mut pets: Vec<Pet> = Vec::new();
    for _ in 0..60 {
        g.tick(DT, &w, &mut pets);
    }
    assert_eq!(g.get(id).unwrap().y(), 600.0);
    let mut d = resting_pet(800.0, 3, &w);
    d.standing = None;
    pets.push(d);
    assert!(g.board(id, &mut pets[0]));
    g.face(id, 1.0);
    let mut turned = false;
    let mut fell = false;
    for _ in 0..600 {
        run_all(&mut g, &mut pets, &w, 1);
        let v = g.get(id).unwrap();
        turned |= v.dir() < 0.0 && v.y() == 600.0;
        fell |= v.y() > 600.0;
        // 창 끝을 넘어 공중에 떠 있지 않음
        if v.is_grounded() && v.y() == 600.0 {
            assert!(v.x() <= 1100.0 + 1.0);
        }
    }
    assert!(turned || fell);
}

#[test]
fn an_own_driver_can_drive_out_to_a_friend_screen() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1600.0, &w);
    g.force_visit(true);
    let mut pets = vec![resting_pet(1600.0, 3, &w)];
    pets[0].set_exit_edges(false, true);
    assert!(g.board(id, &mut pets[0]));
    g.face(id, 1.0);
    let mut out = None;
    for _ in 0..600 {
        for e in run_all(&mut g, &mut pets, &w, 1) {
            if let VehicleEvent::DroveOut { side, kind, driver, .. } = e {
                out = Some((side, kind, driver));
            }
        }
        if out.is_some() {
            break;
        }
    }
    assert_eq!(out, Some((Side::Right, VehicleKind::Car, pets[0].social_key())));
    assert!(g.get(id).is_none(), "the car went along");
    assert_eq!(pets[0].take_exit(), Some(Side::Right));
    // 돌아오면 같은 탈것을 몰고 들어옴
    assert_eq!(g.take_parked_away(pets[0].social_key()), Some(VehicleKind::Car));
    assert_eq!(g.take_parked_away(pets[0].social_key()), None);
}

#[test]
fn a_guest_does_not_drive_my_vehicle_out() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1600.0, &w);
    g.force_visit(true);
    let mut pets = vec![resting_pet(1600.0, 3, &w)];
    pets[0].set_guest(Side::Left, 1000.0);
    pets[0].set_exit_edges(false, true);
    assert!(g.board(id, &mut pets[0]));
    g.face(id, 1.0);
    let events = run_all(&mut g, &mut pets, &w, 600);
    assert!(!events.iter().any(|e| matches!(e, VehicleEvent::DroveOut { .. })));
    assert!(g.get(id).is_some());
}

#[test]
fn an_arriving_pet_drives_in_on_its_vehicle() {
    let w = world(vec![]);
    let mut g = Garage::new(1);
    let mut p = Pet::new_entering(pet_cfg(), &w, Side::Left, 3);
    let id = g.arrive(VehicleKind::Scooter, &w, Side::Left, 1.0, &mut p).expect("room");
    assert_eq!(p.riding(), Some((id, 0)));
    let mut pets = vec![p];
    run_all(&mut g, &mut pets, &w, 120);
    let v = g.get(id).unwrap();
    assert!(v.x() > 100.0, "drove in to {}", v.x());
    assert_eq!(v.dir(), 1.0);
}

#[test]
fn a_foreign_vehicle_leaves_with_its_pet_or_expires() {
    let w = world(vec![]);
    let mut g = Garage::new(1);
    let mut p = Pet::new_entering(pet_cfg(), &w, Side::Left, 3);
    p.set_guest(Side::Left, 1000.0);
    let id = g.arrive(VehicleKind::Car, &w, Side::Left, 1.0, &mut p).unwrap();
    // 손님이 떠나면 그 탈것도 같이 사라짐 (앱이 알림)
    assert_eq!(g.guest_left(p.social_key()), Some(id));
    assert!(g.get(id).is_none());
    // 주인 없이 남은 남의 탈것은 한참 뒤 사라짐
    let mut q = Pet::new_entering(pet_cfg(), &w, Side::Left, 4);
    q.set_guest(Side::Left, 1000.0);
    let id = g.arrive(VehicleKind::Car, &w, Side::Left, 1.0, &mut q).unwrap();
    q.leave_vehicle(1.0);
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..((vehicle::FOREIGN_LIFE_SECS + 2.0) / 0.5) as usize {
        g.tick(0.5, &w, &mut none);
    }
    assert!(g.get(id).is_none());
    assert!(g.take_expired().contains(&id));
}

/// 바닥(x)의 탈것을 잡아 오른쪽(dir)으로 휙 던짐
fn throw_vehicle(g: &mut Garage, id: u64, w: &World, dir: f64) {
    let mut none: Vec<Pet> = Vec::new();
    let (x, y) = (g.get(id).unwrap().x(), g.get(id).unwrap().y() - 20.0);
    assert!(g.pointer_down(id, x, y, &mut none));
    for i in 1..=6 {
        g.pointer_move(x + dir * 60.0 * i as f64, y - 20.0 * i as f64);
        g.tick(DT, w, &mut none);
    }
    g.pointer_up();
}

#[test]
fn a_thrown_vehicle_flies_over_to_a_friend_at_that_edge() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1500.0, &w);
    g.set_throw_edges(false, true);
    throw_vehicle(&mut g, id, &w, 1.0);
    let mut none: Vec<Pet> = Vec::new();
    let mut out = None;
    for _ in 0..240 {
        g.tick(DT, &w, &mut none);
        for e in g.take_events() {
            if let VehicleEvent::ThrownOut { id: gone, kind, side, fling } = e {
                out = Some((gone, kind, side, fling));
            }
        }
        if out.is_some() {
            break;
        }
    }
    let (gone, kind, side, fling) = out.expect("flew out");
    assert_eq!((gone, kind, side), (id, VehicleKind::Car, Side::Right));
    assert!(fling.vx > 0.0);
    assert!(g.get(id).is_none(), "it left this screen");
    // 친구 쪽: 반대편 끝으로 날아 들어와 떨어져 섬 (받은 쪽 것이 됨)
    let mut h = Garage::new(9);
    let got = h.catch_thrown(kind, &w, Side::Left, fling, 1.0).expect("room");
    let v = h.get(got).unwrap();
    assert!(v.x() < 400.0 && !v.is_parked() && !v.is_foreign());
    for _ in 0..600 {
        h.tick(DT, &w, &mut none);
    }
    let v = h.get(got).unwrap();
    assert!(v.is_parked() && v.x() > 100.0, "{}", v.x());
}

#[test]
fn without_a_friend_there_a_thrown_vehicle_stays_on_screen() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1500.0, &w);
    g.set_throw_edges(true, false);
    throw_vehicle(&mut g, id, &w, 1.0);
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..600 {
        g.tick(DT, &w, &mut none);
        assert!(!g.take_events().iter().any(|e| matches!(e, VehicleEvent::ThrownOut { .. })));
    }
    assert!(g.get(id).unwrap().is_parked());
}

#[test]
fn a_full_garage_does_not_catch_and_odd_flings_are_refused() {
    let w = world(vec![]);
    let mut g = Garage::new(1);
    let ok = crate::visit::Fling { height: 0.5, vx: 600.0, vy: -200.0 };
    assert!(g.catch_thrown(VehicleKind::Heli, &w, Side::Up, ok, 1.0).is_none(), "only side edges");
    assert!(g.catch_thrown(VehicleKind::Heli, &w, Side::Right, crate::visit::Fling { vx: f64::NAN, ..ok }, 1.0).is_none());
    for _ in 0..vehicle::MAX_VEHICLES {
        assert!(g.catch_thrown(VehicleKind::Scooter, &w, Side::Right, ok, 1.0).is_some());
    }
    assert!(g.catch_thrown(VehicleKind::Scooter, &w, Side::Right, ok, 1.0).is_none());
}

#[test]
fn a_foreign_vehicle_is_not_thrown_on_to_another_friend() {
    let w = world(vec![]);
    let mut g = Garage::new(1);
    let mut p = Pet::new_entering(pet_cfg(), &w, Side::Left, 3);
    p.set_guest(Side::Left, 1000.0);
    let id = g.arrive(VehicleKind::Car, &w, Side::Left, 1.0, &mut p).unwrap();
    p.leave_vehicle(1.0);
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..240 {
        g.tick(DT, &w, &mut none);
    }
    g.set_throw_edges(true, true);
    throw_vehicle(&mut g, id, &w, 1.0);
    for _ in 0..600 {
        g.tick(DT, &w, &mut none);
        assert!(!g.take_events().iter().any(|e| matches!(e, VehicleEvent::ThrownOut { .. })));
    }
    assert!(g.get(id).is_some());
}
