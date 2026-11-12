//! 🪂 몸 물리 — 몸(Body: 발 자리와 속도), 공중 한 걸음(step_air), 던지는 속도 계산
//! 동물(brain.rs)과 물건(object.rs)이 같이 씀. 던진 비행은 논리 px로 재서 화면 배율과 상관없이 같은 느낌임
use crate::geom::Rect;

/// 평소 중력 (px/s², 화면 아래로 +)
pub const GRAVITY: f64 = 2500.0;
/// 화면 옆 끝에 부딪혀 튕길 때 남는 가로 속도 비율
pub const WALL_DAMPING: f64 = 0.6;
/// 던진 비행 (손에서 놓은 직후 공중): 속도·중력을 논리 px로 잼 → 화면 배율(dpi)이 달라도 같은 느낌.
/// 놓는 순간 커서 속도에 곱하는 비율 (부드럽게)
pub const THROW_FEEL: f64 = 0.55;
/// 가로 속도의 이 비율만큼 위로 띄움 → 옆으로 휙 던져도 둥근 포물선 (나는 동물은 0)
pub const THROW_LIFT: f64 = 0.3;
/// 던진 속도 상한 (논리 px/s, × dpi)
pub const MAX_THROW_SPEED_LOGICAL: f64 = 1600.0;
/// 던진 비행의 중력 (논리 px/s², × dpi) — 평소보다 약해서 포물선이 둥글게 보임
pub const THROW_GRAVITY_LOGICAL: f64 = 1700.0;

/// x = 발 중심, y = 발바닥 (화면 아래로 +).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    /// 발 중심 x
    pub x: f64,
    /// 발바닥 y
    pub y: f64,
    /// 가로 속도 (px/s)
    pub vx: f64,
    /// 세로 속도 (px/s, 아래로 +)
    pub vy: f64,
}

impl Body {
    /// 포물선 위로 step초 (중력을 정확히 적분해 계산한 도착점에 닿음)
    pub fn arc_step(&mut self, step: f64) {
        self.x += self.vx * step;
        self.y += self.vy * step + 0.5 * GRAVITY * step * step;
        self.vy += GRAVITY * step;
    }
}

/// 공중 한 걸음: 중력을 더해 움직이고, 화면 옆 끝에서 튕기고 위 끝에서 멈춤
pub fn step_air(b: &mut Body, dt: f64, bounds: &Rect, half_w: f64, height: f64, gravity: f64) {
    b.vy += gravity * dt;
    b.x += b.vx * dt;
    b.y += b.vy * dt;

    if b.x - half_w < bounds.left {
        b.x = bounds.left + half_w;
        b.vx = b.vx.abs() * WALL_DAMPING;
    } else if b.x + half_w > bounds.right {
        b.x = bounds.right - half_w;
        b.vx = -b.vx.abs() * WALL_DAMPING;
    }
    if b.y - height < bounds.top {
        b.y = bounds.top + height;
        b.vy = b.vy.max(0.0);
    }
}

/// x에서 target 쪽으로 step까지 간 자리 (닿으면 정확히 target)
pub fn step_toward(x: f64, target: f64, step: f64) -> f64 {
    let dx = target - x;
    if dx.abs() <= step {
        target
    } else {
        x + dx.signum() * step
    }
}

/// 손에서 놓을 때의 커서 속도(물리 px/s) → 던진 속도: THROW_FEEL을 곱하고, 가로 속도 × lift만큼 위로 띄우고,
/// MAX_THROW_SPEED_LOGICAL × dpi로 자름
pub fn throw_velocity(vx: f64, vy: f64, dpi: f64, lift: f64) -> (f64, f64) {
    let (vx, vy) = (vx * THROW_FEEL, vy * THROW_FEEL);
    clamp_speed(vx, vy - lift * vx.abs(), MAX_THROW_SPEED_LOGICAL * dpi)
}

/// 손을 놓기 전 이만큼(초)의 커서 기록으로 던지는 속도를 잼
pub const VELOCITY_WINDOW: f64 = 0.12;
/// 처음·마지막 한 구간이 나머지 중간값의 이 배보다 빠르면 튄 이벤트로 봄
const OUTLIER_RATIO: f64 = 3.0;

/// 커서 기록 (시각, x, y)의 처음→끝 평균 속도 (물리 px/s). 같은 틱의 기록은 마지막 것만 쓰고,
/// 처음이나 마지막 한 구간만 유난히 빠르면(튄 이벤트) 그 기록 하나를 뺌
pub fn release_velocity<'a>(samples: impl Iterator<Item = &'a (f64, f64, f64)>) -> (f64, f64) {
    let mut pts: Vec<(f64, f64, f64)> = Vec::new();
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

/// 속도가 max를 넘으면 방향은 두고 크기만 max로 줄임
pub fn clamp_speed(vx: f64, vy: f64, max: f64) -> (f64, f64) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed <= max {
        (vx, vy)
    } else {
        let k = max / speed;
        (vx * k, vy * k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: Rect = Rect { left: 0.0, top: 0.0, right: 1000.0, bottom: 1000.0 };

    #[test]
    fn gravity_accelerates_downward() {
        let mut b = Body { x: 500.0, y: 500.0, vx: 0.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0, GRAVITY);
        assert!((b.vy - 250.0).abs() < 1e-9);
        assert!(b.y > 500.0);
    }

    #[test]
    fn bounces_off_left_wall_with_damping() {
        let mut b = Body { x: 40.0, y: 500.0, vx: -1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0, GRAVITY);
        assert_eq!(b.x, 38.0);
        assert!((b.vx - 600.0).abs() < 1e-9);
    }

    #[test]
    fn bounces_off_right_wall_with_damping() {
        let mut b = Body { x: 960.0, y: 500.0, vx: 1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0, GRAVITY);
        assert_eq!(b.x, 962.0);
        assert!((b.vx + 600.0).abs() < 1e-9);
    }

    #[test]
    fn ceiling_stops_upward_motion() {
        let mut b = Body { x: 500.0, y: 140.0, vx: 0.0, vy: -2000.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0, GRAVITY);
        assert_eq!(b.y, 128.0);
        assert_eq!(b.vy, 0.0);
    }

    #[test]
    fn clamp_speed_limits_magnitude() {
        let (vx, vy) = clamp_speed(6000.0, 8000.0, 3000.0);
        assert!(((vx * vx + vy * vy).sqrt() - 3000.0).abs() < 1e-6);
        assert_eq!(clamp_speed(100.0, -50.0, 3000.0), (100.0, -50.0));
    }

    #[test]
    fn throw_velocity_is_gentler_and_clamped_in_logical_px() {
        let (vx, vy) = throw_velocity(1000.0, -400.0, 1.0, 0.0);
        assert!((vx - 1000.0 * THROW_FEEL).abs() < 1e-9 && (vy + 400.0 * THROW_FEEL).abs() < 1e-9);
        let (vx, vy) = throw_velocity(-1000.0, 0.0, 1.0, THROW_LIFT);
        assert!((vx + 1000.0 * THROW_FEEL).abs() < 1e-9 && (vy + THROW_LIFT * 1000.0 * THROW_FEEL).abs() < 1e-9);
        for dpi in [1.0, 1.5, 2.0] {
            let (vx, vy) = throw_velocity(0.0, 20000.0 * dpi, dpi, THROW_LIFT);
            assert!((vy - MAX_THROW_SPEED_LOGICAL * dpi).abs() < 1e-6 && vx == 0.0, "{dpi}: {vy}");
        }
    }

    #[test]
    fn custom_gravity_is_used() {
        let mut b = Body { x: 500.0, y: 500.0, vx: 0.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0, 1700.0);
        assert!((b.vy - 170.0).abs() < 1e-9);
    }
}
