//! The Node window: a movable panel that shows real shell items with their Explorer
//! icons and names, and opens them on double-click.

use std::path::PathBuf;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontIndirectW, CreateSolidBrush, DeleteDC, DeleteObject,
    DrawTextW, EndPaint, FillRect, InvalidateRect, SelectObject, SetBkMode, SetTextColor, DT_CENTER, DT_EDITCONTROL, DT_END_ELLIPSIS,
    DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DT_WORDBREAK, FW_SEMIBOLD, HDC, PAINTSTRUCT, SRCCOPY, TRANSPARENT,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi, SystemParametersInfoForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::{app, Drag, WM_APP_NODE_LOST};
use crate::{diag, dlog, items, win};

const CLASS: &str = "DeskTuk.Node";

pub const BG: u32 = win::rgb(30, 31, 36);
pub const HEADER: u32 = win::rgb(40, 42, 50);
/// Accent strip along the top edge. The self-test samples screen pixels for this colour.
pub const ACCENT: u32 = win::rgb(45, 125, 210);
const HOVER: u32 = win::rgb(56, 60, 72);
const SELECTED: u32 = win::rgb(44, 78, 120);
const TEXT: u32 = win::rgb(240, 240, 240);
const SUBTEXT: u32 = win::rgb(170, 175, 185);

const MAX_COLS: usize = 4;

const IDM_OPEN: usize = 1;
const IDM_LOCATION: usize = 2;
const IDM_EXIT: usize = 10;

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub pad: i32,
    pub accent: i32,
    pub header: i32,
    pub cell_w: i32,
    pub cell_h: i32,
    pub icon: i32,
    pub cols: i32,
    pub width: i32,
    pub height: i32,
}

pub fn layout(dpi: u32, count: usize) -> Layout {
    let s = |v| win::scale(v, dpi);
    let n = count.max(1);
    let cols = n.min(MAX_COLS) as i32;
    let rows = n.div_ceil(MAX_COLS) as i32;
    let (pad, accent, header, cell_w, cell_h, icon) = (s(8), s(3).max(2), s(26), s(88), s(92), s(48));
    Layout {
        pad,
        accent,
        header,
        cell_w,
        cell_h,
        icon,
        cols,
        width: pad * 2 + cols * cell_w,
        height: accent + header + pad + rows * cell_h + pad,
    }
}

/// Item cell in client coordinates.
pub fn item_rect(l: &Layout, i: usize) -> RECT {
    let col = i as i32 % l.cols;
    let row = i as i32 / l.cols;
    let x = l.pad + col * l.cell_w;
    let y = l.accent + l.header + l.pad + row * l.cell_h;
    win::rect(x, y, x + l.cell_w, y + l.cell_h)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hit {
    Item(usize),
    Header,
    Empty,
}

pub fn hit_test(l: &Layout, count: usize, p: POINT) -> Hit {
    if p.y < l.accent + l.header {
        return Hit::Header;
    }
    (0..count).find(|&i| win::rect_contains(&item_rect(l, i), p)).map(Hit::Item).unwrap_or(Hit::Empty)
}

pub fn current_layout(node: HWND) -> Layout {
    let dpi = unsafe { GetDpiForWindow(node) }.max(96);
    layout(dpi, app().items.borrow().len())
}

/// Screen-coordinate centre of item `i`'s icon (used by the self-test).
pub fn item_icon_center(node: HWND, i: usize) -> POINT {
    let l = current_layout(node);
    let r = item_rect(&l, i);
    let wr = win::window_rect(node);
    POINT { x: wr.left + (r.left + r.right) / 2, y: wr.top + r.top + l.pad / 2 + l.icon / 2 }
}

pub fn register_class() {
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DBLCLKS,
        lpfnWndProc: Some(wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

/// Creates the node window through the current host and shows it.
pub fn create() -> Result<HWND, String> {
    let a = app();
    let host = a.host();
    let spec = host.prepare()?;
    let class = win::wide(CLASS);
    let title = win::wide("DeskTuk Node");
    let hwnd = unsafe {
        CreateWindowExW(
            spec.ex_style,
            class.as_ptr(),
            title.as_ptr(),
            spec.style,
            0,
            0,
            100,
            100,
            spec.parent,
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return Err(format!("CreateWindowExW failed: error {}", unsafe { windows_sys::Win32::Foundation::GetLastError() }));
    }
    a.node.set(hwnd);
    if spec.style & WS_CHILD == 0 {
        // Windows 11 rounded corners; harmless failure on Windows 10.
        let pref: i32 = 2; // DWMWCP_ROUND
        unsafe { DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as u32, (&pref as *const i32).cast(), 4) };
    }
    host.attached(hwnd);
    reposition(hwnd);
    unsafe { ShowWindow(hwnd, SW_SHOWNOACTIVATE) };
    dlog!(
        "node created hwnd={hwnd:?} host={} parent={:?}({}) rect={:?}",
        host.kind().name(),
        spec.parent,
        if spec.parent.is_null() { String::new() } else { win::class_name(spec.parent) },
        rect_tuple(&win::window_rect(hwnd))
    );
    Ok(hwnd)
}

pub fn rect_tuple(r: &RECT) -> (i32, i32, i32, i32) {
    (r.left, r.top, r.right, r.bottom)
}

/// Sizes the node for its DPI/item count and moves it to `app().pos`, clamped so that
/// it stays reachable on the nearest monitor's work area.
pub fn reposition(node: HWND) {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromRect, MONITORINFO, MONITOR_DEFAULTTONEAREST};
    let l = current_layout(node);
    let (x, y) = app().pos.get();
    let mut r = win::rect(x, y, x + l.width, y + l.height);
    unsafe {
        let mon = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) != 0 {
            let w = mi.rcWork;
            let nx = r.left.min(w.right - l.width).max(w.left);
            let ny = r.top.min(w.bottom - l.height).max(w.top);
            r = win::rect(nx, ny, nx + l.width, ny + l.height);
        }
    }
    app().pos.set((r.left, r.top));
    app().host().place(node, &r);
    unsafe { InvalidateRect(node, std::ptr::null(), 0) };
}

fn icon_font(dpi: u32, bold: bool) -> windows_sys::Win32::Graphics::Gdi::HFONT {
    unsafe {
        let mut lf: windows_sys::Win32::Graphics::Gdi::LOGFONTW = std::mem::zeroed();
        SystemParametersInfoForDpi(
            SPI_GETICONTITLELOGFONT,
            std::mem::size_of_val(&lf) as u32,
            (&mut lf as *mut windows_sys::Win32::Graphics::Gdi::LOGFONTW).cast(),
            0,
            dpi,
        );
        if bold {
            lf.lfWeight = FW_SEMIBOLD as i32;
        }
        CreateFontIndirectW(&lf)
    }
}

fn fill(hdc: HDC, r: &RECT, color: u32) {
    unsafe {
        let b = CreateSolidBrush(color);
        FillRect(hdc, r, b);
        DeleteObject(b);
    }
}

fn paint(hwnd: HWND) {
    let a = app();
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let l = layout(dpi, a.items.borrow().len());
    let mut client = win::rect(0, 0, 0, 0);
    unsafe { GetClientRect(hwnd, &mut client) };
    let (w, h) = (client.right, client.bottom);
    unsafe {
        let mem = CreateCompatibleDC(hdc);
        let bmp = CreateCompatibleBitmap(hdc, w.max(1), h.max(1));
        let old_bmp = SelectObject(mem, bmp);

        fill(mem, &client, BG);
        fill(mem, &win::rect(0, 0, w, l.accent), ACCENT);
        fill(mem, &win::rect(0, l.accent, w, l.accent + l.header), HEADER);

        let title_font = icon_font(dpi, true);
        let item_font = icon_font(dpi, false);
        let old_font = SelectObject(mem, title_font);
        SetBkMode(mem, TRANSPARENT as i32);

        let host = a.host().kind();
        let title = if host.experimental() { format!("DeskTuk  \u{00b7}  {} (experimental)", host.name()) } else { "DeskTuk".to_string() };
        let mut tw = win::wide(&title);
        let mut tr = win::rect(l.pad + win::scale(2, dpi), l.accent, w - l.pad, l.accent + l.header);
        SetTextColor(mem, SUBTEXT);
        DrawTextW(mem, tw.as_mut_ptr(), -1, &mut tr, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX);

        SelectObject(mem, item_font);
        let hover = a.hover.get();
        let selected = a.selected.get();
        for (i, item) in a.items.borrow_mut().iter_mut().enumerate() {
            let cell = item_rect(&l, i);
            let inner = win::rect(cell.left + 2, cell.top + 2, cell.right - 2, cell.bottom - 2);
            if Some(i) == selected {
                fill(mem, &inner, SELECTED);
            } else if Some(i) == hover {
                fill(mem, &inner, HOVER);
            }
            let ix = cell.left + (l.cell_w - l.icon) / 2;
            let iy = cell.top + l.pad / 2;
            let icon = item.icon(l.icon);
            if !icon.is_null() {
                DrawIconEx(mem, ix, iy, icon, l.icon, l.icon, 0, std::ptr::null_mut(), DI_NORMAL);
            }
            SetTextColor(mem, if item.missing { SUBTEXT } else { TEXT });
            let mut text = win::wide(&item.name);
            let mut r = win::rect(cell.left + 4, iy + l.icon + win::scale(4, dpi), cell.right - 4, cell.bottom - 2);
            DrawTextW(mem, text.as_mut_ptr(), -1, &mut r, DT_CENTER | DT_WORDBREAK | DT_EDITCONTROL | DT_END_ELLIPSIS | DT_NOPREFIX);
        }

        BitBlt(hdc, 0, 0, w, h, mem, 0, 0, SRCCOPY);
        SelectObject(mem, old_font);
        DeleteObject(title_font);
        DeleteObject(item_font);
        SelectObject(mem, old_bmp);
        DeleteObject(bmp);
        DeleteDC(mem);
        EndPaint(hwnd, &ps);
    }
    if !a.first_paint_done.get() {
        a.first_paint_done.set(true);
        dlog!(
            "METRIC startup first_paint_since_main_ms={:.1} first_paint_since_process_creation_ms={:.1}",
            diag::since_start_ms(),
            diag::since_process_creation_ms()
        );
    }
}

fn client_point(lparam: LPARAM) -> POINT {
    POINT { x: win::x_lparam(lparam), y: win::y_lparam(lparam) }
}

fn cursor_pos() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

fn set_hover(hwnd: HWND, h: Option<usize>) {
    let a = app();
    if a.hover.get() != h {
        a.hover.set(h);
        unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
    }
}

fn item_path(i: usize) -> Option<PathBuf> {
    app().items.borrow().get(i).map(|it| it.path.clone())
}

/// Opens item `i`. The path is copied out first: ShellExecuteEx may pump messages and
/// re-enter the window procedure.
pub fn launch(hwnd: HWND, i: usize) {
    if let Some(path) = item_path(i) {
        let r = items::open(&path, hwnd);
        *app().last_launch.borrow_mut() = Some((path, r));
    }
}

fn context_menu(hwnd: HWND, screen: POINT, item: Option<usize>) {
    let a = app();
    let ctl = a.controller.get();
    unsafe {
        let m = CreatePopupMenu();
        if item.is_some() {
            AppendMenuW(m, MF_STRING, IDM_OPEN, win::wide("&Open").as_ptr());
            AppendMenuW(m, MF_STRING, IDM_LOCATION, win::wide("Open file &location").as_ptr());
            AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
        }
        let host = a.host().kind();
        let label = format!("Host: {}{}", host.name(), if host.experimental() { " (experimental)" } else { "" });
        AppendMenuW(m, MF_STRING | MF_GRAYED, 0, win::wide(&label).as_ptr());
        AppendMenuW(m, MF_STRING, IDM_EXIT, win::wide("E&xit DeskTuk").as_ptr());
        // The menu is owned by the hidden controller window so the node never needs activation.
        SetForegroundWindow(ctl);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, screen.x, screen.y, 0, ctl, std::ptr::null()) as usize;
        PostMessageW(ctl, WM_NULL, 0, 0);
        DestroyMenu(m);
        match (cmd, item) {
            (IDM_OPEN, Some(i)) => launch(hwnd, i),
            (IDM_LOCATION, Some(i)) => {
                if let Some(p) = item_path(i) {
                    items::open_location(&p);
                }
            }
            (IDM_EXIT, _) => crate::app::quit(),
            _ => {}
        }
    }
}

fn end_drag(hwnd: HWND) {
    let a = app();
    if let Some(d) = a.drag.take() {
        if d.moving {
            dlog!("node moved to {:?}", a.pos.get());
            a.save_config();
        }
        if unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetCapture() } == hwnd {
            unsafe { ReleaseCapture() };
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let a = app();
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        // Never take activation on click: the node must not jump above application windows.
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_WINDOWPOSCHANGING => {
            let wp = &mut *(lparam as *mut WINDOWPOS);
            a.host().window_pos_changing(hwnd, wp);
            0
        }
        WM_MOUSEMOVE => {
            if let Some(mut d) = a.drag.get() {
                let p = cursor_pos();
                let (dx, dy) = (p.x - d.cursor.x, p.y - d.cursor.y);
                let dpi = GetDpiForWindow(hwnd).max(96);
                if !d.moving && (dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi)) {
                    d.moving = true;
                }
                if d.moving {
                    let l = current_layout(hwnd);
                    let (x, y) = (d.origin.0 + dx, d.origin.1 + dy);
                    a.pos.set((x, y));
                    a.host().place(hwnd, &win::rect(x, y, x + l.width, y + l.height));
                }
                a.drag.set(Some(d));
            } else {
                let l = current_layout(hwnd);
                let hit = hit_test(&l, a.items.borrow().len(), client_point(lparam));
                set_hover(hwnd, if let Hit::Item(i) = hit { Some(i) } else { None });
                if !a.tracking_leave.get() {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    TrackMouseEvent(&mut tme);
                    a.tracking_leave.set(true);
                }
            }
            0
        }
        WM_MOUSELEAVE => {
            a.tracking_leave.set(false);
            set_hover(hwnd, None);
            0
        }
        WM_LBUTTONDOWN => {
            let l = current_layout(hwnd);
            match hit_test(&l, a.items.borrow().len(), client_point(lparam)) {
                Hit::Item(i) => {
                    a.selected.set(Some(i));
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                Hit::Header | Hit::Empty => {
                    if a.selected.take().is_some() {
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                    a.drag.set(Some(Drag { cursor: cursor_pos(), origin: a.pos.get(), moving: false }));
                    SetCapture(hwnd);
                }
            }
            0
        }
        WM_LBUTTONUP => {
            end_drag(hwnd);
            0
        }
        WM_CAPTURECHANGED => {
            if lparam as HWND != hwnd {
                end_drag(hwnd);
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            let l = current_layout(hwnd);
            if let Hit::Item(i) = hit_test(&l, a.items.borrow().len(), client_point(lparam)) {
                dlog!("double-click on item {i}");
                launch(hwnd, i);
            }
            0
        }
        WM_RBUTTONUP => {
            let l = current_layout(hwnd);
            let p = client_point(lparam);
            let item = match hit_test(&l, a.items.borrow().len(), p) {
                Hit::Item(i) => {
                    a.selected.set(Some(i));
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    Some(i)
                }
                _ => None,
            };
            context_menu(hwnd, cursor_pos(), item);
            0
        }
        WM_DPICHANGED => {
            // Top-level hosts: follow the suggested rectangle's position, recompute size.
            let r = &*(lparam as *const RECT);
            a.pos.set((r.left, r.top));
            reposition(hwnd);
            0
        }
        WM_DPICHANGED_AFTERPARENT => {
            reposition(hwnd);
            0
        }
        WM_DESTROY => {
            for it in a.items.borrow_mut().iter_mut() {
                it.release_icon();
            }
            a.drag.set(None);
            a.tracking_leave.set(false);
            if a.node.get() == hwnd {
                a.node.set(std::ptr::null_mut());
            }
            dlog!("node destroyed hwnd={hwnd:?} quitting={}", a.quitting.get());
            if !a.quitting.get() {
                PostMessageW(a.controller.get(), WM_APP_NODE_LOST, 0, 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_grows_with_items_and_dpi() {
        let one = layout(96, 1);
        let two = layout(96, 2);
        let five = layout(96, 5);
        assert_eq!(one.cols, 1);
        assert_eq!(two.cols, 2);
        assert_eq!(five.cols, MAX_COLS as i32);
        assert!(two.width > one.width);
        assert!(five.height > two.height, "second row adds height");
        let hi = layout(192, 2);
        assert_eq!(hi.width, two.width * 2);
        assert_eq!(hi.icon, 96);
    }

    #[test]
    fn hit_test_finds_items_header_and_gaps() {
        let l = layout(96, 2);
        let c0 = item_rect(&l, 0);
        let c1 = item_rect(&l, 1);
        let mid = |r: RECT| POINT { x: (r.left + r.right) / 2, y: (r.top + r.bottom) / 2 };
        assert_eq!(hit_test(&l, 2, mid(c0)), Hit::Item(0));
        assert_eq!(hit_test(&l, 2, mid(c1)), Hit::Item(1));
        assert_eq!(hit_test(&l, 2, POINT { x: 5, y: 2 }), Hit::Header);
        assert_eq!(hit_test(&l, 2, POINT { x: 1, y: l.height - 1 }), Hit::Empty);
        // Cells never overlap and stay inside the node.
        assert!(c0.right <= c1.left);
        assert!(c1.right <= l.width && c1.bottom <= l.height);
    }
}
