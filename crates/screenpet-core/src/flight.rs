//! 날아다니는 동물(꿀벌)의 비행 (spec 2B §10) — 수치와 순수 계산. brain_fly.rs(Pet)가 쓴다.
//!
//! 나는 동물은 디딤판 위를 걷지 않는다. "이동"은 목표 지점까지 경유점 몇 개를 지나는 부드러운 곡선(Catmull-Rom)으로
//! 나는 것이고 — 가끔 원을 한두 바퀴 그리거나, 좌우로 살랑살랑 오가거나, 느긋한 8자를 그린다 —
//! 쉬기는 제자리에서 살랑살랑 떠 있거나, 가끔 창 윗면·바닥에 내려앉아(perch) 쉬거나 잔다.
//! 좌표는 다른 동물과 같다: x = 몸 가운데, y = 발바닥 (화면 아래로 +).

use std::f64::consts::{PI, TAU};

use crate::behavior::{self, Move};
use crate::geom::Rect;
use crate::surfaces::{Owner, Surface};
use crate::visit::{self, Side};
use crate::world::World;

/// 빠르게 날기(Run) 확률 — 나머지는 보통 날기, 가끔 공중제비(Roll)
pub const RUN_CHANCE: f64 = 0.3;
pub const ROLL_CHANCE: f64 = 0.1;
/// 떠서 쉬기로 정할 때 대신 어딘가에 내려앉으러 갈 확률
pub const PERCH_CHANCE: f64 = 0.35;
/// 내려앉아 쉬다가 다음 할 일을 정할 때 잠들 확률 (떠 있을 땐 자지 않는다)
pub const PERCH_SLEEP_CHANCE: f64 = 0.35;
/// 공중제비 원 반지름 (몸 크기 배 — 지름 1.5 몸)
pub const LOOP_RADIUS: f64 = 0.75;
/// 공중제비를 시작하는 지점 (전체 거리의 비율)
pub const LOOP_AT: f64 = 0.4;
/// 떠 있을 때 위아래로 살랑이는 폭(몸 크기 배)과 빠르기(Hz)
pub const BOB_AMP: f64 = 0.04;
pub const BOB_HZ: f64 = 0.8;
/// 던져진 뒤 공기 저항 (1/s) — 이 속도(px/s)보다 느려지면 다시 제자리에 뜬다
pub const DRAG: f64 = 3.0;
pub const SETTLE_SPEED: f64 = 40.0;
/// 바깥에서 날 때 머리 위로 남기는 말풍선 자리 (몸 크기 배) — 말풍선 띠가 화면 밖으로 나가지 않게
pub const TOP_SPACE: f64 = 0.3;
/// 목표 높이가 화면 위쪽 2/3 안일 확률 (2/3이면 고르게 — 살짝만 위로)
pub const UPPER_SHARE: f64 = 0.7;
/// 화면 가장 바깥 끝에서 이 거리(몸 크기 배) 안으로 들어와 그쪽으로 날면 나가기를 정한다
pub const EDGE_ZONE: f64 = 1.0;
/// 목표까지 최소 거리 (몸 크기 배)
pub const MIN_FLIGHT: f64 = 1.0;
/// 집으로 날 때 활처럼 솟는 높이: 거리의 이 비율, 최대 몸 크기의 HOME_ARC_MAX배
pub const HOME_ARC: f64 = 0.2;
pub const HOME_ARC_MAX: f64 = 1.5;
/// 날기 한 번의 최대 시간 (안전장치 — 원·8자를 그려도 넉넉하게)
pub const MAX_FLIGHT_SECS: f64 = 60.0;
/// 날아갈 목표를 지금 모니터 안에서 고를 확률 (나머지는 아무 모니터)
pub const SAME_AREA_CHANCE: f64 = 0.75;
/// 날다가 좌우로 돌아볼 때: 가로 속도가 나는 속도의 이 비율을 넘어야 하고, 지난 뒤집기에서 이 시간(초)이 지나야 한다
pub const FLIP_SPEED: f64 = 0.3;
pub const FLIP_HOLD: f64 = 0.4;

/// 걷기 속도(cfg.walk_speed = pet.json walk_speed × √배율) 대비 나는 속도
pub fn speed_mult(kind: Move) -> f64 {
    match kind {
        Move::Walk => 1.0,
        Move::Run => 1.8,
        Move::Roll => 1.2,
    }
}

/// 다음 날기 방식: 30% 빠르게, 10% 공중제비, 나머지 보통
pub fn pick_move(rng: &mut fastrand::Rng) -> Move {
    let r = rng.f64();
    if r < RUN_CHANCE {
        Move::Run
    } else if r < RUN_CHANCE + ROLL_CHANCE {
        Move::Roll
    } else {
        Move::Walk
    }
}

/// area 안에서 몸(발 x, 발 y)이 있을 수 있는 범위: 좌우 반 폭, 위로 head(발에서 머리 위 여유까지)
pub fn fly_bounds(area: &Rect, half_w: f64, head: f64) -> Rect {
    let (mut l, mut r) = (area.left + half_w, area.right - half_w);
    if l > r {
        l = (area.left + area.right) / 2.0;
        r = l;
    }
    let t = (area.top + head).min(area.bottom);
    Rect::new(l, t, r, area.bottom)
}

pub fn clamp(b: &Rect, x: f64, y: f64) -> (f64, f64) {
    (x.clamp(b.left, b.right), y.clamp(b.top, b.bottom))
}

/// 날아갈 지점: x는 고르게, 높이는 위쪽 2/3에 70% (거의 고르게)
pub fn pick_target(rng: &mut fastrand::Rng, b: &Rect) -> (f64, f64) {
    let x = behavior::range(rng, (b.left, b.right));
    let h = b.height();
    let u = rng.f64();
    let y = if rng.f64() < UPPER_SHARE { b.top + u * h * 2.0 / 3.0 } else { b.top + (2.0 + u) / 3.0 * h };
    (x, y)
}

/// 날아가는 길의 모양
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wave {
    /// 진행 방향에 수직으로 출렁이는 사인 물결 (amp는 부호로 처음 출렁일 쪽을 정한다)
    Sine { amp: f64, len: f64 },
    /// 위로 솟는 활 모양 (집으로 갈 때)
    Arc { height: f64 },
}

/// from → to 직선을 s만큼 간 지점에 물결을 더한 점, 그리고 다 왔는지.
/// 물결은 양 끝에서 0이 되도록 sin(πu)로 줄인다 — 출발점과 목표에 정확히 닿는다
pub fn path_point(from: (f64, f64), to: (f64, f64), s: f64, wave: Wave) -> ((f64, f64), bool) {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let len = dx.hypot(dy);
    if len < 1e-6 || s >= len {
        return (to, true);
    }
    let u = (s / len).max(0.0);
    let envelope = (PI * u).sin();
    let (bx, by) = (from.0 + dx * u, from.1 + dy * u);
    match wave {
        Wave::Sine { amp, len: wl } => {
            let off = amp * (TAU * s / wl).sin() * envelope;
            ((bx - dy / len * off, by + dx / len * off), false)
        }
        Wave::Arc { height } => ((bx, by - height * envelope), false),
    }
}

/// 공중제비: start에서 진행 방향(facing)으로 원을 한 바퀴 (up이면 위로 돈다). theta = 0 → start
pub fn loop_point(start: (f64, f64), r: f64, theta: f64, facing: f64, up: bool) -> (f64, f64) {
    let x = start.0 + facing * r * theta.sin();
    let y = if up { start.1 - r + r * theta.cos() } else { start.1 + r - r * theta.cos() };
    (x, y)
}

/// 공기 저항으로 줄어든 속도
pub fn drift(vx: f64, vy: f64, dt: f64) -> (f64, f64) {
    let k = (-DRAG * dt).exp();
    (vx * k, vy * k)
}

/// 떠 있을 때 위아래 살랑임 (기준 높이에 더한다)
pub fn bob(clock: f64, size: f64) -> f64 {
    BOB_AMP * size * (TAU * BOB_HZ * clock).sin()
}

// ---- 꼬불꼬불 길 (v0.3.3) ----
//
// 예전 길은 곧은 선에 몸 0.15배짜리 사인 물결이라, 실제 화면에선 거의 일직선으로 보였다 (방향이 바뀌는 양 ≈ 0.2 rad/s).
// 이제 길은 출발점 → 경유점들 → 목표를 잇는 centripetal Catmull-Rom 곡선이다: 경유점을 정확히 지나고, 꺾이지 않고,
// 뾰족하게 되돌아가지 않는다. 목표(창 위 자리)는 움직일 수 있어 마지막 점은 늘 따로 받는다.

/// 굽이 하나가 차지하는 거리 (몸 크기 배) — 길이 길면 굽이가 많다
pub const BEND_EVERY: f64 = 1.6;
pub const MAX_BENDS: usize = 6;
/// 한 길의 경유점 최대 개수 (출발점 포함, 목표 제외)
pub const MAX_POINTS: usize = 32;
/// 그냥 날기의 경유점: 곧은 선에서 옆으로 벗어나는 폭 (몸 크기 배), 거리 대비 최대 비율
pub const BEND: (f64, f64) = (0.6, 1.6);
pub const BEND_CAP: f64 = 0.45;
/// 내려앉으러·먹으러 갈 때의 완만한 굽이 (몸 크기 배, 거리 대비 최대 비율)
pub const GENTLE_BEND: (f64, f64) = (0.3, 0.8);
pub const GENTLE_CAP: f64 = 0.3;
/// 날기 모양 확률: 원, 좌우로 살랑, 8자 (나머지는 그냥 굽이굽이)
pub const CIRCLE_CHANCE: f64 = 0.25;
pub const SWAY_CHANCE: f64 = 0.15;
pub const EIGHT_CHANCE: f64 = 0.1;
/// 원: 반지름 (몸 크기 배), 두 바퀴 돌 확률, 한 바퀴당 경유점 수, 나선으로 부풀거나 줄어드는 정도
pub const CIRCLE_RADIUS: (f64, f64) = (1.0, 1.5);
pub const CIRCLE_TWICE: f64 = 0.35;
pub const CIRCLE_SAMPLES: usize = 8;
pub const SPIRAL: f64 = 0.3;
/// 좌우로 살랑: 한쪽으로 벗어나는 폭 (몸 크기 배), 오가는 횟수, 목표 x를 지금 자리에서 이 안(몸 크기 배)으로 (거의 제자리에서 오간다).
/// 되돌아설 땐 반지름 SWAY_TURN(몸 크기 배)의 반원으로 돌며 위나 아래로 옮겨 간다 — 목표 높이는 SWAY_DRIFT만큼 위나 아래
pub const SWAY_AMP: (f64, f64) = (0.8, 1.6);
pub const SWAY_SWINGS: (usize, usize) = (3, 5);
pub const SWAY_REACH: f64 = 0.6;
pub const SWAY_TURN: f64 = 0.3;
pub const SWAY_DRIFT: (f64, f64) = (2.5, 4.0);
/// 8자: 가로 반폭 (몸 크기 배), 세로 반높이 = 가로의 이 비율, 경유점 수
pub const EIGHT_AMP: (f64, f64) = (1.0, 1.6);
pub const EIGHT_FLAT: f64 = 0.45;
pub const EIGHT_SAMPLES: usize = 12;

/// 날기 한 번의 모양
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// 완만한 굽이 하나 (내려앉기·먹이·창 밖으로)
    Gentle,
    /// 몸 두 개쯤마다 굽이 하나 (최대 MAX_BENDS)
    Wander,
    /// 가다가 원(나선)을 한두 바퀴
    Circle,
    /// 좌우로 곡선을 그리며 왔다갔다 (오가며 천천히 오르거나 내려간다)
    Sway,
    /// 느긋한 가로 8자
    Eight,
}

/// 마음대로 날 때의 모양
pub fn pick_style(rng: &mut fastrand::Rng) -> Style {
    let r = rng.f64();
    if r < CIRCLE_CHANCE {
        Style::Circle
    } else if r < CIRCLE_CHANCE + SWAY_CHANCE {
        Style::Sway
    } else if r < CIRCLE_CHANCE + SWAY_CHANCE + EIGHT_CHANCE {
        Style::Eight
    } else {
        Style::Wander
    }
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// centripetal Catmull-Rom: p1 → p2 구간의 u(0..1) 지점
fn catmull_rom(p0: (f64, f64), p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), u: f64) -> (f64, f64) {
    let knot = |a, b| dist(a, b).sqrt().max(1e-3);
    let t1 = knot(p0, p1);
    let t2 = t1 + knot(p1, p2);
    let t3 = t2 + knot(p2, p3);
    let t = t1 + u * (t2 - t1);
    let lerp = |a: (f64, f64), b: (f64, f64), ta: f64, tb: f64| {
        let w = (t - ta) / (tb - ta);
        (a.0 + (b.0 - a.0) * w, a.1 + (b.1 - a.1) * w)
    };
    let a1 = lerp(p0, p1, 0.0, t1);
    let a2 = lerp(p1, p2, t1, t2);
    let a3 = lerp(p2, p3, t2, t3);
    let b1 = lerp(a1, a2, 0.0, t2);
    let b2 = lerp(a2, a3, t1, t3);
    lerp(b1, b2, t1, t2)
}

/// 출발점과 경유점들을 지나 (따로 받는) 목표까지 가는 곡선, 그리고 지금 어디까지 왔는지
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Path {
    pts: [(f64, f64); MAX_POINTS],
    n: usize,
    seg: usize,
    u: f64,
    pos: (f64, f64),
}

impl Path {
    pub fn new(from: (f64, f64)) -> Self {
        let mut pts = [(0.0, 0.0); MAX_POINTS];
        pts[0] = from;
        Self { pts, n: 1, seg: 0, u: 0.0, pos: from }
    }

    /// 경유점 하나 더 (가득 찼으면 false)
    pub fn push(&mut self, p: (f64, f64)) -> bool {
        if self.n >= MAX_POINTS {
            return false;
        }
        self.pts[self.n] = p;
        self.n += 1;
        true
    }

    /// 출발점과 경유점들 (목표 제외)
    pub fn points(&self) -> &[(f64, f64)] {
        &self.pts[..self.n]
    }

    /// i번째 점 (n번째 = 목표). 양 끝 밖은 거울에 비춘 점
    fn point(&self, i: isize, to: (f64, f64)) -> (f64, f64) {
        let n = self.n as isize;
        let at = |i: isize| if i < n { self.pts[i as usize] } else { to };
        if i < 0 {
            let (a, b) = (at(0), at(1));
            (2.0 * a.0 - b.0, 2.0 * a.1 - b.1)
        } else if i > n {
            let (a, b) = (at(n), at(n - 1));
            (2.0 * a.0 - b.0, 2.0 * a.1 - b.1)
        } else {
            at(i)
        }
    }

    /// seg번째 구간의 u 지점
    pub fn eval(&self, seg: usize, u: f64, to: (f64, f64)) -> (f64, f64) {
        let i = seg as isize;
        catmull_rom(self.point(i - 1, to), self.point(i, to), self.point(i + 1, to), self.point(i + 2, to), u)
    }

    /// 곡선을 따라 거리 d만큼 더 간 지점, 그리고 목표에 다 왔는지 (다 왔으면 정확히 목표)
    pub fn advance(&mut self, d: f64, to: (f64, f64)) -> ((f64, f64), bool) {
        let mut left = d;
        for _ in 0..64 {
            if self.seg >= self.n {
                break;
            }
            if left <= 0.0 {
                return (self.pos, false);
            }
            // 지금 자리에서 곡선의 빠르기 (px / u)를 재서 남은 거리만큼 u를 나아간다
            let h = 1e-3;
            let (ua, ub) = if self.u + h <= 1.0 { (self.u, self.u + h) } else { (self.u - h, self.u) };
            let rate = (dist(self.eval(self.seg, ua, to), self.eval(self.seg, ub, to)) / h).max(1e-6);
            let nu = (self.u + left / rate).min(1.0).min(self.u + 0.05);
            let q = self.eval(self.seg, nu, to);
            left -= dist(self.pos, q);
            self.pos = q;
            self.u = nu;
            if nu >= 1.0 {
                self.seg += 1;
                self.u = 0.0;
            }
        }
        if self.seg >= self.n {
            self.pos = to;
            return (to, true);
        }
        (self.pos, false)
    }
}

/// from → to 곧은 선 위에 k개의 경유점을 고르게 두고, 번갈아 옆으로 벗어나게 한다
fn bends(rng: &mut fastrand::Rng, path: &mut Path, from: (f64, f64), to: (f64, f64), k: usize, amp: (f64, f64), cap: f64, size: f64, b: &Rect) {
    let len = dist(from, to);
    if len < 1e-6 {
        return;
    }
    let (nx, ny) = (-(to.1 - from.1) / len, (to.0 - from.0) / len);
    let mut side = if rng.bool() { 1.0 } else { -1.0 };
    for j in 1..=k {
        let frac = (j as f64 + (rng.f64() - 0.5) * 0.4) / (k + 1) as f64;
        let off = side * (behavior::range(rng, amp) * size).min(cap * len);
        let (x, y) = (from.0 + (to.0 - from.0) * frac + nx * off, from.1 + (to.1 - from.1) * frac + ny * off);
        path.push(clamp(b, x, y));
        side = -side;
    }
}

/// p에서 dir(단위) 쪽으로 날아가며 시작하는 원(나선) 한두 바퀴의 경유점들 — 마지막은 정확히 p. 범위 b에 안 들어가면 None
fn circle(rng: &mut fastrand::Rng, p: (f64, f64), dir: (f64, f64), size: f64, b: &Rect) -> Option<Vec<(f64, f64)>> {
    let r0 = behavior::range(rng, CIRCLE_RADIUS) * size;
    let spiral = (rng.f64() * 2.0 - 1.0) * SPIRAL;
    let turns = if rng.f64() < CIRCLE_TWICE { 2 } else { 1 };
    let reach = r0 * (1.0 + spiral.max(0.0)) * 1.05;
    let fits = |c: (f64, f64)| c.0 - reach >= b.left && c.0 + reach <= b.right && c.1 - reach >= b.top && c.1 + reach <= b.bottom;
    // 왼쪽 법선 (-dy, dx)이면 반시계(+), 오른쪽 법선이면 시계(-) 방향으로 돌아야 처음 방향이 dir
    let mut sides = [((-dir.1, dir.0), 1.0), ((dir.1, -dir.0), -1.0)];
    if rng.bool() {
        sides.swap(0, 1);
    }
    let ((mx, my), s) = sides.into_iter().find(|&((mx, my), _)| fits((p.0 + mx * r0, p.1 + my * r0)))?;
    let c = (p.0 + mx * r0, p.1 + my * r0);
    let phi0 = (-my).atan2(-mx);
    let total = TAU * turns as f64;
    let count = CIRCLE_SAMPLES * turns;
    let mut out: Vec<(f64, f64)> = (1..count)
        .map(|k| {
            let theta = total * k as f64 / count as f64;
            let r = r0 * (1.0 + spiral * (PI * theta / total).sin().powi(2));
            let a = phi0 + s * theta;
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect();
    out.push(p);
    Some(out)
}

/// 날아갈 길을 만든다. b = 몸이 있을 수 있는 범위 (경유점은 그 안)
pub fn build_path(rng: &mut fastrand::Rng, style: Style, from: (f64, f64), to: (f64, f64), b: &Rect, size: f64) -> Path {
    let mut path = Path::new(from);
    let len = dist(from, to);
    let max_k = ((len / (BEND_EVERY * size)).round() as usize).clamp(1, MAX_BENDS);
    match style {
        Style::Gentle => bends(rng, &mut path, from, to, 1, GENTLE_BEND, GENTLE_CAP, size, b),
        Style::Wander => {
            let k = rng.usize(max_k.saturating_sub(1).max(1)..=max_k);
            bends(rng, &mut path, from, to, k, BEND, BEND_CAP, size, b);
        }
        Style::Circle => {
            let k = rng.usize(1..=max_k);
            let mut plain = Path::new(from);
            bends(rng, &mut plain, from, to, k, BEND, BEND_CAP, size, b);
            let pts = plain.points();
            // 원을 그릴 경유점 (출발점 또는 굽이 하나), 거기서 날아가는 방향
            let i = rng.usize(..pts.len());
            let before = if i == 0 { pts[0] } else { pts[i - 1] };
            let after = pts.get(i + 1).copied().unwrap_or(to);
            let (dx, dy) = (after.0 - before.0, after.1 - before.1);
            let l = dx.hypot(dy);
            let ring = if l > 1e-6 { circle(rng, pts[i], (dx / l, dy / l), size, b) } else { None };
            for (j, &p) in pts.iter().enumerate() {
                if j > 0 {
                    path.push(p);
                }
                if j == i {
                    ring.iter().flatten().for_each(|&q| {
                        path.push(q);
                    });
                }
            }
        }
        Style::Sway => {
            // 뱀처럼: 가로로 amp만큼 갔다가 반지름 r의 반원으로 돌아서 (위나 아래로 2r 옮겨) 반대쪽으로 — 되돌아설 때도 둥글다.
            // 높이 차가 모자라 두 번도 못 오가거나, 오갈 폭이 범위에 안 들어가거나, 가로로 너무 많이 옮겨 가야 하면 그냥 굽이굽이
            let r = SWAY_TURN * size;
            let room = (((to.1 - from.1).abs() - r) / (2.0 * r)).max(0.0) as usize;
            let swings = rng.usize(SWAY_SWINGS.0..=SWAY_SWINGS.1).min(room);
            let vd = (to.1 - from.1).signum();
            let mut side = if (to.0 - from.0).abs() > 1.0 { (to.0 - from.0).signum() } else if rng.bool() { 1.0 } else { -1.0 };
            let cx = |j: usize| from.0 + (to.0 - from.0) * j as f64 / (swings + 1) as f64;
            let room_x = (0..swings).map(|j| (cx(j) - b.left).min(b.right - cx(j))).fold(f64::MAX, f64::min);
            let amp = (behavior::range(rng, SWAY_AMP) * size).min(room_x);
            if swings < 2 || amp < SWAY_AMP.0 * size || (to.0 - from.0).abs() / (swings + 1) as f64 > 0.3 * amp {
                return build_path(rng, Style::Wander, from, to, b, size);
            }
            for j in 0..swings {
                // 반원: 바깥 끝 (cx ± amp)에서 둥글게 돈다
                let (tx, y0) = (cx(j) + side * (amp - r), from.1 + vd * 2.0 * r * j as f64);
                for k in 0..=4 {
                    let a = PI * k as f64 / 4.0;
                    path.push((tx + side * r * a.sin(), y0 + vd * r * (1.0 - a.cos())));
                }
                side = -side;
            }
        }
        Style::Eight => {
            let a = behavior::range(rng, EIGHT_AMP) * size;
            let h = EIGHT_FLAT * a;
            // 지금 자리를 가운데로 그린다 — 범위에 안 들어가면 그냥 굽이굽이
            let c = from;
            if c.0 - a < b.left || c.0 + a > b.right || c.1 - h < b.top || c.1 + h > b.bottom {
                return build_path(rng, Style::Wander, from, to, b, size);
            }
            let sx = if (to.0 - from.0).abs() > 1.0 { (to.0 - from.0).signum() } else { 1.0 };
            // 8자의 처음·끝 방향은 (sx, sy) 쪽 — 목표가 있는 쪽으로 들고 나게
            let sy = if (to.1 - c.1).abs() > 1.0 { (to.1 - c.1).signum() } else if rng.bool() { 1.0 } else { -1.0 };
            for k in 1..EIGHT_SAMPLES {
                let phi = TAU * k as f64 / EIGHT_SAMPLES as f64;
                path.push((c.0 + sx * a * phi.sin(), c.1 + sy * h * (2.0 * phi).sin()));
            }
            path.push(c);
            // 8자를 다 그리면 목표로 — 멀면 굽이굽이 (가까운 굽이는 되돌아서며 뾰족해질 수 있어 4몸 넘을 때만)
            let tail = dist(c, to);
            if tail > 4.0 * size {
                let k = ((tail / (BEND_EVERY * size)).round() as usize).clamp(1, MAX_BENDS);
                let mut rest = Path::new(c);
                bends(rng, &mut rest, c, to, k, BEND, BEND_CAP, size, b);
                rest.points()[1..].iter().for_each(|&q| {
                    path.push(q);
                });
            }
        }
    }
    path
}

/// 내려앉을 자리: 디딤판과 발 위치
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perch {
    pub owner: Owner,
    pub x: f64,
    pub y: f64,
}

/// 내려앉을 수 있는 디딤판이면 발 x 범위. 창 안(inside)이면 그 창 안쪽 바닥만, 바깥이면 창 윗면·바닥만.
/// 몸 폭이 들어가야 하고, 앉았을 때 머리와 말풍선 자리(head)가 화면 안이어야 한다
fn perch_range(s: &Surface, inside: Option<u64>, top: f64, half_w: f64, head: f64) -> Option<(f64, f64)> {
    let owner_ok = match (inside, s.owner) {
        (Some(id), Owner::Inside(w)) => id == w,
        (None, Owner::Window(_) | Owner::Ground(_)) => true,
        _ => false,
    };
    (owner_ok && s.width() >= 2.0 * half_w && s.y - head >= top).then_some((s.x_start + half_w, s.x_end - half_w))
}

/// 아무 데나 내려앉을 자리 하나 (없으면 None)
pub fn pick_perch(rng: &mut fastrand::Rng, world: &World, inside: Option<u64>, half_w: f64, head: f64) -> Option<Perch> {
    let spots: Vec<(Owner, f64, (f64, f64))> = world
        .surfaces
        .iter()
        .filter_map(|s| perch_range(s, inside, world.bounds.top, half_w, head).map(|r| (s.owner, s.y, r)))
        .collect();
    if spots.is_empty() {
        return None;
    }
    let (owner, y, range) = spots[rng.usize(..spots.len())];
    Some(Perch { owner, x: behavior::range(rng, range), y })
}

/// (x, y)에서 가장 가까운 내려앉을 자리 — 발보다 아래(같은 높이 포함)에 있는 자리가 있으면 그중에서
pub fn nearest_perch(world: &World, inside: Option<u64>, x: f64, y: f64, half_w: f64, head: f64) -> Option<Perch> {
    let spots: Vec<Perch> = world
        .surfaces
        .iter()
        .filter_map(|s| perch_range(s, inside, world.bounds.top, half_w, head).map(|(lo, hi)| Perch { owner: s.owner, x: x.clamp(lo, hi), y: s.y }))
        .collect();
    let below = spots.iter().any(|p| p.y >= y);
    spots
        .into_iter()
        .filter(|p| !below || p.y >= y)
        .min_by(|a, b| (a.x - x).hypot(a.y - y).total_cmp(&(b.x - x).hypot(b.y - y)))
}

/// 그 높이에 화면 가장 바깥 끝(side)까지 닿는 작업 영역 (발 y 기준)
fn area_at_edge(world: &World, side: Side, y: Option<f64>) -> Option<Rect> {
    let b = world.bounds;
    let mut areas = world.work_areas.iter().filter(|r| match side {
        Side::Left => (r.left - b.left).abs() <= 2.0,
        Side::Right => (r.right - b.right).abs() <= 2.0,
    });
    match y {
        Some(y) => areas.find(|r| r.top < y && y <= r.bottom).copied(),
        // 그 끝에 닿은 영역이 여럿이면 가장 큰 것
        None => areas.max_by(|a, b| a.height().total_cmp(&b.height())).copied(),
    }
}

/// 몸이 화면 가장 바깥 끝에서 몸 하나(EDGE_ZONE) 안으로 들어와 그쪽(facing)으로 날고 있으면 그 끝.
/// 그 높이의 작업 영역이 그 끝까지 닿아 있어야 한다 (모니터 사이 경계는 끝이 아니다)
pub fn edge_ahead(world: &World, x: f64, y: f64, half_w: f64, size: f64, facing: f64) -> Option<Side> {
    let b = world.bounds;
    let side = visit::side_of(facing);
    let gap = match side {
        Side::Left => x - half_w - b.left,
        Side::Right => b.right - (x + half_w),
    };
    (gap <= EDGE_ZONE * size && area_at_edge(world, side, Some(y)).is_some()).then_some(side)
}

/// 화면 끝(side) 쪽으로 나가려고 날아갈 지점 — 그 끝의 작업 영역 안, 끝에서 몸 반 개 안쪽까지
pub fn edge_target(rng: &mut fastrand::Rng, world: &World, side: Side, half_w: f64, head: f64, size: f64) -> Option<(f64, f64)> {
    let area = area_at_edge(world, side, None)?;
    let fb = fly_bounds(&area, half_w, head);
    let (_, y) = pick_target(rng, &fb);
    let inset = rng.f64() * 0.5 * size;
    let x = match side {
        Side::Left => (fb.left + inset).min(fb.right),
        Side::Right => (fb.right - inset).max(fb.left),
    };
    Some((x, y))
}

/// 화면 끝(side) 바깥에서 날아 들어올 출발점: 몸이 막 화면 밖에 있는 자리, 그 끝 작업 영역의 위쪽 40% 높이
pub fn entry_point(world: &World, side: Side, half_w: f64, head: f64) -> (f64, f64) {
    let b = world.bounds;
    let area = area_at_edge(world, side, None).unwrap_or(b);
    let fb = fly_bounds(&area, half_w, head);
    let y = fb.top + 0.4 * fb.height();
    let x = match side {
        Side::Left => b.left - half_w,
        Side::Right => b.right + half_w,
    };
    (x, y)
}

/// 어디로 날아가는지
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Goal {
    /// 그냥 그 지점까지 (도착하면 떠서 쉰다)
    Point(f64, f64),
    /// 디딤판 owner의 기준점에서 rel_x인 자리에 내려앉는다 (창이 움직이면 목표도 따라간다)
    Perch { owner: Owner, rel_x: f64 },
    /// 하우스 문으로
    Home,
}

/// 공중제비 진행
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loop {
    pub start: (f64, f64),
    pub theta: f64,
    pub up: bool,
}

/// 날아가는 길 하나
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flight {
    pub goal: Goal,
    pub from: (f64, f64),
    /// 지금까지 간 거리 (공중제비는 빼고)
    pub s: f64,
    pub wave: Wave,
    /// 이 거리에서 공중제비를 돈다 (Roll)
    pub loop_at: Option<f64>,
    pub looping: Option<Loop>,
    /// 화면 끝 나가기를 이미 정했는지 (날기마다 한 번)
    pub edge_checked: bool,
    pub elapsed: f64,
    /// 길 모양 — 곡선(path)은 처음 날 때 (범위를 알 때) 만든다. path가 없고 만들지도 않으면 wave를 따라간다 (집으로)
    pub style: Style,
    pub path: Option<Path>,
    /// 마지막으로 좌우를 뒤집은 때 (elapsed)
    pub flipped_at: f64,
}

impl Flight {
    pub fn new(goal: Goal, from: (f64, f64), wave: Wave) -> Self {
        Self { goal, from, s: 0.0, wave, loop_at: None, looping: None, edge_checked: false, elapsed: 0.0, style: Style::Gentle, path: None, flipped_at: f64::NEG_INFINITY }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::WindowInfo;

    fn world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    #[test]
    fn numbers() {
        assert_eq!((RUN_CHANCE, PERCH_CHANCE, LOOP_RADIUS, EDGE_ZONE), (0.3, 0.35, 0.75, 1.0));
        assert_eq!((speed_mult(Move::Walk), speed_mult(Move::Run)), (1.0, 1.8));
    }

    #[test]
    fn move_mix_is_mostly_plain_flight_with_30_percent_fast() {
        let mut rng = fastrand::Rng::with_seed(4);
        let n = 20_000;
        let picks: Vec<Move> = (0..n).map(|_| pick_move(&mut rng)).collect();
        let share = |m: Move| picks.iter().filter(|&&p| p == m).count() as f64 / n as f64;
        assert!((0.28..=0.32).contains(&share(Move::Run)));
        assert!((0.08..=0.12).contains(&share(Move::Roll)));
        assert!((0.58..=0.62).contains(&share(Move::Walk)));
    }

    #[test]
    fn fly_bounds_keep_the_body_and_head_room_inside() {
        let b = fly_bounds(&Rect::new(0.0, 0.0, 1920.0, 1040.0), 38.0, 166.4);
        assert_eq!(b, Rect::new(38.0, 166.4, 1882.0, 1040.0));
        // 너무 좁거나 낮은 영역은 한 점·한 줄로
        let tiny = fly_bounds(&Rect::new(100.0, 500.0, 150.0, 600.0), 38.0, 166.4);
        assert_eq!((tiny.left, tiny.right, tiny.top, tiny.bottom), (125.0, 125.0, 600.0, 600.0));
        assert_eq!(clamp(&b, -5.0, 2000.0), (38.0, 1040.0));
    }

    #[test]
    fn targets_are_inside_and_only_a_little_upward() {
        let b = Rect::new(38.0, 166.0, 1882.0, 1040.0);
        let mut rng = fastrand::Rng::with_seed(9);
        let ts: Vec<(f64, f64)> = (0..5000).map(|_| pick_target(&mut rng, &b)).collect();
        assert!(ts.iter().all(|&(x, y)| x >= b.left && x <= b.right && y >= b.top && y <= b.bottom));
        let upper = ts.iter().filter(|t| t.1 <= b.top + b.height() * 2.0 / 3.0).count() as f64 / 5000.0;
        assert!((0.67..=0.73).contains(&upper), "{upper}");
    }

    #[test]
    fn path_starts_and_ends_exactly_and_waves_in_between() {
        let wave = Wave::Sine { amp: 20.0, len: 384.0 };
        let (from, to) = ((100.0, 500.0), (900.0, 300.0));
        assert_eq!(path_point(from, to, 0.0, wave), (from, false));
        let len = (800.0f64).hypot(200.0);
        assert_eq!(path_point(from, to, len, wave), (to, true));
        assert_eq!(path_point(from, to, len + 50.0, wave), (to, true));
        // 중간엔 직선에서 벗어난다 (진폭 이내)
        let mut max_off: f64 = 0.0;
        for i in 1..100 {
            let s = len * i as f64 / 100.0;
            let ((x, y), done) = path_point(from, to, s, wave);
            assert!(!done);
            let u = s / len;
            let off = (x - (100.0 + 800.0 * u)).hypot(y - (500.0 - 200.0 * u));
            assert!(off <= 20.0 + 1e-9);
            max_off = max_off.max(off);
        }
        assert!(max_off > 10.0, "{max_off}");
        // 활: 위로 솟는다
        let ((_, y), _) = path_point((0.0, 1000.0), (1000.0, 1000.0), 500.0, Wave::Arc { height: 100.0 });
        assert!((y - 900.0).abs() < 1e-9);
    }

    /// 곡선을 끝까지 따라가며 (지나간 점들, 가장 큰 한 걸음)
    fn walk(mut path: Path, to: (f64, f64), step: f64) -> (Vec<(f64, f64)>, f64) {
        let (mut pts, mut prev, mut max_step) = (vec![path.points()[0]], path.points()[0], 0.0f64);
        for _ in 0..100_000 {
            let (p, done) = path.advance(step, to);
            max_step = max_step.max(dist(prev, p));
            prev = p;
            pts.push(p);
            if done {
                return (pts, max_step);
            }
        }
        panic!("never arrived");
    }

    #[test]
    fn curved_path_passes_its_waypoints_and_ends_exactly_at_the_goal() {
        let mut path = Path::new((100.0, 500.0));
        for p in [(300.0, 300.0), (500.0, 700.0), (700.0, 300.0)] {
            assert!(path.push(p));
        }
        let to = (900.0, 500.0);
        let (pts, max_step) = walk(path, to, 2.0);
        assert_eq!(*pts.last().unwrap(), to);
        // 한 걸음은 거의 정확히 그 거리 (빠르기가 고르다)
        let steps: Vec<f64> = pts.windows(2).map(|w| dist(w[0], w[1])).collect();
        assert!(max_step <= 2.1, "{max_step}");
        assert!(steps[..steps.len() - 1].iter().all(|&d| (d - 2.0).abs() < 0.1), "{steps:?}");
        for w in [(300.0, 300.0), (500.0, 700.0), (700.0, 300.0)] {
            let near = pts.iter().map(|&p| dist(p, w)).fold(f64::MAX, f64::min);
            assert!(near < 2.0, "{w:?} {near}");
        }
        // 가득 차면 더 받지 않는다
        let mut full = Path::new((0.0, 0.0));
        assert!((1..MAX_POINTS).all(|i| full.push((i as f64, 0.0))));
        assert!(!full.push((0.0, 0.0)));
    }

    /// 걸음마다의 방향 변화: (전체 감은 양의 폭, 가장 큰 한 걸음 꺾임)
    fn winding(pts: &[(f64, f64)]) -> (f64, f64) {
        let (mut wind, mut lo, mut hi, mut kink) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let hs: Vec<f64> = pts.windows(2).filter(|w| dist(w[0], w[1]) > 1e-6).map(|w| (w[1].1 - w[0].1).atan2(w[1].0 - w[0].0)).collect();
        for h in hs.windows(2) {
            let dh = (h[1] - h[0] + PI).rem_euclid(TAU) - PI;
            wind += dh;
            (lo, hi, kink) = (lo.min(wind), hi.max(wind), kink.max(dh.abs()));
        }
        (hi - lo, kink)
    }

    #[test]
    fn every_style_stays_inside_is_smooth_and_arrives() {
        let b = Rect::new(38.0, 166.4, 1882.0, 1040.0);
        let mut rng = fastrand::Rng::with_seed(7);
        let mut circles = 0;
        for i in 0..400 {
            let style = [Style::Gentle, Style::Wander, Style::Circle, Style::Sway, Style::Eight][i % 5];
            let from = pick_target(&mut rng, &b);
            let to = pick_target(&mut rng, &b);
            let path = build_path(&mut rng, style, from, to, &b, 128.0);
            assert!(path.points().iter().all(|&(x, y)| x >= b.left && x <= b.right && y >= b.top && y <= b.bottom), "{style:?} {:?}", path.points());
            let (pts, _) = walk(path, to, 1.5);
            assert_eq!(*pts.last().unwrap(), to);
            // 곡선이 범위를 아주 조금만 넘는다 (넘는 만큼은 날 때 잘린다)
            assert!(pts.iter().all(|&(x, y)| x >= b.left - 40.0 && x <= b.right + 40.0 && y >= b.top - 40.0 && y <= b.bottom + 40.0), "{style:?}");
            let (wind, kink) = winding(&pts);
            // 꺾이지 않는다: 1.5px 걸음마다 0.35 rad 미만
            assert!(kink < 0.35, "{style:?} kink {kink} {from:?} {to:?} {:?}", path.points());
            if style == Style::Circle && wind >= TAU - 0.3 {
                circles += 1;
            }
        }
        // 원 모양은 대개 정말 한 바퀴 넘게 돈다 (범위에 안 들어가면 그냥 굽이굽이)
        assert!(circles >= 60, "circles {circles}");
    }

    #[test]
    fn sway_goes_left_and_right_and_eight_crosses_itself() {
        let b = Rect::new(38.0, 166.4, 1882.0, 1040.0);
        let mut rng = fastrand::Rng::with_seed(3);
        // 높이 차가 없으면 살랑이지 않는다 (뾰족하게 되돌아서지 않게)
        let flat = build_path(&mut rng, Style::Sway, (900.0, 600.0), (1000.0, 560.0), &b, 128.0);
        assert!(flat.points().len() <= 1 + MAX_BENDS && flat.points().iter().all(|p| p.0 >= 850.0 && p.0 <= 1050.0), "{:?}", flat.points());
        let path = build_path(&mut rng, Style::Sway, (900.0, 600.0), (950.0, 200.0), &b, 128.0);
        let xs: Vec<f64> = path.points().iter().map(|p| p.0).collect();
        assert!(xs.len() >= 4);
        // 반원 (경유점 5개)이 번갈아 오른쪽·왼쪽 (첫 번째는 목표 쪽)
        assert!(xs[1..6].iter().all(|&x| x > 950.0) && xs[6..11].iter().all(|&x| x < 850.0), "{xs:?}");
        let (pts, _) = walk(path, (950.0, 200.0), 1.0);
        let (lo, hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
        assert!(hi - lo > 1.5 * 128.0, "{lo} {hi}");
        assert!(winding(&pts).1 < 0.35, "kink {}", winding(&pts).1);
        // 8자: 가운데에서 시작해 한가운데를 다시 지나고 가운데로 돌아온다
        let path = build_path(&mut rng, Style::Eight, (900.0, 600.0), (1200.0, 600.0), &b, 128.0);
        let n = path.points().iter().filter(|&&p| dist(p, (900.0, 600.0)) < 1e-6).count();
        assert_eq!(n, 3, "{:?}", path.points());
        let (pts, _) = walk(path, (1200.0, 600.0), 1.0);
        assert!(pts.iter().any(|p| p.0 < 900.0 - 100.0) && pts.iter().any(|p| p.0 > 900.0 + 100.0));
    }

    #[test]
    fn style_mix() {
        let mut rng = fastrand::Rng::with_seed(5);
        let n = 20_000;
        let picks: Vec<Style> = (0..n).map(|_| pick_style(&mut rng)).collect();
        let share = |s: Style| picks.iter().filter(|&&p| p == s).count() as f64 / n as f64;
        assert!((0.23..=0.27).contains(&share(Style::Circle)));
        assert!((0.13..=0.17).contains(&share(Style::Sway)));
        assert!((0.08..=0.12).contains(&share(Style::Eight)));
        assert!((0.48..=0.52).contains(&share(Style::Wander)));
    }

    #[test]
    fn loop_goes_round_and_comes_back() {
        let start = (500.0, 500.0);
        assert_eq!(loop_point(start, 96.0, 0.0, 1.0, true), start);
        let top = loop_point(start, 96.0, PI, 1.0, true);
        assert!((top.0 - 500.0).abs() < 1e-9 && (top.1 - 308.0).abs() < 1e-9);
        let fwd = loop_point(start, 96.0, PI / 2.0, -1.0, true);
        assert!((fwd.0 - 404.0).abs() < 1e-9 && (fwd.1 - 404.0).abs() < 1e-9);
        let down = loop_point(start, 96.0, PI, 1.0, false);
        assert!((down.1 - 692.0).abs() < 1e-9);
        let back = loop_point(start, 96.0, TAU, 1.0, true);
        assert!((back.0 - 500.0).abs() < 1e-9 && (back.1 - 500.0).abs() < 1e-9);
    }

    #[test]
    fn drift_slows_down_and_bob_is_small() {
        let (vx, vy) = drift(1000.0, -500.0, 1.0);
        assert!((vx - 1000.0 * (-3.0f64).exp()).abs() < 1e-9 && vy < 0.0 && vy > -30.0);
        assert!((0..1000).all(|i| bob(i as f64 * 0.01, 128.0).abs() <= 128.0 * BOB_AMP + 1e-9));
    }

    #[test]
    fn perches_are_window_tops_or_ground_with_head_room() {
        // 창 2는 화면 꼭대기에 너무 가까워 앉으면 말풍선이 화면 밖
        let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0), win(2, 1300.0, 100.0, 1700.0, 600.0)]);
        let mut rng = fastrand::Rng::with_seed(2);
        let head = 128.0 * 1.3;
        let mut owners = std::collections::HashSet::new();
        for _ in 0..500 {
            let p = pick_perch(&mut rng, &w, None, 38.0, head).unwrap();
            assert!(matches!(p.owner, Owner::Window(1) | Owner::Ground(0)), "{p:?}");
            let s = w.piece_at(p.owner, p.x).unwrap();
            assert!(p.x - 38.0 >= s.x_start && p.x + 38.0 <= s.x_end && p.y == s.y);
            owners.insert(p.owner);
        }
        assert_eq!(owners.len(), 2);
        // 창 안이면 그 창 안쪽 바닥만
        let p = pick_perch(&mut rng, &w, Some(1), 38.0, head).unwrap();
        assert_eq!((p.owner, p.y), (Owner::Inside(1), 896.0));
    }

    #[test]
    fn nearest_perch_prefers_spots_below() {
        let w = world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
        let head = 128.0 * 1.3;
        assert_eq!(nearest_perch(&w, None, 800.0, 300.0, 38.0, head).map(|p| (p.owner, p.x, p.y)), Some((Owner::Window(1), 800.0, 400.0)));
        // 창 윗면보다 아래면 바닥으로 (위로 날아 올라가지 않는다)
        assert_eq!(nearest_perch(&w, None, 1050.0, 600.0, 38.0, head).map(|p| p.owner), Some(Owner::Ground(0)));
        // 창 옆 바깥: 윗면 끝 안쪽으로 맞춘다
        let p = nearest_perch(&w, None, 400.0, 350.0, 38.0, head).unwrap();
        assert_eq!((p.owner, p.x), (Owner::Window(1), 538.0));
    }

    #[test]
    fn edge_ahead_needs_the_zone_the_direction_and_an_outer_edge() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 200.0, 3840.0, 1240.0)], 128.0);
        // 오른쪽 끝(3840)에서 몸 하나 안
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 600.0, 38.0, 128.0, 1.0), Some(Side::Right));
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 140.0, 600.0, 38.0, 128.0, 1.0), None);
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 600.0, 38.0, 128.0, -1.0), None);
        // 오른쪽 모니터가 없는 높이 (y 150)
        assert_eq!(edge_ahead(&w, 3840.0 - 38.0 - 100.0, 150.0, 38.0, 128.0, 1.0), None);
        // 모니터 사이 경계는 끝이 아니다
        assert_eq!(edge_ahead(&w, 1920.0 - 38.0 - 50.0, 600.0, 38.0, 128.0, 1.0), None);
        assert_eq!(edge_ahead(&w, 38.0 + 10.0, 600.0, 38.0, 128.0, -1.0), Some(Side::Left));
    }

    #[test]
    fn edge_targets_and_entry_points_sit_at_that_edge() {
        let w = world(vec![]);
        let mut rng = fastrand::Rng::with_seed(1);
        for _ in 0..200 {
            let (x, y) = edge_target(&mut rng, &w, Side::Right, 38.0, 166.4, 128.0).unwrap();
            assert!(x <= 1882.0 && x >= 1882.0 - 64.0 && (166.4..=1040.0).contains(&y));
            assert_eq!(edge_ahead(&w, x, y, 38.0, 128.0, 1.0), Some(Side::Right));
        }
        let (x, y) = entry_point(&w, Side::Left, 38.0, 166.4);
        assert_eq!(x, -38.0);
        assert!((y - (166.4 + 0.4 * (1040.0 - 166.4))).abs() < 1e-9);
        assert_eq!(entry_point(&w, Side::Right, 38.0, 166.4).0, 1958.0);
    }
}
