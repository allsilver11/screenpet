//! 🌐 Windows: 맨 앞 브라우저 페이지의 요소 사각형 읽기 (UI Automation) — 웹 페이지 디딤돌(ledge.rs)의 OS 쪽
//! 따로 도는 스레드 하나에서 COM(MTA)을 열고 0.5초마다 한 번: 맨 앞 창이 브라우저면 페이지 영역(Document)을 성긴 격자로 찍어
//! 그 자리 요소의 경계·종류·프로세스 id만 읽음 (한 번에 최대 BUDGET, 시작 칸은 판마다 돌리고 못 찍은 자리는 지난 판 것을 이어 씀).
//! 캐시 요청에 이름·값·글자를 넣지 않아 읽지 않음. 브라우저가 아니면 지우고, 읽다 실패하면 지난 결과를 둠
use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest, IUIAutomationElement, IUIAutomationTreeWalker,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId, UIA_ProcessIdPropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetWindowThreadProcessId, WindowFromPoint, GA_ROOT};

use super::win32::{exe_path, Win32Source};
use super::WindowSource;
use crate::geom::Rect;
use crate::ledge::{self, LedgeProbe, LedgeSet, Probed};

/// 격자 칸 수 (가로 × 세로)
const COLS: usize = 12;
const ROWS: usize = 10;
/// 한 번 격자 찍기에 쓰는 최대 시간 (페이지 영역 찾기는 빼고) — 넘으면 남은 칸은 다음 판에 (rotate_points·merge_ledges)
const BUDGET: Duration = Duration::from_millis(60);
/// UIA 한 번 부르기를 기다리는 최대 시간 (ms) — 멈춘 브라우저에 오래 매달리지 않음
const UIA_TIMEOUT_MS: u32 = 300;
/// 페이지 영역(Document)을 찾아 부모로 올라가는 최대 걸음
const MAX_CLIMB: usize = 40;
/// 같은 창·같은 경계여도 이만큼 지나면 페이지 영역을 다시 찾음 (개발자 도구를 열어 영역이 바뀐 때 등). 찾는 시간은 예산에 안 셈
const VIEW_KEEP: Duration = Duration::from_secs(10);

/// 이 스레드의 COM을 닫음 (UIA 객체들보다 나중에 버려지도록 Uia의 마지막 필드)
struct ComGuard;

impl Drop for ComGuard {
    /// CoUninitialize
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// 맨 앞 브라우저 창: 핸들, 경계, 프로세스 id
#[derive(Clone, Copy)]
struct Front {
    hwnd: HWND,
    frame: Rect,
    pid: u32,
}

impl Front {
    /// 창 id (WindowInfo.id와 같음)
    fn id(&self) -> u64 {
        self.hwnd.0 as usize as u64
    }
}

/// 지난 판 결과 (같은 창·경계·영역이면 다음 판에 이어 씀)
struct Prev {
    window: u64,
    frame: Rect,
    view: Rect,
    rects: Vec<Rect>,
}

/// 읽기 스레드의 UIA 도구들 (이 스레드에서만 씀)
struct Uia {
    automation: IUIAutomation,
    /// 경계·컨트롤 종류·프로세스 id만 함께 받아 오는 캐시 요청 (한 번 왕복)
    cache: IUIAutomationCacheRequest,
    walker: IUIAutomationTreeWalker,
    /// 찾아 둔 페이지 영역: (창 id, 창 경계, 영역, 찾은 때)
    view: Option<(u64, Rect, Rect, Instant)>,
    /// 지난 판 디딤돌
    prev: Option<Prev>,
    /// 몇 번째 판인지 (격자 시작 칸 돌리기)
    round: usize,
    _com: ComGuard,
}

impl Uia {
    /// COM(MTA)을 열고 UIA를 만듦 (CUIAutomation8이면 시간 제한도 걺). 안 되면 None
    fn new() -> Option<Self> {
        unsafe {
            if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
                return None;
            }
            let com = ComGuard;
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).or_else(|_| CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)).ok()?;
            if let Ok(a2) = automation.cast::<IUIAutomation2>() {
                let _ = a2.SetConnectionTimeout(UIA_TIMEOUT_MS);
                let _ = a2.SetTransactionTimeout(UIA_TIMEOUT_MS);
            }
            let cache = automation.CreateCacheRequest().ok()?;
            cache.AddProperty(UIA_BoundingRectanglePropertyId).ok()?;
            cache.AddProperty(UIA_ControlTypePropertyId).ok()?;
            cache.AddProperty(UIA_ProcessIdPropertyId).ok()?;
            let walker = automation.ControlViewWalker().ok()?;
            Some(Self { automation, cache, walker, view: None, prev: None, round: 0, _com: com })
        }
    }

    /// 한 번 읽기: 맨 앞 창이 브라우저가 아니면 NotBrowser, 페이지 영역을 못 찾았거나 읽는 사이 창이 움직였으면 Failed
    fn probe(&mut self) -> Probed {
        let Some(front) = front_browser() else {
            self.prev = None;
            return Probed::NotBrowser;
        };
        let Some(view) = self.page_view(front) else { return Probed::Failed };
        // 예산은 페이지 영역을 찾은 뒤부터 셈
        let started = Instant::now();
        let cell_h = view.height() / ROWS as f64;
        self.round = self.round.wrapping_add(1);
        let mut found: Vec<Rect> = Vec::new();
        let mut probed: Vec<(f64, f64)> = Vec::new();
        for (x, y) in ledge::rotate_points(ledge::probe_grid(view, COLS, ROWS), self.round) {
            if started.elapsed() > BUDGET {
                break;
            }
            // 이미 찾은 작은 요소 안의 점은 같은 것일 테니 건너뜀 (큰 묶음 안은 그 안의 글을 찾아야 하니 계속 찍음)
            if found.iter().any(|r| r.height() <= 2.0 * cell_h && r.contains(x, y)) {
                continue;
            }
            let pt = POINT { x: x as i32, y: y as i32 };
            // 그 점의 맨 위 창이 그 브라우저일 때만 (내 펫 창·다른 창 위에서는 묻지 않음)
            if !owned_by(pt, front.hwnd) {
                continue;
            }
            let Ok(el) = (unsafe { self.automation.ElementFromPointBuildCache(pt, &self.cache) }) else { continue };
            // 그 사이 다른 창(내 펫 웹뷰 등)이 끼어들었으면 그 요소는 버림
            let Some((kind, rect, pid)) = cached(&el) else { continue };
            if pid != front.pid {
                continue;
            }
            probed.push((x, y));
            if ledge::keeps_control_type(kind) {
                found.push(rect);
            }
        }
        // 읽는 사이 창이 움직이거나 크기가 바뀌었으면 사각형이 어긋났음 — 이번 판은 버림 (지난 결과 유지)
        if Win32Source::default().rect_of(front.id()) != Some(front.frame) {
            self.view = None;
            self.prev = None;
            return Probed::Failed;
        }
        let prev = self.prev.take().filter(|p| p.window == front.id() && p.frame == front.frame && p.view == view).map_or_else(Vec::new, |p| p.rects);
        let rects = ledge::merge_ledges(&prev, &found, &probed, view);
        self.prev = Some(Prev { window: front.id(), frame: front.frame, view, rects: rects.clone() });
        Probed::Found(LedgeSet { window: front.id(), frame: front.frame, rects })
    }

    /// 페이지 보이는 영역: 브라우저가 맨 위인 점(가운데부터 3×3 격자)의 요소에서 부모로 올라가며 만난 가장 바깥 Document의 경계
    /// (창 안으로 자름). 같은 창·경계면 VIEW_KEEP 동안 다시 씀 — 창이 바뀌거나 움직이면 새로 찾음
    fn page_view(&mut self, front: Front) -> Option<Rect> {
        let (id, frame) = (front.id(), front.frame);
        if let Some((vid, vframe, view, at)) = self.view {
            if vid == id && vframe == frame && at.elapsed() < VIEW_KEEP {
                return Some(view);
            }
        }
        self.view = None;
        let center = ((frame.left + frame.right) / 2.0, (frame.top + frame.bottom) / 2.0);
        let starts = std::iter::once(center).chain(ledge::probe_grid(frame, 3, 3));
        let pt = starts.map(|(x, y)| POINT { x: x as i32, y: y as i32 }).find(|&pt| owned_by(pt, front.hwnd))?;
        let mut el = unsafe { self.automation.ElementFromPointBuildCache(pt, &self.cache) }.ok()?;
        if cached(&el).map(|c| c.2) != Some(front.pid) {
            return None;
        }
        let mut doc: Option<Rect> = None;
        for _ in 0..MAX_CLIMB {
            match cached(&el) {
                Some((kind, rect, _)) if kind == UIA_DocumentControlTypeId.0 => doc = Some(rect),
                Some(_) => {}
                None => break,
            }
            match unsafe { self.walker.GetParentElementBuildCache(&el, &self.cache) } {
                Ok(parent) => el = parent,
                Err(_) => break,
            }
        }
        let d = doc?;
        let view = Rect::new(d.left.max(frame.left), d.top.max(frame.top), d.right.min(frame.right), d.bottom.min(frame.bottom));
        if view.width() < ledge::MIN_WIDTH || view.height() < ledge::MIN_HEIGHT {
            return None;
        }
        self.view = Some((id, frame, view, Instant::now()));
        Some(view)
    }
}

/// 캐시해 둔 (컨트롤 종류 번호, 경계, 프로세스 id). 빈 사각형이면 None
fn cached(el: &IUIAutomationElement) -> Option<(i32, Rect, u32)> {
    unsafe {
        let kind = el.CachedControlType().ok()?.0;
        let r = el.CachedBoundingRectangle().ok()?;
        let pid = el.CachedProcessId().ok()? as u32;
        (r.right > r.left && r.bottom > r.top).then(|| (kind, Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64), pid))
    }
}

/// 점의 맨 위 창(최상위)이 hwnd임
fn owned_by(pt: POINT, hwnd: HWND) -> bool {
    unsafe {
        let w = WindowFromPoint(pt);
        !w.0.is_null() && GetAncestor(w, GA_ROOT) == hwnd
    }
}

/// 맨 앞 디딤판 후보 창(내 앱 아님)이 브라우저면 그 창
fn front_browser() -> Option<Front> {
    let front = Win32Source::default().windows().into_iter().next()?;
    let hwnd = HWND(front.id as usize as *mut c_void);
    let path = unsafe { exe_path(hwnd) }?;
    let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    if !ledge::is_browser_exe(&String::from_utf16_lossy(&path[..n])) {
        return None;
    }
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32)) };
    (pid != 0).then_some(Front { hwnd, frame: front.rect, pid })
}

/// 🌐 디딤돌 읽기 스레드를 띄움 (스레드 안에서 COM·UIA를 만듦 — 못 만들면 스레드가 끝나고 디딤돌이 없을 뿐)
pub fn spawn() -> Option<LedgeProbe> {
    ledge::spawn_probe(ledge::PROBE_EVERY, || {
        let mut uia = Uia::new()?;
        Some(move || uia.probe())
    })
}
