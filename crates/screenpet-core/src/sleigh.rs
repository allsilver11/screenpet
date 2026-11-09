//! 🛷 썰매 세리머니 (순수 계산): 한 화면에서 루돌프와 산타가 만나면 썰매로 합쳐 화면을 한 바퀴 날고
//! 처음 자리에 내려앉아 다시 둘로 돌아간다. 창은 src-tauri/src/sleigh.rs.

use crate::brain::Pet;
use crate::geom::Rect;

/// 썰매를 끄는 동물과 타는 동물 (동물 폴더 이름)
pub const PULLER: &str = "rudolph";
pub const RIDER: &str = "santa";
/// 한 바퀴에 걸리는 초 (뜨기·내려앉기 포함)
pub const RIDE_SECS: f64 = 9.0;
/// 한 번 날고 나면 같은 짝은 이만큼(초) 지나야 다시 난다
pub const COOLDOWN_SECS: f64 = 600.0;
/// 뜨는 데, 내려앉는 데 쓰는 몫
const TAKEOFF: f64 = 0.12;
/// 앞이 들리고 숙는 가장 큰 각도 (도)
pub const MAX_TILT: f64 = 35.0;
/// 썰매·산타·루돌프가 차지하는 상자 (큰 동물 한 변의 배수: 너비, 높이) — ui/sleigh.js와 같다
pub const RIDE_BOX: (f64, f64) = (2.5, 1.25);
/// 처음 몇 초 산타 위에 뜨는 말풍선 자리 (한 변의 배수)
pub const BUBBLE: f64 = 0.35;

/// 썰매 창 (논리 px): 썰매 아래 가운데(도는 축)에서 MAX_TILT까지 기울어도 상자가 다 들어간다.
/// below = 축 아래로 남기는 높이 (기울면 한쪽 끝이 축보다 내려간다)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowBox {
    pub width: f64,
    pub height: f64,
    pub below: f64,
}

pub fn window_box(unit: f64) -> WindowBox {
    let (w, h) = (RIDE_BOX.0 * unit, (RIDE_BOX.1 + BUBBLE) * unit);
    let (sin, cos) = MAX_TILT.to_radians().sin_cos();
    let margin = unit * 0.1;
    // 가장 많이 기울었을 때의 모서리: 옆으로 w/2·cos + h·sin, 위로 w/2·sin + h·cos, 아래로 w/2·sin
    WindowBox {
        width: 2.0 * (w / 2.0 * cos + h * sin) + 2.0 * margin,
        height: (w / 2.0 * sin + h * cos) + w / 2.0 * sin + 2.0 * margin,
        below: w / 2.0 * sin + margin,
    }
}

/// 썰매에 탈 수 있다: 디딤판 위에서 걷거나 쉬는 중이고, 들리거나 탑에 있거나 조작·하우스·외출 중이 아니다
pub fn ready(p: &Pet) -> bool {
    p.social_free()
        && p.standing.is_some()
        && !p.is_held()
        && !p.is_on_head()
        && !p.has_rider()
        && !p.is_meeting()
        && !p.is_housed()
        && !p.is_away()
        && !p.is_controlled()
}

/// 둘이 만났다: 같은 높이에서 몸이 거의 닿는다. (x, y, 몸 크기) — 물리 px
pub fn met(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
    (a.0 - b.0).abs() < (a.2 + b.2) * 0.4 && (a.1 - b.1).abs() < a.2.min(b.2) * 0.3
}

/// 날고 있는 썰매의 모습
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// 썰매 가운데 아래 (물리 px)
    pub x: f64,
    pub y: f64,
    /// 오른쪽이 +1, 왼쪽이 -1
    pub facing: f64,
    /// 앞으로 들린 각도 (도, 위로가 +)
    pub tilt: f64,
}

/// 한 바퀴: 처음 자리 → 작업 영역 안의 큰 타원 (아래에서 오른쪽으로 출발해 한 바퀴) → 처음 자리
#[derive(Debug, Clone)]
pub struct Lap {
    start: (f64, f64),
    area: Rect,
    t: f64,
    facing: f64,
}

impl Lap {
    pub fn new(start: (f64, f64), area: Rect) -> Self {
        Lap { start, area, t: 0.0, facing: 1.0 }
    }

    /// 시간을 보낸다. 다 돌았으면 true
    pub fn tick(&mut self, dt: f64) -> bool {
        if dt.is_finite() && dt > 0.0 {
            self.t = (self.t + dt).min(RIDE_SECS);
        }
        self.t >= RIDE_SECS
    }

    /// 0..1 진행
    pub fn progress(&self) -> f64 {
        self.t / RIDE_SECS
    }

    pub fn pose(&mut self) -> Pose {
        let u = self.progress();
        let (x, y) = self.at(u);
        let (nx, ny) = self.at((u + 0.004).min(1.0));
        let (dx, dy) = if u >= 1.0 { let (px, py) = self.at(0.996); (x - px, y - py) } else { (nx - x, ny - y) };
        if dx.abs() > 0.01 {
            self.facing = dx.signum();
        }
        let tilt = if dx.abs() + dy.abs() < 1e-9 { 0.0 } else { (-dy).atan2(dx.abs()).to_degrees().clamp(-MAX_TILT, MAX_TILT) };
        Pose { x, y, facing: self.facing, tilt }
    }

    fn ellipse(&self, theta: f64) -> (f64, f64) {
        let a = &self.area;
        let (cx, cy) = ((a.left + a.right) / 2.0, a.top + a.height() * 0.5);
        let (rx, ry) = (a.width() * 0.4, a.height() * 0.32);
        (cx + rx * theta.cos(), cy + ry * theta.sin())
    }

    /// u = 0..1 에서의 자리
    fn at(&self, u: f64) -> (f64, f64) {
        use std::f64::consts::{FRAC_PI_2, TAU};
        let bottom = self.ellipse(FRAC_PI_2);
        let lerp = |a: (f64, f64), b: (f64, f64), k: f64| (a.0 + (b.0 - a.0) * k, a.1 + (b.1 - a.1) * k);
        let ease = |k: f64| k * k * (3.0 - 2.0 * k);
        if u <= TAKEOFF {
            lerp(self.start, bottom, ease(u / TAKEOFF))
        } else if u >= 1.0 - TAKEOFF {
            lerp(bottom, self.start, ease((u - (1.0 - TAKEOFF)) / TAKEOFF))
        } else {
            // 아래에서 오른쪽으로 → 오른쪽 위로 → 위에서 왼쪽으로 → 아래로 (화면 좌표는 y가 아래로)
            let v = ease((u - TAKEOFF) / (1.0 - 2.0 * TAKEOFF));
            self.ellipse(FRAC_PI_2 - TAU * v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tilted_sleigh_always_fits_its_window() {
        for unit in [64.0, 128.0, 200.0] {
            let b = window_box(unit);
            let (w, h) = (RIDE_BOX.0 * unit, (RIDE_BOX.1 + BUBBLE) * unit);
            for deg in (-35..=35).map(f64::from) {
                let (sin, cos) = deg.to_radians().sin_cos();
                // 썰매 아래 가운데를 축으로 돈 네 모서리 (y는 위로 +)
                for (x, y) in [(-w / 2.0, 0.0), (w / 2.0, 0.0), (-w / 2.0, h), (w / 2.0, h)] {
                    let (rx, ry) = (x * cos - y * sin, x * sin + y * cos);
                    assert!(rx.abs() <= b.width / 2.0, "{unit} {deg}: x {rx} > {}", b.width / 2.0);
                    assert!(ry >= -b.below && ry <= b.height - b.below, "{unit} {deg}: y {ry} outside -{}..{}", b.below, b.height - b.below);
                }
            }
        }
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 1920.0, 1040.0)
    }

    fn fly(start: (f64, f64)) -> Vec<Pose> {
        let mut lap = Lap::new(start, area());
        let mut poses = vec![lap.pose()];
        while !lap.tick(1.0 / 60.0) {
            poses.push(lap.pose());
        }
        poses.push(lap.pose());
        poses
    }

    #[test]
    fn the_sleigh_takes_off_and_lands_where_they_met() {
        let poses = fly((300.0, 1040.0));
        let (first, last) = (poses[0], poses[poses.len() - 1]);
        assert!((first.x - 300.0).abs() < 1e-6 && (first.y - 1040.0).abs() < 1e-6);
        assert!((last.x - 300.0).abs() < 1e-6 && (last.y - 1040.0).abs() < 1e-6);
        assert!((poses.len() as f64 / 60.0 - RIDE_SECS).abs() < 0.1);
    }

    #[test]
    fn it_goes_once_around_the_screen_and_stays_on_it() {
        let poses = fly((1500.0, 1040.0));
        let a = area();
        let (minx, maxx) = poses.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        let miny = poses.iter().map(|p| p.y).fold(f64::MAX, f64::min);
        assert!(minx < a.width() * 0.15 && maxx > a.width() * 0.85, "{minx} {maxx}");
        assert!(miny < a.height() * 0.25, "{miny}");
        assert!(poses.iter().all(|p| p.x >= a.left && p.x <= a.right && p.y >= a.top && p.y <= a.bottom));
        // 한 바퀴: 오른쪽으로도 왼쪽으로도 날았고, 앞이 위로도 아래로도 들렸다
        assert!(poses.iter().any(|p| p.facing > 0.0) && poses.iter().any(|p| p.facing < 0.0));
        assert!(poses.iter().any(|p| p.tilt > 10.0) && poses.iter().any(|p| p.tilt < -10.0));
        assert!(poses.iter().all(|p| p.tilt.abs() <= 35.0));
    }

    #[test]
    fn they_meet_when_almost_touching_on_the_same_floor() {
        assert!(met((500.0, 1040.0, 128.0), (580.0, 1040.0, 128.0)));
        assert!(!met((500.0, 1040.0, 128.0), (700.0, 1040.0, 128.0)));
        assert!(!met((500.0, 1040.0, 128.0), (520.0, 700.0, 128.0)), "one is on a window above");
    }

    #[test]
    fn only_a_free_standing_pet_boards() {
        use crate::test_util::{pet_cfg, run_until};
        use crate::world::World;
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0);
        let mut p = Pet::new(pet_cfg(), 800.0, 1040.0, 7);
        assert!(run_until(&mut p, &w, 600, ready), "a pet walking on the floor can board");
        p.set_control(true);
        assert!(!ready(&p), "not while the user steers it");
    }

    #[test]
    fn a_bad_tick_does_not_break_the_lap() {
        let mut lap = Lap::new((100.0, 1000.0), area());
        assert!(!lap.tick(f64::NAN));
        assert!(!lap.tick(-1.0));
        assert!(lap.tick(1e9));
        let p = lap.pose();
        assert!(p.x.is_finite() && p.y.is_finite());
    }
}
