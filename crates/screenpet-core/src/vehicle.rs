//! 🚗 탈것 (v0.7.1, docs/superpowers/specs/2026-11-20-vehicles.md): 🚗 자동차·🚁 헬리콥터·🛴 킥보드·🛻 픽업 트럭.
//! 순수 계산 — 창은 src-tauri/src/vehicles.rs, 그림은 ui/vehicle.js (tools/draw_vehicles.py), 펫 쪽 상태는 brain_vehicle.rs.
//!
//! - 꺼내기: `🚗 탈것 꺼내기 ▸` 메뉴로 한 화면에 MAX_VEHICLES대까지. 빈 탈것은 물건처럼 떨어져 디딤판에 서고, 서 있는 창을 따라감.
//!   사용자가 잡아 끌고 던질 수 있음 (탄 펫이 있으면 튕겨 내림)
//! - 타기: 한가한 펫이 가까운 빈 탈것을 보면 걸어가 올라탐 (brain_vehicle.rs). 사용자가 펫을 탈것 위에 떨궈도 탐 (offer_drop).
//!   운전석이 차 있으면 트럭은 짐칸 (BED_SEATS), 아니면 못 탐
//! - 달리기: 디딤판을 따라 달리다 끝에서 돌아서거나 (창 모서리면 가끔 떨어짐), 옆 모니터로 이어지면 넘어감. RIDE_SECS 뒤 멈추고 내림
//! - 마주침: 앞에 펫이 있으면 끼익 급정거 (먼지 — Screech) → 그 펫은 폴짝 "whoa!", 운전 펫은 "beep beep!" 뒤 돌아섬.
//!   CRASH_CHANCE로 못 멈추고 쿵 (Crash) — 둘 다 튕겨 나가고 탈것은 살짝 밀림. 짐칸 펫은 급정거에 앞으로 쏠리고 BED_FLING_CHANCE로 튕겨 내림
//! - 🚁 헬리콥터: 떠올라 화면 위를 날다 내려앉음 — 마주치는 대신 위로 피함
//! - 🛻 트럭: 운전 펫이 타면 "hop in!" 하고 BED_WAIT_SECS 동안 가까운 펫을 짐칸에 태움
//! - 친구 화면: 내 펫이 친구가 연결된 화면 끝까지 몰고 가면 VISIT_CHANCE로 탈것째 나감 (DroveOut — 앱이 Visit에 실음).
//!   탈것은 친구 화면에 남았다가 펫이 돌아올 때 다시 타고 옴 (take_parked_away). 친구 화면으로 몰고 온 남의 탈것은
//!   그 펫이 떠나면 같이 사라지고 (guest_left), 아무도 안 타면 FOREIGN_LIFE_SECS 뒤 사라짐
//!
//! - 🪃 던지기: 빈 내 탈것을 친구가 연결된 왼쪽·오른쪽 끝 너머로 던지면 (set_throw_edges) 그 친구 화면으로 날아감 (ThrownOut — 앱이 Toss로 보냄).
//!   받은 쪽은 catch_thrown으로 반대편 끝에서 날아 들어온 탈것을 제 것으로 가짐. 남의 탈것(친구 펫이 몰고 온 것)은 넘기지 않음
//! - 🎮 조작 모드: 운전석 펫이 조작 중이면 키대로 몲 (steer_ground / steer_fly) — ←/→ 운전 (Shift 빠르게), 놓으면 멈춰 기다림
//!   (타기 시간이 줄지 않음), ↑ 빵빵, 🚁 방향키로 상하좌우·↓로 내려앉음. 창 끝은 그대로 몰고 떨어지고, 막힌 끝은 그냥 멈추고,
//!   친구가 연결된 끝은 늘 나감. 앞의 펫은 급정거 없이 쿵 밀어냄. 점프해 빈 탈것에 내려앉으면 태움 (Pet::may_hop_in)
//!
//! 자리는 모두 물리 px, 탈것 크기는 논리 px × dpi. x = 탈것 가운데, y = 바퀴 바닥 (화면 아래로 +).

use std::collections::VecDeque;

use serde::Serialize;

use crate::brain::control_moves::ControlInput;
use crate::brain::{Pet, Speech};
use crate::physics::{self, step_air, GRAVITY, THROW_GRAVITY_LOGICAL, THROW_LIFT, VELOCITY_WINDOW};
use crate::surfaces::{EndKind, Owner};
use crate::visit::{self, Fling};
use crate::world::{Side, World};

/// 한 화면에 있을 수 있는 탈것 수 (내가 꺼낸 것 + 친구 펫이 몰고 온 것)
pub const MAX_VEHICLES: usize = 4;
/// 한 번 타고 다니는 시간 (초)
pub const RIDE_SECS: (f64, f64) = (10.0, 30.0);
/// 펫을 만나면 못 멈추고 쿵 할 확률
pub const CRASH_CHANCE: f64 = 0.25;
/// 급정거 때 짐칸 펫이 튕겨 내릴 확률
pub const BED_FLING_CHANCE: f64 = 0.3;
/// 급정거로 멈춘 뒤 서 있는 시간 (초) — 그 뒤 돌아섬
pub const STOP_SECS: f64 = 1.0;
/// 디딤판 끝에서 돌아서기 전 멈칫하는 시간 (초)
pub const TURN_SECS: f64 = 0.4;
/// 타자마자 출발하기 전 부릉 하는 시간 (초)
pub const START_SECS: f64 = 0.6;
/// 🛻 트럭이 짐칸 손님을 기다리는 시간 (초)
pub const BED_WAIT_SECS: f64 = 2.5;
/// 🛻 짐칸에 부르는 거리 (몸 크기 배)
pub const BED_INVITE_REACH: f64 = 3.0;
/// 펫이 탈것을 알아채는 거리 (몸 크기 배, 같은 디딤판)
pub const NOTICE_REACH: f64 = 6.0;
/// 처음 본 빈 탈것마다 타러 갈 확률
pub const BOARD_CHANCE: f64 = 0.5;
/// 같은 탈것은 이만큼 다시 주사위를 굴리지 않음 (초)
pub const SAME_VEHICLE_SECS: f64 = 40.0;
/// 내린 뒤 이만큼은 다시 타러 가지 않음 (초)
pub const CALM_SECS: (f64, f64) = (30.0, 80.0);
/// 탈것까지 걸어가다 이만큼 못 닿으면 그만둠 (초)
pub const APPROACH_SECS: f64 = 8.0;
/// 창 모서리 (밑이 빈 끝)에서 그대로 몰고 떨어질 확률 (아니면 돌아섬)
pub const EDGE_DROP_CHANCE: f64 = 0.35;
/// 친구가 연결된 끝에 닿으면 탈것째 친구 화면으로 갈 확률 (내 펫만)
pub const VISIT_CHANCE: f64 = 0.35;
/// 아무도 안 타는 남의 탈것이 사라지기까지 (초)
pub const FOREIGN_LIFE_SECS: f64 = 300.0;
/// 좌석에 앉은 펫 그림 중 문(덮개)에 가려지는 아래 몫 (다리)
pub const LEG_SHARE: f64 = 0.32;
/// 창 가장자리 여유 (논리 px — 먼지·출렁임이 잘리지 않게, 옆·위)
pub const WINDOW_PAD: f64 = 24.0;
/// 화면 크기 배율: 그림 숫자 (art) × 이것 — 제일 큰 단계 (어른) 펫에 딱 맞게 (자동차 ≈ 어른 펫 한 마리 길이)
pub const VEHICLE_SCALE: f64 = 0.7;
/// 🎮 Shift를 누르고 몰 때 빠르기 배
pub const STEER_BOOST: f64 = 1.6;
/// 빨라지는 가속 (논리 px/s²)
const ACCEL: f64 = 320.0;
/// 끼익 급정거 감속 (논리 px/s²)
const BRAKE: f64 = 900.0;
/// 보통 멈출 때 감속 (논리 px/s²)
const STOP_DECEL: f64 = 520.0;
/// 주인 없이 밀린 탈것이 멈추는 마찰 (논리 px/s²)
const FRICTION: f64 = 420.0;
/// 쿵 뒤에 탈것이 뒤로 밀리는 빠르기 (논리 px/s)
const NUDGE: f64 = 140.0;
/// 앞을 보는 거리: 펫 크기의 이 배 + 제동 거리 + LOOK_MARGIN (논리 px)
const LOOK_SIZES: f64 = 0.6;
/// 앞을 보는 거리의 여유 (논리 px)
const LOOK_MARGIN: f64 = 20.0;
/// 같은 바닥으로 치는 높이 차 (펫 크기 배)
const SAME_LEVEL: f64 = 0.25;
/// 한 번 마주친 뒤 다시 마주침을 보지 않는 시간 (초)
const ENCOUNTER_CALM_SECS: f64 = 2.0;
/// 쿵: 맞은 펫이 날아가는 속도 (논리 px/s: 앞으로, 위로)
const BONK_V: (f64, f64) = (380.0, 520.0);
/// 쿵·급정거: 운전석·짐칸 펫이 튕겨 나가는 속도 (논리 px/s: 앞으로, 위로)
const FLING_V: (f64, f64) = (300.0, 640.0);
/// 잡힌 탈것에서 펫이 튀어 내리는 속도 (논리 px/s: 옆으로, 위로)
const GRAB_FLING_V: (f64, f64) = (160.0, 520.0);
/// 이보다 세게 떨어지면 살짝 튀어 오름 (논리 px/s)
const BOUNCE_MIN_VY: f64 = 380.0;
/// 🚁 떠오르는·내려앉는 빠르기 (논리 px/s)
const HELI_RISE: f64 = 170.0;
/// 🚁 내려앉는 빠르기 (논리 px/s)
const HELI_DESCEND: f64 = 130.0;
/// 🚁 바닥에서 적어도 이만큼 (펫 크기 배) 위로 날음
const HELI_MIN_ALT: f64 = 2.5;
/// 🚁 날며 새 목적지를 고르는 간격 (초)
const HELI_LEG_SECS: (f64, f64) = (2.5, 6.0);
/// 클릭으로 치는 움직임 한계 (논리 px)
const CLICK_SLOP: f64 = 6.0;

/// 탈것 종류 (그림 이름과 같음)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VehicleKind {
    Car,
    Heli,
    Scooter,
    Truck,
}

/// 탈것 생김새와 좌석 (논리 px, 바퀴 바닥 가운데 기준 — x는 앞쪽 +)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    /// 그림 너비
    pub w: f64,
    /// 그림 높이
    pub h: f64,
    /// 운전석 가로 자리
    pub seat_x: f64,
    /// 운전석 문 윗선 높이 (펫 다리가 이 아래로 가려짐 — 0이면 안 가림)
    pub cover: f64,
    /// 운전석 펫 발의 가장 낮은 높이
    pub seat_min: f64,
    /// 짐칸 좌석 가로 자리 (트럭만)
    pub bed_x: [f64; 2],
    /// 짐칸 벽 윗선 높이
    pub bed_cover: f64,
    /// 달리는 빠르기 (논리 px/s)
    pub speed: f64,
    /// 서서 탐 (킥보드 — 다리를 안 가림)
    pub standing: bool,
}

impl VehicleKind {
    /// 모든 탈것 (메뉴·그림 순서)
    pub const ALL: [VehicleKind; 4] = [VehicleKind::Car, VehicleKind::Heli, VehicleKind::Scooter, VehicleKind::Truck];

    /// 통신과 그림 파일(ui/vehicles/<id>.png)에 쓰는 이름
    pub fn id(self) -> &'static str {
        match self {
            VehicleKind::Car => "car",
            VehicleKind::Heli => "heli",
            VehicleKind::Scooter => "scooter",
            VehicleKind::Truck => "truck",
        }
    }

    /// 아는 이름만 (친구가 보낸 글자 — 그대로 그림 이름에 쓰이므로 목록과 정확히 같아야 함)
    pub fn from_id(id: &str) -> Option<VehicleKind> {
        VehicleKind::ALL.into_iter().find(|k| k.id() == id)
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        match self {
            VehicleKind::Car => "🚗 자동차",
            VehicleKind::Heli => "🚁 헬리콥터",
            VehicleKind::Scooter => "🛴 킥보드",
            VehicleKind::Truck => "🛻 픽업 트럭",
        }
    }

    /// 하늘을 낢 (헬리콥터)
    pub fn flies(self) -> bool {
        self == VehicleKind::Heli
    }

    /// 짐칸 자리 수
    pub fn bed_seats(self) -> usize {
        if self == VehicleKind::Truck {
            2
        } else {
            0
        }
    }

    /// 화면에 뜨는 생김새와 좌석 (논리 px): 그림 숫자 (art)를 VEHICLE_SCALE배로 — 빠르기는 그대로
    pub fn spec(self) -> Spec {
        let a = self.art();
        let k = VEHICLE_SCALE;
        Spec {
            w: a.w * k,
            h: a.h * k,
            seat_x: a.seat_x * k,
            cover: a.cover * k,
            seat_min: a.seat_min * k,
            bed_x: [a.bed_x[0] * k, a.bed_x[1] * k],
            bed_cover: a.bed_cover * k,
            ..a
        }
    }

    /// 그림 시트의 생김새와 좌석 (tools/draw_vehicles.py와 같은 숫자 — 화면에선 VEHICLE_SCALE배)
    fn art(self) -> Spec {
        match self {
            VehicleKind::Car => Spec { w: 176.0, h: 100.0, seat_x: -14.0, cover: 56.0, seat_min: 22.0, bed_x: [0.0; 2], bed_cover: 0.0, speed: 230.0, standing: false },
            VehicleKind::Heli => Spec { w: 208.0, h: 150.0, seat_x: 20.0, cover: 50.0, seat_min: 22.0, bed_x: [0.0; 2], bed_cover: 0.0, speed: 210.0, standing: false },
            VehicleKind::Scooter => Spec { w: 104.0, h: 118.0, seat_x: -8.0, cover: 0.0, seat_min: 15.0, bed_x: [0.0; 2], bed_cover: 0.0, speed: 130.0, standing: true },
            VehicleKind::Truck => Spec { w: 236.0, h: 104.0, seat_x: 58.0, cover: 58.0, seat_min: 24.0, bed_x: [-30.0, -88.0], bed_cover: 44.0, speed: 190.0, standing: false },
        }
    }
}

/// 탈것 창 크기 (논리 px): 그림 + 옆·위 여유 (WINDOW_PAD) + 바닥 그림자 4 — 시트 한 칸 통째로 VEHICLE_SCALE배 (창에 꽉 차게 그려짐)
pub fn window_logical(kind: VehicleKind) -> (f64, f64) {
    let s = kind.spec();
    (s.w + 2.0 * WINDOW_PAD * VEHICLE_SCALE, s.h + (WINDOW_PAD + 4.0) * VEHICLE_SCALE)
}

/// 좌석 하나 (논리 px): 가로 자리, 발 높이, 가려지는 높이
fn seat_geometry(kind: VehicleKind, seat: u8, pet_size_logical: f64) -> (f64, f64, f64) {
    let s = kind.spec();
    let (x, cover, min) = match seat {
        0 => (s.seat_x, s.cover, s.seat_min),
        n => (s.bed_x[(n as usize - 1).min(1)], s.bed_cover, s.seat_min - 2.0),
    };
    if cover <= 0.0 {
        return (x, min, 0.0);
    }
    let feet = (cover - LEG_SHARE * pet_size_logical).max(min).min(cover);
    (x, feet, cover - feet)
}

/// 앱에 알리는 일 (먼지·쿵·친구 화면으로 감)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VehicleEvent {
    /// 끼익 급정거 — 앞 범퍼 자리 (x, y)에 먼지
    Screech { id: u64, x: f64, y: f64 },
    /// 쿵! — 부딪힌 자리
    Crash { id: u64, x: f64, y: f64 },
    /// 탈것째 친구 화면으로 나갔음 (탈것은 이 화면에서 없어짐). driver = 운전한 펫 (social_key)
    DroveOut { id: u64, side: Side, kind: VehicleKind, driver: u64 },
    /// 🪃 던진 빈 탈것이 친구가 연결된 끝을 넘어갔음 (이 화면에서 없어짐 — 앱이 그 친구에게 Toss로 보냄)
    ThrownOut { id: u64, side: Side, kind: VehicleKind, fling: Fling },
}

/// 펫에게 보이는 탈것 (펫이 타러 갈지 정함)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleSpot {
    pub id: u64,
    /// 가운데 x
    pub x: f64,
    /// 바퀴 바닥 y
    pub y: f64,
    /// 서 있는 디딤판 (공중·잡힘이면 None)
    pub owner: Option<Owner>,
    /// 운전석이 빔 (그리고 서 있음 — 탈 수 있음)
    pub driver_free: bool,
    /// 짐칸에 빈자리가 있음 (트럭)
    pub bed_free: bool,
}

/// 탈것 앞을 막는 펫 (Pet::vehicle_obstacle)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obstacle {
    pub key: u64,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub half_w: f64,
}

/// 웹뷰가 그릴 모습 (`vehicle-pose`)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct VehiclePose {
    /// +1 오른쪽을 봄, -1 왼쪽
    pub dir: i8,
    /// 바퀴가 구름
    pub moving: bool,
    /// 프로펠러가 돎 (헬리콥터 시동)
    pub rotor: bool,
    /// 기울기 (도, 앞이 숙으면 +)
    pub tilt: f64,
    /// 끼익 먼지를 피우는 중
    pub dust: bool,
    /// 쿵 / 착지로 출렁
    pub bump: bool,
}

/// 🚁 나는 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum FlyStage {
    /// 바로 위로 떠오름
    Lift,
    /// 목적지(x)로 날아감 — leg = 다음 목적지를 고르기까지
    Cruise { leg: f64 },
    /// 아래로 내려앉음
    Descend,
}

/// 탈것의 지금 단계
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// 디딤판 위 (운전 중이면 달림)
    Ground,
    /// 공중 (떨어지거나 던져짐, 운전 중 창 끝에서 떨어짐)
    Air,
    /// 사용자가 잡고 있음
    Held,
    /// 🚁 날고 있음
    Fly(FlyStage),
    /// 친구 화면 쪽 끝 너머로 달려 나가는 중
    Exiting(Side),
}

/// 마주친 펫 (급정거나 쿵을 기다림)
#[derive(Debug, Clone, Copy, PartialEq)]
struct Encounter {
    key: u64,
    crash: bool,
}

/// 누구 탈것인지
#[derive(Debug, Clone, Copy, PartialEq)]
enum Origin {
    /// 이 화면 주인이 꺼냄
    Own,
    /// 친구 펫(social_key)이 몰고 옴
    Foreign(u64),
}

/// 탈것 한 대
#[derive(Debug, Clone)]
pub struct Vehicle {
    id: u64,
    kind: VehicleKind,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    /// 달리는 빠르기 (dir 쪽, 물리 px/s — 0 이상)
    speed: f64,
    dir: f64,
    dpi: f64,
    phase: Phase,
    /// 서 있는 디딤판과 기준점에서의 x
    on: Option<(Owner, f64)>,
    driver: Option<u64>,
    bed: [Option<u64>; 2],
    /// 이번 타기의 남은 시간
    ride_left: f64,
    /// 출발 전 부릉·짐칸 기다리기 남은 시간
    start_wait: f64,
    /// 트럭 짐칸에 부르기를 했음
    invited: bool,
    /// 멈춰 서 있는 남은 시간 (끝나면 turn_after면 돌아섬)
    pause: f64,
    turn_after: bool,
    /// 끝이 다 돼 멈추는 중 (멈추면 다 내림)
    stopping: bool,
    encounter: Option<Encounter>,
    /// 끼익 브레이크 중
    braking: bool,
    calm: f64,
    /// 이번 끝에서 떨어지기로 했나 (한 번만 굴림)
    edge_roll: Option<bool>,
    /// 🚁 날아갈 목적지와 높이
    target: (f64, f64),
    /// 이번 타기에 친구 화면으로 갈지 (끝에 닿을 때 한 번 정함)
    visit_roll: Option<bool>,
    origin: Origin,
    /// 아무도 안 탄 채 흐른 시간 (남의 탈것만 셈)
    idle_for: f64,
    /// 잡은 자리 (커서에서의 차이)
    grab: (f64, f64),
    grab_start: (f64, f64),
    history: VecDeque<(f64, f64, f64)>,
    /// 끼익 먼지·출렁 그리는 남은 시간
    dust: f64,
    bump: f64,
    /// 던진 비행 (논리 중력)
    thrown: bool,
}

impl Vehicle {
    /// 새 탈것 (공중에서 시작 — 다음 틱부터 떨어짐)
    fn new(id: u64, kind: VehicleKind, x: f64, y: f64, dpi: f64, dir: f64) -> Vehicle {
        Vehicle {
            id,
            kind,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            speed: 0.0,
            dir,
            dpi,
            phase: Phase::Air,
            on: None,
            driver: None,
            bed: [None; 2],
            ride_left: 0.0,
            start_wait: 0.0,
            invited: false,
            pause: 0.0,
            turn_after: false,
            stopping: false,
            encounter: None,
            braking: false,
            calm: 0.0,
            edge_roll: None,
            target: (x, y),
            visit_roll: None,
            origin: Origin::Own,
            idle_for: 0.0,
            grab: (0.0, 0.0),
            grab_start: (0.0, 0.0),
            history: VecDeque::new(),
            dust: 0.0,
            bump: 0.0,
            thrown: false,
        }
    }

    /// 번호
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 종류
    pub fn kind(&self) -> VehicleKind {
        self.kind
    }

    /// 가운데 x (물리 px)
    pub fn x(&self) -> f64 {
        self.x
    }

    /// 바퀴 바닥 y (물리 px)
    pub fn y(&self) -> f64 {
        self.y
    }

    /// 보는 쪽 (+1 오른쪽)
    pub fn dir(&self) -> f64 {
        self.dir
    }

    /// 화면 배율
    pub fn dpi(&self) -> f64 {
        self.dpi
    }

    /// 화면 배율을 바꿈 (창이 다른 모니터로 감)
    pub fn set_dpi(&mut self, dpi: f64) {
        if dpi.is_finite() && dpi > 0.0 {
            self.dpi = dpi;
        }
    }

    /// 몸 길이 절반 (물리 px)
    pub fn half_len(&self) -> f64 {
        self.kind.spec().w / 2.0 * self.dpi
    }

    /// 그림 높이 (물리 px)
    pub fn height(&self) -> f64 {
        self.kind.spec().h * self.dpi
    }

    /// 운전 펫 (social_key)
    pub fn driver(&self) -> Option<u64> {
        self.driver
    }

    /// 짐칸 펫들
    pub fn bed(&self) -> [Option<u64>; 2] {
        self.bed
    }

    /// 사용자가 잡고 있음
    pub fn is_held(&self) -> bool {
        self.phase == Phase::Held
    }

    /// 디딤판 위에 멈춰 있고 아무도 안 탐
    pub fn is_parked(&self) -> bool {
        self.phase == Phase::Ground && self.driver.is_none() && self.speed == 0.0 && self.vx == 0.0
    }

    /// 디딤판 위에 있음 (서 있거나 달림)
    pub fn is_grounded(&self) -> bool {
        self.phase == Phase::Ground
    }

    /// 남의 탈것 (친구 펫이 몰고 옴)
    pub fn is_foreign(&self) -> bool {
        matches!(self.origin, Origin::Foreign(_))
    }

    /// 탄 펫이 있음
    fn occupied(&self) -> bool {
        self.driver.is_some() || self.bed.iter().any(Option::is_some)
    }

    /// 좌석의 펫 발 자리 (물리 px) — pet_size = 그 펫 크기 (물리 px)
    pub fn seat_point(&self, seat: u8, pet_size: f64) -> (f64, f64) {
        let (sx, feet, _) = seat_geometry(self.kind, seat, pet_size / self.dpi);
        (self.x + self.dir * sx * self.dpi, self.y - feet * self.dpi)
    }

    /// 좌석에서 가려지는 높이 (물리 px)
    fn seat_cover(&self, seat: u8, pet_size: f64) -> f64 {
        seat_geometry(self.kind, seat, pet_size / self.dpi).2 * self.dpi
    }

    /// 창 사각형 (물리 px: 왼쪽, 위, 너비, 높이)
    pub fn window_rect(&self) -> (f64, f64, f64, f64) {
        let (w, h) = window_logical(self.kind);
        (self.x - w / 2.0 * self.dpi, self.y - (h - 4.0) * self.dpi, w * self.dpi, h * self.dpi)
    }

    /// 그림 위 점 (x, y)인지 (물리 px — 펫을 떨굴 때)
    pub fn covers(&self, x: f64, y: f64, slack: f64) -> bool {
        (x - self.x).abs() <= self.half_len() && y <= self.y + 4.0 * self.dpi && y >= self.y - self.height() - slack
    }

    /// 웹뷰에 보낼 모습
    pub fn pose(&self) -> VehiclePose {
        let moving = match self.phase {
            Phase::Ground | Phase::Exiting(_) => self.speed > 1.0 || self.vx.abs() > 1.0,
            Phase::Fly(_) => self.speed > 1.0,
            _ => false,
        };
        let rotor = self.kind.flies() && (self.driver.is_some() || matches!(self.phase, Phase::Fly(_)));
        let tilt = if matches!(self.phase, Phase::Fly(_)) {
            (self.speed / (self.kind.spec().speed * self.dpi)).min(1.0) * 10.0
        } else if self.braking {
            4.0
        } else {
            0.0
        };
        VehiclePose { dir: if self.dir < 0.0 { -1 } else { 1 }, moving, rotor, tilt, dust: self.dust > 0.0, bump: self.bump > 0.0 }
    }

    /// 지금 자리에서 떨어지기 시작함
    fn start_fall(&mut self) {
        self.on = None;
        self.phase = Phase::Air;
        self.vy = self.vy.max(0.0);
    }

    /// 서 있는 디딤판 기준점을 따라감. 기준점이 사라졌으면 false
    fn follow(&mut self, world: &World) -> bool {
        let Some((owner, rel)) = self.on else { return false };
        let Some((ax, ay)) = world.anchor(owner) else { return false };
        self.x = ax + rel;
        self.y = ay;
        true
    }

    /// 디딤판에 내려섬
    fn settle(&mut self, owner: Owner, y: f64, world: &World) {
        let (ax, _) = world.anchor(owner).unwrap_or((0.0, y));
        self.y = y;
        self.on = Some((owner, self.x - ax));
        self.vy = 0.0;
        self.thrown = false;
        self.phase = Phase::Ground;
    }

    /// 출렁 (착지·쿵)
    fn bounce_mark(&mut self) {
        self.bump = 0.25;
    }

    /// 공중 한 틱: 떨어지다 디딤판에 내려섬 (세게 떨어지면 살짝 튐)
    fn tick_air(&mut self, dt: f64, world: &World) {
        let g = if self.thrown { THROW_GRAVITY_LOGICAL * self.dpi } else { GRAVITY };
        let prev_y = self.y;
        let mut b = physics::Body { x: self.x, y: self.y, vx: self.vx, vy: self.vy };
        step_air(&mut b, dt, &world.bounds, self.half_len(), self.height(), g);
        self.x = b.x;
        self.y = b.y;
        self.vx = b.vx;
        self.vy = b.vy;
        if let Some(s) = world.fall_landing(self.x, prev_y, self.y, self.vy, false) {
            let hard = self.vy > BOUNCE_MIN_VY * self.dpi;
            let vy = self.vy;
            self.settle(s.owner, s.y, world);
            self.bounce_mark();
            if hard && self.driver.is_none() {
                self.phase = Phase::Air;
                self.on = None;
                self.vy = -vy * 0.3;
                self.vx *= 0.6;
            } else if self.driver.is_some() {
                // 운전 중 떨어졌으면 그대로 계속 달림
                self.vx = 0.0;
                self.edge_roll = None;
            } else {
                self.vx = 0.0;
                self.speed = 0.0;
            }
        } else if self.y > world.bounds.bottom + self.height() * 4.0 {
            // 화면 밖으로 사라짐: 위에서 다시 떨어뜨림
            let b = world.bounds;
            self.x = self.x.clamp(b.left + self.half_len(), (b.right - self.half_len()).max(b.left + self.half_len()));
            self.y = b.top + self.height();
            self.vx = 0.0;
            self.vy = 0.0;
        }
    }
}

/// 차고: 화면의 탈것 전부와 탈것·펫 사이의 일 (코어 — 앱은 창만 맞춤)
pub struct Garage {
    vehicles: Vec<Vehicle>,
    next: u64,
    rng: fastrand::Rng,
    clock: f64,
    events: Vec<VehicleEvent>,
    /// 사용자가 잡은 탈것 id
    held: Option<u64>,
    /// 친구 화면에 두고 온 내 탈것 (운전한 펫 → 종류)
    parked_away: Vec<(u64, VehicleKind)>,
    /// 오래돼 사라진 남의 탈것 (앱이 뿅 치움)
    expired: Vec<u64>,
    /// 테스트: 마주침 결과를 정함
    force_crash: Option<bool>,
    /// 테스트: 급정거 때 짐칸 펫이 늘 튕겨 내림
    force_bed_fling: bool,
    /// 테스트: 친구가 연결된 끝이면 늘 나감
    force_visit: bool,
    /// 🪃 던진 탈것을 받을 친구가 연결된 끝 (왼쪽, 오른쪽)
    throw_edges: (bool, bool),
}

/// 펫에게 시킬 일 (차고가 탈것을 다 움직인 뒤 한꺼번에)
enum Act {
    /// 놀라 폴짝 (dir = 탈것이 오던 쪽)
    Startle(u64, f64),
    /// 쿵 맞고 날아감
    Bonk(u64, f64, f64),
    /// 좌석에서 튕겨 나감
    Fling(u64, f64, f64),
    /// 좌석에서 폴짝 내림 (dir = 내리는 쪽)
    Leave(u64, f64),
    /// 말풍선
    Say(u64, Speech),
    /// 🛻 짐칸에 타러 오라고 부름
    Invite(u64, u64),
    /// 탈것째 친구 화면으로 사라짐
    DroveOut(u64, Side),
}

impl Garage {
    /// 빈 차고 (seed = 주사위)
    pub fn new(seed: u64) -> Garage {
        Garage {
            vehicles: Vec::new(),
            next: 1,
            rng: fastrand::Rng::with_seed(seed ^ 0x7665_6869),
            clock: 0.0,
            events: Vec::new(),
            held: None,
            parked_away: Vec::new(),
            expired: Vec::new(),
            force_crash: None,
            force_bed_fling: false,
            force_visit: false,
            throw_edges: (false, false),
        }
    }

    /// 🪃 던진 탈것을 받을 친구가 연결된 끝 (앱이 매 틱 — 탈것을 받는 새 버전 친구만)
    pub fn set_throw_edges(&mut self, left: bool, right: bool) {
        self.throw_edges = (left, right);
    }

    /// 🪃 친구가 던진 탈것: side 끝으로 날아 들어와 떨어져 섬 (이 화면 주인 것이 됨). 왼쪽·오른쪽 끝만, 자리가 없거나 fling이 이상하면 None
    pub fn catch_thrown(&mut self, kind: VehicleKind, world: &World, side: Side, fling: Fling, dpi: f64) -> Option<u64> {
        if self.vehicles.len() >= MAX_VEHICLES || visit::is_vertical(side) {
            return None;
        }
        let fling = fling.sanitized()?;
        let id = self.next;
        self.next += 1;
        let inward = if side == Side::Left { 1.0 } else { -1.0 };
        let mut v = Vehicle::new(id, kind, 0.0, 0.0, dpi, inward);
        let (half, h, b) = (v.half_len(), v.height(), world.bounds);
        v.x = if side == Side::Left { b.left + half } else { b.right - half };
        let wa = world.work_area_at(v.x, b.bottom);
        v.y = (wa.top + fling.height * wa.height()).clamp(wa.top + h, wa.bottom.max(wa.top + h));
        v.vx = inward * fling.vx.abs() * dpi;
        v.vy = fling.vy * dpi;
        v.thrown = true;
        self.vehicles.push(v);
        Some(id)
    }

    /// 화면의 탈것들
    pub fn vehicles(&self) -> &[Vehicle] {
        &self.vehicles
    }

    /// id인 탈것
    pub fn get(&self, id: u64) -> Option<&Vehicle> {
        self.vehicles.iter().find(|v| v.id == id)
    }

    /// id인 탈것 (고침)
    fn get_mut(&mut self, id: u64) -> Option<&mut Vehicle> {
        self.vehicles.iter_mut().find(|v| v.id == id)
    }

    /// 화면 배율을 바꿈 (앱이 창의 모니터를 알게 되면)
    pub fn set_dpi(&mut self, id: u64, dpi: f64) {
        if let Some(v) = self.get_mut(id) {
            v.set_dpi(dpi);
        }
    }

    /// (x, y)에 새 탈것을 꺼냄 (떨어져 섬). 자리가 없으면 None
    pub fn spawn(&mut self, kind: VehicleKind, x: f64, y: f64, dpi: f64) -> Option<u64> {
        if self.vehicles.len() >= MAX_VEHICLES {
            return None;
        }
        let id = self.next;
        self.next += 1;
        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
        self.vehicles.push(Vehicle::new(id, kind, x, y, dpi, dir));
        Some(id)
    }

    /// 탈것 하나를 치움 (탄 펫은 다음 틱에 떨어짐). 있었으면 true
    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.vehicles.len();
        self.vehicles.retain(|v| v.id != id);
        if self.held == Some(id) {
            self.held = None;
        }
        before != self.vehicles.len()
    }

    /// 모두 치움
    pub fn clear(&mut self) {
        self.vehicles.clear();
        self.held = None;
    }

    /// 펫들에게 보일 탈것 자리
    pub fn spots(&self, out: &mut Vec<VehicleSpot>) {
        out.clear();
        out.extend(self.vehicles.iter().map(|v| {
            let standing = v.phase == Phase::Ground && v.speed == 0.0;
            VehicleSpot {
                id: v.id,
                x: v.x,
                y: v.y,
                owner: v.on.filter(|_| v.phase == Phase::Ground).map(|(o, _)| o),
                driver_free: standing && v.driver.is_none() && v.vx == 0.0,
                bed_free: standing && v.bed.iter().take(v.kind.bed_seats()).any(Option::is_none),
            }
        }));
    }

    /// 펫을 탈것에 태움 (운전석이 비었으면 운전, 아니면 트럭 짐칸). 태웠으면 true
    pub fn board(&mut self, id: u64, pet: &mut Pet) -> bool {
        let key = pet.social_key();
        let size = pet.cfg.size;
        let rng_ride = crate::behavior::range(&mut self.rng, RIDE_SECS);
        let Some(v) = self.get_mut(id) else { return false };
        if v.phase == Phase::Held || v.driver == Some(key) || v.bed.contains(&Some(key)) {
            return false;
        }
        let seat = if v.driver.is_none() && matches!(v.phase, Phase::Ground | Phase::Air) {
            0
        } else if let Some(i) = (0..v.kind.bed_seats()).find(|&i| v.bed[i].is_none()) {
            i as u8 + 1
        } else {
            return false;
        };
        let front = seat > 0;
        let cover = v.seat_cover(seat, size);
        if !pet.take_seat(id, seat, front, cover) {
            return false;
        }
        if seat == 0 {
            v.driver = Some(key);
            v.ride_left = rng_ride;
            v.stopping = false;
            v.encounter = None;
            v.braking = false;
            v.visit_roll = None;
            v.invited = false;
            v.idle_for = 0.0;
            v.start_wait = if v.kind.bed_seats() > 0 { BED_WAIT_SECS } else { START_SECS };
            if v.kind.bed_seats() > 0 {
                pet.vehicle_say(Speech::HopIn);
            } else {
                pet.vehicle_say(Speech::Drive);
            }
        } else {
            v.bed[seat as usize - 1] = Some(key);
        }
        let (sx, sy) = v.seat_point(seat, size);
        let dir = v.dir;
        pet.seat_at(sx, sy, dir, cover, false);
        true
    }

    /// 사용자가 놓은 펫 (방금 손에서 놓음): 탈것 위면 태움. 태웠으면 true
    pub fn offer_drop(&mut self, pet: &mut Pet) -> bool {
        if pet.riding().is_some() || !pet.just_dropped() {
            return false;
        }
        let (x, y) = (pet.body.x, pet.body.y);
        let slack = pet.cfg.size * 0.4;
        let hit = self.vehicles.iter().filter(|v| v.phase != Phase::Held && v.covers(x, y, slack)).min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs())).map(|v| v.id);
        let Some(id) = hit else { return false };
        self.board(id, pet)
    }

    /// 탈것을 눌렀음: 잡고 탄 펫은 튕겨 내림. 잡았으면 true
    pub fn pointer_down<T: AsRef<Pet> + AsMut<Pet>>(&mut self, id: u64, cx: f64, cy: f64, pets: &mut [T]) -> bool {
        if self.held.is_some() {
            return false;
        }
        let clock = self.clock;
        let Some(v) = self.get_mut(id) else { return false };
        if matches!(v.phase, Phase::Exiting(_)) {
            return false;
        }
        let riders: Vec<u64> = v.driver.into_iter().chain(v.bed.iter().flatten().copied()).collect();
        let (dpi, dir) = (v.dpi, v.dir);
        v.driver = None;
        v.bed = [None; 2];
        v.phase = Phase::Held;
        v.on = None;
        v.speed = 0.0;
        v.vx = 0.0;
        v.vy = 0.0;
        v.braking = false;
        v.encounter = None;
        v.grab = (v.x - cx, v.y - cy);
        v.grab_start = (cx, cy);
        v.history.clear();
        v.history.push_back((clock, cx, cy));
        self.held = Some(id);
        for (i, key) in riders.into_iter().enumerate() {
            let side = if i % 2 == 0 { -dir } else { dir };
            if let Some(p) = pets.iter_mut().find(|p| p.as_ref().social_key() == key) {
                p.as_mut().fling_off(side * GRAB_FLING_V.0 * dpi, -GRAB_FLING_V.1 * dpi);
            }
        }
        true
    }

    /// 잡은 탈것을 커서로 옮김
    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        let clock = self.clock;
        let Some(id) = self.held else { return };
        let Some(v) = self.get_mut(id) else { return };
        v.x = cx + v.grab.0;
        v.y = cy + v.grab.1;
        v.history.push_back((clock, cx, cy));
        while v.history.len() > 2 && clock - v.history[0].0 > VELOCITY_WINDOW {
            v.history.pop_front();
        }
    }

    /// 손을 놓음: 커서 속도로 던짐 (거의 안 움직였으면 살며시 떨어짐)
    pub fn pointer_up(&mut self) {
        let Some(id) = self.held.take() else { return };
        let Some(v) = self.get_mut(id) else { return };
        let (vx, vy) = physics::release_velocity(v.history.iter());
        let last = v.history.back().map_or(v.grab_start, |&(_, x, y)| (x, y));
        let moved = (last.0 - v.grab_start.0).hypot(last.1 - v.grab_start.1) / v.dpi;
        let (vx, vy) = if moved < CLICK_SLOP { (0.0, 0.0) } else { physics::throw_velocity(vx, vy, v.dpi, THROW_LIFT) };
        if vx.abs() > 50.0 * v.dpi {
            v.dir = vx.signum();
        }
        v.vx = vx;
        v.vy = vy;
        v.thrown = true;
        v.phase = Phase::Air;
        v.history.clear();
    }

    /// 사용자가 탈것을 잡고 있음
    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }

    /// 잡고 있는 탈것
    pub fn held(&self) -> Option<u64> {
        self.held
    }

    /// 앱에 알릴 일 (꺼내 감)
    pub fn take_events(&mut self) -> Vec<VehicleEvent> {
        std::mem::take(&mut self.events)
    }

    /// 오래돼 사라진 남의 탈것 (꺼내 감 — 앱이 뿅)
    pub fn take_expired(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.expired)
    }

    /// 친구 화면에 두고 온 내 탈것 — 그 펫이 돌아올 때 한 번 꺼냄 (다시 몰고 들어옴)
    pub fn take_parked_away(&mut self, driver: u64) -> Option<VehicleKind> {
        let i = self.parked_away.iter().position(|&(k, _)| k == driver)?;
        Some(self.parked_away.remove(i).1)
    }

    /// 친구 화면으로 몰고 간 탈것을 기억함 (펫이 돌아오면 다시 몰고 옴 — 탈것째 못 갔을 때도)
    pub fn remember_parked_away(&mut self, driver: u64, kind: VehicleKind) {
        self.parked_away.retain(|&(k, _)| k != driver);
        self.parked_away.push((driver, kind));
    }

    /// 탈것을 몰고 화면 끝(side)으로 들어옴 (친구 펫이 몰고 왔거나 내 펫이 돌아옴): 바닥 끝에 놓고 태워 안쪽으로 달림.
    /// 자리가 없거나 바닥이 없으면 None (펫은 그냥 걸어 들어옴)
    pub fn arrive(&mut self, kind: VehicleKind, world: &World, side: Side, dpi: f64, pet: &mut Pet) -> Option<u64> {
        if crate::visit::is_vertical(side) {
            return None;
        }
        let (owner, edge_x, y) = crate::visit::entry_ground(world, side)?;
        let piece = *world.piece_at(owner, edge_x)?;
        let inward = -crate::visit::dir(side);
        let half = kind.spec().w / 2.0 * dpi;
        if piece.width() < 2.0 * half {
            return None;
        }
        let x = edge_x + inward * half;
        let id = self.spawn(kind, x, y, dpi)?;
        let foreign = pet.guest().is_some();
        {
            let v = self.get_mut(id).expect("just spawned");
            v.dir = inward;
            v.settle(owner, y, world);
            if foreign {
                v.origin = Origin::Foreign(pet.social_key());
            }
        }
        if !self.board(id, pet) {
            self.remove(id);
            return None;
        }
        let v = self.get_mut(id).expect("just spawned");
        // 들어오자마자 달림 (부릉 기다리지 않음)
        v.start_wait = 0.0;
        v.invited = true;
        v.speed = kind.spec().speed * dpi * 0.6;
        Some(id)
    }

    /// 손님 펫이 떠났음: 그 펫이 몰고 온 탈것도 같이 사라짐 (있었으면 그 id)
    pub fn guest_left(&mut self, key: u64) -> Option<u64> {
        let id = self.vehicles.iter().find(|v| v.origin == Origin::Foreign(key))?.id;
        self.remove(id);
        Some(id)
    }

    /// 테스트·디버그: 이번 타기를 곧 끝냄
    pub fn end_ride_soon(&mut self, id: u64) {
        if let Some(v) = self.get_mut(id) {
            v.ride_left = 0.0;
        }
    }

    /// 테스트: 보는 쪽을 정함
    pub fn face(&mut self, id: u64, dir: f64) {
        if let Some(v) = self.get_mut(id) {
            v.dir = dir;
        }
    }

    /// 테스트: 마주침 결과를 정함 (None = 주사위)
    pub fn force_crash(&mut self, crash: Option<bool>) {
        self.force_crash = crash;
    }

    /// 테스트: 급정거 때 짐칸 펫이 늘 튕겨 내림
    pub fn force_bed_fling(&mut self, on: bool) {
        self.force_bed_fling = on;
    }

    /// 테스트: 친구가 연결된 끝에 닿으면 늘 탈것째 나감
    pub fn force_visit(&mut self, on: bool) {
        self.force_visit = on;
    }

    /// 한 틱: 타러 온 펫 태우기, 내린 펫 빼기, 탈것 움직이기, 마주침, 좌석에 펫 앉히기
    pub fn tick<T: AsRef<Pet> + AsMut<Pet>>(&mut self, dt: f64, world: &World, pets: &mut [T]) {
        self.clock += dt;
        // 1. 좌석을 떠난 펫 (잡혔거나 사라짐)은 뺌
        for v in &mut self.vehicles {
            let still = |key: u64, seat: u8| pets.iter().any(|p| p.as_ref().social_key() == key && p.as_ref().riding() == Some((v.id, seat)));
            if v.driver.is_some_and(|k| !still(k, 0)) {
                v.driver = None;
                v.braking = false;
                v.encounter = None;
            }
            for i in 0..2 {
                if v.bed[i].is_some_and(|k| !still(k, i as u8 + 1)) {
                    v.bed[i] = None;
                }
            }
        }
        // 2. 다 걸어와 타려는 펫을 태움 (자리가 없으면 그만둠)
        for i in 0..pets.len() {
            if let Some(id) = pets[i].as_ref().boarding() {
                if !self.board(id, pets[i].as_mut()) {
                    pets[i].as_mut().cancel_boarding();
                }
            }
        }
        // 2-1. 🎮 조작 모드로 점프해 빈 탈것 위에 내려앉은 펫을 태움
        for i in 0..pets.len() {
            let p = pets[i].as_ref();
            if !p.may_hop_in() {
                continue;
            }
            let (x, y, slack) = (p.body.x, p.body.y, p.cfg.size * 0.2);
            let hit = self.vehicles.iter().find(|v| v.phase == Phase::Ground && v.driver.is_none() && v.vx == 0.0 && v.covers(x, y, slack)).map(|v| v.id);
            if let Some(id) = hit {
                self.board(id, pets[i].as_mut());
            }
        }
        // 3. 탈것을 움직임
        let steers: Vec<(u64, ControlInput, bool)> = pets.iter().filter_map(|p| { let p = p.as_ref(); p.steering().map(|(k, honk)| (p.social_key(), k, honk)) }).collect();
        let obstacles: Vec<Obstacle> = pets.iter().filter_map(|p| p.as_ref().vehicle_obstacle()).collect();
        let mut acts: Vec<Act> = Vec::new();
        let free: Vec<(u64, f64, f64, f64, Option<Owner>)> =
            pets.iter().filter(|p| p.as_ref().may_board_now()).map(|p| { let p = p.as_ref(); (p.social_key(), p.body.x, p.body.y, p.cfg.size, p.standing.map(|s| s.owner)) }).collect();
        let edges: Vec<(u64, bool, bool)> = pets.iter().map(|p| { let p = p.as_ref(); (p.social_key(), p.may_drive_out(Side::Left), p.may_drive_out(Side::Right)) }).collect();
        let (force_crash, force_bed_fling, force_visit, throw_edges) = (self.force_crash, self.force_bed_fling, self.force_visit, self.throw_edges);
        let mut gone = Vec::new();
        for v in &mut self.vehicles {
            let (steer, honk) = match v.driver.and_then(|d| steers.iter().find(|s| s.0 == d)) {
                Some(&(_, k, h)) => (Some(k), h),
                None => (None, false),
            };
            let mut ctx = Ctx { rng: &mut self.rng, acts: &mut acts, events: &mut self.events, obstacles: &obstacles, free: &free, edges: &edges, force_crash, force_bed_fling, force_visit, throw_edges, steer, honk };
            if step(v, dt, world, &mut ctx) {
                gone.push(v.id);
            }
            // 남의 탈것: 아무도 안 탄 채 오래되면 사라짐
            if v.is_foreign() && !v.occupied() && v.phase != Phase::Held {
                v.idle_for += dt;
                if v.idle_for >= FOREIGN_LIFE_SECS {
                    self.expired.push(v.id);
                    gone.push(v.id);
                }
            } else {
                v.idle_for = 0.0;
            }
        }
        for id in gone {
            if let Some(v) = self.get(id) {
                if !v.is_foreign() {
                    if let (Phase::Exiting(_), Some(d)) = (v.phase, v.driver) {
                        let kind = v.kind;
                        self.remember_parked_away(d, kind);
                    }
                }
            }
            self.remove(id);
        }
        // 4. 펫에게 시킬 일
        for act in acts {
            let key = match act {
                Act::Startle(k, _) | Act::Bonk(k, _, _) | Act::Fling(k, _, _) | Act::Leave(k, _) | Act::Say(k, _) | Act::Invite(k, _) | Act::DroveOut(k, _) => k,
            };
            let Some(p) = pets.iter_mut().find(|p| p.as_ref().social_key() == key) else { continue };
            let p = p.as_mut();
            match act {
                Act::Startle(_, dir) => p.startle(dir),
                Act::Bonk(_, vx, vy) => p.bonk(vx, vy),
                Act::Fling(_, vx, vy) => p.fling_off(vx, vy),
                Act::Leave(_, dir) => p.leave_vehicle(dir),
                Act::Say(_, s) => p.vehicle_say(s),
                Act::Invite(_, id) => {
                    p.invite_ride(id);
                }
                Act::DroveOut(_, side) => p.drove_out(side),
            }
        }
        // 5. 좌석에 앉힘
        for v in &self.vehicles {
            let lurch = v.braking && v.bed.iter().any(Option::is_some);
            let seats = v.driver.map(|k| (k, 0u8)).into_iter().chain((0..2).filter_map(|i| v.bed[i].map(|k| (k, i as u8 + 1))));
            for (key, seat) in seats {
                if let Some(p) = pets.iter_mut().find(|p| p.as_ref().social_key() == key) {
                    let p = p.as_mut();
                    let size = p.cfg.size;
                    let (sx, sy) = v.seat_point(seat, size);
                    let lurch_dx = if lurch && seat > 0 { v.dir * 0.08 * size } else { 0.0 };
                    p.seat_at(sx + lurch_dx, sy, v.dir, v.seat_cover(seat, size), lurch && seat > 0);
                }
            }
        }
    }
}

/// 한 탈것 걸음에 빌려 쓰는 것들
struct Ctx<'a> {
    rng: &'a mut fastrand::Rng,
    acts: &'a mut Vec<Act>,
    events: &'a mut Vec<VehicleEvent>,
    obstacles: &'a [Obstacle],
    /// 탈 수 있는 한가한 펫: key, x, y, 크기, 디딤판
    free: &'a [(u64, f64, f64, f64, Option<Owner>)],
    /// 펫마다 친구 화면으로 몰고 나갈 수 있는 끝 (왼쪽, 오른쪽)
    edges: &'a [(u64, bool, bool)],
    force_crash: Option<bool>,
    force_bed_fling: bool,
    force_visit: bool,
    /// 🪃 던진 탈것을 받을 친구가 연결된 끝 (왼쪽, 오른쪽)
    throw_edges: (bool, bool),
    /// 🎮 운전석 펫이 조작 중이면 그 키
    steer: Option<ControlInput>,
    /// 🎮 이 틱에 ↑를 막 누름 (빵빵)
    honk: bool,
}

/// 탈것 한 틱. 화면에서 없어졌으면 (친구 화면으로 감) true
fn step(v: &mut Vehicle, dt: f64, world: &World, ctx: &mut Ctx) -> bool {
    v.dust = (v.dust - dt).max(0.0);
    v.bump = (v.bump - dt).max(0.0);
    v.calm = (v.calm - dt).max(0.0);
    if v.driver.is_some() && v.start_wait <= 0.0 && !v.stopping && ctx.steer.is_none() {
        v.ride_left -= dt;
    }
    match v.phase {
        Phase::Held => false,
        Phase::Air => {
            if let Some((side, fling)) = throw_exit(v, dt, world, ctx.throw_edges) {
                ctx.events.push(VehicleEvent::ThrownOut { id: v.id, side, kind: v.kind, fling });
                return true;
            }
            v.tick_air(dt, world);
            if v.phase == Phase::Ground && v.driver.is_some() && v.kind.flies() && ctx.steer.is_none() {
                v.phase = Phase::Fly(FlyStage::Lift);
                pick_alt(v, world, ctx.rng);
            }
            false
        }
        Phase::Fly(stage) => {
            fly(v, dt, world, stage, ctx);
            false
        }
        Phase::Exiting(side) => {
            v.speed = (v.speed + ACCEL * v.dpi * dt).min(v.kind.spec().speed * v.dpi);
            v.x += v.dir * v.speed * dt;
            let b = world.bounds;
            let out = match side {
                Side::Left => v.x + v.half_len() < b.left,
                _ => v.x - v.half_len() > b.right,
            };
            if out {
                if let Some(d) = v.driver {
                    ctx.acts.push(Act::DroveOut(d, side));
                    ctx.events.push(VehicleEvent::DroveOut { id: v.id, side, kind: v.kind, driver: d });
                }
                for k in v.bed.iter().flatten() {
                    ctx.acts.push(Act::Fling(*k, -v.dir * FLING_V.0 * 0.5 * v.dpi, -FLING_V.1 * v.dpi));
                }
                return true;
            }
            false
        }
        Phase::Ground => {
            ground(v, dt, world, ctx);
            false
        }
    }
}

/// 🪃 던진 빈 내 탈것이 이번 틱에 친구가 연결된 왼쪽·오른쪽 끝을 넘으면 그 끝과 이어 갈 움직임
fn throw_exit(v: &Vehicle, dt: f64, world: &World, (left, right): (bool, bool)) -> Option<(Side, Fling)> {
    if !v.thrown || v.occupied() || v.is_foreign() {
        return None;
    }
    let (b, half) = (world.bounds, v.half_len());
    let next_x = v.x + v.vx * dt;
    let side = if left && v.vx < 0.0 && next_x < b.left + half {
        Side::Left
    } else if right && v.vx > 0.0 && next_x > b.right - half {
        Side::Right
    } else {
        return None;
    };
    let body = physics::Body { x: v.x, y: v.y, vx: v.vx, vy: v.vy };
    visit::fling_across(world, side, &body, v.dpi).map(|f| (side, f))
}

/// 🚁 떠 있을 높이를 고름 (바닥에서 HELI_MIN_ALT 펫 크기 위 ~ 화면 위쪽)
fn pick_alt(v: &mut Vehicle, world: &World, rng: &mut fastrand::Rng) {
    let wa = world.work_area_at(v.x, v.y - 1.0);
    let top = wa.top + v.height() + 30.0 * v.dpi;
    let low = (v.y - HELI_MIN_ALT * 128.0 * v.dpi).max(top);
    let alt = top + rng.f64() * (low - top);
    let half = v.half_len();
    let tx = wa.left + half + rng.f64() * (wa.width() - 2.0 * half).max(0.0);
    v.target = (tx, alt);
}

/// 🚁 나는 한 틱
fn fly(v: &mut Vehicle, dt: f64, world: &World, stage: FlyStage, ctx: &mut Ctx) {
    if let Some(k) = ctx.steer {
        steer_fly(v, dt, world, k);
        return;
    }
    let dpi = v.dpi;
    let cruise = v.kind.spec().speed * dpi;
    // 운전 펫이 없으면 (잡혔음) 천천히 내려앉음
    let ending = v.driver.is_none() || v.ride_left <= 0.0;
    if v.start_wait > 0.0 {
        v.start_wait -= dt;
        return;
    }
    // 아래의 펫 머리에 닿을 듯하면 위로 피함
    let half = v.half_len();
    let below = ctx.obstacles.iter().any(|o| (o.x - v.x).abs() < half + o.half_w && o.y - o.size < v.y + 0.6 * o.size && o.y > v.y - 1.0);
    match stage {
        FlyStage::Lift => {
            v.speed = 0.0;
            v.y -= HELI_RISE * dpi * dt;
            if v.y <= v.target.1 {
                v.y = v.target.1;
                v.phase = Phase::Fly(FlyStage::Cruise { leg: crate::behavior::range(ctx.rng, HELI_LEG_SECS) });
            }
        }
        FlyStage::Cruise { leg } => {
            if ending {
                v.phase = Phase::Fly(FlyStage::Descend);
                return;
            }
            let dx = v.target.0 - v.x;
            if dx.abs() > 2.0 {
                v.dir = dx.signum();
                v.speed = (v.speed + ACCEL * dpi * dt).min(cruise).min(dx.abs() / dt.max(1e-6));
            } else {
                v.speed = (v.speed - STOP_DECEL * dpi * dt).max(0.0);
            }
            v.x += v.dir * v.speed * dt;
            let dy = v.target.1 - v.y;
            v.y += dy.signum() * (HELI_RISE * 0.5 * dpi * dt).min(dy.abs());
            if below {
                v.y -= HELI_RISE * dpi * dt;
            }
            let leg = leg - dt;
            v.phase = Phase::Fly(FlyStage::Cruise { leg });
            if leg <= 0.0 {
                pick_alt(v, world, ctx.rng);
                v.target.1 = v.target.1.min(v.y + 60.0 * dpi);
                v.phase = Phase::Fly(FlyStage::Cruise { leg: crate::behavior::range(ctx.rng, HELI_LEG_SECS) });
            }
        }
        FlyStage::Descend => {
            v.speed = (v.speed - STOP_DECEL * dpi * dt).max(0.0);
            v.x += v.dir * v.speed * dt;
            if below {
                // 바로 밑에 펫이 있음: 옆으로 비켜 내려앉음
                v.speed = cruise * 0.4;
                v.x += v.dir * v.speed * dt;
                return;
            }
            let prev = v.y;
            v.y += HELI_DESCEND * dpi * dt;
            let b = world.bounds;
            v.x = v.x.clamp(b.left + half, (b.right - half).max(b.left + half));
            if let Some(s) = world.fall_landing(v.x, prev, v.y, 1.0, false) {
                v.speed = 0.0;
                v.settle(s.owner, s.y, world);
                v.bounce_mark();
                unload(v, ctx);
            }
        }
    }
}

/// 다 내림: 운전 펫과 짐칸 펫이 폴짝 내림, 탈것은 멈춤
fn unload(v: &mut Vehicle, ctx: &mut Ctx) {
    if let Some(d) = v.driver.take() {
        ctx.acts.push(Act::Leave(d, -v.dir));
    }
    for (i, k) in v.bed.iter_mut().enumerate() {
        if let Some(k) = k.take() {
            ctx.acts.push(Act::Leave(k, if i == 0 { -v.dir } else { v.dir }));
        }
    }
    v.speed = 0.0;
    v.stopping = false;
    v.braking = false;
    v.encounter = None;
}

/// 디딤판 위 한 틱: 서 있거나 달림
fn ground(v: &mut Vehicle, dt: f64, world: &World, ctx: &mut Ctx) {
    if !v.follow(world) {
        v.start_fall();
        return;
    }
    let dpi = v.dpi;
    // 아무도 안 탐: 밀린 것만 마찰로 멈춤
    if v.driver.is_none() {
        // 운전 펫이 내린 채 달리던 것도 밀린 것처럼 미끄러져 멈춤 (움직임은 vx에)
        if v.speed > 0.0 {
            v.vx = v.dir * v.speed;
            v.speed = 0.0;
        }
        if v.vx != 0.0 {
            let slide = v.vx;
            let left = (slide.abs() - FRICTION * dpi * dt).max(0.0);
            v.vx = left * slide.signum();
            if left > 0.0 {
                move_along(v, world, slide.signum(), left * dt, ctx, false);
            }
        }
        if v.vx == 0.0 {
            v.braking = false;
        }
        // 짐칸에만 남은 펫은 내림
        if v.speed == 0.0 && v.bed.iter().any(Option::is_some) {
            unload(v, ctx);
        }
        return;
    }
    // 🎮 조작 모드: 키대로 (🚁는 ↑로 떠오름)
    if let Some(k) = ctx.steer {
        if v.kind.flies() {
            if k.up && v.start_wait <= 0.0 {
                v.phase = Phase::Fly(FlyStage::Cruise { leg: 1.0 });
                v.on = None;
            } else {
                v.start_wait = (v.start_wait - dt).max(0.0);
            }
            return;
        }
        if v.start_wait > 0.0 {
            v.start_wait -= dt;
            if !v.invited && v.kind.bed_seats() > 0 {
                v.invited = true;
                invite(v, ctx);
            }
            return;
        }
        steer_ground(v, dt, world, k, ctx);
        return;
    }
    // 🚁 시동: 떠오름
    if v.kind.flies() {
        if v.ride_left > 0.0 {
            v.phase = Phase::Fly(FlyStage::Lift);
            pick_alt(v, world, ctx.rng);
        } else {
            unload(v, ctx);
        }
        return;
    }
    if v.start_wait > 0.0 {
        v.start_wait -= dt;
        // 🛻 짐칸에 탈 펫을 한 번 부름
        if !v.invited && v.kind.bed_seats() > 0 {
            v.invited = true;
            invite(v, ctx);
        }
        return;
    }
    if v.pause > 0.0 {
        v.pause -= dt;
        if v.pause <= 0.0 && v.turn_after {
            v.turn_after = false;
            v.dir = -v.dir;
            v.edge_roll = None;
        }
        return;
    }
    if v.ride_left <= 0.0 && !v.stopping {
        v.stopping = true;
    }
    let cruise = v.kind.spec().speed * dpi;
    // 앞의 펫
    if v.encounter.is_none() && v.calm <= 0.0 && v.speed > 1.0 && !v.stopping {
        if let Some(o) = ahead(v, ctx.obstacles) {
            let crash = ctx.force_crash.unwrap_or_else(|| ctx.rng.f64() < CRASH_CHANCE);
            v.encounter = Some(Encounter { key: o.key, crash });
            if !crash {
                start_screech(v, o.key, ctx);
            }
        }
    }
    if let Some(e) = v.encounter {
        match ctx.obstacles.iter().find(|o| o.key == e.key) {
            Some(o) if e.crash => {
                let gap = v.dir * (o.x - v.x) - v.half_len() - o.half_w;
                if gap <= 0.0 {
                    crash(v, o, ctx);
                    return;
                }
                if gap > look_ahead(v, o.size) * 1.5 {
                    v.encounter = None;
                }
            }
            None if e.crash => v.encounter = None,
            _ => {}
        }
    }
    let target = if v.braking || v.stopping { 0.0 } else { cruise };
    let decel = if v.braking { BRAKE } else { STOP_DECEL } * dpi;
    v.speed = if v.speed < target { (v.speed + ACCEL * dpi * dt).min(target) } else { (v.speed - decel * dt).max(target) };
    if v.speed <= 0.0 {
        v.speed = 0.0;
        if v.stopping {
            unload(v, ctx);
            return;
        }
        if v.braking {
            // 끼익 멈췄음: 잠깐 섰다가 돌아섬
            v.braking = false;
            v.encounter = None;
            v.calm = ENCOUNTER_CALM_SECS + STOP_SECS;
            v.pause = STOP_SECS;
            v.turn_after = true;
            return;
        }
    }
    let step = v.speed * dt;
    move_along(v, world, v.dir, step, ctx, true);
}

/// 좌우 방향키로 빠르기를 고침: 같은 쪽(또는 서 있을 때)은 빨라지고, 반대쪽은 끼익 멈춘 뒤 돌아섬, 키 없으면 멈춤
fn steer_speed(v: &mut Vehicle, dt: f64, k: ControlInput) {
    let dpi = v.dpi;
    let want = (k.right as i8 - k.left as i8) as f64;
    let cruise = v.kind.spec().speed * dpi * if k.run { STEER_BOOST } else { 1.0 };
    if want != 0.0 && (want == v.dir || v.speed <= 0.0) {
        if want != v.dir {
            v.dir = want;
            v.edge_roll = None;
        }
        v.braking = false;
        v.speed = if v.speed < cruise { (v.speed + ACCEL * dpi * dt).min(cruise) } else { (v.speed - STOP_DECEL * dpi * dt).max(cruise) };
    } else {
        v.braking = want != 0.0;
        let decel = if v.braking { BRAKE } else { STOP_DECEL } * dpi;
        v.speed = (v.speed - decel * dt).max(0.0);
    }
}

/// 🎮 디딤판 위를 키대로 몲: ↑ 빵빵, 앞의 펫은 쿵 밀어냄 (운전 펫은 그대로 앉아 있음)
fn steer_ground(v: &mut Vehicle, dt: f64, world: &World, k: ControlInput, ctx: &mut Ctx) {
    v.pause = 0.0;
    v.turn_after = false;
    v.stopping = false;
    v.encounter = None;
    if ctx.honk {
        if let Some(d) = v.driver {
            ctx.acts.push(Act::Say(d, Speech::Honk));
        }
    }
    steer_speed(v, dt, k);
    if v.speed > 1.0 && v.calm <= 0.0 {
        let hit = ctx.obstacles.iter().filter(|o| Some(o.key) != v.driver && !v.bed.contains(&Some(o.key))).find(|o| {
            let gap = v.dir * (o.x - v.x) - v.half_len() - o.half_w;
            (o.y - v.y).abs() <= SAME_LEVEL * o.size && gap <= 0.0 && gap > -o.half_w
        });
        if let Some(o) = hit {
            let dpi = v.dpi;
            ctx.events.push(VehicleEvent::Crash { id: v.id, x: v.x + v.dir * v.half_len(), y: v.y - 0.4 * v.height() });
            ctx.acts.push(Act::Bonk(o.key, v.dir * BONK_V.0 * dpi, -BONK_V.1 * dpi));
            if let Some(d) = v.driver {
                ctx.acts.push(Act::Say(d, Speech::Honk));
            }
            v.bounce_mark();
            v.dust = 0.5;
            v.speed *= 0.5;
            v.calm = 0.5;
        }
    }
    let step = v.speed * dt;
    move_along(v, world, v.dir, step, ctx, true);
}

/// 🎮 🚁 키대로 날기: ←/→ 앞뒤, ↑/↓ 위아래 (Shift 빠르게), 키 없으면 그 자리에 떠 있음, ↓로 디딤판에 닿으면 내려앉음 (내리지는 않음)
fn steer_fly(v: &mut Vehicle, dt: f64, world: &World, k: ControlInput) {
    if v.start_wait > 0.0 {
        v.start_wait -= dt;
        return;
    }
    let dpi = v.dpi;
    steer_speed(v, dt, k);
    v.braking = false;
    let half = v.half_len();
    let b = world.bounds;
    v.x = (v.x + v.dir * v.speed * dt).clamp(b.left + half, (b.right - half).max(b.left + half));
    let vert = (k.down as i8 - k.up as i8) as f64;
    let rise = if vert < 0.0 { HELI_RISE } else { HELI_DESCEND * 1.5 };
    let prev = v.y;
    v.y += vert * rise * dpi * dt * if k.run { STEER_BOOST } else { 1.0 };
    let wa = world.work_area_at(v.x, prev - 1.0);
    v.y = v.y.max(wa.top + v.height());
    if vert > 0.0 {
        if let Some(s) = world.fall_landing(v.x, prev, v.y, 1.0, false) {
            v.speed = 0.0;
            v.settle(s.owner, s.y, world);
            v.bounce_mark();
        }
    }
}

/// 디딤판 끝에서: 조작 중이면 그냥 멈춤, 아니면 멈칫했다가 돌아섬
fn road_end(v: &mut Vehicle, limit: f64, world: &World, steered: bool) {
    if steered {
        v.x = limit;
        v.speed = 0.0;
        reanchor(v, world);
    } else {
        turn(v, limit, world);
    }
}

/// 앞을 보는 거리 (물리 px)
fn look_ahead(v: &Vehicle, pet_size: f64) -> f64 {
    let brake_dist = v.speed * v.speed / (2.0 * BRAKE * v.dpi);
    LOOK_SIZES * pet_size + brake_dist + LOOK_MARGIN * v.dpi
}

/// 앞에서 마주칠 펫 (같은 바닥, 앞 범퍼에서 앞을 보는 거리 안 — 가장 가까운 것)
fn ahead<'a>(v: &Vehicle, obstacles: &'a [Obstacle]) -> Option<&'a Obstacle> {
    obstacles
        .iter()
        .filter(|o| Some(o.key) != v.driver && !v.bed.contains(&Some(o.key)))
        .filter(|o| (o.y - v.y).abs() <= SAME_LEVEL * o.size)
        .filter_map(|o| {
            let gap = v.dir * (o.x - v.x) - v.half_len() - o.half_w;
            (gap > -o.half_w && gap <= look_ahead(v, o.size)).then_some((o, gap))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(o, _)| o)
}

/// 끼익 급정거 시작: 먼지, 걷던 펫은 폴짝, 운전 펫은 "beep beep!", 짐칸 펫은 쏠림 (가끔 튕겨 내림)
fn start_screech(v: &mut Vehicle, victim: u64, ctx: &mut Ctx) {
    v.braking = true;
    v.dust = 0.6;
    let front = v.x + v.dir * v.half_len();
    ctx.events.push(VehicleEvent::Screech { id: v.id, x: front, y: v.y });
    ctx.acts.push(Act::Startle(victim, v.dir));
    if let Some(d) = v.driver {
        ctx.acts.push(Act::Say(d, Speech::Honk));
    }
    for slot in v.bed.iter_mut() {
        if let Some(k) = *slot {
            if ctx.force_bed_fling || ctx.rng.f64() < BED_FLING_CHANCE {
                *slot = None;
                ctx.acts.push(Act::Fling(k, v.dir * FLING_V.0 * v.dpi, -FLING_V.1 * 0.7 * v.dpi));
            }
        }
    }
}

/// 쿵! 걷던 펫과 운전 펫 (짐칸 펫도) 이 튕겨 나가고 탈것은 뒤로 살짝 밀림
fn crash(v: &mut Vehicle, o: &Obstacle, ctx: &mut Ctx) {
    let dpi = v.dpi;
    let front = v.x + v.dir * v.half_len();
    ctx.events.push(VehicleEvent::Crash { id: v.id, x: front, y: v.y - 0.4 * v.height() });
    ctx.acts.push(Act::Bonk(o.key, v.dir * BONK_V.0 * dpi, -BONK_V.1 * dpi));
    if let Some(d) = v.driver.take() {
        ctx.acts.push(Act::Fling(d, v.dir * FLING_V.0 * dpi, -FLING_V.1 * dpi));
    }
    for k in v.bed.iter_mut() {
        if let Some(k) = k.take() {
            ctx.acts.push(Act::Fling(k, v.dir * FLING_V.0 * 0.8 * dpi, -FLING_V.1 * 0.9 * dpi));
        }
    }
    v.encounter = None;
    v.braking = false;
    v.stopping = false;
    v.dust = 0.5;
    v.bounce_mark();
    v.speed = 0.0;
    v.vx = -v.dir * NUDGE * dpi;
}

/// 🛻 가까운 한가한 펫을 짐칸에 부름 (빈자리만큼, 가까운 순)
fn invite(v: &Vehicle, ctx: &mut Ctx) {
    let seats = v.bed.iter().take(v.kind.bed_seats()).filter(|s| s.is_none()).count();
    let owner = v.on.map(|(o, _)| o);
    let mut near: Vec<&(u64, f64, f64, f64, Option<Owner>)> = ctx
        .free
        .iter()
        .filter(|f| Some(f.0) != v.driver && f.4 == owner && (f.2 - v.y).abs() < 4.0 && (f.1 - v.x).abs() <= BED_INVITE_REACH * f.3)
        .collect();
    near.sort_by(|a, b| (a.1 - v.x).abs().total_cmp(&(b.1 - v.x).abs()));
    for f in near.into_iter().take(seats) {
        ctx.acts.push(Act::Invite(f.0, v.id));
    }
}

/// 디딤판을 따라 d 쪽으로 step만큼: 끝이면 돌아서거나 (Edge는 가끔) 떨어지고, 옆 모니터로 이어지면 넘어가고,
/// 친구가 연결된 화면 끝이면 가끔 탈것째 나감. driving = 운전 중 (아니면 밀린 것 — 끝에서 그냥 멈춤)
fn move_along(v: &mut Vehicle, world: &World, d: f64, step: f64, ctx: &mut Ctx, driving: bool) {
    let Some((owner, _)) = v.on else { return };
    let Some(piece) = world.piece_at(owner, v.x).copied() else {
        v.start_fall();
        return;
    };
    let half = v.half_len();
    let margin = half * 0.8;
    let new_x = v.x + d * step;
    let (limit, end) = if d > 0.0 { (piece.x_end - margin, piece.end_kind) } else { (piece.x_start + margin, piece.start_kind) };
    let past = if d > 0.0 { new_x > limit } else { new_x < limit };
    if !past {
        v.x = new_x;
        reanchor(v, world);
        return;
    }
    if !driving {
        v.x = limit;
        v.speed = 0.0;
        v.vx = 0.0;
        reanchor(v, world);
        return;
    }
    let side = if d > 0.0 { Side::Right } else { Side::Left };
    let steered = ctx.steer.is_some();
    match end {
        EndKind::Seam { to } => {
            let edge = if d > 0.0 { piece.x_end } else { piece.x_start };
            let next = world.surfaces.iter().find(|s| s.owner == Owner::Ground(to) && s.contains_x(edge + d * 2.0) && (s.y - piece.y).abs() <= 2.0).copied();
            if let Some(next) = next {
                v.x = new_x;
                if (d > 0.0 && v.x > edge) || (d < 0.0 && v.x < edge) {
                    v.on = Some((next.owner, 0.0));
                    v.y = next.y;
                }
                reanchor(v, world);
                return;
            }
            road_end(v, limit, world, steered);
        }
        EndKind::Edge => {
            let drop = steered || *v.edge_roll.get_or_insert_with(|| ctx.rng.f64() < EDGE_DROP_CHANCE);
            if drop {
                v.x = new_x;
                let edge = if d > 0.0 { piece.x_end } else { piece.x_start };
                if (d > 0.0 && v.x > edge) || (d < 0.0 && v.x < edge) {
                    // 붕 — 창 끝에서 떨어짐 (계속 운전)
                    v.vx = d * v.speed;
                    v.vy = 0.0;
                    v.speed = 0.0;
                    v.start_fall();
                    v.phase = Phase::Air;
                    return;
                }
                reanchor(v, world);
                return;
            }
            road_end(v, limit, world, steered);
        }
        EndKind::Wall => {
            let wants = v.driver.is_some_and(|k| ctx.edges.iter().any(|&(key, l, r)| key == k && if side == Side::Left { l } else { r }));
            if wants && !v.stopping {
                let go = steered || *v.visit_roll.get_or_insert_with(|| ctx.force_visit || ctx.rng.f64() < VISIT_CHANCE);
                if go {
                    v.phase = Phase::Exiting(side);
                    v.on = None;
                    v.x = new_x;
                    return;
                }
            }
            road_end(v, limit, world, steered);
        }
        EndKind::Blocked => road_end(v, limit, world, steered),
    }
}

/// 끝에서 멈칫했다가 돌아섬
fn turn(v: &mut Vehicle, limit: f64, world: &World) {
    v.x = limit;
    v.speed = 0.0;
    v.pause = TURN_SECS;
    v.turn_after = true;
    reanchor(v, world);
}

/// 지금 x로 디딤판 기준점에서의 자리를 다시 적음
fn reanchor(v: &mut Vehicle, world: &World) {
    if let Some((owner, _)) = v.on {
        if let Some((ax, ay)) = world.anchor(owner) {
            v.on = Some((owner, v.x - ax));
            v.y = ay;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seats_hide_the_legs_and_keep_feet_above_the_floor() {
        // 청소년(128)은 다리 몫만큼, 아기(50)는 덜 가려짐 — 발은 seat_min 아래로 안 감
        let (_, feet, hidden) = seat_geometry(VehicleKind::Car, 0, 128.0);
        assert!(feet >= VehicleKind::Car.spec().seat_min);
        assert!(hidden > 0.0 && hidden <= LEG_SHARE * 128.0 + 1e-9);
        let (_, feet_baby, hidden_baby) = seat_geometry(VehicleKind::Car, 0, 50.0);
        assert!(feet_baby > feet);
        assert!((hidden_baby - LEG_SHARE * 50.0).abs() < 1e-9);
        // 킥보드는 서서 탐 (안 가림)
        assert_eq!(seat_geometry(VehicleKind::Scooter, 0, 128.0).2, 0.0);
        // 트럭 짐칸은 뒤쪽
        assert!(seat_geometry(VehicleKind::Truck, 1, 128.0).0 < 0.0);
    }

    /// 탈것은 제일 큰 단계 (어른) 펫에 딱 맞는 크기: 자동차 길이 ≈ 어른 펫 한 마리 (아기도 문 위로 보임)
    #[test]
    fn vehicles_are_sized_for_the_biggest_pets() {
        let adult = crate::brain::PET_LOGICAL_SIZE * crate::care::Stage::Adult.scale() * 0.85;
        let car = VehicleKind::Car.spec();
        assert!(car.w >= 0.9 * adult && car.w <= 1.15 * adult, "car {} vs adult {adult}", car.w);
        let (w, h) = window_logical(VehicleKind::Car);
        assert!((w - (176.0 + 48.0) * VEHICLE_SCALE).abs() < 1e-9 && (h - 128.0 * VEHICLE_SCALE).abs() < 1e-9, "창도 그림과 같이 줄어듦");
        let baby = crate::brain::PET_LOGICAL_SIZE * crate::care::Stage::Baby.scale() * 0.85;
        let (_, feet, hidden) = seat_geometry(VehicleKind::Car, 0, baby);
        assert!(hidden < 0.4 * baby && feet + baby - hidden > car.cover, "아기 머리가 문 위로 보임");
    }

    #[test]
    fn windows_fit_the_drawing_with_room_for_dust() {
        for k in VehicleKind::ALL {
            let (w, h) = window_logical(k);
            assert!(w > k.spec().w && h > k.spec().h);
        }
    }

    #[test]
    fn poses_serialize_for_the_webview() {
        let v = Vehicle::new(1, VehicleKind::Heli, 0.0, 0.0, 1.0, -1.0);
        let j = serde_json::to_value(v.pose()).unwrap();
        assert_eq!(j["dir"], -1);
        assert_eq!(j["rotor"], false);
        assert_eq!(serde_json::to_value(VehicleKind::Truck).unwrap(), "truck");
    }
}
