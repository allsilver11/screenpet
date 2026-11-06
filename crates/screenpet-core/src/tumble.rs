//! 물건이 구르는 모양 (순수 계산): 그림 모양과 종류별 무게.
//! 그림은 제 모양 그대로 무게중심을 축으로 돈다 — 동물의 구르기처럼 공이 되지 않는다.
//! 볼록 껍질로 "이 각도에서 어느 점이 바닥에 닿나"를 알아서, 어떤 각도로 굴러도 그림이 바닥에 정확히 닿고, 납작한 면으로 자연스럽게 눕는다.

use crate::object::{ObjectKind, OBJECT_LOGICAL_SIZE};

/// 그림 칸 한 변 (tools/draw_objects.py)
const CELL: f64 = 128.0;
/// 칸 좌표 → 논리 px
const CELL_TO_LOGICAL: f64 = OBJECT_LOGICAL_SIZE / CELL;
/// 창 가장자리에 남기는 여유 (논리 px)
const WINDOW_MARGIN: f64 = 2.0;
const SCALED_WINDOW_SLACK: f64 = 8.0;
const MIN_WINDOW_SIDE: f64 = 56.0;

/// 이보다 느리게 놓으면(논리 px/s) 돌지 않는다 — 살며시 내려놓기
pub const GENTLE_SPEED: f64 = 220.0;
/// 땅에서 이보다 느려지면(도/s) 가까운 면으로 눕는다
pub const SETTLE_SPIN: f64 = 90.0;
/// 눕는 동안 그림을 끌어내리는 힘 (도/s²)
pub const FLOP: f64 = 2000.0;
/// 면이 바닥에 부딪혀 되튀는 비율
pub const REBOUND: f64 = 0.3;
/// 이보다 느리게 부딪히면(도/s) 멈춘다
pub const STOP_SPIN: f64 = 40.0;
/// 이보다 느리면(도/s) 면에 닿아 있을 때 그냥 멈춘 것으로 친다
pub const TINY_SPIN: f64 = 5.0;
/// 구르는 동안 모서리를 넘느라 느려지고 넘어가며 빨라지는 정도 (도/s² ÷ 논리 px/라디안)
pub const CLUNK: f64 = 30.0;
/// 바람은 던지기보다 마찰을 이만큼만 받는다 — 천천히 굴러간다
pub const GUST_EASE: f64 = 0.3;
/// 벽에 부딪히면 반대로 돌며 남는 회전
pub const WALL_SPIN_KEEP: f64 = 0.6;
/// 살며시 내려앉을 때 받는 까딱임 (도/s, 떨어지는 논리 px/s당, 최대)
const WOBBLE_PER_SPEED: f64 = 0.25;
const WOBBLE_MAX: f64 = 110.0;

/// 그림 모양 (128 칸 좌표). `python tools/draw_objects.py --shapes`가 그림에서 찍은 표다
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    /// 무게중심 = 도는 축
    pub pivot: (f64, f64),
    /// 축에서 그림(과 포스트잇) 끝까지의 거리
    pub reach: f64,
    pub hull: &'static [(f64, f64)],
}

impl Shape {
    /// angle(도, 시계 방향)일 때 축에서 가장 낮은 점까지 (논리 px) — 바닥에 닿은 점 위로 축이 이만큼 떠 있다
    pub fn drop_at(&self, angle: f64) -> f64 {
        let (sin, cos) = angle.to_radians().sin_cos();
        let lowest = self.hull.iter().map(|&(x, y)| (x - self.pivot.0) * sin + (y - self.pivot.1) * cos).fold(f64::MIN, f64::max);
        lowest * CELL_TO_LOGICAL
    }

    /// 돌 때 축 높이가 변하는 기울기 (논리 px/라디안). 양수면 시계 방향으로 돌수록 축이 올라간다
    pub fn slope_at(&self, angle: f64) -> f64 {
        (self.drop_at(angle + 1.0) - self.drop_at(angle - 1.0)) / 2f64.to_radians()
    }

    /// angle에서 내리막으로 굴러 닿는 가장 가까운 안정된 각도 (같은 바퀴 수를 유지한다)
    pub fn rest_angle(&self, angle: f64) -> f64 {
        let mut at = angle;
        for step in [1.0, 0.1] {
            let dir = if self.drop_at(at + step) < self.drop_at(at) {
                step
            } else if self.drop_at(at - step) < self.drop_at(at) {
                -step
            } else {
                continue;
            };
            for _ in 0..720 {
                if self.drop_at(at + dir) >= self.drop_at(at) {
                    break;
                }
                at += dir;
            }
        }
        at
    }

    /// 창 한 변 (논리 px): 어떤 각도로 돌아도 축 둘레로 그림이 잘리지 않는다
    pub fn window_side(&self) -> f64 {
        (2.0 * self.reach * CELL_TO_LOGICAL + 2.0 * WINDOW_MARGIN).ceil()
    }

    /// scale배로 그릴 때의 창 한 변 (논리 px): 픽셀 그림은 칸이 정수라 조금 클 수 있어 여유를 둔다.
    /// 지금까지 쓰던 가장 작은 창보다 줄이지 않는다
    pub fn window_side_at(&self, scale: f64) -> f64 {
        if scale >= 1.0 {
            return self.window_side();
        }
        ((2.0 * self.reach * CELL_TO_LOGICAL * scale + 2.0 * WINDOW_MARGIN).ceil() + SCALED_WINDOW_SLACK).max(MIN_WINDOW_SIDE).min(self.window_side())
    }

    /// 그림 칸(96 논리 px) 안의 축 자리 (논리 px)
    pub fn pivot_logical(&self) -> (f64, f64) {
        (self.pivot.0 * CELL_TO_LOGICAL, self.pivot.1 * CELL_TO_LOGICAL)
    }

    /// 칸 가운데에서 축까지 가로 거리 (논리 px)
    pub fn pivot_offset_x(&self) -> f64 {
        (self.pivot.0 - CELL / 2.0) * CELL_TO_LOGICAL
    }
}

pub fn shape_of(kind: ObjectKind) -> Shape {
    match kind {
        ObjectKind::Can => Shape { pivot: (64.0, 93.0), reach: 40.0, hull: &[(40.0, 66.0), (43.0, 61.0), (54.0, 58.0), (74.0, 58.0), (85.0, 61.0), (88.0, 66.0), (88.0, 119.0), (85.0, 126.0), (80.0, 128.0), (48.0, 128.0), (41.0, 123.0), (40.0, 116.0)] },
        ObjectKind::Paper => Shape { pivot: (66.0, 90.0), reach: 62.0, hull: &[(14.0, 87.0), (22.0, 54.0), (61.0, 48.0), (110.0, 48.0), (118.0, 85.0), (118.0, 92.0), (114.0, 123.0), (93.0, 128.0), (32.0, 128.0), (18.0, 122.0), (16.0, 121.0), (14.0, 93.0)] },
        ObjectKind::Stone => Shape { pivot: (65.0, 95.0), reach: 44.0, hull: &[(29.0, 85.0), (40.0, 66.0), (56.0, 58.0), (85.0, 62.0), (100.0, 77.0), (104.0, 93.0), (104.0, 106.0), (95.0, 122.0), (77.0, 128.0), (54.0, 128.0), (39.0, 125.0), (26.0, 111.0)] },
        ObjectKind::Glasses => Shape { pivot: (64.0, 105.0), reach: 68.0, hull: &[(5.0, 74.0), (10.0, 73.0), (92.0, 85.0), (111.0, 89.0), (122.0, 97.0), (125.0, 106.0), (124.0, 114.0), (97.0, 127.0), (93.0, 128.0), (35.0, 128.0), (22.0, 120.0), (7.0, 87.0)] },
        ObjectKind::Computer => Shape { pivot: (65.0, 80.0), reach: 70.0, hull: &[(15.0, 48.0), (22.0, 41.0), (105.0, 26.0), (116.0, 36.0), (118.0, 51.0), (113.0, 108.0), (92.0, 126.0), (84.0, 128.0), (44.0, 128.0), (36.0, 126.0), (18.0, 112.0), (15.0, 107.0)] },
        ObjectKind::Milk => Shape { pivot: (64.0, 92.0), reach: 46.0, hull: &[(38.0, 92.0), (39.0, 80.0), (46.0, 52.0), (82.0, 52.0), (88.0, 76.0), (91.0, 111.0), (91.0, 115.0), (89.0, 127.0), (88.0, 128.0), (41.0, 128.0), (39.0, 126.0), (38.0, 115.0)] },
        ObjectKind::Shirt => Shape { pivot: (67.0, 93.0), reach: 60.0, hull: &[(15.0, 79.0), (34.0, 62.0), (81.0, 59.0), (99.0, 64.0), (124.0, 84.0), (120.0, 102.0), (105.0, 122.0), (99.0, 127.0), (86.0, 128.0), (35.0, 128.0), (28.0, 123.0), (11.0, 99.0)] },
        ObjectKind::Umbrella => Shape { pivot: (66.0, 91.0), reach: 60.0, hull: &[(23.0, 86.0), (43.0, 43.0), (49.0, 35.0), (56.0, 35.0), (59.0, 37.0), (112.0, 85.0), (115.0, 115.0), (110.0, 117.0), (67.0, 127.0), (61.0, 127.0), (38.0, 111.0), (23.0, 93.0)] },
        ObjectKind::Poop => Shape { pivot: (64.0, 98.0), reach: 46.0, hull: &[(24.0, 103.0), (36.0, 79.0), (70.0, 55.0), (78.0, 57.0), (92.0, 79.0), (104.0, 103.0), (105.0, 111.0), (98.0, 121.0), (81.0, 126.0), (47.0, 126.0), (30.0, 121.0), (24.0, 114.0)] },
        ObjectKind::Pink => Shape { pivot: (64.0, 98.0), reach: 46.0, hull: &[(24.0, 103.0), (36.0, 79.0), (70.0, 55.0), (78.0, 57.0), (92.0, 79.0), (104.0, 103.0), (105.0, 111.0), (98.0, 121.0), (81.0, 126.0), (47.0, 126.0), (30.0, 121.0), (24.0, 114.0)] },
        ObjectKind::Rainbow => Shape { pivot: (64.0, 98.0), reach: 46.0, hull: &[(24.0, 103.0), (36.0, 79.0), (70.0, 55.0), (78.0, 57.0), (92.0, 79.0), (104.0, 103.0), (105.0, 111.0), (98.0, 121.0), (81.0, 126.0), (47.0, 126.0), (30.0, 121.0), (24.0, 114.0)] },
        ObjectKind::Pellets => Shape { pivot: (64.0, 116.0), reach: 48.0, hull: &[(19.0, 116.0), (21.0, 113.0), (39.0, 102.0), (45.0, 102.0), (105.0, 104.0), (109.0, 107.0), (110.0, 109.0), (110.0, 115.0), (107.0, 119.0), (91.0, 128.0), (22.0, 128.0), (19.0, 125.0)] },
    }
}

/// 종류별 무게 — 던졌을 때 다르게 구르는 이유
#[derive(Debug, Clone, Copy)]
pub struct Heft {
    /// 던진 속도 → 회전 (도/s per 논리 px/s)
    pub air_spin: f64,
    /// 회전 상한 (도/s)
    pub max_spin: f64,
    /// 땅에서 받는 마찰 (논리 px/s²) — 작을수록 멀리 간다
    pub friction: f64,
    /// 구를 때의 반지름 (논리 px) — 클수록 같은 거리에 덜 돈다
    pub roll_radius: f64,
    /// 땅이 회전을 구르는 빠르기로 맞추는 힘 (1/s)
    pub grip: f64,
    /// 튀어 오를 때 남는 세로 속도
    pub bounce: f64,
    /// 튀어 오를 때 남는 가로 속도
    pub bounce_vx: f64,
    /// 튀어 오를 때 남는 회전 (나머지는 구르는 빠르기로)
    pub bounce_spin: f64,
    /// 튈 때마다 제멋대로 더해지는 회전 (도/s)
    pub jitter: f64,
}

pub fn heft_of(kind: ObjectKind) -> Heft {
    let h = |air_spin, max_spin, friction, roll_radius, grip, bounce, bounce_vx, bounce_spin, jitter| Heft { air_spin, max_spin, friction, roll_radius, grip, bounce, bounce_vx, bounce_spin, jitter };
    match kind {
        // 캔: 동그랗고 가벼워 멀리 굴러간다
        ObjectKind::Can => h(0.55, 900.0, 240.0, 18.0, 10.0, 0.4, 0.85, 0.8, 0.0),
        // 구겨진 신문지: 가볍게 팔랑이며 제멋대로
        ObjectKind::Paper => h(0.7, 1000.0, 420.0, 20.0, 6.0, 0.55, 0.8, 0.8, 320.0),
        // 돌멩이: 묵직해서 거의 안 튀고, 데굴 구르다 금방 멈춘다
        ObjectKind::Stone => h(0.3, 420.0, 1600.0, 24.0, 16.0, 0.08, 0.45, 0.5, 0.0),
        ObjectKind::Glasses => h(0.45, 720.0, 850.0, 30.0, 8.0, 0.3, 0.7, 0.6, 60.0),
        // 컴퓨터: 무거워서 거의 안 구른다
        ObjectKind::Computer => h(0.05, 70.0, 2000.0, 70.0, 14.0, 0.12, 0.45, 0.4, 0.0),
        ObjectKind::Milk => h(0.5, 720.0, 520.0, 24.0, 8.0, 0.35, 0.75, 0.7, 30.0),
        // 티셔츠: 흐물흐물 — 천천히 돌다 철퍼덕 떨어져 거의 안 튀고 안 구른다
        ObjectKind::Shirt => h(0.25, 380.0, 1500.0, 60.0, 18.0, 0.05, 0.35, 0.15, 25.0),
        ObjectKind::Umbrella => h(0.4, 600.0, 950.0, 34.0, 10.0, 0.2, 0.65, 0.55, 40.0),
        // 똥: 철퍽 붙어 거의 안 튀고, 무겁게 데굴 구르다 멈춘다
        ObjectKind::Poop | ObjectKind::Pink | ObjectKind::Rainbow | ObjectKind::Pellets => h(0.3, 420.0, 900.0, 18.0, 16.0, 0.04, 0.5, 0.2, 15.0),
    }
}

impl Heft {
    /// 구르는 빠르기 (도/s). v = 가로 속도 (논리 px/s)
    pub fn rolling_spin(&self, v: f64) -> f64 {
        (v / self.roll_radius).to_degrees()
    }
}

/// 던진 속도(논리 px/s) → 회전 (도/s, 시계 방향이 +). 방향은 가로로 가는 쪽, 거의 수직이면 dir_if_vertical(±1). 살며시 놓으면 0
pub fn throw_spin(kind: ObjectKind, vx: f64, vy: f64, dir_if_vertical: f64) -> f64 {
    let heft = heft_of(kind);
    let speed = vx.hypot(vy);
    if speed <= GENTLE_SPEED {
        return 0.0;
    }
    let dir = if vx.abs() < 30.0 { dir_if_vertical } else { vx.signum() };
    dir * (heft.air_spin * speed).min(heft.max_spin)
}

/// 살며시 내려앉을 때의 까딱임 (도/s). vy = 떨어지는 속도 (논리 px/s)
pub fn wobble_spin(vy: f64) -> f64 {
    (vy.max(0.0) * WOBBLE_PER_SPEED).min(WOBBLE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spin_grows_with_speed_follows_direction_and_is_capped() {
        let slow = throw_spin(ObjectKind::Can, 400.0, 0.0, 1.0);
        let fast = throw_spin(ObjectKind::Can, 1000.0, 0.0, 1.0);
        assert!(slow > 0.0 && fast > slow, "{slow} {fast}");
        assert_eq!(throw_spin(ObjectKind::Can, -1000.0, 0.0, 1.0), -fast);
        // 거의 수직이면 주어진 쪽으로
        assert!(throw_spin(ObjectKind::Can, 0.0, -900.0, -1.0) < 0.0);
        for kind in ObjectKind::ALL {
            let top = throw_spin(kind, 1e5, 0.0, 1.0);
            assert_eq!(top, heft_of(kind).max_spin, "{kind:?}");
        }
    }

    #[test]
    fn a_gentle_release_does_not_spin() {
        for kind in ObjectKind::ALL {
            assert_eq!(throw_spin(kind, 100.0, 50.0, 1.0), 0.0, "{kind:?}");
            assert_eq!(throw_spin(kind, 0.0, 0.0, 1.0), 0.0, "{kind:?}");
        }
    }

    #[test]
    fn each_kind_has_its_own_heft() {
        let (can, paper, computer, sock) = (heft_of(ObjectKind::Can), heft_of(ObjectKind::Paper), heft_of(ObjectKind::Computer), heft_of(ObjectKind::Shirt));
        for kind in ObjectKind::ALL {
            let h = heft_of(kind);
            assert!(can.friction <= h.friction, "the can rolls farthest: {kind:?}");
            assert!(computer.max_spin <= h.max_spin && computer.friction >= h.friction, "the computer is heaviest: {kind:?}");
            assert!(paper.bounce >= h.bounce && paper.jitter >= h.jitter, "paper bounces most: {kind:?}");
        }
        // 돌멩이는 묵직하다: 안 튀고 금방 멈춘다
        let stone = heft_of(ObjectKind::Stone);
        assert!(stone.friction > can.friction * 4.0 && stone.bounce < can.bounce / 3.0 && stone.jitter == 0.0);
        // 티셔츠는 철퍼덕
        assert!(sock.roll_radius > can.roll_radius * 2.0 && sock.bounce < can.bounce / 2.0);
    }

    #[test]
    fn the_lowest_point_comes_from_the_hull() {
        // 90도 돌면 오른쪽 끝이 아래로 온다
        let s = Shape { pivot: (64.0, 64.0), reach: 50.0, hull: &[(44.0, 54.0), (104.0, 54.0), (104.0, 84.0), (44.0, 84.0)] };
        assert!((s.drop_at(0.0) - 20.0 * CELL_TO_LOGICAL).abs() < 1e-9);
        assert!((s.drop_at(90.0) - 40.0 * CELL_TO_LOGICAL).abs() < 1e-9);
        assert!((s.drop_at(-90.0) - 20.0 * CELL_TO_LOGICAL).abs() < 1e-9);
        assert!((s.drop_at(180.0) - 10.0 * CELL_TO_LOGICAL).abs() < 1e-9);
    }

    #[test]
    fn rest_angles_are_stable_and_found_downhill() {
        for kind in ObjectKind::ALL {
            let s = shape_of(kind);
            for start in (-720..720).step_by(13) {
                let start = start as f64;
                let rest = s.rest_angle(start);
                // 내리막이다: 시작보다 높지 않고, 멀어야 한 면 건너 (반 바퀴 안)
                assert!(s.drop_at(rest) <= s.drop_at(start) + 1e-9, "{kind:?} {start}");
                assert!((rest - start).abs() < 180.0, "{kind:?} {start} {rest}");
                // 안정: 양쪽이 더 높다
                assert!(s.drop_at(rest + 0.1) >= s.drop_at(rest) && s.drop_at(rest - 0.1) >= s.drop_at(rest), "{kind:?} {rest}");
                // 다시 찾아도 그대로 — 다 멈춘 각도에서 저절로 움직이지 않는다
                assert_eq!(s.rest_angle(rest), rest, "{kind:?}");
            }
        }
    }

    #[test]
    fn the_window_holds_the_sprite_at_every_angle() {
        for kind in ObjectKind::ALL {
            let s = shape_of(kind);
            let half = s.window_side() / 2.0;
            assert!(s.reach * CELL_TO_LOGICAL < half, "{kind:?}");
            for a in 0..360 {
                let d = s.drop_at(a as f64);
                assert!(d > 0.0 && d <= s.reach * CELL_TO_LOGICAL, "{kind:?} {a} {d}");
            }
            for &(x, y) in s.hull {
                assert!((x - s.pivot.0).hypot(y - s.pivot.1) <= s.reach, "{kind:?}");
            }
        }
    }

    #[test]
    fn wobble_is_small_and_capped() {
        assert_eq!(wobble_spin(-5.0), 0.0);
        assert!(wobble_spin(100.0) < wobble_spin(300.0));
        assert_eq!(wobble_spin(1e6), WOBBLE_MAX);
    }
}
