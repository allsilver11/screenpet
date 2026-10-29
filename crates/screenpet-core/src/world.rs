use crate::geom::{Rect, WindowInfo};
use crate::surfaces::{compute_surfaces, Owner, Surface};

pub struct World {
    pub windows: Vec<WindowInfo>,
    pub work_areas: Vec<Rect>,
    pub bounds: Rect,
    pub surfaces: Vec<Surface>,
    /// 펫 스프라이트 높이 (디딤판·옆면 판정용)
    pub pet_height: f64,
}

/// 창의 어느 옆면인지
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// 던져진 펫이 창 옆면에 부딪힘
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SideHit {
    pub window: u64,
    /// 옆면의 x
    pub x: f64,
    pub side: Side,
}

impl World {
    pub fn new(windows: Vec<WindowInfo>, mut work_areas: Vec<Rect>, pet_height: f64) -> Self {
        if work_areas.is_empty() {
            work_areas.push(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        }
        let bounds = work_areas.iter().copied().reduce(|a, b| a.union(&b)).unwrap();
        let surfaces = compute_surfaces(&windows, &work_areas, pet_height);
        Self { windows, work_areas, bounds, surfaces, pet_height }
    }

    /// x가 속한 작업 영역 (없으면 가장 가까운 것)
    pub fn work_area_at(&self, x: f64, y: f64) -> Rect {
        // 위아래로 쌓인 모니터: 발(x, y)이 들어 있는 작업 영역을 먼저 찾는다
        if let Some(r) = self.work_areas.iter().find(|r| r.left <= x && x < r.right && r.top < y && y <= r.bottom) {
            return *r;
        }
        let dist = |r: &Rect| if x < r.left { r.left - x } else if x > r.right { x - r.right } else { 0.0 };
        *self.work_areas.iter().min_by(|a, b| dist(a).total_cmp(&dist(b))).expect("at least one work area")
    }

    /// 창 옆면 x의 높이 y 지점이 앞쪽 창에 가려졌는지 (창이 없으면 None)
    pub fn side_covered(&self, window: u64, x: f64, y: f64) -> Option<bool> {
        let i = self.windows.iter().position(|w| w.id == window)?;
        let r = |w: &WindowInfo| w.rect.left <= x && x < w.rect.right && w.rect.top <= y && y < w.rect.bottom;
        Some(self.windows[..i].iter().any(r))
    }

    /// 이번 틱에 몸 앞쪽 끝(x ± half_w)이 바깥에서 창 옆면을 넘어섰는지. 발(y)이 창 윗면 + 몸 1/4보다 아래,
    /// 창 아래끝보다 위여야 하고, 그 지점이 앞쪽 창에 가려지지 않아야 한다. 여럿이면 가장 가까운 것.
    pub fn side_hit(&self, x0: f64, x1: f64, y: f64, half_w: f64) -> Option<SideHit> {
        let (lead0, lead1, side) = if x1 > x0 {
            (x0 + half_w, x1 + half_w, Side::Left)
        } else if x1 < x0 {
            (x0 - half_w, x1 - half_w, Side::Right)
        } else {
            return None;
        };
        self.windows
            .iter()
            .filter(|w| y > w.rect.top + 0.25 * self.pet_height && y < w.rect.bottom)
            .filter_map(|w| {
                let x = match side {
                    Side::Left if lead0 <= w.rect.left && lead1 > w.rect.left => w.rect.left,
                    Side::Right if lead0 >= w.rect.right && lead1 < w.rect.right => w.rect.right,
                    _ => return None,
                };
                (self.side_covered(w.id, x, y) == Some(false)).then_some(SideHit { window: w.id, x, side })
            })
            .min_by(|a, b| (a.x - lead0).abs().total_cmp(&(b.x - lead0).abs()))
    }

    pub fn anchor(&self, owner: Owner) -> Option<(f64, f64)> {
        match owner {
            Owner::Window(id) => self.windows.iter().find(|w| w.id == id).map(|w| (w.rect.left, w.rect.top)),
            Owner::Ground(i) => self.work_areas.get(i).map(|r| (r.left, r.bottom)),
        }
    }

    pub fn piece_at(&self, owner: Owner, x: f64) -> Option<&Surface> {
        self.surfaces.iter().find(|s| s.owner == owner && s.contains_x(x))
    }

    /// x 아래에 있는 Ground 디딤판 중 y가 주어진 y 이하인 것 (작업 영역 바닥보다 아래로 내려간 경우용)
    pub fn ground_below(&self, x: f64, y: f64) -> Option<&Surface> {
        self.surfaces
            .iter()
            .filter(|s| matches!(s.owner, Owner::Ground(_)) && s.contains_x(x) && s.y <= y)
            .max_by(|a, b| a.y.total_cmp(&b.y))
    }

    pub fn landing(&self, x: f64, prev_y: f64, new_y: f64) -> Option<&Surface> {
        if new_y <= prev_y {
            return None;
        }
        self.surfaces
            .iter()
            .filter(|s| s.contains_x(x) && prev_y <= s.y && new_y >= s.y)
            .min_by(|a, b| a.y.total_cmp(&b.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        let w1 = WindowInfo { id: 1, rect: Rect::new(500.0, 400.0, 1100.0, 900.0) };
        let w2 = WindowInfo { id: 2, rect: Rect::new(600.0, 700.0, 1000.0, 1000.0) };
        World::new(vec![w1, w2], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    #[test]
    fn empty_work_areas_fall_back_to_default_screen() {
        let w = World::new(vec![], vec![], 128.0);
        assert_eq!(w.bounds, Rect::new(0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn anchors() {
        let w = world();
        assert_eq!(w.anchor(Owner::Window(1)), Some((500.0, 400.0)));
        assert_eq!(w.anchor(Owner::Ground(0)), Some((0.0, 1040.0)));
        assert_eq!(w.anchor(Owner::Window(99)), None);
    }

    #[test]
    fn piece_at_finds_piece_under_x() {
        let w = world();
        assert!(w.piece_at(Owner::Window(1), 800.0).is_some());
        assert!(w.piece_at(Owner::Window(1), 1200.0).is_none());
    }

    #[test]
    fn landing_picks_first_crossed_surface() {
        let w = world();
        let s = w.landing(800.0, 300.0, 1100.0).unwrap();
        assert_eq!(s.owner, Owner::Window(1));
    }

    #[test]
    fn landing_ignores_surfaces_when_rising_or_not_crossed() {
        let w = world();
        assert!(w.landing(800.0, 500.0, 300.0).is_none());
        assert!(w.landing(800.0, 300.0, 390.0).is_none());
    }

    #[test]
    fn landing_requires_x_inside() {
        let w = world();
        let s = w.landing(200.0, 300.0, 1100.0).unwrap();
        assert_eq!(s.owner, Owner::Ground(0));
    }

    fn hit_world(windows: Vec<WindowInfo>) -> World {
        World::new(windows, vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)], 128.0)
    }

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    #[test]
    fn side_hit_only_when_crossing_from_outside() {
        let w = hit_world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
        // 오른쪽으로 날아가며 왼쪽 옆면(500)을 넘음
        assert_eq!(w.side_hit(450.0, 470.0, 600.0, 38.0), Some(SideHit { window: 1, x: 500.0, side: Side::Left }));
        // 왼쪽으로 날아가며 오른쪽 옆면(1100)을 넘음
        assert_eq!(w.side_hit(1150.0, 1130.0, 600.0, 38.0), Some(SideHit { window: 1, x: 1100.0, side: Side::Right }));
        // 아직 닿지 않음 / 창 안쪽에서 바깥으로 / 멀어지는 중
        assert_eq!(w.side_hit(400.0, 420.0, 600.0, 38.0), None);
        assert_eq!(w.side_hit(1050.0, 1070.0, 600.0, 38.0), None);
        assert_eq!(w.side_hit(470.0, 450.0, 600.0, 38.0), None);
        assert_eq!(w.side_hit(470.0, 470.0, 600.0, 38.0), None);
    }

    #[test]
    fn side_hit_needs_feet_between_upper_quarter_and_bottom() {
        let w = hit_world(vec![win(1, 500.0, 400.0, 1100.0, 900.0)]);
        // 윗면 + 32 = 432 초과, 900 미만
        assert_eq!(w.side_hit(450.0, 470.0, 432.0, 38.0), None);
        assert!(w.side_hit(450.0, 470.0, 433.0, 38.0).is_some());
        assert!(w.side_hit(450.0, 470.0, 899.0, 38.0).is_some());
        assert_eq!(w.side_hit(450.0, 470.0, 900.0, 38.0), None);
        assert_eq!(w.side_hit(450.0, 470.0, 300.0, 38.0), None);
    }

    #[test]
    fn side_hit_ignores_points_covered_by_front_windows() {
        let front = win(2, 400.0, 500.0, 700.0, 700.0);
        let w = hit_world(vec![front, win(1, 500.0, 400.0, 1100.0, 900.0)]);
        assert_eq!(w.side_hit(450.0, 470.0, 600.0, 38.0).map(|h| h.window), None);
        assert!(w.side_hit(450.0, 470.0, 800.0, 38.0).is_some());
        assert_eq!(w.side_covered(1, 500.0, 600.0), Some(true));
        assert_eq!(w.side_covered(1, 500.0, 800.0), Some(false));
        assert_eq!(w.side_covered(9, 500.0, 800.0), None);
        // 뒤쪽 창은 가리지 않는다
        let w = hit_world(vec![win(1, 500.0, 400.0, 1100.0, 900.0), front]);
        assert!(w.side_hit(450.0, 470.0, 600.0, 38.0).is_some());
    }

    #[test]
    fn side_hit_picks_the_nearest_crossing() {
        let w = hit_world(vec![win(1, 520.0, 400.0, 900.0, 900.0), win(2, 505.0, 300.0, 800.0, 950.0)]);
        // 앞쪽 끝 448 → 548: 505와 520 둘 다 넘음, 505가 먼저
        assert_eq!(w.side_hit(410.0, 510.0, 600.0, 38.0).map(|h| h.window), Some(2));
    }

    #[test]
    fn work_area_at_picks_the_monitor_under_x() {
        let w = World::new(vec![], vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 100.0, 3840.0, 1300.0)], 128.0);
        assert_eq!(w.work_area_at(100.0, 1040.0).bottom, 1040.0);
        assert_eq!(w.work_area_at(2000.0, 1300.0).bottom, 1300.0);
        assert_eq!(w.work_area_at(5000.0, 1300.0).bottom, 1300.0);
    }

    #[test]
    fn work_area_at_picks_the_stacked_monitor_under_the_feet() {
        let top = Rect::new(0.0, -1080.0, 1920.0, -40.0);
        let bottom = Rect::new(0.0, 0.0, 1920.0, 1040.0);
        let w = World::new(vec![], vec![bottom, top], 128.0);
        assert_eq!(w.work_area_at(800.0, -40.0), top);
        assert_eq!(w.work_area_at(800.0, -500.0), top);
        assert_eq!(w.work_area_at(800.0, 1040.0), bottom);
        assert_eq!(w.work_area_at(800.0, 300.0), bottom);
        // 어느 영역에도 발이 없으면 x만으로 고른다
        assert!([top, bottom].contains(&w.work_area_at(800.0, 5000.0)));
    }

    #[test]
    fn ground_below_finds_ground_under_x_at_or_below_y() {
        let w = world();
        let g = w.ground_below(800.0, 1090.0).unwrap();
        assert_eq!(g.owner, Owner::Ground(0));
        assert!(w.ground_below(800.0, 1000.0).is_none());
        assert!(w.ground_below(5000.0, 1090.0).is_none());
    }
}
