//! Conventional host: an ordinary top-level tool window. No dependency on Explorer
//! internals; always available. This is the safe default and the fallback.
//!
//! * `WS_EX_TOOLWINDOW` keeps it out of the taskbar and Alt+Tab.
//! * In desktop-only mode it is pinned to the bottom of the non-topmost z-order, so normal applications
//!   always cover it, like desktop icons.
//! * "Show desktop" (Win+D) brings Explorer's desktop window above all normal windows,
//!   which would cover the node. When the desktop becomes the foreground window the
//!   node is raised within the normal band. Restoring an application can cover it
//!   immediately, without waiting for the foreground hook to lower a topmost window.
//!   Only documented APIs are used; the "is this the
//!   desktop?" test relies on Explorer's window class names (see `shell.rs`).
//! * Circles default to persistent topmost instead, opt out of Peek fading, and
//!   reject minimization without taking foreground focus. Panel behavior is unchanged.

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_EXCLUDED_FROM_PEEK};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetForegroundWindow, IsWindowVisible, SetWindowPos, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, SWP_HIDEWINDOW,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, WINDOWPOS, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW,
    WS_POPUP,
};

use super::{CreateSpec, DesktopHost, HostKind};
use crate::{dlog, shell, win};

#[derive(Default)]
pub struct WindowHost {
    /// True while shown above the desktop in the normal (never topmost) band.
    raised: Cell<bool>,
    always_on_top: Cell<bool>,
}

/// Desktop surfaces should stay painted during Peek (distinct from Show Desktop's z-order).
pub fn exclude_from_peek(hwnd: HWND) -> bool {
    let excluded: i32 = 1;
    let result = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_EXCLUDED_FROM_PEEK as u32, (&excluded as *const i32).cast(), 4) };
    if result < 0 {
        dlog!("host[window] Peek exclusion failed: {result:#x}");
    }
    result >= 0
}

impl WindowHost {
    /// Taskbar/overflow focus is a desktop interaction, not a switch to an app.
    fn desktop_overlay(hwnd: HWND) -> bool {
        let root = unsafe { GetAncestor(hwnd, windows_sys::Win32::UI::WindowsAndMessaging::GA_ROOT) };
        matches!(
            win::class_name(root).as_str(),
            "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "TopLevelWindowForOverflowXamlIsland" | "ApplicationManager_DesktopShellWindow"
        )
    }

    fn to_bottom(node: HWND) {
        unsafe {
            SetWindowPos(node, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
        }
    }

    fn to_topmost(node: HWND) {
        unsafe { SetWindowPos(node, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER) };
    }
}

impl DesktopHost for WindowHost {
    fn kind(&self) -> HostKind {
        HostKind::Window
    }

    fn prepare(&self) -> Result<CreateSpec, String> {
        Ok(CreateSpec { ex_style: WS_EX_TOOLWINDOW, style: WS_POPUP | WS_CLIPCHILDREN, parent: std::ptr::null_mut() })
    }

    fn attached(&self, node: HWND) {
        if self.always_on_top.get() {
            Self::to_topmost(node);
            return;
        }
        self.raised.set(false);
        Self::to_bottom(node);
        // The foreground event only fires on a *change*. If the desktop is already the
        // foreground window (login auto-start, started right after Win+D, Explorer restart)
        // the node would stay below it, invisible, until the next focus change.
        let fg = unsafe { GetForegroundWindow() };
        self.foreground_changed(node, fg);
    }

    fn reveal(&self, node: HWND) {
        if self.always_on_top.get() {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    node,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE,
                )
            };
            Self::to_topmost(node);
            return;
        }
        self.raised.set(true);
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node, windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE);
            // Explicit reveal raises within the normal band, never permanently above applications.
            SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            SetWindowPos(node, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    fn capture_finished(&self, node: HWND, foreground: HWND, _was_topmost: bool) {
        if self.always_on_top.get() {
            Self::to_topmost(node);
            return;
        }
        self.foreground_changed(node, foreground);
        unsafe {
            SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            if self.raised.get() {
                SetWindowPos(node, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            } else {
                Self::to_bottom(node);
            }
        }
    }

    fn place(&self, node: HWND, r: &RECT) {
        unsafe {
            SetWindowPos(node, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    fn window_pos_changing(&self, _node: HWND, wp: &mut WINDOWPOS) {
        if self.always_on_top.get() {
            // Explorer's Show Desktop sends hide positioning to top-level windows.
            // Keep circles visible while preserving their non-activating style.
            wp.flags = (wp.flags & !SWP_HIDEWINDOW) | SWP_SHOWWINDOW;
            if wp.flags & SWP_NOZORDER == 0 {
                wp.hwndInsertAfter = HWND_TOPMOST;
            }
        } else if !self.raised.get() && wp.flags & SWP_NOZORDER == 0 {
            wp.hwndInsertAfter = HWND_BOTTOM;
        } else if wp.flags & SWP_NOZORDER == 0 && wp.hwndInsertAfter == HWND_TOPMOST {
            wp.hwndInsertAfter = HWND_TOP;
        }
    }

    fn foreground_changed(&self, node: HWND, fg: HWND) {
        if self.always_on_top.get() {
            if unsafe { IsWindowVisible(node) } == 0 || unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(node) } != 0 {
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                        node,
                        windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE,
                    )
                };
                Self::to_topmost(node);
                dlog!("host[window] restored a hidden topmost circle after foreground change");
            }
            return;
        }
        // Ignore our own node, hidden controllers, menus and the taskbar/overflow surface.
        if fg == node || fg.is_null() || unsafe { IsWindowVisible(fg) } == 0 || win::class_name(fg) == "#32768" || Self::desktop_overlay(fg)
        {
            return;
        }
        if unsafe { IsWindowVisible(node) } == 0 || unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsIconic(node) } != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    node,
                    windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE,
                )
            };
        }
        if shell::is_desktop_window(fg) {
            if !self.raised.get() {
                self.raised.set(true);
                unsafe { SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
                dlog!("host[window] desktop is foreground -> node raised");
            }
        } else if self.raised.get() {
            self.raised.set(false);
            unsafe { SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
            Self::to_bottom(node);
            dlog!("host[window] foreground={} -> node back to bottom", win::class_name(fg));
        }
    }

    fn shell_restarted(&self, _node: HWND) -> bool {
        false
    }

    fn set_always_on_top(&self, node: HWND, enabled: bool) {
        self.always_on_top.set(enabled); // before any message-pumping operation
        exclude_from_peek(node);
        if enabled {
            Self::to_topmost(node);
        } else {
            self.raised.set(false);
            unsafe { SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
            Self::to_bottom(node);
            self.foreground_changed(node, unsafe { GetForegroundWindow() });
        }
    }

    fn system_command(&self, command: usize) -> bool {
        self.always_on_top.get() && command & 0xfff0 == windows_sys::Win32::UI::WindowsAndMessaging::SC_MINIMIZE as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    struct TestWindow(HWND);
    impl Drop for TestWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }
    fn window(class: &str, parent: HWND) -> TestWindow {
        let name = win::wide(class);
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let wc =
                WNDCLASSW { lpfnWndProc: Some(DefWindowProcW), hInstance: instance, lpszClassName: name.as_ptr(), ..std::mem::zeroed() };
            RegisterClassW(&wc);
            let style = WS_VISIBLE | if parent.is_null() { WS_POPUP } else { WS_CHILD };
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                name.as_ptr(),
                name.as_ptr(),
                style,
                -30000,
                -30000,
                1,
                1,
                parent,
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null(), "native fixture window must be created");
            TestWindow(hwnd)
        }
    }

    #[test]
    fn topmost_mode_survives_desktop_app_reveal_and_capture_without_activation() {
        let foreground = unsafe { GetForegroundWindow() };
        let node = window("DeskTuk.TestPersistentTopmost", std::ptr::null_mut());
        let desktop = window("Progman", std::ptr::null_mut());
        let application = window("DeskTuk.TestTopmostApplication", std::ptr::null_mut());
        let host = WindowHost::default();
        host.set_always_on_top(node.0, true);
        assert!(exclude_from_peek(node.0), "DWM accepts the desktop surface's Peek exclusion");
        host.attached(node.0);
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node.0, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
        host.foreground_changed(node.0, desktop.0);
        assert_ne!(unsafe { IsWindowVisible(node.0) }, 0, "Show Desktop hide request must be reversed immediately");
        for fg in [desktop.0, application.0, desktop.0, application.0] {
            host.foreground_changed(node.0, fg);
            host.reveal(node.0);
            for was_topmost in [false, true] {
                host.capture_finished(node.0, fg, was_topmost);
                assert_ne!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
            }
            let mut wp: WINDOWPOS = unsafe { std::mem::zeroed() };
            wp.hwndInsertAfter = HWND_BOTTOM;
            host.window_pos_changing(node.0, &mut wp);
            assert_eq!(wp.hwndInsertAfter, HWND_TOPMOST);
            let mut hide = WINDOWPOS { flags: SWP_HIDEWINDOW | SWP_NOZORDER | SWP_NOMOVE | SWP_NOSIZE, ..unsafe { std::mem::zeroed() } };
            host.window_pos_changing(node.0, &mut hide);
            assert_eq!(hide.flags & SWP_HIDEWINDOW, 0);
            assert_ne!(hide.flags & SWP_SHOWWINDOW, 0);
            assert_ne!(hide.flags & SWP_NOZORDER, 0, "do not change band when rejecting a hide-only request");
        }
        assert!(host.system_command(SC_MINIMIZE as usize));
        assert!(!host.system_command(SC_CLOSE as usize));
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
        host.set_always_on_top(node.0, false);
        host.foreground_changed(node.0, application.0);
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
        assert!(!host.system_command(SC_MINIMIZE as usize));
        host.set_always_on_top(node.0, true);
        assert_ne!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }

    #[test]
    fn show_desktop_never_covers_a_restored_app_while_hook_is_pending() {
        let node = window("DeskTuk.TestShowDesktop", std::ptr::null_mut());
        let desktop = window("Progman", std::ptr::null_mut());
        let application = window("DeskTuk.TestRestoredApplication", std::ptr::null_mut());
        let host = WindowHost::default();
        host.foreground_changed(node.0, desktop.0);
        assert!(host.raised.get());
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
        // Restore an application before delivering any foreground notification.
        unsafe { SetWindowPos(application.0, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
        let mut above = unsafe { GetWindow(node.0, GW_HWNDPREV) };
        while !above.is_null() && above != application.0 {
            above = unsafe { GetWindow(above, GW_HWNDPREV) };
        }
        assert_eq!(above, application.0, "restored apps must cover circles without waiting for a hook callback");
        let mut wp: WINDOWPOS = unsafe { std::mem::zeroed() };
        wp.hwndInsertAfter = HWND_TOPMOST;
        host.window_pos_changing(node.0, &mut wp);
        assert_eq!(wp.hwndInsertAfter, HWND_TOP, "desktop z-order requests must not promote to topmost");
    }

    #[test]
    fn capture_finish_keeps_a_tray_revealed_circle_above_the_desktop() {
        let node = window("DeskTuk.TestCaptureHost", std::ptr::null_mut());
        let tray = window("Shell_TrayWnd", std::ptr::null_mut());
        let backdrop = window("DeskTuk.TestBackdrop", std::ptr::null_mut());
        let host = WindowHost::default();
        host.reveal(node.0);
        // A reveal uses the normal band. Capture cleanup must not blindly bury it.
        unsafe {
            SetWindowPos(node.0, backdrop.0, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
        assert_eq!(unsafe { GetWindow(node.0, GW_HWNDPREV) }, backdrop.0);
        host.capture_finished(node.0, tray.0, false);
        assert!(host.raised.get());
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
        assert_ne!(unsafe { GetWindow(node.0, GW_HWNDPREV) }, backdrop.0);
    }

    #[test]
    fn capture_finish_restores_desktop_band_but_keeps_apps_above_the_circle() {
        let node = window("DeskTuk.TestCaptureBands", std::ptr::null_mut());
        let tray = window("Shell_TrayWnd", std::ptr::null_mut());
        let application = window("DeskTuk.TestCaptureApplication", std::ptr::null_mut());
        let host = WindowHost::default();
        for was_topmost in [false, true] {
            host.reveal(node.0);
            if was_topmost {
                unsafe {
                    SetWindowPos(node.0, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
            host.capture_finished(node.0, tray.0, was_topmost);
            assert!(host.raised.get(), "returning to the taskbar keeps the circle visible");
            assert_eq!(
                unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST != 0,
                false,
                "capture must return to the normal band, even after legacy topmost state"
            );
            host.capture_finished(node.0, application.0, was_topmost);
            assert!(!host.raised.get(), "switching to an application lowers the circle");
            assert_eq!(
                unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST,
                0,
                "capture must not leave a circle above applications"
            );
        }
    }

    #[test]
    fn desktop_only_restores_a_shell_hidden_circle_on_foreground_change() {
        let node = window("DeskTuk.TestDesktopOnlyVisibility", std::ptr::null_mut());
        let desktop = window("Progman", std::ptr::null_mut());
        let application = window("DeskTuk.TestDesktopOnlyApplication", std::ptr::null_mut());
        let host = WindowHost::default();
        host.set_always_on_top(node.0, false);
        host.attached(node.0);
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node.0, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
        host.foreground_changed(node.0, desktop.0);
        assert_ne!(unsafe { IsWindowVisible(node.0) }, 0, "desktop reveal must undo Shell hide");
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
        unsafe { windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node.0, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE) };
        host.foreground_changed(node.0, application.0);
        assert_ne!(unsafe { IsWindowVisible(node.0) }, 0, "opening an app must recover a hidden circle");
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
    }

    #[test]
    fn taskbar_and_overflow_focus_preserve_desktop_order_but_apps_lower_it() {
        let node = window("DeskTuk.TestHost", std::ptr::null_mut());
        let host = WindowHost::default();
        for class in
            ["Shell_TrayWnd", "Shell_SecondaryTrayWnd", "TopLevelWindowForOverflowXamlIsland", "ApplicationManager_DesktopShellWindow"]
        {
            let tray = window(class, std::ptr::null_mut());
            host.reveal(node.0);
            host.foreground_changed(node.0, tray.0);
            assert!(host.raised.get(), "{class} must not bury the node under Explorer");
            let child = window("DeskTuk.TestTrayChild", tray.0);
            host.foreground_changed(node.0, child.0);
            assert!(host.raised.get(), "a taskbar descendant must preserve desktop ordering");
        }
        // A generic UWP window is an application, not necessarily the taskbar.
        let application = window("Windows.UI.Core.CoreWindow", std::ptr::null_mut());
        host.foreground_changed(node.0, application.0);
        assert!(!host.raised.get(), "ordinary app focus must return the node behind apps");
        assert_eq!(unsafe { GetWindowLongW(node.0, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST, 0);
    }
}
