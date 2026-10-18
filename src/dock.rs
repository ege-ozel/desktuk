//! Small, persistent recovery dock for the circles UI.
//!
//! Windows' Show Desktop commands can hide desktop-organiser windows. The dock stays
//! in the topmost band and gives the user a direct way to reveal or hide the circles.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, InvalidateRect, ScreenToClient, SetBkMode, SetTextColor,
    DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::circles::circle_win::Which;
use crate::{app, circles, win};

const CLASS: &str = "DeskTuk.Dock";
const BG: u32 = crate::node::BG;
const ACCENT: u32 = crate::node::ACCENT;
const TEXT: u32 = win::rgb(240, 240, 240);
const SUBTEXT: u32 = win::rgb(170, 175, 185);
const ROW_HOVER: u32 = win::rgb(44, 78, 120);
const MAX_ROWS: usize = 7;
const COMPACT_SIZE: i32 = 96;
const WIDTH: i32 = 164;
const HEADER_H: i32 = 96;
const ACTION_SIZE: i32 = 104;
const GAP: i32 = 4;
const ROW_H: i32 = 28;

#[derive(Clone, Copy)]
enum Entry {
    Toggle,
    Target(Which),
}

thread_local! {
    static WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static SCROLL: Cell<usize> = const { Cell::new(0) };
    static EXPANDED: Cell<bool> = const { Cell::new(false) };
    static MENU_OPEN: Cell<bool> = const { Cell::new(false) };
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ACTION_HOVER: Cell<bool> = const { Cell::new(false) };
}

fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

fn rows() -> Vec<(Entry, String)> {
    let Ok(board) = app::app().board.try_borrow() else { return vec![(Entry::Toggle, "Daireleri göster".to_string())] };
    let mut rows = Vec::with_capacity(board.circles.len() + 2);
    rows.push((Entry::Toggle, if circles::any_visible() { "Daireleri gizle" } else { "Daireleri göster" }.to_string()));
    rows.push((Entry::Target(Which::Recent), "Recent".to_string()));
    rows.extend(board.circles.iter().enumerate().map(|(i, circle)| (Entry::Target(Which::User(i)), circle.name.clone())));
    rows
}

fn dimensions(dpi: u32, count: usize) -> (i32, i32) {
    if !EXPANDED.get() {
        return (scale(COMPACT_SIZE, dpi), scale(HEADER_H, dpi));
    }
    let visible = if MENU_OPEN.get() { visible_row_count(dpi, count) as i32 } else { 0 };
    (scale(WIDTH, dpi), scale(HEADER_H + GAP + ACTION_SIZE + GAP + visible * ROW_H, dpi))
}

fn visible_row_count(dpi: u32, count: usize) -> usize {
    let work = work_area();
    let row = scale(ROW_H, dpi).max(1);
    let fixed = scale(HEADER_H + GAP + ACTION_SIZE + GAP, dpi);
    let available = (work.bottom - work.top - fixed).max(0);
    let fit = (available / row) as usize;
    count.min(MAX_ROWS).min(fit.max(1))
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
    let centered = work.top + (work.bottom - work.top - scale(HEADER_H, dpi)) / 2;
    let y = centered.min((work.bottom - height).max(work.top)).max(work.top);
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

fn preserve_dock(position: &mut WINDOWPOS) {
    position.flags = (position.flags & !SWP_HIDEWINDOW) | SWP_SHOWWINDOW;
    position.flags &= !SWP_NOZORDER;
    position.hwndInsertAfter = HWND_TOPMOST;
}

pub fn contains_screen(point: POINT) -> bool {
    WINDOW.with(|window| {
        let hwnd = window.get();
        if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
            return false;
        }
        let mut rect: RECT = unsafe { std::mem::zeroed() };
        unsafe { GetWindowRect(hwnd, &mut rect) != 0 && win::rect_contains(&rect, point) }
    })
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
    let corner: i32 = 1; // DWMWCP_DONOTROUND: closed state is a square Desktop tile.
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
    let visible_count = if EXPANDED.get() && MENU_OPEN.get() { visible_row_count(dpi, count) } else { 0 };
    let scroll = SCROLL.with(Cell::get).min(count.saturating_sub(visible_count));
    let row_h = scale(ROW_H, dpi);
    let header_h = scale(HEADER_H, dpi);
    let action_h = scale(ACTION_SIZE, dpi);
    let expanded = EXPANDED.get();
    let menu_open = MENU_OPEN.get();
    let bg = unsafe { CreateSolidBrush(BG) };
    let accent = unsafe { CreateSolidBrush(ACCENT) };
    let hover = unsafe { CreateSolidBrush(ROW_HOVER) };
    let action_bg = unsafe { CreateSolidBrush(ROW_HOVER) };
    let header_left = client.right - header_h;
    let header_tile = win::rect(header_left, 0, client.right, header_h);
    let stripe = win::rect(header_left, 0, header_left + scale(3, dpi), header_h);
    let mut cursor = POINT { x: 0, y: 0 };
    unsafe {
        FillRect(hdc, &client, bg);
        FillRect(hdc, &header_tile, hover);
        FillRect(hdc, &stripe, accent);
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, TEXT);
        let mut title = win::wide("Desktop");
        let mut title_rect = win::rect(header_left + scale(8, dpi), scale(2, dpi), client.right - scale(26, dpi), header_h - scale(2, dpi));
        DrawTextW(hdc, title.as_mut_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SetTextColor(hdc, ACCENT);
        let mut arrow = win::wide(if menu_open { "^" } else { "v" });
        let mut arrow_rect = win::rect(client.right - scale(22, dpi), 0, client.right - scale(6, dpi), header_h);
        DrawTextW(hdc, arrow.as_mut_ptr(), -1, &mut arrow_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        if !expanded {
            DeleteObject(bg);
            DeleteObject(accent);
            DeleteObject(hover);
            DeleteObject(action_bg);
            EndPaint(hwnd, &ps);
            return;
        }
        let action_top = header_h + scale(GAP, dpi);
        let action_bottom = action_top + action_h;
        let action_left = client.right - action_h;
        let action_rect = win::rect(action_left, action_top, action_left + action_h, action_bottom);
        FillRect(hdc, &action_rect, action_bg);
        SetTextColor(hdc, ACCENT);
        let mut glyph = win::wide(if circles::any_visible() { "●" } else { "○" });
        let mut glyph_rect = win::rect(action_left, action_top + scale(2, dpi), action_left + action_h, action_top + scale(42, dpi));
        DrawTextW(hdc, glyph.as_mut_ptr(), -1, &mut glyph_rect, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        SetTextColor(hdc, TEXT);
        let mut action = win::wide(if circles::any_visible() { "Daireleri gizle" } else { "Daireleri göster" });
        let mut action_text =
            win::rect(action_left + scale(2, dpi), action_top + scale(42, dpi), action_left + action_h - scale(2, dpi), action_bottom);
        DrawTextW(hdc, action.as_mut_ptr(), -1, &mut action_text, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        if !menu_open {
            DeleteObject(bg);
            DeleteObject(accent);
            DeleteObject(hover);
            DeleteObject(action_bg);
            EndPaint(hwnd, &ps);
            return;
        }
        SetTextColor(hdc, SUBTEXT);
        for visible_index in 0..visible_count {
            let index = scroll + visible_index;
            let top = action_bottom + scale(GAP, dpi) + visible_index as i32 * row_h;
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
        DeleteObject(bg);
        DeleteObject(accent);
        DeleteObject(hover);
        DeleteObject(action_bg);
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
            TRACKING.with(|tracking| {
                if !tracking.replace(true) {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    unsafe { TrackMouseEvent(&mut tme) };
                }
            });
            if !EXPANDED.get() {
                EXPANDED.set(true);
                place(hwnd);
            }
            let y = win::y_lparam(lparam);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let action_top = scale(HEADER_H + GAP, dpi);
            let action_bottom = action_top + scale(ACTION_SIZE, dpi);
            let hovering_action = !MENU_OPEN.get() && y >= action_top && y < action_bottom;
            if hovering_action && !ACTION_HOVER.replace(true) {
                circles::toggle_visibility();
            } else if !hovering_action {
                ACTION_HOVER.set(false);
            }
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_MOUSELEAVE => {
            TRACKING.set(false);
            ACTION_HOVER.set(false);
            EXPANDED.set(false);
            MENU_OPEN.set(false);
            SCROLL.set(0);
            place(hwnd);
            unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            0
        }
        WM_LBUTTONUP => {
            let dpi = GetDpiForWindow(hwnd).max(96);
            let y = win::y_lparam(lparam);
            let header_end = scale(HEADER_H, dpi);
            let action_end = scale(HEADER_H + GAP + ACTION_SIZE, dpi);
            if y < header_end {
                MENU_OPEN.set(!MENU_OPEN.get());
                EXPANDED.set(true);
                ACTION_HOVER.set(false);
                SCROLL.set(0);
                place(hwnd);
                unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
            } else if MENU_OPEN.get() && y >= action_end {
                let entries = rows();
                let start = action_end + scale(GAP, dpi);
                let visible = visible_row_count(dpi, entries.len());
                let row_height = scale(ROW_H, dpi).max(1);
                let offset = ((y - start).max(0) / row_height) as usize;
                let index = SCROLL.with(Cell::get).saturating_add(offset);
                if offset < visible {
                    if let Some((entry, _)) = entries.get(index).cloned() {
                        match entry {
                            Entry::Toggle => circles::toggle_visibility(),
                            Entry::Target(which) => circles::open_target(which),
                        }
                        MENU_OPEN.set(false);
                        ACTION_HOVER.set(false);
                        place(hwnd);
                        unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
                    }
                }
            }
            0
        }
        WM_MOUSEWHEEL => {
            if !MENU_OPEN.get() {
                return 0;
            }
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
        EXPANDED.set(false);
        MENU_OPEN.set(false);
        assert_eq!(dimensions(96, 1), (96, 96));
        EXPANDED.set(true);
        MENU_OPEN.set(true);
        assert_eq!(dimensions(192, 8).0, 328);
        assert_eq!(scale(ROW_H, 192), 56);
        EXPANDED.set(false);
        MENU_OPEN.set(false);
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
