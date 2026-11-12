//! 🪟 Windows 창·모니터·커서·키 읽기 (Win32 API) — WindowSource 구현과 창 옮기기·크기 바꾸기
//! 디딤판이 될 창만 고름: 보이고, 최소화되지 않았고, 내 앱·셸·도구 창·제목 없는 창이 아니고, MIN_SIZE 이상
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
use crate::typing::{KeyEdges, MAX_PER_POLL};

/// 디딤판이 되면 안 되는 셸 창 클래스
const SHELL_CLASSES: [&str; 4] = ["Progman", "WorkerW", "Shell_TrayWnd", "Shell_SecondaryTrayWnd"];
/// 이보다 작은 창 (물리 px)은 디딤판이 아님
const MIN_SIZE: f64 = 50.0;

/// 창·화면·커서는 상태 없이 읽음. 상태는 ⌨️ 타자 응원의 올라감 판단 (지난 읽기 때 눌려 있던 키 비트)과 마지막 입력 시각뿐
#[derive(Default)]
pub struct Win32Source {
    keys: KeyEdges,
    last_input: Option<u32>,
}

impl WindowSource for Win32Source {
    /// 보이는 최상위 창 목록 (EnumWindows, 앞 → 뒤, DWM 테두리 기준)
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

    /// 모니터별 작업 영역 (작업표시줄 뺌)
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

    /// 커서 자리 (물리 px)
    fn cursor(&mut self) -> (f64, f64) {
        let mut p = POINT::default();
        unsafe {
            let _ = GetCursorPos(&mut p);
        }
        (p.x as f64, p.y as f64)
    }

    /// 왼쪽 버튼이 눌려 있음
    fn left_button_down(&mut self) -> bool {
        key_down(VK_LBUTTON.0)
    }

    /// 오른쪽 버튼이 눌려 있음
    fn right_button_down(&mut self) -> bool {
        key_down(VK_RBUTTON.0)
    }

    /// Esc가 눌려 있음
    fn escape_down(&mut self) -> bool {
        key_down(VK_ESCAPE.0)
    }

    /// GetAsyncKeyState는 상태를 읽기만 함 (키는 앞에 있는 앱에도 그대로 감). 권한이 필요 없음
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

    // ---- ⌨️ typing sensor: 개수만 센다 (crate::typing). 키 번호는 이 읽기의 올라감 판단에만 쓰고 남기지 않는다
    /// 타자 키 ~70개의 눌림을 읽어 새로 눌린 수만 돌려줌. GetAsyncKeyState는 상태를 읽기만 함 (키는 앞 앱에 그대로 감,
    /// 훅을 걸지 않음, 권한이 필요 없음). 마지막 입력 시각 (GetLastInputInfo)이 그대로면 키를 읽지도 않음 —
    /// 손이 쉬면 틱마다 가벼운 호출 하나, 키보드·마우스를 쓰는 동안만 ~70번
    fn key_presses(&mut self) -> Option<u32> {
        /// 글자·숫자·기호·공백·지우기·엔터·탭·숫자판 (Ctrl·Alt·Shift·화살표·F키는 타자로 치지 않음)
        const TYPING_KEYS: [(u8, u8); 10] =
            [(0x08, 0x09), (0x0D, 0x0D), (0x20, 0x20), (0x2E, 0x2E), (0x30, 0x39), (0x41, 0x5A), (0x60, 0x6F), (0xBA, 0xC0), (0xDB, 0xDF), (0xE2, 0xE2)];
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        let input_at = unsafe { GetLastInputInfo(&mut info).as_bool() }.then_some(info.dwTime);
        let first = self.last_input.is_none();
        if input_at.is_some() && input_at == self.last_input {
            return Some(0);
        }
        self.last_input = input_at.or(Some(0));
        let mut presses = 0u32;
        for (from, to) in TYPING_KEYS {
            for vk in from..=to {
                // 높은 비트 = 지금 눌림, 낮은 비트 = 지난 읽기 뒤 눌린 적 있음 (빨리 친 키)
                let state = unsafe { GetAsyncKeyState(vk as i32) } as u16;
                if self.keys.press(vk, state & 0x8000 != 0, state & 1 != 0) {
                    presses += 1;
                }
            }
        }
        // 처음 읽기는 예전에 눌린 흔적까지 섞여 있어 세지 않는다
        Some(if first { 0 } else { presses.min(MAX_PER_POLL) })
    }
    // ---- end typing sensor

    /// 틱 수는 49.7일마다 한 바퀴 돎 — wrapping_sub면 그때도 맞음
    fn idle_seconds(&mut self) -> Option<f64> {
        let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        unsafe {
            if !GetLastInputInfo(&mut info).as_bool() {
                return None;
            }
            Some(GetTickCount().wrapping_sub(info.dwTime) as f64 / 1000.0)
        }
    }

    /// 📌 그 창 프로세스의 실행 파일 아이콘을 px 크기로 (PrivateExtractIconsW — 큰 아이콘을 골라 줄임), 안 되면 창의 아이콘
    /// (WM_GETICON — 멈춘 창은 기다리지 않음 · 창 클래스 아이콘). 검은 바탕·흰 바탕에 한 번씩 그려 투명함을 얻음 (icon::from_black_white).
    /// 경로는 아이콘을 꺼내는 데만 쓰고 버림
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

/// 창 프로세스의 실행 파일에서 px 크기 아이콘을 꺼냄 (DestroyIcon은 부른 쪽이)
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

/// 가상 키가 지금 눌려 있음 (GetAsyncKeyState)
fn key_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

/// EnumWindows 콜백: 창 핸들을 목록에 모음
unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HWND>);
    list.push(hwnd);
    TRUE
}

/// EnumDisplayMonitors 콜백: 모니터 작업 영역을 목록에 모음
unsafe extern "system" fn collect_monitor(hmon: HMONITOR, _hdc: HDC, _clip: *mut RECT, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<Rect>);
    let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
    if GetMonitorInfoW(hmon, &mut info).as_bool() {
        list.push(to_rect(info.rcWork));
    }
    TRUE
}

/// Win32 RECT → Rect
fn to_rect(r: RECT) -> Rect {
    Rect::new(r.left as f64, r.top as f64, r.right as f64, r.bottom as f64)
}

/// 디딤판 후보 창임 (보임·최소화 아님·내 앱 아님·도구/투명 창 아님·제목 있음 등)
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

/// 우리 창(hwnd) 크기를 물리 px로 바꿈 — 자리·Z 순서·포커스는 그대로 (Tauri set_size는 창을 활성화함)
pub fn resize_window(hwnd: isize, w: i32, h: i32) -> bool {
    let h_ = HWND(hwnd as *mut c_void);
    unsafe { SetWindowPos(h_, HWND::default(), 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS).is_ok() }
}

/// 우리 창(hwnd)을 물리 px (x, y)로 바로 옮김 — Tauri set_position은 이벤트 루프를 한 번 거쳐 늦고 들쭉날쭉함
/// 크기·Z 순서·포커스는 그대로, 창 스레드를 기다리지 않음 (SWP_ASYNCWINDOWPOS). 실패하면 false
pub fn move_window(hwnd: isize, x: i32, y: i32) -> bool {
    let h = HWND(hwnd as *mut c_void);
    unsafe { SetWindowPos(h, HWND::default(), x, y, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS).is_ok() }
}
