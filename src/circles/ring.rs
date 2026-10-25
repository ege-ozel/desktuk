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

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_CONTROL, VK_ESCAPE, VK_LBUTTON, VK_MBUTTON, VK_RETURN, VK_UP,
};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use super::circle_win::{self, Which};
use super::draw::{self, RingView, SlotLook, SlotView};
use super::geom::{self, RingHit};
use super::model::Place;
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
    loaded: bool,
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
    external_requested: bool,
}

struct Ring {
    hwnd: HWND,
    anchor: HWND,
    target: Target,
    refs: Vec<SlotRef>,
    query: String,
    search_base: Option<(Vec<SlotRef>, usize)>,
    suggestions: Vec<Suggestion>,
    start: usize,
    page: Vec<PageSlot>,
    hover: Option<usize>,
    /// Keyboard/default selection survives pointer movement through empty space.
    selected: Option<usize>,
    drop_after: bool,
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
    static ITEM_WINDOW: Cell<HWND> = const { Cell::new(std::ptr::null_mut()) };
    static ITEM_POINTER: Cell<Option<(POINT, bool)>> = const { Cell::new(None) };
    static ITEM_ROUTE: Cell<bool> = const { Cell::new(false) };
    static ITEM_PRESS_ID: Cell<usize> = const { Cell::new(0) };
    static MOUSE_HOOK: Cell<HHOOK> = const { Cell::new(std::ptr::null_mut()) };
}

const WM_ITEM_POINTER: u32 = WM_APP + 43;
const WM_APP_OUTSIDE_POINTER: u32 = WM_APP + 41;
const WM_APP_EXTERNAL_DRAG: u32 = WM_APP + 46;

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
        if let Some((anchor, _, release_message)) = circle_win::pressed_mouse_route() {
            if kind == WM_MOUSEMOVE || kind == release_message {
                // One coalesced screen-coordinate stream, even when native capture succeeds.
                let point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
                circle_win::queue_pointer(anchor, point, kind == release_message);
                return unsafe { CallNextHookEx(MOUSE_HOOK.get(), code, message, data) };
            }
        }
        if let Some(hwnd) = item_pressed_window() {
            if matches!(kind, WM_MOUSEMOVE | WM_LBUTTONUP) && !super::drag_capture::is_active_for(hwnd) {
                let point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
                queue_item_pointer(hwnd, point, kind == WM_LBUTTONUP);
            }
            return unsafe { CallNextHookEx(MOUSE_HOOK.get(), code, message, data) };
        }
        if matches!(kind, WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN) {
            let point = unsafe { (*(data as *const MSLLHOOKSTRUCT)).pt };
            let outside = with(|ring| hit_screen(ring, point) == RingHit::Outside).unwrap_or(false);
            let pointer_only = kind == WM_MOUSEMOVE;
            if outside && !pointer_only {
                super::launcher::dismiss_outside(point);
            }
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

fn item_pressed_window() -> Option<HWND> {
    let hwnd = ITEM_WINDOW.get();
    (!hwnd.is_null()).then_some(hwnd)
}

pub fn is_open() -> bool {
    with(|_| true).unwrap_or(false)
}

pub fn item_drag_pending() -> bool {
    item_pressed_window().is_some()
}

fn reset_item_pointer() {
    ITEM_WINDOW.set(std::ptr::null_mut());
    ITEM_POINTER.set(None);
    ITEM_ROUTE.set(false);
    ITEM_PRESS_ID.set(ITEM_PRESS_ID.get().wrapping_add(1));
}

fn queue_item_pointer(hwnd: HWND, point: POINT, released: bool) {
    if ITEM_POINTER.get().is_some_and(|(_, released)| released) {
        return;
    }
    ITEM_ROUTE.set(true);
    if ITEM_POINTER.replace(Some((point, released))).is_none()
        && unsafe { PostMessageW(hwnd, WM_ITEM_POINTER, ITEM_PRESS_ID.get(), 0) } == 0
    {
        ITEM_POINTER.set(None);
        ITEM_ROUTE.set(false);
    }
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

/// Opens the ring for `target` over the circle window `anchor`, closing any other ring.
pub fn open(anchor: HWND, target: Target) {
    if crate::dock::locked() {
        return;
    }
    if item_pressed_window().is_some() || circle_win::pressed_window().is_some() {
        return;
    }
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
        query: String::new(),
        search_base: None,
        suggestions,
        start: 0,
        page: Vec::new(),
        hover: None,
        selected: None,
        drop_after: false,
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
    super::search_input::install();
    dlog!(
        "ring opened target={target:?} slots={slots} icon_px={} per_page={} diameter={size} inner_slots={} inner_r={} outer_r={}",
        m.icon,
        m.per_page(),
        m.inner_slots,
        m.inner_r,
        m.outer_r
    );
}

fn spawn_loader(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
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
                    slot.loaded = false;
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
                    slot.loaded = true;
                    item.apply(meta);
                    item.set_icon(icon, batch.px);
                    any = true;
                }
            }
        }
        if any && !closing(r) {
            if r.press.is_some() {
                super::defer_refresh();
            } else {
                redraw(r);
            }
        }
    });
}

/// Destroys the ring immediately (no animation). Safe to call when none is open.
pub fn close_now() {
    super::launcher::close_ring();
    super::search_input::uninstall();
    items::cancel_ring_loader();
    reset_item_pointer();
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
        }
        crate::app::schedule_trim();
    }
}

fn begin_close() {
    if with(|r| r.menu_open).unwrap_or(false) {
        return;
    }
    if circle_win::pressed_window().is_some() || item_pressed_window().is_some() {
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
    super::search_input::uninstall();
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
    if item_drag_pending() {
        super::defer_refresh();
        return;
    }
    let to_load = with(|r| {
        let (mut refs, suggestions) = super::slots_for(r.target);
        if !r.query.is_empty() {
            if let Ok(board) = app().board.try_borrow() {
                let len = refs.len();
                let source = view::search_candidates(&board, r.target, refs);
                refs = view::filter_refs(&source, &r.query);
                r.search_base = Some((source, len));
            }
        }
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
            if r.press.is_some() {
                super::defer_refresh();
            } else {
                redraw(r);
            }
        }
    });
}

/// Builds `page` for the current `start`; returns the item paths whose metadata must load.
fn load_page(r: &mut Ring) -> Vec<PathBuf> {
    items::cancel_ring_loader();
    let range = geom::page_range(r.refs.len(), r.start, r.metrics.per_page());
    r.start = range.start;
    r.hover = None;
    r.selected = (!range.is_empty()).then_some(0);
    let mut previous = std::mem::take(&mut r.page);
    r.page = r.refs[range]
        .iter()
        .map(|sr| {
            if let Some(index) = previous.iter().position(|s| s.r == *sr) {
                return previous.swap_remove(index);
            }
            PageSlot {
                r: sr.clone(),
                load_id: 0,
                loaded: false,
                item: match sr {
                    SlotRef::Item(p) => Some(Item::placeholder(p.clone())),
                    _ => None,
                },
            }
        })
        .collect();
    r.page.iter().filter(|s| !s.loaded).filter_map(|s| s.item.as_ref().map(|i| i.path.clone())).collect()
}

fn set_target(r: &mut Ring, t: Target) -> Vec<PathBuf> {
    r.query.clear();
    r.search_base = None;
    r.target = t;
    let (refs, suggestions) = super::slots_for(t);
    r.refs = refs;
    r.suggestions = suggestions;
    r.start = 0;
    let to_load = load_page(r);
    redraw(r);
    to_load
}

pub(super) fn search_area() -> Option<(RECT, POINT, i32)> {
    with(|r| {
        (
            win::rect(
                r.center.x - r.metrics.ring_r,
                r.center.y - r.metrics.ring_r,
                r.center.x + r.metrics.ring_r,
                r.center.y + r.metrics.ring_r,
            ),
            r.center,
            r.metrics.circle_d,
        )
    })
}
pub(super) fn search_window() -> Option<HWND> {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut point);
    }
    let hover_editor = super::launcher::hover_editor();
    let over_editor = hover_editor && super::launcher::contains_editor(point);
    with(|r| {
        (!closing(r)
            && !super::rename::is_active()
            && r.press.is_none()
            && circle_win::pressed_window().is_none()
            && (hover_editor || (!r.menu_open && !super::launcher::is_active()))
            && (over_editor || hit_screen(r, point) != RingHit::Outside))
            .then_some(r.hwnd)
    })
    .flatten()
}

pub(super) fn search_key(key: u32) {
    let action = with(|r| {
        if closing(r) || r.refs.is_empty() {
            return Action::None;
        }
        if key == VK_RETURN as u32 {
            return match r.page.get(r.hover.or(r.selected).unwrap_or(0)).map(|s| s.r.clone()) {
                Some(SlotRef::Item(p)) => Action::Open(p),
                Some(SlotRef::Group(gi)) => {
                    if let Target::Circle(ci) = r.target {
                        Action::Load(set_target(r, Target::Group(ci, gi)))
                    } else {
                        Action::None
                    }
                }
                Some(SlotRef::Back) => {
                    if let Target::Group(ci, _) = r.target {
                        Action::Load(set_target(r, Target::Circle(ci)))
                    } else {
                        Action::None
                    }
                }
                Some(SlotRef::Suggestion(i)) => r.suggestions.get(i).cloned().map(Action::Accept).unwrap_or(Action::None),
                _ => Action::None,
            };
        }
        let index = match r.hover.or(r.selected) {
            Some(i) if key == VK_UP as u32 => (r.start + i).saturating_sub(1),
            Some(i) => (r.start + i + 1).min(r.refs.len() - 1),
            None if key == VK_UP as u32 => r.refs.len() - 1,
            None => 0,
        };
        let start = index / r.metrics.per_page() * r.metrics.per_page();
        let paths = if start != r.start {
            r.start = start;
            Some(load_page(r))
        } else {
            None
        };
        r.hover = None;
        r.selected = Some(index - r.start);
        redraw(r);
        paths.map(Action::Load).unwrap_or(Action::None)
    })
    .unwrap_or(Action::None);
    run(action);
}

fn search_char(value: u32) {
    if super::launcher::ring_char(value) {
        return;
    }
    let current = with(|r| r.query.clone()).unwrap_or_default();
    let mut query = current;
    match value {
        8 => {
            query.pop();
        }
        27 => query.clear(),
        _ => {
            if let Some(c) = char::from_u32(value).filter(|c| !c.is_control()) {
                if query.chars().count() < 80 {
                    query.push(c);
                }
            }
        }
    }
    set_search(&query);
    if value != 27 {
        let at = with(|r| {
            (POINT { x: r.center.x - win::scale(170, r.dpi), y: r.center.y - r.metrics.circle_d / 2 - win::scale(56, r.dpi) }, r.dpi)
        });
        if let Some((at, dpi)) = at {
            super::launcher::edit_ring(at, &query, dpi);
            if value == 1 {
                super::launcher::ring_char(1);
            }
        }
    }
}

pub(super) fn editing_started() {
    with(|r| r.menu_open = true);
    // Keep the temporary hook until foreground activation succeeds.
}
pub(super) fn editing_finished() {
    if with(|r| r.menu_open = false).is_some() {
        super::search_input::install();
    }
}

pub(super) fn set_search(query: &str) {
    let paths = with(|r| {
        if r.query == query {
            return None;
        }
        r.query = query.to_owned();
        if r.query.is_empty() {
            if let Some((mut source, original_len)) = r.search_base.take() {
                source.truncate(original_len);
                r.refs = source;
            }
        } else {
            if r.search_base.is_none() {
                let refs = std::mem::take(&mut r.refs);
                let len = refs.len();
                let source = if let Ok(board) = app().board.try_borrow() { view::search_candidates(&board, r.target, refs) } else { refs };
                r.search_base = Some((source, len));
            }
            if let Some((source, _)) = &r.search_base {
                r.refs = view::filter_refs(source, &r.query);
            }
        }
        r.start = 0;
        let paths = load_page(r);
        redraw(r);
        dlog!("ring search chars={} results={}", r.query.chars().count(), r.refs.len());
        Some(paths)
    })
    .flatten();
    if let Some(paths) = paths {
        spawn_loader(paths);
    }
}

fn subtitle(r: &Ring) -> String {
    if !r.query.is_empty() {
        return format!(
            "{}: {}\n{} {}",
            crate::i18n::choose("Ara", "Search"),
            r.query,
            r.refs.len(),
            crate::i18n::choose("sonuç", "results")
        );
    }
    if r.press.as_ref().is_some_and(|p| p.active) {
        return if r.hover.is_some() {
            if r.drop_after {
                crate::i18n::text("Arkasına bırak").into()
            } else {
                crate::i18n::text("Önüne bırak").into()
            }
        } else {
            crate::i18n::text("Taşı / sırala").into()
        };
    }
    let label = geom::page_label(r.refs.len(), r.start, r.metrics.per_page());
    if !label.is_empty() {
        return label;
    }
    if r.refs.is_empty() {
        let no_circles = app().board.try_borrow().map(|b| b.circles.is_empty()).unwrap_or(false);
        return if r.target == Target::Recent && no_circles {
            crate::i18n::text("Right-click: New circle").into()
        } else {
            crate::i18n::text("Drop files here").into()
        };
    }
    crate::i18n::text("Aramak i\u{e7}in yaz\u{131}n").into()
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
                    labels.push(crate::i18n::text("Back").into());
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
    let title = if r.target == Target::Recent {
        crate::i18n::text("Recent").to_string()
    } else {
        app().board.try_borrow().map(|b| view::title(&b, r.target)).unwrap_or_default()
    };
    let sub = subtitle(r);
    let views: Vec<SlotView> = labels.iter().zip(looks).map(|(l, look)| SlotView { label: l, look }).collect();
    draw::ring(
        &mut r.surface,
        r.dpi,
        &m,
        &RingView {
            title: &title,
            subtitle: &sub,
            search: (!r.query.is_empty()).then_some((r.query.as_str(), r.refs.len())),
            slots: &views,
            hover: r.hover.or_else(|| r.press.is_none().then_some(r.selected).flatten()),
            recent: r.target == Target::Recent,
            insertion: r.press.as_ref().filter(|p| p.active).and(r.hover).map(|k| (k, r.drop_after)),
        },
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
    if let Some(hwnd) = item_pressed_window() {
        if wparam & 1 != 0 && !ITEM_ROUTE.get() {
            move_item(circle_win::mouse_point(hwnd, lparam));
        }
        return;
    }
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    if with(|r| hit(r, x, y) == RingHit::Outside).unwrap_or(false) {
        begin_close();
        return;
    }
    with(|r| {
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
        let hover = match hit(r, x, y) {
            RingHit::Slot(k) => Some(k),
            _ => None,
        };
        if hover != r.hover {
            r.hover = hover;
            redraw(r);
        }
        None::<()>
    });
}

fn move_item(c: POINT) {
    let export_requested = unsafe { GetAsyncKeyState(VK_CONTROL as i32) } < 0;
    let requested = with(|r| r.press.as_ref().is_some_and(|p| p.external_requested)).unwrap_or(false);
    if requested {
        return;
    }
    let export = if export_requested && with(|r| r.press.as_ref().is_some_and(|p| p.active)).unwrap_or(false) {
        with(|r| {
            let hwnd = r.hwnd;
            let p = r.press.as_mut()?;
            if p.external_requested {
                return None;
            }
            p.external_requested = true;
            Some((hwnd, p.path.clone()))
        })
        .flatten()
    } else {
        None
    };
    if let Some((hwnd, _path)) = export {
        unsafe { PostMessageW(hwnd, WM_APP_EXTERNAL_DRAG, 0, 0) };
        return;
    }
    with(|r| {
        if r.press.is_some() {
            let next = geom::drag_slot(&r.metrics, r.page.len(), c.x - r.center.x, c.y - r.center.y, r.hover, win::scale(6, r.dpi));
            let after = next.is_some_and(|k| {
                let (x, y) = geom::slot_center(&r.metrics, k);
                // Clockwise tangent divides each fixed slot into before/after halves.
                let tangent = (c.x - r.center.x - x) as i64 * -(y as i64) + (c.y - r.center.y - y) as i64 * x as i64;
                let band = win::scale(4, r.dpi) as i64 * (x.abs() + y.abs()) as i64;
                if next == r.hover && tangent.abs() < band {
                    r.drop_after
                } else {
                    tangent > 0
                }
            });
            if next != r.hover || after != r.drop_after {
                r.hover = next;
                r.drop_after = after;
                redraw(r);
            }
        }
    });
    let export = with(|r| {
        let hwnd = r.hwnd;
        let p = r.press.as_mut()?;
        if p.active && export_requested && !p.external_requested {
            p.external_requested = true;
            Some((hwnd, p.path.clone()))
        } else {
            None
        }
    })
    .flatten();
    if let Some((hwnd, _path)) = export {
        unsafe { PostMessageW(hwnd, WM_APP_EXTERNAL_DRAG, 0, 0) };
        return;
    }
    let preview = with(|r| {
        let dpi = r.dpi;
        if let Some(p) = r.press.as_mut() {
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
        None
    })
    .flatten();
    if let Some((owner, at, dpi, surface)) = preview {
        if let Some(surface) = surface {
            with(redraw);
            super::drag_preview::show(surface, owner, at, dpi);
        } else {
            super::drag_preview::move_to(at);
        }
    }
}

fn start_external_drag(hwnd: HWND) {
    let source = with(|r| {
        if r.hwnd != hwnd {
            return None;
        }
        let press = r.press.take()?;
        r.hover = None;
        redraw(r);
        Some(press.path)
    })
    .flatten();
    let Some(path) = source else { return };

    reset_item_pointer();
    super::drag_preview::close();
    uninstall_mouse_hook();
    match super::external_drag::start(hwnd, &path) {
        Ok(()) => dlog!("external file drag finished: {}", path.display()),
        Err(error) => dlog!("external file drag failed for {}: {error}", path.display()),
    }
    // Keep foreground/capture ownership through OLE startup; ending it first caused
    // an unnecessary focus/z-order round trip right at the start of an external drag.
    super::drag_capture::end(hwnd);
    if with(|r| r.hwnd == hwnd).unwrap_or(false) {
        install_mouse_hook();
        close_if_outside();
    }
    super::interaction_ended();
}

/// Something the ring must do after its state borrow has ended.
enum Action {
    None,
    Open(PathBuf),
    Load(Vec<PathBuf>),
    Accept(Suggestion),
    Drop { path: PathBuf, destination: Option<DropTarget> },
}

enum LeftPress {
    Circle(HWND, POINT, POINT),
    Item(HWND),
}

fn on_left_down(lparam: LPARAM) {
    dlog!(
        "ring item press client=({}, {}) hit={:?}",
        win::x_lparam(lparam),
        win::y_lparam(lparam),
        with(|r| (hit(r, win::x_lparam(lparam), win::y_lparam(lparam)), closing(r)))
    );
    super::rename::commit_active();
    reset_item_pointer();
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let capture = with(|r| {
        if closing(r) {
            return None;
        }
        match hit(r, x, y) {
            RingHit::Center | RingHit::Ring => Some(LeftPress::Circle(r.anchor, circle_win::mouse_point(r.hwnd, lparam), r.center)),
            RingHit::Slot(k) => match &r.page[k].r {
                SlotRef::Item(p) => {
                    r.press = Some(Press {
                        path: p.clone(),
                        cursor: circle_win::mouse_point(r.hwnd, lparam),
                        active: false,
                        external_requested: false,
                    });
                    ITEM_WINDOW.set(r.hwnd);
                    Some(LeftPress::Item(r.hwnd))
                }
                _ => None,
            },
            _ => None,
        }
    })
    .flatten();
    match capture {
        Some(LeftPress::Circle(anchor, at, center)) => circle_win::begin_ring_drag(anchor, at, center, VK_LBUTTON),
        Some(LeftPress::Item(h)) if !super::drag_capture::begin(h) => {
            // Keep the press: the already installed hook routes screen samples.
            install_mouse_hook();
            dlog!("ring drag foreground capture unavailable; using pointer fallback");
        }
        Some(LeftPress::Item(_)) | None => {}
    }
}

fn on_middle_down(lparam: LPARAM) {
    super::rename::commit_active();
    let (x, y) = (win::x_lparam(lparam), win::y_lparam(lparam));
    let circle =
        with(|r| (!closing(r) && hit(r, x, y) != RingHit::Outside).then(|| (r.anchor, circle_win::mouse_point(r.hwnd, lparam), r.center)))
            .flatten();
    if let Some((anchor, at, center)) = circle {
        circle_win::begin_ring_drag(anchor, at, center, VK_MBUTTON);
    }
}

/// Centre pages normally; with a query it reopens the native text editor. Drag still moves.
pub fn center_click(anchor: HWND) -> bool {
    let editor = with(|r| {
        (r.anchor == anchor && !closing(r) && !r.query.is_empty()).then(|| {
            (
                POINT { x: r.center.x - win::scale(170, r.dpi), y: r.center.y - r.metrics.circle_d / 2 - win::scale(56, r.dpi) },
                r.query.clone(),
                r.dpi,
            )
        })
    })
    .flatten();
    if let Some((at, query, dpi)) = editor {
        super::launcher::edit_ring(at, &query, dpi);
        return true;
    }

    let paths = with(|r| {
        if r.anchor != anchor || closing(r) {
            return None;
        }
        if r.refs.len() <= r.metrics.per_page() {
            return Some(Vec::new());
        }
        let next = geom::scroll(r.refs.len(), r.start, 1, r.metrics.per_page());
        r.start = if next == r.start { 0 } else { next };
        let paths = load_page(r);
        redraw(r);
        Some(paths)
    })
    .flatten();
    if let Some(paths) = paths {
        spawn_loader(paths);
        true
    } else {
        false
    }
}

fn on_left_up(lparam: LPARAM) {
    if ITEM_ROUTE.get() {
        return;
    }
    let point = with(|r| circle_win::mouse_point(r.hwnd, lparam));
    if let Some(point) = point {
        on_left_up_at(point);
    }
}

fn on_left_up_at(at: POINT) {
    move_item(at);
    let anchor = with(|r| circle_win::is_release_message(r.anchor, WM_LBUTTONUP).then_some(r.anchor)).flatten();
    if let Some(anchor) = anchor {
        circle_win::end_press(anchor, true);
        return;
    }
    let (x, y) = with(|r| {
        let tl = top_left(r);
        (at.x - tl.x, at.y - tl.y)
    })
    .unwrap_or((0, 0));
    let (press, hwnd) = with(|r| (r.press.take(), r.hwnd)).unwrap_or((None, std::ptr::null_mut()));
    // Capture release/restoring focus can pump messages. Resolve the destination
    // against the source ring before those calls can replace its view/state.
    let destination = if press.as_ref().is_some_and(|p| p.active) { drop_destination(at) } else { None };
    with(|r| {
        r.hover = None;
        redraw(r);
    });
    reset_item_pointer();
    super::drag_preview::close();
    super::drag_capture::end(hwnd);
    if !press.as_ref().is_some_and(|p| p.active) && with(|r| hit_screen(r, at) == RingHit::Outside).unwrap_or(false) {
        begin_close();
        super::interaction_ended();
        return;
    }
    let action = match press {
        Some(p) if p.active => Action::Drop { path: p.path, destination },
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
    super::interaction_ended();
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
        Action::Drop { path, destination } => {
            if let Some(destination) = destination {
                if let Some(place) = destination.place {
                    super::insert_item(&path, place, destination.before.as_deref());
                } else {
                    super::assign(&[path], None);
                }
            }
            close_if_outside();
        }
    }
}

/// An item dragged out of a slot was released at `at` (screen). Dropping it on a group
/// slot, the Back slot, another circle or Recent resolves a reference destination.
struct DropTarget {
    place: Option<Place>,
    before: Option<PathBuf>,
}

fn drop_destination(at: POINT) -> Option<DropTarget> {
    let anchor = with(|r| r.anchor).unwrap_or(std::ptr::null_mut());
    if let Some(which) = super::circle_at(at, anchor) {
        return Some(DropTarget { place: view::drop_place(which.target(), None), before: None });
    }
    with(|r| {
        if hit_screen(r, at) == RingHit::Outside {
            return None;
        }
        let k = r.hover?;
        let slot = &r.page.get(k)?.r;
        let place = view::drop_place(r.target, Some(slot));
        let before = if let SlotRef::Item(path) = slot {
            // Snapshot a path anchor before releasing capture; empty space remains invalid.
            if r.drop_after {
                let index = r.refs.iter().position(|item| item == slot)?;
                r.refs[index + 1..].iter().find_map(|item| match item {
                    SlotRef::Item(p) => Some(p.clone()),
                    _ => None,
                })
            } else {
                Some(path.clone())
            }
        } else {
            None
        };
        Some(DropTarget { place, before })
    })
    .flatten()
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
                let accept = format!("{} {}", s.paths.len(), crate::i18n::choose("dosyayı bu çembere ekle", "file(s): add to this circle"));
                match super::popup(&[(IDM_ACCEPT, accept.as_str()), (IDM_DISMISS, "&Not now")], at) {
                    IDM_ACCEPT => super::accept_suggestion(s),
                    IDM_DISMISS => super::dismiss_suggestion(s),
                    _ => {}
                }
            }
        }
        Some(SlotRef::Back) => {}
        None => {
            if matches!(target, Target::Group(..)) {
                let command = super::popup(&[(super::IDM_SORT, "Sırala")], at);
                super::sort_target(target, command);
            } else {
                super::circle_menu(which_of(target), at);
            }
        }
    });
}

fn item_menu(target: Target, path: &std::path::Path, at: POINT) {
    let removable = target != Target::Recent;
    let mut extras = vec![(crate::desktop_view::MENU_ID, crate::desktop_view::MENU_LABEL)];
    if removable {
        extras.extend([
            (super::IDM_SORT_NAME, "Ada göre sırala"),
            (super::IDM_SORT_TYPE, "Türe göre sırala"),
            (super::IDM_SORT_ADDED, "Eklenme sırası"),
        ]);
    }
    if removable {
        extras.insert(0, (IDM_REMOVE, "Remove from circle"));
    }
    match shell_menu::show(app().controller.get(), path, at, &extras) {
        shell_menu::Choice::Extra(command @ super::IDM_SORT_NAME..=super::IDM_SORT_ADDED) => super::sort_target(target, command),
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
        r.hover = None;
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
        WM_ITEM_POINTER => {
            if wparam == ITEM_PRESS_ID.get() && item_pressed_window() == Some(hwnd) {
                if let Some((point, released)) = ITEM_POINTER.take() {
                    move_item(point);
                    if released {
                        on_left_up_at(point);
                    }
                }
            }
            0
        }
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
                circle_win::native_release(anchor);
            }
            0
        }
        WM_CAPTURECHANGED => {
            // Capture taken away mid-drag (e.g. Alt+Tab): forget the drag.
            if lparam as HWND != hwnd {
                with(|r| {
                    let active = r.press.take().is_some_and(|p| p.active);
                    if active {
                        r.hover = None;
                        redraw(r);
                    }
                });
                reset_item_pointer();
                super::drag_preview::close();
                super::drag_capture::end(hwnd);
                super::interaction_ended();
            }
            0
        }
        WM_CANCELMODE | WM_KEYDOWN if msg == WM_CANCELMODE || wparam == VK_ESCAPE as usize => {
            let anchor = with(|r| r.anchor);
            if let Some(anchor) = anchor {
                circle_win::end_press(anchor, false);
            }
            with(|r| {
                let active = r.press.take().is_some_and(|p| p.active);
                if active {
                    r.hover = None;
                    redraw(r);
                }
            });
            reset_item_pointer();
            super::drag_preview::close();
            super::drag_capture::end(hwnd);
            super::interaction_ended();
            close_if_outside();
            0
        }
        super::search_input::WM_SEARCH_KEY => {
            super::launcher::ring_key(wparam as u32, lparam as usize);
            0
        }
        super::search_input::WM_SEARCH_CHAR => {
            search_char(wparam as u32);
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
        WM_APP_EXTERNAL_DRAG => {
            start_external_drag(hwnd);
            0
        }
        WM_DESTROY => {
            super::search_input::uninstall();
            reset_item_pointer();
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
