//! Closed circles: small round layered tool windows that sit on the desktop.
//!
//! Hover (or click) opens the ring, dragging moves the circle, right-click opens the
//! circle menu, and files dropped from Explorer are added to the circle.

use std::cell::Cell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{WM_MOUSEHOVER, WM_MOUSELEAVE};
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_HOVER, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::surface::Surface;
use super::view::{self, Target};
use super::{draw, geom, ring};
use crate::app::app;
use crate::host::{self, DesktopHost, HostKind};
use crate::{dlog, win};

const CLASS: &str = "DeskTuk.Circle";
/// How long the pointer must rest on a circle before its ring opens.
const HOVER_MS: u32 = 250;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Recent,
    /// Index into `Board::circles`.
    User(usize),
}

impl Which {
    pub fn target(self) -> Target {
        match self {
            Which::Recent => Target::Recent,
            Which::User(ci) => Target::Circle(ci),
        }
    }
}

pub struct CircleWin {
    pub hwnd: HWND,
    /// Each circle has its own host: `WindowHost` keeps per-window "raised" state.
    pub host: Rc<dyn DesktopHost>,
    pub which: Which,
}

#[derive(Clone, Copy)]
struct Press {
    hwnd: HWND,
    cursor: POINT,
    origin: (i32, i32),
    moving: bool,
    ring_click: bool,
}

thread_local! {
    static PRESS: Cell<Option<Press>> = const { Cell::new(None) };
    /// The circle window that currently has `TrackMouseEvent` hover/leave tracking.
    static TRACKING: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(circle_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_HAND) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Creates the (still invisible) window for `which`; `render` sizes, paints and shows it.
pub fn create(which: Which, pos: (i32, i32)) -> Option<CircleWin> {
    let class = win::wide(CLASS);
    let title = win::wide("DeskTuk Circle");
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            pos.0,
            pos.1,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        dlog!("circle window creation failed: error {}", unsafe { windows_sys::Win32::Foundation::GetLastError() });
        return None;
    }
    unsafe { DragAcceptFiles(hwnd, 1) };
    let host = host::create(HostKind::Window);
    host.attached(hwnd);
    Some(CircleWin { hwnd, host, which })
}

/// Paints `which` into `hwnd` (name + item count), moves it to its saved position
/// (clamped to the nearest work area) and shows it. Uses a temporary surface: the
/// system keeps the presented pixels, so nothing stays allocated here.
pub fn render(hwnd: HWND, which: Which) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let m = geom::metrics(dpi);
    let recent_count = if which == Which::Recent { super::recent_paths().len() } else { 0 };
    let (name, count, pos) = {
        let Ok(b) = app().board.try_borrow() else { return };
        match which {
            Which::Recent => ("Recent".to_string(), recent_count, b.recent_pos.unwrap_or_else(super::default_recent_pos)),
            Which::User(ci) => match b.circles.get(ci) {
                Some(c) => (c.name.clone(), view::circle_count(&b, ci), c.pos),
                None => return,
            },
        }
    };
    let Some(mut s) = Surface::new(m.circle_d, m.circle_d) else { return };
    draw::circle(&s, dpi, &name, count, which == Which::Recent);
    let d = m.circle_d as usize;
    geom::mask_disc(s.pixels(), d, d, m.circle_d as f32 / 2.0);
    let (x, y) = clamp_to_work(pos, m.circle_d);
    s.present(hwnd, POINT { x, y });
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
}

/// Keeps a `d`x`d` window at `pos` fully inside the nearest monitor's work area.
pub fn clamp_to_work(pos: (i32, i32), d: i32) -> (i32, i32) {
    let r = win::rect(pos.0, pos.1, pos.0 + d, pos.1 + d);
    unsafe {
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return pos;
        }
        let w = mi.rcWork;
        (pos.0.min(w.right - d).max(w.left), pos.1.min(w.bottom - d).max(w.top))
    }
}

fn cursor_pos() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

/// Paths from a `WM_DROPFILES` handle (which is released).
pub fn dropped_paths(h: HDROP) -> Vec<PathBuf> {
    let mut out = Vec::new();
    unsafe {
        let n = DragQueryFileW(h, u32::MAX, std::ptr::null_mut(), 0);
        for i in 0..n {
            let len = DragQueryFileW(h, i, std::ptr::null_mut(), 0) as usize;
            let mut buf = vec![0u16; len + 1];
            let got = DragQueryFileW(h, i, buf.as_mut_ptr(), buf.len() as u32) as usize;
            if got > 0 {
                out.push(PathBuf::from(OsString::from_wide(&buf[..got])));
            }
        }
        DragFinish(h);
    }
    out
}

/// Mouse coordinates belong to the queued event, not the current cursor position.
pub fn mouse_point(hwnd: HWND, lparam: LPARAM) -> POINT {
    let mut at = POINT { x: win::x_lparam(lparam), y: win::y_lparam(lparam) };
    if unsafe { ClientToScreen(hwnd, &mut at) } == 0 {
        return cursor_pos();
    }
    at
}

pub fn is_pressed(hwnd: HWND) -> bool {
    PRESS.get().map(|p| p.hwnd == hwnd).unwrap_or(false)
}

/// A press on the open ring's centre belongs to its circle, just like a closed press.
pub fn begin_ring_drag(hwnd: HWND, at: POINT) {
    begin_press(hwnd, at, true);
}

fn begin_press(hwnd: HWND, at: POINT, ring_click: bool) {
    if let Some(old) = PRESS.take() {
        super::drag_capture::end(old.hwnd);
    }
    let r = win::window_rect(hwnd);
    PRESS.set(Some(Press { hwnd, cursor: at, origin: (r.left, r.top), moving: false, ring_click }));
    let captured = super::drag_capture::begin(hwnd);
    dlog!("circle press hwnd={hwnd:?} from_ring={ring_click} captured={captured}");
    if !captured {
        dlog!("circle drag foreground capture unavailable");
    }
}

pub fn move_press(hwnd: HWND, c: POINT) {
    if let Some(mut p) = PRESS.get().filter(|p| p.hwnd == hwnd) {
        let (dx, dy) = (c.x - p.cursor.x, c.y - p.cursor.y);
        let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
        let beyond = unsafe { dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi) };
        let started = !p.moving && beyond;
        if started {
            p.moving = true;
        }
        PRESS.set(Some(p));
        if started {
            ring::close_now();
        }
        if p.moving && is_pressed(hwnd) {
            unsafe {
                SetWindowPos(hwnd, std::ptr::null_mut(), p.origin.0 + dx, p.origin.1 + dy, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
            };
        }
    }
}

fn on_move(hwnd: HWND, lparam: LPARAM) {
    if is_pressed(hwnd) {
        move_press(hwnd, mouse_point(hwnd, lparam));
        return;
    }
    if TRACKING.get() != hwnd {
        let mut tme = TRACKMOUSEEVENT {
            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_HOVER | TME_LEAVE,
            hwndTrack: hwnd,
            dwHoverTime: HOVER_MS,
        };
        unsafe { TrackMouseEvent(&mut tme) };
        TRACKING.set(hwnd);
    }
}

/// Ends a left-button press. `released`: the button went up (a click opens the ring);
/// otherwise capture was lost and nothing is opened.
pub fn end_press(hwnd: HWND, released: bool) {
    if !is_pressed(hwnd) {
        return;
    }
    let Some(p) = PRESS.take() else { return };
    super::drag_capture::end(hwnd); // PRESS is already empty before message-pumping calls
    let Some(which) = super::which_of(hwnd) else { return };
    if p.moving {
        let r = win::window_rect(hwnd);
        super::moved(which, (r.left, r.top));
    } else if released && p.ring_click {
        ring::center_clicked(hwnd);
    } else if released {
        ring::open(hwnd, which.target());
    }
}

unsafe extern "system" fn circle_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Never take activation: a circle must not jump above application windows.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_WINDOWPOSCHANGING => {
            // A drag needs foreground capture. Pinning activation's position request to
            // HWND_BOTTOM prevents activation; the capture helper restores it at release.
            if !super::drag_capture::is_active() {
                if let Some(h) = super::host_of(hwnd) {
                    h.window_pos_changing(hwnd, &mut *(lparam as *mut WINDOWPOS));
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            on_move(hwnd, lparam);
            0
        }
        WM_MOUSEHOVER => {
            TRACKING.set(std::ptr::null_mut());
            if PRESS.get().is_none() && !ring::is_open_for(hwnd) {
                if let Some(w) = super::which_of(hwnd) {
                    ring::open(hwnd, w.target());
                }
            }
            0
        }
        WM_MOUSELEAVE => {
            if TRACKING.get() == hwnd {
                TRACKING.set(std::ptr::null_mut());
            }
            0
        }
        WM_LBUTTONDOWN => {
            super::rename::commit_active();
            begin_press(hwnd, mouse_point(hwnd, lparam), false);
            0
        }
        WM_LBUTTONUP => {
            end_press(hwnd, true);
            0
        }
        WM_CAPTURECHANGED => {
            if lparam as HWND != hwnd {
                end_press(hwnd, false);
            }
            0
        }
        WM_RBUTTONUP => {
            super::rename::commit_active();
            if let Some(w) = super::which_of(hwnd) {
                super::circle_menu(w, cursor_pos());
            }
            0
        }
        WM_DROPFILES => {
            let paths = dropped_paths(wparam as HDROP);
            if let Some(w) = super::which_of(hwnd) {
                dlog!("dropped {} file(s) on {w:?}", paths.len());
                super::assign(&paths, view::drop_place(w.target(), None));
            }
            0
        }
        WM_DPICHANGED => {
            if let Some(w) = super::which_of(hwnd) {
                render(hwnd, w);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
