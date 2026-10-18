//! The open ring: at most one layered window, owned by the circle it opened from (so it
//! always stays just above that circle). It shows one page of slots, reveals itself with
//! a short animation, follows the wheel, and closes when the pointer leaves the disc.
//!
//! State lives in `RING`. Every access goes through `with`, which uses `try_borrow_mut`
//! and simply skips the message if the state is busy. Nothing that can pump messages,
//! open a menu or destroy a window runs inside `with` - those calls happen after it
//! returns. (`UpdateLayeredWindow` does run inside: it only sends `WM_WINDOWPOS*`, which
//! `ring_proc` passes to `DefWindowProcW` without touching `RING`.)

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, ScreenToClient, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::circle_win::{self, Which};
use super::draw::{self, RingView, SlotLook, SlotView};
use super::geom::{self, RingHit};
use super::suggest::Suggestion;
use super::surface::Surface;
use super::view::{self, SlotRef, Target};
use crate::app::app;
use crate::items::{self, Item, Meta};
use crate::{diag, dlog, shell_menu, win};

const CLASS: &str = "DeskTuk.Ring";
const TIMER_ANIM: usize = 1;
/// Animation frame interval (only while animating; no timer runs when the ring is still).
const FRAME_MS: u32 = 15;
const IDM_REMOVE: usize = 0x8001;
const IDM_OPEN: usize = 1;
const IDM_REMOVE_PLAIN: usize = 2;
const IDM_ACCEPT: usize = 3;
const IDM_DISMISS: usize = 4;
const IDM_RENAME_GROUP: usize = 5;
const IDM_DELETE_GROUP: usize = 6;

struct PageSlot {
    r: SlotRef,
    /// Shell metadata/icon for `SlotRef::Item` slots.
    item: Option<Item>,
    load_id: u64,
}

#[derive(Clone, Copy)]
struct Anim {
    t0: f64,
    from: f32,
    to: f32,
    duration_ms: f64,
    closing: bool,
}

/// Left button pressed on an item slot; becomes a drag once it moves far enough.
struct Press {
    path: PathBuf,
    cursor: POINT,
    active: bool,
}

struct Ring {
    hwnd: HWND,
    anchor: HWND,
    target: Target,
    refs: Vec<SlotRef>,
    suggestions: Vec<Suggestion>,
    start: usize,
    page: Vec<PageSlot>,
    hover: Option<usize>,
    wheel: i32,
    dpi: u32,
    metrics: geom::Metrics,
    /// Screen position of the ring centre (= the circle centre).
    center: POINT,
    radius: f32,
    anim: Option<Anim>,
    /// Fully drawn and masked frame, kept only while animating.
    full: Option<Vec<u32>>,
    surface: Surface,
    menu_open: bool,
    tracking: bool,
    press: Option<Press>,
}

thread_local! {
    static RING: RefCell<Option<Ring>> = const { RefCell::new(None) };
    static NEXT_LOAD: Cell<u64> = const { Cell::new(0) };
    static OPENING: Cell<bool> = const { Cell::new(false) };
    static MOUSE_HOOK: Cell<HHOOK> = const { Cell::new(std::ptr::null_mut()) };
}

const WM_APP_OUTSIDE_POINTER: u32 = WM_APP + 41;

fn install_mouse_hook() {
    MOUSE_HOOK.with(|hook| {
        if !hook.get().is_null() {
            return;
        }
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        let installed = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), instance, 0) };
        if !installed.is_null() {
            hook.set(installed);
        } else {
            dlog!("ring mouse dismissal hook failed: {}", unsafe { windows_sys::Win32::Foundation::GetLastError() });
        }
    });
}

fn uninstall_mouse_hook() {
    MOUSE_HOOK.with(|hook| {
        let installed = hook.replace(std::ptr::null_mut());
        if !installed.is_null() {
            unsafe { UnhookWindowsHookEx(installed) };
        }
    });
}

unsafe extern "system" fn mouse_hook(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0 {
        let kind = message as u32;
        if let Some((anchor, button_flag, release_message)) = circle_win::pressed_mouse_route() {
            if kind == WM_MOUSEMOVE || kind == release_message {
                if !super::drag_capture::is_active_for(anchor) {
                    let mut point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
                    if unsafe { ScreenToClient(anchor, &mut point) } != 0 {
                        let msg = if kind == WM_MOUSEMOVE { WM_MOUSEMOVE } else { release_message };
                        let keys = if kind == WM_MOUSEMOVE { button_flag } else { 0 };
                        let packed = ((point.x as u16 as u32) | ((point.y as u16 as u32) << 16)) as isize;
                        unsafe { PostMessageW(anchor, msg, keys, packed) };
                    }
                }
                return unsafe { CallNextHookEx(MOUSE_HOOK.get(), code, message, data) };
            }
        }
        if matches!(kind, WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
            let point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
            let outside = with(|ring| hit_screen(ring, point) == RingHit::Outside).unwrap_or(false);
            let pointer_only = kind == WM_MOUSEMOVE;
            if outside && !(pointer_only && crate::dock::contains_screen(point)) {
                let hwnd = with(|ring| ring.hwnd).unwrap_or(std::ptr::null_mut());
                if !hwnd.is_null() {
                    unsafe { PostMessageW(hwnd, WM_APP_OUTSIDE_POINTER, 0, 0) };
                }
            }
        }
    }
    unsafe { CallNextHookEx(MOUSE_HOOK.get(), code, message, data) }
}

struct Opening;
impl Drop for Opening {
    fn drop(&mut self) {
        OPENING.with(|busy| busy.set(false));
    }
}

fn with<R>(f: impl FnOnce(&mut Ring) -> R) -> Option<R> {
    RING.with(|cell| {
        let mut g = cell.try_borrow_mut().ok()?;
        g.as_mut().map(f)
    })
}

pub fn register_class() {
    super::drag_preview::register_class();
    let name = win::wide(CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(ring_proc),
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

fn which_of(t: Target) -> Which {
    match t {
        Target::Recent => Which::Recent,
        Target::Circle(ci) | Target::Group(ci, _) => Which::User(ci),
    }
}

fn top_left(r: &Ring) -> POINT {
    let m = r.metrics;
    POINT { x: r.center.x - m.ring_r, y: r.center.y - m.ring_r }
}

fn closing(r: &Ring) -> bool {
    r.anim.is_some_and(|a| a.closing)
}

/// Whether a ring (not already closing) is open for the circle window `anchor`.
pub fn is_open_for(anchor: HWND) -> bool {
    with(|r| r.anchor == anchor && !closing(r)).unwrap_or(false)
}

pub fn close_target(anchor: HWND, target: Target) -> bool {
    let (matches, is_closing) = with(|r| (r.anchor == anchor && r.target == target, closing(r))).unwrap_or((false, false));
    if matches && !is_closing {
        begin_close();
    }
    matches
}

/// Opens the ring for `target` over the circle window `anchor`, closing any other ring.
pub fn open(anchor: HWND, target: Target) {
    if OPENING.with(|busy| busy.replace(true)) {
        return;
    }
    let _opening = Opening;
    close_now();
    let dpi = unsafe { GetDpiForWindow(anchor) }.max(96);
    // Query Explorer outside RING/board borrows: cross-process COM can pump messages.
    let icon = crate::desktop_view::icon_size()
        .unwrap_or_else(|e| {
            dlog!("desktop icon size unavailable: {e}");
            win::scale(48, dpi)
        })
        .max(48); // prefer the shell's 48 px extra-large image list over a smaller desktop icon.
    if unsafe { IsWindow(anchor) } == 0 {
        return;
    }
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    let max_diameter = if unsafe { GetMonitorInfoW(MonitorFromWindow(anchor, MONITOR_DEFAULTTONEAREST), &mut mi) } != 0 {
        (mi.rcWork.right - mi.rcWork.left).min(mi.rcWork.bottom - mi.rcWork.top) - win::scale(16, dpi)
    } else {
        win::scale(800, dpi)
    };
    let m = geom::desktop_metrics(dpi, icon, max_diameter);
    let wr = win::window_rect(anchor);
    let size = 2 * m.ring_r;
    // Centred on the circle, but shifted inwards near a screen edge so every slot stays reachable.
    let (cx, cy) = ((wr.left + wr.right) / 2, (wr.top + wr.bottom) / 2);
    let (left, top) = circle_win::clamp_to_work((cx - m.ring_r, cy - m.ring_r), size);
    let center = POINT { x: left + m.ring_r, y: top + m.ring_r };
    let Some(surface) = Surface::new(size, size) else { return };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            win::wide(CLASS).as_ptr(),
            win::wide("DeskTuk Ring").as_ptr(),
            WS_POPUP,
            center.x - m.ring_r,
            center.y - m.ring_r,
            size,
            size,
            anchor, // owner: the ring always stays just above its circle
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    };
    if hwnd.is_null() {
        return;
    }
    crate::host::exclude_from_peek(hwnd);
    unsafe { DragAcceptFiles(hwnd, 1) };
    let (refs, suggestions) = super::slots_for(target);
    let mut ring = Ring {
        hwnd,
        anchor,
        target,
        refs,
        suggestions,
        start: 0,
        page: Vec::new(),
        hover: None,
        wheel: 0,
        dpi,
        metrics: m,
        center,
        radius: m.circle_d as f32 / 2.0,
        anim: Some(Anim {
            t0: diag::since_start_ms(),
            from: m.circle_d as f32 / 2.0,
            to: m.ring_r as f32,
            duration_ms: geom::OPEN_MS,
            closing: false,
        }),
        full: None,
        surface,
        menu_open: false,
        tracking: false,
        press: None,
    };
    let to_load = load_page(&mut ring);
    let slots = ring.refs.len();
    let stored = RING.with(|cell| match cell.try_borrow_mut() {
        Ok(mut g) => {
            *g = Some(ring);
            true
        }
        Err(_) => false,
    });
    if !stored {
        unsafe { DestroyWindow(hwnd) };
        return;
    }
    with(redraw);
    spawn_loader(to_load);
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetTimer(hwnd, TIMER_ANIM, FRAME_MS, None);
    }
    install_mouse_hook();
    dlog!("ring opened target={target:?} slots={slots} icon_px={} per_page={} diameter={size}", m.icon, m.per_page());
}

fn spawn_loader(paths: Vec<PathBuf>) {
    let request = NEXT_LOAD.with(|n| {
        let id = n.get().wrapping_add(1);
        n.set(id);
        id
    });
    let px = with(|r| {
        for slot in &mut r.page {
            if let SlotRef::Item(path) = &slot.r {
                if paths.contains(path) {
                    slot.load_id = request;
                }
            }
        }
        r.metrics.icon
    });
    if let Some(px) = px {
        items::spawn_ring_loader(app().controller.get(), paths, px, request);
    }
}

pub fn apply_batch(batch: items::RingBatch) {
    with(|r| {
        if batch.px != r.metrics.icon {
            return;
        }
        let mut any = false;
        for (path, meta, icon) in batch.entries {
            if let Some(slot) = r.page.iter_mut().find(|s| s.load_id == batch.request && s.r == SlotRef::Item(path.clone())) {
                if let Some(item) = &mut slot.item {
                    item.apply(meta);
                    item.set_icon(icon, batch.px);
                    any = true;
                }
            }
        }
        if any && !closing(r) {
            redraw(r);
        }
    });
}

/// Destroys the ring immediately (no animation). Safe to call when none is open.
pub fn close_now() {
    let taken = RING.with(|cell| cell.try_borrow_mut().ok().and_then(|mut g| g.take()));
    if let Some(r) = taken {
        let h = r.hwnd;
        if circle_win::pressed_window().is_none() {
            uninstall_mouse_hook();
        }
        drop(r); // releases icons and the surface
        super::drag_preview::close();
        super::drag_capture::end(h);
        unsafe {
            KillTimer(h, TIMER_ANIM);
            DestroyWindow(h);
            // Let COM unload shell extensions the icons pulled in, then trim the working set.
            windows_sys::Win32::System::Com::CoFreeUnusedLibrariesEx(0, 0);
        }
        crate::app::schedule_trim();
    }
}

fn begin_close() {
    if circle_win::pressed_window().is_some() {
        return;
    }
    with(|r| {
        if closing(r) {
            return;
        }
        let m = r.metrics;
        if r.full.is_none() {
            r.full = Some(r.surface.pixels().to_vec());
        }
        r.anim = Some(Anim {
            t0: diag::since_start_ms(),
            from: r.radius,
            to: m.circle_d as f32 / 2.0,
            duration_ms: geom::CLOSE_MS,
            closing: true,
        });
        unsafe { SetTimer(r.hwnd, TIMER_ANIM, FRAME_MS, None) };
    });
    uninstall_mouse_hook();
}

/// Keep a short-lived global fallback while dragging a circle. A no-activate layered
/// window can be denied foreground mouse capture; the hook then routes only movement
/// and release until the press ends, so the grid and drag cannot become stranded.
pub fn circle_drag_started() {
    install_mouse_hook();
}

pub fn circle_drag_finished() {
    if with(|_| true).is_none() {
        uninstall_mouse_hook();
    }
}

/// Re-reads the slots (after the board or the Desktop changed) and repaints.
pub fn refresh() {
    let to_load = with(|r| {
        let (refs, suggestions) = super::slots_for(r.target);
        r.suggestions = suggestions;
        let mut to_load = Vec::new();
        if refs != r.refs {
            r.refs = refs;
            to_load = load_page(r);
        }
        if !closing(r) {
            redraw(r);
        }
        to_load
    });
    if let Some(paths) = to_load {
        spawn_loader(paths);
    }
}

/// Shell metadata from the loader thread for items on the current page.
/// Re-query visible references when their files are deleted, recreated or updated,
/// even when the board paths and ring slots have not changed.
pub fn on_shell_change(change: &crate::watch::Change) {
    let paths = with(|r| {
        let mut paths: Vec<PathBuf> = r.page.iter().filter_map(|s| s.item.as_ref().map(|i| i.path.clone())).collect();
        crate::watch::apply_change(&mut paths, change)
    });
    if let Some(paths) = paths {
        spawn_loader(paths);
    }
}

/// Shell metadata from the loader thread for items on the current page.
pub fn apply_metas(metas: Vec<(PathBuf, Meta)>) {
    with(|r| {
        let mut any = false;
        for (p, m) in metas {
            if let Some(it) = r.page.iter_mut().filter_map(|s| s.item.as_mut()).find(|i| i.path == p) {
                it.apply(m);
                any = true;
            }
        }
        if any && !closing(r) {
            redraw(r);
        }
    });
}

/// Builds `page` for the current `start`; returns the item paths whose metadata must load.
fn load_page(r: &mut Ring) -> Vec<PathBuf> {
    let range = geom::page_range(r.refs.len(), r.start, r.metrics.per_page());
    r.start = range.start;
    r.hover = None;
    r.page = r.refs[range]
        .iter()
        .map(|sr| PageSlot {
            r: sr.clone(),
            load_id: 0,
            item: match sr {
                SlotRef::Item(p) => Some(Item::placeholder(p.clone())),
                _ => None,
            },
        })
        .collect();
    r.page.iter().filter_map(|s| s.item.as_ref().map(|i| i.path.clone())).collect()
}

fn set_target(r: &mut Ring, t: Target) -> Vec<PathBuf> {
    r.target = t;
    let (refs, suggestions) = super::slots_for(t);
    r.refs = refs;
    r.suggestions = suggestions;
    r.start = 0;
    let to_load = load_page(r);
    redraw(r);
    to_load
}

fn subtitle(r: &Ring) -> String {
    let label = geom::page_label(r.refs.len(), r.start, r.metrics.per_page());
    if !label.is_empty() {
        return label;
    }
    if r.refs.is_empty() {
        let no_circles = app().board.try_borrow().map(|b| b.circles.is_empty()).unwrap_or(false);
        return if r.target == Target::Recent && no_circles { "Right-click: New circle".into() } else { "Drop files here".into() };
    }
    String::new()
}

fn render(r: &mut Ring) {
    let m = r.metrics;
    let mut labels: Vec<String> = Vec::with_capacity(r.page.len());
    let mut looks: Vec<SlotLook> = Vec::with_capacity(r.page.len());
    {
        let board = app().board.try_borrow().ok();
        let circle_name = |ci: usize| board.as_ref().and_then(|b| b.circles.get(ci)).map(|c| c.name.clone()).unwrap_or_default();
        for s in r.page.iter_mut() {
            match (&s.r, s.item.as_mut()) {
                (SlotRef::Item(_), Some(it)) => {
                    labels.push(it.name.clone());
                    looks.push(SlotLook::Item { icon: it.icon(m.icon), dim: it.missing });
                }
                (SlotRef::Group(gi), _) => {
                    let ci = match r.target {
                        Target::Circle(ci) => ci,
                        _ => usize::MAX,
                    };
                    let name = board.as_ref().and_then(|b| b.circles.get(ci)).and_then(|c| c.groups.get(*gi)).map(|g| g.name.clone());
                    labels.push(name.unwrap_or_default());
                    looks.push(SlotLook::Group);
                }
                (SlotRef::Back, _) => {
                    labels.push("Back".into());
                    looks.push(SlotLook::Back);
                }
                (SlotRef::Suggestion(i), _) => match r.suggestions.get(*i) {
                    Some(sg) => {
                        labels.push(format!(".{} \u{2192} {}", sg.ext, circle_name(sg.circle)));
                        looks.push(SlotLook::Suggestion { count: sg.paths.len() });
                    }
                    None => {
                        labels.push(String::new());
                        looks.push(SlotLook::Suggestion { count: 0 });
                    }
                },
                (SlotRef::Item(_), None) => {
                    labels.push(String::new());
                    looks.push(SlotLook::Item { icon: std::ptr::null_mut(), dim: true });
                }
            }
        }
    }
    let title = app().board.try_borrow().map(|b| view::title(&b, r.target)).unwrap_or_default();
    let sub = subtitle(r);
    let views: Vec<SlotView> = labels.iter().zip(looks).map(|(l, look)| SlotView { label: l, look }).collect();
    draw::ring(
        &mut r.surface,
        r.dpi,
        &m,
        &RingView { title: &title, subtitle: &sub, slots: &views, hover: r.hover, recent: r.target == Target::Recent },
    );
    let size = r.surface.w as usize;
    let px = r.surface.pixels();
    geom::mask_disc(px, size, size, m.ring_r as f32);
    if r.anim.is_some() {
        let full = px.to_vec();
        geom::reveal(&full, px, size, size, r.radius);
        r.full = Some(full);
    } else {
        r.full = None;
    }
}

fn redraw(r: &mut Ring) {
    render(r);
    r.surface.present(r.hwnd, top_left(r));
}

fn tick() {
    let now = diag::since_start_ms();
    let closed = with(|r| {
        let Some(a) = r.anim else {
            unsafe { KillTimer(r.hwnd, TIMER_ANIM) };
            return false;
        };
        let (rad, done) = geom::anim_radius(a.from, a.to, now - a.t0, a.duration_ms);
        r.radius = rad;
        if done {
            r.anim = None;
            unsafe { KillTimer(r.hwnd, TIMER_ANIM) };
            if a.closing {
                return true;
            }
        }
        let size = r.surface.w as usize;
        if let Some(full) = r.full.take() {
            let px = r.surface.pixels();
            if r.anim.is_some() {
                geom::reveal(&full, px, size, size, rad);
                r.full = Some(full);
            } else {
                px.copy_from_slice(&full); // finished opening: keep only the surface
            }
        }
        r.surface.present(r.hwnd, top_left(r));
        false
    })
    .unwrap_or(false);
    if closed {
        close_now();
    }
}

/// What is under client point `(x, y)` of the ring window.
fn hit(r: &Ring, x: i32, y: i32) -> RingHit {
    let m = r.metrics;
    geom::ring_hit(&m, r.page.len(), x - m.ring_r, y - m.ring_r)
}

fn hit_screen(r: &Ring, p: POINT) -> RingHit {
    let tl = top_left(r);
    hit(r, p.x - tl.x, p.y - tl.y)
}

fn cursor_pos() -> POINT {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    p
}

/// Starts closing if the pointer is no longer over the ring (after a menu or a drag).
fn close_if_outside() {
    if circle_win::pressed_window().is_some() {
        return;
    }
    let outside = with(|r| hit_screen(r, cursor_pos()) == RingHit::Outside).unwrap_or(false);
    if outside {
        begin_close();
    }
}

fn on_mouse_move(wparam: WPARAM, lparam: LPARAM) {
    // Normally capture delivers these events to the circle. Also route events still
    // delivered to the ring if foreground capture was denied, without holding RING.
    let circle_drag = with(|r| {
        circle_win::pressed_mouse_route()
            .filter(|(anchor, _, _)| *anchor == r.anchor)
            .map(|_| (r.anchor, circle_win::mouse_point(r.hwnd, lparam)))
    })
    .flatten();
    if let Some((anchor, at)) = circle_drag {
        let button_flag = circle_win::pressed_mouse_route().map(|(_, flag, _)| flag).unwrap_or(0);
        if wparam & button_flag != 0 {
            circle_win::move_press(anchor, at);
        }
        return;
    }
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    if with(|r| hit(r, x, y) == RingHit::Outside).unwrap_or(false) {
        begin_close();
        return;
    }
    let preview = with(|r| {
        if closing(r) {
            return None;
        }
        if !r.tracking {
            let mut tme = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: r.hwnd,
                dwHoverTime: 0,
            };
            unsafe { TrackMouseEvent(&mut tme) };
            r.tracking = true;
        }
        let dpi = r.dpi;
        if let Some(p) = r.press.as_mut() {
            if wparam & 1 == 0 {
                return None;
            } // MK_LBUTTON
            let c = circle_win::mouse_point(r.hwnd, lparam);
            let (dx, dy) = (c.x - p.cursor.x, c.y - p.cursor.y);
            let started = !p.active
                && unsafe { dx.abs() > GetSystemMetricsForDpi(SM_CXDRAG, dpi) || dy.abs() > GetSystemMetricsForDpi(SM_CYDRAG, dpi) };
            if started {
                p.active = true;
            }
            if p.active {
                unsafe { SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_SIZEALL)) };
            }
            if p.active {
                let surface = if started {
                    let icon = r.page.iter().filter_map(|s| s.item.as_ref()).find(|it| it.path == p.path).map(|it| it.cached_icon());
                    draw::drag_icon(icon.unwrap_or(std::ptr::null_mut()), dpi)
                } else {
                    None
                };
                return Some((r.hwnd, c, dpi, surface));
            }
            return None;
        }
        let hover = match hit(r, x, y) {
            RingHit::Slot(k) => Some(k),
            _ => None,
        };
        if hover != r.hover {
            r.hover = hover;
            redraw(r);
        }
        None
    })
    .flatten();
    // Creating/moving/destroying the preview can pump messages: RING is released.
    if let Some((owner, at, dpi, surface)) = preview {
        if let Some(surface) = surface {
            super::drag_preview::show(surface, owner, at, dpi);
        } else {
            super::drag_preview::move_to(at);
        }
    }
}

/// Something the ring must do after its state borrow has ended.
enum Action {
    None,
    Open(PathBuf),
    Load(Vec<PathBuf>),
    Accept(Suggestion),
    Drop { path: PathBuf, at: POINT },
}

enum LeftPress {
    Item(HWND),
}

fn on_left_down(lparam: LPARAM) {
    super::rename::commit_active();
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let capture = with(|r| {
        if closing(r) {
            return None;
        }
        match hit(r, x, y) {
            RingHit::Center => None,
            RingHit::Slot(k) => match &r.page[k].r {
                SlotRef::Item(p) => {
                    r.press = Some(Press { path: p.clone(), cursor: circle_win::mouse_point(r.hwnd, lparam), active: false });
                    Some(LeftPress::Item(r.hwnd))
                }
                _ => None,
            },
            _ => None,
        }
    })
    .flatten();
    match capture {
        Some(LeftPress::Item(h)) if !super::drag_capture::begin(h) => {
            with(|r| r.press = None);
            dlog!("ring drag foreground capture unavailable");
        }
        Some(LeftPress::Item(_)) | None => {}
    }
}

fn on_middle_down(lparam: LPARAM) {
    super::rename::commit_active();
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let circle =
        with(|r| (!closing(r) && hit(r, x, y) == RingHit::Center).then(|| (r.anchor, circle_win::mouse_point(r.hwnd, lparam)))).flatten();
    if let Some((anchor, at)) = circle {
        circle_win::begin_ring_drag(anchor, at);
    }
}

fn on_left_up(lparam: LPARAM) {
    let anchor = with(|r| circle_win::is_release_message(r.anchor, WM_LBUTTONUP).then_some(r.anchor)).flatten();
    if let Some(anchor) = anchor {
        circle_win::end_press(anchor, true);
        return;
    }
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    if with(|r| hit(r, x, y) == RingHit::Outside).unwrap_or(false) {
        begin_close();
        return;
    }
    let (press, hwnd) = with(|r| (r.press.take(), r.hwnd)).unwrap_or((None, std::ptr::null_mut()));
    super::drag_preview::close();
    super::drag_capture::end(hwnd);
    let action = match press {
        Some(p) if p.active => Action::Drop { path: p.path, at: circle_win::mouse_point(hwnd, lparam) },
        _ => with(|r| {
            if closing(r) {
                return Action::None;
            }
            match hit(r, x, y) {
                RingHit::Slot(k) => match r.page[k].r.clone() {
                    SlotRef::Item(p) => Action::Open(p),
                    SlotRef::Group(gi) => match r.target {
                        Target::Circle(ci) => Action::Load(set_target(r, Target::Group(ci, gi))),
                        _ => Action::None,
                    },
                    SlotRef::Back => match r.target {
                        Target::Group(ci, _) => Action::Load(set_target(r, Target::Circle(ci))),
                        _ => Action::None,
                    },
                    SlotRef::Suggestion(i) => r.suggestions.get(i).cloned().map(Action::Accept).unwrap_or(Action::None),
                },
                // Clicking the centre turns the page (wraps to the first page at the end);
                // useful when "scroll inactive windows" is switched off.
                RingHit::Center if r.refs.len() > r.metrics.per_page() => {
                    let next = geom::scroll(r.refs.len(), r.start, 1, r.metrics.per_page());
                    r.start = if next == r.start { 0 } else { next };
                    let to_load = load_page(r);
                    redraw(r);
                    Action::Load(to_load)
                }
                _ => Action::None,
            }
        })
        .unwrap_or(Action::None),
    };
    run(action);
}

fn run(action: Action) {
    match action {
        Action::None => {}
        Action::Open(p) => {
            close_now();
            let _ = items::open(&p, app().controller.get());
        }
        Action::Load(paths) => spawn_loader(paths),
        Action::Accept(s) => super::accept_suggestion(&s),
        Action::Drop { path, at } => {
            finish_drag(path, at);
            close_if_outside();
        }
    }
}

/// An item dragged out of a slot was released at `at` (screen). Dropping it on a group
/// slot, the Back slot, another circle or the Recent circle moves it there.
fn finish_drag(path: PathBuf, at: POINT) {
    let under = unsafe { WindowFromPoint(at) };
    let own = with(|r| (r.hwnd == under).then(|| (r.target, hit_screen(r, at), r.page.iter().map(|s| s.r.clone()).collect::<Vec<_>>())));
    let place = match own.flatten() {
        Some((target, RingHit::Slot(k), refs)) => Some(view::drop_place(target, refs.get(k))),
        Some(_) => None, // released on the ring itself but not on a slot: nothing to do
        None => super::which_of(under).map(|w| view::drop_place(w.target(), None)),
    };
    if let Some(place) = place {
        super::assign(&[path], place);
    }
}

/// Runs `f` (a menu) with the ring marked busy so leaving it does not close it.
fn with_menu(f: impl FnOnce()) {
    with(|r| r.menu_open = true);
    f();
    with(|r| r.menu_open = false);
    close_if_outside();
}

fn on_right_up(lparam: LPARAM) {
    super::rename::commit_active();
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let info = with(|r| {
        if closing(r) {
            return None;
        }
        let slot = match hit(r, x, y) {
            RingHit::Slot(k) => Some(r.page[k].r.clone()),
            _ => None,
        };
        Some((r.target, slot, r.suggestions.clone()))
    })
    .flatten();
    let Some((target, slot, suggestions)) = info else { return };
    let at = cursor_pos();
    with_menu(|| match slot {
        Some(SlotRef::Item(path)) => item_menu(target, &path, at),
        Some(SlotRef::Group(gi)) => {
            if let Target::Circle(ci) = target {
                match super::popup(&[(IDM_RENAME_GROUP, "&Rename group\u{2026}"), (IDM_DELETE_GROUP, "&Delete group")], at) {
                    IDM_RENAME_GROUP => super::begin_rename(super::rename::RenameTarget::Group(ci, gi)),
                    IDM_DELETE_GROUP => super::delete_group(ci, gi),
                    _ => {}
                }
            }
        }
        Some(SlotRef::Suggestion(i)) => {
            if let Some(s) = suggestions.get(i) {
                let accept = format!("&Add {} file(s) to this circle", s.paths.len());
                match super::popup(&[(IDM_ACCEPT, accept.as_str()), (IDM_DISMISS, "&Not now")], at) {
                    IDM_ACCEPT => super::accept_suggestion(s),
                    IDM_DISMISS => super::dismiss_suggestion(s),
                    _ => {}
                }
            }
        }
        Some(SlotRef::Back) => {}
        None => super::circle_menu(which_of(target), at),
    });
}

fn item_menu(target: Target, path: &std::path::Path, at: POINT) {
    let removable = target != Target::Recent;
    let mut extras = vec![(crate::desktop_view::MENU_ID, crate::desktop_view::MENU_LABEL)];
    if removable {
        extras.insert(0, (IDM_REMOVE, "Remove from circle"));
    }
    match shell_menu::show(app().controller.get(), path, at, &extras) {
        shell_menu::Choice::Extra(IDM_REMOVE) => super::assign(&[path.to_path_buf()], None),
        shell_menu::Choice::Extra(crate::desktop_view::MENU_ID) => crate::desktop_view::toggle_from_menu(),
        shell_menu::Choice::Failed => {
            let mut entries = vec![(IDM_OPEN, "&Open")];
            if removable {
                entries.push((IDM_REMOVE_PLAIN, "&Remove from circle"));
            }
            entries.extend([(0, ""), (crate::desktop_view::MENU_ID, crate::desktop_view::MENU_LABEL)]);
            match super::popup(&entries, at) {
                IDM_OPEN => {
                    let _ = items::open(path, app().controller.get());
                }
                IDM_REMOVE_PLAIN => super::assign(&[path.to_path_buf()], None),
                crate::desktop_view::MENU_ID => crate::desktop_view::toggle_from_menu(),
                _ => {}
            }
        }
        _ => {}
    }
}

fn on_wheel(wparam: WPARAM) {
    let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
    let to_load = with(|r| {
        if closing(r) {
            return None;
        }
        r.wheel += delta;
        let (pages, rest) = geom::wheel_pages(r.wheel);
        r.wheel = rest;
        if pages == 0 {
            return None;
        }
        let s = geom::scroll(r.refs.len(), r.start, pages, r.metrics.per_page());
        if s == r.start {
            return None;
        }
        r.start = s;
        let to_load = load_page(r);
        redraw(r);
        Some(to_load)
    })
    .flatten();
    if let Some(paths) = to_load {
        spawn_loader(paths);
    }
}

fn on_drop_files(wparam: WPARAM) {
    let paths = circle_win::dropped_paths(wparam as HDROP);
    let place = with(|r| {
        let slot = match hit_screen(r, cursor_pos()) {
            RingHit::Slot(k) => Some(r.page[k].r.clone()),
            _ => None,
        };
        view::drop_place(r.target, slot.as_ref())
    });
    if let Some(place) = place {
        super::assign(&paths, place);
    }
}

unsafe extern "system" fn ring_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_MOUSEMOVE => {
            on_mouse_move(wparam, lparam);
            0
        }
        WM_MOUSELEAVE => {
            let close = with(|r| {
                r.tracking = false;
                !r.menu_open && r.press.is_none() && !circle_win::is_pressed(r.anchor)
            })
            .unwrap_or(false);
            if close {
                begin_close();
            }
            0
        }
        WM_LBUTTONDOWN => {
            on_left_down(lparam);
            0
        }
        WM_LBUTTONUP => {
            on_left_up(lparam);
            0
        }
        WM_MBUTTONDOWN => {
            on_middle_down(lparam);
            0
        }
        WM_MBUTTONUP => {
            if let Some(anchor) = with(|r| circle_win::is_release_message(r.anchor, WM_MBUTTONUP).then_some(r.anchor)).flatten() {
                circle_win::end_press(anchor, true);
            }
            0
        }
        WM_CAPTURECHANGED => {
            // Capture taken away mid-drag (e.g. Alt+Tab): forget the drag.
            if lparam as HWND != hwnd {
                with(|r| r.press = None);
                super::drag_preview::close();
                super::drag_capture::end(hwnd);
            }
            0
        }
        WM_CANCELMODE | WM_KEYDOWN if msg == WM_CANCELMODE || wparam == VK_ESCAPE as usize => {
            let anchor = with(|r| r.anchor);
            if let Some(anchor) = anchor {
                circle_win::end_press(anchor, false);
            }
            with(|r| r.press = None);
            super::drag_preview::close();
            super::drag_capture::end(hwnd);
            close_if_outside();
            0
        }
        WM_RBUTTONUP => {
            on_right_up(lparam);
            0
        }
        WM_MOUSEWHEEL => {
            on_wheel(wparam);
            0
        }
        WM_DROPFILES => {
            on_drop_files(wparam);
            0
        }
        WM_TIMER if wparam == TIMER_ANIM => {
            tick();
            0
        }
        WM_APP_OUTSIDE_POINTER => {
            close_if_outside();
            0
        }
        WM_DESTROY => {
            if circle_win::pressed_window().is_none() {
                uninstall_mouse_hook();
            }
            // Destroyed from outside (its owner circle went away): drop the state.
            RING.with(|cell| {
                if let Ok(mut g) = cell.try_borrow_mut() {
                    if g.as_ref().map(|r| r.hwnd == hwnd).unwrap_or(false) {
                        *g = None;
                    }
                }
            });
            super::drag_preview::close();
            super::drag_capture::end(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
