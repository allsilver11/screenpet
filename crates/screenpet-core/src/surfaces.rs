use crate::geom::{Rect, WindowInfo};

/// 디딤판이 누구 것인지. Ground의 값은 work_areas 인덱스.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    Window(u64),
    Ground(usize),
}

/// 디딤판 조각 끝이 왜 끝났는지.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    /// 창 모서리
    Edge,
    /// 앞쪽 창에 가려짐
    Blocked,
    /// 화면 끝
    Wall,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub owner: Owner,
    pub y: f64,
    pub x_start: f64,
    pub x_end: f64,
    pub start_kind: EndKind,
    pub end_kind: EndKind,
}

impl Surface {
    pub fn width(&self) -> f64 {
        self.x_end - self.x_start
    }

    pub fn contains_x(&self, x: f64) -> bool {
        x >= self.x_start && x <= self.x_end
    }
}

pub fn compute_surfaces(windows: &[WindowInfo], work_areas: &[Rect], min_y: f64) -> Vec<Surface> {
    let mut out = Vec::new();
    let Some(bounds) = work_areas.iter().copied().reduce(|a, b| a.union(&b)) else {
        return out;
    };

    for (i, w) in windows.iter().enumerate() {
        let y = w.rect.top;
        if y < min_y || y >= bounds.bottom {
            continue;
        }
        // (x_start, x_end, start_kind, end_kind)
        let mut segs = vec![clip(w.rect.left, w.rect.right, &bounds)];
        for front in &windows[..i] {
            let r = front.rect;
            if !(r.top <= y && r.bottom > y) {
                continue;
            }
            segs = segs
                .into_iter()
                .flat_map(|(a, b, ka, kb)| {
                    let mut kept = Vec::new();
                    if r.right <= a || r.left >= b {
                        kept.push((a, b, ka, kb));
                    } else {
                        if r.left > a {
                            kept.push((a, r.left, ka, EndKind::Blocked));
                        }
                        if r.right < b {
                            kept.push((r.right, b, EndKind::Blocked, kb));
                        }
                    }
                    kept
                })
                .collect();
        }
        for (a, b, ka, kb) in segs {
            if b > a {
                out.push(Surface { owner: Owner::Window(w.id), y, x_start: a, x_end: b, start_kind: ka, end_kind: kb });
            }
        }
    }

    for (i, wa) in work_areas.iter().enumerate() {
        out.push(Surface {
            owner: Owner::Ground(i),
            y: wa.bottom,
            x_start: wa.left,
            x_end: wa.right,
            start_kind: EndKind::Wall,
            end_kind: EndKind::Wall,
        });
    }
    out
}

fn clip(left: f64, right: f64, bounds: &Rect) -> (f64, f64, EndKind, EndKind) {
    let (a, ka) = if left < bounds.left { (bounds.left, EndKind::Wall) } else { (left, EndKind::Edge) };
    let (b, kb) = if right > bounds.right { (bounds.right, EndKind::Wall) } else { (right, EndKind::Edge) };
    (a, b, ka, kb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(id: u64, l: f64, t: f64, r: f64, b: f64) -> WindowInfo {
        WindowInfo { id, rect: Rect::new(l, t, r, b) }
    }

    fn screen() -> Vec<Rect> {
        vec![Rect::new(0.0, 0.0, 1920.0, 1040.0)]
    }

    fn pieces_of(s: &[Surface], owner: Owner) -> Vec<Surface> {
        s.iter().copied().filter(|p| p.owner == owner).collect()
    }

    #[test]
    fn ground_per_work_area_with_walls() {
        let s = compute_surfaces(&[], &screen(), 0.0);
        assert_eq!(s.len(), 1);
        let g = s[0];
        assert_eq!(g.owner, Owner::Ground(0));
        assert_eq!((g.y, g.x_start, g.x_end), (1040.0, 0.0, 1920.0));
        assert_eq!((g.start_kind, g.end_kind), (EndKind::Wall, EndKind::Wall));
    }

    #[test]
    fn single_window_top_has_edges() {
        let s = compute_surfaces(&[win(1, 500.0, 400.0, 1100.0, 900.0)], &screen(), 0.0);
        let p = pieces_of(&s, Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].y, p[0].x_start, p[0].x_end), (400.0, 500.0, 1100.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Edge, EndKind::Edge));
    }

    #[test]
    fn front_window_splits_back_window_top() {
        let ws = [win(2, 700.0, 300.0, 900.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].x_start, p[0].x_end), (500.0, 700.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Edge, EndKind::Blocked));
        assert_eq!((p[1].x_start, p[1].x_end), (900.0, 1100.0));
        assert_eq!((p[1].start_kind, p[1].end_kind), (EndKind::Blocked, EndKind::Edge));
    }

    #[test]
    fn front_window_covering_left_end() {
        let ws = [win(2, 400.0, 300.0, 600.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].x_start, p[0].x_end), (600.0, 1100.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Blocked, EndKind::Edge));
    }

    #[test]
    fn front_window_below_the_top_line_does_not_cover() {
        let ws = [win(2, 700.0, 450.0, 900.0, 600.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn fully_covered_window_has_no_pieces() {
        let ws = [win(2, 0.0, 0.0, 1920.0, 1040.0), win(1, 500.0, 400.0, 1100.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert!(p.is_empty());
    }

    #[test]
    fn back_window_does_not_cover_front_window() {
        let ws = [win(1, 500.0, 400.0, 1100.0, 900.0), win(2, 700.0, 300.0, 900.0, 600.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].x_start, p[0].x_end), (500.0, 1100.0));
    }

    #[test]
    fn tops_above_min_y_or_below_screen_are_ignored() {
        let ws = [win(1, 500.0, 50.0, 1100.0, 900.0), win(2, 500.0, 1040.0, 1100.0, 1200.0)];
        let s = compute_surfaces(&ws, &screen(), 128.0);
        assert!(pieces_of(&s, Owner::Window(1)).is_empty());
        assert!(pieces_of(&s, Owner::Window(2)).is_empty());
    }

    #[test]
    fn offscreen_part_is_clipped_as_wall() {
        let ws = [win(1, -300.0, 400.0, 600.0, 900.0)];
        let p = pieces_of(&compute_surfaces(&ws, &screen(), 0.0), Owner::Window(1));
        assert_eq!((p[0].x_start, p[0].x_end), (0.0, 600.0));
        assert_eq!((p[0].start_kind, p[0].end_kind), (EndKind::Wall, EndKind::Edge));
    }
}
