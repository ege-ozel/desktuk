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
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_ESCAPE, VK_LBUTTON, VK_MBUTTON};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::surface::Surface;
use super::view::{self, Target};
use super::{draw, geom, ring};
use crate::app::app;
use crate::host::{self, DesktopHost, HostKind};
use crate::{dlog, win};

const CLASS: &str = "DeskTuk.Circle";
const WM_DRAG_POINTER: u32 = WM_APP + 42;

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
    button: u16,
}

thread_local! {
    static PRESS: Cell<Option<Press>> = const { Cell::new(None) };
    static POINTER: Cell<Option<(POINT, bool)>> = const { Cell::new(None) };
    static POINTER_ROUTE: Cell<bool> = const { Cell::new(false) };
    static PRESS_ID: Cell<usize> = const { Cell::new(0) };
}

pub fn register_class() {
    super::grid_overlay::register_class();
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
    let always_on_top = app().board.try_borrow().map(|b| b.always_on_top).unwrap_or(true);
    host.set_always_on_top(hwnd, always_on_top);
    host.attached(hwnd);
    Some(CircleWin { hwnd, host, which })
}

/// Paints `which` into `hwnd` (name + item count), moves it to its saved position
/// (clamped to the nearest work area) and shows it. Uses a temporary surface: the
/// system keeps the presented pixels, so nothing stays allocated here.
pub fn render(hwnd: HWND, which: Which) {
    if is_pressed(hwnd) {
        super::defer_refresh();
        return;
    }
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
    draw::circle(&mut s, dpi, &name, count, crate::dock::locked());
    let d = m.circle_d as usize;
    geom::mask_disc(s.pixels(), d, d, m.circle_d as f32 / 2.0);
    let (x, y) = clamp_to_work(pos, m.circle_d);
    s.present(hwnd, POINT { x, y });
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
}

/// Keeps a `d`x`d` window at `pos` fully inside the nearest monitor's work area.
pub fn clamp_to_work(pos: (i32, i32), d: i32) -> (i32, i32) {
    snap_to_work(pos, d, 0)
}

/// Clamp or snap using this monitor's actual work area, independent of taskbar edge.
pub fn snap_to_work(pos: (i32, i32), d: i32, divisions: u8) -> (i32, i32) {
    work_area(pos, d).map(|w| super::grid::position(pos, d, w, divisions)).unwrap_or(pos)
}

fn work_area(pos: (i32, i32), d: i32) -> Option<(i32, i32, i32, i32)> {
    let r = win::rect(pos.0, pos.1, pos.0 + d, pos.1 + d);
    unsafe {
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) == 0 {
            return None;
        }
        let w = mi.rcWork;
        Some((w.left, w.top, w.right, w.bottom))
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

pub fn pressed_window() -> Option<HWND> {
    PRESS.get().map(|p| p.hwnd)
}

pub fn pressed_mouse_route() -> Option<(HWND, usize, u32)> {
    PRESS.get().map(|p| if p.button == VK_MBUTTON { (p.hwnd, 0x0010, WM_MBUTTONUP) } else { (p.hwnd, 0x0001, WM_LBUTTONUP) })
}

/// The hook stores screen coordinates, never client coordinates of a moving window.
/// Keep only the newest sample so a fast mouse cannot build a movement backlog.
pub fn queue_pointer(hwnd: HWND, point: POINT, released: bool) {
    if POINTER.get().is_some_and(|(_, released)| released) {
        return;
    }
    POINTER_ROUTE.set(true);
    let previous = POINTER.replace(Some((point, released)));
    if previous.is_none() {
        let posted = unsafe { PostMessageW(hwnd, WM_DRAG_POINTER, PRESS_ID.get(), 0) };
        if posted == 0 {
            POINTER.set(None);
            POINTER_ROUTE.set(false);
        }
    }
}

fn apply_pointer(hwnd: HWND) {
    if let Some((point, released)) = POINTER.take() {
        move_press_screen(hwnd, point);
        if released {
            end_press(hwnd, true);
        }
    }
}

pub fn native_release(hwnd: HWND) {
    if !POINTER_ROUTE.get() {
        end_press(hwnd, true);
    }
}

pub fn is_release_message(hwnd: HWND, msg: u32) -> bool {
    pressed_mouse_route().is_some_and(|(target, _, release)| target == hwnd && release == msg)
}

/// A press on the open ring's centre belongs to its circle, just like a closed press.
pub fn begin_ring_drag(hwnd: HWND, at: POINT, center: POINT, button: u16) {
    let d = geom::metrics(unsafe { GetDpiForWindow(hwnd) }.max(96)).circle_d;
    begin_press_from(hwnd, at, button, Some((center.x - d / 2, center.y - d / 2)));
}

fn begin_press(hwnd: HWND, at: POINT, button: u16) {
    begin_press_from(hwnd, at, button, None);
}

fn begin_press_from(hwnd: HWND, at: POINT, button: u16, origin: Option<(i32, i32)>) {
    super::grid_overlay::close();
    if let Some(old) = PRESS.take() {
        super::drag_capture::end(old.hwnd);
    }
    POINTER.set(None);
    POINTER_ROUTE.set(false);
    PRESS_ID.set(PRESS_ID.get().wrapping_add(1));
    let r = win::window_rect(hwnd);
    PRESS.set(Some(Press { hwnd, cursor: at, origin: origin.unwrap_or((r.left, r.top)), moving: false, button }));
    ring::circle_drag_started();
    let captured = super::drag_capture::begin_with_button(hwnd, button);
    dlog!("circle press hwnd={hwnd:?} button={button:#x} captured={captured}");
    if !captured {
        dlog!("circle drag foreground capture unavailable");
    }
    if is_pressed(hwnd) {
        update_grid(hwnd);
    }
}

fn update_grid(hwnd: HWND) {
    let divisions = app().board.try_borrow().map(|b| b.grid).unwrap_or(0);
    let r = win::window_rect(hwnd);
    let d = geom::metrics(unsafe { GetDpiForWindow(hwnd) }.max(96)).circle_d;
    if let Some(work) = work_area((r.left, r.top), d) {
        super::grid_overlay::update(hwnd, (r.left, r.top), d, work, divisions);
    }
}

pub fn move_press(hwnd: HWND, c: POINT) {
    if !POINTER_ROUTE.get() {
        move_press_screen(hwnd, c);
    }
}

fn move_press_screen(hwnd: HWND, c: POINT) {
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
            // Move freely while dragging. The selected grid cell is shown as a guide;
            // the circle snaps only when the button is released.
            let raw = (p.origin.0 + dx, p.origin.1 + dy);
            let d = geom::metrics(dpi).circle_d;
            let position = clamp_to_work(raw, d);
            unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), position.0, position.1, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
            if is_pressed(hwnd) {
                update_grid(hwnd);
            }
        }
    }
}

fn on_move(hwnd: HWND, lparam: LPARAM) {
    if is_pressed(hwnd) {
        move_press(hwnd, mouse_point(hwnd, lparam));
        return;
    }
    if !ring::is_open_for(hwnd) {
        if let Some(w) = super::which_of(hwnd) {
            ring::open(hwnd, w.target());
        }
    }
}

/// Ends a mouse press. `released`: the button went up (a left click opens the ring);
/// otherwise capture was lost and nothing is opened.
pub fn end_press(hwnd: HWND, released: bool) {
    if !is_pressed(hwnd) {
        return;
    }
    let Some(p) = PRESS.take() else { return };
    POINTER.set(None);
    POINTER_ROUTE.set(false);
    super::grid_overlay::close();
    super::drag_capture::end(hwnd); // PRESS is already empty before message-pumping calls
    ring::circle_drag_finished();
    let Some(which) = super::which_of(hwnd) else { return };
    if p.moving {
        let r = win::window_rect(hwnd);
        super::moved(which, (r.left, r.top));
    } else if released && p.button == VK_LBUTTON && !ring::center_click(hwnd) {
        ring::open(hwnd, which.target());
    }
    super::interaction_ended();
}

unsafe extern "system" fn circle_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        // Showing or interacting with circles must not take keyboard focus.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_SYSCOMMAND => {
            let blocked = super::host_of(hwnd).is_some_and(|h| h.system_command(wparam));
            if blocked {
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
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
        WM_DRAG_POINTER => {
            if wparam == PRESS_ID.get() && is_pressed(hwnd) {
                apply_pointer(hwnd);
            }
            0
        }
        WM_MOUSEMOVE if is_pressed(hwnd) && POINTER_ROUTE.get() => 0,
        WM_MOUSEMOVE => {
            // Repaint/hover messages without the active button must not advance a drag.
            let pressed = pressed_mouse_route().filter(|(target, _, _)| *target == hwnd);
            let has_button = pressed.is_some_and(|(_, key, _)| wparam & key != 0);
            if (pressed.is_some() && has_button) || (pressed.is_none() && wparam & 1 == 0) {
                on_move(hwnd, lparam);
            }
            0
        }
        WM_LBUTTONDOWN => {
            super::rename::commit_active();
            begin_press(hwnd, mouse_point(hwnd, lparam), VK_LBUTTON);
            0
        }
        WM_MBUTTONDOWN => {
            super::rename::commit_active();
            begin_press(hwnd, mouse_point(hwnd, lparam), VK_MBUTTON);
            0
        }
        WM_LBUTTONUP => {
            if is_release_message(hwnd, WM_LBUTTONUP) && !POINTER_ROUTE.get() {
                end_press(hwnd, true);
            }
            0
        }
        WM_MBUTTONUP => {
            if is_release_message(hwnd, WM_MBUTTONUP) && !POINTER_ROUTE.get() {
                end_press(hwnd, true);
            }
            0
        }
        WM_CAPTURECHANGED => {
            if lparam as HWND != hwnd {
                end_press(hwnd, false);
            }
            0
        }
        WM_CANCELMODE => {
            end_press(hwnd, false);
            0
        }
        WM_DESTROY => {
            if is_pressed(hwnd) {
                PRESS.take();
                POINTER.set(None);
                POINTER_ROUTE.set(false);
                super::grid_overlay::close();
                super::drag_capture::end(hwnd);
            }
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            end_press(hwnd, false);
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

#[cfg(test)]
mod pointer_tests {
    use super::*;

    #[test]
    fn fast_fallback_samples_coalesce_in_screen_space_and_release_is_final() {
        // A real message-only sink lets PostMessage exercise the production queue,
        // without injecting physical input or touching the application board.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                crate::win::wide("STATIC").as_ptr(),
                std::ptr::null(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null());
        POINTER.set(None);
        for i in 0..1000 {
            queue_pointer(hwnd, POINT { x: -1200 + i, y: 300 + i }, false);
        }
        let (point, released) = POINTER.get().expect("latest sample");
        assert_eq!((point.x, point.y, released), (-201, 1299, false));
        queue_pointer(hwnd, POINT { x: -50, y: 1400 }, true);
        queue_pointer(hwnd, POINT { x: 800, y: 20 }, false);
        let (point, released) = POINTER.take().expect("release sample");
        assert_eq!((point.x, point.y, released), (-50, 1400, true));
        let mut message: MSG = unsafe { std::mem::zeroed() };
        assert_ne!(unsafe { PeekMessageW(&mut message, hwnd, WM_DRAG_POINTER, WM_DRAG_POINTER, PM_REMOVE) }, 0);
        assert_eq!(
            unsafe { PeekMessageW(&mut message, hwnd, WM_DRAG_POINTER, WM_DRAG_POINTER, PM_REMOVE) },
            0,
            "one notification for the entire burst"
        );
        POINTER_ROUTE.set(false);
        unsafe { DestroyWindow(hwnd) };
    }
}
