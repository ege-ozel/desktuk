//! Phase 2 circles UI: round circles on the desktop that open into a ring of their files
//! on hover. Spec: `docs/superpowers/specs/2026-10-13-circles-design.md`.
//!
//! * `model`, `geom`, `view`, `suggest` and the pure half of `desktop` are plain logic
//!   with unit tests.
//! * `circle_win`, `ring`, `rename`, `surface`, `draw` are the Win32 side.
//! * This file owns the runtime state and the commands (assign, rename, delete, ...).
//!
//! RefCell rule (AGENTS.md): code here uses `try_borrow*` on shared state and copies what
//! it needs into locals before any call that can pump messages or destroy a window.

pub mod circle_win;
pub mod desktop;
mod drag_capture;
mod drag_preview;
pub mod draw;
pub mod geom;
pub mod model;
pub mod rename;
pub mod ring;
pub mod suggest;
pub mod surface;
pub mod view;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{HANDLE, HWND, POINT};
use windows_sys::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::{app, TIMER_DESKTOP};
use crate::host::DesktopHost;
use crate::watch::Change;
use crate::{diag, dlog, win};
use circle_win::{CircleWin, Which};
use model::Place;
use suggest::Suggestion;
use view::{SlotRef, Target};

const IDM_NEW_CIRCLE: usize = 1;
const IDM_NEW_GROUP: usize = 2;
const IDM_RENAME: usize = 3;
const IDM_DELETE: usize = 4;
const IDM_EXIT: usize = 10;

/// A download or copy can fire several folder events in a row; re-list once they settle.
const DESKTOP_DEBOUNCE_MS: u32 = 250;

struct State {
    dirs: Vec<PathBuf>,
    entries: RefCell<Vec<desktop::Entry>>,
    /// `[0]` is Recent, then one window per `Board::circles` entry, in order.
    windows: RefCell<Vec<CircleWin>>,
    watcher: RefCell<Option<desktop::Watcher>>,
    /// Suggestions dismissed this session: (extension, circle index).
    dismissed: RefCell<Vec<(String, usize)>>,
}

thread_local! {
    static STATE: Cell<Option<&'static State>> = const { Cell::new(None) };
}

fn state() -> Option<&'static State> {
    STATE.with(|s| s.get())
}

/// Lists the Desktop, starts watching it and shows every circle.
pub fn start() {
    let t = diag::since_start_ms();
    let dirs = desktop::desktop_dirs();
    let entries = desktop::list(&dirs);
    let watcher = desktop::Watcher::new(&dirs);
    let n_entries = entries.len();
    let st: &'static State = Box::leak(Box::new(State {
        dirs,
        entries: RefCell::new(entries),
        windows: RefCell::new(Vec::new()),
        watcher: RefCell::new(Some(watcher)),
        dismissed: RefCell::new(Vec::new()),
    }));
    STATE.with(|s| s.set(Some(st)));
    circle_win::register_class();
    ring::register_class();
    rename::register_class();
    rebuild();
    let circles = app().board.try_borrow().map(|b| b.circles.len()).unwrap_or(0);
    dlog!("METRIC circles_started circles={circles} desktop_entries={n_entries} ms={:.1}", diag::since_start_ms() - t);
}

/// Destroys every circle window, the ring and the rename box; stops watching.
pub fn shutdown() {
    let Some(st) = state() else { return };
    ring::close_now();
    rename::cancel();
    let old: Vec<CircleWin> = st.windows.try_borrow_mut().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    for w in &old {
        unsafe { DestroyWindow(w.hwnd) };
    }
    if let Ok(mut w) = st.watcher.try_borrow_mut() {
        *w = None;
    }
}

/// Re-creates all circle windows from the board (after a circle was added or removed).
pub fn rebuild() {
    let Some(st) = state() else { return };
    ring::close_now();
    rename::cancel(); // circle indices are about to change
    let old: Vec<CircleWin> = st.windows.try_borrow_mut().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
    for w in &old {
        unsafe { DestroyWindow(w.hwnd) };
    }
    drop(old);
    let (recent_pos, positions) = match app().board.try_borrow() {
        Ok(b) => (b.recent_pos, b.circles.iter().map(|c| c.pos).collect::<Vec<_>>()),
        Err(_) => return,
    };
    let mut new = Vec::new();
    if let Some(w) = circle_win::create(Which::Recent, recent_pos.unwrap_or_else(default_recent_pos)) {
        new.push(w);
    }
    for (ci, pos) in positions.into_iter().enumerate() {
        if let Some(w) = circle_win::create(Which::User(ci), pos) {
            new.push(w);
        }
    }
    let list: Vec<(HWND, Which)> = new.iter().map(|w| (w.hwnd, w.which)).collect();
    if let Ok(mut w) = st.windows.try_borrow_mut() {
        *w = new;
    }
    for (h, which) in list {
        circle_win::render(h, which);
    }
}

/// Explicit tray recovery: recreate missing windows, clamp positions and reveal the circles.
pub fn show() {
    let Some(st) = state() else { return };
    let missing = match st.windows.try_borrow() {
        Ok(windows) => windows.is_empty() || windows.iter().any(|w| unsafe { IsWindow(w.hwnd) } == 0),
        Err(_) => return,
    };
    if missing {
        rebuild();
    }
    ring::close_now();
    let list: Vec<_> = match st.windows.try_borrow() {
        Ok(windows) => windows.iter().map(|w| (w.hwnd, w.which, w.host.clone())).collect(),
        Err(_) => return,
    };
    for (hwnd, which, host) in list {
        circle_win::render(hwnd, which);
        host.reveal(hwnd);
    }
}

/// Repaints every circle (names, counts) and the open ring.
pub fn refresh_all() {
    let Some(st) = state() else { return };
    let list: Vec<(HWND, Which)> = match st.windows.try_borrow() {
        Ok(w) => w.iter().map(|w| (w.hwnd, w.which)).collect(),
        Err(_) => return,
    };
    for (h, which) in list {
        circle_win::render(h, which);
    }
    ring::refresh();
}

pub fn which_of(hwnd: HWND) -> Option<Which> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.hwnd == hwnd).map(|w| w.which)
}

pub fn host_of(hwnd: HWND) -> Option<Rc<dyn DesktopHost>> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.hwnd == hwnd).map(|w| w.host.clone())
}

fn window_of(which: Which) -> Option<HWND> {
    state()?.windows.try_borrow().ok()?.iter().find(|w| w.which == which).map(|w| w.hwnd)
}

/// The foreground window changed: each circle's host applies the "Show desktop" policy.
pub fn foreground_changed(fg: HWND) {
    // Drag capture and the rename popup temporarily activate our own windows.
    // They do not represent switching from the Desktop to another application.
    if crate::shell::window_pid(fg) == unsafe { windows_sys::Win32::System::Threading::GetCurrentProcessId() } {
        return;
    }
    let Some(st) = state() else { return };
    let list: Vec<(HWND, Rc<dyn DesktopHost>)> = match st.windows.try_borrow() {
        Ok(w) => w.iter().map(|w| (w.hwnd, w.host.clone())).collect(),
        Err(_) => return,
    };
    for (h, host) in list {
        host.foreground_changed(h, fg);
    }
}

/// Top-right of the primary work area, like the Phase 1 panel.
pub fn default_recent_pos() -> (i32, i32) {
    let mut work = win::rect(0, 0, 1024, 768);
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut windows_sys::Win32::Foundation::RECT).cast(), 0) };
    let dpi = unsafe { GetDpiForSystem() }.max(96);
    let m = geom::metrics(dpi);
    (work.right - m.circle_d - win::scale(40, dpi), work.top + win::scale(40, dpi))
}

/// Desktop entries in no circle, newest first.
pub fn recent_paths() -> Vec<PathBuf> {
    let Some(st) = state() else { return Vec::new() };
    let (Ok(entries), Ok(board)) = (st.entries.try_borrow(), app().board.try_borrow()) else { return Vec::new() };
    desktop::recent(&entries, &board)
}

/// The slots of `target` (and, for Recent, the suggestions they refer to).
pub fn slots_for(target: Target) -> (Vec<SlotRef>, Vec<Suggestion>) {
    let recent = if target == Target::Recent { recent_paths() } else { Vec::new() };
    let Ok(board) = app().board.try_borrow() else { return (Vec::new(), Vec::new()) };
    let suggestions = match (target, state()) {
        (Target::Recent, Some(st)) => {
            let dismissed = st.dismissed.try_borrow().map(|d| d.clone()).unwrap_or_default();
            suggest::suggest(&board, &recent, &dismissed)
        }
        _ => Vec::new(),
    };
    (view::slot_refs(&board, target, &recent, &suggestions), suggestions)
}

/// Kernel handles the message loop waits on (empty when the circles UI is not running).
pub fn wait_handles() -> Vec<HANDLE> {
    state().and_then(|st| st.watcher.try_borrow().ok().and_then(|w| w.as_ref().map(|w| w.handles().to_vec()))).unwrap_or_default()
}

/// Desktop folder handle `i` was signalled: re-arm it and re-list after a short pause.
pub fn on_desktop_signal(i: usize) {
    let Some(st) = state() else { return };
    if let Ok(w) = st.watcher.try_borrow() {
        if let Some(w) = w.as_ref() {
            w.rearm(i);
        }
    }
    unsafe { SetTimer(app().controller.get(), TIMER_DESKTOP, DESKTOP_DEBOUNCE_MS, None) };
}

/// `TIMER_DESKTOP` fired: re-list the Desktop and repaint.
pub fn refresh_desktop() {
    let Some(st) = state() else { return };
    unsafe { KillTimer(app().controller.get(), TIMER_DESKTOP) };
    let entries = desktop::list(&st.dirs);
    dlog!("desktop changed: {} entries", entries.len());
    if let Ok(mut e) = st.entries.try_borrow_mut() {
        *e = entries;
    }
    refresh_all();
}

/// A shell rename: stored paths follow it.
pub fn on_shell_change(change: &Change) {
    let changed = app().board.try_borrow_mut().map(|mut b| b.follow(change)).unwrap_or(false);
    if changed {
        app().save_config();
        refresh_all();
    }
    ring::on_shell_change(change);
}

/// Moves `paths` to `to` (`None` = back to Recent), saves and repaints.
pub fn assign(paths: &[PathBuf], to: Option<Place>) {
    if paths.is_empty() {
        return;
    }
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        for p in paths {
            match to {
                Some(place) => {
                    b.assign(p, place);
                }
                None => {
                    b.unassign(p);
                }
            }
        }
    }
    app().save_config();
    refresh_all();
}

/// A circle was dragged to `pos` (screen top-left).
pub fn moved(which: Which, pos: (i32, i32)) {
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        match which {
            Which::Recent => b.recent_pos = Some(pos),
            Which::User(ci) => {
                if let Some(c) = b.circles.get_mut(ci) {
                    c.pos = pos;
                }
            }
        }
    }
    app().save_config();
}

pub fn accept_suggestion(s: &Suggestion) {
    dlog!("suggestion accepted: {} .{} file(s) -> circle {}", s.paths.len(), s.ext, s.circle);
    assign(&s.paths, Some(Place::Circle(s.circle)));
}

pub fn dismiss_suggestion(s: &Suggestion) {
    if let Some(st) = state() {
        if let Ok(mut d) = st.dismissed.try_borrow_mut() {
            d.push((s.ext.clone(), s.circle));
        }
    }
    ring::refresh();
}

/// A context menu owned by the hidden controller window (so no circle needs activation).
/// An id of 0 adds a separator. Returns the chosen id or 0.
pub fn popup(entries: &[(usize, &str)], at: POINT) -> usize {
    let ctl = app().controller.get();
    unsafe {
        let m = CreatePopupMenu();
        for &(id, label) in entries {
            if id == 0 {
                AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
            } else {
                let flags = if id == crate::desktop_view::MENU_ID { crate::desktop_view::menu_flags() } else { MF_STRING };
                AppendMenuW(m, flags, id, win::wide(label).as_ptr());
            }
        }
        SetForegroundWindow(ctl);
        let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, at.x, at.y, 0, ctl, std::ptr::null()) as usize;
        PostMessageW(ctl, WM_NULL, 0, 0);
        DestroyMenu(m);
        cmd
    }
}

pub fn circle_menu(which: Which, at: POINT) {
    let mut entries = vec![(IDM_NEW_CIRCLE, "&New circle")];
    if let Which::User(_) = which {
        entries.extend([(IDM_NEW_GROUP, "New &group"), (IDM_RENAME, "&Rename\u{2026}"), (IDM_DELETE, "&Delete circle")]);
    }
    entries.extend([(0, ""), (crate::desktop_view::MENU_ID, crate::desktop_view::MENU_LABEL), (IDM_EXIT, "E&xit DeskTuk")]);
    match (popup(&entries, at), which) {
        (IDM_NEW_CIRCLE, _) => new_circle(which),
        (IDM_NEW_GROUP, Which::User(ci)) => new_group(ci),
        (IDM_RENAME, Which::User(ci)) => begin_rename(rename::RenameTarget::Circle(ci)),
        (IDM_DELETE, Which::User(ci)) => delete_circle(ci),
        (IDM_EXIT, _) => crate::app::quit(),
        (crate::desktop_view::MENU_ID, _) => crate::desktop_view::toggle_from_menu(),
        _ => {}
    }
}

/// Adds "New circle" to the left of `from` and starts renaming it.
fn new_circle(from: Which) {
    let anchor = window_of(from);
    let dpi = anchor.map(|h| unsafe { GetDpiForWindow(h) }).unwrap_or(96).max(96);
    let m = geom::metrics(dpi);
    let r = anchor.map(win::window_rect).unwrap_or_else(|| win::rect(400, 200, 400 + m.circle_d, 200 + m.circle_d));
    let pos = circle_win::clamp_to_work((r.left - m.circle_d - win::scale(24, dpi), r.top), m.circle_d);
    let ci = {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        b.add_circle("New circle", pos)
    };
    app().save_config();
    rebuild();
    begin_rename(rename::RenameTarget::Circle(ci));
}

fn new_group(ci: usize) {
    let gi = {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        b.add_group(ci, "New group")
    };
    let Some(gi) = gi else { return };
    app().save_config();
    refresh_all();
    begin_rename(rename::RenameTarget::Group(ci, gi));
}

pub fn begin_rename(target: rename::RenameTarget) {
    let ci = match target {
        rename::RenameTarget::Circle(ci) | rename::RenameTarget::Group(ci, _) => ci,
    };
    let initial = {
        let Ok(b) = app().board.try_borrow() else { return };
        match target {
            rename::RenameTarget::Circle(ci) => b.circles.get(ci).map(|c| c.name.clone()),
            rename::RenameTarget::Group(ci, gi) => b.circles.get(ci).and_then(|c| c.groups.get(gi)).map(|g| g.name.clone()),
        }
    };
    let (Some(initial), Some(h)) = (initial, window_of(Which::User(ci))) else { return };
    rename::begin(target, win::window_rect(h), &initial);
}

pub fn commit_rename(target: rename::RenameTarget, text: &str) {
    {
        let Ok(mut b) = app().board.try_borrow_mut() else { return };
        match target {
            rename::RenameTarget::Circle(ci) => b.rename_circle(ci, text),
            rename::RenameTarget::Group(ci, gi) => b.rename_group(ci, gi, text),
        }
    }
    app().save_config();
    refresh_all();
}

fn delete_circle(ci: usize) {
    let (name, count) = match app().board.try_borrow() {
        Ok(b) => match b.circles.get(ci) {
            Some(c) => (c.name.clone(), view::circle_count(&b, ci)),
            None => return,
        },
        Err(_) => return,
    };
    let text = format!("Delete the circle \"{name}\"?\n\nIts {count} item(s) go back to Recent. No files are changed or deleted.");
    let ok = unsafe {
        MessageBoxW(std::ptr::null_mut(), win::wide(&text).as_ptr(), win::wide("DeskTuk").as_ptr(), MB_YESNO | MB_ICONQUESTION) == IDYES
    };
    if !ok {
        return;
    }
    if let Ok(mut b) = app().board.try_borrow_mut() {
        b.remove_circle(ci);
    }
    if let Some(st) = state() {
        if let Ok(mut d) = st.dismissed.try_borrow_mut() {
            d.clear(); // circle indices shifted
        }
    }
    app().save_config();
    rebuild();
}

pub fn delete_group(ci: usize, gi: usize) {
    if let Ok(mut b) = app().board.try_borrow_mut() {
        b.remove_group(ci, gi);
    }
    app().save_config();
    refresh_all();
}
