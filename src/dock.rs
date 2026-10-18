//! Small, persistent recovery dock for the circles UI.
//!
//! Windows' Show Desktop commands can hide desktop-organiser windows. The dock stays
//! in the topmost band and gives the user a direct way to reveal or hide the circles.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect, ScreenToClient, SetBkMode, SetTextColor,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::circles::circle_win::Which;
use crate::{app, circles, win};

const CLASS: &str = "DeskTuk.Dock";
const BG: u32 = win::rgb(20, 20, 24);
const ACCENT: u32 = win::rgb(242, 38, 137);
const TEXT: u32 = win::rgb(255, 255, 255);
const SUBTEXT: u32 = win::rgb(205, 170, 190);
const ROW_HOVER: u32 = win::rgb(52, 33, 46);
const MAX_ROWS: usize = 7;
const HEADER_H: i32 = 40;
const ACTION_H: i32 = 38;
const ROW_H: i32 = 30;
const FOOTER_H: i32 = 18;

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static SCROLL: Cell<usize> = const { Cell::new(0) };
}

fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

fn rows() -> Vec<(Which, String)> {
    let Ok(board) = app::app().board.try_borrow() else { return vec![(Which::Recent, "Recent".to_string())] };
    let mut rows = Vec::with_capacity(board.circles.len() + 1);
    rows.push((Which::Recent, "Recent".to_string()));
    rows.extend(board.circles.iter().enumerate().map(|(i, circle)| (Which::User(i), circle.name.clone())));
    rows
}

fn dimensions(dpi: u32, count: usize) -> (i32, i32) {
    let visible = visible_row_count(dpi, count) as i32;
    let footer = if count > visible as usize { FOOTER_H } else { 0 };
    (scale(238, dpi), scale(HEADER_H + ACTION_H + visible * ROW_H + 10 + footer, dpi))
}

fn visible_row_count(dpi: u32, count: usize) -> usize {
    let work = work_area();
    let row = scale(ROW_H, dpi).max(1);
    let fixed = scale(HEADER_H + ACTION_H + 10, dpi);
    let available = (work.bottom - work.top - fixed).max(0);
    let fit = (available / row) as usize;
    let max = count.min(MAX_ROWS).min(fit.max(1));
    if count > max {
        let with_footer = ((available - scale(FOOTER_H, dpi)).max(0) / row) as usize;
        count.min(MAX_ROWS).min(with_footer.max(1))
    } else {
        max
    }
}

fn work_area() -> RECT {
    let mut work = win::rect(0, 0, 1024, 768);
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
    }
    work
}

fn place(hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let (width, height) = dimensions(dpi, rows().len());
    let work = work_area();
    let gap = scale(12, dpi);
    let x = (work.right - width - gap).max(work.left);
    let y = (work.top + (work.bottom - work.top - height) / 2).max(work.top);
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

fn preserve_dock(position: &mut WINDOWPOS) {
    position.flags = (position.flags & !SWP_HIDEWINDOW) | SWP_SHOWWINDOW;
    position.flags &= !SWP_NOZORDER;
    position.hwndInsertAfter = HWND_TOPMOST;
}

pub fn create() {
    let name = win::wide(CLASS);
    let class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(wndproc),
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
    unsafe { RegisterClassExW(&class) };

    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() }.max(96);
    let (width, height) = dimensions(dpi, rows().len());
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
            name.as_ptr(),
            win::wide("DeskTuk").as_ptr(),
            WS_POPUP,
            0,
            0,
            width,
            height,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    WINDOW.with(|w| w.set(hwnd));
    let corner: i32 = 2;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&corner as *const i32).cast(),
            std::mem::size_of_val(&corner) as u32,
        );
    }
    crate::host::exclude_from_peek(hwnd);
    place(hwnd);
}

pub fn show() {
    WINDOW.with(|window| {
        let hwnd = window.get();
        if !hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0 {
            place(hwnd);
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
        }
    });
}

pub fn refresh() {
    WINDOW.with(|window| {
        let hwnd = window.get();
        if !hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0 {
            let count = rows().len();
            let visible = visible_row_count(unsafe { GetDpiForWindow(hwnd) }.max(96), count);
            SCROLL.with(|scroll| scroll.set(scroll.get().min(count.saturating_sub(visible))));
            place(hwnd);
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
        }
    });
}

pub fn shutdown() {
    WINDOW.with(|window| {
        let hwnd = window.replace(std::ptr::null_mut());
        if !hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0 {
            unsafe { DestroyWindow(hwnd) };
        }
    });
}

fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let entries = rows();
    let count = entries.len();
    let visible_count = visible_row_count(dpi, count);
    let scroll = SCROLL.with(Cell::get).min(count.saturating_sub(visible_count));
    let row_h = scale(ROW_H, dpi);
    let header_h = scale(HEADER_H, dpi);
    let action_h = scale(ACTION_H, dpi);
    let bg = unsafe { CreateSolidBrush(BG) };
    let accent = unsafe { CreateSolidBrush(ACCENT) };
    let hover = unsafe { CreateSolidBrush(ROW_HOVER) };
    let stripe = win::rect(0, 0, scale(3, dpi), client.bottom);
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe {
        FillRect(hdc, &client, bg);
        FillRect(hdc, &stripe, accent);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, TEXT);
        let mut title = win::wide("DeskTuk");
        let mut title_rect = win::rect(scale(14, dpi), scale(3, dpi), client.right - scale(8, dpi), header_h - scale(3, dpi));
        DrawTextW(hdc, title.as_mut_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        let action_top = header_h;
        let action_bottom = action_top + action_h;
        let action_rect = win::rect(scale(8, dpi), action_top, client.right - scale(8, dpi), action_bottom);
        let action_brush = CreateSolidBrush(ROW_HOVER);
        FillRect(hdc, &action_rect, action_brush);
        DeleteObject(action_brush);
        SetTextColor(hdc, ACCENT);
        let mut action = win::wide(if circles::any_visible() { "Daireleri gizle" } else { "Daireleri göster" });
        let mut action_text = win::rect(scale(16, dpi), action_top, client.right - scale(8, dpi), action_bottom);
        DrawTextW(hdc, action.as_mut_ptr(), -1, &mut action_text, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SetTextColor(hdc, SUBTEXT);
        for visible_index in 0..visible_count {
            let index = scroll + visible_index;
            let top = action_bottom + scale(4, dpi) + visible_index as i32 * row_h;
            let row = win::rect(scale(8, dpi), top, client.right - scale(8, dpi), top + row_h);
            if GetCursorPos(&mut cursor) != 0 {
                let mut local = cursor;
                if ScreenToClient(hwnd, &mut local) != 0 && win::rect_contains(&row, local) {
                    FillRect(hdc, &row, hover);
                }
            }
            let mut label = win::wide(&entries[index].1);
            let mut label_rect = win::rect(scale(16, dpi), top, client.right - scale(8, dpi), top + row_h);
            DrawTextW(hdc, label.as_mut_ptr(), -1, &mut label_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        }
        if count > visible_count {
            SetTextColor(hdc, SUBTEXT);
            let mut more = win::wide("Daha fazlası için tekerleği kaydır");
            let footer_top = client.bottom - scale(FOOTER_H, dpi);
            let mut footer = win::rect(scale(16, dpi), footer_top, client.right - scale(8, dpi), client.bottom);
            DrawTextW(hdc, more.as_mut_ptr(), -1, &mut footer, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        }
        DeleteObject(bg);
        DeleteObject(accent);
        DeleteObject(hover);
        EndPaint(hwnd, &ps);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_MOUSEMOVE => {
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_LBUTTONUP => {
            let dpi = GetDpiForWindow(hwnd).max(96);
            let y = win::y_lparam(lparam);
            let action_end = scale(HEADER_H + ACTION_H, dpi);
            if y >= scale(HEADER_H, dpi) && y < action_end {
                circles::toggle_visibility();
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            } else if y >= action_end {
                let entries = rows();
                let start = action_end + scale(4, dpi);
                let visible = visible_row_count(dpi, entries.len());
                let row_height = scale(ROW_H, dpi).max(1);
                let offset = ((y - start).max(0) / row_height) as usize;
                let index = SCROLL.with(Cell::get).saturating_add(offset);
                if offset < visible {
                    if let Some((which, _)) = entries.get(index).cloned() {
                        circles::open_target(which);
                    }
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16).signum();
            if delta == 0 {
                return 0;
            }
            let entries = rows();
            let max = entries.len().saturating_sub(visible_row_count(GetDpiForWindow(hwnd).max(96), entries.len()));
            SCROLL.with(|scroll| {
                let next = if delta > 0 { scroll.get().saturating_sub(1) } else { (scroll.get() + 1).min(max) };
                scroll.set(next);
            });
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_WINDOWPOSCHANGING => {
            let position = &mut *(lparam as *mut WINDOWPOS);
            preserve_dock(position);
            0
        }
        WM_SYSCOMMAND if wparam & 0xfff0 == SC_MINIMIZE as usize => 0,
        WM_SIZE if wparam == SIZE_MINIMIZED as usize => {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            place(hwnd);
            0
        }
        WM_DISPLAYCHANGE => {
            place(hwnd);
            0
        }
        WM_DPICHANGED => {
            let suggested = &*(lparam as *const RECT);
            let dpi = ((wparam & 0xffff) as u32).max(96);
            let (width, height) = dimensions(dpi, rows().len());
            let work = work_area();
            let x = suggested.left.max(work.left).min(work.right - width);
            let y = suggested.top.max(work.top).min(work.bottom - height);
            SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
            0
        }
        WM_DESTROY => {
            WINDOW.with(|window| {
                if window.get() == hwnd {
                    window.set(std::ptr::null_mut());
                }
            });
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_dimensions_scale_with_dpi() {
        assert_eq!(dimensions(96, 1).0, 238);
        assert_eq!(dimensions(192, 8).0, 476);
        assert_eq!(scale(ROW_H, 192), 60);
    }

    #[test]
    fn shell_hide_and_z_order_requests_keep_dock_visible_and_topmost() {
        let mut position = WINDOWPOS {
            hwnd: std::ptr::null_mut(),
            hwndInsertAfter: HWND_BOTTOM,
            x: 0,
            y: 0,
            cx: 0,
            cy: 0,
            flags: SWP_HIDEWINDOW | SWP_NOZORDER,
        };
        preserve_dock(&mut position);
        assert_eq!(position.flags & SWP_HIDEWINDOW, 0);
        assert_ne!(position.flags & SWP_SHOWWINDOW, 0);
        assert_eq!(position.flags & SWP_NOZORDER, 0);
        assert_eq!(position.hwndInsertAfter, HWND_TOPMOST);
    }
}
