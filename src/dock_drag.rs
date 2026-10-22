//! Temporary, coalesced screen-pointer routing for the no-activate DeskTuk dock.
use crate::dock;
use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WM_POINTER: u32 = WM_APP + 44;
#[derive(Clone, Copy)]
struct Press {
    hwnd: HWND,
    cursor: POINT,
    origin: (i32, i32),
    previous: Option<(i32, i32)>,
    previous_top: Option<i32>,
    previous_edge: Option<crate::dock_layout::Edge>,
    moving: bool,
}
thread_local! {
    static PRESS: Cell<Option<Press>> = const { Cell::new(None) };
    static HOOK: Cell<HHOOK> = const { Cell::new(std::ptr::null_mut()) };
    static POINTER: Cell<Option<(POINT,bool)>> = const { Cell::new(None) };
    static ROUTED: Cell<bool> = const { Cell::new(false) };
    static ID: Cell<usize> = const { Cell::new(0) };
}

pub fn active() -> bool {
    PRESS.get().is_some()
}
pub fn prepare_click() {
    if !active() {
        ROUTED.set(false);
    }
}
pub fn begin(hwnd: HWND, at: POINT, origin: (i32, i32), previous: Option<(i32, i32)>) {
    shutdown();
    ID.set(ID.get().wrapping_add(1));
    PRESS.set(Some(Press {
        hwnd,
        cursor: at,
        origin,
        previous,
        previous_top: dock::current_top_anchor(),
        previous_edge: dock::current_edge(),
        moving: false,
    }));
    let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), GetModuleHandleW(std::ptr::null()), 0) };
    HOOK.set(hook);
    if hook.is_null() {
        crate::dlog!("dock pointer hook unavailable");
    }
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 && matches!(message as u32, WM_MOUSEMOVE | WM_LBUTTONUP) {
        if let Some(p) = PRESS.get() {
            if !POINTER.get().is_some_and(|(_, released)| released) {
                let point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
                ROUTED.set(true);
                if POINTER.replace(Some((point, message as u32 == WM_LBUTTONUP))).is_none()
                    && unsafe { PostMessageW(p.hwnd, WM_POINTER, ID.get(), 0) } == 0
                {
                    POINTER.set(None);
                    ROUTED.set(false);
                }
            }
        }
    }
    unsafe { CallNextHookEx(HOOK.get(), code, message, data) }
}

fn move_to(at: POINT) {
    let Some(mut p) = PRESS.get() else { return };
    let (dx, dy) = (at.x - p.cursor.x, at.y - p.cursor.y);
    let dpi = unsafe { GetDpiForWindow(p.hwnd) }.max(96);
    if !p.moving {
        p.moving = unsafe { dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi) };
        PRESS.set(Some(p));
    }
    if p.moving {
        dock::move_anchor(p.hwnd, Some((p.origin.0 + dx, p.origin.1 + dy)));
    }
}

fn finish(released: bool) -> bool {
    let Some(p) = PRESS.take() else { return false };
    POINTER.set(None);
    let hook = HOOK.replace(std::ptr::null_mut());
    if !hook.is_null() {
        unsafe { UnhookWindowsHookEx(hook) };
    }
    if p.moving {
        if released {
            dock::save_position();
        } else {
            dock::restore_anchor(p.hwnd, p.previous, p.previous_top, p.previous_edge);
        }
        dock::after_drag();
    }
    crate::circles::interaction_ended();
    p.moving
}

pub fn move_native(at: POINT) {
    if !ROUTED.get() {
        move_to(at);
    }
}
pub fn release_native(at: POINT) -> bool {
    if ROUTED.get() {
        return true;
    }
    if !active() {
        return false;
    }
    move_to(at);
    finish(true)
}
pub fn dispatch(id: usize) {
    if id != ID.get() {
        return;
    }
    if let Some((point, released)) = POINTER.take() {
        let hwnd = PRESS.get().map(|p| p.hwnd);
        move_to(point);
        if released && !finish(true) {
            if let Some(hwnd) = hwnd {
                dock::click_screen(hwnd, point);
            }
        }
    }
}
pub fn cancel() {
    finish(false);
    ROUTED.set(false);
}
pub fn shutdown() {
    PRESS.set(None);
    POINTER.set(None);
    ROUTED.set(false);
    let hook = HOOK.replace(std::ptr::null_mut());
    if !hook.is_null() {
        unsafe { UnhookWindowsHookEx(hook) };
    }
}
