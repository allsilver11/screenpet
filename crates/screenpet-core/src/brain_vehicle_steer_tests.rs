//! 🎮🚗 조작 모드로 탈것 몰기 (brain_vehicle.rs steering + vehicle.rs 조종): ←/→ 운전, 키 없으면 멈춰 기다림 (타기가 끝나지 않음),
//! 반대 방향키 = 멈췄다 돌아섬, ↑ 빵빵, Space 폴짝 내림, 🚁 방향키로 위아래 좌우, 창 끝은 그대로 몰고 떨어짐,
//! 친구가 연결된 끝은 늘 나감, 점프해서 탈것 위에 내려앉으면 올라탐

use super::tests::{place, win, world};
use super::control_moves::ControlInput;
use super::*;
use crate::test_util::{pet_cfg, DT};
use crate::vehicle::{Garage, VehicleEvent, VehicleKind, VehicleSpot};

/// 바닥(x)에 쉬는 조작 모드 동물
fn controlled_pet(x: f64, w: &World) -> Pet {
    let mut p = Pet::new(pet_cfg(), x, 0.0, 3);
    place(&mut p, w, Owner::Ground(0), x, State::Idle { remaining: 1000.0, look: false });
    p.clock = 1000.0;
    p.hush_stage();
    p.set_control(true);
    p
}

/// 바닥(x)에 서 있는 탈것 하나
fn garage_with(kind: VehicleKind, x: f64, y: f64, w: &World) -> (Garage, u64) {
    let mut g = Garage::new(7);
    let id = g.spawn(kind, x, y, 1.0).expect("room");
    let mut none: Vec<Pet> = Vec::new();
    for _ in 0..120 {
        g.tick(DT, w, &mut none);
    }
    (g, id)
}

/// 키를 누른 채 n틱 (게임 루프처럼: 키 → 자리 알림 → 동물 update → 차고 tick)
fn drive(g: &mut Garage, pets: &mut [Pet], w: &World, keys: ControlInput, n: usize) -> Vec<VehicleEvent> {
    let mut events = Vec::new();
    let mut spots: Vec<VehicleSpot> = Vec::new();
    for _ in 0..n {
        g.spots(&mut spots);
        for p in pets.iter_mut() {
            p.control_keys(keys, DT);
            p.notice_vehicles(&spots, w);
            p.update(DT, w);
        }
        g.tick(DT, w, pets);
        events.extend(g.take_events());
    }
    events
}

const NONE: ControlInput = ControlInput { left: false, right: false, up: false, down: false, jump: false, run: false, special: false };
const RIGHT: ControlInput = ControlInput { right: true, ..NONE };
const LEFT: ControlInput = ControlInput { left: true, ..NONE };
const UP: ControlInput = ControlInput { up: true, ..NONE };
const DOWN: ControlInput = ControlInput { down: true, ..NONE };
const JUMP: ControlInput = ControlInput { jump: true, ..NONE };

#[test]
fn arrow_keys_drive_the_car_and_letting_go_parks_it_with_the_pet_still_aboard() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(900.0, &w)];
    assert!(g.board(id, &mut pets[0]));
    drive(&mut g, &mut pets, &w, NONE, 60);
    let x0 = g.get(id).unwrap().x();
    // 키를 안 누르면 저절로 달리지 않음
    assert!((g.get(id).unwrap().x() - 900.0).abs() < 1.0);
    drive(&mut g, &mut pets, &w, RIGHT, 90);
    let x1 = g.get(id).unwrap().x();
    assert!(x1 > x0 + 60.0, "drove right {x0} → {x1}");
    assert_eq!(g.get(id).unwrap().dir(), 1.0);
    // 놓으면 멈춰 서고, 타기 시간이 다 지나도 내리지 않음
    drive(&mut g, &mut pets, &w, NONE, (40.0 / DT) as usize);
    assert!(!g.get(id).unwrap().pose().moving);
    assert_eq!(pets[0].riding(), Some((id, 0)));
    // 반대 방향키: 돌아서 왼쪽으로
    let x2 = g.get(id).unwrap().x();
    drive(&mut g, &mut pets, &w, LEFT, 120);
    assert_eq!(g.get(id).unwrap().dir(), -1.0);
    assert!(g.get(id).unwrap().x() < x2 - 40.0);
    // Shift = 더 빨리
    let mut fast = vec![controlled_pet(900.0, &w)];
    let (mut g2, id2) = garage_with(VehicleKind::Car, 900.0, 1040.0, &w);
    assert!(g2.board(id2, &mut fast[0]));
    drive(&mut g2, &mut fast, &w, NONE, 60);
    drive(&mut g2, &mut fast, &w, ControlInput { run: true, ..RIGHT }, 90);
    assert!(g2.get(id2).unwrap().x() > x1 + 10.0, "boost {} vs {}", g2.get(id2).unwrap().x(), x1);
}

#[test]
fn space_hops_out_and_up_honks() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(900.0, &w)];
    assert!(g.board(id, &mut pets[0]));
    drive(&mut g, &mut pets, &w, NONE, 60);
    pets[0].speech = None;
    drive(&mut g, &mut pets, &w, UP, 1);
    assert_eq!(pets[0].speech, Some(Speech::Honk));
    drive(&mut g, &mut pets, &w, NONE, 1);
    drive(&mut g, &mut pets, &w, JUMP, 1);
    assert_eq!(pets[0].riding(), None);
    assert!(matches!(pets[0].state, State::Air { .. }));
    // 폴짝 내린 뒤 바로 다시 그 차에 내려앉아도 또 타지 않음
    drive(&mut g, &mut pets, &w, NONE, 40);
    assert_eq!(pets[0].riding(), None);
    assert!(g.get(id).unwrap().driver().is_none());
}

#[test]
fn a_steered_car_drives_off_a_window_edge_instead_of_turning() {
    let w = world(vec![win(1, 500.0, 600.0, 1100.0, 900.0)]);
    let (mut g, id) = garage_with(VehicleKind::Car, 800.0, 500.0, &w);
    assert_eq!(g.get(id).unwrap().y(), 600.0);
    let mut p = controlled_pet(800.0, &w);
    p.standing = None;
    let mut pets = vec![p];
    assert!(g.board(id, &mut pets[0]));
    drive(&mut g, &mut pets, &w, NONE, 40);
    drive(&mut g, &mut pets, &w, RIGHT, 300);
    let v = g.get(id).unwrap();
    assert_eq!(v.y(), 1040.0, "fell off the window onto the floor");
    assert_eq!(pets[0].riding(), Some((id, 0)));
}

#[test]
fn a_steered_car_stops_at_a_plain_wall_without_turning() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1600.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(1600.0, &w)];
    assert!(g.board(id, &mut pets[0]));
    drive(&mut g, &mut pets, &w, NONE, 40);
    drive(&mut g, &mut pets, &w, RIGHT, 400);
    let v = g.get(id).unwrap();
    assert_eq!(v.dir(), 1.0);
    assert!(v.x() > 1750.0, "{}", v.x());
}

#[test]
fn a_steered_driver_always_drives_out_at_a_friend_edge() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 1600.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(1600.0, &w)];
    pets[0].set_exit_edges(false, true);
    assert!(g.board(id, &mut pets[0]));
    drive(&mut g, &mut pets, &w, NONE, 40);
    let events = drive(&mut g, &mut pets, &w, RIGHT, 400);
    assert!(events.iter().any(|e| matches!(e, VehicleEvent::DroveOut { side: Side::Right, .. })));
    assert!(g.get(id).is_none());
}

#[test]
fn a_steered_helicopter_flies_where_the_arrows_point_and_lands_on_down() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Heli, 900.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(900.0, &w)];
    assert!(g.board(id, &mut pets[0]));
    // 키가 없으면 뜨지 않음
    drive(&mut g, &mut pets, &w, NONE, 90);
    assert_eq!(g.get(id).unwrap().y(), 1040.0);
    drive(&mut g, &mut pets, &w, UP, 90);
    let y1 = g.get(id).unwrap().y();
    assert!(y1 < 1000.0, "rose to {y1}");
    // 놓으면 그 자리에 떠 있음
    drive(&mut g, &mut pets, &w, NONE, 120);
    assert!((g.get(id).unwrap().y() - y1).abs() < 2.0);
    let x1 = g.get(id).unwrap().x();
    drive(&mut g, &mut pets, &w, LEFT, 90);
    assert!(g.get(id).unwrap().x() < x1 - 40.0);
    drive(&mut g, &mut pets, &w, DOWN, 600);
    let v = g.get(id).unwrap();
    assert_eq!(v.y(), 1040.0);
    assert!(v.is_parked() || v.is_grounded());
    assert_eq!(pets[0].riding(), Some((id, 0)), "landing does not unload a steered heli");
}

#[test]
fn jumping_onto_a_vehicle_in_control_mode_boards_it() {
    let w = world(vec![]);
    let (mut g, id) = garage_with(VehicleKind::Car, 900.0, 1040.0, &w);
    let mut pets = vec![controlled_pet(900.0, &w)];
    drive(&mut g, &mut pets, &w, JUMP, 1);
    drive(&mut g, &mut pets, &w, NONE, 120);
    assert_eq!(pets[0].riding(), Some((id, 0)));
}
