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
    EnumWindows, GetClassNameW, GetCursorPos, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, SetWindowLongW, SWP_FRAMECHANGED, WS_EX_LAYERED,
    IsIconic, IsWindow, IsWindowVisible, SetWindowPos, ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE, GWL_EXSTYLE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
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
    fn session_events(&mut self) -> crate::wake::SessionEvents {
        session_watch::events()
    }

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

/// 창 프로세스의 실행 파일 경로 (끝의 0 포함, 260자 안 — 더 긴 경로·내 앱은 None). 아이콘 꺼내기·🌐 브라우저 알아보기에만 쓰고 버림
pub(super) unsafe fn exe_path(h: HWND) -> Option<[u16; 260]> {
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
    path[len as usize] = 0;
    Some(path)
}

/// 창 프로세스의 실행 파일에서 px 크기 아이콘을 꺼냄 (DestroyIcon은 부른 쪽이)
unsafe fn exe_icon(h: HWND, px: u32) -> Option<HICON> {
    let path = exe_path(h)?;
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

/// 숨겨 둔 우리 창(hwnd)을 활성화 없이 보임 — 지금 쓰던 앱의 포커스·입력을 뺏지 않음 (Tauri show()는 SW_SHOW라 활성화함)
pub fn show_window_quietly(hwnd: isize) {
    let h = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
    }
}

/// 우리 창(hwnd)이 마우스를 통과시킬지 바꿈 (tao가 IGNORE_CURSOR_EVENT에 붙이는 것과 같은 WS_EX_TRANSPARENT | WS_EX_LAYERED).
/// tao의 set_ignore_cursor_events는 창 플래그를 통째로 다시 적용하며, tao가 "안 보임"으로 아는 창(show_window_quietly로 보인 창)을
/// SW_HIDE로 숨겨 버림 — 그래서 tao를 거치지 않음. 보임·위치·크기·Z 순서·포커스는 그대로. 실패하면 false
pub fn set_click_through(hwnd: isize, on: bool) -> bool {
    let h = HWND(hwnd as *mut c_void);
    unsafe {
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let bits = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;
        let next = if on { ex | bits } else { ex & !bits };
        if next != ex {
            SetWindowLongW(h, GWL_EXSTYLE, next as i32);
            let _ = SetWindowPos(h, HWND::default(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
        IsWindow(h).as_bool()
    }
}

/// 우리 창(hwnd)이 지금 보이는 상태임 (WS_VISIBLE이고 최소화 아님 — 화면 밖이어도 보이는 상태면 true).
/// 최소화된 창도 IsWindowVisible은 true라 따로 봄 (Win+D·Win+M에 최소화된 펫이 투명인간처럼 돌던 버그, v0.7.3)
pub fn is_window_shown(hwnd: isize) -> bool {
    let h = HWND(hwnd as *mut c_void);
    unsafe { IsWindowVisible(h).as_bool() && !IsIconic(h).as_bool() }
}

/// 우리 떠 있는 창(hwnd)을 도구 창으로 (WS_EX_TOOLWINDOW, WS_EX_APPWINDOW 뺌): 작업표시줄·Alt-Tab에 아예 안 나옴.
/// tao의 skip_taskbar는 DeleteTab뿐이라 조용히 숨겼다 보이면 작업표시줄 아이콘이 다시 생겼음
pub fn keep_off_taskbar(hwnd: isize) {
    let h = HWND(hwnd as *mut c_void);
    unsafe {
        let ex = GetWindowLongW(h, GWL_EXSTYLE) as u32;
        let next = (ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
        if next != ex {
            SetWindowLongW(h, GWL_EXSTYLE, next as i32);
            let _ = SetWindowPos(h, HWND::default(), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }
}

/// 우리 창(hwnd)을 숨김 — show_window_quietly와 짝 (tao가 기억하는 보임 상태와 상관없이 늘 숨김)
pub fn hide_window_quietly(hwnd: isize) {
    let h = HWND(hwnd as *mut c_void);
    unsafe {
        let _ = ShowWindow(h, SW_HIDE);
    }
}

/// 🌗 Windows 테마 (lumos.rs — 루모스·녹스로 화면 밝기 바꾸기): HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize의
/// AppsUseLightTheme·SystemUsesLightTheme (DWORD)를 읽고 쓰고, 바꾸면 모든 창에 WM_SETTINGCHANGE "ImmersiveColorSet"을 알림
pub struct Win32Theme;

/// 테마 값이 있는 레지스트리 키
const PERSONALIZE: windows::core::PCWSTR = windows::core::w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

/// DWORD 값 하나를 읽음 (없으면 None)
fn read_dword(name: windows::core::PCWSTR) -> Option<u32> {
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: value·len은 이 함수의 지역 변수이고 DWORD 하나만큼만 씀
    let err = unsafe { RegGetValueW(HKEY_CURRENT_USER, PERSONALIZE, name, RRF_RT_REG_DWORD, None, Some(&mut value as *mut u32 as *mut c_void), Some(&mut len)) };
    err.is_ok().then_some(value)
}

/// DWORD 값 하나를 씀 (됐으면 true)
fn write_dword(name: windows::core::PCWSTR, value: u32) -> bool {
    use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD};
    // SAFETY: value는 이 함수의 지역 변수이고 그 크기만큼만 읽힘
    let err = unsafe { RegSetKeyValueW(HKEY_CURRENT_USER, PERSONALIZE, name, REG_DWORD.0, Some(&value as *const u32 as *const c_void), size_of::<u32>() as u32) };
    err.is_ok()
}

/// 주 작업표시줄과 모든 보조 모니터 작업표시줄에 테마가 바뀌었다고 직접 알림 (창마다 최대 200 ms).
/// 모든 창에 차례로 보내는 알림은 멈춘 창마다 기다려서 다른 모니터 작업표시줄이 한참 늦게 바뀌었음 (2026-11-26)
fn notify_taskbars(area: windows::core::PCWSTR) {
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowExW, SendMessageTimeoutW, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
    for class in [windows::core::w!("Shell_TrayWnd"), windows::core::w!("Shell_SecondaryTrayWnd")] {
        let mut after = HWND::default();
        // 같은 이름의 창을 차례로 (보조 작업표시줄은 모니터마다 하나) — 무한히 돌지 않게 상한
        for _ in 0..16 {
            // SAFETY: 클래스 이름은 정적 넓은 문자열, after는 앞에서 찾은 창 (없으면 처음부터)
            let Ok(h) = (unsafe { FindWindowExW(HWND::default(), after, class, windows::core::PCWSTR::null()) }) else { break };
            if h.is_invalid() {
                break;
            }
            // SAFETY: area는 정적 넓은 문자열 — 알림 동안 살아 있음
            unsafe {
                SendMessageTimeoutW(h, WM_SETTINGCHANGE, WPARAM(0), LPARAM(area.as_ptr() as isize), SMTO_ABORTIFHUNG, 200, None);
            }
            after = h;
        }
    }
}

impl crate::lumos::ThemeBackend for Win32Theme {
    /// 지금 테마 (값이 없으면 라이트로 봄 — Windows 기본)
    fn read(&mut self) -> Option<crate::lumos::Theme> {
        let apps = read_dword(windows::core::w!("AppsUseLightTheme")).unwrap_or(1);
        let system = read_dword(windows::core::w!("SystemUsesLightTheme")).unwrap_or(1);
        Some(crate::lumos::Theme { apps_light: apps != 0, system_light: system != 0 })
    }

    /// 두 값을 쓰고 모든 모니터 작업표시줄, 그다음 모든 창에 테마가 바뀌었다고 알림 (멈춘 창은 기다리지 않음 — 창마다 최대 200 ms)
    fn write(&mut self, theme: crate::lumos::Theme) -> bool {
        use windows::Win32::UI::WindowsAndMessaging::{SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE};
        let ok = write_dword(windows::core::w!("AppsUseLightTheme"), u32::from(theme.apps_light)) & write_dword(windows::core::w!("SystemUsesLightTheme"), u32::from(theme.system_light));
        let area = windows::core::w!("ImmersiveColorSet");
        // 모든 모니터의 작업표시줄부터 (한 모니터만 바뀌고 나머지가 한참 늦던 문제)
        notify_taskbars(area);
        // SAFETY: area는 정적 넓은 문자열 — 알림 동안 살아 있음
        unsafe {
            SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, WPARAM(0), LPARAM(area.as_ptr() as isize), SMTO_ABORTIFHUNG, 200, None);
        }
        ok
    }
}

/// 💤 화면 잠금·풀기 (WM_WTSSESSION_CHANGE — WTSRegisterSessionNotification)와 절전에서 깸 (WM_POWERBROADCAST)을 세는 숨은 창.
/// 따로 스레드 하나가 보이지 않는 최상위 창을 만들고 메시지를 돌림 (알림은 최상위 창에만 옴). 처음 물을 때 띄움
mod session_watch {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Once;

    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage, MSG, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND,
        WINDOW_EX_STYLE, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_OVERLAPPED, WTS_SESSION_LOCK, WTS_SESSION_UNLOCK,
    };

    use crate::wake::SessionEvents;

    /// 잠금 수
    static LOCKS: AtomicU32 = AtomicU32::new(0);
    /// 풀기 수
    static UNLOCKS: AtomicU32 = AtomicU32::new(0);
    /// 깸 수
    static RESUMES: AtomicU32 = AtomicU32::new(0);
    /// 감시 스레드는 한 번만
    static START: Once = Once::new();

    /// 지금까지 센 수 (처음 부르면 감시 스레드를 띄움)
    pub fn events() -> SessionEvents {
        START.call_once(|| {
            let _ = std::thread::Builder::new().name("session-watch".into()).spawn(run);
        });
        SessionEvents { locks: LOCKS.load(Ordering::Relaxed), unlocks: UNLOCKS.load(Ordering::Relaxed), resumes: RESUMES.load(Ordering::Relaxed) }
    }

    /// 숨은 창을 만들고 알림을 받으며 메시지를 돌림 (앱이 끝날 때까지)
    fn run() {
        unsafe {
            let Ok(module) = GetModuleHandleW(None) else { return };
            let class = w!("ScreenPetSessionWatch");
            let wc = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: module.into(), lpszClassName: class, ..Default::default() };
            if RegisterClassW(&wc) == 0 {
                return;
            }
            // 보이지 않는 최상위 창 (ShowWindow를 부르지 않음 — 메시지 전용 창은 세션 알림을 못 받을 수 있음)
            let Ok(hwnd) = CreateWindowExW(WINDOW_EX_STYLE(0), class, w!(""), WS_OVERLAPPED, 0, 0, 0, 0, None, None, module, None) else { return };
            if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
                eprintln!("session-watch: could not register for lock/unlock notices");
            }
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// 숨은 창의 메시지 처리: 잠금·풀기·깸만 셈
    unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_WTSSESSION_CHANGE if wparam.0 as u32 == WTS_SESSION_LOCK => {
                LOCKS.fetch_add(1, Ordering::Relaxed);
            }
            WM_WTSSESSION_CHANGE if wparam.0 as u32 == WTS_SESSION_UNLOCK => {
                UNLOCKS.fetch_add(1, Ordering::Relaxed);
            }
            WM_POWERBROADCAST if matches!(wparam.0 as u32, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) => {
                RESUMES.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}
