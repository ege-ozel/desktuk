//! Conventional host: an ordinary top-level tool window. No dependency on Explorer
//! internals; always available. This is the safe default and the fallback.
//!
//! * `WS_EX_TOOLWINDOW` keeps it out of the taskbar and Alt+Tab.
//! * It is pinned to the bottom of the (non-topmost) z-order, so normal applications
//!   always cover it, like desktop icons.
//! * "Show desktop" (Win+D) brings Explorer's desktop window above all normal windows,
//!   which would cover the node. When the desktop becomes the foreground window the
//!   node is temporarily raised into the topmost band, and dropped back to the bottom
//!   as soon as any other window becomes foreground. (The same technique Rainmeter
//!   uses for its "On desktop" mode.) Only documented APIs are used; the "is this the
//!   desktop?" test relies on Explorer's window class names (see `shell.rs`).

use std::cell::Cell;

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsWindowVisible, SetWindowPos, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, WINDOWPOS, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use super::{CreateSpec, DesktopHost, HostKind};
use crate::{dlog, shell, win};

#[derive(Default)]
pub struct WindowHost {
    /// True while "Show desktop" is active and the node is in the topmost band.
    raised: Cell<bool>,
}

impl WindowHost {
    fn to_bottom(node: HWND) {
        unsafe {
            SetWindowPos(node, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER);
        }
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
        self.raised.set(false);
        Self::to_bottom(node);
        // The foreground event only fires on a *change*. If the desktop is already the
        // foreground window (login auto-start, started right after Win+D, Explorer restart)
        // the node would stay below it, invisible, until the next focus change.
        let fg = unsafe { GetForegroundWindow() };
        self.foreground_changed(node, fg);
    }

    fn reveal(&self, node: HWND) {
        self.raised.set(true);
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(node, windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNOACTIVATE);
            // Explicit reveal raises within the normal band, never permanently above applications.
            SetWindowPos(node, HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            SetWindowPos(node, HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        }
    }

    fn place(&self, node: HWND, r: &RECT) {
        unsafe {
            SetWindowPos(node, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    fn window_pos_changing(&self, _node: HWND, wp: &mut WINDOWPOS) {
        if !self.raised.get() && wp.flags & SWP_NOZORDER == 0 {
            wp.hwndInsertAfter = HWND_BOTTOM;
        }
    }

    fn foreground_changed(&self, node: HWND, fg: HWND) {
        // Ignore our own node, invisible windows (our controller while a menu is up) and menus.
        if fg == node || fg.is_null() || unsafe { IsWindowVisible(fg) } == 0 || win::class_name(fg) == "#32768" {
            return;
        }
        if shell::is_desktop_window(fg) {
            if !self.raised.get() {
                self.raised.set(true);
                unsafe { SetWindowPos(node, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
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
}
