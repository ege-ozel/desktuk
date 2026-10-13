//! Inline rename box: a small ordinary (non-layered) popup with an EDIT control, shown
//! over a circle. Enter or clicking elsewhere commits, Esc cancels.
//!
//! Layered windows painted with `UpdateLayeredWindow` cannot show child controls, which
//! is why this is a separate window.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, GetSysColorBrush, COLOR_WINDOW, LOGFONTW};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::EM_SETSEL;
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE, VK_RETURN};
use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::model::MAX_NAME;
use crate::win;

const CLASS: &str = "DeskTuk.Rename";
/// Posted to the popup; `wparam` 1 = commit, 0 = cancel.
const WM_APP_RENAME_DONE: u32 = WM_APP + 50;
const EM_LIMITTEXT: u32 = 0x00C5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameTarget {
    Circle(usize),
    Group(usize, usize),
}

thread_local! {
    /// (popup, edit, target) while a rename box is open.
    static ACTIVE: Cell<Option<(HWND, HWND, RenameTarget)>> = const { Cell::new(None) };
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(popup_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: unsafe { GetSysColorBrush(COLOR_WINDOW) },
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Opens a rename box centred on `anchor` (screen rect) with `initial` selected.
pub fn begin(target: RenameTarget, anchor: RECT, initial: &str) {
    cancel();
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let (w, h, pad) = (win::scale(220, dpi), win::scale(30, dpi), win::scale(4, dpi));
    let cx = (anchor.left + anchor.right) / 2;
    let cy = (anchor.top + anchor.bottom) / 2;
    let hinst = unsafe { GetModuleHandleW(std::ptr::null()) };
    unsafe {
        let popup = CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            win::wide(CLASS).as_ptr(),
            win::wide("Rename").as_ptr(),
            WS_POPUP | WS_BORDER,
            cx - w / 2,
            cy - h / 2,
            w,
            h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if popup.is_null() {
            return;
        }
        let mut cr = win::rect(0, 0, 0, 0);
        GetClientRect(popup, &mut cr);
        let edit = CreateWindowExW(
            0,
            win::wide("EDIT").as_ptr(),
            win::wide(initial).as_ptr(),
            WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
            pad,
            pad,
            cr.right - 2 * pad,
            cr.bottom - 2 * pad,
            popup,
            1 as _,
            hinst,
            std::ptr::null(),
        );
        if edit.is_null() {
            DestroyWindow(popup);
            return;
        }
        let mut lf: LOGFONTW = std::mem::zeroed();
        SystemParametersInfoForDpi(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of::<LOGFONTW>() as u32,
            (&mut lf as *mut LOGFONTW).cast(),
            0,
            dpi,
        );
        let font = CreateFontIndirectW(&lf);
        SetWindowLongPtrW(popup, GWLP_USERDATA, font as isize); // deleted in WM_DESTROY
        SendMessageW(edit, WM_SETFONT, font as usize, 0);
        SendMessageW(edit, EM_LIMITTEXT, MAX_NAME, 0);
        SetWindowSubclass(edit, Some(edit_proc), 1, popup as usize);
        ACTIVE.set(Some((popup, edit, target)));
        ShowWindow(popup, SW_SHOW);
        SetForegroundWindow(popup);
        SetFocus(edit);
        SendMessageW(edit, EM_SETSEL, 0, -1);
    }
}

/// Closes an open rename box without committing.
pub fn cancel() {
    if let Some((popup, _, _)) = ACTIVE.take() {
        unsafe { DestroyWindow(popup) };
    }
}

fn window_text(h: HWND) -> String {
    unsafe {
        let n = GetWindowTextLengthW(h).max(0) as usize;
        let mut buf = vec![0u16; n + 1];
        let got = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buf[..got])
    }
}

fn finish(popup: HWND, commit: bool) {
    let Some((p, edit, target)) = ACTIVE.get() else { return };
    if p != popup {
        return;
    }
    ACTIVE.set(None);
    let text = window_text(edit);
    unsafe { DestroyWindow(popup) };
    if commit {
        super::commit_rename(target, &text);
    }
}

unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, popup: usize) -> LRESULT {
    match msg {
        WM_KEYDOWN if wparam == VK_RETURN as usize => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 1, 0);
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 0, 0);
            0
        }
        // Swallow the characters of Enter/Esc so the edit control does not beep.
        WM_CHAR if wparam == 0x0d || wparam == 0x1b => 0,
        WM_KILLFOCUS => {
            PostMessageW(popup as HWND, WM_APP_RENAME_DONE, 1, 0);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        WM_NCDESTROY => {
            RemoveWindowSubclass(hwnd, Some(edit_proc), 1);
            DefSubclassProc(hwnd, msg, wparam, lparam)
        }
        _ => DefSubclassProc(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn popup_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_APP_RENAME_DONE => {
            finish(hwnd, wparam == 1);
            0
        }
        WM_DESTROY => {
            let font = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
            if font != 0 {
                DeleteObject(font as _);
            }
            if ACTIVE.get().map(|(p, _, _)| p == hwnd).unwrap_or(false) {
                ACTIVE.set(None);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
