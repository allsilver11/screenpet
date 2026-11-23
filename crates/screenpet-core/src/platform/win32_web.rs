//! 🌐 Windows: 맨 앞 브라우저 페이지의 요소 사각형 읽기 (UI Automation) — 웹 페이지 디딤돌(ledge.rs)의 OS 쪽
//! 따로 도는 스레드 하나에서 COM(MTA)을 열고 0.5초마다 한 번: 맨 앞 창이 브라우저면 페이지 영역(Document)을 성긴 격자로 찍어
//! 그 자리 요소의 경계·종류만 읽음 (한 번에 최대 BUDGET). 캐시 요청에 이름·값·글자를 넣지 않아 읽지 않음. 안 되면 빈 결과
use std::ffi::c_void;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest, IUIAutomationElement, IUIAutomationTreeWalker,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId,
};
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, WindowFromPoint, GA_ROOT};

use super::win32::{exe_path, Win32Source};
use super::WindowSource;
use crate::geom::Rect;
use crate::ledge::{self, LedgeProbe, LedgeSet};

/// 격자 칸 수 (가로 × 세로)
const COLS: usize = 12;
const ROWS: usize = 10;
/// 한 번 읽기에 쓰는 최대 시간 — 넘으면 남은 칸은 건너뜀 (브라우저가 느려도 CPU를 오래 잡지 않음)
const BUDGET: Duration = Duration::from_millis(60);
/// UIA 한 번 부르기를 기다리는 최대 시간 (ms) — 멈춘 브라우저에 오래 매달리지 않음
const UIA_TIMEOUT_MS: u32 = 300;
/// 페이지 영역(Document)을 찾아 부모로 올라가는 최대 걸음
const MAX_CLIMB: usize = 40;
/// 같은 창·같은 경계면 찾아 둔 페이지 영역을 이만큼 다시 씀
const VIEW_KEEP: Duration = Duration::from_secs(2);

/// 이 스레드의 COM을 닫음 (UIA 객체들보다 나중에 버려지도록 Uia의 마지막 필드)
struct ComGuard;

impl Drop for ComGuard {
    /// CoUninitialize
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// 읽기 스레드의 UIA 도구들 (이 스레드에서만 씀)
struct Uia {
    automation: IUIAutomation,
    /// 경계·컨트롤 종류만 함께 받아 오는 캐시 요청 (한 번 왕복)
    cache: IUIAutomationCacheRequest,
    walker: IUIAutomationTreeWalker,
    /// 찾아 둔 페이지 영역: (창 id, 창 경계, 영역, 찾은 때)
    view: Option<(u64, Rect, Rect, Instant)>,
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
            let walker = automation.ControlViewWalker().ok()?;
            Some(Self { automation, cache, walker, view: None, _com: com })
        }
    }

    /// 한 번 읽기: 맨 앞 창이 브라우저가 아니거나 페이지 영역을 못 찾으면 None
    fn probe(&mut self) -> Option<LedgeSet> {
        let (hwnd, frame) = front_browser()?;
        let started = Instant::now();
        let view = self.page_view(hwnd, frame)?;
        let cell_h = view.height() / ROWS as f64;
        let mut found: Vec<Rect> = Vec::new();
        for (x, y) in ledge::probe_grid(view, COLS, ROWS) {
            if started.elapsed() > BUDGET {
                break;
            }
            // 이미 찾은 작은 요소 안의 점은 같은 것일 테니 건너뜀 (큰 묶음 안은 그 안의 글을 찾아야 하니 계속 찍음)
            if found.iter().any(|r| r.height() <= 2.0 * cell_h && r.contains(x, y)) {
                continue;
            }
            let pt = POINT { x: x as i32, y: y as i32 };
            // 그 점의 맨 위 창이 그 브라우저일 때만 (내 펫 창·다른 창 위에서는 묻지 않음)
            if !owned_by(pt, hwnd) {
                continue;
            }
            let Ok(el) = (unsafe { self.automation.ElementFromPointBuildCache(pt, &self.cache) }) else { continue };
            if let Some((kind, rect)) = cached(&el) {
                if ledge::keeps_control_type(kind) {
                    found.push(rect);
                }
            }
        }
        Some(LedgeSet { window: hwnd.0 as usize as u64, frame, rects: ledge::pick_ledges(&found, view) })
    }

    /// 페이지 보이는 영역: 창 가운데 요소에서 부모로 올라가며 만난 가장 바깥 Document의 경계 (창 안으로 자름). 잠깐 기억해 둠
    fn page_view(&mut self, hwnd: HWND, frame: Rect) -> Option<Rect> {
        let id = hwnd.0 as usize as u64;
        if let Some((vid, vframe, view, at)) = self.view {
            if vid == id && vframe == frame && at.elapsed() < VIEW_KEEP {
                return Some(view);
            }
        }
        self.view = None;
        let pt = POINT { x: ((frame.left + frame.right) / 2.0) as i32, y: ((frame.top + frame.bottom) / 2.0) as i32 };
        if !owned_by(pt, hwnd) {
            return None;
        }
        let mut el = unsafe { self.automation.ElementFromPointBuildCache(pt, &self.cache) }.ok()?;
        let mut doc: Option<Rect> = None;
        for _ in 0..MAX_CLIMB {
            match cached(&el) {
                Some((kind, rect)) if kind == UIA_DocumentControlTypeId.0 => doc = Some(rect),
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

/// 캐시해 둔 (컨트롤 종류 번호, 경계). 빈 사각형이면 None
fn cached(el: &IUIAutomationElement) -> Option<(i32, Rect)> {
    unsafe {
        let kind = el.CachedControlType().ok()?.0;
        let r = el.CachedBoundingRectangle().ok()?;
        (r.right > r.left && r.bottom > r.top).then(|| (kind, Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)))
    }
}

/// 점의 맨 위 창(최상위)이 hwnd임
fn owned_by(pt: POINT, hwnd: HWND) -> bool {
    unsafe {
        let w = WindowFromPoint(pt);
        !w.0.is_null() && GetAncestor(w, GA_ROOT) == hwnd
    }
}

/// 맨 앞 디딤판 후보 창(내 앱 아님)이 브라우저면 (핸들, 경계)
fn front_browser() -> Option<(HWND, Rect)> {
    let front = Win32Source::default().windows().into_iter().next()?;
    let hwnd = HWND(front.id as usize as *mut c_void);
    let path = unsafe { exe_path(hwnd) }?;
    let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
    ledge::is_browser_exe(&String::from_utf16_lossy(&path[..n])).then_some((hwnd, front.rect))
}

/// 🌐 디딤돌 읽기 스레드를 띄움 (스레드 안에서 COM·UIA를 만듦 — 못 만들면 스레드가 끝나고 디딤돌이 없을 뿐)
pub fn spawn() -> Option<LedgeProbe> {
    ledge::spawn_probe(ledge::PROBE_EVERY, || {
        let mut uia = Uia::new()?;
        Some(move || uia.probe())
    })
}
