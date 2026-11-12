use std::ffi::c_void;
use std::mem::size_of;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, BOOL, FALSE, HANDLE, HWND, LPARAM, POINT, RECT, TRUE, WPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors, GdiFlush, GetMonitorInfoW, SelectObject, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBRUSH, HDC, HGDIOBJ, HMONITOR, MONITORINFO,
};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetLastInputInfo, LASTINPUTINFO, VK_DOWN, VK_ESCAPE, VK_LBUTTON, VK_LEFT, VK_RBUTTON, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DestroyIcon, DrawIconEx, GetClassLongPtrW, PrivateExtractIconsW, SendMessageTimeoutW, DI_NORMAL, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL2,
    SMTO_ABORTIFHUNG, WM_GETICON,
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId,
    IsIconic, IsWindow, IsWindowVisible, SetWindowPos, GWL_EXSTYLE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

use super::WindowSource;
use crate::brain::control_moves::ControlInput;
use crate::geom::{Rect, WindowInfo};
use crate::icon::{self, IconImage};

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

    /// GetAsyncKeyState는 상태를 읽기만 한다 (키는 앞에 있는 앱에도 그대로 간다). 권한이 필요 없다
    fn control_keys(&mut self) -> ControlInput {
        ControlInput {
            left: key_down(VK_LEFT.0),
            right: key_down(VK_RIGHT.0),
            up: key_down(VK_UP.0),
            down: key_down(VK_DOWN.0),
            jump: key_down(VK_SPACE.0),
            run: key_down(VK_SHIFT.0),
        }
    }

    /// 틱 수는 49.7일마다 한 바퀴 돈다 — wrapping_sub면 그때도 맞다
    fn idle_seconds(&mut self) -> Option<f64> {
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        unsafe {
            if !GetLastInputInfo(&mut info).as_bool() {
                return None;
            }
            Some(GetTickCount().wrapping_sub(info.dwTime) as f64 / 1000.0)
        }
    }

    /// 📌 그 창 프로세스의 실행 파일 아이콘을 px 크기로 (PrivateExtractIconsW — 큰 아이콘을 골라 줄인다), 안 되면 창의 아이콘
    /// (WM_GETICON — 멈춘 창은 기다리지 않는다 · 창 클래스 아이콘). 검은 바탕·흰 바탕에 한 번씩 그려 투명함을 얻는다 (icon::from_black_white).
    /// 경로는 아이콘을 꺼내는 데만 쓰고 버린다
    fn app_icon(&mut self, window: u64, px: u32) -> Option<IconImage> {
        let h = HWND(window as usize as *mut c_void);
        if px == 0 || px > 256 {
            return None;
        }
        unsafe {
            if !IsWindow(h).as_bool() {
                return None;
            }
            if let Some(hicon) = exe_icon(h, px) {
                let img = render_icon(hicon, px);
                let _ = DestroyIcon(hicon);
                if img.is_some() {
                    return img;
                }
            }
            // 창이 가진 아이콘 (빌린 것 — 지우지 않는다)
            window_icons(h).into_iter().find_map(|hicon| render_icon(hicon, px))
        }
    }
}

/// 창 프로세스의 실행 파일에서 px 크기 아이콘을 꺼낸다 (DestroyIcon은 부른 쪽이)
unsafe fn exe_icon(h: HWND, px: u32) -> Option<HICON> {
    let mut pid = 0u32;
    GetWindowThreadProcessId(h, Some(&mut pid as *mut u32));
    if pid == 0 || pid == std::process::id() {
        return None;
    }
    let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid).ok()?;
    let mut path = [0u16; 260];
    let mut len = path.len() as u32;
    let ok = QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(path.as_mut_ptr()), &mut len).is_ok();
    let _ = CloseHandle(process);
    // 끝의 0까지 들어가야 한다 (더 긴 경로는 건너뛴다)
    if !ok || len as usize >= path.len() {
        return None;
    }
    let mut icons = [HICON::default(); 1];
    let n = PrivateExtractIconsW(&path, 0, px as i32, px as i32, Some(&mut icons), None, 0);
    (n == 1 && !icons[0].0.is_null()).then_some(icons[0])
}

/// 창이 가진 아이콘들: 큰 것부터 (빌린 핸들)
unsafe fn window_icons(h: HWND) -> Vec<HICON> {
    let mut out = Vec::new();
    for kind in [ICON_BIG, ICON_SMALL2] {
        let mut got = 0usize;
        let sent = SendMessageTimeoutW(h, WM_GETICON, WPARAM(kind as usize), LPARAM(0), SMTO_ABORTIFHUNG, 100, Some(&mut got as *mut usize));
        if sent.0 != 0 && got != 0 {
            out.push(HICON(got as *mut c_void));
        }
    }
    for index in [GCLP_HICON, GCLP_HICONSM] {
        let v = GetClassLongPtrW(h, index);
        if v != 0 {
            out.push(HICON(v as *mut c_void));
        }
    }
    out
}

/// 아이콘을 px × px 32비트 DIB에 검은 바탕·흰 바탕으로 한 번씩 그려 RGBA로
unsafe fn render_icon(hicon: HICON, px: u32) -> Option<IconImage> {
    let dc = CreateCompatibleDC(HDC::default());
    if dc.0.is_null() {
        return None;
    }
    let side = px as i32;
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: side,
            // 음수 = 위→아래 줄
            biHeight: -side,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let Ok(bmp) = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0) else {
        let _ = DeleteDC(dc);
        return None;
    };
    let old = SelectObject(dc, HGDIOBJ(bmp.0));
    let n = (px * px * 4) as usize;
    let draw = |fill: u8| -> Option<Vec<u8>> {
        if bits.is_null() {
            return None;
        }
        let cells = std::slice::from_raw_parts_mut(bits as *mut u8, n);
        cells.fill(fill);
        DrawIconEx(dc, 0, 0, hicon, side, side, 0, HBRUSH::default(), DI_NORMAL).ok()?;
        let _ = GdiFlush();
        Some(cells.to_vec())
    };
    let black = draw(0);
    let white = draw(255);
    SelectObject(dc, old);
    let _ = DeleteObject(HGDIOBJ(bmp.0));
    let _ = DeleteDC(dc);
    icon::from_black_white(px, &black?, &white?)
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

/// 우리 창(hwnd) 크기를 물리 px로 바꾼다 — 자리·Z 순서·포커스는 그대로 (Tauri set_size는 창을 활성화한다)
pub fn resize_window(hwnd: isize, w: i32, h: i32) -> bool {
    let h_ = HWND(hwnd as *mut c_void);
    unsafe { SetWindowPos(h_, HWND::default(), 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS).is_ok() }
}

/// 우리 창(hwnd)을 물리 px (x, y)로 바로 옮긴다 — Tauri set_position은 이벤트 루프를 한 번 거쳐 늦고 들쭉날쭉하다.
/// 크기·Z 순서·포커스는 그대로, 창 스레드를 기다리지 않는다 (SWP_ASYNCWINDOWPOS). 실패하면 false
pub fn move_window(hwnd: isize, x: i32, y: i32) -> bool {
    let h = HWND(hwnd as *mut c_void);
    unsafe { SetWindowPos(h, HWND::default(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS).is_ok() }
}
