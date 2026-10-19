//! Small, persistent recovery dock for the circles UI.
//!
//! Windows' Show Desktop commands can hide desktop-organiser windows. The dock stays
//! in the topmost band and gives the user a direct way to reveal or hide the circles.

use std::cell::{Cell, RefCell};

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, InvalidateRect, ScreenToClient, SelectObject,
    SetBkMode, SetTextColor, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HFONT, PAINTSTRUCT, TRANSPARENT,
};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::circles::circle_win::Which;
use crate::{app, circles, dock_drag, dock_layout, win};

const CLASS: &str = "DeskTuk.Dock";
const BG: u32 = crate::theme::BG;
const ACCENT: u32 = crate::theme::BORDER;
const TEXT: u32 = crate::theme::TEXT;
const SUBTEXT: u32 = crate::theme::MUTED;
const ROW_HOVER: u32 = crate::theme::RAISED;
const TEAL: u32 = crate::theme::MINT;
const SWITCH_BG: u32 = crate::theme::SURFACE;
const MAX_ROWS: usize = 7;
const PAD: i32 = 8;
const HEADER_H: i32 = 40;
const ACTION_SIZE: i32 = 48;
const GAP: i32 = 4;
const ROW_H: i32 = 28;
const ACTION_TIMER: usize = 1;
const ACTION_DWELL_MS: u32 = 300;

#[derive(Clone, Copy)]
enum Entry {
    Target(Which),
}

#[derive(PartialEq)]
struct VisibilityKey {
    size: i32,
    dpi: u32,
    visible: bool,
    progress: Option<u8>,
    locked: bool,
}

thread_local! {
    static POSITION: Cell<Option<(i32,i32)>> = const { Cell::new(None) };
    static EXPANDED: Cell<bool> = const { Cell::new(true) };
    static WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static SCROLL: Cell<usize> = const { Cell::new(0) };
    static TRACKING: Cell<bool> = const { Cell::new(false) };
    static ACTION_TARGET: Cell<Option<bool>> = const { Cell::new(None) };
    static PAINT_BUFFER: RefCell<Option<circles::surface::Surface>> = const { RefCell::new(None) };
    static ACTION_START: Cell<Option<f64>> = const { Cell::new(None) };
    static ACTION_HOVER: Cell<bool> = const { Cell::new(false) };
    static ACTION_INSIDE: Cell<bool> = const { Cell::new(false) };
    static HOVER_ROW: Cell<isize> = const { Cell::new(-1) };
    static VISIBILITY_CACHE: RefCell<Option<(VisibilityKey, circles::surface::Surface)>> = const { RefCell::new(None) };
}

fn scale(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

fn rows() -> Vec<(Entry, String)> {
    let Ok(board) = app::app().board.try_borrow() else { return Vec::new() };
    let mut rows = Vec::with_capacity(board.circles.len() + 1);
    rows.push((Entry::Target(Which::Recent), "Recent".to_string()));
    rows.extend(board.circles.iter().enumerate().map(|(i, circle)| (Entry::Target(Which::User(i)), circle.name.clone())));
    rows
}

fn icon_title_font(dpi: u32) -> HFONT {
    crate::theme::font(dpi, false, 100)
}

fn expanded_width(dpi: u32) -> i32 {
    scale(153, dpi)
}

pub fn locked() -> bool {
    app::app().board.try_borrow().map(|b| b.dock_locked).unwrap_or(true)
}

fn toggle_lock(hwnd: HWND) {
    cancel_dwell(hwnd);
    ACTION_INSIDE.set(false);
    ACTION_HOVER.set(false);
    HOVER_ROW.set(-1);
    let changed = if let Ok(mut board) = app::app().board.try_borrow_mut() {
        board.dock_locked = !board.dock_locked;
        true
    } else {
        false
    };
    if changed {
        circles::ring::close_now();
        app::app().save_config();
        unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
    }
}

// Freeze the completed icon while showing/hiding windows pumps nested paints.
// Never expose a partially updated board or reset to the old dwell endpoint.
fn finish_visibility(hwnd: HWND) {
    unsafe { KillTimer(hwnd, ACTION_TIMER) };
    ACTION_START.set(None);
    ACTION_TARGET.set(Some(!circles::any_visible()));
    circles::toggle_visibility();
    ACTION_TARGET.set(None);
    let rect = action_rect(unsafe { GetDpiForWindow(hwnd) }.max(96));
    unsafe { InvalidateRect(hwnd, &rect, 0) };
}

fn visibility_frame(visible: bool, progress: Option<f32>, target: Option<bool>) -> (bool, Option<f32>) {
    match target {
        Some(target) => (target, None),
        None => (visible, progress),
    }
}

fn cancel_dwell(hwnd: HWND) {
    unsafe { KillTimer(hwnd, ACTION_TIMER) };
    if ACTION_START.take().is_none() {
        return;
    }
    let rect = action_rect(unsafe { GetDpiForWindow(hwnd) }.max(96));
    unsafe { InvalidateRect(hwnd, &rect, 0) };
}

fn lock_rect(dpi: u32) -> RECT {
    if EXPANDED.get() {
        win::rect(scale(103, dpi), scale(9, dpi), scale(125, dpi), scale(31, dpi))
    } else {
        win::rect(scale(43, dpi), scale(1, dpi), scale(63, dpi), scale(21, dpi))
    }
}

fn dimensions(dpi: u32, count: usize) -> (i32, i32) {
    if !EXPANDED.get() {
        let side = scale(ACTION_SIZE + PAD * 2, dpi);
        return (side, side);
    }
    let visible = visible_row_count(dpi, count) as i32;
    (expanded_width(dpi), scale(HEADER_H + GAP + ACTION_SIZE + GAP + visible * ROW_H, dpi))
}

fn action_rect(dpi: u32) -> RECT {
    let size = scale(ACTION_SIZE, dpi);
    let left = if EXPANDED.get() { (expanded_width(dpi) - size) / 2 } else { scale(PAD, dpi) };
    let top = scale(if EXPANDED.get() { HEADER_H + GAP } else { PAD }, dpi);
    win::rect(left, top, left + size, top + size)
}

fn arrow_rect(dpi: u32) -> RECT {
    let width = expanded_width(dpi);
    win::rect(width - scale(26, dpi), 0, width, scale(HEADER_H, dpi))
}

fn set_expanded(hwnd: HWND, expanded: bool) {
    EXPANDED.set(expanded);
    cancel_dwell(hwnd);
    ACTION_INSIDE.set(false);
    ACTION_HOVER.set(false);
    HOVER_ROW.set(-1);
    place(hwnd);
    unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
}

fn row_rect(dpi: u32, index: usize) -> RECT {
    let scroll = SCROLL.with(Cell::get);
    let top = scale(HEADER_H + GAP + ACTION_SIZE + GAP, dpi) + index.saturating_sub(scroll) as i32 * scale(ROW_H, dpi);
    win::rect(scale(8, dpi), top, expanded_width(dpi) - scale(PAD, dpi), top + scale(ROW_H, dpi))
}

fn visible_row_count(dpi: u32, count: usize) -> usize {
    let work = work_area();
    let row = scale(ROW_H, dpi).max(1);
    let fixed = scale(HEADER_H + GAP + ACTION_SIZE + GAP, dpi);
    let available = (work.bottom - work.top - fixed).max(0);
    let fit = (available / row) as usize;
    count.min(MAX_ROWS).min(fit.max(1))
}

fn monitor_area() -> (RECT, RECT) {
    let point = POSITION.get().map(|(x, y)| POINT { x, y }).unwrap_or(POINT { x: 0, y: 0 });
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST), &mut info) } != 0 {
        return (info.rcMonitor, info.rcWork);
    }
    let mut work = win::rect(0, 0, 1024, 768);
    unsafe {
        SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0);
    }
    (work, work)
}
fn work_area() -> RECT {
    monitor_area().1
}
fn bounds(r: RECT) -> dock_layout::Bounds {
    (r.left, r.top, r.right, r.bottom)
}

fn place(hwnd: HWND) {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let size = dimensions(dpi, rows().len());
    let (monitor, work) = monitor_area();
    let at = POSITION.get().unwrap_or((work.right, work.top + (work.bottom - work.top) / 2));
    let edge = dock_layout::edge(bounds(monitor), bounds(work), at);
    let action = action_rect(dpi);
    let (x, y) = dock_layout::position(bounds(work), edge, at, size, scale(12, dpi), (action.top + action.bottom) / 2);
    unsafe {
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, size.0, size.1, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

pub fn move_anchor(hwnd: HWND, at: Option<(i32, i32)>) {
    POSITION.set(at);
    place(hwnd);
}

pub fn save_position() {
    let hwnd = WINDOW.get();
    let rect = win::window_rect(hwnd);
    let action = action_rect(unsafe { GetDpiForWindow(hwnd) }.max(96));
    let at = (rect.left + (rect.right - rect.left) / 2, rect.top + (action.top + action.bottom) / 2);
    POSITION.set(Some(at));
    {
        let Ok(mut board) = app::app().board.try_borrow_mut() else { return };
        board.dock_pos = Some(at);
    }
    app::app().save_config();
}

pub fn after_drag() {
    // Require a fresh hover entry after moving, while allowing a deliberate click.
    ACTION_START.set(None);
    ACTION_INSIDE.set(true);
    ACTION_HOVER.set(false);
    HOVER_ROW.set(-1);
}

fn screen_point(hwnd: HWND, lparam: isize) -> POINT {
    let mut point = POINT { x: win::x_lparam(lparam), y: win::y_lparam(lparam) };
    unsafe {
        ClientToScreen(hwnd, &mut point);
    }
    point
}

pub fn click_screen(hwnd: HWND, mut point: POINT) {
    if unsafe { ScreenToClient(hwnd, &mut point) } != 0 {
        unsafe {
            click_client(hwnd, point.x, point.y);
        }
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
    POSITION.set(app::app().board.try_borrow().ok().and_then(|b| b.dock_pos));
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
    let corner: i32 = 2; // DWMWCP_ROUND: native rounded brand surface.
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
    EXPANDED.set(true);
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
    dock_drag::shutdown();
    WINDOW.with(|window| {
        let hwnd = window.replace(std::ptr::null_mut());
        if !hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0 {
            unsafe { DestroyWindow(hwnd) };
        }
    });
}

/// Composite coverage into the known opaque dock background, just like circle rims.
/// The small surface is transient; text remains native GDI at the original DPI.
fn rounded_surface(w: i32, h: i32, radius: i32, color: u32) -> Option<circles::surface::Surface> {
    let mut surface = circles::surface::Surface::new(w, h)?;
    let radius = radius.clamp(0, w.min(h) / 2) as f32;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let pixels = surface.pixels();
    for y in 0..h {
        for x in 0..w {
            let dx = ((x as f32 + 0.5 - cx).abs() - (cx - radius)).max(0.0);
            let dy = ((y as f32 + 0.5 - cy).abs() - (cy - radius)).max(0.0);
            let coverage = if radius == 0.0 { 255 } else { circles::geom::coverage(dx, dy, radius) as u32 };
            let channel = |shift: u32| (((color >> shift) & 255) * coverage + ((BG >> shift) & 255) * (255 - coverage) + 127) / 255;
            // COLORREF is BBGGRR; opaque DIB pixels are AARRGGBB.
            pixels[(y * w + x) as usize] = 0xff00_0000 | channel(0) << 16 | channel(8) << 8 | channel(16);
        }
    }
    Some(surface)
}

unsafe fn rounded_fill(dc: windows_sys::Win32::Graphics::Gdi::HDC, r: RECT, color: u32, radius: i32) {
    use windows_sys::Win32::Graphics::Gdi::{BitBlt, GetStockObject, RoundRect, NULL_PEN, SRCCOPY};
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    if w <= 0 || h <= 0 {
        return;
    }
    if let Some(surface) = rounded_surface(w, h, radius, color) {
        BitBlt(dc, r.left, r.top, w, h, surface.dc(), 0, 0, SRCCOPY);
    } else {
        // Preserve a usable control even under GDI resource exhaustion.
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(NULL_PEN));
        RoundRect(dc, r.left, r.top, r.right, r.bottom, radius * 2, radius * 2);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }
}

fn mix_color(a: u32, b: u32, amount: f32) -> u32 {
    let alpha = (amount.clamp(0.0, 1.0) * 255.0).round() as u32;
    let channel = |shift: u32| (((a >> shift) & 255) * (255 - alpha) + ((b >> shift) & 255) * alpha + 127) / 255;
    channel(0) | channel(8) << 8 | channel(16) << 16
}

fn blend_pixel(pixel: &mut u32, color: u32, alpha: f32) {
    let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    let channel = |dst: u32, src: u32| (((*pixel >> dst) & 255) * (255 - a) + ((color >> src) & 255) * a + 127) / 255;
    *pixel = 0xff00_0000 | channel(16, 0) << 16 | channel(8, 8) << 8 | channel(0, 16);
}

fn line_distance(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * vx + (y - a.1) * vy) / (vx * vx + vy * vy).max(0.001)).clamp(0.0, 1.0);
    ((x - a.0 - t * vx).powi(2) + (y - a.1 - t * vy).powi(2)).sqrt()
}

/// Analytic coverage, no glyph/font dependence or animated blur. Work is bounded
/// to the 48-DIP control and only repeated during the 300ms dwell.
fn visibility_surface(size: i32, dpi: u32, visible: bool, progress: Option<f32>, is_locked: bool) -> Option<circles::surface::Surface> {
    let level = match progress {
        Some(p) if visible => 1.0 - p,
        Some(p) => p,
        None => {
            if visible {
                1.0
            } else {
                0.0
            }
        }
    };
    let color = mix_color(SWITCH_BG, crate::theme::ACCENT, if is_locked { 0.0 } else { 0.19 * level });
    let mut surface = rounded_surface(size, size, scale(8, dpi), color)?;
    let factor = size as f32 / 48.0;
    let strength = if is_locked { 0.0 } else { level };
    let arc = mix_color(SUBTEXT, crate::theme::ACCENT, strength);
    let folder = mix_color(SUBTEXT, TEAL, strength);
    let points = [(16.5, 20.0), (22.0, 20.0), (24.5, 23.0), (32.5, 23.0), (32.5, 32.5), (16.5, 32.5), (16.5, 20.0)];
    let pixels = surface.pixels();
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5) / factor, (y as f32 + 0.5) / factor);
            let pixel = &mut pixels[(y * size + x) as usize];
            let (dx, dy) = (px - 24.0, py - 24.0);
            let distance = ((dx * dx + dy * dy).sqrt() - 15.3).abs();
            {
                let coverage = ((1.0 + 0.2 * level - distance) * factor + 0.5).clamp(0.0, 1.0);
                blend_pixel(pixel, arc, coverage * if is_locked { 0.55 } else { 0.60 + 0.40 * level });
            }
            let edge = points.windows(2).map(|p| line_distance(px, py, p[0], p[1])).fold(f32::INFINITY, f32::min);
            let mut inside = false;
            for p in points.windows(2) {
                if (p[0].1 > py) != (p[1].1 > py) && px < (p[1].0 - p[0].0) * (py - p[0].1) / (p[1].1 - p[0].1) + p[0].0 {
                    inside = !inside;
                }
            }
            if inside {
                blend_pixel(pixel, folder, level * 0.25);
            }
            blend_pixel(
                pixel,
                folder,
                ((0.85 + 0.15 * level - edge) * factor + 0.5).clamp(0.0, 1.0) * if is_locked { 0.55 } else { 0.70 + 0.30 * level },
            );
            let fold = line_distance(px, py, (17.0, 25.5), (31.5, 25.5));
            blend_pixel(pixel, folder, ((0.65 - fold) * factor + 0.5).clamp(0.0, 1.0) * 0.65);
            let slash = line_distance(px, py, (15.0, 15.0), (34.0, 34.0));
            // A background halo makes the hidden slash readable through the folder.
            blend_pixel(pixel, color, ((1.9 - slash) * factor + 0.5).clamp(0.0, 1.0) * (1.0 - level));
            let hidden = 1.0 - level;
            let glow = 0.22 * (-slash * slash / 12.5).exp() * hidden;
            blend_pixel(pixel, win::rgb(246, 87, 104), glow);
            let gradient = mix_color(TEAL, crate::theme::ACCENT, ((py - 15.0) / 19.0).clamp(0.0, 1.0));
            blend_pixel(pixel, gradient, ((0.9 - slash) * factor + 0.5).clamp(0.0, 1.0) * hidden);
        }
    }
    Some(surface)
}

fn lock_surface(size: i32, color: u32, is_locked: bool) -> Option<circles::surface::Surface> {
    let mut surface = circles::surface::Surface::new(size, size)?;
    let factor = size as f32 / 20.0;
    let pixels = surface.pixels();
    for y in 0..size {
        for x in 0..size {
            let pixel = &mut pixels[(y * size + x) as usize];
            *pixel = 0xff00_0000 | (color & 255) << 16 | color & 0xff00 | (color >> 16) & 255;
            // One union mask avoids double blending where the tangent shackle
            // stems meet the rounded body. Subpixel coverage includes both joins.
            let mut covered = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let sub_x = x * 4 + sx;
                    // Mirror integer sample addresses before float conversion:
                    // half-pixel ties stay identical on both sides at 125% DPI.
                    let sub_x = if is_locked { sub_x.min(size * 4 - 1 - sub_x) } else { sub_x };
                    let px = (sub_x as f32 + 0.5) / (4.0 * factor);
                    let py = (y as f32 + (sy as f32 + 0.5) / 4.0) / factor;
                    if lock_distance(px, py, is_locked) <= 0.7 {
                        covered += 1;
                    }
                }
            }
            blend_pixel(pixel, if is_locked { TEAL } else { SUBTEXT }, covered as f32 / 16.0);
        }
    }
    Some(surface)
}

fn lock_distance(px: f32, py: f32, closed: bool) -> f32 {
    // Rounded-rectangle centreline: 4.5..15.5 x 9..16, radius 1.5 DIP.
    let qx = (px - 10.0).abs() - 4.0;
    let qy = (py - 12.5).abs() - 2.0;
    let body = (qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 1.5).abs();
    let (cx, cy) = if closed { (10.0, 6.8) } else { (11.8, 5.5) };
    let radius = 3.2;
    let arc = if py <= cy {
        ((px - cx).hypot(py - cy) - radius).abs()
    } else {
        (px - cx + radius).hypot(py - cy).min((px - cx - radius).hypot(py - cy))
    };
    let left = line_distance(px, py, (cx - radius, cy), (cx - radius, 9.0));
    let right = if closed { line_distance(px, py, (cx + radius, cy), (cx + radius, 9.0)) } else { f32::INFINITY };
    let key = line_distance(px, py, (10.0, 12.0), (10.0, 13.5));
    body.min(arc).min(left).min(right).min(key)
}

unsafe fn draw_lock(dc: windows_sys::Win32::Graphics::Gdi::HDC, r: RECT, dpi: u32, is_locked: bool) {
    let side = scale(20, dpi);
    let color = if EXPANDED.get() { ROW_HOVER } else { BG };
    if let Some(surface) = lock_surface(side, color, is_locked) {
        use windows_sys::Win32::Graphics::Gdi::{BitBlt, SRCCOPY};
        BitBlt(dc, (r.left + r.right - side) / 2, (r.top + r.bottom - side) / 2, side, side, surface.dc(), 0, 0, SRCCOPY);
    }
}

fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let window_dc = unsafe { BeginPaint(hwnd, &mut ps) };
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    // Own the buffer locally: painting/layout may pump messages, so no RefCell
    // borrow may span the render. Reuse its allocation until the client resizes.
    let mut buffer = PAINT_BUFFER.with(|slot| slot.borrow_mut().take());
    if buffer.as_ref().is_none_or(|s| s.w != client.right || s.h != client.bottom) {
        buffer = circles::surface::Surface::new(client.right, client.bottom);
    }
    let hdc = buffer.as_ref().map_or(window_dc, |s| s.dc());
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let entries = rows();
    let count = entries.len();
    let visible_count = visible_row_count(dpi, count);
    let scroll = SCROLL.with(Cell::get).min(count.saturating_sub(visible_count));
    let row_h = scale(ROW_H, dpi);
    let header_h = scale(HEADER_H, dpi);

    let bg = unsafe { CreateSolidBrush(BG) };
    let accent = unsafe { CreateSolidBrush(ACCENT) };
    let hover = unsafe { CreateSolidBrush(ROW_HOVER) };
    let switch_on = circles::any_visible();
    let font = icon_title_font(dpi);
    let old_font = if font.is_null() { std::ptr::null_mut() } else { unsafe { SelectObject(hdc, font) } };
    let header_tile = win::rect(0, 0, client.right, header_h);
    let switch_rect = action_rect(dpi);
    let switch_bottom = switch_rect.bottom;
    let border = win::rect(0, 0, client.right, client.bottom);
    unsafe {
        FillRect(hdc, &client, bg);
        if EXPANDED.get() {
            FillRect(hdc, &header_tile, hover);
        }
        FrameRect(hdc, &border, accent);
        SetBkMode(hdc, TRANSPARENT as i32);
        if EXPANDED.get() {
            let mark = LoadImageW(
                GetModuleHandleW(std::ptr::null()),
                std::ptr::without_provenance(1),
                IMAGE_ICON,
                scale(32, dpi),
                scale(32, dpi),
                LR_SHARED,
            );
            if !mark.is_null() {
                DrawIconEx(hdc, scale(6, dpi), scale(4, dpi), mark, scale(32, dpi), scale(32, dpi), 0, std::ptr::null_mut(), DI_NORMAL);
            }
            let heading_font = crate::theme::font(dpi, true, 100);
            if !heading_font.is_null() {
                SelectObject(hdc, heading_font);
            }
            SetTextColor(hdc, TEXT);
            let mut title = win::wide("DeskTuk");
            let mut title_rect = win::rect(scale(42, dpi), scale(2, dpi), lock_rect(dpi).left - scale(2, dpi), header_h - scale(2, dpi));
            DrawTextW(hdc, title.as_mut_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(hdc, if font.is_null() { old_font } else { font });
            if !heading_font.is_null() {
                DeleteObject(heading_font);
            }
            let mut arrow = win::wide("⌄");
            let mut arrow_area = arrow_rect(dpi);
            DrawTextW(hdc, arrow.as_mut_ptr(), -1, &mut arrow_area, DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
        }
        let is_locked = locked();
        let progress =
            ACTION_START.get().map(|start| ((crate::diag::since_start_ms() - start) / ACTION_DWELL_MS as f64).clamp(0.0, 1.0) as f32);
        let (switch_on, progress) = visibility_frame(switch_on, progress, ACTION_TARGET.get());
        let key = VisibilityKey {
            size: switch_rect.right - switch_rect.left,
            dpi,
            visible: switch_on,
            progress: progress.map(|p| (p * 255.0).round() as u8),
            locked: is_locked,
        };
        VISIBILITY_CACHE.with(|slot| {
            let mut cache = slot.borrow_mut();
            if cache.as_ref().is_none_or(|(previous, _)| *previous != key) {
                *cache = visibility_surface(key.size, key.dpi, key.visible, key.progress.map(|p| p as f32 / 255.0), key.locked)
                    .map(|surface| (key, surface));
            }
            if let Some((_, surface)) = cache.as_ref() {
                use windows_sys::Win32::Graphics::Gdi::{BitBlt, SRCCOPY};
                // BitBlt does not pump messages; no window/layout calls under this borrow.
                BitBlt(hdc, switch_rect.left, switch_rect.top, surface.w, surface.h, surface.dc(), 0, 0, SRCCOPY);
            }
        });
        let lock = lock_rect(dpi);
        draw_lock(hdc, lock, dpi, is_locked);
        SetTextColor(hdc, TEXT);
        if EXPANDED.get() {
            SetTextColor(hdc, SUBTEXT);
            for visible_index in 0..visible_count.min(count.saturating_sub(scroll)) {
                let index = scroll + visible_index;
                let top = switch_bottom + scale(GAP, dpi) + visible_index as i32 * row_h;
                let row = win::rect(scale(8, dpi), top, client.right - scale(8, dpi), top + row_h);
                if HOVER_ROW.with(Cell::get) == index as isize {
                    rounded_fill(hdc, row, ROW_HOVER, scale(6, dpi));
                }
                let mut label = win::wide(&entries[index].1);
                let mut label_rect = win::rect(scale(16, dpi), top, client.right - scale(8, dpi), top + row_h);
                DrawTextW(hdc, label.as_mut_ptr(), -1, &mut label_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            }
        }
        DeleteObject(bg);
        DeleteObject(accent);
        DeleteObject(hover);
        if !font.is_null() {
            if !old_font.is_null() {
                SelectObject(hdc, old_font);
            }
            DeleteObject(font);
        }
        if let Some(surface) = buffer.as_ref() {
            use windows_sys::Win32::Graphics::Gdi::{BitBlt, SRCCOPY};
            let r = ps.rcPaint;
            // The window sees only completed frames, through BeginPaint's clip.
            BitBlt(window_dc, r.left, r.top, r.right - r.left, r.bottom - r.top, surface.dc(), r.left, r.top, SRCCOPY);
        }
        PAINT_BUFFER.with(|slot| *slot.borrow_mut() = buffer);
        EndPaint(hwnd, &ps);
    }
}

unsafe fn click_client(hwnd: HWND, x: i32, y: i32) {
    let dpi = GetDpiForWindow(hwnd).max(96);
    let action = action_rect(dpi);
    let (action_top, action_bottom, action_left, action_right) = (action.top, action.bottom, action.left, action.right);
    if win::rect_contains(&lock_rect(dpi), POINT { x, y }) {
        toggle_lock(hwnd);
    } else if EXPANDED.get() && win::rect_contains(&arrow_rect(dpi), POINT { x, y }) {
        set_expanded(hwnd, false);
    } else if x >= action_left && x < action_right && y >= action_top && y < action_bottom {
        cancel_dwell(hwnd);
        if !locked() && !ACTION_HOVER.replace(true) {
            finish_visibility(hwnd);
        }
        ACTION_INSIDE.set(true);
    } else if !locked() && EXPANDED.get() && y >= action_bottom + scale(GAP, dpi) {
        let entries = rows();
        let start = action_bottom + scale(GAP, dpi);
        let visible = visible_row_count(dpi, entries.len());
        let row_height = scale(ROW_H, dpi).max(1);
        let offset = ((y - start).max(0) / row_height) as usize;
        let index = SCROLL.with(Cell::get).saturating_add(offset);
        if offset < visible {
            if let Some((entry, _)) = entries.get(index).cloned() {
                match entry {
                    Entry::Target(which) => circles::open_target(which),
                }
            }
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_PAINT => {
            paint(hwnd);
            0
        }
        // paint() supplies an opaque completed frame; erase must never flash first.
        WM_ERASEBKGND => 1,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as isize,
        WM_MOUSEMOVE => {
            if dock_drag::active() {
                if wparam & 1 != 0 {
                    dock_drag::move_native(screen_point(hwnd, lparam));
                }
                return 0;
            }
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
            let x = win::x_lparam(lparam);
            let y = win::y_lparam(lparam);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let action = action_rect(dpi);
            let (action_top, action_bottom, action_left, action_right) = (action.top, action.bottom, action.left, action.right);
            let hovering_action = !locked()
                && !win::rect_contains(&lock_rect(dpi), POINT { x, y })
                && x >= action_left
                && x < action_right
                && y >= action_top
                && y < action_bottom;
            if hovering_action && !ACTION_INSIDE.replace(true) {
                ACTION_START.set(Some(crate::diag::since_start_ms()));
                unsafe { SetTimer(hwnd, ACTION_TIMER, 16, None) };
            } else if !hovering_action {
                cancel_dwell(hwnd);
                ACTION_HOVER.set(false);
                ACTION_INSIDE.set(false);
            }
            let entries = rows();
            let list_top = action_bottom + scale(GAP, dpi);
            let scroll = SCROLL.with(Cell::get);
            let list_index = if EXPANDED.get() && y >= list_top {
                Some(scroll.saturating_add(((y - list_top) / scale(ROW_H, dpi).max(1)) as usize))
            } else {
                None
            };
            let next_hover = list_index
                .filter(|_| !locked())
                .filter(|&i| i < entries.len() && i < scroll + visible_row_count(dpi, entries.len()))
                .map_or(-1, |i| i as isize);
            let previous_hover = HOVER_ROW.replace(next_hover);
            if previous_hover != next_hover {
                if previous_hover >= 0 {
                    let rect = row_rect(dpi, previous_hover as usize);
                    unsafe { InvalidateRect(hwnd, &rect, 0) };
                }
                if next_hover >= 0 {
                    let rect = row_rect(dpi, next_hover as usize);
                    unsafe { InvalidateRect(hwnd, &rect, 0) };
                }
            }
            0
        }
        WM_MOUSELEAVE => {
            TRACKING.set(false);
            cancel_dwell(hwnd);
            ACTION_INSIDE.set(false);
            ACTION_HOVER.set(false);
            let old_hover = HOVER_ROW.replace(-1);
            if old_hover >= 0 {
                let dpi = GetDpiForWindow(hwnd).max(96);
                let rect = row_rect(dpi, old_hover as usize);
                unsafe { InvalidateRect(hwnd, &rect, 0) };
            }
            0
        }
        WM_TIMER if wparam == ACTION_TIMER => {
            if ACTION_START.get().is_none() {
                return 0; // A killed timer may already have a message queued.
            }
            if dock_drag::active() || locked() {
                cancel_dwell(hwnd);
                return 0;
            }
            let mut cursor = POINT { x: 0, y: 0 };
            let dpi = GetDpiForWindow(hwnd).max(96);
            let action = action_rect(dpi);
            let inside = GetCursorPos(&mut cursor) != 0
                && ScreenToClient(hwnd, &mut cursor) != 0
                && win::rect_contains(&action, cursor)
                && !win::rect_contains(&lock_rect(dpi), cursor);
            if !inside {
                cancel_dwell(hwnd);
                ACTION_INSIDE.set(false);
                ACTION_HOVER.set(false);
            } else if ACTION_START.get().is_some_and(|t| crate::diag::since_start_ms() - t >= ACTION_DWELL_MS as f64) {
                if !ACTION_HOVER.replace(true) {
                    finish_visibility(hwnd);
                } else {
                    cancel_dwell(hwnd);
                }
            }
            InvalidateRect(hwnd, &action, 0);
            0
        }
        WM_LBUTTONDOWN => {
            dock_drag::prepare_click();
            cancel_dwell(hwnd);
            let dpi = GetDpiForWindow(hwnd).max(96);
            let point = POINT { x: win::x_lparam(lparam), y: win::y_lparam(lparam) };
            let heading = EXPANDED.get() && point.y >= 0 && point.y < scale(HEADER_H, dpi) && !win::rect_contains(&arrow_rect(dpi), point);
            if !win::rect_contains(&lock_rect(dpi), point) && (heading || win::rect_contains(&action_rect(dpi), point)) {
                let r = win::window_rect(hwnd);
                let action = action_rect(dpi);
                dock_drag::begin(
                    hwnd,
                    screen_point(hwnd, lparam),
                    (r.left + (r.right - r.left) / 2, r.top + (action.top + action.bottom) / 2),
                    POSITION.get(),
                );
            }
            0
        }
        WM_LBUTTONUP => {
            if !dock_drag::release_native(screen_point(hwnd, lparam)) {
                click_client(hwnd, win::x_lparam(lparam), win::y_lparam(lparam));
            }
            0
        }
        WM_CANCELMODE | WM_CAPTURECHANGED => {
            dock_drag::cancel();
            0
        }
        WM_KEYDOWN if wparam == 0x1b => {
            dock_drag::cancel();
            0
        }
        dock_drag::WM_POINTER => {
            dock_drag::dispatch(wparam);
            0
        }
        WM_RBUTTONUP => {
            cancel_dwell(hwnd);
            let menu = CreatePopupMenu();
            if !menu.is_null() {
                let label = win::wide(if EXPANDED.get() { "Menüyü gizle" } else { "Menüyü göster" });
                AppendMenuW(menu, MF_STRING, 1, label.as_ptr());
                AppendMenuW(menu, MF_STRING, 2, win::wide(if locked() { "Kilidi çöz" } else { "Kilitle" }).as_ptr());
                let mut point = POINT { x: 0, y: 0 };
                GetCursorPos(&mut point);
                SetForegroundWindow(hwnd);
                let command =
                    TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, std::ptr::null());
                DestroyMenu(menu);
                if command == 2 {
                    toggle_lock(hwnd);
                }
                if command == 1 {
                    set_expanded(hwnd, !EXPANDED.get());
                }
            }
            0
        }
        WM_MOUSEWHEEL if !EXPANDED.get() => 0,
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
            let old_hover = HOVER_ROW.replace(-1);
            if old_hover >= 0 {
                let rect = row_rect(GetDpiForWindow(hwnd).max(96), old_hover as usize);
                unsafe { InvalidateRect(hwnd, &rect, 0) };
            }
            let list_top = scale(HEADER_H + GAP + ACTION_SIZE + GAP, GetDpiForWindow(hwnd).max(96));
            let mut client: RECT = unsafe { std::mem::zeroed() };
            unsafe { GetClientRect(hwnd, &mut client) };
            let list_rect = win::rect(0, list_top, client.right, client.bottom);
            unsafe { InvalidateRect(hwnd, &list_rect, 0) };
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
            place(hwnd);
            0
        }
        WM_DESTROY => {
            cancel_dwell(hwnd);
            PAINT_BUFFER.with(|slot| slot.borrow_mut().take());
            VISIBILITY_CACHE.with(|slot| slot.borrow_mut().take());
            dock_drag::shutdown();
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
    fn rounded_controls_blend_corners_symmetrically_at_desktop_dpis() {
        let dib = |c: u32| 0xff00_0000 | (c & 255) << 16 | (c & 0xff00) | (c >> 16) & 255;
        for dpi in [96, 120, 144, 192] {
            for (width, height, radius, color) in [(48, 48, 8, TEAL), (48, 48, 8, SWITCH_BG), (137, 28, 6, ROW_HOVER)] {
                let (w, h) = (scale(width, dpi), scale(height, dpi));
                let mut surface = rounded_surface(w, h, scale(radius, dpi), color).expect("native control surface");
                let pixels = surface.pixels();
                assert_eq!(pixels[0], dib(BG), "outside corner must retain the dock background");
                assert_eq!(pixels[(h / 2 * w + w / 2) as usize], dib(color));
                assert_eq!(pixels[(w / 2) as usize], dib(color), "straight edge must stay crisp");
                let mut blended = 0;
                for y in 0..h {
                    for x in 0..w {
                        let p = pixels[(y * w + x) as usize];
                        assert_eq!(p, pixels[(y * w + w - 1 - x) as usize]);
                        assert_eq!(p, pixels[((h - 1 - y) * w + x) as usize]);
                        if p != dib(BG) && p != dib(color) {
                            blended += 1;
                            for shift in [0, 8, 16] {
                                let channel = (p >> shift) & 255;
                                let (a, b) = ((dib(BG) >> shift) & 255, (dib(color) >> shift) & 255);
                                assert!((a.min(b)..=a.max(b)).contains(&channel), "no dark/color fringes");
                            }
                        }
                    }
                }
                assert!(blended >= 8, "rounded corners must contain intermediate coverage pixels at {dpi} DPI");
            }
        }
    }

    #[test]
    fn lock_joins_are_connected_rounded_and_antialiased_at_desktop_dpis() {
        for closed in [false, true] {
            let x = if closed { 6.8 } else { 8.6 };
            let start = if closed { 6.8 } else { 5.5 };
            for step in 0..=100 {
                let y = start + (9.0 - start) * step as f32 / 100.0;
                assert!(lock_distance(x, y, closed) < 0.01, "shackle/body joint must have no gap");
            }
            for dpi in [96, 120, 144, 192] {
                let size = scale(20, dpi);
                let pixels = lock_surface(size, BG, closed).unwrap().pixels().to_vec();
                let dib = |c: u32| 0xff00_0000 | (c & 255) << 16 | (c & 0xff00) | (c >> 16) & 255;
                let background = dib(BG);
                let foreground = dib(if closed { TEAL } else { SUBTEXT });
                let partial = pixels.iter().filter(|&&p| p != background && p != foreground).count();
                assert!(partial > 15, "rounded edges need subpixel coverage at {dpi} DPI");
                assert!(pixels.iter().all(|p| p >> 24 == 255));
                if closed {
                    for row in pixels.chunks(size as usize) {
                        assert!(row.iter().eq(row.iter().rev()), "closed lock must remain symmetric at {dpi} DPI");
                    }
                }
            }
        }
        assert!(lock_distance(15.0, 7.4, false) > 0.7, "open shackle must keep a visible gap");
        assert!(lock_distance(13.2, 8.0, true) < 0.01);
    }

    #[test]
    fn visibility_dwell_reverses_exactly_without_changing_corner_coverage() {
        for dpi in [96, 120, 144, 192] {
            let size = scale(48, dpi);
            let render = |visible, progress, locked| visibility_surface(size, dpi, visible, progress, locked).unwrap().pixels().to_vec();
            let hidden = render(false, None, false);
            let shown = render(true, None, false);
            assert_ne!(hidden, shown);
            assert_eq!(render(false, Some(0.0), false), hidden);
            assert_eq!(render(false, Some(1.0), false), shown);
            assert_eq!(render(true, Some(0.0), false), shown);
            assert_eq!(render(true, Some(1.0), false), hidden);
            assert_eq!(render(true, Some(0.5), false), render(false, Some(0.5), false));
            assert_ne!(render(false, Some(0.5), false), hidden);
            assert_ne!(render(true, None, true), shown);
            assert_eq!(hidden[0], shown[0]);
            assert!(shown.iter().all(|p| p >> 24 == 255));
        }
    }

    #[test]
    fn completed_dwell_never_flashes_old_or_partial_board_state() {
        for target in [false, true] {
            for actual in [false, true] {
                for progress in [None, Some(0.0), Some(0.5), Some(1.0)] {
                    assert_eq!(visibility_frame(actual, progress, Some(target)), (target, None));
                    assert_eq!(visibility_frame(actual, progress, None), (actual, progress));
                }
            }
        }
    }

    #[test]
    fn dock_dimensions_scale_with_dpi() {
        EXPANDED.set(true);
        let width = dimensions(96, 1).0;
        assert!(width > ACTION_SIZE + PAD * 2);
        assert!(dimensions(192, 8).0 >= width * 2 - 4);
        EXPANDED.set(false);
        assert_eq!(dimensions(96, 8), (64, 64));
        assert_eq!(dimensions(192, 8), (128, 128));
        let action = action_rect(192);
        assert_eq!((action.left, action.top, 128 - action.right, 128 - action.bottom), (16, 16, 16, 16));
        EXPANDED.set(true);
        assert_eq!(scale(ACTION_DWELL_MS as i32, 192), 600);
        assert_eq!(scale(ROW_H, 192), 56);
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
