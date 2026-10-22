//! Small, persistent recovery dock for the circles UI.
//!
//! Windows' Show Desktop commands can hide desktop-organiser windows. The dock stays
//! in the topmost band and gives the user a direct way to reveal or hide the circles.

use std::cell::{Cell, RefCell};

use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE};
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect, FrameRect, InvalidateRect, ScreenToClient, SelectObject,
    SetBkMode, SetTextColor, DT_LEFT, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, HFONT, PAINTSTRUCT, TRANSPARENT,
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
const UI_TIMER: usize = 2;
const UI_MS: f64 = 190.0;

#[derive(Clone, Copy)]
struct PanelEase {
    from: RECT,
    to: RECT,
    action: (RECT, RECT),
    lock: (RECT, RECT),
    menu: (RECT, RECT),
    start: f64,
    progress: f32,
    content: (f32, f32),
}
impl PanelEase {
    fn amount(self) -> f32 {
        self.progress
    }
    fn content(self) -> f32 {
        self.content.0 + (self.content.1 - self.content.0) * self.progress
    }
}
fn mix_rect(a: RECT, b: RECT, t: f32) -> RECT {
    let mix = |x: i32, y: i32| (x as f32 + (y - x) as f32 * t).round() as i32;
    win::rect(mix(a.left, b.left), mix(a.top, b.top), mix(a.right, b.right), mix(a.bottom, b.bottom))
}
#[derive(Clone, Copy)]
struct HoverEase<const N: usize> {
    from: [f32; N],
    to: [f32; N],
    start: f64,
}
impl<const N: usize> HoverEase<N> {
    fn value(self, now: f64) -> [f32; N] {
        let t = circles::geom::ease_out_cubic((now - self.start) / 120.0) as f32;
        std::array::from_fn(|i| self.from[i] + (self.to[i] - self.from[i]) * t)
    }
    fn done(self, now: f64) -> bool {
        now - self.start >= 120.0
    }
}
struct LockFrames {
    size: i32,
    open: circles::surface::Surface,
    closed: circles::surface::Surface,
    frame: circles::surface::Surface,
}

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
    static UI_RUNNING: Cell<bool> = const { Cell::new(false) };
    static TOP_ANCHOR: Cell<Option<i32>> = const { Cell::new(None) };
    static PANEL_EASE: Cell<Option<PanelEase>> = const { Cell::new(None) };
    static HEADER_HOVER: Cell<u8> = const { Cell::new(0) };
    static CONTROL_EASE: Cell<Option<HoverEase<2>>> = const { Cell::new(None) };
    static ROW_EASE: Cell<Option<HoverEase<MAX_ROWS>>> = const { Cell::new(None) };
    static LOCK_FRAMES: RefCell<Option<LockFrames>> = const { RefCell::new(None) };
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

pub(crate) fn context_labels() -> (&'static str, &'static str) {
    (
        if EXPANDED.get() { "Men\u{fc}y\u{fc} gizle" } else { "Men\u{fc}y\u{fc} g\u{f6}ster" },
        if locked() { "Kilidi \u{e7}\u{f6}z" } else { "Kilitle" },
    )
}

pub(crate) fn context_action(lock: bool) {
    let hwnd = WINDOW.get();
    if lock {
        toggle_lock(hwnd);
    } else {
        set_expanded(hwnd, !EXPANDED.get());
    }
}

pub fn locked() -> bool {
    app::app().board.try_borrow().map(|b| b.dock_locked).unwrap_or(true)
}

fn toggle_lock(hwnd: HWND) {
    cancel_dwell(hwnd);
    ACTION_INSIDE.set(false);
    ACTION_HOVER.set(false);
    set_row_hover(hwnd, -1, unsafe { GetDpiForWindow(hwnd) }.max(96));
    let changed = if let Ok(mut board) = app::app().board.try_borrow_mut() {
        board.dock_locked = !board.dock_locked;
        true
    } else {
        false
    };
    if changed {
        circles::ring::close_now();
        app::app().save_config();
        if circles::any_visible() {
            circles::refresh_all();
        }
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

fn mode_lock(dpi: u32, expanded: bool) -> RECT {
    if expanded {
        win::rect(scale(4, dpi), scale(7, dpi), scale(30, dpi), scale(33, dpi))
    } else {
        win::rect(scale(1, dpi), scale(1, dpi), scale(27, dpi), scale(27, dpi))
    }
}
fn lock_rect(dpi: u32) -> RECT {
    PANEL_EASE.get().map_or_else(|| mode_lock(dpi, EXPANDED.get()), |a| mix_rect(a.lock.0, a.lock.1, a.amount()))
}
fn mode_menu(dpi: u32, expanded: bool) -> RECT {
    let width = if expanded { expanded_width(dpi) } else { scale(ACTION_SIZE + 2 * PAD, dpi) };
    if expanded {
        win::rect(width - scale(28, dpi), scale(9, dpi), width - scale(6, dpi), scale(31, dpi))
    } else {
        win::rect(width - scale(22, dpi), scale(1, dpi), width - scale(2, dpi), scale(21, dpi))
    }
}
fn arrow_rect(dpi: u32) -> RECT {
    PANEL_EASE.get().map_or_else(|| mode_menu(dpi, EXPANDED.get()), |a| mix_rect(a.menu.0, a.menu.1, a.amount()))
}
fn dimensions(dpi: u32, count: usize) -> (i32, i32) {
    if !EXPANDED.get() {
        let side = scale(ACTION_SIZE + PAD * 2, dpi);
        return (side, side);
    }
    let visible = visible_row_count(dpi, count) as i32;
    (expanded_width(dpi), scale(HEADER_H + GAP + ACTION_SIZE + GAP + visible * ROW_H + PAD, dpi))
}
fn mode_action(dpi: u32, expanded: bool) -> RECT {
    let size = scale(ACTION_SIZE, dpi);
    let left = if expanded { (expanded_width(dpi) - size) / 2 } else { scale(PAD, dpi) };
    let top = scale(if expanded { HEADER_H + GAP } else { PAD }, dpi);
    win::rect(left, top, left + size, top + size)
}
fn action_rect(dpi: u32) -> RECT {
    PANEL_EASE.get().map_or_else(|| mode_action(dpi, EXPANDED.get()), |a| mix_rect(a.action.0, a.action.1, a.amount()))
}
fn start_ui(hwnd: HWND) {
    if !UI_RUNNING.replace(true) && unsafe { SetTimer(hwnd, UI_TIMER, 16, None) } == 0 {
        UI_RUNNING.set(false);
    }
}
fn set_expanded(hwnd: HWND, expanded: bool) {
    if EXPANDED.get() == expanded {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let from = win::window_rect(hwnd);
    let from_action = action_rect(dpi);
    let from_lock = lock_rect(dpi);
    let from_menu = arrow_rect(dpi);
    let from_content = PANEL_EASE.get().map_or(f32::from(EXPANDED.get()), PanelEase::content);
    PANEL_EASE.set(None);
    EXPANDED.set(expanded);
    TOP_ANCHOR.set(Some(from.top));
    cancel_dwell(hwnd);
    ACTION_INSIDE.set(false);
    ACTION_HOVER.set(false);
    set_row_hover(hwnd, -1, dpi);
    let to = placement(hwnd);
    TOP_ANCHOR.set(Some(to.top));
    PANEL_EASE.set(Some(PanelEase {
        from,
        to,
        action: (from_action, mode_action(dpi, expanded)),
        lock: (from_lock, mode_lock(dpi, expanded)),
        menu: (from_menu, mode_menu(dpi, expanded)),
        start: crate::diag::since_start_ms(),
        progress: 0.0,
        content: (from_content, f32::from(expanded)),
    }));
    start_ui(hwnd);
    unsafe { InvalidateRect(hwnd, std::ptr::null(), 0) };
}
fn control_at(dpi: u32, point: POINT) -> u8 {
    if win::rect_contains(&lock_rect(dpi), point) {
        1
    } else if win::rect_contains(&arrow_rect(dpi), point) {
        2
    } else {
        0
    }
}
fn control_weights(now: f64) -> [f32; 2] {
    CONTROL_EASE.get().map_or_else(|| std::array::from_fn(|i| f32::from(HEADER_HOVER.get() == i as u8 + 1)), |a| a.value(now))
}
fn set_control_hover(hwnd: HWND, next: u8) {
    if HEADER_HOVER.get() == next {
        return;
    }
    let now = crate::diag::since_start_ms();
    let from = control_weights(now);
    HEADER_HOVER.set(next);
    CONTROL_EASE.set(Some(HoverEase { from, to: std::array::from_fn(|i| f32::from(next == i as u8 + 1)), start: now }));
    start_ui(hwnd);
}
fn row_weights(now: f64) -> [f32; MAX_ROWS] {
    ROW_EASE.get().map_or_else(|| std::array::from_fn(|i| f32::from(HOVER_ROW.get() == (SCROLL.get() + i) as isize)), |a| a.value(now))
}
fn set_row_hover(hwnd: HWND, next: isize, _dpi: u32) {
    if HOVER_ROW.get() == next {
        return;
    }
    let now = crate::diag::since_start_ms();
    let from = row_weights(now);
    HOVER_ROW.set(next);
    ROW_EASE.set(Some(HoverEase { from, to: std::array::from_fn(|i| f32::from(next == (SCROLL.get() + i) as isize)), start: now }));
    start_ui(hwnd);
}
fn ui_tick(hwnd: HWND) {
    if PANEL_EASE.get().is_none() && CONTROL_EASE.get().is_none() && ROW_EASE.get().is_none() {
        UI_RUNNING.set(false);
        unsafe { KillTimer(hwnd, UI_TIMER) };
        return;
    }
    let now = crate::diag::since_start_ms();
    let mut active = false;
    if let Some(mut a) = PANEL_EASE.get() {
        a.progress = circles::geom::ease_out_cubic((now - a.start) / UI_MS) as f32;
        let done = now - a.start >= UI_MS;
        let rect = if done { a.to } else { mix_rect(a.from, a.to, a.amount()) };
        if done {
            PANEL_EASE.set(None);
        } else {
            PANEL_EASE.set(Some(a));
            active = true;
        }
        unsafe { SetWindowPos(hwnd, HWND_TOPMOST, rect.left, rect.top, rect.right - rect.left, rect.bottom - rect.top, SWP_NOACTIVATE) };
    }
    if let Some(a) = CONTROL_EASE.get() {
        if a.done(now) {
            CONTROL_EASE.set(None);
        } else {
            active = true;
        }
    }
    if let Some(a) = ROW_EASE.get() {
        if a.done(now) {
            ROW_EASE.set(None);
        } else {
            active = true;
        }
    }
    if !active {
        UI_RUNNING.set(false);
        unsafe { KillTimer(hwnd, UI_TIMER) };
    }
    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
        // Present the same sampled geometry before processing the next timer tick.
        windows_sys::Win32::Graphics::Gdi::UpdateWindow(hwnd);
    };
}

fn row_rect(dpi: u32, index: usize) -> RECT {
    let scroll = SCROLL.with(Cell::get);
    let top = scale(HEADER_H + GAP + ACTION_SIZE + GAP, dpi) + index.saturating_sub(scroll) as i32 * scale(ROW_H, dpi);
    win::rect(scale(8, dpi), top, expanded_width(dpi) - scale(PAD, dpi), top + scale(ROW_H, dpi))
}

fn visible_row_count(dpi: u32, count: usize) -> usize {
    let work = work_area();
    let row = scale(ROW_H, dpi).max(1);
    let fixed = scale(HEADER_H + GAP + ACTION_SIZE + GAP + PAD, dpi);
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

fn placement(hwnd: HWND) -> RECT {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let size = dimensions(dpi, rows().len());
    let (monitor, work) = monitor_area();
    let at = POSITION.get().unwrap_or((work.right, work.top + (work.bottom - work.top) / 2));
    let edge = dock_layout::edge(bounds(monitor), bounds(work), at);
    let action = mode_action(dpi, EXPANDED.get());
    let (x, mut y) = dock_layout::position(bounds(work), edge, at, size, scale(12, dpi), (action.top + action.bottom) / 2);
    if let Some(top) = TOP_ANCHOR.get() {
        let min = (work.top + scale(12, dpi)).min((work.bottom - size.1).max(work.top));
        let max = (work.bottom - size.1 - scale(12, dpi)).max(min);
        y = top.clamp(min, max);
    }
    win::rect(x, y, x + size.0, y + size.1)
}
fn place(hwnd: HWND) {
    if PANEL_EASE.get().is_some() {
        return;
    }
    let rect = placement(hwnd);
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
}
pub fn current_top_anchor() -> Option<i32> {
    TOP_ANCHOR.get()
}
pub fn restore_anchor(hwnd: HWND, at: Option<(i32, i32)>, top: Option<i32>) {
    PANEL_EASE.set(None);
    TOP_ANCHOR.set(top);
    POSITION.set(at);
    place(hwnd);
}
pub fn move_anchor(hwnd: HWND, at: Option<(i32, i32)>) {
    restore_anchor(hwnd, at, None);
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
        hIcon: crate::win::app_icon(false),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_HAND) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: crate::win::app_icon(true),
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
    WINDOW.with(|window| {
        let hwnd = window.get();
        if !hwnd.is_null() && unsafe { IsWindow(hwnd) } != 0 {
            if EXPANDED.get() {
                place(hwnd);
            } else {
                set_expanded(hwnd, true);
            }
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

#[cfg(test)]
pub(crate) fn lock_surface(size: i32, color: u32, is_locked: bool) -> Option<circles::surface::Surface> {
    lock_surface_tint(size, color, is_locked, if is_locked { TEAL } else { SUBTEXT })
}

pub(crate) fn lock_surface_tint(size: i32, color: u32, is_locked: bool, ink: u32) -> Option<circles::surface::Surface> {
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
            blend_pixel(pixel, ink, covered as f32 / 16.0);
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

unsafe fn draw_lock(dc: windows_sys::Win32::Graphics::Gdi::HDC, r: RECT, dpi: u32, is_locked: bool, background: u32) {
    let side = scale(26, dpi);
    LOCK_FRAMES.with(|slot| {
        let mut cache = slot.borrow_mut();
        if cache.as_ref().is_none_or(|c| c.size != side) {
            *cache = (|| {
                Some(LockFrames {
                    size: side,
                    open: lock_surface_tint(side, 0, false, 0x00ff_ffff)?,
                    closed: lock_surface_tint(side, 0, true, 0x00ff_ffff)?,
                    frame: circles::surface::Surface::new(side, side)?,
                })
            })();
        }
        if let Some(c) = cache.as_mut() {
            let mask = if is_locked { c.closed.pixels() } else { c.open.pixels() };
            let frame = c.frame.pixels();
            let ink = if is_locked { TEAL } else { SUBTEXT };
            for (out, coverage) in frame.iter_mut().zip(mask.iter()) {
                *out = 0xff00_0000 | (background & 255) << 16 | background & 0xff00 | (background >> 16) & 255;
                blend_pixel(out, ink, (*coverage & 255) as f32 / 255.0);
            }
            unsafe {
                windows_sys::Win32::Graphics::Gdi::BitBlt(
                    dc,
                    (r.left + r.right - side) / 2,
                    (r.top + r.bottom - side) / 2,
                    side,
                    side,
                    c.frame.dc(),
                    0,
                    0,
                    windows_sys::Win32::Graphics::Gdi::SRCCOPY,
                )
            };
        }
    });
}

unsafe fn draw_menu(dc: windows_sys::Win32::Graphics::Gdi::HDC, rect: RECT, dpi: u32, expanded: bool, background: u32) {
    use windows_sys::Win32::Graphics::Gdi::{BitBlt, SRCCOPY};
    let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
    let Some(mut surface) = rounded_surface(w, h, scale(5, dpi), background) else { return };
    let factor = dpi as f32 / 96.0;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let direction = if expanded { -1.0 } else { 1.0 };
    let pixels = surface.pixels();
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let qx = (px - cx).abs() - (cx - 4.5 * factor);
            let qy = (py - cy).abs() - (cy - 4.5 * factor);
            let edge = (qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - 3.5 * factor).abs();
            let a = (cx - 5.0 * factor, cy - direction * 2.5 * factor);
            let b = (cx, cy + direction * 2.5 * factor);
            let c = (cx + 5.0 * factor, a.1);
            let glyph = line_distance(px, py, a, b).min(line_distance(px, py, b, c));
            let coverage = (factor * 0.8 + 0.5 - glyph).clamp(0.0, 1.0);
            let border = (factor * 0.5 + 0.5 - edge).clamp(0.0, 1.0);
            blend_pixel(&mut pixels[(y * w + x) as usize], TEAL, coverage.max(border * 0.65));
        }
    }
    unsafe { BitBlt(dc, rect.left, rect.top, w, h, surface.dc(), 0, 0, SRCCOPY) };
}

fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = unsafe { std::mem::zeroed() };
    let window_dc = unsafe { BeginPaint(hwnd, &mut ps) };
    let mut client: RECT = unsafe { std::mem::zeroed() };
    unsafe { GetClientRect(hwnd, &mut client) };
    // Own the buffer locally: painting/layout may pump messages, so no RefCell
    // borrow may span the render. Reuse its allocation until the client resizes.
    let mut buffer = PAINT_BUFFER.with(|slot| slot.borrow_mut().take());
    let (buffer_w, buffer_h) = PANEL_EASE.get().map_or((client.right, client.bottom), |a| {
        ((a.from.right - a.from.left).max(a.to.right - a.to.left), (a.from.bottom - a.from.top).max(a.to.bottom - a.to.top))
    });
    if buffer.as_ref().is_none_or(|s| s.w != buffer_w || s.h != buffer_h) {
        buffer = circles::surface::Surface::new(buffer_w, buffer_h);
    }
    let hdc = buffer.as_ref().map_or(window_dc, |s| s.dc());
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let entries = rows();
    let count = entries.len();
    let visible_count = visible_row_count(dpi, count);
    let scroll = SCROLL.with(Cell::get).min(count.saturating_sub(visible_count));
    let row_h = scale(ROW_H, dpi);
    let header_h = scale(HEADER_H, dpi);
    let content_weight = PANEL_EASE.get().map_or(f32::from(EXPANDED.get()), PanelEase::content);

    let bg = unsafe { CreateSolidBrush(BG) };
    let accent = unsafe { CreateSolidBrush(ACCENT) };
    let switch_on = circles::any_visible();
    let font = icon_title_font(dpi);
    let old_font = if font.is_null() { std::ptr::null_mut() } else { unsafe { SelectObject(hdc, font) } };
    let switch_rect = action_rect(dpi);
    let border = win::rect(0, 0, client.right, client.bottom);
    unsafe {
        FillRect(hdc, &client, bg);
        FrameRect(hdc, &border, accent);
        SetBkMode(hdc, TRANSPARENT as i32);
        if content_weight > 0.0 {
            let heading_font = crate::theme::font(dpi, true, 100);
            if !heading_font.is_null() {
                SelectObject(hdc, heading_font);
            }
            SetTextColor(hdc, mix_color(BG, TEXT, content_weight));
            let mut title = win::wide("DeskTuk");
            let mut title_rect = win::rect(
                lock_rect(dpi).right + scale(6, dpi),
                scale(2, dpi),
                arrow_rect(dpi).left - scale(4, dpi),
                header_h - scale(2, dpi),
            );
            DrawTextW(hdc, title.as_mut_ptr(), -1, &mut title_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            SelectObject(hdc, if font.is_null() { old_font } else { font });
            if !heading_font.is_null() {
                DeleteObject(heading_font);
            }
        }
        let is_locked = locked();
        let progress = ACTION_START
            .get()
            .map(|start| circles::geom::ease_out_cubic((crate::diag::since_start_ms() - start) / ACTION_DWELL_MS as f64) as f32);
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
        let now = crate::diag::since_start_ms();
        let control_hover = control_weights(now);
        for (index, rect) in [lock_rect(dpi), arrow_rect(dpi)].into_iter().enumerate() {
            if control_hover[index] > 0.0 {
                rounded_fill(hdc, rect, mix_color(BG, ROW_HOVER, control_hover[index]), scale(6, dpi));
            }
        }
        draw_menu(hdc, arrow_rect(dpi), dpi, EXPANDED.get(), mix_color(BG, ROW_HOVER, control_hover[1]));
        draw_lock(hdc, lock_rect(dpi), dpi, is_locked, mix_color(BG, ROW_HOVER, control_hover[0]));
        SetTextColor(hdc, TEXT);
        if content_weight > 0.0 {
            SetTextColor(hdc, mix_color(BG, SUBTEXT, content_weight));
            for visible_index in 0..visible_count.min(count.saturating_sub(scroll)) {
                let index = scroll + visible_index;
                let top = scale(HEADER_H + GAP + ACTION_SIZE + GAP, dpi) + visible_index as i32 * row_h;
                let row = win::rect(scale(8, dpi), top, client.right - scale(8, dpi), top + row_h);
                let weight = row_weights(now)[visible_index];
                if weight > 0.0 {
                    rounded_fill(hdc, row, mix_color(BG, ROW_HOVER, weight), scale(6, dpi));
                }
                let mut label = win::wide(&entries[index].1);
                let mut label_rect = win::rect(scale(16, dpi), top, client.right - scale(8, dpi), top + row_h);
                DrawTextW(hdc, label.as_mut_ptr(), -1, &mut label_rect, DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX);
            }
        }
        DeleteObject(bg);
        DeleteObject(accent);
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
    } else if win::rect_contains(&arrow_rect(dpi), POINT { x, y }) {
        set_expanded(hwnd, !EXPANDED.get());
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
            set_control_hover(hwnd, control_at(dpi, POINT { x, y }));
            let hovering_action = !locked()
                && control_at(dpi, POINT { x, y }) == 0
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
            set_row_hover(hwnd, next_hover, dpi);
            0
        }
        WM_MOUSELEAVE => {
            TRACKING.set(false);
            cancel_dwell(hwnd);
            ACTION_INSIDE.set(false);
            ACTION_HOVER.set(false);
            set_control_hover(hwnd, 0);
            set_row_hover(hwnd, -1, GetDpiForWindow(hwnd).max(96));
            0
        }
        WM_TIMER if wparam == UI_TIMER => {
            ui_tick(hwnd);
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
                && control_at(dpi, cursor) == 0;
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
            if control_at(dpi, point) == 0 && (heading || win::rect_contains(&action_rect(dpi), point)) {
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
            let mut point = POINT { x: 0, y: 0 };
            GetCursorPos(&mut point);
            circles::dock_menu(point);
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
            ROW_EASE.set(None);
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
            PANEL_EASE.set(None);
            place(hwnd);
            0
        }
        WM_DPICHANGED => {
            PANEL_EASE.set(None);
            place(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, UI_TIMER);
            UI_RUNNING.set(false);
            PANEL_EASE.set(None);
            CONTROL_EASE.set(None);
            ROW_EASE.set(None);
            LOCK_FRAMES.with(|slot| slot.borrow_mut().take());
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
