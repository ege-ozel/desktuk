//! A transient, click-through icon beside the pointer during an internal item drag.
//! Window calls run outside RING borrows. No timer, shell query or retained bitmap.

use std::cell::Cell;
use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::surface::Surface;
use crate::win;

const CLASS: &str = "DeskTuk.DragPreview";
thread_local! {
    static WINDOW: Cell<Option<(HWND, i32)>> = const { Cell::new(None) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSW {
        lpfnWndProc: Some(DefWindowProcW),
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        lpszClassName: name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassW(&wc) };
}

pub fn show(surface: Surface, owner: HWND, at: POINT, dpi: u32) {
    close();
    let offset = win::scale(12, dpi);
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            win::wide(CLASS).as_ptr(),
            win::wide("Dragged item").as_ptr(),
            WS_POPUP,
            at.x + offset,
            at.y + offset,
            surface.w,
            surface.h,
            owner,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    if !surface.present(hwnd, POINT { x: at.x + offset, y: at.y + offset }) {
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    WINDOW.set(Some((hwnd, offset)));
    unsafe { SetWindowPos(hwnd, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW) };
}

pub fn move_to(at: POINT) {
    if let Some((hwnd, offset)) = WINDOW.get() {
        unsafe { SetWindowPos(hwnd, std::ptr::null_mut(), at.x + offset, at.y + offset, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE) };
    }
}

pub fn close() {
    if let Some((hwnd, _)) = WINDOW.take() {
        if unsafe { IsWindow(hwnd) } != 0 {
            unsafe { DestroyWindow(hwnd) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_follows_pointer_without_activation_and_closes_cleanly() {
        register_class();
        let foreground = unsafe { GetForegroundWindow() };
        let mut surface = super::super::draw::drag_icon(std::ptr::null_mut(), 96).expect("fixture surface");
        assert!(surface.pixels().iter().any(|p| p >> 24 > 0), "fallback must be visible");
        show(surface, std::ptr::null_mut(), POINT { x: -30000, y: -30000 }, 96);
        let (hwnd, offset) = WINDOW.get().expect("preview window");
        let style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
        assert_eq!(style & (WS_EX_TRANSPARENT | WS_EX_NOACTIVATE), WS_EX_TRANSPARENT | WS_EX_NOACTIVATE);
        assert_eq!(style & WS_EX_TOPMOST, 0);
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
        move_to(POINT { x: -29900, y: -29800 });
        let rect = win::window_rect(hwnd);
        assert_eq!((rect.left, rect.top), (-29900 + offset, -29800 + offset));
        close();
        assert_eq!(unsafe { IsWindow(hwnd) }, 0);
        assert!(WINDOW.get().is_none());
        close();
    }
}
