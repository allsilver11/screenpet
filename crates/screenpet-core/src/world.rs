use crate::geom::{Rect, WindowInfo};
use crate::surfaces::{compute_surfaces, Owner, Surface};

pub struct World {
    pub windows: Vec<WindowInfo>,
    pub work_areas: Vec<Rect>,
    pub bounds: Rect,
    pub surfaces: Vec<Surface>,
}

impl World {
    pub fn new(windows: Vec<WindowInfo>, mut work_areas: Vec<Rect>, pet_height: f64) -> Self {
        if work_areas.is_empty() {
            work_areas.push(Rect::new(0.0, 0.0, 1920.0, 1080.0));
        }
        let bounds = work_areas.iter().copied().reduce(|a, b| a.union(&b)).unwrap();
        let surfaces = compute_surfaces(&windows, &work_areas, bounds.top + pet_height);
        Self { windows, work_areas, bounds, surfaces }
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
}
