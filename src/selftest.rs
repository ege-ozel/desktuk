//! `--selftest`: an automated desktop-integration test that drives the real UI with
//! synthetic input (`SendInput`) and checks the outcome with hit-testing
//! (`WindowFromPoint`) and screen pixel sampling. It runs as a timer-driven state
//! machine on the UI thread so the node keeps processing messages normally.
//!
//! It moves the mouse, presses Win+D and (with `--selftest-explorer-restart`) kills
//! explorer.exe: run it on a test machine or CI runner, not while working.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, GetDC, GetDIBits, GetMonitorInfoW,
    GetPixel, MonitorFromWindow, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, CAPTUREBLT, DIB_RGB_COLORS, MONITORINFO,
    MONITOR_DEFAULTTONEAREST, SRCCOPY,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetCurrentProcessId, OpenProcess, TerminateProcess, PROCESS_INFORMATION, PROCESS_TERMINATE, STARTUPINFOW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEINPUT, VK_LWIN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::app::{app, quit, TIMER_SELFTEST};
use crate::{diag, dlog, node, shell, win};

enum Next {
    /// Run the next step after the delay (ms).
    Wait(u32),
    /// Run this step again after the delay (ms).
    Repeat(u32),
}

#[derive(Default)]
struct State {
    step: usize,
    pass: u32,
    fail: u32,
    test_wnd: HWND,
    full_wnd: HWND,
    drag_origin: (i32, i32),
    launch_fg: HWND,
    explorer_killed_at: Option<f64>,
    explorer_started: bool,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static EXIT: Cell<i32> = const { Cell::new(0) };
}

pub fn exit_code() -> i32 {
    EXIT.with(|e| e.get())
}

pub fn start() {
    dlog!("SELFTEST start host={}", app().host().kind().name());
    STATE.with(|s| *s.borrow_mut() = Some(State::default()));
    register_test_class();
    schedule(1500);
}

fn schedule(ms: u32) {
    unsafe { SetTimer(app().controller.get(), TIMER_SELFTEST, ms, None) };
}

pub fn on_timer() {
    unsafe { KillTimer(app().controller.get(), TIMER_SELFTEST) };
    // Take the state out so nothing re-entrant can observe a held borrow.
    let Some(mut st) = STATE.with(|s| s.borrow_mut().take()) else { return };
    let next = run_step(&mut st);
    let finished = next.is_none();
    if let Some(n) = next {
        match n {
            Next::Wait(ms) => {
                st.step += 1;
                schedule(ms);
            }
            Next::Repeat(ms) => schedule(ms),
        }
    }
    if finished {
        finish(&st);
    } else {
        STATE.with(|s| *s.borrow_mut() = Some(st));
    }
}

fn check(st: &mut State, name: &str, ok: bool, detail: String) {
    if ok {
        st.pass += 1;
    } else {
        st.fail += 1;
    }
    dlog!("TEST host={} {name} {} {detail}", app().host().kind().name(), if ok { "PASS" } else { "FAIL" });
}

fn info(name: &str, detail: String) {
    dlog!("TEST host={} {name} INFO {detail}", app().host().kind().name());
}

// ---- observation helpers -------------------------------------------------------

fn node() -> HWND {
    app().node.get()
}

fn hit(p: POINT) -> HWND {
    unsafe { WindowFromPoint(p) }
}

fn is_node(h: HWND) -> bool {
    let n = node();
    !n.is_null() && (h == n || unsafe { GetAncestor(h, GA_ROOT) } == n || unsafe { IsChild(n, h) } != 0)
}

fn describe(h: HWND) -> String {
    if h.is_null() {
        return "null".into();
    }
    let mut title = [0u16; 64];
    unsafe { GetWindowTextW(h, title.as_mut_ptr(), title.len() as i32) };
    format!("{h:?}/{}/{:?}/pid{}", win::class_name(h), win::from_wide(&title), shell::window_pid(h))
}

fn item_center(i: usize) -> POINT {
    node::item_icon_center(node(), i)
}

/// A point on the node's accent strip, away from (possibly rounded) corners.
fn accent_point() -> POINT {
    let r = win::window_rect(node());
    let dpi = unsafe { windows_sys::Win32::UI::HiDpi::GetDpiForWindow(node()) }.max(96);
    POINT { x: r.left + (r.right - r.left) * 3 / 4, y: r.top + win::scale(3, dpi).max(2) / 2 }
}

fn pixel(p: POINT) -> u32 {
    unsafe {
        let dc = GetDC(std::ptr::null_mut());
        let c = GetPixel(dc, p.x, p.y);
        ReleaseDC(std::ptr::null_mut(), dc);
        c
    }
}

fn is_accent(c: u32) -> bool {
    let ch = |v: u32, s: u32| ((v >> s) & 0xff) as i32;
    let close = |s| (ch(c, s) - ch(node::ACCENT, s)).abs() <= 10;
    c != u32::MAX && close(0) && close(8) && close(16)
}

fn visible_on_screen() -> (bool, String) {
    let p = accent_point();
    let c = pixel(p);
    (is_accent(c), format!("pixel@({},{})=#{:06x}", p.x, p.y, c))
}

fn cloaked(h: HWND) -> u32 {
    let mut v = 0u32;
    unsafe { DwmGetWindowAttribute(h, DWMWA_CLOAKED as u32, (&mut v as *mut u32).cast(), 4) };
    v
}

fn window_state(h: HWND) -> String {
    unsafe { format!("exists={} visible={} iconic={} cloaked={}", IsWindow(h) != 0, IsWindowVisible(h) != 0, IsIconic(h) != 0, cloaked(h)) }
}

// ---- input helpers -------------------------------------------------------------

fn mouse(flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn key(vk: u16, up: bool) -> INPUT {
    let mut flags = if up { KEYEVENTF_KEYUP } else { 0 };
    if vk == VK_LWIN {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } } }
}

fn send(inputs: &[INPUT]) -> u32 {
    let n = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if n as usize != inputs.len() {
        dlog!("SendInput sent {n}/{} (error {})", inputs.len(), unsafe { windows_sys::Win32::Foundation::GetLastError() });
    }
    n
}

fn click_at(p: POINT, double: bool) {
    unsafe { SetCursorPos(p.x, p.y) };
    let mut v = vec![mouse(MOUSEEVENTF_LEFTDOWN), mouse(MOUSEEVENTF_LEFTUP)];
    if double {
        v.extend([mouse(MOUSEEVENTF_LEFTDOWN), mouse(MOUSEEVENTF_LEFTUP)]);
    }
    send(&v);
}

fn win_d() {
    send(&[key(VK_LWIN, false), key(b'D' as u16, false), key(b'D' as u16, true), key(VK_LWIN, true)]);
}

fn activate(h: HWND) {
    unsafe {
        let ok = SetForegroundWindow(h) != 0;
        if GetForegroundWindow() != h {
            // Foreground lock: a real click on the window is always allowed to activate it.
            let r = win::window_rect(h);
            click_at(POINT { x: r.right - 60, y: r.top + 12 }, false);
        }
        dlog!("activate {} SetForegroundWindow={ok}", describe(h));
    }
}

// ---- test windows --------------------------------------------------------------

const TEST_CLASS: &str = "DeskTuk.SelfTestWindow";

fn register_test_class() {
    let name = win::wide(TEST_CLASS);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(DefWindowProcW),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: unsafe { GetModuleHandleW(std::ptr::null()) },
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: unsafe { CreateSolidBrush(win::rgb(190, 70, 60)) },
        lpszMenuName: std::ptr::null(),
        lpszClassName: name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };
    unsafe { RegisterClassExW(&wc) };
}

fn create_test_window(title: &str, style: u32, r: RECT) -> HWND {
    unsafe {
        CreateWindowExW(
            0,
            win::wide(TEST_CLASS).as_ptr(),
            win::wide(title).as_ptr(),
            style | WS_VISIBLE,
            r.left,
            r.top,
            r.right - r.left,
            r.bottom - r.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            GetModuleHandleW(std::ptr::null()),
            std::ptr::null(),
        )
    }
}

fn destroy(h: &mut HWND) {
    if !h.is_null() {
        unsafe { DestroyWindow(*h) };
        *h = std::ptr::null_mut();
    }
}

// ---- screenshots ---------------------------------------------------------------

fn screenshot(name: &str) {
    let Some(dir) = app().args.shots.clone() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path: PathBuf = dir.join(format!("{}-{name}.bmp", app().host().kind().name()));
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let screen = GetDC(std::ptr::null_mut());
        let mem = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp);
        BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT);
        SelectObject(mem, old);
        let stride = ((w * 3 + 3) & !3) as usize;
        let mut data = vec![0u8; stride * h as usize];
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: h,
            biPlanes: 1,
            biBitCount: 24,
            biCompression: 0,
            biSizeImage: data.len() as u32,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        GetDIBits(mem, bmp, 0, h as u32, data.as_mut_ptr().cast(), &mut bi, DIB_RGB_COLORS);
        DeleteObject(bmp);
        DeleteDC(mem);
        ReleaseDC(std::ptr::null_mut(), screen);
        let mut file = Vec::with_capacity(54 + data.len());
        file.extend_from_slice(b"BM");
        file.extend_from_slice(&((54 + data.len()) as u32).to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        file.extend_from_slice(&54u32.to_le_bytes());
        let hdr = std::slice::from_raw_parts((&bi.bmiHeader as *const BITMAPINFOHEADER).cast::<u8>(), 40);
        file.extend_from_slice(hdr);
        file.extend_from_slice(&data);
        let _ = std::fs::write(&path, file);
    }
    dlog!("screenshot {}", path.display());
}

// ---- explorer restart ----------------------------------------------------------

fn explorer_pids() -> Vec<u32> {
    let mut pids = Vec::new();
    for class in ["Shell_TrayWnd", "Progman"] {
        let h = unsafe { FindWindowW(win::wide(class).as_ptr(), std::ptr::null()) };
        if !h.is_null() {
            let pid = shell::window_pid(h);
            if pid != 0 && pid != unsafe { GetCurrentProcessId() } && !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

fn start_explorer() -> bool {
    let mut cmd = win::wide("explorer.exe");
    unsafe {
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessW(
            std::ptr::null(),
            cmd.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            0,
            std::ptr::null(),
            std::ptr::null(),
            &si,
            &mut pi,
        ) != 0;
        if ok {
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
        ok
    }
}

// ---- the script ----------------------------------------------------------------

fn run_step(st: &mut State) -> Option<Next> {
    let n = node();
    if n.is_null() && st.explorer_killed_at.is_none() {
        check(st, "node_exists", false, format!("node window missing at step {}", st.step));
        return None;
    }
    match st.step {
        0 => {
            diag::log_resources("selftest_start");
            let p = item_center(0);
            let h = hit(p);
            let (vis, px) = visible_on_screen();
            info("foreground_at_start", describe(unsafe { GetForegroundWindow() }));
            check(st, "initially_visible", is_node(h) && vis, format!("hit={} {px}", describe(h)));
            // Clicking the desktop outside the node must still reach Explorer.
            let r = win::window_rect(n);
            let outside = POINT { x: r.left - 60, y: r.bottom + 60 };
            let ho = hit(outside);
            info(
                "desktop_outside_node",
                format!("hit@({},{})={} desktop_tree={}", outside.x, outside.y, describe(ho), shell::is_desktop_tree(ho)),
            );
            screenshot("01-initial");
            Some(Next::Wait(200))
        }
        // -- drag the node by its header
        1 => {
            let r = win::window_rect(n);
            st.drag_origin = (r.left, r.top);
            let p = POINT { x: (r.left + r.right) / 2, y: r.top + node::current_layout(n).accent + 6 };
            unsafe { SetCursorPos(p.x, p.y) };
            send(&[mouse(MOUSEEVENTF_LEFTDOWN)]);
            Some(Next::Wait(150))
        }
        2 => {
            let (x, y) = st.drag_origin;
            let r = win::window_rect(n);
            let w = r.right - r.left;
            unsafe { SetCursorPos(x + w / 2 - 60, y + 40) };
            Some(Next::Wait(150))
        }
        3 => {
            let (x, y) = st.drag_origin;
            let r = win::window_rect(n);
            let w = r.right - r.left;
            unsafe { SetCursorPos(x + w / 2 - 120, y + 6 + node::current_layout(n).accent + 80) };
            Some(Next::Wait(150))
        }
        4 => {
            send(&[mouse(MOUSEEVENTF_LEFTUP)]);
            Some(Next::Wait(300))
        }
        5 => {
            let r = win::window_rect(n);
            let (dx, dy) = (r.left - st.drag_origin.0, r.top - st.drag_origin.1);
            check(
                st,
                "drag_moves_node",
                (dx + 120).abs() <= 2 && (dy - 80).abs() <= 2,
                format!("moved by ({dx},{dy}), expected (-120,80)"),
            );
            app().pos.set(st.drag_origin);
            node::reposition(n);
            Some(Next::Wait(300))
        }
        // -- double-click launches the first item through the shell
        6 => {
            *app().last_launch.borrow_mut() = None;
            click_at(item_center(0), true);
            Some(Next::Wait(3000))
        }
        7 => {
            let launched = app().last_launch.borrow().clone();
            let fg = unsafe { GetForegroundWindow() };
            match launched {
                Some((path, Ok(pid))) => {
                    let still_there = path.exists();
                    check(st, "double_click_launch", true, format!("path={} pid={pid} foreground={}", path.display(), describe(fg)));
                    check(st, "original_file_untouched", still_there, format!("exists_after_launch={still_there}"));
                    // Close what was opened so it doesn't cover the node in later steps.
                    if !fg.is_null()
                        && !is_node(fg)
                        && !shell::is_desktop_window(fg)
                        && shell::window_pid(fg) != unsafe { GetCurrentProcessId() }
                    {
                        st.launch_fg = fg;
                        unsafe { PostMessageW(fg, WM_CLOSE, 0, 0) };
                    }
                    if pid != 0 {
                        unsafe {
                            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
                            if !h.is_null() {
                                // Ignored if WM_CLOSE already ended it.
                                Sleep(500);
                                TerminateProcess(h, 0);
                                CloseHandle(h);
                            }
                        }
                    }
                }
                Some((path, Err(e))) => {
                    check(st, "double_click_launch", false, format!("path={} ShellExecuteEx error={e}", path.display()))
                }
                None => check(st, "double_click_launch", false, "no launch recorded (double-click not received)".into()),
            }
            Some(Next::Wait(1000))
        }
        // -- stacking: a normal application window must cover the node
        8 => {
            let r = win::window_rect(n);
            let tr = win::rect(r.left - 40, r.top - 40, r.left + (r.right - r.left) / 2, r.bottom + 40);
            st.test_wnd = create_test_window("DeskTuk self-test window", WS_OVERLAPPEDWINDOW, tr);
            activate(st.test_wnd);
            Some(Next::Wait(800))
        }
        9 => {
            let h = hit(item_center(0));
            check(st, "app_window_covers_node", h == st.test_wnd, format!("hit={}", describe(h)));
            // Click the exposed half of the node; it must not jump above the application.
            let items = app().items.borrow().len();
            click_at(item_center(items.saturating_sub(1)), false);
            Some(Next::Wait(800))
        }
        10 => {
            let h = hit(item_center(0));
            check(
                st,
                "click_does_not_raise_node",
                h == st.test_wnd,
                format!("hit={} foreground={}", describe(h), describe(unsafe { GetForegroundWindow() })),
            );
            win_d();
            Some(Next::Wait(2500))
        }
        // -- Win+D (Show desktop)
        11 => {
            let h = hit(item_center(0));
            let (vis, px) = visible_on_screen();
            check(
                st,
                "visible_after_show_desktop",
                is_node(h) && vis,
                format!(
                    "hit={} {px} foreground={} test_window[{}]",
                    describe(h),
                    describe(unsafe { GetForegroundWindow() }),
                    window_state(st.test_wnd)
                ),
            );
            screenshot("02-show-desktop");
            win_d();
            Some(Next::Wait(2500))
        }
        12 => {
            let h = hit(item_center(0));
            let restored = unsafe { IsIconic(st.test_wnd) } == 0 && cloaked(st.test_wnd) == 0;
            check(
                st,
                "below_apps_after_show_desktop_toggle",
                restored && h == st.test_wnd,
                format!("hit={} test_window[{}]", describe(h), window_state(st.test_wnd)),
            );
            screenshot("03-restored");
            destroy(&mut st.test_wnd);
            Some(Next::Wait(800))
        }
        // -- a fullscreen application must cover the node
        13 => {
            let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            unsafe { GetMonitorInfoW(MonitorFromWindow(n, MONITOR_DEFAULTTONEAREST), &mut mi) };
            st.full_wnd = create_test_window("DeskTuk self-test fullscreen", WS_POPUP, mi.rcMonitor);
            activate(st.full_wnd);
            Some(Next::Wait(1200))
        }
        14 => {
            let h = hit(item_center(0));
            let (vis, px) = visible_on_screen();
            check(st, "fullscreen_covers_node", h == st.full_wnd && !vis, format!("hit={} {px}", describe(h)));
            screenshot("04-fullscreen");
            destroy(&mut st.full_wnd);
            Some(Next::Wait(1200))
        }
        15 => {
            let h = hit(item_center(0));
            let (vis, px) = visible_on_screen();
            check(st, "visible_after_fullscreen_closes", is_node(h) && vis, format!("hit={} {px}", describe(h)));
            if !app().args.selftest_explorer {
                return Some(Next::Wait(100));
            }
            let pids = explorer_pids();
            info("explorer_kill", format!("pids={pids:?}"));
            for pid in pids {
                unsafe {
                    let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
                    if !h.is_null() {
                        TerminateProcess(h, 1);
                        CloseHandle(h);
                    }
                }
            }
            st.explorer_killed_at = Some(diag::since_start_ms());
            Some(Next::Wait(4000))
        }
        // -- Explorer restart (optional)
        16 => {
            let Some(t0) = st.explorer_killed_at else { return Some(Next::Wait(100)) };
            if !st.explorer_started && shell::find_progman().is_null() {
                st.explorer_started = true;
                let ok = start_explorer();
                info("explorer_start", format!("explorer did not auto-restart; CreateProcess(explorer.exe)={ok}"));
            }
            let elapsed = diag::since_start_ms() - t0;
            let n = node();
            let recovered = !n.is_null() && shell::desktop_windows().is_some() && {
                let h = hit(item_center(0));
                is_node(h) && visible_on_screen().0
            };
            if recovered {
                check(
                    st,
                    "survives_explorer_restart",
                    true,
                    format!(
                        "visible again {elapsed:.0} ms after kill; TaskbarCreated={} node_recreated={} shell: {}",
                        app().taskbar_created_count.get(),
                        app().node_recreated_count.get(),
                        shell::describe()
                    ),
                );
                screenshot("05-after-explorer-restart");
                Some(Next::Wait(500))
            } else if elapsed > 45_000.0 {
                let h = if n.is_null() { std::ptr::null_mut() } else { hit(item_center(0)) };
                check(
                    st,
                    "survives_explorer_restart",
                    false,
                    format!(
                        "not visible {elapsed:.0} ms after kill; node={n:?} hit={} TaskbarCreated={} shell: {}",
                        describe(h),
                        app().taskbar_created_count.get(),
                        shell::describe()
                    ),
                );
                screenshot("05-after-explorer-restart");
                Some(Next::Wait(500))
            } else {
                Some(Next::Repeat(500))
            }
        }
        _ => None,
    }
}

fn finish(st: &State) {
    diag::log_resources("selftest_end");
    dlog!("SELFTEST done host={} pass={} fail={}", app().host().kind().name(), st.pass, st.fail);
    EXIT.with(|e| e.set(st.fail as i32));
    quit();
}

#[allow(non_snake_case)]
unsafe fn Sleep(ms: u32) {
    windows_sys::Win32::System::Threading::Sleep(ms)
}
