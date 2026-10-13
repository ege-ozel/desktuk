//! Application state, start-up, the hidden controller window and shell lifecycle.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, RedrawWindow, RDW_ALLCHILDREN, RDW_INVALIDATE, RDW_UPDATENOW};
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet as EmptyWorkingSet;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcess};
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Controls::{InitCommonControlsEx, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX};
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::config::{self, Args, Config};
use crate::host::{self, DesktopHost, HostKind};
use crate::items::{self, Item};
use crate::{diag, dlog, node, selftest, shell, watch, win};

pub const WM_APP_NODE_LOST: u32 = WM_APP + 1;
/// Posted by the item loader thread; lparam is a raw Box<Vec<(PathBuf, items::Meta)>>.
pub const WM_APP_ITEMS_LOADED: u32 = WM_APP + 2;
/// Shell change notification (SHChangeNotifyRegister, new delivery).
pub const WM_APP_SHELL_CHANGE: u32 = WM_APP + 3;

const TIMER_RECREATE: usize = 1;
const TIMER_EXIT: usize = 2;
const TIMER_IDLE_METRICS: usize = 3;
pub const TIMER_SELFTEST: usize = 4;
const TIMER_HEALTH: usize = 5;
const TIMER_REPAINT: usize = 6;
const TIMER_TRIM: usize = 7;

/// How often experimental hosts verify that their Explorer parent/owner still exists.
const HEALTH_INTERVAL_MS: u32 = 2000;

/// Give up on an experimental host after this long without a desktop to attach to.
const RECREATE_FALLBACK_MS: f64 = 30_000.0;

#[derive(Clone, Copy)]
pub struct Drag {
    pub cursor: POINT,
    pub origin: (i32, i32),
    pub moving: bool,
}

/// Process-wide state. Lives on the UI thread for the whole run; every field uses
/// interior mutability because window procedures re-enter freely.
pub struct App {
    pub args: Args,
    host: RefCell<Rc<dyn DesktopHost>>,
    pub controller: Cell<HWND>,
    pub node: Cell<HWND>,
    pub items: RefCell<Vec<Item>>,
    pub pos: Cell<(i32, i32)>,
    pub quitting: Cell<bool>,
    pub first_paint_done: Cell<bool>,
    pub hover: Cell<Option<usize>>,
    pub selected: Cell<Option<usize>>,
    pub tracking_leave: Cell<bool>,
    pub drag: Cell<Option<Drag>>,
    pub last_launch: RefCell<Option<(PathBuf, Result<u32, u32>)>>,
    pub taskbar_created: u32,
    pub taskbar_created_count: Cell<u32>,
    pub node_recreated_count: Cell<u32>,
    /// `diag::since_start_ms()` when the node was lost, while it is being re-created.
    lost_at: Cell<Option<f64>>,
    hook: Cell<HWINEVENTHOOK>,
    /// SHChangeNotifyRegister registration id (0 = none).
    notify_id: Cell<u32>,
}

thread_local! {
    static APP: Cell<Option<&'static App>> = const { Cell::new(None) };
}

pub fn app() -> &'static App {
    APP.with(|a| a.get()).expect("app not initialised")
}

impl App {
    pub fn host(&self) -> Rc<dyn DesktopHost> {
        self.host.borrow().clone()
    }

    pub fn save_config(&self) {
        if self.args.no_save {
            return;
        }
        let c = Config {
            host: self.host().kind(),
            pos: Some(self.pos.get()),
            items: self.items.borrow().iter().map(|i| i.path.clone()).collect(),
        };
        if let Err(e) = config::save(&c) {
            dlog!("config save failed: {e}");
        }
    }
}

pub fn quit() {
    let a = app();
    a.quitting.set(true);
    let node = a.node.get();
    if !node.is_null() {
        unsafe { DestroyWindow(node) };
    }
    unsafe { DestroyWindow(a.controller.get()) };
}

fn message_box(text: &str, error: bool) {
    let t = win::wide(text);
    let c = win::wide("DeskTuk");
    let flags = if error { MB_ICONERROR } else { MB_ICONINFORMATION };
    unsafe { MessageBoxW(std::ptr::null_mut(), t.as_ptr(), c.as_ptr(), MB_OK | flags) };
}

fn default_pos(count: usize) -> (i32, i32) {
    let mut work = win::rect(0, 0, 1024, 768);
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut windows_sys::Win32::Foundation::RECT).cast(), 0) };
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() };
    let l = node::layout(dpi, count);
    (work.right - l.width - win::scale(40, dpi), work.top + win::scale(40, dpi))
}

pub fn run() -> i32 {
    diag::mark_start();
    let args = match config::parse_args() {
        Ok(a) => a,
        Err(e) => {
            message_box(&format!("{e}\n\n{}", config::USAGE), true);
            return 2;
        }
    };
    if args.help {
        message_box(config::USAGE, false);
        return 0;
    }
    if let Some(p) = &args.log {
        diag::open_log(p);
    }
    unsafe {
        // The manifest already declares PerMonitorV2; this covers a stripped manifest.
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        CoInitializeEx(std::ptr::null(), (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32);
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_STANDARD_CLASSES };
        InitCommonControlsEx(&icc);
    }
    dlog!("DeskTuk {} starting; args={:?}", env!("CARGO_PKG_VERSION"), std::env::args().collect::<Vec<_>>());
    dlog!("windows: {}", shell::windows_version());
    dlog!(
        "display: primary={}x{} virtual={}x{} system_dpi={}",
        unsafe { GetSystemMetrics(SM_CXSCREEN) },
        unsafe { GetSystemMetrics(SM_CYSCREEN) },
        unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForSystem() }
    );
    dlog!("shell: {}", shell::describe());
    dlog!(
        "desktop icons hidden (SHELLSTATE.fHideIcons)={} registry HideIcons={:?}",
        shell::desktop_icons_hidden(),
        shell::hide_icons_registry()
    );
    if args.diag_only {
        diag::log_resources("diag");
        return 0;
    }

    if !args.allow_multiple {
        let name = win::wide("Local\\DeskTuk.SingleInstance");
        let m = unsafe { CreateMutexW(std::ptr::null(), 1, name.as_ptr()) };
        if !m.is_null() && unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            dlog!("another instance is running; exiting");
            return 0;
        }
        // The handle is intentionally kept open for the process lifetime.
    }

    // Configuration: command line > saved file > first-run defaults.
    let saved = if args.selftest { None } else { config::load() };
    let kind = args.host.or(saved.as_ref().map(|c| c.host)).unwrap_or(HostKind::Window);
    let paths: Vec<PathBuf> = if !args.items.is_empty() {
        args.items.clone()
    } else if let Some(c) = saved.as_ref().filter(|c| !c.items.is_empty()) {
        c.items.clone()
    } else {
        items::default_items()
    };
    let t_items = diag::since_start_ms();
    let loaded: Vec<Item> = paths.iter().cloned().map(Item::placeholder).collect();
    let load_paths = paths;
    dlog!("METRIC items_placeholder count={} ms={:.1}", loaded.len(), diag::since_start_ms() - t_items);
    let pos = args.pos.or(saved.as_ref().and_then(|c| c.pos)).unwrap_or_else(|| default_pos(loaded.len()));
    let first_run = saved.is_none();

    let state: &'static App = Box::leak(Box::new(App {
        host: RefCell::new(host::create(kind)),
        controller: Cell::new(std::ptr::null_mut()),
        node: Cell::new(std::ptr::null_mut()),
        items: RefCell::new(loaded),
        pos: Cell::new(pos),
        quitting: Cell::new(false),
        first_paint_done: Cell::new(false),
        hover: Cell::new(None),
        selected: Cell::new(None),
        tracking_leave: Cell::new(false),
        drag: Cell::new(None),
        last_launch: RefCell::new(None),
        taskbar_created: unsafe { RegisterWindowMessageW(win::wide("TaskbarCreated").as_ptr()) },
        taskbar_created_count: Cell::new(0),
        node_recreated_count: Cell::new(0),
        lost_at: Cell::new(None),
        hook: Cell::new(std::ptr::null_mut()),
        notify_id: Cell::new(0),
        args,
    }));
    APP.with(|a| a.set(Some(state)));
    let a = app();

    let controller = create_controller();
    a.controller.set(controller);
    items::spawn_loader(controller, load_paths);
    a.notify_id.set(watch::register(controller));
    node::register_class();

    if let Err(e) = node::create() {
        dlog!("host '{}' unavailable: {e}; falling back to 'window'", kind.name());
        *a.host.borrow_mut() = host::create(HostKind::Window);
        if let Err(e) = node::create() {
            message_box(&format!("Could not create the DeskTuk node: {e}"), true);
            return 1;
        }
    }
    if first_run {
        a.save_config();
    }

    // Foreground changes drive the "Show desktop" handling of the window host.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    a.hook.set(hook);

    unsafe {
        SetTimer(controller, TIMER_TRIM, 1000, None);
        SetTimer(controller, TIMER_IDLE_METRICS, 2000, None);
        if a.host().kind() != HostKind::Window {
            SetTimer(controller, TIMER_HEALTH, HEALTH_INTERVAL_MS, None);
        }
        if let Some(ms) = a.args.exit_after_ms {
            SetTimer(controller, TIMER_EXIT, ms, None);
        }
    }
    if a.args.selftest {
        selftest::start();
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    while unsafe { GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    unsafe { UnhookWinEvent(a.hook.get()) };
    diag::log_resources("exit");
    let code = selftest::exit_code();
    dlog!("exit code={code}");
    code
}

unsafe extern "system" fn win_event(_h: HWINEVENTHOOK, event: u32, hwnd: HWND, id_object: i32, _child: i32, _thread: u32, _time: u32) {
    if event != EVENT_SYSTEM_FOREGROUND || id_object != OBJID_WINDOW {
        return;
    }
    let a = app();
    let node = a.node.get();
    dlog!("foreground -> {hwnd:?} class={:?}", win::class_name(hwnd));
    if !node.is_null() {
        a.host().foreground_changed(node, hwnd);
    }
}

fn create_controller() -> HWND {
    let class = win::wide("DeskTuk.Controller");
    unsafe {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(controller_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(std::ptr::null()),
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc);
        // A hidden, unowned top-level window: it receives the TaskbarCreated broadcast
        // (message-only windows do not), display changes and timers.
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            win::wide("DeskTuk").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        );
        // Let TaskbarCreated through UIPI when DeskTuk runs elevated.
        ChangeWindowMessageFilterEx(hwnd, app().taskbar_created, MSGFLT_ALLOW, std::ptr::null_mut());
        hwnd
    }
}

/// The node window, or null. Clears a handle that became invalid without our window
/// procedure seeing WM_DESTROY (observed when the parent's process is killed).
fn live_node() -> HWND {
    let a = app();
    let n = a.node.get();
    if !n.is_null() && unsafe { IsWindow(n) } == 0 {
        dlog!("node handle {n:?} became invalid without WM_DESTROY");
        a.node.set(std::ptr::null_mut());
        a.drag.set(None);
        a.tracking_leave.set(false);
        return std::ptr::null_mut();
    }
    n
}

/// Periodic check for experimental hosts: re-create the node if Explorer took it down
/// or left it orphaned, without depending only on the TaskbarCreated broadcast.
fn health_check() {
    let a = app();
    if a.quitting.get() || a.host().kind() == HostKind::Window {
        return;
    }
    let n = live_node();
    if n.is_null() {
        if a.lost_at.get().is_none() {
            on_node_lost();
        }
    } else if !a.host().is_healthy(n) {
        dlog!("node {n:?} lost its Explorer parent/owner; re-creating");
        if a.lost_at.get().is_none() {
            a.lost_at.set(Some(diag::since_start_ms()));
        }
        unsafe { DestroyWindow(n) };
    }
}

fn try_recreate_node() -> bool {
    let a = app();
    if !live_node().is_null() {
        return true;
    }
    match node::create() {
        Ok(_) => {
            a.node_recreated_count.set(a.node_recreated_count.get() + 1);
            // A freshly started Explorer paints its desktop asynchronously and can draw
            // over a just-attached node; repaint once it has settled.
            unsafe { SetTimer(a.controller.get(), TIMER_REPAINT, 1000, None) };
            if let Some(t) = a.lost_at.take() {
                dlog!("METRIC node_recreated host={} after_ms={:.1}", a.host().kind().name(), diag::since_start_ms() - t);
            }
            true
        }
        Err(e) => {
            let since = a.lost_at.get().map(|t| diag::since_start_ms() - t).unwrap_or(0.0);
            if since > RECREATE_FALLBACK_MS && a.host().kind() != HostKind::Window {
                dlog!("host '{}' still unavailable after {since:.0} ms ({e}); falling back to 'window'", a.host().kind().name());
                *a.host.borrow_mut() = host::create(HostKind::Window);
                return try_recreate_node();
            }
            false
        }
    }
}

fn on_node_lost() {
    let a = app();
    if a.quitting.get() || !live_node().is_null() {
        return;
    }
    dlog!("node lost (host={}); re-creating when the desktop is available", a.host().kind().name());
    if a.lost_at.get().is_none() {
        a.lost_at.set(Some(diag::since_start_ms()));
    }
    if !try_recreate_node() {
        unsafe { SetTimer(a.controller.get(), TIMER_RECREATE, 250, None) };
    }
}

fn on_shell_restarted() {
    let a = app();
    a.taskbar_created_count.set(a.taskbar_created_count.get() + 1);
    let node = live_node();
    dlog!("TaskbarCreated (Explorer started); shell: {}", shell::describe());
    if a.host().shell_restarted(node) {
        if a.lost_at.get().is_none() {
            a.lost_at.set(Some(diag::since_start_ms()));
        }
        if !node.is_null() {
            unsafe { DestroyWindow(node) };
        }
        if !try_recreate_node() {
            unsafe { SetTimer(a.controller.get(), TIMER_RECREATE, 250, None) };
        }
    }
}

unsafe extern "system" fn controller_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let a = app();
    if msg == a.taskbar_created && msg != 0 {
        on_shell_restarted();
        return 0;
    }
    match msg {
        WM_APP_NODE_LOST => {
            on_node_lost();
            0
        }
        WM_APP_ITEMS_LOADED => {
            let metas = *Box::from_raw(lparam as *mut Vec<(PathBuf, items::Meta)>);
            {
                let mut items = a.items.borrow_mut();
                for (p, m) in metas {
                    if let Some(it) = items.iter_mut().find(|i| i.path == p) {
                        it.apply(m);
                    }
                }
            } // borrow ends before any window call
            let n = a.node.get();
            if !n.is_null() {
                InvalidateRect(n, std::ptr::null(), 0);
            }
            dlog!("METRIC items_loaded_async ms={:.1}", diag::since_start_ms());
            0
        }
        WM_TIMER => {
            match wparam {
                TIMER_RECREATE => {
                    if try_recreate_node() {
                        KillTimer(hwnd, TIMER_RECREATE);
                    }
                }
                TIMER_EXIT => {
                    KillTimer(hwnd, TIMER_EXIT);
                    quit();
                }
                TIMER_TRIM => {
                    KillTimer(hwnd, TIMER_TRIM);
                    // Start-up touches shell32/ole32/gdi code once; drop those pages from the
                    // working set so the idle footprint reflects what the Node really needs.
                    EmptyWorkingSet(GetCurrentProcess());
                }
                TIMER_IDLE_METRICS => {
                    KillTimer(hwnd, TIMER_IDLE_METRICS);
                    diag::log_resources("idle_2s");
                }
                TIMER_SELFTEST => selftest::on_timer(),
                TIMER_HEALTH => health_check(),
                TIMER_REPAINT => {
                    KillTimer(hwnd, TIMER_REPAINT);
                    let n = live_node();
                    if !n.is_null() {
                        RedrawWindow(n, std::ptr::null(), std::ptr::null_mut(), RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN);
                    }
                }
                _ => {}
            }
            0
        }
        WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
            let node = a.node.get();
            if !node.is_null() {
                node::reposition(node);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_APP_SHELL_CHANGE => {
            if let Some(change) = watch::decode(wparam, lparam) {
                // Copy paths out, compute, write back: no borrow is held across a window call.
                let mut paths: Vec<PathBuf> = a.items.borrow().iter().map(|i| i.path.clone()).collect();
                let before = paths.clone();
                let requery = watch::apply_change(&mut paths, &change);
                if !requery.is_empty() {
                    {
                        let mut items = a.items.borrow_mut();
                        for (it, p) in items.iter_mut().zip(paths.iter()) {
                            it.path = p.clone();
                        }
                    }
                    if paths != before {
                        a.save_config();
                    }
                    items::spawn_loader(a.controller.get(), requery);
                }
            }
            0
        }
        WM_DESTROY => {
            watch::deregister(a.notify_id.get());
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
