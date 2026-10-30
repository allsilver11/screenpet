//! macOS 창·화면·커서 읽기. 좌표계와 배율 처리는 `mac_coords` 문서를 본다 —
//! 게임 세계는 tao/Tauri의 "물리 px" (= CG 전역 포인트 × 그 화면의 backingScaleFactor).
//!
//! 창 *제목*은 읽지 않는다 (제목만 화면 기록 권한이 필요하다). 경계·레이어·pid·알파는 권한 없이 읽힌다.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
use objc2_core_graphics::{
    kCGNullWindowID, kCGWindowAlpha, kCGWindowBounds, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
    CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode, CGError, CGEvent, CGEventSource,
    CGEventSourceStateID, CGGetActiveDisplayList, CGMouseButton, CGRectMakeWithDictionaryRepresentation,
    CGWindowListCopyWindowInfo, CGWindowListOption,
};

use super::mac_coords::{self, Screen};
use super::WindowSource;
use crate::geom::{Rect, WindowInfo};

/// 이보다 작은 창 (물리 px)은 디딤판이 아니다 — Windows와 같은 값
const MIN_SIZE: f64 = 50.0;

pub struct MacSource;

impl WindowSource for MacSource {
    fn windows(&mut self) -> Vec<WindowInfo> {
        let screens = screens();
        let own_pid = std::process::id() as i64;
        let option = CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
        let Some(list) = CGWindowListCopyWindowInfo(option, kCGNullWindowID) else {
            return Vec::new();
        };
        // SAFETY: CGWindowListCopyWindowInfo는 CFDictionary(CFString → 값)들의 배열을 돌려준다.
        let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> = unsafe { CFRetained::cast_unchecked(list) };
        // 앞→뒤 순서로 온다
        list.iter().filter_map(|info| window_info(&info, &screens, own_pid)).collect()
    }

    fn work_areas(&mut self) -> Vec<Rect> {
        mac_coords::work_areas_physical(&screens())
    }

    /// 창 하나만 묻는다 (kCGWindowListOptionIncludingWindow) — 전체 목록보다 훨씬 싸다
    fn rect_of(&mut self, id: u64) -> Option<Rect> {
        let id = u32::try_from(id).ok()?;
        let list = CGWindowListCopyWindowInfo(CGWindowListOption::OptionIncludingWindow, id)?;
        // SAFETY: windows()와 같다 — CFDictionary(CFString → 값)들의 배열
        let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> = unsafe { CFRetained::cast_unchecked(list) };
        let screens = screens();
        let own_pid = std::process::id() as i64;
        list.iter().filter_map(|info| window_info(&info, &screens, own_pid)).find(|w| w.id == id as u64).map(|w| w.rect)
    }

    fn cursor(&mut self) -> (f64, f64) {
        let Some(ev) = CGEvent::new(None) else { return (0.0, 0.0) };
        let p = CGEvent::location(Some(&ev));
        mac_coords::point_to_physical(&screens(), p.x, p.y)
    }

    fn left_button_down(&mut self) -> bool {
        CGEventSource::button_state(CGEventSourceStateID::CombinedSessionState, CGMouseButton::Left)
    }

    fn right_button_down(&mut self) -> bool {
        CGEventSource::button_state(CGEventSourceStateID::CombinedSessionState, CGMouseButton::Right)
    }

    /// 다른 앱이 앞에 있을 때 일반 키 상태는 macOS 10.15+에서 "입력 모니터링" 권한이 없으면 항상 false일 수 있다
    /// (권한을 요청하지는 않는다 — 오른쪽 버튼으로도 거둘 수 있다).
    fn escape_down(&mut self) -> bool {
        CGEventSource::key_state(CGEventSourceStateID::CombinedSessionState, KVK_ESCAPE)
    }
}

/// 창 정보 사전 하나 → 디딤판 후보 창 (보통 앱 창 레이어, 다른 프로세스, 보이는 것, 충분히 큰 것만)
fn window_info(info: &CFDictionary<CFString, CFType>, screens: &[Screen], own_pid: i64) -> Option<WindowInfo> {
    // 레이어 0 = 보통 앱 창 (메뉴 막대·Dock·떠 있는 패널·화면 보호기 등은 다른 레이어)
    if number(info, unsafe { kCGWindowLayer })?.as_i64()? != 0 {
        return None;
    }
    if number(info, unsafe { kCGWindowOwnerPID })?.as_i64()? == own_pid {
        return None;
    }
    let alpha = number(info, unsafe { kCGWindowAlpha }).and_then(|n| n.as_f64()).unwrap_or(1.0);
    if alpha <= 0.0 {
        return None;
    }
    let id = number(info, unsafe { kCGWindowNumber })?.as_i64()? as u64;
    let bounds = info.get(unsafe { kCGWindowBounds })?.downcast::<CFDictionary>().ok()?;
    let mut r = CGRect::default();
    if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut r) } {
        return None;
    }
    let pts = Rect::new(r.origin.x, r.origin.y, r.origin.x + r.size.width, r.origin.y + r.size.height);
    let rect = mac_coords::rect_to_physical(screens, pts);
    (rect.width() >= MIN_SIZE && rect.height() >= MIN_SIZE).then_some(WindowInfo { id, rect })
}

/// Carbon kVK_Escape
const KVK_ESCAPE: u16 = 0x35;

fn number(info: &CFDictionary<CFString, CFType>, key: &CFString) -> Option<CFRetained<CFNumber>> {
    info.get(key)?.downcast::<CFNumber>().ok()
}

// ---- 화면 목록 ----
//
// NSScreen은 메인 스레드에서만 읽을 수 있는데 게임 루프·하우스 끌기는 다른 스레드에서 돈다.
// 그래서 마지막으로 읽은 목록을 들고 있다가 돌려주고, 메인 큐에 비동기로 새로 읽기를 건다
// (기다리지 않는다 — 메인 스레드가 게임 루프를 기다리는 중이어도 교착이 없다).
// 아직 한 번도 못 읽었으면 CoreGraphics로 화면 전체 경계를 대신 쓴다 (메뉴 막대·Dock 포함, 첫 틱만).

static CACHE: OnceLock<Mutex<Option<Vec<Screen>>>> = OnceLock::new();
static REFRESHING: AtomicBool = AtomicBool::new(false);

fn cache() -> &'static Mutex<Option<Vec<Screen>>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn screens() -> Vec<Screen> {
    if let Some(mtm) = MainThreadMarker::new() {
        let list = read_ns_screens(mtm);
        *cache().lock().unwrap() = Some(list.clone());
        return list;
    }
    if !REFRESHING.swap(true, Ordering::AcqRel) {
        DispatchQueue::main().exec_async(|| {
            if let Some(mtm) = MainThreadMarker::new() {
                let list = read_ns_screens(mtm);
                *cache().lock().unwrap() = Some(list);
            }
            REFRESHING.store(false, Ordering::Release);
        });
    }
    if let Some(list) = cache().lock().unwrap().clone() {
        return list;
    }
    cg_screens()
}

fn read_ns_screens(mtm: MainThreadMarker) -> Vec<Screen> {
    let all = NSScreen::screens(mtm);
    // Cocoa 원점은 screens[0] (메뉴 막대가 있는 주 화면)의 왼쪽 아래
    let Some(primary) = all.iter().next() else { return Vec::new() };
    let primary_height = primary.frame().size.height;
    all.iter()
        .map(|s| {
            let (f, v) = (s.frame(), s.visibleFrame());
            Screen {
                frame: mac_coords::cocoa_to_top_left(f.origin.x, f.origin.y, f.size.width, f.size.height, primary_height),
                visible: mac_coords::cocoa_to_top_left(v.origin.x, v.origin.y, v.size.width, v.size.height, primary_height),
                scale: s.backingScaleFactor(),
            }
        })
        .collect()
}

/// 아무 스레드에서나 읽히는 대체 목록 — CGDisplayBounds는 이미 왼쪽 위 원점 포인트다.
/// 배율은 디스플레이 모드의 픽셀 너비 / 포인트 너비.
fn cg_screens() -> Vec<Screen> {
    let mut ids = [0 as CGDirectDisplayID; 16];
    let mut count = 0u32;
    let err = unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut count) };
    if err != CGError::Success {
        return Vec::new();
    }
    ids[..count as usize]
        .iter()
        .map(|&id| {
            let b = CGDisplayBounds(id);
            let frame = Rect::new(b.origin.x, b.origin.y, b.origin.x + b.size.width, b.origin.y + b.size.height);
            let scale = CGDisplayCopyDisplayMode(id)
                .map(|m| {
                    let (px, pt) = (CGDisplayMode::pixel_width(Some(&m)), CGDisplayMode::width(Some(&m)));
                    if pt > 0 { px as f64 / pt as f64 } else { 1.0 }
                })
                .unwrap_or(1.0);
            Screen { frame, visible: frame, scale }
        })
        .collect()
}
