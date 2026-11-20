//! 🌀 여름 태풍 (v0.6.8) — 돌풍 박자와 회오리의 순수한 계산 (창·웹뷰는 앱이: src-tauri/src/weather.rs, 그림은 ui/weather.js).
//!
//! - Gusts: 늘 부는 센 바람 + 몇 초마다 더 센 돌풍. 돌풍이 막 일면 앱이 동물(Pet::feel_gust)과 바닥의 물건(Object::blow)에 알림
//! - Tornado: 한 모니터 바닥을 이리저리 돌아다니는 회오리. 가까운 동물·물건(Loose)을 빨아들여 깔때기를 따라 빙글빙글
//!   올려 돌리다(Whirl::Hold) 휙 내던짐(Whirl::Fling). 친구 쪽 끝(Loose.cross)이 있으면 자주 그 끝까지 닿게 던짐 —
//!   그 뒤는 손으로 던진 것과 같은 길 (물건 toss_away, 내 동물 own_exited, 손님 guest_left). 동물 넘기기만 PET_CROSS_GAP에 한 번
//! - Storm: 태풍 동안 회오리를 잠깐 쉬었다 하나씩 (사라질 때 든 것은 이 화면에 내던져 잃어버리지 않음)
//! - 🌀 허리케인급 (HURRICANE_CHANCE): 가끔 오는 훨씬 크고 높은 회오리 — 더 멀리서 더 많이 빨아들이고, 거대한 💩·🪨(Loose.heavy)까지
//!   들어 올려 이 화면에 내던짐. 그동안 돌풍·바람도 셈. 크기(size)는 장면에 실려 웹뷰 깔때기도 그만큼 큼
//!
//! 좌표·속도는 물리 px (dpi를 곱함), 상수는 논리 px.

use crate::geom::Rect;
use crate::physics::{MAX_THROW_SPEED_LOGICAL, THROW_GRAVITY_LOGICAL};
use crate::visit::Side;

/// 돌풍 사이 간격 (초)
pub const GUST_EVERY: (f64, f64) = (3.0, 7.0);
/// 돌풍 하나의 길이 (초)
pub const GUST_SECS: (f64, f64) = (1.0, 2.5);
/// 늘 부는 바람 세기 (웹뷰의 wind, 부호 = 방향)
const STEADY_WIND: f64 = 0.7;
/// 돌풍이 더하는 세기 (세기 1일 때)
const GUST_WIND: f64 = 0.9;
/// 바람 세기 상한
pub const MAX_WIND: f64 = STEADY_WIND + GUST_WIND;
/// 돌풍이 거꾸로 불 확률 (태풍 바람 방향은 그대로)
const GUST_FLIP: f64 = 0.2;

/// 회오리가 빨아들이는 가로 거리 (논리 px, 깔때기 가운데에서)
pub const PULL_RADIUS: f64 = 300.0;
/// 바닥에서 이 높이 안의 것만 빨아들임 (논리 px — 높은 창 위는 그대로)
pub const PULL_HEIGHT: f64 = 260.0;
/// 한 번에 도는 수
pub const MAX_RIDERS: usize = 8;
/// 빙글빙글 도는 시간 (초)
pub const SPIN_SECS: (f64, f64) = (1.4, 2.6);
/// 올라가는 높이 (논리 px)
pub const LIFT: f64 = 240.0;
/// 도는 빠르기 (라디안/초)
const OMEGA: f64 = 7.5;
/// 깔때기 반지름: 바닥 → 꼭대기 (논리 px — ui/weather.js 그림과 같음)
const FUNNEL_R: (f64, f64) = (26.0, 90.0);
/// 내던진 것은 이만큼 (초) 다시 빨아들이지 않음
pub const RECATCH_SECS: f64 = 3.0;
/// 친구 쪽 끝이 있으면 그쪽으로 내던질 확률
pub const CROSS_CHANCE: f64 = 0.75;
/// 동물을 친구 화면으로 넘기는 최소 간격 (초, 회오리마다)
pub const PET_CROSS_GAP: f64 = 8.0;
/// 이 화면에 내던지는 빠르기 (논리 px/s): 옆, 위
const LOCAL_VX: (f64, f64) = (350.0, 800.0);
const LOCAL_VY: (f64, f64) = (350.0, 700.0);
/// 돌아다니는 빠르기 (논리 px/s)
const ROAM_SPEED: (f64, f64) = (80.0, 240.0);
/// 빠르기·방향을 다시 고르는 간격 (초)
const ROAM_TURN: (f64, f64) = (1.2, 3.5);
/// 방향을 바꿀 확률 (다시 고를 때)
const ROAM_FLIP: f64 = 0.4;
/// 회오리 하나가 사는 시간 (초)
pub const TORNADO_SECS: (f64, f64) = (40.0, 90.0);
/// 회오리 사이 쉬는 시간 (초) — 처음은 FIRST_TORNADO
pub const TORNADO_REST: (f64, f64) = (4.0, 12.0);
const FIRST_TORNADO: (f64, f64) = (2.0, 5.0);
/// 🌀 회오리가 허리케인급일 확률 (회오리마다)
pub const HURRICANE_CHANCE: f64 = 0.22;
/// 허리케인급 깔때기 크기 배수 (반지름·높이·올라가는 높이 — ui/weather.js도 장면의 크기로 그만큼 크게 그림)
pub const HURRICANE_SIZE: f64 = 2.0;
/// 허리케인급이 빨아들이는 거리·높이 배수
const HURRICANE_PULL: f64 = 1.8;
/// 허리케인급이 한 번에 도는 수
pub const HURRICANE_RIDERS: usize = 14;
/// 허리케인급 동안 돌풍 세기 배수 (세기가 1을 넘음 — 동물이 더 잘 날아감)
pub const HURRICANE_GUST: f64 = 1.4;
/// 허리케인급 동안 바람 세기 배수 (웹뷰의 wind)
const HURRICANE_WIND: f64 = 1.3;
/// 허리케인급이 이 화면에 내던지는 빠르기 배수
const HURRICANE_FLING: f64 = 1.3;
/// 친구 쪽으로 던질 때 해 보는 가로 빠르기 (논리 px/s)
const CROSS_VX: [f64; 4] = [800.0, 1000.0, 1200.0, 1400.0];
/// 위로 던지는 최소 빠르기 (논리 px/s)
const CROSS_MIN_UP: f64 = 200.0;

/// [a, b) 사이 난수임
fn between(rng: &mut fastrand::Rng, (a, b): (f64, f64)) -> f64 {
    a + rng.f64() * (b - a)
}

/// 돌풍 하나 (막 일었을 때 한 번)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gust {
    /// 부는 쪽 (±1)
    pub dir: f64,
    /// 세기 0.3…1 (허리케인급 동안 HURRICANE_GUST배까지)
    pub strength: f64,
}

/// 늘 부는 바람과 몇 초마다의 돌풍
#[derive(Debug, Clone)]
pub struct Gusts {
    rng: fastrand::Rng,
    /// 태풍 바람 방향 (±1)
    dir: f64,
    wait: f64,
    /// 지금 돌풍: (남은 시간, 전체 길이, 세기, 방향)
    gust: Option<(f64, f64, f64, f64)>,
}

impl Gusts {
    /// 바람을 시작함 (방향은 seed로)
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let dir = if rng.bool() { 1.0 } else { -1.0 };
        let wait = between(&mut rng, GUST_EVERY);
        Self { rng, dir, wait, gust: None }
    }

    /// dt초 보냄: 돌풍이 막 일면 Some
    pub fn tick(&mut self, dt: f64) -> Option<Gust> {
        if let Some((left, len, s, dir)) = self.gust {
            let left = left - dt;
            self.gust = (left > 0.0).then_some((left, len, s, dir));
            return None;
        }
        self.wait -= dt;
        if self.wait > 0.0 {
            return None;
        }
        self.wait = between(&mut self.rng, GUST_EVERY);
        let dir = if self.rng.f64() < GUST_FLIP { -self.dir } else { self.dir };
        let strength = between(&mut self.rng, (0.3, 1.0));
        let len = between(&mut self.rng, GUST_SECS);
        self.gust = Some((len, len, strength, dir));
        Some(Gust { dir, strength })
    }

    /// 지금 바람 (부호 = 방향, 크기 MAX_WIND까지): 돌풍은 부드럽게 불었다 잦아듦 (거꾸로 부는 돌풍은 바람을 잠깐 꺾음)
    pub fn wind(&self) -> f64 {
        let boost = self.gust.map_or(0.0, |(left, len, s, dir)| dir * s * (std::f64::consts::PI * left / len).sin());
        self.dir * STEADY_WIND + GUST_WIND * boost
    }
}

/// 돌풍에 걷는 동물이 하는 것
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stagger {
    /// 버팀 ("hold on!" + 덜덜)
    Lean,
    /// 데굴 넘어짐
    Tumble,
    /// 발이 떨어져 날아감
    Blown,
}

/// 세기와 난수(roll 0…1)로 반응을 고름 — 셀수록 날아감
pub fn gust_reaction(strength: f64, roll: f64) -> Stagger {
    if roll < 0.35 * strength {
        Stagger::Blown
    } else if roll < 0.75 * strength {
        Stagger::Tumble
    } else {
        Stagger::Lean
    }
}

/// 돌풍에 날리는 빠르기 (물리 px/s): 바람 쪽 옆으로 세게, 위로 조금 (roll 0…1)
pub fn blow_velocity(dir: f64, strength: f64, dpi: f64, roll: f64) -> (f64, f64) {
    let vx = dir.signum() * (320.0 + 480.0 * strength) * (0.85 + 0.3 * roll);
    let vy = -(120.0 + 260.0 * strength * roll);
    (vx * dpi, vy * dpi)
}

/// from(발 자리)에서 edge_x까지 바닥(floor_y)에 닿기 전에 가는 포물선 (던진 중력, 물리 px/s). 최대 빠르기로도 안 되면 None
pub fn cross_throw(from: (f64, f64), edge_x: f64, floor_y: f64, dpi: f64) -> Option<(f64, f64)> {
    let g = THROW_GRAVITY_LOGICAL * dpi;
    let d = (edge_x - from.0).abs();
    let h = (floor_y - from.1).max(0.0);
    let dir = (edge_x - from.0).signum();
    let max = MAX_THROW_SPEED_LOGICAL * dpi * 0.97;
    CROSS_VX.iter().find_map(|&vx| {
        let vx = vx * dpi;
        // 끝까지 가는 시간에 여유를 조금 — 그동안 바닥보다 위에 있도록 위로 던짐 (g t² / 2 − u t ≤ h)
        let t = d * 1.1 / vx + 0.05;
        let up = ((g * t * t - 2.0 * h) / (2.0 * t)).max(CROSS_MIN_UP * dpi);
        (vx.hypot(up) <= max).then_some((dir * vx, -up))
    })
}

/// 회오리가 다루는 것: 동물 (social_key) 또는 물건 (id)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Thing {
    Pet(u64),
    Object(u64),
}

/// 이번 틱에 빨아들일 수 있는 것 (도는 중인 것도 계속 넣어야 함 — 빠지면 놓아줌)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loose {
    pub thing: Thing,
    /// 바닥 가운데 x
    pub x: f64,
    /// 바닥(발) y
    pub y: f64,
    /// 내던져 넘길 수 있는 친구 쪽 끝 (왼쪽·오른쪽만)
    pub cross: Option<Side>,
    /// 거대한 것 (💩·🪨): 허리케인급만 빨아들이고, 친구에게는 던지지 않음
    pub heavy: bool,
}

/// 회오리가 앱에 시키는 것
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Whirl {
    /// 이 자리(바닥 가운데, 바닥 y)에 붙들어 돌림. angle = 물건 그림 각도 (도)
    Hold { thing: Thing, x: f64, y: f64, angle: f64 },
    /// 이 빠르기(물리 px/s)로 내던짐. cross = 친구 쪽 끝으로 (던진 비행 — 끝을 넘어감)
    Fling { thing: Thing, vx: f64, vy: f64, cross: bool },
}

impl Whirl {
    /// 누구 일인지
    pub fn thing(&self) -> Thing {
        match *self {
            Whirl::Hold { thing, .. } | Whirl::Fling { thing, .. } => thing,
        }
    }
}

/// 도는 것 하나
#[derive(Debug, Clone, Copy)]
struct Rider {
    thing: Thing,
    cross: Option<Side>,
    heavy: bool,
    t: f64,
    secs: f64,
    angle: f64,
    /// 빨려 들어온 거리 (처음 반지름)
    start_r: f64,
    start_h: f64,
}

/// 깔때기가 모니터 wa 밖에 있지만 그 끝에 걸쳐 보이는 자리임 (scale = 그 모니터 배율, size = 깔때기 크기 Tornado::size).
/// 옆 모니터로 넘어가는 동안 양쪽에 반쪽씩 그림
pub fn straddles(x: f64, wa: &Rect, scale: f64, size: f64) -> bool {
    let reach = (FUNNEL_R.1 + 30.0) * size * scale;
    (x < wa.left && x > wa.left - reach) || (x > wa.right && x < wa.right + reach)
}

/// 한 모니터 바닥을 돌아다니는 회오리
#[derive(Debug, Clone)]
pub struct Tornado {
    area: Rect,
    /// 지금 모니터들 (옆 모니터로 넘어갈 때 씀). 비면 제 모니터만
    areas: Vec<Rect>,
    bounds: Rect,
    dpi: f64,
    x: f64,
    vx: f64,
    turn_in: f64,
    parked: bool,
    clock: f64,
    riders: Vec<Rider>,
    recent: Vec<(Thing, f64)>,
    next_pet_cross: f64,
    last_from: Option<(f64, f64)>,
    /// 🌀 허리케인급
    hurricane: bool,
    rng: fastrand::Rng,
}

impl Tornado {
    /// area 모니터(작업 영역) 바닥에 생김. bounds = 세계 전체 (친구 쪽 끝)
    pub fn new(area: Rect, bounds: Rect, dpi: f64, seed: u64) -> Self {
        Self::build(area, bounds, dpi, seed, false)
    }

    /// 🌀 허리케인급 회오리 (크고 높고, 거대한 것도 들어 올림)
    pub fn hurricane(area: Rect, bounds: Rect, dpi: f64, seed: u64) -> Self {
        Self::build(area, bounds, dpi, seed, true)
    }

    /// 회오리를 만듦 (hurricane = 허리케인급)
    fn build(area: Rect, bounds: Rect, dpi: f64, seed: u64, hurricane: bool) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed);
        let dpi = dpi.max(0.01);
        let size = if hurricane { HURRICANE_SIZE } else { 1.0 };
        let margin = (FUNNEL_R.1 * size * dpi).min(area.width() / 2.0);
        let x = area.left + margin + rng.f64() * (area.width() - 2.0 * margin).max(0.0);
        let dir = if rng.bool() { 1.0 } else { -1.0 };
        let vx = dir * between(&mut rng, ROAM_SPEED) * dpi;
        let turn_in = between(&mut rng, ROAM_TURN);
        Self { area, areas: Vec::new(), bounds, dpi, x, vx, turn_in, parked: false, clock: 0.0, riders: Vec::new(), recent: Vec::new(), next_pet_cross: 0.0, last_from: None, hurricane, rng }
    }

    /// 🌀 허리케인급임
    pub fn is_hurricane(&self) -> bool {
        self.hurricane
    }

    /// 깔때기 크기 배수 (보통 1, 허리케인급 HURRICANE_SIZE — 장면에 실어 웹뷰도 그만큼 크게 그림)
    pub fn size(&self) -> f64 {
        if self.hurricane { HURRICANE_SIZE } else { 1.0 }
    }

    /// 빨아들이는 거리·높이 배수
    fn pull(&self) -> f64 {
        if self.hurricane { HURRICANE_PULL } else { 1.0 }
    }

    /// 지금 모니터들을 알려 줌: 옆에 붙은 모니터가 있으면 그쪽 끝에서 돌아서지 않고 넘어감
    pub fn set_areas(&mut self, areas: &[Rect]) {
        self.areas.clear();
        self.areas.extend_from_slice(areas);
    }

    /// side 쪽 끝에 맞붙고 위아래가 겹치는 모니터 (없으면 None)
    fn neighbor(&self, right: bool) -> Option<Rect> {
        let a = self.area;
        self.areas.iter().copied().find(|b| {
            let touches = if right { (b.left - a.right).abs() <= 2.0 } else { (b.right - a.left).abs() <= 2.0 };
            touches && b.top < a.bottom && a.top < b.bottom
        })
    }

    /// 깔때기 가운데 x
    pub fn x(&self) -> f64 {
        self.x
    }

    /// 바닥 y (모니터 작업 영역 아래)
    pub fn ground(&self) -> f64 {
        self.area.bottom
    }

    /// 지금 가로 빠르기 (물리 px/s)
    pub fn vx(&self) -> f64 {
        self.vx
    }

    /// 사는 모니터
    pub fn area(&self) -> Rect {
        self.area
    }

    /// 도는 수
    pub fn riders(&self) -> usize {
        self.riders.len()
    }

    /// 이것을 돌리는 중
    pub fn holds(&self, thing: Thing) -> bool {
        self.riders.iter().any(|r| r.thing == thing)
    }

    /// 테스트: x에 멈춰 세움
    #[cfg(test)]
    pub(crate) fn park(&mut self, x: f64) {
        self.x = x;
        self.vx = 0.0;
        self.parked = true;
    }

    /// 테스트: 마지막으로 내던진 자리
    #[cfg(test)]
    pub(crate) fn last_fling_from(&self) -> Option<(f64, f64)> {
        self.last_from
    }

    /// dt초: 돌아다니고, 빠진 것은 놓아주고, 가까운 것을 빨아들이고, 다 돈 것은 내던짐 (out에 더함)
    pub fn step(&mut self, dt: f64, loose: &[Loose], out: &mut Vec<Whirl>) {
        self.clock += dt;
        self.roam(dt);
        let clock = self.clock;
        self.recent.retain(|&(_, until)| until > clock);
        // 사용자가 잡았거나 치웠거나 하우스에 들어갔음 — 그냥 놓아줌 (그쪽이 알아서)
        self.riders.retain(|r| loose.iter().any(|l| l.thing == r.thing));
        self.catch(loose);
        let mut i = 0;
        while i < self.riders.len() {
            let r = &mut self.riders[i];
            r.t += dt;
            r.angle += OMEGA * dt;
            let r = *r;
            let (x, y) = self.ride_spot(&r);
            if r.t >= r.secs {
                self.riders.swap_remove(i);
                out.push(self.fling(r, (x, y)));
                continue;
            }
            out.push(Whirl::Hold { thing: r.thing, x, y, angle: (r.angle * 57.3 * 0.6) % 360.0 });
            i += 1;
        }
    }

    /// 회오리가 사라짐: 도는 것을 모두 이 화면에 내던짐
    pub fn release_all(&mut self, out: &mut Vec<Whirl>) {
        for r in std::mem::take(&mut self.riders) {
            let at = self.ride_spot(&r);
            out.push(self.local_fling(r.thing, at));
        }
    }

    /// 이리저리: 가끔 빠르기·방향을 다시 고르고, 모니터 끝에서 돌아섬. 옆에 모니터가 붙어 있으면 그 끝은 넘어가 그 모니터 회오리가 됨
    fn roam(&mut self, dt: f64) {
        if self.parked {
            return;
        }
        self.turn_in -= dt;
        if self.turn_in <= 0.0 {
            self.turn_in = between(&mut self.rng, ROAM_TURN);
            let dir = if self.rng.f64() < ROAM_FLIP { -self.vx.signum() } else { self.vx.signum() };
            self.vx = dir * between(&mut self.rng, ROAM_SPEED) * self.dpi;
        }
        let margin = (FUNNEL_R.1 * self.size() * self.dpi).min(self.area.width() / 2.0);
        let (left_n, right_n) = (self.neighbor(false), self.neighbor(true));
        let lo = if left_n.is_some() { self.area.left } else { self.area.left + margin };
        let hi = if right_n.is_some() { self.area.right } else { self.area.right - margin };
        self.x += self.vx * dt;
        match (left_n, right_n) {
            (Some(n), _) if self.x < self.area.left => {
                self.area = n;
                return;
            }
            (_, Some(n)) if self.x > self.area.right => {
                self.area = n;
                return;
            }
            _ => {}
        }
        if self.x < lo {
            self.x = lo;
            self.vx = self.vx.abs();
        } else if self.x > hi {
            self.x = hi.max(lo);
            self.vx = -self.vx.abs();
        }
    }

    /// 가까운 것을 빨아들임 (MAX_RIDERS까지 — 허리케인급은 더 멀리서 HURRICANE_RIDERS까지, 거대한 것도. 방금 내던진 것은 빼고)
    fn catch(&mut self, loose: &[Loose]) {
        let (reach, height) = (PULL_RADIUS * self.pull() * self.dpi, PULL_HEIGHT * self.pull() * self.dpi);
        let most = if self.hurricane { HURRICANE_RIDERS } else { MAX_RIDERS };
        for l in loose {
            if self.riders.len() >= most {
                return;
            }
            if l.heavy && !self.hurricane {
                continue;
            }
            let dx = l.x - self.x;
            let h = self.area.bottom - l.y;
            let fresh = !self.holds(l.thing) && !self.recent.iter().any(|&(t, _)| t == l.thing);
            if !fresh || dx.abs() > reach || !(-4.0 * self.dpi..=height).contains(&h) {
                continue;
            }
            let secs = between(&mut self.rng, SPIN_SECS);
            let angle = if dx >= 0.0 { 0.0 } else { std::f64::consts::PI };
            self.riders.push(Rider { thing: l.thing, cross: l.cross, heavy: l.heavy, t: 0.0, secs, angle, start_r: dx.abs(), start_h: h.max(0.0) });
        }
    }

    /// 도는 자리: 처음 자리에서 깔때기 둘레로 점점 끌려가며 올라감
    fn ride_spot(&self, r: &Rider) -> (f64, f64) {
        let k = (r.t / r.secs).clamp(0.0, 1.0);
        let pull = (k * 3.0).min(1.0);
        let ease = 1.0 - (1.0 - k) * (1.0 - k);
        let (size, lift) = (self.size(), LIFT * self.size() * self.dpi);
        let h = r.start_h + (lift - r.start_h).max(0.0) * ease;
        let funnel = (FUNNEL_R.0 + (FUNNEL_R.1 - FUNNEL_R.0) * (h / lift).min(1.0)) * size * self.dpi;
        let radius = r.start_r + (funnel - r.start_r) * pull;
        (self.x + radius * r.angle.cos(), self.area.bottom - h)
    }

    /// 다 돈 것을 내던짐: 친구 쪽 끝이 있으면 자주 그 끝까지 (동물은 PET_CROSS_GAP에 한 번, 거대한 것은 안 됨), 아니면 이 화면에
    fn fling(&mut self, r: Rider, at: (f64, f64)) -> Whirl {
        self.recent.push((r.thing, self.clock + RECATCH_SECS));
        self.last_from = Some(at);
        let is_pet = matches!(r.thing, Thing::Pet(_));
        let may = r.cross.filter(|s| matches!(s, Side::Left | Side::Right)).filter(|_| !r.heavy && (!is_pet || self.clock >= self.next_pet_cross));
        if let Some(side) = may.filter(|_| self.rng.f64() < CROSS_CHANCE) {
            let edge = if side == Side::Left { self.bounds.left } else { self.bounds.right };
            if let Some((vx, vy)) = cross_throw(at, edge, self.area.bottom, self.dpi) {
                if is_pet {
                    self.next_pet_cross = self.clock + PET_CROSS_GAP;
                }
                return Whirl::Fling { thing: r.thing, vx, vy, cross: true };
            }
        }
        self.local_fling(r.thing, at)
    }

    /// 이 화면 안으로 내던짐 (대개 넓은 쪽으로, 허리케인급은 더 세게)
    fn local_fling(&mut self, thing: Thing, at: (f64, f64)) -> Whirl {
        let room = (self.area.right - at.0) - (at.0 - self.area.left);
        let dir = if self.rng.f64() < 0.7 { room.signum() } else { -room.signum() };
        let dir = if dir == 0.0 { 1.0 } else { dir };
        let k = if self.hurricane { HURRICANE_FLING } else { 1.0 } * self.dpi;
        let vx = dir * between(&mut self.rng, LOCAL_VX) * k;
        let vy = -between(&mut self.rng, LOCAL_VY) * k;
        Whirl::Fling { thing, vx, vy, cross: false }
    }
}

/// 태풍 동안: 바람과, 쉬었다 하나씩 오는 회오리
#[derive(Debug, Clone)]
pub struct Storm {
    gusts: Gusts,
    tornado: Option<Tornado>,
    /// 다음 회오리까지 (없을 때) 또는 지금 회오리가 사라질 때까지 (있을 때)
    timer: f64,
    rng: fastrand::Rng,
}

impl Storm {
    /// 태풍이 시작됨
    pub fn new(seed: u64) -> Self {
        let mut rng = fastrand::Rng::with_seed(seed ^ 0x7a1f);
        let timer = between(&mut rng, FIRST_TORNADO);
        Self { gusts: Gusts::new(seed), tornado: None, timer, rng }
    }

    /// 지금 회오리
    pub fn tornado(&self) -> Option<&Tornado> {
        self.tornado.as_ref()
    }

    /// 지금 허리케인급 회오리가 있음 (바람·돌풍이 셈)
    fn raging(&self) -> bool {
        self.tornado.as_ref().is_some_and(Tornado::is_hurricane)
    }

    /// 지금 바람 (웹뷰의 wind — 허리케인급 동안 더 셈)
    pub fn wind(&self) -> f64 {
        self.gusts.wind() * if self.raging() { HURRICANE_WIND } else { 1.0 }
    }

    /// dt초: 바람을 돌리고 (돌풍이 막 일면 Some — 허리케인급 동안 HURRICANE_GUST배), 회오리를 만들거나 (HURRICANE_CHANCE로 허리케인급)
    /// 돌리거나 보냄. areas = 모니터 작업 영역, bounds = 세계 전체
    pub fn tick(&mut self, dt: f64, areas: &[Rect], bounds: Rect, dpi: f64, loose: &[Loose], out: &mut Vec<Whirl>) -> Option<Gust> {
        let raging = self.raging();
        let gust = self.gusts.tick(dt).map(|g| if raging { Gust { strength: g.strength * HURRICANE_GUST, ..g } } else { g });
        self.timer -= dt;
        // 모니터가 바뀌어 회오리 자리가 없어졌으면 보냄
        let gone = self.tornado.as_ref().is_some_and(|t| !areas.contains(&t.area()));
        match self.tornado.as_mut() {
            Some(t) if self.timer <= 0.0 || gone => {
                t.release_all(out);
                self.tornado = None;
                self.timer = between(&mut self.rng, TORNADO_REST);
            }
            Some(t) => {
                t.set_areas(areas);
                t.step(dt, loose, out)
            }
            None if self.timer <= 0.0 && !areas.is_empty() => {
                let area = areas[self.rng.usize(..areas.len())];
                let seed = self.rng.u64(..);
                let big = self.rng.f64() < HURRICANE_CHANCE;
                self.tornado = Some(if big { Tornado::hurricane(area, bounds, dpi, seed) } else { Tornado::new(area, bounds, dpi, seed) });
                self.timer = between(&mut self.rng, TORNADO_SECS);
            }
            None => {}
        }
        gust
    }
}

#[cfg(test)]
#[path = "typhoon_tests.rs"]
mod tests;
