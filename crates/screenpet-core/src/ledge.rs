//! 🌐 웹 페이지 디딤돌(ledge) — 맨 앞 브라우저 창 안 글 덩어리·상자·그림·버튼의 윗변 (⚙️ 설정에서 켬, 실험, 기본 끔)
//! OS 쪽(platform/win32_web.rs)이 따로 도는 스레드에서 사각형만 읽어 LedgeSet으로 넘기고, World::set_ledges가 디딤판(Owner::Ledge)으로 만듦.
//! 사각형만 다룸 — 글자·이름·값은 읽지도 남기지도 친구에게 보내지도 않음. 디딤돌은 창이 아님 (가림·옆면·숨기·거미줄 등은 모름)
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::geom::{Rect, WindowInfo};

/// 브라우저로 보는 실행 파일 이름 (소문자, 경로 뺀 것)
pub const BROWSER_EXES: [&str; 7] = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "whale.exe", "opera.exe", "vivaldi.exe"];
/// 디딤돌 최소 너비 (물리 px, 보이는 부분)
pub const MIN_WIDTH: f64 = 60.0;
/// 디딤돌 최소 높이 (물리 px, 요소 전체)
pub const MIN_HEIGHT: f64 = 14.0;
/// 한 번에 쓰는 디딤돌 수 상한
pub const MAX_LEDGES: usize = 40;
/// 페이지 보이는 영역 위·아래 끝에서 이만큼 안쪽 윗변만 씀 (반쯤 스크롤돼 윗변이 안 보이는 요소는 뺌)
pub const VIEW_MARGIN: f64 = 4.0;
/// 윗변 높이 차가 이 안이고 가로로 반 넘게 겹치면 같은 디딤돌로 봄 (더 넓은 것만 남김)
pub const SAME_TOP: f64 = 6.0;
/// 읽기 간격 (한 번 읽기 시작 → 다음 읽기 시작)
pub const PROBE_EVERY: Duration = Duration::from_millis(500);
/// 이보다 오래된 읽기는 버림 (브라우저가 멈춰 읽기가 밀렸을 때 낡은 디딤돌을 쓰지 않음)
pub const STALE_AFTER: Duration = Duration::from_secs(2);

/// 한 번 읽은 디딤돌들: 그 브라우저 창(window), 읽을 때의 창 경계(frame), 요소 윗변 사각형들 (화면 물리 px)
#[derive(Debug, Clone, PartialEq)]
pub struct LedgeSet {
    /// 브라우저 창 id (WindowInfo.id)
    pub window: u64,
    /// 읽을 때의 창 경계 — 창이 옮겨졌으면 그만큼 같이 옮기고, 크기가 바뀌었으면 버림
    pub frame: Rect,
    /// 요소 사각형들 (pick_ledges로 고른 것)
    pub rects: Vec<Rect>,
}

/// 지금 세상에 놓인 디딤돌 하나 (창 이동을 반영한 화면 자리)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ledge {
    /// 디딤돌 id (창 안 상대 자리에서 만듦 — 그대로면 같은 id, 스크롤로 움직이면 새 id)
    pub id: u64,
    /// 그 브라우저 창 id
    pub window: u64,
    /// 화면 자리
    pub rect: Rect,
}

/// 실행 파일 경로(또는 이름)가 브라우저임 (대소문자 무시)
pub fn is_browser_exe(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or(path).to_ascii_lowercase();
    BROWSER_EXES.contains(&name.as_str())
}

/// UI Automation 컨트롤 종류 번호 중 디딤돌로 쓰는 것: 글·묶음·그림·버튼·링크·입력칸·목록·목록 항목·표·표 칸·사용자 정의·콤보.
/// 문서 자체(50030)·창·창틀·메뉴·탭·스크롤 막대 등 브라우저 껍데기는 뺌
pub fn keeps_control_type(id: i32) -> bool {
    const KEEP: [i32; 12] = [50020, 50026, 50006, 50000, 50005, 50004, 50008, 50007, 50036, 50029, 50025, 50003];
    KEEP.contains(&id)
}

/// 보이는 영역을 cols × rows 칸으로 나눈 칸 가운데 점들 (위 줄부터 왼쪽→오른쪽)
pub fn probe_grid(view: Rect, cols: usize, rows: usize) -> Vec<(f64, f64)> {
    let (cw, ch) = (view.width() / cols.max(1) as f64, view.height() / rows.max(1) as f64);
    (0..rows).flat_map(|r| (0..cols).map(move |c| (view.left + (c as f64 + 0.5) * cw, view.top + (r as f64 + 0.5) * ch))).collect()
}

/// 읽은 요소 사각형들에서 디딤돌을 고름: 윗변이 보이는 영역 안(VIEW_MARGIN 안쪽)이고, 높이 MIN_HEIGHT 이상,
/// 보이는 영역으로 가로를 자른 너비 MIN_WIDTH 이상. 윗변이 거의 같은 것끼리는 넓은 것만. 넓은 순 MAX_LEDGES개, 위→아래 순으로
pub fn pick_ledges(found: &[Rect], view: Rect) -> Vec<Rect> {
    let mut kept: Vec<Rect> = Vec::new();
    let mut cands: Vec<Rect> = found
        .iter()
        .filter(|r| r.height() >= MIN_HEIGHT && r.top >= view.top + VIEW_MARGIN && r.top <= view.bottom - VIEW_MARGIN)
        .map(|r| Rect::new(r.left.max(view.left), r.top, r.right.min(view.right), r.bottom.min(view.bottom)))
        .filter(|r| r.width() >= MIN_WIDTH)
        .collect();
    cands.sort_by(|a, b| b.width().total_cmp(&a.width()).then(a.top.total_cmp(&b.top)).then(a.left.total_cmp(&b.left)));
    for r in cands {
        let same = |k: &Rect| {
            let overlap = r.right.min(k.right) - r.left.max(k.left);
            (r.top - k.top).abs() <= SAME_TOP && overlap >= 0.5 * r.width().min(k.width())
        };
        if !kept.iter().any(same) {
            kept.push(r);
        }
        if kept.len() == MAX_LEDGES {
            break;
        }
    }
    kept.sort_by(|a, b| a.top.total_cmp(&b.top).then(a.left.total_cmp(&b.left)));
    kept
}

/// 디딤돌 id: 창 id와 창 왼쪽 위 기준 상대 자리(정수 px)로 만듦 — 창만 옮겨지면 그대로, 스크롤로 움직이면 바뀜
pub fn ledge_id(window: u64, frame: &Rect, rect: &Rect) -> u64 {
    let parts = [window as i64, (rect.left - frame.left).round() as i64, (rect.top - frame.top).round() as i64, (rect.right - frame.left).round() as i64];
    // FNV-1a
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for p in parts {
        for b in p.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// 디딤돌을 지금 창 자리에 놓음: 그 창이 목록에 없거나 크기가 바뀌었으면 없음, 옮겨졌으면 같이 옮김
pub fn place_ledges(windows: &[WindowInfo], set: &LedgeSet) -> Vec<Ledge> {
    let Some(w) = windows.iter().find(|w| w.id == set.window) else { return Vec::new() };
    if (w.rect.width() - set.frame.width()).abs() > 1.0 || (w.rect.height() - set.frame.height()).abs() > 1.0 {
        return Vec::new();
    }
    let (dx, dy) = (w.rect.left - set.frame.left, w.rect.top - set.frame.top);
    set.rects
        .iter()
        .map(|r| Ledge { id: ledge_id(set.window, &set.frame, r), window: set.window, rect: Rect::new(r.left + dx, r.top + dy, r.right + dx, r.bottom + dy) })
        .collect()
}

/// 읽기 스레드가 넘기는 마지막 결과와 그 시각
struct Latest {
    set: Option<LedgeSet>,
    at: Option<Instant>,
}

/// 🌐 디딤돌 읽기 스레드 손잡이 (게임 루프가 가짐). 버리면 스레드가 다음 쉼에서 끝남
pub struct LedgeProbe {
    latest: Arc<Mutex<Latest>>,
    stop: Arc<AtomicBool>,
}

impl LedgeProbe {
    /// 마지막으로 읽은 디딤돌 (STALE_AFTER보다 오래됐거나 아직 없으면 None). 게임 루프를 막지 않음 (잠금은 복사하는 동안만)
    pub fn latest(&self) -> Option<LedgeSet> {
        let l = self.latest.lock().ok()?;
        l.at.filter(|at| at.elapsed() <= STALE_AFTER).and(l.set.clone())
    }
}

impl Drop for LedgeProbe {
    /// 스레드에 그만하라고 알림 (기다리지 않음)
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// 디딤돌 읽기 스레드를 띄움: make를 그 스레드에서 한 번 불러 읽개를 만들고 (COM 등은 그 스레드 것), every마다 한 번 읽어 넘김.
/// 읽개를 못 만들면 스레드가 바로 끝남. 스레드를 못 띄우면 None
pub fn spawn_probe<M, F>(every: Duration, make: M) -> Option<LedgeProbe>
where
    M: FnOnce() -> Option<F> + Send + 'static,
    F: FnMut() -> Option<LedgeSet>,
{
    let latest = Arc::new(Mutex::new(Latest { set: None, at: None }));
    let stop = Arc::new(AtomicBool::new(false));
    let (out, quit) = (latest.clone(), stop.clone());
    std::thread::Builder::new()
        .name("web-ledges".into())
        .spawn(move || {
            let Some(mut probe) = make() else { return };
            while !quit.load(Ordering::Relaxed) {
                let start = Instant::now();
                let set = probe();
                if let Ok(mut l) = out.lock() {
                    *l = Latest { set, at: Some(Instant::now()) };
                }
                // 남은 시간을 짧게 나눠 쉬며 그만하라는 말을 봄
                while !quit.load(Ordering::Relaxed) && start.elapsed() < every {
                    std::thread::sleep((every.saturating_sub(start.elapsed())).min(Duration::from_millis(50)));
                }
            }
        })
        .ok()?;
    Some(LedgeProbe { latest, stop })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    /// 테스트: 페이지 보이는 영역
    fn view() -> Rect {
        Rect::new(100.0, 200.0, 1100.0, 900.0)
    }

    #[test]
    fn browser_exes_are_known_by_file_name_in_any_case() {
        assert!(is_browser_exe(r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
        assert!(is_browser_exe(r"C:\Program Files (x86)\Microsoft\Edge\Application\MSEDGE.EXE"));
        assert!(is_browser_exe("whale.exe"));
        assert!(is_browser_exe("C:/x/Firefox.exe"));
        assert!(!is_browser_exe(r"C:\Windows\notepad.exe"));
        assert!(!is_browser_exe(r"C:\x\chrome.exe.bak"));
        assert!(!is_browser_exe(""));
    }

    #[test]
    fn only_page_content_control_types_are_kept() {
        // 글·묶음·그림·버튼·링크·입력칸·목록 항목
        for id in [50020, 50026, 50006, 50000, 50005, 50004, 50007] {
            assert!(keeps_control_type(id), "{id}");
        }
        // 문서·창·창틀·탭·메뉴 막대·스크롤 막대·툴바
        for id in [50030, 50032, 50033, 50018, 50010, 50014, 50021] {
            assert!(!keeps_control_type(id), "{id}");
        }
    }

    #[test]
    fn probe_grid_puts_points_in_cell_centers() {
        let pts = probe_grid(Rect::new(0.0, 0.0, 100.0, 50.0), 2, 2);
        assert_eq!(pts, vec![(25.0, 12.5), (75.0, 12.5), (25.0, 37.5), (75.0, 37.5)]);
        assert_eq!(probe_grid(view(), 12, 10).len(), 120);
        assert!(probe_grid(view(), 12, 10).iter().all(|&(x, y)| view().contains(x, y)));
    }

    #[test]
    fn small_thin_or_hidden_top_boxes_are_dropped() {
        let found = [
            Rect::new(200.0, 300.0, 600.0, 330.0), // 글 덩어리 — 남음
            Rect::new(200.0, 400.0, 250.0, 430.0), // 너무 좁음
            Rect::new(200.0, 500.0, 600.0, 510.0), // 너무 얇음
            Rect::new(200.0, 150.0, 600.0, 450.0), // 윗변이 스크롤돼 안 보임
            Rect::new(200.0, 202.0, 600.0, 450.0), // 윗변이 보이는 영역 맨 위에 붙음
            Rect::new(200.0, 898.0, 600.0, 950.0), // 맨 아래에 붙음
            Rect::new(1080.0, 600.0, 1400.0, 640.0), // 보이는 부분은 20 px
        ];
        assert_eq!(pick_ledges(&found, view()), vec![Rect::new(200.0, 300.0, 600.0, 330.0)]);
    }

    #[test]
    fn boxes_are_clipped_to_the_page_view() {
        let found = [Rect::new(-50.0, 300.0, 1500.0, 1200.0)];
        assert_eq!(pick_ledges(&found, view()), vec![Rect::new(100.0, 300.0, 1100.0, 900.0)]);
    }

    #[test]
    fn near_identical_tops_keep_the_widest_and_far_ones_stay() {
        let found = [
            Rect::new(200.0, 300.0, 500.0, 330.0),
            Rect::new(180.0, 303.0, 700.0, 500.0), // 같은 윗변 + 많이 겹침 → 이것(더 넓음)만
            Rect::new(200.0, 300.0, 500.0, 330.0), // 똑같은 것 (다른 칸에서 또 찾음)
            Rect::new(800.0, 301.0, 1000.0, 330.0), // 같은 높이지만 따로 떨어짐 → 남음
            Rect::new(200.0, 340.0, 500.0, 370.0), // 다음 줄 → 남음
        ];
        let got = pick_ledges(&found, view());
        assert_eq!(got, vec![Rect::new(800.0, 301.0, 1000.0, 330.0), Rect::new(180.0, 303.0, 700.0, 500.0), Rect::new(200.0, 340.0, 500.0, 370.0)]);
    }

    #[test]
    fn at_most_max_ledges_widest_first_in_top_order() {
        let found: Vec<Rect> = (0..60).map(|i| Rect::new(200.0, 210.0 + 10.0 * i as f64, 300.0 + i as f64, 230.0 + 10.0 * i as f64)).collect();
        let got = pick_ledges(&found, Rect::new(100.0, 200.0, 1100.0, 900.0));
        assert_eq!(got.len(), MAX_LEDGES);
        assert!(got.windows(2).all(|p| p[0].top <= p[1].top));
        // 좁은 것(앞쪽)이 빠짐
        assert!(got.iter().all(|r| r.width() >= 100.0 + (60 - MAX_LEDGES) as f64));
    }

    #[test]
    fn placed_ledges_follow_a_moved_window_and_vanish_when_resized_or_gone() {
        let frame = Rect::new(0.0, 0.0, 1200.0, 1000.0);
        let set = LedgeSet { window: 7, frame, rects: vec![Rect::new(200.0, 300.0, 600.0, 330.0)] };
        let here = [WindowInfo { id: 7, rect: frame }];
        let placed = place_ledges(&here, &set);
        assert_eq!(placed.len(), 1);
        assert_eq!(placed[0].rect, Rect::new(200.0, 300.0, 600.0, 330.0));
        let moved = [WindowInfo { id: 7, rect: Rect::new(50.0, 20.0, 1250.0, 1020.0) }];
        let m = place_ledges(&moved, &set);
        assert_eq!(m[0].rect, Rect::new(250.0, 320.0, 650.0, 350.0));
        assert_eq!(m[0].id, placed[0].id, "moving the window keeps the id");
        let resized = [WindowInfo { id: 7, rect: Rect::new(0.0, 0.0, 900.0, 1000.0) }];
        assert!(place_ledges(&resized, &set).is_empty());
        assert!(place_ledges(&[WindowInfo { id: 8, rect: frame }], &set).is_empty());
    }

    #[test]
    fn ledge_id_changes_when_the_box_scrolls() {
        let frame = Rect::new(0.0, 0.0, 1200.0, 1000.0);
        let a = ledge_id(7, &frame, &Rect::new(200.0, 300.0, 600.0, 330.0));
        assert_eq!(a, ledge_id(7, &frame, &Rect::new(200.0, 300.0, 600.0, 330.0)));
        assert_ne!(a, ledge_id(7, &frame, &Rect::new(200.0, 260.0, 600.0, 290.0)));
        assert_ne!(a, ledge_id(8, &frame, &Rect::new(200.0, 300.0, 600.0, 330.0)));
    }

    #[test]
    fn probe_thread_hands_over_results_and_stops_when_dropped() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let probe = spawn_probe(Duration::from_millis(20), move || {
            Some(move || {
                c.fetch_add(1, Ordering::SeqCst);
                Some(LedgeSet { window: 1, frame: Rect::new(0.0, 0.0, 10.0, 10.0), rects: Vec::new() })
            })
        })
        .unwrap();
        let start = Instant::now();
        while probe.latest().is_none() && start.elapsed() < Duration::from_secs(2) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(probe.latest().map(|s| s.window), Some(1));
        drop(probe);
        std::thread::sleep(Duration::from_millis(120));
        let after = calls.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(calls.load(Ordering::SeqCst), after, "the thread stopped");
    }

    #[test]
    fn a_probe_that_cannot_start_hands_over_nothing() {
        let probe = spawn_probe(Duration::from_millis(20), || None::<fn() -> Option<LedgeSet>>).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert!(probe.latest().is_none());
    }
}
