//! EXPERIMENTAL host: a top-level popup *owned* by Explorer's `Progman` window.
//!
//! An owned window always stays above its owner, so when Explorer raises the desktop
//! for "Show desktop" the node is expected to come along. The relationship is
//! cross-process: Windows attaches the input queues of the two threads, so a hang
//! in either process can stall the other. When Explorer restarts, `Progman` is
//! destroyed and the node must be re-created against the new one.

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindow, SetWindowPos, GW_OWNER, SWP_NOACTIVATE, SWP_NOZORDER, WS_CLIPCHILDREN, WS_EX_TOOLWINDOW, WS_POPUP,
};

use super::{CreateSpec, DesktopHost, HostKind};
use crate::shell;

pub struct OwnedHost;

impl DesktopHost for OwnedHost {
    fn kind(&self) -> HostKind {
        HostKind::Owned
    }

    fn prepare(&self) -> Result<CreateSpec, String> {
        let progman = shell::find_progman();
        if progman.is_null() {
            return Err("Progman window not found (Explorer not running?)".into());
        }
        Ok(CreateSpec { ex_style: WS_EX_TOOLWINDOW, style: WS_POPUP | WS_CLIPCHILDREN, parent: progman })
    }

    fn attached(&self, _node: HWND) {}

    fn place(&self, node: HWND, r: &RECT) {
        unsafe {
            SetWindowPos(node, std::ptr::null_mut(), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    fn shell_restarted(&self, node: HWND) -> bool {
        node.is_null() || unsafe { GetWindow(node, GW_OWNER) } != shell::find_progman()
    }
}
