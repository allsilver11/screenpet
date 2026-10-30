use std::ffi::c_void;
use std::mem::size_of;

use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON, VK_RBUTTON};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, SetWindowPos, GWL_EXSTYLE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOSIZE,
    SWP_NOZORDER, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use super::WindowSource;
use crate::geom::{Rect, WindowInfo};

/// 디딤판이 되면 안 되는 셸 창 클래스
const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
const MIN_SIZE: f64 = 50.0;

pub struct Win32Source;

impl WindowSource for Win32Source {
    fn windows(&mut self) -> Vec<WindowInfo> {
        let mut hwnds: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect_window), LPARAM(&mut hwnds as *mut Vec<HWND> as isize));
        }
        let own_pid = std::process::id();
        hwnds
            .into_iter()
            .filter(|&h| is_candidate(h, own_pid))
            .filter_map(|h| {
                let rect = frame_rect(h)?;
                (rect.width() >= MIN_SIZE && rect.height() >= MIN_SIZE)
                    .then_some(WindowInfo { id: h.0 as usize as u64, rect })
            })
            .collect()
    }

    fn work_areas(&mut self) -> Vec<Rect> {
        let mut list: Vec<Rect> = Vec::new();
        unsafe {
            let _ = EnumDisplayMonitors(HDC::default(), None, Some(collect_monitor), LPARAM(&mut list as *mut Vec<Rect> as isize));
        }
        list
    }

    /// 창 하나만: 핸들 확인 + DWM 테두리 한 번 (EnumWindows·창마다 검사 없이)
    fn rect_of(&mut self, id: u64) -> Option<Rect> {
        let h = HWND(id as usize as *mut c_void);
        unsafe {
            if !IsWindow(h).as_bool() || !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
                return None;
            }
        }
        frame_rect(h)
    }

    fn cursor(&mut self) -> (f64, f64) {
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
        }
        (p.x as f64, p.y as f64)
    }

    fn left_button_down(&mut self) -> bool {
        key_down(VK_LBUTTON.0)
    }

    fn right_button_down(&mut self) -> bool {
        key_down(VK_RBUTTON.0)
    }

    fn escape_down(&mut self) -> bool {
        key_down(VK_ESCAPE.0)
    }
}

fn key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    TRUE
}

unsafe extern "system" fn collect_monitor(hmon: HMONITOR, _hdc: HDC, _clip: *mut RECT, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<Rect>);
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(hmon, &mut info).as_bool() {
        list.push(to_rect(info.rcWork));
    }
    TRUE
}

fn to_rect(r: RECT) -> Rect {
    Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}

fn is_candidate(hwnd: HWND, own_pid: u32) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == own_pid {
            return false;
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & (WS_EX_TOOLWINDOW.0 | WS_EX_TRANSPARENT.0) != 0 {
            return false;
        }
        // 제목 없는 창은 대부분 오버레이·보조 창
        if GetWindowTextLengthW(hwnd) == 0 {
            return false;
        }
        // 다른 가상 데스크톱, 일시 중단된 UWP 앱 등
        let mut cloaked = 0u32;
        let res = DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut c_void, size_of::<u32>() as u32);
        if res.is_ok() && cloaked != 0 {
            return false;
        }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf).max(0) as usize;
        let class = String::from_utf16_lossy(&buf[..n]);
        !SHELL_CLASSES.contains(&class.as_str())
    }
}

/// 그림자를 뺀 실제 창 테두리
fn frame_rect(hwnd: HWND) -> Option<Rect> {
    let mut r = RECT::default();
    unsafe {
        DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &mut r as *mut RECT as *mut c_void, size_of::<RECT>() as u32).ok()?;
    }
    Some(to_rect(r))
}

/// 우리 창(hwnd)을 물리 px (x, y)로 바로 옮긴다 — Tauri set_position은 이벤트 루프를 한 번 거쳐 늦고 들쭉날쭉하다.
/// 크기·Z 순서·포커스는 그대로, 창 스레드를 기다리지 않는다 (SWP_ASYNCWINDOWPOS). 실패하면 false
pub fn move_window(hwnd: isize, x: i32, y: i32) -> bool {
    let h = HWND(hwnd as *mut c_void);
    unsafe { SetWindowPos(h, HWND::default(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS).is_ok() }
}
