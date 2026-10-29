use crate::geom::Rect;

pub const GRAVITY: f64 = 2500.0;
pub const WALL_DAMPING: f64 = 0.6;
pub const MAX_THROW_SPEED: f64 = 3000.0;

/// x = 발 중심, y = 발바닥 (화면 아래로 +).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Body {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

pub fn step_air(b: &mut Body, dt: f64, bounds: &Rect, half_w: f64, height: f64) {
    b.vy += GRAVITY * dt;
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

pub fn clamp_speed(vx: f64, vy: f64) -> (f64, f64) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed <= MAX_THROW_SPEED {
        (vx, vy)
    } else {
        let k = MAX_THROW_SPEED / speed;
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
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert!((b.vy - 250.0).abs() < 1e-9);
        assert!(b.y > 500.0);
    }

    #[test]
    fn bounces_off_left_wall_with_damping() {
        let mut b = Body { x: 40.0, y: 500.0, vx: -1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.x, 38.0);
        assert!((b.vx - 600.0).abs() < 1e-9);
    }

    #[test]
    fn bounces_off_right_wall_with_damping() {
        let mut b = Body { x: 960.0, y: 500.0, vx: 1000.0, vy: 0.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.x, 962.0);
        assert!((b.vx + 600.0).abs() < 1e-9);
    }

    #[test]
    fn ceiling_stops_upward_motion() {
        let mut b = Body { x: 500.0, y: 140.0, vx: 0.0, vy: -2000.0 };
        step_air(&mut b, 0.1, &B, 38.0, 128.0);
        assert_eq!(b.y, 128.0);
        assert_eq!(b.vy, 0.0);
    }

    #[test]
    fn clamp_speed_limits_magnitude() {
        let (vx, vy) = clamp_speed(6000.0, 8000.0);
        assert!(((vx * vx + vy * vy).sqrt() - MAX_THROW_SPEED).abs() < 1e-6);
        assert_eq!(clamp_speed(100.0, -50.0), (100.0, -50.0));
    }
}
