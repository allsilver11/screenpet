//! 물건 (쓰레기 장난감, phase 3): 뇌도 성장도 없는 물리 덩어리.
//! 스스로 움직이지 않고 — 가끔 부는 바람에만 조금 구른다 — 잡아 끌고 던질 수 있고, 친구가 연결된 화면 끝으로 세게 던지면 그 화면으로 날아간다.
//! 던지기 전에 포스트잇을 붙일 수 있다: 받는 사람이 누르면 10초 보이고 영영 사라진다.
//! 대부분의 시간엔 잠들어 있다 (`is_asleep`): 앱은 창도 웹뷰도 건드리지 않는다.

use std::collections::VecDeque;

use serde::Serialize;

use crate::physics::{clamp_speed, step_air, throw_velocity, Body, THROW_GRAVITY_LOGICAL, THROW_LIFT};
use crate::visit::{self, ExitEdges, Fling, Side};
use crate::surfaces::Owner;
use crate::world::World;

/// 물건 창 한 변 (논리 px). 그림은 128 칸에 그려 이 크기로 줄인다 (tools/draw_objects.py)
pub const OBJECT_LOGICAL_SIZE: f64 = 96.0;
/// 한 사람이 만들어 둘 수 있는 물건 수 — 넘치면 가장 오래된 것이 뿅 사라진다
pub const MAX_OWN_OBJECTS: usize = 5;
/// 친구에게서 받아 화면에 있을 수 있는 물건 수 — 넘치면 가장 오래된 것부터
pub const MAX_FOREIGN_OBJECTS: usize = 20;
/// 포스트잇을 눌러 읽는 시간 (초). 그 뒤엔 다시 볼 수 없다
pub const NOTE_READ_SECS: f64 = 10.0;
/// 바람이 부는 간격 (초, 물건마다 따로)
pub const GUST_EVERY_SECS: (f64, f64) = (30.0, 90.0);

/// 몸 폭 절반 / 창 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 바람에 밀려 가다 멈추는 마찰 (논리 px/s²) — 천천히 굴러간다
const GUST_FRICTION: f64 = 160.0;
/// 던져져 내려앉은 뒤 미끄러지다 멈추는 마찰 (논리 px/s²)
const LAND_FRICTION: f64 = 900.0;
/// 구르는 물건의 반지름 (논리 px) — 그림의 회전이 이동 거리와 맞는다
const ROLL_RADIUS: f64 = 14.0;
/// 납작한 물건이 뒤집힐 때 뜨는 높이 (논리 px)
const FLIP_LIFT: f64 = 10.0;
/// 이보다 세게 떨어지면 살짝 튀어 오른다 (논리 px/s)
const BOUNCE_MIN_VY: f64 = 350.0;
const BOUNCE_KEEP_VY: f64 = 0.35;
const BOUNCE_KEEP_VX: f64 = 0.75;
/// 날아가는 동안 도는 빠르기 (가로 논리 px/s당 도/s)
const AIR_SPIN: f64 = 0.5;
/// 그림이 제자리로 돌아오는 빠르기 (1/s)
const SETTLE_RATE: f64 = 10.0;
/// 놓을 때 커서 속도를 재는 구간 (초, 동물과 같다)
const VELOCITY_WINDOW: f64 = 0.12;
const OUTLIER_RATIO: f64 = 3.0;
/// 이만큼 안에서 이만큼 안 움직이고 놓으면 클릭 (논리 px, 초)
const CLICK_SLOP: f64 = 6.0;
const CLICK_SECS: f64 = 0.5;

/// 바람에 움직이는 모양
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GustStyle {
    /// 둥근 것: 데굴데굴 구른다
    Roll,
    /// 납작한 것: 미끄러지다 한 번 뒤집힌다
    Flip,
    /// 각진 것: 들썩들썩 흔들리며 조금 간다
    Rock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Can,
    Paper,
    Glasses,
    Computer,
    Banana,
    Milk,
    Sock,
    Umbrella,
}

impl ObjectKind {
    pub const ALL: [ObjectKind; 8] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Banana,
        ObjectKind::Milk,
        ObjectKind::Sock,
        ObjectKind::Umbrella,
    ];

    /// 통신과 그림 파일(ui/objects/<id>.png)에 쓰는 이름
    pub fn id(self) -> &'static str {
        match self {
            ObjectKind::Can => "can",
            ObjectKind::Paper => "paper",
            ObjectKind::Glasses => "glasses",
            ObjectKind::Computer => "computer",
            ObjectKind::Banana => "banana",
            ObjectKind::Milk => "milk",
            ObjectKind::Sock => "sock",
            ObjectKind::Umbrella => "umbrella",
        }
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Can => "🥫 찌그러진 캔",
            ObjectKind::Paper => "📄 구긴 종이",
            ObjectKind::Glasses => "👓 금 간 안경",
            ObjectKind::Computer => "💻 고장난 컴퓨터",
            ObjectKind::Banana => "🍌 바나나 껍질",
            ObjectKind::Milk => "🥛 빈 우유팩",
            ObjectKind::Sock => "🧦 찢어진 양말",
            ObjectKind::Umbrella => "☂️ 부러진 우산",
        }
    }

    /// 아는 이름만 (친구가 보낸 글자 — 그대로 파일 이름·창 스크립트에 쓰이므로 목록과 정확히 같아야 한다)
    pub fn from_id(id: &str) -> Option<ObjectKind> {
        ObjectKind::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn gust_style(self) -> GustStyle {
        match self {
            ObjectKind::Can | ObjectKind::Paper => GustStyle::Roll,
            ObjectKind::Banana | ObjectKind::Sock | ObjectKind::Glasses => GustStyle::Flip,
            ObjectKind::Computer | ObjectKind::Milk | ObjectKind::Umbrella => GustStyle::Rock,
        }
    }

    /// 한 번의 바람에 가는 거리 (논리 px, 최소·최대) — 무거운 것은 거의 안 간다
    fn gust_reach(self) -> (f64, f64) {
        match self {
            ObjectKind::Can => (80.0, 180.0),
            ObjectKind::Paper => (60.0, 170.0),
            ObjectKind::Sock => (40.0, 120.0),
            ObjectKind::Banana => (30.0, 90.0),
            ObjectKind::Glasses => (20.0, 60.0),
            ObjectKind::Milk => (30.0, 90.0),
            ObjectKind::Umbrella => (30.0, 80.0),
            ObjectKind::Computer => (6.0, 18.0),
        }
    }
}

/// 웹뷰가 그릴 모습: 회전(도), 위아래 뒤집기(1 = 그대로, -1 = 뒤집힘), 살짝 뜬 높이(논리 px)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Pose {
    pub angle: f64,
    pub flip: f64,
    pub lift: f64,
}

impl Default for Pose {
    fn default() -> Self {
        Pose { angle: 0.0, flip: 1.0, lift: 0.0 }
    }
}

/// 물건에 붙은 포스트잇
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Postit {
    #[default]
    None,
    /// 내가 붙였다 — 눌러도 읽히지 않고, 떼거나 던져 보낼 수 있다
    Mine,
    /// 받았다, 아직 안 읽음 — 누르면 읽는다
    Unread,
    /// 읽는 중 — 글자는 이미 웹뷰로 갔고 여기엔 남아 있지 않다
    Reading,
}

/// 포인터를 놓은 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    Click,
    Throw,
}

/// 던져서 화면 끝을 넘어갔다 (이 물건은 이 화면에서 끝난다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exit {
    pub side: Side,
    pub fling: Fling,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Gust {
    /// 갈 거리 (물리 px)와 간 거리
    total: f64,
    done: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Held,
    Air,
    /// 디딤판 위에서 미끄러지거나 구르거나 그림이 제자리로 돌아오는 중
    Ground { friction: f64, gust: Option<Gust> },
    /// 잠듦 — 다음 바람이나 세계 변화까지 할 일 없음
    Rest,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub kind: ObjectKind,
    /// x = 바닥 중심, y = 바닥 (화면 아래로 +)
    pub body: Body,
    dpi: f64,
    phase: Phase,
    /// 디딘 곳과 기준점에서 떨어진 거리
    standing: Option<(Owner, f64)>,
    pose: Pose,
    /// 던진 비행 — 이때만 화면 끝을 넘어간다 (바람·낙하는 넘어가지 않는다)
    thrown: bool,
    gust_in: f64,
    rng: fastrand::Rng,
    clock: f64,
    /// 잡은 자리 (몸 - 커서), 잡은 시각, 가장 멀리 움직인 거리, 커서 기록 (시각, x, y)
    grab: (f64, f64),
    grabbed_at: f64,
    moved: f64,
    grabbed_from: (f64, f64),
    samples: Vec<(f64, f64, f64)>,
    postit: Postit,
    note: Option<String>,
    read_left: f64,
    note_ended: bool,
}

impl Object {
    /// (x, y)에서 떨어지기 시작한다
    pub fn new(kind: ObjectKind, x: f64, y: f64, dpi: f64, seed: u64) -> Object {
        let mut rng = fastrand::Rng::with_seed(seed);
        let gust_in = next_gust(&mut rng);
        Object {
            kind,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            dpi: dpi.max(0.01),
            phase: Phase::Air,
            standing: None,
            pose: Pose::default(),
            thrown: false,
            gust_in,
            rng,
            clock: 0.0,
            grab: (0.0, 0.0),
            grabbed_at: 0.0,
            moved: 0.0,
            grabbed_from: (x, y),
            samples: Vec::new(),
            postit: Postit::None,
            note: None,
            read_left: 0.0,
            note_ended: false,
        }
    }

    /// 친구 화면에서 던져져 side 끝으로 날아 들어온다 (높이·속도를 이어 받는다). fling은 다듬어져 있어야 한다 (accept_toss)
    pub fn flown_in(kind: ObjectKind, world: &World, side: Side, fling: Fling, dpi: f64, seed: u64) -> Object {
        let mut o = Object::new(kind, 0.0, 0.0, dpi, seed);
        let (size, hw, dpi) = (o.size(), o.half_w(), o.dpi);
        let b = world.bounds;
        let g = THROW_GRAVITY_LOGICAL * dpi;
        o.body = if visit::is_vertical(side) {
            let x = visit::vertical_entry_x(world, side, Some(fling.height), hw, &mut o.rng);
            if side == Side::Up {
                Body { x, y: b.top + size, vx: fling.vx * dpi, vy: (fling.vy * dpi).max(0.0) }
            } else {
                // 아래 끝에서: 적어도 몸 둘 반은 튀어 오른다
                let pop = -(2.0 * g * 2.5 * size).sqrt();
                Body { x, y: b.bottom, vx: fling.vx * dpi, vy: (fling.vy * dpi).min(pop) }
            }
        } else {
            let (x, inward) = match side {
                Side::Left => (b.left + hw, 1.0),
                _ => (b.right - hw, -1.0),
            };
            let wa = world.work_area_at(x, b.bottom);
            let y = (wa.top + fling.height * wa.height()).clamp(wa.top + size, wa.bottom.max(wa.top + size));
            Body { x, y, vx: inward * fling.vx.abs() * dpi, vy: fling.vy * dpi }
        };
        o.thrown = true;
        o
    }

    fn size(&self) -> f64 {
        OBJECT_LOGICAL_SIZE * self.dpi
    }

    pub fn half_w(&self) -> f64 {
        self.size() * HALF_WIDTH_RATIO
    }

    pub fn set_dpi(&mut self, dpi: f64) {
        self.dpi = dpi.max(0.01);
    }

    pub fn dpi(&self) -> f64 {
        self.dpi
    }

    pub fn pose(&self) -> Pose {
        self.pose
    }

    pub fn is_held(&self) -> bool {
        self.phase == Phase::Held
    }

    /// 다음 바람이나 세계 변화까지 할 일이 없다 (창도 웹뷰도 건드리지 않는다)
    pub fn is_asleep(&self) -> bool {
        self.phase == Phase::Rest && self.postit != Postit::Reading
    }

    pub fn owner(&self) -> Option<Owner> {
        self.standing.map(|(o, _)| o)
    }

    /// 윗면에 앉아 있는 창
    pub fn ridden_window(&self) -> Option<u64> {
        match self.owner() {
            Some(Owner::Window(id) | Owner::Inside(id)) => Some(id),
            _ => None,
        }
    }

    // ---- 포인터 ----

    /// 잡았다 (cx, cy = 커서)
    pub fn pointer_down(&mut self, cx: f64, cy: f64) {
        self.phase = Phase::Held;
        self.standing = None;
        self.thrown = false;
        self.body.vx = 0.0;
        self.body.vy = 0.0;
        self.grab = (self.body.x - cx, self.body.y - cy);
        self.grabbed_at = self.clock;
        self.grabbed_from = (cx, cy);
        self.moved = 0.0;
        self.samples.clear();
        self.samples.push((self.clock, cx, cy));
    }

    pub fn pointer_move(&mut self, cx: f64, cy: f64) {
        if !self.is_held() {
            return;
        }
        self.body.x = cx + self.grab.0;
        self.body.y = cy + self.grab.1;
        self.moved = self.moved.max((cx - self.grabbed_from.0).hypot(cy - self.grabbed_from.1));
        self.samples.push((self.clock, cx, cy));
        let keep_from = self.clock - VELOCITY_WINDOW;
        self.samples.retain(|s| s.0 >= keep_from);
    }

    /// 놓았다: 거의 안 움직이고 금방 놓으면 클릭(그 자리에 내려앉는다), 아니면 던지기
    pub fn pointer_up(&mut self) -> Release {
        if !self.is_held() {
            return Release::Throw;
        }
        self.phase = Phase::Air;
        if self.moved < CLICK_SLOP * self.dpi && self.clock - self.grabbed_at < CLICK_SECS {
            return Release::Click;
        }
        let (vx, vy) = release_velocity(&self.samples);
        let (vx, vy) = throw_velocity(vx, vy, self.dpi, THROW_LIFT);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown = true;
        Release::Throw
    }

    // ---- 매 틱 ----

    /// gusts_ok = 바람이 불어도 된다 (모두 숨겨 둔 동안엔 아니다). 던져서 화면 끝을 넘어가면 Exit
    pub fn tick(&mut self, dt: f64, world: &World, edges: ExitEdges, gusts_ok: bool) -> Option<Exit> {
        self.clock += dt;
        self.tick_note(dt);
        match self.phase {
            Phase::Held => None,
            Phase::Air => self.tick_air(dt, world, edges),
            Phase::Ground { friction, gust } => {
                self.tick_ground(dt, world, friction, gust);
                None
            }
            Phase::Rest => {
                self.gust_in -= dt;
                if self.gust_in <= 0.0 {
                    self.gust_in = next_gust(&mut self.rng);
                    if gusts_ok {
                        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
                        self.start_gust(dir);
                    }
                }
                None
            }
        }
    }

    /// 바람이 분다 (dir = ±1). 잠들어 있을 때만
    pub fn start_gust(&mut self, dir: f64) {
        if self.phase != Phase::Rest {
            return;
        }
        let (lo, hi) = self.kind.gust_reach();
        let mut reach = lo + self.rng.f64() * (hi - lo);
        if self.kind.gust_style() == GustStyle::Roll {
            // 딱 몇 바퀴: 멈출 때 그림이 똑바로 선다
            let round = std::f64::consts::TAU * ROLL_RADIUS;
            reach = (reach / round).round().max(1.0) * round;
        }
        let total = reach * self.dpi;
        let friction = GUST_FRICTION * self.dpi;
        self.body.vx = dir * (2.0 * friction * total).sqrt();
        self.phase = Phase::Ground { friction, gust: Some(Gust { total, done: 0.0 }) };
    }

    fn start_fall(&mut self) {
        self.standing = None;
        self.body.vy = 0.0;
        self.thrown = false;
        self.phase = Phase::Air;
    }

    fn tick_ground(&mut self, dt: f64, world: &World, friction: f64, mut gust: Option<Gust>) {
        let anchor = self.standing.and_then(|(owner, rel_x)| world.anchor(owner).map(|(ax, _)| (owner, rel_x, ax)));
        let Some((owner, rel_x, ax)) = anchor else {
            self.start_fall();
            return;
        };
        let v0 = self.body.vx;
        let mut v = (v0.abs() - friction * dt).max(0.0) * v0.signum();
        if v0 == 0.0 {
            v = 0.0;
        }
        let (lo, hi) = (world.bounds.left + self.half_w(), world.bounds.right - self.half_w());
        let start = ax + rel_x;
        let mut x = start + v * dt;
        if x < lo || x > hi {
            x = x.clamp(lo, hi.max(lo));
            v = 0.0;
        }
        self.body.x = x;
        self.body.vx = v;
        let Some(piece) = world.piece_at(owner, x).copied() else {
            // 디딤판 끝을 지나 떨어진다 (바람 때문이어도 — 화면 끝은 위에서 막았다)
            self.start_fall();
            return;
        };
        self.body.y = piece.y;
        self.standing = Some((owner, x - ax));
        if let Some(g) = gust.as_mut() {
            g.done += (x - start).abs();
            self.gust_pose(*g, x - start);
            if v == 0.0 {
                gust = None;
            }
        } else {
            self.settle_pose(dt);
        }
        if v == 0.0 && gust.is_none() && self.pose == Pose::default() {
            self.phase = Phase::Rest;
        } else {
            self.phase = Phase::Ground { friction, gust };
        }
    }

    fn gust_pose(&mut self, g: Gust, dx: f64) {
        let p = (g.done / g.total.max(1.0)).clamp(0.0, 1.0);
        let tau = std::f64::consts::TAU;
        match self.kind.gust_style() {
            GustStyle::Roll => self.pose.angle += (dx / (ROLL_RADIUS * self.dpi)).to_degrees(),
            GustStyle::Flip => {
                self.pose.flip = (tau * p).cos();
                self.pose.lift = (tau / 2.0 * p).sin() * FLIP_LIFT;
            }
            GustStyle::Rock => {
                // 멀리 가는 것일수록 크게 흔들린다
                let amp = (self.kind.gust_reach().1 / 4.0).clamp(4.0, 25.0);
                self.pose.angle = (2.0 * tau * p).sin() * amp * (1.0 - p);
                self.pose.lift = (tau * p).sin().abs() * amp / 3.0;
            }
        }
    }

    /// 그림을 제자리로 (가까운 쪽으로 돌아간다)
    fn settle_pose(&mut self, dt: f64) {
        let keep = (-SETTLE_RATE * dt).exp();
        let a = (self.pose.angle + 180.0).rem_euclid(360.0) - 180.0;
        self.pose.angle = if (a * keep).abs() < 0.5 { 0.0 } else { a * keep };
        self.pose.lift = if self.pose.lift * keep < 0.2 { 0.0 } else { self.pose.lift * keep };
        let flip = 1.0 - (1.0 - self.pose.flip) * keep;
        self.pose.flip = if flip > 0.98 { 1.0 } else { flip };
    }

    fn tick_air(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if let Some(exit) = self.fling_out(dt, world, edges) {
            return Some(exit);
        }
        let prev_y = self.body.y;
        let g = THROW_GRAVITY_LOGICAL * self.dpi;
        let (size, hw) = (self.size(), self.half_w());
        step_air(&mut self.body, dt, &world.bounds, hw, size, g);
        self.pose.angle += self.body.vx / self.dpi * AIR_SPIN * dt;
        let landing = world.landing(self.body.x, prev_y, self.body.y).copied().or_else(|| {
            // 이미 작업 영역 바닥보다 아래로 내려간 경우(작업표시줄 위에 놓음)엔 Ground를 바닥으로 취급
            if self.body.vy >= 0.0 {
                world.ground_below(self.body.x, self.body.y).copied()
            } else {
                None
            }
        });
        if let Some(s) = landing {
            self.body.y = s.y;
            self.thrown = false;
            if self.body.vy > BOUNCE_MIN_VY * self.dpi {
                self.body.vy *= -BOUNCE_KEEP_VY;
                self.body.vx *= BOUNCE_KEEP_VX;
                return None;
            }
            self.body.vy = 0.0;
            let ax = world.anchor(s.owner).map_or(0.0, |(ax, _)| ax);
            self.standing = Some((s.owner, self.body.x - ax));
            self.phase = Phase::Ground { friction: LAND_FRICTION * self.dpi, gust: None };
        } else if self.body.y > world.bounds.bottom + size * 4.0 {
            // 화면 밖으로 사라짐 → 위에서 다시 떨어진다
            let b = world.bounds;
            self.body = Body { x: self.body.x.clamp(b.left + hw, (b.right - hw).max(b.left + hw)), y: b.top + size, vx: 0.0, vy: 0.0 };
        }
        None
    }

    /// 던진 비행이 친구가 연결된 화면 끝을 넘으면 그대로 넘어간다 (동물과 같은 규칙, brain_fling.rs)
    fn fling_out(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if !self.thrown {
            return None;
        }
        let b = world.bounds;
        let (hw, size, dpi) = (self.half_w(), self.size(), self.dpi);
        let Body { x, y, vx, vy } = self.body;
        let (next_x, next_y) = (x + vx * dt, y + vy * dt);
        let exit = if next_x < b.left + hw && vx < 0.0 && edges.left {
            Some(Side::Left)
        } else if next_x > b.right - hw && vx > 0.0 && edges.right {
            Some(Side::Right)
        } else if vy < 0.0 && next_y < b.top + size && edges.up && visit::outer_edge_at(world, Side::Up, x) {
            Some(Side::Up)
        } else if vy >= visit::FLING_DOWN_MIN_SPEED * dpi && y <= b.bottom && next_y >= b.bottom && edges.down && visit::outer_edge_at(world, Side::Down, x) {
            Some(Side::Down)
        } else {
            None
        };
        let side = exit?;
        let height = if visit::is_vertical(side) {
            visit::edge_fraction(&b, x)
        } else {
            let wa = world.work_area_at(x, y);
            if wa.height() > 0.0 {
                (y - wa.top) / wa.height()
            } else {
                1.0
            }
        };
        let fling = Fling { height, vx: vx / dpi, vy: vy / dpi }.sanitized()?;
        self.thrown = false;
        Some(Exit { side, fling })
    }

    /// 세계가 바뀌었다 (창이 움직임·사라짐 등): 앉은 창을 따라가거나 떨어지기 시작한다. 잠든 물건에만 일이 있다. 움직였으면 true
    pub fn reseat(&mut self, world: &World) -> bool {
        if self.phase != Phase::Rest {
            return false;
        }
        let Some((owner, rel_x)) = self.standing else {
            self.start_fall();
            return true;
        };
        let Some((ax, _)) = world.anchor(owner) else {
            self.start_fall();
            return true;
        };
        let x = ax + rel_x;
        let Some(piece) = world.piece_at(owner, x).copied() else {
            self.body.x = x;
            self.start_fall();
            return true;
        };
        let moved = x != self.body.x || piece.y != self.body.y;
        self.body.x = x;
        self.body.y = piece.y;
        moved
    }

    // ---- 포스트잇 ----

    pub fn postit(&self) -> Postit {
        self.postit
    }

    /// 물건에 노란 쪽지 그림이 붙어 있다
    pub fn has_sticker(&self) -> bool {
        matches!(self.postit, Postit::Mine | Postit::Unread)
    }

    /// 내가 쓴 포스트잇을 붙인다 (다듬은 글자가 비었거나 친구 것이 붙어 있으면 false)
    pub fn attach_note(&mut self, raw: &str) -> bool {
        if !matches!(self.postit, Postit::None | Postit::Mine) {
            return false;
        }
        let Some(note) = visit::sanitize_note(raw) else { return false };
        self.note = Some(note);
        self.postit = Postit::Mine;
        true
    }

    /// 내가 붙인 포스트잇을 뗀다
    pub fn peel_note(&mut self) -> bool {
        if self.postit != Postit::Mine {
            return false;
        }
        self.note = None;
        self.postit = Postit::None;
        true
    }

    /// 친구가 붙여 보낸 포스트잇 (accept_toss로 다듬은 글자)
    pub fn receive_note(&mut self, note: Option<String>) {
        self.postit = if note.is_some() { Postit::Unread } else { Postit::None };
        self.note = note;
    }

    /// 눌렀다: 받은 포스트잇이면 글자를 딱 한 번 내주고 10초를 재기 시작한다. 글자는 여기서 지운다 — 다시 읽을 수 없다
    pub fn read_note(&mut self) -> Option<String> {
        if self.postit != Postit::Unread {
            return None;
        }
        self.postit = Postit::Reading;
        self.read_left = NOTE_READ_SECS;
        self.note.take()
    }

    /// 던져 보낼 때 같이 갈 글자 (읽기 시작한 포스트잇은 따라가지 않는다)
    pub fn note_to_send(&self) -> Option<String> {
        if self.has_sticker() {
            self.note.clone()
        } else {
            None
        }
    }

    fn tick_note(&mut self, dt: f64) {
        if self.postit != Postit::Reading {
            return;
        }
        self.read_left -= dt;
        if self.read_left <= 0.0 {
            self.postit = Postit::None;
            self.note_ended = true;
        }
    }

    /// 읽는 10초가 끝났다 (한 번만 참)
    pub fn take_note_ended(&mut self) -> bool {
        std::mem::take(&mut self.note_ended)
    }
}

fn next_gust(rng: &mut fastrand::Rng) -> f64 {
    GUST_EVERY_SECS.0 + rng.f64() * (GUST_EVERY_SECS.1 - GUST_EVERY_SECS.0)
}

/// 놓을 때의 커서 속도: 같은 틱의 기록은 마지막 것만, 튀는 첫·끝 구간 하나는 뺀다 (brain.rs Pet::release_velocity와 같은 계산)
fn release_velocity(samples: &[(f64, f64, f64)]) -> (f64, f64) {
    let mut pts: Vec<(f64, f64, f64)> = Vec::with_capacity(samples.len());
    for &s in samples {
        match pts.last_mut() {
            Some(last) if s.0 - last.0 < 1e-9 => *last = s,
            _ => pts.push(s),
        }
    }
    let speed = |a: (f64, f64, f64), b: (f64, f64, f64)| (b.1 - a.1).hypot(b.2 - a.2) / (b.0 - a.0);
    let (mut lo, mut hi) = (0, pts.len().saturating_sub(1));
    if pts.len() >= 4 {
        let segs: Vec<f64> = pts.windows(2).map(|w| speed(w[0], w[1])).collect();
        let n = segs.len();
        let median_of = |skip: usize| {
            let mut rest: Vec<f64> = segs.iter().enumerate().filter(|&(i, _)| i != skip).map(|(_, &v)| v).collect();
            rest.sort_by(f64::total_cmp);
            rest[rest.len() / 2]
        };
        let (first, last) = (segs[0], segs[n - 1]);
        let first_out = first > OUTLIER_RATIO * median_of(0);
        let last_out = last > OUTLIER_RATIO * median_of(n - 1);
        if last_out && (!first_out || last >= first) {
            hi -= 1;
        } else if first_out {
            lo += 1;
        }
    }
    let (Some(&(t0, x0, y0)), Some(&(t1, x1, y1))) = (pts.get(lo), pts.get(hi)) else {
        return (0.0, 0.0);
    };
    let dt = t1 - t0;
    if dt < 1e-6 {
        return (0.0, 0.0);
    }
    ((x1 - x0) / dt, (y1 - y0) / dt)
}

/// 친구가 던져 보낸 물건을 다듬은 것
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedToss {
    pub kind: ObjectKind,
    pub fling: Fling,
    pub note: Option<String>,
}

/// fling이 없거나 이상할 때: 옆에서 살짝 포물선으로 들어온다
const DEFAULT_FLING: Fling = Fling { height: 0.4, vx: 400.0, vy: -200.0 };

/// 친구가 보낸 것을 다듬는다: 모르는 종류는 버리고, 움직임은 범위 안으로, 글자는 편지처럼 (보이지 않는 글자 없이 80자까지)
pub fn accept_toss(kind: &str, fling: Option<Fling>, note: Option<&str>) -> Option<AcceptedToss> {
    let kind = ObjectKind::from_id(kind)?;
    let fling = fling.and_then(Fling::sanitized).unwrap_or(DEFAULT_FLING);
    let (vx, vy) = clamp_speed(fling.vx, fling.vy, visit::MAX_FLING_SPEED);
    let note = note.and_then(visit::sanitize_note);
    Some(AcceptedToss { kind, fling: Fling { height: fling.height, vx, vy }, note })
}

/// 물건이 어디서 왔나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// 내가 만들었다 (최대 5개)
    Own,
    /// 친구에게서 날아왔다 (최대 20개)
    Foreign,
}

/// 화면 위 물건 수 지키기: 넘치면 그 쪽에서 가장 오래된 것을 내보낸다
#[derive(Debug, Default)]
pub struct Roster {
    own: VecDeque<u64>,
    foreign: VecDeque<u64>,
}

impl Roster {
    /// id를 더한다. 넘쳐서 치워야 할 물건이 있으면 돌려준다
    pub fn add(&mut self, id: u64, origin: Origin) -> Option<u64> {
        let (list, max) = match origin {
            Origin::Own => (&mut self.own, MAX_OWN_OBJECTS),
            Origin::Foreign => (&mut self.foreign, MAX_FOREIGN_OBJECTS),
        };
        list.push_back(id);
        if list.len() > max {
            list.pop_front()
        } else {
            None
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        for list in [&mut self.own, &mut self.foreign] {
            if let Some(i) = list.iter().position(|&x| x == id) {
                list.remove(i);
                return true;
            }
        }
        false
    }

    pub fn own_count(&self) -> usize {
        self.own.len()
    }

    pub fn foreign_count(&self) -> usize {
        self.foreign.len()
    }

    pub fn clear(&mut self) {
        self.own.clear();
        self.foreign.clear();
    }
}

/// 받았지만 아직 창을 만들지 않은 물건 — 앱이 틱마다 하나씩 꺼낸다 (창 만들기는 메인 스레드를 기다려서, 한꺼번에 오면 모든 동물이 멈춘다).
/// 기다리는 것까지 합쳐 받은 물건 20개다: 넘치면 가장 오래된 것이 먼저 — 이미 화면에 있는 물건, 없으면 가장 오래 기다린 것
#[derive(Debug)]
pub struct TossInbox<T> {
    queue: VecDeque<T>,
}

impl<T> Default for TossInbox<T> {
    fn default() -> Self {
        Self { queue: VecDeque::new() }
    }
}

impl<T> TossInbox<T> {
    /// 줄에 세운다. 화면의 물건을 치워야 하면 그 id를 돌려준다 (로스터에서는 이미 뺐다)
    pub fn push(&mut self, item: T, roster: &mut Roster) -> Option<u64> {
        let mut evicted = None;
        if roster.foreign_count() + self.queue.len() >= MAX_FOREIGN_OBJECTS {
            evicted = roster.foreign.pop_front();
            if evicted.is_none() {
                self.queue.pop_front();
            }
        }
        self.queue.push_back(item);
        evicted
    }

    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

/// 친구 한 명에게 던져 보내는 예산: 10개까지 한 번에, 그 뒤엔 6초에 하나 (받는 쪽 net의 제한과 같다 — 넘치면 받는 쪽이 버리므로 미리 튕긴다)
#[derive(Debug, Clone, Copy)]
pub struct TossBudget {
    tokens: f64,
    at: f64,
}

const TOSS_BURST: f64 = 10.0;
const TOSS_EVERY_SECS: f64 = 6.0;

impl Default for TossBudget {
    fn default() -> Self {
        TossBudget { tokens: TOSS_BURST, at: 0.0 }
    }
}

impl TossBudget {
    fn refilled(&self, now: f64) -> f64 {
        (self.tokens + (now - self.at).max(0.0) / TOSS_EVERY_SECS).min(TOSS_BURST)
    }

    pub fn can_spend(&self, now: f64) -> bool {
        self.refilled(now) >= 1.0
    }

    pub fn spend(&mut self, now: f64) {
        self.tokens = (self.refilled(now) - 1.0).max(0.0);
        self.at = now;
    }
}

#[cfg(test)]
#[path = "object_tests.rs"]
mod tests;
