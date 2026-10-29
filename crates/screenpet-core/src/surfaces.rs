use crate::geom::{Rect, WindowInfo};
use crate::inside;

/// 디딤판이 누구 것인지. Ground의 값은 work_areas 인덱스.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Owner {
    /// 창 윗면 (밖)
    Window(u64),
    Ground(usize),
    /// 창 안쪽 바닥 (아래 테두리 바로 위, spec 2B §9) — 그 창 안에 있는 동물만 디딘다
    Inside(u64),
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
    /// 옆 모니터로 이어지는 바닥 경계. to = 이웃 작업 영역 인덱스
    Seam { to: usize },
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

/// 옆 모니터와 맞닿았다고 볼 x 오차 (px)
const SEAM_TOLERANCE: f64 = 2.0;

pub fn compute_surfaces(windows: &[WindowInfo], work_areas: &[Rect], pet_height: f64) -> Vec<Surface> {
    let mut out = Vec::new();
    let Some(bounds) = work_areas.iter().copied().reduce(|a, b| a.union(&b)) else {
        return out;
    };

    for (i, w) in windows.iter().enumerate() {
        let y = w.rect.top;
        if y >= bounds.bottom {
            continue;
        }
        // 창 윗면 중앙이 속한 작업 영역 기준으로, 펫이 머리를 둘 공간이 없으면 건너뜀
        let cx = (w.rect.left + w.rect.right) / 2.0;
        match work_areas.iter().find(|wa| cx >= wa.left && cx <= wa.right) {
            Some(wa) if y >= wa.top + pet_height => {}
            _ => continue,
        }
        // (x_start, x_end, start_kind, end_kind)
        let segs = cut_by_front(vec![clip(w.rect.left, w.rect.right, &bounds)], &windows[..i], y);
        for (a, b, ka, kb) in segs {
            if b > a {
                out.push(Surface { owner: Owner::Window(w.id), y, x_start: a, x_end: b, start_kind: ka, end_kind: kb });
            }
        }
    }

    // 창 안쪽 바닥: 아래 테두리에서 INSET 위, 옆 테두리에서 INSET 안쪽. 양 끝(안쪽 벽)은 Wall
    for (i, w) in windows.iter().enumerate() {
        if !inside::has_room(&w.rect, pet_height) {
            continue;
        }
        let y = inside::floor_y(&w.rect);
        if y > bounds.bottom || y - pet_height < bounds.top {
            continue;
        }
        let (a, b, _, _) = clip(w.rect.left + inside::INSET, w.rect.right - inside::INSET, &bounds);
        let segs = cut_by_front(vec![(a, b, EndKind::Wall, EndKind::Wall)], &windows[..i], y);
        for (a, b, ka, kb) in segs {
            if b > a {
                out.push(Surface { owner: Owner::Inside(w.id), y, x_start: a, x_end: b, start_kind: ka, end_kind: kb });
            }
        }
    }

    for (i, wa) in work_areas.iter().enumerate() {
        out.push(Surface {
            owner: Owner::Ground(i),
            y: wa.bottom,
            x_start: wa.left,
            x_end: wa.right,
            start_kind: seam_at(work_areas, i, wa.left, |o| o.right),
            end_kind: seam_at(work_areas, i, wa.right, |o| o.left),
        });
    }
    out
}

/// 바닥 끝 x에 옆으로 맞닿고(±SEAM_TOLERANCE) 세로로 겹치는 다른 작업 영역이 있으면 Seam, 없으면 Wall.
/// facing_edge = 이웃의 맞닿는 쪽 x
fn seam_at(work_areas: &[Rect], i: usize, x: f64, facing_edge: impl Fn(&Rect) -> f64) -> EndKind {
    let wa = work_areas[i];
    work_areas
        .iter()
        .enumerate()
        .find(|&(j, o)| j != i && (facing_edge(o) - x).abs() <= SEAM_TOLERANCE && o.top < wa.bottom && wa.top < o.bottom)
        .map_or(EndKind::Wall, |(to, _)| EndKind::Seam { to })
}

/// (x_start, x_end, start_kind, end_kind) 조각들에서 높이 y의 선을 가리는 앞쪽 창 부분을 잘라 낸다 (잘린 끝은 Blocked)
fn cut_by_front(mut segs: Vec<(f64, f64, EndKind, EndKind)>, fronts: &[WindowInfo], y: f64) -> Vec<(f64, f64, EndKind, EndKind)> {
    for front in fronts {
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
    segs
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
    fn side_by_side_grounds_meet_at_a_seam() {
        let areas = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1921.5, 200.0, 3840.0, 1300.0)];
        let s = compute_surfaces(&[], &areas, 0.0);
        let a = pieces_of(&s, Owner::Ground(0))[0];
        let b = pieces_of(&s, Owner::Ground(1))[0];
        assert_eq!((a.start_kind, a.end_kind), (EndKind::Wall, EndKind::Seam { to: 1 }));
        assert_eq!((b.start_kind, b.end_kind), (EndKind::Seam { to: 0 }, EndKind::Wall));
    }

    #[test]
    fn grounds_without_horizontal_contact_stay_walls() {
        // 위아래로 쌓인 모니터
        let stacked = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(0.0, -1080.0, 1920.0, -40.0)];
        // 3 px 넘게 떨어짐 / 옆이지만 세로로 안 겹침
        let gap = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1923.0, 0.0, 3840.0, 1040.0)];
        let diagonal = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 1080.0, 3840.0, 2120.0)];
        for areas in [stacked, gap, diagonal] {
            for g in compute_surfaces(&[], &areas, 0.0) {
                assert_eq!((g.start_kind, g.end_kind), (EndKind::Wall, EndKind::Wall), "{areas:?}");
            }
        }
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
    fn tops_too_high_or_below_screen_are_ignored() {
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

    #[test]
    fn min_y_is_per_work_area_of_the_window() {
        let areas = vec![Rect::new(0.0, 0.0, 1920.0, 1040.0), Rect::new(1920.0, 300.0, 3840.0, 1340.0)];
        let maxed_b = win(1, 1920.0, 300.0, 3840.0, 1340.0);
        let s = compute_surfaces(&[maxed_b], &areas, 128.0);
        assert!(pieces_of(&s, Owner::Window(1)).is_empty());
        let lower_b = win(2, 2200.0, 438.0, 3000.0, 1200.0);
        let s = compute_surfaces(&[lower_b], &areas, 128.0);
        assert_eq!(pieces_of(&s, Owner::Window(2)).len(), 1);
    }
}
