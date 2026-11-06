//! 물건 (쓰레기 장난감, phase 3): 뇌도 성장도 없는 물리 덩어리.
//! 스스로 움직이지 않고 — 가끔 부는 바람에만 조금 구른다 — 잡아 끌고 던질 수 있고, 친구가 연결된 화면 끝으로 세게 던지면 그 화면으로 날아간다.
//! 던지면 제 모양 그대로 빙글빙글 돌며 날아가고, 떨어진 뒤에도 굴러가다 마찰로 멈춰 가까운 면으로 눕는다 (모양과 무게는 tumble.rs).
//! 던지기 전에 포스트잇을 붙일 수 있다: 받는 사람이 누르면 10초 보이고 영영 사라진다.
//! 대부분의 시간엔 잠들어 있다 (`is_asleep`): 앱은 창도 웹뷰도 건드리지 않는다.

use std::collections::VecDeque;

use crate::collide::{rebound, Face};
use crate::physics::{clamp_speed, step_air, throw_velocity, Body, THROW_GRAVITY_LOGICAL, THROW_LIFT, VELOCITY_WINDOW};
use crate::tumble::{self, heft_of, shape_of, throw_spin, wobble_spin};
use crate::visit::{self, ExitEdges, Fling, Side};
use crate::surfaces::Owner;
use crate::world::World;

/// 물건 창 한 변 (논리 px). 그림은 128 칸에 그려 이 크기로 줄인다 (tools/draw_objects.py)
pub const OBJECT_LOGICAL_SIZE: f64 = 96.0;
/// 한 사람이 만들어 둘 수 있는 물건 수 — 넘치면 가장 오래된 것이 뿅 사라진다
pub const MAX_OWN_OBJECTS: usize = 10;
/// 친구에게서 받아 화면에 있을 수 있는 물건 수 — 넘치면 가장 오래된 것부터
pub const MAX_FOREIGN_OBJECTS: usize = 20;
/// 화면에 있을 수 있는 똥 수 (내 동물·손님·친구가 던진 것 모두) — 넘치면 가장 오래된 똥이 사라진다. 쓰레기 수와 따로 센다
pub const MAX_POOPS: usize = 10;
/// 포스트잇을 눌러 읽는 시간 (초). 그 뒤엔 다시 볼 수 없다
pub const NOTE_READ_SECS: f64 = 10.0;
/// 바람이 부는 간격 (초, 물건마다 따로)
pub const GUST_EVERY_SECS: (f64, f64) = (30.0, 90.0);

/// 몸 폭 절반 / 창 크기
const HALF_WIDTH_RATIO: f64 = 0.3;
/// 이보다 세게 떨어지면 살짝 튀어 오른다 (논리 px/s)
const BOUNCE_MIN_VY: f64 = 350.0;
/// 빗자루에 쓸린 똥의 최소 가로 속도와 뜨는 속도 (논리 px/s)
const SWEEP_MIN_VX: f64 = 240.0;
const SWEEP_POP: f64 = 160.0;
/// 이만큼 안에서 이만큼 안 움직이고 놓으면 클릭 (논리 px, 초)
const CLICK_SLOP: f64 = 6.0;
const CLICK_SECS: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    Can,
    Paper,
    Stone,
    Glasses,
    Computer,
    Milk,
    Shirt,
    Umbrella,
    /// 💩 동물이 싼 똥 (poop.rs): 둘둘 쌓인 큰 똥, 작은 똥, 토끼의 알갱이. 메뉴에 없고 포스트잇을 못 붙인다
    Poop,
    Pink,
    Rainbow,
    Pellets,
}

impl ObjectKind {
    pub const ALL: [ObjectKind; 12] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Stone,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Milk,
        ObjectKind::Shirt,
        ObjectKind::Umbrella,
        ObjectKind::Poop,
        ObjectKind::Pink,
        ObjectKind::Rainbow,
        ObjectKind::Pellets,
    ];
    /// `물건 ▸` 메뉴로 꺼내는 쓰레기
    pub const TRASH: [ObjectKind; 8] = [
        ObjectKind::Can,
        ObjectKind::Paper,
        ObjectKind::Stone,
        ObjectKind::Glasses,
        ObjectKind::Computer,
        ObjectKind::Milk,
        ObjectKind::Shirt,
        ObjectKind::Umbrella,
    ];

    /// 똥이다: 수를 따로 세고, 저절로 사라지고, 바람에 구르지 않고, 빗자루에 쓸린다
    pub fn is_poop(self) -> bool {
        matches!(self, ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets)
    }

    /// 그림 크기 배율: 쓰레기는 96 논리 px 칸의 절반 (똥은 싼 동물을 따라 set_scale로 정한다)
    pub fn base_scale(self) -> f64 {
        if self.is_poop() {
            1.0
        } else {
            0.5
        }
    }

    /// 통신과 그림 파일(ui/objects/<id>.png)에 쓰는 이름
    pub fn id(self) -> &'static str {
        match self {
            ObjectKind::Can => "can",
            ObjectKind::Paper => "paper",
            ObjectKind::Stone => "stone",
            ObjectKind::Glasses => "glasses",
            ObjectKind::Computer => "computer",
            ObjectKind::Milk => "milk",
            ObjectKind::Shirt => "shirt",
            ObjectKind::Umbrella => "umbrella",
            ObjectKind::Poop => "poop",
            ObjectKind::Pink => "pink",
            ObjectKind::Rainbow => "rainbow",
            ObjectKind::Pellets => "pellets",
        }
    }

    /// 메뉴 글자
    pub fn label(self) -> &'static str {
        match self {
            ObjectKind::Can => "🥫 찌그러진 캔",
            ObjectKind::Paper => "📰 구겨진 신문지",
            ObjectKind::Stone => "🪨 돌멩이",
            ObjectKind::Glasses => "👓 금 간 안경",
            ObjectKind::Computer => "💻 고장난 컴퓨터",
            ObjectKind::Milk => "🥛 빈 우유팩",
            ObjectKind::Shirt => "👕 누더기 티셔츠",
            ObjectKind::Umbrella => "☂️ 부러진 우산",
            ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets => "💩 똥",
        }
    }

    /// 아는 이름만 (친구가 보낸 글자 — 그대로 파일 이름·창 스크립트에 쓰이므로 목록과 정확히 같아야 한다)
    pub fn from_id(id: &str) -> Option<ObjectKind> {
        if id == "dropping" {
            // 예전의 초록 똥 (옛 버전 친구가 보낸다)
            return Some(ObjectKind::Poop);
        }
        ObjectKind::ALL.into_iter().find(|k| k.id() == id || k.wire_id() == id)
    }

    /// 친구가 던진 이름과 색 표시 → 종류 (색은 똥에만, 모르는 색은 그냥 똥)
    pub fn wire_kind(id: &str, look: Option<&str>) -> Option<ObjectKind> {
        match (ObjectKind::from_id(id)?, look) {
            (ObjectKind::Poop, Some("pink")) => Some(ObjectKind::Pink),
            (ObjectKind::Poop, Some("rainbow")) => Some(ObjectKind::Rainbow),
            (kind, _) => Some(kind),
        }
    }

    /// 똥 색 (Toss::look으로 보낸다 — 옛 버전 친구는 몰라서 갈색 똥으로 받는다)
    pub fn look(self) -> Option<&'static str> {
        match self {
            ObjectKind::Pink => Some("pink"),
            ObjectKind::Rainbow => Some("rainbow"),
            _ => None,
        }
    }

    /// 친구에게 던질 때의 이름: 티셔츠는 양말 자리를 물려받아, 옛 버전 친구는 양말로 받는다
    pub fn wire_id(self) -> &'static str {
        // 옛 버전 친구는 무지개를 몰라 그냥 똥으로 받는다 (Toss::rainbow로 알린다)
        if self == ObjectKind::Shirt {
            "sock"
        } else if self.look().is_some() {
            "poop"
        } else {
            self.id()
        }
    }

    /// 한 번의 바람에 가는 거리 (논리 px, 최소·최대) — 무거운 것은 거의 안 간다
    fn gust_reach(self) -> (f64, f64) {
        match self {
            ObjectKind::Can => (80.0, 180.0),
            ObjectKind::Paper => (60.0, 170.0),
            ObjectKind::Shirt => (40.0, 120.0),
            ObjectKind::Glasses => (20.0, 60.0),
            ObjectKind::Milk => (30.0, 90.0),
            ObjectKind::Umbrella => (30.0, 80.0),
            ObjectKind::Stone => (8.0, 24.0),
            ObjectKind::Computer => (6.0, 18.0),
            // 바닥에 붙어 있다 (start_gust가 막는다)
            ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets => (0.0, 0.0),
        }
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
    /// 똥: 포스트잇을 붙일 수 없다 (저절로 사라지면 받은 글자도 사라지니까)
    Never,
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
enum Phase {
    Held,
    Air,
    /// 디딤판 위: settle이 없으면 구르는 중, 있으면 그 각도로 눕는 중 (마찰은 논리 px/s² × dpi)
    Ground { friction: f64, settle: Option<f64> },
    /// 잠듦 — 다음 바람이나 세계 변화까지 할 일 없음
    Rest,
}

#[derive(Debug, Clone)]
pub struct Object {
    /// 그림 크기 배율 (1 = 96 논리 px 칸)
    scale: f64,
    pub kind: ObjectKind,
    /// x = 바닥 중심, y = 바닥 (화면 아래로 +)
    pub body: Body,
    dpi: f64,
    phase: Phase,
    /// 디딘 곳과 기준점에서 떨어진 거리
    standing: Option<(Owner, f64)>,
    /// 그림 각도 (도, 시계 방향이 +)와 회전 (도/s). 잠들 때 각도를 한 바퀴 안으로 접는다
    angle: f64,
    spin: f64,
    /// 던진 비행 — 이때만 화면 끝을 넘어간다 (바람·낙하는 넘어가지 않는다)
    thrown: bool,
    /// 놓는 순간 아래로 던졌다 (visit::aimed_down) — 바닥에 닿으면 넘어간다
    thrown_down: bool,
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
    /// 그림 크기 배율 (똥은 싼 동물 크기를 따른다, 웹뷰도 같은 값으로 그린다)
    pub fn set_scale(&mut self, scale: f64) {
        self.scale = scale;
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn new(kind: ObjectKind, x: f64, y: f64, dpi: f64, seed: u64) -> Object {
        let mut rng = fastrand::Rng::with_seed(seed);
        let gust_in = next_gust(&mut rng);
        Object {
            kind,
            body: Body { x, y, vx: 0.0, vy: 0.0 },
            dpi: dpi.max(0.01),
            scale: kind.base_scale(),
            phase: Phase::Air,
            standing: None,
            angle: 0.0,
            spin: 0.0,
            thrown: false,
            thrown_down: false,
            gust_in,
            rng,
            clock: 0.0,
            grab: (0.0, 0.0),
            grabbed_at: 0.0,
            moved: 0.0,
            grabbed_from: (x, y),
            samples: Vec::new(),
            postit: if kind.is_poop() { Postit::Never } else { Postit::None },
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
        // 회전은 통신으로 오지 않는다: 들어오는 속도로 새로 돈다
        o.spin = o.spin_of_throw();
        o
    }

    /// 지금 속도로 던져졌을 때의 회전
    fn spin_of_throw(&mut self) -> f64 {
        let dir = if self.rng.bool() { 1.0 } else { -1.0 };
        throw_spin(self.kind, self.body.vx / self.dpi, self.body.vy / self.dpi, dir)
    }

    fn size(&self) -> f64 {
        OBJECT_LOGICAL_SIZE * self.dpi * self.scale
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

    /// 그림 각도 (도, 시계 방향이 +)
    pub fn angle(&self) -> f64 {
        self.angle
    }

    /// 회전 (도/s)
    pub fn spin(&self) -> f64 {
        self.spin
    }

    pub fn is_held(&self) -> bool {
        self.phase == Phase::Held
    }

    /// 땅에서 구르는 중 (눕기 전)
    #[cfg(test)]
    pub fn is_rolling(&self) -> bool {
        matches!(self.phase, Phase::Ground { settle: None, .. })
    }

    /// 던져져 날아가는 중 (동물에 부딪힐 수 있다). 떨어지거나 구르거나 들린 것은 아니다
    pub fn in_flight(&self) -> bool {
        self.phase == Phase::Air && self.thrown
    }

    /// 그려진 크기의 부딪힘 상자 (반 폭, 높이, 물리 px) — 똥은 작게 그려진다
    pub fn hit_box(&self) -> (f64, f64) {
        let hw = self.half_w();
        (hw, 2.0 * hw)
    }

    /// 동물에 부딪혀 되튄다 (away = 맞은 몸에서 멀어지는 쪽). 비행은 이어지지만 아래 친구를 노린 것은 잊는다
    pub fn bounce_off(&mut self, face: Face, away: f64) {
        let (vx, vy) = rebound(face, self.body.vx, self.body.vy, away, self.dpi);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown_down = false;
        self.spin *= -tumble::WALL_SPIN_KEEP;
    }

    /// 그림이 도는 축의 화면 자리 (물리 px): 창은 이곳을 가운데로 놓는다. 가장 낮은 점이 언제나 body.y에 온다
    pub fn pivot(&self) -> (f64, f64) {
        let shape = shape_of(self.kind);
        (self.body.x + shape.pivot_offset_x() * self.scale * self.dpi, self.body.y - shape.drop_at(self.angle) * self.scale * self.dpi)
    }

    /// 다음 바람이나 세계 변화까지 할 일이 없다 (창도 웹뷰도 건드리지 않는다)
    pub fn is_asleep(&self) -> bool {
        self.phase == Phase::Rest && self.postit != Postit::Reading
    }

    pub fn owner(&self) -> Option<Owner> {
        self.standing.map(|(o, _)| o)
    }

    /// 똥이 다 말라 사라질 때다 (들고 있는 동안은 기다린다)
    pub fn expired(&self) -> bool {
        self.kind.is_poop() && self.clock >= crate::poop::LIFE_SECS && !self.is_held()
    }

    /// 바닥에 놓인 똥의 자리 (동물이 알아챈다). 들렸거나 날아가는 중이면 None
    pub fn poop_spot(&self, id: u64) -> Option<crate::poop::PoopSpot> {
        if !self.kind.is_poop() || !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) {
            return None;
        }
        Some(crate::poop::PoopSpot { id, x: self.body.x, y: self.body.y, owner: self.owner()? })
    }

    /// 빗자루(vx, vy = 빗자루 속도, 물리 px/s)가 바닥의 똥을 쓸었다: 그 쪽으로 던진 것과 같다 — 친구가 연결된 화면 끝을 넘으면 평소처럼 날아간다.
    /// 살살 쓸어도 조금은 밀리고 살짝 뜬다. 날아가는 중이거나 들려 있으면 그대로 (내려앉아야 다시 쓸린다). 쓸렸으면 true
    pub fn sweep(&mut self, vx: f64, vy: f64) -> bool {
        if !self.kind.is_poop() || !matches!(self.phase, Phase::Ground { .. } | Phase::Rest) || (vx == 0.0 && vy == 0.0) {
            return false;
        }
        let (mut vx, mut vy) = throw_velocity(vx, vy, self.dpi, THROW_LIFT);
        if vx != 0.0 && vx.abs() < SWEEP_MIN_VX * self.dpi {
            vx = vx.signum() * SWEEP_MIN_VX * self.dpi;
        }
        self.thrown_down = visit::aimed_down(vx, vy, self.dpi);
        if !self.thrown_down {
            vy = vy.min(-SWEEP_POP * self.dpi);
        }
        self.body.vx = vx;
        self.body.vy = vy;
        self.standing = None;
        self.thrown = true;
        self.phase = Phase::Air;
        self.spin = self.spin_of_throw();
        true
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
        self.spin = 0.0;
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
        let (vx, vy) = crate::physics::release_velocity(self.samples.iter());
        let (vx, vy) = throw_velocity(vx, vy, self.dpi, THROW_LIFT);
        self.body.vx = vx;
        self.body.vy = vy;
        self.thrown = true;
        self.thrown_down = visit::aimed_down(vx, vy, self.dpi);
        self.spin = self.spin_of_throw();
        Release::Throw
    }

    // ---- 매 틱 ----

    /// gusts_ok = 바람이 불어도 된다 (object::gusts_ok). 던져서 화면 끝을 넘어가면 Exit
    pub fn tick(&mut self, dt: f64, world: &World, edges: ExitEdges, gusts_ok: bool) -> Option<Exit> {
        self.clock += dt;
        self.tick_note(dt);
        match self.phase {
            Phase::Held => None,
            Phase::Air => self.tick_air(dt, world, edges),
            Phase::Ground { friction, settle } => {
                self.tick_ground(dt, world, friction, settle);
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

    /// 바람이 분다 (dir = ±1). 잠들어 있을 때만. 던졌을 때와 같은 구르기를 살살
    pub fn start_gust(&mut self, dir: f64) {
        if self.phase != Phase::Rest || self.kind.is_poop() {
            return;
        }
        let heft = heft_of(self.kind);
        let (lo, hi) = self.kind.gust_reach();
        let reach = lo + self.rng.f64() * (hi - lo);
        let friction = heft.friction * tumble::GUST_EASE * self.dpi;
        self.body.vx = dir * (2.0 * friction * reach * self.dpi).sqrt();
        self.spin = heft.rolling_spin(self.body.vx / self.dpi);
        self.phase = Phase::Ground { friction, settle: None };
    }

    fn start_fall(&mut self) {
        self.standing = None;
        self.body.vy = 0.0;
        self.thrown = false;
        self.phase = Phase::Air;
    }

    fn tick_ground(&mut self, dt: f64, world: &World, friction: f64, mut settle: Option<f64>) {
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
        let mut x = ax + rel_x + v * dt;
        if x < lo || x > hi {
            x = x.clamp(lo, hi.max(lo));
            v = 0.0;
        }
        self.body.x = x;
        self.body.vx = v;
        let Some(piece) = world.piece_at(owner, x).copied() else {
            // 디딤판 끝을 지나 돌면서 떨어진다 (바람 때문이어도 — 화면 끝은 위에서 막았다)
            self.start_fall();
            return;
        };
        self.body.y = piece.y;
        self.standing = Some((owner, x - ax));
        let lying = match settle {
            None => {
                self.roll(dt);
                if v == 0.0 && self.spin.abs() < tumble::SETTLE_SPIN {
                    settle = Some(shape_of(self.kind).rest_angle(self.angle));
                }
                false
            }
            Some(target) => self.flop_toward(dt, target),
        };
        if v == 0.0 && lying {
            self.angle = (self.angle + 180.0).rem_euclid(360.0) - 180.0;
            self.spin = 0.0;
            self.phase = Phase::Rest;
        } else {
            self.phase = Phase::Ground { friction, settle };
        }
    }

    /// 굴러가는 한 틱: 땅이 회전을 구르는 빠르기로 맞추고, 모서리를 넘을 땐 느려졌다 넘어가며 빨라진다
    fn roll(&mut self, dt: f64) {
        let heft = heft_of(self.kind);
        let rolling = heft.rolling_spin(self.body.vx / self.dpi);
        self.spin += (rolling - self.spin) * (1.0 - (-heft.grip * dt).exp());
        self.spin -= tumble::CLUNK * shape_of(self.kind).slope_at(self.angle) * dt;
        self.angle += self.spin * dt;
    }

    /// 눕는 한 틱: 면 쪽으로 넘어가 바닥에 부딪히고, 살짝 되튀다 멈춘다 (바닥을 뚫고 지나가지 않는다). 다 누웠으면 true
    fn flop_toward(&mut self, dt: f64, target: f64) -> bool {
        let off = self.angle - target;
        if off.abs() < 0.2 && self.spin.abs() < tumble::TINY_SPIN {
            self.angle = target;
            self.spin = 0.0;
            return true;
        }
        let side = if off != 0.0 { off.signum() } else { self.spin.signum() };
        self.spin -= side * tumble::FLOP * dt;
        let next = self.angle + self.spin * dt;
        if self.spin * side < 0.0 && (next - target) * side <= 0.0 {
            self.angle = target;
            self.spin = -self.spin * tumble::REBOUND;
            if self.spin.abs() < tumble::STOP_SPIN {
                self.spin = 0.0;
                return true;
            }
            return false;
        }
        self.angle = next;
        false
    }

    fn tick_air(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if let Some(exit) = self.fling_out(dt, world, edges) {
            return Some(exit);
        }
        let prev_y = self.body.y;
        let g = THROW_GRAVITY_LOGICAL * self.dpi;
        let (size, hw) = (self.size(), self.half_w());
        let vx_before = self.body.vx;
        step_air(&mut self.body, dt, &world.bounds, hw, size, g);
        if self.body.vx * vx_before < 0.0 {
            // 벽에 부딪혔다: 반대로 돈다
            self.spin *= -tumble::WALL_SPIN_KEEP;
        }
        self.angle += self.spin * dt;
        // 아래 친구에게 던졌으면 창 윗면을 지나 바닥까지 간다
        let to_ground = self.thrown && self.thrown_down && edges.down;
        let landing = world.fall_landing(self.body.x, prev_y, self.body.y, self.body.vy, to_ground);
        if let Some(s) = landing {
            self.body.y = s.y;
            self.thrown = false;
            if self.body.vy > BOUNCE_MIN_VY * self.dpi {
                self.bounce();
                return None;
            }
            let landed_at = self.body.vy;
            self.body.vy = 0.0;
            let ax = world.anchor(s.owner).map_or(0.0, |(ax, _)| ax);
            self.standing = Some((s.owner, self.body.x - ax));
            let friction = heft_of(self.kind).friction * self.dpi;
            self.phase = if self.spin == 0.0 {
                // 살며시 내려앉았다: 그 자세로 살짝 까딱하고 만다. 공중에서 붙잡아 기운 채 놓았으면 가까운 면으로 눕는다 (처음 모습 0도는 그대로 둔다)
                let dir = if self.rng.bool() { 1.0 } else { -1.0 };
                self.spin = dir * wobble_spin(landed_at / self.dpi);
                let settle = if self.angle == 0.0 { 0.0 } else { shape_of(self.kind).rest_angle(self.angle) };
                Phase::Ground { friction, settle: Some(settle) }
            } else {
                Phase::Ground { friction, settle: None }
            };
        } else if self.body.y > world.bounds.bottom + size * 4.0 {
            // 화면 밖으로 사라짐 → 위에서 다시 떨어진다
            let b = world.bounds;
            self.body = Body { x: self.body.x.clamp(b.left + hw, (b.right - hw).max(b.left + hw)), y: b.top + size, vx: 0.0, vy: 0.0 };
        }
        None
    }

    /// 바닥을 치고 튀어 오른다: 도는 물건이면 회전이 구르는 쪽으로 쏠리고, 가벼운 것은 제멋대로 튄다
    fn bounce(&mut self) {
        let heft = heft_of(self.kind);
        self.body.vy *= -heft.bounce;
        self.body.vx *= heft.bounce_vx;
        if self.spin != 0.0 {
            let rolling = heft.rolling_spin(self.body.vx / self.dpi);
            let kick = (self.rng.f64() * 2.0 - 1.0) * heft.jitter;
            self.spin = (self.spin * heft.bounce_spin + rolling * (1.0 - heft.bounce_spin) + kick).clamp(-heft.max_spin, heft.max_spin);
            if self.spin == 0.0 {
                // 회전이 딱 0이면 살며시 내려앉은 것으로 치므로, 구르던 물건은 0이 되지 않게 한다
                self.spin = f64::MIN_POSITIVE;
            }
        }
    }

    /// 던진 비행이 친구가 연결된 화면 끝을 넘으면 그대로 넘어간다 (동물과 같은 규칙, brain_fling.rs)
    fn fling_out(&mut self, dt: f64, world: &World, edges: ExitEdges) -> Option<Exit> {
        if !self.thrown {
            return None;
        }
        let b = world.bounds;
        let (hw, size, dpi) = (self.half_w(), self.size(), self.dpi);
        let Body { x, y, vx, vy } = self.body;
        // next_y는 이번 틱의 중력까지 더한다: 이 틱에 바닥을 지나면 내려앉기 전에 넘어가야 한다
        let (next_x, next_y) = (x + vx * dt, y + (vy + THROW_GRAVITY_LOGICAL * dpi * dt) * dt);
        // 위/아래 끝은 이 줄의 끝 (모니터마다 작업표시줄·크기가 달라도 위아래에 다른 모니터가 없으면 넘어간다)
        let col = visit::column_bounds(world, x);
        let exit = if next_x < b.left + hw && vx < 0.0 && edges.left {
            Some(Side::Left)
        } else if next_x > b.right - hw && vx > 0.0 && edges.right {
            Some(Side::Right)
        } else if vy < 0.0 && edges.up && col.is_some_and(|c| next_y < c.top + size) {
            Some(Side::Up)
        } else if edges.down && col.is_some_and(|c| next_y >= c.bottom) && visit::falls_through(self.thrown_down, vy, visit::FLING_DOWN_MIN_SPEED, dpi) {
            Some(Side::Down)
        } else {
            None
        };
        let side = exit?;
        let fling = visit::fling_across(world, side, &self.body, dpi)?;
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
        if self.postit == Postit::Never {
            return;
        }
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

#[cfg(test)]
impl Object {
    /// 테스트: 지금 자리에서 (vx, vy)로 던져져 날아간다
    pub(crate) fn force_throw(&mut self, vx: f64, vy: f64) {
        self.body.vx = vx;
        self.body.vy = vy;
        self.standing = None;
        self.phase = Phase::Air;
        self.thrown = true;
    }
}

/// 바람이 불어도 되는 때: 모두 숨겨 두지 않았고 사용자가 자리에 있다
pub fn gusts_ok(hidden: bool, away: bool) -> bool {
    !hidden && !away
}

fn next_gust(rng: &mut fastrand::Rng) -> f64 {
    GUST_EVERY_SECS.0 + rng.f64() * (GUST_EVERY_SECS.1 - GUST_EVERY_SECS.0)
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
    // 똥엔 포스트잇이 없다 (옛 버전·고친 앱이 보내도)
    let note = note.filter(|_| !kind.is_poop()).and_then(visit::sanitize_note);
    Some(AcceptedToss { kind, fling: Fling { height: fling.height, vx, vy }, note })
}

/// 물건이 어디서 왔나
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// 내가 만들었다 (최대 MAX_OWN_OBJECTS개)
    Own,
    /// 친구에게서 날아왔다 (최대 20개)
    Foreign,
    /// 똥: 어디서 왔든 최대 MAX_POOPS개
    Poop,
}

/// 화면 위 물건 수 지키기: 넘치면 그 쪽에서 가장 오래된 것을 내보낸다
#[derive(Debug, Default)]
pub struct Roster {
    own: VecDeque<u64>,
    foreign: VecDeque<u64>,
    poops: VecDeque<u64>,
}

impl Roster {
    /// id를 더한다. 넘쳐서 치워야 할 물건이 있으면 돌려준다
    pub fn add(&mut self, id: u64, origin: Origin) -> Option<u64> {
        let (list, max) = match origin {
            Origin::Own => (&mut self.own, MAX_OWN_OBJECTS),
            Origin::Foreign => (&mut self.foreign, MAX_FOREIGN_OBJECTS),
            Origin::Poop => (&mut self.poops, MAX_POOPS),
        };
        list.push_back(id);
        if list.len() > max {
            list.pop_front()
        } else {
            None
        }
    }

    pub fn remove(&mut self, id: u64) -> bool {
        for list in [&mut self.own, &mut self.foreign, &mut self.poops] {
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

    #[cfg(test)]
    pub fn poop_count(&self) -> usize {
        self.poops.len()
    }

    pub fn clear(&mut self) {
        self.own.clear();
        self.foreign.clear();
        self.poops.clear();
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

const TOSS_BURST: f64 = 50.0;
const TOSS_EVERY_SECS: f64 = 1.2;

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

/// 메뉴 한 번에 던지는 물건 수
const VOLLEY_SIZE: u32 = 10;
/// 연달아 눌러 쌓을 수 있는 횟수
const MAX_VOLLEYS: u32 = 5;

/// 친구 한 명에게 "물건 10개 던지기"로 기다리는 수: 다시 눌러도 10개까지만 기다린다
#[derive(Debug, Default, Clone, Copy)]
pub struct Volley {
    left: u32,
}

impl Volley {
    /// 누를 때마다 10개씩, 다섯 번까지 쌓인다
    pub fn request(&mut self) {
        self.left = (self.left + VOLLEY_SIZE).min(VOLLEY_SIZE * MAX_VOLLEYS);
    }

    /// 예산이 되는 만큼 예산을 쓰고, 지금 던질 수를 돌려준다
    pub fn release(&mut self, budget: &mut TossBudget, now: f64) -> u32 {
        let mut sent = 0;
        while self.left > 0 && budget.can_spend(now) {
            budget.spend(now);
            self.left -= 1;
            sent += 1;
        }
        sent
    }

    pub fn is_done(&self) -> bool {
        self.left == 0
    }
}

/// 던질 물건 하나: 쓰레기 중 아무거나 (똥은 빼고), 들어오는 높이와 속도도 제각각
pub fn volley_piece(rng: &mut fastrand::Rng) -> (ObjectKind, Fling) {
    let kind = ObjectKind::TRASH[rng.usize(..ObjectKind::TRASH.len())];
    let fling = Fling { height: 0.1 + rng.f64() * 0.6, vx: 300.0 + rng.f64() * 500.0, vy: -(100.0 + rng.f64() * 400.0) };
    (kind, fling)
}

#[cfg(test)]
#[path = "object_tests.rs"]
mod tests;
