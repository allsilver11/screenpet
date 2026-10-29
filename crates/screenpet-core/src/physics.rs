use crate::geom::Rect;

pub const GRAVITY: f64 = 2500.0;
pub const WALL_DAMPING: f64 = 0.6;
/// 던진 비행 (손에서 놓은 직후 공중): 속도·중력을 논리 px로 잰다 → 화면 배율(dpi)이 달라도 같은 느낌.
/// 놓는 순간 커서 속도에 곱하는 비율 (부드럽게)
pub const THROW_FEEL: f64 = 0.55;
/// 가로 속도의 이 비율만큼 위로 띄운다 → 옆으로 휙 던져도 둥근 포물선 (나는 동물은 0)
pub const THROW_LIFT: f64 = 0.3;
/// 던진 속도 상한 (논리 px/s, × dpi)
pub const MAX_THROW_SPEED_LOGICAL: f64 = 1600.0;
/// 던진 비행의 중력 (논리 px/s², × dpi) — 평소보다 약해서 포물선이 둥글게 보인다
pub const THROW_GRAVITY_LOGICAL: f64 = 1700.0;

/// x = 발 중심, y = 발바닥 (화면 아래로 +).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

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

/// 손에서 놓을 때의 커서 속도(물리 px/s) → 던진 속도: THROW_FEEL을 곱하고, 가로 속도 × lift만큼 위로 띄우고,
/// MAX_THROW_SPEED_LOGICAL × dpi로 자른다
pub fn throw_velocity(vx: f64, vy: f64, dpi: f64, lift: f64) -> (f64, f64) {
    let (vx, vy) = (vx * THROW_FEEL, vy * THROW_FEEL);
    clamp_speed(vx, vy - lift * vx.abs(), MAX_THROW_SPEED_LOGICAL * dpi)
}

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
